// Mods & Addons › Discover › Graphics (ui/graphics.js, src-tauri/src/gfx.rs) in Chromium with a fake Tauri core:
// the tab, the client's settings, a preset in one click, a change saved, "Back to how it was".
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/graphics.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');

const FAKE = () => {
  const calls = (window.__calls = []);
  try { localStorage.setItem('leyline.page', 'mods'); localStorage.setItem('leyline.dcTab', 'graphics'); } catch (_) { /* fine */ }
  const tweaks = [
    { key: 'farclip', label: 'View distance', help: 'How far the world is drawn.', kind: 'slider', min: 177, max: 1277, step: 50, choices: [], default: '1277' },
    { key: 'gxMultisample', label: 'Anti-aliasing', help: 'Smoother edges.', kind: 'choice', min: 0, max: 0, step: 0, choices: [['1', 'Off'], ['2', '2x'], ['4', '4x'], ['8', '8x']], default: '1' },
    { key: 'gxVSync', label: 'Vertical sync', help: 'No tearing.', kind: 'toggle', min: 0, max: 1, step: 1, choices: [], default: '1' },
  ];
  let values = { farclip: '777', gxMultisample: '1', gxVSync: '1' };
  let backup = false;
  const state = () => ({ client: 'My Wrath', build: '3.3.5a', file: 'C:/Games/Wrath/WTF/Config.wtf', tweaks, values: { ...values },
    presets: [['low', 'Low'], ['balanced', 'Balanced'], ['high', 'High'], ['ultra', 'Ultra']], has_backup: backup });
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c1', name: 'My Wrath', path: 'C:/Games/Wrath', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c1', selected_realm: 'play.openvanilla.app', wine: { enabled: false, command: 'wine', prefix: '' } },
        official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'gfx_get': return state();
      case 'gfx_set': values = { ...values, ...args.values }; backup = true; return state();
      case 'gfx_preset': values = args.preset === 'ultra' ? { farclip: '1277', gxMultisample: '8', gxVSync: '1' } : values; backup = true; return state();
      case 'gfx_restore': values = { farclip: '777', gxMultisample: '1', gxVSync: '1' }; backup = false; return state();
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'realm_status': return { state: 'online', ping_ms: 10 };
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c1', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return false;
      case 'addons_status': return { prefs: { auto_update: true, dont_ask: true }, clients: [], available: null };
      case 'addons_should_offer': return false;
      case 'addons_auto_update': return [];
      default: return null;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

test('Graphics: the tab, a preset, a change saved, back to how it was', async () => {
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 860 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE);
  await page.goto(INDEX);
  await page.evaluate(() => { const b = document.querySelector('.nav[data-page="mods"]'); if (b) b.click(); });
  await page.evaluate(() => { const t = document.querySelector('.page-tab[data-tab="discover"]'); if (t) t.click(); });
  await page.waitForSelector('#discover-tabs .ly-tab[data-tab="graphics"]');
  const tabs = await page.$$eval('#discover-tabs .ly-tab', (b) => b.map((x) => x.textContent));
  assert.deepEqual(tabs, ['Mods', 'Addons', 'Themes', 'Graphics']);
  await page.click('#discover-tabs .ly-tab[data-tab="graphics"]');
  await page.waitForSelector('#gfx-farclip');
  assert.equal(await page.inputValue('#gfx-farclip'), '777');
  assert.equal(await page.inputValue('#gfx-gxMultisample'), '1');
  assert.ok(await page.isChecked('#gfx-gxVSync'));
  assert.ok(await page.isHidden('#themes-discover'));
  if (process.env.SHOTS) await page.screenshot({ path: path.join(process.env.SHOTS, 'graphics.png') });
  // a preset
  await page.click('.gfx-preset[data-preset="ultra"]');
  await page.waitForFunction(() => document.getElementById('gfx-gxMultisample') && document.getElementById('gfx-gxMultisample').value === '8');
  assert.match(await page.textContent('#gfx-note'), /Ultra saved/);
  assert.ok(await page.isVisible('#gfx-back'));
  // one change by hand
  await page.selectOption('#gfx-gxMultisample', '4');
  assert.equal(await page.textContent('#gfx-save'), 'Save 1 change');
  await page.click('#gfx-save');
  await page.waitForFunction(() => /Saved\./.test(document.getElementById('gfx-note').textContent));
  const set = await page.evaluate(() => window.__calls.filter(([c]) => c === 'gfx_set').pop()[1]);
  assert.deepEqual(set, { client: 'c1', values: { gxMultisample: '4' } });
  // back to how it was
  await page.click('#gfx-back');
  await page.waitForFunction(() => document.getElementById('gfx-gxMultisample').value === '1');
  assert.equal(await page.isVisible('#gfx-back'), false);
  // the other tabs still show their own content
  await page.click('#discover-tabs .ly-tab[data-tab="mods"]');
  assert.ok(await page.isHidden('#graphics-discover'));
  const text = await page.evaluate(() => document.body.innerText);
  assert.doesNotMatch(text, /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i);
  assert.deepEqual(errors, []);
  await ctx.close();
});
