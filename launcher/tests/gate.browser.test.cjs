// The sign-in screen (ui/gate.js) and seamless realm switching (ui/app.js) in Chromium, with a fake Tauri core (owner
// 2 Nov: "create login flow for the launcher so people have to log in to use it etc with hero key sensed from browser
// login or pasted"; "in the end swapping realms on community and official needs to be seamless, swap realm list, work
// with hero key etc"; "not game 3.3.5a but wrath 3.3.5a for example and ordered properly").
//
// Covers: signed out, the screen covers the launcher (Log in | Sign up, no Discord); Sign in with your browser (the
// website's code, no way named), Google, a Hero Key pasted or chosen as its file, a file without a key; the signed-in
// moment (picture, name#tag) and the chip after; Sign up (Create your Hero Key, the key shown once in Manage account);
// a kept sign-in lets you in offline; the website down: said plainly, Try again, Continue without signing in; PLAY on
// the official realm signed out opens the screen and goes on once signed in; Sign out brings it back; switching between
// the official realm and community realms (the picker, the Realms tabs, PLAY and the cards' Play) calls the right
// command with the right realm and client each time, the old status line gone; clients named by era, in era order.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/gate.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const os = require('node:os');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const KEY = 'ovk1.AbCdEf1234.' + 'k'.repeat(43);
const OFFICIAL = 'play.openvanilla.app';

