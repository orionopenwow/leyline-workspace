// Friends on the Leyline network (ui/friends.js; app.rs friends_*; docs/LEYLINE-FRIENDS.md), in Chromium with a fake
// Tauri core:
//   * mount(el) and data-leyline-friends draw the section; friends_start once;
//   * my friend code: shown and copied; pasting a code sends friends_add and says which way it went;
//   * requests: Accept / Decline / Cancel call friends_answer with the right action;
//   * people near: Add asks friends_ask_near;
//   * friends online first, what they play, last seen; Invite sends the chosen world; Join opens the usual Join card
//     (LeylineSolo.offerJoin); Remove asks first, on the row (Keep / Escape keep them);
//   * the list kit (listkit.js): tabs All / Online / Requests / Near you / Favourites, groups, search, sort by name and
//     last seen, stars, keys (Enter = Join or Invite, Escape clears the search), counts, empty and loading states;
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
  window.confirm = () => { window.__confirmed = true; return true; };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    window.__calls.push([cmd, args]);
    if (o.fail && o.fail[cmd]) throw o.fail[cmd];
    if (o.hold && cmd === 'friends_state') await new Promise((r) => { window.__release = r; });
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
  if (o.size) await page.setViewportSize(o.size);
  if (o.dark !== false) await page.emulateMedia({ colorScheme: 'dark' });
  // a real origin, so localStorage (favourites, the sort) works
  await page.route('http://leyline.test/**', (r) => r.fulfill({ contentType: 'text/html', body: '<!doctype html><html><body><div id="auto" data-leyline-friends></div><div id="other"></div></body></html>' }));
  await page.goto('http://leyline.test/');
  await page.addStyleTag({ path: path.join(UI, 'listkit.css') });
  await page.addStyleTag({ path: path.join(UI, 'friends.css') });
  await page.addScriptTag({ path: path.join(UI, 'listkit.js') });
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
    await page.waitForSelector(`${row} .lf-confirm`);
    assert.match(await page.textContent(`${row} .lf-confirm`), /Remove Hero CCCC from your friends\? They'll need your code to add you again\./);
    assert.equal(await page.evaluate(() => document.activeElement.textContent), 'Keep', 'the safe choice has focus');
    await page.keyboard.press('Escape');
    await page.waitForSelector(`${row} .lf-confirm`, { state: 'detached' });
    assert.equal((await calls(page, 'friends_answer')).length, 0, 'Escape keeps them');
    await page.click(`${row} .lf-remove`);
    await page.click(`${row} .lf-remove-no`);
    assert.equal((await calls(page, 'friends_answer')).length, 0, 'Keep keeps them');
    await page.click(`${row} .lf-remove`);
    await page.click(`${row} .lf-remove-yes`);
    await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'friends_answer'));
    assert.equal(await page.evaluate(() => !!window.__confirmed), false, 'no browser pop-up');
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
    await page.addStyleTag({ path: path.join(UI, 'listkit.css') });
    await page.addStyleTag({ path: path.join(UI, 'friends.css') });
    await page.addScriptTag({ path: path.join(UI, 'listkit.js') });
    await page.addScriptTag({ path: path.join(UI, 'friends.js') });
    await page.waitForFunction(() => /No friends yet — share your code/.test((document.querySelector('#auto .lf-empty') || {}).textContent || ''));
    assert.equal(await page.isHidden('#auto .lk-bar'), true, 'nothing to search before the first friend');
    // the empty state's button shows the code
    await page.click('#auto .lf-empty button');
    await page.waitForSelector('#auto .lf-code:not([hidden])');
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
    await page2.addScriptTag({ path: path.join(UI, 'listkit.js') });
    await page2.addScriptTag({ path: path.join(UI, 'friends.js') });
    await page2.waitForFunction(() => /newer Leyline/.test(document.querySelector('#auto .lf-list').textContent));
    assert.equal(await page2.$$eval('#auto .lf-friend, #auto .lf-req, #auto .lf-near-row', (l) => l.length), 0);
  } finally { await b2.close(); }
});

