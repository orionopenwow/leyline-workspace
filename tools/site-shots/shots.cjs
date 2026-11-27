// The website's launcher screenshots (stream/www-files/art/leyline-ui-*.webp), retaken from the launcher UI as it ships
// (launcher/ui, the serverless "lite" app: no sign-in, no official realm, no relay) in Chromium with a fake Tauri core,
// the same way the launcher's browser tests draw it (launcher/tests/lite.browser.test.cjs, hostflow.browser.test.cjs).
// Made-up sample data only: realm and player names are invented, addresses are .example, and no game art: the hero's
// slide picture is the launcher's own nebula (launcher/ui/art/leyline-bg.webp). Every other request is refused.
//
//   NODE_PATH=$(npm root -g) node tools/site-shots/shots.cjs [outDir]        (default: stream/www-files/art)
//   SHOTS_PNG=<dir> also keeps the full-size PNGs; SHOTS_ONLY=hero,realms picks some.
// Needs playwright (npm i -g playwright, or NODE_PATH pointing at one) and python3 with Pillow (WebP encoding).
'use strict';
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..', '..');
const UI = path.join(ROOT, 'launcher', 'ui');
const INDEX = 'file://' + path.join(UI, 'index.html');
const NEBULA = path.join(UI, 'art', 'leyline-bg.webp');
const OUT = path.resolve(process.argv[2] || path.join(ROOT, 'stream', 'www-files', 'art'));
const PNG = process.env.SHOTS_PNG ? path.resolve(process.env.SHOTS_PNG) : fs.mkdtempSync(path.join(os.tmpdir(), 'site-shots-'));
const ONLY = (process.env.SHOTS_ONLY || '').split(',').filter(Boolean);
// the built-in modes (the static site's list) and the Host page's game modes (their pictures are the launcher's own SVGs)
const MODES = JSON.parse(fs.readFileSync(path.join(ROOT, 'stream', 'playmodes.json'), 'utf8')).modes
  .map((m) => { const c = Object.assign({}, m); delete c.listed; return c; });
const HOST_MODES = JSON.parse(fs.readFileSync(path.join(ROOT, 'stream', 'modepacks.json'), 'utf8')).modes
  .map((m) => ({ id: m.id, name: m.name, line: m.line, version: m.version, author: m.author, based_on: m.based_on, status: m.status, profile: m.profile,
    scripts: m.scripts, env: m.env, mods: m.mods, den: m.den, tags: m.tags, jam: null, credit: m.credit || null, settings: m.settings,
    picture: m.picture_data || null, own: false }));
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;
const BAD = /\blog ?in\b|sign ?in|wrath unleashed|official realm|something went wrong|failed|unknown /i;

