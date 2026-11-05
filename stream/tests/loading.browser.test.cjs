// Headless Chromium (Playwright): the world-loading cover (www-files/wu-loading.js + wu-loading.css) names what the
// load waits on and fills its bar from the client's own fraction. boot.js turns the client's `load:<what>` into a
// 'wenilla-loading' event ({what, text}) and its `cover:<pct>` into 'wenilla-cover' ({active, percent}); the cover
// shows the words, never a number, and the measured bar wins over the prefetch count.
//
//   NODE_PATH=$(npm root -g) node --test stream/tests/loading.browser.test.cjs
//   SHOTS=<dir> ... also saves a screenshot of the cover
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { chromium } = require('playwright');

const WWW = path.join(__dirname, '..', 'www-files');
const TYPES = { '.js': 'text/javascript', '.html': 'text/html', '.css': 'text/css', '.webp': 'image/webp', '.woff2': 'font/woff2' };

function serve() {
  const srv = http.createServer((req, res) => {
    const u = new URL(req.url, 'http://x');
    if (u.pathname === '/page.html') {
      // ?go=<kind:id>: the session's go link, as wrath-page.js leaves it on window.wuGo (set after this module runs)
      const go = u.searchParams.get('go');
      const set = go ? `setTimeout(() => { window.wuGo = { kind: ${JSON.stringify(go.split(':')[0])}, id: ${JSON.stringify(go.slice(go.indexOf(':') + 1))} }; }, 0);` : '';
      res.writeHead(200, { 'content-type': 'text/html' });
      return res.end('<!doctype html><html><head><meta charset="utf-8"><title>t</title>'
        + '<link rel="stylesheet" href="./wu-loading.css"></head><body style="margin:0;background:#000">'
        + `<script type="module">import { installLoading } from "./wu-loading.js"; window.st = installLoading(); ${set}</script>`
        + '</body></html>');
    }
    const f = path.join(WWW, path.normalize(u.pathname).replace(/^([/\\])+/, ''));
    if (!f.startsWith(WWW) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); return res.end(); }
    res.writeHead(200, { 'content-type': TYPES[path.extname(f)] || 'application/octet-stream' });
    fs.createReadStream(f).pipe(res);
  });
  return new Promise((ok) => srv.listen(0, '127.0.0.1', () => ok(srv)));
}

test('the cover names what it loads and shows the client\'s real progress', async () => {
  const srv = await serve();
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1100, height: 700 } });
    await page.goto(`http://127.0.0.1:${srv.address().port}/page.html`);
    await page.waitForFunction(() => window.st);
    const read = () => page.evaluate(() => {
      const c = document.getElementById('wu-loading');
      return { on: c.classList.contains('on'), text: c.querySelector('.stage').textContent,
        line: c.querySelector('.stage').dataset.line || null,
        width: c.querySelector('.bar > i').style.width, pulse: c.querySelector('.bar').classList.contains('pulse') };
    });
    await page.evaluate(() => dispatchEvent(new CustomEvent('ow:progress', { detail: 'enter' })));
    let s = await read();
    assert.equal(s.on, true);
    assert.equal(s.text, 'Entering the world…');
    assert.equal(s.pulse, true, 'no measure yet: the gliding bar');

    await page.evaluate(() => {
      dispatchEvent(new CustomEvent('wenilla-loading', { detail: { what: 'buildings', text: 'Loading buildings…' } }));
      dispatchEvent(new CustomEvent('wenilla-cover', { detail: { active: true, percent: 42 } }));
    });
    s = await read();
    assert.equal(s.text, 'Loading buildings…');
    assert.equal(s.line, 'Loading buildings…', 'the CSS line reads data-line');
    assert.equal(s.width, '42%');
    assert.equal(s.pulse, false);
    assert.doesNotMatch(s.text, /\d/, 'no numbers on screen');

    // the prefetch count no longer moves the bar once the client measures
    await page.evaluate(() => {
      const el = document.createElement('div');
      el.id = 'wenilla-corner';
      el.textContent = 'preparing world data — 1/100';
      document.body.append(el);
    });
    await page.waitForTimeout(50);
    assert.equal((await read()).width, '42%');
    if (process.env.SHOTS) await page.screenshot({ path: path.join(process.env.SHOTS, 'loading-cover.png') });

    await page.evaluate(() => {
      document.getElementById('wenilla-corner').remove();
      dispatchEvent(new CustomEvent('ow:progress', { detail: 'world' }));
    });
    assert.equal((await read()).on, false);
    // a new cover starts plain again
    await page.evaluate(() => dispatchEvent(new CustomEvent('ow:progress', { detail: 'loading:on' })));
    s = await read();
    assert.equal(s.text, 'Entering the world…');
    assert.equal(s.line, null);
    assert.equal(s.pulse, true);
  } finally {
    await browser.close();
    srv.close();
  }
});

