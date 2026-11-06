// The website on a real phone (phone-sim.cjs: iPhone 13 and Pixel 7 descriptors, CDP touches), owner 6 Nov: "none of the
// tabs go to the right area either, we need a full mobile browser version overhaul". What the sim found, kept fixed:
//   * Menu's sheet grew inside the sticky top bar, taller than the screen: its last rows (On your phone, News, Join the
//     Discord, Creators, Download) were out of reach, and a swipe on it scrolled the page away under it;
//   * a link to a #section put the heading under the sticky bar;
//   * the Mods tabs hid "Loadouts" past the edge; footer links were 21 px tall.
// Now: every Menu item is reachable and a tap on it opens its own page; the page behind the sheet stays put; the bottom
// tab bar (upright phones) goes where it says; #sections land below the bar; nav, tab bar and footer targets are 44 px.
//
//   node --test tests/phone-site.browser.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { chromium } = require('playwright');
const sim = require('./phone-sim.cjs');

let server, browser, base;
test.before(async () => {
  server = await sim.startPortal(0);
  base = `http://127.0.0.1:${server.port}`;
  browser = await chromium.launch();
});
test.after(async () => { if (browser) await browser.close(); if (server) server.kill(); });

let ipN = 0;
const phone = (dev, o = {}) => sim.phone(browser, dev, { headers: { 'X-Forwarded-For': `10.97.${dev === 'pixel' ? 1 : 2}.${++ipN}` }, ...o });
// where a tap went: the page itself, or (a page that needs a login, signed out) the sign-in page on its way there
const went = (tr, href) => {
  const want = new URL(href, base);
  return tr.navs.some((u) => { const g = new URL(u); return (g.origin + g.pathname) === (want.origin + want.pathname)
    || (/^\/(wrath\/signin|login)$/.test(g.pathname) && [want.pathname, '/wrath'].includes(g.searchParams.get('next'))); });
};

for (const dev of ['iphone', 'pixel']) {
  test(`${dev}: every Menu item can be reached and opens its own page; the page behind stays put`, async () => {
    const { ctx, p, errors } = await phone(dev);
    await p.goto(base + '/wrath/leyline');
    let tr = await sim.tap(p, '.hb-menu-b');
    assert.equal(tr.clickTarget !== null, true, sim.describe(tr));
    assert.equal(await p.evaluate(() => document.getElementById('hb-menu').checked), true, 'Menu opened');
    // a swipe up on the sheet scrolls the sheet, not the page
    const y0 = await p.evaluate(() => scrollY);
    await sim.swipe(p, [[200, 560], [200, 460], [200, 360], [200, 260]]);
    await p.waitForTimeout(400);
    const after = await p.evaluate(() => ({ y: scrollY, sheet: document.querySelector('.hb-nav').scrollTop }));
    assert.equal(after.y, y0, 'the page behind did not move');
    if (await p.evaluate(() => { const n = document.querySelector('.hb-nav'); return n.scrollHeight > n.clientHeight + 4; }))
      assert.ok(after.sheet > 0, 'the sheet scrolled');
    const items = await p.evaluate(() => [...document.querySelectorAll('.hb-nav a')].filter((a) => a.getBoundingClientRect().width > 0)
      .map((a, i) => ({ text: a.textContent.trim(), href: a.getAttribute('href') })));
    assert.ok(items.length >= 15, 'the whole menu: ' + items.length);
    const bad = [];
    for (const it of items) {
      await p.goto(base + '/wrath/leyline');
      await sim.tap(p, '.hb-menu-b', { settleMs: 250 });
      // scrolled into view inside the sheet as a finger would, then: is it the thing under the finger?
      const at = await p.evaluate((t) => {
        const a = [...document.querySelectorAll('.hb-nav a')].find((x) => x.textContent.trim() === t && x.getBoundingClientRect().width > 0);
        a.scrollIntoView({ block: 'center' });
        const r = a.getBoundingClientRect(); const x = r.left + r.width / 2, y = r.top + r.height / 2;
        const top = document.elementFromPoint(x, y);
        return { x, y, ok: !!top && (top === a || a.contains(top)), top: top && __sim.sel(top), h: r.height };
      }, it.text);
      if (!at.ok) { bad.push(`${it.text}: covered by ${at.top}`); continue; }
      if (at.h < 44) bad.push(`${it.text}: ${at.h} px tall`);
      if (/^https?:/.test(it.href)) continue;   // off the site (Discord): reachable is enough
      tr = await sim.tap(p, [at.x, at.y], { settleMs: 400 });
      if (!went(tr, it.href)) bad.push(`${it.text}: went to ${tr.to || 'nowhere'}, not ${it.href}`);
    }
    assert.deepEqual(bad, []);
    assert.deepEqual(errors, []);
    await ctx.close();
  });

  test(`${dev}: the bottom tab bar, upright: each tab goes where it says, Menu opens the sheet`, async () => {
    const { ctx, p } = await phone(dev);
    await p.goto(base + '/wrath/leyline/realms');
    const tabs = await p.evaluate(() => [...document.querySelectorAll('.hb-tabs a')].map((a) => ({ text: a.textContent.trim(), href: a.getAttribute('href'), on: a.classList.contains('on'), h: a.getBoundingClientRect().height })));
    assert.deepEqual(tabs.map((t) => t.text), ['Play', 'Realms', 'Bosses', 'Mods']);
    assert.ok(tabs.find((t) => t.text === 'Realms').on, 'this page\'s tab is lit');
    assert.ok(tabs.every((t) => t.h >= 44), 'big enough for a thumb');
    for (const t of tabs) {
      await p.goto(base + '/wrath/leyline/realms');
      const tr = await sim.tap(p, `.hb-tabs a[href="${t.href}"]`, { settleMs: 400 });
      assert.ok(went(tr, t.href), `${t.text}: ${sim.describe(tr)}`);
    }
    await p.goto(base + '/wrath/leyline/realms');
    // the page's own last link is not hidden under the tab bar
    const last = await p.evaluate(() => { scrollTo(0, document.documentElement.scrollHeight); const f = document.querySelector('.hb-foot-links a:last-child');
      const r = f.getBoundingClientRect(); const top = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2); return !!top && f.contains(top); });
    assert.equal(last, true, 'the footer\'s last link is above the tab bar');
    await sim.tap(p, '.hb-tab-menu');
    assert.equal(await p.evaluate(() => document.getElementById('hb-menu').checked), true);
    await ctx.close();
  });
}

