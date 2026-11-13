// Leaderboards (ui/boards.js, src-tauri/src/boards.rs; docs/LEADERBOARDS.md), in Chromium with a fake Tauri core:
//   * mounted on the Boss Rush page (#br-boards) and the Modes page (#md-boards); nothing asked of the core until a
//     mount's page is open;
//   * the list kit's tabs (Boss Rush, Duels, Local runs), the choices (when, fight, difficulty, board, realm), the
//     search; ranks: Boss Rush by time (each character's best), duels by rating / points, the 1v1 arena by wins;
//   * every row marked with the realm that signed it (Official / Community), local runs marked "not checked";
//   * "That's me": your rows lit and marked You, the Only you chip, on every mount;
//   * this week's featured fight (the line, and Boss Rush's tile); Share a best time onto the local-run board;
//   * the boards not reachable, a board that didn't check; no brand words, no raw errors.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/boards.browser.test.cjs     (BOARDS_SHOTS=<dir> for screenshots)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.BOARDS_SHOTS || '';
const BRANDS = /world of warcraft|warcraft|lich king|blizzard|\bwow\b/i;

const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const FIGHTS = [['frozen-throne', 'The Frozen Throne', true, true], ['bone-lord', 'The Bone Lord', true, true], ['cult-mistress', 'The Cult Mistress', true, true]]
  .map(([key, title, heroic, light]) => ({ key, title, raid: 'Icecrown Citadel', map: 631, heroic, light, blurb: 'A fight.' }));
const CLASSES = [{ word: 'mage', label: 'Mage', specs: [['arcane', 'Arcane', 'dps'], ['fire', 'Fire', 'dps'], ['frost', 'Frost', 'dps']] }];
const ES = { supported: true, installed: true, can_get: false, set_up: true, running: true, account: 'RUSH', password: 'ABCD2345', kind: 'boss_rush', realm_name: 'Boss Rush', other_running: null };
const BOARD = { bests: [{ fight: 'bone-lord', title: 'The Bone Lord', difficulty: 'normal', engine: 'exact', class: 'mage', spec: 'fire', ms: 245000, clock: '4:05.0', cleared: true, at: 1794500000, new_best: false }], recent: [] };

const WEEK = 1794182400;          // Monday 9 Nov 2026 00:00 UTC
const NOW = 1794510000;
const OFF = { id: 'k3x9q2m4p7w8r5t2', name: 'Wrath Unleashed', kind: 'official', signed: true };
const COM = { id: 'frosthavenaaaa22', name: 'Frost Haven', kind: 'community', signed: true };
const br = (name, cls, ms, at, realm, more) => Object.assign({ mode: 'bossrush', board: 'frozen-throne', diff: 'normal', kind: 'bots', name, class: cls, ms,
  size: 10, bots: 9, party: [], at, run: ms, realm }, more || {});
