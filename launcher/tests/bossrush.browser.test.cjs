// Boss Rush (ui/bossrush.js, host/bossrush.rs; docs/BOSS-RUSH.md), in Chromium with a fake Tauri core:
//   * its side menu item (under Single player) and Home's row lead to its page; nothing is asked of the core before;
//   * the catalog: every fight a tile, heroic only where the raid has one, the light version only where there is one,
//     the hero's class and its specs;
//   * the exact engine: Set up (bossrush_setup with the game folder, its steps from engine_progress kind "bossrush"),
//     Start / Stop, Single player running on the same ports (Stop it and start Boss Rush);
//   * Fight: bossrush_play with the pick (exact), or bossrush_play then Your Den's den_play_lite (light); the login to
//     type when the game can't be signed in by itself; the fight's code to share, a friend's code read back;
//   * the board: best times, the result card after a run with Try again; no brand words, no raw errors.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/bossrush.browser.test.cjs     (BOSSRUSH_SHOTS=<dir> for screenshots)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.BOSSRUSH_SHOTS || '';
const BRANDS = /world of warcraft|warcraft|lich king|blizzard|\bwow\b/i;

const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
// host/bossrush.rs FIGHTS (the test of that file checks they are the module's own)
const FIGHTS = [
  ['frozen-throne', 'The Frozen Throne', 'Icecrown Citadel', 631, true, true],
  ['bone-lord', 'The Bone Lord', 'Icecrown Citadel', 631, true, true],
  ['cult-mistress', 'The Cult Mistress', 'Icecrown Citadel', 631, true, true],
  ['twilight-destroyer', 'The Twilight Destroyer', 'The Ruby Sanctum', 724, true, true],
  ['obsidian-sanctum', 'The Obsidian Sanctum', 'The Obsidian Sanctum', 615, false, true],
  ['obsidian-three', 'The Obsidian Sanctum, three drakes', 'The Obsidian Sanctum', 615, false, false],
  ['brood-mother', "The Brood Mother's Lair", "Onyxia's Lair", 249, false, true],
  ['stone-watcher', 'The Stone Watcher', 'Vault of Archavon', 624, false, true],
  ['storm-watcher', 'The Storm Watcher', 'Vault of Archavon', 624, false, false],
  ['flame-watcher', 'The Flame Watcher', 'Vault of Archavon', 624, false, false],
  ['frost-watcher', 'The Frost Watcher', 'Vault of Archavon', 624, false, false],
  ['spell-weaver', "The Spellweaver's Eye", 'The Eye of Eternity', 616, false, false],
].map(([key, title, raid, map, heroic, light]) => ({ key, title, raid, map, heroic, light, blurb: 'A fight.' }));
const CLASSES = [
  { word: 'warrior', label: 'Warrior', specs: [['arms', 'Arms', 'dps'], ['fury', 'Fury', 'dps'], ['prot', 'Protection', 'tank']] },
  { word: 'mage', label: 'Mage', specs: [['arcane', 'Arcane', 'dps'], ['fire', 'Fire', 'dps'], ['frost', 'Frost', 'dps']] },
  { word: 'priest', label: 'Priest', specs: [['disc', 'Discipline', 'heal'], ['holy', 'Holy', 'heal'], ['shadow', 'Shadow', 'dps']] },
];
const ES_NEW = { supported: true, installed: false, can_get: true, set_up: false, running: false, account: '', password: '', kind: 'boss_rush', realm_name: 'Boss Rush', other_running: null };
const ES_READY = Object.assign({}, ES_NEW, { installed: true, can_get: false, set_up: true, running: true, account: 'RUSH', password: 'ABCD2345' });
const ES_NO = Object.assign({}, ES_NEW, { supported: false, can_get: false });
const BOARD = { bests: [{ fight: 'brood-mother', title: "The Brood Mother's Lair", difficulty: 'normal', engine: 'exact', class: 'mage', spec: '-', ms: 251234, clock: '4:11.2', cleared: true, at: 100, new_best: false }], recent: [] };

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  window.__es = o.es;
  window.__board = o.board;
  window.__prog = { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7, secs: 0, problem: null, pct: null, detail: '' };
  const st = { version: 'test', settings: { clients: o.clients, selected_client: o.clients[0] ? o.clients[0].id : null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'select': return null;
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'docker_info': return { state: 'ready', version: '27', help: '', get_url: '', can_start: false };
      case 'engine_status': return { supported: false, installed: false, can_get: false, set_up: false, running: false, update: false, account: '', password: '' };
      case 'host_dens': return [];
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return true;
      case 'open_link': return null;
      // Boss Rush (host/bossrush.rs, app.rs bossrush_*)
      case 'bossrush_catalog': return { fights: FIGHTS_, classes: CLASSES_, maps: [249, 615, 616, 624, 631, 724], exact: JSON.parse(JSON.stringify(window.__es)), light: true };
      case 'bossrush_board': return JSON.parse(JSON.stringify(window.__board));
      case 'bossrush_setup':
        window.__prog = { idle: false, kind: 'bossrush', running: true, code: null, step: 'Reading the maps from your game folder', n: 3, of: 7, secs: 12, problem: null, pct: 40, detail: 'Extract IcecrownCitadel' };
        return 'Getting the engine…';
      case 'bossrush_start': window.__es = Object.assign({}, window.__es, { running: true, other_running: null }); return 'Starting Boss Rush…';
      case 'bossrush_stop': window.__es = Object.assign({}, window.__es, { running: false }); return 'Stopped. Everyone was saved.';
      case 'engine_progress': return JSON.parse(JSON.stringify(window.__prog));
      case 'engine_stop': return 'Stopped. Everyone was saved.';
      case 'bossrush_play':
        if (args.engine === 'light') return { message: 'Your fight is ready: Den Lite opens it.', engine: 'light', hero: '', made: false, auto: true, account: '', password: '', code: 'ley1.fight.r.Ym9zcw.1234abcd' };
        return { message: 'Started My Wrath on 127.0.0.1:3724.', engine: 'exact', hero: 'Kalinra', made: true, auto: !o.typed, account: o.typed ? 'RUSHBF' : '', password: o.typed ? 'PASS2345XY' : '', code: 'ley1.fight.r.Ym9zcw.1234abcd' };
      case 'den_play_lite': return { message: 'Starting Your Den…', auto: true, why: '', account: 'DEN', password: '' };
      case 'bossrush_code': if (String(args.code).startsWith('ley1.fight.')) return { fight: 'twilight-destroyer', diff: 'heroic', class: 'priest', spec: 'disc', engine: 'exact' }; throw "That isn't a Boss Rush code.";
      default: throw 'unknown ' + cmd;
    }
  } } };
  const FIGHTS_ = o.fights, CLASSES_ = o.classes;
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });
async function open(o, scheme = 'dark') {
  const opts = Object.assign({ clients: [WRATH], es: ES_READY, board: BOARD, fights: FIGHTS, classes: CLASSES, typed: false }, o);
  const ctx = await browser.newContext({ colorScheme: scheme, viewport: { width: 1400, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineBossRush && window.__calls.some(([c]) => c === 'host_info'));
  if (scheme === 'light') await page.evaluate(() => document.documentElement.setAttribute('data-base', 'light'));
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, name + '.png'), fullPage: true }); };
async function toPage(page) {
  await page.click('#hm-bossrush-go');
  await page.waitForSelector('#br .br-fight');
}

