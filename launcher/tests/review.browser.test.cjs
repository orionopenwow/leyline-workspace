// Review tests (launcher review, 4 Nov 2026): edge cases around the sign-in dialog (ui/gate.js), guest accounts
// (gate.js createGuest, account.js, social.js signOut), PLAY (ui/app.js play-form, playRealm) and the words on every
// page, in Chromium with the same fake Tauri core as gate.browser.test.cjs.
//
//   NODE_PATH=/opt/npm-tools/node_modules node --test launcher/tests/review.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
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
      case 'social_password_signin': window.__signed = true; return { ok: true, me: { pid: 'aaaaaaaaaa', name: 'Alice', tag: '4821' } };
      case 'save_text_file': window.__savedText = args.text; return true;
      case 'social_key_signin': if (!args.key.includes(KEY) && !args.key.startsWith('ovk1.NewNew1234.')) throw "That Hero Key didn't work. It may have been replaced by a newer one: use your newest key.";
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

// ---- tests
const BRAND = /world\s*of\s*warcraft|warcraft|lich\s*king|(?<!endorsed by )blizzard|\bwow\b/i;
const MONEY = /donat|patreon|ko-fi|paypal|tip jar|buy me a coffee|support us/i;

test('website down, signed out: "Continue without logging in" then PLAY again does not open the same dialog again', async () => {
  // A player whose website is down presses PLAY, reads "Continue without logging in" and takes it. Pressing PLAY a
  // second time should not trap them in the same dialog: they asked to go on without logging in.
  const { page, errors } = await open({ down: true });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'realms_list'));
  await page.click('#play-go');
  await page.waitForSelector('#gate-skip');
  await page.click('#gate-skip');
  assert.equal(await gateOpen(page), false);
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play'));
  assert.equal((await callsOf(page, 'play')).length, 1, 'the PLAY that opened the dialog goes on (the game opens at its login screen)');
  await page.click('#play-go');   // waits out the short hold after a start
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'play').length === 2);
  assert.equal(await gateOpen(page), false, 'the dialog the player just skipped does not come back');
  assert.deepEqual(errors, []);
  await page.close();
});

test('an unsaved guest who logs out can get back in: the dialog offers "Log in as <name>"', async () => {
  // Create a guest account, never save its login, Log out: before the fix the account was unreachable (its key and
  // password sat in localStorage, shown nowhere signed out).
  const { page, errors } = await open();
  await page.click('#hf-signin');
  await page.click('#gate-new');
  await page.waitForSelector('#hf-me-btn');
  await page.click('#hf-me-btn');
  await page.click('#hf-panel .ap-out');
  // Log out asks first while the guest's login isn't saved (review 4 Nov): Cancel keeps you in, Save my login opens
  // Manage account, Log out anyway goes on
  await page.waitForSelector('#so-dlg[open]');
  assert.match(await page.textContent('#so-dlg-p'), /Brightfeather42#1234 has no saved login yet/);
  assert.equal(await page.evaluate(() => document.activeElement.id), 'so-save', 'Save my login has the focus');
  await page.keyboard.press('Escape');
  assert.equal(await page.isVisible('#hf-me-btn'), true, 'Esc: still logged in');
  assert.equal((await callsOf(page, 'social_signout')).length, 0);
  await page.click('#hf-me-btn');
  await page.click('#hf-panel .ap-out');
  await page.click('#so-save');
  await page.waitForSelector('#page-account.on');
  assert.equal((await callsOf(page, 'social_signout')).length, 0, 'Save my login: not logged out');
  await page.click('#hf-me-btn');
  await page.click('#hf-panel .ap-out');
  await page.click('#so-out');
  await page.waitForSelector('#hf-signin');
  await page.click('#hf-signin');
  await page.waitForSelector('#gate-guest-again');
  assert.equal(await page.textContent('#gate-guest-again'), 'Log in as Brightfeather42#1234');
  await page.click('#gate-guest-again');
  await page.waitForSelector('#hf-me-btn');
  const k = await callsOf(page, 'social_key_signin');
  assert.equal(k.length, 1);
  assert.match(k[0].key, /^ovk1\.NewNew1234\./, 'its own Hero Key signs it in');
  assert.deepEqual(errors, []);
  await page.close();
});

test('another account signs in: an unsaved guest login is not offered to them, and is kept for its owner', async () => {
  const { page, errors } = await open({}, () => {
    localStorage.setItem('leyline.guestLogin', JSON.stringify({ name: 'Brightfeather42#1234', pass: 'abcd-efgh-jkmn-pqrs', key: 'ovk1.NewNew1234.' + 'n'.repeat(43), at: 1 }));
  });
  await page.click('#hf-signin');
  await page.waitForSelector('#gate-browser');
  assert.equal(await page.locator('#gate-guest-again').count(), 1, 'offered while signed out');
  await page.click('#gate-keylink');
  await page.fill('#gate-key', KEY);
  await page.click('#gate-keyform button[type=submit]');
  await page.waitForSelector('#gate-go');
  await page.click('#gate-go');
  await page.evaluate(() => window.LeylineNav.show('account'));
  await page.waitForTimeout(300);
  assert.equal(await page.locator('#ac-guest').count(), 0, "Alice never sees the guest's password");
  assert.notEqual(await page.evaluate(() => localStorage.getItem('leyline.guestLogin')), null, 'kept for the guest');
  assert.deepEqual(errors, []);
  await page.close();
});

test('a double click on PLAY starts the game once', async () => {
  const { page, errors } = await open({ signed: true });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'realms_list') && document.querySelector('#hf-me-btn'));
  await page.dblclick('#play-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'play'));
  if (process.env.DBG) console.log(JSON.stringify(await page.evaluate(() => window.__calls.map(([c]) => c).slice(-14))));
  await page.waitForTimeout(400);
  assert.equal((await callsOf(page, 'play')).length, 1, 'one game, not two');
  assert.deepEqual(errors, []);
  await page.close();
});

