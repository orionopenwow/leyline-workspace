// The mobile release page's pictures (stream/mobile_page.py, /wrath/mobile; docs/MOBILE-RELEASE.md), made from the real
// pages at phone sizes in Chromium, never drawn by hand and never a picture of game data:
//   phone-play, phone-play-landscape, phone-realms, phone-mods, phone-account   the phone app (launcher/ui with the phone
//       layout, a fake core: launcher/tests/phonejourney.browser.test.cjs PHONE_SHOTS)
//   phone-join          Join a friend on the app's Play tab, an invite link pasted
//   phone-solo          Play solo on this phone, on the app's Play tab
//   phone-touch         the browser game's touch controls (stream/www-files/touch.js) over a stand-in for the game:
//                       a plain field, the real controls (stream/tests/touch-harness.cjs)
// Written as small WebP files to stream/www-files/art/mobile/ (served at /wrath/art/mobile/<name>.webp).
//
//   NODE_PATH=$(npm root -g) node tools/mobile-shots/mobile_shots.cjs        (needs Playwright and python3 with Pillow)
'use strict';
const path = require('node:path');
const fs = require('node:fs');
const os = require('node:os');
const { spawnSync } = require('node:child_process');
const { chromium } = require('playwright');

const REPO = path.join(__dirname, '..', '..');
const OUT = path.join(REPO, 'stream', 'www-files', 'art', 'mobile');
const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'mobile-shots-'));

function run(cmd, args, env) {
  const r = spawnSync(cmd, args, { cwd: REPO, env: Object.assign({}, process.env, env || {}), stdio: 'inherit' });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')} failed (${r.status})`);
}

async function appShots() {
  // the whole journey test, with its screenshots on (the same fake core as the tests)
  run(process.execPath, ['--test', path.join(REPO, 'launcher', 'tests', 'phonejourney.browser.test.cjs')], { PHONE_SHOTS: TMP });
  const map = { 'play-tab': 'phone-play', 'play-tab-landscape': 'phone-play-landscape', 'realms-tab': 'phone-realms',
    'mods-tab': 'phone-mods', 'account-signed-out': 'phone-account' };
  for (const [from, to] of Object.entries(map)) fs.renameSync(path.join(TMP, from + '.png'), path.join(TMP, to + '.png'));
}

async function cropShots(browser) {
  // Join a friend and Play solo, cut from the app's Play tab (the same page, an invite link typed in)
  const INDEX = 'file://' + path.join(REPO, 'launcher', 'ui', 'index.html');
  const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true, colorScheme: 'dark', deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  await page.route(/^https?:\/\//, (r) => r.abort());
  await page.addInitScript(() => {
    Object.defineProperty(Navigator.prototype, 'gpu', { configurable: true, get: () => ({}) });
    const platform = { os: 'android', mobile: true, node: false };
    const ok = { platform, social_status: false, social_ways: { herokey: true, google: true }, social_key_found: null, deep_link_take: null,
      get_state: { version: 'shots', settings: { clients: [], selected_client: null, selected_realm: 'leylinelauncher.com', wine: { enabled: false, command: 'wine', prefix: '' } },
        official_realm: 'leylinelauncher.com', browser_url: 'https://leylinelauncher.com/wrath', windows: false, data_dir: '/data', platform },
      realms_list: { realms: [], network: {}, from_cache: false, saved_at: 1, error: null } };
    window.__TAURI__ = { core: { invoke: async (c) => { if (c in ok) return JSON.parse(JSON.stringify(ok[c])); throw `Command ${c} not found`; } } };
  });
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylinePhone && window.LeylinePhone.on && document.querySelector('#page-phone.on'));
  await page.fill('#ph-invite', 'https://relay.leylinelauncher.com/join/abcdefghijk2345a#ABCDEFGHJKMNPQRSTVWX');
  await page.locator('.ph-join-box').screenshot({ path: path.join(TMP, 'phone-join.png') });
  await page.locator('.ph-solo-box').screenshot({ path: path.join(TMP, 'phone-solo.png') });
  await ctx.close();
}

async function touchShot(browser) {
  const { open } = require(path.join(REPO, 'stream', 'tests', 'touch-harness.cjs'));
  const { ctx, p } = await open(browser, { state: 'inworld', w: 844, h: 390 });
  await p.waitForTimeout(600);
  await p.screenshot({ path: path.join(TMP, 'phone-touch.png') });
  await ctx.close();
}

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  await appShots();
  const browser = await chromium.launch();
  try { await cropShots(browser); await touchShot(browser); } finally { await browser.close(); }
  // small WebP files: at most 720 px wide (1100 sideways), quality 78
  run('python3', ['-I', path.join(__dirname, 'to_webp.py'), TMP, OUT]);
  console.log('mobile shots ->', OUT);
})().catch((e) => { console.error(e); process.exit(1); });
