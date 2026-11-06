// The sign-in dialog (ui/gate.js; owner 2 Nov 23:14: "no immediate login screen, but encouragement to log in top right
// ... a generated username and password ... as well as Google"; 23:18: the downloaded Hero Key file found by itself)
// and seamless realm switching (ui/app.js) in Chromium, with a fake Tauri core (owner
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
      case 'social_key_found': return window.__found || null;
      case 'social_key_file': window.__signed = true; return { ok: true, me: { pid: 'aaaaaaaaaa', name: 'Alice', tag: '4821' } };
      case 'save_text_file': window.__savedText = args.text; return true;
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
        if (args.action === 'password') { window.__pw = args.args.password; return { ok: true }; }
        if (args.action === 'herokey_saved') return { ok: true };
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

async function open(opts, extra, arg) {
  const page = await browser.newPage({ viewport: { width: 1400, height: 860 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.abort());
  await page.addInitScript(FAKE, opts || {});
  if (extra) await page.addInitScript(extra, arg);
  await page.goto(INDEX);
  return { page, errors };
}
const callsOf = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c).map(([, a]) => a), cmd);
const gateOpen = (page) => page.evaluate(() => !document.getElementById('gate').hidden);

test('signed out: the launcher opens and works; Log In top right invites; the dialog: browser, Google, name and password; no Discord', async () => {
  const { page, errors } = await open();
  await page.waitForSelector('#hf-signin');
  assert.equal(await gateOpen(page), false, 'no sign-in screen before the launcher');
  assert.equal(await page.evaluate(() => document.querySelector('.frame').inert), false, 'every button works');
  assert.match(await page.textContent('#hf-me'), /Play online, keep your progress/);
  await page.click('.nav[data-page="realms"]');   // the launcher is usable signed out
  assert.ok(await page.locator('#page-realms').evaluate((e) => e.classList.contains('on')));
  await page.click('#hf-signin');
  await page.waitForSelector('#gate-browser');
  assert.ok(await gateOpen(page));
  assert.match(await page.textContent('#gate-browser'), /Log in with your browser/);
  assert.doesNotMatch(await page.textContent('#gate'), /Sign in|Sign out|Signing in/, 'the word is Log in');
  assert.match(await page.textContent('#gate'), /Instant launcher recognition/);
  assert.match(await page.textContent('#gate-new'), /Create a guest account/);
  const text = await page.textContent('#gate');
  assert.doesNotMatch(text, /discord/i, 'no Discord sign-in, even when the website offers it');
  assert.doesNotMatch(text, /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i);
  assert.match(text, /Continue with Google/);
  assert.equal(await page.getAttribute('#gate-pwname', 'autocomplete'), 'username', 'a password manager fills it');
  assert.equal(await page.getAttribute('#gate-pw', 'autocomplete'), 'current-password');
  assert.equal(await page.locator('#gate-drop').count(), 0, 'the Hero Key file is folded away');
  // Not now closes; Esc too
  await page.click('#gate-close-dlg');
  assert.equal(await gateOpen(page), false);
  await page.click('#hf-signin');
  await page.keyboard.press('Escape');
  assert.equal(await gateOpen(page), false);
  // the browser sign-in
  await page.click('#hf-signin');
  await page.click('#gate-browser');
  await page.waitForSelector('#gate-code');
  assert.equal(await page.textContent('#gate-code'), 'BCDF-GHJK');
  assert.deepEqual(await callsOf(page, 'social_signin_start'), [{ via: null }], 'the website picks the way: a signed-in browser only approves');
  await page.waitForSelector('#gate-in', { timeout: 8000 });
  assert.equal(await page.textContent('#gate-in .gate-in-n'), 'Alice#4821');
  await page.click('#gate-go');
  assert.equal(await gateOpen(page), false);
  assert.equal(await page.textContent('#hf-me .hf-me-name'), 'Alice');
  assert.deepEqual(errors, []);
  await page.close();
});

