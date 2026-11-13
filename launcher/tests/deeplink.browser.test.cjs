// leyline:// links in the launcher (ui/app.js handleDeepLink) in Chromium with a fake Tauri core: a realm link or a
// friend's invite shows at once even while the mods list is still loading (only an install link waits for it), and a
// link that reaches the already open launcher (the core's `deep-link` event, app.rs deep_link_arrived: the
// single-instance and deep-link plugins) is taken and shown without a second launcher.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/deeplink.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');

const FAKE = ({ link, catDelay }) => {
  const calls = (window.__calls = []);
  try { localStorage.setItem('leyline.page', 'play'); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  const listeners = (window.__listeners = {});
  window.__fire = (name) => (listeners[name] || []).forEach((f) => f({ event: name, payload: null }));
  const P = { id: 'mod:alpha', source: 'packaged', name: 'Alpha', description: 'Alpha does things.', category: 'addons', builds: ['3.3.5a'],
    author: 'Someone', author_url: '', licence: 'MIT', repo: '', page: '', credits: 'Alpha by Someone · MIT', stars: null, last_push: '',
    version: '1.0.0', kind: 'ui', permissions: [], on_wrath: false, download: 'https://leylinelauncher.com/wrath/mods/alpha/download',
    sha256: 'ab'.repeat(32), size: 1000 };
  window.__TAURI__ = {
    event: { listen: async (name, f) => { (listeners[name] = listeners[name] || []).push(f); return () => {}; } },
    core: { invoke: async (cmd, args) => {
      calls.push([cmd, args]);
      switch (cmd) {
        case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c335a-1', name: 'My Wrath', path: '/x', build: '3.3.5a', realmlist: '' }],
          selected_client: 'c335a-1', selected_realm: 'leylinelauncher.com', wine: { enabled: false, command: 'wine', prefix: '' } },
          official_realm: 'leylinelauncher.com', browser_url: 'https://leylinelauncher.com/wrath', windows: true, data_dir: '/tmp' };
        case 'mods_catalog':
          await new Promise((r) => setTimeout(r, window.__catDelay));
          return { categories: [{ id: 'addons', name: 'Addons', line: '', icon: '' }], projects: [P], from_cache: false, saved_at: 1, error: null };
        case 'mods_ranked': return { ids: ['mod:alpha'], metrics: {}, from_cache: false, error: null };
        case 'mods_installed': return { client: 'c335a-1', build: '3.3.5a', mods: [] };
        case 'mods_updates': return [];
        case 'deep_link_take': { const l = window.__link; window.__link = null; return l || null; }
        case 'realms_list': return { realms: [{ slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a',
          core: 'azerothcore', tags: [], players: 5, bots: 0, address: 'leylinelauncher.com' }], network: { realms: 1, players: 5, bots: 0 }, from_cache: false, saved_at: 1, error: null };
        case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
        case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
        case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
        case 'themes_list': return { active: 'dark', themes: [] };
        default: throw 'unknown ' + cmd;
      }
    } },
  };
  window.__link = link || null;
  window.__catDelay = catDelay || 0;
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts) {
  const page = await browser.newPage();
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  return page;
}
const onPage = (page) => page.evaluate(() => (document.querySelector('.page.on') || {}).id);

test('a realm link shows at once while the mods list is still loading', async () => {
  const page = await open({ link: { action: 'realm', id: 'official' }, catDelay: 20000 });
  await page.waitForFunction(() => /From the website: Wrath Unleashed/.test(document.getElementById('rl-status').textContent), null, { timeout: 4000 });
  assert.equal(await onPage(page), 'page-realms');
  assert.equal(await page.evaluate(() => document.querySelectorAll('#cat-grid .mod-card').length), 0, 'the mods list has not come yet');
  await page.close();
});

test('an install link waits for the mods list, then shows the project', async () => {
  const page = await open({ link: { action: 'install', id: 'mod:alpha' }, catDelay: 600 });
  await page.waitForFunction(() => /From the hub: Alpha/.test(document.getElementById('cat-status').textContent), null, { timeout: 6000 });
  assert.equal(await page.evaluate(() => window.__calls.filter(([c]) => c === 'mod_install').length), 0, 'nothing installs by itself');
  await page.close();
});

test('a link that reaches the open launcher is taken and shown (no second launcher)', async () => {
  const page = await open({});
  await page.waitForFunction(() => window.__listeners['deep-link'] && window.__listeners['deep-link'].length === 1);
  await page.waitForFunction(() => document.querySelector('.page.on'));
  const before = await page.evaluate(() => window.__calls.filter(([c]) => c === 'deep_link_take').length);
  assert.equal(before, 1, 'taken once at start');
  await page.evaluate(() => { window.__link = { action: 'realm', id: 'official' }; window.__fire('deep-link'); });
  await page.waitForFunction(() => /From the website: Wrath Unleashed/.test(document.getElementById('rl-status').textContent), null, { timeout: 4000 });
  assert.equal(await onPage(page), 'page-realms');
  // the same event again with nothing waiting: nothing changes
  await page.evaluate(() => { window.__fire('deep-link'); });
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__calls.filter(([c]) => c === 'deep_link_take').length), 3);
  await page.close();
});
