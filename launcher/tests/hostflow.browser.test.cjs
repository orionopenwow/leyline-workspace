// The Host page's flow (ui/hostflow.js, ui/hostflow-catalog.js; src-tauri/src/host/flow.rs) in Chromium with a fake Tauri
// core: one status line for the Host pack and Docker with one fix button; Private or Public; the templates (pitch, key
// numbers, best for); the settings panel (a slider change shows "Custom (from X)", the summary follows, a setting that
// doesn't apply to the chosen world is off); Ask Claude's paste-back fills the panel and leaves out unknown keys and
// clamps out-of-range ones; share codes; Host (host_flow_apply, then the ruleset, then the quiet start) and the running
// realm's card; Game modes, Fork and Play solo one click away; the Guide tab's start-to-finish guide.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/hostflow.browser.test.cjs     (HOST_SHOTS=<dir> for screenshots)
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
const SHOTS = process.env.HOST_SHOTS;
const shot = async (page, name, opts) => {
  if (!SHOTS) return;
  await page.waitForTimeout(400);
  if (opts && opts.el) {   // a tall window, so the whole part shows (the page scrolls inside its frame)
    const vp = page.viewportSize();
    await page.setViewportSize({ width: vp.width, height: 2400 });
    await page.waitForTimeout(300);
    await page.locator(opts.el).screenshot({ path: path.join(SHOTS, name) });
    await page.setViewportSize(vp);
  }
  else await page.screenshot({ path: path.join(SHOTS, name), fullPage: !!(opts && opts.full) });
};
const NO_BRANDS = /warcraft|\bwow\b|lich king|blizzard/i;

