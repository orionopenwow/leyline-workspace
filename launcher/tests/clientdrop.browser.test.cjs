// Adding a game client by dropping it (ui/app.js clientImport, core: src-tauri/src/clientimport.rs) and the launcher's
// own update line in the title bar (ui/app.js renderUpdate), in Chromium with a fake Tauri core.
//
// The drop: a link dropped on the client list (or the Find your game files page) asks where to unpack, then shows a
// progress bar (download %, unpacking %), Cancel stops it, and the new client is picked; a folder is added straight
// away; a magnet link gets the one plain sentence and nothing starts; the page never names a source.
// The update: hidden while nothing is ready; "Update ready — restart to finish" with Restart; Restart waits while a
// game is open; "Finishing the update…" with no button when the last run's update goes in by itself.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/clientdrop.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const TORRENT = "Leyline can't download game clients from torrents. Drop your own client folder or zip instead.";

const FAKE = ({ upd }) => {
  const calls = (window.__calls = []);
  try { localStorage.removeItem('leyline.page'); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  const st = { version: 'test', settings: { clients: [], selected_client: null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  // the import, driven by the test: window.__imp is what client_import_state answers
  window.__imp = { phase: 'idle', done: 0, total: 0, message: '' };
  window.__upd = upd || { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'select': st.settings.selected_client = args.client; return null;
      case 'realms_list': return { realms: [{ slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a',
        core: 'azerothcore', tags: [], players: 5, bots: 0, address: 'play.openvanilla.app' }], network: { realms: 0, players: 5, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return JSON.parse(JSON.stringify(window.__upd));
      case 'launcher_update_restart': if (window.__upd.game_running) throw 'Your game is still running. Close it first: Leyline restarts only when no game is open.'; return null;
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return false;
      case 'pick_folder': return null;
      case 'client_import_check': {
        const t = args.input.trim();
        if (/^magnet:|\.torrent$/i.test(t)) throw "Leyline can't download game clients from torrents. Drop your own client folder or zip instead.";
        if (t.startsWith('https://')) return { kind: 'link', url: t, name: 'files.example.com', unpacks: true, default_folder: '/home/me/.local/share/Leyline/Game clients' };
        if (t.endsWith('.zip')) return { kind: 'zip', path: t, name: 'Wrath.zip', unpacks: true, default_folder: '/home/me/.local/share/Leyline/Game clients' };
        return { kind: 'folder', path: t, name: 'Wrath', unpacks: false, default_folder: '/home/me/.local/share/Leyline/Game clients' };
      }
      case 'client_import_start': window.__imp = { phase: 'checking', done: 0, total: 0, message: '' }; return null;
      case 'client_import_state': return JSON.parse(JSON.stringify(window.__imp));
      case 'client_import_cancel': window.__imp = { phase: 'cancelled', done: 0, total: 0, message: 'Stopped. Nothing was added.' }; return null;
      case 'client_import_pick_folder': return '/mnt/big/Games';
      case 'client_import_pick_zip': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
  // a finished import adds the client to the state the UI reads next
  window.__finish = (name) => {
    st.settings.clients.push({ id: 'c335a-1', name, path: '/home/me/.local/share/Leyline/Game clients/my backup/Wrath', build: '3.3.5a', realmlist: '' });
    st.settings.selected_client = 'c335a-1';
    window.__imp = { phase: 'done', done: 0, total: 0, message: `Added ${name}.`, client_id: 'c335a-1', client_name: name };
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
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && document.querySelector('#page-play.on') && window.__calls.some(([c]) => c === 'realms_list'));
  return { page, ctx, errors };
}
// An HTML drop of text (a link dragged out of a browser) on an element.
async function dropText(page, sel, text) {
  await page.evaluate(([sel, text]) => {
    const n = document.querySelector(sel);
    const dt = new DataTransfer();
    dt.setData('text/uri-list', text);
    n.dispatchEvent(new DragEvent('dragover', { bubbles: true, cancelable: true, dataTransfer: dt }));
    n.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: dt }));
  }, [sel, text]);
}
const shots = process.env.SHOTS;
async function shot(page, name) { if (shots) await page.screenshot({ path: path.join(shots, name + '.png') }); }

test('a link dropped on the client list: where to unpack, a progress bar, then the client is added and picked', async () => {
  const { page, ctx, errors } = await open();
  await page.click('#pb-client');
  assert.ok(await page.isVisible('#pb-menu'));
  await dropText(page, '#pb-menu', 'https://files.example.com/my%20backup.zip');
  await page.waitForSelector('#cimp[data-state="ask"]');
  assert.equal(await page.isVisible('#pb-menu'), false, 'the list closes for the panel');
  assert.equal(await page.textContent('#cimp-what'), 'Download from files.example.com and unpack it.');
  assert.equal(await page.textContent('#cimp-dest'), '/home/me/.local/share/Leyline/Game clients');
  // pick another folder
  await page.click('#cimp-change');
  await page.waitForFunction(() => document.getElementById('cimp-dest').textContent === '/mnt/big/Games');
  await shot(page, 'clientdrop-ask');
  await page.click('#cimp-go');
  const start = await page.evaluate(() => window.__calls.find(([c]) => c === 'client_import_start')[1]);
  assert.deepEqual(start, { input: 'https://files.example.com/my%20backup.zip', folder: '/mnt/big/Games' });
  await page.waitForSelector('#cimp[data-state="run"]');
  // download 42%
  await page.evaluate(() => { window.__imp = { phase: 'downloading', done: 42, total: 100, message: '' }; });
  await page.waitForFunction(() => document.getElementById('cimp-pct').textContent === '42%');
  assert.equal(await page.textContent('#cimp-phase'), 'Downloading…');
  assert.equal(await page.getAttribute('#cimp-bar', 'aria-valuenow'), '42');
  assert.ok(await page.isVisible('#cimp-cancel'));
  await shot(page, 'clientdrop-downloading');
  // then unpacking 7%
  await page.evaluate(() => { window.__imp = { phase: 'unpacking', done: 7, total: 100, message: '' }; });
  await page.waitForFunction(() => document.getElementById('cimp-phase').textContent === 'Unpacking…');
  assert.equal(await page.textContent('#cimp-pct'), '7%');
  // then done: added, picked, the bar's client name
  await page.evaluate(() => window.__finish('my backup (Wrath 3.3.5a)'));
  await page.waitForSelector('#cimp[data-state="done"]');
  assert.equal(await page.textContent('#cimp-status'), 'Added my backup (Wrath 3.3.5a).');
  assert.equal(await page.textContent('#pb-client-name'), 'my backup (Wrath 3.3.5a)');
  assert.equal(await page.inputValue('#play-client'), 'c335a-1');
  await shot(page, 'clientdrop-done');
  await page.click('#cimp-ok');
  assert.equal(await page.isVisible('#cimp'), false);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Cancel stops the download and says nothing was added', async () => {
  const { page, ctx, errors } = await open();
  await page.click('#pb-client');
  await page.click('#pb-menu >> text=Add from a zip or a link…');
  await page.waitForSelector('#cimp[data-state="pick"]');
  assert.match(await page.textContent('#cimp'), /Drop your game folder or a zip of it here/);
  await shot(page, 'clientdrop-pick');
  await page.fill('#cimp-link', 'https://files.example.com/b.zip');
  await page.press('#cimp-link', 'Enter');
  await page.waitForSelector('#cimp[data-state="ask"]');
  await page.click('#cimp-go');
  await page.evaluate(() => { window.__imp = { phase: 'downloading', done: 0, total: 0, message: '' }; });
  await page.waitForFunction(() => document.getElementById('cimp-bar').classList.contains('unknown'), null, { timeout: 3000 });
  assert.equal(await page.textContent('#cimp-pct'), '', 'no percent while the size is not known');
  await page.click('#cimp-cancel');
  await page.waitForSelector('#cimp[data-state="done"]');
  assert.equal(await page.textContent('#cimp-status'), 'Stopped. Nothing was added.');
  assert.ok(await page.evaluate(() => window.__calls.some(([c]) => c === 'client_import_cancel')));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a magnet link is refused in one plain sentence, and nothing starts', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('findfiles'));
  await dropText(page, '#page-findfiles', 'magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567');
  await page.waitForSelector('#cimp[data-state="error"]');
  assert.equal(await page.textContent('#cimp-status'), TORRENT);
  assert.equal(await page.isVisible('#cimp-go'), false);
  assert.ok(!(await page.evaluate(() => window.__calls.some(([c]) => c === 'client_import_start'))), 'nothing started');
  await shot(page, 'clientdrop-magnet');
  // pasted into the guide's box, the same
  await page.click('#cimp-ok');
  await page.fill('#ff-link', 'https://files.example.com/client.torrent');
  await page.click('#ff-link-go');
  await page.waitForSelector('#cimp[data-state="error"]');
  assert.equal(await page.textContent('#cimp-status'), TORRENT);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a dropped folder is added straight away; the guide takes drops and names no source', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('findfiles'));
  const guide = await page.textContent('#page-findfiles');
  assert.match(guide, /Drop your game folder, or a zip of it, anywhere on this page\. Or paste a direct link to your own backup zip\./);
  assert.doesNotMatch(guide, /world of warcraft|\bwow\b|warcraft|lich king|blizzard|torrent|magnet|\d+\.\d+\.\d+/i);
  assert.deepEqual(await page.$$eval('#page-findfiles a, #page-findfiles [data-link]', (l) => l.map((x) => x.dataset.link || x.href)), ['discord', 'gettingstarted']);
  await shot(page, 'clientdrop-guide');
  await dropText(page, '#page-findfiles', '/home/me/Games/Wrath');
  await page.waitForSelector('#cimp[data-state="run"]');
  const start = await page.evaluate(() => window.__calls.find(([c]) => c === 'client_import_start')[1]);
  assert.deepEqual(start, { input: '/home/me/Games/Wrath', folder: null }, 'a folder is added where it is: no unpack folder');
  assert.equal(await page.isVisible('#cimp-ask'), false);
  await page.evaluate(() => window.__finish('Wrath (Wrath 3.3.5a)'));
  await page.waitForSelector('#cimp[data-state="done"]');
  assert.equal(await page.textContent('#pb-client-name'), 'Wrath (Wrath 3.3.5a)');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a failed import says why in plain words, never the raw error', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineClientImport.drop('/home/me/backup.zip'));
  await page.waitForSelector('#cimp[data-state="ask"]');
  assert.equal(await page.textContent('#cimp-what'), 'Unpack Wrath.zip.');
  await page.click('#cimp-go');
  await page.evaluate(() => { window.__imp = { phase: 'failed', message: 'os error 28: No space left on device (/tmp/x)' }; });
  await page.waitForSelector('#cimp[data-state="error"]');
  const t = await page.textContent('#cimp-status');
  assert.doesNotMatch(t, /os error|\/tmp/);
  await page.click('#cimp-ok');
  await page.evaluate(() => window.LeylineClientImport.drop('/home/me/backup.zip'));
  await page.waitForSelector('#cimp[data-state="ask"]');
  await page.click('#cimp-go');
  await page.evaluate(() => { window.__imp = { phase: 'failed', message: "It is unpacked, but no game client is in it. The right folder holds the game's program and a Data folder." }; });
  await page.waitForSelector('#cimp[data-state="error"]');
  assert.match(await page.textContent('#cimp-status'), /no game client is in it/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the update line: hidden until ready, then one calm line with Restart', async () => {
  const { page, ctx, errors } = await open();
  assert.equal(await page.isVisible('#tb-update'), false, 'nothing while there is nothing to do');
  await page.evaluate(() => { window.__upd = { mode: 'auto', downloading: true, ready: null, info: { current: '0.3.1', latest: '0.3.2', newer: true } }; });
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'launcher_update_state').length > 2);
  assert.equal(await page.isVisible('#tb-update'), false, 'a download in the background says nothing in the title bar');
  await page.evaluate(() => { window.__upd = { mode: 'auto', ready: '0.3.2', game_running: false, info: { current: '0.3.1', latest: '0.3.2', newer: true } }; });
  await page.waitForSelector('#tb-update:not([hidden])');
  assert.equal(await page.textContent('#tb-update-text'), 'Update ready — restart to finish');
  assert.equal(await page.textContent('#tb-update-go'), 'Restart');
  assert.equal(await page.isDisabled('#tb-update-go'), false);
  await shot(page, 'update-ready');
  await page.click('#tb-update-go');
  assert.ok(await page.evaluate(() => window.__calls.some(([c]) => c === 'launcher_update_restart')));
  // a game is open: Restart waits
  await page.evaluate(() => { window.__upd = { mode: 'auto', ready: '0.3.2', game_running: true, info: { current: '0.3.1', latest: '0.3.2', newer: true } }; });
  await page.waitForFunction(() => document.getElementById('tb-update-go').disabled);
  assert.equal(await page.textContent('#tb-update-text'), 'Update ready — restart after your game');
  assert.match(await page.getAttribute('#tb-update', 'title'), /finishes the next time you open Leyline/);
  // the last run's update going in by itself
  await page.evaluate(() => { window.__upd = { mode: 'auto', ready: '0.3.2', finishing: true, info: { current: '0.3.1', latest: '0.3.2', newer: true } }; });
  await page.waitForFunction(() => document.getElementById('tb-update-text').textContent === 'Finishing the update…');
  assert.equal(await page.isVisible('#tb-update-go'), false);
  // Settings says the same, in plain words
  await page.evaluate(() => window.LeylineNav.show('settings'));
  assert.match(await page.textContent('#upd-status'), /Finishing the update/);
  assert.match(await page.textContent('#page-settings'), /Press Restart when you like, or it finishes the next time you open Leyline\. It never restarts while your game is open\./);
  assert.deepEqual(errors, []);
  await ctx.close();
});
