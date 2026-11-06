// Leyline: the original world, single player, in one click (docs/SINGLE-PLAYER-ONE-CLICK.md; the answer to a player's
// wish, 5 Nov: "one EXE file that just puts you into the game ... self sustainable and doesn't require a server").
//
// Two parts:
//   * window.LeylineWorlds: the one path every local world plays by (Single player, the original world, Boss Rush:
//     host/engine.rs KINDS, app.rs world_*): its state, set up (once) or started when it isn't running, its progress
//     followed (engine_progress, the line's own kind), then the game signed in by itself (world_play: the world's own
//     login, made by the launcher; Quick start's new character, or a picked one). One world runs at a time (they
//     share this PC's ports): another one running is said, with "Stop it and play", never stopped unasked.
//   * The original world's card at the top of the Single player page (#sv): one button, "Play the original world
//     (level 60) with companions", and Quick start (a level 1 character of a race and class picked here, made and
//     entered by the game's own screens). Once it is set up: Running / Stopped, Stop, Invite a friend, Characters.
//     Not on Windows (the Built-in engine is a Windows build for now): Vanilla+ on the Host page, in Docker.
//
// Loaded after app.js and single.js. Every core call is caught: an older core without world_* shows the card's line.
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
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* private window */ } } };

  // ---------------------------------------------------------------- the worlds (host/engine.rs Kind)
  // word: the commands' name; job: engine_progress's kind; serde: EngineStatus.kind / other_running
  const WORLDS = {
    single: { word: "single", job: "engine", serde: "single", title: "Single player" },
    vanilla: { word: "vanilla", job: "vanilla", serde: "vanilla", title: "Original world" },
    bossrush: { word: "bossrush", job: "bossrush", serde: "boss_rush", title: "Boss Rush" },
  };
  const bySerde = (s) => Object.values(WORLDS).find((w) => w.serde === s || w.word === s) || null;
  const BUILD = "3.3.5a";
  const appState = () => (window.LeylineApp && window.LeylineApp.state ? window.LeylineApp.state() : null);
  // the folder the local worlds play with: the one picked on the Single player page, else any of its version
  function folder() {
    const s = appState();
    const list = s ? s.settings.clients.filter((c) => c.build === BUILD) : [];
    return list.find((c) => c.id === store.get("leyline.single.folder")) || list[0] || null;
  }
  async function status(kind) {
    try { const s = await call("world_status", { kind }); return s && typeof s === "object" ? s : null; } catch (_) { return null; }
  }
  // the other world running on these ports, as a WORLDS entry (null: none)
  const otherOf = (st) => (st && st.other_running ? bySerde(st.other_running) : null);
  async function stopWorld(kind) {
    return call(kind === "single" ? "engine_stop" : "world_stop", kind === "single" ? undefined : { kind });
  }
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  // Make a world ready and play it. o: { quick, pick, say(text, kind), job(progress|null), stopOther } → the play's answer.
  // Throws { other } when another world runs here and stopOther isn't set; { docker } when the Built-in engine isn't
  // made for this PC; a string otherwise.
  async function run(kind, o) {
    o = o || {};
    const say = o.say || (() => {});
    const w = WORLDS[kind];
    if (!w) throw "That isn't one of this PC's worlds.";
    const c = folder();
    if (!c) throw { folder: true };
    let st = await status(kind);
    if (!st) throw "This Leyline is from before the original world: update Leyline, then try again.";
    if (!st.supported) throw { docker: true };
    const other = otherOf(st);
    if (other) {
      if (!o.stopOther) throw { other };
      say(`Stopping ${other.title}… Its characters are saved.`);
      await stopWorld(other.word);
      st = await status(kind);
    }
    if (!(st && st.running)) {
      say(st && st.set_up ? `Starting ${w.title}…` : `Setting up ${w.title} (once)…`);
      await call(st && st.set_up ? "world_start" : "world_setup", st && st.set_up ? { kind } : { client: c.id, kind });
      // follow its progress line until it is done
      for (;;) {
        await sleep(o.pollMs || 2000);
        let p = null;
        try { p = await call("engine_progress"); } catch (_) { p = null; }
        if (p && p.kind && p.kind !== w.job) {
          // another world's line: ours isn't going (it was running already, or didn't start)
          if (p.running) continue;
          const now = await status(kind);
          if (now && now.running) break;
          throw `${w.title} didn't start. Press Play again.`;
        }
        if (o.job) o.job(p && !p.idle ? p : null);
        if (p && p.running) continue;
        if (p && !p.idle && p.code !== 0) throw p.problem || `${w.title} didn't start.`;
        break;
      }
      if (o.job) o.job(null);
    }
    say(o.quick ? "Making your character and taking you in…" : "Taking you in…");
    const args = { client: c.id, kind };
    if (o.pick) args.pick = o.pick;
    if (o.quick) args.quick = o.quick;
    return call("world_play", args);
  }
  // The line after a play: signed in by itself, or the login to type once.
  function played(r, title) {
    if (!r) return ["Started.", "ok"];
    if (r.auto === false && r.password) return [`Started. At the login screen: ${r.account}, password ${r.password} (made for you; kept on this PC).`, "warn"];
    return [r.made ? `Started: making ${r.made} and taking you into ${title}.` : (r.message || "Started."), "ok"];
  }
  window.LeylineWorlds = { WORLDS, folder, status, run, played, stopWorld, otherOf, bySerde };

  // ---------------------------------------------------------------- the original world's card (#sv)
  const page = byId("page-single");
  const root = byId("sv");
  if (!page || !root) return;
  const KIND = "vanilla";
  // TODO(owner): the card's name and line, his words
  const NAME = "Original world";
  const LINE = "The original world as it first was, to level 60, with your companions and dungeon buddies. It runs on this PC: no server, no account to make.";
  const GO = "Play the original world (level 60) with companions";
  const MARK = '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z"/><path d="M3.6 9h16.8M3.6 15h16.8M12 3c2.5 2.6 3.6 5.6 3.6 9s-1.1 6.4-3.6 9c-2.5-2.6-3.6-5.6-3.6-9S9.5 5.6 12 3z"/></svg>';

  let st = null;        // world_status vanilla
  let job = null;       // its progress line while it starts
  let line = ["", ""];
  let busy = false;
  let askOther = null;  // another world running: { other, quick }
  let choices = null;   // world_choices: races and their classes
  let quickOpen = false;
  let pick = { race: Number(store.get("leyline.vanilla.race")) || 1, class: Number(store.get("leyline.vanilla.class")) || 1, female: store.get("leyline.vanilla.female") === "1", name: "" };

  function say(text, kind) { line = [text || "", kind || ""]; const s = byId("sv-status"); if (s) { s.textContent = line[0]; s.className = "status sv-status" + (line[1] ? " " + line[1] : ""); } }

  async function read() {
    st = await status(KIND);
    paint();
  }
  async function readChoices() {
    if (choices) return;
    try { choices = await call("world_choices", { kind: KIND }); } catch (_) { choices = null; }
    if (choices && choices.name && !pick.name) pick.name = choices.name;
    fixPick();
    paint();
  }
  // the class picked must be one the race can be
  function fixPick() {
    if (!choices || !choices.races || !choices.races.length) return;
    const r = choices.races.find((x) => x.id === pick.race) || choices.races[0];
    pick.race = r.id;
    if (!r.classes.some((c) => c[0] === pick.class)) pick.class = r.classes[0][0];
  }

  async function go(quick, stopOther) {
    if (busy) return;
    askOther = null;
    busy = true;
    paint();
    try {
      const r = await run(KIND, { quick: quick ? { name: pick.name || null, race: pick.race, class: pick.class, female: pick.female } : null,
        stopOther, say: (t, k) => say(t, k), job: (p) => { job = p; paint(); } });
      const [t, k] = played(r, NAME);
      say(t, k);
      if (quick && choices) { choices = null; pick.name = ""; }
    } catch (e) {
      if (e && e.other) { askOther = { other: e.other, quick }; say(""); }
      else if (e && e.docker) say("The original world's one click runs on the Built-in engine, made for Windows PCs for now. On this PC, Vanilla+ runs the same world in Docker: open it on the Host page.", "warn");
      else if (e && e.folder) {
        say("Add your game folder first: the original world reads its maps from it once. Leyline never downloads the game.", "warn");
        const L = window.LeylineLocal;
        if (L && L.addFolder) L.addFolder("solo", "The original world plays with your own game folder. Add it, then press Play again.");
      } else say(oops(e, "The original world didn't start. Try again."), "bad");
    }
    job = null;
    busy = false;
    await read();
    if (quickOpen && !choices) readChoices();   // a fresh name to suggest after Quick start made one
  }

  function progressNode() {
    const box = mk("div", "sp-prog");
    box.id = "sv-prog";
    const head = mk("div", "sp-prog-h");
    head.append(mk("b", "", job.step || "Getting ready"), mk("span", "sp-fine", `Step ${Math.max(1, job.n || 1)} of ${job.of || 7}`));
    const bar = mk("div", "sp-bar");
    const fill = mk("i", "");
    const at = typeof job.pct === "number" ? Math.max((job.n || 1) - 1, 0) + job.pct / 100 : Math.max((job.n || 1) - 0.5, 0.3);
    fill.style.width = `${Math.round(at / Math.max(job.of || 7, 1) * 100)}%`;
    bar.append(fill);
    bar.setAttribute("role", "progressbar");
    bar.setAttribute("aria-label", "Setting up the original world");
    box.append(head, bar);
    if (job.detail || typeof job.pct === "number") box.append(mk("p", "sp-fine", [typeof job.pct === "number" ? `${job.pct}%` : "", job.detail || ""].filter(Boolean).join(" · ")));
    box.append(mk("p", "sp-fine", "The first time gets the engine (shared with Single player), reads the maps from your game folder and fills the world: 10 to 30 minutes. Then it takes you in by itself."));
    return box;
  }

  function quickNode() {
    const box = mk("div", "sv-quick");
    box.id = "sv-quick";
    if (!choices) { box.append(mk("p", "sp-fine", "Reading the races and classes…")); return box; }
    const row = mk("div", "sp-row");
    const race = mk("select", "sv-sel");
    race.id = "sv-race";
    race.setAttribute("aria-label", "Race");
    for (const r of choices.races) { const o = mk("option", "", r.label); o.value = String(r.id); o.selected = r.id === pick.race; race.append(o); }
    race.addEventListener("change", () => { pick.race = Number(race.value); fixPick(); store.set("leyline.vanilla.race", String(pick.race)); paint(); });
    const cls = mk("select", "sv-sel");
    cls.id = "sv-class";
    cls.setAttribute("aria-label", "Class");
    const r = choices.races.find((x) => x.id === pick.race) || choices.races[0];
    for (const [id, label] of r.classes) { const o = mk("option", "", label); o.value = String(id); o.selected = id === pick.class; cls.append(o); }
    cls.addEventListener("change", () => { pick.class = Number(cls.value); store.set("leyline.vanilla.class", String(pick.class)); });
    const sex = mk("select", "sv-sel");
    sex.id = "sv-sex";
    sex.setAttribute("aria-label", "Male or female");
    for (const [v, label] of [["0", "Male"], ["1", "Female"]]) { const o = mk("option", "", label); o.value = v; o.selected = (v === "1") === pick.female; sex.append(o); }
    sex.addEventListener("change", () => { pick.female = sex.value === "1"; store.set("leyline.vanilla.female", pick.female ? "1" : "0"); });
    const name = mk("input", "sv-name");
    name.id = "sv-name";
    name.type = "text";
    name.maxLength = 12;
    name.placeholder = "Name";
    name.value = pick.name || "";
    name.setAttribute("aria-label", "Name");
    name.addEventListener("input", () => { pick.name = name.value; });
    const qg = btn(busy ? "Starting…" : "Quick start", "btn small primary", () => go(true, false), "sv-quick-go");
    qg.disabled = busy;
    row.append(race, cls, sex, name, qg);
    box.append(row, mk("p", "sp-fine", "A new level 1 character, made by the game's own screens, then straight into the world. The original world has the first era's races and classes only."));
    return box;
  }

  function paint() {
    const card = mk("section", "sv");
    card.setAttribute("aria-label", NAME);
    const head = mk("div", "sv-head");
    const ic = mk("span", "sp-mark sv-mark");
    ic.innerHTML = MARK;
    const ht = mk("div", "sp-head-t");
    const h = mk("h3", "", NAME);
    const tags = mk("span", "sv-tags", "Level 60 · Companions · Just you, and friends you invite");
    ht.append(h, tags, mk("p", "sp-line", LINE));
    head.append(ic, ht);
    const known = !!st;
    const up = !!(st && st.running);
    if (st && st.set_up) {
      const chip = mk("span", "sp-dock " + (up ? "ok" : job ? "wait" : "warn"), up ? "Running" : job ? "Starting…" : "Stopped");
      chip.id = "sv-run";
      head.append(chip);
    }
    card.append(head);
    if (askOther) {
      const o = mk("div", "sv-other");
      o.id = "sv-other";
      o.append(mk("p", "sp-line", `${askOther.other.title} is running on this PC, and one world runs at a time. Stop it (its characters are saved) and play the original world?`));
      const r = mk("div", "sp-row");
      r.append(btn(`Stop ${askOther.other.title} and play`, "btn small primary", () => go(askOther.quick, true), "sv-other-go"), btn("Not now", "btn small", () => { askOther = null; paint(); }));
      o.append(r);
      card.append(o);
    }
    const row = mk("div", "sp-row");
    const play = btn(busy ? "Starting…" : GO, "btn primary sv-go", () => go(false, false), "sv-play");
    play.disabled = busy;
    row.append(play);
    const qt = btn(quickOpen ? "Hide Quick start" : "Quick start: a new character", "btn small", () => { quickOpen = !quickOpen; if (quickOpen) readChoices(); paint(); }, "sv-quick-open");
    qt.setAttribute("aria-expanded", String(quickOpen));
    row.append(qt);
    if (st && st.set_up) {
      const stop = btn("Stop", "btn small", async (ev) => { ev.currentTarget.disabled = true; try { say(await call("world_stop", { kind: KIND }), "ok"); } catch (e) { say(oops(e), "bad"); } read(); }, "sv-stop");
      stop.disabled = !up || busy;
      const inv = btn("Invite a friend", "btn small", () => {
        if (window.LeylineHostFriends && window.LeylineHostFriends.open) window.LeylineHostFriends.open({ kind: KIND });
        else say("Start it, then invite from Play with friends.", "warn");
      }, "sv-invite");
      inv.disabled = !up;
      inv.title = up ? "A friend joins your original world: same network or through the Leyline relay" : "Start it first: friends join while it runs";
      row.append(stop, inv);
    }
    // every world's characters, this one's and the online ones (characters.js), from here: no side menu item
    row.append(btn("Characters", "btn small", () => nav("characters"), "sv-chars"));
    card.append(row);
    if (quickOpen) card.append(quickNode());
    if (job) card.append(progressNode());
    if (known && !st.supported) {
      const d = mk("div", "sv-docker");
      d.id = "sv-docker";
      d.append(mk("p", "sp-line", "Here the one click isn't made yet (the Built-in engine is for Windows PCs for now). Vanilla+ is the same original world, in Docker:"));
      d.append(btn("Open Vanilla+ on the Host page", "btn small", () => { nav("host"); document.dispatchEvent(new CustomEvent("leyline:host-ruleset", { detail: { profile: "vanillaplus" } })); }, "sv-docker-go"));
      card.append(d);
    } else if (!(st && st.set_up)) {
      card.append(mk("p", "sp-fine", "Needs your own game folder, once (Leyline never downloads or ships the game). The first time takes 10 to 30 minutes; after that, a minute or two."));
    }
    const s = mk("p", "status sv-status" + (line[1] ? " " + line[1] : ""), line[0]);
    s.id = "sv-status";
    s.setAttribute("role", "status");
    card.append(s);
    root.replaceChildren(card);
  }

  // a start already under way (begun on the Characters page, or before the page was opened) is followed here too
  async function load() {
    await read();
    if (busy) return;
    let p = null;
    try { p = await call("engine_progress"); } catch (_) { p = null; }
    if (p && p.running && p.kind === WORLDS.vanilla.job) {
      job = p;
      paint();
      for (let i = 0; i < 4000 && job; i++) {
        await sleep(2000);
        try { p = await call("engine_progress"); } catch (_) { p = null; }
        job = p && p.running && p.kind === WORLDS.vanilla.job ? p : null;
        paint();
      }
      await read();
    }
  }
  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "single") load(); });
  // Home's Single player row: the original world in one click (on this page, where its progress shows), Characters
  const hv = byId("hm-vanilla-go");
  if (hv) hv.addEventListener("click", () => { nav("single"); go(false, false); });
  const hc = byId("hm-chars");
  if (hc) hc.addEventListener("click", () => nav("characters"));
  window.LeylineVanilla = { load, go, NAME, GO, state: () => st };
  paint();
  if (page.classList.contains("on")) load();
})();
