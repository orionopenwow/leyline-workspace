// Game mods and realms' mods (ui/gamemods.js, docs/GAME-MODS.md) in Chromium with a fake Tauri core: Mods › Discover ›
// Game mods lists the Leyline Modes and the Workshop's game-category mods, each with what it needs and "Play it on" from
// the realms' own mod lists (live ones only); the join offer (one click: "Get them and play") installs only catalog mods
// with a client part through mod_install, asks for native code per mod, and never installs without the click.
// Screenshots go to $GAMEMODS_SHOTS when set.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/gamemods.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const SHOTS = process.env.GAMEMODS_SHOTS || '';
const P = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: `${name} does a thing.`, category: 'addons',
  builds: ['3.3.5a'], author: 'Leyline', author_url: '', licence: 'MIT', repo: '', page: `https://play.openvanilla.app/wrath/mods/${id.slice(4)}`,
  credits: name, stars: null, last_push: '', version: '0.1.0', kind: 'ui', permissions: [], on_wrath: true,
  download: 'https://play.openvanilla.app/wrath/mods/x/download', sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
const CATALOG = { categories: [{ id: 'addons', name: 'Addons and UI' }, { id: 'sandbox', name: 'Sandbox and game modes' }],
  projects: [P('mod:pings', 'Pings', { kind: 'both' }), P('mod:dalaran-dice', 'Dalaran Dice', { category: 'sandbox', kind: 'both' }),
    P('mod:hud', 'Hud', { category: 'tools', kind: 'realm', on_wrath: false }),
    P('mod:native', 'Native Thing', { on_wrath: false }), P('mod:nohash', 'No Hash', { sha256: null, on_wrath: false })],
  from_cache: false, saved_at: 1, error: null };
const MODE = (id, title, extra) => Object.assign({ id, title, line: `${title} is fun.`, kind: 'realm', players: { min: 1, max: 8 }, status: 'live',
  start: { go: 'mode:' + id }, credit: null }, extra || {});
const MODES = [MODE('minigolf', 'Minigolf'), MODE('bumper-balls', 'Bumper Balls'), MODE('coin-dash', 'Coin Dash'), MODE('community', 'Community modes'),
  MODE('brawl', 'Wrath Brawl', { kind: 'realm-type' }), MODE('arena-1v1', '1v1 arena', { kind: 'in-game' })];

const FAKE = ({ catalog, modes, realms, installedMods, addonsInstalled }) => {
  const calls = (window.__calls = []);
  try {
    localStorage.setItem('leyline.page', 'discover');
    localStorage.removeItem('leyline.dcView');
  } catch (_) { /* fine */ }
  let mods = installedMods || [];
  let nativeOk = false;
  const base = { official: false, kind: 'PvE', ruleset: 'main', state: 'live', version: '3.3.5a', core: 'azerothcore', build: '3.3.5a', tags: [],
    region: null, region_estimated: false, language: null, max_players: null, uptime_24h: null, uptime_7d: null, is_new: false, score: 0,
    featured: false, verified: false, direct: false, rules: [], claimed: true, players: 1, bots: 0, tagline: '', summary: '', join_url: null,
    discord: null, relay: 'relay.openvanilla.app', mods: [] };
  const list = realms.map((r) => Object.assign({}, base, r));
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c1', name: 'My Wrath', path: '/x', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c1', selected_realm: 'play.openvanilla.app', wine: { enabled: true, command: 'wine', prefix: '' }, count_installs: true },
        official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: false, data_dir: '/tmp' };
      case 'mods_catalog': return catalog;
      case 'mods_ranked': throw 'offline';
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods };
      case 'mod_install': {
        if (args.project === 'mod:native' && !nativeOk) throw 'NATIVE_CONFIRM:' + JSON.stringify({ key: 'mod:native', name: 'Native Thing', files: ['x.dll'] });
        const p = catalog.projects.find((x) => x.id === args.project);
        mods = mods.concat([{ key: p.id, name: p.name, version: p.version, source: p.source, update: null, in_catalog: true, enabled: true, files: [], addons: [], patches: [] }]);
        return { key: p.id, name: p.name, version: p.version, warnings: [], kept: [] };
      }
      case 'mod_native_confirm': nativeOk = true; return null;
      case 'deep_link_take': return null;
      case 'mods_updates': return [];
      case 'realms_list': return { realms: list, network: { realms: 2, players: 2, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'host_info': return { dir: '', is_kit: false, problems: [], kit: null, seeds: [], realm: null };
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'modes_list': return { modes, from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'addons_status': return { pack: { version: '1', addons: ['WrathUnleashedUI'] }, from_cache: false, error: null, prefs: { auto_update: true, dont_ask: false },
        clients: [{ client: 'c1', installed: !!addonsInstalled, update: false, missing: [] }] };
      case 'addons_should_offer': return false;
      case 'addons_install': return { installed: true };
      case 'play_realm': return 'Started.';
      case 'play': return 'Started.';
      case 'open_url': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

const OFFICIAL = { slug: 'official', name: 'Wrath Unleashed', official: true, group: 'official', address: 'play.openvanilla.app',
  mods: [{ id: 'mode:minigolf' }, { id: 'mode:bumper-balls' }, { id: 'mode:coin-dash' }, { id: 'mode:community' },
    { id: 'mod:dalaran-dice', version: '0.1.0' }, { id: 'mod:pings', version: '0.1.0' }] };
const FROST = { slug: 'c-aaaaaaaaaaaaaaaa', realm_id: 'aaaaaaaaaaaaaaaa', name: 'Frost Haven', group: 'community', address: 'relay.openvanilla.app:40002',
  mods: [{ id: 'mode:coin-dash' }, { id: 'mod:dalaran-dice', version: '0.1.0' }, { id: 'mod:hud' }, { id: 'up:not-in-catalog' }, { id: 'https://evil.example/x.zip' }] };
const ASLEEP = { slug: 'c-bbbbbbbbbbbbbbbb', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Asleep', state: 'offline', group: 'offline', mods: [{ id: 'mode:bumper-balls' }] };

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(opts = {}, viewport) {
  const page = await browser.newPage({ viewport: viewport || { width: 1280, height: 860 } });
  const errors = [];
  page.on('pageerror', (e) => { errors.push(String(e)); if (process.env.DBG) console.log(e.stack); });
  await page.addInitScript(FAKE, Object.assign({ catalog: CATALOG, modes: MODES, realms: [OFFICIAL, FROST, ASLEEP] }, opts));
  await page.goto(INDEX);
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length > 0 && realmsCache.length > 0);
  return { page, errors };
}
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, name + '.png') }); };
const calls = (page, name) => page.evaluate((n) => window.__calls.filter(([c]) => c === n).map(([, a]) => a), name);