test('Google, name and password, the Hero Key file or paste; a file without a key says so', async () => {
  const { page, errors } = await open();
  await page.click('#hf-signin');
  await page.waitForSelector('#gate-google');
  await page.click('#gate-google');
  await page.waitForSelector('#gate-cancel');
  assert.deepEqual(await callsOf(page, 'social_signin_start'), [{ via: 'google' }]);
  await page.click('#gate-cancel');
  // name and password: a name without its #tag is said before anything goes anywhere
  await page.fill('#gate-pwname', 'Alice');
  await page.fill('#gate-pw', 'secret-words-here');
  await page.click('#gate-pwgo');
  assert.match(await page.textContent('#gate-err'), /#tag/);
  assert.deepEqual(await callsOf(page, 'social_password_signin'), []);
  await page.click('#gate-keylink');
  await page.waitForSelector('#gate-key');
  await page.fill('#gate-key', 'hello');
  await page.click('#gate-keyform button[type=submit]');
  assert.match(await page.textContent('#gate-err'), /starts with ovk1/);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'gate-'));
  fs.writeFileSync(path.join(dir, 'notes.txt'), 'just some notes');
  await page.setInputFiles('#gate-file', path.join(dir, 'notes.txt'));
  await page.waitForFunction(() => /no Hero Key in that file/.test((document.getElementById('gate-err') || {}).textContent || ''));
  assert.deepEqual(await callsOf(page, 'social_key_signin'), []);
  fs.writeFileSync(path.join(dir, 'Leyline Hero Key.txt'), `Leyline Hero Key\r\n\r\nName: Alice#4821\r\nPassword: abcd-efgh-jkmn-pqrs\r\nKey: ${KEY}\r\n`);
  await page.setInputFiles('#gate-file', path.join(dir, 'Leyline Hero Key.txt'));
  await page.waitForSelector('#gate-in');
  assert.deepEqual(await callsOf(page, 'social_key_signin'), [{ key: KEY }], 'only the key goes to the core');
  await page.click('#gate-go');
  // Sign out: no screen comes back; Log In does
  await page.click('#hf-me-btn');
  await page.click('#hf-panel .ap-out');
  await page.waitForSelector('#hf-signin');
  assert.equal(await gateOpen(page), false);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Create a guest account: signed in and on at once; the login waits in Manage account until saved as a text file', async () => {
  const { page, errors } = await open();
  await page.click('#hf-signin');
  await page.click('#gate-new');
  await page.waitForSelector('#hf-me-btn');
  assert.equal(await gateOpen(page), false, 'nothing to read first: straight in');
  const pw = await page.evaluate(() => window.__pw);
  assert.match(pw, /^[a-z2-9]{4}(-[a-z2-9]{4}){3}$/, 'a password made for it, easy to read');
  assert.equal(await page.textContent('#hf-me .hf-me-name'), 'Brightfeather42');
  assert.match(await page.textContent('#gate-toast'), /You're in as Brightfeather42#1234\. Save your login any time: Manage account/);
  assert.equal(await page.locator('#page-account').evaluate((e) => e.classList.contains('on')), false, 'no trip to Manage account');
  await page.click('#gate-toast');   // the reminder opens Manage account › Your login
  await page.waitForSelector('#ac-guest');
  assert.equal(await page.textContent('#ac-guest-name'), 'Brightfeather42#1234');
  assert.doesNotMatch(await page.textContent('#ac-guest'), new RegExp(pw), 'hidden until Show');
  await page.click('#ac-guest-show');
  assert.equal(await page.textContent('#ac-guest-pass'), pw);
  await page.click('#ac-guest-save');
  await page.waitForFunction(() => !!window.__savedText);
  const t = await page.evaluate(() => window.__savedText);
  assert.match(t, /Name: Brightfeather42#1234/);
  assert.ok(t.includes(`Password: ${pw}`) && t.includes('Key: ovk1.NewNew1234.'));
  await page.waitForFunction(() => !document.getElementById('ac-guest'));
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.guestLogin')), null, 'forgotten on this PC once saved');
  assert.deepEqual(errors, []);
  await page.close();
});

test('a Hero Key file downloaded: signed out at the start, it signs in by itself; one from before Sign out is not used', async () => {
  const { page, errors } = await open({}, () => { window.__found = { path: 'C:\\Users\\me\\Downloads\\Leyline Hero Key.txt', mtime_ms: Date.now(), name: 'Alice#4821' }; });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'social_key_file'));
  await page.waitForSelector('#hf-me-btn');
  assert.match(await page.textContent('#gate-toast'), /Logged in as Alice#4821 with the Hero Key you downloaded/);
  assert.deepEqual(await callsOf(page, 'social_key_file'), [{ path: 'C:\\Users\\me\\Downloads\\Leyline Hero Key.txt' }]);
  // Sign out: the same file is not used again, the dialog checks for a new one while open
  await page.click('#hf-me-btn');
  await page.click('#hf-panel .ap-out');
  await page.waitForSelector('#hf-signin');
  await page.click('#hf-signin');
  await page.waitForTimeout(3500);
  assert.equal((await callsOf(page, 'social_key_file')).length, 1);
  await page.evaluate(() => { window.__found = { path: 'C:\\Users\\me\\Downloads\\Leyline Hero Key (1).txt', mtime_ms: Date.now() + 5, name: 'Alice#4821' }; });
  await page.waitForSelector('#gate-in', { timeout: 8000 });
  assert.equal((await callsOf(page, 'social_key_file')).length, 2, 'a newly downloaded one signs in');
  assert.deepEqual(errors, []);
  await page.close();
});