const FAKE = (o) => {
  const KEY = 'ovk1.AbCdEf1234.' + 'k'.repeat(43);
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  window.__signed = !!o.signed;
  window.__poll = 'signed_in';
  const clients = o.clients || [{ id: 'c335a-1', name: 'My Wrath', path: '/g/wrath', build: '3.3.5a', realmlist: '' }];
  const st = { version: 'test', settings: { clients, selected_client: clients[0] ? clients[0].id : null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  const base = { official: false, kind: 'PvE', ruleset: 'main', state: 'live', core: 'azerothcore', tags: [], players: 3, bots: 0, region: null, language: null,
    max_players: null, featured: false, verified: false, direct: false, rules: [], claimed: true, tagline: '', summary: '', join_url: null, relay: 'relay.openvanilla.app' };
  const realms = [
    Object.assign({}, base, { slug: 'official', name: 'Wrath Unleashed', official: true, build: '3.3.5a', version: '3.3.5a', address: 'play.openvanilla.app', group: 'official' }),
    Object.assign({}, base, { slug: 'c-dddddddddddddddd', realm_id: 'dddddddddddddddd', name: 'Old World', build: '1.12.1', version: '1.12.1', core: 'vmangos',
      address: 'relay.openvanilla.app:40020', group: 'community', join_url: 'https://oldworld.example/join' }),
    Object.assign({}, base, { slug: 'c-bbbbbbbbbbbbbbbb', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Busy Den', build: '3.3.5a', version: '3.3.5a',
      address: 'relay.openvanilla.app:40002', group: 'community' }),
  ];
  const me = { pid: 'aaaaaaaaaa', name: 'Alice', tag: '4821', offline: false,
    profile: { avatar: '', setup: true, avatar_url: 'https://play.openvanilla.app/wrath/avatar/u/aaaaaaaaaa.svg?v=2', bio: '' } };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'select': st.settings.selected_client = args.client; st.settings.selected_realm = args.realm; return null;
      case 'social_status': return window.__signed;
      case 'social_ways': if (o.down && !window.__up) throw "Can't reach the Leyline server. Check your connection."; return { herokey: true, google: true, discord: true };
      case 'social_signin_start': return { state: 'pending', user_code: 'BCDF-GHJK', verify_url: 'https://play.openvanilla.app/wrath/link?code=BCDF-GHJK' };
      case 'social_signin_poll': if (window.__poll === 'signed_in') window.__signed = true; return { state: window.__poll, user_code: 'BCDF-GHJK', verify_url: '' };
      case 'social_signin_cancel': case 'social_signin_reopen': return null;
      case 'social_signout': window.__signed = false; return null;
      case 'social_key_signin': if (!args.key.includes(KEY)) throw "That Hero Key didn't work. It may have been replaced by a newer one: use your newest key.";
        window.__signed = true; return { ok: true, me: { pid: 'aaaaaaaaaa', name: 'Alice', tag: '4821' } };
      case 'social_new_account': window.__signed = true; me.name = 'Brightfeather42'; me.tag = '1234';
        return { ok: true, key: 'ovk1.NewNew1234.' + 'n'.repeat(43), me: { pid: 'aaaaaaaaaa', name: 'Brightfeather42', tag: '1234' } };
      case 'social_call':
        if (args.action === 'me') {
          if (o.offline) throw "Can't reach the Leyline server. Check your connection.";
          if (!window.__signed) throw 'signed_out';
          return { me: JSON.parse(JSON.stringify(me)), avatars: [], ways: { herokey: true, google: true }, manage_url: '' };
        }
        if (args.action === 'herokey_status') return { web_only: false, has_key: true, key: '', saved: false, made: '2 Nov 2026', last_used: '', has_password: false };
        if (args.action === 'state') return { friends: [], incoming: [], outgoing: [] };
        throw 'unexpected social call ' + args.action;
      case 'realms_list': return { realms, network: { realms: 3, players: 9, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 3 };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'play': st.settings.selected_client = args.client; st.settings.selected_realm = args.realm;
        return `Started ${clients.find((c) => c.id === args.client).name} on ${args.realm}.`;
      case 'play_realm': { const r = realms.find((x) => x.slug === args.slug); st.settings.selected_client = args.client; st.settings.selected_realm = r.address;
        return `Started ${clients.find((c) => c.id === args.client).name} on ${r.address}.`; }
      case 'forum_call': return { threads: [], categories: [] };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'deep_link_take': return null;
      case 'open_link': case 'open_url': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts) {
  const page = await browser.newPage({ viewport: { width: 1400, height: 860 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.abort());
  await page.addInitScript(FAKE, opts || {});
  await page.goto(INDEX);
  return { page, errors };
}
const callsOf = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c).map(([, a]) => a), cmd);
const gateOpen = (page) => page.evaluate(() => !document.getElementById('gate').hidden);

test('signed out: the screen covers the launcher; Log in | Sign up; no Discord; the browser sign-in, then name#tag and picture', async () => {
  const { page, errors } = await open();
  await page.waitForSelector('#gate-browser');
  assert.ok(await gateOpen(page));
  assert.ok(await page.evaluate(() => document.querySelector('.frame').inert), 'nothing behind it can be used');
  assert.equal(await page.getAttribute('#gate-tab-login', 'aria-selected'), 'true');
  assert.equal(await page.getAttribute('#gate-tab-signup', 'aria-selected'), 'false');
  const text = await page.textContent('#gate');
  assert.doesNotMatch(text, /discord/i, 'no Discord sign-in, even when the website offers it');
  assert.doesNotMatch(text, /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i);
  assert.match(text, /Continue with Google/);
  assert.match(text, /Drop your Hero Key file here/);
  await page.click('#gate-browser');
  await page.waitForSelector('#gate-code');
  assert.equal(await page.textContent('#gate-code'), 'BCDF-GHJK');
  assert.deepEqual(await callsOf(page, 'social_signin_start'), [{ via: null }], 'the website picks the way: a signed-in browser only approves');
  // approved: the signed-in moment, then the launcher with the chip
  await page.waitForSelector('#gate-in', { timeout: 8000 });
  assert.equal(await page.textContent('#gate-in .gate-in-n'), 'Alice#4821');
  assert.equal(await page.locator('#gate-in .gate-av img').count(), 1, 'the picture');
  await page.click('#gate-go');
  assert.equal(await gateOpen(page), false);
  assert.equal(await page.evaluate(() => document.querySelector('.frame').inert), false);
  assert.equal(await page.textContent('#hf-me .hf-me-name'), 'Alice');
  assert.equal(await page.textContent('#hf-me .hf-me-st'), '#4821');
  assert.deepEqual(errors, []);
  await page.close();
});

test('Google, a pasted Hero Key, a chosen key file; a file without a key says so', async () => {
  const { page, errors } = await open();
  await page.waitForSelector('#gate-google');
  await page.click('#gate-google');
  await page.waitForSelector('#gate-cancel');
  assert.deepEqual(await callsOf(page, 'social_signin_start'), [{ via: 'google' }]);
  await page.click('#gate-cancel');
  await page.waitForSelector('#gate-key');
  // a wrong paste: said before anything goes anywhere
  await page.fill('#gate-key', 'hello');
  await page.click('#gate-keyform button[type=submit]');
  assert.match(await page.textContent('#gate-err'), /starts with ovk1/);
  assert.deepEqual(await callsOf(page, 'social_key_signin'), []);
  // a file that isn't a key
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'gate-'));
  fs.writeFileSync(path.join(dir, 'notes.txt'), 'just some notes');
  await page.setInputFiles('#gate-file', path.join(dir, 'notes.txt'));
  await page.waitForFunction(() => /no Hero Key in that file/.test((document.getElementById('gate-err') || {}).textContent || ''));
  assert.deepEqual(await callsOf(page, 'social_key_signin'), []);
  // the launcher's own key file
  fs.writeFileSync(path.join(dir, 'Leyline Hero Key.txt'), `Leyline Hero Key\r\n\r\nName: Alice#4821\r\nKey: ${KEY}\r\nMade: 2 November 2026\r\n`);
  await page.setInputFiles('#gate-file', path.join(dir, 'Leyline Hero Key.txt'));
  await page.waitForSelector('#gate-in');
  assert.deepEqual(await callsOf(page, 'social_key_signin'), [{ key: KEY }], 'only the key goes to the core');
  await page.click('#gate-go');
  assert.equal(await gateOpen(page), false);
  // Sign out: the screen again
  await page.click('#hf-me-btn');
  await page.click('#hf-panel .ap-out');
  await page.waitForSelector('#gate-browser');
  assert.ok(await gateOpen(page));
  // a pasted key (spaces and an upper-case prefix are fine)
  await page.fill('#gate-key', '  OVK1.' + KEY.slice(5) + ' ');
  await page.click('#gate-keyform button[type=submit]');
  await page.waitForSelector('#gate-in');
  assert.deepEqual((await callsOf(page, 'social_key_signin')).slice(-1), [{ key: KEY }]);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Sign up: Create your Hero Key makes the account; Manage account shows the key once to save', async () => {
  const { page, errors } = await open();
  await page.waitForSelector('#gate-tab-signup');
  await page.click('#gate-tab-signup');
  assert.equal(await page.getAttribute('#gate-tab-signup', 'aria-selected'), 'true');
  assert.match(await page.textContent('#gate-pane'), /What is a Hero Key\?/);
  await page.click('#gate-new');
  await page.waitForFunction(() => document.getElementById('gate').hidden);
  assert.ok(await page.locator('#page-account').evaluate((e) => e.classList.contains('on')), 'Manage account');
  await page.waitForSelector('#ac-keytext');
  await page.click('#ac-key-show');
  assert.equal(await page.textContent('#ac-keytext'), 'ovk1.NewNew1234.' + 'n'.repeat(43));
  assert.equal(await page.textContent('#hf-me .hf-me-name'), 'Brightfeather42');
  assert.deepEqual(errors, []);
  await page.close();
});

test('a sign-in kept from before: straight in, offline too (never locked out)', async () => {
  const { page, errors } = await open({ signed: true, offline: true });
  await page.waitForSelector('#hf-retry');
  assert.equal(await gateOpen(page), false);
  assert.equal(await page.textContent('#hf-me .hf-me-name'), 'Signed in');
  assert.match(await page.textContent('#hf-me .hf-me-st'), /Offline/);
  assert.equal(await page.locator('#hf-signin').count(), 0, 'no Log In: the sign-in is kept');
  assert.deepEqual(errors, []);
  await page.close();
});

test('the website down: said plainly, Try again, Continue without; PLAY on the official realm opens sign-in, then goes on', async () => {
  const { page, errors } = await open({ down: true });
  await page.waitForSelector('#gate-down');
  assert.match(await page.textContent('#gate-down'), /website isn't answering/);
  await page.click('#gate-retry');   // still down: still said
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'social_ways').length >= 2);
  assert.ok(await page.isVisible('#gate-down'));
  await page.click('#gate-skip');
  assert.equal(await gateOpen(page), false);
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'realms_list'));
  // PLAY on Wrath Unleashed, signed out: never a start at the stock login screen; the screen, with why
  await page.click('#play-go');
  await page.waitForSelector('#gate-why');
  assert.match(await page.textContent('#gate-why'), /Wrath Unleashed signs you in with your Leyline account/);
  assert.deepEqual(await callsOf(page, 'play'), [], 'nothing started');
  await page.evaluate(() => { window.__up = true; });
  await page.click('#gate-retry');
  await page.fill('#gate-key', KEY);
  await page.click('#gate-keyform button[type=submit]');
  await page.waitForSelector('#gate-go');
  assert.equal(await page.textContent('#gate-go'), 'Continue to PLAY');
  await page.click('#gate-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play'));
  assert.deepEqual(await callsOf(page, 'play'), [{ client: 'c335a-1', realm: OFFICIAL }], 'the PLAY goes on, once');
  assert.deepEqual(errors, []);
  await page.close();
});

