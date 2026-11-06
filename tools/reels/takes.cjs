// The reel kit's launcher and website takes (tools/reels/scripts/takes.json, source "launcher-fake", and "site" /
// "phone" with offline: true): the launcher's own pages (launcher/ui) with the test core, and the website's own pages
// (site_pages.py renders them from stream/wrath_portal.py), played by a soft cursor and recorded as screencast frames
// with their times. tools/reels/edit.py turns each folder into a steady 30 fps take the first time it is used.
//
//   node tools/reels/takes.cjs [--out tools/reels/takes] [take ...]
//
// Each launcher take is recorded twice: wide (1920x1080 frames: <take>/) for 16:9 videos and tall (1080x1620:
// <take>@reel/) for the 9:16 reels. The shared bits (the test core, the cursor, eased moves, the recorder) are
// tools/video/lib.cjs; this file adds what the reels need on top of its core (logging in, adding a game folder,
// Single player, the HD pack) and the scenes. Every scene checks the screen for the names we never show. Nothing
// leaves the machine: every https request is answered from the repo or refused.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { chromium } = require('playwright');
// slow motion: 4 gives about 30 frames a second of movement from headless Chromium's screencast (lib.cjs SLOW)
if (!process.env.SLOW) process.env.SLOW = '4';
const L = require('../video/lib.cjs');

const HERE = __dirname;
const ROOT = path.resolve(HERE, '..', '..');
const ART = path.join(ROOT, 'stream', 'www-files', 'art');
const WWW = path.join(ROOT, 'stream', 'www-files');
const args = process.argv.slice(2);
const outAt = args.indexOf('--out');
const OUT = path.resolve(outAt >= 0 ? args[outAt + 1] : path.join(HERE, 'takes'));
const ONLY = args.filter((a, i) => !a.startsWith('--') && (outAt < 0 || i !== outAt + 1));
const CAT = JSON.parse(fs.readFileSync(path.join(HERE, 'scripts', 'takes.json'), 'utf8')).takes;

const WIDE = { width: 1280, height: 720, dpr: 1.5 };     // 1920x1080 frames
const TALL = { width: 800, height: 1200, dpr: 1.35 };    // 1080x1620 frames, set into the 9:16 reel (at 720 wide the play bar cuts the realm's name)
const PHONE = { width: 432, height: 768, dpr: 2.5 };     // 1080x1920 frames: a phone, already 9:16
const SITE_WIDE = { width: 1280, height: 720, dpr: 1.5 };
const TEST_KEY = 'ovk1.TestTestAa.' + 'k'.repeat(43);   // the launcher tests' fake Hero Key: the fake core takes it

