# Site speed audit (read-only), 4 Nov 2026

The owner reported that some things on leylinelauncher.com load slowly. This audit reads the code only and changes
nothing. The cloud shell cannot reach the live site (the proxy refuses the CONNECT), so the server numbers below
come from running the portal locally (`stream/wrath_portal.py` with the repo's `stream/hub/community-index.json`, 177
entries; SOAP and the relay pointed at a closed port, so they fail at once). Production will be slower: the box is
shared with the worldserver and its bots, and the Workshop and index there are bigger. **Measure first** (section 4).

## 1. How the site is served (facts)

| Part | What | Evidence |
|---|---|---|
| Portal | One Python process, `ThreadingHTTPServer` (one thread per connection), up to 512 at once, then 503 | `stream/wrath_portal.py:4585-4620`, `MAX_CONN` at `:560` |
| Protocol | `PageHandler` leaves `protocol_version` at HTTP/1.0, so every request from Caddy is a new TCP connection and a new thread (no keep-alive) | no `protocol_version` anywhere in `stream/*.py` |
| Container | `wu-portal` runs python:3.12-alpine with **`--memory 128m`** and no CPU limit, on the same host as the worldserver | `server/deploy.sh:156`, `stream/server/stream-lib.sh:33`, `stream/fastloop.sh:346` |
| Caddy (site) | The live Caddyfile is on the server (`/opt/openvanilla-classicplus/Caddyfile`), not in the repo. Our scripts only patch it. The domain-move test fixture models the site block with `encode zstd gzip` (`tools/domain/tests/test_domain.py:40`), but **the repo cannot prove the live block compresses** | `server/deploy.sh:160-175`, `stream/fastloop.sh:378-465` |
| Caddy (stream) | `encode zstd gzip`, `/data` behind `forward_auth wu-portal:8091 /__auth` | `stream/server/stream-up.sh:130-180` |
| Game files | `wu-play` (wenilla-host, Rust/axum, 512 MB, 1.5 CPU). `--www` is served `no-cache` with precompressed .br/.gz. Caddy rewrites `/data` world files to `private, max-age=604800` and everything else in `/data` to **`max-age=600`** | `client/patches/wotlk-web-port-wip.patch:43262-43285`, `stream-up.sh:152-169` |

Memory locally: the portal uses about 42 MB after import and 47 MB after rendering pages. That is under the 128 MB cap,
but SQLite caches, the Workshop, the forum and many threads add to it in production. Check it (section 4, M7).

## 2. Likely slow spots, most likely first

### A. Every website page is big, inline and never cached (high confidence, every visitor, every click)
- Every hub-look page inlines the same ~119 KB of CSS (`hub.py:1063-1067`: `LOOK_VARS + SITE_TOKENS_CSS + SPIN_CSS +
  HUB_CSS + SITE_CSS + SIDE_CSS + STONE_CSS + ...` into `<style>`). `HUB_CSS` alone starts at `hub.py:3442`.
- Every HTML response is `Cache-Control: no-store` (`wrath_portal.py:3614`), so nothing is reused between clicks,
  not even with Back.
- Measured locally (HTML only):

| Page | HTML | gzip | Inline CSS | Inline JS | Render (CPU, local) |
|---|---|---|---|---|---|
| /wrath/leyline (front) | 170 KB | 40 KB | 122 KB | 18 KB | **67 ms** (50 ms over HTTP, warm) |
| /wrath/leyline/gallery | 157 KB | 36 KB | 119 KB | 18 KB | 29 ms |
| /wrath/leyline/catalog | 147 KB | 30 KB | 119 KB | 0 | 17-19 ms |
| /wrath/leyline/realms | 150 KB | 35 KB | 119 KB | 18 KB | 4 ms |
| /wrath (sign-in) | 153 KB | 35 KB | 123 KB | 19 KB | 2 ms |
| even a 404 | 134 KB | 29 KB | 124 KB | 0 | 2 ms |

- **If the live site block does not have `encode`, every page is ~150 KB raw instead of ~35 KB.** That is the
  first thing to check (M1).

