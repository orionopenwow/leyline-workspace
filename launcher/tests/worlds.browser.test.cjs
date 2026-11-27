// Worlds in three clicks (ui/worlds.js; src-tauri/src/host/quickworld.rs, host/invite.rs; docs/UNSTOPPABLE.md "Worlds in
// three clicks") in Chromium with a fake Tauri core, as the shipped app runs (no server of our own): Home › Host › a
// template › Start is at most three clicks to "World running"; Invite a friend gives the site's one-click link; a
// friend opening a leyline://join/ invite gets "Join <name>'s world?" and joins in one click (the existing join path);
// a bad invite says why; background readiness shows its small note and stays off when the setting is off.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/worlds.browser.test.cjs     (WORLDS_SHOTS=<dir> for screenshots)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const SHOTS = process.env.WORLDS_SHOTS || '';
const BRAND = /world of warcraft|\bwow\b|warcraft|lich king|(?<!endorsed by )blizzard/i;
const INVITE = 'lw1.eyJ2IjoxfQ.c2ln';
const LINK = 'https://relay.leylinelauncher.com/join/abcdefghijk2345a#ABCDEFGHJKLMNPQR~~GABC12345.PASS1234WORD~~lan=192.168.1.20:3724~~lf1';

const FAKE = (o) => {
  const calls = (window.__calls = []);
  window.__clicks = 0;
  document.addEventListener('click', (e) => { if (e.isTrusted) window.__clicks++; }, true);
  try { localStorage.clear(); } catch (_) { /* fine */ }
  const worlds = { single: { set_up: !!o.setUp, running: false }, vanilla: { set_up: false, running: false } };
  const status = (kind) => ({ kind, supported: true, set_up: worlds[kind] ? worlds[kind].set_up : false, running: worlds[kind] ? worlds[kind].running : false,
    other_running: null, characters: [], engine: 'builtin' });
  let job = null, prep = null;
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c335a-1', name: 'Wrath 3.3.5a', path: '/games/wrath', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c335a-1', selected_realm: 'leylinelauncher.com', wine: { enabled: false, command: 'wine', prefix: '' }, world_get_ready: o.getReady !== false },
        official_realm: 'leylinelauncher.com', browser_url: 'https://leylinelauncher.com/wrath', windows: true, data_dir: '/tmp/leyline' };
      case 'platform': return { os: 'windows', mobile: false, node: true };
      case 'deep_link_take': { const l = o.link || null; o.link = null; return l; }
      case 'realms_list': case 'realms_list_cached': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'mods_catalog': case 'mods_catalog_cached': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c335a-1', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: { current: '0.4.0', latest: '0.4.0' }, checked_at: 1 };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'host_progress': return { idle: true, kind: 'host', running: false, code: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: true, saved_at: null, error: null };
      case 'news_list': return { items: [], from_cache: false };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'world_status': return status(args.kind);
      case 'engine_status': return status('single');
      case 'world_resume_pending': return null;
      case 'world_find_folder': return { picked: 'c335a-1', found: [] };
      case 'world_setup': case 'world_start': job = { kind: args.kind, n: 0 }; return 'Getting the engine…';
      case 'engine_progress': {
        if (!job) return { idle: true, kind: 'engine', running: false, code: null };
        job.n++;
        const done = job.n >= 3;
        if (done) { worlds[job.kind].running = true; worlds[job.kind].set_up = true; }
        const steps = o.setUp ? ['Starting your world'] : ['Copying your prepared world (seconds, not minutes)', 'Creating your local login', 'Starting your world'];
        return { idle: false, kind: job.kind === 'single' ? 'engine' : job.kind, running: !done, code: done ? 0 : null, step: done ? 'Ready' : steps[Math.min(job.n - 1, steps.length - 1)],
          n: job.n, of: 3, secs: job.n, problem: null, pct: null, detail: '' };
      }
      case 'world_invite_link': return { invite: 'lw1.abc.def', app_link: 'leyline://join/lw1.abc.def', site_link: 'https://leylinelauncher.com/j/#lw1.abc.def', expires: 9, ways: [] };
      case 'world_prepare': prep = 0; return 'Getting worlds ready in the background.';
      case 'world_prepare_progress': {
        if (prep === null) return { idle: true, kind: 'prepare', running: false, code: null };
        prep++;
        return prep < 2 ? { idle: false, kind: 'prepare', running: true, code: null, step: 'Reading the maps from your game folder (one time)', pct: 42 }
          : { idle: false, kind: 'prepare', running: false, code: 0, step: 'Ready' };
      }
      case 'invite_open': if (o.badInvite) throw "That invite has run out (they last a week). Ask your friend for a new one.";
        return { title: "Join Bo's world?", name: 'Bo', peer: 'abcdefghabcdefgh', kind: 'single', expires: 9, ways: ['192.168.1.20:3724'], link: o.legacyLink };
      case 'join_invite': window.__joined = args; return { address: '127.0.0.1:3724', name: "Your friend's world", way: 'lan', tried: [],
        guest: { auto: true, account: 'GABC12345', password: '', why: '', message: 'ok' } };
      case 'agents_state': return { allow: false, waiting: [], audit: [], cc: {}, desktop: {}, raw: '', json: '{}', tiers: { read: [], ask: [], never: [] } };
      case 'friends_state': return { friends: [], me: null };
      case 'friend_worlds': return [];
      case 'addons_should_offer': return false;
      case 'addons_status': return { pack: { version: '1', addons: [] }, from_cache: false, error: null, prefs: { auto_update: true, dont_ask: false },
        clients: [{ client: 'c335a-1', installed: true, update: false, missing: [] }] };
      default: return null;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(o = {}) {
  const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: 1280, height: 800 }, permissions: ['clipboard-read', 'clipboard-write'] });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route(/^https?:\/\//, (r) => r.abort());
  await page.addInitScript(`(${FAKE.toString()})(${JSON.stringify(Object.assign({ legacyLink: LINK }, o))})`);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineHostWorlds && document.querySelector('.page.on'));
  await page.evaluate(() => { window.LeylineHostWorlds.cfg.pollMs = 30; window.LeylineHostWorlds.cfg.prepPollMs = 30; window.__clicks = 0; });
  return { page, ctx, errors };
}
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, 'worlds-' + name + '.png') }); };
const called = (page, cmd) => page.evaluate((c) => window.__calls.filter(([n]) => n === c).map(([, a]) => a), cmd);

