// Headless Chromium (Playwright) for "Play solo on this device" (docs/PHONE-SOLO.md): a cross-origin-isolated page
// like the game's (/__env.js, then wrath-early.js, then the page's module script) opened with ?go=solo. wrath-early.js
// writes solo.js in, which replaces WebSocket before the module script runs; the module script then does what the
// browser client does on its sockets (tests/solo/wire335.js: SRP6 login on /ws/3724, the realm list, the world's auth
// session with its header encryption, the character list, entering the world, ping). Checked: it all goes to the
// page's own worker (no socket to any server), the env signs in the device's player with the ready-made character,
// other sockets stay real ones, the Dens folder is kept in IndexedDB across reloads, a save copy restores, and a
// hidden page pauses the worker.
//
// The engine is the stand-in server (tests/solo/stub-engine.js), or Den Lite's own wasm when it is built
// (DEN_LITE_WASM=<file>, or labs/den-lite/target/wasm32-wasip1/release/den-lite-web.wasm): then the page loads
// solo/den-lite-web.wasm exactly as players get it.
//
//   node --test tests/solo.browser.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { chromium } = require('playwright');

const WWW = path.join(__dirname, '..', 'www-files');
const TESTS = path.join(__dirname, 'solo');
const WASM = process.env.DEN_LITE_WASM || path.join(__dirname, '..', '..', 'labs', 'den-lite', 'target', 'wasm32-wasip1', 'release', 'den-lite-web.wasm');
const REAL = fs.existsSync(WASM);

const PAGE = `<!doctype html><meta charset="utf-8"><title>solo</title>
<meta name="viewport" content="width=device-width,initial-scale=1">
<script>window.__wuSoloEngine = ${REAL ? 'undefined' : JSON.stringify({ kind: 'script', url: '/t/stub-engine.js' })};</script>
<script src="/__env.js"></script>
<script src="./wrath-early.js"></script>
<script src="/t/wire335.js"></script>
<script type="module">
  // the client: what benilla-app net/ws.rs does with its sockets, from the env the page gave it
  const env = window.__wenilla_env;
  window.__client = (async () => {
    const t0 = performance.now();
    const ls = Wire335.socketStream(new WebSocket('wss://' + location.host + '/ws/3724'));
    await ls.opened;
    const L = await Wire335.login(ls, env.user, env.pass);
    const port = L.realms[0].address.split(':').pop();
    const ws = Wire335.socketStream(new WebSocket('wss://' + location.host + '/ws/' + port));
    await ws.opened;
    const w = await Wire335.enterWorld(ws, env.user, L.K);
    ls.close();
    window.__world = w.world;
    return { ms: Math.round(performance.now() - t0), realm: L.realms[0].name, address: L.realms[0].address, name: w.name,
             map: w.verify.map, seen: w.seen.length, env: { user: env.user, char: env.char, solo: env.solo, hop: env.hop || null } };
  })().catch((e) => ({ error: String(e && e.message || e) + ' ' + JSON.stringify(window.__wuSolo ? window.__wuSolo.status() : window.__wuSoloEarly) }));
</script>`;

function startServer() {
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json', '.wasm': 'application/wasm', '.css': 'text/css' };
  const hits = [];
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    hits.push(url.pathname);
    const head = { 'cross-origin-opener-policy': 'same-origin', 'cross-origin-embedder-policy': 'require-corp', 'cross-origin-resource-policy': 'same-origin', 'cache-control': 'no-store' };
    let file = null;
    if (url.pathname === '/' || url.pathname === '/index.html') { res.writeHead(200, { ...head, 'content-type': 'text/html' }); return res.end(PAGE); }
    if (url.pathname === '/__env.js') {
      // a signed-in streamed session's env, with a hop left over: solo must replace all of it
      res.writeHead(200, { ...head, 'content-type': 'text/javascript' });
      return res.end('window.__wenilla_env = {"user":"WR000123","pass":"Pa55word","host":"play.example","slice":"mini","brand":"leyline","hop":"abcdefghijklmnop","hop_ws":"wss://relay.example/ws/abcdefghijklmnop"};');
    }
    if (url.pathname.startsWith('/ws/')) { hits.push('SOCKET ' + url.pathname); res.writeHead(404, head); return res.end(); }
    if (url.pathname.startsWith('/t/')) file = path.join(TESTS, path.basename(url.pathname));
    else if (url.pathname === '/solo/den-lite-web.wasm') file = WASM;
    else if (url.pathname.startsWith('/solo/')) file = path.join(WWW, 'solo', path.basename(url.pathname));
    else if (/^\/[a-z-]+\.js$/.test(url.pathname)) file = path.join(WWW, path.basename(url.pathname));
    if (!file || !fs.existsSync(file) || (url.pathname === '/wrath-page.js')) { res.writeHead(404, head); return res.end(); }
    res.writeHead(200, { ...head, 'content-type': types[path.extname(file)] || 'application/octet-stream' });
    res.end(fs.readFileSync(file));
  });
  server.on('upgrade', (req, sock) => { hits.push('UPGRADE ' + req.url); sock.destroy(); });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok({ server, hits, base: `http://127.0.0.1:${server.address().port}` })));
}

let browser, site;
test.before(async () => {
  site = await startServer();
  browser = await chromium.launch({ args: ['--enable-features=SharedArrayBuffer'] });
});
test.after(async () => {
  if (browser) await browser.close();
  if (site) site.server.close();
});

