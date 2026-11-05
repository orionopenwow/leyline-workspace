// Headless Chromium (Playwright), a phone in landscape, in the world: thumb targeting and casting.
//   * the reticle on the ring is the nearest-enemy button (TARGETNEARESTENEMY through the bridge, Tab without it);
//   * a sideways swipe on the target frame cycles enemies (left: next, right: last; Shift-Tab without the bridge);
//   * soft lock: a tap on open ground (no unit under it) keeps the target, a tap on a unit still clicks, and with
//     soft lock off the ground tap clicks as before;
//   * the cast bar over the ring, from UnitCastingInfo / UnitChannelInfo, filling (a channel drains), on screen and
//     clear of the ring's controls;
//   * a ground-targeted spell (SpellIsTargeting after the key): the next tap in the world places it (a left click
//     there, soft lock or not), and the X cancels it (SpellStopTargeting).
//
//   NODE_PATH=$(npm root -g) node --test stream/tests/touch-combat.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { chromium } = require('playwright');
const { open, finger, centre } = require('./touch-harness.cjs');

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

const TF = [300, 10, 500, 70];          // the target frame, css px (the fake game says so)
// A fake game in the world: the action poll answers with a hostile target, the target frame at TF, CAST (a cast
// string or ''), AIM (a ground spell waiting); "what is under the cursor" is TargetFrame over TF, else WorldFrame;
// MOUSEOVER says whether a unit is under it. UseAction of a ground spell (GROUND) sets AIM; a click clears it.
function fakeWorld() {
  window.CAST = ''; window.AIM = 0; window.MOUSEOVER = 0; window.GROUND = 0;
  const tf = [300, 10, 500, 70];
  const at = () => LOG.filter((e) => e.t === 'pointermove').at(-1) || { cx: -1, cy: -1 };
  addEventListener('DOMContentLoaded', () => document.getElementById('benilla').addEventListener('pointerup', () => { window.AIM = 0; }));
  window.LUAFN = async (c) => {
    await new Promise((ok) => setTimeout(ok, 10));
    if (/GetActionTexture/.test(c)) {
      return [Array(12).fill('|0.0|10|nil|0').join('\n'), 1, 1, 0, '', '', 0, 0, '', '', '', 'fight', '', tf.join(','), window.CAST, window.AIM];
    }
    if (/UseAction/.test(c)) { if (window.GROUND) window.AIM = 1; return [window.AIM]; }
    if (/SpellStopTargeting/.test(c)) { window.AIM = 0; return []; }
    if (/GetMouseFocus/.test(c)) { const m = at(); return [m.cx >= tf[0] && m.cx <= tf[2] && m.cy >= tf[1] && m.cy <= tf[3] ? 'TargetFrame' : 'WorldFrame']; }
    if (/UnitExists\("mouseover"\)/.test(c)) return [window.MOUSEOVER];
    return undefined;
  };
}
const world = (live = true) => open(browser, { state: 'inworld', live, init: live ? fakeWorld : null });
const clearLog = (p) => p.evaluate(() => { LOG.length = 0; FIRED.length = 0; });
const presses = (p) => p.evaluate(() => LOG.filter((e) => e.t === 'pointerdown').map((e) => [e.x, e.y]));

