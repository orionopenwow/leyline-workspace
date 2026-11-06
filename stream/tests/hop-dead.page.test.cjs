// Headless Chromium: a hop to a realm whose host's realm isn't answering (player report 5 Nov: "stuck in a cycle of
// Connecting and Connected"). The relay closes such login sockets with 4502; after two of them (or three silent closes
// from an older relay) wrath-early.js shows one plain note. A login socket that hears anything resets the count.
//
//   node --test tests/hop-dead.page.test.cjs          (from the stream folder; needs Playwright)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const EARLY = fs.readFileSync(path.join(__dirname, '..', 'www-files', 'wrath-early.js'), 'utf8');
const RID = 'abcdefghijklmnop';
const ENV = { hop: RID, hop_name: 'Frost Haven', hop_ws: `wss://relay.example/ws/${RID}`,
  hop_join: `https://relay.example/join/${RID}?hop=1`, host: 'relay.example', home: 'https://play.example' };

// a stand-in WebSocket: the test decides what each socket hears and how it closes
const FAKE = `
class FakeWS extends EventTarget {
  constructor(url) { super(); this.url = url; (window.__made = window.__made || []).push(this); }
  send() {}
  close() {}
  hear() { this.dispatchEvent(new MessageEvent('message', { data: new ArrayBuffer(3) })); }
  shut(code) { const e = new Event('close'); e.code = code; this.dispatchEvent(e); }
}
window.WebSocket = FakeWS;`;

async function page(browser) {
  const p = await browser.newPage();
  await p.route('https://game.test/wrath-early.js', (r) => r.fulfill({ contentType: 'text/javascript', body: EARLY }));
  await p.route('https://game.test/', (r) => r.fulfill({ contentType: 'text/html', body:
    `<!doctype html><meta charset="utf-8"><body><script>${FAKE}</script>
<script>window.__wenilla_env = ${JSON.stringify(ENV)};</script><script src="/wrath-early.js"></script>` }));
  await p.goto('https://game.test/');
  return p;
}
const open = (p, port) => p.evaluate((port) => { new WebSocket('wss://game.test/ws/' + port); return window.__made.length - 1; }, port);

test('two 4502 closes on the login socket: one plain note, with Try again', async () => {
  const browser = await chromium.launch();
  try {
    const p = await page(browser);
    let i = await open(p, 3724);
    await p.evaluate((i) => window.__made[i].shut(4502), i);
    assert.equal(await p.$('#wu-hop-dead'), null, 'not after one');
    i = await open(p, 3724);
    await p.evaluate((i) => window.__made[i].shut(4502), i);
    const text = await p.textContent('#wu-hop-dead');
    assert.match(text, /Frost Haven isn't answering/);
    assert.match(text, /first start can take several minutes/);
    assert.ok(await p.$('#wu-hop-dead button'), 'buttons');
    i = await open(p, 3724);
    await p.evaluate((i) => window.__made[i].shut(4502), i);
    assert.equal((await p.$$('#wu-hop-dead')).length, 1, 'shown once');
    assert.equal(await p.evaluate(() => window.__made[0].url), `wss://relay.example/ws/${RID}/auth`, 'the bridge');
  } finally { await browser.close(); }
});

test('a login socket that hears anything resets the count; world sockets do not count', async () => {
  const browser = await chromium.launch();
  try {
    const p = await page(browser);
    let i = await open(p, 3724);
    await p.evaluate((i) => window.__made[i].shut(1006), i);
    i = await open(p, 3724);
    await p.evaluate((i) => window.__made[i].shut(1006), i);
    i = await open(p, 3724);
    await p.evaluate((i) => { window.__made[i].hear(); window.__made[i].shut(1000); }, i);
    i = await open(p, 8085);
    await p.evaluate((i) => window.__made[i].shut(1006), i);
    i = await open(p, 3724);
    await p.evaluate((i) => window.__made[i].shut(1006), i);
    assert.equal(await p.$('#wu-hop-dead'), null, 'heard once: the count started again');
    for (let k = 0; k < 2; k++) {
      i = await open(p, 3724);
      await p.evaluate((i) => window.__made[i].shut(1006), i);
    }
    assert.ok(await p.$('#wu-hop-dead'), 'three silent login closes in a row (an older relay)');
  } finally { await browser.close(); }
});
