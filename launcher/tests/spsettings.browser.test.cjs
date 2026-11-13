// Single player's world settings and its friend and info parts (owner 12 Nov), in Chromium with a fake Tauri core:
//   * "Your world" (oneclick.js view "world"): the first Play of a world not set up whose settings weren't chosen shows
//     what happens and the settings BEFORE any setup; Set up my world saves them (world_settings_set) and goes on;
//     Cancel starts nothing; never again once chosen, never for a world set up;
//   * the quiet defaults (companions Off, dungeon buddies On, Quiet, the profile's XP);
//   * the Single player page's settings (#wss): each change saved at once; a running world says "next start" and
//     offers Restart; a refused value is put back;
//   * the page's own Start (single.js) asks first too; the setup card's plain step names and the first-time line;
//   * Play with a friend (#pf): Invite a friend → one link with Copy, the ways, and the friend's state
//     (none → invited → connected);
//   * the "i" button: a bubble for every top-level page with Learn more (open_link), Read the docs elsewhere, Esc and a
//     click outside close it.
// Screenshots: SPSETTINGS_SHOTS=<dir> (default /mnt/user-data/outputs/gallery when it exists), spsettings-*.png.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/spsettings.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.SPSETTINGS_SHOTS || (fs.existsSync('/mnt/user-data/outputs') ? '/mnt/user-data/outputs/gallery' : '');
const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const WS = (o) => Object.assign({ supported: true, installed: false, set_up: false, running: false, backups: 0, other_running: null, account: '', password: '' }, o);
const SET = (o) => Object.assign({ companions_auto: false, companions_follow: 0, dungeon_auto: true, roaming_bots: 0, xp_rate: 2, chosen: false, set_up: false, running: false }, o);
const STEPS = [
  { step: 'Getting the engine (one-time download)', n: 1, of: 7, pct: 40 },
  { step: 'Filling the world: placing creatures, quests and items into its database (one time)', n: 5, of: 7, pct: 50 },
  { step: 'Starting your world', n: 7, of: 7, pct: 10 },
];

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try {
    if (!sessionStorage.getItem('__booted')) {
      sessionStorage.setItem('__booted', '1');
      localStorage.clear();
      localStorage.setItem('leyline.mode', 'online');
    }
  } catch (_) { /* fine */ }
  window.__ws = o.worlds;
  window.__set = o.sets;
  window.__o = o;
  window.__net = o.net || { running: false, connected: false, relay: '', realm_id: '', address: '', auth_port: 0, world_port: 0, streams: 0, last_error: '' };
  window.__eprog = { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7 };
  const st = { version: 'test', settings: { clients: o.clients, selected_client: o.clients[0] ? o.clients[0].id : null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  const job = { single: 'engine', vanilla: 'vanilla', bossrush: 'bossrush' };
  const start = (kind) => {
    window.__seq = o.steps.map((s) => Object.assign({ idle: false, kind: job[kind], running: true, code: null, detail: '' }, s));
    window.__eprog = window.__seq[0];
    window.__kind = kind;
  };
  const missing = new Set(o.missing || []);
  const copy = (x) => JSON.parse(JSON.stringify(x));
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    if (missing.has(cmd)) throw `Command ${cmd} not found`;
    switch (cmd) {
      case 'get_state': return copy(st);
      case 'select': return null;
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'docker_info': return { state: 'ready', version: '27', help: '', get_url: '', can_start: false };
      case 'host_console': throw 'no kit';
      case 'host_progress': return { idle: true, kind: 'host', running: false, code: null };
      case 'social_status': return true;
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'engine_status': return copy(window.__ws.single);
      case 'world_status': return copy(Object.assign({}, window.__ws[args.kind], { kind: args.kind }));
      case 'world_find_folder': return { picked: 'c335a-1', found: [Object.assign({ added: true }, { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a' })] };
      case 'world_preflight': return { ok: true, free_gb: 80, need_gb: 6, message: '', download_mb: 380, download_known: true };
      case 'world_resume_pending': return null;
      case 'world_settings_get': if (window.__o.getErr) throw window.__o.getErr; return copy(Object.assign({}, window.__set[args.kind], { kind: args.kind, running: !!window.__ws[args.kind].running, set_up: !!window.__ws[args.kind].set_up }));
      case 'world_settings_set': {
        if (window.__o.setErr) { const e = window.__o.setErr; window.__o.setErr = null; throw e; }
        const s = window.__set[args.kind];
        for (const k of ['companions_auto', 'companions_follow', 'dungeon_auto', 'roaming_bots', 'xp_rate']) if (args[k] !== undefined) s[k] = args[k];
        s.chosen = true;
        const running = !!window.__ws[args.kind].running;
        return { settings: copy(Object.assign({}, s, { kind: args.kind, running })), restart: running,
          message: running ? 'Saved. Single player is running: they apply at its next start (Restart now, or the next time you play).' : 'Saved. They apply when your world starts.' };
      }
      case 'world_setup': case 'world_start': case 'engine_setup': case 'engine_start': start(args && args.kind ? args.kind : 'single'); return 'Starting…';
      case 'engine_progress': {
        if (!window.__eprog.running) return copy(window.__eprog);
        if (window.__seq.length > 1) { window.__seq.shift(); window.__eprog = window.__seq[0]; return copy(window.__eprog); }
        if (window.__o.hold) return copy(window.__eprog);
        window.__eprog = Object.assign({}, window.__eprog, { running: false, code: 0, step: 'Ready' });
        window.__ws[window.__kind] = Object.assign({}, window.__ws[window.__kind], { set_up: true, running: true, installed: true });
        return copy(window.__eprog);
      }
      case 'world_stop': case 'engine_stop': {
        const k = cmd === 'engine_stop' ? 'single' : args.kind;
        window.__ws[k] = Object.assign({}, window.__ws[k], { running: false });
        return 'Stopped. Everyone was saved.';
      }
      case 'local_characters': return [{ kind: 'single', title: 'Single player', chars: [{ name: 'Odo' }] }, { kind: 'vanilla', title: 'Original world', chars: [{ name: 'Brakka' }] }];
      case 'world_play': return { message: 'Started.', kind: args.kind, auto: true, account: 'SOLO', password: '', made: '', hero: false };
      case 'set_world_stop_on_exit': st.settings.world_stop_on_exit = args.on; return null;
      case 'world_invite': return { link: 'https://relay.leylinelauncher.com/join/abcdefghijklmnop#k~~via', code: 'DEN-ABCD', name: 'your friend', expires: 0,
        tunnel: { running: true, connected: true, streams: 0 }, ways: [{ id: 'lan', ok: true, line: 'Same network: on, at 192.168.1.20.' }, { id: 'own', ok: false, line: 'Your own address: not used.' }, { id: 'relay', ok: true, line: 'Leyline relay: on.' }] };
      case 'den_net_state': return copy(window.__net);
      case 'open_link': if (window.__o.noLink && args.name !== 'docs') throw 'Unknown link.'; return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });
async function open(o) {
  const opts = Object.assign({ clients: [WRATH], worlds: { single: WS({}), vanilla: WS({}), bossrush: WS({}) },
    sets: { single: SET({}), vanilla: SET({}) }, steps: STEPS }, o);
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: 1360, height: 880 } });
  await ctx.grantPermissions(['clipboard-read', 'clipboard-write']);
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineOneClick && window.LeylineWorlds && window.LeylineInfo);
  await page.evaluate(() => { window.LeylineOneClick.cfg.pollMs = 150; window.LeylineOneClick.cfg.minRateMs = 250; });
  return { page, ctx, errors };
}
const cmds = (page) => page.evaluate(() => window.__calls.map(([c]) => c));
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const waitView = (page, v) => page.waitForFunction((v) => { const c = document.getElementById('oc'); return !c.hidden && c.dataset.view === v; }, v, { timeout: 15000 });
const checked = (page, id) => page.$eval(`#${id} [aria-checked="true"]`, (b) => b.textContent);
const noBrand = async (page) => assert.doesNotMatch(await page.evaluate(() => document.body.innerText), /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard|3\.3\.5a? |1\.12/i);
async function shot(page, name) { if (!SHOTS) return; await page.waitForTimeout(350); await page.screenshot({ path: path.join(SHOTS, `spsettings-${name}.png`) }); }
async function homeGo(page) {
  await page.evaluate(() => window.LeylineNav.show('play'));
  await page.waitForSelector('#hm-oneclick:not([hidden]) #hm-oneclick-go');
  await page.click('#hm-oneclick-go');
}

test('the first Play: Your world before any setup, quiet defaults, the settings saved before the setup starts', async () => {
  const { page, ctx, errors } = await open({ hold: true });
  await homeGo(page);
  await waitView(page, 'world');
  assert.equal(await page.textContent('#oc-title'), 'Your world');
  // nothing is downloaded or set up yet
  assert.ok(!(await cmds(page)).some((c) => c === 'world_setup' || c === 'engine_setup'));
  // what happens, in plain words, with the download's size
  const steps = await page.$$eval('#ws-steps li', (l) => l.map((x) => x.textContent));
  assert.equal(steps.length, 5);
  assert.match(steps[0], /one-time download of about 380 MB/);
  assert.match(steps[1], /maps from your own game folder/);
  assert.match(steps[2], /Fills the world: every creature, quest, item, vendor and spell/);
  assert.match(steps[3], /local login/);
  assert.match(await page.textContent('#ws-why'), /5–20 minutes.*seconds/);
  assert.match(await page.textContent('#ws-friend'), /own game folder.*Leyline relay.*Social panel's Leyline tab/s);
  // the quiet defaults
  assert.equal(await checked(page, 'ws-follow'), 'Off');
  assert.equal(await checked(page, 'ws-bots'), 'Quiet');
  assert.equal(await checked(page, 'ws-xp'), '2x');
  assert.equal(await page.isChecked('#ws-dungeon'), true);
  await noBrand(page);
  await shot(page, 'panel');
  // choose, then Set up my world
  await page.click('#ws-follow [data-v="2"]');
  await page.click('#ws-bots [data-v="60"]');
  await page.click('#ws-xp [data-v="1.5"]');
  assert.equal(await checked(page, 'ws-follow'), '2');
  await shot(page, 'panel-chosen');
  await page.click('#ws-go');
  await waitView(page, 'setup');
  assert.deepEqual(await callsOf(page, 'world_settings_set'), [{ kind: 'single', companions_auto: true, companions_follow: 2, dungeon_auto: true, roaming_bots: 60, xp_rate: 1.5 }]);
  const order = await cmds(page);
  assert.ok(order.indexOf('world_settings_set') < order.indexOf('world_setup'), 'saved before the setup starts');
  // the setup card: plain step names with a line each, the first-time line
  await page.waitForSelector('#oc-steps li[data-state="now"] .oc-step-s');
  assert.equal(await page.textContent('#oc-first'), 'First time only: 5–20 minutes. After this, your world starts in seconds.');
  await page.waitForFunction(() => /Filling the world/.test((document.querySelector('#oc-steps li[data-state="now"] .oc-step-t') || {}).textContent || ''), null, { timeout: 15000 });
  assert.match(await page.textContent('#oc-steps li[data-state="now"] .oc-step-s'), /Tens of thousands/);
  await shot(page, 'setup-card');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Cancel starts nothing; chosen or set up: no panel', async () => {
  let { page, ctx } = await open({});
  await homeGo(page);
  await waitView(page, 'world');
  await page.click('#ws-cancel');
  await page.waitForFunction(() => document.getElementById('oc').hidden);
  await page.waitForTimeout(300);
  const c = await cmds(page);
  assert.ok(!c.includes('world_setup') && !c.includes('world_settings_set'));
  assert.equal((await page.evaluate(() => window.LeylineOneClick.state())).busy, false);
  await ctx.close();
  // chosen already
  ({ page, ctx } = await open({ hold: true, sets: { single: SET({ chosen: true }), vanilla: SET({}) } }));
  await homeGo(page);
  await waitView(page, 'setup');
  assert.deepEqual(await callsOf(page, 'world_settings_set'), []);
  await ctx.close();
  // set up already: straight in
  ({ page, ctx } = await open({ worlds: { single: WS({ installed: true, set_up: true, running: true }), vanilla: WS({}), bossrush: WS({}) } }));
  await homeGo(page);
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'));
  assert.deepEqual(await callsOf(page, 'world_settings_set'), []);
  assert.equal(await page.$('#ws-go'), null, 'no Your world panel');
  await ctx.close();
  // an older core without the command: today's flow
  ({ page, ctx } = await open({ hold: true, missing: ['world_settings_get', 'world_settings_set'] }));
  await homeGo(page);
  await waitView(page, 'setup');
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#oc-stop-exit');
  await page.waitForTimeout(300);
  assert.equal(await page.$('#wss'), null, 'no settings section without the command');
  await ctx.close();
});

test("the page's Start asks first too, and the original world's Play", async () => {
  const { page, ctx } = await open({ hold: true });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#sp-start:not([disabled])');
  await page.click('#sp-start');
  await waitView(page, 'world');
  assert.deepEqual(await callsOf(page, 'engine_setup'), []);
  await page.click('#ws-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'engine_setup'));
  assert.equal((await callsOf(page, 'world_settings_set')).length, 1);
  await page.waitForSelector('#sp-first');
  assert.equal(await page.textContent('#sp-first'), 'First time only: 5–20 minutes. After this, your world starts in seconds.');
  await page.waitForSelector('#sp-prog-sub');
  await shot(page, 'start-card');
  // the original world: its own panel title
  await page.evaluate(() => { window.__o.hold = false; });
  await page.click('#sv-play');
  await waitView(page, 'world');
  assert.equal(await page.textContent('#oc-title'), 'Your original world');
  await page.click('#ws-cancel');
  await page.waitForTimeout(300);
  assert.ok(!(await callsOf(page, 'world_setup')).some((a) => a.kind === 'vanilla'));
  await ctx.close();
});

test('the page settings: saved at once, a running world says next start and offers Restart, a refused value goes back', async () => {
  const { page, ctx, errors } = await open({ worlds: { single: WS({ installed: true, set_up: true, running: true }), vanilla: WS({}), bossrush: WS({}) },
    sets: { single: SET({ chosen: true, companions_auto: true, companions_follow: 2, roaming_bots: 7 }), vanilla: SET({}) } });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#wss-form');
  assert.equal(await checked(page, 'wss-follow'), '2');
  assert.equal(await checked(page, 'wss-bots'), 'Custom (7)', 'a value not in the list is shown as it is');
  await page.click('#wss-bots [data-v="150"]');
  await page.waitForSelector('#wss-restart');
  assert.deepEqual((await callsOf(page, 'world_settings_set')).pop(), { kind: 'single', roaming_bots: 150 });
  assert.match(await page.textContent('#wss-status'), /next start/);
  assert.equal(await checked(page, 'wss-bots'), 'Busy');
  await page.$eval('#oc-set', (e) => e.scrollIntoView());
  await shot(page, 'page-settings');
  await page.click('#wss-restart');
  await page.waitForFunction(() => /Restarting with your settings/.test(document.getElementById('wss-status').textContent));
  const c = await cmds(page);
  assert.ok(c.lastIndexOf('engine_stop') < c.lastIndexOf('world_start'));
  assert.equal(await page.$('#wss-restart'), null);
  // companions off: both keys
  await page.click('#wss-follow [data-v="0"]');
  await page.waitForFunction(() => /Saved/.test(document.getElementById('wss-status').textContent));
  assert.deepEqual((await callsOf(page, 'world_settings_set')).pop(), { kind: 'single', companions_auto: false, companions_follow: 0 });
  // refused: put back
  await page.evaluate(() => { window.__o.setErr = '[other] XP rate: 0.5 to 10.'; });
  await page.click('#wss-xp [data-v="1"]');
  await page.waitForFunction(() => /XP rate/.test(document.getElementById('wss-status').textContent));
  assert.equal(await checked(page, 'wss-xp'), '2x');
  // the other world
  await page.selectOption('#wss-kind', 'vanilla');
  await page.waitForSelector('#wss-form');
  assert.deepEqual((await callsOf(page, 'world_settings_get')).pop(), { kind: 'vanilla' });
  assert.equal(await checked(page, 'wss-follow'), 'Off');
  await noBrand(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Play with a friend: how a friend joins, Invite a friend gives one link with Copy, then invited and connected', async () => {
  const { page, ctx, errors } = await open({});
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#pf-invite');
  assert.equal(await page.getAttribute('#pf-status', 'data-state'), 'none');
  assert.match(await page.textContent('#pf'), /Leyline and their own game folder.*same network.*Leyline relay.*Social panel's Leyline tab/s);
  // no world running: says to start it
  await page.click('#pf-invite');
  await page.waitForFunction(() => /Start your world first/.test(document.getElementById('pf-line').textContent));
  assert.deepEqual(await callsOf(page, 'world_invite'), []);
  // running: one link
  await page.evaluate(() => { window.__ws.single = Object.assign({}, window.__ws.single, { set_up: true, installed: true, running: true }); });
  await page.click('#pf-invite');
  await page.waitForSelector('#pf-link');
  assert.deepEqual(await callsOf(page, 'world_invite'), [{ kind: 'single' }]);
  assert.match(await page.inputValue('#pf-link'), /^https:\/\/relay\.leylinelauncher\.com\/join\//);
  assert.equal(await page.getAttribute('#pf-status', 'data-state'), 'invited');
  assert.deepEqual(await page.$$eval('#pf-ways li', (l) => l.map((x) => x.textContent)), ['Same network: on, at 192.168.1.20.', 'Leyline relay: on.']);
  await page.evaluate(() => navigator.clipboard.writeText(''));
  await page.click('#pf-copy');
  assert.match(await page.evaluate(() => navigator.clipboard.readText()), /join\/abcdefghijklmnop/);
  await page.$eval('#pf', (e) => e.scrollIntoView());
  await shot(page, 'friend-invited');
  // the friend comes in
  await page.evaluate(() => { window.__net = { running: true, connected: true, relay: 'r', realm_id: 'x', address: '', auth_port: 0, world_port: 0, streams: 1, last_error: '' }; });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForFunction(() => document.getElementById('pf-status').dataset.state === 'connected');
  assert.equal(await page.textContent('#pf-status'), 'Your friend is in');
  await shot(page, 'friend-connected');
  await noBrand(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the i button: a bubble for every top-level page, Learn more opens its docs, Esc and a click outside close it', async () => {
  const { page, ctx, errors } = await open({});
  const want = { play: ['Home', 'gettingstarted'], single: ['Single player', 'docsingle'], realms: ['Realms', 'docsplay'], modes: ['Modes', 'docsmodes'],
    mods: ['Mods & Addons', 'docsmods'], host: ['Host', 'hostarealm'], forum: ['Forum', 'docslauncher'], news: ['News', 'docslauncher'], settings: ['Settings', 'docslauncher'] };
  for (const [pg, [title, link]] of Object.entries(want)) {
    await page.evaluate((p) => window.LeylineNav.show(p), pg);
    await page.click('#ll-info');
    await page.waitForSelector('#ll-info-b:not([hidden])');
    // Modes: its Leaderboards panel has its own bubble when it is mid-screen (a short Modes list here)
    if (pg === 'modes') assert.ok(['Modes', 'Leaderboards'].includes(await page.textContent('#ll-info-t')), pg);
    else assert.equal(await page.textContent('#ll-info-t'), title, pg);
    const lines = await page.$$eval('#ll-info-b p', (l) => l.length);
    assert.ok(lines >= 1 && lines <= 4, `${pg}: ${lines} lines`);
    assert.equal(await page.getAttribute('#ll-info-more', 'data-link'), link);
    assert.equal(await page.textContent('#ll-info-more'), 'Learn more');
    await noBrand(page);
    if (pg === 'single') await shot(page, 'info-single');
    await page.keyboard.press('Escape');
    await page.waitForSelector('#ll-info-b[hidden]', { state: 'attached' });
    assert.equal(await page.evaluate(() => document.activeElement && document.activeElement.id), 'll-info', 'focus back on the button');
  }
  // Learn more: open_link with the page's docs
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.focus('#ll-info');
  await page.keyboard.press('Enter');
  await page.waitForSelector('#ll-info-b:not([hidden])');
  assert.equal(await page.evaluate(() => document.activeElement && document.activeElement.id), 'll-info-more');
  await page.click('#ll-info-more');
  assert.deepEqual((await callsOf(page, 'open_link')).pop(), { name: 'docsingle' });
  await page.waitForSelector('#ll-info-b[hidden]', { state: 'attached' });
  // a page with no bubble of its own (none today: info.browser.test.cjs checks every page): Read the docs
  await page.evaluate(() => document.dispatchEvent(new CustomEvent('leyline:page', { detail: { page: 'nowhere' } })));
  await page.click('#ll-info');
  assert.equal(await page.textContent('#ll-info-more'), 'Read the docs');
  assert.equal(await page.getAttribute('#ll-info-more', 'data-link'), 'docs');
  // a click outside closes it
  await page.mouse.click(400, 300);
  await page.waitForSelector('#ll-info-b[hidden]', { state: 'attached' });
  // an older core without that link: the docs' front page
  await page.evaluate(() => { window.__o.noLink = true; window.LeylineNav.show('mods'); });
  await page.click('#ll-info');
  await page.click('#ll-info-more');
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'open_link').length >= 3);
  assert.deepEqual((await callsOf(page, 'open_link')).slice(-2), [{ name: 'docsmods' }, { name: 'docs' }]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('one Friends area on the Single player page: invite, friends on your world, your friends list; its own i bubble', async () => {
  const { page, ctx, errors } = await open({ worlds: { single: WS({ set_up: true, installed: true, running: true }), vanilla: WS({}), bossrush: WS({}) } });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#pf-invite');
  // the parts in one panel, in reading order, after the settings and the world's mods (spmods.js #spm)
  const order = await page.evaluate(() => [...document.querySelectorAll('#page-single > .panel')].map((p) => p.id));
  assert.deepEqual(order.slice(-3), ['oc-set', 'spm', 'spf']);
  assert.deepEqual(await page.evaluate(() => [...document.getElementById('spf').children].map((c) => c.id || c.className)), ['spf-head', 'pf', 'wf', 'fw', 'lf']);
  assert.equal(await page.textContent('#spf-title'), 'Friends');
  assert.equal(await page.$$eval('#spf h2', (h) => h.length), 1, 'one heading for the area');
  assert.match(await page.textContent('#pf'), /Invite a friend to your world/);
  assert.match(await page.textContent('#pf'), /Their character stays on your world/);
  // the friends list is on the list kit (listkit.js): its tabs keep the page's ids; a core without friends says so in it
  await page.waitForSelector('#lf-tab-all', { state: 'attached' });
  assert.deepEqual(await page.$$eval('#lf [role="tab"]', (b) => b.map((x) => x.id)), ['lf-tab-all', 'lf-tab-on', 'lf-tab-req', 'lf-tab-near', 'lf-tab-fav']);
  assert.equal(await page.$$eval('#lf-list.lk', (l) => l.length), 1);
  // the i button over the Friends area explains it, with both docs
  await page.$eval('#spf', (e) => e.scrollIntoView({ block: 'start' }));
  await page.click('#ll-info');
  await page.waitForSelector('#ll-info-b:not([hidden])');
  assert.equal(await page.textContent('#ll-info-t'), 'Friends');
  assert.equal(await page.getAttribute('#ll-info-b', 'data-section'), 'spf');
  assert.equal(await page.$$eval('#ll-info-b p', (l) => l.length), 4);
  assert.equal(await page.getAttribute('#ll-info-more', 'data-link'), 'hostfriends');
  assert.equal(await page.getAttribute('#ll-info-also', 'data-link'), 'docsingle');
  await noBrand(page);
  await shot(page, 'friends-area');
  await page.click('#ll-info-also');
  assert.deepEqual((await callsOf(page, 'open_link')).pop(), { name: 'docsingle' });
  // back at the top: the page's own bubble
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.$eval('#sp', (e) => e.scrollIntoView({ block: 'start' }));
  await page.click('#ll-info');
  await page.waitForSelector('#ll-info-b:not([hidden])');
  assert.equal(await page.textContent('#ll-info-t'), 'Single player');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test("settings that wouldn't apply aren't offered; a refusal is said once, not read again in a loop", async () => {
  const a = await open({ worlds: { single: WS({ supported: false }), vanilla: WS({ supported: false }), bossrush: WS({}) } });
  await a.page.evaluate(() => window.LeylineNav.show('single'));
  await a.page.waitForFunction(() => /for the Built-in engine, on Windows PCs/.test(document.getElementById('oc-set').textContent));
  assert.equal(await a.page.$$eval('#wss', (l) => l.length), 0);
  await noBrand(a.page);
  await a.ctx.close();
  const b = await open({ getErr: '[other] Single player has no world settings.' });
  await b.page.evaluate(() => window.LeylineNav.show('single'));
  await b.page.waitForFunction(() => /has no world settings/.test((document.getElementById('wss-status') || {}).textContent || ''));
  const n = (await callsOf(b.page, 'world_settings_get')).length;
  await b.page.waitForTimeout(800);
  assert.ok((await callsOf(b.page, 'world_settings_get')).length - n <= 1, 'no loop');
  assert.deepEqual(b.errors, []);
  await b.ctx.close();
});
