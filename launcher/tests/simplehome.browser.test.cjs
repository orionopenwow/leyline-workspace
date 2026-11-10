// Simple mode (ui/simplehome.js): the four-button home, in Chromium with a fake Tauri core.
//   * the switch in the side menu's foot turns it on: only Home (the four buttons), the switch, Settings and Help stay
//     in the side menu, the play bar goes; "Show everything" turns it off again; it is remembered;
//   * Play goes through window.LeylineWorlds.run (world_status, world_start, world_play), and no game folder sends the
//     player to the Single player page; another world running asks "Stop it and play";
//   * Host opens Play with friends; Visit opens Realms' invite box or the realm list;
//   * Back up backs up every world that is set up (world_backup), says how many are kept, and "Back up every day"
//     backs up once by itself when the last back-up is over 20 hours old (and not again);
//   * no brand names in what it shows.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/simplehome.browser.test.cjs     (SIMPLE_SHOTS=<dir> for screenshots)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.SIMPLE_SHOTS || '';
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
      case 'world_status': return JSON.parse(JSON.stringify(Object.assign({}, window.__ws[args.kind], { kind: args.kind })));
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
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });
async function open(o) {
  const opts = Object.assign({ clients: [WRATH], worlds: { single: WS({}), vanilla: WS({}), bossrush: WS({}) }, storage: {} }, o);
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: 1300, height: 860 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineSimpleHome && window.LeylineWorlds);
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const visible = (page, sel) => page.evaluate((sel) => { const e = document.querySelector(sel); return !!e && getComputedStyle(e).display !== 'none' && e.offsetParent !== null; }, sel);
async function shot(page, name) { if (!SHOTS) return; await page.waitForTimeout(400); await page.screenshot({ path: path.join(SHOTS, name + '.png') }); }

test('the switch turns Simple mode on and off, and it is remembered', async () => {
  const { page, ctx, errors } = await open({});
  assert.equal(await visible(page, '.nav[data-page="simple"]'), false);
  assert.equal(await visible(page, '.nav[data-page="realms"]'), true);
  await page.click('#nav-simple-toggle');
  await page.waitForSelector('#page-simple.on #sh-play-go');
  assert.equal(await visible(page, '.nav[data-page="simple"]'), true);
  assert.equal(await visible(page, '.nav[data-page="realms"]'), false);
  assert.equal(await visible(page, '.playbar'), false);
  assert.equal(await visible(page, '.nav[data-page="settings"]'), true);
  assert.equal(await page.textContent('#nav-simple-toggle span'), 'Show everything');
  const titles = await page.$$eval('.sh-title', (h) => h.map((x) => x.textContent));
  assert.deepEqual(titles, ['Play', 'Host', 'Visit', 'Back up']);
  const text = await page.textContent('#sh');
  assert.doesNotMatch(text, /world of warcraft|\bwow\b|warcraft|blizzard|3\.3\.5|1\.12/i);
  await shot(page, 'simple-on');
  // remembered across a reload
  await page.reload();
  await page.waitForFunction(() => window.LeylineSimpleHome && document.querySelector('#page-simple.on #sh-play-go'));
  assert.equal(await page.evaluate(() => document.documentElement.classList.contains('ll-simple')), true);
  await page.click('#nav-simple-toggle');
  assert.equal(await page.evaluate(() => document.documentElement.classList.contains('ll-simple')), false);
  assert.equal(await visible(page, '.nav[data-page="realms"]'), true);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Play sets up and starts the world through the launcher\'s own path', async () => {
  const { page, ctx, errors } = await open({ storage: { 'leyline.simple': '1', 'leyline.simple.daily': '0' } });
  await page.waitForSelector('#page-simple.on #sh-play-go');
  assert.match(await page.textContent('#sh-play-go'), /Set up and play/);
  await page.click('#sh-play-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual((await callsOf(page, 'world_setup'))[0], { client: 'c335a-1', kind: 'single' });
  await page.waitForFunction(() => /Started/.test(document.getElementById('sh-status').textContent));
  assert.match(await page.textContent('#sh-play-state'), /Running now/);
  // the other world: picked, then Play asks to stop the running one
  await page.selectOption('#sh-world', 'vanilla');
  await page.evaluate(() => { window.__ws.vanilla = Object.assign({}, window.__ws.vanilla, { set_up: true, other_running: 'single' }); });
  await page.click('#sh-play-go');
  await page.waitForSelector('#sh-stop-other');
  assert.match(await page.textContent('#sh-status'), /Single player is running/);
  await page.click('#sh-stop-other');
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'world_play').length === 2, null, { timeout: 15000 });
  assert.equal((await callsOf(page, 'world_play'))[1].kind, 'vanilla');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Play with no game folder goes to the Single player page', async () => {
  const { page, ctx } = await open({ clients: [], storage: { 'leyline.simple': '1', 'leyline.simple.daily': '0' } });
  await page.waitForSelector('#page-simple.on #sh-play-go');
  await page.click('#sh-play-go');
  await page.waitForSelector('#page-single.on');
  await ctx.close();
});

