// Your Den (ui/den.js, ui/den.css; src-tauri host/yourden.rs, host/lite.rs) in Chromium with a fake Tauri core: the
// entries (the side menu right under Home, the Home banner), the one click (Den Lite straight in; without Den Lite the
// Docker Den's three steps here; a Docker Den: start it if stopped, its account filled in, then play), Share with
// friends (the Lite Den moved to the Docker Den, then opened there unchanged), Invite a friend, Saved Dens (open, save a
// copy, add a file, add from a Workshop link), Packs, Share on the Workshop (List your mod in its Den shape), and no
// game or publisher names anywhere.
//
//   node launcher/tests/den.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.page', o.page || 'play'); } catch (_) { /* fine */ }
  const S = (window.__S = {
    lite: o.lite, kit: !!o.kit, docker_den: !!o.docker_den, world: o.world || 'ok', packInstalled: false,
    dock: o.dock || 'ready', progressDone: false, forge: o.forge || 'missing', forgeOpen: false, forgeLooks: 0, forgeLast: null, clients: o.clients === false ? [] : [{ id: 'c335a-1', name: 'My Game', path: '/x', build: '3.3.5a', realmlist: '' }],
  });
  const dens = [{ name: 'house', bytes: 300, props: 12, saved: '2026-11-01T01:00:00Z' }, { name: 'backup', bytes: 100, props: 3, saved: '2026-10-31T01:00:00Z' }];
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: S.clients, selected_client: S.clients[0] ? 'c335a-1' : null,
        selected_realm: 'play.openvanilla.app', wine: { enabled: false, command: 'wine', prefix: '' } },
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
      case 'social_status': return false;
      case 'den_state': return {
        lite: { available: !!S.lite, running: S.lite === 'running', address: '127.0.0.1', account: 'DEN', den_dir: '/app/den-lite/state/dens' },
        kit: S.kit || S.docker_den, docker_den: S.docker_den, other_realm: null, account: S.docker_den ? 'BEA42' : '',
        saves: S.docker_den ? 'kit' : 'lite', dens: o.noDens ? [] : dens,
        packs: o.packs ? [{ id: 'goldshire', title: 'Goldshire', line: 'The inn and the square, ready to build around.' }] : [] };
      case 'den_play_lite': S.lite = 'running'; return 'Started My Game on 127.0.0.1.';
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
      case 'join_invite': return { address: 'relay.openvanilla.app:40001', name: 'My Den' };
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

test('the entries: Your Den right under Home in the side menu, and first on Home', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped' });
  const nav = await page.$$eval('aside nav .nav[data-page]', (b) => b.map((x) => x.dataset.page));
  assert.deepEqual(nav.slice(0, 2), ['play', 'den']);
  assert.equal((await page.textContent('.nav[data-page="den"]')).trim(), 'Your Den');
  const order = await page.$$eval('#home-more > .hm-sec', (s) => s.map((x) => x.id));
  assert.equal(order[0], 'hm-den', 'the banner leads the rows under the hero');
  assert.equal((await page.textContent('#hm-den-go')).trim(), 'Your Den');
  await page.click('.nav[data-page="den"]');
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
  await page.waitForFunction(() => /Open on this PC/.test(document.querySelector('#yd-state').textContent));
  assert.deepEqual(await calls(page, 'host_start_quiet'), [], 'no Docker');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('no game folder: Play says so and opens the add flow', async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped', clients: false, page: 'den' });
  await page.waitForSelector('#yd-play');
  await page.click('#yd-play');
  await page.waitForFunction(() => /3\.3\.5a game folder/.test(document.querySelector('#yd .yd-status').textContent));
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
  assert.deepEqual(await page.$$eval('#yd .yd-acts button', (b) => b.map((x) => x.textContent)), ['Play', 'Invite friends', 'Ask Claude to edit it']);
  // Den Lite: friends join a Den on Docker, so Invite friends opens those steps here
  await page.click('#yd-friends');
  await page.waitForSelector('#yd .yd-steps');
  assert.match(await statusText(page), /three steps below/);
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
