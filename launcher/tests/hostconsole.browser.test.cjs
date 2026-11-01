// The host console (ui/hostconsole.js) in Chromium, with a fake Tauri core: once a realm is set up on this PC, the Host
// page is "Your realm console" (the website's mockup, stream/hub.py host_showcase): the realm's name and Online · listed,
// the six service tiles, the GM controls (announce at once; kick, ban and restart only after a confirm; a restart never
// carries a delay), the health check's five checks, the realm tools (invite links, accounts for friends through the
// invite link only, public or private, who may sign up, logs, house rules) and the cores. Before that: the setup start
// (the rulesets with Vanilla+ always on, Link my server) and the guide, whose links are named links in the core.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/hostconsole.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const RID = 'abcdefghijk2345a';

const FAKE = (setUp) => {
  const calls = (window.__calls = []);
  const den = setUp === 'den';   // a Den set up on this PC (docs/DEN.md)
  let realm = setUp ? { realm_id: 'abcdefghijk2345a', name: den ? 'My Den' : 'Frost Haven', visibility: den ? 'private' : 'public', relay: 'relay.openvanilla.app', profile: den ? 'den' : 'vanillaplus' } : null;
  const svc = (role, state, text) => ({ role, state, text });
  let rules = ['Be kind.', 'No gold selling.'];
  let visibility = den ? 'private' : 'public';
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [], selected_client: null, selected_realm: 'play.openvanilla.app',
        wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app',
        browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'deep_link_take': return null;
      case 'host_info': if (window.__jobDone && !realm) realm = window.__jobDone; return { dir: '/home/me/kit', is_kit: !window.__noKit, has_windows_script: true, has_unix_script: true, problems: [], seeds: [], realm };
      case 'host_console': return { set_up: !!realm, linked: !!(realm && realm.profile === 'linked'), name: realm ? realm.name : '', realm_id: realm ? realm.realm_id : '',
        profile: realm ? realm.profile : 'vanillaplus', core: 'azerothcore', visibility, signup: 'invite', herokey: false, claim_sent: false, browser: true, max_players: den ? 10 : 0,
        relay: 'relay.openvanilla.app', relay_connected: true, relay_error: null, docker: true,
        services: [svc('database', 'ok', 'ready'), svc('login', 'ok', 'ready'), svc('world', 'ok', 'ready'), svc('agent', 'ok', 'connected')],
        uptime: '3 days', gm: true, gm_note: null, invite: 'https://relay.openvanilla.app/join/abcdefghijk2345a' };
      case 'host_status': return { realm_id: args.realmId, online: true, name: realm ? realm.name : 'Frost Haven', visibility, address: 'relay.openvanilla.app:40002', players: 12, bots: 0, listed: visibility === 'public' };
      case 'host_gm':
        if (args.action === 'status') return 'gm=1\nup=3725\nplayers=12\nbots=0\nrestart=0\nallow=off\nallowed=0\nvis=public\nmotd=-';
        return { announce: 'Announced to everyone online.', kick: `${args.data} was kicked.`, ban: 'Banned.', restart: 'Restart in 5 minutes. Everyone online was told.' }[args.action] || 'Done.';
      case 'host_check': return { ok: false, summary: "1 problem(s) found. Players can't join until they're fixed.", rows: [
        { id: 'pc', state: 'ok', lines: [{ kind: 'ok', text: 'The login server is running.', fix: null }] },
        { id: 'tunnel', state: 'ok', lines: [{ kind: 'ok', text: 'Your realm is connected to the relay.', fix: null }] },
        { id: 'login', state: 'bad', lines: [{ kind: 'bad', text: 'The world server is not reachable through the relay.', fix: 'Run the check again in a minute.' }] },
        { id: 'invite', state: 'ok', lines: [{ kind: 'ok', text: 'The join page works.', fix: null }] },
        { id: 'list', state: 'warn', lines: [{ kind: 'note', text: 'Your realm is not in the list yet.', fix: 'It appears within a minute.' }] }] };
      case 'host_tool': if (args.name === 'private' || args.name === 'public') visibility = args.name; return 'Saved.';
      case 'host_invite': return 'https://relay.openvanilla.app/join/abcdefghijk2345a';
      case 'host_new_invite': return 'https://relay.openvanilla.app/join/abcdefghijk2345a';
      case 'host_rules': return rules;
      case 'host_rules_set': rules = args.lines.filter((l) => l.trim()); return 'Saved. The realm list shows them within a minute.';
      case 'host_logs': return 'wu-host-worldserver | World initialized';
      // a Den's save files (host/dens.rs)
      case 'host_dens': return (window.__dens = window.__dens || [
        { name: 'house', bytes: 2400, props: 16, saved: '2026-10-31T22:50:37Z' }, { name: 'backup', bytes: 900, props: 4, saved: '2026-10-31T20:00:00Z' }]);
      case 'host_den_text': return '{"format":"leyline-den","version":1,"name":"' + args.name + '","props":[]}';
      case 'host_den_add': if (!/"leyline-den"/.test(args.text)) throw "That isn't a Den file."; window.__dens.push({ name: 'friend', bytes: 99, props: 1, saved: '' }); return 'friend';
      case 'host_den_open': return args.name;
      case 'host_stop': return 'Stopped.';
      case 'host_start': return 'Started in its own window.';
      case 'host_link_start': return 'Started in its own window: answer its four questions.';
      case 'host_choose': return { profile: args.profile, client_dir: '' };
      case 'host_profiles': return [[
        { id: 'vanillaplus', label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c1', name: 'Mine', path: '/g' }], note: null },
        { id: 'dalaran', label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c1', name: 'Mine', path: '/g' }], note: null },
        { id: 'den', label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c1', name: 'Mine', path: '/g' }], note: null },
        { id: 'main', label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c1', name: 'Mine', path: '/g' }], note: null },
        { id: 'cmangos-1.12.1', label: 'x', version: '1.12.1', core: 'cmangos', in_kit: true, clients: [], note: 'Add your own client of that version on the Clients page first.' },
      ], null];
      case 'hostpack_info': return window.__noKit ? { installed: null, available: { version: '7', size: 412000 }, update: false, kit_dir: '/home/me/kit' }
        : { installed: { version: '7' }, available: { version: '7' }, update: false, kit_dir: '/home/me/kit' };
      case 'hostpack_install': window.__noKit = false; return [{ version: '7' }, '/home/me/kit'];
      case 'set_host_kit': return null;
      case 'docker_info': if (window.__dock) return window.__dock(); return window.__noKit ? { state: 'missing', version: null, help: 'Docker is not installed.', get_url: 'https://docs.docker.com/desktop/' } : { state: 'ready', version: '27.3.1', help: 'Docker is ready.', get_url: 'https://docs.docker.com/desktop/' };
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'open_link': return null;
      case 'host_scan': return window.__found || null;
      case 'host_start_quiet': window.__jobAt = 0; return 'Setting up your realm. The first start takes 5 to 20 minutes.';
      case 'host_link_quiet': window.__jobAt = 0; window.__jobKind = 'link'; return 'Linking your server. It shows on the realm list within a minute.';
      case 'host_progress': {
        if (window.__jobAt === undefined) return { idle: true, kind: '', running: false, code: null, step: '', n: 0, of: 0, secs: 0, problem: null };
        const link = window.__jobKind === 'link';
        const steps = link ? ['Getting the helper (about a minute)', "Saving your server's details", 'Connecting your server to the network', 'Done']
          : ['Getting the helper (about a minute)', 'Saving your settings', 'Downloading the realm (the biggest step)', 'Reading the game data from your game folder',
            'Starting the servers (the first time: 5 to 20 minutes)', 'Setting up the ruleset', 'Connecting to the network', 'Done'];
        const n = Math.min(++window.__jobAt, steps.length);
        if (window.__jobFail && n >= 4) return { idle: false, kind: 'host', running: false, code: 1, step: steps[3], n: 4, of: 8, secs: 61,
          problem: 'Your game folder was not found or could not be read. Add it on the Clients page, then try again.' };
        const done = n >= (window.__jobSteps || steps.length);
        if (done && !window.__jobDone) window.__jobDone = link
          ? { realm_id: 'abcdefghijk2345a', name: 'My Server', visibility: 'public', relay: 'relay.openvanilla.app', profile: 'linked' }
          : { realm_id: 'abcdefghijk2345a', name: 'Dalaran Hangout', visibility: 'private', relay: 'relay.openvanilla.app', profile: 'dalaran' };
        return { idle: false, kind: link ? 'link' : 'host', running: !done, code: done ? 0 : null, step: done ? 'Done' : steps[n - 1], n, of: steps.length, secs: n * 37, problem: null };
      }
      case 'docker_start': window.__started = (window.__started || 0) + 1; return 'Starting Docker. This usually takes under a minute.';
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(setUp, light) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, setUp);
  if (light) await page.addInitScript(() => { try { localStorage.setItem('leyline.theme', 'light'); } catch (_) { /* */ } });
  await page.goto(INDEX);
  await page.click('.nav[data-page="host"]');
  await page.waitForFunction(() => document.getElementById('hc-pane-main').classList.contains('on'));
  return { page, errors };
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c).map(([, a]) => a), cmd);
const NO_BRANDS = /warcraft|\bwow\b|lich king|blizzard/i;

test('a realm set up on this PC: the Host page is its console', async () => {
  const { page, errors } = await open(true);
  await page.waitForFunction(() => document.querySelector('.hc-run').textContent === 'Online · listed');
  assert.equal(await page.textContent('#hc-tab-main'), 'Console');
  assert.equal(await page.textContent('.hc-t'), 'Frost Haven');
  const tiles = await page.$$eval('.hc-svc li', (l) => l.map((x) => [x.querySelector('b').textContent, x.querySelector('span').textContent, x.className]));
  assert.deepEqual(tiles, [['Database', 'ready', 'hc-tile ok'], ['Login server', 'ready', 'hc-tile ok'], ['World server', 'ready', 'hc-tile ok'],
    ['Leyline network', 'listed', 'hc-tile ok'], ['Players', '12 online', 'hc-tile ok'], ['Uptime', '3 days', 'hc-tile ok']]);
  assert.deepEqual(await page.$$eval('.hc-act-h b', (l) => l.map((x) => x.textContent)),
    ['Announce', 'On-screen notice', 'Message of the day', 'Kick', 'Mute or unmute', 'Send to Dalaran', 'Ban account', 'Allow-list', 'Restart']);
  assert.deepEqual(await page.$$eval('.hc-chk li span', (l) => l.map((x) => x.textContent)),
    ['The servers on your PC', 'Your link to the Leyline network', 'A test login from outside', 'Your invite link', 'Your place on the realm list']);
  assert.deepEqual(await page.$$eval('.hc-tool', (l) => l.filter((x) => !x.parentElement.hidden).map((x) => x.textContent)),
    ['Invite links', 'Accounts for friends', 'Public or private', 'Who may sign up', 'Browser players', 'Max players', 'Server logs', 'House rules']);
  assert.deepEqual(await page.$$eval('.hc-core', (l) => l.map((x) => [x.dataset.core, x.querySelector('span span').textContent])),
    [['azerothcore', 'One click'], ['cmangos', 'One click'], ['trinitycore', 'Coming'], ['vmangos', 'Coming'], ['own', 'Small helper']]);
  assert.equal(await page.getAttribute('.hc-core[data-core="azerothcore"]', 'aria-current'), 'true', 'the core this realm runs');
  assert.ok(await page.isHidden('.hc-top-acts button:has-text("Start")') && await page.isVisible('.hc-top-acts button:has-text("Stop")'));
  assert.doesNotMatch(await page.textContent('#page-host'), NO_BRANDS);
  assert.equal(await page.locator('#page-host input[type="password"]').count(), 0, 'Leyline never asks for a password');
  assert.deepEqual(errors, []);
});

test('GM controls: announce at once; kick, Ban account (with a reason) and restart only after a confirm; mute, Send to Dalaran, notice; a restart never carries a delay', async () => {
  const { page, errors } = await open(true);
  await page.waitForFunction(() => document.querySelector('.hc-run').textContent === 'Online · listed');
  await page.click('[data-gm="announce"] .hc-act-h');
  await page.fill('[data-gm="announce"] input', 'Back in ten');
  await page.click('[data-gm="announce"] button:has-text("Send")');
  await page.waitForFunction(() => document.querySelector('.hc-status').textContent === 'Announced to everyone online.');
  assert.deepEqual((await calls(page, 'host_gm')).filter((a) => a.action !== 'status'), [{ action: 'announce', data: 'Back in ten' }]);

  // kick: Cancel does nothing, Kick does it
  await page.click('[data-gm="kick"] .hc-act-h');
  assert.ok(await page.isHidden('[data-gm="announce"] .hc-act-f'), 'one form open at a time');
  await page.fill('[data-gm="kick"] input', 'Grumpy');
  await page.click('[data-gm="kick"] .hc-act-f button:has-text("Kick")');
  await page.waitForSelector('#hc-confirm[open]');
  assert.equal(await page.textContent('#hc-confirm-title'), 'Kick Grumpy?');
  await page.click('#hc-confirm-no');
  await page.waitForTimeout(50);
  assert.equal((await calls(page, 'host_gm')).filter((a) => a.action === 'kick').length, 0, 'cancelled: nothing sent');
  await page.click('[data-gm="kick"] .hc-act-f button:has-text("Kick")');
  await page.click('#hc-confirm-yes');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_gm' && a.action === 'kick'));
  assert.deepEqual((await calls(page, 'host_gm')).find((a) => a.action === 'kick'), { action: 'kick', data: 'Grumpy' });

  // Ban account for a week: the whole account, a reason first (HQ's words)
  await page.click('[data-gm="ban"] .hc-act-h');
  await page.fill('[data-gm="ban"] input.hc-name', 'Grumpy');
  await page.selectOption('[data-gm="ban"] select', '168');
  await page.click('[data-gm="ban"] .hc-act-f button:has-text("Ban account")');
  await page.waitForFunction(() => document.querySelector('.hc-status').textContent === 'Say why (3 letters or more).');
  assert.ok(await page.isHidden('#hc-confirm[open]'), 'no reason: nothing asked, nothing sent');
  await page.fill('[data-gm="ban"] input.hc-reason', 'Cheating in the arena');
  await page.click('[data-gm="ban"] .hc-act-f button:has-text("Ban account")');
  await page.waitForSelector('#hc-confirm[open]');
  assert.equal(await page.textContent('#hc-confirm-title'), "Ban Grumpy's account for 1 week?");
  assert.match(await page.textContent('#hc-confirm-body'), /Every character on their account/);
  await page.click('#hc-confirm-yes');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_gm' && a.action === 'ban'));
  assert.deepEqual((await calls(page, 'host_gm')).find((a) => a.action === 'ban'), { action: 'ban', data: 'Grumpy\t168\tCheating in the arena' });
  await page.selectOption('[data-gm="ban"] select', '0');
  await page.click('[data-gm="ban"] .hc-act-f button:has-text("Ban account")');
  await page.waitForSelector('#hc-confirm[open]');
  assert.equal(await page.textContent('#hc-confirm-title'), "Ban Grumpy's account for good?");
  await page.click('#hc-confirm-no');
  await page.click('[data-gm="ban"] .hc-act-f button:has-text("Unban account")');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_gm' && a.action === 'unban'));

  // mute (with a reason), Send to Dalaran, the on-screen notice
  await page.click('[data-gm="mute"] .hc-act-h');
  await page.fill('[data-gm="mute"] input.hc-name', 'Grumpy');
  await page.fill('[data-gm="mute"] input.hc-reason', 'Spam in trade');
  await page.click('[data-gm="mute"] .hc-act-f button:has-text("Mute")');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_gm' && a.action === 'mute'));
  assert.deepEqual((await calls(page, 'host_gm')).find((a) => a.action === 'mute'), { action: 'mute', data: 'Grumpy\t60\tSpam in trade' });
  await page.click('[data-gm="rescue"] .hc-act-h');
  await page.fill('[data-gm="rescue"] input', 'Grumpy');
  await page.click('[data-gm="rescue"] .hc-act-f button:has-text("Send to Dalaran")');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_gm' && a.action === 'rescue'));
  assert.deepEqual((await calls(page, 'host_gm')).find((a) => a.action === 'rescue'), { action: 'rescue', data: 'Grumpy' });
  await page.click('[data-gm="notify"] .hc-act-h');
  await page.fill('[data-gm="notify"] input', 'Restart soon');
  await page.click('[data-gm="notify"] .hc-act-f button:has-text("Show")');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_gm' && a.action === 'notify'));
  assert.deepEqual((await calls(page, 'host_gm')).find((a) => a.action === 'notify'), { action: 'notify', data: 'Restart soon' });

  // restart: always the five-minute one
  await page.click('[data-gm="restart"] .hc-act-h');
  await page.click('[data-gm="restart"] button:has-text("Restart in 5 minutes")');
  await page.waitForSelector('#hc-confirm[open]');
  assert.match(await page.textContent('#hc-confirm-body'), /five minutes/);
  await page.click('#hc-confirm-yes');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_gm' && a.action === 'restart'));
  assert.deepEqual((await calls(page, 'host_gm')).find((a) => a.action === 'restart'), { action: 'restart', data: '' });
  assert.deepEqual(errors, []);
});

test('the health check, and the realm tools', async () => {
  const { page, errors } = await open(true);
  await page.waitForFunction(() => document.querySelector('.hc-run').textContent === 'Online · listed');
  await page.click('#hc-run-check');
  await page.waitForFunction(() => !document.querySelector('.hc-sum').hidden);
  assert.deepEqual(await page.$$eval('.hc-chk li', (l) => l.map((x) => x.className)), ['c-ok', 'c-ok', 'c-bad', 'c-ok', 'c-warn']);
  assert.match(await page.textContent('[data-check="login"] small'), /not reachable through the relay\. Run the check again/);
  assert.match(await page.textContent('.hc-sum'), /1 problem/);

  // accounts for friends: the invite link, never an account or a password
  await page.click('.hc-tool[data-tool="friends"]');
  assert.match(await page.textContent('.hc-drawer'), /their own account on your join page.*never makes accounts or sees passwords/);
  assert.equal(await page.locator('.hc-drawer input').count(), 0);
  // public or private
  await page.click('.hc-tool[data-tool="vis"]');
  assert.equal(await page.getAttribute('.hc-seg-b[data-value="public"]', 'aria-pressed'), 'true');
  await page.click('.hc-seg-b[data-value="private"]');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_tool' && a.name === 'private'));
  await page.waitForFunction(() => document.querySelector('.hc-seg-b[data-value="private"]').getAttribute('aria-pressed') === 'true');
  // who may sign up
  await page.click('.hc-tool[data-tool="signup"]');
  await page.click('.hc-seg-b[data-value="open"]');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_tool' && a.name === 'signup-open'));
  // browser players (Warden off) and max players (the server's cap)
  await page.click('.hc-tool[data-tool="browser"]');
  assert.equal(await page.getAttribute('.hc-seg-b[data-value="on"]', 'aria-pressed'), 'true');
  await page.click('.hc-seg-b[data-value="off"]');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_tool' && a.name === 'browser-off'));
  await page.click('.hc-tool[data-tool="max"]');
  await page.fill('.hc-drawer input[type=number]', '25');
  await page.click('.hc-drawer button:has-text("Save")');
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_tool' && a.name === 'max' && a.arg === '25'));
  // house rules
  await page.click('.hc-tool[data-tool="rules"]');
  await page.waitForFunction(() => document.querySelector('.hc-rules') && document.querySelector('.hc-rules').value === 'Be kind.\nNo gold selling.');
  await page.fill('.hc-rules', 'Be kind.\nNo gold selling.\nHave fun.');
  await page.click('.hc-drawer button:has-text("Save")');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_rules_set'));
  assert.deepEqual((await calls(page, 'host_rules_set'))[0], { lines: ['Be kind.', 'No gold selling.', 'Have fun.'] });
  // a new invite link asks first
  await page.click('.hc-tool[data-tool="invite"]');
  await page.waitForSelector('.hc-link');
  await page.click('.hc-drawer button:has-text("New link")');
  await page.waitForSelector('#hc-confirm[open]');
  await page.click('#hc-confirm-no');
  assert.equal((await calls(page, 'host_new_invite')).length, 0);
  // the logs
  await page.click('.hc-tool[data-tool="logs"]');
  await page.waitForFunction(() => document.querySelector('.hc-logs') && /World initialized/.test(document.querySelector('.hc-logs').textContent));
  assert.deepEqual(errors, []);
});

test('no realm yet: the setup start (Vanilla+ always on, Link my server) and the guide', async () => {
  const { page, errors } = await open(false);
  assert.equal(await page.textContent('#hc-tab-main'), 'Start');
  await page.waitForFunction(() => document.querySelectorAll('.hc-rs').length === 5);
  assert.deepEqual(await page.$$eval('.hc-rs', (l) => l.map((x) => [x.dataset.profile, x.querySelector('b').textContent, x.disabled])),
    [['vanillaplus', 'Vanilla+', false], ['den', 'My Den', false], ['dalaran', 'Dalaran hangout', false], ['main', 'Classic levelling', false], ['cmangos-1.12.1', 'True Vanilla', true]]);
  assert.equal(await page.textContent('.hc-rs[data-profile="vanillaplus"] em'), 'Always on');
  assert.equal(await page.textContent('#hc-start'), 'Start Dalaran hangout', 'a ruleset is picked already: Start is one press');
  assert.equal((await calls(page, 'host_choose')).length, 0, 'nothing written until Start or a pick');
  await page.click('.hc-rs[data-profile="vanillaplus"]');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_choose'));
  assert.deepEqual((await calls(page, 'host_choose'))[0], { profile: 'vanillaplus', clientId: 'c1' });
  assert.equal(await page.textContent('#hc-start'), 'Start Vanilla+');
  assert.match(await page.textContent('.hc-go'), /account name/, 'on a Mac or Linux the host names their own account first');
  assert.equal(await page.inputValue('#hc-name'), 'Vanilla Plus');
  await page.click('#hc-start');
  assert.equal((await calls(page, 'host_start_quiet')).length, 0, 'not without the account name');
  await page.fill('#hc-gm', 'hostguy');
  assert.match(await page.textContent('.hc-go'), /Always on/);
  await page.click('#hc-vis .hc-seg-b[data-value="public"]');
  await page.click('#hc-start');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_start_quiet'));
  assert.deepEqual((await calls(page, 'host_start_quiet'))[0], { answers: { name: 'Vanilla Plus', visibility: 'public', gm: 'hostguy' } });
  assert.equal((await calls(page, 'host_start')).length, 0, 'no window');
  await page.click('#hc-link');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_link_start'));
  assert.match(await page.textContent('.hc-start .hc-status'), /four questions/);
  assert.match(await page.textContent('.hc-pre'), /Host pack ready.*Docker ready/);

  // the guide beside it, and on its own tab; its links are names in the core's list, never a URL
  assert.equal(await page.locator('.hc-side .hc-g').count(), 11);
  await page.click('#hc-tab-guide');
  assert.ok(await page.isVisible('#hc-pane-guide'));
  const topics = await page.$$eval('#hc-pane-guide .hc-g summary', (l) => l.map((x) => x.textContent));
  assert.deepEqual(topics, ['How the Leyline network works', 'Start your realm', 'Stop it', 'Update', 'Go public', 'Invite friends', 'Claim it with your Hero Key', 'Keep it up 24/7',
    'Link a server you already run', 'Backups', "When something's wrong"]);
  await page.click('#hc-pane-guide .hc-g[data-topic="alwayson"] summary');
  await page.click('#hc-pane-guide .hc-g[data-topic="alwayson"] .hc-more');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'open_link'));
  assert.deepEqual((await calls(page, 'open_link'))[0], { name: 'alwayson' });
  const names = await page.$$eval('#hc-pane-guide .hc-more', (l) => [...new Set(l.map((x) => x.dataset.link))].sort());
  assert.deepEqual(names, ['alwayson', 'hostdocs', 'network']);
  // what a host needs and how long it takes, before the rulesets (and on the Host pack tab)
  const needs = await page.textContent('#hc-pane-main .hc-needs');
  assert.match(needs, /You need: Docker \(a free program that runs your realm in its own space\), your own game folder 3\.3\.5a, about 10 GB free\. The first start takes 5 to 20 minutes, and your PC must stay on for others to play\./);
  assert.match(await page.textContent('#host-needs'), /about 10 GB free/);
  // the kit and its settings are one tab away
  await page.click('#hc-tab-kit');
  assert.ok(await page.isVisible('#hp-card') && await page.isVisible("#hc-pane-kit ol.steps"));
  assert.doesNotMatch(await page.textContent('#page-host'), NO_BRANDS);
  assert.deepEqual(errors, []);
});

