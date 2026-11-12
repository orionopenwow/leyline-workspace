// A brand-new player's first run, start to end, in Chromium with a fake Tauri core that starts empty (no settings, no
// game folders, not signed in) and keeps what the player does: the first-run tour (guide.js), Find your game files and
// a game folder for each version (Wrath and Vanilla), Home, Realms (Official and Community), picking the official
// realm, PLAY, Your Den from the realm picker, Mods & Addons (Discover, install an addon), Host, Forum, News,
// Settings, Agents, Help, then Log In and Manage account. Every step: no page errors, the key things on screen, no
// game or publisher names, no old "Open Vanilla" name, no version number in a sentence, and a screenshot.
//
// The sign-in is one helper (signIn) so the coming sign-in gate (people log in first) changes one place.
// The real PC's end-to-end run (the real exe, the real game) is docs/FIRST-RUN-TEST.md.
//
//   node --test launcher/tests/firstrun.browser.test.cjs      FIRSTRUN_SHOTS=<dir> also writes NN-name.png there
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.FIRSTRUN_SHOTS || '';
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;
const OLD_NAME = /open ?vanilla|openvanilla\.app/i;
// a version number inside a sentence ("your 3.3.5a folder", "Leyline 0.3 adds"); a chip holding only "3.3.5a" is fine
const VERSION_PROSE = /[A-Za-z,][ \t]+v?\d+\.\d+(\.\d+)?[a-z]?[ \t]+[a-z]/;

