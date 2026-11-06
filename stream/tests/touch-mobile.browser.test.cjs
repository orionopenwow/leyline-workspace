// Headless Chromium (Playwright) with real touch emulation (hasTouch, isMobile, CDP touch points): the mobile pass of
// 5 Nov night (owner: "Do a mobile controls pass", "bug fix features to everything that makes sense"). touch.js build
// 2026-11-05c. Each test is one thing a phone does:
//   * phones and a tablet, landscape and portrait, with a notch: every control on screen and clear of the notch, the
//     turn-sideways cover on a phone held upright only, one scripted click per tap on the character screens;
//   * the name box with a phone keyboard that composes words (Android): each letter reaches the game once;
//   * the keyboard button and the chat box focus inside the lift (a phone raises its keyboard only then);
//   * the chat sheet sits above the phone's keyboard (visualViewport);
//   * a quick pinch ends in no click; a slow one zooms; a two-finger tap right-clicks (also on a bag item);
//   * Touch and hold: right-click;
//   * the page hidden lets go of the stick, Jump and a camera drag, and asks for the screen lock again;
//   * a lost lift doesn't jam the stick or turn the next tap into a pinch;
//   * a hardware keyboard hides the thumb controls until the next touch;
//   * Button size, Opacity, vibration (12 ms; the iPhone switch), no page zoom, the debug line's new fields.
//
//   NODE_PATH=$(npm root -g) node --test stream/tests/touch-mobile.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { chromium } = require('playwright');
const { open, finger, centre } = require('./touch-harness.cjs');

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

const IPHONE_UA = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1';
const ANDROID_UA = 'Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36';
const DEVICES = [
  { name: 'iPhone', w: 844, h: 390, dpr: 3, ua: IPHONE_UA, notch: { left: 47, right: 47, top: 0, bottom: 21 }, phone: true },
  { name: 'Android phone', w: 915, h: 412, dpr: 2.625, ua: ANDROID_UA, notch: { left: 0, right: 0, top: 0, bottom: 0 }, phone: true },
  { name: 'tablet', w: 1180, h: 820, dpr: 2, ua: ANDROID_UA.replace('Mobile ', ''), notch: { left: 0, right: 0, top: 24, bottom: 20 }, phone: false },
];

async function device(d, portrait = false) {
  const [w, h] = portrait ? [d.h, d.w] : [d.w, d.h];
  return browser.newContext({ viewport: { width: w, height: h }, hasTouch: true, isMobile: true, deviceScaleFactor: d.dpr, userAgent: d.ua });
}
async function notch(p, n) {
  const cdp = await p.context().newCDPSession(p);
  await cdp.send('Emulation.setSafeAreaInsetsOverride', { insets: n });
  await p.evaluate(() => dispatchEvent(new Event('resize')));
  await p.waitForTimeout(150);
}
/** Several fingers by CDP: frames = [[type, [{x, y, id}, ...]], ...] (touchStart/Move/End with every finger still down). */
async function fingers(p, frames, ms = 30) {
  const cdp = await p.context().newCDPSession(p);
  for (const [type, pts] of frames) {
    await cdp.send('Input.dispatchTouchEvent', { type, touchPoints: pts });
    await p.waitForTimeout(ms);
  }
  await cdp.detach();
}
const presses = (p) => p.evaluate(() => LOG.filter((e) => e.t === 'pointerdown' || e.t === 'pointerup').map((e) => `${e.t === 'pointerdown' ? 'd' : 'u'}${e.b}@${e.cx},${e.cy}`));
const box = (p, sel) => p.evaluate((s) => { const r = document.querySelector(s).getBoundingClientRect(); return { l: r.left, t: r.top, r: r.right, b: r.bottom }; }, sel);