const du = (board, period, name, cls, rating, wins, losses, points, realm) => ({ mode: 'duels', board, period, name, class: cls, rating, wins, losses, points, at: NOW, realm });
const DATA = {
  week: WEEK, now: NOW, featured: 'frozen-throne', featured_local: 'cult-mistress', skipped: 0, from_cache: false,
  realms: [{ id: OFF.id, name: OFF.name, kind: 'official', ts: NOW }, { id: COM.id, name: COM.name, kind: 'community', ts: NOW }],
  rows: [
    br('Orion', 'deathknight', 400000, WEEK + 3600, OFF, { deaths: 2 }),
    br('Orion', 'deathknight', 350000, WEEK - 86400, OFF),
    br('Jaina', 'mage', 380000, WEEK + 7200, OFF, { kind: 'group', bots: 0, deaths: 0 }),
    br('Valeera', 'rogue', 390000, WEEK + 9000, COM),
    br('Thrall', 'shaman', 300000, WEEK + 100, OFF, { diff: 'heroic' }),
    du('duel', 'all', 'Thrall', 'shaman', 1700, 3, 3, 12, OFF),
    du('duel', 'all', 'Orion', 'deathknight', 1640, 8, 2, 26, OFF),
    du('duel', 'week:2026-11-09', 'Jaina', 'mage', 0, 5, 0, 15, OFF),
    du('duel', 'week:2026-11-09', 'Orion', 'deathknight', 0, 3, 0, 9, OFF),
    du('arena-1v1', 'week:2026-11-09', 'Sylvana', 'hunter', 0, 3, 1, 0, COM),
  ],
  local: [{ mode: 'bossrush', board: 'frozen-throne', diff: 'normal', engine: 'light', name: 'Kalinra', class: 'priest', ms: 333000, at: WEEK + 500, local: true }],
};

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  window.__boards = o.boards;
  window.__boardsFail = o.fail;
  const st = { version: 'test', settings: { clients: [o.client], selected_client: o.client.id, selected_realm: 'play.openvanilla.app',
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
      case 'engine_progress': return { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7, secs: 0, problem: null, pct: null, detail: '' };
      case 'bossrush_catalog': return { fights: o.fights, classes: o.classes, maps: [631], exact: o.es, light: true };
      case 'bossrush_board': return JSON.parse(JSON.stringify(o.board));
      // the boards (boards.rs)
      case 'boards_get':
        if (window.__boardsFail) throw window.__boardsFail;
        return JSON.parse(JSON.stringify(window.__boards));
      case 'boards_share_local':
        if (args.name === 'Nologin') throw 'signed_out';
        return 'On the local-run board.';
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });
async function open(o, scheme = 'dark') {
  const opts = Object.assign({ client: WRATH, boards: DATA, fail: null, fights: FIGHTS, classes: CLASSES, es: ES, board: BOARD }, o);
  const ctx = await browser.newContext({ colorScheme: scheme, viewport: { width: 1400, height: 1000 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineBoards && window.__calls.some(([c]) => c === 'host_info'));
  if (scheme === 'light') await page.evaluate(() => document.documentElement.setAttribute('data-base', 'light'));
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const shot = async (page, name, sel) => {
  if (!SHOTS) return;
  if (sel) await (await page.$(sel)).screenshot({ path: path.join(SHOTS, name + '.png') });
  else await page.screenshot({ path: path.join(SHOTS, name + '.png'), fullPage: true });
};
const names = (page, mount) => page.$$eval(`#${mount} .lb-row`, (rs) => rs.map((r) => r.querySelector('.lb-name').textContent));
async function toBossRush(page) {
  await page.click('#hm-bossrush-go');
  await page.waitForSelector('#br .br-fight');
  await page.waitForSelector('#br-boards .lb-row');
}

test('mounted on the Boss Rush page: asked only once the page opens; this week by time, each character once; the realm that signed each row', async () => {
  const { page, ctx, errors } = await open({});
  assert.ok(await page.$('#br-boards.lb') && await page.$('#md-boards.lb'), 'both mounts are there');
  assert.equal((await callsOf(page, 'boards_get')).length, 0, 'nothing asked before a page with a board opens');
  await toBossRush(page);
  assert.equal((await callsOf(page, 'boards_get')).length, 1);
  assert.equal(await page.textContent('#br-boards-feat'), "This week's featured fight: The Frozen Throne");
  assert.equal(await page.getAttribute('#br-boards-tab-bossrush', 'aria-selected'), 'true');
  assert.deepEqual(await names(page, 'br-boards'), ['Jaina', 'Valeera', 'Orion'], 'this week by time; last week\'s faster run not this week');
  const first = await page.textContent('#br-boards .lb-row:nth-child(1)');
  assert.ok(first.includes('6:20.0') && first.includes('Wrath Unleashed') && first.includes('Official') && first.includes('0 deaths'), first);
  const orion = await page.textContent('#br-boards .lb-row:nth-child(3)');
  assert.ok(orion.includes('6:40.0') && orion.includes('2 deaths') && orion.includes('10 · 9 companions'), orion);
  assert.ok((await page.textContent('#br-boards .lb-row:nth-child(2)')).includes('Community'));
  assert.match(await page.getAttribute('#br-boards .lb-row:nth-child(1) .lb-mark', 'title'), /Signed by Wrath Unleashed \(k3x9q2m4p7w8r5t2\); the signature was checked on this PC/);
  assert.ok((await page.textContent('#br-boards-note')).includes('signed by the realm that recorded it'));
  // the week's featured fight on Boss Rush's own tile (the catalog in turn, for this PC's fights)
  assert.ok((await page.textContent('.br-fight[data-fight="cult-mistress"]')).includes('This week'));
  assert.ok(!(await page.textContent('.br-fight[data-fight="frozen-throne"]')).includes('This week'));
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), BRANDS);
  await shot(page, 'boards-bossrush-dark', '#br-boards');
  await shot(page, 'boards-bossrush-page-dark');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the choices: all time, heroic, a realm, a name; Duels by rating, by this week\'s points, the 1v1 arena by wins', async () => {
  const { page, ctx, errors } = await open({});
  await toBossRush(page);
  await page.selectOption('#br-boards-period', 'all');
  assert.deepEqual(await names(page, 'br-boards'), ['Orion', 'Jaina', 'Valeera'], 'all time: last week\'s faster run counts');
  assert.ok((await page.textContent('#br-boards .lb-row:nth-child(1)')).includes('5:50.0'));
  await page.selectOption('#br-boards-diff', 'heroic');
  assert.deepEqual(await names(page, 'br-boards'), ['Thrall']);
  await page.selectOption('#br-boards-diff', 'normal');
  await page.selectOption('#br-boards-realm', COM.id);
  assert.deepEqual(await names(page, 'br-boards'), ['Valeera']);
  await page.selectOption('#br-boards-realm', '');
  await page.fill('#br-boards .lk-search input', 'jai');
  await page.waitForFunction(() => document.querySelectorAll('#br-boards .lb-row').length === 1);
  assert.deepEqual(await names(page, 'br-boards'), ['Jaina']);
  await page.fill('#br-boards .lk-search input', '');
  await page.click('#br-boards-tab-duels');
  await page.waitForFunction(() => document.querySelectorAll('#br-boards .lb-row').length === 2);
  assert.deepEqual(await names(page, 'br-boards'), ['Thrall', 'Orion'], 'all time by rating');
  assert.equal(await page.$('#br-boards-fight'), null, 'no fight choice on Duels');
  assert.ok((await page.textContent('#br-boards .lb-row:nth-child(1) .lb-val')).includes('1700'));
  await page.selectOption('#br-boards-period', 'week');
  assert.deepEqual(await names(page, 'br-boards'), ['Jaina', 'Orion'], 'this week by points');
  assert.equal(await page.textContent('#br-boards .lb-row:nth-child(1) .lb-val'), '15');
  await page.selectOption('#br-boards-duel', 'arena-1v1');
  assert.deepEqual(await names(page, 'br-boards'), ['Sylvana']);
  assert.equal(await page.textContent('#br-boards .lb-row:nth-child(1) .lb-sub'), '3-1');
  await shot(page, 'boards-duels-dark', '#br-boards');
  // the choices are kept for next time
  assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('leyline.boards.sel')).duel), 'arena-1v1');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('That\'s me: your rows lit and marked You on every board, and the Only you chip', async () => {
  const { page, ctx, errors } = await open({});
  await toBossRush(page);
  await page.click('#br-boards .lb-row:nth-child(3) .lb-mebtn');
  assert.ok(await page.$eval('#br-boards .lb-row:nth-child(3)', (r) => r.classList.contains('lb-me')));
  assert.equal(await page.textContent('#br-boards .lb-row:nth-child(3) .lb-you'), 'You');
  assert.equal(await page.getAttribute('#br-boards .lb-row:nth-child(3) .lb-mebtn', 'aria-pressed'), 'true');
  await page.click('#br-boards .lk-chip:has-text("Only you")');
  assert.deepEqual(await names(page, 'br-boards'), ['Orion']);
  await page.click('#br-boards .lk-chip:has-text("Only you")');
  // the same name on the same realm is you on the duel boards too
  await page.click('#br-boards-tab-duels');
  const lit = await page.$$eval('#br-boards .lb-row.lb-me .lb-name', (n) => n.map((x) => x.textContent));
  assert.deepEqual(lit, ['Orion']);
  // and on the Modes page's board
  await page.evaluate(() => window.LeylineNav.show('modes'));
  await page.waitForSelector('#md-boards .lb-row');
  assert.ok((await page.$$eval('#md-boards .lb-row.lb-me .lb-name', (n) => n.map((x) => x.textContent))).includes('Orion'));
  assert.equal((await callsOf(page, 'boards_get')).length, 1, 'one read for both mounts');
  await shot(page, 'boards-you-modes-dark', '#md-boards');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Local runs: apart, marked not checked; Share a best time from Boss Rush onto the local-run board', async () => {
  const { page, ctx, errors } = await open({});
  await toBossRush(page);
  await page.click('#br-boards-tab-local');
  await page.waitForFunction(() => document.querySelectorAll('#br-boards .lb-row').length === 1);
  const row = await page.textContent('#br-boards .lb-row');
  assert.ok(row.includes('Kalinra') && row.includes('Local run') && row.includes('not checked by a server') && row.includes('Light version'), row);
  assert.ok((await page.textContent('#br-boards-note')).includes('kept apart from the signed boards'));
  // never on a signed board
  await page.click('#br-boards-tab-bossrush');
  assert.ok(!(await names(page, 'br-boards')).includes('Kalinra'));
  // Share from Boss Rush's best times
  await page.fill('#br-share-name', 'Orion');
  await page.click('.br-share-run[data-fight="bone-lord"]');
  await page.waitForSelector('#br-share-msg');
  assert.equal(await page.textContent('#br-share-msg'), 'On the local-run board.');
  assert.deepEqual(await callsOf(page, 'boards_share_local'), [{ fight: 'bone-lord', diff: 'normal', engine: 'exact', name: 'Orion', class: 'mage', spec: 'fire', ms: 245000, at: 1794500000 }]);
  assert.equal((await callsOf(page, 'boards_get')).length, 2, 'the boards read again after a share');
  assert.equal(await page.inputValue('#br-share-name'), 'Orion', 'the name is kept');
  await page.fill('#br-share-name', 'x');
  await page.click('.br-share-run[data-fight="bone-lord"]');
  await page.waitForFunction(() => document.getElementById('br-share-msg').textContent.includes('2 to 12 letters'));
  await page.fill('#br-share-name', 'Nologin');
  await page.click('.br-share-run[data-fight="bone-lord"]');
  await page.waitForFunction(() => document.getElementById('br-share-msg').textContent.includes('Log in to share'));
  assert.equal((await callsOf(page, 'boards_share_local')).length, 2, 'a bad name never reaches the core');
  await shot(page, 'boards-local-dark', '#page-bossrush');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the Modes page board (light theme); the boards not reachable; a board that didn\'t check; from the last copy', async () => {
  let { page, ctx, errors } = await open({ boards: Object.assign({}, DATA, { skipped: 1, from_cache: true }) }, 'light');
  await page.evaluate(() => window.LeylineNav.show('modes'));
  await page.waitForSelector('#md-boards .lb-row');
  assert.deepEqual(await names(page, 'md-boards'), ['Jaina', 'Valeera', 'Orion']);
  const note = await page.textContent('#md-boards-note');
  assert.ok(note.includes("1 board didn't check and isn't shown") && note.includes('Offline: the last boards Leyline saw'), note);
  await shot(page, 'boards-modes-light', '#md-boards');
  assert.deepEqual(errors, []);
  await ctx.close();
  ({ page, ctx, errors } = await open({ fail: 'Can\'t reach leylinelauncher.com: connection refused (os error 111)' }));
  await page.evaluate(() => window.LeylineNav.show('modes'));
  await page.waitForFunction(() => /Try again/.test((document.querySelector('#md-boards .lb-list') || {}).textContent || ''));
  const text = await page.textContent('#md-boards .lb-list');
  assert.doesNotMatch(text, /os error|refused/, 'never a raw error');
  await page.evaluate(() => { window.__boardsFail = null; });
  await page.click('#md-boards .lb-list button:has-text("Try again")');
  await page.waitForSelector('#md-boards .lb-row');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the ranking matches the website\'s rules (boards.js rank)', async () => {
  const { page, ctx } = await open({});
  const out = await page.evaluate((d) => {
    const r = window.LeylineBoards.rank(d, { period: 'all', fight: 'frozen-throne', diff: 'normal', duel: 'duel', realm: '' });
    return r.map((x) => [x.tab, x.rank, x.name]);
  }, DATA);
  assert.deepEqual(out, [['bossrush', 1, 'Orion'], ['bossrush', 2, 'Jaina'], ['bossrush', 3, 'Valeera'], ['duels', 1, 'Thrall'], ['duels', 2, 'Orion'],
    ['local', 1, 'Kalinra']]);
  assert.equal(await page.evaluate(() => window.LeylineBoards.clock(312345)), '5:12.3');
  await ctx.close();
});
