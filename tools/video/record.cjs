// Leyline feature videos: record the launcher (launcher/ui with the fake core in lib.cjs) and the website (a local run of
// stream/wrath_portal.py with fake data, read only) as screencast frames, one folder per scene, plus the cursor's path
// and clicks (the cursor itself is drawn by assemble.py).
//
//   SLOW=8 CATALOG=<the site's /wrath/mods/all.json, without entries naming the game> SITE=http://127.0.0.1:18951 \
//     node tools/video/record.cjs <out dir> [scene ...]
//
// SLOW runs the page's animations and every move here that many times slower (headless Chromium paints 1080p frames
// slowly; assemble.py plays it back at speed). w-mods and w-forum sometimes stall at 8: use 4 for those. Then
// tools/video/assemble.py makes the videos. Every scene checks the page text for the names we never show before it ends.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');
const L = require('./lib.cjs');
const { routes } = require('./routes.cjs');

const OUT = path.resolve(process.argv[2]);
const ONLY = process.argv.slice(3);
const SITE = process.env.SITE || 'http://127.0.0.1:18951';
const CATALOG = process.env.CATALOG ? JSON.parse(fs.readFileSync(process.env.CATALOG, 'utf8')) : { categories: [], projects: [] };
const WIDE = { width: +(process.env.WIDE_W || 1280), height: +(process.env.WIDE_H || 720), dpr: +(process.env.WIDE_DPR || 1.5) };   // 1920×1080 frames
const TALL = { width: 900, height: 1200, dpr: +(process.env.TALL_DPR || 1.8) };   // 1080×1440 frames, set into the 9:16 reel

let browser;
async function launcher(size, o = {}) {
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: size.width, height: size.height }, deviceScaleFactor: size.dpr });
  await ctx.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: 'null' }).catch(() => {});
  const page = await ctx.newPage();
  page.on('pageerror', (e) => console.error('  pageerror', String(e).slice(0, 200)));
  await routes(page);
  await page.addInitScript(L.FAKE, Object.assign({ modes: L.modes(), catalog: CATALOG, signed: true }, o));
  await page.goto(L.INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.__calls.some(([c]) => c === 'realms_list'));
  await page.waitForTimeout(1800);   // pictures in, the page fade done
  await L.setCursor(page, size.width * 0.62, size.height * 0.82);
  await page.waitForTimeout(200);
  return { ctx, page };
}
async function site(size, url) {
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: size.width, height: size.height }, deviceScaleFactor: size.dpr });
  await ctx.route(/^https?:\/\/(?!127\.0\.0\.1)/, (r) => r.abort());   // nothing but the local run
  const page = await ctx.newPage();
  await page.goto(SITE + url, { waitUntil: 'load' });
  await page.waitForTimeout(1500);
  await L.setCursor(page, size.width * 0.6, size.height * 0.8);
  return { ctx, page };
}
// a gentle scroll of whatever is under the cursor (by the clock, like the moves: as many wheel steps as the page takes)
async function scroll(page, dy, ms = 900) {
  const t0 = Date.now(), dur = ms * L.SLOW;
  let done = 0;
  for (;;) {
    const t = Math.min(1, (Date.now() - t0) / dur);
    const e = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
    const want = Math.round(dy * e);
    if (want !== done) { await page.mouse.wheel(0, want - done); done = want; }
    if (t >= 1) break;
    await page.waitForTimeout(30);
  }
}
const nav = (p) => `.nav[data-page="${p}"]`;

