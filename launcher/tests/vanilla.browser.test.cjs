// The original world in one click, Quick start, Characters and Invite a friend from a single player world (ui/vanilla.js,
// ui/characters.js, ui/hostfriends.js; app.rs world_* / local_characters / online_characters / world_invite;
// docs/SINGLE-PLAYER-ONE-CLICK.md), in Chromium with a fake Tauri core:
//   * the Single player page's top card: "Play the original world (level 60) with companions" sets the world up with
//     the game folder (world_setup kind vanilla), follows its own progress line, then plays it (world_play): the game
//     signs in by itself; Home's Original world button does the same;
//   * Quick start: the first era's races only, each with its own classes; the pick, the body and the name go to
//     world_play's quick;
//   * another world running here (one at a time): said, with "Stop it and play" (never stopped unasked);
//   * not on Windows: Vanilla+ on the Host page (Docker);
//   * Characters: On this PC (each world, its characters, Play enters that one; a stopped world is started first) and
//     Online (the official realm, the names the game folder played, Play joins as the realm's card does);
//   * Play with friends: the original world's tile when it is set up, Invite a friend is world_invite, the checklist
//     for that world; no brand words, no page errors.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/vanilla.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const PICTURE = path.join(__dirname, '..', 'ui', 'art', 'leyline-bg.webp');
const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const NET = [{ slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 5, bots: 0, address: 'play.openvanilla.app' }];
const ES = (o) => Object.assign({ supported: true, installed: false, can_get: true, set_up: false, running: false, update: false, account: '', password: '',
  kind: 'vanilla', realm_name: 'Original world', other_running: null }, o);
