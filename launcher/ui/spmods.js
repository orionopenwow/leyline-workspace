// Leyline: "Mods in this world" on the Single player page (owner 12 Nov: "make sure it's easy to get mods from the mod
// store for singleplayer and user has interface for that in the single player menu to see what mods they have on").
//
// One panel (#spm, after Your world's settings, before Friends) lists what's on for the single player world:
//   * In your game folder: the mods, addons, HD packs and UI looks Leyline put in the game folder the single player
//     worlds play with (LeylineWorlds.folder(), the one world_find_folder picked), from mods_installed; the look in use
//     (looks_state); the Leyline add-ons (addons_status). Each: Turn off / Turn on where the core can (mod_set_enabled),
//     Update when one is ready, Remove, and its page.
//   * Built into your world: the engine's own modules (world_mods_get, host/worldmods.rs), Off / On for the optional ones
//     (world_mods_set). A running world takes the change at its next start: Restart now (oneclick.js's restart).
// Everything here is read from disk by the core: no network to show what's on. Only Update and Get mods go online.
//
// "Get mods for single player" opens Mods & Addons › Discover on that game folder with "Works with this game folder"
// on (app.js LeylineApp.modsForSingle); a card there for another folder offers "Add to single player" (app.js
// decorateCard, reading has() here). After an install anywhere, app.js sends leyline:mods-changed and this panel reads
// again. Pending items stay pending: this panel installs nothing new except the Leyline add-ons' own pack; every other
// install is the mods page's own card.
// The list is the list kit's (listkit.js, docs/LAUNCHER-LISTS.md). Text only (textContent). Loaded after oneclick.js.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (typeof e === "string" ? e : (fb || "Something went wrong. Try again.")));
  const missing = (e) => /not found|unknown (command )?/i.test(String(e || ""));
  const mk = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
  const btn = (label, cls, on, id) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (id) b.id = id; if (on) b.addEventListener("click", on); return b; };
  const byId = (id) => document.getElementById(id);
  const root = byId("spm");
  const WORLDS = [["single", "Single player"], ["vanilla", "Original world"]];
  const titleOf = (k) => (WORLDS.find((w) => w[0] === k) || [k, k])[1];
  const KIND_KEY = "leyline.spm.kind";
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* fine */ } } };

  // TODO(owner): his words for the panel
  const COPY = {
    title: "Mods in this world",
    lede: "What's on when you play single player: the mods in your game folder, and the ones built into your world.",
    none: "No mods yet",
    noneLine: "Add-ons, HD packs and looks from the mod store go into the game folder your world plays with.",
    noFolder: "Single player plays with your own game folder. Add it, and its mods show here.",
    get: "Get mods for single player",
    browse: "Browse the mod store",
    builtLine: "Built into your world's engine. Turn the optional ones off or on; it changes at the world's next start.",
  };

  const S = {
    kind: store.get(KIND_KEY) === "vanilla" ? "vanilla" : "single",
    client: null,        // the game folder the single player worlds play with
    mods: [],            // mods_installed for it
    looks: null,         // looks_state for it
    pack: null,          // the Leyline add-ons for it (addons_status' client row)
    world: null,         // world_mods_get
    worldGone: false,    // an older core: no world mods
    running: false,
    restart: false,
    busy: "",            // the row key with something running
    line: ["", ""],
    loading: false,
    loaded: false,
  };
  const keys = new Set();   // what's in the single player folder (has(): app.js's "Add to single player")

  const W = () => window.LeylineWorlds || null;
  const catalog = () => { const c = window.LeylineApp && window.LeylineApp.catalog ? window.LeylineApp.catalog() : null; return (c && c.projects) || []; };
  // the mods list's own entries, and its looks (themes.js hands those to looks.js: LeylineLooks.list())
  const looksList = () => (window.LeylineLooks && window.LeylineLooks.list ? window.LeylineLooks.list() : []);
  const projectOf = (id) => catalog().find((p) => p.id === id) || looksList().find((p) => p.id === id) || null;
  const pageOf = (id) => { const p = projectOf(id); return p ? p.page || p.repo || "" : ""; };
  async function openPage(url) { try { await call("open_url", { url }); } catch (e) { say(oops(e, "That page didn't open."), "bad"); } }
  function say(t, k) { S.line = [t || "", k || ""]; paint(); }

  // what a game folder row is: an HD pack, a UI look, an addon or a mod (shelf.js: one rule)
  function kindOf(m) {
    const p = projectOf(m.key);
    if (p && p.source === "look") {
      const t = p.look && p.look.type;
      return t === "graphics-pack" ? ["pack", "HD pack"] : t === "ui-look" ? ["look", "UI look"] : ["look", "Look"];
    }
    const sh = window.LeylineShelf ? window.LeylineShelf.installed(m, catalog()) : "mods";
    return sh === "addons" ? ["addon", "Addon"] : ["mod", "Mod"];
  }

  // ---------------------------------------------------------------- reading (all from disk)
  async function read() {
    if (!root || S.loading) return;
    S.loading = true;
    if (!S.loaded) paint();
    const w = W();
    S.client = w && w.folder ? w.folder() : null;
    keys.clear();
    if (S.client) {
      const id = S.client.id;
      const [m, l, a] = await Promise.all([
        call("mods_installed", { client: id }).catch(() => null),
        call("looks_state", { client: id }).catch(() => null),
        (async () => {
          const A = window.LeylineAddons;
          let ov = A && A.status ? A.status() : null;
          if (!ov) { try { ov = await call("addons_status", { refresh: false }); } catch (_) { ov = null; } }
          return ov && Array.isArray(ov.clients) ? ov.clients.find((x) => x.client === id) || null : null;
        })(),
      ]);
      S.mods = m && Array.isArray(m.mods) ? m.mods : [];
      S.looks = l && typeof l === "object" ? l : null;
      S.pack = a;
      for (const x of S.mods) keys.add(x.key);
    } else { S.mods = []; S.looks = null; S.pack = null; }
    try {
      const v = await call("world_mods_get", { kind: S.kind });
      S.world = v && Array.isArray(v.mods) ? v : null;
      S.running = !!(v && v.running);
      S.worldGone = false;
    } catch (e) {
      S.world = null;
      S.worldGone = missing(e);
      try { const st = w ? await w.status(S.kind) : null; S.running = !!(st && st.running); } catch (_) { S.running = false; }
    }
    S.loading = false;
    S.loaded = true;
    paint();
    document.dispatchEvent(new CustomEvent("leyline:spmods", { detail: { client: S.client ? S.client.id : null } }));
  }

  // ---------------------------------------------------------------- the rows
  function rows() {
    const out = [];
    for (const m of S.mods) {
      const [k, tag] = kindOf(m);
      out.push({ key: "m:" + m.key, kind: k, tag, where: "folder", name: m.name, version: m.version || "", on: m.enabled !== false,
        canToggle: true, update: m.update || "", page: pageOf(m.key), src: m, line: lineOf(m) });
    }
    // the look in use that isn't a file in the folder (a preset of the game's own settings)
    const st = S.looks;
    if (st && Array.isArray(st.on)) {
      for (const id of st.on) {
        if (keys.has(id) || (st.how && st.how[id] !== "preset")) continue;
        const p = projectOf(id);
        out.push({ key: "l:" + id, kind: "look", tag: "Look", where: "folder", name: p ? p.name : "A look", version: "", on: true,
          canToggle: false, update: "", page: "", look: id, line: "In use: your game's graphics settings." });
      }
    }
    const a = S.pack;
    if (a && a.supported && (a.installed || a.update)) {
      out.push({ key: "a:pack", kind: "pack-ll", tag: "Leyline", where: "folder", name: "Leyline add-ons", version: a.installed || "", on: true,
        canToggle: false, update: a.update ? "ready" : "", page: "", pack: a,
        line: a.missing && a.missing.length ? `A folder has gone: ${a.missing.join(", ")}. Install puts it back.` : "Game modes, golf, Edit mode and your Den, in the game." });
    }
    if (S.world) {
      for (const x of S.world.mods) {
        out.push({ key: "w:" + x.id, kind: "world", tag: x.locked ? "Always on" : "World", where: "world", name: x.name, version: "", on: !!x.on,
          canToggle: !x.locked, update: "", page: "", world: x, line: x.locked && x.note ? `${x.line} ${x.note}` : x.line });
      }
    }
    return out;
  }
  function lineOf(m) {
    const what = [];
    if (m.addons && m.addons.length) what.push("Addons: " + m.addons.join(", "));
    if (m.patches && m.patches.length) what.push("Patch: " + m.patches.join(", "));
    if (!what.length && m.credits) what.push(m.credits);
    return what.join(" · ");
  }
  const folderRows = () => rows().filter((r) => r.where === "folder");

  // ---------------------------------------------------------------- the actions
  async function act(r, what) {
    if (S.busy) return;
    S.busy = r.key;
    const c = S.client;
    try {
      if (r.kind === "world") {
        say(`${what === "on" ? "Turning on" : "Turning off"} ${r.name}…`);
        const v = await call("world_mods_set", { kind: S.kind, id: r.world.id, on: what === "on" });
        if (v && Array.isArray(v.mods)) S.world = Object.assign({}, S.world, { mods: v.mods });
        S.restart = !!(v && v.restart);
        S.busy = "";
        say((v && v.message) || "Saved.", "ok");
        return;
      }
      if (r.kind === "pack-ll") {
        const A = window.LeylineAddons;
        if (what === "remove") { if (A && A.remove) await A.remove(c); else await call("addons_remove", { client: c.id }); say("Removed the Leyline add-ons from this game folder.", "ok"); }
        else { say("Updating the Leyline add-ons…"); if (A && A.install) await A.install(c); else await call("addons_install", { client: c.id, replace: false }); say("The Leyline add-ons are up to date.", "ok"); }
      } else if (what === "on" || what === "off") {
        await call("mod_set_enabled", { client: c.id, key: r.src.key, on: what === "on" });
        say(what === "on" ? `Turned on ${r.name}.` : `Turned off ${r.name}.`, "ok");
      } else if (what === "update") {
        say(`Downloading ${r.name} v${r.update}…`);
        const run = () => call("mod_install", { client: c.id, project: r.src.key });
        await (typeof withNativeOk === "function" ? withNativeOk(run) : run());
        say(`Updated ${r.name}.`, "ok");
      } else if (what === "remove") {
        const v = await call("mod_uninstall", { client: c.id, key: r.src.key });
        let t = `Removed ${r.name}.`;
        if (v && v.changed && v.changed.length) t += ` Left the files you changed: ${v.changed.join(", ")}.`;
        say(t, v && v.changed && v.changed.length ? "warn" : "ok");
      } else if (what === "unlook") {
        await call("looks_remove", { client: c.id, look: r.look });
        say(`Stopped using ${r.name}.`, "ok");
      }
    } catch (e) { say(oops(e, "That didn't work. Close the game and try again."), "bad"); }
    S.busy = "";
    // the mods page and its badges catch up too (app.js)
    document.dispatchEvent(new CustomEvent("leyline:mods-changed", { detail: { client: c ? c.id : null, from: "single" } }));
    await read();
  }
  async function restartNow() {
    const O = window.LeylineOneClick;
    S.busy = "restart";
    say("Restarting… Everyone is saved first.");
    try {
      if (O && O.restart) await O.restart(S.kind);
      else { const w = W(); if (w) await w.stopWorld(S.kind); await call("world_start", { kind: S.kind }); }
      S.restart = false;
      S.busy = "";
      say("Restarting with your mods. It's ready in a few seconds.", "ok");
    } catch (e) { S.busy = ""; say(oops(e, "It didn't restart. Press Play to start it."), "bad"); }
  }
  function getMods() {
    if (window.LeylineApp && window.LeylineApp.modsForSingle) window.LeylineApp.modsForSingle();
    else if (window.LeylineNav) window.LeylineNav.show("mods", false, "discover");
  }

  // ---------------------------------------------------------------- one row
  function toggle(r) {
    const b = btn(r.on ? "On" : "Off", "spm-switch" + (r.on ? " on" : ""), () => act(r, r.on ? "off" : "on"));
    b.setAttribute("role", "switch");
    b.setAttribute("aria-checked", String(r.on));
    b.setAttribute("aria-label", `${r.name}: ${r.on ? "on" : "off"}`);
    b.title = r.on ? `Turn off ${r.name}` : `Turn on ${r.name}`;
    return b;
  }
  function render(r, ctx) {
    const row = mk("div", "spm-row" + (r.on ? "" : " off") + (r.update ? " has-update" : ""));
    row.dataset.key = r.key;
    row.dataset.kind = r.kind;
    const top = mk("div", "spm-top");
    const name = mk("span", "spm-name");
    name.append(ctx.hl(r.name));
    top.append(name, mk("span", "sp-dock spm-tag" + (r.kind === "world" && !r.canToggle ? "" : ""), r.tag));
    if (r.version) top.append(mk("span", "spm-ver", "v" + r.version));
    if (r.update) top.append(mk("span", "sp-dock ok spm-upd", r.kind === "pack-ll" ? "Update ready" : `v${r.update} ready`));
    if (!r.on) top.append(mk("span", "sp-dock warn spm-offchip", "Turned off"));
    const line = mk("p", "spm-line");
    line.append(ctx.hl(r.line || ""));
    const acts = mk("div", "spm-acts");
    const off = !!S.busy;
    if (r.canToggle) acts.append(toggle(r));
    if (r.update) acts.append(btn("Update", "btn small primary spm-update", () => act(r, "update")));
    if (r.kind !== "world") {
      if (r.look) acts.append(btn("Stop using", "btn small spm-remove", () => act(r, "unlook")));
      else acts.append(btn("Remove", "btn small danger spm-remove", () => act(r, "remove")));
    }
    if (r.page) { const p = btn("Page ↗", "btn small spm-page", () => openPage(r.page)); p.title = `Open ${r.name}'s page`; acts.append(p); }
    for (const b of acts.querySelectorAll("button")) b.disabled = off;
    row.append(top);
    if (r.line) row.append(line);
    if (acts.childNodes.length) row.append(acts);
    return row;
  }

  // ---------------------------------------------------------------- the list (the list kit)
  let list = null;
  function listMake(box) {
    if (list || !window.LeylineList) return list;
    list = window.LeylineList.create(box, {
      id: "spmods",
      key: (r) => r.key,
      label: COPY.title,
      noun: ["mod", "mods"],
      render,
      search: { fields: (r) => [r.name, r.tag, r.line], placeholder: "Find a mod in this world", label: "Search the mods in this world" },
      tabs: { items: [
        { id: "all", label: "All", suggest: false },
        { id: "folder", label: "Game folder", long: "your game folder", filter: (r) => r.where === "folder" },
        { id: "world", label: "World", long: "your world", filter: (r) => r.where === "world" },
      ], idPrefix: "spm-tab-", label: "Which mods", storageKey: "leyline.spm.tab" },
      filters: [
        { id: "upd", label: "Updates", test: (r) => !!r.update },
        { id: "off", label: "Turned off", test: (r) => !r.on },
      ],
      filtersLabel: "Show",
      group: { of: (r) => (r.where === "world" ? "Built into your world" : "In your game folder"), order: ["In your game folder", "Built into your world"], hideSingle: false },
      primary: (r, node) => {
        const b = node.querySelector(".spm-update:not([disabled])") || node.querySelector(".spm-switch:not([disabled])");
        if (b) b.click();
      },
      empty: (info) => {
        if (info.reason === "loading") return "Reading your world's mods…";
        if (info.reason === "none" || (info.reason === "tab" && info.tab === "folder")) return { text: `${COPY.none}. ${COPY.noneLine}`, actions: [{ label: COPY.browse, primary: true, run: getMods }] };
        return null;
      },
      classes: { body: "spm-body" },
    });
    return list;
  }

  // ---------------------------------------------------------------- the panel
  function head() {
    const h = mk("div", "spm-head");
    const t = mk("div", "spm-titles");
    const h2 = mk("h2", "spm-title", COPY.title);
    h2.id = "spm-title";
    t.append(h2, mk("p", "spm-lede", COPY.lede));
    const tools = mk("div", "spm-tools");
    if (S.client) {
      const f = mk("span", "sp-dock spm-folder", `Game folder: ${S.client.name}`);
      f.id = "spm-folder";
      f.title = S.client.path || "";
      tools.append(f);
    }
    const sel = mk("select", "oc-sel spm-kind");
    sel.id = "spm-kind";
    sel.setAttribute("aria-label", "Which world");
    for (const [k, label] of WORLDS) { const o = mk("option", "", label); o.value = k; o.selected = k === S.kind; sel.append(o); }
    sel.addEventListener("change", () => { S.kind = sel.value; store.set(KIND_KEY, S.kind); S.restart = false; S.line = ["", ""]; read(); });
    if (!S.worldGone) tools.append(sel);
    const get = btn(COPY.get, "btn primary spm-get", getMods, "spm-get");
    get.title = "Opens the mod store with what works in this game folder";
    tools.append(get);
    h.append(t, tools);
    return h;
  }
  // the panel's parts, made once: the list box stays in place, so a repaint keeps the scroll and the focus in it
  let parts = null;
  function skeleton() {
    const box = mk("div", "spm-list");
    box.id = "spm-list";
    parts = { head: mk("div", "spm-hslot"), empty: mk("div", "spm-eslot"), box, foot: mk("div", "spm-foot") };
    root.replaceChildren(parts.head, parts.empty, parts.box, parts.foot);
  }
  function paint() {
    if (!root) return;
    if (!parts) skeleton();
    if (!parts.head.contains(document.activeElement)) parts.head.replaceChildren(head());
    else { const f = byId("spm-folder"); if (f && S.client) f.textContent = `Game folder: ${S.client.name}`; }
    const empty = [];
    if (!S.client && S.loaded) {
      const e = mk("div", "spm-empty");
      e.id = "spm-empty";
      e.append(mk("b", "spm-empty-t", "No game folder yet"), mk("p", "", COPY.noFolder));
      if (window.LeylineLocal && window.LeylineLocal.addFolder) e.append(btn("Add a game folder", "btn small primary", () => window.LeylineLocal.addFolder("solo"), "spm-add-folder"));
      empty.push(e);
    } else if (S.loaded && !folderRows().length) {
      // nothing in the game folder yet: the store, one click away (the world's own modules still list below)
      const e = mk("div", "spm-empty");
      e.id = "spm-empty";
      e.append(mk("b", "spm-empty-t", COPY.none), mk("p", "", COPY.noneLine), btn(COPY.browse, "btn small primary", getMods, "spm-browse"));
      empty.push(e);
    }
    if (!parts.empty.contains(document.activeElement)) parts.empty.replaceChildren(...empty);
    const foot = [];
    if (S.world) foot.push(mk("p", "oc-fine spm-fine", COPY.builtLine));
    else if (S.worldGone) foot.push(mk("p", "oc-fine spm-fine", "Your world's own modules show here with a newer Leyline."));
    const st = mk("p", "status spm-status" + (S.line[1] ? " " + S.line[1] : ""), S.line[0]);
    st.id = "spm-status";
    st.setAttribute("role", "status");
    foot.push(st);
    if (S.restart) {
      const rb = btn("Restart now", "btn small primary", restartNow, "spm-restart");
      rb.disabled = !!S.busy;
      const r = mk("div", "row spm-restart-row");
      r.append(rb, mk("span", "oc-fine", "Saves everyone, then starts your world again with your mods."));
      foot.push(r);
    }
    const had = document.activeElement && document.activeElement.id === "spm-restart";
    parts.foot.replaceChildren(...foot);
    if (had && byId("spm-restart")) byId("spm-restart").focus();
    const l = listMake(parts.box);
    if (l) {
      l.setLoading(!S.loaded);
      l.update(S.loaded ? rows() : []);
    }
  }

  // ---------------------------------------------------------------- when to read
  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "single") read(); });
  // an install, an update or a removal anywhere (the mods page, a card's "Add to single player", the Leyline add-ons)
  document.addEventListener("leyline:mods-changed", (e) => {
    const d = e.detail || {};
    if (d.from === "single") return;   // this panel's own change: it reads again by itself
    if (!d.client || (S.client && d.client === S.client.id) || !S.loaded) read();
  });
  document.addEventListener("leyline:catalog", () => { if (S.loaded) paint(); });   // the names and pages of the store
  document.addEventListener("leyline:state", () => {
    const w = W();
    const c = w && w.folder ? w.folder() : null;
    if ((c && c.id) !== (S.client && S.client.id)) read();
  });
  paint();

  window.LeylineSPMods = {
    read, refresh: read, COPY,
    client: () => S.client,
    has: (id) => keys.has(id),
    state: () => ({ client: S.client ? S.client.id : null, kind: S.kind, rows: rows().map((r) => ({ key: r.key, kind: r.kind, on: r.on, update: r.update })), restart: S.restart, line: S.line.slice() }),
  };
})();
