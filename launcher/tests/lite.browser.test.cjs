// Leyline as a launcher aggregator (ui/lite.js, owner 26 Nov 2026: Leyline's only server is shut down; docs/LAUNCHER-LITE.md)
// in Chromium with a fake Tauri core: the app opens straight to Home with no sign-in and no call before Home is on
// screen; nothing that needed the server is shown or called (the sign-in, the Hero Key, accounts, the official realm,
// the browser game, the relay's invite links, the forum, social, the Den market, the official boards, bug reports to the
// server, Host); what stays works: the realm list of other servers and an address of your own, Single player and the
// original world first in the play bar, Your Den, mods, news, Modes with Boss Rush, Settings, Help with Copy debug info
// and GitHub issues, the Community page (the Discord), Restart to update. Screenshots of the whole app
// (LITE_SHOTS=<dir>, the gallery's lite-*.png).
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/lite.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const SHOTS = process.env.LITE_SHOTS || '';
// the built-in modes (what the core lists when the static site's modes.json is empty: modes.rs builtin)
const MODES = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'stream', 'playmodes.json'), 'utf8')).modes
  .map((m) => { const c = Object.assign({}, m); delete c.listed; return c; });
if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true });

// what only the old server answered: never called in the lite app
const SERVER_CMDS = ['social_status', 'social_call', 'social_key_found', 'forum_call', 'forum_post', 'forum_open', 'bug_report', 'play_realm_herokey',
  'open_browser_play', 'back_to_official', 'dens_browse', 'dens_post', 'dens_share', 'listing_post', 'listing_mine', 'boards_get', 'friends_mailbox'];