test(`solo on this device: into the world with no server (${REAL ? 'Den Lite wasm' : 'stand-in server'})`, { timeout: 90000 }, async () => {
  const ctx = await browser.newContext({ viewport: { width: 412, height: 915 }, isMobile: true, hasTouch: true });
  const page = await ctx.newPage();
  const logs = [];
  page.on('console', (m) => logs.push(m.text()));
  page.on('pageerror', (e) => logs.push('pageerror ' + e.message));
  try {
    await page.goto(site.base + '/?go=solo&nogpucheck=1');
    const r = await page.evaluate(() => window.__client);
    assert.equal(r.error, undefined, r.error + '\n' + logs.join('\n'));
    assert.equal(r.name, 'Wren');
    assert.deepEqual(r.env, { user: 'SOLO', char: 'Wren', solo: 'phone', hop: null });
    assert.match(r.address, /:8085$/);
    const early = await page.evaluate(() => window.__wuSoloEarly);
    assert.equal(early.ok, true);
    assert.deepEqual(site.hits.filter((h) => /SOCKET|UPGRADE/.test(h)), [], 'not one socket to a server');
    const st = await page.evaluate(() => window.__wuSolo.status());
    assert.equal(st.ready, true);
    assert.ok(st.events.toServer >= 5 && st.events.fromServer >= 5, JSON.stringify(st.events));
    console.log(`  phone page: in the world in ${r.ms} ms after the client's first socket (worker ready ${st.events.readyMs} ms after install), ${r.seen} world messages`);
    // other sockets are real ones
    const other = await page.evaluate(() => { const s = new WebSocket('ws://127.0.0.1:9/elsewhere'); const solo = s instanceof window.__wuSolo.Socket; s.close(); return solo; });
    assert.equal(other, false);
    // a hidden page pauses the worker (no beats), and it goes on when shown
    const paused = await page.evaluate(async () => {
      Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
      document.dispatchEvent(new Event('visibilitychange'));
      const p = window.__wuSolo.bus.ring.paused();
      Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'visible' });
      document.dispatchEvent(new Event('visibilitychange'));
      // still alive: a ping through the world socket
      window.__world.send(Wire335.OP.CMSG_PING, Wire335.cat(Wire335.u32le(9), Wire335.u32le(30)));
      const pong = await window.__world.until([Wire335.OP.SMSG_PONG]);
      return { p, pong: Wire335.rd32(pong.body, 0), now: window.__wuSolo.bus.ring.paused() };
    });
    assert.deepEqual(paused, { p: true, pong: 9, now: false });
  } finally {
    await ctx.close();
  }
});

test('the solo world is kept in this browser, and a save copy brings it back', { timeout: 90000, skip: REAL ? 'the stand-in keeps a counter file; Den Lite keeps its own saves (checked in CI by the first test)' : false }, async () => {
  const ctx = await browser.newContext();
  const page = await ctx.newPage();
  try {
    const visits = async () => {
      await page.goto(site.base + '/?solo=1&nogpucheck=1');
      const r = await page.evaluate(() => window.__client);
      assert.equal(r.error, undefined, r.error);
      // the worker hands the file to the page within half a second; the page keeps it in IndexedDB
      return page.evaluate(async () => {
        for (let i = 0; i < 60; i++) {
          const files = await window.__wuSolo.bus.store.load();
          const f = files['/dens/stub-visits.json'];
          if (f && window.__lastSeen !== new TextDecoder().decode(f)) { window.__lastSeen = new TextDecoder().decode(f); }
          if (f) { const v = JSON.parse(new TextDecoder().decode(f)).visits; if (v >= (window.__want || 1)) return v; }
          await new Promise((r) => setTimeout(r, 50));
        }
        return 0;
      });
    };
    assert.equal(await visits(), 1);
    await page.evaluate(() => { window.__want = 2; });
    const second = await (async () => {
      await page.goto(site.base + '/?solo=1&nogpucheck=1');
      await page.evaluate(() => window.__client);
      return page.evaluate(async () => {
        for (let i = 0; i < 60; i++) {
          const f = (await window.__wuSolo.bus.store.load())['/dens/stub-visits.json'];
          const v = f ? JSON.parse(new TextDecoder().decode(f)).visits : 0;
          if (v >= 2) return v;
          await new Promise((r) => setTimeout(r, 50));
        }
        return -1;
      });
    })();
    assert.equal(second, 2, 'the reload opened the kept world');
    // a save copy, changed and restored: the next visit counts on from it
    const saved = await page.evaluate(() => window.__wuSolo.exportSave());
    const doc = JSON.parse(saved);
    assert.equal(doc.format, 'leyline-solo');
    assert.ok(doc.files['/dens/stub-visits.json']);
    doc.files['/dens/stub-visits.json'] = Buffer.from(JSON.stringify({ visits: 41 })).toString('base64');
    const n = await page.evaluate((t) => window.__wuSolo.importSave(t), JSON.stringify(doc));
    assert.equal(n, 1);
    await page.goto(site.base + '/?solo=1&nogpucheck=1');
    await page.evaluate(() => window.__client);
    const after = await page.evaluate(async () => {
      for (let i = 0; i < 60; i++) {
        const f = (await window.__wuSolo.bus.store.load())['/dens/stub-visits.json'];
        const v = f ? JSON.parse(new TextDecoder().decode(f)).visits : 0;
        if (v >= 42) return v;
        await new Promise((r) => setTimeout(r, 50));
      }
      return -1;
    });
    assert.equal(after, 42);
    await assert.rejects(page.evaluate(() => window.__wuSolo.importSave('{"format":"nope"}')));
  } finally {
    await ctx.close();
  }
});
