// The launcher's side menu, page tabs and backdrop (ui/index.html, ui/app.js, ui/style.css) in Chromium with a fake
// Tauri core: the menu matches the website's (Home, Realms, Mods, Host, Forum, News, Join the Discord), then the folds (Play:
// Single player, Modes; Launcher: Simple mode, Settings, Agents, Help) open and close, are remembered, open for their page
// and count their badges when closed; Mods has the tabs Installed and Discover, the Forum replaced Community (Friends and Hub) and the old page ids
// (mymods, discover, friends, community, hub, mods) still land on the right page and tab, the arrow keys move between tabs, the
// Your realms rows carry the realm's icon and client version, and the website's backdrop sits behind the content area
// only, with the plain colour when it fails to load or Settings › Background picture is off.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/nav.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const BACKDROP = 'https://play.openvanilla.app/wrath/art/leyline-shot-backdrop.webp';
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');   // a stand-in: the test never reaches the site

const FAKE = ({ page, backdrop, tabs }) => {
  const calls = (window.__calls = []);
  try {
    if (page) localStorage.setItem('leyline.page', page); else localStorage.removeItem('leyline.page');
    localStorage.setItem('leyline.dcView', 'all'); localStorage.setItem('leyline.dcMore', '1');   // the full catalog, More filters open (simple.js)
    if (backdrop) localStorage.setItem('leyline.backdrop', backdrop); else localStorage.removeItem('leyline.backdrop');
    if (tabs) localStorage.setItem('leyline.tabs', JSON.stringify(tabs)); else localStorage.removeItem('leyline.tabs');
  } catch (_) { /* fine */ }
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c335a-1', name: 'My Wrath', path: '/x', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c335a-1', selected_realm: 'play.openvanilla.app', wine: { enabled: false, command: 'wine', prefix: '' } },
        official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'realms_list': return { realms: [{ slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a',
        version: '3.3.5a', core: 'azerothcore', tags: [], players: 5, bots: 0, address: 'play.openvanilla.app' }],
        network: { realms: 0, players: 5, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c335a-1', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return true;   // a sign-in kept (the website not asked: the sign-in screen stays closed)
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts, picture) {
  const ctx = await browser.newContext({ colorScheme: 'dark' });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route(BACKDROP, (r) => (picture === 'fail' ? r.abort() : r.fulfill({ path: PICTURE, contentType: 'image/webp' })));
  await page.addInitScript(FAKE, opts || {});
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && document.querySelector('.page.on'));
  return { page, ctx, errors };
}
const where = (page) => page.evaluate(() => ({
  nav: [...document.querySelectorAll('.nav.on[data-page]')].map((b) => b.dataset.page),
  page: [...document.querySelectorAll('.page.on')].map((p) => p.id),
  panes: [...document.querySelectorAll('.page.on .tabpane.on')].map((p) => p.id),
}));

