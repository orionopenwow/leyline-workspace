// Leyline: worlds in three clicks (docs/UNSTOPPABLE.md, "Worlds in three clicks"): Host › a template › Start, then
// friends join from one link. The page (#page-worlds, the side menu's Host while Leyline has no server of its own):
//
//   1. Templates: a tile each (Your own world, The original world, Your Den). A world's tile opens its Start card with
//      its sensible defaults in one line (private: only friends you invite; levelling speed; bots), changeable later on
//      the world's card (the Single player page's "Your world"). One main button: Start.
//   2. Start: the game folder (world_find_folder: one found is used without a word), then world_setup or world_start
//      (host/engine.rs: the prepared database is copied when this PC has one, host/quickworld.rs), its progress
//      followed (window.LeylineWorlds.followJob), then "World running".
//   3. World running: Play (the usual play path), Invite a friend (world_invite_link: a signed one-click invite,
//      host/invite.rs, as the site's https link to copy; the leyline:// form for a friend with Leyline open), Stop.
//   4. Background readiness: after the game folder is found, world_prepare fetches the engine and reads the maps
//      quietly (Settings › Get worlds ready in the background); a small note shows how far it is.
//
// Every core answer is shown with textContent. Loaded after vanilla.js and oneclick.js.
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
  const W = () => window.LeylineWorlds || null;
  const root = byId("wd");
  if (!root) return;
  const cfg = { pollMs: 2000, prepDelayMs: 15000, prepPollMs: 3000 };

  // the templates (host/engine.rs Kind; Your Den has its own page)
  const TEMPLATES = [
    { id: "single", title: "Your own world", line: "Level up from the start with helpers who join you in dungeons.",
      defaults: ["Private: only friends you invite", "Levelling speed ×2", "No roaming bots", "Dungeon helpers on"] },
    { id: "vanilla", title: "The original world", line: "The first era, up to level 60, with the same helpers.",
      defaults: ["Private: only friends you invite", "Levelling speed ×2", "No roaming bots", "Dungeon helpers on"] },
    { id: "den", title: "Your Den", line: "A small place of your own to build and hang out in.", page: "den" },
  ];
  const S = { pick: null, busy: false, line: ["", ""], step: null, running: null, invite: null, prep: null };

  function say(t, k) { S.line = [t || "", k || ""]; const st = byId("wd-status"); if (st) { st.textContent = S.line[0]; st.className = "status wd-status" + (k ? " " + k : ""); } }

  // ---------------------------------------------------------------- drawing
  function paint() {
    root.textContent = "";
    root.append(mk("h2", "", "Host a world"), mk("p", "lede", "Pick one and press Start. Friends join from one link."));
    const grid = mk("div", "wd-tiles");
    grid.setAttribute("role", "list");
    for (const t of TEMPLATES) {
      const b = btn("", "wd-tile" + (S.pick === t.id ? " on" : ""), () => pick(t.id), "wd-t-" + t.id);
      b.setAttribute("role", "listitem");
      b.setAttribute("aria-pressed", String(S.pick === t.id));
      b.disabled = S.busy;
      b.append(mk("b", "wd-tile-h", t.title), mk("span", "wd-tile-l", t.line));
      grid.append(b);
    }
    root.append(grid);
    const t = TEMPLATES.find((x) => x.id === S.pick);
    if (t && !t.page) root.append(card(t));
    const prep = mk("p", "wd-prep");
    prep.id = "wd-prep";
    prep.setAttribute("role", "status");
    root.append(prep);
    paintPrep();
  }

  function card(t) {
    const c = mk("section", "wd-card");
    c.id = "wd-card";
    c.setAttribute("aria-label", t.title);
    if (S.running === t.id) {
      c.append(mk("b", "wd-card-h", "World running"));
      c.id = "wd-card";
      c.dataset.state = "running";
      c.append(mk("p", "wd-fine", `${t.title} is running on this PC. Friends join from your link while it runs.`));
      const acts = mk("div", "wd-acts");
      acts.append(btn("Play", "btn primary", () => play(t.id), "wd-play"), btn("Invite a friend", "btn", () => invite(t.id), "wd-invite"),
        btn("Stop", "btn small", () => stop(t.id), "wd-stop"));
      c.append(acts);
      if (S.invite && S.invite.kind === t.id) c.append(inviteBox(S.invite));
    } else {
      c.append(mk("b", "wd-card-h", t.title));
      const ul = mk("ul", "wd-defaults");
      for (const d of t.defaults) ul.append(mk("li", "", d));
      c.append(ul, mk("p", "wd-fine", "Change these later on the world's card."));
      const go = btn(S.busy ? "Starting…" : "Start", "btn primary", () => start(t.id), "wd-start");
      go.disabled = S.busy;
      c.append(go);
      if (S.step) {
        const pr = mk("div", "wd-progress");
        pr.id = "wd-progress";
        pr.append(mk("span", "wd-step", S.step.step || "Working…"));
        if (typeof S.step.pct === "number") { const bar = mk("progress"); bar.max = 100; bar.value = S.step.pct; pr.append(bar); }
        c.append(pr);
      }
    }
    const st = mk("p", "status wd-status" + (S.line[1] ? " " + S.line[1] : ""), S.line[0]);
    st.id = "wd-status";
    st.setAttribute("role", "status");
    c.append(st);
    return c;
  }

  function inviteBox(inv) {
    const box = mk("div", "wd-link");
    const input = mk("input", "wd-link-in");
    input.id = "wd-link";
    input.readOnly = true;
    input.value = inv.site_link;
    input.setAttribute("aria-label", "Your invite link");
    const copy = (text, done) => async () => {
      try { await navigator.clipboard.writeText(text); say(done, "ok"); } catch (_) { input.select(); say("Select the link and copy it.", "warn"); }
    };
    box.append(input, btn("Copy link", "btn small primary", copy(inv.site_link, "Copied. Send it to one friend: it opens Join in their Leyline."), "wd-copy"),
      btn("Copy the Leyline link", "link-s", copy(inv.app_link, "Copied: it opens straight in a friend's Leyline."), "wd-copy-app"));
    box.append(mk("p", "wd-fine", "One friend per link, good for a week. Same network works today; friends far away need the new connection (coming)."));
    return box;
  }

  // ---------------------------------------------------------------- the three clicks
  async function pick(id) {
    const t = TEMPLATES.find((x) => x.id === id);
    if (!t) return;
    if (t.page) { nav(t.page); return; }
    S.pick = id;
    S.invite = null;
    say("");
    const st = W() ? await W().status(id) : null;
    S.running = st && st.running ? id : null;
    paint();
    const b = byId(S.running ? "wd-play" : "wd-start");
    if (b) b.focus();
  }

  async function start(kind) {
    if (S.busy || !W()) return;
    S.busy = true;
    S.step = { step: "Finding your game folder" };
    say("");
    paint();
    try {
      let found = null;
      try { found = await call("world_find_folder", { kind }); } catch (_) { found = null; }
      const client = found && found.picked ? found.picked : (W().folder() || {}).id;
      if (!client) throw { folder: true };
      let st = await W().status(kind);
      if (!st) throw "This Leyline is too old for worlds in three clicks: update Leyline, then try again.";
      if (!st.supported) throw { docker: true };
      const other = W().otherOf(st);
      if (other) { say(`Stopping ${other.title}… Its characters are saved.`); await W().stopWorld(other.word); st = await W().status(kind); }
      if (!st.running) {
        await call(st.set_up ? "world_start" : "world_setup", st.set_up ? { kind } : { client, kind });
        await W().followJob(kind, { pollMs: cfg.pollMs, job: (p) => { S.step = p; paint(); } });
      }
      S.running = kind;
      S.step = null;
      say("Your world is running. Press Play, or invite a friend.", "ok");
    } catch (e) {
      S.step = null;
      if (e && e.folder) say("Add your game folder first (Settings › Game folders), then press Start again.", "warn");
      else if (e && e.docker) say("On this PC worlds run in Docker: the Single player page sets that up.", "warn");
      else say(oops(e, "Your world didn't start. Press Start again."), "bad");
    }
    S.busy = false;
    paint();
  }

  async function play(kind) {
    say("Taking you in…");
    try {
      if (window.LeylineOneClick && window.LeylineOneClick.play) await window.LeylineOneClick.play(kind);
      else await W().run(kind, {});
    } catch (e) { say(oops(e, "The game didn't start. Press Play again."), "bad"); }
  }

  async function invite(kind) {
    say("Making your link…");
    try {
      const r = await call("world_invite_link", { kind });
      S.invite = Object.assign({ kind }, r);
      paint();
      say("Your link is ready. Send it to one friend.", "ok");
    } catch (e) { say(oops(e, "The link couldn't be made. Is your world running?"), "bad"); }
  }

  async function stop(kind) {
    say("Stopping… everyone is saved first.");
    try { await W().stopWorld(kind); S.running = null; S.invite = null; paint(); say("Stopped. Everyone was saved.", "ok"); }
    catch (e) { say(oops(e), "bad"); }
  }

  // ---------------------------------------------------------------- background readiness
  function paintPrep() {
    const n = byId("wd-prep");
    if (!n) return;
    const p = S.prep;
    if (!p || p.idle) { n.textContent = ""; n.hidden = true; return; }
    n.hidden = false;
    if (p.running) n.textContent = `Getting worlds ready in the background: ${p.step || "working"}${typeof p.pct === "number" ? ` (${p.pct}%)` : ""}.`;
    else if (p.code === 0) n.textContent = "Worlds are ready to start quickly.";
    else n.textContent = "Getting ready in the background stopped. Start does it instead.";
  }
  async function followPrep() {
    for (;;) {
      try { S.prep = await call("world_prepare_progress"); } catch (_) { return; }
      paintPrep();
      if (!S.prep || !S.prep.running) return;
      await new Promise((r) => setTimeout(r, cfg.prepPollMs));
    }
  }
  async function prepare() {
    const app = window.LeylineApp && window.LeylineApp.state ? window.LeylineApp.state() : null;
    if (app && app.settings && app.settings.world_get_ready === false) return;
    let r = null;
    try { r = await call("world_prepare"); } catch (_) { return; }
    if (typeof r === "string" && /background|already/i.test(r)) followPrep();
  }

  // each visit: a game folder added since launch starts readiness now (the core does nothing when it's done or off)
  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "worlds") { if (!root.firstChild) paint(); prepare().then(followPrep); } });
  paint();
  setTimeout(prepare, cfg.prepDelayMs);

  window.LeylineHostWorlds = { TEMPLATES, cfg, pick, start, invite, prepare, state: () => ({ pick: S.pick, busy: S.busy, running: S.running, invite: S.invite }) };
})();
