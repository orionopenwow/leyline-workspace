// The phone app (docs/ANDROID.md; ui/platform.js, ui/phone.js, ui/phone.css) in Chromium as a phone, with a fake core
// that answers like the phone build's (src-tauri/src/mobile.rs: its short list of commands, every other one "not found"):
// platform "android" puts the phone layout on (html.phone) and hides everything that needs a computer (the side menu,
// the play bar, the window buttons, game folders, Host, Your Den, Single player, installs, Themes, Graphics, hosting on
// Realms); a tab bar (Play, Realms, Mods, Account) instead; at 390x844 and 844x390 every tab fits the width (no sideways
// scrolling), every control is at least 44 px, no banned words; PLAY opens the website's Play page in the app and Back
// comes back; a web view without WebGPU gets one line and Open in Chrome; the official realm's Play is the browser game,
// a community realm or a Den invite says it needs a computer; a computer (or an older core) keeps the desktop launcher.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/phone.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PLAY_URL = 'https://leylinelauncher.com/wrath';
const SOLO_URL = 'https://leylinelauncher.com/wrath/play?mini=mini&go=solo:phone';
const PORTRAIT = { width: 390, height: 844 };
const LANDSCAPE = { width: 844, height: 390 };
const BANNED = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;

const P = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: `${name} does a thing.`, category: 'addons',
  builds: ['3.3.5a'], author: 'Leyline', author_url: '', licence: 'MIT', repo: '', page: '', credits: name, stars: null, last_push: '',
  version: '0.1.0', kind: 'ui', permissions: [], on_wrath: true, download: 'https://leylinelauncher.com/wrath/mods/x/download',
  sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
const CATALOG = { categories: [{ id: 'addons', name: 'Addons and UI' }, { id: 'sandbox', name: 'Sandbox and game modes' }],
  projects: [P('mod:pings', 'Pings'), P('mod:dice', 'Dalaran Dice', { category: 'sandbox', kind: 'both' }),
    P('mod:far', 'Far Sight', { category: 'sandbox', kind: 'client' })], from_cache: false, saved_at: 1, error: null };

