// List your mod (ui/listmod.js, src-tauri/src/listing.rs, listed.rs) in Chromium with a fake Tauri core: Mods ›
// Installed has "List your mod"; signed in, a pasted link is checked by the site and the dialog shows exactly what one
// click installs (or why it can't), then lists it (live or waiting for a moderator); a listed mod's card says
// "Community · not reviewed", installs with one click through mod_install, and has Report with reasons.
// Screenshots go to $LISTMOD_SHOTS when set.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/listmod.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const SHOTS = process.env.LISTMOD_SHOTS || '';
const LISTED = { id: 'link:3f9a0c12bd47', source: 'listed', name: 'Frames', description: 'Unit frames, rebuilt.', category: 'addons',
  builds: ['3.3.5a'], author: 'alice', author_url: '', licence: 'MIT', repo: 'https://github.com/alice/Frames',
  page: 'https://play.openvanilla.app/wrath/mods/p/3f9a0c12bd47', credits: 'Frames by alice', stars: null, last_push: '',
  version: '1.2', kind: null, download: null, sha256: null, size: 52, format: 'addon', state: 'community', flags: [],
  plan: ['Downloads it from the author.', 'Adds 1 addon folder to Interface/AddOns: Frames.'], native: false,
  install: { kind: 'addon', from: 'commit-archive', url: 'https://github.com/alice/Frames/archive/x.zip', rev: 'x', files: {} } };
const CATALOG = { categories: [{ id: 'addons', name: 'Addons and UI' }], projects: [LISTED], from_cache: false, saved_at: 1, error: null };
const CHECK_OK = { ok: true, format: 'addon', label: 'Addon', client: true, problems: [], flags: [], notes: ['2 file(s) the game does not load are left out'],
  plan: ['Downloads Frames.zip from the author\'s own repo (pinned to abc1234, 4 KB), checks every file\'s fingerprint and installs only those 2 file(s).',
    'Adds 1 addon folder to Interface/AddOns: Frames.'], name: 'Frames', version: '1.2', licence: 'MIT', repo: 'https://github.com/alice/Frames',
  token: '0123456789abcdef0123456789abcdef', files: 2 };
const CHECK_BAD = { ok: false, format: 'addon', label: 'Addon', client: true, problems: ['Frames/m.m2: .m2 files are never allowed'], flags: [], notes: [],
  plan: [], name: 'Frames', token: 'ffffffffffffffffffffffffffffffff' };

const FAKE = ({ catalog, signed }) => {
  const calls = (window.__calls = []);
  try { localStorage.setItem('leyline.page', 'mods'); } catch (_) { /* fine */ }
  let mods = [];
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c1', name: 'My Wrath', path: '/x', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c1', selected_realm: 'play.openvanilla.app', wine: { enabled: true, command: 'wine', prefix: '' }, count_installs: true },
        official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: false, data_dir: '/tmp' };
      case 'social_status': return signed;
      case 'social_ways': return { herokey: true, google: true, discord: false };
      case 'social_call':
        if (args.action === 'me') { if (!signed) throw 'signed_out'; return { me: { pid: 'aaaaaaaaaa', name: 'Alice', tag: '4821', main: '', offline: false } }; }
        throw 'signed_out';
      case 'listing_post':
        if (args.action === 'check') return { ok: true, check: /models/.test(args.args.url) ? window.__bad : window.__ok };
        if (args.action === 'submit') return { ok: true, id: 'link:aaaaaaaaaaaa', pid: 'aaaaaaaaaaaa', state: 'live', why: [] };
        if (args.action === 'report') return { ok: true, message: 'Thanks: the admins will take a look.' };
        if (args.action === 'remove') return { ok: true, message: 'Removed.' };
        throw 'unknown listing action';
      case 'listing_mine': return { ok: true, listings: [{ id: 'link:aaaaaaaaaaaa', pid: 'aaaaaaaaaaaa', name: 'Frames', format: 'addon',
        state: 'live · community, not reviewed', why: [], update: false }] };
      case 'listing_open': return null;
      case 'mods_catalog': return catalog;
      case 'mods_ranked': throw 'offline';
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods };
      case 'mod_install': {
        const p = catalog.projects.find((x) => x.id === args.project);
        mods = mods.concat([{ key: p.id, name: p.name, version: p.version, source: 'listed', update: null, in_catalog: true, enabled: true, files: [], addons: ['Frames'], patches: [] }]);
        return { key: p.id, name: p.name, version: p.version, warnings: [], kept: [] };
      }
      case 'deep_link_take': return null;
      case 'mods_updates': return [];
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'host_info': return { dir: '', is_kit: false, problems: [], kit: null, seeds: [], realm: null };
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'addons_status': return { pack: { version: '1', addons: [] }, from_cache: false, error: null, prefs: { auto_update: true, dont_ask: false },
        clients: [{ client: 'c1', installed: true, update: false, missing: [] }] };
      case 'addons_should_offer': return false;
      default: return null;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(opts = {}) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, Object.assign({ catalog: CATALOG, signed: true }, opts));
  await page.addInitScript(([ok, bad]) => { window.__ok = ok; window.__bad = bad; }, [CHECK_OK, CHECK_BAD]);
  await page.goto(INDEX);
  await page.waitForFunction(() => !!document.getElementById('mods-list-yours') && !!window.LeylineSocial);
  return { page, errors };
}
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, name + '.png') }); };
const calls = (page, name) => page.evaluate((n) => window.__calls.filter(([c]) => c === n).map(([, a]) => a), name);

