// Headless Chromium (Playwright): the in-game realm browser's page bridge and the fast hop in a real, cross-origin
// isolated game page (COOP same-origin + COEP require-corp, as wu-play serves it). The REAL wrath-page.js takes the
// game's ask through a fake bridge, fetches the network and relay listings from other origins (CORS), answers with
// the Lua the game reads, prefetches the Hero Key assertion from the portal (same origin) when a preview opens; Join
// redeems it at the "relay" (CORS POST, no cookies), switches the session (POST /__gate?hop=) and lands on the game
// page, where the REAL wrath-early.js signs in with the kept one-time login (no redeem, no join page). A realm that
// needs another game version gets the launcher bar (leyline://realm/<id>), never a hop. Shots of the hop cover and the
// launcher bar go to $HOP_SHOTS when set.
//
//   node --test tests/browse.browser.test.cjs          (from the stream folder; needs Playwright)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { chromium } = require('playwright');

const WWW = path.join(__dirname, '..', 'www-files');
const A = 'aaaaaaaaaaaaaaaa', B = 'bbbbbbbbbbbbbbbb';
const rec = (id, name, o = {}) => Object.assign({ schema: 1, name, official: false, state: 'live', profile: { build: 12340 },
  community: { realmId: id, lastHeartbeat: { players: 2, bots: 0 } } }, o);

function listen(handler) {
  const srv = http.createServer(handler);
  return new Promise((r) => srv.listen(0, '127.0.0.1', () => r({ srv, port: srv.address().port })));
}