// ---------------------------------------------------------------- the core, with what the reels add (runs in the page)
// lib.cjs FAKE answers most of the launcher; this wraps it for: logging in (signed out until a key is pasted), adding a
// game folder (an empty list to start with), Single player's own profile, the addons offer, and the HD pack.
const EXTRA = (o) => {
  const base = window.__TAURI__.core.invoke;
  window.__signed = !!o.signed;
  window.__clients = null;   // null: lib.cjs's five folders; [] when the take starts with none
  if (o.noClients) window.__clients = [];
  const me = { pid: 'kkkkkkkkkk', name: 'Kestrel', tag: '1290' };
  window.__TAURI__.core.invoke = async (cmd, a) => {
    switch (cmd) {
      case 'social_status': return window.__signed;
      case 'social_ways': return { herokey: true, google: true, discord: false };
      case 'social_key_signin':
        await new Promise((r) => setTimeout(r, 500));
        window.__signed = true; return { ok: true, me };
      case 'social_signout': window.__signed = false; return null;
      case 'social_call': if (!window.__signed) throw 'signed_out'; return base(cmd, a);
      case 'get_state': {
        const s = await base(cmd, a);
        if (window.__clients) { s.settings.clients = JSON.parse(JSON.stringify(window.__clients)); s.settings.selected_client = window.__clients[0] ? window.__clients[0].id : null; }
        return s;
      }
      case 'pick_folder': return window.__nextFolder || null;
      case 'add_client': {
        await new Promise((r) => setTimeout(r, 450));
        const p = { id: 'c335a-9', name: 'My Wrath', path: a.path, build: '3.3.5a', realmlist: '', locale: 'enUS' };
        window.__clients = (window.__clients || []).concat([p]);
        return { profile: p, detection: { build: '3.3.5a', build_label: '3.3.5a (12340)', detected_by: 'exe_version', complete: true, missing: [] } };
      }
      case 'check_client': return { complete: true, missing: [], exe_version: '3.3.5.12340' };
      case 'select': case 'rename_client': case 'guide_seen': case 'set_count_installs': return null;
      case 'client_seed_state': return { active: false, name: '', nodes: 0, uploaded: 0, up_speed: 0 };
      case 'client_import_state': return { phase: 'idle', done: 0, total: 0, message: '' };
      case 'addons_should_offer': return false;
      case 'addons_status': return { pack: null, from_cache: false, error: null, prefs: { auto_update: true, dont_ask: true }, clients: [] };
      case 'addons_auto_update': return [];
      case 'looks_state': return { looks: [], on: [], changed: false };
      case 'listing_mine': return { ok: true, listings: [] };
      case 'news_list': return { posts: [], from_cache: false };
      case 'social_key_found': return null;   // no Hero Key file on this PC: the take pastes one
      case 'testing_get': return { client_torrents: false, share: false };
      case 'loadouts_status': return { loadouts: [], active: null, installed: [], drift: [] };
      case 'gfx_get': return { values: {}, backup: false, presets: [] };
      // Single player: its own profile offered by the Host pack (lib.cjs offers only "main")
      case 'host_profiles': if (o.single) return [['main', 'solo'].map((id) => ({ id, label: id, version: '3.3.5a', core: 'azerothcore', in_kit: true,
        clients: [{ id: 'c335a-1', name: 'Northrend', path: 'C:\\Games\\Northrend' }], note: null })), null]; return base(cmd, a);
      case 'host_info': if (o.single) { const r = await base(cmd, a); r.realm = null; return r; } return base(cmd, a);
      // the Host page's flow (hostflow.js: who it is for, a template, the settings, Host)
      case 'host_flow_apply': window.__applied = a; return { profile: 'main', visibility: a.visibility, mode_name: 'x', lines: [] };
      case 'host_flow_current': return null;
      case 'host_plan_ask': case 'host_plan_take': return null;
      case 'host_stop': return 'Stopped.';
      default: return base(cmd, a);
    }
  };
};

// The mods list the HD take browses: a few addons and mods, and the HD pack credited to Project Reforged (owner 4 Nov:
// credited as Project Reforged, linked to their page, with their permission). Shown in a test core only: what is in
// the real catalog stays the owner's call (pending until he approves).
function catalog() {
  const P = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: name + '.', category: 'addons', builds: ['3.3.5a'],
    author: 'Bramble', author_url: '', licence: 'MIT', repo: '', page: '', credits: `${name} by Bramble · MIT`, stars: null, last_push: '',
    version: '1.0.0', kind: 'ui', permissions: [], on_wrath: false, download: 'https://leylinelauncher.com/wrath/mods/x/download', sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
  return { categories: [{ id: 'graphics', name: 'Looks and HD', line: '', icon: '' }, { id: 'addons', name: 'Addons and UI', line: '', icon: '' },
    { id: 'sandbox', name: 'Sandbox and game modes', line: '', icon: '' }],
  projects: [
    P('mod:project-reforged', 'Project Reforged HD', { category: 'graphics', kind: 'patch', author: 'Project Reforged', author_url: 'https://projectreforged.github.io/',
      page: 'https://projectreforged.github.io/', credits: 'Project Reforged HD by Project Reforged, used with their permission',
      description: 'Sharper characters, creatures and places: the HD pack by Project Reforged.', size: 4200000000 }),
    P('mod:golfcart', 'Golf Cart', { category: 'sandbox', kind: 'both', description: 'A minigolf course you can play with friends.' }),
    P('mod:tidy-bags', 'Tidy Bags', { description: 'Keeps your bags tidy.' }),
    P('mod:quest-helper', 'Quest Helper', { description: 'Where to go next, on the map.' }),
  ] };
}

