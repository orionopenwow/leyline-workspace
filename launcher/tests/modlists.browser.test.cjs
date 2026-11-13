// The Mods page's lists on the list kit (listkit.js, docs/LAUNCHER-LISTS.md), in Chromium with a fake Tauri core:
// Discover's full list (app.js catListMake: search across name, author, description and tags with the match marked,
// the kit's chips Works with this game folder / Installed / Updates / Favourites, Sort with Name, the count, the
// Installed and Update marks on the cards, "/" and the arrow keys, Enter for Install), after an install the player stays
// on that tab with its card lit (the owner's UX finding: Installed used to open on the Mods tab), the stars shared with
// Installed (instListMake: the Mods | Addons tabs with their ids and badges, a search, the Updates chip, Enter for Update,
// the empty words with Find addons), Looks (looks.js: the kinds as tabs, a search) and Game modes (gamemods.js: groups).
// A card with nothing to install (a link) never gets an Install from the kit: Enter opens its page.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/modlists.browser.test.cjs    (SHOTS=<dir> saves lists-mods*.png)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const SHOTS = process.env.SHOTS || '';

const P = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: name + ' does things.', category: 'addons',
  builds: ['3.3.5a'], author: 'Someone', author_url: '', licence: 'MIT', repo: '', page: `https://leylinelauncher.com/wrath/mods/${id.slice(4)}`,
  credits: `${name} by Someone · MIT`, stars: null, last_push: '', version: '1.0.0', kind: 'ui', permissions: [], on_wrath: false, tags: [],
  download: `https://leylinelauncher.com/wrath/mods/${id.slice(4)}/download`, sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
const LOOK = (id, name, type, extra) => Object.assign({ id, source: 'look', name, description: name + ' changes the look.', category: 'graphics',
  builds: ['3.3.5a'], author: 'Leyline', page: '', repo: '', credits: `${name} by Leyline · MIT`, size: 0,
  look: { type, preset: type === 'preset' ? 'high' : '', uses: [], changes: ['Shadows'], order: 1, note: '', how: 'launcher' } }, extra || {});
const CATALOG = { categories: [{ id: 'addons', name: 'Addons and UI', line: '', icon: '' }, { id: 'sandbox', name: 'Sandbox and game modes', line: '', icon: '' },
  { id: 'tools', name: 'Tools', line: '', icon: '' }],
  projects: [
    P('mod:alpha', 'Alpha Bars', { author: 'Ann', tags: ['action-bars'] }),
    P('mod:bravo', 'Bravo Bags', { author: 'Bob', description: 'Sorts your inventory into one bag.' }),
    P('mod:charlie', 'Charlie Frames', { author: 'Cat' }),
    P('mod:old-bars', 'Old Bars', { builds: ['1.12'] }),
    P('mod:golfcart', 'Golf Cart', { category: 'sandbox', kind: 'both' }),
    P('link:0123456789ab', 'Map Viewer', { source: 'community', download: null, sha256: null, kind: null, category: 'tools', page: 'https://example.org/map-viewer' }),
    LOOK('look:high', 'High', 'preset'), LOOK('look:sky', 'Bright sky', 'graphics-pack'), LOOK('look:frames', 'Clean frames', 'ui-look', { builds: ['1.12'] }),
  ], from_cache: false, saved_at: 1, error: null };
const ORDER = ['mod:bravo', 'mod:alpha', 'mod:charlie', 'mod:golfcart', 'mod:old-bars', 'link:0123456789ab'];

