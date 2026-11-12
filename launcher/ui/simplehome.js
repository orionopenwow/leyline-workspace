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
      // the one-click flow (oneclick.js): the folder found or asked for, room, Quick start, the setup card, errors
      const OC = window.LeylineOneClick;
      if (OC) {
        const r = await OC.play(k, { stopOther: !!stopOther, say: (t, kind) => say(t, kind || ""), job: (p) => { S.job = p; paintLine(); },
          onOther: (e) => { S.other = e.other; say(`${e.other.title} is running on this PC. Stop it and play ${title}? Its characters are saved.`, "warn"); },
          onDocker: () => { say("This PC needs one more step for your own world. The Single player page sets it up.", "warn"); nav("single"); } });
        if (r) store.set("leyline.single.played", String(Date.now()));
        return;
      }
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
  // One panel like every other page (its title on the header strip), four cards like Home's (the label on the card's
  // header strip with its line icon, the same as the side menu's), one line for what is happening under the title.
  const ICONS = {
    play: '<path d="M8 5.5v13l10.5-6.5z"/>',
    host: '<circle cx="9" cy="8.5" r="3"/><path d="M3.5 19a5.5 5.5 0 0 1 11 0"/><path d="M16 6.5a3 3 0 0 1 0 5.6M18 19a5.5 5.5 0 0 0-2.5-4.6"/>',
    visit: '<path d="M4 20V5a1 1 0 0 1 1-1h9v16"/><path d="M14 6h4a1 1 0 0 1 1 1v13"/><path d="M10.5 12.5h.01"/><path d="M2.5 20h19"/>',
    backup: '<path d="M12 3 5 6v5c0 4.5 3 8.4 7 10 4-1.6 7-5.5 7-10V6z"/><path d="M9 12l2 2 4-4"/>',
  };
  function icon(key) {
    const s = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    s.setAttribute("viewBox", "0 0 24 24");
    s.setAttribute("aria-hidden", "true");
    s.setAttribute("class", "sh-ic");
    s.innerHTML = ICONS[key];
    return s;
  }
  // a card: the header strip (icon, title, a short state on the right), the line, then the actions at the bottom and
  // one quiet line under them, so the four line up whatever their text
  function card(key, chipText, chipKind, actions, meta) {
    const [title, line] = WORDS[key];
    const c = mk("section", "card sh-card sh-" + key);
    c.id = "sh-" + key;
    c.setAttribute("aria-labelledby", `sh-${key}-title`);
    const head = mk("div", "card-head sh-ch");
    const name = mk("span", "sh-name");
    const t = mk("span", "sh-title", title);
    t.id = `sh-${key}-title`;
    name.append(icon(key), t);
    head.append(name);
    if (chipText) head.append(mk("span", "sh-chip" + (chipKind ? " " + chipKind : ""), chipText));
    const act = mk("div", "sh-actions");
    act.append(...actions);
    c.append(head, mk("p", "sh-line", line), act);
    for (const m of meta) if (m) c.append(m);
    return c;
  }
  function lastText() {
    const last = Number(store.get(KEY_LAST)) || 0;
    if (!last) return "Not backed up from here yet.";
    const hours = Math.floor((Date.now() - last) / 3600000);
    return hours < 1 ? "Last back-up: just now." : hours < 48 ? `Last back-up: ${hours} hour${hours === 1 ? "" : "s"} ago.` : `Last back-up: ${Math.floor(hours / 24)} days ago.`;
  }
  const metaLine = (text, id, cls) => { const p = mk("p", "sh-meta" + (cls ? " " + cls : ""), text); if (id) p.id = id; return p; };

  function paint() {
    const k = world();
    const w = W() && W().WORLDS[k];
    const st = S.st[k];
    const running = !!(st && st.running);
    const ready = !!(st && st.set_up);

    // Play
    // Home in Simple mode: "Play single player" (or the original world), one click whatever its state
    const go = btn(S.busy === "play" ? "Starting…" : k === "single" ? "Play single player" : `Play the ${(w ? w.title : "world").toLowerCase()}`, "btn primary", () => play(false), "sh-play-go");
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
    pick.disabled = !!S.busy;
    pick.addEventListener("change", () => { store.set(KEY_WORLD, pick.value); paint(); });
    const playActs = [go, pick];
    if (S.other) playActs.push(btn(`Stop ${S.other.title} and play`, "btn", () => play(true), "sh-stop-other"));
    const playState = metaLine(st ? (running ? "Running now. Play takes you straight in." : ready ? "Ready. Starts in a minute or two." : "Not set up yet. Play sets it up once (5–20 minutes, first time only), then takes you in.")
      : (w ? "Checking…" : "Update Leyline to play your own world."), "sh-play-state");

    // Host
    const hostGo = btn("Invite a friend", "btn", host, "sh-host-go");
    hostGo.disabled = !!S.busy;
    const hostMore = btn("Host settings", "link", () => nav("host"), "sh-host-more");
    const hostState = metaLine(running ? `${w ? w.title : "Your world"} is running: friends can join as soon as you invite them.`
      : "Friends join with your invite. No ports to open: they come in through the relay.", "sh-host-state");

    // Visit
    const code = btn("I have a link or code", "btn", visitCode, "sh-visit-code");
    const find = btn("Find a world", "btn", () => nav("realms"), "sh-visit-find");
    const soon = metaLine(WORDS.soon, "sh-visit-soon", "soon");

    // Back up
    const n = backups();
    const any = setUp().length > 0;
    const bgo = btn(S.busy === "backup" ? "Backing up…" : "Back up now", "btn", () => backUp(false), "sh-backup-go");
    bgo.disabled = !!S.busy || !any;
    const count = metaLine(any ? `${n} back-up${n === 1 ? "" : "s"} on this PC (the newest 10 kept per world).` : "Nothing to back up yet: play once first.", "sh-backup-state");
    const last = any ? metaLine(lastText(), "sh-backup-last", "tight") : null;
    const lab = mk("label", "check sh-check");
    const box = mk("input");
    box.type = "checkbox";
    box.id = "sh-daily";
    box.checked = daily();
    box.addEventListener("change", () => { store.set(KEY_DAILY, box.checked ? "1" : "0"); if (box.checked) maybeDaily(); });
    lab.append(box, document.createTextNode("Back up every day, by itself"));

    // the panel
    const panel = mk("div", "panel wide sh-panel");
    const head = mk("div", "panel-head");
    const h = mk("h2", "", WORDS.title);
    h.id = "sh-head";
    head.append(h);
    const line = mk("p", "status sh-status");
    line.id = "sh-status";
    line.setAttribute("role", "status");
    line.setAttribute("aria-live", "polite");
    const grid = mk("div", "sh-grid");
    grid.append(
      card("play", st ? (running ? "Running" : ready ? "Ready" : "Not set up") : "", running ? "ok" : ready ? "" : "idle", playActs, [playState]),
      card("host", running ? "Open" : "", "ok", [hostGo, hostMore], [hostState]),
      card("visit", "", "", [code, find], [soon]),
      card("backup", any ? (daily() ? "Daily" : "By hand") : "", daily() ? "ok" : "", [bgo], [count, last, lab]));
    panel.append(head, line, grid);
    root.replaceChildren(panel);
    paintLine();
  }
  function paintLine() {
    const l = byId("sh-status");
    if (!l) return;
    let t = S.line[0];
    if (S.job && S.job.step) t = `${S.job.step}${S.job.pct != null ? ` (${Math.round(S.job.pct)}%)` : ""}`;
    l.textContent = t;
    l.className = "status sh-status" + (S.line[1] && !S.job ? " " + S.line[1] : "");
    l.hidden = !t;
  }

  async function load() {
    paint();
    await read();
    paint();
    maybeDaily();
  }

  document.addEventListener("leyline:page", (e) => {
    const pg = e.detail && e.detail.page;
    if (pg === "simple") load();
    // In Simple mode the four buttons are Home: the full Home page (the launcher opening on it, or anything sending
    // the player "home") lands here instead.
    else if (pg === "play" && isOn()) setTimeout(() => nav("simple"), 0);
  });
  apply();
  // Simple mode opens on its own page; the daily back-up is checked once the launcher has opened, either way.
  if (isOn()) setTimeout(() => nav("simple"), 0);
  setTimeout(() => { if (!isOn()) read().then(maybeDaily); }, 4000);
  window.LeylineSimpleHome = { load, play, host, backUp, maybeDaily, setOn, isOn, WORDS, DAY_MS };
})();
