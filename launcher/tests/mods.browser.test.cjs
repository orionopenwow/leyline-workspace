// The launcher's Mods & Addons › Discover tab (ui/app.js) with the hub rankings, in Chromium with a fake Tauri core: the
// Sort menu (Hot, Rising, Top rated, Most installed, This month, New, Updated) asking the core for the order, the Mods and
// Addons tabs (owner 31 Oct 19:22: distinct; shelf.js) with the Mods tab's categories as chips (only those with something
// in them) and nothing addon-kind on Mods or mod-kind on Addons, the picture cards (the catalog's picture, the Workshop screenshot,
// the drawn tile when there is none or it fails), each card's rating and installs, INSTALL in one click (mod_install
// into the picked client) with INSTALLED ✓ and UPDATE states, a click on the card opening its page, rating stars (mod_rate, and the signed-out note), the remembered sort, a leyline://install/<id>
// link opening Mods › Discover on that project without installing anything, the Count my installs setting, offline
// fallback, and catalog text shown as text.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/mods.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');

const P = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: name + ' does things.', category: 'addons',
  builds: ['3.3.5a'], author: 'Someone', author_url: '', licence: 'MIT', repo: '', page: '', credits: `${name} by Someone · MIT`,
  stars: null, last_push: '', version: '1.0.0', kind: 'ui', permissions: [], on_wrath: false,
  download: `https://play.openvanilla.app/wrath/mods/${id.slice(4)}/download`, sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
const CATALOG = { categories: [{ id: 'addons', name: 'Addons and UI', line: '', icon: '' }, { id: 'sandbox', name: 'Sandbox and game modes', line: '', icon: '' },
  { id: 'tools', name: 'Tools', line: '', icon: '' }],
  projects: [
    P('mod:alpha', 'Alpha'), P('mod:bravo', 'Bravo'), P('mod:golfcart', 'Golf Cart', { category: 'sandbox', kind: 'both' }),
    P('mod:old-bars', 'Old Bars', { builds: ['1.12'] }),
    P('link:0123456789ab', '<img src=x onerror="window.__pwned=1">', { source: 'community', download: null, sha256: null, kind: null, category: 'tools' }),
  ], from_cache: false, saved_at: 1, error: null };
const M = (id, kind, all, d30, avg, n, hot, rising, nw, upd) => ({ id, kind, installs: { all, d30, d7: 0 },
  rating: { avg, n, score: n ? avg - 0.2 : null }, hot, rising, new: nw, updated: upd });
const METRICS = {
  'mod:alpha': M('mod:alpha', 'addon', 500, 40, 4.0, 30, 1.2, null, '2026-09-01', '2026-10-01'),
  'mod:bravo': M('mod:bravo', 'addon', 50, 45, 5.0, 1, 2.5, 1.8, '2026-10-25', '2026-10-28'),
  'mod:golfcart': M('mod:golfcart', 'mode', 90, 90, null, 0, 1.9, 0.4, '2026-10-20', ''),
};
// The order the core would give (rankings.rs rank(); its own unit tests check the formulas).
const ORDER = { hot: ['mod:bravo', 'mod:golfcart', 'mod:alpha', 'mod:old-bars', 'link:0123456789ab'],
  rising: ['mod:bravo', 'mod:golfcart', 'mod:alpha', 'mod:old-bars', 'link:0123456789ab'],
  top: ['mod:alpha', 'mod:bravo', 'mod:golfcart', 'mod:old-bars', 'link:0123456789ab'],
  installs: ['mod:alpha', 'mod:golfcart', 'mod:bravo', 'mod:old-bars', 'link:0123456789ab'],
  installs30: ['mod:golfcart', 'mod:bravo', 'mod:alpha', 'mod:old-bars', 'link:0123456789ab'],
  new: ['mod:bravo', 'mod:golfcart', 'mod:alpha', 'mod:old-bars', 'link:0123456789ab'],
  updated: ['mod:bravo', 'mod:alpha', 'mod:golfcart', 'mod:old-bars', 'link:0123456789ab'] };

