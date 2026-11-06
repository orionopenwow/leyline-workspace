// Play with friends (ui/hostfriends.js; app.rs host_ready / host_ready_fix / realm_answers, host/ready.rs; owner 5 Nov:
// "highly develop the boarding to solo and joint private realms ... invite a friend easy 1 step and guide and
// troubleshoot"), in Chromium with a fake Tauri core:
//   * the Host page's banner opens the one window; Invite a friend makes the link, then Ready for friends? runs before
//     the link is shown; all green: copied; a red line: not copied, its fix sentence and button (Start again invites
//     again, Open sleep settings asks the core), Check again; Copy diagnostics never carries an invite or a password;
//   * Your Den's Share card shows the same checklist under its Invite a friend;
//   * the friend: Join's steps (reaching the host, the host answering, logging in) and the one fix when stuck, on the
//     leyline://join card and on Realms' Got an invite link?;
//   * the guide's troubleshooting table; no game or publisher names anywhere.
//
//   node --test launcher/tests/hostfriends.browser.test.cjs
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
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;
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
      case 'join_invite': if (o.joinFails) throw o.joinFails; return { address: 'relay.openvanilla.app:40007', name: 'Hosty world', guest: args.link.includes('~') ? { auto: true, account: 'G1', password: '', why: '', message: '' } : null };
      // Ready for friends? (host/ready.rs): o.ready (a function of the call count) or all green
      case 'host_ready': if (o.readyFn) return eval(o.readyFn)(calls.filter(([c]) => c === 'host_ready').length, args); return { kind: args.kind, ok: true, relay: 'relay.leylinelauncher.com', realm_id: 'dddddddddddddddd', relay_ms: 30, path_ms: 80, os: 'windows', diagnostics: '[ok] server',
        checks: [{ id: 'server', state: 'ok', label: 'Your world is running', line: 'Running.', fix: '', action: '' }, { id: 'path', state: 'ok', label: 'A friend can reach you', line: 'A test login went through.', fix: '', action: '' }] };
      case 'realm_answers': if (o.answers) throw o.answers; return 40;
      case 'host_ready_fix': return 'Your power settings are open: set Sleep to Never (plugged in), then press Check again.';
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
  const opts = Object.assign({ clients: [WRATH, VANILLA], kit: NO_KIT, net: NET, docker: READY, modes: CATALOG.modes, packs: PACKS, lite: 'running' }, o);
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://**', (r) => r.abort());
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineHostFriends && window.__calls.some(([c]) => c === 'host_info'));
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const SECRET = 'DENSECRETDENSECR';
// the first check red (the world not answering), the rest green, with a diagnostics text that (wrongly) has the link in it
const RED = `(n) => ({ kind: 'lite', ok: n > 1, relay: 'relay.leylinelauncher.com', realm_id: 'dddddddddddddddd', relay_ms: 220, path_ms: null, os: 'windows',
  diagnostics: 'see https://relay.leylinelauncher.com/join/dddddddddddddddd#${SECRET}~G1~p password hunter22',
  checks: [
    n > 1 ? { id: 'server', state: 'ok', label: 'Your world is running', line: 'Running.', fix: '', action: '' }
      : { id: 'server', state: 'bad', label: 'Your world is running', line: "Your Den isn't running on this PC.", fix: 'Press Start again.', action: 'start' },
    { id: 'speed', state: 'warn', label: 'Your connection', line: 'About 220 ms to the relay: friends may feel lag.', fix: 'Use a wired connection.', action: '' },
    { id: 'sleep', state: 'warn', label: 'Your PC stays awake', line: 'This PC sleeps after 30 minutes.', fix: 'Set Sleep to Never.', action: 'power' },
  ] })`;