// ---------------------------------------------------------------- the list kit
const SHOTS = process.env.LISTS_SHOTS || '/mnt/user-data/outputs/gallery';
async function shot(page, name, sel) {
  try {
    require('node:fs').mkdirSync(SHOTS, { recursive: true });
    const el = sel ? await page.$(sel) : null;
    await (el || page).screenshot({ path: path.join(SHOTS, `lists-friends-${name}.png`) });
  } catch (_) { /* screenshots are a nicety */ }
}
// the launcher's whole look for the screenshots: every stylesheet index.html links and its art, the section in the
// Single player page's Friends panel (.panel.wide.spf), so a shot shows what the app shows
const { appLook } = require('./fixtures/app-look.cjs');
const themed = (page, o) => appLook(page, Object.assign({ root: '#auto', panel: 'spf' }, o));
const visibleNames = (page) => page.$$eval('#auto .lf-kit .lk-body > [data-lk-key] .lf-name', (e) => e.map((x) => x.textContent));

test('the kit: tabs, groups (Requests, Online, Offline, Near you), counts, ids', async () => {
  const { browser, page, errors } = await open({ size: { width: 900, height: 1100 } });
  try {
    assert.deepEqual(await page.$$eval('#auto [role="tab"]', (b) => b.map((x) => x.id)), ['auto-tab-all', 'auto-tab-on', 'auto-tab-req', 'auto-tab-near', 'auto-tab-fav']);
    assert.equal(await page.getAttribute('#auto-tab-all', 'aria-selected'), 'true');
    assert.deepEqual(await page.$$eval('#auto .lk-group .lk-group-l', (e) => e.map((x) => x.textContent)), ['Requests', 'Online', 'Offline', 'Near you']);
    assert.deepEqual(await visibleNames(page), ['Dee', 'Hero EEEE', 'Bo', 'Hero CCCC', 'Fay']);
    assert.equal(await page.textContent('#auto-tab-req .lk-tab-n'), '2');
    assert.equal(await page.textContent('#auto-tab-on .lk-tab-n'), '1');
    assert.equal(await page.textContent('#auto .lf-count'), '2 friends');
    assert.equal(await page.getAttribute('#auto .lf-friend.on .lf-dot', 'aria-label'), 'online');
    assert.equal(await page.textContent('#auto .lf-friend.on .lf-playing'), 'Playing Single player');
    await themed(page);
    await shot(page, 'all', '#auto .lf-list');
    await page.click('#auto-tab-on');
    assert.deepEqual(await visibleNames(page), ['Bo']);
    assert.equal(await page.$$eval('#auto .lk-group', (g) => g.length), 0, 'one group: no header');
    await page.click('#auto-tab-req');
    assert.deepEqual(await visibleNames(page), ['Dee', 'Hero EEEE']);
    await shot(page, 'requests', '#auto .lf-list');
    await page.click('#auto-tab-near');
    assert.deepEqual(await visibleNames(page), ['Fay']);
    // arrow keys move between tabs
    await page.focus('#auto-tab-near');
    await page.keyboard.press('ArrowRight');
    assert.equal(await page.getAttribute('#auto-tab-fav', 'aria-selected'), 'true');
    assert.match(await page.textContent('#auto .lf-kit .lk-empty'), /No favourites yet/);
    assert.doesNotMatch(await page.textContent('body'), BRANDS);
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});

test('the kit: search marks the match, counts what shows, Escape clears; no match offers a way out', async () => {
  const { browser, page } = await open({ size: { width: 900, height: 900 } });
  try {
    const q = '#auto .lf-kit .lk-search input';
    assert.equal(await page.getAttribute(q, 'placeholder'), 'Find a friend');
    await page.fill(q, 'tonight');
    assert.deepEqual(await visibleNames(page), ['Bo'], 'the status is searched');
    assert.equal(await page.textContent('#auto .lf-kit mark.lk-hl'), 'tonight');
    assert.equal(await page.textContent('#auto .lf-count'), '1 of 5 shown');
    await page.fill(q, 'single');
    assert.deepEqual(await visibleNames(page), ['Bo'], 'what they play is searched');
    await page.fill(q, 'fa');
    assert.deepEqual(await visibleNames(page), ['Fay']);
    await page.press(q, 'Escape');
    assert.equal(await page.inputValue(q), '');
    assert.equal((await visibleNames(page)).length, 5);
    // "/" goes to the search
    await page.click('#auto .lf-title');
    await page.keyboard.press('/');
    assert.equal(await page.evaluate(() => document.activeElement.type), 'search');
    await page.keyboard.type('zzz');
    await page.waitForSelector('#auto .lf-kit .lk-empty-nomatch');
    assert.match(await page.textContent('#auto .lf-kit .lk-empty'), /No friends match “zzz”/);
    await themed(page);
    await shot(page, 'nomatch', '#auto .lf-list');
    await page.click('#auto .lf-kit .lk-empty button');
    assert.equal((await visibleNames(page)).length, 5);
    // in a tab with nothing, the kit offers the tab that has it
    await page.click('#auto-tab-on');
    await page.fill(q, 'fay');
    assert.match(await page.textContent('#auto .lf-kit .lk-empty'), /Show 1 in Near you/);
    await page.click('#auto .lf-kit .lk-empty button >> nth=0');
    assert.equal(await page.getAttribute('#auto-tab-near', 'aria-selected'), 'true');
  } finally { await browser.close(); }
});

test('the kit: sort by name and by last seen; favourites kept and shown under Favourites', async () => {
  const more = JSON.parse(JSON.stringify(STATE));
  more.friends.push({ peer_id: 'aaaaaaaaaaaaaaaa', name: 'Ama', status: '', online: false, near: false, last_seen: NOW - 120 * 60, playing: null, open: null, invite: null, worlds: [], since: NOW });
  more.friends.push({ peer_id: 'zzzzzzzzzzzzzzzz', name: 'Zed', status: '', online: true, near: true, last_seen: NOW, playing: null, open: null, invite: null, worlds: [], since: NOW });
  const { browser, page } = await open({ state: more });
  try {
    await page.click('#auto-tab-on');
    await page.click('#auto-tab-all');
    // online first (the groups), the core's order inside a group
    assert.deepEqual(await visibleNames(page), ['Dee', 'Hero EEEE', 'Bo', 'Zed', 'Hero CCCC', 'Ama', 'Fay']);
    await page.selectOption('#auto .lf-kit .lk-sort-s', 'seen');
    assert.deepEqual(await visibleNames(page), ['Dee', 'Hero EEEE', 'Bo', 'Zed', 'Ama', 'Hero CCCC', 'Fay'], 'most recently seen first');
    await page.selectOption('#auto .lf-kit .lk-sort-s', 'name');
    assert.deepEqual(await visibleNames(page), ['Dee', 'Hero EEEE', 'Bo', 'Zed', 'Ama', 'Hero CCCC', 'Fay']);
    assert.deepEqual(await page.evaluate(() => JSON.parse(localStorage.getItem('leyline.list.friends.sort'))), { id: 'name', dir: 1 });
    // a star: only friends have one
    assert.equal(await page.$$eval('#auto .lf-req .lk-star, #auto .lf-near-row .lk-star, #auto .lf-out .lk-star', (l) => l.length), 0);
    await page.click('#auto .lf-friend[data-peer="cccccccccccccccc"] .lk-star');
    assert.equal(await page.getAttribute('#auto .lf-friend[data-peer="cccccccccccccccc"] .lk-star', 'aria-pressed'), 'true');
    assert.match(await page.getAttribute('#auto .lf-friend[data-peer="cccccccccccccccc"] .lk-star', 'aria-label'), /Remove Hero CCCC from favourites/);
    assert.deepEqual(await page.evaluate(() => JSON.parse(localStorage.getItem('leyline.list.friends.fav'))), ['cccccccccccccccc']);
    await page.click('#auto-tab-fav');
    assert.deepEqual(await visibleNames(page), ['Hero CCCC']);
    // the next read keeps the star and the tab
    await page.evaluate(() => document.getElementById('auto').__friends.refresh());
    assert.deepEqual(await visibleNames(page), ['Hero CCCC']);
  } finally { await browser.close(); }
});

test('the kit: keys (Enter joins their invite, else invites; on a request accepts), the focus survives a read', async () => {
  const { browser, page } = await open();
  try {
    await page.focus('#auto .lf-friend[data-peer="bbbbbbbbbbbbbbbb"]');
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => window.__joined);
    await page.keyboard.press('ArrowDown');
    assert.equal(await page.evaluate(() => document.activeElement.dataset.peer), 'cccccccccccccccc');
    // a read in the background keeps the place
    await page.evaluate(() => document.getElementById('auto').__friends.refresh());
    assert.equal(await page.evaluate(() => document.activeElement.dataset.peer), 'cccccccccccccccc');
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'friends_invite'));
    assert.deepEqual(await calls(page, 'friends_invite'), [{ peerId: 'cccccccccccccccc', kind: 'single' }]);
    await page.keyboard.press('Home');
    assert.equal(await page.evaluate(() => document.activeElement.dataset.peer), 'dddddddddddddddd');
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'friends_answer'));
    assert.deepEqual(await calls(page, 'friends_answer'), [{ peerId: 'dddddddddddddddd', action: 'accept' }]);
    // Tab goes into the row's own buttons
    await page.focus('#auto .lf-friend[data-peer="cccccccccccccccc"]');
    await page.keyboard.press('Tab');
    assert.match(await page.evaluate(() => document.activeElement.className), /lf-world/);
  } finally { await browser.close(); }
});

