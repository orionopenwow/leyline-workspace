// The launcher's screenshots for the docs and the site (docs/AUDIT.md "Mobile and screenshots"; owner 3 Nov: "new
// screenshots if things updated, a full sweep pass"): the launcher UI (launcher/ui) in headless Chromium with a fake
// Tauri core, as launcher/tests do, one JPEG per main page in docs/screens/launcher/ (each under 200 KB).
//
// The fake core is the first-run walk's (launcher/tests/firstrun.browser.test.cjs FAKE, read from that file so the
// two never drift apart), with a player who already has two game folders, two mods and two loadouts, the Host pack
// and Docker ready, and the game modes from stream/playmodes.json and stream/modepacks.json. Every page: no page
// errors and no game or publisher names, or the script says so and exits 1.
//
//   NODE_PATH=$(npm root -g) node tools/audit/launcher_shots.cjs [out dir]      (default docs/screens/launcher)
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const ROOT = path.join(__dirname, '..', '..');
const INDEX = 'file://' + path.join(ROOT, 'launcher', 'ui', 'index.html');
const PICTURE = path.join(ROOT, 'launcher', 'ui', 'art', 'leyline-bg.webp');
const OUT = path.resolve(process.argv[2] || path.join(ROOT, 'docs', 'screens', 'launcher'));
const SIZE = { width: 1280, height: 800 };
const MAX = 200 * 1024;
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;

// The first-run walk's fake core, as source text: `const FAKE = () => { ... };` up to its closing line.
function firstRunFake() {
  const src = fs.readFileSync(path.join(ROOT, 'launcher', 'tests', 'firstrun.browser.test.cjs'), 'utf8');
  const a = src.indexOf('const FAKE = () => {');
  const b = src.indexOf('\n};\n', a);
  if (a < 0 || b < 0) throw new Error('launcher/tests/firstrun.browser.test.cjs has no "const FAKE = () => {" block any more: update launcher_shots.cjs');
  return src.slice(a + 'const FAKE = '.length, b + 2);
}

const MODES = JSON.parse(fs.readFileSync(path.join(ROOT, 'stream', 'playmodes.json'), 'utf8')).modes || [];
const PACKS = (JSON.parse(fs.readFileSync(path.join(ROOT, 'stream', 'modepacks.json'), 'utf8')).modes || []).map((m) => ({
  id: m.id, name: m.name, line: m.line, version: m.version, author: m.author, based_on: m.based_on, status: m.status, profile: m.profile,
  scripts: m.scripts, env: m.env, mods: m.mods, den: m.den, tags: m.tags, jam: null, credit: m.credit || null, settings: m.settings,
  picture: m.picture_data || null, own: false }));

