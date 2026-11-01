// Modes anyone can host and fork (ui/hostmodes.js, src-tauri/src/host/modes.rs) in Chromium, with a fake Tauri core and
// the real templates (stream/modepacks.json): the Host page's first screen is the mode cards (picture, name, one line,
// who made it, players on realms running it), "Pick a ruleset yourself" goes to the ruleset picker and back; Host opens
// the short form (realm name, who can join, the mode's settings), checks it, and starts the mode's ruleset with its
// answers, then "You're live" with the share link; Fork copies a mode (based_on set) and opens its settings; a listed
// realm's mode shows on its row with "based on" and Fork this mode. No modes: the ruleset picker as before.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/hostmodes.browser.test.cjs     (SHOT_DIR=<dir> for screenshots)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const CAT = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'stream', 'modepacks.json'), 'utf8'));
const MODES = CAT.modes.map((m) => ({ id: m.id, name: m.name, line: m.line, version: m.version, author: m.author, based_on: m.based_on,
  status: m.status, profile: m.profile, scripts: m.scripts, env: m.env, mods: m.mods, den: m.den, tags: m.tags, jam: null, credit: m.credit || null,
  settings: m.settings, picture: m.picture_data || null, own: false }));
const RID = 'abcdefghijk2345a';
const SHOTS = process.env.SHOT_DIR;
const shot = async (page, name, full) => { if (SHOTS) { await page.waitForTimeout(500); await page.screenshot({ path: path.join(SHOTS, name), fullPage: !!full }); } };   // after the page fade
const NO_BRANDS = /warcraft|\bwow\b|lich king|blizzard/i;

