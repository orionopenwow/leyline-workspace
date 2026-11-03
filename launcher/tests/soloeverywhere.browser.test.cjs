// Single player everywhere, Invite a friend, the friend's Join and the GM command center (ui/solo.js, ui/gmcenter.js;
// owner 3 Nov: "single player for all modes apparent and available... 1 click invite friend to single player world and
// GM command center available for full command sandbox control"), in Chromium with a fake Tauri core:
//   * every Modes card, every realm card and every Host mode card: one "Play solo" (or a quiet "Online only" with why)
//     right after its online button, and one line under it; the label the same everywhere; Docker missing said before
//     anything is pressed; Play solo on a mode opens that mode's form private and for you; on the official realm it is
//     Solo leveling; nothing starts by itself;
//   * Home: the other ways to play solo and Invite a friend: Den Lite's link (den_invite) or the Host pack realm's
//     (host_invite), copied;
//   * leyline://join?link=…: one Join card; nothing joins before Join; Join knocks with a 3.3.5a folder; none: says so;
//   * the GM command center: your realm's groups, each control as one checked host_gm call (sandbox verbs, the name
//     to act on), search then Spawn, kick / ban / restart / clear / kill asked first (No sends nothing), controls off
//     while the realm is stopped, Your Den's in-game note, the official realm only as Leyline HQ; no brand names.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/soloeverywhere.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const CATALOG = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'stream', 'playmodes.json'), 'utf8'));
const PACKS = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'stream', 'modepacks.json'), 'utf8')).modes.map((m) => ({
  id: m.id, name: m.name, line: m.line, version: m.version, author: m.author, based_on: m.based_on, status: m.status, profile: m.profile,
  scripts: m.scripts, env: m.env, mods: m.mods, den: m.den, tags: m.tags, jam: null, credit: m.credit || null, settings: m.settings,
  picture: null, own: false }));
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;
const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const VANILLA = { id: 'c1121-1', name: 'My Vanilla', path: 'D:\\Vanilla', build: '1.12.1', realmlist: '' };
const NET = [
  { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 5, bots: 0, address: 'play.openvanilla.app' },
  { slug: 'c-b', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Busy Den', state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 30, bots: 0, address: 'relay.openvanilla.app:40002' },
  { slug: 'c-v', realm_id: 'vvvvvvvvvvvvvvvv', name: 'Old Days', state: 'live', build: '1.12.1', version: '1.12.1', core: 'vmangos', tags: [], players: 2, bots: 0, address: 'relay.openvanilla.app:40050' },
];
const LINK = 'https://relay.leylinelauncher.com/join/abcdefghijk2345a#ABCDEFGHJKLMNPQR~GUEST7~pw9';
const KIT = { dir: '/home/me/kit', is_kit: true, has_windows_script: true, has_unix_script: true, problems: [], seeds: [],
  realm: { realm_id: 'abcdefghijk2345a', name: 'My Sandbox', visibility: 'private', relay: 'relay.openvanilla.app', profile: 'main' } };
const NO_KIT = { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
const READY = { state: 'ready', version: '27', help: 'Docker is ready.', get_url: '', can_start: false };
const MISSING = { state: 'missing', version: null, help: 'Get Docker Desktop: free for personal use.', get_url: 'https://docs.docker.com/desktop/', can_start: false };

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  window.__clip = null;
  try { Object.defineProperty(navigator, 'clipboard', { value: { writeText: async (t) => { window.__clip = t; } }, configurable: true }); } catch (_) { /* fine */ }
  window.__deep = o.deep || null;
  window.__answer = o.answer || {};
  const st = { version: 'test', settings: { clients: o.clients, selected_client: o.clients[0] ? o.clients[0].id : null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'select': return null;
      case 'realms_list': return { realms: o.net, network: { realms: 3, players: 5, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': { const d = window.__deep; window.__deep = null; return d; }
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return JSON.parse(JSON.stringify(o.kit));
      case 'hostpack_info': return { installed: o.kit.is_kit ? { version: '7' } : null, available: { version: '7' }, update: false, kit_dir: '/home/me/kit' };
      case 'docker_info': return o.docker;
      case 'host_profiles': return [[], null];
      case 'host_scan': return null;
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      case 'host_modes': return JSON.parse(JSON.stringify(o.packs));
      case 'host_console': return o.kit.realm ? { set_up: true, linked: false, name: o.kit.realm.name, realm_id: o.kit.realm.realm_id, profile: o.kit.realm.profile,
        core: o.core || 'azerothcore', visibility: 'private', signup: 'invite', browser: false, max_players: 5, docker: true,
        services: [{ role: 'world', state: o.stopped ? 'off' : 'ok', text: o.stopped ? 'stopped' : 'up' }], gm: !o.stopped,
        gm_note: o.stopped ? 'Start your realm first.' : null } : { set_up: false, linked: false, services: [], gm: false };
      case 'host_status': return { realm_id: args.realmId, online: !o.stopped, name: 'x', visibility: 'private', players: 1, bots: 0, listed: false };
      case 'host_dens': return [];
      case 'host_invite': return o.kit.realm ? 'https://relay.leylinelauncher.com/join/abcdefghijk2345a#HOSTLINKHOSTLINK' : null;
      case 'host_gm': {
        const k = args.action + (args.data ? ':' + args.data.split('\t')[0] : '');
        if (window.__answer[k] !== undefined) return window.__answer[k];
        return 'Done: ' + args.action;
      }
      case 'den_state': return { lite: { available: !!o.lite, can_get: !!o.lite, running: o.lite === 'running', address: '127.0.0.1', account: '', password: '' },
        kit: !!o.kit.is_kit, docker_den: false, other_realm: null, account: '', saves: 'lite', dens: [], packs: [] };
      case 'den_invite': return { link: 'https://relay.leylinelauncher.com/join/dddddddddddddddd#DENSECRETDENSECR~G1~p', name: 'Guest one', expires: 1, tunnel: { running: true } };
      case 'den_net_state': return { running: false };
      case 'forge_state': return { running: false };
      case 'join_invite': return { address: 'relay.openvanilla.app:40007', name: 'Hosty world', guest: args.link.includes('~') ? { auto: true, account: 'G1', password: '', why: '', message: '' } : null };
      case 'mode_prepare_desktop': return 'Minigolf starts after you log in.';
      case 'mode_open_browser': return 'Opened it in your browser.';
      case 'open_link': return null;
      case 'modes_list': return { modes: o.modes, from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return true;
      case 'host_progress': return { idle: true };
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(o = {}) {
  const opts = Object.assign({ clients: [WRATH, VANILLA], kit: NO_KIT, net: NET, docker: READY, modes: CATALOG.modes, packs: PACKS, lite: 'stopped' }, o);
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://**', (r) => r.abort());
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineSolo && window.LeylineGM && window.__calls.some(([c]) => c === 'host_info'));
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const pageOn = (page) => page.evaluate(() => document.querySelector('.page.on').id);
// each card: [its first button, what follows it, the solo line]
const soloRows = (page, sel) => page.$$eval(sel, (cards) => cards.map((c) => {
  const s = c.querySelector('.solo-go, .solo-none');
  return { id: c.dataset.mode || (c.querySelector('.name') || {}).textContent, label: s ? s.textContent : null, kind: s ? s.className : null,
    after: s && s.previousElementSibling ? s.previousElementSibling.textContent : null, note: (c.querySelector('.solo-note') || {}).textContent || null,
    disabled: !!(s && s.disabled) };
}));

test('Modes: every card has Play solo (or Online only, with why) right after its online button, one line under it', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('modes'));
  await page.waitForFunction(() => document.querySelectorAll('#md-list .md-card').length > 0);
  await page.waitForFunction(() => /set up once/.test(document.querySelector('#md-list .md-card[data-mode="minigolf"] .solo-note').textContent));
  const rows = await soloRows(page, '#md-list .md-card');
  assert.equal(rows.length, CATALOG.modes.length);
  for (const r of rows) {
    assert.ok(r.label === 'Play solo' || r.label === 'Online only', `${r.id}: ${r.label}`);
    assert.ok(r.note && r.note.length > 10, `${r.id} says where it runs`);
  }
  const by = Object.fromEntries(rows.map((r) => [r.id, r]));
  for (const id of ['minigolf', 'bumper-balls', 'coin-dash', 'community', 'brawl']) {
    assert.equal(by[id].label, 'Play solo', id);
    assert.equal(by[id].after, id === 'brawl' ? 'Host it' : 'Play', `${id}: right after the online button`);
    assert.match(by[id].note, /^On this PC, with your own game files/);
  }
  assert.equal(by['arena-1v1'].label, 'Online only');
  assert.match(by['arena-1v1'].note, /^Online only: Two players by design/);
  assert.equal(by['azeroth-survivors'].label, 'Play solo');
  assert.ok(by['azeroth-survivors'].disabled, 'not out yet: no button that fails');
  // Play solo on Minigolf: its mode's form, private and for you (no realm on this PC yet)
  await page.click('#md-list .md-card[data-mode="minigolf"] .solo-go');
  await page.waitForFunction(() => document.querySelector('#page-host.on') && document.querySelector('#hx .hx-solo'));
  assert.match(await page.textContent('#hx .hx-solo'), /Play solo: just you, on this PC/);
  assert.equal(await page.inputValue('#hx-name'), 'Solo Minigolf night');
  assert.equal(await page.locator('#hx-vis').count(), 0, 'no Everyone: solo is private');
  const started = await page.evaluate(() => window.__calls.map(([c]) => c).filter((c) => /^(play|host_start|host_choose|join_invite)/.test(c)));
  assert.deepEqual(started, [], 'nothing starts before you press it');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Docker missing: the line says so before anything is pressed; Play solo opens the setup with the way to get it', async () => {
  const { page, ctx, errors } = await open({ docker: MISSING });
  await page.evaluate(() => window.LeylineNav.show('modes'));
  await page.waitForFunction(() => /Needs Docker/.test((document.querySelector('#md-list .md-card[data-mode="brawl"] .solo-note') || {}).textContent));
  assert.equal(await page.textContent('#md-list .md-card[data-mode="brawl"] .solo-note'),
    'On this PC, with your own game files. Needs Docker (free): Get Docker Desktop: free for personal use.');
  assert.match(await page.getAttribute('#md-list .md-card[data-mode="brawl"] .solo-go', 'title'), /Needs Docker first/);
  await page.click('#md-list .md-card[data-mode="brawl"] .solo-go');
  await page.waitForFunction(() => /runs in Docker on this PC/.test((document.querySelector('#hx .hx-status') || {}).textContent || ''));
  assert.equal(await pageOn(page), 'page-host');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Realms: Play solo after Play on each realm card; the official realm plays Solo leveling on this PC', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('realms'));
  await page.waitForFunction(() => document.querySelectorAll('#rl-list .realm-card .solo-go').length > 0);
  const rows = await soloRows(page, '#rl-list .realm-card');
  assert.equal(rows[0].label, 'Play solo');
  assert.equal(rows[0].after, 'Play', 'right after Play');
  assert.match(rows[0].note, /Solo leveling/);
  await page.click('#rl-tab-community');
  await page.waitForFunction(() => document.querySelectorAll('#rl-list .realm-card').length === 2);
  const com = await soloRows(page, '#rl-list .realm-card');
  assert.deepEqual(com.map((r) => r.label), ['Play solo', 'Play solo']);
  assert.match(com[1].note, /your own realm of this version/, 'another version: its own realm on this PC');
  await page.click('#rl-tab-official');
  await page.waitForFunction(() => document.querySelector('#rl-list .realm-card .solo-go'));
  await page.click('#rl-list .realm-card .solo-go');
  await page.waitForFunction(() => document.querySelector('#page-host.on'));
  const started = await page.evaluate(() => window.__calls.map(([c]) => c).filter((c) => /^(play|host_start|join_invite)/.test(c)));
  assert.deepEqual(started, [], 'the Host page says the next step; nothing starts by itself');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Host: every mode card has Play solo after Host; the Den has its own page', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('host'));
  await page.evaluate(() => window.LeylineHostModes.grid());   // Host's More ways to host > Game modes (hostflow.js, 3 Nov)
  await page.waitForFunction(() => document.querySelectorAll('#hx .hx-card').length > 0);
  const rows = await soloRows(page, '#hx .hx-card');
  for (const r of rows.filter((x) => x.id !== 'survivors')) {
    assert.equal(r.label, 'Play solo', r.id);
    assert.equal(r.after, 'Host', `${r.id}: right after Host`);
  }
  assert.equal(rows.find((r) => r.id === 'survivors').label, null, 'not a realm: no solo row on the Host page');
  await page.click('#hx .hx-card[data-mode="duel-ladder"] .solo-go');
  await page.waitForFunction(() => document.querySelector('#hx .hx-solo'));
  assert.equal(await page.inputValue('#hx-name'), 'Solo Duel ladder');
  // Host keeps its own form (who can join)
  await page.click('#hx .hx-back');
  await page.click('#hx .hx-card[data-mode="duel-ladder"] .hx-host');
  await page.waitForFunction(() => document.querySelector('#hx-vis'));
  assert.equal(await page.locator('#hx .hx-solo').count(), 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Home: the ways to play solo, and Invite a friend copies Your Den\'s link (Den Lite)', async () => {
  const { page, ctx, errors } = await open({ lite: 'running' });
  await page.waitForSelector('#solo-home');
  assert.match(await page.textContent('#solo-home .solo-ways'), /Solo leveling.*Modes.*Your own realm.*GM command center/);
  await page.click('#solo-home .solo-invite');
  await page.waitForFunction(() => /Invite link copied/.test(document.getElementById('solo-status').textContent));
  assert.deepEqual(await callsOf(page, 'den_invite'), [{ name: null }]);
  assert.equal(await page.evaluate(() => window.__clip), 'https://relay.leylinelauncher.com/join/dddddddddddddddd#DENSECRETDENSECR~G1~p');
  assert.match(await page.textContent('#solo-status'), /Open in Leyline.*24 hours/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Invite a friend from a Host pack realm: its invite link', async () => {
  const { page, ctx, errors } = await open({ kit: KIT, lite: false });
  await page.waitForSelector('#solo-home');
  await page.click('#solo-home .solo-invite');
  await page.waitForFunction(() => /Invite link copied/.test(document.getElementById('solo-status').textContent));
  assert.equal(await page.evaluate(() => window.__clip), 'https://relay.leylinelauncher.com/join/abcdefghijk2345a#HOSTLINKHOSTLINK');
  assert.equal((await callsOf(page, 'den_invite')).length, 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Invite with no world of yours yet says to Play solo first', async () => {
  const { page, ctx, errors } = await open({ lite: false });
  await page.click('#solo-home .solo-invite');
  await page.waitForFunction(() => /Play solo once first/.test(document.getElementById('solo-status').textContent));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('The friend: leyline://join shows one Join; nothing joins before it; Join knocks with the 3.3.5a folder', async () => {
  const { page, ctx, errors } = await open({ deep: { action: 'join', id: LINK }, clients: [VANILLA, WRATH] });
  await page.waitForSelector('#solo-join');
  assert.equal(await page.getAttribute('#solo-join', 'role'), 'dialog');
  assert.equal((await callsOf(page, 'join_invite')).length, 0, 'nothing before Join');
  await page.click('#solo-join-go');
  await page.waitForFunction(() => /signs you in by itself/.test(document.querySelector('.solo-join-status').textContent));
  assert.deepEqual(await callsOf(page, 'join_invite'), [{ client: 'c335a-1', link: LINK }]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('The friend without a 3.3.5a folder: told how, nothing joins', async () => {
  const { page, ctx, errors } = await open({ deep: { action: 'join', id: LINK }, clients: [VANILLA] });
  await page.waitForSelector('#solo-join');
  await page.click('#solo-join-go');
  await page.waitForFunction(() => /your own game folder, the same version/.test(document.querySelector('.solo-join-status').textContent));
  assert.equal((await callsOf(page, 'join_invite')).length, 0);
  await page.click('#solo-join .link-s');
  await page.waitForFunction(() => document.querySelector('#page-clients.on'));
  await page.click('#solo-join .btn:has-text("Not now")');
  assert.equal(await page.locator('#solo-join').count(), 0, 'Not now closes it');
  // a link that isn't an invite is never offered
  await page.evaluate(() => window.LeylineSolo.offerJoin('https://example.com/x'));
  assert.equal(await page.locator('#solo-join').count(), 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('GM command center: your realm, each control one checked call, search then Spawn, asked first where it hurts', async () => {
  const { page, ctx, errors } = await open({ kit: KIT, lite: 'stopped',
    answer: { 'sandbox:find-creature': '68\tCity Guard\n1001\tGuard Captain', 'sandbox:tele': 'Hosty is on the way to Dalaran.' } });
  await page.evaluate(() => window.LeylineNav.show('host'));
  await page.click('#gm-open-host');
  await page.waitForFunction(() => document.querySelector('#page-gm.on') && document.querySelector('.gm-grid'));
  assert.deepEqual(await page.$$eval('#gm-worlds .gm-world', (b) => b.map((x) => [x.dataset.world, x.getAttribute('aria-pressed')])),
    [['realm', 'true'], ['den', 'false'], ['official', 'false']]);
  assert.deepEqual(await page.$$eval('.gm-group h3', (h) => h.map((x) => x.textContent)), ['Move', 'Spawn', 'World', 'Character', 'Players & realm']);
  const gm = () => callsOf(page, 'host_gm');
  await page.selectOption('#gm-tele', 'dalaran');
  await page.click('.gm-group:nth-child(1) .gm-row:nth-child(3) .btn');
  await page.waitForFunction(() => /on the way to Dalaran/.test(document.getElementById('gm-status').textContent));
  assert.deepEqual((await gm()).pop(), { action: 'sandbox', data: 'tele\tdalaran\t' });
  // a friend by name
  await page.fill('#gm-who', 'Friendly');
  await page.dispatchEvent('#gm-who', 'change');
  await page.click('.gm-speed:has-text("3x")');
  await page.waitForFunction(() => /Done/.test(document.getElementById('gm-status').textContent));
  assert.deepEqual((await gm()).pop(), { action: 'sandbox', data: 'speed\t3\tFriendly' });
  await page.fill('#gm-who', 'Rob); DROP');
  await page.dispatchEvent('#gm-who', 'change');
  assert.equal(await page.textContent('#gm-status'), 'A character name: letters only.');
  assert.equal(await page.inputValue('#gm-who'), 'Friendly', 'kept the last good name');
  await page.fill('#gm-who', '');
  await page.dispatchEvent('#gm-who', 'change');
  // search, then Spawn
  await page.fill('#gm-spawn-q', 'guard');
  await page.press('#gm-spawn-q', 'Enter');
  await page.waitForFunction(() => document.querySelectorAll('#gm-spawn-found .gm-hit').length === 2);
  assert.deepEqual((await gm()).pop(), { action: 'sandbox', data: 'find-creature\tguard\t' });
  await page.click('#gm-spawn-found .gm-hit:first-child .btn');
  await page.waitForFunction(async () => window.__calls.filter(([c]) => c === 'host_gm').length >= 5);
  assert.deepEqual((await gm()).pop(), { action: 'sandbox', data: 'spawn-creature\t68\t' });
  // time and weather
  await page.selectOption('#gm-hour', '22');
  await page.click('.gm-group:nth-child(3) .gm-row:nth-child(3) .btn');
  await page.click('.gm-wx:has-text("Snow")');
  await page.waitForFunction(async () => window.__calls.filter(([c]) => c === 'host_gm').length >= 7);
  assert.deepEqual((await gm()).slice(-2), [{ action: 'sandbox', data: 'time\t22\t' }, { action: 'sandbox', data: 'weather\tsnow\t' }]);
  // asked first: No sends nothing, Yes sends
  const n = (await gm()).length;
  page.once('dialog', (d) => d.dismiss());
  await page.fill('#gm-kick', 'Grumpy');
  await page.click('.gm-group:nth-child(5) .gm-row:nth-child(4) .btn');
  await page.waitForTimeout(150);
  assert.equal((await gm()).length, n, 'No: nothing sent');
  page.once('dialog', (d) => { assert.match(d.message(), /Kick Grumpy/); d.accept(); });
  await page.click('.gm-group:nth-child(5) .gm-row:nth-child(4) .btn');
  await page.waitForFunction((n) => window.__calls.filter(([c]) => c === 'host_gm').length > n, n);
  assert.deepEqual((await gm()).pop(), { action: 'kick', data: 'Grumpy' });
  page.once('dialog', (d) => { assert.match(d.message(), /five minutes/); d.accept(); });
  await page.click('button:has-text("Restart the server")');
  await page.waitForFunction((n) => window.__calls.filter(([c]) => c === 'host_gm').length > n + 1, n);
  assert.deepEqual((await gm()).pop(), { action: 'restart', data: '' });
  // ban needs a reason; then asks
  await page.fill('#gm-ban', 'Grumpy');
  await page.fill('#gm-ban-why', 'x');
  await page.click('.gm-group:nth-child(5) .gm-row:nth-child(5) .btn');
  assert.equal(await page.textContent('#gm-status'), 'Say why (3 letters or more).');
  await page.fill('#gm-ban-why', 'Spam in chat');
  page.once('dialog', (d) => { assert.match(d.message(), /for 24 hours/); d.accept(); });
  await page.click('.gm-group:nth-child(5) .gm-row:nth-child(5) .btn');
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'host_gm').pop()[1].action === 'ban');
  assert.deepEqual((await gm()).pop(), { action: 'ban', data: 'Grumpy\t24\tSpam in chat' });
  assert.doesNotMatch(await page.evaluate(() => document.getElementById('page-gm').innerText), BRANDS);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('GM command center: off while the realm is stopped; Your Den in game; the official realm only as Leyline HQ', async () => {
  const { page, ctx, errors } = await open({ kit: KIT, stopped: true, lite: 'stopped' });
  await page.evaluate(() => window.LeylineGM.open());
  await page.waitForFunction(() => document.querySelector('.gm-grid'));
  assert.equal(await page.textContent('.gm-off'), 'Start your realm first.');
  assert.ok(await page.$$eval('.gm-grid button', (b) => b.every((x) => x.disabled)), 'every control off');
  await page.click('.gm-world[data-world="den"]');
  assert.match(await page.textContent('.gm-den'), /Den panel \(type \/den\)/);
  assert.equal(await page.textContent('.gm-den .solo-go'), 'Play solo');
  await page.click('.gm-world[data-world="official"]');
  assert.match(await page.textContent('.gm-official'), /never run from here.*checks you are staff/);
  await page.click('.gm-official .btn');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'open_link'));
  assert.deepEqual(await callsOf(page, 'open_link'), [{ name: 'hq' }]);
  assert.equal((await callsOf(page, 'host_gm')).length, 0, 'nothing sent anywhere');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('GM command center: another core or a linked server keeps its own console', async () => {
  const { page, ctx, errors } = await open({ kit: KIT, core: 'cmangos', lite: false });
  await page.evaluate(() => window.LeylineGM.open('realm'));
  await page.waitForFunction(() => /runs another core/.test((document.querySelector('#gm-body') || {}).textContent || ''));
  assert.equal(await page.locator('.gm-grid').count(), 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});
