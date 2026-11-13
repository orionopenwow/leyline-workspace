// Leyline Launcher UI. Plain JS over the Rust core's commands (src-tauri/src/app.rs).
"use strict";
// the site, the game and the relay on their new and old names (addresses.js; docs/DOMAIN-MOVE.md)
const ADDR = window.LeylineAddresses;

const $ = (id) => document.getElementById(id);
const el = (tag, cls, text) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
};

// Inside the app this is Tauri's invoke. Opened as a plain web page (a design preview) there is no core, so
// every call says so instead of failing silently.
const invoke = window.__TAURI__ && window.__TAURI__.core
  ? window.__TAURI__.core.invoke
  : async (cmd) => {
      if (cmd === "get_state") {
        return { version: "preview", settings: { clients: [], selected_client: null, selected_realm: ADDR.SITE_HOST,
          wine: { enabled: true, command: "wine", prefix: "" } }, official_realm: ADDR.SITE_HOST,
          browser_url: ADDR.site("/wrath"), windows: false, data_dir: "(app data folder)" };
      }
      throw "This is a preview page: open it in the Leyline app.";
    };

// What's new, shown on Home and on the News page. Newest first. The website's news feed replaces these when it loads
// (loadNews: the core's news_list, news.rs, /wrath/leyline/news.json); these are the offline copy.
const NEWS = [
  { tag: "LEYLINE LAUNCHER", title: "EVERY CLIENT, EVERY SERVER", items: [
    "Finds every game client on your PC",
    "PLAY picks the client a realm needs",
    "Updates in the background, never mid-game",
  ] },
  { tag: "GAME CLIENT", title: "CHARACTER CREATE IS BACK IN COLOUR", items: [
    "Animated race backgrounds at character select and create",
    "No more falling through ground that hasn't loaded yet",
    "Duels arena now on the realm list",
  ] },
  { tag: "LEYLINE LAUNCHER", title: "MODS, REALMS AND HOSTING", items: [
    "Install, update, turn off and remove mods for each game, or drop in your own zip",
    "Play on community realms from the realm list, and back to Wrath Unleashed in one click",
    "Host your own realm from the Host page with the Host pack",
  ] },
  { tag: "LEYLINE LAUNCHER", title: "ONE LAUNCHER FOR EVERY REALM", items: [
    "Uses the client you already have",
    "Sets the realm for you and keeps your original realmlist as a backup",
    "Or play Wrath in your browser, with nothing to install",
  ] },
];

let state = null;
let catalogNames = [];
let realmNames = [];

function say(id, text, kind, ver) {
  const s = $(id);
  s.textContent = text || "";
  if (ver) s.append(" ", el("span", "rl-ver sm", ver));   // the version as a chip, never in the sentence
  s.className = "status" + (kind ? " " + kind : "");
}
// Never a raw error to the player (friendly.js): the error goes to the console, the player gets a plain line. A line
// the core wrote for players is shown as it is; anything technical gets the line for that place.
const OOPS = {
  "play-status": "That didn't work. Try again, or ask on the Discord.",
  "cl-status": "Leyline couldn't use that folder. Pick the folder that holds the game, or see Find your game files.",
  "cat-status": "Can't load the mods. Try again.",
  "mods-status": "That mod change didn't work. Try again, or ask on the Discord.",
  "rl-status": "Can't reach the realm list right now. Try again, or ask on the Discord.",
  "host-status-line": "The host didn't answer. Try again, or see the Guide tab.",
  "set-status": "Couldn't save that. Try again.",
  "count-status": "Couldn't save that. Try again.",
  "upd-status": "Couldn't check for updates right now. Leyline tries again later.",
};
function oops(e, fallback) {
  return window.LeylineFriendly ? window.LeylineFriendly(e, fallback) : (fallback || "Something went wrong. Try again.");
}
function sayErr(id, e, kind) { say(id, oops(e, OOPS[id]), kind || "bad"); }

// ------------------------------------------------------------------ window (the title bar is ours)
const win = window.__TAURI__ && window.__TAURI__.window ? window.__TAURI__.window.getCurrentWindow() : null;
$("win-min").addEventListener("click", () => win && win.minimize());
$("win-max").addEventListener("click", () => win && win.toggleMaximize());
$("win-close").addEventListener("click", () => win && win.close());

// ------------------------------------------------------------------ links (a fixed list in the core)
async function openLink(name) {
  try { await invoke("open_link", { name }); }
  catch (e) { sayErr("play-status", e); }
}
document.querySelectorAll("[data-link]").forEach((b) => b.addEventListener("click", () => openLink(b.dataset.link)));
$("tb-account").addEventListener("click", () => openLink("account"));

// ------------------------------------------------------------------ navigation
// The side menu matches the website's (stream/hub.py nav_items): Home (play), Realms, Mods (the page Mods & Addons, the
// website's Mods and Addons), Host, Forum, News, Join the Discord; then the folds (below): Play (Single player, Modes) and
// Launcher (Simple mode, Settings, Agents, Help; owner 12 Nov: "make them folds"). Mods & Addons is one page with tabs, Installed and Discover,
// each split into Mods and Addons (shelf.js; Discover also has Themes). The page ids from before (mymods, discover, and community,
// friends and hub, which became the Forum on 2026-10-30) still work everywhere: show() maps them to the page + tab, so a remembered page, a
// data-page-go button or a leyline:// link lands where it used to; a page with tabs alone opens its last tab. Single player
// and Modes have their items in the Play fold (4 Nov: the website's menu gets Single player, Modes and Guides).
const TABS = { mods: ["discover", "installed", "gamemodes", "dens"] };   // Game modes (owner 3 Nov): gamemods.js showSection; Dens: denmarket.js
// Mods & Addons opens on Discover every time it is clicked (owner 3 Nov: "make the discovery the main tab that opens when
// its clicked"); a button or link that names a tab (Installed, Game modes) still lands there.
const OPENS_ON = { mods: "discover" };
const ALIASES = { mymods: ["mods", "installed"], installed: ["mods", "installed"], discover: ["mods", "discover"],
  community: ["forum", ""], friends: ["forum", ""], hub: ["forum", ""], chat: ["forum", ""] };
const loaders = { "mods:discover": () => loadCatalog(), "mods:installed": () => loadInstalled(),
  "mods:gamemodes": () => window.LeylineGameMods && window.LeylineGameMods.showSection(),
  "mods:dens": () => window.LeylineDenMarket && window.LeylineDenMarket.load(), realms: () => loadRealms(), host: () => loadHost(),
  forum: () => window.LeylineForum && window.LeylineForum.load() };
let currentPage = "play";
let currentTab = "";
const lastTab = {};
try { Object.assign(lastTab, JSON.parse(localStorage.getItem("leyline.tabs") || "{}")); } catch (_) { /* storage may be off */ }
function resolvePage(page, tab) {
  if (ALIASES[page]) [page, tab] = ALIASES[page];
  if (!document.getElementById("page-" + page)) return ["play", ""];
  if (!TABS[page]) return [page, ""];
  if (!TABS[page].includes(tab)) tab = OPENS_ON[page] || (TABS[page].includes(lastTab[page]) ? lastTab[page] : TABS[page][0]);
  return [page, tab];
}
// The side menu's folds (index.html .nav-fold; style.css): a header button (aria-expanded, aria-controls) opens and closes
// its items; what the player chose is remembered (leyline.navFolds), and a fold not yet chosen starts as its markup says
// (Play open: Single player is the flagship; Launcher closed). show() opens the fold that holds the page it shows (a
// menu click, a data-page-go button, a link, LeylineNav.show) without remembering that, and while a fold is closed its
// header carries the page-is-here dot and the sum of the badges inside it (Agents' count, say).
const FOLDS_KEY = "leyline.navFolds";
const foldChoice = {};
try { Object.assign(foldChoice, JSON.parse(localStorage.getItem(FOLDS_KEY) || "{}")); } catch (_) { /* storage may be off */ }
const folds = [...document.querySelectorAll(".side .nav-fold[data-fold]")];
function foldCount(f) {
  const out = f.querySelector(".nf-count");
  if (!out) return;
  let n = 0;
  f.querySelectorAll(".nav-fold-inner .badge").forEach((b) => { if (!b.hidden) n += parseInt(b.textContent, 10) || 0; });
  const show = n > 0 && !f.classList.contains("open");
  out.hidden = !show;
  out.textContent = show ? String(n) : "";
  const head = f.querySelector(".nav-fold-head");
  const name = f.querySelector(".nf-name");
  if (head && name) head.setAttribute("aria-label", name.textContent + (show ? ", " + n + " new inside" : ""));
}
function setFold(f, open, remember) {
  if (typeof f === "string") f = folds.find((x) => x.dataset.fold === f);
  if (!f) return;
  f.classList.toggle("open", !!open);
  const head = f.querySelector(".nav-fold-head");
  if (head) head.setAttribute("aria-expanded", String(!!open));
  if (remember) {
    foldChoice[f.dataset.fold] = !!open;
    try { localStorage.setItem(FOLDS_KEY, JSON.stringify(foldChoice)); } catch (_) { /* storage may be off */ }
  }
  foldCount(f);
}
for (const f of folds) {
  const head = f.querySelector(".nav-fold-head");
  const name = f.dataset.fold;
  setFold(f, name in foldChoice ? foldChoice[name] : head && head.getAttribute("aria-expanded") === "true", false);
  if (head) head.addEventListener("click", () => {
    const open = !f.classList.contains("open");
    setFold(f, open, true);
    // opened at the foot of a short window: bring its items into view once it has slid open
    if (open) setTimeout(() => { if (f.scrollIntoView) f.scrollIntoView({ block: "nearest", behavior: "smooth" }); }, 240);
  });
  // a badge inside changing (agents.js, the mods count) updates the closed header's count
  const inner = f.querySelector(".nav-fold-inner");
  if (inner && window.MutationObserver) {
    new MutationObserver(() => foldCount(f)).observe(inner, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ["hidden"] });
  }
}
function revealFold(page) {
  for (const f of folds) {
    const has = !!f.querySelector(`.nav[data-page="${page}"]`);
    f.classList.toggle("has-on", has);
    if (has && !f.classList.contains("open")) setFold(f, true, false);
  }
}
function show(want, noLoad, wantTab) {
  const [page, tab] = resolvePage(want, wantTab);
  // another page starts at its top (smoke O9: Your Den from Home's banner opened scrolled, its title cut off)
  // Settings opens at its top every time it is opened (known 3 Nov: it had opened on Advanced: peer-to-peer)
  if (page !== currentPage || page === "settings") { const m = document.querySelector("main"); if (m && m.scrollTo) m.scrollTo({ top: 0, behavior: "instant" }); }
  currentPage = page;
  currentTab = tab;
  document.querySelectorAll(".nav[data-page]").forEach((b) => b.classList.toggle("on", b.dataset.page === page));
  revealFold(page);
  document.querySelectorAll(".page").forEach((p) => p.classList.toggle("on", p.id === "page-" + page));
  if (tab) {
    lastTab[page] = tab;
    document.querySelectorAll(`.page-tab[data-group="${page}"]`).forEach((b) => {
      const on = b.dataset.tab === tab;
      b.setAttribute("aria-selected", String(on));
      b.tabIndex = on ? 0 : -1;
    });
    for (const t of TABS[page]) { const pane = $("pane-" + t); if (pane) pane.classList.toggle("on", t === tab); }
  }
  const key = tab ? page + ":" + tab : page;
  if (loaders[key] && !noLoad) loaders[key]();
  try { localStorage.setItem("leyline.page", page); localStorage.setItem("leyline.tabs", JSON.stringify(lastTab)); } catch (_) { /* storage may be off */ }
  document.dispatchEvent(new CustomEvent("leyline:page", { detail: { page, tab } }));
}
document.querySelectorAll(".nav[data-page]").forEach((b) => b.addEventListener("click", () => show(b.dataset.page)));
document.querySelectorAll("[data-page-go]").forEach((b) => b.addEventListener("click", () => show(b.dataset.pageGo)));
document.querySelectorAll(".page-tab[data-group]").forEach((b) => {
  b.addEventListener("click", () => show(b.dataset.group, false, b.dataset.tab));
  // the arrow keys move between a page's tabs (the usual tab list keys)
  b.addEventListener("keydown", (e) => {
    const tabs = TABS[b.dataset.group];
    const i = tabs.indexOf(b.dataset.tab);
    const j = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? tabs.length - 1 : null;
    if (j === null) return;
    e.preventDefault();
    const t = tabs[(j + tabs.length) % tabs.length];
    show(b.dataset.group, false, t);
    const next = document.querySelector(`.page-tab[data-group="${b.dataset.group}"][data-tab="${t}"]`);
    if (next) next.focus();
  });
});
window.LeylineNav = { show, current: () => ({ page: currentPage, tab: currentTab }), TABS, ALIASES,
  // fold(name): is it open; fold(name, open): open or close it, remembered like a click on its header
  fold: (name, open) => { if (open !== undefined) setFold(name, open, true); const f = folds.find((x) => x.dataset.fold === name); return !!(f && f.classList.contains("open")); } };

// Settings › the parts of the page (review 4 Nov: one long page): a row of jumps built from the page's own headings when
// it opens (themes.js and host.js add theirs after load), each to its heading. The page itself always opens at its top.
function renderSetJump() {
  const nav = $("set-jump");
  const panel = nav && nav.closest(".panel");
  if (!panel) return;
  // the parts only: a heading straight in the page, or the first one of a part's own box (not Themes' Built in / Installed)
  const heads = [...panel.querySelectorAll("h3")].filter((h) => h.offsetParent !== null && h.textContent.trim()
    && (h.parentElement === panel || h.parentElement.querySelector("h3") === h) && !h.closest(".themes-group"));
  nav.replaceChildren(...heads.map((h, i) => {
    const words = h.textContent.trim().replace(/^Advanced:\s*/i, "Advanced: ");
    if (!h.id) h.id = "set-h-" + i;
    h.tabIndex = -1;
    const b = el("button", "set-jump-b", words.replace(/^Advanced: peer-to-peer$/i, "Advanced"));
    b.type = "button";
    b.addEventListener("click", () => { h.scrollIntoView({ block: "start", behavior: "smooth" }); h.focus({ preventScroll: true }); });
    return b;
  }));
  nav.hidden = heads.length < 3;
}
document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "settings") setTimeout(renderSetJump, 0); });

// ------------------------------------------------------------------ the backdrop behind the pages
// The website's picture (stream/hub.py BACKDROP), fetched from the site when Leyline starts. It sits behind the content
// area only (not the side bar or the menus), under the theme's wash (--t-backdrop-wash). Until it loads, and offline,
// the picture that ships with the app (art/leyline-bg.webp; audit 12 Nov: offline showed only the plain colour). With
// Settings › Background picture off: the plain colour. The window's CSP allows images from exactly this origin
// (tauri.conf.json img-src).
const BACKDROP_URL = ADDR.site("/wrath/art/leyline-shot-backdrop.webp");
const BACKDROP_LOCAL = "art/leyline-bg.webp";
let backdropTried = false;
function backdropOn() {
  try { return localStorage.getItem("leyline.backdrop") !== "off"; } catch (_) { return true; }
}
function applyBackdrop() {
  const box = $("backdrop");
  if (!box) return;
  const on = backdropOn();
  if ($("set-backdrop")) $("set-backdrop").checked = on;
  if (!on) { box.classList.remove("has-pic"); return; }
  box.classList.add("has-pic");   // the site's picture, or the app's own until it comes
  if (backdropTried) return;
  backdropTried = true;
  if (box.dataset.loaded !== "1") box.style.setProperty("--t-backdrop", `url("${BACKDROP_LOCAL}")`);
  const img = new Image();
  img.decoding = "async";
  img.addEventListener("load", () => {
    if (img.dataset.retried) box.style.setProperty("--t-backdrop", `url("${img.src}")`);   // came from the old name
    else box.style.removeProperty("--t-backdrop");   // the theme's own (the site's picture)
    box.dataset.loaded = "1";
  });
  img.addEventListener("error", () => {
    const old = ADDR.oldName(img.src);   // the new name not reachable yet: the old one, once (docs/DOMAIN-MOVE.md)
    if (old !== img.src && !img.dataset.retried) { img.dataset.retried = "1"; img.src = old; return; }
    box.dataset.loaded = "0";   // offline: the app's own picture stays
  });
  img.src = BACKDROP_URL;
}
if ($("set-backdrop")) {
  $("set-backdrop").addEventListener("change", () => {
    try { localStorage.setItem("leyline.backdrop", $("set-backdrop").checked ? "on" : "off"); } catch (_) { /* storage may be off */ }
    if ($("set-backdrop").checked && $("backdrop").dataset.loaded === "0") backdropTried = false;   // offline before: try again
    applyBackdrop();
  });
}
applyBackdrop();