const FAKE = (opts) => {
  const calls = (window.__calls = []);
  let realm = opts.realm || null;
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c1', name: 'Mine', path: '/g', build: '3.3.5a' }], selected_client: 'c1', selected_realm: 'leylinelauncher.com',
        wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'leylinelauncher.com', browser_url: 'https://leylinelauncher.com/wrath', windows: false, data_dir: '/tmp' };
      case 'deep_link_take': return null;
      case 'host_modes': return JSON.parse(JSON.stringify(opts.modes));
      case 'host_mode_use': return null;
      case 'host_info': if (window.__jobDone && !realm) realm = window.__jobDone;
        return { dir: '/home/me/kit', is_kit: !opts.noKit, has_windows_script: true, has_unix_script: true, problems: [], seeds: [], realm };
      case 'host_console': return { set_up: !!realm, linked: false, name: realm ? realm.name : '', realm_id: realm ? RID_ : '', profile: realm ? realm.profile : 'main', core: 'azerothcore',
        visibility: realm ? realm.visibility : 'private', signup: 'invite', herokey: false, claim_sent: false, browser: true, max_players: 0, relay: 'relay.openvanilla.app', relay_connected: true,
        relay_error: null, docker: true, services: [], uptime: '1 min', gm: true, gm_note: null, invite: 'https://relay.openvanilla.app/join/' + RID_ + '#secret' };
      case 'host_status': return { realm_id: args.realmId, online: true, name: realm ? realm.name : '', visibility: realm ? realm.visibility : 'private', players: 3, bots: 0, listed: true };
      case 'host_invite': return 'https://relay.openvanilla.app/join/' + RID_ + '#secret';
      case 'host_gm': return 'gm=1\nup=60\nplayers=0\nbots=0\nrestart=0\nallow=off\nallowed=0\nvis=public\nmotd=-';
      case 'host_profiles': return [['main', 'vanillaplus', 'progression', 'brawl', 'dalaran', 'solo', 'den'].map((id) =>
        ({ id, label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c1', name: 'Mine', path: '/g' }], note: null })), null];
      case 'host_choose': return { profile: args.profile, client_dir: '' };
      case 'host_flow_apply': {
        const w = { whole: 'main', eras: 'progression', first: 'vanillaplus', hangout: 'dalaran', top: 'brawl', solo: 'solo' }[args.values.world];
        if (args.values.world === 'solo' && args.visibility === 'public') throw 'Just me, with helper bots is private only: pick Private, or another world.';
        window.__applied = args;
        return { profile: w, visibility: args.visibility, mode_name: 'x', lines: [] };
      }
      case 'host_flow_current': return window.__current || null;
      case 'host_plan_ask': window.__asked = args.text; return null;
      case 'host_plan_take': return window.__planAnswer ? (() => { const a = window.__planAnswer; window.__planAnswer = null; return a; })() : null;
      case 'agents_state': return { allow: !!opts.agents, pending: [], audit: [], tools: [], never: [], now: 1 };
      case 'host_start_quiet': window.__jobAt = 0; window.__answers = args.answers; return 'Setting up your realm.';
      case 'host_stop': return 'Stopped.';
      case 'host_progress': {
        if (window.__jobAt === undefined) return { idle: true, kind: '', running: false, code: null, step: '', n: 0, of: 0, secs: 0, problem: null };
        const n = Math.min(++window.__jobAt, 3);
        const done = n >= 3;
        if (done && !window.__jobDone) window.__jobDone = { realm_id: RID_, name: window.__answers.name || 'Fun Town', visibility: window.__applied.visibility, relay: 'relay.openvanilla.app', profile: 'main' };
        return { idle: false, kind: 'host', running: !done, code: done ? 0 : null, step: done ? 'Done' : ['Getting the helper', 'Starting the servers'][n - 1], n, of: 3, secs: n * 30, problem: null };
      }
      case 'hostpack_info': return { installed: opts.noKit ? null : { version: '7' }, available: { version: '7', size: 400000 }, update: false, kit_dir: '/home/me/kit' };
      case 'hostpack_install': return [{ version: '7' }, '/home/me/kit'];
      case 'set_host_kit': opts.noKit = false; return null;
      case 'docker_info': return Object.assign({ get_url: 'https://docs.docker.com/desktop/' }, window.__dock || { state: 'ready', version: '27', help: 'Docker is ready.', get_url: 'https://docs.docker.com/desktop/' });
      case 'docker_start': window.__dock = { state: 'ready', version: '27', help: 'Docker is ready.' }; return 'Starting Docker Desktop.';
      case 'host_scan': return null;
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 5 };
      case 'open_link': window.__opened = args.name; return null;
      case 'themes_list': return { active: 'dark', themes: [] };
      default: throw 'unknown ' + cmd;
    }
  } } };
  var RID_ = opts.rid;   // eslint-disable-line no-var
  if (opts.dock) window.__dock = opts.dock;
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(o = {}) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, Object.assign({ modes: MODES, rid: RID }, o));
  await page.addInitScript(() => { try { localStorage.removeItem('leyline.hostTemplates'); } catch (_) { /* */ } });
  await page.goto(INDEX);
  await page.click('.nav[data-page="host"]');
  await page.waitForSelector('#hst .hst-vis');
  return { page, errors };
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c).map(([, a]) => a), cmd);
const sumTitle = (page) => page.textContent('.hst-sum-t');

