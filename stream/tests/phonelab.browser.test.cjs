// Phone Lab's recorder (stream/phonelab.py RECORDER_JS) in a real phone (phone-sim.cjs: Pixel 7, CDP touches) against the
// website on loopback (fake services). A session is written straight into the portal's Phone Lab folder (starting one
// needs a staff login: tests/test_phonelab.py covers that), and this phone gets its cookie. Then:
//   * the site's pages carry the recorder and its banner; a real tap is recorded with its target, words and the event
//     sequence (touchstart -> pointerdown -> touchend -> pointerup -> click), and a sketch of the screen;
//   * what is typed into a field never leaves the phone, and the sign-in pages give no words and no sketch;
//   * the game page (another origin) loads it through wrath-early.js and its batches still arrive; a full screen ask
//     is recorded with whether it was a user gesture;
//   * Stop (a real tap on the banner) ends the session: the cookie goes, nothing more is recorded.
//
//   node --test tests/phonelab.browser.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { chromium } = require('playwright');
const sim = require('./phone-sim.cjs');

const DIR = fs.mkdtempSync(path.join(os.tmpdir(), 'wu-phonelab-'));
const WWW = path.join(__dirname, '..', 'www-files');
let server, browser, base;
test.before(async () => {
  server = await sim.startPortal(0, { WU_PHONELAB_DIR: DIR });
  base = `http://127.0.0.1:${server.port}`;
  browser = await chromium.launch();
});
test.after(async () => { if (browser) await browser.close(); if (server) server.kill(); });

let n = 0;
function session() {
  const code = ['TESTAB', 'TESTCD', 'TESTEF', 'TESTGH'][n++];
  const t = Math.floor(Date.now() / 1000);
  fs.writeFileSync(path.join(DIR, code + '.json'), JSON.stringify({ code, started: t, ends: t + 1800, stopped: null, by: 'test', ua: '',
    batches: 0, bytes: 0, shots: 0, recs: 0, last: null }));
  return { code, cookie: { name: 'wu_phonelab', value: `${code}.${t + 1800}`, path: '/' } };
}
const batches = (code) => { try { return fs.readFileSync(path.join(DIR, code + '.jsonl'), 'utf8').trim().split('\n').filter(Boolean).map((l) => JSON.parse(l)); } catch { return []; } };
const recs = (code) => batches(code).flatMap((b) => b.recs);
async function flushed(p, code, want = 1) {
  await p.evaluate(() => window.__phonelab && window.__phonelab.flush());
  for (let i = 0; i < 40 && batches(code).length < want; i++) await p.waitForTimeout(100);
}

test('a site page: the banner, a real tap with its events and a sketch; typing never leaves the phone', async () => {
  const s = session();
  const { ctx, p, errors } = await sim.phone(browser, 'pixel', { headers: { 'X-Forwarded-For': '10.98.0.1' } });
  await ctx.addCookies([{ ...s.cookie, domain: '127.0.0.1' }]);
  await p.goto(base + '/wrath/leyline/catalog');
  await p.waitForFunction(() => window.__phonelab, null, { timeout: 10000 });
  assert.match(await p.textContent('#wu-phonelab'), new RegExp('Phone Lab ' + s.code));
  await sim.tap(p, '.hb-seg a:nth-child(2)', { settleMs: 200 });   // a tab: the page goes on to Addons
  await p.waitForLoadState('domcontentloaded');
  await p.waitForFunction(() => window.__phonelab, null, { timeout: 10000 });
  // a search field: tapped, then typed into
  await sim.tap(p, '#hb-q', { settleMs: 900 });
  await p.keyboard.type('secret words 12345');
  await p.waitForTimeout(300);
  await flushed(p, s.code, 2);
  const all = recs(s.code);
  const raw = fs.readFileSync(path.join(DIR, s.code + '.jsonl'), 'utf8');
  const tab = all.find((r) => r.kind === 'tap' && /hb-seg/.test(r.on || ''));
  assert.ok(tab, 'the tab tap is there: ' + JSON.stringify(all.map((r) => r.kind)));
  const types = tab.seq.map((x) => x[0]);
  for (const t of ['touchstart', 'pointerdown', 'touchend', 'pointerup', 'click']) assert.ok(types.includes(t), `${t} in ${types}`);
  assert.equal(tab.text, 'Addons');
  assert.ok(tab.before && tab.before.w === 412, 'the viewport: ' + JSON.stringify(tab.before));
  const fld = all.find((r) => r.kind === 'tap' && /hb-q/.test(r.on || ''));
  assert.ok(fld, 'the field tap is there');
  assert.equal(fld.text, '[input search]', 'a field is named by its kind only');
  assert.doesNotMatch(raw, /secret words|12345/, 'what was typed never left the phone');
  assert.ok(all.some((r) => r.kind === 'page' && r.dpr === 2.625 && r.touch > 0), 'the phone is described');
  const shot = batches(s.code).flatMap((b) => b.shots)[0];
  assert.ok(shot && shot.data.startsWith('data:image/jpeg;base64,') && shot.data.length < 64000, 'a sketch after a tap');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the sign-in pages: taps and timings only, no words, no sketch', async () => {
  const s = session();
  const { ctx, p } = await sim.phone(browser, 'iphone', { headers: { 'X-Forwarded-For': '10.98.0.2' } });
  await ctx.addCookies([{ ...s.cookie, domain: '127.0.0.1' }]);
  await p.goto(base + '/wrath/signin');
  await p.waitForFunction(() => window.__phonelab, null, { timeout: 10000 });
  await sim.tap(p, 'h1', { settleMs: 900 });
  await flushed(p, s.code);
  const t = recs(s.code).find((r) => r.kind === 'tap');
  assert.ok(t, 'the tap is there');
  assert.equal(t.text, '');
  assert.equal(t.shot, undefined);
  assert.equal(batches(s.code).flatMap((b) => b.shots).length, 0);
  await ctx.close();
});

