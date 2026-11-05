// Leyline: single player everywhere (owner 3 Nov: "Entirely make sure single player for all modes is apparent and
// available for people with local files etc, and then 1 click invite friend to single player world").
//
// One label and one place: every mode and realm card that can be played on this PC gets a "Play solo" button right
// after its online button (Play / Host), and one line under it saying where it runs and what it needs. The worlds
// it opens, all on this PC with the player's own game folder (Leyline never ships game files):
//   * Your Den: Den Lite, a small server built into Leyline. Nothing else to install (den.js, app.js LOCAL.den).
//   * Single player: a full realm from the Host pack, in Docker (app.js LOCAL.solo).
//   * a mode package (docs/MODES.md, hosting/kit/modes): the same realm, private, with that mode's ruleset and scripts
//     (hostmodes.js open(id, {solo})); a catalog mode that runs on one (stream/playmodes.json) gets its pick written
//     so it starts after you log in (modes.rs mode_prepare_desktop) once that realm runs here.
//   * another game version's own realm (the Host pack's cmangos / tcpp profiles), private.
//   * a browser mode (Azeroth Survivors): it is a one-player game already, in your browser.
// What can't run on this PC says so in the same place ("Online only"), never a button that fails.
// A realm needing Docker says so before you press anything; its button goes to the Host page's own Docker steps.
//
// Invite a friend (owner: "1 click invite friend"): from any solo world, one button makes the relay invite link
// (Den Lite: app.rs den_invite, a login for one friend inside it; a Host pack realm: host_invite) and copies it. The
// friend opens it: the relay's join page has "Open in Leyline" (leyline://join?link=…, deeplink.rs), and their
// launcher shows one Join button (offerJoin) that knocks and starts the game (app.rs join_invite).
//
// Loaded before modes.js: modes.js, hostmodes.js and app.js call it while they draw their cards. Everything else it
// uses (app.js's LOCAL, localGo, kit; den.js; hostmodes.js) is looked up when a button is pressed.
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
  const byId = (id) => document.getElementById(id);

  const LABEL = "Play solo";
  const ONLINE_ONLY = "Online only";
  // TODO(owner): the one line under every Play solo, his words
  const WHERE = "On this PC, with your own game files";

  // catalog modes (stream/playmodes.json) -> the mode package that runs them on your own realm (hosting/kit/modes)
  const PACK_FOR_MODE = {
    minigolf: "minigolf-night", "bumper-balls": "minigolf-night", "coin-dash": "minigolf-night", community: "minigolf-night",
    brawl: "duel-ladder",
  };
  // why a catalog mode has no solo world yet (the official realm's hub, or more than one player by design)
  const ONLINE_WHY = {
    "arena-1v1": "Two players by design: it pairs you with someone on the official realm.",
    "arena-queue": "Two players by design: it pairs you with someone on the official realm.",
    "frozen-throne": "It lives on the official realm for now: a raid of 10, with companions or players.",
    keystones: "It lives on the official realm's hub for now.",
    transmog: "It lives on the official realm's hub for now.",
    hardcore: "It lives on the official realm's hub for now.",
  };
  // a game version -> the Host pack profile that runs it (host/profiles.rs)
  const PROFILE_FOR_BUILD = { "3.3.5a": "solo", "1.12.1": "cmangos-1.12.1", "2.4.3": "cmangos-2.4.3", "4.3.4": "tcpp-4.3.4" };
  const PACK_NAMES = { "minigolf-night": "Minigolf night", "duel-ladder": "Duel ladder", "solo-leveling": "Single player",
    levelling: "Classic levelling", "hide-and-seek": "Hide and seek" };

  // ---------------------------------------------------------------- Docker, asked once (and again after a setup)
  let dock = null, dockAsk = null;
  function docker() {
    if (!dockAsk) {
      dockAsk = call("docker_info").then((d) => { dock = d && typeof d === "object" ? d : null; paintNotes(); return dock; })
        .catch(() => { dock = null; return null; });
    }
    return dockAsk;
  }
  const dockerReady = () => !!(dock && dock.state === "ready");
  function kitNow() { try { return typeof kit !== "undefined" ? kit : null; } catch (_) { return null; } }   // app.js
  function hosted(profile) { const k = kitNow(); return !!(k && k.realm && k.realm.profile === profile); }

  // ---------------------------------------------------------------- what Play solo opens
  // { kind: "den" | "realm" | "browser" | "none", name, profile, pack, pick, mode, why, coming }
  const DEN = { kind: "den", name: "Your Den" };
  const SOLO_LEVELING = { kind: "realm", name: "Single player", profile: "solo" };
  function forMode(m) {
    if (!m || !m.id) return null;
    if (m.kind === "offline") return { kind: "browser", name: m.title, mode: m.id, coming: m.status !== "live" };
    const pack = PACK_FOR_MODE[m.id] || (m.start && m.start.host && PACK_NAMES[m.start.host] ? m.start.host : null);
    if (pack) return { kind: "realm", name: PACK_NAMES[pack] || pack, pack, profile: null, pick: m.kind === "realm" ? m.id : null, coming: m.status !== "live" };
    if (m.kind === "realm-type" && m.status !== "live") return { kind: "none", why: "Coming soon.", coming: true };
    return { kind: "none", why: ONLINE_WHY[m.id] || "It needs the official realm for now." };
  }
  function forPack(p) {
    if (!p || !p.id) return null;
    if (p.profile === "den") return DEN;
    if (!p.profile) return null;   // not a realm (Survivors: the Modes page's card)
    return { kind: "realm", name: p.name, pack: p.id, profile: p.profile, coming: p.status === "coming" };
  }
  function forRealm(r) {
    if (!r) return null;
    if (r.mode && r.mode.id && PACK_NAMES[r.mode.id]) return { kind: "realm", name: r.mode.name || PACK_NAMES[r.mode.id], pack: r.mode.id };
    const prof = PROFILE_FOR_BUILD[r.build || r.version || "3.3.5a"];
    if (prof === "solo") return SOLO_LEVELING;
    if (prof) return { kind: "realm", name: "your own realm of this version", profile: prof };
    return { kind: "none", why: "Leyline can't run this game version on your PC yet." };
  }

  // the line under the button
  function noteText(t) {
    if (!t) return "";
    if (t.kind === "none") return `${ONLINE_ONLY}: ${t.why}`;
    if (t.coming) return "Solo too, once it's out.";
    if (t.kind === "den") return `${WHERE}. Nothing else to install.`;
    if (t.kind === "browser") return "Just you, in your browser. Nothing to install.";
    // a realm in Docker
    if (t.profile && hosted(t.profile)) return `${WHERE}: ${t.name} is set up here.`;
    if (dock && !dockerReady()) return `${WHERE}. Needs Docker (free): ${dock.help || "set it up once"}`;
    if (dock) return `${WHERE}: ${t.name}, set up once (5 to 20 minutes).`;
    return `${WHERE}: ${t.name}, in Docker.`;
  }
  const realmTitle = (t) => (dock && !dockerReady() ? "Needs Docker first: this opens the one-time setup" : `Play ${t.name} on this PC`);
  const notes = new Set();
  function paintNotes() {
    for (const n of [...notes]) {
      if (!n.isConnected) { notes.delete(n); continue; }
      n.textContent = noteText(n.__solo);
      const b = n.__soloBtn;
      if (b && n.__solo.kind === "realm" && !n.__solo.coming) b.title = realmTitle(n.__solo);
    }
  }

  // ---------------------------------------------------------------- the button and its line
  // `button(t)`: the Play solo button (or a quiet "Online only" chip). `note(t)`: its one line. `attach(acts, after, t)`:
  // the button right after the online one in `acts`, the line at the end of `body` (or returned, for the caller to place).
  function button(t) {
    if (!t) return null;
    if (t.kind === "none") {
      const s = mk("span", "solo-none", ONLINE_ONLY);
      s.title = t.why;
      return s;
    }
    const b = mk("button", "btn small solo-go", LABEL);
    b.type = "button";
    b.dataset.solo = t.kind;
    if (t.coming) { b.disabled = true; b.title = "Not out yet"; return b; }
    b.title = t.kind === "den" ? "Your Den on this PC: no Docker, no sign-in" : t.kind === "browser" ? "Just you, in your browser" : realmTitle(t);
    b.addEventListener("click", (ev) => { ev.stopPropagation(); go(t, b); });
    if (t.kind === "realm") docker();
    return b;
  }
  function note(t, b) {
    if (!t) return null;
    const p = mk("p", "solo-note", "");
    p.__solo = t;
    p.__soloBtn = b || null;
    p.textContent = noteText(t);
    notes.add(p);
    return p;
  }
  function attach(acts, body, t) {
    if (!t || !acts) return null;
    const b = button(t);
    const online = acts.firstElementChild;   // the card's online button stays first: Play solo right beside it
    if (online) online.after(b); else acts.append(b);
    const n = note(t, b);
    if (body) body.append(n);
    return { button: b, note: n };
  }

  // ---------------------------------------------------------------- Play solo
  let status = null;   // where the last line goes (the card's page status, else Home's solo row)
  function say(text, kind) {
    const s = status && status.isConnected ? status : byId("solo-status");
    if (!s) return;
    if (!s.dataset.base) s.dataset.base = s.className.replace(/\s+(ok|bad|warn)\b/g, "") || "status";   // its own classes kept
    s.textContent = text || "";
    s.className = s.dataset.base + (kind ? " " + kind : "");
  }
  function statusFor(b) {
    const page = b && b.closest ? b.closest(".page") : null;
    return page ? page.querySelector("#md-status, .hx-status, #rl-status, #solo-status") : null;
  }
  function nav(id) { if (window.LeylineNav) window.LeylineNav.show(id); }
  const local = (id) => { try { return typeof LOCAL !== "undefined" ? LOCAL[id] : null; } catch (_) { return null; } };   // app.js

  async function go(t, b) {
    status = statusFor(b);
    if (t.kind === "den") {
      const L = local("den");
      if (typeof localGo === "function" && L) localGo(L);
      else if (window.LeylineDen) { nav("den"); window.LeylineDen.go(); }
      return;
    }
    if (t.kind === "browser") {
      try { say(await call("mode_open_browser", { id: t.mode }), "ok"); } catch (e) { say(oops(e), "bad"); }
      return;
    }
    if (t.kind !== "realm") return;
    // Single player: app.js's own steps (game folder, set up, switch, knock, PLAY)
    if (t.profile === "solo" && !t.pack) {
      const L = local("solo");
      if (typeof localGo === "function" && L) { localGo(L); return; }
    }
    const prof = t.profile || (t.pack === "solo-leveling" ? "solo" : null);
    const k = kitNow();
    const here = k && k.realm && ((prof && k.realm.profile === prof) || (t.pack && k.realm.mode === t.pack));
    if (here) {
      // already on this PC: the mode's pick first (it starts after you log in), then in
      if (t.pick) {
        try {
          const st = await call("get_state");
          const c = (st.settings.clients || []).find((x) => x.build === "3.3.5a");
          if (c) say(await call("mode_prepare_desktop", { client: c.id, id: t.pick }), "ok");
        } catch (e) { say(oops(e), "warn"); }
      }
      if (window.LeylineLocal && window.LeylineLocal.playHosted) { window.LeylineLocal.playHosted(); return; }
      nav("host");
      return;
    }
    // one realm per PC: another one runs here now
    if (k && k.realm) {
      nav("host");
      say(`This PC runs ${k.realm.name || "a realm"} now, and one PC runs one realm at a time. Stop it on the Host page, then press ${LABEL} again.`, "warn");
      return;
    }
    // not set up here yet: the mode's form, private, for you (hostmodes.js); else the Host page with that ruleset
    if (t.pack && window.LeylineHostModes && window.LeylineHostModes.open) {
      await window.LeylineHostModes.open(t.pack, { solo: true });
    } else {
      nav("host");
      if (window.LeylineHostModes && window.LeylineHostModes.rulesets) window.LeylineHostModes.rulesets();
      document.dispatchEvent(new CustomEvent("leyline:host-ruleset", { detail: { profile: prof || "solo", solo: true } }));
    }
    status = document.querySelector("#page-host .hx-status") || document.getElementById("host-status-line");   // the lines go where you are now
    await docker();
    if (dock && !dockerReady()) say(`${t.name} runs in Docker on this PC. ${dock.help || "Set it up once on the Host page."}`, "warn");
  }

  // ---------------------------------------------------------------- Invite a friend
  // The link for one friend, copied: Den Lite's (with their own login inside) when Your Den runs there, else the Host
  // pack realm's. Returns { link, copied } or throws a plain line.
  async function inviteLink() {
    let den = null;
    try { den = await call("den_state"); } catch (_) { den = null; }
    const k = kitNow() || (await call("host_info").catch(() => null));
    const docker = !!(den && den.docker_den) || !!(k && k.realm);
    if (den && den.lite && (den.lite.available || den.lite.running) && !docker) {
      const r = await call("den_invite", { name: null });
      return { link: r.link, who: r.name, hours: 24 };
    }
    if (k && k.realm) {
      let link = null;
      try { link = await call("host_invite"); } catch (_) { link = null; }
      if (!link) link = await call("host_new_invite");
      return { link, who: null, hours: null };
    }
    throw "Play solo once first: your world on this PC is what your friend joins.";
  }
  async function invite(b, sayTo) {
    const out = sayTo || ((t, k) => say(t, k));
    if (b) b.disabled = true;
    out("Making your invite link… (a few seconds)");
    try {
      const r = await inviteLink();
      let copied = true;
      try { await navigator.clipboard.writeText(r.link); } catch (_) { copied = false; }
      // TODO(owner): the words
      const how = "They open it and press Open in Leyline (or paste it in Leyline under Realms › Got an invite link?).";
      out(copied ? `Invite link copied: send it to one friend. ${how}${r.hours ? ` It works for ${r.hours} hours.` : ""}`
        : `Send this link to one friend: ${r.link} ${how}`, "ok");
      return r;
    } catch (e) {
      out(oops(e, "Your invite link couldn't be made. Try again."), "bad");
      return null;
    } finally { if (b) b.disabled = false; }
  }
  function inviteButton(sayTo) {
    const b = mk("button", "btn small solo-invite", "Invite a friend");
    b.type = "button";
    b.title = "Copies a link for one friend to join your world on this PC (private: only people with the link get in)";
    b.addEventListener("click", () => invite(b, sayTo));
    return b;
  }

  // ---------------------------------------------------------------- the friend's side: one Join
  // leyline://join?link=… (app.js handleDeepLink): one card with Join. Nothing joins until it is pressed.
  async function offerJoin(link) {
    if (typeof link !== "string" || !/^https:\/\/[a-z0-9.-]+\/join\/[a-z2-7]{16}#/.test(link)) return;
    let box = byId("solo-join");
    if (box) box.remove();
    box = mk("div", "solo-join");
    box.id = "solo-join";
    box.setAttribute("role", "dialog");
    box.setAttribute("aria-label", "Join your friend's world");
    const card = mk("div", "solo-join-card");
    card.append(mk("b", "", "A friend invited you to their world"),
      mk("p", "solo-join-line", "Join starts your game on their world, through the Leyline relay. Your address isn't shown to them."));
    const st = mk("p", "status solo-join-status");
    st.setAttribute("role", "status");
    const acts = mk("div", "solo-join-acts");
    const join = mk("button", "btn primary", "Join");
    join.type = "button";
    join.id = "solo-join-go";
    const later = mk("button", "btn small", "Not now");
    later.type = "button";
    later.addEventListener("click", () => box.remove());
    join.addEventListener("click", async () => {
      join.disabled = true;
      const line = (t, k) => { st.textContent = t; st.className = "status solo-join-status" + (k ? " " + k : ""); };
      let s = null;
      try { s = await call("get_state"); } catch (e) { line(oops(e), "bad"); join.disabled = false; return; }
      const clients = (s.settings && s.settings.clients) || [];
      const c = clients.find((x) => x.id === s.settings.selected_client && x.build === "3.3.5a") || clients.find((x) => x.build === "3.3.5a");
      if (!c) {
        line("Your friend's world plays with your own game folder, the same version as theirs: add it, then press Join again.", "warn");
        const add = mk("button", "link-s", "Add your game folder ›");
        add.type = "button";
        add.addEventListener("click", () => nav("clients"));
        st.append(" ", add);
        join.disabled = false;
        return;
      }
      line("Joining…");
      try {
        const j = await call("join_invite", { client: c.id, link });
        const via = { lan: " on your shared network", own: " through your friend's own address", relay: " through the Leyline relay" }[j.way] || "";
        if (j.guest) line(j.guest.auto ? `Joining your friend's world${via}: the game signs you in by itself.`
          : `Joining your friend's world${via}. At the login screen: account ${j.guest.account}, password ${j.guest.password}.`, "ok");
        else line(`${j.name || "Their world"} is open for you: ${c.name} now points at it. Press PLAY.`, "ok");
        later.textContent = "Close";
        if (typeof refresh === "function") refresh().catch(() => { /* the bar as it was */ });
      } catch (e) { line(oops(e, "Their world didn't answer. Ask your friend to check it is running."), "bad"); join.disabled = false; }
    });
    acts.append(join, later);
    card.append(acts, st);
    box.append(card);
    document.body.append(box);
    join.focus();
  }

  // ---------------------------------------------------------------- Home: every way to play solo, in one row
  function homeRow() {
    const den = byId("hm-den");
    if (!den || byId("solo-home")) return;
    const row = mk("div", "solo-home");
    row.id = "solo-home";
    const ways = mk("div", "solo-ways");
    const way = (label, title, on) => { const b = mk("button", "link-s solo-way", label); b.type = "button"; b.title = title; b.addEventListener("click", on); return b; };
    ways.append(mk("span", "solo-ways-h", "More on your own:"),
      way("Single player ›", "Just you and your bot companions, on this PC (Docker)", (ev) => go(SOLO_LEVELING, ev.currentTarget)),
      way("Modes ›", "Every mode, with Play solo where it runs on this PC", () => nav("modes")),
      way("Your own realm ›", "Any ruleset, private, on this PC (Docker)", () => nav("host")),
      way("GM command center ›", "Teleport, spawn, time, weather, fly: full control of your own worlds", () => { if (window.LeylineGM) window.LeylineGM.open(); }));
    const st = mk("p", "status solo-status");
    st.id = "solo-status";
    st.dataset.base = "status solo-status";
    st.setAttribute("role", "status");
    row.append(ways, inviteButton(), st);
    den.append(row);
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", homeRow); else homeRow();

  window.LeylineSolo = { LABEL, WHERE, forMode, forPack, forRealm, button, note, attach, go, invite, inviteLink, inviteButton, offerJoin,
    docker: () => docker(), DEN, SOLO_LEVELING, PACK_FOR_MODE };
})();
