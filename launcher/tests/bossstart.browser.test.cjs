// Boss Rush, start anywhere (ui/bossstart.js; host/bossrush.rs RAIDS, app.rs bossrush_start_at / _state / _retry /
// _go_now / _leave / _invite; docs/BOSS-RUSH.md "Start anywhere"), in Chromium with a fake Tauri core:
//   * its part sits on the Boss Rush page above Boss Rush's own card (#bs over #br) and asks nothing before the page;
//   * where: a tab per raid; the whole raid, each wing and each boss as the list kit's rows, grouped by wing, searchable,
//     with the keyboard; heroic only where the raid has one; 10 or 25;
//   * who: With companions (the roles the companions bring, as icons with counts, following the hero's role and the
//     size) or With friends (Invite a friend: the friend's hero, the link; companions in the seats left, or not);
//   * one Start: bossrush_start_at with the pick; then the raid as the realm says (ready, fighting, wiped with Retry from
//     this boss, cleared with Again; waiting for friends with Start now; Leave);
//   * no brand words, no raw errors, no page errors; screenshots (BOSSSTART_SHOTS=<dir>, the gallery's bossstart*.png).
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/bossstart.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.BOSSSTART_SHOTS || '';
const BRANDS = /world of warcraft|warcraft|lich king|blizzard|\bwow\b/i;