const TWO = [
  { id: 'c335a-1', name: 'Game 3.3.5a (3.3.5a (12340))', path: '/g/wrath', build: '3.3.5a', realmlist: '' },
  { id: 'c1121-1', name: 'Game (1.12.1 (5875))', path: '/g/vanilla', build: '1.12.1', realmlist: '' },
  { id: 'c243-1', name: 'Outland (2.4.3 (8606))', path: '/g/tbc', build: '2.4.3', realmlist: '' },
];

test('clients: named by era (not "Game 3.3.5a (3.3.5a (12340))"), in era order everywhere', async () => {
  const { page, errors } = await open({ signed: true, clients: TWO });
  await page.waitForFunction(() => document.getElementById('pb-client-name').textContent === 'Wrath 3.3.5a');
  await page.click('#pb-client');
  const mine = await page.$$eval('#pb-menu .pb-mine .pb-opt-name', (n) => n.map((x) => x.textContent));
  assert.deepEqual(mine, ['Vanilla 1.12.1', 'Outland (TBC)', 'Wrath 3.3.5a']);
  assert.equal(await page.locator('#pb-menu .pb-missing').count(), 0, 'every offered family found');
  await page.keyboard.press('Escape');
  assert.deepEqual(await page.$$eval('#play-client option', (o) => o.map((x) => x.textContent)), ['Vanilla 1.12.1', 'Outland (TBC)', 'Wrath 3.3.5a']);
  assert.deepEqual(await page.$$eval('.client-pick', (s) => [...s[0].options].map((x) => x.textContent)), ['Vanilla 1.12.1', 'Outland (TBC) · 2.4.3', 'Wrath 3.3.5a']);
  await page.evaluate(() => window.LeylineNav.show('clients'));
  assert.deepEqual(await page.$$eval('#cl-list .card .name', (n) => n.map((x) => x.textContent)), ['Vanilla 1.12.1', 'Outland (TBC)', 'Wrath 3.3.5a']);
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), /Game 3\.3\.5a|\(12340\)/);
  assert.deepEqual(errors, []);
  await page.close();
});

