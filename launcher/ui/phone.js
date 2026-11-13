// Leyline Launcher: the phone app (docs/ANDROID.md). Nothing here runs on a computer: it waits for platform.js to say
// "phone" (window.LeylinePlatform; the core's `platform` command, mobile.rs).
//
// On a phone the launcher is the same pages laid out for a small screen (phone.css), with a tab bar in place of the
// side menu and the play bar: Play (#page-phone), Realms, Mods, Account.
//   * PLAY opens the website's Play page (get_state's browser_url, the normal /wrath sign-in and Play) inside the app,
//     full screen; the phone's Back button comes back to the launcher. The game draws with WebGPU: a web view without
//     it gets one line and Open in Chrome (open_browser_play) instead of a black page.
//   * Realms: the official realm's Play is the browser game too, and so is a community realm the browser game can
//     play (a relay realm for the 3.3.5a client: the site's /wrath?go=r:<id> hop, the same as the website's "Play in
//     your browser"). Any other community realm plays with your own game on a computer.
//   * Join a friend (docs/MOBILE-RELEASE.md): a friend's invite link (https://<Leyline relay>/join/<id>#<invite>, or
//     its leyline://join?link=… form), pasted on Play or under Realms › Got an invite link?, plays their PC-hosted realm
//     in the app in one tap: /wrath?go=r:<id>#inv=<invite> (the invite stays in the #fragment, never sent to a server;
//     wrath-page.js hopInvite hands it to the realm's own join page, where they make their account). Paste and join
//     reads the clipboard. A friend's Den (its link has the Den's login after "~~", or a DEN-… code) still needs the
//     game on a computer: the Den's world isn't in the browser game's world.
//   * Mods & Addons: Discover only, to browse; the install buttons are hidden. Each card says whether it works on the
//     phone: an addon is added in the browser game itself (its dock's Your addons, from the addon's .zip), a realm mod
//     works on a realm that runs it, and a client mod, a look or a theme needs the game on a computer.
//   * Pages that need a computer (game folders, Host, Your Den, Single player, Agents, Settings, Find your files) never
//     open: a button or a remembered page that leads there lands on Play instead.
//
//   * Play solo on this phone (docs/PHONE-SOLO.md): the same game page with ?go=solo:phone, through the site's Play
//     (the realm list's "Play solo on this device"): the page runs its own server (Den Lite in a worker), no realm.
//
//   window.LeylinePhone = { on, play, solo, join, parseInvite, go }   go(url): how PLAY leaves for the game (the tests replace it)
(function () {
  "use strict";
  const $ = (id) => document.getElementById(id);
  const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args);
  // the pages a phone has; every other one needs a computer
  const PAGES = ["phone", "realms", "mods", "account", "forum", "news", "modes", "help"];
  const TAB_OF = { phone: "phone", realms: "realms", modes: "realms", mods: "mods", account: "account", forum: "phone", news: "phone", help: "account" };
  const INSTALL = /^(Install|Installed ✓|Update|Use|In use|Remove|Uninstall|Turn on|Turn off)$/;
  const DEN = /^DEN-|\/den\/|leyline:\/\/den\//i;
  const PC_NOTE = "Browse here. Addons work on your phone: add an addon's .zip in the game's Your addons. Mods, loadouts and looks install into a game folder, on your computer.";
  const ADDR = window.LeylineAddresses || {};
  const RELAYS = [ADDR.RELAY_HOST || "relay.leylinelauncher.com", ADDR.OLD_RELAY_HOST || "relay.openvanilla.app"];
  const REALM_ID = /^[a-z2-7]{16}$/;
  const BROWSER_BUILD = "3.3.5a";   // the browser game's client: the only realms it can play
  // what each kind of mod does on a phone (catalog.rs kinds; looks and themes are their own tabs)
  const FIT = {
    ui: ["yes", "Works on your phone: add its .zip in the game's Your addons"],
    realm: ["yes", "Works on your phone, on a realm that runs it"],
    both: ["part", "On your phone: the realm part only. The rest needs a computer"],
    client: ["no", "Needs the game on a computer"],
    web: ["yes", "Works on your phone, in the browser"],
    den: ["no", "Needs the game on a computer"],
  };
  let browserUrl = "https://leylinelauncher.com/wrath";
  let on = false;

  const Phone = (window.LeylinePhone = {
    get on() { return on; },
    go: (url) => window.location.assign(url),
    play,
    solo,
    join,
    parseInvite,
  });

  function nav() { return window.LeylineNav; }
  function show(page, tab) { if (nav()) nav().show(page, false, tab); }

  function setTab(page) {
    const t = TAB_OF[page] || "phone";
    document.querySelectorAll("#phone-tabs .ph-tab").forEach((b) => {
      const sel = b.dataset.phTab === t;
      if (sel) b.setAttribute("aria-current", "page"); else b.removeAttribute("aria-current");
    });
  }

  // WebGPU: the browser game draws with it (a web view on an older phone, or one with it switched off, has none)
  function canDraw() { return !!(navigator && navigator.gpu); }

  // PLAY: the website's Play page in this app, full screen; or Open in Chrome when this web view can't draw it
  function play() { return goGame(browserUrl); }

  // Solo on this phone: the site's Play with go=solo:phone (signs in first when needed, then the page plays solo)
  function soloUrl() {
    try { return new URL("/wrath/play?mini=mini&go=solo:phone", browserUrl).href; } catch (_) { return "https://leylinelauncher.com/wrath/play?mini=mini&go=solo:phone"; }
  }
  function solo() { return goGame(soloUrl()); }

  // a line on the Realms page (rl-status is the page's own status line), or on Play's Join a friend
  function realmsSay(text, id) {
    const s = $(id || "rl-status");
    if (!s) return;
    s.textContent = text;
    s.className = (id ? "ph-say " : "status ") + "warn";
  }

  // The site's address for a page of its own (the app's browser_url decides the site)
  function siteUrl(rel) {
    try { return new URL(rel, browserUrl).href; } catch (_) { return "https://leylinelauncher.com" + rel; }
  }
  // the browser game, full screen in the app; no WebGPU: the one line and Open in Chrome
  function goGame(url) {
    // offline: the game page comes from the website, so say so here instead of a blank page (audit 12 Nov)
    const off = $("ph-offline");
    if (navigator.onLine === false) {
      if (off) {
        off.hidden = false;
        off.textContent = url.includes("go=solo")
          ? "You're offline. Play solo opens from leylinelauncher.com, so it needs the internet: connect, then tap it again. Your world stays on this phone."
          : "You're offline. The game opens from leylinelauncher.com: connect to Wi-Fi or mobile data, then tap Play again.";
      }
      show("phone");
      return false;
    }
    if (off) off.hidden = true;
    const no = $("ph-nogpu");
    if (!canDraw()) {
      if (no) no.hidden = false;
      show("phone");
      return false;
    }
    if (no) no.hidden = true;
    Phone.go(url);
    return true;
  }

  // A friend's invite as {rid, invite, den}, or {den: true} for a Den code, or null. The same shapes as the desktop's
  // realms.rs parse_invite (https://<relay>/join/<id>#<secret>[~<host key>][~~<account>.<password>]) and its
  // leyline://join?link=… form; only Leyline's own relays.
  function parseInvite(text) {
    let t = String(text || "").trim();
    if (!t || t.length > 600) return null;
    if (/^DEN-/i.test(t) || /^leyline:\/\/den\//i.test(t)) return { den: true };
    const ll = /^leyline:\/\/join\?link=([^&\s]+)/i.exec(t);
    if (ll) { try { t = decodeURIComponent(ll[1]); } catch (_) { return null; } }
    const m = /^https:\/\/([a-z0-9.-]+)\/join\/([a-z2-7]{16})\/?#([^\s]*)$/i.exec(t);
    if (!m || !RELAYS.includes(m[1].toLowerCase()) || !REALM_ID.test(m[2])) return null;
    let frag = m[3];
    try { frag = decodeURIComponent(frag); } catch (_) { return null; }
    const invite = frag.split("~")[0];
    if (!/^[A-Z2-9]{16,40}$/.test(invite)) return null;
    return { rid: m[2], invite, den: frag.includes("~~") };
  }

  // Join a friend's realm: in the app, the browser game hops to it with the invite (Den: on a computer)
  function join(text, sayId) {
    const v = parseInvite(text);
    if (!v) {
      realmsSay(DEN.test(String(text || "").trim())
        ? "That's an invite to a friend's Den. Dens run in the game on a computer: open Leyline on your PC and paste it under Realms › Got an invite link?"
        : "That doesn't look like a Leyline invite link. It starts with https://relay.leylinelauncher.com/join/ : ask your friend to send it again.", sayId);
      return false;
    }
    if (v.den) {
      realmsSay("That's an invite to a friend's Den. Dens run in the game on a computer: open Leyline on your PC and paste it under Realms › Got an invite link?", sayId);
      return false;
    }
    return goGame(siteUrl("/wrath?go=r:" + v.rid) + "#inv=" + encodeURIComponent(v.invite));
  }

  // Paste and join: the clipboard's invite link, in one tap (a web view that won't read it: the box gets the focus)
  async function pasteJoin() {
    const box = $("ph-invite");
    let text = "";
    try { text = (navigator.clipboard && navigator.clipboard.readText) ? await navigator.clipboard.readText() : ""; } catch (_) { text = ""; }
    if (!text || !parseInvite(text)) {
      if (box && box.value.trim()) return join(box.value, "ph-join-say");
      if (box) box.focus();
      realmsSay(text ? "What you copied isn't a Leyline invite link. Paste the link your friend sent in the box, then Join."
        : "Paste the invite link your friend sent in the box, then Join.", "ph-join-say");
      return false;
    }
    if (box) box.value = text.trim();
    return join(text, "ph-join-say");
  }

  // the Realms list: the official realm's Play is the browser game, and so is a community realm for the browser
  // game's client on Leyline's relay; any other needs the game on a computer
  function fixRealms() {
    document.querySelectorAll("#rl-list .realm-card .rl-play").forEach((b) => {
      const card = b.closest(".realm-card");
      const official = !!card.querySelector(".rl-bdg.o");
      const hop = !official && REALM_ID.test(card.dataset.realmId || "") && card.dataset.build === BROWSER_BUILD;
      b.disabled = false;
      b.dataset.phone = official ? "browser" : hop ? "hop" : "pc";
      b.title = official || hop ? "Play in the app, full screen" : "This realm plays with your own game on a computer";
      if (hop) b.dataset.rid = card.dataset.realmId;
    });
  }
  function onRealmsClick(e) {
    const b = e.target.closest && e.target.closest("#rl-list .rl-play, #rl-join");
    if (!b) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (b.id === "rl-join") {
      join((($("rl-invite") || {}).value || "").trim());
      return;
    }
    if (b.dataset.phone === "browser") play();
    else if (b.dataset.phone === "hop" && REALM_ID.test(b.dataset.rid || "")) goGame(siteUrl("/wrath?go=r:" + b.dataset.rid));
    else realmsSay("This community realm plays with your own game on a computer: open Leyline on your PC to join it.");
  }

  // Mods & Addons: browsing only (the install, update and use buttons are hidden; one line says why)
  function fixMods() {
    const page = $("page-mods");
    if (!page) return;
    page.querySelectorAll("button").forEach((b) => {
      if (!b.dataset.phoneHidden && INSTALL.test(b.textContent.trim())) { b.hidden = true; b.dataset.phoneHidden = "1"; }
    });
    // each card: does it work on a phone?
    page.querySelectorAll(".dc-card[data-id]").forEach((card) => {
      if (card.querySelector(".ph-fit")) return;
      const f = FIT[card.dataset.kind || ""];
      if (!f) return;
      const line = document.createElement("p");
      line.className = "ph-fit " + f[0];
      line.textContent = f[1];
      const body = card.querySelector(".dc-body") || card;
      body.append(line);
    });
    if (!$("ph-mods-note")) {
      const n = document.createElement("p");
      n.id = "ph-mods-note";
      n.className = "ph-note";
      n.setAttribute("role", "note");
      n.textContent = PC_NOTE;
      const head = page.querySelector(".panel-head") || page.firstElementChild;
      if (head) head.after(n); else page.prepend(n);
    }
  }

  function start() {
    if (on) return;
    on = true;
    document.querySelectorAll("#phone-tabs .ph-tab").forEach((b) => b.addEventListener("click", () => {
      const t = b.dataset.phTab;
      show(t, t === "mods" ? "discover" : undefined);
    }));
    document.querySelectorAll("[data-ph-go]").forEach((b) => b.addEventListener("click", () => show(b.dataset.phGo)));
    document.querySelectorAll("[data-ph-link]").forEach((b) => b.addEventListener("click", () => {
      invoke("open_link", { name: b.dataset.phLink }).catch(() => {});
    }));
    const pb = $("ph-play");
    if (pb) pb.addEventListener("click", play);
    // back online: the offline line goes by itself
    window.addEventListener("online", () => { const off = $("ph-offline"); if (off) off.hidden = true; });
    const sb = $("ph-solo");
    if (sb) sb.addEventListener("click", solo);
    const pj = $("ph-paste-join");
    if (pj) pj.addEventListener("click", pasteJoin);
    const jb = $("ph-join");
    if (jb) jb.addEventListener("click", () => join((($("ph-invite") || {}).value || "").trim(), "ph-join-say"));
    const ib = $("ph-invite");
    if (ib) ib.addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); join(ib.value.trim(), "ph-join-say"); } });
    const ch = $("ph-chrome");
    if (ch) ch.addEventListener("click", () => {
      invoke("open_browser_play").catch(() => {
        const no = $("ph-nogpu");
        if (no) no.querySelector("p").textContent = "Chrome didn't open. Open leylinelauncher.com/wrath in Chrome yourself.";
      });
    });
    // the official realm on the play bar's PLAY (a guide or another page may still press it): the browser game too
    const form = $("play-form");
    if (form) form.addEventListener("submit", (e) => { e.preventDefault(); e.stopImmediatePropagation(); play(); }, true);
    const rl = $("page-realms");
    if (rl) rl.addEventListener("click", onRealmsClick, true);
    const list = $("rl-list");
    if (list) new MutationObserver(fixRealms).observe(list, { childList: true, subtree: true });
    const mods = $("page-mods");
    if (mods) new MutationObserver(fixMods).observe(mods, { childList: true, subtree: true });
    fixRealms();
    fixMods();
    invoke("get_state").then((s) => {
      if (s && typeof s.browser_url === "string" && /^https:\/\/[a-z0-9.-]+\/[\w/-]*$/i.test(s.browser_url)) browserUrl = s.browser_url;
    }).catch(() => {});
    // the pages a phone has: anything else (a remembered page, a button that leads to Host or Your Den) lands on Play;
    // Mods & Addons stays on Discover
    document.addEventListener("leyline:page", (e) => {
      const d = e.detail || {};
      if (!PAGES.includes(d.page)) { show("phone"); return; }
      if (d.page === "mods" && d.tab && d.tab !== "discover") { show("mods", "discover"); return; }
      setTab(d.page);
    });
    const cur = nav() ? nav().current().page : "play";
    if (PAGES.includes(cur) && cur !== "mods") { setTab(cur); show(cur); } else show(cur === "mods" ? "mods" : "phone", cur === "mods" ? "discover" : undefined);
    // the first-run tour shows the desktop's menus: guide.js leaves it out on a phone; one already open closes
    if (window.LeylineGuide && typeof window.LeylineGuide.endTour === "function") { try { window.LeylineGuide.endTour(); } catch (_) { /* none open */ } }
  }

  const P = window.LeylinePlatform;
  if (P && P.mobile) start();
  document.addEventListener("leyline:platform", (e) => { if (e.detail && e.detail.mobile) start(); });
})();
