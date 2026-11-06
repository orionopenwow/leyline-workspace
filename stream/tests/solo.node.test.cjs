// Solo on this device (docs/PHONE-SOLO.md), in node: the crypto and SRP6 of the test client, the page's ring to the
// worker, the worker's WASI file system, save copies, the wrath-early.js hook, and the whole way in (login, realm
// list, world auth with its header encryption, the character list, entering the world, ping) through the real
// den-lite-worker.js in a worker thread and solo.js's fake sockets. The engine is the stand-in server
// (tests/solo/stub-engine.js), or Den Lite's own wasm when it is built: DEN_LITE_WASM=<file>, or
// labs/den-lite/target/wasm32-wasip1/release/den-lite-web.wasm (CI: den-lite-web.yml).
//
//   node --test tests/solo.node.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { Worker } = require('node:worker_threads');

const SOLO = path.join(__dirname, '..', 'www-files', 'solo');
const Ring = require(path.join(SOLO, 'solo-ring.js'));
const W = require(path.join(SOLO, 'solo-wasi.js'));
const Solo = require(path.join(SOLO, 'solo.js'));
const Wire = require('./solo/wire335.js');
const hex = (u8) => Buffer.from(u8).toString('hex');
const WASM = process.env.DEN_LITE_WASM || path.join(__dirname, '..', '..', 'labs', 'den-lite', 'target', 'wasm32-wasip1', 'release', 'den-lite-web.wasm');
const REAL = fs.existsSync(WASM);

test('sha1, hmac and rc4 match the published vectors', () => {
  assert.equal(hex(Wire.sha1('abc')), 'a9993e364706816aba3e25717850c26c9cd0d89d');
  assert.equal(hex(Wire.sha1('')), 'da39a3ee5e6b4b0d3255bfef95601890afd80709');
  assert.equal(hex(Wire.sha1('a'.repeat(1000))), '291e9a6c66994949b57ba5e650361e98fc36b1ba');
  assert.equal(hex(Wire.hmacSha1(new TextEncoder().encode('key'), new TextEncoder().encode('The quick brown fox jumps over the lazy dog'))),
    'de7c9b85b8b78aa6bc8a7a36f70a90701c9db4d9');
  const r = new Wire.Rc4(new TextEncoder().encode('Key'));
  assert.equal(hex(r.apply(new TextEncoder().encode('Plaintext'))), 'bbf316e8d940af0ad3');
});

test('srp6: client and server agree on the key and the proofs; a wrong password does not', () => {
  const s = Wire.srpServer('SOLO', 'den');
  const c = Wire.srpClient('solo', 'DEN', s.B, s.salt);   // names and passwords are upper-cased
  const ok = s.check(c.A, c.M1);
  assert.ok(ok);
  assert.equal(hex(ok.K), hex(c.K));
  assert.equal(hex(ok.M2), hex(c.M2));
  const bad = Wire.srpClient('SOLO', 'nope', s.B, s.salt);
  assert.equal(s.check(bad.A, bad.M1), null);
  // the header streams of one side are the other side's
  const a = Wire.headerCrypto(c.K, 'client'), b = Wire.headerCrypto(c.K, 'server');
  const h = new Uint8Array([0, 4, 0x37, 0, 0, 0]);
  assert.equal(hex(b.in.apply(a.out.apply(h.slice()))), hex(h));
});

test('the ring keeps records whole and in order, wraps around, and says when it is full', () => {
  const r = Ring.create(4096);
  for (let round = 0; round < 50; round++) {
    const body = new Uint8Array(700 + round).fill(round);
    assert.equal(r.push(2, round, body), true);
    const got = r.next();
    assert.equal(got.kind, 2);
    assert.equal(got.conn, round);
    assert.equal(got.data.length, 700 + round);
    assert.ok(got.data.every((x) => x === (round & 255)));
  }
  const big = new Uint8Array(1500);
  assert.equal(r.push(2, 1, big), true);
  assert.equal(r.push(2, 2, big), true);
  assert.equal(r.push(2, 3, big), false, 'no room: nothing written');
  assert.equal(r.next().conn, 1);
  assert.equal(r.drained, true, 'the reader says it drained');
  assert.equal(r.push(2, 3, big), true);
  assert.deepEqual([r.next().conn, r.next().conn, r.next()], [2, 3, null]);
  assert.throws(() => r.push(2, 1, new Uint8Array(5000)), RangeError);
  r.setPaused(true);
  assert.equal(r.paused(), true);
});

