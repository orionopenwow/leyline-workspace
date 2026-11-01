// Leyline Launcher: Your Den (docs/DEN.md "Your Den"). One button, on Home and in the side menu, into your own white room
// to build in; one page for everything about it, each thing one click with a one-line hint.
//
// Where the Den runs (host/yourden.rs, host/lite.rs):
//   * Den Lite, the built-in Den server: no Docker, straight into the room. Only when the program is on this PC
//     (den_state().lite.available); else the Docker Den below is the way in.
//   * the Docker Den (the Host pack's "den" profile): what friends join through the Leyline network. "Share with friends"
//     moves the Lite Den there (den_share_move: the same files, the newest opened there unchanged) through three steps:
//     the Host pack, Docker Desktop, Start. Then Invite a friend.
// Saved Dens, Packs and Share on the Workshop work the same for both (den_text / den_add / den_open / den_fetch on
// whichever Den is in use). Playing the Docker Den is app.js's own (playLocal(DEN): the relay's knock, then PLAY), with
// its account filled in on the login screen (den_prefill_account).
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

  const status = mk("p", "status yd-status");
  status.setAttribute("role", "status");
  function say(text, kind) { status.textContent = text || ""; status.dataset.kind = kind || ""; }

  // ---------------------------------------------------------------- what this PC has
  const own = () => (typeof clientForRealm === "function" ? clientForRealm("den", BUILD) : null);
  const liteOn = () => !!(st && st.lite && st.lite.available);
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
    paint();
  }

  // ---------------------------------------------------------------- the one click
  // First time: Den Lite straight in (or, without it, the Docker Den's steps here). After that: start it if it's
  // stopped, then play. Never fails without saying why.
  async function go() {
    if (busy) return;
    if (!st) await load();
    const c = own();
    if (!c) {
      if (typeof addGameFolder === "function") addGameFolder(`Your Den plays with your own ${BUILD} game folder. Add it, then press Your Den again.`);
      say(`Your Den plays with your own ${BUILD} game folder. Add it on the Clients page, then press Your Den again.`, "warn");
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

  async function playLite(c) {
    busy = true; paint();
    say(liteUp() ? "Opening your Den…" : "Starting your Den… (a few seconds)");
    try {
      await call("den_play_lite", { client: c.id });
      say(`Your Den is open. At the login screen the account is ${st.lite.account || "filled in"}: press Login.`, "ok");
    } catch (e) { say(oops(e, "Your Den didn't open. Try again."), "bad"); }
    busy = false;
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
      } else say("Pick My Den in the play bar, then PLAY.", "warn");
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

  // ---------------------------------------------------------------- Share on the Workshop
  function workshop(f) {
    const d = mk("div", "yd-share");
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
    if (w === "lite") return liteUp() ? ["ok", "Open on this PC (Den Lite)"] : ["", "Ready on this PC: no setup needed"];
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
    const sb = btn(liteOn() ? "Move my Den and start" : "Start my Den", "btn small primary");
    sb.id = "yd-start";
    sb.disabled = !(haveKit && dockOk) || !!(job && job.running);
    sb.addEventListener("click", () => startDen(inp, sb));
    r.append(inp, sb);
    step(3, false, "Start", liteOn() ? "Your Den moves over as it is. The first start takes 5 to 20 minutes." : "The first start takes 5 to 20 minutes.", r);
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
    const play = btn(busy ? "Opening…" : "Play", "btn primary yd-play", go);
    play.id = "yd-play";
    play.disabled = busy;
    root.append(head, mk("p", "lede", "A white room to build in, on this PC. Place anything, save it to a file, invite friends when you like."),
      mk("div", "yd-row", undefined), status);
    root.children[2].append(play, mk("span", "yd-hint", where() === "lite" ? "Straight into your room." : where() === "docker" ? "Starts it if it's stopped, then plays." : "Sets it up once, then plays."));
    const grid = mk("div", "yd-grid");
    // Share with friends (Den Lite) / Invite a friend (the Docker Den)
    if (where() === "docker") {
      const b = mk("div", "yd-row");
      const inv = btn("Invite a friend", "btn small primary", copyInvite); inv.id = "yd-invite";
      const jp = btn("Make my account", "btn small"); jp.addEventListener("click", () => joinPage(jp));
      b.append(inv, jp);
      grid.append(tile("Friends", "Copies your invite link. Friends open it once and make their own account.", b));
    } else {
      const sb = mk("div", "");
      const open = btn(sharing ? "Hide the steps" : "Share with friends", "btn small" + (sharing ? "" : " primary"), () => {
        sharing = !sharing; paint(); if (sharing) loadSetup();
      });
      open.id = "yd-share";
      sb.append(open);
      if (sharing) { const steps = mk("div", "yd-steps"); paintSteps(steps); paintJob(steps); sb.append(steps); }
      grid.append(tile("Share with friends", liteOn() ? "Moves this Den to a server friends can join. Needs Docker Desktop." : "Your Den on a server friends can join. Needs Docker Desktop.", sb, sharing ? "wide" : ""));
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
      t.append(mk("b", "", f.name === "backup" ? "backup (before the last open)" : f.name),
        mk("span", "yd-hint", [`${f.props} prop${f.props === 1 ? "" : "s"}`, denDate(f.saved)].filter(Boolean).join(" · ")));
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
    sd.append(addRow, fromLink());
    grid.append(tile("Saved Dens", "Open one to step into it. Add a file a friend sent, or a link from the Workshop.", sd, "wide"));
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
  }

  // ---------------------------------------------------------------- the entries: the side menu, Home, and this page
  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "den") load(); });
  const homeGo = byId("hm-den-go");
  if (homeGo) homeGo.addEventListener("click", () => { if (window.LeylineNav) window.LeylineNav.show("den"); go(); });
  const homeOpen = byId("hm-den-page");
  if (homeOpen) homeOpen.addEventListener("click", () => window.LeylineNav && window.LeylineNav.show("den"));
  window.LeylineDen = { go, load, state: () => st, where };
  if (page.classList.contains("on")) load(); else paint();
})();