// A player some days in: two game folders, two mods on, two loadouts, the Host pack and Docker ready.
const SETTLED = (o) => {
  const S = window.__S;
  S.clients.push({ id: 'c335a-1', name: 'Wrath 3.3.5a', path: 'C:\\Games\\Wrath', build: '3.3.5a', realmlist: '', locale: 'enUS' },
    { id: 'c1121-2', name: 'Vanilla 1.12.1', path: 'C:\\Games\\Vanilla', build: '1.12.1', realmlist: '', locale: 'enUS' });
  S.selected_client = 'c335a-1';
  S.seen = ['tour', 'play', 'den', 'host', 'files', 'cheer-hosted', 'cheer-friend', 'cheer-mod'];
  const mod = (key, name) => ({ key, name, version: '1.0.0', source: 'catalog', update: null, in_catalog: true, enabled: true, files: [], addons: [name], patches: [], natives: [], warnings: [] });
  const mods = [mod('mod:tidy-bags', 'Tidy Bags'), mod('mod:quest-helper', 'Quest Helper')];
  const book = { active: 'questing', previous: '', loadouts: [
    { id: 'questing', name: 'Questing', items: ['mod:tidy-bags', 'mod:quest-helper'], gfx: 'balanced', from: '', created_at: 1 },
    { id: 'raid-night', name: 'Raid night', items: ['mod:tidy-bags'], gfx: 'high', from: '', created_at: 2 }] };
  const copy = (x) => JSON.parse(JSON.stringify(x));
  const base = window.__TAURI__.core.invoke;
  let flow = null;
  window.__TAURI__.core.invoke = async (cmd, args) => {
    switch (cmd) {
      case 'mods_installed': return { client: args.client, build: '3.3.5a', mods: args.client === 'c335a-1' ? copy(mods) : [] };
      case 'loadouts_status': return copy({ client: 'c335a-1', active: book.active, previous: book.previous, loadouts: book.loadouts,
        installed: mods.map((m) => ({ key: m.key, name: m.name, enabled: m.enabled })), drift: [] });
      case 'modes_list': return { modes: copy(o.modes), from_cache: false, builtin: false, saved_at: null, error: null };
      case 'host_modes': return copy(o.packs);
      case 'host_mode_use': return null;
      case 'host_profiles': return [['main', 'vanillaplus', 'progression', 'brawl', 'dalaran', 'solo', 'den'].map((id) =>
        ({ id, label: id, version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c335a-1', name: 'Wrath 3.3.5a', path: 'C:\\Games\\Wrath' }], note: null })), null];
      case 'host_choose': return { profile: args.profile, client_dir: '' };
      case 'host_flow_apply': flow = args; return { profile: 'main', visibility: args.visibility, mode_name: 'x', lines: [] };
      case 'host_flow_current': return flow;
      case 'hostpack_info': return { installed: { version: '7' }, available: { version: '7', size: 400000 }, update: false, kit_dir: 'C:\\Leyline\\host' };
      case 'docker_info': return { state: 'ready', version: '27', help: 'Docker is ready.', get_url: 'https://docs.docker.com/desktop/', can_start: false, cli: null, desktop: null };
      case 'host_progress': return { idle: true, kind: '', running: false, code: null, step: '', n: 0, of: 0, secs: 0, problem: null };
      case 'host_console': return { set_up: false, linked: false, name: '', realm_id: '', profile: 'main', core: 'azerothcore' };
      case 'host_plan_take': return null;
      case 'addons_should_offer': return false;
      default: return base(cmd, args);
    }
  };
};

const shots = [];
const problems = [];
async function shoot(page, errors, name) {
  await page.waitForTimeout(500);   // the page fade and the menus' open animation
  const words = await page.evaluate(() => [...document.querySelectorAll('.page.on, header, #play-form, [role="dialog"]:not([hidden]), #gate, .gate')]
    .filter((e) => e.offsetParent || getComputedStyle(e).position === 'fixed').map((e) => e.innerText).join('\n'));
  const brand = words.match(BRANDS);
  if (brand) problems.push(`${name}: a game or publisher name shows: "${brand[0]}"`);
  if (errors.length) problems.push(`${name}: page errors: ${errors.splice(0).join('; ').slice(0, 300)}`);
  const file = path.join(OUT, name + '.jpg');
  let q = 82;
  let buf = await page.screenshot({ type: 'jpeg', quality: q });
  while (buf.length > MAX && q > 40) { q -= 8; buf = await page.screenshot({ type: 'jpeg', quality: q }); }
  if (buf.length > MAX) problems.push(`${name}: ${Math.round(buf.length / 1024)} KB even at quality ${q}`);
  fs.writeFileSync(file, buf);
  shots.push(`${name}.jpg ${Math.round(buf.length / 1024)} KB`);
  process.stdout.write(`  ${name}.jpg (${Math.round(buf.length / 1024)} KB)\n`);
}

const go = async (page, name) => {
  await page.click(`.side .nav[data-page="${name}"]`);
  await page.waitForFunction((n) => document.querySelector('#page-' + n + '.on'), name);
};

async function main() {
  fs.mkdirSync(OUT, { recursive: true });
  for (const f of fs.readdirSync(OUT)) if (f.endsWith('.jpg')) fs.rmSync(path.join(OUT, f));
  const browser = await chromium.launch();
  try {
    const ctx = await browser.newContext({ colorScheme: 'dark', viewport: SIZE });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    page.on('dialog', (d) => d.dismiss());
    await page.route('https://**', (r) => (r.request().resourceType() === 'image' ? r.fulfill({ path: PICTURE, contentType: 'image/webp' }) : r.fulfill({ status: 404, body: '' })));
    await page.addInitScript({ content: `(${firstRunFake()})();` });
    await page.addInitScript(SETTLED, { modes: MODES, packs: PACKS });
    await page.goto(INDEX);
    await page.waitForFunction(() => window.LeylineNav && window.LeylineApp && window.LeylineApp.state());

    // the sign-in screen (gate.js), then Log in with a pasted test Hero Key (the fake core's, not a real one)
    await page.waitForSelector('#hf-signin');
    if (!(await page.evaluate(() => window.LeylineGate.isOpen()))) await page.click('#hf-signin');
    await page.waitForSelector('#gate-browser');
    await shoot(page, errors, '01-sign-in');
    await page.click('#gate-keylink');
    await page.waitForSelector('#gate-key');
    await page.fill('#gate-key', 'ovk1.TestTestAa.' + 'k'.repeat(43));
    await page.click('#gate-keyform button[type="submit"]');
    await page.click('#gate-go');
    await page.waitForSelector('#hf-me-btn');

    await go(page, 'play');
    await page.waitForFunction(() => document.getElementById('pb-realm').textContent === 'Wrath Unleashed');
    await shoot(page, errors, '02-home');
    await go(page, 'realms');
    await page.waitForFunction(() => /Wrath Unleashed/.test(document.getElementById('rl-pane').textContent));
    await shoot(page, errors, '03-realms-official');
    await page.click('#rl-tab-community');
    await page.waitForFunction(() => /Frost Haven/.test(document.getElementById('rl-pane').textContent));
    await shoot(page, errors, '04-realms-community');
    await page.click('#rl-tab-official');

    await go(page, 'play');
    await page.click('#pb-realm-btn');
    await page.click('#pb-rmenu >> text=Your Den');
    await page.waitForSelector('#page-den.on');
    await shoot(page, errors, '05-your-den');

    await go(page, 'mods');
    await page.click('#tab-mods-discover');
    await page.waitForFunction(() => /Golf Cart/.test(document.getElementById('pane-discover').textContent));
    await shoot(page, errors, '06-mods-discover');
    await page.click('#discover-tabs .ly-tab[data-tab="addons"]');
    await page.waitForFunction(() => /Tidy Bags/.test(document.getElementById('pane-discover').textContent));
    await shoot(page, errors, '07-addons-discover');
    await page.click('#tab-mods-installed');
    await page.waitForSelector('#lo-box:not([hidden]) .lo-chip', { timeout: 10000 });
    await shoot(page, errors, '08-installed-loadouts');
    await page.click('#tab-mods-gamemodes');
    await page.waitForSelector('#pane-gamemodes.on, #pane-gamemodes:not([hidden])');
    await shoot(page, errors, '09-game-modes');

    await go(page, 'host');
    await page.waitForSelector('#hst .hst-vis');
    await page.waitForSelector('.hst-ready.ok', { timeout: 10000 }).catch(() => {});
    await shoot(page, errors, '10-host-start');
    await page.click('.hst-vis-b[data-vis="private"]');
    await page.locator('.hst-s2').scrollIntoViewIfNeeded();
    await shoot(page, errors, '11-host-templates');
    await page.click('.hst-tpl[data-tpl="vanilla-plus"]');
    await page.waitForSelector('.hst-s3 .hst-group');
    await page.locator('.hst-s3').scrollIntoViewIfNeeded();
    await shoot(page, errors, '12-host-settings');

    for (const [n, name] of [['13', 'settings'], ['14', 'forum'], ['15', 'news'], ['16', 'agents'], ['17', 'help']]) {
      await go(page, name);
      await shoot(page, errors, `${n}-${name}`);
    }
    await page.click('#hf-me-btn');
    await page.click('#hf-panel [role="menuitem"]:has-text("Manage account")');
    await page.waitForSelector('#page-account.on');
    await shoot(page, errors, '18-account');
    const unknown = [...new Set(await page.evaluate(() => window.__unknown))];
    if (unknown.length) process.stdout.write(`  (core calls the fake doesn't answer: ${unknown.join(', ')})\n`);
    await ctx.close();
  } finally {
    await browser.close();
  }
  process.stdout.write(`${shots.length} launcher screenshots in ${path.relative(ROOT, OUT) || OUT}\n`);
  if (problems.length) {
    process.stdout.write('PROBLEMS:\n  ' + problems.join('\n  ') + '\n');
    process.exit(1);
  }
}

main().catch((e) => { process.stderr.write('launcher shots broke: ' + String(e && e.stack || e).slice(0, 800) + '\n'); process.exit(2); });