// ------------------------------------------------------------------------------------------- the fake core (empty)
const FAKE = () => {
  const calls = (window.__calls = []);
  window.__unknown = [];
  try { localStorage.clear(); } catch (_) { /* a fresh profile: nothing kept */ }
  const OFFICIAL = 'leylinelauncher.com';
  // a fresh core: Settings::default() (settings.rs), seen: [] so the tour shows
  const S = window.__S = { clients: [], selected_client: null, selected_realm: OFFICIAL, wine: { enabled: false, command: 'wine', prefix: '' },
    count_installs: true, hybrid_login: true, seen: [] };
  let signed = false;
  const me = { pid: 'aaaaaaaaaa', name: 'Brightfeather42', tag: '1234', main: '', offline: false,
    profile: { bio: '', avatar: '', avatar_url: '', setup: true, name_generated: true } };
  const installed = {};   // client id -> mods
  const addons = {};      // client id -> installed version or null
  const pack = { format: 1, name: 'Leyline addons', version: '2026.1102.900', build: '3.3.5a', download: 'x', sha256: 'a'.repeat(64),
    addons: [{ name: 'OVQoL', title: 'Quality of life', version: '1.0.0' }], files: {} };
  const P = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: name + ' keeps your bags tidy.', category: 'addons',
    builds: ['3.3.5a'], author: 'Bramble', author_url: '', licence: 'MIT', repo: '', page: '', credits: `${name} by Bramble · MIT`,
    stars: null, last_push: '', version: '1.0.0', kind: 'ui', permissions: [], on_wrath: false,
    download: `https://leylinelauncher.com/wrath/mods/${id.slice(4)}/download`, sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
  const catalog = { categories: [{ id: 'addons', name: 'Addons and UI', line: '', icon: '' }, { id: 'sandbox', name: 'Sandbox and game modes', line: '', icon: '' }],
    projects: [P('mod:tidy-bags', 'Tidy Bags'), P('mod:quest-helper', 'Quest Helper'), P('mod:golfcart', 'Golf Cart', { category: 'sandbox', kind: 'both' })] };
  const realms = [
    { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 41, bots: 0, address: OFFICIAL },
    { slug: 'c-v', realm_id: 'v', name: 'Vanilla Remastered', state: 'live', build: '1.12.1', version: '1.12.1', core: 'vmangos', tags: [], players: 6, bots: 0, address: 'relay.leylinelauncher.com:40050' },
    { slug: 'c-a', realm_id: 'a', name: 'Frost Haven', state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: ['pvp'], players: 12, bots: 0, address: 'relay.leylinelauncher.com:40002' },
    { slug: 'c-b', realm_id: 'b', name: 'Quiet Meadow', state: 'offline', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 0, bots: 0, address: 'relay.leylinelauncher.com:40004' },
  ];
  const now = Math.floor(Date.now() / 1000);
  const T = (id, cat, title) => ({ id, cat, title, author: 'Bramble', tag: '4655', created: now - 7200, bumped: now - 3600, replies: 2, pinned: false,
    locked: false, url: 'https://leylinelauncher.com/wrath/leyline/forum/t/' + id });
  const cats = [{ id: 'realms', name: 'Share your realm', line: 'Realms you host and invite links.', staff_only: false, threads: 1, posts: 3, last: T(1, 'realms', 'Frost Haven') },
    { id: 'help', name: 'Help', line: 'Ask anything.', staff_only: false, threads: 1, posts: 2, last: T(2, 'help', 'Where do I find my game folder?') }];
  const exe = 'C:\\Users\\pat\\AppData\\Local\\Leyline\\Leyline.exe';
  const agents = { allow: false, pending: [], audit: [], exe, command: `claude mcp add --scope user leyline -- "${exe}" mcp`,
    json: JSON.stringify({ mcpServers: { leyline: { command: exe, args: ['mcp'] } } }, null, 2),
    claude_code: { state: 'no', path: 'C:\\Users\\pat\\.claude.json' }, desktop: { state: 'missing', path: 'C:\\Users\\pat\\AppData\\Roaming\\Claude\\claude_desktop_config.json' },
    never: ['Sign in, see or change a password, a Hero Key or your account', 'Anything with money'], now };
  const addonsStatus = () => ({ pack, from_cache: false, error: null, prefs: { auto_update: true, dont_ask: false }, clients: S.clients.map((c) => ({
    client: c.id, name: c.name, build: c.build, supported: c.build === '3.3.5a', installed_at: 1, installed: addons[c.id] || null, update: false, foreign: [], backups: [], missing: [] })) });
  const copy = (x) => JSON.parse(JSON.stringify(x));
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: '0.6.0', settings: copy(S), official_realm: OFFICIAL, browser_url: 'https://leylinelauncher.com/wrath', windows: true,
        data_dir: 'C:\\Users\\pat\\AppData\\Roaming\\Leyline' };
      case 'guide_seen': if (!S.seen.includes(args.key)) S.seen.push(args.key); return null;
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'launcher_update_check': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'testing_get': return { client_torrents: false, share: false };
      // the game folders: a folder picked for each version
      case 'pick_folder': return window.__nextFolder || null;
      case 'add_client': {
        const vanilla = /vanilla/i.test(args.path);
        const build = vanilla ? '1.12.1' : '3.3.5a';
        const p = { id: (vanilla ? 'c1121-' : 'c335a-') + (S.clients.length + 1), name: vanilla ? 'My Vanilla' : 'My Wrath', path: args.path, build, realmlist: '', locale: 'enUS' };
        S.clients.push(p); if (!S.selected_client) S.selected_client = p.id; installed[p.id] = [];
        return { profile: p, detection: { build, build_label: vanilla ? '1.12.1 (5875)' : '3.3.5a (12340)', detected_by: 'exe_version', complete: true, missing: [] } };
      }
      case 'check_client': return { complete: true, missing: [], exe_version: '3.3.5.12340' };
      case 'select': if (args.client) S.selected_client = args.client; if (args.realm) S.selected_realm = args.realm; return null;
      case 'rename_client': return null;
      case 'client_seed_state': return { active: false, name: '', nodes: 0, uploaded: 0, up_speed: 0 };
      case 'client_import_state': return { phase: 'idle', done: 0, total: 0, message: '' };
      // realms
      case 'realms_list': return { realms: copy(realms), network: { realms: 3, players: 59, bots: 0 }, from_cache: false, saved_at: now, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 34, players: 41 };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      // PLAY on the official realm, signed out (app.rs official_note) or signed in
      case 'play': case 'play_realm': return 'Started My Wrath on Wrath Unleashed.' + (signed ? ' It signs you in by itself and opens at character select.'
        : ' Log in to Leyline and PLAY signs you in by itself.');
      case 'open_browser_play': return null;
      // the Leyline addons
      case 'addons_status': return addonsStatus();
      case 'addons_should_offer': return !addons[args.client];
      case 'addons_set_prefs': return { auto_update: true, dont_ask: !!args.dontAsk };
      case 'addons_auto_update': return [];
      case 'addons_install': addons[args.client] = pack.version; return { version: pack.version, addons: ['OVQoL'], moved_aside: [], kept: [], already: false };
      // mods
      case 'mods_catalog': return Object.assign({ from_cache: false, saved_at: now, error: null }, copy(catalog));
      case 'mods_ranked': return { ids: catalog.projects.map((p) => p.id), metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: copy(installed[args.client] || []) };
      case 'mods_updates': return [];
      case 'mod_install': {
        const p = catalog.projects.find((x) => x.id === args.project);
        installed[args.client] = (installed[args.client] || []).filter((m) => m.key !== p.id).concat([{ key: p.id, name: p.name, version: p.version, source: 'catalog',
          update: null, in_catalog: true, enabled: true, files: [], addons: [p.name], patches: [] }]);
        return { key: p.id, name: p.name, version: p.version, warnings: [], kept: [] };
      }
      case 'set_count_installs': return null;
      case 'listing_mine': return { ok: true, listings: [] };
      case 'looks_state': return { looks: [], on: [], changed: false };
      // Your Den (Den Lite ready, nothing hosted yet)
      case 'den_state': return { lite: { available: true, can_get: false, running: !!window.__denUp, address: '127.0.0.1', account: 'DEN', den_dir: 'C:\\den' },
        kit: false, docker_den: false, other_realm: null, account: '', saves: 'lite', dens: [], packs: [] };
      // app.rs SoloPlay: the hybrid glue signs in by itself (denclick.rs is the real PC's check of this)
      case 'den_play_lite': window.__denUp = true; return { message: 'Started My Wrath on 127.0.0.1.', auto: true, why: '', account: 'DEN', password: '' };
      case 'den_net_state': return { running: false, connected: false, relay: '', realm_id: '', address: '', auth_port: 0, world_port: 0, streams: 0, last_error: '' };
      case 'forge_state': return { installed: false, running: false, exe: '', sha256: '', last: null, source: '', licence: '' };
      // Host
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'hostpack_info': return { available: { size: 4096 }, installed: null };
      case 'docker_info': return { state: 'missing', version: null, can_start: false, help: 'Docker Desktop is not installed.', get_url: 'https://docs.docker.com/desktop/', cli: null, desktop: null };
      case 'host_modes': return [];
      case 'host_profiles': return [];
      case 'host_progress': return { idle: true };
      case 'host_scan': return null;
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      case 'host_invite': throw 'No realm yet.';
      // Forum, News
      case 'forum_call': {
        const a = args.action;
        if (a === 'categories') return { format: 1, categories: cats };
        if (a === 'latest') return { threads: [T(1, 'realms', 'Frost Haven'), T(2, 'help', 'Where do I find my game folder?')] };
        if (a === 'threads') return { cat: args.args.cat, threads: [T(1, 'realms', 'Frost Haven')], page: 1, more: false };
        return { threads: [], categories: [] };
      }
      case 'forum_open': case 'open_link': case 'open_url': return null;
      case 'news_list': return { posts: [{ tag: 'LAUNCHER', date: '2026-11-02', title: 'YOUR DEN IN ONE CLICK', items: ['Your Den opens straight into the world.'] }], from_cache: false };
      // Settings
      case 'gfx_get': return { values: { farclip: '777' }, backup: false, presets: [] };
      // Agents
      case 'agents_state': return copy(agents);
      case 'agents_set_allow': agents.allow = args.on; return null;
      // the account
      case 'social_status': return signed;
      case 'social_ways': return { herokey: true, google: true, discord: false };
      case 'social_key_signin': if (!/^ovk1\.TestTestAa\./.test(args.key)) throw "That Hero Key didn't work."; signed = true; return { ok: true, me: { pid: me.pid, name: me.name, tag: me.tag } };
      case 'social_signout': signed = false; return null;
      case 'social_call':
        if (!signed) throw 'signed_out';
        if (args.action === 'me') return { me: copy(me), avatars: [], ways: { herokey: true, google: true, discord: false }, manage_url: 'https://leylinelauncher.com/wrath/profile' };
        if (args.action === 'state') return { v: 1, me: { pid: me.pid, name: me.name, tag: me.tag }, friends: [], incoming: [], outgoing: [], blocked: [] };
        if (args.action === 'herokey_status') return { web_only: false, has_key: true, key: '', saved: true, made: '2 Nov 2026', last_used: '2 Nov 2026, Leyline', has_password: false };
        if (args.action === 'profile') { Object.assign(me.profile, args.args); return { ok: true, me: copy(me) }; }
        return {};
      default: window.__unknown.push(cmd); throw 'unknown ' + cmd;
    }
  } } };
};

