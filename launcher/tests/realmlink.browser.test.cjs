// Paste to add and Live now (ui/realmlink.js, with app.js barImport and home.js), in Chromium with a fake Tauri core.
//
// Each kind of realm text, on the Realms page's "Paste a realm link or address" box and on the play bar's realm box:
// a share link (/wrath/r/<short>, /r/<id>), leyline://realm/<id>, a join link, a host:port address, the official
// realm's link. Recognised: the realm is in Your realms (the play bar's pick), the client it needs is picked
// (leyline.clientFor), and Play now starts it (never by itself). Unknown text: one plain line. A game folder or a zip
// in the play bar still goes to the game-folder import. Home: Live now (players, then uptime; mode badge; Play) and
// New this week (new on the list with a player in the last day), and their plain empty states.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/realmlink.browser.test.cjs
//   SHOTS=<dir> … also saves screenshots (dark and light)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');

const REALMS = [
  { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 5, bots: 0, address: 'play.openvanilla.app', ruleset: 'main' },
  { slug: 'c-frostpeakaaaaaaa', realm_id: 'frostpeakaaaaaaa', name: 'Frost Peak', official: false, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 12, bots: 0, address: 'relay.openvanilla.app:40001', ruleset: 'vanillaplus', uptime_7d: 99, is_new: false },
  { slug: 'c-oldlandsbbbbbbbb', realm_id: 'oldlandsbbbbbbbb', name: 'Old Lands', official: false, state: 'live', build: '1.12.1', version: '1.12.1', core: 'vmangos', tags: [], players: 12, bots: 0, address: 'relay.openvanilla.app:40002', ruleset: 'main', uptime_7d: 60, is_new: true },
  { slug: 'c-quietccccccccccc', realm_id: 'quietccccccccccc', name: 'Quiet One', official: false, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 0, bots: 0, address: null, ruleset: 'brawl', uptime_7d: 100, is_new: true },
  { slug: 'c-gonedddddddddddd', realm_id: 'gonedddddddddddd', name: 'Gone Realm', official: false, state: 'offline', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: null, bots: 0, address: 'relay.openvanilla.app:40004', ruleset: 'custom', is_new: true },
];

const FAKE = ({ theme, realms, clients, seen, savedRealm, officialRealm }) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  if (theme === 'light') localStorage.setItem('leyline.theme', JSON.stringify({ id: 'light', kind: 'light' }));
  if (seen) localStorage.setItem('leyline.lastPlayers', JSON.stringify(seen));
  const st = { version: 'test', settings: { clients, selected_client: clients[0] ? clients[0].id : null, selected_realm: savedRealm || 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: officialRealm || 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  window.__st = st;
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'select': st.settings.selected_client = args.client; st.settings.selected_realm = args.realm; return null;
      case 'realms_list': return { realms: JSON.parse(JSON.stringify(realms)), network: { realms: realms.length, players: 29, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: theme || 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return false;
      case 'testing_get': return { client_torrents: false, share: false };
      case 'client_import_check': return { kind: 'zip', path: args.input, name: 'Wrath.zip', unpacks: true, default_folder: '/tmp' };
      case 'client_import_pick_dest': return null;   // the picker closed: nothing starts
      case 'play': return 'Started.';
      case 'play_realm': return 'Started.';
      default: throw 'unknown ' + cmd;
    }
  } } };
};

const CLIENTS = [
  { id: 'c335', name: 'Wrath', path: '/games/wrath', build: '3.3.5a', realmlist: '' },
  { id: 'c112', name: 'Classic', path: '/games/classic', build: '1.12.1', realmlist: '' },
];

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(opts = {}, scheme = 'dark') {
  const ctx = await browser.newContext({ colorScheme: scheme, viewport: { width: 1400, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, { theme: scheme, realms: REALMS, clients: CLIENTS, ...opts });
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineRealmLink && window.__calls.some(([c]) => c === 'realms_list'));
  await page.waitForFunction(() => window.LeylineApp.realms().length > 1);
  return { page, ctx, errors };
}
const shots = process.env.SHOTS;
async function shot(page, sel, name) {
  if (shots) await page.locator(sel).screenshot({ path: path.join(shots, name + '.png') });
}
const picked = (page) => page.evaluate(() => ({ realm: document.getElementById('play-realm').value, custom: document.getElementById('play-custom').value,
  client: document.getElementById('play-client').value }));
async function pastePage(page, text) {
  await page.evaluate(() => window.LeylineNav.show('realms'));
  await page.fill('#rl-paste-in', text);
  await page.click('#rl-paste button[type=submit]');
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c), cmd);

