// Be a node (ui/node.js, src-tauri/src/p2p/node.rs) in Chromium, with a fake Tauri core: the card at the top of the Host
// page (on every Host tab), the one switch, not offered = "Not open yet", the live stats, Pause/Resume, the resting and
// stopped states, Limits (upload, connections, MB per day, idle, keep running, start with Windows), Advanced (the full
// relay's players, the network check that never shows an address), the Settings part that opens the card, and the
// Settings > Advanced list that leaves be_a_node to the card.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/node.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');

const FAKE = (opts) => {
  const calls = (window.__calls = []);
  const o = opts || {};
  const settings = Object.assign({ on: false, paused: false, upload_kbps: 256, max_peers: 20, day_mb: 2048, only_idle: true,
    keep_running: false, start_with_windows: false }, o.settings || {});
  const stats = Object.assign({ since: 0, uptime_secs: 0, shared_bytes: 0, day: 0, today_bytes: 0, files: 0, peers_now: 0, network: '',
    full_until: 0, last_fill: 0, last_flags: 0, last_tick: 0, state: 'off', peer_strikes: 0 }, o.stats || {});
  let view = { settings, state: o.state || (settings.on ? 'running' : 'off'), offered: o.offered !== false, stats,
    engine: o.engine !== false, has_files: o.hasFiles !== false, autostart: !!o.autostart, started_as_node: false,
    carry: Object.assign({ offered: false, on: false, running: false, players: 0 }, o.carry || {}) };
  window.__node = (patch) => { view = Object.assign({}, view, patch); };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [], selected_client: null, selected_realm: 'play.openvanilla.app',
        wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app',
        browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'deep_link_take': return null;
      case 'node_state': return JSON.parse(JSON.stringify(view));
      case 'node_set': {
        const s = args.settings;
        if (s.on && !view.settings.on && !view.offered) throw 'Be a node is not open yet. It opens for everyone at once.';
        const was = view.settings.on;
        const st = !s.on ? 'off' : s.paused ? 'paused' : 'running';
        view = Object.assign({}, view, { settings: s, state: st,
          stats: Object.assign({}, view.stats, s.on ? { files: 3, today_bytes: 12.5 * 1048576, shared_bytes: 3.2 * 1073741824, uptime_secs: 7860, peers_now: 4 } : {}) });
        const line = !s.on ? 'Off. This PC is not a node now.' : s.paused ? 'Paused. Resume when you like.' : !was ? 'On: this PC is a node now. Thank you.' : 'Saved.';
        return [line, JSON.parse(JSON.stringify(view))];
      }
      case 'node_check_network': return o.nat || 'symmetric';
      case 'node_quit': return null;
      case 'host_info': return { dir: '/home/me/kit', is_kit: true, has_windows_script: true, has_unix_script: true, problems: [], seeds: [], realm: null };
      case 'host_network': return { set_up: false, node: {} };
      case 'host_carry': return { settings: { on: false, public_host: '', region: '', kbps: 512, max_players: 20, only_idle: true },
        node: { running: false, players: 0, kbps: 0, paused: '' }, offered: true, idle_secs: null, on_ac: null };
      case 'host_rules': return [];
      case 'hostpack_info': return { installed: { version: '7' }, available: { version: '7' }, update: false, kit_dir: '/home/me/kit' };
      case 'docker_info': return { state: 'ready', version: '27', help: 'Docker is ready.', get_url: '' };
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      case 'p2p_flags': return { effective: { limits: { seed_upload_kbps: 0, seed_ratio_pct: 0, max_peers: 0 } },
        known: [['torrent_downloads', 'Download packages over BitTorrent'], ['be_a_node', 'Offer Be a node on the Host page']],
        local: { overrides: {} }, site: {} };
      case 'host_profiles': return [[], null];
      case 'host_scan': return null;
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
  await page.waitForSelector('#node-card:not([hidden]) #node-switch');
  return { page, errors };
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c).map(([, a]) => a), cmd);
const NO_BRANDS = /warcraft|\bwow\b|lich king|(?<!endorsed by )blizzard/i;
const IP = /\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b/;
const text = (page, sel) => page.textContent(sel);

