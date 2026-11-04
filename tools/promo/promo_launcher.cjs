// Promo clip: the Leyline launcher UI (launcher/ui) recorded in headless Chromium, for Reels / TikTok (owner 3 Nov, launch
// night: "what Reels / TikToks could we quickly make to show off the launcher? you autorecord them and i post").
//
// The fake Tauri core is the first-run walk's (launcher/tests/firstrun.browser.test.cjs FAKE) with honest data swapped
// in: only the real official realm (no invented community realms, no player counts), and the real catalog's addons
// (stream/upstream-addons.json) and packaged mods (mods/catalog.json). A visible cursor dot shows each click.
// Writes <out>/launcher-raw.webm and <out>/marks.json (where each scene starts, for the cut).
//
//   NODE_PATH=$(npm root -g) node tools/promo/promo_launcher.cjs [out dir]     (default /tmp/promo)
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..', '..');
const INDEX = 'file://' + path.join(ROOT, 'launcher', 'ui', 'index.html');
const PICTURE = path.join(ROOT, 'launcher', 'ui', 'art', 'leyline-bg.webp');
const OUT = path.resolve(process.argv[2] || '/tmp/promo');
const SIZE = { width: 1280, height: 800 };
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;

function firstRunFake() {
  const src = fs.readFileSync(path.join(ROOT, 'launcher', 'tests', 'firstrun.browser.test.cjs'), 'utf8');
  const a = src.indexOf('const FAKE = () => {');
  const b = src.indexOf('\n};\n', a);
  return src.slice(a + 'const FAKE = '.length, b + 2);
}

const UP = JSON.parse(fs.readFileSync(path.join(ROOT, 'stream', 'upstream-addons.json'), 'utf8')).projects;
const PK = JSON.parse(fs.readFileSync(path.join(ROOT, 'mods', 'catalog.json'), 'utf8')).mods || [];
const clean = (s) => String(s || '').replace(BRANDS, '').replace(/\s{2,}/g, ' ').trim();
const PROJECTS = [
  ...PK.map((m) => ({ id: 'mod:' + m.id, source: 'packaged', name: m.name, description: clean(m.description), category: m.category || 'sandbox',
    builds: ['3.3.5a'], author: m.author === 'WrathUnleashed' ? 'Leyline' : m.author, author_url: '', licence: m.licence || '', repo: '', page: '',
    credits: '', stars: null, last_push: '', version: m.version, kind: m.kind || 'both', permissions: m.permissions || [], on_wrath: true,
    download: 'x', sha256: 'ab'.repeat(32), size: m.size || 1000 })),
  ...UP.map((p) => ({ ...p, description: clean(p.description), on_wrath: false, stars: p.stars ?? null })),
];

const HONEST = (o) => {
  const base = window.__TAURI__.core.invoke;
  const now = Math.floor(Date.now() / 1000);
  const official = { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a',
    core: 'azerothcore', tags: [], players: null, bots: 0, address: 'leylinelauncher.com' };
  const catalog = { categories: [{ id: 'addons', name: 'Addons and UI', line: '', icon: '' }, { id: 'sandbox', name: 'Sandbox and game modes', line: '', icon: '' }],
    projects: o.projects };
  const S = window.__S;
  S.clients.push({ id: 'c335a-1', name: 'Wrath 3.3.5a', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '', locale: 'enUS' });
  S.selected_client = 'c335a-1';
  S.seen = ['tour', 'play', 'den', 'host', 'files', 'cheer-hosted', 'cheer-friend', 'cheer-mod'];
  window.__TAURI__.core.invoke = async (cmd, args) => {
    switch (cmd) {
      case 'realms_list': return { realms: [official], network: { realms: 1, players: 0, bots: 0 }, from_cache: false, saved_at: now, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 40, players: null };
      case 'mods_catalog': return { from_cache: false, saved_at: now, error: null, ...JSON.parse(JSON.stringify(catalog)) };
      // the forum's one real thread (the 3 Nov audit read it), not the test's invented ones
      case 'forum_call':
        if (args && (args.action === 'latest' || args.action === 'threads'))
          return { threads: [{ id: 1, cat: 'news', title: 'Welcome to the Leyline forum', author: 'Orion', tag: '0001', created: now - 86400,
            bumped: now - 86400, replies: 0, pinned: true, locked: false, url: 'https://leylinelauncher.com/wrath/leyline/forum/t/1' }], page: 1, more: false };
        return base(cmd, args);
      case 'mods_ranked': return { ids: catalog.projects.map((p) => p.id), metrics: {}, from_cache: false, error: null };
      default: return base(cmd, args);
    }
  };
  // the cursor: a soft ring that follows the mouse and pulses on a click
  addEventListener('DOMContentLoaded', () => {
    const c = document.createElement('div');
    c.style.cssText = 'position:fixed;z-index:2147483647;width:26px;height:26px;margin:-13px 0 0 -13px;border-radius:50%;'
      + 'border:3px solid #fff;box-shadow:0 0 0 2px rgba(0,0,0,.45),0 0 14px rgba(140,170,255,.8);pointer-events:none;'
      + 'left:-50px;top:-50px;transition:transform .12s';
    document.body.appendChild(c);
    addEventListener('mousemove', (e) => { c.style.left = e.clientX + 'px'; c.style.top = e.clientY + 'px'; }, true);
    addEventListener('mousedown', () => { c.style.transform = 'scale(.6)'; }, true);
    addEventListener('mouseup', () => { c.style.transform = 'scale(1)'; }, true);
  });
};

