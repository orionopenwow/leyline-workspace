// Headless Chromium (Playwright), a phone in landscape: tap reliability on the character screens (5 Nov: "real phones
// still often don't press game buttons on character screens even though the tap bubble is in the right place").
//   * a release sent within one game update of its press is sent again after one more settle (and only then);
//   * the button magnet: a tap within 10 px of a scanned button's edge, where the game says nothing is under the
//     cursor, clicks the button's centre; with something under it the tap stays; character creation never moves a
//     tap sideways (only up or down onto a button the finger is already over);
//   * the last 10 taps, with timings, in window.owTouch.taps; Menu > Controls > Copy debug info copies one line;
//   * the tap test: five targets, hits and misses, the factor, and (with the bridge) where the game's cursor went;
//     it works on the character screens (the corner Controls button) and clicks nothing in the game.
//
//   NODE_PATH=$(npm root -g) node --test stream/tests/touch-taps2.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { chromium } = require('playwright');
const { open, finger, centre } = require('./touch-harness.cjs');

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

const ups = (log) => log.filter((e) => e.t === 'pointerup');
const downs = (log) => log.filter((e) => e.t === 'pointerdown');
const clearLog = (p) => p.evaluate(() => { LOG.length = 0; });
const waitUp = (p, n = 1) => p.waitForFunction((k) => LOG.filter((e) => e.t === 'pointerup').length >= k, n, { timeout: 5000 });

test('a release inside one game update of its press is sent again after one more settle; otherwise once', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'charselect' });
  // a normal phone: one release
  await clearLog(p);
  await p.touchscreen.tap(300, 120);
  await waitUp(p);
  await p.waitForTimeout(300);
  let log = await p.evaluate(() => LOG);
  assert.equal(ups(log).length, 1, 'one release when the game had time to see the press');
  let last = await p.evaluate(() => T.taps.at(-1));
  assert.equal(last.resent, undefined);
  assert.ok(last.held >= 1, `the press-to-release time is recorded (${last.held} ms)`);
  // a game whose update is longer than the gap between press and release (forced: a 5 s game update)
  await p.evaluate(() => { T.tapTime.force = 5000; });
  await clearLog(p);
  await p.touchscreen.tap(320, 140);
  await waitUp(p, 2);
  log = await p.evaluate(() => LOG);
  const [u1, u2] = ups(log), [d] = downs(log);
  assert.equal(ups(log).length, 2, 'the release went twice');
  assert.deepEqual([u2.x, u2.y], [d.x, d.y], 'the second release at the press too');
  assert.ok(u2.at > u1.at + 15, 'after another settle, not at once');
  last = await p.evaluate(() => T.taps.at(-1));
  assert.equal(last.resent, true, 'the tap record says so');
  assert.ok(await p.evaluate(() => T.tapTime.resent) >= 1);
  assert.deepEqual(errors, []);
  await ctx.close();
});

const BTN = { x: 400, y: 200, w: 80, h: 14 };
// the bridge answers what is under the cursor with HOVER (and every other chunk with [] after a game update)
const hoverIs = (p, name) => p.evaluate((n) => {
  window.HOVER = n;
  window.LUAFN = async (c) => { if (/GetMouseFocus/.test(c)) { await new Promise((ok) => setTimeout(ok, 15)); return [window.HOVER]; } return undefined; };
}, name);

test('button magnet: a near miss where the game hovers nothing clicks the button; a hovered spot stays; never sideways on creation', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'charselect', live: true });
  await p.evaluate((b) => T.snapAt(0, 0, [b]), BTN);
  // the pure rule
  assert.deepEqual(await p.evaluate(() => T.magnetAt(392, 207)), { x: 440, y: 207 }, '8 px left of it: its centre');
  assert.equal(await p.evaluate(() => T.magnetAt(380, 207)), null, '20 px off: nothing');
  // 8 px left of the button (snap() never goes sideways): the game hovers nothing there
  await hoverIs(p, '');
  await clearLog(p);
  await p.touchscreen.tap(392, 207);
  await waitUp(p);
  let d = downs(await p.evaluate(() => LOG))[0];
  assert.deepEqual([d.x, d.y], [440, 207], 'clicked the button\'s centre');
  assert.equal(await p.evaluate(() => T.taps.at(-1).how), 'magnet');
  // the same spot, but the game has a button there: the tap stays where the finger is
  await hoverIs(p, 'CharSelectAddonsButton');
  await p.waitForTimeout(400);
  await clearLog(p);
  await p.touchscreen.tap(392, 207);
  await waitUp(p);
  d = downs(await p.evaluate(() => LOG))[0];
  assert.deepEqual([d.x, d.y], [392, 207], 'something under the finger: no magnet');
  assert.equal(await p.evaluate(() => T.taps.at(-1).how), 'tap');
  // character creation: sideways never; straight under a button the finger is over: its height, not its x
  await p.evaluate(() => setScreen('charcreate'));
  await p.evaluate((b) => T.snapAt(0, 0, [b]), BTN);
  await hoverIs(p, '');
  assert.equal(await p.evaluate(() => T.magnetAt(392, 207)), null, 'creation: no sideways magnet');
  assert.deepEqual(await p.evaluate(() => T.magnetAt(430, 222)), { x: 430, y: 207 }, 'creation: up onto it, x kept');
  await p.waitForTimeout(400);
  await clearLog(p);
  await p.touchscreen.tap(392, 207);
  await waitUp(p);
  d = downs(await p.evaluate(() => LOG))[0];
  assert.deepEqual([d.x, d.y], [392, 207], 'creation: the tap stays where the finger is');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the last 10 taps with timings, and Copy debug info (from the character screen\'s corner button)', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'charselect' });
  for (let i = 0; i < 12; i++) { await p.touchscreen.tap(200 + i * 20, 100 + (i % 3) * 30); await p.waitForTimeout(60); }
  await p.waitForFunction(() => T.tapCount >= 12, null, { timeout: 8000 });
  const taps = await p.evaluate(() => T.taps);
  assert.equal(taps.length, 10, 'ten kept');
  assert.deepEqual([taps[9].x, taps[9].y], [420, 160], 'the newest last');
  for (const t of taps) {
    assert.equal(t.screen, 'charselect');
    assert.ok(typeof t.at === 'number' && typeof t.finger === 'number', 'when, and how long the finger was down');
  }
  await p.waitForTimeout(600);
  // the corner button on the character screens opens Menu > Controls
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .gear')).display), 'flex', 'the Controls button shows here');
  await p.touchscreen.tap(...await centre(p, '#ow-touch .gear'));
  await p.waitForTimeout(200);
  assert.ok(await p.evaluate(() => document.querySelector('#ow-touch .sheet.controls').classList.contains('open')), 'Controls opened');
  await p.touchscreen.tap(...await centre(p, '#ow-touch [data-c="debug"]'));
  await p.waitForFunction(() => COPIED.length > 0, null, { timeout: 3000 });
  const line = await p.evaluate(() => COPIED[0]);
  assert.ok(!line.includes('\n'), 'one line');
  const build = await p.evaluate(() => M.TOUCH_BUILD);
  for (const want of [build, 'charselect', 'dpr 2', 'buffer 1600x800', 'css 800x400', 'factor 1.00,1.00', 'mode pointer', 'taps 12', 'tap 420,160 f']) {
    assert.ok(line.includes(want), `the line has "${want}": ${line}`);
  }
  assert.doesNotMatch(line, /World of Warcraft|\bWoW\b|Warcraft|Lich King|Blizzard/i);
  assert.deepEqual(errors, []);
  await ctx.close();
});