test('not offered yet: the card is there, the switch greyed, nothing else', async () => {
  const { page, errors } = await open({ offered: false }, { width: 420, height: 900 });
  assert.equal(await text(page, '#node-title'), 'Be a node');
  assert.ok(await page.isDisabled('#node-switch'));
  assert.equal(await page.getAttribute('#node-switch', 'aria-checked'), 'false');
  assert.match(await text(page, '#node-card'), /Not open yet: it opens for everyone at once/);
  assert.equal(await page.locator('#node-limits').count(), 0);
  // the card sits above the Host tabs' panes: the first thing on the page after its title
  assert.ok(await page.evaluate(() => document.querySelector('.hc-panel > .panel-head').nextElementSibling.id === 'node-card'));
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
  assert.equal(overflow, false, 'no sideways scroll at phone width');
  assert.deepEqual(errors, []);
});

test('one switch: on, working with live stats, pause and resume, off', async () => {
  const { page, errors } = await open({});
  assert.match(await text(page, '#node-card'), /While you're not playing, this PC helps the Leyline network/);
  assert.match(await text(page, '#node-card'), /Never game files/);
  assert.equal(await page.locator('#node-state').count(), 0, 'off: the switch says it');
  assert.equal(await page.locator('#node-today').count(), 0, 'no stats while off');
  await page.click('#node-switch');
  await page.waitForFunction(() => document.getElementById('node-switch').getAttribute('aria-checked') === 'true');
  assert.equal((await calls(page, 'node_set'))[0].settings.on, true);
  assert.equal(await text(page, '.node-status'), 'On: this PC is a node now. Thank you.');
  assert.equal(await text(page, '#node-state'), "Working: sharing 3 files of Leyline's own, 4 connected now.");
  assert.equal(await text(page, '#node-today b'), '13 MB');
  assert.equal(await text(page, '#node-total b'), '3.2 GB');
  assert.equal(await text(page, '#node-uptime b'), '2 h 11 min');
  assert.equal(await page.locator('#node-players').count(), 0, 'players carried only with the full relay on');
  await page.click('#node-pause');
  await page.waitForFunction(() => document.getElementById('node-state').textContent === 'Paused.');
  assert.equal(await text(page, '#node-pause'), 'Resume');
  assert.equal((await calls(page, 'node_set')).pop().settings.paused, true);
  await page.click('#node-pause');
  await page.waitForFunction(() => /Working/.test(document.getElementById('node-state').textContent));
  await page.click('#node-switch');
  await page.waitForFunction(() => document.getElementById('node-switch').getAttribute('aria-checked') === 'false');
  assert.equal(await text(page, '.node-status'), 'Off. This PC is not a node now.');
  // the card is above every Host tab
  await page.click('#hc-tab-guide');
  assert.ok(await page.isVisible('#node-card'));
  assert.doesNotMatch(await text(page, '#node-card'), NO_BRANDS);
  await page.screenshot({ path: path.join(process.env.SHOTS || require('os').tmpdir(), 'node-card.png') });
  assert.deepEqual(errors, []);
});

test('the states say why it rests, and a withdrawn offer can still be switched off', async () => {
  const on = { on: true };
  for (const [state, want] of [['resting_in_use', 'Resting while you use this PC.'], ['resting_battery', 'Resting while this PC runs on battery.'],
    ['day_cap', "Today's limit is used up: back tomorrow."], ['full', 'Lots of players at once: resting a minute or two.']]) {
    const { page, errors } = await open({ settings: on, state });
    assert.equal(await text(page, '#node-state'), want, state);
    assert.ok(await page.evaluate(() => document.getElementById('node-state').classList.contains('node-rest')));
    assert.deepEqual(errors, []);
    await page.close();
  }
  const { page, errors } = await open({ settings: on, state: 'not_offered', offered: false });
  assert.match(await text(page, '#node-state'), /Stopped for now by the Leyline team/);
  assert.ok(await page.isEnabled('#node-switch'), 'on but withdrawn: the switch can still turn it off');
  await page.click('#node-switch');
  await page.waitForFunction(() => document.getElementById('node-switch').getAttribute('aria-checked') === 'false');
  assert.deepEqual(errors, []);
  await page.close();
  const v = await open({ settings: on, engine: false });
  assert.equal(await text(v.page, '#node-state'), "On, but this version of Leyline can't upload yet.");
  await v.page.close();
  const w = await open({ settings: on, hasFiles: false });
  assert.match(await text(w.page, '#node-state'), /waiting for Leyline's signed file list/);
  await w.page.close();
});

test('Limits: the caps, idle, keep running (with Quit), start with Windows only where it works', async () => {
  const { page, errors } = await open({ settings: { on: true }, autostart: false });
  assert.equal(await page.getAttribute('#node-limits', 'open'), null, 'closed until asked');
  await page.click('#node-limits summary');
  assert.ok(await page.isChecked('#node-idle'), 'only while idle and plugged in: on by default');
  assert.ok(!(await page.isChecked('#node-keep')) && !(await page.isChecked('#node-boot')), 'both off by default');
  assert.ok(await page.isDisabled('#node-boot'), 'start with Windows: Windows only');
  await page.fill('#node-kbps', '500');
  await page.fill('#node-peers', '8');
  await page.fill('#node-day', '1000');
  await page.uncheck('#node-idle');
  await page.check('#node-keep');
  await page.click('#node-limits button:text("Save limits")');
  await page.waitForFunction(() => document.querySelector('.node-status').textContent === 'Saved.');
  assert.deepEqual((await calls(page, 'node_set')).pop().settings, { on: true, paused: false, upload_kbps: 500, max_peers: 8, day_mb: 1000,
    only_idle: false, keep_running: true, start_with_windows: false });
  assert.equal(await page.getAttribute('#node-limits', 'open'), '', 'stays open after saving');
  await page.click('button:text("Quit Leyline")');
  assert.equal((await calls(page, 'node_quit')).length, 1);
  assert.deepEqual(errors, []);
  const w = await open({ settings: { on: true }, autostart: true });
  await w.page.click('#node-limits summary');
  assert.ok(await w.page.isEnabled('#node-boot'));
  await w.page.check('#node-boot');
  await w.page.click('#node-limits button:text("Save limits")');
  await w.page.waitForFunction(() => document.querySelector('.node-status').textContent === 'Saved.');
  assert.equal((await calls(w.page, 'node_set')).pop().settings.start_with_windows, true);
  await w.page.close();
});

test('Advanced: the full relay, players carried, the network check never shows an address', async () => {
  const { page, errors } = await open({ settings: { on: true }, carry: { offered: true, on: true, running: true, players: 3 }, nat: 'endpoint_independent' });
  assert.equal(await text(page, '#node-players b'), '3');
  await page.click('#node-advanced summary');
  assert.match(await text(page, '#node-advanced'), /Docker, a public name for this PC and ports open/);
  assert.match(await text(page, '#node-advanced'), /carrying 3 players now/);
  await page.click('#node-advanced button:text("Can other players reach this PC?")');
  await page.waitForFunction(() => /Probably/.test(document.getElementById('node-net').textContent));
  assert.doesNotMatch(await text(page, '#node-card'), IP, 'no address on screen');
  await page.click('#node-advanced button:text("Open Help carry the network")');
  await page.waitForSelector('#hc-pane-network.on #hc-net-carry');
  assert.equal(await page.getAttribute('#hc-tab-network', 'aria-selected'), 'true');
  assert.deepEqual(errors, []);
});

test('Settings: a Network part opens the card; Advanced leaves be_a_node to it', async () => {
  const { page, errors } = await open({});
  await page.click('.nav[data-page="settings"]');
  await page.waitForSelector('#set-node');
  assert.match(await text(page, '#set-node'), /Be a node: let this PC help the Leyline network while you're not playing/);
  await page.waitForFunction(() => document.querySelectorAll('#p2p-adv select').length > 0);
  const advText = await text(page, '#p2p-adv');
  assert.match(advText, /Download packages over BitTorrent/);
  assert.doesNotMatch(advText, /Offer Be a node/, 'its switch is the card, not a flag row');
  assert.ok(await page.evaluate(() => { const a = document.getElementById('set-node'), b = document.getElementById('p2p-adv'); return !!(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING); }), 'before Advanced');
  await page.click('#set-node button:text("Open Be a node")');
  await page.waitForFunction(() => document.querySelector('.page.on').id === 'page-host' && document.activeElement && document.activeElement.id === 'node-switch');
  assert.deepEqual(errors, []);
});