test('Make my Den: how it starts (Forest glade, Dark keep, Empty), opened once the Den is up', async () => {
  const { page, errors } = await open(false);
  await page.waitForFunction(() => document.querySelectorAll('.hc-rs').length === 5);
  assert.equal(await page.isVisible('.hc-denstart'), false, 'only for a Den');
  await page.click('.hc-rs[data-profile="den"]');
  await page.waitForSelector('.hc-denstart:not([hidden])');
  assert.equal(await page.textContent('.hc-denstart .hc-bh'), 'Start your Den as');
  assert.deepEqual(await page.$$eval('.hc-denstart .hc-ds', (l) => l.map((x) => [x.dataset.start, x.querySelector('b').textContent, x.getAttribute('aria-checked')])),
    [['forest-glade', 'Forest glade', 'true'], ['dark-keep', 'Dark keep', 'false'], ['empty', 'Empty', 'false']]);
  await page.click('.hc-denstart .hc-ds[data-start="dark-keep"]');
  assert.equal(await page.getAttribute('.hc-denstart .hc-ds[data-start="dark-keep"]', 'aria-checked'), 'true');
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.denStart')), 'dark-keep', 'kept for next time');
  await page.click('.hc-rs[data-profile="main"]');
  assert.equal(await page.isVisible('.hc-denstart'), false);
  await page.click('.hc-rs[data-profile="den"]');
  await page.fill('#hc-gm', 'denboss');
  await page.screenshot({ path: process.env.SHOT_DIR ? path.join(process.env.SHOT_DIR, 'den-start.png') : '/dev/null' }).catch(() => {});
  await page.evaluate(() => { window.__jobSteps = 2; });
  await page.click('#hc-start');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_den_open'), null, { timeout: 15000 });
  assert.deepEqual((await calls(page, 'host_den_open')).map((a) => a.name), ['dark-keep']);
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.denStartPending')), null, 'opened once');
  assert.doesNotMatch(await page.textContent('#page-host'), NO_BRANDS);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Make my Den, Empty: nothing opened', async () => {
  const { page, errors } = await open(false);
  await page.waitForFunction(() => document.querySelectorAll('.hc-rs').length === 5);
  await page.click('.hc-rs[data-profile="den"]');
  await page.click('.hc-denstart .hc-ds[data-start="empty"]');
  await page.fill('#hc-gm', 'denboss');
  await page.evaluate(() => { window.__jobSteps = 2; });
  await page.click('#hc-start');
  await page.waitForFunction(() => window.__jobDone, null, { timeout: 15000 });
  await page.waitForTimeout(500);
  assert.equal((await calls(page, 'host_den_open')).length, 0);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Make my Den: private and invite-only, no bots, no player limit; picked from the play bar; its console', async () => {
  const { page, errors } = await open(false);
  await page.waitForFunction(() => document.querySelectorAll('.hc-rs').length === 5);
  const card = '.hc-rs[data-profile="den"]';
  assert.equal(await page.textContent(`${card} .hc-rs-l`), 'A private room to build in: you and the friends you invite. No bots.');
  assert.equal(await page.textContent(`${card} em`), 'Private');
  assert.equal(await page.isVisible('.hc-den'), false, 'the Den facts only once it is picked');
  await page.click(card);
  await page.waitForFunction(() => window.__calls.some(([c, a]) => c === 'host_choose' && a.profile === 'den'));
  assert.equal(await page.textContent('#hc-start'), 'Start My Den');
  assert.ok(await page.isVisible('.hc-den'));
  assert.deepEqual(await page.$$eval('.hc-den li', (l) => l.map((x) => x.querySelector('b').textContent)), ['Private', 'Sign-up', 'No bots']);
  assert.match(await page.textContent('.hc-den'), /invite link only/);
  assert.equal(await page.locator('#page-host input[type="number"]:visible, #page-host [data-tool="max"]:visible').count(), 0, 'no bots or max-players control');
  await page.click('.hc-rs[data-profile="main"]');
  assert.equal(await page.isVisible('.hc-den'), false);
  // the play bar's My Den asks for it: picked again
  await page.evaluate(() => document.dispatchEvent(new CustomEvent('leyline:host-ruleset', { detail: { profile: 'den' } })));
  await page.waitForFunction(() => document.querySelector('.hc-rs[data-profile="den"].on'));
  assert.equal((await calls(page, 'host_choose')).pop().profile, 'den');
  assert.equal(await page.isVisible('#hc-vis'), false, 'a Den is always friends only');
  assert.equal(await page.inputValue('#hc-name'), 'My Den');
  await page.fill('#hc-gm', 'denboss');
  await page.click('#hc-start');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_start_quiet'));
  assert.deepEqual((await calls(page, 'host_start_quiet'))[0].answers, { name: 'My Den', visibility: 'private', gm: 'denboss' });
  assert.doesNotMatch(await page.textContent('#page-host'), NO_BRANDS);
  assert.deepEqual(errors, []);

  // a Den set up on this PC: its console, with no Max players tool
  const den = await open('den');
  await den.page.waitForFunction(() => document.querySelector('.hc-run') && document.querySelector('.hc-run').textContent === 'Online · private');
  assert.equal(await den.page.textContent('.hc-k'), 'Your Den');
  assert.equal(await den.page.textContent('.hc-t'), 'My Den');
  const shown = await den.page.$$eval('.hc-tool', (l) => l.filter((x) => !x.parentElement.hidden).map((x) => x.dataset.tool));
  assert.ok(!shown.includes('max') && shown.includes('invite') && shown.includes('signup'), shown.join(','));
  assert.deepEqual(den.errors, []);
});

