// The release audit's phone and screenshot sweep (tools/audit/mobile.py runs it; docs/AUDIT.md "Mobile").
//
// Reads one JSON job on stdin (never on the command line: it can hold the audit's test Hero Key):
//   { base, signin, fake, shots, previous, pages: [{key, title, path}], key, name, threshold }
// and, in headless Chromium:
//   1. every page on three phones (iPhone 13, Pixel 7, a small 360x640): the answer, no sideways scrolling, primary
//      buttons at least 40 px, no console or page errors, no text cut off, the words a player reads; a screenshot to
//      <shots>/mobile/<page>-<device>.png;
//   2. on the iPhone 13, the Hero Key journey: paste the key on /wrath/key, the signed-in card, the profile, Submit a
//      link while logged in, Log out (shots: login-signed-in, profile, submit-signed-in, logged-out);
//   3. every page on a desktop (1440x900): a screenshot to <shots>/desktop/<page>.png;
//   4. each new page screenshot against the one in <previous> (when there is one): the share of pixels that changed
//      (the Log in journey's are not compared: they show a new made-up name each run).
// Prints progress on stderr and one line "SWEEP_JSON {...}" on stdout. The key is never printed, typed into a
// screenshot or written anywhere: the form is filled and sent before any picture of it. Standard Playwright only.
//
//   NODE_PATH=$(npm root -g) node tools/audit/mobile_sweep.cjs < job.json
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { chromium, devices } = require('playwright');

