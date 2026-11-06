// Leyline: Single player, the flagship (owner 4 Nov plan, Phase 1: "a blatant, top-level choice"). Before, it was the
// play bar's "Solo leveling" row, set up through the Host page. Now it has its own page, a side menu item, a Home row
// and its own row in the realm picker (app.js LOCAL.solo; the ruleset id stays "solo", host/profiles.rs SOLO).
//
// Three steps, each with its state, and one progress line over them:
//   1) Your game folder: one of your added folders of the version it needs, or drop / choose one. The add paths are
//      app.js's own (window.LeylineClientImport.drop, the client list's Add a game folder…), never copies of them.
//   2) Engine: Built-in, recommended (Phase 2, docs/BUILTIN-ENGINE.md: the realm natively on this Windows PC, no
//      Docker; host/engine.rs, engine_status), or Docker, the fallback, checked live (docker_info: found / not running
//      / not installed, with Start Docker, Get Docker and Re-check; looked at again every 5 s while it is the pick).
//      The pick is kept (localStorage leyline.single.engine); where Built-in isn't made yet (not Windows), Docker.
//   3) Start. Built-in: engine_setup (the engine downloaded once, the maps read from the game folder, the database
//      filled, the account made with a generated password), its steps from engine_progress with the download's and
//      the maps' own %, then in (engine_play: the client at 127.0.0.1:3724, the login filled in). Docker: the Host pack
//      (downloaded once if missing), host_choose solo, host_start_quiet (private, no questions, no password typed),
//      its steps from host_progress, then in as the realm picker's row plays it (app.js playLocal: host_invite,
//      join_invite, PLAY). One PC runs one realm on Docker: My Den running there is switched on the Host page (it asks
//      first); anything else is said, with the Host page one click away.
// Once it is set up: the Manage card, for either engine (Play, Stop, Start, Logs; Built-in: Back up, the login with
// Copy, Update when the release has a newer engine; Docker: Settings, the Host page with it picked, Invite a friend).
// "Open to friends" on the Built-in engine (moving the world to the Host pack) is a follow-up (docs/BUILTIN-ENGINE.md).
//
// Loaded after app.js and den.js. app.js's kit, LOCAL and window.LeylineLocal are looked up when used.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (fb || "Something went wrong. Try again."));
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const btn = (label, cls, on) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (on) b.addEventListener("click", on); return b; };
  const byId = (id) => document.getElementById(id);
  const page = byId("page-single");
  const root = byId("sp");
  if (!page || !root) return;

  const PROFILE = "solo";
  const BUILD = "3.3.5a";
  const NAME = "Single player";
  // TODO(owner): the final marketing line, his words
  const TITLE = "Just you and your bot companions, on this PC: dungeons and raids with helper bots, levelling at your pace";
  // the sword mark (app.js MARKS.solo), drawn bigger
  const SWORD = '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M19.8 4.2 10.5 13.5l-1.35-1.35M19.8 4.2l-.6 3.15M19.8 4.2l-3.15.6M6.6 12.9l4.5 4.5M8.85 15.15 4.5 19.5"/></svg>';

  let dock = null;       // docker_info
  let es = null;         // engine_status (the Built-in engine)
  let dockBusy = false;
  let con = null;        // host_console, once it is set up here
  let job = null;        // host_progress or engine_progress (job.kind "host" / "engine")
  let jobTimer = null;
  let busy = false;
  let logs = null;       // the log lines shown under Manage (null: hidden)
  let line = ["", ""];   // the page's status line: [text, kind]

  const Local = () => window.LeylineLocal || null;
  const kitNow = () => { try { return typeof kit !== "undefined" ? kit : null; } catch (_) { return null; } };   // app.js
  const appState = () => (window.LeylineApp && window.LeylineApp.state ? window.LeylineApp.state() : null);
  const folders = () => { const s = appState(); return s ? s.settings.clients.filter((c) => c.build === BUILD) : []; };
  const kitHosted = () => { const k = kitNow(); return !!(k && k.realm && k.realm.profile === PROFILE); };
  const other = () => { const k = kitNow(); return k && k.realm && k.realm.profile !== PROFILE ? k.realm : null; };
  // the engine picked: Built-in where it is made for this PC, unless the player picked Docker
  const ENGINE_KEY = "leyline.single.engine";
  const builtinOk = () => !!(es && es.supported);
  const engine = () => (store.get(ENGINE_KEY) === "docker" || !builtinOk() ? "docker" : "builtin");
  // the engine Single player is set up on here (null: not yet): Built-in when it is set up and picked (or Docker isn't)
  const managed = () => {
    const b = !!(es && es.set_up);
    if (b && (engine() === "builtin" || !kitHosted())) return "builtin";
    return kitHosted() ? "docker" : null;
  };
  const hosted = () => !!managed();
  const isOn = () => page.classList.contains("on");
  // the folder its realm is made from (the maps are read from it): the one picked here, else the play bar's own
  // choice for it (app.js clientForRealm), else any of its version
  const PICK_KEY = "leyline.single.folder";
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* private window */ } } };
  const chosen = () => {
    const list = folders();
    const mine = list.find((c) => c.id === store.get(PICK_KEY));
    if (mine) return mine;
    const L = Local();
    const c = L && L.clientFor ? L.clientFor(PROFILE) : null;
    return c && c.build === BUILD ? c : list[0] || null;
  };
  const dockReady = () => !!(dock && dock.state === "ready");
  const running = () => (managed() === "builtin" ? !!(es && es.running)
    : !!(con && (con.services || []).some((x) => x.role === "world" && (x.state === "ok" || x.state === "starting"))));

  function say(text, kind) { line = [text || "", kind || ""]; const s = byId("sp-status"); if (s) { s.textContent = line[0]; s.className = "status sp-status" + (line[1] ? " " + line[1] : ""); } }
  function nav(id) { if (window.LeylineNav) window.LeylineNav.show(id); }

  // ---------------------------------------------------------------- the steps' states
  // "done" | "now" (the next thing to do) | "wait" (being checked) | "todo" (after an earlier step) | "run"
  function folderState() { return chosen() ? "done" : "now"; }
  function engineState() {
    if (engine() === "builtin") return es ? "done" : "wait";
    return dockReady() ? "done" : dock ? "now" : "wait";
  }
  function startState() {
    if (hosted()) return "done";
    if (job && job.running) return "run";
    return folderState() === "done" && engineState() === "done" ? "now" : "todo";
  }
  const CHIP = { done: "Done", now: "Next", wait: "Checking…", todo: "After the steps above", run: "Setting up…" };

  // ---------------------------------------------------------------- step 1: your game folder
  function folderBody() {
    const box = mk("div", "sp-body");
    const list = folders();
    const cur = chosen();
    if (list.length) {
      const g = mk("div", "sp-folders");
      g.setAttribute("role", "radiogroup");
      g.setAttribute("aria-label", "Your game folder");
      for (const c of list) {
        const on = !!cur && cur.id === c.id;
        const b = btn("", "sp-folder" + (on ? " on" : ""), () => { store.set(PICK_KEY, c.id); const L = Local(); if (L && L.useClient) L.useClient(PROFILE, c); paint(); });
        b.setAttribute("role", "radio");
        b.setAttribute("aria-checked", String(on));
        b.dataset.client = c.id;
        b.title = c.path;
        b.append(mk("b", "", c.name), mk("span", "rl-ver sm", c.build), mk("span", "sp-path", c.path));
        g.append(b);
      }
      box.append(g);
    } else {
      const p = mk("p", "sp-line", "Single player plays with your own game folder. Leyline never downloads the game. Needs ");
      p.append(mk("span", "rl-ver sm", BUILD));
      box.append(p);
    }
    // drop or choose: app.js's own add paths (clientImport, the client list's Add a game folder…)
    const drop = mk("div", "sp-drop");
    drop.id = "sp-drop";
    drop.append(mk("span", "", list.length ? "Another folder? Drop it here, or" : "Drop your game folder here, or"));
    const pick = btn("Choose a folder…", "btn small" + (list.length ? "" : " primary"), async () => {
      try { const f = await call("pick_folder"); if (f && window.LeylineClientImport) window.LeylineClientImport.drop(f); }
      catch (e) { say(oops(e), "bad"); }
    });
    pick.id = "sp-pick";
    const more = btn("More ways to add it ›", "link-s", () => { const L = Local(); if (L && L.addFolder) L.addFolder(PROFILE); });
    more.id = "sp-add";
    drop.append(pick, more);
    drop.addEventListener("dragover", (e) => { e.preventDefault(); if (e.dataTransfer) e.dataTransfer.dropEffect = "copy"; drop.classList.add("drag"); });
    drop.addEventListener("dragleave", (e) => { if (!drop.contains(e.relatedTarget)) drop.classList.remove("drag"); });
    drop.addEventListener("drop", (e) => {
      e.preventDefault();
      e.stopPropagation();
      drop.classList.remove("drag");
      const dt = e.dataTransfer;
      const t = dt ? ((dt.getData("text/uri-list") || "").trim() || (dt.getData("text/plain") || "").trim()) : "";
      if (t && window.LeylineClientImport) window.LeylineClientImport.drop(t);
    });
    box.append(drop);
    return box;
  }

  // ---------------------------------------------------------------- step 2: the engine
  function choose(which) {
    if (which === "builtin" && !builtinOk()) return;
    store.set(ENGINE_KEY, which);
    paint();
    watchDocker();
  }
  function engineCard(id, which, on, off) {
    const c = mk("div", "sp-engine" + (on ? " on" : "") + (off ? " off" : ""));
    c.id = id;
    c.setAttribute("role", "radio");
    c.setAttribute("aria-checked", String(on));
    if (off) c.setAttribute("aria-disabled", "true");
    else {
      c.tabIndex = 0;
      c.addEventListener("click", (e) => { if (!e.target.closest("button")) choose(which); });
      c.addEventListener("keydown", (e) => { if ((e.key === "Enter" || e.key === " ") && e.target === c) { e.preventDefault(); choose(which); } });
    }
    return c;
  }
  function engineBody() {
    const box = mk("div", "sp-body");
    const g = mk("div", "sp-engines");
    g.setAttribute("role", "radiogroup");
    g.setAttribute("aria-label", "Engine");
    const cur = engine();
    // Built-in: the realm inside Leyline, nothing else to install (host/engine.rs)
    const lite = engineCard("sp-engine-builtin", "builtin", cur === "builtin", !!es && !builtinOk());
    const lh = mk("span", "sp-engine-h");
    lh.append(mk("b", "", "Built-in"), mk("em", "", !es ? "Checking…" : builtinOk() ? "Recommended" : "Windows only for now"));
    if (builtinOk()) {
      const ch = mk("span", "sp-dock " + (es.installed ? "ok" : "wait"), es.installed ? "Ready" : "Downloads at Start");
      ch.id = "sp-builtin-chip";
      lh.append(ch);
    }
    // TODO(owner): the words
    lite.append(lh, mk("span", "sp-engine-l", !es || builtinOk()
      ? "Runs inside Leyline with nothing else to install. Downloaded once, the first time you press Start."
      : "Runs inside Leyline on Windows PCs. On this PC, Docker runs Single player."));
    // Docker: checked live
    const dk = engineCard("sp-engine-docker", "docker", cur === "docker", false);
    const dh = mk("span", "sp-engine-h");
    const [kind, text] = dockerLine();
    const chip = mk("span", "sp-dock " + kind, text);
    chip.id = "sp-dock";
    dh.append(mk("b", "", "Docker"), chip);
    dk.append(dh, mk("span", "sp-engine-l", builtinOk() ? "Free. The other way: runs the realm in Docker on this PC, and can open it to friends." : "Free. Runs the realm in the background on this PC."));
    if (cur === "docker") {
      if (dock && !dockReady() && dock.help) { const h = mk("span", "sp-engine-l sp-dock-help", dock.help); dk.append(h); }
      const acts = mk("div", "sp-row");
      if (dock && dock.state === "missing") {
        const get = btn("Get Docker", "btn small primary", () => call("open_link", { name: String(dock.get_url || "").includes("engine") ? "dockerengine" : "docker" }).catch((e) => say(oops(e), "bad")));
        get.id = "sp-dock-get";
        get.title = dock.get_url || "docker.com";
        acts.append(get);
      }
      if (dock && dock.can_start && !dockReady() && dock.state !== "starting") {
        const s = btn("Start Docker", "btn small primary", (ev) => startDocker(ev.currentTarget));
        s.id = "sp-dock-start";
        acts.append(s);
      }
      if (!dockReady()) {
        const r = btn(dockBusy ? "Checking…" : "Re-check", "btn small", () => checkDocker(true));
        r.id = "sp-dock-check";
        r.disabled = dockBusy;
        acts.append(r);
      }
      if (acts.childElementCount) dk.append(acts);
    }
    g.append(lite, dk);
    box.append(g);
    return box;
  }
  function dockerLine() {
    if (!dock) return ["wait", "Checking…"];
    switch (dock.state) {
      case "ready": return ["ok", "Found: running"];
      case "missing": return ["bad", "Not installed"];
      case "stopped": case "desktop_stopped": return ["warn", "Not running"];
      case "starting": return ["wait", "Starting…"];
      default: return ["warn", "Needs a fix"];
    }
  }
  async function checkDocker(paintNow) {
    if (dockBusy) return;
    dockBusy = true;
    if (paintNow) paint();
    let d = null;
    try { d = await call("docker_info"); } catch (_) { d = null; }
    dockBusy = false;
    if (d && typeof d === "object") dock = d;
    else if (!dock) dock = { state: "missing", help: "", get_url: "", can_start: false };
    paint();
    watchDocker();
  }
  // looked at again every 5 s while this page shows and Docker isn't ready (someone opens Docker Desktop and comes back)
  let dockTimer = null;
  function watchDocker() {
    if (dockTimer) { clearTimeout(dockTimer); dockTimer = null; }
    if (isOn() && engine() === "docker" && !dockReady() && !hosted() && document.visibilityState !== "hidden") dockTimer = setTimeout(() => checkDocker(false), 5000);
  }
  async function startDocker(b) {
    if (b) b.disabled = true;
    try { say(await call("docker_start") || "Starting Docker… This usually takes under a minute."); }
    catch (e) { say(oops(e, "Docker did not start. Open Docker Desktop yourself: this page sees it."), "warn"); }
    dock = Object.assign({}, dock, { state: "starting", can_start: false });
    paint();
    for (let i = 0; i < 40 && isOn(); i++) {
      await new Promise((r) => setTimeout(r, 3000));
      try { const d = await call("docker_info"); if (d) dock = d; } catch (_) { /* again */ }
      if (dockReady()) break;
    }
    say(dockReady() ? "Docker is running." : "Docker hasn't answered yet. Look at Docker Desktop's window, then press Re-check.", dockReady() ? "ok" : "warn");
    paint();
  }

  // ---------------------------------------------------------------- step 3: Start
  function startBody() {
    const box = mk("div", "sp-body");
    if (job && !job.idle && (job.running || job.code !== 0)) box.append(progress());
    // the log under it (a start that stopped: Logs), as the Manage card shows it
    if (logs !== null) { const pre = mk("pre", "sp-logs", logs); pre.id = "sp-logs"; box.append(pre); }
    if (job && job.running) return box;
    const built = engine() === "builtin";
    const o = built ? null : other();
    const ready = startState() === "now";
    const go = btn(busy ? "Starting…" : "Start Single player", "btn primary", start);
    go.id = "sp-start";
    go.disabled = busy || !ready;
    const row = mk("div", "sp-row");
    row.append(go);
    box.append(row);
    let hint;
    if (!ready) hint = folderState() !== "done" ? "Add your game folder first (step 1)." : built ? "The engine first (step 2)." : "Docker first (step 2).";
    else if (built) hint = "Sets it up once, then takes you in. The first start gets the engine, reads the maps from your game folder and fills the world: 10 to 30 minutes.";
    else if (o && o.profile === "den") hint = `Your PC runs ${o.name || "My Den"} now, and one PC runs one realm at a time. Start asks, then switches it: your Den and characters are kept.`;
    else if (o) hint = `Your PC hosts ${o.name || "a realm"} now, and one PC runs one realm at a time. Stop it on the Host page first.`;
    else hint = "Sets it up once, then takes you in. The first start reads the maps from your game folder and takes 5 to 20 minutes.";
    const h = mk("p", "sp-line", hint);
    h.id = "sp-start-hint";
    box.append(h);
    if (o && o.profile !== "den") row.append(btn("Open the Host page", "btn small", () => nav("host")));
    return box;
  }
  function fmt(secs) { const m = Math.floor(secs / 60), s = secs % 60; return m ? `${m} min ${String(s).padStart(2, "0")} s` : `${s} s`; }
  function progress() {
    const failed = !job.running && job.code !== 0;
    const box = mk("div", "sp-prog" + (failed ? " bad" : ""));
    box.id = "sp-prog";
    const head = mk("div", "sp-prog-h");
    head.append(mk("b", "", failed ? "Setting up stopped" : job.step || "Getting ready"),
      mk("span", "sp-fine", job.running ? `Step ${Math.max(1, job.n)} of ${job.of} · ${fmt(job.secs || 0)}` : fmt(job.secs || 0)));
    const bar = mk("div", "sp-bar");
    const fill = mk("i", "");
    // within a step, its own % when it knows one (the download, the maps)
    const at = job.running ? (typeof job.pct === "number" ? Math.max(job.n - 1, 0) + job.pct / 100 : Math.max(job.n - 0.5, 0.3)) : job.of;
    fill.style.width = `${Math.round(at / Math.max(job.of, 1) * 100)}%`;
    bar.append(fill);
    bar.setAttribute("role", "progressbar");
    bar.setAttribute("aria-valuemin", "0"); bar.setAttribute("aria-valuemax", String(job.of)); bar.setAttribute("aria-valuenow", String(job.n));
    box.append(head, bar);
    if (job.running && (job.detail || typeof job.pct === "number")) {
      const d = mk("p", "sp-fine sp-prog-d", [typeof job.pct === "number" ? `${job.pct}%` : "", job.detail || ""].filter(Boolean).join(" · "));
      d.id = "sp-prog-d";
      box.append(d);
    }
    if (job.running) box.append(mk("p", "sp-fine", "The first start takes 5 to 20 minutes. You can use the rest of Leyline meanwhile: it plays by itself when it's ready."));
    if (failed) {
      box.append(mk("p", "sp-line", job.problem || "Something went wrong."));
      const r = mk("div", "sp-row");
      r.append(btn("Try again", "btn small primary", () => { const k = job.kind; job = null; if (k === "engine" && managed() === "builtin") startAgain(null); else start(); }), btn("Logs", "btn small", () => showLogs(true)));
      box.append(r);
    }
    return box;
  }
  const ENGINE_JOB = () => ({ idle: false, kind: "engine", running: true, code: null, step: "Getting ready", n: 0, of: 7, secs: 0, problem: null, pct: null, detail: "" });
  async function start() {
    if (engine() === "builtin") { startBuiltin(); return; }
    const o = other();
    // My Den runs here: the Host page asks, then switches the same realm (hostconsole.js switchTo)
    if (o && o.profile === "den") {
      nav("host");
      document.dispatchEvent(new CustomEvent("leyline:host-ruleset", { detail: { profile: PROFILE, switch: true } }));
      return;
    }
    if (o) { say(`This PC hosts ${o.name || "a realm"} now. Stop it on the Host page, then press Start again.`, "warn"); return; }
    const c = chosen();
    if (!c) { say("Add your game folder first.", "warn"); return; }
    busy = true;
    paint();
    try {
      let k = kitNow();
      if (!k || !k.is_kit) {
        say("Getting the Host pack… (a one-time download)");
        const r = await call("hostpack_install");
        await call("set_host_kit", { path: r[1] });
        if (Local() && Local().readKit) await Local().readKit();
      }
      say("Starting Single player…");
      await call("host_choose", { profile: PROFILE, clientId: c.id });
      await call("host_start_quiet", { answers: { name: NAME, visibility: "private", gm: null } });
      job = { idle: false, kind: "host", running: true, code: null, step: "Getting ready", n: 0, of: 8, secs: 0, problem: null };
      say("");
      follow(true);
    } catch (e) { say(oops(e, "Single player didn't start. Try again."), "bad"); }
    busy = false;
    paint();
  }
  // the Built-in engine: whatever isn't done yet (the engine, the maps, the database, the account), then in
  async function startBuiltin(update) {
    const c = chosen();
    if (!c) { say("Add your game folder first.", "warn"); return; }
    busy = true;
    paint();
    try {
      say(update ? "Getting the new engine…" : "Starting Single player…");
      await call("engine_setup", update ? { client: c.id, update: true } : { client: c.id });
      job = ENGINE_JOB();
      say("");
      follow(true);
    } catch (e) { say(oops(e, "Single player didn't start. Try again."), "bad"); }
    busy = false;
    paint();
  }
  async function readEngine() {
    try { const s = await call("engine_status"); if (s && typeof s === "object") es = s; } catch (_) { if (!es) es = { supported: false }; }
  }
  async function follow(thenPlay) {
    if (jobTimer) clearTimeout(jobTimer);
    const kind = job && job.kind === "engine" ? "engine" : "host";
    try { const p = await call(kind === "engine" ? "engine_progress" : "host_progress"); if (p) job = Object.assign({ kind }, p); } catch (_) { /* again */ }
    paint();
    if (job && job.running) { jobTimer = setTimeout(() => follow(thenPlay), 2000); return; }
    jobTimer = null;
    if (job && !job.idle && job.code === 0) {
      job = null;
      if (kind === "engine") await readEngine();
      else if (Local() && Local().readKit) await Local().readKit();
      await readConsole();
      say(thenPlay ? "Single player is ready. Opening it…" : "Single player is running.", "ok");
      paint();
      if (thenPlay) play();
    }
  }

  // ---------------------------------------------------------------- Manage (set up on this PC)
  async function readConsole() {
    if (!kitHosted()) { con = null; return; }
    try { con = await call("host_console"); } catch (_) { con = null; }
  }
  function play() {
    if (managed() === "builtin") { playBuiltin(); return; }
    const L = Local();
    if (L && L.play) L.play(PROFILE);
  }
  // Play on the Built-in engine: started first when it is stopped (then in by itself), else the game at 127.0.0.1:3724
  async function playBuiltin() {
    const c = chosen();
    if (!c) { say("Add your game folder first.", "warn"); return; }
    await readEngine();
    if (!(es && es.running)) {
      if (job && job.running) return;
      try { await call("engine_start"); job = ENGINE_JOB(); say(""); paint(); follow(true); } catch (e) { say(oops(e, "Single player didn't start. Try again."), "bad"); }
      return;
    }
    try { const r = await call("engine_play", { client: c.id }); say((r && r.message) || "Started.", "ok"); }
    catch (e) { say(oops(e, "The game didn't start."), "bad"); }
  }
  async function stop(b) {
    b.disabled = true;
    say("Stopping… Your characters are saved.");
    const built = managed() === "builtin";
    try { say(await call(built ? "engine_stop" : "host_stop"), "ok"); } catch (e) { say(oops(e), "bad"); }
    if (built) await readEngine(); else await readConsole();
    paint();
  }
  async function startAgain(b) {
    if (b) b.disabled = true;
    say("Starting Single player…");
    if (managed() === "builtin") {
      try { await call("engine_start"); job = ENGINE_JOB(); say(""); paint(); follow(false); }
      catch (e) { say(oops(e, "Single player didn't start. Try again."), "bad"); if (b) b.disabled = false; }
      return;
    }
    try {
      await call("host_start_quiet", { answers: { name: null, visibility: "private", gm: null } });
      job = { idle: false, kind: "host", running: true, code: null, step: "Getting ready", n: 0, of: 8, secs: 0, problem: null };
      say("");
      follow(false);
    } catch (e) { say(oops(e, "Single player didn't start. Try again."), "bad"); if (b) b.disabled = false; }
  }
  async function backUp(b) {
    b.disabled = true;
    say("Backing up your characters…");
    try { const at = await call("engine_backup"); say(`Backed up: ${at}`, "ok"); await readEngine(); }
    catch (e) { say(oops(e, "The back-up didn't work. Try again."), "bad"); }
    paint();
  }
  async function copyLogin() {
    try { await navigator.clipboard.writeText(es.password); say("Password copied. Paste it on the login screen.", "ok"); }
    catch (_) { say(`Your password: ${es.password}`); }
  }
  async function showLogs(force) {
    if (logs !== null && !force) { logs = null; paint(); return; }
    logs = "Reading the log…";
    paint();
    // the engine's logs when it is the one set up, or the pick while it is being set up
    const built = managed() ? managed() === "builtin" : engine() === "builtin";
    try { logs = (await call(built ? "engine_logs" : "host_logs", { lines: 200 })) || "(no log lines yet)"; } catch (e) { logs = oops(e); }
    paint();
    const pre = byId("sp-logs");
    if (pre) pre.scrollTop = pre.scrollHeight;
  }
  function settings() {
    nav("host");
    document.dispatchEvent(new CustomEvent("leyline:host-ruleset", { detail: { profile: PROFILE } }));
  }
  function manageCard() {
    const built = managed() === "builtin";
    const card = mk("section", "sp-manage");
    card.id = "sp-manage";
    card.dataset.engine = built ? "builtin" : "docker";
    card.setAttribute("aria-label", "Manage Single player");
    const head = mk("div", "sp-manage-h");
    const up = running();
    const known = built ? !!es : !!con;
    const starting = !!(job && job.running);
    const chip = mk("span", "sp-dock " + (!known ? "wait" : up ? "ok" : starting ? "wait" : "warn"), !known ? "Checking…" : up ? "Running" : starting ? "Starting…" : "Stopped");
    chip.id = "sp-run";
    head.append(mk("b", "", "Set up on this PC"), chip, mk("span", "sp-fine", built ? "Built-in engine" : "Docker"));
    const acts = mk("div", "sp-row");
    const p = btn("Play", "btn primary", play); p.id = "sp-play";
    const s = btn("Stop", "btn small", (ev) => stop(ev.currentTarget)); s.id = "sp-stop"; s.disabled = (known && !up) || starting;
    const st = btn("Start", "btn small", (ev) => startAgain(ev.currentTarget)); st.id = "sp-start-again"; st.disabled = (known && up) || starting;
    const lg = btn(logs === null ? "Logs" : "Hide logs", "btn small", () => showLogs(false)); lg.id = "sp-logs-btn";
    acts.append(p, s, st, lg);
    if (built) {
      const bk = btn("Back up", "btn small", (ev) => backUp(ev.currentTarget)); bk.id = "sp-backup";
      bk.title = "A copy of your characters and their bots, kept in Leyline's folder";
      acts.append(bk);
      if (es.update) {
        const u = btn("Update the engine", "btn small", () => startBuiltin(true)); u.id = "sp-update";
        u.disabled = starting;
        acts.append(u);
      }
    } else {
      const se = btn("Settings", "btn small", settings); se.id = "sp-settings";
      se.title = "The Host page, with Single player picked";
      acts.append(se);
      if (window.LeylineSolo && window.LeylineSolo.inviteButton) acts.append(window.LeylineSolo.inviteButton((t, k) => say(t, k)));
    }
    card.append(head, acts, mk("p", "sp-line", "Play starts it if it's stopped, then takes you in. Stop saves everyone and shuts it down."));
    if (built && es.account) {
      // the login made for them (host/engine.rs: generated, kept on this PC): the name is filled in on the login screen
      const lr = mk("p", "sp-line sp-login");
      lr.id = "sp-login";
      lr.append(mk("span", "", "Your login: "), mk("b", "", es.account), mk("span", "", " · password "), mk("code", "", es.password), mk("span", "", " "));
      const cp = btn("Copy", "link-s", copyLogin); cp.id = "sp-login-copy";
      lr.append(cp);
      card.append(lr);
    }
    if (window.LeylineSingleUpdate) card.append(window.LeylineSingleUpdate.node((t, k) => say(t, k)));   // the bots (singleupdate.js)
    if (job && !job.idle && (job.running || job.code !== 0)) card.append(progress());
    if (logs !== null) { const pre = mk("pre", "sp-logs", logs); pre.id = "sp-logs"; card.append(pre); }
    return card;
  }

  // ---------------------------------------------------------------- the page
  function step(n, id, title, state, body) {
    const li = mk("li", "sp-step " + state);
    li.id = id;
    li.dataset.state = state;
    const h = mk("div", "sp-step-h");
    const num = mk("span", "sp-num", state === "done" ? "✓" : String(n));
    num.setAttribute("aria-hidden", "true");
    const chip = mk("span", "sp-chip " + state, CHIP[state]);
    h.append(num, mk("b", "", title), chip);
    li.append(h);
    if (state !== "todo" || n === 3) li.append(body());
    return li;
  }
  function paint() {
    const head = mk("div", "sp-head");
    const ic = mk("span", "sp-mark");
    ic.innerHTML = SWORD;
    const ht = mk("div", "sp-head-t");
    ht.append(mk("h2", "", NAME), mk("p", "lede sp-title", TITLE));
    head.append(ic, ht);
    const nodes = [head];
    if (hosted()) nodes.push(manageCard());
    else {
      const states = [folderState(), engineState(), startState()];
      const done = states.filter((s) => s === "done").length;
      const prog = mk("div", "sp-progress");
      prog.id = "sp-progress";
      const at = states.findIndex((s) => s !== "done");
      prog.append(mk("span", "sp-progress-t", at < 0 ? "All set" : `Step ${at + 1} of 3 · ${done} done`));
      const bar = mk("div", "sp-bar");
      bar.setAttribute("role", "progressbar");
      bar.setAttribute("aria-label", "Single player setup");
      bar.setAttribute("aria-valuemin", "0"); bar.setAttribute("aria-valuemax", "3"); bar.setAttribute("aria-valuenow", String(done));
      const fill = mk("i", "");
      fill.style.width = `${Math.round(done / 3 * 100)}%`;
      bar.append(fill);
      prog.append(bar);
      const ol = mk("ol", "sp-steps");
      ol.append(step(1, "sp-step-folder", "Your game folder", states[0], folderBody),
        step(2, "sp-step-engine", "Engine", states[1], engineBody),
        step(3, "sp-step-start", "Start", states[2], startBody));
      nodes.push(prog, ol);
    }
    const s = mk("p", "status sp-status" + (line[1] ? " " + line[1] : ""), line[0]);
    s.id = "sp-status";
    s.setAttribute("role", "status");
    nodes.push(s);
    root.replaceChildren(...nodes);
    paintHome();
  }

  // Home's Single player row (index.html #hm-single): Play once it's set up here, else this page
  function paintHome() {
    const b = byId("hm-single-go");
    if (b) b.textContent = hosted() ? "Play" : "Start";
  }
  const homeGo = byId("hm-single-go");
  if (homeGo) homeGo.addEventListener("click", () => { if (hosted()) play(); else nav("single"); });

  async function load() {
    paint();
    const L = Local();
    if (L && L.readKit) await L.readKit();
    await Promise.all([checkDocker(false), readConsole(), readEngine()]);
    paint();
    // a start still running (begun here, or on the Host page for Docker): followed here too
    if (!jobTimer) {
      try { const p = await call("engine_progress"); if (p && p.running) { job = Object.assign({}, p, { kind: "engine" }); follow(false); return; } } catch (_) { /* none */ }
      try { const p = await call("host_progress"); if (p && p.running && p.kind === "host") { job = p; follow(false); } } catch (_) { /* none */ }
    }
  }
  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "single") load(); else { watchDocker(); paintHome(); } });
  document.addEventListener("leyline:state", () => { if (isOn()) paint(); });
  document.addEventListener("leyline:kit", () => { if (isOn()) readConsole().then(paint); else paintHome(); });
  document.addEventListener("visibilitychange", watchDocker);
  // app.js: the realm picker's Single player row plays the Built-in engine straight away once it is set up
  window.LeylineSingle = { load, paint, start, play, NAME, TITLE, builtin: () => managed() === "builtin" };
  if (isOn()) load(); else { paint(); readEngine().then(paintHome); }
})();
