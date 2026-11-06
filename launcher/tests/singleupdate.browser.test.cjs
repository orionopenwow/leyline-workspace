// The bots, always up to date (ui/singleupdate.js; owner 4 Nov, Single player Phase 4; docs/BOTS-UPDATES.md), on the
// Single player page's Manage card, in Chromium with a fake Tauri core:
//   * newer bots: "New bots" with the date chip and Update the bots; pressing it gets them (bots_update), a newer
//     Host pack first when they come with one (hostpack_install, set_host_kit), then asks: Restart now or Later;
//   * nothing is ever automatic: no download, no stop, no start until pressed;
//   * "Bots updated" once after the bots changed (a first look only marks them), dismissed with Got it;
//   * no kit or no answer: nothing shown, no errors; no brand names, no version numbers in sentences.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/singleupdate.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');

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
const STOPPED = { state: 'desktop_stopped', version: null, help: 'Open Docker Desktop.', get_url: 'https://docs.docker.com/desktop/', can_start: true };

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  window.__kit = o.kit;
  window.__dock = o.docker;
  window.__prog = { idle: true, kind: 'host', running: false, code: null, step: '', n: 0, of: 8, secs: 0, problem: null };
  window.__stopped = !!o.stopped;
  window.__bots = o.bots || null;
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
      case 'bots_status': if (!window.__bots) throw 'Get the Host pack first.'; return JSON.parse(JSON.stringify(window.__bots));
      case 'bots_update': window.__bots = Object.assign({}, window.__bots, { update: false, pack_first: false, engine: window.__bots.latest }); return 'The new bots are downloaded. They join at the next start of Single player.';
      case 'host_logs': return 'world: ready\nauth: ready';
      case 'hostpack_install': window.__kit = Object.assign({}, window.__kit, { dir: '/home/me/kit', is_kit: true }); return [true, '/home/me/kit'];
      case 'set_host_kit': return null;
      case 'hostpack_info': return window.__kit.is_kit ? { installed: { version: '7' }, available: { version: '7' }, update: false, kit_dir: '/home/me/kit' }
        : { installed: null, available: { version: '7', size: 400000 }, update: false, kit_dir: '/home/me/kit' };
      case 'docker_info': return JSON.parse(JSON.stringify(window.__dock));
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

const A = 'e88cc4099e69f35126b28942a412ca702d20453b';
const B = '05a1f6fceb8a32aa879b13f5f48934c6e46a774c';
const C = '3f2969ddb0da42b48b4fca050c02b19163ba1e20';
const pins = (pb, date, changes) => ({ playerbots: pb, core: C, date, changes });
const BOTS = (o) => Object.assign({ engine: pins(A, '2026-11-04', null), latest: pins(B, '2026-11-10', 12), update: true, pack_first: false, prebuilt: true, note: null }, o);
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(o) {
  const opts = Object.assign({ clients: [WRATH], kit: KIT('solo', 'Single player'), net: NET, docker: READY }, o);
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineSingle && window.LeylineSingleUpdate && window.__calls.some(([c]) => c === 'host_info'));
  await page.click('.side .nav[data-page="single"]');
  await page.waitForFunction(() => document.querySelector('#sp-run') && document.querySelector('#sp-run').textContent === 'Running');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'bots_status'));
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const order = (page, names) => page.evaluate((names) => window.__calls.map(([c]) => c).filter((c) => names.includes(c)), names);
async function wording(page) {
  const text = await page.evaluate(() => { const c = document.querySelector('#sp-bots').cloneNode(true); c.querySelectorAll('.rl-ver').forEach((x) => x.remove()); return c.innerText; });
  assert.doesNotMatch(text, BRANDS);
  assert.doesNotMatch(text, /sign in|sign-in|signin/i);
  assert.doesNotMatch(text, /\b\d+\.\d+\.\d+a?\b/);
  assert.doesNotMatch(text, /[0-9a-f]{7,}/, 'no commit ids for players');
  assert.doesNotMatch(text, /donat|\btip\b|patreon|ko-fi/i);
}

