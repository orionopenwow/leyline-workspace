# The static site (leylinelauncher.com on GitHub Pages)

Leyline's only server (the Hetzner box) was shut down in November 2026. With it went the Python portal
(`stream/wrath_portal.py`), the official realm, the relay and the Discord bridge. The website now is a folder of plain
files on GitHub Pages, and Leyline is a launcher that lists other people's servers, runs single player and Your Den on
your PC, and installs mods from their authors.

## How it builds

```
python3 tools/site-static/build.py --out _site          # the whole site, about 9 MB, in a few seconds
python3 tools/site-static/build.py --out _site --list   # the same, then every file and its size
python3 tools/site-static/test_build.py                  # the checks (builds twice: about a minute)
```

`build.py` loads the old portal and the modules beside it **inside its own process**, serves them on 127.0.0.1 only,
with empty data folders, no SOAP, no database and **no network** (every connection off the machine is refused, and
proxies are switched off), asks it for each read-only page and feed, and saves the answers at the **same URL paths**.
Launchers already installed ask for `https://leylinelauncher.com/wrath/mods/all.json` and get the file
`wrath/mods/all.json`; a page such as `/wrath/docs/mods` is saved as `wrath/docs/mods/index.html` (Pages answers
`/wrath/docs/mods` with a redirect to `/wrath/docs/mods/`). Query strings are ignored by Pages, so
`/wrath/leyline/mods.json?sort=hot&limit=1000` is the file `wrath/leyline/mods.json` with the full list.

Then it tidies every page: the frame's account chip, Play button, status dot, phone hint and ratings go; the menu is
the static one; every link to a page that no longer exists is taken out (in the docs the words stay, unlinked); forms
that posted to the server go; the CSP nonces go (no CSP on Pages). Same inputs give the same bytes (the feeds carry
`--stamp`, `SOURCE_DATE_EPOCH`, or the newest input file's time).

The front page, Start here, Realms and the 404 page are drawn by `build.py` itself: the portal's versions were about
the server. Copy in the owner's own voice is marked `TODO(owner)` in `build.py` and `site/docs/`.

## The inputs

| File | What it is |
|---|---|
| `site/realms.json` | the realm list, curated by hand (empty to start; how to add a server: `site/README.md`) |
| `site/news.json` | the News posts (empty to start) |
| `site/docs/*.md` | short pages that replace the docs pages about the server (`build.py` RETIRED_DOCS) |
| `site/hub-approved.json` | the hub index's approved and delisted repos (it was the admin console's `approved.json`) |
| `stream/hub/community-index.json` | the community mod index (the daily hub-index workflow, merged by a person) |
| `stream/upstream-addons.json`, `stream/looks.json` | the approved one-click addons and looks (only reviewed ones show) |
| `docs/leyline/*.md` | the docs |
| `stream/www-files/art/...` | the art the pages use and the Looks pictures |

The build refuses (exit 1, nothing written) a realm or news entry with a game or maker name, a version number in its
text, a money or donation word, a link that is not plain `https://`, a link to the old server, or an unknown field.

## What is static

**Feeds** (what launchers read):

| Path | Content |
|---|---|
| `/wrath/leyline/flags.json` | every feature flag **off** (p2p, torrents, mesh, relays, Hero Key, magnet links, sharing, Be a node), no STUN servers, hashed as `p2p/flags.rs` checks it |
| `/wrath/leyline/news.json` | from `site/news.json` |
| `/wrath/leyline/mods.json` | the mod rankings feed (the whole list; install counts are zero now) |
| `/wrath/leyline/catalog.json`, `/wrath/mods/all.json` | the catalog: approved upstream addons, approved looks, the community index and the Host pack (on GitHub releases). Leyline's own Workshop mods are left out: their downloads were on the server |
| `/wrath/modes.json` | `{"schema": 1, "modes": []}`: every mode ran on the official realm (the launcher has its own copy built in) |
| `/wrath/realms.json` | from `site/realms.json`, in the shape the launcher's `realms.rs` reads (see below) |
| `/wrath/status.json` | the official realm: `"online": false, "retired": true` |
| `/wrath/api/boards/index.json` | the leaderboards index, empty |
| `/wrath/docs/search.json` | the docs search index |