// the phone build's core (mobile.rs), or a computer's (os: 'windows') for the last test
const FAKE = ({ os, gpu, catalog, page }) => {
  const calls = (window.__calls = []);
  try { if (page) localStorage.setItem('leyline.page', page); else localStorage.removeItem('leyline.page'); } catch (_) { /* fine */ }
  // WebGPU: a phone's web view has it, or not (navigator.gpu)
  Object.defineProperty(Navigator.prototype, 'gpu', { configurable: true, get: () => (gpu ? { requestAdapter: async () => ({}) } : undefined) });
  const mobile = os === 'android';
  const base = { official: false, kind: 'PvE', ruleset: 'main', state: 'live', version: '3.3.5a', core: 'azerothcore', build: '3.3.5a', tags: [],
    region: null, language: null, max_players: null, is_new: false, featured: false, verified: false, direct: false, rules: [], claimed: true,
    players: 0, bots: 0, tagline: '', summary: '', join_url: null, discord: null, relay: 'relay.leylinelauncher.com', report_url: null };
  const realms = [
    Object.assign({}, base, { slug: 'official', name: 'Wrath Unleashed', official: true, address: 'leylinelauncher.com', group: 'official', players: 12, bots: 40 }),
    Object.assign({}, base, { slug: 'c-bbbbbbbbbbbbbbbb', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Busy Den with a long realm name for a small screen',
      tags: ['bots'], players: 30, bots: 40, max_players: 100, address: 'relay.leylinelauncher.com:40002', group: 'community' }),
  ];
  const platform = { os, mobile, node: !mobile };
  const ok = {
    platform,
    get_state: { version: 'test', settings: { clients: [], selected_client: null, selected_realm: 'leylinelauncher.com',
      wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'leylinelauncher.com',
      browser_url: 'https://leylinelauncher.com/wrath', windows: false, data_dir: '/data/user/0/com.leylinelauncher.app', platform },
    realms_list: { realms, network: { realms: 1, players: 42, bots: 80 }, from_cache: false, saved_at: 1, error: null },
    realm_status: { state: 'online', ping_ms: 40, players: 12 },
    mods_catalog: catalog || { categories: [], projects: [], from_cache: false, saved_at: 1, error: null },
    mods_ranked: { ids: [], metrics: {}, from_cache: false, error: null },
    deep_link_take: null,
    social_status: true,
    modes_list: { modes: [], from_cache: false, builtin: false, saved_at: null, error: null },
    news_list: { posts: [], from_cache: false, error: null },
    open_browser_play: null, open_link: null, open_url: null,
  };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    if (Object.prototype.hasOwnProperty.call(ok, cmd)) return JSON.parse(JSON.stringify(ok[cmd]));
    throw `Command ${cmd} not found`;   // what Tauri says for a command the phone build doesn't have
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts = {}, viewport = PORTRAIT) {
  const ctx = await browser.newContext({ viewport, hasTouch: true, isMobile: true, colorScheme: 'dark' });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route(/^https?:\/\//, (r) => (r.request().url() === PLAY_URL || r.request().url() === SOLO_URL
    ? r.fulfill({ contentType: 'text/html', body: `<!doctype html><title>Play</title><p id="game">${r.request().url() === SOLO_URL ? 'solo' : 'the browser game'}</p>` })
    : r.abort()));
  await page.addInitScript(FAKE, Object.assign({ os: 'android', gpu: true }, opts));
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylinePhone && document.querySelector('.page.on'));
  if ((opts.os || 'android') === 'android') await page.waitForFunction(() => window.LeylinePhone.on && document.querySelector('#page-phone.on'));
  return { page, ctx, errors };
}
const tab = async (page, name) => {
  await page.click(`#phone-tabs [data-ph-tab="${name}"]`);
  await page.waitForTimeout(250);
};
const visible = (page, sel) => page.evaluate((sel) => [...document.querySelectorAll(sel)].some((e) => {
  const r = e.getBoundingClientRect(); const s = getComputedStyle(e);
  return s.display !== 'none' && s.visibility !== 'hidden' && r.width > 0 && r.height > 0;
}), sel);

// what a phone screen shows: nothing wider than the screen (no sideways scrolling, nothing cut off at the right) and every
// control a finger can hit (44 px), in the page, the tab bar and the title bar
const fit = (page) => page.evaluate(() => {
  const W = innerWidth;
  const main = document.querySelector('main');
  const shown = (e) => { const s = getComputedStyle(e); const r = e.getBoundingClientRect(); return s.display !== 'none' && s.visibility !== 'hidden' && r.width > 0 && r.height > 0; };
  const clipped = (e) => { for (let p = e.parentElement; p && p !== main; p = p.parentElement) { const o = getComputedStyle(p).overflowX; if (o !== 'visible') return true; } return false; };
  const wide = [], small = [];
  for (const e of document.querySelectorAll('main *, #phone-tabs *, .titlebar *')) {
    if (!shown(e)) continue;
    const r = e.getBoundingClientRect();
    if (r.right > W + 1 && !clipped(e)) wide.push(`${e.tagName}.${e.className}#${e.id}`);
    if (e.matches('button, .btn, select, input:not([type="checkbox"]):not([type="radio"]), summary, a[href]') && (r.height < 44 || r.width < 44)) {
      small.push(`${e.tagName}#${e.id} "${(e.textContent || e.value || '').trim().slice(0, 24)}" ${Math.round(r.width)}x${Math.round(r.height)}`);
    }
  }
  return { scroll: main.scrollWidth - main.clientWidth, docScroll: document.documentElement.scrollWidth - W, wide: wide.slice(0, 8), small: small.slice(0, 8),
    text: document.body.innerText };
});

