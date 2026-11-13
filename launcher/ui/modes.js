// Leyline Launcher: the Modes page. Every game mode one click away, over the Rust core's modes commands
// (src-tauri/src/modes.rs), which read the site's catalog (https://leylinelauncher.com/wrath/modes.json, cached,
// with a copy built into Leyline).
//
// Loaded before app.js: it adds its own page, so app.js's navigation and its client pickers (.client-pick) pick it up
// like the others. Its side menu item is under Realms (4 Nov: the website's menu gets a Modes page, and the launcher's
// menu matches the website's); Home's featured gallery, Realms › MODES and show("modes") (window.LeylineNav) open it too. PLAY follows the Home page's play setting (localStorage
// "leyline.mode"): in the browser it opens the mode's link; on the desktop it writes the pick into your 3.3.5a
// client (mode_prepare_desktop) and starts it on the official realm (app.rs "play"). The UI only ever names a
// mode id: every address is built by the core. Everything from the catalog is shown with textContent.
(function () {
  "use strict";
  const tauri = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core.invoke : null;
  const invoke = tauri || (async () => { throw "This is a preview page: open it in the Leyline app."; });
  // never a raw error to the player (friendly.js): the error goes to the console, the player gets a plain line
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (fb || "Something went wrong. Try again."));
  const $ = (id) => document.getElementById(id);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const KIND = { realm: "On our realm", offline: "No server", "realm-type": "Its own realm type", "in-game": "In game" };
  const OFFICIAL = window.LeylineAddresses.SITE_HOST;
  // Key art shipped with the launcher (art/modes/), by mode id: on the mode's card and, for the featured mode, as a
  // banner on Home. A mode without art, or art that fails to load, simply has none (the card and Home as before).
  const ART = {
    "azeroth-survivors": { src: "art/modes/azeroth-survivors.webp", src2: "art/modes/azeroth-survivors@1.5x.webp", w: 483, h: 298,
      // the cards show a short loop of play (owner 2 Nov: "a small gif of gameplay for the site and launcher card");
      // Home's banner keeps the key art with its lettering
      play: "art/modes/azeroth-survivors-play.webp", playAlt: "Azeroth Survivors in play: a mage in the middle of a horde, spells flying.",
      title: "Azeroth Survivors", alt: "Azeroth Survivors key art: the title over a band of heroes and a moonlit castle." },
  };
  // Duels and Boss Rush only (owner 12 Nov: "remove the silly modes from modes and make them duels and boss rush only as
  // we build out the larger feature set"). The catalog marks the others "listed": false (stream/playmodes.json): the
  // site's /wrath/modes.json leaves them out, but the copy built into Leyline still has them and the core does not pass
  // the flag on, so the same ids are left out here (launcher/tests/modes.browser.test.cjs keeps the two equal). To bring
  // one back, delete its "listed": false in the catalog and its id here. window.LEYLINE_MODES_HIDDEN: a test's own list.
  const HIDDEN = new Set(Array.isArray(window.LEYLINE_MODES_HIDDEN) ? window.LEYLINE_MODES_HIDDEN : [
    "minigolf", "bumper-balls", "coin-dash", "community", "brawl", "azeroth-survivors", "hordes-of-azeroth",
    "arena-1v1", "keystones", "transmog", "hardcore"]);
  const listed = (m) => !!m && m.listed !== false && !HIDDEN.has(m.id);
  const ORDER = ["arena-queue", "frozen-throne"];   // TODO(owner): the order on the page
  const byOrder = (a, b) => (ORDER.indexOf(a.id) + 1 || 99) - (ORDER.indexOf(b.id) + 1 || 99);
  // The two modes' fuller cards: what it is, how to start, the Play label, and the leaderboard mount (data-board). A
  // listed mode without an entry gets the plain card. TODO(owner): the words, yours (the site has the same steps:
  // stream/playmodes.py DETAILS).
  const DETAILS = {
    "arena-queue": { kick: "1 v 1 · Level 80", play: "Queue now", board: "duels",
      how: ["Play logs you in and puts your character in the 1v1 queue at level 80. New characters can boost with .boost.",
        "Or meet at the duel spot in Dalaran: target someone and type /duel.",
        "Rated games: type .q1v1 rated in the game."] },
    "frozen-throne": { kick: "Raid finale · Companions fill", play: "Fight now", board: "bossrush", local: true,
      how: ["Fight now logs you in and takes your hero straight onto the platform; companions fill the raid of 10.",
        "Or pick any fight of any raid and play it here: Boss Rush on this PC, with your own clock and board.",
        "In the game, /bf opens the Boss fights window: Heroic, or queue with players."] },
  };
  // TODO(owner): the line under the two modes, your words
  const MORE = "More modes are on the way as the bigger features land. News comes first on the Discord.";
  const HOST_INVITE = false;   // "Run it with friends on your own world": for the minigames; back on with them
  const FEATURED = HIDDEN.has("azeroth-survivors") ? null : "azeroth-survivors";   // TODO(owner): the mode featured on Home (none while it is hidden)
  function artImg(id, cls) {
    const a = ART[id];
    if (!a) return null;
    const img = el("img", cls);
    const play = cls !== "hs-art" && a.play;   // cards: the loop of play; Home's banner: the key art
    img.src = play || a.src;
    if (a.src2 && !play) img.srcset = `${a.src} 1x, ${a.src2} 1.5x`;
    img.width = a.w; img.height = a.h; img.alt = (play && a.playAlt) || a.alt; img.decoding = "async";
    return img;
  }

  // ---------------------------------------------------------------- the page (no menu item)
  function openPage() {
    if (window.LeylineNav) { window.LeylineNav.show("modes"); return; }
    document.querySelectorAll(".page").forEach((p) => p.classList.toggle("on", p === page));
  }
  const page = el("section", "page");
  page.id = "page-modes";
  page.innerHTML = `
    <div class="panel wide md">
      <h2>Modes</h2>
      <p class="lede">Duels and Boss Rush. PLAY signs you in and starts the game once your character is in the world.</p>
      <div class="toolbar">
        <label class="inline">Play with <select class="client-pick" data-for="modes"></select></label>
        <span class="md-how" id="md-how"></span>
        <button type="button" class="btn small" id="md-refresh">Refresh</button>
      </div>
      <p class="net-line" id="md-net" aria-live="polite"></p>
      <p class="status" id="md-status" role="status"></p>
      <div class="md-list" id="md-list"></div>
      <p class="md-more" id="md-more"></p>
    </div>
    <!-- Leaderboards (boards.js mounts here: Boss Rush and Duels, signed by the realms; docs/LEADERBOARDS.md) -->
    <div class="panel wide lb-panel" id="md-boards" data-board="bossrush"></div>`;
  const main = document.querySelector("main");
  const realmsPage = document.getElementById("page-realms");
  if (realmsPage) realmsPage.after(page); else if (main) main.appendChild(page);

  function say(text, kind) {
    const s = $("md-status");
    s.textContent = text || "";
    s.className = "status" + (kind ? " " + kind : "");
  }
  function browserMode() {
    try { return localStorage.getItem("leyline.mode") === "browser"; } catch (_) { return false; }
  }
  function renderHow() {
    $("md-how").textContent = browserMode() ? "PLAY opens your web browser (change it in the play bar)." : "PLAY starts your game client (change it in the play bar).";
  }

  function playersText(p) {
    if (p.max === null || p.max === undefined) return p.min === 1 ? "Any number of players" : `${p.min}+ players`;
    if (p.min === p.max) return p.min === 1 ? "1 player" : `${p.min} players`;
    return `${p.min}-${p.max} players`;
  }

  // ---------------------------------------------------------------- the list
  let modes = [];
  let every = [];   // the list as it came, hidden modes too (their names, for a realm that still lists one)
  let loaded = false;

  async function load() {
    loaded = true;
    renderHow();
    $("md-net").textContent = "Loading…";
    let l;
    try { l = await invoke("modes_list"); }
    catch (e) { loaded = false; $("md-net").textContent = ""; say(oops(e), "bad"); return; }   // opening the page tries again
    every = Array.isArray(l.modes) ? l.modes : [];
    modes = every.filter(listed).sort(byOrder);
    const live = modes.filter((m) => m.status === "live").length;
    let line = `${live} to play` + (modes.length - live ? ` · ${modes.length - live} coming soon` : "");
    if (l.builtin) line += " · offline: the list built into Leyline";
    else if (l.from_cache) line += " · offline: the last list Leyline saw";
    $("md-net").textContent = line;
    render();
    document.dispatchEvent(new CustomEvent("leyline:modes"));   // Home's Game modes row (home.js)
  }
  // Home asks for the list once at start (the same modes_list the page makes; the core caches it).
  function ensure() { if (!loaded) load(); }

  function card(m) {
    const c = el("div", "realm-card md-card");
    c.dataset.mode = m.id;
    const art = artImg(m.id, "md-art");
    if (art) {
      c.classList.add("has-art");
      art.addEventListener("error", () => { art.remove(); c.classList.remove("has-art"); });
      c.append(art);
    }
    const d = DETAILS[m.id] || null;
    if (d) { c.classList.add("md-feature"); c.dataset.board = d.board; }
    if (d && d.kick) c.append(el("p", "md-kick", d.kick));
    const top = el("div", "top");
    top.append(el("span", "name", m.title));
    if (m.status === "coming") top.append(el("span", "tag warn", "Coming soon"));
    c.append(top);
    const tags = el("div", "tags");
    tags.append(el("span", "tag", KIND[m.kind] || m.kind), el("span", "tag", playersText(m.players)));
    if (m.credit) tags.append(el("span", "tag gold", "Community made"));
    c.append(tags, el("p", "desc", m.line));
    if (d && d.how) {
      c.append(el("p", "md-sub", "How to start"));
      const ol = el("ol", "md-steps");
      for (const x of d.how) ol.append(el("li", "", x));
      c.append(ol);
    }
    if (d && d.board) {
      // The leaderboard mount (docs/MODES.md, "Leaderboard mounts"): a leaderboards module fills it; until then it says
      // what will be there. It is rebuilt with the list: fill it again on "leyline:modes".
      const box = el("div", "md-board");
      box.id = "md-board-" + d.board;
      box.dataset.board = d.board;
      box.setAttribute("aria-live", "polite");
      const row = el("div", "md-board-row");
      for (const [k, v] of [["Your best", "—"], ["Your rank", "—"]]) {
        const cell = el("span", "md-stat");
        cell.append(el("small", "", k), el("b", "", v));
        row.append(cell);
      }
      box.append(row, el("p", "md-board-note", "The full leaderboard is below."));
      c.append(box);
    }
    if (m.kind === "in-game" && m.start && m.start.cmd) {
      // an NPC in Dalaran: PLAY opens the game, this line says how to find it there
      const how = el("p", "md-cmd");
      how.append(document.createTextNode("In game, type "), el("code", "", m.start.cmd), document.createTextNode(" to find it."));
      c.append(how);
    }
    if (m.credit) {
      const cr = el("p", "credit");
      cr.append(document.createTextNode(`By ${m.credit.by}${m.credit.note ? " · " + m.credit.note : ""} · ${m.credit.license}`));
      c.append(cr);
    }
    const acts = el("div", "acts");
    if (m.status === "live" && (m.kind === "realm" || m.kind === "offline" || m.kind === "in-game")) {
      const b = el("button", "btn small md-play" + (d ? " primary" : ""), (d && d.play) || "Play");
      b.type = "button";
      b.addEventListener("click", () => play(m, b));
      acts.append(b);
    } else if (m.status === "live" && m.kind === "realm-type") {
      const b = el("button", "btn small", "Host it");
      b.type = "button";
      b.title = "This mode is a realm type: host one with the Host pack, or find one on Realms.";
      b.addEventListener("click", () => {
        const go = document.querySelector('.nav[data-page="host"]');
        if (go) go.click();
      });
      acts.append(b);
    } else if (m.status !== "live") {
      // Not playable yet: say so where PLAY would be, so it never looks like a broken button.
      const b = el("button", "btn small md-soon", "Coming soon");
      b.type = "button";
      b.disabled = true;
      b.title = `${m.title} isn't playable yet. It shows here when it is.`;
      acts.append(b);
    }
    const links = (m.credit ? [m.credit.source].concat(m.credit.more || []) : []);
    links.forEach((u, i) => {
      const b = el("button", "link-s", i === 0 ? "Source ›" : "More source ›");
      b.type = "button";
      b.title = u;
      b.addEventListener("click", async () => {
        try { await invoke("mode_open_credit", { id: m.id, n: i }); } catch (e) { say(oops(e), "bad"); }
      });
      acts.append(b);
    });
    if (d && d.local && document.getElementById("page-bossrush")) {
      // Boss Rush on this PC (bossrush.js): any fight of any raid, on its own small realm here
      const b = el("button", "btn small md-local", "Boss Rush on this PC");
      b.type = "button";
      b.title = "Pick any fight of any raid and play it on this PC";
      b.addEventListener("click", () => { if (window.LeylineNav) window.LeylineNav.show("bossrush"); });
      acts.append(b);
    }
    // Play solo first, beside Play (solo.js): on this PC, with your own game files; or why it's online only
    const solo = window.LeylineSolo ? window.LeylineSolo.attach(acts, null, window.LeylineSolo.forMode(m)) : null;
    if (acts.childNodes.length) c.append(acts);
    if (solo) c.append(solo.note);
    return c;
  }

  // One line under the list, once a day: run these with friends on your own world (the Host page).
  const INVITE_KEY = "leyline.hostInvite.modes";
  let inviteToday = null;   // shown in this session already: it stays through a Refresh
  function hostInvite() {
    if (!HOST_INVITE) return null;
    const today = new Date().toISOString().slice(0, 10);
    if (inviteToday !== today) {
      let last = null;
      try { last = localStorage.getItem(INVITE_KEY); } catch (_) { /* no storage: show it */ }
      if (last === today) return null;
      try { localStorage.setItem(INVITE_KEY, today); } catch (_) { /* fine */ }
      inviteToday = today;
    }
    const p = el("p", "md-invite");
    p.append(document.createTextNode("Liked a mode? Run it with friends on your own world, your rules. "));
    const b = el("button", "link-s", "Host your own ›");
    b.type = "button";
    b.addEventListener("click", () => { const go = document.querySelector('.nav[data-page="host"]'); if (go) go.click(); });
    p.append(b);
    return p;
  }

  function render() {
    const box = $("md-list");
    box.replaceChildren(...modes.map(card));
    if (!modes.length) box.append(el("p", "hint", "No modes listed right now."));
    const old = document.getElementById("md-invite");
    if (old) old.remove();
    const inv = modes.length ? hostInvite() : null;
    if (inv) { inv.id = "md-invite"; box.after(inv); }
    $("md-more").textContent = modes.length ? MORE : "";
  }

  // ---------------------------------------------------------------- PLAY
  function pickedClient(state) {
    const sel = page.querySelector(".client-pick");
    const id = (sel && sel.value) || state.settings.selected_client;
    return state.settings.clients.find((c) => c.id === id) || null;
  }

  async function play(m, btn) {
    // the official realm signs in with the Leyline account: the sign-in screen first, then this goes on (gate.js)
    const G = window.LeylineGate;
    if (m.kind !== "offline" && !browserMode() && G && !G.ready()) { G.need(`${m.title} is on the official realm, which signs you in with your Leyline account. Log in, and it goes on by itself.`, () => play(m, btn)); return; }
    btn.disabled = true;
    try {
      if (m.kind === "offline" || browserMode()) {
        say(await invoke("mode_open_browser", { id: m.id }), "ok");
        return;
      }
      const state = await invoke("get_state");
      const c = pickedClient(state);
      if (!c) { say("Add your game folder first, or pick Play in your browser in the play bar.", "bad"); return; }
      if (c.build !== "3.3.5a") { say(`${m.title} plays with your Wrath game folder: pick it in the play bar, or add it first.`, "bad"); return; }
      if (window.LeylineAddons) await window.LeylineAddons.beforePlay(c);   // modes want the Leyline addons (addons.js)
      say("Starting…");
      // an in-game mode has no pick to write: the game itself is the way in
      const ready = m.kind === "in-game" ? `In game, type ${m.start.cmd}.` : await invoke("mode_prepare_desktop", { client: c.id, id: m.id });
      const started = await invoke("play", { client: c.id, realm: state.official_realm || OFFICIAL });
      say(`${started} ${ready}`, "ok");
      if (window.LeylineAddons) window.LeylineAddons.afterPlay();
    } catch (e) {
      say(oops(e), "bad");
    } finally {
      btn.disabled = false;
    }
  }

  // ---------------------------------------------------------------- open one mode: this page with its card flashed (Home's
  // gallery and its Game modes row call it; home.js builds the featured mode's slide from ART and artImg).
  function show(id) {
    openPage();
    const go = () => {
      const c = document.querySelector(`#md-list .md-card[data-mode="${id}"]`);
      if (!c) return false;
      c.scrollIntoView({ block: "nearest" });
      c.classList.remove("md-flash"); void c.offsetWidth; c.classList.add("md-flash");
      return true;
    };
    if (!go()) {
      const mo = new MutationObserver(() => { if (go()) mo.disconnect(); });
      mo.observe($("md-list"), { childList: true });
      setTimeout(() => mo.disconnect(), 10000);
    }
  }

  // ---------------------------------------------------------------- start
  $("md-refresh").addEventListener("click", load);
  // whenever the page opens (show("modes"), or the page remembered from last time): load once, then keep "PLAY …" current
  new MutationObserver(() => { if (page.classList.contains("on")) { if (!loaded) load(); else renderHow(); } }).observe(page, { attributes: true, attributeFilter: ["class"] });

  // boards(): the leaderboard mounts on the page now ([data-board] duels | bossrush; docs/MODES.md)
  const boards = () => Array.from(document.querySelectorAll("#md-list .md-board[data-board]"));
  window.LeylineModes = { load, ensure, modes: () => modes, all: () => every, show, open: openPage, play, ART, FEATURED, artImg, KIND, playersText,
    listed, HIDDEN, DETAILS, boards };
})();