test('a Den: Saved Dens lists the files, opens one after a confirm, saves a copy, adds one; only on a Den', async () => {
  const other = await open(true);
  await other.page.waitForSelector('.hc-tool[data-tool="dens"]', { state: 'attached' });
  assert.equal(await other.page.$eval('.hc-tool[data-tool="dens"]', (b) => b.parentElement.hidden), true, 'not on other realms');
  const { page, errors } = await open('den');
  await page.waitForFunction(() => document.querySelector('.hc-run') && document.querySelector('.hc-run').textContent === 'Online · private');
  await page.click('.hc-tool[data-tool="dens"]');
  await page.waitForSelector('.hc-den-list li');
  const rows = await page.$$eval('.hc-den-list li', (l) => l.map((x) => x.querySelector('.hc-den-t').textContent));
  assert.equal(rows.length, 2);
  assert.match(rows[0], /^house16 props · /);
  assert.match(rows[1], /backup \(before the last open\)4 props/);
  // Open: a confirm first; no means nothing is sent
  await page.click('.hc-den-list li:first-child button.primary');
  await page.waitForSelector('#hc-confirm[open]');
  assert.match(await page.textContent('#hc-confirm-body'), /replaces what's in your Den now \(a copy is kept as 'backup'\)/);
  await page.click('#hc-confirm-no');
  assert.deepEqual(await calls(page, 'host_den_open'), []);
  await page.click('.hc-den-list li:first-child button.primary');
  await page.click('#hc-confirm-yes');
  await page.waitForFunction(() => /opens when you're in your Den/.test(document.querySelector('.hc-status').textContent));
  assert.deepEqual(await calls(page, 'host_den_open'), [{ name: 'house' }]);
  assert.deepEqual(await calls(page, 'host_start'), [], 'the realm is up: no start');
  // Save a copy: a download of the file's text
  const [dl] = await Promise.all([page.waitForEvent('download'), page.click('.hc-den-list li:first-child button:not(.primary)')]);
  assert.equal(dl.suggestedFilename(), 'house.den.json');
  // Add a Den file from this PC
  await page.setInputFiles('.hc-dens ~ .hc-row input[type=file]', { name: 'Friend.den.json', mimeType: 'application/json',
    buffer: Buffer.from('{"format":"leyline-den","version":1,"props":[{"entry":2413}]}') });
  await page.waitForFunction(() => document.querySelectorAll('.hc-den-list li').length === 3);
  assert.match(await page.textContent('.hc-status'), /Added as 'friend'/);
  await page.setInputFiles('.hc-dens ~ .hc-row input[type=file]', { name: 'x.json', mimeType: 'application/json', buffer: Buffer.from('nope') });
  await page.waitForFunction(() => document.querySelector('.hc-status').classList.contains('bad'));
  await page.screenshot({ path: process.env.DEN_SHOT || '/tmp/hc-dens.png', fullPage: true });
  assert.doesNotMatch(await page.textContent('#page-host'), NO_BRANDS);
  assert.deepEqual(errors, []);
});

test('the console in the Light theme reads the light tokens', async () => {
  const { page, errors } = await open(true, true);
  await page.evaluate(() => { document.documentElement.dataset.base = 'light'; });
  await page.waitForFunction(() => document.querySelector('.hc-run').textContent === 'Online · listed');
  const bg = await page.evaluate(() => getComputedStyle(document.querySelector('.hc-sw')).backgroundImage);
  assert.match(bg, /rgb\(253, 252, 249\)/, 'the light slate');
  const tile = await page.evaluate(() => getComputedStyle(document.querySelector('.hc-tile b')).color);
  assert.equal(tile, 'rgb(20, 24, 32)', 'dark text on light');
  assert.deepEqual(errors, []);
});

test('no Host pack yet: two big numbered steps come first, and Get the Host pack installs and uses it', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  await page.addInitScript(() => { window.__noKit = true; });
  await page.addInitScript(FAKE, false);
  await page.goto(INDEX);
  await page.click('.nav[data-page="host"]');
  await page.waitForSelector('.hc-ready:not([hidden]) .hc-step');
  const steps = await page.$$eval('.hc-step', (l) => l.map((x) => [x.querySelector('b').textContent, x.classList.contains('done')]));
  assert.deepEqual(steps, [['Get the Host pack', false], ['Get Docker', false]]);
  assert.equal(await page.textContent('#hc-get-pack'), 'Get the Host pack');
  const order = await page.evaluate(() => { const r = document.querySelector('.hc-ready'), g = document.querySelector('.hc-rulesets');
    return !!(r.compareDocumentPosition(g) & Node.DOCUMENT_POSITION_FOLLOWING); });
  assert.ok(order, 'the steps come before the rulesets');
  if (process.env.SHOT) await page.screenshot({ path: process.env.SHOT });
  await page.click('#hc-get-pack');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'set_host_kit'));
  assert.ok((await calls(page, 'hostpack_install')).length === 1);
  assert.deepEqual((await calls(page, 'set_host_kit'))[0], { path: '/home/me/kit' });
  await page.close();
});

