// Leyline Solo, the app (ui-solo/: docs/LEYLINE-SOLO.md), in Chromium with a fake Tauri core (the core's classic_*
// commands, the shared clients commands):
//   * first run: the game folder (a later-era folder said plainly; an incomplete one; nothing typed), then Set up with
//     one progress bar (the progress line of kind "classic") and the honest time estimate, then Home;
//   * Home: Play starts a stopped world, then plays (signed in by itself, Simple sign-in by default); Running/Stopped;
//     Stop; another world on the ports: "Stop it and play" (never stopped unasked);
//   * Quick start: races and classes only from classic_choices; a bad name is refused before anything starts;
//   * Characters: one row each (name, level, class, zone); Play enters that one;
//   * Invite a friend: only while it runs; one link and Copy;
//   * Settings: the folder, Simple/Quick sign-in, Show password (only when pressed) with Copy, the XP rate, roaming bots
//     (off by default), the logs folder, removing the sign-in, About with the legal line;
//   * a core without classic_* says so and breaks nothing; no brand words but the legal line; keyboard reachable.
// Screenshots go to $SOLO_APP_SHOTS when set (solo-*.png).
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/soloapp.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui-solo', 'index.html');
const SHOTS = process.env.SOLO_APP_SHOTS || '';
const FIRST = { id: 'c1121-1', name: 'Original', path: 'C:\\Games\\Original', build: '1.12.1', realmlist: '' };
const LATER = { id: 'c335a-1', name: 'Later', path: 'C:\\Games\\Later', build: '3.3.5a', realmlist: '' };
const DET = (build, o) => Object.assign({ build, build_label: build, complete: true, missing: [] }, o || {});
const RACES = [
  { id: 1, label: 'Human', classes: [[1, 'Warrior'], [2, 'Paladin'], [4, 'Rogue'], [5, 'Priest'], [8, 'Mage'], [9, 'Warlock']] },
  { id: 2, label: 'Orc', classes: [[1, 'Warrior'], [3, 'Hunter'], [4, 'Rogue'], [7, 'Shaman'], [9, 'Warlock']] },
  { id: 6, label: 'Tauren', classes: [[1, 'Warrior'], [3, 'Hunter'], [7, 'Shaman'], [11, 'Druid']] },
];
const CHARS = [
  { name: 'Tamsin', level: 14, race_label: 'Night Elf', class_label: 'Druid', zone: 'Darkshore' },
  { name: 'Brakka', level: 60, race_label: 'Orc', class_label: 'Warrior', zone: 'Orgrimmar' },
];
const ST = (o) => Object.assign({ supported: true, set_up: false, running: false, busy: false, xp_rate: 2, logs_dir: 'C:\\Users\\me\\AppData\\Roaming\\Leyline Solo\\classic\\logs' }, o || {});
const BANNED = [/world\s+of\s+warcraft/i, /warcraft/i, /lich\s+king/i, /blizzard/i, /\bWoW\b/, /\b1\.12(\.1)?\b|\b3\.3\.5a?\b/];

