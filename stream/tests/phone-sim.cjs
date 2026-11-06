// A real phone in headless Chromium, for the mobile tests (docs/MOBILE-WEB-OVERHAUL.md). Not a resized desktop window:
// Playwright's iPhone 13 / Pixel 7 descriptors (mobile user agent, isMobile so the page's viewport tag is honoured,
// hasTouch, the device pixel ratio, the screen size), driven by real touch input (CDP Input.dispatchTouchEvent:
// touchStart, touchMove..., touchEnd with a finger's timing), never mouse clicks. Each tap is traced:
//   the element under the finger, every event the page saw (touchstart -> pointerdown -> touchend -> pointerup ->
//   mousedown -> mouseup -> click), which page listeners ran for them, whether one called preventDefault, the time to
//   the page's first response (a DOM change, a navigation or a full screen request), and every requestFullscreen call
//   with whether the browser counted it as a user gesture (navigator.userActivation.isActive) and from which event.
//
//   const sim = require('./phone-sim.cjs');
//   const { ctx, p } = await sim.phone(browser, 'pixel', { landscape: true, throttle: true });
//   const tr = await sim.tap(p, '#play');           // or tap(p, [x, y]), sim.swipe(p, [[x,y],...]), sim.touchscreenTap(p, x, y)
//   console.log(sim.describe(tr));
//
// Chromium headless cannot really show full screen, so Element.prototype.requestFullscreen is wrapped: the call is
// recorded (and the original still runs). sim.startPortal(port) runs the website (stream/wrath_portal.py, fake
// services) on loopback, as site-phone.browser.test.cjs does.
'use strict';
const path = require('node:path');
const { spawn } = require('node:child_process');
const { devices } = require('playwright');

const DEVICES = { iphone: 'iPhone 13', pixel: 'Pixel 7' };

/** The page side: installed before any page script (addInitScript). Everything goes to window.__sim. */
function instrument() {
  if (window.__sim) return;
  const sim = window.__sim = { events: [], fs: [], mut: [], nav: [], errors: [], listeners: [] };
  const now = () => performance.now();
  const sel = (el) => {
    if (!el || el.nodeType !== 1) return el === window ? 'window' : el === document ? 'document' : String(el && el.nodeName || '?');
    let s = el.tagName.toLowerCase();
    if (el.id) return s + '#' + el.id;
    const c = typeof el.className === 'string' ? el.className.trim().split(/\s+/).filter(Boolean).slice(0, 2) : [];
    if (c.length) s += '.' + c.join('.');
    const p = el.parentElement;
    if (p && p !== document.body && p !== document.documentElement) {
      const ps = p.id ? '#' + p.id : (typeof p.className === 'string' && p.className.trim() ? '.' + p.className.trim().split(/\s+/)[0] : p.tagName.toLowerCase());
      s = ps + ' > ' + s;
    }
    return s;
  };
  sim.sel = sel;
  const TYPES = ['touchstart', 'touchmove', 'touchend', 'touchcancel', 'pointerdown', 'pointerup', 'pointercancel',
    'mousedown', 'mouseup', 'click', 'dblclick', 'contextmenu'];
  // which listeners ran: page listeners for these types are wrapped (named by the element they're on and their name)
  const wrapped = new WeakMap();
  const add = EventTarget.prototype.addEventListener, rem = EventTarget.prototype.removeEventListener;
  EventTarget.prototype.addEventListener = function (type, fn, opts) {
    if (!TYPES.includes(type) || typeof fn !== 'function' || fn.__simOwn) return add.call(this, type, fn, opts);
    let per = wrapped.get(fn);
    if (!per) { per = new Map(); wrapped.set(fn, per); }
    const key = type + '|' + (typeof opts === 'boolean' ? opts : !!(opts && opts.capture));
    let w = per.get(key);
    if (!w) {
      const on = this, name = fn.name || 'anon';
      const passive = !!(opts && typeof opts === 'object' && opts.passive);
      w = function (e) {
        sim.listeners.push({ t: now(), type: e.type, on: sel(on), fn: name, passive });
        return fn.apply(this, arguments);
      };
      per.set(key, w);
    }
    return add.call(this, type, w, opts);
  };
  EventTarget.prototype.removeEventListener = function (type, fn, opts) {
    const per = fn && wrapped.get(fn);
    const key = type + '|' + (typeof opts === 'boolean' ? opts : !!(opts && opts.capture));
    return rem.call(this, type, (per && per.get(key)) || fn, opts);
  };
  // every event of a tap: seen first (capture, on window) and last (bubble, on window: was the default prevented?)
  const first = (e) => sim.events.push({ t: now(), type: e.type, target: sel(e.target), trusted: e.isTrusted,
    pt: e.pointerType || '', x: Math.round(e.clientX ?? e.changedTouches?.[0]?.clientX ?? -1),
    y: Math.round(e.clientY ?? e.changedTouches?.[0]?.clientY ?? -1), active: !!navigator.userActivation?.isActive, prevented: false });
  first.__simOwn = true;
  const last = (e) => { for (let i = sim.events.length - 1; i >= 0; i--) if (sim.events[i].type === e.type) { sim.events[i].prevented = e.defaultPrevented; break; } };
  last.__simOwn = true;
  for (const t of TYPES) { add.call(window, t, first, { capture: true, passive: true }); add.call(window, t, last, false); }
  // full screen requests: from which event, and was it a user gesture
  for (const [proto, name] of [[Element.prototype, 'requestFullscreen'], [Element.prototype, 'webkitRequestFullscreen']]) {
    const orig = proto[name];
    proto[name] = function () {
      const e = window.event;
      sim.fs.push({ t: now(), el: sel(this), event: e ? e.type : '', trusted: e ? e.isTrusted : null, active: !!navigator.userActivation?.isActive });
      try { const r = orig ? orig.apply(this, arguments) : undefined; return r && r.catch ? r.catch(() => {}) : Promise.resolve(); } catch (err) { return Promise.resolve(); }
    };
  }
  // the browser's own full screen support, as a phone has it (headless Chromium may report it off)
  // (sim.phone(..., { fullscreen: false }): a browser without it, as iPhone Safari)
  const fsOn = !window.__simNoFs;
  for (const k of ['fullscreenEnabled', 'webkitFullscreenEnabled']) {
    try { Object.defineProperty(Document.prototype, k, { configurable: true, get: () => fsOn }); } catch { /* fine */ }
  }
  addEventListener('error', (e) => sim.errors.push(String(e.message || e)));
  addEventListener('pagehide', () => sim.nav.push({ t: now(), to: 'pagehide' }));
  const mo = () => new MutationObserver((list) => { const t = now(); for (const m of list) sim.mut.push({ t, target: sel(m.target), kind: m.type }); if (sim.mut.length > 2000) sim.mut.splice(0, 1000); })
    .observe(document.documentElement, { subtree: true, childList: true, attributes: true, attributeFilter: ['class', 'style', 'hidden', 'open', 'aria-expanded', 'aria-selected'] });
  if (document.documentElement) mo(); else addEventListener('DOMContentLoaded', mo);
}