const FAKE = ({ catalog, order, installedMods, tab, view }) => {
  const calls = (window.__calls = []);
  try {
    localStorage.setItem('leyline.page', 'discover');
    localStorage.setItem('leyline.dcView', view || 'all');
    localStorage.setItem('leyline.dcTab', tab || 'addons');
  } catch (_) { /* fine */ }
  let mods = installedMods || [];
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c1', name: 'My game', path: '/x', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c1', selected_realm: 'leylinelauncher.com', wine: { enabled: true, command: 'wine', prefix: '' }, count_installs: true },
        official_realm: 'leylinelauncher.com', browser_url: 'https://leylinelauncher.com/wrath', windows: false, data_dir: '/tmp' };
      case 'mods_catalog': return catalog;
      case 'mods_ranked': return { ids: order, metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c1', build: '3.3.5a', mods };
      case 'mod_install': {
        const p = catalog.projects.find((x) => x.id === args.project);
        mods = mods.filter((m) => m.key !== p.id).concat([{ key: p.id, name: p.name, version: p.version, source: 'catalog', update: null, in_catalog: true, enabled: true, files: [], addons: [p.name], patches: [] }]);
        return { key: p.id, name: p.name, version: p.version, warnings: [], kept: [] };
      }
      case 'mod_set_enabled': mods = mods.map((m) => (m.key === args.key ? Object.assign({}, m, { enabled: args.on }) : m)); return null;
      case 'looks_state': return { build: '3.3.5a', on: [], changed: false, how: {} };
      case 'deep_link_take': return null;
      case 'open_url': return null;
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
  const page = await browser.newPage({ viewport: viewport || { width: 1360, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route(/^https?:\/\//, (r) => r.abort());   // no network in a test
  await page.addInitScript(FAKE, Object.assign({ catalog: CATALOG, order: ORDER }, opts));
  await page.goto(INDEX);
  if (!opts.view || opts.view === 'all') await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length > 0);
  return { page, errors };
}
const names = (page, sel) => page.$$eval(`${sel || '#cat-grid'} .mod-card .name`, (n) => n.map((x) => x.textContent));
const calls = (page, name) => page.evaluate((n) => window.__calls.filter(([c]) => c === n).map(([, a]) => a), name);
const chip = (bar, id) => `${bar} .lk-chip[data-lk-filter="${id}"]`;
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, `lists-mods-${name}.png`) }); };
const BRAND = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;

