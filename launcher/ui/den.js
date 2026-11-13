// Leyline Launcher: Your Den (docs/DEN.md "Your Den"). One click, from the side menu, the play bar's realm picker or Home's
// banner (all go()), into your own quiet green world to build in; one page for everything about it: Play, Invite friends and
// Ask Claude to edit it on top, each thing one click with a one-line hint.
//
// Where the Den runs (host/yourden.rs, host/lite.rs):
//   * Den Lite, the built-in Den server: no Docker, straight into the room. Only when the program is on this PC
//     (den_state().lite.available); else the Docker Den below is the way in.
//   * the Docker Den (the Host pack's "den" profile): the bigger server. "Move to a full server" moves the Lite Den
//     there (den_share_move: the same files, the newest opened there unchanged) through three steps: the Host pack,
//     Docker Desktop, Start. Then Invite a friend.
// Share with friends on Den Lite needs no Docker (owner 4 Nov: "easy Den hosting for friends ... one click"; docs/
// DEN-HOSTING.md): Invite a friend puts this Den on the Leyline relay (no port forwarding, no home address shown) and
// copies a link with a login for one friend, and a code to paste; the Friends list (den_friends, host/denhost.rs)
// shows who is in, with Build, Keep and Kick for each friend, and the links that still work. Sharing stays on across
// restarts until Stop sharing.
// Saved Dens, Packs and Share on the Workshop work the same for both (den_text / den_add / den_open / den_fetch on
// whichever Den is in use). Playing the Docker Den is app.js's own (playLocal(DEN): the relay's knock, then PLAY), with
// its account filled in on the login screen (den_prefill_account).
// Shape the ground (host/forge.rs, host/terrain.rs): Forge, the Den's terrain editor (a GPL program of its own), is
// got once ("Get Forge": the GPL notice and the source link first, once), then "Open in Forge" starts it on the Den's
// tile; when it closes the launcher takes what it saved as the Den's ground and says "Terrain updated".
// Single player's page has Your Den too (owner 12 Nov: "add the den feature to the single player tab as well"): a
// card (#spd) drawn here from the same state and the same functions as this page: what it is, Open my Den (go), Invite
// a friend (inviteFriends), its state and the last thing said, and Stop sharing while it is on the network.
// Loaded after app.js (its LOCAL, playLocal, clientForRealm, addGameFolder, readKit) and hostconsole.js.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (fb || "Something went wrong. Try again."));
  const byId = (id) => document.getElementById(id);
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const btn = (label, cls, on) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (on) b.addEventListener("click", on); return b; };
  const page = byId("page-den");
  const root = byId("yd");
  if (!page || !root) return;
  const spd = byId("spd");   // Single player's Your Den card (paintCard)
  const BUILD = "3.3.5a";
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { if (v == null) localStorage.removeItem(k); else localStorage.setItem(k, v); } catch (_) { /* private window */ } } };
  const PENDING = "leyline.denStartPending";   // hostconsole.js opens this Den file once the Docker Den's start is done
  const ACCOUNT_KEY = "leyline.denAccount";
  const ACCT_OK = /^[A-Za-z0-9]{3,16}$/;

  let st = null;        // den_state
  let cons = null;      // host_console (the Docker Den only)
  let dock = null;      // docker_info
  let pack = null;      // hostpack_info
  let job = null;       // host_progress
  let jobTimer = null;
  let sharing = false;  // the Share with friends steps are open
  let busy = false;
  let hint = "";        // one line under the status after Open my Den (how to skip typing next time)
  let forge = null;     // forge_state
  let forgeBusy = "";   // "get" while Forge downloads
  let forgeGpl = false; // the GPL notice is open
  let forgeTimer = null;
  const FORGE_GPL = "leyline.forgeGplSeen";
  let invited = null;   // den_invite: the link and its ways (host/denways.rs), shown under the buttons
  let friends = null;   // den_friends: who's in, the friends' logins, sharing (host/denhost.rs)
  let friendsTimer = null;
  let friendBusy = "";  // the friend whose button is working
  let models = null;    // den_models: the player's own models (host/denmodels.rs)
  let modelsBusy = false;
  let modelName = "";

  const status = mk("p", "status yd-status");
  status.setAttribute("role", "status");
  function say(text, kind, ver) {
    status.textContent = text || "";
    if (ver) status.append(" ", mk("span", "rl-ver sm", ver));   // the version as a chip, never in the sentence
    status.dataset.kind = kind || "";
    paintCard();
  }

  // ---------------------------------------------------------------- what this PC has
  const own = () => (typeof clientForRealm === "function" ? clientForRealm("den", BUILD) : null);
  // Den Lite here, or got by Play on first use (one small download, st.lite.can_get)
  const liteOn = () => !!(st && st.lite && (st.lite.available || st.lite.can_get));
  const liteHas = () => !!(st && st.lite && st.lite.available);
  const liteGet = () => !!(st && st.lite && !st.lite.available && st.lite.can_get);
  const liteUp = () => !!(st && st.lite && st.lite.running);
  const dockerUp = () => !!(cons && (cons.services || []).some((s) => s.role === "world" && s.state === "ok"));
  // where your Den is: "lite" (Den Lite, no Docker), "docker" (the Host pack's Den), or "new" (nothing yet)
  function where() {
    if (st && st.docker_den) return "docker";
    if (liteOn()) return "lite";
    return "new";
  }

  async function load() {
    try { st = await call("den_state"); } catch (e) { st = null; say(oops(e, "Your Den isn't answering. Try again."), "bad"); }
    cons = null;
    if (st && st.docker_den) { try { cons = await call("host_console"); } catch (_) { cons = null; } }
    try { forge = await call("forge_state"); } catch (_) { forge = null; }
    paint();
    if (forge && forge.running) followForge();
    try { net = await call("den_net_state"); } catch (_) { net = null; }
    if (net && net.running) { paint(); followNet(); }
    if (where() === "lite") loadFriends();
    try { models = await call("den_models"); paint(); } catch (_) { models = null; }
    // Docker on this PC: Single player is offered as the bigger way to play alone (never waited for)
    if (!dock) call("docker_info").then((d) => { dock = d; paint(); }).catch(() => { /* no Docker: not offered */ });
  }
  // Single player (app.js LOCAL.solo, the Host kit's "solo" ruleset): its own page (single.js) until it is set up, then played
  function soloLeveling() {
    if (typeof localGo === "function" && typeof LOCAL !== "undefined" && LOCAL.solo) localGo(LOCAL.solo);
    else if (window.LeylineNav) window.LeylineNav.show("host");
  }

  // ---------------------------------------------------------------- Shape the ground: Forge
  async function getForge() {
    if (!store.get(FORGE_GPL) && !forgeGpl) { forgeGpl = true; paint(); return; }   // the notice first, once
    store.set(FORGE_GPL, "1");
    forgeGpl = false;
    forgeBusy = "get"; paint();
    say("Getting Forge… (a one-time download)");
    try { await call("forge_install"); say("Forge is ready. Open in Forge to shape your Den's ground.", "ok"); }
    catch (e) { say(oops(e, "Forge didn't download. Try again."), "bad"); }
    forgeBusy = "";
    try { forge = await call("forge_state"); } catch (_) { /* as it was */ }
    paint();
  }
  async function openForge() {
    const c = own();
    if (!c) { say("Your Den's ground is made from your own game folder. Add it on the Game folders page first.", "warn"); return; }
    try { say(await call("forge_open", { client: c.id }), "ok"); }
    catch (e) { say(oops(e, "Forge didn't open. Try again."), "bad"); }
    try { forge = await call("forge_state"); } catch (_) { /* as it was */ }
    paint();
    followForge();
  }
  // while Forge is open: ask every 2 seconds; once it closes, say what its close did ("Terrain updated")
  function followForge() {
    if (forgeTimer) return;
    const seq0 = forge && forge.last ? forge.last.seq : 0;
    const tick = async () => {
      let f = null;
      try { f = await call("forge_state"); } catch (_) { /* again */ }
      if (f) forge = f;
      if (forge && forge.running) { forgeTimer = setTimeout(tick, 2000); paint(); return; }
      if (forge && forge.last && forge.last.seq > seq0) {
        forgeTimer = null;
        say(forge.last.message, forge.last.ok ? "ok" : "bad");
        paint();
        return;
      }
      forgeTimer = setTimeout(tick, 2000);   // closed, the ground still being taken in
    };
    forgeTimer = setTimeout(tick, 2000);
  }
  function groundTile() {
    const box = mk("div", "");
    const installed = !!(forge && forge.installed);
    const running = !!(forge && forge.running);
    // TODO(owner): the button words and the hints, his words
    const label = running ? "Forge is open…" : forgeBusy === "get" ? "Getting Forge…" : installed ? "Open in Forge" : "Get Forge";
    const b = btn(label, "btn small primary", installed ? openForge : getForge);
    b.id = "yd-forge";
    b.disabled = running || !!forgeBusy || !forge;
    const row = mk("div", "yd-row");
    row.append(b);
    box.append(row);
    if (forgeGpl && !installed) {
      const n = mk("div", "yd-share");
      n.id = "yd-forge-gpl";
      n.append(mk("p", "", (forge && forge.licence) || "Forge is free software under the GNU GPL-3.0."),
        mk("p", "yd-hint", "Its source is public, and you may share and change it under the same licence."));
      const r = mk("div", "yd-row");
      const go = btn("Get Forge", "btn small primary", getForge); go.id = "yd-forge-go";
      const src = btn("Source", "btn small", () => call("open_link", { name: "forge-source" }).catch(() => say((forge && forge.source) || "", "ok")));
      src.id = "yd-forge-src";
      r.append(go, src, btn("Not now", "btn small", () => { forgeGpl = false; paint(); }));
      n.append(r);
      box.append(n);
    }
    if (forge && forge.last && forge.last.message) {
      const note = mk("p", "yd-hint", forge.last.message);
      note.id = "yd-forge-note";
      box.append(note);
    }
    return tile("Shape the ground",
      installed ? "Raise hills, paint, dig a pond, plant trees: Forge opens on your Den. Save and close it to bring your changes in."
        : "Forge, a free terrain editor, shapes your Den's ground. One download; your own game files stay on this PC.", box);
  }

  // ---------------------------------------------------------------- the one click
  // First time: Den Lite straight in (or, without it, the Docker Den's steps here). After that: start it if it's
  // stopped, then play. Never fails without saying why.
  async function go() {
    if (busy) return;
    if (!st) await load();
    const c = own();
    if (!c) {
      if (typeof addGameFolder === "function") addGameFolder("Your Den plays with your own game folder. Add it, then press Your Den again.", BUILD);
      say("Your Den plays with your own game folder. Add it on the Game folders page, then press Your Den again.", "warn", BUILD);
      return;
    }
    const w = where();
    if (w === "lite") return playLite(c);
    if (w === "docker") return playDocker();
    sharing = true;   // no Den Lite on this PC: the Docker Den, set up here
    paint();
    say("Your Den needs a one-time setup on this PC: three steps below.", "warn");
    loadSetup();
  }

  // My Den (app.rs den_play_lite): Den Lite started on this PC, the game pointed at it and started, then signed in
  // and taken into the world by itself (r.auto): Leyline's login screen code where the game takes it, else the
  // launcher's own keys (denclick.rs). Only where neither can (not Windows), the login screen has the account
  // filled in and wants the local password (a test login that never leaves this PC). No internet needed.
  async function playLite(c) {
    busy = true; paint();
    say(liteUp() ? "Opening your Den…" : liteGet() ? "Getting your Den ready… (a one-time download of a few MB)" : "Starting your Den… (a few seconds)");
    try {
      if (window.LeylineAddons) await window.LeylineAddons.beforePlay(c, { offer: false });   // the Den bar is in the Leyline addons (addons.js)
      const r = await call("den_play_lite", { client: c.id });
      if (window.LeylineAddons) window.LeylineAddons.afterPlay();
      if (r && typeof r === "object" && r.auto) say("Your Den is opening. The game signs you in and takes you straight to your Den.", "ok");
      else {
        const acct = (r && r.account) || (st && st.lite && st.lite.account) || "DEN";
        const pass = (r && r.password) || (st && st.lite && st.lite.password) || "den";
        say(`Your Den is open. At the login screen: account ${acct}, password ${pass}. Press Login.`, "ok");
        // TODO(owner): the words
        if (r && typeof r === "object" && r.why) hint = "On Windows the launcher signs you in by itself.";
      }
    } catch (e) { say(oops(e, "Your Den didn't open. Try again."), "bad"); }
    busy = false;
    // the play bar: the core pointed the game at Your Den, so it shows Your Den now (smoke O3)
    if (window.LeylineApp && window.LeylineApp.refresh) window.LeylineApp.refresh();
    load();
  }

  async function playDocker() {
    const c = own();
    busy = true; paint();
    try {
      if (!dockerUp()) {
        say("Starting your Den… The servers take a minute or two.");
        await call("host_choose", { profile: "den", clientId: c ? c.id : null }).catch(() => null);
        await call("host_start_quiet", { answers: { name: null, visibility: "private", gm: null } });
        job = { idle: false, kind: "host", running: true, code: null, step: "Getting ready", n: 0, of: 8, secs: 0, problem: null };
        followJob(true);
        return;
      }
      if (c) { try { await call("den_prefill_account", { client: c.id }); } catch (_) { /* the login screen asks then */ } }
      if (typeof playLocal === "function" && typeof DEN !== "undefined") {
        say("Opening your Den…");
        await playLocal(DEN);
        say(st.account ? `At the login screen your account ${st.account} is filled in: type your password.` : "", "ok");
      } else say("Pick Your Den in the play bar, then PLAY.", "warn");
    } catch (e) { say(oops(e, "Your Den didn't start. Try again."), "bad"); }
    busy = false;
    paint();
  }

  // ---------------------------------------------------------------- the Docker Den's steps (Share with friends)
  async function loadSetup() {
    const [p, d] = await Promise.all([call("hostpack_info").catch(() => null), call("docker_info").catch(() => null)]);
    pack = p; dock = d;
    paint();
  }
  async function getPack(b) {
    b.disabled = true; b.textContent = "Downloading…";
    try {
      const r = await call("hostpack_install");
      await call("set_host_kit", { path: r[1] });
      if (typeof readKit === "function") await readKit();
      say("The Host pack is ready.", "ok");
    } catch (e) { say(oops(e, "The Host pack didn't download. Try again."), "bad"); }
    await load(); await loadSetup();
  }
  async function startDocker(b) {
    b.disabled = true;
    try { say(await call("docker_start") || "Starting Docker… This usually takes under a minute."); }
    catch (e) { say(oops(e, "Docker did not start. Open Docker Desktop yourself: this page sees it."), "warn"); }
    for (let i = 0; i < 40; i++) {
      await new Promise((r) => setTimeout(r, 3000));
      try { dock = await call("docker_info"); } catch (_) { /* again */ }
      if (dock && dock.state === "ready") break;
    }
    say(dock && dock.state === "ready" ? "Docker is running." : "Docker hasn't answered yet. Look at Docker Desktop's window, then press Check again.",
      dock && dock.state === "ready" ? "ok" : "warn");
    paint();
  }
  async function startDen(input, b) {
    const acct = (input.value || "").trim();
    if (!ACCT_OK.test(acct)) { say("Your account name: 3 to 16 letters or digits. You make it on your Den's join page next.", "warn"); input.focus(); return; }
    store.set(ACCOUNT_KEY, acct);
    b.disabled = true;
    try {
      const moved = st && st.lite && st.lite.available ? await call("den_share_move") : null;
      store.set(PENDING, moved || "forest-glade");   // the same Den, unchanged (or the first starter place for a new one)
      const c = own();
      await call("host_choose", { profile: "den", clientId: c ? c.id : null });
      await call("host_start_quiet", { answers: { name: "My Den", visibility: "private", gm: acct } });
      job = { idle: false, kind: "host", running: true, code: null, step: "Getting ready", n: 0, of: 8, secs: 0, problem: null };
      followJob(false);
    } catch (e) { say(oops(e, "Your Den didn't start. Try again."), "bad"); b.disabled = false; }
  }
  async function followJob(thenPlay) {
    if (jobTimer) clearTimeout(jobTimer);
    try { const p = await call("host_progress"); if (p) job = p; } catch (_) { /* again */ }
    paint();
    if (job && job.running) { jobTimer = setTimeout(() => followJob(thenPlay), 2000); return; }
    jobTimer = null;
    busy = false;
    if (job && job.code === 0) {
      const name = store.get(PENDING);
      if (name) { store.set(PENDING, null); try { await call("den_open", { name }); } catch (_) { /* in game: .den load */ } }
      sharing = false;
      if (typeof readKit === "function") await readKit();
      await load();
      say(thenPlay ? "Your Den is up." : "Your Den is up and ready for friends: make your account on its join page, then Invite a friend.", "ok");
      if (!thenPlay && window.LeylineGuide) window.LeylineGuide.moment("hosted");   // your Den, live for friends: one line, once (guide.js)
      if (thenPlay) playDocker();
    } else if (job && !job.idle) say(job.problem || "Setting up stopped. Try again.", "bad");
  }
  async function joinPage(b) {
    b.disabled = true;
    try { await call("den_join_page"); say("Your Den's join page is open in your browser: make your account there, then come back and Play.", "ok"); }
    catch (e) { say(oops(e, "Copy the invite link and open it in your browser instead."), "warn"); }
    b.disabled = false;
  }
  async function copyInvite() {
    let link = null;
    try { link = await call("host_invite"); } catch (_) { /* no kit */ }
    if (!link) { say("Your Den has no invite link yet: start it once first.", "warn"); return; }
    try { await navigator.clipboard.writeText(link); say("Invite link copied. Send it to a friend: they open it once and make their own account.", "ok"); }
    catch (_) { say(link, "ok"); }
  }

  // Invite friends: the Docker Den copies its invite link. Den Lite (app.rs den_invite, host/litenet.rs): your Den
  // goes on the Leyline network from this PC at once (private: only people with a link get in; the relay only
  // passes the game's connection through), and the link for one friend, with their own login in it, is copied.
  let net = null;   // den_net_state
  let readyBox = null;   // Ready for friends? (hostfriends.js checklist, app.rs host_ready): the checks after an invite
  async function inviteLite() {
    busy = true; paint();
    say("Putting your Den on the Leyline network… (a few seconds)");
    try {
      const r = await call("den_invite", { name: null });
      net = r.tunnel;
      invited = r;
      // the way a friend comes in, checked line by line (each red line with its fix)
      if (window.LeylineHostFriends) readyBox = window.LeylineHostFriends.checklist({ kind: "lite", id: "yd-ready", prefix: "yd-c-", quietWithout: true, onStart: () => inviteLite() });
      let copied = true;
      try { await navigator.clipboard.writeText(r.link); } catch (_) { copied = false; }
      // TODO(owner): the words
      say(copied ? `Invite link copied: send it to one friend. In their Leyline they paste it (or the code below) under Realms › Got an invite link?, press Join, and play as ${r.name}. It works for 24 hours (Keep makes it last).`
        : `Send this link to one friend: ${r.link}`, "ok");
    } catch (e) { say(oops(e, "Your Den couldn't go on the network. Try again."), "bad"); }
    busy = false;
    paint();
    followNet();
    loadFriends();
  }
  let netTimer = null;
  function followNet() {
    if (netTimer) return;
    const tick = async () => {
      try { net = await call("den_net_state"); } catch (_) { /* as it was */ }
      paint();
      netTimer = net && net.running ? setTimeout(tick, 3000) : null;
    };
    netTimer = setTimeout(tick, 1500);
  }
  async function stopSharing() {
    try { await call("den_net_stop"); net = null; invited = null; readyBox = null; say("Your Den is off the network, and your friends' links stopped working.", "ok"); }
    catch (e) { say(oops(e), "bad"); }
    await loadFriends();
    paint();
  }

  // ---------------------------------------------------------------- the Friends list (host/denhost.rs)
  // read when the page opens and every few seconds while it is open and the Den runs (Den Lite rewrites who is in
  // within a second of a change)
  async function loadFriends() {
    let f = null;
    try { f = await call("den_friends"); } catch (_) { f = null; /* an older core: no list */ }
    const before = JSON.stringify(friends);
    friends = f;
    if (JSON.stringify(friends) !== before) paint();
    if (friendsTimer) clearTimeout(friendsTimer);
    friendsTimer = friends && (friends.running || friends.sharing) && page.classList.contains("on") ? setTimeout(loadFriends, 3000) : null;
  }
  async function friendDo(action, who, on, label) {
    friendBusy = who + ":" + action; paint();
    try { say(await call("den_friend", { action, who, on: !!on }) || "Done.", "ok"); }
    catch (e) { say(oops(e, `${label || "That"} didn't work. Try again.`), "bad"); }
    friendBusy = "";
    await loadFriends();
    paint();
  }
  const sinceText = (t) => {
    const m = Math.max(0, Math.round((Date.now() / 1000 - (t || 0)) / 60));
    return !t ? "" : m < 1 ? "just came in" : m < 60 ? `in for ${m} min` : `in for ${Math.round(m / 60)} h`;
  };
  const untilText = (t) => {
    const h = Math.round(((t || 0) - Date.now() / 1000) / 3600);
    return h <= 0 ? "ends soon" : h < 48 ? `works for ${h} more hour${h === 1 ? "" : "s"}` : `works for ${Math.round(h / 24)} more days`;
  };
  // one friend's row: their name and state, then Build, Keep and Kick (or Take back for a link not in use)
  function friendRow(f, inv, inNow) {
    const li = mk("li", "yd-friend" + (inNow ? " in" : ""));
    const who = (inv && inv.user) || f.user || f.name;
    li.dataset.who = f.name;
    const t = mk("div", "yd-li-t");
    const builds = inNow ? !!f.builds : !!(inv && inv.build);
    const keep = inNow ? !!f.keep : !!(inv && inv.keep);
    t.append(mk("b", "", f.name), mk("span", "yd-hint", [inNow ? sinceText(f.since) : "not in now", builds ? "can build" : "looks around",
      keep ? "kept" : inv ? untilText(inv.expires) : ""].filter(Boolean).join(" · ")));
    const acts = mk("div", "yd-row");
    const busyHere = (a) => friendBusy === who + ":" + a;
    const b = btn(builds ? "Can build" : "Let build", "btn small yd-f-build" + (builds ? " on" : ""), () => friendDo("build", who, !builds, "Build"));
    b.title = builds ? "They build with you. Press to make them a visitor again." : "Let this friend build in your Den (kept for next time).";
    b.setAttribute("aria-pressed", String(builds));
    const k = btn(keep ? "Kept" : "Keep", "btn small yd-f-keep" + (keep ? " on" : ""), () => friendDo("keep", who, !keep, "Keep"));
    k.title = keep ? "Their link works for a week from their last visit." : "Keep this friend: their link keeps working (a week from each visit) instead of 24 hours.";
    k.setAttribute("aria-pressed", String(keep));
    const x = btn(inNow ? "Kick" : "Take back", "btn small yd-f-kick", () => friendDo("kick", who, true, inNow ? "Kick" : "Take back"));
    x.title = inNow ? "Out of your Den now; their link stops working." : "Their link stops working.";
    for (const [el, a] of [[b, "build"], [k, "keep"], [x, "kick"]]) if (busyHere(a)) el.disabled = true;
    acts.append(b, k, x);
    li.append(t, acts);
    return li;
  }
  function friendsBox() {
    const box = mk("div", "yd-friends");
    box.id = "yd-friends-list";
    const f = friends || { online: { players: [] }, invites: [] };
    const here = ((f.online && f.online.players) || []).filter((p) => !p.host);
    const invites = f.invites || [];
    const head = mk("div", "yd-row yd-friends-head");
    head.append(mk("b", "", here.length ? `In your Den now: ${here.length}` : "Nobody is in yet"));
    const all = !!(f.online && f.online.all_build);
    if (f.running) {
      const ab = btn(all ? "Everyone builds" : "Let everyone build", "btn small yd-f-all" + (all ? " on" : ""), () => friendDo("build", "all", !all, "Build"));
      ab.id = "yd-f-all";
      ab.setAttribute("aria-pressed", String(all));
      ab.title = all ? "Every friend builds now (until your Den restarts). Press for only the ones you picked." : "Every friend in your Den may build, until your Den restarts.";
      head.append(ab);
    }
    box.append(head);
    const ul = mk("ul", "yd-list");
    ul.id = "yd-friends-in";
    for (const p of here) ul.append(friendRow(p, invites.find((i) => i.user === p.user) || null, true));
    // the links that still work for friends who aren't in
    for (const i of invites.filter((i) => !here.some((p) => p.user === i.user))) ul.append(friendRow({ name: i.name, user: i.user }, i, false));
    if (!ul.children.length) ul.append(mk("li", "yd-hint", "Invite a friend: they show here when they come in, with Build, Keep and Kick."));
    box.append(ul);
    const tn = f.online && f.online.tourney;
    if (tn && tn.title) {
      const tl = mk("p", "yd-hint yd-f-tourney");
      tl.id = "yd-f-tourney";
      // TODO(owner): the words
      tl.textContent = tn.champion ? `${tn.title}: ${tn.champion} is the champion.` : `${tn.title}: ${tn.players} signed up (${tn.phase === "signup" ? "sign-ups open" : "under way"}). Run it in your game: the Tournament button on the Den bar.`;
      box.append(tl);
    }
    return box;
  }
  // the invite code (the same way in as the link, to paste in Realms › Got an invite link?)
  function codeRow() {
    const r = mk("div", "yd-row yd-code-row");
    const ci = mk("input", "");
    ci.id = "yd-code"; ci.readOnly = true; ci.value = invited.code; ci.setAttribute("aria-label", "The invite code");
    const cc = btn("Copy code", "btn small", async () => {
      try { await navigator.clipboard.writeText(invited.code); say("Invite code copied: your friend pastes it in Leyline under Realms › Got an invite link?", "ok"); } catch (_) { ci.select(); }
    });
    cc.id = "yd-code-copy";
    r.append(ci, cc);
    return r;
  }
  // the ways in the link (app.rs InviteWay): same network, your own address, the relay, in the order a friend's
  // Leyline tries them; the link to copy again; your own address (optional)
  function waysBox() {
    const box = mk("div", "yd-ways");
    box.id = "yd-ways";
    box.append(mk("b", "", "How your friend gets in"));
    const ol = mk("ol", "yd-way-list");
    for (const w of invited.ways || []) {
      const li = mk("li", "yd-way " + (w.ok ? "on" : "off"), w.line);
      li.id = "yd-way-" + w.id;
      ol.append(li);
    }
    box.append(ol);
    // TODO(owner): the words
    box.append(mk("p", "yd-hint", "Their Leyline tries these in this order and tells them which one worked. Your friend needs Leyline, their own game folder and a Hero Key. Game files never travel: each of you plays with your own. The Den invite guide has every step."));
    const lr = mk("div", "yd-row");
    const li = mk("input", "");
    li.id = "yd-link"; li.readOnly = true; li.value = invited.link; li.setAttribute("aria-label", "The invite link");
    const cp = btn("Copy", "btn small", async () => {
      try { await navigator.clipboard.writeText(invited.link); say("Invite link copied again.", "ok"); } catch (_) { li.select(); }
    });
    cp.id = "yd-link-copy";
    lr.append(li, cp);
    const or = mk("div", "yd-row");
    const oi = mk("input", "");
    oi.id = "yd-own"; oi.maxLength = 260; oi.placeholder = "Your own address (optional): den.example.org";
    oi.setAttribute("aria-label", "Your own address for friends");
    const own = (invited.ways || []).find((w) => w.id === "own");
    if (own && own.ok) oi.value = (own.line.match(/^Your own address: (\S+)/) || [])[1] || "";
    const ob = btn("Save", "btn small", async () => {
      try {
        const h = await call("den_own_address", { host: oi.value.trim() });
        say(h ? `Your own address is ${h}. Press Invite friends again for a link that has it.` : "Your own address is taken out.", "ok");
      } catch (e) { say(oops(e, "That address couldn't be kept."), "bad"); }
    });
    ob.id = "yd-own-save";
    or.append(oi, ob);
    box.append(lr, or);
    return box;
  }

  // ---------------------------------------------------------------- My models (host/denmodels.rs)
  const MODEL_EXT = /\.(m2|skin|blp|wmo)$/i;
  const MODEL_MB = { m2: 16, skin: 4, blp: 8, wmo: 16 };
  const modelIn = mk("input", "");
  modelIn.type = "file"; modelIn.multiple = true; modelIn.accept = ".m2,.skin,.blp,.wmo"; modelIn.hidden = true; modelIn.id = "yd-model-files";
  modelIn.addEventListener("change", () => { const f = Array.from(modelIn.files || []); modelIn.value = ""; addModels(f); });
  async function asBase64(f) {
    const b = new Uint8Array(await f.arrayBuffer());
    let s = "";
    for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode.apply(null, b.subarray(i, i + 0x8000));
    return btoa(s);
  }
  // the same limits as host/denfiles.rs, said before anything is read
  function modelProblem(list) {
    if (!list.length) return "Drop a model's files: an .m2 with its .skin and .blp files, or a .wmo with its groups.";
    if (list.length > 80) return "Up to 80 files at once.";
    for (const f of list) {
      const m = f.name.match(MODEL_EXT);
      if (!m) return `File ${f.name}: only .m2, .skin, .blp and .wmo files (your own model's files).`;
      if (f.size > MODEL_MB[m[1].toLowerCase()] * 1024 * 1024) return `File ${f.name}: too big (${MODEL_MB[m[1].toLowerCase()]} MB at most).`;
    }
    if (list.reduce((n, f) => n + f.size, 0) > 48 * 1024 * 1024) return "That's more than 48 MB at once.";
    if (!list.some((f) => /\.(m2|wmo)$/i.test(f.name))) return "There's no model in that: drop the .m2 (or the building's .wmo) with its files.";
    return "";
  }
  async function addModels(files) {
    const list = Array.from(files || []);
    const bad = modelProblem(list);
    if (bad) { say(bad, "warn"); return; }
    modelsBusy = true; paint();
    say("Adding your model…");
    try {
      const payload = [];
      for (const f of list) payload.push({ name: f.name, data: await asBase64(f) });
      const m = await call("den_models_add", { files: payload, label: modelName.trim() || null });
      const miss = (m.missing || []).length;
      // TODO(owner): the words
      say(`${m.name} is in your Den's gallery under My models ${liteUp() ? "after you close your game and press Open my Den again" : "the next time you press Open my Den"}.`
        + (miss ? ` ${miss} texture${miss === 1 ? "" : "s"} it names weren't in your drop: they stay as they are (your game may have them).` : ""), "ok");
      modelName = "";
    } catch (e) { say(oops(e, "That model couldn't be added."), "bad"); }
    try { models = await call("den_models"); } catch (_) { /* as it was */ }
    modelsBusy = false;
    paint();
  }
  async function removeModel(m) {
    try { await call("den_models_remove", { slug: m.slug }); say(`${m.name} is out of your Den's gallery (from the next Open my Den).`, "ok"); }
    catch (e) { say(oops(e), "bad"); }
    try { models = await call("den_models"); } catch (_) { /* as it was */ }
    paint();
  }
  function modelsTile() {
    const box = mk("div", "");
    const drop = mk("div", "yd-drop" + (modelsBusy ? " busy" : ""));
    drop.id = "yd-models-drop";
    drop.tabIndex = 0;
    drop.setAttribute("role", "button");
    drop.append(mk("b", "", modelsBusy ? "Adding…" : "Drop your own model files here, or click to choose them"),
      mk("span", "yd-hint", "An .m2 with its .skin and .blp files, or a building's .wmo with its group files and .blp files."));
    drop.addEventListener("dragover", (e) => { e.preventDefault(); drop.classList.add("over"); });
    drop.addEventListener("dragleave", () => drop.classList.remove("over"));
    drop.addEventListener("drop", (e) => { e.preventDefault(); drop.classList.remove("over"); if (!modelsBusy) addModels(e.dataTransfer && e.dataTransfer.files); });
    drop.addEventListener("click", () => { if (!modelsBusy) modelIn.click(); });
    drop.addEventListener("keydown", (e) => { if ((e.key === "Enter" || e.key === " ") && !modelsBusy) { e.preventDefault(); modelIn.click(); } });
    const nm = mk("input", "");
    nm.id = "yd-model-name"; nm.maxLength = 32; nm.placeholder = "Its name in the gallery (optional)"; nm.value = modelName;
    nm.setAttribute("aria-label", "The model's name in the gallery");
    nm.addEventListener("input", () => { modelName = nm.value; });
    box.append(drop, nm, modelIn);
    const ul = mk("ul", "yd-list");
    ul.id = "yd-models";
    for (const m of models || []) {
      const li = mk("li", "");
      const t = mk("div", "yd-li-t");
      t.append(mk("b", "", m.name), mk("span", "yd-hint", [m.kind === "wmo" ? "Building" : "Model", `${Math.max(1, Math.round((m.bytes || 0) / 1024))} KB`,
        (m.missing || []).length ? `${m.missing.length} texture name${m.missing.length === 1 ? "" : "s"} from your game` : ""].filter(Boolean).join(" · ")));
      li.append(t, btn("Remove", "btn small", () => removeModel(m)));
      ul.append(li);
    }
    box.append(ul);
    // TODO(owner): the words
    const note = mk("p", "yd-hint yd-models-note", "Your files stay on this PC: nothing is uploaded or shared. A friend who joins sees a plain stand-in where each model stands, because their game hasn't your files.");
    note.id = "yd-models-note";
    box.append(note);
    return tile("My models", "Your own 3D models as props: in the Builder's gallery under My models.", box, "wide");
  }

  function inviteFriends() {
    if (where() === "docker") { copyInvite(); return; }
    if (where() === "lite") { inviteLite(); return; }
    sharing = true;
    paint();
    loadSetup();
    say("Friends join your Den on a server on this PC: a one-time setup, three steps below. Then Invite friends copies your link.", "warn");
    const steps = root.querySelector(".yd-steps");
    if (steps && steps.scrollIntoView) steps.scrollIntoView({ block: "nearest" });
  }
  // Ask Claude to edit it: the Agents page, where Claude is connected once; its Den tools (agents/den.rs) then read
  // and build in your Den, every change after your OK
  function askClaude() {
    if (window.LeylineNav) window.LeylineNav.show("agents");
  }

  // ---------------------------------------------------------------- Saved Dens (the same for both)
  const denDate = (s) => { const t = Date.parse(s || ""); return isNaN(t) ? "" : new Date(t).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }); };
  const fileIn = mk("input", "");
  fileIn.type = "file"; fileIn.accept = ".json,application/json"; fileIn.hidden = true;
  fileIn.addEventListener("change", async () => {
    const f = fileIn.files && fileIn.files[0];
    fileIn.value = "";
    if (!f) return;
    if (f.size > 512 * 1024) { say("That file is too big for a Den (512 KB at most).", "warn"); return; }
    try { say(`Added as '${await call("den_add", { fileName: f.name, text: await f.text() })}'.`, "ok"); }
    catch (e) { say(oops(e, "That isn't a Den file."), "bad"); }
    load();
  });
  async function openDen(f, b) {
    b.disabled = true;
    try { await call("den_open", { name: f.name }); say(`'${f.name}' opens when you're in your Den (a copy of what was there is kept as 'backup').`, "ok"); }
    catch (e) { say(oops(e), "bad"); }
    b.disabled = false;
  }
  async function saveCopy(f) {
    let text;
    try { text = await call("den_text", { name: f.name }); } catch (e) { say(oops(e), "bad"); return false; }
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([text], { type: "application/json" }));
    a.download = `${f.name}.den.json`;
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 5000);
    say(`Saved a copy: ${f.name}.den.json, in your Downloads.`, "ok");
    return true;
  }
  function fromLink() {
    const box = mk("div", "yd-link");
    const u = mk("input", "");
    u.type = "url"; u.maxLength = 400; u.placeholder = "https://github.com/someone/their-den/releases/download/v1/garden.den.json";
    u.setAttribute("aria-label", "A shared Den's link");
    u.spellcheck = false;
    const add = btn("Add", "btn small primary", async () => {
      const url = u.value.trim();
      if (!/^https:\/\/github\.com\/.+\.den\.json$/i.test(url)) { say("Paste the link to a Den file on a GitHub release (it ends in .den.json).", "warn"); return; }
      add.disabled = true;
      try { say(`Added as '${await call("den_fetch", { url, sha256: null })}'. Open it to step into it.`, "ok"); u.value = ""; }
      catch (e) { say(oops(e, "That Den file couldn't be added."), "bad"); }
      add.disabled = false;
      load();
    });
    box.append(u, add);
    return box;
  }

  // ---------------------------------------------------------------- Den codes and Sky Island (denpack.js)
  // A code a friend sent (or a leyline://pack/ link, or a .denpack file): decoded here, checked by the core as any Den
  // file (den_add), and by Den Lite again when it opens. A tale's code becomes a small Den file Den Lite's
  // .den tale list reads.
  async function addCode(text, open) {
    const P = window.LeylineDenPack;
    if (!P) { say("Den codes need a newer launcher.", "warn"); return null; }
    let d;
    try { d = await P.decode(text); } catch (e) { say(e.message || String(e), "warn"); return null; }
    const f = P.asDenFile(d.kind, d.text);
    let name;
    try { name = await call("den_add", { fileName: f.fileName, text: f.text }); }
    catch (e) { say(oops(e, "That code doesn't hold a Den Leyline can open."), "bad"); return null; }
    if (d.kind === "tale") say(`The tale is in your Dens as '${name}'. In your Den: .den tale play ${name}`, "ok");
    else if (open) {
      try { await call("den_open", { name }); say(`'${name}' opens when you're in your Den (a copy of what was there is kept as 'backup').`, "ok"); }
      catch (e) { say(oops(e), "bad"); }
    } else say(`Added as '${name}'. Open it to step into it.`, "ok");
    load();
    return name;
  }
  function fromCode() {
    const box = mk("div", "yd-link yd-code");
    const u = mk("input", "");
    u.type = "text"; u.maxLength = 100000; u.placeholder = "ley1.den.z.… (a Den code a friend sent, or a leyline://pack/ link)";
    u.setAttribute("aria-label", "A Den code");
    u.spellcheck = false;
    u.id = "yd-code";
    // a leyline://pack/ link that came before this page was drawn (app.js handleDeepLink)
    if (window.LeylinePendingCode) { u.value = String(window.LeylinePendingCode); window.LeylinePendingCode = null; }
    const add = btn("Open the code", "btn small primary", async () => {
      add.disabled = true;
      if (await addCode(u.value, true)) u.value = "";
      add.disabled = false;
    });
    const pick = mk("input", "");
    pick.type = "file"; pick.accept = ".denpack,.txt"; pick.hidden = true;
    pick.addEventListener("change", async () => {
      const f = pick.files && pick.files[0];
      pick.value = "";
      if (f && f.size < 100 * 1024) await addCode(await f.text(), false);
      else if (f) say("That code file is too big.", "warn");
    });
    box.append(u, add, btn("A .denpack file", "btn small", () => pick.click()), pick);
    return box;
  }
  // Sky Island (owner 5 Nov): a floating island over open sky, a big tree, dummies to spar with, the whole editor, its
  // welcome tale; one click: the island's file in your Dens, then Your Den, and Den Lite floats it when you're in
  async function skyIsland(b) {
    if (b) b.disabled = true;
    const name = await addCode(window.LeylineDenPack ? window.LeylineDenPack.ISLAND_CODE : "", true);
    if (b) b.disabled = false;
    if (name) go();
  }
  function islandTile() {
    const b = mk("div", "");
    const play = btn("Play Sky Island", "btn primary", () => skyIsland(play));
    play.id = "yd-sky";
    const copy = btn("Copy its code", "btn small", async () => {
      try { await navigator.clipboard.writeText((window.LeylineDenPack.LINK) + window.LeylineDenPack.ISLAND_CODE); say("Sky Island's link is copied: paste it to a friend.", "ok"); }
      catch (_) { say("Couldn't copy here.", "warn"); }
    });
    b.append(play, copy, mk("p", "yd-hint", "In the game: .den island (and .den island off), .den pack makes your Den a code, .den tale for its tales."));
    // TODO(owner): the tile's words
    return tile("Sky Island", "A floating island in open sky: build, spar, play its tale, pass it on as one code. Your own game's models, nothing to download.", b);
  }

  // ---------------------------------------------------------------- Share on the Workshop
  function workshop(f) {
    const d = mk("div", "yd-share");
    // Shared Dens (denmarket.js, docs/DEN-MARKETPLACE.md): one step, pictures from the game, a moderator's look
    if (window.LeylineDenMarket) {
      const one = btn("Share it on Leyline", "btn small primary", () => window.LeylineDenMarket.share(f.name));
      one.className = "btn small primary yd-share-dm";
      d.append(mk("p", "", `Share '${f.name}' with every Leyline player: a title, pictures your game takes, and a moderator's look.`), one,
        mk("p", "yd-hint", "Or on your own GitHub, for the Workshop:"));
    }
    d.append(mk("p", "", `To share '${f.name}' on the Workshop:`));
    const ol = mk("ol", "yd-steps-s");
    ol.append(mk("li", "", "Save a copy (below): the file lands in your Downloads."),
      mk("li", "", "On GitHub, make a release on any repo of yours and attach that .den.json file."),
      mk("li", "", "List it: paste the release link. Leyline checks the Den file, then it is on the Workshop for anyone to add."));
    const row = mk("div", "yd-row");
    row.append(btn("Save a copy", "btn small", () => saveCopy(f)),
      btn("List it on the Workshop", "btn small primary", () => {
        if (window.LeylineListMod && window.LeylineListMod.open) window.LeylineListMod.open("den");
        else say("List your mod isn't on this page.", "warn");
      }));
    d.append(ol, row);
    return d;
  }

  // ---------------------------------------------------------------- painting
  function tile(title, hint, body, cls) {
    const t = mk("section", "yd-tile" + (cls ? " " + cls : ""));
    t.append(mk("h3", "", title), mk("p", "yd-hint", hint));
    if (body) t.append(body);
    return t;
  }
  function stateLine() {
    const w = where();
    if (w === "lite") return liteUp() ? ["ok", "Open on this PC"] : ["", "Ready on this PC: no setup needed"];
    if (w === "docker") return dockerUp() ? ["ok", "Online on this PC · private · friends with your invite link"] : ["", "Stopped: Play starts it"];
    return ["warn", "Not set up yet: Play sets it up"];
  }
  function paintSteps(box) {
    const haveKit = !!(st && st.kit);
    const dockOk = !!(dock && dock.state === "ready");
    const step = (n, done, title, line, action) => {
      const r = mk("div", "yd-step" + (done ? " done" : ""));
      r.append(mk("span", "yd-step-n", done ? "✓" : String(n)));
      const t = mk("div", "yd-step-t");
      t.append(mk("b", "", title), mk("span", "", line));
      r.append(t);
      if (action && !done) r.append(action);
      box.append(r);
    };
    let kb = null;
    if (!haveKit) { kb = btn("Get the Host pack", "btn small primary"); kb.addEventListener("click", () => getPack(kb)); if (pack && !pack.available && !pack.installed) kb.disabled = true; }
    step(1, haveKit, "The Host pack", haveKit ? "Installed." : "One download: the Den's servers and tools. No game files.", kb);
    let db = null;
    if (dock && dock.can_start) { db = btn("Start Docker", "btn small primary"); db.addEventListener("click", () => startDocker(db)); }
    else if (!dockOk) db = btn(dock && dock.state === "missing" ? "Get Docker Desktop" : "Check again", "btn small",
      () => (dock && dock.state === "missing" ? call("open_link", { name: "docker" }).catch(() => {}) : loadSetup()));
    step(2, dockOk, "Docker Desktop", dockOk ? "Running." : (dock && dock.help) || "Free. It runs your Den's servers so friends can join. Install it, open it once, then Check again.", db);
    const r = mk("div", "yd-step-go");
    const inp = mk("input", "");
    inp.id = "yd-acct"; inp.maxLength = 16; inp.placeholder = "Your account name"; inp.value = store.get(ACCOUNT_KEY) || "";
    inp.setAttribute("aria-label", "Your account name in your Den");
    const sb = btn(liteHas() ? "Move my Den and start" : "Start my Den", "btn small primary");
    sb.id = "yd-start";
    sb.disabled = !(haveKit && dockOk) || !!(job && job.running);
    sb.addEventListener("click", () => startDen(inp, sb));
    r.append(inp, sb);
    step(3, false, "Start", liteHas() ? "Your Den moves over as it is. The first start takes 5 to 20 minutes." : "The first start takes 5 to 20 minutes.", r);
  }
  function paintJob(box) {
    if (!job || job.idle) return;
    const p = mk("div", "yd-prog" + (!job.running && job.code !== 0 ? " bad" : ""));
    p.append(mk("b", "", job.running ? job.step : job.code === 0 ? "Set up" : "Setting up stopped"),
      mk("span", "yd-hint", job.running ? `Step ${Math.max(1, job.n)} of ${job.of}. You can use the rest of Leyline meanwhile.` : (job.problem || "")));
    const bar = mk("div", "yd-bar");
    const fill = mk("i", "");
    fill.style.width = `${Math.round((job.running ? Math.max(job.n - 0.5, 0.3) : job.of) / Math.max(job.of, 1) * 100)}%`;
    bar.append(fill);
    bar.setAttribute("role", "progressbar");
    bar.setAttribute("aria-valuemin", "0"); bar.setAttribute("aria-valuemax", String(job.of)); bar.setAttribute("aria-valuenow", String(job.n));
    p.append(bar);
    box.append(p);
  }
  function paint() {
    root.replaceChildren();
    const head = mk("div", "yd-head");
    const h = mk("h2", "", "Your Den");
    const [kind, line] = stateLine();
    const chip = mk("span", "yd-chip " + kind, line);
    chip.id = "yd-state";
    head.append(h, chip);
    // "Open my Den", not "Play solo" (4 Nov: it clashed with Single player, the bots game on this PC)
    // TODO(owner): "Open my Den", his words
    const play = btn(busy ? "Opening…" : "Open my Den", "btn primary yd-play", go);
    play.id = "yd-play";
    play.disabled = busy;
    // the three things to do with your Den, one click each (owner 1 Nov: "invite friends, connect Claude to edit it")
    const invite = btn("Invite friends", "btn yd-top", inviteFriends);
    invite.id = "yd-friends";
    invite.title = where() === "new" ? "Friends join your Den on a server: a one-time setup on this PC"
      : "Copies a link for one friend: your Den goes on the Leyline network from this PC";
    // TODO(owner): the label and its tooltip, his words
    const claude = btn("Ask Claude to edit it", "btn yd-top", askClaude);
    claude.id = "yd-claude";
    claude.title = "Your own Claude builds in your Den with you: connect it once on the Agents page. Every change waits for your OK.";
    const acts = mk("div", "yd-row yd-acts");
    acts.append(play, invite, claude);
    root.append(head, mk("p", "lede", "Just you, on this PC: no internet or sign-in needed. A quiet green world to build in, with your companion to help. Invite friends when you like."),
      acts, mk("p", "yd-hint yd-play-hint", where() === "lite" ? "Open my Den starts your own server on this PC, signs you in and takes you into your Den, with your look and addons." : where() === "docker" ? "Open my Den starts it if it's stopped, then plays." : "Open my Den sets it up once, then plays."), status);
    if (hint) { const hl = mk("p", "yd-hint", hint); hl.id = "yd-login-hint"; root.append(hl); }
    // on the network (Den Lite's tunnel): one line, and Stop sharing
    if (net && net.running) {
      const nl = mk("div", "yd-row yd-net");
      nl.id = "yd-net";
      nl.append(mk("span", "yd-hint", net.connected ? `On the Leyline network, private: friends with your link can join${net.streams ? ` (${Math.ceil(net.streams / 2)} playing)` : ""}.`
        : net.last_error ? `Not reaching the network yet: ${net.last_error}` : "Connecting to the Leyline network…"));
      const sb = btn("Stop sharing", "btn small", stopSharing); sb.id = "yd-net-stop";
      nl.append(sb);
      root.append(nl);
    }
    if (invited && where() === "lite") root.append(waysBox());
    // the bigger way to play alone, only where Docker runs (or Single player is already hosted here)
    const soloHosted = typeof kit !== "undefined" && kit && kit.realm && kit.realm.profile === "solo";
    if (soloHosted || (dock && dock.state === "ready")) {
      const sl = mk("div", "yd-row yd-solo");
      sl.id = "yd-solo";
      const b = btn(soloHosted ? "Play Single player" : "Single player", "btn small", soloLeveling);
      b.id = "yd-solo-go";
      // TODO(owner): the words
      sl.append(mk("span", "yd-hint", soloHosted ? "Level from 1 on your own realm on this PC, with helper bots."
        : "Want to level from 1 instead? Single player is a full realm on this PC with helper bots and dungeons sized for you. It uses Docker; the first start takes 5 to 20 minutes."), b);
      root.append(sl);
    }
    const grid = mk("div", "yd-grid");
    // Share with friends (Den Lite) / Invite a friend (the Docker Den)
    if (where() === "docker") {
      const b = mk("div", "yd-row");
      const inv = btn("Invite a friend", "btn small primary", copyInvite); inv.id = "yd-invite";
      const jp = btn("Make my account", "btn small"); jp.addEventListener("click", () => joinPage(jp));
      b.append(inv, jp);
      grid.append(tile("Friends", "Copies your invite link. Friends open it once and make their own account.", b));
    } else if (where() === "lite") {
      // no Docker (host/denhost.rs): the relay, a link and a code per friend, the Friends list
      const sb = mk("div", "");
      const row1 = mk("div", "yd-row");
      const inv = btn(busy ? "Opening…" : "Invite a friend", "btn small primary", inviteLite);
      inv.id = "yd-share-invite";
      inv.disabled = busy;
      inv.title = "Copies a link for one friend: your Den goes on the Leyline network from this PC. No Docker, no router settings.";
      row1.append(inv);
      sb.append(row1);
      if (invited && invited.code) sb.append(codeRow());
      if (invited && readyBox) sb.append(readyBox);
      sb.append(friendsBox());
      // the bigger server, for a Den that should run without this PC's Den Lite (the Host pack, Docker)
      const more = mk("div", "yd-row yd-share-more");
      const open = btn(sharing ? "Hide the steps" : "Move to a full server…", "link-s", () => { sharing = !sharing; paint(); if (sharing) loadSetup(); });
      open.id = "yd-share";
      open.title = "Moves this Den to the full Den server on this PC (needs Docker Desktop): combat, duels with ratings, more players.";
      more.append(open);
      sb.append(more);
      if (sharing) { const steps = mk("div", "yd-steps"); paintSteps(steps); paintJob(steps); sb.append(steps); }
      // TODO(owner): the hint, his words
      grid.append(tile("Share with friends", "Friends join this Den through the Leyline relay: no Docker, no port forwarding, and your home address is never shown.", sb, "wide"));
    } else {
      const sb = mk("div", "");
      const open = btn(sharing ? "Hide the steps" : "Share with friends", "btn small" + (sharing ? "" : " primary"), () => {
        sharing = !sharing; paint(); if (sharing) loadSetup();
      });
      open.id = "yd-share";
      sb.append(open);
      if (sharing) { const steps = mk("div", "yd-steps"); paintSteps(steps); paintJob(steps); sb.append(steps); }
      grid.append(tile("Share with friends", "Your Den on a server friends can join. Needs Docker Desktop.", sb, sharing ? "wide" : ""));
    }
    // Saved Dens
    const sd = mk("div", "");
    const list = (st && st.dens) || [];
    if (!list.length) sd.append(mk("p", "yd-hint", "No saved Dens yet. In your Den, type .den save and a name."));
    const ul = mk("ul", "yd-list");
    ul.id = "yd-dens";
    for (const f of list) {
      const li = mk("li", "");
      const t = mk("div", "yd-li-t");
      t.append(mk("b", "", f.name === "backup" ? "backup (before the last open)" : f.name === "autosave" ? "autosave (your Den as you left it)" : f.name),
        mk("span", "yd-hint", [`${f.props} prop${f.props === 1 ? "" : "s"}`, denDate(f.saved)].filter(Boolean).join(" · ")));
      // the last duel tournament fought in it (labs/den-lite tourney.rs; host/dens.rs champion_line)
      if (f.champion) { const c = mk("span", "yd-hint yd-champ", "Champion: " + f.champion); t.append(c); }
      const acts = mk("div", "yd-row");
      const o = btn("Open", "btn small primary"); o.addEventListener("click", () => openDen(f, o));
      acts.append(o, btn("Save a copy", "btn small", () => saveCopy(f)), btn("Share", "btn small", () => {
        const old = li.querySelector(".yd-share");
        if (old) old.remove(); else li.append(workshop(f));
      }));
      li.append(t, acts);
      ul.append(li);
    }
    sd.append(ul);
    const addRow = mk("div", "yd-row");
    addRow.append(btn("Add a Den file", "btn small", () => fileIn.click()), fileIn);
    sd.append(addRow, fromLink(), fromCode());
    grid.append(tile("Saved Dens", "Open one to step into it. Add a file a friend sent, a Den code, or a link from the Workshop.", sd, "wide"));
    if (where() === "lite") grid.append(islandTile());
    // Packs
    const pk = mk("div", "");
    const packs = (st && st.packs) || [];
    if (!packs.length) pk.append(mk("p", "yd-hint", "No packs yet. Packs you build from the game's places show here."));
    const pl = mk("ul", "yd-list");
    pl.id = "yd-packs";
    for (const p of packs) {
      const li = mk("li", "");
      const t = mk("div", "yd-li-t");
      t.append(mk("b", "", p.title), mk("span", "yd-hint", p.line || ""));
      li.append(t);
      pl.append(li);
    }
    pk.append(pl);
    grid.append(tile("Packs", "Ready-made places to build around.", pk));
    if (where() === "lite") grid.append(modelsTile());
    grid.append(groundTile());
    // Share on the Workshop
    const ws = mk("div", "");
    ws.append(btn("Share on the Workshop", "btn small", () => {
      if (!list.length) { say("Save your Den first: in your Den, type .den save and a name.", "warn"); return; }
      const li = ul.querySelector("li");
      if (li && !li.querySelector(".yd-share")) li.append(workshop(list[0]));
      ul.scrollIntoView({ block: "nearest" });
    }));
    grid.append(tile("Share on the Workshop", "Put a saved Den on the Workshop for anyone to add.", ws));
    root.append(grid);
    paintCard();
  }

  // ---------------------------------------------------------------- Single player's Your Den card
  // The same Den, from Single player's page: one click in (go: Den Lite started, the game signed in and taken into
  // your Den behind Leyline's cover), Invite a friend (inviteFriends: the link copied; a Den not set up yet opens its
  // steps on the Den page), its state, the last line said, and Stop sharing. Everything else is on the Den page.
  function paintCard() {
    if (!spd) return;
    const [kind, line] = stateLine();
    const head = mk("div", "spd-head");
    const chip = mk("span", "yd-chip " + kind, line);
    chip.id = "spd-state";
    head.append(mk("h2", "", "Your Den"), chip);
    // TODO(owner): the words
    const lede = mk("p", "spd-lede", "Your own private room on this PC to build in, with your companion. One click takes you in, no internet needed, and a friend can join with your link.");
    const acts = mk("div", "yd-row spd-acts");
    const play = btn(busy ? "Opening…" : "Open my Den", "btn primary", () => go());
    play.id = "spd-play";
    play.disabled = busy;
    const inv = btn("Invite a friend", "btn", () => {
      if (where() === "new" && window.LeylineNav) window.LeylineNav.show("den");   // its one-time steps are there
      inviteFriends();
    });
    inv.id = "spd-invite";
    inv.disabled = busy;
    inv.title = where() === "new" ? "Friends join your Den on a server: a one-time setup on this PC"
      : "Copies a link for one friend: your Den goes on the Leyline network from this PC";
    const more = btn("More on Your Den", "link-s", () => { if (window.LeylineNav) window.LeylineNav.show("den"); });
    more.id = "spd-more";
    more.title = "Saved Dens, packs, Shape the ground, your friends and the Workshop";
    acts.append(play, inv, more);
    const said = status.cloneNode(true);
    said.id = "spd-status";
    said.className = "status spd-status";
    const parts = [head, lede, acts, said];
    if (net && net.running) {
      const nl = mk("div", "yd-row spd-net");
      nl.id = "spd-net";
      nl.append(mk("span", "yd-hint", net.connected ? `On the Leyline network, private: friends with your link can join${net.streams ? ` (${Math.ceil(net.streams / 2)} playing)` : ""}.`
        : net.last_error ? `Not reaching the network yet: ${net.last_error}` : "Connecting to the Leyline network…"));
      const sb = btn("Stop sharing", "btn small", stopSharing);
      sb.id = "spd-net-stop";
      nl.append(sb);
      parts.push(nl);
    }
    spd.replaceChildren(...parts);
  }
  // read again when Single player's page opens (quietly: a Den that isn't answering keeps its last state)
  async function loadCard() {
    try { st = await call("den_state"); } catch (_) { /* as it was */ }
    try { net = await call("den_net_state"); } catch (_) { /* as it was */ }
    paint();
    if (net && net.running) followNet();
  }

  // ---------------------------------------------------------------- the entries: the side menu, Home, and this page
  document.addEventListener("leyline:page", (e) => {
    if (e.detail && e.detail.page === "den") load();
    else if (e.detail && e.detail.page === "single" && spd) loadCard();
  });
  const homeGo = byId("hm-den-go");
  if (homeGo) homeGo.addEventListener("click", () => { if (window.LeylineNav) window.LeylineNav.show("den"); go(); });
  window.LeylineDen = { go, load, state: () => st, where, addCode, skyIsland, invite: inviteFriends, card: paintCard };
  // Your Den's state read once at the start too (not only on its page): the play bar's PLAY and the realm picker go by it
  if (page.classList.contains("on")) load(); else { paint(); call("den_state").then((x) => { if (!st) { st = x; paint(); } }).catch(() => { /* asked again on go() */ }); }
})();