test('the side menu matches the website, with the same line icons', async () => {
  const { page, ctx, errors } = await open();
  // Simple mode's Home item (simplehome.js) is only shown while Simple mode is on
  const items = await page.$$eval('aside nav .nav[data-page]', (b) => b.filter((x) => x.offsetParent !== null).map((x) => [x.dataset.page, x.textContent.trim().replace(/\d+$/, '')]));
  // owner 12 Nov: "Home, Realm Mods, Host, Forum, News Discord", the rest in folds; no Your Den item (owner 2026-11-02:
  // it is in the play bar's realm list next to PLAY); Single player first in the Play fold, open at first (the flagship)
  assert.deepEqual(items, [['play', 'Home'], ['realms', 'Realms'], ['mods', 'Mods'], ['host', 'Host'], ['forum', 'Forum'], ['news', 'News'],
    ['single', 'Single player'], ['modes', 'Modes']]);
  assert.equal(await page.getAttribute('.nav[data-page="mods"]', 'title'), 'Mods & Addons', 'the page keeps its full name');
  assert.equal(await page.locator('aside nav .nav[data-page]:visible svg').count(), 8, 'each with its line icon');
  // owner 31 Oct 21:59: Join the Discord under News (a link, not a page), then the Play fold
  assert.equal(await page.$eval('#tb-discord', (b) => b.previousElementSibling.dataset.page + '|' + b.dataset.link + '|' + b.textContent.trim() + '|' + b.nextElementSibling.id), 'news|discord|Join the Discord|fold-play');
  assert.deepEqual(await page.$$eval('.side .nav-fold', (f) => f.map((x) => [x.dataset.fold, x.querySelector('.nf-name').textContent, x.querySelector('.nav-fold-head').getAttribute('aria-expanded')])),
    [['play', 'Play', 'true'], ['launcher', 'Launcher', 'false']]);
  const foot = await page.$$eval('.side-foot .nav-fold .nav', (b) => b.map((x) => x.dataset.page || x.id));
  assert.deepEqual(foot, ['nav-simple-toggle', 'settings', 'agents', 'help'], 'the Launcher fold at the foot');
  assert.equal(await page.locator('.side-foot .nav:visible').count(), 0, 'closed at first');
  assert.equal(await page.textContent('.side-label'), 'YOUR REALMS');
  assert.equal(await page.locator('[data-link="support"]:visible').count(), 0, 'support buttons stay hidden');
  // Your realms: the crystal for the official realm and the gold client version
  await page.waitForFunction(() => document.querySelector('#realm-rows .realm-row .name').textContent === 'Wrath Unleashed');
  assert.equal(await page.getAttribute('#realm-rows .realm-row .rl-ic img', 'src'), 'art/brand/leyline-crystal-256.webp');
  assert.equal(await page.textContent('#realm-rows .realm-row .rl-ver'), '3.3.5a');
  assert.equal(await page.textContent('#pb-realm-ver'), '3.3.5a', 'the play bar too');
  assert.equal(await page.locator('#page-play #srv-name, #page-play #client-now').count(), 0, 'no SERVER or GAME CLIENT card on Home');
  const text = await page.evaluate(() => document.body.innerText);
  assert.doesNotMatch(text, /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Mods & Addons: Installed and Discover tabs, each with Mods | Addons; the Forum (no tabs, no Friends); the arrow keys', async () => {
  const { page, ctx, errors } = await open();
  await page.click('.nav[data-page="mods"]');
  assert.deepEqual(await where(page), { nav: ['mods'], page: ['page-mods'], panes: ['pane-discover'] }, 'Mods opens on Discover');
  assert.deepEqual(await page.$$eval('.page-tab[data-group="mods"]', (b) => b.map((x) => x.dataset.tab)), ['discover', 'installed', 'gamemodes', 'dens'], 'Discover is the first tab; Dens last (shared Dens, denmarket.js)');
  assert.equal(await page.textContent('#page-mods h2'), 'Mods & Addons');
  await page.click('#tab-mods-installed');
  assert.deepEqual(await page.$$eval('#inst-tabs .ly-tab', (b) => b.map((x) => x.textContent.replace(/\d+$/, ''))), ['Mods', 'Addons'], 'Installed is split too');
  await page.click('#inst-tabs .ly-tab[data-shelf="addons"]');
  assert.equal(await page.getAttribute('#pane-installed', 'data-shelf'), 'addons');
  assert.equal(await page.textContent('#mods-find'), 'Find addons');
  await page.click('#tab-mods-discover');
  assert.deepEqual((await where(page)).panes, ['pane-discover']);
  assert.ok(await page.evaluate(() => window.__calls.some(([c]) => c === 'mods_catalog')), 'Discover loads the Workshop');
  // the Discover features are all there: the Mods | Addons | Themes tabs, the Sort menu
  assert.deepEqual(await page.$$eval('#discover-tabs .ly-tab', (b) => b.map((x) => x.textContent)), ['Mods', 'Addons', 'Looks', 'Themes', 'Graphics']);
  assert.deepEqual(await page.$$eval('#cat-sort option', (o) => o.map((x) => x.textContent)), ['Popular', 'Rising', 'Top rated', 'Most installed', 'This month', 'New', 'Recently updated', 'Name']);
  await page.click('#discover-tabs [data-tab="themes"]');
  assert.ok(await page.isVisible('#themes-discover'));
  await page.click('#discover-tabs [data-tab="mods"]');
  // the arrow keys move between the page's tabs
  await page.focus('#tab-mods-discover');
  await page.keyboard.press('ArrowRight');
  assert.deepEqual((await where(page)).panes, ['pane-installed']);
  assert.equal(await page.evaluate(() => document.activeElement.id), 'tab-mods-installed');
  await page.keyboard.press('ArrowRight');
  assert.deepEqual((await where(page)).panes, ['pane-gamemodes'], 'then Game modes (owner 3 Nov)');
  assert.equal(await page.evaluate(() => document.activeElement.id), 'tab-mods-gamemodes');
  await page.keyboard.press('ArrowRight');
  assert.deepEqual((await where(page)).panes, ['pane-dens'], 'then Dens (shared Dens)');
  await page.click('#tab-mods-installed');
  // FIND ADDONS on Installed › Addons goes to Discover › Addons
  await page.click('#pane-installed [data-page-go="discover"]');
  assert.deepEqual((await where(page)).panes, ['pane-discover']);
  assert.equal(await page.getAttribute('#pane-discover', 'data-tab'), 'addons');
  // a page remembers its last tab
  await page.click('.nav[data-page="forum"]');
  assert.deepEqual(await where(page), { nav: ['forum'], page: ['page-forum'], panes: [] });
  assert.ok(await page.isVisible('#fm-discord'), 'the Forum page has Join the Discord');
  assert.ok(await page.evaluate(() => window.__calls.some(([c, a]) => c === 'forum_call' && a.action === 'categories')), 'opening it reads the forum');
  assert.equal(await page.locator('#page-community, #pane-friends, #nav-friends-count, .nav[data-page="community"]').count(), 0, 'no Community page, Friends tab or friends badge');
  await page.click('.nav[data-page="mods"]');
  assert.deepEqual((await where(page)).panes, ['pane-discover'], 'Mods opens on Discover again (it was left on Installed)');
  assert.deepEqual(await page.evaluate(() => [localStorage.getItem('leyline.page'), JSON.parse(localStorage.getItem('leyline.tabs'))]),
    ['mods', { mods: 'discover' }]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the folds: open and close by click and keyboard, slide, remembered; the page shown opens its fold; badges counted on a closed header', async () => {
  const { page, ctx, errors } = await open();
  const state = () => page.evaluate(() => [...document.querySelectorAll('.side .nav-fold')].map((f) => {
    const h = f.querySelector('.nav-fold-head');
    return [f.dataset.fold, h.getAttribute('aria-expanded'), f.classList.contains('open'), [...f.querySelectorAll('.nav')].filter((x) => x.offsetParent !== null && getComputedStyle(x.closest('.nav-fold-body')).visibility === 'visible').length];
  }));
  assert.deepEqual(await state(), [['play', 'true', true, 2], ['launcher', 'false', false, 0]]);
  assert.equal(await page.getAttribute('#fold-play-head', 'aria-controls'), 'fold-play-body');
  assert.equal(await page.getAttribute('#fold-play-body', 'role'), 'group');
  // a click closes Play; its items are out of reach (no Tab stop) and the chevron turns back
  await page.click('#fold-play-head');
  await page.waitForTimeout(300);
  assert.deepEqual((await state())[0], ['play', 'false', false, 0]);
  assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById('fold-play-body')).visibility), 'hidden');
  // the keyboard: Enter and Space on the header
  await page.focus('#fold-launcher-head');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(300);
  assert.deepEqual((await state())[1], ['launcher', 'true', true, 4]);
  await page.keyboard.press('Tab');
  assert.equal(await page.evaluate(() => document.activeElement.id), 'nav-simple-toggle', 'Tab goes into the open fold');
  await page.focus('#fold-launcher-head');
  await page.keyboard.press('Space');
  assert.equal(await page.getAttribute('#fold-launcher-head', 'aria-expanded'), 'false');
  await page.keyboard.press('Space');
  assert.deepEqual(await page.evaluate(() => JSON.parse(localStorage.getItem('leyline.navFolds'))), { play: false, launcher: true }, 'remembered');
  // remembered: a window opened with that kept opens the folds as they were left (kept in the store here: a second
  // document in the same test browser does not always see the first one's latest write at once)
  const p2 = await ctx.newPage();
  await p2.addInitScript(() => { try { localStorage.setItem('leyline.navFolds', '{"play":false,"launcher":true}'); } catch (_) { /* fine */ } });
  await p2.addInitScript(FAKE, {});
  await p2.goto(INDEX);
  await p2.waitForFunction(() => window.LeylineNav && document.querySelector('.page.on'));
  assert.deepEqual(await p2.$$eval('.side .nav-fold-head', (h) => h.map((x) => x.getAttribute('aria-expanded'))), ['false', 'true']);
  await p2.close();
  // a page shown from code or a link opens its fold (not remembered); the fold holding the page shown is marked
  await page.evaluate(() => window.LeylineNav.show('single'));
  assert.equal(await page.getAttribute('#fold-play-head', 'aria-expanded'), 'true');
  assert.ok(await page.isVisible('.nav.on[data-page="single"]'));
  assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('leyline.navFolds')).play), false, 'an automatic open is not a choice');
  await page.click('#fold-play-head');
  assert.ok(await page.evaluate(() => document.getElementById('fold-play').classList.contains('has-on')), 'closed over the page shown: the header is marked');
  await page.click('#fold-launcher-head');   // close Launcher
  await page.evaluate(() => window.LeylineNav.show('settings'));
  assert.equal(await page.getAttribute('#fold-launcher-head', 'aria-expanded'), 'true', 'Settings from a link opens Launcher');
  assert.ok(await page.isVisible('.nav.on[data-page="settings"]'));
  assert.equal(await page.evaluate(() => document.getElementById('fold-play').classList.contains('has-on')), false);
  // a closed fold's header counts the badges inside it (Agents' count)
  await page.click('#fold-launcher-head');
  await page.evaluate(() => { const b = document.getElementById('nav-agents-count'); b.textContent = '3'; b.hidden = false; });
  await page.waitForFunction(() => !document.querySelector('#fold-launcher .nf-count').hidden);
  assert.equal(await page.textContent('#fold-launcher .nf-count'), '3');
  assert.match(await page.getAttribute('#fold-launcher-head', 'aria-label'), /Launcher, 3 new inside/);
  await page.click('#fold-launcher-head');
  assert.equal(await page.isHidden('#fold-launcher .nf-count'), true, 'open: the badge itself shows instead');
  assert.equal(await page.evaluate(() => window.LeylineNav.fold('launcher')), true);
  assert.equal(await page.evaluate(() => window.LeylineNav.fold('launcher', false)), false);
  assert.deepEqual(errors, []);
  await ctx.close();
});