const FAKE = ({ catalog, metrics, order, link, signedOut, noRanks, installedMods, sort, tab }) => {
  const calls = (window.__calls = []);
  try {
    localStorage.setItem('leyline.page', 'discover');
    localStorage.setItem('leyline.dcView', 'all'); localStorage.setItem('leyline.dcMore', '1');   // the full catalog, More filters open (simple.js)
    localStorage.setItem('leyline.dcTab', tab || 'addons');   // Discover's tab: Mods | Addons | Themes (themes.js)
    if (sort) localStorage.setItem('leyline.catSort', sort); else localStorage.removeItem('leyline.catSort');
  } catch (_) { /* fine */ }
  let mods = installedMods || [];
  const kindOf = (id) => (metrics[id] ? metrics[id].kind : (catalog.projects.find((p) => p.id === id).category === 'tools' ? 'mod' : 'addon'));
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [
          { id: 'c335a-1', name: 'My Wrath', path: '/x', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c335a-1', selected_realm: 'play.openvanilla.app', wine: { enabled: true, command: 'wine', prefix: '' },
        count_installs: true },
        official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: false, data_dir: '/tmp' };
      case 'mods_catalog': return catalog;
      case 'mods_ranked': {
        if (noRanks) throw 'offline';
        const ids = order[args.sort].filter((id) => !args.kind || kindOf(id) === args.kind);
        return { ids, metrics, from_cache: false, error: null };
      }
      case 'mods_installed': return { client: 'c335a-1', build: '3.3.5a', mods };
      case 'mod_install': {
        const p = catalog.projects.find((x) => x.id === args.project);
        mods = mods.filter((m) => m.key !== p.id).concat([{ key: p.id, name: p.name, version: p.version, source: 'catalog', update: null, in_catalog: true, enabled: true, files: [], addons: [p.name], patches: [] }]);
        return { key: p.id, name: p.name, version: p.version, warnings: [], kept: [] };
      }
      case 'mod_rate': if (signedOut) throw 'signed_out'; return { ok: true, mine: args.stars, rating: { avg: args.stars, n: 1, score: 3 } };
      case 'deep_link_take': { const l = window.__link; window.__link = null; return l || null; }
      case 'set_count_installs': return null;
      case 'open_url': return null;
      case 'mods_updates': return [];
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'host_info': return { dir: '', is_kit: false, problems: [], kit: null, seeds: [], realm: null };
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      default: throw 'unknown ' + cmd;
    }
  } } };
  window.__link = link || null;
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts = {}) {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, Object.assign({ catalog: CATALOG, metrics: METRICS, order: ORDER }, opts));
  await page.goto(INDEX);
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length > 0);
  return { page, errors };
}
const names = (page) => page.$$eval('#cat-grid .mod-card .name', (n) => n.map((x) => x.textContent));
const calls = (page, name) => page.evaluate((n) => window.__calls.filter(([c]) => c === n).map(([, a]) => a), name);
const card = (page, id) => page.locator(`#cat-grid .mod-card[data-id="${id}"]`);
const tabTo = async (page, t) => {
  await page.click(`#discover-tabs .ly-tab[data-tab="${t}"]`);
  await page.waitForFunction((x) => document.getElementById('pane-discover').dataset.shelf === x, t);
};
// the shelf (shelf.js) of every card on the grid, by the catalog's own entry
const shelves = (page) => page.$$eval('#cat-grid .mod-card', (c) => c.map((x) => window.LeylineShelf.of(catalog.projects.find((p) => p.id === x.dataset.id))));

