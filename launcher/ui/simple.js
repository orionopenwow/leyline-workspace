// Mods & Addons › Discover, its Mods and Addons tabs (owner 31 Oct: "'Mods' should also have a simple section, trim
// search"; 19:22: "Mods and Addons (distinct, addons are in game UI helpers and mods mod anything from engine physics to
// new models and game modes and server modes)"). Which entry is which: window.LeylineShelf (shelf.js, docs/WORDS.md);
// the tabs themselves are themes.js's (Mods | Addons | Themes).
// The Simple view is one line saying what is on the tab, one search box and a few short rows of big cards: on Mods,
// Popular and one row per category (Game modes, Realm & server modes, Engine & graphics...); on Addons, Popular, Make it
// look nicer and Quality of life. "Browse everything" is the full list, where the tab's categories are the chips (Mods:
// its categories; Addons: the marketplace's kinds) and the rest (Sort, Only for) waits under More filters. The website's
// /catalog and /addons do the same (stream/hub.py simple_rows). app.js calls render() at the end of renderCatalog(); it
// reads app.js's globals (catalog, catFilter, ranked, projectCard, pickedClient, fitsClient, renderCatalog) when called.
"use strict";
(() => {
  const $ = (id) => document.getElementById(id);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const S = () => window.LeylineShelf;
  const LOOK_TAGS = ["full-ui", "unit-frames", "raid-frames", "action-bars", "nameplates", "skins", "combat-text", "auras"];
  const QOL_TAGS = ["quality-of-life", "bags", "quests", "maps", "minimap", "tooltips", "chat", "auction", "gear", "social", "professions"];
  const ROW_N = 3;
  const has = (p, list) => (p.tags || []).some((t) => list.includes(t));
  // the line at the top of each tab (the website says the same: stream/hub.py SHELF_LINE)
  const LINE = {
    mods: "Mods change the game itself: models and textures, graphics, game modes, realm and server modes, sounds and maps. Interface helpers are under Addons.",
    addons: "Addons are in-game interface helpers: unit frames, bags, maps, meters, quest helpers. They go in your game's AddOns folder. Anything that changes the game itself is under Mods.",
  };
  // Addons' rows: [key, title, line, fits]
  const ADDON_ROWS = [
    ["looks", "Make it look nicer", "Interfaces, frames and nameplates.", (p) => has(p, LOOK_TAGS) || !!p.replaces_ui],
    ["qol", "Quality of life", "Small helpers: bags, quests, maps, chat and more.", (p) => has(p, QOL_TAGS) || !(p.tags || []).length],
  ];

  let view = "simple";
  try { view = localStorage.getItem("leyline.dcView") === "all" ? "all" : "simple"; } catch (_) { /* fine */ }
  const pane = () => $("pane-discover");
  // "mods" | "addons" (Themes draws itself)
  const shelf = () => (window.LeylineThemes && window.LeylineThemes.tab && window.LeylineThemes.tab() === "addons" ? "addons" : "mods");
  const onShelf = (p, sh) => (S() ? S().of(p) === (sh || shelf()) : true);

  function setView(v, keepText) {
    view = v === "all" ? "all" : "simple";
    try { localStorage.setItem("leyline.dcView", view); } catch (_) { /* fine */ }
    if (pane()) pane().dataset.view = view;
    if (view === "simple" && !keepText && $("cat-q")) $("cat-q").value = "";
    if (typeof renderCatalog === "function") renderCatalog();
  }

  // The catalog in the hub's order, only what fits the picked client, on this tab's shelf.
  function pool(sh) {
    const c = typeof pickedClient === "function" ? pickedClient() : null;
    const all = (typeof catalog !== "undefined" && catalog && catalog.projects) || [];
    let list = all;
    if (typeof ranked !== "undefined" && ranked && ranked.ids) {
      const byId = new Map(all.map((p) => [p.id, p]));
      list = ranked.ids.map((id) => byId.get(id)).filter(Boolean).concat(all.filter((p) => !ranked.ids.includes(p.id)));
    }
    return list.filter((p) => p.category !== "themes" && onShelf(p, sh) && (!c || typeof fitsClient !== "function" || fitsClient(p, c)));
  }

  // The Simple view's rows: [key, title, line, [projects], what "See all" opens]
  function rows(sh) {
    sh = sh || shelf();
    const list = pool(sh);
    const out = [];
    const popular = list.slice(0, ROW_N);
    if (popular.length) out.push(["popular", "Popular", "What players pick most.", popular, {}]);
    const defs = sh === "addons" ? ADDON_ROWS
      : (S() ? S().GROUPS : []).map(([key, title, , line]) => [key, title, line, (p) => S().group(p) === key]);
    const used = new Set();
    for (const [key, title, line, fits] of defs) {
      const got = list.filter((p) => !used.has(p.id) && fits(p)).slice(0, ROW_N);
      for (const p of got) used.add(p.id);
      if (got.length) out.push([key, title, line, got, sh === "addons" ? {} : { group: key }]);
    }
    return out;
  }

  let modesAsked = false;
  // Mods: open the full list on one category (game, realm, ...); "" for all of them.
  function openGroup(group, quiet) {
    if (typeof catFilter !== "undefined") { catFilter.group = group || ""; catFilter.cat = ""; catFilter.kind = ""; }
    if (quiet) { view = "all"; try { localStorage.setItem("leyline.dcView", view); } catch (_) { /* fine */ } return; }
    setView("all");
  }
  function seeAll(open) {
    if (window.LeylineMarket) { window.LeylineMarket.state.sub = ""; window.LeylineMarket.state.works = false; }
    openGroup(open.group || "");
  }

  function drawSimple() {
    const box = $("dc-rows");
    if (!box) return;
    const c = typeof pickedClient === "function" ? pickedClient() : null;
    const sh = shelf();
    const kids = [];
    let all = rows(sh);
    // Mods: the Game modes row is the Leyline Modes and the game mods, with where to play them (gamemods.js)
    const gmods = sh === "mods" && window.LeylineGameMods ? window.LeylineGameMods.entries().filter((g) => g.status === "live").slice(0, ROW_N) : [];
    if (gmods.length) {
      all = all.filter((r) => r[0] !== "game");
      all.splice(all.length && all[0][0] === "popular" ? 1 : 0, 0, ["game", "Game modes", S().groupLine("game"), gmods, { group: "game", gm: true }]);
    } else if (sh === "mods" && window.LeylineGameMods && !modesAsked) {
      modesAsked = true;
      window.LeylineGameMods.loadModes().then(() => { if (view === "simple") drawSimple(); });
    }
    for (const [key, title, line, list, open] of all) {
      const sec = el("section", "dc-srow");
      sec.dataset.row = key;
      const head = el("div", "dc-srow-h");
      const words = el("div");
      const h = el("h3", "", title);
      h.id = "dc-sr-" + key;
      sec.setAttribute("aria-labelledby", h.id);
      words.append(h, el("p", "hint", line));
      const more = el("button", "btn small", "See all");
      more.type = "button";
      more.addEventListener("click", () => seeAll(open));
      head.append(words, more);
      const grid = el("div", "mod-grid dc-grid dc-big");
      for (const p of list) grid.append(open.gm ? window.LeylineGameMods.card(p) : projectCard(p, c));
      sec.append(head, grid);
      kids.push(sec);
    }
    if (!kids.length) kids.push(el("p", "hint dc-empty", sh === "addons" ? "The addons list is not here yet." : "The mods list is not here yet."));
    box.replaceChildren(...kids);
  }

  const chip = (label, on, click, cls, title) => {
    const b = el("button", "chip-btn" + (on ? " on" : "") + (cls ? " " + cls : ""), label);
    b.type = "button";
    b.setAttribute("aria-pressed", on ? "true" : "false");
    if (title) b.title = title;
    b.addEventListener("click", click);
    return b;
  };
  // The full view's chips: the tab's own categories, one click each. Everything else is under More filters.
  function drawChips() {
    const box = $("dc-smart");
    if (!box || typeof catFilter === "undefined") return;
    const sh = shelf();
    // the rest of the addon kinds, under More filters (market.js chips)
    let rest = $("mk-more");
    if (!rest && $("dc-more-body")) {
      rest = el("div", "chips mk-more");
      rest.id = "mk-more";
      rest.setAttribute("role", "group");
      rest.setAttribute("aria-label", "More kinds");
      $("dc-more-body").append(rest);
    }
    if (rest) rest.hidden = true;
    if (sh === "addons") {   // the marketplace's kinds (market.js)
      box.setAttribute("aria-label", "Addons by kind");
      box.replaceChildren(...(window.LeylineMarket ? window.LeylineMarket.chips(pool("addons"), rest) : []));
    } else {
      box.setAttribute("aria-label", "Mods by kind");
      const n = {};
      for (const p of pool("mods")) { const g = S() ? S().group(p) : ""; if (g) n[g] = (n[g] || 0) + 1; }
      if (window.LeylineGameMods && window.LeylineGameMods.entries().some((g) => g.kind === "mode")) n.game = n.game || 1;
      const set = (g) => () => { catFilter.group = g; catFilter.cat = ""; renderCatalog(); };
      const kids = [chip("All mods", !catFilter.group && !catFilter.cat, set(""))];
      for (const [key, title] of S() ? S().GROUPS : []) {
        if (n[key] || catFilter.group === key) kids.push(chip(title, catFilter.group === key, set(key), "", n[key] ? `${n[key]} here` : ""));
      }
      box.replaceChildren(...kids);
    }
    // More filters opens by itself when something inside it is on (a sort, the client filter off)
    const more = $("dc-more");
    const mk = window.LeylineMarket ? window.LeylineMarket.state : { sub: "" };
    const inside = (catFilter.sort && catFilter.sort !== "hot") || ($("cat-fit") && !$("cat-fit").checked)
      || (sh === "addons" && mk.sub && !!(rest && rest.querySelector(".chip-btn.on")));
    if (more && inside && !more.open) more.open = true;
  }

  // The words on the tab: its line, the search boxes, "List your mod" or "List your addon".
  function drawWords() {
    const sh = shelf();
    const word = sh === "addons" ? "addons" : "mods";
    if ($("dc-line")) $("dc-line").textContent = LINE[sh];
    for (const id of ["dc-sq", "cat-q"]) {
      const q = $(id);
      if (q) { q.placeholder = `Search ${word}`; q.setAttribute("aria-label", `Search ${word}`); }
    }
    const lm = $("dc-list-mod");
    if (lm) lm.textContent = sh === "addons" ? "List your addon" : "List your mod";
    if (pane()) pane().dataset.shelf = sh;
  }

  function render() {
    if (pane()) pane().dataset.view = view;
    drawWords();
    if (view === "simple") drawSimple(); else drawChips();
  }

  function mount() {
    if (pane()) pane().dataset.view = view;
    const sq = $("dc-sq");
    if (sq) sq.addEventListener("input", () => {
      const v = sq.value;
      if (!v.trim()) return;
      sq.value = "";
      $("cat-q").value = v;
      setView("all", true);
      $("cat-q").focus();
    });
    if ($("dc-all")) $("dc-all").addEventListener("click", () => seeAll({}));
    if ($("dc-simple-btn")) $("dc-simple-btn").addEventListener("click", () => setView("simple"));
    // "List your mod" / "List your addon": the submission flow (window.LeylineListMod, when it is there); else the website's form
    if ($("dc-list-mod")) $("dc-list-mod").addEventListener("click", () => {
      const kind = shelf() === "addons" ? "addon" : "mod";
      if (window.LeylineListMod && typeof window.LeylineListMod.open === "function") window.LeylineListMod.open(kind);
      else if (typeof openUrl === "function") openUrl("https://play.openvanilla.app/wrath/leyline/submit" + (kind === "addon" ? "?kind=addon" : ""));
    });
    const more = $("dc-more");
    if (more) {
      try { more.open = localStorage.getItem("leyline.dcMore") === "1"; } catch (_) { /* fine */ }
      more.addEventListener("toggle", () => { try { localStorage.setItem("leyline.dcMore", more.open ? "1" : "0"); } catch (_) { /* fine */ } });
    }
    drawWords();
  }

  window.LeylineSimple = { render, setView, openGroup, view: () => view, shelf, onShelf, rows, LINE, LOOK_TAGS, QOL_TAGS };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", mount);
  else mount();
})();