const SCENES = {
  // Home: the official realm's slide, one click on Play now
  'w-home': async (page, rec, size) => {
    await L.wait(page, 900);
    await L.hover(page, '.hero-slide.on .hs-title', 900).catch(() => {});
    rec.mark('hero');
    await L.wait(page, 500);
    await L.click(page, '.hero-slide.on button:has-text("Play now")', { ms: 800 });
    rec.mark('clicked');
    await L.wait(page, 2200);
  },
  // the play bar's realm picker, then Play
  'pick': async (page, rec) => {
    await L.wait(page, 700);
    await L.click(page, '#pb-realm-btn', { ms: 800 });
    rec.mark('open');
    await L.wait(page, 900);
    await L.hover(page, '#pb-rmenu >> text=Your Den', 500).catch(() => {});
    await L.wait(page, 250);
    await L.click(page, '#pb-rmenu >> text=Wrath Unleashed', { ms: 500 });
    rec.mark('picked');
    await L.wait(page, 500);
    await L.click(page, '#play-go', { ms: 800 });
    rec.mark('play');
    await L.wait(page, 2000);
  },
  // Your Den: the page, Play, Shape the ground (Forge opens, then Terrain updated), Save a copy
  'w-den': async (page, rec, size) => {
    await L.wait(page, 500);
    await L.click(page, nav('den'), { ms: 800 });
    rec.mark('den');
    await L.wait(page, 1300);
    await L.hover(page, '#yd .lede', 700);
    await L.wait(page, 700);
    await L.click(page, '#yd button:has-text("Play")', { ms: 700 });
    rec.mark('play');
    await L.wait(page, 1600);
    await L.moveTo(page, size.width * 0.6, size.height * 0.6, 500);
    await scroll(page, 520, 1100);
    await L.wait(page, 300);
    await L.click(page, '#yd button:has-text("Open in Forge")', { ms: 700 });
    rec.mark('forge');
    await page.waitForFunction(() => /Terrain updated/.test(document.getElementById('yd').textContent), null, { timeout: 15000 });
    rec.mark('terrain');
    await L.wait(page, 1200);
    await L.click(page, '#yd button:has-text("Save a copy") >> nth=0', { ms: 800 });
    rec.mark('saved');
    await L.wait(page, 1800);
  },
  // Your Den (tall, the Docker Den so Invite friends copies the link): Play, then Invite friends, then your friends
  't-den': async (page, rec, size) => {
    await L.wait(page, 600);
    await L.click(page, nav('den'), { ms: 800 });
    rec.mark('den');
    await L.wait(page, 1300);
    await L.hover(page, '#yd .lede', 700);
    await L.wait(page, 600);
    await L.click(page, '#yd button:has-text("Play")', { ms: 700 });
    rec.mark('play');
    await L.wait(page, 1800);
    await L.click(page, '#yd button:has-text("Invite friends")', { ms: 700 });
    rec.mark('invite');
    await L.wait(page, 2000);
    await L.click(page, '#hf-me-btn', { ms: 800 });
    rec.mark('friends');
    await L.wait(page, 2200);
  },
  // Host a server from a template, then You're live
  'host': async (page, rec, size) => {
    await L.wait(page, 500);
    await L.click(page, nav('host'), { ms: 800 });
    rec.mark('host');
    await page.waitForSelector('#hx .hx-card');
    await L.wait(page, 900);
    await L.hover(page, '[data-mode="levelling"] .hx-name', 700);
    await L.wait(page, 400);
    await L.click(page, '[data-mode="levelling"] .hx-host', { ms: 600 });
    rec.mark('form');
    await page.waitForSelector('#hx-name');
    await L.wait(page, 700);
    await L.click(page, '#hx-name', { ms: 700, after: 150 });
    await page.keyboard.press('Control+A');
    await L.typeSlow(page, 'Ember Vale', 75);
    await L.wait(page, 300);
    await L.click(page, '#hx-gm', { ms: 600, after: 150 });
    await L.typeSlow(page, 'KESTREL', 60);
    await L.wait(page, 250);
    const pub = await page.$('#hx-vis .hc-seg-b[data-value="public"]');
    if (pub) await L.click(page, '#hx-vis .hc-seg-b[data-value="public"]', { ms: 600 });
    rec.mark('typed');
    await L.click(page, '#hx-go', { ms: 700 });
    rec.mark('start');
    await page.waitForSelector('.hx-live-box', { timeout: 20000 });
    rec.mark('live');
    await L.wait(page, 500);
    await L.hover(page, '.hx-live-box .hc-live-t', 700).catch(() => {});
    await L.wait(page, 1500);
    // the network list: the Realms page, with the new realm on it
    await L.click(page, nav('realms'), { ms: 800 });
    rec.mark('realms');
    await L.wait(page, 1200);
    await L.hover(page, '.realm-card:has-text("Ember Vale")', 800);
    rec.mark('row');
    await L.wait(page, 2200);
  },
  // Mods & Addons: Discover, a look down the list, Install on one
  'w-mods': async (page, rec, size) => {
    await L.wait(page, 500);
    await L.click(page, nav('mods'), { ms: 800 });
    rec.mark('mods');
    await L.wait(page, 600);
    await L.click(page, '#tab-mods-discover', { ms: 600 });
    await L.wait(page, 500);
    await L.click(page, '#page-mods .ly-tab:visible:has-text("Addons")', { ms: 600 });
    rec.mark('discover');
    await L.wait(page, 1000);
    await L.moveTo(page, size.width * 0.6, size.height * 0.62, 500);
    await scroll(page, 330, 1200);
    await L.wait(page, 400);
    await L.click(page, '#pane-discover button:visible:has-text("Install") >> nth=0', { ms: 800 });
    rec.mark('install');
    await L.wait(page, 2200);
  },
  // the player portraits: your chip, Change name or picture, pick one
  'w-portraits': async (page, rec, size) => {
    await L.wait(page, 500);
    await L.click(page, '#hf-me-btn', { ms: 800 });
    rec.mark('panel');
    await L.wait(page, 1600);
    await L.click(page, '#hf-panel button:has-text("Change name or picture")', { ms: 700 });
    rec.mark('grid');
    await page.waitForSelector('#ac-view input[name="ac-avatar"]');
    await L.wait(page, 900);
    await L.click(page, '#ac-view label:has(input[value="owl"])', { ms: 900 });
    await L.wait(page, 500);
    await L.click(page, '#ac-view label:has(input[value="knight"])', { ms: 700 });
    rec.mark('picked');
    await L.wait(page, 1600);
  },
  // the website: the forum, then the Discord
  'site-forum': async (page, rec, size) => {
    await L.wait(page, 900);
    await L.hover(page, 'text=Welcome to the forum >> nth=0', 900).catch(() => {});
    rec.mark('thread');
    await L.wait(page, 1000);
    await L.hover(page, 'a.fm-btn:has-text("Join the Discord")', 900);
    rec.mark('discord');
    await L.wait(page, 1800);
  },
  'site-realms': async (page, rec, size) => {
    await L.wait(page, 800);
    await L.moveTo(page, size.width * 0.6, size.height * 0.55, 500);
    await scroll(page, 300, 1400);
    await L.wait(page, 1200);
  },
  // the website's realm list on a phone: down to the new realm
  'site-ember': async (page, rec, size) => {
    await L.wait(page, 700);
    await L.moveTo(page, size.width * 0.55, size.height * 0.6, 500);
    const y = await page.evaluate(() => { const h = [...document.querySelectorAll('h2, h3, b, strong, a, span')].find((e) => e.textContent.trim() === 'Ember Vale'); return h.getBoundingClientRect().top; });
    rec.mark('scroll');
    await scroll(page, y - 150, 1600);
    await L.wait(page, 300);
    await L.hover(page, 'text=Ember Vale >> nth=0', 700);
    rec.mark('row');
    await L.wait(page, 2200);
  },
  'site-bug': async (page, rec, size) => {   // Report a bug: opened, never sent
    await L.wait(page, 800);
    await L.hover(page, 'form textarea, form input[type="text"] >> nth=0', 900).catch(() => {});
    await L.wait(page, 1500);
  },
};
const PLAN = {
  'w-home': ['launcher', WIDE, {}],
  'w-pick': ['launcher', WIDE, {}, 'pick'],
  'w-den': ['launcher', WIDE, {}],
  'w-host': ['launcher', WIDE, {}, 'host'],
  'w-mods': ['launcher', WIDE, {}],
  'w-portraits': ['launcher', WIDE, {}],
  'w-forum': ['site', WIDE, '/wrath/leyline/forum', 'site-forum'],
  't-home': ['launcher', TALL, {}, 'w-home'],
  't-pick': ['launcher', TALL, {}, 'pick'],
  't-den': ['launcher', TALL, { dockerDen: true }],
  't-host': ['launcher', TALL, {}, 'host'],
  't-site-realms': ['site', { width: 432, height: 576, dpr: 2.5 }, '/wrath/leyline/realms', 'site-ember'],
};

(async () => {
  const browsers = {};
  for (const [name, [kind, size, arg, scene]] of Object.entries(PLAN)) {
    if (ONLY.length && !ONLY.includes(name)) continue;
    // the screencast gives device pixels only when the whole browser runs at that scale
    browser = browsers[size.dpr] = browsers[size.dpr] || await chromium.launch({ args: ['--force-device-scale-factor=' + size.dpr] });
    const { ctx, page } = kind === 'site' ? await site(size, arg) : await launcher(size, arg);
    const rec = await L.record(page, path.join(OUT, name));
    try {
      await SCENES[scene || name](page, rec, size);
      await L.brandCheck(page, name);
    } catch (e) { console.error(name, 'FAILED', String(e).split('\n')[0]); }
    const n = await rec.stop();
    const unknown = kind === 'site' ? [] : await page.evaluate(() => [...new Set(window.__unknown)]);
    console.log(name, n, 'frames', unknown.length ? 'unknown: ' + unknown.join(' ') : '');
    await ctx.close();
  }
  for (const b of Object.values(browsers)) await b.close();
})();
