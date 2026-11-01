// The play bar's realm box as a drop target (ui/app.js barImport; core: src-tauri/src/clientimport.rs and
// clienttorrent.rs), in Chromium with a fake Tauri core.
//
// Idle (nothing over the box), drag-over ("Drop to add your game"), Where should the game go? (the folder picker, at
// the last choice), the box as the progress bar ("Downloading · 1.2 GB / 5.0 GB · 14 MB/s · about 5 min left"), then
// Unpacking, Checking, and done: the client picked for every realm of its version, a realm of that version in the bar
// (the official one when it fits), "Ready — press PLAY", never a launch. An error is one line with Try again. With
// Settings › Testing off a magnet gets today's refusal; on, it starts like a zip.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/bardrop.browser.test.cjs
//   SHOTS=<dir> … also saves screenshots (dark and light)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const TORRENT = "Leyline can't download game clients from torrents. Drop your own client folder or zip instead.";
const DEFAULT = '/home/me/.local/share/Leyline/Game clients';

const FAKE = ({ clients, theme }) => {
  const calls = (window.__calls = []);
  try { localStorage.removeItem('leyline.page'); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  if (theme === 'light') localStorage.setItem('leyline.theme', JSON.stringify({ id: 'light', kind: 'light' }));
  const st = { version: 'test', settings: { clients: clients || [], selected_client: null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  window.__st = st;
  window.__imp = { phase: 'idle', done: 0, total: 0, message: '' };
  window.__testing = { client_torrents: false, share: false };
  window.__pick = null;   // the folder picker's answer (a promise the test resolves)
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'select': st.settings.selected_client = args.client; return null;
      case 'realms_list': return { realms: [
        { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 5, bots: 0, address: 'play.openvanilla.app' },
        { slug: 'old', name: 'Old Lands', official: false, state: 'live', build: '1.12.1', version: '1.12.1', core: 'vmangos', tags: [], players: 3, bots: 0, address: 'old.example.net' },
        { slug: 'frost', name: 'Frost Peak', official: false, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 2, bots: 0, address: 'frost.example.net' },
      ], network: { realms: 3, players: 10, bots: 0 }, from_cache: false, saved_at: 1, error: null };
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
      case 'pick_folder': return null;
      case 'testing_get': return { ...window.__testing };
      case 'testing_set': window.__testing = { client_torrents: args.clientTorrents, share: args.clientTorrents && args.share }; return { ...window.__testing };
      case 'client_import_check': {
        const t = args.input.trim();
        if (/^magnet:|\.torrent$/i.test(t)) {
          if (!window.__testing.client_torrents) throw TORRENT_MSG;
          return { kind: /^magnet:/i.test(t) ? 'magnet' : 'torrent', name: 'My Client', unpacks: true, default_folder: DEFAULT_DIR };
        }
        if (t.startsWith('https://')) return { kind: 'link', url: t, name: 'pub-1a2b.r2.dev', unpacks: true, default_folder: DEFAULT_DIR };
        if (t.endsWith('.zip')) return { kind: 'zip', path: t, name: 'Wrath.zip', unpacks: true, default_folder: DEFAULT_DIR };
        return { kind: 'folder', path: t, name: 'Wrath', unpacks: false, default_folder: DEFAULT_DIR };
      }
      case 'client_import_pick_dest': return window.__pick ? await window.__pick : DEFAULT_DIR;
      case 'client_import_start': window.__imp = { phase: 'downloading', done: 0, total: 0, message: '', line: 'Downloading · finding the download…' }; return null;
      case 'client_import_state': return JSON.parse(JSON.stringify(window.__imp));
      case 'client_import_cancel': window.__imp = { phase: 'cancelled', done: 0, total: 0, message: 'Stopped. Nothing was added.' }; return null;
      case 'play': throw 'the test never plays';
      default: throw 'unknown ' + cmd;
    }
  } } };
  const TORRENT_MSG = "Leyline can't download game clients from torrents. Drop your own client folder or zip instead.";
  const DEFAULT_DIR = '/home/me/.local/share/Leyline/Game clients';
  // the folder picker waits until the test answers
  window.__askPick = () => { window.__pick = new Promise((r) => { window.__answer = r; }); };
  // a finished import: the client is in the state the UI reads next
  window.__finish = (name, build, id) => {
    st.settings.clients.push({ id, name, path: '/mnt/big/Games/' + name, build, realmlist: '' });
    window.__imp = { phase: 'done', done: 0, total: 0, message: `Added ${name}.`, client_id: id, client_name: name };
  };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(opts = {}, scheme = 'dark') {
  const ctx = await browser.newContext({ colorScheme: scheme, viewport: { width: 1400, height: 860 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, { theme: scheme, ...opts });
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && document.querySelector('#page-play.on') && window.__calls.some(([c]) => c === 'realms_list'));
  await page.waitForFunction(() => document.querySelectorAll('#pb-realm').length && window.__calls.filter(([c]) => c === 'realms_list').length);
  return { page, ctx, errors };
}
async function drag(page, type, text) {
  await page.evaluate(([type, text]) => {
    const n = document.querySelector('#pb-realm-btn');
    const dt = new DataTransfer();
    if (text) dt.setData('text/uri-list', text);
    n.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer: dt, relatedTarget: type === 'dragleave' ? document.body : null }));
  }, [type, text]);
}
async function drop(page, text) { await drag(page, 'dragover', text); await drag(page, 'drop', text); }
const shots = process.env.SHOTS;
async function shot(page, name) {
  if (shots) await page.locator('#play-form').screenshot({ path: path.join(shots, name + '.png') });
}
const state = (page) => page.getAttribute('#pbd', 'data-state');
const clientFor = (page) => page.evaluate(() => JSON.parse(localStorage.getItem('leyline.clientFor') || '{}'));