test('Docker installed but closed: the step says Start Docker (never Get Docker), and the page sees it start by itself', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  await page.addInitScript(() => {
    window.__dockState = 'desktop_stopped';
    window.__dock = () => ({ state: window.__dockState, version: null, get_url: 'https://docs.docker.com/desktop/',
      help: window.__dockState === 'missing' ? 'Get Docker Desktop: free for personal use.' : 'Docker Desktop is installed but closed.' });
  });
  await page.addInitScript(FAKE, false);
  await page.goto(INDEX);
  await page.click('.nav[data-page="host"]');
  await page.waitForSelector('.hc-step[data-docker="desktop_stopped"]');
  const s = await page.$eval('.hc-step[data-docker]', (x) => [x.querySelector('b').textContent, x.querySelector('.hc-step-t span').textContent, x.querySelectorAll('button').length]);
  assert.equal(s[0], 'Start Docker');
  assert.match(s[1], /installed but closed/);
  assert.ok(!(await page.textContent('.hc-ready')).includes('Get Docker'), 'never sent to the website when it is installed');
  assert.ok(await page.isDisabled('#hc-start'));
  assert.match(await page.textContent('.hc-go .hc-fine'), /Docker first/);
  assert.ok(await page.evaluate(() => window.__hcDockerWatch()), 'watching while the Start tab shows');
  if (process.env.SHOT_DIR) await page.screenshot({ path: process.env.SHOT_DIR + '/docker-closed.png' });
  // Docker Desktop opened by hand: within about 5 s the step goes and Start is ready
  const before = (await calls(page, 'docker_info')).length;
  await page.evaluate(() => { window.__dockState = 'ready'; });
  await page.waitForSelector('.hc-ready[hidden]', { state: 'attached', timeout: 8000 });
  assert.ok((await calls(page, 'docker_info')).length > before);
  assert.ok(!(await page.isDisabled('#hc-start')) || !(await page.$('.hc-rs.on')), 'Start once a ruleset is picked');
  // hidden tab: no more checks
  await page.click('#hc-tab-guide');
  assert.equal(await page.evaluate(() => window.__hcDockerWatch()), false, 'stops when the Start tab is hidden');
  await page.click('#hc-tab-main');
  assert.ok(await page.evaluate(() => window.__hcDockerWatch()));
  await page.click('.nav[data-page="play"]').catch(() => page.click('.nav[data-page]:not([data-page="host"])'));
  assert.equal(await page.evaluate(() => window.__hcDockerWatch()), false, 'stops when the Host page is left');
  // missing: Get Docker, and the line says the page notices
  await page.evaluate(() => { window.__dockState = 'missing'; });
  await page.click('.nav[data-page="host"]');
  await page.waitForSelector('.hc-step[data-docker="missing"]', { timeout: 8000 });
  assert.equal(await page.textContent('.hc-step[data-docker] b'), 'Get Docker');
  assert.match(await page.textContent('.hc-step[data-docker] .hc-step-t span'), /sees it/);
  if (process.env.SHOT_DIR) await page.screenshot({ path: process.env.SHOT_DIR + '/docker-missing.png' });
  await page.close();
});

