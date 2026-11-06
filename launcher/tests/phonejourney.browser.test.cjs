// The phone app's whole journey (docs/MOBILE-RELEASE.md), end to end in Chromium as a phone, with a fake core that
// answers like the phone build's (src-tauri/src/mobile.rs: its short list of commands, every other one "not found"):
//   1. signed out: Account › Log in with your Hero Key, then PLAY opens the website's Play page in the app;
//   2. Join a friend: a friend's invite link to their PC-hosted realm plays it in the app in one tap (Paste invite link
//      and join reads the clipboard): /wrath?go=r:<id>#inv=<invite>, the invite only in the #fragment; the host's own
//      link (~<host key>) gives the invite only; the leyline://join?link=… form too; a friend's Den link or code, a
//      link on another site and an empty clipboard each get one plain line and nothing opens;
//   3. every page a phone player reaches (Play, Realms, Mods, Account, News, Forum, Modes, Help, the sign-in dialog)
//      fits the screen upright and sideways, with no banned words.
// PHONE_SHOTS=<dir> also saves the screenshots the mobile release page uses (tools/mobile-shots/README.md).
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/phonejourney.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const SITE = 'https://leylinelauncher.com';
const PLAY_URL = SITE + '/wrath';
const KEY = 'ovk1.AbCdEf1234.' + 'k'.repeat(43);
const RID = 'abcdefghijk2345a';
const SECRET = 'ABCDEFGHJKMNPQRSTVWX';
const INVITE = `https://relay.leylinelauncher.com/join/${RID}#${SECRET}`;
const HOP = `${PLAY_URL}?go=r:${RID}#inv=${SECRET}`;
const PORTRAIT = { width: 390, height: 844 };
const LANDSCAPE = { width: 844, height: 390 };
const BANNED = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;
const SHOTS = process.env.PHONE_SHOTS || '';

const FAKE = (o) => {
  const KEY = 'ovk1.AbCdEf1234.' + 'k'.repeat(43);
  const calls = (window.__calls = []);
  try { localStorage.clear(); } catch (_) { /* fine */ }
  window.__signed = !!o.signed;
  Object.defineProperty(Navigator.prototype, 'gpu', { configurable: true, get: () => (o.gpu === false ? undefined : { requestAdapter: async () => ({}) }) });
  const platform = { os: 'android', mobile: true, node: false };
  const base = { official: false, kind: 'PvE', ruleset: 'main', state: 'live', version: '3.3.5a', core: 'azerothcore', build: '3.3.5a', tags: [],
    region: null, language: null, max_players: null, is_new: false, featured: false, verified: false, direct: false, rules: [], claimed: true,
    players: 0, bots: 0, tagline: '', summary: '', join_url: null, discord: null, relay: 'relay.leylinelauncher.com', report_url: null };
  const realms = [
    Object.assign({}, base, { slug: 'official', name: 'Wrath Unleashed', official: true, address: 'leylinelauncher.com', group: 'official', players: 12, bots: 40 }),
    Object.assign({}, base, { slug: 'c-bbbbbbbbbbbbbbbb', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Frost Haven', tags: ['bots'], players: 30, bots: 40,
      max_players: 100, address: 'relay.leylinelauncher.com:40002', group: 'community' }),
  ];
  const me = { pid: 'aaaaaaaaaa', name: 'Alice', tag: '4821', offline: false, profile: { avatar: '', setup: true, avatar_url: '', bio: '' } };
  const P = (id, name, extra) => Object.assign({ id, source: 'packaged', name, description: `${name} does a thing.`, category: 'addons',
    builds: ['3.3.5a'], author: 'Leyline', author_url: '', licence: 'MIT', repo: '', page: '', credits: name, stars: null, last_push: '',
    version: '0.1.0', kind: 'ui', permissions: [], on_wrath: true, download: 'https://leylinelauncher.com/wrath/mods/x/download',
    sha256: 'ab'.repeat(32), size: 1000 }, extra || {});
  const catalog = { categories: [{ id: 'addons', name: 'Addons and UI' }, { id: 'sandbox', name: 'Sandbox and game modes' }],
    projects: [P('mod:pings', 'Pings'), P('mod:dice', 'Dalaran Dice', { category: 'sandbox', kind: 'both' }),
      P('mod:far', 'Far Sight', { category: 'sandbox', kind: 'client' })], from_cache: false, saved_at: 1, error: null };
  const ok = {
    platform,
    get_state: { version: 'test', settings: { clients: [], selected_client: null, selected_realm: 'leylinelauncher.com',
      wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'leylinelauncher.com',
      browser_url: 'https://leylinelauncher.com/wrath', windows: false, data_dir: '/data/user/0/com.leylinelauncher.app', platform },
    realms_list: { realms, network: { realms: 2, players: 42, bots: 80 }, from_cache: false, saved_at: 1, error: null },
    realm_status: { state: 'online', ping_ms: 40, players: 12 },
    mods_catalog: catalog,
    mods_ranked: { ids: [], metrics: {}, from_cache: false, error: null },
    deep_link_take: null,
    modes_list: { modes: [], from_cache: false, builtin: false, saved_at: null, error: null },
    news_list: { posts: [{ title: 'Leyline on your phone', date: '2026-11-05', summary: 'Play in the app, solo on your phone, join a friend.', url: 'https://leylinelauncher.com/wrath/mobile' }], from_cache: false, error: null },
    forum_call: { threads: [], categories: [] },
    social_ways: { herokey: true, google: true },
    social_key_found: null,
    open_browser_play: null, open_link: null, open_url: null,
  };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    if (cmd === 'social_status') return window.__signed;
    if (cmd === 'social_key_signin') {
      if (!String(args.key).includes(KEY)) throw "That Hero Key didn't work.";
      window.__signed = true;
      return { ok: true, me: { pid: me.pid, name: me.name, tag: me.tag } };
    }
    if (cmd === 'social_call') {
      if (args.action === 'me') { if (!window.__signed) throw 'signed_out'; return { me: JSON.parse(JSON.stringify(me)), avatars: [], ways: { herokey: true, google: true }, manage_url: '' }; }
      if (args.action === 'herokey_status') return { web_only: false, has_key: true, key: '', saved: true, made: '5 Nov 2026', last_used: '', has_password: false };
      if (args.action === 'state') return { friends: [], incoming: [], outgoing: [] };
      return {};
    }
    if (Object.prototype.hasOwnProperty.call(ok, cmd)) return JSON.parse(JSON.stringify(ok[cmd]));
    throw `Command ${cmd} not found`;   // what Tauri says for a command the phone build doesn't have
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true }); });
test.after(async () => { await browser.close(); });