### B. The front page rebuilds the whole catalog several times per request (high confidence, CPU and the GIL)
- `front_page` (`hub.py:2234`) calls `items()` (`:788`) directly, again in `featured_mod`→`featured_slides`→
  `gallery_tiles` (`:1746`, `:2057`), and again in `status_cards` (`:1164`). It also calls `realm_rows()` twice
  (`:2237`, `:1160`).
- Each `items()` → `index()` (`:753`) runs **`validate_index` over the whole community index again** (`:504`, about
  12 ms per call locally, 3 calls per front page). The JSON is cached by mtime (`_json`, `:733`), but the validation
  and clean-up are not. A profile of 20 front pages: 1.31 s of 1.91 s is in `validate_index`. `canonical()` runs
  10,620 times and `strptime` 12,366 times.
- `workshop()` (`:778`) serializes the Workshop's all.json to text (`workshop.py:912-934`) and parses it straight
  back, on every `items()` call.
- All of this is pure Python under one GIL. Ten visitors landing at once queue behind each other: about 0.7 s locally,
  more on the shared production CPU. The gallery and the catalog pay the same `items()` cost.

### C. Inline SOAP calls on the front page and the status bar (medium-high confidence, intermittent stalls)
- `health()` (`wrath_portal.py:3425-3435`) **holds `_health_lock` while calling SOAP**, every 10 s. Every concurrent
  request that reaches `health()` (front page `status_cards`, `hub.py:1150-1156`, and `/wrath/healthz`) waits behind
  it.
- `ac_soap` waits up to `SOAP_TIMEOUT` (8 s, `:633`) for the global `_soap_lock`, then up to 8 s for the answer
  (`:653-657`). So a busy or stuck worldserver can **stall a front-page view by up to 16 s**. When it is healthy,
  "server info" still waits for a world tick, which is slow under playerbots.
- `status_json()` (`:3513`) is better: it refreshes every 30 s and only the refreshing request waits. But it is a
  second, independent SOAP call for the same "server info" text.
- Expected symptom: the front page is usually quick, with a slow load every ~10-30 s.

### D. Signed-in pages call the website twice, in series (medium confidence, signed-in users)
- `social.cookie_user` (`social.py:1167-1192`) runs on every hub page for anyone with `wr_session`
  (`hub.py:5193-5202`, called from `get` at `:5367`). It calls `/api/me` and then `/api/play` on the website, with
  `portal_get` timeouts of 15 s each (`wrath_portal.py:707-727`). The good answer is cached for 60 s per session, but
  **a failure is not cached**: while the website is slow, every page a signed-in player opens waits for it.
- `signed_in_game_login` (`wrath_portal.py:3714-3733`) does the same two calls uncached on every `/wrath` and
  `/wrath/play` visit. `ensure_account` SOAP is cached per process (`:679-692`).
- `profiles.ways()` (`profiles.py:379-397`) fetches the website's `/login` page inline every 5 min (6 s timeout).
  It has no single-flight, so concurrent sign-in page views all fetch it.

### E. Art: mostly fine, a few heavy files, no revalidation (medium confidence)
- Art is read from disk on every request (`art_bytes`, `wrath_portal.py:1520-1526`), up to 4 MB. There is no
  ETag/Last-Modified or 304 (`art()`, `:3630-3672`). A week's `max-age` covers most of it. Home art gets 1 h, then a
  full re-download.
- Video byte ranges re-read the whole file per range request (`:3639-3651`; the 886 KB mp4).
- Front-page weight, if everything loads: about 2.6 MB of art. The page is well behaved: lazy images, width and
  height set, `preload="none"` on the video. The ones worth fixing:
  - `leyline-ui-clients.webp`, 187 KB, 1600×900: this is the **LCP hero** (`fetchpriority=high`). A 1200-wide
    version at q≈75 would be about 80-100 KB.
  - `leyline-swirl-512.png` (160 KB) and `leyline-crystal-512.png` (29 KB) are CSS backgrounds for the spinning mark
    (SPIN_CSS). A 512 WebP of the swirl would be about 30-40 KB.
  - `leyline-favicon-180.png`, 42 KB (the apple-touch icon), could be about 8 KB.
  - Hub thumbnails and `/wrath/mods/<id>/shot/0` on the live front page: check their size and caching (M3).