// ------------------------------------------------------------------------------------------- helpers
let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) { fs.rmSync(SHOTS, { recursive: true, force: true }); fs.mkdirSync(SHOTS, { recursive: true }); } });
test.after(async () => { await browser.close(); });

let n = 0;
const SMALL = { width: 1100, height: 760 };
const LARGE = { width: 1400, height: 860 };
const pageOn = (page) => page.evaluate(() => document.querySelector('.page.on').id);
// a page in a side menu fold (Play, Launcher): its fold opened first, as a player would
const go = async (page, name) => {
  await page.evaluate((n) => { const f = document.querySelector(`.side .nav[data-page="${n}"]`).closest('.nav-fold'); if (f) window.LeylineNav.fold(f.dataset.fold, true); }, name);
  await page.click(`.side .nav[data-page="${name}"]`); await page.waitForFunction((n) => document.querySelector('#page-' + n + '.on'), name); };
const text = (page, sel) => page.evaluate((s) => [...document.querySelectorAll(s)].filter((e) => e.offsetParent || e === document.body).map((e) => e.innerText).join('\n'), sel);

// The words a player sees on this step: the open page, the title bar, the play bar and anything open on top.
async function seen(page) {
  return page.evaluate(() => {
    const parts = [];
    for (const s of ['.page.on', 'header', '#play-form', '#gd-tour', '#gd-toast', '[role="dialog"]:not([hidden])', '.pop:not([hidden])', '#hf-panel', '#hf-code']) {
      for (const e of document.querySelectorAll(s)) if (e.offsetParent || getComputedStyle(e).position === 'fixed') parts.push(e.innerText);
    }
    return parts.join('\n');
  });
}