test('Discover: one list: search everywhere, the chips, Sort with Name, the count, stars, keys, Enter installs and you stay on its card', async () => {
  const { page, errors } = await open({ installedMods: [
    { key: 'mod:charlie', name: 'Charlie Frames', version: '1.0.0', source: 'catalog', update: '1.1.0', in_catalog: true, enabled: true, files: [], addons: ['CharlieFrames'], patches: [] }] });
  assert.deepEqual(await names(page), ['Bravo Bags', 'Alpha Bars', 'Charlie Frames'], 'the hub order; Old Bars (1.12) waits behind Works with this game folder');
  assert.equal(await page.textContent('#dc-kbar .lk-count'), '3 of 4 addons');
  assert.deepEqual(await page.$$eval('#dc-kbar .lk-chip', (b) => b.map((x) => x.textContent)),
    ['Works with this game folder', 'Installed', 'Updates', 'Favourites', 'Any game', 'Vanilla', 'Wrath'], 'two games in the list: the Game chips');
  assert.equal(await page.getAttribute(chip('#dc-kbar', 'fit'), 'aria-pressed'), 'true');
  // the marks: Installed, or an update ready
  assert.equal(await page.textContent('#cat-grid .mod-card[data-id="mod:charlie"] .dc-have'), 'Update ready');
  assert.equal(await page.locator('#cat-grid .mod-card[data-id="mod:bravo"] .dc-have').count(), 0);
  await shot(page, 'discover');
  // search: the author, the description, a tag's words; the match is marked
  await page.fill('#cat-q', 'bob');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 1);
  assert.deepEqual(await names(page), ['Bravo Bags']);
  await page.fill('#cat-q', 'inventory');
  await page.waitForFunction(() => document.querySelector('#cat-grid .mod-card .desc mark.lk-hl'));
  assert.equal(await page.textContent('#cat-grid .mod-card .desc mark.lk-hl'), 'inventory');
  await page.fill('#cat-q', 'action bars');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 1);
  assert.deepEqual(await names(page), ['Alpha Bars'], 'every word, in any field (the tag "action-bars")');
  await page.fill('#cat-q', 'zzz');
  await page.waitForSelector('#cat-grid .lk-empty-nomatch');
  assert.match(await page.textContent('#cat-grid .lk-empty'), /No addons match “zzz”/);
  await shot(page, 'discover-nomatch');
  await page.click('#cat-grid .lk-empty .btn:text-is("Reset")');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 4);
  assert.equal(await page.getAttribute(chip('#dc-kbar', 'fit'), 'aria-pressed'), 'false', 'Reset clears the chips too');
  await page.click(chip('#dc-kbar', 'fit'));
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 3);
  // the chips: Installed, Updates
  await page.click(chip('#dc-kbar', 'have'));
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 1);
  await page.click(chip('#dc-kbar', 'have'));
  await page.click(chip('#dc-kbar', 'upd'));
  assert.deepEqual(await names(page), ['Charlie Frames']);
  await page.click(chip('#dc-kbar', 'upd'));
  // Sort: Name is the list's own A to Z; the hub's sorts still ask the core
  await page.selectOption('#cat-sort', 'name');
  await page.waitForFunction(() => document.querySelector('#cat-grid .mod-card .name').textContent === 'Alpha Bars');
  assert.deepEqual(await names(page), ['Alpha Bars', 'Bravo Bags', 'Charlie Frames']);
  assert.ok((await calls(page, 'mods_ranked')).every((a) => a.sort !== 'name'), 'the core never gets "name"');
  await page.selectOption('#cat-sort', 'new');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'mods_ranked' && a.sort === 'new'));
  await page.waitForFunction(() => document.querySelector('#cat-grid .mod-card .name').textContent === 'Bravo Bags');
  // a star: Favourites shows it; the store is the Mods page's one set (leyline.list.mods.fav)
  await page.click('#cat-grid .mod-card[data-id="mod:alpha"] .dc-star');
  assert.deepEqual(await page.evaluate(() => JSON.parse(localStorage.getItem('leyline.list.mods.fav'))), ['mod:alpha']);
  await page.click(chip('#dc-kbar', 'fav'));
  assert.deepEqual(await names(page), ['Alpha Bars']);
  await page.click(chip('#dc-kbar', 'fav'));
  // keys: "/" goes to the search; Down to the first card; Enter is its Install; the player stays here, the card lit
  await page.click('#dc-line');
  await page.keyboard.press('/');
  assert.equal(await page.evaluate(() => document.activeElement.id), 'cat-q');
  await page.keyboard.press('ArrowDown');
  assert.equal(await page.evaluate(() => document.activeElement.dataset.lkKey), 'mod:bravo');
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'mod_install'));
  assert.deepEqual(await calls(page, 'mod_install'), [{ client: 'c1', project: 'mod:bravo' }]);
  await page.waitForSelector('#cat-grid .mod-card.just[data-id="mod:bravo"]');
  assert.ok(await page.isVisible('#pane-discover'), 'still on Discover');
  assert.equal(await page.getAttribute('#pane-discover', 'data-shelf'), 'addons', 'still on Addons');
  assert.equal(await page.textContent('#cat-grid .mod-card[data-id="mod:bravo"] .dc-have'), 'Installed');
  assert.equal(await page.evaluate(() => document.activeElement.closest('[data-lk-key]').dataset.lkKey), 'mod:bravo', 'the focus stays on its card');
  await shot(page, 'discover-installed');
  // Installed opens on Addons, with that card lit and the star from Discover
  await page.click('#tab-mods-installed');
  await page.waitForSelector('#mods-list .card.just[data-key="mod:bravo"]');
  assert.equal(await page.getAttribute('#inst-tab-addons', 'aria-selected'), 'true');
  // one set of stars: a star on Installed is the same star on Discover
  await page.click('#mods-list .card[data-key="mod:bravo"] .inst-star');
  assert.deepEqual((await page.evaluate(() => JSON.parse(localStorage.getItem('leyline.list.mods.fav')))).sort(), ['mod:alpha', 'mod:bravo']);
  await page.click('#tab-mods-discover');
  await page.waitForSelector('#cat-grid .mod-card[data-id="mod:bravo"] .dc-star[aria-pressed="true"]');
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), BRAND);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Discover: Enter on a card with nothing to install opens its page; Mods has its category chips beside the kit\'s', async () => {
  const { page, errors } = await open({ tab: 'mods' });
  assert.deepEqual(await names(page), ['Golf Cart', 'Map Viewer']);
  assert.equal(await page.textContent('#dc-kbar .lk-count'), '2 mods');
  assert.ok(await page.locator('#dc-smart .chip-btn.lk-chip').count() >= 2, 'the categories wear the kit\'s chip');
  await page.focus('#cat-grid .mod-card[data-id="link:0123456789ab"]');
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'open_url'));
  assert.deepEqual(await calls(page, 'open_url'), [{ url: 'https://example.org/map-viewer' }]);
  assert.deepEqual(await calls(page, 'mod_install'), [], 'nothing installs from a card without an Install');
  assert.equal(await page.locator('#cat-grid .mod-card[data-id="link:0123456789ab"] .acts .btn:text-is("Install")').count(), 0);
  // a category chip and a search: the kit's no-match words with the way out
  await page.click('#dc-smart .chip-btn:text-is("Tools")');
  await page.fill('#cat-q', 'golf');
  await page.waitForSelector('#cat-grid .lk-empty');
  assert.match(await page.textContent('#cat-grid .lk-empty'), /No mods match “golf”/);
  await page.click('#cat-grid .lk-empty .btn:text-is("Reset")');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 2);
  assert.equal(await page.getAttribute('#dc-smart .chip-btn:text-is("All mods")', 'aria-pressed'), 'true', 'Reset clears the category too');
  assert.deepEqual(errors, []);
  await page.close();
});