test('Game mods tab: the Leyline Modes and game-category mods, what each needs, Play it on from live realms', async () => {
  const { page, errors } = await open();
  await page.click('#discover-tabs .ly-tab[data-tab="game"]');
  await page.waitForFunction(() => document.querySelectorAll('#game-discover .gm-card').length >= 5);
  await shot(page, 'launcher-game-mods');
  const ids = await page.$$eval('#game-discover .gm-card', (c) => c.map((x) => x.dataset.id));
  assert.deepEqual(ids, ['mode:minigolf', 'mode:bumper-balls', 'mode:coin-dash', 'mode:community', 'mod:dalaran-dice'], 'realm modes and game categories only');
  const on = async (id) => page.textContent(`#game-discover .gm-card[data-id="${id}"] .gm-on`);
  assert.equal(await on('mode:minigolf'), 'Play it on: Wrath Unleashed');
  assert.equal(await on('mode:coin-dash'), 'Play it on: Wrath Unleashed, Frost Haven');
  assert.equal(await on('mod:dalaran-dice'), 'Play it on: Wrath Unleashed, Frost Haven');
  assert.equal(await page.isVisible('#cat-grid'), false, 'the other Discover parts are hidden');
  const tags = await page.$$eval('#game-discover .gm-card[data-id="mode:minigolf"] .gm-tag', (t) => t.map((x) => x.textContent));
  assert.deepEqual(tags, ['Needs a realm that allows it', 'Plays through the Leyline addons']);
  const dtags = await page.$$eval('#game-discover .gm-card[data-id="mod:dalaran-dice"] .gm-tag', (t) => t.map((x) => x.textContent));
  assert.deepEqual(dtags, ['Needs a realm that allows it', 'Adds files to your game']);
  assert.equal(await page.textContent('#game-discover .gm-card[data-id="mod:dalaran-dice"] .acts .btn.primary'), 'Install');
  assert.equal(await page.textContent('#game-discover .gm-card[data-id="mode:minigolf"] .acts .btn.primary'), 'Play');
  assert.doesNotMatch(await page.textContent('#game-discover'), /Asleep|Coming|skating/i, 'offline realms and made-up entries never shown');
  assert.deepEqual(errors, []);
  await page.close();
});

test('the official realm without a list from the site: its own (modes it runs, Workshop mods on it)', async () => {
  const { page, errors } = await open({ realms: [Object.assign({}, OFFICIAL, { mods: [] })] });
  const got = await page.evaluate(() => window.LeylineGameMods.realmMods(realmsCache.find((r) => r.official)).map((m) => m.id));
  assert.deepEqual(got, ['mode:minigolf', 'mode:bumper-balls', 'mode:coin-dash', 'mode:community', 'mod:pings', 'mod:dalaran-dice']);
  assert.deepEqual(errors, []);
  await page.close();
});

// ------------------------------------------------------------------ joining a realm: the one-click offer
async function playOn(page, name) {
  await page.click('.nav[data-page="realms"]');
  await page.waitForFunction(() => document.querySelectorAll('#rl-list .realm-card').length > 0);
  const card = page.locator('#rl-list .realm-card', { has: page.locator(`.name:text-is("${name}")`) });
  await card.locator('.rl-play').click();
  return card;
}

