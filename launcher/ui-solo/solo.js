// Leyline Solo (docs/LEYLINE-SOLO.md): the first-era game, single player, on the player's own game folder. A world
// of your own on this PC (the core's classic_* commands), companions, offline, signed in by itself, friends by invite.
//
// Screens: First run (the game folder, then Set up with one progress bar), Home (Play, Quick start, Running/Stopped,
// Stop), Characters, Invite a friend, Settings. No Hero Key, no website, no realm list, no Den, no mods.
// Every core call is caught: a build whose core has no classic_* commands yet says so in plain words, never breaks.
"use strict";
(() => {
  const invoke = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline Solo app."; };
  const call = (cmd, args) => invoke(cmd, args);
  const text = (e) => (typeof e === "string" ? e : e && typeof e.message === "string" ? e.message : e && typeof e.error === "string" ? e.error : "");
  // Tauri's answer for a command this core doesn't have
  const notFound = (e) => /command .* not found|not found|unknown command|not allowed/i.test(text(e)) && !/folder|file|character|account/i.test(text(e));
  const portsBusy = (e) => (e && typeof e === "object" && (e.ports_busy || e.other)) || /\bports?\b.*\b(busy|in use|taken)\b|already (running|in use)|another world/i.test(text(e));
  const mk = (tag, cls, t) => { const e = document.createElement(tag); if (cls) e.className = cls; if (t !== undefined) e.textContent = t; return e; };
  const btn = (label, cls, on, id) => { const b = mk("button", cls || "btn", label); b.type = "button"; if (id) b.id = id; if (on) b.addEventListener("click", on); return b; };
  const byId = (id) => document.getElementById(id);
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* private window */ } } };

  const FIRST_ERA = "1.12.1";      // the build field of a first-era game folder (clients.rs Build); never shown
  const KIND = "classic";          // the progress line's kind for this world (engine_progress)
  const POLL = Number(window.__SOLO_POLL_MS) || 1500;
  const NAME_RE = /^[A-Za-z]{2,12}$/;
  // TODO(owner): the app's words in your own voice (the first-run line, Home's line)
  const INTRO = "Your own world of the first-era game, on this PC: you, your companions, and friends you invite. No server, no account to make, works offline.";
  const ESTIMATE = "The first time takes about 20 to 50 minutes: the world's own files download (about 90 MB), the maps are read from your game folder, and the world is filled. You can leave it running. After that it starts in a minute or two.";
  const LEGAL = "Leyline is an independent fan project, not affiliated with or endorsed by Blizzard Entertainment.";

  const S = {
    app: null,            // get_state
    st: null,             // classic_status (null: not read, or no core)
    noCore: false,        // the core has no classic_* commands
    page: "first",
    firstStep: "folder",
    job: null,            // the setup/start progress line
    busy: false,
    ask: null,            // { other, then }: another world on these ports
    choices: null,        // classic_choices
    quickOpen: false,
    pick: { race: Number(store.get("solo.race")) || 1, class: Number(store.get("solo.class")) || 1, female: store.get("solo.female") === "1", name: "" },
    chars: null,          // classic_characters
    charsErr: "",
    invite: null,         // classic_invite
    password: null,       // classic_password, only after "Show password"
    say: ["", ""],        // the page's own line
    folderSay: ["", ""],
    update: null,
  };
  const prefs = {
    get signin() { return store.get("solo.signin") === "quick" ? "quick" : "simple"; },   // Simple until the owner approves Quick
    set signin(v) { store.set("solo.signin", v === "quick" ? "quick" : "simple"); },
    get roaming() { return store.get("solo.roaming") === "1"; },
    set roaming(v) { store.set("solo.roaming", v ? "1" : "0"); },
  };

  // ------------------------------------------------------------------------------------------ the core
  function folder() {
    const list = S.app && S.app.settings && Array.isArray(S.app.settings.clients) ? S.app.settings.clients.filter((c) => c.build === FIRST_ERA) : [];
    return list.find((c) => c.id === store.get("solo.folder")) || list[0] || null;
  }
  async function readApp() {
    try { S.app = await call("get_state"); } catch (_) { S.app = S.app || null; }
  }
  async function readStatus() {
    try {
      const st = await call("classic_status");
      S.st = st && typeof st === "object" ? st : null;
      S.noCore = false;
    } catch (e) {
      if (notFound(e)) S.noCore = true;
      S.st = null;
    }
    return S.st;
  }
  const running = () => !!(S.st && S.st.running);
  const setUp = () => !!(S.st && S.st.set_up);
  // the world's progress line: classic_status's own, else the shared engine_progress line when it is ours
  async function progress() {
    let p = null;
    try { p = await call("engine_progress"); } catch (_) { p = null; }
    if (p && p.kind && p.kind !== KIND) p = null;
    if (!p) {
      const st = await readStatus();
      if (st && st.progress && typeof st.progress === "object") p = st.progress;
      else if (st && st.busy) p = { running: true, step: "Working", n: 1, of: 1 };
    }
    return p;
  }
  // follow a job to its end; throws its problem
  async function follow(label) {
    for (let i = 0; ; i++) {
      await sleep(POLL);
      const p = await progress();
      if (p && p.running) { S.job = Object.assign({ label }, p); paint(); continue; }
      S.job = null;
      if (p && !p.idle && typeof p.code === "number" && p.code !== 0) throw p.problem || `${label} didn't finish.`;
      if (p && p.problem && p.ok === false) throw p.problem;
      // the job may not have shown its line yet: one more look before calling it done
      if (i === 0 && !(p && typeof p.code === "number")) continue;
      break;
    }
    await readStatus();
  }
  const NO_CORE = "This build of Leyline Solo doesn't have its world yet. Update Leyline Solo (Settings, About), then try again.";

  // ------------------------------------------------------------------------------------------ the line
  function say(t, kind) {
    S.say = [t || "", kind || ""];
    const s = byId("so-status");
    if (s) { s.textContent = S.say[0]; s.className = "so-status" + (S.say[1] ? " " + S.say[1] : ""); }
  }
  const oops = (e, fb) => {
    const t = text(e);
    if (notFound(e)) return NO_CORE;
    return t || fb || "Something went wrong. Try again.";
  };

  // ------------------------------------------------------------------------------------------ actions
  async function useFolder(path) {
    path = String(path || "").trim();
    S.folderSay = ["", ""];
    if (!path) { S.folderSay = ["Choose your game folder first.", "warn"]; paint(); return false; }
    let d;
    try { d = await call("detect_client", { path }); } catch (e) {
      S.folderSay = [`That folder isn't a game folder Leyline Solo can read: ${oops(e, "no game found there")}. Pick the folder that holds the game's program.`, "bad"];
      paint();
      return false;
    }
    const build = d && (d.build || (d.profile && d.profile.build));
    if (build !== FIRST_ERA) {
      S.folderSay = ["That folder holds a later version of the game. Leyline Solo plays the first-era game only (the original, before any expansion): pick that game's folder.", "bad"];
      paint();
      return false;
    }
    if (d.complete === false) {
      S.folderSay = ["That game folder looks incomplete" + (Array.isArray(d.missing) && d.missing.length ? ` (missing ${d.missing.slice(0, 3).join(", ")})` : "") + ". Repair or copy the game again, then pick it.", "bad"];
      paint();
      return false;
    }
    let profile = null;
    try { profile = (await call("add_client", { path })).profile; } catch (e) {
      // already in the list: use that one
      await readApp();
      const list = (S.app && S.app.settings && S.app.settings.clients) || [];
      profile = list.find((c) => String(c.path).replace(/[\\/]+$/, "") === path.replace(/[\\/]+$/, "")) || null;
      if (!profile) { S.folderSay = [oops(e, "That folder couldn't be added."), "bad"]; paint(); return false; }
    }
    store.set("solo.folder", profile.id);
    await readApp();
    S.folderSay = ["Your first-era game folder is ready.", "ok"];
    return true;
  }
  async function chooseFolder() {
    let path = null;
    try { path = await call("pick_folder"); } catch (e) { S.folderSay = [oops(e), "bad"]; paint(); return false; }
    if (!path) return false;
    const input = byId("so-folder-path");
    if (input) input.value = path;
    return useFolder(path);
  }

  async function setup() {
    if (S.busy) return;
    const c = folder();
    if (!c) { S.firstStep = "folder"; paint(); return; }
    S.busy = true; S.job = { label: "Setting up", step: "Starting", n: 1, of: 1, running: true };
    say("Setting up your world (once)…");
    paint();
    try {
      await call("classic_setup", { client: c.id });
      await follow("Setting up");
      if (setUp()) { S.page = "home"; say("Set up. Press Play.", "ok"); }
      else say("Set up didn't finish. Press Set up again: it carries on from where it stopped.", "warn");
    } catch (e) {
      S.job = null;
      say(oops(e, "Set up didn't finish. Press Set up again."), "bad");
    }
    S.busy = false;
    paint();
  }

  // Play: start the world when it isn't running, then the game, signed in by itself. o: { quick, pick, stopOther }
  async function play(o) {
    o = o || {};
    if (S.busy) return;
    const c = folder();
    if (!c) { S.page = "first"; S.firstStep = "folder"; say("Point Leyline Solo at your game folder first.", "warn"); paint(); return; }
    S.ask = null; S.busy = true;
    paint();
    try {
      await readStatus();
      if (S.noCore) throw NO_CORE;
      if (!setUp()) { S.busy = false; S.page = "first"; S.firstStep = "setup"; say("Set it up first (once).", "warn"); paint(); return; }
      if (!running()) {
        say("Starting your world…");
        S.job = { label: "Starting", step: "Starting", n: 1, of: 1, running: true };
        paint();
        await call("classic_start", { roaming: prefs.roaming, stopOther: !!o.stopOther });
        await follow("Starting");
      }
      say(o.quick ? "Making your character and taking you in…" : "Taking you in…");
      const args = { client: c.id, signin: prefs.signin };
      if (o.pick) args.pick = o.pick;
      if (o.quick) args.quick = o.quick;
      if (o.stopOther) args.stopOther = true;
      const r = await call("classic_play", args);
      if (r && r.auto === false && r.account) say(`Started. At the login screen type the password for ${r.account} (Settings, Show password).`, "warn");
      else say(r && r.made ? `Started: making ${r.made} and taking you in.` : (r && r.message) || "Started. The game opens and signs you in.", "ok");
      if (o.quick) { S.choices = null; S.pick.name = ""; }
    } catch (e) {
      S.job = null;
      if (portsBusy(e) && !o.stopOther) {
        S.ask = { other: (e && typeof e === "object" && (e.other_title || e.other)) || "", then: o };
        say("");
      } else say(oops(e, "Your world didn't start. Press Play again."), "bad");
    }
    S.busy = false;
    await readStatus();
    paint();
  }

  async function stop() {
    if (S.busy) return;
    S.busy = true; paint();
    try { const r = await call("classic_stop"); say(typeof r === "string" && r ? r : "Stopped. Your characters are saved.", "ok"); } catch (e) { say(oops(e, "It didn't stop. Try again."), "bad"); }
    S.busy = false;
    await readStatus();
    paint();
  }

  async function readChoices() {
    if (S.choices) return;
    try { S.choices = await call("classic_choices"); } catch (e) { S.choices = null; say(oops(e, "The races and classes couldn't be read."), "bad"); }
    if (S.choices && S.choices.name && !S.pick.name) S.pick.name = S.choices.name;
    fixPick();
    paint();
  }
  function fixPick() {
    const races = S.choices && Array.isArray(S.choices.races) ? S.choices.races : [];
    if (!races.length) return;
    const r = races.find((x) => x.id === S.pick.race) || races[0];
    S.pick.race = r.id;
    if (!r.classes.some((c) => c[0] === S.pick.class)) S.pick.class = r.classes[0][0];
  }
  function quickStart() {
    const name = String(S.pick.name || "").trim();
    if (!NAME_RE.test(name)) {
      say("A name is 2 to 12 letters, no spaces, numbers or marks.", "bad");
      const n = byId("so-name"); if (n) { n.setAttribute("aria-invalid", "true"); n.focus(); }
      return;
    }
    play({ quick: { name: name[0].toUpperCase() + name.slice(1).toLowerCase(), race: S.pick.race, class: S.pick.class, female: S.pick.female } });
  }

  async function readChars() {
    S.charsErr = "";
    try {
      const r = await call("classic_characters");
      S.chars = Array.isArray(r) ? r : r && Array.isArray(r.chars) ? r.chars : [];
    } catch (e) { S.chars = null; S.charsErr = oops(e, "Your characters couldn't be read."); }
    paint();
  }

  async function makeInvite() {
    if (S.busy) return;
    S.busy = true; S.invite = null; paint();
    try {
      const r = await call("classic_invite");
      S.invite = typeof r === "string" ? { link: r } : r;
      if (!S.invite || !S.invite.link) throw "The invite link couldn't be made. Try again.";
      say("Your invite link is ready.", "ok");
    } catch (e) { S.invite = null; say(oops(e, "The invite link couldn't be made. Try again."), "bad"); }
    S.busy = false;
    paint();
  }
  async function copy(t, ok) {
    try { await navigator.clipboard.writeText(t); say(ok, "ok"); } catch (_) { say("Copy didn't work here: select the text and copy it yourself.", "warn"); }
  }

  async function showPassword() {
    try {
      const r = await call("classic_password");
      S.password = typeof r === "string" ? { password: r } : r;
    } catch (e) { S.password = null; say(oops(e, "The password couldn't be read."), "bad"); }
    paint();
  }
  async function saveSettings() {
    try { await call("classic_settings", { signin: prefs.signin, roaming: prefs.roaming }); } catch (_) { /* kept here; sent with Play and Start */ }
  }
  async function signinOff() {
    const c = folder();
    try { const r = await call("classic_signin_off", { client: c ? c.id : null }); say(typeof r === "string" && r ? r : "Your game folder is as it was.", "ok"); } catch (e) { say(oops(e, "The sign-in couldn't be removed. Close the game, then try again."), "bad"); }
  }
  async function openLogs() {
    try { await call("classic_open_logs"); say("The logs folder is open.", "ok"); } catch (e) {
      const where = S.st && S.st.logs_dir;
      say(where ? `The logs are in ${where}` : oops(e, "The logs folder couldn't be opened."), where ? "" : "bad");
    }
  }

  async function readUpdate() {
    try { S.update = await call("launcher_update_state"); } catch (_) { S.update = null; }
    paintUpdate();
  }

  // ------------------------------------------------------------------------------------------ painting
  function go(page) {
    S.page = page;
    say("");
    paint();
    const m = byId("so-main"); if (m) m.focus();
    if (page === "characters") readChars();
  }
  function paintTop() {
    const nav = byId("so-nav");
    nav.hidden = S.page === "first";
    for (const b of nav.querySelectorAll("button")) {
      if (b.dataset.page === S.page) b.setAttribute("aria-current", "page"); else b.removeAttribute("aria-current");
    }
    const run = byId("so-run");
    run.hidden = !setUp();
    run.textContent = running() ? "Running" : S.job ? "Starting…" : "Stopped";
    run.className = "so-run " + (running() ? "ok" : S.job ? "wait" : "");
  }
  function paintUpdate() {
    const u = byId("so-update");
    const ready = S.update && S.update.ready;
    u.hidden = !ready;
    if (!ready) return;
    u.replaceChildren(mk("span", "", `A new Leyline Solo (${typeof ready === "string" ? ready : ready.version || "update"}) is ready.`),
      btn("Restart to update", "btn small", async () => { try { await call("launcher_update_restart"); } catch (e) { say(oops(e), "bad"); } }, "so-update-go"));
  }

  function progressNode() {
    const j = S.job;
    const box = mk("div", "so-prog");
    box.id = "so-prog";
    const of = Math.max(Number(j.of) || 1, 1);
    const n = Math.min(Math.max(Number(j.n) || 1, 1), of);
    const at = typeof j.pct === "number" ? (n - 1 + j.pct / 100) / of : (n - 0.5) / of;
    const pct = Math.max(2, Math.min(100, Math.round(at * 100)));
    const head = mk("div", "so-prog-h");
    head.append(mk("b", "", j.step || j.label || "Working"), mk("span", "so-fine", of > 1 ? `Step ${n} of ${of}` : ""));
    const bar = mk("div", "so-bar");
    bar.setAttribute("role", "progressbar");
    bar.setAttribute("aria-label", j.label || "Progress");
    bar.setAttribute("aria-valuemin", "0");
    bar.setAttribute("aria-valuemax", "100");
    bar.setAttribute("aria-valuenow", String(pct));
    const fill = mk("i", "");
    fill.style.width = pct + "%";
    bar.append(fill);
    box.append(head, bar);
    if (j.detail) box.append(mk("p", "so-fine", j.detail));
    return box;
  }
  function askNode() {
    const box = mk("div", "so-say warn");
    box.id = "so-other";
    const who = S.ask.other ? `${S.ask.other} is using this PC's game ports` : "Another world is using this PC's game ports";
    box.append(mk("p", "", `${who}, and one world runs at a time. Stop it (its characters are saved) and play?`));
    const row = mk("div", "so-row");
    row.append(btn("Stop it and play", "btn small primary", () => { const t = S.ask.then; play(Object.assign({}, t, { stopOther: true })); }, "so-other-go"),
      btn("Not now", "btn small", () => { S.ask = null; paint(); }, "so-other-no"));
    box.append(row);
    return box;
  }
  function noCoreNode() {
    const box = mk("div", "so-say warn", NO_CORE);
    box.id = "so-nocore";
    return box;
  }

  function firstPage(m) {
    m.append(mk("h1", "", "Welcome to Leyline Solo"), mk("p", "so-line", INTRO));
    const steps = mk("ol", "so-steps");
    for (const [k, label] of [["folder", "1. Your game folder"], ["setup", "2. Set up"]]) {
      const li = mk("li", "", label);
      if (S.firstStep === k) li.setAttribute("aria-current", "step");
      steps.append(li);
    }
    m.append(steps);
    if (S.firstStep === "folder") m.append(folderCard(true));
    else {
      const card = mk("section", "so-card");
      card.setAttribute("aria-labelledby", "so-setup-h");
      const h = mk("h2", "", "Set up your world"); h.id = "so-setup-h";
      const c = folder();
      card.append(h, mk("p", "so-line", ESTIMATE));
      if (c) card.append(mk("p", "so-fine", `Game folder: ${c.path}`));
      if (S.noCore) card.append(noCoreNode());
      const row = mk("div", "so-row");
      const go = btn(S.busy ? "Setting up…" : "Set up", "btn primary", () => setup(), "so-setup");
      go.disabled = S.busy || S.noCore;
      row.append(go, btn("Change the game folder", "btn small", () => { S.firstStep = "folder"; paint(); }, "so-setup-back"));
      card.append(row);
      if (S.job) card.append(progressNode());
      card.append(mk("p", "so-fine", "It reads your game folder and never changes it while setting up. Leyline Solo never downloads, offers or shares the game."));
      m.append(card);
    }
  }
  function folderCard(first) {
    const card = mk("section", "so-card");
    card.setAttribute("aria-labelledby", "so-folder-h");
    const h = mk("h2", "", first ? "Point it at your game folder" : "Game folder"); h.id = "so-folder-h";
    card.append(h, mk("p", "so-line", "The folder of your own first-era game (the original, before any expansion): the one that holds the game's program. Leyline Solo reads its maps from it once."));
    const c = folder();
    const row = mk("div", "so-row");
    const lab = mk("label", "so-field so-path", "Folder");
    const input = mk("input", "");
    input.type = "text"; input.id = "so-folder-path"; input.value = c ? c.path : ""; input.placeholder = "C:\\Games\\your game folder";
    input.addEventListener("keydown", (e) => { if (e.key === "Enter") use(); });
    lab.append(input);
    const use = async () => { if (await useFolder(input.value) && first) S.firstStep = "setup"; paint(); };
    const choose = btn("Choose folder…", "btn primary", async () => { if (await chooseFolder() && first) S.firstStep = "setup"; paint(); }, "so-folder-pick");
    row.append(lab, choose, btn("Use this folder", "btn", use, "so-folder-use"));
    card.append(row);
    if (S.folderSay[0]) { const s = mk("p", "so-say " + S.folderSay[1], S.folderSay[0]); s.id = "so-folder-say"; s.setAttribute("role", "alert"); card.append(s); }
    if (first && c && S.firstStep === "folder") card.append(btn("Next: Set up", "btn small", () => { S.firstStep = "setup"; paint(); }, "so-folder-next"));
    return card;
  }

  function homePage(m) {
    m.append(mk("h1", "", "Your world"), mk("p", "so-line", INTRO));
    const card = mk("section", "so-card");
    card.setAttribute("aria-label", "Play");
    if (S.noCore) card.append(noCoreNode());
    const row = mk("div", "so-row");
    const p = btn(S.busy ? "Starting…" : "Play", "btn primary play", () => play(), "so-play");
    p.disabled = S.busy;
    const q = btn(S.quickOpen ? "Hide Quick start" : "Quick start: a new character", "btn", () => { S.quickOpen = !S.quickOpen; if (S.quickOpen) readChoices(); paint(); }, "so-quick-open");
    q.setAttribute("aria-expanded", String(S.quickOpen));
    q.setAttribute("aria-controls", "so-quick");
    const s = btn("Stop", "btn", () => stop(), "so-stop");
    s.disabled = !running() || S.busy;
    row.append(p, q, s);
    card.append(row);
    card.append(mk("p", "so-fine", running() ? "Running on this PC. Play opens your game and signs you in by itself." : "Stopped. Play starts your world, then opens your game and signs you in by itself."));
    if (S.ask) card.append(askNode());
    if (S.job) card.append(progressNode());
    if (S.quickOpen) card.append(quickNode());
    m.append(card);
    const tips = mk("section", "so-card");
    tips.append(mk("h2", "", "Companions"), mk("p", "so-line", "In the game, type .bot group in chat: four companions of the right classes at your level join your party. In party chat: follow, stay, attack."));
    m.append(tips);
  }
  function quickNode() {
    const box = mk("div", "so-quick");
    box.id = "so-quick";
    box.append(mk("h2", "", "Quick start"));
    const races = S.choices && Array.isArray(S.choices.races) ? S.choices.races : null;
    if (!races) { box.append(mk("p", "so-fine", "Reading the races and classes…")); return box; }
    const row = mk("div", "so-row");
    const field = (label, el) => { const l = mk("label", "so-field", label); l.append(el); return l; };
    const race = mk("select", ""); race.id = "so-race";
    for (const r of races) { const o = mk("option", "", r.label); o.value = String(r.id); o.selected = r.id === S.pick.race; race.append(o); }
    race.addEventListener("change", () => { S.pick.race = Number(race.value); fixPick(); store.set("solo.race", String(S.pick.race)); paint(); byId("so-race").focus(); });
    const cls = mk("select", ""); cls.id = "so-class";
    const r = races.find((x) => x.id === S.pick.race) || races[0];
    for (const [id, label] of r.classes) { const o = mk("option", "", label); o.value = String(id); o.selected = id === S.pick.class; cls.append(o); }
    cls.addEventListener("change", () => { S.pick.class = Number(cls.value); store.set("solo.class", String(S.pick.class)); });
    const sex = mk("select", ""); sex.id = "so-sex";
    for (const [v, label] of [["0", "Male"], ["1", "Female"]]) { const o = mk("option", "", label); o.value = v; o.selected = (v === "1") === S.pick.female; sex.append(o); }
    sex.addEventListener("change", () => { S.pick.female = sex.value === "1"; store.set("solo.female", S.pick.female ? "1" : "0"); });
    const name = mk("input", ""); name.id = "so-name"; name.type = "text"; name.maxLength = 12; name.value = S.pick.name || ""; name.autocomplete = "off"; name.spellcheck = false;
    name.addEventListener("input", () => { S.pick.name = name.value; name.removeAttribute("aria-invalid"); });
    name.addEventListener("keydown", (e) => { if (e.key === "Enter") quickStart(); });
    const goq = btn(S.busy ? "Starting…" : "Make and play", "btn primary", () => quickStart(), "so-quick-go");
    goq.disabled = S.busy;
    row.append(field("Race", race), field("Class", cls), field("Male or female", sex), field("Name", name), goq);
    box.append(row, mk("p", "so-fine", "A new level 1 character, made by the game's own screens, then straight into the world. Names are 2 to 12 letters."));
    return box;
  }

  function charsPage(m) {
    m.append(mk("h1", "", "Characters"), mk("p", "so-line", "Your characters in your own world. Play enters that one."));
    const card = mk("section", "so-card");
    if (S.ask) card.append(askNode());
    if (S.job) card.append(progressNode());
    if (S.charsErr) card.append(mk("p", "so-say bad", S.charsErr));
    else if (!S.chars) card.append(mk("p", "so-fine", "Reading your characters…"));
    else if (!S.chars.length) {
      card.append(mk("p", "so-line", "No characters yet."));
      card.append(btn("Quick start: a new character", "btn primary", () => { S.quickOpen = true; go("home"); readChoices(); }, "so-chars-quick"));
    } else {
      const ul = mk("ul", "so-chars");
      ul.id = "so-chars";
      for (const c of S.chars) {
        const li = mk("li", "so-char");
        const t = mk("div", "");
        const cls = c.class_label || c.class || "";
        const zone = c.zone_label || c.zone || "";
        t.append(mk("b", "", c.name), mk("p", "so-fine", [`Level ${c.level}`, [c.race_label, cls].filter(Boolean).join(" "), zone].filter(Boolean).join(" · ")));
        const b = btn(S.busy ? "Starting…" : "Play", "btn small primary", () => play({ pick: c.name }));
        b.disabled = S.busy;
        b.setAttribute("aria-label", `Play ${c.name}`);
        li.append(t, b);
        ul.append(li);
      }
      card.append(ul);
      card.append(mk("p", "so-fine", "Companions are characters on your account too, so they show here."));
    }
    m.append(card);
  }

  function invitePage(m) {
    m.append(mk("h1", "", "Invite a friend"), mk("p", "so-line", "One link. On the same network your friend joins you directly; anywhere else, through the Leyline relay. No router settings."));
    const card = mk("section", "so-card");
    if (!running()) {
      card.append(mk("p", "so-say warn", "Start your world first (Home, Play): friends join while it runs."));
    }
    const row = mk("div", "so-row");
    const b = btn(S.busy ? "Making the link…" : S.invite ? "Make a new link" : "Make the invite link", "btn primary", () => makeInvite(), "so-invite");
    b.disabled = S.busy || !running();
    row.append(b);
    card.append(row);
    if (S.invite) {
      const r2 = mk("div", "so-row");
      r2.style.marginTop = "12px";
      const lab = mk("label", "so-field so-path", "Invite link");
      const li = mk("input", ""); li.type = "text"; li.readOnly = true; li.id = "so-invite-link"; li.value = S.invite.link;
      lab.append(li);
      r2.append(lab, btn("Copy", "btn", () => copy(S.invite.link, "Invite link copied: send it to your friend."), "so-invite-copy"));
      card.append(r2);
      if (Array.isArray(S.invite.ways) && S.invite.ways.length) {
        const ul = mk("ul", "so-fine");
        for (const w of S.invite.ways) ul.append(mk("li", "", w.line || w.kind || ""));
        card.append(ul);
      }
    }
    card.append(mk("p", "so-fine", "Your friend needs their own first-era game folder and Leyline Solo (or the Leyline Launcher). They open the link and press Join."));
    m.append(card);
  }

  function settingsPage(m) {
    m.append(mk("h1", "", "Settings"));
    m.append(folderCard(false));

    const si = mk("section", "so-card");
    si.setAttribute("aria-labelledby", "so-si-h");
    const h = mk("h2", "", "Signing in"); h.id = "so-si-h";
    si.append(h);
    const radios = mk("div", "so-radio");
    radios.setAttribute("role", "radiogroup");
    radios.setAttribute("aria-labelledby", "so-si-h");
    for (const [v, label, line] of [
      ["simple", "Simple", "The game's login screen opens with your account name filled in: type the password once and the game remembers it."],
      ["quick", "Quick sign-in", "A small add-on signs you in by itself, straight to character select."]]) {
      const l = mk("label", "");
      const r = mk("input", ""); r.type = "radio"; r.name = "so-signin"; r.value = v; r.id = "so-signin-" + v; r.checked = prefs.signin === v;
      r.addEventListener("change", () => { prefs.signin = v; saveSettings(); say(`Sign-in: ${label}.`, "ok"); });
      const t = mk("span", ""); t.append(mk("b", "", label), mk("span", "so-fine", " " + line));
      l.append(r, t);
      radios.append(l);
    }
    si.append(radios);
    const pr = mk("div", "so-row"); pr.style.marginTop = "12px";
    if (S.password && S.password.password) {
      const sec = mk("span", "so-secret", S.password.password); sec.id = "so-password";
      pr.append(mk("span", "", (S.password.account || "Password") + ":"), sec,
        btn("Copy", "btn small", () => copy(S.password.password, "Password copied."), "so-password-copy"),
        btn("Hide", "btn small", () => { S.password = null; paint(); }, "so-password-hide"));
    } else pr.append(btn("Show password", "btn small", () => showPassword(), "so-password-show"));
    si.append(pr);
    m.append(si);

    const w = mk("section", "so-card");
    w.append(mk("h2", "", "Your world"));
    const dl = mk("dl", "so-kv");
    const xp = S.st && (S.st.xp_rate || S.st.rate);
    const dd1 = mk("dd", "", xp ? `${xp}x` : "Set when it is set up"); dd1.id = "so-xp";
    const tog = mk("input", ""); tog.type = "checkbox"; tog.id = "so-roaming"; tog.checked = prefs.roaming;
    tog.addEventListener("change", () => { prefs.roaming = tog.checked; saveSettings(); say(tog.checked ? "Roaming bots on: they come in at the next start." : "Roaming bots off: from the next start, just you and your companions.", "ok"); });
    const tl = mk("label", ""); tl.htmlFor = "so-roaming"; tl.append(tog, mk("span", "", " Other adventurers roam the world (off: just you and your companions)"));
    const dd2 = mk("dd", ""); dd2.append(tl);
    dl.append(mk("dt", "", "Experience rate"), dd1, mk("dt", "", "Roaming bots"), dd2);
    w.append(dl);
    const r3 = mk("div", "so-row"); r3.style.marginTop = "14px";
    r3.append(btn("Open the logs folder", "btn small", () => openLogs(), "so-logs"),
      btn("Remove the sign-in from my game folder", "btn small", () => signinOff(), "so-signin-off"));
    w.append(r3, mk("p", "so-fine", "Removing the sign-in puts your game folder back as it was. Close the game first."));
    m.append(w);

    const a = mk("section", "so-card");
    a.setAttribute("aria-labelledby", "so-about-h");
    const ah = mk("h2", "", "About"); ah.id = "so-about-h";
    a.append(ah, mk("p", "", `Leyline Solo ${(S.app && S.app.version) || ""}`.trim()));
    a.append(mk("p", "so-fine", "Leyline Solo is open source (MIT). The world it runs is built from source at pinned commits: the CMaNGOS Classic server (GPL-2.0), its playerbots module (companions) and the classic-db world database, each under its own licence; their sources and licences come with the world's files. The font is Inter (SIL Open Font License 1.1)."));
    a.append(mk("p", "so-fine", "It uses your own game folder. It never downloads, offers or shares the game."));
    const lg = mk("p", "so-legal", LEGAL); lg.id = "so-legal";
    a.append(lg);
    m.append(a);
  }

  function paint() {
    paintTop();
    const m = byId("so-main");
    const keep = document.activeElement && document.activeElement.id;
    m.replaceChildren();
    if (S.page === "first") firstPage(m);
    else if (S.page === "characters") charsPage(m);
    else if (S.page === "invite") invitePage(m);
    else if (S.page === "settings") settingsPage(m);
    else homePage(m);
    if (keep && keep !== "so-main") { const el = byId(keep); if (el) el.focus(); }
    say(S.say[0], S.say[1]);
  }

  // ------------------------------------------------------------------------------------------ start
  for (const b of byId("so-nav").querySelectorAll("button")) b.addEventListener("click", () => go(b.dataset.page));
  async function start() {
    await readApp();
    await readStatus();
    if (folder() && setUp()) S.page = "home";
    else { S.page = "first"; S.firstStep = folder() ? "setup" : "folder"; }
    paint();
    // a setup or start already going (the app was closed and opened again) is followed here
    const p = await progress();
    if (p && p.running) {
      S.busy = true; S.job = Object.assign({ label: setUp() ? "Starting" : "Setting up" }, p); paint();
      try { await follow(S.job.label); if (S.page === "first" && setUp()) S.page = "home"; } catch (e) { say(oops(e), "bad"); }
      S.busy = false; paint();
    }
    readUpdate();
  }
  window.LeylineSolo = { state: () => S, go, play, setup, useFolder };
  start();
})();
