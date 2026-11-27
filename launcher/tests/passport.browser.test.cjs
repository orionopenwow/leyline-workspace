// The Passport (ui/passport.js; app.rs passport_*; docs/leyline/passport.md), in Chromium with a fake Tauri core:
//   * first launch: a name, Make my Passport, the 12 words, "I wrote these words down", Done: at most 3 clicks, and
//     Done waits for the box;
//   * Back up: show the phrase, guardians (add from friends, remove, Save needs 2 to 5);
//   * Devices: the list, Remove asks first; Add a device finds the other PC and sends with the typed code;
//   * Recover: the phrase; the other PC shows its code; 2 guardians' answers; helping a friend;
//   * a warning with Stop it when someone asks the guardians; errors in plain words; no game or publisher names; no
//     key anywhere on the page.
//
//   node --test launcher/tests/passport.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const UI = path.join(__dirname, '..', 'ui');
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard|3\.3\.5|1\.12/i;
const WORDS12 = 'legal winner thank year wave sausage worth useful legal winner thank yellow'.split(' ');
const NEW = { peer_id: 'mmmmmmmmmmmmmmmm', short: 'MMMM', name: '', picture: '', setup_done: false, phrase_waiting: true, phrase_words: 12,
  devices: [{ id: 'dddddddddddddddd', name: 'Desk', created: 1764200000, this: true }], guardians: [], guarding: [], alerts: [], recovering: null,
  rotated: false, herokey: null, friends: [{ peer_id: 'bbbbbbbbbbbbbbbb', name: 'Bo' }, { peer_id: 'cccccccccccccccc', name: 'Cy' }, { peer_id: 'eeeeeeeeeeeeeeee', name: 'Ed' }], near: 0 };
const READY = { ...NEW, name: 'Ari', setup_done: true, phrase_waiting: false, phrase_words: 24,
  devices: [{ id: 'dddddddddddddddd', name: 'Desk', created: 1764200000, this: true }, { id: 'llllllllllllllll', name: 'Laptop', created: 1764300000, this: false }],
  guardians: [{ peer_id: 'bbbbbbbbbbbbbbbb', name: 'Bo' }], guarding: [{ peer_id: 'cccccccccccccccc', name: 'Cy' }] };

const FAKE = (o) => {
  window.__calls = [];
  let st = JSON.parse(JSON.stringify(o.state));
  let answers = 0;
  let pair = 0;
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    window.__calls.push([cmd, args]);
    if (o.fail && o.fail[cmd]) throw o.fail[cmd];
    switch (cmd) {
      case 'passport_state': return JSON.parse(JSON.stringify(st));
      case 'passport_setup': st.name = args.name; st.picture = args.picture; return o.words;
      case 'passport_finish': st.setup_done = true; st.phrase_waiting = false; return null;
      case 'passport_phrase': return o.words;
      case 'passport_guardians': return `Your ${args.peerIds.length} guardians are set.`;
      case 'passport_device_revoke': st.devices = st.devices.filter((d) => d.id !== args.id); return 'Laptop is off your Passport.';
      case 'passport_pair_find': return 'Laptop';
      case 'passport_pair_confirm': return 'Done: Laptop is part of your Passport now.';
      case 'passport_pair_wait': return null;
      case 'passport_pair_status': pair += 1; return pair < 2 ? { state: 'waiting' } : { state: 'show', code: '482913' };
      case 'passport_pair_cancel': return null;
      case 'passport_restore': return 'Done: this PC is your Passport again (Hero MMMM). Restart Leyline to finish.';
      case 'passport_recover_start': st.recovering = { code: 'LR1-ABCDEFG', approvals: 0, needed: 2 }; return 'LR1-ABCDEFG';
      case 'passport_recover_add': answers += 1; st.recovering.approvals = answers; return answers >= 2 ? { done: true, told: 3, friends: 3 } : { done: false, approvals: answers };
      case 'passport_recover_cancel': st.recovering = null; return null;
      case 'passport_help_friend': return 'LG1-ANSWERTEXT';
      case 'passport_veto': st.alerts = []; return 'Stopped.';
      case 'passport_herokey_import': st.herokey = { id: 'ABCDEFGHIJ' }; return 'Done: your old Hero Key (ABCDEFGHIJ) is part of your Passport now.';
      case 'peer_key_export': return 'Saved.';
      default: throw 'unknown command ' + cmd;
    }
  } } };
};

