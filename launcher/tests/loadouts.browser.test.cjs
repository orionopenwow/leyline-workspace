// Loadouts (ui/loadouts.js over src-tauri/src/loadouts.rs) in Chromium with a fake Tauri core: the box on Mods & Addons ›
// Installed (Clean, save what's on, switch, Undo, Back to Clean, rename, copy, delete, graphics), templates (only items in
// the catalog: reviewed entries; the missing ones installed through mod_install), paste a code and share one, the game
// folder card's "Loadout: <name>" and Change, a realm's suggested loadout on join (Use it, Keep mine, Cancel stops PLAY,
// Back to ...), Installed opening on what was just installed (an addon: the Addons tab, its card marked), and Mods &
// Addons › Game modes (the modes to play, Your Den, the game mode mods).
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/loadouts.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const MODES = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'stream', 'playmodes.json'), 'utf8')).modes;

const UP = (slug, name) => ({ id: 'up:' + slug, source: 'upstream', name, description: name + ' helps.', category: 'addons', builds: ['3.3.5a'],
  author: 'a', author_url: '', licence: 'MIT', repo: 'https://github.com/a/' + slug, page: '', credits: '', stars: null, last_push: '',
  version: '1.0', kind: 'ui', permissions: [], on_wrath: false, download: null, sha256: null, size: 1000, tags: ['meters'],
  install: { from: 'commit-archive', url: 'x', folders: { '': name } }, addons: [name], requires: [], conflicts: [], replaces_ui: false });
const PK = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: name + ' does things.', category: 'sandbox',
  builds: ['3.3.5a'], author: 'S', author_url: '', licence: 'MIT', repo: '', page: '', credits: '', stars: null, last_push: '',
  version: '1.0.0', kind: 'both', permissions: [], on_wrath: false, download: 'https://leylinelauncher.com/wrath/mods/x/download',
  sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
