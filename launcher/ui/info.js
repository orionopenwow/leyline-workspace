// Leyline: the "i" button (owner 12 Nov: "a lower information bubble or information eye icon somewhere within the
// launcher to ask more about what is on the screen, or go to our docs to learn everything"). A small round icon at the
// main window's bottom right, always there and reachable by keyboard; it opens a bubble that says what the page on
// screen is in two to four lines, with Learn more: that page's docs on the website (open_link, the fixed links in
// lib.rs LINKS; the docs pages are stream/docs.py PAGES). Esc or a click outside closes it.
// Loaded after app.js. Text only (textContent).
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const mk = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };

  // page id (index.html data-page) → { title, lines, link (lib.rs LINKS name), more (the button's words) }
  // TODO(owner): his words for each page
  const PAGES = {
    play: { title: "Home", link: "gettingstarted", lines: [
      "Home is where you start. Play single player runs a world on this PC just for you.",
      "The realm rows take you online with other players, and Your Den is your own place to build.",
    ] },
    simple: { title: "Simple mode", link: "gettingstarted", lines: [
      "The fewest buttons: Play, Host, Visit and Back up.",
      "Switch back to the full launcher any time from the menu.",
    ] },
    single: { title: "Single player", link: "docsingle", lines: [
      "Single player runs a small game server on your PC, made only for you, with bot companions if you want them.",
      "The first setup takes 5–20 minutes; after that your world starts in seconds.",
      "Choose companions, dungeon buddies and how busy the world is in the settings below.",
      "Friends: invite one to your world, and they keep their character there for next time.",
    ], also: { label: "Playing with friends", link: "hostfriends" } },
    realms: { title: "Realms", link: "docsplay", lines: [
      "Realms are worlds you play online with other people.",
      "Official realms are run by Leyline; Community realms are hosted by players over the relay.",
      "Pick one and press Play: Leyline logs you in.",
    ] },
    modes: { title: "Modes", link: "docsplay", lines: [
      "Modes are other ways to play: quick fights, challenges, and modes other players made.",
      "Each card says what it is and how to start it.",
    ] },
    mods: { title: "Mods & Addons", link: "docsmods", lines: [
      "Mods and addons change how the game looks and plays.",
      "Discover shows what's available; Installed shows what's in your game folder, and removes anything in one click.",
    ] },
    host: { title: "Host", link: "hostarealm", lines: [
      "Host runs a realm on your PC, for friends or for everyone.",
      "Pick a mode, choose Private or Public, and Leyline sets it up. Invite a friend sends one link.",
    ] },
    forum: { title: "Forum", link: "docslauncher", lines: [
      "The forum is where players talk: questions, guides, bugs and ideas.",
      "You read it here; posting opens the website.",
    ] },
    news: { title: "News", link: "docslauncher", lines: [
      "News from Leyline: updates, new features and events.",
    ] },
    settings: { title: "Settings", link: "docslauncher", lines: [
      "Settings for Leyline itself: your game folders, how it looks, updates and the advanced options.",
    ] },
  };
  // a part of a page with its own bubble while it is on screen (its element id → the same shape as PAGES)
  const SECTIONS = {
    spf: { page: "single", title: "Friends", link: "hostfriends", more: "Playing with friends", lines: [
      "Invite a friend: start your world, press Invite a friend and send the link. They keep their character on your world for next time.",
      "Friends with a character on your world: Trust lets them come back without a new invite. Remove keeps their characters unless you delete them.",
      "Your friends' worlds: Join again needs no new link while your friend shares their world.",
      "Your friends list: add a friend with a friend code, or someone on your own network. It's signed by you both and kept on your PCs, with no server.",
    ], also: { label: "Single player docs", link: "docsingle" } },
  };
  const FALLBACK = { title: "Leyline", link: "docs", more: "Read the docs", lines: [
    "Leyline's docs explain every page, step by step.",
  ] };

  let page = "";
  const b = mk("button", "ll-info", "i");
  b.type = "button";
  b.id = "ll-info";
  b.setAttribute("aria-label", "About this page");
  b.setAttribute("aria-haspopup", "dialog");
  b.setAttribute("aria-expanded", "false");
  b.setAttribute("aria-controls", "ll-info-b");
  b.title = "About this page";
  const bubble = mk("div", "ll-info-b");
  bubble.id = "ll-info-b";
  bubble.setAttribute("role", "dialog");
  bubble.setAttribute("aria-labelledby", "ll-info-t");
  bubble.hidden = true;
  document.body.append(b, bubble);

  const current = () => {
    if (page) return page;
    const on = document.querySelector(".page.on");
    return on && on.id ? on.id.replace(/^page-/, "") : "";
  };
  // the section most on screen, when the page has one (the Friends area of Single player)
  function sectionOf(p) {
    for (const [id, sec] of Object.entries(SECTIONS)) {
      if (sec.page !== p) continue;
      const el = document.getElementById(id);
      if (!el || el.hidden || !el.getClientRects().length) continue;
      if (el.contains(document.activeElement) && document.activeElement !== document.body) return id;
      const r = el.getBoundingClientRect();
      const h = window.innerHeight || document.documentElement.clientHeight;
      if (r.top < h * 0.5 && r.bottom > h * 0.35) return id;
    }
    return "";
  }
  let section = "";
  const infoOf = (p) => (section && SECTIONS[section]) || PAGES[p] || FALLBACK;

  function fill() {
    const i = infoOf(current());
    const h = mk("h3", "", i.title);
    h.id = "ll-info-t";
    const nodes = [h, ...i.lines.slice(0, 4).map((l) => mk("p", "", l))];
    const more = mk("button", "btn small primary", i.more || "Learn more");
    more.type = "button";
    more.id = "ll-info-more";
    more.dataset.link = i.link;
    const note = mk("span", "ll-info-note", "");
    note.id = "ll-info-note";
    more.addEventListener("click", async () => {
      try { await call("open_link", { name: i.link }); close(false); }
      catch (_) {
        // an older core without that page's link: the docs' front page
        try { await call("open_link", { name: "docs" }); close(false); } catch (e) { note.textContent = typeof e === "string" ? e : "The browser didn't open."; }
      }
    });
    const r = mk("div", "ll-info-row");
    r.append(more);
    if (i.also) {
      const also = mk("button", "btn small", i.also.label);
      also.type = "button";
      also.id = "ll-info-also";
      also.dataset.link = i.also.link;
      also.addEventListener("click", async () => {
        try { await call("open_link", { name: i.also.link }); close(false); } catch (e) { note.textContent = typeof e === "string" ? e : "The browser didn't open."; }
      });
      r.append(also);
    }
    r.append(note);
    nodes.push(r);
    bubble.replaceChildren(...nodes);
    bubble.dataset.page = current() || "";
    bubble.dataset.section = section;
  }
  function open() {
    section = sectionOf(current());
    fill();
    bubble.hidden = false;
    b.setAttribute("aria-expanded", "true");
    const m = document.getElementById("ll-info-more");
    if (m) m.focus();
  }
  function close(focusBack) {
    if (bubble.hidden) return;
    bubble.hidden = true;
    b.setAttribute("aria-expanded", "false");
    if (focusBack) b.focus();
  }
  b.addEventListener("click", () => (bubble.hidden ? open() : close(true)));
  document.addEventListener("keydown", (e) => { if (e.key === "Escape" && !bubble.hidden) { e.stopPropagation(); close(true); } }, true);
  document.addEventListener("mousedown", (e) => { if (!bubble.hidden && !bubble.contains(e.target) && !b.contains(e.target)) close(false); });
  document.addEventListener("leyline:page", (e) => {
    page = (e.detail && e.detail.page) || "";
    section = "";
    if (!bubble.hidden) fill();
  });

  window.LeylineInfo = { open, close: () => close(false), PAGES, SECTIONS, FALLBACK, page: current, section: () => section };
})();