const FAKE = (opts) => {
  const calls = (window.__calls = []);
  let modes = JSON.parse(JSON.stringify(opts.modes));
  let realm = null;
  const recipe = Object.assign({}, modes.find((m) => m.id === 'duel-ladder'), { id: 'ann-duels', name: 'Ann duels', author: 'Ann',
    based_on: { id: 'duel-ladder', author: 'Leyline' } });
  delete recipe.picture;
  const listed = [
    { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', version: '3.3.5a', build: '3.3.5a', core: 'azerothcore', tags: [], players: 5, bots: 0, address: 'play.openvanilla.app', mods: [], mode: null },
    { slug: 'c-aaaa', realm_id: 'aaaaaaaaaaaaaaaa', name: 'Duel Town', official: false, state: 'live', version: '3.3.5a', build: '3.3.5a', core: 'azerothcore', tags: [], players: 7, bots: 0, mods: [],
      mode: { id: 'duel-ladder', name: 'Duel ladder', author: 'Leyline', forkable: true, recipe: modes.find((m) => m.id === 'duel-ladder') } },
    { slug: 'c-bbbb', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Ann Arena', official: false, state: 'live', version: '3.3.5a', build: '3.3.5a', core: 'azerothcore', tags: [], players: 3, bots: 0, mods: [],
      mode: { id: 'ann-duels', name: 'Ann duels', author: 'Ann', based_on: { id: 'duel-ladder', author: 'Leyline' }, forkable: true, recipe } },
    { slug: 'c-cccc', realm_id: 'cccccccccccccccc', name: 'Quiet Den', official: false, state: 'offline', version: '3.3.5a', build: '3.3.5a', core: 'azerothcore', tags: [], players: 0, bots: 0, mods: [],
      mode: { id: 'den', name: 'My Den', author: 'Leyline', forkable: true, recipe: modes.find((m) => m.id === 'den') } },
  ];
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c1', name: 'Mine', path: '/g', build: '3.3.5a' }], selected_client: 'c1', selected_realm: 'play.openvanilla.app',
        wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: false, data_dir: '/tmp' };
      case 'deep_link_take': return null;
      case 'host_modes': if (opts.noModes) throw 'unknown host_modes'; return JSON.parse(JSON.stringify(modes));
      case 'host_mode_fork': {
        const src = args.listed || modes.find((m) => m.id === args.id);
        const f = Object.assign({}, src, { id: args.id + '-2', name: 'My ' + src.name, author: 'Host', based_on: { id: src.id, author: src.author }, own: true,
          picture: (modes.find((m) => m.id === (src.based_on ? src.based_on.id : src.id)) || {}).picture || null });
        modes.push(f);
        return f;
      }
      case 'host_mode_save': {
        const m = modes.find((x) => x.id === args.id);
        Object.assign(m, { name: args.edits.name, line: args.edits.line, author: args.edits.author, jam: args.edits.jam || null });
        for (const s of m.settings) if (s.key in args.edits.defaults) s.default = args.edits.defaults[s.key];
        return m;
      }
      case 'host_mode_remove': modes = modes.filter((m) => m.id !== args.id); return null;
      case 'host_mode_use': if (!args.id) return null; { const m = modes.find((x) => x.id === args.id); return { profile: m.profile, name: m.name, den: null }; }
      case 'host_info': if (window.__jobDone && !realm) realm = window.__jobDone;
        return { dir: '/home/me/kit', is_kit: !opts.noKit, has_windows_script: true, has_unix_script: true, problems: [], seeds: [], realm };
      case 'host_console': return { set_up: !!realm, linked: false, name: realm ? realm.name : '', realm_id: realm ? RID_ : '', profile: realm ? realm.profile : 'dalaran', core: 'azerothcore',
        visibility: realm ? realm.visibility : 'private', signup: 'invite', herokey: false, claim_sent: false, browser: true, max_players: 0, relay: 'relay.openvanilla.app', relay_connected: true,
        relay_error: null, docker: true, services: [], uptime: '1 min', gm: true, gm_note: null, invite: 'https://relay.openvanilla.app/join/' + RID_ + '#secret' };
      case 'host_status': return { realm_id: args.realmId, online: window.__online !== false, name: realm ? realm.name : '', visibility: realm ? realm.visibility : 'private', players: 0, bots: 0, listed: true };
      case 'host_invite': return 'https://relay.openvanilla.app/join/' + RID_ + '#secret';
      case 'host_gm': return 'gm=1\nup=60\nplayers=0\nbots=0\nrestart=0\nallow=off\nallowed=0\nvis=public\nmotd=-';
      case 'host_profiles': return [[
        { id: 'brawl', label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c1', name: 'Mine', path: '/g' }], note: null },
        { id: 'dalaran', label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c1', name: 'Mine', path: '/g' }], note: null },
        { id: 'den', label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c1', name: 'Mine', path: '/g' }], note: null },
        { id: 'main', label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c1', name: 'Mine', path: '/g' }], note: null },
      ], null];
      case 'host_choose': return { profile: args.profile, client_dir: '' };
      case 'host_start_quiet': window.__jobAt = 0; window.__answers = args.answers; return 'Setting up your realm.';
      case 'host_progress': {
        if (window.__jobAt === undefined) return { idle: true, kind: '', running: false, code: null, step: '', n: 0, of: 0, secs: 0, problem: null };
        const n = Math.min(++window.__jobAt, 3);
        const done = n >= 3;
        if (done && !window.__jobDone) window.__jobDone = { realm_id: RID_, name: window.__answers.name, visibility: window.__answers.visibility, relay: 'relay.openvanilla.app', profile: 'brawl' };
        return { idle: false, kind: 'host', running: !done, code: done ? 0 : null, step: done ? 'Done' : ['Getting the helper', 'Starting the servers'][n - 1], n, of: 3, secs: n * 30, problem: null };
      }
      case 'hostpack_info': return { installed: { version: '7' }, available: { version: '7' }, update: false, kit_dir: '/home/me/kit' };
      case 'docker_info': return { state: 'ready', version: '27', help: 'Docker is ready.', get_url: 'https://docs.docker.com/desktop/' };
      case 'host_scan': return null;
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'realms_list': return { realms: listed, network: { realms: 3, players: 10, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'open_link': return null;
      case 'themes_list': return { active: opts.light ? 'light' : 'dark', themes: [] };
      default: throw 'unknown ' + cmd;
    }
  } } };
  var RID_ = opts.rid;   // eslint-disable-line no-var
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(o = {}) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, Object.assign({ modes: MODES, rid: RID }, o));
  if (o.light) await page.addInitScript(() => { try { localStorage.setItem('leyline.theme', 'light'); } catch (_) { /* */ } });
  await page.goto(INDEX);
  await page.click('.nav[data-page="host"]');
  await page.waitForFunction(() => document.getElementById('hc-pane-main').classList.contains('on'));
  return { page, errors };
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c).map(([, a]) => a), cmd);

