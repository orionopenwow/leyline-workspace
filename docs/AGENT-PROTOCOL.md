# The Agent Protocol: how AI agents build Leyline on autopilot

**The short version.** Leyline is built by a human who holds the vision and AI agents who do the work, joined by
one strict loop: the owner says *what the end product should feel like*, an agent turns that into a small numbered
step, the step lands in `UPDATES-PENDING.txt` with one plain line saying what it brings, Leyline Control (or
`UPDATE.bat`) runs the queue in order and writes each finished id to `UPDATES-DONE.txt`, and `Ship to GitHub.bat`
pulls main, checks for secrets and game data, commits as "Leyline update <date> <time>" and pushes to main, with
no questions asked. Every agent leaves a short note of what it did and what it thought of, so the next agent (or
the owner) picks up exactly where it stopped. Done with discipline, this loop makes the whole network one protocol,
**Commonhold**: one main server that does well because it uses the same tech everyone else gets, Dens anyone can
hop between in seconds, a Den Lite repo anyone can fork to make their own world, a 3.3.5a Den world editor (Forge)
to shape it, and signed launchers so server hopping works across a network of clean, trusted installs while links
can still be external, internal or both.

---

## 1. The loop (what already exists here)

- **The owner's window:** `Leyline Control.bat` (`tools/control/control.py`). *Waiting on you* holds each question
  an agent has for the owner, with its own buttons. *Ready to launch* is the queue. Autopilot runs new steps by
  itself while the window is open.
- **The queue:** `UPDATES-PENDING.txt`, one line per step: `<id> <step> <what it brings>` (for example
  `1025j scripts  Level 80 guide: .guide, shown on reaching 80`). Ids are date + letter (`1110a`, `1110b`…), so
  order is clear and nothing collides.
- **The memory:** `UPDATES-DONE.txt`. A step that is in it never runs again, so running the queue twice is always
  safe.
- **The step kinds:** every kind (`ship`, `client`, `scripts`, `foreverui`, `slice`, `leyline`…) is a function in
  `STEPS` in `control.py`. An agent never invents a new way to deploy; it adds or reuses a step.
- **The ship:** `Ship to GitHub.bat` brings in main, refuses secrets and game data, commits and pushes to main. It
  closes on success and stays open on a problem. In Autopilot each ship also refreshes the public copy
  (`tools/leyline-repo/export.py` → github.com/orionopenwow/Leyline).
- **The builds:** GitHub Actions builds the launcher (`launcher-latest`) and Den Lite (`den-lite-latest`) after
  every change, so nobody builds by hand.

## 2. The rules every agent follows (the consistency)

1. **Read first.** `README.md`, `docs/REPO-MAP.md`, and the doc for the area you touch (`docs/DEN.md`,
   `docs/leyline-realm-hopping.md`, `docs/HYBRID-CLIENT.md`…). Match what is there; do not restructure.
2. **Small steps.** One step = one thing a player or the owner would notice. If a step needs a restart, say so
   and how long ("5-min restart", "live reload, nobody kicked").
3. **Plain words in the queue.** The line in `UPDATES-PENDING.txt` is written for the owner, not for engineers.
4. **Never push by hand.** The last step of any batch is a `ship` step. The `.bat` does the commit and push to main.
5. **No secrets, no game data.** Tokens stay in env vars or the credential helper. Game files are read from the
   player's own 3.3.5a install, never committed or redistributed.
6. **Docs move with code.** Change a feature → change its doc in the same step.
7. **Ask through Control, not chat.** Anything the owner must do (a file, a key, a yes/no) goes into *Waiting on
   you*, with the button that does it.
8. **Leave the note** (section 3). No note = the step is not finished.
9. **Two agents, two lanes.** When several agents work at once, each owns its folders (for example one in
   `labs/den-lite/`, one in `launcher/`, one in `stream/`) and only the coordinator writes the `ship` step.

## 3. The note every agent leaves

At the end of each step, the agent adds this to its reply and to `docs/history/` (one file per day is fine):

```
## <id> <step>: <one-line title>
Did:      what changed, in plain words, with the main files.
Thought:  ideas, risks or better ways noticed but not done (each one a possible next step).
Vision:   which part of the owner's end product this moves forward (see section 4).
Check:    how the owner can see it working in under a minute.
Next:     the step id that should follow, if any.
```

The *Thought* lines are the most valuable part: they are how agents "collaborate" across days without the
owner having to remember anything.

## 4. Mapping the owner's vision to the end product

Keep one table in `docs/ROADMAP.md` and point every step at a row. The current vision, as stated by the owner:

