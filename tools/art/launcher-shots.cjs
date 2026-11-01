// Launcher screens for the site and the README (launcher/ui with a fake core): node tools/art/launcher-shots.cjs launcher/ui <out> [clients play den host realms]; W/H/DPR env for size (the hero shot: W=1600 H=900 DPR=2)
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const UI = path.resolve(process.argv[2]);
const OUT = path.resolve(process.argv[3]);
const ONLY = process.argv.slice(4);
const INDEX = 'file://' + path.join(UI, 'index.html');
const CAT = JSON.parse(fs.readFileSync(path.join(UI, '..', '..', 'stream', 'modepacks.json'), 'utf8'));
const MODES = CAT.modes.map((m) => ({ id: m.id, name: m.name, line: m.line, version: m.version, author: m.author, based_on: m.based_on,
  status: m.status, profile: m.profile, scripts: m.scripts, env: m.env, mods: m.mods, den: m.den, tags: m.tags, jam: null, credit: m.credit || null,
  settings: m.settings, picture: m.picture_data || null, own: false }));

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.page', o.page || 'play'); localStorage.setItem('leyline.mode', 'online'); localStorage.setItem('leyline.joined', JSON.stringify(['relay.openvanilla.app:40002'])); } catch (_) { /* fine */ }
  const clients = [
    { id: 'c335a-1', name: 'Northrend', path: 'C:\\Games\\Northrend', build: '3.3.5a', realmlist: '' },
    { id: 'c1121-1', name: 'Classic', path: 'C:\\Games\\Classic', build: '1.12.1', realmlist: '' },
    { id: 'c243-1', name: 'Outland', path: 'C:\\Games\\Outland', build: '2.4.3', realmlist: '' },
    { id: 'c434-1', name: 'Shattered World', path: 'C:\\Games\\Shattered', build: '4.3.4', realmlist: '' },
    { id: 'c548-1', name: 'Misty Isles', path: 'C:\\Games\\Misty Isles', build: '5.4.8', realmlist: '' },
  ];
  const realms = [
    { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 42, bots: 120, address: 'play.openvanilla.app', mods: [] },
    { slug: 'c-jgt5rpofn7yaisep', realm_id: 'jgt5rpofn7yaisep', name: "Builders' Realm", state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: ['custom'], players: 3, bots: 0, address: 'relay.openvanilla.app:40030', mods: [] },
    { slug: 'c-aaaa', realm_id: 'aaaaaaaaaaaaaaaa', name: 'Frost Haven', state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: ['pve'], players: 17, bots: 30, address: 'relay.openvanilla.app:40002', mods: [], tagline: 'A friendly levelling realm' },
    { slug: 'c-bbbb', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Duel Town', state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: ['pvp'], players: 7, bots: 0, address: 'relay.openvanilla.app:40003', mods: [] },
  ];
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: '0.9', settings: { clients, selected_client: 'c335a-1', selected_realm: 'play.openvanilla.app',
        wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: 'C:\\Users\\me\\AppData\\Leyline' };
      case 'realms_list': return { realms: JSON.parse(JSON.stringify(realms)), network: { realms: 3, players: 66, bots: 150 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 24, players: 42 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c335a-1', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: '/kit', is_kit: true, problems: [], seeds: [], realm: null };
      case 'host_modes': return o.modes;
      case 'host_profiles': return [[], null];
      case 'hostpack_info': return { installed: { version: '7' }, available: { version: '7' }, update: false, kit_dir: '/kit' };
      case 'docker_info': return { state: 'ready', version: '27', help: '', get_url: '' };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return false;
      case 'agents_state': return { allow: true, pending: [], audit: [], connections: { claude_code: 'connected', desktop: 'none' }, command: 'leyline mcp', tools: {} };
      case 'den_state': return { lite: { available: true, running: false, address: '127.0.0.1', account: 'DEN', password: 'den', den_dir: '/den' },
        kit: true, docker_den: false, other_realm: null, account: '', saves: 'lite',
        dens: [{ name: 'house', bytes: 300, props: 42, saved: '2026-11-01T01:00:00Z' }, { name: 'garden', bytes: 300, props: 18, saved: '2026-10-31T19:00:00Z' }],
        packs: [] };
      default: throw 'unknown ' + cmd;
    }
  } } };
};

const SCREENS = {
  // the play bar's realm picker open over Home
  clients: async (page) => { await page.click('#hero-next'); await page.waitForTimeout(900); await page.click('#pb-client'); await page.waitForTimeout(400); },
  mods: async (page) => { await page.evaluate(() => window.LeylineNav.show('mods')); await page.waitForTimeout(500); await page.click('#page-mods >> text=Discover'); await page.waitForTimeout(1200); },
  play: async (page) => { await page.click('#pb-realm-btn'); await page.waitForTimeout(300); },
  den: async (page) => { await page.evaluate(() => window.LeylineNav.show('den')); await page.waitForSelector('#yd-play'); },
  host: async (page) => { await page.evaluate(() => window.LeylineNav.show('host')); await page.waitForSelector('#hx .hx-card'); },
  realms: async (page) => { await page.evaluate(() => window.LeylineNav.show('realms')); await page.waitForSelector('#rl-list .rl-row, #rl-list > *'); },
};

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch();
  for (const [name, go] of Object.entries(SCREENS)) {
    if (ONLY.length && !ONLY.includes(name)) continue;
    const ctx = await browser.newContext({ colorScheme: 'dark', viewport: { width: +(process.env.W||1280), height: +(process.env.H||800) }, deviceScaleFactor: +(process.env.DPR||1) });
    const page = await ctx.newPage();
    page.on('pageerror', (e) => console.error(name, String(e)));
    await page.route('https://**', (r) => (/leyline-bg|backdrop|\.webp$/.test(r.request().url()) ? r.fulfill({ path: path.join(UI, 'art', 'leyline-bg.webp'), contentType: 'image/webp' }) : r.abort()));
    await page.addInitScript(FAKE, { modes: MODES });
    await page.goto(INDEX);
    await page.waitForFunction(() => window.LeylineNav && window.__calls.some(([c]) => c === 'realms_list'));
    await page.waitForTimeout(400);
    await go(page);
    await page.waitForTimeout(600);
    await page.screenshot({ path: path.join(OUT, name + '.png') });
    console.log('wrote', name);
    await ctx.close();
  }
  await browser.close();
})();
