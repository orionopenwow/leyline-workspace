// Headless Chromium: the relay's join page tells the friend what's happening (owner 5 Nov): Reaching the host's
// realm, The host's realm answers, Logging in, and the one fix when it's stuck. The relay's API is a stand-in (routes
// on the relay's own address), and so is the WebSocket: the test decides whether the host's realm answers the probe
// or the relay closes with 4502.
//
//   node --test hosting/tests/join-steps.page.test.cjs          (needs Playwright)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const PAGE = fs.readFileSync(path.join(__dirname, '..', 'relay', 'wurelay', 'www', 'join.html'), 'utf8');
const RID = 'kzdvvj2umnduyauf';
const RELAY = 'https://relay.leylinelauncher.com';
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;

// o.answer: 'yes' | 4502; o.online; o.knock: 200 | 403
const FAKE_WS = (answer) => `
window.__sent = [];
class FakeWS extends EventTarget {
  constructor(url) { super(); this.url = url; window.__wsUrl = url; setTimeout(() => this.onopen && this.onopen(), 10); }
  send(b) { window.__sent.push(Array.from(b)); setTimeout(() => {
    if (${JSON.stringify(answer)} === 'yes') this.onmessage && this.onmessage({ data: new Uint8Array([0, 0, 4]).buffer });
    else this.onclose && this.onclose({ code: ${JSON.stringify(answer)}, reason: "the host's realm isn't answering" }); }, 10); }
  close() {}
}
window.WebSocket = FakeWS;`;

async function open(browser, o, hash) {
  const p = await browser.newPage();
  const errors = [];
  p.on('pageerror', (e) => errors.push(String(e)));
  await p.addInitScript(FAKE_WS(o.answer));
  let realmAsks = 0;
  await p.route(`${RELAY}/api/realms/${RID}`, (r) => { realmAsks++; r.fulfill({ contentType: 'application/json', body: JSON.stringify({ realm_id: RID, online: o.online !== false && !(o.onlineAfter && realmAsks < 2), name: 'Den Lite', visibility: 'private', signup: 'off' }) }); });
  await p.route(`${RELAY}/api/realms/${RID}/knock`, (r) => r.fulfill({ status: o.knock || 200, contentType: 'application/json',
    body: JSON.stringify(o.knock === 403 ? { error: 'this invite link is not valid (the host may have made a new one)' }
      : { ok: true, name: 'Den Lite', visibility: 'private', realmlist: 'relay.leylinelauncher.com:40006', realmlist_port: 'relay.leylinelauncher.com:40006', signup: 'off', valid_hours: 24 }) }));
  await p.route(`${RELAY}/r/${RID}/status.json`, (r) => r.fulfill({ contentType: 'application/json', body: '{"online":true,"build":"3.3.5a"}' }));
  await p.route(`${RELAY}/join/${RID}`, (r) => r.fulfill({ contentType: 'text/html', body: PAGE }));
  await p.goto(`${RELAY}/join/${RID}${hash || ''}`);
  p.errors = errors;
  return p;
}
const cls = (p, id) => p.$eval('#st-' + id, (e) => e.className);
const text = (p, sel) => p.$eval(sel, (e) => e.textContent);

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { if (browser) await browser.close(); });

test('a host that answers: every step green, and the Den login from the link', async () => {
  const p = await open(browser, { answer: 'yes' }, '#ABCDEFGHJKLMNPQRSTUVWXYZ~~GABCDE2345.ABCDEFGH23456789');
  await p.waitForFunction(() => document.getElementById('st-login').className === 'ok');
  assert.equal(await cls(p, 'reach'), 'ok');
  assert.equal(await cls(p, 'answer'), 'ok');
  assert.match(await text(p, '#st-answer'), /Yes \(\d+ ms\)/);
  assert.equal(await p.$eval('#den-login', (e) => e.hidden), false);
  assert.match(await text(p, '#den-login-text'), /account GABCDE2345 +password ABCDEFGH23456789/);
  assert.equal(await p.evaluate(() => window.__wsUrl), `wss://relay.leylinelauncher.com/r/${RID}/ws/3724`, 'no invite in the probe: the knock let this PC in');
  const sent = await p.evaluate(() => window.__sent[0]);
  assert.deepEqual(sent.slice(-12), Array.from(Buffer.from('LEYLINECHECK')));
  assert.equal(await p.$eval('#retry', (e) => e.hidden), true);
  assert.doesNotMatch(await p.evaluate(() => document.body.innerText), BRANDS);
  assert.deepEqual(p.errors, []);
  await p.close();
});

test("a host whose realm isn't answering (4502): the answer step says so, with what to tell the host and Try again", async () => {
  const p = await open(browser, { answer: 4502 }, '#ABCDEFGHJKLMNPQRSTUVWXYZ~~GABCDE2345.ABCDEFGH23456789');
  await p.waitForFunction(() => document.getElementById('st-answer').className === 'bad');
  assert.equal(await cls(p, 'reach'), 'ok');
  assert.equal(await cls(p, 'login'), 'skip');
  assert.match(await text(p, '#fix'), /open Leyline and press Invite a friend/);
  assert.equal(await p.$eval('#retry', (e) => e.hidden), false);
  assert.deepEqual(p.errors, []);
  await p.close();
});

test('an old link: the reach step says it no longer works and to ask for a new one', async () => {
  const p = await open(browser, { answer: 'yes', knock: 403 }, '#OLDSECRETOLDSECRETOLDSEC');
  await p.waitForFunction(() => document.getElementById('st-reach').className === 'bad');
  assert.match(await text(p, '#st-reach'), /doesn't work any more/);
  assert.match(await text(p, '#fix'), /new link/);
  assert.equal(await cls(p, 'answer'), 'skip');
  await p.close();
});

test('a host that is offline: it says to open Leyline, then Try again finds it online', async () => {
  const p = await open(browser, { answer: 'yes', onlineAfter: true }, '#ABCDEFGHJKLMNPQRSTUVWXYZ');
  await p.waitForFunction(() => document.getElementById('st-reach').className === 'bad');
  assert.match(await text(p, '#fix'), /open Leyline/);
  await p.click('#retry');
  await p.waitForFunction(() => document.getElementById('st-answer').className === 'ok');
  assert.match(await text(p, '#st-login'), /host makes accounts|step 2/);
  assert.deepEqual(p.errors, []);
  await p.close();
});