test('newer bots: offered on the Manage card, nothing automatic; Update the bots gets them, then Later keeps it running', async () => {
  const { page, ctx, errors } = await open({ bots: BOTS() });
  await page.waitForSelector('#sp-bots-update');
  assert.match(await page.textContent('#sp-bots'), /New bots/);
  assert.equal(await page.textContent('#sp-bots .rl-ver'), '10 Nov', 'the date as a chip');
  await wording(page);
  await page.waitForTimeout(400);
  for (const c of ['bots_update', 'hostpack_install', 'host_stop', 'host_start_quiet']) assert.equal((await callsOf(page, c)).length, 0, c + ': nothing by itself');
  await page.click('#sp-bots-update');
  await page.waitForSelector('#sp-bots-restart');
  assert.deepEqual(await order(page, ['hostpack_install', 'bots_update']), ['bots_update'], 'no pack when the bots come without one');
  assert.match(await page.textContent('#sp-status'), /downloaded/);
  assert.match(await page.textContent('#sp-bots'), /Restart Single player now/);
  await wording(page);
  await page.click('#sp-bots-later');
  await page.waitForFunction(() => /next start/.test(document.querySelector('#sp-status').textContent));
  assert.equal(await page.$('#sp-bots-restart'), null);
  assert.equal((await callsOf(page, 'host_stop')).length, 0, 'Later: not restarted');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('bots that come with a newer Host pack: the pack first, then the bots; Restart now stops and starts it', async () => {
  const { page, ctx, errors } = await open({ bots: BOTS({ pack_first: true }) });
  await page.click('#sp-bots-update');
  await page.waitForSelector('#sp-bots-restart');
  assert.deepEqual(await order(page, ['hostpack_install', 'set_host_kit', 'bots_update']), ['hostpack_install', 'set_host_kit', 'bots_update']);
  await page.click('#sp-bots-restart');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_start_quiet'));
  assert.deepEqual(await order(page, ['host_stop', 'host_start_quiet']), ['host_stop', 'host_start_quiet']);
  assert.deepEqual((await callsOf(page, 'host_start_quiet'))[0], { answers: { name: null, visibility: 'private', gm: null } });
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('"Bots updated" once after the bots changed; a first look only marks them; Got it', async () => {
  const { page, ctx, errors } = await open({ bots: BOTS({ update: false, latest: pins(A, '2026-11-04', null) }) });
  await page.waitForFunction(() => localStorage.getItem('leyline.bots.seen'));
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.bots.seen')), A);
  assert.equal(await page.$('#sp-bots-news'), null, 'a first look: nothing to say');
  assert.equal(await page.$('#sp-bots-update'), null);
  // the realm now runs newer bots (updated here, or on the Host page)
  await page.evaluate((B) => { window.__bots = Object.assign({}, window.__bots, { engine: { playerbots: B, core: 'x', date: '2026-11-10', changes: 12 } }); return window.LeylineSingleUpdate.refresh(true); }, B);
  await page.waitForSelector('#sp-bots-news');
  assert.match(await page.textContent('#sp-bots-news'), /Bots updated/);
  assert.match(await page.textContent('#sp-bots-news'), /12 changes to how your bots play/);
  assert.equal(await page.textContent('#sp-bots-news .rl-ver'), '10 Nov');
  await wording(page);
  await page.click('#sp-bots-seen');
  await page.waitForFunction(() => !document.querySelector('#sp-bots-news'));
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.bots.seen')), B);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('built here, no answer, or nothing new: no button; the note when there is one; no errors', async () => {
  let r = await open({ bots: BOTS({ update: false, prebuilt: false, note: 'This realm runs images built on this PC (or stock ones): its bots update when you build it again.' }) });
  await r.page.waitForFunction(() => /built on this PC/.test(document.querySelector('#sp-bots').textContent));
  assert.equal(await r.page.$('#sp-bots-update'), null);
  assert.deepEqual(r.errors, []);
  await r.ctx.close();
  r = await open({ bots: null });
  await r.page.waitForTimeout(300);
  assert.equal(await r.page.textContent('#sp-bots'), '');
  assert.deepEqual(await r.page.$$eval('#sp-manage > .sp-row > button', (l) => l.map((x) => x.textContent)), ['Play', 'Stop', 'Start', 'Logs', 'Settings', 'Invite a friend'], 'the card as before');
  assert.deepEqual(r.errors, []);
  await r.ctx.close();
});

test('the date chip', async () => {
  const { page, ctx } = await open({ bots: null });
  assert.deepEqual(await page.evaluate(() => ['2026-11-10', '2026-01-02', 'soon', ''].map(window.LeylineSingleUpdate.day)), ['10 Nov', '2 Jan', '', '']);
  await ctx.close();
});