// ------------------------------------------------------------------ search
// The title bar's search: a page or a feature first ("single player", "den", "friends", "boss rush": that page), then a
// realm, then the mods list (audit 12 Nov: a feature's name fell into an empty mods search).
const FEATURES = [
  { page: "single", words: ["single player", "single", "solo", "singleplayer", "offline", "companions", "companion", "dungeon buddies", "bots", "local world", "my world"] },
  { page: "single", section: "spf", words: ["friends", "friend", "invite", "invite a friend", "friends list", "friend code"] },
  { page: "den", words: ["your den", "den", "my den", "build", "building", "den code"] },
  { page: "bossrush", words: ["boss rush", "bossrush", "boss", "bosses", "boss fight", "boss fights", "raid boss"] },
  { page: "mods", tab: "discover", words: ["mods", "mod", "addons", "addon", "discover", "workshop"] },
  { page: "modes", words: ["modes", "mode", "game modes", "duels", "duel", "leaderboards", "leaderboard", "rankings", "arena"] },
  { page: "characters", words: ["characters", "character", "my characters", "alts"] },
  { page: "host", words: ["host", "hosting", "host a server", "my server", "my realm", "server"] },
  { page: "realms", words: ["realms", "realm list", "community realms", "official realm"] },
  { page: "settings", words: ["settings", "options", "preferences", "privacy", "updates"] },
  { page: "agents", words: ["agents", "agent", "claude", "ai", "assistant"] },
  { page: "help", words: ["help", "guide", "tour", "how to", "how do i"] },
  { page: "news", words: ["news", "patch notes", "what's new"] },
  { page: "forum", words: ["forum", "posts"] },
  { page: "account", words: ["account", "manage account", "hero key", "herokey", "profile", "log in", "login"] },
  { page: "findfiles", words: ["game files", "find files", "find my game", "install the game"] },
  { page: "clients", words: ["game folders", "game folder", "folders", "clients"] },
  { page: "mods", tab: "installed", words: ["installed", "my mods", "my addons"] },
];
// the feature the words name: the whole words, or the start of one (three letters at least)
function featureFor(q) {
  const w = q.replace(/[^a-z0-9' ]+/g, " ").replace(/\s+/g, " ").trim();
  if (!w) return null;
  return FEATURES.find((f) => f.words.includes(w))
    || (w.length >= 3 && FEATURES.find((f) => f.words.some((x) => x.startsWith(w))))
    || null;
}
$("search-form").addEventListener("submit", (ev) => {
  ev.preventDefault();
  const q = $("search").value.trim().toLowerCase();
  if (!q) return;
  const f = featureFor(q);
  if (f) {
    show(f.page, false, f.tab);
    if (f.section) setTimeout(() => { const s = $(f.section); if (s && !s.hidden) s.scrollIntoView({ block: "start" }); }, 60);
    return;
  }
  if (realmNames.some((n) => n.toLowerCase().includes(q)) || "wrathunleashed".includes(q)) {
    show("realms");
  } else {
    $("cat-q").value = q;
    catFilter.cat = "";
    catFilter.group = "";
    // Mods or Addons: the tab where the words find something (Mods when both or neither do)
    const hit = (p) => `${p.name} ${p.description} ${p.author}`.toLowerCase().includes(q);
    const sh = window.LeylineShelf ? window.LeylineShelf.of : () => "mods";
    const tab = !catalog.projects.some((p) => sh(p) === "mods" && hit(p)) && catalog.projects.some((p) => sh(p) === "addons" && hit(p)) ? "addons" : "mods";
    if (window.LeylineThemes) window.LeylineThemes.showTab(tab);
    if (window.LeylineSimple) window.LeylineSimple.setView("all", true);   // a search is the full list (simple.js)
    show("mods", false, "discover");
  }
});

// ------------------------------------------------------------------ news
// Home's hero gallery and its News row read the news too (home.js, App.news()); the News page lists every entry.
let news = NEWS;
// The News page's line: offline, the date of the copy shown; never loaded, that these are the notes kept in Leyline.
function newsLine(r) {
  if (!$("news-status")) return;
  if (r && r.from_cache) {
    const when = r.saved_at ? new Date(r.saved_at * 1000).toLocaleDateString() : "an earlier visit";
    say("news-status", `Offline: the news as of ${when}.`, "warn");
  } else if (!r) say("news-status", "News from the website appears when you're online. Below: notes kept in Leyline.", "warn");
  else say("news-status", "");
}
async function loadNews() {
  let r = null;
  try { r = await invoke("news_list"); } catch (e) { console.error("[leyline] news", e); newsLine(null); return; }   // keep the built-in posts
  const posts = r && Array.isArray(r.posts) ? r.posts.filter((p) => p && typeof p.title === "string" && p.title) : [];
  if (!posts.length) { newsLine(null); return; }
  newsLine(r);
  news = posts.map((p) => ({ tag: [p.tag, p.date].filter(Boolean).join(" · "), title: p.title, items: Array.isArray(p.items) ? p.items : [] }));
  renderNews();
  document.dispatchEvent(new CustomEvent("leyline:news"));   // Home's What's new slide and News row (home.js)
}
function renderNews() {
  $("news-full").replaceChildren(...news.map((x) => {
    const d = el("div", "news-entry");
    const ul = el("ul");
    ul.replaceChildren(...x.items.map((t) => el("li", "", t)));
    d.append(el("b", "", x.title), ul);
    return d;
  }));
}

// ------------------------------------------------------------------ state
async function refresh() {
  state = tidyState(await invoke("get_state"));
  $("ver").textContent = "Leyline v" + state.version;
  $("data-dir").textContent = state.data_dir;
  renderPlay();
  renderClients();
  renderSettings();
  renderClientPickers();
  document.dispatchEvent(new CustomEvent("leyline:state"));   // single.js: the game folders may have changed
  // this PC's worlds (Single player, the original world): their rows and the bar's line follow what they are now
  readWorlds().then((changed) => { if (changed) renderPlayBar(); }).catch(() => {});
}

// ------------------------------------------------------------------ realm: the chosen realm, the rows, live status
function isOfficial() { return $("play-realm").value === "official"; }
// the official realm's address on either of its names (leylinelauncher.com, or play.openvanilla.app saved before the move)
function isOfficialAddr(a) { return a === state.official_realm || ADDR.isOfficialRealm(a); }
function chosenRealm() { return isOfficial() ? state.official_realm : $("play-custom").value.trim(); }
// The official realm signs in with the Leyline account (app.rs play_official): signed in first, or the sign-in screen
// with `then` to go on afterwards (gate.js). true: signed in (or no way to tell), go on now.
function signedInFirst(then) {
  const G = window.LeylineGate;
  if (!G || G.ready()) return true;
  G.need(`${officialName()} signs you in with your Leyline account. Log in, and PLAY goes on by itself.`, then);
  return false;
}
// PLAY's line in the bar, short enough to fit beside the realm (the core's whole answer is its tooltip): what started,
// and how you get past the login screen. A community realm has its own accounts (owner 2 Nov: "community realms say
// plainly what login they need"); the official realm signs in by itself.
function playLine(msg, c) {
  const m = String(msg || "");
  if (!/^Started /.test(m)) return m;
  let out = `Started ${c.name}.`;
  if (/signs you in by itself/.test(m)) out += " It signs you in by itself.";
  else if (/account name is filled in/.test(m)) out += " Type your game password at the login screen.";
  else if (!isOfficial() && !localChosen() && chosenRealm() !== DEN_LITE) out += " Log in with your account there.";
  return out;
}
// The official realm (launch, owner 2026-10-30; renamed "Wrath Unleashed" for now, owner 2026-10-30 23:32): where
// everyone meets in Dalaran. One name everywhere, whatever the listing calls it (the Sample Hangout, the realm's own
// name). The one place the launcher's UI names it: home.js and addons.js read it from here (window.LeylineApp.HANGOUT).
const HANGOUT = { name: "Wrath Unleashed", kind: "Official realm" };
// The official lineup after the Hangout: a record the network marks official or pinned, or one known by name until it
// does (Vanilla Remastered). Pinned under the Hangout in the play bar's realm picker.
const LINEUP_NAMES = ["vanilla remastered"];
function inLineup(r) {
  return !!r && !r.official && (!!r.pinned || !!r.lineup || LINEUP_NAMES.includes(String(r.name || "").trim().toLowerCase()));
}
function officialName() { return HANGOUT.name; }
// Realms the launcher never shows (owner 1 Nov: "hide Builders' Realm because we don't need it"; it is being turned off).
// realms.rs leaves it out too (HIDDEN_REALM_IDS); this is the same rule, should an older core still list it.
const HIDDEN_REALM_IDS = ["jgt5rpofn7yaisep"];
function hiddenRealm(r) {
  if (!r) return false;
  const id = r.realm_id || String(r.slug || "").replace(/^c-/, "");
  return HIDDEN_REALM_IDS.includes(id) || String(r.name || "").toLowerCase().replace(/[^a-z0-9]/g, "") === "buildersrealm";
}
function realmLabel() {
  if (isOfficial()) return officialName();
  const a = chosenRealm();
  if (worldChosen()) return WORLD_TITLE[worldChosen()];   // Single player or the original world, Built-in engine
  if (localAt(a)) return localAt(a).name;   // My Den or Single player
  if (a === DEN_LITE) return DEN.name;      // Your Den on Den Lite (host/lite.rs ADDRESS)
  const r = realmsCache.find((x) => x.address === a);
  return r ? r.name : (a || "Another realm");
}

// The picked client has one home: the core's selected_client (owner 2 Nov: "hitting play doesnt go into the selected
// client"). Whatever picks one (the play bar's list, a page's .client-pick, a realm of another version) ends here, so
// state.settings, every .client-pick and the core all say the same client, and PLAY anywhere starts that one.
function saveSelection() {
  const c = currentClient();
  if (c) {
    state.settings.selected_client = c.id;
    document.querySelectorAll(".client-pick").forEach((sel) => { if (sel.value !== c.id) sel.value = c.id; });
    invoke("select", { client: c.id, realm: chosenRealm() }).catch(() => {});
  }
  if (mode !== "browser" && !isOfficial()) rememberJoined(chosenRealm());
}
// the community realms you played on (the realm picker's Your realms), the latest first, five at most, on this PC only
const JOINED_KEY = "leyline.joined";
function joinedRealms() {
  try { const v = JSON.parse(localStorage.getItem(JOINED_KEY) || "[]"); return Array.isArray(v) ? v.filter((x) => typeof x === "string" && x).slice(0, 5) : []; }
  catch (_) { return []; }
}
function rememberJoined(addr) {
  if (!addr || isOfficialAddr(addr) || localAt(addr)) return;
  try { localStorage.setItem(JOINED_KEY, JSON.stringify([addr, ...joinedRealms().filter((x) => x !== addr)].slice(0, 5))); } catch (_) { /* fine */ }
}

let lastStatusClass = "";
let realmState = "";   // the chosen realm's last check: online, partial, offline, unknown (no answer), "" (not checked yet)
// The side bar's "Your realms", dressed like the website's realm list: the realm's icon (the crystal for the official
// realm, else its initial on a colour from its name), its name, the gold client version and the live dot.
function renderRealmRows() {
  const rows = $("realm-rows");
  rows.replaceChildren();
  const add = (name, official, ver) => {
    const on = official ? isOfficial() : !isOfficial();
    const b = el("button", "realm-row" + (on ? " on" : ""));
    b.type = "button";
    const dot = el("span", "dot");
    if (on && lastStatusClass) b.classList.add(lastStatusClass);
    const txt = el("span", "rr-txt");
    txt.append(el("span", "name", name));
    if (ver) { const v = el("span", "rl-ver sm", ver); v.title = "Game client version"; txt.append(v); }
    if (official && queueN()) { const q = el("span", "rr-busy", `Busy · queue ${queueN()}`); q.title = "People waiting to get in"; txt.append(q); }
    b.append(!official && localAt(custom) && name === localAt(custom).name ? markIcon(localAt(custom).mark) : realmIcon(name, official, "sm"), txt, dot);
    b.title = ver ? `${name} · ${ver}` : name;
    b.addEventListener("click", () => {
      $("play-realm").value = official ? "official" : "custom";
      realmChanged();
      saveSelection();
      show("play");
    });
    rows.append(b);
  };
  add(officialName(), true, "3.3.5a");
  const saved = state.settings.selected_realm;
  const custom = !isOfficial() ? chosenRealm() : (saved && !isOfficialAddr(saved) ? saved : "");
  if (custom) {
    const beacon = realmsCache.find((x) => x.address === custom);
    if (localAt(custom)) add(localAt(custom).name, false, localAt(custom).build);   // this PC's Den or Single player (the play bar's rows)
    else add(beacon ? beacon.name : custom, false, realmVersion(beacon));
  }
}

// Home's SERVER card: the chosen realm's icon, name and client version (the players bar comes with its status). Home has
// no SERVER or GAME CLIENT card since the play bar shows both (owner 2026-10-30): these do nothing without them.
function renderSrvHead() {
  if (!$("srv-name")) return;
  const name = realmLabel();
  $("srv-name").textContent = name;
  const r = isOfficial() ? null : realmAt(chosenRealm());
  fillRealmIcon($("srv-ic"), name, isOfficial());
  const ver = isOfficial() ? "3.3.5a" : realmVersion(r);
  $("srv-ver").hidden = !ver;
  $("srv-ver").textContent = ver || "";
}
function renderSrvBar(players) {
  const bar = $("srv-bar");
  if (!bar) return;
  if (players == null) { bar.hidden = true; return; }
  const o = isOfficial() ? (realmsCache || []).find((x) => x.official) : realmAt(chosenRealm());
  bar.hidden = false;
  bar.firstElementChild.style.width = playersFill(players, o && o.max_players) + "%";
}

let statusSeq = 0;
let officialGamedata;   // false = the official realm's server streams no game data (browser play needs the player's folder)
// The official realm's queues (status.json, docs/QUEUE.md): its login queue ("queue") and the browser seats' line
// ("browser_queue"). The realm row and PLAY say "Busy · queue N" for the mode you play in, with the skip-the-line card.
let officialQueue = { realm: 0, browser: 0, wait: 0 };
let queueCardShut = false;   // "Not now" on the card: shut until the line is gone
function queueN() {
  if (!isOfficial() || localChosen()) return 0;
  return mode === "browser" ? officialQueue.browser : officialQueue.realm;
}
function renderQueueCard() {
  const box = $("pb-queue");
  if (!box) return;
  const n = queueN();
  if (!n) queueCardShut = false;
  box.hidden = !n || queueCardShut;
  if (box.hidden) return;
  $("pbq-line").textContent = mode === "browser"
    ? `${n} waiting for a browser seat${officialQueue.wait ? `, about ${officialQueue.wait} min` : ""}.`
    : `${n} waiting to get into ${officialName()}.`;
  $("pbq-pc").hidden = mode !== "browser";
}
async function refreshStatus() {
  const realm = chosenRealm();
  const seq = ++statusSeq;
  const set = (cls, pill, line, detail, st) => {
    if (seq !== statusSeq) return;
    lastStatusClass = cls;
    const was = realmState;
    realmState = st || "";
    if (was !== realmState) renderPlayBar();   // PLAY's line says the realm's real state (audit 12 Nov)
    $("tb-status").className = "pill " + cls;
    $("tb-status-text").textContent = pill;
    if ($("srv-state")) {
      $("srv-state").className = "srv-state " + cls;
      $("srv-state-text").textContent = line;
      $("srv-detail").textContent = detail;
    }
    document.querySelectorAll(".realm-row.on").forEach((r) => {
      r.classList.remove("is-online", "is-partial", "is-offline");
      if (cls) r.classList.add(cls);
    });
  };
  renderSrvHead();
  renderSrvBar(null);
  if (!realm) return set("", "No realm", "NO REALM CHOSEN", "Type a realm address under Realms.");
  set("", "Checking…", "CHECKING…", realm);
  try {
    const beacon = realmsCache.find((x) => !x.official && x.address === realm && x.status_url);
    const s = await invoke("realm_status", beacon ? { realm, statusUrl: beacon.status_url } : { realm });
    // The official realm's game data killswitch (status.json "gamedata_streaming"): BROWSER mode says what it needs.
    if (seq === statusSeq && !beacon && isOfficial()) {
      officialGamedata = s.gamedata_streaming;
      officialQueue = { realm: s.queue || 0, browser: s.browser_waiting || 0, wait: s.browser_wait_min || 0 };
      renderPlayBar();
      renderRealmRows();
    }
    const ms = s.ping_ms != null ? `${s.ping_ms} ms` : "";
    const counts = [s.players != null ? `Players: ${s.players}` : "", s.bots != null ? `Bots: ${s.bots}` : ""].filter(Boolean);
    const host = realm.includes(":") ? realm : `${realm}:3724`;
    if (seq === statusSeq) renderSrvBar(s.state === "online" && s.players != null ? s.players : null);
    if (s.state === "online") set("is-online", ms ? `Online  ${ms}` : "Online", ms ? `ONLINE  (${ms})` : "ONLINE", (counts.length ? counts : [host]).join(" · "), "online");
    else if (s.state === "partial") set("is-partial", "Partly up", "PARTLY UP", "Login answers · world server down", "partial");
    else if (s.state === "offline") set("is-offline", "Offline", "OFFLINE", "World server down", "offline");
    else set("is-partial", "No answer", "NO ANSWER", "The realm didn't answer: check your connection", "unknown");
  } catch (e) {
    set("is-partial", "No answer", "STATUS UNAVAILABLE", oops(e, "Couldn't check the realm right now."), "unknown");
  }
}
// every minute, but not while the window is hidden or minimised (a game is usually running then); back in view: at once
setInterval(() => { if (state && !document.hidden) refreshStatus(); }, 60000);
let hiddenSince = 0;
document.addEventListener("visibilitychange", () => {
  if (document.hidden) { hiddenSince = Date.now(); return; }
  if (state && hiddenSince && Date.now() - hiddenSince > 30000) refreshStatus();
  hiddenSince = 0;
});

// ------------------------------------------------------------------ play bar
let mode = "online";
try { mode = localStorage.getItem("leyline.mode") === "browser" ? "browser" : "online"; } catch (_) { /* fine */ }

function setMode(m) {
  mode = m;
  try { localStorage.setItem("leyline.mode", m); } catch (_) { /* fine */ }
  renderPlayBar();
  document.dispatchEvent(new CustomEvent("leyline:mode", { detail: { mode: m } }));
}

function currentClient() {
  return state.settings.clients.find((x) => x.id === $("play-client").value);
}

function renderPlay() {
  const s = state.settings;
  const sel = $("play-client");
  sel.replaceChildren();
  for (const c of s.clients) {
    const o = el("option", "", c.name);
    o.value = c.id;
    sel.append(o);
  }
  if (s.clients.length === 0) sel.append(el("option", "", "No game folder yet"));
  if (s.selected_client) sel.value = s.selected_client;
  sel.disabled = s.clients.length === 0;
  renderClientMenu();
  if ($("play-empty")) $("play-empty").hidden = s.clients.length !== 0;
  if ($("client-change")) $("client-change").textContent = s.clients.length ? "Change game folder" : "Add your game folder";
  const official = !s.selected_realm || isOfficialAddr(s.selected_realm);
  $("play-realm").value = official ? "official" : "custom";
  $("play-custom").value = official ? "" : s.selected_realm;
  realmChanged();
}

function renderClientCard() {
  const box = $("client-now");
  if (!box) return;
  box.replaceChildren();
  const c = currentClient();
  if (!c) return;
  box.append(el("b", "", c.name), el("span", "", buildLabel(c.build)));
}

// The build the chosen realm needs, when the list says ("3.3.5a" for the official realm).
function chosenRealmBuild() {
  if (isOfficial()) return "3.3.5a";
  if (worldChosen()) return WORLD_BUILD;
  const r = realmAt(chosenRealm());
  return r && r.build ? r.build : null;
}

// the realm, client and mode the play bar's status line was last said for: another one, and the old line goes (owner
// 2 Nov: "Started … on 127.0.0.1." from a Den start stayed after switching to Wrath Unleashed)
let statusFor = null;
const playKey = () => `${mode}|${isOfficial() ? "official" : chosenRealm()}|${$("play-client").value}`;
function realmChanged() {
  if (playKey() !== statusFor) { if (statusFor !== null) { say("play-status", ""); $("play-status").title = ""; } statusFor = playKey(); }
  $("play-custom-wrap").hidden = isOfficial();
  const c = currentClient();
  const need = chosenRealmBuild();
  let hint = "";
  if (c && need && c.build !== need) hint = `This realm runs ${need}. Pick a ${need} client, or another realm for this ${c.build} client.`;
  else if (c) hint = `PLAY sets this client's realm address (${BUILDS[c.build] ? BUILDS[c.build].file : "its realm file"}; your original is kept as a backup) and starts it.`;
  $("play-hint").textContent = hint;
  renderClientCard();
  renderRealmRows();
  renderPlayBar();
  refreshStatus();
}

// ------------------------------------------------------------------ the play bar's client picker: a drop-up list
// Top: Add a game folder… and Find your game files. Then the families Leyline plays that have no folder yet, greyed
// ("Not found": a click opens the Find your game files guide). Nearest the button: the folders you added, the picked
// one marked. Arrow keys, Home / End, Enter or Space, Esc; a click outside closes it. #play-client stays the model.
// the families, oldest first (ERAS): the offered ones only
const FAMILIES = () => ERAS.filter(([b]) => offeredBuild(b));
let menuOpen = false;
function clientVersion(c) { return c && BUILDS[c.build] ? c.build : (c && c.exe_version) || ""; }
// the version chip beside a client's name, unless the name already says it ("Wrath 3.3.5a": owner 2 Nov 23:15, the
// drop-up list showed every number twice)
function clientBadge(c) { const v = clientVersion(c); return v && !String((c && c.name) || "").includes(v) ? v : ""; }
function renderClientMenu() {
  if (!state) return;
  const c = currentClient();
  $("pb-client-name").textContent = c ? c.name : "No game folder";
  $("pb-client").classList.toggle("empty", !c);
  const v = clientBadge(c);
  $("pb-client-ver").hidden = !v;
  $("pb-client-ver").textContent = v;
  $("pb-client").title = c ? `${c.name} · ${c.path}` : "Add your game folder";
  if (menuOpen) return;   // never rebuilt under the keyboard: it is built fresh each time it opens
  const menu = $("pb-menu");
  const rows = [];
  const row = (cls, label, opts) => {
    const li = el("li", "pb-opt " + cls);
    li.setAttribute("role", "option");
    li.tabIndex = -1;
    li.id = "pb-opt-" + rows.length;
    const name = el("span", "pb-opt-name", label);
    li.append(name);
    if (opts.ver) li.append(el("span", "rl-ver sm", opts.ver));
    if (opts.sub) li.append(el("span", "pb-opt-sub", opts.sub));
    li.setAttribute("aria-selected", String(!!opts.selected));
    if (opts.disabled) li.setAttribute("aria-disabled", "true");
    if (opts.title) li.title = opts.title;
    li.addEventListener("click", () => { closeClientMenu(true); opts.run(); });
    rows.push(li);
    return li;
  };
  const sep = () => { const d = el("li", "pb-sep"); d.setAttribute("role", "presentation"); return d; };
  const top = [
    row("pb-act", "Add a game folder…", { run: () => show("clients") }),
    row("pb-act", "Add from a zip or a link…", { run: () => cimpOpen() }),
    row("pb-act", "Find your game files", { run: () => openFindFiles() }),
  ];
  const have = state.settings.clients;
  const missing = FAMILIES().filter(([b]) => !have.some((x) => x.build === b)).map(([b, name]) =>
    row("pb-missing", name, { ver: b, sub: "Not found", disabled: true, title: `No ${b} folder added yet: how to find it`, run: () => openFindFiles(name, b) }));
  const tail = (t) => ([...t].length > 30 ? "…" + [...t].slice(-29).join("") : t);   // the end of a long path, where the folder name is
  const mine = have.map((x) => row("pb-mine", x.name, { ver: clientBadge(x), sub: tail(x.path), selected: c && x.id === c.id, title: x.path,
    run: () => chooseClient(x) }));   // remembered for the realm picked, as the realm picker's client row
  const parts = [top, missing, mine].filter((g) => g.length);
  menu.replaceChildren(...parts.flatMap((g, i) => (i ? [sep(), ...g] : g)));
}
function menuRows() { return [...$("pb-menu").querySelectorAll(".pb-opt")]; }
function openClientMenu() {
  if (menuOpen) return;
  if (typeof closeRealmMenu === "function") closeRealmMenu(false);
  renderClientMenu();
  menuOpen = true;
  $("pb-menu").hidden = false;
  $("pb-client").setAttribute("aria-expanded", "true");
  const rows = menuRows();
  const at = rows.find((r) => r.getAttribute("aria-selected") === "true") || rows[rows.length - 1];
  if (at) at.focus();
}
function closeClientMenu(refocus) {
  if (!menuOpen) return;
  menuOpen = false;
  $("pb-menu").hidden = true;
  $("pb-client").setAttribute("aria-expanded", "false");
  if (refocus) $("pb-client").focus();
}
$("pb-client").addEventListener("click", () => (menuOpen ? closeClientMenu(true) : openClientMenu()));
$("pb-client").addEventListener("keydown", (e) => {
  if (e.key === "ArrowUp" || e.key === "ArrowDown") { e.preventDefault(); openClientMenu(); }
});
$("pb-menu").addEventListener("keydown", (e) => {
  const rows = menuRows();
  const i = rows.indexOf(document.activeElement);
  let j = null;
  if (e.key === "ArrowDown") j = Math.min(rows.length - 1, i + 1);
  else if (e.key === "ArrowUp") j = Math.max(0, i - 1);
  else if (e.key === "Home") j = 0;
  else if (e.key === "End") j = rows.length - 1;
  else if (e.key === "Escape") { e.preventDefault(); closeClientMenu(true); return; }
  else if (e.key === "Tab") { closeClientMenu(false); return; }
  else if ((e.key === "Enter" || e.key === " ") && i >= 0) { e.preventDefault(); rows[i].click(); return; }
  if (j === null) return;
  e.preventDefault();
  rows[j].focus();
});
document.addEventListener("pointerdown", (e) => {
  if (menuOpen && !e.target.closest(".pb-left .pb-pick")) closeClientMenu(false);
});

// The Find your game files guide (page-findfiles), with the family asked for when a greyed row opened it.
let ffWant = null;
function openFindFiles(name, build) {
  ffWant = name ? `Looking for ${name} ${build}. ` : null;
  show("findfiles");
}
document.addEventListener("leyline:page", (e) => {
  if (e.detail.page !== "findfiles") return;
  $("ff-want").hidden = !ffWant;
  $("ff-want").textContent = ffWant || "";
  ffWant = null;   // opened any other way (Clients' link, a remembered page): the general guide
});
$("ff-pick").addEventListener("click", async () => {
  show("clients");
  try { addClient(await invoke("pick_folder")); } catch (e) { sayErr("cl-status", e); }
});
$("ff-browser").addEventListener("click", () => { setMode("browser"); show("play"); });

// ------------------------------------------------------------------ add a game client by dropping it (clientimport.rs)
// Owner 31 Oct: drop a folder, a zip or a direct link to your own backup on the client list (or Find your game files),
// with a progress bar in the launcher: download %, then unpacking %, Cancel. A folder is added where it is; a zip or a
// link is unpacked into a folder you pick (by default in Leyline's own folder). Never a torrent: the core says so.
const cimp = { input: null, checked: null, folder: null, timer: null };
const CIMP_PHASE = { downloading: "Downloading", unpacking: "Unpacking", checking: "Looking for the game" };
function cimpView(view) {
  const box = $("cimp");
  box.hidden = false;
  box.dataset.state = view;
  $("cimp-pick").hidden = view !== "pick";
  $("cimp-ask").hidden = view !== "ask";
  $("cimp-run").hidden = view !== "run";
  $("cimp-go").hidden = view !== "ask";
  $("cimp-cancel").hidden = !(view === "ask" || view === "run");
  $("cimp-ok").hidden = !(view === "done" || view === "error");
  if (view !== "done" && view !== "error") say("cimp-status", "");
}
function cimpClose() {
  clearTimeout(cimp.timer);
  $("cimp").hidden = true;
  $("cimp").classList.remove("drag");
}
// From the menu's Add from a zip or a link…: the drop area, a link box, Choose a zip… or a folder.
function cimpOpen() {
  if (cimp.running) { $("cimp").hidden = false; return; }
  closeClientMenu(false);
  $("cimp-what").textContent = "Drop your game folder, a zip of it, or a direct link to your own backup.";
  $("cimp-link").value = "";
  cimpView("pick");
  $("cimp-link").focus();
}
function cimpError(e) {
  if (typeof closeClientMenu === "function") closeClientMenu(false);
  $("cimp-what").textContent = "";
  cimpView("error");
  say("cimp-status", oops(e, "That didn't work. Drop your game folder or a zip of it, or try again."), "bad");
}
// Anything dropped or pasted: the core says what it is (or why not), then a folder is added and a zip or link asks where.
async function clientImport(input) {
  input = String(input || "").trim();
  if (!input) return;
  if (cimp.running) { $("cimp").hidden = false; say("cimp-status", "Leyline is already adding a game client.", "warn"); return; }
  closeClientMenu(false);
  let c;
  try { c = await invoke("client_import_check", { input }); }
  catch (e) { cimpError(e); return; }
  cimp.input = input;
  cimp.checked = c;
  cimp.folder = c.default_folder;
  if (!c.unpacks) { $("cimp-what").textContent = `Adding ${c.name || "your folder"}…`; cimpStart(); return; }
  $("cimp-what").textContent = c.kind === "link" ? `Download from ${c.name} and unpack it.`
    : c.kind === "zip" ? `Unpack ${c.name}.` : `Download ${c.name}.`;
  cimpDest(cimp.folder);
  cimpView("ask");
  $("cimp-go").focus();
}
async function cimpStart() {
  cimpView("run");
  cimpProgress({ phase: "checking", done: 0, total: 0 });
  cimp.running = true;
  try { await invoke("client_import_start", { input: cimp.input, folder: cimp.checked && cimp.checked.unpacks ? cimp.folder : null }); }
  catch (e) { cimp.running = false; cimpError(e); return; }
  cimpPoll();
}
// the end of a long folder path, where its name is
function cimpDest(f) {
  const t = [...String(f || "")];
  $("cimp-dest").textContent = t.length > 46 ? "…" + t.slice(-45).join("") : t.join("");
  $("cimp-dest").title = f || "";
}
function cimpProgress(p) {
  const pct = p.total > 0 ? Math.min(100, Math.floor((p.done * 100) / p.total)) : null;
  $("cimp-phase").textContent = (CIMP_PHASE[p.phase] || "Working") + "…";
  $("cimp-pct").textContent = pct === null ? "" : pct + "%";
  $("cimp-fill").style.width = (pct === null ? 100 : pct) + "%";
  $("cimp-bar").classList.toggle("unknown", pct === null);
  if (pct === null) $("cimp-bar").removeAttribute("aria-valuenow"); else $("cimp-bar").setAttribute("aria-valuenow", String(pct));
}
async function cimpPoll() {
  clearTimeout(cimp.timer);
  let p;
  try { p = await invoke("client_import_state"); } catch (e) { cimp.running = false; cimpError(e); return; }
  if (p.phase === "downloading" || p.phase === "unpacking" || p.phase === "checking") {
    cimpProgress(p);
    cimp.timer = setTimeout(cimpPoll, 400);
    return;
  }
  cimp.running = false;
  if (p.phase === "done") {
    cimpView("done");
    $("cimp-what").textContent = "";
    say("cimp-status", p.message || "Added.", "ok");
    await refresh();
    if (p.client_id) { $("play-client").value = p.client_id; realmChanged(); saveSelection(); }
    const added = state && state.settings.clients.find((x) => x.id === p.client_id);
    if (added && window.LeylineAddons) window.LeylineAddons.offer(added);
    $("cimp-ok").focus();
  } else if (p.phase === "cancelled") {
    cimpView("done");
    $("cimp-what").textContent = "";
    say("cimp-status", p.message || "Stopped. Nothing was added.", "warn");
  } else cimpError(p.message);
}
$("cimp-go").addEventListener("click", cimpStart);
$("cimp-cancel").addEventListener("click", async () => {
  if (!cimp.running) { cimpClose(); return; }
  $("cimp-phase").textContent = "Stopping…";
  try { await invoke("client_import_cancel"); } catch (_) { /* it ends by itself */ }
});
$("cimp-ok").addEventListener("click", cimpClose);
$("cimp-x").addEventListener("click", () => { if (!cimp.running) cimpClose(); else $("cimp-cancel").click(); });
$("cimp-change").addEventListener("click", async () => {
  try { const f = await invoke("client_import_pick_folder"); if (f) { cimp.folder = f; cimpDest(f); } }
  catch (e) { say("cimp-status", oops(e), "bad"); }
});
const cimpLink = (id) => () => clientImport($(id).value);
$("cimp-link-go").addEventListener("click", cimpLink("cimp-link"));
$("cimp-link").addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); cimpLink("cimp-link")(); } });
$("ff-link-go").addEventListener("click", cimpLink("ff-link"));
$("ff-link").addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); cimpLink("ff-link")(); } });
const cimpPickZip = async () => { try { const z = await invoke("client_import_pick_zip"); if (z) clientImport(z); } catch (e) { cimpError(e); } };
$("cimp-zip").addEventListener("click", cimpPickZip);
$("ff-zip").addEventListener("click", cimpPickZip);
$("cimp-folder-pick").addEventListener("click", async () => { try { const f = await invoke("pick_folder"); if (f) clientImport(f); } catch (e) { cimpError(e); } });
document.addEventListener("keydown", (e) => { if (e.key === "Escape" && !$("cimp").hidden && !cimp.running) cimpClose(); });
// Where a drop adds a game client: the client button, its open list, this panel, and the whole Find your game files page.
function clientDropAt(x, y) {
  if (currentPage === "findfiles") return true;
  return [$("pb-client"), menuOpen && $("pb-menu"), !$("cimp").hidden && $("cimp"), currentPage === "single" && $("sp-drop")].some((n) => {
    if (!n) return false;
    const r = n.getBoundingClientRect();
    return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
  });
}
function clientDragOn(on) {
  $("pb-menu").classList.toggle("drag", on);
  $("cimp").classList.toggle("drag", on);
  $("ff-drop").classList.toggle("drag", on);
}
// Links and text dragged out of a browser arrive as HTML drops (files come through the window: setupDrop).
function dropText(dt) {
  if (!dt) return "";
  const uris = (dt.getData("text/uri-list") || "").trim();
  return uris || (dt.getData("text/plain") || "").trim();
}
for (const id of ["pb-client", "pb-menu", "cimp", "page-findfiles"]) {
  const n = $(id);
  n.addEventListener("dragover", (e) => {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    if (id === "pb-client" && !menuOpen && $("cimp").hidden) openClientMenu();
    clientDragOn(true);
  });
  n.addEventListener("dragleave", (e) => { if (!n.contains(e.relatedTarget)) clientDragOn(false); });
  n.addEventListener("drop", (e) => {
    e.preventDefault();
    e.stopPropagation();
    clientDragOn(false);
    const t = dropText(e.dataTransfer);
    if (t) clientImport(t);
  });
}
// A link pasted while the client list, this panel or the guide is open (not into a text box).
document.addEventListener("paste", (e) => {
  if (e.defaultPrevented) return;   // the realm box took it (barImport)
  if (e.target && e.target.closest && e.target.closest("input, textarea, [contenteditable]")) return;
  if (!(menuOpen || !$("cimp").hidden || currentPage === "findfiles")) return;
  const t = ((e.clipboardData && e.clipboardData.getData("text")) || "").trim();
  if (t) { e.preventDefault(); clientImport(t); }
});
window.LeylineClientImport = { drop: clientImport, open: cimpOpen };

// ------------------------------------------------------------------ the realm box takes drops too (owner 31 Oct)
// "magnet links and zip and cloudflare stuff can be dropped in this area and autostart with folder selection and turn
// into a loading bar with speed stats". Drop (or paste, with the box focused) a folder, a zip, an https link, or, with
// Settings › Testing on, a magnet link or torrent file: Where should the game go? (a folder picker at the last choice),
// then the box is the progress bar (clientimport.rs Progress.line), Cancel; when done the client is picked for every
// realm of its version (leyline.clientFor) and the bar picks a realm of that version: Ready — press PLAY. Never
// launches the game by itself.
const bimp = { input: null, timer: null, running: false, name: "", hideAt: null };
const BAR_FOLDER_KEY = "leyline.gameFolder";
function bimpView(view, head, line) {
  const box = $("pbd");
  clearTimeout(bimp.hideAt);
  if (view === "idle") { box.hidden = true; box.dataset.state = "idle"; return; }
  box.hidden = false;
  box.dataset.state = view;
  $("pbd-head").textContent = head || "";
  $("pbd-head").title = head || "";
  $("pbd-line").textContent = line || "";
  $("pbd-cancel").hidden = !(view === "run" || view === "paused" || view === "picking");
  $("pbd-pause").hidden = !(view === "run" || view === "paused");
  if (view !== "run" && view !== "paused" && !(view === "done" && bimp.seeding)) $("pbd-nodes").hidden = true;
  $("pbd-retry").hidden = view !== "error";
  $("pbd-x").hidden = !(view === "error" || view === "done");
  if ($("pbd-play")) $("pbd-play").hidden = true;   // a realm link's Play now (realmlink.js) shows it after this
  box.classList.toggle("unknown", view === "picking" || view === "run");
  if (view !== "run" && view !== "paused") $("pbd-fill").style.width = view === "done" ? "100%" : "0";
  if (view !== "over" && view !== "run" && view !== "paused") $("pbd-say").textContent = [head, line].filter(Boolean).join(". ");
}
const bimpIdle = () => { if (!bimp.running) bimpView("idle"); };
function bimpError(e) {
  bimp.running = false;
  bimpView("error", oops(e, "That didn't work. Try again."), "");
}
const BIMP_WORD = { downloading: "Downloading", unpacking: "Unpacking", checking: "Checking your game folder…" };
// The blue bar (owner 31 Oct 21:20): "Downloading <name> · 24%" over "1.2 GB / 5.0 GB · 14 MB/s · about 4 min left",
// pause / play, and for a torrent the Nodes chip (the PCs sharing it; pressed: this PC is a node too).
function bimpProgress(p) {
  const pct = p.total > 0 ? Math.min(100, Math.floor((p.done * 100) / p.total)) : null;
  const raw = p.line || (p.phase === "checking" ? BIMP_WORD.checking : (BIMP_WORD[p.phase] || "Working") + "…");
  const parts = raw.split(" · ");
  const word = parts.length > 1 ? parts.shift() : raw;
  const rest = parts.length && parts[0] !== raw ? parts : [];
  if (p.sharing && p.up_speed > 0) rest.push("sharing ↑ " + bimpSpeed(p.up_speed));
  const head = p.phase === "checking" ? raw : [word, bimp.name].filter(Boolean).join(" ") + (pct === null ? "" : " · " + pct + "%");
  bimpView(p.paused ? "paused" : "run", head, rest.join(" · "));
  $("pbd").classList.toggle("unknown", pct === null && !p.paused);
  $("pbd-fill").style.width = (pct === null ? 0 : pct) + "%";
  const pp = $("pbd-pause");
  pp.hidden = p.phase !== "downloading";
  pp.classList.toggle("paused", !!p.paused);
  pp.setAttribute("aria-label", p.paused ? "Go on" : "Pause");
  pp.title = p.paused ? "Go on" : "Pause";
  bimpNodes(p.nodes, p.seeds, !!p.sharing);
  // the live region: now and then, not every poll
  const now = Date.now();
  if (!bimp.said || now - bimp.said > 15000 || p.paused !== bimp.wasPaused) {
    $("pbd-say").textContent = [head, rest.join(", ")].filter(Boolean).join(". ");
    bimp.said = now;
    bimp.wasPaused = p.paused;
  }
}
function bimpSpeed(bps) {
  return bps >= 1048576 ? (bps / 1048576).toFixed(1) + " MB/s" : Math.max(1, Math.round(bps / 1024)) + " KB/s";
}
function bimpNodes(nodes, seeds, sharing, word) {
  const b = $("pbd-nodes");
  if (nodes === null || nodes === undefined) { b.hidden = true; return; }
  b.hidden = false;
  $("pbd-nodes-n").textContent = String(nodes);
  $("pbd-nodes-w").textContent = word || (sharing ? (nodes === 1 ? "Node · you share" : "Nodes · you share") : (nodes === 1 ? "Node" : "Nodes"));
  b.setAttribute("aria-pressed", sharing ? "true" : "false");
  b.setAttribute("aria-label", `${nodes} ${nodes === 1 ? "node" : "nodes"}${seeds ? `, ${seeds} with all of it` : ""}. ` +
    (sharing ? "You are a node: press to stop sharing." : "Press to be a node and share it too."));
}
$("pbd-pause").addEventListener("click", async () => {
  const on = !$("pbd-pause").classList.contains("paused");
  try { const p = await invoke("client_import_pause", { on }); if (bimp.running && p && p.phase === "downloading") bimpProgress(p); }
  catch (e) { say("play-status", oops(e), "bad"); }
});
$("pbd-nodes").addEventListener("click", async () => {
  const on = $("pbd-nodes").getAttribute("aria-pressed") !== "true";
  if (!bimp.running) {   // after the download: the only thing to do is stop sharing
    try { await invoke("client_seed_stop"); } catch (_) { /* it stops by itself when Leyline closes */ }
    bimp.seeding = false;
    $("pbd-nodes").hidden = true;
    return;
  }
  try { const p = await invoke("client_import_share", { on }); if (p) bimpNodes(p.nodes, p.seeds, !!p.sharing); }
  catch (e) { say("play-status", oops(e), "bad"); }
});
// After a torrent with Nodes on: "Sharing · 3 nodes · ↑ 120 KB/s" on the Ready box until Stop (the chip).
async function bimpSeedPoll() {
  clearTimeout(bimp.seedTimer);
  let s = null;
  try { s = await invoke("client_seed_state"); } catch (_) { /* older core: no sharing */ }
  bimp.seeding = !!(s && s.active);
  if (!bimp.seeding) { if ($("pbd").dataset.state === "done") $("pbd-nodes").hidden = true; return; }
  if ($("pbd").dataset.state === "done") {
    bimpNodes(s.nodes, 0, true, "Sharing" + (s.up_speed > 0 ? " ↑ " + bimpSpeed(s.up_speed) : ""));
    $("pbd-nodes").title = "This PC is sharing the download it finished. Press to stop sharing.";
  }
  bimp.seedTimer = setTimeout(bimpSeedPoll, 1500);
}
async function barImport(input) {
  input = String(input || "").trim();
  if (!input) return;
  if (window.LeylineRealmLink && window.LeylineRealmLink.take(input, "bar")) return;   // a realm link or address (realmlink.js)
  if (bimp.running || cimp.running) { say("play-status", "Leyline is already adding a game client.", "warn"); return; }
  closeRealmMenu(false);
  closeClientMenu(false);
  bimp.input = input;
  let c;
  try { c = await invoke("client_import_check", { input }); } catch (e) { bimpError(e); return; }
  bimp.name = c.name || "";
  let folder = null;
  if (c.unpacks) {
    bimpView("picking", "Where should the game go?", "Pick a folder in the window that opened");
    let last = "";
    try { last = localStorage.getItem(BAR_FOLDER_KEY) || ""; } catch (_) { /* the default then */ }
    try { folder = await invoke("client_import_pick_dest", { start: last || null }); } catch (e) { bimpError(e); return; }
    if (!folder) { bimpView("idle"); return; }   // the picker was closed: nothing starts
    try { localStorage.setItem(BAR_FOLDER_KEY, folder); } catch (_) { /* asked again next time */ }
  }
  bimp.running = true;
  cimp.running = true;   // the client list's panel waits too: one import at a time
  bimpProgress({ phase: c.unpacks ? "downloading" : "checking", done: 0, total: 0 });
  try { await invoke("client_import_start", { input, folder }); }
  catch (e) { cimp.running = false; bimpError(e); return; }
  bimpPoll();
}
async function bimpPoll() {
  clearTimeout(bimp.timer);
  let p;
  try { p = await invoke("client_import_state"); } catch (e) { cimp.running = false; bimpError(e); return; }
  if (p.phase === "downloading" || p.phase === "unpacking" || p.phase === "checking") {
    bimpProgress(p);
    bimp.timer = setTimeout(bimpPoll, 400);
    return;
  }
  bimp.running = false;
  cimp.running = false;
  if (p.phase === "done") {
    await refresh();
    const where = autoConnect(p.client_id);
    if (where !== null) bimpView("done", "Ready — press PLAY", [p.client_name, where].filter(Boolean).join(" · "));
    else bimpView("done", `Added ${p.client_name || "your game"}`, "No realm plays this version yet: find one under Realms");
    await bimpSeedPoll();
    if (!bimp.seeding) bimp.hideAt = setTimeout(bimpIdle, 15000);
  } else if (p.phase === "cancelled") bimpView("error", p.message || "Stopped. Nothing was added.", "");
  else bimpError(p.message);
}
// The new client for every realm of its version, and a realm of that version in the bar: the official realm when it
// fits (Wrath Unleashed), else the first realm of the list, My Den or Single player of that version. Returns the
// realm's name, or null when none fits.
function autoConnect(id) {
  const c = state && state.settings.clients.find((x) => x.id === id);
  if (!c) return null;
  const off = (realmsCache || []).find((x) => x.official);
  const offBuild = (off && off.build) || "3.3.5a";
  let pick = null;
  if (offBuild === c.build) pick = { official: true };
  else {
    const r = (realmsCache || []).find((x) => !x.official && x.address && x.build === c.build && x.state === "live" && !hiddenBuild(x.build))
      || (realmsCache || []).find((x) => !x.official && x.address && x.build === c.build && !hiddenBuild(x.build));
    const L = Object.values(LOCAL).find((x) => x.build === c.build && localAddress(x));
    if (r) pick = { official: false, address: r.address };
    else if (L) pick = { official: false, address: localAddress(L) };
  }
  const v = clientChoices();   // as it was: pickRealm would note the new client for the realm it leaves
  if (pick) pickRealm(pick);
  // remembered for every realm of that version
  v.realm = v.realm && typeof v.realm === "object" ? v.realm : {};
  v.build = v.build && typeof v.build === "object" ? v.build : {};
  if (offBuild === c.build) v.realm.official = c.id;
  for (const r of realmsCache || []) if (!r.official && r.address && r.build === c.build) v.realm[r.address] = c.id;
  for (const L of Object.values(LOCAL)) if (L.build === c.build) v.realm[L.profile] = c.id;
  v.build[c.build] = c.id;
  try { localStorage.setItem(CHOICE_KEY, JSON.stringify(v)); } catch (_) { /* fine: picked now anyway */ }
  $("play-client").value = c.id;
  realmChanged();
  saveSelection();
  const need = neededBuild();
  return pick && mode !== "browser" && (!need || need === c.build) ? realmLabel() : null;
}
$("pbd-cancel").addEventListener("click", async () => {
  if (!bimp.running) { bimpView("idle"); return; }
  $("pbd-head").textContent = "Stopping…";
  try { await invoke("client_import_cancel"); } catch (_) { /* it ends by itself */ }
});
$("pbd-retry").addEventListener("click", () => { const i = bimp.input; bimpView("idle"); barImport(i); });
$("pbd-x").addEventListener("click", () => bimpView("idle"));
$("pbd").addEventListener("click", (e) => { if ($("pbd").dataset.state === "done" && !e.target.closest("button")) { clearTimeout(bimp.seedTimer); bimpView("idle"); } });
$("play-form").addEventListener("submit", () => { if ($("pbd").dataset.state === "done") bimpView("idle"); }, true);
// Over the realm box: "Drop to add your game"
function barDragOn(on) {
  if (bimp.running || $("pbd").dataset.state === "picking") return;
  if (on) bimpView("over", "Drop to add your game", "A game folder, a zip or a link");
  else if ($("pbd").dataset.state === "over") bimpView("idle");
}
function barDropAt(x, y) {
  const r = $("pb-realm-btn").getBoundingClientRect();
  return x >= r.left && x <= r.right && y >= r.top - 24 && y <= r.bottom;   // the box and its REALM label
}
{
  const mid = $("pb-realm-btn").closest(".pb-mid");
  mid.addEventListener("dragover", (e) => {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    barDragOn(true);
  });
  mid.addEventListener("dragleave", (e) => { if (!mid.contains(e.relatedTarget)) barDragOn(false); });
  mid.addEventListener("drop", (e) => {
    e.preventDefault();
    e.stopPropagation();
    barDragOn(false);
    const t = dropText(e.dataTransfer);
    if (t) barImport(t);
  });
}
// pasted with the realm box focused (capture: before the client list's paste)
document.addEventListener("paste", (e) => {
  const a = document.activeElement;
  if (!a || !a.closest || !a.closest(".pb-mid") || a.closest("input, textarea")) return;
  const t = ((e.clipboardData && e.clipboardData.getData("text")) || "").trim();
  if (t) { e.preventDefault(); barImport(t); }
}, true);
window.LeylineBarImport = { drop: barImport, dragOn: barDragOn, view: bimpView };