/** A phone: { ctx, p, cdp, errors }. name: 'iphone' | 'pixel'; landscape; throttle: true (4x CPU, Fast 3G) or { cpu, net }. */
async function phone(browser, name = 'pixel', { landscape = false, throttle = false, init = null, headers = null, fullscreen = true } = {}) {
  const d = { ...devices[DEVICES[name] + (landscape ? ' landscape' : '')] || devices[name] };
  delete d.defaultBrowserType;   // the descriptor, in Chromium: the phone's screen, touch and user agent
  const ctx = await browser.newContext({ ...d, ...(headers ? { extraHTTPHeaders: headers } : {}) });
  if (!fullscreen) await ctx.addInitScript(() => { window.__simNoFs = true; });
  await ctx.addInitScript(instrument);
  if (init) await ctx.addInitScript(init);
  const p = await ctx.newPage();
  const errors = [];
  p.on('pageerror', (e) => errors.push(String(e)));
  const navs = [];
  p.on('framenavigated', (f) => { if (f === p.mainFrame()) navs.push({ at: Date.now(), url: f.url() }); });
  p.__navs = navs;
  const cdp = await ctx.newCDPSession(p);
  p.__cdp = cdp;
  if (throttle) await setThrottle(p, throttle === true ? {} : throttle);
  return { ctx, p, cdp, errors };
}

/** Lighthouse's mobile profile: CPU 4x slower, "Fast 3G" (1.6 Mbps down, 750 kbps up, 150 ms; DevTools' preset). */
async function setThrottle(p, { cpu = 4, net = 'fast3g' } = {}) {
  const cdp = p.__cdp;
  await cdp.send('Emulation.setCPUThrottlingRate', { rate: cpu });
  const NET = { fast3g: { latency: 562.5, downloadThroughput: 1.6e6 / 8 * 0.9, uploadThroughput: 750e3 / 8 * 0.9 },
    slow4g: { latency: 150, downloadThroughput: 1.6e6 / 8 * 0.9 * 6, uploadThroughput: 750e3 / 8 * 0.9 * 3 } };
  if (net && NET[net]) { await cdp.send('Network.enable'); await cdp.send('Network.emulateNetworkConditions', { offline: false, ...NET[net] }); }
}