const PHONE = (name, d) => Object.assign({}, d, { deviceScaleFactor: Math.min(2, d.deviceScaleFactor || 2), defaultBrowserType: undefined, name });
const PHONES = [
  PHONE('iphone13', devices['iPhone 13']),
  PHONE('pixel7', devices['Pixel 7']),
  PHONE('small360', { viewport: { width: 360, height: 640 }, screen: { width: 360, height: 640 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true,
    userAgent: devices['Pixel 7'].userAgent }),
];
const DESKTOP = { name: 'desktop', viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 };
const PRIMARY = 'a.primary, button.primary, .btn.primary, .hk-btn.primary, .ly-btn:not(.small):not(.sm), .ly-cta, .hb-cta:not(.sm), .dl-big, '
  + 'button[type="submit"]:not(.small):not(.sm)';
const log = (s) => process.stderr.write(s + '\n');

function readJob() {
  return JSON.parse(fs.readFileSync(0, 'utf8'));
}

// What one page shows: everything the checks need, measured in the page.
async function measure(page, primary) {
  return page.evaluate((sel) => {
    const se = document.scrollingElement || document.documentElement;
    const vis = (e) => { const r = e.getBoundingClientRect(); const cs = getComputedStyle(e); return r.width > 0 && r.height > 0 && cs.visibility !== 'hidden' && cs.display !== 'none'; };
    const small = [];
    for (const e of document.querySelectorAll(sel)) {
      if (!vis(e) || e.closest('[hidden], details:not([open]) > :not(summary)')) continue;
      const r = e.getBoundingClientRect();
      if (r.height < 40 || r.width < 40) small.push(`${(e.innerText || e.value || e.getAttribute('aria-label') || e.className).trim().slice(0, 30)} ${Math.round(r.width)}x${Math.round(r.height)}`);
    }
    const cut = [];
    for (const e of document.body ? document.body.querySelectorAll('*') : []) {
      if (!e.childNodes.length || ![...e.childNodes].some((c) => c.nodeType === 3 && c.textContent.trim())) continue;
      if (!vis(e) || e.closest('[aria-hidden="true"], svg, select, option')) continue;
      if ([...e.classList].some((c) => /(^|[-_])sr($|[-_])|sr-only|visually-hidden/.test(c)) || e.clientWidth <= 2 || e.clientHeight <= 2) continue;   // screen-reader text is tiny on purpose
      const cs = getComputedStyle(e);
      if (cs.textOverflow === 'ellipsis') continue;   // an ellipsis is meant
      const hid = (v) => v === 'hidden' || v === 'clip';
      if ((hid(cs.overflowX) && e.scrollWidth > e.clientWidth + 2) || (hid(cs.overflowY) && cs.webkitLineClamp === 'none' && e.scrollHeight > e.clientHeight + 4 && e.clientHeight > 0)) {
        cut.push(((e.id ? '#' + e.id : (e.className && typeof e.className === 'string' ? '.' + e.className.split(' ')[0] : e.tagName)) + ': ' + e.textContent.trim().replace(/\s+/g, ' ').slice(0, 40)));
      }
    }
    // text that reaches past the right edge of the screen (cut off by the screen itself)
    const past = [];
    const W = window.innerWidth;
    for (const e of document.querySelectorAll('h1, h2, h3, p, li, a, button, label, td, th, span, b')) {
      if (!vis(e) || !(e.innerText || '').trim()) continue;
      const r = e.getBoundingClientRect();
      if (r.right > W + 1 && r.left < W && !e.closest('[style*="overflow"], pre, code, table, .scroll, .hb-rail, [class*="rail"], [class*="scroll"], [class*="track"]')) {
        let p = e.parentElement, scrolls = false;
        while (p && p !== document.body) { const o = getComputedStyle(p).overflowX; if (o === 'auto' || o === 'scroll') { scrolls = true; break; } p = p.parentElement; }
        if (!scrolls) past.push(e.innerText.trim().replace(/\s+/g, ' ').slice(0, 40));
      }
    }
    return { scrollWidth: se.scrollWidth, innerWidth: window.innerWidth, small: small.slice(0, 8), cut: [...new Set(cut)].slice(0, 8),
      past: [...new Set(past)].slice(0, 8), text: (document.title || '') + '\n' + (document.body ? document.body.innerText : '').slice(0, 200000),
      attrs: [...document.querySelectorAll('[alt],[title],[aria-label],[placeholder]')].map((e) => ['alt', 'title', 'aria-label', 'placeholder'].map((a) => e.getAttribute(a) || '').join(' ')).join(' ').slice(0, 50000) };
  }, primary);
}

function watch(page, origin) {
  const errors = [];
  page.on('pageerror', (e) => errors.push('page error: ' + String(e.message || e).slice(0, 160)));
  page.on('console', (m) => {
    if (m.type() !== 'error') return;
    const t = m.text();
    if (/^Failed to load resource/.test(t)) return;   // answers are checked below, by address
    errors.push('console: ' + t.slice(0, 160));
  });
  page.on('response', (r) => {
    const u = r.url();
    if (r.status() >= 400 && u.startsWith(origin) && r.request().resourceType() !== 'document') errors.push(`${r.status()} ${u.slice(origin.length, origin.length + 100)}`);
  });
  return errors;
}

async function newContext(browser, job, dev) {
  const o = Object.assign({}, dev);
  delete o.name; delete o.defaultBrowserType;
  const ctx = await browser.newContext(Object.assign(o, { colorScheme: 'dark', ignoreHTTPSErrors: false }));
  const origin = new URL(job.base).origin;
  if (job.fake) {
    // the fakes: only the site itself answers; the website's own /logout is the fake sign-in service's
    await ctx.route('**/*', async (route) => {
      const u = new URL(route.request().url());
      if (u.origin === origin && u.pathname === '/logout' && job.signin) {
        const r = await route.fetch({ url: job.signin.replace(/\/$/, '') + '/logout', maxRedirects: 0,
          headers: Object.assign({}, route.request().headers(), { 'sec-fetch-site': 'same-origin' }) });
        return route.fulfill({ response: r });
      }
      if (u.origin === origin && route.request().method() === 'POST') {
        // the fake site runs on http://127.0.0.1, not on one of the site's own https names: a form's Origin is the
        // name the fake site believes it has (its PUBLIC_HOST), so its same-site check sees what the live one does
        // (a browser won't let a navigation's Origin be changed, so the post is sent from here and its answer handed back)
        const r = await route.fetch({ headers: Object.assign({}, route.request().headers(), { origin: job.origin_as || origin, 'sec-fetch-site': 'same-origin' }), maxRedirects: 0 });
        return route.fulfill({ response: r });
      }
      if (u.origin === origin || u.protocol === 'data:' || u.protocol === 'blob:') return route.continue();
      return route.abort();
    });
  }
  return { ctx, origin };
}

async function visit(page, url) {
  let resp = null;
  try {
    resp = await page.goto(url, { waitUntil: 'load', timeout: 45000 });
  } catch (e) {
    return { status: 0, error: String(e.message || e).split('\n')[0].slice(0, 160) };
  }
  await page.waitForTimeout(700);
  return { status: resp ? resp.status() : 0, url: page.url() };
}

async function shot(page, file) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  await page.screenshot({ path: file, fullPage: false });
  return file;
}

