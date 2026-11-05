// The official realm's line in the launcher (docs/QUEUE.md; ui/app.js queueN / renderQueueCard): when the website's
// status (realm_status from status.json) says people are waiting, the realm row and PLAY say "Busy · queue N" for the
// mode you play in (the realm's login queue online, the browser seats' line in the browser), and a small card offers
// the other ways in: Host your own realm, Become a node (and, in the browser, your own game folder). No line: none of it.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/queue.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');

const FAKE = ({ status, browser }) => {
  const calls = (window.__calls = []);
  try { localStorage.removeItem('leyline.page'); localStorage.setItem('leyline.mode', browser ? 'browser' : 'online'); } catch (_) { /* fine */ }
  const list = [{ id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' }];
  const st = { version: 'test', settings: { clients: list, selected_client: 'c335a-1', selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  window.__status = status;
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'select': return null;
      case 'realms_list': return { realms: [{ slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a',
        core: 'azerothcore', tags: [], players: 250, bots: 40, address: 'play.openvanilla.app' }], network: { realms: 0, players: 250, bots: 40 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return window.__status;
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return true;   // a sign-in kept: the sign-in screen stays closed
      case 'open_link': return null;
      case 'pick_folder': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(opts) {
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: 1400, height: 860 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && document.querySelector('#page-play.on') && window.__calls.some(([c]) => c === 'realm_status'));
  await page.waitForTimeout(150);
  return { page, ctx, errors };
}
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;

test('no line: no badge, no card, READY', async () => {
  const { page, ctx, errors } = await open({ status: { state: 'online', ping_ms: 10, players: 120, browser_waiting: 0 } });
  assert.equal(await page.$('.realm-row .rr-busy'), null);
  assert.equal(await page.isVisible('#pb-queue'), false);
  assert.match(await page.textContent('#play-headline'), /^READY/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the realm has a login queue: Busy · queue N on the row and PLAY, and the skip-the-line card', async () => {
  const { page, ctx, errors } = await open({ status: { state: 'online', ping_ms: 10, players: 250, queue: 12, browser_waiting: 3, browser_wait_min: 9 } });
  assert.equal(await page.textContent('.realm-row .rr-busy'), 'Busy · queue 12', 'online play: the realm\'s own queue');
  assert.equal(await page.textContent('#play-headline'), 'BUSY · QUEUE 12');
  assert.equal(await page.textContent('#pb-detail'), 'Or skip the line');
  assert.ok(await page.isVisible('#pb-queue'));
  assert.match(await page.textContent('#pbq-line'), /12 waiting to get into Wrath Unleashed/);
  assert.ok(await page.isVisible('#pbq-host') && await page.isVisible('#pbq-node'));
  assert.equal(await page.isVisible('#pbq-pc'), false, 'online already uses your game folder');
  await page.click('#pbq-node');
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-host', 'Become a node opens the Host page\'s Be a node card');
  await page.click('#pbq-host');
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-host', 'Host your own realm opens Host');
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), BRANDS);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the browser seats have a line: the browser mode shows it, with the wait and your own game folder', async () => {
  const { page, ctx, errors } = await open({ browser: true, status: { state: 'online', ping_ms: 10, players: 250, browser_waiting: 7, browser_wait_min: 12 } });
  assert.equal(await page.textContent('#play-headline'), 'BUSY · QUEUE 7');
  assert.equal(await page.textContent('#pb-detail'), 'About 12 min · or skip the line');
  assert.equal(await page.textContent('.realm-row .rr-busy'), 'Busy · queue 7');
  assert.match(await page.textContent('#pbq-line'), /7 waiting for a browser seat, about 12 min/);
  assert.ok(await page.isVisible('#pbq-pc'));
  await page.click('#pbq-pc');
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.mode')), 'online', 'your game folder: no browser seat needed');
  assert.equal(await page.isVisible('#pb-queue'), false, 'online with no realm queue: the card goes');
  assert.equal(await page.$('.realm-row .rr-busy'), null, 'and the row\'s badge');
  assert.match(await page.textContent('#play-headline'), /^READY/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Not now shuts the card; PLAY still says busy', async () => {
  const { page, ctx, errors } = await open({ status: { state: 'online', ping_ms: 10, players: 250, queue: 4 } });
  await page.screenshot({ path: process.env.QUEUE_SHOT || '/tmp/queue-card.png' });
  await page.click('#pbq-x');
  assert.equal(await page.isVisible('#pb-queue'), false);
  assert.equal(await page.textContent('#play-headline'), 'BUSY · QUEUE 4');
  assert.deepEqual(errors, []);
  await ctx.close();
});
