// Headless Chromium (Playwright), a phone in landscape, in the world: Menu > Controls > Edit layout, the layout code,
// and the controller card.
//   * Edit layout: the stick, the ring (by Attack), Jump and the tray take drags (a key fires nothing meanwhile); a drag
//     off the screen is clamped on it; Ring + makes the ring bigger; Done saves it for this character (the game's
//     UnitName and GetRealmName), it comes back after a reload, another character keeps the preset; Reset goes back;
//   * the layout code: L1 + 18 characters, used again it puts the same layout back; rubbish is refused;
//   * a controller connecting: touch controls hidden, a one-time "Controller connected" card with its buttons'
//     jobs (the mapper's own list when the bridge has one); unplugged, touch comes back; the card only once.
//
//   NODE_PATH=$(npm root -g) node --test stream/tests/touch-edit.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { chromium } = require('playwright');
const { open, finger, centre } = require('./touch-harness.cjs');

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

// the game says who is playing: CHAR (set before the page loads, kept in sessionStorage across reloads)
function whoIs() {
  window.LUAFN = async (c) => {
    if (/UnitName\("player"\)/.test(c)) return [sessionStorage.getItem('char') || 'Aria', 'Leyline'];
    return undefined;
  };
}
const ITEMS = ['.stick', '.attack', '.target', '.key', '.jump', '.dots', '.tray'];
const boxes = (p) => p.evaluate((sels) => sels.flatMap((s) => [...document.querySelectorAll('#ow-touch ' + s)].map((e, i) => {
  const r = e.getBoundingClientRect();
  return { k: s + (i || ''), l: r.left, t: r.top, r: r.right, b: r.bottom, x: (r.left + r.right) / 2, y: (r.top + r.bottom) / 2, w: r.width };
})), ITEMS);
const near = (a, b, tol = 3) => Math.abs(a - b) <= tol;

