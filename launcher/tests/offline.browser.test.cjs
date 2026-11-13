// The launcher when the network is slow, down or gone, and the smaller fixes from the 12 Nov audit, in Chromium with a
// fake Tauri core: the kept realm and mods lists show at once while the network answers (realms_list_cached,
// mods_catalog_cached); the play bar says the realm's real state (offline, partly up, no answer) instead of a green
// READY; the realm check pauses while the window is hidden; the title bar's search finds pages and features before
// mods; News says when it is a kept copy, and the built-in notes carry no version numbers; Settings' privacy line
// names everything Leyline fetches; Home's rows show a focus ring; Host is on the stone.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/offline.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const OFFICIAL = { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore',
  tags: [], players: 5, bots: 0, address: 'leylinelauncher.com' };
const COMM = { slug: 'c-bbbbbbbbbbbbbbbb', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Kept Realm', official: false, state: 'live', build: '3.3.5a', version: '3.3.5a',
  core: 'azerothcore', tags: [], players: 3, bots: 0, address: 'relay.leylinelauncher.com:40002', group: 'community' };
const P = { id: 'mod:alpha', source: 'packaged', name: 'Alpha Kept', description: 'Alpha does things.', category: 'addons', builds: ['3.3.5a'],
  author: 'Someone', author_url: '', licence: 'MIT', repo: '', page: '', credits: 'Alpha', stars: null, last_push: '', version: '1.0.0', kind: 'ui',
  permissions: [], on_wrath: false, download: 'https://leylinelauncher.com/wrath/mods/alpha/download', sha256: 'ab'.repeat(32), size: 1000 };

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.setItem('leyline.page', o.page || 'play'); localStorage.setItem('leyline.mode', 'online'); localStorage.setItem('leyline.dcView', 'all'); } catch (_) { /* fine */ }
  window.__status = o.status || 'online';
  window.__slow = o.slow || 0;
  const wait = () => new Promise((r) => setTimeout(r, window.__slow));
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c1', name: 'My Wrath', path: '/x', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c1', selected_realm: 'leylinelauncher.com', wine: { enabled: false, command: 'wine', prefix: '' } },
        official_realm: 'leylinelauncher.com', browser_url: 'https://leylinelauncher.com/wrath', windows: true, data_dir: '/tmp' };
      case 'realms_list_cached': return o.kept ? { realms: [o.kept.official, o.kept.comm], network: { realms: 1, players: 8, bots: 0 }, from_cache: true, saved_at: 1762900000, error: null } : null;
      case 'realms_list': await wait(); return { realms: [o.official], network: { realms: 0, players: 5, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status':
        if (window.__status === 'throw') throw 'timed out';
        return { state: window.__status, ping_ms: 10, players: 5 };
      case 'mods_catalog_cached': return o.kept ? { categories: [{ id: 'addons', name: 'Addons' }], projects: [o.kept.mod], from_cache: true, saved_at: 1762900000, error: null } : null;
      case 'mods_catalog': await wait(); return { categories: [{ id: 'addons', name: 'Addons' }], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c1', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'deep_link_take': return null;
      case 'news_list': if (o.news === 'throw') throw 'Offline and nothing cached yet.'; return o.news || { posts: [], from_cache: false, saved_at: null };
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'social_status': return true;
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(o = {}) {
  const page = await browser.newPage({ viewport: { width: 1360, height: 860 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route(/^https:\/\//, (r) => r.abort());
  await page.addInitScript(FAKE, Object.assign({ official: OFFICIAL }, o));
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineApp && window.LeylineApp.state());
  return { page, errors };
}
const head = (page) => page.textContent('#play-headline');

test('the kept realm and mods lists show at once while a slow network answers', async () => {
  const { page, errors } = await open({ slow: 5000, page: 'realms', kept: { official: OFFICIAL, comm: COMM, mod: P } });
  await page.waitForFunction(() => window.LeylineApp.realms().some((r) => r.name === 'Kept Realm'), null, { timeout: 2500 });
  await page.waitForFunction(() => window.LeylineApp.catalog().projects.some((p) => p.name === 'Alpha Kept'), null, { timeout: 2500 });
  // the network's answer replaces them
  await page.waitForFunction(() => !window.LeylineApp.realms().some((r) => r.name === 'Kept Realm'), null, { timeout: 9000 });
  assert.deepEqual(errors, []);
  await page.close();
});

test('the play bar says the realm is down or did not answer; online it is READY again', async () => {
  const { page, errors } = await open({ status: 'offline' });
  await page.waitForFunction(() => /OFFLINE/.test(document.getElementById('play-headline').textContent));
  assert.equal(await head(page), 'OFFLINE · WORLD SERVER DOWN');
  assert.equal(await page.evaluate(() => document.getElementById('play-headline').parentElement.classList.contains('warn')), true, 'the warn look, not green');
  assert.equal(await page.isEnabled('#play-go'), true, 'PLAY still works');
  await page.evaluate(() => { window.__status = 'throw'; });
  await page.evaluate(() => window.LeylineApp.refresh());
  await page.waitForFunction(() => /CAN'T CHECK/.test(document.getElementById('play-headline').textContent));
  assert.match(await page.textContent('#pb-detail'), /You may be offline/);
  await page.evaluate(() => { window.__status = 'partial'; });
  await page.evaluate(() => window.LeylineApp.refresh());
  await page.waitForFunction(() => /PARTLY UP/.test(document.getElementById('play-headline').textContent));
  await page.evaluate(() => { window.__status = 'online'; });
  await page.evaluate(() => window.LeylineApp.refresh());
  await page.waitForFunction(() => document.getElementById('play-headline').textContent === 'READY TO PLAY');
  assert.deepEqual(errors, []);
  await page.close();
});

test('the realm check waits while the window is hidden', async () => {
  const page = await browser.newPage();
  await page.clock.install();
  await page.route(/^https:\/\//, (r) => r.abort());
  await page.addInitScript(FAKE, { official: OFFICIAL });
  await page.goto(INDEX);
  await page.clock.runFor(3000);
  const n = () => page.evaluate(() => window.__calls.filter(([c]) => c === 'realm_status').length);
  const a = await n();
  await page.clock.runFor(61000);
  const b = await n();
  assert.ok(b > a, 'every minute while shown');
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, get: () => true }); document.dispatchEvent(new Event('visibilitychange')); });
  await page.clock.runFor(185000);
  assert.equal(await n(), b, 'nothing while hidden');
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, get: () => false }); document.dispatchEvent(new Event('visibilitychange')); });
  await page.clock.runFor(500);
  assert.ok(await n() > b, 'back in view: at once');
  await page.close();
});

