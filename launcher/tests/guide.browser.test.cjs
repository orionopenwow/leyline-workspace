// The guide (ui/guide.js, ui/guide.css; src-tauri settings.rs Settings::seen, app.rs guide_seen) in Chromium with a fake
// Tauri core: the first-run tour (3 or 4 steps on the real controls, Find your game files only with no game folder;
// Skip, Esc, Done; kept in the core's settings, never localStorage), Help › Show me again and Start here, nothing at all
// with an older core, and the one-time encouragement toasts (each once, one at a time, never over the tour; the
// hosted one copies the invite link). No game or publisher names.
//
//   node launcher/tests/guide.browser.test.cjs          GUIDE_SHOTS=<dir> also writes screenshots
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;
const SHOTS = process.env.GUIDE_SHOTS || '';

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.page', o.page || 'play'); } catch (_) { /* fine */ }
  const clients = o.clients ? [{ id: 'c335a-1', name: 'My Game', path: '/x', build: '3.3.5a', realmlist: '' }] : [];
  const settings = { clients, selected_client: clients[0] ? 'c335a-1' : null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } };
  if (o.seen) settings.seen = o.seen.slice();
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: JSON.parse(JSON.stringify(settings)), official_realm: 'play.openvanilla.app',
        browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'guide_seen': if (!/^[a-z0-9-]{1,32}$/.test(args.key)) throw 'That is not a guide step.'; if (settings.seen && !settings.seen.includes(args.key)) settings.seen.push(args.key); return null;
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 0 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c335a-1', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return false;
      case 'den_state': return { lite: { available: true, running: false }, kit: false, docker_den: false, dens: [], packs: [] };
      case 'forge_state': return { installed: false, running: false, last: null };
      case 'host_invite': return 'https://relay.openvanilla.app/join/abcdefghijk2345a#INVITECODE1234567890';
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts, theme) {
  const ctx = await browser.newContext({ colorScheme: theme || 'dark', viewport: { width: 1280, height: 800 } });
  await ctx.grantPermissions(['clipboard-read', 'clipboard-write']);
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, opts || {});
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineGuide && window.LeylineApp.state());
  return { page, ctx, errors };
}
const seenCalls = (page) => page.evaluate(() => window.__calls.filter((x) => x[0] === 'guide_seen').map((x) => x[1].key));
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, name + '.png') }); };
// the ring sits on the step's control
async function ringOn(page, sel) {
  await page.waitForTimeout(260);   // the ring glides to the next control
  const [ring, el] = await Promise.all([page.locator('#gd-ring').boundingBox(), page.locator(sel).first().boundingBox()]);
  assert.ok(ring && el, sel);
  assert.ok(ring.x <= el.x && ring.y <= el.y && ring.x + ring.width >= el.x + el.width && ring.y + ring.height >= el.y + el.height, sel);
}
async function inView(page) {
  const b = await page.locator('#gd-tour').boundingBox();
  const vp = page.viewportSize();
  assert.ok(b.x >= 0 && b.y >= 0 && b.x + b.width <= vp.width && b.y + b.height <= vp.height, 'the card is on screen');
}

