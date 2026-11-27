# The static site's own data

Leyline's website (leylinelauncher.com) is a folder of plain files on GitHub Pages, built by
`tools/site-static/build.py` (how it works: `docs/STATIC-SITE.md`). Two files in this folder are its curated data.
Edit them, ship, and the site rebuilds by itself (the "leyline site" workflow on the public Leyline repo).

| File | What it feeds |
|---|---|
| `realms.json` | the Realms page (`/wrath/leyline/realms`) and the launcher's realm list (`/wrath/realms.json`) |
| `news.json` | the News page (`/wrath/leyline/news`) and the launcher's News (`/wrath/leyline/news.json`) |

The build checks both files and stops with a plain message when an entry is wrong, so a typo never reaches the site.

## Add a server to the realm list

Only list a server whose owner said yes. TODO(owner): add the servers you want listed.

Add one entry to the `realms` list in `realms.json`:

```json
{
  "name": "Example Realm",
  "realmlist": "logon.example.org",
  "website": "https://example.org",
  "discord": "https://discord.gg/example",
  "game": "3.3.5a",
  "type": "PvE",
  "core": "azerothcore",
  "tagline": "A short line under the name.",
  "notes": "A few sentences: rates, rules, what makes it different.",
  "tags": ["blizzlike", "bots"],
  "region": "eu",
  "language": "en"
}
```

| Field | Needed | What to put |
|---|---|---|
| `name` | yes | the server's name, up to 32 characters |
| `realmlist` | yes | the address players put in their realm list: `host` or `host:port` (port 3724 when left out) |
| `website` | yes | the server's own site, `https://...` (players make their account there) |
| `discord` | no | its Discord invite, `https://discord.gg/...` |
| `game` | yes | the game client it takes: `1.12.1`, `2.4.3` or `3.3.5a` |
| `type` | no | `PvE`, `PvP`, `RP`, `RP-PvP` or `Fun` (default `PvE`) |
| `core` | no | `azerothcore`, `trinitycore`, `cmangos`, `vmangos`, `mangos` or `other` (default `other`) |
| `tagline` | no | one line, up to 80 characters |
| `notes` | no | up to 400 characters |
| `tags` | no | any of `blizzlike`, `boosted`, `custom`, `pvp`, `rp`, `hangout`, `bots` |
| `region` | no | one of `na-east`, `na-west`, `sa`, `eu`, `africa`, `me`, `asia`, `oce` |
| `language` | no | a language code: `en`, `de`, `pt-BR` |
| `featured` | no | `true` puts it first |

Rules the build enforces: no game names or the game maker's name in any text (the realm list is read by players), no
version numbers in `tagline` or `notes`, `website` and `discord` must be plain `https://` addresses, and no money,
donation or shop links.

The list never shows live player counts: Leyline no longer runs a server that watches realms. The launcher checks each
server itself when it shows the list.

## Post news

Add a post at the top of the `posts` list in `news.json` (newest first, 20 at most):

```json
{
  "id": "2026-11-27-hello",
  "date": "2026-11-27",
  "tag": "Update",
  "title": "One line, up to 120 characters",
  "items": ["A bullet, up to 200 characters.", "Another bullet."]
}
```

TODO(owner): the first post (for example, what changed when the server shut down), in your words.