async function tapTest(p, offsets) {
  await p.touchscreen.tap(...await centre(p, '#ow-touch .gear'));
  await p.waitForTimeout(200);
  await p.touchscreen.tap(...await centre(p, '#ow-touch [data-c="taptest"]'));
  await p.waitForTimeout(200);
  assert.ok(await p.evaluate(() => document.querySelector('#ow-touch .taptest').classList.contains('on')), 'the tap test is open');
  for (const [dx, dy] of offsets) {
    const [x, y] = await centre(p, '#ow-touch .tt-dot');
    await p.touchscreen.tap(x + dx, y + dy);
    await p.waitForTimeout(120);
  }
  await p.waitForFunction(() => T.tapTest, null, { timeout: 5000 });
  return p.evaluate(() => ({ r: T.tapTest, msg: document.querySelector('#ow-touch .tt-msg').textContent }));
}

test('the tap test on the character screens: hits, misses and the factor; nothing is clicked in the game', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'charselect' });
  await clearLog(p);
  const { r, msg } = await tapTest(p, [[0, 0], [3, 2], [0, 0], [40, 0], [-2, 4]]);
  assert.equal(r.n, 5);
  assert.equal(r.hits, 4, 'one tap 40 px off is a miss');
  assert.deepEqual(r.factor, [1, 1]);
  assert.equal(r.game, null, 'no bridge: the game side is not checked');
  assert.match(msg, /4 of 5 hit/);
  assert.match(msg, /Factor 1\.00, 1\.00/);
  assert.deepEqual(downs(await p.evaluate(() => LOG)), [], 'no press reached the game');
  // Copy result: the debug line, with the test in it
  await p.touchscreen.tap(...await centre(p, '#ow-touch [data-tt="copy"]'));
  await p.waitForFunction(() => COPIED.length > 0, null, { timeout: 3000 });
  assert.match(await p.evaluate(() => COPIED.at(-1)), /tap test 4\/5 hits/);
  await p.touchscreen.tap(...await centre(p, '#ow-touch [data-tt="close"]'));
  await p.waitForTimeout(150);
  assert.equal(await p.evaluate(() => document.querySelector('#ow-touch .taptest').classList.contains('on')), false, 'Close closes it');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the tap test with the bridge: where the game\'s cursor went for each tap', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'charselect', live: true });
  // a game that reads every pointer 12 px to the right of where it was sent
  await p.evaluate(() => {
    window.LUAFN = async (c) => {
      if (!/GetCursorPosition/.test(c)) return undefined;
      const m = LOG.filter((e) => e.t === 'pointermove').at(-1);
      return [(m.cx + 12) / 800, 1 - m.cy / 400];
    };
  });
  const { r, msg } = await tapTest(p, [[0, 0], [0, 0], [0, 0], [0, 0], [0, 0]]);
  assert.equal(r.hits, 5);
  assert.deepEqual(r.game, [12, 0], 'the game\'s cursor 12 px right of the finger');
  assert.match(msg, /12 px away from your finger/);
  assert.deepEqual(downs(await p.evaluate(() => LOG)), [], 'still nothing clicked');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('?taptest=1 opens the tap test at once', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'charcreate', query: 'taptest=1' });
  await p.waitForFunction(() => document.querySelector('#ow-touch .taptest').classList.contains('on'), null, { timeout: 3000 });
  assert.deepEqual(errors, []);
  await ctx.close();
});
