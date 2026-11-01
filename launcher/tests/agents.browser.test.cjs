// The launcher's Agents page (ui/agents.js; src-tauri/src/agents.rs) in Chromium with a fake Tauri core: the Agents item
// in the side menu's foot, Allow agents (off by default), the three tiers in plain words, Connect for Claude Code (the
// command, Add for me, Remove), the Claude desktop app and other agents, the audit list, and an agent's request as the
// approval card on another page (Approve runs agents_answer; Deny; text never read as markup).
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/agents.browser.test.cjs
//   AGENTS_SHOTS=<dir> also writes screenshots (dark and light).
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const SHOTS = process.env.AGENTS_SHOTS;

const FAKE = (opts) => {
  const calls = (window.__calls = []);
  try { if (opts.page) localStorage.setItem('leyline.page', opts.page); else localStorage.removeItem('leyline.page'); } catch (_) { /* fine */ }
  const now = Math.floor(Date.now() / 1000);
  const exe = 'C:\\Users\\pat\\AppData\\Local\\Leyline\\Leyline.exe';
  const st = window.__agents = {
    allow: !!opts.allow,
    pending: opts.pending ? [{ id: '0123456789abcdef', tool: 'mod_install', args: { client: 'c1', project: 'mod:quest-helper' },
      card: { title: 'Install Quest Helper <b>x</b>', lines: ['Into your client "Main".', 'From the Workshop, by Bramble, version 1.4.', 'You can turn it off or remove it in Mods › Installed.'] },
      from: 'claude-code', at: now - 5, expires: now + 115 }] : [],
    audit: [
      { at: now - 60, from: 'claude-code', tool: 'realm_status', tier: 'read', outcome: 'done', note: '' },
      { at: now - 300, from: 'claude-code', tool: 'forum_reply', tier: 'ask', outcome: 'denied', note: 'Reply on the forum as you' },
      { at: now - 900, from: 'Claude', tool: 'mod_install', tier: 'ask', outcome: 'timed out', note: 'Install Quest Helper' },
      { at: now - 1200, from: 'claude-code', tool: 'host_logs', tier: 'read', outcome: 'agents off', note: '' },
    ],
    exe,
    command: `claude mcp add --scope user leyline -- "${exe}" mcp`,
    json: JSON.stringify({ mcpServers: { leyline: { command: exe, args: ['mcp'] } } }, null, 2),
    claude_code: { state: opts.ccState || 'no', path: 'C:\\Users\\pat\\.claude.json' },
    desktop: { state: 'missing', path: 'C:\\Users\\pat\\AppData\\Roaming\\Claude\\claude_desktop_config.json' },
    never: ['Sign in, see or change a password, a Hero Key or your account', 'Anything with money', 'Delete files, remove a game client, wipe a realm',
      'Make your realm public, open its sign-up, or make invites', 'Kick, ban or unban players, or change who may join',
      "Change your realm list address, or your launcher's updates", "Use the website's staff tools"],
    now,
  };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [], selected_client: null, selected_realm: 'play.openvanilla.app',
        wine: { enabled: true, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app',
        browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'agents_state': return JSON.parse(JSON.stringify(st));
      case 'agents_set_allow': st.allow = args.on; return null;
      case 'agents_connect':
        st.claude_code.state = args.on ? 'connected' : 'no';
        return args.on ? 'Added. The old file is kept as .claude.json.leyline-backup-1. Start a new Claude Code session to use it.' : 'Removed.';
      case 'agents_answer': {
        const p = st.pending.shift();
        st.audit.unshift({ at: now, from: p.from, tool: p.tool, tier: 'ask', outcome: args.approve ? 'done' : 'denied', note: '' });
        return args.approve ? 'Installed Quest Helper 1.4 into "Main".' : 'Denied. Nothing was done.';
      }
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts) {
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, opts || {});
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineNav && window.LeylineAgents && window.LeylineAgents.state());
  return { page, errors };
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c).map(([, a]) => a), cmd);
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, name + '.png') }); };