- Fonts: `marcellus-400.woff2`, 14.5 KB. Fine.

### F. The browser game page (wu-play): revalidation and per-file auth (medium confidence, game players)
- The wasm is about 27 MB brotli (90 MB raw). The client keeps it in Cache Storage and revalidates it with an ETag
  (patch `:53626-53680`), and `warm.html` pre-warms it from the realm list. **This part is already good.**
- The page layer is a chain of separate files, served `no-cache`, with no `modulepreload`:
  - `/__env.js` (a blocking classic script from the Python portal, `no-store`);
  - `wrath-early.js` (classic, blocking), which imports `wu-addons.js` → `wu-addons-core.js` + `ownfiles.js`, and
    `wrath-page.js` (85 KB);
  - `touch.js` (136 KB) → `input.js` + `touch-driver.js`;
  - `index.html`'s module imports `wenilla.js`, `boot.js`, `ownfiles.js` (60 KB), `sections.js`, `wasi_stubs.js`.

  That is about 20 files, each a conditional request on every visit, three import levels deep
  (`stream/server/www-patch.py:19-31`).
- **Only the files on Caddy's `@client` list skip the gate** (`stream-up.sh:146`). Every other page-layer file
  (`ownfiles.js`, `sections.js`, `mpq.js`, `chain.js`, `wrath-page.js`, `touch.js`, `wu-loading.*`, `art/*`, ...)
  and **every `/data` file** costs a Caddy `forward_auth` round trip to the Python portal first: a new TCP connection
  and a new thread (HTTP/1.0), sharing the GIL with page renders.
- The boot read-set (`boot-manifest.json`, ~100+ files, mostly interface and tables) falls under Caddy's `@dataother`
  with `max-age=600`. **Any visit more than 10 minutes after the last one revalidates the whole boot set**, each
  revalidation through `forward_auth`. The comment in `boot.js` assumes `/data` is immutable for a year; the Caddy
  override makes that untrue for those files.
- Not measured here: wenilla-host's own disk reads for `--patch` (`--only-listed`) under load (512 MB, 1.5 CPU).

### G. Smaller items
- `/` → `302 /wrath/leyline` (`server/deploy.sh:171`): one extra round trip for anyone typing the bare domain.
- `send()` runs five body rewrites per response (`wrath_portal.py:3594-3600`: brand, brand file, tab icon,
  legacy names regex over the whole body, shutdown banner). Each is ~1 ms on a 170 KB page. Small, but on every page.
- `gamedata_off()`, `shutdown_doc()` and `stream_on()` do an `os.stat`/`exists` per request (`:741`, `:481`). Cheap.
- SQLite (forum, social) has the right indexes and WAL (`forum.py:211-230`, `social.py:253-305`). Not a suspect.
- `downloads.py` (GitHub releases) refreshes on a background thread and never blocks a page (`downloads.py:125-138`).
  The community realm list and project refresh are also background (`wrath_portal.py:2054-2074`, `projects.py:756`).

## 3. Plan

### Quick wins (hours, low risk, each one deploys with `signin`/fastloop)