async function open(opts = {}, viewport = PORTRAIT) {
  const ctx = await browser.newContext({ viewport, hasTouch: true, isMobile: true, colorScheme: 'dark', deviceScaleFactor: SHOTS ? 2 : 1 });
  if (opts.clipboard !== undefined) await ctx.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: 'null' }).catch(() => {});
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route(/^https?:\/\//, (r) => (r.request().url().startsWith(PLAY_URL)
    ? r.fulfill({ contentType: 'text/html', body: '<!doctype html><title>Play</title><p id="game">the browser game</p>' })
    : r.abort()));
  await page.addInitScript(FAKE, opts);
  if (opts.clipboard !== undefined) {
    // the phone's clipboard: what the friend's message app copied (null: a web view that won't read it)
    await page.addInitScript((text) => {
      Object.defineProperty(Navigator.prototype, 'clipboard', { configurable: true,
        get: () => (text === null ? undefined : { readText: async () => text, writeText: async () => {} }) });
    }, opts.clipboard);
  }
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylinePhone && window.LeylinePhone.on && document.querySelector('#page-phone.on'));
  return { page, ctx, errors };
}
const tab = async (page, name) => { await page.click(`#phone-tabs [data-ph-tab="${name}"]`); await page.waitForTimeout(250); };
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, name + '.png') }); };

const fit = (page, root) => page.evaluate((root) => {
  const W = innerWidth;
  const top = document.querySelector(root);
  const shown = (e) => { const s = getComputedStyle(e); const r = e.getBoundingClientRect(); return s.display !== 'none' && s.visibility !== 'hidden' && r.width > 0 && r.height > 0; };
  const clipped = (e) => { for (let p = e.parentElement; p && p !== top; p = p.parentElement) { if (getComputedStyle(p).overflowX !== 'visible') return true; } return false; };
  const wide = [], small = [];
  for (const e of top.querySelectorAll('*')) {
    if (!shown(e)) continue;
    const r = e.getBoundingClientRect();
    if (r.right > W + 1 && !clipped(e)) wide.push(`${e.tagName}.${e.className}#${e.id}`);
    if (e.matches('button, .btn, select, input:not([type="checkbox"]):not([type="radio"]), summary') && (r.height < 44 || r.width < 44)) {
      small.push(`${e.tagName}#${e.id}.${e.className} "${(e.textContent || e.value || '').trim().slice(0, 24)}" ${Math.round(r.width)}x${Math.round(r.height)}`);
    }
  }
  return { docScroll: document.documentElement.scrollWidth - W, wide: wide.slice(0, 8), small: small.slice(0, 8), text: top.innerText };
}, root);