async function sweepPage(page, errors, job, p, dev, dir) {
  errors.length = 0;
  const t0 = Date.now();
  const v = await visit(page, job.base + p.path);
  const out = { page: p.key, device: dev.name, status: v.status, error: v.error || '', final: v.url ? v.url.replace(job.base, '') : '' };
  if (!v.status) return out;
  Object.assign(out, await measure(page, PRIMARY));
  out.errors = errors.slice(0, 8);
  if (job.key && (out.text.includes(job.key) || out.attrs.includes(job.key))) { out.leak = true; out.text = ''; return out; }
  out.shot = path.relative(job.shots, await shot(page, path.join(dir, dev.name === 'desktop' ? `${p.key}.png` : `${p.key}-${dev.name}.png`)));
  out.secs = (Date.now() - t0) / 1000;
  return out;
}

// The Hero Key on a phone: paste it, signed in, the profile, Submit a link logged in, Log out.
async function journey(page, errors, job, dev, dir) {
  const steps = [];
  const step = async (key, fn) => {
    errors.length = 0;
    const r = { step: key, ok: false, detail: '' };
    try {
      Object.assign(r, await fn());
    } catch (e) {
      r.detail = String(e.message || e).split('\n')[0].slice(0, 200).split(job.key).join('[hidden]');
    }
    if (r.ok && r.shoot !== false) {
      const m = await measure(page, PRIMARY);
      if (m.text.includes(job.key) || m.attrs.includes(job.key)) { r.ok = false; r.detail = 'the key shows on the page'; }
      else {
        Object.assign(r, { scrollWidth: m.scrollWidth, innerWidth: m.innerWidth, small: m.small, cut: m.cut, past: m.past, text: m.text, attrs: m.attrs });
        r.shot = path.relative(job.shots, await shot(page, path.join(dir, `${key}-${dev.name}.png`)));
      }
    }
    r.errors = errors.slice(0, 8);
    delete r.shoot;
    steps.push(r);
    return r.ok;
  };
  const ok = await step('login-signed-in', async () => {
    const v = await visit(page, job.base + '/wrath/key');
    if (v.status !== 200) return { detail: `/wrath/key: HTTP ${v.status || v.error}` };
    if (!(await page.isVisible('#hk-paste'))) return { detail: 'no key paste box on the Log in page' };
    await page.fill('#hk-paste', job.key);
    await Promise.all([page.waitForNavigation({ waitUntil: 'load', timeout: 30000 }), page.click('#hk-login button[type="submit"]')]);
    await page.waitForTimeout(500);
    const signed = await page.isVisible('#hk');
    const msg = signed ? '' : ((await page.textContent('.hk-msg').catch(() => '')) || '').trim().slice(0, 120);
    return { ok: signed, detail: signed ? 'pasted, Log in, the signed-in card' : `no signed-in card after Log in${msg ? ': ' + msg : ''}` };
  });
  if (!ok) return steps;
  await step('profile', async () => {
    const v = await visit(page, job.base + '/wrath/profile');
    const t = await page.evaluate(() => document.body.innerText);
    const named = job.name && t.includes(job.name);
    const out = await page.isVisible('form[action="/logout"] button');
    return { ok: v.status === 200 && named && out, detail: v.status !== 200 ? `HTTP ${v.status || v.error}` : `${named ? 'the name shows' : 'the name is not on it'}, ${out ? 'a Log out button' : 'no Log out button'}` };
  });
  await step('submit-signed-in', async () => {
    const v = await visit(page, job.base + '/wrath/submit');
    const form = await page.isVisible('form input[name="url"], form textarea, form input[type="url"]');
    const asks = /Log in first/.test(await page.evaluate(() => document.body.innerText));
    return { ok: v.status === 200 && form && !asks, detail: v.status !== 200 ? `HTTP ${v.status || v.error}` : (form && !asks ? 'the form shows, logged in' : 'it still asks to log in') };
  });
  await step('logged-out', async () => {
    const v = await visit(page, job.base + '/wrath/profile');
    if (v.status !== 200) return { detail: `/wrath/profile: HTTP ${v.status || v.error}` };
    const btn = page.locator('form[action="/logout"] button').first();
    await btn.scrollIntoViewIfNeeded();
    await Promise.all([page.waitForNavigation({ waitUntil: 'load', timeout: 30000 }).catch(() => null), btn.click()]);
    await page.waitForTimeout(500);
    const k = await visit(page, job.base + '/wrath/key');
    const out = k.status === 200 && (await page.isVisible('#hk-out')) && !(await page.isVisible('#hk'));
    return { ok: out, detail: out ? 'Log out, then /wrath/key shows Log in again' : `after Log out: HTTP ${k.status}, ${await page.isVisible('#hk') ? 'still signed in' : 'no Log in card'}` };
  });
  return steps;
}

