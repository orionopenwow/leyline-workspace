// Mods & Addons › Discover made simple (ui/simple.js) in Chromium with a fake Tauri core: the Simple view by default on
// both tabs (the line that says what is on the tab, one search box, short rows of big cards: Mods has Popular and one
// row per category, Addons has Popular, Make it look nicer, Quality of life), typing a search opens the full list with
// the words in its one search box, Browse everything / See all / ‹ Simple, each tab's own chips (Mods: its categories;
// Addons: the marketplace's kinds), and Sort and Only for under More filters. Screenshots go to $SIMPLE_SHOTS when set.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/simple.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const SHOTS = process.env.SIMPLE_SHOTS || '';
const P = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: `${name} does a thing.`, category: 'addons',
  builds: ['3.3.5a'], author: 'Leyline', author_url: '', licence: 'MIT', repo: '', page: '', credits: name, stars: null, last_push: '',
  version: '0.1.0', kind: 'ui', permissions: [], on_wrath: true, download: 'https://play.openvanilla.app/wrath/mods/x/download',
  sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
const U = (slug, name, tags) => P('up:' + slug, name, { source: 'upstream', tags, download: null, sha256: null, on_wrath: null,
  repo: `https://github.com/a/${slug}`, page: `https://github.com/a/${slug}`, addons: [name], requires: [], conflicts: [], replaces_ui: false,
  install: { from: 'commit-archive', url: 'https://github.com/a/x/archive/' + '0'.repeat(40) + '.zip', rev: '0'.repeat(40) },
  compat: { realm: 'Wrath Unleashed', status: 'untested', checked_at: '' } });
const CATALOG = { categories: [{ id: 'addons', name: 'Addons and UI' }, { id: 'sandbox', name: 'Sandbox and game modes' }, { id: 'tools', name: 'Tools' }],
  projects: [P('mod:pings', 'Pings'), P('mod:dice', 'Dalaran Dice', { category: 'sandbox', kind: 'both' }), P('mod:hud', 'Hud', { category: 'tools', kind: 'realm' }),
    U('frames', 'Frames', ['unit-frames']), U('bags', 'Bagger', ['bags']), U('plates', 'Plates', ['nameplates']), U('meter', 'Meter', ['meters'])],
  from_cache: false, saved_at: 1, error: null };

const FAKE = ({ catalog, view, tab }) => {
  const calls = (window.__calls = []);
  try {
    localStorage.setItem('leyline.page', 'discover');
    if (view) localStorage.setItem('leyline.dcView', view); else localStorage.removeItem('leyline.dcView');
    if (tab) localStorage.setItem('leyline.dcTab', tab); else localStorage.removeItem('leyline.dcTab');
    localStorage.removeItem('leyline.dcMore'); localStorage.removeItem('leyline.mkSub'); localStorage.removeItem('leyline.mkWorks');
  } catch (_) { /* fine */ }
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c1', name: 'My Wrath', path: '/x', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c1', selected_realm: 'play.openvanilla.app', wine: { enabled: true, command: 'wine', prefix: '' }, count_installs: true },
        official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: false, data_dir: '/tmp' };
      case 'mods_catalog': return catalog;
      case 'mods_ranked': throw 'offline';
      case 'mods_installed': return { client: 'c1', build: '3.3.5a', mods: [] };
      case 'deep_link_take': return null;
      case 'mods_updates': return [];
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'host_info': return { dir: '', is_kit: false, problems: [], kit: null, seeds: [], realm: null };
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(opts = {}, viewport) {
  const page = await browser.newPage({ viewport: viewport || { width: 1280, height: 860 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, Object.assign({ catalog: CATALOG }, opts));
  await page.goto(INDEX);
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length > 0);
  return { page, errors };
}
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, name + '.png') }); };
const rowNames = (page, key) => page.$$eval(`#dc-rows .dc-srow[data-row="${key}"] .mod-card .name`, (n) => n.map((x) => x.textContent));

const names = (page, sel) => page.$$eval(sel || '#cat-grid .mod-card .name', (n) => n.map((x) => x.textContent));
const heads = (page) => page.$$eval('#dc-rows .dc-srow h3', (h) => h.map((x) => x.textContent));
const tabTo = async (page, t) => {
  await page.click(`#discover-tabs .ly-tab[data-tab="${t}"]`);
  await page.waitForFunction((x) => document.getElementById('pane-discover').dataset.shelf === x, t);
};