// Text cut off: an element with text whose content is wider than its box (overflow hidden, no ellipsis meant).
async function clipped(page) {
  return page.evaluate(() => {
    const out = [];
    for (const e of document.querySelectorAll('.page.on *, header *, #play-form *')) {
      if (!e.offsetParent || !e.childNodes.length || ![...e.childNodes].some((c) => c.nodeType === 3 && c.textContent.trim())) continue;
      const cs = getComputedStyle(e);
      if (cs.textOverflow === 'ellipsis' || e.closest('.sr, .sr-only, .visually-hidden')) continue;   // an ellipsis is meant; screen-reader text is 1px on purpose
      if ((cs.overflowX === 'hidden' || cs.overflow === 'hidden') && e.scrollWidth > e.clientWidth + 2) out.push((e.id ? '#' + e.id : e.className || e.tagName) + ': ' + e.textContent.trim().slice(0, 50));
    }
    return out;
  });
}

// One step: the checks every step has, then the screenshot. Findings (soft) are collected, not failed on.
const findings = [];
async function step(page, errors, name, size, check) {
  n += 1;
  const nn = String(n).padStart(2, '0');
  if (size) await page.setViewportSize(size);
  if (check) await check();
  await page.waitForTimeout(450);   // the page fade and the menus' open animation
  const words = await seen(page);
  assert.doesNotMatch(words, BRANDS, `${nn}-${name}: no game or publisher names`);
  const old = words.match(OLD_NAME); if (old) findings.push(`${nn}-${name}: the old name shows: "${old[0]}"`);
  const ver = words.match(VERSION_PROSE); if (ver) findings.push(`${nn}-${name}: a version in a sentence: "${ver[0]}"`);
  for (const c of await clipped(page)) findings.push(`${nn}-${name}: cut off: ${c}`);
  const wide = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
  if (wide) findings.push(`${nn}-${name}: the window scrolls sideways`);
  assert.deepEqual(errors, [], `${nn}-${name}: no page errors`);
  if (SHOTS) await page.screenshot({ path: path.join(SHOTS, `${nn}-${name}.png`) });
}

