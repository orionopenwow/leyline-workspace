// A real phone (phone-sim.cjs: Pixel 7 / iPhone 13 descriptors, CDP touches with a finger's timing and roll) on the game
// page's first screens: the owner's "takes a few taps to enter the fullscreen version" (6 Nov, on his own phone).
//   * the very first tap anywhere asks for full screen: on "Signing in…" (before, nothing did until the character
//     list's prompt showed), from the finger's lift, as a user gesture, once;
//   * a finger that rolls a few pixels on touch.js's "Tap to play full screen" still counts (before: the browser took
//     it as a pan, pointercancel, no lift, the tap was lost);
//   * buttons and links keep their own tap (no full screen ask in the way of a file picker or the install prompt);
//   * no ask with ?fs=0, after "Not now", with a mouse, or where the browser has no full screen (iPhone Safari).
// The page is a stand-in (a canvas, the "Signing in…" cover as wrath-page.js draws it, touch.js on a fake bridge) with
// wrath-early.js's first-tap block as the real page runs it. PHONE_WWW=<dir> runs it against other www-files (the doc's
// before/after).
//
//   node --test tests/phone-fullscreen.browser.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');
const sim = require('./phone-sim.cjs');

const WWW = process.env.PHONE_WWW || path.join(__dirname, '..', 'www-files');
const early = fs.readFileSync(path.join(WWW, 'wrath-early.js'), 'utf8');
const m = early.match(/\/\/ >>> first-tap-fullscreen[\s\S]*?\/\/ <<< first-tap-fullscreen/);
const FIRST_TAP = m ? m[0] : '/* no first-tap block in this wrath-early.js */';

const page = (state, cover) => `<!doctype html><html><head><meta name=viewport content="width=device-width,initial-scale=1,viewport-fit=cover">
<script>${FIRST_TAP}</script></head>
<body style="margin:0;background:#2d4a22;height:100vh"><canvas id=canvas style="width:100vw;height:100vh;display:block"></canvas>
${cover ? '<div id=ow-signin style="position:fixed;inset:0;z-index:25;display:grid;place-items:center;background:rgba(5,4,10,.86);color:#f0d79a">Signing in…</div>' : ''}
<button id=pick data-x style="position:fixed;left:8px;top:8px;z-index:30;width:120px;height:48px">Use my game folder</button>
<script type=module>
const h = {};
window.wenilla = { live: true, stats: {}, session: () => '${state}', fire() {}, hold() {}, chat() {}, force() {}, gamepad: {},
  lua: async () => [], on(e, c) { h[e] = c; }, state: () => ({ state: '${state}' }) };
const m = await import('/touch.js');
window.T = m.installTouch(window.wenilla);
if (h.state) h.state({ state: '${state}' });
window.ready = 1;
</script></body></html>`;

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(dev, { landscape = true, state = 'login', cover = true, query = '', fullscreen = true, init = null } = {}) {
  const { ctx, p, errors } = await sim.phone(browser, dev, { landscape, fullscreen, init });
  await p.route('http://game.test/**', (r) => {
    const f = new URL(r.request().url()).pathname.slice(1);
    if (f === 'h.html') return r.fulfill({ contentType: 'text/html', body: page(state, cover) });
    const fp = path.join(WWW, f);
    if (!fs.existsSync(fp)) return r.fulfill({ status: 404, body: '' });
    return r.fulfill({ contentType: f.endsWith('.js') ? 'text/javascript' : 'application/octet-stream', body: fs.readFileSync(fp) });
  });
  await p.goto('http://game.test/h.html?touch=1' + (query ? '&' + query : ''));
  await p.waitForFunction(() => window.ready, null, { timeout: 15000 });
  await p.waitForTimeout(300);
  return { ctx, p, errors };
}

