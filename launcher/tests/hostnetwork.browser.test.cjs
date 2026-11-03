// The host console's Network tab (ui/hostnetwork.js, src-tauri/src/host/network.rs) in Chromium, with a fake Tauri core:
// listed or not, Hero Key sign-in on/off with Test Hero Key sign-in (every step's line), the opt-in to share names, the
// mods editor (required or optional, checked ids), house rules, players from the network, and Help carry the network
// (one sentence, caps, the live "carrying N players" line, one-click off, not offered = not open yet).
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/hostnetwork.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');

const FAKE = (opts) => {
  const calls = (window.__calls = []);
  const o = opts || {};
  const net = {
    set_up: o.setUp !== false, linked: true, official: false, public: true, listed: true, connected: true,
    herokey_on: !!o.herokey, herokey_key: !!o.herokey, herokey_db: true, herokey_live: !!o.herokey, share_names: false,
    players: 4, who: ['Grumpy', 'Aelyn'], mods: [{ id: 'up:skada', name: 'Skada', required: true, client: true }],
    rules: ['Be kind.'], arrivals_hour: 2, arrivals_day: 9,
    node: { running: false, players: 0, kbps: 0, paused: '', registered: false, healthy: false, max_players: 0, max_kbps: 0 },
  };
  let carry = { settings: { on: false, public_host: '', region: '', kbps: 512, max_players: 20, only_idle: true },
    node: net.node, offered: o.offered !== false, idle_secs: null, on_ac: null };
  let rules = ['Be kind.'];
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [], selected_client: null, selected_realm: 'play.openvanilla.app',
        wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app',
        browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'deep_link_take': return null;
      case 'host_info': return { dir: '/home/me/kit', is_kit: true, has_windows_script: true, has_unix_script: true, problems: [], seeds: [],
        realm: { realm_id: 'abcdefghijk2345a', name: 'My Server', visibility: 'public', relay: 'relay.openvanilla.app', profile: 'linked' } };
      case 'host_console': return { set_up: true, linked: true, name: 'My Server', realm_id: 'abcdefghijk2345a', profile: 'linked', core: 'azerothcore',
        visibility: 'public', signup: 'off', herokey: false, claim_sent: false, browser: false, max_players: 0, relay: 'relay.openvanilla.app',
        relay_connected: true, relay_error: null, docker: true, services: [], uptime: null, gm: false, gm_note: null, invite: null };
      case 'host_status': return { realm_id: 'abcdefghijk2345a', online: true, name: 'My Server', visibility: 'public', players: 4, bots: 0, listed: true };
      case 'host_network': return net;
      case 'host_network_set':
        if (args.what === 'herokey-on') { net.herokey_on = true; return 'Hero Key sign-in on. Run Test Hero Key sign-in to check every step.'; }
        if (args.what === 'names-on') { net.share_names = true; return 'The realm list shows who is online (never GMs or bots).'; }
        if (args.what === 'listed-off') { net.public = false; net.listed = false; return 'Not listed.'; }
        if (args.what === 'mods') { net.mods = JSON.parse(args.arg); return 'Mods saved. The realm list shows them within a minute.'; }
        return 'Saved.';
      case 'host_herokey_test': return { ok: false, steps: [
        { id: 'on', ok: true, text: 'Hero Key sign-in is switched on for this realm.' },
        { id: 'key', ok: false, text: "The website's key is not pinned yet." },
        { id: 'account', ok: true, text: 'A test account HKABCDEFGHIJKLMN was made, its one-time login checked, and all of it undone.' }] };
      case 'host_rules': return rules;
      case 'host_rules_set': rules = args.lines; return 'Saved. The realm list shows them within a minute.';
      case 'host_carry': return carry;
      case 'host_carry_set':
        if (args.settings.on && !/^[a-z0-9.-]+\.[a-z]+$/.test(args.settings.public_host)) throw "Give this PC's public name (like node.example.org): players reach it by that name.";
        carry = Object.assign({}, carry, { settings: args.settings,
          node: args.settings.on ? { running: true, players: 3, kbps: 41, paused: '', registered: true, healthy: true, max_players: args.settings.max_players, max_kbps: args.settings.kbps } : net.node });
        return args.settings.on ? `On: this PC helps carry the network as ${args.settings.public_host}, up to ${args.settings.max_players} players.` : 'Off. This PC carries nobody now.';
      case 'hostpack_info': return { installed: { version: '7' }, available: { version: '7' }, update: false, kit_dir: '/home/me/kit' };
      case 'docker_info': return { state: 'ready', version: '27', help: 'Docker is ready.', get_url: '' };
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      case 'p2p_flags': return { effective: { limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'host_profiles': return [[], null];
      case 'host_scan': return null;
      case 'host_check': return { ok: true, summary: '', rows: [] };
      case 'host_progress': return { idle: true, kind: '', running: false, code: null, step: '', n: 0, of: 0, secs: 0, problem: null };
      case 'open_link': case 'set_host_kit': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts, viewport) {
  const page = await browser.newPage({ viewport: viewport || { width: 1280, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.click('.nav[data-page="host"]');
  await page.click('#hc-tab-network');
  await page.waitForSelector('#hc-pane-network.on .hc-net-box');
  return { page, errors };
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c).map(([, a]) => a), cmd);
const NO_BRANDS = /warcraft|\bwow\b|lich king|blizzard/i;
const status = (page) => page.textContent('.hc-net-status');

test('the Network tab: its own tab, the realm on the list, Hero Key with its self-test', async () => {
  const { page, errors } = await open({});
  assert.equal(await page.getAttribute('#hc-tab-network', 'aria-selected'), 'true');
  assert.equal(await page.getAttribute('#hc-tab-main', 'aria-selected'), 'false');
  assert.ok(await page.isHidden('#hc-pane-main'));
  assert.deepEqual(await page.$$eval('.hc-net-box .hc-bh', (l) => l.map((x) => x.textContent)),
    ['On the realm list', 'Hero Key sign-in', "Who's online", 'Mods players need', 'House rules', 'Players from the network', 'Help carry the network']);
  assert.deepEqual(await page.$$eval('.hc-net-pill', (l) => l.map((x) => x.textContent)), ['Listed', 'Hero Key sign-in off', 'Connected']);
  // Hero Key on, then the dry run: one line per step
  await page.click('.hc-net-box:has(.hc-bh:text("Hero Key sign-in")) .hc-seg-b:text("On")');
  await page.waitForFunction(() => /Hero Key sign-in on/.test(document.querySelector('.hc-net-status').textContent));
  assert.deepEqual((await calls(page, 'host_network_set')).map((a) => a.what), ['herokey-on']);
  await page.click('#hc-net-hktest');
  await page.waitForSelector('#hc-net-hkresults li');
  assert.deepEqual(await page.$$eval('#hc-net-hkresults li', (l) => l.map((x) => x.className)), ['ok', 'bad', 'ok']);
  assert.match(await status(page), /each line says what to do/);
  // back to another tab and here again
  await page.click('#hc-tab-guide');
  assert.equal(await page.getAttribute('#hc-tab-network', 'aria-selected'), 'false');
  assert.ok(await page.isHidden('#hc-pane-network'));
  await page.click('#hc-tab-network');
  assert.ok(await page.isVisible('#hc-pane-network'));
  assert.doesNotMatch(await page.textContent('#hc-pane-network'), NO_BRANDS);
  assert.equal(await page.locator('#hc-pane-network input[type="password"]').count(), 0, 'no password here');
  await page.screenshot({ path: path.join(process.env.SHOTS || require('os').tmpdir(), 'hostnetwork.png'), fullPage: true });
  assert.deepEqual(errors, []);
});

test('names opt-in, mods editor, rules, not listed', async () => {
  const { page, errors } = await open({});
  await page.check('#hc-net-names');
  await page.waitForFunction(() => /who is online/.test(document.querySelector('.hc-net-status').textContent));
  await page.waitForSelector('.hc-net-line.hc-net-names');
  assert.equal(await page.textContent('.hc-net-line.hc-net-names'), 'Grumpy, Aelyn');
  // a bad id is refused here; a good one is added and the list saved with required/optional
  await page.fill('#hc-net-modid', 'https://evil.example/x.zip');
  await page.click('.hc-net-box:has(#hc-net-modid) button:text("Add")');
  assert.match(await status(page), /not a mod id/);
  await page.fill('#hc-net-modid', 'mod:dalaran-dice');
  await page.click('.hc-net-box:has(#hc-net-modid) button:text("Add")');
  await page.fill('.hc-net-mod:nth-child(2) input[type="text"]', 'Dice');
  await page.click('.hc-net-mod:nth-child(1) .hc-seg-b:text("Optional")');
  await page.click('button:text("Save mods")');
  await page.waitForFunction(() => /Mods saved/.test(document.querySelector('.hc-net-status').textContent));
  const saved = JSON.parse((await calls(page, 'host_network_set')).find((a) => a.what === 'mods').arg);
  assert.deepEqual(saved, [{ id: 'up:skada', name: 'Skada', required: false, client: true }, { id: 'mod:dalaran-dice', name: 'Dice', required: false, client: true }]);
  await page.fill('#hc-net-rules', 'Be kind.\nNo gold selling.');
  await page.click('button:text("Save rules")');
  await page.waitForFunction(() => /Saved/.test(document.querySelector('.hc-net-status').textContent));
  assert.deepEqual((await calls(page, 'host_rules_set'))[0].lines, ['Be kind.', 'No gold selling.']);
  await page.click('.hc-seg-b:text("Not listed")');
  await page.waitForFunction(() => /Not listed/.test(document.querySelector('.hc-net-pill').textContent));
  assert.match(await page.textContent('.hc-net-box:nth-of-type(1)'), /Players from the network|realm list/);
  assert.match(await page.textContent('#hc-pane-network'), /2 logins came in through the Leyline network in the last hour, 9 today/);
  assert.deepEqual(errors, []);
});

test('Help carry the network: one sentence, caps, carrying N players, one-click off', async () => {
  const { page, errors } = await open({});
  const box = await page.textContent('#hc-net-carry');
  assert.match(box, /Passes other players' game connections through this PC/);
  assert.match(box, /never stores or shares game files/);
  assert.equal(await page.textContent('#hc-net-carrying'), 'Off.');
  assert.ok(await page.isChecked('#hc-net-idle'), 'only while idle and plugged in: on by default');
  await page.click('#hc-net-carry-go');
  await page.waitForFunction(() => /public name/.test(document.querySelector('.hc-net-status').textContent));
  await page.fill('#hc-net-host', 'node.example.org');
  await page.fill('#hc-net-kbps', '300');
  await page.fill('#hc-net-max', '8');
  await page.selectOption('#hc-net-region', 'eu');
  await page.click('#hc-net-carry-go');
  await page.waitForFunction(() => /Carrying 3 players now/.test((document.getElementById('hc-net-carrying') || {}).textContent || ''));
  const sent = (await calls(page, 'host_carry_set')).pop().settings;
  assert.deepEqual(sent, { on: true, public_host: 'node.example.org', region: 'eu', kbps: 300, max_players: 8, only_idle: true });
  assert.equal(await page.textContent('#hc-net-carry-go'), 'Turn off');
  await page.click('#hc-net-carry-go');
  await page.waitForFunction(() => document.getElementById('hc-net-carrying').textContent === 'Off.');
  assert.equal((await calls(page, 'host_carry_set')).pop().settings.on, false);
  await page.screenshot({ path: path.join(process.env.SHOTS || require('os').tmpdir(), 'hostnetwork-narrow.png'), fullPage: true });
  assert.deepEqual(errors, []);
});

test('not offered yet, and no realm yet', async () => {
  const { page, errors } = await open({ offered: false, setUp: false }, { width: 420, height: 900 });
  assert.match(await page.textContent('#hc-net-carry'), /Not open yet/);
  assert.equal(await page.locator('#hc-net-carry-go').count(), 0);
  assert.match(await page.textContent('#hc-pane-network'), /Set up your realm/);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
  assert.equal(overflow, false, 'no sideways scroll at phone width');
  assert.deepEqual(errors, []);
});
