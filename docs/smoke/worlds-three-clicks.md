# Worlds in three clicks: phase 0 timings

To fill on the owner's fresh-PC run (docs/UNSTOPPABLE.md, phase 0 and section 4). Every number comes from
`<Leyline data folder>/world-timings.log` (one line per step: `<unix time> <world> <step> <seconds>`), also in each
world's `logs/setup.log` as `timing: …` lines. Click counts come from doing it, not from the tests.

**PC:** (CPU, RAM, disk type, Windows version) · **Leyline:** (version) · **Engine:** (engine.json version) · **Date:**

## Fresh PC: first world

| Step | World | Seconds | Note |
|---|---|---|---|
| download (engine, in the background or on Start) | prepare / single | | |
| unpack | prepare / single | | |
| maps (read from the game folder) | prepare / single | | |
| database (MySQL's first run) | single | | |
| fill (dbimport) | single | | |
| keeping the prepared copy (setup.log "kept a prepared copy … s") | single | | |
| account | single | | |
| start (first start: bots made) | single | | |
| **total-fresh** | single | | |
| Clicks from Home to World running | | | |

## Second template on the same PC (the prepared copy)

| Step | World | Seconds | Note |
|---|---|---|---|
| copy (the prepared database) | vanilla | | |
| account | vanilla | | |
| start | vanilla | | |
| **total-prepared** | vanilla | | |
| Boss Rush maps (copied from Single player's read) | bossrush | | |

## Started again (set up, stopped)

| Step | World | Seconds | Note |
|---|---|---|---|
| **total-start** | single | | |

## Invite

| What | Result | Note |
|---|---|---|
| Clicks from opening the invite link to the game starting (friend) | | |
| Same network join works | | |
| Disk used by `engine-cache/` | | |