// Settings › Testing (clienttorrent.rs): saved on this PC only, off by default
async function loadTesting() {
  let t = { client_torrents: false, share: false };
  try { t = (await invoke("testing_get")) || t; } catch (_) { /* an older core: off */ }
  $("set-ctorrent").checked = !!t.client_torrents;
  $("set-cshare").checked = !!t.share;
  $("set-cshare").disabled = !t.client_torrents;
}
async function saveTesting() {
  try {
    const t = (await invoke("testing_set", { clientTorrents: $("set-ctorrent").checked, share: $("set-cshare").checked })) || {};
    $("set-ctorrent").checked = !!t.client_torrents;
    $("set-cshare").checked = !!t.share;
    $("set-cshare").disabled = !t.client_torrents;
    say("testing-status", t.client_torrents ? "On for this PC. Drop a magnet link or torrent file on the realm box." : "Off.", "ok");
  } catch (e) { sayErr("testing-status", e); loadTesting(); }
}
$("set-ctorrent").addEventListener("change", saveTesting);
$("set-cshare").addEventListener("change", saveTesting);
loadTesting();

// ------------------------------------------------------------------ the play bar's realm picker: a drop-up list
// (owner 2026-10-30: the ONLINE / SOLO / BROWSER switch gave way to it; owner 1 Nov: kept short, renderRealmMenu.)
// Picking a realm selects it the way the side bar's Your realms does, with a client of the version it needs. The same
// keys as the client list.
// Your Den (docs/DEN.md, owner 2026-10-30) and Single player (owner 2026-10-31: "add solo leveling"; renamed 4 Nov): the realms made
// for this PC, hosted by the host kit (its state/host.json profile "den" or "solo", read by host_info). One kit runs
// one realm (one state/host.json, one compose project), so the two take turns: the row of the one not running (Single player: its own row under Your Den) offers
// to switch (hostconsole.js asks, then runs it on the same realm, its accounts and characters kept). Each row says its
// next step: Needs your game folder (the add flow; setup goes on by itself once a folder of its build is in), Make your
// Den / Set it up once (the Host page with that ruleset picked: Host pack and Docker steps, then Start), Switch from
// <the other>, or On this PC (played as the Host page's Play would: host_invite, then join_invite: the relay's knock,
// the client pointed at it, then PLAY). PLAY on one of them knocks again first, and never fails without saying why.
const LOCAL = {
  den: { name: "Your Den", profile: "den", build: "3.3.5a", key: "leyline.den", mark: "den", setup: "Make your Den",
    title: "A private room to build in: you and the friends you invite" },
  // Single player (owner 4 Nov: the flagship, "Solo leveling" before; the ruleset id stays "solo"): its own page,
  // single.js, sets it up in three steps. TODO(owner): the final marketing line, his words
  solo: { name: "Single player", profile: "solo", build: "3.3.5a", key: "leyline.solo", mark: "solo", setup: "Set it up once",
    title: "Just you and your bot companions, on this PC: dungeons and raids with helper bots, levelling at your pace" },
};
const DEN = LOCAL.den;
const DEN_LITE = "127.0.0.1";   // where Den Lite plays (host/lite.rs ADDRESS)
const localHosted = (L) => !!(kit && kit.realm && kit.realm.profile === L.profile);
const denHosted = () => localHosted(DEN);
function localAddress(L) {
  try { return localStorage.getItem(L.key) || ""; } catch (_) { return ""; }
}
function denAddress() { return localAddress(DEN); }
// ---- this PC's own worlds on the Built-in engine (vanilla.js window.LeylineWorlds; host/engine.rs; owner 10 Nov:
// "robust single player in launcher"; Discord, Squishy: "why I can't select my solo server on the server list").
// Before, the realm picker had no row for the original world, and Single player's row started the game on the click
// without ever being picked, so it never looked selected. Now both are rows like any realm: a click picks it (the bar
// says what PLAY will do), and PLAY plays it the one way every local world plays (LeylineWorlds.run: set up once,
// started when stopped, then signed in by itself). Picked = the realm field at the engine's address with the world's
// kind kept in localStorage (leyline.world.chosen). Another world running on these ports: PLAY says so, and a second
// PLAY stops it (its characters are saved) and plays the picked one; nothing is stopped unasked.
var WORLD_ADDR = "127.0.0.1:3724";      // where the Built-in engine's worlds play (host/engine.rs)
var WORLD_BUILD = "3.3.5a";
var WORLD_KEY = "leyline.world.chosen";
var WORLD_KINDS = ["single", "vanilla"];
var WORLD_TITLE = { single: "Single player", vanilla: "Original world" };
var WORLD_LINE = { single: "Just you and your companions, on this PC", vanilla: "The original world at level 60, with companions, on this PC" };
var worldSt = {};                         // kind -> world_status (null: this core has no such world)
var worldStopOk = "";                     // the kind a second PLAY may stop the other world for
// (var and a function declaration, so the realm label and build can ask before this part of the file has run)
function worldChosen() {
  if (!WORLD_ADDR) return null;
  try { if (mode === "browser") return null; } catch (_) { return null; }
  if (isOfficial() || chosenRealm() !== WORLD_ADDR) return null;
  let k = "";
  try { k = localStorage.getItem(WORLD_KEY) || ""; } catch (_) { /* fine */ }
  return WORLD_KINDS.includes(k) ? k : "single";
}
async function readWorlds() {
  const got = await Promise.all(WORLD_KINDS.map((k) => invoke("world_status", { kind: k }).catch(() => null)));
  let changed = false;
  WORLD_KINDS.forEach((k, i) => {
    const v = got[i] && typeof got[i] === "object" ? got[i] : null;
    if (JSON.stringify(v) !== JSON.stringify(worldSt[k])) changed = true;
    worldSt[k] = v;
  });
  return changed;
}
// the Built-in engine is made for this PC (Windows for now): the worlds get their own rows
function worldsHere() { return !!(worldSt && worldSt.single && worldSt.single.supported); }
function worldSub(k) {
  const w = worldSt[k];
  if (!w) return "On this PC";
  if (w.running) return "Running on this PC";
  if (w.set_up) return "On this PC";
  if (!haveBuild(WORLD_BUILD)) return "Needs your game folder";
  return "Set it up once";
}
function pickWorld(k) {
  rememberClient(realmKey(), currentClient());
  if (mode !== "browser" && !isOfficial() && chosenRealm() !== WORLD_ADDR) rememberJoined(chosenRealm());
  try { localStorage.setItem(WORLD_KEY, k); } catch (_) { /* fine */ }
  $("play-realm").value = "custom";
  $("play-custom").value = WORLD_ADDR;
  if (mode === "browser") setMode("online");
  worldStopOk = "";
  useClientFor(realmKey(), WORLD_BUILD);
  realmChanged();
  saveSelection();
  renderPlayBar();
}
// the bar's line for a picked world (localHint)
function worldHint(k) {
  const w = worldSt[k];
  const title = WORLD_TITLE[k];
  if (!haveBuild(WORLD_BUILD)) return { head: "ADD YOUR GAME FOLDER", detail: `${title} plays with your own game folder`, warn: true };
  if (w && !w.supported) return { head: `${title.toUpperCase()} NEEDS ONE MORE STEP`, detail: "PLAY opens Single player to set it up", warn: true };
  if (w && w.running) return { head: "READY TO PLAY", detail: `${title}, running on this PC`, warn: false };
  if (w && w.set_up) return { head: "READY TO PLAY", detail: `${title}, on this PC: starts in seconds`, warn: false };
  return { head: "PLAY SETS IT UP", detail: `${title}, on this PC: the first time takes a while`, warn: false };
}
// PLAY on a picked world: the one-click flow (oneclick.js: the folder found or asked for, room on the disk, Quick start
// for a first entry, the setup card, errors with their buttons), else LeylineWorlds.run as before
async function playWorld(k) {
  const W = window.LeylineWorlds;
  const OC = window.LeylineOneClick;
  const title = WORLD_TITLE[k];
  if (!W) { show("single"); return; }
  if (!OC && !haveBuild(WORLD_BUILD)) { addGameFolder(`${title} plays with your own game folder. Add it, and PLAY goes on from there.`, WORLD_BUILD); return; }
  const stopOther = worldStopOk === k;
  worldStopOk = "";
  $("play-go").disabled = true;
  let played = false;
  const sayLine = (t, kind) => say("play-status", t, kind === "warn" ? "" : kind);
  const onOther = (e) => { worldStopOk = k; say("play-status", `${e.other.title} is running on this PC. Press PLAY again to stop it (its characters are saved) and play ${title}.`, "bad"); };
  const onDocker = () => { show("single"); say("play-status", `${title} needs one more step on this PC: Single player walks you through it.`, "bad"); };
  try {
    if (OC) {
      const r = await OC.play(k, { stopOther, say: sayLine, onOther, onDocker,
        job: (p) => { if (p && p.step) say("play-status", `${p.step}${p.pct != null ? ` (${Math.round(p.pct)}%)` : ""}…`); } });
      if (r) { statusFor = playKey(); played = true; }
      return;
    }
    const r = await W.run(k, {
      stopOther,
      say: (t) => say("play-status", t),
      job: (p) => { if (p && p.step) say("play-status", `${p.step}${p.pct != null ? ` (${Math.round(p.pct)}%)` : ""}…`); },
    });
    const [t, kind] = W.played(r, title);
    say("play-status", t, kind === "warn" ? "" : "ok");
    statusFor = playKey();
    played = true;
  } catch (e) {
    if (e && e.folder) addGameFolder(`${title} plays with your own game folder. Add it, and PLAY goes on from there.`, WORLD_BUILD);
    else if (e && e.docker) onDocker();
    else if (e && e.other) onOther(e);
    else sayErr("play-status", e);
  } finally {
    holdPlay($("play-go"), played);
    if (await readWorlds()) renderPlayBar();
  }
}

// the local realm the address its invite gave belongs to (the two share one realm, so the last one played keeps it)
const localAt = (addr) => (addr ? Object.values(LOCAL).find((L) => localAddress(L) === addr) || null : null);
// the kit as it is now (the realm picker asks each time it opens: a Den or Single player may have been made since)
async function readKit() {
  try { kit = await invoke("host_info"); } catch (_) { /* a preview page, or no kit */ }
  return kit;
}
// marks for the rows that are not a realm on the list: a house for My Den, a sword for Single player, a globe for
// browser play
const MARKS = {
  den: '<svg viewBox="0 0 16 16" width="14" height="14" focusable="false"><path d="M2.5 7.4 8 2.8l5.5 4.6M4 6.3v6.9h8V6.3M6.6 13.2V9.6h2.8v3.6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  solo: '<svg viewBox="0 0 16 16" width="14" height="14" focusable="false"><path d="M13.2 2.8 7 9l-.9-.9M13.2 2.8l-.4 2.1M13.2 2.8l-2.1.4M4.4 8.6l3 3M5.9 10.1 3 13" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  web: '<svg viewBox="0 0 16 16" width="14" height="14" focusable="false"><g fill="none" stroke="currentColor" stroke-width="1.4"><circle cx="8" cy="8" r="5.6"/><path d="M2.4 8h11.2M8 2.4c1.6 1.6 2.3 3.5 2.3 5.6S9.6 12 8 13.6M8 2.4C6.4 4 5.7 5.9 5.7 8s.7 4 2.3 5.6"/></g></svg>',
};
function markIcon(kind) {
  const box = el("span", "rl-ic sm pb-mark-ic pb-" + kind + "-ic");
  box.setAttribute("aria-hidden", "true");
  box.innerHTML = MARKS[kind];
  return box;
}
const denIcon = () => markIcon("den");
// the chosen realm is one of this PC's own (the address its invite gave)
const localChosen = () => (mode !== "browser" && !isOfficial() ? localAt(chosenRealm()) : null);
const denChosen = () => localChosen() === DEN;

// ---- which game client a realm launches with (owner 31 Oct: "choose client here"; "hopping between client versions
// should be easy too"). Kept per realm and per client version (localStorage leyline.clientFor: {realm: {key: client
// id}, build: {version: client id}}): picking a realm takes the client used there before, else the last one of the
// version it needs, else any of that version; none of it: one plain line and Add a game folder….
const CHOICE_KEY = "leyline.clientFor";
function clientChoices() {
  try { const v = JSON.parse(localStorage.getItem(CHOICE_KEY) || "{}"); return v && typeof v === "object" ? v : {}; } catch (_) { return {}; }
}
function rememberClient(key, c) {
  if (!c) return;
  const v = clientChoices();
  v.realm = v.realm && typeof v.realm === "object" ? v.realm : {};
  v.build = v.build && typeof v.build === "object" ? v.build : {};
  if (key) { delete v.realm[key]; v.realm[key] = c.id; }
  if (c.build) v.build[c.build] = c.id;
  const ks = Object.keys(v.realm);
  ks.slice(0, Math.max(0, ks.length - 40)).forEach((k) => delete v.realm[k]);   // the 40 latest realms
  try { localStorage.setItem(CHOICE_KEY, JSON.stringify(v)); } catch (_) { /* fine: the first fitting one then */ }
}
// the chosen realm's key for that memory (none in the browser)
const realmKey = () => (mode === "browser" ? null : isOfficial() ? "official" : localChosen() ? localChosen().profile : chosenRealm() || null);
// the client version the chosen realm needs, when known
const neededBuild = () => (mode === "browser" ? null : localChosen() ? localChosen().build : chosenRealmBuild());
function clientForRealm(key, build) {
  const have = state ? state.settings.clients : [];
  const v = clientChoices();
  const fits = (c) => !!c && (!build || c.build === build);
  const byId = (id) => have.find((x) => x.id === id);
  const cur = currentClient();
  // the client picked now first, when it can play there (owner 2 Nov: PLAY starts exactly the picked client); the
  // one used there before only when the realm needs another version
  return [cur, byId((v.realm || {})[key]), byId((v.build || {})[build]), ...have].find(fits) || null;
}
function useClientFor(key, build) {
  const c = clientForRealm(key, build);
  if (c && $("play-client").value !== c.id) $("play-client").value = c.id;
  return c;
}
function chooseClient(c) {
  $("play-client").value = c.id;
  rememberClient(realmKey(), c);
  realmChanged();
  saveSelection();
}
// the existing add flow: the game client list's own "Add a game folder…" (whatever it does), else the Clients page
function addGameFolder(why, ver) {
  if (why) { say("play-status", why, "warn", ver); say("cl-status", why, "warn", ver); }
  renderClientMenu();
  const add = [...$("pb-menu").querySelectorAll(".pb-opt")].find((r) => /add a game folder/i.test(r.textContent));
  if (add) add.click(); else show("clients");
}

// ---- the local realms' steps
let pendingLocal = null;   // a local realm waiting for its game folder: it goes on once one is added
let localKnocked = false;  // PLAY right after a knock: no second knock
const haveBuild = (b) => !!state && state.settings.clients.some((x) => x.build === b);
function localStep(L) {
  if (localHosted(L)) return "play";
  if (!haveBuild(L.build)) return "folder";
  if (kit && kit.realm && Object.values(LOCAL).some((o) => o !== L && localHosted(o))) return "switch";
  if (kit && kit.realm) return "busy";
  return "setup";
}
// Single player set up on the Built-in engine (single.js, host/engine.rs: no Host pack, no relay): its row plays it
const soloBuiltin = (L) => L === LOCAL.solo && !!(window.LeylineSingle && window.LeylineSingle.builtin && window.LeylineSingle.builtin());
function localSub(L) {
  if (soloBuiltin(L)) return "On this PC";
  const step = localStep(L);
  if (step === "play") return "On this PC";
  if (step === "folder") return "Needs your game folder";
  if (step === "switch") return `Switch from ${Object.values(LOCAL).find(localHosted).name}`;
  if (step === "busy") return `Your PC hosts ${kit.realm.name || "a realm"}`;
  return L.setup;
}
function localGo(L) {
  if (soloBuiltin(L)) { pendingLocal = null; window.LeylineSingle.play(); return; }
  const step = localStep(L);
  // My Den without a Docker Den running: Your Den's one click (Den Lite straight in, else its setup there), not
  // the Host page (owner 1 Nov 02:46: "clicking the button doesn't launch the den")
  if (L === DEN && step !== "play" && window.LeylineDen && window.LeylineNav) {
    pendingLocal = null;
    // the bar follows at once (owner 2 Nov UX pass: it kept Wrath Unleashed / READY TO PLAY and the old line): Your Den
    // on Den Lite is 127.0.0.1; else the old line goes
    if (window.LeylineDen.where && window.LeylineDen.where() === "lite") {
      $("play-realm").value = "custom"; $("play-custom").value = DEN_LITE;
      if (mode === "browser") setMode("online");
      realmChanged();
    } else { say("play-status", ""); $("play-status").title = ""; }
    window.LeylineNav.show("den");
    window.LeylineDen.go();
    return;
  }
  if (step === "play") { playLocal(L); return; }
  // Single player not on this PC yet: its own page (single.js: game folder, engine, Start), not the Host page
  if (L === LOCAL.solo && $("page-single")) {
    pendingLocal = null;
    show("single");
    return;
  }
  if (step === "folder") {
    pendingLocal = L.profile;
    addGameFolder(`${L.name} plays with your own game folder. Add it, and setup goes on from there.`, L.build);
    return;
  }
  pendingLocal = null;
  show("host");
  document.dispatchEvent(new CustomEvent("leyline:host-ruleset", { detail: { profile: L.profile, switch: step === "switch" } }));
}
function makeDen() { localGo(DEN); }
// a folder of the waited-for version is in: on to the next step
function continuePending() {
  const P = pendingLocal && LOCAL[pendingLocal];
  if (!P || !haveBuild(P.build)) return;
  pendingLocal = null;
  say("play-status", "");
  say("cl-status", `Found it. On to ${P.name}…`, "ok");
  setTimeout(() => localGo(P), 0);
}
// it didn't answer: the Host page, which says whether it runs and has its Start
function localNotUp(L, why) {
  say("play-status", `${why} Start it on the Host page.`, "warn");
  show("host");
  document.dispatchEvent(new CustomEvent("leyline:host-ruleset", { detail: { profile: L.profile, start: true, why } }));
}
async function playLocal(L) {
  if (!(await knockLocal(L))) return;
  localKnocked = true;
  $("play-form").requestSubmit();
}
// the knock: the client of its version pointed at it, the bar set to it (true), else its next step said (false)
async function knockLocal(L) {
  await readKit();
  if (kit && kit.is_kit && !localHosted(L)) { localGo(L); return false; }   // not set up (any more): its next step
  const own = clientForRealm(L.profile, L.build);
  if (!own) {
    pendingLocal = L.profile;
    addGameFolder(`${L.name} plays with your own game folder. Add it, then pick ${L.name} again.`, L.build);
    return false;
  }
  say("play-status", `Opening ${L.name}…`);
  let link = null;
  try { link = await invoke("host_invite"); } catch (_) { /* no kit */ }
  if (!link) { localNotUp(L, `${L.name} has no invite link yet.`); return false; }
  let j;
  try { j = await invoke("join_invite", { client: own.id, link }); }
  catch (e) { localNotUp(L, oops(e, `${L.name} didn't answer.`)); return false; }
  for (const o of Object.values(LOCAL)) {
    try {
      if (o === L) localStorage.setItem(o.key, j.address);
      else if (localStorage.getItem(o.key) === j.address) localStorage.removeItem(o.key);
    } catch (_) { /* the bar then shows the address */ }
  }
  rememberClient(realmKey(), currentClient());
  $("play-client").value = own.id;
  $("play-realm").value = "custom";
  $("play-custom").value = j.address;
  if (mode === "browser") setMode("online");
  rememberClient(L.profile, own);
  realmChanged();
  saveSelection();
  return true;
}
// PLAY with a local realm chosen: knock again first (true: stop, its next step was said)
async function localBeforePlay() {
  // Your Den on Den Lite (127.0.0.1): PLAY is Your Den's one click (den.js: start it, sign in by itself), never a
  // plain start at a login screen (smoke O3)
  // (owner 2 Nov 20:48 "Den still doesn't go straight to den from launcher": with the Den page never opened since the
  // launcher started, where() had nothing to go on and PLAY started the game at the login screen. go() reads it.)
  if (localKnocked) { localKnocked = false; return false; }   // the knock just ran (Your Den's Docker play too): PLAY on
  if (chosenRealm() === DEN_LITE && window.LeylineDen) { window.LeylineDen.go(); return true; }
  const L = localChosen();
  if (!L || localKnocked) { localKnocked = false; return false; }
  return !(await knockLocal(L));
}
// the bar's status line for a local realm (renderPlayBar)
function localHint() {
  continuePending();
  if (worldChosen()) return worldHint(worldChosen());
  if (chosenRealm() === DEN_LITE) return { head: "READY TO PLAY", detail: "Your Den, on this PC", warn: false };
  const L = localChosen();
  if (!L) return null;
  if (kit && kit.is_kit && !localHosted(L)) return { head: `${L.name.toUpperCase()} IS NOT SET UP`, detail: "PLAY sets it up", warn: true };
  if (!clientForRealm(L.profile, L.build)) return { head: "ADD YOUR GAME FOLDER", detail: `${L.name} needs your game folder`, warn: true };
  return { head: "READY TO PLAY", detail: "On this PC", warn: false };
}
// the realm the Host pack runs on this PC, whatever its ruleset (solo.js Play solo on a mode package): knocked and
// played as Your Den and Single player are
function hostedLocal() {
  if (!(kit && kit.realm)) return null;
  const L = Object.values(LOCAL).find(localHosted);
  const ver = /(\d+\.\d+\.\d+a?)$/.exec(kit.realm.profile || "");
  return L || { name: kit.realm.name || "Your realm", profile: kit.realm.profile, build: ver ? ver[1] : "3.3.5a", key: "leyline.hosted", mark: "solo" };
}
window.LeylineLocal = { play: (id) => (LOCAL[id] ? playLocal(LOCAL[id]) : null), names: () => Object.fromEntries(Object.values(LOCAL).map((L) => [L.profile, L.name])),
  // single.js: the next step of a local realm, and the add flow for its game folder (it goes on to that realm once in)
  step: (id) => (LOCAL[id] ? localStep(LOCAL[id]) : null), info: (id) => LOCAL[id] || null, readKit,
  addFolder: (id, why) => { const L = LOCAL[id]; if (!L) return; pendingLocal = L.profile; addGameFolder(why || `${L.name} plays with your own game folder. Add it, and setup goes on from there.`, L.build); },
  useClient: (id, c) => { const L = LOCAL[id]; if (L && c) rememberClient(L.profile, c); },
  clientFor: (id) => (LOCAL[id] ? clientForRealm(LOCAL[id].profile, LOCAL[id].build) : null),
  playHosted: async () => { await readKit(); const L = hostedLocal(); if (L) return playLocal(L); show("host"); return null; } };