test('Start Docker: one press starts Docker Desktop, the step counts the seconds, and moves on once Docker answers', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, colorScheme: 'dark' });
  await page.addInitScript(() => {
    let polls = 0;
    window.__dock = () => {
      if (window.__started) polls++;
      const ready = window.__started && polls >= 3;
      return ready ? { state: 'ready', version: '27.3.1', help: 'Docker is ready.', get_url: 'https://docs.docker.com/desktop/', can_start: false }
        : { state: 'desktop_stopped', version: null, help: 'Docker Desktop is installed but closed.', get_url: 'https://docs.docker.com/desktop/', can_start: true,
          desktop: 'C:/Program Files/Docker/Docker/Docker Desktop.exe' };
    };
  });
  await page.addInitScript(FAKE, false);
  await page.goto(INDEX);
  await page.click('.nav[data-page="host"]');
  await page.waitForSelector('#hc-docker-start');
  assert.equal(await page.textContent('#hc-docker-start'), 'Start Docker');
  await page.click('#hc-docker-start');
  await page.waitForSelector('.hc-step.wait[data-docker="starting"]');
  assert.match(await page.textContent('.hc-step[data-docker] .hc-step-t span'), /\d+ s so far/);
  assert.equal(await page.textContent('.hc-step[data-docker] b'), 'Starting Docker…');
  if (process.env.SHOT_DIR) await page.screenshot({ path: process.env.SHOT_DIR + '/docker-starting.png' });
  await page.waitForSelector('.hc-ready[hidden]', { state: 'attached', timeout: 15000 });
  assert.equal(await page.evaluate(() => window.__started), 1, 'one start, no command from the page');
  assert.deepEqual((await calls(page, 'docker_start'))[0], undefined, 'docker_start takes nothing from the page');
  assert.match(await page.textContent('.hc-start .hc-status'), /Docker is running/);
  await page.close();
});