test('closing the dialog drops the PLAY that opened it: a later Log in starts nothing by itself', async () => {
  const { page, errors } = await open();
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'realms_list'));
  await page.click('#play-go');
  await page.waitForSelector('#gate-why');
  await page.mouse.click(5, 400);   // beside the card
  assert.equal(await gateOpen(page), false);
  await page.click('#hf-signin');
  assert.equal(await page.locator('#gate-why').count(), 0, 'the reason line went with the PLAY');
  await page.click('#gate-keylink');
  await page.fill('#gate-key', KEY);
  await page.click('#gate-keyform button[type=submit]');
  await page.waitForSelector('#gate-go');
  assert.equal(await page.textContent('#gate-go'), 'Continue', 'not "Continue to PLAY"');
  await page.click('#gate-go');
  await page.waitForTimeout(300);
  assert.deepEqual(await callsOf(page, 'play'), [], 'nothing started');
  assert.deepEqual(errors, []);
  await page.close();
});

test('Esc while the browser code waits cancels the wait and keeps the dialog; a second Esc closes it', async () => {
  const { page, errors } = await open({}, () => { window.__poll = 'pending'; });
  await page.click('#hf-signin');
  await page.click('#gate-browser');
  await page.waitForSelector('#gate-code');
  await page.keyboard.press('Escape');
  await page.waitForSelector('#gate-browser');
  assert.equal(await gateOpen(page), true);
  assert.ok((await callsOf(page, 'social_signin_cancel')).length >= 1, 'the wait is cancelled in the core too');
  await page.keyboard.press('Escape');
  assert.equal(await gateOpen(page), false);
  assert.deepEqual(errors, []);
  await page.close();
});

test('the made password: 4 groups of 4 from letters and digits that cannot be mistaken, never repeated', async () => {
  const { page, errors } = await open();
  const pws = await page.evaluate(() => Array.from({ length: 300 }, () => window.LeylineGate.makePassword()));
  for (const p of pws) assert.match(p, /^[a-hjkmnp-z2-9]{4}(-[a-hjkmnp-z2-9]{4}){3}$/);
  assert.equal(new Set(pws).size, pws.length);
  assert.ok(!pws.join('').match(/[01ilo]/), 'no 0, 1, i, l or o');
  assert.deepEqual(errors, []);
  await page.close();
});

test('every page, signed in: no brand words, no money links, no raw errors', async () => {
  const { page, errors } = await open({ signed: true });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'realms_list') && document.querySelector('#hf-me-btn'));
  const pages = await page.$$eval('.nav[data-page]', (n) => [...new Set(n.map((x) => x.dataset.page))]);
  assert.ok(pages.length >= 6, 'the side menu has its pages: ' + pages.join(','));
  for (const p of pages) {
    await page.evaluate((id) => window.LeylineNav.show(id), p);
    await page.waitForTimeout(150);
    const t = await page.evaluate(() => document.body.innerText);
    assert.doesNotMatch(t, BRAND, `brand word on ${p}`);
    assert.doesNotMatch(t, MONEY, `money link on ${p}`);
    assert.doesNotMatch(t, /unknown [a-z_]+|undefined|\[object Object\]|NaN/, `raw text on ${p}`);
  }
  const hrefs = await page.$$eval('a[href]', (a) => a.map((x) => x.href));
  for (const h of hrefs) assert.doesNotMatch(h, MONEY, h);
  assert.deepEqual(errors, []);
  await page.close();
});

