// The title bar's profile chip and its account panel (ui/social.js) in Chromium, with a fake Tauri core. Owner 2026-10-30
// ("remove the friends area and concept in favor of discord encouragement"): no Friends page, tab, chat or badge, Home's
// side panel hidden, a subtle Join the Discord right of the search. Owner 31 Oct 18:36 ("open a panel downward from it and
// open friends list, other stuff and sign out manage account etc"): the whole chip opens a panel joined to it: you
// (picture, name#tag, Online, Change name or picture; a guest account is told to pick its own), your friends (online
// first, a count, requests to accept or decline, Add a friend, a plain empty state; asked when the panel opens, never a
// live poll), quick links (My profile, My Den, Solo leveling, Forum, Settings, Help), then Manage account, Hero Key and
// Sign out last. Esc and a click outside close it, Tab stays inside, arrows move between items, focus goes back to the
// chip, it fits the smallest window. Signed out: Sign in opens the chooser in the same panel look, Get your Hero Key first.
// Names and titles are text, never markup; a revoked sign-in goes back to Sign in.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/social.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');

const THREADS = [
  { id: 7, cat: 'realms', title: 'Frost Haven: a cosy realm', author: 'Bramble', tag: '4655', created: 1, bumped: 2, replies: 3, pinned: false, locked: false },
  { id: 8, cat: 'dens', title: '<img src=x onerror=window.__xss=1>', author: 'Kestrel', tag: '1', created: 1, bumped: 2, replies: 1, pinned: false, locked: false },
  { id: 9, cat: 'help', title: 'Game folder not found', author: 'Alice', tag: '4821', created: 1, bumped: 2, replies: 0, pinned: false, locked: false },
];

// The fake core: get_state for app.js, the social_* and forum_* commands. window.__calls records them.
const FAKE = (opts) => {
  const calls = (window.__calls = []);
  let signed = !!(opts && opts.signed), polls = 0, revoked = false;
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [], selected_client: null, selected_realm: 'play.openvanilla.app',
        wine: { enabled: true, command: 'wine', prefix: '' } }, official_realm: 'play.openvanilla.app',
        browser_url: 'https://play.openvanilla.app/wrath', windows: false, data_dir: '/tmp' };
      case 'social_status': return signed;
      case 'social_ways': return { herokey: true, google: true, discord: false };
      case 'social_signin_start': return { state: 'pending', user_code: 'BCDF-GHJK', verify_url: 'https://play.openvanilla.app/wrath/link?code=BCDF-GHJK' };
      case 'social_signin_poll': polls++; signed = true; return { state: 'signed_in', user_code: 'BCDF-GHJK', verify_url: '' };
      case 'social_signout': signed = false; return null;
      case 'social_call':
        if (revoked) throw 'signed_out';
        if (args.action === 'me') return { me: { pid: 'aaaaaaaaaa', name: opts && opts.evil ? '<b>Alice</b>' : (opts && opts.guest ? 'GuestABCD2345' : 'Alice'), tag: '4821', main: '', offline: false,
          profile: { bio: '', avatar: '', setup: true, avatar_url: '', name_generated: !!(opts && opts.guest) } } };
        if (args.action === 'state') return { v: 1, me: { pid: 'aaaaaaaaaa', name: 'Alice', tag: '4821', status: 'launcher', offline: false },
          friends: window.__friends.slice(), incoming: window.__incoming.slice(), outgoing: window.__outgoing.slice(), blocked: [] };
        if (args.action === 'request') { if (args.args.to === 'Nobody#0000') throw 'No one has that name and tag.'; window.__outgoing.push({ pid: 'zzzzzzzzzz', name: args.args.to, at: 1 }); return { ok: true }; }
        if (args.action === 'accept' || args.action === 'decline') {
          const r = window.__incoming.find((x) => x.pid === args.args.pid);
          window.__incoming = window.__incoming.filter((x) => x.pid !== args.args.pid);
          if (r && args.action === 'accept') window.__friends.push({ pid: r.pid, name: r.name, tag: r.tag, main: '', status: 'launcher', realm: '', char: '', unread: 0 });
          return { ok: true };
        }
        if (args.action === 'herokey_status') return { web_only: false, has_key: true, key: '', saved: false, made: '31 Oct 2026', last_used: '', has_password: false };
        throw 'unexpected social call ' + args.action;
      case 'social_new_account': signed = true; return { ok: true, key: 'ovk1.AbCdEf1234.' + 'k'.repeat(43), me: { pid: 'aaaaaaaaaa', name: 'Brightfeather42', tag: '1234' } };
      case 'forum_call':
        if (args.action === 'latest') return { threads: window.__threads.slice(0, args.args.n || 8) };
        if (args.action === 'categories') return { format: 1, categories: [{ id: 'realms', name: 'Share your realm', line: 'x', staff_only: false, threads: 1, posts: 1, last: null }] };
        if (args.action === 'thread') return { thread: Object.assign({}, window.__threads[0], { id: args.args.id }), posts: [], page: 1, pages: 1, more: false };
        throw 'unexpected forum call';
      case 'open_link': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
  window.__revoke = () => { revoked = true; };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

