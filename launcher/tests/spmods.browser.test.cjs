// Single player's "Mods in this world" (spmods.js, index.html #spm) and getting mods for single player (app.js
// modsForSingle, spCardButton), in Chromium with a fake Tauri core (owner 12 Nov: "make sure it's easy to get mods from
// the mod store for singleplayer and user has interface for that in the single player menu to see what mods they have
// on"):
//   * the panel sits after Your world's settings and before Friends, and lists the game folder single player plays
//     with (not the play bar's): its mods, addons, HD packs, the look in use and the Leyline add-ons, then the world's
//     own modules; read from disk only (no addons refresh, no catalog fetch of its own);
//   * On / Off (mod_set_enabled), Update (mod_install), Remove (mod_uninstall), Page; the world's modules Off / On
//     (world_mods_set) with Restart now while the world runs; the locked ones have no switch;
//   * nothing yet: "No mods yet" with Browse the mod store;
//   * Get mods for single player: Discover on single player's game folder, "Works with this game folder" on, the bar
//     with Back to Single player; Add to single player on a card for another folder, never on one with no Install;
//     after an install the panel shows it;
//   * an older core without world_mods_get: the game folder's part still shows;
//   * no brand words.
// Full-app screenshots: SPMODS_SHOTS=<dir> (default /mnt/user-data/outputs/gallery when it exists), spmods-*.png.
//
//   node --test launcher/tests/spmods.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');
const { appLook } = require('./fixtures/app-look.cjs');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.SPMODS_SHOTS || (fs.existsSync('/mnt/user-data/outputs') ? '/mnt/user-data/outputs/gallery' : '');
const SP = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const OTHER = { id: 'c335a-2', name: 'Spare Wrath', path: 'D:\\Spare', build: '3.3.5a', realmlist: '' };
const WS = (o) => Object.assign({ supported: true, installed: true, set_up: true, running: false, backups: 0, other_running: null, account: '', password: '' }, o);
const SET = { companions_auto: false, companions_follow: 0, dungeon_auto: true, roaming_bots: 0, xp_rate: 2, chosen: true, set_up: true, running: false };
const M = (key, name, o) => Object.assign({ key, name, version: '1.0.0', enabled: true, source: 'catalog', addons: [], patches: [], natives: [], credits: '', warnings: [],
  native_code: false, update: null, in_catalog: true, files: [] }, o || {});
