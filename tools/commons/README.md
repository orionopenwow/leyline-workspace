# The commons kit: make your own Leyline

If Leyline ever goes quiet, anyone can carry it on. This folder holds the quickest way to do that. Why it exists, and
the three ways to keep the network alive, are in [docs/MOVEMENT.md](../../docs/MOVEMENT.md). The plan to split
everything into pieces is in [docs/BLANK-SLATE.md](../../docs/BLANK-SLATE.md).

## The three things to click

| File | What it does | Needs |
|---|---|---|
| **Make my own Leyline.bat** | Asks a few questions, then makes your own website and Log in page, browser kit, host kit and launcher branding, ready to upload | Python 3 |
| **Start my own server.bat** | Your own realm on this PC, from your own game client. Starts private | Docker Desktop, your own game folder |
| **Be a node.bat** | This machine carries other players' game connections for the network | Docker, a public name, open ports 80, 443 and 41000-41051 |

`Start my own server.bat` and `Be a node.bat` call the host kit's own scripts (`Host a Leyline realm.bat`, and
`setup.py volunteer-relay` with `docker-compose.volunteer-relay.yml`). They look for the host kit in their own
folder, then in `host-kit\` next to them, then in the repo's `hosting\kit`. "Make my own Leyline" puts both inside
the host kit it packs, so the zip you share has them at the top.

`Be a node.bat` needs the relay code, which only the packed host kit carries (the repo's `hosting\kit` doesn't). It
says so if it's missing.

## Make my own Leyline

Double-click it, or:

    python3 -I tools/commons/commons.py make                        # asks
    python3 -I tools/commons/commons.py make --answers my.json      # no questions
    python3 -I tools/commons/commons.py make --yes                  # every default (Leyline's own look)

The questions: your network's name, three colours (main, background, text), a logo (.png or .webp, up to 1 MB),
your domain (or none yet) and a Discord invite (or none). They become one `brand.json`:

```json
{
  "format": 1,
  "name": "Emberline",
  "colours": {"bg": "#201010", "text": "#fff0e0", "accent": "#e0602a", "accent_strong": "#e67d50"},
  "logo": "brand-logo.png",
  "links": {"site": "https://play.example.org", "discord": "https://discord.gg/abc", "reddit": ""}
}
```

It is refused if the name uses the game's or its maker's names, or if any link is a money, tip or donation link.
Only site, Discord and Reddit links are allowed.

What you get in `tools/commons/out/<your name>/`:

| Folder | What | Reads brand.json |
|---|---|---|
| `site/` | the website (`wrath_portal.py` and its modules), the Log in page, the public guides, the art, `run-site.sh`, `portal.env.example`, `Caddyfile.example` | yes: `WU_BRAND_FILE` (name, colours, logo, tab icon, Discord link) |
| `browser-kit/` | the game page around the browser client, with `brand.css` | yes: colours and logo, the page title |
| `host-kit/` | the network's host kit folder and zip, with the two wrappers inside | no (see below) |
| `launcher/` | `branding.json` for `launcher/scripts/rebrand.py`, your logo, a README | yes, once you build it |
| `UPLOAD.md` | the steps to put it online | |

Look at the site before you upload it: answer **y** at the end, or run
`python3 -I tools/commons/commons.py preview tools/commons/out/<name>/site`. It runs on `127.0.0.1` only.

Every packed file goes through the public export's own checks (`tools/leyline-repo/export.py`): no game data, no
secrets, no logs, no decompiled code, and no game or maker names in what people read. If anything fails, nothing is
written. Leyline's own files are never changed: the kit only reads them.

## What can't be reskinned yet (honestly)

- **Log in** needs the sign-in service. It is a separate program that is not in this repo. Without it the Log in page
  says it isn't answering. Players' Hero Keys from leylinelauncher.com only work on a site that has the key table.
- **The host kit** and the in-game windows still say Leyline, and realms still dial the network's relay.
- **The launcher** carries your name only once you build it yourself, with your own updater key and your own update
  sources (`launcher/README.txt` in the output, and docs/FORKING.md). Until then, a built copy updates itself back to
  Leyline.
- **The browser client** (`wenilla.js` and its wasm) and its small server are built from a separate source that is not
  here. The browser kit reskins the page around it.
- **Words inside scripts** (the launcher UI's JavaScript, the site's scripts) keep "Leyline".
- **The site's code isn't public yet.** Until the owner publishes `stream/` with a licence (docs/SUCCESSION.md, D1),
  only people with this repo can run "Make my own Leyline".

## The directory

`directory.json` is the living list of every piece Leyline is made of, and where its mods lists live.

    python3 -I tools/commons/commons.py check-directory      # every path exists, no brand words, no money
    python3 -I tools/commons/commons.py wiki                 # the forum's pinned post, in posts the forum accepts

## Tests

    python3 -I tools/commons/test_commons.py
    python3 -I stream/tests/test_brandfile.py stream/wrath_portal.py
