// Game modes (owner 31 Oct: "a client mod section like future skating and golf ... like minecraft style adaptations and
// downloads and like gmod too"; 19:22: they are Mods, never Addons): Mods & Addons › Discover › Mods › Game modes lists
// what changes play in game (the Leyline Modes, and the mods list's game modes: shelf.js group "game"), what each needs and the live realms that run it ("Play it on: ..."), from the
// realms' own mod lists (realms.rs Realm.mods; docs/GAME-MODS.md). The same lists drive the join offer: when a player
// joins a realm whose list names client mods this client lacks, beforeJoin() asks ONCE ("This realm uses X, Y") and
// installs them through the core's own mod_install (catalog ids only: the core finds each in its own Workshop catalog
// and checks its hashes; native code still asks per mod through app.js withNativeOk). A realm can only name ids: never a
// URL, a file or code, and nothing installs without the click. Loaded before app.js; reads its globals when called
// (catalog, realmsCache, installProject, withNativeOk, cardThumb, firstLine, openUrl, pickedClient, fitsClient).
"use strict";
(() => {
  const $ = (id) => document.getElementById(id);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const invoke = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core.invoke : async () => { throw "This is a preview page: open it in the Leyline app."; };
  // The same rules as stream/gamemods.py (ID_RE, VERSION_RE, MAX_MODS, BUILTIN_MODES, GAME_CATS).
  const ID_RE = /^(?:mod:[a-z][a-z0-9-]{2,31}|up:[a-z0-9][a-z0-9._-]{0,63}|mode:[a-z][a-z0-9-]{1,31})$/;
  const VERSION_RE = /^[0-9A-Za-z][0-9A-Za-z.+-]{0,31}$/;
  const MAX_MODS = 32;
  const BUILTIN = ["minigolf", "bumper-balls", "coin-dash", "community"];
  const GAME_CATS = ["sandbox", "maps", "encounters", "conversions"];

  let modeList = null;   // modes_list when the Modes page has not loaded them yet
  const projects = () => (typeof catalog !== "undefined" && catalog && catalog.projects) || [];
  const realms = () => (typeof realmsCache !== "undefined" && Array.isArray(realmsCache) ? realmsCache : []);
  const modes = () => {
    const m = window.LeylineModes && window.LeylineModes.modes ? window.LeylineModes.modes() : null;
    return (m && m.length ? m : modeList) || [];
  };
  async function loadModes() {
    if (modes().length) return;
    try { const r = await invoke("modes_list"); modeList = (r && r.modes) || []; } catch (_) { modeList = modeList || []; }
  }

  function cleanMods(v) {
    const out = [], seen = new Set();
    for (let m of (Array.isArray(v) ? v : []).slice(0, MAX_MODS * 2)) {
      if (typeof m === "string") m = { id: m };
      if (!m || typeof m.id !== "string" || !ID_RE.test(m.id) || seen.has(m.id)) continue;
      seen.add(m.id);
      const one = { id: m.id };
      if (typeof m.version === "string" && VERSION_RE.test(m.version)) one.version = m.version;
      out.push(one);
      if (out.length >= MAX_MODS) break;
    }
    return out;
  }
  // Our realm's list when the realm list does not carry one (an older site): what it runs by its own config.
  function officialMods() {
    const live = modes().filter((m) => m.kind === "realm" && m.status === "live").map((m) => m.id);
    const list = (live.length ? live : BUILTIN).map((id) => ({ id: "mode:" + id }));
    for (const p of projects()) if (p.source === "packaged" && p.on_wrath === true) list.push({ id: p.id, version: p.version });
    return cleanMods(list);
  }
  const realmMods = (r) => (r && Array.isArray(r.mods) && r.mods.length ? cleanMods(r.mods) : r && r.official ? officialMods() : []);
  const side = (id, p) => (id.startsWith("mode:") ? "realm" : id.startsWith("up:") ? "client" : ({ ui: "client", both: "both", realm: "realm" })[(p || {}).kind] || "client");
  const byId = (id) => projects().find((p) => p.id === id) || null;
  const nameOf = (id) => {
    if (id.startsWith("mode:")) { const m = modes().find((x) => x.id === id.slice(5)); return m ? m.title : id.slice(5); }
    const p = byId(id);
    return p ? p.name : id.replace(/^(mod|up):/, "");
  };
  // A catalog entry the core can install and check: a packaged mod with its download and hash, or a reviewed upstream addon.
  const installable = (p) => !!p && ((p.source === "packaged" && p.download && p.sha256) || (p.source === "upstream" && p.install));

  function entries() {
    const out = [];
    for (const m of modes()) {
      if (m.kind !== "realm") continue;
      out.push({ id: "mode:" + m.id, mode: m, name: m.title, line: m.line, status: m.status === "coming" ? "coming" : "live", realm: true, addon: true, client: false, kind: "mode" });
    }
    for (const p of projects()) {
      if (!["packaged", "upstream"].includes(p.source)) continue;
      if (window.LeylineShelf ? window.LeylineShelf.group(p) !== "game" : !GAME_CATS.includes(p.category)) continue;   // a mod, in Game modes
      const s = side(p.id, p);
      out.push({ id: p.id, project: p, name: p.name, line: p.description || "", status: p.status === "coming" ? "coming" : "live",
        realm: s !== "client", addon: false, client: s !== "realm", kind: "mod" });
    }
    return out;
  }
  const runsOn = (id) => realms().filter((r) => r.state === "live" && realmMods(r).some((m) => m.id === id));

  // ------------------------------------------------------------------ the card
  function card(g) {
    const c = typeof pickedClient === "function" ? pickedClient() : null;
    const art = el("article", "mod-card dc-card gm-card");
    art.dataset.id = g.id;
    const thumb = typeof cardThumb === "function" ? cardThumb(g.project || { id: g.id, name: g.name, category: "sandbox" }) : el("div", "dc-thumb");
    thumb.append(el("span", "dc-tag", g.kind === "mode" ? "Leyline Mode" : "Game mode"));
    const body = el("div", "dc-body");
    body.append(el("b", "name", g.name));
    const line = typeof firstLine === "function" ? firstLine(g.line) : g.line;
    const desc = el("p", "desc", line);
    if (g.line && g.line !== line) desc.title = g.line;
    body.append(desc);
    const tags = el("div", "gm-tags");
    if (g.status === "coming") tags.append(el("span", "gm-tag soon", "Coming"));
    tags.append(el("span", "gm-tag", g.realm ? "Needs a realm that allows it" : "Works on any realm"));
    if (g.addon) { const t = el("span", "gm-tag", "Plays through the Leyline addons"); t.title = "Leyline offers them when you press Play"; tags.append(t); }
    else if (g.client) tags.append(el("span", "gm-tag", "Adds files to your game"));
    body.append(tags);
    const on = runsOn(g.id);
    const where = el("p", "gm-on" + (on.length ? "" : " off"));
    if (on.length) {
      where.append(el("b", "", "Play it on: "), document.createTextNode(on.slice(0, 3).map((r) => r.name).join(", ") + (on.length > 3 ? ` and ${on.length - 3} more` : "")));
      where.title = on.map((r) => r.name).join(", ");
    } else if (g.status === "live") where.textContent = "No live realm runs it right now.";
    body.append(where);
    const foot = el("div", "dc-foot");
    const acts = el("div", "acts");
    if (g.kind === "mode" && g.status === "live" && on.length && window.LeylineModes) {
      const b = el("button", "btn small primary", "Play");
      b.type = "button";
      b.title = `Play ${g.name} on ${on[0].name}`;
      b.addEventListener("click", () => {
        if (window.LeylineModes.play) { window.LeylineModes.show(g.mode.id); window.LeylineModes.play(g.mode, b); }
        else window.LeylineModes.show(g.mode.id);
      });
      acts.append(b);
    }
    const p = g.project;
    if (p) {
      const mine = (typeof installed !== "undefined" ? installed : []).find((x) => x.key === p.id);
      if (g.client && installable(p)) {
        const b = el("button", "btn small" + (mine ? " done" : " primary"), mine ? "Installed ✓" : "Install");
        b.type = "button";
        if (!c) { b.disabled = true; b.title = "Add a game client first"; }
        else if (typeof fitsClient === "function" && !fitsClient(p, c)) { b.disabled = true; b.title = `Made for ${p.builds.join(" / ")} clients`; }
        else if (mine) b.disabled = true;
        b.addEventListener("click", () => installProject(p, b).then(render));
        acts.append(b);
      }
      if (p.page || p.repo) {
        const v = el("button", "btn small", "View ↗");
        v.type = "button";
        v.addEventListener("click", () => openUrl(p.page || p.repo));
        acts.append(v);
      }
    }
    foot.append(el("div", "dc-meta"), acts);
    body.append(foot);
    art.append(thumb, body);
    return art;
  }

  // Mods › Game modes in the full list: the Leyline Modes (they are not in the mods list), above the list's own game
  // modes (app.js draws those as ordinary cards).
  function render() {
    const box = $("game-discover");
    if (!box) return;
    const list = entries().filter((g) => g.kind === "mode");
    const live = list.filter((g) => g.status === "live"), soon = list.filter((g) => g.status === "coming");
    const kids = [el("h3", "gm-h", "Leyline Modes"), el("p", "lede", "Game modes that run on any realm with the Leyline realm scripts. Some need a realm that allows them: realms say which mods they use, and Leyline adds the ones your game needs when you join.")];
    const grid = el("div", "mod-grid dc-grid dc-big");
    for (const g of live) grid.append(card(g));
    kids.push(live.length ? grid : el("p", "hint dc-empty", "No Leyline Modes are listed right now."));
    if (soon.length) {
      kids.push(el("h3", "gm-h", "Coming"));
      const g2 = el("div", "mod-grid dc-grid dc-big");
      for (const g of soon) g2.append(card(g));
      kids.push(g2);
    }
    box.replaceChildren(...kids);
  }
  async function show() {
    render();
    await loadModes();
    render();
  }

  // ------------------------------------------------------------------ the join offer
  // What joining realm r with client c needs: the catalog mods with a client part this client lacks (get), the ones
  // it has, and the names the catalog doesn't know (left out: a realm can't make the launcher fetch anything else).
  function plan(r, c, have) {
    const keys = new Set((have || []).map((m) => m.key));
    const out = { get: [], have: [], realmOnly: [], unknown: [], leyline: false };
    for (const m of realmMods(r)) {
      if (m.id.startsWith("mode:")) { out.realmOnly.push(m.id); out.leyline = true; continue; }
      const p = byId(m.id);
      if (!p) { out.unknown.push(m.id); continue; }
      if (side(m.id, p) === "realm") { out.realmOnly.push(m.id); continue; }
      if (keys.has(p.id)) { out.have.push(p.id); continue; }
      if (!installable(p) || (c && typeof fitsClient === "function" && !fitsClient(p, c))) { out.unknown.push(m.id); continue; }
      out.get.push(p);
    }
    return out;
  }
  // The modal: { title, body, list, yes, no, cancel } -> "yes" | "no" | "cancel"
  function ask(o) {
    return new Promise((resolve) => {
      let d = $("gm-dlg");
      if (!d) {
        d = el("dialog", "dlg gm-dlg");
        d.id = "gm-dlg";
        d.setAttribute("aria-labelledby", "gm-dlg-title");
        document.body.append(d);
      }
      const h = el("h3", "", o.title);
      h.id = "gm-dlg-title";
      const ul = el("ul", "mk-list");
      for (const x of o.list) ul.append(el("li", "", x));
      const row = el("div", "row");
      const done = (v) => { if (d.open) d.close(); resolve(v); };
      const mk = (label, cls, v) => { const b = el("button", cls, label); b.type = "button"; b.addEventListener("click", () => done(v)); return b; };
      const yes = mk(o.yes, "btn primary", "yes");
      row.append(yes, mk(o.no, "btn", "no"), mk("Cancel", "btn", "cancel"));
      d.oncancel = (ev) => { ev.preventDefault(); done("cancel"); };
      d.replaceChildren(h, el("p", "", o.body), ul, el("p", "small", o.fine), row);
      if (d.showModal) { d.showModal(); yes.focus(); } else resolve(window.confirm(o.title) ? "yes" : "no");
    });
  }
  // Before PLAY on realm r with client c: { go: false } when the player cancelled; leyline: true when this offer already
  // asked about the Leyline addons (so addons.js beforePlay doesn't ask twice). Nothing installs without "yes".
  async function beforeJoin(r, c, sayFn) {
    const GO = { go: true, leyline: false };
    if (!r || !c) return GO;
    if (!projects().length) return GO;   // no catalog yet: nothing we could check, PLAY goes on
    let have = [];
    try { have = ((await invoke("mods_installed", { client: c.id })) || {}).mods || []; } catch (_) { have = []; }
    const pl = plan(r, c, have);
    let leyline = false;   // the Leyline addons (addons.js), for a realm that runs Leyline Modes
    if (pl.leyline && window.LeylineAddons && c.build === "3.3.5a") {
      const st = window.LeylineAddons.status && window.LeylineAddons.status();
      const mine = st && st.clients ? st.clients.find((x) => x.client === c.id) : null;
      leyline = !!(st && st.pack && mine && !mine.installed);
    }
    if (!pl.get.length && !leyline) return GO;
    const names = pl.get.map((p) => p.name).concat(leyline ? ["Leyline addons (game modes, golf)"] : []);
    const a = await ask({
      title: `${r.name} uses ${names.length === 1 ? names[0] : names.length + " mods your game doesn't have"}`,
      body: `This realm uses ${names.join(", ")}. Get them and play?`,
      list: names,
      fine: "Leyline downloads them and checks every file first. Remove them any time in Mods & Addons › Installed.",
      yes: "Get them and play", no: "Play without them",
    });
    if (a === "cancel") return { go: false, leyline };
    if (a === "no") return { go: true, leyline };
    for (const p of pl.get) {
      if (sayFn) sayFn(`Getting ${p.name}…`);
      try {
        const run = () => invoke("mod_install", { client: c.id, project: p.id });
        await (typeof withNativeOk === "function" ? withNativeOk(run) : run());
      } catch (e) {
        if (sayFn) sayFn(`${p.name} didn't go in: ${String(e).slice(0, 160)}. Playing without it.`, "warn");
      }
    }
    if (leyline) { try { await window.LeylineAddons.install(c); } catch (_) { /* PLAY goes on */ } }
    if (typeof loadInstalled === "function") loadInstalled(true);
    return { go: true, leyline };
  }

  document.addEventListener("leyline:catalog", () => { const b = $("game-discover"); if (b && !b.hidden) render(); });
  window.LeylineGameMods = { entries, render, show, plan, beforeJoin, realmMods, cleanMods, officialMods, runsOn, nameOf, card, loadModes };
})();