test('Host opens Play with friends; Visit opens the invite box and the realm list', async () => {
  const { page, ctx, errors } = await open({ storage: { 'leyline.simple': '1', 'leyline.simple.daily': '0' } });
  await page.waitForSelector('#page-simple.on #sh-host-go');
  await page.click('#sh-host-go');
  await page.waitForFunction(() => { const r = document.querySelector('#wf-invite'); return r && r.closest('[hidden]') === null; });
  await page.keyboard.press('Escape');
  await page.evaluate(() => window.LeylineHostFriends.close && window.LeylineHostFriends.close());
  await page.click('#sh-visit-code');
  await page.waitForSelector('#page-realms.on');
  assert.equal(await page.evaluate(() => document.activeElement && document.activeElement.id), 'rl-invite');
  await page.evaluate(() => window.LeylineNav.show('simple'));
  await page.click('#sh-visit-find');
  await page.waitForSelector('#page-realms.on');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Back up backs up every world that is set up, and the daily back-up runs once by itself', async () => {
  const worlds = { single: WS({ set_up: true, backups: 2 }), vanilla: WS({ set_up: true, backups: 0 }), bossrush: WS({}) };
  const old = String(Date.now() - 30 * 3600 * 1000);
  const { page, ctx, errors } = await open({ worlds, storage: { 'leyline.simple': '1', 'leyline.simple.lastBackup': old } });
  await page.waitForSelector('#page-simple.on #sh-backup-go');
  // daily: last back-up 30 h ago, so once by itself, for both worlds
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'world_backup').length === 2);
  assert.deepEqual((await callsOf(page, 'world_backup')).map((a) => a.kind).sort(), ['single', 'vanilla']);
  await page.waitForFunction(() => /daily back-up is done/.test(document.getElementById('sh-status').textContent));
  assert.match(await page.textContent('#sh-backup-state'), /4 back-ups on this PC/);
  // not again on the next visit
  await page.evaluate(() => window.LeylineNav.show('play'));
  await page.evaluate(() => window.LeylineNav.show('simple'));
  await page.waitForTimeout(300);
  assert.equal((await callsOf(page, 'world_backup')).length, 2);
  // by hand
  await page.click('#sh-backup-go');
  await page.waitForFunction(() => /Backed up 2 worlds/.test(document.getElementById('sh-status').textContent));
  assert.equal((await callsOf(page, 'world_backup')).length, 4);
  assert.match(await page.textContent('#sh-backup-last'), /Last back-up: just now/);
  await shot(page, 'simple-backup');
  // turning the daily back-up off is remembered
  await page.uncheck('#sh-daily');
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.simple.daily')), '0');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('nothing set up: Back up is off, says so, and backs nothing up', async () => {
  const { page, ctx } = await open({ storage: { 'leyline.simple': '1' } });
  await page.waitForSelector('#page-simple.on #sh-backup-go');
  assert.equal(await page.isDisabled('#sh-backup-go'), true);
  assert.match(await page.textContent('#sh-backup-state'), /Nothing to back up yet/);
  await page.evaluate(() => window.LeylineSimpleHome.backUp(false));
  await page.waitForFunction(() => /nothing to back up yet/i.test(document.getElementById('sh-status').textContent));
  assert.equal((await callsOf(page, 'world_backup')).length, 0);
  await ctx.close();
});
