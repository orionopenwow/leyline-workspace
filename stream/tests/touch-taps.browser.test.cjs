// Headless Chromium (Playwright) with touch: a tap on the character screens and in the world reaches the game as one
// scripted mouse click (owner 3 Nov 18:45: "mobile mode on desktop still not clicking the buttons in the character
// create ... sometimes it flashed like its going to be clicked"). What the game's canvas receives is recorded:
//   * only scripted events (no trusted touch or mouse event gets through to it while touch controls are on);
//   * the press and the release land on the same spot, pointerType "mouse";
//   * the canvas keeps focus from the press to the release (a blur in between is what left buttons pushed);
//   * a stray mouse move first (a desktop browser in phone emulation makes one) doesn't hand taps to the game.
//
//   NODE_PATH=$(npm root -g) node --test stream/tests/touch-taps.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const WWW = path.join(__dirname, '..', 'www-files');
const page = (state) => `<!doctype html><html><head><meta name=viewport content="width=device-width,initial-scale=1"></head>
<body style="margin:0;background:#2d4a22;height:100vh"><canvas id=benilla tabindex=0 width=1600 height=800 style="width:100vw;height:100vh;display:block"></canvas><script type=module>
const c = document.getElementById('benilla');
window.LOG = [];
for (const t of ['pointermove', 'pointerdown', 'pointerup', 'mousedown', 'mouseup', 'touchstart', 'touchend', 'click'])
  c.addEventListener(t, (e) => LOG.push({ t, trusted: e.isTrusted, pt: e.pointerType || '', x: Math.round(e.offsetX), y: Math.round(e.offsetY), focus: document.activeElement === c }));
c.addEventListener('blur', () => LOG.push({ t: 'blur' }));
const h = {};
window.wenilla = { live: true, stats: {}, session: () => '${state}', fire() {}, hold() {}, chat() {}, force() {}, gamepad: {},
  lua: async () => ['WorldFrame'], on(e, cb) { h[e] = cb; }, state: () => ({ state: '${state}' }) };
(await import('/input.js')).installInput();
const m = await import('/touch.js');
window.T = m.installTouch(window.wenilla);
if (h.state) h.state({ state: '${state}' });
c.focus();
window.ready = 1;
</script></body></html>`;

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(state, phone = true) {
  const ctx = await browser.newContext({ viewport: { width: 800, height: 400 }, hasTouch: true, isMobile: phone, deviceScaleFactor: 2 });
  // the "tap for full screen" offer takes a phone's first tap; this player has said no to it
  await ctx.addInitScript(() => { try { sessionStorage.setItem('ow:fs-skip', '1'); } catch { /* fine */ } });
  // a touch-screen PC has a fine pointer (its mouse or trackpad); headless Chromium with touch reports none
  if (!phone) await ctx.addInitScript(() => { const mm = window.matchMedia.bind(window);
    window.matchMedia = (q) => (/any-pointer:\s*fine/.test(q) ? { matches: true, media: q, addEventListener() {}, removeEventListener() {} } : mm(q)); });
  const p = await ctx.newPage();
  const errors = [];
  p.on('pageerror', (e) => errors.push(String(e)));
  await p.route('http://game.test/**', (r) => {
    const f = new URL(r.request().url()).pathname.slice(1);
    if (f === 'h.html') return r.fulfill({ contentType: 'text/html', body: page(state) });
    const fp = path.join(WWW, f);
    if (!fs.existsSync(fp)) return r.fulfill({ status: 404, body: '' });
    return r.fulfill({ contentType: f.endsWith('.js') ? 'text/javascript' : 'application/octet-stream', body: fs.readFileSync(fp) });
  });
  await p.goto('http://game.test/h.html?touch=1');
  await p.waitForFunction(() => window.ready, null, { timeout: 10000 });
  await p.waitForTimeout(400);
  return { ctx, p, errors };
}

function checkClick(log, what) {
  const onCanvas = log.filter((e) => e.t !== 'blur');
  assert.ok(onCanvas.length, `${what}: the game got something`);
  assert.deepEqual(onCanvas.filter((e) => e.trusted), [], `${what}: no real touch or mouse event reached the game`);
  const down = onCanvas.findIndex((e) => e.t === 'pointerdown'), up = onCanvas.findIndex((e) => e.t === 'pointerup');
  assert.ok(down >= 0 && up > down, `${what}: a press, then a release`);
  assert.equal(onCanvas[down].pt, 'mouse', `${what}: a mouse press, not a touch`);
  assert.deepEqual([onCanvas[up].x, onCanvas[up].y], [onCanvas[down].x, onCanvas[down].y], `${what}: released where it pressed`);
  const between = log.slice(log.indexOf(onCanvas[down]), log.indexOf(onCanvas[up]) + 1);
  assert.ok(!between.some((e) => e.t === 'blur'), `${what}: the game kept focus from the press to the release`);
  assert.ok(onCanvas[down].focus && onCanvas[up].focus, `${what}: focused at the press and at the release`);
}

for (const stray of [false, true]) {
  test(`character creation: a tap is one scripted click${stray ? ', after a stray mouse move (desktop phone emulation)' : ''}`, async () => {
    const { ctx, p, errors } = await open('charcreate');
    if (stray) { await p.mouse.move(50, 50); await p.mouse.move(60, 58); }
    await p.evaluate(() => { LOG.length = 0; });
    await p.touchscreen.tap(400, 300);
    await p.waitForFunction(() => LOG.some((e) => e.t === 'pointerup'), null, { timeout: 5000 });
    await p.waitForTimeout(150);
    checkClick(await p.evaluate(() => LOG), 'character creation');
    assert.equal(await p.evaluate(() => document.getElementById('ow-touch').classList.contains('mouse')), false,
      'a finger keeps the touch layers on');
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

test('in the world: a tap on a game window is one scripted click, after a stray mouse move too', async () => {
  const { ctx, p, errors } = await open('inworld');
  await p.evaluate(() => { window.wenilla.lua = async () => ['CharacterFrame']; });
  await p.mouse.move(50, 50); await p.mouse.move(60, 58);
  await p.evaluate(() => { LOG.length = 0; });
  await p.touchscreen.tap(400, 150);
  await p.waitForFunction(() => LOG.some((e) => e.t === 'pointerup'), null, { timeout: 5000 });
  await p.waitForTimeout(150);
  checkClick(await p.evaluate(() => LOG), 'a game window');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a real mouse on a touch-screen PC still goes straight to the game', async () => {
  const { ctx, p, errors } = await open('inworld', false);   // a touch-screen PC: a touch screen and a fine pointer
  await p.waitForTimeout(900);                       // no finger for a while
  await p.mouse.move(100, 100); await p.mouse.move(140, 120);
  await p.waitForFunction(() => document.getElementById('ow-touch').classList.contains('mouse'), null, { timeout: 2000 });
  await p.evaluate(() => { LOG.length = 0; });
  await p.mouse.click(480, 110);                     // open ground: not the stick zone or a control
  await p.waitForTimeout(150);
  const log = await p.evaluate(() => LOG);
  assert.ok(log.some((e) => e.t === 'pointerdown' && e.trusted && e.pt === 'mouse'), 'the real click reached the game');
  assert.deepEqual(errors, []);
  await ctx.close();
});