const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
// host/bossrush.rs FIGHTS and RAIDS (that file's tests check they are the module's own)
const FIGHTS = [
  ['frozen-throne', 'The Frozen Throne', 'Icecrown Citadel', 631, true], ['bone-lord', 'The Bone Lord', 'Icecrown Citadel', 631, true],
  ['cult-mistress', 'The Cult Mistress', 'Icecrown Citadel', 631, true], ['twilight-destroyer', 'The Twilight Destroyer', 'The Ruby Sanctum', 724, true],
  ['obsidian-sanctum', 'The Obsidian Sanctum', 'The Obsidian Sanctum', 615, false], ['obsidian-three', 'The Obsidian Sanctum, three drakes', 'The Obsidian Sanctum', 615, false],
  ['brood-mother', "The Brood Mother's Lair", "Onyxia's Lair", 249, false], ['stone-watcher', 'The Stone Watcher', 'Vault of Archavon', 624, false],
  ['storm-watcher', 'The Storm Watcher', 'Vault of Archavon', 624, false], ['flame-watcher', 'The Flame Watcher', 'Vault of Archavon', 624, false],
  ['frost-watcher', 'The Frost Watcher', 'Vault of Archavon', 624, false], ['spell-weaver', "The Spellweaver's Eye", 'The Eye of Eternity', 616, false],
  ['icc-gunship', 'The Gunship Battle', 'Icecrown Citadel', 631, true], ['icc-deathbringer', 'The Deathbringer', 'Icecrown Citadel', 631, true],
  ['icc-bloat', 'The Bloated Horror', 'Icecrown Citadel', 631, true], ['icc-ooze', 'The Ooze Horror', 'Icecrown Citadel', 631, true],
  ['icc-professor', 'The Mad Professor', 'Icecrown Citadel', 631, true], ['icc-council', 'The Blood Council', 'Icecrown Citadel', 631, true],
  ['icc-blood-queen', 'The Blood Queen', 'Icecrown Citadel', 631, true], ['icc-dreamwalker', 'The Dreamwalker', 'Icecrown Citadel', 631, true],
  ['icc-frost-queen', 'The Frost Queen', 'Icecrown Citadel', 631, true],
].map(([key, title, raid, map, heroic]) => ({ key, title, raid, map, heroic, light: false, blurb: 'A fight of ' + raid + '.' }));
const W = (key, title, fights) => ({ key, title, fights });
const RAIDS = [
  { key: 'icc', title: 'Icecrown Citadel', map: 631, heroic: true, whole: 'icc-raid',
    fights: ['bone-lord', 'cult-mistress', 'icc-gunship', 'icc-deathbringer', 'icc-bloat', 'icc-ooze', 'icc-professor', 'icc-council', 'icc-blood-queen', 'icc-dreamwalker', 'icc-frost-queen', 'frozen-throne'],
    wings: [W('icc-spire', 'The Lower Spire', ['bone-lord', 'cult-mistress', 'icc-deathbringer']), W('icc-plague', 'The Plagueworks', ['icc-bloat', 'icc-ooze', 'icc-professor']),
      W('icc-crimson', 'The Crimson Hall', ['icc-council', 'icc-blood-queen']), W('icc-frostwing', 'The Frostwing Halls', ['icc-dreamwalker', 'icc-frost-queen']), W('', 'The Frozen Throne', ['frozen-throne'])] },
  { key: 'rs', title: 'The Ruby Sanctum', map: 724, heroic: true, whole: 'rs-raid', fights: ['twilight-destroyer'], wings: [W('', 'The Ruby Sanctum', ['twilight-destroyer'])] },
  { key: 'os', title: 'The Obsidian Sanctum', map: 615, heroic: false, whole: '', fights: ['obsidian-sanctum', 'obsidian-three'], wings: [W('', 'The Obsidian Sanctum', ['obsidian-sanctum'])] },
  { key: 'ony', title: "Onyxia's Lair", map: 249, heroic: false, whole: '', fights: ['brood-mother'], wings: [W('', "Onyxia's Lair", ['brood-mother'])] },
  { key: 'voa', title: 'Vault of Archavon', map: 624, heroic: false, whole: 'voa-raid', fights: ['stone-watcher', 'storm-watcher', 'flame-watcher', 'frost-watcher'], wings: [W('', 'Vault of Archavon', ['stone-watcher'])] },
  { key: 'eoe', title: 'The Eye of Eternity', map: 616, heroic: false, whole: '', fights: ['spell-weaver'], wings: [W('', 'The Eye of Eternity', ['spell-weaver'])] },
];
const CLASSES = [
  { word: 'warrior', label: 'Warrior', specs: [['arms', 'Arms', 'dps'], ['fury', 'Fury', 'dps'], ['prot', 'Protection', 'tank']] },
  { word: 'mage', label: 'Mage', specs: [['arcane', 'Arcane', 'dps'], ['fire', 'Fire', 'dps'], ['frost', 'Frost', 'dps']] },
  { word: 'priest', label: 'Priest', specs: [['disc', 'Discipline', 'heal'], ['holy', 'Holy', 'heal'], ['shadow', 'Shadow', 'dps']] },
];
const ES_READY = { supported: true, installed: true, can_get: false, set_up: true, running: true, account: 'RUSH', password: 'ABCD2345', kind: 'boss_rush', realm_name: 'Boss Rush', other_running: null };
const ES_NEW = Object.assign({}, ES_READY, { installed: false, set_up: false, running: false, can_get: true });
const RUN = (o) => Object.assign({ kind: 'run', start: 'frozen-throne', diff: 'normal', phase: 'ready', leader: 'Kalinra', map: 631, instance: 7, x: 517, y: -2124.6, z: 840.9,
  dist: 1.5, members: 10, bots: 9, inside: 10, step: 'frozen-throne', step_title: 'The Frozen Throne', done: 0, of: 1, boss: 0, wipes: 0, clears: 0, marks: [], last: '', ms: 0, clock: '0:00.0' }, o);

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  window.__es = o.es;
  window.__state = o.state;
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
      case 'engine_progress': return { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7, secs: 0, problem: null, pct: null, detail: '' };
      case 'bossrush_catalog': return { fights: o.fights, raids: o.raids, classes: o.classes, maps: [249, 615, 616, 624, 631, 724], exact: JSON.parse(JSON.stringify(window.__es)), light: true };
      case 'bossrush_board': return { bests: [], recent: [] };
      case 'bossrush_state': return JSON.parse(JSON.stringify(window.__state));
      case 'bossrush_start_at':
        window.__state = args.party === 'friends' ? { kind: 'lobby', start: args.start, diff: args.diff, waiting: 1, size: args.size }
          : Object.assign({}, window.__state.kind === 'run' ? window.__state : {}, { kind: 'run', start: args.start, phase: 'ready', members: args.size, bots: args.size - 1, inside: args.size, step: args.start });
        return { message: 'Started My Wrath on 127.0.0.1:3724.', engine: 'exact', hero: 'Kalinra', made: false, auto: true, account: '', password: '', code: '' };
      case 'bossrush_retry': window.__state = Object.assign({}, window.__state, { phase: 'ready', step: 'icc-ooze', step_title: 'The Ooze Horror' }); return { kind: 'ok', word: 'retry icc-ooze' };
      case 'bossrush_go_now': window.__state = { kind: 'run', start: 'icc-plague', diff: 'normal', phase: 'forming', leader: 'Kalinra', members: 10, bots: 8, inside: 2, step: 'icc-bloat', step_title: 'The Bloated Horror', done: 0, of: 3, ms: 0, clock: '0:00.0', marks: [] }; return { kind: 'ok', word: 'go' };
      case 'bossrush_leave': window.__state = RUN_IDLE; return { kind: 'ok', word: 'left' };
      case 'bossrush_invite': return { link: 'leyline://join#abc~~GTEST12345~~lf1', code: 'den1.abc', name: 'your friend', expires: 1, tunnel: {}, ways: [] };
      default: throw 'unknown ' + cmd;
    }
  } } };
  const RUN_IDLE = { kind: 'run', start: '', diff: '', phase: 'idle', members: 0, bots: 0, inside: 0, step: '', done: 0, of: 0, ms: 0, clock: '0:00.0', marks: [] };
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });
async function open(o = {}, scheme = 'dark', viewport = { width: 1400, height: 1000 }) {
  const opts = Object.assign({ clients: [WRATH], es: ES_READY, state: { kind: 'offline' }, fights: FIGHTS, raids: RAIDS, classes: CLASSES }, o);
  const ctx = await browser.newContext({ colorScheme: scheme, viewport });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineBossStart && window.__calls.some(([c]) => c === 'host_info'));
  if (scheme === 'light') await page.evaluate(() => document.documentElement.setAttribute('data-base', 'light'));
  return { page, ctx, errors };
}
async function toPage(page) {
  if (await page.isVisible('#hm-bossrush-go')) await page.click('#hm-bossrush-go');
  else await page.evaluate(() => window.LeylineNav.show('bossrush'));   // a phone's Home has no room for the row
  await page.waitForSelector('#bs .bs-row');
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const shot = async (page, name) => { if (SHOTS) await page.locator('#bs').screenshot({ path: path.join(SHOTS, name + '.png') }); };
const rows = (page) => page.$$eval('#bs .lk-body .bs-row', (r) => r.map((x) => x.id.replace('bs-start-', '')));

test('on the Boss Rush page above its card, nothing asked before; the citadel: the whole raid, its wings, every boss', async () => {
  const { page, ctx, errors } = await open();
  assert.equal((await callsOf(page, 'bossrush_catalog')).length, 0, 'nothing asked until the page opens');
  await toPage(page);
  assert.equal(await page.evaluate(() => document.getElementById('bs').nextElementSibling.id), 'br', 'its own part, above Boss Rush\'s card');
  assert.equal(await page.textContent('#bs h2'), 'Start anywhere');
  assert.deepEqual(await page.$$eval('.bs-raid', (b) => b.map((x) => x.textContent)), RAIDS.map((r) => r.title));
  assert.equal(await page.getAttribute('#bs-raid-icc', 'aria-selected'), 'true');
  const keys = await rows(page);
  for (const k of ['icc-raid', 'icc-spire', 'icc-plague', 'icc-crimson', 'icc-frostwing', ...RAIDS[0].fights]) assert.ok(keys.includes(k), k);
  assert.equal(keys.length, 1 + 4 + 12, 'the whole raid, four wings, twelve bosses');
  assert.deepEqual(await page.$$eval('#bs .lk-group-l', (g) => g.map((x) => x.textContent)),
    ['The whole raid', 'The Lower Spire', 'The Plagueworks', 'The Crimson Hall', 'The Frostwing Halls', 'The Frozen Throne', 'More fights']);
  assert.ok((await page.textContent('#bs-start-icc-plague')).includes('Wing · 3 bosses'));
  assert.ok((await page.textContent('#bs-start-icc-raid')).includes('Whole raid'));
  assert.ok(await page.$eval('#bs-start-frozen-throne', (e) => e.classList.contains('on')), 'the final fight picked first');
  assert.ok((await page.textContent('#bs-what')).includes('The Frozen Throne · Normal · 10 players · with companions'));
  assert.doesNotMatch(await page.evaluate(() => document.getElementById('bs').innerText), BRANDS);
  await page.mouse.move(0, 0);
  await shot(page, 'bossstart-picker');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('where and how: a wing by click, a boss by keys, search; heroic only where it exists; 25 players; the companions\' roles', async () => {
  const { page, ctx, errors } = await open();
  await toPage(page);
  await page.click('#bs-start-icc-plague');
  assert.ok((await page.textContent('#bs-what')).startsWith('The Plagueworks (wing)'));
  // the keyboard: down from the wing is its first boss
  await page.focus('#bs-start-icc-plague');
  await page.keyboard.press('ArrowDown');
  assert.equal(await page.evaluate(() => window.LeylineBossStart.pick().start), 'icc-bloat');
  // companions: you (damage) and 2 tanks, 3 healers, 4 damage; at 25: 3, 6, 15; a healer hero: 3, 5, 16
  const team = () => page.$$eval('#bs-team .bs-role b', (b) => b.map((x) => Number(x.textContent)));
  assert.deepEqual(await team(), [2, 3, 4]);
  await page.click('#bs-size-25');
  assert.deepEqual(await team(), [3, 6, 15]);
  await page.selectOption('#bs-class', 'priest');
  assert.deepEqual(await page.$$eval('[id^="bs-spec-"]', (b) => b.map((x) => x.textContent)), ['Discipline', 'Holy', 'Shadow']);
  assert.deepEqual(await team(), [3, 5, 16]);
  assert.equal(await page.getAttribute('#bs-team .bs-role.heal', 'aria-label'), '5 healers');
  // search
  await page.fill('#bs .lk-search input', 'queen');
  await page.waitForFunction(() => document.querySelectorAll('#bs .lk-body .bs-row').length === 2);
  assert.deepEqual(await rows(page), ['icc-blood-queen', 'icc-frost-queen']);
  await page.fill('#bs .lk-search input', '');
  // a raid with no heroic and no wings: the vault (its whole raid and its four watchers)
  await page.click('#bs-raid-voa');
  assert.ok(await page.isDisabled('#bs-diff-heroic'));
  assert.deepEqual(await rows(page), ['voa-raid', 'stone-watcher', 'storm-watcher', 'flame-watcher', 'frost-watcher']);
  await page.click('#bs-raid-ony');
  assert.deepEqual(await rows(page), ['brood-mother'], 'one fight, no whole raid of its own');
  assert.equal(await page.evaluate(() => window.LeylineBossStart.pick().diff), 'normal');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Start with companions: bossrush_start_at with the pick; the raid as the realm says; a wipe, Retry from this boss', async () => {
  const { page, ctx, errors } = await open();
  await toPage(page);
  await page.click('#bs-raid-icc');
  await page.click('#bs-start-icc-plague');
  await page.click('#bs-diff-heroic');
  await page.selectOption('#bs-class', 'warrior');
  await page.click('#bs-spec-prot');
  await page.click('#bs-start');
  await page.waitForFunction(() => document.getElementById('bs-status').textContent.includes('Kalinra is ready'));
  assert.deepEqual(await callsOf(page, 'bossrush_start_at'), [{ client: 'c335a-1', start: 'icc-plague', diff: 'heroic', size: 10, party: 'companions', companions: true, class: 'warrior', spec: 'prot' }]);
  // the realm's word, read every few seconds
  await page.evaluate(() => { window.__state = { kind: 'run', start: 'icc-plague', diff: 'heroic', phase: 'fighting', leader: 'Kalinra', map: 631, instance: 9, dist: 30, members: 10, bots: 9, inside: 10,
    step: 'icc-bloat', step_title: 'The Bloated Horror', done: 1, of: 3, boss: 1, wipes: 0, clears: 0, marks: [], last: '', ms: 95000, clock: '1:35.0' }; });
  await page.waitForSelector('#bs-state.fighting', { timeout: 8000 });
  assert.ok((await page.textContent('#bs-state')).includes('1 of 3 bosses down'));
  assert.ok((await page.textContent('#bs-state')).includes('1:35.0'));
  assert.equal(await page.$('#bs-leave'), null, 'no Leave while fighting');
  await page.evaluate(() => { window.__state = Object.assign({}, window.__state, { phase: 'wiped', wipes: 1, last: 'wipe', step: 'icc-ooze', step_title: 'The Ooze Horror' }); });
  await page.waitForSelector('#bs-retry', { timeout: 8000 });
  assert.equal(await page.textContent('#bs-retry'), 'Retry from this boss');
  await shot(page, 'bossstart-wiped');
  await page.click('#bs-retry');
  await page.waitForSelector('#bs-state.ready');
  assert.equal((await callsOf(page, 'bossrush_retry')).length, 1);
  assert.ok((await page.textContent('#bs-state')).includes('at The Ooze Horror'), 'in front of the boss still standing');
  assert.ok((await page.textContent('#bs-status')).includes('Back in front of the boss'));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('With friends: the friend\'s hero, Invite a friend (the link), Start waits for them, Start now; companions or players only', async () => {
  const { page, ctx, errors } = await open();
  await toPage(page);
  await page.click('#bs-start-icc-plague');
  await page.click('#bs-party-friends');
  assert.ok(await page.isVisible('#bs-friends'));
  assert.deepEqual(await page.$$eval('#bs-team .bs-role b', (b) => b.map((x) => Number(x.textContent))), [2, 2, 4], 'you (damage) and your friend (holy priest): companions in the seats left');
  await page.selectOption('#bs-f-class', 'warrior');
  await page.click('#bs-f-spec-prot');
  await page.click('#bs-invite');
  await page.waitForSelector('#bs-link');
  assert.deepEqual(await callsOf(page, 'bossrush_invite'), [{ start: 'icc-plague', diff: 'normal', size: 10, companions: true, friendClass: 'warrior', friendSpec: 'prot' }]);
  assert.equal(await page.inputValue('#bs-link'), 'leyline://join#abc~~GTEST12345~~lf1');
  await page.mouse.move(0, 0);
  await shot(page, 'bossstart-friends');
  // players only: no companions
  await page.uncheck('#bs-fill');
  assert.ok((await page.textContent('#bs-team')).includes('Players only'));
  await page.check('#bs-fill');
  await page.click('#bs-start');
  await page.waitForSelector('#bs-state.lobby');
  assert.equal((await callsOf(page, 'bossrush_start_at'))[0].party, 'friends');
  assert.ok((await page.textContent('#bs-state')).includes('Waiting for friends'));
  assert.ok((await page.textContent('#bs-state')).includes('1 of 10 here'));
  await page.click('#bs-go-now');
  await page.waitForSelector('#bs-state.forming');
  assert.deepEqual(await callsOf(page, 'bossrush_go_now'), [{ companions: true }]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('not ready: no Start before Boss Rush is set up and running; light theme, phone width', async () => {
  const { page, ctx, errors } = await open({ es: ES_NEW }, 'light', { width: 420, height: 900 });
  await page.evaluate(() => document.documentElement.classList.add('phone'));
  await toPage(page);
  assert.ok(await page.isDisabled('#bs-start'));
  assert.ok((await page.textContent('.bs-go')).includes('Set up and start Boss Rush below first'));
  assert.equal((await callsOf(page, 'bossrush_state')).length, 0, 'no realm to ask');
  await shot(page, 'bossstart-phone-light');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('an older launcher core without the raid list says so; an older realm answers kindly', async () => {
  const { page, ctx, errors } = await open({ raids: undefined });
  await page.click('#hm-bossrush-go');
  await page.waitForFunction(() => (document.getElementById('bs') || {}).textContent.includes('no raid list yet'));
  assert.deepEqual(errors, []);
  await ctx.close();
});
