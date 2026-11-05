// Loadouts (owner 3 Nov: "Encourage downloading mods onto one's own client: style / loadout for clients. Make that part
// intuitive so people can load various versions of their games, templates and other people's realms without losing game
// file access and ease"), over src-tauri/src/loadouts.rs. A loadout is a named set of mods, addons and a graphics preset
// for one game folder ("Clean", "Raider", "Best graphics", "My friend's realm"). Where players already look:
//   - Mods & Addons › Installed: the Loadouts box on top (switch in one click, save what's on, templates, paste a code,
//     copy, rename, share, graphics, delete), with Undo after every switch;
//   - each game folder card (Game folders): "Loadout: <name>" and Change;
//   - joining a realm that suggests a loadout (realms.rs Realm.loadout): "Use this realm's loadout" once, then "Back to ..."
//     on the box.
// Switching is safe: the core only turns Leyline's own installs on and off (they wait in Leyline/disabled/), copies the
// game's WTF folder first, and never deletes or changes a game file. Clean (every Leyline mod off) is always there.
// Missing items are installed through the core's own mod_install (catalog ids only, its own hashes; native code still asks
// through app.js withNativeOk). Templates are the website's stream/loadouts.json (TEMPLATES below, the same list:
// stream/tests/test_loadouts.py), shown only with the items this launcher's catalog has (reviewed entries only).
// Loaded after app.js and addons.js; reads app.js globals when called (catalog, installed, loadInstalled, withNativeOk,
// pickedClient, show).
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
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : String(e));
  const CLEAN = "clean";
  const LEYLINE_ADDONS = "leyline:addons";
  const PRESETS = { low: "Low", classic: "Classic", balanced: "Balanced", high: "High", ultra: "Ultra" };
  const TEMPLATES = /* TEMPLATES-START */
