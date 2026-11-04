# Mods & Addons, every day

Owner, 2 Nov 2026: "add as many mods and graphics settings to the mods and addons discovery thing and make a plan to
expand it daily".

**Headline: about 10 new reviewed-ready items a day (5 to 8 addons, 2 to 4 projects or graphics finds), found and
checked by machines, published only after the owner says yes in the Admin Console.**

## What there is (2 Nov 2026)

| List | File | Who publishes it | Today |
|---|---|---|---|
| Upstream addons (one click in the launcher) | `tools/addon-curate/seeds-addons.json` → `mods/upstream.json` → `stream/upstream-addons.json` | the owner's `approve` (Admin Console) | 99 entries: 29 approved, 70 pending (44 added today); 19 exclusions |
| Community index (link-out projects, site and launcher Discover) | `tools/hub-index/seeds.json`, `candidates.json` → `stream/hub/community-index.json` | the daily gate's review PR | 234 seeds (48 added today), 337 candidates (33 added today) |
| External HD packs (link-only) | `tools/hub-index/external-packs.json` → the index's `external` list | the owner, by removing the pack's `skip` line | 1 listed, 2 waiting (added today) |
| Looks (graphics packs, UI looks, graphics presets) | `stream/looks.json` | `tools/looks/looks.py approve` | 14 looks: Performance and High presets added today, waiting |

"Graphics settings" here means three different things:
1. **Graphics presets**: the launcher already writes the game's own `Config.wtf` (`launcher/src-tauri/src/gfx.rs`, the
   Mods & Addons › Graphics tab): Low, Classic, Balanced, High and Ultra, each value checked against a range, with a backup
   and "Back to how it was". The site's Looks showed only three of them; Performance (Low) and High are now looks
   too (pending), and `stream/tests/test_looks.py` checks that every gfx.rs preset has a look.
2. **Graphics projects**: renderers and translation layers (DXVK, the Modern Renderer and its original), in the
   index's `graphics` category. ReShade and the HD font DLL are recorded as skips with the reason (see TODO(owner)).
3. **HD packs**: client patch files, so link-only and never entries (owner, 2026-10-30).

## The daily run

Two halves. The cloud half already exists; the PC half is new (`mods-daily`).

| When (UTC) | Where | What | Writes | Alone or owner |
|---|---|---|---|---|
| 05:43 | GitHub Actions, `scout.yml` | `scout.py run --probe 40`: new and rising repos per area (addons, graphics, tools, cores, bots), Codeberg, GitLab, r/wowservers links; lures and rule-breakers rejected with reasons | `docs/scout/<date>.md`, `scout-state.json`, finds into `candidates.json` | alone; opens one PR, never merges |
| 07:17 | GitHub Actions, `hub-index.yml` | `hub_index.py build --thumbs 30`, then `gate`: stars, release and last-change refresh for listed entries | `stream/hub/community-index.json`, thumbnails | refreshes alone; **new entries, removals and other changes wait** in the review PR |
| owner's choice, once a day | owner's PC, Control step `mods-daily` (`tools/hub-index/Mods daily.bat`) | `curate.py run` (every addon seed: new ones curated, approved ones get a `pending_update` when upstream moved), `hub_index.py thumbs --new`, `hub_index.py check`, `looks.py check`, `curate.py list` | `mods/upstream.json`, thumbnails | alone; **approves nothing** |
| when the owner has a few minutes | Admin Console (Mods) | approve or leave pending: addons (`approve_entry`), updates of approved addons (the diff is shown), looks, the gate PR, external packs | `stream/upstream-addons.json`, `stream/looks.json` | **owner only** |
| after approving | Control: Ship, then `signin` | puts the approved lists on the site and in the launcher | | owner |

What machines may do alone: search, probe (shallow clones, deleted straight after), record licences and hashes, add
seeds and candidates as **pending**, refresh numbers (stars, last change, release) of entries already listed, make
thumbnails from GitHub's own preview images, and write skips and exclusions with a reason.

What always waits for the owner: anything new becoming visible to players, an approved addon's new files, a new
look, an external pack (its `skip` line comes out only by the owner), a waiver of any rule, and any removal that is not
a hard rule (the repo is gone, a game-format file appeared, a blocklisted owner).

### Seeding (the part a person or agent does each day)