test('parse: each link kind, and junk', async () => {
  const { page, ctx, errors } = await open();
  const got = await page.evaluate(() => {
    const p = window.LeylineRealmLink.parse;
    return {
      short: p('https://play.openvanilla.app/wrath/r/frostpea'), root: p('play.openvanilla.app/r/frostpeakaaaaaaa'),
      app: p('leyline://realm/oldlandsbbbbbbbb'), official: p('https://play.openvanilla.app/wrath/r/official'), appOff: p('leyline://realm/official'),
      join: p('https://relay.openvanilla.app/join/quietccccccccccc#secret'), addr: p('Relay.Example.org:3724'), ip: p('10.0.0.5:3725'),
      other: p('https://evil.example/r/frostpea'), words: p('hello there'), word: p('hello'), port0: p('host.example:0'), zip: p('C:\\Games\\Wrath.zip'),
    };
  });
  assert.deepEqual(got.short, { kind: 'short', short: 'frostpea' });
  assert.deepEqual(got.root, { kind: 'id', id: 'frostpeakaaaaaaa', via: 'link' });
  assert.deepEqual(got.app, { kind: 'id', id: 'oldlandsbbbbbbbb', via: 'app' });
  assert.deepEqual(got.official, { kind: 'official' });
  assert.deepEqual(got.appOff, { kind: 'official' });
  assert.deepEqual(got.join, { kind: 'id', id: 'quietccccccccccc', via: 'join' });
  assert.deepEqual(got.addr, { kind: 'address', address: 'relay.example.org:3724' });
  assert.deepEqual(got.ip, { kind: 'address', address: '10.0.0.5:3725' });
  for (const k of ['other', 'words', 'word', 'port0', 'zip']) assert.equal(got[k], null, k);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the new names (docs/DOMAIN-MOVE.md): links on leylinelauncher.com, and the official realm saved under its old name', async () => {
  const { page, ctx, errors } = await open({ savedRealm: 'play.openvanilla.app', officialRealm: 'leylinelauncher.com' });
  const got = await page.evaluate(() => {
    const p = window.LeylineRealmLink.parse;
    return { short: p('https://leylinelauncher.com/wrath/r/frostpea'), official: p('leylinelauncher.com/wrath/r/official'),
      join: p('https://relay.leylinelauncher.com/join/quietccccccccccc#secret'), lookalike: p('https://leylinelauncher.com.evil.example/r/frostpea'),
      A: window.LeylineAddresses };
  });
  assert.deepEqual(got.short, { kind: 'short', short: 'frostpea' });
  assert.deepEqual(got.official, { kind: 'official' });
  assert.deepEqual(got.join, { kind: 'id', id: 'quietccccccccccc', via: 'join' });
  assert.equal(got.lookalike, null);
  assert.equal(got.A.SITE, 'https://leylinelauncher.com');
  // the play bar shows the old saved name as the official realm, not as "Your address"
  assert.equal((await picked(page)).realm, 'official');
  assert.equal(await page.locator('.pb-realm-opt', { hasText: 'play.openvanilla.app' }).count(), 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

for (const scheme of ['dark', 'light']) {
  test(`${scheme}: Realms page: a share link adds the realm, picks its client, Play now starts it`, async () => {
    const { page, ctx, errors } = await open({}, scheme);
    await pastePage(page, 'https://play.openvanilla.app/wrath/r/oldlands');
    await page.waitForFunction(() => !document.getElementById('rl-paste-play').hidden);
    assert.match(await page.textContent('#rl-paste-status'), /^Added Old Lands to Your realms\. Press Play now to start\.$/);
    assert.deepEqual(await picked(page), { realm: 'custom', custom: 'relay.openvanilla.app:40002', client: 'c112' }, 'the 1.12.1 client for a 1.12.1 realm');
    const sel = await calls(page, 'select');
    assert.equal(sel.at(-1)[1].realm, 'relay.openvanilla.app:40002', 'saved: Your realms');
    assert.equal((await calls(page, 'play')).length, 0, 'never starts by itself');
    await shot(page, '#rl-paste', `paste-${scheme}-added`);
    await page.click('#rl-paste-play');
    await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play'));
    const pl = (await calls(page, 'play'))[0][1];
    assert.deepEqual(pl, { client: 'c112', realm: 'relay.openvanilla.app:40002' });
    // the play bar's menu shows it under Your realms
    await page.evaluate(() => window.LeylineNav.show('play'));
    await page.click('#pb-realm-btn');
    const mine = await page.evaluate(() => [...document.querySelectorAll('#pb-rmenu li')].map((l) => l.textContent));
    assert.ok(mine.includes('Your realms') && mine.some((t) => t.includes('Old Lands')), mine.join(' | '));
    // unknown text: one plain line, no Play now
    await pastePage(page, 'hello');
    assert.match(await page.textContent('#rl-paste-status'), /^That isn't a realm link\. Paste a realm's share link, a leyline:\/\/ link, a join link or an address like realm\.example\.org:3724\.$/);
    assert.equal(await page.isVisible('#rl-paste-play'), false);
    await shot(page, '#rl-paste', `paste-${scheme}-unknown`);
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

test('Realms page: leyline://, join link, address, official, and a realm not on the list', async () => {
  const { page, ctx, errors } = await open();
  await pastePage(page, 'leyline://realm/frostpeakaaaaaaa');
  await page.waitForFunction(() => /Added Frost Peak/.test(document.getElementById('rl-paste-status').textContent));
  assert.deepEqual(await picked(page), { realm: 'custom', custom: 'relay.openvanilla.app:40001', client: 'c335' });
  // a join link to a listed realm without an address yet: Play now goes through the Realms page's PLAY (play_realm)
  await pastePage(page, 'https://relay.openvanilla.app/join/quietccccccccccc#abc');
  await page.waitForFunction(() => /Added Quiet One/.test(document.getElementById('rl-paste-status').textContent));
  await page.click('#rl-paste-play');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play_realm'));
  assert.equal((await calls(page, 'play_realm'))[0][1].slug, 'c-quietccccccccccc');
  // an address nobody lists: added as typed
  await pastePage(page, 'my.realm.example:3724');
  await page.waitForFunction(() => /Added my\.realm\.example:3724/.test(document.getElementById('rl-paste-status').textContent));
  assert.deepEqual(await picked(page), { realm: 'custom', custom: 'my.realm.example:3724', client: 'c335' });
  // the official realm's link
  await pastePage(page, 'https://play.openvanilla.app/wrath/r/official');
  await page.waitForFunction(() => /Added Wrath Unleashed/.test(document.getElementById('rl-paste-status').textContent));
  assert.equal((await picked(page)).realm, 'official');
  // a well-formed link to a realm that isn't listed
  await pastePage(page, 'leyline://realm/zzzzzzzzzzzzzzzz');
  await page.waitForFunction(() => /isn't on the realm list right now/.test(document.getElementById('rl-paste-status').textContent));
  assert.equal(await page.isVisible('#rl-paste-play'), false);
  // pasting into the box adds at once
  await page.evaluate(() => {
    const i = document.getElementById('rl-paste-in');
    i.value = '';
    const dt = new DataTransfer(); dt.setData('text/plain', 'https://play.openvanilla.app/r/frostpea');
    i.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
  });
  await page.waitForFunction(() => /Added Frost Peak/.test(document.getElementById('rl-paste-status').textContent));
  assert.deepEqual(errors, []);
  await ctx.close();
});

for (const scheme of ['dark', 'light']) {
  test(`${scheme}: play bar realm box: a dropped realm link adds it with Play now; junk gets the plain line; a zip still imports`, async () => {
    const { page, ctx, errors } = await open({}, scheme);
    await page.evaluate(() => window.LeylineBarImport.drop('https://play.openvanilla.app/wrath/r/frostpea'));
    await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'done' && !document.getElementById('pbd-play').hidden);
    assert.equal(await page.textContent('#pbd-head'), 'Added Frost Peak to Your realms.');
    assert.deepEqual(await picked(page), { realm: 'custom', custom: 'relay.openvanilla.app:40001', client: 'c335' });
    assert.equal((await calls(page, 'client_import_check')).length, 0, 'not a game import');
    await shot(page, '#play-form', `bar-${scheme}-realm`);
    await page.click('#pbd-play');
    await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play'));
    assert.deepEqual((await calls(page, 'play'))[0][1], { client: 'c335', realm: 'relay.openvanilla.app:40001' });
    assert.equal(await page.isVisible('#pbd'), false, 'the box is back to the realm');
    // an address pasted with the box focused
    await page.focus('#pb-realm-btn');
    await page.evaluate(() => {
      const dt = new DataTransfer(); dt.setData('text/plain', 'relay.openvanilla.app:40002');
      document.getElementById('pb-realm-btn').dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
    });
    await page.waitForFunction(() => document.getElementById('pbd-head').textContent === 'Added Old Lands to Your realms.');
    assert.equal((await picked(page)).client, 'c112');
    // junk
    await page.evaluate(() => window.LeylineBarImport.drop('hello'));
    await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'error');
    assert.match(await page.textContent('#pbd-head'), /^That isn't a realm link\./);
    assert.equal(await page.isVisible('#pbd-retry'), false);
    assert.equal(await page.isVisible('#pbd-play'), false);
    await shot(page, '#play-form', `bar-${scheme}-unknown`);
    // a zip goes on to the game-folder import as before
    await page.evaluate(() => window.LeylineBarImport.drop('C:\\Games\\Wrath.zip'));
    await page.waitForFunction(() => window.__calls.some(([c]) => c === 'client_import_check'));
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

for (const scheme of ['dark', 'light']) {
  test(`${scheme}: Home: Live now by players then uptime with mode badges and Play; New this week`, async () => {
    const now = Date.now();
    const { page, ctx, errors } = await open({ seen: { quietccccccccccc: now - 3600e3, gonedddddddddddd: now - 3 * 86400e3 } }, scheme);
    await page.evaluate(() => window.LeylineNav.show('play'));
    await page.waitForFunction(() => !document.getElementById('hm-realms').hidden && document.querySelectorAll('#hm-realms-row .hm-lv').length);
    const live = await page.evaluate(() => [...document.querySelectorAll('#hm-realms-row .hm-lv')].map((c) => [c.querySelector('.hm-name').textContent,
      c.querySelector('.hm-mode-bdg').textContent, c.querySelector('.hm-line').textContent]));
    assert.deepEqual(live, [['Frost Peak', 'Vanilla+', '12 playing'], ['Old Lands', 'Level 1 or boost to 80', '12 playing'], ['Quiet One', 'Brawl: everyone 80', 'Online']]);
    const fresh = await page.evaluate(() => [...document.querySelectorAll('#hm-new-row .hm-name')].map((n) => n.textContent));
    assert.deepEqual(fresh, ['Old Lands', 'Quiet One'], 'players now, or seen with players an hour ago; not 3 days ago');
    await page.locator('#hm-realms').scrollIntoViewIfNeeded();
    await shot(page, '#home-more', `home-${scheme}-live`);
    await page.click('#hm-realms-row .hm-lv:first-child .hm-play');
    await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play'));
    assert.deepEqual((await calls(page, 'play'))[0][1], { client: 'c335', realm: 'relay.openvanilla.app:40001' });
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

test('Home: empty states in plain words', async () => {
  const { page, ctx, errors } = await open({ realms: [REALMS[0], REALMS[4]] });
  await page.evaluate(() => window.LeylineNav.show('play'));
  await page.waitForFunction(() => !document.getElementById('hm-realms').hidden);
  assert.equal(await page.textContent('#hm-realms-row'), 'No player realms are up right now. Check back soon, or host your own.');
  assert.equal(await page.textContent('#hm-new-row'), 'No new realms with players this week yet.');
  await page.locator('#hm-realms').scrollIntoViewIfNeeded();
  await shot(page, '#home-more', 'home-dark-empty');
  assert.deepEqual(errors, []);
  await ctx.close();
});