**Pages**: `/` and `/wrath/leyline` (the front page), `/wrath/start`, `/wrath/download`, `/wrath/leyline/realms`,
`/wrath/leyline/catalog` and `/wrath/leyline/addons` (with their filter links as pages of their own, such as
`/wrath/leyline/catalog/sort-new/`), every item page `/wrath/leyline/p/<id>`, `/wrath/looks`,
`/wrath/leyline/creators`, `/wrath/leyline/agents`, `/wrath/leyline/news`, `/wrath/docs` and every docs page,
`/wrath/docs/search` (it searches `search.json` in the browser), `/404.html`, `/robots.txt`, `/sitemap.xml`, `CNAME`
(`leylinelauncher.com`), `.nojekyll`, `favicon.png`.

**Pages that move on** (a small page with a refresh): `/wrath/realms` and `/wrath/mods` to their new pages; the old
preview addresses; `/wrath/leyline/submit` to the "Submit a mod" GitHub issue form; `/wrath/leyline/forum` to the
Discord; `/wrath/bugs` to GitHub issues; and the closed pages (`/wrath`, `/wrath/play`, sign-in, profile, the Hero
Key, leaderboards, boss fights, arena, Survivors, the Den market, the phone page, modes, Host) to the front page,
which says what closed.

## What the launcher fetches that can't be static

`tools/site-static/test_build.py` reads every site address in `launcher/src-tauri/src` (`site!("...")` and the path
constants) and `launcher/ui` (`LeylineAddresses.site(...)`, literal addresses), and fails when one is neither a file
in the site nor in this list. These needed the server; on the static site they are a 404 (or a page that moves on):

| Address | What it was |
|---|---|
| `/wrath` | Play in the browser and the account page (a page that moves to the front page) |
| `/wrath/play` | the browser game |
| `/wrath/profile` | profiles |
| `/account/key` | the Hero Key |
| `/auth/google/` | Google sign-in |
| `/wrath/link` | linked launchers |
| `/wrath/leyline/forum/new` | a new forum topic |
| `/wrath/forum/api` | the forum's API |
| `/wrath/api/` | every API under it: social (friends and messages on the website), listings (submissions), dens (the Den market), boards (local runs, POST), hub (install counts, ratings, me) |
| `/wrath/api/social` | friends and messages on the website |
| `/wrath/api/listings` | listing submissions |
| `/wrath/api/dens` | the Den market's API |
| `/wrath/mods/list` | List your mod (a form) |
| `/wrath/dens` | the Den market |
| `/wrath/bugs/api/report` | bug reports (POST) |
| `/wrath/admin` | HQ, the admin console |
| `/wrath/host` | the Host page on the website |
| `/wrath/realms/report` | report a realm |
| `/wrath/r/` | share links |
| `/wrath/avatar/p/` | profile pictures |
| `/wrath/art/leyline-shot-backdrop.webp` | the owner's backdrop picture: not ours to publish, so not on the site |

Also gone, not static: the relay (`relay.leylinelauncher.com`: the realm listing, join pages, invites, knocks), the
release mirror (`/wrath/leyline/releases/`: launchers update from GitHub first anyway), `/wrath/leyline/herokey.json`,
`/__auth`, `/__gate`, `/__env.js`, the browser game's files and every slice.

## What was dropped from the site

Sign-in (Hero Key, Google, Log in, account, profile), the forum, social, the browser game and its play and queue
pages and slices, Azeroth Survivors, the leaderboards and boards, boss fights, duels and arena pages, the Den market
and sharing, HQ (admin), the mod and listing submission forms (Submit a mod is a GitHub issue now), ratings and
install counts, the status pages and the status dot, the official realm and its Play now, Host a Beacon, the phone
page, Phone Lab, Leyline Solo's page, the gallery, Forge's page, the modes pages, guides and press pages, the web
armory and share links. The docs pages about the server (`index`, `getting-started`, `play`, `network`, `hero-key`,
`mobile`, `be-a-node`, `join-network`, `always-on`, `wrath-unleashed`, `host-for-friends`) are short pages from
`site/docs/` now; every other docs page that still mentions what closed carries a note under its heading.

## The realm list's shape

`/wrath/realms.json` keeps the network listing's schema-1 records, so `realms.rs parse_community_in` keeps each
server: `community.realmId` (16 base32 characters, made from the address), `community.relay` (the realm list host),
`profile.port` (3724 unless given), `profile.build` and `profile.version`, `state: "live"` (the launcher checks each
server itself; there are no live numbers), `discord`, `tags`, `region`, `language`. New fields for a launcher that
reads them: `realmlist` (as players type it), `website`, `listed: true`, and `connect.kind: "website"`. There is
**no `sample` block and no official realm**. Note for the launcher: `realms.rs officials()` still adds the official
realm itself, and `REPORT_BASE` points at a page that is gone.