// The share of pixels that differ between two PNGs (decoded by the browser itself), or 1 when the sizes differ.
async function compare(page, a, b) {
  return page.evaluate(async ([x, y]) => {
    const load = async (s) => createImageBitmap(await (await fetch('data:image/png;base64,' + s)).blob());
    const [i, j] = [await load(x), await load(y)];
    if (i.width !== j.width || i.height !== j.height) return 1;
    const px = (bm) => { const c = new OffscreenCanvas(bm.width, bm.height); const g = c.getContext('2d'); g.drawImage(bm, 0, 0); return g.getImageData(0, 0, bm.width, bm.height).data; };
    const p = px(i), q = px(j);
    let n = 0;
    for (let k = 0; k < p.length; k += 4) {
      if (Math.abs(p[k] - q[k]) > 32 || Math.abs(p[k + 1] - q[k + 1]) > 32 || Math.abs(p[k + 2] - q[k + 2]) > 32) n++;
    }
    return n / (p.length / 4);
  }, [fs.readFileSync(a).toString('base64'), fs.readFileSync(b).toString('base64')]);
}

async function main() {
  const job = readJob();
  const res = { mobile: [], desktop: [], journey: [], changed: [], same: 0, fresh: [], compared: 0 };
  const browser = await chromium.launch();
  try {
    for (const dev of PHONES) {
      const { ctx, origin } = await newContext(browser, job, dev);
      const page = await ctx.newPage();
      const errors = watch(page, origin);
      for (const p of job.pages) {
        const r = await sweepPage(page, errors, job, p, dev, path.join(job.shots, 'mobile'));
        log(`  ${dev.name} ${p.key}: ${r.status || r.error}`);
        res.mobile.push(r);
      }
      if (dev.name === 'iphone13' && job.key) res.journey = await journey(page, errors, job, dev, path.join(job.shots, 'mobile'));
      await ctx.close();
    }
    if (job.desktop !== false) {
      const { ctx, origin } = await newContext(browser, job, DESKTOP);
      const page = await ctx.newPage();
      const errors = watch(page, origin);
      for (const p of job.pages) {
        const r = await sweepPage(page, errors, job, p, DESKTOP, path.join(job.shots, 'desktop'));
        log(`  desktop ${p.key}: ${r.status || r.error}`);
        delete r.text; delete r.attrs;
        res.desktop.push(r);
      }
      await ctx.close();
    }
    // what changed since the last run
    if (job.previous && fs.existsSync(job.previous)) {
      const ctx = await browser.newContext();
      const page = await ctx.newPage();
      // the Log in journey's pictures show a new made-up account each run, so they always differ: not compared
      for (const r of [...res.mobile, ...res.desktop]) {
        if (!r.shot) continue;
        const old = path.join(job.previous, r.shot);
        if (!fs.existsSync(old)) { res.fresh.push(r.shot); continue; }
        const d = await compare(page, old, path.join(job.shots, r.shot));
        res.compared += 1;
        if (d > (job.threshold || 0.005)) res.changed.push({ shot: r.shot, share: Math.round(d * 1000) / 10 });
        else res.same += 1;
      }
      await ctx.close();
    }
  } finally {
    await browser.close();
  }
  process.stdout.write('SWEEP_JSON ' + JSON.stringify(res) + '\n');
}

main().catch((e) => { process.stderr.write('mobile sweep broke: ' + String(e && e.stack || e).slice(0, 600) + '\n'); process.exit(2); });