test('step 1: one status line with one fix; Private or Public; the old grid waits behind More ways to host', async () => {
  const { page, errors } = await open({ noKit: true, dock: { state: 'desktop_stopped', can_start: true, help: 'Docker Desktop is installed but closed.' } });
  await page.waitForSelector('#hst-fix');
  assert.equal(await page.textContent('#hst-fix'), 'Get the Host pack');
  assert.equal(await page.locator('.hst-ready').count(), 1, 'one line, not one per card');
  assert.ok(await page.isHidden('.hc-setup'), 'the ruleset picker waits underneath');
  assert.equal(await page.locator('#hx').count(), 0, 'the mode grid waits underneath');
  assert.doesNotMatch(await page.textContent('#hst'), /Needs Docker/, 'no Docker line repeated on cards');
  await shot(page, '1-start-not-ready.png');
  await page.click('#hst-fix');   // the Host pack: installed, then Docker is the one fix
  await page.waitForFunction(() => document.getElementById('hst-fix') && document.getElementById('hst-fix').textContent === 'Start Docker');
  await page.click('#hst-fix');
  await page.waitForSelector('.hst-ready.ok', { timeout: 10000 });
  assert.match(await page.textContent('.hst-ready'), /Ready to host/);
  // Private / Public, one line each
  const vis = await page.$$eval('.hst-vis-b', (l) => l.map((b) => [b.dataset.vis, b.querySelector('b').textContent, b.querySelector('.hst-vis-t span').textContent]));
  assert.deepEqual(vis, [['private', 'Private', 'Just me and friends I invite.'], ['public', 'Public', 'On the Community realms list: anyone can join.']]);
  await page.click('.hst-vis-b[data-vis="public"]');
  assert.equal(await page.getAttribute('.hst-vis-b[data-vis="public"]', 'aria-checked'), 'true');
  assert.ok(await page.isVisible('.hst-node-b'), 'Public: be a node, optional');
  // the templates: pitch, key numbers, best for
  const tpls = await page.$$eval('.hst-tpl', (l) => l.map((b) => b.dataset.tpl));
  assert.deepEqual(tpls, ['pure-vanilla', 'vanilla-plus', 'original', 'classic', 'fun', 'rp', 'hardcore', 'solo-bots', 'pvp', 'event-night', 'blank']);
  assert.deepEqual(await page.$$eval('.hst-tpl[data-tpl="pure-vanilla"] .hst-nums i', (l) => l.map((x) => x.textContent)), ['XP 1x', 'To 60', 'No bots', 'PvE']);
  assert.deepEqual(await page.$$eval('.hst-tpl[data-tpl="fun"] .hst-nums i', (l) => l.map((x) => x.textContent)), ['XP 10x', 'To 80', '60 bots', 'PvE', 'Boost']);
  assert.equal(await page.textContent('.hst-tpl[data-tpl="rp"] .hst-best'), 'Storytellers');
  assert.ok(await page.isVisible('.hst-tpl[data-tpl="solo-bots"] .hst-priv'));
  // More ways to host: the game modes grid (Fork, Play solo) and back
  await page.click('.hst-more summary');
  await page.click('.hst-modes');
  await page.waitForSelector('#hx .hx-card');
  assert.equal(await page.locator('#hst').count(), 0, 'the flow steps aside');
  assert.ok(await page.locator('[data-mode="duel-ladder"] .hx-fork').count() === 1);
  await page.click('.hx-flow-back');
  await page.waitForSelector('#hst .hst-vis');
  assert.equal(await page.locator('#hx').count(), 0);
  assert.match(await page.textContent('.hst-soon'), /Coming soon.*Survivors/);
  assert.doesNotMatch(await page.textContent('#hst'), NO_BRANDS);
  assert.deepEqual(errors, []);
});

test('step 2-3: a template fills the panel; a slider change is Custom (from it); settings that do not apply are off', async () => {
  const { page, errors } = await open();
  await page.click('.hst-vis-b[data-vis="private"]');
  await page.click('.hst-tpl[data-tpl="vanilla-plus"]');
  await page.waitForSelector('.hst-s3 .hst-group');
  assert.equal(await sumTitle(page), 'Vanilla+');
  assert.deepEqual(await page.$$eval('.hst-group legend', (l) => l.map((x) => x.textContent)), ['Levels', 'Rates', 'World', 'Players', 'Rules', 'Addons and modes']);
  assert.equal(await page.inputValue('#hst-in-xp'), '1.5');
  assert.equal(await page.inputValue('#hst-in-world'), 'first');
  assert.ok(await page.isDisabled('#hst-in-boost'), 'no boost on the first era');
  assert.match(await page.textContent('.hst-row[data-key="boost"] .hst-help'), /Not on "The first era only, to 60"/);
  assert.ok(await page.isVisible('.hst-row[data-key="weather"] .hx-chip'), 'a coming setting says so');
  await shot(page, '2-templates.png', { el: '.hst-s2' });
  await shot(page, '3-settings-panel.png', { el: '.hst-s3' });
  // a slider: Custom (from Vanilla+), the summary follows
  await page.locator('#hst-in-xp').fill('4');
  await page.waitForFunction(() => document.querySelector('.hst-sum-t').textContent === 'Custom (from Vanilla+)');
  assert.match(await page.textContent('.hst-sum'), /XP 4x/);
  assert.equal(await page.textContent('.hst-row[data-key="xp"] .hst-val'), '4x');
  // a choice that opens the boost: the whole world
  await page.selectOption('#hst-in-world', 'whole');
  await page.waitForFunction(() => !document.getElementById('hst-in-boost').disabled);
  await page.check('#hst-in-boost');
  assert.match(await page.textContent('.hst-sum'), /one-time boost to 80/);
  await shot(page, '3b-custom.png', { el: '.hst-s3' });
  // back to the template
  await page.click('.hst-reset');
  assert.equal(await sumTitle(page), 'Vanilla+');
  // save as my template, then it is a card of its own; share and use a code
  await page.locator('#hst-in-rep').fill('3');
  await page.click('.hst-save-tpl');
  await page.fill('.hst-mine-name', 'Friday Vanilla');
  await page.click('.hst-mine .btn.primary');
  await page.waitForSelector('.hst-tpl[data-tpl^="mine:"]');
  assert.equal(await sumTitle(page), 'Friday Vanilla');
  await page.click('.hst-share');
  const code = await page.textContent('.hst-code');
  assert.match(code, /^LLR1\./);
  await page.click('.hst-tpl[data-tpl="classic"]');
  await page.fill('#hst-code-in', code);
  await page.click('.hst-code-in .btn');
  await page.waitForFunction(() => document.querySelector('.hst-sum-t').textContent === 'Custom (from Vanilla+)');
  assert.equal(await page.inputValue('#hst-in-rep'), '3');
  assert.deepEqual(errors, []);
});