let rmenuOpen = false;
const realmRowsOf = (m) => [...m.querySelectorAll(".pb-opt")];
function pickRealm(r) {
  rememberClient(realmKey(), currentClient());   // the client used with the realm left, for next time
  if (mode !== "browser" && !isOfficial()) rememberJoined(chosenRealm());   // the realm left stays under Your realms
  $("play-realm").value = r.official ? "official" : "custom";
  if (!r.official) $("play-custom").value = r.address || "";
  if (mode === "browser") setMode("online");
  useClientFor(realmKey(), neededBuild());       // another version: a client of that version, by itself
  realmChanged();
  saveSelection();
}
// the picker, kept short (owner 1 Nov 03:22: "too messy lets make everything more intuitive and less redundant and
// proper"; "we only need Wrath Unleashed and local Dens for now, and the ability to 1-click host new servers from
// templates"): Wrath Unleashed (and the official lineup when listed) and Your Den; Your realms (only when you host one
// or picked a community realm); Host a server… (the Host page's templates) and a quiet Play in your browser; then the
// one game client the realm plays with (its path in the tooltip; a click opens the client list).
function renderRealmMenu() {
  const menu = $("pb-rmenu");
  const rows = [];
  const row = (cls, label, opts) => {
    const li = el("li", "pb-opt " + cls);
    li.setAttribute("role", "option");
    li.tabIndex = -1;
    li.id = "pb-ropt-" + rows.length;
    if (opts.icon) li.append(opts.icon);
    li.append(el("span", "pb-opt-name", label));
    if (opts.kind) li.append(el("span", "pb-opt-kind", opts.kind));
    if (opts.ver) li.append(el("span", "rl-ver sm", opts.ver));
    if (opts.sub) li.append(el("span", "pb-opt-sub" + (opts.subCls ? " " + opts.subCls : ""), opts.sub));
    li.setAttribute("aria-selected", String(!!opts.selected));
    if (opts.disabled) li.setAttribute("aria-disabled", "true");
    if (opts.title) li.title = opts.title;
    li.addEventListener("click", () => { if (opts.disabled) return; closeRealmMenu(!opts.keepFocus); opts.run(); });
    rows.push(li);
    return li;
  };
  const head = (text) => { const h = el("li", "pb-head", text); h.setAttribute("role", "presentation"); return h; };
  const sep = () => { const d = el("li", "pb-sep"); d.setAttribute("role", "presentation"); return d; };
  const online = mode !== "browser";
  const here = chosenRealm();
  const players = (r) => (!r ? "" : r.state !== "live" ? "Offline" : r.players == null ? "Online" : `${r.players} playing`);
  const realmRow = (r) => row("pb-realm-opt" + (r.state === "live" ? "" : " off"), r.name, {
    icon: realmIcon(r.name, r.official, "sm"), ver: r.official ? "3.3.5a" : realmVersion(r), sub: players(r), subCls: r.state === "live" ? "live" : "",
    kind: r.official ? HANGOUT.kind : "",
    selected: online && (r.official ? isOfficial() : !isOfficial() && r.address === here), title: r.tagline || r.name, run: () => pickRealm(r) });
  const localRow = (L) => {
    const on = localStep(L) === "play";
    return row("pb-local pb-" + L.mark, L.name, { icon: markIcon(L.mark), ver: on ? L.build : "", sub: L === DEN ? denSub() : localSub(L), subCls: on ? "live" : "",
      selected: localChosen() === L, title: on ? `${L.name} on this PC: play it` : L.title, run: () => localGo(L) });
  };
  // Wrath Unleashed (HANGOUT) labelled, the lineup when its record is on the list, then Your Den
  const off = (realmsCache || []).find((x) => x.official) || { official: true, name: officialName(), state: "live", players: null };
  const lineup = (realmsCache || []).filter((r) => inLineup(r) && r.address && !hiddenBuild(r.build));
  // Single player (owner 4 Nov: the flagship) has its own row right under Your Den, hosted here or not; on the Built-in
  // engine, Single player and the original world are picked like any realm (worldRow), unless Single player runs on
  // Docker here (then its Docker row, as before)
  const worldRow = (k) => {
    const w = worldSt[k];
    const on = !!(w && (w.set_up || w.running));
    return row("pb-local pb-world pb-world-" + k, WORLD_TITLE[k], { icon: markIcon("solo"), ver: on ? WORLD_BUILD : "", sub: worldSub(k),
      subCls: w && w.running ? "live" : "", selected: worldChosen() === k, title: WORLD_LINE[k], run: () => pickWorld(k) });
  };
  const builtinSolo = worldsHere() && !localHosted(LOCAL.solo);
  const solo = builtinSolo ? [worldRow("single"), worldRow("vanilla")] : [localRow(LOCAL.solo)];
  const top = [realmRow(off), ...lineup.map(realmRow), localRow(DEN), ...solo];
  // Favourites: the realms starred on the Realms page (the list kit's store, leyline.list.realms.fav), the ones not
  // already above
  const favStore = window.LeylineList ? window.LeylineList.favourites("realms") : null;
  const favs = favStore ? (realmsCache || []).filter((r) => !r.official && !inLineup(r) && r.address && !hiddenBuild(r.build) && !hiddenRealm(r)
    && favStore.has(realmKeyOf(r)) && !(kit && kit.realm && kit.realm.realm_id && r.realm_id === kit.realm.realm_id)) : [];
  // Your realms: the one this PC hosts (not the Den or Single player: they have their rows) and the community realm you picked
  const mine = [];
  const hosted = kit && kit.realm && kit.realm.profile !== DEN.profile && kit.realm.profile !== LOCAL.solo.profile ? kit.realm : null;
  const hostedLocal = hosted ? Object.values(LOCAL).find((L) => L.profile === hosted.profile) : null;
  const hostedListed = hosted && hosted.realm_id ? (realmsCache || []).find((r) => r.realm_id === hosted.realm_id && r.address) : null;
  if (hostedLocal) mine.push(localRow(hostedLocal));
  else if (hostedListed) mine.push(realmRow(hostedListed));
  else if (hosted) mine.push(row("pb-realm-opt pb-hosted", hosted.name || "Your realm", { icon: realmIcon(hosted.name || "Your realm", false, "sm"), sub: "On this PC",
    run: () => show("host") }));
  // and the community realms you played on: the one picked now (or saved), then the ones before it
  const saved = !isOfficial() ? here : (state.settings.selected_realm && !isOfficialAddr(state.settings.selected_realm) ? state.settings.selected_realm : "");
  for (const addr of [...new Set([saved, ...joinedRealms()].filter(Boolean))]) {
    const r = realmAt(addr);
    const L = localAt(addr);
    if (L) { if (L !== DEN && L !== LOCAL.solo && L !== hostedLocal) mine.push(localRow(L)); }
    else if (r) { if (!inLineup(r) && r !== hostedListed && !hiddenRealm(r) && !r.official && !favs.includes(r)) mine.push(realmRow(r)); }
    else if (!isOfficialAddr(addr)) mine.push(row("pb-realm-opt", addr, { icon: realmIcon(addr, false, "sm"), selected: online && !isOfficial() && addr === here, sub: "Your address",
      run: () => pickRealm({ official: false, address: addr }) }));
  }
  const acts = [
    row("pb-act pb-host-act", "Host a server…", { title: "Host a realm on this PC from a template: one click", run: hostFromTemplate }),
    row("pb-browser pb-quiet", "Play in your browser", { selected: !online, title: "The official realm in your browser: nothing to install",
      run: () => setMode("browser") }),
    // a friend's Den (docs/DEN-HOSTING.md): their link or code, pasted once
    row("pb-act pb-join-den", "Join a friend's Den…", { title: "Paste the link or code your friend sent you", run: joinFriendsDen }),
  ];
  // the game client this realm plays with: one row (its path only in the tooltip; a click opens the client list, which
  // has Add a game folder…); none of the version it needs: one plain line and Add a game folder…
  const clients = [];
  if (online) {
    const need = neededBuild();
    const cur = currentClient();
    const fit = cur && (!need || cur.build === need) ? cur : state.settings.clients.find((x) => !need || x.build === need);
    if (fit) {
      clients.push(row("pb-client-opt", fit.name, { ver: clientBadge(fit), sub: "Game client", title: `${fit.name} · ${fit.path}\nPick another game client or add a folder`,
        keepFocus: true, run: () => openClientMenu() }));
    } else {
      const note = el("li", "pb-note", need ? "No game folder for this realm yet." : "No game folder yet.");
      if (need) note.append(" ", el("span", "rl-ver sm", need));
      note.setAttribute("role", "presentation");
      clients.push(note, row("pb-act pb-add-client", "Add a game folder…", { run: () => addGameFolder(need ? "Add your game folder for this realm." : "", need) }));
    }
  }
  const parts = [top];
  if (favs.length) parts.push([head("Favourites"), ...favs.map(realmRow)]);
  if (mine.length) parts.push([head("Your realms"), ...mine]);
  parts.push(acts);
  if (clients.length) parts.push(clients);
  menu.replaceChildren(...parts.flatMap((g, i) => (i ? [sep(), ...g] : g)));
}
// Your Den's row: what one click does now (den.js: Den Lite straight in; else the Docker Den, started if stopped)
function denSub() {
  const step = localStep(DEN);
  if (step === "play") return "On this PC";
  if (step === "folder") return "Needs your game folder";
  const w = window.LeylineDen && window.LeylineDen.where ? window.LeylineDen.where() : "";
  if (w === "lite") return "One click, on this PC";   // Den Lite runs beside anything the Host pack runs
  if (step === "switch" || step === "busy") return localSub(DEN);
  if (w === "docker") return "Stopped: one click starts it";
  return "Make your Den";
}
// Join a friend's Den…: Realms' invite box, open and ready for the link or code they sent (app.rs join_invite takes
// both; host/denhost.rs)
function joinFriendsDen() {
  show("realms");
  const o = $("rl-other");
  if (o) o.open = true;
  const i = $("rl-invite");
  if (i) { i.focus(); if (i.scrollIntoView) i.scrollIntoView({ block: "center" }); }
  say("rl-status", "Paste the link or code your friend sent you, then press Join. Your Leyline finds the way in.", "");
}
// Host a server…: the Host page's templates (hostmodes.js: one click to host from a template)
function hostFromTemplate() {
  if (window.LeylineHostModes && window.LeylineHostModes.open) window.LeylineHostModes.open();
  else show("host");
}
function openRealmMenu() {
  if (rmenuOpen) return;
  closeClientMenu(false);
  renderRealmMenu();
  rmenuOpen = true;
  $("pb-rmenu").hidden = false;
  $("pb-realm-btn").setAttribute("aria-expanded", "true");
  const focusPicked = () => {
    const rows = realmRowsOf($("pb-rmenu"));
    const at = rows.find((r) => r.getAttribute("aria-selected") === "true") || rows[0];
    if (at) at.focus();
  };
  focusPicked();
  // Your Den and Single player: hosted on this PC or not, as the kit says now, and where Your Den runs (den.js, read
  // once): the rows change only when that changed
  const hosted = () => (kit && kit.realm ? kit.realm.profile : "") + "|" + denSub() + "|" + JSON.stringify(worldSt);
  const had = hosted();
  const den = window.LeylineDen && !window.LeylineDen.state() ? window.LeylineDen.load() : null;
  Promise.all([readKit(), den, readWorlds()]).then(() => {
    if (!rmenuOpen || hosted() === had) return;
    const i = realmRowsOf($("pb-rmenu")).indexOf(document.activeElement);
    renderRealmMenu();
    const rows = realmRowsOf($("pb-rmenu"));
    if (i >= 0 && rows[i]) rows[i].focus(); else focusPicked();
  });
}
function closeRealmMenu(refocus) {
  if (!rmenuOpen) return;
  rmenuOpen = false;
  $("pb-rmenu").hidden = true;
  $("pb-realm-btn").setAttribute("aria-expanded", "false");
  if (refocus) $("pb-realm-btn").focus();
}
$("pb-realm-btn").addEventListener("click", () => (rmenuOpen ? closeRealmMenu(true) : openRealmMenu()));
$("pb-realm-btn").addEventListener("keydown", (e) => {
  if (e.key === "ArrowUp" || e.key === "ArrowDown") { e.preventDefault(); openRealmMenu(); }
});
$("pb-rmenu").addEventListener("keydown", (e) => {
  const rows = realmRowsOf($("pb-rmenu"));
  const i = rows.indexOf(document.activeElement);
  let j = null;
  if (e.key === "ArrowDown") j = Math.min(rows.length - 1, i + 1);
  else if (e.key === "ArrowUp") j = Math.max(0, i - 1);
  else if (e.key === "Home") j = 0;
  else if (e.key === "End") j = rows.length - 1;
  else if (e.key === "Escape") { e.preventDefault(); closeRealmMenu(true); return; }
  else if (e.key === "Tab") { closeRealmMenu(false); return; }
  else if ((e.key === "Enter" || e.key === " ") && i >= 0) { e.preventDefault(); rows[i].click(); return; }
  if (j === null) return;
  e.preventDefault();
  rows[j].focus();
});
document.addEventListener("pointerdown", (e) => {
  if (rmenuOpen && !e.target.closest(".pb-mid .pb-pick")) closeRealmMenu(false);
});
// a star on the Realms page: the picker's Favourites follow at once
document.addEventListener("leyline:list-fav", (e) => { if (rmenuOpen && e.detail && e.detail.list === "realms") renderRealmMenu(); });

function renderPlayBar() {
  if (!state) return;
  const c = currentClient();
  const bar = $("play-headline").parentElement;   // the status line under the realm's name
  let head = "READY TO PLAY", detail = "", warn = false;
  if (mode === "browser" && officialGamedata === false) {
    head = "BROWSER PLAY · NEEDS YOUR GAME FOLDER"; warn = true;
    detail = "The browser will ask for your game folder (it stays on your PC)";
  } else if (mode === "browser") {
    head = "READY · NOTHING TO INSTALL";
  } else if (!c) {
    // no game folder: PLAY opens Find your game files (which has browser play too)
    head = "ADD YOUR GAME FOLDER"; warn = true;
    detail = "PLAY shows where to find it, or play in your browser";
  } else if (chosenRealmBuild() && c.build !== chosenRealmBuild()) {
    const need = chosenRealmBuild();
    head = "THIS REALM NEEDS ANOTHER GAME FOLDER"; warn = true;   // its version: the chip beside its name
    const other = state.settings.clients.find((x) => x.build === need);
    detail = other ? `Pick ${other.name}` : "Add the game folder for this realm";
  } else if (!chosenRealm()) {
    head = "CHOOSE A REALM"; warn = true;
    detail = "Type the realm's address under Realms";
  } else {
    detail = "";
  }
  if (queueN() && head.startsWith("READY")) {   // a line on the official realm: say so, and offer the other ways in
    head = `BUSY · QUEUE ${queueN()}`; warn = true;
    detail = mode === "browser" && officialQueue.wait ? `About ${officialQueue.wait} min · or skip the line` : "Or skip the line";
  }
  // the realm's own state (refreshStatus): never a green READY over a realm that is down or didn't answer. PLAY still
  // works: it may be back by the time the game opens.
  if (mode !== "browser" && head.startsWith("READY") && !localChosen() && !worldChosen()) {
    if (realmState === "offline") { head = "OFFLINE · WORLD SERVER DOWN"; warn = true; detail = "PLAY still tries: it may be back soon"; }
    else if (realmState === "partial") { head = "PARTLY UP · WORLD SERVER DOWN"; warn = true; detail = "Login answers; the world may not yet"; }
    else if (realmState === "unknown") { head = "CAN'T CHECK THE REALM"; warn = true; detail = "You may be offline · PLAY still tries"; }
  }
  const lh = mode === "browser" ? null : localHint();   // My Den / Single player: their own next step
  if (lh) ({ head, detail, warn } = lh);
  $("play-headline").textContent = head;
  bar.classList.toggle("warn", warn);
  $("pb-detail").textContent = detail;
  $("pb-realm").textContent = mode === "browser" ? "In your browser" : realmLabel();
  $("pb-realm-btn").title = mode === "browser" ? "Plays in your browser: choose a realm instead" : `${realmLabel()}: choose another realm`;
  // the realm's icon and client version beside its name (none in the browser)
  const official = isOfficial();
  const rv = mode === "browser" ? null : official ? "3.3.5a" : worldChosen() ? WORLD_BUILD : localChosen() ? localChosen().build : realmVersion(realmAt(chosenRealm()));
  $("pb-realm-ver").hidden = !rv;
  $("pb-realm-ver").textContent = rv || "";
  $("pb-realm-ic").hidden = mode === "browser";
  if (mode !== "browser") fillRealmIcon($("pb-realm-ic"), realmLabel(), official);
  $("pb-realm-ic").classList.toggle("pb-mark-ic", !!localChosen() || !!worldChosen());
  if (localChosen()) $("pb-realm-ic").innerHTML = MARKS[localChosen().mark];
  else if (worldChosen()) $("pb-realm-ic").innerHTML = MARKS.solo;
  if (rmenuOpen) renderRealmMenu();
  renderClientMenu();
  $("play-caption").textContent = mode === "browser" ? "IN YOUR BROWSER" : (c ? `ONLINE  ·  ${c.name.toUpperCase()}` : "NO GAME FOLDER YET");
  renderQueueCard();
}
if ($("pbq-host")) $("pbq-host").addEventListener("click", () => show("host"));
// Become a node: the Host page's Be a node card (node.js, 5 Nov), the website's guide only without it
if ($("pbq-node")) $("pbq-node").addEventListener("click", () => (window.LeylineNode ? window.LeylineNode.focus() : openLink("beanode")));
if ($("pbq-pc")) $("pbq-pc").addEventListener("click", () => setMode("online"));   // your own game folder: no browser seat
if ($("pbq-x")) $("pbq-x").addEventListener("click", () => { queueCardShut = true; renderQueueCard(); });
document.addEventListener("leyline:mode", () => { if (officialQueue.realm || officialQueue.browser) renderRealmRows(); });   // the row's count follows the mode

$("play-realm").addEventListener("change", () => { realmChanged(); saveSelection(); });
$("play-custom").addEventListener("change", () => { realmChanged(); saveSelection(); });
$("play-client").addEventListener("change", () => { realmChanged(); saveSelection(); });

$("play-form").addEventListener("submit", async (ev) => {
  ev.preventDefault();
  if (mode === "browser") {
    try { await invoke("open_browser_play"); say("play-status", "Opened the browser client.", "ok"); }
    catch (e) { sayErr("play-status", e); }
    return;
  }
  if (worldChosen()) { await playWorld(worldChosen()); return; }   // Single player / the original world, Built-in engine
  if (await localBeforePlay()) return;   // My Den / Single player: knocked again first, or its next step
  const c = currentClient();
  if (!c) { openFindFiles(); return; }   // no game folder yet: where to find one (and browser play)
  // the official realm signs in by itself with your Leyline account, so never a start signed out (it opened at the
  // stock login screen): the sign-in screen first, then this PLAY goes on (gate.js)
  if (playHeld()) return;   // a game started a moment ago (holdPlay)
  if (isOfficial() && !signedInFirst(() => $("play-form").requestSubmit())) return;
  const realm = chosenRealm();
  if (!realm) { show("realms"); $("play-custom").focus(); return say("play-status", "Type a realm address.", "bad"); }
  $("play-go").disabled = true;
  let played = false;
  try {
    // the realm's mods (gamemods.js): one offer for the client mods it uses that this game lacks; Cancel stops PLAY
    const rr = isOfficial() ? realmsCache.find((x) => x.official) : realmAt(realm);
    // the realm's suggested loadout (loadouts.js): "Use this realm's loadout" once; Cancel stops PLAY
    if (window.LeylineLoadouts && rr && !(await window.LeylineLoadouts.beforeJoin(rr, c)).go) { say("play-status", ""); return; }
    const j = window.LeylineGameMods && rr ? await window.LeylineGameMods.beforeJoin(rr, c, (t, k) => say("play-status", t, k)) : { go: true, leyline: false };
    if (!j.go) { say("play-status", ""); return; }
    // the Leyline addons up to date first (any realm), and offered on the official realm (addons.js)
    if (window.LeylineAddons && !j.leyline) await window.LeylineAddons.beforePlay(c, { offer: isOfficial() });
    say("play-status", "Starting…");
    const started = await invoke("play", { client: c.id, realm });
    say("play-status", playLine(started, c), "ok");
    if (window.LeylineAddons) window.LeylineAddons.afterPlay();   // "In-game add-ons updated" on the play bar
    $("play-status").title = String(started);   // the whole of it on hover
    statusFor = playKey();
    played = true;
    state = tidyState(await invoke("get_state"));
  } catch (e) {
    sayErr("play-status", e);
  } finally {
    holdPlay($("play-go"), played);
  }
});

// A game just started: PLAY stays off a moment, so a double click (or an impatient second press) never starts a second
// game (review 4 Nov: the core answers as soon as the game is spawned, and PLAY was on again at once).
// The hold is the launcher's, not one button's: the realm cards are rebuilt after a start (renderRealms), so a second
// click on the new card's Play, or PLAY in the bar right after a card's Play, is held too (review 4 Nov).
const PLAY_HOLD_MS = 2500;
let playHeldUntil = 0;
const playHeld = () => Date.now() < playHeldUntil;
function holdPlay(btn, started) {
  if (started) {
    playHeldUntil = Date.now() + PLAY_HOLD_MS;
    // the bar's PLAY shows it is held too when a card started the game (it is never off for another reason)
    const bar = $("play-go");
    if (bar && bar !== btn) { bar.disabled = true; setTimeout(() => { bar.disabled = false; }, PLAY_HOLD_MS); }
  }
  if (!btn) return;
  if (!started && !playHeld()) { btn.disabled = false; return; }
  btn.disabled = true;
  setTimeout(() => { btn.disabled = false; }, Math.max(0, playHeldUntil - Date.now()));
}

// ------------------------------------------------------------------ clients
function renderClients() {
  const list = $("cl-list");
  list.replaceChildren();
  if (state.settings.clients.length === 0) {
    // the empty state: where to look first
    const p = el("p", "hint cl-empty", "No game folder yet. ");
    const go = el("button", "link", "Find your game files ›");
    go.type = "button";
    go.addEventListener("click", () => openFindFiles());
    p.append(go);
    list.append(p);
    return;
  }
  for (const c of state.settings.clients) {
    const card = el("div", "card");
    const top = el("div", "top");
    top.append(el("span", "name", c.name), el("span", "build", offeredBuild(c.build) ? c.build : "other build"));
    const tags = el("div", "tags");
    if (buildTag(c.build) === "other") tags.append(el("span", "tag warn", "Realms only, no mods"));
    const meta = el("div", "meta");
    meta.append(el("span", "", "Folder: " + c.path));
    const check = el("span", "", "");
    meta.append(check);
    // the technical lines, for whoever is helping: behind Details
    const more = el("details", "cl-more");
    more.append(el("summary", "", "Details"));
    const tech = el("div", "meta");
    if (buildTag(c.build) === "other") tech.append(el("span", "", `Build: ${c.exe_version || "unknown"}`));
    else {
      tech.append(el("span", "", `Build: ${buildLabel(c.build)}`));
      tech.append(el("span", "", `Addons: Interface ${{"1.12.1": "11200", "2.4.3": "20400", "3.3.5a": "30300", "4.3.4": "40300", "5.4.8": "50400"}[c.build]}`));
    }
    if (BUILDS[c.build]) tech.append(el("span", "", `Realm address file: ${BUILDS[c.build].file}`));
    tech.append(el("span", "", "Realm: " + (c.realmlist || "not set")));
    if (c.locale) tech.append(el("span", "", "Locale: " + c.locale));
    more.append(tech);
    const acts = el("div", "acts");
    const bCheck = el("button", "btn small", "Check");
    const bRename = el("button", "btn small", "Rename");
    const bRemove = el("button", "btn small danger", "Remove");
    acts.append(bCheck, bRename, bRemove);
    card.append(top, tags, meta, more, acts);
    card.dataset.client = c.id;
    if (window.LeylineAddons) window.LeylineAddons.clientCard(card, c);   // the Leyline addons row (addons.js)
    if (window.LeylineLoadouts) window.LeylineLoadouts.clientCard(card, c);   // "Loadout: <name>" and Change (loadouts.js)
    list.append(card);

    const runCheck = async () => {
      check.className = "";
      check.textContent = "Checking…";
      try {
        const d = await invoke("check_client", { id: c.id });
        if (d.complete) {
          check.className = "ok";
          check.textContent = "Looks complete";
          if (d.exe_version) check.title = `Executable ${d.exe_version}`;   // the technical part, on hover
        } else {
          check.className = "warn";
          check.textContent = "Missing: " + d.missing.join(", ");
        }
      } catch (e) {
        check.className = "warn";
        check.textContent = oops(e, "Couldn't check this folder right now.");
      }
    };
    bCheck.addEventListener("click", runCheck);
    // Rename in place (no prompt(): not every webview has one).
    bRename.addEventListener("click", () => {
      const input = el("input");
      input.value = c.name;
      input.maxLength = 60;
      const save = async () => {
        try { await invoke("rename_client", { id: c.id, name: input.value }); await refresh(); }
        catch (e) { sayErr("cl-status", e); }
      };
      input.addEventListener("keydown", (e) => {
        if (e.key === "Enter") save();
        if (e.key === "Escape") renderClients();
      });
      input.addEventListener("blur", save);
      top.firstChild.replaceWith(input);
      input.focus();
      input.select();
    });
    // Remove takes a second click (no confirm(), same reason).
    let armed = null;
    bRemove.addEventListener("click", async () => {
      if (!armed) {
        bRemove.textContent = "Click again to remove";
        armed = setTimeout(() => { armed = null; bRemove.textContent = "Remove"; }, 3000);
        return;
      }
      clearTimeout(armed);
      try { await invoke("remove_client", { id: c.id }); await refresh(); say("cl-status", "Removed from Leyline. Your game folder was not touched.", "ok"); }
      catch (e) { sayErr("cl-status", e); }
    });
    runCheck();
  }
}

async function addClient(path) {
  if (!path) return;
  say("cl-status", "Looking at the folder…");
  try {
    const r = await invoke("add_client", { path });
    const d = r.detection;
    // the name says the era and version ("Added Wrath 3.3.5a."); how it was found stays out of the sentence
    let msg = `Added ${tidyName(r.profile.name, r.profile.build)}.`;
    if (d.build === "other") msg += " Leyline does not know this build: you can play realms with it, but it offers no mods for it.";
    if (!d.complete) msg += " Some files are missing: " + d.missing.join(", ");
    await refresh();
    say("cl-status", msg, d.complete ? "ok" : "warn");
    $("cl-path").value = "";
    if (window.LeylineAddons) window.LeylineAddons.offer(r.profile);   // a 3.3.5a client: the Leyline addons step (addons.js)
  } catch (e) {
    sayErr("cl-status", e);
  }
}
$("cl-pick").addEventListener("click", async () => {
  try { addClient(await invoke("pick_folder")); } catch (e) { sayErr("cl-status", e); }
});
$("cl-add").addEventListener("click", () => addClient($("cl-path").value.trim()));
$("cl-path").addEventListener("keydown", (e) => { if (e.key === "Enter") addClient($("cl-path").value.trim()); });

// ------------------------------------------------------------------ settings
function renderSettings() {
  const w = state.settings.wine;
  $("wine-on").checked = !!w.enabled;
  $("wine-cmd").value = w.command || "";
  $("wine-prefix").value = w.prefix || "";
  $("wine-note").textContent = state.windows
    ? "You are on Windows: clients start directly and these settings are not used."
    : "Clients are Windows programs. On this system Leyline starts them through Wine.";
  // on Windows clients start directly: no Wine settings to show (nor their Save)
  $("wine-set").hidden = !!state.windows;
  $("wine-set").disabled = !!state.windows;
  if ($("set-save")) $("set-save").hidden = !!state.windows;
  $("set-count").checked = state.settings.count_installs !== false;
  $("set-hybrid").checked = state.settings.hybrid_login === true;
  $("set-cover").checked = state.settings.signin_cover !== false;
}
// Settings › Hide the login behind Leyline's screen (src-tauri/src/cover.rs; the official realm, Single player, Your Den; on unless turned off)
$("set-cover").addEventListener("change", async () => {
  const on = $("set-cover").checked;
  try {
    await invoke("set_signin_cover", { on });
    state.settings.signin_cover = on;
    say("cover-status", on ? "Leyline's screen covers the game while it logs you in." : "You'll see the game's login screen while Leyline logs you in.", "ok");
  } catch (e) { $("set-cover").checked = !on; sayErr("cover-status", e); }
});
$("set-hybrid").addEventListener("change", async () => {
  const on = $("set-hybrid").checked;
  try {
    const msg = await invoke("set_hybrid_login", { on });
    state.settings.hybrid_login = on;
    say("hybrid-status", msg, "ok");
  } catch (e) { $("set-hybrid").checked = !on; sayErr("hybrid-status", e); }
});
$("set-count").addEventListener("change", async () => {
  const on = $("set-count").checked;
  try {
    await invoke("set_count_installs", { on });
    state.settings.count_installs = on;
    say("count-status", on ? "Your installs count toward the rankings." : "Your installs are not counted.", "ok");
  } catch (e) { $("set-count").checked = !on; sayErr("count-status", e); }
});
$("set-form").addEventListener("submit", async (ev) => {
  ev.preventDefault();
  const wine = { enabled: $("wine-on").checked, command: $("wine-cmd").value.trim() || "wine", prefix: $("wine-prefix").value.trim() };
  try { await invoke("set_wine", { wine }); await refresh(); say("set-status", "Saved.", "ok"); }
  catch (e) { sayErr("set-status", e); }
});