test('Sort menu: every sort asks the core and shows its order', async () => {
  const { page, errors } = await open();
  assert.ok(await page.isVisible('#pane-discover'), "the old page id 'discover' opens Mods › Discover");
  assert.equal(await page.getAttribute('#tab-mods-discover', 'aria-selected'), 'true');
  const tabs = await page.$$eval('#cat-sort option', (b) => b.map((x) => x.textContent));
  assert.deepEqual(tabs, ['Hot', 'Rising', 'Top rated', 'Most installed', 'This month', 'New', 'Updated']);
  assert.equal(await page.inputValue('#cat-sort'), 'hot', 'Hot by default');
  assert.equal(await page.locator('#cat-chips, #cat-sorts, #cat-kind').count(), 0, 'no chips, no separate Kind menu');
  // "Only mods for this client" is on: Old Bars (1.12) is hidden from the 3.3.5a client; Addons shows the addons only
  assert.deepEqual(await names(page), ['Bravo', 'Alpha']);
  for (const [sort, first] of [['top', 'Alpha'], ['installs', 'Alpha'], ['installs30', 'Bravo'], ['rising', 'Bravo'], ['updated', 'Bravo']]) {
    const before = (await calls(page, 'mods_ranked')).length;
    await page.selectOption('#cat-sort', sort);
    await page.waitForFunction((n) => window.__calls.filter(([c]) => c === 'mods_ranked').length > n, before);
    await page.waitForFunction(([f]) => document.querySelector('#cat-grid .mod-card .name').textContent === f, [first]);
    assert.equal((await names(page))[0], first, sort);
  }
  const sorts = (await calls(page, 'mods_ranked')).map((a) => a.sort);
  assert.deepEqual(sorts.slice(-5), ['top', 'installs', 'installs30', 'rising', 'updated']);
  assert.equal((await calls(page, 'mods_ranked'))[0].refresh, true, 'the first load fetches mods.json');
  assert.equal((await calls(page, 'mods_ranked')).slice(-1)[0].refresh, false, 'a tab change uses the cached copy');
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.catSort')), 'updated', 'the sort is remembered');
  await tabTo(page, 'mods');
  assert.deepEqual(await names(page), ['Golf Cart', '<img src=x onerror="window.__pwned=1">'], 'Mods: the mods, in the same sort');
  assert.ok((await calls(page, 'mods_ranked')).every((a) => a.kind === ''), 'no kind filter: the tabs are the kinds');
  assert.deepEqual(errors, []);
  await page.close();
});

test('Mods and Addons tabs: Mods has its categories as chips; never an addon on Mods, never a mod on Addons; numbers on the cards', async () => {
  const { page, errors } = await open({ sort: 'top', tab: 'mods' });
  assert.equal(await page.inputValue('#cat-sort'), 'top', 'the remembered sort');
  assert.deepEqual(await page.$$eval('#discover-tabs .ly-tab', (b) => b.map((x) => x.textContent)), ['Mods', 'Addons', 'Themes', 'Graphics']);
  assert.equal(await page.getAttribute('#cat-q', 'placeholder'), 'Search mods');
  assert.match(await page.textContent('#dc-line'), /^Mods change the game itself/);
  assert.equal(await page.locator('#cat-cat').count(), 0, 'no Show menu: the chips are the categories');
  // only the categories with something for this client
  assert.deepEqual(await page.$$eval('#dc-smart .chip-btn', (b) => b.map((x) => x.textContent)), ['All mods', 'Game modes', 'Tools']);
  assert.deepEqual((await shelves(page)).filter((x) => x !== 'mods'), [], 'nothing addon-kind on Mods');
  await page.click('#dc-smart .chip-btn:has-text("Game modes")');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 1);
  assert.deepEqual(await names(page), ['Golf Cart']);
  await page.click('#dc-smart .chip-btn:has-text("Tools")');
  await page.waitForFunction(() => document.querySelector('#cat-grid .mod-card .name').textContent !== 'Golf Cart');
  assert.deepEqual(await names(page), ['<img src=x onerror="window.__pwned=1">']);
  await page.click('#dc-smart .chip-btn:has-text("All mods")');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 2);
  assert.match(await card(page, 'mod:golfcart').locator('.dc-meta').textContent(), /90/);
  assert.equal(await card(page, 'mod:golfcart').locator('.rating').count(), 0, 'no ratings yet');
  assert.equal(await card(page, 'mod:golfcart').locator('.rising').count(), 1);
  await tabTo(page, 'addons');
  assert.equal(await page.getAttribute('#cat-q', 'placeholder'), 'Search addons');
  assert.match(await page.textContent('#dc-line'), /^Addons are in-game interface helpers/);
  assert.equal(await page.textContent('#dc-list-mod'), 'List your addon');
  assert.deepEqual(await names(page), ['Alpha', 'Bravo']);
  assert.deepEqual((await shelves(page)).filter((x) => x !== 'addons'), [], 'no mod on Addons');
  assert.deepEqual(await page.$$eval('#dc-smart .chip-btn', (b) => b.map((x) => x.textContent)), ['All addons'], 'the marketplace kinds (none here)');
  assert.match(await card(page, 'mod:alpha').locator('.dc-meta').textContent(), /v1\.0\.0.*★ 4\.0.*500/);
  assert.equal(await card(page, 'mod:alpha').locator('.rating').getAttribute('title'), '30 ratings');
  assert.equal(await card(page, 'mod:alpha').locator('.installs').getAttribute('title'), '500 installs');
  assert.equal(await card(page, 'mod:alpha').locator('.rising').count(), 0, 'not rising');
  assert.equal(await card(page, 'mod:alpha').locator('.dc-tag').textContent(), 'Addons and UI', 'one small tag: the category');
  assert.equal(await card(page, 'mod:alpha').locator('.desc').textContent(), 'Alpha does things.');
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.deepEqual(errors, []);
  await page.close();
});