## Publishing

* The site is built and deployed by `.github/workflows/leyline-site.yml` in the **public** repo
  `orionopenwow/Leyline` (source copy: `tools/github-workflows/leyline-site.yml`): on a push to `main` that changes the
  site's inputs, and by hand (Actions › leyline site › Run workflow). It runs the tests, builds `_site`, checks for game
  data, then `actions/upload-pages-artifact` and `actions/deploy-pages`. Both jobs run only in `orionopenwow/Leyline`:
  Leyline-Launcher gets the same export and keeps its own Pages site.
* Control's `leyline` step (`tools/leyline-repo/export.py`) exports what the build needs: `tools/site-static`,
  `site/`, this page, `mods/categories.json`, the portal modules the pages come from (never the sign-in, forum, social,
  admin, queue or browser-game ones), the community index, the art, and the workflow itself (`EXTRA`, the one file
  written under `.github`). Pushing a workflow file needs the `workflow` scope on the sign-in the step pushes with: if
  the `leyline` step's push is refused for `.github/workflows/leyline-site.yml`, run `gh auth refresh -s workflow`
  once and press `leyline` again. "Turn on GitHub builds.bat" also installs it (`tools/github-builds`, `LY_FILES`).
* So a change reaches the site like this: edit `site/realms.json` (or the docs, or merge the hub index's PR), `ship`,
  and the `leyline` step's push starts the Pages build.
* The hub-index workflow (`hub-index.yml`, in the workspace) reads `site/hub-approved.json` instead of the old site's
  `approved.json`; its merged refreshes reach the site with the next export. `realm-scripts.yml`'s deploy job (SSH to
  the old server) is off (`if: false`); its tests still run.

## The owner's one-time steps

1. **Verify the domain** (recommended by GitHub, so no other account can claim it): github.com › your profile
   Settings › Pages › Add a domain › `leylinelauncher.com`; add the TXT record it shows at your DNS provider, then
   Verify.
2. **Turn on Pages** on `orionopenwow/Leyline`: Settings › Pages › Build and deployment › Source: **GitHub Actions**.
3. **Run the site once**: Actions › leyline site › Run workflow (or press `ship` after this branch is merged). Its
   `deploy` job shows the address when it is done.
4. **Custom domain**: Settings › Pages › Custom domain: `leylinelauncher.com` › Save. Do this **before** the DNS
   change (GitHub's order). The site's `CNAME` file says the same.
5. **DNS** at your DNS provider, for `leylinelauncher.com` (remove the old A/AAAA records that pointed at the Hetzner
   server first, and any wildcard record):

   | Name | Type | Value |
   |---|---|---|
   | `@` | A | `185.199.108.153` |
   | `@` | A | `185.199.109.153` |
   | `@` | A | `185.199.110.153` |
   | `@` | A | `185.199.111.153` |
   | `@` | AAAA | `2606:50c0:8000::153` |
   | `@` | AAAA | `2606:50c0:8001::153` |
   | `@` | AAAA | `2606:50c0:8002::153` |
   | `@` | AAAA | `2606:50c0:8003::153` |
   | `www` | CNAME | `orionopenwow.github.io` |

   (Values from GitHub's "Managing a custom domain for your GitHub Pages site", checked 26 Nov 2026.) GitHub then
   sends `www` to the apex by itself. Delete the records for `relay.leylinelauncher.com` and
   `survivors.leylinelauncher.com` (nothing answers there now; a record pointing at an IP someone else may get
   later is a takeover risk).
6. **HTTPS**: when the certificate is ready (up to a day), tick **Enforce HTTPS** in Settings › Pages.
7. **play.openvanilla.app** (the old name): Pages takes one custom domain per repo, and `openvanilla.app` itself
   is already Leyline-Launcher's Pages site. Two choices:
   * simplest: at the DNS provider, a URL forward (301, keep the path) from `play.openvanilla.app` to
     `https://leylinelauncher.com`, if your provider offers one; old bookmarks and old launchers' feeds then land on
     the new site;
   * or delete the `play.openvanilla.app` record (it pointed at the old server): old addresses stop answering.

   Do not point it at `orionopenwow.github.io` without a repo that claims it: an unclaimed name there is a takeover
   risk.

## Sizes

About 470 files and 9 MB: 306 pages (64 of them catalog filter pages, about 200 item pages), 11 feeds, the art the
pages use (the community index's preview pictures are most of it).