test('three presses from Host to "You\'re live": no window, the steps here, then the console says so', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, colorScheme: 'dark' });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => { window.__noKit = true; window.__jobSteps = 8; });
  await page.addInitScript(FAKE, false);
  await page.goto(INDEX);
  let presses = 0;
  const press = async (sel) => { presses++; await page.click(sel); };
  await press('.nav[data-page="host"]');                  // 1: Host
  await page.waitForSelector('#hc-get-pack');
  await press('#hc-get-pack');                            // 2: Get the Host pack
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'set_host_kit'));
  await page.waitForFunction(() => document.querySelector('#hc-start') && !document.querySelector('#hc-start').disabled);
  assert.equal(await page.textContent('#hc-start'), 'Start Dalaran hangout');
  await page.fill('#hc-gm', 'hostguy');                   // typing, not a press (Mac and Linux only)
  if (process.env.SHOT_DIR) await page.screenshot({ path: process.env.SHOT_DIR + '/ready-to-start.png' });
  await press('#hc-start');                               // 3: Start
  await page.waitForSelector('.hc-prog:not([hidden]) .hc-bar');
  await page.waitForFunction(() => /Step [3-6] of 8/.test(document.querySelector('.hc-prog').textContent));
  assert.ok(await page.isHidden('#hc-start'), 'no second start while one runs');
  if (process.env.SHOT_DIR) await page.screenshot({ path: process.env.SHOT_DIR + '/setting-up.png' });
  await page.waitForSelector('.hc-live.on', { timeout: 30000 });
  assert.equal(await page.textContent('.hc-live b'), "You're live");
  assert.equal(presses, 3);
  assert.match(await page.textContent('.hc-live'), /invite link/);
  if (process.env.SHOT_DIR) await page.screenshot({ path: process.env.SHOT_DIR + '/youre-live.png' });
  await page.click('.hc-live .btn');
  assert.equal((await calls(page, 'host_start')).length, 0, 'never a window');
  assert.doesNotMatch(await page.textContent('#page-host'), NO_BRANDS);
  assert.deepEqual(errors, []);
  await page.close();
});