test('first run with no game folder: four steps on the real controls, then Done keeps it in the settings', async () => {
  const { page, ctx, errors } = await open({ seen: [] });
  await page.waitForSelector('#gd-tour');
  assert.equal(await page.evaluate(() => document.activeElement.id), 'gd-tour-next', 'Next has the focus');
  const want = [['Play Wrath Unleashed', '#play-go'], ['Your Den', '.nav-den'], ['Host a server', '.side .nav[data-page="host"]'],
    ['Find your game files', '#pb-client']];
  for (let i = 0; i < want.length; i++) {
    assert.equal(await page.textContent('#gd-tour-title'), want[i][0]);
    assert.equal(await page.textContent('#gd-tour-n'), `${i + 1} of 4`);
    assert.ok((await page.textContent('#gd-tour-text')).length < 160, 'short');
    await ringOn(page, want[i][1]);
    await inView(page);
    await shot(page, `tour-${i + 1}`);
    assert.equal((await page.textContent('#gd-tour-next')).trim(), i === 3 ? 'Done' : 'Next');
    await page.click('#gd-tour-next');
  }
  assert.equal(await page.locator('#gd-tour, #gd-ring').count(), 0);
  assert.deepEqual(await seenCalls(page), ['tour']);
  assert.equal(await page.evaluate(() => Object.keys(localStorage).filter((k) => /guide|tour/i.test(k)).length), 0, 'not in localStorage');
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), BRANDS);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('with a game folder: three steps (no Find your game files); Skip and Esc both end it once', async () => {
  const { page, ctx, errors } = await open({ seen: [], clients: true });
  await page.waitForSelector('#gd-tour');
  assert.equal(await page.textContent('#gd-tour-n'), '1 of 3');
  await page.click('#gd-tour-next');
  await page.click('#gd-tour-next');
  assert.equal(await page.textContent('#gd-tour-title'), 'Host a server');
  assert.equal((await page.textContent('#gd-tour-next')).trim(), 'Done');
  await page.click('#gd-tour-skip');
  assert.equal(await page.locator('#gd-tour').count(), 0);
  // Help › Show me again, from another page: back on Home, Esc ends it
  await page.click('.side .nav[data-page="help"]');
  assert.equal(await page.locator('#page-help #guide-again').count(), 1);
  assert.equal((await page.textContent('#page-help #guide-again')).trim(), 'Show me again');
  assert.equal(await page.locator('#page-help [data-link="start"]').count(), 1, 'Start here on the website');
  await page.click('#guide-again');
  await page.waitForSelector('#gd-tour');
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-play');
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('#gd-tour').count(), 0);
  assert.deepEqual(await seenCalls(page), ['tour', 'tour']);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('seen before: no tour on start; an older core (no Settings::seen): no tour, no toasts, Show me again does nothing', async () => {
  let { page, ctx, errors } = await open({ seen: ['tour'] });
  await page.waitForTimeout(900);
  assert.equal(await page.locator('#gd-tour').count(), 0);
  await ctx.close();
  ({ page, ctx, errors } = await open({}));
  await page.waitForTimeout(900);
  assert.equal(await page.locator('#gd-tour').count(), 0);
  assert.equal(await page.evaluate(() => window.LeylineGuide.moment('mod')), false);
  await page.click('.side .nav[data-page="help"]');
  await page.click('#guide-again');
  assert.equal(await page.locator('#gd-tour, #gd-toast').count(), 0);
  assert.deepEqual(await seenCalls(page), []);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('moments: one short line each, once, one at a time, held while the tour is open; hosted copies the invite link', async () => {
  const { page, ctx, errors } = await open({ seen: [] }, 'light');
  await page.waitForSelector('#gd-tour');
  // during the tour: kept for after it
  assert.equal(await page.evaluate(() => window.LeylineGuide.moment('mod')), true);
  assert.equal(await page.locator('#gd-toast').count(), 0, 'never over the tour');
  await page.click('#gd-tour-skip');
  await page.waitForSelector('#gd-toast');
  assert.match(await page.textContent('#gd-toast'), /Your first mod is in/);
  assert.equal(await page.getAttribute('#gd-toast', 'role'), 'status');
  assert.equal(await page.evaluate(() => window.LeylineGuide.moment('mod')), false, 'once');
  // the next one waits for this one
  assert.equal(await page.evaluate(() => window.LeylineGuide.moment('hosted')), true);
  assert.match(await page.textContent('#gd-toast'), /first mod/);
  await page.click('#gd-toast .gd-toast-x');
  await page.waitForFunction(() => /You're live/.test((document.querySelector('#gd-toast') || {}).textContent || ''));
  await page.waitForTimeout(300);
  await shot(page, 'toast-hosted');
  await page.click('#gd-toast-go');
  await page.waitForFunction(() => /Invite link copied/.test(document.querySelector('#gd-toast').textContent));
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), 'https://relay.openvanilla.app/join/abcdefghijk2345a#INVITECODE1234567890');
  // quiet: the page said it already, it only counts as shown
  assert.equal(await page.evaluate(() => window.LeylineGuide.moment('friend', true)), false);
  assert.equal(await page.evaluate(() => window.LeylineGuide.moment('friend')), false);
  assert.equal(await page.evaluate(() => window.LeylineGuide.moment('nonsense')), false);
  assert.deepEqual(await seenCalls(page), ['cheer-mod', 'tour', 'cheer-hosted', 'cheer-friend']);
  // every line: short, one sentence or two, no brand names, no client numbers
  const lines = await page.evaluate(() => Object.values(window.LeylineGuide.MOMENTS).map((m) => m.line)
    .concat(window.LeylineGuide.TOUR.flatMap((t) => [t.title, t.text])));
  for (const l of lines) { assert.ok(l.length <= 140, l); assert.doesNotMatch(l, BRANDS); assert.doesNotMatch(l, /\b\d\.\d+\.\d/); }
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the toast and the tour sit inside the window on a small launcher', async () => {
  const { page, ctx, errors } = await open({ seen: [] });
  await page.setViewportSize({ width: 900, height: 620 });
  await page.waitForSelector('#gd-tour');
  for (let i = 0; i < 4; i++) { await inView(page); await page.click('#gd-tour-next'); }
  await page.evaluate(() => window.LeylineGuide.moment('hosted'));
  await page.waitForSelector('#gd-toast');
  const b = await page.locator('#gd-toast').boundingBox();
  assert.ok(b.x >= 0 && b.x + b.width <= 900 && b.y + b.height <= 620);
  assert.deepEqual(errors, []);
  await ctx.close();
});