for (const [old, want] of [['mymods', ['page-mods', 'pane-installed']], ['discover', ['page-mods', 'pane-discover']],
  ['friends', ['page-forum', null]], ['hub', ['page-forum', null]], ['community', ['page-forum', null]], ['chat', ['page-forum', null]],
  ['mods', ['page-mods', 'pane-discover']], ['modes', ['page-modes', null]], ['nonsense', ['page-play', null]]]) {
  test(`the old page id "${old}" (remembered, or from a button or link) lands on the right page and tab`, async () => {
    const { page, ctx, errors } = await open({ page: old });
    const w = await where(page);
    assert.deepEqual([w.page[0], w.panes[0] || null], want);
    await page.evaluate(() => window.LeylineNav.show('play'));
    await page.evaluate((o) => window.LeylineNav.show(o), old);
    const v = await where(page);
    assert.deepEqual([v.page[0], v.panes[0] || null], want);
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

test('the backdrop: behind the content area only, under the wash; the picture that ships with the app until it loads and when it fails; plain when off', async () => {
  let { page, ctx, errors } = await open();
  await page.waitForFunction(() => document.getElementById('backdrop').dataset.loaded === '1');
  const r = await page.evaluate(() => {
    const b = document.getElementById('backdrop'), m = document.querySelector('main'), s = document.querySelector('.side');
    const cs = getComputedStyle(b);
    const rect = (e) => { const x = e.getBoundingClientRect(); return [x.left, x.top, x.width, x.height].map(Math.round); };
    return { img: cs.backgroundImage, b: rect(b), m: rect(m), sideBg: getComputedStyle(s).backgroundImage, panel: getComputedStyle(document.querySelector('.home .news')).backdropFilter };
  });
  assert.match(r.img, /linear-gradient\(rgba\(9, 11, 17, 0\.52\).*rgba\(9, 11, 17, 0\.8\) 80%\), url\("https:\/\/play\.openvanilla\.app\/wrath\/art\/leyline-shot-backdrop\.webp"\)/);
  assert.equal(r.b[0], r.m[0], 'starts at the content area\'s left edge, not behind the side bar');
  assert.ok(r.b[1] <= r.m[1] && r.b[1] + r.b[3] === r.m[1] + r.m[3], 'behind the content area, and up behind the see-through title bar');
  assert.ok(r.b[2] >= r.m[2], 'as wide as it, and on Home under the docked profile panel too');
  assert.ok(!/leyline-shot-backdrop/.test(r.sideBg), 'not behind the side bar (the side is stone, owner 31 Oct)');
  assert.match(r.panel, /blur/, 'panels are a blurred layer over it');
  assert.equal(await page.isChecked('#set-backdrop'), true, 'on by default');
  // Settings › Background picture off: the plain colour, remembered
  await page.evaluate(() => window.LeylineNav.fold('launcher', true));   // Settings, Agents and Help are in the Launcher fold
  await page.click('.nav[data-page="settings"]');
  await page.click('#set-backdrop');
  assert.equal(await page.evaluate(() => document.getElementById('backdrop').classList.contains('has-pic')), false);
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.backdrop')), 'off');
  assert.deepEqual(errors, []);
  await ctx.close();

  ({ page, ctx, errors } = await open({ backdrop: 'off' }));
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => document.getElementById('backdrop').classList.contains('has-pic')), false);
  assert.equal(await page.isChecked('#set-backdrop'), false);
  await ctx.close();

  ({ page, ctx, errors } = await open({}, 'fail'));
  await page.waitForFunction(() => document.getElementById('backdrop').dataset.loaded === '0');
  assert.match(await page.evaluate(() => getComputedStyle(document.getElementById('backdrop')).backgroundImage), /art\/leyline-bg\.webp/, 'offline: the picture that ships with the app');
  assert.deepEqual(errors, []);
  await ctx.close();
});