const FAKE = (o) => {
  o = Object.assign({ races: null }, o);
  window.__SOLO_POLL_MS = 20;
  const calls = (window.__calls = []);
  try { localStorage.clear(); } catch (_) { /* fine */ }
  window.__copied = [];
  try { Object.defineProperty(navigator, 'clipboard', { value: { writeText: async (t) => { window.__copied.push(t); } }, configurable: true }); } catch (_) { /* fine */ }
  const clients = JSON.parse(JSON.stringify(o.clients || []));
  window.__st = o.status === 'missing' ? null : JSON.parse(JSON.stringify(o.status));
  window.__left = 0;
  let playFails = o.playFails || 0;
  const job = (steps, then) => { window.__left = steps; window.__then = then; };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args === undefined ? null : JSON.parse(JSON.stringify(args))]);
    const missing = (c) => { throw `Command ${c} not found`; };
    if (o.status === 'missing' && cmd.startsWith('classic_')) missing(cmd);
    switch (cmd) {
      case 'get_state': return { version: '0.9.0-test', settings: { clients: JSON.parse(JSON.stringify(clients)), selected_client: clients[0] ? clients[0].id : null }, windows: true, data_dir: '/tmp' };
      case 'pick_folder': return o.pick || null;
      case 'detect_client': {
        const d = (o.detect || {})[args.path];
        if (!d) throw 'No game client found in that folder';
        return d;
      }
      case 'add_client': {
        const d = (o.detect || {})[args.path];
        const p = { id: 'c' + d.build.replace(/\W/g, '') + '-9', name: 'Game', path: args.path, build: d.build, realmlist: '' };
        clients.push(p);
        return { profile: p, detection: d };
      }
      case 'engine_progress': {
        if (window.__left > 0) {
          window.__left -= 1;
          return { idle: false, kind: 'classic', running: true, code: null, step: 'Reading the maps from your game folder', n: 3, of: 6, pct: 40, detail: 'vmaps: 1 of 3' };
        }
        if (window.__then) { Object.assign(window.__st, window.__then); window.__then = null; }
        return { idle: false, kind: 'classic', running: false, code: 0, step: 'Done', n: 6, of: 6 };
      }
      case 'classic_status': return JSON.parse(JSON.stringify(window.__st));
      case 'classic_setup': job(3, { set_up: true, running: true }); return 'Setting up';
      case 'classic_start': job(2, { running: true }); return 'Starting';
      case 'classic_stop': window.__st.running = false; return 'Stopped. Your characters are saved.';
      case 'classic_play':
        if (playFails > 0) { playFails -= 1; throw 'The game ports are in use: another world is already running on this PC.'; }
        return { auto: true, account: 'SOLO', made: args.quick ? args.quick.name : null };
      case 'classic_choices': return { races: o.races, name: 'Aldric' };
      case 'classic_characters': return o.chars || [];
      case 'classic_invite': return { link: 'https://relay.leylinelauncher.com/join/abcd1234abcd1234?k=xyz', ways: [{ kind: 'lan', line: 'Same network: direct' }, { kind: 'relay', line: 'Anywhere: through the Leyline relay' }] };
      case 'classic_password': return { account: 'SOLO', password: 'K7QH2M9X' };
      case 'classic_settings': return null;
      case 'classic_signin_off': return 'SIGNIN OFF: your game folder is as it was.';
      case 'classic_open_logs': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null };
      default: throw `Command ${cmd} not found`;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });
async function open(o) {
  o = Object.assign({ races: RACES }, o);
  const page = await browser.newPage({ viewport: { width: 980, height: 720 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, o);
  await page.goto(INDEX);
  await page.waitForFunction(() => document.querySelector('#so-main').children.length > 0);
  return { page, errors };
}
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, `solo-${name}.png`), fullPage: true }); };
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter((x) => x[0] === c).map((x) => x[1]), cmd);
const statusText = (page) => page.textContent('#so-status');
async function noBrand(page) {
  const t = (await page.evaluate(() => document.body.innerText)).replace(/not affiliated with or endorsed by Blizzard Entertainment/gi, '');
  for (const rx of BANNED) assert.doesNotMatch(t, rx);
}

test('first run: a later-era folder is said plainly, the first-era one goes on to Set up, one progress bar, then Home', async () => {
  const { page, errors } = await open({ clients: [], status: ST(), pick: 'D:\\Later', detect: { 'D:\\Later': DET('3.3.5a'), 'D:\\Original': DET('1.12.1') } });
  assert.equal(await page.isVisible('#so-nav'), false, 'no pages before it is set up');
  assert.match(await page.textContent('h2#so-folder-h'), /Point it at your game folder/);
  await shot(page, '1-first-run');
  await page.click('#so-folder-use');
  assert.match(await page.textContent('#so-folder-say'), /Choose your game folder first/);
  await page.click('#so-folder-pick');
  await page.waitForSelector('#so-folder-say');
  assert.match(await page.textContent('#so-folder-say'), /later version of the game.*first-era game only/);
  assert.equal((await calls(page, 'add_client')).length, 0, 'a later-era folder is never added');
  await shot(page, '2-wrong-era');
  await page.fill('#so-folder-path', 'D:\\Original');
  await page.press('#so-folder-path', 'Enter');
  await page.waitForSelector('#so-setup');
  assert.match(await page.textContent('#so-main'), /20 to 50 minutes/);
  assert.match(await page.textContent('#so-main'), /Game folder: D:\\Original/);
  await shot(page, '3-setup');
  await page.click('#so-setup');
  await page.waitForFunction(() => /Reading the maps/.test((document.querySelector('#so-prog') || {}).textContent || ''));
  assert.equal(await page.locator('[role="progressbar"]').count(), 1, 'one progress bar');
  assert.match(await page.textContent('#so-prog'), /Reading the maps from your game folder/);
  const now = Number(await page.getAttribute('#so-prog [role="progressbar"]', 'aria-valuenow'));
  assert.ok(now > 30 && now < 50, String(now));
  await shot(page, '4-setup-progress');
  await page.waitForSelector('#so-play');
  assert.deepEqual(await calls(page, 'classic_setup'), [{ client: 'c1121-9' }]);
  assert.equal(await page.isVisible('#so-nav'), true);
  assert.equal(await page.textContent('#so-run'), 'Running');
  assert.match(await statusText(page), /Set up\. Press Play/);
  await noBrand(page);
  assert.deepEqual(errors, []);
  await page.close();
});