| # | Change | Where | Expected effect | Risk |
|---|---|---|---|---|
| Q1 | **Confirm `encode zstd gzip` is on the live leylinelauncher.com site block**; add it if missing (deploy-script edit with backup and reload, like fastloop does) | live Caddyfile | If missing: pages 150 KB → 35 KB, and the CSS/JS/SVG art shrinks too. If present: nothing | Very low. Caddy keeps the old file on a failed reload |
| Q2 | Cache `validate_index` + `index()` by the file's mtime (with the delist set); build `items()` once per request (or memo for a few seconds) and pass it down to `featured_mod`/`status_cards`/`community`; `realm_rows()` once | `hub.py:733-800`, `:1117-1167`, `:2234-2262` | Front page CPU ~67 ms → ~15-20 ms; catalog and gallery about halve | Low. The tests cover the pages; staff delist must still show at once (key the cache on the approved doc's mtime too) |
| Q3 | `health()`: don't hold the lock across SOAP. Use single-flight like `status_json`, serving the last answer while one thread refreshes; or derive it from `status_json()`'s cached result (one SOAP call per 30 s instead of two) | `wrath_portal.py:3421-3435` | Removes the periodic stalls of up to 16 s on the front page | Low |
| Q4 | Negative-cache `cookie_user` failures for ~10-15 s and cut `portal_get` timeouts for page decoration to ~3 s | `social.py:1178-1187`, `wrath_portal.py:718` | Signed-in pages stop hanging while the website is slow | Low. A sign-in shows a few seconds late after an outage |
| Q5 | `ways()`: refresh in the background (serve the last answer; first call uses the default) | `profiles.py:379-397` | No 6 s sign-in page stalls every 5 min | Low |
| Q6 | Recompress the heavy art: hero `leyline-ui-clients.webp` (1200w, plus `srcset` 1600), `leyline-swirl-512.png` → WebP, `leyline-favicon-180.png` | `stream/www-files/art`, `ART_TYPES` (`wrath_portal.py:1403`), SPIN_CSS | ~250 KB less on first view; faster LCP on phones | Very low (needs owner OK on the look) |

### Medium (a day or two)

| # | Change | Expected effect | Risk |
|---|---|---|---|
| M-a | Move the shared ~119 KB CSS out of every page into `/wrath/art/site.css?v=<hash>` (like `theme_version()` does for wu-theme.css, `wrath_portal.py:1550`), cached for a year (`immutable`). CSP already allows `style-src 'self'` | Every page after the first: ~30 KB less gzip, faster parse. The first page costs one more request | Medium. Many tests assert CSS strings in the page (`tests/test_hub.py`); a flash of unstyled page if the link fails |
| M-b | `PageHandler.protocol_version = "HTTP/1.1"` (keep-alive between Caddy and the portal). Every response already sets Content-Length; the redirects and `send_response` callers need checking | Removes a TCP handshake and a thread spawn per request, mostly for `/__auth` bursts | Medium. Any path that writes no Content-Length would hang. Needs a sweep and the full test suites |
| M-c | Add the static page-layer files (`ownfiles.js`, `sections.js`, `mpq.js`, `chain.js`, `wrath-page.js`, `touch*.js`, `input.js`, `wu-*.js/css`, `leyline-look.css`, `art/*`) to Caddy's `@client` allow-list. They are our own code and hold no game data | ~20 fewer `forward_auth` round trips per game-page load | Low-medium. Check the list holds no game data (it doesn't today); the gate still covers `/data` and `/ws` |
| M-d | Give `@dataother` boot files a longer cache with revalidation (`max-age=600, stale-while-revalidate=86400`), or move the boot manifest's interface/table set into a versioned path that can be immutable | Return visits after 10 min skip ~100+ revalidations on the critical path | Medium. A Forever UI update reaches cached players later; needs owner OK |
| M-e | ETag/Last-Modified and 304 on `/wrath/art/*`, and a cached art bytes dict keyed on mtime; true range reads for video | Fewer full re-downloads of home art after 1 h; less disk and CPU | Low |
| M-f | Raise the wu-portal memory cap to 256 MB if M7 shows it near 128 MB or restarting | No OOM restarts or swap stalls | Low (owner OK: server resources) |

### Larger (only if the measurements point here)

| # | Change | Expected effect | Risk |
|---|---|---|---|
| L-a | Short-TTL cache of rendered signed-out HTML per path (2-10 s; the nonce re-stamped per response) for the front, catalog, gallery and realms | Bursts (a Reddit or Discord post) cost one render, not N | Medium: the per-viewer bits, the nonce and the brand switch must be kept apart |
| L-b | Content-hashed names for the game page layer (`wrath-page.<hash>.js`), `immutable`, plus `<link rel=modulepreload>` in `index.html` | Repeat game loads skip ~20 revalidations and the import waterfall | Medium: touches the build and `www-patch.py` |
| L-c | Run the portal with several worker processes (or move the heavy hub pages to a pre-rendered static file refreshed in the background) | Removes the GIL bottleneck under load | Larger change; shared in-memory state (sessions, rate limits, queue) would need a store |

## 4. Measure before changing (owner or the built-in browser pane)