test('the reticle is the nearest-enemy button: through the bridge, and Tab without it', async () => {
  let { ctx, p, errors } = await world(true);
  await clearLog(p);
  await p.touchscreen.tap(...await centre(p, '#ow-touch .target'));
  await p.waitForTimeout(100);
  assert.deepEqual(await p.evaluate(() => FIRED), ['TARGETNEARESTENEMY']);
  assert.equal(await p.getAttribute('#ow-touch .target', 'aria-label'), 'Nearest enemy');
  assert.deepEqual(errors, []);
  await ctx.close();
  ({ ctx, p, errors } = await world(false));
  await clearLog(p);
  await p.touchscreen.tap(...await centre(p, '#ow-touch .target'));
  await p.waitForTimeout(150);
  assert.deepEqual(await p.evaluate(() => LOG.filter((e) => e.t === 'keydown').map((e) => e.key)), ['Tab'], 'no bridge: Tab');
  // and the last enemy without the bridge: Shift-Tab
  await p.evaluate(async () => { const { makeDriver } = await import('/touch-driver.js'); makeDriver({ ready: false }).fire('TARGETPREVIOUSENEMY'); });
  await p.waitForTimeout(150);
  assert.deepEqual(await p.evaluate(() => LOG.filter((e) => e.t === 'keydown').map((e) => [e.key, e.shift])).then((k) => k.at(-1)), ['Tab', true]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('swipe the target frame: left for the next enemy, right for the last; a tap there still clicks it', async () => {
  const { ctx, p, errors } = await world();
  await p.waitForFunction(() => T.inworld, null, { timeout: 3000 });
  await p.waitForTimeout(500);                       // a poll has said where the target frame is
  await clearLog(p);
  await finger(p, [[450, 40], [430, 41], [410, 42], [385, 43], [370, 43]]);
  await p.waitForTimeout(150);
  assert.deepEqual(await p.evaluate(() => FIRED), ['TARGETNEARESTENEMY'], 'left: the next one');
  assert.equal(await p.evaluate(() => T.taps.at(-1).how), 'swipe');
  await clearLog(p);
  await finger(p, [[350, 40], [370, 41], [395, 41], [420, 42], [440, 42]]);
  await p.waitForTimeout(150);
  assert.deepEqual(await p.evaluate(() => FIRED), ['TARGETPREVIOUSENEMY'], 'right: the last one');
  // the frame may have been pressed in while the game was asked what is under the finger, but a swipe lets it out off
  // the screen: no release on the frame (no click), and no camera drag
  const sent = await p.evaluate(() => LOG.filter((e) => e.t !== 'pointermove'));
  assert.deepEqual(sent.filter((e) => e.t === 'pointerup' && e.cx >= 300 && e.cx <= 500 && e.cy >= 10 && e.cy <= 70), [], 'no click on the frame');
  assert.deepEqual(sent.filter((e) => e.t === 'pointerdown' && e.b === 2), [], 'no camera drag');
  // a plain tap on the frame is a click on it
  await p.waitForTimeout(400);
  await clearLog(p);
  await p.touchscreen.tap(400, 40);
  await p.waitForFunction(() => LOG.some((e) => e.t === 'pointerup'), null, { timeout: 4000 });
  assert.deepEqual((await presses(p))[0], [400, 40], 'tap: clicked the frame');
  assert.deepEqual(await p.evaluate(() => FIRED), [], 'and no enemy change');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('soft lock: a tap on open ground keeps the target; on a unit it clicks; soft lock off clicks the ground', async () => {
  const { ctx, p, errors } = await world();
  await p.waitForFunction(() => T.inworld, null, { timeout: 3000 });
  await p.waitForTimeout(500);
  await clearLog(p);
  await p.touchscreen.tap(480, 150);
  await p.waitForFunction(() => T.taps.at(-1)?.how === 'kept', null, { timeout: 4000 });
  await p.waitForTimeout(150);
  assert.deepEqual(await presses(p), [], 'open ground: nothing clicked, the target stays');
  // a unit under the finger: the click goes in (another spot, past the double-tap time)
  await p.evaluate(() => { window.MOUSEOVER = 1; });
  await p.waitForTimeout(400);
  await clearLog(p);
  await p.touchscreen.tap(520, 170);
  await p.waitForFunction(() => LOG.some((e) => e.t === 'pointerup'), null, { timeout: 4000 });
  assert.deepEqual((await presses(p))[0], [520, 170], 'a unit: clicked');
  // soft lock off (Menu > Controls): open ground clicks again
  await p.evaluate(() => { window.MOUSEOVER = 0; });
  await p.evaluate(() => document.querySelector('#ow-touch [data-c="softlock"]').click());
  assert.equal(await p.textContent('#ow-touch [data-c="softlock"]'), 'Soft lock: off');
  await p.waitForTimeout(400);
  await clearLog(p);
  await p.touchscreen.tap(480, 150);
  await p.waitForFunction(() => LOG.some((e) => e.t === 'pointerup'), null, { timeout: 4000 });
  assert.deepEqual((await presses(p))[0], [480, 150], 'soft lock off: the ground tap clicks');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the cast bar: shown over the ring while casting, filling, a channel draining; gone after', async () => {
  const { ctx, p, errors } = await world();
  await p.waitForFunction(() => T.inworld, null, { timeout: 3000 });
  // a 2.5 s cast, 1 s in (the game's clock: start 1000 ms, end 3500 ms, now 2.000 s)
  await p.evaluate(() => { window.CAST = 'Fireball~Interface\\Icons\\Spell_Fire_FlameBolt~1000~3500~0~2.000'; });
  await p.waitForFunction(() => document.querySelector('#ow-touch .castbar').classList.contains('on'), null, { timeout: 2000 });
  assert.equal(await p.textContent('#ow-touch .castbar span'), 'Fireball');
  await p.waitForTimeout(100);
  const w1 = await p.evaluate(() => parseFloat(document.querySelector('#ow-touch .castbar i').style.width));
  assert.ok(w1 >= 35 && w1 <= 75, `about 40% in (${w1})`);
  // on screen and clear of the ring
  const boxes = await p.evaluate(() => ['.castbar', '.attack', '.target', '.key', '.dots', '.jump', '.tray'].flatMap((s) =>
    [...document.querySelectorAll('#ow-touch ' + s)].map((e) => { const r = e.getBoundingClientRect(); return { s, l: r.left, t: r.top, r: r.right, b: r.bottom }; })));
  const cb = boxes[0];
  assert.ok(cb.l >= 0 && cb.t >= 0 && cb.r <= 800 && cb.b <= 400, 'on screen');
  for (const o of boxes.slice(1)) {
    const apart = o.r <= cb.l || o.l >= cb.r || o.b <= cb.t || o.t >= cb.b;
    assert.ok(apart, `clear of ${o.s} (${[o.l, o.t, o.r, o.b].map(Math.round)} vs ${[cb.l, cb.t, cb.r, cb.b].map(Math.round)})`);
  }
  // a channel: the bar drains, and has the channel's colour
  await p.evaluate(() => { window.CAST = 'Arcane Missiles~x~1000~6000~1~2.000'; });
  await p.waitForFunction(() => document.querySelector('#ow-touch .castbar').classList.contains('chan'), null, { timeout: 2000 });
  await p.waitForTimeout(100);
  const w2 = await p.evaluate(() => parseFloat(document.querySelector('#ow-touch .castbar i').style.width));
  assert.ok(w2 >= 70 && w2 <= 82, `a channel 1 s into 5 s shows about 80% left (${w2})`);
  // done: gone
  await p.evaluate(() => { window.CAST = ''; });
  await p.waitForFunction(() => !document.querySelector('#ow-touch .castbar').classList.contains('on'), null, { timeout: 2000 });
  // the parser
  assert.deepEqual(await p.evaluate(() => T.parseCast('Heal~i~0~1500~0~0.500')), { name: 'Heal', icon: 'i', start: 0, end: 1500, channel: false, now: 500 });
  assert.equal(await p.evaluate(() => T.parseCast('')), null);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a ground-targeted spell: the next world tap places it there; the X cancels it', async () => {
  const { ctx, p, errors } = await world();
  await p.waitForFunction(() => T.inworld, null, { timeout: 3000 });
  await p.evaluate(() => { window.GROUND = 1; });
  await p.waitForTimeout(300);
  await p.touchscreen.tap(...await centre(p, '#ow-touch .key'));
  await p.waitForFunction(() => document.getElementById('ow-touch').classList.contains('aimground'), null, { timeout: 2000 });
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .gaim')).display), 'flex', 'says to tap the spot, with an X');
  assert.match(await p.textContent('#ow-touch .gaim'), /Tap where it should land/);
  await p.waitForTimeout(400);
  await clearLog(p);
  await p.touchscreen.tap(480, 150);                 // open ground, MOUSEOVER 0: soft lock would keep it, but this places
  await p.waitForFunction(() => LOG.some((e) => e.t === 'pointerup'), null, { timeout: 4000 });
  const d = await p.evaluate(() => LOG.find((e) => e.t === 'pointerdown'));
  assert.deepEqual([d.x, d.y], [480, 150], 'a left click on the spot');
  assert.equal(await p.evaluate(() => T.taps.at(-1).how), 'ground');
  await p.waitForFunction(() => !document.getElementById('ow-touch').classList.contains('aimground'), null, { timeout: 2000 });
  // again, and cancel
  await p.waitForTimeout(400);
  await p.touchscreen.tap(...await centre(p, '#ow-touch .key'));
  await p.waitForFunction(() => document.getElementById('ow-touch').classList.contains('aimground'), null, { timeout: 2000 });
  await p.waitForTimeout(500);
  await p.touchscreen.tap(...await centre(p, '#ow-touch .gaim .gx'));
  await p.waitForFunction(() => !document.getElementById('ow-touch').classList.contains('aimground'), null, { timeout: 2000 });
  assert.ok(await p.evaluate(() => RAN.some((c) => /SpellStopTargeting\(\)/.test(c))), 'SpellStopTargeting');
  assert.deepEqual(errors, []);
  await ctx.close();
});