// ------------------------------------------------------------------------------------------- the fake core
const FAKE = (o) => {
  const calls = (window.__calls = []);
  window.__unknown = [];
  try {
    if (!sessionStorage.getItem('__booted')) {
      sessionStorage.setItem('__booted', '1');
      localStorage.clear();
      for (const [k, v] of Object.entries(o.storage || {})) localStorage.setItem(k, v);
    }
  } catch (_) { /* fine */ }
  const clients = [
    { id: 'c-north', name: 'Northrend', path: 'C:\\Games\\Northrend', build: '3.3.5a', realmlist: '' },
    { id: 'c-classic', name: 'Classic', path: 'C:\\Games\\Classic', build: '1.12.1', realmlist: '' },
    { id: 'c-outland', name: 'Outland', path: 'C:\\Games\\Outland', build: '2.4.3', realmlist: '' },
    { id: 'c-shattered', name: 'Shattered World', path: 'C:\\Games\\Shattered', build: '4.3.4', realmlist: '' },
    { id: 'c-misty', name: 'Misty Isles', path: 'C:\\Games\\Misty Isles', build: '5.4.8', realmlist: '' },
  ];
  const R = (id, name, kind, version, core, players, address, extra) => Object.assign({ slug: 'c-' + id, realm_id: id, name, official: false, kind, state: 'live',
    version, build: version, core, tags: [], players, bots: 0, address, group: 'community' }, extra || {});
  const realms = [
    R('aaaaaaaaaaaaaaaa', 'Emberfall Crossing', 'PvE', '3.3.5a', 'azerothcore', 214, 'logon.emberfall.example', { tags: ['blizzlike'], tagline: 'A calm, long-running realm' }),
    R('bbbbbbbbbbbbbbbb', 'Lanternwood', 'RP', '3.3.5a', 'azerothcore', 96, 'play.lanternwood.example', { tags: ['rp'], tagline: 'Stories every evening' }),
    R('cccccccccccccccc', 'Old Harbor', 'PvP', '1.12.1', 'vmangos', 138, 'old-harbor.example', { tagline: 'The first era, as it was' }),
    R('dddddddddddddddd', 'Stonebrook Keep', 'PvE', '2.4.3', 'cmangos', 57, 'stonebrook.example', { tags: ['progression'] }),
    R('eeeeeeeeeeeeeeee', 'Saltmarsh Rest', 'PvE', '3.3.5a', 'trinitycore', 31, 'saltmarsh.example', { tags: ['fun'], tagline: 'Fast levels, friendly crowd' }),
    R('ffffffffffffffff', 'Quiet Meadow', 'PvE', '1.12.1', 'vmangos', 12, 'quiet-meadow.example', {}),
  ];
  const world = (kind) => ({ kind, supported: true, set_up: kind === 'single', running: false, characters: [], engine: 'builtin' });
  const now = Math.floor(Date.now() / 1000);
  const P = (id, name, desc, cat, author) => ({ id, name, source: 'upstream', download: 'https://example.org/' + id, builds: ['3.3.5a'], category: cat, kind: 'addon',
    description: desc, author, licence: 'MIT' });
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: '0.6.0', settings: { clients, selected_client: o.client || 'c-north', selected_realm: o.realm || 'logon.emberfall.example',
        wine: { enabled: false, command: 'wine', prefix: '' }, seen: ['tour', 'welcome', 'guide', 'home', 'realms', 'modes', 'host', 'single', 'den'] },
        official_realm: 'leylinelauncher.com', browser_url: 'https://leylinelauncher.com/wrath', windows: true, data_dir: 'C:\\Users\\player\\AppData\\Roaming\\Leyline' };
      case 'platform': return { os: 'windows', mobile: false, node: true };
      case 'select': case 'guide_seen': case 'deep_link_take': case 'world_resume_pending': case 'host_scan': case 'host_mode_use': return null;
      case 'realms_list': case 'realms_list_cached': return { realms, network: { realms: realms.length, players: realms.reduce((a, r) => a + r.players, 0), bots: 0 },
        from_cache: false, saved_at: now, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 38, players: 214 };
      case 'mods_catalog': case 'mods_catalog_cached': return { categories: [{ id: 'ui', name: 'Interface' }, { id: 'qol', name: 'Quality of life' }], projects: [
        P('up:bars', 'Better Bars', 'Action bars you can move and resize.', 'ui', 'Tallowmere'),
        P('up:bags', 'Tidy Bags', 'One bag for everything, sorted.', 'qol', 'Penwick'),
        P('up:map', 'Clear Map', 'Coordinates and quest marks on the map.', 'qol', 'Juniper Vale')], from_cache: false, saved_at: now, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c-north', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'launcher_update_state': case 'launcher_update_check': return { mode: 'auto', checking: false, downloading: false, ready: null, info: { current: '0.6.0', latest: '0.6.0' }, checked_at: now };
      case 'modes_list': return { modes: o.modes, from_cache: false, builtin: true, saved_at: null, error: null };
      case 'den_state': return { lite: { available: true, can_get: false, running: false, address: '127.0.0.1', account: 'DEN', den_dir: 'C:\\Leyline\\den' },
        kit: false, docker_den: false, other_realm: null, account: '', saves: 'lite', dens: [{ name: 'garden', saved: now - 86400 }], packs: [] };
      case 'news_list': return { items: [
        { tag: 'LEYLINE', title: 'Leyline 0.6: lighter', date: '2026-11-26', items: ['No account needed: open it and play.', 'Every realm on the list in one place.', 'Single player and Boss Rush on your PC.'] },
        { tag: 'MODS', title: 'New in Discover', date: '2026-11-20', items: ['Better Bars and Clear Map.'] }], from_cache: false };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'world_status': return world(args.kind);
      case 'engine_status': return world('single');
      case 'engine_progress': return { idle: true, kind: 'engine', running: false, code: null };
      case 'open_link': return null;
      case 'agents_state': return { allow: false, waiting: [], pending: [], audit: [], cc: {}, desktop: {}, raw: 'leyline mcp', json: '{}', tiers: { read: [], ask: [], never: [] }, tools: [], never: [], now };
      case 'friends_state': return { friends: [], me: null };
      case 'friend_worlds': return [];
      case 'addons_status': return { pack: { version: '1', addons: [] }, from_cache: false, error: null, prefs: { auto_update: true, dont_ask: false },
        clients: [{ client: 'c-north', installed: true, update: false, missing: [] }] };
      case 'addons_should_offer': return false;
      case 'testing_get': return { client_torrents: false, share: false };
      // the Host page (hostflow.js, hostmodes.js): the Host pack and Docker ready, nothing hosted yet
      case 'host_info': return { dir: 'C:\\Leyline\\host', is_kit: true, has_windows_script: true, has_unix_script: true, problems: [], seeds: [], realm: null };
      case 'host_progress': return { idle: true, kind: '', running: false, code: null, step: '', n: 0, of: 0, secs: 0, problem: null };
      case 'host_modes': return JSON.parse(JSON.stringify(o.hostModes));
      case 'host_flow_current': case 'host_plan_take': return null;
      case 'host_console': return { set_up: false, linked: false, name: '', realm_id: '', profile: 'main', core: 'azerothcore', visibility: 'private', signup: 'invite',
        herokey: false, claim_sent: false, browser: false, max_players: 0, relay: '', relay_connected: false, relay_error: null, docker: true, services: [], uptime: '', gm: false, gm_note: null, invite: '' };
      case 'host_profiles': return [['main', 'vanillaplus', 'progression', 'brawl', 'dalaran', 'solo', 'den'].map((id) =>
        ({ id, label: id, version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [clients[0]], note: null })), null];
      case 'hostpack_info': return { installed: { version: '7' }, available: { version: '7', size: 400000 }, update: false, kit_dir: 'C:\\Leyline\\host' };
      case 'docker_info': return { state: 'ready', version: '27', help: 'Docker is ready.', get_url: 'https://docs.docker.com/desktop/' };
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      default: window.__unknown.push(cmd); return null;
    }
  } } };
};

