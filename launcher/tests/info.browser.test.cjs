// The "i" button (ui/info.js) on every page of the launcher: each page in index.html (and the Leaderboards panels on
// Modes and Boss Rush) has its own bubble, never the generic one, and every Learn more names a link the core knows
// (src-tauri/src/lib.rs LINKS), so it opens that page's docs on the website.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/info.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const HTML = fs.readFileSync(path.join(__dirname, '..', 'ui', 'index.html'), 'utf8');
const LIB = fs.readFileSync(path.join(__dirname, '..', 'src-tauri', 'src', 'lib.rs'), 'utf8');
const LINKS = new Set([...LIB.split('pub const LINKS')[1].split('];')[0].matchAll(/^\s*\("([a-z0-9-]+)",/gm)].map((m) => m[1]));
const PAGE_IDS = [...HTML.matchAll(/<section class="page[^"]*" id="page-([a-z0-9-]+)"/g)].map((m) => m[1]);

const FAKE = () => {
  try { localStorage.setItem('leyline.page', 'play'); } catch (_) { /* fine */ }
  window.__TAURI__ = { core: { invoke: async (cmd) => {
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [], selected_client: null, selected_realm: 'leylinelauncher.com',
        wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'leylinelauncher.com', browser_url: 'https://leylinelauncher.com/wrath', windows: true, data_dir: '/tmp' };
      case 'deep_link_take': return null;
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'social_status': return true;
      default: throw 'offline';
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

test('the page list is read from index.html', () => {
  assert.ok(PAGE_IDS.length >= 18, PAGE_IDS.join(','));
  for (const id of ['den', 'bossrush', 'characters', 'agents', 'help', 'account', 'findfiles', 'clients', 'phone']) {
    assert.ok(PAGE_IDS.includes(id), id);
  }
  assert.ok(LINKS.has('docs') && LINKS.has('docsden') && !LINKS.has('linkdiscord'));
});

test('every page has its own bubble, and every Learn more is a link the core knows', async () => {
  const page = await browser.newPage();
  await page.addInitScript(FAKE);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineInfo);
  const info = await page.evaluate(() => ({ pages: window.LeylineInfo.PAGES, sections: window.LeylineInfo.SECTIONS }));
  const ids = PAGE_IDS.concat(['modes']);   // Modes is drawn by modes.js into its page
  for (const id of ids) {
    await page.evaluate((p) => document.dispatchEvent(new CustomEvent('leyline:page', { detail: { page: p } })), id);
    await page.click('#ll-info');
    await page.waitForSelector('#ll-info-b:not([hidden])');
    const title = await page.textContent('#ll-info-t');
    assert.notEqual(title, 'Leyline', `${id}: its own bubble, not the generic one`);
    const link = await page.getAttribute('#ll-info-more', 'data-link');
    assert.ok(LINKS.has(link), `${id}: ${link} is in lib.rs LINKS`);
    const also = await page.$('#ll-info-also');
    if (also) assert.ok(LINKS.has(await also.getAttribute('data-link')), `${id}: the second button's link`);
    const n = await page.$$eval('#ll-info-b p', (l) => l.length);
    assert.ok(n >= 1 && n <= 4, `${id}: ${n} lines`);
    assert.doesNotMatch(await page.textContent('#ll-info-b'), /world of warcraft|\bwow\b|warcraft|lich king|blizzard|\d\.\d+\.\d/i);
    await page.keyboard.press('Escape');
  }
  for (const [id, sec] of Object.entries(info.sections)) {
    assert.ok(LINKS.has(sec.link), `${id}: ${sec.link}`);
    if (sec.also) assert.ok(LINKS.has(sec.also.link), `${id}: ${sec.also.link}`);
  }
  for (const k of Object.keys(info.pages)) assert.ok(ids.includes(k), `${k} is a page`);
  await page.close();
});

test('the Leaderboards panel has its own bubble while it is on screen', async () => {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.addInitScript(FAKE);
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineInfo && window.LeylineNav);
  await page.evaluate(() => window.LeylineNav.show('bossrush'));
  await page.evaluate(() => {
    const b = document.getElementById('br-boards');
    b.style.minHeight = '400px';
    document.getElementById('br').style.display = 'none';
    b.scrollIntoView({ block: 'center' });
  });
  await page.click('#ll-info');
  await page.waitForSelector('#ll-info-b:not([hidden])');
  assert.equal(await page.textContent('#ll-info-t'), 'Leaderboards');
  assert.equal(await page.getAttribute('#ll-info-more', 'data-link'), 'docsmodes');
  await page.close();
});