test('a start that stops says why in plain words, with Try again and the window as a way out', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, colorScheme: 'dark' });
  await page.addInitScript(() => { window.__jobFail = true; });
  await page.addInitScript(FAKE, false);
  await page.goto(INDEX);
  await page.click('.nav[data-page="host"]');
  await page.waitForFunction(() => document.querySelector('#hc-start') && !document.querySelector('#hc-start').disabled);
  await page.fill('#hc-gm', 'hostguy');
  await page.click('#hc-start');
  await page.waitForSelector('.hc-prog.bad', { timeout: 15000 });
  assert.match(await page.textContent('.hc-prog'), /Setting up stopped.*Clients page/s);
  assert.ok(await page.isVisible('.hc-prog button:has-text("Try again")'));
  if (process.env.SHOT_DIR) await page.screenshot({ path: process.env.SHOT_DIR + '/start-stopped.png' });
  await page.click('.hc-prog button:has-text("Open in a window instead")');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_start'));
  await page.close();
});

test('a server already on this PC: found, and two presses put it on Leyline with no questions', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, colorScheme: 'dark' });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    window.__found = { kind: 'docker', running: true, confirmed: true, core: 'azerothcore', client: 'wrath',
      auth: '127.0.0.1:3724', world: '127.0.0.1:8085', what: 'AzerothCore in Docker (login 3724, world 8085)' };
  });
  await page.addInitScript(FAKE, false);
  await page.goto(INDEX);
  let presses = 0;
  const press = async (sel) => { presses++; await page.click(sel); };
  await press('.nav[data-page="host"]');                  // 1: Host
  await page.waitForSelector('.hc-found:not([hidden]) #hc-put');
  assert.match(await page.textContent('.hc-found'), /Found a server on this PC.*AzerothCore in Docker \(login 3724, world 8085\)/s);
  assert.equal(await page.textContent('.hc-start .hc-bh:not(.hc-gap)'), 'Or host a new realm');
  assert.equal(await page.$('#hc-link-client'), null, 'the game it is for was told by the server itself');
  assert.equal(await page.inputValue('#hc-link-name'), 'My Server');
  if (process.env.SHOT_DIR) await page.screenshot({ path: process.env.SHOT_DIR + '/server-found.png' });
  await press('#hc-put');                                 // 2: Put it on Leyline
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_link_quiet'));
  assert.deepEqual((await calls(page, 'host_link_quiet'))[0], { answers: { name: 'My Server', auth: '127.0.0.1:3724', world: '127.0.0.1:8085', client: 'wrath', core: 'azerothcore' } });
  assert.equal((await calls(page, 'host_link_start')).length, 0, 'no window, no four questions');
  await page.waitForSelector('.hc-live.on', { timeout: 20000 });
  assert.equal(presses, 2);
  assert.equal(await page.textContent('.hc-live b'), "You're live");
  assert.match(await page.textContent('.hc-live'), /on the realm list.*WrongPass\.MaxCount = 0/s);
  if (process.env.SHOT_DIR) await page.screenshot({ path: process.env.SHOT_DIR + '/linked-live.png' });
  assert.doesNotMatch(await page.textContent('#page-host'), NO_BRANDS);
  assert.deepEqual(errors, []);
  await page.close();
});