test('signed out: Account › Log in with your Hero Key, then PLAY opens the website\'s Play page in the app', async () => {
  const { page, ctx, errors } = await open({ signed: false });
  await shot(page, 'play-tab');
  await tab(page, 'account');
  await page.waitForSelector('#page-account .sf-herokey');
  await shot(page, 'account-signed-out');
  await page.click('#page-account .sf-herokey');
  await page.waitForSelector('#gate:not([hidden]) #gate-key, #page-account #sf-key, #page-account textarea, #page-account input[type="password"]', { timeout: 4000 });
  // the Hero Key goes in where it is asked for (the sign-in dialog, or the Account page's own form)
  const where = await page.evaluate(() => (document.querySelector('#gate:not([hidden]) #gate-key') ? 'gate' : 'account'));
  if (where === 'gate') {
    const g = await fit(page, '#gate');
    assert.deepEqual(g.wide, [], 'the sign-in dialog fits the phone');
    assert.doesNotMatch(g.text, BANNED);
    await page.fill('#gate-key', KEY);
    await page.click('#gate-keyform button[type=submit]');
  } else {
    const box = await page.$('#page-account textarea, #page-account input[name="key"], #page-account #sf-key');
    await box.fill(KEY);
    await box.press('Enter');
  }
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'social_key_signin'));
  assert.ok(await page.evaluate(() => window.__calls.filter(([c]) => c === 'social_key_signin').every(([, a]) => a.key.includes('ovk1.'))), 'only the key goes to the core');
  await page.waitForFunction(() => window.__signed === true);
  await page.waitForFunction(() => window.LeylineSocial && window.LeylineSocial.signedIn(), null, { timeout: 8000 });
  await page.waitForTimeout(1800);   // the welcome moment closes by itself
  await tab(page, 'phone');
  await Promise.all([page.waitForURL(PLAY_URL), page.click('#ph-play')]);
  assert.equal(await page.textContent('#game'), 'the browser game');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Join a friend: Paste invite link and join plays their PC-hosted realm in the app, the invite only in the #fragment', async () => {
  const { page, ctx, errors } = await open({ signed: true, clipboard: INVITE });
  assert.equal(await page.textContent('#ph-join-title'), 'Join a friend');
  await shot(page, 'join-a-friend');
  const req = [];
  page.on('request', (r) => req.push(r.url()));
  await Promise.all([page.waitForURL(HOP), page.click('#ph-paste-join')]);
  assert.equal(page.url(), HOP);
  assert.ok(req.every((u) => !u.includes(SECRET)), 'the invite never goes to a server (it is after #)');
  assert.ok(!(await page.evaluate(() => (window.__calls || []).some(([c]) => /^(join_invite|play|play_realm)$/.test(c)))));
  await page.goBack();
  await page.waitForFunction(() => window.LeylinePhone && window.LeylinePhone.on && document.querySelector('#page-phone.on'));
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Join a friend: the host\'s own link, the leyline:// form and Realms › Got an invite link? all hop with the invite only', async () => {
  const { page, ctx, errors } = await open({ signed: true });
  const parse = (t) => page.evaluate((t) => window.LeylinePhone.parseInvite(t), t);
  assert.deepEqual(await parse(INVITE), { rid: RID, invite: SECRET, den: false });
  assert.deepEqual(await parse(INVITE + '~HOSTSECRETHOSTSECRET'), { rid: RID, invite: SECRET, den: false }, 'the host key never leaves');
  assert.deepEqual(await parse('leyline://join?link=' + encodeURIComponent(INVITE)), { rid: RID, invite: SECRET, den: false });
  assert.deepEqual(await parse(`https://relay.openvanilla.app/join/${RID}#${SECRET}`), { rid: RID, invite: SECRET, den: false }, 'the old relay name');
  assert.deepEqual(await parse(INVITE + '~~GABCD.PASSWORD1'), { rid: RID, invite: SECRET, den: true });
  for (const bad of [`https://evil.example.com/join/${RID}#${SECRET}`, `http://relay.leylinelauncher.com/join/${RID}#${SECRET}`,
    `https://relay.leylinelauncher.com/join/${RID}`, `https://relay.leylinelauncher.com/join/../x#${SECRET}`, `https://relay.leylinelauncher.com/join/${RID}#short`]) {
    assert.equal(await parse(bad), null, bad);
  }
  // typed into the box on Play
  await page.fill('#ph-invite', INVITE + '~HOSTSECRETHOSTSECRET');
  await Promise.all([page.waitForURL(HOP), page.click('#ph-join')]);
  await page.goBack();
  await page.waitForFunction(() => window.LeylinePhone && window.LeylinePhone.on);
  // Realms › Got an invite link?
  await tab(page, 'realms');
  await page.waitForSelector('#rl-list .realm-card');
  await page.click('#rl-other summary');
  assert.ok(await page.isVisible('#rl-other .ph-only'));
  assert.equal(await page.isVisible('#rl-other .pc-only'), false);
  await page.fill('#rl-invite', 'leyline://join?link=' + encodeURIComponent(INVITE));
  await Promise.all([page.waitForURL(HOP), page.click('#rl-join')]);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Join a friend: a Den, another site, an empty or unreadable clipboard, and no WebGPU each get one plain line', async () => {
  for (const [clip, typed, line] of [
    [INVITE + '~~GABCD.PASSWORD1', '', /^That's an invite to a friend's Den\. Dens run in the game on a computer/],
    ['DEN-ABCD-EFGH', '', /^That's an invite to a friend's Den/],
    ['', 'DEN-ABCD-EFGH', /^That's an invite to a friend's Den/],
    [`https://evil.example.com/join/${RID}#${SECRET}`, '', /^What you copied isn't a Leyline invite link/],
    ['', '', /^Paste the invite link your friend sent in the box, then Join\.$/],
    [null, '', /^Paste the invite link your friend sent in the box, then Join\.$/],
  ]) {
    const { page, ctx, errors } = await open({ signed: true, clipboard: clip });
    if (typed) await page.fill('#ph-invite', typed);
    await page.click(typed ? '#ph-join' : '#ph-paste-join');
    await page.waitForTimeout(150);
    assert.equal(page.url(), INDEX, `${clip} ${typed}: nothing opens`);
    assert.match(await page.textContent('#ph-join-say'), line, `${clip} ${typed}`);
    assert.ok(await page.isVisible('#ph-join-say'));
    if (clip === INVITE + '~~GABCD.PASSWORD1') await shot(page, 'join-den-line');
    assert.deepEqual(errors, []);
    await ctx.close();
  }
  const { page, ctx, errors } = await open({ signed: true, clipboard: INVITE, gpu: false });
  await page.click('#ph-paste-join');
  await page.waitForTimeout(150);
  assert.equal(page.url(), INDEX);
  assert.ok(await page.isVisible('#ph-nogpu'), 'the same Open in Chrome line as PLAY');
  assert.deepEqual(errors, []);
  await ctx.close();
});

