// Friends on the Leyline network (ui/friends.js; app.rs friends_*; docs/LEYLINE-FRIENDS.md), in Chromium with a fake
// Tauri core:
//   * mount(el) and data-leyline-friends draw the section; friends_start once;
//   * my friend code: shown and copied; pasting a code sends friends_add and says which way it went;
//   * requests: Accept / Decline / Cancel call friends_answer with the right action;
//   * people near: Add asks friends_ask_near;
//   * friends online first, what they play, last seen; Invite sends the chosen world; Join opens the usual Join card
//     (LeylineSolo.offerJoin); Remove asks first;
//   * the profile saves name, status, sharing, network visibility and the mailbox;
//   * an error from the core is said in plain words; no game or publisher names anywhere.
//
//   node --test launcher/tests/friends.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const UI = path.join(__dirname, '..', 'ui');
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard|3\.3\.5|1\.12/i;
const NOW = Math.floor(Date.now() / 1000);
const STATE = {
  me: { peer_id: 'mmmmmmmmmmmmmmmm', profile: { name: 'Ari', status: '', share_playing: true, lan: 'friends', mailbox: true, relay: 'relay.leylinelauncher.com', worlds: [], seq: 1 } },
  friends: [
    { peer_id: 'bbbbbbbbbbbbbbbb', name: 'Bo', status: 'around tonight', online: true, near: false, last_seen: NOW - 30, playing: 'Single player', open: null, invite: { world: { kind: 'single', name: 'Single player' }, ts: NOW - 60 }, worlds: [], since: NOW - 999 },
    { peer_id: 'cccccccccccccccc', name: '', status: '', online: false, near: false, last_seen: NOW - 3 * 3600, playing: null, open: null, invite: null, worlds: [], since: NOW - 999 },
  ],
  incoming: [{ peer_id: 'dddddddddddddddd', name: 'Dee', status: '', ts: NOW - 5, via: 'code' }],
  outgoing: [{ peer_id: 'eeeeeeeeeeeeeeee', ts: NOW - 50 }],
  near: [{ peer_id: 'ffffffffffffffff', name: 'Fay', asked: false }],
  net: { running: true, lan_port: 41190, tcp_port: 41191, mailbox: 'ok', last_error: '', notices: [] },
};

const FAKE = (o) => {
  window.__calls = [];
  window.__clip = null;
  try { Object.defineProperty(navigator, 'clipboard', { value: { writeText: async (t) => { window.__clip = t; } }, configurable: true }); } catch (_) { /* fine */ }
  window.__joined = null;
  window.LeylineSolo = { offerJoin: (l) => { window.__joined = l; } };
  window.confirm = () => true;
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    window.__calls.push([cmd, args]);
    if (o.fail && o.fail[cmd]) throw o.fail[cmd];
    switch (cmd) {
      case 'friends_start': return o.state.net;
      case 'friends_state': return JSON.parse(JSON.stringify(o.state));
      case 'friends_code': return 'LF1-' + 'a'.repeat(60) + '@relay.leylinelauncher.com';
      case 'friends_add': return 'relay';
      case 'friends_answer': return 'lan';
      case 'friends_ask_near': return 'lan';
      case 'friends_invite': return 'relay';
      case 'friends_join': return 'https://relay.leylinelauncher.com/join/abcdefghijk2345a#SECRET~~G1.pw';
      case 'friends_profile': return null;
      default: throw 'unknown command ' + cmd;
    }
  } } };
};

async function open(o = {}) {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, { state: o.state || STATE, fail: o.fail || null });
  await page.goto('about:blank');
  await page.setContent('<!doctype html><html><body><div id="auto" data-leyline-friends></div><div id="other"></div></body></html>');
  await page.addStyleTag({ path: path.join(UI, 'friends.css') });
  await page.addScriptTag({ path: path.join(UI, 'friends.js') });
  await page.waitForSelector('#auto .lf-friend');
  return { browser, page, errors };
}