test('a sign-in kept from before: straight in, offline too (never locked out)', async () => {
  const { page, errors } = await open({ signed: true, offline: true });
  await page.waitForSelector('#hf-retry');
  assert.equal(await gateOpen(page), false);
  assert.equal(await page.textContent('#hf-me .hf-me-name'), 'Logged in');
  assert.match(await page.textContent('#hf-me .hf-me-st'), /Offline/);
  assert.equal(await page.locator('#hf-signin').count(), 0, 'no Log In: the sign-in is kept');
  assert.deepEqual(errors, []);
  await page.close();
});

test('the website down: said plainly in the dialog; PLAY on the official realm opens sign-in, then goes on', async () => {
  const { page, errors } = await open({ down: true });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'realms_list'));
  assert.equal(await gateOpen(page), false);
  await page.click('#play-go');
  await page.waitForSelector('#gate-why');
  assert.match(await page.textContent('#gate-why'), /Wrath Unleashed signs you in with your Leyline account/);
  assert.match(await page.textContent('#gate-down'), /website isn't answering/);
  assert.deepEqual(await callsOf(page, 'play'), [], 'nothing started');
  await page.evaluate(() => { window.__up = true; });
  await page.click('#gate-retry');
  await page.click('#gate-keylink');
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

// ------------------------------------------------------------------------------- a new machine, in as few clicks as can be
// (owner 3 Nov: "hero key intuitive, few steps as possible to get everywhere. Can log in and out ... everything makes
// sense on new machine"). Each way in counts the clicks in the launcher (a browser's own approve and a file picker not
// counted); the numbers are the most each may take, so a change that adds a step fails here.
function counter(page) {
  let n = 0;
  return { click: async (sel) => { n += 1; await page.click(sel); }, get n() { return n; }, reset() { n = 0; } };
}
const KEPT = { path: 'C:\\Users\\me\\Downloads\\Leyline Hero Key.txt', name: 'Alice#4821' };

test('back on the same PC after Log out: the kept Hero Key file is one click, "Log in as Alice#4821" (never by itself)', async () => {
  const { page, errors } = await open({}, (k) => {
    localStorage.setItem('leyline.signedOutAt', String(Date.now()));
    window.__found = { path: k.path, name: k.name, mtime_ms: Date.now() - 60000 };
  }, KEPT);
  await page.waitForSelector('#hf-signin');
  await page.waitForTimeout(500);
  assert.deepEqual(await callsOf(page, 'social_key_file'), [], 'a file from before the Log out is not used by itself');
  const c = counter(page);
  await c.click('#hf-signin');
  await page.waitForSelector('#gate-again');
  assert.equal(await page.textContent('#gate-again'), 'Log in as Alice#4821');
  assert.match(await page.textContent('#gate-pane'), /With Leyline Hero Key\.txt from your Downloads/);
  assert.ok(await page.evaluate(() => document.activeElement && document.activeElement.id === 'gate-again'), 'it has the focus: Enter logs in');
  assert.equal(await page.getAttribute('#gate-browser', 'class'), 'btn gate-big', 'one primary button');
  await c.click('#gate-again');
  await page.waitForSelector('#hf-me-btn');
  assert.equal(c.n, 2, 'Log In, Log in as …');
  assert.deepEqual(await callsOf(page, 'social_key_file'), [{ path: KEPT.path }]);
  assert.deepEqual(errors, []);
  await page.close();
});

test('no kept key file: no "Log in as", the browser way is the primary button', async () => {
  const { page, errors } = await open();
  await page.click('#hf-signin');
  await page.waitForSelector('#gate-browser');
  await page.waitForTimeout(300);
  assert.equal(await page.locator('#gate-again').count(), 0);
  assert.equal(await page.getAttribute('#gate-browser', 'class'), 'btn primary gate-big');
  assert.deepEqual(errors, []);
  await page.close();
});

test('new machine click counts: browser 2, guest 2, key file in Downloads 0, log out 2, guest login saved 3, PLAY signed out 2', async () => {
  // the browser: Log In, Log in with your browser (then approve in the browser); in by itself
  let { page, errors } = await open();
  let c = counter(page);
  await c.click('#hf-signin');
  await c.click('#gate-browser');
  await page.waitForSelector('#hf-me-btn', { timeout: 8000 });
  assert.equal(c.n, 2, 'browser');
  // Log out: the chip, Log out
  c.reset();
  await c.click('#hf-me-btn');
  await c.click('#hf-panel .ap-out');
  await page.waitForSelector('#hf-signin');
  assert.equal(c.n, 2, 'log out');
  assert.deepEqual(errors, []);
  await page.close();

  // a guest account: Log In, Create a guest account; then its login saved: the chip, Hero Key, Save as text file
  ({ page, errors } = await open());
  c = counter(page);
  await c.click('#hf-signin');
  await c.click('#gate-new');
  await page.waitForSelector('#hf-me-btn');
  assert.equal(c.n, 2, 'guest');
  c.reset();
  await c.click('#hf-me-btn');
  await c.click('#hf-panel [role="menuitem"]:has-text("Hero Key")');
  await page.waitForSelector('#ac-guest-save');
  await c.click('#ac-guest-save');
  await page.waitForFunction(() => !!window.__savedText);
  assert.equal(c.n, 3, 'guest login saved as a text file');
  assert.match(await page.evaluate(() => window.__savedText), /Log in to the Leyline launcher/);
  assert.deepEqual(errors, []);
  await page.close();

  // the Hero Key file in Downloads (the website's Create a Hero Key saved it): no click at all
  ({ page, errors } = await open({}, (k) => { window.__found = { path: k.path, name: k.name, mtime_ms: Date.now() }; }, KEPT));
  await page.waitForSelector('#hf-me-btn');
  assert.deepEqual(await callsOf(page, 'social_key_file'), [{ path: KEPT.path }], 'key file: 0 clicks');
  assert.deepEqual(errors, []);
  await page.close();

  // PLAY on the official realm signed out, a key file kept from before: PLAY, Log in as …, and the game starts
  ({ page, errors } = await open({}, (k) => {
    localStorage.setItem('leyline.signedOutAt', String(Date.now()));
    window.__found = { path: k.path, name: k.name, mtime_ms: Date.now() - 60000 };
  }, KEPT));
  await page.waitForFunction(() => window.__calls.some(([x]) => x === 'realms_list'));
  c = counter(page);
  await c.click('#play-go');
  await page.waitForSelector('#gate-again');
  await c.click('#gate-again');
  await page.waitForFunction(() => window.__calls.some(([x]) => x === 'play'), null, { timeout: 8000 });
  assert.equal(c.n, 2, 'PLAY signed out');
  assert.deepEqual(await callsOf(page, 'play'), [{ client: 'c335a-1', realm: OFFICIAL }]);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Settings › Your account: signed out it opens Log in; a guest is told to save the login and lands on it; the P2P part is last', async () => {
  const { page, errors } = await open();
  await page.click('.nav[data-page="settings"]');
  await page.waitForSelector('#page-settings.on');
  assert.match(await page.textContent('#set-acct-line'), /Not logged in/);
  const first = await page.evaluate(() => document.querySelector('#page-settings .panel h3').textContent);
  assert.equal(first, 'Your account', 'Settings opens on the account, not the advanced peer-to-peer part');
  await page.click('#set-acct');
  await page.waitForSelector('#gate-new');
  await page.click('#gate-new');
  await page.waitForSelector('#hf-me-btn');
  await page.click('.nav[data-page="settings"]');
  await page.waitForFunction(() => /isn't saved yet/.test(document.getElementById('set-acct-line').textContent));
  assert.equal(await page.textContent('#set-acct'), 'Save your login');
  // guest → its login, from Settings: Your account, Save as text file (2 clicks from Settings)
  await page.click('#set-acct');
  await page.waitForSelector('#ac-guest-save');
  await page.click('#ac-guest-save');
  await page.waitForFunction(() => !!window.__savedText);
  await page.click('.nav[data-page="settings"]');
  await page.waitForFunction(() => /Logged in as Brightfeather42#1234/.test(document.getElementById('set-acct-line').textContent));
  assert.equal(await page.textContent('#set-acct'), 'Your login and Hero Key');
  assert.deepEqual(errors, []);
  await page.close();
});