test('"Slow loading?": after a while on the cover, the desktop app and Download sections; never on a quick load', async () => {
  const srv = await serve();
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1100, height: 700 } });
    await page.goto(`http://127.0.0.1:${srv.address().port}/page.html`);
    await page.waitForFunction(() => window.st);
    const { addSlowHelp } = await page.evaluate(async () => Object.keys(await import('./wu-loading.js')).includes('addSlowHelp') ? { addSlowHelp: true } : {});
    assert.ok(addSlowHelp, 'wu-loading.js exports addSlowHelp');
    // a quick load: the cover goes down before the line is due, and no line ever comes
    await page.evaluate(async () => {
      const m = await import('./wu-loading.js');
      const c = document.getElementById('wu-loading') || (window.st.show('t'), document.getElementById('wu-loading'));
      window.st.hide('t');
      m.addSlowHelp(c.querySelector('.box'), 50);
    });
    await page.waitForTimeout(200);
    assert.equal(await page.locator('#wu-loading .ly-slow').count(), 0, 'nothing after a quick load');
    // a slow one: the page's Download sections button exists; the line offers both
    await page.evaluate(async () => {
      const dl = document.createElement('button'); dl.id = 'dl'; dl.onclick = () => { window.dlClicked = true; }; document.body.append(dl);
      const m = await import('./wu-loading.js');
      window.st.show('slow');
      const box = document.querySelector('#wu-loading .box');
      delete box.dataset.slowArmed;
      m.addSlowHelp(box, 50);
    });
    await page.waitForSelector('#wu-loading .ly-slow');
    const line = await page.textContent('#wu-loading .ly-slow');
    assert.match(line, /^Slow loading\? Get the Leyline desktop app: it plays with your own game folder/);
    assert.equal(await page.getAttribute('#wu-loading .ly-slow a', 'href'), 'https://play.openvanilla.app/wrath/download');
    await page.click('#wu-loading .ly-slow button');
    assert.equal(await page.evaluate(() => window.dlClicked), true, 'Download sections opens the page\'s panel');
    assert.equal(await page.locator('#wu-loading .ly-slow').count(), 1, 'one line only');
    assert.doesNotMatch(line, /World of Warcraft|Warcraft|WoW|Blizzard|Lich King|\d+\.\d+\.\d+/);
  } finally {
    await browser.close();
    srv.close();
  }
});

test('a boss fight session: the cover says what it gets ready, no numbers; other sessions say nothing', async () => {
  const srv = await serve();
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 390, height: 760 } });
    const note = () => page.evaluate(() => { const n = document.querySelector('#wu-loading .ly-go'); return n ? n.textContent : null; });
    const base = `http://127.0.0.1:${srv.address().port}/page.html`;
    await page.goto(base + '?go=bf:frozen-throne:normal:bots');
    await page.waitForFunction(() => window.st && window.wuGo);
    await page.evaluate(() => dispatchEvent(new CustomEvent('ow:progress', { detail: 'enter' })));
    assert.equal(await note(), 'Boss fight: The Frozen Throne \u2014 getting your raid ready');
    const order = await page.evaluate(() => [...document.querySelector('#wu-loading .box').children].map((c) => c.className));
    assert.ok(order.indexOf('ly-go') < order.indexOf('ly-tip'), 'the note sits above the tips: ' + order);
    assert.doesNotMatch(await note(), /\d/, 'no seat counts or places in line');
    if (process.env.SHOTS) await page.screenshot({ path: path.join(process.env.SHOTS, 'loading-bossfight-phone.png') });

    await page.goto(base + '?go=bf:frozen-throne:heroic:group');
    await page.waitForFunction(() => window.st && window.wuGo);
    await page.evaluate(() => dispatchEvent(new CustomEvent('ow:progress', { detail: 'enter' })));
    assert.equal(await note(), 'Boss fight: The Frozen Throne (heroic) \u2014 getting your raid ready');

    for (const go of ['h:dalaran', 'bf:somewhere:normal:bots', 'bf:frozen-throne:mythic:bots', '']) {
      await page.goto(base + (go ? '?go=' + go : ''));
      await page.waitForFunction(() => window.st);
      await page.evaluate(() => new Promise((ok) => setTimeout(ok, 20)));
      await page.evaluate(() => dispatchEvent(new CustomEvent('ow:progress', { detail: 'enter' })));
      assert.equal(await note(), null, 'no note for ' + (go || 'no go'));
    }
    const { goNote } = await page.evaluate(async () => ({ goNote: (await import('./wu-loading.js')).goNote('arena:1v1') }));
    assert.equal(goNote, '1v1 arena \u2014 getting you into the queue');
  } finally {
    await browser.close();
    srv.close();
  }
});