for (const scheme of ['dark', 'light']) {
  test(`${scheme}: a link dropped on the realm box: drag-over, where it goes, the bar with speed, unpacking, then Ready`, async () => {
    const { page, ctx, errors } = await open({}, scheme);
    assert.equal(await page.isVisible('#pbd'), false, 'idle: the realm box as it is');
    assert.equal(await page.evaluate(() => document.documentElement.dataset.base), scheme);
    assert.match(await page.textContent('#play-headline'), /ADD YOUR GAME FOLDER/);
    await shot(page, `bar-${scheme}-idle`);
    // drag-over
    await drag(page, 'dragover', 'https://pub-1a2b.r2.dev/backup.zip');
    assert.equal(await state(page), 'over');
    assert.equal(await page.textContent('#pbd-head'), 'Drop to add your game');
    await shot(page, `bar-${scheme}-over`);
    await drag(page, 'dragleave');
    assert.equal(await page.isVisible('#pbd'), false, 'leaving puts it back');
    // drop: the folder picker first, at the last choice
    await page.evaluate(() => { localStorage.setItem('leyline.gameFolder', '/mnt/big/Games'); window.__askPick(); });
    await drop(page, 'https://pub-1a2b.r2.dev/backup.zip');
    await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'picking');
    assert.equal(await page.textContent('#pbd-head'), 'Where should the game go?');
    const ask = await page.evaluate(() => window.__calls.find(([c]) => c === 'client_import_pick_dest')[1]);
    assert.deepEqual(ask, { start: '/mnt/big/Games' });
    assert.ok(!(await page.evaluate(() => window.__calls.some(([c]) => c === 'client_import_start'))), 'nothing starts before a folder is picked');
    await shot(page, `bar-${scheme}-picking`);
    await page.evaluate(() => window.__answer('/mnt/games2'));
    await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'run');
    const start = await page.evaluate(() => window.__calls.find(([c]) => c === 'client_import_start')[1]);
    assert.deepEqual(start, { input: 'https://pub-1a2b.r2.dev/backup.zip', folder: '/mnt/games2' });
    assert.equal(await page.evaluate(() => localStorage.getItem('leyline.gameFolder')), '/mnt/games2', 'the choice is remembered');
    // downloading, with speed and time left
    await page.evaluate(() => { window.__imp = { phase: 'downloading', done: 1288490189, total: 5368709120, speed: 14680064, line: 'Downloading · 1.2 GB / 5.0 GB · 14 MB/s · about 5 min left', message: '' }; });
    await page.waitForFunction(() => document.getElementById('pbd-head').textContent.includes('14 MB/s'));
    assert.equal(await page.textContent('#pbd-head'), 'Downloading · 1.2 GB / 5.0 GB · 14 MB/s · about 5 min left');
    assert.equal(await page.textContent('#pbd-line'), 'pub-1a2b.r2.dev · 24%');
    assert.equal(await page.evaluate(() => document.getElementById('pbd-fill').style.width), '24%');
    assert.ok(await page.isVisible('#pbd-cancel'));
    await shot(page, `bar-${scheme}-downloading`);
    // unpacking, checking
    await page.evaluate(() => { window.__imp = { phase: 'unpacking', done: 2147483648, total: 4294967296, line: 'Unpacking · 2.0 GB / 4.0 GB · about 2 min left', message: '' }; });
    await page.waitForFunction(() => document.getElementById('pbd-head').textContent.startsWith('Unpacking'));
    assert.equal(await page.textContent('#pbd-line'), 'pub-1a2b.r2.dev · 50%');
    await shot(page, `bar-${scheme}-unpacking`);
    await page.evaluate(() => { window.__imp = { phase: 'checking', done: 0, total: 0, line: 'Checking your game folder…', message: '' }; });
    await page.waitForFunction(() => document.getElementById('pbd-head').textContent === 'Checking your game folder…');
    // done: a 3.3.5a client: the official realm kept, the client picked for every 3.3.5a realm
    await page.evaluate(() => window.__finish('backup (Wrath 3.3.5a)', '3.3.5a', 'c335a-1'));
    await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'done');
    assert.equal(await page.textContent('#pbd-head'), 'Ready — press PLAY');
    assert.equal(await page.textContent('#pbd-line'), 'backup (Wrath 3.3.5a) · Wrath Unleashed');
    assert.equal(await page.inputValue('#play-client'), 'c335a-1');
    assert.equal(await page.inputValue('#play-realm'), 'official');
    const cf = await clientFor(page);
    assert.deepEqual(cf.build, { '3.3.5a': 'c335a-1' });
    assert.equal(cf.realm.official, 'c335a-1');
    assert.equal(cf.realm['frost.example.net'], 'c335a-1');
    assert.equal(cf.realm.den, 'c335a-1');
    assert.equal(cf.realm.solo, 'c335a-1');
    assert.equal(cf.realm['old.example.net'], undefined, 'a realm of another version is left alone');
    assert.ok(!(await page.evaluate(() => window.__calls.some(([c]) => c === 'play'))), 'the game is never launched by itself');
    await shot(page, `bar-${scheme}-ready`);
    // a click puts the realm box back, which says it is ready
    await page.click('#pbd');
    assert.equal(await page.isVisible('#pbd'), false);
    assert.match(await page.textContent('#play-headline'), /READY TO PLAY/);
    assert.equal(await page.textContent('#pb-realm'), 'Wrath Unleashed');
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