// THE SIGN-IN, in one place: the title bar's Log In opens the sign-in dialog (gate.js); a test Hero Key, pasted there.
const TEST_KEY = 'ovk1.TestTestAa.' + 'k'.repeat(43);
async function signIn(page) {
  if (!(await page.evaluate(() => window.LeylineGate.isOpen()))) await page.click('#hf-signin');
  await page.click('#gate-keylink');
  await page.waitForSelector('#gate-key');
  await page.fill('#gate-key', TEST_KEY);
  await page.click('#gate-keyform button[type="submit"]');
  await page.click('#gate-go');   // the signed-in moment: picture, name#tag, Continue
  await page.waitForSelector('#hf-me-btn');
}

// ------------------------------------------------------------------------------------------- the walk
test('a brand-new player, start to end: tour, game folders, realms, PLAY, Your Den, mods, every page, Log In', async () => {
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: SMALL });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => { errors.push(String(e)); if (process.env.FIRSTRUN_DEBUG) console.log(e.stack); });
  page.on('dialog', (d) => { findings.push('a browser dialog: ' + d.message()); d.dismiss(); });
  await page.route('https://**', (r) => (r.request().resourceType() === 'image' ? r.fulfill({ path: PICTURE, contentType: 'image/webp' }) : r.fulfill({ status: 404, body: '' })));
  await page.addInitScript(FAKE);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineApp && window.LeylineApp.state());

  // 0. the launcher at once, signed out (owner 2 Nov 23:14): Log In top right invites; it opens the sign-in dialog
  await step(page, errors, 'sign-in', SMALL, async () => {
    await page.waitForSelector('#hf-signin');
    assert.equal(await page.evaluate(() => document.querySelector('.frame').inert), false, 'the launcher works signed out');
    await page.click('#hf-signin');
    await page.waitForSelector('#gate-browser');
  });
  await signIn(page);

  // 1. first launch: no settings, no game folders; Home, with the tour on top
  await step(page, errors, 'first-launch', SMALL, async () => {
    await page.waitForSelector('#gd-tour');
    assert.equal(await pageOn(page), 'page-play');
    assert.equal(await page.textContent('#pb-client-name'), 'No game folder');
  });

  // 2. the tour: four steps (Find your game files is the last, with no game folder), then Done
  const titles = [];
  for (let i = 0; i < 4; i++) {
    titles.push(await page.textContent('#gd-tour-title'));
    await step(page, errors, `tour-${i + 1}`, i === 0 ? LARGE : SMALL, async () => {
      await page.waitForTimeout(280);
      assert.equal(await page.textContent('#gd-tour-n'), `${i + 1} of 4`);
    });
    await page.click('#gd-tour-next');
  }
  assert.equal(titles[3], 'Find your game files');
  assert.equal(await page.locator('#gd-tour').count(), 0, 'Done closes the tour');
  await page.setViewportSize(SMALL);

  // 3. Find your game files, from the client picker; then a game folder for each version
  await step(page, errors, 'client-picker-empty', SMALL, async () => {
    await page.click('#pb-client');
    assert.ok(await page.isVisible('#pb-menu'));
  });
  await page.click('#pb-menu >> text=Find your game files');
  await step(page, errors, 'find-your-game-files', SMALL, async () => {
    assert.equal(await pageOn(page), 'page-findfiles');
    assert.match(await text(page, '#page-findfiles'), /can't download the game for you/);
  });
  await page.evaluate(() => { window.__nextFolder = 'C:\\Games\\Wrath'; });
  await page.click('#ff-pick');
  await page.waitForFunction(() => /Added My Wrath/.test(document.getElementById('cl-status').textContent));
  await step(page, errors, 'added-wrath-addons-offer', SMALL, async () => {
    assert.equal(await pageOn(page), 'page-clients');
    await page.waitForSelector('#ad-dlg[open]');
  });
  await page.click('#ad-dlg button:has-text("Not now")');   // the offer comes back at PLAY (step 7)
  await page.waitForSelector('#ad-dlg[open]', { state: 'detached', timeout: 2000 }).catch(() => {});
  await step(page, errors, 'added-wrath', SMALL);
  await page.evaluate(() => { window.__nextFolder = 'C:\\Games\\Vanilla'; });
  await page.click('#cl-pick');
  await page.waitForFunction(() => /Added My Vanilla/.test(document.getElementById('cl-status').textContent));
  await step(page, errors, 'added-vanilla', LARGE, async () => {
    assert.equal(await page.locator('#cl-list .client, #cl-list > *').count() >= 2, true);
  });

  // 4. Home
  await go(page, 'play');
  await step(page, errors, 'home', SMALL, async () => {
    await page.waitForFunction(() => document.getElementById('pb-realm').textContent === 'Wrath Unleashed');
    assert.equal(await page.textContent('#pb-client-name'), 'My Wrath');
  });

  // 5. Realms: Official, then Community
  await go(page, 'realms');
  await step(page, errors, 'realms-official', SMALL, async () => {
    await page.waitForFunction(() => /Wrath Unleashed/.test(document.getElementById('rl-pane').textContent));
  });
  await page.click('#rl-tab-community');
  await step(page, errors, 'realms-community', SMALL, async () => {
    await page.waitForFunction(() => /Frost Haven/.test(document.getElementById('rl-pane').textContent));
  });
  await page.click('#rl-tab-official');

  // 6. the official realm, picked from the play bar's realm picker
  await go(page, 'play');
  await page.click('#pb-realm-btn');
  await step(page, errors, 'realm-picker', SMALL, async () => {
    assert.ok(await page.isVisible('#pb-rmenu'));
    assert.match(await page.textContent('#pb-rmenu'), /Wrath Unleashed[\s\S]*Your Den/);
  });
  await page.click('#pb-rmenu >> text=Wrath Unleashed');
  assert.equal(await page.textContent('#pb-realm'), 'Wrath Unleashed');

  // 7. PLAY: the Leyline addons offer first (a new Wrath folder), then the game starts
  await page.click('#play-go');
  await page.waitForTimeout(300);
  await step(page, errors, 'play-offer', SMALL);
  const offer = page.locator('button', { hasText: /^Install and play$/ });
  if (await offer.count()) await offer.first().click();
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play' || c === 'play_realm'), null, { timeout: 5000 });
  await page.waitForTimeout(300);
  await step(page, errors, 'play-started', SMALL, async () => {
    assert.match(await page.textContent('#play-status'), /Started/);
  });

  // 8. Your Den, from the realm picker
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu >> text=Your Den');
  await page.waitForTimeout(400);
  await step(page, errors, 'your-den', SMALL, async () => {
    assert.equal(await pageOn(page), 'page-den');
  });

  // 9. Mods & Addons: Discover, install an addon into My Wrath, then Installed
  await go(page, 'mods');
  await page.click('#tab-mods-discover');
  await page.waitForFunction(() => /Golf Cart/.test(document.getElementById('pane-discover').textContent));
  await step(page, errors, 'mods-discover', LARGE);
  await page.click('#discover-tabs .ly-tab[data-tab="addons"]');
  await page.waitForFunction(() => /Tidy Bags/.test(document.getElementById('pane-discover').textContent));
  await step(page, errors, 'addons-discover', SMALL);
  const card = page.locator('#pane-discover', { hasText: 'Tidy Bags' }).locator('button', { hasText: /^install$/i }).first();
  await card.click();
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'mod_install'), null, { timeout: 5000 });
  await page.waitForTimeout(300);
  await step(page, errors, 'mods-installed-one', SMALL);
  await page.click('#tab-mods-installed');
  await page.waitForTimeout(300);
  await step(page, errors, 'mods-installed-tab', SMALL);
  await page.click('#inst-tabs .ly-tab[data-shelf="addons"]');
  await step(page, errors, 'mods-installed-addons', SMALL, async () => {
    await page.waitForFunction(() => /Tidy Bags/.test(document.getElementById('mods-list').textContent));
  });

  // 10–15. every other page
  for (const [name, check] of [
    ['host', async () => { assert.ok((await text(page, '#page-host')).length > 40); }],
    ['forum', async () => { await page.waitForFunction(() => /Frost Haven/.test(document.getElementById('page-forum').textContent)); }],
    ['news', async () => { await page.waitForFunction(() => /YOUR DEN IN ONE CLICK/.test(document.getElementById('page-news').textContent)); }],
    ['settings', async () => { assert.match(await text(page, '#page-settings'), /Leyline/); }],
    ['agents', async () => { assert.ok((await text(page, '#page-agents')).length > 40); }],
    ['help', async () => { assert.ok(await page.isVisible('#guide-again')); }],
  ]) {
    await go(page, name);
    await page.waitForTimeout(250);
    await step(page, errors, name, name === 'host' ? LARGE : SMALL, check);
  }

  // 16. signed in since the start: the chip, then Manage account
  await go(page, 'play');
  await step(page, errors, 'signed-in', SMALL, async () => { assert.ok(await page.isVisible('#hf-me-btn')); });
  await page.click('#hf-me-btn');
  await step(page, errors, 'account-panel', SMALL);
  await page.click('#hf-panel [role="menuitem"]:has-text("Manage account")');
  await page.waitForSelector('#page-account.on');
  await page.waitForTimeout(300);
  await step(page, errors, 'manage-account', LARGE);
  // PLAY once more, signed in: it says it signs in by itself
  await go(page, 'play');
  await page.click('#play-go');
  await page.waitForFunction(() => /signs you in by itself/.test(document.getElementById('play-status').textContent));
  await step(page, errors, 'play-signed-in', SMALL);

  if (SHOTS) fs.writeFileSync(path.join(SHOTS, 'findings.txt'), [...new Set(findings)].join('\n') + '\nunknown core calls: ' + JSON.stringify([...new Set(await page.evaluate(() => window.__unknown))]) + '\n');
  if (findings.length) console.log('firstrun findings:\n  ' + [...new Set(findings)].join('\n  '));
  assert.deepEqual(errors, []);
  await ctx.close();
});