test('Ask Claude: the paste-back fills the panel, leaves out unknown keys and clamps out-of-range ones; never who joins', async () => {
  const { page, errors } = await open();
  await page.click('.hst-vis-b[data-vis="private"]');
  await page.click('.hst-tpl[data-tpl="classic"]');
  await page.click('.hst-ask-open');
  await page.fill('#hst-ask-words', 'a cozy RP realm for 10 friends, slow levelling, no PvP');
  await page.click('.hst-show-prompt, .hst-way .hc-more');
  const prompt = await page.textContent('#hst-prompt');
  assert.match(prompt, /What I want: a cozy RP realm for 10 friends/);
  assert.match(prompt, /- xp: Experience\. a number from 0\.5 to 10/);
  assert.doesNotMatch(prompt, /- signup:/, 'who can join is never asked of the AI');
  assert.doesNotMatch(prompt, NO_BRANDS);
  await shot(page, '4-ask-claude.png', { el: '.hst-s3' });
  const answer = 'Sure! Here are the settings:\n```json\n' + JSON.stringify({
    template: 'rp', name: 'Cozy Hearth',
    settings: { xp: 0.2, rested: 3, realm_type: 'rp', max_players: 10, bots: 7, signup: 'open', weather_magic: true, boost: 'off', start_level: 20 },
    why: { xp: 'Slow levelling, as you asked.', realm_type: 'RP rules, no open-world PvP.', max_players: 'Room for 10 friends.' },
  }) + '\n```\nHave fun!';
  await page.fill('#hst-paste', answer);
  await page.click('.hst-ask-fill');
  await page.waitForSelector('.hst-answer');
  assert.equal(await page.inputValue('#hst-in-xp'), '0.5', 'clamped to its range');
  assert.equal(await page.inputValue('#hst-in-max_players'), '10');
  assert.equal(await page.inputValue('#hst-in-bots'), '5', 'snapped to its step');
  assert.equal(await page.inputValue('#hst-in-realm_type'), 'rp');
  assert.equal(await page.inputValue('#hst-in-signup'), 'invite', 'Private keeps invite-only: an answer never opens sign-up');
  assert.equal(await page.isChecked('#hst-in-rp_names'), true, 'from the RP template');
  const note = await page.textContent('.hst-answer');
  assert.match(note, /Changed to fit: Experience: 0\.2 to 0\.5 \(its range is 0\.5 to 10\)/);
  for (const k of ['weather_magic: not a setting', 'signup: you choose who can join yourself', 'Any start level: coming, not yet']) assert.ok(note.includes(k), k);
  assert.equal(await page.textContent('.hst-row[data-key="xp"] .hst-why'), 'Slow levelling, as you asked.');
  assert.equal(await page.inputValue('#hst-name'), 'Cozy Hearth');
  assert.equal(await sumTitle(page), 'Custom (from RP realm)');
  await shot(page, '5-ask-claude-filled.png', { el: '.hst-s3' });
  // a bad paste says so and changes nothing (the box stays open)
  await page.fill('#hst-paste', 'I think you should have fun!');
  await page.click('.hst-ask-fill');
  assert.match(await page.textContent('.hst-ask-status'), /No settings block/);
  assert.equal(await page.inputValue('#hst-in-xp'), '0.5');
  // the page's own checks, directly
  const r = await page.evaluate(() => window.LeylineHostFlow.parseAnswer('{"settings": {"xp": "2.2x", "honor": -4, "world": "mars"}}'));
  assert.deepEqual([r.template, r.values.xp, r.values.honor, r.values.world], ['classic', 2, 1, 'whole']);
  assert.match(r.leftOut.join(), /World and top level: not one of its choices/);
  assert.deepEqual(errors, []);
});