test('Edit layout: drag, clamp, resize, save per character, reload, reset and the layout code', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, init: whoIs });
  await p.waitForFunction(() => T.layoutKey === 'ow:touch-layout:Leyline/Aria', null, { timeout: 3000 });
  const before = await boxes(p);
  const atk0 = before.find((b) => b.k === '.attack');
  // Menu > Controls > Edit layout
  await p.evaluate(() => document.querySelector('#ow-touch [data-c="edit"]').click());
  assert.ok(await p.evaluate(() => document.getElementById('ow-touch').classList.contains('editing')), 'editing');
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .editbar')).display), 'flex', 'the edit bar shows');
  // a key takes a drag, not a press
  await p.evaluate(() => { RAN.length = 0; });
  await p.touchscreen.tap(...await centre(p, '#ow-touch .key'));
  await p.waitForTimeout(200);
  assert.deepEqual(await p.evaluate(() => RAN.filter((c) => /UseAction/.test(c))), [], 'no key fired while editing');
  // the ring, by Attack, to the middle of the screen
  await finger(p, [[atk0.x, atk0.y], [atk0.x - 100, atk0.y - 40], [atk0.x - 250, atk0.y - 80], [420, 220]]);
  let now = await boxes(p);
  let atk = now.find((b) => b.k === '.attack');
  assert.ok(near(atk.x, 420) && near(atk.y, 220), `Attack followed the finger (${Math.round(atk.x)},${Math.round(atk.y)})`);
  const k0 = before.find((b) => b.k === '.key'), k1 = now.find((b) => b.k === '.key');
  assert.ok(near(k1.x - atk.x, k0.x - atk0.x) && near(k1.y - atk.y, k0.y - atk0.y), 'the keys came with it');
  // the stick dragged far off the screen: kept on it
  const st0 = now.find((b) => b.k === '.stick');
  await finger(p, [[st0.x, st0.y], [st0.x - 40, st0.y + 40], [-150, 600]]);
  // Jump on its own, to the bottom edge and past it
  const j0 = now.find((b) => b.k === '.jump');
  await finger(p, [[j0.x, j0.y], [j0.x + 30, j0.y + 30], [600, 520]]);
  now = await boxes(p);
  for (const b of now) assert.ok(b.l >= 0 && b.t >= 0 && b.r <= 800 && b.b <= 400, `${b.k} kept on screen (${[b.l, b.t, b.r, b.b].map(Math.round)})`);
  const jump = now.find((b) => b.k === '.jump');
  assert.ok(near(jump.x, 600, 4), 'Jump went where it was put, across');
  // Ring +
  const w0 = now.find((b) => b.k === '.attack').w;
  await p.touchscreen.tap(...await centre(p, '#ow-touch [data-e="bigger"]'));
  await p.waitForTimeout(150);
  const w1 = (await boxes(p)).find((b) => b.k === '.attack').w;
  assert.ok(near(w1, w0 * 1.1, 2), `the ring is bigger (${w0} -> ${w1})`);
  // Done: saved for this character
  await p.touchscreen.tap(...await centre(p, '#ow-touch [data-e="done"]'));
  await p.waitForTimeout(150);
  assert.equal(await p.evaluate(() => document.getElementById('ow-touch').classList.contains('editing')), false);
  const saved = JSON.parse(await p.evaluate(() => localStorage.getItem('ow:touch-layout:Leyline/Aria')));
  assert.ok(saved && saved.z === 1.1 && saved.j, `saved: ${JSON.stringify(saved)}`);
  assert.match(await p.textContent('#ow-touch [data-c="preset"]'), /\(edited\)/);
  const code = await p.evaluate(() => T.layoutCode());
  assert.match(code, /^L1[0-9a-z]{18}$/);
  const edited = await boxes(p);
  // a reload: the same places
  await p.reload();
  await p.waitForFunction(() => window.ready && T.layoutKey === 'ow:touch-layout:Leyline/Aria' && T.layout?.custom, null, { timeout: 4000 });
  const again = await boxes(p);
  for (const b of edited) { const a = again.find((x) => x.k === b.k); assert.ok(near(a.x, b.x) && near(a.y, b.y), `${b.k} back where it was`); }
  // Reset: the preset again; the code puts the edited layout back
  await p.evaluate(() => { T.editMode(true); });
  await p.touchscreen.tap(...await centre(p, '#ow-touch [data-e="reset"]'));
  await p.waitForTimeout(150);
  assert.equal(await p.evaluate(() => T.layout.custom), false);
  const reset = (await boxes(p)).find((b) => b.k === '.attack');
  assert.ok(near(reset.x, atk0.x) && near(reset.y, atk0.y), 'Attack back in its corner');
  await p.fill('#ow-touch .editbar input', code);
  await p.touchscreen.tap(...await centre(p, '#ow-touch [data-e="use"]'));
  await p.waitForTimeout(200);
  const used = (await boxes(p)).find((b) => b.k === '.attack');
  const was = edited.find((b) => b.k === '.attack');
  assert.ok(near(used.x, was.x, 3) && near(used.y, was.y, 3), 'the code put the edited ring back');
  assert.equal(await p.evaluate(() => T.useLayoutCode('L1nonsense')), false, 'rubbish refused');
  assert.equal(await p.evaluate(() => M.decodeLayout('hello')), null);
  // the code survives a round trip
  const rt = await p.evaluate((c) => M.encodeLayout(M.decodeLayout(c)), code);
  assert.equal(rt, code);
  await p.evaluate(() => T.editMode(false));
  // another character: the preset
  await p.evaluate(() => sessionStorage.setItem('char', 'Bryn'));
  await p.reload();
  await p.waitForFunction(() => window.ready && T.layoutKey === 'ow:touch-layout:Leyline/Bryn', null, { timeout: 4000 });
  assert.equal(await p.evaluate(() => T.layout.custom), false, 'Bryn has the preset');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Edit layout waits for the world; Left-handed and Layout cycle from Menu > Controls', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'charselect' });
  await p.evaluate(() => document.querySelector('#ow-touch [data-c="edit"]').click());
  assert.equal(await p.evaluate(() => document.getElementById('ow-touch').classList.contains('editing')), false, 'not on the character screens');
  await p.evaluate(() => setScreen('inworld'));
  await p.waitForFunction(() => T.inworld, null, { timeout: 2000 });
  await p.evaluate(() => document.querySelector('#ow-touch [data-c="lefty"]').click());
  assert.equal(await p.textContent('#ow-touch [data-c="lefty"]'), 'Left-handed: on');
  assert.ok(await p.evaluate(() => T.layout.mirror && T.layout.ax < innerWidth / 2), 'the ring moved to the left');
  for (const want of ['Layout: Phone', 'Layout: Tablet', 'Layout: Auto']) {
    await p.evaluate(() => document.querySelector('#ow-touch [data-c="preset"]').click());
    assert.equal(await p.textContent('#ow-touch [data-c="preset"]'), want);
  }
  await p.evaluate(() => document.querySelector('#ow-touch [data-c="haptics"]').click());
  assert.equal(await p.textContent('#ow-touch [data-c="haptics"]'), 'Vibration: off');
  assert.equal(await p.evaluate(() => localStorage.getItem('ow:haptics')), 'off');
  // Bigger interface: the slider sets the game's interface size
  await p.evaluate(() => { const s = document.querySelector('#ow-touch [data-c="uisize"]'); s.value = '120'; s.dispatchEvent(new Event('input')); });
  assert.equal(await p.evaluate(() => localStorage.getItem('ow:touch-ui')), '1.2');
  assert.equal(await p.textContent('#ow-touch .slider .v'), '120%');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a controller: touch hidden, a one-time card with its buttons, touch back when it goes', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'inworld' });
  await p.waitForFunction(() => T.inworld, null, { timeout: 2000 });
  const pad = (type) => p.evaluate((t) => { const e = new Event(t); e.gamepad = { id: 'Xbox Wireless Controller (STANDARD GAMEPAD Vendor: 045e)', mapping: 'standard' }; dispatchEvent(e); }, type);
  await pad('gamepadconnected');
  await p.waitForTimeout(100);
  assert.equal(await p.evaluate(() => T.on), false, 'touch controls off');
  const card = await p.evaluate(() => { const i = document.querySelector('#ow-touch .info'); return { shown: getComputedStyle(i).display, text: i.innerText }; });
  assert.equal(card.shown, 'flex', 'the card shows');
  assert.match(card.text, /Controller connected/);
  assert.match(card.text, /Xbox Wireless Controller/);
  assert.match(card.text, /Left stick\s+Move/);
  assert.match(card.text, /Jump/);
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .attack')).display), 'none', 'the ring is hidden');
  assert.equal(await p.evaluate(() => localStorage.getItem('ow:pad-card')), '1');
  await pad('gamepaddisconnected');
  await p.waitForTimeout(100);
  assert.equal(await p.evaluate(() => T.on), true, 'touch back');
  // again: no card the second time, the mapper's own list if it has one
  await p.evaluate(() => { document.querySelector('#ow-touch .info').classList.remove('on'); window.wenilla.gamepad.mapping = [['A', 'Jump'], ['RT', 'Attack']]; });
  await pad('gamepadconnected');
  await p.waitForTimeout(100);
  assert.equal(await p.evaluate(() => T.padCard), 1, 'the card only once');
  assert.equal(await p.evaluate(() => document.querySelector('#ow-touch .info').classList.contains('on')), false);
  // the mapper's list, when the card does show (a new device)
  await p.evaluate(() => localStorage.removeItem('ow:pad-card'));
  await pad('gamepaddisconnected');
  await pad('gamepadconnected');
  await p.waitForTimeout(100);
  assert.match(await p.evaluate(() => document.querySelector('#ow-touch .info').innerText), /RT\s+Attack/);
  assert.deepEqual(errors, []);
  await ctx.close();
});