// the site's Home pictures: the launcher's own nebula for every slide (no game screenshots)
const HOME_JSON = JSON.stringify({ slides: { single: [{ file: 'leyline-nebula.webp', alt: 'The Leyline nebula', credit: '' }],
  realms: [{ file: 'leyline-nebula.webp', alt: 'The Leyline nebula', credit: '' }], news: [{ file: 'leyline-nebula.webp', alt: 'The Leyline nebula', credit: '' }] } });

async function open(browser, o) {
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: o.viewport, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route(/^https?:\/\//, (r) => {
    const u = r.request().url();
    if (/\/wrath\/art\/home\/home\.json$/.test(u)) return r.fulfill({ status: 200, contentType: 'application/json', body: HOME_JSON });
    if (/\/wrath\/art\/home\/leyline-nebula\.webp$/.test(u)) return r.fulfill({ path: NEBULA, contentType: 'image/webp' });
    return r.abort();
  });
  await page.addInitScript(`(${FAKE.toString()})(${JSON.stringify(Object.assign({ modes: MODES, hostModes: HOST_MODES }, o.fake || {}))})`);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && document.querySelector('.page.on'));
  await page.waitForTimeout(800);
  return { page, ctx, errors };
}

// stop the hero gallery on its first slide, so the picture is the same every time
// (slide: 0 Single player, 1 Pick a realm with the realm list beside it)
async function holdHero(page, slide) {
  for (let i = 0; i < (slide || 0); i++) await page.click('#hero-next');
  await page.hover('#hero');
  await page.waitForTimeout(900);   // the slide's fade
}

