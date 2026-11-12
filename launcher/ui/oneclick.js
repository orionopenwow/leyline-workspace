// Leyline: single player in one click (owner 11 Nov: "do it all"). One flow for every button that plays this PC's own
// world: Home's "Play single player", Simple mode's Play, the play bar's Single player and Original world rows
// (app.js playWorld). It wraps window.LeylineWorlds.run (vanilla.js) with what a first-time player needs:
//
//   1. The game folder: world_find_folder. One picked: used without a word. Several: a one-tap chooser (name and path).
//      None: the folder picker opens by itself (pick_folder, then app.js's own add path), and the flow goes on once
//      the folder is in.
//   2. Room on the disk: world_preflight. Not enough: its message, and Retry once there's room.
//   3. The first entry: no characters in that world yet (local_characters) → a tiny Quick start sheet, a random race
//      and class (world_choices) and the suggested name already filled in: Play (world_play first + quick), Change,
//      or Skip (the game's own creation screen).
//   4. The setup card: the steps in plain words, a bar, the time left (from the progress rate), Hide (it keeps going:
//      a chip in the play bar brings it back), and the game opens by itself when it's done, hidden or not. On launch,
//      world_resume_pending: an interrupted setup offers Continue (and continues by itself when Play was pressed).
//   5. Errors: the core's "[code] sentence" with buttons for that code: Retry, Repair (world_repair), Pick folder,
//      Open log (world_logs, with Copy), "Stop the other one and play" for [ports], a hint for [antivirus].
//   6. After play: "Taking you in…", "Logged in with your Hero Key", or the login to type once with Copy.
// And Single player's setting "Stop the world when the game closes" (settings world_stop_on_exit, on by default).
//
// An older core without these commands: each step falls back to what Leyline did before (the page's own folder, no
// preflight, no sheet). Everything from the core is shown with textContent. Loaded after vanilla.js and app.js.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (typeof e === "string" ? e : fb || "Something went wrong. Try again."));
  const mk = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
  const btn = (label, cls, on, id) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (id) b.id = id; if (on) b.addEventListener("click", on); return b; };
  const byId = (id) => document.getElementById(id);
  const nav = (id) => { if (window.LeylineNav) window.LeylineNav.show(id); };
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { if (v === null) localStorage.removeItem(k); else localStorage.setItem(k, v); } catch (_) { /* private window */ } } };
  const W = () => window.LeylineWorlds || null;
  const BUILD = "3.3.5a";
  const PRESSED = "leyline.oneclick.pressed";   // the world whose setup Play started (a launch after an interruption goes on)
  const PICK_KEY = "leyline.single.folder";     // the folder the local worlds play with (single.js, vanilla.js)
  const cfg = { pollMs: 2000, minRateMs: 4000 };
  const titleOf = (kind) => (W() && W().WORLDS[kind] ? W().WORLDS[kind].title : "Single player");

  // ---------------------------------------------------------------- the core's answers
  const CODES = ["folder", "disk", "download", "maps", "db", "ports", "antivirus", "login", "other"];
  // "[code] One sentence." → { code, text }; anything else: null
  function coded(e) {
    const t = typeof e === "string" ? e : e && typeof e.message === "string" ? e.message : "";
    const m = /^\s*\[([a-z]+)\]\s*([\s\S]*)$/.exec(t);
    return m && CODES.includes(m[1]) ? { code: m[1], text: m[2].trim() } : null;
  }
  // a command this core may not have yet: { ok, v } or { ok: false } (then: what Leyline did before). A coded error
  // is a real answer and goes on to the error card.
  async function maybe(cmd, args) {
    try { return { ok: true, v: await call(cmd, args) }; } catch (e) { if (coded(e)) throw e; return { ok: false, e }; }
  }
  const REPAIRS = ["download", "maps", "db", "antivirus", "login", "other"];

  // ---------------------------------------------------------------- the steps' plain names
  const PHASES = ["Getting the engine", "Reading your maps", "Filling the world", "Starting"];
  function phaseOf(p) {
    const t = String((p && p.step) || "").toLowerCase();
    if (/map/.test(t)) return 1;
    if (/database|fill|world data|\bdb\b|login|account|quest|creature/.test(t)) return 2;
    if (/start|ready|sign|enter|open/.test(t)) return 3;
    if (/download|engine|unpack|getting|check/.test(t)) return 0;
    const f = frac(p);
    return Math.min(3, Math.floor(f * 4));
  }
  function frac(p) {
    if (!p) return 0;
    const of = Math.max(p.of || 7, 1);
    const n = Math.max(p.n || 1, 1);
    const at = typeof p.pct === "number" ? (n - 1) + p.pct / 100 : n - 0.5;
    return Math.min(1, Math.max(0, at / of));
  }
  // the time left from the rate so far: null until it has enough to go on
  function timeLeft() {
    const s = S.samples;
    if (s.length < 2) return null;
    const a = s[0], b = s[s.length - 1];
    const dt = b.t - a.t, df = b.f - a.f;
    if (dt < cfg.minRateMs || df <= 0) return null;
    return Math.max(0, (1 - b.f) / (df / dt) / 1000);
  }
  function leftText(secs) {
    if (secs === null) return "Working out the time left…";
    if (secs < 60) return "Less than a minute left";
    const m = Math.round(secs / 60);
    if (m < 90) return `About ${m} min left`;
    return `About ${Math.round(m / 60)} h left`;
  }

  // ---------------------------------------------------------------- state
  const S = {
    kind: "single",
    flow: null,         // the running flow's promise
    view: "",           // "", folder, folders, disk, quick, setup, error, other, resume, done
    hidden: false,      // the card hidden while the setup goes on (the chip shows)
    job: null,          // engine_progress while setting up
    samples: [],        // { t, f } for the time left
    err: null,          // { code, text, raw }
    log: null,          // world_logs text when open
    found: [],          // world_find_folder's found
    disk: null,         // world_preflight's answer when not ok
    quick: null,        // { choices, pick: { race, class, female, name }, edit }
    done: null,         // { text, kind, login }
    other: null,        // the other world running
    wait: null,         // { resolve, kind: "folder"|"choice"|"quick"|"answer" }
    opts: {},           // the caller's say / job / onOther / onDocker
  };
  const callerSay = (t, k) => { if (S.opts.say) S.opts.say(t, k); };

  // a step that waits for the player: the card shows `view`, and the promise resolves with their answer
  function ask(view) {
    S.view = view;
    S.hidden = false;
    paint();
    return new Promise((resolve) => { S.wait = { resolve, view }; });
  }
  function answer(v) { const w = S.wait; S.wait = null; if (w) w.resolve(v); }
  function close() { answer(null); S.view = ""; S.hidden = false; S.log = null; paint(); }

  // ---------------------------------------------------------------- 1. the game folder
  const appState = () => (window.LeylineApp && window.LeylineApp.state ? window.LeylineApp.state() : null);
  const folderIds = () => { const s = appState(); return s ? s.settings.clients.filter((c) => c.build === BUILD).map((c) => c.id) : []; };
  async function findFolder(kind) {
    const r = await maybe("world_find_folder", { kind });
    if (!r.ok) { const c = W().folder(); return c ? c.id : addFolder(); }   // an older core: the page's own pick, else ask
    const v = r.v && typeof r.v === "object" ? r.v : {};
    // the core may have just added the folders it found (added: true): the launcher's own list catches up
    if ((v.picked || (Array.isArray(v.found) && v.found.length)) && !folderIds().includes(v.picked || v.found[0].id) && window.LeylineApp && window.LeylineApp.refresh) {
      try { await window.LeylineApp.refresh(); } catch (_) { /* the id still plays */ }
    }
    if (v.picked) return v.picked;
    S.found = Array.isArray(v.found) ? v.found : [];
    if (S.found.length) return chooseFolder();
    return addFolder();
  }
  // several (or ones not added yet): one tap each
  async function chooseFolder() {
    const got = await ask("folders");
    if (!got) return null;
    if (got.add) return addFolder();
    if (got.added) return got.id;
    // found on this PC but not added yet: app.js's own add path, then on when it's in
    if (window.LeylineClientImport) window.LeylineClientImport.drop(got.path);
    return waitForFolder("Adding your game folder…");
  }
  // none: the folder picker opens by itself; the card says what it's for, with the other ways to add one
  async function addFolder() {
    const p = waitForFolder("");
    (async () => {
      let f = null;
      try { f = await call("pick_folder"); } catch (_) { f = null; }
      if (f && window.LeylineClientImport) { S.adding = true; paint(); window.LeylineClientImport.drop(f); }
    })();
    return p;
  }
  function waitForFolder(line) {
    S.before = new Set(folderIds());
    S.adding = !!line;
    return ask("folder");
  }
  // a folder of the version arrived (app.js refresh → leyline:state): the flow goes on with it
  document.addEventListener("leyline:state", () => {
    if (!S.wait || S.wait.view !== "folder") return;
    const fresh = folderIds().find((id) => !S.before.has(id));
    if (fresh) { store.set(PICK_KEY, fresh); S.adding = false; answer(fresh); }
  });

  // ---------------------------------------------------------------- 3. the first entry
  async function firstEntry(kind) {
    if (kind === "bossrush") return {};   // Boss Rush hands out its own ready heroes
    const lc = await maybe("local_characters");
    if (!lc.ok || !Array.isArray(lc.v)) return {};
    const w = lc.v.find((x) => x && (x.kind === kind || (W().WORLDS[kind] && x.kind === W().WORLDS[kind].serde)));
    if (!w || !Array.isArray(w.chars) || w.chars.length) return {};
    const ch = await maybe("world_choices", { kind });
    const races = ch.ok && ch.v && Array.isArray(ch.v.races) ? ch.v.races.filter((r) => r && Array.isArray(r.classes) && r.classes.length) : [];
    if (!races.length) return {};
    const r = races[Math.floor(Math.random() * races.length)];
    const c = r.classes[Math.floor(Math.random() * r.classes.length)];
    S.quick = { races, edit: false, pick: { race: r.id, class: c[0], female: Math.random() < 0.5, name: String(ch.v.name || "") } };
    const got = await ask("quick");
    if (!got) return null;          // closed: nothing is started
    if (got.skip) return {};
    const p = S.quick.pick;
    return { first: true, quick: { name: p.name.trim() || null, race: p.race, class: p.class, female: p.female } };
  }

  // ---------------------------------------------------------------- the flow
  // play(kind, opts): opts { say(text, kind), job(progress|null), onOther(e), onDocker(), stopOther, repair, client }
  // → the play's answer, or null (not played: the card or the caller's line says why). Never throws.
  function play(kind, opts) {
    kind = kind || "single";
    if (S.flow) { S.hidden = false; paint(); return S.flow; }
    S.kind = kind;
    S.opts = opts || {};
    S.flow = flow(kind, S.opts).finally(() => { S.flow = null; paintChip(); });
    return S.flow;
  }
  async function flow(kind, o) {
    const w = W();
    if (!w) { nav("single"); return null; }
    const title = titleOf(kind);
    S.err = null; S.log = null; S.done = null; S.other = null; S.disk = null;
    let setup = false;
    try {
      // 1. the folder
      let client = o.client || null;
      if (!client) client = await findFolder(kind);
      if (!client) { close(); callerSay(""); return null; }
      store.set(PICK_KEY, client);
      // where it stands
      const st = await w.status(kind);
      if (st && st.supported === false) throw { docker: true };
      const running = !!(st && st.running);
      // 2. room for it (only when it has work to do)
      if (!running || o.repair) {
        const pf = await maybe("world_preflight", { kind, client });
        if (pf.ok && pf.v && (pf.v.ok === false || (pf.v.ok !== true && pf.v.message))) {
          S.disk = pf.v;
          const again = await ask("disk");
          S.view = "";
          if (!again) { close(); return null; }
          S.flow = null;
          return play(kind, Object.assign({}, o, { client }));
        }
      }
      // 3. the first entry
      const first = o.pick ? {} : await firstEntry(kind);
      if (first === null) { close(); return null; }
      // 4. setup (the card) or a start (the caller's line)
      setup = o.repair || !(st && st.set_up);
      S.job = null;
      S.samples = [];
      if (setup) { store.set(PRESSED, kind); S.view = "setup"; paint(); }
      else if (S.view) { S.view = ""; paint(); }
      const r = await w.run(kind, {
        client, stopOther: !!o.stopOther, repair: !!o.repair, resume: !!o.resume, pick: o.pick || null,
        quick: first.quick || null, first: !!first.first, pollMs: cfg.pollMs,
        say: (t, k) => { callerSay(t, k); if (setup) paint(); },
        job: (p) => {
          S.job = p;
          if (p) { S.samples.push({ t: Date.now(), f: frac(p) }); if (S.samples.length > 60) S.samples.splice(1, 1); }
          if (o.job) o.job(p);
          paint();
        },
      });
      store.set(PRESSED, null);
      S.job = null;
      // 6. after play
      const [text, k] = w.played(r, title);
      const login = w.login ? w.login(r) : null;
      callerSay(text, k);
      S.done = { text, kind: k, login };
      if (login || (setup && !S.hidden)) {
        S.view = "done"; S.hidden = false; paint();
        if (!login) setTimeout(() => { if (S.view === "done" && !S.flow) close(); }, 6000);
      } else { S.view = ""; S.hidden = false; paint(); }
      return r;
    } catch (e) {
      S.job = null;
      store.set(PRESSED, null);
      if (e && e.other) {
        if (o.onOther) { close(); o.onOther(e); return null; }
        S.other = e.other;
        const go = await ask("other");
        S.view = "";
        if (!go) { close(); return null; }
        S.flow = null;
        return play(kind, Object.assign({}, o, { stopOther: true }));
      }
      if (e && e.docker) {
        close();
        if (o.onDocker) o.onDocker();
        else { callerSay(`${title} needs one more step on this PC: the Single player page walks you through it.`, "warn"); nav("single"); }
        return null;
      }
      if (e && e.folder) { close(); return null; }
      const c = coded(e);
      S.err = c ? { code: c.code, text: c.text || oops(e) } : { code: "", text: oops(e, `${title} didn't start. Try again.`) };
      callerSay(S.err.text, "bad");
      const next = await ask("error");
      S.view = "";
      if (!next) { close(); return null; }
      S.flow = null;
      if (next === "repair") return play(kind, Object.assign({}, o, { repair: true, stopOther: false }));
      if (next === "ports") { await stopOthers(kind); S.flow = null; return play(kind, Object.assign({}, o, { stopOther: true, repair: false })); }
      if (next === "folder") { const f = await pickAnother(kind); if (!f) { close(); return null; } S.flow = null; return play(kind, Object.assign({}, o, { client: f, repair: false })); }
      return play(kind, Object.assign({}, o, { repair: false }));
    }
  }
  // [ports]: the other world on these ports stopped (its characters are saved), the existing "Stop it and play" path
  async function stopOthers(kind) {
    const w = W();
    for (const k of Object.keys(w.WORLDS).filter((x) => x !== kind)) {
      const st = await w.status(k);
      if (st && st.running) { callerSay(`Stopping ${w.WORLDS[k].title}… Its characters are saved.`); try { await w.stopWorld(k); } catch (_) { /* the play says if it's still in the way */ } }
    }
  }
  // Pick folder after a [folder] error: the chooser (with "Another folder…"), else the picker
  async function pickAnother(kind) {
    const r = await maybe("world_find_folder", { kind }).catch(() => ({ ok: false }));
    S.found = r.ok && r.v && Array.isArray(r.v.found) ? r.v.found : [];
    if (!S.found.length) {
      const s = appState();
      S.found = s ? s.settings.clients.filter((c) => c.build === BUILD).map((c) => ({ id: c.id, name: c.name, path: c.path, build: c.build, added: true })) : [];
    }
    return S.found.length ? chooseFolder() : addFolder();
  }

  async function openLog() {
    S.log = "Reading the log…";
    paint();
    try { S.log = String((await call("world_logs", { kind: S.kind, lines: 200 })) || "(no log lines yet)"); }
    catch (e) { S.log = oops(e, "The log couldn't be read."); }
    paint();
    const pre = byId("oc-log");
    if (pre) pre.scrollTop = pre.scrollHeight;
  }
  async function copy(text, okLine) {
    try { await navigator.clipboard.writeText(text); flash(okLine); } catch (_) { flash("Select it and copy it with Ctrl+C."); }
  }
  function flash(t) { const f = byId("oc-flash"); if (f) f.textContent = t; }

  // ---------------------------------------------------------------- 5. on launch: an interrupted setup
  async function resume() {
    let k = null;
    try { k = await call("world_resume_pending"); } catch (_) { k = null; }
    if (!k || typeof k !== "string" || !W() || !W().WORLDS[k] || S.flow) return null;
    // world_setup {client: ""} goes on with the folder the setup started with (the folder found still plays it)
    if (store.get(PRESSED) === k) return play(k, { resume: true });   // Play was pressed: it goes on by itself
    S.kind = k;
    const go = await ask("resume");
    S.view = "";
    paint();
    if (!go) { store.set(PRESSED, null); return null; }
    return play(k, { resume: true });
  }

  // ---------------------------------------------------------------- the card
  const card = mk("div", "oc");
  card.id = "oc";
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-modal", "false");
  card.setAttribute("aria-labelledby", "oc-title");
  card.hidden = true;
  document.body.append(card);

  function head(text, sub) {
    const h = mk("div", "oc-head");
    const t = mk("h3", "oc-title", text);
    t.id = "oc-title";
    h.append(t);
    if (sub) h.append(mk("p", "oc-sub", sub));
    return h;
  }
  const row = (...b) => { const r = mk("div", "oc-row"); r.append(...b.filter(Boolean)); return r; };

  function setupNode(title) {
    const box = mk("div", "oc-body");
    const p = S.job;
    const at = p ? phaseOf(p) : 0;
    const ol = mk("ol", "oc-steps");
    ol.id = "oc-steps";
    PHASES.forEach((name, i) => {
      const li = mk("li", "oc-step " + (i < at ? "done" : i === at ? "now" : "todo"), name);
      li.dataset.state = i < at ? "done" : i === at ? "now" : "todo";
      ol.append(li);
    });
    const bar = mk("div", "oc-bar");
    bar.id = "oc-bar";
    bar.setAttribute("role", "progressbar");
    bar.setAttribute("aria-label", `Setting up ${title}`);
    const pct = Math.round(frac(p) * 100);
    bar.setAttribute("aria-valuemin", "0"); bar.setAttribute("aria-valuemax", "100"); bar.setAttribute("aria-valuenow", String(pct));
    const fill = mk("i");
    fill.style.width = `${p ? Math.max(pct, 2) : 2}%`;
    bar.append(fill);
    const left = mk("p", "oc-left", p ? `${pct}% · ${leftText(timeLeft())}` : "Getting ready…");
    left.id = "oc-left";
    box.append(ol, bar, left);
    if (p && p.detail) box.append(mk("p", "oc-fine", p.detail));
    box.append(mk("p", "oc-fine", "The first time takes 10 to 30 minutes. You can hide this: it keeps going, and the game opens by itself when it's ready."));
    box.append(row(btn("Hide", "btn small", () => { S.hidden = true; paint(); }, "oc-hide")));
    return box;
  }

  function errorNode() {
    const box = mk("div", "oc-body");
    const e = S.err || { code: "", text: "" };
    const p = mk("p", "oc-err", e.text);
    p.id = "oc-err";
    p.setAttribute("role", "alert");
    box.append(p);
    if (e.code === "antivirus") box.append(mk("p", "oc-fine", "Your antivirus may be blocking Leyline. Allow Leyline's folder in it, then press Repair."));
    if (e.code === "disk") box.append(mk("p", "oc-fine", "Free some space, then Retry."));
    const b = [btn("Retry", "btn small primary", () => answer("retry"), "oc-retry")];
    if (e.code === "ports") b.push(btn("Stop the other one and play", "btn small", () => answer("ports"), "oc-ports"));
    if (REPAIRS.includes(e.code)) b.push(btn("Repair", "btn small", () => answer("repair"), "oc-repair"));
    if (e.code === "folder" || e.code === "maps") b.push(btn(e.code === "maps" ? "Pick another folder" : "Pick folder", "btn small", () => answer("folder"), "oc-pick"));
    b.push(btn(S.log === null ? "Open log" : "Hide log", "btn small", () => { if (S.log === null) openLog(); else { S.log = null; paint(); } }, "oc-log-btn"));
    box.append(row(...b));
    if (S.log !== null) {
      const pre = mk("pre", "oc-log", S.log);
      pre.id = "oc-log";
      box.append(pre, row(btn("Copy", "btn small", () => copy(S.log, "Log copied."), "oc-log-copy"), mk("span", "oc-fine", "")));
      box.lastChild.lastChild.id = "oc-flash";
    }
    return box;
  }

  function quickNode() {
    const box = mk("div", "oc-body");
    const q = S.quick;
    const p = q.pick;
    const race = q.races.find((r) => r.id === p.race) || q.races[0];
    const cls = race.classes.find((c) => c[0] === p.class) || race.classes[0];
    if (!q.edit) {
      const who = mk("p", "oc-who");
      who.id = "oc-who";
      who.append(mk("b", "", p.name || "A new hero"), document.createTextNode(` · ${race.label} ${cls[1]} · ${p.female ? "Female" : "Male"}`));
      box.append(who);
    } else {
      const sel = (id, label, items, val, on) => {
        const s = mk("select", "oc-sel");
        s.id = id;
        s.setAttribute("aria-label", label);
        for (const [v, t] of items) { const o = mk("option", "", t); o.value = String(v); o.selected = String(v) === String(val); s.append(o); }
        s.addEventListener("change", () => on(s.value));
        return s;
      };
      const fix = () => { const r = q.races.find((x) => x.id === p.race) || q.races[0]; if (!r.classes.some((c) => c[0] === p.class)) p.class = r.classes[0][0]; };
      const name = mk("input", "oc-name");
      name.id = "oc-name";
      name.type = "text";
      name.maxLength = 12;
      name.placeholder = "Name";
      name.value = p.name;
      name.setAttribute("aria-label", "Name");
      name.addEventListener("input", () => { p.name = name.value; });
      box.append(row(
        sel("oc-race", "Race", q.races.map((r) => [r.id, r.label]), p.race, (v) => { p.race = Number(v); fix(); paint(); }),
        sel("oc-class", "Class", race.classes, p.class, (v) => { p.class = Number(v); }),
        sel("oc-sex", "Male or female", [["0", "Male"], ["1", "Female"]], p.female ? "1" : "0", (v) => { p.female = v === "1"; }),
        name));
    }
    box.append(row(
      btn("Play", "btn primary", () => answer({ play: true }), "oc-quick-go"),
      q.edit ? null : btn("Change", "btn small", () => { q.edit = true; paint(); }, "oc-quick-change"),
      btn("Skip", "btn small", () => answer({ skip: true }), "oc-quick-skip")));
    box.append(mk("p", "oc-fine", "A new level 1 character, made for you and taken straight in. Skip to make one on the game's own screen."));
    return box;
  }

  function foldersNode() {
    const box = mk("div", "oc-body");
    const list = mk("div", "oc-folders");
    list.setAttribute("role", "list");
    for (const f of S.found) {
      const b = btn("", "oc-folder", () => answer(f));
      b.setAttribute("role", "listitem");
      b.dataset.client = f.id || "";
      b.title = f.path || "";
      b.append(mk("b", "", f.name || "Game folder"), mk("span", "oc-path", f.path || ""));
      if (!f.added) b.append(mk("span", "oc-new", "Found on this PC"));
      list.append(b);
    }
    box.append(list, row(btn("Another folder…", "link-s", () => answer({ add: true }), "oc-folder-other")));
    return box;
  }

  function folderNode() {
    const box = mk("div", "oc-body");
    if (S.adding) { box.append(mk("p", "oc-line", "Adding your game folder…")); return box; }
    box.append(mk("p", "oc-line", "Leyline plays with your own game folder (it never downloads the game). Choose it once, and Leyline goes on from there."));
    const choose = btn("Choose your game folder…", "btn small primary", async () => {
      let f = null;
      try { f = await call("pick_folder"); } catch (e) { flash(oops(e, "The folder picker didn't open. Try the other ways.")); return; }
      if (f && window.LeylineClientImport) { S.adding = true; paint(); window.LeylineClientImport.drop(f); }
    }, "oc-folder-pick");
    const more = btn("Other ways to add it", "btn small", () => { if (typeof window.addGameFolder === "function") window.addGameFolder("", BUILD); else nav("clients"); }, "oc-folder-more");
    const fl = mk("span", "oc-fine");
    fl.id = "oc-flash";
    box.append(row(choose, more), fl);
    return box;
  }

  function doneNode() {
    const box = mk("div", "oc-body");
    const d = S.done || { text: "", kind: "ok" };
    const p = mk("p", "oc-line oc-done " + (d.kind || ""), d.login ? "" : d.text);
    p.id = "oc-done";
    if (d.login) {
      p.append(document.createTextNode("Log in once with "), mk("b", "", d.login.account), document.createTextNode(", password "), mk("code", "oc-pw", d.login.password), document.createTextNode(". Leyline made it for you and keeps it on this PC."));
      box.append(p, row(
        btn("Copy the name", "btn small", () => copy(d.login.account, "Name copied. Paste it on the login screen."), "oc-copy-name"),
        btn("Copy the password", "btn small primary", () => copy(d.login.password, "Password copied. Paste it on the login screen."), "oc-copy-pw")));
      const fl = mk("p", "oc-fine");
      fl.id = "oc-flash";
      box.append(fl);
    } else box.append(p);
    return box;
  }

  function paint() {
    const title = titleOf(S.kind);
    const v = S.view;
    if (!v || S.hidden) { card.hidden = true; card.replaceChildren(); paintChip(); return; }
    const x = btn("×", "oc-x", () => (v === "setup" ? (S.hidden = true, paint()) : close()), "oc-x");
    x.setAttribute("aria-label", v === "setup" ? "Hide" : "Close");
    const nodes = [];
    if (v === "setup") nodes.push(head(`Setting up ${title}`, "Once only. Then one click takes you in."), setupNode(title));
    else if (v === "error") nodes.push(head(`${title} didn't start`), errorNode());
    else if (v === "quick") nodes.push(head("Your first character", "Ready to go, or change anything first."), quickNode());
    else if (v === "folders") nodes.push(head("Which game folder?", `${title} plays with your own game folder. Tap one.`), foldersNode());
    else if (v === "folder") nodes.push(head("Your game folder"), folderNode());
    else if (v === "disk") {
      const b = mk("div", "oc-body");
      const raw = S.disk && S.disk.message ? String(S.disk.message) : "";
      const m = mk("p", "oc-err", (coded(raw) ? coded(raw).text : raw) || "There isn't enough free space on this PC.");
      m.id = "oc-disk";
      b.append(m);
      if (S.disk && S.disk.need_gb) b.append(mk("p", "oc-fine", `Needs about ${S.disk.need_gb} GB; ${Number(S.disk.free_gb || 0).toFixed(1)} GB is free.`));
      b.append(mk("p", "oc-line", "Free some space, then Retry."), row(btn("Retry", "btn small primary", () => answer(true), "oc-disk-retry")));
      nodes.push(head("Not enough room yet"), b);
    } else if (v === "other") {
      const b = mk("div", "oc-body");
      b.append(mk("p", "oc-line", `${S.other ? S.other.title : "Another world"} is running on this PC, and one world runs at a time. Stop it (its characters are saved) and play ${title}?`),
        row(btn(`Stop ${S.other ? S.other.title : "it"} and play`, "btn small primary", () => answer(true), "oc-other-go"), btn("Not now", "btn small", () => answer(false))));
      nodes.push(head("One world at a time"), b);
    } else if (v === "resume") {
      const b = mk("div", "oc-body");
      b.append(mk("p", "oc-line", `${title}'s setup was interrupted.`),
        row(btn("Continue", "btn small primary", () => answer(true), "oc-resume-go"), btn("Not now", "btn small", () => answer(false), "oc-resume-no")));
      nodes.push(head("Pick up where it stopped"), b);
    } else if (v === "done") nodes.push(head(title), doneNode());
    card.dataset.view = v;
    card.replaceChildren(x, ...nodes);
    card.hidden = false;
    paintChip();
  }

  // the chip in the play bar while the card is hidden (Simple mode: in the corner, the play bar is hidden there)
  function paintChip() {
    let chip = byId("oc-chip");
    const show = S.hidden && !!S.view;
    if (!show) { if (chip) chip.hidden = true; return; }
    if (!chip) {
      chip = btn("", "oc-chip", () => { S.hidden = false; paint(); }, "oc-chip");
      const bar = byId("play-form");
      (bar || document.body).append(chip);
    }
    const pct = S.job ? Math.round(frac(S.job) * 100) : 0;
    chip.textContent = S.view === "setup" ? `${titleOf(S.kind)}: setting up · ${pct}%` : `${titleOf(S.kind)}: needs you`;
    chip.title = "Show the setup";
    chip.hidden = false;
  }

  // ---------------------------------------------------------------- Home's button
  const home = byId("hm-oneclick");
  const homeGo = byId("hm-oneclick-go");
  if (homeGo) homeGo.addEventListener("click", () => play("single", { say: (t, k) => { const s = byId("hm-oneclick-status"); if (s) { s.textContent = t || ""; s.className = "status hm-oc-status" + (k ? " " + k : ""); } } }));
  async function paintHome() {
    if (!home || !W()) return;
    const st = await W().status("single");
    home.hidden = !(st && st.supported);
  }

  // ---------------------------------------------------------------- 8. Single player's setting
  const setRoot = byId("oc-set");
  const stopOnExit = () => { const s = appState(); const v = s && s.settings ? s.settings.world_stop_on_exit : undefined; return v === undefined ? true : !!v; };
  function paintSetting() {
    if (!setRoot) return;
    const h = mk("h3", "oc-set-h", "Single player settings");
    const lab = mk("label", "check oc-set-row");
    const box = mk("input");
    box.type = "checkbox";
    box.id = "oc-stop-exit";
    box.checked = stopOnExit();
    const line = mk("p", "status oc-set-status");
    line.id = "oc-set-status";
    box.addEventListener("change", async () => {
      const on = box.checked;
      box.disabled = true;
      try {
        await call("set_world_stop_on_exit", { on });
        const s = appState();
        if (s && s.settings) s.settings.world_stop_on_exit = on;
        line.textContent = on ? "Your world stops (and saves) when the game closes." : "Your world keeps running after the game closes.";
        line.className = "status oc-set-status ok";
      } catch (e) {
        box.checked = !on;
        line.textContent = oops(e, "That setting needs a newer Leyline. Update Leyline, then try again.");
        line.className = "status oc-set-status bad";
      }
      box.disabled = false;
    });
    lab.append(box, document.createTextNode("Stop the world when the game closes"));
    setRoot.replaceChildren(h, lab, mk("p", "oc-fine", "On: everyone is saved and the world stops when you close the game. Off: it keeps running, so friends can stay."), line);
  }

  document.addEventListener("leyline:page", (e) => {
    const pg = e.detail && e.detail.page;
    if (pg === "play") paintHome();
    if (pg === "single") paintSetting();
  });
  document.addEventListener("leyline:state", () => { if (setRoot && !setRoot.contains(document.activeElement)) paintSetting(); });
  paintSetting();
  setTimeout(() => { paintHome(); resume(); }, 300);

  window.LeylineOneClick = { play, resume, coded, phaseOf, PHASES, cfg, state: () => ({ view: S.view, hidden: S.hidden, busy: !!S.flow, kind: S.kind }) };
})();