test('List your mod: check a link, see what one click installs, list it', async () => {
  const { page, errors } = await open();
  await page.evaluate(() => window.LeylineSocial.refresh());
  await page.click('.nav[data-page="mods"]');
  await page.click('#tab-mods-installed');
  await page.click('#mods-list-yours');
  await page.waitForSelector('#lm-dlg[open] #lm-url');
  await page.fill('#lm-url', 'https://github.com/alice/Frames');
  await page.click('#lm-dlg .btn.primary');
  await page.waitForSelector('#lm-dlg .lm-plan li');
  const plan = await page.$$eval('#lm-dlg .lm-plan li', (l) => l.map((x) => x.textContent));
  assert.match(plan[0], /fingerprint/);
  assert.match(plan[1], /Interface\/AddOns: Frames/);
  // Addon or mod (docs/WORDS.md): Addon first, then Mod with the Mods tab's categories; an addon folder starts on Addon
  assert.equal(await page.textContent('label[for="lm-cat"]'), 'Addon or mod');
  assert.deepEqual(await page.$$eval('#lm-cat optgroup', (g) => g.map((x) => x.label)), ['Addon: an in-game interface helper', 'Mod: changes the game itself']);
  assert.deepEqual(await page.$$eval('#lm-cat optgroup:first-child option', (o) => o.map((x) => x.value)), ['addons']);
  assert.deepEqual(await page.$$eval('#lm-cat optgroup:last-child option', (o) => o.map((x) => x.value)),
    ['sandbox', 'models', 'realm-scripts', 'graphics', 'audio', 'maps', 'tools'], 'in the Mods tab\'s order');
  assert.equal(await page.inputValue('#lm-cat'), 'addons');
  await shot(page, 'listmod-check');
  await page.fill('#lm-desc', 'Unit frames, rebuilt for raids.');
  await page.click('#lm-dlg button:text-is("List it")');
  await page.waitForSelector('#lm-dlg .lm-ok');
  assert.match(await page.textContent('#lm-dlg .lm-ok'), /Community · not reviewed/);
  const sub = (await calls(page, 'listing_post')).find((a) => a.action === 'submit');
  assert.deepEqual(sub.args, { t: CHECK_OK.token, category: 'addons', description: 'Unit frames, rebuilt for raids.' });
  await shot(page, 'listmod-listed');
  assert.deepEqual(errors, []);
  await page.close();
});

test('a refused check says why and offers no List it', async () => {
  const { page, errors } = await open();
  await page.evaluate(() => window.LeylineSocial.refresh());
  await page.evaluate(() => window.LeylineListMod.open());
  await page.fill('#lm-url', 'https://github.com/ld/models');
  await page.click('#lm-dlg .btn.primary');
  await page.waitForSelector('#lm-dlg .lm-bad');
  assert.match(await page.textContent('#lm-dlg .lm-bad'), /\.m2/);
  assert.equal(await page.$('#lm-desc'), null);
  assert.deepEqual(errors, []);
  await page.close();
});

test('signed out: sign in or open the website, nothing is sent', async () => {
  const { page, errors } = await open({ signed: false });
  await page.evaluate(() => window.LeylineListMod.open());
  await page.waitForSelector('#lm-dlg[open]');
  assert.match(await page.textContent('#lm-dlg'), /Sign in to list a mod/);
  assert.equal((await calls(page, 'listing_post')).length, 0);
  assert.deepEqual(errors, []);
  await page.close();
});

test('a listed mod: Community · not reviewed, one-click Install, Report with reasons', async () => {
  const { page, errors } = await open();
  await page.evaluate(() => window.LeylineSocial.refresh());
  await page.click('.nav[data-page="mods"]');
  await page.click('#tab-mods-discover');
  // the card as Discover draws it (projectCard), whichever view is showing
  const html = await page.evaluate((p) => { const c = projectCard(p, { id: 'c1', name: 'My Wrath', build: '3.3.5a' }); document.body.append(c); c.id = 'lm-test-card'; return c.outerHTML; }, LISTED);
  assert.match(html, /Community · not reviewed/);
  await page.click('#lm-test-card .acts .btn.primary');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'mod_install'));
  assert.deepEqual((await calls(page, 'mod_install'))[0], { client: 'c1', project: LISTED.id });
  await page.click('#lm-test-card .acts button:text-is("Report")');
  await page.waitForSelector('#lm-dlg[open] #lm-reason');
  await page.selectOption('#lm-reason', 'malware');
  await page.click('#lm-dlg button:text-is("Send report")');
  await page.waitForFunction(() => /Thanks/.test(document.querySelector('#lm-dlg .lm-status').textContent));
  assert.deepEqual((await calls(page, 'listing_post')).find((a) => a.action === 'report').args, { id: LISTED.id, reason: 'malware', note: '' });
  assert.deepEqual(errors, []);
  await page.close();
});
