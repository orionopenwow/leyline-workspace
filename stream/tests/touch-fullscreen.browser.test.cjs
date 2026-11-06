// Headless Chromium (Playwright), a phone in landscape: the touch controls' full-screen prompt and the home-screen
// button (owner 5 Nov: "the fullscreen tap to activate sometimes breaks", "add a button to save the mobile version to
// the home screen with a Leyline logo shortcut").
//   * full screen is asked for when the finger LIFTS (a touch's pointerdown is not a user gesture to the browser, so
//     asking then failed unless an earlier tap had left the page activated), never on the press;
//   * "Add Leyline to your home screen" shows on a phone; with the browser's install prompt it opens that prompt,
//     without one it says how (Share > Add to Home Screen on iPhone);
//   * "Not now" still hides the prompt.
//
//   NODE_PATH=$(npm root -g) node --test stream/tests/touch-fullscreen.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const WWW = path.join(__dirname, '..', 'www-files');
const page = `<!doctype html><html><head><meta name=viewport content="width=device-width,initial-scale=1,viewport-fit=cover"></head>
<body style="margin:0;background:#2d4a22;height:100vh"><canvas id=canvas style="width:100vw;height:100vh;display:block"></canvas><script type=module>
window.fsCalls = [];
document.documentElement.requestFullscreen = function () { window.fsCalls.push(window.lastPointer || '?'); return Promise.resolve(); };
for (const t of ['pointerdown', 'pointerup']) addEventListener(t, () => { window.lastPointer = t; }, true);
const h = {};
window.wenilla = { live: true, stats: {}, session: () => 'charselect', fire() {}, hold() {}, chat() {}, force() {}, gamepad: {},
  lua: async () => [], on(e, c) { h[e] = c; }, state: () => ({ state: 'charselect' }) };
const m = await import('/touch.js');
window.T = m.installTouch(window.wenilla);
if (h.state) h.state({ state: 'charselect' });
window.ready = 1;
</script></body></html>`;

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open() {
  const ctx = await browser.newContext({ viewport: { width: 844, height: 390 }, hasTouch: true, isMobile: true, deviceScaleFactor: 3 });
  const p = await ctx.newPage();
  const errors = [];
  p.on('pageerror', (e) => errors.push(String(e)));
  await p.route('http://game.test/**', (r) => {
    const f = new URL(r.request().url()).pathname.slice(1);
    if (f === 'h.html') return r.fulfill({ contentType: 'text/html', body: page });
    const fp = path.join(WWW, f);
    if (!fs.existsSync(fp)) return r.fulfill({ status: 404, body: '' });
    return r.fulfill({ contentType: f.endsWith('.js') ? 'text/javascript' : 'application/octet-stream', body: fs.readFileSync(fp) });
  });
  // the browser's own full-screen support, as a phone has it (headless Chromium may report it off)
  await p.addInitScript(() => { try { Object.defineProperty(Document.prototype, 'fullscreenEnabled', { get: () => true }); } catch { /* fine */ } });
  await p.goto('http://game.test/h.html?touch=1');
  await p.waitForFunction(() => window.ready, null, { timeout: 10000 });
  await p.waitForTimeout(400);
  return { ctx, p, errors };
}

// a finger: down, a pause, up, at the element's centre
async function finger(p, sel, holdMs = 120) {
  const c = await p.evaluate((s) => { const r = document.querySelector(s).getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; }, sel);
  const cdp = await p.context().newCDPSession(p);
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: c.x, y: c.y }] });
  await p.waitForTimeout(holdMs);
  const atDown = await p.evaluate(() => window.fsCalls.length);
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await p.waitForTimeout(150);
  return atDown;
}

test('full screen is asked for when the finger lifts, not when it lands', async () => {
  const { ctx, p, errors } = await open();
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .fsgo')).display), 'flex', 'the prompt shows on a phone in landscape');
  const atDown = await finger(p, '#ow-touch .fsgo svg');
  assert.equal(atDown, 0, 'nothing asked while the finger is down');
  const calls = await p.evaluate(() => window.fsCalls);
  assert.deepEqual(calls, ['pointerup'], 'asked once, from the lift');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('"Add Leyline to your home screen" shows on a phone and opens the browser\'s install prompt', async () => {
  const { ctx, p, errors } = await open();
  assert.ok(await p.evaluate(() => document.querySelector('#ow-touch').classList.contains('caninstall')), 'offered on a phone');
  const inst = '#ow-touch .fsgo .inst';
  assert.notEqual(await p.evaluate((s) => getComputedStyle(document.querySelector(s)).display, inst), 'none');
  assert.match(await p.textContent(inst), /home screen/i);
  // Android Chrome: the browser hands the page its install prompt
  await p.evaluate(() => {
    const e = new Event('beforeinstallprompt', { cancelable: true });
    e.prompt = () => { window.prompted = (window.prompted || 0) + 1; return Promise.resolve(); };
    dispatchEvent(e);
  });
  await finger(p, inst);
  assert.equal(await p.evaluate(() => window.prompted), 1, 'the install prompt opened once');
  assert.deepEqual(await p.evaluate(() => window.fsCalls), [], 'the button does not also go full screen');
  // no prompt left (iPhone Safari, or already used): it says how instead
  await finger(p, inst);
  await p.waitForTimeout(200);
  assert.match(await p.evaluate(() => document.querySelector('#ow-touch').innerText), /Add to Home screen|Add to Home Screen/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('"Not now" still hides the prompt, on the lift', async () => {
  const { ctx, p, errors } = await open();
  await finger(p, '#ow-touch .fsgo .skip');
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .fsgo')).display), 'none');
  assert.deepEqual(await p.evaluate(() => window.fsCalls), []);
  assert.deepEqual(errors, []);
  await ctx.close();
});