for (const [label, viewport] of [['portrait', PORTRAIT], ['landscape', LANDSCAPE]]) {
  test(`${label}: every page a phone player reaches fits the screen, with 44 px controls and no banned words`, async () => {
    const { page, ctx, errors } = await open({ signed: true }, viewport);
    for (const p of ['phone', 'realms', 'mods', 'account', 'news', 'forum', 'modes', 'help']) {
      await page.evaluate((p) => window.LeylineNav.show(p, false, p === 'mods' ? 'discover' : undefined), p);
      await page.waitForTimeout(300);
      if (p === 'realms') await page.waitForSelector('#rl-list .realm-card');
      if (p === 'mods') await page.waitForSelector('#page-mods .ph-fit', { timeout: 4000 }).catch(() => {});
      const on = await page.$$eval('.page.on', (s) => s.map((x) => x.id));
      assert.deepEqual(on, ['page-' + p], `${p} opens on a phone`);
      const f = await fit(page, 'main');
      assert.ok(f.docScroll <= 0, `${p}: no sideways scrolling (${f.docScroll})`);
      assert.deepEqual(f.wide, [], `${p}: nothing wider than the screen`);
      assert.deepEqual(f.small, [], `${p}: every control at least 44 px`);
      assert.doesNotMatch(f.text, BANNED, p);
      if (label === 'portrait' && ['realms', 'mods'].includes(p)) await shot(page, p + '-tab');
    }
    if (label === 'landscape') await page.evaluate(() => window.LeylineNav.show('phone')).then(() => page.waitForTimeout(200)).then(() => shot(page, 'play-tab-landscape'));
    assert.deepEqual(errors, []);
    await ctx.close();
  });
}