test('INSTALL in one click into the picked client, then INSTALLED ✓ and UPDATE', async () => {
  const upd = [{ key: 'mod:bravo', name: 'Bravo', version: '0.9.0', source: 'catalog', update: '1.0.0', in_catalog: true, enabled: true, files: [], addons: ['Bravo'], patches: [] }];
  const { page, errors } = await open({ installedMods: upd });
  const btn = card(page, 'mod:alpha').locator('.acts .btn');
  assert.equal(await btn.textContent(), 'Install');
  assert.equal(await card(page, 'mod:bravo').locator('.acts .btn').textContent(), 'Update');
  assert.equal(await card(page, 'mod:bravo').locator('.acts .btn').getAttribute('title'), 'Update to 1.0.0');
  await btn.click();
  await page.waitForFunction(() => /Installed Alpha v1\.0\.0 in My Wrath/.test(document.getElementById('cat-status').textContent));
  assert.deepEqual(await calls(page, 'mod_install'), [{ client: 'c335a-1', project: 'mod:alpha' }]);
  await page.waitForFunction(() => document.querySelector('#cat-grid .mod-card[data-id="mod:alpha"] .acts .btn').textContent === 'Installed ✓');
  assert.ok(await card(page, 'mod:alpha').locator('.acts .btn').isDisabled());
  assert.deepEqual(await calls(page, 'open_url'), [], 'the button does not open the page');
  await tabTo(page, 'mods');
  assert.equal(await card(page, 'link:0123456789ab').locator('.acts .btn').count(), 0, 'a community link has no INSTALL (and no page here)');
  assert.deepEqual(errors, []);
  await page.close();
});

test('rating stars: saved, or the sign-in note', async () => {
  let { page, errors } = await open();
  await card(page, 'mod:alpha').locator('.stars button').nth(3).click();
  await page.waitForFunction(() => /you rated Alpha 4 stars/.test(document.getElementById('cat-status').textContent));
  assert.deepEqual(await calls(page, 'mod_rate'), [{ project: 'mod:alpha', stars: 4 }]);
  assert.equal(await card(page, 'mod:alpha').locator('.stars button.on').count(), 4);
  await page.close();
  ({ page, errors } = await open({ signedOut: true }));
  await card(page, 'mod:bravo').locator('.stars button').first().click();
  await page.waitForFunction(() => /Sign in \(top right\) to rate mods/.test(document.getElementById('cat-status').textContent));
  assert.deepEqual(errors, []);
  await page.close();
});

