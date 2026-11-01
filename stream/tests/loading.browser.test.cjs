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
      res.writeHead(200, { 'content-type': 'text/html' });
      return res.end('<!doctype html><html><head><meta charset="utf-8"><title>t</title>'
        + '<link rel="stylesheet" href="./wu-loading.css"></head><body style="margin:0;background:#000">'
        + '<script type="module">import { installLoading } from "./wu-loading.js"; window.st = installLoading();</script>'
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