test('Ask my assistant: not connected says where; connected sends the words and its approved answer lands in the panel', async () => {
  const { page, errors } = await open();
  await page.click('.hst-vis-b[data-vis="public"]');
  await page.click('.hst-tpl[data-tpl="classic"]');
  await page.click('.hst-ask-open');
  await page.fill('#hst-ask-words', 'a PvP realm');
  await page.click('.hst-ask-send');
  await page.waitForFunction(() => /isn't connected/.test(document.querySelector('.hst-ask-status').textContent));
  assert.equal((await calls(page, 'host_plan_ask')).length, 0);
  await page.close();
  const b = await open({ agents: true });
  await b.page.click('.hst-vis-b[data-vis="public"]');
  await b.page.click('.hst-tpl[data-tpl="classic"]');
  await b.page.click('.hst-ask-open');
  await b.page.fill('#hst-ask-words', 'a PvP realm with a duel ladder');
  await b.page.click('.hst-ask-send');
  await b.page.waitForFunction(() => window.__asked === 'a PvP realm with a duel ladder');
  await b.page.evaluate(() => { window.__planAnswer = { template: 'pvp', values: Object.assign({}, window.LeylineHostFlow.catalog.base, window.LeylineHostFlow.catalog.templates.find((t) => t.id === 'pvp').set, { honor: 4 }),
    why: { honor: 'More honor for fights.' }, name: null, changed: [], left_out: [] }; });
  await b.page.waitForSelector('.hst-answer', { timeout: 10000 });
  assert.match(await b.page.textContent('.hst-answer'), /From your assistant/);
  assert.equal(await b.page.inputValue('#hst-in-honor'), '4');
  assert.equal(await b.page.inputValue('#hst-in-world'), 'top');
  assert.deepEqual(errors.concat(b.errors), []);
});

test('step 4: Host applies the plan, starts its ruleset, then the running realm card; Solo with bots is private only', async () => {
  const { page, errors } = await open();
  await page.click('.hst-vis-b[data-vis="public"]');
  await page.click('.hst-tpl[data-tpl="solo-bots"]');
  assert.equal(await page.getAttribute('.hst-vis-b[data-vis="private"]', 'aria-checked'), 'true', 'switched to Private');
  assert.match(await page.textContent('.hst-status-line'), /private only/);
  await page.click('.hst-vis-b[data-vis="public"]');
  assert.equal(await page.inputValue('#hst-in-world'), 'whole', 'Public: not the solo world');
  await page.click('.hst-tpl[data-tpl="fun"]');
  await page.locator('#hst-in-xp').fill('7');
  await page.fill('#hst-own-rules', 'Be kind.\nNo spam.');
  assert.match(await page.textContent('.hst-what'), /Community realms list within a minute/);
  await shot(page, '6-host-step.png', { el: '.hst-s4' });
  await page.fill('#hst-name', 'Fun Town');
  await page.fill('#hst-gm', 'HOSTER');
  await page.click('#hst-go');
  await page.waitForSelector('.hst-run .hc-prog');
  const applied = (await calls(page, 'host_flow_apply'))[0];
  assert.equal(applied.template, 'fun');
  assert.equal(applied.visibility, 'public');
  assert.equal(applied.values.xp, 7);
  assert.equal(applied.values.signup, 'open', 'Public: anyone may make an account, unless the host changed it');
  assert.deepEqual(applied.rules, ['Be kind.', 'No spam.']);
  assert.deepEqual((await calls(page, 'host_choose'))[0], { profile: 'main', clientId: 'c1' });
  assert.deepEqual((await calls(page, 'host_start_quiet'))[0], { answers: { name: 'Fun Town', visibility: 'public', gm: 'HOSTER' } });
  await page.waitForSelector('.hst-live', { timeout: 15000 });
  assert.equal(await page.textContent('#hst-share'), `https://leylinelauncher.com/wrath/r/${RID}`);
  assert.match(await page.textContent('.hst-live-st'), /Fun Town is online · 3 playing · Custom \(from Fun realm\)/);
  assert.deepEqual(await page.$$eval('.hst-live-acts button', (l) => l.map((x) => x.textContent)),
    ['Copy its page', 'GM command center', 'Edit settings', 'Stop', 'Open the console']);
  await shot(page, '7-live.png', { el: '.hst-run' });
  assert.deepEqual(errors, []);
});

test('a realm on this PC: the console with Edit settings; save and restart applies, and says a restart is needed', async () => {
  const realm = { realm_id: RID, name: 'Fun Town', visibility: 'private', relay: 'relay.openvanilla.app', profile: 'main' };
  const { page, errors } = await (async () => {
    const pg = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    const errs = [];
    pg.on('pageerror', (e) => errs.push(String(e)));
    await pg.addInitScript(FAKE, { modes: MODES, rid: RID, realm });
    await pg.addInitScript(() => { window.__current = { template: 'rp', values: { xp: 2 }, visibility: 'private', rules: ['Be kind.'] }; });
    await pg.goto(INDEX);
    await pg.click('.nav[data-page="host"]');
    await pg.waitForSelector('.hst-editbar');
    return { page: pg, errors: errs };
  })();
  await page.click('.hst-edit-open');
  await page.waitForSelector('.hst-s3');
  assert.equal(await page.inputValue('#hst-in-xp'), '2');
  assert.equal(await sumTitle(page), 'Custom (from RP realm)');
  assert.equal(await page.inputValue('#hst-own-rules'), 'Be kind.');
  assert.match(await page.textContent('.hst-edit-go'), /needs a restart/);
  await page.locator('#hst-in-xp').fill('1');
  await page.click('.hst-save-restart');
  await page.waitForSelector('.hst-run');
  const a = (await calls(page, 'host_flow_apply'))[0];
  assert.deepEqual([a.template, a.values.xp, a.visibility], ['rp', 1, 'private']);
  assert.deepEqual((await calls(page, 'host_start_quiet'))[0], { answers: {} });
  assert.deepEqual(errors, []);
});

test('the Guide tab: start to finish, every template and every setting from the same list', async () => {
  const { page, errors } = await open();
  await page.click('#hc-tab-guide');
  await page.waitForSelector('#hst-guide');
  const t = await page.textContent('#hst-guide');
  for (const s of ['What the server software is', 'Private or Public', 'The templates', 'Every setting', 'Ask Claude', 'Invite friends', 'Go public', 'Be a node', 'Backups and updates', "When something's wrong"]) {
    assert.ok(t.includes(s), s);
  }
  for (const n of ['Pure vanilla', 'Vanilla+', 'Hardcore', 'Event night', 'Blank', 'Roaming bots', 'Weather and time of day']) assert.ok(t.includes(n), n);
  assert.doesNotMatch(t, NO_BRANDS);
  await page.click('#hst-guide .hst-guide-h .hc-more');
  assert.equal(await page.evaluate(() => window.__opened), 'hostarealm');
  await page.click('#hst-guide details:nth-of-type(1) summary');
  await shot(page, '8-guide.png', { el: '#hst-guide' });
  assert.deepEqual(errors, []);
});