test('an incomplete folder or no game at all: said, nothing added', async () => {
  const { page, errors } = await open({ clients: [], status: ST(), detect: { 'D:\\Half': DET('1.12.1', { complete: false, missing: ['Data\\terrain.MPQ'] }) } });
  await page.fill('#so-folder-path', 'D:\\Half');
  await page.click('#so-folder-use');
  await page.waitForSelector('#so-folder-say');
  assert.match(await page.textContent('#so-folder-say'), /looks incomplete \(missing Data\\terrain\.MPQ\)/);
  await page.fill('#so-folder-path', 'D:\\Nothing');
  await page.click('#so-folder-use');
  await page.waitForFunction(() => /isn't a game folder/.test(document.querySelector('#so-folder-say').textContent));
  assert.equal((await calls(page, 'add_client')).length, 0);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Home: Play starts a stopped world, then plays with Simple sign-in; Stop', async () => {
  const { page, errors } = await open({ clients: [FIRST], status: ST({ set_up: true }) });
  await page.waitForSelector('#so-play');
  assert.equal(await page.textContent('#so-run'), 'Stopped');
  assert.equal(await page.isDisabled('#so-stop'), true);
  await shot(page, '5-home');
  await page.click('#so-play');
  await page.waitForFunction(() => /Started/.test(document.querySelector('#so-status').textContent));
  assert.deepEqual(await calls(page, 'classic_start'), [{ roaming: false, stopOther: false }]);
  assert.deepEqual(await calls(page, 'classic_play'), [{ client: 'c1121-1', signin: 'simple' }]);
  assert.equal(await page.textContent('#so-run'), 'Running');
  await page.click('#so-stop');
  await page.waitForFunction(() => document.querySelector('#so-run').textContent === 'Stopped');
  assert.match(await statusText(page), /characters are saved/);
  assert.deepEqual(errors, []);
  await page.close();
});

test('another world on the ports: "Stop it and play", never stopped unasked', async () => {
  const { page, errors } = await open({ clients: [FIRST], status: ST({ set_up: true, running: true }), playFails: 1 });
  await page.click('#so-play');
  await page.waitForSelector('#so-other');
  assert.match(await page.textContent('#so-other'), /one world runs at a time/);
  assert.equal((await calls(page, 'classic_play')).length, 1);
  await shot(page, '6-ports-busy');
  await page.click('#so-other-go');
  await page.waitForFunction(() => /Started/.test(document.querySelector('#so-status').textContent));
  const plays = await calls(page, 'classic_play');
  assert.equal(plays.length, 2);
  assert.equal(plays[1].stopOther, true);
  assert.equal(await page.isVisible('#so-other'), false);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Quick start: first-era races and their classes from the core; a bad name is refused before anything starts', async () => {
  const { page, errors } = await open({ clients: [FIRST], status: ST({ set_up: true, running: true }) });
  await page.click('#so-quick-open');
  await page.waitForSelector('#so-race');
  assert.deepEqual(await page.$$eval('#so-race option', (o) => o.map((x) => x.textContent)), ['Human', 'Orc', 'Tauren']);
  assert.equal(await page.inputValue('#so-name'), 'Aldric', 'the core suggests a name');
  await page.selectOption('#so-race', '6');
  assert.deepEqual(await page.$$eval('#so-class option', (o) => o.map((x) => x.textContent)), ['Warrior', 'Hunter', 'Shaman', 'Druid']);
  await page.selectOption('#so-class', '11');
  await page.selectOption('#so-sex', '1');
  await page.fill('#so-name', 'a1');
  await page.click('#so-quick-go');
  assert.match(await statusText(page), /2 to 12 letters/);
  assert.equal(await page.getAttribute('#so-name', 'aria-invalid'), 'true');
  assert.equal((await calls(page, 'classic_play')).length, 0);
  await page.fill('#so-name', 'mooncalf');
  await shot(page, '7-quick-start');
  await page.click('#so-quick-go');
  await page.waitForFunction(() => /making Mooncalf/.test(document.querySelector('#so-status').textContent));
  assert.deepEqual((await calls(page, 'classic_play'))[0].quick, { name: 'Mooncalf', race: 6, class: 11, female: true });
  assert.deepEqual(errors, []);
  await page.close();
});

test('Characters: a row each (name, level, class, zone); Play enters that one; none: Quick start', async () => {
  const { page, errors } = await open({ clients: [FIRST], status: ST({ set_up: true, running: true }), chars: CHARS });
  await page.click('#so-nav-characters');
  await page.waitForSelector('#so-chars');
  assert.equal(await page.getAttribute('#so-nav-characters', 'aria-current'), 'page');
  const rows = await page.$$eval('#so-chars .so-char', (l) => l.map((x) => x.innerText.replace(/\s+/g, ' ').trim()));
  assert.match(rows[0], /^Tamsin Level 14 · Night Elf Druid · Darkshore Play$/);
  assert.match(rows[1], /^Brakka Level 60 · Orc Warrior · Orgrimmar Play$/);
  await shot(page, '8-characters');
  await page.click('button[aria-label="Play Brakka"]');
  await page.waitForFunction(() => /Started/.test(document.querySelector('#so-status').textContent));
  assert.equal((await calls(page, 'classic_play'))[0].pick, 'Brakka');
  await page.close();
  const b = await open({ clients: [FIRST], status: ST({ set_up: true }), chars: [] });
  await b.page.click('#so-nav-characters');
  await b.page.waitForSelector('#so-chars-quick');
  await b.page.click('#so-chars-quick');
  await b.page.waitForSelector('#so-quick');
  assert.deepEqual(errors.concat(b.errors), []);
  await b.page.close();
});

test('Invite a friend: only while it runs; one link and Copy', async () => {
  const { page, errors } = await open({ clients: [FIRST], status: ST({ set_up: true }) });
  await page.click('#so-nav-invite');
  assert.equal(await page.isDisabled('#so-invite'), true);
  assert.match(await page.textContent('#so-main'), /Start your world first/);
  await page.evaluate(() => { window.__st.running = true; });
  await page.click('#so-nav-home');
  await page.click('#so-play');
  await page.waitForFunction(() => /Started/.test(document.querySelector('#so-status').textContent));
  await page.click('#so-nav-invite');
  await page.click('#so-invite');
  await page.waitForSelector('#so-invite-link');
  assert.match(await page.inputValue('#so-invite-link'), /^https:\/\/relay\.leylinelauncher\.com\/join\//);
  assert.match(await page.textContent('#so-main'), /Same network: direct/);
  assert.match(await page.textContent('#so-main'), /their own first-era game folder and Leyline Solo \(or the Leyline Launcher\)/);
  await shot(page, '9-invite');
  await page.click('#so-invite-copy');
  assert.deepEqual(await page.evaluate(() => window.__copied), [await page.inputValue('#so-invite-link')]);
  assert.match(await statusText(page), /copied/);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Settings: folder, Simple sign-in by default, password only on Show, XP rate, roaming bots off, logs, sign-in off, About', async () => {
  const { page, errors } = await open({ clients: [FIRST], status: ST({ set_up: true, running: true }) });
  await page.click('#so-nav-settings');
  assert.equal(await page.inputValue('#so-folder-path'), 'C:\\Games\\Original');
  assert.equal(await page.isChecked('#so-signin-simple'), true);
  assert.equal(await page.isChecked('#so-signin-quick'), false);
  assert.equal(await page.isChecked('#so-roaming'), false);
  assert.equal(await page.textContent('#so-xp'), '2x');
  assert.equal(await page.locator('#so-password').count(), 0, 'no password before Show password');
  assert.equal((await calls(page, 'classic_password')).length, 0);
  assert.match(await page.textContent('#so-legal'), /not affiliated with or endorsed by Blizzard Entertainment/);
  assert.match(await page.textContent('#so-main'), /CMaNGOS Classic server \(GPL-2\.0\).*playerbots.*classic-db/);
  await shot(page, '10-settings');
  await page.click('#so-password-show');
  await page.waitForSelector('#so-password');
  assert.equal(await page.textContent('#so-password'), 'K7QH2M9X');
  await page.click('#so-password-copy');
  assert.deepEqual(await page.evaluate(() => window.__copied), ['K7QH2M9X']);
  await page.click('#so-password-hide');
  assert.equal(await page.locator('#so-password').count(), 0);
  await page.check('#so-signin-quick');
  await page.check('#so-roaming');
  assert.deepEqual((await calls(page, 'classic_settings')).pop(), { signin: 'quick', roaming: true });
  await page.click('#so-logs');
  assert.equal((await calls(page, 'classic_open_logs')).length, 1);
  await page.click('#so-signin-off');
  await page.waitForFunction(() => /as it was/.test(document.querySelector('#so-status').textContent));
  assert.deepEqual(await calls(page, 'classic_signin_off'), [{ client: 'c1121-1' }]);
  await page.click('#so-nav-home');
  await page.click('#so-play');
  await page.waitForFunction(() => /Started/.test(document.querySelector('#so-status').textContent));
  assert.equal((await calls(page, 'classic_play'))[0].signin, 'quick', 'the chosen sign-in goes with Play');
  await noBrand(page);
  assert.deepEqual(errors, []);
  await page.close();
});

test('a core without classic_*: said plainly, Set up off, nothing breaks; a later-era folder alone is not used', async () => {
  const { page, errors } = await open({ clients: [LATER, FIRST], status: 'missing' });
  await page.waitForSelector('#so-setup');
  assert.equal(await page.isDisabled('#so-setup'), true);
  assert.match(await page.textContent('#so-nocore'), /doesn't have its world yet/);
  assert.match(await page.textContent('#so-main'), /Game folder: C:\\Games\\Original/, 'the first-era folder, not the later one');
  await shot(page, '11-no-core');
  await page.close();
  const b = await open({ clients: [LATER], status: ST() });
  assert.match(await b.page.textContent('h2#so-folder-h'), /Point it at your game folder/, 'only a later-era folder: still asks for the folder');
  assert.deepEqual(errors.concat(b.errors), []);
  await b.page.close();
});

test('keyboard: the pages and Play are reachable with Tab, and the page heading takes focus on a page change', async () => {
  const { page, errors } = await open({ clients: [FIRST], status: ST({ set_up: true, running: true }) });
  await page.waitForSelector('#so-play');
  const seen = [];
  for (let i = 0; i < 12; i++) { await page.keyboard.press('Tab'); seen.push(await page.evaluate(() => document.activeElement.id || document.activeElement.textContent)); }
  for (const id of ['so-nav-home', 'so-nav-characters', 'so-nav-invite', 'so-nav-settings', 'so-play']) assert.ok(seen.includes(id), id + ' ' + seen.join(','));
  await page.focus('#so-nav-settings');
  await page.keyboard.press('Enter');
  assert.equal(await page.evaluate(() => document.activeElement.id), 'so-main');
  assert.equal(await page.getAttribute('#so-nav-settings', 'aria-current'), 'page');
  assert.deepEqual(errors, []);
  await page.close();
});

test('the shared art is the launcher\'s own bytes (fonts, the crystal), and the solo window loads nothing remote', () => {
  const ui = path.join(__dirname, '..', 'ui');
  const solo = path.join(__dirname, '..', 'ui-solo');
  for (const [a, b] of [['art/fonts/inter-latin-400-normal.woff2', 'art/fonts/inter-latin-400-normal.woff2'],
    ['art/fonts/inter-latin-600-normal.woff2', 'art/fonts/inter-latin-600-normal.woff2'], ['art/fonts/Inter-OFL.txt', 'art/fonts/Inter-OFL.txt'],
    ['art/brand/leyline-crystal-128.png', 'art/leyline-crystal-128.png']]) {
    assert.ok(fs.readFileSync(path.join(ui, a)).equals(fs.readFileSync(path.join(solo, b))), b);
  }
  for (const f of ['index.html', 'solo.js', 'solo.css']) assert.doesNotMatch(fs.readFileSync(path.join(solo, f), 'utf8'), /(src|href)=["']https?:|url\(["']?https?:/, f);
  const conf = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'src-tauri', 'tauri.solo.conf.json'), 'utf8'));
  assert.equal(conf.productName, 'Leyline Solo');
  assert.equal(conf.build.frontendDist, '../ui-solo');
  assert.notEqual(conf.identifier, JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'src-tauri', 'tauri.conf.json'), 'utf8')).identifier);
  assert.equal(conf.app.windows[0].label, 'main', 'the capabilities are for the window called main');
});
