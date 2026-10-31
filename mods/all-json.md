# /wrath/mods/all.json

One list of every Workshop project: our packaged mods and the community links, in one schema. The
Leyline Launcher (`launcher/`) and the in-game window read it so they show the same list as
https://play.openvanilla.app/wrath/mods. It is public, `Access-Control-Allow-Origin: *`, no sign-in.

```json
{
  "format": 1,
  "generated_at": "2026-10-25T12:00:00Z",
  "categories": [{"id": "addons", "name": "Addons and UI", "line": "...", "icon": "interface"}],
  "builds": ["3.3.5a", "1.12", "any"],
  "projects": [
    {
      "id": "mod:pings",
      "source": "packaged",
      "name": "Pings",
      "description": "Alt-click the world map ...",
      "category": "addons",
      "builds": ["3.3.5a"],
      "author": "WrathUnleashed",
      "author_url": "",
      "licence": "MIT",
      "repo": "https://github.com/orionopenwow/WrathUnleashed/tree/main/mods/catalog/pings",
      "page": "https://play.openvanilla.app/wrath/mods/pings",
      "credits": "Pings by WrathUnleashed · MIT · github.com",
      "stars": null, "last_push": "",
      "submitted_by": "", "submitted_at": "", "last_checked": "",
      "version": "0.1.0", "kind": "both", "permissions": ["chat", "timers"], "on_wrath": true,
      "download": "https://play.openvanilla.app/wrath/mods/pings/download",
      "sha256": "..."
    },
    {
      "id": "link:3f9a0c12bd47",
      "source": "community",
      "name": "frame-kit",
      "description": "A tidy set of unit frames for raids.",
      "category": "addons",
      "builds": ["3.3.5a", "1.12"],
      "author": "alice", "author_url": "https://github.com/alice",
      "licence": "MIT",
      "repo": "https://github.com/alice/frame-kit",
      "page": "https://play.openvanilla.app/wrath/mods/p/3f9a0c12bd47",
      "credits": "frame-kit by alice · MIT · github.com",
      "stars": 42, "last_push": "2026-10-20T10:00:00Z",
      "submitted_by": "Alice", "submitted_at": "2026-10-25T11:58:00Z", "last_checked": "2026-10-25T11:58:00Z",
      "version": null, "kind": null, "permissions": null, "on_wrath": null, "download": null, "sha256": null
    }
  ]
}
```

| field | meaning |
| --- | --- |
| `id` | stable: `mod:<mod id>`, `up:<slug>` or `link:<12 hex>` |
| `source` | `packaged` (our catalog, with a zip), `community` (a link only: nothing is hosted) or `upstream` (a reviewed addon the launcher installs from its author's repo) |
| `category` | one of the 13 `categories` ids (`mods/categories.json`); `themes` holds launcher theme packs (`docs/THEMES.md`) |
| `builds` | `3.3.5a`, `1.12`, or `any`; a project for both lists both |
| `author`, `author_url`, `licence`, `repo` | the credit. `licence` is `""` when the repo states none: show "licence not stated" |
| `credits` | the credit line exactly as the site prints it under each card |
| `stars`, `last_push`, `last_checked` | from the forge's public API (GitHub, GitLab, Codeberg); `null` / `""` when unknown |
| `version` … `sha256` | packaged mods only; `null` for community links |
| `featured` | `true` when staff featured it in the Admin Console (docs/ADMIN-CONSOLE.md); show it first or badge it |
| `tags`, `install`, `addons`, `requires`, `conflicts`, `replaces_ui`, `native`, `blp`, `mirror_allowed`, `licence_source`, `review`, `compat`, `folder_name`, `size`, `file_count` | upstream addons only (`source: "upstream"`, id `up:<slug>`): curated third-party addons the launcher installs from the author's own repo, never hosted by us unless `mirror_allowed`. `install` is `{from, url, rev, sha256, size, file_count, strip_prefix, folders, files_digest}` (the full file list stays in `mods/upstream.json`). `credits` reads "<name> by <author> · <licence or "licence not stated"> · Installs from the author's GitHub"; `page` is the repo. Only reviewed entries are in all.json. mods/SPEC.md, "Upstream addons", has the rules |

Sorted by name. Hidden links, and mods staff rejected or unpublished, are left out. New fields may appear in format 1: ignore what you don't know.
Show the text as text (never as HTML) and open `repo` / `page` in the browser.

## Listed mods (`source: "listed"`)

A community link whose author listed it with **List your mod** (stream/listings.py, docs/LIST-YOUR-MOD.md) and whose
check found something for a game client. Same id (`link:<12 hex>`), plus:

| Field | Meaning |
|---|---|
| `format`, `format_label` | `addon`, `package`, `patch` or `native`, and its words |
| `state` | `community` ("Community · not reviewed") or `reviewed` (a moderator approved it) |
| `flags`, `plan` | what the check flagged (a moderator looked first), and what one click does in plain words |
| `native`, `blp`, `size`, `file_count`, `version` | native code (the launcher asks first), `.blp` author art, the installed size |
| `install` | `kind` (`addon`: addon folders, installed like an upstream addon, with `folders` and `blp`; `files`: staged and checked like a dropped zip: patches, native code, a package's client half), `from` (`release-asset` or `commit-archive`), `url` (the listing's own repo), `rev`, `sha256` (release files only), `size`, `file_count`, `strip_prefix`, `files` (path -> sha256 of every installed file), `files_digest` |

The launcher (`launcher/src-tauri/src/listed.rs`) installs it only when the block checks out (`listed::install_ok`) and
every downloaded file matches; otherwise it shows it as a plain link. Listings that wait for a moderator, are hidden by
reports or were taken down are not in all.json. Realm scripts and game modes stay `community` links (nothing for a client).
