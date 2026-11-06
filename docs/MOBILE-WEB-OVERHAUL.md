# Mobile web overhaul (6 Nov 2026)

What the owner reported from his own phone that day:

- "mobile controls on site still slow takes a few taps to enter the fullscreen version"
- "none of the tabs go to the right area either, we need a full mobile browser version overhaul"
- "you need to simulate a phone not a desktop browser"
- "or connect and tap on my phone with screenshare invent a method"

This doc covers what the phone sim found (with evidence), what changed, the timings before and after, and Phone Lab.

## Phone Lab: how to use it (for the owner)

1. On your phone, log in on the site, then open **leylinelauncher.com/wrath/phonelab** and press **Start Phone Lab**.
2. Use the site and the game as you normally would. A small red "Phone Lab" pill shows the code and has a Stop button. It switches itself off after 30 minutes.
3. Press **Stop** when you're done, or just leave it to time out.
4. On the PC, press the Control step **`phonelab`**. It prints every tap. The session and its sketches are copied to `tools\phonelab\live\`.

It records where each tap landed and on what, the touch events in order with their timings, full screen asks, errors and load times, plus a small sketch of the screen. It never records anything you type, any password, key or cookie, or anything but tap timings on the log in, key and account pages. Sessions are deleted after 24 hours.

You can also start a session on the PC and open its join link (shown on the page) on the phone.

## The phone sim

`stream/tests/phone-sim.cjs` drives headless Chromium as a real phone, not a resized desktop window:

- Playwright's iPhone 13 and Pixel 7 descriptors: mobile user agent, `isMobile` (so the page's viewport tag is honoured), `hasTouch`, the device pixel ratio and the screen size.
- Real touch input through CDP `Input.dispatchTouchEvent`: touchStart, an optional few-pixel roll, then touchEnd after a finger-like 80 ms. No mouse clicks.

Each tap returns a trace with:

- the element under the finger;
- every event the page saw, in order, with preventDefault marks;
- which page listeners ran;
- the time to the page's first response (a DOM change, a navigation or a full screen ask);
- every `requestFullscreen` call, with the event it came from and whether the browser counted it as a user gesture (`navigator.userActivation.isActive`).

It also has `swipe`, `touchscreenTap`, `setThrottle` (4x CPU and Fast 3G, the Lighthouse mobile profile), `timings` and `startPortal` (the website on loopback with fake services and a free port).

## What was wrong (evidence from the sim) and what changed

### 1. "Takes a few taps to enter the fullscreen version" (game page)

**Evidence.** `phone-fullscreen.browser.test.cjs` run against the old files (`PHONE_WWW=<old www-files>`) fails 4 of its 7 checks:

- **A tap on "Signing in…" did nothing.** Only touch.js's "Tap to play full screen" prompt asked for full screen, and that prompt only shows once the character list is up. Every tap during "Loading…" and "Signing in…" (10 s or more on a phone) was a dead tap.
- **A finger that rolled about 12 px on the prompt lost the tap.** The prompt had no `touch-action`, so the browser took the roll as a pan. The trace showed `pointerdown touchstart touchmove pointercancel touchend`: there was no `pointerup`, so touch.js's lift handler never ran. With `touch-action: none` the same roll gives `pointerup` and full screen.
- **Upright, a tap on "turn sideways" did nothing** until the character list appeared.

**Fix.**

- `wrath-early.js` (`first-tap-fullscreen` block): on a phone, the first finger lift anywhere on the page asks for full screen, then for landscape. That includes the loading screen, "Signing in…" and the character list. A "Tap anywhere for full screen" hint shows until it works.
  - The ask happens on `pointerup` or `touchend` (click is the fallback), from a trusted event, once per tap.
  - Buttons, links and form fields keep their own tap, so a file picker or the install prompt still gets its gesture.
  - It's off with `?fs=0`, after "Not now", in the installed app, with a mouse, or where the browser has no full screen (iPhone Safari keeps its "Add to Home Screen" tip).
- `touch.js`: the prompts get `touch-action: none` and ask through the same function, so one tap never asks twice. Build `wrath 2026-11-06a`.

The test checks that the first tap asks exactly once, from the lift, as a user gesture, on both the iPhone 13 and the Pixel 7 profiles.

### 2. "None of the tabs go to the right area" (website)

**Evidence** (iPhone 13 and Pixel 7, real taps, `phone-site.browser.test.cjs`; the audit scripts printed these):

- **The Menu sheet couldn't be fully reached.** It grew inside the sticky top bar and was taller than the screen: 19 tiles at 77 px each, 896 px against a 664 px iPhone 13 viewport.
  - "On your phone", "News", "Join the Discord", "Creators" and "Download the launcher" sat below the screen.
  - Swiping up on the sheet scrolled the page out from under it, leaving the header at the bottom and an empty page above. A finger then landed on whatever had moved under it.
  - The module pages' menu (/wrath/modes and others, `ly-nav`) had the same problem.
- **Links to #sections hid their heading.** /wrath/mobile "#android" and "#faq" put the heading at y=0, under the 65 px sticky bar.
- **The Mods tabs (Mods, Addons, Themes, Game modes, Loadouts) hid "Loadouts" past the right edge.** The row scrolled sideways with no scrollbar.
- **Many tap targets were under 44 px:** footer links (21 px), the footer Discord button (34 px), realm filter chips, Copy, "Source"/"See all" (30 px), carousel dots (8 px) and Share (30 px).

**Fix** (`hub.py` PHONE_CSS and `Hub._tabs`, `wrath_portal.py` SITE_CSS):

- **Menu is its own sheet.** It's fixed under the top bar and scrolls by itself, and the page behind is locked. The bar's backdrop filter is dropped while the menu is open, because it would otherwise trap the fixed sheet. Rows are 52 px with icon and label. The module pages' menu scrolls inside itself the same way.
- **A bottom tab bar on upright touch phones:** Play, Realms, Bosses, Mods and Menu (the same sheet). The current page's tab is lit. There's room under the page so the footer stays above the bar.
- **Anchors:** `scroll-padding-top` matches the bars (78 px upright, 52 px on a phone held sideways, 64 px on the module pages). `scroll-padding-bottom` keeps scrolled-to items above the tab bar.
- **Taps:** with a touch pointer, nav, tab bar, footer, Share, chips, Copy and small buttons are at least 44 px. The carousel dots and arrows get bigger hit areas. Below 640 px the Mods tabs wrap instead of hiding, and the sideways chip rows fade at the edge so you can tell there's more.

**Tested:** every Menu item is reached and a real tap opens its own page (a page that needs a login opens the log in page with it as `next`); a swipe on the sheet moves the sheet, not the page; each tab goes where it says; the tab bar is only for upright touch phones; #sections land below the bar; targets are 44 px at 360 and 390 px wide; all the Mods tabs are visible.

### 3. Slowness

**Site** (Pixel 7, 4x CPU, Fast 3G; loopback portal, no compression, so absolute numbers are pessimistic):

| Page | Before: load, bytes | After: load, bytes | First paint |
|---|---|---|---|
| /wrath/leyline (front) | 8.6 s, 1,369 KB | 6.5 s, 981 KB | about 1.35 s both |
| /wrath/mobile | 3.5 s, 434 KB | 3.0 s, 343 KB | about 1.4 s |
| /wrath/bossfights | 3.1 s, 348 KB | 2.8 s, 294 KB | about 1.4 s |
| /wrath/signin | 3.0 s, 326 KB | 2.7 s, 273 KB | about 1.36 s |

What changed:

- `stone-btn.webp` was stored lossless at 69 KB. At WebP quality 90 it's 11 KB (mean difference about 1/255), and it's on every page.
- The front page's launcher picture has a 1280 px copy for phones (srcset): 59 KB instead of 187 KB.
- The hero swirl is a WebP (image-set): 67 KB instead of 160 KB. The PNG stays as the fallback.

**Game page.** The page layer itself (wrath-early → wrath-page → touch.js and the rest, about 430 KB uncompressed) has the touch controls ready about 0.35 to 0.45 s after navigation at 4x CPU. The sim can't run the real client.

The 10 s and more of "Loading…" then "Signing in…" is the client's download and boot plus the client's login to the realm, which the page layer can't shorten. Two things change here:

- **Those seconds are no longer dead.** The first tap on them now goes full screen.
- **Phone Lab records where they go.** On the game page it sends the stage timings (`window.owStages` marks), the sign-in cover's state and the first-tap full screen counters every 15 s and when you leave the page.

The next step is one Phone Lab session from the owner's phone, signed in, from Play to the character list. It will show which stage the 10 s are in.

## Phone Lab: how it works

- **Portal:** `stream/phonelab.py`.
  - `/wrath/phonelab` is the page. Start is for staff with the admin role or above, through `admin.who`, with a CSRF token. Stop works with the session cookie alone.
  - `/wrath/phonelab/join/<code>` is the join link. `/wrath/phonelab.js` is the recorder. `/wrath/phonelab/rec` takes the batches.
  - `/wrath/phonelab/<code>.json`, `latest.json` and `sessions.json` are staff only (401 signed out, 403 for a mod). `?shots=0` leaves out the sketches.
- **Cookie:** `wu_phonelab=<code>.<ends>` on the site's parent domain, so the game page on play.<site> sees it. It lasts 30 minutes, is `SameSite=Lax` and `Secure`, and is not HttpOnly because the game page's script reads it.
- **On the site's pages:** while the browser has a live session, `PageHandler.send()` adds the recorder with the page's CSP nonce (it makes one for pages without script) and `connect-src 'self'`.
- **On the game page:** `wrath-early.js` (the `phonelab` block) loads the recorder from the site, but only when that cookie is there. It sends with `no-cors` text/plain, so there's no preflight.
- **Limits:**
  - one batch at most 160 KB, 120 records and 3 sketches;
  - one session at most 6 MB, 900 batches and 150 sketches;
  - 90 batches a minute per address and 60 per session;
  - kept for 24 hours.
- **Masking:**
  - no key or input listeners at all;
  - a form field is named only by its kind (`[input search]`);
  - token-like strings are masked on the phone and again on the server;
  - fields named like secrets (value, password, token, cookie, key) are dropped on the server;
  - paths only, no query or hash;
  - the log in, key, account and profile pages and `/__gate` give taps and timings but no words and no sketch.
- **Server data:** `$WU/portal-data/phonelab` (`/data/phonelab` in the container).
- **Reader:** `tools/phonelab/phonelab.py`. The Control step `phonelab` is read-only (READ_ONLY in control.py).

## Tests

- `stream/tests/phone-fullscreen.browser.test.cjs` (7): the first tap goes full screen; a rolled tap still counts; upright; buttons are left alone; the off switches; a desktop never.
- `stream/tests/phone-site.browser.test.cjs` (9): the Menu sheet, the tab bar, anchors, the module pages' menu, tap target sizes, the Mods tabs.
- `stream/tests/phonelab.browser.test.cjs` (4): a real tap is recorded with its event sequence and a sketch; typing never leaves the phone; the sign-in pages are recorded without words or sketches; the game page on another origin, including its full screen ask with the gesture flag; Stop.
- `stream/tests/test_phonelab.py` (41): routes, staff only, CSRF, cookie, injection and CSP, batches (caps, rate limits, masking, 410/413/429/507), join, Stop, expiry, the 24 h prune, the deploy lists.
- `tools/phonelab/test_phonelab.py` (4): the reader, the sketches, the .bat, the step.
- Still green: touch-*.browser (79), site-phone, brand, prose, hop, spin, wu-addons, loading; test_hub 585/585, test_site 546/546, test_brand 287/287, test_mobile_page 108/108.

Run them from `stream/`:

```
node --test tests/phone-fullscreen.browser.test.cjs tests/phone-site.browser.test.cjs tests/phonelab.browser.test.cjs
python3 -I tests/test_phonelab.py wrath_portal.py
```

## Deploy

- **`signin`:** the website. That's hub.py, wrath_portal.py, the new `phonelab.py` (added to the step's file list and to fastloop's backup, restore, install and self-check), and the art (`stone-btn.webp`, `leyline-ui-clients-1280.webp`, `leyline-swirl-512.webp`).
- **`page`:** the game page's layer (wrath-early.js, touch.js).
- **`ship`:** GitHub.