test('the title bar search finds pages and features before mods', async () => {
  const { page, errors } = await open();
  const want = { 'single player': 'single', 'Den': 'den', 'friends': 'single', 'boss rush': 'bossrush', 'leaderboards': 'modes', 'host': 'host',
    'settings': 'settings', 'hero key': 'account', 'game folders': 'clients', 'bos': 'bossrush', 'mods': 'mods' };
  for (const [q, p] of Object.entries(want)) {
    const f = await page.evaluate((q) => window.LeylineApp.featureFor(q.toLowerCase()), q);
    assert.ok(f && f.page === p, `${q} → ${p} (${JSON.stringify(f)})`);
  }
  for (const q of ['bagger', 'xyzzy', 'ai helper']) assert.equal(await page.evaluate((q) => window.LeylineApp.featureFor(q), q), null, q);
  await page.fill('#search', 'Boss Rush');
  await page.press('#search', 'Enter');
  await page.waitForFunction(() => document.querySelector('.page.on') && document.querySelector('.page.on').id === 'page-bossrush');
  await page.fill('#search', 'bagger');
  await page.press('#search', 'Enter');
  await page.waitForFunction(() => document.querySelector('.page.on').id === 'page-mods');
  assert.equal(await page.inputValue('#cat-q'), 'bagger', 'no feature: the mods search as before');
  assert.equal(await page.getAttribute('#search', 'placeholder'), 'Search pages, mods and realms');
  assert.deepEqual(errors.filter((e) => !/unknown /.test(e)), []);
  await page.close();
});

test('News: offline it says the date of the kept copy; never loaded, the built-in notes say so and carry no version numbers', async () => {
  let { page } = await open({ page: 'news', news: { posts: [{ tag: 'LAUNCHER', date: '2026-11-10', title: 'KEPT POST', items: ['x'] }], from_cache: true, saved_at: 1762900000 } });
  await page.waitForFunction(() => /KEPT POST/.test(document.getElementById('news-full').textContent));
  assert.match(await page.textContent('#news-status'), /^Offline: the news as of /);
  await page.close();
  ({ page } = await open({ page: 'news', news: 'throw' }));
  await page.waitForFunction(() => document.getElementById('news-status').textContent.length > 0);
  assert.match(await page.textContent('#news-status'), /appears when you're online/);
  const tags = await page.evaluate(() => window.LeylineApp.news().map((n) => n.tag).join(' '));
  assert.doesNotMatch(tags, /\d/, 'no version numbers in the built-in notes');
  await page.close();
  ({ page } = await open({ page: 'news', news: { posts: [{ tag: 'LAUNCHER', date: '2026-11-10', title: 'FRESH', items: [] }], from_cache: false, saved_at: 1 } }));
  await page.waitForFunction(() => /FRESH/.test(document.getElementById('news-full').textContent));
  assert.equal(await page.textContent('#news-status'), '');
  await page.close();
});

test('privacy names what Leyline fetches; Home rows show a focus ring; Host is on the stone', async () => {
  const { page } = await open();
  const priv = await page.evaluate(() => [...document.querySelectorAll('#page-settings h3')].find((h) => h.textContent === 'Privacy').nextElementSibling.textContent);
  for (const w of ['news', 'forum', 'modes', 'leaderboards', 'melty.gg', 'relay', 'backdrop']) assert.match(priv, new RegExp(w), w);
  const css = require('node:fs').readFileSync(path.join(__dirname, '..', 'ui', 'home.css'), 'utf8');
  assert.match(css, /\.hs-rl:focus-visible \{ outline: 2px solid var\(--t-accent\)/);
  assert.match(css, /\.hs-side-a:focus-visible \{ outline: 2px solid/);
  await page.evaluate(() => window.LeylineNav.show('host'));
  const slate = await page.evaluate(() => getComputedStyle(document.getElementById('page-host')).getPropertyValue('--hc-glow').trim());
  assert.equal(slate, 'none', 'no blue and violet glow on Host');
  await page.close();
});
