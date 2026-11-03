// The slice's misses in the page layer, in node (no browser): wrath-page.js missReporter sends each game file the
// slice lacked (wrath-early.js window.__wuMissing) to POST /__gate?missing=1 once, in batches, and stops when the
// portal says this is not a streamed session.
//
//   node --test tests/misses.page.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const WWW = path.join(__dirname, '..', 'www-files');

async function load() {
  globalThis.window = { __wenilla_env: {} };
  return import(pathToFileURL(path.join(WWW, 'wrath-page.js')).href);
}

function harness(m, opts = {}) {
  const missing = new Map();
  const posts = [];
  const listeners = {};
  let tick = null;
  let status = opts.status || 200;
  const r = m.missReporter({
    missing,
    fetchFn: async (url, init) => { posts.push({ url, init, body: JSON.parse(init.body) }); return { status }; },
    setIntervalFn: (f) => { tick = f; return 1; },
    listen: (t, f) => { listeners[t] = f; },
    hidden: () => opts.hidden !== false,
    search: opts.search || '',
  });
  return { r, missing, posts, listeners, tick: () => tick(), setStatus: (s) => { status = s; } };
}

test('each missed name goes to the portal once, same origin, as JSON', async () => {
  const m = await load();
  const h = harness(m);
  h.missing.set('creature\\druidcat\\druidcat.m2', {});
  h.missing.set('creature\\druidcat\\druidcat00.skin', {});
  assert.equal(await h.r.send(), 2);
  assert.equal(h.posts[0].url, '/__gate?missing=1');
  assert.equal(h.posts[0].init.method, 'POST');
  assert.equal(h.posts[0].init.credentials, 'same-origin');
  assert.equal(h.posts[0].init.headers['Content-Type'], 'application/json');
  assert.deepEqual(h.posts[0].body.names, ['creature\\druidcat\\druidcat.m2', 'creature\\druidcat\\druidcat00.skin']);
  assert.equal(await h.r.send(), 0, 'nothing new: no post');
  h.missing.set('creature\\wolf\\wolf.m2', {});
  h.tick();
  await new Promise((res) => setImmediate(res));
  assert.deepEqual(h.posts[1].body.names, ['creature\\wolf\\wolf.m2']);
});

test('batches of MISS_BATCH; a hidden tab sends at once with keepalive; overlong names are dropped', async () => {
  const m = await load();
  const h = harness(m);
  for (let i = 0; i < m.MISS_BATCH + 5; i++) h.missing.set(`creature\\x\\x${i}.blp`, {});
  h.missing.set('a'.repeat(250) + '.m2', {});
  assert.equal(await h.r.send(), m.MISS_BATCH);
  h.listeners.visibilitychange();
  await new Promise((res) => setImmediate(res));
  assert.equal(h.posts.length, 2);
  assert.equal(h.posts[1].init.keepalive, true);
  assert.equal(h.posts[1].body.names.length, 5);
  assert.ok(h.posts.every((p) => p.body.names.every((n) => n.length <= 200)));
});

test('stops when the portal refuses (not a streamed session, an older portal)', async () => {
  const m = await load();
  const h = harness(m, { status: 403 });
  h.missing.set('creature\\a\\a.m2', {});
  await h.r.send();
  h.missing.set('creature\\b\\b.m2', {});
  assert.equal(await h.r.send(), 0);
  assert.equal(h.posts.length, 1);
  assert.equal(h.r.stopped, true);
});

test('off with your own game folder, ?nomissreport=1, or without the capture', async () => {
  const m = await load();
  assert.equal(harness(m, { search: '?ownfiles=1' }).r, null);
  assert.equal(harness(m, { search: '?x=1&nomissreport=1' }).r, null);
  assert.equal(m.missReporter({ missing: undefined, search: '' }), null);
});
