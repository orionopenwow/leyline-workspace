// Headless Chromium tests (Playwright) for the website on phones (site review 4 Nov): the portal
// (stream/wrath_portal.py, with the hub's front door on, fake services, nothing live) on loopback,
// every main page at 360 and 390 px wide: nothing scrolls sideways (the Mods tab bar did, by 57 px,
// with "Loadouts" cut off), no script errors, no picture that fails to load, one h1.
//
//   node --test tests/site-phone.browser.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { chromium } = require('playwright');

const PORTAL = path.join(__dirname, '..', 'wrath_portal.py');
const PORT = 18496;
const BOOT = `
import importlib.util, os, sys, tempfile, threading, time, shutil
from http.server import ThreadingHTTPServer
here = os.path.dirname(os.path.abspath(sys.argv[1])); tmp = tempfile.mkdtemp(prefix="wu-phone-")
for d in ("data", "mods/files", "armory/c", "hub", "items"): os.makedirs(os.path.join(tmp, d), exist_ok=True)
shutil.copy(os.path.join(here, "hub", "community-index.json"), os.path.join(tmp, "hub"))
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
    WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="leylinelauncher.com", WU_DL_PROBE="0", WU_HUB_FRONT="1",
    WU_RELAY_LISTING="http://127.0.0.1:9/realms.json", WU_PORTAL_DATA=os.path.join(tmp, "data"),
    WU_HUB_DIR=os.path.join(tmp, "hub"), WU_MODS_DIR=os.path.join(tmp, "mods"), WU_WORKSHOP_ITEMS_DIR=os.path.join(tmp, "items"),
    WU_ARMORY_DIR=os.path.join(tmp, "armory"), WU_ART_DIR=os.path.join(here, "www-files", "art"))
for k in ("WU_HUB", "WU_BRAND_FILE", "WU_GUEST_DEMO"): os.environ.pop(k, None)
spec = importlib.util.spec_from_file_location("wp", sys.argv[1]); wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
wp.realm_status = lambda: {"online": True, "players": 3, "bots": 10}
wp.social.setup(start_thread=False); wp.profiles.setup(); wp.herokeys.setup(); wp.forum.setup()
s = ThreadingHTTPServer(("127.0.0.1", int(sys.argv[2])), wp.PageHandler); s.daemon_threads = True
print("ready", flush=True); s.serve_forever()
`;

const PAGES = ['/wrath/leyline', '/wrath/leyline/catalog', '/wrath/leyline/addons', '/wrath/leyline/game-modes',
  '/wrath/leyline/loadouts', '/wrath/leyline/realms', '/wrath/leyline/forum', '/wrath/leyline/gallery',
  '/wrath/leyline/news', '/wrath/leyline/creators', '/wrath/leyline/submit', '/wrath/key', '/wrath/signin',
  '/wrath/start', '/wrath/download', '/wrath/looks', '/wrath/docs', '/wrath/docs/launcher', '/wrath/bugs',
  '/wrath/submit', '/wrath/survivors', '/wrath/leaderboards', '/wrath/host', '/wrath/modes', '/wrath/mobile', '/wrath/docs/mobile'];

let server, browser;
test.before(async () => {
  server = spawn('python3', ['-I', '-c', BOOT, PORTAL, String(PORT)], { stdio: ['ignore', 'pipe', 'pipe'] });
  await new Promise((ok, bad) => {
    const t = setTimeout(() => bad(new Error('the portal did not start')), 60000);
    server.stdout.on('data', (d) => { if (String(d).includes('ready')) { clearTimeout(t); ok(); } });
    server.on('exit', (c) => bad(new Error('the portal exited ' + c)));
  });
  browser = await chromium.launch();
});
test.after(async () => { if (browser) await browser.close(); if (server) server.kill(); });

for (const width of [360, 390]) {
  test(`phones ${width} px: no sideways scroll, no script errors, pictures load, one h1`, async () => {
    const ctx = await browser.newContext({ viewport: { width, height: 800 }, extraHTTPHeaders: { 'X-Forwarded-For': `10.96.0.${width % 250}` } });
    const pg = await ctx.newPage();
    const errs = [];
    pg.on('pageerror', (e) => errs.push(String(e)));
    const bad = [];
    for (const p of PAGES) {
      errs.length = 0;
      const r = await pg.goto(`http://127.0.0.1:${PORT}${p}`, { waitUntil: 'load' });
      if (r.status() >= 500) { bad.push(`${p}: ${r.status()}`); continue; }
      const got = await pg.evaluate(() => ({
        over: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        broken: [...document.images].filter((i) => i.complete && i.naturalWidth === 0 && i.loading !== 'lazy').map((i) => i.getAttribute('src')),
        h1: document.querySelectorAll('h1').length,
      }));
      if (got.over > 0) bad.push(`${p}: scrolls sideways by ${got.over} px`);
      if (got.broken.length) bad.push(`${p}: pictures that don't load: ${got.broken.join(' ')}`);
      if (got.h1 !== 1) bad.push(`${p}: ${got.h1} h1`);
      if (errs.length) bad.push(`${p}: script errors: ${errs.join(' | ')}`);
    }
    await ctx.close();
    assert.deepEqual(bad, []);
  });
}
