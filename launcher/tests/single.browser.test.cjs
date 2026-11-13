// Single player (ui/single.js; owner 4 Nov plan, Phase 1: "Single player is a blatant, top-level choice"), in Chromium
// with a fake Tauri core:
//   * its side menu item (the sword mark), Home's row and the realm picker's row (under Your Den) all lead to its page;
//   * the page's three steps, each with its state, and the progress line: Your game folder (the added folders of its
//     version, or drop / choose one through app.js's own add paths), Engine (Built-in recommended and picked where it
//     is made for the PC, off with "Windows only for now" elsewhere; Docker the fallback, checked live: not installed
//     with Get Docker, not running with Start Docker, Re-check), Start;
//   * Built-in (Phase 2, docs/BUILTIN-ENGINE.md): Start is engine_setup with the folder, its steps from engine_progress
//     with the download's and the maps' own %, then in (engine_play); Manage: Play, Stop, Start, Logs, Back up, the
//     login made for the player with Copy, Update the engine; the realm picker's row plays it; Docker still pickable;
//   * Start: the Host pack once, host_choose solo, host_start_quiet (private, no questions, no password), its steps,
//     then in as the picker's row plays it (host_invite, join_invite, PLAY); My Den running here: the Host page asks;
//   * once set up: the Manage card (Play, Stop, Start, Logs, Settings); no brand names, "Log in" never "Sign in".
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/single.browser.test.cjs     (SINGLE_SHOTS=<dir> for screenshots)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.SINGLE_SHOTS || '';

const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const WRATH2 = { id: 'c335a-2', name: 'Wrath copy', path: 'D:\\Wrath2', build: '3.3.5a', realmlist: '' };
const VANILLA = { id: 'c1121-1', name: 'My Vanilla', path: '/home/me/Games/vanilla', build: '1.12.1', realmlist: '' };
const NET = [
  { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 5, bots: 0, address: 'play.openvanilla.app' },
];
const KIT = (profile, name) => ({ dir: '/home/me/kit', is_kit: true, has_windows_script: true, has_unix_script: true, problems: [], seeds: [],
  realm: profile ? { realm_id: 'abcdefghijk2345a', name, visibility: 'private', relay: 'relay.openvanilla.app', profile } : null });
