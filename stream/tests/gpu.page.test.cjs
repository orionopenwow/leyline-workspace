// The graphics check in the page (wrath-early.js, 5 Nov "someone said they can't see anything"): the browser
// client draws with WebGPU only, so a browser without it (or with graphics acceleration off) now gets a note
// with the fix for that browser and the desktop app, instead of a dark page. A browser with a GPU adapter gets
// nothing. ?nogpucheck=1 skips the check.
// Run: node stream/tests/gpu.page.test.cjs
'use strict';
const path = require('path');
const EARLY = path.join(__dirname, '..', 'www-files', 'wrath-early.js');

function fakeEl(tag) {
  const e = {
    tag, attrs: {}, kids: [], listeners: {}, style: {}, textContent: '', className: '',
    setAttribute(k, v) { this.attrs[k] = String(v); if (k === 'id') this.id = String(v); },
    getAttribute(k) { return this.attrs[k]; },
    addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); },
    appendChild(x) { this.kids.push(x); x.parent = this; return x; },
    append(...k) { for (const x of k) this.kids.push(x); },
    remove() { if (this.parent) this.parent.kids = this.parent.kids.filter((k) => k !== this); this.removed = true; },
    querySelector() { return null; }, querySelectorAll() { return []; },
  };
  Object.defineProperty(e, 'id', { value: undefined, writable: true, enumerable: false });
  return e;
}
function textOf(e) {
  if (e == null) return '';
  return (e.textContent || '') + ' ' + (e.kids || []).map(textOf).join(' ');
}
function findId(e, id) {
  if (!e) return null;
  if (e.id === id) return e;
  for (const k of e.kids || []) { const f = findId(k, id); if (f) return f; }
  return null;
}
function findTag(e, tag) {
  if (!e) return null;
  if (e.tag === tag) return e;
  for (const k of e.kids || []) { const f = findTag(k, tag); if (f) return f; }
  return null;
}

const ok = (c, m) => { console.log((c ? 'PASS ' : 'FAIL ') + m); if (!c) process.exitCode = 1; };
console.error = () => {}; console.warn = () => {};
global.window = global;
global.setTimeout = (f) => { f(); return 0; };
global.fetch = async () => ({ status: 200, ok: true, headers: { get: () => '' }, json: async () => ({}) });
global.XMLHttpRequest = class { open() {} send() {} };

async function run({ gpu, ua = 'Mozilla/5.0 (Windows NT 10.0) Chrome/150 Safari/537.36', search = '', coarse = false, host = 'play.leylinelauncher.com' }) {
  const body = fakeEl('body');
  global.document = {
    body, head: fakeEl('head'), documentElement: fakeEl('html'), currentScript: null,
    createElement: fakeEl, getElementById: (id) => findId(body, id),
    addEventListener() {}, querySelector() { return null; }, querySelectorAll() { return []; },
  };
  global.location = { href: `https://${host}/${search}`, origin: `https://${host}`, hostname: host, pathname: '/', search, hash: '' };
  Object.defineProperty(global, 'navigator', { value: { userAgent: ua, gpu }, configurable: true, writable: true });
  global.matchMedia = (q) => ({ matches: coarse && /coarse/.test(q) });
  delete global.__wuGpu;
  delete require.cache[require.resolve(EARLY)];
  require(EARLY);
  await new Promise((r) => setImmediate(r));
  return { body, note: findId(body, 'wu-gpu-off') };
}

(async () => {
  const adapter = { requestAdapter: async () => ({}) };
  let r = await run({ gpu: adapter });
  ok(!r.note && window.__wuGpu && window.__wuGpu.ok === true, 'a browser with a GPU adapter: no note, __wuGpu.ok');

  r = await run({ gpu: undefined });
  ok(!!r.note, 'no WebGPU: the note shows');
  ok(/can't draw the game/.test(textOf(r.note)) && /graphics acceleration/.test(textOf(r.note)), 'Chrome without WebGPU: says to turn on graphics acceleration');
  ok(window.__wuGpu && window.__wuGpu.ok === false && /no WebGPU/.test(window.__wuGpu.why), '__wuGpu says why');
  const a = findTag(r.note, 'a');
  ok(!!a && a.href === 'https://leylinelauncher.com/wrath/download', 'the note links to the desktop app');
  const b = findTag(r.note, 'button');
  ok(!!b && /Try anyway/.test(b.textContent), 'a Try anyway button');
  b.listeners.click[0]();
  ok(!findId(r.body, 'wu-gpu-off'), 'Try anyway closes the note');

  r = await run({ gpu: { requestAdapter: async () => null } });
  ok(!!r.note && /blocked|off/.test(window.__wuGpu.why), 'WebGPU but no adapter (acceleration off): the note shows');

  r = await run({ gpu: { requestAdapter: async () => { throw new Error('boom'); } } });
  ok(!!r.note && /failed/.test(window.__wuGpu.why), 'the adapter request fails: the note shows');

  r = await run({ gpu: undefined, ua: 'Mozilla/5.0 (X11; Linux) Gecko/20100101 Firefox/140.0' });
  ok(/Firefox/.test(textOf(r.note)) && /Chrome or Edge/.test(textOf(r.note)), 'Firefox: says to open the page in Chrome or Edge');

  r = await run({ gpu: undefined, ua: 'Mozilla/5.0 (Macintosh) AppleWebKit/605 Version/18 Safari/605.1.15' });
  ok(/Safari needs its newest update/.test(textOf(r.note)), 'Safari: says to update it');

  r = await run({ gpu: undefined, ua: 'Mozilla/5.0 (Linux; Android 14) Chrome/150 Mobile Safari/537.36', coarse: true });
  ok(/phone/.test(textOf(r.note)), 'a phone: says to try Chrome or a computer');

  r = await run({ gpu: undefined, search: '?nogpucheck=1' });
  ok(!r.note, '?nogpucheck=1: no check');

  r = await run({ gpu: undefined, host: 'play.openvanilla.app' });
  ok(findTag(r.note, 'a').href === 'https://play.openvanilla.app/wrath/download', 'the old name links to its own site');

  for (const w of ['World of Warcraft', 'WoW', 'Warcraft', 'Blizzard', 'Lich King']) {
    r = await run({ gpu: undefined });
    ok(!new RegExp('\\b' + w + '\\b').test(textOf(r.note)), `no ${w} in the note`);
  }
  process.exit(process.exitCode || 0);
})().catch((e) => { console.log('FAIL threw ' + (e && e.stack || e)); process.exit(1); });
