// Mods & Addons › Discover › Looks (ui/looks.js, src-tauri/src/looks.rs) in Chromium with a fake Tauri core: the tab, the
// looks from the mods list (never in the Mods or Addons lists), a preset in one click, a pack in and out, a link-only look
// opening its author's page, the type filter, "Back to the original look", and the website's leyline://install/look: link.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/looks.browser.test.cjs      (SHOTS=<dir> also saves screenshots)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');

const FAKE = (opts) => {
  const calls = (window.__calls = []);
  try { localStorage.setItem('leyline.page', 'mods'); localStorage.setItem('leyline.dcTab', opts.tab || 'looks'); } catch (_) { /* fine */ }
  const look = (id, name, type, extra) => Object.assign({ id, source: 'look', name, description: `${name}: what it does.`, category: type === 'ui-look' ? 'addons' : 'graphics',
    builds: ['1.12', '2.4.3', '3.3.5a'], author: 'Leyline', author_url: '', licence: 'MIT', repo: 'https://github.com/orionopenwow/Leyline', page: '',
    credits: `${name} by Leyline · MIT · github.com`, size: 0, look: { type, preset: '', uses: [], changes: ['View distance', 'Shadows'], featured: true, order: 1, note: '', how: 'launcher' } }, extra);
  const projects = [
    look('look:max', 'Max', 'preset', { look: { type: 'preset', preset: 'ultra', uses: [], changes: ['View distance', 'Shadows', 'Textures'], order: 1, note: 'Changes the game\'s own settings file.', how: 'launcher' } }),
    look('look:classic', 'Classic', 'preset', { look: { type: 'preset', preset: 'classic', uses: [], changes: ['View distance'], order: 3, note: '', how: 'launcher' } }),
    look('look:water', 'Clear water', 'graphics-pack', { author: 'alice', repo: 'https://github.com/alice/water', page: 'https://github.com/alice/water', builds: ['3.3.5a'],
      licence: 'CC-BY-4.0', credits: 'Clear water by alice · CC-BY-4.0 · github.com', size: 7340032,
      look: { type: 'graphics-pack', preset: '', uses: [], changes: ['Water'], order: 5, note: '', how: 'launcher' } }),
    look('look:forever-ui', 'Forever UI', 'ui-look', { author: 'Adrestie', repo: 'https://github.com/Adrestie/mod-forever-ui', page: 'https://github.com/Adrestie/mod-forever-ui',
      builds: ['3.3.5a'], licence: 'GPL-2.0', credits: 'Forever UI by Adrestie · GPL-2.0 · github.com', size: 346030080,
      look: { type: 'ui-look', preset: '', uses: [], changes: ['Interface', 'Login screen'], order: 4, note: 'Get it from its author\'s page.', how: 'link' } }),
    look('look:hd', 'HD pack', 'graphics-pack', { author: 'the pack authors', repo: '', page: 'https://example.org/hd-pack', builds: ['1.12'], licence: '',
      credits: 'HD pack by the pack authors · licence not stated · example.org', size: null,
      look: { type: 'graphics-pack', preset: '', uses: [], changes: ['Textures'], order: 6, note: '', how: 'link' } }),
    { id: 'mod:pings', source: 'packaged', name: 'Pings', description: 'Alt-click the map.', category: 'addons', builds: ['3.3.5a'], author: 'WU', licence: 'MIT',
      repo: '', page: '', credits: 'Pings by WU', version: '0.1.0', kind: 'ui', download: 'https://play.openvanilla.app/wrath/mods/pings/download', sha256: 'a'.repeat(64) },
  ];
  const how = { 'look:max': 'preset', 'look:classic': 'preset', 'look:water': 'pack', 'look:forever-ui': 'link', 'look:hd': 'link' };
  let on = [];
  let changed = false;
  const state = () => ({ client: 'My Wrath', build: '3.3.5a', on: on.slice(), how, changed });
  let link = opts.link ? { action: 'install', id: opts.link } : null;
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c1', name: 'My Wrath', path: 'C:/Games/Wrath', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c1', selected_realm: 'play.openvanilla.app', wine: { enabled: false, command: 'wine', prefix: '' } },
        official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'looks_state': return state();
      case 'looks_apply':
        if (args.look === 'look:max' || args.look === 'look:classic') { on = on.filter((x) => x !== 'look:max' && x !== 'look:classic'); }
        on.push(args.look); changed = true;
        return { id: args.look, name: args.look, how: how[args.look], said: args.look === 'look:max' ? 'Max is on. It takes effect the next time the game starts.' : 'Clear water is in. It shows the next time the game starts.', installed: [], warnings: [] };
      case 'looks_remove': on = on.filter((x) => x !== args.look); return 1;
      case 'looks_reset': { const n = on.filter((x) => how[x] !== 'preset').length; on = []; changed = false;
        return { removed: n, settings_restored: true, said: `Back to the original look: ${n} look taken out, and the game's settings are as they were before Leyline changed them.` }; }
      case 'open_url': return null;
      case 'gfx_get': return { client: 'My Wrath', build: '3.3.5a', file: 'x', tweaks: [], values: {}, presets: [], has_backup: false };
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'realm_status': return { state: 'online', ping_ms: 10 };
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'mods_catalog': return { categories: [{ id: 'addons', name: 'Addons and UI', line: '', icon: '' }, { id: 'graphics', name: 'Shaders and graphics', line: '', icon: '' }], projects, from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: projects.map((p) => p.id), metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c1', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'deep_link_take': { const l = link; link = null; return l; }
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return false;
      case 'addons_status': return { prefs: { auto_update: true, dont_ask: true }, clients: [], available: null };
      case 'addons_should_offer': return false;
      case 'addons_auto_update': return [];
      default: return null;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts = {}) {
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  if (!opts.link) {   // a link opens the page itself (app.js handleDeepLink)
    await page.evaluate(() => { const b = document.querySelector('.nav[data-page="mods"]'); if (b) b.click(); });
    await page.evaluate(() => { const t = document.querySelector('.page-tab[data-tab="discover"]'); if (t) t.click(); });
  }
  await page.waitForSelector('#discover-tabs .ly-tab[data-tab="looks"]');
  return { ctx, page, errors };
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c).map(([, a]) => a), cmd);