test('the kit: loading while the first read runs; Requests and Near you hide when they hold nobody', async () => {
  const only = Object.assign({}, STATE, { incoming: [], outgoing: [], near: [] });
  const browser = await chromium.launch();
  const page = await browser.newPage();
  try {
    await page.addInitScript(FAKE, { state: only, hold: true });
    await page.goto('about:blank');
    await page.setContent('<div id="auto" data-leyline-friends></div>');
    await page.addStyleTag({ path: path.join(UI, 'listkit.css') });
    await page.addStyleTag({ path: path.join(UI, 'friends.css') });
    await page.addScriptTag({ path: path.join(UI, 'listkit.js') });
    await page.addScriptTag({ path: path.join(UI, 'friends.js') });
    await page.waitForSelector('#auto .lf-kit .lk-empty-loading');
    assert.match(await page.textContent('#auto .lf-kit .lk-empty'), /Reading your friends…/);
    await page.waitForFunction(() => window.__release);
    await page.evaluate(() => window.__release());
    await page.waitForSelector('#auto .lf-friend');
    assert.equal(await page.isHidden('#auto-tab-req'), true);
    assert.equal(await page.isHidden('#auto-tab-near'), true);
    assert.equal(await page.isVisible('#auto-tab-on'), true);
    assert.equal(await page.$$eval('#auto .lk-group', (g) => g.length), 2, 'Online, Offline');
  } finally { await browser.close(); }
});

