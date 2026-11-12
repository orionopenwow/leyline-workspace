// The sign-in cover (ui/cover.html, cover.js, cover.css; src-tauri/src/cover.rs): the page draws what the core says
// (the heading, the four steps with the current one lit and those before it done, the progress line), follows the
// core as the sign-in moves on, never puts the core's text in as markup, and "Show the game" or Esc asks the core to
// hide it for this start. Settings has its switch, on by default.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/cover.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const COVER = 'file://' + path.join(__dirname, '..', 'ui', 'cover.html');
const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const BRAND = /World of Warcraft|\bWoW\b|Warcraft|Lich King|(?<!endorsed by )Blizzard/;

// the core's cover_state, changed by the test through window.__view
const COVER_FAKE = (view) => {
  const calls = (window.__calls = []);
  window.__view = view;
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'cover_state': return window.__view;
      case 'cover_hide': return null;
      case 'cover_link': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

const STEPS = ['Opening the game', 'Signing in', 'Choosing your realm', 'Character select'];

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function openCover(view, size) {
  const ctx = await browser.newContext({ viewport: size || { width: 1280, height: 720 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.addInitScript(COVER_FAKE, view);
  await page.goto(COVER);
  await page.waitForFunction(() => document.querySelectorAll('#cv-steps li').length > 0);
  return { page, ctx, errors };
}
const steps = (page) => page.$$eval('#cv-steps li', (ls) => ls.map((l) => [l.textContent, l.className.replace('cv-step ', ''), l.getAttribute('aria-current')]));

test('the cover draws the heading and the steps, and follows the sign-in', async () => {
  const { page, ctx, errors } = await openCover({ id: 7, title: 'Logging you in as Orion…', steps: STEPS, step: 0, shown: true });
  assert.equal(await page.textContent('#cv-title'), 'Logging you in as Orion…');
  assert.deepEqual(await steps(page), [
    ['Opening the game', 'now', 'step'], ['Signing in', 'next', null], ['Choosing your realm', 'next', null], ['Character select', 'next', null]]);
  const w0 = await page.$eval('#cv-fill', (f) => parseFloat(f.style.width));
  // the keys went, then the realm: the page catches up within a look or two
  await page.evaluate(() => { window.__view = Object.assign({}, window.__view, { step: 2 }); });
  await page.waitForFunction(() => document.querySelector('#cv-steps li[aria-current]').textContent === 'Choosing your realm');
  assert.deepEqual((await steps(page)).map((s) => s[1]), ['done', 'done', 'now', 'next']);
  const w2 = await page.$eval('#cv-fill', (f) => parseFloat(f.style.width));
  assert.ok(w2 > w0 && w2 < 100, `the line fills as it goes: ${w0} → ${w2}`);
  // the whole cover fills the window (it sits exactly over the game's picture)
  const box = await page.$eval('.cover', (c) => { const r = c.getBoundingClientRect(); return [r.width, r.height]; });
  assert.deepEqual(box, [1280, 720]);
  // the mark spins, the "Show the game" button is there, and nothing names the game
  assert.ok(await page.$('.cv-mark.ly-spin'));
  assert.match(await page.textContent('#cv-show'), /Show the game/);
  assert.doesNotMatch(await page.textContent('body'), BRAND);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the Den has its own last step, and a small game window still fits', async () => {
  const den = ['Opening the game', 'Signing in', 'Choosing your realm', 'Into your Den'];
  const { page, ctx, errors } = await openCover({ id: 2, title: 'Opening your Den…', steps: den, step: 3, shown: true }, { width: 800, height: 600 });
  assert.equal(await page.textContent('#cv-title'), 'Opening your Den…');
  assert.deepEqual((await steps(page)).map((s) => s[1]), ['done', 'done', 'done', 'now']);
  const fits = await page.$eval('.cv-card', (c) => { const r = c.getBoundingClientRect(); return r.top >= 0 && r.bottom <= 600 && r.left >= 0 && r.right <= 800; });
  assert.ok(fits, 'the card fits in an 800×600 game window');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test("the core's words are text, never markup, and a broken answer draws nothing odd", async () => {
  const { page, ctx, errors } = await openCover({ id: 1, title: '<img src=x onerror="window.__x=1">', steps: ['<b>a</b>', 'b'], step: 9, shown: true });
  assert.equal(await page.$('#cv-title img'), null);
  assert.equal(await page.$('#cv-steps b'), null);
  assert.equal(await page.textContent('#cv-title'), '<img src=x onerror="window.__x=1">');
  assert.deepEqual((await steps(page)).map((s) => s[1]), ['done', 'now'], 'a step past the end is the last one');
  await page.evaluate(() => { window.__view = null; });
  await page.waitForTimeout(400);
  assert.equal((await steps(page)).length, 2, 'kept as it was');
  assert.equal(await page.evaluate(() => window.__x), undefined);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('"Show the game" and Esc ask the core to hide this start\'s cover', async () => {
  const { page, ctx } = await openCover({ id: 42, title: 'Logging you in…', steps: STEPS, step: 1, shown: true });
  await page.click('#cv-show');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'cover_hide'));
  let hides = await page.evaluate(() => window.__calls.filter((c) => c[0] === 'cover_hide'));
  assert.deepEqual(hides, [['cover_hide', { id: 42 }]]);
  await page.keyboard.press('Escape');
  await page.waitForFunction(() => window.__calls.filter((c) => c[0] === 'cover_hide').length === 2);
  hides = await page.evaluate(() => window.__calls.filter((c) => c[0] === 'cover_hide'));
  assert.deepEqual(hides[1], ['cover_hide', { id: 42 }]);
  await ctx.close();
});

test('single player: its three steps, the link box told to the core for click-through, and a short line when it did not get through', async () => {
  const single = ['Starting your world', 'Signing in', 'Entering the world'];
  const { page, ctx, errors } = await openCover({ id: 5, title: 'Logging you in…', steps: single, step: 1, shown: true, error: '' }, { width: 1024, height: 768 });
  assert.deepEqual((await steps(page)).map((s) => s[0]), single);
  assert.equal(await page.isHidden('#cv-error'), true);
  // the core is told where "Show the game" is, in CSS pixels with the scale, for this start
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'cover_link'));
  const link = await page.evaluate(() => window.__calls.filter((c) => c[0] === 'cover_link').pop()[1]);
  const box = await page.$eval('#cv-show', (b) => { const r = b.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; });
  assert.deepEqual(link, Object.assign({ id: 5, scale: 1 }, box));
  assert.ok(link.x > 512 && link.y > 600, 'bottom right');
  // told once, not on every look
  await page.waitForTimeout(700);
  assert.equal(await page.evaluate(() => window.__calls.filter((c) => c[0] === 'cover_link').length), 1);
  // a smaller window: told again
  await page.setViewportSize({ width: 800, height: 600 });
  await page.waitForFunction(() => window.__calls.filter((c) => c[0] === 'cover_link').length === 2);
  // the sign-in didn't get through: the line, as text, instead of the note
  await page.evaluate(() => { window.__view = Object.assign({}, window.__view, { error: 'Leyline couldn\'t log you in by itself this time. <b>x</b>' }); });
  await page.waitForFunction(() => !document.getElementById('cv-error').hidden);
  assert.equal(await page.textContent('#cv-error'), "Leyline couldn't log you in by itself this time. <b>x</b>");
  assert.equal(await page.$('#cv-error b'), null);
  assert.equal(await page.isHidden('#cv-note'), true);
  assert.doesNotMatch(await page.textContent('body'), BRAND);
  assert.deepEqual(errors, []);
  await ctx.close();
});

// the launcher's Settings: the switch, on unless turned off
const LAUNCHER_FAKE = (saved) => {
  const calls = (window.__calls = []);
  try { localStorage.setItem('leyline.page', 'settings'); } catch (_) { /* fine */ }
  const settings = Object.assign({ clients: [], selected_client: null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' }, count_installs: true, hybrid_login: true, seen: ['tour'] }, saved);
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings, official_realm: 'play.openvanilla.app',
        browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'set_signin_cover': return null;
      case 'themes_list': return { active: 'dark', themes: [{ id: 'dark', name: 'Dark', kind: 'dark', builtin: true }] };
      case 'realm_status': return { state: 'online', ping_ms: 10 };
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'mods_catalog': return new Promise(() => {});
      case 'mods_updates': return [];
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return true;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

test("Settings: Hide the login behind Leyline's screen, on by default, turned off and on", async () => {
  for (const [saved, on] of [[{}, true], [{ signin_cover: false }, false]]) {
    const ctx = await browser.newContext();
    const page = await ctx.newPage();
    await page.addInitScript(LAUNCHER_FAKE, saved);
    await page.goto(INDEX);
    await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'get_state'));
    await page.waitForFunction((want) => document.getElementById('set-cover').checked === want, on);
    const label = await page.$eval('#set-cover', (i) => i.closest('label').textContent);
    assert.match(label, /Hide the login behind Leyline's screen/);
    assert.match(label, /Single player and Your Den/);
    assert.doesNotMatch(label, BRAND);
    if (on) {
      await page.evaluate(() => { location.hash = ''; });
      await page.$eval('#set-cover', (i) => i.click());
      await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'set_signin_cover'));
      assert.deepEqual(await page.evaluate(() => window.__calls.find((c) => c[0] === 'set_signin_cover')), ['set_signin_cover', { on: false }]);
      await page.waitForFunction(() => /login screen/.test(document.getElementById('cover-status').textContent));
    }
    await ctx.close();
  }
});