test('Looks: the tab, the cards, a preset, a pack in and out, a link, the filter, back to the original look', async () => {
  const { ctx, page, errors } = await open();
  assert.deepEqual(await page.$$eval('#discover-tabs .ly-tab', (b) => b.map((x) => x.textContent)), ['Mods', 'Addons', 'Looks', 'Themes', 'Graphics']);
  await page.click('#discover-tabs .ly-tab[data-tab="looks"]');
  await page.waitForSelector('#looks-grid .lk-card');
  const ids = await page.$$eval('#looks-grid .lk-card', (c) => c.map((x) => x.dataset.id));
  assert.deepEqual(ids, ['look:max', 'look:classic', 'look:forever-ui', 'look:water', 'look:hd'], 'in their order');
  assert.ok(await page.isHidden('#cat-grid'), 'the Mods list is not on this tab');
  const max = '#looks-grid .lk-card[data-id="look:max"]';
  assert.match(await page.textContent(max), /Graphics preset/);
  assert.match(await page.textContent(max), /Changes: View distance, Shadows, Textures/);
  assert.match(await page.textContent(max), /Nothing to download/);
  assert.equal(await page.textContent(`${max} .lk-credit`), 'Max by Leyline · MIT · github.com');
  assert.deepEqual(await page.$$eval(`${max} .lk-chips li`, (l) => l.map((x) => x.textContent)), ['1.12', '2.4.3', '3.3.5a']);
  assert.equal(await page.$eval(`${max} .lk-chips li.on`, (x) => x.textContent), '3.3.5a', 'this game version marked');
  assert.ok(await page.isDisabled('#looks-reset'), 'nothing to put back yet');
  if (process.env.SHOTS) await page.screenshot({ path: path.join(process.env.SHOTS, 'launcher-looks.png') });
  // a preset in one click
  await page.click(`${max} .lk-go`);
  await page.waitForFunction(() => /Max is on/.test(document.getElementById('looks-note').textContent));
  assert.deepEqual(await calls(page, 'looks_apply'), [{ client: 'c1', look: 'look:max' }]);
  assert.equal(await page.textContent(`${max} .lk-go`), 'In use');
  assert.ok(await page.isDisabled(`${max} .lk-go`));
  assert.equal(await page.textContent('#looks-grid .lk-card[data-id="look:classic"] .lk-go'), 'Use');
  // a pack in, then out
  const water = '#looks-grid .lk-card[data-id="look:water"]';
  assert.match(await page.textContent(water), /7 MB/);
  await page.click(`${water} .lk-go`);
  await page.waitForFunction((s) => document.querySelector(s + ' .lk-go').textContent === 'Remove', water);
  await page.click(`${water} .lk-go`);
  await page.waitForFunction((s) => document.querySelector(s + ' .lk-go').textContent === 'Install', water);
  assert.deepEqual(await calls(page, 'looks_remove'), [{ client: 'c1', look: 'look:water' }]);
  assert.match(await page.textContent('#looks-note'), /Clear water is out/);
  // a link-only look: no Install, its author's page in the browser
  const fui = '#looks-grid .lk-card[data-id="look:forever-ui"]';
  assert.equal(await page.$(`${fui} .lk-go`), null);
  await page.click(`${fui} .lk-page`);
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'open_url'));
  assert.deepEqual(await calls(page, 'open_url'), [{ url: 'https://github.com/Adrestie/mod-forever-ui' }]);
  // a look for another game version: no one-click on this game
  assert.equal(await page.$('#looks-grid .lk-card[data-id="look:hd"] .lk-go'), null);
  assert.match(await page.textContent('#looks-grid .lk-card[data-id="look:hd"]'), /Size on the author's page/);
  // the filter
  await page.click('.lk-filter .ly-tab[data-type="ui-look"]');
  assert.deepEqual(await page.$$eval('#looks-grid .lk-card', (c) => c.map((x) => x.dataset.id)), ['look:forever-ui']);
  await page.click('.lk-filter .ly-tab[data-type=""]');
  // back to the original look
  assert.ok(await page.isEnabled('#looks-reset'));
  await page.click('#looks-reset');
  await page.waitForFunction(() => /Back to the original look/.test(document.getElementById('looks-note').textContent));
  assert.deepEqual(await calls(page, 'looks_reset'), [{ client: 'c1' }]);
  assert.equal(await page.textContent(`${max} .lk-go`), 'Use');
  assert.ok(await page.isDisabled('#looks-reset'));
  // the looks stay out of the Mods and Addons lists
  await page.click('#discover-tabs .ly-tab[data-tab="addons"]');
  await page.waitForTimeout(150);
  const cat = await page.$$eval('#cat-grid .mod-card', (c) => c.map((x) => x.dataset.id));
  assert.ok(!cat.some((id) => id.startsWith('look:')), JSON.stringify(cat));
  assert.ok(await page.isHidden('#looks-discover'));
  const text = await page.evaluate(() => document.body.innerText);
  assert.doesNotMatch(text, /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test("Looks: the website's Install with Leyline opens the Looks tab at that card; nothing installs until the click", async () => {
  const { ctx, page, errors } = await open({ tab: 'mods', link: 'look:water' });
  await page.waitForSelector('#looks-grid .lk-card.linked[data-id="look:water"]');
  assert.equal(await page.$eval('#discover-tabs .ly-tab[aria-selected="true"]', (b) => b.dataset.tab), 'looks');
  assert.match(await page.textContent('#looks-note'), /From the website: Clear water\. Press Install/);
  assert.deepEqual(await calls(page, 'looks_apply'), []);
  if (process.env.SHOTS) await page.screenshot({ path: path.join(process.env.SHOTS, 'launcher-looks-link.png') });
  assert.deepEqual(errors, []);
  await ctx.close();
});