| The owner wants… | So the product has… | Where it lives |
|---|---|---|
| One server that thrives on its own tech | The main realm runs the same launcher, host kit, relay and Den as everyone | `realm/`, `hosting/`, `launcher/` |
| Seeded into the internet | Anyone hosts in one click; realms are listed for everyone; P2P/relay/mesh | `hosting/`, `docs/LEYLINE-MESH.md`, `docs/SEEDED-REALM-PLAN.md` |
| Hop between worlds quickly | **The Den** as home base: a door to any listed realm or friend's Den, back in one click | `docs/leyline-realm-hopping.md`, `docs/DEN-INVITE.md` |
| People make their own worlds | **Den Lite as a template repo** anyone forks | `labs/den-lite/` → its own public repo |
| A real editor for those worlds | **Forge**, the 3.3.5a Den world editor (from Noggit Red, GPL-3.0) | `forge/`, `docs/NOGGIT-DEN-PLAN.md` |
| Clean clients, not a mess of patched ones | **Signed launcher + signed patch manifests** that verify the player's own client | `launcher/`, `docs/HYBRID-CLIENT.md` |
| Open links, inside and out | Hybrid links: `leyline://` inside the launcher, web links outside, both resolve the same realm | `stream/`, `launcher/` |

## 5. Commonhold: the protocol for the network

Commonhold is the agreement every Leyline world follows so hopping is smooth and nobody needs a messy client.

- **One save format.** Every Den is a `leyline-den` file (version 1, `docs/DEN.md`), with the same stamps. A Den
  made in Den Lite moves to a hosted Den and back without changes.
- **The Den as the hub.** Your Den is always one click away (Den Lite runs on the player's PC with no Docker and no
  database). From it, doors lead to listed realms and friends' Dens; leaving any world brings you home.
- **The Den Lite repo.** Publish `labs/den-lite/` as its own template repo ("Use this template" on GitHub) with:
  the server, an example Den, the stamps, a `Host my Den.bat`, a `Ship my Den.bat` that mirrors our ship step, and
  this protocol file. Anyone forks it, makes a world, and lists it on the hub.
- **The editor.** The launcher fetches Forge on first "Shape ground"; edits save as terrain deltas
  (`docs/terrain-delta-leyline.schema.json`), so worlds share ground changes without shipping game files.
- **Signed installs.** We sign what *we* make, never Blizzard files:
  - the launcher (Tauri's updater signature, plus code signing when a certificate is available);
  - each patch / mod / Den pack, through a manifest signed with the Leyline key (Ed25519, e.g. minisign) and the
    SHA-256 list already on `/wrath/download`;
  - realms and Dens that join the network get a signed listing, so the launcher shows "verified" and warns on the
    rest instead of blocking them.
  The launcher checks the player's own 3.3.5a client (build 12340) by hash and applies only signed patches. Result:
  one clean client that can hop to any Commonhold world.
- **Hybrid links.** Every world has one id. `leyline://realm/<id>` opens it in the launcher, `https://leylinelauncher.com/wrath?go=r:<id>`
  opens it on the web or offers the launcher, and private invites stay in the URL fragment (never sent to a server).
  External hosts keep their own address; the listing just points to it.
- **Fair rules for hosts.** A world that follows Commonhold (format, signed listing, invite rules, no game data
  redistributed) gets listed and hoppable; that is the only gate.

> Note: the signed listing, the Den Lite template repo and the `leyline://` scheme are the next pieces to build;
> the Den format, Den Lite, Forge, the hop links and the checksums already exist.

## 6. Prompts to get going

**The 3-sentence starter** (paste at the top of any agent session):

> You are working in the Leyline workspace (`Desktop\Leyline`); read `README.md`, `docs/REPO-MAP.md` and
> `docs/AGENT-PROTOCOL.md` first and follow its rules exactly. Turn my request into small numbered steps in
> `UPDATES-PENDING.txt` (plain-word lines, existing step kinds from `control.py`), finish with a `ship` step, and
> never push by hand or commit secrets or game data. When done, give me the Did / Thought / Vision / Check / Next
> note for each step, and put anything you need from me into Leyline Control's *Waiting on you*.

**Recommended longer kickoff** (for a big push like Commonhold):

> Goal: make Commonhold real. In order: (1) publish `labs/den-lite/` as a public template repo with an example
> Den, stamps, `Host my Den.bat`, `Ship my Den.bat` and this protocol; (2) add signed manifests (Ed25519) for
> launcher patches and Den packs, and a "verified" badge for signed listings; (3) add the `leyline://` link scheme
> with matching web links, and doors in Your Den to any listed world; (4) make Forge open any Den from the Den
> Lite repo. Work in lanes if you use several agents, one coordinator writes the `ship` steps, and update the
> vision table in `docs/ROADMAP.md` as each row moves.

**Daily autopilot prompt** (for a scheduled run):

> Read `UPDATES-PENDING.txt`, `UPDATES-DONE.txt` and yesterday's notes in `docs/history/`; pick the top *Thought*
> worth doing that moves a vision row forward, do it as one step plus a `ship` step, and leave the note.