test('a technical error from the core never shows raw in the Log in dialog', async () => {
  const RAW = 'error sending request for url (https://play.openvanilla.app/wrath/api/herokey): tls handshake eof';
  const { page, errors } = await open({}, (raw) => {
    const inv = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = async (cmd, args) => { if (cmd === 'social_key_signin') { window.__calls.push([cmd, args]); throw raw; } return inv(cmd, args); };
  }, RAW);
  await page.click('#hf-signin');
  await page.click('#gate-keylink');
  await page.fill('#gate-key', KEY);
  await page.click('#gate-keyform button[type=submit]');
  // a network failure reads as the website not answering: its box (with Continue without logging in), never the raw text
  await page.waitForSelector('#gate-err, #gate-down');
  const t = await page.textContent('#gate-pane');
  assert.doesNotMatch(t, /error sending|https?:|tls/i, t);
  assert.match(t, /Try again/);
  assert.equal(await page.getAttribute('#gate-key', 'aria-describedby'), await page.$('#gate-down') ? 'gate-down' : 'gate-err', 'the field points at the line');
  assert.deepEqual(errors, []);
  await page.close();
});

// KNOWN FAILURE (owner decision, not fixed): a player's own game folder path is shown as it is under Game folders ›
// Details ("Folder: C:\Games\World of Warcraft 3.3.5a"), so a brand word reaches the screen. Masking it (e.g. "C:\Games\…")
// changes what the player recognises as their folder, so it is left for Orion. ui/app.js renderClients ("Folder: " + c.path).
test('Game folders: a folder path with a brand word in it does not show the brand word', { todo: 'owner decision: mask brand words in shown folder paths (app.js renderClients)' }, async () => {
  const clients = [{ id: 'c335a-1', name: 'Wrath 3.3.5a', path: 'C:\\Games\\World of Warcraft 3.3.5a', build: '3.3.5a', realmlist: '' }];
  const { page, errors } = await open({ signed: true, clients });
  await page.waitForFunction(() => document.querySelector('#hf-me-btn'));
  await page.evaluate(() => window.LeylineNav.show('clients'));
  await page.waitForSelector('#cl-list .card');
  await page.$$eval('#cl-list details', (d) => d.forEach((x) => { x.open = true; }));
  assert.doesNotMatch(await page.textContent('#cl-list'), BRAND);
  assert.deepEqual(errors, []);
  await page.close();
});

test('PLAY pressed before the sign-in check answers never starts the official realm signed out', async () => {
  // social_status is slow on a cold start (the keychain); a fast PLAY used to see "no way to tell" and start at once
  const { page, errors } = await open({}, () => {
    const inv = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = async (cmd, args) => {
      if (cmd === 'social_status') { await new Promise((r) => setTimeout(r, 1500)); }
      return inv(cmd, args);
    };
  });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'realms_list'));
  await page.click('#play-go');
  await page.waitForTimeout(2200);
  assert.deepEqual(await callsOf(page, 'play'), [], 'nothing started signed out');
  assert.equal(await gateOpen(page), true, 'the Log in dialog opened once the check said signed out');
  assert.deepEqual(errors, []);
  await page.close();
});

// ---- the fixes after the review (4 Nov, agent gr-fix-launcher)

test('a realm card started a game: the rebuilt card\'s Play and the bar\'s PLAY are held a moment (one game, not two)', async () => {
  const { page, errors } = await open({ signed: true, clients: [{ id: 'c335a-1', name: 'My Wrath', path: '/g/wrath', build: '3.3.5a', realmlist: '' }] });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'realms_list'));
  await page.click('.nav[data-page="realms"]');
  await page.click('#rl-tab-community');
  const play = () => page.locator('#rl-list .realm-card', { hasText: 'Busy Den' }).locator('.rl-play');
  await play().click();
  await page.waitForFunction(() => /Started/.test(document.getElementById('rl-status').textContent));
  // the cards were rebuilt after the start: the new card's Play is off, and a forced click starts nothing
  assert.equal(await play().isDisabled(), true, 'the rebuilt card is held too');
  await play().click({ force: true });
  assert.equal(await page.isDisabled('#play-go'), true, 'the bar\'s PLAY is held as well');
  await page.waitForTimeout(300);
  assert.equal((await callsOf(page, 'play_realm')).length, 1, 'one game');
  await page.waitForFunction(() => !document.querySelector('#play-go').disabled, null, { timeout: 5000 });
  assert.equal(await play().isDisabled(), false, 'Play comes back after the hold');
  assert.deepEqual(errors, []);
  await page.close();
});