// What the launcher fetches from the site (Home's gallery, the backdrop), from the repo's own art (tools/video/routes.cjs
// does the same; its news picture was renamed, so a missing picture here falls back to the backdrop, never a crash)
const HOME = { slides: { hangout: [{ file: 'leyline-shot-dalaran.webp', alt: '', credit: '' }], host: [{ file: 'leyline-shot-city.webp', alt: '', credit: '' }],
  browser: [{ file: 'leyline-shot-start.webp', alt: '', credit: '' }], news: [{ file: 'leyline-shot-project-reforged.webp', alt: '', credit: 'Project Reforged' }] } };
async function routes(page) {
  const bg = path.join(L.UI, 'art', 'leyline-bg.webp');
  await page.route(/^https:\/\//, (r) => {
    const u = r.request().url();
    if (/\/wrath\/art\/home\/home\.json$/.test(u)) return r.fulfill({ contentType: 'application/json', headers: { 'Access-Control-Allow-Origin': '*' }, body: JSON.stringify(HOME) });
    const m = /\/wrath\/art\/(?:home\/)?([a-z0-9-]+\.webp)$/.exec(u);
    if (m) { const f = path.join(ART, m[1]); return r.fulfill({ path: fs.existsSync(f) ? f : bg, contentType: 'image/webp' }); }
    if (/leyline-bg|backdrop/.test(u)) return r.fulfill({ path: bg, contentType: 'image/webp' });
    return r.abort();
  });
}

// ---------------------------------------------------------------- the website's own pages (site_pages.py), offline
let SITE = null;
function sitePages() {
  if (SITE) return SITE;
  const dir = path.join(OUT, '_site');
  const py = process.env.PYTHON || (process.platform === 'win32' ? 'py' : 'python3');
  execFileSync(py, (process.platform === 'win32' && py === 'py' ? ['-3'] : []).concat(['-I', path.join(HERE, 'site_pages.py'), dir]), { stdio: 'inherit' });
  SITE = { dir, index: JSON.parse(fs.readFileSync(path.join(dir, 'pages.json'), 'utf8')) };
  return SITE;
}
async function siteRoutes(ctx) {
  const s = sitePages();
  await ctx.route(/^https?:\/\//, (r) => {
    const u = new URL(r.request().url());
    if (u.hostname !== 'leylinelauncher.com') return r.abort();
    const p = u.pathname.replace(/\/$/, '');
    if (s.index[p]) return r.fulfill({ path: path.join(s.dir, s.index[p]), contentType: 'text/html; charset=utf-8' });
    const art = /^\/wrath\/art\/([A-Za-z0-9._-]+)$/.exec(p);
    if (art && fs.existsSync(path.join(ART, art[1]))) return r.fulfill({ path: path.join(ART, art[1]) });
    const top = /^\/wrath\/([A-Za-z0-9._-]+\.(?:css|js|webp|png|svg|woff2))$/.exec(p);
    if (top && fs.existsSync(path.join(WWW, top[1]))) return r.fulfill({ path: path.join(WWW, top[1]) });
    if (r.request().resourceType() === 'document') return r.fulfill({ status: 204, body: '' });   // a link followed: stay put
    return r.fulfill({ status: 404, body: '' });
  });
}