for (const dev of ['pixel', 'iphone']) {
  test(`${dev}: the first tap on "Signing in…" goes full screen, from the lift, as a user gesture, once`, async () => {
    const { ctx, p, errors } = await open(dev);
    assert.equal(await p.evaluate(() => !!document.getElementById('wu-fs-hint')), true, 'the hint says a tap goes full screen');
    const tr = await sim.tap(p, [200, 200], { jitter: 6 });   // a finger rolls a little
    assert.equal(tr.target, 'div#ow-signin', 'the finger landed on the sign-in cover');
    assert.equal(tr.fullscreen.length, 1, 'asked once: ' + sim.describe(tr));
    const f = tr.fullscreen[0];
    assert.ok(['pointerup', 'touchend'].includes(f.event), 'from the lift, not the press: ' + f.event);
    assert.equal(f.trusted, true, 'from the real touch');
    assert.equal(f.active, true, 'the browser counts it as a user gesture');
    assert.match(f.el, /^html\b/, 'the whole page goes full screen');
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

test('touch.js\'s "Tap to play full screen": a finger that rolls 12 px still counts (no pan, no pointercancel)', async () => {
  const { ctx, p, errors } = await open('pixel', { state: 'charselect', cover: false,
    init: () => { try { sessionStorage.removeItem('ow:fs-skip'); } catch { /* fine */ } } });
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .fsgo')).display), 'flex', 'the prompt shows');
  const at = await sim.centreOf(p, '#ow-touch .fsgo svg');
  const tr = await sim.tap(p, at, { jitter: 12 });
  assert.ok(!tr.sequence.some((s) => s.startsWith('pointercancel')), 'the browser did not take it as a pan: ' + tr.sequence.join(' '));
  assert.equal(tr.fullscreen.length, 1, 'asked once: ' + sim.describe(tr));
  assert.equal(tr.fullscreen[0].active, true);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('upright: the first tap on "turn sideways" goes full screen and asks for landscape', async () => {
  const { ctx, p, errors } = await open('pixel', { landscape: false, state: 'charselect', cover: false,
    init: () => { window.LOCKS = []; try { Object.defineProperty(screen.orientation, 'lock', { configurable: true, value: (o) => { window.LOCKS.push(o); return Promise.resolve(); } }); } catch { /* fine */ } } });
  const tr = await sim.tap(p, await sim.centreOf(p, '#ow-touch .rotate svg'));
  assert.equal(tr.fullscreen.length, 1, sim.describe(tr));
  await p.waitForTimeout(200);
  assert.ok((await p.evaluate(() => window.LOCKS)).includes('landscape'), 'landscape asked for once full screen');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('buttons keep their own tap: no full screen ask on "Use my game folder"', async () => {
  const { ctx, p } = await open('pixel');
  const tr = await sim.tap(p, '#pick');
  assert.equal(tr.clickTarget, 'button#pick');
  assert.deepEqual(tr.fullscreen, [], 'the button\'s gesture is left for its own use');
  await ctx.close();
});

test('no ask with ?fs=0, after "Not now", or without full screen in the browser (iPhone Safari)', async () => {
  for (const o of [{ query: 'fs=0' }, { init: () => { sessionStorage.setItem('ow:fs-skip', '1'); } }, { fullscreen: false, dev: 'iphone' }]) {
    const { ctx, p } = await open(o.dev || 'pixel', o);
    const tr = await sim.tap(p, [200, 200]);
    assert.deepEqual(tr.fullscreen, [], JSON.stringify(Object.keys(o)) + ': ' + sim.describe(tr));
    assert.equal(await p.evaluate(() => !!document.getElementById('wu-fs-hint')), false, 'no hint either');
    await ctx.close();
  }
});

test('a desktop with a mouse: never', async () => {
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  await ctx.addInitScript(sim.instrument);
  const p = await ctx.newPage();
  await p.route('http://game.test/**', (r) => {
    const f = new URL(r.request().url()).pathname.slice(1);
    if (f === 'h.html') return r.fulfill({ contentType: 'text/html', body: page('login', true) });
    const fp = path.join(WWW, f);
    return fs.existsSync(fp) ? r.fulfill({ contentType: 'text/javascript', body: fs.readFileSync(fp) }) : r.fulfill({ status: 404, body: '' });
  });
  await p.goto('http://game.test/h.html');
  await p.waitForFunction(() => window.ready);
  await p.mouse.click(400, 400);
  await p.waitForTimeout(200);
  assert.deepEqual(await p.evaluate(() => __sim.fs), []);
  await ctx.close();
});
