// Hosting friends from Your Den with no Docker (ui/den.js friendsBox, codeRow; src-tauri host/denhost.rs, app.rs
// den_friends / den_friend; docs/DEN-HOSTING.md) in Chromium with a fake Tauri core: Invite a friend is one click on
// the Share with friends tile (no Docker steps), the code to paste, the Friends list (who's in, Build, Keep, Kick, the
// links that still work, Let everyone build), the Docker move kept as a quiet link, the realm picker's Join a
// friend's Den… row and the code pasted there, the champion on Saved Dens, and no game or publisher names anywhere.
//
//   node launcher/tests/denhost.browser.test.cjs
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
    lite: o.lite, selected: o.selected, kit: !!o.kit, docker_den: !!o.docker_den, world: o.world || 'ok', packInstalled: false,
    dock: o.dock || 'ready', progressDone: false, forge: o.forge || 'missing', forgeOpen: false, forgeLooks: 0, forgeLast: null, clients: o.clients === false ? [] : [{ id: 'c335a-1', name: 'My Game', path: '/x', build: '3.3.5a', realmlist: '' }],
  });
  const dens = [{ name: 'house', bytes: 300, props: 12, saved: '2026-11-01T01:00:00Z', champion: o.champion ? 'Rowan won the Spring Cup (round robin, 4 players)' : '' }, { name: 'backup', bytes: 100, props: 3, saved: '2026-10-31T01:00:00Z' }];
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
      // the Friends list (host/denhost.rs): S.friends is changed by den_friend as Den Lite would
      case 'den_friends': return { running: S.lite === 'running', sharing: !!S.net, tunnel: S.net || {},
        online: { fresh: true, all_build: !!S.allBuild, players: S.lite === 'running' ? [{ name: 'Wren', host: true, user: '', builds: true, keep: false, since: 0 }].concat(S.inNow || []) : [],
          tourney: S.tourney || null },
        invites: S.invites || [] };
      case 'den_friend': {
        if (args.action === 'kick') { S.inNow = (S.inNow || []).filter((p) => p.user !== args.who); S.invites = (S.invites || []).filter((i) => i.user !== args.who); return 'Rowan is out of your Den, and their link no longer works.'; }
        if (args.action === 'build' && args.who === 'all') { S.allBuild = args.on; return args.on ? 'Every friend can build now.' : 'Only you build now.'; }
        for (const l of [S.inNow || [], S.invites || []]) for (const p of l) if (p.user === args.who) { if (args.action === 'build') { p.builds = args.on; p.build = args.on; } else { p.keep = args.on; } }
        return args.action === 'build' ? (args.on ? 'Rowan can build now.' : "Rowan can't build now.") : 'Rowan is kept: their link works for a week from their last visit.';
      }
      case 'den_invite': S.net = { running: true, connected: true, relay: 'relay.openvanilla.app', realm_id: 'kzdvvj2umnduyauf', address: 'relay.openvanilla.app', auth_port: 40006, world_port: 40007, streams: 0, last_error: '' };
        S.invites = (S.invites || []).concat([{ user: 'GABCDE2345', name: 'Rowan', expires: Math.floor(Date.now() / 1000) + 86400, build: false, keep: false }]);
        return { link: 'https://relay.openvanilla.app/join/kzdvvj2umnduyauf#INVITESECRETINVITESECRET~~GABCDE2345.ABCDEFGH23456789' + (o.lan ? '~~lan=192.168.1.20:3724' : ''), code: 'DEN-KZDVVJ2UMNDUYAUF-INVITESECRETINVITESECRET-GABCDE2345-ABCDEFGH23456789', name: 'Rowan', expires: 1, tunnel: S.net,
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

test('Share with friends on Den Lite: one click, no Docker steps; the link and a code; the Docker move is a quiet link', async () => {
  const { page, ctx, errors } = await open({ lite: 'running', page: 'den' });
  await page.waitForSelector('#yd-share-invite');
  assert.match(await page.textContent('#yd .yd-tile.wide h3'), /Share with friends/);
  assert.match(await page.textContent('#yd'), /no Docker, no port forwarding, and your home address is never shown/);
  assert.equal(await page.locator('#yd-start').count(), 0, 'no Docker steps until asked');
  assert.match(await page.textContent('#yd-friends-list'), /Nobody is in yet/);
  await page.click('#yd-share-invite');
  await page.waitForFunction(() => /Invite link copied/.test(document.querySelector('#yd .yd-status').textContent));
  assert.match(await page.evaluate(() => navigator.clipboard.readText()), /^https:\/\/relay\.openvanilla\.app\/join\/kzdvvj2umnduyauf#/);
  assert.equal(await page.inputValue('#yd-code'), 'DEN-KZDVVJ2UMNDUYAUF-INVITESECRETINVITESECRET-GABCDE2345-ABCDEFGH23456789');
  await page.click('#yd-code-copy');
  assert.match(await page.evaluate(() => navigator.clipboard.readText()), /^DEN-/);
  assert.match(await statusText(page), /Invite code copied/);
  assert.deepEqual(await calls(page, 'den_share_move'), [], 'nothing moves to Docker');
  // the friend not in yet: their link, how long it works, Keep and Take back
  await page.waitForSelector('#yd-friends-in li.yd-friend');
  const row = await page.textContent('#yd-friends-in li.yd-friend');
  assert.match(row, /Rowan/);
  assert.match(row, /not in now · looks around · works for (23|24) more hours/);
  assert.equal(await page.textContent('#yd-friends-in .yd-f-kick'), 'Take back');
  // the Docker Den is still there, as a quiet link
  await page.click('#yd-share');
  await page.waitForSelector('#yd-start');
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test("the Friends list: who's in, Build and Keep per friend, Kick, Let everyone build, a tournament under way", async () => {
  const inNow = [{ name: 'Rowan', host: false, user: 'GABCDE2345', builds: false, keep: false, since: Math.floor(Date.now() / 1000) - 600 }];
  const invites = [{ user: 'GABCDE2345', name: 'Rowan', expires: Math.floor(Date.now() / 1000) + 3600 * 20, build: false, keep: false },
    { user: 'GBRIAR2345', name: 'Briar', expires: Math.floor(Date.now() / 1000) + 3600 * 5, build: true, keep: false }];
  const { page, ctx, errors } = await open({ lite: 'running', page: 'den' });
  await page.evaluate(([a, b]) => { window.__S.inNow = a; window.__S.invites = b; window.__S.tourney = { title: 'Spring Cup', phase: 'signup', players: 3, champion: null }; }, [inNow, invites]);
  await page.evaluate(() => window.LeylineDen.load());
  await page.waitForFunction(() => /In your Den now: 1/.test(document.querySelector('#yd-friends-list').textContent));
  const rows = await page.$$eval('#yd-friends-in li.yd-friend', (l) => l.map((x) => [x.dataset.who, x.classList.contains('in'), x.querySelector('.yd-hint').textContent]));
  assert.deepEqual(rows.map((r) => [r[0], r[1]]), [['Rowan', true], ['Briar', false]], 'who is in first, then the links not in use');
  assert.match(rows[0][2], /in for 10 min · looks around · works for 20 more hours/);
  assert.match(rows[1][2], /not in now · can build · works for 5 more hours/);
  assert.match(await page.textContent('#yd-f-tourney'), /Spring Cup: 3 signed up \(sign-ups open\)/);
  // Build for Rowan
  await page.click('#yd-friends-in li[data-who="Rowan"] .yd-f-build');
  await page.waitForFunction(() => /Rowan can build now/.test(document.querySelector('#yd .yd-status').textContent));
  assert.deepEqual(await calls(page, 'den_friend'), [{ action: 'build', who: 'GABCDE2345', on: true }], 'by their account');
  await page.waitForFunction(() => document.querySelector('#yd-friends-in li[data-who="Rowan"] .yd-f-build').getAttribute('aria-pressed') === 'true');
  // Keep Briar
  await page.click('#yd-friends-in li[data-who="Briar"] .yd-f-keep');
  await page.waitForFunction(() => /kept/.test(document.querySelector('#yd-friends-in li[data-who="Briar"] .yd-hint').textContent));
  // Let everyone build
  await page.click('#yd-f-all');
  await page.waitForFunction(() => /Every friend can build now/.test(document.querySelector('#yd .yd-status').textContent));
  assert.equal(await page.getAttribute('#yd-f-all', 'aria-pressed'), 'true');
  // Kick Rowan: out of the list
  await page.click('#yd-friends-in li[data-who="Rowan"] .yd-f-kick');
  await page.waitForFunction(() => !document.querySelector('#yd-friends-in li[data-who="Rowan"]'));
  assert.match(await statusText(page), /Rowan is out of your Den/);
  assert.deepEqual((await calls(page, 'den_friend')).map((c) => c.action), ['build', 'keep', 'build', 'kick']);
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test("the realm picker's Join a friend's Den… opens the invite box; a Den code joins like a link", async () => {
  const { page, ctx, errors } = await open({ lite: 'stopped' });
  await page.click('#pb-realm-btn');
  await page.click("#pb-rmenu >> text=Join a friend's Den…");
  await page.waitForFunction(() => document.querySelector('.page.on').id === 'page-realms');
  assert.equal(await page.evaluate(() => document.getElementById('rl-other').open), true);
  assert.equal(await page.evaluate(() => document.activeElement.id), 'rl-invite');
  assert.match(await page.getAttribute('#rl-invite', 'placeholder'), /DEN-/);
  await page.fill('#rl-invite', 'DEN-KZDVVJ2UMNDUYAUF-INVITESECRETINVITESECRET-GABCDE2345-ABCDEFGH23456789');
  await page.click('#rl-join');
  await page.waitForFunction(() => window.__calls.some((c) => c[0] === 'join_invite'));
  assert.deepEqual((await calls(page, 'join_invite')).map((c) => c.link), ['DEN-KZDVVJ2UMNDUYAUF-INVITESECRETINVITESECRET-GABCDE2345-ABCDEFGH23456789'], 'the core turns the code into its link');
  await noBrands(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Saved Dens: the last tournament fought in a Den on its card', async () => {
  const { page, ctx, errors } = await open({ lite: 'running', page: 'den', champion: true });
  await page.waitForSelector('#yd-dens .yd-champ');
  assert.equal(await page.textContent('#yd-dens .yd-champ'), 'Champion: Rowan won the Spring Cup (round robin, 4 players)');
  assert.equal(await page.locator('#yd-dens .yd-champ').count(), 1, 'only the Den it was fought in');
  assert.deepEqual(errors, []);
  await ctx.close();
});