const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter((x) => x[0] === c).map((x) => x[1]), cmd);

test('the section: friends online first, requests, people near, no brand words', async () => {
  const { browser, page, errors } = await open();
  try {
    const names = await page.$$eval('#auto .lf-friend .lf-name', (e) => e.map((x) => x.textContent));
    assert.deepEqual(names, ['Bo', 'Hero CCCC']);
    const subs = await page.$$eval('#auto .lf-friend .lf-sub', (e) => e.map((x) => x.textContent));
    assert.match(subs[0], /Playing Single player · around tonight/);
    assert.match(subs[1], /Last seen 3 hours ago/);
    assert.equal(await page.$$eval('#auto .lf-friend.on', (e) => e.length), 1);
    assert.match(await page.textContent('#auto .lf-list > b'), /1 online/);
    assert.equal(await page.$$eval('#auto .lf-join', (e) => e.length), 1, 'Join only where there is an invite');
    assert.equal(await page.textContent('#auto .lf-req .lf-name'), 'Dee');
    assert.equal(await page.textContent('#auto .lf-near-row .lf-name'), 'Fay');
    assert.equal((await calls(page, 'friends_start')).length, 1, 'started once');
    // mount() on another element draws there too, and twice is once
    await page.evaluate(() => { window.LeylineFriends.mount(document.getElementById('other')); window.LeylineFriends.mount(document.getElementById('other')); });
    await page.waitForSelector('#other .lf-friend');
    assert.equal(await page.$$eval('#other .lf-head', (e) => e.length), 1);
    assert.doesNotMatch(await page.textContent('body'), BRANDS);
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});

test('friend codes: mine shown and copied, a pasted one added', async () => {
  const { browser, page } = await open();
  try {
    await page.click('#auto .lf-show-code');
    await page.waitForSelector('#auto .lf-code:not([hidden])');
    assert.match(await page.evaluate(() => window.__clip), /^LF1-/);
    assert.match(await page.textContent('#auto .lf-status'), /copied.*a day/);
    await page.click('#auto .lf-add-go');
    assert.match(await page.textContent('#auto .lf-status'), /Paste your friend's code first/);
    await page.fill('#auto .lf-paste', '  LF1-xyz  ');
    await page.click('#auto .lf-add-go');
    await page.waitForFunction(() => /Request sent/.test(document.querySelector('#auto .lf-status').textContent));
    assert.deepEqual(await calls(page, 'friends_add'), [{ code: 'LF1-xyz' }]);
    assert.match(await page.textContent('#auto .lf-status'), /mailbox/);
  } finally { await browser.close(); }
});

test('requests and people near: accept, decline, cancel, add', async () => {
  const { browser, page } = await open();
  try {
    await page.click('#auto .lf-accept');
    await page.waitForFunction(() => /are friends/.test(document.querySelector('#auto .lf-status').textContent));
    await page.click('#auto .lf-decline');
    await page.click('#auto .lf-cancel');
    await page.click('#auto .lf-near-add');
    await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'friends_ask_near'));
    assert.deepEqual((await calls(page, 'friends_answer')).map((a) => a.action + ':' + a.peerId),
      ['accept:dddddddddddddddd', 'decline:dddddddddddddddd', 'cancel:eeeeeeeeeeeeeeee']);
    assert.deepEqual(await calls(page, 'friends_ask_near'), [{ peerId: 'ffffffffffffffff' }]);
  } finally { await browser.close(); }
});

