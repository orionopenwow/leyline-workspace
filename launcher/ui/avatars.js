// Leyline Launcher: the player portraits (window.LeylineAvatars). Round flat-graphic character pictures on a grey slate
// medallion (owner 2026-11-01: "grey slate stylize these user icons and not letters ... a cool array of flat graphics art
// characters"), original art drawn in this repo (tools/art/avatars/make_avatars.py), shipped in art/avatars/ so they
// show at once and offline. The website serves the same files (stream/profiles.py PRESETS: the same ids in the same
// order) and the same default: a player who hasn't picked one gets a portrait from their display name, never a letter.
(function () {
  "use strict";
  // [id, name] in the default-picture order: never reorder, only append (stream/profiles.py PRESETS says the same)
  const LIST = [
    ["alchemist", "Alchemist"], ["bard", "Bard"], ["brute", "Brute"], ["captain", "Captain"], ["cat", "Cat"],
    ["cleric", "Cleric"], ["drake", "Drake"], ["druid", "Druid"], ["fox", "Fox"], ["golem", "Golem"], ["knight", "Knight"],
    ["mage", "Mage"], ["monk", "Monk"], ["owl", "Owl"], ["ranger", "Ranger"], ["revenant", "Revenant"], ["rogue", "Rogue"],
    ["shieldmaid", "Shieldmaid"], ["smith", "Smith"], ["wanderer", "Wanderer"], ["warrior", "Warrior"], ["witch", "Witch"],
    ["wizard", "Wizard"], ["wolf", "Wolf"],
  ];
  const IDS = new Set(LIST.map((x) => x[0]));
  // the shapes the website offered before the portraits: the portrait put in each one's place (profiles.py LEGACY)
  const LEGACY = { star: "wizard", moon: "witch", flame: "smith", leaf: "druid", peak: "ranger", gem: "mage", sun: "cleric",
    bolt: "warrior", wave: "captain", shield: "knight", crown: "shieldmaid", tree: "wanderer" };
  const ANON = "wanderer";   // signed out: the hooded wanderer

  // FNV-1a 32 over the lower-cased name's UTF-8, the same as profiles.py default_portrait
  function pick(name) {
    const s = String(name == null ? "" : name).trim().toLowerCase();
    if (!s || s === "?") return ANON;
    let h = 0x811c9dc5;
    for (const b of new TextEncoder().encode(s)) { h ^= b; h = Math.imul(h, 0x01000193) >>> 0; }
    return LIST[h % LIST.length][0];
  }
  function known(id) { id = LEGACY[id] || id; return IDS.has(id) ? id : ""; }
  const src = (id) => "art/avatars/" + (known(id) || ANON) + ".svg";
  const nameOf = (id) => { const x = LIST.find((p) => p[0] === known(id)); return x ? x[1] : ""; };
  // a website portrait address (…/wrath/avatar/p/<id>.svg) -> its id, so the local copy shows instead; else ""
  function fromUrl(url) {
    const m = /\/wrath\/avatar\/p\/([a-z]{2,12})\.svg$/.exec(String(url || ""));
    return m ? known(m[1]) : "";
  }
  window.LeylineAvatars = { list: () => LIST.map((x) => x.slice()), pick, src, known, nameOf, fromUrl };
})();