/** A WASI against a real memory, called as wasm would: strings and structs at addresses. */
function wasiHarness(fs0, opts = {}) {
  const mem = new WebAssembly.Memory({ initial: 2 });
  const changed = [], removed = [];
  const fsys = new W.MemFS(fs0 || {}, opts);
  const w = new W.Wasi({ fs: fsys, memory: () => mem, onChange: (p, b) => changed.push([p, Buffer.from(b).toString()]), onRemove: (p) => removed.push(p) });
  const u8 = () => new Uint8Array(mem.buffer), dv = () => new DataView(mem.buffer);
  let top = 1024;
  const str = (s) => { const b = new TextEncoder().encode(s); const at = top; u8().set(b, at); top += b.length + 8; return [at, b.length]; };
  const out = () => { const at = top; top += 64; return at; };
  const open = (p, oflags, write, fdflags = 0) => {
    const [a, l] = str(p), o = out();
    const e = w.imports.path_open(3, 0, a, l, oflags, write ? 64n : 2n, 0n, fdflags, o);
    return e ? -e : dv().getUint32(o, true);
  };
  const write = (fd, s) => {
    const [a, l] = str(s), iov = out(), n = out();
    dv().setUint32(iov, a, true); dv().setUint32(iov + 4, l, true);
    return w.imports.fd_write(fd, iov, 1, n);
  };
  const read = (fd, max = 256) => {
    const buf = out(); top += max; const iov = out(), n = out();
    dv().setUint32(iov, buf, true); dv().setUint32(iov + 4, max, true);
    assert.equal(w.imports.fd_read(fd, iov, 1, n), 0);
    return new TextDecoder().decode(u8().slice(buf, buf + dv().getUint32(n, true)));
  };
  return { w, fsys, mem, changed, removed, str, out, open, write, read, dv, u8 };
}

test('the worker file system: create, write, append, read, rename into place, list, remove', () => {
  const h = wasiHarness({ '/dens/old.den.json': new TextEncoder().encode('{"v":1}') });
  const E = W.errno;
  assert.equal(h.open('dens/missing', 0, false), -E.NOENT);
  assert.equal(h.open('nowhere/x', 1, true), -E.NOENT, 'no parent folder');
  const fd = h.open('dens/.autosave.tmp', 1 | 8, true);
  assert.ok(fd > 3);
  assert.equal(h.write(fd, 'hello '), 0);
  assert.equal(h.write(fd, 'den'), 0);
  assert.equal(h.w.imports.fd_close(fd), 0);
  assert.deepEqual(h.changed.pop(), ['/dens/.autosave.tmp', 'hello den']);
  const [a, al] = h.str('dens/.autosave.tmp'), [b, bl] = h.str('dens/autosave.den.json');
  assert.equal(h.w.imports.path_rename(3, a, al, 3, b, bl), 0);
  assert.deepEqual(h.changed.pop(), ['/dens/autosave.den.json', 'hello den']);
  assert.ok(h.removed.includes('/dens/.autosave.tmp'));
  const ap = h.open('dens/autosave.den.json', 0, true, 1);
  h.write(ap, '!');
  h.w.imports.fd_close(ap);
  const rd = h.open('dens/autosave.den.json', 0, false);
  assert.equal(h.read(rd), 'hello den!');
  assert.equal(h.write(rd, 'x'), E.BADF, 'opened to read only');
  // metadata: size and kind
  const [p, pl] = h.str('dens/autosave.den.json'), st = h.out();
  assert.equal(h.w.imports.path_filestat_get(3, 0, p, pl, st), 0);
  assert.equal(h.dv().getUint8(st + 16), 4);
  assert.equal(Number(h.dv().getBigUint64(st + 32, true)), 10);
  // the folder's entries (., .., then sorted names)
  const dfd = h.open('dens', 2, false), buf = h.out(); h.out(); h.out(); h.out(); const used = h.out();
  assert.equal(h.w.imports.fd_readdir(dfd, buf, 256, 0n, used), 0);
  const names = [];
  for (let o = 0; o < h.dv().getUint32(used, true);) {
    const n = h.dv().getUint32(buf + o + 16, true);
    names.push(new TextDecoder().decode(h.u8().slice(buf + o + 24, buf + o + 24 + n)));
    o += 24 + n;
  }
  assert.deepEqual(names, ['.', '..', 'autosave.den.json', 'old.den.json']);
  const [d, dl] = h.str('dens/sub');
  assert.equal(h.w.imports.path_create_directory(3, d, dl), 0);
  assert.equal(h.w.imports.path_create_directory(3, d, dl), E.EXIST);
  const [o2, ol2] = h.str('dens/old.den.json');
  assert.equal(h.w.imports.path_unlink_file(3, o2, ol2), 0);
  assert.ok(h.removed.includes('/dens/old.den.json'));
  assert.equal(h.w.imports.path_remove_directory(3, d, dl), 0);
  assert.equal(h.w.imports.sock_accept(), E.NOSYS, 'anything else: not supported');
});

