// Mods › Discover, made simple (owner 31 Oct: "'Mods' should also have a simple section, trim search"): the Simple
// view is one search box and a few short rows of big cards (Popular, Game modes, Make it look nicer, Quality of life);
// "Browse everything" is the full catalog, where the search is one box, a few smart chips, and the rest (Sort, Show,
// Only for, the addon kinds) waits under More filters. The website's /catalog does the same (stream/hub.py
// simple_rows, with the same three lists below). app.js calls render() at the end of renderCatalog(); it reads app.js's
// globals (catalog, catFilter, ranked, projectCard, pickedClient, fitsClient, renderCatalog) when it is called.
"use strict";
(() => {
  const $ = (id) => document.getElementById(id);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const realm = () => (window.LeylineApp && window.LeylineApp.HANGOUT && window.LeylineApp.HANGOUT.name) || "Wrath Unleashed";
  const GAME_CATS = ["sandbox", "maps", "encounters", "conversions"];
  const LOOK_TAGS = ["full-ui", "unit-frames", "raid-frames", "action-bars", "nameplates", "skins", "combat-text", "auras"];
  const QOL_TAGS = ["quality-of-life", "bags", "quests", "maps", "minimap", "tooltips", "chat", "auction", "gear", "social", "professions"];
  const ROW_N = 3;
  const has = (p, list) => (p.tags || []).some((t) => list.includes(t));
  // [key, title, line, fits, what "See all" opens]
  const ROWS = [
    ["game", "Game modes", "Minigames and mods that change how you play.", (p) => GAME_CATS.includes(p.category), { cat: "sandbox" }],
    ["looks", "Make it look nicer", "Interfaces, frames and nameplates. Looks for the launcher are under Themes.",
      (p) => p.category === "graphics" || has(p, LOOK_TAGS) || !!p.replaces_ui, { cat: "addons" }],
    ["qol", "Quality of life", "Small helpers: bags, quests, maps, chat and more.",
      (p) => has(p, QOL_TAGS) || (p.category === "addons" && !(p.tags || []).length), { cat: "addons" }],
  ];

  let view = "simple";
  try { view = localStorage.getItem("leyline.dcView") === "all" ? "all" : "simple"; } catch (_) { /* fine */ }
  const pane = () => $("pane-discover");

  function setView(v, keepText) {
    view = v === "all" ? "all" : "simple";
    try { localStorage.setItem("leyline.dcView", view); } catch (_) { /* fine */ }
    if (pane()) pane().dataset.view = view;
    if (view === "simple" && !keepText && $("cat-q")) $("cat-q").value = "";
    if (typeof renderCatalog === "function") renderCatalog();
  }

  // The Simple view's rows, from the catalog the full view also shows (only what fits the picked client).
  function rows() {
    const c = typeof pickedClient === "function" ? pickedClient() : null;
    const all = (typeof catalog !== "undefined" && catalog && catalog.projects) || [];
    let pool = all;
    if (typeof ranked !== "undefined" && ranked && ranked.ids) {
      const byId = new Map(all.map((p) => [p.id, p]));
      pool = ranked.ids.map((id) => byId.get(id)).filter(Boolean).concat(all.filter((p) => !ranked.ids.includes(p.id)));
    }
    pool = pool.filter((p) => p.category !== "themes" && (!c || typeof fitsClient !== "function" || fitsClient(p, c)));
    const out = [];
    const popular = pool.slice(0, ROW_N);
    if (popular.length) out.push(["popular", "Popular", "What players pick most.", popular, {}]);
    const used = new Set();
    for (const [key, title, line, fits, open] of ROWS) {
      const got = pool.filter((p) => !used.has(p.id) && fits(p)).slice(0, ROW_N);
      for (const p of got) used.add(p.id);
      if (got.length) out.push([key, title, line, got, open]);
    }
    return out;
  }

  let modesAsked = false;
  function seeAll(open) {
    if (open.tab && window.LeylineThemes) { window.LeylineThemes.showTab(open.tab); return; }
    if (typeof catFilter !== "undefined") { catFilter.cat = open.cat || ""; catFilter.kind = ""; }
    if (window.LeylineMarket) { window.LeylineMarket.state.sub = ""; window.LeylineMarket.state.works = false; }
    setView("all");
  }

  function drawSimple() {
    const box = $("dc-rows");
    if (!box) return;
    const c = typeof pickedClient === "function" ? pickedClient() : null;
    const kids = [];
    let all = rows();
    const gmods = window.LeylineGameMods ? window.LeylineGameMods.entries().filter((g) => g.status === "live").slice(0, ROW_N) : [];
    if (gmods.length) {   // the Game modes row is the Game mods tab's first cards, with where to play them (gamemods.js)
      all = all.filter((r) => r[0] !== "game");
      all.splice(all.length && all[0][0] === "popular" ? 1 : 0, 0, ["game", "Game modes", "Minigames and mods that change how you play.", gmods, { tab: "game" }]);
    } else if (window.LeylineGameMods && !modesAsked) {
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
      const all = el("button", "btn small", "See all");
      all.type = "button";
      all.addEventListener("click", () => seeAll(open));
      head.append(words, all);
      const grid = el("div", "mod-grid dc-grid dc-big");
      for (const p of list) grid.append(key === "game" && open.tab ? window.LeylineGameMods.card(p) : projectCard(p, c));
      sec.append(head, grid);
      kids.push(sec);
    }
    if (!kids.length) kids.push(el("p", "hint dc-empty", "The Workshop list is not here yet."));
    box.replaceChildren(...kids);
  }

  // The few smart chips of the full view: one click each. Everything else is under More filters.
  function drawChips() {
    const box = $("dc-smart");
    if (!box || typeof catFilter === "undefined") return;
    const mk = window.LeylineMarket ? window.LeylineMarket.state : { works: false, sub: "" };
    const chip = (label, on, click, cls) => {
      const b = el("button", "chip-btn" + (on ? " on" : "") + (cls ? " " + cls : ""), label);
      b.type = "button";
      b.setAttribute("aria-pressed", on ? "true" : "false");
      b.addEventListener("click", click);
      return b;
    };
    const set = (cat, works) => () => {
      catFilter.cat = cat; catFilter.kind = "";
      if (window.LeylineMarket) { mk.works = works; if (cat !== "addons") mk.sub = ""; }
      renderCatalog();
    };
    const ups = ((typeof catalog !== "undefined" && catalog && catalog.projects) || []).some((p) => p.source === "upstream");
    const kids = [
      chip("Everything", !catFilter.cat && !catFilter.kind && !mk.works, set("", false)),
      chip("Addons", catFilter.cat === "addons" && !mk.works, set("addons", false)),
      chip("Game modes", catFilter.cat === "sandbox", set("sandbox", false)),
    ];
    if (ups) kids.push(chip(`Works on ${realm()}`, !!mk.works, () => { mk.works = !mk.works; if (mk.works && catFilter.cat && catFilter.cat !== "addons") catFilter.cat = ""; renderCatalog(); }, "mk-works-chip"));
    box.replaceChildren(...kids);
    // More filters opens by itself when something inside it is on (a sort, a Show choice, an addon kind)
    const more = $("dc-more");
    const inside = (catFilter.sort && catFilter.sort !== "hot") || catFilter.kind
      || (catFilter.cat && !["addons", "sandbox"].includes(catFilter.cat)) || mk.sub || ($("cat-fit") && !$("cat-fit").checked);
    if (more && inside && !more.open) more.open = true;
  }

  function render() {
    if (pane()) pane().dataset.view = view;
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
    // "List your mod": a slot for the submission flow (window.LeylineListMod, when it is there); else the website's form
    if ($("dc-list-mod")) $("dc-list-mod").addEventListener("click", () => {
      if (window.LeylineListMod && typeof window.LeylineListMod.open === "function") window.LeylineListMod.open();
      else if (typeof openUrl === "function") openUrl("https://play.openvanilla.app/wrath/leyline/submit");
    });
    const more = $("dc-more");
    if (more) {
      try { more.open = localStorage.getItem("leyline.dcMore") === "1"; } catch (_) { /* fine */ }
      more.addEventListener("toggle", () => { try { localStorage.setItem("leyline.dcMore", more.open ? "1" : "0"); } catch (_) { /* fine */ } });
    }
  }

  window.LeylineSimple = { render, setView, view: () => view, rows, GAME_CATS, LOOK_TAGS, QOL_TAGS };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", mount);
  else mount();
})();