const FAKE = (o, MODES) => {
  const calls = (window.__calls = []);
  window.__t0 = performance.now();
  try {
    if (!sessionStorage.getItem('__booted')) {
      sessionStorage.setItem('__booted', '1');
      localStorage.clear();
      for (const [k, v] of Object.entries(o.storage || {})) localStorage.setItem(k, v);
    }
  } catch (_) { /* fine */ }
  const world = (kind) => ({ kind, supported: true, set_up: kind === 'single', running: false, characters: [], engine: 'builtin' });
  const realms = [
    { slug: 'c-aaaaaaaaaaaaaaaa', realm_id: 'aaaaaaaaaaaaaaaa', name: 'Frost Keep', official: false, kind: 'PvE', state: 'live', version: '3.3.5a', build: '3.3.5a',
      core: 'azerothcore', tags: ['blizzlike'], players: 120, bots: 0, address: 'logon.frostkeep.example:3724', group: 'community', tagline: 'A long-running server' },
    { slug: 'c-bbbbbbbbbbbbbbbb', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Old Days', official: false, kind: 'PvP', state: 'live', version: '1.12.1', build: '1.12.1',
      core: 'vmangos', tags: [], players: 40, bots: 0, address: 'old-days.example', group: 'community' },
  ];
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args, performance.now()]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c335a-1', name: 'Wrath 3.3.5a', path: '/games/wrath', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c335a-1', selected_realm: o.realm || 'leylinelauncher.com', wine: { enabled: false, command: 'wine', prefix: '' } },
        official_realm: 'leylinelauncher.com', browser_url: 'https://leylinelauncher.com/wrath', windows: true, data_dir: '/tmp/leyline' };
      case 'platform': return { os: 'windows', mobile: false, node: true };
      case 'select': return null;
      case 'realms_list': case 'realms_list_cached': return { realms, network: { realms: 2, players: 160, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 40, players: 120 };
      case 'mods_catalog': case 'mods_catalog_cached': return { categories: [{ id: 'ui', name: 'Interface' }], projects: [
        { id: 'up:bars', name: 'Better Bars', source: 'upstream', download: 'https://github.com/x/bars', builds: ['3.3.5a'], category: 'ui', kind: 'addon', description: 'Bars, better.', author: 'Someone', licence: 'MIT' }],
        from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c335a-1', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return o.update || { mode: 'auto', checking: false, downloading: false, ready: null, info: { current: '0.4.0', latest: '0.4.0' }, checked_at: 1 };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'host_progress': case 'engine_progress': return { idle: true, kind: 'engine', running: false, code: null };
      case 'modes_list': return { modes: MODES, from_cache: false, builtin: true, saved_at: null, error: null };
      case 'den_state': return { lite: { available: true, can_get: false, running: false, address: '127.0.0.1', account: 'DEN', den_dir: '/den' },
        kit: false, docker_den: false, other_realm: null, account: '', saves: 'lite', dens: [{ name: 'garden', saved: 1 }], packs: [] };
      case 'news_list': return { items: [{ tag: 'LEYLINE', title: 'Leyline, lighter', date: '2026-11-26', items: ['The launcher, without a server of its own.'] }], from_cache: false };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'world_status': return world(args.kind);
      case 'engine_status': return world('single');
      case 'world_resume_pending': return null;
      case 'debug_info': return 'Leyline test\nSystem: windows\nWhere: ' + (args && args.place) + '\nRecent log: nothing\n';
      case 'open_link': window.__opened = (window.__opened || []).concat(args.name); return null;
      case 'agents_state': return { allow: false, waiting: [], audit: [], cc: {}, desktop: {}, raw: 'leyline mcp', json: '{}', tiers: { read: [], ask: [], never: [] } };
      case 'friends_state': return { friends: [], me: null };
      case 'friend_worlds': return [];
      case 'addons_status': return { pack: { version: '1', addons: [] }, from_cache: false, error: null, prefs: { auto_update: true, dont_ask: false },
        clients: [{ client: 'c335a-1', installed: true, update: false, missing: [] }] };
      case 'addons_should_offer': return false;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(o = {}) {
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: 1280, height: 800 }, permissions: ['clipboard-read', 'clipboard-write'] });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e) + (process.env.LITE_STACK ? e.stack : '')));
  // nothing leaves this page: the site's pictures stand in, every other request is refused and kept
  const net = [];
  await page.route(/^https?:\/\//, (r) => { net.push(r.request().url()); return r.abort(); });
  await page.addInitScript(`(${FAKE.toString()})(${JSON.stringify(o)}, ${JSON.stringify(MODES)})`);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && document.querySelector('.page.on'));
  return { page, ctx, errors, net };
}
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, 'lite-' + name + '.png') }); };
const visibleText = (page) => page.evaluate(() => document.body.innerText);
const NO_SERVER = /\b(log ?in|sign ?in|sign up|hero key|google|manage account|wrath unleashed|official realm|play in your browser|forum|leaderboard|report a bug|host your own|be a node)\b/i;
const BRAND = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;

