// The browser game's service worker (www-files/wrath-sw.js) in a Node vm with a fake worker scope: which /data reads it
// keeps (world data only, cache-first, one network fetch for parallel asks), what it never touches (Interface\,
// DBFilesClient\, /data/__*, ranges, other origins, non-GET), and its kill switch (/sw-config.json {"sw":"off"}).
//
//   node --test tests/sw.page.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const SRC = fs.readFileSync(path.join(__dirname, '..', 'www-files', 'wrath-sw.js'), 'utf8');
const ORIGIN = 'https://play.example';

function scope({ config = {}, body = (u) => 'body of ' + u } = {}) {
  const handlers = {};
  const stores = new Map();
  const net = [];
  const st = { unregistered: false, config };
  const cacheOf = (name) => {
    if (!stores.has(name)) stores.set(name, new Map());
    const m = stores.get(name);
    return {
      match: async (u) => (m.has(String(u)) ? m.get(String(u)).clone() : undefined),
      put: async (u, r) => { m.set(String(u), r); },
      keys: async () => [...m.keys()].map((url) => ({ url })),
      delete: async (k) => m.delete(String(k && k.url ? k.url : k)),
    };
  };
  const caches = {
    open: async (n) => cacheOf(n),
    delete: async (n) => stores.delete(n),
    keys: async () => [...stores.keys()],
  };
  const fetch = async (req, init) => {
    const url = typeof req === 'string' ? req : req.url;
    net.push(url);
    await new Promise((r) => setTimeout(r, 5));
    if (url.endsWith('/sw-config.json')) return new Response(JSON.stringify(st.config), { status: 200, headers: { 'content-type': 'application/json' } });
    const r = new Response(body(url), { status: 200, headers: { 'content-type': 'application/octet-stream' } });
    Object.defineProperty(r, 'type', { value: 'basic' });
    return r;
  };
  const self = {
    location: new URL(ORIGIN + '/wrath-sw.js'),
    addEventListener: (t, f) => { handlers[t] = f; },
    skipWaiting: () => {},
    clients: { claim: async () => {} },
    registration: { unregister: async () => { st.unregistered = true; } },
  };
  const ctx = vm.createContext({ self, caches, fetch, URL, Response, Request, Headers, Map, Date, setTimeout, console,
    decodeURIComponent, navigator: { storage: { estimate: async () => ({ usage: 0, quota: 1e9 }) } } });
  vm.runInContext(SRC, ctx);
  const ask = (url, { method = 'GET', headers = {} } = {}) => {
    let answered = null;
    const req = new Request(url, { method, headers });
    handlers.fetch({ request: req, respondWith: (p) => { answered = p; } });
    return answered;   // null: the browser's own fetch (not touched)
  };
  return { handlers, stores, net, st, ask };
}
const data = (name) => `${ORIGIN}/data/${encodeURIComponent(name)}`;

test('world data: kept and answered from the cache; parallel asks share one fetch', async () => {
  const s = scope();
  const u = data('World\\Maps\\Azeroth\\Azeroth_32_48.adt');
  const [a, b] = [s.ask(u), s.ask(u)];
  assert.ok(a && b, 'the worker answers world data');
  assert.equal(await (await a).text(), 'body of ' + u);
  assert.equal(await (await b).text(), 'body of ' + u);
  assert.equal(s.net.filter((x) => x === u).length, 1, 'one network fetch for two asks');
  await new Promise((r) => setTimeout(r, 20));
  const again = await s.ask(u);
  assert.equal(await again.text(), 'body of ' + u);
  assert.equal(s.net.filter((x) => x === u).length, 1, 'the second visit is the cache');
  for (const n of ['Creature\\Wolf\\Wolf.m2', 'Textures\\Minimap\\x.blp', 'sound/music/a.mp3', 'SPELLS\\Fire.m2']) assert.ok(s.ask(data(n)), n);
});

test('never touched: the interface, the tables, the lists, ranges, other sites, posts', () => {
  const s = scope();
  for (const n of ['Interface\\GlueXML\\GlueXML.toc', 'DBFilesClient\\Spell.dbc', 'Fonts\\FRIZQT__.TTF', 'WTF\\DefaultBindings.wtf']) {
    assert.equal(s.ask(data(n)), null, n + ' (can change under its name)');
  }
  assert.equal(s.ask(`${ORIGIN}/data/__index`), null);
  assert.equal(s.ask(`${ORIGIN}/data/__tier/index.json`), null);
  assert.equal(s.ask(`${ORIGIN}/wenilla_bg.wasm`), null);
  assert.equal(s.ask(data('World\\x.adt'), { headers: { range: 'bytes=0-9' } }), null, 'a range read');
  assert.equal(s.ask('https://elsewhere.example/data/' + encodeURIComponent('World\\x.adt')), null, 'another site');
  assert.equal(s.ask(data('World\\x.adt'), { method: 'POST' }), null);
  assert.equal(s.ask(`${ORIGIN}/data/%E0%A4%A`), null, 'a broken escape is not ours');
});

test('the kill switch: {"sw":"off"} drops the cache, unregisters, and stops answering', async () => {
  const s = scope({ config: { sw: 'off' } });
  let done = null;
  s.handlers.activate({ waitUntil: (p) => { done = p; } });
  await done;
  assert.equal(s.st.unregistered, true);
  assert.equal(s.stores.has('wu-data-v1'), false);
  assert.equal(s.ask(data('World\\x.adt')), null, 'disabled: the browser fetches by itself');
});

test('an old cache version goes on activate; the current one stays', async () => {
  const s = scope();
  await (await s.stores.set('wu-data-v0', new Map()));
  s.stores.set('wu-data-v1', new Map());
  s.stores.set('wu-sections-v1', new Map());
  let done = null;
  s.handlers.activate({ waitUntil: (p) => { done = p; } });
  await done;
  assert.deepEqual([...s.stores.keys()].sort(), ['wu-data-v1', 'wu-sections-v1']);
});