test('the entries: Home leads to the page (no side menu item, owner 12 Nov); nothing asked before', async () => {
  const { page, ctx, errors } = await open({});
  const nav = await page.$$eval('aside nav .nav[data-page]', (b) => b.filter((x) => x.offsetParent !== null).map((x) => x.dataset.page));
  assert.deepEqual(nav.slice(0, 3), ['play', 'single', 'realms']);
  assert.equal(await page.$('.nav[data-page="bossrush"]'), null);
  assert.equal((await callsOf(page, 'bossrush_catalog')).length, 0, 'nothing asked until it is opened');
  assert.equal(await page.textContent('#hm-bossrush b'), 'Boss Rush');
  await page.click('#hm-bossrush-go');
  await page.waitForSelector('#br .br-fight');
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-bossrush');
  assert.equal(await page.textContent('#br h2'), 'Boss Rush');
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), BRANDS);
  await shot(page, 'bossrush-dark');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the catalog: every fight a tile; heroic and the light version only where they exist; the hero\'s specs follow the class', async () => {
  const { page, ctx, errors } = await open({});
  await toPage(page);
  assert.equal(await page.locator('#br .br-fight').count(), FIGHTS.length);
  assert.equal(await page.getAttribute('.br-fight[data-fight="frozen-throne"]', 'aria-checked'), 'true', 'the featured fight first and picked');
  assert.ok((await page.textContent('.br-fight[data-fight="brood-mother"]')).includes('Best 4:11.2'), 'its best time on its tile');
  await page.click('.br-seg[data-diff="heroic"]');
  assert.equal(await page.getAttribute('.br-seg[data-diff="heroic"]', 'aria-checked'), 'true');
  await page.click('.br-fight[data-fight="brood-mother"]');
  assert.ok(await page.isDisabled('.br-seg[data-diff="heroic"]'), 'the lair has no heroic');
  assert.equal(await page.getAttribute('.br-seg[data-diff="normal"]', 'aria-checked'), 'true', 'back to normal');
  await page.click('.br-fight[data-fight="spell-weaver"]');
  assert.equal(await page.getAttribute('#br-engine-light', 'aria-disabled'), 'true', 'no light version of the Eye yet');
  await page.selectOption('#br-class', 'priest');
  assert.deepEqual(await page.$$eval('.br-specs .br-seg', (b) => b.map((x) => x.textContent)), ['Discipline', 'Holy', 'Shadow']);
  assert.ok((await page.textContent('.br-what')).includes('Discipline Priest'));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the exact engine: Set up with the game folder, its steps, then Fight', async () => {
  const { page, ctx, errors } = await open({ es: ES_NEW });
  await toPage(page);
  assert.ok(await page.isDisabled('#br-go'), 'no Fight before the set-up');
  await page.click('#br-setup');
  assert.deepEqual(await callsOf(page, 'bossrush_setup'), [{ client: 'c335a-1' }]);
  await page.waitForFunction(() => (document.getElementById('br-prog') || {}).textContent && document.getElementById('br-prog').textContent.includes('Step 3 of 7'));
  assert.ok((await page.textContent('#br-prog')).includes('Reading the maps from your game folder'));
  await page.evaluate(() => { window.__es = Object.assign({}, window.__es, { set_up: true, installed: true, running: true }); window.__prog = { idle: false, kind: 'bossrush', running: false, code: 0, step: 'Ready', n: 7, of: 7, problem: null }; });
  await page.waitForFunction(() => document.getElementById('br-status').textContent.includes('Boss Rush is ready'));
  assert.ok(!(await page.isDisabled('#br-go')));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Fight (exact): the hero and the fight asked for, the code to share; the login to type when it must be typed', async () => {
  const { page, ctx, errors } = await open({ typed: true });
  await toPage(page);
  await page.click('.br-fight[data-fight="twilight-destroyer"]');
  await page.click('.br-seg[data-diff="heroic"]');
  await page.selectOption('#br-class', 'warrior');
  await page.click('.br-seg[data-spec="prot"]');
  await page.click('#br-go');
  await page.waitForFunction(() => document.getElementById('br-status').textContent.includes('Kalinra is made'));
  assert.deepEqual(await callsOf(page, 'bossrush_play'), [{ client: 'c335a-1', fight: 'twilight-destroyer', diff: 'heroic', class: 'warrior', spec: 'prot', engine: 'exact' }]);
  assert.equal(await page.inputValue('#br-code'), 'ley1.fight.r.Ym9zcw.1234abcd');
  assert.ok((await page.textContent('#br-login')).includes('RUSHBF') && (await page.textContent('#br-login')).includes('PASS2345XY'));
  assert.equal((await callsOf(page, 'den_play_lite')).length, 0);
  // the pick is kept for next time
  assert.equal(await page.evaluate(() => window.LeylineBossRush.pick().fight), 'twilight-destroyer');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Fight (light version): the fight written for Den Lite, then Your Den\'s own one-click start', async () => {
  const { page, ctx, errors } = await open({});
  await toPage(page);
  await page.click('.br-fight[data-fight="bone-lord"]');
  await page.click('#br-engine-light');
  assert.equal(await page.textContent('#br-go'), 'Fight (light version)');
  await page.click('#br-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'den_play_lite'));
  assert.equal((await callsOf(page, 'bossrush_play'))[0].engine, 'light');
  assert.deepEqual(await callsOf(page, 'den_play_lite'), [{ client: 'c335a-1' }]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the result card after a run, with Try again; a friend\'s code picks their fight', async () => {
  const { page, ctx, errors } = await open({});
  await toPage(page);
  await page.click('#br-go');
  await page.waitForFunction(() => document.getElementById('br-status').textContent.includes('Kalinra'));
  await page.evaluate(() => { window.__board = { bests: window.__board.bests, recent: [{ fight: 'frozen-throne', title: 'The Frozen Throne', difficulty: 'normal', engine: 'exact', class: 'mage', spec: '-', ms: 612000, clock: '10:12.0', cleared: true, at: 999, new_best: true }] }; });
  await page.evaluate(() => window.LeylineBossRush.load());
  await page.waitForSelector('#br-result');
  assert.ok((await page.textContent('#br-result')).includes('CLEARED') && (await page.textContent('#br-result')).includes('10:12.0'));
  assert.ok((await page.textContent('#br-result')).includes('New best'));
  await page.click('#br-again');
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'bossrush_play').length === 2);
  await page.fill('#br-paste', 'ley1.fight.r.abc.00000000');
  await page.click('#br-use');
  await page.waitForFunction(() => document.querySelector('.br-fight.on') && document.querySelector('.br-fight.on').dataset.fight === 'twilight-destroyer');
  assert.equal(await page.getAttribute('.br-seg[data-diff="heroic"]', 'aria-checked'), 'true');
  await page.fill('#br-paste', 'nonsense');
  await page.click('#br-use');
  await page.waitForFunction(() => document.getElementById('br-status').classList.contains('bad'));
  assert.doesNotMatch(await page.textContent('#br-status'), /unknown|undefined/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Single player running on the same ports: Stop it and start Boss Rush; not on Windows: the light version only', async () => {
  const { page, ctx, errors } = await open({ es: Object.assign({}, ES_READY, { running: false, other_running: 'single' }) });
  await toPage(page);
  assert.ok((await page.textContent('#br-exact')).includes('Single player is running'));
  await page.click('#br-stop-other');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'bossrush_start'));
  assert.equal((await callsOf(page, 'engine_stop')).length, 1);
  await ctx.close();
  const b = await open({ es: ES_NO });
  await toPage(b.page);
  assert.ok((await b.page.textContent('#br-exact')).includes('made for Windows PCs'));
  await b.page.click('#br-engine-light');
  assert.ok(!(await b.page.isDisabled('#br-go')), 'the light version plays here');
  await shot(b.page, 'bossrush-light');
  assert.deepEqual(errors.concat(b.errors), []);
  await b.ctx.close();
});