test('opens straight to Home: no sign-in, no server call, Single player first in the play bar', async () => {
  const { page, ctx, errors } = await open();
  assert.equal(await page.evaluate(() => document.documentElement.classList.contains('lite')), true);
  assert.equal(await page.evaluate(() => window.LeylineLite.SERVER_FEATURES), false);
  assert.deepEqual(await page.$$eval('.page.on', (p) => p.map((x) => x.id)), ['page-play'], 'Home');
  assert.equal(await page.isVisible('#gate'), false, 'no sign-in screen');
  assert.equal(await page.isVisible('#tb-account'), false, 'no account chip');
  // the first network-ish call comes after Home is drawn
  const first = await page.evaluate(() => {
    const net = ['realms_list', 'mods_catalog', 'news_list', 'launcher_update_state', 'modes_list'];
    const c = window.__calls.find(([n]) => net.includes(n));
    return c ? c[0] : null;
  });
  assert.ok(first, 'the lists still load');
  await page.waitForTimeout(400);
  const called = await page.evaluate(() => window.__calls.map(([c]) => c));
  for (const c of SERVER_CMDS) assert.ok(!called.includes(c), `never ${c}`);
  // the play bar: Single player (not the official realm), and its list has no official realm or browser game
  assert.equal((await page.textContent('#pb-realm')).trim(), 'Single player');
  await page.click('#pb-realm-btn');
  const opts = await page.$$eval('#pb-rmenu .pb-opt .pb-opt-name', (o) => o.map((x) => x.textContent));
  assert.equal(opts[0], 'Single player');
  assert.ok(opts.includes('Your Den') || opts.some((x) => /den/i.test(x)), String(opts));
  assert.ok(!opts.some((x) => /wrath unleashed|browser|host a server/i.test(x)), String(opts));
  await page.keyboard.press('Escape');
  // the side menu: no Host or Forum; Community in their place
  const items = await page.$$eval('aside nav .nav[data-page]', (b) => b.filter((x) => x.offsetParent !== null).map((x) => x.dataset.page));
  assert.deepEqual(items, ['play', 'realms', 'mods', 'community', 'news', 'single', 'modes']);
  assert.doesNotMatch(await page.textContent('#realm-rows'), /wrath unleashed/i, 'Your realms: no official realm');
  const text = await visibleText(page);
  assert.doesNotMatch(text, BRAND);
  assert.doesNotMatch(text, NO_SERVER);
  await shot(page, 'home');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Realms: other servers only, an address of your own, no relay invite or hosting', async () => {
  const { page, ctx, errors } = await open();
  await page.click('.nav[data-page="realms"]');
  await page.waitForFunction(() => document.querySelectorAll('#rl-list .rl-card, #rl-list [data-key]').length > 0 || /Frost Keep/.test(document.querySelector('#rl-list').textContent));
  const tabs = await page.$$eval('#rl-tabs [role="tab"]', (t) => t.map((x) => x.textContent.trim()));
  assert.ok(!tabs.some((t) => /official/i.test(t)), String(tabs));
  assert.ok(tabs.includes('Other servers'), String(tabs));
  assert.match(await page.textContent('#rl-list'), /Frost Keep/);
  assert.equal(await page.isVisible('#rl-host-cta'), false);
  assert.equal(await page.isVisible('#rl-invite'), false);
  assert.equal(await page.locator('#play-realm option[value="official"]').count(), 0);
  assert.equal(await page.isVisible('#rl-paste-in'), true, 'any realm by its address');
  const text = await visibleText(page);
  assert.doesNotMatch(text, BRAND);
  assert.doesNotMatch(text, NO_SERVER);
  await shot(page, 'realms');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the pages that needed the server land elsewhere; Community has the Discord', async () => {
  const { page, ctx, errors } = await open({ storage: { 'leyline.page': 'forum' } });
  assert.deepEqual(await page.$$eval('.page.on', (p) => p.map((x) => x.id)), ['page-community'], 'a remembered Forum opens Community');
  await page.evaluate(() => window.LeylineNav.show('account'));
  assert.deepEqual(await page.$$eval('.page.on', (p) => p.map((x) => x.id)), ['page-play']);
  await page.click('.nav[data-page="community"]');
  await page.click('#cm-discord');
  assert.deepEqual(await page.evaluate(() => window.__opened), ['discord']);
  await shot(page, 'community');
  await page.click('.nav[data-page="mods"]');
  assert.equal(await page.isVisible('#tab-mods-dens'), false, 'no Den market');
  await shot(page, 'mods');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Help: Copy debug info and GitHub issues instead of a report to the server', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('help'));
  assert.equal(await page.isVisible('#bug-open'), false);
  await page.click('#dbg-copy');
  await page.waitForFunction(() => /Copied|copy/.test(document.querySelector('#dbg-status').textContent));
  assert.match(await page.inputValue('#dbg-out'), /^Leyline test\nSystem: windows\nWhere: Launcher/);
  await page.click('button[data-link="issues"]');
  assert.deepEqual(await page.evaluate(() => window.__opened), ['issues']);
  const text = await visibleText(page);
  assert.doesNotMatch(text, BRAND);
  assert.doesNotMatch(text.replace(/Report a bug/g, ''), /official realm|play in your browser|hero key|forum/i);
  await shot(page, 'help');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Settings: no account, privacy says what it fetches now; Restart to update stays', async () => {
  const { page, ctx, errors } = await open({ update: { mode: 'auto', checking: false, downloading: false, ready: true, game_running: false, info: { current: '0.4.0', latest: '0.4.1', newer: true }, checked_at: 1 } });
  await page.evaluate(() => window.LeylineNav.show('settings'));
  assert.equal(await page.isVisible('#set-acct-box'), false);
  assert.equal(await page.isVisible('#set-count'), false, 'no install counts posted');
  assert.match(await page.textContent('#page-settings'), /there's no account/);
  await page.waitForFunction(() => !document.querySelector('#tb-update').hidden);
  assert.equal((await page.textContent('#tb-update-go')).trim(), 'Restart');
  assert.equal(await page.isVisible('#upd-restart'), true);
  const text = await visibleText(page);
  assert.doesNotMatch(text, BRAND);
  assert.doesNotMatch(text, /hero key|manage account|forum|wrath unleashed|official realm/i);
  await shot(page, 'settings');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Single player: plays without asking to log in', async () => {
  const { page, ctx, errors } = await open();
  await page.click('.nav[data-page="single"]');
  await page.waitForTimeout(300);
  const text = await visibleText(page);
  assert.doesNotMatch(text, BRAND);
  assert.doesNotMatch(text, /hero key|log in to|sign in|wrath unleashed|official realm|relay/i);
  await shot(page, 'single');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Modes: Boss Rush on this PC, no Duels and no boards; the tour and the i bubbles say what is here', async () => {
  const { page, ctx, errors } = await open();
  await page.click('.nav[data-page="modes"]');
  await page.waitForSelector('#md-list .md-card');
  const ids = await page.$$eval('#md-list .md-card', (c) => c.map((x) => x.dataset.mode));
  assert.deepEqual(ids, ['frozen-throne'], 'Duels ran on the official realm');
  assert.equal(await page.isVisible('#md-boards'), false);
  await page.click('#md-list .md-card[data-mode="frozen-throne"] .btn.primary');
  assert.deepEqual(await page.$$eval('.page.on', (p) => p.map((x) => x.id)), ['page-bossrush'], 'a fight on this PC');
  assert.equal(await page.isVisible('#br-boards'), false);
  await page.evaluate(() => window.LeylineNav.show('modes'));
  await page.waitForTimeout(500);   // the page's fade-in
  const text = await visibleText(page);
  assert.doesNotMatch(text, BRAND);
  assert.doesNotMatch(text, /duel|leaderboard|official realm|log in/i);
  await shot(page, 'modes');
  // the tour: no official realm or Host step; the realms instead
  const tour = await page.evaluate(() => window.LeylineGuide.TOUR.map((t) => t.key));
  assert.ok(!tour.includes('play') && !tour.includes('host') && tour.includes('realms'), String(tour));
  // the i bubble on Realms
  await page.evaluate(() => window.LeylineNav.show('realms'));
  await page.click('#ll-info');
  await page.waitForSelector('#ll-info-b:not([hidden])');
  assert.match(await page.textContent('#ll-info-b'), /other servers/i);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Your Den: a copy to send a friend, no Workshop or shared Dens; Characters: this PC only', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('den'));
  await page.waitForSelector('#yd .yd-tile');
  const tiles = await page.$$eval('#yd .yd-tile h3', (h) => h.map((x) => x.textContent));
  assert.ok(!tiles.includes('Share on the Workshop') && tiles.includes('Share a Den'), String(tiles));
  const text = await visibleText(page);
  assert.doesNotMatch(text, /workshop|relay|hero key|log in/i);
  await page.evaluate(() => window.LeylineNav.show('characters'));
  await page.waitForSelector('#ch-tab-local');
  assert.equal(await page.locator('#ch-tab-online').count(), 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});
