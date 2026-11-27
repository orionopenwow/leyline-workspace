// Your Den (ui/den.js, ui/den.css; src-tauri host/yourden.rs, host/lite.rs) in Chromium with a fake Tauri core: the
// entries (the side menu right under Home, the Home banner), the one click (Den Lite straight in; without Den Lite the
// Docker Den's three steps here; a Docker Den: start it if stopped, its account filled in, then play), Share with
// friends (the Lite Den moved to the Docker Den, then opened there unchanged), Invite a friend, Saved Dens (open, save a
// copy, add a file, add from a Workshop link), Packs, Share on the Workshop (List your mod in its Den shape), Your Den
// on Single player's page too (#spd: the same Open my Den, Invite a friend and state), and no game or publisher names
// anywhere.
//
//   node launcher/tests/den.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.page', o.page || 'play'); } catch (_) { /* fine */ }
  const S = (window.__S = {
    lite: o.lite, selected: o.selected, kit: !!o.kit, docker_den: !!o.docker_den, world: o.world || 'ok', packInstalled: false,
    dock: o.dock || 'ready', progressDone: false, forge: o.forge || 'missing', forgeOpen: false, forgeLooks: 0, forgeLast: null, clients: o.clients === false ? [] : [{ id: 'c335a-1', name: 'My Game', path: '/x', build: '3.3.5a', realmlist: '' }],
  });
  const dens = [{ name: 'house', bytes: 300, props: 12, saved: '2026-11-01T01:00:00Z' }, { name: 'backup', bytes: 100, props: 3, saved: '2026-10-31T01:00:00Z' }];
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: S.clients, selected_client: S.clients[0] ? 'c335a-1' : null,
        selected_realm: S.selected || 'play.openvanilla.app', wine: { enabled: false, command: 'wine', prefix: '' } },
        official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 0 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c335a-1', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return S.docker_den
        ? { dir: '/kit', is_kit: true, problems: [], seeds: [], realm: { realm_id: 'abcdefghijk2345a', name: 'My Den', visibility: 'private', relay: 'relay.openvanilla.app', profile: 'den' } }
        : { dir: S.kit ? '/kit' : null, is_kit: S.kit, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return true;   // a sign-in kept (the website not asked: the sign-in screen stays closed)
      case 'den_state': return {
        lite: { available: !!S.lite && S.lite !== 'get', can_get: S.lite === 'get', running: S.lite === 'running', address: '127.0.0.1', account: 'DEN', den_dir: '/app/den-lite/state/dens' },
        kit: S.kit || S.docker_den, docker_den: S.docker_den, other_realm: null, account: S.docker_den ? 'BEA42' : '',
        saves: S.docker_den ? 'kit' : 'lite', dens: o.noDens ? [] : dens,
        packs: o.packs ? [{ id: 'goldshire', title: 'Goldshire', line: 'The inn and the square, ready to build around.' }] : [] };
      // app.rs SoloPlay: o.auto: the hybrid glue signs in by itself (Forever UI there); else the password to type
      case 'den_play_lite': if (S.lite === 'get') await new Promise((r) => setTimeout(r, 400)); S.lite = 'running'; S.selected = '127.0.0.1'; return o.auto
        ? { message: 'Started My Game on 127.0.0.1.', auto: true, why: '', account: 'DEN', password: '' }
        : { message: 'Started My Game on 127.0.0.1.', auto: false, why: "Forever UI's login screens aren't in this game folder yet (install Forever UI first).", account: 'DEN', password: 'den' };
      case 'den_open': return args.name;
      case 'den_text': return '{"format":"leyline-den","version":1,"props":[]}';
      case 'den_add': return 'friend';
      case 'den_fetch': return 'garden';
      case 'den_share_move': return 'house';
      case 'den_prefill_account': return 'BEA42';
      case 'den_join_page': return null;
      case 'host_console': return { set_up: true, profile: 'den', services: [{ role: 'world', state: S.world }], invite: null };
      case 'hostpack_info': return { available: { size: 4096 }, installed: S.packInstalled ? {} : null };
      case 'hostpack_install': S.packInstalled = true; return [{}, '/kit'];
      case 'set_host_kit': S.kit = true; return { dir: '/kit', is_kit: true, problems: [], seeds: [], realm: null };
      case 'docker_info': return { state: S.dock, can_start: S.dock === 'stopped', help: S.dock === 'ready' ? '' : 'Docker Desktop is closed.' };
      case 'docker_start': S.dock = 'ready'; return 'Starting Docker Desktop…';
      case 'host_choose': return { profile: args.profile };
      case 'host_start_quiet': return 'Setting up your realm.';
      case 'host_progress': return S.progressDone ? { idle: false, kind: 'host', running: false, code: 0, step: 'Done', n: 8, of: 8, secs: 300, problem: null }
        : { idle: false, kind: 'host', running: true, code: null, step: 'Starting the servers', n: 5, of: 8, secs: 60, problem: null };
      case 'host_invite': return 'https://relay.openvanilla.app/join/abcdefghijk2345a#INVITECODE1234567890';
      case 'join_invite': return /~~/.test(args.link)
        ? (/~~lan=/.test(args.link)
          ? { address: '192.168.1.20', name: "Your friend's Den", way: 'lan', tried: [], guest: { auto: true, account: 'GABCDE2345', password: '', why: '', message: 'Started.' } }
          : { address: 'relay.openvanilla.app:40007', name: 'Den Lite', way: 'relay', tried: [], guest: { auto: true, account: 'GABCDE2345', password: '', why: '', message: 'Started.' } })
        : { address: 'relay.openvanilla.app:40001', name: 'My Den' };
      case 'den_invite': S.net = { running: true, connected: true, relay: 'relay.openvanilla.app', realm_id: 'kzdvvj2umnduyauf', address: 'relay.openvanilla.app', auth_port: 40006, world_port: 40007, streams: 0, last_error: '' };
        return { link: 'https://relay.openvanilla.app/join/kzdvvj2umnduyauf#INVITESECRETINVITESECRET~~GABCDE2345.ABCDEFGH23456789' + (o.lan ? '~~lan=192.168.1.20:3724' : ''), name: 'Rowan', expires: 1, tunnel: S.net,
          ways: [
            o.lan ? { id: 'lan', ok: true, line: 'Same network: on, at 192.168.1.20 (friends at your home or on your network come in here first).' }
              : { id: 'lan', ok: false, line: "Same network: this PC isn't on a home or office network." },
            S.own ? { id: 'own', ok: true, line: `Your own address: ${S.own} (your router must send ports 3724 and 8085 to this PC).` }
              : { id: 'own', ok: false, line: 'Your own address: not set (only for a router you set up, or your own server; see the guide).' },
            { id: 'relay', ok: true, line: 'Leyline relay: on (works from anywhere; the relay only passes the game\'s connection through).' }] };
      case 'den_own_address': if (args.host != null) { if (/\s/.test(args.host)) throw "That isn't an address"; S.own = args.host; } return S.own || '';
      // My models (host/denmodels.rs): what den_models_add would keep, by the same first checks
      case 'den_models': return S.models || [];
      case 'den_models_add': {
        const m2 = args.files.find((f) => /\.(m2|wmo)$/i.test(f.name));
        if (!m2) throw "There's no model in that: drop the .m2 (or the building's .wmo) with its files.";
        if (atob(m2.data).slice(0, 4) !== 'MD20') throw `File ${m2.name}: that isn't a model file of the game's own kind.`;
        const slug = (args.label || m2.name.replace(/\.m2$/i, '')).toLowerCase().replace(/[^a-z0-9-]/g, '_');
        const m = { entry: 980001 + (S.models || []).length, display: 60001, name: args.label || slug, slug, kind: 'm2', model: '', box: [0, 0, 0, 1, 1, 1],
          files: args.files.map((f) => f.name), missing: ['World\\Generic\\stone.blp'], unused: [], bytes: args.files.reduce((n, f) => n + atob(f.data).length, 0) };
        S.models = (S.models || []).concat([m]);
        return m;
      }
      case 'den_models_remove': S.models = (S.models || []).filter((m) => m.slug !== args.slug); return true;
      case 'den_net_state': return S.net || { running: false, connected: false, relay: '', realm_id: '', address: '', auth_port: 0, world_port: 0, streams: 0, last_error: '' };
      case 'den_net_stop': S.net = null; return null;
      case 'play': return 'Started My Game on relay.openvanilla.app:40001.';
      // Forge (host/forge.rs): o.forge = 'missing' | 'installed'; an open Forge closes after two looks
      case 'forge_state': {
        if (S.forgeOpen && ++S.forgeLooks > 2) { S.forgeOpen = false; S.forgeLast = { seq: 1, ok: true, changed: true, chunks: 1, message: 'Terrain updated: 1 part of your Den changed. Start your game again to see it.' }; }
        return { installed: S.forge === 'installed', running: !!S.forgeOpen, exe: '', sha256: '', last: S.forgeLast || null,
          source: 'https://github.com/orionopenwow/leyline-workspace/tree/main/forge',
          licence: 'Forge is free software under the GNU GPL-3.0, built on Noggit Red by the Noggit contributors.' };
      }
      case 'forge_install': S.forge = 'installed'; return '/app/forge/Forge.exe';
      case 'forge_open': S.forgeOpen = true; S.forgeLooks = 0; return 'Forge is open on your Den.';
      case 'open_link': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts) {
  const ctx = await browser.newContext({ colorScheme: 'dark' });
  await ctx.grantPermissions(['clipboard-read', 'clipboard-write']);
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, opts || {});
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineDen && document.querySelector('.page.on'));
  return { page, ctx, errors };
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter((x) => x[0] === c).map((x) => x[1]), cmd);
const statusText = (page) => page.textContent('#yd .yd-status');
const noBrands = async (page) => assert.doesNotMatch(await page.evaluate(() => document.body.innerText), BRANDS);