test('the first screen is the mode cards; Pick a ruleset yourself goes to the picker and back', async () => {
  const { page, errors } = await open();
  await page.waitForSelector('#hx .hx-card');
  // the realm list (for the players on realms running each mode) comes in from the Realms page's load
  await page.evaluate(() => window.LeylineNav && window.LeylineNav.show('realms'));
  await page.waitForSelector('.realm-card .rl-mode');
  await page.click('.nav[data-page="host"]');
  const cards = await page.$$eval('#hx .hx-card', (l) => l.map((c) => [c.dataset.mode, c.querySelector('.hx-name').firstChild.textContent, !!c.querySelector('.hx-pic img[src^="data:image/svg+xml"]'),
    !!c.querySelector('.hx-host')]));
  assert.deepEqual(cards.map((c) => c[0]), ['levelling', 'solo-leveling', 'duel-ladder', 'hide-and-seek', 'minigolf-night', 'survivors'],
    'no Den template: Your Den has its own page');
  assert.ok(cards.every((c) => c[2]), 'every card has its picture');
  assert.deepEqual(cards.filter((c) => !c[3]).map((c) => c[0]), ['survivors'], 'a coming mode has no Host');
  assert.equal(await page.textContent('[data-mode="duel-ladder"] .hx-live'), '1 realm live · 7 playing');
  assert.equal(await page.$('[data-mode="duel-ladder"] .hx-by'), null, 'our own modes carry no "by" line');
  assert.ok(await page.isHidden('.hc-setup'), 'the ruleset picker waits underneath');
  await shot(page, 'host-grid.png', true);
  await page.click('.hx-other');
  await page.waitForSelector('.hc-setup', { state: 'visible' });
  assert.ok(await page.isVisible('.hx-back-bar'));
  assert.deepEqual((await calls(page, 'host_mode_use')).at(-1), { id: null, values: {} }, 'a plain ruleset: no mode');
  await page.click('.hx-back-bar .hx-back');
  await page.waitForSelector('#hx .hx-card');
  assert.ok(await page.isHidden('.hc-setup'));
  assert.doesNotMatch(await page.textContent('#hx'), NO_BRANDS);
  assert.deepEqual(errors, []);
});

test('Host: the short form, checked, then the mode\'s ruleset with its answers, then You\'re live with the share link', async () => {
  const { page, errors } = await open();
  await page.click('[data-mode="duel-ladder"] .hx-host');
  await page.waitForSelector('#hx-name');
  assert.equal(await page.inputValue('#hx-name'), 'Duel ladder');
  assert.deepEqual(await page.$$eval('.hx-f .hx-l', (l) => l.map((x) => x.textContent)),
    ['Shortest duel that counts', 'Rated duels per pair a day', 'Points for a win', 'Win streak lines']);
  assert.ok(await page.isVisible('#hx-vis'), 'who can join');
  await shot(page, 'host-form.png', true);
  await page.fill('#hx-name', 'Duel Town Two');
  await page.fill('#hx-gm', 'HOSTER');
  await page.fill('.hx-f[data-key="min_s"] input', '1000');
  await page.click('#hx-go');
  await page.waitForFunction(() => document.querySelector('.hx-status').textContent === 'Shortest duel that counts: 5 to 120.');
  assert.equal((await calls(page, 'host_mode_use')).length, 0, 'nothing started');
  await page.fill('.hx-f[data-key="min_s"] input', '30');
  await page.uncheck('.hx-f[data-key="streaks"] input');
  await page.click('#hx-vis .hc-seg-b[data-value="public"]');
  await page.click('#hx-go');
  await page.waitForSelector('.hx-run .hc-prog');
  assert.deepEqual((await calls(page, 'host_mode_use'))[0], { id: 'duel-ladder', values: { min_s: 30, per_day: 3, win_points: 3, streaks: false } });
  assert.deepEqual((await calls(page, 'host_choose'))[0], { profile: 'brawl', clientId: 'c1' });
  assert.deepEqual((await calls(page, 'host_start_quiet'))[0], { answers: { name: 'Duel Town Two', visibility: 'public', gm: 'HOSTER' } });
  await shot(page, 'host-starting.png');
  await page.waitForSelector('.hx-live-box', { timeout: 15000 });
  assert.equal(await page.textContent('.hx-live-box .hc-live-t b'), "You're live");
  assert.equal(await page.textContent('#hx-share'), `https://play.openvanilla.app/wrath/r/${RID}`);
  assert.match(await page.textContent('.hx-live-box'), /Duel Town Two runs Duel ladder and is on the realm list/);
  await shot(page, 'host-live.png', true);
  await page.click('.hx-live-box button:has-text("Open the console")');
  await page.waitForFunction(() => !document.getElementById('hx'));
  assert.deepEqual(errors, []);
});

test('a Den mode (a fork of it, or opened by id) is always private: no who-can-join, and the live link is the invite', async () => {
  const { page, errors } = await open();
  await page.waitForSelector('#hx .hx-card');
  await page.evaluate(() => window.LeylineHostModes.open('den'));
  await page.waitForSelector('#hx-name');
  assert.equal(await page.locator('#hx-vis').count(), 0);
  assert.equal(await page.inputValue('.hx-f[data-key="look"] select'), 'white');
  await page.selectOption('.hx-f[data-key="look"] select', 'wood');
  await page.fill('#hx-gm', 'HOSTER');
  await page.click('#hx-go');
  await page.waitForSelector('.hx-live-box', { timeout: 15000 });
  assert.deepEqual((await calls(page, 'host_mode_use'))[0], { id: 'den', values: { look: 'wood', budget: 1500 } });
  assert.equal((await calls(page, 'host_start_quiet'))[0].answers.visibility, 'private');
  assert.equal(await page.textContent('#hx-share'), `https://relay.openvanilla.app/join/${RID}#secret`);
  assert.deepEqual(errors, []);
});