const SHOTS = [
  { name: 'hero', viewport: { width: 1600, height: 900 },
    out: [{ file: 'leyline-ui-clients.webp', w: 3200, h: 1800 }, { file: 'leyline-ui-clients-1280.webp', w: 1280, h: 720 }],
    fake: { realm: 'logon.emberfall.example', storage: { 'leyline.joined': JSON.stringify(['logon.emberfall.example', 'old-harbor.example']) } },
    run: async (page) => {
      await holdHero(page, 1);
      await page.click('#pb-client');
      await page.waitForSelector('#pb-menu:not([hidden])');
    } },
  { name: 'launcher', viewport: { width: 1240, height: 760 }, out: [{ file: 'leyline-ui-launcher.webp', w: 2480, h: 1520 }],
    fake: { realm: 'logon.emberfall.example', storage: { 'leyline.joined': JSON.stringify(['logon.emberfall.example', 'old-harbor.example']) } },
    run: async (page) => {
      await holdHero(page);
      await page.click('#pb-realm-btn');
      await page.waitForSelector('#pb-rmenu:not([hidden])');
    } },
  { name: 'realms', viewport: { width: 1240, height: 760 }, out: [{ file: 'leyline-ui-realms.webp', w: 2480, h: 1520 }],
    fake: { realm: 'logon.emberfall.example' },
    run: async (page) => {
      await page.click('.nav[data-page="realms"]');
      await page.waitForFunction(() => /Emberfall/.test(document.querySelector('#rl-list').textContent));
    } },
  { name: 'hosting', viewport: { width: 1240, height: 760 }, out: [{ file: 'leyline-ui-hosting.webp', w: 2480, h: 1520 }],
    run: async (page) => {
      // no menu item in the serverless app, but the page stays for this PC's own worlds (app.js LITE_GONE note)
      await page.evaluate(() => window.LeylineNav.show('host'));
      await page.waitForSelector('#hst .hst-vis, #hx', { timeout: 10000 });
      if (await page.locator('.hst-more summary').count()) {
        await page.click('.hst-more summary');
        await page.click('.hst-modes');
        await page.waitForSelector('#hx .hx-card');
      }
      await page.mouse.move(5, 5);
      // the Host panel's header at the top of the page (the Den banner above it scrolled away, as the old picture had it)
      await page.evaluate(() => {
        const panel = document.querySelector('#hx').closest('.panel') || document.querySelector('#hx');
        let sc = panel.parentElement;
        while (sc && !(sc.scrollHeight > sc.clientHeight && /auto|scroll/.test(getComputedStyle(sc).overflowY))) sc = sc.parentElement;
        if (sc) sc.scrollTop += panel.getBoundingClientRect().top - sc.getBoundingClientRect().top - 12;
      });
      await page.evaluate(() => document.activeElement && document.activeElement.blur());
    } },
  { name: 'modes', viewport: { width: 1240, height: 760 }, out: [{ file: 'leyline-ui-modes.webp', w: 2480, h: 1520 }],
    run: async (page) => {
      await page.click('.nav[data-page="modes"]');
      await page.waitForSelector('#md-list .md-card');
    } },
];

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  fs.mkdirSync(PNG, { recursive: true });
  const browser = await chromium.launch();
  let bad = 0;
  for (const s of SHOTS) {
    if (ONLY.length && !ONLY.includes(s.name)) continue;
    const { page, ctx, errors } = await open(browser, s);
    await s.run(page);
    await page.waitForTimeout(900);   // fades and pictures
    const text = await page.evaluate(() => document.body.innerText);
    const unknown = await page.evaluate(() => [...new Set(window.__unknown)]);
    const png = path.join(PNG, s.name + '.png');
    await page.screenshot({ path: png });
    const problems = [...errors];
    if (BRANDS.test(text)) problems.push('a game or publisher name on screen: ' + text.match(BRANDS)[0]);
    if (BAD.test(text)) problems.push('on screen: ' + text.match(BAD)[0]);
    console.log(`${s.name}: ${png}` + (unknown.length ? `  (core calls answered null: ${unknown.join(', ')})` : ''));
    for (const p of problems) { console.log('  PROBLEM ' + p); bad++; }
    for (const o of s.out) {
      const dest = path.join(OUT, o.file);
      execFileSync('python3', ['-I', '-c', `import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert('RGB')
w, h = int(sys.argv[3]), int(sys.argv[4])
if im.size != (w, h): im = im.resize((w, h), Image.LANCZOS)
im.save(sys.argv[2], 'WEBP', quality=int(sys.argv[5]), method=6)`, png, dest, String(o.w), String(o.h), String(o.q || 82)]);
      console.log(`  -> ${dest} ${o.w}x${o.h} ${Math.round(fs.statSync(dest).size / 1024)} KB`);
    }
    await ctx.close();
  }
  await browser.close();
  process.exit(bad ? 1 : 0);
})();