All read-only. M1-M6 can be pasted into the DevTools console, or run by us through the built-in browser pane on a
leylinelauncher.com tab.

1. **M1: compression and cache headers.** On a /wrath/leyline tab:
   `fetch(location.href,{cache:'no-store'}).then(r=>console.log(r.headers.get('content-encoding'),r.headers.get('cache-control')))`,
   then `performance.getEntriesByType('navigation')[0]` and compare `encodedBodySize` with `decodedBodySize`.
   Equal values (~170 KB) mean the site is not compressed (Q1). From a shell:
   `curl -sI -H 'Accept-Encoding: gzip, zstd' https://leylinelauncher.com/wrath/leyline`.
2. **M2: server time vs. network** (TTFB per kind of route). Run this 3 times:
   ```js
   (async()=>{for(const p of ['/wrath/leyline','/wrath/leyline/catalog','/wrath/leyline/gallery','/wrath/status.json','/wrath/art/stone-bar.webp']){const t=performance.now();const r=await fetch(p,{cache:'no-store'});await r.arrayBuffer();console.log(p,r.status,Math.round(performance.now()-t),'ms')}})()
   ```
   If art takes ~X ms and the front page X + 300 ms, the time is Python rendering (B). If both are slow, it is the
   network or Caddy.
3. **M3: what the front page waits on.**
   `performance.getEntriesByType('resource').map(e=>({n:e.name.split('/').slice(-2).join('/'),ms:Math.round(e.duration),kb:Math.round(e.transferSize/1024)})).sort((a,b)=>b.ms-a.ms).slice(0,15)`.
   Also `new PerformanceObserver(l=>console.log(l.getEntries().at(-1))).observe({type:'largest-contentful-paint',buffered:true})`
   for the LCP element and time.
4. **M4: periodic stalls (C).** Sample the front page every 2 s for 2 minutes and look for spikes on a ~10-30 s
   rhythm:
   ```js
   (async()=>{const out=[];for(let i=0;i<60;i++){const t=performance.now();await (await fetch('/wrath/leyline',{cache:'no-store'})).arrayBuffer();out.push(Math.round(performance.now()-t));await new Promise(r=>setTimeout(r,2000))}console.log(out.join(' '))})()
   ```
   Run it once signed out and once signed in (D).
5. **M5: concurrency (B).** Fire 10 at once and compare against the single-request time:
   `Promise.all([...Array(10)].map(()=>{const t=performance.now();return fetch('/wrath/leyline',{cache:'no-store'}).then(r=>r.arrayBuffer()).then(()=>Math.round(performance.now()-t))})).then(console.log)`.
   Times that climb in steps (100, 200, 300 ... ms) are GIL serialization.
6. **M6: the game page.** On the stream site, after character select:
   - `performance.getEntriesByType('resource')`: count the entries; count `transferSize===0` (cache hits) against
     small `transferSize` (~200-400 B, 304 revalidations) against full downloads; sum the durations of `/data/`;
   - `__wenilla_entry` and the `boot:` console lines (see `client/patches/FAST-LOAD.md`).

   Do it once on a first visit, once within 10 minutes, and once after 10+ minutes (F, M-d).
7. **M7: the server (Control `check`, read-only).**
   - `docker stats --no-stream wu-portal wu-play`: memory against 128 MB / 512 MB, and CPU;
   - `docker inspect -f '{{.RestartCount}} {{.State.OOMKilled}}' wu-portal`;
   - `uptime` (load), and whether the worldserver is pinning the CPU.
8. **M8: the Caddy access log.** Find the slowest paths: the JSON logs have `duration` and `uri`. Use the site
   block's log if one is configured, or `docker compose logs caddy` in `/opt/wenilla-realm` (read only, never `up`).
   Count 503s ("Busy. Try again", the portal's `MAX_CONN`) and 502s.

## 5. Not covered

- The website's own Rust service (`ovlive-realm:8090`, everything outside `/wrath*`) is not in this repo's Python.
  If the slow pages are outside `/wrath`, M2/M3 will show it.
- Survivors (survivors.leylinelauncher.com) has its own Caddy block (`hosting/survivors/survivors.caddy`, with
  `encode`). Not audited.