// ------------------------------------------------------------------ the client pickers on Mods, Discover, Realms
// They all follow the one selected client (the play bar's).
function renderClientPickers() {
  document.querySelectorAll(".client-pick").forEach((sel) => {
    sel.replaceChildren();
    for (const c of state.settings.clients) {
      const o = el("option", "", pickLabel(c));
      o.value = c.id;
      sel.append(o);
    }
    if (!state.settings.clients.length) sel.append(el("option", "", "No game folder yet"));
    sel.disabled = !state.settings.clients.length;
    if (state.settings.selected_client) sel.value = state.settings.selected_client;
  });
}
function pickedClient() {
  const all = state.settings.clients;
  return all.find((c) => c.id === state.settings.selected_client) || currentClient() || all[0] || null;
}
// one client picked everywhere (a page's picker, Get mods for single player, Add to single player)
function selectClient(id) {
  if (!state.settings.clients.some((c) => c.id === id)) return false;
  state.settings.selected_client = id;
  $("play-client").value = id;
  renderClientPickers();
  realmChanged();
  saveSelection();
  return true;
}
document.querySelectorAll(".client-pick").forEach((sel) => sel.addEventListener("change", async () => {
  selectClient(sel.value);
  const page = sel.dataset.for;
  if (page === "mods") loadInstalled();
  if (page === "discover") { await loadInstalled(true); renderCatalog(); }
}));
function needClient(statusId) {
  const c = pickedClient();
  if (!c) { say(statusId, "Add your game folder first: pick Add a game folder… in the play bar.", "bad"); return null; }
  return c;
}
// The game eras, oldest first (owner 2 Nov: "more accurately named not game 3.3.5a but wrath 3.3.5a for example and
// ordered properly"; clients.rs Build::era). Every client list follows this order: the state's own list is sorted by it
// (tidyState), so the play bar's list, Clients, the pickers and the core's id all agree.
const ERAS = [["1.12.1", "Vanilla"], ["2.4.3", "TBC"], ["3.3.5a", "Wrath"], ["4.3.4", "Cata"], ["5.4.8", "MoP"]];
const eraRank = (b) => { const i = ERAS.findIndex(([v]) => v === b); return i < 0 ? ERAS.length : i; };
const eraName = (b) => { const e = ERAS.find(([v]) => v === b); return e ? `${e[1]} ${b}` : ""; };
// A name an older core made ("Game 3.3.5a (3.3.5a (12340))", "<folder> (3.3.5a (12340))") as the new core names it
// (clients.rs tidy_name: "Wrath 3.3.5a", "<folder> (Wrath)": the version shows beside it). A name the player typed is kept.
const NOTHING_WORDS = ["game", "client", "wotlk", "wrath", "tbc", "vanilla", "classic", "cata", "cataclysm", "mop", "build", "enus", "engb"];
const saysNothing = (n) => String(n).split(/[^\p{L}\p{N}]+/u).every((w) => !w || NOTHING_WORDS.includes(w.toLowerCase()) || /^\d+[a-z]?$/i.test(w));
function tidyName(name, build) {
  const era = eraName(build), b = BUILDS[build];
  if (!era || !b || typeof name !== "string") return name;
  const label = `${build} (${b.num})`;
  let bare;
  if (name === label) bare = "";
  else if (name.endsWith(`(${label})`)) bare = name.slice(0, -label.length - 2).trim();
  else return name;
  return saysNothing(bare) ? era : `${bare} (${era.split(" ")[0]})`;
}
function tidyState(s) {
  if (s && s.settings && Array.isArray(s.settings.clients)) {
    s.settings.clients = s.settings.clients.map((c) => Object.assign({}, c, { name: tidyName(c.name, c.build) }))
      .sort((a, b) => eraRank(a.build) - eraRank(b.build));   // stable: the order added, within an era
  }
  return s;
}
// a client in a picker: its name, and its version when the name doesn't say it
const pickLabel = (c) => (String(c.name).includes(c.build) ? c.name : `${c.name} · ${c.build}`);
// The builds Leyline knows (clients.rs). "other" is any build it does not: it plays realms, but takes no mods.
const BUILDS = {
  "1.12.1": { num: 5875, tag: "1.12", file: "realmlist.wtf" },
  "2.4.3": { num: 8606, tag: "2.4.3", file: "Data\\<locale>\\realmlist.wtf" },
  "3.3.5a": { num: 12340, tag: "3.3.5a", file: "Data\\<locale>\\realmlist.wtf" },
  "4.3.4": { num: 15595, tag: "4.3.4", file: "WTF\\Config.wtf" },
  "5.4.8": { num: 18414, tag: "5.4.8", file: "WTF\\Config.wtf" },
};
// The newest build offered (clients.rs MAX_OFFERED_BUILD). Hidden by the owner on 2026-10-30: 4.3.4 and 5.4.8 are
// not shown for now. Set this back to 18414 (or window.LEYLINE_ALL_CLIENTS = true) to show them again.
const MAX_OFFERED_BUILD = 12340;
const offeredBuild = (b) => !!BUILDS[b] && (window.LEYLINE_ALL_CLIENTS === true || BUILDS[b].num <= MAX_OFFERED_BUILD);
const hiddenBuild = (b) => !!BUILDS[b] && !offeredBuild(b);
const buildLabel = (b) => (offeredBuild(b) ? `${b} · ${BUILDS[b].num}` : "other build");
const buildTag = (b) => (offeredBuild(b) ? BUILDS[b].tag : "other");
const fitsClient = (p, c) => !c || (buildTag(c.build) !== "other" && (!p.builds.length || p.builds.includes("any") || p.builds.includes(buildTag(c.build))));
// The listed realm at this address, if any (its build and server).
const realmAt = (addr) => realmsCache.find((x) => x.address === addr) || null;
const clientFits = (r, c) => !r || !r.build || (c && c.build === r.build);
// a plain line for a realm the picked client can't play (no silent swap): which of your folders to pick, or to add one
function wrongClientText(r, c) {
  const other = state.settings.clients.find((x) => clientFits(r, x));
  return other ? `${r.name} plays with another game folder than ${c.name}: pick ${other.name} in the play bar, then PLAY.`
    : `${r.name} plays with another game folder than ${c.name}. Add yours with Add a game folder… in the play bar.`;
}
const clientFor = (r) => {
  const c = pickedClient();
  if (clientFits(r, c)) return c;
  return state.settings.clients.find((x) => clientFits(r, x)) || null;
};

// ------------------------------------------------------------------ opening listed pages
async function openUrl(url) {
  try { await invoke("open_url", { url }); } catch (e) { sayErr("play-status", e); }
}
function linkBtn(label, url) {
  const b = el("button", "link-s", label);
  b.type = "button";
  b.addEventListener("click", () => openUrl(url));
  return b;
}

// ------------------------------------------------------------------ Discover: the Workshop catalog
let catalog = { categories: [], projects: [] };
let installed = [];          // the picked client's mods
// group: the Mods tab's category (shelf.js GROUPS); cat: one mods-list category (an older link); kind: unused now
const catFilter = { cat: "", group: "", sort: "hot", kind: "" };
try { catFilter.sort = localStorage.getItem("leyline.catSort") || "hot"; } catch (_) { /* storage may be off */ }
// The hub's rankings (rankings.rs: mods.json): the catalog ids in the chosen order, and each one's numbers.
let ranked = null;
let linkedId = "";            // a project a leyline:// link asked for: shown highlighted
async function loadRanks(refresh) {
  try {
    ranked = await invoke("mods_ranked", { sort: catFilter.sort === "name" ? "hot" : catFilter.sort, kind: catFilter.kind, client: null, refresh: !!refresh });
  } catch (_) { ranked = null; }
}
// Mods and Addons are Discover's tabs (shelf.js), each with its own chips (simple.js); themes are Discover › Themes.
const SORTS = ["hot", "rising", "top", "installs", "installs30", "new", "updated", "name"];
if (!SORTS.includes(catFilter.sort)) catFilter.sort = "hot";
function renderSorts() {
  $("cat-sort").value = catFilter.sort;
}
$("cat-sort").addEventListener("change", async () => {
  catFilter.sort = $("cat-sort").value;
  try { localStorage.setItem("leyline.catSort", catFilter.sort); } catch (_) { /* fine */ }
  if (catList) catList.setSort(catFilter.sort === "name" ? "name" : null);
  if (catFilter.sort !== "name") await loadRanks(false);   // Name is the list's own A to Z: the hub's order stays as it was
  renderCatalog();
});
let catFailed = false;   // the Workshop could not be read and there is nothing to show: the status line says so, alone
let catalogIn = false;   // the first mods list has answered (or failed): until then simple.js draws placeholder rows
async function loadCatalog(quiet) {
  if (!quiet) say("cat-status", "Loading mods…");
  if (!catalogIn && window.LeylineSimple) window.LeylineSimple.render();   // the placeholder rows at once, not a bare pane
  // the mods list kept from last time, at once, while the network answers (audit 12 Nov)
  if (!catalogIn && !catalog.projects.length) {
    try {
      const k = await invoke("mods_catalog_cached");
      if (k && Array.isArray(k.projects) && k.projects.length && !catalogIn) {
        catalog = window.LeylineThemes ? window.LeylineThemes.takeThemes(k) : k;
        catalogNames = catalog.projects.map((p) => p.name);
        catalogIn = true;
        renderCatalog();
      }
    } catch (_) { /* an older core: the network list only */ }
  }
  try {
    catalog = await invoke("mods_catalog");
    if (window.LeylineThemes) catalog = window.LeylineThemes.takeThemes(catalog); // themes.js: theme packs go under Discover › Themes
    catalogNames = catalog.projects.map((p) => p.name);
    const when = catalog.saved_at ? new Date(catalog.saved_at * 1000).toLocaleString() : "an earlier visit";
    if (catalog.from_cache) say("cat-status", `Offline: showing the mods as they were ${when}.`, "warn");
    else {
      const sh = window.LeylineShelf ? window.LeylineShelf.of : () => "mods";
      const nm = catalog.projects.filter((p) => sh(p) === "mods").length, na = catalog.projects.filter((p) => sh(p) === "addons").length;
      say("cat-status", catalog.projects.length ? `${nm} mods and ${na} addons.` : "", "ok");
    }
    catFailed = false;
  } catch (e) {
    // one line, no count (owner audit 2026-10-30): the grid stays empty under it
    console.error("[leyline] workshop", e);
    catFailed = !catalog.projects.length;
    say("cat-status", "Can't load the mods. Try again.", "bad");
  }
  catalogIn = true;
  await loadRanks(true);
  renderModTiles();
  document.dispatchEvent(new CustomEvent("leyline:catalog"));  // Home's Workshop spotlight and Mods to try (home.js)
  await loadInstalled(true);
  renderCatalog();
}

// Discover's full list on the list kit (listkit.js, docs/LAUNCHER-LISTS.md): the search (#cat-q), the tab's category chips
// (simple.js, market.js: the page's own filters), the kit's chips (Works with this game folder, Installed, Updates,
// Favourites, the game), the Sort menu (#cat-sort: the hub's order from the core, or Name), the count, the stars,
// keys and the empty / loading words. Mods or Addons is the tab (themes.js); the rows are that tab's entries.
const ERA = { "1.12": "Vanilla", "2.4.3": "TBC", "3.3.5a": "Wrath" };
let catList = null;
let justInstalled = "";   // the key of what was just installed (noteInstalled): its card is lit on Discover and Installed
let catFit = true;   // Works with this game folder (on by default; remembered)
try { catFit = localStorage.getItem("leyline.catFit") !== "0"; } catch (_) { /* fine */ }
const catShelf = () => (window.LeylineSimple ? window.LeylineSimple.shelf() : "mods");
const haveMod = (id) => installed.find((x) => x.key === id) || null;
// the words search looks at: name, author, description, tags, category, and the addon's own words (market.js)
function catWords(p) {
  const S = window.LeylineShelf;
  const cat = catalog.categories.find((k) => k.id === p.category);
  return [p.name, p.author, p.description, (p.tags || []).join(" "), p.category, cat ? cat.name : "",
    S && S.group(p) ? S.groupName(S.group(p)) : "", window.LeylineMarket ? window.LeylineMarket.words(p) : ""];
}
function catPageFilters() {
  const tab = catShelf();
  const mk = window.LeylineMarket ? window.LeylineMarket.state : { sub: "", works: false };
  return (catFilter.cat ? 1 : 0) + (tab === "mods" && catFilter.group ? 1 : 0) + (tab === "addons" ? (mk.sub ? 1 : 0) + (mk.works ? 1 : 0) : 0);
}
function catMatch(p) {
  const S = window.LeylineShelf;
  const tab = catShelf();
  if (catFilter.cat && p.category !== catFilter.cat) return false;
  if (tab === "mods" && catFilter.group && S && S.group(p) !== catFilter.group) return false;
  if (tab === "addons" && window.LeylineMarket && !window.LeylineMarket.keep(p)) return false;   // the addon chips (market.js)
  return true;
}
function catReset() {
  catFilter.cat = "";
  catFilter.group = "";
  if (window.LeylineMarket) {
    window.LeylineMarket.state.sub = "";
    window.LeylineMarket.state.works = false;
    try { localStorage.setItem("leyline.mkSub", ""); localStorage.setItem("leyline.mkWorks", "0"); } catch (_) { /* fine */ }
  }
  if (window.LeylineSimple) window.LeylineSimple.render();   // the category chips, off
}
// Mods › Game modes: the Leyline Modes above the list's own game modes (gamemods.js), when nothing is searched
function catGameBox(q) {
  const gm = $("game-discover");
  const on = !!(gm && window.LeylineGameMods && catShelf() === "mods" && catFilter.group === "game" && window.LeylineSimple && window.LeylineSimple.view() === "all" && !q);
  if (gm) { const was = !gm.hidden; gm.hidden = !on; if (on && !was) window.LeylineGameMods.show(); }
  return on;
}
// One mark on every card (Discover's full list, its Simple rows, Game modes): Installed, or an update ready, and the
// card of what was just installed lit (owner's UX finding: after an install you stay where you were, on its card).
function decorateCard(card, p) {
  if (!card || !p) return card;
  const m = haveMod(p.id);
  const thumb = card.querySelector(".dc-thumb");
  if (m && thumb && !thumb.querySelector(".dc-have")) {
    const t = el("span", "dc-have" + (m.update ? " upd" : ""), m.update ? "Update ready" : "Installed");
    t.title = m.update ? `An update is ready: ${m.update}` : "In your game folder";
    thumb.append(t);
  }
  card.classList.toggle("is-installed", !!m);
  card.classList.toggle("just", !!justInstalled && p.id === justInstalled);
  spCardButton(card, p);
  return card;
}

// ------------------------------------------------------------------ mods for single player (spmods.js, the Single player page)
// The single player worlds play with one game folder (vanilla.js LeylineWorlds.folder(): the one world_find_folder
// picked). "Get mods for single player" opens Discover on that folder with Works with this game folder on; a card for
// another folder offers "Add to single player", which picks that folder and runs the card's own Install (the same
// checks: pending items have no Install, so they never get this button either). While picking for single player, a bar
// on Discover says so, with Back to Single player.
const spFolder = () => (window.LeylineWorlds && window.LeylineWorlds.folder ? window.LeylineWorlds.folder() : null);
let spPicking = false;   // Discover was opened for single player (or a card added to it): the bar shows
let spAdded = "";        // the name of what went in while picking
const spCanInstall = (p) => p && p.kind !== "realm" && ((p.source === "packaged" && !!p.download) || (p.source === "listed" && !!p.install) || p.source === "upstream");
function spCardButton(card, p) {
  const sp = spFolder(), c = pickedClient();
  const acts = card.querySelector(".acts");
  const old = card.querySelector(".dc-sp-add");
  if (old) old.remove();
  if (!acts || !sp || !c || sp.id === c.id || !spCanInstall(p) || !fitsClient(p, sp)) return;
  const have = !!(window.LeylineSPMods && window.LeylineSPMods.has(p.id));
  const b = el("button", "btn small dc-sp-add", have ? "In single player ✓" : "Add to single player");
  b.type = "button";
  b.disabled = have;
  b.title = have ? `Already in ${sp.name}, the game folder single player plays with` : `Put it in ${sp.name}, the game folder single player plays with`;
  b.addEventListener("click", (ev) => { ev.stopPropagation(); addToSingle(p, b); });
  acts.append(b);
}
async function addToSingle(p, btn) {
  const sp = spFolder();
  if (!sp) return;
  selectClient(sp.id);
  spPicking = true;
  await loadInstalled(true);
  if (haveMod(p.id)) { renderCatalog(); say("cat-status", `${p.name} is already in single player's game folder.`, "ok"); return; }
  if (p.source === "upstream" && window.LeylineMarket) await window.LeylineMarket.install(p, btn);
  else await installProject(p, btn);
}
function modsForSingle() {
  const sp = spFolder();
  if (sp) selectClient(sp.id);
  spPicking = true;
  spAdded = "";
  show("mods", false, "discover");
  const list = catListMake();
  if (list && sp) list.setFilter("fit", true);   // Works with this game folder
  loadInstalled(true).then(renderCatalog);
}
function paintSpBar() {
  const bar = $("dc-sp");
  if (!bar) return;
  const sp = spFolder(), c = pickedClient();
  const on = spPicking && !!sp && !!c && sp.id === c.id;
  bar.hidden = !on;
  if (!on) return;
  $("dc-sp-text").textContent = spAdded ? `Added ${spAdded} to single player. It's on the next time you play.` : `Picking mods for single player: they go into ${sp.name}.`;
  $("dc-sp-back").classList.toggle("primary", !!spAdded);
}
if ($("dc-sp-back")) $("dc-sp-back").addEventListener("click", () => { spPicking = false; spAdded = ""; paintSpBar(); show("single"); });
document.addEventListener("leyline:page", (e) => { const pg = e.detail && e.detail.page; if (pg && pg !== "mods") { spPicking = false; spAdded = ""; paintSpBar(); } });
// the single player folder's mods were read again (spmods.js): the cards' Add to single player catch up
document.addEventListener("leyline:spmods", () => { if (currentPage === "mods" && catList) catList.refresh(); });
// The kit's no-match words, with this list's own word for its rows ("No addons match “x”."): its noun is a function (the
// count says mods or addons by the tab), so its own words would say "items".
function noMatch(info, api, many) {
  const acts = [];
  if (info.suggest) acts.push({ label: `Show ${info.suggest.n} in ${info.suggest.long}`, run: () => api.setTab(info.suggest.tab, true) });
  if (info.query && info.filtersOn) acts.push({ label: "Reset", run: () => api.reset() });
  else if (info.query) acts.push({ label: "Clear search", run: () => api.setQuery("", true) });
  else if (info.filtersOn) acts.push({ label: "Clear filters", run: () => api.resetFilters() });
  const text = info.query ? `No ${many} match “${info.query}”.` : `No ${many} match these filters.`;
  return { text: text + (info.query && !info.filtersOn ? " Try fewer words." : ""), actions: acts };
}
function catListMake() {
  if (catList || !window.LeylineList || !$("cat-grid")) return catList;
  const word = () => (catShelf() === "addons" ? ["addon", "addons"] : ["mod", "mods"]);
  catList = window.LeylineList.create($("cat-grid"), {
    id: "mods-discover",
    key: (p) => p.id,
    label: "Mods",
    noun: (n, total) => { const [one, many] = word(); return n === total ? `${n} ${n === 1 ? one : many}` : `${n} of ${total} ${total === 1 ? one : many}`; },
    render: (p, ctx) => {
      const card = projectCard(p, pickedClient());
      const thumb = card.querySelector(".dc-thumb");
      const star = ctx.star();
      star.classList.add("dc-star");
      if (thumb) thumb.append(star); else card.prepend(star);
      if (ctx.query) for (const n of card.querySelectorAll(".dc-body .name, .dc-body .desc, .mk-credit")) window.LeylineList.mark(n, ctx.query);
      return card;
    },
    search: { input: $("cat-q"), fields: catWords },
    filters: [
      { id: "fit", label: "Works with this game folder", test: (p) => { const c = pickedClient(); return !c || fitsClient(p, c); } },
      { id: "have", label: "Installed", test: (p) => !!haveMod(p.id) },
      { id: "upd", label: "Updates", test: (p) => { const m = haveMod(p.id); return !!(m && m.update); } },
      { id: "fav", label: "Favourites", test: (p) => !!(catList && catList.isFav(p)) },
      // the game each one is for: only when the list has more than one (a lone "All" chip is hidden: modlists.css)
      { id: "era", label: "Game", all: "Any game", options: (rows) => {
        const tags = new Set();
        for (const p of rows) for (const b of p.builds || []) if (b !== "any") tags.add(b);
        return tags.size < 2 ? [] : [...tags].sort().map((b) => ({ value: b, label: ERA[b] || b }));
      }, test: (p, v) => !(p.builds || []).length || p.builds.includes("any") || p.builds.includes(v) },
    ],
    filtersLabel: "Show",
    match: catMatch,
    filtersOn: catPageFilters,
    onReset: catReset,
    sort: { options: [{ id: "name", label: "Name", compare: (a, b) => String(a.name).localeCompare(String(b.name)) }], menu: false, storageKey: false },
    favourites: { id: "mods", label: "Favourites" },
    primary: (p, node) => {
      const b = node.querySelector(".acts .btn.primary:not([disabled])");
      if (b) { b.click(); return; }
      const n = node.querySelector(".dc-body .name");
      if (n && n.tagName === "BUTTON") n.click();
    },
    empty: (info, api) => {
      const tab = catShelf();
      if (info.reason === "loading") return tab === "addons" ? "Loading the addons…" : "Loading the mods…";
      if (catFailed) return false;   // "Can't load the mods. Try again." is already the one line
      if (catGameBox(info.query)) return false;   // the Leyline Modes above are the Game modes
      if (info.reason === "none") return tab === "addons" ? "The addons list is not here yet." : "The mods list is not here yet.";
      const S = window.LeylineShelf;
      const gname = tab === "mods" && catFilter.group && S ? S.groupName(catFilter.group) : "";
      if (gname && !info.query && !catalog.projects.some((p) => S.group(p) === catFilter.group)) {
        return { text: `No mods in ${gname} yet. Make the first one: see Make a mod.`, actions: [{ label: "All mods", run: () => { catFilter.group = ""; renderCatalog(); } }] };
      }
      return noMatch(info, api, tab);   // the kit's words: Clear search, Clear filters, Reset
    },
    classes: { body: "mod-grid dc-grid", empty: "dc-empty" },
    bar: $("dc-kbar"),
    onChange: (view) => {
      catGameBox(view.query);
      const f = !!(catList ? catList.filters().fit : catFit);
      if (f !== catFit) { catFit = f; try { localStorage.setItem("leyline.catFit", f ? "1" : "0"); } catch (_) { /* fine */ } }
      if ($("pane-discover")) $("pane-discover").classList.toggle("fit-off", !f);
    },
  });
  if (catFit) catList.setFilter("fit", true);
  if (catFilter.sort === "name") catList.setSort("name");
  // the Sort menu sits in the kit's bar, before the count
  const sortBox = $("cat-sort") ? $("cat-sort").closest("label") : null;
  const count = $("dc-kbar") ? $("dc-kbar").querySelector(".lk-count") : null;
  if (sortBox && count) count.before(sortBox);
  return catList;
}

function renderCatalog() {
  paintSpBar();
  if (window.LeylineSimple) window.LeylineSimple.render();   // the Simple view's rows, or the full view's chips (simple.js)
  renderSorts();
  const list = catListMake();
  if (!list) return;
  if (list.query() !== $("cat-q").value) list.setQuery($("cat-q").value);
  // Discover's tab: Mods or Addons, never both on one tab (shelf.js: one rule, docs/WORDS.md)
  const S = window.LeylineShelf;
  const tab = catShelf();
  let ordered = catalog.projects;
  if (ranked && ranked.ids) {
    const byId = new Map(catalog.projects.map((p) => [p.id, p]));
    ordered = ranked.ids.map((id) => byId.get(id)).filter(Boolean);
  }
  list.setLoading(!catalogIn);
  list.update(ordered.filter((p) => !S || S.of(p) === tab));
  // what was just installed: its card, lit and in reach (the player stays on this tab)
  if (justInstalled && catJustFocus && currentPage === "mods" && currentTab === "discover") {
    catJustFocus = false;
    if (list.visible().some((p) => p.id === justInstalled)) list.focusRow(justInstalled);
  }
}
let catJustFocus = false;

// Rating stars on a card's picture (shown on hover and focus). Rating needs the Leyline sign-in on Home (the
// same website sign-in).
function rateStars(p) {
  const stars = el("span", "stars");
  stars.setAttribute("role", "group");
  stars.setAttribute("aria-label", `Rate ${p.name}`);
  for (let n = 1; n <= 5; n++) {
    const b = el("button", "", "★");
    b.type = "button";
    b.title = `${n} star${n > 1 ? "s" : ""}`;
    b.setAttribute("aria-label", `Rate ${n} star${n > 1 ? "s" : ""}`);
    b.addEventListener("click", (ev) => { ev.stopPropagation(); rateProject(p, n, stars); });
    stars.append(b);
  }
  return stars;
}
async function rateProject(p, n, box) {
  try {
    const r = await invoke("mod_rate", { project: p.id, stars: n });
    box.querySelectorAll("button").forEach((b, i) => b.classList.toggle("on", i < n));
    if (ranked && ranked.metrics && ranked.metrics[p.id] && r && r.rating) ranked.metrics[p.id].rating = r.rating;
    say("cat-status", `Thanks: you rated ${p.name} ${n} star${n > 1 ? "s" : ""}.`, "ok");
  } catch (e) {
    say("cat-status", String(e) === "signed_out" ? "Log in (in the title bar) to rate mods: the same account as the website." : oops(e, "Couldn't save your rating. Try again."), "bad");
  }
}

// ------------------------------------------------------------------ Discover cards: a picture, a name, one line
// The picture: the catalog's own (thumb, preview, screenshots[0], images[0], when a catalog has them), else the
// Workshop's first screenshot of a packaged mod (/wrath/mods/<id>/shot/0), else the hub's copy of a GitHub repo's
// preview (/wrath/art/hub-thumbs/<owner>__<repo>.webp). Only from the site itself (the window's CSP allows no other
// host). Lazy; while it loads, and when there is none (a 404), the tile drawn from the name shows.
const SITE = ADDR.SITE;
function siteImage(u) {
  if (typeof u !== "string" || !u) return "";
  if (/^\/wrath\/[A-Za-z0-9._~\/-]+$/.test(u) && !u.includes("..")) return SITE + u;
  // on either of the site's names (a catalog fetched on the old one lists its pictures there)
  if ((u.startsWith(SITE + "/") || u.startsWith(ADDR.OLD_SITE + "/")) && /^[A-Za-z0-9._~\/:-]+$/.test(u) && !u.includes("..")) return u;
  return "";
}
function thumbUrl(p) {
  const first = (v) => (Array.isArray(v) ? v[0] : v);
  for (const v of [p.thumb, p.thumbnail, p.preview, first(p.screenshots), first(p.images), p.icon]) {
    const u = siteImage(typeof v === "object" && v ? v.url : v);
    if (u) return u;
  }
  const mid = p.id.startsWith("mod:") ? p.id.slice(4) : "";
  if (p.source === "packaged" && /^[a-z0-9-]+$/.test(mid)) return `${SITE}/wrath/mods/${mid}/shot/0`;
  const gh = /^https:\/\/github\.com\/([A-Za-z0-9][A-Za-z0-9._-]{0,99})\/([A-Za-z0-9][A-Za-z0-9._-]{0,99}?)(?:\.git)?\/?$/.exec(p.repo || "");
  if (gh) return `${SITE}/wrath/art/hub-thumbs/${gh[1].toLowerCase()}__${gh[2].toLowerCase()}.webp`;
  return "";
}
// A small glyph per category (24x24, stroked), for the drawn tile.
const CAT_GLYPHS = {
  addons: "M9 4h4v2.5a1.5 1.5 0 0 0 3 0V4h4v6h-2.5a1.5 1.5 0 0 0 0 3H20v7H4v-7h2.5a1.5 1.5 0 0 0 0-3H4V4z",
  graphics: "M12 3l2.2 5.8L20 11l-5.8 2.2L12 19l-2.2-5.8L4 11l5.8-2.2z",
  sandbox: "M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5",
  maps: "M3 6l6-2 6 2 6-2v14l-6 2-6-2-6 2zM9 4v14M15 6v14",
  builds: "M4 20h16M6 20V10l6-5 6 5v10M10 20v-5h4v5",
  encounters: "M5 19L19 5M14 5h5v5M5 5l5 5M14 14l5 5M19 14v5h-5",
  models: "M12 3l8 4.5v9L12 21l-8-4.5v-9zM4 7.5l8 4.5 8-4.5M12 12v9",
  "realm-scripts": "M8 7l-5 5 5 5M16 7l5 5-5 5M14 4l-4 16",
  presets: "M5 6h14M5 12h14M5 18h14M9 4v4M15 10v4M8 16v4",
  audio: "M9 18V6l11-2v12M9 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0zM20 16a3 3 0 1 1-6 0 3 3 0 0 1 6 0z",
  tools: "M14.5 5.5a4 4 0 0 0-5 5L4 16v4h4l5.5-5.5a4 4 0 0 0 5-5L16 12l-4-4z",
  conversions: "M12 3l9 5-9 5-9-5zM3 13l9 5 9-5M3 17.5l9 5 9-5",
};
function genTile(p) {
  const t = el("span", "dc-gen");
  t.setAttribute("aria-hidden", "true");
  const h = nameHue(p.name || p.id);
  t.style.setProperty("--h1", String(h));
  t.style.setProperty("--h2", String((h + 48) % 360));
  t.append(el("b", "", ([...String(p.name || "?").trim()][0] || "?").toUpperCase()));
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  const path = document.createElementNS(ns, "path");
  path.setAttribute("d", CAT_GLYPHS[p.category] || CAT_GLYPHS.addons);
  svg.append(path);
  t.append(svg);
  return t;
}
function cardThumb(p) {
  const box = el("div", "dc-thumb");
  box.append(genTile(p));
  const u = thumbUrl(p);
  if (u) {
    const img = el("img");
    img.alt = "";
    img.loading = "lazy";
    img.decoding = "async";
    img.referrerPolicy = "no-referrer";
    img.width = 480;
    img.height = 270;
    img.addEventListener("load", () => { if (img.naturalWidth > 1) box.classList.add("has-img"); else img.remove(); });
    img.addEventListener("error", () => { img.remove(); box.classList.remove("has-img"); });
    img.src = u;
    box.append(img);
  }
  return box;
}
// The first sentence of a description (the card clamps it to one line; the whole text is its tooltip).
function firstLine(text) {
  const t = String(text || "").replace(/\s+/g, " ").trim();
  const m = /^(.+?[.!?])(\s|$)/.exec(t);
  return m ? m[1] : t;
}
const shortNum = (n) => (n >= 10000 ? Math.round(n / 1000) + "k" : n >= 1000 ? (n / 1000).toFixed(1).replace(/\.0$/, "") + "k" : String(n));