for (const d of DEVICES) {
  test(`${d.name}, landscape with its notch: every control on screen and clear of the notch; the game frames too`, async () => {
    const ctx = await device(d);
    const { p, errors } = await open(browser, { ctx, state: 'inworld', live: true, w: d.w, h: d.h });
    await notch(p, d.notch);
    const L = await p.evaluate(() => T.layout);
    assert.deepEqual(L.safe, [d.notch.left, d.notch.right, d.notch.top, d.notch.bottom], 'the notch is measured');
    for (const sel of ['.attack', '.target', '.jump', '.tray .handle', '.dots', ...[0, 1, 2, 3].map((i) => `.key[data-k="${i}"]`)]) {
      const b = await box(p, '#ow-touch ' + sel);
      assert.ok(b.l >= d.notch.left - 0.5 && b.r <= d.w - d.notch.right + 0.5 && b.t >= d.notch.top - 0.5 && b.b <= d.h - d.notch.bottom + 0.5,
        `${sel} inside the safe area: ${JSON.stringify(b)}`);
    }
    // the game's own player frame and minimap move in by the notch
    const lay = await p.evaluate(() => T.luaChunks().layoutOn);
    assert.match(lay, new RegExp(`place\\("PlayerFrame", "TOPLEFT", "TOPLEFT", \\(16 \\+ ${d.notch.left}\\) \\* u`));
    assert.match(lay, new RegExp(`local drawer|-${10 + 36 + 8 + d.notch.right} \\* u`));
    assert.deepEqual(errors, []);
    await ctx.close();
  });

  test(`${d.name}, character list: the keyboard and Controls buttons clear the notch; a tap is one scripted click`, async () => {
    const ctx = await device(d);
    const { p, errors } = await open(browser, { ctx, state: 'charselect', live: false, w: d.w, h: d.h });
    await notch(p, d.notch);
    for (const sel of ['.kbd', '.gear']) {
      const b = await box(p, '#ow-touch ' + sel);
      assert.ok(b.r <= d.w - d.notch.right - 4 && b.t >= d.notch.top + 4, `${sel} clear of the notch: ${JSON.stringify(b)}`);
    }
    await p.evaluate(() => { LOG.length = 0; });
    await finger(p, [[Math.round(d.w * 0.5), Math.round(d.h * 0.6)]], 40);
    await p.waitForFunction(() => LOG.some((e) => e.t === 'pointerup'), null, { timeout: 3000 });
    await p.waitForTimeout(300);
    const log = await p.evaluate(() => LOG);
    assert.deepEqual(log.filter((e) => e.trusted), [], 'no raw touch reached the game');
    assert.equal(log.filter((e) => e.t === 'pointerdown').length, 1, 'one press');
    assert.ok(log.filter((e) => e.t === 'pointerdown').every((e) => e.pt === 'mouse' && e.b === 0), 'a left mouse press');
    assert.deepEqual(errors, []);
    await ctx.close();
  });

  test(`${d.name}, portrait: ${d.phone ? 'asked to turn sideways' : 'plays upright (no cover on a tablet)'}`, async () => {
    const ctx = await device(d, true);
    const { p, errors } = await open(browser, { ctx, state: 'charselect', w: d.h, h: d.w });
    const shown = await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .rotate')).display);
    assert.equal(shown, d.phone ? 'flex' : 'none');
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

test('the name box with a composing phone keyboard: each letter reaches the game once; autocorrect and Backspace too', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'charcreate', w: 844, h: 390 });
  await p.evaluate(() => { window.KEYS = []; document.getElementById('benilla').addEventListener('keydown', (e) => KEYS.push(e.key)); });
  await finger(p, [await centre(p, '#ow-touch .kbd')], 60);
  await p.waitForTimeout(100);
  assert.equal(await p.evaluate(() => document.activeElement.className), 'ghost', 'the box has the keyboard');
  assert.ok(await p.evaluate(() => document.querySelector('#ow-touch').classList.contains('ghosting')), 'the typed-text echo shows');
  const cdp = await ctx.newCDPSession(p);
  for (const t of ['B', 'Bo', 'Bob']) { await cdp.send('Input.imeSetComposition', { text: t, selectionStart: t.length, selectionEnd: t.length }); await p.waitForTimeout(20); }
  await cdp.send('Input.insertText', { text: 'Bob' });
  await p.waitForTimeout(60);
  assert.deepEqual(await p.evaluate(() => KEYS), ['B', 'o', 'b'], 'composed "Bob" is three keys');
  assert.equal(await p.textContent('#ow-touch .kecho'), 'Bob');
  // autocorrect: "Bob" becomes "Rob" (the word replaced): Backspaces and the new letters
  await p.evaluate(() => { KEYS.length = 0; const g = document.querySelector('#ow-touch .ghost'); g.value = g.value.replace(/Bob$/, 'Rob'); g.dispatchEvent(new InputEvent('input', { inputType: 'insertReplacementText' })); });
  assert.deepEqual(await p.evaluate(() => KEYS), ['Backspace', 'Backspace', 'Backspace', 'R', 'o', 'b']);
  // Backspace past the typed text still deletes in the game (the box is padded), and Enter goes through
  await p.evaluate(() => { KEYS.length = 0; });
  for (let i = 0; i < 5; i++) await p.keyboard.press('Backspace');
  await p.keyboard.press('Enter');
  assert.deepEqual(await p.evaluate(() => KEYS), ['Backspace', 'Backspace', 'Backspace', 'Backspace', 'Backspace', 'Enter']);
  assert.notEqual(await p.evaluate(() => document.activeElement.className), 'ghost', 'Enter puts the keyboard away');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the keyboard button and the chat box focus inside the lift (a phone raises its keyboard only then)', async () => {
  const init = () => {
    window.FOCUSES = [];
    const f0 = HTMLElement.prototype.focus;
    HTMLElement.prototype.focus = function (...a) { if (this.tagName === 'INPUT') FOCUSES.push(`${this.className || this.closest('.sheet')?.className || 'input'}:${window.event?.type || 'none'}`); return f0.apply(this, a); };
  };
  {
    const { ctx, p, errors } = await open(browser, { state: 'charcreate', w: 844, h: 390, init });
    await finger(p, [await centre(p, '#ow-touch .kbd')], 80);
    await p.waitForTimeout(100);
    assert.deepEqual(await p.evaluate(() => FOCUSES), ['ghost:pointerup']);
    // a second tap puts it away
    await finger(p, [await centre(p, '#ow-touch .kbd')], 60);
    await p.waitForTimeout(100);
    assert.notEqual(await p.evaluate(() => document.activeElement.className), 'ghost');
    assert.deepEqual(errors, []);
    await ctx.close();
  }
  {
    const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 844, h: 390, init });
    await finger(p, [await centre(p, '#ow-touch .tray .handle')], 40);
    await p.waitForTimeout(150);
    await finger(p, [await centre(p, '#ow-touch .tray [data-act="chat"]')], 80);
    await p.waitForTimeout(150);
    assert.ok(await p.evaluate(() => document.querySelector('#ow-touch .sheet.chat').classList.contains('open')), 'the chat sheet opened');
    assert.deepEqual(await p.evaluate(() => FOCUSES.filter((f) => /chat/.test(f))), ['sheet chat open:pointerup']);
    assert.deepEqual(errors, []);
    await ctx.close();
  }
});