test('platform "android": the phone layout, a tab bar, and nothing that needs a computer', async () => {
  const { page, ctx, errors } = await open();
  assert.equal(await page.evaluate(() => document.documentElement.className.includes('phone') && document.documentElement.dataset.platform), 'android');
  assert.deepEqual(await page.evaluate(() => ({ os: window.LeylinePlatform.os, mobile: window.LeylinePlatform.mobile, node: window.LeylinePlatform.node })),
    { os: 'android', mobile: true, node: false }, 'Be a node is off on a phone');
  assert.match(await page.getAttribute('meta[name="viewport"]', 'content'), /viewport-fit=cover/);
  assert.deepEqual(await page.$$eval('#phone-tabs .ph-tab', (b) => b.map((x) => [x.dataset.phTab, x.textContent.trim()])),
    [['phone', 'Play'], ['realms', 'Realms'], ['mods', 'Mods'], ['account', 'Account']]);
  assert.equal(await page.getAttribute('#phone-tabs [data-ph-tab="phone"]', 'aria-current'), 'page');
  for (const sel of ['.side', '.playbar', '.winbtns', '#play-go', '#pb-client', '.titlebar .search', '#page-play']) {
    assert.equal(await visible(page, sel), false, `${sel} is hidden on a phone`);
  }
  // the pages that need a computer never open: a button or a remembered page that leads there lands on Play
  for (const p of ['clients', 'host', 'den', 'single', 'settings', 'agents', 'findfiles', 'play']) {
    await page.evaluate((p) => window.LeylineNav.show(p), p);
    assert.deepEqual(await page.$$eval('.page.on', (s) => s.map((x) => x.id)), ['page-phone'], p);
  }
  // Realms: no game folder picker, no hosting, no paste-a-realm, no realmlist menu
  await tab(page, 'realms');
  await page.waitForSelector('#rl-list .realm-card');
  for (const sel of ['#rl-host-cta', '#rl-host', '.rl-client', '#rl-paste', '#rl-more', '#rl-back', '.solo-go', '.solo-note']) {
    assert.equal(await visible(page, sel), false, `${sel} is hidden on Realms`);
  }
  // Mods: Discover only, no Themes or Graphics, no List your mod
  await tab(page, 'mods');
  for (const sel of ['#tab-mods-installed', '#tab-mods-gamemodes', '#tab-mods-dens', '#discover-tabs [data-tab="themes"]',
    '#discover-tabs [data-tab="graphics"]', '#dc-list-mod']) {
    assert.equal(await visible(page, sel), false, `${sel} is hidden on Mods`);
  }
  assert.ok(await visible(page, '#tab-mods-discover'));
  assert.ok(await visible(page, '#discover-tabs [data-tab="looks"]'), 'Looks stay, to read about');
  assert.deepEqual(await page.evaluate(() => window.LeylineNav.current()), { page: 'mods', tab: 'discover' });
  await page.evaluate(() => window.LeylineNav.show('mods', false, 'installed'));
  assert.deepEqual(await page.evaluate(() => window.LeylineNav.current()), { page: 'mods', tab: 'discover' }, 'Installed sends back to Discover');
  // the desktop's first-run tour doesn't start on a phone
  await page.waitForTimeout(800);
  assert.equal(await page.locator('.gd-card, .gd-ring').count(), 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

for (const [label, viewport] of [['portrait 390x844', PORTRAIT], ['landscape 844x390', LANDSCAPE]]) {
  test(`${label}: every tab fits the screen, with 44 px controls and no banned words`, async () => {
    const { page, ctx, errors } = await open({ catalog: CATALOG }, viewport);
    for (const t of ['phone', 'realms', 'mods', 'account']) {
      await tab(page, t);
      if (t === 'realms') await page.waitForSelector('#rl-list .realm-card');
      if (t === 'realms') { await page.click('#rl-other summary'); await page.waitForTimeout(150); }
      if (t === 'mods') await page.waitForSelector('#page-mods .mod-card, #page-mods .dc-card', { timeout: 4000 }).catch(() => {});
      const f = await fit(page);
      assert.ok(f.scroll <= 0 && f.docScroll <= 0, `${t}: no sideways scrolling (${f.scroll}, ${f.docScroll})`);
      assert.deepEqual(f.wide, [], `${t}: nothing wider than the screen`);
      assert.deepEqual(f.small, [], `${t}: every control at least 44 px`);
      assert.doesNotMatch(f.text, BANNED, t);
      assert.equal(await page.getAttribute(`#phone-tabs [data-ph-tab="${t}"]`, 'aria-current'), 'page', t);
    }
    // the tab bar sits at the bottom of the screen, the full width
    const bar = await page.$eval('#phone-tabs', (e) => { const r = e.getBoundingClientRect(); return [Math.round(r.left), Math.round(r.right), Math.round(r.bottom)]; });
    assert.deepEqual(bar, [0, viewport.width, viewport.height]);
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

test('PLAY opens the website\'s Play page in the app; Back comes back to the launcher', async () => {
  const { page, ctx, errors } = await open();
  assert.ok(await visible(page, '#ph-play'));
  assert.equal(await page.isHidden('#ph-nogpu'), true);
  await Promise.all([page.waitForURL(PLAY_URL), page.click('#ph-play')]);
  assert.equal(await page.textContent('#game'), 'the browser game');
  await page.goBack();
  await page.waitForFunction(() => window.LeylinePhone && window.LeylinePhone.on && document.querySelector('#page-phone.on'));
  assert.equal(page.url(), INDEX);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Play solo on this phone opens the game page in solo through the site\'s Play; Back comes back', async () => {
  const { page, ctx, errors } = await open();
  assert.ok(await visible(page, '#ph-solo'));
  await Promise.all([page.waitForURL(SOLO_URL), page.click('#ph-solo')]);
  assert.equal(await page.textContent('#game'), 'solo');
  await page.goBack();
  await page.waitForFunction(() => window.LeylinePhone && window.LeylinePhone.on && document.querySelector('#page-phone.on'));
  // no WebGPU: the same one line as PLAY, never a black page
  await ctx.close();
  const b = await open({ gpu: false });
  await b.page.click('#ph-solo');
  assert.equal(b.page.url(), INDEX);
  assert.ok(await visible(b.page, '#ph-nogpu'));
  assert.deepEqual(errors, []);
  assert.deepEqual(b.errors, []);
  await b.ctx.close();
});

test('no WebGPU in this web view: one line and Open in Chrome, never a black page', async () => {
  const { page, ctx, errors } = await open({ gpu: false });
  await page.click('#ph-play');
  assert.equal(page.url(), INDEX, 'the app stays on the launcher');
  assert.ok(await visible(page, '#ph-nogpu'));
  assert.equal(await page.textContent('#ph-nogpu p'), "This phone's web view can't draw the game yet: open in Chrome.");
  await page.click('#ph-chrome');
  await page.waitForTimeout(100);
  assert.ok(await page.evaluate(() => window.__calls.some(([c]) => c === 'open_browser_play')), 'Open in Chrome asks the core to open the Play page in the browser');
  // the official realm's Play on Realms says the same
  await tab(page, 'realms');
  await page.waitForSelector('#rl-list .realm-card .rl-play');
  await page.click('#rl-list .realm-card:has(.rl-bdg.o) .rl-play');
  assert.deepEqual(await page.$$eval('.page.on', (s) => s.map((x) => x.id)), ['page-phone']);
  assert.ok(await visible(page, '#ph-nogpu'));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Realms: the official realm plays in the app; a community realm and a Den invite say they need a computer', async () => {
  const { page, ctx, errors } = await open();
  await tab(page, 'realms');
  await page.waitForSelector('#rl-list .realm-card .rl-play');
  const play = await page.$eval('#rl-list .realm-card:has(.rl-bdg.o) .rl-play', (b) => [b.disabled, b.dataset.phone]);
  assert.deepEqual(play, [false, 'browser'], 'no game folder needed for the official realm on a phone');
  // a community realm
  await page.click('#rl-tab-community');
  await page.waitForSelector('#rl-list .realm-card:not(:has(.rl-bdg.o)) .rl-play');
  await page.click('#rl-list .realm-card:not(:has(.rl-bdg.o)) .rl-play');
  assert.match(await page.textContent('#rl-status'), /^Community realms play with your own game on a computer/);
  // a friend's Den invite: opens, and explains
  await page.click('#rl-other summary');
  await page.fill('#rl-invite', 'DEN-ABCD-EFGH');
  await page.click('#rl-join');
  assert.match(await page.textContent('#rl-status'), /friend's Den\. Dens run in the game on a computer/);
  const started = await page.evaluate(() => window.__calls.map(([c]) => c)
    .filter((c) => /^(play|play_realm|play_realm_herokey|join_invite|den_launch|den_join_page|den_invite|host_start|engine_play)$/.test(c)));
  assert.deepEqual(started, [], 'nothing desktop-only was asked of the core');
  // the official realm: the browser game in the app
  await page.click('#rl-tab-official');
  await page.waitForSelector('#rl-list .realm-card:has(.rl-bdg.o) .rl-play');
  await Promise.all([page.waitForURL(PLAY_URL), page.click('#rl-list .realm-card:has(.rl-bdg.o) .rl-play')]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Mods: browsing only; the install buttons are hidden and one line says installs need a computer', async () => {
  const { page, ctx, errors } = await open({ catalog: CATALOG });
  await tab(page, 'mods');
  await page.waitForSelector('#page-mods .dc-card, #page-mods .mod-card', { timeout: 4000 }).catch(() => {});
  assert.equal(await page.textContent('#ph-mods-note'), 'Browse here. Installing mods, addons, loadouts and looks needs Leyline on your computer for now.');
  const shownButtons = await page.$$eval('#page-mods button', (b) => b.filter((x) => x.offsetParent !== null).map((x) => x.textContent.trim()));
  assert.ok(!shownButtons.some((t) => /^(Install|Update|Use|Remove)$/.test(t)), JSON.stringify(shownButtons));
  assert.ok(!(await page.evaluate(() => window.__calls.some(([c]) => /^mod_install|^addons_install|^looks_apply/.test(c)))));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the sign-in dialog fits a phone, with no window buttons', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineGate.show());
  await page.waitForSelector('#gate:not([hidden])');
  assert.equal(await visible(page, '.gate-winbtns'), false);
  const g = await page.evaluate(() => {
    const W = innerWidth; const out = [];
    for (const e of document.querySelectorAll('#gate *')) {
      const r = e.getBoundingClientRect(); const s = getComputedStyle(e);
      if (s.display === 'none' || r.width === 0) continue;
      if (r.right > W + 1) out.push(e.tagName + '.' + e.className);
    }
    return { out, text: document.getElementById('gate').innerText };
  });
  assert.deepEqual(g.out, []);
  assert.doesNotMatch(g.text, BANNED);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a computer (or an older core with no platform command) keeps the desktop launcher', async () => {
  for (const os of ['windows', null]) {
    const ctx = await browser.newContext({ viewport: { width: 1240, height: 760 } });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.route(/^https?:\/\//, (r) => r.abort());
    await page.addInitScript(FAKE, { os: os || 'windows', gpu: true });
    if (!os) await page.addInitScript(() => { const inv = window.__TAURI__.core.invoke; window.__TAURI__.core.invoke = (c, a) => (c === 'platform' ? Promise.reject('Command platform not found') : inv(c, a)); });
    await page.goto(INDEX);
    await page.waitForFunction(() => window.LeylineNav && document.querySelector('.page.on') && window.LeylinePlatform);
    await page.evaluate(() => window.LeylinePlatform.ready);
    assert.equal(await page.evaluate(() => document.documentElement.classList.contains('phone')), false, String(os));
    assert.equal(await page.evaluate(() => window.LeylinePhone.on), false);
    assert.equal(await visible(page, '#phone-tabs'), false);
    assert.ok(await visible(page, '.side'));
    assert.ok(await visible(page, '.playbar'));
    assert.deepEqual(await page.$$eval('.page.on', (s) => s.map((x) => x.id)), ['page-play']);
    assert.deepEqual(errors, [], String(os));
    await ctx.close();
  }
});
