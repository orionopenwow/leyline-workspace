// Leyline: the host console. The Host page once a realm is set up on this PC, after the website's "Your realm console"
// (stream/hub.py host_showcase): the realm's name and state, six service tiles, the GM controls, the health check, the
// realm tools and the cores it runs on. Before a realm is set up: the setup start (a ruleset, or Link my server) and the
// guide. Commands: src-tauri/src/host/console.rs (through p2p/cmds.rs) plus the Host page's own (app.js, host.js).
// Leyline makes no accounts and handles no passwords: friends make their own on the realm's join page.
// Loaded after app.js and host.js; app.js sends "leyline:kit" each time it reads the kit.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  // never a raw error to the player (friendly.js): the error goes to the console, the player gets a plain line
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (fb || "Something went wrong. Try again."));
  const byId = (id) => document.getElementById(id);
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const btn = (label, cls, on) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (on) b.addEventListener("click", on); return b; };
  const page = byId("page-host");
  const main = byId("hc-pane-main");
  const guidePane = byId("hc-pane-guide");
  if (!page || !main || !guidePane) return;

  // ---------------------------------------------------------------- our own shapes (as on the website: no game emblem)
  const studs = Array.from({ length: 8 }, (_, k) =>
    `<circle cx="${(24 + 19.5 * Math.cos(k * Math.PI / 4)).toFixed(2)}" cy="${(24 + 19.5 * Math.sin(k * Math.PI / 4)).toFixed(2)}" r="1.3"/>`).join("");
  const CREST = `<svg class="hc-crest" viewBox="0 0 48 48" width="44" height="44" aria-hidden="true" focusable="false">
    <circle cx="24" cy="24" r="22" class="r1"/><circle cx="24" cy="24" r="17" class="r2"/><g class="st">${studs}</g>
    <g transform="translate(13.6 9.6) scale(.65)"><polygon points="16,1.5 9,11 16,30.5" fill="#c9b2ff"/><polygon points="16,1.5 23,11 16,30.5" fill="#8b6cf0"/>
    <polygon points="9,11 16,30.5 5.5,13" fill="#5b3fc4"/><polygon points="23,11 16,30.5 26.5,13" fill="#4a2fa8"/>
    <polygon points="16,1.5 9,11 5.5,13" fill="#b9d0ff"/><polygon points="16,1.5 23,11 26.5,13" fill="#7aa3f5"/></g></svg>`;
  const RUNE = '<svg viewBox="0 0 64 14" width="64" height="14" focusable="false"><circle cx="3" cy="7" r="1.6" class="dt"/>'
    + '<path d="M14 7l3-3 3 3-3 3z" class="dm s"/><path d="M32 1l6 6-6 6-6-6z" class="dm"/><path d="M44 7l3-3 3 3-3 3z" class="dm s"/>'
    + '<circle cx="61" cy="7" r="1.6" class="dt"/></svg>';
  const rune = () => { const d = mk("div", "hc-rune"); d.setAttribute("aria-hidden", "true"); d.innerHTML = RUNE; return d; };
  const crest = () => { const s = mk("span", "hc-crest-w"); s.innerHTML = CREST; return s; };
  const CHEV = '<svg class="hc-chev" viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"><path d="M3 4.5l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>';

  // ---------------------------------------------------------------- the words (the website's console, HOST_* in hub.py)
  const SERVICES = [["database", "Database"], ["login", "Login server"], ["world", "World server"], ["agent", "Relay agent"]];
  // the same words as the in-game Your realm window and Leyline HQ: a ban is the player's whole ACCOUNT
  const GM = [
    ["announce", "Announce", "to everyone online"],
    ["notify", "On-screen notice", "in the middle of everyone's screen"],
    ["motd", "Message of the day", "shown at login"],
    ["kick", "Kick", "one player"],
    ["mute", "Mute or unmute", "no chat for a while"],
    ["rescue", "Send to Dalaran", "a stuck player"],
    ["ban", "Ban account", "the player's whole account"],
    ["allow", "Allow-list", "only listed characters"],
    ["restart", "Restart", "with a five-minute warning"],
  ];
  // how long (hours; 0 = permanent) and the mute's minutes: HQ's limits (stream/hq.py build)
  const BAN_FOR = [["24", "1 day"], ["72", "3 days"], ["168", "1 week"], ["720", "30 days"], ["0", "Permanent"]];
  const MUTE_FOR = [["15", "15 minutes"], ["60", "1 hour"], ["1440", "1 day"], ["10080", "1 week"]];
  const CHECKS = [["pc", "The servers on your PC"], ["tunnel", "The tunnel to the relay"], ["login", "A login through the relay"],
    ["invite", "Your invite link"], ["list", "Your place on the realm list"]];
  const TOOLS = [["invite", "Invite links"], ["friends", "Accounts for friends"], ["vis", "Public or private"],
    ["signup", "Who may sign up"], ["browser", "Browser players"], ["max", "Max players"], ["logs", "Server logs"], ["rules", "House rules"],
    ["dens", "Saved Dens"]];
  // tools that change a setting of the kit's own realm: not for a linked server (its own settings), browser only on AzerothCore
  // and a Den has no player limit to set (the kit sets 10 for it, docs/DEN.md)
  const toolShown = (k, c) => !(c.linked && (k === "browser" || k === "max" || k === "signup")) && !(k === "browser" && (c.core || "azerothcore") !== "azerothcore")
    && !(k === "max" && LOCAL_IDS.includes(c.profile)) && !(k === "dens" && c.profile !== DEN);
  // the cores the host pack runs (stream/hub.py HOST_CORES): one click only for what the kit hosts today
  const CORES = [["azerothcore", "AzerothCore", "One click"], ["cmangos", "CMaNGOS", "One click"], ["trinitycore", "TrinityCore", "Coming"],
    ["vmangos", "vMaNGOS", "Coming"], ["own", "Your own", "Relay agent"]];
  // the rulesets, in plain words (host/profiles.rs PROFILES; the kit's own presets)
  const RULESETS = {
    vanillaplus: ["Vanilla+", "The original world to 60, made to stay up.", "Always on"],
    den: ["My Den", "A private room to build in: you and the friends you invite. No bots.", "Private"],
    solo: ["Solo leveling", "Just you, levelling from 1 at 2x. Helper bots fill your group; dungeons scale to you.", "Private"],
    dalaran: ["Dalaran hangout", "Light: a one-time boost, no bots."],
    main: ["Classic levelling", "From level 1 at 3x, or a one-time boost."],
    brawl: ["Brawl", "Everyone starts at the top level."],
    progression: ["Progression", "1x from level 1, era by era."],
    "cmangos-1.12.1": ["True Vanilla", "A plain realm on CMaNGOS, for the original client."],
    "cmangos-2.4.3": ["Second era", "A plain realm on CMaNGOS."],
    "tcpp-4.3.4": ["Fourth era", "A plain realm on TrinityCore."],
  };
  const ORDER = ["vanillaplus", "den", "solo", "dalaran", "main", "progression", "brawl", "cmangos-1.12.1", "cmangos-2.4.3", "tcpp-4.3.4"];
  // Make my Den (docs/DEN.md): the kit's "den" profile, private and sign-up from the invite link only, never bots, at
  // most 10 players (the kit writes those; host/profiles.rs starts it with --private). No bots or max-players choice.
  const DEN = "den";
  const DEN_FACTS = [["Private", "only friends with your invite link"], ["Sign-up", "from your invite link only"], ["No bots", "just you and your friends"]];
  // Solo leveling (realm/profiles/solo.env, owner 2026-10-31): private like a Den, helper bots on, at most 5 players
  const SOLO = "solo";
  const SOLO_FACTS = [["Private", "just you, on this PC"], ["Helper bots", "type .recruit party to fill your group"], ["Dungeons", "scaled to the group you bring"]];
  const FACTS = { den: DEN_FACTS, solo: SOLO_FACTS };
  // the realms made for this PC alone (the play bar's rows): always private. One Host pack hosts one realm, so these
  // two take turns on it (switchTo: the same realm, its accounts and characters kept)
  const LOCAL_IDS = [DEN, SOLO];
  // the guide: short how-tos, each with its page on the website (lib.rs LINKS: hostdocs, alwayson)
  const GUIDE = [
    ["network", "How the Leyline network works", "Players reach your realm through the Leyline relay, by its realm id: nobody sees your address and you open no ports. Public realms are listed; private ones take an invite link. Leyline never ships game files.", "network"],
    ["start", "Start your realm", "Pick a ruleset and press Start. A window opens: leave it running. The first start imports the databases, 5 to 20 minutes.", "hostdocs"],
    ["stop", "Stop it", "Stop keeps characters, accounts and your invite link. Start again any time; it picks up where it was.", "hostdocs"],
    ["update", "Update", "When a new Host pack is out, update it under Host pack & settings, then press Start again. Your realm and its links are kept.", "hostdocs"],
    ["public", "Go public", "Realm tools, Public or private. A public realm is on the Leyline realm list within a minute; a private one only takes your invite link.", "hostdocs"],
    ["invite", "Invite friends", "Realm tools, Invite links: copy the link and send it. Friends open it once and make their own account there.", "hostdocs"],
    ["claim", "Claim it with your Hero Key", "A listed realm shows as Unclaimed until you claim it. Get a claim token on the website and paste it under Public or private.", "alwayson"],
    ["alwayson", "Always on", "Vanilla+ starts always on: Docker brings it back after a reboot. With Docker Desktop, turn on its start at sign-in and keep the PC awake.", "alwayson"],
    ["link", "Link a server you already run", "Any core, here or elsewhere: Link my server runs only the relay agent next to it and never changes your server.", "alwayson"],
    ["backups", "Backups", "Stop the realm, then copy the Host pack folder: state holds your realm's key and links. Characters live in Docker's wu-database volume.", "hostdocs"],
    ["trouble", "When something's wrong", "Run the health check: each line says what to fix. Server logs are under Realm tools.", "hostdocs"],
  ];

  // ---------------------------------------------------------------- tabs
  const tabs = [...page.querySelectorAll(".page-tab[data-hc]")];
  function tab(name) {
    for (const b of tabs) { const on = b.dataset.hc === name; b.setAttribute("aria-selected", String(on)); b.tabIndex = on ? 0 : -1; }
    for (const p of page.querySelectorAll(".hc-pane")) p.classList.toggle("on", p.id === "hc-pane-" + name);
  }
  tabs.forEach((b) => {
    b.addEventListener("click", () => tab(b.dataset.hc));
    b.addEventListener("keydown", (e) => {
      const i = tabs.indexOf(b);
      const j = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : null;
      if (j === null) return;
      e.preventDefault();
      const t = tabs[(j + tabs.length) % tabs.length];
      tab(t.dataset.hc); t.focus();
    });
  });

  // ---------------------------------------------------------------- the confirm (kick, ban, restart, stop, new link)
  function ask(title, body, yes) {
    const d = byId("hc-confirm");
    return new Promise((resolve) => {
      if (!d || !d.showModal) { resolve(window.confirm(`${title}\n\n${body}`)); return; }
      byId("hc-confirm-title").textContent = title;
      byId("hc-confirm-body").textContent = body;
      byId("hc-confirm-yes").textContent = yes;
      const done = (v) => { if (d.open) d.close(); resolve(v); };
      byId("hc-confirm-yes").onclick = () => done(true);
      byId("hc-confirm-no").onclick = () => done(false);
      d.oncancel = (ev) => { ev.preventDefault(); done(false); };
      d.showModal();
      byId("hc-confirm-no").focus();
    });
  }

  // ---------------------------------------------------------------- the guide (its own tab, and beside the setup start)
  function guideList(compact) {
    const box = mk("div", "hc-guide" + (compact ? " compact" : ""));
    if (!compact) box.append(mk("p", "hc-lede hc-guide-lede", "Short answers. Each links to its full page on the website."));
    for (const [id, title, body, link] of GUIDE) {
      const d = mk("details", "hc-g");
      d.dataset.topic = id;
      const s = mk("summary", "", title);
      s.insertAdjacentHTML("beforeend", CHEV);
      const p = mk("p", "", body);
      const more = btn("More on the website", "hc-more", () => call("open_link", { name: link }).catch(() => {}));
      more.dataset.link = link;
      d.append(s, p, more);
      box.append(d);
    }
    return box;
  }
  guidePane.append(guideList(false));

  // ---------------------------------------------------------------- state
  let kit = null;
  let con = null;
  let stat = null;
  let statErr = null;
  let gmStat = null;
  let mode = null;            // "console" | "setup"
  let autoTab = true;
  let openTool = null;
  let timer = null;
  let askedFor = null;        // a ruleset asked for from elsewhere (the play bar's My Den / Solo leveling): picked once offered
  let askedHow = {};          // and how: { switch: from the other local realm, start: it didn't answer, why }
  const ui = {};

  const say = (text, kind) => { if (ui.status) { ui.status.textContent = text || ""; ui.status.className = "status hc-status" + (kind ? " " + kind : ""); } };
  const isOn = () => page.classList.contains("on");

  document.addEventListener("leyline:kit", (e) => {
    kit = e.detail || null;
    const want = kit && kit.realm ? "console" : "setup";
    if (want !== mode) { mode = want; build(); if (autoTab) { tab("main"); autoTab = false; } }
    if (mode === "console") { refresh(); applyWant(); } else loadSetup();
  });
  // the play bar's My Den / Solo leveling not on this PC: the Host page with that ruleset picked; the other one on
  // this PC: switch to it; it didn't answer: say so beside Start
  document.addEventListener("leyline:host-ruleset", (e) => {
    askedFor = (e.detail && e.detail.profile) || null;
    askedHow = (e.detail && typeof e.detail === "object" && e.detail) || {};
    tab("main");
    applyWant();
  });
  const rsName = (id) => (RULESETS[id] || [id])[0];
  function applyWant() {
    if (!askedFor || !mode) return;
    const id = askedFor;
    const how = askedHow || {};
    if (mode === "console") {
      askedFor = null;
      const cur = kit && kit.realm;
      const name = (cur && cur.name) || "a realm";
      if (cur && cur.profile === id) {
        const running = !!(con && (con.services || []).some((x) => x.role === "world" && (x.state === "ok" || x.state === "starting")));
        if (running) say(`${rsName(id)} is running. If PLAY didn't get in, try again in a minute.`, "warn");
        else { say(`${how.why ? how.why + " " : ""}Press Start to start ${rsName(id)}.`, "warn"); if (ui.start) ui.start.focus(); }
        return;
      }
      if (cur && LOCAL_IDS.includes(cur.profile) && LOCAL_IDS.includes(id)) { switchTo(id, cur); return; }
      if (LOCAL_IDS.includes(id)) say(`This PC runs ${name}, and one PC hosts one realm at a time, so ${rsName(id)} can't run beside it.`, "warn");
      return;
    }
    if (!offers) return;    // loadSetup calls again once the rulesets are in
    askedFor = null;
    const o = offers.find((x) => x.id === id);
    if (!o) { say(`Your Host pack has no ${rsName(id)} yet: update it under Host pack & settings.`, "warn"); return; }
    if (o.note) { say(o.note, "warn"); return; }
    pick(id).then(() => {
      const b = ui.rules && ui.rules.querySelector(`.hc-rs[data-profile="${id}"]`);
      if (b) { b.scrollIntoView({ block: "nearest" }); b.focus(); }
    });
  }

  // My Den <-> Solo leveling on this PC's one realm: asked first, then the kit runs the other ruleset (no window)
  async function switchTo(id, cur) {
    const to = rsName(id), from = cur.name || rsName(cur.profile);
    if (!(await ask(`Switch to ${to}?`, `This PC runs one realm at a time, and it runs ${from} now. ${to} takes its place: your account and characters stay, and you can switch back any time. Make a new character for ${to}. It takes a few minutes.`, `Switch to ${to}`))) {
      say(`Kept ${from}.`);
      return;
    }
    say(`Switching to ${to}…`);
    try {
      const prof = await call("host_profiles").catch(() => null);
      const o = Array.isArray(prof) && Array.isArray(prof[0]) ? prof[0].find((x) => x && x.id === id) : null;
      await call("host_choose", { profile: id, clientId: o && o.clients && o.clients[0] ? o.clients[0].id : null });
      await call("host_start_quiet", { answers: { name: NAMES[id], visibility: "private", gm: null } });
      job = { idle: false, kind: "host", running: true, code: null, step: "Getting ready", n: 0, of: 8, secs: 0, problem: null };
      followJob();
    } catch (e) { say(oops(e), "bad"); }
  }

  function build() {
    main.replaceChildren();
    Object.keys(ui).forEach((k) => delete ui[k]);
    openTool = null;
    byId("hc-tab-main").textContent = mode === "console" ? "Console" : "Start";
    if (mode === "console") buildConsole(); else buildSetup();
  }

  // ================================================================ the console (a realm is set up)
  function buildConsole() {
    const sw = mk("section", "hc-sw");
    sw.setAttribute("aria-label", "Your realm console");
    // the header: crest, the realm's name, its state, start / stop
    const top = mk("div", "hc-top");
    const id = mk("div", "hc-id");
    ui.kicker = mk("p", "hc-k", "Your realm console");
    ui.name = mk("p", "hc-t", (kit.realm && kit.realm.name) || "Your realm");
    id.append(ui.kicker, ui.name);
    ui.badge = mk("p", "hc-run wait");
    ui.badge.setAttribute("role", "status");
    ui.badge.append(mk("span", "", ""), mk("b", "", "Checking…"));
    ui.badge.firstChild.setAttribute("aria-hidden", "true");
    ui.acts = mk("div", "hc-top-acts");
    ui.start = btn("Start", "btn small primary", startRealm);
    ui.stop = btn("Stop", "btn small", stopRealm);
    ui.refresh = btn("", "btn small hc-icon", () => refresh());
    ui.refresh.setAttribute("aria-label", "Refresh");
    ui.refresh.title = "Refresh";
    ui.refresh.innerHTML = '<svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2.5v3h-3" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>';
    // My Den / Solo leveling: play it from here too (app.js window.LeylineLocal: the invite knock, then PLAY)
    ui.play = btn("Play", "btn small primary", () => { if (con && window.LeylineLocal) window.LeylineLocal.play(con.profile); });
    ui.play.id = "hc-play";
    ui.play.hidden = true;
    ui.acts.append(ui.play, ui.start, ui.stop, ui.refresh);
    top.append(crest(), id, ui.badge, ui.acts);

    // the six tiles
    ui.svc = mk("ul", "hc-svc");
    ui.svc.setAttribute("aria-label", "Health");
    ui.tiles = {};
    for (const [k, label] of [...SERVICES, ["players", "Players"], ["uptime", "Uptime"]]) {
      const li = mk("li", "hc-tile off");
      li.dataset.svc = k;
      const dot = mk("i", "");
      dot.setAttribute("aria-hidden", "true");
      const v = mk("span", "", "–");
      li.append(dot, mk("b", "", label), v);
      ui.svc.append(li);
      ui.tiles[k] = { li, v };
    }

    // GM controls | health check + realm tools
    const cols = mk("div", "hc-cols");
    const gmBox = mk("div", "hc-box");
    gmBox.append(mk("p", "hc-bh", "GM controls"));
    ui.gmNote = mk("p", "hc-note");
    ui.gmNote.hidden = true;
    ui.gm = mk("ul", "hc-acts");
    for (const [a, label, sub] of GM) ui.gm.append(gmRow(a, label, sub));
    gmBox.append(ui.gmNote, ui.gm);

    const right = mk("div", "hc-box");
    const chHead = mk("div", "hc-bh-row");
    chHead.append(mk("p", "hc-bh", "Health check"));
    ui.run = btn("Run check", "btn small", runCheck);
    ui.run.id = "hc-run-check";
    chHead.append(ui.run);
    ui.chk = mk("ul", "hc-chk");
    ui.checks = {};
    for (const [k, label] of CHECKS) {
      const li = mk("li", "c-idle");
      li.dataset.check = k;
      const i = mk("i", "");
      i.setAttribute("aria-hidden", "true");
      const t = mk("span", "", label);
      const d = mk("small", "");
      d.hidden = true;
      li.append(i, t, d);
      ui.chk.append(li);
      ui.checks[k] = { li, d };
    }
    ui.chkSum = mk("p", "hc-sum");
    ui.chkSum.hidden = true;
    ui.tools = mk("ul", "hc-tools");
    for (const [k, label] of TOOLS) {
      const li = mk("li", "");
      const b = btn(label, "hc-tool", () => toggleTool(k));
      b.dataset.tool = k;
      b.setAttribute("aria-pressed", "false");
      li.append(b);
      ui.tools.append(li);
    }
    right.append(chHead, ui.chk, ui.chkSum, mk("p", "hc-bh hc-gap", "Realm tools"), ui.tools);
    cols.append(gmBox, right);

    ui.drawer = mk("div", "hc-drawer");
    ui.drawer.hidden = true;
    ui.status = mk("p", "status hc-status");
    ui.status.setAttribute("role", "status");
    // "You're live": once, right after a start from this page, when the relay sees the realm
    ui.live = mk("div", "hc-live");
    ui.live.hidden = true;
    ui.prog = mk("div", "hc-prog");
    ui.prog.hidden = true;
    sw.append(top, ui.live, ui.prog, rune(), ui.svc, cols, ui.drawer, ui.status, rune(), mk("p", "hc-bh", "Runs on"), coreChips());
    main.append(sw);
  }

  function coreChips(onOwn) {
    const ul = mk("ul", "hc-cores");
    ui.cores = {};
    for (const [id, name, tag] of CORES) {
      const li = mk("li", "hc-core" + (tag === "Coming" ? " soon" : "") + (id === "own" ? " own" : ""));
      li.dataset.core = id;
      const inner = id === "own" && onOwn ? btn("", "hc-core-b", onOwn) : mk("span", "hc-core-b");
      inner.append(mk("b", "", name), mk("span", "", tag));
      li.append(inner);
      ul.append(li);
      ui.cores[id] = li;
    }
    return ul;
  }

  // ---------------------------------------------------------------- GM rows: one line each, its form opens below it
  function gmRow(a, label, sub) {
    const li = mk("li", "hc-act");
    li.dataset.gm = a;
    const h = btn("", "hc-act-h");
    h.setAttribute("aria-expanded", "false");
    const s = mk("span", "", sub);
    h.append(mk("b", "", label), s);
    h.insertAdjacentHTML("beforeend", CHEV);
    const f = mk("div", "hc-act-f");
    f.hidden = true;
    h.addEventListener("click", () => {
      const open = f.hidden;
      for (const o of ui.gm.querySelectorAll(".hc-act")) { o.querySelector(".hc-act-f").hidden = true; o.querySelector(".hc-act-h").setAttribute("aria-expanded", "false"); o.classList.remove("open"); }
      f.hidden = !open;
      h.setAttribute("aria-expanded", String(open));
      li.classList.toggle("open", open);
      if (open) { const i = f.querySelector("input"); if (i) i.focus(); }
    });
    const input = (ph, max, cls) => { const i = mk("input", cls || ""); i.type = "text"; i.placeholder = ph; i.maxLength = max; i.spellcheck = false; i.autocomplete = "off"; return i; };
    const name = () => input("Character name", 12, "hc-name");
    const reason = () => input("Reason (the player sees it)", 160, "hc-reason");
    const choose = (list, label, pick) => {
      const sel = mk("select", "hc-hours");
      sel.setAttribute("aria-label", label);
      for (const [v, t] of list) { const o = mk("option", "", t); o.value = v; sel.append(o); }
      if (pick) sel.value = pick;
      return sel;
    };
    if (a === "announce" || a === "motd" || a === "notify") {
      const t = input(a === "announce" ? "Everyone online sees this" : a === "notify" ? "On everyone's screen, once" : "Shown to everyone at login", 200);
      const go = btn(a === "motd" ? "Set" : a === "notify" ? "Show" : "Send", "btn small primary", async () => {
        if (!t.value.trim()) { t.focus(); return; }
        if (await gm(a, t.value, go)) { if (a !== "motd") t.value = ""; }
      });
      t.addEventListener("keydown", (e) => { if (e.key === "Enter") go.click(); });
      f.append(t, go);
    } else if (a === "kick") {
      const n = name();
      const go = btn("Kick", "btn small danger", async () => {
        const who = n.value.trim();
        if (!who) { n.focus(); return; }
        if (await ask(`Kick ${who}?`, "They're sent off the realm now. They can log in again.", "Kick")) gm("kick", who, go);
      });
      f.append(n, go);
    } else if (a === "mute") {
      const n = name();
      const mins = choose(MUTE_FOR, "How long", "60");
      const why = reason();
      const go = btn("Mute", "btn small danger", () => {
        const who = n.value.trim();
        if (!who) { n.focus(); return; }
        if (why.value.trim().length < 3) { say("Say why (3 letters or more).", "warn"); why.focus(); return; }
        gm("mute", `${who}\t${mins.value}\t${why.value.trim()}`, go);
      });
      const un = btn("Unmute", "btn small", () => { const who = n.value.trim(); if (!who) { n.focus(); return; } gm("unmute", who, un); });
      f.append(n, mins, why, go, un);
    } else if (a === "rescue") {
      const n = name();
      const go = btn("Send to Dalaran", "btn small primary", () => { const who = n.value.trim(); if (!who) { n.focus(); return; } gm("rescue", who, go); });
      f.append(n, go);
    } else if (a === "ban") {
      const n = name();
      const hrs = choose(BAN_FOR, "How long");
      const why = reason();
      const go = btn("Ban account", "btn small danger", async () => {
        const who = n.value.trim();
        if (!who) { n.focus(); return; }
        if (why.value.trim().length < 3) { say("Say why (3 letters or more).", "warn"); why.focus(); return; }
        const long = hrs.value === "0" ? "for good" : `for ${hrs.options[hrs.selectedIndex].textContent}`;
        if (await ask(`Ban ${who}'s account ${long}?`, "Every character on their account is locked out of your realm until it ends. You can unban it any time.", "Ban account")) {
          gm("ban", `${who}\t${hrs.value}\t${why.value.trim()}`, go);
        }
      });
      const un = btn("Unban account", "btn small", () => { const who = n.value.trim(); if (!who) { n.focus(); return; } gm("unban", who, un); });
      f.append(n, hrs, why, go, un);
    } else if (a === "allow") {
      const n = name();
      const add = btn("Add", "btn small", () => { const who = n.value.trim(); if (!who) { n.focus(); return; } gm("allow", `add\t${who}`, add).then((ok) => { if (ok) n.value = ""; }); });
      const del = btn("Remove", "btn small", () => { const who = n.value.trim(); if (!who) { n.focus(); return; } gm("allow", `del\t${who}`, del); });
      const tog = btn("Turn on", "btn small", async () => {
        const on = !(gmStat && gmStat.allow === "on");
        if (on && !(await ask("Only listed characters?", "From now on, only characters on your list (and GMs) can log in. Others are told and sent off at login.", "Turn on"))) return;
        if (await gm("allow", on ? "on" : "off", tog)) gmStatus();
      });
      ui.allowToggle = tog;
      f.append(n, add, del, mk("span", "hc-sep"), tog);
    } else if (a === "restart") {
      const go = btn("Restart in 5 minutes", "btn small danger", async () => {
        if (await ask("Restart the realm?", "Everyone online is told now, and the realm restarts in five minutes. It's back a minute or two later.", "Restart in 5 minutes")) {
          if (await gm("restart", "", go)) gmStatus();
        }
      });
      const cancel = btn("Cancel restart", "btn small", async () => { if (await gm("cancel", "", cancel)) gmStatus(); });
      cancel.hidden = true;
      ui.restartGo = go;
      ui.restartCancel = cancel;
      f.append(mk("span", "hc-act-t", "Everyone online gets five minutes' warning."), go, cancel);
    }
    li.append(h, f);
    return li;
  }

  async function gm(action, data, b) {
    if (con && !con.gm) { say(con.gm_note || "Start your realm to use these.", "warn"); return false; }
    if (b) b.disabled = true;
    say("Sending to your realm…");
    try { say(await call("host_gm", { action, data }), "ok"); return true; }
    catch (e) { say(oops(e), "bad"); return false; }
    finally { if (b) b.disabled = false; }
  }

  // the GM controls' own state (restart coming, allow list, message of the day) from the realm itself
  async function gmStatus() {
    if (!con || !con.gm) return;
    let t;
    try { t = await call("host_gm", { action: "status", data: "" }); } catch (_) { return; }
    gmStat = {};
    for (const line of String(t).split("\n")) { const i = line.indexOf("="); if (i > 0) gmStat[line.slice(0, i)] = line.slice(i + 1); }
    const sub = (a, text) => { const r = ui.gm && ui.gm.querySelector(`[data-gm="${a}"] .hc-act-h span`); if (r) r.textContent = text; };
    if (gmStat.motd && gmStat.motd !== "-") sub("motd", `“${gmStat.motd.slice(0, 40)}”`);
    sub("allow", gmStat.allow === "on" ? `on · ${gmStat.allowed || 0} listed` : "only listed characters");
    if (ui.allowToggle) ui.allowToggle.textContent = gmStat.allow === "on" ? "Turn off" : "Turn on";
    const left = +gmStat.restart || 0;
    sub("restart", left > 0 ? `coming in ${Math.ceil(left / 60)} min` : "with a five-minute warning");
    if (ui.restartCancel) { ui.restartCancel.hidden = !(left > 0); ui.restartGo.hidden = left > 0; }
    if (gmStat.players && ui.tiles && !(stat && stat.players != null)) setTile("players", "ok", `${gmStat.players} online`);
  }

  // ---------------------------------------------------------------- data
  function setTile(k, state, text) {
    const t = ui.tiles && ui.tiles[k];
    if (!t) return;
    t.li.className = "hc-tile " + state;
    t.v.textContent = text;
  }
  function setBadge(cls, text) {
    if (!ui.badge) return;
    ui.badge.className = "hc-run " + cls;
    ui.badge.lastChild.textContent = text;
  }

  let busy = false;
  async function refresh(quiet) {
    if (mode !== "console" || busy) return;
    busy = true;
    const rid = kit && kit.realm && kit.realm.realm_id;
    try {
      const [c, s] = await Promise.all([
        call("host_console").catch((e) => { if (!quiet) say(oops(e), "bad"); return null; }),
        rid ? call("host_status", { realmId: rid }).then((x) => { statErr = null; return x; }).catch((e) => { statErr = String(e); return null; }) : null,
      ]);
      if (mode !== "console") return;
      con = c; stat = s;
      paint();
      if (con && con.gm && !gmStat) gmStatus();
    } finally { busy = false; }
  }

  function paint() {
    const c = con || { services: [], docker: false };
    if (c.name) ui.name.textContent = c.name;
    ui.kicker.textContent = c.linked ? "Your server, linked" : c.profile === DEN ? "Your Den" : c.profile === SOLO ? "Your solo realm" : "Your realm console";
    // the badge
    if (stat && stat.online) {
      setBadge("ok", stat.listed ? "Online · listed" : (stat.visibility === "private" || c.visibility === "private") ? "Online · private" : "Online");
    } else if (stat) setBadge("bad", "Offline");
    else if (statErr) setBadge("warn", "Not seen yet");
    else setBadge("wait", "Checking…");
    // the tiles
    for (const [k] of SERVICES) {
      const s = (c.services || []).find((x) => x.role === k);
      if (!s) { setTile(k, "off", "–"); continue; }
      let text = s.text;
      if (k === "agent" && s.state === "ok" && stat && stat.listed) text = "listed";
      setTile(k, s.state, text);
    }
    const players = stat && stat.players != null ? stat.players : null;
    setTile("players", stat && stat.online ? "ok" : "off", players != null ? `${players} online` : "–");
    setTile("uptime", c.uptime ? "ok" : "off", c.uptime || "–");
    // start or stop
    const running = (c.services || []).some((s) => s.state === "ok" || s.state === "starting");
    ui.start.hidden = running || c.linked;
    ui.stop.hidden = !running || c.linked;
    if (ui.play) { ui.play.hidden = !running || !LOCAL_IDS.includes(c.profile) || !window.LeylineLocal; ui.play.textContent = `Play ${rsName(c.profile)}`; }
    // GM controls
    ui.gmNote.hidden = !c.gm_note;
    ui.gmNote.textContent = c.gm_note || "";
    ui.gm.classList.toggle("off", !c.gm);
    for (const h of ui.gm.querySelectorAll(".hc-act-h")) h.disabled = !c.gm;
    if (!c.gm) for (const f of ui.gm.querySelectorAll(".hc-act-f")) f.hidden = true;
    // the cores: the one this realm runs
    for (const [id, li] of Object.entries(ui.cores || {})) {
      const cur = id === (c.core || "azerothcore");
      li.classList.toggle("cur", cur);
      if (cur) li.setAttribute("aria-current", "true"); else li.removeAttribute("aria-current");
    }
    for (const b of ui.tools.querySelectorAll(".hc-tool")) b.parentElement.hidden = !toolShown(b.dataset.tool, c);
    if (openTool && !toolShown(openTool, c)) toggleTool(openTool);
    if (!c.docker && con) say("Docker is not answering: start Docker Desktop, then refresh.", "warn");
    paintLive(c);
    paintProg();
    if (openTool) fillTool(openTool, true);
  }

  function paintLive(c) {
    const box = ui.live;
    if (!box) return;
    box.hidden = !liveWanted;
    if (!liveWanted) return;
    const on = !!(stat && stat.online);
    const was = box.classList.contains("on");
    box.className = "hc-live" + (on ? " on" : "");
    box.replaceChildren();
    const t = mk("div", "hc-live-t");
    const priv = (stat && stat.visibility === "private") || c.visibility === "private";
    t.append(mk("b", "", on ? "You're live" : "Almost there"),
      mk("span", "", !on ? "Connecting to the network. This takes a minute." : c.linked ? "Your server is on the realm list."
        : priv ? "Send friends your invite link. They make their account there, then play." : "Your realm is on the realm list. Send the link to anyone."));
    if (on && c.linked) {
      const ul = mk("ul", "hc-live-l");
      for (const line of ["Players reach your server through the network: the realm address your server gives out must already work for players.",
        "Turn off your login server's wrong-password ban (WrongPass.MaxCount = 0): every player comes through one address."]) ul.append(mk("li", "", line));
      t.append(ul);
    }
    box.append(t);
    if (on && !c.linked) box.append(btn("Copy invite link", "btn primary", copyInvite));
    if (on) { const x = btn("", "hc-x", () => { liveWanted = false; box.hidden = true; }); x.setAttribute("aria-label", "Close"); x.textContent = "×"; box.append(x); }
    if (on && !was) box.scrollIntoView({ block: "nearest" });
    if (!on && !liveTimer) liveTimer = setTimeout(() => { liveTimer = null; refresh(true); }, 10000);
  }
  let liveTimer = null;

  async function startRealm() {
    say("Starting your realm…");
    try { say(await call("host_start", {}), "ok"); } catch (e) { say(oops(e), "bad"); }
    setTimeout(() => refresh(true), 20000);
  }
  async function stopRealm() {
    if (!(await ask("Stop the realm?", "Everyone online is saved and sent off. Characters, accounts and your invite link are kept.", "Stop"))) return;
    ui.stop.disabled = true;
    say("Stopping…");
    try { say(await call("host_stop"), "ok"); } catch (e) { say(oops(e), "bad"); }
    ui.stop.disabled = false;
    gmStat = null;
    refresh(true);
  }

  // ---------------------------------------------------------------- the health check
  async function runCheck() {
    ui.run.disabled = true;
    ui.run.textContent = "Checking…";
    ui.chkSum.hidden = true;
    for (const { li, d } of Object.values(ui.checks)) { li.className = "c-run"; d.hidden = true; }
    let r = null;
    try { r = await call("host_check"); } catch (e) { say(oops(e), "bad"); }
    ui.run.disabled = false;
    ui.run.textContent = "Run check";
    if (!r) { for (const { li } of Object.values(ui.checks)) li.className = "c-idle"; return; }
    for (const row of r.rows || []) {
      const c = ui.checks[row.id];
      if (!c) continue;
      c.li.className = "c-" + row.state;
      const bad = (row.lines || []).find((l) => l.kind === "bad") || (row.lines || []).find((l) => l.kind === "note");
      c.d.hidden = !bad;
      c.d.textContent = bad ? (bad.fix ? `${bad.text} ${bad.fix}` : bad.text) : "";
      c.li.title = (row.lines || []).map((l) => l.text).join("\n");
    }
    ui.chkSum.hidden = false;
    ui.chkSum.className = "hc-sum " + (r.ok ? "ok" : "bad");
    ui.chkSum.textContent = r.ok ? "Players can reach your realm." : (r.summary || "Something needs fixing: see above.");
  }

  // ---------------------------------------------------------------- realm tools: one drawer under the panels
  function toggleTool(k) {
    openTool = openTool === k ? null : k;
    for (const b of ui.tools.querySelectorAll(".hc-tool")) b.setAttribute("aria-pressed", String(b.dataset.tool === openTool));
    ui.drawer.hidden = !openTool;
    if (openTool) fillTool(openTool);
  }

  function seg(options, current, on) {
    const g = mk("div", "hc-seg");
    g.setAttribute("role", "group");
    for (const [v, label] of options) {
      const b = btn(label, "hc-seg-b", () => on(v, b));
      b.dataset.value = v;
      b.setAttribute("aria-pressed", String(v === current));
      g.append(b);
    }
    return g;
  }
  async function tool(name, arg, b) {
    if (b) b.disabled = true;
    say("Saving…");
    try { say(await call("host_tool", { name, arg: arg || null }), "ok"); await refresh(true); return true; }
    catch (e) { say(oops(e), "bad"); return false; }
    finally { if (b) b.disabled = false; }
  }
  async function copyInvite() {
    let link = null;
    try { link = await call("host_invite"); } catch (_) { /* no kit */ }
    if (!link) { say("No invite link yet: it's made on the first start.", "warn"); return; }
    try { await navigator.clipboard.writeText(link); say("Invite link copied. Friends open it once, then play.", "ok"); }
    catch (_) { say(link, "ok"); }
  }

  async function fillTool(k, keep) {
    const d = ui.drawer;
    // a refresh repaints only the choices that show the realm's state, and never over something being typed
    const typing = [...d.querySelectorAll("input, textarea")].some((i) => i.value || document.activeElement === i);
    if (keep && d.dataset.tool === k && ((k !== "vis" && k !== "signup" && k !== "browser") || typing)) return;
    d.dataset.tool = k;
    d.replaceChildren();
    const head = mk("div", "hc-drawer-h");
    head.append(mk("b", "", TOOLS.find((t) => t[0] === k)[1]));
    const close = btn("", "hc-x", () => toggleTool(k));
    close.setAttribute("aria-label", "Close");
    close.innerHTML = '<svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>';
    head.append(close);
    d.append(head);
    const c = con || {};
    if (k === "invite") {
      let link = null;
      try { link = await call("host_invite"); } catch (_) { /* no kit */ }
      const code = mk("code", "hc-link", link || "Made on the first start.");
      const row = mk("div", "hc-row");
      row.append(btn("Copy", "btn small primary", copyInvite), btn("New link", "btn small", async (ev) => {
        const b = ev.currentTarget;
        if (!(await ask("Make a new invite link?", "The old link stops letting anyone new in. Friends who joined keep playing.", "Make a new link"))) return;
        b.disabled = true;
        try { code.textContent = await call("host_new_invite"); say("New invite link made.", "ok"); } catch (e) { say(oops(e), "bad"); }
        b.disabled = false;
      }));
      d.append(code, row, mk("p", "hc-fine", c.visibility === "private" ? "Private: the part after # is the key. Send it only to friends." : "Friends open it once, make their own account, then play."));
    } else if (k === "friends") {
      d.append(mk("p", "", "Friends make their own account on your join page, with your invite link. Leyline never makes accounts or sees passwords."));
      const row = mk("div", "hc-row");
      row.append(btn("Copy invite link", "btn small primary", copyInvite), btn("Who may sign up", "btn small", () => toggleTool("signup")));
      d.append(row);
    } else if (k === "vis") {
      d.append(seg([["public", "Public · on the realm list"], ["private", "Private · invite link only"]], c.visibility, (v, b) => { if (v !== c.visibility) tool(v, null, b); }));
      if (c.visibility === "public") {
        const tok = mk("input", "hc-token");
        tok.type = "text"; tok.placeholder = "Claim token from the website"; tok.maxLength = 64; tok.spellcheck = false; tok.autocomplete = "off";
        const row = mk("div", "hc-row");
        row.append(tok, btn("Claim with Hero Key", "btn small", (ev) => { if (!tok.value.trim()) { tok.focus(); return; } tool("claim", tok.value.trim(), ev.currentTarget); }));
        d.append(mk("p", "hc-fine", c.claim_sent ? "Claim sent: the realm list shows who it belongs to once the website sees it." : "Listed as Unclaimed until you claim it for your Hero Key account."), row);
      }
    } else if (k === "signup") {
      const now = c.signup || "invite";
      d.append(seg([["invite", "Invite link only"], ["open", "Anyone"], ["off", "Nobody new"]], now, (v, b) => { if (v !== now) tool("signup-" + v, null, b); }),
        mk("p", "hc-fine", "Who may make an account on your join page."));
    } else if (k === "browser") {
      const now = c.browser ? "on" : "off";
      d.append(seg([["on", "On · browsers can join"], ["off", "Off · desktop clients only"]], now, (v, b) => { if (v !== now) tool("browser-" + v, null, b); }),
        mk("p", "hc-fine", "The browser client has no anti-cheat module, so it can join only while the core's client anti-cheat (Warden) is off. The server's own checks stay on. Applies at the next start."));
    } else if (k === "max") {
      const n = mk("input", "hc-token");
      n.type = "number"; n.min = "0"; n.max = "10000"; n.placeholder = "0 = the ruleset's own";
      if (c.max_players) n.value = String(c.max_players);
      const row = mk("div", "hc-row");
      row.append(n, btn("Save", "btn small primary", (ev) => {
        const v = n.value.trim() || "0";
        if (!/^\d{1,5}$/.test(v) || Number(v) > 10000) { say("Max players: 0 to 10000.", "warn"); n.focus(); return; }
        tool("max", v, ev.currentTarget);
      }));
      d.append(row, mk("p", "hc-fine", "The most players at once: the server turns away the next one, and the realm list shows it. Applies at the next start."));
    } else if (k === "logs") {
      const pre = mk("pre", "hc-logs", "Reading the logs…");
      const row = mk("div", "hc-row");
      const load = async () => { try { pre.textContent = (await call("host_logs", { lines: 200 })) || "(no log lines yet)"; pre.scrollTop = pre.scrollHeight; } catch (e) { pre.textContent = oops(e); } };
      row.append(btn("Refresh", "btn small", load));
      d.append(pre, row);
      load();
    } else if (k === "dens") {
      fillDens(d, c);
    } else if (k === "rules") {
      const ta = mk("textarea", "hc-rules");
      ta.rows = 5; ta.spellcheck = true;
      ta.placeholder = "One rule per line, up to 10";
      const n = mk("span", "hc-fine");
      const count = () => { const l = ta.value.split("\n").filter((x) => x.trim()); n.textContent = `${l.length}/10`; n.classList.toggle("bad", l.length > 10 || l.some((x) => x.trim().length > 120)); };
      ta.addEventListener("input", count);
      try { ta.value = (await call("host_rules")).join("\n"); } catch (_) { /* none yet */ }
      count();
      const row = mk("div", "hc-row");
      row.append(btn("Save", "btn small primary", async (ev) => {
        const b = ev.currentTarget; b.disabled = true;
        try { say(await call("host_rules_set", { lines: ta.value.split("\n") }), "ok"); } catch (e) { say(oops(e), "bad"); }
        b.disabled = false;
      }), n);
      d.append(ta, row);
    }
  }

  // ---------------------------------------------------------------- Saved Dens (host/dens.rs; docs/DEN.md "The save file")
  // In game ".den save <name>" writes a file in the kit's state/dens; here: open one (the realm opens it when you're in
  // your Den), save a copy of one, or add a file from this PC (a friend's Den, a copy you kept).
  const denDate = (s) => { const t = Date.parse(s || ""); return isNaN(t) ? "" : new Date(t).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }); };
  async function fillDens(d, c) {
    const box = mk("div", "hc-dens");
    d.append(mk("p", "hc-fine", "In your Den, type .den save and a name: the whole Den goes into a file on this PC. Open one to put your Den back the way it was."), box);
    const add = mk("input", "");
    add.type = "file"; add.accept = ".json,application/json"; add.hidden = true;
    add.addEventListener("change", async () => {
      const f = add.files && add.files[0];
      add.value = "";
      if (!f) return;
      if (f.size > 512 * 1024) { say("That file is too big for a Den (512 KB at most).", "warn"); return; }
      try { const n = await call("host_den_add", { fileName: f.name, text: await f.text() }); say(`Added as '${n}'.`, "ok"); }
      catch (e) { say(oops(e, "That isn't a Den file."), "bad"); }
      paintDens(box, c);
    });
    const row = mk("div", "hc-row");
    row.append(btn("Add a Den file", "btn small", () => add.click()), add);
    d.append(row);
    await paintDens(box, c);
  }
  async function paintDens(box, c) {
    let list = [];
    try { list = await call("host_dens"); } catch (e) { box.replaceChildren(mk("p", "hc-fine", oops(e))); return; }
    box.replaceChildren();
    if (!list.length) { box.append(mk("p", "hc-fine", "No saved Dens yet.")); return; }
    const ul = mk("ul", "hc-den-list");
    for (const f of list) {
      const li = mk("li", "");
      const t = mk("div", "hc-den-t");
      t.append(mk("b", "", f.name === "backup" ? "backup (before the last open)" : f.name),
        mk("span", "", [`${f.props} prop${f.props === 1 ? "" : "s"}`, denDate(f.saved)].filter(Boolean).join(" · ")));
      const acts = mk("div", "hc-row");
      acts.append(btn("Open", "btn small primary", (ev) => openDen(f, ev.currentTarget, c)), btn("Save a copy", "btn small", () => saveDen(f)));
      li.append(t, acts);
      ul.append(li);
    }
    box.append(ul);
  }
  async function openDen(f, b, c) {
    const running = ((c && c.services) || []).some((s) => s.role === "world" && (s.state === "ok" || s.state === "starting"));
    if (!(await ask(`Open '${f.name}'?`, "It replaces what's in your Den now (a copy is kept as 'backup'). It opens as soon as you're in your Den" +
      (running ? ", within a few seconds." : ": the realm starts now."), "Open"))) return;
    b.disabled = true;
    try {
      await call("host_den_open", { name: f.name });
      if (running) say(`'${f.name}' opens when you're in your Den (or at your next login).`, "ok");
      else { say(`Starting your Den with '${f.name}'…`); say(await call("host_start", {}), "ok"); setTimeout(() => refresh(true), 20000); }
    } catch (e) { say(oops(e), "bad"); }
    b.disabled = false;
  }
  async function saveDen(f) {
    let text;
    try { text = await call("host_den_text", { name: f.name }); } catch (e) { say(oops(e), "bad"); return; }
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([text], { type: "application/json" }));
    a.download = `${f.name}.den.json`;
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 5000);
    say(`Saved a copy: ${f.name}.den.json.`, "ok");
  }

  // ================================================================ the setup start (no realm on this PC yet)
  let picked = null;
  let offers = null;
  function buildSetup() {
    const wrap = mk("div", "hc-setup");
    const sw = mk("section", "hc-sw hc-start");
    sw.setAttribute("aria-label", "Host a realm");
    const top = mk("div", "hc-top");
    const id = mk("div", "hc-id");
    id.append(mk("p", "hc-k", "The host pack"), mk("p", "hc-t", "Host a realm"));
    ui.prereq = mk("div", "hc-pre");
    top.append(crest(), id, ui.prereq);
    // TODO(owner): the setup start's line, in your own words.
    const lede = mk("p", "hc-lede", "Run a realm on this PC for friends or everyone. Players reach it through the Leyline relay: no ports to open, your address stays hidden.");
    // what a host needs and how long it takes, up front (the website's /wrath/host says the same). TODO(owner): the wording.
    const needs = mk("p", "hc-needs");
    needs.append(mk("b", "", "You need: "), document.createTextNode("Docker, your own game folder "),
      mk("span", "hc-ver", "3.3.5a"), document.createTextNode(", about 10 GB free. The first start takes 5 to 20 minutes, and your PC must stay on for others to play."));
    ui.rules = mk("div", "hc-rulesets");
    ui.rules.setAttribute("role", "radiogroup");
    ui.rules.setAttribute("aria-label", "Ruleset");
    const go = mk("div", "hc-go");
    ui.go = btn("Start", "btn primary", startPicked);
    ui.go.id = "hc-start";
    ui.go.disabled = true;
    ui.goHint = mk("span", "hc-fine", "Pick a ruleset.");
    ui.goWin = btn("Open in a window instead", "hc-more hc-win", startInWindow);
    ui.goWin.hidden = true;
    go.append(ui.go, ui.goHint, ui.goWin);
    // the kit's questions, answered here so no window asks them (host.rs Answers)
    ui.form = mk("div", "hc-form");
    const nameL = mk("label", "hc-field");
    ui.nameIn = mk("input", "hc-token");
    ui.nameIn.id = "hc-name"; ui.nameIn.type = "text"; ui.nameIn.maxLength = 32; ui.nameIn.spellcheck = false; ui.nameIn.autocomplete = "off";
    ui.nameIn.addEventListener("input", () => { ui.nameIn.dataset.typed = "1"; paintGo(); });
    nameL.append(mk("span", "", "Realm name"), ui.nameIn);
    ui.visL = mk("div", "hc-field");
    ui.visL.append(mk("span", "", "Who can join"));
    ui.visSeg = seg([["private", "Friends with my link"], ["public", "Everyone"]], visPick, (v) => { visPick = v; paintForm(); });
    ui.visSeg.id = "hc-vis";
    ui.visL.append(ui.visSeg);
    ui.gmL = mk("label", "hc-field");
    ui.gmIn = mk("input", "hc-token");
    ui.gmIn.id = "hc-gm"; ui.gmIn.type = "text"; ui.gmIn.maxLength = 16; ui.gmIn.spellcheck = false; ui.gmIn.autocomplete = "off";
    ui.gmIn.placeholder = "3 to 16 letters or digits";
    ui.gmIn.addEventListener("input", paintGo);
    ui.gmL.append(mk("span", "", "Your account name (it gets GM)"), ui.gmIn);
    ui.gmL.hidden = !needsGm;
    ui.form.append(nameL, ui.visL, ui.gmL);
    ui.prog = mk("div", "hc-prog");
    ui.prog.hidden = true;
    // My Den / Solo leveling: what the kit sets for it, in place of choices (paintGo fills it for the one picked)
    ui.den = mk("ul", "hc-den");
    ui.den.setAttribute("aria-label", "My Den");
    ui.den.hidden = true;
    ui.status = mk("p", "status hc-status");
    ui.status.setAttribute("role", "status");
    const link = mk("div", "hc-linkcard");
    const lt = mk("div", "");
    lt.append(mk("b", "", "Already run a server?"), mk("span", "", "List it with the relay agent. Your server is not changed."));
    ui.linkBtn = btn("Link my server", "btn small", linkServer);
    ui.linkBtn.id = "hc-link";
    link.append(lt, ui.linkBtn);
    // before anything else, while the Host pack or Docker is missing (owner 2026-10-30: "make getting the host pack more obvious")
    ui.ready = mk("div", "hc-ready");
    ui.ready.hidden = true;
    // a server already on this PC (host/scan.rs): put it on Leyline in one press
    ui.found = mk("div", "hc-found");
    ui.found.hidden = true;
    ui.pickH = mk("p", "hc-bh", "Pick a ruleset");
    sw.append(top, rune(), lede, needs, ui.ready, ui.found, ui.pickH, ui.rules, ui.den, ui.form, go, ui.prog, ui.status, link, rune(), mk("p", "hc-bh", "Runs on"), coreChips(linkServer));
    const side = mk("aside", "hc-box hc-side");
    side.append(mk("p", "hc-bh", "Guide"), guideList(true));
    wrap.append(sw, side);
    main.append(wrap);
    paintRulesets();
    paintProg();
  }

  // the form: a name for the ruleset (until one is typed), who can join (a Den is always friends only), and on a Mac or
  // Linux the host's own account name (`leyline host` never makes accounts; the Windows kit makes a GM login itself)
  const NAMES = { den: "My Den", solo: "Solo leveling", dalaran: "Dalaran Hangout", vanillaplus: "Vanilla Plus" };
  const needsGm = !/Windows/i.test(navigator.userAgent || "");
  let visPick = "private";
  function paintForm() {
    if (!ui.form) return;
    if (ui.nameIn && !ui.nameIn.dataset.typed) ui.nameIn.value = NAMES[picked] || "My Realm";
    ui.visL.hidden = LOCAL_IDS.includes(picked);
    for (const b of ui.visSeg.querySelectorAll(".hc-seg-b")) b.setAttribute("aria-pressed", String(b.dataset.value === visPick));
  }
  const NAME_OK = /^[A-Za-z0-9 _'-]{1,32}$/;
  const GM_OK = /^[A-Za-z0-9]{3,16}$/;
  function answers() {
    return { name: ui.nameIn.value.trim(), visibility: LOCAL_IDS.includes(picked) ? "private" : visPick, gm: needsGm ? ui.gmIn.value.trim() : null };
  }
  function formProblem() {
    if (!ui.nameIn) return null;
    const a = answers();
    if (!NAME_OK.test(a.name)) return "Give your realm a name: letters, digits and spaces.";
    if (needsGm && !GM_OK.test(a.gm || "")) return "Type your account name: 3 to 16 letters or digits.";
    return null;
  }

  // ---------------------------------------------------------------- a server already on this PC
  const CLIENTS = [["vanilla", "The original game (to 60)"], ["tbc", "Second era"], ["wrath", "Third era"]];
  let found = null;
  let scanned = false;
  async function scanServers() {
    let f = null;
    try { f = await call("host_scan"); } catch (_) { /* nothing found */ }
    scanned = true;
    found = f || null;
    paintFound();
  }
  function paintFound() {
    const box = ui.found;
    if (!box) return;
    box.hidden = !found;
    if (ui.pickH) ui.pickH.textContent = found ? "Or host a new realm" : "Pick a ruleset";
    if (!found) return;
    box.replaceChildren();
    const f = found;
    const head = mk("div", "hc-found-h");
    const t = mk("div", "hc-found-t");
    t.append(mk("b", "", !f.running ? "Your server is stopped" : f.confirmed || f.kind !== "ports" ? "Found a server on this PC" : "Is this your server?"), mk("span", "", f.what));
    head.append(t);
    box.append(head);
    if (!f.running) {
      box.append(mk("p", "hc-fine", "Start it the way you usually do, then press Look again."));
      const again = btn("Look again", "btn small", (ev) => { ev.currentTarget.disabled = true; scanServers(); });
      box.append(again);
      return;
    }
    const row = mk("div", "hc-found-f");
    const nl = mk("label", "hc-field");
    ui.linkName = mk("input", "hc-token");
    ui.linkName.id = "hc-link-name"; ui.linkName.type = "text"; ui.linkName.maxLength = 32; ui.linkName.spellcheck = false; ui.linkName.autocomplete = "off";
    ui.linkName.value = "My Server";
    nl.append(mk("span", "", "Its name on the realm list"), ui.linkName);
    row.append(nl);
    ui.linkClient = null;
    if (!f.client) {
      const cl = mk("label", "hc-field");
      ui.linkClient = mk("select", "hc-hours");
      ui.linkClient.id = "hc-link-client";
      for (const [v, label] of CLIENTS) { const o = mk("option", "", label); o.value = v; ui.linkClient.append(o); }
      cl.append(mk("span", "", "The game it's for"), ui.linkClient);
      row.append(cl);
    }
    const put = btn(f.confirmed || f.kind !== "ports" ? "Put it on Leyline" : "Yes, link it", "btn primary hc-step-go", linkFound);
    put.id = "hc-put";
    row.append(put);
    box.append(row, mk("p", "hc-fine", "Only a small helper runs next to it. Your server, its accounts and its settings are not changed."));
  }
  async function linkFound() {
    const f = found;
    if (!f) { linkServer(); return; }
    const name = ((ui.linkName && ui.linkName.value) || "").trim();
    if (!NAME_OK.test(name)) { say("Give your server a name: letters, digits and spaces.", "warn"); if (ui.linkName) ui.linkName.focus(); return; }
    if (dock && dock.state !== "ready") { say("Docker first (step 2 above): the helper runs in it.", "warn"); return; }
    const put = byId("hc-put");
    if (put) put.disabled = true;
    try {
      if (!(kit && kit.is_kit)) {   // one press: the Host pack comes with it
        say("Getting the Host pack…");
        const r = await call("hostpack_install");
        await call("set_host_kit", { path: r[1] });
      }
      const client = f.client || (ui.linkClient && ui.linkClient.value) || "wrath";
      await call("host_link_quiet", { answers: { name, auth: f.auth, world: f.world, client, core: f.core || null } });
      say("");
      job = { idle: false, kind: "link", running: true, code: null, step: "Getting ready", n: 0, of: 4, secs: 0, problem: null };
      followJob();
    } catch (e) { say(oops(e), "bad"); }
    finally { if (put) put.disabled = false; }
  }

  // ---------------------------------------------------------------- a start with no window: the kit's steps, here
  let job = null;          // host_progress
  let jobTimer = null;
  let liveWanted = false;  // a start just finished: the console says "You're live" once the relay sees the realm
  function fmt(secs) { const m = Math.floor(secs / 60), s2 = secs % 60; return m ? `${m} min ${String(s2).padStart(2, "0")} s` : `${s2} s`; }
  function paintProg() {
    const box = ui.prog;
    if (!box) return;
    // in the console a finished start is said by "You're live" instead
    box.hidden = !job || job.idle || (mode === "console" && !job.running && job.code === 0);
    if (box.hidden) return;
    const failed = !job.running && job.code !== 0;
    box.className = "hc-prog" + (failed ? " bad" : job.running ? "" : " ok");
    box.replaceChildren();
    const head = mk("div", "hc-prog-h");
    head.append(mk("b", "", failed ? (job.kind === "link" ? "Linking stopped" : "Setting up stopped") : job.running ? job.step : (job.kind === "link" ? "Linked" : "Set up")),
      mk("span", "hc-fine", job.running ? `Step ${Math.max(1, job.n)} of ${job.of} · ${fmt(job.secs)}` : fmt(job.secs)));
    const bar = mk("div", "hc-bar");
    const fill = mk("i", "");
    fill.style.width = `${Math.round((job.running ? Math.max(job.n - 0.5, 0.3) : job.of) / Math.max(job.of, 1) * 100)}%`;
    bar.append(fill);
    bar.setAttribute("role", "progressbar");
    bar.setAttribute("aria-valuemin", "0"); bar.setAttribute("aria-valuemax", String(job.of)); bar.setAttribute("aria-valuenow", String(job.n));
    box.append(head, bar);
    if (job.running) box.append(mk("p", "hc-fine", job.kind === "link" ? "Usually under a minute." : "The first start takes 5 to 20 minutes. You can use the rest of Leyline meanwhile."));
    if (failed) {
      box.append(mk("p", "hc-prog-why", job.problem || "Something went wrong."));
      const row = mk("div", "hc-row");
      row.append(btn("Try again", "btn small primary", job.kind === "link" ? linkFound : startPicked));
      if (job.kind !== "link") row.append(btn("Open in a window instead", "btn small", startInWindow));
      box.append(row);
    }
    if (ui.go) ui.go.hidden = !!(job.running);
    if (ui.goHint) ui.goHint.hidden = !!(job.running);
  }
  async function followJob() {
    if (jobTimer) clearTimeout(jobTimer);
    let p = null;
    try { p = await call("host_progress"); } catch (_) { /* try again */ }
    const wasRunning = !!(job && job.running);
    if (p) job = p;
    paintProg();
    paintGo();
    if (ui.prog && !ui.prog.hidden && (!wasRunning || !job.running)) ui.prog.scrollIntoView({ block: "nearest" });
    if (job && job.running) { jobTimer = setTimeout(followJob, 2000); return; }
    jobTimer = null;
    if (job && !job.idle && job.code === 0) {
      liveWanted = true;
      say(job.kind === "link" ? "Linked. Checking the realm list…" : "Your realm is up. Checking the network…", "ok");
      if (typeof loadHost === "function") loadHost();   // app.js: read the kit again (it now hosts a realm: the console)
    }
  }

  function paintRulesets() {
    if (!ui.rules) return;
    ui.rules.replaceChildren();
    const list = offers || ORDER.slice(0, 6).map((id) => ({ id, clients: [], note: null, core: "azerothcore" }));
    const ids = ORDER.filter((id) => list.some((o) => o.id === id));
    if (!picked && offers) {
      const free = ids.filter((id) => !list.find((o) => o.id === id).note);
      picked = free.includes("dalaran") ? "dalaran" : free[0] || null;
    }
    for (const id of ids) {
      const o = list.find((x) => x.id === id);
      const [title, line, chip] = RULESETS[id] || [o.label || id, ""];
      const b = btn("", "hc-rs" + (picked === id ? " on" : "") + (o.note ? " off" : ""), () => pick(id));
      b.setAttribute("role", "radio");
      b.setAttribute("aria-checked", String(picked === id));
      b.dataset.profile = id;
      b.disabled = !!o.note;
      const h = mk("span", "hc-rs-h");
      h.append(mk("b", "", title));
      if (chip) h.append(mk("em", "", chip));
      b.append(h, mk("span", "hc-rs-l", o.note || line));
      ui.rules.append(b);
    }
    paintGo();
  }
  function paintGo() {
    if (!ui.go) return;
    const ready = kit && kit.is_kit;
    const name = picked ? (RULESETS[picked] || [picked])[0] : null;
    const dockOk = !dock || dock.state === "ready";
    paintForm();
    const why = formProblem();
    ui.go.disabled = !ready || !picked || !dockOk || !!(job && job.running);
    ui.go.textContent = name ? `Start ${name}` : "Start";
    ui.goHint.textContent = !ready ? "Get the Host pack first (step 1 above)." : !dockOk ? "Docker first (step 2 above)." : !picked ? "Pick a ruleset."
      : why ? why
      : picked === "vanillaplus" ? "Always on: it comes back after a reboot."
      : picked === DEN ? "Then invite friends with your invite link. The first start takes 5 to 20 minutes."
      : picked === SOLO ? "Then pick Solo leveling in the play bar. The first start takes 5 to 20 minutes." : "The first start takes 5 to 20 minutes.";
    if (ui.den) {
      const facts = FACTS[picked];
      ui.den.hidden = !facts;
      if (facts && ui.den.dataset.for !== picked) {
        ui.den.dataset.for = picked;
        ui.den.setAttribute("aria-label", rsName(picked));
        ui.den.replaceChildren(...facts.map(([b, t]) => { const li = mk("li", ""); li.append(mk("b", "", b), mk("span", "", t)); return li; }));
      }
    }
  }
  async function pick(id) {
    const o = (offers || []).find((x) => x.id === id);
    picked = id;
    paintRulesets();
    try { await call("host_choose", { profile: id, clientId: o && o.clients[0] ? o.clients[0].id : null }); say(""); }
    catch (e) { say(oops(e), "bad"); }
  }
  async function startPicked() {
    const why = formProblem();
    if (why) { say(why, "warn"); (needsGm && !GM_OK.test(ui.gmIn.value.trim()) ? ui.gmIn : ui.nameIn).focus(); return; }
    ui.go.disabled = true;
    say("");
    try {
      const o = (offers || []).find((x) => x.id === picked);
      await call("host_choose", { profile: picked, clientId: o && o.clients[0] ? o.clients[0].id : null });
      await call("host_start_quiet", { answers: answers() });
      job = { idle: false, kind: "host", running: true, code: null, step: "Getting ready", n: 0, of: 8, secs: 0, problem: null };
      followJob();
    } catch (e) { say(oops(e), "bad"); if (ui.goWin) ui.goWin.hidden = false; }
    paintGo();
  }
  // the old way: the kit's script in a window of its own, asking its questions there
  async function startInWindow() {
    say("Opening it in a window…");
    try { say(await call("host_start", {}), "ok"); } catch (e) { say(oops(e), "bad"); }
    setTimeout(() => { if (typeof loadHost === "function") loadHost(); }, 20000);
  }
  async function linkServer() {
    if (!(kit && kit.is_kit)) { say("Get the Host pack first (Host pack & settings).", "warn"); return; }
    say("Opening Link my server…");
    try { say(await call("host_link_start"), "ok"); } catch (e) { say(oops(e), "bad"); }
  }

  // Docker, as the step says it (host/docker.rs DockerState; `help` is its plain line for this OS): the step's title, and
  // its button (none while Docker is closed: the page sees it start by itself)
  const DOCK_TITLE = { missing: "Get Docker", stopped: "Start Docker", desktop_stopped: "Start Docker", starting: "Starting Docker…",
    no_permission: "Let Docker in", no_compose: "Update Docker", ready: "Docker" };
  const dockLink = (d) => () => call("open_link", { name: (d.get_url || "").includes("engine") ? "dockerengine" : "docker" }).catch(() => {});
  function dockAction(d) {
    if (d.state === "starting") return btn("Cancel", "btn hc-step-go", stopWaiting);
    if (d.can_start) { const b = btn("Start Docker", "btn primary hc-step-go", () => startDocker(b)); b.id = "hc-docker-start"; return b; }
    if (d.state === "missing") return btn("Get Docker", "btn hc-step-go", dockLink(d));
    if (d.state === "no_compose") return btn("Update Docker", "btn hc-step-go", dockLink(d));
    return null;
  }
  // the two things a host needs first, as two big numbered steps; gone once both are ready
  function paintReady(p, d, haveKit) {
    const box = ui.ready;
    if (!box) return;
    const dockOk = !d || d.state === "ready";
    box.hidden = haveKit && dockOk;
    if (box.hidden) return;
    box.replaceChildren(mk("p", "hc-ready-h", "Before you start"));
    const step = (n, done, title, line, action) => {
      const r = mk("div", "hc-step" + (done ? " done" : ""));
      const num = mk("span", "hc-step-n", done ? "✓" : String(n));
      num.setAttribute("aria-hidden", "true");
      const t = mk("div", "hc-step-t");
      t.append(mk("b", "", title), mk("span", "", line));
      r.append(num, t);
      if (action) r.append(action);
      box.append(r);
    };
    const av = p && p.available, inst = p && p.installed;
    let kitBtn = null;
    if (!haveKit) {
      const label = inst ? "Use the Host pack" : "Get the Host pack";
      kitBtn = btn(label, "btn primary hc-step-go", () => getPack(kitBtn, inst && p.kit_dir));
      kitBtn.id = "hc-get-pack";
      if (!inst && !av) kitBtn.disabled = true;
    }
    const kitLine = haveKit ? "Installed and ready." : inst ? "Downloaded: one click to use it."
      : av ? `One download${av.size ? `, ${Math.ceil(av.size / 1024)} KB` : ""}: the realm, its console and the relay agent. No game files.`
      : "Not listed right now: try again in a minute.";
    step(1, haveKit, "Get the Host pack", kitLine, kitBtn);
    const st = d ? d.state : "ready";
    const line = dockOk ? "Docker is running." : st === "missing" ? `${d.help || "Free. It runs the realm's servers."} This page sees it once it's in.`
      : (d.help || "Docker is not answering yet.");
    step(2, dockOk, dockOk ? "Docker" : (DOCK_TITLE[st] || "Get Docker"), line, dockOk ? null : dockAction(d));
    const s2 = box.lastChild;
    s2.dataset.docker = st;
    s2.classList.toggle("wait", st === "starting");
  }
  async function getPack(b, dir) {
    b.disabled = true;
    b.textContent = dir ? "Setting up…" : "Downloading…";
    try {
      if (!dir) { const r = await call("hostpack_install"); dir = r[1]; }
      await call("set_host_kit", { path: dir });
      say("The Host pack is ready.", "ok");
      if (typeof loadHost === "function") loadHost();   // app.js: re-read the kit
    } catch (e) { say(oops(e), "bad"); b.disabled = false; b.textContent = "Try again"; }
  }

  let pk = null;      // hostpack_info
  let dock = null;    // docker_info
  async function loadSetup() {
    paintGo();
    const [p, d, prof] = await Promise.all([
      call("hostpack_info").catch(() => null), call("docker_info").catch(() => null), call("host_profiles").catch(() => null)]);
    if (mode !== "setup") return;
    pk = p; dock = d;
    if (Array.isArray(prof) && Array.isArray(prof[0])) {
      offers = prof[0].filter((o) => o && Array.isArray(o.clients));
      if (prof[1] && prof[1].profile && !picked) picked = prof[1].profile;
    }
    paintRulesets();
    applyWant();
    paintPrereq();
    watchDocker();
    if (!scanned || !found) scanServers();
  }

  // Docker is looked at again every 5 s while the Start tab shows (someone installs or opens Docker Desktop and comes
  // back: the step moves on by itself); never while the page, the tab or the window is hidden
  const DOCK_EVERY = 5000;
  let dockTimer = null;
  let dockBusy = false;
  const startShown = () => mode === "setup" && isOn() && main.classList.contains("on") && document.visibilityState !== "hidden";
  function watchDocker() {
    if (startShown()) { if (!dockTimer) dockTimer = setInterval(recheckDocker, DOCK_EVERY); }
    else if (dockTimer) { clearInterval(dockTimer); dockTimer = null; }
  }
  async function recheckDocker(force) {
    if (!startShown()) { watchDocker(); return; }
    if (dockBusy || (waiting && force !== true)) return;
    dockBusy = true;
    let d = null;
    try { d = await call("docker_info"); } catch (_) { /* keep the last answer */ } finally { dockBusy = false; }
    if (!d || mode !== "setup") return;
    const was = dock && dock.state;
    dock = d;
    if (d.state !== was) { paintPrereq(); paintGo(); if (d.state === "ready") scanServers(); }
  }
  document.addEventListener("visibilitychange", watchDocker);
  new MutationObserver(watchDocker).observe(page, { attributes: true, attributeFilter: ["class"] });
  new MutationObserver(watchDocker).observe(main, { attributes: true, attributeFilter: ["class"] });
  window.__hcDockerWatch = () => !!dockTimer;   // for the tests

  // Start Docker: Docker Desktop's own start (or its app), then `docker info` every 3 s for up to two minutes
  const WAIT_EVERY = 3000, WAIT_MAX = 120000;
  let waiting = null;    // { t0, timer }
  async function startDocker(b) {
    if (b) b.disabled = true;
    let note;
    try { note = await call("docker_start"); }
    catch (e) { say(oops(e, "Docker did not start. Open Docker Desktop yourself: this page sees it."), "warn"); if (b) b.disabled = false; return; }
    say(note || "");
    waiting = { t0: Date.now(), timer: null };
    dock = Object.assign({}, dock, { state: "starting", can_start: false, help: "This usually takes under a minute." });
    paintPrereq(); paintGo();
    waitTick();
  }
  function stopWaiting(msg) {
    if (waiting && waiting.timer) clearTimeout(waiting.timer);
    waiting = null;
    if (dock && dock.state === "starting") recheckDocker(true);
    if (typeof msg === "string") say(msg, "warn"); else say("");
  }
  async function waitTick() {
    if (!waiting || mode !== "setup") return;
    let d = null;
    try { d = await call("docker_info"); } catch (_) { /* try again */ }
    if (!waiting) return;
    const secs = Math.round((Date.now() - waiting.t0) / 1000);
    if (d && d.state === "ready") { waiting = null; dock = d; say("Docker is running.", "ok"); paintPrereq(); paintGo(); return; }
    if (Date.now() - waiting.t0 >= WAIT_MAX) {
      waiting = null; dock = d || dock; paintPrereq(); paintGo();
      say("Docker hasn't answered yet. Look at Docker Desktop's window: it may ask you to accept its terms, then try again.", "warn");
      return;
    }
    const help = secs < 45 ? `${secs} s so far. This usually takes under a minute.`
      : `${secs} s so far. If Docker Desktop shows a window, accept it there.`;
    dock = Object.assign({}, d || dock, { state: "starting", can_start: false, help });
    paintPrereq();
    waiting.timer = setTimeout(waitTick, WAIT_EVERY);
  }

  function paintPrereq() {
    const pre = ui.prereq;
    const p = pk, d = dock;
    if (!pre) return;
    pre.replaceChildren();
    const chip = (ok, text, onFix) => {
      const c = mk(onFix ? "button" : "span", "hc-pill " + (ok ? "ok" : "warn"));
      if (onFix) { c.type = "button"; c.addEventListener("click", onFix); }
      const dot = mk("span", "");
      dot.setAttribute("aria-hidden", "true");
      c.append(dot, document.createTextNode(text));
      return c;
    };
    const haveKit = kit && kit.is_kit;
    paintReady(p, d, haveKit);
    if (ui.ready && !ui.ready.hidden) return;   // the steps say it already: no pills in the corner
    pre.append(chip(haveKit, haveKit ? "Host pack ready" : (p && p.installed ? "Host pack: use it" : "Get the Host pack"), haveKit ? null : () => tab("kit")));
    if (d) pre.append(chip(d.state === "ready", d.state === "ready" ? "Docker ready" : (DOCK_TITLE[d.state] || "Get Docker"), d.state === "ready" ? null : dockLink(d)));
  }
})();