const FRIENDS = [
  { pid: 'dddddddddd', name: 'Moss', tag: '3301', main: '', status: 'offline', realm: '', char: '', unread: 0 },
  { pid: 'bbbbbbbbbb', name: 'Bramble', tag: '4655', main: '', status: 'game', realm: 'Frost Haven', char: 'Thistle', unread: 0 },
  { pid: 'eeeeeeeeee', name: '<img src=x onerror=window.__xss=1>', tag: '7712', main: '', status: 'offline', realm: '', char: '', unread: 0 },
  { pid: 'cccccccccc', name: 'Kestrel', tag: '1290', main: '', status: 'launcher', realm: '', char: '', unread: 0 },
];

async function open(opts, size) {
  const page = await browser.newPage({ viewport: size || { width: 1400, height: 860 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript((t) => { window.__threads = t; }, THREADS);
  await page.addInitScript((f) => { window.__friends = f.friends; window.__incoming = f.incoming; window.__outgoing = []; },
    opts && opts.nofriends ? { friends: [], incoming: [] } : { friends: FRIENDS, incoming: [{ pid: 'gggggggggg', name: 'Wren', tag: '5521', at: 1 }] });
  await page.addInitScript(FAKE, opts || {});
  await page.goto(INDEX);
  return { page, errors };
}

test('no Friends page or tab; the friends are asked for only when the panel opens, never a live poll', async () => {
  const { page, errors } = await open({ signed: true });
  await page.waitForFunction(() => document.querySelector('#hf-me .hf-me-name') && document.querySelector('#hf-me .hf-me-name').textContent === 'Alice');
  assert.equal(await page.locator('#page-community, #pane-friends, #sx-chat, #sx-friends, #hf-list, #nav-friends-count, #tab-friends-count').count(), 0);
  assert.equal(await page.locator('.nav[data-page="community"], .nav[data-page="friends"]').count(), 0);
  const asked = (what) => page.evaluate((w) => window.__calls.filter(([c, a]) => c === 'social_call' && a.action === w).length, what);
  assert.equal(await asked('state'), 0, 'nothing asked before the panel opens');
  await page.click('#hf-me-btn');
  await page.waitForSelector('#hf-panel .ap-f');
  assert.equal(await asked('state'), 1);
  assert.equal(await asked('events'), 0, 'no live poll');
  assert.deepEqual(errors, []);
  await page.close();
});

test('signed in: the profile sits in the title bar, top right, with Manage account and Sign out', async () => {
  const { page, errors } = await open({ signed: true });
  await page.waitForFunction(() => document.querySelector('#hf-me .hf-me-name') && document.querySelector('#hf-me .hf-me-name').textContent === 'Alice');
  assert.equal(await page.textContent('#hf-me .hf-me-st'), '#4821');
  assert.equal(await page.getAttribute('#hf-me-btn', 'title'), 'Alice#4821');
  // owner 2026-10-30 22:11: the side panel is hidden; the profile is in the title bar beside the window buttons
  const r = await page.evaluate(() => {
    const me = document.getElementById('hf-me').getBoundingClientRect();
    const win = document.querySelector('.titlebar .winbtns').getBoundingClientRect();
    return { inBar: !!document.getElementById('hf-me').closest('.titlebar'), top: Math.round(me.top), gap: Math.round(win.left - me.right) };
  });
  assert.ok(r.inBar, 'in the title bar'); assert.ok(r.top < 30, 'at the top'); assert.ok(r.gap >= 0 && r.gap < 40, 'just left of the window buttons (gap ' + r.gap + ')');
  await page.click('#hf-me-btn');
  assert.deepEqual(await page.$$eval('#hf-panel [role="menuitem"] .ap-item-t b', (b) => b.map((x) => x.textContent)),
    ['My profile', 'My Den', 'Solo leveling', 'Forum', 'Settings', 'Help', 'Manage account', 'Hero Key', 'Sign out']);
  await page.click('#hf-panel [role="menuitem"]:has-text("Manage account")');
  await page.waitForSelector('#hf-panel', { state: 'detached' });
  // Manage account is a page in the launcher now (account.js), not the website
  await page.waitForSelector('#page-account.on');
  assert.ok(!(await page.evaluate(() => window.__calls.some(([c, a]) => c === 'open_link' && a.name === 'account'))));
  await page.click('#hf-me-btn');
  await page.click('#hf-panel [role="menuitem"]:has-text("Sign out")');
  await page.waitForSelector('#hf-signin');
  assert.ok(await page.evaluate(() => window.__calls.some(([c]) => c === 'social_signout')));
  assert.match(await page.textContent('#hf-status'), /Signed out/);
  assert.deepEqual(errors, []);
  await page.close();
});

test('signed out: Sign in, pick a way, the code under the chip, then your name', async () => {
  const { page, errors } = await open();
  await page.waitForSelector('#hf-signin');
  assert.equal(await page.textContent('#hf-me .hf-me-st'), 'Signed out');
  await page.click('#hf-signin');
  await page.waitForSelector('#hf-code:not([hidden]) .sf-way');
  await page.click('#hf-code .sf-way[data-via="google"]');
  await page.waitForSelector('#hf-code .sx-code');
  assert.match(await page.textContent('#hf-code'), /BCDF-GHJK/);
  await page.waitForFunction(() => document.querySelector('#hf-me .hf-me-name') && document.querySelector('#hf-me .hf-me-name').textContent === 'Alice', null, { timeout: 9000 });
  assert.ok(await page.isHidden('#hf-code'));
  assert.deepEqual(errors, []);
  await page.close();
});

test('the side panel is hidden: Home uses the full width; Join the Discord is in the side menu under News', async () => {
  const { page, errors } = await open({ signed: true, evil: true });
  await page.waitForFunction(() => document.querySelector('#hf-me .hf-me-name'));
  assert.equal(await page.textContent('#hf-me .hf-me-name'), '<b>Alice</b>', 'a name is text');
  assert.equal(await page.evaluate(() => window.__xss), undefined);
  assert.ok(await page.isHidden('#home-side'), 'the panel is hidden');
  const g = await page.evaluate(() => {
    const d = document.getElementById('tb-discord');
    const m = document.querySelector('main').getBoundingClientRect();
    return { inSide: !!d.closest('.side nav'), afterNews: d.previousElementSibling && d.previousElementSibling.dataset.page === 'news',
      inTitle: !!document.querySelector('.titlebar #tb-discord'), mainRight: Math.round(innerWidth - m.right) };
  });
  // owner 31 Oct 21:59: Join the Discord moved to the side menu, under News; the title bar keeps Log In (the PLAY stone)
  assert.ok(g.inSide && g.afterNews && !g.inTitle, 'in the side menu, under News');
  assert.ok(g.mainRight >= 0 && g.mainRight <= 10, 'Home reaches the window edge again (inside the metal edge)');
  assert.match(await page.textContent('#tb-discord'), /Join the Discord/);
  await page.click('#tb-discord');
  assert.ok(await page.evaluate(() => window.__calls.some(([c, a]) => c === 'open_link' && a.name === 'discord')), 'opens the Discord through openLink');
  assert.deepEqual(errors, []);
  await page.close();
});

test('a revoked sign-in goes back to Sign in', async () => {
  const { page, errors } = await open({ signed: true });
  await page.waitForFunction(() => document.querySelector('#hf-me-btn'));
  await page.evaluate(() => { window.__revoke(); return window.LeylineSocial.refresh(); });
  await page.waitForSelector('#hf-signin');
  assert.match(await page.textContent('#hf-status'), /signed out/i);
  assert.deepEqual(errors, []);
  await page.close();
});

test('the panel: joined to the chip, opens and closes (Esc, outside, the chip), focus stays inside and goes back', async () => {
  const { page, errors } = await open({ signed: true });
  await page.waitForSelector('#hf-me-btn');
  assert.equal(await page.getAttribute('#hf-me-btn', 'aria-expanded'), 'false');
  assert.equal(await page.getAttribute('#hf-me-btn', 'aria-haspopup'), 'dialog');
  await page.focus('#hf-me-btn');
  await page.keyboard.press('Enter');
  await page.waitForSelector('#hf-panel[role="dialog"]');
  assert.equal(await page.getAttribute('#hf-me-btn', 'aria-expanded'), 'true');
  assert.equal(await page.evaluate(() => document.activeElement.textContent), 'Change name or picture', 'focus moves into the panel');
  // one piece with the chip: right edges line up, the panel starts where the chip ends, and is at least as wide
  await page.evaluate(() => Promise.all(document.getElementById('hf-panel').getAnimations().map((a) => a.finished)));
  const g = await page.evaluate(() => {
    const c = document.getElementById('hf-me').getBoundingClientRect(), p = document.getElementById('hf-panel').getBoundingClientRect();
    return { right: Math.abs(c.right - p.right), top: Math.round(p.top - c.bottom), wider: p.width >= c.width };
  });
  assert.ok(g.right <= 1, 'right edges line up'); assert.ok(g.top >= -1 && g.top <= 1, 'joined: ' + g.top); assert.ok(g.wider);
  assert.ok(await page.evaluate(() => document.querySelector('.tb-me').classList.contains('open')));
  // Tab wraps inside the panel
  for (let i = 0; i < 40; i++) {
    await page.keyboard.press('Tab');
    assert.ok(await page.evaluate(() => !!document.activeElement.closest('#hf-panel')), 'focus stays in the panel');
  }
  await page.keyboard.press('Shift+Tab');
  assert.ok(await page.evaluate(() => !!document.activeElement.closest('#hf-panel')));
  // arrows move between the menu items
  await page.focus('#hf-panel [role="menuitem"]:has-text("Sign out")');
  await page.keyboard.press('ArrowDown');
  assert.match(await page.evaluate(() => document.activeElement.textContent), /^My profile/);
  await page.keyboard.press('ArrowUp');
  assert.match(await page.evaluate(() => document.activeElement.textContent), /^Sign out/);
  // Esc closes, focus back on the chip
  await page.keyboard.press('Escape');
  await page.waitForSelector('#hf-panel', { state: 'detached' });
  assert.equal(await page.evaluate(() => document.activeElement.id), 'hf-me-btn');
  assert.equal(await page.getAttribute('#hf-me-btn', 'aria-expanded'), 'false');
  // a click outside closes it; the chip toggles it
  await page.click('#hf-me-btn');
  await page.waitForSelector('#hf-panel');
  await page.mouse.click(600, 500);
  await page.waitForSelector('#hf-panel', { state: 'detached' });
  await page.click('#hf-me-btn'); await page.waitForSelector('#hf-panel');
  await page.click('#hf-me-btn'); await page.waitForSelector('#hf-panel', { state: 'detached' });
  assert.deepEqual(errors, []);
  await page.close();
});

test('friends: online first with where they are, a count, requests to accept, Add a friend, names as text', async () => {
  const { page, errors } = await open({ signed: true });
  await page.waitForSelector('#hf-me-btn');
  await page.click('#hf-me-btn');
  await page.waitForSelector('#hf-panel .ap-f.st-game');
  assert.equal(await page.textContent('#ap-fcount'), '2 of 4 online');
  const rows = await page.$$eval('#ap-flist .ap-f', (l) => l.map((x) => [x.className.replace('ap-f ', ''), x.querySelector('.ap-f-t b').textContent, x.querySelector('.ap-f-t span').textContent]));
  assert.deepEqual(rows.slice(0, 3), [['ap-req', 'Wren#5521', 'Wants to be friends'], ['st-game', 'Bramble', 'Playing Thistle on Frost Haven'], ['st-launcher', 'Kestrel', 'In the launcher']]);
  assert.deepEqual(rows.slice(3).map((r) => r[0]), ['st-offline', 'st-offline'], 'offline last');
  assert.ok(rows.some((r) => r[1] === '<img src=x onerror=window.__xss=1>'), 'a name is text');
  assert.equal(await page.evaluate(() => window.__xss), undefined);
  // accept a request: they move into the list
  await page.click('#ap-flist .ap-req button:has-text("Accept")');
  await page.waitForFunction(() => !document.querySelector('#ap-flist .ap-req'));
  assert.ok(await page.evaluate(() => window.__calls.some(([c, a]) => c === 'social_call' && a.action === 'accept' && a.args.pid === 'gggggggggg')));
  assert.equal(await page.textContent('#ap-fcount'), '3 of 5 online');
  // add a friend: empty, refused, then sent
  assert.ok(await page.isHidden('#ap-addform'));
  await page.click('#ap-add');
  assert.equal(await page.getAttribute('#ap-add', 'aria-expanded'), 'true');
  assert.equal(await page.evaluate(() => document.activeElement.id), 'ap-addin');
  await page.click('#ap-addform button[type="submit"]');
  assert.match(await page.textContent('#ap-addnote'), /Type their name and tag/);
  await page.fill('#ap-addin', 'Nobody#0000'); await page.press('#ap-addin', 'Enter');
  await page.waitForFunction(() => /No one has that name/.test(document.getElementById('ap-addnote').textContent));
  await page.fill('#ap-addin', 'Rook#2468'); await page.press('#ap-addin', 'Enter');
  await page.waitForFunction(() => /Asked Rook#2468/.test(document.getElementById('ap-addnote').textContent));
  assert.match(await page.textContent('#ap-flist .ap-wait'), /Rook#2468.*waiting/);
  assert.deepEqual(errors, []);
  await page.close();
});

test('no friends yet: plain words with your own name and tag; the links go where they say', async () => {
  const { page, errors } = await open({ signed: true, nofriends: true });
  await page.waitForSelector('#hf-me-btn');
  await page.click('#hf-me-btn');
  await page.waitForSelector('#ap-flist .ap-empty:has-text("No friends yet")');
  assert.match(await page.textContent('#ap-flist'), /Alice#1234.*Alice#4821/);
  assert.equal(await page.textContent('#ap-fcount'), '');
  await page.click('#hf-panel [role="menuitem"]:has-text("My profile")');
  assert.ok(await page.evaluate(() => window.__calls.some(([c, a]) => c === 'open_link' && a.name === 'profile')));
  await page.click('#hf-me-btn');
  await page.click('#hf-panel [role="menuitem"]:has-text("Settings")');
  await page.waitForSelector('#page-settings.on');
  await page.click('#hf-me-btn');
  await page.click('#hf-panel [role="menuitem"]:has-text("My Den")');
  await page.waitForSelector('#page-host.on');
  await page.click('#hf-me-btn');
  await page.click('#hf-panel .ap-edit');
  await page.waitForSelector('#page-account.on');
  assert.deepEqual(errors, []);
  await page.close();
});

test('a guest account: told it is one, pick your own name first, save the Hero Key', async () => {
  const { page, errors } = await open({ signed: true, guest: true, nofriends: true });
  await page.waitForSelector('#hf-me-btn');
  await page.click('#hf-me-btn');
  await page.waitForSelector('#hf-panel');
  assert.equal(await page.textContent('#hf-panel .ap-guest'), 'Guest account');
  assert.equal(await page.textContent('#hf-panel .ap-edit'), 'Pick your own name and picture');
  assert.match(await page.textContent('#hf-panel [role="menuitem"]:has-text("Hero Key")'), /Save it to keep this guest account/);
  await page.click('#hf-panel [role="menuitem"]:has-text("Hero Key")');
  await page.waitForSelector('#page-account.on #ac-key .ac-key-meta');   // Manage account, at Your Hero Key (no browser)
  assert.match(await page.textContent('#ac-key'), /not saved.*Make a new one/);
  assert.ok(!(await page.evaluate(() => window.__calls.some(([c, a]) => c === 'open_link' && a.name === 'herokey'))));
  assert.deepEqual(errors, []);
  await page.close();
});

test('signed out: the chooser in the same panel look, Get your Hero Key first; Esc closes it', async () => {
  const { page, errors } = await open();
  await page.waitForSelector('#hf-signin');
  await page.click('#hf-signin');
  await page.waitForSelector('#hf-code.acct-panel:not([hidden]) .sf-new');
  assert.equal(await page.getAttribute('#hf-signin', 'aria-expanded'), 'true');
  assert.deepEqual(await page.$$eval('#hf-code .sf-way', (b) => b.map((x) => x.textContent)), ['Get your Hero Key', 'Sign in with your Hero Key', 'Continue with Google']);
  await page.keyboard.press('Escape');
  assert.ok(await page.isHidden('#hf-code'));
  assert.equal(await page.evaluate(() => document.activeElement.id), 'hf-signin');
  await page.click('#hf-signin');
  await page.click('#hf-code .sf-new');
  await page.waitForSelector('#page-account.on #ac-keytext');
  assert.ok(await page.evaluate(() => window.__calls.some(([c]) => c === 'social_new_account')), 'a new account, right here');
  assert.ok(!(await page.evaluate(() => window.__calls.some(([c]) => c === 'social_signin_start'))), 'no browser, no code');
  assert.deepEqual(errors, []);
  await page.close();
});

test('the smallest window: the panel stays on screen with Sign out in view, dark and light', async () => {
  for (const base of ['dark', 'light']) {
    const { page, errors } = await open({ signed: true }, { width: 1000, height: 660 });
    if (base === 'light') { await page.evaluate(() => { document.documentElement.dataset.base = 'light'; }); }
    await page.waitForSelector('#hf-me-btn');
    await page.click('#hf-me-btn');
    await page.waitForSelector('#hf-panel .ap-f.st-game');
    const g = await page.evaluate(() => {
      const p = document.getElementById('hf-panel').getBoundingClientRect(), o = document.querySelector('#hf-panel .ap-out').getBoundingClientRect();
      return { left: p.left, right: innerWidth - p.right, bottom: innerHeight - p.bottom, out: o.bottom <= p.bottom && o.top >= p.top };
    });
    assert.ok(g.left >= 0 && g.right >= 0 && g.bottom >= 0, `on screen (${base}): ${JSON.stringify(g)}`);
    assert.ok(g.out, 'Sign out in view');
    assert.deepEqual(errors, []);
    await page.close();
  }
});