const WORLD = [
  { id: 'companions', name: 'Companions and bots', line: 'The bots that travel with you.', on: true, locked: true, note: 'Your companions need it.', mine: false },
  { id: 'transmog', name: 'Transmogrification', line: 'Change how your gear looks and keep its stats.', on: true, locked: false, note: '', mine: false },
  { id: 'aoeloot', name: 'Area loot', line: 'Loot every corpse around you at once.', on: true, locked: false, note: '', mine: false },
];

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try {
    if (!sessionStorage.getItem('__booted')) {
      sessionStorage.setItem('__booted', '1');
      localStorage.clear();
      localStorage.setItem('leyline.mode', 'online');
      localStorage.setItem('leyline.single.folder', o.sp);
      localStorage.setItem('leyline.dcView', 'all');
      localStorage.setItem('leyline.dcTab', 'addons');
    }
  } catch (_) { /* fine */ }
  window.__o = o;
  window.__ws = o.worlds;
  window.__mods = o.mods;   // client id -> installed
  window.__world = o.world;
  const st = { version: 'test', settings: { clients: o.clients, selected_client: o.selected, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  const P = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: name + ' does things.', category: 'addons',
    builds: ['3.3.5a'], author: 'Someone', author_url: '', licence: 'MIT', repo: '', page: 'https://leylinelauncher.com/wrath/mods/' + id.replace(/\W/g, ''),
    credits: `${name} by Someone · MIT`, stars: null, last_push: '', version: '1.1.0', kind: 'ui', permissions: [], on_wrath: false, tags: [],
    download: 'https://leylinelauncher.com/x', sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
  const catalog = { categories: [{ id: 'addons', name: 'Addons and UI', line: '', icon: '' }], projects: [
    P('mod:alpha', 'Alpha Bars'), P('mod:bravo', 'Bravo Bags'), P('mod:charlie', 'Charlie Frames'),
    // pending: listed with no download (the card has Get, never Install)
    P('mod:pending', 'Delta Pending', { download: null }),
    P('mod:realmy', 'Echo Realm Script', { kind: 'realm', category: 'realm-scripts' }),
    P('look:water', 'Clear Water HD', { source: 'look', category: 'graphics', look: { type: 'graphics-pack', how: 'launcher' } }),
    P('look:max', 'Max Settings', { source: 'look', category: 'graphics', download: null, look: { type: 'preset', how: 'preset' } }),
  ], from_cache: false, saved_at: 1, error: null };
  const copy = (x) => JSON.parse(JSON.stringify(x));
  const missing = new Set(o.missing || []);
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    if (missing.has(cmd)) throw `unknown command ${cmd}`;
    switch (cmd) {
      case 'get_state': return copy(st);
      case 'select': st.settings.selected_client = args.client; return null;
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'mods_catalog': return copy(catalog);
      case 'mods_ranked': return { ids: catalog.projects.map((p) => p.id), metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: copy(window.__mods[args.client] || []) };
      case 'mods_updates': return [];
      case 'mod_set_enabled': { const m = (window.__mods[args.client] || []).find((x) => x.key === args.key); if (m) m.enabled = args.on; return null; }
      case 'mod_install': {
        const list = (window.__mods[args.client] = window.__mods[args.client] || []);
        const p = catalog.projects.find((x) => x.id === args.project);
        const had = list.find((x) => x.key === args.project);
        if (had) { had.version = had.update || had.version; had.update = null; return Object.assign({}, had, { kept: [], warnings: [] }); }
        const m = { key: p.id, name: p.name, version: p.version, enabled: true, source: 'catalog', addons: [p.name.replace(/\W/g, '')], patches: [], natives: [], credits: '', warnings: [], native_code: false, update: null, in_catalog: true, files: [] };
        list.push(m);
        return Object.assign({}, m, { kept: [] });
      }
      case 'mod_uninstall': { window.__mods[args.client] = (window.__mods[args.client] || []).filter((x) => x.key !== args.key); return { removed_files: 3, kept: [], changed: [] }; }
      case 'looks_state': return { client: args.client, build: '3.3.5a', on: args.client === window.__o.sp ? window.__o.lookOn || [] : [], how: { 'look:water': 'pack', 'look:max': 'preset' }, changed: false };
      case 'looks_remove': window.__o.lookOn = (window.__o.lookOn || []).filter((x) => x !== args.look); return { said: 'Stopped.' };
      case 'addons_status': return { pack: { version: '2.4.0', addons: [{ name: 'Leyline', title: 'Leyline' }] }, from_cache: false, error: null, prefs: { auto_update: true, dont_ask: false },
        clients: st.settings.clients.map((c) => ({ client: c.id, name: c.name, build: c.build, supported: true, installed: window.__o.pack && c.id === window.__o.sp ? '2.3.0' : null,
          installed_at: null, update: !!(window.__o.pack && c.id === window.__o.sp), missing: [], foreign: [], backups: [] })) };
      case 'world_mods_get': return { kind: args.kind, mods: copy(window.__world), set_up: true, running: !!window.__ws[args.kind].running };
      case 'world_mods_set': {
        const m = window.__world.find((x) => x.id === args.id);
        if (!m || m.locked) throw '[other] That stays on.';
        m.on = args.on; m.mine = true;
        const running = !!window.__ws[args.kind].running;
        return { mods: copy(window.__world), restart: running, message: `${m.name} is ${args.on ? 'on' : 'off'}.` + (running ? ' Single player is running: it changes at its next start (Restart now, or the next time you play).' : ' It changes when your world starts.') };
      }
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
      case 'engine_status': return copy(window.__ws.single);
      case 'world_status': return copy(Object.assign({}, window.__ws[args.kind], { kind: args.kind }));
      case 'world_settings_get': return Object.assign({}, window.__o.set, { kind: args.kind, running: !!window.__ws[args.kind].running });
      case 'engine_progress': return { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7 };
      case 'world_resume_pending': return null;
      case 'world_stop': case 'engine_stop': { const k = cmd === 'engine_stop' ? 'single' : args.kind; window.__ws[k] = Object.assign({}, window.__ws[k], { running: false }); return 'Stopped.'; }
      case 'world_start': window.__ws[args.kind] = Object.assign({}, window.__ws[args.kind], { running: true }); return 'Starting…';
      case 'local_characters': return [];
      case 'den_net_state': return { running: false, connected: false, relay: '', realm_id: '', address: '', auth_port: 0, world_port: 0, streams: 0, last_error: '' };
      case 'open_url': case 'open_link': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });
async function open(o, view) {
  const opts = Object.assign({ clients: [SP, OTHER], sp: SP.id, selected: SP.id, worlds: { single: WS({}), vanilla: WS({}), bossrush: WS({}) }, set: SET,
    mods: {
      [SP.id]: [M('mod:alpha', 'Alpha Bars', { addons: ['AlphaBars'], update: '1.1.0' }), M('mod:bravo', 'Bravo Bags', { addons: ['BravoBags'], enabled: false }),
        M('look:water', 'Clear Water HD', { patches: ['patch-W.MPQ'] })],
      [OTHER.id]: [],
    }, world: WORLD, lookOn: ['look:water', 'look:max'], pack: true }, o);
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: (view && view.viewport) || { width: 1360, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineSPMods && window.LeylineWorlds && window.LeylineApp && window.LeylineApp.modsForSingle);
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const rowKeys = (page) => page.$$eval('#spm-list .spm-row', (l) => l.map((r) => r.dataset.key));
const rowOf = (key) => `#spm-list .spm-row[data-key="${key}"]`;
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;
async function single(page) {
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#spm-list .spm-row');
}
async function shot(page, name, o) {
  if (!SHOTS) return;
  await appLook(page, Object.assign({ root: o && o.root }, o || {}));
  if (o && o.scrollTo) await page.$eval(o.scrollTo, (e, block) => e.scrollIntoView({ block }), (o && o.block) || 'start');
  await page.waitForTimeout(300);
  const el = o && o.el ? await page.$(o.el) : null;
  if (el) await el.screenshot({ path: path.join(SHOTS, `spmods-${name}.png`) });
  else await page.screenshot({ path: path.join(SHOTS, `spmods-${name}.png`) });
}

test("the panel: after Your world's settings, before Friends; single player's folder, from disk", async () => {
  // the play bar is on another folder: the panel still shows single player's
  const { page, ctx, errors } = await open({ selected: OTHER.id });
  await single(page);
  const order = await page.$$eval('#page-single > .panel', (l) => l.map((p) => p.id));
  assert.ok(order.indexOf('oc-set') + 1 === order.indexOf('spm') && order.indexOf('spm') + 1 === order.indexOf('spf'), order.join(' '));
  assert.equal(await page.textContent('#spm-title'), 'Mods in this world');
  assert.equal(await page.textContent('#spm-folder'), 'Game folder: My Wrath');
  const keys = await rowKeys(page);
  assert.deepEqual(keys.filter((k) => !k.startsWith('w:')), ['m:mod:alpha', 'm:mod:bravo', 'm:look:water', 'l:look:max', 'a:pack']);
  assert.deepEqual(keys.filter((k) => k.startsWith('w:')), ['w:companions', 'w:transmog', 'w:aoeloot']);
  // the groups, the kinds
  const groups = await page.$$eval('#spm-list .lk-group', (l) => l.map((g) => g.textContent));
  assert.ok(groups.some((g) => /In your game folder/.test(g)) && groups.some((g) => /Built into your world/.test(g)), groups.join('|'));
  assert.equal(await page.textContent(`${rowOf('m:look:water')} .spm-tag`), 'HD pack');
  assert.equal(await page.textContent(`${rowOf('a:pack')} .spm-name`), 'Leyline add-ons');
  assert.ok(await page.$(`${rowOf('a:pack')} .spm-upd`), 'the add-ons: Update ready');
  // read from disk, for single player's folder only; nothing asked of the network
  assert.ok((await callsOf(page, 'mods_installed')).some((a) => a.client === SP.id));
  assert.ok((await callsOf(page, 'world_mods_get')).some((a) => a.kind === 'single'));
  // each row's buttons
  assert.ok(await page.$(`${rowOf('m:mod:alpha')} .spm-switch[aria-checked="true"]`));
  assert.ok(await page.$(`${rowOf('m:mod:alpha')} .spm-update`));
  assert.ok(await page.$(`${rowOf('m:mod:alpha')} .spm-remove`));
  assert.ok(await page.$(`${rowOf('m:mod:alpha')} .spm-page`), 'its page');
  assert.ok(await page.$(`${rowOf('m:mod:bravo')} .spm-switch[aria-checked="false"]`));
  assert.ok(await page.$(`${rowOf('m:mod:bravo')} .spm-offchip`));
  assert.equal(await page.$(`${rowOf('w:companions')} .spm-switch`), null, 'locked: no switch');
  assert.equal(await page.$(`${rowOf('w:transmog')} .spm-remove`), null, 'a world module is never removed');
  assert.doesNotMatch(await page.textContent('#spm'), BRANDS);
  await shot(page, 'page', { scrollTo: '#oc-set' });
  await shot(page, 'panel', { scrollTo: '#spm' });
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('On / Off, Update, Remove, Stop using, Page: the game folder single player plays with', async () => {
  const { page, ctx, errors } = await open({ selected: OTHER.id });
  await single(page);
  await page.click(`${rowOf('m:mod:bravo')} .spm-switch`);
  await page.waitForSelector(`${rowOf('m:mod:bravo')} .spm-switch[aria-checked="true"]`);
  assert.deepEqual(await callsOf(page, 'mod_set_enabled'), [{ client: SP.id, key: 'mod:bravo', on: true }]);
  assert.match(await page.textContent('#spm-status'), /Turned on Bravo Bags/);
  await page.click(`${rowOf('m:mod:alpha')} .spm-update`);
  await page.waitForFunction(() => !document.querySelector('#spm-list .spm-row[data-key="m:mod:alpha"] .spm-update'));
  assert.deepEqual(await callsOf(page, 'mod_install'), [{ client: SP.id, project: 'mod:alpha' }]);
  await page.click(`${rowOf('m:look:water')} .spm-remove`);
  await page.waitForFunction(() => !document.querySelector('#spm-list .spm-row[data-key="m:look:water"]'));
  assert.deepEqual(await callsOf(page, 'mod_uninstall'), [{ client: SP.id, key: 'look:water' }]);
  await page.click(`${rowOf('l:look:max')} .spm-remove`);
  await page.waitForFunction(() => !document.querySelector('#spm-list .spm-row[data-key="l:look:max"]'));
  assert.deepEqual(await callsOf(page, 'looks_remove'), [{ client: SP.id, look: 'look:max' }]);
  await page.click(`${rowOf('m:mod:alpha')} .spm-page`);
  assert.match((await callsOf(page, 'open_url')).pop().url, /alpha/);
  // the play bar's folder was never touched
  assert.equal(await page.evaluate(() => window.LeylineApp.state().settings.selected_client), OTHER.id);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test("the world's own modules: Off / On, and Restart now while it runs", async () => {
  const { page, ctx, errors } = await open({ worlds: { single: WS({ running: true }), vanilla: WS({}), bossrush: WS({}) } });
  await single(page);
  await page.click(`${rowOf('w:transmog')} .spm-switch`);
  await page.waitForSelector(`${rowOf('w:transmog')} .spm-switch[aria-checked="false"]`);
  assert.deepEqual(await callsOf(page, 'world_mods_set'), [{ kind: 'single', id: 'transmog', on: false }]);
  assert.match(await page.textContent('#spm-status'), /Transmogrification is off.*next start/);
  await page.waitForSelector('#spm-restart');
  await shot(page, 'restart', { scrollTo: '#spm-restart', block: 'center' });
  await page.click('#spm-restart');
  await page.waitForFunction(() => /Restarting with your mods/.test(document.getElementById('spm-status').textContent));
  const cmds = await page.evaluate(() => window.__calls.map(([c]) => c));
  assert.ok(cmds.includes('engine_stop') && cmds.lastIndexOf('world_start') > cmds.lastIndexOf('engine_stop'), 'stopped (saved), then started');
  assert.equal(await page.$('#spm-restart'), null);
  // the original world: its own list
  await page.selectOption('#spm-kind', 'vanilla');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'world_mods_get' && a.kind === 'vanilla'));
  // the World tab alone
  await page.click('#spm-tab-world');
  assert.ok((await rowKeys(page)).every((k) => k.startsWith('w:')));
  await page.click('#spm-tab-all');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('nothing yet: No mods yet, and Browse the mod store opens Discover on single player\'s folder', async () => {
  const { page, ctx, errors } = await open({ selected: OTHER.id, mods: { [SP.id]: [], [OTHER.id]: [] }, lookOn: [], pack: false });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#spm-empty');
  assert.match(await page.textContent('#spm-empty'), /No mods yet/);
  assert.equal(await page.textContent('#spm-browse'), 'Browse the mod store');
  // the Game folder tab says the same
  await page.click('#spm-tab-folder');
  assert.match(await page.textContent('#spm-list'), /No mods yet/);
  await page.click('#spm-tab-all');
  await shot(page, 'empty', { scrollTo: '#spm' });
  await page.click('#spm-browse');
  await page.waitForSelector('#page-mods.on #pane-discover.on');
  assert.equal(await page.evaluate(() => window.LeylineApp.state().settings.selected_client), SP.id, "single player's folder is picked");
  assert.ok((await callsOf(page, 'select')).some((a) => a.client === SP.id));
  await page.waitForSelector('#dc-sp:not([hidden])');
  assert.match(await page.textContent('#dc-sp-text'), /Picking mods for single player: they go into My Wrath/);
  // Works with this game folder is on
  const fit = await page.$$eval('#dc-kbar [aria-pressed="true"], #dc-kbar .on, #dc-kbar [aria-checked="true"]', (l) => l.map((x) => x.textContent).join('|'));
  assert.match(fit, /Works with this game folder/);
  // install from the store's own card: the bar says so; back on Single player it's listed
  await page.waitForSelector('#cat-grid .mod-card[data-id="mod:charlie"] .acts .btn.primary');
  await page.click('#cat-grid .mod-card[data-id="mod:charlie"] .acts .btn.primary');
  await page.waitForFunction(() => /Added Charlie Frames to single player/.test(document.getElementById('dc-sp-text').textContent));
  assert.deepEqual((await callsOf(page, 'mod_install')).map((a) => a.client), [SP.id]);
  await shot(page, 'discover', { scrollTo: '#pane-discover' });
  await page.click('#dc-sp-back');
  await page.waitForSelector('#page-single.on');
  await page.waitForSelector(rowOf('m:mod:charlie'));
  assert.equal(await page.$('#spm-empty'), null);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Add to single player on a card for another folder; never on a card with no Install', async () => {
  const { page, ctx, errors } = await open({ selected: OTHER.id });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector(rowOf('m:mod:alpha'));
  await page.evaluate(() => window.LeylineNav.show('mods', false, 'discover'));
  await page.waitForSelector('#cat-grid .mod-card[data-id="mod:charlie"]');
  // Works with this game folder off: every card shows
  await page.evaluate(() => { const b = [...document.querySelectorAll('#dc-kbar button')].find((x) => /Works with this game folder/.test(x.textContent)); if (b && b.getAttribute('aria-pressed') === 'true') b.click(); });
  const btnOf = (id) => page.$(`#cat-grid .mod-card[data-id="${id}"] .dc-sp-add`);
  await page.waitForSelector('#cat-grid .mod-card[data-id="mod:charlie"] .dc-sp-add');
  assert.equal(await page.textContent('#cat-grid .mod-card[data-id="mod:charlie"] .dc-sp-add'), 'Add to single player');
  // already in single player's folder: says so
  assert.equal(await page.textContent('#cat-grid .mod-card[data-id="mod:alpha"] .dc-sp-add'), 'In single player ✓');
  assert.equal(await page.$eval('#cat-grid .mod-card[data-id="mod:alpha"] .dc-sp-add', (b) => b.disabled), true);
  // pending (no download) and a realm mod: no button
  assert.equal(await btnOf('mod:pending'), null);
  assert.equal(await btnOf('mod:realmy'), null);
  await page.click('#cat-grid .mod-card[data-id="mod:charlie"] .dc-sp-add');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'mod_install'));
  assert.deepEqual(await callsOf(page, 'mod_install'), [{ client: SP.id, project: 'mod:charlie' }]);
  await page.waitForSelector('#dc-sp:not([hidden])');
  await page.waitForFunction(() => /Added Charlie Frames/.test(document.getElementById('dc-sp-text').textContent));
  // single player's folder is the picked one now: no more Add buttons (Install is the card's own)
  assert.equal(await page.$('#cat-grid .dc-sp-add'), null);
  await page.click('#dc-sp-back');
  await page.waitForSelector(rowOf('m:mod:charlie'));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('an older core without world mods; no game folder at all', async () => {
  let { page, ctx, errors } = await open({ missing: ['world_mods_get', 'world_mods_set'] });
  await single(page);
  assert.ok((await rowKeys(page)).every((k) => !k.startsWith('w:')));
  assert.match(await page.textContent('#spm'), /newer Leyline/);
  assert.equal(await page.$('#spm-kind'), null);
  assert.deepEqual(errors, []);
  await ctx.close();
  ({ page, ctx, errors } = await open({ clients: [], selected: null, mods: {} }));
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#spm-empty');
  assert.match(await page.textContent('#spm-empty'), /No game folder yet/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the "i" bubble on Single player mentions mods, and the panel has its own', async () => {
  const { page, ctx } = await open({});
  await single(page);
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.click('#ll-info');
  await page.waitForSelector('#ll-info-b:not([hidden])');
  const t = await page.textContent('#ll-info-b');
  assert.match(t, /Mods in this world/);
  assert.match(t, /Get mods for single player/);
  assert.equal(await page.$$eval('#ll-info-b p', (l) => l.length), 4);
  await page.keyboard.press('Escape');
  // focus inside the panel: its own bubble
  await page.focus(`${rowOf('m:mod:alpha')} .spm-update`);
  await page.click('#ll-info');
  await page.waitForSelector('#ll-info-b:not([hidden])');
  assert.equal(await page.textContent('#ll-info-t'), 'Mods in this world');
  await ctx.close();
});

test('full app look: Dark and Light, and a narrow window', async () => {
  if (!SHOTS) return;
  for (const light of [false, true]) {
    const { page, ctx } = await open({ worlds: { single: WS({ running: true }), vanilla: WS({}), bossrush: WS({}) } });
    await single(page);
    await shot(page, light ? 'panel-light' : 'panel-dark', { scrollTo: '#spm', light });
    await ctx.close();
  }
  // a narrow window (the phone app has no Single player page: it needs a computer)
  const { page, ctx } = await open({}, { viewport: { width: 900, height: 900 } });
  await single(page);
  await shot(page, 'narrow', { scrollTo: '#spm' });
  await ctx.close();
});