test('the game page (another origin): loaded by wrath-early.js, batches arrive, the full screen ask is recorded', async () => {
  const s = session();
  // the site as another public origin (Chromium keeps a public page off 127.0.0.1): site.test is the portal
  const { ctx, p, errors } = await sim.phone(browser, 'pixel', { landscape: true, init: 'window.__wuPhonelabSite = "http://site.test";' });
  await p.route('http://site.test/**', async (r) => {
    const u = new URL(r.request().url());
    r.fulfill({ response: await r.fetch({ url: base + u.pathname + u.search, headers: { ...r.request().headers(), 'x-forwarded-for': '10.98.0.3' } }) });
  });
  await ctx.addCookies([{ ...s.cookie, domain: 'game.test' }]);
  const early = fs.readFileSync(path.join(WWW, 'wrath-early.js'), 'utf8');
  const block = (name) => early.match(new RegExp(`// >>> ${name}[\\s\\S]*?// <<< ${name}`))[0];
  await p.route('http://game.test/**', (r) => r.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head>
    <meta name=viewport content="width=device-width,initial-scale=1"><script>${block('first-tap-fullscreen')}\n${block('phonelab')}</script></head>
    <body style="margin:0;height:100vh;background:#123"><canvas id=canvas style="width:100vw;height:100vh;display:block"></canvas>
    <div id=ow-signin style="position:fixed;inset:0;display:grid;place-items:center;color:#fff">Signing in…</div></body></html>` }));
  await p.goto('http://game.test/h.html');
  await p.waitForFunction(() => window.__phonelab, null, { timeout: 10000 });
  await sim.tap(p, [400, 200], { settleMs: 900 });
  await flushed(p, s.code);
  for (let i = 0; i < 30 && !recs(s.code).some((r) => r.kind === 'fs-ask'); i++) { await p.evaluate(() => window.__phonelab.flush()); await p.waitForTimeout(100); }
  const all = recs(s.code);
  const t = all.find((r) => r.kind === 'tap');
  assert.ok(t && t.on === 'div#ow-signin', 'the tap on the sign-in cover: ' + JSON.stringify(t));
  assert.match(t.fsAsk || '', /^(pointerup|touchend)\+gesture$/, 'its full screen ask, from the lift, as a gesture');
  const ask = all.find((r) => r.kind === 'fs-ask');
  assert.ok(ask && ask.gesture === true && ask.trusted === true, JSON.stringify(ask));
  assert.ok(all.some((r) => r.kind === 'page' && r.game === true));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Stop on the banner (a real tap): the session ends, the cookie goes, nothing more is recorded', async () => {
  const s = session();
  const { ctx, p } = await sim.phone(browser, 'pixel', { headers: { 'X-Forwarded-For': '10.98.0.4' } });
  await ctx.addCookies([{ ...s.cookie, domain: '127.0.0.1' }]);
  await p.goto(base + '/wrath/leyline/realms');
  await p.waitForFunction(() => window.__phonelab, null, { timeout: 10000 });
  await sim.tap(p, '#wu-phonelab button', { settleMs: 600 });
  for (let i = 0; i < 30 && !JSON.parse(fs.readFileSync(path.join(DIR, s.code + '.json'), 'utf8')).stopped; i++) await p.waitForTimeout(100);
  assert.ok(JSON.parse(fs.readFileSync(path.join(DIR, s.code + '.json'), 'utf8')).stopped, 'the session ended');
  assert.equal(await p.$('#wu-phonelab'), null, 'the banner went');
  assert.equal((await ctx.cookies()).filter((c) => c.name === 'wu_phonelab').length, 0, 'the cookie went');
  assert.ok(!recs(s.code).some((r) => r.kind === 'tap' && /wu-phonelab/.test(r.on || '')), 'the banner\'s own tap is not recorded');
  await p.goto(base + '/wrath/leyline/realms');
  assert.equal(await p.evaluate(() => !!document.getElementById('wu-phonelab-js')), false, 'no recorder on the next page');
  await ctx.close();
});