async function open(o = {}) {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, { state: o.state || NEW, fail: o.fail || null, words: o.words || WORDS12 });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.route('http://leyline.test/**', (r) => r.fulfill({ contentType: 'text/html', body: '<!doctype html><html><body><div id="lp" data-leyline-passport></div></body></html>' }));
  await page.goto('http://leyline.test/');
  await page.addStyleTag({ path: path.join(UI, 'passport.css') });
  await page.addScriptTag({ path: path.join(UI, 'passport.js') });
  await page.waitForSelector(o.wait || '#lp-make');
  return { browser, page, errors };
}

const clicks = (page) => page.evaluate(() => window.__clicks || 0);
async function countClicks(page) {
  await page.evaluate(() => { window.__clicks = 0; document.addEventListener('click', (e) => { if (e.target.closest('button, input[type=checkbox], label')) window.__clicks += 1; }, true); });
}
const calls = (page, cmd) => page.evaluate((c) => window.__calls.filter((x) => x[0] === c), cmd);

test('first launch: Make your Passport in 3 clicks, Done waits for the box', async () => {
  const { browser, page, errors } = await open();
  try {
    await countClicks(page);
    assert.match(await page.textContent('#lp'), /Make your Passport/);
    await page.fill('#lp-name', 'Ari');
    await page.click('#lp-make');
    await page.waitForSelector('.lp-words li');
    const shown = await page.$$eval('.lp-words li', (l) => l.map((x) => x.textContent));
    assert.deepEqual(shown, WORDS12);
    assert.equal(await page.isDisabled('#lp-done'), true, 'Done waits for "I wrote these words down"');
    await page.click('#lp-saved');
    assert.equal(await page.isDisabled('#lp-done'), false);
    await page.click('#lp-done');
    await page.waitForSelector('#lp-go-backup');
    assert.ok((await clicks(page)) <= 3, `setup took ${await clicks(page)} clicks`);
    assert.equal((await calls(page, 'passport_setup'))[0][1].name, 'Ari');
    assert.equal((await calls(page, 'passport_finish')).length, 1);
    assert.match(await page.textContent('#lp'), /Your Passport is ready/);
    assert.doesNotMatch(await page.textContent('#lp'), /legal winner/, 'the words are gone after Done');
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});

test('back up: the phrase on its own screen, guardians 2 to 5, the old Hero Key', async () => {
  const { browser, page, errors } = await open({ state: READY, wait: '#lp-go-backup', words: [...WORDS12, ...WORDS12] });
  try {
    await page.click('#lp-go-backup');
    assert.match(await page.textContent('#lp'), /24 words/);
    await page.click('#lp-show');
    await page.waitForSelector('.lp-words li');
    assert.equal(await page.$$eval('.lp-words li', (l) => l.length), 24);
    await page.click('#lp-hide');
    // one guardian: Save waits for a second
    assert.equal(await page.isDisabled('#lp-save-guardians'), true);
    await page.selectOption('#lp-add-guardian', 'eeeeeeeeeeeeeeee');
    assert.equal(await page.isDisabled('#lp-save-guardians'), false);
    await page.click('#lp-save-guardians');
    await page.waitForFunction(() => /guardians are set/.test(document.querySelector('.lp-status').textContent));
    assert.deepEqual((await calls(page, 'passport_guardians'))[0][1].peerIds, ['bbbbbbbbbbbbbbbb', 'eeeeeeeeeeeeeeee']);
    await page.click('#lp-herokey');
    await page.waitForFunction(() => /Brought in \(ABCDEFGHIJ\)/.test(document.querySelector('#lp').textContent));
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});

test('devices: Remove asks first; add one by its code', async () => {
  const { browser, page, errors } = await open({ state: READY, wait: '#lp-go-devices' });
  try {
    await page.click('#lp-go-devices');
    assert.match(await page.textContent('#lp'), /Desk \(this PC\)/);
    await page.click('.lp-device >> text=Remove');
    assert.equal((await calls(page, 'passport_device_revoke')).length, 0, 'asks first');
    await page.click('text=Remove it');
    await page.waitForFunction(() => !/Laptop/.test(document.querySelector('.lp-list').textContent));
    await page.click('#lp-find');
    await page.waitForSelector('#lp-code');
    await page.fill('#lp-code', '482913');
    await page.click('#lp-send');
    await page.waitForFunction(() => /part of your Passport/.test(document.querySelector('.lp-status').textContent));
    assert.equal((await calls(page, 'passport_pair_confirm'))[0][1].code, '482913');
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});

test('recover: phrase, the other PC shows a code, 2 guardians, help a friend, stop a theft', async () => {
  const st = { ...READY, alerts: [{ id: 'f'.repeat(32), effective: 1764600000 }] };
  const { browser, page, errors } = await open({ state: st, wait: '.lp-alert' });
  try {
    await page.click('.lp-alert button');
    await page.waitForFunction(() => !document.querySelector('.lp-alert'));
    await page.click('#lp-go-recover');
    await page.fill('#lp-phrase', WORDS12.join(' '));
    await page.click('#lp-restore');
    await page.waitForFunction(() => /Restart Leyline/.test(document.querySelector('.lp-status').textContent));
    await page.click('#lp-wait');
    await page.waitForSelector('.lp-bigcode', { timeout: 5000 });
    assert.equal(await page.textContent('.lp-bigcode'), '482913');
    await page.click('#lp-ask');
    await page.waitForSelector('#lp-answer');
    assert.match(await page.textContent('#lp'), /LR1-ABCDEFG/);
    await page.fill('#lp-answer', 'LG1-ONE');
    await page.click('#lp-add-answer');
    await page.waitForFunction(() => /1 of 2/.test(document.querySelector('#lp').textContent));
    await page.fill('#lp-answer', 'LG1-TWO');
    await page.click('#lp-add-answer');
    await page.waitForSelector('#lp-done');
    assert.match(await page.textContent('#lp'), /You're back/);
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
  const b = await open({ state: READY, wait: '#lp-go-help' });
  try {
    await b.page.click('#lp-go-help');
    await b.page.fill('#lp-help-code', 'LR1-XYZ');
    await b.page.click('#lp-help-go');
    await b.page.waitForFunction(() => /LG1-ANSWERTEXT/.test(document.querySelector('#lp').textContent));
    assert.equal((await calls(b.page, 'passport_help_friend'))[0][1].peerId, 'cccccccccccccccc');
  } finally { await b.browser.close(); }
});

test('errors in plain words; no brand names; no keys on the page', async () => {
  const { browser, page } = await open({ fail: { passport_setup: 'This PC has no random source.' } });
  try {
    await page.click('#lp-make');
    await page.waitForFunction(() => /random source/.test(document.querySelector('.lp-status').textContent));
    const text = await page.evaluate(() => document.body.innerText + [...document.querySelectorAll('[title],[placeholder],[aria-label]')].map((e) => e.title + e.placeholder + e.getAttribute('aria-label')).join(' '));
    assert.doesNotMatch(text, BRANDS);
    assert.doesNotMatch(text, /[0-9a-f]{64}/, 'never a key');
  } finally { await browser.close(); }
  const src = require('node:fs').readFileSync(path.join(UI, 'passport.js'), 'utf8');
  assert.doesNotMatch(src.replace(/^\s*\/\/.*$/gm, ''), BRANDS);
});