// the catalog has only reviewed entries: Skada, Questie, IDTip and a game mode; TellMeWhen etc. are not in it
const CATALOG = { categories: [{ id: 'addons', name: 'Addons', line: '', icon: '' }, { id: 'sandbox', name: 'Game modes', line: '', icon: '' }],
  projects: [UP('skada', 'Skada'), UP('questie', 'Questie'), UP('idtip', 'IDTip'), PK('mod:golfcart', 'Golf Cart')], from_cache: false, saved_at: 1, error: null };

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try {
    localStorage.setItem('leyline.page', o.page || 'installed');
    localStorage.setItem('leyline.mode', 'online');
    localStorage.setItem('leyline.instShelf', 'mods');   // the bug: Installed remembered Mods
  } catch (_) { /* fine */ }
  const clients = [{ id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' }];
  const cat = o.catalog;
  // the installed mods (mods_installed's shape) and the loadout book (loadouts.rs, in short)
  let mods = (o.mods || []).map((m) => Object.assign({ version: '1.0', source: 'catalog', update: null, in_catalog: true, enabled: true, files: [], addons: [m.name], patches: [], natives: [], warnings: [] }, m));
  const book = Object.assign({ active: '', previous: '', loadouts: [] }, o.book || {});
  const on = () => mods.filter((m) => m.enabled).map((m) => m.key);
  const sync = () => {
    const l = book.loadouts.find((x) => x.id === book.active);
    if (!l) return;
    const items = l.items.filter((i) => { const m = mods.find((x) => x.key === i); return !m || m.enabled; });
    for (const k of on()) if (!items.includes(k)) items.push(k);
    l.items = items;
  };
  const idOf = (name) => { let b = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'loadout'; if (b === 'clean') b = 'loadout'; let id = b, n = 2; while (book.loadouts.some((l) => l.id === id)) id = `${b}-${n++}`; return id; };
  const add = (name, items, gfx, from) => { sync(); const l = { id: idOf(name), name, items, gfx: gfx || '', from: from || '', created_at: 1 }; book.loadouts.push(l); return JSON.parse(JSON.stringify(l)); };
  const status = () => { sync(); return JSON.parse(JSON.stringify({ client: 'c335a-1', active: book.active, previous: book.previous, loadouts: book.loadouts,
    installed: mods.map((m) => ({ key: m.key, name: m.name, enabled: m.enabled })), drift: book.active === 'clean' ? on() : [] })); };
  const PREFIX = 'leyline-loadout:1;';
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients, selected_client: 'c335a-1', selected_realm: 'play.openvanilla.app',
        wine: { enabled: false, command: 'wine', prefix: '' }, count_installs: true }, official_realm: 'play.openvanilla.app',
        browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'select': return null;
      case 'check_client': return { complete: true, missing: [], exe_version: '3.3.5.12340' };
      case 'mods_catalog': return cat;
      case 'mods_ranked': return { ids: cat.projects.map((p) => p.id), metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c335a-1', build: '3.3.5a', mods: JSON.parse(JSON.stringify(mods)) };
      case 'mods_updates': return [];
      case 'mod_install': {
        const p = cat.projects.find((x) => x.id === args.project);
        mods = mods.filter((m) => m.key !== p.id).concat([{ key: p.id, name: p.name, version: p.version, source: 'catalog', update: null, in_catalog: true, enabled: true, files: [], addons: p.addons || [p.name], patches: [], natives: [], warnings: [] }]);
        return { key: p.id, name: p.name, version: p.version, warnings: [], kept: [], addons: p.addons || [], patches: [] };
      }
      case 'mod_set_enabled': { const m = mods.find((x) => x.key === args.key); m.enabled = args.on; return null; }
      case 'realms_list': return { realms: o.realms || [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: o.modes || [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return true;
      case 'addons_status': return { pack: null, from_cache: false, error: null, prefs: { auto_update: false, dont_ask: true }, clients: [] };
      case 'addons_auto_update': return [];
      case 'play_realm': return 'Started.';
      // loadouts.rs
      case 'loadouts_status': return status();
      case 'loadout_new': return add(args.name, args.empty ? [] : on(), '', '');
      case 'loadout_add_shared': return add(args.shared.name, args.shared.items, args.shared.gfx, args.from);
      case 'loadout_import': {
        if (!String(args.code).startsWith(PREFIX)) throw "That isn't a Leyline loadout code.";
        const parts = Object.fromEntries(args.code.slice(PREFIX.length).split(';').map((p) => p.split('=')));
        return add(decodeURIComponent(parts.n || 'Shared loadout'), (parts.i || '').split(',').filter(Boolean), parts.g || '', 'code');
      }
      case 'loadout_code': { sync(); const l = book.loadouts.find((x) => x.id === args.id); return `${PREFIX}n=${encodeURIComponent(l.name)};g=${l.gfx};i=${l.items.filter((i) => !i.startsWith('local:')).join(',')}`; }
      case 'loadout_copy': { sync(); const l = book.loadouts.find((x) => x.id === args.id); return add(args.name, l.items.slice(), l.gfx, l.from); }
      case 'loadout_rename': { const l = book.loadouts.find((x) => x.id === args.id); l.name = args.name; return l; }
      case 'loadout_set_gfx': { const l = book.loadouts.find((x) => x.id === args.id); l.gfx = args.gfx; return l; }
      case 'loadout_delete': { if (book.active === args.id) throw 'That loadout is on.'; book.loadouts = book.loadouts.filter((l) => l.id !== args.id); return null; }
      case 'loadout_switch': {
        sync();
        const l = args.id === 'clean' ? { name: 'Clean', items: [], gfx: '' } : book.loadouts.find((x) => x.id === args.id);
        const r = { active: args.id, name: l.name, previous: '', enabled: [], disabled: [], missing: [], problems: [], backup: 'Leyline/loadouts/x/WTF', gfx: args.id === 'clean' ? 'original' : l.gfx ? 'preset' : '' };
        for (const m of mods) {
          const want = l.items.includes(m.key);
          if (want && !m.enabled) { m.enabled = true; r.enabled.push(m.key); } else if (!want && m.enabled) { m.enabled = false; r.disabled.push(m.key); }
        }
        r.missing = l.items.filter((i) => !mods.some((m) => m.key === i));
        if (book.active !== args.id) book.previous = book.active;
        book.active = args.id;
        r.previous = book.previous;
        return r;
      }
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(opts = {}) {
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: 1400, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://**', (r) => r.abort());
  await page.addInitScript(FAKE, Object.assign({ catalog: CATALOG }, opts));
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineLoadouts && window.LeylineLoadouts.status());
  await page.waitForSelector('#lo-box:not([hidden]) .lo-chip');
  return { page, ctx, errors };
}
const calls = (page, name) => page.evaluate((n) => window.__calls.filter(([c]) => c === n).map(([, a]) => a), name);
const chips = (page) => page.$$eval('#lo-box .lo-chip', (l) => l.map((x) => (x.classList.contains('on') ? '*' : '') + x.querySelector('.lo-chip-b').textContent));
const statusText = (page) => page.textContent('#lo-status');
const dialogOpen = (page) => page.waitForSelector('#lo-dlg[open]');

test('Installed: Clean, save what is on now, switch to Clean and back with Undo; game files never mentioned as gone', async () => {
  const { page, ctx, errors } = await open({ mods: [{ key: 'up:skada', name: 'Skada' }, { key: 'up:questie', name: 'Questie' }] });
  assert.deepEqual(await chips(page), ['Clean'], 'Clean is always there');
  assert.match(await page.textContent('#lo-box'), /No loadout is on yet/);
  assert.match(await page.textContent('#lo-box .lo-safe'), /never deletes or changes your game files/);
  await page.click('#lo-box button:has-text("Save what\'s on now")');
  await dialogOpen(page);
  await page.fill('#lo-dlg-input', 'Raider');
  await page.click('#lo-dlg button:has-text("Save")');
  await page.waitForFunction(() => /Raider is on/.test(document.getElementById('lo-status').textContent));
  assert.deepEqual(await chips(page), ['Clean', '*Raider']);
  assert.deepEqual((await calls(page, 'loadout_new'))[0], { client: 'c335a-1', name: 'Raider', empty: false });
  // to Clean: everything Leyline added turns off; Undo puts Raider back
  await page.click('#lo-box .lo-chip[data-id="clean"] .lo-chip-b');
  await page.waitForFunction(() => /Clean is on/.test(document.getElementById('lo-status').textContent));
  assert.match(await statusText(page), /Turned off: Skada, Questie\..*Graphics back to how they were/);
  assert.deepEqual(await chips(page), ['*Clean', 'Raider']);
  await page.waitForFunction(() => document.querySelectorAll('#mods-list .card.off').length === 0 || true);
  await page.click('#lo-status .lo-undo');
  await page.waitForFunction(() => /Raider is on/.test(document.getElementById('lo-status').textContent));
  assert.match(await statusText(page), /Turned on: Skada, Questie/);
  assert.deepEqual((await calls(page, 'loadout_switch')).map((a) => a.id), ['raider', 'clean', 'raider']);
  // Back to Clean is one button too
  assert.ok(await page.isVisible('#lo-box button:has-text("Back to Clean")'));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Manage: rename, copy, graphics, delete (never the one that is on), share code', async () => {
  const { page, ctx, errors } = await open({ mods: [{ key: 'up:skada', name: 'Skada' }],
    book: { active: 'raider', loadouts: [{ id: 'raider', name: 'Raider', items: ['up:skada'], gfx: '', from: '' }] } });
  assert.deepEqual(await chips(page), ['Clean', '*Raider']);
  assert.equal(await page.locator('#lo-box .lo-chip[data-id="raider"] .lo-chip-x').count(), 0, 'no delete on the one that is on');
  await page.click('#lo-box button:has-text("Rename")');
  await dialogOpen(page);
  await page.fill('#lo-dlg-input', 'Raid night');
  await page.click('#lo-dlg button:has-text("Rename")');
  await page.waitForFunction(() => /Renamed to Raid night/.test(document.getElementById('lo-status').textContent));
  await page.click('#lo-box button:has-text("Copy")');
  await dialogOpen(page);
  assert.equal(await page.inputValue('#lo-dlg-input'), 'Raid night copy');
  await page.click('#lo-dlg button.primary');
  await page.waitForFunction(() => document.querySelectorAll('#lo-box .lo-chip').length === 3);
  assert.deepEqual(await chips(page), ['Clean', '*Raid night', 'Raid night copy']);
  await page.selectOption('#lo-box .lo-gfx select', 'high');
  await page.waitForFunction(() => /uses High graphics/.test(document.getElementById('lo-status').textContent));
  assert.deepEqual((await calls(page, 'loadout_set_gfx'))[0], { client: 'c335a-1', id: 'raider', gfx: 'high' });
  // delete the copy: only the loadout goes
  await page.click('#lo-box .lo-chip[data-id="raid-night-copy"] .lo-chip-x');
  await dialogOpen(page);
  assert.match(await page.textContent('#lo-dlg'), /Every mod and addon stays installed/);
  await page.click('#lo-dlg button:has-text("Delete")');
  await page.waitForFunction(() => document.querySelectorAll('#lo-box .lo-chip').length === 2);
  // share: a code of catalog ids, read only, Copy
  await page.click('#lo-box button:has-text("Share")');
  await dialogOpen(page);
  assert.equal(await page.inputValue('#lo-dlg-input'), 'leyline-loadout:1;n=Raid%20night;g=high;i=up:skada');
  assert.equal(await page.getAttribute('#lo-dlg-input', 'readonly'), '');
  await page.click('#lo-dlg button:has-text("Close")');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Templates: only what the catalog has (reviewed), Use adds a new loadout and installs what is missing', async () => {
  const { page, ctx, errors } = await open({ mods: [{ key: 'up:skada', name: 'Skada' }] });
  await page.click('#lo-box button:has-text("Templates")');
  await dialogOpen(page);
  const names = await page.$$eval('#lo-dlg .lo-tpl b', (l) => l.map((x) => x.textContent));
  assert.ok(names.includes('Quester') && names.includes('Raider') && names.includes('Best graphics') && names.includes('Old PC'), names.join());
  assert.ok(!names.includes('Tidy interface'), 'none of its addons is in the catalog: left out');
  const raider = await page.textContent('#lo-dlg .lo-tpl[data-id="raider"] small');
  assert.equal(raider, 'Skada · Graphics: Balanced', 'TellMeWhen and the rest are not reviewed here: never shown');
  await page.click('#lo-dlg .lo-tpl[data-id="quester"] button');
  await page.waitForFunction(() => /Quester is on/.test(document.getElementById('lo-status').textContent) && /Installed:/.test(document.getElementById('lo-status').textContent));
  const add = (await calls(page, 'loadout_add_shared'))[0];
  assert.deepEqual(add, { client: 'c335a-1', shared: { name: 'Quester', items: ['up:questie', 'up:idtip'], gfx: 'balanced' }, from: 'template:quester' });
  assert.deepEqual((await calls(page, 'mod_install')).map((a) => a.project), ['up:questie', 'up:idtip']);
  assert.match(await statusText(page), /Turned off: Skada\..*Installed: Questie, IDTip\./);
  // using it again reuses the loadout made from it
  await page.click('#lo-box .lo-chip[data-id="clean"] .lo-chip-b');
  await page.waitForFunction(() => /Clean is on/.test(document.getElementById('lo-status').textContent));
  await page.click('#lo-box button:has-text("Templates")');
  await dialogOpen(page);
  await page.click('#lo-dlg .lo-tpl[data-id="quester"] button');
  await page.waitForFunction(() => /Quester is on/.test(document.getElementById('lo-status').textContent));
  assert.equal((await calls(page, 'loadout_add_shared')).length, 1);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Paste a code: added and used; a bad code says so and changes nothing', async () => {
  const { page, ctx, errors } = await open({ mods: [] });
  await page.click('#lo-box button:has-text("Paste a code")');
  await dialogOpen(page);
  await page.fill('#lo-dlg-input', 'hello');
  await page.click('#lo-dlg button:has-text("Add and use")');
  await page.waitForFunction(() => document.getElementById('lo-status').className.includes('bad'));
  assert.equal((await calls(page, 'loadout_switch')).length, 0);
  await page.click('#lo-box button:has-text("Paste a code")');
  await dialogOpen(page);
  await page.fill('#lo-dlg-input', 'leyline-loadout:1;n=Friend;g=;i=up:skada');
  await page.click('#lo-dlg button:has-text("Add and use")');
  await page.waitForFunction(() => /Friend is on/.test(document.getElementById('lo-status').textContent));
  assert.deepEqual((await calls(page, 'loadout_import'))[1], { client: 'c335a-1', code: 'leyline-loadout:1;n=Friend;g=;i=up:skada' });
  assert.deepEqual((await calls(page, 'mod_install')).map((a) => a.project), ['up:skada']);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Game folders: each card says its loadout, Change opens Installed', async () => {
  const { page, ctx, errors } = await open({ mods: [{ key: 'up:skada', name: 'Skada' }],
    book: { active: 'raider', loadouts: [{ id: 'raider', name: 'Raider', items: ['up:skada'], gfx: '', from: '' }] } });
  await page.evaluate(() => window.LeylineNav.show('clients'));
  await page.waitForFunction(() => /Loadout: Raider/.test((document.querySelector('#cl-list .lo-slot') || {}).textContent || ''));
  await page.click('#cl-list .lo-slot .lo-change');
  await page.waitForFunction(() => document.getElementById('pane-installed').classList.contains('on'));
  assert.ok(await page.isVisible('#lo-box .lo-chip.on'));
  assert.deepEqual(errors, []);
  await ctx.close();
});

const FROST = { slug: 'c-frost', name: 'Frost', official: false, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [],
  players: 3, bots: 0, address: 'frost.example', mods: [], loadout: { name: 'Frost raiders', items: ['up:skada', 'up:questie'], gfx: 'high' } };

test("A realm's loadout on join: Use it (reversible: Back to ...), Keep mine (not asked again), Cancel stops", async () => {
  const { page, ctx, errors } = await open({ mods: [{ key: 'up:idtip', name: 'IDTip' }], realms: [FROST],
    book: { active: 'mine', loadouts: [{ id: 'mine', name: 'Mine', items: ['up:idtip'], gfx: '', from: '' }] } });
  const join = () => page.evaluate((r) => window.LeylineLoadouts.beforeJoin(r, window.LeylineApp.state().settings.clients[0]).then((x) => (window.__join = x)), FROST);
  // Cancel: PLAY stops, nothing changes
  page.evaluate(() => { window.__join = null; });
  const j1 = join();
  await dialogOpen(page);
  assert.match(await page.textContent('#lo-dlg'), /Frost suggests a loadout: Frost raiders \(it uses Skada, Questie, with High graphics\)\. Your current loadout \(Mine\) stays saved/);
  await page.click('#lo-dlg button:has-text("Cancel")');
  await j1;
  assert.deepEqual(await page.evaluate(() => window.__join), { go: false });
  assert.equal((await calls(page, 'loadout_add_shared')).length, 0);
  // Use it
  const j2 = join();
  await dialogOpen(page);
  await page.click('#lo-dlg button:has-text("Use it")');
  await j2;
  assert.deepEqual(await page.evaluate(() => window.__join), { go: true });
  assert.deepEqual((await calls(page, 'loadout_add_shared'))[0].from, 'realm:c-frost');
  await page.waitForSelector('#lo-box .lo-back');
  assert.equal(await page.textContent('#lo-box .lo-back'), 'Back to Mine');
  // already on: no question
  assert.deepEqual(await page.evaluate((r) => window.LeylineLoadouts.beforeJoin(r, window.LeylineApp.state().settings.clients[0]), FROST), { go: true });
  await page.click('#lo-box .lo-back');
  await page.waitForFunction(() => /Mine is on/.test(document.getElementById('lo-status').textContent));
  // Keep mine: remembered for that realm
  const j3 = join();
  await dialogOpen(page);
  await page.click('#lo-dlg button:has-text("Keep mine")');
  await j3;
  assert.deepEqual(await page.evaluate(() => window.__join), { go: true });
  const n = (await calls(page, 'loadout_switch')).length;
  assert.deepEqual(await page.evaluate((r) => window.LeylineLoadouts.beforeJoin(r, window.LeylineApp.state().settings.clients[0]), FROST), { go: true });
  assert.equal(await page.locator('#lo-dlg[open]').count(), 0, 'not asked again');
  assert.equal((await calls(page, 'loadout_switch')).length, n);
  // a realm without a loadout: nothing to ask
  assert.deepEqual(await page.evaluate(() => window.LeylineLoadouts.beforeJoin({ name: 'Plain', slug: 'c-plain' }, window.LeylineApp.state().settings.clients[0])), { go: true });
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Installed opens on what was just installed: an addon from Discover lands on Addons with its card marked', async () => {
  const { page, ctx, errors } = await open({ mods: [] });
  assert.equal(await page.getAttribute('#pane-installed', 'data-shelf'), 'mods', 'it remembered Mods');
  await page.evaluate(() => window.LeylineNav.show('mods', false, 'discover'));
  await page.evaluate(async () => { const p = catalog.projects.find((x) => x.id === 'up:skada'); await installProject(p, document.createElement('button')); });
  await page.click('#tab-mods-installed');
  await page.waitForSelector('#mods-list .card.just[data-key="up:skada"]');
  assert.equal(await page.getAttribute('#pane-installed', 'data-shelf'), 'addons');
  assert.equal(await page.getAttribute('#inst-tabs .ly-tab[data-shelf="addons"]', 'aria-selected'), 'true');
  // and a game mode (a mod) lands on Mods
  await page.evaluate(async () => { const p = catalog.projects.find((x) => x.id === 'mod:golfcart'); await installProject(p, document.createElement('button')); });
  await page.click('#tab-mods-discover');
  await page.click('#tab-mods-installed');
  await page.waitForSelector('#mods-list .card.just[data-key="mod:golfcart"]');
  assert.equal(await page.getAttribute('#pane-installed', 'data-shelf'), 'mods');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Mods & Addons › Game modes: the modes to play, Your Den, the game mode mods with Install', async () => {
  const { page, ctx, errors } = await open({ mods: [], modes: MODES });
  await page.click('#tab-mods-gamemodes');
  await page.waitForFunction(() => document.querySelectorAll('#gm-section .gm-play').length > 3);
  const text = await page.textContent('#gm-section');
  assert.match(text, /Play now/);
  assert.match(text, /Minigolf/);
  assert.match(text, /Your Den/);
  assert.match(text, /Azeroth Survivors/);
  assert.match(text, /Game mode mods/);
  assert.ok(await page.isVisible('#gm-section .gm-play[data-id="mode:minigolf"] button:has-text("Play")'));
  assert.ok(await page.isVisible('#gm-section .gm-den button:has-text("Open Your Den")'));
  await page.click('#gm-section .gm-card[data-id="mod:golfcart"] button:has-text("Install")');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'mod_install' && a.project === 'mod:golfcart'));
  assert.ok(!/World of Warcraft|\bWoW\b|Warcraft|Lich King|(?<!endorsed by )Blizzard/i.test(text));
  assert.deepEqual(errors, []);
  await ctx.close();
});
