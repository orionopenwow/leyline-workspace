// Friends with a character on a single player world, both sides (ui/worldfriends.js; app.rs world_friends,
// world_friend, friend_worlds, join_friend_again, friend_world_forget; host/worldfriends.rs), in Chromium with a fake
// Tauri core:
//   * the host's list: names from the friends list, characters, Invited / Can come back any time / Needs a new invite;
//     Trust and Stop trusting; Remove asks on the row and keeps characters; Remove and delete characters asks in plain
//     words (Keep them cancels) and only then deletes; Remove needs the world running;
//   * the world picker reads the other world;
//   * the friend's side: Join again with this PC's game folder (join_friend_again), Forget, hidden when empty;
//   * an older core without the commands hides both; errors in plain words; no game or publisher names anywhere.
//
//   node --test launcher/tests/worldfriends.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const UI = path.join(__dirname, '..', 'ui');
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard|3\.3\.5|1\.12/i;
const NOW = Math.floor(Date.now() / 1000);
const FRIENDS = {
  single: [
    { user: 'GAAAAAAAAAAAAAAA', peer: 'bbbbbbbbbbbbbbbb', name: 'Bo', on_list: true, online: true, trusted: false, enabled: true, legacy: false, seen: NOW - 60,
      chars: [{ user: 'GAAAAAAAAAAAAAAA', name: 'Brakka', level: 12, online: true }, { user: 'GAAAAAAAAAAAAAAA', name: 'Odo', level: 3, online: false }] },
    { user: 'GCCCCCCCCCCCCCCC', peer: 'cccccccccccccccc', name: '', on_list: false, online: false, trusted: true, enabled: false, legacy: false, seen: NOW - 9000, chars: [] },
    { user: 'GOLD234567', peer: '', name: '', on_list: false, online: false, trusted: false, enabled: false, legacy: true, seen: NOW - 99999, chars: [] },
  ],
  vanilla: [],
};
const WORLDS = [
  { realm_id: 'kzdvvj2umnduyauf', name: "Bo's world", user: 'GZZZZZZZZZZZZZZZ', at: NOW - 7200 },
  { realm_id: 'aaaaaaaaaaaaaaaa', name: '', user: 'GYYYYYYYYYYYYYYY', at: NOW - 86400 * 3 },
];

const FAKE = (o) => {
  window.__calls = [];
  window.__o = o;
  window.LeylineWorlds = { status: async (k) => ({ running: !!o.running[k] }) };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    window.__calls.push([cmd, args]);
    if (o.missing && o.missing.includes(cmd)) throw `Command ${cmd} not found`;
    if (o.fail && o.fail[cmd]) throw o.fail[cmd];
    switch (cmd) {
      case 'world_friends': return JSON.parse(JSON.stringify(o.friends[args.kind] || []));
      case 'world_friend': {
        const l = o.friends[args.kind];
        const f = l.find((x) => x.user === args.user);
        if (args.action === 'trust') { f.trusted = true; return 'Trusted: they can come back any time this world is shared, without a new invite.'; }
        if (args.action === 'untrust') { f.trusted = false; return 'Not trusted: they need an invite again after you stop sharing.'; }
        o.friends[args.kind] = l.filter((x) => x.user !== args.user);
        return args.action === 'delete' ? 'Removed, and their characters on your world are deleted.' : 'Removed. Their characters stay in your world: invite them again and they get them back.';
      }
      case 'friend_worlds': return JSON.parse(JSON.stringify(o.worlds));
      case 'friend_world_forget': o.worlds = o.worlds.filter((w) => w.realm_id !== args.realmId); return null;
      case 'get_state': return { settings: { clients: o.clients, selected_client: o.clients[0] ? o.clients[0].id : null } };
      case 'join_friend_again': return { address: '127.0.0.1:3724', name: "Bo's world", guest: { auto: true, account: 'GZZZZZZZZZZZZZZZ', password: '', why: '', message: '' }, way: 'lan', tried: [] };
      default: throw 'unknown command ' + cmd;
    }
  } } };
};

