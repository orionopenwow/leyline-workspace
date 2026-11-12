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
  const SETTINGS_KINDS = ["single", "vanilla"];   // the worlds with companions (host/spsettings.rs supports)
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
  // owner 12 Nov: "more descriptive of what filling the world means". The core's own step names (host/engine.rs
  // Step::label) say the same; each gets one line on what it is.
  const STEPS = [
    { t: "Getting the engine (one-time download)", sub: "The small game server that runs your world. Downloaded once, kept on this PC." },
    { t: "Reading the maps from your game folder (one time)", sub: "The land, water and buildings, so everything knows where it can walk. Read once, kept." },
    { t: "Filling the world: placing creatures, quests and items into its database (one time)", sub: "Tens of thousands of creatures, quests, items, vendors and spells, placed once." },
    { t: "Creating your local login", sub: "A login for this PC only. Leyline fills it in for you." },
    { t: "Starting your world", sub: "Switching the server on: the only step from now on, a few seconds." },
  ];
  const PHASES = STEPS.map((s) => s.t);
  const FIRST_LINE = "First time only: 5–20 minutes. After this, your world starts in seconds.";
  function phaseOf(p) {
    const t = String((p && p.step) || "").toLowerCase();
    if (/map/.test(t)) return 1;
    if (/login|account/.test(t)) return 3;
    if (/database|fill|world data|\bdb\b|quest|creature/.test(t)) return 2;
    if (/start|ready|sign|enter|open/.test(t)) return 4;
    if (/download|engine|unpack|getting|check/.test(t)) return 0;
    const f = frac(p);
    return Math.min(4, Math.floor(f * 5));
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
    world: null,        // the "Your world" panel: { kind, v: world_settings_get, pre: world_preflight, saving, line }
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
      let pre = null;
      if (!running || o.repair) {
        const pf = await maybe("world_preflight", { kind, client });
        if (pf.ok && pf.v && typeof pf.v === "object") pre = pf.v;
        if (pf.ok && pf.v && (pf.v.ok === false || (pf.v.ok !== true && pf.v.message))) {
          S.disk = pf.v;
          const again = await ask("disk");
          S.view = "";
          if (!again) { close(); return null; }
          S.flow = null;
          return play(kind, Object.assign({}, o, { client }));
        }
      }
      // 2b. Your world (owner 12 Nov): what happens and its settings, before anything is downloaded; the first time
      // only (a world not set up whose settings weren't chosen). An older core without world_settings_get: skipped.
      if (!(st && st.set_up) && !o.repair && !o.resume && SETTINGS_KINDS.includes(kind)) {
        const g = await maybe("world_settings_get", { kind });
        if (g.ok && g.v && typeof g.v === "object" && !g.v.chosen) {
          S.world = { kind, v: Object.assign({}, g.v), pre, saving: false, line: "" };
          const ok = await ask("world");
          S.world = null;
          if (!ok) { close(); callerSay(""); return null; }
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

  // The panel on its own, for a start that doesn't go through play() (single.js's Start): true to go on (shown and
  // saved, or not needed), false when the player cancelled.
  async function askWorld(kind, client) {
    if (S.flow || S.wait || !SETTINGS_KINDS.includes(kind)) return true;
    const st = W() ? await W().status(kind) : null;
    if (st && st.set_up) return true;
    const g = await maybe("world_settings_get", { kind }).catch(() => ({ ok: false }));
    if (!g.ok || !g.v || typeof g.v !== "object" || g.v.chosen) return true;
    const pf = client ? await maybe("world_preflight", { kind, client }).catch(() => ({ ok: false })) : { ok: false };
    S.kind = kind;
    S.world = { kind, v: Object.assign({}, g.v), pre: pf.ok && pf.v && typeof pf.v === "object" ? pf.v : null, saving: false, line: "" };
    const ok = await ask("world");
    S.world = null;
    S.view = "";
    paint();
    return !!ok;
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
    STEPS.forEach((s, i) => {
      const li = mk("li", "oc-step " + (i < at ? "done" : i === at ? "now" : "todo"));
      li.dataset.state = i < at ? "done" : i === at ? "now" : "todo";
      li.append(mk("span", "oc-step-t", s.t), mk("span", "oc-step-s", s.sub));
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
    const fl = mk("p", "oc-line oc-first", FIRST_LINE);
    fl.id = "oc-first";
    box.append(fl, mk("p", "oc-fine", "You can hide this: it keeps going, and the game opens by itself when it's ready."));
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

  // ---------------------------------------------------------------- Your world: the settings (panel and page)
  const COMPANIONS = [[0, "Off"], [1, "1"], [2, "2"], [3, "3"], [4, "4"]];
  const POPULATION = [[0, "Quiet"], [20, "Some"], [60, "Lively"], [150, "Busy"]];
  const XP = [[1, "1x"], [1.5, "1.5x"], [2, "2x"]];
  // one row of choices (a radio group of buttons): [value, label]
  function choices(id, label, items, cur, on) {
    const g = mk("div", "ws-choices");
    g.id = id;
    g.setAttribute("role", "radiogroup");
    g.setAttribute("aria-label", label);
    const list = items.slice();
    if (!list.some(([v]) => v === cur)) list.push([cur, `Custom (${cur})`]);
    for (const [v, t] of list) {
      const b = btn(t, "ws-choice" + (v === cur ? " on" : ""), () => on(v));
      b.setAttribute("role", "radio");
      b.setAttribute("aria-checked", String(v === cur));
      b.dataset.v = String(v);
      g.append(b);
    }
    return g;
  }
  function setting(title, ctl, line) {
    const r = mk("div", "ws-row");
    r.append(mk("b", "ws-name", title), ctl, mk("p", "oc-fine ws-line", line));
    return r;
  }
  // the four settings; on(key, value) is called with the world_settings_set key(s) changed
  function settingsForm(v, idp, on) {
    const box = mk("div", "ws-form");
    box.id = `${idp}-form`;
    const follow = v.companions_auto ? Math.max(1, Number(v.companions_follow) || 0) : 0;
    box.append(setting("Companions that travel with you",
      choices(`${idp}-follow`, "Companions that travel with you", COMPANIONS, follow, (n) => on({ companions_auto: n > 0, companions_follow: n })),
      "Bot companions that fight beside you. They join when you enter the world; change them any time in the Bots window."));
    const dl = mk("label", "check ws-check");
    const dc = mk("input");
    dc.type = "checkbox";
    dc.id = `${idp}-dungeon`;
    dc.checked = !!v.dungeon_auto;
    dc.addEventListener("change", () => on({ dungeon_auto: dc.checked }));
    dl.append(dc, document.createTextNode(dc.checked ? "On" : "Off"));
    box.append(setting("Dungeon buddies", dl, "Bots fill your group only when you enter a dungeon, and head home when you leave."));
    box.append(setting("World population",
      choices(`${idp}-bots`, "World population", POPULATION, Number(v.roaming_bots) || 0, (n) => on({ roaming_bots: n })),
      "Other adventurers (bots) roaming the world: Quiet has none, Busy about 150. More feels alive and uses more of your PC."));
    if (typeof v.xp_rate === "number") {
      box.append(setting("XP rate",
        choices(`${idp}-xp`, "XP rate", XP, v.xp_rate, (n) => on({ xp_rate: n })),
        "How fast you level: kills, quests and exploring give this much experience."));
    }
    return box;
  }
  const payload = (kind, v) => ({ kind, companions_auto: !!v.companions_auto, companions_follow: Number(v.companions_follow) || 0,
    dungeon_auto: !!v.dungeon_auto, roaming_bots: Number(v.roaming_bots) || 0, xp_rate: v.xp_rate });

  // How a friend joins: the same lines in the panel and on the page (TODO(owner): his words)
  const FRIEND_LINES = [
    "Your friend needs Leyline and their own game folder, the same version as yours.",
    "Press Invite a friend while your world runs, and send them the link. They open it, and Leyline takes them in.",
    "Their character stays on your world: next time they press Join again, with no new link.",
    "On the same network they connect straight to your PC; from anywhere else, through the Leyline relay.",
    "In the game, the Social panel's Leyline tab invites a friend without leaving the world.",
  ];
  function friendLines() {
    const ul = mk("ul", "ws-friend-lines");
    for (const l of FRIEND_LINES) ul.append(mk("li", "", l));
    return ul;
  }

  function worldNode() {
    const w = S.world;
    const box = mk("div", "oc-body ws");
    // what happens
    const what = mk("section", "ws-what");
    what.id = "ws-what";
    what.setAttribute("aria-label", "What happens");
    what.append(mk("h4", "ws-h", "What happens"), mk("p", "oc-line", "Single player runs a small game server on your PC, made only for you. The first time, Leyline:"));
    const mb = w.pre && Number(w.pre.download_mb) > 0 ? Number(w.pre.download_mb) : 0;
    // a core that tells the size says 0 once the engine is here (world_preflight download_mb)
    const installed = !!w.pre && w.pre.download_mb === 0;
    const engineLine = mb ? `Gets the engine: a one-time download of about ${mb >= 1024 ? (mb / 1024).toFixed(1) + " GB" : mb + " MB"}.`
      : installed ? "Gets the engine: it's already on this PC." : "Gets the engine: a one-time download.";
    const ol = mk("ol", "ws-steps");
    ol.id = "ws-steps";
    for (const l of [engineLine,
      "Reads the maps from your own game folder: the land, water and buildings, so everything knows where it can walk. Read once, kept.",
      "Fills the world: every creature, quest, item, vendor and spell placed into your world's own database, tens of thousands of entries. Done once.",
      "Creates your local login, for this PC only.",
      "Starts your world and takes you in."]) ol.append(mk("li", "", l));
    const why = mk("p", "oc-line ws-why", "Why the wait: the first time takes 5–20 minutes because of the reading and filling above. Everything is kept on your PC, so from then on a start only switches the server on: seconds.");
    why.id = "ws-why";
    what.append(ol, why);
    // the settings
    const set = mk("section", "ws-set");
    set.setAttribute("aria-label", "Your settings");
    set.append(mk("h4", "ws-h", "Your settings"),
      settingsForm(w.v, "ws", (ch) => { Object.assign(w.v, ch); paint(); }),
      mk("p", "oc-fine", "You can change all of these later in Single player settings."));
    // a friend
    const fr = mk("section", "ws-friend");
    fr.id = "ws-friend";
    fr.setAttribute("aria-label", "Play with a friend");
    fr.append(mk("h4", "ws-h", "Play with a friend"), friendLines());
    box.append(what, set, fr);
    if (w.line) { const e = mk("p", "oc-err", w.line); e.id = "ws-err"; e.setAttribute("role", "alert"); box.append(e); }
    const go = btn(w.saving ? "Saving…" : "Set up my world", "btn primary", saveWorld, "ws-go");
    go.disabled = !!w.saving;
    box.append(row(go, btn("Cancel", "btn small", () => answer(false), "ws-cancel")));
    return box;
  }
  async function saveWorld() {
    const w = S.world;
    if (!w || w.saving) return;
    w.saving = true; w.line = ""; paint();
    try { await call("world_settings_set", payload(w.kind, w.v)); answer(true); }
    catch (e) { w.saving = false; w.line = coded(e) ? coded(e).text : oops(e, "Your settings weren't saved. Try again."); paint(); }
  }

  function paint() {
    const title = titleOf(S.kind);
    const v = S.view;
    if (!v || S.hidden) { card.hidden = true; card.replaceChildren(); paintChip(); return; }
    const x = btn("×", "oc-x", () => (v === "setup" ? (S.hidden = true, paint()) : close()), "oc-x");
    x.setAttribute("aria-label", v === "setup" ? "Hide" : "Close");
    const nodes = [];
    if (v === "setup") nodes.push(head(`Setting up ${title}`, "Once only. Then one click takes you in."), setupNode(title));
    else if (v === "world") nodes.push(head(S.world && S.world.kind === "vanilla" ? "Your original world" : "Your world", "Set it up once. Here's what happens, and how you want it."), worldNode());
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

  // ---------------------------------------------------------------- 8. Single player's settings
  const setRoot = byId("oc-set");
  const stopOnExit = () => { const s = appState(); const v = s && s.settings ? s.settings.world_stop_on_exit : undefined; return v === undefined ? true : !!v; };
  // the world settings on the page: { kind, v, line: [text, kind], restart, busy, missing }
  const PS = { kind: "single", v: null, line: ["", ""], restart: false, busy: false, missing: false, loading: false };
  async function loadWorldSettings() {
    if (PS.loading) return;
    PS.loading = true;
    const g = await maybe("world_settings_get", { kind: PS.kind }).catch((e) => ({ ok: true, v: null, e }));
    // the Built-in engine reads these; where it doesn't run (Docker plays Single player there) they'd change nothing
    try { const st = W() ? await W().status(PS.kind) : null; PS.unsupported = !!st && st.supported === false; } catch (_) { PS.unsupported = false; }
    PS.loading = false;
    PS.missing = !g.ok;
    PS.v = g.ok && g.v && typeof g.v === "object" ? g.v : null;
    // a refusal is said once (no reading again in a loop); picking the world again or the page again retries
    PS.failed = !!g.e;
    if (g.e) PS.line = [coded(g.e) ? coded(g.e).text : oops(g.e, "Your world's settings didn't load."), "bad"];
    paintSetting();
  }
  async function changeWorld(ch) {
    if (!PS.v || PS.busy) return;
    const before = Object.assign({}, PS.v);
    Object.assign(PS.v, ch);
    PS.busy = true;
    paintSetting();
    try {
      const r = await call("world_settings_set", Object.assign({ kind: PS.kind }, ch));
      if (r && r.settings) PS.v = Object.assign({}, PS.v, r.settings);
      PS.restart = !!(r && r.restart);
      PS.line = [(r && r.message) || "Saved.", "ok"];
    } catch (e) {
      PS.v = before;
      PS.line = [coded(e) ? coded(e).text : oops(e, "That setting wasn't saved. Try again."), "bad"];
    }
    PS.busy = false;
    paintSetting();
  }
  async function restartWorld() {
    const w = W();
    PS.busy = true;
    PS.line = ["Restarting… Everyone is saved first.", ""];
    paintSetting();
    try {
      if (w) await w.stopWorld(PS.kind);
      await call("world_start", { kind: PS.kind });
      PS.restart = false;
      PS.line = ["Restarting with your settings. It's ready in a few seconds.", "ok"];
    } catch (e) { PS.line = [oops(e, "It didn't restart. Press Play to start it."), "bad"]; }
    PS.busy = false;
    paintSetting();
  }
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
    const nodes = [h, lab, mk("p", "oc-fine", "On: everyone is saved and the world stops when you close the game. Off: it keeps running, so friends can stay."), line];
    // the world's own settings (host/spsettings.rs), for later changes
    if (!PS.missing && PS.unsupported) nodes.push(mk("p", "oc-fine", "Your world's companion and population settings are for the Built-in engine, on Windows PCs."));
    if (!PS.missing && !PS.unsupported) {
      const ws = mk("section", "ws-page");
      ws.id = "wss";
      ws.setAttribute("aria-label", "Your world's settings");
      const top = mk("div", "ws-page-h");
      const sel = mk("select", "oc-sel");
      sel.id = "wss-kind";
      sel.setAttribute("aria-label", "Which world");
      for (const k of SETTINGS_KINDS) { const o = mk("option", "", titleOf(k)); o.value = k; o.selected = k === PS.kind; sel.append(o); }
      sel.addEventListener("change", () => { PS.kind = sel.value; PS.v = null; PS.failed = false; PS.restart = false; PS.line = ["", ""]; paintSetting(); loadWorldSettings(); });
      top.append(mk("b", "ws-h", "Your world"), sel);
      ws.append(top);
      if (!PS.v && !PS.failed) ws.append(mk("p", "oc-fine", "Reading your world's settings…"));
      else if (PS.v) {
        const f = settingsForm(PS.v, "wss", changeWorld);
        if (PS.busy) f.querySelectorAll("button, input").forEach((x) => { x.disabled = true; });
        ws.append(f);
        ws.append(mk("p", "oc-fine", PS.v.running ? "Your world is running: changes apply at its next start." : "Changes apply when your world starts."));
      }
      const st = mk("p", "status ws-status" + (PS.line[1] ? " " + PS.line[1] : ""), PS.line[0]);
      st.id = "wss-status";
      st.setAttribute("role", "status");
      ws.append(st);
      if (PS.restart) {
        const rb = btn("Restart now", "btn small primary", restartWorld, "wss-restart");
        rb.disabled = PS.busy;
        ws.append(row(rb, mk("span", "oc-fine", "Saves everyone, then starts it again with your settings.")));
      }
      nodes.push(ws);
    }
    setRoot.replaceChildren(...nodes);
    if (!PS.v && !PS.missing && !PS.loading && !PS.failed) loadWorldSettings();
  }

  // ---------------------------------------------------------------- 9. Play with a friend (the Single player page)
  // One instant link from the running world (app.rs world_invite: same network straight in, else the Leyline relay),
  // Copy, how a friend joins, and whether they're in (den_net_state: the world's tunnel and its streams). The full
  // checklist (Ready for friends?) stays in hostfriends.js, one button away.
  const pfRoot = byId("pf");
  const PF = { kind: null, link: "", ways: [], line: ["", ""], busy: false, net: null, timer: null };
  async function runningWorld() {
    const w = W();
    if (!w) return null;
    for (const k of SETTINGS_KINDS) { const st = await w.status(k); if (st && st.running) return k; }
    return null;
  }
  function friendState() {
    const n = PF.net;
    if (PF.link && n && n.running && n.connected && Number(n.streams) > 0) return "connected";
    if (PF.link) return "invited";
    return "none";
  }
  async function readFriends() {
    const was = friendState();
    PF.kind = await runningWorld();
    try { PF.net = await call("den_net_state"); } catch (_) { PF.net = null; }
    if (!PF.kind && PF.link) { PF.link = ""; PF.ways = []; }
    paintFriends();
    // a friend came in (or left): who keeps a character here, read again
    if (friendState() !== was && window.LeylineWorldFriends) window.LeylineWorldFriends.refresh();
  }
  function watchFriends() {
    if (PF.timer) { clearTimeout(PF.timer); PF.timer = null; }
    const on = byId("page-single");
    if (on && on.classList.contains("on") && document.visibilityState !== "hidden") PF.timer = setTimeout(() => readFriends().then(watchFriends), 5000);
  }
  async function inviteFriend() {
    if (PF.busy) return;
    PF.busy = true;
    PF.line = ["Making your invite link…", ""];
    paintFriends();
    PF.kind = await runningWorld();
    if (!PF.kind) {
      PF.busy = false;
      PF.line = ["Start your world first (Play): friends join it while it runs.", "warn"];
      paintFriends();
      return;
    }
    try {
      const r = await call("world_invite", { kind: PF.kind });
      PF.link = String((r && r.link) || "");
      PF.ways = Array.isArray(r && r.ways) ? r.ways : [];
      PF.line = PF.link ? ["Your link is ready. Copy it and send it to your friend.", "ok"] : ["No link came back. Try again.", "bad"];
      if (PF.link) { try { await navigator.clipboard.writeText(PF.link); PF.line = ["Link copied. Send it to your friend.", "ok"]; } catch (_) { /* the Copy button */ } }
    } catch (e) { PF.line = [oops(e, "The invite didn't work. Try again."), "bad"]; }
    try { PF.net = await call("den_net_state"); } catch (_) { /* as it was */ }
    PF.busy = false;
    paintFriends();
    if (window.LeylineWorldFriends) window.LeylineWorldFriends.refresh();
  }
  const STATE_TEXT = { none: "No friend invited yet", invited: "Invited: waiting for your friend", connected: "Your friend is in" };
  function paintFriends() {
    if (!pfRoot) return;
    const h = mk("div", "pf-head");
    const fs = friendState();
    const chip = mk("span", "sp-dock pf-state " + (fs === "connected" ? "ok" : fs === "invited" ? "wait" : "warn"),
      fs === "connected" && PF.net && Number(PF.net.streams) > 1 ? `${STATE_TEXT.connected} (${PF.net.streams} connections)` : STATE_TEXT[fs]);
    chip.id = "pf-status";
    chip.dataset.state = fs;
    h.append(mk("h3", "oc-set-h", "Invite a friend to your world"), chip);
    const nodes = [h, friendLines()];
    const go = btn(PF.busy ? "Making the link…" : PF.link ? "New link" : "Invite a friend", "btn primary", inviteFriend, "pf-invite");
    go.disabled = PF.busy;
    const acts = [go];
    if (window.LeylineHostFriends && window.LeylineHostFriends.open) acts.push(btn("Check it works for friends", "btn small", () => window.LeylineHostFriends.open({ kind: PF.kind || "single" }), "pf-more"));
    nodes.push(row(...acts));
    if (!PF.kind && !PF.link) nodes.push(mk("p", "oc-fine", "Your world runs only while you play: start it, then invite."));
    if (PF.link) {
      const inp = mk("input", "pf-link");
      inp.id = "pf-link";
      inp.type = "text";
      inp.readOnly = true;
      inp.value = PF.link;
      inp.setAttribute("aria-label", "Your invite link");
      inp.addEventListener("focus", () => inp.select());
      nodes.push(row(inp, btn("Copy", "btn small", () => copyTo(PF.link, "Link copied. Send it to your friend."), "pf-copy")));
      if (PF.ways.length) {
        const ul = mk("ul", "pf-ways");
        ul.id = "pf-ways";
        for (const w of PF.ways) if (w && w.id !== "own") { const li = mk("li", w.ok ? "ok" : "off", String(w.line || "")); ul.append(li); }
        nodes.push(ul);
      }
    }
    const st = mk("p", "status pf-line" + (PF.line[1] ? " " + PF.line[1] : ""), PF.line[0]);
    st.id = "pf-line";
    st.setAttribute("role", "status");
    nodes.push(st);
    pfRoot.replaceChildren(...nodes);
  }
  async function copyTo(text, okLine) {
    try { await navigator.clipboard.writeText(text); PF.line = [okLine, "ok"]; } catch (_) { PF.line = ["Select the link and copy it with Ctrl+C.", "warn"]; }
    paintFriends();
  }

  document.addEventListener("leyline:page", (e) => {
    const pg = e.detail && e.detail.page;
    if (pg === "play") paintHome();
    if (pg === "single") { PS.v = null; PS.failed = false; paintSetting(); readFriends().then(watchFriends); }
  });
  document.addEventListener("visibilitychange", watchFriends);
  document.addEventListener("leyline:state", () => { if (setRoot && !setRoot.contains(document.activeElement)) paintSetting(); });
  paintSetting();
  paintFriends();
  setTimeout(() => { paintHome(); resume(); }, 300);

  window.LeylineOneClick = { play, resume, askWorld, coded, phaseOf, PHASES, STEPS, FIRST_LINE, cfg, friends: () => ({ state: friendState(), link: PF.link, kind: PF.kind }), state: () => ({ view: S.view, hidden: S.hidden, busy: !!S.flow, kind: S.kind }) };
})();
