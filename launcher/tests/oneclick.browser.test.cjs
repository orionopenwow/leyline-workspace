// Single player in one click (ui/oneclick.js, docs/SINGLE-PLAYER-ONE-CLICK.md), in Chromium with a fake Tauri core:
//   * one button: Home's "Play single player", Simple mode's Play and the play bar's Single player row share one flow;
//   * the folder: world_find_folder picked → silent; several → a one-tap chooser; none → the picker opens by itself and
//     the flow goes on once the folder is added;
//   * room: world_preflight not ok → its message, "Free some space, then Retry";
//   * the setup card: plain steps, a bar, the time left, Hide (a chip in the play bar), the game opened by itself when
//     done; an interrupted setup on launch (world_resume_pending): Continue, or on by itself after Play;
//   * errors by code: Retry, Repair (world_repair), Pick folder, Open log (world_logs, Copy), [ports], [antivirus];
//   * the first entry: Quick start pre-filled, Change, Skip; first + quick to world_play;
//   * after play: "Taking you in…", "Logged in with your Hero Key", the login with Copy;
//   * the setting "Stop the world when the game closes";
//   * an older core without the new commands: today's behaviour.
// Screenshots: ONECLICK_SHOTS=<dir> (default /mnt/user-data/outputs/gallery when it exists), oneclick-*.png.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/oneclick.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.ONECLICK_SHOTS || (fs.existsSync('/mnt/user-data/outputs') ? '/mnt/user-data/outputs/gallery' : '');
const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const OTHER = { id: 'c335a-2', name: 'Wrath (D drive)', path: 'D:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const WS = (o) => Object.assign({ supported: true, installed: false, set_up: false, running: false, backups: 0, other_running: null, account: '', password: '' }, o);
const RACES = [
  { id: 1, label: 'Human', side: 'alliance', classes: [[1, 'Warrior'], [2, 'Paladin'], [8, 'Mage']] },
  { id: 4, label: 'Night Elf', side: 'alliance', classes: [[1, 'Warrior'], [3, 'Hunter'], [11, 'Druid']] },
  { id: 6, label: 'Tauren', side: 'horde', classes: [[1, 'Warrior'], [7, 'Shaman'], [11, 'Druid']] },
];
const STEPS = [
  { step: 'Getting the engine', n: 1, of: 7, pct: 30 },
  { step: 'Getting the engine', n: 1, of: 7, pct: 90 },
  { step: 'Reading the maps from your game folder', n: 3, of: 7, pct: 20 },
  { step: 'Reading the maps from your game folder', n: 3, of: 7, pct: 70 },
  { step: 'Filling the database', n: 5, of: 7, pct: 50 },
  { step: 'Starting the world', n: 7, of: 7, pct: 10 },
];
const NEW = ['world_find_folder', 'world_preflight', 'world_resume_pending', 'world_repair', 'set_world_stop_on_exit'];

// o: { clients, worlds, find, preflight[], resume, chars, choices, play, setupErr, progressErr, missing[], storage, hold }
const FAKE = (o) => {
  const calls = (window.__calls = []);
  try {
    if (!sessionStorage.getItem('__booted')) {
      sessionStorage.setItem('__booted', '1');
      localStorage.clear();
      localStorage.setItem('leyline.mode', 'online');
      for (const [k, v] of Object.entries(o.storage || {})) localStorage.setItem(k, v);
    }
  } catch (_) { /* fine */ }
  window.__ws = o.worlds;
  window.__o = o;
  window.__seq = [];
  window.__eprog = { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7 };
  window.__hold = !!o.hold;        // keep the setup going until a test lets it finish
  const st = { version: 'test', settings: Object.assign({ clients: o.clients, selected_client: o.clients[0] ? o.clients[0].id : null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, o.settings || {}), official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  window.__st = st;
  const job = { single: 'engine', vanilla: 'vanilla', bossrush: 'bossrush' };
  const start = (kind) => {
    window.__seq = o.steps.map((s) => Object.assign({ idle: false, kind: job[kind], running: true, code: null, detail: '' }, s));
    window.__eprog = window.__seq[0];
    window.__kind = kind;
  };
  const missing = new Set(o.missing || []);
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    if (missing.has(cmd)) throw `Command ${cmd} not found`;
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
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
      case 'engine_status': return JSON.parse(JSON.stringify(window.__ws.single));
      case 'world_status': return JSON.parse(JSON.stringify(Object.assign({}, window.__ws[args.kind], { kind: args.kind })));
      // the one-click contract
      case 'world_find_folder': return JSON.parse(JSON.stringify(window.__o.find || { picked: null, found: [] }));
      case 'world_preflight': { const q = window.__o.preflight || []; return q.length > 1 ? q.shift() : (q[0] || { ok: true, free_gb: 80, need_gb: 6, message: '' }); }
      case 'world_resume_pending': return window.__o.resume || null;
      case 'world_repair': start(args.kind); window.__o.setupErr = null; window.__o.progressErr = null; return 'Repairing…';
      case 'world_setup': case 'world_start':
        if (window.__o.setupErr) throw window.__o.setupErr;
        start(args.kind);
        return 'Starting…';
      case 'engine_progress': {
        if (!window.__eprog.running) return JSON.parse(JSON.stringify(window.__eprog));
        if (window.__seq.length > 1) { window.__seq.shift(); window.__eprog = window.__seq[0]; return JSON.parse(JSON.stringify(window.__eprog)); }
        if (window.__hold) return JSON.parse(JSON.stringify(window.__eprog));
        if (window.__o.progressErr) { window.__eprog = Object.assign({}, window.__eprog, { running: false, code: 1, problem: window.__o.progressErr }); return JSON.parse(JSON.stringify(window.__eprog)); }
        window.__eprog = Object.assign({}, window.__eprog, { running: false, code: 0, step: 'Ready' });
        const k = window.__kind;
        window.__ws[k] = Object.assign({}, window.__ws[k], { set_up: true, running: true, other_running: null });
        return JSON.parse(JSON.stringify(window.__eprog));
      }
      case 'world_stop': case 'engine_stop': {
        const k = cmd === 'engine_stop' ? 'single' : args.kind;
        window.__ws[k] = Object.assign({}, window.__ws[k], { running: false });
        for (const x of Object.keys(window.__ws)) window.__ws[x].other_running = null;
        return 'Stopped. Everyone was saved.';
      }
      case 'world_logs': return '[auth] listening on 127.0.0.1:3724\n[world] reading maps… failed: map 0 missing';
      case 'local_characters': return JSON.parse(JSON.stringify(window.__o.chars || []));
      case 'world_choices': return { races: window.__o.races, first_era: false, name: 'Wren' };
      case 'world_play': return Object.assign({ message: 'Started.', kind: args.kind, auto: true, account: 'SOLO', password: '', made: args.quick ? (args.quick.name || 'Wren') : '', hero: false }, window.__o.play || {});
      case 'set_world_stop_on_exit': st.settings.world_stop_on_exit = args.on; return null;
      // the folder picker and the add path (app.js clientImport)
      case 'pick_folder': { const q = window.__o.picks || []; return q.length ? q.shift() : null; }
      case 'client_import_check': return { kind: 'folder', name: 'Wrath', unpacks: false, default_folder: '' };
      case 'client_import_start': st.settings.clients.push({ id: 'c335a-new', name: 'Wrath', path: args.input, build: '3.3.5a', realmlist: '' }); return null;
      case 'client_import_state': return { phase: 'done', message: 'Added.', client_id: 'c335a-new' };
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });
async function open(o) {
  const opts = Object.assign({ clients: [WRATH], worlds: { single: WS({}), vanilla: WS({}), bossrush: WS({}) }, races: RACES, steps: STEPS,
    chars: [{ kind: 'single', title: 'Single player', chars: [{ name: 'Odo' }] }, { kind: 'vanilla', title: 'Original world', chars: [{ name: 'Brakka' }] }],
    find: { picked: 'c335a-1', found: [Object.assign({ added: true }, WRATH)] } }, o);
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: 1360, height: 880 } });
  await ctx.grantPermissions(['clipboard-read', 'clipboard-write']);
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineOneClick && window.LeylineWorlds);
  await page.evaluate(() => { window.LeylineOneClick.cfg.pollMs = 150; window.LeylineOneClick.cfg.minRateMs = 250; });
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const view = (page) => page.evaluate(() => { const c = document.getElementById('oc'); return c.hidden ? '' : c.dataset.view; });
const waitView = (page, v) => page.waitForFunction((v) => { const c = document.getElementById('oc'); return !c.hidden && c.dataset.view === v; }, v, { timeout: 15000 });
const noBrand = async (page) => assert.doesNotMatch(await page.evaluate(() => document.body.innerText), /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard|3\.3\.5a? |1\.12/i);
async function shot(page, name) { if (!SHOTS) return; await page.waitForTimeout(350); await page.screenshot({ path: path.join(SHOTS, `oneclick-${name}.png`) }); }
async function homeGo(page) {
  await page.evaluate(() => window.LeylineNav.show('play'));
  await page.waitForSelector('#hm-oneclick:not([hidden]) #hm-oneclick-go');
  await page.click('#hm-oneclick-go');
}