test('invite to my world, join theirs, remove', async () => {
  const { browser, page } = await open();
  try {
    const row = '#auto .lf-friend[data-peer="cccccccccccccccc"]';
    await page.selectOption(`${row} .lf-world`, 'vanilla');
    await page.click(`${row} .lf-invite`);
    await page.waitForFunction(() => /Invite sent/.test(document.querySelector('#auto .lf-status').textContent));
    assert.deepEqual(await calls(page, 'friends_invite'), [{ peerId: 'cccccccccccccccc', kind: 'vanilla' }]);
    assert.match(await page.textContent('#auto .lf-status'), /their character stays on your world for next time/, 'a lasting login, said');
    // the pick survives the list's next read
    await page.evaluate(() => document.getElementById('auto').__friends.refresh());
    assert.equal(await page.inputValue(`${row} .lf-world`), 'vanilla');
    await page.click('#auto .lf-join');
    await page.waitForFunction(() => window.__joined);
    assert.match(await page.evaluate(() => window.__joined), /\/join\/abcdefghijk2345a#/);
    await page.click(`${row} .lf-remove`);
    await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'friends_answer'));
    assert.deepEqual(await calls(page, 'friends_answer'), [{ peerId: 'cccccccccccccccc', action: 'remove' }]);
  } finally { await browser.close(); }
});

test('the profile loads and saves', async () => {
  const { browser, page } = await open();
  try {
    await page.click('#auto .lf-profile summary');
    assert.equal(await page.inputValue('#auto .lf-profile input >> nth=0'), 'Ari');
    await page.fill('#auto .lf-profile input >> nth=1', 'Raiding at 8');
    await page.selectOption('#auto .lf-profile select', 'everyone');
    await page.click('#auto .lf-save');
    await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'friends_profile'));
    assert.deepEqual(await calls(page, 'friends_profile'), [{ name: 'Ari', status: 'Raiding at 8', sharePlaying: true, lan: 'everyone', mailbox: true }]);
  } finally { await browser.close(); }
});

test('errors in plain words; an empty list says what to do', async () => {
  const empty = Object.assign({}, STATE, { friends: [], incoming: [], outgoing: [], near: [] });
  const browser = await chromium.launch();
  const page = await browser.newPage();
  try {
    await page.addInitScript(FAKE, { state: empty, fail: { friends_add: "That isn't a Leyline friend code (it starts with LF1-)." } });
    await page.goto('about:blank');
    await page.setContent('<div id="auto" data-leyline-friends></div>');
    await page.addScriptTag({ path: path.join(UI, 'friends.js') });
    await page.waitForFunction(() => /share your code/.test((document.querySelector('#auto .lf-empty') || {}).textContent || ''));
    assert.equal(await page.isHidden('#auto .lf-requests'), true);
    assert.equal(await page.isHidden('#auto .lf-near'), true);
    await page.fill('#auto .lf-paste', 'nope');
    await page.click('#auto .lf-add-go');
    await page.waitForFunction(() => /friend code/.test(document.querySelector('#auto .lf-status').textContent));
  } finally { await browser.close(); }
});

test("Invite picks the world that runs; a core without friends says to update, with no empty boxes", async () => {
  const { browser, page } = await open();
  try {
    await page.evaluate(() => { window.LeylineWorlds = { status: async (k) => ({ running: k === 'vanilla' }) }; });
    await page.evaluate(() => document.getElementById('auto').__friends.refresh());
    assert.equal(await page.inputValue('#auto .lf-friend[data-peer="bbbbbbbbbbbbbbbb"] .lf-world'), 'vanilla');
  } finally { await browser.close(); }
  const b2 = await chromium.launch();
  const page2 = await b2.newPage();
  try {
    await page2.addInitScript(FAKE, { state: STATE, fail: { friends_state: 'Command friends_state not found', friends_start: 'Command friends_start not found' } });
    await page2.goto('about:blank');
    await page2.setContent('<div id="auto" data-leyline-friends></div>');
    await page2.addScriptTag({ path: path.join(UI, 'friends.js') });
    await page2.waitForFunction(() => /newer Leyline/.test(document.querySelector('#auto .lf-list').textContent));
    assert.equal(await page2.isHidden('#auto .lf-requests'), true);
    assert.equal(await page2.isHidden('#auto .lf-near'), true);
  } finally { await b2.close(); }
});