// ---------------------------------------------------------------- opening a page
let browser;
const browsers = {};
async function launcherPage(size, o = {}) {
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: size.width, height: size.height }, deviceScaleFactor: size.dpr });
  const page = await ctx.newPage();
  page.on('pageerror', (e) => console.error('  pageerror', String(e).slice(0, 200)));
  await routes(page);
  const opts = Object.assign({ modes: L.modes(), catalog: catalog(), signed: true }, o);
  await page.addInitScript(L.FAKE, opts);
  await page.addInitScript(EXTRA, opts);
  await page.addInitScript(L.CURSOR, { scale: size.dpr > 1.3 ? 1.1 : 1, x: size.width * 0.62, y: size.height * 0.82 });
  await page.goto(L.INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.__calls.some(([c]) => c === 'realms_list'));
  await page.waitForTimeout(1500);
  await L.setCursor(page, size.width * 0.62, size.height * 0.82);
  return { ctx, page };
}
async function sitePage(size, url, mobile) {
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: size.width, height: size.height }, deviceScaleFactor: size.dpr,
    isMobile: !!mobile, hasTouch: !!mobile });
  await siteRoutes(ctx);
  const page = await ctx.newPage();
  await page.addInitScript(L.CURSOR, { scale: mobile ? 1.6 : 1.1, x: size.width * 0.6, y: size.height * 0.8 });
  await page.goto('https://leylinelauncher.com' + url, { waitUntil: 'load' });
  await page.waitForTimeout(1200);
  await L.setCursor(page, size.width * 0.6, size.height * 0.8);
  return { ctx, page };
}
const nav = (p) => `.side .nav[data-page="${p}"]`;
async function scroll(page, dy, ms = 900) {
  const t0 = Date.now(), dur = ms * L.SLOW;
  let done = 0;
  for (;;) {
    const t = Math.min(1, (Date.now() - t0) / dur);
    const e = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
    const want = Math.round(dy * e);
    if (want !== done) { await page.mouse.wheel(0, want - done); done = want; }
    if (t >= 1) break;
    await page.waitForTimeout(30);
  }
}
async function scrollTo(page, sel, ms = 1200, above = 120) {
  const y = await page.evaluate((s) => { const e = document.querySelector(s); return e ? e.getBoundingClientRect().top : 0; }, sel);
  if (Math.abs(y - above) > 20) await scroll(page, y - above, ms);
}