test('a leyline://install link opens that project and installs nothing by itself', async () => {
  const { page, errors } = await open({ link: { action: 'install', id: 'mod:old-bars' }, tab: 'mods' });
  await page.waitForFunction(() => /From the hub: Old Bars/.test(document.getElementById('cat-status').textContent));
  assert.ok(await page.isVisible('#pane-discover'));
  assert.equal(await page.getAttribute('#pane-discover', 'data-shelf'), 'addons', 'an addon opens on the Addons tab');
  assert.equal(await page.isChecked('#cat-fit'), false, 'shown even though it is for another build');
  assert.ok(await card(page, 'mod:old-bars').evaluate((n) => n.classList.contains('linked')));
  assert.deepEqual(await calls(page, 'mod_install'), [], 'nothing installed without a click');
  assert.ok(await card(page, 'mod:old-bars').locator('.acts .btn').isDisabled(), "a 1.12 mod can't go into the 3.3.5a client");
  await page.close();
  const two = await open({ link: { action: 'install', id: 'mod:not-here' } });
  await two.page.waitForFunction(() => /isn.t in the mods list here yet/.test(document.getElementById('cat-status').textContent));
  assert.deepEqual(two.errors, []);
  assert.deepEqual(errors, []);
  await two.page.close();
});

test('offline rankings: the catalog order, the tabs and chips still work; Count my installs setting', async () => {
  const { page, errors } = await open({ noRanks: true });
  assert.deepEqual(await names(page), ['Alpha', 'Bravo']);
  assert.equal(await page.locator('#cat-grid .dc-meta .rating, #cat-grid .dc-meta .installs').count(), 0, 'no numbers without rankings');
  await tabTo(page, 'mods');
  assert.deepEqual(await names(page), ['Golf Cart', '<img src=x onerror="window.__pwned=1">']);
  await page.click('#dc-smart .chip-btn:has-text("Game modes")');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 1);
  await page.click('.nav[data-page="settings"]');
  assert.equal(await page.isChecked('#set-count'), true);
  await page.click('#set-count');
  await page.waitForFunction(() => /not counted/.test(document.getElementById('count-status').textContent));
  assert.deepEqual(await calls(page, 'set_count_installs'), [{ on: false }]);
  const text = await page.textContent('#page-settings');
  assert.match(text, /no account, nothing about you or your PC/);
  assert.doesNotMatch(text + await page.textContent('#page-mods'), /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i);
  assert.deepEqual(errors, []);
  await page.close();
});

test('pictures: the catalog field, the Workshop screenshot, the drawn tile on error; the card opens its page', async () => {
  const cat = JSON.parse(JSON.stringify(CATALOG));
  cat.projects[0].thumb = '/wrath/art/hub-thumbs/someone__alpha.webp';             // a catalog picture (site path)
  cat.projects[1].thumb = 'https://elsewhere.example/bravo.png';                     // another host: never used
  cat.projects[1].page = 'https://play.openvanilla.app/wrath/mods/bravo';
  cat.projects[4].repo = 'https://github.com/Owner/Repo';
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const asked = [];
  const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAABAAAAAJCAIAAAC0SDtlAAAAEklEQVR4nGNgGAWjYBSMAggAAAQQAAGFP6pyAAAAAElFTkSuQmCC', 'base64');
  await page.route('**/*', (r) => {
    const u = r.request().url();
    if (u.startsWith('file:')) return r.continue();
    asked.push(u);
    if (/hub-thumbs\/someone__alpha\.webp$/.test(u)) return r.fulfill({ status: 200, contentType: 'image/png', body: PNG });
    return r.fulfill({ status: 404, body: '' });
  });
  await page.addInitScript(FAKE, { catalog: cat, metrics: METRICS, order: ORDER });
  await page.goto(INDEX);
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length > 0);
  // Alpha: the catalog's picture, loaded
  await page.waitForFunction(() => document.querySelector('#cat-grid .mod-card[data-id="mod:alpha"] .dc-thumb').classList.contains('has-img'));
  assert.equal(await card(page, 'mod:alpha').locator('.dc-thumb img').getAttribute('src'), 'https://play.openvanilla.app/wrath/art/hub-thumbs/someone__alpha.webp');
  assert.equal(await card(page, 'mod:alpha').locator('.dc-thumb img').getAttribute('loading'), 'lazy');
  // Bravo: not the other host; the Workshop's first screenshot, a 404, so the drawn tile stays
  await page.waitForFunction(() => !document.querySelector('#cat-grid .mod-card[data-id="mod:bravo"] .dc-thumb img'));
  assert.ok(!asked.some((u) => u.includes('elsewhere.example')), 'never another host');
  assert.ok(asked.includes('https://play.openvanilla.app/wrath/mods/bravo/shot/0'), 'the Workshop screenshot was tried');
  assert.equal(await card(page, 'mod:bravo').locator('.dc-gen b').textContent(), 'B');
  assert.ok(await card(page, 'mod:bravo').locator('.dc-gen svg path').count() === 1, 'a category glyph');
  assert.ok(await card(page, 'mod:bravo').locator('.dc-gen').isVisible());
  const hues = await page.$$eval('#cat-grid .dc-gen', (g) => g.map((x) => x.style.getPropertyValue('--h1')));
  assert.ok(new Set(hues).size > 1, 'hues from the names');
  // the community link (on Mods): the hub's copy of its GitHub preview, lower-cased
  await tabTo(page, 'mods');
  await page.waitForFunction(() => document.querySelector('#cat-grid .mod-card[data-id="link:0123456789ab"]'));
  await page.waitForTimeout(100);
  assert.ok(asked.includes('https://play.openvanilla.app/wrath/art/hub-thumbs/owner__repo.webp'));
  assert.ok(asked.every((u) => u.startsWith('https://play.openvanilla.app/')), 'only the site');
  await tabTo(page, 'addons');
  // a click on the card (not its button) opens the page; INSTALL still installs
  await card(page, 'mod:bravo').locator('.desc').click();
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'open_url'));
  assert.deepEqual(await calls(page, 'open_url'), [{ url: 'https://play.openvanilla.app/wrath/mods/bravo' }]);
  await card(page, 'mod:bravo').locator('.acts .btn').click();
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'mod_install'));
  assert.deepEqual(await calls(page, 'mod_install'), [{ client: 'c335a-1', project: 'mod:bravo' }]);
  assert.equal((await calls(page, 'open_url')).length, 1, 'INSTALL does not open the page');
  // the client toggle: off shows the 1.12 mod, with its warning tag and a disabled INSTALL
  await page.click('#cat-fit');
  await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length === 3);
  assert.equal(await card(page, 'mod:old-bars').locator('.dc-tag').textContent(), 'Not for 3.3.5a');
  assert.ok(await card(page, 'mod:old-bars').locator('.acts .btn').isDisabled());
  assert.deepEqual(errors, []);
  await page.close();
});