function projectCard(p, c) { return decorateCard(projectCardOf(p, c), p); }
function projectCardOf(p, c) {
  if (p.source === "upstream" && window.LeylineMarket) return window.LeylineMarket.card(p, c);   // from the author (market.js)
  const card = el("article", "mod-card dc-card" + (p.id === linkedId ? " linked" : ""));
  card.dataset.id = p.id;
  if (p.kind) card.dataset.kind = p.kind;   // the phone app's "works on your phone" line (phone.js)
  if (p.category) card.dataset.category = p.category;
  const fits = fitsClient(p, c);
  const thumb = cardThumb(p);
  const cat = catalog.categories.find((k) => k.id === p.category);
  if (c && !fits) thumb.append(el("span", "dc-tag warn", `Not for ${c.build}`));
  else if (cat) thumb.append(el("span", "dc-tag", cat.name));
  thumb.append(rateStars(p));
  const body = el("div", "dc-body");
  const page = p.page || p.repo || "";
  const name = el(page ? "button" : "b", "name", p.name);
  if (page) {
    name.type = "button";
    name.title = `Open ${p.name}'s page`;
    name.addEventListener("click", () => openUrl(page));
    card.classList.add("can-open");
  }
  const line = firstLine(p.description);
  const desc = el("p", "desc", line);
  if (p.description && p.description.trim() !== line) desc.title = p.description;
  body.append(name, desc);
  const foot = el("div", "dc-foot");
  const meta = el("div", "dc-meta");
  if (p.version) meta.append(el("span", "ver-s", "v" + p.version));
  const m = ranked && ranked.metrics ? ranked.metrics[p.id] : null;
  if (m) {
    const r = m.rating || {};
    if (r.n) { const s = el("span", "rating", `★ ${Number(r.avg).toFixed(1)}`); s.title = `${r.n} rating${r.n === 1 ? "" : "s"}`; meta.append(s); }
    const all = (m.installs && m.installs.all) || 0;
    const ins = el("span", "installs", shortNum(all));
    ins.title = `${all} install${all === 1 ? "" : "s"}`;
    meta.append(ins);
    if (m.rising) { const r = el("span", "rising", "▲"); r.title = "Rising"; r.setAttribute("aria-label", "Rising"); meta.append(r); }
  } else if (p.source !== "packaged") meta.append(el("span", "src", "Community"));
  // a player's listed mod (listmod.js, listed.rs): installs from its author; says whether a moderator looked
  if (p.source === "listed" && window.LeylineListMod) window.LeylineListMod.decorate(p, thumb, meta);
  const acts = el("div", "acts");
  const mine = installed.find((x) => x.key === p.id);
  if ((p.source === "packaged" && p.download) || (p.source === "listed" && p.install)) {
    const b = el("button", "btn small");
    b.type = "button";
    if (!c) { b.textContent = "Install"; b.disabled = true; b.title = "Add a game client first"; }
    else if (!fits) { b.textContent = "Install"; b.disabled = true; b.title = `Made for ${p.builds.join(" / ")} clients`; }
    else if (mine && mine.update) { b.textContent = "Update"; b.title = `Update to ${mine.update}`; b.classList.add("primary"); }
    else if (mine) { b.textContent = "Installed ✓"; b.disabled = true; b.classList.add("done"); }
    else { b.textContent = "Install"; b.classList.add("primary"); }
    b.addEventListener("click", (ev) => { ev.stopPropagation(); installProject(p, b); });
    if (p.kind === "realm") b.title = "A realm mod: it runs on a realm that installs it, not in your game";
    if (p.source === "listed" && p.plan && p.plan.length && !b.title) b.title = p.plan.join(" ");
    acts.append(b);
    if (p.source === "listed" && window.LeylineListMod) acts.append(window.LeylineListMod.reportButton(p));
  } else if (page) {
    const b = el("button", "btn small", "Get ↗");
    b.type = "button";
    b.title = "Get it from its page, then drop the zip on Mods & Addons › Installed.";
    b.addEventListener("click", (ev) => { ev.stopPropagation(); openUrl(page); });
    acts.append(b);
  }
  foot.append(meta, acts);
  body.append(foot);
  card.append(thumb, body);
  // the whole card opens the page (the name is the keyboard way in; buttons and stars keep their own clicks)
  if (page) card.addEventListener("click", (ev) => { if (!ev.target.closest("button, a, select, input")) openUrl(page); });
  return card;
}

// ------------------------------------------------------------------ native code: a one-time OK per mod
// The core refuses a mod with .dll/.asi/.exe files until the player says yes once; its error starts with
// NATIVE_CONFIRM: and carries {key, name, files}. The OK is kept in the launcher's settings.
const NATIVE = "NATIVE_CONFIRM:";
function askNative(info) {
  return new Promise((resolve) => {
    const d = $("native-dlg");
    $("native-name").textContent = info.name || "This mod";
    $("native-files").replaceChildren(...(info.files || []).slice(0, 30).map((f) => el("li", "", f)));
    const done = (yes) => { if (d.open) d.close(); resolve(yes); };
    $("native-yes").onclick = () => done(true);
    $("native-no").onclick = () => done(false);
    d.oncancel = (ev) => { ev.preventDefault(); done(false); };
    if (d.showModal) d.showModal(); else resolve(window.confirm(`${info.name} has native code. Install it?`));
  });
}
async function withNativeOk(run) {
  try {
    return await run();
  } catch (e) {
    const t = String(e);
    if (!t.startsWith(NATIVE)) throw e;
    let info;
    try { info = JSON.parse(t.slice(NATIVE.length)); } catch (_) { throw "Leyline could not read this mod's native code notice."; }
    if (!(await askNative(info))) throw `Not installed: you said no to ${info.name || "its"} native code.`;
    await invoke("mod_native_confirm", { key: info.key });
    return await run();
  }
}

// What an install says afterwards: its notes, and the player's changed files an update kept.
function installedText(verb, m, c) {
  let t = `${verb} ${m.name}${m.version ? " v" + m.version : ""} in ${c.name}.`;
  if (m.kept && m.kept.length) t += " Kept your changes: " + m.kept.join("; ") + ".";
  if (m.warnings && m.warnings.length) t += " Note: " + m.warnings.join("; ");
  return t;
}

async function installProject(p, btn) {
  const c = needClient("cat-status");
  if (!c) return;
  btn.disabled = true;
  btn.textContent = "Installing…";
  const was = installed.find((m) => m.key === p.id);
  say("cat-status", `Downloading ${p.name}…`);
  try {
    const m = await withNativeOk(() => invoke("mod_install", { client: c.id, project: p.id }));
    say("cat-status", installedText(was ? "Updated" : "Installed", m, c), m.warnings.length ? "warn" : "ok");
    noteInstalled(m.key || p.id, window.LeylineShelf ? window.LeylineShelf.of(p) : "");
    if (spPicking) { const sp = spFolder(); if (sp && sp.id === c.id) spAdded = p.name; }
    catJustFocus = true;   // Discover stays on this tab with its card lit (renderCatalog)
    if (!was && window.LeylineGuide) window.LeylineGuide.moment("mod");   // the first mod: one line, once (guide.js)
  } catch (e) {
    sayErr("cat-status", e);
  }
  await loadInstalled(true);
  renderCatalog();
}

// The mods list's first load, while it runs: only an install link waits for it (handleDeepLink).
let catalogLoading = null;

// A hub page's "Install with Leyline" (leyline://install/<id>): show that project with its INSTALL button. Nothing
// installs until the player clicks INSTALL. Also a realm, a friend's invite, a Den, a Den code or the game's Host
// window. Taken at start, and again whenever a link reaches the open launcher (the core's `deep-link` event, app.rs
// deep_link_arrived: Leyline is never opened twice).
async function handleDeepLink() {
  let link = null;
  try { link = await invoke("deep_link_take"); } catch (_) { return; }
  if (link && link.action === "realm" && typeof link.id === "string") return showLinkedRealm(link.id);
  if (link && link.action === "host" && typeof link.id === "string") return showHostLink(link.id);
  // a friend's invite (leyline://join?link=…, deeplink.rs): one Join button, nothing joins before it (solo.js)
  if (link && link.action === "join" && typeof link.id === "string") return window.LeylineSolo && window.LeylineSolo.offerJoin(link.id);
  // a shared Den's "Open in Leyline" on the website (leyline://den/<id>): its detail in Mods & Addons › Dens (denmarket.js)
  // (it may come before denmarket.js has loaded: then it takes the id when it does)
  if (link && link.action === "den" && typeof link.id === "string") {
    if (window.LeylineDenMarket) return window.LeylineDenMarket.focus(link.id);
    window.LeylinePendingDen = link.id;
    return;
  }
  // a Den code's link (leyline://pack/<code>, deeplink.rs): Your Den's page with the code in its box; nothing opens until
  // the player presses Open the code (den.js)
  if (link && link.action === "pack" && typeof link.id === "string") {
    if (window.LeylineNav) window.LeylineNav.show("den"); else show("den", true);
    const box = document.getElementById("yd-code");
    if (box) { box.value = link.id; box.focus(); }
    else window.LeylinePendingCode = link.id;
    return;
  }
  if (!link || link.action !== "install" || typeof link.id !== "string") return;
  if (catalogLoading) { try { await catalogLoading; } catch (_) { /* the list as it is */ } }   // only installs wait for the mods list
  if (link.id.startsWith("look:")) {   // the website's Looks page: Discover › Looks, that card (looks.js); nothing installs yet
    show("mods", true, "discover");
    if (!(window.LeylineLooks && window.LeylineLooks.focus(link.id))) say("cat-status", "That look isn't in the list here yet. Try again in a few minutes.", "warn");
    return;
  }
  const p = catalog.projects.find((x) => x.id === link.id);
  show("mods", true, "discover");   // the catalog was just loaded
  if (!p) { say("cat-status", "That mod isn't in the mods list here yet. Try again in a few minutes.", "warn"); return; }
  linkedId = p.id;
  catFilter.cat = "";
  catFilter.group = "";
  catFilter.kind = "";
  if (window.LeylineMarket) { window.LeylineMarket.state.sub = ""; window.LeylineMarket.state.works = false; }
  if (window.LeylineThemes && window.LeylineShelf && window.LeylineShelf.of(p) !== "themes") window.LeylineThemes.showTab(window.LeylineShelf.of(p));   // its tab
  $("cat-q").value = "";
  const c = pickedClient();
  const list = catListMake();
  if (list) {
    list.setQuery("");
    for (const f of ["have", "upd", "fav", "era"]) list.setFilter(f, null);   // nothing of the player's may hide it
    if (c && !fitsClient(p, c)) list.setFilter("fit", null);
  }
  await loadRanks(false);
  if (window.LeylineSimple && window.LeylineSimple.view() !== "all") window.LeylineSimple.setView("all", true);   // the card is in the full catalog
  renderCatalog();
  const card = document.querySelector(`#cat-grid .mod-card[data-id="${CSS.escape(p.id)}"]`);
  if (card) { card.scrollIntoView({ block: "center" }); const b = card.querySelector(".acts .btn"); if (b && !b.disabled) b.focus(); }
  say("cat-status", c ? `From the hub: ${p.name}. Press Install to put it in ${c.name}.` : `From the hub: ${p.name}. Add your game folder first, then press Install.`, "ok");
}

// The in-game Host window (leyline://host?<choices>, checked by deeplink.rs): the Host page with the host's choices and
// "MAKE IT ON THE WEBSITE" (the website's form filled in; the realm key is still made on this PC). From the game's
// "Your realm" tab (leyline://host?manage=<what>): the Host page with what to do for that control. Nothing starts.
const HOST_WORDS = {
  core: { azerothcore: "AzerothCore", cmangos: "CMaNGOS" },
  preset: { den: "My Den", dalaran: "Dalaran hangout", main: "Classic levelling", brawl: "Brawl", progression: "Progression", vanillaplus: "Vanilla+",
    "cmangos-1.12.1": "CMaNGOS Classic", "cmangos-2.4.3": "CMaNGOS TBC" },
  type: { normal: "Normal", pvp: "PvP", rp: "RP", rppvp: "RP-PvP" },
  start: { level1: "Level 1 only", boost: "Level 1, or the boost to 80" },
  vis: { public: "Public: on the realm list", private: "Private: invite link only" },
  signup: { open: "Open: anyone", invite: "Invite link only", off: "Closed" },
  herokey: { on: "On", off: "Off" },
};
const HOST_LABELS = [["core", "Core"], ["preset", "Ruleset"], ["name", "Realm name"], ["type", "Type"], ["xp", "XP rate"],
  ["loot", "Loot rate"], ["rep", "Reputation rate"], ["start", "New characters"], ["bots", "Bots"], ["vis", "Who can find it"],
  ["signup", "Accounts"], ["herokey", "Hero Key sign-in"], ["region", "Region"], ["lang", "Language"], ["tags", "Tags"],
  ["max", "Max players shown"]];
const HOST_MANAGE = {
  bots: ["Bots", "In your Host pack folder: leyline options --bots 40 (0 turns them off), then restart the realm (leyline stop, then leyline host)."],
  rates: ["Rates", "In your Host pack folder: leyline options --xp 3 --loot 1 --rep 2 (1 to 10), then restart the realm (leyline stop, then leyline host)."],
  signup: ["Sign-up", "In your Host pack folder: leyline signup open (anyone), leyline signup invite (invite link only) or leyline signup off."],
  list: ["The realm list", "In your Host pack folder: leyline public (on the list) or leyline private (invite link only). The list changes within a minute."],
  invite: ["Invite link", "Your realm's invite link is below, under Your realm: Copy invite link. New invite link stops the old one."],
};

function hostChoices(id) {
  const p = new URLSearchParams(id);
  // a Den has no bots and no player limit to choose (the kit sets both)
  const den = p.get("preset") === "den";
  return HOST_LABELS.filter(([k]) => p.has(k) && !(den && (k === "bots" || k === "max"))).map(([k, label]) => {
    const v = p.get(k);
    const words = HOST_WORDS[k] && HOST_WORDS[k][v];
    return [label, words || (["xp", "loot", "rep"].includes(k) ? v + "x" : k === "tags" ? v.split(",").join(", ") : v)];
  });
}

function showHostLink(id) {
  show("host");
  const box = $("host-from-game");
  if (!box) return;
  box.hidden = false;
  const list = $("host-from-game-list");
  const go = $("host-from-game-go");
  list.replaceChildren();
  if (id.startsWith("manage=")) {
    const m = HOST_MANAGE[id.slice(7)];
    if (!m) return;
    $("host-from-game-title").textContent = `FROM THE GAME: ${m[0].toUpperCase()}`;
    list.append(el("li", "", m[1]));
    go.hidden = true;
    say("host-status-line", `From the game: ${m[0]}.`, "ok");
    return;
  }
  $("host-from-game-title").textContent = "FROM THE GAME: YOUR NEW REALM";
  for (const [label, v] of hostChoices(id)) {
    const li = el("li", "");
    li.append(el("b", "", label + ": "), document.createTextNode(v));
    list.append(li);
  }
  go.hidden = false;
  go.onclick = async () => {
    try { await invoke("open_host_form", { query: id }); say("host-status-line", "The website's host page is open with your choices: log in, add your GM account name, press Start server, then come back here for steps 2 and 3.", "ok"); }
    catch (e) { sayErr("host-status-line", e); }
  };
  say("host-status-line", "Your choices from the game are below. Make the realm on the website, then start it here.", "ok");
}

// A website's "Open in launcher" (leyline://realm/<id>): show that realm on the Realms page with PLAY focused.
// Nothing starts until the player clicks PLAY.
async function showLinkedRealm(id) {
  show("realms", true);
  if (!realmsCache.length) { try { await loadRealms(false); } catch (_) {} }
  const r = realmsCache.find((x) => (id === "official" ? x.official : x.realm_id === id));
  if (!r) { say("rl-status", "That realm isn't on the realm list right now: it may be offline. Try Refresh in a minute.", "warn"); return; }
  if (r.official || r.address) pickRealm(r);   // the play bar too, with a client of the version it needs
  $("rl-mine").checked = false;
  if (rlList && rlList.query()) rlList.setQuery("");   // a search for something else would hide it
  setRealmsTab(realmTab(r));   // a community realm shows on the Community tab (renders the list)
  rlLinked = realmKeyOf(r);   // lit until the player leaves the page (the list repaints: it stays lit)
  renderRealms();
  const card = [...document.querySelectorAll("#rl-list .realm-card")].find((c) => c.querySelector(".name")?.textContent === r.name);
  if (card) { card.classList.add("linked"); card.scrollIntoView({ block: "center" }); const b = card.querySelector(".acts .btn"); if (b && !b.disabled) b.focus(); }
  const need = r.official ? "3.3.5a" : r.build;
  if (need && BUILDS[need] && !state.settings.clients.some((x) => x.build === need)) say("rl-status", `${r.name} needs the game files for this realm. Add that game folder first: Add a game folder… in the play bar.`, "warn", need);
  else say("rl-status", `From the website: ${r.name}. Press PLAY to join.`, "ok");
}

// #cat-q is the list kit's search box (catListMake): it repaints the list itself

// The three mod tiles (owner 2026-10-30: taken off Home; kept for a page that brings the box back).
function renderModTiles() {
  const box = $("mod-tiles");
  if (!box) return;
  box.replaceChildren();
  const mine = installed.slice().sort((x, y) => (y.update ? 1 : 0) - (x.update ? 1 : 0)).slice(0, 3);
  const picks = catalog.projects.filter((p) => p.source === "packaged").concat(catalog.projects.filter((p) => p.source !== "packaged"));
  for (let i = 0; i < 3; i++) {
    const m = mine[i];
    const p = m ? null : picks[i - mine.length];
    const t = el("button", "tile" + (m && m.update ? " has-update" : ""));
    t.type = "button";
    const txt = el("span", "tile-txt");
    let name = "Mods", line = "Loading…", cls = "";
    if (m) {
      name = m.name;
      if (m.update) { line = `Update to v${m.update}`; cls = "upd"; }
      else if (!m.enabled) { line = "Turned off"; cls = "off"; }
      else line = m.version ? `Installed · v${m.version}` : "Installed";
    } else if (p) {
      name = p.name;
      line = "In the mods list";
    } else if (catalog.projects.length || installed.length) {
      continue;
    }
    txt.append(el("b", "", name), el("i", cls, line));
    t.append(el("span", "tile-ico", name.charAt(0).toUpperCase()), txt);
    t.title = m ? "Open Mods & Addons › Installed" : "Open Mods & Addons › Discover";
    t.addEventListener("click", () => show("mods", false, m ? "installed" : "discover"));
    box.append(t);
  }
}

// ------------------------------------------------------------------ My Mods
async function loadInstalled(quiet) {
  const c = pickedClient();
  if (!c) { installed = []; if (!quiet) renderInstalled(); return; }
  try {
    const r = await invoke("mods_installed", { client: c.id });
    installed = r.mods;
  } catch (e) {
    installed = [];
    if (!quiet) sayErr("mods-status", e);
  }
  for (const badge of [$("nav-mods-count"), $("tab-mods-count")]) {
    if (!badge) continue;
    badge.hidden = !installed.length;
    badge.textContent = String(installed.length);
  }
  renderInstalled();
  renderModTiles();
  refreshModUpdates();
  document.dispatchEvent(new CustomEvent("leyline:mods-changed", { detail: { client: c.id, from: "mods" } }));
}

// The title bar's pill: Workshop updates for mods in any of your clients.
let modUpdates = [];
async function refreshModUpdates() {
  try { modUpdates = await invoke("mods_updates"); } catch (_) { modUpdates = []; }
  const pill = $("tb-modupd");
  pill.hidden = !modUpdates.length;
  pill.textContent = modUpdates.length === 1 ? "1 mod update" : `${modUpdates.length} mod updates`;
  pill.title = modUpdates.map((u) => `${u.name}: v${u.version || "?"} → v${u.update} (${u.client_name})`).join("\n");
}
$("tb-modupd").addEventListener("click", () => {
  // Open Mods › Installed on a client that has an update, if the picked one has none.
  const c = pickedClient();
  const u = modUpdates.find((x) => c && x.client === c.id) || modUpdates[0];
  if (u && (!c || u.client !== c.id)) {
    state.settings.selected_client = u.client;
    $("play-client").value = u.client;
    renderClientPickers();
    realmChanged();
    saveSelection();
  }
  show("mods", false, "installed");
});

async function updateMod(c, m) {
  const r = await withNativeOk(() => invoke("mod_install", { client: c.id, project: m.key }));
  return installedText("Updated", r, c);
}

// What was just installed (owner's UX finding, 2 Nov: after an addon install Installed opened on the Mods tab): Installed
// opens on its tab (Mods or Addons) with its card marked, wherever it was installed from (Discover, Game modes, a drop,
// the Leyline addons, a loadout).
function noteInstalled(key, shelf) {
  justInstalled = key || "";
  if (shelf === "mods" || shelf === "addons") {
    instShelf = shelf;
    try { localStorage.setItem("leyline.instShelf", instShelf); } catch (_) { /* fine */ }
    if (instList && instList.tab() !== instShelf) instList.setTab(instShelf);
  }
}

// Installed › Mods | Addons (owner 31 Oct 19:22): the same rule as Discover (shelf.js installed(): the mods list's entry,
// else an addon is one that only adds addon folders). The Leyline addons are under Addons (addons.js). The list is the
// list kit's (listkit.js): the two tabs (#inst-tabs), a search, chips (Updates, Turned off, Favourites), Sort, the count,
// the stars (the same favourites as Discover), keys, and Enter for Update or the mod's page.
let instShelf = "mods";
try { if (localStorage.getItem("leyline.instShelf") === "addons") instShelf = "addons"; } catch (_) { /* fine */ }
const INST_LINE = {
  mods: "Mods change the game itself: models, graphics, game modes, realm scripts, sounds and maps.",
  addons: "Addons are in-game interface helpers. They live in your game's AddOns folder.",
};
const shelfOfInstalled = (m) => (window.LeylineShelf ? window.LeylineShelf.installed(m, catalog.projects) : "mods");
function setInstShelf(sh) {
  sh = sh === "addons" ? "addons" : "mods";
  if (instList && instList.tab() !== sh) { instList.setTab(sh); return; }   // its onChange draws the rest
  instShelf = sh;
  try { localStorage.setItem("leyline.instShelf", instShelf); } catch (_) { /* fine */ }
  renderInstalled();
}
// Find mods / Find addons: Discover on the same tab
if ($("mods-find")) $("mods-find").addEventListener("click", () => { if (window.LeylineThemes) window.LeylineThemes.showTab(instShelf); });
const instPage = (m) => { const p = catalog.projects.find((x) => x.id === m.key); return p ? p.page || p.repo || "" : ""; };
function instWords(m) {
  const p = catalog.projects.find((x) => x.id === m.key);
  return [m.name, (m.addons || []).join(" "), (m.patches || []).join(" "), (m.natives || []).join(" "), m.credits,
    m.source === "catalog" ? "Workshop" : "From your PC", p ? p.author : "", p ? p.description : "", p ? (p.tags || []).join(" ") : ""];
}
let instList = null;
function instListMake() {
  if (instList || !window.LeylineList || !$("mods-list")) return instList;
  const tabsEl = $("inst-tabs");
  const badges = { mods: $("inst-n-mods"), addons: $("inst-n-addons") };   // kept: the kit's tab buttons get them back
  instList = window.LeylineList.create($("mods-list"), {
    id: "mods-installed",
    key: (m) => m.key,
    label: "Installed",
    noun: (n, total) => { const w = instShelf === "addons" ? ["addon", "addons"] : ["mod", "mods"]; return n === total ? `${n} ${n === 1 ? w[0] : w[1]}` : `${n} of ${total} ${total === 1 ? w[0] : w[1]}`; },
    render: instCard,
    search: { placeholder: "Search what you installed", label: "Search installed mods and addons", fields: instWords },
    tabs: { items: [{ id: "mods", label: "Mods", long: "Mods", filter: (m) => shelfOfInstalled(m) === "mods" },
      { id: "addons", label: "Addons", long: "Addons", filter: (m) => shelfOfInstalled(m) === "addons" }],
    value: instShelf, el: tabsEl, idPrefix: "inst-tab-", buttonClass: "ly-tab", storageKey: "leyline.instShelf", label: "Installed",
    panel: $("pane-installed"),
    onChange: (t) => { instShelf = t; renderInstalled(); } },
    filters: [
      { id: "upd", label: "Updates", test: (m) => !!m.update },
      { id: "off", label: "Turned off", test: (m) => !m.enabled },
      { id: "fav", label: "Favourites", test: (m) => !!(instList && instList.isFav(m)) },
    ],
    filtersLabel: "Show",
    sort: { options: [{ id: "name", label: "Name", compare: (a, b) => String(a.name).localeCompare(String(b.name)) },
      { id: "upd", label: "Updates first", compare: (a, b) => (b.update ? 1 : 0) - (a.update ? 1 : 0) }], none: "As installed", label: "Sort" },
    favourites: { id: "mods", label: "Favourites" },
    primary: (m, node) => {
      const up = node.querySelector(".inst-upd:not([disabled])");
      if (up) { up.click(); return; }
      const page = instPage(m);
      if (page) openUrl(page);
    },
    empty: (info, api) => {
      const c = pickedClient();
      if (!c) return "Add your game client first.";
      if (buildTag(c.build) === "other" && !installed.length) return "Leyline does not know this client's build, so it installs no mods or addons here. Realms work.";
      if (info.reason === "none" || info.reason === "tab") {
        const word = instShelf === "addons" ? "addons" : "mods";
        return { text: `No ${word} in this client yet. Find some in Discover › ${instShelf === "addons" ? "Addons" : "Mods"}, or drop one above.`,
          actions: [{ label: instShelf === "addons" ? "Find addons" : "Find mods", primary: true, run: () => { show("mods", false, "discover"); if (window.LeylineThemes) window.LeylineThemes.showTab(instShelf); } }] };
      }
      return noMatch(info, api, instShelf === "addons" ? "addons" : "mods");
    },
    classes: { body: "cards mods-grid", empty: "inst-empty" },
  });
  instShelf = instList.tab();
  for (const [k, b] of Object.entries(badges)) {
    const btn = $("inst-tab-" + k);
    if (!btn) continue;
    btn.dataset.shelf = k;   // the old selector (#inst-tabs .ly-tab[data-shelf])
    if (b) btn.append(b);
  }
  return instList;
}
// One installed mod or addon: its card (name, version badge, where from, Update ready, what it put in), its star, and
// Turn off / Update / Remove.
function instCard(m, ctx) {
  const c = pickedClient();
  const card = el("div", "card" + (m.enabled ? "" : " off") + (m.update ? " has-update" : "") + (m.key === justInstalled ? " just" : ""));
  card.dataset.key = m.key;
  const top = el("div", "top");
  const name = el("span", "name");
  name.append(ctx.hl(m.name));
  const star = ctx.star();
  star.classList.add("inst-star");
  top.append(name, el("span", "build", m.version ? "v" + m.version : (m.source === "catalog" ? "Workshop" : "Local")), star);
  const tags = el("div", "tags");
  tags.append(el("span", "tag", m.source === "catalog" ? "Workshop" : "From your PC"));
  if (m.update) tags.append(el("span", "tag gold", `v${m.update} ready`));
  if (m.native_code) tags.append(el("span", "tag warn", "Native code"));
  if (!m.enabled) tags.append(el("span", "tag", "Turned off"));
  const meta = el("div", "meta");
  const what = [];
  if (m.addons.length) what.push("Addons: " + m.addons.join(", "));
  if (m.patches.length) what.push("Patch: " + m.patches.join(", "));
  if (m.natives && m.natives.length) what.push("Native: " + m.natives.join(", "));
  meta.append(el("span", "", what.join(" · ")));
  if (m.credits) meta.append(el("span", "", m.credits));
  if (m.update) meta.append(el("span", "ok", `Update available: v${m.update}`));
  else if (m.source === "catalog" && !m.in_catalog) meta.append(el("span", "warn", "No longer in the mods list for this game"));
  for (const w of m.warnings || []) meta.append(el("span", "warn", w));
  if (ctx.query) window.LeylineList.mark(meta, ctx.query);
  const acts = el("div", "acts");
  const tog = el("button", "btn small", m.enabled ? "Turn off" : "Turn on");
  tog.addEventListener("click", () => modAction("mod_set_enabled", { client: c.id, key: m.key, on: !m.enabled }, m.enabled ? `Turned off ${m.name}.` : `Turned on ${m.name}.`));
  acts.append(tog);
  if (m.update) {
    const up = el("button", "btn small primary inst-upd", "Update");
    up.addEventListener("click", async () => {
      up.disabled = true;
      up.textContent = "Updating…";
      say("mods-status", `Downloading ${m.name} v${m.update}…`);
      try { say("mods-status", await updateMod(c, m), "ok"); }
      catch (e) { sayErr("mods-status", e); }
      await loadInstalled();
      renderCatalog();
    });
    acts.append(up);
  }
  // One click: removal takes out only the files Leyline put in and you have not changed, so nothing of yours goes.
  const rm = el("button", "btn small danger", "Remove");
  rm.addEventListener("click", async () => {
    rm.disabled = true;
    rm.textContent = "Removing…";
    try {
      const r = await invoke("mod_uninstall", { client: c.id, key: m.key });
      let t = `Removed ${m.name}: ${r.removed_files} files.`;
      if (r.changed && r.changed.length) t += ` Left the files you changed: ${r.changed.join(", ")}.`;
      if (r.kept.length) t += ` Kept files Leyline did not put there: ${r.kept.join(", ")}.`;
      if (m.source === "catalog") t += " Install it again any time from Mods & Addons › Discover.";
      say("mods-status", t, r.changed && r.changed.length ? "warn" : "ok");
    } catch (e) { sayErr("mods-status", e); }
    await loadInstalled();
    renderCatalog();
  });
  acts.append(rm);
  card.append(top, tags, meta, acts);
  return card;
}
function renderInstalled() {
  const c = pickedClient();
  const list = instListMake();
  if (list && list.tab() !== instShelf) instShelf = list.tab();
  const pane = $("pane-installed");
  if (pane) pane.dataset.shelf = instShelf;   // addons.js shows the Leyline addons only on Addons
  if (!list) document.querySelectorAll("#inst-tabs .ly-tab").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.shelf === instShelf)));
  const n = { mods: 0, addons: 0 };
  for (const m of installed) n[shelfOfInstalled(m)]++;
  for (const k of ["mods", "addons"]) { const b = $("inst-n-" + k); if (b) { b.hidden = !n[k]; b.textContent = String(n[k]); } }
  if ($("inst-line")) $("inst-line").textContent = INST_LINE[instShelf];
  if ($("mods-find")) $("mods-find").textContent = instShelf === "addons" ? "Find addons" : "Find mods";
  if ($("mods-list-yours")) $("mods-list-yours").textContent = instShelf === "addons" ? "List your addon" : "List your mod";
  if (window.LeylineAddons) window.LeylineAddons.renderMods(c);   // the Leyline addons, first on Installed › Addons (addons.js)
  if (window.LeylineLoadouts) window.LeylineLoadouts.renderInstalled(c);   // the Loadouts box on top (loadouts.js)
  $("mods-client-note").textContent = c ? (offeredBuild(c.build) ? c.build : "Other build") : "";
  $("mods-client-note").title = c ? `${buildLabel(c.build)} · ${c.path}` : "";
  const ups = installed.filter((m) => m.update);
  $("mods-update-all").hidden = ups.length < 2;
  $("mods-update-all").textContent = `Update all (${ups.length})`;
  if (!list) return;
  list.update(c ? installed : []);
  const just = list.body.querySelector(".card.just");
  if (just && currentPage === "mods" && currentTab === "installed" && just.scrollIntoView) just.scrollIntoView({ block: "nearest" });
}