test('the Agents item, off by default, the tiers in plain words, Connect and the audit', async () => {
  const { page, errors } = await open();
  assert.deepEqual(await page.$$eval('.side-foot .nav', (b) => b.map((x) => x.dataset.page)), ['settings', 'agents', 'help']);
  await page.click('.nav[data-page="agents"]');
  assert.ok(await page.isVisible('#page-agents'));
  assert.equal(await page.isChecked('#ag-allow'), false, 'off by default');
  assert.match(await page.textContent('#ag-allow-note'), /no answers at all/);
  assert.match(await page.textContent('#ag-wait'), /Nothing can ask while agents are off/);
  assert.equal(await page.locator('#ag-t-read li').count(), 11);   // + Den status, catalog, recipes and messages (1 Nov)
  assert.equal(await page.locator('#ag-t-ask li').count(), 9);   // + place a Den layout, the companion says, save a recipe (1 Nov)
  assert.equal(await page.locator('#ag-t-never li').count(), 7);
  assert.match(await page.textContent('#ag-t-never'), /password.*money.*Delete files.*public.*ban/s);
  assert.equal(await page.textContent('#ag-cc-cmd'), 'claude mcp add --scope user leyline -- "C:\\Users\\pat\\AppData\\Local\\Leyline\\Leyline.exe" mcp');
  assert.equal(await page.textContent('#ag-raw'), '"C:\\Users\\pat\\AppData\\Local\\Leyline\\Leyline.exe" mcp');
  assert.equal(await page.textContent('#ag-cc-state'), 'Not added');
  assert.equal(await page.textContent('#ag-dt-state'), 'App not found');
  assert.deepEqual(await page.$$eval('#ag-audit .ag-res', (r) => r.map((x) => x.textContent)),
    ['Done', 'You said no', 'No answer, not done', 'Blocked: agents were off']);
  assert.match(await page.textContent('#ag-audit li:first-child'), /claude-code.*If a realm is up/);
  // turn it on
  await page.click('.ag-allow');
  assert.deepEqual((await calls(page, 'agents_set_allow')).pop(), { on: true });
  await page.waitForFunction(() => document.getElementById('ag-allow-note').textContent.startsWith('On'));
  // Add for me, then Remove
  await page.click('#ag-cc-add');
  assert.deepEqual((await calls(page, 'agents_connect')).pop(), { target: 'claude_code', on: true });
  await page.waitForFunction(() => document.getElementById('ag-cc-state').textContent === 'Added');
  assert.match(await page.textContent('#ag-status'), /old file is kept/);
  assert.ok(await page.isHidden('#ag-cc-add'));
  await page.click('#ag-cc-del');
  assert.deepEqual((await calls(page, 'agents_connect')).pop(), { target: 'claude_code', on: false });
  await page.waitForFunction(() => document.getElementById('ag-cc-state').textContent === 'Not added');
  const text = await page.evaluate(() => document.getElementById('page-agents').innerText);
  assert.doesNotMatch(text, /world of warcraft|\bwow\b|warcraft|lich king|blizzard|3\.3\.5|1\.12/i);
  assert.deepEqual(errors, []);
  await page.close();
});

test('an agent asks: the card on another page, Approve runs it in the launcher', async () => {
  const { page, errors } = await open({ allow: true, pending: true, page: 'mods' });
  await page.waitForSelector('#ag-card:not([hidden])');
  assert.equal(await page.textContent('#ag-card-title'), 'Install Quest Helper <b>x</b>', 'text, not markup');
  assert.equal(await page.locator('#ag-card-title b').count(), 0);
  assert.match(await page.textContent('#ag-card-from'), /claude-code asks/i);
  assert.match(await page.textContent('#ag-card-left'), /^1\d\d s left$/);
  assert.deepEqual(await page.$$eval('#ag-card-lines li', (l) => l.map((x) => x.textContent)),
    ['Into your client "Main".', 'From the Workshop, by Bramble, version 1.4.', 'You can turn it off or remove it in Mods › Installed.']);
  assert.equal(await page.textContent('#nav-agents-count'), '1');
  await shot(page, 'agents-card-dark');
  await page.evaluate(() => { document.documentElement.dataset.base = 'light'; });
  await shot(page, 'agents-card-light');
  await page.click('#ag-card-yes');
  assert.deepEqual((await calls(page, 'agents_answer')).pop(), { id: '0123456789abcdef', approve: true });
  await page.waitForFunction(() => /Installed Quest Helper/.test(document.getElementById('ag-card-status').textContent));
  await page.waitForSelector('#ag-card[hidden]', { state: 'attached', timeout: 6000 });
  assert.ok(await page.isHidden('#nav-agents-count'));
  assert.deepEqual(errors, []);
  await page.close();
});

test('Deny from the Agents page; screenshots of the page', async () => {
  const { page, errors } = await open({ allow: true, pending: true, page: 'agents', ccState: 'connected' });
  await page.waitForSelector('#ag-wait .ag-req');
  assert.ok(await page.isHidden('#ag-card'), 'on the Agents page the request shows in the page, not as the floating card');
  assert.equal(await page.textContent('#ag-cc-state'), 'Added');
  await shot(page, 'agents-page-dark');
  await page.evaluate(() => { document.documentElement.dataset.base = 'light'; });
  await shot(page, 'agents-page-light');
  await page.evaluate(() => { document.documentElement.dataset.base = 'dark'; });
  await page.setViewportSize({ width: 1400, height: 1600 });
  await page.evaluate(() => window.scrollTo(0, 0));
  await shot(page, 'agents-page-full-dark');
  await page.click('#ag-wait .ag-req .btn:not(.primary)');
  assert.deepEqual((await calls(page, 'agents_answer')).pop(), { id: '0123456789abcdef', approve: false });
  await page.waitForFunction(() => /Nothing is waiting/.test(document.getElementById('ag-wait').textContent));
  assert.match(await page.textContent('#ag-status'), /Denied/);
  assert.equal(await page.textContent('#ag-audit .ag-res'), 'You said no');
  assert.deepEqual(errors, []);
  await page.close();
});

test('Help points to Agents', async () => {
  const { page, errors } = await open({ page: 'help' });
  await page.click('#page-help [data-page-go="agents"]');
  assert.ok(await page.isVisible('#page-agents'));
  assert.deepEqual(errors, []);
  await page.close();
});