test('Installed: the Mods | Addons tabs keep their ids and badges; search, Updates, Enter updates, the empty words', async () => {
  const MODS = [
    { key: 'mod:alpha', name: 'Alpha Bars', version: '1.0.0', source: 'catalog', update: '1.2.0', in_catalog: true, enabled: true, files: [], addons: ['AlphaBars'], patches: [] },
    { key: 'mod:charlie', name: 'Charlie Frames', version: '1.0.0', source: 'catalog', update: null, in_catalog: true, enabled: false, files: [], addons: ['CharlieFrames'], patches: [] },
    { key: 'local:sky', name: 'Sky patch', version: '', source: 'local', update: null, in_catalog: false, enabled: true, files: [], addons: [], patches: ['patch-4.MPQ'] },
  ];
  const { page, errors } = await open({ installedMods: MODS });
  await page.click('#tab-mods-installed');
  await page.waitForSelector('#inst-tab-addons');
  assert.deepEqual(await page.$$eval('#inst-tabs .ly-tab', (b) => b.map((x) => x.dataset.shelf)), ['mods', 'addons'], 'the old selector still works');
  assert.equal(await page.textContent('#inst-n-addons'), '2');
  assert.equal(await page.textContent('#inst-n-mods'), '1');
  await page.click('#inst-tabs .ly-tab[data-shelf="addons"]');
  await page.waitForFunction(() => document.querySelectorAll('#mods-list .card').length === 2);
  assert.equal(await page.textContent('#mods-list .lk-count'), '2 addons');
  assert.equal(await page.textContent('#inst-line'), "Addons are in-game interface helpers. They live in your game's AddOns folder.");
  await shot(page, 'installed');
  // search, and the chips
  await page.fill('#mods-list .lk-search input', 'charlie');
  await page.waitForFunction(() => document.querySelectorAll('#mods-list .card').length === 1);
  assert.equal(await page.textContent('#mods-list .card .name mark.lk-hl'), 'Charlie');
  await page.fill('#mods-list .lk-search input', '');
  await page.click(chip('#mods-list', 'off'));
  await page.waitForFunction(() => document.querySelectorAll('#mods-list .card').length === 1);
  await page.click(chip('#mods-list', 'off'));
  await page.click(chip('#mods-list', 'upd'));
  await page.waitForFunction(() => document.querySelectorAll('#mods-list .card').length === 1);
  // Enter on a card with an update: Update
  await page.focus('#mods-list .card[data-key="mod:alpha"]');
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'mod_install'));
  assert.deepEqual(await calls(page, 'mod_install'), [{ client: 'c1', project: 'mod:alpha' }]);
  await page.click(chip('#mods-list', 'upd'));
  // the Mods tab, then a tab with nothing in it: the words and Find mods
  await page.click('#inst-tab-mods');
  await page.waitForFunction(() => document.querySelectorAll('#mods-list .card').length === 1);
  assert.equal(await page.textContent('#mods-list .card .name'), 'Sky patch');
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.instShelf')), 'mods', 'the tab is remembered where it always was');
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), BRAND);
  assert.deepEqual(errors, []);
  await page.close();
  const two = await open({ installedMods: [] });
  await two.page.click('#tab-mods-installed');
  await two.page.waitForSelector('#mods-list .lk-empty');
  assert.match(await two.page.textContent('#mods-list .lk-empty'), /No (mods|addons) in this client yet/);
  assert.equal(await two.page.locator('#mods-list .lk-empty .btn').count(), 1, 'Find mods / Find addons');
  await shot(two.page, 'installed-empty');
  assert.deepEqual(two.errors, []);
  await two.page.close();
});