test('the entries: Your Den in the realm list next to PLAY (not the side menu), and first on Home', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped' });
  const nav = await page.$$eval('aside nav .nav[data-page]', (b) => b.filter((x) => x.offsetParent !== null).map((x) => x.dataset.page));
  assert.ok(!nav.includes('den'), 'no side menu item (owner 2026-11-02)');
  await page.click('#pb-realm-btn');
  assert.ok(await page.locator('#pb-rmenu', { hasText: 'Your Den' }).count(), 'the realm list has it');
  await page.keyboard.press('Escape');
  const order = await page.$$eval('#home-more > .hm-sec', (s) => s.map((x) => x.id));
  // single player in one click leads Home since 11 Nov (oneclick.js); the Den's banner comes right after it
  assert.deepEqual(order.slice(0, 2), ['hm-oneclick', 'hm-den'], 'Play single player, then the Den, under the hero');
  assert.equal((await page.textContent('#hm-den-go')).trim(), 'Open my Den');
  assert.equal((await page.textContent('#hm-den .hm-den-txt b')).trim(), 'Your Den', 'not "Play solo": that clashed with Single player');
  await page.evaluate(() => window.LeylineNav.show('den'));   // the page itself still opens (Home's banner, the realm list)
  await page.waitForSelector('#yd-play');
  assert.match(await page.textContent('#yd-state'), /Ready on this PC: no setup needed/);
  for (const t of ['Share with friends', 'Saved Dens', 'Packs', 'Share on the Workshop']) {
    assert.ok(await page.locator('#yd .yd-tile h3', { hasText: t }).count(), t);
  }
  assert.ok(await page.locator('#yd .yd-tile .yd-hint').count() >= 4, 'a one-line hint by each');
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Den Lite: one click on Home plays straight into the room with its account', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped' });
  await page.click('#hm-den-go');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'den_play_lite'));
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-den');
  assert.deepEqual(await calls(page, 'den_play_lite'), [{ client: 'c335a-1' }]);
  await page.waitForFunction(() => /Your Den is open/.test(document.querySelector('#yd .yd-status').textContent));
  assert.match(await statusText(page), /account DEN, password den/);
  assert.match(await page.textContent('#yd-login-hint'), /signs you in by itself/, 'where it signs in by itself');
  await page.waitForFunction(() => /Open on this PC/.test(document.querySelector('#yd-state').textContent));
  assert.deepEqual(await calls(page, 'host_start_quiet'), [], 'no Docker');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('no game folder: Play says so and opens the add flow', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', clients: false, page: 'den' });
  await page.waitForSelector('#yd-play');
  await page.click('#yd-play');
  await page.waitForFunction(() => /Your Den plays with your own game folder\./.test(document.querySelector('#yd .yd-status').textContent));
  assert.equal(await page.textContent('#yd .yd-status .rl-ver'), '3.3.5a', 'the version as a chip, not in the sentence');
  assert.deepEqual(await calls(page, 'den_play_lite'), []);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('without Den Lite: the Docker Den set up here in three steps, then its first place opened', async () => {
  const { page, ctx, errors } = await open({ lite: null, dock: 'stopped', page: 'den' });
  await page.waitForSelector('#yd-play');
  assert.match(await page.textContent('#yd-state'), /Not set up yet/);
  await page.click('#yd-play');
  await page.waitForSelector('#yd .yd-step');
  assert.match(await statusText(page), /one-time setup/);
  const steps = await page.$$eval('#yd .yd-step-t b', (b) => b.map((x) => x.textContent));
  assert.deepEqual(steps, ['The Host pack', 'Docker Desktop', 'Start']);
  assert.equal(await page.isDisabled('#yd-start'), true, 'Start waits for the Host pack and Docker');
  await page.click('#yd .yd-step button:has-text("Get the Host pack")');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'set_host_kit'));
  await page.click('#yd .yd-step button:has-text("Start Docker")');
  await page.waitForFunction(() => /Docker is running/.test(document.querySelector('#yd .yd-status').textContent), null, { timeout: 10000 });
  await page.waitForFunction(() => !document.querySelector('#yd-start').disabled);
  await page.fill('#yd-acct', 'x');
  await page.click('#yd-start');
  assert.match(await statusText(page), /3 to 16 letters or digits/);
  await page.fill('#yd-acct', 'bea42');
  await page.click('#yd-start');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'host_start_quiet'));
  assert.deepEqual(await calls(page, 'den_share_move'), [], 'no Den Lite: nothing to move');
  assert.deepEqual(await calls(page, 'host_choose'), [{ profile: 'den', clientId: 'c335a-1' }]);
  assert.deepEqual(await calls(page, 'host_start_quiet'), [{ answers: { name: 'My Den', visibility: 'private', gm: 'bea42' } }]);
  await page.waitForSelector('#yd .yd-prog [role="progressbar"]');
  assert.match(await page.textContent('#yd .yd-prog'), /Starting the servers/);
  await page.evaluate(() => { window.__S.progressDone = true; window.__S.docker_den = true; });
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'den_open'), null, { timeout: 8000 });
  assert.deepEqual(await calls(page, 'den_open'), [{ name: 'forest-glade' }], 'a new Den starts in the first starter place');
  await page.waitForSelector('#yd-invite');
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Share with friends: the Lite Den moves to the Docker Den and opens there unchanged', async () => {
  const { page, ctx, errors } = await open({ lite: 'running', kit: true, page: 'den' });
  await page.waitForSelector('#yd-share');
  await page.click('#yd-share');
  await page.waitForSelector('#yd-start');
  assert.equal((await page.textContent('#yd-start')).trim(), 'Move my Den and start');
  await page.waitForFunction(() => !document.querySelector('#yd-start').disabled);
  await page.fill('#yd-acct', 'Bea42');
  await page.click('#yd-start');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'host_start_quiet'));
  const order = await page.evaluate(() => window.__calls.map((c) => c[0]).filter((c) => /den_share_move|host_choose|host_start_quiet/.test(c)));
  assert.deepEqual(order, ['den_share_move', 'host_choose', 'host_start_quiet'], 'the files move before the start');
  await page.evaluate(() => { window.__S.progressDone = true; window.__S.docker_den = true; });
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'den_open'), null, { timeout: 8000 });
  assert.deepEqual(await calls(page, 'den_open'), [{ name: 'house' }], 'the moved Den, by the name it has there');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the Docker Den: Invite a friend, Make my account, and Play with its account filled in', async () => {
  const { page, ctx, errors } = await open({ docker_den: true, kit: true, page: 'den' });
  await page.waitForSelector('#yd-invite');
  assert.match(await page.textContent('#yd-state'), /Online on this PC · private/);
  assert.equal(await page.locator('#yd-share').count(), 0, 'no Share with friends once it is the Docker Den');
  await page.click('#yd-invite');
  await page.waitForFunction(() => /Invite link copied/.test(document.querySelector('#yd .yd-status').textContent));
  assert.match(await page.evaluate(() => navigator.clipboard.readText()), /^https:\/\/relay\.openvanilla\.app\/join\//);
  await page.click('#yd button:has-text("Make my account")');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'den_join_page'));
  await page.click('#yd-play');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'join_invite'), null, { timeout: 8000 });
  const order = await page.evaluate(() => window.__calls.map((c) => c[0]).filter((c) => /den_prefill_account|join_invite/.test(c)));
  assert.deepEqual(order, ['den_prefill_account', 'join_invite']);
  assert.deepEqual(await calls(page, 'host_start_quiet'), [], 'it runs: no start');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('one click from Your Den: Invite friends and Ask Claude to edit it, beside Play', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', page: 'den' });
  await page.waitForSelector('#yd-play');
  assert.deepEqual(await page.$$eval('#yd .yd-acts button', (b) => b.map((x) => x.textContent)), ['Open my Den', 'Invite friends', 'Ask Claude to edit it']);
  // Den Lite: Invite friends puts it on the network from this PC (no Docker steps; the full test is below)
  await page.click('#yd-friends');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'den_invite'));
  assert.equal(await page.$('#yd .yd-steps'), null);
  // Ask Claude to edit it: the Agents page, where Claude is connected once
  await page.click('#yd-claude');
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-agents');
  await noBrands(page);
  await ctx.close();
  // the Docker Den: Invite friends copies the invite link
  const d = await open({ docker_den: true, kit: true, page: 'den' });
  await d.page.waitForSelector('#yd-friends');
  await d.page.click('#yd-friends');
  await d.page.waitForFunction(() => /Invite link copied/.test(document.querySelector('#yd .yd-status').textContent));
  assert.match(await d.page.evaluate(() => navigator.clipboard.readText()), /^https:\/\/relay\.openvanilla\.app\/join\//);
  assert.deepEqual([...errors, ...d.errors], []);
  await d.ctx.close();
});