const READY = { installed: true, can_get: false, maps: true, db: true, filled: true, account_made: true, set_up: true, running: true, db_up: true, auth_up: true, world_up: true, account: 'ORIGINAL', password: 'ABCD2345' };
const FIRST_ERA = [
  { id: 1, label: 'Human', side: 'alliance', classes: [[1, 'Warrior'], [2, 'Paladin'], [4, 'Rogue'], [5, 'Priest'], [8, 'Mage'], [9, 'Warlock']] },
  { id: 2, label: 'Orc', side: 'horde', classes: [[1, 'Warrior'], [3, 'Hunter'], [4, 'Rogue'], [7, 'Shaman'], [9, 'Warlock']] },
  { id: 3, label: 'Dwarf', side: 'alliance', classes: [[1, 'Warrior'], [2, 'Paladin'], [3, 'Hunter'], [4, 'Rogue'], [5, 'Priest']] },
  { id: 4, label: 'Night Elf', side: 'alliance', classes: [[1, 'Warrior'], [3, 'Hunter'], [4, 'Rogue'], [5, 'Priest'], [11, 'Druid']] },
  { id: 5, label: 'Undead', side: 'horde', classes: [[1, 'Warrior'], [4, 'Rogue'], [5, 'Priest'], [8, 'Mage'], [9, 'Warlock']] },
  { id: 6, label: 'Tauren', side: 'horde', classes: [[1, 'Warrior'], [3, 'Hunter'], [7, 'Shaman'], [11, 'Druid']] },
  { id: 7, label: 'Gnome', side: 'alliance', classes: [[1, 'Warrior'], [4, 'Rogue'], [8, 'Mage'], [9, 'Warlock']] },
  { id: 8, label: 'Troll', side: 'horde', classes: [[1, 'Warrior'], [3, 'Hunter'], [4, 'Rogue'], [5, 'Priest'], [7, 'Shaman'], [8, 'Mage']] },
];
const TAMSIN = { name: 'Tamsin', race: 4, class: 11, female: true, level: 14, played: 7260, last: 1762400000, race_label: 'Night Elf', class_label: 'Druid', side: 'alliance' };
const BRAKKA = { name: 'Brakka', race: 2, class: 1, female: false, level: 60, played: 360000, last: 1762300000, race_label: 'Orc', class_label: 'Warrior', side: 'horde' };

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  window.__worlds = o.worlds;          // kind -> world_status
  window.__eprog = { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7 };
  window.__left = 0;                   // engine_progress calls still "running"
  const st = { version: 'test', settings: { clients: o.clients, selected_client: o.clients[0] ? o.clients[0].id : null, selected_realm: 'play.openvanilla.app',
    wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
  const W = (k) => JSON.parse(JSON.stringify(window.__worlds[k] || { supported: true, set_up: false, running: false, other_running: null }));
  const starting = (kind) => { window.__left = 1; window.__eprog = { idle: false, kind, running: true, code: null, step: 'Reading the maps from your game folder', n: 3, of: 7, pct: 40, detail: '' }; };
  const job = { single: 'engine', vanilla: 'vanilla', bossrush: 'bossrush' };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'select': return null;
      case 'realms_list': return { realms: o.net, network: { realms: 1, players: 5, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'docker_info': return { state: 'ready', version: '27', help: '', get_url: '', can_start: false };
      case 'engine_status': return W('single');
      case 'host_console': throw 'no kit';
      case 'host_progress': return { idle: true, kind: 'host', running: false, code: null };
      case 'social_status': return true;
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      // the worlds (app.rs world_*)
      case 'world_status': return W(args.kind);
      case 'world_setup': case 'world_start': starting(job[args.kind]); return 'Starting…';
      case 'engine_progress':
        if (window.__eprog.running) {
          if (window.__left-- <= 0) {
            window.__eprog = Object.assign({}, window.__eprog, { running: false, code: 0, step: 'Ready', n: 7 });
            const k = Object.keys(job).find((x) => job[x] === window.__eprog.kind);
            window.__worlds[k] = Object.assign({}, window.__worlds[k] || {}, { set_up: true, running: true, other_running: null });
          }
        }
        return JSON.parse(JSON.stringify(window.__eprog));
      case 'engine_stop': case 'world_stop': {
        const k = cmd === 'engine_stop' ? 'single' : args.kind;
        window.__worlds[k] = Object.assign({}, window.__worlds[k], { running: false });
        for (const x of Object.keys(window.__worlds)) if (window.__worlds[x]) window.__worlds[x].other_running = null;
        return 'Stopped. Everyone was saved.';
      }
      case 'world_choices': return { races: o.races, first_era: true, name: 'Wren' };
      case 'world_play': return { message: 'Started My Wrath on 127.0.0.1:3724.', kind: args.kind, auto: true, account: 'ORIGINAL', password: '', made: args.quick ? (args.quick.name || 'Wren') : '' };
      case 'local_characters': return JSON.parse(JSON.stringify(o.local || []));
      case 'online_characters': return { realm: 'Wrath Unleashed', chars: [{ name: 'Odo', realm: 'Wrath Unleashed' }] };
      case 'play_realm': return 'Started My Wrath on Wrath Unleashed.';
      // Play with friends
      case 'den_state': return { lite: { available: true, can_get: true, running: false, address: '127.0.0.1', account: '', password: '' } };
      case 'world_invite': return { link: 'https://relay.leylinelauncher.com/join/vvvvvvvvvvvvvvvv#WORLDSECRETWORLDSE~~GABCDE2345.ABCDEFGH2345~~lan=192.168.1.20:3724', code: 'DEN-X', name: 'your friend', expires: 0,
        tunnel: { running: true, connected: true }, ways: [] };
      case 'host_ready': return { kind: args.kind, ok: true, relay: 'relay.leylinelauncher.com', realm_id: 'vvvvvvvvvvvvvvvv', relay_ms: 30, path_ms: 80, os: 'windows', diagnostics: '[ok] server',
        checks: [{ id: 'server', state: 'ok', label: 'Your world is running', line: 'Your world is running on this PC.', fix: '', action: '' }] };
      case 'open_link': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(o) {
  const opts = Object.assign({ clients: [WRATH], net: NET, races: FIRST_ERA, worlds: { vanilla: ES({}) } }, o);
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('https://play.openvanilla.app/**', (r) => r.fulfill({ path: PICTURE, contentType: 'image/webp' }));
  await page.addInitScript(FAKE, opts);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineVanilla && window.LeylineCharacters && window.LeylineWorlds);
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const noBrand = async (page) => assert.doesNotMatch(await page.evaluate(() => document.body.innerText), /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i);

test('one click: the original world is set up with the game folder, its own progress followed, then the game signs in by itself', async () => {
  const { page, ctx, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#sv-play');
  assert.equal((await page.textContent('#sv-play')).trim(), 'Play the original world (level 60) with companions');
  assert.match(await page.textContent('#sv'), /Level 60 · Companions/);
  assert.match(await page.textContent('#sv'), /never downloads or ships the game/);
  await page.click('#sv-play');
  await page.waitForSelector('#sv-prog');
  assert.match(await page.textContent('#sv-prog'), /Reading the maps from your game folder/);
  await page.waitForFunction(() => document.getElementById('sv-status').textContent.startsWith('Started'), null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_setup'), [{ client: 'c335a-1', kind: 'vanilla' }]);
  assert.deepEqual(await callsOf(page, 'world_play'), [{ client: 'c335a-1', kind: 'vanilla' }], 'no Quick start, no pick: character select');
  await page.waitForSelector('#sv-run');
  assert.equal(await page.textContent('#sv-run'), 'Running');
  assert.equal(await page.isEnabled('#sv-invite'), true);
  // the next press plays straight away (set up and running)
  await page.click('#sv-play');
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'world_play').length === 2);
  assert.equal((await callsOf(page, 'world_setup')).length, 1);
  assert.equal((await callsOf(page, 'world_start')).length, 0);
  await noBrand(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Home: Original world plays it in one click, Characters opens the page', async () => {
  const { page, ctx, errors } = await open({ worlds: { vanilla: ES(READY) } });
  await page.click('#hm-vanilla-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'));
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-single');
  await page.click('.nav[data-page="play"]');
  await page.click('#hm-chars');
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-characters');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Quick start: the first era\'s races, each with its classes; the pick goes to world_play', async () => {
  const { page, ctx, errors } = await open({ worlds: { vanilla: ES(READY) } });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.click('#sv-quick-open');
  await page.waitForSelector('#sv-race option', { state: 'attached' });
  const races = await page.$$eval('#sv-race option', (o) => o.map((x) => x.textContent));
  assert.deepEqual(races, ['Human', 'Orc', 'Dwarf', 'Night Elf', 'Undead', 'Tauren', 'Gnome', 'Troll'], 'no later races');
  assert.equal(await page.inputValue('#sv-name'), 'Wren', 'a free name suggested');
  await page.selectOption('#sv-race', '4');
  const classes = await page.$$eval('#sv-class option', (o) => o.map((x) => x.textContent));
  assert.deepEqual(classes, ['Warrior', 'Hunter', 'Rogue', 'Priest', 'Druid']);
  await page.selectOption('#sv-class', '11');
  await page.selectOption('#sv-sex', '1');
  await page.fill('#sv-name', 'Tamsin');
  await page.click('#sv-quick-go');
  await page.waitForFunction(() => document.getElementById('sv-status').textContent.includes('making Tamsin'));
  assert.deepEqual(await callsOf(page, 'world_play'), [{ client: 'c335a-1', kind: 'vanilla', quick: { name: 'Tamsin', race: 4, class: 11, female: true } }]);
  // a class the new race can't be is moved to one it can
  await page.selectOption('#sv-race', '6');
  assert.deepEqual(await page.$$eval('#sv-class option', (o) => o.map((x) => x.textContent)), ['Warrior', 'Hunter', 'Shaman', 'Druid']);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('another world running here: said, with Stop it and play; never stopped unasked', async () => {
  const { page, ctx, errors } = await open({ worlds: { vanilla: ES(Object.assign({}, READY, { running: false, other_running: 'single' })), single: ES({ kind: 'single', set_up: true, running: true }) } });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.click('#sv-play');
  await page.waitForSelector('#sv-other');
  assert.match(await page.textContent('#sv-other'), /Single player is running on this PC, and one world runs at a time/);
  assert.equal((await callsOf(page, 'engine_stop')).length, 0, 'not stopped unasked');
  await page.click('#sv-other-go');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'), null, { timeout: 15000 });
  assert.equal((await callsOf(page, 'engine_stop')).length, 1);
  assert.deepEqual(await callsOf(page, 'world_start'), [{ kind: 'vanilla' }], 'set up already: started');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('not on Windows: Vanilla+ on the Host page, in Docker', async () => {
  const { page, ctx, errors } = await open({ worlds: { vanilla: ES({ supported: false, can_get: false }) } });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#sv-docker');
  await page.evaluate(() => document.addEventListener('leyline:host-ruleset', (e) => { window.__ruleset = e.detail.profile; }));
  await page.click('#sv-docker-go');
  assert.equal(await page.evaluate(() => window.__ruleset), 'vanillaplus');
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-host');
  assert.equal((await callsOf(page, 'world_setup')).length, 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Characters: On this PC by world, Play enters that character (a stopped world started first); Online: the official realm', async () => {
  const local = [
    { kind: 'single', title: 'Single player', realm: 'Single player', supported: true, set_up: true, running: false, other_running: null, live: false, at: 5, chars: [BRAKKA] },
    { kind: 'vanilla', title: 'Original world', realm: 'Original world', supported: true, set_up: true, running: true, other_running: null, live: true, at: 9, chars: [TAMSIN] },
    { kind: 'bossrush', title: 'Boss Rush', realm: 'Boss Rush', supported: true, set_up: false, running: false, other_running: null, live: false, at: 0, chars: [] },
  ];
  const { page, ctx, errors } = await open({ local, worlds: { vanilla: ES(READY), single: ES({ kind: 'single', set_up: true, running: false }) } });
  await page.evaluate(() => window.LeylineNav.show('characters'));
  await page.waitForSelector('#ch-world-vanilla li[data-name="Tamsin"]');
  assert.match(await page.textContent('#ch-pane-local'), /On this PC: just you \(and friends you invite\)/);
  assert.match(await page.textContent('#ch-world-vanilla'), /Level 14 Night Elf Druid/);
  assert.match(await page.textContent('#ch-world-vanilla'), /2 h 1 min played/);
  assert.match(await page.textContent('#ch-world-single'), /As they were when this world last ran/);
  assert.match(await page.textContent('#ch-world-bossrush'), /Not set up/);
  await page.click('#ch-world-vanilla li[data-name="Tamsin"] button');
  await page.waitForFunction(() => window.__calls.some(([c]) => c === 'world_play'));
  assert.deepEqual(await callsOf(page, 'world_play'), [{ client: 'c335a-1', kind: 'vanilla', pick: 'Tamsin' }]);
  // a stopped world: started, then that character
  await page.waitForSelector('#ch-world-single li[data-name="Brakka"] button:not([disabled])');
  await page.click('#ch-world-single li[data-name="Brakka"] button');
  await page.waitForFunction(() => window.__calls.filter(([c]) => c === 'world_play').length === 2, null, { timeout: 15000 });
  assert.deepEqual(await callsOf(page, 'world_start'), [{ kind: 'single' }]);
  assert.deepEqual((await callsOf(page, 'world_play'))[1], { client: 'c335a-1', kind: 'single', pick: 'Brakka' });
  // Online
  await page.click('#ch-tab-online');
  await page.waitForSelector('#ch-pane-online li[data-name="Odo"]');
  assert.match(await page.textContent('#ch-pane-online'), /Online: a shared realm/);
  assert.match(await page.textContent('#ch-world-online h3'), /Wrath Unleashed/);
  await page.evaluate(() => { window.LeylineApp.playRealm = (r) => { window.__played = r.slug; }; });
  await page.click('#ch-online-play');
  assert.equal(await page.evaluate(() => window.__played), 'official');
  await noBrand(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Play with friends: the original world\'s tile, Invite a friend is world_invite, the checklist for that world', async () => {
  const { page, ctx, errors } = await open({ worlds: { vanilla: ES(READY) } });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#sv-invite:not([disabled])');
  await page.click('#sv-invite');
  await page.waitForSelector('#wf-pick-vanilla');
  assert.equal(await page.getAttribute('#wf-pick-vanilla', 'aria-pressed'), 'true');
  assert.equal(await page.locator('#wf-pick-single').count(), 0, 'Single player isn\'t set up here: no tile');
  await page.click('#wf-invite');
  await page.waitForFunction(() => !document.getElementById('wf-link-box').hidden);
  assert.deepEqual(await callsOf(page, 'world_invite'), [{ kind: 'vanilla' }]);
  assert.deepEqual(await callsOf(page, 'host_ready'), [{ kind: 'vanilla' }]);
  assert.match(await page.inputValue('#wf-link'), /~~GABCDE2345\./);
  assert.equal((await callsOf(page, 'den_invite')).length, 0, 'not Your Den');
  assert.deepEqual(errors, []);
  await ctx.close();
});
