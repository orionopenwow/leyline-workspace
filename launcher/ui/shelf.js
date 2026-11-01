// Mods and Addons: one rule for the whole launcher (docs/WORDS.md; owner 31 Oct 19:22: "Mods and Addons (distinct, addons
// are in game UI helpers and mods mod anything from engine physics to new models and game modes and server modes)").
//   Addons  in-game interface helpers that go in the game's AddOns folder: unit frames, bags, maps, meters, quest helpers;
//           the addon marketplace (source "upstream") and the Leyline addons.
//   Mods    everything that changes the game itself: models and textures, engine and graphics, game modes and minigames,
//           realm and server modes and realm scripts, sounds, maps, tools.
//   Themes  the launcher's own looks (Discover › Themes and Settings › Themes).
// The website keeps the same rule in stream/hub.py (shelf_of, MOD_GROUPS); stream/tests/test_hub.py checks both match.
// Loaded before market.js, gamemods.js, simple.js and app.js, which all read window.LeylineShelf.
"use strict";
(() => {
  // a game mode or a map stays a mod even when all it adds is an addon folder
  const GAME_CATS = ["sandbox", "conversions", "maps", "builds"];
  // The Mods tab's categories, from the mods list's own categories (mods/categories.json). [key, label, categories, line]
  const GROUPS = [
    ["game", "Game modes", ["sandbox", "conversions"], "Minigames and modes that change how you play."],
    ["models", "Models & textures", ["models"], "New creatures, mounts, props and buildings."],
    ["realm", "Realm & server modes", ["realm-scripts", "presets", "encounters"], "Realm scripts, server rules and boss fights, for realms that run them."],
    ["engine", "Engine & graphics", ["graphics"], "Lighting, sky, water and graphics packs."],
    ["sounds", "Sounds", ["audio"], "Music, ambience and sound packs."],
    ["maps", "Maps & builds", ["maps", "builds"], "New zones, arenas and saved builds."],
    ["tools", "Tools", ["tools"], "Helpers for hosts and makers: map viewers, log readers, overlays."],
  ];
  const GROUP_OF = {};
  for (const [key, , cats] of GROUPS) for (const c of cats) GROUP_OF[c] = key;
  const NAME = { mods: "Mods", addons: "Addons", themes: "Themes" };

  // "mods" | "addons" | "themes" for a mods-list entry (all.json shape: category, source, kind, type)
  function of(p) {
    if (!p || typeof p !== "object") return "mods";
    if (p.category === "themes" || p.source === "theme" || p.type === "theme") return "themes";
    if (p.category === "addons" || p.source === "upstream") return "addons";
    if (GAME_CATS.includes(p.category)) return "mods";
    if (p.type === "addon" || p.kind === "ui") return "addons";
    return "mods";
  }
  // the Mods tab's category key ("" for an addon, a theme, or a category the list doesn't know)
  const group = (p) => (of(p) === "mods" ? GROUP_OF[p.category] || "" : "");
  const groupName = (key) => (GROUPS.find((g) => g[0] === key) || [, ""])[1];
  const groupLine = (key) => (GROUPS.find((g) => g[0] === key) || [, , , ""])[3];
  // An installed mod (mods_installed): its catalog entry's shelf; one from your PC with only addon folders is an addon.
  function installed(m, projects) {
    const p = (projects || []).find((x) => x.id === m.key);
    if (p) return of(p);
    const only = (m.addons || []).length && !(m.patches || []).length && !(m.natives || []).length;
    return only ? "addons" : "mods";
  }

  window.LeylineShelf = { of, group, groupName, groupLine, installed, GROUPS, GAME_CATS, NAME };
})();