test('the Docker Den stopped: one click starts it, then plays', async () => {
  const { page, ctx, errors } = await open({ docker_den: true, kit: true, world: 'stopped', page: 'den' });
  await page.waitForSelector('#yd-play');
  assert.match(await page.textContent('#yd-state'), /Stopped: Play starts it/);
  await page.click('#yd-play');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'host_start_quiet'));
  assert.deepEqual(await calls(page, 'host_start_quiet'), [{ answers: { name: null, visibility: 'private', gm: null } }]);
  await page.evaluate(() => { window.__S.progressDone = true; window.__S.world = 'ok'; });
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'join_invite'), null, { timeout: 10000 });
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Saved Dens, Packs and Share on the Workshop', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', packs: true, page: 'den' });
  await page.waitForSelector('#yd-dens li');
  const names = await page.$$eval('#yd-dens li .yd-li-t b', (b) => b.map((x) => x.textContent));
  assert.deepEqual(names, ['house', 'backup (before the last open)']);
  assert.match(await page.textContent('#yd-dens li'), /12 props/);
  await page.click('#yd-dens li:first-child button:has-text("Open")');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'den_open'));
  assert.deepEqual(await calls(page, 'den_open'), [{ name: 'house' }]);
  const dl = page.waitForEvent('download');
  await page.click('#yd-dens li:first-child button:has-text("Save a copy")');
  assert.equal((await dl).suggestedFilename(), 'house.den.json');
  // add from a link: only a GitHub release's .den.json goes to the core
  await page.fill('#yd .yd-link input', 'https://example.com/garden.den.json');
  await page.click('#yd .yd-link button');
  assert.match(await statusText(page), /GitHub release/);
  assert.deepEqual(await calls(page, 'den_fetch'), []);
  await page.fill('#yd .yd-link input', 'https://github.com/bea/den/releases/download/v1/garden.den.json');
  await page.click('#yd .yd-link button');
  await page.waitForFunction(() => /Added as 'garden'/.test(document.querySelector('#yd .yd-status').textContent));
  assert.deepEqual(await calls(page, 'den_fetch'), [{ url: 'https://github.com/bea/den/releases/download/v1/garden.den.json', sha256: null }]);
  // packs from the kit's state folder
  assert.deepEqual(await page.$$eval('#yd-packs li b', (b) => b.map((x) => x.textContent)), ['Goldshire']);
  // Share on the Workshop: the steps, then List your mod in its Den shape
  await page.click('#yd button:has-text("Share on the Workshop")');
  await page.waitForSelector('#yd-dens .yd-share');
  assert.match(await page.textContent('#yd-dens .yd-share'), /GitHub, make a release/);
  await page.click('#yd-dens .yd-share button:has-text("List it on the Workshop")');
  await page.waitForSelector('#lm-dlg[open]');
  assert.equal(await page.textContent('#lm-title'), 'Share your Den on the Workshop');
  await page.keyboard.press('Escape');
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('no saved Dens and no packs yet: plain words, and Share on the Workshop says what to do first', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', noDens: true, page: 'den' });
  await page.waitForSelector('#yd-play');
  assert.match(await page.textContent('#yd'), /No saved Dens yet\. In your Den, type \.den save/);
  assert.match(await page.textContent('#yd'), /No packs yet/);
  await page.click('#yd button:has-text("Share on the Workshop")');
  assert.match(await statusText(page), /Save your Den first/);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Shape the ground: not installed, Get Forge shows the GPL notice and source once, then installs', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', page: 'den', forge: 'missing' });
  await page.waitForSelector('#yd-forge');
  assert.ok(await page.locator('#yd .yd-tile h3', { hasText: 'Shape the ground' }).count());
  assert.equal((await page.textContent('#yd-forge')).trim(), 'Get Forge');
  await page.click('#yd-forge');
  await page.waitForSelector('#yd-forge-gpl');
  assert.match(await page.textContent('#yd-forge-gpl'), /GPL-3\.0/);
  assert.deepEqual(await calls(page, 'forge_install'), [], 'nothing downloads before the notice is read');
  await page.click('#yd-forge-src');
  assert.deepEqual(await calls(page, 'open_link'), [{ name: 'forge-source' }]);
  await page.click('#yd-forge-go');
  await page.waitForFunction(() => document.querySelector('#yd-forge').textContent.trim() === 'Open in Forge');
  assert.equal((await calls(page, 'forge_install')).length, 1);
  assert.equal(await page.$('#yd-forge-gpl'), null);
  assert.match(await statusText(page), /Forge is ready/);
  // once: the next Get Forge (say it was removed) goes straight to the download
  await page.evaluate(() => { window.__S.forge = 'missing'; window.LeylineDen.load(); });
  await page.waitForFunction(() => document.querySelector('#yd-forge').textContent.trim() === 'Get Forge');
  await page.click('#yd-forge');
  await page.waitForFunction(() => window.__calls.filter((c) => c[0] === 'forge_install').length === 2);
  assert.equal(await page.$('#yd-forge-gpl'), null, 'the notice shows once');
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Shape the ground: installed, Open in Forge, then "Terrain updated" when it closes', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', page: 'den', forge: 'installed' });
  await page.waitForSelector('#yd-forge');
  assert.equal((await page.textContent('#yd-forge')).trim(), 'Open in Forge');
  await page.click('#yd-forge');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'forge_open'));
  assert.deepEqual(await calls(page, 'forge_open'), [{ client: 'c335a-1' }]);
  await page.waitForFunction(() => document.querySelector('#yd-forge').textContent.trim() === 'Forge is open…');
  assert.equal(await page.isDisabled('#yd-forge'), true, 'one Forge at a time');
  await page.waitForSelector('#yd-forge-note', { timeout: 15000 });
  assert.match(await page.textContent('#yd-forge-note'), /^Terrain updated/);
  assert.match(await statusText(page), /Terrain updated/);
  assert.equal((await page.textContent('#yd-forge')).trim(), 'Open in Forge');
  assert.equal(await page.isDisabled('#yd-forge'), false);
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Shape the ground: no game folder yet says what to do, and starts nothing', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', page: 'den', forge: 'installed', clients: false });
  await page.waitForSelector('#yd-forge');
  await page.click('#yd-forge');
  await page.waitForFunction(() => /made from your own game folder/.test(document.querySelector('#yd .yd-status').textContent));
  assert.deepEqual(await calls(page, 'forge_open'), []);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Play solo: one click, the game signs in by itself to character select; Single player offered where Docker runs', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', auto: true, dock: 'ready' });
  await page.click('#hm-den-go');
  await page.waitForFunction(() => /Your Den is open/.test(document.querySelector('#yd .yd-status').textContent));
  assert.match(await statusText(page), /signs you in and takes you straight to your Den/);
  assert.doesNotMatch(await statusText(page), /password/, 'nothing to type');
  assert.equal(await page.$('#yd-login-hint'), null);
  assert.equal((await calls(page, 'den_play_lite')).length, 1, 'one click, one start');
  // the bigger option: Single player (owner 4 Nov: "Solo leveling" before), worded as such, on its own page (single.js)
  await page.waitForSelector('#yd-solo-go');
  assert.match(await page.textContent('#yd-solo'), /level from 1.*Single player.*helper bots.*Docker.*5 to 20 minutes/i);
  assert.equal(await page.textContent('#yd-solo-go'), 'Single player');
  await page.click('#yd-solo-go');
  await page.waitForFunction(() => document.querySelector('#page-single.on'));
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Play solo: no Docker on this PC, no Single player offer (nothing that cannot work)', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', dock: 'missing', page: 'den' });
  await page.waitForSelector('#yd-play');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'docker_info'));
  await page.waitForTimeout(100);
  assert.equal(await page.$('#yd-solo'), null);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('after Play solo the play bar shows Your Den, and its PLAY is the same one click (smoke O3); the Den page opens at its top (O9)', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', auto: true });
  await page.evaluate(() => { const m = document.querySelector('main'); if (m) m.scrollTop = 900; });
  await page.click('#hm-den-go');
  await page.waitForFunction(() => /Your Den is open/.test(document.querySelector('#yd .yd-status').textContent));
  assert.equal(await page.evaluate(() => document.querySelector('main').scrollTop), 0, 'the title in view');
  await page.waitForFunction(() => document.querySelector('#pb-realm').textContent === 'Your Den');
  assert.equal(await page.textContent('#play-headline'), 'READY TO PLAY');
  assert.doesNotMatch(await page.textContent('#pb-realm'), /Wrath Unleashed/);
  // the bar's PLAY: Your Den's one click again (Den Lite, the self sign-in), not a plain start
  await page.evaluate(() => document.getElementById('play-form').requestSubmit());
  await page.waitForFunction(() => window.__calls.filter((c) => c[0] === 'den_play_lite').length === 2);
  assert.equal((await calls(page, 'play')).length, 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('the launcher opened with Your Den picked: PLAY goes straight into the Den, never a plain start at the login screen (owner 2 Nov 20:48)', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', auto: true, selected: '127.0.0.1' });
  await page.waitForFunction(() => document.querySelector('#pb-realm').textContent === 'Your Den');
  // the Den page never opened: PLAY in the bar
  await page.evaluate(() => document.getElementById('play-form').requestSubmit());
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'den_play_lite'));
  assert.equal((await calls(page, 'play')).length, 0, 'no plain start');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Play solo on a new PC: Den Lite comes with the first click (a one-time download), never the Docker steps (smoke O10)', async () => {
  const { page, ctx, errors } = await open({ lite: 'get', auto: true, page: 'den' });
  await page.waitForSelector('#yd-play');
  assert.match(await page.textContent('#yd-state'), /Ready on this PC: no setup needed/);
  await page.click('#yd-play');
  await page.waitForFunction(() => /one-time download/.test(document.querySelector('#yd .yd-status').textContent));
  await page.waitForFunction(() => /Your Den is open/.test(document.querySelector('#yd .yd-status').textContent));
  assert.equal(await page.$('.yd-steps'), null, 'no Host pack or Docker steps');
  assert.equal((await calls(page, 'hostpack_info')).length, 0);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Invite friends on Den Lite: on the network from this PC at once, a link for one friend copied, Stop sharing; the friend joins with the link', async () => {
  const { page, ctx, errors } = await open({ lite: 'running', page: 'den' });
  await page.waitForSelector('#yd-friends');
  await page.click('#yd-friends');
  await page.waitForFunction(() => /Invite link copied/.test(document.querySelector('#yd .yd-status').textContent));
  assert.equal((await calls(page, 'den_invite')).length, 1);
  assert.equal(await page.$('.yd-steps'), null, 'no Docker steps');
  const clip = await page.evaluate(() => navigator.clipboard.readText());
  assert.match(clip, /^https:\/\/relay\.openvanilla\.app\/join\/kzdvvj2umnduyauf#INVITESECRETINVITESECRET~~GABCDE2345\./);
  assert.match(await statusText(page), /play as Rowan.*24 hours/);
  await page.waitForSelector('#yd-net');
  assert.match(await page.textContent('#yd-net'), /On the Leyline network, private/);
  await page.click('#yd-net-stop');
  await page.waitForFunction(() => !document.querySelector('#yd-net'));
  assert.equal((await calls(page, 'den_net_stop')).length, 1);
  assert.match(await statusText(page), /off the network/);
  // the friend: the link on the Realms page starts the game with their own login
  await page.evaluate(() => window.LeylineNav.show('realms'));
  await page.evaluate(() => { const d = document.getElementById('rl-other'); if (d) d.open = true; });
  await page.fill('#rl-invite', clip);
  await page.click('#rl-join');
  await page.waitForFunction(() => /friend's Den/.test(document.getElementById('rl-status').textContent));
  assert.match(await page.textContent('#rl-status'), /signs you in by itself and opens at character select/);
  assert.equal((await calls(page, 'join_invite'))[0].link, clip);
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Invite friends: the ways in the link in the order they are tried, the link to copy again, your own address; the friend is told the way', async () => {
  const { page, ctx, errors } = await open({ lite: 'running', page: 'den', lan: true });
  await page.waitForSelector('#yd-friends');
  await page.click('#yd-friends');
  await page.waitForSelector('#yd-ways');
  assert.deepEqual(await page.$$eval('#yd-ways li', (l) => l.map((x) => [x.id, x.className])),
    [['yd-way-lan', 'yd-way on'], ['yd-way-own', 'yd-way off'], ['yd-way-relay', 'yd-way on']]);
  assert.match(await page.textContent('#yd-ways'), /tries these in this order.*own game folder and a Hero Key.*Game files never travel/s);
  const link = await page.inputValue('#yd-link');
  assert.match(link, /~~GABCDE2345\.[A-Z0-9]+~~lan=192\.168\.1\.20:3724$/);
  await page.evaluate(() => navigator.clipboard.writeText(''));
  await page.click('#yd-link-copy');
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), link);
  // your own address: kept, then the next invite has it
  await page.fill('#yd-own', 'den.example.org');
  await page.click('#yd-own-save');
  await page.waitForFunction(() => /Your own address is den\.example\.org/.test(document.querySelector('#yd .yd-status').textContent));
  assert.deepEqual((await calls(page, 'den_own_address')).map((a) => a.host), ['den.example.org']);
  await page.click('#yd-friends');
  await page.waitForFunction(() => document.querySelector('#yd-way-own') && document.querySelector('#yd-way-own').classList.contains('on'));
  assert.equal(await page.inputValue('#yd-own'), 'den.example.org');
  // the friend: the link joins on the same network, and they are told so
  await page.evaluate(() => window.LeylineNav.show('realms'));
  await page.evaluate(() => { const d = document.getElementById('rl-other'); if (d) d.open = true; });
  await page.fill('#rl-invite', link);
  await page.click('#rl-join');
  await page.waitForFunction(() => /friend's Den/.test(document.getElementById('rl-status').textContent));
  assert.match(await page.textContent('#rl-status'), /Joining your friend's Den on your shared network/);
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('My models: drop your own model files on Your Den, checked first, listed, removed; friends see a stand-in, nothing uploaded', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', page: 'den' });
  await page.waitForSelector('#yd-models-drop');
  assert.match(await page.textContent('#yd-models-note'), /stay on this PC: nothing is uploaded or shared.*stand-in/);
  // a drop is the browser's own: files made in the page and handed to the drop event
  const dropFiles = (files) => page.evaluate((fs) => {
    const dt = new DataTransfer();
    for (const f of fs) dt.items.add(new File([new Uint8Array(f.bytes)], f.name));
    const el = document.getElementById('yd-models-drop');
    el.dispatchEvent(new DragEvent('dragover', { dataTransfer: dt, bubbles: true, cancelable: true }));
    el.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
  }, files);
  const MD20 = [77, 68, 50, 48, 8, 1, 0, 0];
  // checked before anything is sent: the wrong kind of file, no model in the drop
  await dropFiles([{ name: 'tower.fbx', bytes: [1, 2, 3] }]);
  assert.match(await statusText(page), /tower\.fbx: only \.m2, \.skin, \.blp and \.wmo files/);
  await dropFiles([{ name: 'tower.blp', bytes: [66, 76, 80, 50] }]);
  assert.match(await statusText(page), /There's no model in that/);
  assert.equal((await calls(page, 'den_models_add')).length, 0);
  // a model with its skin and texture, under its own name
  await page.fill('#yd-model-name', 'Lighthouse');
  await dropFiles([{ name: 'Tower.m2', bytes: MD20 }, { name: 'Tower00.skin', bytes: [83, 75, 73, 78] }, { name: 'tower.blp', bytes: [66, 76, 80, 50] }]);
  await page.waitForFunction(() => /Lighthouse is in your Den's gallery under My models/.test(document.querySelector('#yd .yd-status').textContent));
  assert.match(await statusText(page), /1 texture it names weren't in your drop/);
  const sent = (await calls(page, 'den_models_add'))[0];
  assert.deepEqual(sent.files.map((f) => f.name), ['Tower.m2', 'Tower00.skin', 'tower.blp']);
  assert.equal(sent.label, 'Lighthouse');
  assert.equal(Buffer.from(sent.files[0].data, 'base64').subarray(0, 4).toString(), 'MD20', 'the bytes as base64');
  await page.waitForSelector('#yd-models li');
  assert.match(await page.textContent('#yd-models'), /Lighthouse.*Model/s);
  // the server's own check still answers (a file that only looks right by name)
  await dropFiles([{ name: 'fake.m2', bytes: [1, 2, 3, 4] }]);
  await page.waitForFunction(() => /isn't a model file/.test(document.querySelector('#yd .yd-status').textContent));
  await page.click('#yd-models li button');
  await page.waitForFunction(() => !document.querySelector('#yd-models li'));
  assert.deepEqual((await calls(page, 'den_models_remove')).map((a) => a.slug), ['lighthouse']);
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Sky Island in one click, and a Den code a friend sent (docs/DEN-NEXT.md)', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', page: 'den', auto: true });
  await page.waitForSelector('#yd-sky');
  await page.click('#yd-sky');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'den_play_lite'), null, { timeout: 8000 });
  const add = (await calls(page, 'den_add'))[0];
  assert.equal(add.fileName, 'sky-island.den.json');
  const doc = JSON.parse(add.text);
  assert.equal(doc.room.look, 'sky', 'the island file, decoded from its code here');
  assert.deepEqual(await calls(page, 'den_open'), [{ name: 'friend' }], 'opened, then played');
  // a code pasted (with a chat's line break in it): decoded, then the core checks it as a Den file
  const code = await page.evaluate(() => window.LeylineDenPack.encode('den', JSON.stringify({ format: 'leyline-den', version: 1, name: 'garden', props: [] })));
  await page.fill('#yd-code', code.slice(0, 15) + '\n' + code.slice(15));
  await page.click('#yd .yd-code button:has-text("Open the code")');
  await page.waitForFunction(() => window.__calls.filter((c) => c[0] === 'den_add').length === 2);
  assert.equal((await calls(page, 'den_add'))[1].fileName, 'garden.den.json');
  // a changed code: refused here, in plain words, before the core
  await page.fill('#yd-code', code.slice(0, -1) + (code.endsWith('0') ? '1' : '0'));
  await page.click('#yd .yd-code button:has-text("Open the code")');
  await page.waitForFunction(() => /changed or copied wrong|wrong letter/.test(document.querySelector('#yd .yd-status').textContent));
  assert.equal((await calls(page, 'den_add')).length, 2);
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Your Den on the Single player page: what it is, Open my Den, Invite a friend, its state and Stop sharing, all the Den page\'s own', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', auto: true });
  await page.evaluate(() => window.LeylineNav.show('single'));
  await page.waitForSelector('#spd-play');
  // after the Start card, before the world's settings and the Friends area
  const order = await page.evaluate(() => [...document.querySelectorAll('#page-single > .panel')].map((p) => p.id));
  assert.ok(order.indexOf('spd') > order.indexOf('sp') && order.indexOf('spd') < order.indexOf('oc-set') && order.indexOf('spd') < order.indexOf('spf'), order.join());
  assert.equal(await page.textContent('#spd h2'), 'Your Den');
  assert.match(await page.textContent('#spd .spd-lede'), /private room on this PC to build in/);
  assert.match(await page.textContent('#spd-state'), /Ready on this PC: no setup needed/);
  // one click in, the same as the Den page's (den_play_lite: the game signed in behind Leyline's cover)
  await page.click('#spd-play');
  await page.waitForFunction(() => /Your Den is opening/.test(document.getElementById('spd-status').textContent));
  assert.deepEqual(await calls(page, 'den_play_lite'), [{ client: 'c335a-1' }]);
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-single', 'stays on Single player');
  assert.equal(await page.textContent('#spd-status'), await statusText(page), 'the Den page says the same');
  await page.waitForFunction(() => /Open on this PC/.test(document.getElementById('spd-state').textContent));
  // Invite a friend: the Den page's own invite (the link copied), then Stop sharing here
  await page.click('#spd-invite');
  await page.waitForFunction(() => /Invite link copied/.test(document.getElementById('spd-status').textContent));
  assert.equal((await calls(page, 'den_invite')).length, 1);
  assert.match(await page.evaluate(() => navigator.clipboard.readText()), /^https:\/\/relay\.openvanilla\.app\/join\//);
  await page.waitForSelector('#spd-net');
  assert.match(await page.textContent('#spd-net'), /On the Leyline network, private/);
  await page.click('#spd-net-stop');
  await page.waitForFunction(() => !document.getElementById('spd-net'));
  assert.equal((await calls(page, 'den_net_stop')).length, 1);
  // More on Your Den: its page
  await page.click('#spd-more');
  assert.equal(await page.evaluate(() => document.querySelector('.page.on').id), 'page-den');
  await noBrands(page);
  assert.doesNotMatch(await page.evaluate(() => document.getElementById('spd').innerText), /sign in|sign-in|signin/i);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Your Den on the Single player page: no game folder says so; a Den not set up yet opens its steps on the Den page', async () => {
  const a = await open({ clients: false });
  await a.page.evaluate(() => window.LeylineNav.show('single'));
  await a.page.waitForSelector('#spd-play');
  await a.page.click('#spd-play');
  await a.page.waitForFunction(() => /Add it on the Game folders page/.test(document.getElementById('spd-status').textContent));
  assert.deepEqual(await calls(a.page, 'den_play_lite'), []);
  assert.deepEqual(a.errors, []);
  await a.ctx.close();
  // no Den Lite and no Docker Den: Invite a friend opens the one-time steps where they are
  const b = await open({});
  await b.page.evaluate(() => window.LeylineNav.show('single'));
  await b.page.waitForSelector('#spd-invite');
  assert.match(await b.page.textContent('#spd-state'), /Not set up yet/);
  await b.page.click('#spd-invite');
  await b.page.waitForSelector('#yd .yd-steps');
  assert.equal(await b.page.evaluate(() => document.querySelector('.page.on').id), 'page-den');
  assert.deepEqual(b.errors, []);
  await b.ctx.close();
});