test('a log that only grows is cut to its newest half, and stdout reaches the console by lines', () => {
  const h = wasiHarness({}, { cap: (p) => (p.endsWith('.events') ? 100 : 0) });
  h.fsys.mkdirp('/dens');
  const fd = h.open('dens/den-lite.events', 1, true, 1);
  for (let i = 0; i < 30; i++) h.write(fd, `line ${i}\n`);
  const f = h.fsys.files.get('/dens/den-lite.events');
  assert.ok(f.size <= 100 && Buffer.from(f.data.slice(0, f.size)).toString().includes('line 29'));
  const lines = [];
  const w2 = new W.Wasi({ memory: () => h.mem, log: (fd2, l) => lines.push([fd2, l]) });
  w2.flushLine(2, 'den-lite: one\nden-lite: tw');
  w2.flushLine(2, 'o\n');
  assert.deepEqual(lines, [[2, 'den-lite: one'], [2, 'den-lite: two']]);
});

test('a save copy goes round, and one that reaches outside the Dens folder is refused', () => {
  const files = { '/dens/autosave.den.json': new TextEncoder().encode('{"format":"leyline-den"}'), '/dens/account-data/SOLO/0.bin': new Uint8Array([0, 255, 7]) };
  const doc = Solo.packSave(files, Date.UTC(2026, 10, 5));
  assert.equal(doc.format, 'leyline-solo');
  const back = Solo.unpackSave(JSON.stringify(doc));
  assert.deepEqual(Object.keys(back).sort(), Object.keys(files).sort());
  assert.deepEqual([...back['/dens/account-data/SOLO/0.bin']], [0, 255, 7]);
  assert.throws(() => Solo.unpackSave({ format: 'leyline-solo', version: 1, files: { '/dens/../etc/x': 'AA==' } }), /outside/);
  assert.throws(() => Solo.unpackSave({ format: 'other', version: 1, files: {} }), /not a Leyline solo save/);
});

const early = fs.readFileSync(path.join(__dirname, '..', 'www-files', 'wrath-early.js'), 'utf8');
const SOLO_SRC = early.slice(early.indexOf('// >>> leyline-solo'), early.indexOf('// <<< leyline-solo'));

