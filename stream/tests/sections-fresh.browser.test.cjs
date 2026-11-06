// Headless Chromium tests (Playwright) for "Download this section" (www-files/sections.js) across a NEW SLICE:
// a downloaded tier holds interface files and client tables (Interface\, DBFilesClient\) as well as world data.
// A slice upload or a Forever UI update changes those under the same name (stream-up.sh gives them 10 minutes in
// the HTTP cache; wrath-sw.js never keeps them), so a section downloaded before must not keep answering the old
// bytes forever. World data (World\, Creature\, Textures\...) never changes under its name and stays downloaded.
//
//   node --test tests/sections-fresh.browser.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..', 'www-files');
const SITE = fs.mkdtempSync(path.join(os.tmpdir(), 'sections-fresh-'));
for (const f of fs.readdirSync(ROOT)) if (fs.statSync(path.join(ROOT, f)).isFile()) fs.copyFileSync(path.join(ROOT, f), path.join(SITE, f));

const UI = 'Interface\\GlueXML\\ForeverUIGlue.lua';
const DBC = 'DBFilesClient\\Spell.dbc';
const WORLD = 'World\\Maps\\Azeroth\\Azeroth_32_48.adt';
const TIER = [UI, DBC, WORLD];

fs.writeFileSync(path.join(SITE, 'harness.html'), `<!doctype html><meta charset="utf-8"><title>harness</title>
<body><script type="module">
  import * as S from './sections.js';
  window.S = S;
  window.syncGet = (name) => {
    const x = new XMLHttpRequest();
    x.open('GET', '/data/' + encodeURIComponent(name), false);
    x.overrideMimeType('text/plain; charset=x-user-defined');
    x.send();
    return { status: x.status, text: x.status === 200 ? x.responseText : '' };
  };
  S.installSections().then((r) => { window.__installed = r; document.title = 'ready'; });
</script>`);

function startServer() {
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json' };
  const st = { version: 'v1', hits: {}, down: false };
  const body = (name) => Buffer.from(`${name} ${name === WORLD ? 'same' : st.version}`);
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    const base = { 'cross-origin-opener-policy': 'same-origin', 'cross-origin-embedder-policy': 'require-corp',
      'cross-origin-resource-policy': 'same-origin', 'cache-control': 'no-store' };
    if (url.pathname.startsWith('/data/')) {
      const raw = url.pathname.slice(6);
      st.hits[raw] = (st.hits[raw] || 0) + 1;
      const files = TIER.map((n) => [body(n).length, n]);
      const bytes = files.reduce((a, f) => a + f[0], 0);
      if (raw === '__tier/index.json') {
        res.writeHead(200, { ...base, 'content-type': 'application/json' });
        // a new slice: other sizes (a Forever UI update changes the base tier's bytes)
        return res.end(JSON.stringify([{ tier: 0, name: 'base', files: TIER.length, bytes: bytes + (st.version === 'v1' ? 0 : 7) }]));
      }
      if (raw === '__tier/0.txt') {
        res.writeHead(200, { ...base, 'content-type': 'text/plain' });
        return res.end(`# tier 0 base\n` + files.map(([b, n]) => `${b}\t${n}\n`).join(''));
      }
      if (raw.startsWith('__')) { res.writeHead(404, base); return res.end(); }
      const name = decodeURIComponent(raw).replace(/\//g, '\\');
      if (!TIER.includes(name) || st.down) { res.writeHead(st.down ? 503 : 404, base); return res.end(); }
      res.writeHead(200, { ...base, 'content-type': 'application/octet-stream' });
      return res.end(body(name));
    }
    const file = path.join(SITE, path.normalize(url.pathname).replace(/^[/\\]+/, '') || 'index.html');
    if (!file.startsWith(SITE) || !fs.existsSync(file)) { res.writeHead(404, base); return res.end(); }
    res.writeHead(200, { ...base, 'content-type': types[path.extname(file)] || 'application/octet-stream' });
    res.end(fs.readFileSync(file));
  });
  server.st = st;
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

test('a new slice: downloaded interface files and tables are fetched again, world data stays downloaded', async () => {
  const server = await startServer();
  const ctx = await browser.newContext();
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  try {
    await page.goto(`http://127.0.0.1:${server.address().port}/harness.html`);
    await page.waitForFunction(() => document.title === 'ready');
    assert.equal((await page.evaluate(() => window.__installed)).installed, true);
    const r = await page.evaluate(() => window.S.downloadTier(0));
    assert.equal(r.failed, 0);
    // the same slice, a reload: everything still from the cache (the server is down)
    await page.reload();
    await page.waitForFunction(() => document.title === 'ready');
    server.st.down = true;
    for (const n of TIER) assert.equal((await page.evaluate((n) => window.syncGet(n), n)).status, 200, `${n} from the cache`);
    assert.match((await page.evaluate((n) => window.syncGet(n), UI)).text, /v1$/);
    server.st.down = false;

    // A new slice goes up (other tier sizes in index.json; the UI and the tables changed under their names).
    server.st.version = 'v2';
    await page.reload();
    await page.waitForFunction(() => document.title === 'ready');
    const ui = await page.evaluate((n) => window.syncGet(n), UI);
    assert.match(ui.text, /v2$/, 'the new interface file, not the one downloaded for the old slice');
    assert.match((await page.evaluate((n) => window.syncGet(n), DBC)).text, /v2$/, 'the new client table');
    const worldHits = server.st.hits[encodeURIComponent(WORLD)] || 0;
    assert.equal((await page.evaluate((n) => window.syncGet(n), WORLD)).status, 200);
    assert.equal(server.st.hits[encodeURIComponent(WORLD)] || 0, worldHits, 'world data still from the download');
    const status = await page.evaluate(() => window.S.sectionsStatus());
    assert.equal(status[0] && status[0].done, false, 'the section shows as partly downloaded, so "Download" fetches the new files');
    // and once more with the same slice: nothing else goes
    await page.reload();
    await page.waitForFunction(() => document.title === 'ready');
    server.st.down = true;
    assert.equal((await page.evaluate((n) => window.syncGet(n), WORLD)).status, 200);
    assert.deepEqual(errors, []);
  } finally {
    await ctx.close();
    server.close();
  }
});