test('the Simple view first, on Mods and on Addons: the line, one search box, rows of big cards, the full toolbar hidden', async () => {
  const { page, errors } = await open();
  await page.waitForSelector('#dc-rows .dc-srow');
  await shot(page, 'launcher-simple-mods');
  assert.equal(await page.getAttribute('#pane-discover', 'data-view'), 'simple');
  assert.equal(await page.getAttribute('#pane-discover', 'data-shelf'), 'mods', 'Mods first');
  assert.match(await page.textContent('#dc-line'), /^Mods change the game itself/);
  assert.equal(await page.getAttribute('#dc-sq', 'placeholder'), 'Search mods');
  assert.equal(await page.textContent('#dc-list-mod'), 'List your mod');
  assert.deepEqual(await heads(page), ['Popular', 'Game modes', 'Tools'], 'Mods: Popular, then its categories with something in them');
  assert.deepEqual(await rowNames(page, 'game'), ['Dalaran Dice']);
  assert.deepEqual(await rowNames(page, 'tools'), ['Hud']);
  assert.deepEqual((await rowNames(page, 'popular')).sort(), ['Dalaran Dice', 'Hud'], 'no addon on Mods');
  await tabTo(page, 'addons');
  await shot(page, 'launcher-simple-addons');
  assert.match(await page.textContent('#dc-line'), /^Addons are in-game interface helpers/);
  assert.equal(await page.getAttribute('#dc-sq', 'placeholder'), 'Search addons');
  assert.equal(await page.textContent('#dc-list-mod'), 'List your addon');
  assert.deepEqual(await heads(page), ['Popular', 'Make it look nicer', 'Quality of life']);
  assert.deepEqual((await rowNames(page, 'looks')).sort(), ['Frames', 'Plates']);
  assert.deepEqual((await rowNames(page, 'qol')).sort(), ['Bagger', 'Pings']);
  const all = await names(page, '#dc-rows .mod-card .name');
  assert.ok(!all.includes('Dalaran Dice') && !all.includes('Hud'), 'no mod on Addons');
  assert.equal(await page.isVisible('#dc-sq'), true);
  for (const id of ['#cat-q', '#cat-sort', '#cat-grid', '#dc-smart']) assert.equal(await page.isVisible(id), false, id + ' hidden');
  assert.equal(await page.locator('#pane-discover input[type="search"]:visible').count(), 1, 'one search box');
  assert.deepEqual(errors, []);
  await page.close();
});

test('typing a search opens the full list with the words; ‹ Simple goes back', async () => {
  const { page, errors } = await open({ tab: 'addons' });
  await page.type('#dc-sq', 'bag');
  await page.waitForFunction(() => document.getElementById('pane-discover').dataset.view === 'all');
  assert.equal(await page.inputValue('#cat-q'), 'bag');
  await page.type('#cat-q', 'ger');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 1);
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.dcView')), 'all', 'remembered');
  await page.click('#dc-simple-btn');
  assert.equal(await page.getAttribute('#pane-discover', 'data-view'), 'simple');
  assert.equal(await page.inputValue('#cat-q'), '', 'the search is cleared');
  assert.deepEqual(errors, []);
  await page.close();
});

test('full view: Mods has its categories as chips, Addons the marketplace kinds; Sort waits under More filters', async () => {
  const { page, errors } = await open();
  await page.click('#dc-rows .dc-srow[data-row="game"] .dc-srow-h .btn');
  await page.waitForFunction(() => document.getElementById('pane-discover').dataset.view === 'all');
  assert.equal(await page.getAttribute('#pane-discover', 'data-tab'), 'mods', 'Game modes is a category of Mods');
  assert.deepEqual(await page.$$eval('#dc-smart .chip-btn', (b) => b.map((x) => x.textContent)), ['All mods', 'Game modes', 'Tools']);
  assert.equal(await page.getAttribute('#dc-smart .chip-btn:text-is("Game modes")', 'aria-pressed'), 'true');
  assert.deepEqual(await names(page), ['Dalaran Dice']);
  await shot(page, 'launcher-mods-game-modes');
  await page.click('#dc-smart .chip-btn:text-is("All mods")');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 2);
  assert.equal(await page.isVisible('#cat-sort'), false, 'Sort is under More filters');
  await tabTo(page, 'addons');
  assert.deepEqual(await page.$$eval('#dc-smart .chip-btn', (b) => b.map((x) => x.textContent)),
    ['All addons', 'Unit frames', 'Nameplates', 'Meters', 'Bags', 'Works on Wrath Unleashed']);
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 5);
  await page.click('#dc-smart .chip-btn:text-is("Bags")');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 1);
  assert.deepEqual(await names(page), ['Bagger']);
  await page.click('#dc-more summary');
  await shot(page, 'launcher-more-filters');
  assert.equal(await page.isVisible('#cat-sort'), true);
  assert.equal(await page.locator('#cat-cat, #mk-bar').count(), 0, 'no Show menu, no second row of kinds');
  await page.click('#dc-smart .chip-btn:text-is("All addons")');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 5);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Browse everything from the Simple view; phone width keeps one column', async () => {
  const { page, errors } = await open({ tab: 'addons' }, { width: 390, height: 844 });
  await page.waitForSelector('#dc-rows .dc-srow');
  await shot(page, 'launcher-simple-390');
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
  assert.equal(overflow, false, 'no sideways scroll');
  await page.click('#dc-all');
  await page.waitForFunction(() => document.getElementById('pane-discover').dataset.view === 'all');
  assert.equal(await page.locator('#cat-grid .mod-card').count(), 5, 'every addon');
  assert.deepEqual(errors, []);
  await page.close();
});