// ---------------------------------------------------------------- the scenes (marks: takes.json "marks")
const SCENES = {
  // signed out: Log In, a Hero Key pasted, the signed-in moment, Home
  login: { o: { signed: false }, run: async (page, rec, size) => {
    await L.wait(page, 600);
    if (!(await page.evaluate(() => window.LeylineGate && window.LeylineGate.isOpen()))) await L.click(page, '#hf-signin', { ms: 800 });
    await page.waitForSelector('#gate-keylink');
    rec.mark('gate');
    await L.wait(page, 1000);
    await L.click(page, '#gate-keylink', { ms: 800 });
    await page.waitForSelector('#gate-key');
    await L.click(page, '#gate-key', { ms: 600, after: 200 });
    await page.fill('#gate-key', TEST_KEY);   // a paste
    rec.mark('key');
    await L.wait(page, 700);
    await L.click(page, '#gate-keyform button[type="submit"]', { ms: 700 });
    // the signed-in moment (picture, name#tag, Continue); the dialog may go on by itself
    await page.waitForSelector('#gate-in, #hf-me-btn', { state: 'visible' });
    rec.mark('welcome');
    await L.wait(page, 1300);
    if (await page.$('#gate-go:visible')) await L.click(page, '#gate-go', { ms: 700 }).catch(() => {});
    await page.waitForSelector('#hf-me-btn');
    rec.mark('home');
    await L.wait(page, 1800);
  } },
  'play-official': { run: async (page, rec) => {
    await L.wait(page, 600);
    await L.click(page, '#pb-realm-btn', { ms: 800 });
    rec.mark('picker');
    await L.wait(page, 900);
    await L.click(page, '#pb-rmenu >> text=Wrath Unleashed', { ms: 600 });
    rec.mark('picked');
    await L.wait(page, 500);
    await L.click(page, '#play-go', { ms: 800 });
    rec.mark('play');
    await L.wait(page, 2200);
  } },
  // Realms: the community list; play Frost Haven; back to Wrath Unleashed and play
  switch: { run: async (page, rec, size) => {
    await L.wait(page, 500);
    await L.click(page, nav('realms'), { ms: 800 });
    await L.wait(page, 500);
    await L.click(page, '#rl-tab-community', { ms: 700 });
    rec.mark('community');
    await page.waitForFunction(() => /Frost Haven/.test(document.getElementById('rl-pane').textContent));
    await L.wait(page, 900);
    await L.hover(page, '#rl-pane .realm-card:has-text("Frost Haven")', 700).catch(() => {});
    await L.wait(page, 700);
    await L.click(page, nav('play'), { ms: 800 });
    await L.click(page, '#pb-realm-btn', { ms: 700 });
    await L.click(page, '#pb-rmenu >> text=Frost Haven', { ms: 600 });
    rec.mark('picked-community');
    await L.wait(page, 400);
    await L.click(page, '#play-go', { ms: 700 });
    rec.mark('play-community');
    await L.wait(page, 1800);
    await L.click(page, '#pb-realm-btn', { ms: 800 });
    await L.click(page, '#pb-rmenu >> text=Wrath Unleashed', { ms: 600 });
    rec.mark('official');
    await L.wait(page, 400);
    await L.click(page, '#play-go', { ms: 700 });
    rec.mark('play-official');
    await L.wait(page, 2000);
  } },
  single: { o: { single: true }, run: async (page, rec, size) => {
    await L.wait(page, 500);
    await L.click(page, nav('single'), { ms: 800 });
    await page.waitForFunction(() => document.querySelector('#page-single.on') && document.querySelector('#sp-dock') && !/Checking/.test(document.querySelector('#sp-dock').textContent));
    rec.mark('page');
    await L.wait(page, 1200);
    await L.hover(page, '#sp-step-folder', 700).catch(() => {});
    rec.mark('steps');
    await L.wait(page, 700);
    await L.hover(page, '#sp-step-engine', 600).catch(() => {});
    await L.wait(page, 600);
    await L.click(page, '#sp-start', { ms: 800 });
    rec.mark('start');
    await page.waitForSelector('#sp-prog', { timeout: 15000 }).catch(() => {});
    await L.wait(page, 2400);
  } },
  'mods-hd': { run: async (page, rec, size) => {
    await L.wait(page, 500);
    await L.click(page, nav('mods'), { ms: 800 });
    await L.wait(page, 400);
    await L.click(page, '#tab-mods-discover', { ms: 600 });
    await L.click(page, '#discover-tabs .ly-tab[data-tab="mods"]', { ms: 500 }).catch(() => {});
    rec.mark('mods');
    await page.waitForFunction(() => /Project Reforged/.test(document.getElementById('pane-discover').textContent), null, { timeout: 15000 });
    await L.wait(page, 900);
    const card = '#pane-discover :is(.card, .mod, .mk-card, article, li):has-text("Project Reforged HD") >> nth=-1';
    await L.hover(page, card, 800).catch(() => {});
    rec.mark('hd');
    await L.wait(page, 1300);
    await L.click(page, '#pane-discover :is(.card, .mod, .mk-card, article, li):has-text("Project Reforged HD") >> button:has-text("Install") >> nth=0', { ms: 700 })
      .catch(() => L.click(page, '#pane-discover button:visible:has-text("Install") >> nth=0', { ms: 700 }));
    rec.mark('install');
    await L.wait(page, 2200);
  } },
  'den-open': { run: async (page, rec) => {
    await L.wait(page, 600);
    await L.click(page, '#pb-realm-btn', { ms: 800 });
    rec.mark('picker');
    await L.wait(page, 800);
    await L.click(page, '#pb-rmenu >> text=Your Den', { ms: 600 });
    await page.waitForSelector('#page-den.on');
    rec.mark('den');
    await L.wait(page, 1300);
    await L.click(page, '#yd button:has-text("Play") >> nth=0', { ms: 800 });
    rec.mark('play');
    await L.wait(page, 2000);
  } },
  // Host a realm (hostflow.js): Public, a template, a name, Host; the running realm's card; listed under Community
  host: { run: async (page, rec, size) => {
    await L.wait(page, 500);
    await L.click(page, nav('host'), { ms: 800 });
    await page.waitForSelector('#hst .hst-vis');
    rec.mark('host');
    await L.wait(page, 1000);
    await L.click(page, '.hst-vis-b[data-vis="public"]', { ms: 800 });
    await L.wait(page, 500);
    await L.moveTo(page, size.width * 0.6, size.height * 0.6, 500);
    await scrollTo(page, '.hst-tpl[data-tpl="vanilla-plus"]', 900, size.height * 0.3);
    await L.click(page, '.hst-tpl[data-tpl="vanilla-plus"]', { ms: 700 });
    rec.mark('form');
    await page.waitForSelector('.hst-s3 .hst-group');
    await L.wait(page, 900);
    await L.moveTo(page, size.width * 0.6, size.height * 0.6, 500);
    await scrollTo(page, '#hst-name', 1600, size.height * 0.35);
    await L.click(page, '#hst-name', { ms: 700, after: 150 });
    await page.keyboard.press('Control+A');
    await L.typeSlow(page, 'Ember Vale', 75);
    await L.click(page, '#hst-gm', { ms: 600, after: 150 });
    await page.keyboard.press('Control+A');
    await L.typeSlow(page, 'KESTREL', 60);
    rec.mark('typed');
    await L.wait(page, 400);
    await L.click(page, '#hst-go', { ms: 700 });
    rec.mark('start');
    await page.waitForSelector('.hst-live', { timeout: 30000 });
    rec.mark('live');
    await L.wait(page, 500);
    await L.hover(page, '.hst-live-st', 700).catch(() => {});
    await L.wait(page, 1500);
    await L.click(page, nav('realms'), { ms: 800 });
    rec.mark('realms');
    await L.click(page, '#rl-tab-community', { ms: 600 }).catch(() => {});
    await L.wait(page, 1000);
    await L.hover(page, '.realm-card:has-text("Ember Vale")', 800);
    rec.mark('row');
    await L.wait(page, 2200);
  } },
  // no game folder yet: Find your game files, a folder picked, added, PLAY
  'add-folder': { o: { noClients: true }, run: async (page, rec) => {
    await L.wait(page, 600);
    await L.click(page, '#pb-client', { ms: 800 });
    await L.wait(page, 500);
    await L.click(page, '#pb-menu >> text=Find your game files', { ms: 700 });
    await page.waitForSelector('#page-findfiles.on');
    rec.mark('find');
    await L.wait(page, 1600);
    await page.evaluate(() => { window.__nextFolder = 'C:\\Games\\Wrath'; });
    await L.click(page, '#ff-pick', { ms: 800 });
    rec.mark('picked');
    await page.waitForFunction(() => /Added/.test((document.getElementById('cl-status') || {}).textContent || ''), null, { timeout: 10000 });
    rec.mark('added');
    await L.wait(page, 1800);
    const notNow = await page.$('#ad-dlg[open] button:has-text("Not now")');
    if (notNow) { await L.click(page, '#ad-dlg button:has-text("Not now")', { ms: 600 }); }
    await L.click(page, nav('play'), { ms: 800 });
    await L.wait(page, 600);
    await L.click(page, '#play-go', { ms: 800 });
    rec.mark('play');
    await L.wait(page, 2000);
  } },
  // the website (offline: its own pages)
  scroll: { site: true, run: async (page, rec, size) => {
    rec.mark('top');
    await L.wait(page, 1400);
    await L.moveTo(page, size.width * 0.55, size.height * 0.55, 700);
    await scroll(page, size.height * 0.9, 1800);
    rec.mark('scrolled');
    await L.wait(page, 900);
    await scroll(page, size.height * 0.9, 1800);
    await L.wait(page, 1500);
  } },
  queue: { site: true, run: async (page, rec, size) => {
    rec.mark('line');
    await L.hover(page, '#q-pos', 900).catch(() => {});
    await L.wait(page, 1800);
    await scrollTo(page, '#q-skip-h', 1400, size.height * 0.15);
    rec.mark('skip');
    await L.wait(page, 600);
    await L.hover(page, '.q-skip a >> nth=0', 700).catch(() => {});
    await L.wait(page, 900);
    await L.hover(page, '.q-skip a >> nth=1', 600).catch(() => {});
    await L.wait(page, 1500);
  } },
  'phone-play': { site: true, mobile: true, run: async (page, rec, size) => {
    rec.mark('top');
    await L.wait(page, 1500);
    await scrollTo(page, '#st-play', 1300, size.height * 0.3);
    await L.wait(page, 400);
    rec.mark('play');
    await L.click(page, '#st-play', { ms: 700, after: 600 });
    await L.wait(page, 1600);
  } },
};