The tools find; a seed is our own words. Each day an agent (a scheduled cloud task, see below) takes the scout digest
and the curate failures, and:
1. picks the best 5 to 8 addons (TOC `## Interface:` for a supported build, an author's own repo, not a re-upload
   collection) and writes seeds with our name, our one-sentence description and tags from mods/SPEC.md;
2. picks 2 to 4 projects (graphics, tools, realm modules) for `seeds.json` with category, type, builds and `found`;
3. runs `curate.py run --only <new ids>` and the hub tests, and opens one PR. Nothing is approved.

## How many a day

| | Aim | Ceiling | Why |
|---|---|---|---|
| New addon seeds | 5 to 8 | 15 | each needs a look at its art (.blp) and licence; more than the owner can review in 10 minutes is noise |
| New index seeds / candidates | 2 to 4 seeds, up to 20 candidates (scout) | 40 candidates | the gate holds the rest |
| Graphics finds (projects, presets, packs) | 1 a week | | the pool is small; quality first |
| Approved updates of approved addons | all that pass | | updates keep the approved install until approved |

The pool of credible 3.3.5a and 1.12 addon repos found today is about 600; at 5 to 8 a day the backlog lasts months, so
the number of good items, not the search, is the limit. When a week's yield drops under 3 a day, widen the sources
(below) before lowering the bar.

### Catch-up sweeps (by hand, when the pool needs to grow)

Two tools search wider than the daily run (owner, 3 Nov 2026: "as many GitHub mods and extensions as you can"). Both
add **pending** items only.

- `python3 -I tools/hub-index/scout.py run --wide --max-pages 3 --probe 2500 --jobs 8`: every area's queries plus
  `WIDE_QUERIES` (launchers and addon managers, realm modules for AzerothCore, TrinityCore, CMaNGOS and VMaNGOS, bots,
  addons and UI for the three classic clients, tools, editors and sites, web and mobile clients, graphics), all-time
  with 3+ stars. A find only the sweep saw joins `candidates.json` only once probed and judged listable, or held for a
  licence or a person.
- `python3 -I tools/addon-curate/curate.py discover --limit N`: repo searches, the TOC's `## Interface: 30300` line by
  code search, and the repos of authors who backport often; forks, collections, the hub blocklist and hard rules and
  what we leave out by choice (one-key rotations, native code, patched clients, paid guides) stay out; each find must
  pass the launcher's rules (`analyze`) before it becomes a seed with our words and `found: "discover <date>"`. Its
  generated description is a draft: read it before approving. The daily default `--limit` is 15, the ceiling above.

The first sweeps (3 Nov 2026): 557 new pending hub candidates (`docs/scout/2026-11-03.md`) and 389 new pending addon
seeds (plus 11 new `exclude` lines: the game's own interface files, a paid guide viewer, templates, a spammer, reroll
automation, collections). Of the 389: 241 state no open licence, 47 are another edition of an addon already seeded
(their notes say which), 20 are ElvUI plugins, and 247 still carry a placeholder description that needs our words
before approving (their notes say so). It also fixed three rules: other games (Minecraft,
Factorio, Fallout) and one-key rotation addons are left out by the index, a weak build word is not its own game
signal, and a database password in a README's config example is not a lure.

## Sources

- GitHub search (`gh api search/repositories`, 30 a minute with a token): "3.3.5a addon", "wotlk addon",
  "1.12.1 addon", "vanilla addon", `topic:wotlk-addon`, `topic:vanilla-addons`, `topic:azerothcore-module`, addon names
  plus "3.3.5" (for backports), "reshade", "dxvk", "hd patch". The scout's `AREAS` hold the standing list.
- Known authors who backport often: bkader, s0h2x, NoM0Re, KhalGH, Zidras, refaim, EinBaum, shagu, tdymel, Drakensangs.
- Codeberg and GitLab (scout), and r/wowservers' public feeds (scout, links only).
- Awesome lists and wikis as leads only, never as install sources: they point to repos we then check one by one.
- For graphics: the renderer projects' own READMEs (they name their peers), ReShade preset repos, DXVK's wiki.
- Never: re-upload collections, file hosts (Mega, Google Drive, MediaFire), anything that bundles a game client or a
  game executable.

## Quality and safety rules (all already enforced by the tools; listed here so the daily agent follows them)