async function main() {
  fs.mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch();
  const marks = [];
  let t0 = 0;
  const mark = (name) => marks.push({ name, t: (Date.now() - t0) / 1000 });
  try {
    const ctx = await browser.newContext({ colorScheme: 'dark', viewport: SIZE, recordVideo: { dir: OUT, size: SIZE } });
    const page = await ctx.newPage();
    t0 = Date.now();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    page.on('dialog', (d) => d.dismiss());
    await page.route('https://**', (r) => (r.request().resourceType() === 'image' ? r.fulfill({ path: PICTURE, contentType: 'image/webp' }) : r.fulfill({ status: 404, body: '' })));
    await page.addInitScript({ content: `(${firstRunFake()})();` });
    await page.addInitScript(HONEST, { projects: PROJECTS });
    await page.goto(INDEX);
    await page.waitForFunction(() => window.LeylineNav && window.LeylineApp && window.LeylineApp.state());
    // signed in off camera (a test Hero Key the fake core takes)
    await page.waitForSelector('#hf-signin');
    if (!(await page.evaluate(() => window.LeylineGate.isOpen()))) await page.click('#hf-signin');
    await page.click('#gate-keylink');
    await page.fill('#gate-key', 'ovk1.TestTestAa.' + 'k'.repeat(43));
    await page.click('#gate-keyform button[type="submit"]');
    await page.click('#gate-go');
    await page.waitForSelector('#hf-me-btn');

    const glide = async (sel, opts = {}) => {
      const box = await page.locator(sel).first().boundingBox();
      if (!box) return;
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 18 });
      await page.waitForTimeout(opts.pause ?? 250);
      if (opts.click !== false) await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    };
    const nav = async (name) => { await glide(`.side .nav[data-page="${name}"]`); await page.waitForFunction((n) => document.querySelector('#page-' + n + '.on'), name); };

    await nav('play');
    await page.waitForTimeout(600);
    mark('home');
    await page.waitForTimeout(1600);
    await glide('#pb-realm-btn');
    await page.waitForTimeout(1400);
    await page.keyboard.press('Escape');
    mark('realms');
    await nav('realms');
    await page.waitForTimeout(1800);
    mark('mods');
    await nav('mods');
    await page.waitForTimeout(500);
    await glide('#discover-tabs .ly-tab[data-tab="addons"]');
    await page.waitForFunction(() => /AdiBags/.test(document.getElementById('pane-discover').textContent));
    await page.waitForTimeout(900);
    await page.mouse.move(700, 500, { steps: 10 });
    for (let i = 0; i < 6; i++) { await page.mouse.wheel(0, 140); await page.waitForTimeout(260); }
    await page.waitForTimeout(400);
    for (let i = 0; i < 6; i++) { await page.mouse.wheel(0, -140); await page.waitForTimeout(120); }
    const install = page.locator('#pane-discover', { hasText: 'AdiBags' }).locator('button', { hasText: /^install$/i }).first();
    if (await install.count()) { const b = await install.boundingBox(); if (b) { await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 16 }); await page.waitForTimeout(250); await install.click(); } }
    await page.waitForTimeout(2200);
    mark('host');
    await nav('host');
    await page.waitForSelector('#hst .hst-vis');
    await page.waitForTimeout(800);
    await glide('.hst-vis-b[data-vis="private"]');
    await page.waitForTimeout(400);
    await page.locator('.hst-s2').scrollIntoViewIfNeeded();
    await page.waitForTimeout(900);
    await glide('.hst-tpl[data-tpl="vanilla-plus"]');
    await page.waitForTimeout(1800);
    mark('den');
    await nav('play');
    await glide('#pb-realm-btn');
    await page.waitForTimeout(500);
    await glide('#pb-rmenu >> text=Your Den');
    await page.waitForSelector('#page-den.on');
    await page.waitForTimeout(2400);
    mark('end');
    const words = await page.evaluate(() => document.body.innerText);
    const brand = words.match(BRANDS);
    await ctx.close();
    const vid = fs.readdirSync(OUT).filter((f) => f.endsWith('.webm')).map((f) => path.join(OUT, f))
      .sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs)[0];
    fs.renameSync(vid, path.join(OUT, 'launcher-raw.webm'));
    fs.writeFileSync(path.join(OUT, 'marks.json'), JSON.stringify(marks, null, 1));
    process.stdout.write(JSON.stringify(marks) + '\n');
    if (errors.length) process.stdout.write('page errors: ' + errors.join('; ').slice(0, 400) + '\n');
    if (brand) { process.stdout.write(`a game or publisher name shows: ${brand[0]}\n`); process.exit(1); }
  } finally {
    await browser.close();
  }
}

main().catch((e) => { process.stderr.write(String(e && e.stack || e).slice(0, 800) + '\n'); process.exit(2); });