test('Fork: a copy with based_on set opens its settings; saved, its card says whose and on what it is based', async () => {
  const { page, errors } = await open();
  await page.click('[data-mode="duel-ladder"] .hx-fork');
  await page.waitForSelector('#hx-e-name');
  assert.deepEqual((await calls(page, 'host_mode_fork'))[0], { id: 'duel-ladder', listed: null, author: null });
  assert.equal(await page.inputValue('#hx-e-name'), 'My Duel ladder');
  assert.equal(await page.inputValue('#hx-e-author'), '', 'your name, typed by you');
  assert.match(await page.textContent('.hx-head .hx-by'), /based on Duel ladder by Leyline/);
  await shot(page, 'fork-settings.png', true);
  await page.click('.hx-save');
  await page.waitForFunction(() => document.querySelector('.hx-status').textContent.startsWith('Type your name'));
  await page.fill('#hx-e-name', "Ann's duels");
  await page.fill('#hx-e-author', 'Ann');
  await page.fill('#hx-e-jam', 'first-jam');
  await page.fill('.hx-f[data-key="per_day"] input', '5');
  await page.click('.hx-save');
  await page.waitForSelector('[data-mode="duel-ladder-2"]');
  assert.deepEqual((await calls(page, 'host_mode_save'))[0], { id: 'duel-ladder-2', edits: { name: "Ann's duels", line: MODES[2].line, author: 'Ann', jam: 'first-jam',
    defaults: { min_s: 15, per_day: 5, win_points: 3, streaks: true } } });
  assert.equal(await page.textContent('[data-mode="duel-ladder-2"] .hx-by'), 'based on Duel ladder by Leyline · by Ann');
  assert.ok(await page.isVisible('[data-mode="duel-ladder-2"] .hx-chip'), 'Yours');
  assert.ok(await page.isVisible('[data-mode="duel-ladder-2"] .hx-change') && await page.isVisible('[data-mode="duel-ladder-2"] .hx-remove'));
  await shot(page, 'grid-with-fork.png', true);
  page.once('dialog', (d) => d.accept());
  await page.click('[data-mode="duel-ladder-2"] .hx-remove');
  await page.waitForFunction(() => !document.querySelector('[data-mode="duel-ladder-2"]'));
  assert.deepEqual(errors, []);
});

test('the Realms page: a realm\'s mode and its credit on its row, and Fork this mode', async () => {
  const { page, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('realms'));
  await page.waitForSelector('.realm-card .rl-mode');
  const rows = await page.$$eval('.realm-card', (l) => l.map((c) => [c.querySelector('.name').textContent, (c.querySelector('.rl-mode') || {}).textContent || '', !!c.querySelector('.rl-fork')]));
  assert.deepEqual(rows.find((r) => r[0] === 'Ann Arena'), ['Ann Arena', 'Mode: Ann duels by Ann · based on Duel ladder by Leyline', true]);
  assert.deepEqual(rows.find((r) => r[0] === 'Duel Town'), ['Duel Town', 'Mode: Duel ladder by Leyline', true]);
  assert.equal(rows.find((r) => r[1] === '' && r[0] !== 'Ann Arena')[2], false, 'no mode: no Fork');
  await page.locator('.realm-card', { hasText: 'Ann Arena' }).locator('.rl-mode').scrollIntoViewIfNeeded();
  await shot(page, 'realm-rows.png');
  await page.locator('.realm-card', { hasText: 'Ann Arena' }).locator('.rl-fork').click();
  await page.waitForSelector('#hx-e-name');
  const f = (await calls(page, 'host_mode_fork'))[0];
  assert.equal(f.id, 'ann-duels');
  assert.equal(f.listed.based_on.id, 'duel-ladder', 'the realm list\'s recipe goes to the core, which checks it again');
  assert.ok(await page.isVisible('#page-host.on'), 'the Host page, on the fork\'s settings');
  assert.match(await page.textContent('.hx-head .hx-by'), /based on Ann duels by Ann/);
  assert.deepEqual(errors, []);
});

test('in the Light theme too', async () => {
  const { page, errors } = await open({ light: true });
  await page.waitForSelector('#hx .hx-card');
  await shot(page, 'host-grid-light.png');
  assert.deepEqual(errors, []);
});

test('no modes from the core: the ruleset picker as before', async () => {
  const { page, errors } = await open({ noModes: true });
  await page.waitForSelector('.hc-setup .hc-rs');
  assert.equal(await page.locator('#hx').count(), 0);
  assert.ok(await page.isVisible('.hc-setup'));
  assert.deepEqual(errors, []);
});