- No game files, no client patch files hosted or shipped. HD packs are link-only, to their authors' page, held with
  `skip` until the owner lets them through. A pack whose page also offers a full client or a modified game executable
  is never added (today: one installer offers a modified client exe, and a texture repo's README mocks the IP issue;
  both left out).
- Nothing decompiled, nothing from wowdecomp or Waystone; blocklisted owners and look-alike lure repos stay out.
- Licences: the hub needs an open licence (else `needs_licence`); upstream addons record the licence and are mirrored
  only when `mirror_allowed` (open licence and no .blp). A licence that forbids modified copies excludes a backport of it.
- No native code (DLLs, injectors, memory editing) in the launcher's installs; no bots, rotation players or macro
  sequencers.
- Our text: no game or company brand words, no client version numbers in prose, no money, tip or donation links; the
  tests check names and descriptions.
- Never approve. Seeds, candidates, curated entries, looks and packs are pending until the owner says yes.

## Scheduling

- **Cloud (exists):** `scout.yml` 05:43 and `hub-index.yml` 07:17 UTC; both open a PR and never merge.
- **PC (new):** the Control step `mods-daily`. The owner can run it by hand, or add it to the Autopilot run order once a
  day (it needs only Python and git; it is not read-only, since it rewrites `mods/upstream.json`).
- **Scheduled cloud task (proposed, not created):** once a day at about 08:40 UTC, after the index gate: a fresh
  session in this repo reads the day's `docs/scout/<date>.md` and `mods/upstream.json`'s `failed` list, adds 5 to 8
  addon seeds and 2 to 4 project seeds by the rules above, runs `curate.py run --only <new>` and the hub-index,
  addon-curate and looks tests, and opens one PR titled "Mods daily <date>" with the counts. Its prompt must say: never
  approve, never push to main, delete clones as it goes. Push notification on.

## Metrics (in the PR body and the Admin Console)

- New items a day by list (addon seeds, index seeds, candidates, looks, packs) and by category.
- Review queue: pending addons, pending updates, gate PR size; and its age (the oldest pending item).
- Approval rate a week (approved / reviewed) and the top reasons for "no": a falling rate means the seeding bar is wrong.
- Curate failures and their reasons (Interface for another build, missing libraries, gone repos).
- Coverage: approved addons per tag and per build; graphics presets with a look; categories with fewer than 5 entries.
- Installs per addon from the launcher (when its counter exists) to rank what to review first.

## TODO(owner)

- The 3 Nov sweeps added 389 pending addon seeds and 557 pending hub candidates. They become pending entries at the next
  `addons-curate` / `mods-daily` run and the daily index build; nothing is approved. Review in batches, and decide
  whether editions of the same addon (for example several AdiBags, Leatrix Plus or WeakAuras backports) should all be
  offered or only the best one.

- Review the 44 new pending addons (Admin Console › Mods). Ones with flags to look at first: `up:masque` and
  `up:atlasloot` (a folder name carries a brand word), `up:pretty-actionbar`, `up:pretty-lootalert`, `up:leatrix-maps`
  and `up:gladius` (textures named like stock art), `up:bagzen` and `up:dpsmate` (the .toc lists files that are not in
  the repo). 21 of the 44 have an open licence; the rest state none.
- Approve or drop the looks `look:performance` and `look:high` (they need a screenshot each; the wording is ours).
- External packs: let TurtleHD and Project Reforged through by removing their `skip` line, or not.
- ReShade: its own description calls it an injector, which the cheats rule refuses by that word. Waive it for ReShade
  (and reshade-shaders, which has no top-level licence file) or keep it out.
- The HD font renderer (Lexara): GPL, but it commits built DLLs and is a DLL loaded into the client. Link-out or not.
- `look:epoch-water`: its README's download link points to a repo of the blocklisted owner `olive-spore-734` (a
  lure-campaign account). The look is approved today; consider taking it back to pending.
- The Modern Renderer already listed (kasperfriend/friends-graphics) carries the README of Corfirean/modern-wow-renderer,
  now seeded as "the original": decide which to credit, or list both. Neither has a licence yet.
- Whether to create the daily scheduled cloud task described above, and whether `mods-daily` joins the Autopilot.
- The daily aim (10 a day) and the review time it implies (about 10 minutes).
