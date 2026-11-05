# Shared Dens: the builds marketplace

Players share what they built in Your Den: a whole **Den**, or a **stamp** (a small ready-made place, up to 60
pieces within 40 yards of its middle). Others browse them, open one in their own Den, or place it as a stamp. Every
share is checked by a Leyline moderator before anyone else sees it. Every build shows its maker, what it was remixed
from, and "Made with Leyline".

**The name.** The launcher tab is **Dens** (Mods & Addons › Dens), not "Worlds": it is what the side menu's Your Den
makes, and "worlds" reads like realms. The website pages are `/wrath/dens`.

**The switch.** `WU_DENS` on the portal, **off by default until the grand release**:

| `WU_DENS` | API (`/wrath/api/dens`) | Pages (`/wrath/dens`) |
|---|---|---|
| unset / `0` | 404 `not_open` (the launcher tab says "Shared Dens open with the grand release") | 404 |
| `beta` | on (the launcher tab works, for testing) | 404 |
| `1` | on | on |

Staff can always open `/wrath/dens/review` (what waits, with its pictures), whatever the switch says.

## Where things are

| Part | File |
|---|---|
| Server: API, storage, moderation, pages | `stream/denmarket.py` (wired in `stream/wrath_portal.py`; Needs you in `stream/hq.py`) |
| Launcher core | `launcher/src-tauri/src/denmarket.rs` (commands `dens_*`), `deeplink.rs` (`leyline://den/<id>`) |
| Launcher tab | `launcher/ui/denmarket.js`, `denmarket.css`; Your Den › Saved Dens › Share leads here (`den.js`) |
| Den Lite | `labs/den-lite/src/den.rs` (credits, `stamps-shared`), `showcase.rs`, `world.rs` (`run_showcase`) |
| In-game | `WrathUnleashedUI/DenBar.lua` (`D.Showcase`) |
| Tests | `stream/tests/test_denmarket.py` (93), `launcher/tests/denmarket.browser.test.cjs` (5), Rust tests in `denmarket.rs`, `deeplink.rs`, `den.rs`, `showcase.rs`, `stream/tests/test_wuui_denbar.lua` (showcase) |

## The flow

1. **Share** (launcher, logged in with the Hero Key). Pick a saved Den, Den or stamp, a title (3 to 48), what it is
   (600), up to 5 tags from a fixed list, a credit line (80, shown under the maker's name) and a licence:
   CC BY, CC BY-SA, CC BY-NC, CC0, or **Look only** (others may open it, but a remix of it can't be shared by anyone
   else). Pictures: **Take pictures in the game** (below) or **Add your own** (JPEG/PNG, 900 KB each, 4 at most).
2. **Waits.** The build is `pending`. Leyline HQ › Needs you lists it (Approve / Reject with a reason the maker reads),
   with a link to `/wrath/dens/review/<id>` to see its pictures. **Nothing is ever approved by itself.**
3. **Live.** In the catalogue (launcher and website), with Featured on top (HQ: `den.feature`).
4. **Open in my Den.** The launcher reads the build's sha256 from its detail, downloads its file (512 KB at most),
   checks the sha256 and the Den file rules (`host::dens::check`), adds it to Saved Dens (named from its title) and
   opens it when you're in your Den (the old Den kept as `backup`), like Add from a link.
5. **Add as a stamp** (builds that fit a stamp): the file goes to Den Lite's `<dens>/stamps-shared/<name>.den.json`;
   `.den stamp <name>` or "Jaina, build <name>" places it.
6. **Update.** A pending or rejected build changes in place and waits again. A **live** build's update waits as
   `next.json` / `next-N.jpg` while the old version stays up; HQ's "Approve the update" swaps it in (rev + 1).
7. **Unpublish** (the maker): taken down, files deleted; copies people opened stay theirs, remixes keep its name.
8. **Like** once per player; **opened** counts once a day per player (or address); **Report** (a reason from a list)
   goes to Needs you first (Dismiss / Take it down).

## Credit

- The file a player downloads always carries `"credit"`: `{id, title, by, line, licence, based_on: [{id, title, by}...],
  made_with: "Made with Leyline", url}`, put in by the server at download time (whatever the uploaded file said).
- **Remix line.** When a shared file still has a `credit` (you opened a shared Den, built on, shared it) the new build
  is "based on" it; or the player names `based_on`. The chain is walked up to 8 deep; a taken-down original keeps
  its title and maker. Look-only builds refuse other people's remixes.
- **Stamps keep credit in someone else's Den.** When Den Lite places a stamp (or adds a Den beside the one there)
  that has a `credit` or `credits`, the makers join that Den's `credits` list (each once, up to 32) and every piece
  gets `"credit": <index>`. The list survives save, open and add, so sharing that Den on names everyone.
  (The Docker Den's realm Lua, `ov_den.lua`, does not keep these fields yet: only Den Lite does.)

## Pictures (the showcase)

"Take pictures in the game" (Den Lite, in your Den with the build open):

1. The launcher writes `<dens>/showcase.json` (`{id, file}`); Den Lite's open.json watcher picks it up.
2. Den Lite puts its far-camera object over the middle of the props (`showcase::pose`, zoom 8 to 50 yards) and sets
   the view on it, then tells the Den bar `showcase` `4|1800|<zoom>`.
3. The Den bar hides the interface, sets JPEG screenshots (quality 6, restored after), zooms out, and takes
   `Screenshot()` four times with a quarter turn (`FlipCameraYaw(90)`) after each, then shows the interface again.
4. Den Lite gives the view back and writes `showcase-done.json`. The launcher shows the newest screenshots since its
   ask from the game folders' `Screenshots` (`find_shots`); the player removes any they don't want.

Only files directly in a game folder's `Screenshots` (jpg/png, 900 KB) can be sent back (`shot_ok`), so nothing
else on the PC can leave. Not in the Den: the answer says so and the UI asks to go in first.

## Checks and limits

- Den file: `stream/listings.py den_problems` (the Den loader's rules) plus the `credit`/`credits` shapes; 508 KB on
  upload (room for the credit); a stamp: 60 pieces within 40 yards.
- Text: control characters out, the brand words out (`hub.neutral` plus a last check); no client numbers on pages.
- Pictures: JPEG/PNG magic, 320x180 to 3840x2160, 900 KB, 4. Body 7 MB for a share (**Caddy's request size limit
  must allow it**: check before switching on).
- 6 shares an hour per player; site-wide sanctions (`admin.blocked("submit")`) stop shares and updates.
- Pages have no script; the showcase's Like/Report are plain forms with a CSRF token.

## Data

`$WU_PORTAL_DATA/dens` (or `WU_DENS_DIR`): `dens.db` (sqlite: builds, likes, downloads, reports, shares), `files/<id>/`
(`den.json`, `N.jpg|png`, and `next.*` for a waiting update), `secret` (the forms' CSRF key). A Den file is a layout
only (object entries and positions): no game file is ever stored or served. Players' screenshots are their own
pictures of their own builds.

## Open for the owner

- TODO(owner): the catalogue's one line on `/wrath/dens` (a draft is in `catalogue_page`).
- Switch on: `WU_DENS=beta` to try the launcher tab, `WU_DENS=1` at the grand release; check Caddy's body limit.
- Live check: the showcase in the real game (FlipCameraYaw and the hidden interface in a 3.3.5a client; the far
  camera's tilt is the player's own).