test('page bridge and fast hop in a cross-origin isolated page', async (t) => {
  const seen = { redeem: [], gate: [], cookies: [] };
  // "the relay" and "the website": another origin, listings with CORS *, the herokey route with CORS for the game page
  const site = await listen((req, res) => {
    const u = new URL(req.url, 'http://x');
    const cors = { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Headers': 'Content-Type', 'Access-Control-Allow-Methods': 'POST, GET' };
    if (req.method === 'OPTIONS') { res.writeHead(204, cors); return res.end(); }
    seen.cookies.push(req.headers.cookie || '');
    const json = (doc, st = 200) => { res.writeHead(st, { ...cors, 'Content-Type': 'application/json' }); res.end(JSON.stringify(doc)); };
    if (u.pathname === '/wrath/leyline/flags.json') return json({ flags: { herokey_sso: true } });
    if (u.pathname === '/wrath/realms.json') return json({ realms: [rec(A, 'Frost Haven'), rec(B, 'Old Days', { profile: { build: 5875 } })] });
    if (u.pathname === '/api/listing/realms.json') return json({ realms: [rec(A, 'Frost Haven', { herokey: true, online: 3, who: ['Mira'], rules: 'Be kind.',
      mods: [{ id: 'mode:minigolf', name: 'Minigolf', required: true, client: false }] })] });
    if (u.pathname === `/api/realms/${A}/herokey` && req.method === 'POST') {
      let body = ''; req.on('data', (d) => { body += d; }); req.on('end', () => { seen.redeem.push(body); json({ ok: true }); });
      return undefined;
    }
    res.writeHead(404, cors); res.end();
  });
  const SITE = `http://127.0.0.1:${site.port}`;
  let hopDelay = 0;
  const game = await listen((req, res) => {
    const u = new URL(req.url, 'http://x');
    const h = { 'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp', 'Cache-Control': 'no-store' };
    if (req.method === 'POST' && u.pathname === '/__gate') {
      seen.gate.push(u.search);
      let body = ''; req.on('data', (d) => { body += d; });
      req.on('end', () => setTimeout(() => {
        res.writeHead(200, { ...h, 'Content-Type': 'application/json' });
        if (u.searchParams.get('herokey')) {
          return res.end(JSON.stringify({ ok: true, assertion: { aud: A }, sig: 'ab'.repeat(64), redeem: `https://relay.test/api/realms/${A}/herokey`,
            login: { user: 'HKABCDEFGHIJKLMN', pass: 'Q7W8E9R2T3Y4U5I6' }, ttl: 120 }));
        }
        seen.hopBody = body;
        res.end(JSON.stringify({ ok: true, land: `/?go=r:${A}` }));
      }, u.searchParams.get('hop') ? hopDelay : 0));
      return;
    }
    if (u.pathname.endsWith('.js')) {
      const f = path.join(WWW, path.basename(u.pathname));
      if (fs.existsSync(f)) { res.writeHead(200, { ...h, 'Content-Type': 'text/javascript' }); return res.end(fs.readFileSync(f)); }
      res.writeHead(404, h); return res.end();
    }
    if (u.pathname === '/' && u.searchParams.get('go')) {   // the landing page after the hop: the hop session's env
      const env = { hop: A, hop_name: 'Frost Haven', hop_ws: `wss://relay.test/ws/${A}`, hop_join: `https://relay.test/join/${A}?hop=1`, home: SITE,
        hop_sso: undefined };
      res.writeHead(200, { ...h, 'Content-Type': 'text/html' });
      return res.end(`<!doctype html><meta charset="utf-8"><body style="background:#000"><script>window.__wenilla_env = ${JSON.stringify(env)};
        window.__wenilla_env_copy = JSON.stringify(window.__wenilla_env);</script>
        <script>${extractHop()}</script>`);
    }
    res.writeHead(200, { ...h, 'Content-Type': 'text/html' });
    res.end(`<!doctype html><meta charset="utf-8"><title>game</title>
<body style="margin:0;background:radial-gradient(120% 90% at 50% 40%,#22324a 0%,#0b0f18 70%);height:100vh"></body>
<script type="module">
  import * as m from './wrath-page.js';
  const env = { home: 'https://play.openvanilla.app', hop_ws: 'wss://relay.test/ws/${B}' };
  const out = [];
  let pending = null;
  const bridge = { ready: true, state: {}, lua: async (code) => { if (code === m.BROWSE_TAKE) { const a = pending; pending = null; return [a]; } out.push(code); return []; } };
  const net = m.makeNet({ env });
  const h = m.watchBrowse(bridge, { env, net, pollMs: 0 });
  window.T = { m, env, net, h, out, ask: (a) => { pending = a; return h.tick(); },
    hop: (id) => m.fastHop(id, { env, net }) };
</script>`);
  });
  function extractHop() {
    const early = fs.readFileSync(path.join(WWW, 'wrath-early.js'), 'utf8');
    return early.slice(early.indexOf('// >>> leyline-hop'), early.indexOf('// <<< leyline-hop'));
  }
  const browser = await chromium.launch();
  t.after(async () => { await browser.close(); site.srv.close(); game.srv.close(); });
  const page = await browser.newPage({ viewport: { width: 1229, height: 768 } });
  // the website (its address is https in the env, as live): served from the test's own site
  // and the relay at https://relay.test, as live: both served from the test's own site
  const proxy = async (route) => {
    const q = route.request();
    const u = new URL(q.url());
    if (q.method() === 'OPTIONS') {
      return route.fulfill({ status: 204, headers: { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Headers': 'Content-Type', 'Access-Control-Allow-Methods': 'POST, GET' } });
    }
    const r = await fetch(SITE + u.pathname, { method: q.method(), body: q.postData() || undefined, headers: { 'Content-Type': 'application/json', Cookie: (await q.allHeaders()).cookie || '' } });
    await route.fulfill({ status: r.status, headers: { 'Access-Control-Allow-Origin': '*', 'Content-Type': 'application/json' }, body: await r.text() });
  };
  await page.route('https://play.openvanilla.app/**', proxy);
  await page.route('https://relay.test/**', proxy);
  await page.goto(`http://127.0.0.1:${game.port}/`);
  await page.waitForFunction(() => !!window.T);
  assert.equal(await page.evaluate(() => crossOriginIsolated), true);

  // the list, through the bridge
  assert.equal(await page.evaluate(() => T.ask('list|1||')), 'list');
  const list = await page.evaluate(() => T.out[T.out.length - 1]);
  assert.match(list, /^WUUI_LEYLINE_NET = \{ v = 1, page = 1, pages = 1, total = 2/);
  assert.match(list, /name = "Frost Haven"/);
  assert.match(list, /hk = true/);
  assert.ok(seen.cookies.every((c) => c === ''), 'no cookies to the listings');
  // the preview: details and the Hero Key sign-in prepared (from the portal, same origin; the realm isn't asked)
  assert.equal(await page.evaluate((a) => T.ask('preview|' + a), A), 'preview');
  const pv = await page.evaluate(() => T.out[T.out.length - 1]);
  assert.match(pv, /hk = "ready"/);
  assert.match(pv, /who = \{ "Mira" \}/);
  assert.match(pv, /verdict = "ok"/);
  assert.deepEqual(seen.gate, [`?herokey=${A}`]);
  assert.equal(seen.redeem.length, 0, 'nothing sent to the realm at the preview');

  // another game version: the launcher bar, never a hop
  await page.evaluate((b) => T.ask('launch|' + b), B);
  const bar = await page.evaluate(() => { const b = document.getElementById('wu-hop-launch'); return b && { text: b.textContent, href: b.querySelector('a.go').getAttribute('href') }; });
  assert.equal(bar.href, `leyline://realm/${B}`);
  assert.match(bar.text, /needs a different game version/);
  assert.doesNotMatch(bar.text, /\d\.\d/);
  if (process.env.HOP_SHOTS) await page.screenshot({ path: path.join(process.env.HOP_SHOTS, 'page-launcher-bar.png') });
  await page.evaluate(() => document.getElementById('wu-hop-launch').remove());

  // Join: redeem at the relay, switch the session, land; the cover meanwhile
  hopDelay = 700;
  const nav = page.waitForURL((u) => u.search === `?go=r:${A}`);
  await page.evaluate((a) => { window.HOPPING = T.hop(a); }, A);
  await page.waitForSelector('#wu-hop-go');
  await page.waitForFunction(() => /Switching realm/.test(document.querySelector('#wu-hop-go small').textContent));
  const cover = await page.evaluate(() => document.getElementById('wu-hop-go').textContent);
  assert.match(cover, /Going to Frost Haven…/);
  if (process.env.HOP_SHOTS) await page.screenshot({ path: path.join(process.env.HOP_SHOTS, 'page-going-to.png') });
  await nav;
  assert.equal(seen.redeem.length, 1);
  assert.deepEqual(Object.keys(JSON.parse(seen.redeem[0])).sort(), ['assertion', 'sig']);
  assert.ok(!seen.redeem[0].includes('Q7W8E9R2T3Y4U5I6'), 'the password never goes to the relay');
  assert.equal(JSON.parse(seen.hopBody).sso, 'pre');
  // the landing page: wrath-early.js signs in with the kept one-time login, no redeem, no join page
  const st = await page.evaluate(() => ({ sso: window.__wuHop.state.sso, login: window.__wuHop.state.login, user: window.__wenilla_env.user,
    signedIn: window.__wuHop.status().signedIn }));
  assert.deepEqual(st, { sso: 'pre', login: true, user: 'HKABCDEFGHIJKLMN', signedIn: true });
  assert.equal(seen.redeem.length, 1, 'not redeemed twice');
});