test('switching official and community realms back and forth: PLAY each time starts the right client on the right realm', async () => {
  const { page, errors } = await open({ signed: true, clients: TWO.slice(0, 2) });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'realms_list') && document.querySelector('#pb-realm').textContent === 'Wrath Unleashed');
  const last = async (cmd) => (await callsOf(page, cmd)).slice(-1)[0];
  const playNow = async () => {
    const n = (await callsOf(page, 'play')).length;
    await page.click('#play-go');
    await page.waitForFunction((n) => window.__calls.filter(([c]) => c === 'play').length > n, n);
    await page.waitForFunction(() => /Started/.test(document.getElementById('play-status').textContent));
    return last('play');
  };
  // 1. the official realm, Wrath folder
  assert.deepEqual(await playNow(), { client: 'c335a-1', realm: OFFICIAL });
  // 2. Realms › Community › Old World (1.12.1): its card's Play takes the Vanilla folder, the bar follows
  await page.click('.nav[data-page="realms"]');
  await page.click('#rl-tab-community');
  await page.waitForSelector('#rl-list .realm-card');
  const card = (name) => page.locator('#rl-list .realm-card', { hasText: name }).locator('.rl-play');
  await card('Old World').click();
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play_realm'));
  assert.deepEqual(await last('play_realm'), { client: 'c1121-1', slug: 'c-dddddddddddddddd' });
  await page.waitForFunction(() => /its own accounts/.test(document.getElementById('rl-status').textContent));
  assert.match(await page.textContent('#rl-status'), /Old World has its own accounts: log in with yours there, or make one with Make an account\./);
  assert.equal(await page.textContent('#pb-realm'), 'Old World');
  assert.equal(await page.inputValue('#play-client'), 'c1121-1');
  // 3. PLAY in the bar: the same realm and client; the line says what login it needs
  await page.click('.nav[data-page="play"]');
  assert.deepEqual(await playNow(), { client: 'c1121-1', realm: 'relay.openvanilla.app:40020' });
  assert.equal(await page.textContent('#play-status'), 'Started Vanilla 1.12.1. Log in with your account there.', 'short, and says what login it needs');
  assert.match(await page.getAttribute('#play-status', 'title'), /^Started .* on relay\.openvanilla\.app:40020\.$/, 'the whole answer on hover');
  // 4. the picker: back to Wrath Unleashed: the Wrath folder again, the old line gone
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu .pb-realm-opt >> text=Wrath Unleashed');
  assert.equal(await page.textContent('#play-status'), '', 'no stale "Started … on relay…" line');
  assert.equal(await page.inputValue('#play-client'), 'c335a-1');
  assert.deepEqual(await last('select'), { client: 'c335a-1', realm: OFFICIAL });
  assert.deepEqual(await playNow(), { client: 'c335a-1', realm: OFFICIAL });
  assert.doesNotMatch(await page.textContent('#play-status'), /Log in/, 'the official realm signs in by itself');
  // 5. the picker: Old World again, under Your realms: the Vanilla folder again
  await page.click('#pb-realm-btn');
  await page.click('#pb-rmenu .pb-realm-opt >> text=Old World');
  assert.equal(await page.textContent('#play-status'), '');
  assert.equal(await page.inputValue('#play-client'), 'c1121-1');
  assert.deepEqual(await playNow(), { client: 'c1121-1', realm: 'relay.openvanilla.app:40020' });
  // 6. Realms › Official › Wrath Unleashed's Play, then Community › Busy Den (3.3.5a)
  await page.click('.nav[data-page="realms"]');
  await page.click('#rl-tab-official');
  assert.equal(await page.textContent('#rl-status'), '', 'the other tab\'s line went with it');
  await card('Wrath Unleashed').click();
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'play_realm').length === 2);
  assert.deepEqual(await last('play_realm'), { client: 'c335a-1', slug: 'official' });
  await page.click('#rl-tab-community');
  await card('Busy Den').click();
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'play_realm').length === 3);
  assert.deepEqual(await last('play_realm'), { client: 'c335a-1', slug: 'c-bbbbbbbbbbbbbbbb' });
  await page.click('.nav[data-page="play"]');
  assert.equal(await page.textContent('#pb-realm'), 'Busy Den');
  assert.deepEqual(await playNow(), { client: 'c335a-1', realm: 'relay.openvanilla.app:40002' });
  assert.deepEqual(errors, []);
  await page.close();
});

test('signed out, the realm card Play on the official realm signs in first, then starts', async () => {
  const { page, errors } = await open({ down: true });
  await page.waitForSelector('#gate-skip');
  await page.click('#gate-skip');
  await page.click('.nav[data-page="realms"]');
  await page.click('#rl-tab-official');
  await page.waitForSelector('#rl-list .realm-card .rl-play');
  await page.click('#rl-list .realm-card .rl-play');
  await page.waitForSelector('#gate-why');
  assert.deepEqual(await callsOf(page, 'play_realm'), []);
  await page.evaluate(() => { window.__up = true; });
  await page.click('#gate-retry');
  await page.click('#gate-browser');
  await page.waitForSelector('#gate-go', { timeout: 8000 });
  await page.click('#gate-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play_realm'));
  assert.deepEqual(await callsOf(page, 'play_realm'), [{ client: 'c335a-1', slug: 'official' }]);
  assert.deepEqual(errors, []);
  await page.close();
});