const centreOf = (p, s) => p.evaluate((q) => {
  const el = document.querySelector(q);
  if (!el) return null;
  el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
  const r = el.getBoundingClientRect();
  return [r.left + r.width / 2, r.top + r.height / 2];
}, s);

async function mark(p) {
  return p.evaluate(() => ({ t: performance.now(), e: __sim.events.length, l: __sim.listeners.length, f: __sim.fs.length, m: __sim.mut.length, href: location.href }));
}

async function collect(p, m0, t0node) {
  let got;
  try {
    got = await p.evaluate((m) => ({ events: __sim.events.slice(m.e), listeners: __sim.listeners.slice(m.l), fs: __sim.fs.slice(m.f),
      mut: __sim.mut.slice(m.m), href: location.href, t: m.t, active: !!navigator.userActivation?.isActive,
      fsEl: !!document.fullscreenElement }), m0);
  } catch { got = { events: [], listeners: [], fs: [], mut: [], href: p.url(), t: m0.t, navigatedAway: true }; }
  const navs = p.__navs.filter((n) => n.at >= t0node);
  const navigated = navs.length > 0 || got.href !== m0.href;
  const rel = (x) => Math.round(x - m0.t);
  const firstResponse = [...got.mut.map((m) => m.t), ...got.fs.map((f) => f.t)].sort((a, b) => a - b)[0];
  return {
    from: m0.href, to: navigated ? (navs.at(-1)?.url || got.href) : null, navigated,
    target: got.events.find((e) => e.type === 'touchstart')?.target || got.events[0]?.target || null,
    clickTarget: got.events.find((e) => e.type === 'click')?.target || null,
    sequence: got.events.map((e) => `${e.type}${e.prevented ? '(prevented)' : ''}@${rel(e.t)}`),
    listeners: [...new Set(got.listeners.map((l) => `${l.type}:${l.on}:${l.fn}`))],
    fullscreen: got.fs.map((f) => ({ ...f, t: rel(f.t) })),
    responseMs: navigated ? Date.now() - t0node : firstResponse != null ? rel(firstResponse) : null,
    clicked: got.events.some((e) => e.type === 'click'),
  };
}

/** A finger on a selector or [x, y]: touchStart, a pause (holdMs, a real tap is 50-120 ms), touchEnd; then settleMs. */
async function tap(p, at, { holdMs = 80, settleMs = 350, jitter = 0 } = {}) {
  const xy = Array.isArray(at) ? at : await centreOf(p, at);
  if (!xy) throw new Error('nothing to tap: ' + at);
  const [x, y] = xy;
  const m0 = await mark(p);
  const t0 = Date.now();
  const cdp = p.__cdp;
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y, radiusX: 6, radiusY: 6, force: 1 }] });
  if (jitter) {   // a real finger rolls a few px while down
    await p.waitForTimeout(holdMs / 2);
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: x + jitter, y: y + jitter, radiusX: 6, radiusY: 6, force: 1 }] });
    await p.waitForTimeout(holdMs / 2);
  } else await p.waitForTimeout(holdMs);
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await p.waitForTimeout(settleMs);
  try { await p.waitForLoadState('domcontentloaded', { timeout: 15000 }); } catch { /* still going */ }
  const tr = await collect(p, m0, t0);
  tr.at = [Math.round(x), Math.round(y)];
  tr.what = Array.isArray(at) ? `[${tr.at}]` : at;
  return tr;
}

/** Playwright's own touch tap (page.touchscreen.tap: a touchStart and touchEnd with no pause). */
async function touchscreenTap(p, x, y, { settleMs = 350 } = {}) {
  const m0 = await mark(p);
  const t0 = Date.now();
  await p.touchscreen.tap(x, y);
  await p.waitForTimeout(settleMs);
  return collect(p, m0, t0);
}

/** One finger through points [[x, y], ...], ms between them. */
async function swipe(p, points, ms = 16) {
  const cdp = p.__cdp;
  const pt = ([x, y]) => [{ x, y, radiusX: 6, radiusY: 6, force: 1 }];
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: pt(points[0]) });
  for (const q of points.slice(1)) { await p.waitForTimeout(ms); await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: pt(q) }); }
  await p.waitForTimeout(ms);
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
}

