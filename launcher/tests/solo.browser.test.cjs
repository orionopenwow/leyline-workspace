// The play bar's realm drop-up (ui/app.js "the play bar's realm picker") for Solo leveling, the game client part and
// the first-run steps (owner 31 Oct: "add solo leveling and choose client here and make setup flawless and easy";
// "hopping between client versions should be easy too"), in Chromium with a fake Tauri core:
//   * Solo leveling beside My Den: Set it up once (the Host page with it picked), Needs your game folder (the add
//     flow, then setup goes on by itself), Switch from My Den (one realm per PC: asked, then run on the same realm),
//     On this PC (the invite knock, then PLAY; PLAY again knocks again); not answering: the Host page says Start;
//   * the Game client part: the clients of the version the realm needs, the picked one marked, Add a game folder…;
//     none of that version: one plain line; picking a realm of another version takes a client of that version by
//     itself, per realm and per version remembered; a leyline://realm/<id> link picks it with the right client;
//   * the bar's status line says the same next step; no brand names; screenshots dark and light (SOLO_SHOTS).
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/solo.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.SOLO_SHOTS || '';

const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const WRATH2 = { id: 'c335a-2', name: 'Wrath copy', path: 'D:\\Wrath2', build: '3.3.5a', realmlist: '' };
const VANILLA = { id: 'c1121-1', name: 'My Vanilla', path: '/home/me/Games/vanilla', build: '1.12.1', realmlist: '' };
const NET = [
  { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 5, bots: 0, address: 'play.openvanilla.app' },
  { slug: 'c-b', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Busy Den', state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 30, bots: 0, address: 'relay.openvanilla.app:40002' },
  { slug: 'c-v', realm_id: 'vvvvvvvvvvvvvvvv', name: 'Vanilla Remastered', state: 'live', build: '1.12.1', version: '1.12.1', core: 'vmangos', tags: [], players: 2, bots: 0, address: 'relay.openvanilla.app:40050' },
  { slug: 'c-t', realm_id: 'tttttttttttttttt', name: 'Outland Rest', state: 'live', build: '2.4.3', version: '2.4.3', core: 'cmangos', tags: [], players: 4, bots: 0, address: 'relay.openvanilla.app:40060' },
];
const KIT = (profile, name) => ({ dir: '/home/me/kit', is_kit: true, has_windows_script: true, has_unix_script: true, problems: [], seeds: [],
  realm: profile ? { realm_id: 'abcdefghijk2345a', name, visibility: 'private', relay: 'relay.openvanilla.app', profile } : null });
const NO_KIT = { dir: null, is_kit: false, problems: [], seeds: [], realm: null };

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  window.__kit = o.kit;
  window.__joinFails = !!o.joinFails;
  window.__deep = o.deep || null;
  const st = (window.__st = { version: 'test', settings: { clients: o.clients, selected_client: o.clients[0] ? o.clients[0].id : null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' });
  const offer = (id) => ({ id, label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true,
    clients: st.settings.clients.filter((c) => c.build === '3.3.5a').map((c) => ({ id: c.id, name: c.name, path: c.path })),
    note: st.settings.clients.some((c) => c.build === '3.3.5a') ? null : 'Add your own 3.3.5a client on the Clients page first: the server data is made from it.' });
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'select': st.settings.selected_client = args.client; st.settings.selected_realm = args.realm; return null;
      case 'realms_list': return { realms: o.net, network: { realms: 3, players: 5, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': { const d = window.__deep; window.__deep = null; return d; }
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return JSON.parse(JSON.stringify(window.__kit));
      case 'host_invite': return window.__kit.realm ? 'https://relay.openvanilla.app/join/abcdefghijk2345a#k3y' : null;
      case 'join_invite': if (window.__joinFails) throw 'The realm did not answer.'; return { address: 'relay.openvanilla.app:40007', name: window.__kit.realm.name };
      case 'play': return 'Started.';
      case 'host_profiles': return [['vanillaplus', 'den', 'solo', 'dalaran'].map(offer), null];
      case 'host_choose': return { profile: args.profile, client_dir: '' };
      case 'host_start_quiet': return 'Setting up your realm.';
      case 'host_progress': return { idle: false, kind: 'host', running: true, code: null, step: 'Getting ready', n: 1, of: 8, secs: 3, problem: null };
      case 'hostpack_info': return o.kit.is_kit ? { installed: { version: '7' }, available: { version: '7' }, update: false, kit_dir: '/home/me/kit' }
        : { installed: null, available: { version: '7', size: 400000 }, update: false, kit_dir: '/home/me/kit' };
      case 'docker_info': return o.docker || { state: 'ready', version: '27', help: '', get_url: '' };
      case 'host_scan': return null;
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      case 'host_console': return { set_up: true, linked: false, name: window.__kit.realm && window.__kit.realm.name, realm_id: 'abcdefghijk2345a',
        profile: window.__kit.realm && window.__kit.realm.profile, core: 'azerothcore', visibility: 'private', signup: 'invite', browser: false, max_players: 5,
        docker: true, services: [{ role: 'world', state: o.stopped ? 'off' : 'ok', text: o.stopped ? 'stopped' : 'up' }], gm: false, gm_note: null };
      case 'host_status': return { realm_id: args.realmId, online: !o.stopped, name: 'x', visibility: 'private', players: 1, bots: 0, listed: false };
      case 'host_dens': return [];
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return false;
      case 'pick_folder': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });
async function open(o, scheme = 'dark') {
  const opts = Object.assign({ clients: [WRATH, VANILLA], kit: NO_KIT, net: NET }, o);
  const ctx = await browser.newContext({ colorScheme: scheme, viewport: { width: 1400, height: 860 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.__calls.some(([c]) => c === 'realms_list') && window.__calls.some(([c]) => c === 'host_info'));
  await page.waitForFunction(() => document.getElementById('pb-realm').textContent.length > 0);
  if (scheme === 'light') await page.evaluate(() => document.documentElement.setAttribute('data-base', 'light'));
  return { page, ctx, errors };
}
const rrows = (page) => page.$$eval('#pb-rmenu > li', (l) => l.map((x) => (x.classList.contains('pb-sep') ? '---' : x.classList.contains('pb-head') ? '# ' + x.textContent
  : x.classList.contains('pb-note') ? '! ' + x.textContent
  : [x.querySelector('.pb-opt-name').textContent, (x.querySelector('.rl-ver') || {}).textContent || '', (x.querySelector('.pb-opt-sub') || {}).textContent || '',
    x.getAttribute('aria-selected')].join('|'))));
const row = async (page, name) => (await rrows(page)).find((x) => x.startsWith(name + '|'));
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const pageOn = (page) => page.evaluate(() => document.querySelector('.page.on').id);
const bar = async (page) => [await page.textContent('#play-headline'), await page.textContent('#pb-detail')];
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;
async function shot(page, name) {
  if (!SHOTS) return;
  await page.screenshot({ path: path.join(SHOTS, name + '.png') });
}

test('a new player (no game folder, no Host pack, no Docker): every row says its next step, none is a dead end', async () => {
  const { page, ctx, errors } = await open({ clients: [], docker: { state: 'missing', help: 'Get Docker Desktop: free.', get_url: 'https://docker.com', can_start: false } });
  assert.deepEqual(await bar(page), ['ADD YOUR GAME FOLDER', 'PLAY shows where to find it, or play in your browser']);
  await page.click('#pb-realm-btn');
  const rows = await rrows(page);
  assert.equal(await row(page, 'My Den'), 'My Den||Needs your game folder|false');
  assert.equal(await row(page, 'Solo leveling'), 'Solo leveling||Needs your game folder|false');
  assert.deepEqual(rows.slice(rows.indexOf('# Game client')), ['# Game client', '! This realm plays with 3.3.5a. No 3.3.5a game folder yet.', 'Add a game folder…|||false']);
  await shot(page, 'first-run-dropup-dark');
  // Solo leveling with no folder: the add flow, and it says what for
  await page.click('#pb-rmenu .pb-solo');
  assert.equal(await pageOn(page), 'page-clients');
  assert.match(await page.textContent('#cl-status'), /Solo leveling plays with your own 3\.3\.5a game folder/);
  // the folder is added: setup goes on by itself, to the Host page with Solo leveling picked (Host pack, Docker steps first)
  await page.evaluate((w) => { window.__st.settings.clients.push(w); window.__st.settings.selected_client = w.id; return refresh(); }, WRATH);
  await page.waitForFunction(() => document.querySelector('#page-host.on') && document.querySelector('.hc-rs[data-profile="solo"].on'));
  assert.deepEqual((await callsOf(page, 'host_choose'))[0], { profile: 'solo', clientId: 'c335a-1' });
  assert.equal(await page.textContent('#hc-start'), 'Start Solo leveling');
  assert.ok(await page.isVisible('#hc-get-pack'), 'step 1: the Host pack');
  assert.match(await page.textContent('.hc-ready'), /Get Docker/, 'step 2: Docker, in plain words');
  assert.match(await page.textContent('.hc-den'), /Helper bots/);
  assert.equal(await page.isVisible('#hc-vis'), false, 'always private: no public choice');
  await shot(page, 'first-run-host-solo-dark');
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), BRANDS);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Solo leveling not set up: Set it up once opens the Host page with it picked; nothing starts', async () => {
  for (const scheme of ['dark', 'light']) {
    const { page, ctx, errors } = await open({ kit: KIT(null) }, scheme);
    await page.click('#pb-realm-btn');
    assert.equal(await row(page, 'Solo leveling'), 'Solo leveling||Set it up once|false');
    assert.equal(await page.locator('#pb-rmenu .pb-solo .pb-solo-ic svg').count(), 1, 'its sword mark');
    await shot(page, 'dropup-' + scheme);
    await page.click('#pb-rmenu .pb-solo');
    assert.equal(await pageOn(page), 'page-host');
    await page.waitForFunction(() => document.querySelector('.hc-rs[data-profile="solo"].on'));
    assert.equal(await page.inputValue('#hc-name'), 'Solo leveling');
    assert.match(await page.textContent('.hc-rs[data-profile="solo"] .hc-rs-l'), /Helper bots fill your group/);
    if (await page.isVisible('#hc-gm')) await page.fill('#hc-gm', 'soloone');   // a Mac or Linux kit asks for your account name
  assert.match(await page.textContent('#page-host'), /Then pick Solo leveling in the play bar/);
    assert.equal((await callsOf(page, 'host_start_quiet')).length, 0);
    await shot(page, 'host-solo-' + scheme);
    assert.deepEqual(errors, []);
    await ctx.close();
  }
});

test('Solo leveling on this PC: the row plays it (knock, then PLAY); PLAY again knocks again; the bar names it', async () => {
  const { page, ctx, errors } = await open({ kit: KIT('solo', 'Solo leveling'), clients: [VANILLA, WRATH] });
  await page.click('#pb-realm-btn');
  assert.equal(await row(page, 'Solo leveling'), 'Solo leveling|3.3.5a|On this PC|false');
  assert.equal(await row(page, 'My Den'), 'My Den||Switch from Solo leveling|false', 'one realm per PC: the other offers to switch');
  await page.click('#pb-rmenu .pb-solo');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play'));
  assert.deepEqual(await callsOf(page, 'join_invite'), [{ client: 'c335a-1', link: 'https://relay.openvanilla.app/join/abcdefghijk2345a#k3y' }], 'the 3.3.5a folder, not the picked 1.12.1 one');
  assert.deepEqual(await callsOf(page, 'play'), [{ client: 'c335a-1', realm: 'relay.openvanilla.app:40007' }]);
  assert.equal(await page.textContent('#pb-realm'), 'Solo leveling');
  assert.equal(await page.textContent('#pb-realm-ver'), '3.3.5a');
  assert.ok(await page.evaluate(() => document.querySelector('#pb-realm-ic.pb-mark-ic svg')));
  assert.deepEqual(await bar(page), ['READY TO PLAY', 'On this PC']);
  await page.click('#play-go');
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'play').length === 2);
  assert.equal((await callsOf(page, 'join_invite')).length, 2, 'PLAY knocks again first');
  await page.click('#pb-realm-btn');
  assert.equal(await row(page, 'Solo leveling'), 'Solo leveling|3.3.5a|On this PC|true');
  assert.ok(!(await rrows(page)).some((x) => x.startsWith('relay.openvanilla.app:40007')), 'not again as an address');
  await shot(page, 'solo-playing-dark');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Solo leveling on this PC but stopped: PLAY never fails silently: the Host page says Start', async () => {
  const { page, ctx, errors } = await open({ kit: KIT('solo', 'Solo leveling'), joinFails: true, stopped: true });
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu .pb-solo');
  await page.waitForFunction(() => document.querySelector('#page-host.on'));
  await page.waitForFunction(() => /Press Start to start Solo leveling/.test(document.querySelector('#page-host').textContent));
  assert.match(await page.textContent('#page-host'), /didn't answer|did not answer/);
  assert.equal((await callsOf(page, 'play')).length, 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('My Den on this PC, Solo leveling picked: one realm per PC, so it asks, then switches the same realm', async () => {
  const { page, ctx, errors } = await open({ kit: KIT('den', 'My Den') });
  await page.click('#pb-realm-btn');
  assert.equal(await row(page, 'Solo leveling'), 'Solo leveling||Switch from My Den|false');
  await page.click('#pb-rmenu .pb-solo');
  await page.waitForSelector('#hc-confirm[open]');
  assert.equal(await page.textContent('#hc-confirm-title'), 'Switch to Solo leveling?');
  assert.match(await page.textContent('#hc-confirm-body'), /one realm at a time, and it runs My Den now/);
  await shot(page, 'switch-ask-dark');
  await page.click('#hc-confirm-no');
  await page.waitForFunction(() => /Kept My Den/.test(document.querySelector('#page-host').textContent));
  assert.equal((await callsOf(page, 'host_start_quiet')).length, 0, 'no: nothing changes');
  await page.click('.nav[data-page="play"]');
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu .pb-solo');
  await page.waitForSelector('#hc-confirm[open]');
  await page.click('#hc-confirm-yes');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_start_quiet'));
  assert.deepEqual((await callsOf(page, 'host_choose')).pop(), { profile: 'solo', clientId: 'c335a-1' });
  assert.deepEqual(await callsOf(page, 'host_start_quiet'), [{ answers: { name: 'Solo leveling', visibility: 'private', gm: null } }]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the game client part: the version the realm needs, picked by itself when you hop; per realm and per version remembered', async () => {
  const { page, ctx, errors } = await open({ clients: [WRATH, VANILLA, WRATH2] });
  await page.click('#pb-realm-btn');
  let rows = await rrows(page);
  assert.deepEqual(rows.slice(rows.indexOf('# Game client')), ['# Game client', 'My Wrath|3.3.5a|C:\\Games\\Wrath|true', 'Wrath copy|3.3.5a|D:\\Wrath2|false', 'Add a game folder…|||false'],
    'only the 3.3.5a folders for a 3.3.5a realm');
  // a 1.12.1 realm: My Vanilla by itself, its chip in the picker
  await page.click('#pb-rmenu >> text=Vanilla Remastered');
  assert.equal(await page.inputValue('#play-client'), 'c1121-1');
  assert.equal(await page.textContent('#pb-client-ver'), '1.12.1');
  assert.equal(await page.textContent('#pb-realm-ver'), '1.12.1');
  assert.equal((await bar(page))[0], 'READY TO PLAY');
  // back to the official realm: the 3.3.5a one used there
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu >> text=Wrath Unleashed');
  assert.equal(await page.inputValue('#play-client'), 'c335a-1');
  // Busy Den with the second 3.3.5a folder, picked here; remembered for Busy Den only
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu >> text=Busy Den');
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu .pb-client-opt >> text=Wrath copy');
  assert.equal(await page.inputValue('#play-client'), 'c335a-2');
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu >> text=Wrath Unleashed');
  assert.equal(await page.inputValue('#play-client'), 'c335a-1', 'the official realm keeps its own');
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu >> text=Busy Den');
  assert.equal(await page.inputValue('#play-client'), 'c335a-2', 'Busy Den keeps its own');
  // a 2.4.3 realm with no 2.4.3 folder: one plain line and Add a game folder…; the bar says the same
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu >> text=Outland Rest');
  assert.deepEqual(await bar(page), ['THIS SERVER NEEDS 2.4.3', 'Add a 2.4.3 folder']);
  await page.click('#pb-realm-btn');
  rows = await rrows(page);
  assert.deepEqual(rows.slice(rows.indexOf('# Game client')), ['# Game client', '! This realm plays with 2.4.3. No 2.4.3 game folder yet.', 'Add a game folder…|||false']);
  await shot(page, 'client-none-dark');
  await page.click('#pb-rmenu .pb-add-client');
  assert.ok(await page.isVisible('#pb-menu') || (await pageOn(page)) === 'page-clients', 'the existing add flow');
  assert.match(await page.textContent('#play-status'), /Add your 2\.4\.3 game folder/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a leyline://realm/<id> link (the in-game realm browser) picks that realm with a client of its version', async () => {
  const { page, ctx, errors } = await open({ clients: [WRATH, VANILLA], deep: { action: 'realm', id: 'vvvvvvvvvvvvvvvv' } });
  await page.waitForFunction(() => document.getElementById('pb-realm').textContent === 'Vanilla Remastered');
  assert.equal(await page.inputValue('#play-client'), 'c1121-1');
  assert.match(await page.textContent('#rl-status'), /Vanilla Remastered\. Press PLAY/);
  await ctx.close();
  const b = await open({ clients: [WRATH], deep: { action: 'realm', id: 'tttttttttttttttt' } });
  await b.page.waitForFunction(() => document.getElementById('pb-realm').textContent === 'Outland Rest');
  assert.match(await b.page.textContent('#rl-status'), /Outland Rest plays with 2\.4\.3\. Add your 2\.4\.3 game folder first/);
  assert.deepEqual([...errors, ...b.errors], []);
  await b.ctx.close();
});