[
  {
    "id": "official",
    "name": "Official realm",
    "line": "The Leyline addons for game modes, golf and your Den, with balanced graphics.",
    "items": [
      "leyline:addons"
    ],
    "gfx": "balanced",
    "builds": [
      "3.3.5a"
    ]
  },
  {
    "id": "quester",
    "name": "Quester",
    "line": "Quest help on the map, gathering spots, item ids and stat values in tooltips.",
    "items": [
      "up:questie",
      "up:gathermate",
      "up:idtip",
      "up:ratingbuster"
    ],
    "gfx": "balanced",
    "builds": [
      "3.3.5a"
    ]
  },
  {
    "id": "raider",
    "name": "Raider",
    "line": "A damage meter, cooldown and aura tracking, raid tools and gear checks.",
    "items": [
      "up:skada",
      "up:tellmewhen",
      "up:weakauras-bunny67",
      "up:sharedmedia",
      "up:blt",
      "up:krt",
      "up:gearscorelite"
    ],
    "gfx": "balanced",
    "builds": [
      "3.3.5a"
    ]
  },
  {
    "id": "clean-frames",
    "name": "Tidy interface",
    "line": "Movable action bars, one sorted bag, clearer unit frames, nameplates and minimap.",
    "items": [
      "up:dominos",
      "up:adibags",
      "up:ruf",
      "up:platebuffs",
      "up:pretty-minimap"
    ],
    "gfx": "",
    "builds": [
      "3.3.5a"
    ]
  },
  {
    "id": "best-graphics",
    "name": "Best graphics",
    "line": "The game's own settings turned all the way up. No files added.",
    "items": [],
    "gfx": "ultra",
    "builds": [
      "1.12",
      "2.4.3",
      "3.3.5a"
    ]
  },
  {
    "id": "old-pc",
    "name": "Old PC",
    "line": "The game's own settings turned down for speed. No files added.",
    "items": [],
    "gfx": "low",
    "builds": [
      "1.12",
      "2.4.3",
      "3.3.5a"
    ]
  }
]
/* TEMPLATES-END */;

  // TODO(owner): the loadouts lines in your own voice (the box's opening line, Home's line, the realm offer)
  const COPY = {
    lede: "Make it yours: save the mods and addons you like as a loadout and switch in one click. Clean is always one click away.",
    clean: "Clean: every mod and addon Leyline added is off. The game as you installed it.",
    safe: "Switching never deletes or changes your game files. Leyline turns its own mods and addons off and on, and keeps a copy of your game settings for each loadout first. Addons you added yourself stay as they are.",
  };

  let st = null;          // loadouts_status for the picked client
  let forClient = null;   // its id
  let busy = false;
  let note = null;        // [text, kind, undoId] after the last thing that happened
  const cardNames = {};   // client id -> the loadout's name, for the game folder cards

  const clients = () => { const s = window.LeylineApp && window.LeylineApp.state ? window.LeylineApp.state() : null; return (s && s.settings && s.settings.clients) || []; };
  const picked = () => (typeof pickedClient === "function" ? pickedClient() : clients()[0] || null);
  const projects = () => (typeof catalog !== "undefined" && catalog && Array.isArray(catalog.projects) ? catalog.projects : []);
  const byId = (id) => projects().find((p) => p.id === id) || null;
  const nameOf = (id) => (id === LEYLINE_ADDONS ? "Leyline addons" : (byId(id) || {}).name
    || ((st && st.installed.find((m) => m.key === id)) || {}).name || id.replace(/^(mod|up|look|local|patch):/, ""));
  const loadoutName = (id) => (id === CLEAN ? "Clean" : ((st && st.loadouts.find((l) => l.id === id)) || {}).name || "your last loadout");
  const fits = (builds, c) => !builds || !builds.length || builds.some((b) => b === "any" || (c && String(c.build).startsWith(b)));
  // installable here: the Leyline addons on a 3.3.5a folder, or a catalog entry the core can install
  const canHave = (id, c) => (id === LEYLINE_ADDONS ? !!c && c.build === "3.3.5a" : !!byId(id));

  // ------------------------------------------------------------------ the dialog (one, reused)
  // { title, body, input: { label, value, readonly }, rows: [nodes], yes, no, third } -> Promise<{ pick: "yes"|"no"|"third"|"cancel", value }>
  function dlg(o) {
    return new Promise((resolve) => {
      let d = $("lo-dlg");
      if (!d) {
        d = el("dialog", "dlg lo-dlg");
        d.id = "lo-dlg";
        d.setAttribute("aria-labelledby", "lo-dlg-title");
        document.body.append(d);
      }
      const h = el("h3", "", o.title);
      h.id = "lo-dlg-title";
      const kids = [h];
      if (o.body) kids.push(el("p", "", o.body));
      let inp = null;
      if (o.input) {
        const lab = el("label", "lo-dlg-in");
        lab.append(el("span", "", o.input.label));
        inp = el("input");
        inp.type = "text";
        inp.id = "lo-dlg-input";
        inp.value = o.input.value || "";
        inp.maxLength = o.input.max || 4000;
        inp.spellcheck = false;
        if (o.input.readonly) inp.readOnly = true;
        lab.append(inp);
        kids.push(lab);
      }
      if (o.rows && o.rows.length) { const box = el("div", "lo-dlg-rows"); box.append(...o.rows); kids.push(box); }
      if (o.fine) kids.push(el("p", "small", o.fine));
      const row = el("div", "row");
      const done = (pick) => { if (d.open) d.close(); resolve({ pick, value: inp ? inp.value : "" }); };
      const mk = (label, cls, pick) => { const b = el("button", cls, label); b.type = "button"; b.addEventListener("click", () => done(pick)); return b; };
      if (o.yes) row.append(mk(o.yes, "btn primary", "yes"));
      if (o.third) row.append(mk(o.third, "btn", "third"));
      row.append(mk(o.no || "Cancel", "btn", "no"));
      kids.push(row);
      d.replaceChildren(...kids);
      d.oncancel = (ev) => { ev.preventDefault(); done("cancel"); };
      if (inp) inp.addEventListener("keydown", (ev) => { if (ev.key === "Enter" && o.yes) { ev.preventDefault(); done("yes"); } });
      // a row's own button (a template's Use) answers for the dialog
      d.answer = (v) => { if (d.open) d.close(); resolve({ pick: "row", value: v }); };
      if (d.showModal) { d.showModal(); (inp && !inp.readOnly ? inp : row.querySelector("button")).focus(); if (inp && inp.readOnly) inp.select(); }
      else resolve({ pick: "no", value: "" });
    });
  }

  // ------------------------------------------------------------------ loading
  async function load(c) {
    c = c || picked();
    forClient = c ? c.id : null;
    if (!c || (c.build && !/^(1\.12|2\.4\.3|3\.3\.5a)/.test(c.build))) { st = null; render(); return null; }
    try { st = await invoke("loadouts_status", { client: c.id }); }
    catch (e) { st = null; note = [oops(e, "Leyline couldn't read this game folder's loadouts."), "bad"]; }
    if (st) cardNames[c.id] = st.active ? loadoutName(st.active) : "";
    render();
    renderCards();
    return st;
  }

  // ------------------------------------------------------------------ switching
  async function installMissing(c, ids) {
    const done = [], left = [];
    for (const id of ids) {
      try {
        if (id === LEYLINE_ADDONS) {
          if (window.LeylineAddons && c.build === "3.3.5a" && (await window.LeylineAddons.install(c))) done.push(nameOf(id)); else left.push(nameOf(id));
          continue;
        }
        const p = byId(id);
        if (!p) { left.push(nameOf(id)); continue; }
        say(`Installing ${p.name}…`, "");
        const run = () => invoke("mod_install", { client: c.id, project: p.id });
        await (typeof withNativeOk === "function" ? withNativeOk(run) : run());
        // Installed opens on the tab of what went in last (Mods or Addons; app.js noteInstalled)
        if (window.LeylineApp && window.LeylineApp.noteInstalled) window.LeylineApp.noteInstalled(p.id, window.LeylineShelf ? window.LeylineShelf.of(p) : "");
        done.push(p.name);
      } catch (e) {
        left.push(nameOf(id));
        console.error("[leyline] loadout install", id, e);
      }
    }
    return { done, left };
  }

  async function switchTo(id, opts = {}) {
    const c = picked();
    if (!c || busy) return false;
    busy = true;
    say(`Switching to ${loadoutName(id)}…`, "");
    let ok = false;
    try {
      const r = await invoke("loadout_switch", { client: c.id, id });
      const parts = [`${r.name} is on in ${c.name}.`];
      if (r.enabled.length) parts.push(`Turned on: ${r.enabled.map(nameOf).join(", ")}.`);
      if (r.disabled.length) parts.push(`Turned off: ${r.disabled.map(nameOf).join(", ")}.`);
      if (!r.enabled.length && !r.disabled.length && !r.missing.length) parts.push("Nothing needed to change.");
      if (r.gfx === "preset") parts.push("Graphics set.");
      else if (r.gfx === "original") parts.push("Graphics back to how they were.");
      let kind = "ok";
      if (r.missing.length) {
        const got = await installMissing(c, r.missing.filter((x) => canHave(x, c)));
        const skipped = r.missing.filter((x) => !canHave(x, c)).map(nameOf).concat(got.left);
        if (got.done.length) parts.push(`Installed: ${got.done.join(", ")}.`);
        if (skipped.length) { parts.push(`Not in the mods list here, left out: ${skipped.join(", ")}.`); kind = "warn"; }
      }
      if (r.problems.length) { parts.push("Couldn't move: " + r.problems.join("; ") + "."); kind = "warn"; }
      note = [parts.join(" "), kind, opts.noUndo ? null : r.previous || CLEAN];
      ok = true;
    } catch (e) {
      note = [oops(e, "Leyline couldn't switch the loadout. Nothing was deleted."), "bad"];
    }
    busy = false;
    if (typeof loadInstalled === "function") await loadInstalled(true); else await load(c);
    await load(c);
    return ok;
  }

  // a template, a realm's or a pasted loadout: a new loadout in this game folder (or the one made from it before), then on
  async function useShared(shared, from) {
    const c = picked();
    if (!c) return false;
    try {
      if (!st || forClient !== c.id) await load(c);
      const had = from && from !== "code" && st ? st.loadouts.find((l) => l.from === from) : null;
      const l = had || await invoke("loadout_add_shared", { client: c.id, shared: { name: shared.name, items: shared.items || [], gfx: shared.gfx || "" }, from: from || "" });
      return await switchTo(l.id);
    } catch (e) {
      note = [oops(e, "Leyline couldn't add that loadout."), "bad"];
      render();
      return false;
    }
  }

  // ------------------------------------------------------------------ the actions
  async function saveNow() {
    const c = picked();
    if (!c) return;
    const a = await dlg({ title: "Save what's on now", body: "A new loadout with the mods and addons that are on in " + c.name + " now.",
      input: { label: "Name", value: "", max: 40 }, yes: "Save" });
    if (a.pick !== "yes" || !a.value.trim()) return;
    try {
      const l = await invoke("loadout_new", { client: c.id, name: a.value, empty: false });
      note = [`Saved ${l.name}. Switch to it any time.`, "ok"];
      await switchTo(l.id, { noUndo: true });
    } catch (e) { note = [oops(e, "Leyline couldn't save the loadout."), "bad"]; render(); }
  }

  function templatesFor(c) {
    return TEMPLATES.filter((t) => fits(t.builds, c))
      .map((t) => Object.assign({}, t, { items: t.items.filter((i) => canHave(i, c)) }))
      .filter((t) => t.items.length || t.gfx);
  }

  async function pickTemplate() {
    const c = picked();
    if (!c) return;
    const list = templatesFor(c);
    const d = () => $("lo-dlg");
    const rows = list.map((t) => {
      const r = el("div", "lo-tpl");
      r.dataset.id = t.id;
      const txt = el("div", "lo-tpl-t");
      txt.append(el("b", "", t.name), el("span", "", t.line));
      const bits = [];
      if (t.items.length) bits.push(t.items.map(nameOf).join(", "));
      if (t.gfx) bits.push("Graphics: " + PRESETS[t.gfx]);
      txt.append(el("small", "", bits.join(" · ")));
      const b = el("button", "btn small primary", "Use");
      b.type = "button";
      b.addEventListener("click", () => d().answer(t.id));
      r.append(txt, b);
      return r;
    });
    const a = await dlg({ title: "Start from a template", body: list.length ? "Ready-made loadouts, made only from reviewed mods and addons. Using one adds it as a new loadout: your own stay as they are."
      : "No templates fit this game folder yet.", rows, no: "Close" });
    if (a.pick !== "row") return;
    const t = list.find((x) => x.id === a.value);
    if (t) await useShared(t, "template:" + t.id);
  }

  async function paste() {
    const c = picked();
    if (!c) return;
    const a = await dlg({ title: "Paste a loadout code", body: "A friend's loadout code names the mods and addons to use. Leyline installs only what it finds in its own mods list, and your loadouts stay as they are.",
      input: { label: "Code", value: "" }, yes: "Add and use" });
    if (a.pick !== "yes" || !a.value.trim()) return;
    try {
      const l = await invoke("loadout_import", { client: c.id, code: a.value.trim() });
      await switchTo(l.id);
    } catch (e) { note = [oops(e, "That code didn't work. Check you copied all of it."), "bad"]; render(); }
  }

  async function share(l) {
    const c = picked();
    try {
      const code = await invoke("loadout_code", { client: c.id, id: l.id });
      const a = await dlg({ title: `Share ${l.name}`, body: "Send this code to a friend. It lists mods and addons by name only, never files. Mods from your own PC are left out.",
        input: { label: "Code", value: code, readonly: true }, yes: "Copy", no: "Close" });
      if (a.pick === "yes") {
        try { await navigator.clipboard.writeText(code); note = ["Copied the code.", "ok"]; } catch (_) { note = ["Select the code and copy it.", "warn"]; }
        render();
      }
    } catch (e) { note = [oops(e, "Leyline couldn't make the code."), "bad"]; render(); }
  }

  async function copyOf(l) {
    const c = picked();
    const a = await dlg({ title: `Copy ${l.name}`, input: { label: "Name of the copy", value: `${l.name} copy`, max: 40 }, yes: "Copy" });
    if (a.pick !== "yes" || !a.value.trim()) return;
    try { const n = await invoke("loadout_copy", { client: c.id, id: l.id, name: a.value }); note = [`Made ${n.name}.`, "ok"]; }
    catch (e) { note = [oops(e, "Leyline couldn't copy the loadout."), "bad"]; }
    await load(c);
  }

  async function renameOf(l) {
    const c = picked();
    const a = await dlg({ title: `Rename ${l.name}`, input: { label: "New name", value: l.name, max: 40 }, yes: "Rename" });
    if (a.pick !== "yes" || !a.value.trim()) return;
    try { const n = await invoke("loadout_rename", { client: c.id, id: l.id, name: a.value }); note = [`Renamed to ${n.name}.`, "ok"]; }
    catch (e) { note = [oops(e, "Leyline couldn't rename the loadout."), "bad"]; }
    await load(c);
  }

  async function removeOf(l) {
    const c = picked();
    const a = await dlg({ title: `Delete ${l.name}?`, body: "Only the loadout goes. Every mod and addon stays installed, and nothing in your game changes.", yes: "Delete", no: "Keep it" });
    if (a.pick !== "yes") return;
    try { await invoke("loadout_delete", { client: c.id, id: l.id }); note = [`Deleted ${l.name}.`, "ok"]; }
    catch (e) { note = [oops(e, "Leyline couldn't delete the loadout."), "bad"]; }
    await load(c);
  }

  async function setGfx(l, gfx) {
    const c = picked();
    try { await invoke("loadout_set_gfx", { client: c.id, id: l.id, gfx }); note = [gfx ? `${l.name} uses ${PRESETS[gfx]} graphics from its next switch.` : `${l.name} leaves the graphics as they are.`, "ok"]; }
    catch (e) { note = [oops(e, "Leyline couldn't save that."), "bad"]; }
    await load(c);
  }

  // ------------------------------------------------------------------ the box on Mods & Addons › Installed
  function say(text, kind) { note = [text, kind || ""]; paintNote(); }
  function paintNote() {
    const p = $("lo-status");
    if (!p) return;
    p.replaceChildren();
    p.className = "status lo-status" + (note && note[1] ? " " + note[1] : "");
    if (!note) return;
    p.append(document.createTextNode(note[0]));
    if (note[2] && !busy) {
      const u = el("button", "link lo-undo", "Undo");
      u.type = "button";
      u.title = `Back to ${loadoutName(note[2])}`;
      const back = note[2];
      u.addEventListener("click", () => switchTo(back, { noUndo: true }));
      p.append(" ", u);
    }
  }

  function box() {
    let b = $("lo-box");
    if (b) return b;
    const pane = $("pane-installed");
    if (!pane) return null;
    b = el("section", "lo-box");
    b.id = "lo-box";
    b.setAttribute("aria-label", "Loadouts");
    pane.insertBefore(b, $("inst-tabs") || pane.firstChild);
    return b;
  }

  function chip(id, name, on, from) {
    const wrap = el("span", "lo-chip" + (on ? " on" : "") + (id === CLEAN ? " clean" : ""));
    wrap.dataset.id = id;
    const b = el("button", "lo-chip-b", name);
    b.type = "button";
    b.setAttribute("aria-pressed", String(on));
    b.title = id === CLEAN ? COPY.clean : on ? `${name} is on` : `Switch to ${name}`;
    if (from && from.startsWith("realm:")) b.title += " (a realm's loadout)";
    b.disabled = busy;
    b.addEventListener("click", () => { if (!on) switchTo(id); });
    wrap.append(b);
    if (id !== CLEAN && !on) {
      const x = el("button", "lo-chip-x", "×");
      x.type = "button";
      x.setAttribute("aria-label", `Delete ${name}`);
      x.title = `Delete ${name} (nothing in your game changes)`;
      x.addEventListener("click", () => removeOf(st.loadouts.find((l) => l.id === id)));
      wrap.append(x);
    }
    return wrap;
  }

  function render() {
    const b = box();
    if (!b) return;
    const c = picked();
    b.hidden = !c || !st || forClient !== (c && c.id);
    if (b.hidden) { b.replaceChildren(); return; }
    const head = el("div", "card-head lo-head");
    head.append(el("span", "", "LOADOUTS"), el("span", "lo-for", c.name));
    const lede = el("p", "hint lo-lede", COPY.lede);
    const chips = el("div", "lo-chips");
    chips.setAttribute("role", "group");
    chips.setAttribute("aria-label", "Loadouts for " + c.name);
    chips.append(chip(CLEAN, "Clean", st.active === CLEAN, ""));
    for (const l of st.loadouts) chips.append(chip(l.id, l.name, st.active === l.id, l.from));
    const kids = [head, lede, chips];
    if (!st.active) kids.push(el("p", "hint lo-first", st.installed.length
      ? "No loadout is on yet: your mods and addons are as you set them up. Save them as your first loadout, or try Clean."
      : "No mods or addons from Leyline here yet. Start from a template, or install some and save them as a loadout."));
    if (st.active === CLEAN && st.drift.length) {
      const p = el("p", "hint lo-drift", `${st.drift.length === 1 ? "1 mod is" : st.drift.length + " mods are"} on while Clean is chosen: ${st.drift.map(nameOf).join(", ")}. `);
      const s = el("button", "link", "Save them as a loadout");
      s.type = "button";
      s.addEventListener("click", saveNow);
      p.append(s);
      kids.push(p);
    }
    const active = st.loadouts.find((l) => l.id === st.active);
    if (active && active.from.startsWith("realm:") && st.previous && st.previous !== st.active) {
      const p = el("p", "hint lo-realm", `This is a realm's loadout. `);
      const back = el("button", "btn small", `Back to ${loadoutName(st.previous)}`);
      back.type = "button";
      back.className = "btn small lo-back";
      back.addEventListener("click", () => switchTo(st.previous));
      p.append(back);
      kids.push(p);
    }
    const acts = el("div", "acts lo-acts");
    const btn = (label, fn, cls) => { const x = el("button", "btn small" + (cls ? " " + cls : ""), label); x.type = "button"; x.disabled = busy; x.addEventListener("click", fn); acts.append(x); return x; };
    btn("Save what's on now", saveNow, "primary");
    btn("Templates", pickTemplate);
    btn("Paste a code", paste);
    if (active) {
      btn("Share", () => share(active));
      btn("Copy", () => copyOf(active));
      btn("Rename", () => renameOf(active));
      const lab = el("label", "inline lo-gfx");
      lab.append(el("span", "", "Graphics "));
      const sel = el("select");
      sel.setAttribute("aria-label", `Graphics for ${active.name}`);
      for (const [v, n] of [["", "As they are"]].concat(Object.entries(PRESETS))) { const o = el("option", "", n); o.value = v; sel.append(o); }
      sel.value = active.gfx || "";
      sel.disabled = busy;
      sel.addEventListener("change", () => setGfx(active, sel.value));
      lab.append(sel);
      acts.append(lab);
    }
    if (st.active && st.active !== CLEAN) btn("Back to Clean", () => switchTo(CLEAN), "lo-clean");
    const status = el("p", "status lo-status");
    status.id = "lo-status";
    status.setAttribute("role", "status");
    kids.push(acts, status, el("p", "hint lo-safe", COPY.safe));
    b.replaceChildren(...kids);
    paintNote();
  }

  // ------------------------------------------------------------------ the game folder cards
  function renderCards() {
    document.querySelectorAll("#cl-list .card[data-client]").forEach((card) => {
      const c = clients().find((x) => x.id === card.dataset.client);
      if (c) clientCard(card, c);
    });
  }
  async function clientCard(card, c) {
    if (!c || !/^(1\.12|2\.4\.3|3\.3\.5a)/.test(c.build || "")) return;
    let slot = card.querySelector(".lo-slot");
    if (!slot) {
      slot = el("div", "lo-slot");
      slot.dataset.client = c.id;
      const acts = card.querySelector(":scope > .acts");
      if (acts) card.insertBefore(slot, acts); else card.append(slot);
    }
    if (!(c.id in cardNames)) {
      cardNames[c.id] = "";
      try { const s = await invoke("loadouts_status", { client: c.id }); cardNames[c.id] = s.active === CLEAN ? "Clean" : ((s.loadouts.find((l) => l.id === s.active) || {}).name || ""); }
      catch (_) { /* the card says "As you set it up" */ }
    }
    const line = el("span", "lo-slot-t");
    line.append(el("b", "", "Loadout: "), document.createTextNode(cardNames[c.id] || "As you set it up"));
    const go = el("button", "btn small lo-change", "Change");
    go.type = "button";
    go.title = "Mods & Addons › Installed › Loadouts";
    go.addEventListener("click", () => openFor(c));
    slot.replaceChildren(line, go);
  }
  function openFor(c) {
    const s = window.LeylineApp && window.LeylineApp.state ? window.LeylineApp.state() : null;
    if (s && c && s.settings.selected_client !== c.id) {
      s.settings.selected_client = c.id;
      if ($("play-client")) $("play-client").value = c.id;
      if (typeof renderClientPickers === "function") renderClientPickers();
      if (typeof saveSelection === "function") saveSelection();
    }
    if (window.LeylineNav) window.LeylineNav.show("mods", false, "installed");
    setTimeout(() => { const b = $("lo-box"); if (b && b.scrollIntoView) b.scrollIntoView({ block: "start" }); }, 0);
  }

  // ------------------------------------------------------------------ joining a realm that suggests a loadout
  // Before the game starts (app.js PLAY and the realm cards). { go: false } only on Cancel.
  async function beforeJoin(r, c) {
    if (!r || !r.loadout || !c) return { go: true };
    const from = "realm:" + (r.slug || r.name);
    try {
      if (!st || forClient !== c.id) await load(c);
      if (!st) return { go: true };
      const on = st.loadouts.find((l) => l.id === st.active);
      if (on && on.from === from) return { go: true };   // already on
      let asked = false;
      try { asked = localStorage.getItem("leyline.lo.keep." + from) === "1"; } catch (_) { /* fine */ }
      if (asked) return { go: true };
      const items = (r.loadout.items || []).filter((i) => canHave(i, c));
      const bits = [];
      if (items.length) bits.push("it uses " + items.map(nameOf).join(", "));
      if (r.loadout.gfx) bits.push(`${PRESETS[r.loadout.gfx] || r.loadout.gfx} graphics`);
      const a = await dlg({
        title: "Use this realm's loadout?",
        body: `${r.name} suggests a loadout: ${r.loadout.name || r.name}${bits.length ? " (" + bits.join(", with ") + ")" : ""}. ` +
          `Your current loadout (${st.active ? loadoutName(st.active) : "as you set it up"}) stays saved: one click puts it back.`,
        yes: "Use it", third: "Keep mine", no: "Cancel",
        fine: "Leyline installs only what it finds in its own mods list. Game files are never changed.",
      });
      if (a.pick === "no" || a.pick === "cancel") return { go: false };
      if (a.pick === "third") { try { localStorage.setItem("leyline.lo.keep." + from, "1"); } catch (_) { /* fine */ } return { go: true }; }
      await useShared({ name: r.loadout.name || r.name, items: r.loadout.items || [], gfx: r.loadout.gfx || "" }, from);
    } catch (_) { /* PLAY goes on */ }
    return { go: true };
  }

  // ------------------------------------------------------------------ hooks
  // app.js renderInstalled (the picked client changed, or something was installed): this box follows
  function renderInstalled(c) {
    if (!c) { st = null; render(); return; }
    load(c);
  }

  window.LeylineLoadouts = { load, renderInstalled, clientCard, switchTo, useShared, beforeJoin, templatesFor, TEMPLATES, status: () => st, COPY };
  // the box and the cards app.js drew before this file loaded
  setTimeout(() => { if ($("pane-installed") && picked()) load(picked()); else renderCards(); }, 0);
})();