test('realm cards say how many mods a realm uses (bad ids never counted)', async () => {
  const { page, errors } = await open();
  await page.click('.nav[data-page="realms"]');
  await page.waitForFunction(() => document.querySelectorAll('#rl-list .realm-card .rl-mods').length === 3);
  const card = page.locator('#rl-list .realm-card', { has: page.locator('.name:text-is("Frost Haven")') });
  assert.equal(await card.locator('.rl-mods').textContent(), 'Uses 4 mods');
  assert.equal(await card.locator('.rl-mods').getAttribute('title'), 'Coin Dash, Dalaran Dice 0.1.0, Hud, not-in-catalog');
  await shot(page, 'launcher-realms-mods');
  assert.deepEqual(errors, []);
  await page.close();
});

test('join: one offer for the client mods this game lacks; "Get them and play" installs only those, then plays', async () => {
  const { page, errors } = await open();
  await playOn(page, 'Frost Haven');
  await page.waitForSelector('#gm-dlg[open]');
  await shot(page, 'launcher-join-offer');
  assert.equal(await page.textContent('#gm-dlg h3'), 'Frost Haven uses 2 mods your game doesn\'t have');
  assert.deepEqual(await page.$$eval('#gm-dlg .mk-list li', (l) => l.map((x) => x.textContent)), ['Dalaran Dice', 'Leyline addons (game modes, golf)'],
    'realm-only (Hud) and unknown ids (not in the catalog, a URL) are never offered');
  assert.equal((await calls(page, 'mod_install')).length, 0, 'nothing before the click');
  await page.click('#gm-dlg .btn.primary');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play_realm'));
  assert.deepEqual((await calls(page, 'mod_install')).map((a) => a.project), ['mod:dalaran-dice']);
  assert.equal((await calls(page, 'addons_install')).length, 1, 'the Leyline addons, for the modes');
  const order = await page.evaluate(() => window.__calls.map(([c]) => c).filter((c) => ['mod_install', 'addons_install', 'play_realm'].includes(c)));
  assert.deepEqual(order, ['mod_install', 'addons_install', 'play_realm'], 'installed first, then PLAY');
  assert.deepEqual(errors, []);
  await page.close();
});

test('join: Play without them installs nothing; Cancel stops PLAY; nothing to ask when the game has them', async () => {
  let { page, errors } = await open();
  await playOn(page, 'Frost Haven');
  await page.waitForSelector('#gm-dlg[open]');
  await page.click('#gm-dlg .btn:text-is("Play without them")');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play_realm'));
  assert.equal((await calls(page, 'mod_install')).length + (await calls(page, 'addons_install')).length, 0);
  assert.deepEqual(errors, []);
  await page.close();
  ({ page, errors } = await open());
  await playOn(page, 'Frost Haven');
  await page.waitForSelector('#gm-dlg[open]');
  await page.click('#gm-dlg .btn:text-is("Cancel")');
  await page.waitForTimeout(300);
  assert.equal((await calls(page, 'play_realm')).length + (await calls(page, 'mod_install')).length, 0, 'Cancel: no PLAY, no install');
  await page.close();
  ({ page, errors } = await open({ addonsInstalled: true, installedMods: [{ key: 'mod:dalaran-dice', name: 'Dalaran Dice', version: '0.1.0', source: 'packaged',
    update: null, in_catalog: true, enabled: true, files: [], addons: [], patches: [] }] }));
  await playOn(page, 'Frost Haven');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play_realm'));
  assert.equal(await page.locator('#gm-dlg[open]').count(), 0, 'no offer when nothing is missing');
  assert.deepEqual(errors, []);
  await page.close();
});

test('join: a native-code mod still asks its own one-time OK; saying no skips it and PLAY goes on', async () => {
  const realms = [OFFICIAL, Object.assign({}, FROST, { name: 'Native Realm', mods: [{ id: 'mod:native' }, { id: 'mod:nohash' }] })];
  const { page, errors } = await open({ realms, addonsInstalled: true });
  await playOn(page, 'Native Realm');
  await page.waitForSelector('#gm-dlg[open]');
  assert.deepEqual(await page.$$eval('#gm-dlg .mk-list li', (l) => l.map((x) => x.textContent)), ['Native Thing'], 'a mod without a hash is never offered');
  await page.click('#gm-dlg .btn.primary');
  await page.waitForSelector('#native-dlg[open]');
  assert.equal(await page.textContent('#native-name'), 'Native Thing');
  await page.click('#native-no');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play_realm'));
  assert.equal((await calls(page, 'mod_native_confirm')).length, 0);
  assert.equal((await calls(page, 'mod_install')).length, 1, 'tried once, refused by the core until the OK');
  assert.deepEqual(errors, []);
  await page.close();
});
