// Shared by the touch-*.browser tests: a fake game page (a canvas that logs what reaches it) with touch.js on it and a
// fake page bridge (window.wenilla). Nothing of the real client is loaded.
//   open(browser, { state, live, w, h, query, init })
//     state  the screen ('charselect', 'charcreate', 'inworld', ...)
//     live   the bridge answers Lua (wenilla.ready); window.LUAFN(chunk) answers each chunk (default: [] after 20 ms,
//            about a game update, so the page's "wait for the game" steps take a game-like time)
//     init   a function run in the page before anything loads (addInitScript)
//   In the page: LOG (what the canvas got), RAN (Lua chunks), FIRED (fire() commands), HELD (hold() calls),
//   COPIED (clipboard writes), window.T (window.owTouch).
//   finger(p, points, ms) moves one finger through points by CDP (touchStart, touchMove..., touchEnd).
'use strict';
const fs = require('node:fs');
const path = require('node:path');

const WWW = path.join(__dirname, '..', 'www-files');

const page = (state, live) => `<!doctype html><html><head><meta name=viewport content="width=device-width,initial-scale=1,viewport-fit=cover"></head>
<body style="margin:0;background:#2d4a22;height:100vh"><canvas id=benilla tabindex=0 width=1600 height=800 style="width:100vw;height:100vh;display:block"></canvas><script type=module>
const c = document.getElementById('benilla');
window.LOG = []; window.RAN = []; window.FIRED = []; window.HELD = [];
for (const t of ['pointermove', 'pointerdown', 'pointerup', 'keydown'])
  c.addEventListener(t, (e) => LOG.push({ t, trusted: e.isTrusted, pt: e.pointerType || '', x: Math.round(e.offsetX), y: Math.round(e.offsetY),
    cx: Math.round(e.clientX), cy: Math.round(e.clientY), key: e.key, shift: !!e.shiftKey, b: e.button, at: performance.now() }));
const h = {};
window.STATE = '${state}';
window.wenilla = { ready: ${live ? 'true' : 'false'}, stats: {}, gamepad: {},
  get state() { return { session: { state: window.STATE } }; },
  fire(cmd) { FIRED.push(cmd); }, hold(cmd, down) { HELD.push([cmd, down]); }, chat() {}, releaseAll() {},
  async lua(chunk) {
    RAN.push(chunk);
    if (window.LUAFN) { const r = await window.LUAFN(chunk); if (r !== undefined) return r; }
    await new Promise((ok) => setTimeout(ok, 20));
    return [];
  },
  on(e, cb) { h[e] = cb; } };
window.setScreen = (st) => { window.STATE = st; if (h.state) h.state({ state: st }); };
(await import('/input.js')).installInput();
const m = await import('/touch.js');
window.M = m;
window.T = m.installTouch(window.wenilla);
if (h.state) h.state({ state: '${state}' });
c.focus();
window.ready = 1;
</script></body></html>`;

async function open(browser, { state = 'charselect', live = false, w = 800, h = 400, query = '', init = null, ctx: given = null } = {}) {
  const ctx = given || await browser.newContext({ viewport: { width: w, height: h }, hasTouch: true, isMobile: true, deviceScaleFactor: 2 });
  await ctx.addInitScript(() => {
    try { sessionStorage.setItem('ow:fs-skip', '1'); } catch { /* fine */ }
    window.COPIED = [];
    try { Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: async (t) => { window.COPIED.push(t); } } }); } catch { /* fine */ }
  });
  if (init) await ctx.addInitScript(init);
  const p = await ctx.newPage();
  const errors = [];
  p.on('pageerror', (e) => errors.push(String(e)));
  await p.route('http://game.test/**', (r) => {
    const f = new URL(r.request().url()).pathname.slice(1);
    if (f === 'h.html') return r.fulfill({ contentType: 'text/html', body: page(state, live) });
    const fp = path.join(WWW, f);
    if (!fs.existsSync(fp)) return r.fulfill({ status: 404, body: '' });
    return r.fulfill({ contentType: f.endsWith('.js') ? 'text/javascript' : 'application/octet-stream', body: fs.readFileSync(fp) });
  });
  await p.goto('http://game.test/h.html?touch=1' + (query ? '&' + query : ''));
  await p.waitForFunction(() => window.ready, null, { timeout: 10000 });
  await p.waitForTimeout(400);
  return { ctx, p, errors };
}

/** One finger through points [[x, y], ...], ms between them. */
async function finger(p, points, ms = 30) {
  const cdp = await p.context().newCDPSession(p);
  const [x0, y0] = points[0];
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: x0, y: y0 }] });
  for (const [x, y] of points.slice(1)) {
    await p.waitForTimeout(ms);
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x, y }] });
  }
  await p.waitForTimeout(ms);
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await cdp.detach();
}

/** The centre of the first element matching sel. */
const centre = (p, sel) => p.evaluate((s) => { const r = document.querySelector(s).getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; }, sel);

module.exports = { open, finger, centre, WWW };
