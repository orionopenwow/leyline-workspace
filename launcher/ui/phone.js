// Leyline Launcher: the phone app (docs/ANDROID.md). Nothing here runs on a computer: it waits for platform.js to say
// "phone" (window.LeylinePlatform; the core's `platform` command, mobile.rs).
//
// On a phone the launcher is the same pages laid out for a small screen (phone.css), with a tab bar in place of the
// side menu and the play bar: Play (#page-phone), Realms, Mods, Account.
//   * PLAY opens the website's Play page (get_state's browser_url, the normal /wrath sign-in and Play) inside the app,
//     full screen; the phone's Back button comes back to the launcher. The game draws with WebGPU: a web view without
//     it gets one line and Open in Chrome (open_browser_play) instead of a black page.
//   * Realms: the official realm's Play is the browser game too. A community realm plays with your own game on a
//     computer, and an invite link or code to a friend's Den opens a line saying the Den needs a computer.
//   * Mods & Addons: Discover only, to browse; the install buttons are hidden and one line says installs need a computer.
//     Looks are shown for their information only.
//   * Pages that need a computer (game folders, Host, Your Den, Single player, Agents, Settings, Find your files) never
//     open: a button or a remembered page that leads there lands on Play instead.
//
//   * Play solo on this phone (docs/PHONE-SOLO.md): the same game page with ?go=solo:phone, through the site's Play
//     (the realm list's "Play solo on this device"): the page runs its own server (Den Lite in a worker), no realm.
//
//   window.LeylinePhone = { on, play, solo, go }   go(url): how PLAY leaves for the game (the tests replace it)
(function () {
  "use strict";
  const $ = (id) => document.getElementById(id);
  const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args);
  // the pages a phone has; every other one needs a computer
  const PAGES = ["phone", "realms", "mods", "account", "forum", "news", "modes", "help"];
  const TAB_OF = { phone: "phone", realms: "realms", modes: "realms", mods: "mods", account: "account", forum: "phone", news: "phone", help: "account" };
  const INSTALL = /^(Install|Installed ✓|Update|Use|In use|Remove|Uninstall|Turn on|Turn off)$/;
  const DEN = /^DEN-|\/den\/|leyline:\/\/den\//i;
  const PC_NOTE = "Browse here. Installing mods, addons, loadouts and looks needs Leyline on your computer for now.";
  let browserUrl = "https://leylinelauncher.com/wrath";
  let on = false;

  const Phone = (window.LeylinePhone = {
    get on() { return on; },
    go: (url) => window.location.assign(url),
    play,
    solo,
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
  function play() {
    const no = $("ph-nogpu");
    if (!canDraw()) {
      if (no) no.hidden = false;
      show("phone");
      return false;
    }
    if (no) no.hidden = true;
    Phone.go(browserUrl);
    return true;
  }

  // Solo on this phone: the site's Play with go=solo:phone (signs in first when needed, then the page plays solo)
  function soloUrl() {
    try { return new URL("/wrath/play?mini=mini&go=solo:phone", browserUrl).href; } catch (_) { return "https://leylinelauncher.com/wrath/play?mini=mini&go=solo:phone"; }
  }
  function solo() {
    const no = $("ph-nogpu");
    if (!canDraw()) {
      if (no) no.hidden = false;
      show("phone");
      return false;
    }
    if (no) no.hidden = true;
    Phone.go(soloUrl());
    return true;
  }

  // a line on the Realms page (rl-status is the page's own status line)
  function realmsSay(text) {
    const s = $("rl-status");
    if (!s) return;
    s.textContent = text;
    s.className = "status warn";
  }

  // the Realms list: the official realm's Play is the browser game; a community realm needs the game on a computer
  function fixRealms() {
    document.querySelectorAll("#rl-list .realm-card .rl-play").forEach((b) => {
      const official = !!b.closest(".realm-card").querySelector(".rl-bdg.o");
      b.disabled = false;
      b.dataset.phone = official ? "browser" : "pc";
      b.title = official ? "Play in the app, full screen" : "Community realms play with your own game on a computer";
    });
  }
  function onRealmsClick(e) {
    const b = e.target.closest && e.target.closest("#rl-list .rl-play, #rl-join");
    if (!b) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (b.id === "rl-join") {
      const text = (($("rl-invite") || {}).value || "").trim();
      realmsSay(DEN.test(text)
        ? "That's an invite to a friend's Den. Dens run in the game on a computer: open Leyline on your PC and paste it under Realms › Got an invite link?"
        : "Invite links play with your own game on a computer: open Leyline on your PC and paste it there.");
      return;
    }
    if (b.dataset.phone === "browser") play();
    else realmsSay("Community realms play with your own game on a computer: open Leyline on your PC to join this one.");
  }

  // Mods & Addons: browsing only (the install, update and use buttons are hidden; one line says why)
  function fixMods() {
    const page = $("page-mods");
    if (!page) return;
    page.querySelectorAll("button").forEach((b) => {
      if (!b.dataset.phoneHidden && INSTALL.test(b.textContent.trim())) { b.hidden = true; b.dataset.phoneHidden = "1"; }
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
    const sb = $("ph-solo");
    if (sb) sb.addEventListener("click", solo);
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
