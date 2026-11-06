// den-lite-worker.js — the page's own Den Lite, in a Web Worker (docs/PHONE-SOLO.md). Started by solo.js with:
//   {t: 'start', sab, files: {path: Uint8Array}, env: {...}, engine: {kind: 'wasm', url} | {kind: 'script', url}}
// It runs Den Lite's wasm (labs/den-lite, `den-lite-web`, wasm32-wasip1) on solo-wasi.js, with the `leyline`
// imports Den Lite's web.rs needs: ly_recv (a record from the page's ring, solo-ring.js; this is where the worker
// blocks), ly_send (bytes for one of the client's sockets) and ly_close. A 'script' engine is the same contract in
// JavaScript (self.LeylineSoloEngine.run(ly)): the tests' stand-in server, and the door for another world.
//
// To the page: {t:'ready'}, {t:'d', c, b} (bytes for socket c; b is transferred), {t:'c', c} (socket c closed by
// the server), {t:'files', put: {path: bytes}, del: [path]} (the Dens folder changed: the page keeps it),
// {t:'drain'} (room in the ring again), {t:'log', fd, line}, {t:'stats', ...}, {t:'exit', code, error}.
/* global importScripts, LeylineSoloRing, LeylineSoloWasi */
'use strict';
importScripts('./solo-ring.js', './solo-wasi.js');

var DENS = '/dens/';
var started = false;

/** What the page keeps of the Dens folder: the saves and settings, not the log that only grows nor temp files. */
function kept(path) {
  if (path.indexOf(DENS) !== 0) return false;
  var name = path.slice(path.lastIndexOf('/') + 1);
  return name !== 'den-lite.events' && name.charAt(0) !== '.' && !/\.tmp$/.test(name);
}

function run(m) {
  var ring = new LeylineSoloRing(m.sab);
  var fs = new LeylineSoloWasi.MemFS(m.files || {}, {
    cap: function (p) { return /den-lite\.events$/.test(p) ? 256 * 1024 : 0; },
  });
  fs.mkdirp('/dens');
  var put = {}, del = new Set(), dirty = false, lastFlush = 0, lastStats = 0, records = 0, bytesIn = 0, bytesOut = 0;
  var memory = null;
  var flush = function (force) {
    var now = Date.now();
    if (dirty && (force || now - lastFlush > 500)) {
      var out = {}, list = [];
      Object.keys(put).forEach(function (k) { out[k] = put[k]; list.push(put[k].buffer); });
      self.postMessage({ t: 'files', put: out, del: Array.from(del) }, list);
      put = {}; del = new Set(); dirty = false; lastFlush = now;
    }
    if (now - lastStats > 10000) {
      lastStats = now;
      var held = 0;
      fs.files.forEach(function (f) { held += f.data.length; });
      self.postMessage({ t: 'stats', memory: memory ? memory.buffer.byteLength : 0, files: fs.files.size, fileBytes: held,
                         records: records, bytesIn: bytesIn, bytesOut: bytesOut });
    }
  };
  var wasi = new LeylineSoloWasi.Wasi({
    fs: fs,
    args: ['den-lite-web'],
    env: Object.assign({ DEN_LITE_DENS: '/dens' }, m.env || {}),
    memory: function () { return memory; },
    onChange: function (p, b) { if (kept(p)) { put[p] = b; del.delete(p); dirty = true; } },
    onRemove: function (p) { if (kept(p)) { delete put[p]; del.add(p); dirty = true; } },
    log: function (fd, line) { self.postMessage({ t: 'log', fd: fd, line: String(line).slice(0, 400) }); },
  });
  var tookOne = function () {
    records++;
    if (ring.drained) self.postMessage({ t: 'drain' });
  };
  var send = function (conn, bytes) {
    var b = new Uint8Array(bytes.length);
    b.set(bytes);
    bytesOut += b.length;
    self.postMessage({ t: 'd', c: conn, b: b.buffer }, [b.buffer]);
  };
  var leyline = {
    ly_recv: function (buf, cap, timeout) {
      flush(false);
      if (!ring.wait(timeout)) return 0;
      var len = ring.peek();
      if (len < 0) return 0;
      if (len > cap) return -len;
      ring.take(new Uint8Array(memory.buffer, buf, len), 0);
      bytesIn += len;
      tookOne();
      return len;
    },
    ly_send: function (conn, ptr, len) { send(conn, new Uint8Array(memory.buffer, ptr, len)); },
    ly_close: function (conn) { self.postMessage({ t: 'c', c: conn }); },
  };
  var engine = m.engine || {};
  if (engine.kind === 'script') {
    importScripts(engine.url);
    self.postMessage({ t: 'ready', engine: 'script' });
    var code = self.LeylineSoloEngine.run({
      fs: fs, env: wasi.env,
      recv: function (timeout) { flush(false); if (!ring.wait(timeout)) return null; var r = ring.next(); if (r) { bytesIn += r.data.length + 5; tookOne(); } return r; },
      send: function (conn, bytes) { send(conn, bytes); },
      close: function (conn) { self.postMessage({ t: 'c', c: conn }); },
      changed: function (p) { var b = fs.readFile(p); if (b && kept(p)) { put[p] = b; dirty = true; } },
    });
    flush(true);
    self.postMessage({ t: 'exit', code: code || 0 });
    return Promise.resolve();
  }
  var load = m.module ? Promise.resolve(m.module)
    : (WebAssembly.compileStreaming ? WebAssembly.compileStreaming(fetch(engine.url)) : fetch(engine.url).then(function (r) { return r.arrayBuffer(); }).then(WebAssembly.compile));
  return load.then(function (mod) {
    return WebAssembly.instantiate(mod, { wasi_snapshot_preview1: wasi.imports, leyline: leyline });
  }).then(function (inst) {
    memory = inst.exports.memory;
    self.postMessage({ t: 'ready', engine: 'wasm' });
    var code = wasi.start(inst);
    flush(true);
    self.postMessage({ t: 'exit', code: code });
  });
}

self.onmessage = function (e) {
  var m = e.data || {};
  if (m.t !== 'start' || started) return;
  started = true;
  Promise.resolve().then(function () { return run(m); }).catch(function (err) {
    self.postMessage({ t: 'exit', code: -1, error: String((err && err.stack) || err).slice(0, 600) });
  });
};