test('the chat sheet sits above the phone keyboard (visualViewport), and comes back down when it closes', async () => {
  // a visual viewport the test can shrink, as a phone keyboard does (the page itself keeps its size)
  const init = () => {
    const vv = new EventTarget();
    Object.assign(vv, { width: innerWidth, height: innerHeight, offsetTop: 0, offsetLeft: 0, scale: 1 });
    Object.defineProperty(window, 'visualViewport', { configurable: true, get: () => vv });
    window.KB = (px) => { vv.height = innerHeight - px; vv.dispatchEvent(new Event('resize')); };
  };
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 844, h: 390, init });
  await finger(p, [await centre(p, '#ow-touch .tray .handle')], 40);
  await p.waitForTimeout(150);
  await finger(p, [await centre(p, '#ow-touch .tray [data-act="chat"]')], 60);
  await p.waitForTimeout(150);
  const before = await box(p, '#ow-touch .sheet.chat');
  assert.ok(before.b > 390 - 30, 'at the bottom with no keyboard');
  await p.evaluate(() => KB(200));
  await p.waitForTimeout(80);
  const up = await box(p, '#ow-touch .sheet.chat');
  assert.ok(up.b <= 390 - 200, `above the keyboard: bottom ${up.b}`);
  assert.ok(up.t >= 0, `and its top on screen: ${up.t}`);
  const inp = await box(p, '#ow-touch .sheet.chat input');
  assert.ok(inp.b <= 190 && inp.t >= 0, 'the box being typed in is visible');
  await p.evaluate(() => KB(0));
  await p.waitForTimeout(80);
  assert.ok((await box(p, '#ow-touch .sheet.chat')).b > 390 - 30, 'back down');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a quick pinch ends in no click; a slow pinch zooms; the last finger never clicks', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 844, h: 390 });
  await p.evaluate(() => { LOG.length = 0; FIRED.length = 0; });
  const A = { x: 380, y: 150, id: 1 }, B = { x: 460, y: 150, id: 2 };
  await fingers(p, [['touchStart', [A]], ['touchStart', [A, B]], ['touchMove', [{ ...A, x: 350 }, { ...B, x: 500 }]],
    ['touchEnd', [{ ...A, x: 350 }]], ['touchEnd', []]]);
  await p.waitForTimeout(600);
  assert.deepEqual(await presses(p), [], 'no click after the pinch');
  assert.ok((await p.evaluate(() => FIRED)).filter((c) => c === 'CAMERAZOOMIN').length >= 2, 'it zoomed in');
  // a slower pinch out, the fingers lifting the other way round
  await p.evaluate(() => { LOG.length = 0; FIRED.length = 0; });
  const frames = [['touchStart', [A]], ['touchStart', [A, B]]];
  for (let k = 1; k <= 5; k++) frames.push(['touchMove', [{ ...A, x: 380 + 8 * k }, { ...B, x: 460 - 8 * k }]]);
  frames.push(['touchEnd', [{ ...A, x: 420 }]], ['touchEnd', []]);
  await fingers(p, frames, 60);
  await p.waitForTimeout(600);
  assert.deepEqual(await presses(p), [], 'no click');
  assert.ok((await p.evaluate(() => FIRED)).includes('CAMERAZOOMOUT'), 'it zoomed out');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a two-finger tap right-clicks under the first finger, in the world and on a bag item', async () => {
  for (const on of ['', 'ContainerFrame1Item3']) {
    const init = on ? `window.LUAFN = async (c) => (/GetMouseFocus/.test(c) ? ['${on}'] : undefined);` : null;
    const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 844, h: 390, query: 'tapscale=1', init: init ? new Function(init) : null });
    await p.evaluate(() => { LOG.length = 0; });
    const A = { x: 400, y: 160, id: 1 }, B = { x: 470, y: 200, id: 2 };
    await fingers(p, [['touchStart', [A]], ['touchStart', [A, B]], ['touchEnd', [B]], ['touchEnd', []]], 25);
    await p.waitForFunction(() => LOG.some((e) => e.t === 'pointerup' && e.b === 2), null, { timeout: 3000 });
    await p.waitForTimeout(300);
    const pr = await presses(p);
    assert.ok(pr.includes('d2@400,160') && pr.includes('u2@400,160'), `${on || 'world'}: a right-click at the first finger: ${pr}`);
    assert.ok(!pr.some((x) => /^[du]0@400,160/.test(x)), `${on || 'world'}: no left press or click there first (a bag item would be picked up): ${pr}`);
    const last = await p.evaluate(() => T.taps.at(-1));
    assert.equal(last.how, 'two');
    assert.deepEqual(errors, []);
    await ctx.close();
  }
});