test('Looks: the kinds are the kit\'s tabs (old ids and data-type kept), a search, Works with this game folder; Game modes in groups', async () => {
  const { page, errors } = await open({ tab: 'looks', view: 'all' });
  await page.waitForSelector('#looks-grid .lk-card');
  assert.deepEqual(await page.$$eval('.lk-filter .ly-tab', (b) => b.map((x) => x.dataset.type)), ['', 'graphics-pack', 'ui-look', 'preset']);
  assert.equal(await page.textContent('#looks-list .lk-count'), '3 looks');
  await page.click('.lk-filter .ly-tab[data-type="preset"]');
  assert.deepEqual(await page.$$eval('#looks-grid .lk-card', (c) => c.map((x) => x.dataset.id)), ['look:high']);
  await page.click('.lk-filter .ly-tab[data-type=""]');
  await page.fill('#looks-list .lk-search input', 'sky');
  await page.waitForFunction(() => document.querySelectorAll('#looks-grid .lk-card').length === 1);
  await page.fill('#looks-list .lk-search input', '');
  await page.click(chip('#looks-list', 'fit'));
  await page.waitForFunction(() => document.querySelectorAll('#looks-grid .lk-card').length === 2);
  assert.ok(!(await page.$$eval('#looks-grid .lk-card', (c) => c.map((x) => x.dataset.id))).includes('look:frames'), 'a look for another game steps aside');
  await shot(page, 'looks');
  await page.click('#tab-mods-gamemodes');
  await page.waitForSelector('#gm-list .lk-group');
  assert.deepEqual(await page.$$eval('#gm-list .lk-group-l', (g) => g.map((x) => x.textContent)), ['Play now', 'Game mode mods']);
  assert.ok(await page.isVisible('#gm-section .gm-den button:has-text("Open Your Den")'));
  await page.fill('#gm-list .lk-search input', 'golf');
  await page.waitForFunction(() => document.querySelectorAll('#gm-list .gm-card').length === 1);
  await shot(page, 'gamemodes');
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), BRAND);
  assert.deepEqual(errors, []);
  await page.close();
});

test('the phone: the Mods page\'s list fits, one column, no sideways scroll', async () => {
  const { page, errors } = await open({}, { width: 390, height: 844 });
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
  assert.equal(overflow, false);
  await shot(page, 'phone');
  assert.deepEqual(errors, []);
  await page.close();
});
