# List your mod

Made a mod? List it, and players can install it in the Leyline launcher with one click. Any player with an account can
list one. It takes a minute.

## How

1. Put your mod in a public repo on **GitHub**, **GitLab** or **Codeberg**.
2. Open **List your mod**: in the launcher, Mods & Addons › Installed › **List your mod** (or Discover › **List your mod** / **List your addon**); the form asks whether it is an addon or a mod; on the website, `/wrath/mods/list`.
3. Paste the link: the repo, a branch, a release, or one release file (a `.zip` or an `.MPQ`).
4. We read it once and show you **exactly what one click will do**: which folders it adds, which files, how big.
   If something is wrong, it says what and where, so you can fix it in your repo and check again.
5. Pick a category, write one line about it, and press **List it**.

## What you can list

| Your mod | What one click does |
|---|---|
| An addon (a folder with a `.toc` for our client) | Adds the addon folder(s) to `Interface/AddOns` |
| A Leyline mod package (`mod.json`, see `mods/SPEC.md`) | Adds its addons; its realm part runs only on realms that add it |
| A texture or client patch (`patch-X.MPQ`) | Adds it to your `Data` folder, on a free patch letter, so the game's own patches are never replaced |
| A native mod (`.dll`, `.asi`, `.exe` at the top) | Puts it in the game folder, after the player says yes: native code has full access to their PC |
| A realm script (Lua for realms) | Listed for realm owners; nothing goes into the game |
| A game mode (one Modes Lua file) | Listed for the Modes editor; nothing goes into the game |

Loose game files (an `Interface/` or `World/` folder of textures) are not loaded by the game: pack them into a
`patch-X.MPQ` and release that.

## Live now, or a moderator first

- **Live at once**, marked **Community · not reviewed**: addons, mod packages, realm scripts and game modes that raise
  no flag.
- **A moderator looks first**: native code, client patches, textures (`.blp`), textures named like the game's own
  art, big packs, the first listings of a new account, and anything that looks like a cheat or leaked material.
  You see why; it goes live as soon as they say yes.
- **Never allowed**: game files (models, maps, databases), programs inside an addon folder, zips inside the zip,
  realm scripts that reach outside the realm (files, the OS, loading code). The check tells you which file.

The owner can change which kinds go live at once (in Leyline HQ).

## Your files stay yours

We never copy your mod to our site. The launcher downloads it from **your** repo, at the exact version that was
checked, and compares every file's fingerprint before installing anything. If the file changed, nothing is installed.
A repo without a licence is fine: it still installs from you only.

## Updates

If you listed a repo or a branch, or "latest release", we look for new versions every hour or so. A clean update goes
out by itself and players see **Update** in My Mods. An update that raises a flag waits for a moderator; the old version
stays until then. A link to one exact commit or tag never moves on its own: list the new one when you're ready.

## Ratings and reports

- Every player can rate a listing once (and change it). Listings show up under **Hot**, **Rising** and **Top**.
- Anyone signed in can **Report** a listing with a reason. After a few reports from different players it hides itself
  until a moderator looks. You can't report your own.

## Changing or removing it

Open List your mod and look under **Your listings**: change the one-line description, or **Remove** it. Players who
installed it keep it. Moderators can take a listing down, and say why.

## Limits

A few listings a day per account (one a day for new accounts), ten checks an hour, and a size cap on what we read.