test('another version: the first realm of that version is picked', async () => {
  const { page, ctx, errors } = await open();
  await drop(page, '/home/me/Games/Vanilla');
  await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'run');
  assert.ok(!(await page.evaluate(() => window.__calls.some(([c]) => c === 'client_import_pick_dest'))), 'a folder is added where it is: no picker');
  await page.evaluate(() => window.__finish('Vanilla (1.12.1)', '1.12.1', 'c1121-1'));
  await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'done');
  assert.equal(await page.textContent('#pbd-head'), 'Ready — press PLAY');
  assert.equal(await page.inputValue('#play-realm'), 'custom');
  assert.equal(await page.inputValue('#play-custom'), 'old.example.net');
  assert.equal(await page.inputValue('#play-client'), 'c1121-1');
  assert.equal(await page.textContent('#pbd-line'), 'Vanilla (1.12.1) · Old Lands');
  const cf = await clientFor(page);
  assert.equal(cf.realm['old.example.net'], 'c1121-1');
  assert.equal(cf.realm.official, undefined);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('an error is one line with Try again; Cancel stops; closing the picker starts nothing', async () => {
  const { page, ctx, errors } = await open();
  await drop(page, 'https://pub-1a2b.r2.dev/b.zip');
  await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'run');
  await page.evaluate(() => { window.__imp = { phase: 'failed', message: "That link didn't give a zip file. Use a direct link to the zip itself." }; });
  await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'error');
  assert.equal(await page.textContent('#pbd-head'), "That link didn't give a zip file. Use a direct link to the zip itself.");
  assert.ok(await page.isVisible('#pbd-retry'));
  assert.equal(await page.isVisible('#pbd-cancel'), false);
  await shot(page, 'bar-dark-error');
  // Try again: the same link, from the top (the picker again)
  const before = await page.evaluate(() => window.__calls.filter(([c]) => c === 'client_import_start').length);
  await page.click('#pbd-retry');
  await page.waitForFunction((n) => window.__calls.filter(([c]) => c === 'client_import_start').length === n + 1, before);
  await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'run');
  // Cancel
  await page.click('#pbd-cancel');
  await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'error');
  assert.equal(await page.textContent('#pbd-head'), 'Stopped. Nothing was added.');
  await page.click('#pbd-x');
  assert.equal(await page.isVisible('#pbd'), false);
  // the picker closed without a folder: back to idle, nothing started
  const n = await page.evaluate(() => window.__calls.filter(([c]) => c === 'client_import_start').length);
  await page.evaluate(() => { window.__askPick(); });
  await drop(page, '/home/me/backup.zip');
  await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'picking');
  await page.evaluate(() => window.__answer(null));
  await page.waitForFunction(() => document.getElementById('pbd').hidden);
  assert.equal(await page.evaluate(() => window.__calls.filter(([c]) => c === 'client_import_start').length), n);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Testing switch: off, a magnet gets the refusal; on (Settings › Testing), it starts; paste works on the focused box', async () => {
  const { page, ctx, errors } = await open();
  const mag = 'magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=My%20Client';
  await drop(page, mag);
  await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'error');
  assert.equal(await page.textContent('#pbd-head'), TORRENT);
  assert.ok(!(await page.evaluate(() => window.__calls.some(([c]) => c === 'client_import_pick_dest' || c === 'client_import_start'))), 'nothing asked, nothing started');
  await shot(page, 'bar-dark-refused');
  await page.click('#pbd-x');
  // Settings › Testing: off by default, share needs the first switch
  await page.evaluate(() => window.LeylineNav.show('settings'));
  const settings = await page.textContent('#page-settings');
  assert.match(settings, /Allow torrent downloads of game clients \(testing only\)/);
  assert.equal(await page.isChecked('#set-ctorrent'), false);
  assert.equal(await page.isDisabled('#set-cshare'), true);
  assert.doesNotMatch(settings, /world of warcraft|\bwow\b|warcraft|lich king|blizzard|magnet:\?/i);
  await page.check('#set-ctorrent');
  await page.waitForFunction(() => window.__testing.client_torrents === true);
  assert.equal(await page.isDisabled('#set-cshare'), false);
  assert.equal(await page.isChecked('#set-cshare'), false, 'sharing stays off');
  if (shots) {
    await page.locator('#set-ctorrent').scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(shots, 'settings-dark-testing.png') });
  }
  await page.evaluate(() => window.LeylineNav.show('play'));
  // pasted with the realm box focused
  await page.focus('#pb-realm-btn');
  await page.evaluate((t) => {
    const e = new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: new DataTransfer() });
    e.clipboardData.setData('text', t);
    document.activeElement.dispatchEvent(e);
  }, mag);
  await page.waitForFunction(() => document.getElementById('pbd').dataset.state === 'run');
  const start = await page.evaluate(() => window.__calls.find(([c]) => c === 'client_import_start')[1]);
  assert.deepEqual(start, { input: mag, folder: '/home/me/.local/share/Leyline/Game clients' });
  assert.equal(await page.textContent('#pbd-head'), 'Downloading · finding the download…');
  assert.equal(await page.isVisible('#cimp'), false, 'the client list panel stays shut');
  assert.deepEqual(errors, []);
  await ctx.close();
});