test('screenshots: the remove question, an empty list, the phone', async () => {
  const { browser, page } = await open({ size: { width: 900, height: 900 } });
  try {
    await themed(page);
    await page.click('#auto .lf-friend[data-peer="cccccccccccccccc"] .lf-remove');
    await page.waitForSelector('#auto .lf-confirm');
    await shot(page, 'remove', '#auto .lf-list');
  } finally { await browser.close(); }
  const empty = Object.assign({}, STATE, { friends: [], incoming: [], outgoing: [], near: [] });
  const browser2 = await chromium.launch();
  const p2 = await browser2.newPage({ viewport: { width: 860, height: 700 } });
  try {
    await p2.addInitScript(FAKE, { state: empty });
    await p2.emulateMedia({ colorScheme: 'dark' });
    await p2.goto('about:blank');
    await p2.setContent('<div id="auto" data-leyline-friends></div>');
    await p2.addStyleTag({ path: path.join(UI, 'listkit.css') });
    await p2.addStyleTag({ path: path.join(UI, 'friends.css') });
    await p2.addScriptTag({ path: path.join(UI, 'listkit.js') });
    await p2.addScriptTag({ path: path.join(UI, 'friends.js') });
    await p2.waitForSelector('#auto .lf-empty');
    await themed(p2);
    await shot(p2, 'empty', '#auto .lf-list');
  } finally { await browser2.close(); }
  const ph = await open({ size: { width: 390, height: 900 } });
  try {
    await themed(ph.page, { phone: true, width: 390 });
    await shot(ph.page, 'phone', '#auto .lf-list');
  } finally { await ph.browser.close(); }
});