test('Installed is split the same way: an addon only under Installed › Addons, a mod only under Installed › Mods', async () => {
  const I = (key, name, extra) => Object.assign({ key, name, version: '1.0.0', source: 'catalog', update: null, in_catalog: true, enabled: true,
    files: [], addons: [], patches: [], natives: [] }, extra || {});
  const installedMods = [I('mod:alpha', 'Alpha', { addons: ['Alpha'] }), I('mod:golfcart', 'Golf Cart', { addons: ['GolfCart'] }),
    I('local:frames', 'My Frames', { source: 'local', addons: ['MyFrames'] }), I('local:hd', 'HD Patch', { source: 'local', patches: ['patch-x.MPQ'] })];
  const { page, errors } = await open({ installedMods });
  await page.click('#tab-mods-installed');
  const shown = () => page.$$eval('#mods-list .card .name', (n) => n.map((x) => x.textContent));
  await page.click('#inst-tabs .ly-tab[data-shelf="mods"]');
  await page.waitForFunction(() => document.querySelectorAll('#mods-list .card').length === 2);
  assert.deepEqual(await shown(), ['Golf Cart', 'HD Patch'], 'a game mode and a client patch are mods');
  assert.match(await page.textContent('#inst-line'), /^Mods change the game itself/);
  assert.equal(await page.textContent('#inst-n-mods'), '2');
  await page.click('#inst-tabs .ly-tab[data-shelf="addons"]');
  await page.waitForFunction(() => document.querySelectorAll('#mods-list .card').length === 2 && document.getElementById('pane-installed').dataset.shelf === 'addons');
  assert.deepEqual(await shown(), ['Alpha', 'My Frames'], 'an addon from the list, and one from your PC with only addon folders');
  assert.equal(await page.textContent('#inst-n-addons'), '2');
  assert.equal(await page.textContent('#mods-find'), 'Find addons');
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.instShelf')), 'addons', 'remembered');
  assert.deepEqual(errors, []);
  await page.close();
});