const NO_KIT = { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
const READY = { state: 'ready', version: '27', help: '', get_url: 'https://docs.docker.com/desktop/', can_start: false };
const MISSING = { state: 'missing', version: null, help: 'Get Docker Desktop: free for personal use.', get_url: 'https://docs.docker.com/desktop/', can_start: false };
// engine_status (host/engine.rs EngineStatus): not made for this PC (the Docker tests), made but not set up, set up
const ES_NO = { supported: false, installed: false, can_get: false, set_up: false, running: false, update: false, account: '', password: '' };
const ES_NEW = Object.assign({}, ES_NO, { supported: true, can_get: true });
const ES_READY = Object.assign({}, ES_NEW, { installed: true, can_get: false, maps: true, db: true, filled: true, account_made: true, set_up: true,
  running: true, db_up: true, auth_up: true, world_up: true, account: 'SOLO', password: 'ABCD2345', realm: '127.0.0.1:3724', version: '2026.11.04-abc1234' });
const STOPPED = { state: 'desktop_stopped', version: null, help: 'Open Docker Desktop.', get_url: 'https://docs.docker.com/desktop/', can_start: true };

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  window.__kit = o.kit;
  window.__dock = o.docker;
  window.__prog = { idle: true, kind: 'host', running: false, code: null, step: '', n: 0, of: 8, secs: 0, problem: null };
  window.__stopped = !!o.stopped;
  window.__es = o.engine || { supported: false, installed: false, can_get: false, set_up: false, running: false, update: false, account: '', password: '' };
  window.__eprog = { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7, secs: 0, problem: null, pct: null, detail: '' };
  const st = (window.__st = { version: 'test', settings: { clients: o.clients, selected_client: o.clients[0] ? o.clients[0].id : null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' });
  const offer = (id) => ({ id, label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true,
    clients: st.settings.clients.filter((c) => c.build === '3.3.5a').map((c) => ({ id: c.id, name: c.name, path: c.path })), note: null });
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'select': st.settings.selected_client = args.client; st.settings.selected_realm = args.realm; return null;
      case 'realms_list': return { realms: o.net, network: { realms: 1, players: 5, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return JSON.parse(JSON.stringify(window.__kit));
      case 'host_invite': return window.__kit.realm ? 'https://relay.openvanilla.app/join/abcdefghijk2345a#k3y' : null;
      case 'join_invite': return { address: 'relay.openvanilla.app:40007', name: window.__kit.realm.name };
      case 'play': return 'Started.';
      case 'host_profiles': return [['vanillaplus', 'den', 'solo', 'dalaran'].map(offer), null];
      case 'host_choose': return { profile: args.profile, client_dir: '' };
      case 'host_start_quiet':
        window.__prog = { idle: false, kind: 'host', running: true, code: null, step: 'Reading the maps from your game folder', n: 3, of: 8, secs: 65, problem: null };
        return 'Setting up your realm.';
      case 'host_progress': return JSON.parse(JSON.stringify(window.__prog));
      case 'host_stop': window.__stopped = true; return 'Stopped. Everyone was saved.';
      case 'host_logs': return 'world: ready\nauth: ready';
      case 'hostpack_install': window.__kit = Object.assign({}, window.__kit, { dir: '/home/me/kit', is_kit: true }); return [true, '/home/me/kit'];
      case 'set_host_kit': return null;
      case 'hostpack_info': return window.__kit.is_kit ? { installed: { version: '7' }, available: { version: '7' }, update: false, kit_dir: '/home/me/kit' }
        : { installed: null, available: { version: '7', size: 400000 }, update: false, kit_dir: '/home/me/kit' };
      case 'docker_info': return JSON.parse(JSON.stringify(window.__dock));
      // the Built-in engine (host/engine.rs)
      case 'engine_status': return JSON.parse(JSON.stringify(window.__es));
      case 'engine_setup': case 'engine_start':
        window.__eprog = { idle: false, kind: 'engine', running: true, code: null, step: 'Getting the engine (a one-time download)', n: 1, of: 7, secs: 3, problem: null, pct: 0, detail: '' };
        return 'Getting the engine…';
      case 'engine_progress': return JSON.parse(JSON.stringify(window.__eprog));
      case 'engine_stop': window.__es = Object.assign({}, window.__es, { running: false, world_up: false, auth_up: false, db_up: false }); return 'Stopped. Everyone was saved.';
      case 'engine_logs': return 'setup: done\nworld: World initialized';
      case 'engine_backup': return 'C:\\Users\\me\\AppData\\Roaming\\Leyline\\engine\\backups\\single-player-2026-11-04-200509.zip';
      case 'engine_update_check': return JSON.parse(JSON.stringify(window.__es));
      case 'world_update_check': return JSON.parse(JSON.stringify(window.__es));
      case 'world_password': return 'ABCD2345';
      case 'world_backups': return [{ name: 'single-player-2026-11-11-080000.zip', at: '2026-11-11 08:00:00', why: 'daily', kb: 800 }];
      case 'engine_play': return { message: 'Started My Wrath on 127.0.0.1:3724.', account: window.__es.account, password: window.__es.password };
      case 'docker_start': window.__dock = Object.assign({}, window.__dock, { state: 'ready', can_start: false }); return 'Starting Docker…';
      case 'open_link': return null;
      case 'host_scan': return null;
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      case 'host_console': return { set_up: true, linked: false, name: window.__kit.realm && window.__kit.realm.name, realm_id: 'abcdefghijk2345a',
        profile: window.__kit.realm && window.__kit.realm.profile, core: 'azerothcore', visibility: 'private', signup: 'invite', browser: false, max_players: 5,
        docker: true, services: [{ role: 'world', state: window.__stopped ? 'off' : 'ok', text: window.__stopped ? 'stopped' : 'up' }], gm: false, gm_note: null };
      case 'host_status': return { realm_id: args.realmId, online: !window.__stopped, name: 'x', visibility: 'private', players: 1, bots: 0, listed: false };
      case 'host_dens': return [];
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return true;   // a sign-in kept (the website not asked: the sign-in screen stays closed)
      case 'pick_folder': return 'C:\\Games\\Wrath';
      // app.js's own add path (clientImport): a folder is added where it is
      case 'client_import_check': return { kind: 'folder', name: 'Wrath', unpacks: false, default_folder: null };
      case 'client_import_start': st.settings.clients.push({ id: 'c335a-9', name: 'Dropped Wrath', path: args.input, build: '3.3.5a', realmlist: '' }); return null;
      case 'client_import_state': return { phase: 'done', done: 1, total: 1, message: 'Added Dropped Wrath.', client_id: 'c335a-9' };
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });
async function open(o, scheme = 'dark') {
  const opts = Object.assign({ clients: [WRATH, VANILLA], kit: NO_KIT, net: NET, docker: READY }, o);
  const ctx = await browser.newContext({ colorScheme: scheme, viewport: { width: 1400, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineSingle && window.__calls.some(([c]) => c === 'host_info'));
  await page.waitForFunction(() => document.getElementById('pb-realm').textContent.length > 0);
  if (scheme === 'light') await page.evaluate(() => document.documentElement.setAttribute('data-base', 'light'));
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const pageOn = (page) => page.evaluate(() => document.querySelector('.page.on').id);
const states = (page) => page.$$eval('#sp .sp-step', (l) => l.map((x) => x.id.replace('sp-step-', '') + ':' + x.dataset.state));
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;
const TITLE = 'Just you and your bot companions, on this PC: dungeons and raids with helper bots, levelling at your pace';
async function openSingle(page) {
  await page.click('.side .nav[data-page="single"]');
  await page.waitForFunction(() => document.querySelector('#page-single.on') && document.querySelector('#sp-dock') && !/Checking/.test(document.querySelector('#sp-dock').textContent));
}
async function shot(page, name) { if (!SHOTS) return; await page.waitForTimeout(500); await page.screenshot({ path: path.join(SHOTS, name + '.png') }); }   // after the page's fade-in
// no brand names, no client version numbers in sentences (the chips are fine), "Log in" never "Sign in"
async function wording(page) {
  const text = await page.evaluate(() => {
    const c = document.querySelector('#page-single').cloneNode(true);
    c.querySelectorAll('.rl-ver').forEach((x) => x.remove());
    return c.innerText;
  });
  assert.doesNotMatch(text, BRANDS);
  assert.doesNotMatch(text, /sign in|sign-in|signin/i);
  assert.doesNotMatch(text, /\b\d+\.\d+\.\d+a?\b/, 'no version numbers in the sentences');
  assert.doesNotMatch(text, /donat|\btip\b|patreon|ko-fi/i);
}

test('Single player is a top-level choice: the side menu (sword mark), Home and the realm picker all lead to its page', async () => {
  const { page, ctx, errors } = await open({});
  const nav = await page.$$eval('aside nav .nav[data-page]', (b) => b.filter((x) => x.offsetParent !== null).map((x) => x.dataset.page));
  assert.equal(nav[nav.indexOf('news') + 1], 'single', 'first in the Play fold, under the website\'s pages (owner 12 Nov)');
  assert.equal(await page.getAttribute('#fold-play-head', 'aria-expanded'), 'true', 'the Play fold starts open');
  assert.equal((await page.textContent('.nav[data-page="single"]')).trim(), 'Single player');
  assert.ok(await page.$('.nav[data-page="single"] svg path'), 'the sword mark');
  // Home: its own row, like Your Den's
  assert.equal(await page.textContent('#hm-single b'), 'Single player');
  assert.equal(await page.textContent('#hm-single .hm-den-txt span'), TITLE);
  await page.click('#hm-single-go');
  assert.equal(await pageOn(page), 'page-single');
  assert.equal(await page.textContent('#sp h2'), 'Single player');
  assert.equal(await page.textContent('#sp .sp-title'), TITLE);
  // the realm picker: its own row right under Your Den, with the sword mark; not set up yet: its page
  await page.evaluate(() => window.LeylineNav.show('play'));
  await page.click('#pb-realm-btn');
  const rows = await page.$$eval('#pb-rmenu > li.pb-opt', (l) => l.map((x) => x.querySelector('.pb-opt-name').textContent));
  assert.equal(rows[rows.indexOf('Your Den') + 1], 'Single player');
  assert.equal(await page.textContent('#pb-rmenu .pb-solo .pb-opt-sub'), 'Set it up once');
  assert.equal(await page.getAttribute('#pb-rmenu .pb-solo', 'title'), TITLE);
  assert.ok(await page.$('#pb-rmenu .pb-solo .pb-solo-ic svg'));
  assert.doesNotMatch(await page.textContent('#pb-rmenu'), /Solo leveling/);
  await page.click('#pb-rmenu .pb-solo');
  assert.equal(await pageOn(page), 'page-single');
  assert.equal((await callsOf(page, 'host_start_quiet')).length, 0, 'nothing starts by itself');
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), /Solo leveling/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a new player (no folder, no Docker): each step says its state; Built-in is off; Get Docker and Re-check; Start waits', async () => {
  for (const scheme of ['dark', 'light']) {
    const { page, ctx, errors } = await open({ clients: [VANILLA], docker: MISSING }, scheme);
    await openSingle(page);
    assert.deepEqual(await states(page), ['folder:now', 'engine:now', 'start:todo']);
    assert.equal(await page.textContent('#sp-progress .sp-progress-t'), 'Step 1 of 3 · 0 done');
    assert.equal(await page.getAttribute('#sp-progress .sp-bar', 'aria-valuenow'), '0');
    // step 1: only folders of its version (none here), drop or choose
    assert.equal(await page.$$eval('#sp .sp-folder', (l) => l.length), 0, 'the 1.12.1 folder is not offered');
    assert.match(await page.textContent('#sp-step-folder'), /Drop your game folder here/);
    assert.equal(await page.textContent('#sp-step-folder .rl-ver'), '3.3.5a', 'the version as a chip');
    // step 2: Built-in off (not made for this PC), Docker not installed
    assert.equal(await page.getAttribute('#sp-engine-builtin', 'aria-disabled'), 'true');
    assert.match(await page.textContent('#sp-engine-builtin'), /Windows only for now/);
    assert.equal(await page.getAttribute('#sp-engine-docker', 'aria-checked'), 'true');
    assert.equal(await page.textContent('#sp-dock'), 'Not installed');
    assert.match(await page.textContent('#sp-engine-docker'), /Get Docker Desktop: free/);
    await page.click('#sp-dock-get');
    assert.deepEqual(await callsOf(page, 'open_link'), [{ name: 'docker' }]);
    const n = (await callsOf(page, 'docker_info')).length;
    await page.click('#sp-dock-check');
    await page.waitForFunction((n) => window.__calls.filter(([c]) => c === 'docker_info').length > n, n);
    // step 3: waits for the others
    assert.ok(await page.isDisabled('#sp-start'));
    assert.equal(await page.textContent('#sp-start-hint'), 'Add your game folder first (step 1).');
    await wording(page);
    await shot(page, 'single-new-' + scheme);
    assert.deepEqual(errors, []);
    await ctx.close();
  }
});

test('step 1: a folder dropped on the page goes through the launcher\'s own add path, then is picked; two folders: pick one', async () => {
  const { page, ctx, errors } = await open({ clients: [] });
  await openSingle(page);
  await page.evaluate(() => {
    const dt = new DataTransfer();
    dt.setData('text/plain', 'C:\\Games\\Wrath');
    document.getElementById('sp-drop').dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  await page.waitForFunction(() => document.querySelector('#sp-step-folder[data-state="done"]'));
  assert.deepEqual(await callsOf(page, 'client_import_check'), [{ input: 'C:\\Games\\Wrath' }]);
  assert.equal(await page.textContent('#sp .sp-folder.on b'), 'Dropped Wrath');
  assert.deepEqual(await states(page), ['folder:done', 'engine:done', 'start:now']);
  assert.equal(await page.textContent('#sp-progress .sp-progress-t'), 'Step 3 of 3 · 2 done');
  await ctx.close();
  // two folders of its version: one picked, the other one pick away (Choose a folder… is the same add path)
  const b = await open({ clients: [WRATH, WRATH2, VANILLA] });
  await openSingle(b.page);
  assert.deepEqual(await b.page.$$eval('#sp .sp-folder', (l) => l.map((x) => x.dataset.client + ':' + x.getAttribute('aria-checked'))), ['c335a-1:true', 'c335a-2:false']);
  await b.page.click('#sp .sp-folder[data-client="c335a-2"]');
  assert.equal(await b.page.getAttribute('#sp .sp-folder[data-client="c335a-2"]', 'aria-checked'), 'true');
  await b.page.click('#sp-pick');
  await b.page.waitForFunction(() => window.__calls.some(([c]) => c === 'client_import_check'));
  assert.deepEqual([...errors, ...b.errors], []);
  await b.ctx.close();
});

test('Docker not running: Start Docker, then the step is done by itself', async () => {
  const { page, ctx, errors } = await open({ docker: STOPPED });
  await openSingle(page);
  assert.equal(await page.textContent('#sp-dock'), 'Not running');
  assert.equal(await page.textContent('#sp-start-hint'), 'Docker first (step 2).');
  await page.click('#sp-dock-start');
  await page.waitForFunction(() => document.querySelector('#sp-step-engine[data-state="done"]'), null, { timeout: 10000 });
  assert.equal(await page.textContent('#sp-dock'), 'Found: running');
  assert.ok(!(await page.isDisabled('#sp-start')));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Start: the Host pack once, Single player chosen and started private with no questions, its steps, then in (knock, PLAY)', async () => {
  const { page, ctx, errors } = await open({ clients: [VANILLA, WRATH] });
  await openSingle(page);
  assert.deepEqual(await states(page), ['folder:done', 'engine:done', 'start:now']);
  assert.match(await page.textContent('#sp-start-hint'), /reads the maps from your game folder/);
  await page.click('#sp-start');
  await page.waitForSelector('#sp-prog');
  assert.equal((await callsOf(page, 'hostpack_install')).length, 1, 'no Host pack yet: got once');
  assert.deepEqual(await callsOf(page, 'set_host_kit'), [{ path: '/home/me/kit' }]);
  assert.deepEqual(await callsOf(page, 'host_choose'), [{ profile: 'solo', clientId: 'c335a-1' }], 'the 3.3.5a folder');
  assert.deepEqual(await callsOf(page, 'host_start_quiet'), [{ answers: { name: 'Single player', visibility: 'private', gm: null } }], 'no password, no questions');
  await page.waitForFunction(() => /Reading the maps/.test(document.querySelector('#sp-prog').textContent));
  assert.match(await page.textContent('#sp-prog'), /Step 3 of 8/);
  assert.equal(await page.getAttribute('#sp-step-start', 'data-state'), 'run');
  await wording(page);
  await shot(page, 'single-starting-dark');
  // it finishes: the kit hosts it now, and it plays by itself as the picker's row does
  await page.evaluate(() => {
    window.__kit = { dir: '/home/me/kit', is_kit: true, problems: [], seeds: [], realm: { realm_id: 'abcdefghijk2345a', name: 'Single player', visibility: 'private', relay: 'relay.openvanilla.app', profile: 'solo' } };
    window.__prog = { idle: false, kind: 'host', running: false, code: 0, step: 'Done', n: 8, of: 8, secs: 400, problem: null };
  });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play'), null, { timeout: 10000 });
  assert.deepEqual(await callsOf(page, 'join_invite'), [{ client: 'c335a-1', link: 'https://relay.openvanilla.app/join/abcdefghijk2345a#k3y' }]);
  assert.deepEqual(await callsOf(page, 'play'), [{ client: 'c335a-1', realm: 'relay.openvanilla.app:40007' }]);
  assert.equal(await page.textContent('#pb-realm'), 'Single player');
  await page.waitForSelector('#sp-manage');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a start that stops: says why, with Try again and Logs', async () => {
  const { page, ctx, errors } = await open({ kit: KIT(null) });
  await openSingle(page);
  await page.click('#sp-start');
  await page.waitForSelector('#sp-prog');
  assert.equal((await callsOf(page, 'hostpack_install')).length, 0, 'the Host pack is there already');
  await page.evaluate(() => { window.__prog = { idle: false, kind: 'host', running: false, code: 1, step: 'Reading the maps', n: 3, of: 8, secs: 90, problem: 'The maps could not be read from your game folder.' }; });
  await page.waitForFunction(() => /Setting up stopped/.test(document.querySelector('#sp-prog').textContent), null, { timeout: 10000 });
  assert.match(await page.textContent('#sp-prog'), /The maps could not be read/);
  await page.click('#sp-prog >> text=Logs');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_logs'));
  assert.equal((await callsOf(page, 'play')).length, 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('set up on this PC: the Manage card: Play, Stop, Start, Logs, Settings (the Host page with it picked); Home says Play', async () => {
  for (const scheme of ['dark', 'light']) {
    const { page, ctx, errors } = await open({ kit: KIT('solo', 'Single player') }, scheme);
    assert.equal(await page.textContent('#hm-single-go'), 'Play');
    await page.click('.side .nav[data-page="single"]');
    await page.waitForFunction(() => document.querySelector('#sp-run') && document.querySelector('#sp-run').textContent === 'Running');
    assert.equal(await page.$('#sp .sp-steps'), null, 'no steps once set up');
    assert.deepEqual(await page.$$eval('#sp-manage .sp-row > button', (l) => l.map((x) => x.textContent)), ['Play', 'Stop', 'Start', 'Logs', 'Settings', 'Invite a friend']);
    assert.ok(await page.isDisabled('#sp-start-again'), 'running: Start is off');
    await shot(page, 'single-manage-' + scheme);
    await wording(page);
    await page.click('#sp-logs-btn');
    await page.waitForFunction(() => document.querySelector('#sp-logs') && /world: ready/.test(document.querySelector('#sp-logs').textContent));
    await page.click('#sp-stop');
    await page.waitForFunction(() => document.querySelector('#sp-run').textContent === 'Stopped');
    assert.equal((await callsOf(page, 'host_stop')).length, 1);
    assert.ok(!(await page.isDisabled('#sp-start-again')));
    await page.click('#sp-start-again');
    await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_start_quiet'));
    assert.deepEqual(await callsOf(page, 'host_start_quiet'), [{ answers: { name: null, visibility: 'private', gm: null } }], 'its name kept');
    await page.evaluate(() => { window.__stopped = false; window.__prog = { idle: false, kind: 'host', running: false, code: 0, step: 'Done', n: 8, of: 8, secs: 30, problem: null }; });
    await page.waitForFunction(() => document.querySelector('#sp-run').textContent === 'Running', null, { timeout: 10000 });
    await page.click('#sp-play');
    await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play'));
    assert.deepEqual(await callsOf(page, 'play'), [{ client: 'c335a-1', realm: 'relay.openvanilla.app:40007' }]);
    await page.evaluate(() => window.LeylineNav.show('single'));
    await page.waitForSelector('#sp-settings');
    await page.click('#sp-settings');
    assert.equal(await pageOn(page), 'page-host');
    assert.deepEqual(errors, []);
    await ctx.close();
  }
});

test('My Den runs on this PC: Start says so, and the Host page asks before it switches the same realm', async () => {
  const { page, ctx, errors } = await open({ kit: KIT('den', 'My Den') });
  await openSingle(page);
  assert.match(await page.textContent('#sp-start-hint'), /Your PC runs My Den now, and one PC runs one realm at a time/);
  await page.click('#sp-start');
  await page.waitForSelector('#hc-confirm[open]');
  assert.equal(await page.textContent('#hc-confirm-title'), 'Switch to Single player?');
  assert.equal((await callsOf(page, 'host_start_quiet')).length, 0, 'nothing switches before the answer');
  await page.click('#hc-confirm-yes');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_start_quiet'));
  assert.deepEqual(await callsOf(page, 'host_start_quiet'), [{ answers: { name: 'Single player', visibility: 'private', gm: null } }]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

// ---------------------------------------------------------------- the Built-in engine (Phase 2)
async function openBuiltin(page) {
  await page.click('.side .nav[data-page="single"]');
  await page.waitForFunction(() => document.querySelector('#page-single.on') && window.__calls.some(([c]) => c === 'engine_status')
    && document.querySelector('#sp-engine-builtin em') && !/Checking/.test(document.querySelector('#sp-engine-builtin em').textContent));
}

test('Built-in (a Windows PC): recommended and picked, Docker not needed; Start sets it up with the folder, its steps with their %, then in', async () => {
  for (const scheme of ['dark', 'light']) {
    const { page, ctx, errors } = await open({ clients: [VANILLA, WRATH], engine: ES_NEW, docker: MISSING }, scheme);
    await openBuiltin(page);
    assert.deepEqual(await states(page), ['folder:done', 'engine:done', 'start:now'], 'no Docker needed');
    assert.equal(await page.getAttribute('#sp-engine-builtin', 'aria-checked'), 'true');
    assert.equal(await page.getAttribute('#sp-engine-builtin', 'aria-disabled'), null);
    assert.equal(await page.textContent('#sp-engine-builtin em'), 'Recommended');
    assert.equal(await page.textContent('#sp-builtin-chip'), 'Downloads at Start');
    assert.equal(await page.getAttribute('#sp-engine-docker', 'aria-checked'), 'false');
    assert.equal(await page.$('#sp-dock-get'), null, "Docker's own buttons only when Docker is the pick");
    assert.match(await page.textContent('#sp-start-hint'), /gets the engine, reads the maps from your game folder and fills the world/);
    await wording(page);
    await shot(page, 'single-builtin-new-' + scheme);
    if (scheme === 'light') { await ctx.close(); continue; }
    await page.click('#sp-start');
    await page.waitForSelector('#sp-prog');
    assert.deepEqual(await callsOf(page, 'engine_setup'), [{ client: 'c335a-1' }], 'the 3.3.5a folder');
    for (const c of ['hostpack_install', 'host_choose', 'host_start_quiet']) assert.equal((await callsOf(page, c)).length, 0, c);
    // the download, with its own %
    await page.evaluate(() => { Object.assign(window.__eprog, { pct: 42, detail: '84 MB of 200 MB', secs: 20 }); });
    await page.waitForFunction(() => document.querySelector('#sp-prog-d') && document.querySelector('#sp-prog-d').textContent === '42% · 84 MB of 200 MB', null, { timeout: 10000 });
    assert.match(await page.textContent('#sp-prog'), /Getting the engine \(a one-time download\)/);
    assert.match(await page.textContent('#sp-prog'), /Step 1 of 7/);
    assert.equal(await page.getAttribute('#sp-step-start', 'data-state'), 'run');
    // unpacking, the maps, the first start
    for (const [n, step, pct, detail] of [[2, 'Checking and unpacking it', null, ''], [3, 'Reading the maps from your game folder', 60, 'Extracting maps'],
      [7, 'Starting the realm', null, 'The world (the first start can take several minutes)']]) {
      await page.evaluate((x) => { Object.assign(window.__eprog, { n: x[0], step: x[1], pct: x[2], detail: x[3] }); }, [n, step, pct, detail]);
      await page.waitForFunction((s) => document.querySelector('#sp-prog') && document.querySelector('#sp-prog').textContent.includes(s), step, { timeout: 10000 });
      assert.match(await page.textContent('#sp-prog'), new RegExp(`Step ${n} of 7`));
      if (pct !== null) assert.match(await page.textContent('#sp-prog-d'), new RegExp(`^${pct}%`));
    }
    await wording(page);
    await shot(page, 'single-builtin-starting-dark');
    // done: set up and running, and in by itself
    await page.evaluate((r) => { window.__es = r; Object.assign(window.__eprog, { running: false, code: 0, step: 'Ready', n: 7, pct: null }); }, ES_READY);
    await page.waitForFunction(() => window.__calls.some(([c]) => c === 'engine_play'), null, { timeout: 10000 });
    assert.deepEqual(await callsOf(page, 'engine_play'), [{ client: 'c335a-1' }]);
    assert.equal((await callsOf(page, 'play')).length, 0, 'not through the relay');
    await page.waitForSelector('#sp-manage[data-engine="builtin"]');
    assert.match(await page.textContent('#sp-login'), /Your login: SOLO · password ABCD2345/);
    assert.deepEqual(errors, []);
    await ctx.close();
  }
});

test('Manage on the Built-in engine: Play, Stop, Start, Logs, Back up, the login with Copy, Update when newer; Home says Play', async () => {
  for (const scheme of ['dark', 'light']) {
    const { page, ctx, errors } = await open({ engine: ES_READY, docker: MISSING }, scheme);
    await page.waitForFunction(() => window.__calls.some(([c]) => c === 'engine_status'));
    await page.waitForFunction(() => document.getElementById('hm-single-go').textContent === 'Play');
    await page.click('.side .nav[data-page="single"]');
    await page.waitForFunction(() => document.querySelector('#sp-run') && document.querySelector('#sp-run').textContent === 'Running');
    assert.equal(await page.$('#sp .sp-steps'), null, 'no steps once set up');
    assert.deepEqual(await page.$$eval('#sp-manage .sp-row > button', (l) => l.map((x) => x.textContent)), ['Play', 'Stop', 'Start', 'Logs', 'Back up']);
    assert.equal(await page.$('#sp-settings'), null, 'the Host page is for the Docker engine');
    assert.ok(await page.isDisabled('#sp-start-again'), 'running: Start is off');
    assert.match(await page.textContent('#sp-manage'), /Built-in engine/);
    await wording(page);
    await shot(page, 'single-builtin-manage-' + scheme);
    if (scheme === 'light') { await ctx.close(); continue; }
    await page.click('#sp-logs-btn');
    await page.waitForFunction(() => document.querySelector('#sp-logs') && /World initialized/.test(document.querySelector('#sp-logs').textContent));
    assert.deepEqual(await callsOf(page, 'engine_logs'), [{ lines: 200 }]);
    assert.equal((await callsOf(page, 'host_logs')).length, 0);
    await page.click('#sp-backup');
    await page.waitForFunction(() => /Backed up: .*single-player-2026-11-04-200509\.zip/.test(document.getElementById('sp-status').textContent));
    await page.click('#sp-login-copy');
    await page.waitForFunction(() => /Password copied|Your password: ABCD2345/.test(document.getElementById('sp-status').textContent));
    // a newer core keeps the password out of the status it polls: Show asks for it (world_password)
    await page.evaluate((r) => { window.__es = Object.assign({}, r, { password: '' }); }, ES_READY);
    await page.evaluate(() => window.LeylineSingle.load());
    await page.waitForSelector('#sp-login-show');
    assert.doesNotMatch(await page.textContent('#sp-login'), /ABCD2345/);
    await page.click('#sp-login-show');
    await page.waitForFunction(() => /password ABCD2345/.test(document.getElementById('sp-login').textContent));
    assert.deepEqual(await callsOf(page, 'world_password'), [{ kind: 'single' }]);
    await page.click('#sp-play');
    await page.waitForFunction(() => window.__calls.some(([c]) => c === 'engine_play'));
    await page.click('#sp-stop');
    await page.waitForFunction(() => document.querySelector('#sp-run').textContent === 'Stopped');
    assert.equal((await callsOf(page, 'engine_stop')).length, 1);
    assert.equal((await callsOf(page, 'host_stop')).length, 0);
    await page.click('#sp-start-again');
    await page.waitForFunction(() => window.__calls.some(([c]) => c === 'engine_start'));
    await page.evaluate((r) => { window.__es = r; Object.assign(window.__eprog, { running: false, code: 0, step: 'Ready', n: 7 }); }, ES_READY);
    await page.waitForFunction(() => document.querySelector('#sp-run').textContent === 'Running', null, { timeout: 10000 });
    // stopped, Play starts it first and then goes in
    await page.click('#sp-stop');
    await page.waitForFunction(() => document.querySelector('#sp-run').textContent === 'Stopped');
    const plays = (await callsOf(page, 'engine_play')).length;
    await page.click('#sp-play');
    await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'engine_start').length === 2);
    await page.evaluate((r) => { window.__es = r; Object.assign(window.__eprog, { running: false, code: 0, step: 'Ready', n: 7 }); }, ES_READY);
    await page.waitForFunction((n) => window.__calls.filter(([c]) => c === 'engine_play').length === n + 1, plays, { timeout: 10000 });
    // a newer engine on the release
    await page.evaluate((r) => { window.__es = Object.assign({}, r, { update: true }); }, ES_READY);
    await page.evaluate(() => window.LeylineSingle.load());
    await page.waitForSelector('#sp-update');
    assert.ok((await callsOf(page, 'world_update_check')).some((a) => a && a.kind === 'single' && !a.force), 'the release looked at (cached in the core)');
    assert.match(await page.textContent('#sp-manage'), /backed up first/);
    // back-ups and Restore on the card
    await page.click('#wt-backups-open-single');
    await page.waitForFunction(() => /2026-11-11 08:00:00.*daily/.test(document.getElementById('wt-backups-single').textContent));
    await page.click('#sp-update');
    await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'engine_setup' && a && a.update === true));
    assert.deepEqual(errors, []);
    await ctx.close();
  }
});

test('Docker can still be picked on a Windows PC: the pick is kept, its checks show, Start goes through the Host pack', async () => {
  const { page, ctx, errors } = await open({ engine: ES_NEW, docker: MISSING });
  await openBuiltin(page);
  await page.click('#sp-engine-docker');
  assert.equal(await page.getAttribute('#sp-engine-docker', 'aria-checked'), 'true');
  assert.equal(await page.getAttribute('#sp-engine-builtin', 'aria-checked'), 'false');
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.single.engine')), 'docker');
  assert.ok(await page.$('#sp-dock-get'), 'Get Docker');
  assert.deepEqual(await states(page), ['folder:done', 'engine:now', 'start:todo']);
  assert.equal(await page.textContent('#sp-start-hint'), 'Docker first (step 2).');
  // and back, with the keyboard
  await page.focus('#sp-engine-builtin');
  await page.keyboard.press('Enter');
  assert.deepEqual(await states(page), ['folder:done', 'engine:done', 'start:now']);
  // Docker picked and ready: Start is the Host pack's, never the engine's
  await page.click('#sp-engine-docker');
  await page.evaluate((d) => { window.__dock = d; }, READY);
  await page.click('#sp-dock-check');
  await page.waitForFunction(() => document.querySelector('#sp-step-engine[data-state="done"]'));
  await page.click('#sp-start');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_start_quiet'));
  assert.equal((await callsOf(page, 'engine_setup')).length, 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a Built-in start that stops: says why, with Try again (resumes) and Logs (the engine log)', async () => {
  const { page, ctx, errors } = await open({ engine: ES_NEW });
  await openBuiltin(page);
  await page.click('#sp-start');
  await page.waitForSelector('#sp-prog');
  await page.evaluate(() => { Object.assign(window.__eprog, { running: false, code: 1, n: 3, step: 'Reading the maps from your game folder', pct: null,
    problem: "That game folder has no Data folder, so its maps can't be read. Pick the folder the game itself is in." }); });
  await page.waitForFunction(() => /Setting up stopped/.test(document.querySelector('#sp-prog').textContent), null, { timeout: 10000 });
  assert.match(await page.textContent('#sp-prog'), /no Data folder/);
  await page.click('#sp-prog >> text=Logs');
  await page.waitForFunction(() => document.querySelector('#sp-logs') && /World initialized/.test(document.querySelector('#sp-logs').textContent));
  await page.click('#sp-prog >> text=Try again');
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'engine_setup').length === 2);
  assert.equal((await callsOf(page, 'engine_play')).length, 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the realm picker: Single player on the Built-in engine is "On this PC" and plays it straight away', async () => {
  const { page, ctx, errors } = await open({ engine: ES_READY });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'engine_status'));
  await page.waitForFunction(() => window.LeylineSingle.builtin());
  await page.click('#pb-realm-btn');
  assert.equal(await page.textContent('#pb-rmenu .pb-solo .pb-opt-sub'), 'On this PC');
  await page.click('#pb-rmenu .pb-solo');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'engine_play'));
  assert.deepEqual(await callsOf(page, 'engine_play'), [{ client: 'c335a-1' }]);
  assert.equal((await callsOf(page, 'host_invite')).length, 0, 'no relay knock');
  assert.deepEqual(errors, []);
  await ctx.close();
});
