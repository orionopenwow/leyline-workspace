// Headless Chromium (Playwright): the touch controls' layout on phone and tablet screens (www-files/touch.js layout(),
// owner 3 Nov 01:41: "fix mobile touch controls, the buttons are misaligned, do alignment fixes and tests"), and the
// character screens' tap snapping (01:42: "it fails to hit buttons on creating character").
//
// In the world, at six screen sizes from a small phone to a tablet, every control is inside the screen and keeps
// at least 6 px from every other (round controls measured as circles): the stick, Attack, the reticle, the four keys
// on one ring round Attack the same angle apart, Jump under Attack, the page dots, the tray and the flip button.
// On the character screens: character creation never snaps a tap to another place; elsewhere a tap snaps only
// straight above or below a button, never sideways onto a neighbour.
//
//   NODE_PATH=$(npm root -g) node --test stream/tests/touch-layout.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const WWW = path.join(__dirname, '..', 'www-files');
const SIZES = [[568, 320, 'small phone'], [667, 375, 'iPhone SE'], [740, 360, 'small Android'], [844, 390, 'iPhone'],
  [932, 430, 'iPhone Pro Max'], [1024, 768, 'iPad']];
const page = (state) => `<!doctype html><html><head><meta name=viewport content="width=device-width,initial-scale=1,viewport-fit=cover"></head>
<body style="margin:0;background:#2d4a22;height:100vh"><canvas id=canvas style="width:100vw;height:100vh;display:block"></canvas><script type=module>
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

async function open(w, h, state, ls = {}) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, hasTouch: true, isMobile: true, deviceScaleFactor: 2 });
  // Menu > Controls settings, as the player left them (ow:touch-preset, ow:lefty)
  await ctx.addInitScript((kv) => { for (const [k, v] of Object.entries(kv)) localStorage.setItem(k, v); }, ls);
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

// the distance between two controls' edges: circles as circles, the rest as boxes
function gap(a, b) {
  const box = (r) => ({ l: r.l, t: r.t, r: r.r, b: r.b });
  const circ = (r) => ({ x: (r.l + r.r) / 2, y: (r.t + r.b) / 2, rad: (r.r - r.l) / 2 });
  if (a.round && b.round) { const p = circ(a), q = circ(b); return Math.hypot(p.x - q.x, p.y - q.y) - p.rad - q.rad; }
  if (a.round || b.round) {
    const c = circ(a.round ? a : b), r = box(a.round ? b : a);
    const dx = Math.max(r.l - c.x, 0, c.x - r.r), dy = Math.max(r.t - c.y, 0, c.y - r.b);
    return Math.hypot(dx, dy) - c.rad;
  }
  return Math.max(Math.max(a.l, b.l) - Math.min(a.r, b.r), Math.max(a.t, b.t) - Math.min(a.b, b.b));
}

// Phone and Tablet presets (Menu > Controls > Layout), each right- and left-handed, at every size: the same rules
const VARIANTS = [['auto', {}], ['Phone', { 'ow:touch-preset': 'phone' }], ['Tablet', { 'ow:touch-preset': 'tablet' }],
  ['Phone, left-handed', { 'ow:touch-preset': 'phone', 'ow:lefty': 'on' }], ['Tablet, left-handed', { 'ow:touch-preset': 'tablet', 'ow:lefty': 'on' }]];
for (const [w, h, name] of SIZES) for (const [vname, ls] of VARIANTS) {
  test(`in the world, ${name} (${w}x${h}), ${vname}: every control on screen, 6 px or more apart, the keys evenly on one ring`, async () => {
    const { ctx, p, errors } = await open(w, h, 'inworld', ls);
    const items = await p.evaluate(() => {
      const out = [];
      for (const s of ['.stick', '.attack', '.target', '.key', '.jump', '.dots', '.tray', '.flip']) {
        document.querySelectorAll('#ow-touch ' + s).forEach((e, i) => {
          const r = e.getBoundingClientRect();
          if (!r.width || getComputedStyle(e).display === 'none') return;
          out.push({ k: s + (i ? i : ''), l: r.left, t: r.top, r: r.right, b: r.bottom, round: ['.stick', '.attack', '.target', '.key'].includes(s) });
        });
      }
      return out;
    });
    for (const k of ['.stick', '.attack', '.target', '.key', '.key1', '.key2', '.key3', '.jump', '.dots', '.tray']) {
      assert.ok(items.some((i) => i.k === k), `${k} is shown`);
    }
    for (const i of items) assert.ok(i.l >= 0 && i.t >= 0 && i.r <= w && i.b <= h, `${i.k} inside the screen (${[i.l, i.t, i.r, i.b].map(Math.round)})`);
    for (let a = 0; a < items.length; a++) {
      for (let b = a + 1; b < items.length; b++) {
        const g = gap(items[a], items[b]);
        assert.ok(g >= 6, `${items[a].k} and ${items[b].k} are ${g.toFixed(1)} px apart`);
      }
    }
    // the four keys: the same distance from Attack's centre, the same angle apart
    const c = (i) => [(i.l + i.r) / 2, (i.t + i.b) / 2];
    const [ax, ay] = c(items.find((i) => i.k === '.attack'));
    const keys = ['.key', '.key1', '.key2', '.key3'].map((k) => c(items.find((i) => i.k === k)));
    const rad = keys.map(([x, y]) => Math.hypot(x - ax, y - ay));
    const ang = keys.map(([x, y]) => Math.atan2(ay - y, x - ax) * 180 / Math.PI);
    assert.ok(Math.max(...rad) - Math.min(...rad) <= 2, `one ring: ${rad.map(Math.round)}`);
    // (mirrored: the keys go round the other way)
    const norm = (a) => ((a % 360) + 360) % 360;
    const steps = [ang[1] - ang[0], ang[2] - ang[1], ang[3] - ang[2]].map((x) => Math.abs(((norm(x) + 180) % 360) - 180));
    assert.ok(Math.max(...steps) - Math.min(...steps) <= 2, `evenly apart: ${steps.map(Math.round)}`);
    const st = c(items.find((i) => i.k === '.stick'));
    if (ls['ow:lefty']) assert.ok(st[0] > w / 2 && ax < w / 2, 'left-handed: the stick on the right, the ring on the left');
    else assert.ok(st[0] < w / 2 && ax > w / 2, 'the stick on the left, the ring on the right');
    const L = await p.evaluate(() => window.T.layout);
    assert.equal(L.preset, ls['ow:touch-preset'] || (Math.min(w, h) >= 600 ? 'tablet' : 'phone'));
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

test('character creation: a tap is never moved to a "button" the scan found; other screens snap only straight up or down', async () => {
  const { ctx, p, errors } = await open(844, 390, 'charcreate');
  // a red piece of Forever UI's art the scan would take for a button, right next to the tap
  const near = [{ x: 400, y: 200, w: 80, h: 14 }];
  assert.equal(await p.evaluate((t) => window.T.snapAt(395, 206, t), near), null, 'next to it: the tap stays where the finger is');
  assert.equal(await p.evaluate((t) => window.T.snapAt(440, 220, t), near), null, 'under it: still no snap on character creation');
  await ctx.close();
  const o = await open(844, 390, 'charselect');
  const btn = { x: 400, y: 200, w: 80, h: 14 };
  assert.deepEqual(await o.p.evaluate((t) => window.T.snapAt(440, 220, t), [btn]), btn, 'character select: just under a button clicks it');
  assert.equal(await o.p.evaluate((t) => window.T.snapAt(392, 206, t), [btn]), null, 'never sideways onto it');
  assert.equal(await o.p.evaluate((t) => window.T.snapAt(440, 240, t), [btn]), null, 'nor from far below');
  assert.deepEqual(o.errors, []);
  assert.deepEqual(errors, []);
  await o.ctx.close();
});