async function modAction(cmd, args, okText) {
  try { await invoke(cmd, args); say("mods-status", okText, "ok"); }
  catch (e) { sayErr("mods-status", e); }
  await loadInstalled();
  renderCatalog();
}

function showProblems(text) {
  const ul = $("mods-problems");
  const lines = String(text).split("\n").filter(Boolean);
  ul.hidden = lines.length < 2;
  ul.replaceChildren(...(lines.length < 2 ? [] : lines.slice(0, 30).map((l) => el("li", "", l))));
  return lines.length < 2 ? String(text) : `It can't be installed (${lines.length} problems):`;
}

async function installLocal(paths) {
  const c = needClient("mods-status");
  if (!c || !paths.length) return;
  $("mods-problems").hidden = true;
  for (const path of paths) {
    const name = path.split(/[\\/]/).pop();
    say("mods-status", `Checking ${name}…`);
    try {
      const m = await withNativeOk(() => invoke("mod_install_local", { client: c.id, path }));
      say("mods-status", installedText("Installed", m, c), m.warnings.length ? "warn" : "ok");
      noteInstalled(m.key, shelfOfInstalled(m));
      if (window.LeylineGuide) window.LeylineGuide.moment("mod");   // the first mod: one line, once (guide.js)
    } catch (e) {
      say("mods-status", `${name}: ` + showProblems(e), "bad");
      break;
    }
  }
  await loadInstalled();
  renderCatalog();
}
$("mods-update-all").addEventListener("click", async () => {
  const c = needClient("mods-status");
  if (!c) return;
  const ups = installed.filter((m) => m.update);
  const b = $("mods-update-all");
  b.disabled = true;
  const done = [];
  for (const m of ups) {
    say("mods-status", `Updating ${m.name} (${done.length + 1} of ${ups.length})…`);
    try { await updateMod(c, m); done.push(m.name); }
    catch (e) { say("mods-status", `${m.name}: ` + oops(e, OOPS["mods-status"]), "bad"); break; }
  }
  if (done.length === ups.length) say("mods-status", `Updated ${done.join(", ")}.`, "ok");
  b.disabled = false;
  await loadInstalled();
  renderCatalog();
});
$("mods-pick-file").addEventListener("click", async () => { const p = await invoke("pick_mod_file").catch(() => null); if (p) installLocal([p]); });
$("mods-pick-folder").addEventListener("click", async () => { const p = await invoke("pick_mod_folder").catch(() => null); if (p) installLocal([p]); });

// Files dropped on the window (Tauri gives their paths). Only Mods › Installed takes them; anywhere else they lead there.
function setupDrop() {
  const wv = window.__TAURI__ && window.__TAURI__.webview && window.__TAURI__.webview.getCurrentWebview
    ? window.__TAURI__.webview.getCurrentWebview() : null;
  if (!wv || !wv.onDragDropEvent) return;
  // Over the play bar's game client (its button, its open list, the add panel) or on Find your game files, a drop
  // adds a game client (clientImport); over the rest of the play bar nothing moves; anywhere else it is a mod.
  wv.onDragDropEvent((ev) => {
    const t = ev.payload && ev.payload.type;
    // the sign-in screen: a drop is the Hero Key file (gate.js), nothing else
    if (window.LeylineGate && window.LeylineGate.isOpen()) {
      if (t === "enter" || t === "over") window.LeylineGate.dragOver(true);
      else if (t === "leave") window.LeylineGate.dragOver(false);
      else if (t === "drop") window.LeylineGate.dropPath((ev.payload.paths || [])[0]);
      return;
    }
    const here = currentPage === "mods" && currentTab === "installed";
    const pos = ev.payload && ev.payload.position;
    const dpr = window.devicePixelRatio || 1;
    const x = pos ? pos.x / dpr : -1, y = pos ? pos.y / dpr : -1;
    const client = clientDropAt(x, y);
    const realmBox = !client && barDropAt(x, y);
    const bar = $("play-form").getBoundingClientRect();
    const onBar = y >= bar.top && y <= bar.bottom;
    if (t === "enter" || t === "over") {
      barDragOn(realmBox);
      if (realmBox) { clientDragOn(false); $("drop").classList.remove("over"); return; }
      if (client) {
        if (!menuOpen && $("cimp").hidden && currentPage !== "findfiles") openClientMenu();
        clientDragOn(true);
        $("drop").classList.remove("over");
      } else {
        clientDragOn(false);
        if (onBar) return;
        $("drop").classList.add("over");
        if (!here) show("mods", false, "installed");
      }
    } else if (t === "leave") { $("drop").classList.remove("over"); clientDragOn(false); barDragOn(false); }
    else if (t === "drop") {
      $("drop").classList.remove("over");
      clientDragOn(false);
      barDragOn(false);
      const paths = ev.payload.paths || [];
      if (realmBox) { if (paths[0]) barImport(paths[0]); return; }
      if (client) { if (paths[0]) clientImport(paths[0]); return; }
      if (onBar) return;
      if (!here) show("mods", false, "installed");
      installLocal(paths);
    }
  });
}

// ------------------------------------------------------------------ Realms
let realmsCache = [];
let realmsFresh = false;   // the network's realm list has answered once (before that the kept copy may show)
async function loadRealms(quiet) {
  if (!quiet) say("rl-status", "Loading the realm list…");
  // the list kept from last time, at once, while the network answers (audit 12 Nov: a site that is down made the page
  // wait). It never replaces a fresher list.
  if (!realmsFresh && !realmsCache.length) {
    try {
      const k = await invoke("realms_list_cached");
      if (k && k.realms && !realmsFresh) {
        k.realms = k.realms.filter((r) => !hiddenRealm(r));
        realmsCache = k.realms.map((r) => (r.official && r.slug === "official" ? Object.assign({}, r, { name: HANGOUT.name }) : r));
        realmNames = k.realms.map((r) => r.name);
        networkTotals = k.network || null;
        fillRealmFilters();
        rlLoading = false;
        renderRealms();
        renderRealmRows();
        renderSrvHead();
        renderPlayBar();
      }
    } catch (_) { /* an older core: the network list only */ }
  }
  try {
    const l = await invoke("realms_list");
    realmsFresh = true;
    // the core's official record (slug "official", realms.rs official()) is the Hangout; community records never are.
    // Its card keeps the record's own type (PvE): the Official badge already says what HANGOUT.kind would.
    l.realms = l.realms.filter((r) => !hiddenRealm(r));
    realmsCache = l.realms.map((r) => (r.official && r.slug === "official" ? Object.assign({}, r, { name: HANGOUT.name }) : r));
    realmNames = l.realms.map((r) => r.name);
    const when = l.saved_at ? new Date(l.saved_at * 1000).toLocaleString() : "an earlier visit";
    const n = l.realms.length - 1;
    networkTotals = l.network || null;
    if (l.from_cache) say("rl-status", `Offline: showing the list as it was ${when}.`, "warn");
    else if (l.error) { console.error("[leyline] realm list", l.error); say("rl-status", "The community list isn't reachable right now. Try Refresh in a minute.", "warn"); }
    else say("rl-status", "", "ok");   // the count is already on the network line above
    fillRealmFilters();
  } catch (e) {
    sayErr("rl-status", e);
  }
  rlLoading = false;
  renderRealms();
  renderRealmRows();
  renderSrvHead();
  renderPlayBar();   // the play bar's realm name and version come from the list too
  document.dispatchEvent(new CustomEvent("leyline:realms"));   // Home's Popular realms row (home.js)
}

// The network (realms.rs order): featured community realms, the Leyline Sample Hangout (ours, temporary), the
// other live realms by the network's fair score, offline ones. The filters narrow the community realms.
let networkTotals = null;
let p2pHosting = false;   // direct connections are shown only when the p2p_hosting flag is on (Settings > Advanced)
let heroKeyOn = false;    // "Log in with your Hero Key" only with the herokey_sso flag on (docs/HERO-KEY-SSO.md)
const TAG_LABELS = { blizzlike: "Original rules", boosted: "Boosted", custom: "Custom", pvp: "PvP", rp: "RP", hangout: "Hangout", bots: "Bots" };
const REGION_LABELS = { "na-east": "North America east", "na-west": "North America west", sa: "South America", eu: "Europe",
  africa: "Africa", me: "Middle East", asia: "Asia", oce: "Oceania" };
const RL_FILTERS = [
  ["rl-f-build", (r) => (r.build && offeredBuild(r.build) ? [r.build] : []), (v) => v],
  ["rl-f-tag", (r) => r.tags || [], (v) => TAG_LABELS[v] || v],
  ["rl-f-region", (r) => (r.region ? [r.region] : []), (v) => REGION_LABELS[v] || v],
  ["rl-f-lang", (r) => (r.language ? [r.language.split("-")[0]] : []), (v) => v],
];
function fillRealmFilters() {
  for (const [id, values, label] of RL_FILTERS) {
    const sel = $(id);
    if (!sel) continue;
    const cur = sel.value;
    const vs = [...new Set(realmsCache.flatMap(values))].sort();   // both tabs: the filters work in whichever is open
    sel.replaceChildren(el("option", "", "any"), ...vs.map((v) => { const o = el("option", "", label(v)); o.value = v; return o; }));
    sel.firstChild.value = "";
    sel.value = vs.includes(cur) ? cur : "";
  }
}
function realmMatches(r) {
  if (hiddenBuild(r.build) && !r.official) return false;   // 4.3.4 / 5.4.8 realms are not offered for now (MAX_OFFERED_BUILD)
  for (const [id, values] of RL_FILTERS) {
    const v = $(id) && $(id).value;
    if (v && !values(r).includes(v)) return false;
  }
  if ($("rl-f-now") && $("rl-f-now").checked && !(r.state === "live" && (r.players || 0) > 0)) return false;
  return true;
}
// The search box: a realm's name, line and tags (the official realm too).
function realmSearched(r) {
  const q = $("rl-q") ? $("rl-q").value.trim().toLowerCase() : "";
  if (!q) return true;
  return [r.name, r.tagline, r.kind, CORE_LABELS[r.core], REGION_LABELS[r.region], r.language, ...(r.tags || []).map((t) => TAG_LABELS[t] || t)]
    .filter(Boolean).join(" ").toLowerCase().includes(q);
}
// How many filters are on (the number on the FILTERS button).
function realmFilterCount() {
  let n = RL_FILTERS.filter(([id]) => $(id) && $(id).value).length;
  if ($("rl-f-now") && $("rl-f-now").checked) n++;
  if ($("rl-mine") && $("rl-mine").checked) n++;
  return n;
}
// The lists (owner 2026-11-02: Official realms are ours, Community realms the players' own; 12 Nov: the in-game Realm
// Selection's categories, WrathUnleashedUI/Leyline.lua TABS): All, Official, Community, Single player & Den (this
// PC's own worlds), Friends' worlds (where your Leyline friends are, and friends' worlds you joined) and Favourites
// (the star on any row). One panel; the list is the list kit's (listkit.js, docs/LAUNCHER-LISTS.md): the search, the
// tabs, the sort, the stars, the keys and the empty states are the same as every other list in the launcher.
const RL_TABS = ["all", "official", "community", "mine", "friends", "fav"];
const RL_TAB_DEFS = [
  { id: "all", label: "All", long: "All realms", suggest: false },
  { id: "official", label: "Official", long: "Official realms" },
  { id: "community", label: "Community", long: "Community realms" },
  { id: "mine", label: "Single player & Den", long: "Single player & Den" },
  { id: "friends", label: "Friends' worlds", long: "Friends' worlds" },
  { id: "fav", label: "Favourites", long: "Favourites", suggest: false },
];
const realmTab = (r) => (r && (r.official || inLineup(r)) ? "official" : "community");
const plural = (n, one) => `${n} ${one}${n === 1 ? "" : "s"}`;
// The count line of the open tab, from the list itself: the network's own totals count every realm online, ours
// too (realms.rs totals), which is how the official realm came to be called a "community realm".
const listedRealms = () => realmsCache.filter((r) => !(hiddenBuild(r.build) && !r.official));
const tabRealms = (tab) => listedRealms().filter((r) => realmTab(r) === tab);
const livePlayers = (rs, k) => rs.reduce((n, r) => n + (r.state === "live" ? r[k] || 0 : 0), 0);

// Friends' worlds: your Leyline friends in a world now (friends.js friends_state: "playing" is the realm's name when
// they share it) and the friends' worlds this PC joined (worldfriends.js friend_worlds). A core without them: none.
let rlFriends = { friends: [], worlds: [], loaded: false };
async function loadRealmFriends() {
  const [s, w] = await Promise.all([invoke("friends_state").catch(() => null), invoke("friend_worlds").catch(() => null)]);
  const friends = s && Array.isArray(s.friends) ? s.friends : [];
  const worlds = Array.isArray(w) ? w.filter((x) => x && x.realm_id) : [];
  const was = JSON.stringify([rlFriends.friends, rlFriends.worlds]);
  rlFriends = { friends, worlds, loaded: true };
  if (JSON.stringify([friends, worlds]) !== was) renderRealms();
}
const sameName = (a, b) => String(a || "").trim().toLowerCase() === String(b || "").trim().toLowerCase() && !!String(a || "").trim();
const friendLabel = (f) => f.name || `Hero ${String(f.peer_id || "").slice(0, 4).toUpperCase()}`;
function friendsOn(name) {
  return rlFriends.friends.filter((f) => f.online && sameName(f.playing, name)).map(friendLabel);
}
function friendsLine(names) {
  if (!names.length) return "";
  if (names.length === 1) return `${names[0]} is here`;
  if (names.length === 2) return `${names[0]} and ${names[1]} are here`;
  return `${names[0]} and ${names.length - 1} friends are here`;
}

// The Population column of the in-game list (Leyline.lua L.Population): Low, Medium, High or Full against the
// realm's maximum when it has one (a quarter, 60 %, full), else 10 and 50 players (people, not bots).
const POP_RANK = { Low: 1, Medium: 2, High: 3, Full: 4 };
function population(r) {
  if (!r || r.state !== "live") return "Offline";
  const p = r.players;
  if (p == null) return "";
  if (r.max_players > 0) { const f = p / r.max_players; return f >= 1 ? "Full" : f >= 0.6 ? "High" : f >= 0.25 ? "Medium" : "Low"; }
  return p >= 50 ? "High" : p >= 10 ? "Medium" : "Low";
}
// The Type column: PvP, RP, RP PvP where they fit, else the realm's own type (PvE)
function realmType(r) {
  const t = (r && r.tags) || [];
  if (t.includes("pvp") && t.includes("rp")) return "RP PvP";
  if (t.includes("pvp")) return "PvP";
  if (t.includes("rp")) return "RP";
  return (r && r.kind) || "";
}

// The rows of the list: { k: key, type: "realm" | "local" | "world" | "fw", ... }. Keys stay the same across loads,
// so a star, the focus and the selection follow the realm: a listed realm by its id, ours as "r:official".
const realmKeyOf = (r) => "r:" + (r.official && (r.slug === "official" || !r.realm_id) ? "official" : r.realm_id || r.slug || r.address || r.name);
function realmListRows() {
  const order = realmsCache.filter((r) => r.official).concat(realmsCache.filter(inLineup), realmsCache.filter((r) => !r.official && !inLineup(r)));
  const out = order.filter((r) => !(hiddenBuild(r.build) && !r.official)).map((r) => ({ k: realmKeyOf(r), type: "realm", r }));
  // this PC's own worlds (the play bar's realm picker has the same rows: Your Den, Single player, the original world);
  // a phone has Play solo on its own Play page instead (phone.js), and no Den
  if (onPhone()) return out;
  out.push({ k: "l:den", type: "local", L: DEN, name: DEN.name });
  if (worldsHere() && !localHosted(LOCAL.solo)) for (const k of WORLD_KINDS) out.push({ k: "w:" + k, type: "world", world: k, name: WORLD_TITLE[k] });
  else out.push({ k: "l:solo", type: "local", L: LOCAL.solo, name: LOCAL.solo.name });
  // friends' worlds this PC joined (Join again needs no new link)
  for (const w of rlFriends.worlds) out.push({ k: "f:" + w.realm_id, type: "fw", w, name: w.name || "Your friend's world" });
  return out;
}
function onPhone() { return !!(window.LeylinePlatform && window.LeylinePlatform.mobile); }
const hostedRealmId = () => (kit && kit.realm && kit.realm.realm_id) || "";
function rowInTab(row, tab) {
  if (tab === "all") return true;
  if (row.type === "realm") {
    if (tab === "official" || tab === "community") return realmTab(row.r) === tab;
    if (tab === "mine") return !!hostedRealmId() && row.r.realm_id === hostedRealmId();   // the realm this PC hosts
    if (tab === "friends") return friendsOn(row.r.name).length > 0;
    return false;
  }
  if (tab === "mine") return row.type === "local" || row.type === "world";
  if (tab === "friends") return row.type === "fw";
  return false;
}
// what the search box looks at: the name, the line, the type, the rules, the region, the language, the version
function rowWords(row) {
  if (row.type === "realm") {
    const r = row.r;
    return [r.name, r.tagline, r.kind, realmType(r), CORE_LABELS[r.core], REGION_LABELS[r.region], r.language, r.build,
      population(r), ...(r.tags || []).map((t) => TAG_LABELS[t] || t), ...friendsOn(r.name)];
  }
  if (row.type === "local") return [row.name, row.L.title, "on this pc", row.L === DEN ? "den" : "single player"];
  if (row.type === "world") return [row.name, WORLD_LINE[row.world], "on this pc", "single player"];
  return [row.name, "friend", "friends' world"];
}

const RL_SORTS = [
  { id: "name", label: "Name", compare: (a, b) => String(a.r ? a.r.name : a.name).localeCompare(String(b.r ? b.r.name : b.name), undefined, { sensitivity: "base" }) },
  { id: "pop", label: "Population", desc: true, compare: (a, b) => (POP_RANK[population(a.r)] || 0) - (POP_RANK[population(b.r)] || 0) },
  { id: "players", label: "Players · bots", desc: true, compare: (a, b) => {
    const v = (x) => (x.r && x.r.state === "live" ? (x.r.players || 0) * 1000 + (x.r.bots || 0) : -1);
    return v(a) - v(b);
  } },
  { id: "type", label: "Type", compare: (a, b) => String(a.r ? realmType(a.r) : "~").localeCompare(String(b.r ? realmType(b.r) : "~")) },
];
const rowLive = (row) => (row.type === "realm" ? row.r.state === "live" : row.type !== "fw");
var rlList = null;   // the list kit's list (made below, once the page's elements are in)
function setRealmsTab(tab, focus) {
  if (!RL_TABS.includes(tab) || !rlList) return;
  rlList.setTab(tab, focus);
}
const rlTabNow = () => (rlList ? rlList.tab() : "official");

function networkLine(tab) {
  if (tab === "mine") return "Your own worlds, on this PC";
  if (tab === "friends") {
    const on = rlFriends.friends.filter((f) => f.online && f.playing).length;
    return on ? `${plural(on, "friend")} in a world now` : "Your Leyline friends' worlds";
  }
  if (tab === "fav") { const n = rlList ? rlList.rows().filter((x) => rlList.isFav(x)).length : 0; return plural(n, "favourite"); }
  if (!realmsCache.length) return "";
  const rs = tab === "all" ? listedRealms() : tabRealms(tab);
  const word = tab === "official" ? "official realm" : tab === "community" ? "community realm" : "realm";
  return `${plural(rs.length, word)} · ${livePlayers(rs, "players")} playing`;
}
const RL_NOTES = {
  all: ["Official realms are run by us around the clock; community realms are run by players.", "community", "See community realms"],
  official: ["Community realms are run by players and may go offline.", "community", "See community realms"],
  community: ["Official realms are run by us around the clock.", "official", "See official realms"],
  mine: ["Your own worlds run on this PC: just you, or the friends you invite.", "", ""],
  friends: ["Add friends on the Single player page: their worlds and the realms they play show here.", "", ""],
  fav: ["Press the star on any realm to keep it here, and in the play bar's realm list.", "", ""],
};
// the line above the list, the line under it and the FILTERS count, after every paint of the list
function realmsChrome(view) {
  const tab = view ? view.tab : rlTabNow();
  const net = $("rl-net");
  if (net) {
    net.textContent = networkLine(tab);
    const bots = tab === "official" || tab === "community" || tab === "all" ? livePlayers(tab === "all" ? listedRealms() : tabRealms(tab), "bots") : 0;
    net.title = bots ? `${bots} bots too` : "";
  }
  const note = $("rl-tab-note");
  if (note) {
    const [text, other, label] = RL_NOTES[tab] || ["", "", ""];
    const parts = [el("span", "", text)];
    if (other) {
      const sw = el("button", "link-s", label);
      sw.type = "button";
      sw.addEventListener("click", () => setRealmsTab(other, true));
      parts.push(" ", sw);
    } else if (tab === "friends" || tab === "mine") {
      const go = el("button", "link-s", tab === "friends" ? "Open Friends" : "Open Single player");
      go.type = "button";
      go.addEventListener("click", () => show("single"));
      parts.push(" ", go);
    }
    note.replaceChildren(...parts);
  }
  const nf = realmFilterCount();
  $("rl-fcount").hidden = !nf;
  $("rl-fcount").textContent = nf ? String(nf) : "";
  $("rl-filters-btn").classList.toggle("on", nf > 0);
  $("rl-filters-btn").setAttribute("aria-label", nf ? `Filters, ${nf} on` : "Filters");
}

// The words when the open tab shows nothing (the list kit's info: why), and the one button that helps.
function realmsEmpty(info) {
  const tab = info.tab;
  if (info.reason === "loading") return { text: "Loading the realm list…" };
  if (!info.inTab) {
    if (tab === "community") return "No community realms yet. Be the first to run one: see Host your own below.";   // the banner below is the one call to action
    if (tab === "official") return "The official realms aren't on the list right now. Try Refresh in a minute.";
    if (tab === "all") return "The realm list is empty right now. Try Refresh in a minute.";
    if (tab === "fav") return "No favourites yet. Press the star by a realm to keep it here.";
    if (tab === "friends") {
      return { text: rlFriends.friends.length ? "None of your Leyline friends are in a world right now." : "Your friends' worlds show here once you add friends.",
        actions: [{ label: "Open Friends", run: () => show("single") }] };
    }
    if (tab === "mine") return { text: "Your own worlds show here: Single player and Your Den.", actions: [{ label: "Open Single player", run: () => show("single") }] };
  }
  const q = info.query;
  const word = tab === "official" || tab === "community" ? tab + " " : "";
  if (info.suggest) {
    return { text: q ? `No ${word}realm called “${q}”.` : `No ${word}realms match these filters.`,
      actions: [{ label: `Show ${info.suggest.n} in ${info.suggest.long}`, run: () => setRealmsTab(info.suggest.tab, true) }] };
  }
  if (q) return { text: `No realm called “${q}”.`, actions: [{ label: "Clear search", run: () => rlList.setQuery("", true) }] };
  const mine = $("rl-mine").checked;
  return { text: mine && info.filtersOn === 1 ? "None of the listed realms fits your clients." : "No realms match these filters.",
    actions: [{ label: "Clear filters", run: clearRealmFilters }] };
}
// A realm's look, as on the website: a hue from its name, the crystal for the official realm.
function nameHue(name) {
  let h = 2166136261;
  for (const ch of String(name)) h = Math.imul(h ^ ch.codePointAt(0), 16777619) >>> 0;
  return h % 360;
}
function fillRealmIcon(box, name, official) {
  box.replaceChildren();
  box.classList.toggle("crystal", !!official);
  if (official) {
    const img = el("img");
    img.src = "art/brand/leyline-crystal-256.webp";
    img.alt = "";
    img.decoding = "async";
    box.append(img);
    box.style.removeProperty("--h");
  } else {
    box.style.setProperty("--h", String(nameHue(name)));
    box.textContent = ([...String(name).trim()][0] || "?").toUpperCase();
  }
  return box;
}
function realmIcon(name, official, size) {
  const box = el("span", "rl-ic" + (size ? " " + size : ""));
  box.setAttribute("aria-hidden", "true");
  return fillRealmIcon(box, name, official);
}
// The client version a realm takes ("3.3.5a"), when the list says and the build is offered.
function realmVersion(r) {
  if (!r) return null;
  if (r.build) return offeredBuild(r.build) ? r.build : null;
  return r.version || null;
}
// How full a realm is, for the bar: at least a sliver when anyone is on; a realm without a limit counts 100.
function playersFill(players, max) {
  if (!players) return 0;
  const cap = max > 0 ? max : 100;
  return Math.max(6, Math.min(100, Math.round((100 * players) / cap)));
}