test('Touch and hold: right-click (Menu > Controls) right-clicks once, and the lift adds nothing', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 844, h: 390, query: 'tapscale=1', init: () => localStorage.setItem('ow:hold', 'right') });
  assert.match(await p.textContent('#ow-touch [data-c="hold"]'), /right-click/);
  await p.evaluate(() => { LOG.length = 0; });
  await finger(p, [[420, 170], [421, 170]], 700);
  await p.waitForTimeout(400);
  const pr = await presses(p);
  assert.deepEqual(pr.filter((x) => /@420,170/.test(x)), ['d2@420,170', 'u2@420,170'], `one right-click: ${pr}`);
  assert.equal(await p.evaluate(() => T.taps.at(-1).how), 'hold');
  // on a window (a bag item): no left press first, and a quick tap there still left-clicks
  await p.evaluate(() => { window.LUAFN = async (c) => (/GetMouseFocus/.test(c) ? ['ContainerFrame1Item3'] : undefined); LOG.length = 0; });
  await finger(p, [[420, 170], [421, 170]], 700);
  await p.waitForTimeout(400);
  assert.deepEqual((await presses(p)).filter((x) => /@420,170/.test(x)), ['d2@420,170', 'u2@420,170'], 'a bag item: right-click only');
  await p.evaluate(() => { LOG.length = 0; });
  await finger(p, [[420, 170]], 60);
  await p.waitForTimeout(500);
  assert.deepEqual((await presses(p)).filter((x) => /@420,170/.test(x)), ['d0@420,170', 'u0@420,170'], 'a quick tap on it: a left click');
  await p.evaluate(() => { window.LUAFN = null; });
  // the setting back to tooltips: a hold is a hover again (no press at all)
  await p.evaluate(() => { localStorage.removeItem('ow:hold'); LOG.length = 0; });
  await finger(p, [[420, 170], [421, 170]], 700);
  await p.waitForTimeout(400);
  assert.deepEqual(await presses(p), []);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the page hidden (another app, a call) lets go of the stick, Jump and a camera drag; the screen lock comes back', async () => {
  const init = () => {
    window.WAKE = 0;
    Object.defineProperty(navigator, 'wakeLock', { configurable: true, value: { request: async () => { WAKE += 1; const s = new EventTarget(); s.release = async () => {}; return s; } } });
    window.HIDE = (h) => {
      Object.defineProperty(document, 'hidden', { configurable: true, get: () => h });
      Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => (h ? 'hidden' : 'visible') });
      document.dispatchEvent(new Event('visibilitychange'));
    };
  };
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 844, h: 390, init });
  assert.equal(await p.evaluate(() => WAKE), 1, 'the screen lock asked for in the world');
  const cdp = await ctx.newCDPSession(p);
  const S = { x: 120, y: 300, id: 1 }, J = { ...(await centre(p, '#ow-touch .jump')).reduce((o, v, i) => ({ ...o, [i ? 'y' : 'x']: v }), {}), id: 2 }, C = { x: 450, y: 150, id: 3 };
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [S] });
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ ...S, y: 240 }] });
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ ...S, y: 240 }, J] });
  await p.waitForTimeout(60);
  assert.deepEqual(await p.evaluate(() => HELD), [['MOVEFORWARD', true], ['JUMP', true]]);
  await p.evaluate(() => { LOG.length = 0; HELD.length = 0; });
  await p.evaluate(() => HIDE(true));
  await p.waitForTimeout(80);
  const held = await p.evaluate(() => HELD);
  assert.deepEqual(held.sort(), [['JUMP', false], ['MOVEFORWARD', false]], `let go: ${JSON.stringify(held)}`);
  assert.ok(await p.evaluate(() => !document.querySelector('#ow-touch .stick').classList.contains('live')), 'the stick went home');
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await p.evaluate(() => HIDE(false));
  await p.waitForTimeout(80);
  assert.equal(await p.evaluate(() => WAKE), 2, 'the screen lock asked for again');
  // a camera drag let go the same way (its button released)
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [C] });
  for (let k = 1; k <= 4; k++) { await p.waitForTimeout(30); await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ ...C, x: 450 + 15 * k }] }); }
  await p.waitForTimeout(100);
  await p.evaluate(() => { LOG.length = 0; HIDE(true); });
  await p.waitForTimeout(80);
  assert.ok((await presses(p)).some((x) => x.startsWith('u2')), 'the camera drag\'s button let go');
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  assert.ok(await p.evaluate(() => T.letGo >= 2));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a lost lift does not jam the stick or turn the next tap into a pinch', async () => {
  // a pointer the browser never ends (it lost the lift): made by script, so capturing it is skipped
  const init = () => { Element.prototype.setPointerCapture = function () {}; };
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 844, h: 390, query: 'tapscale=1', init });
  await p.evaluate(() => {
    const ghost = (sel, x, y, id) => document.querySelector(sel).dispatchEvent(new PointerEvent('pointerdown', { pointerId: id, isPrimary: true, pointerType: 'touch', clientX: x, clientY: y, bubbles: true, cancelable: true }));
    ghost('#ow-touch .stick-zone', 120, 300, 91);
    ghost('#ow-touch .world', 500, 120, 92);
    HELD.length = 0; LOG.length = 0;
  });
  // a real finger on the stick walks
  await finger(p, [[120, 300], [120, 240], [120, 236]], 40);
  assert.deepEqual((await p.evaluate(() => HELD)).filter(([c]) => c === 'MOVEFORWARD'), [['MOVEFORWARD', true], ['MOVEFORWARD', false]], 'the stick works');
  // and a real tap in the world clicks (it is no "pinch" with the lost finger)
  await finger(p, [[430, 160]], 40);
  await p.waitForFunction(() => LOG.some((e) => e.t === 'pointerup' && e.b === 0), null, { timeout: 3000 });
  assert.ok((await presses(p)).includes('d0@430,160'));
  assert.ok(await p.evaluate(() => T.stale >= 2), 'the lost ones were counted');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a hardware keyboard hides the thumb controls until the next touch (and the setting turns that off)', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 1180, h: 820 });
  const vis = () => p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .attack')).visibility);
  assert.equal(await vis(), 'visible');
  await p.evaluate(() => document.getElementById('benilla').focus());
  await p.keyboard.press('KeyW');
  await p.waitForTimeout(50);
  assert.equal(await vis(), 'hidden', 'hidden by a real W');
  assert.ok(await p.evaluate(() => T.kbdMode));
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .tray')).visibility), 'visible', 'the tray stays');
  await finger(p, [[600, 300]], 40);
  await p.waitForTimeout(50);
  assert.equal(await vis(), 'visible', 'back with a touch');
  // the keys this layer sends itself (no bridge: key presses) and typing in a box never count
  await p.evaluate(() => document.getElementById('benilla').dispatchEvent(new KeyboardEvent('keydown', { key: 'w', bubbles: true })));
  assert.equal(await vis(), 'visible');
  await p.evaluate(() => localStorage.setItem('ow:kbdhide', 'off'));
  await p.keyboard.press('KeyW');
  assert.equal(await vis(), 'visible', 'the setting off: they stay');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Button size and Opacity: the sliders resize the ring and the stick, fade the controls, and are kept', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 1180, h: 820 });
  const s0 = await p.evaluate(() => T.layout.s);
  const slide = (c, v) => p.evaluate(([c, v]) => { const el = document.querySelector(`#ow-touch [data-c="${c}"]`); el.value = String(v); el.dispatchEvent(new Event('input')); }, [c, v]);
  await slide('size', 130);
  const L = await p.evaluate(() => T.layout);
  assert.ok(Math.abs(L.s - Math.min(1.5, s0 * 1.3)) < 0.02, `the ring grew (to at most 150%): ${s0} -> ${L.s}`);
  assert.equal(L.stick, 1.3);
  assert.equal(await p.evaluate(() => document.querySelector('#ow-touch .stick').style.scale), '1.3');
  await slide('alpha', 50);
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .attack')).opacity), '0.5');
  assert.match(await p.textContent('#ow-touch [data-c="alpha"] + .v'), /50%/);
  await p.reload();
  await p.waitForFunction(() => window.ready, null, { timeout: 10000 });
  await p.waitForTimeout(300);
  assert.equal(await p.evaluate(() => T.layout.stick), 1.3, 'kept after a reload');
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .attack')).opacity), '0.5');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('vibration: 12 ms on a press where the phone has it; an iPhone gets the switch tick; Vibration off is silent', async () => {
  {
    const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 844, h: 390, init: () => { window.VIB = []; navigator.vibrate = (ms) => { VIB.push(ms); return true; }; } });
    assert.equal(await p.evaluate(() => T.haptics), 'vibrate');
    await finger(p, [await centre(p, '#ow-touch .target')], 40);
    assert.deepEqual(await p.evaluate(() => VIB), [12]);
    await p.evaluate(() => localStorage.setItem('ow:haptics', 'off'));
    await finger(p, [await centre(p, '#ow-touch .target')], 40);
    assert.deepEqual(await p.evaluate(() => VIB), [12], 'off: nothing');
    assert.deepEqual(errors, []);
    await ctx.close();
  }
  {
    const ctx = await browser.newContext({ viewport: { width: 844, height: 390 }, hasTouch: true, isMobile: true, deviceScaleFactor: 3, userAgent: IPHONE_UA });
    const init = () => {
      delete Navigator.prototype.vibrate;
      Object.defineProperty(HTMLInputElement.prototype, 'switch', { configurable: true, get() { return this.hasAttribute('switch'); } });
      window.TICKS = 0;
      document.addEventListener('click', (e) => { if (e.target.matches?.('.hap input')) TICKS += 1; }, true);
    };
    const { p, errors } = await open(browser, { ctx, state: 'inworld', live: true, w: 844, h: 390, init });
    assert.equal(await p.evaluate(() => T.haptics), 'switch');
    await p.evaluate(() => document.getElementById('benilla').focus());
    await finger(p, [await centre(p, '#ow-touch .target')], 40);
    assert.equal(await p.evaluate(() => TICKS), 1, 'one tick');
    assert.equal(await p.evaluate(() => document.activeElement.id), 'benilla', 'the game keeps the focus');
    assert.deepEqual(errors, []);
    await ctx.close();
  }
});

