// Help › Report a bug (ui/bugreport.js) in Chromium with a fake Tauri core: the Help page's entry, signed out (Log in,
// or the website's form), the form filled in from the page the player was on and the play bar's realm, too short
// (nothing sent), sent (bug_report with only its fields, the log only when ticked, off by default), the thank-you,
// a rate limit, a sign-in the server no longer knows, and an unreachable server in plain words.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/bugreport.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');

const FAKE = (opts) => {
  const calls = (window.__calls = []);
  try { localStorage.setItem('leyline.page', opts.page || 'play'); } catch (_) { /* fine */ }
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [], selected_client: null, selected_realm: 'play.openvanilla.app',
        wine: { enabled: true, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app',
        browser_url: 'https://play.openvanilla.app/wrath', windows: false, data_dir: '/tmp' };
      case 'social_status': return !!window.__signed;
      case 'social_call':
        if (args.action === 'me') return { me: { pid: 'aaaaaaaaaa', name: 'Orion', tag: '1001', main: '', offline: false,
          profile: { bio: '', avatar: '', avatar_url: '', setup: true } }, avatars: [], ways: { herokey: true, google: true, discord: false } };
        throw 'unexpected social call ' + args.action;
      case 'bug_report':
        if (window.__fail) { const f = window.__fail; window.__fail = null; throw f; }
        return { ok: true, id: 'a1b2c3d4e5f6' };
      case 'open_link': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
  window.__signed = !!opts.signed;
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
  await page.waitForFunction(() => window.LeylineNav && window.LeylineBugReport);
  return { page, errors };
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter(([x]) => x === c).map(([, a]) => a), cmd);
async function fromHelp(page, before) {
  if (before) await page.evaluate((p) => window.LeylineNav.show(p, true), before);
  await page.click('aside .side-foot .nav[data-page="help"]');
  await page.click('#bug-open');
  await page.waitForSelector('#bug-dlg[open]');
}

test('signed out: Log in, or the website\'s form; no form and nothing sent', async () => {
  const { page, errors } = await open();
  await page.click('aside .side-foot .nav[data-page="help"]');
  assert.match(await page.textContent('#page-help'), /Found a bug\?.*Report a bug ›/s);
  await page.click('#bug-open');
  await page.waitForSelector('#bug-dlg[open]');
  assert.ok(await page.isVisible('#bug-out'));
  assert.ok(!(await page.isVisible('#bug-form')));
  assert.equal((await page.textContent('#bug-out p')).trim(), "Log in to report a bug, or use the website's form.");
  await page.click('#bug-out button[data-link="bugs"]');
  assert.deepEqual((await calls(page, 'open_link')).pop(), { name: 'bugs' });
  await page.click('#bug-login');
  assert.ok(!(await page.isVisible('#bug-dlg')));
  assert.deepEqual(await page.evaluate(() => window.LeylineNav.current().page), 'account');
  assert.deepEqual(await calls(page, 'bug_report'), []);
  assert.deepEqual(errors, []);
  await page.close();
});