test('the Log in dialog: a pasted key is cleaned or refused, the name stays after a refused password, Enter moves on', async () => {
  const { page, errors } = await open({}, () => {
    const inv = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = async (cmd, args) => {
      if (cmd === 'social_password_signin') { window.__calls.push([cmd, args]); throw "That name and password didn't match."; }
      if (cmd === 'social_key_signin' && !/^ovk1\.[A-Za-z0-9._-]+$/.test(args.key)) { window.__calls.push([cmd, args]); throw 'bad key'; }
      return inv(cmd, args);
    };
  });
  await page.click('#hf-signin');
  // Enter on the name goes to the password while it is empty
  await page.fill('#gate-pwname', 'Alice#4821');
  await page.press('#gate-pwname', 'Enter');
  assert.equal(await page.evaluate(() => document.activeElement.id), 'gate-pw');
  await page.fill('#gate-pw', 'wrong-password');
  await page.press('#gate-pw', 'Enter');
  await page.waitForSelector('#gate-err');
  assert.match(await page.textContent('#gate-err'), /didn't match/);
  assert.equal(await page.inputValue('#gate-pwname'), 'Alice#4821', 'the name is still there');
  assert.equal(await page.getAttribute('#gate-pw', 'aria-describedby'), 'gate-err');
  assert.equal(await page.getAttribute('#gate-pw', 'aria-invalid'), 'true');
  // a key that isn't the strict shape: spaces and an invisible character come out before it is sent
  await page.click('#gate-keylink');
  await page.fill('#gate-key', 'ovk1.Abc d​.short');
  await page.click('#gate-keyform button[type=submit]');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'social_key_signin'));
  assert.equal((await callsOf(page, 'social_key_signin'))[0].key, 'ovk1.Abcd.short', 'sent cleaned');
  // not a key at all: refused here, nothing sent
  await page.fill('#gate-key', 'ovk1. hello there!');
  await page.click('#gate-keyform button[type=submit]');
  await page.waitForFunction(() => /starts with ovk1/.test((document.getElementById('gate-err') || {}).textContent || ''));
  assert.equal((await callsOf(page, 'social_key_signin')).length, 1);
  assert.deepEqual(errors, []);
  await page.close();
});

test('the Log in dialog stops looking in Downloads after five minutes open, and looks again when the window is back', async () => {
  const page = await browser.newPage({ viewport: { width: 1400, height: 860 } });
  await page.clock.install();
  await page.route('https://play.openvanilla.app/**', (r) => r.abort());
  await page.addInitScript(FAKE, {});
  await page.goto(INDEX);
  await page.clock.runFor(2000);
  await page.click('#hf-signin');
  const looks = () => page.evaluate(() => window.__calls.filter(([c]) => c === 'social_key_found').length);
  await page.clock.runFor(60 * 1000);
  const minute = await looks();
  assert.ok(minute >= 15, `looks every few seconds at first (${minute})`);
  await page.clock.runFor(5 * 60 * 1000);
  const after = await looks();
  await page.clock.runFor(5 * 60 * 1000);
  assert.equal(await looks(), after, 'then stops');
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await page.clock.runFor(10 * 1000);
  assert.ok(await looks() > after, 'the window back: it looks again');
  await page.close();
});

test('Settings: the parts to jump to, About with the legal line, and it opens at its top every time', async () => {
  const { page, errors } = await open({ signed: true });
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'get_state'));
  await page.click('.nav[data-page="settings"]');
  await page.waitForSelector('#set-jump .set-jump-b');
  const jumps = await page.$$eval('#set-jump .set-jump-b', (b) => b.map((x) => x.textContent));
  assert.equal(jumps[0], 'Your account');
  assert.ok(jumps.includes('About') && jumps.includes('Advanced'), jumps.join(', '));
  assert.ok(!jumps.includes('Built in') && !jumps.includes('Installed'), 'only the parts, not the boxes inside them');
  assert.equal(await page.textContent('#set-legal'), 'Leyline is not affiliated with or endorsed by Blizzard Entertainment.');
  const order = await page.$$eval('#page-settings .panel > *', (k) => k.map((x) => x.id).filter(Boolean));
  assert.ok(order.indexOf('p2p-adv') < order.indexOf('set-about') && order.indexOf('set-about') === order.length - 1, 'Advanced, then About last');
  await page.click('.set-jump-b:text-is("About")');
  await page.waitForFunction(() => document.querySelector('main').scrollTop > 100);
  await page.waitForTimeout(1000);   // the smooth scroll ends
  await page.click('.nav[data-page="settings"]');
  await page.waitForTimeout(100);
  assert.equal(await page.evaluate(() => document.querySelector('main').scrollTop), 0, 'Settings again: at its top, not on Advanced');
  assert.deepEqual(errors, []);
  await page.close();
});
