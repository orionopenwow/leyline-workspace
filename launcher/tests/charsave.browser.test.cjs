// Characters that survive (ui/characters.js; app.rs chars_saves / chars_backup_now / chars_bring_preview / chars_bring /
// chars_ask_back; host/charsave.rs; docs/leyline/characters.md), in Chromium with a fake Tauri core:
//   * each character shows its last backup and which friends hold a sealed copy (or "Not backed up yet");
//   * each world has Back up now (chars_backup_now with its kind), and the answer is said;
//   * Bring to another world: pick a world, See what comes (the preview: what comes, the vault, what stays), Bring;
//     a refusal is said in plain words and Bring isn't offered;
//   * Your backups: characters brought back from friends, and "Get my characters back from friends";
//   * no brand words, no page errors.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/charsave.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const WRATH = { id: 'c335a-1', name: 'My Wrath', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '' };
const TAMSIN = { name: 'Tamsin', race: 4, class: 11, female: true, level: 70, played: 7260, last: 1762400000, race_label: 'Night Elf', class_label: 'Druid', side: 'alliance' };
const ODO = { name: 'Odo', race: 1, class: 1, female: false, level: 12, played: 600, last: 0, race_label: 'Human', class_label: 'Warrior', side: 'alliance' };
const LOCAL = [
  { kind: 'single', title: 'Single player', realm: 'Single player', supported: true, set_up: true, running: false, other_running: null, live: false, at: 5, chars: [TAMSIN, ODO] },
  { kind: 'vanilla', title: 'Original world', realm: 'Original world', supported: true, set_up: true, running: false, other_running: null, live: false, at: 9, chars: [] },
  { kind: 'bossrush', title: 'Boss Rush', realm: 'Boss Rush', supported: true, set_up: false, running: false, other_running: null, live: false, at: 0, chars: [] },
];
const PLAN_OK = { ok: true, why: '', name: 'Tamsin', level: 60, from_level: 70,
  comes: ['Tamsin at level 60', '12 items, waiting in your mailbox', '2 finished quests'],
  vault: [{ id: 'stock-335a:item:34000', n: 1, why: 'needs level 70' }, { id: 'stock-335a:item:90001', n: 3, why: "Original world doesn't have it" }],
  stays: ['Gold (500 gold): gold never travels.', '1 raid lockouts: they belong to the old world.', 'Talents: you pick them again.'] };

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.mode', 'online'); } catch (_) { /* fine */ }
  const now = Math.floor(Date.now() / 1000);
  const st = { version: 'test', settings: { clients: [o.client], selected_client: o.client.id, selected_realm: '', wine: { enabled: false, command: 'wine', prefix: '' } },
    official_realm: '', browser_url: '', windows: true, data_dir: '/tmp' };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return JSON.parse(JSON.stringify(st));
      case 'local_characters': return JSON.parse(JSON.stringify(o.local));
      case 'online_characters': return { realm: '', chars: [] };
      case 'chars_saves': return { holders: 3, keep_for_friends: true, held_bytes: 0, held_owners: 0, chars: [
        { id: 'caaaaaaaaaaaaaaa', name: 'Tamsin', world: 'single', seq: 4, made: now - 600, holders: ['Bo', 'Ann'], vault: 0 },
        { id: 'cbbbbbbbbbbbbbbb', name: 'Isra', world: '', seq: 9, made: now - 7200, holders: [], vault: 0 }] };
      case 'chars_backup_now': return 'Saved one character. 2 friends have a sealed copy.';
      case 'chars_bring_preview': return JSON.parse(JSON.stringify(o.plan));
      case 'chars_bring': return 'Tamsin is in Original world now. Your things wait in the mailbox. 2 went to your vault. Press Play there.';
      case 'chars_ask_back': return 'Asked 2 friends: copies they hold come back by themselves.';
      case 'world_status': return { supported: true, set_up: true, running: false, other_running: null };
      case 'engine_progress': return { idle: true, kind: 'engine', running: false, code: null, step: '', n: 0, of: 7 };
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });
async function open(o) {
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, Object.assign({ client: WRATH, local: LOCAL, plan: PLAN_OK }, o));
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineCharacters);
  await page.evaluate(() => window.LeylineNav.show('characters'));
  await page.waitForSelector('#ch-world-single li[data-name="Tamsin"] .ch-backup');
  return { page, ctx, errors };
}
const callsOf = (page, cmd) => page.evaluate((cmd) => window.__calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);
const noBrand = async (page) => assert.doesNotMatch(await page.evaluate(() => document.body.innerText), /world of warcraft|\bwow\b|warcraft|lich king|blizzard|azerothcore/i);