test('signed in: where is filled in, too short sends nothing, a report goes with only its fields and the log off', async () => {
  const { page, errors } = await open({ signed: true });
  await fromHelp(page, 'host');
  assert.ok(await page.isVisible('#bug-form'));
  assert.equal(await page.inputValue('#bug-where'), 'host');
  const realm = (await page.textContent('#pb-realm')).trim();
  assert.equal(await page.inputValue('#bug-page'), `Launcher › Host · realm: ${realm}`);
  assert.equal(await page.isChecked('#bug-log'), false, 'the log is off by default');
  assert.equal(await page.getAttribute('#bug-text', 'maxlength'), '1500');
  assert.equal(await page.getAttribute('#bug-page', 'maxlength'), '120');
  await page.fill('#bug-text', 'broke');
  assert.match(await page.textContent('#bug-count'), /At least 10 characters \(5 so far\)/);
  await page.click('#bug-send');
  assert.match(await page.textContent('#bug-status'), /at least 10 characters/);
  assert.deepEqual(await calls(page, 'bug_report'), [], 'nothing sent');
  await page.fill('#bug-text', '  The Start button on Host does nothing  ');
  await page.click('#bug-send');
  await page.waitForSelector('#bug-done:not([hidden])');
  assert.deepEqual((await calls(page, 'bug_report')).pop(), { args: { text: 'The Start button on Host does nothing', where: 'host',
    page: `Launcher › Host · realm: ${realm}` } });
  assert.match(await page.textContent('#bug-done-text'), /Thanks, we have it \(report a1b2c3d4e5f6\)/);
  await page.click('#bug-done button[data-bug-close]');
  assert.ok(!(await page.isVisible('#bug-dlg')));
  // another page: its kind and name
  await fromHelp(page, 'den');
  assert.equal(await page.inputValue('#bug-where'), 'den');
  assert.match(await page.inputValue('#bug-page'), /^Launcher › Your Den/);
  assert.equal(await page.inputValue('#bug-text'), '', 'a fresh form after a sent report');
  assert.deepEqual(errors, []);
  await page.close();
});

test('the log tail goes only when ticked: the window\'s recent warnings and errors, the newest 40', async () => {
  const { page, errors } = await open({ signed: true });
  await page.evaluate(() => { for (let i = 0; i < 45; i++) console.warn('line', i); console.error('[leyline]', 'read /home/orion/x failed'); });
  await fromHelp(page, 'realms');
  assert.equal(await page.inputValue('#bug-where'), 'launcher');
  await page.fill('#bug-text', 'Realms list shows nothing at all');
  await page.check('#bug-log');
  await page.click('#bug-send');
  await page.waitForSelector('#bug-done:not([hidden])');
  const a = (await calls(page, 'bug_report')).pop().args;
  assert.equal(a.log.length, 40);
  assert.match(a.log[39], /^\d\d:\d\d:\d\d error \[leyline\] read \/home\/orion\/x failed$/, 'the core cleans it (bugs.rs)');
  assert.ok(!a.log.some((l) => / line 0$/.test(l)), 'the oldest dropped');
  // ticked again by hand each time
  await page.click('#bug-done button[data-bug-close]');
  await fromHelp(page);
  assert.equal(await page.isChecked('#bug-log'), false);
  assert.deepEqual(errors, []);
  await page.close();
});

test('a rate limit, a lost sign-in and an unreachable server, in plain words', async () => {
  const { page, errors } = await open({ signed: true });
  await fromHelp(page);
  await page.fill('#bug-text', 'The News page never loads');
  await page.evaluate(() => { window.__fail = 'rate_limited'; });
  await page.click('#bug-send');
  await page.waitForFunction(() => /a lot of reports/.test(document.getElementById('bug-status').textContent));
  assert.match(await page.getAttribute('#bug-status', 'class'), /warn/);
  assert.ok(await page.isVisible('#bug-form'), 'the words stay to send later');
  assert.equal(await page.inputValue('#bug-text'), 'The News page never loads');
  assert.ok(await page.isEnabled('#bug-send'));
  await page.evaluate(() => { window.__fail = 'error sending request for url (https://x/): tcp connect error'; });
  await page.click('#bug-send');
  await page.waitForFunction(() => /didn't go/.test(document.getElementById('bug-status').textContent));
  assert.doesNotMatch(await page.textContent('#bug-status'), /tcp|https/);
  await page.evaluate(() => { window.__fail = "Can't reach the Leyline server. Check your connection."; });
  await page.click('#bug-send');
  await page.waitForFunction(() => /Can't reach/.test(document.getElementById('bug-status').textContent));
  await page.evaluate(() => { window.__fail = 'signed_out'; window.__signed = false; });
  await page.click('#bug-send');
  await page.waitForSelector('#bug-out:not([hidden])');
  assert.ok(!(await page.isVisible('#bug-form')));
  assert.deepEqual(errors, []);
  await page.close();
});