test('the tab bar is for upright phones only (not sideways, not a desktop)', async () => {
  const { ctx, p } = await phone('pixel', { landscape: true });
  await p.goto(base + '/wrath/leyline');
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('.hb-tabs')).display), 'none');
  await ctx.close();
  const d = await browser.newContext({ viewport: { width: 1366, height: 800 } });
  const q = await d.newPage();
  await q.goto(base + '/wrath/leyline');
  assert.equal(await q.evaluate(() => getComputedStyle(document.querySelector('.hb-tabs')).display), 'none');
  await d.close();
});

test('a link to a #section lands below the sticky bar (hub pages and the module pages)', async () => {
  const { ctx, p } = await phone('iphone');
  const bad = [];
  for (const [path, sel] of [['/wrath/mobile', 'a[href="#android"]'], ['/wrath/mobile', 'a[href="#faq"]'], ['/wrath/modes', 'a[href="#bosses"]']]) {
    await p.goto(base + path);
    await sim.tap(p, sel, { settleMs: 1200 });
    const pos = await p.evaluate(() => { const t = document.getElementById(decodeURIComponent(location.hash.slice(1)));
      const bar = document.querySelector('.hb-top,#ly-site-nav').getBoundingClientRect(); return { top: t ? t.getBoundingClientRect().top : null, bar: bar.bottom }; });
    if (pos.top === null || pos.top < pos.bar - 1) bad.push(`${path} ${sel}: heading at ${Math.round(pos.top)}, bar ends at ${Math.round(pos.bar)}`);
  }
  assert.deepEqual(bad, []);
  await ctx.close();
});

test('the module pages\' Menu (Modes): its last item can be reached', async () => {
  const { ctx, p } = await phone('iphone');
  await p.goto(base + '/wrath/modes');
  await sim.tap(p, '.ly-menu-b');
  const y0 = await p.evaluate(() => scrollY);
  for (let i = 0; i < 4; i++) await sim.swipe(p, [[200, 560], [200, 400], [200, 240]]);
  await p.waitForTimeout(400);
  const r = await p.evaluate(() => { const a = [...document.querySelectorAll('#ly-site-nav .ly-links a')].at(-1); const b = a.getBoundingClientRect();
    const top = document.elementFromPoint(b.left + b.width / 2, b.top + b.height / 2); return { ok: !!top && a.contains(top), y: scrollY, b: b.bottom, vh: innerHeight }; });
  assert.ok(r.ok && r.b <= r.vh, 'the last item is on screen and under the finger: ' + JSON.stringify(r));
  assert.equal(r.y, y0, 'the page behind did not move');
  await ctx.close();
});

for (const width of [360, 390]) {
  test(`${width} px: nav, tab bar, tabs and footer targets are 44 px; the Mods tabs all show`, async () => {
    const { ctx, p } = await phone('pixel');
    await p.setViewportSize({ width, height: 780 });
    const bad = [];
    for (const path of ['/wrath/leyline', '/wrath/leyline/catalog', '/wrath/leyline/realms', '/wrath/mobile', '/wrath/modes', '/wrath/bossfights']) {
      await p.goto(base + path);
      const got = await p.evaluate(() => {
        const out = [];
        for (const el of document.querySelectorAll('.hb-top .hb-end a, .hb-menu-b, .hb-tabs a, .hb-tabs label, .hb-foot a, .hb-seg a, .ly-nav > a, .ly-menu-b, .ly-foot a, .ly-share summary')) {
          const r = el.getBoundingClientRect();
          if (!r.width || getComputedStyle(el).visibility === 'hidden') continue;
          if (r.height < 44) out.push(`${__sim.sel(el)} "${el.textContent.trim().slice(0, 20)}" ${Math.round(r.height)} px`);
        }
        const seg = document.querySelector('.hb-seg');
        if (seg) for (const a of seg.querySelectorAll('a')) { const r = a.getBoundingClientRect(), s = seg.getBoundingClientRect(); if (r.right > s.right + 1 || r.left < s.left - 1) out.push(`tab "${a.textContent.trim()}" is cut off`); }
        return out;
      });
      for (const g of got) bad.push(`${path}: ${g}`);
    }
    assert.deepEqual(bad, []);
    await ctx.close();
  });
}