/** Paint and load timings of the page now open: first paint, first contentful paint, DOM ready, load, long tasks. */
async function timings(p) {
  return p.evaluate(() => {
    const n = performance.getEntriesByType('navigation')[0] || {};
    const paint = Object.fromEntries(performance.getEntriesByType('paint').map((e) => [e.name, Math.round(e.startTime)]));
    const res = performance.getEntriesByType('resource');
    return { fp: paint['first-paint'] ?? null, fcp: paint['first-contentful-paint'] ?? null, dcl: Math.round(n.domContentLoadedEventEnd || 0),
      load: Math.round(n.loadEventEnd || 0), bytes: res.reduce((s, r) => s + (r.transferSize || 0), 0) + (n.transferSize || 0),
      scripts: res.filter((r) => r.initiatorType === 'script').length, requests: res.length + 1 };
  });
}

/** One line per tap, for logs and the doc. */
function describe(tr) {
  const fs = tr.fullscreen.length ? ` fullscreen:${tr.fullscreen.map((f) => `${f.event || 'none'}${f.active ? '+gesture' : '-NO-GESTURE'}@${f.t}`).join(',')}` : '';
  return `${tr.what} -> ${tr.target}${tr.clickTarget && tr.clickTarget !== tr.target ? ` (click on ${tr.clickTarget})` : ''}` +
    ` | ${tr.navigated ? 'went to ' + tr.to : 'stayed'} | response ${tr.responseMs ?? 'none'} ms${fs}\n    ${tr.sequence.join(' ')}`;
}

// ---- the website on loopback (fake services; nothing live) ----------------------------------------------------------
const PORTAL = path.join(__dirname, '..', 'wrath_portal.py');
const BOOT = `
import importlib.util, os, sys, tempfile, threading, time, shutil
from http.server import ThreadingHTTPServer
here = os.path.dirname(os.path.abspath(sys.argv[1])); tmp = tempfile.mkdtemp(prefix="wu-phonesim-")
for d in ("data", "mods/files", "armory/c", "hub", "items"): os.makedirs(os.path.join(tmp, d), exist_ok=True)
shutil.copy(os.path.join(here, "hub", "community-index.json"), os.path.join(tmp, "hub"))
os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
    WU_STREAM="0", STREAM_HOST="wrath.example", PUBLIC_HOST="leylinelauncher.com", WU_DL_PROBE="0", WU_HUB_FRONT="1",
    WU_RELAY_LISTING="http://127.0.0.1:9/realms.json", WU_PORTAL_DATA=os.path.join(tmp, "data"),
    WU_HUB_DIR=os.path.join(tmp, "hub"), WU_MODS_DIR=os.path.join(tmp, "mods"), WU_WORKSHOP_ITEMS_DIR=os.path.join(tmp, "items"),
    WU_ARMORY_DIR=os.path.join(tmp, "armory"), WU_ART_DIR=os.path.join(here, "www-files", "art"))
for k in ("WU_HUB", "WU_BRAND_FILE", "WU_GUEST_DEMO"): os.environ.pop(k, None)
import json; os.environ.update(json.loads(sys.argv[3]))
spec = importlib.util.spec_from_file_location("wp", sys.argv[1]); wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
wp.realm_status = lambda: {"online": True, "players": 3, "bots": 10}
wp.social.setup(start_thread=False); wp.profiles.setup(); wp.herokeys.setup(); wp.forum.setup()
s = ThreadingHTTPServer(("127.0.0.1", int(sys.argv[2])), wp.PageHandler); s.daemon_threads = True
print("ready", flush=True); s.serve_forever()
`;

/** The website on 127.0.0.1:port (as site-phone.browser.test.cjs). Resolves to the child process (kill() it after). */
function startPortal(port, env = {}) {
  const server = spawn('python3', ['-I', '-c', BOOT, PORTAL, String(port), JSON.stringify(env)], { stdio: ['ignore', 'pipe', 'pipe'] });
  let err = '';
  server.stderr.on('data', (d) => { err = (err + d).slice(-4000); });
  return new Promise((ok, bad) => {
    const t = setTimeout(() => bad(new Error('the portal did not start\n' + err)), 60000);
    server.stdout.on('data', (d) => { if (String(d).includes('ready')) { clearTimeout(t); ok(server); } });
    server.on('exit', (c) => bad(new Error('the portal exited ' + c + '\n' + err)));
  });
}

module.exports = { DEVICES, phone, setThrottle, tap, touchscreenTap, swipe, timings, describe, startPortal, instrument, centreOf };