// ---------------------------------------------------------------- the plan: each take, at each size
function plan() {
  const out = [];
  for (const [id, t] of Object.entries(CAT)) {
    if (ONLY.length && !ONLY.includes(id)) continue;
    if (t.source === 'launcher-fake') {
      out.push({ id, dir: id, scene: t.scene, size: WIDE });
      out.push({ id, dir: id + '@reel', scene: t.scene, size: TALL });
    } else if ((t.source === 'site' || t.source === 'phone') && t.offline) {
      if (t.source === 'phone') out.push({ id, dir: id, scene: t.flow, size: PHONE, url: t.url });
      else { out.push({ id, dir: id, scene: t.flow, size: SITE_WIDE, url: t.url }); out.push({ id, dir: id + '@reel', scene: t.flow, size: PHONE, url: t.url, mobile: true }); }
    }
  }
  return out;
}

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  let failed = 0;
  for (const p of plan()) {
    const sc = SCENES[p.scene];
    if (!sc) { console.error(p.id, 'no scene called', p.scene); failed++; continue; }
    browser = browsers[p.size.dpr] = browsers[p.size.dpr] || await chromium.launch({ args: ['--force-device-scale-factor=' + p.size.dpr] });
    const { ctx, page } = sc.site ? await sitePage(p.size, p.url, p.mobile || sc.mobile) : await launcherPage(p.size, sc.o || {});
    const dir = path.join(OUT, p.dir);
    const rec = await L.record(page, dir);
    let ok = true;
    try {
      await sc.run(page, rec, p.size);
      await L.brandCheck(page, p.dir);
    } catch (e) {
      ok = false; failed++; console.error(p.dir, 'FAILED', String(e).split('\n')[0]);
      await page.screenshot({ path: path.join(OUT, '_failed-' + p.dir + '.png') }).catch(() => {});   // what was on screen
    }
    const n = await rec.stop().catch(() => { ok = false; return 0; });
    const unknown = sc.site ? [] : await page.evaluate(() => [...new Set(window.__unknown || [])]);
    console.log(p.dir, ok ? 'OK' : 'FAILED', n, 'frames', unknown.length ? '(core asked for: ' + unknown.join(' ') + ')' : '');
    if (!ok) fs.rmSync(dir, { recursive: true, force: true });   // never a half take in a reel
    await ctx.close().catch(() => {});
  }
  for (const b of Object.values(browsers)) await b.close();
  process.exit(failed ? 1 : 0);
})();
