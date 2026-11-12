// The realm picker's own worlds (app.js worldRow, pickWorld, playWorld; owner 10 Nov "robust single player in
// launcher", after a player on Discord couldn't select his solo server): on the Built-in engine, Single player and the
// original world are rows like any realm. A click picks it (selected, the bar names it and says what PLAY does); PLAY
// plays it through window.LeylineWorlds.run (world_setup / world_start, world_play), never the plain `play`; another
// world running: PLAY says so and a second PLAY stops it and plays; no Built-in engine here: the Docker row as before.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/realmworlds.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const WS = (o) => Object.assign({ supported: true, installed: true, set_up: false, running: false, backups: 0, other_running: null, account: '', password: '' }, o);

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try {
    if (sessionStorage.getItem('__booted')) throw 'reload: keep what the page stored';
    sessionStorage.setItem('__booted', '1');
    localStorage.clear();
    localStorage.setItem('leyline.mode', 'online');
    for (const [k, v] of Object.entries(o.storage || {})) localStorage.setItem(k, v);
  } catch (_) { /* fine */ }
  window.__ws = o.worlds;
  window.__eprog = { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7, secs: 0, problem: null, pct: null, detail: '' };
  const st = { version: 'test', settings: { clients: o.clients, selected_client: o.clients[0] ? o.clients[0].id : null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  try { const sel = JSON.parse(sessionStorage.getItem('__sel') || 'null'); if (sel) { st.settings.selected_client = sel[0]; st.settings.selected_realm = sel[1]; } } catch (_) { /* fine */ }
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'social_status': return true;
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'docker_info': return { state: 'ready', version: '27', help: '', get_url: '', can_start: false };
      case 'engine_status': return JSON.parse(JSON.stringify(window.__ws.single));
      case 'world_status': return window.__ws[args.kind] ? JSON.parse(JSON.stringify(Object.assign({}, window.__ws[args.kind], { kind: args.kind }))) : null;
      case 'select': st.settings.selected_client = args.client; st.settings.selected_realm = args.realm;
        try { sessionStorage.setItem('__sel', JSON.stringify([args.client, args.realm])); } catch (_) { /* fine */ }
        return null;
      case 'play': return 'Started.';
      case 'world_start': case 'world_setup':
        window.__ws[args.kind] = Object.assign({}, window.__ws[args.kind], { set_up: true, running: true });
        return 'Starting…';
      case 'engine_progress': return JSON.parse(JSON.stringify(window.__eprog));
      case 'world_play': return { message: 'Started.', kind: args.kind, auto: true, account: 'SOLO', password: '', made: '' };
      case 'world_stop': case 'engine_stop':
        for (const k of Object.keys(window.__ws)) window.__ws[k] = Object.assign({}, window.__ws[k], { running: false, other_running: null });
        return 'Stopped.';
      case 'world_backup': window.__ws[args.kind].backups = (window.__ws[args.kind].backups || 0) + 1; return 'C:\\Leyline\\backups\\single-player-2026-11-10.zip';
      case 'world_invite': return { link: 'https://relay.openvanilla.app/join/abc#k', code: 'ABCD-2345' };
      case 'world_choices': return { races: [], first_era: false, name: 'Hero' };
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(o) {
  const opts = Object.assign({ clients: [WRATH], worlds: { single: WS({ set_up: true }), vanilla: WS({ set_up: true }), bossrush: WS({}) }, storage: {} }, o);
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: 1300, height: 860 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineWorlds && window.__calls.some(([c]) => c === 'world_status'));
  await page.waitForTimeout(300);
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
async function menuRows(page) {
  await page.click('#pb-realm-btn');
  await page.waitForSelector('#pb-rmenu:not([hidden]) .pb-world');
  return page.$$eval('#pb-rmenu [role="option"], #pb-rmenu li.pb-local, #pb-rmenu li.pb-realm-opt', (l) => l.map((x) => [x.querySelector('.pb-opt-name') ? x.querySelector('.pb-opt-name').textContent : '', x.getAttribute('aria-selected')]));
}

test('both worlds are rows; a click picks one, and the bar says so', async () => {
  const { page, ctx, errors } = await open({});
  const rows = await menuRows(page);
  const names = rows.map((r) => r[0]);
  assert.ok(names.includes('Single player') && names.includes('Original world'), names.join(', '));
  await page.click('#pb-rmenu .pb-world-vanilla');
  await page.waitForFunction(() => document.getElementById('pb-realm').textContent === 'Original world');
  assert.equal(await page.textContent('#play-headline'), 'READY TO PLAY');
  assert.match(await page.textContent('#pb-detail'), /Original world, on this PC/);
  assert.equal(await page.textContent('#pb-realm-ver'), '3.3.5a');
  // reopened: it's the selected row
  await page.click('#pb-realm-btn');
  await page.waitForSelector('#pb-rmenu:not([hidden]) .pb-world-vanilla');
  assert.equal(await page.getAttribute('#pb-rmenu .pb-world-vanilla', 'aria-selected'), 'true');
  assert.equal(await page.getAttribute('#pb-rmenu .pb-world-single', 'aria-selected'), 'false');
  // nothing was started by picking it
  assert.equal((await callsOf(page, 'world_play')).length, 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('PLAY plays the picked world through the one local path, never the plain play', async () => {
  const { page, ctx, errors } = await open({});
  await menuRows(page);
  await page.click('#pb-rmenu .pb-world-single');
  await page.click('#play-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.equal((await callsOf(page, 'world_play'))[0].kind, 'single');
  assert.deepEqual((await callsOf(page, 'world_start'))[0], { kind: 'single' });
  assert.equal((await callsOf(page, 'play')).length, 0);
  await page.waitForFunction(() => /Taking you in/.test(document.getElementById('play-status').textContent));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('remembered across a restart, and the bar says what PLAY will do', async () => {
  const { page, ctx } = await open({ worlds: { single: WS({}), vanilla: WS({ set_up: true }), bossrush: WS({}) } });
  await menuRows(page);
  await page.click('#pb-rmenu .pb-world-single');
  await page.waitForFunction(() => document.getElementById('play-headline').textContent === 'PLAY SETS IT UP');
  await page.reload();
  await page.waitForFunction(() => window.LeylineWorlds && document.getElementById('pb-realm').textContent === 'Single player');
  await page.waitForFunction(() => document.getElementById('play-headline').textContent === 'PLAY SETS IT UP');
  await ctx.close();
});

test('another world running: PLAY says so, a second PLAY stops it and plays', async () => {
  const worlds = { single: WS({ set_up: true, running: true }), vanilla: WS({ set_up: true, other_running: 'single' }), bossrush: WS({}) };
  const { page, ctx, errors } = await open({ worlds });
  await menuRows(page);
  await page.click('#pb-rmenu .pb-world-vanilla');
  await page.click('#play-go');
  await page.waitForFunction(() => /Press PLAY again/.test(document.getElementById('play-status').textContent), null, { timeout: 10000 });
  assert.equal((await callsOf(page, 'world_play')).length, 0);
  await page.waitForFunction(() => !document.getElementById('play-go').disabled, null, { timeout: 15000 });
  await page.click('#play-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.ok((await callsOf(page, 'engine_stop')).length + (await callsOf(page, 'world_stop')).length >= 1);
  assert.equal((await callsOf(page, 'world_play'))[0].kind, 'vanilla');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('no Built-in engine on this PC: Single player keeps its Docker row, no original-world row', async () => {
  const worlds = { single: WS({ supported: false }), vanilla: WS({ supported: false }), bossrush: WS({ supported: false }) };
  const { page, ctx, errors } = await open({ worlds });
  await page.click('#pb-realm-btn');
  await page.waitForSelector('#pb-rmenu:not([hidden]) .pb-solo');
  assert.equal(await page.locator('#pb-rmenu .pb-world').count(), 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});