test('found, but the era is not told or the ports alone answer: the host picks, and a stopped server is said so', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, colorScheme: 'dark' });
  await page.addInitScript(() => {
    window.__found = { kind: 'native', running: true, confirmed: true, core: 'cmangos', client: '', auth: '127.0.0.1:3724', world: '127.0.0.1:8085',
      what: 'CMaNGOS running on this PC (login 3724, world 8085)' };
  });
  await page.addInitScript(FAKE, false);
  await page.goto(INDEX);
  await page.click('.nav[data-page="host"]');
  await page.waitForSelector('#hc-link-client');
  await page.selectOption('#hc-link-client', 'tbc');
  await page.fill('#hc-link-name', 'Old Friends');
  await page.click('#hc-put');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'host_link_quiet'));
  const a = (await calls(page, 'host_link_quiet'))[0].answers;
  assert.deepEqual([a.name, a.client, a.core], ['Old Friends', 'tbc', 'cmangos']);
  await page.close();

  const p2 = await browser.newPage({ viewport: { width: 1280, height: 900 }, colorScheme: 'dark' });
  await p2.addInitScript(() => {
    window.__found = { kind: 'docker', running: false, confirmed: false, core: 'azerothcore', client: 'wrath', auth: '127.0.0.1:3724', world: '127.0.0.1:8085',
      what: 'AzerothCore in Docker, stopped' };
  });
  await p2.addInitScript(FAKE, false);
  await p2.goto(INDEX);
  await p2.click('.nav[data-page="host"]');
  await p2.waitForSelector('.hc-found:not([hidden])');
  assert.match(await p2.textContent('.hc-found'), /Your server is stopped/);
  assert.equal(await p2.$('#hc-put'), null, 'never linked while it is stopped');
  if (process.env.SHOT_DIR) await p2.screenshot({ path: process.env.SHOT_DIR + '/server-stopped.png' });
  await p2.close();
});