test('last backup, friends holding a copy, Back up now', async () => {
  const { page, ctx, errors } = await open();
  assert.match(await page.textContent('#ch-world-single li[data-name="Tamsin"] .ch-backup'), /^Backed up 10 min ago · copies with Bo, Ann$/);
  assert.equal(await page.textContent('#ch-world-single li[data-name="Odo"] .ch-backup'), 'Not backed up yet.');
  assert.equal(await page.locator('#ch-world-single li[data-name="Odo"] .ch-bring-open').count(), 0, 'nothing to bring without a save');
  await page.click('#ch-backup-single');
  await page.waitForFunction(() => /2 friends have a sealed copy/.test(document.getElementById('ch-status').textContent));
  assert.deepEqual(await callsOf(page, 'chars_backup_now'), [{ kind: 'single' }]);
  await noBrand(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Bring to another world: pick, see what comes and what goes to the vault, Bring', async () => {
  const { page, ctx, errors } = await open();
  await page.click('#ch-world-single li[data-name="Tamsin"] .ch-bring-open');
  await page.waitForSelector('#ch-bring');
  assert.equal(await page.inputValue('#ch-bring-to'), 'vanilla', 'the other world that is set up');
  assert.deepEqual(await page.$$eval('#ch-bring-to option', (o) => o.map((x) => x.value)), ['vanilla'], 'not the world it is in, not Boss Rush');
  assert.equal(await page.locator('#ch-bring-go').count(), 0, 'Bring only after the preview');
  await page.click('#ch-bring-look');
  await page.waitForSelector('#ch-bring-go');
  assert.deepEqual(await callsOf(page, 'chars_bring_preview'), [{ id: 'caaaaaaaaaaaaaaa', to: 'vanilla' }]);
  assert.match(await page.textContent('#ch-bring-comes'), /Tamsin at level 60.*waiting in your mailbox/);
  assert.match(await page.textContent('#ch-bring-vault'), /Item 34000: needs level 70.*Item 90001 ×3: Original world doesn't have it/);
  assert.match(await page.textContent('#ch-bring-stays'), /gold never travels/);
  assert.match(await page.textContent('#ch-bring'), /Gold never travels, and items can't be sent between worlds/);
  await page.click('#ch-bring-go');
  await page.waitForFunction(() => /is in Original world now/.test(document.getElementById('ch-status').textContent));
  assert.deepEqual(await callsOf(page, 'chars_bring'), [{ id: 'caaaaaaaaaaaaaaa', to: 'vanilla' }]);
  assert.equal(await page.locator('#ch-bring').count(), 0, 'the panel closes');
  await noBrand(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('a world that refuses says why, and offers no Bring', async () => {
  const { page, ctx, errors } = await open({ plan: { ok: false, why: 'Original world has no Blood Elf Paladins: they came later.', comes: [], vault: [], stays: [] } });
  await page.click('#ch-world-single li[data-name="Tamsin"] .ch-bring-open');
  await page.click('#ch-bring-look');
  await page.waitForFunction(() => /they came later/.test(document.getElementById('ch-bring').textContent));
  assert.equal(await page.locator('#ch-bring-go').count(), 0);
  await page.click('#ch-bring-close');
  assert.equal(await page.locator('#ch-bring').count(), 0);
  assert.deepEqual(await callsOf(page, 'chars_bring'), []);
  assert.deepEqual(errors, []);
  await ctx.close();
});

test('Your backups: a character back from a friend, and asking friends for copies', async () => {
  const { page, ctx, errors } = await open();
  assert.match(await page.textContent('#ch-world-backups li[data-name="Isra"]'), /Saved 2 h ago, from a friend's copy/);
  await page.click('#ch-ask-back');
  await page.waitForFunction(() => /Asked 2 friends/.test(document.getElementById('ch-status').textContent));
  await page.click('#ch-world-backups li[data-name="Isra"] button');
  await page.waitForSelector('#ch-bring');
  assert.deepEqual(await page.$$eval('#ch-bring-to option', (o) => o.map((x) => x.value)), ['single', 'vanilla']);
  await noBrand(page);
  assert.deepEqual(errors, []);
  await ctx.close();
});