test('Home: Play single player, folder picked silently, the setup card with plain steps and time left, then in with the Hero Key', async () => {
  const { page, ctx, errors } = await open({ hold: true, play: { hero: true } });
  await page.evaluate(() => window.LeylineNav.show('play'));
  await page.waitForSelector('#hm-oneclick:not([hidden])');
  assert.equal(await page.textContent('#hm-oneclick-go'), 'Play single player');
  await page.$eval('#hm-oneclick', (e) => e.scrollIntoView());
  await shot(page, 'home');
  await page.click('#hm-oneclick-go');
  await waitView(page, 'setup');
  assert.deepEqual(await callsOf(page, 'world_find_folder'), [{ kind: 'single' }]);
  assert.deepEqual(await callsOf(page, 'world_preflight'), [{ kind: 'single', client: 'c335a-1' }]);
  assert.deepEqual(await callsOf(page, 'world_setup'), [{ client: 'c335a-1', kind: 'single' }]);
  assert.deepEqual(await page.$$eval('#oc-steps li .oc-step-t', (l) => l.map((x) => x.textContent)), ['Getting the engine (one-time download)',
    'Reading the maps from your game folder (one time)', 'Filling the world: placing creatures, quests and items into its database (one time)',
    'Creating your local login', 'Starting your world']);
  // the steps move on, the time left is worked out from the rate
  await page.waitForFunction(() => document.querySelector('#oc-steps li[data-state="now"]') && document.querySelector('#oc-steps li[data-state="now"] .oc-step-t').textContent === 'Starting your world', null, { timeout: 15000 });
  await page.waitForFunction(() => /min left|Less than a minute left/.test(document.getElementById('oc-left').textContent), null, { timeout: 15000 });
  assert.ok(Number(await page.getAttribute('#oc-bar', 'aria-valuenow')) > 50);
  await shot(page, 'setup');
  assert.equal((await callsOf(page, 'world_play')).length, 0, 'not before it is ready');
  await page.evaluate(() => { window.__hold = false; });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_play'), [{ client: 'c335a-1', kind: 'single' }]);
  await waitView(page, 'done');
  assert.match(await page.textContent('#oc-done'), /Logged in with your Hero Key/);
  assert.match(await page.textContent('#hm-oneclick-status'), /Logged in with your Hero Key/);
  await shot(page, 'done-hero');
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.oneclick.pressed')), null, 'done: nothing to resume');
  await noBrand(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Hide: the setup goes on, a chip in the play bar brings it back, and the game opens by itself when done', async () => {
  const { page, ctx, errors } = await open({ hold: true });
  await homeGo(page);
  await waitView(page, 'setup');
  await page.click('#oc-hide');
  assert.equal(await view(page), '');
  await page.waitForSelector('#play-form #oc-chip:not([hidden])');
  assert.match(await page.textContent('#oc-chip'), /Single player: setting up · \d+%/);
  await shot(page, 'chip');
  await page.click('#oc-chip');
  await waitView(page, 'setup');
  await page.click('#oc-hide');
  await page.evaluate(() => { window.__hold = false; });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  await page.waitForFunction(() => document.getElementById('oc-chip').hidden);
  assert.equal(await view(page), '', 'hidden stays hidden: the game opened by itself');
  assert.match(await page.textContent('#hm-oneclick-status'), /Taking you in…/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('several game folders: a one-tap chooser (name and path); the tapped one is played', async () => {
  const find = { picked: null, found: [Object.assign({ added: true }, WRATH), Object.assign({ added: true }, OTHER)] };
  const { page, ctx, errors } = await open({ clients: [WRATH, OTHER], find, worlds: { single: WS({ set_up: true, installed: true }), vanilla: WS({}), bossrush: WS({}) } });
  await homeGo(page);
  await waitView(page, 'folders');
  assert.deepEqual(await page.$$eval('.oc-folder b', (l) => l.map((x) => x.textContent)), ['My Wrath', 'Wrath (D drive)']);
  assert.match(await page.textContent('.oc-folder[data-client="c335a-2"] .oc-path'), /D:\\Games\\Wrath/);
  await shot(page, 'folders');
  await page.click('.oc-folder[data-client="c335a-2"]');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_start'), [{ kind: 'single' }]);
  assert.equal((await callsOf(page, 'world_play'))[0].client, 'c335a-2');
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.single.folder')), 'c335a-2');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('no game folder: the picker opens by itself; cancelled, the card offers it again; once added the flow goes on', async () => {
  const { page, ctx, errors } = await open({ clients: [], find: { picked: null, found: [] }, picks: [null, 'E:\\Games\\Wrath'] });
  await homeGo(page);
  await waitView(page, 'folder');
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'pick_folder').length === 1);
  assert.match(await page.textContent('#oc'), /never downloads the game/);
  await shot(page, 'folder');
  await page.click('#oc-folder-pick');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_setup'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_setup'), [{ client: 'c335a-new', kind: 'single' }]);
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.equal((await callsOf(page, 'world_play'))[0].client, 'c335a-new');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('not enough room: the message and "Free some space, then Retry"; Retry goes on', async () => {
  const preflight = [{ ok: false, free_gb: 2.4, need_gb: 12, message: '[disk] Single player needs about 12 GB free on drive C: and 2.4 GB is free.' }, { ok: true, free_gb: 40, need_gb: 12, message: '' }];
  const { page, ctx, errors } = await open({ preflight });
  await homeGo(page);
  await waitView(page, 'disk');
  assert.equal(await page.textContent('#oc-disk'), 'Single player needs about 12 GB free on drive C: and 2.4 GB is free.', 'the sentence without its code');
  assert.match(await page.textContent('#oc'), /Free some space, then Retry/);
  assert.equal((await callsOf(page, 'world_setup')).length, 0);
  await shot(page, 'disk');
  await page.click('#oc-disk-retry');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.equal((await callsOf(page, 'world_preflight')).length, 2);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('first entry: Quick start pre-filled from world_choices; Change edits it; Play sends first + quick; Skip sends neither', async () => {
  const chars = [{ kind: 'single', title: 'Single player', chars: [] }];
  const { page, ctx, errors } = await open({ chars, worlds: { single: WS({ set_up: true, installed: true, running: true }), vanilla: WS({}), bossrush: WS({}) } });
  await homeGo(page);
  await waitView(page, 'quick');
  const who = await page.textContent('#oc-who');
  assert.match(who, /^Wren · (Human|Night Elf|Tauren) \w+ · (Male|Female)$/);
  await shot(page, 'quick');
  await page.click('#oc-quick-change');
  await page.selectOption('#oc-race', '4');
  assert.deepEqual(await page.$$eval('#oc-class option', (o) => o.map((x) => x.textContent)), ['Warrior', 'Hunter', 'Druid']);
  await page.selectOption('#oc-class', '11');
  await page.selectOption('#oc-sex', '1');
  await page.fill('#oc-name', 'Tamsin');
  await shot(page, 'quick-change');
  await page.click('#oc-quick-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_play'), [{ client: 'c335a-1', kind: 'single', quick: { name: 'Tamsin', race: 4, class: 11, female: true }, first: true }]);
  await page.waitForFunction(() => /Making Tamsin/.test(document.getElementById('hm-oneclick-status').textContent));
  // Skip: the game's own creation screen
  await page.waitForFunction(() => !window.LeylineOneClick.state().busy);
  await page.click('#hm-oneclick-go');
  await waitView(page, 'quick');
  await page.click('#oc-quick-skip');
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'world_play').length === 2, null, { timeout: 15000 });
  assert.deepEqual((await callsOf(page, 'world_play'))[1], { client: 'c335a-1', kind: 'single' });
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('errors by code: [maps] Retry, Repair (world_repair, then in), Open log with Copy', async () => {
  const { page, ctx, errors } = await open({ setupErr: '[maps] The maps couldn\'t be read from your game folder.' });
  await homeGo(page);
  await waitView(page, 'error');
  assert.equal(await page.textContent('#oc-err'), 'The maps couldn\'t be read from your game folder.', 'the sentence without its code');
  assert.deepEqual(await page.$$eval('#oc .oc-row button', (l) => l.map((x) => x.textContent)), ['Retry', 'Repair', 'Pick another folder', 'Open log']);
  await page.click('#oc-log-btn');
  await page.waitForFunction(() => /map 0 missing/.test((document.getElementById('oc-log') || {}).textContent || ''));
  assert.deepEqual(await callsOf(page, 'world_logs'), [{ kind: 'single', lines: 200 }]);
  await page.click('#oc-log-copy');
  await page.waitForFunction(() => document.getElementById('oc-flash').textContent === 'Log copied.');
  assert.match(await page.evaluate(() => navigator.clipboard.readText()), /map 0 missing/);
  await shot(page, 'error-log');
  await page.click('#oc-repair');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_repair'), [{ kind: 'single', client: 'c335a-1' }]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('errors by code: [folder] Pick folder, [antivirus] its hint, a progress failure, [ports] stops the other one', async () => {
  // [folder]: Pick folder, no Repair; the chooser, the tapped one played
  let { page, ctx, errors } = await open({ clients: [WRATH, OTHER], setupErr: '[folder] That folder doesn\'t hold the game.' });
  await homeGo(page);
  await waitView(page, 'error');
  assert.deepEqual(await page.$$eval('#oc .oc-row button', (l) => l.map((x) => x.textContent)), ['Retry', 'Pick folder', 'Open log']);
  await page.evaluate(() => { window.__o.setupErr = null; window.__o.find = { picked: 'c335a-1', found: [] }; });
  await page.click('#oc-pick');
  await waitView(page, 'folders');
  await page.click('.oc-folder[data-client="c335a-2"]');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.equal((await callsOf(page, 'world_setup')).pop().client, 'c335a-2');
  assert.deepEqual(errors, []);
  await ctx.close();
  // [antivirus] from the progress line
  ({ page, ctx, errors } = await open({ progressErr: '[antivirus] Something on this PC removed the engine\'s files.' }));
  await homeGo(page);
  await waitView(page, 'error');
  assert.match(await page.textContent('#oc'), /Allow Leyline's folder in it/);
  assert.deepEqual(await page.$$eval('#oc .oc-row button', (l) => l.map((x) => x.textContent)), ['Retry', 'Repair', 'Open log']);
  await shot(page, 'antivirus');
  await ctx.close();
  // [ports]: the other world stopped, then in
  ({ page, ctx, errors } = await open({ setupErr: '[ports] Another program is using the game\'s ports.', worlds: { single: WS({}), vanilla: WS({ set_up: true, running: true }), bossrush: WS({}) } }));
  await homeGo(page);
  await waitView(page, 'error');
  assert.deepEqual(await page.$$eval('#oc .oc-row button', (l) => l.map((x) => x.textContent)), ['Retry', 'Stop the other one and play', 'Open log']);
  assert.equal((await callsOf(page, 'world_stop')).length, 0, 'never stopped unasked');
  await page.evaluate(() => { window.__o.setupErr = null; });
  await page.click('#oc-ports');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_stop'), [{ kind: 'vanilla' }]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('an interrupted setup on launch: Continue; on by itself when Play had been pressed', async () => {
  let { page, ctx, errors } = await open({ resume: 'single' });
  await waitView(page, 'resume');
  assert.match(await page.textContent('#oc'), /Single player's setup was interrupted\./);
  assert.equal((await callsOf(page, 'world_setup')).length, 0, 'not without Continue');
  await shot(page, 'resume');
  await page.click('#oc-resume-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_setup'), [{ client: '', kind: 'single' }], 'goes on with the folder it started with');
  assert.equal((await callsOf(page, 'world_play'))[0].client, 'c335a-1');
  assert.deepEqual(errors, []);
  await ctx.close();
  ({ page, ctx, errors } = await open({ resume: 'single', storage: { 'leyline.oneclick.pressed': 'single' } }));
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_setup'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_setup'), [{ client: '', kind: 'single' }]);
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('after play: the login to type once with Copy buttons when the game can\'t sign in by itself', async () => {
  const { page, ctx, errors } = await open({ play: { auto: false, account: 'SOLO', password: 'ABCD2345' }, worlds: { single: WS({ set_up: true, installed: true, running: true }), vanilla: WS({}), bossrush: WS({}) } });
  await homeGo(page);
  await waitView(page, 'done');
  assert.match(await page.textContent('#oc-done'), /Log in once with SOLO, password ABCD2345\./);
  await page.click('#oc-copy-pw');
  await page.waitForFunction(() => /Password copied/.test(document.getElementById('oc-flash').textContent));
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), 'ABCD2345');
  await shot(page, 'login');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the setting: Stop the world when the game closes, on by default; a core without it keeps it on and says so', async () => {
  let { page, ctx, errors } = await open({});
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#oc-stop-exit');
  assert.equal(await page.isChecked('#oc-stop-exit'), true);
  assert.match(await page.textContent('#oc-set'), /Stop the world when the game closes/);
  await page.uncheck('#oc-stop-exit');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'set_world_stop_on_exit'));
  assert.deepEqual(await callsOf(page, 'set_world_stop_on_exit'), [{ on: false }]);
  await page.waitForFunction(() => /keeps running/.test(document.getElementById('oc-set-status').textContent));
  await page.$eval('#oc-set', (e) => e.scrollIntoView());
  await shot(page, 'setting');
  assert.deepEqual(errors, []);
  await ctx.close();
  ({ page, ctx, errors } = await open({ missing: ['set_world_stop_on_exit'] }));
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#oc-stop-exit');
  await page.click('#oc-stop-exit');   // it springs back: the core can't keep it
  await page.waitForFunction(() => /newer Leyline/.test(document.getElementById('oc-set-status').textContent));
  assert.equal(await page.isChecked('#oc-stop-exit'), true);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('one flow: Simple mode\'s Play and the play bar\'s Single player row go through it too', async () => {
  let { page, ctx, errors } = await open({ storage: { 'leyline.simple': '1', 'leyline.simple.daily': '0' }, worlds: { single: WS({ set_up: true, installed: true }), vanilla: WS({}), bossrush: WS({}) } });
  await page.waitForSelector('#page-simple.on #sh-play-go');
  assert.equal(await page.textContent('#sh-play-go'), 'Play single player');
  await shot(page, 'simple');
  await page.click('#sh-play-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_find_folder'), [{ kind: 'single' }]);
  await page.waitForFunction(() => /Taking you in/.test(document.getElementById('sh-status').textContent));
  assert.deepEqual(errors, []);
  await ctx.close();
  ({ page, ctx, errors } = await open({ worlds: { single: WS({ set_up: true, installed: true }), vanilla: WS({}), bossrush: WS({}) } }));
  await page.evaluate(() => window.LeylineNav.show('play'));
  await page.click('#pb-realm-btn');
  await page.waitForSelector('#pb-rmenu:not([hidden]) .pb-world-single');
  await page.click('#pb-rmenu .pb-world-single');
  await page.click('#play-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_find_folder'), [{ kind: 'single' }]);
  await page.waitForFunction(() => /Taking you in/.test(document.getElementById('play-status').textContent));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('an older core without the new commands: today\'s behaviour (the page\'s folder, no preflight, no sheet, no resume)', async () => {
  const { page, ctx, errors } = await open({ missing: NEW.concat(['local_characters', 'world_choices']) });
  await homeGo(page);
  await waitView(page, 'setup');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_setup'), [{ client: 'c335a-1', kind: 'single' }]);
  assert.deepEqual(await callsOf(page, 'world_play'), [{ client: 'c335a-1', kind: 'single' }]);
  assert.match(await page.textContent('#hm-oneclick-status'), /Taking you in…/);
  // an uncoded error: its sentence, Retry and Open log (no Repair to a core that hasn't got it)
  await page.evaluate(() => { window.__ws.single = Object.assign({}, window.__ws.single, { set_up: false, running: false }); window.__o.setupErr = 'The engine download stopped. Try again.'; });
  await page.waitForFunction(() => !window.LeylineOneClick.state().busy);
  await page.click('#hm-oneclick-go');
  await waitView(page, 'error');
  assert.equal(await page.textContent('#oc-err'), 'The engine download stopped. Try again.');
  assert.deepEqual(await page.$$eval('#oc .oc-row button', (l) => l.map((x) => x.textContent)), ['Retry', 'Open log']);
  await shot(page, 'error-old-core');
  await noBrand(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});