let rlOtherShown = false;
let rlLoading = true;
let rlLinked = "";      // the realm a website's Open in launcher showed (showLinkedRealm): lit while the page is open   // until the first realm list answer: the list says it is loading, not that it is empty
// One realm: icon · name, version and one tag (Official or Featured) / one line (type · style) · players, population
// and bots · PLAY, Play solo and the star.
function realmCard(r, ctx) {
  const now = state.settings.selected_realm;
  const fit = clientFor(r);
  const live = r.state === "live";
  const card = el("article", "realm-card rl" + (r.address && r.address === now ? " on" : "") + (r.featured ? " featured" : "")
    + (r.official ? " official" : "") + (live ? "" : " off"));
  if (rlLinked && realmKeyOf(r) === rlLinked) card.classList.add("linked");
  // what the phone app reads (phone.js): a relay realm's id and its version, to play it in the browser game
  if (r.realm_id) card.dataset.realmId = r.realm_id;
  if (r.build) card.dataset.build = r.build;
  card.dataset.live = live ? "1" : "0";
  const main = el("div", "rl-main");
  const title = el("div", "rl-title");
  const name = el("b", "name");
  name.append(ctx.hl(r.name));
  if (r.tagline) name.title = r.tagline;
  title.append(name);
  if (r.verified) { const v = el("span", "rl-ok", "✓"); v.title = "Verified host: checked by Leyline staff"; v.setAttribute("aria-label", "Verified host"); title.append(v); }
  const ver = realmVersion(r);
  if (ver) { const v = el("span", "rl-ver", ver); v.title = "Game client version"; title.append(v); }
  if (r.official) { const b = el("span", "rl-bdg o", "Official"); if (r.temporary) b.title = "Temporary: our own realm for now"; title.append(b); }
  else if (r.featured) title.append(el("span", "rl-bdg f", "Featured"));
  const line = el("div", "rl-meta");
  // the realm's type and style (owner 2 Nov UX pass: no server software names, "AzerothCore" is jargon to players)
  const style = (r.official ? [] : (r.tags || []).filter((t) => t !== "pvp" && t !== "rp")).map((t) => TAG_LABELS[t] || t).slice(0, 2);
  const bits = [r.kind, ...style];
  if (r.direct && p2pHosting) bits.push("Direct connect available");
  line.append(el("span", "", bits.filter(Boolean).join(" · ")));
  // everything else about the realm, on hover
  const more = [realmTab(r) === "official" ? "Run by Leyline" : "Community realm", r.is_new ? "New" : "",
    r.region ? (REGION_LABELS[r.region] || r.region) + (r.region_estimated ? " (estimated)" : "") : "", r.language || "",
    ...(r.official ? [] : (r.tags || []).map((t) => TAG_LABELS[t] || t))];
  const u = r.uptime_7d != null ? r.uptime_7d : r.uptime_24h;
  if (u != null) more.push(`${u}% up · ${r.uptime_7d != null ? "7 d" : "24 h"}`);
  if (r.address) more.push("Realmlist: " + r.address);
  line.title = more.filter(Boolean).join(" · ") + (r.rules && r.rules.length ? "\n" + r.rules.slice(0, 4).join("\n") : "");
  main.append(title, line);
  // your Leyline friends on it now (friends.js: when they share what they play)
  const fr = friendsOn(r.name);
  if (fr.length) { const f = el("div", "rl-friends", friendsLine(fr)); f.title = fr.join(", "); main.append(f); }
  const rmods = window.LeylineGameMods ? window.LeylineGameMods.realmMods(r) : [];
  if (rmods.length) {   // the realm's own mod list (docs/GAME-MODS.md); PLAY offers the ones this game lacks
    const um = el("div", "rl-mods", `Uses ${rmods.length} mod${rmods.length === 1 ? "" : "s"}`);
    um.title = rmods.map((m) => window.LeylineGameMods.nameOf(m.id) + (m.version ? " " + m.version : "")).join(", ");
    main.append(um);
  }
  // never a version in the sentence (the card's gold chip already shows it)
  if (r.build && !fit) main.append(el("div", "rl-note", "Needs your own game folder for this version"));
  // the realm's own links, quiet
  const links = el("div", "rl-links");
  if (r.join_url) links.append(linkBtn("Make an account", r.join_url));
  if (r.discord) links.append(linkBtn("Discord", r.discord));
  if (r.report_url && !r.official) { const rep = linkBtn("Report", r.report_url); rep.title = "Report this realm to Leyline staff (on the website)"; links.append(rep); }
  if (links.children.length) main.append(links);
  // players: an online dot and the count, the population (Low, Medium, High, Full) and bots, a small fill bar
  const pop = el("div", "rl-pop");
  const st = el("span", "state " + (live ? "is-online" : "is-offline"));
  st.append(el("span", "dot"));
  if (!live) st.append(el("span", "", "Offline"));
  else if (r.players == null) st.append(el("span", "", "Online"));
  else {
    const n = el("span", "");
    n.append(el("b", "rl-n", String(r.players)), document.createTextNode(r.max_players ? ` / ${r.max_players}` : " playing"));
    st.append(n);
    st.title = `${r.players}${r.max_players ? " of " + r.max_players : ""} playing` + (r.bots ? ` · ${r.bots} bots` : "");
  }
  pop.append(st);
  const level = live ? population(r) : "";
  if (level) {
    const pl = el("span", "rl-popl");
    const lv = el("span", "rl-lv lv-" + level.toLowerCase(), level);
    lv.title = `Population: ${level}`;
    pl.append(lv);
    if (r.bots) pl.append(el("span", "rl-bots", ` · ${r.bots} bots`));
    pop.append(pl);
  }
  if (live) {
    const bar = el("span", "rl-bar");
    bar.setAttribute("aria-hidden", "true");
    const fill = el("i");
    fill.style.width = playersFill(r.players, r.max_players) + "%";
    bar.append(fill);
    pop.append(bar);
  }
  const acts = el("div", "acts");
  if (heroKeyOn && !r.official && r.realm_id && r.state === "live") {
    const hk = el("button", "btn small", "Log in with your Hero Key");
    hk.title = "No account to make: your Hero Key signs you in here (a one-time password, shown below)";
    hk.disabled = !state.settings.clients.length;
    hk.addEventListener("click", () => playRealmHeroKey(r, hk));
    acts.append(hk);
  }
  const play = el("button", "btn small primary rl-play", "Play");
  if (!state.settings.clients.length) { play.disabled = true; play.title = "Add a game client first"; }
  else if (!fit) { play.title = "This realm needs another game folder: Play says which"; }
  else play.title = `Plays with ${fit.name}`;
  // said before Play, not after it (review 4 Nov): the list's own word, and Play still tries
  if (!live && state.settings.clients.length) play.title = "The list says this realm is offline right now: Play tries anyway";
  if (playHeld()) holdPlay(play, false);   // rebuilt right after a start: held like the one it replaces
  play.addEventListener("click", () => playRealm(r, play));
  acts.prepend(play);
  if (window.LeylineHostModes) window.LeylineHostModes.realmRow(r, main, acts);   // its mode, "based on", Fork this mode (hostmodes.js)
  // Play solo right after Play (solo.js, owner 3 Nov): the same game on this PC, with your own game files
  if (window.LeylineSolo) window.LeylineSolo.attach(acts, main, window.LeylineSolo.forRealm(r), window.LeylineSolo.LABEL + " · ");
  acts.append(ctx.star());
  card.append(realmIcon(r.name, r.official), main, pop, acts);
  return card;
}
// One of this PC's own worlds (the play bar's realm picker has the same rows and the same one click): Your Den,
// Single player, and the original world on the Built-in engine.
function localCard(row, ctx) {
  const k = row.world;
  const L = row.L;
  const w = k ? worldSt[k] : null;
  const ready = k ? !!(w && (w.set_up || w.running)) : localStep(L) === "play";
  const running = k ? !!(w && w.running) : ready;
  const picked = mode !== "browser" && (k ? worldChosen() === k : L === DEN ? (denChosen() || (!isOfficial() && chosenRealm() === DEN_LITE)) : localChosen() === L);
  const card = el("article", "realm-card rl rl-local" + (picked ? " on" : ""));
  card.dataset.local = k || L.profile;
  const icon = markIcon(k ? "solo" : L.mark);
  icon.classList.remove("sm");
  const main = el("div", "rl-main");
  const title = el("div", "rl-title");
  const name = el("b", "name");
  name.append(ctx.hl(row.name));
  title.append(name);
  if (ready) { const v = el("span", "rl-ver", k ? WORLD_BUILD : L.build); v.title = "Game client version"; title.append(v); }
  const line = el("div", "rl-meta");
  line.append(el("span", "", k ? WORLD_LINE[k] : L.title));
  main.append(title, line);
  const pop = el("div", "rl-pop");
  const st = el("span", "state " + (running ? "is-online" : "is-local"));
  st.append(el("span", "dot"), el("span", "", k ? worldSub(k) : L === DEN ? denSub() : localSub(L)));
  pop.append(st);
  const acts = el("div", "acts");
  const play = el("button", "btn small primary rl-play", "Play");
  play.title = k ? `${WORLD_TITLE[k]}, on this PC` : L.title;
  if (playHeld()) holdPlay(play, false);
  play.addEventListener("click", () => {
    if (k) { pickWorld(k); playWorld(k); } else localGo(L);
    renderRealms();
  });
  acts.append(play, ctx.star());
  card.append(icon, main, pop, acts);
  return card;
}
// A friend's world this PC joined: Join again (no new link: this PC's own key proves it's you)
function friendWorldCard(row, ctx) {
  const w = row.w;
  const card = el("article", "realm-card rl rl-fw");
  card.dataset.realmId = w.realm_id;
  const main = el("div", "rl-main");
  const title = el("div", "rl-title");
  const name = el("b", "name");
  name.append(ctx.hl(row.name));
  title.append(name);
  const line = el("div", "rl-meta");
  line.append(el("span", "", "Your friend's world · your character is kept there"));
  main.append(title, line);
  const pop = el("div", "rl-pop");
  const st = el("span", "state is-local");
  st.append(el("span", "dot"), el("span", "", "A friend's PC"));
  pop.append(st);
  const acts = el("div", "acts");
  const go = el("button", "btn small primary rl-play", "Join again");
  go.title = "No new link needed while your friend shares their world (or trusts you)";
  go.addEventListener("click", () => joinFriendWorld(w, go));
  acts.append(go, ctx.star());
  card.append(realmIcon(row.name, false), main, pop, acts);
  return card;
}
async function joinFriendWorld(w, btn) {
  const cs = state.settings.clients;
  const c = cs.find((x) => x.id === state.settings.selected_client && x.build === WORLD_BUILD) || cs.find((x) => x.build === WORLD_BUILD);
  if (!c) { say("rl-status", "Your friend's world plays with your own game folder, the same version as theirs: add it with Add a game folder… in the play bar, then press Join again.", "warn"); return; }
  btn.disabled = true;
  say("rl-status", "Joining your friend's world…");
  try {
    const j = await invoke("join_friend_again", { client: c.id, realmId: w.realm_id });
    const g = j && j.guest;
    say("rl-status", g && !g.auto ? `Joining your friend's world. At the login screen: account ${g.account}, password ${g.password}.`
      : "Joining your friend's world: your game signs you in by itself, with your character there.", "ok");
  } catch (e) { sayErr("rl-status", e); }
  btn.disabled = false;
}

function renderRealms() {
  if (!rlList) return;
  if (!rlOtherShown && !isOfficial()) { rlOtherShown = true; $("rl-other").open = true; }   // their own realm address: show where it is set
  rlList.update(realmListRows());
}
// the FILTERS popover's own filters (the list kit asks for them: match, filtersOn, onReset)
function clearRealmSelects() {
  for (const [id] of RL_FILTERS) if ($(id)) $(id).value = "";
  $("rl-f-now").checked = false;
  $("rl-mine").checked = false;
}
function clearRealmFilters() {
  clearRealmSelects();
  renderRealms();
}
function realmRowMatches(row) {
  if (row.type !== "realm") return realmFilterCount() === 0;   // a filter is about realms: your own worlds step aside
  const r = row.r;
  return !($("rl-mine").checked && !clientFor(r)) && realmMatches(r);
}
if (window.LeylineList) {
  rlList = window.LeylineList.create($("rl-list"), {
    id: "realms",
    key: (row) => row.k,
    nameOf: (row) => (row.r ? row.r.name : row.name),
    label: "Realms",
    noun: (n, total) => (n === total ? "" : `Showing ${n} of ${total}`),
    render: (row, ctx) => (row.type === "realm" ? realmCard(row.r, ctx) : row.type === "fw" ? friendWorldCard(row, ctx) : localCard(row, ctx)),
    search: { input: $("rl-q"), fields: rowWords, debounce: "auto" },
    tabs: { items: RL_TAB_DEFS.map((t) => Object.assign({}, t, t.id === "fav" ? { favourites: true } : { filter: (row) => rowInTab(row, t.id) })),
      value: "official", el: $("rl-tabs"), idPrefix: "rl-tab-", panel: $("rl-pane"), storageKey: "leyline.realmsTab", buttonClass: "page-tab",
      label: "Realm lists",
      onChange: (t) => {
        say("rl-status", "");   // a line about the other tab's realm goes with it
        if (t === "friends" || t === "all") loadRealmFriends();
        if (t === "mine") Promise.all([readKit(), readWorlds()]).then(renderRealms).catch(() => {});
      } },
    sort: { options: RL_SORTS, none: "Suggested", el: $("rl-kbar"), label: "Sort" },
    rank: (row) => (rowLive(row) ? 0 : 1),   // offline last, whatever the sort (as in the game's own list)
    group: { of: (row) => (row.type === "fw" ? "Friends' worlds" : row.type !== "realm" ? "On this PC" : row.r.state === "live" ? "Online" : "Offline"),
      order: ["Online", "On this PC", "Friends' worlds", "Offline"] },
    favourites: { label: "Favourites" },
    primary: (row, node) => { const b = node.querySelector(".rl-play"); if (b && !b.disabled) b.click(); },
    match: realmRowMatches,
    filtersOn: () => realmFilterCount(),
    onReset: clearRealmSelects,
    empty: (info) => (rlLoading && !realmsCache.length && (info.tab === "official" || info.tab === "community") && !info.inTab ? "Loading the realm list…" : realmsEmpty(info)),
    classes: { empty: "rl-empty" },
    bar: $("rl-kbar"),
    onChange: realmsChrome,
  });
}
for (const id of ["rl-f-build", "rl-f-tag", "rl-f-region", "rl-f-lang", "rl-f-now"]) {
  if ($(id)) $(id).addEventListener("change", () => renderRealms());
}
// the Realms page: the friends and this PC's own worlds as they are now (the realm list itself loads on its own)
document.addEventListener("leyline:page", (e) => {
  if (!e.detail || e.detail.page !== "realms") { rlLinked = ""; return; }
  loadRealmFriends();
  Promise.all([readKit(), readWorlds()]).then(renderRealms).catch(() => {});
});
$("rl-f-clear").addEventListener("click", clearRealmFilters);
if ($("rl-host")) $("rl-host").addEventListener("click", () => show("host"));
if ($("rl-host-link")) $("rl-host-link").addEventListener("click", () => show("host"));   // the Host page has Link my server
// the hosting guide on the website (docs/leyline/hosting.md, stream/docs.py), on both tabs
if ($("rl-host-guide")) $("rl-host-guide").addEventListener("click", () => openUrl(ADDR.site("/wrath/docs/hosting")));
// The FILTERS popover and the … menu: one open at a time; a click outside or Escape closes it.
function rlPopover(btnId, boxId) {
  const btn = $(btnId), box = $(boxId);
  const set = (open) => { box.hidden = !open; btn.setAttribute("aria-expanded", String(open)); btn.classList.toggle("open", open); };
  btn.addEventListener("click", (ev) => {
    ev.stopPropagation();
    const open = box.hidden;
    document.querySelectorAll("#page-realms .rl-popover").forEach((b) => { if (b !== box && !b.hidden) b.dispatchEvent(new Event("rl-close")); });
    set(open);
    if (open) { const f = box.querySelector("select, input, button"); if (f) f.focus(); }
  });
  box.addEventListener("rl-close", () => set(false));
  box.addEventListener("click", (ev) => ev.stopPropagation());
  box.addEventListener("keydown", (ev) => { if (ev.key === "Escape") { set(false); btn.focus(); } });
  document.addEventListener("click", () => { if (!box.hidden) set(false); });
  return set;
}
rlPopover("rl-filters-btn", "rl-filters");
const rlMenu = rlPopover("rl-more", "rl-menu");
invoke("p2p_flags").then((v) => {
  p2pHosting = !!(v && v.effective && v.effective.p2p_hosting);
  heroKeyOn = !!(v && v.effective && v.effective.herokey_sso);   // Hero Key sign-in on realms (docs/HERO-KEY-SSO.md)
}).catch(() => {});
const CORE_LABELS = { azerothcore: "AzerothCore", trinitycore: "TrinityCore", cmangos: "CMaNGOS", vmangos: "vMaNGOS", mangos: "MaNGOS", other: "Other server" };
$("rl-mine").addEventListener("change", () => renderRealms());
async function playRealm(r, btn) {
  if (playHeld()) return;   // a game started a moment ago (holdPlay): the rebuilt card's Play too
  if (r.official && !signedInFirst(() => playRealm(r, btn))) return;
  // the play bar follows the card: this realm, and the client its picker takes for it (the picked one when it can
  // play there, else one of the version it needs), so PLAY and the bar say the same afterwards
  if (r.official || r.address) pickRealm(r);
  const c = needClient("rl-status");
  if (!c) return;
  if (!clientFits(r, c)) return say("rl-status", wrongClientText(r, c), "warn");   // never another client by itself
  btn.disabled = true;
  let started = false;
  say("rl-status", `Starting ${c.name} on ${r.name}…`);
  try {
    // the realm's suggested loadout (loadouts.js), then its mods (gamemods.js): "This realm uses X, Y: Get them and play"
    if (window.LeylineLoadouts && !(await window.LeylineLoadouts.beforeJoin(r, c)).go) { say("rl-status", ""); btn.disabled = false; return; }
    const j = window.LeylineGameMods ? await window.LeylineGameMods.beforeJoin(r, c, (t, k) => say("rl-status", t, k)) : { go: true, leyline: false };
    if (!j.go) { say("rl-status", ""); btn.disabled = false; return; }
    if (window.LeylineAddons && !j.leyline) await window.LeylineAddons.beforePlay(c, { offer: !!r.official });   // up to date; offered on the official realm (addons.js)
    const msg = await invoke("play_realm", { client: c.id, slug: r.slug });
    started = true;
    if (window.LeylineAddons) window.LeylineAddons.afterPlay();
    holdPlay(null, true);
    say("rl-status", msg + (r.official ? "" : ` ${r.name} has its own accounts: log in with yours there`
      + (r.join_url ? ", or make one with Make an account." : ".")), "ok");
    await refresh();
    renderRealms();
  } catch (e) { sayErr("rl-status", e); }
  holdPlay(btn, started);
}

// "Sign in with your Hero Key" (the herokey_sso flag): the realm sets a one-time login for this player's account
// there; the client starts with the account name filled in, and the password is shown here to type (the client
// can't take one from outside). Nothing is saved.
let ssoClearT = 0;
async function playRealmHeroKey(r, btn) {
  if (playHeld()) return;
  const picked = needClient("rl-status");
  if (!picked) return;
  if (!clientFits(r, picked)) return say("rl-status", wrongClientText(r, picked), "warn");
  btn.disabled = true;
  say("rl-status", `Logging in to ${r.name} with your Hero Key…`);
  try {
    if (window.LeylineAddons) await window.LeylineAddons.beforePlay(picked, { offer: false });   // the Leyline addons up to date (addons.js)
    const l = await invoke("play_realm_herokey", { client: picked.id, slug: r.slug });
    if (window.LeylineAddons) window.LeylineAddons.afterPlay();
    const mins = Math.max(1, Math.round((l.valid_s || 600) / 60));
    if (l.auto) say("rl-status", `Started on ${r.name} as ${l.user}. The game signs you in by itself.`, "ok");
    else {
      const line = `Started on ${r.name} as ${l.user}. At the login screen type this one-time password: ${l.pass} `
        + `(it works for ${mins} minutes and is not saved).` + (l.auto_why ? ` Logging in by itself didn't work here: ${oops(l.auto_why, "the game's login screen didn't take it.")}` : "");
      say("rl-status", line, "ok");
      // the password leaves the screen once it stops working (review 4 Nov: it stayed until the next status line)
      clearTimeout(ssoClearT);
      ssoClearT = setTimeout(() => {
        if ($("rl-status").textContent === line) say("rl-status", `The one-time password for ${r.name} has expired. Log in with your Hero Key again for a new one.`);
      }, (l.valid_s || 600) * 1000);
    }
    holdPlay(null, true);
    await refresh();
    renderRealms();
  } catch (e) { sayErr("rl-status", e); }
  holdPlay(btn, false);
}

$("rl-refresh").addEventListener("click", () => loadRealms());
$("rl-back").addEventListener("click", async () => {
  rlMenu(false);
  const c = needClient("rl-status");
  if (!c) return;
  try { say("rl-status", await invoke("back_to_official", { client: c.id }), "ok"); await refresh(); renderRealms(); }
  catch (e) { sayErr("rl-status", e); }
});
$("rl-restore").addEventListener("click", async () => {
  rlMenu(false);
  const c = needClient("rl-status");
  if (!c) return;
  try { say("rl-status", await invoke("restore_realmlist", { client: c.id }), "ok"); await refresh(); renderRealms(); }
  catch (e) { sayErr("rl-status", e); }
});
$("rl-join").addEventListener("click", async () => {
  const c = needClient("rl-status");
  if (!c) return;
  const link = $("rl-invite").value.trim();
  if (!link) return say("rl-status", "Paste the invite link (or a friend's Den code) first.", "bad");
  try {
    const j = await invoke("join_invite", { client: c.id, link });
    // a friend's Den (host/litenet.rs): the game is already starting with your own login from the link
    // which way in it took (host/denways.rs): the same network, the friend's own address, or the relay
    const via = { lan: " on your shared network", own: " through your friend's own address", relay: " through the Leyline relay" }[j.way] || "";
    if (j.guest) say("rl-status", j.guest.auto ? `Joining your friend's Den${via}: the game signs you in by itself and opens at character select.`
      : `Joining your friend's Den${via}. At the login screen: account ${j.guest.account}, password ${j.guest.password}.`, "ok");
    else say("rl-status", `${j.name || "The realm"} is open for this PC; ${c.name} now points at ${j.address}. Press PLAY.`, "ok");
    $("rl-invite").value = "";
    await refresh();
    // does the host's world answer there? (hostfriends.js afterJoin: the friend's "what's happening" line)
    if (window.LeylineHostFriends) window.LeylineHostFriends.afterJoin(j, null, (t, k) => say("rl-status", t, k));
  } catch (e) {
    const x = window.LeylineHostFriends ? window.LeylineHostFriends.explainJoin(e) : null;
    if (x) say("rl-status", `${x.line} ${x.fix}`, "bad"); else sayErr("rl-status", e);
  }
});

// ------------------------------------------------------------------ Host
let kit = null;
const NO_KIT = { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
async function loadHost() {
  // no answer from the core: the page still opens on Start (the setup), never an empty tab
  try { kit = (await invoke("host_info")) || Object.assign({}, NO_KIT); } catch (e) { sayErr("host-status-line", e); kit = Object.assign({}, NO_KIT); }
  renderHost();
}

function hostRealmId() {
  if (kit && kit.realm) return kit.realm.realm_id;
  const s = kit && kit.seeds.find((x) => x.in_kit);
  return s ? s.realm_id : null;
}

function renderHost() {
  $("host-kit").textContent = kit.dir
    ? (kit.is_kit ? `Host pack: ${kit.dir}` : `Chosen folder: ${kit.dir}`)
    : "No Host pack folder chosen yet.";
  const probs = $("host-problems");
  probs.hidden = !kit.problems.length;
  probs.replaceChildren(...kit.problems.map((p) => el("li", "", p)));
  const seeds = $("host-seeds");
  seeds.replaceChildren();
  const want = state.windows ? "bat" : "sh";
  for (const s of kit.seeds) {
    if (s.kind !== want) continue;
    const card = el("div", "card");
    const top = el("div", "top");
    top.append(el("span", "name", s.file_name), el("span", "build", s.in_kit ? "In the Host pack" : "In Downloads"));
    const acts = el("div", "acts");
    if (s.in_kit) {
      const b = el("button", "btn small", "Start this realm");
      b.disabled = !kit.is_kit;
      b.addEventListener("click", () => hostStart(s.file_name));
      acts.append(b);
    } else {
      const b = el("button", "btn small", "Copy into the Host pack");
      b.disabled = !kit.is_kit;
      b.addEventListener("click", async () => {
        try { kit = await invoke("host_copy_seed", { fileName: s.file_name }); renderHost(); say("host-status-line", "Copied: now start it.", "ok"); }
        catch (e) { sayErr("host-status-line", e); }
      });
      acts.append(b);
    }
    const meta = el("div", "meta");
    meta.append(el("span", "", `Your realm file from the website (realm ${s.realm_id}). It holds your realm's key: keep it to yourself.`));
    card.append(top, meta, acts);
    seeds.append(card);
  }
  $("host-start").disabled = !kit.is_kit;
  $("host-start").textContent = kit.realm ? `Start ${kit.realm.name || "my realm"}` : "Start my realm";
  const rid = hostRealmId();
  $("host-card").hidden = !rid;
  if (rid) {
    $("host-name").textContent = (kit.realm && kit.realm.name) || "Your realm";
    hostStatus();
  }
  document.dispatchEvent(new CustomEvent("leyline:kit", { detail: kit }));   // hostconsole.js: the console or the setup start
}

async function hostStart(seed) {
  say("host-status-line", "Starting the Host pack…");
  try { say("host-status-line", await invoke("host_start", seed ? { seed } : {}), "ok"); }
  catch (e) { sayErr("host-status-line", e); }
  setTimeout(hostStatus, 20000);
}

async function hostStatus() {
  const rid = hostRealmId();
  if (!rid) return;
  const set = (cls, text, detail) => {
    $("host-state").className = "srv-state " + cls;
    $("host-state-text").textContent = text;
    $("host-detail").textContent = detail;
  };
  set("", "CHECKING…", "");
  try {
    const s = await invoke("host_status", { realmId: rid });
    const counts = [s.players != null ? `Players: ${s.players}` : "", s.bots != null ? `Bots: ${s.bots}` : ""].filter(Boolean);
    if (s.online) {
      set("is-online", s.listed ? "ONLINE · ON THE REALM LIST" : "ONLINE",
        [...counts, s.visibility === "private" ? "Private: friends join with your invite link" : "", s.address ? `Realmlist: ${s.address}` : ""].filter(Boolean).join(" · "));
    } else {
      set("is-offline", "OFFLINE", "The Leyline network knows your realm, but your PC isn't connected. Start it above.");
    }
  } catch (e) {
    set("is-partial", "NOT SEEN YET", oops(e, "The Leyline network hasn't seen your realm yet."));
  }
}

$("host-pick").addEventListener("click", async () => {
  const p = await invoke("pick_host_kit").catch(() => null);
  if (!p) return;
  try { kit = await invoke("set_host_kit", { path: p }); renderHost(); }
  catch (e) { sayErr("host-status-line", e); }
});
$("host-start").addEventListener("click", () => hostStart(null));
$("host-refresh").addEventListener("click", () => loadHost());

// ------------------------------------------------------------------ the launcher's own updates
// The core checks at start and every 30 minutes and downloads in the background (app.rs). The UI only shows it:
// One calm line in the title bar (owner 31 Oct): "Update ready — restart to finish" and Restart. Never a restart by
// itself (owner 3 Nov: "make the restart to update button optional and appear and not auto"). Ignored, the update
// finishes by itself the next time Leyline opens ("Finishing the update…"); never while a game Leyline started runs.
let upd = null;
function updLine(u) {
  const i = (u && u.info) || {};
  if (!u) return null;
  if (u.finishing) return { text: "Finishing the update…", go: null };
  if (u.ready) return u.game_running
    ? { text: "Update ready — restart after your game", go: "Restart", disabled: true, title: "Your game is open. Restart after you close it, or the update finishes the next time you open Leyline." }
    : { text: "Update ready — restart to finish", go: "Restart", title: "Restart Leyline to finish the update. Or leave it: it finishes the next time you open Leyline." };
  if (u.mode === "link" && i.newer) return { text: "A new Leyline is out", go: "Get it", title: "Open the download page" };
  return null;
}
function renderUpdate() {
  if (!upd) return;
  const i = upd.info || {};
  $("upd-channel").value = upd.channel || "stable";
  const line = updLine(upd);
  $("tb-update").hidden = !line;
  if (line) {
    $("tb-update-text").textContent = line.text;
    $("tb-update-go").hidden = !line.go;
    $("tb-update-go").textContent = line.go || "";
    $("tb-update-go").disabled = !!line.disabled;
    $("tb-update").title = line.title || "";
    $("tb-update").classList.toggle("busy", !!upd.finishing);
  }
  $("upd-restart").hidden = !upd.ready;
  $("upd-restart").disabled = !!upd.game_running;
  $("upd-open").hidden = !i.url;
  let text = "", kind = "";
  if (upd.finishing) { text = "Finishing the update. Leyline opens again in a moment."; kind = "ok"; }
  else if (upd.checking) text = "Checking…";
  else if (upd.downloading) text = "Downloading the update in the background…";
  else if (upd.ready) { text = "Update ready. Press Restart when you like, or it finishes the next time you open Leyline." + (upd.game_running ? " Your game is open: Leyline waits until you close it." : ""); kind = "ok"; }
  else if (i.newer) { text = `A new Leyline is out (${i.latest}). ${upd.note || ""}`; kind = "ok"; }
  else if (i.latest) { text = `You have the newest Leyline (${i.current}).` + (i.from_cache ? " (offline: from the last check)" : ""); kind = "ok"; }
  else if (upd.checked_at) { text = i.error ? oops(i.error, OOPS["upd-status"]) : "No launcher releases on this channel yet."; kind = i.error ? "warn" : ""; }
  if (upd.error && !upd.ready && !upd.downloading) { text += (text ? " " : "") + oops(upd.error, "The download didn't finish: Leyline tries again later."); kind = "warn"; }
  say("upd-status", text, kind);
}
// Every minute; every second while a check, a download or the finish is going, and at start until the first check has
// answered (at most half a minute; review 4 Nov: it asked every second for the whole first half minute regardless).
const updStarted = Date.now();
let updTimer = null;
async function loadUpdateState() {
  clearTimeout(updTimer);
  try { upd = await invoke("launcher_update_state"); renderUpdate(); } catch (_) { /* preview page */ }
  const fast = (upd && (upd.checking || upd.downloading || upd.finishing)) || (Date.now() - updStarted < 30000 && !(upd && upd.checked_at));
  updTimer = setTimeout(loadUpdateState, fast ? 1000 : 60000);
}
async function checkUpdate() {
  say("upd-status", "Checking…");
  try { upd = await invoke("launcher_update_check"); renderUpdate(); }
  catch (e) { sayErr("upd-status", e); }
}
async function restartToUpdate() {
  if (!upd) return;
  if (!upd.ready) { if (upd.info && upd.info.url) openUrl(upd.info.url); return; }
  try { await invoke("launcher_update_restart"); }
  catch (e) { sayErr("upd-status", e); sayErr("play-status", e); await loadUpdateState(); }
}
$("upd-check").addEventListener("click", checkUpdate);
$("upd-restart").addEventListener("click", restartToUpdate);
$("upd-open").addEventListener("click", () => { if (upd && upd.info && upd.info.url) openUrl(upd.info.url); });
$("tb-update-go").addEventListener("click", restartToUpdate);
$("upd-channel").addEventListener("change", async () => {
  say("upd-status", "Saved. Checking the new channel…");
  try { upd = await invoke("set_update_channel", { channel: $("upd-channel").value }); renderUpdate(); }
  catch (e) { sayErr("upd-status", e); }
});

// ------------------------------------------------------------------ for Home's gallery and rows (home.js, loaded after this)
// Home's "Join <HANGOUT.name>" slide: select the Hangout and press PLAY. With a 3.3.5a client, that client on
// the Hangout; without one, browser play (the Hangout plays in the browser, nothing to install).
function playHangout() {
  $("play-realm").value = "official";
  const c = currentClient();
  const own = c && c.build === "3.3.5a" ? c : (state && state.settings.clients.find((x) => x.build === "3.3.5a"));
  if (own) {
    $("play-client").value = own.id;
    if (mode === "browser") setMode("online");
  } else if (mode !== "browser") setMode("browser");
  realmChanged();
  saveSelection();
  $("play-form").requestSubmit();
}

// signed in or out: the bar's line was about the other state (a signed-out note): it goes, the bar is drawn again
document.addEventListener("leyline:me", () => {
  if (!state) return;
  if (/log in/i.test($("play-status").textContent)) { say("play-status", ""); $("play-status").title = ""; }
  renderPlayBar();
});

window.LeylineApp = {
  featureFor,   // the title bar's search (tests)
  news: () => news, realms: () => realmsCache, catalog: () => catalog, mode: () => mode, setMode, openLink, playHangout, HANGOUT,
  realmIcon, realmVersion, hiddenBuild, nameHue,
  state: () => state,   // addons.js: the clients
  noteInstalled,   // addons.js: the Leyline addons just went in (Installed opens on Addons)
  refresh: () => refresh().then(refreshStatus).catch(() => { /* as it was */ }),   // den.js: after Play solo, the bar shows Your Den
  pickRealm, playRealm, loadRealms, showRealm: showLinkedRealm,   // realmlink.js: paste to add, Live now
  realmTab, setRealmsTab,   // realmlink.js: a pasted community realm opens the Community tab
  modsForSingle, selectClient,   // spmods.js: Get mods for single player (Discover on its game folder)
};

// ------------------------------------------------------------------ start
renderNews();
renderModTiles();
refresh().then(() => {
  setMode(mode);
  catalogLoading = loadCatalog(true).finally(() => { catalogLoading = null; });
  handleDeepLink();   // a friend's invite or a realm link doesn't wait for the mods list
  try {
    const ev = window.__TAURI__ && window.__TAURI__.event;
    if (ev && ev.listen) ev.listen("deep-link", () => { handleDeepLink(); });
  } catch (_) { /* no events: links still come at start */ }
  loadRealms(true);
  loadUpdateState();
  loadNews();
  readKit();   // the realm picker's My Den: hosted on this PC or not
  setupDrop();
  let page = "play";
  try { page = localStorage.getItem("leyline.page") || "play"; } catch (_) { /* fine */ }
  show(page);   // an unknown or old page id: resolvePage maps it, or Home
}).catch((e) => {
  document.body.prepend(el("p", "status bad", oops(e, "Leyline couldn't start. Close it and open it again, or ask on the Discord.")));
});