test('Home › Host › a template › Start: three clicks to a running world', async () => {
  const { page, ctx, errors } = await open();
  assert.deepEqual(await page.$$eval('.page.on', (p) => p.map((x) => x.id)), ['page-play'], 'starts on Home');
  await page.click('.nav[data-page="worlds"]');                       // 1
  assert.ok(await page.isVisible('#page-worlds'));
  assert.deepEqual(await page.$$eval('.wd-tile .wd-tile-h', (b) => b.map((x) => x.textContent)), ['Your own world', 'The original world', 'Your Den']);
  await page.click('#wd-t-single');                                    // 2
  const defaults = await page.$$eval('.wd-defaults li', (l) => l.map((x) => x.textContent));
  assert.ok(defaults.some((d) => /private/i.test(d)) && defaults.some((d) => /levelling speed/i.test(d)), String(defaults));
  assert.equal(await page.locator('#wd-card .btn.primary').count(), 1, 'one main button');
  await shot(page, 'start');
  await page.click('#wd-start');                                       // 3
  await page.waitForSelector('#wd-card[data-state="running"]');
  assert.match(await page.textContent('#wd-card'), /World running/);
  assert.ok((await page.evaluate(() => window.__clicks)) <= 3, 'at most three clicks');
  // a world never set up: set up with the folder found by itself (the core copies the prepared database when it has one)
  assert.deepEqual(await called(page, 'world_setup'), [{ client: 'c335a-1', kind: 'single' }]);
  assert.deepEqual(await called(page, 'world_find_folder'), [{ kind: 'single' }]);
  await shot(page, 'running');
  assert.doesNotMatch(await page.evaluate(() => document.body.innerText), BRAND);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a world set up before only starts, and Your Den opens its own page', async () => {
  const { page, ctx, errors } = await open({ setUp: true });
  await page.click('.nav[data-page="worlds"]');
  await page.click('#wd-t-single');
  await page.click('#wd-start');
  await page.waitForSelector('#wd-card[data-state="running"]');
  assert.deepEqual(await called(page, 'world_start'), [{ kind: 'single' }]);
  assert.deepEqual(await called(page, 'world_setup'), []);
  await page.click('#wd-t-den');
  assert.deepEqual(await page.$$eval('.page.on', (p) => p.map((x) => x.id)), ['page-den']);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Invite a friend: the site link with the invite after #, and the leyline:// form', async () => {
  const { page, ctx, errors } = await open({ setUp: true });
  await page.click('.nav[data-page="worlds"]');
  await page.click('#wd-t-single');
  await page.click('#wd-start');
  await page.waitForSelector('#wd-invite');
  await page.click('#wd-invite');
  await page.waitForSelector('#wd-link');
  assert.equal(await page.inputValue('#wd-link'), 'https://leylinelauncher.com/j/#lw1.abc.def');
  await page.click('#wd-copy');
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), 'https://leylinelauncher.com/j/#lw1.abc.def');
  await page.click('#wd-copy-app');
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), 'leyline://join/lw1.abc.def');
  assert.deepEqual(await called(page, 'world_invite_link'), [{ kind: 'single' }]);
  await shot(page, 'invite');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test("a friend's invite: \"Join Bo's world?\", one click to join", async () => {
  const { page, ctx, errors } = await open({ link: { action: 'invite', id: INVITE } });
  await page.waitForSelector('#solo-join');
  assert.equal(await page.textContent('.solo-join-card b'), "Join Bo's world?");
  assert.deepEqual(await called(page, 'invite_open'), [{ invite: INVITE }]);
  await page.evaluate(() => { window.__clicks = 0; });
  await shot(page, 'join');
  await page.click('#solo-join-go');                                   // 1
  await page.waitForFunction(() => window.__joined);
  assert.deepEqual(await page.evaluate(() => window.__joined), { client: 'c335a-1', link: LINK });
  assert.equal(await page.evaluate(() => window.__clicks), 1, 'one click from the invite to joining');
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('an invite that has run out says so, and joins nothing', async () => {
  const { page, ctx, errors } = await open({ link: { action: 'invite', id: INVITE }, badInvite: true });
  await page.waitForSelector('#solo-join');
  assert.match(await page.textContent('#solo-join'), /run out/);
  assert.equal(await page.locator('#solo-join-go').count(), 0, 'no Join');
  assert.deepEqual(await called(page, 'join_invite'), []);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('background readiness: a small note while it works, and nothing when the setting is off', async () => {
  const { page, ctx, errors } = await open();
  // opening Host asks for it (the core does nothing when it's done); the note follows its line
  await page.click('.nav[data-page="worlds"]');
  await page.waitForFunction(() => /ready to start quickly/.test(document.getElementById('wd-prep').textContent));
  assert.equal((await called(page, 'world_prepare')).length, 1);
  await ctx.close();
  const off = await open({ getReady: false });
  await off.page.click('.nav[data-page="worlds"]');
  await off.page.evaluate(() => window.LeylineHostWorlds.prepare());
  assert.deepEqual(await called(off.page, 'world_prepare'), [], 'off: never asked');
  assert.deepEqual([...errors, ...off.errors], []);
  await off.ctx.close();
});
