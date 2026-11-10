// Leyline: Simple mode, the four-button home (owner 10 Nov 2026: "a more simplified launcher" built on the Commonhold
// growth plan: "Play, Host, Visit, Back up; everything else automatic"). Staged: this mode inside Leyline first, then
// the same screen published as a forkable reference launcher in the neutral Commonhold org (docs/SIMPLE-MODE.md).
//
// What it is: one page (#page-simple) with four big choices, and a switch (side menu foot, #nav-simple-toggle) that
// hides the rest of the side menu and the play bar while it is on (html.ll-simple). Everything it does goes through
// the launcher's existing paths, never copies of them:
//   * Play: window.LeylineWorlds.run (vanilla.js), the one path every local world plays by; no game folder or no
//     Built-in engine for this PC sends the player to the Single player page, which handles both.
//   * Host: window.LeylineHostFriends.open (hostfriends.js), Play with friends, Invite a friend in one step.
//   * Visit: Realms' invite box for a link or code a friend sent (app.js joinFriendsDen), or the realm list. Taking
//     your own character to another world arrives with the Commonhold passport (later; said plainly on the card).
//   * Back up: world_backup for every world that is set up (app.rs; host/engine.rs backup: the saves dumped and zipped,
//     the newest 10 kept), the count from world_status, and "Back up every day" (on by default): when this page or the
//     launcher opens and the last back-up made here is over 20 hours old, it backs up once, quietly.
//
// Loaded after vanilla.js, hostfriends.js and app.js. Every core call is caught; an older core says so in plain words.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (typeof e === "string" ? e : fb || "Something went wrong. Try again."));
  const mk = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
  const btn = (label, cls, on, id) => { const b = mk("button", cls || "btn", label); b.type = "button"; if (id) b.id = id; if (on) b.addEventListener("click", on); return b; };
  const byId = (id) => document.getElementById(id);
  const nav = (id) => { if (window.LeylineNav) window.LeylineNav.show(id); };
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* private window */ } } };

  const KEY_ON = "leyline.simple";            // "1": Simple mode on
  const KEY_WORLD = "leyline.simple.world";   // the world Play starts
  const KEY_DAILY = "leyline.simple.daily";   // "0": Back up every day off (on unless turned off)
  const KEY_LAST = "leyline.simple.lastBackup";
  const DAY_MS = 20 * 3600 * 1000;
  // TODO(owner): the four lines in your own voice
  const WORDS = {
    title: "What do you want to do?",
    play: ["Play", "Your own world on this PC, with companions. Works offline."],
    host: ["Host", "Open your world to friends with one invite."],
    visit: ["Visit", "Join a friend's world, or find one to play on."],
    backup: ["Back up", "Keep your characters safe, every day, by itself."],
    soon: "Soon: take your own character to a friend's world and bring it home.",
  };

  const page = byId("page-simple");
  const root = byId("sh");
  if (!page || !root) return;
  const W = () => window.LeylineWorlds;
  const S = { st: {}, busy: "", line: ["", ""], job: null, other: null };

  const isOn = () => store.get(KEY_ON) === "1";
  const daily = () => store.get(KEY_DAILY) !== "0";
  const world = () => {
    const w = store.get(KEY_WORLD);
    return W() && W().WORLDS[w] && w !== "bossrush" ? w : "single";
  };
  const say = (text, kind) => { S.line = [text || "", kind || ""]; paintLine(); };

  // ------------------------------------------------------------------------------------------ the switch
  function apply() {
    const on = isOn();
    document.documentElement.classList.toggle("ll-simple", on);
    const t = byId("nav-simple-toggle");
    if (t) {
      t.setAttribute("aria-pressed", String(on));
      const label = t.querySelector("span");
      if (label) label.textContent = on ? "Show everything" : "Simple mode";
    }
  }
  function setOn(on) {
    store.set(KEY_ON, on ? "1" : "0");
    apply();
    nav(on ? "simple" : "play");
  }
  const toggle = byId("nav-simple-toggle");
  if (toggle) toggle.addEventListener("click", () => setOn(!isOn()));

  // ------------------------------------------------------------------------------------------ reading state
  async function read() {
    const kinds = W() ? Object.keys(W().WORLDS).filter((k) => k !== "bossrush") : [];
    const got = await Promise.all(kinds.map((k) => W().status(k)));
    S.st = {};
    kinds.forEach((k, i) => { S.st[k] = got[i]; });
  }
  const setUp = () => Object.keys(S.st).filter((k) => S.st[k] && S.st[k].set_up);
  const backups = () => setUp().reduce((n, k) => n + (Number(S.st[k].backups) || 0), 0);

  // ------------------------------------------------------------------------------------------ the four choices
  async function play(stopOther) {
    if (S.busy) return;
    if (!W()) { nav("single"); return; }
    const k = world();
    const title = W().WORLDS[k].title;
    S.busy = "play"; S.other = null; paint();
    try {
      const r = await W().run(k, { stopOther: !!stopOther, say: (t) => say(t, ""), job: (p) => { S.job = p; paintLine(); } });
      store.set("leyline.single.played", String(Date.now()));
      const [t, kind] = W().played(r, title);
      say(t, kind);
    } catch (e) {
      if (e && e.folder) { say("First, show Leyline your game folder. The Single player page walks you through it.", "warn"); nav("single"); }
      else if (e && e.docker) { say("This PC needs one more step for your own world. The Single player page sets it up.", "warn"); nav("single"); }
      else if (e && e.other) { S.other = e.other; say(`${e.other.title} is running on this PC. Stop it and play ${title}? Its characters are saved.`, "warn"); }
      else say(oops(e, "Your world didn't start. Press Play again."), "bad");
    } finally {
      S.busy = ""; S.job = null;
      await read();
      paint();
    }
  }

  async function host() {
    if (window.LeylineHostFriends && window.LeylineHostFriends.open) {
      try { await window.LeylineHostFriends.open({ kind: world() }); } catch (e) { say(oops(e), "bad"); }
    } else nav("host");
  }

  function visitCode() {
    if (typeof window.joinFriendsDen === "function") window.joinFriendsDen();
    else nav("realms");
  }

  async function backUp(quiet) {
    if (S.busy) return false;
    const kinds = setUp();
    if (!kinds.length) {
      if (!quiet) say("There's nothing to back up yet. Press Play once to make your world.", "warn");
      return false;
    }
    S.busy = "backup"; paint();
    if (!quiet) say("Backing up your characters…", "");
    let ok = 0;
    const errs = [];
    for (const k of kinds) {
      try { await call("world_backup", { kind: k }); ok++; } catch (e) { errs.push(`${W().WORLDS[k].title}: ${oops(e)}`); }
    }
    if (ok) store.set(KEY_LAST, String(Date.now()));
    S.busy = "";
    await read();
    paint();
    if (errs.length) say(errs.join(" "), "bad");
    else if (!quiet) say(ok === 1 ? "Backed up. Your characters are safe on this PC." : `Backed up ${ok} worlds. Your characters are safe on this PC.`, "ok");
    return ok > 0;
  }

  // Back up every day: once, quietly, when the last back-up made here is over 20 hours old (or never).
  async function maybeDaily() {
    if (!daily() || S.busy) return;
    const last = Number(store.get(KEY_LAST)) || 0;
    if (Date.now() - last < DAY_MS) return;
    if (!setUp().length) return;
    const done = await backUp(true);
    if (done && isOn()) say("Your daily back-up is done.", "ok");
  }

  // ------------------------------------------------------------------------------------------ painting
  function card(key, id, extra) {
    const [title, line] = WORDS[key];
    const c = mk("section", "sh-card sh-" + key);
    c.id = "sh-" + key;
    c.setAttribute("aria-labelledby", id + "-title");
    const h = mk("h2", "sh-title", title);
    h.id = id + "-title";
    c.append(h, mk("p", "sh-line", line), ...extra);
    return c;
  }
  function lastText() {
    const last = Number(store.get(KEY_LAST)) || 0;
    if (!last) return "Not backed up from here yet.";
    const hours = Math.floor((Date.now() - last) / 3600000);
    return hours < 1 ? "Last back-up: just now." : hours < 48 ? `Last back-up: ${hours} hour${hours === 1 ? "" : "s"} ago.` : `Last back-up: ${Math.floor(hours / 24)} days ago.`;
  }

  function paint() {
    const k = world();
    const w = W() && W().WORLDS[k];
    const st = S.st[k];
    // Play
    const go = btn(S.busy === "play" ? "Starting…" : st && st.running ? "Play" : st && st.set_up ? "Play" : "Set up and play", "btn primary big", () => play(false), "sh-play-go");
    go.disabled = !!S.busy;
    const pick = mk("select", "sh-pick");
    pick.id = "sh-world";
    pick.setAttribute("aria-label", "Which world");
    if (W()) for (const key of Object.keys(W().WORLDS).filter((x) => x !== "bossrush")) {
      const o = mk("option", "", W().WORLDS[key].title);
      o.value = key;
      if (key === k) o.selected = true;
      pick.append(o);
    }
    pick.addEventListener("change", () => { store.set(KEY_WORLD, pick.value); paint(); });
    const state = mk("p", "sh-state", st ? (st.running ? "Running now." : st.set_up ? "Ready. Starts in a minute or two." : "Not set up yet. The first time takes a while.") : (w ? "" : "Update Leyline to play your own world."));
    state.id = "sh-play-state";
    const playExtra = [mk("div", "sh-row"), state];
    playExtra[0].append(go, pick);
    if (S.other) {
      const stop = btn(`Stop ${S.other.title} and play`, "btn", () => play(true), "sh-stop-other");
      playExtra.push(stop);
    }

    // Host
    const hostGo = btn("Invite a friend", "btn big", host, "sh-host-go");
    hostGo.disabled = !!S.busy;

    // Visit
    const code = btn("I have a link or code", "btn big", visitCode, "sh-visit-code");
    const find = btn("Find a world", "btn", () => nav("realms"), "sh-visit-find");
    const vrow = mk("div", "sh-row");
    vrow.append(code, find);
    const soon = mk("p", "sh-soon", WORDS.soon);

    // Back up
    const n = backups();
    const bgo = btn(S.busy === "backup" ? "Backing up…" : "Back up now", "btn big", () => backUp(false), "sh-backup-go");
    bgo.disabled = !!S.busy;
    const count = mk("p", "sh-state", setUp().length ? `${n} back-up${n === 1 ? "" : "s"} kept on this PC (the newest 10 per world). ${lastText()}` : "Nothing to back up yet.");
    count.id = "sh-backup-state";
    const lab = mk("label", "sh-check");
    const box = mk("input");
    box.type = "checkbox";
    box.id = "sh-daily";
    box.checked = daily();
    box.addEventListener("change", () => { store.set(KEY_DAILY, box.checked ? "1" : "0"); if (box.checked) maybeDaily(); });
    lab.append(box, document.createTextNode(" Back up every day, by itself"));

    const head = mk("h1", "sh-head", WORDS.title);
    head.id = "sh-head";
    const grid = mk("div", "sh-grid");
    grid.append(card("play", "sh-play", playExtra), card("host", "sh-host", [hostGo]),
      card("visit", "sh-visit", [vrow, soon]), card("backup", "sh-backup", [bgo, count, lab]));
    const line = mk("p", "status sh-status");
    line.id = "sh-status";
    line.setAttribute("role", "status");
    line.setAttribute("aria-live", "polite");
    const more = btn("Show everything", "linkish", () => setOn(false), "sh-more");
    const foot = mk("p", "sh-foot");
    foot.append(more);
    root.replaceChildren(head, grid, line, foot);
    paintLine();
  }
  function paintLine() {
    const l = byId("sh-status");
    if (!l) return;
    let t = S.line[0];
    if (S.job && S.job.step) t = `${S.job.step}${S.job.pct != null ? ` (${Math.round(S.job.pct)}%)` : ""}`;
    l.textContent = t;
    l.className = "status sh-status" + (S.line[1] && !S.job ? " " + S.line[1] : "");
  }

  async function load() {
    paint();
    await read();
    paint();
    maybeDaily();
  }

  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "simple") load(); });
  apply();
  // Simple mode opens on its own page; the daily back-up is checked once the launcher has opened, either way.
  if (isOn()) setTimeout(() => nav("simple"), 0);
  setTimeout(() => { if (!isOn()) read().then(maybeDaily); }, 4000);
  window.LeylineSimpleHome = { load, play, host, backUp, maybeDaily, setOn, isOn, WORDS, DAY_MS };
})();