test('the Host page banner: one press makes the link, checks the way in, and copies it once green', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('host'));
  await page.waitForSelector('#wf-banner');
  await page.click('#wf-banner-go');
  await page.waitForFunction(() => /Ready: your invite link is copied/.test(document.getElementById('wf-status').textContent));
  assert.deepEqual(await callsOf(page, 'den_invite'), [{ name: null }]);
  assert.deepEqual(await callsOf(page, 'host_ready'), [{ kind: 'lite' }]);
  assert.equal(await page.evaluate(() => window.__clip), 'https://relay.leylinelauncher.com/join/dddddddddddddddd#DENSECRETDENSECR~G1~p');
  assert.equal(await page.$eval('#wf-ready', (e) => e.dataset.state), 'ok');
  assert.equal(await page.locator('#wf-ready .wf-check.ok').count(), 2);
  assert.equal(await page.isVisible('#wf-link-box'), true);
  assert.equal(await page.inputValue('#wf-link'), 'https://relay.leylinelauncher.com/join/dddddddddddddddd#DENSECRETDENSECR~G1~p');
  // the leyline:// form for a friend who has Leyline open
  await page.click('#wf-copy-ll');
  assert.equal(await page.evaluate(() => window.__clip), 'leyline://join?link=' + encodeURIComponent('https://relay.leylinelauncher.com/join/dddddddddddddddd#DENSECRETDENSECR~G1~p'));
  // the guide's troubleshooting table
  await page.click('#wf-guide summary');
  const rows = await page.$$eval('#wf-trouble tr td:first-child', (l) => l.map((x) => x.textContent));
  for (const want of [/Connecting and Connected/, /laggy/, /isn't valid/, /can't make an account/]) assert.ok(rows.some((r) => want.test(r)), String(want));
  assert.doesNotMatch(await page.textContent('#wf'), BRANDS);
  await page.keyboard.press('Escape');
  assert.equal(await page.isVisible('#wf'), false);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a red line: the link is not copied, the fix says what to do, Start again invites again and turns it green', async () => {
  const { page, ctx, errors } = await open({ readyFn: RED });
  await page.evaluate(() => window.LeylineHostFriends.open({ go: true }));
  await page.waitForFunction(() => document.getElementById('wf-ready') && document.getElementById('wf-ready').dataset.state === 'bad');
  assert.equal(await page.evaluate(() => window.__clip), null, 'never copied while red');
  assert.match(await page.textContent('#wf-status'), /Not ready yet/);
  assert.match(await page.textContent('#wf-c-server'), /isn't running.*Press Start again/);
  assert.equal(await page.$eval('#wf-c-speed', (e) => e.className), 'wf-check warn');
  // sleep: its button asks the core to open the PC's settings
  await page.click('#wf-c-sleep-fix');
  await page.waitForFunction(() => /power settings are open/.test(document.querySelector('#wf-ready .wf-ready-note').textContent));
  assert.deepEqual(await callsOf(page, 'host_ready_fix'), [{ what: 'power' }]);
  // Copy diagnostics: no invite secret, no password, even if the core's text had one
  await page.click('#wf-ready-diag');
  const clip = await page.evaluate(() => window.__clip);
  assert.doesNotMatch(clip, new RegExp(SECRET));
  assert.doesNotMatch(clip, /hunter22|~G1/);
  assert.match(clip, /\/join\/dddddddddddddddd#…/);
  // Start again: a new invite and a new check, now green and copied
  await page.click('#wf-c-server-fix');
  await page.waitForFunction(() => /Ready/.test(document.getElementById('wf-status').textContent) && document.getElementById('wf-ready').dataset.state !== 'bad');
  assert.equal((await callsOf(page, 'den_invite')).length, 2);
  assert.equal((await callsOf(page, 'host_ready')).length, 2);
  assert.match(await page.evaluate(() => window.__clip), /^https:\/\/relay/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test("Your Den's Share card: Invite a friend shows the same checklist", async () => {
  const { page, ctx, errors } = await open({ readyFn: RED });
  await page.evaluate(() => { window.LeylineNav.show('den'); });
  await page.waitForSelector('#yd-share-invite');
  await page.click('#yd-share-invite');
  await page.waitForFunction(() => document.getElementById('yd-ready') && document.getElementById('yd-ready').dataset.state === 'bad');
  assert.match(await page.textContent('#yd-c-server'), /Press Start again/);
  assert.equal(await page.locator('#yd-ready .wf-check').count(), 3);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test("the friend: Join's steps when the host's world isn't answering, and when it is", async () => {
  const { page, ctx, errors } = await open({ deep: { action: 'join', id: LINK }, answers: "the connection closed without an answer (the world behind it isn't answering)" });
  await page.waitForSelector('#solo-join');
  assert.equal(await page.locator('#solo-join-steps').count(), 0, 'nothing before Join');
  await page.click('#solo-join-go');
  await page.waitForFunction(() => document.querySelector('#solo-join-steps [data-step="answer"]').className === 'wf-step bad');
  assert.equal(await page.$eval('#solo-join-steps [data-step="reach"]', (e) => e.className), 'wf-step ok');
  assert.match(await page.textContent('#solo-join-steps'), /isn't answering/);
  assert.match(await page.textContent('.solo-join-status'), /Tell your friend: open Leyline and press Invite a friend/);
  assert.deepEqual(await callsOf(page, 'realm_answers'), [{ address: 'relay.openvanilla.app:40007' }]);
  assert.deepEqual(errors, []);
  await ctx.close();
  const b = await open({ deep: { action: 'join', id: LINK } });
  await b.page.waitForSelector('#solo-join');
  await b.page.click('#solo-join-go');
  await b.page.waitForFunction(() => document.querySelector('#solo-join-steps [data-step="login"]').className === 'wf-step ok');
  assert.match(await b.page.textContent('#solo-join-steps [data-step="answer"]'), /Yes \(40 ms\)/);
  assert.deepEqual(b.errors, []);
  await b.ctx.close();
});

test('the friend: a host that is offline, an old link, no game folder: each says the one fix', async () => {
  for (const [fails, step, fix] of [
    ['The Leyline relay: realm offline', 'reach', /open Leyline.*keep their PC awake/],
    ['The Leyline relay: this invite link is not valid (the host may have made a new one)', 'reach', /Ask your friend for a new link/],
  ]) {
    const { page, ctx, errors } = await open({ deep: { action: 'join', id: LINK }, joinFails: fails });
    await page.waitForSelector('#solo-join');
    await page.click('#solo-join-go');
    await page.waitForFunction((s) => document.querySelector(`#solo-join-steps [data-step="${s}"]`).className === 'wf-step bad', step);
    assert.match(await page.textContent('.solo-join-status'), fix);
    assert.equal(await page.$eval('#solo-join-steps [data-step="login"]', (e) => e.className), 'wf-step skip');
    assert.equal(await page.isDisabled('#solo-join-go'), false, 'Join again');
    assert.deepEqual(errors, []);
    await ctx.close();
  }
  const { page, ctx } = await open();
  const x = await page.evaluate(() => [
    window.LeylineHostFriends.explainJoin("Your friend's Den plays with a Wrath game folder: pick that one."),
    window.LeylineHostFriends.explainJoin('private realm: open your invite link on this computer first'),
    window.LeylineHostFriends.explainJoin('too many connections from your address'),
    window.LeylineHostFriends.redact('https://relay.x/join/dddddddddddddddd#SECRETSECRETSECRET~~G1.PASSWORDPASS ok password: hunter2'),
  ]);
  assert.equal(x[0].step, 'folder');
  assert.match(x[1].fix, /Open the invite link on this PC/);
  assert.match(x[2].fix, /Wait a few minutes/);
  assert.doesNotMatch(x[3], /SECRET|PASSWORDPASS|hunter2/);
  await ctx.close();
});

test("Realms' Got an invite link?: a stuck Join says the fix, a good one says the host's world answers", async () => {
  const { page, ctx, errors } = await open({ joinFails: 'The Leyline relay: realm offline' });
  await page.evaluate(() => window.LeylineNav.show('realms'));
  await page.evaluate(() => { const o = document.getElementById('rl-other'); if (o) o.open = true; });
  await page.fill('#rl-invite', LINK);
  await page.click('#rl-join');
  await page.waitForFunction(() => /isn't online right now.*open Leyline/.test(document.getElementById('rl-status').textContent));
  assert.deepEqual(errors, []);
  await ctx.close();
});