async function open(o = {}) {
  const opts = Object.assign({ friends: JSON.parse(JSON.stringify(FRIENDS)), worlds: JSON.parse(JSON.stringify(WORLDS)), running: { single: true },
    clients: [{ id: 'c1', name: 'My folder', build: '3.3.5a' }] }, o);
  const browser = await chromium.launch();
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, opts);
  await page.goto('about:blank');
  await page.setContent('<!doctype html><html><body><div class="spf" id="spf"><div class="wf" id="wf"></div><div class="wf fw" id="fw" hidden></div></div></body></html>');
  await page.addStyleTag({ path: path.join(UI, 'friends.css') });
  await page.addScriptTag({ path: path.join(UI, 'worldfriends.js') });
  await page.evaluate(() => window.LeylineWorldFriends.refresh());
  return { browser, page, errors };
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter((x) => x[0] === c).map((x) => x[1]), cmd);
const line = (page, id) => page.textContent(id);

test("the host's list: names, characters, whether they can come back, no brand words", async () => {
  const { browser, page, errors } = await open();
  try {
    await page.waitForSelector('#wf-list');
    const names = await page.$$eval('#wf-list .wf-name', (e) => e.map((x) => x.textContent));
    assert.deepEqual(names, ['Bo', 'Hero CCCC', 'An earlier friend (GOLD234567)']);
    const chips = await page.$$eval('#wf-list .wf-state', (e) => e.map((x) => x.textContent));
    assert.deepEqual(chips, ['Invited', 'Can come back any time', 'Needs a new invite']);
    assert.match(await page.textContent('#wf-list li:nth-child(1) .wf-chars'), /Brakka \(level 12, in the world now\), Odo \(level 3\)/);
    assert.match(await page.textContent('#wf-list li:nth-child(1)'), /On your friends list, online/);
    assert.equal(await page.textContent('#wf-list li:nth-child(2) .wf-trust'), 'Stop trusting');
    assert.deepEqual(await calls(page, 'world_friends'), [{ kind: 'single' }]);
    assert.ok(await page.isHidden('#fw') === false, 'the friend side shows: this PC joined worlds');
    assert.doesNotMatch(await page.textContent('body'), BRANDS);
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});

test('Trust, Remove (keeps characters) and Remove and delete characters (asks in plain words first)', async () => {
  const { browser, page } = await open();
  try {
    await page.waitForSelector('#wf-list');
    await page.click('#wf-list li:nth-child(1) .wf-trust');
    await page.waitForFunction(() => /Trusted/.test(document.getElementById('wf-line').textContent));
    assert.deepEqual((await calls(page, 'world_friend')).pop(), { kind: 'single', user: 'GAAAAAAAAAAAAAAA', action: 'trust' });
    assert.equal(await page.textContent('#wf-list li:nth-child(1) .wf-state'), 'Can come back any time');
    // delete asks first, on the row, naming what goes; Keep them changes nothing
    await page.click('#wf-list li:nth-child(1) .wf-delete');
    await page.waitForSelector('#wf-list .wf-confirm.bad');
    assert.match(await page.textContent('#wf-list .wf-confirm'), /Delete Bo's login and all 2 of their characters on your world for good\? This can't be undone\./);
    assert.equal(await page.evaluate(() => document.activeElement && document.activeElement.textContent), 'Keep them', 'the safe choice has focus');
    await page.click('#wf-list .wf-no');
    await page.waitForSelector('#wf-list .wf-confirm', { state: 'detached' });
    assert.equal((await calls(page, 'world_friend')).length, 1, 'nothing deleted');
    await page.click('#wf-list li:nth-child(1) .wf-delete');
    await page.click('#wf-list .wf-yes');
    await page.waitForFunction(() => /deleted/.test(document.getElementById('wf-line').textContent));
    assert.deepEqual((await calls(page, 'world_friend')).pop(), { kind: 'single', user: 'GAAAAAAAAAAAAAAA', action: 'delete' });
    assert.equal(await page.$$eval('#wf-list li', (l) => l.length), 2);
    // remove keeps characters, and says so before and after
    await page.click('#wf-list li:nth-child(1) .wf-remove');
    assert.match(await page.textContent('#wf-list .wf-confirm'), /their characters stay here: invite them again and they get them back/);
    await page.click('#wf-list .wf-yes');
    await page.waitForFunction(() => /Their characters stay/.test(document.getElementById('wf-line').textContent));
    assert.deepEqual((await calls(page, 'world_friend')).pop(), { kind: 'single', user: 'GCCCCCCCCCCCCCCC', action: 'remove' });
  } finally { await browser.close(); }
});

test('the world not running: Remove waits for it; the other world is read on its own; an empty world says what happens', async () => {
  const { browser, page } = await open({ running: {} });
  try {
    await page.waitForSelector('#wf-list');
    assert.ok(await page.isDisabled('#wf-list li:nth-child(1) .wf-remove'));
    assert.ok(await page.isDisabled('#wf-list li:nth-child(1) .wf-delete'));
    assert.ok(!(await page.isDisabled('#wf-list li:nth-child(1) .wf-trust')), 'trust needs no running world');
    assert.match(await page.textContent('#wf'), /Start Single player to remove a friend/);
    await page.selectOption('#wf-kind', 'vanilla');
    await page.waitForSelector('#wf .wf-empty');
    assert.match(await page.textContent('#wf .wf-empty'), /keeps their login and characters on your world for next time/);
    assert.deepEqual((await calls(page, 'world_friends')).pop(), { kind: 'vanilla' });
  } finally { await browser.close(); }
});

test("the friend's side: Join again with this PC's folder, Forget, hidden when there's nothing", async () => {
  const { browser, page, errors } = await open();
  try {
    await page.waitForSelector('#fw-list');
    assert.deepEqual(await page.$$eval('#fw-list .wf-name', (e) => e.map((x) => x.textContent)), ["Bo's world", "Your friend's world"]);
    assert.match(await page.textContent('#fw-list li:nth-child(1)'), /joined 2 hours ago/);
    await page.click('#fw-list li:nth-child(1) .fw-join');
    await page.waitForFunction(() => /signs you in by itself/.test(document.getElementById('fw-line').textContent));
    assert.deepEqual(await calls(page, 'join_friend_again'), [{ client: 'c1', realmId: 'kzdvvj2umnduyauf' }]);
    await page.click('#fw-list li:nth-child(2) .fw-forget');
    await page.waitForFunction(() => document.querySelectorAll('#fw-list li').length === 1);
    assert.deepEqual(await calls(page, 'friend_world_forget'), [{ realmId: 'aaaaaaaaaaaaaaaa' }]);
    await page.click('#fw-list li:nth-child(1) .fw-forget');
    await page.waitForSelector('#fw', { state: 'hidden' });
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});

test('Join again with no game folder, or refused by the friend: said in plain words', async () => {
  const { browser, page } = await open({ clients: [] });
  try {
    await page.waitForSelector('#fw-list');
    await page.click('#fw-list li:nth-child(1) .fw-join');
    await page.waitForFunction(() => /add it under Settings › Game folders/.test(document.getElementById('fw-line').textContent));
    assert.equal((await calls(page, 'join_friend_again')).length, 0);
  } finally { await browser.close(); }
  const b = await open({ fail: { join_friend_again: "Your friend's world: Your friend's world isn't open to you right now: ask them to invite you again." } });
  try {
    await b.page.waitForSelector('#fw-list');
    await b.page.click('#fw-list li:nth-child(1) .fw-join');
    await b.page.waitForFunction(() => /invite you again/.test(document.getElementById('fw-line').textContent));
    assert.ok(!(await b.page.isDisabled('#fw-list li:nth-child(1) .fw-join')), 'can try again');
  } finally { await b.browser.close(); }
});

test('an older core without these commands: both parts hidden, no errors', async () => {
  const { browser, page, errors } = await open({ missing: ['world_friends', 'friend_worlds'] });
  try {
    await page.waitForFunction(() => window.__calls.length >= 2);
    await page.waitForSelector('#wf', { state: 'hidden' });
    assert.ok(await page.isHidden('#fw'));
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});
