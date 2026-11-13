// The Single player page's Friends area in the FULL app (index.html with every stylesheet), next to the Realms list and
// Mods Discover, so the friends lists are judged by the same eye as the screens they must match (owner 12 Nov: "the two
// odd blue ones out need to match the art style and asset usage of the others"):
//   * #lf (friends.js), #wf and #fw (worldfriends.js) draw on the Single player page with a fake core;
//   * their rows are the launcher's stone cards, their buttons the launcher's buttons (the primary one the red stone),
//     their titles the Marcellus section titles, in Dark and in Light;
//   * screenshots of each part, and of the Realms list and Mods Discover for comparison.
// Screenshots: LOOK_SHOTS=<dir> (default /mnt/user-data/outputs/gallery when it exists): look-friends-*.png and
// look-compare-*.png.
//
//   node --test launcher/tests/friendslook.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const SHOTS = process.env.LOOK_SHOTS || (fs.existsSync('/mnt/user-data/outputs') ? '/mnt/user-data/outputs/gallery' : '');
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;
const NOW = Math.floor(Date.now() / 1000);

const FAKE = (o) => {
  const now = Math.floor(Date.now() / 1000);
  window.__calls = [];
  try {
    if (!sessionStorage.getItem('__booted')) {
      sessionStorage.setItem('__booted', '1');
      localStorage.clear();
      localStorage.setItem('leyline.mode', 'online');
      localStorage.setItem('leyline.page', 'discover');
      localStorage.setItem('leyline.dcView', 'all');
      localStorage.setItem('leyline.dcTab', 'addons');
    }
  } catch (_) { /* fine */ }
  window.LeylineSolo = window.LeylineSolo || undefined;
  const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
  const st = { version: 'test', settings: { clients: [WRATH], selected_client: WRATH.id, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  const WS = { supported: true, installed: true, set_up: true, running: true, backups: 0, other_running: null, account: '', password: '' };
  const SET = { companions_auto: false, companions_follow: 0, dungeon_auto: true, roaming_bots: 0, xp_rate: 2, chosen: true, set_up: true, running: true };
  const base = { official: false, kind: 'PvE', ruleset: 'main', state: 'live', version: '3.3.5a', core: 'azerothcore',
    build: '3.3.5a', tags: [], region: null, region_estimated: false, language: null, max_players: null, uptime_24h: null,
    uptime_7d: null, is_new: false, score: 0, featured: false, verified: false, direct: false, rules: [], claimed: true,
    players: 0, bots: 0, tagline: '', summary: '', join_url: null, discord: null, relay: 'relay.openvanilla.app', report_url: null };
  const R = (x) => Object.assign({}, base, x);
  const realms = [
    R({ slug: 'official', name: 'Leyline Sample Hangout', official: true, tagline: 'Our own realm.', address: 'play.openvanilla.app', group: 'official', players: 12, bots: 40 }),
    R({ slug: 'c-bbbbbbbbbbbbbbbb', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Busy Den', tags: ['bots'], players: 30, bots: 40, max_players: 100, address: 'relay.openvanilla.app:40002', group: 'community' }),
    R({ slug: 'c-cccccccccccccccc', realm_id: 'cccccccccccccccc', name: 'Asleep', state: 'offline', address: 'relay.openvanilla.app:40004', group: 'offline' }),
  ];
  const P = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: name + ' does things.', category: 'addons',
    builds: ['3.3.5a'], author: 'Someone', author_url: '', licence: 'MIT', repo: '', page: '', credits: `${name} by Someone · MIT`, stars: null,
    last_push: '', version: '1.0.0', kind: 'ui', permissions: [], on_wrath: false, tags: [], download: 'https://leylinelauncher.com/x', sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
  const catalog = { categories: [{ id: 'addons', name: 'Addons and UI', line: '', icon: '' }], projects: [P('mod:alpha', 'Alpha Bars'), P('mod:bravo', 'Bravo Bags'), P('mod:charlie', 'Charlie Frames')], from_cache: false, saved_at: 1, error: null };
  const FRIENDS = {
    me: { peer_id: 'mmmmmmmmmmmmmmmm', profile: { name: 'Ari', status: '', share_playing: true, lan: 'friends', mailbox: true, relay: 'relay.leylinelauncher.com', worlds: [], seq: 1 } },
    friends: o.empty ? [] : [
      { peer_id: 'bbbbbbbbbbbbbbbb', name: 'Bo', status: 'around tonight', online: true, near: false, last_seen: now - 30, playing: 'Single player', open: null, invite: { world: { kind: 'single', name: 'Single player' }, ts: now - 60 }, worlds: [], since: now - 999 },
      { peer_id: 'cccccccccccccccc', name: '', status: '', online: false, near: false, last_seen: now - 3 * 3600, playing: null, open: null, invite: null, worlds: [], since: now - 999 },
    ],
    incoming: o.empty ? [] : [{ peer_id: 'dddddddddddddddd', name: 'Dee', status: '', ts: now - 5, via: 'code' }],
    outgoing: [], near: o.empty ? [] : [{ peer_id: 'ffffffffffffffff', name: 'Fay', asked: false }],
    net: { running: true, lan_port: 41190, tcp_port: 41191, mailbox: 'ok', last_error: '', notices: [] },
  };
  const WF = o.empty ? [] : [
    { user: 'GAAAAAAAAAAAAAAA', peer: 'bbbbbbbbbbbbbbbb', name: 'Bo', on_list: true, online: true, trusted: false, enabled: true, legacy: false, seen: now - 60,
      chars: [{ user: 'GAAAAAAAAAAAAAAA', name: 'Brakka', level: 12, online: true }, { user: 'GAAAAAAAAAAAAAAA', name: 'Odo', level: 3, online: false }] },
    { user: 'GCCCCCCCCCCCCCCC', peer: 'cccccccccccccccc', name: '', on_list: false, online: false, trusted: true, enabled: false, legacy: false, seen: now - 9000, chars: [] },
  ];
  const FW = o.empty ? [] : [{ realm_id: 'kzdvvj2umnduyauf', name: "Bo's world", user: 'GZZZZZZZZZZZZZZZ', at: now - 7200 },
    { realm_id: 'aaaaaaaaaaaaaaaa', name: '', user: 'GYYYYYYYYYYYYYYY', at: now - 86400 * 3 }];
  const copy = (x) => JSON.parse(JSON.stringify(x));
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    window.__calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return copy(st);
      case 'select': return null;
      case 'realms_list': return { realms: copy(realms), network: { realms: 3, players: 42, bots: 80 }, from_cache: false, saved_at: 1, error: null };
      case 'mods_catalog': return copy(catalog);
      case 'mods_ranked': return { ids: ['mod:alpha', 'mod:bravo', 'mod:charlie'], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: WRATH.id, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'looks_state': return { build: '3.3.5a', on: [], changed: false, how: {} };
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'theme_apply': return null;
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'docker_info': return { state: 'ready', version: '27', help: '', get_url: '', can_start: false };
      case 'host_console': throw 'no kit';
      case 'host_progress': return { idle: true, kind: 'host', running: false, code: null };
      case 'social_status': return true;
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'engine_status': return copy(WS);
      case 'world_status': return copy(Object.assign({}, WS, { kind: args.kind }));
      case 'world_resume_pending': return null;
      case 'world_settings_get': return copy(Object.assign({}, SET, { kind: args.kind }));
      case 'engine_progress': return { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7 };
      case 'local_characters': return [{ kind: 'single', title: 'Single player', chars: [{ name: 'Odo' }] }];
      case 'den_net_state': return { running: false, connected: false, relay: '', realm_id: '', address: '', auth_port: 0, world_port: 0, streams: 0, last_error: '' };
      case 'world_invite': return { link: 'https://relay.leylinelauncher.com/join/abcdefghijklmnop#k~~via', code: 'DEN-ABCD', name: 'your friend', expires: 0,
        tunnel: { running: true, connected: true, streams: 0 }, ways: [] };
      case 'friends_start': return FRIENDS.net;
      case 'friends_state': return copy(FRIENDS);
      case 'friends_code': return 'LF1-' + 'a'.repeat(60) + '@relay.leylinelauncher.com';
      case 'world_friends': return copy(args.kind === 'single' ? WF : []);
      case 'friend_worlds': return copy(FW);
      case 'open_link': case 'open_url': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });

async function open(o = {}) {
  const ctx = await browser.newContext({ colorScheme: o.light ? 'light' : 'dark', viewport: { width: 1360, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, o);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineWorldFriends && window.LeylineFriends && window.LeylineThemes);
  if (o.light) { await page.evaluate(() => window.LeylineThemes.apply('light', { save: true })); await page.waitForFunction(() => document.documentElement.dataset.base === 'light'); }
  return { page, ctx, errors };
}
async function single(page, o = {}) {
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.evaluate(() => window.LeylineWorldFriends.refresh());
  if (!o.empty) {
    await page.waitForSelector('#lf .lf-friend');
    await page.waitForSelector('#wf .wf-friend');
    await page.waitForSelector('#fw:not([hidden]) .fw-world');
  } else await page.waitForSelector('#lf .lf-empty, #lf .lk-empty');
  await page.waitForTimeout(300);
}
// an element shot: the window made tall enough that the page doesn't scroll under the play bar, then put back
async function shotEl(page, sel, name) {
  if (!SHOTS) return;
  const el = await page.$(sel);
  if (!el || !(await el.isVisible())) return;
  const vp = page.viewportSize();
  await page.setViewportSize({ width: vp.width, height: 3400 });
  await page.waitForTimeout(200);
  await el.scrollIntoViewIfNeeded();
  await el.screenshot({ path: path.join(SHOTS, name + '.png') });
  await page.setViewportSize(vp);
}

for (const theme of ['dark', 'light']) {
  test(`the Friends area in the full app (${theme}): stone cards, the launcher's buttons and titles; Realms and Mods to compare`, async () => {
    const light = theme === 'light';
    const { page, ctx, errors } = await open({ light });
    await single(page);
    // the rows are the launcher's cards: not the flat page colour, with an edge
    const look = await page.evaluate(() => {
      const cs = (s) => { const e = document.querySelector(s); return e ? getComputedStyle(e) : null; };
      const wf = cs('#wf .wf-friend'), lf = cs('#lf .lf-friend'), fw = cs('#fw .fw-world');
      const h = cs('#wf .oc-set-h'), lh = cs('#lf .lf-title'), sh = cs('#spf-title');
      const join = cs('#fw .fw-join');
      return { wfBg: wf.backgroundImage + '|' + wf.backgroundColor, lfBg: lf.backgroundImage + '|' + lf.backgroundColor, fwBg: fw.backgroundImage + '|' + fw.backgroundColor,
        wfBorder: wf.borderTopWidth, h: h.fontFamily, lh: lh.fontFamily, sh: sh.fontFamily, join: join.borderImageSource,
        wfPos: getComputedStyle(document.getElementById('wf')).position };
    });
    assert.equal(look.wfPos, 'static', '#wf sits in the page (not the Play with friends window)');
    for (const k of ['wfBg', 'lfBg', 'fwBg']) assert.notEqual(look[k], 'none|rgba(0, 0, 0, 0)', k + ': a card, not a bare row');
    assert.notEqual(look.wfBorder, '0px');
    for (const k of ['h', 'lh', 'sh']) assert.match(look[k], /Marcellus/, k + ': the launcher\'s title face');
    assert.match(look.join, /stone-btn/, 'Join again is the red stone button');
    if (!light) assert.match(look.wfBg, /stone-side/, 'the stone texture under the cards');
    assert.doesNotMatch(await page.evaluate(() => document.getElementById('spf').innerText), BRANDS);
    await shotEl(page, '#spf', `look-friends-${theme}-area`);
    await shotEl(page, '#wf', `look-friends-${theme}-world`);
    await shotEl(page, '#fw', `look-friends-${theme}-worlds`);
    await shotEl(page, '#lf', `look-friends-${theme}-list`);
    // the remove question on a row
    await page.click('#wf .wf-friend[data-user="GAAAAAAAAAAAAAAA"] .wf-delete');
    await page.waitForSelector('#wf .wf-confirm.bad');
    await shotEl(page, '#wf .wf-friend[data-user="GAAAAAAAAAAAAAAA"]', `look-friends-${theme}-confirm`);
    await page.click('#wf .wf-no');
    await page.click('#lf .lf-friend .lf-remove');
    await page.waitForSelector('#lf .lf-confirm');
    await shotEl(page, '#lf .lf-list', `look-friends-${theme}-list-confirm`);
    // the screens they must match
    await page.evaluate(() => window.LeylineNav.show('realms'));
    await page.waitForSelector('#page-realms .realm-card.rl');
    await page.waitForTimeout(300);
    if (SHOTS) await page.screenshot({ path: path.join(SHOTS, `look-compare-${theme}-realms.png`) });
    await page.evaluate(() => window.LeylineNav.show('mods'));
    await page.waitForFunction(() => document.querySelectorAll('#cat-grid .mod-card').length > 0, null, { timeout: 15000 }).catch(() => {});
    await page.waitForTimeout(300);
    if (SHOTS) await page.screenshot({ path: path.join(SHOTS, `look-compare-${theme}-mods.png`) });
    await page.evaluate(() => window.LeylineNav.show('single'));
    await page.waitForTimeout(300);
    await page.evaluate(() => document.getElementById('spf').scrollIntoView());
    await page.waitForTimeout(200);
    if (SHOTS) await page.screenshot({ path: path.join(SHOTS, `look-compare-${theme}-single.png`), fullPage: false });
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}

test('the empty Friends area in the full app: the kit\'s empty words on the stone', async () => {
  const { page, ctx, errors } = await open({ empty: true });
  await single(page, { empty: true });
  assert.equal(await page.isHidden('#fw'), true, 'no friends\' worlds: hidden');
  await shotEl(page, '#spf', 'look-friends-dark-empty');
  assert.deepEqual(errors, []);
  await ctx.close();
});