test('no page zoom while touch is on: the viewport tag, the iPhone gesture events, the panels; put back when off', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 844, h: 390 });
  const vp = () => p.evaluate(() => document.querySelector('meta[name=viewport]').content);
  assert.match(await vp(), /maximum-scale=1,user-scalable=no$/);
  assert.equal(await p.evaluate(() => { const e = new Event('gesturestart', { cancelable: true }); document.dispatchEvent(e); return e.defaultPrevented; }), true);
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .sheet.menu')).touchAction), 'pan-x pan-y');
  assert.equal(await p.evaluate(() => getComputedStyle(document.body).overscrollBehaviorY), 'none');
  assert.equal(await p.evaluate(() => getComputedStyle(document.getElementById('benilla')).touchAction), 'none');
  await p.evaluate(() => T.turnOff());
  assert.equal(await vp(), 'width=device-width,initial-scale=1,viewport-fit=cover');
  assert.equal(await p.evaluate(() => { const e = new Event('gesturestart', { cancelable: true }); document.dispatchEvent(e); return e.defaultPrevented; }), false);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Copy debug info in the world: the layout (not "?"), the notch, the keyboard, haptics and the new settings', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: true, w: 844, h: 390 });
  await p.waitForTimeout(1500);                     // the game layout has been applied (it used to overwrite the layout)
  const line = await p.evaluate(() => T.debugReport());
  for (const want of ['2026-11-06a', 'layout phone', 'size 100 alpha 100 hold tip', 'safe 0,0,0,0', 'vv 844x390 kb 0', 'haptics', 'two 0', 'stale 0', 'letgo 0']) {
    assert.ok(line.includes(want), `has "${want}": ${line}`);
  }
  assert.ok(!line.includes('\n'));
  assert.doesNotMatch(line, /World of Warcraft|\bWoW\b|Warcraft|Lich King|Blizzard/i);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the tray (no bridge, as on the live page): nothing covers it; Bags and Quests open on the lift (pushed in and buzzing on the press), Chat and Menu open their sheets', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'inworld', live: false, w: 844, h: 390 });
  await finger(p, [await centre(p, '#ow-touch .tray .handle')], 40);
  await p.waitForTimeout(120);
  const cdp = await ctx.newCDPSession(p);
  const [bx, by] = await centre(p, '#ow-touch .tray [data-act="bags"]');
  // no bridge (the live client today): the screen-switch button used to sit on top of Bags
  for (const act of ['bags', 'quests', 'chat', 'menu']) {
    const [x, y] = await centre(p, `#ow-touch .tray [data-act="${act}"]`);
    assert.equal(await p.evaluate(([x, y]) => document.elementFromPoint(x, y).closest('.rbtn')?.dataset.act, [x, y]), act, `${act} is on top`);
  }
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: bx, y: by }] });
  await p.waitForTimeout(60);
  assert.ok(await p.evaluate(() => document.querySelector('#ow-touch .tray [data-act="bags"]').classList.contains('pressed')), 'shows pushed');
  const bKeys = () => p.evaluate(() => LOG.filter((e) => e.t === 'keydown' && e.key === 'b').length);
  assert.equal(await bKeys(), 0, 'nothing yet');
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await p.waitForTimeout(60);
  assert.equal(await bKeys(), 1, 'Bags (its B key, no bridge) on the lift');
  assert.ok(await p.evaluate(() => document.querySelector('#ow-touch .closer').classList.contains('on')), 'Close shows');
  await finger(p, [await centre(p, '#ow-touch .tray .handle')], 40);
  await p.waitForTimeout(120);
  await finger(p, [await centre(p, '#ow-touch .tray [data-act="menu"]')], 40);
  await p.waitForTimeout(120);
  assert.ok(await p.evaluate(() => document.querySelector('#ow-touch .sheet.menu').classList.contains('open')), 'the Menu sheet');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the character screens with no bridge: the screen-switch button and the Controls button are both reachable', async () => {
  const { ctx, p, errors } = await open(browser, { state: 'charselect', live: false, w: 844, h: 390 });
  for (const sel of ['.flip', '.gear', '.kbd']) {
    const [x, y] = await centre(p, '#ow-touch ' + sel);
    assert.ok(await p.evaluate(([x, y, s]) => !!document.elementFromPoint(x, y).closest('#ow-touch ' + s), [x, y, sel]), `${sel} is on top at its centre`);
  }
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('turning the phone mid-game: upright asks to turn back, sideways again lays every control out on the new screen', async () => {
  const ctx = await device(DEVICES[0]);
  const { p, errors } = await open(browser, { ctx, state: 'inworld', live: true, w: 844, h: 390 });
  await p.setViewportSize({ width: 390, height: 844 });
  await p.waitForTimeout(200);
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .rotate')).display), 'flex');
  await p.setViewportSize({ width: 844, height: 390 });
  await p.waitForTimeout(200);
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .rotate')).display), 'none');
  const L = await p.evaluate(() => T.layout);
  assert.deepEqual([L.W, L.H], [844, 390]);
  for (const sel of ['.attack', '.jump', '.tray .handle', '.key[data-k="3"]']) {
    const b = await box(p, '#ow-touch ' + sel);
    assert.ok(b.l >= 0 && b.r <= 844 && b.t >= 0 && b.b <= 390, `${sel} on screen after the turn`);
  }
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('played from the home screen (the installed app): no full-screen prompt or install button, "app" on the debug line', async () => {
  const init = () => {
    const mm = window.matchMedia.bind(window);
    window.matchMedia = (q) => (/display-mode:\s*(fullscreen|standalone)/.test(q) ? { matches: true, media: q, addEventListener() {}, removeEventListener() {} } : mm(q));
    try { sessionStorage.removeItem('ow:fs-skip'); } catch { /* fine */ }
    try { Object.defineProperty(Document.prototype, 'fullscreenEnabled', { get: () => true }); } catch { /* fine */ }
  };
  const ctx = await device(DEVICES[1]);
  const { p, errors } = await open(browser, { ctx, state: 'charselect', w: 915, h: 412, init });
  const root = await p.evaluate(() => document.querySelector('#ow-touch').className);
  assert.doesNotMatch(root, /\bcanfs\b/, 'no full-screen offer in the app');
  assert.doesNotMatch(root, /\bcaninstall\b/, 'no install button in the app');
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('#ow-touch .fsgo')).display), 'none');
  assert.match(await p.evaluate(() => T.debugReport()), / app \|/);
  assert.deepEqual(errors, []);
  await ctx.close();
});