test('wrath-early.js: solo when asked (and only then), the page made its own, solo.js written in', () => {
  const run = (search, env, extra = {}) => {
    const written = [];
    const win = Object.assign({ location: { search }, __wenilla_env: env, Worker: function () {}, SharedArrayBuffer, crossOriginIsolated: true,
      document: { readyState: 'loading', write: (s) => written.push(s) }, console }, extra);
    win.window = win;
    vm.runInNewContext(SOLO_SRC, win);
    return { st: win.__wuSoloEarly, env: win.__wenilla_env, written };
  };
  assert.equal(run('', { user: 'A', pass: 'b' }).st.wanted, false);
  assert.equal(run('?solo=0&go=solo', {}).st.wanted, false);
  const a = run('?go=solo', { user: 'WR1', pass: 'x', hop: 'abcdefghijklmnop', hop_ws: 'wss://r/ws/abcdefghijklmnop' });
  assert.equal(a.st.ok, true);
  assert.equal(a.env.hop, undefined, 'never a hop in solo');
  assert.equal(a.env.solo, 'phone');
  assert.match(a.written.join(''), /solo\/solo\.js.*LeylineSolo\.install/);
  assert.equal(run('', { solo: 'phone' }).st.ok, true, 'the Android app asks through the env');
  const no = run('?solo=1', {}, { crossOriginIsolated: false });
  assert.equal(no.st.ok, false);
  assert.match(no.st.why, /shared memory/);
  assert.equal(no.written.length, 0, 'the page plays as before');
});

/** solo.js's Bus and sockets, with a node worker thread as the page's Worker. */
function nodeWin() {
  class NodeWorker {
    constructor(url) {
      this.w = new Worker(path.join(__dirname, 'solo', 'node-worker-shim.cjs'), { workerData: { script: url } });
      this.w.on('message', (m) => this.onmessage && this.onmessage({ data: m }));
      this.w.on('error', (e) => this.onerror && this.onerror(e));
    }
    postMessage(m, t) { this.w.postMessage(m, t); }
    terminate() { return this.w.terminate(); }
  }
  return { Worker: NodeWorker, setTimeout, performance, document: null, addEventListener() {} };
}
function memStore(init = {}) {
  const files = { ...init };
  return { files, load: async () => ({ ...files }), apply: async (put, del) => { Object.assign(files, put); (del || []).forEach((k) => delete files[k]); }, replace: async (f) => { Object.keys(files).forEach((k) => delete files[k]); Object.assign(files, f); } };
}

test(`the whole way in through the worker and the fake sockets (${REAL ? 'Den Lite wasm' : 'stand-in server'})`, { timeout: 60000 }, async () => {
  const win = nodeWin();
  const store = memStore();
  const engine = REAL ? { kind: 'wasm', url: WASM } : { kind: 'script', url: path.join(__dirname, 'solo', 'stub-engine.js') };
  const bus = new Solo.Bus(win, { workerUrl: path.join(SOLO, 'den-lite-worker.js'), engine, store, env: { DEN_LITE_PASSWORD: 'den', DEN_LITE_NAME: 'Wren' } });
  const Sock = Solo.makeSocketClass(win, bus);
  const t0 = performance.now();
  try {
    const ls = Wire.socketStream(new Sock('ws://game.example/ws/3724'));
    await ls.opened;
    const L = await Wire.login(ls, 'SOLO', 'den');
    assert.equal(L.realms.length, 1);
    const port = Number(L.realms[0].address.split(':').pop());
    assert.ok(port > 0 && port !== 3724, L.realms[0].address);
    const ws = Wire.socketStream(new Sock(`ws://game.example/ws/${port}`));
    await ws.opened;
    const inWorld = await Wire.enterWorld(ws, 'SOLO', L.K);
    assert.equal(inWorld.name, 'Wren');
    assert.ok(Number.isFinite(inWorld.verify.x) && Number.isFinite(inWorld.verify.z));
    const ms = Math.round(performance.now() - t0);
    console.log(`  in the world in ${ms} ms (${REAL ? 'wasm' : 'stand-in'}; worker ready in ${bus.events.readyMs} ms), map ${inWorld.verify.map}, ${inWorld.seen.length} messages`);
    ls.close();
    ws.close();
    // the Dens folder reaches the page's store (the stand-in writes a file when the player is in; Den Lite its events
    // are not kept, its autosave and account data are)
    if (!REAL) {
      for (let i = 0; i < 40 && !store.files['/dens/stub-visits.json']; i++) await new Promise((r) => setTimeout(r, 50));
      assert.ok(store.files['/dens/stub-visits.json'], 'kept');
    }
    assert.ok(bus.status().events.toServer > 4);
  } catch (e) {
    console.log('  solo status:', JSON.stringify(bus.status()));
    throw e;
  } finally {
    bus.stop();
    if (bus.worker) await bus.worker.terminate();
  }
});
