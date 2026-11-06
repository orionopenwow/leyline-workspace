// solo.js — "Play solo on this device" (docs/PHONE-SOLO.md): the browser game with no server. Den Lite (our own
// server, labs/den-lite) runs in a Web Worker on this page (den-lite-worker.js) and the client's sockets are fake
// ones that talk to it, so after the page has loaded nothing goes over the network but the game's own files.
//
//   * SoloSocket: what the client gets for `new WebSocket('{page}/ws/<port>')` (benilla-app net/ws.rs dials /ws/3724
//     for the login and /ws/<the realm list's port> for the world). Binary messages, one per server write, as the
//     relay's bridges and wu-play send them; the client reassembles them as a stream.
//   * Bus: one worker, one shared ring for the client's bytes (solo-ring.js; the worker blocks on it), postMessage
//     for the server's. The page being hidden pauses the worker (no beats, no CPU); pagehide stops it.
//   * Store: the Dens folder (the Den's autosave, the class and looks, the key bindings the game uploads) in this
//     browser's IndexedDB ("leyline-solo"), with a save copy to a file and a restore from one.
//
// wrath-early.js installs it before the client starts when the page asks for solo (?solo=1, ?go=solo, or env.solo).
// A later transport (a friend's page over a WebRTC data channel: "Sky Island" in the doc) is one more Bus.attach().
(function (root, factory) {
  'use strict';
  if (typeof module === 'object' && module.exports) module.exports = factory(root);
  else root.LeylineSolo = factory(root);
})(typeof window !== 'undefined' ? window : globalThis, function (root) {
  'use strict';
  var Ring = root.LeylineSoloRing || (typeof require === 'function' ? require('./solo-ring.js') : null);
  var KIND = { OPEN: 1, DATA: 2, CLOSE: 3, STOP: 4 };
  var DB = 'leyline-solo', STORE = 'files', FORMAT = 'leyline-solo';
  var HERO = 'Wren', ACCOUNT = 'SOLO', PASSWORD = 'den';

  // -- Saves (IndexedDB) ------------------------------------------------------------------------------------------

  function idb(win) {
    return new Promise(function (ok, fail) {
      var r = win.indexedDB.open(DB, 1);
      r.onupgradeneeded = function () { r.result.createObjectStore(STORE); };
      r.onsuccess = function () { ok(r.result); };
      r.onerror = function () { fail(r.error); };
    });
  }
  function tx(db, mode, fn) {
    return new Promise(function (ok, fail) {
      var t = db.transaction(STORE, mode), s = t.objectStore(STORE), out = fn(s);
      t.oncomplete = function () { ok(out && out.result !== undefined ? out.result : out); };
      t.onerror = function () { fail(t.error); };
      t.onabort = function () { fail(t.error); };
    });
  }
  function Store(win) { this.win = win; this.db = null; }
  Store.prototype.open = function () {
    var self = this;
    if (!this.win.indexedDB) return Promise.resolve(null);
    if (!this.db) this.db = idb(this.win).catch(function () { return null; });
    return this.db.then(function (d) { return d; }, function () { return null; });
  };
  /** Every kept file, {path: Uint8Array}. */
  Store.prototype.load = function () {
    return this.open().then(function (db) {
      if (!db) return {};
      return new Promise(function (ok) {
        var out = {}, t = db.transaction(STORE, 'readonly'), c = t.objectStore(STORE).openCursor();
        c.onsuccess = function () {
          var cur = c.result;
          if (!cur) return;
          out[cur.key] = new Uint8Array(cur.value);
          cur.continue();
        };
        t.oncomplete = function () { ok(out); };
        t.onerror = function () { ok(out); };
      });
    });
  };
  Store.prototype.apply = function (put, del) {
    return this.open().then(function (db) {
      if (!db) return;
      return tx(db, 'readwrite', function (s) {
        Object.keys(put || {}).forEach(function (k) { s.put(put[k], k); });
        (del || []).forEach(function (k) { s.delete(k); });
      });
    });
  };
  Store.prototype.replace = function (files) {
    return this.open().then(function (db) {
      if (!db) return;
      return tx(db, 'readwrite', function (s) {
        s.clear();
        Object.keys(files).forEach(function (k) { s.put(files[k], k); });
      });
    });
  };

  function b64(u8) {
    var s = '';
    for (var i = 0; i < u8.length; i += 0x8000) s += String.fromCharCode.apply(null, u8.subarray(i, i + 0x8000));
    return root.btoa ? root.btoa(s) : Buffer.from(s, 'binary').toString('base64');
  }
  function unb64(s) {
    var bin = root.atob ? root.atob(s) : Buffer.from(s, 'base64').toString('binary');
    var u8 = new Uint8Array(bin.length);
    for (var i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
    return u8;
  }
  /** A save copy: {format, version, saved, files: {path: base64}} (the Dens folder only). */
  function packSave(files, now) {
    var out = {};
    Object.keys(files).sort().forEach(function (k) { if (k.indexOf('/dens/') === 0) out[k] = b64(files[k]); });
    return { format: FORMAT, version: 1, saved: new Date(now || Date.now()).toISOString(), files: out };
  }
  /** A save copy read back: {path: Uint8Array}; throws on anything that isn't one. Paths stay inside /dens/. */
  function unpackSave(doc) {
    if (typeof doc === 'string') doc = JSON.parse(doc);
    if (!doc || doc.format !== FORMAT || doc.version !== 1 || typeof doc.files !== 'object') throw new Error('not a Leyline solo save');
    var out = {}, n = 0;
    Object.keys(doc.files).forEach(function (k) {
      if (!/^\/dens\/[A-Za-z0-9._\-/]{1,160}$/.test(k) || k.indexOf('..') >= 0) throw new Error('a file outside the save: ' + k.slice(0, 40));
      if (typeof doc.files[k] !== 'string') throw new Error('a damaged file in the save');
      out[k] = unb64(doc.files[k]);
      if (++n > 2000) throw new Error('too many files in the save');
    });
    return out;
  }

  // -- The fake socket ----------------------------------------------------------------------------------------------

  function makeSocketClass(win, bus) {
    var Native = win.WebSocket;
    var mkEvent = function (type, init) {
      try {
        if (type === 'message' && typeof win.MessageEvent === 'function') return new win.MessageEvent('message', init);
        if (type === 'close' && typeof win.CloseEvent === 'function') return new win.CloseEvent('close', init);
        if (typeof win.Event === 'function') return new win.Event(type);
      } catch (e) { /* a plain object below */ }
      var o = { type: type };
      Object.keys(init || {}).forEach(function (k) { o[k] = init[k]; });
      return o;
    };
    function SoloSocket(url, protocols) {
      var m = /\/ws\/(\d{1,5})(?:[?#].*)?$/.exec(String(url));
      this.url = String(url);
      this.protocol = '';
      this.extensions = '';
      this.bufferedAmount = 0;
      this.binaryType = 'blob';
      this._state = 0;
      this._port = m ? Number(m[1]) : 0;
      this._listeners = {};
      this.onopen = this.onmessage = this.onclose = this.onerror = null;
      void protocols;
      bus.open(this);
    }
    if (Native && Native.prototype) {
      // `instanceof WebSocket` holds (web-sys casts); every member the client uses is this class's own
      SoloSocket.prototype = Object.create(Native.prototype);
    }
    SoloSocket.prototype.constructor = SoloSocket;
    Object.defineProperty(SoloSocket.prototype, 'readyState', { get: function () { return this._state; }, configurable: true });
    ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED'].forEach(function (k, i) {
      SoloSocket[k] = i;
      Object.defineProperty(SoloSocket.prototype, k, { value: i, configurable: true });
    });
    SoloSocket.prototype.addEventListener = function (type, fn) {
      if (typeof fn !== 'function' && !(fn && typeof fn.handleEvent === 'function')) return;
      (this._listeners[type] = this._listeners[type] || []).push(fn);
    };
    SoloSocket.prototype.removeEventListener = function (type, fn) {
      var l = this._listeners[type];
      if (l) this._listeners[type] = l.filter(function (x) { return x !== fn; });
    };
    SoloSocket.prototype.dispatchEvent = function (ev) {
      var self = this, h = this['on' + ev.type];
      try { if (typeof h === 'function') h.call(this, ev); } catch (e) { setTimeout(function () { throw e; }); }
      (this._listeners[ev.type] || []).slice().forEach(function (fn) {
        try { if (typeof fn === 'function') fn.call(self, ev); else fn.handleEvent(ev); } catch (e) { setTimeout(function () { throw e; }); }
      });
      return true;
    };
    SoloSocket.prototype.send = function (data) {
      if (this._state === 0) throw new (win.DOMException || Error)('Still in CONNECTING state.', 'InvalidStateError');
      if (this._state !== 1) return;
      var u8;
      if (data instanceof ArrayBuffer) u8 = new Uint8Array(data.slice(0));
      else if (ArrayBuffer.isView(data)) u8 = new Uint8Array(data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength));
      else if (typeof data === 'string') u8 = new TextEncoder().encode(data);
      else if (win.Blob && data instanceof win.Blob) {
        var self = this;
        data.arrayBuffer().then(function (b) { if (self._state === 1) bus.data(self, new Uint8Array(b)); });
        return;
      } else return;
      bus.data(this, u8);
    };
    SoloSocket.prototype.close = function (code, reason) {
      if (this._state >= 2) return;
      this._state = 2;
      bus.closeFromClient(this);
      this._closed(code || 1000, reason || '', true);
    };
    SoloSocket.prototype._opened = function () {
      if (this._state !== 0) return;
      this._state = 1;
      this.dispatchEvent(mkEvent('open', {}));
      // what the server said before the open event went out (it answers the moment the socket exists)
      var early = this._early || [];
      this._early = null;
      for (var i = 0; i < early.length && this._state === 1; i++) this._message(early[i]);
    };
    SoloSocket.prototype._message = function (buf) {
      if (this._state === 0) { (this._early = this._early || []).push(buf); return; }
      if (this._state !== 1) return;
      var data = this.binaryType === 'arraybuffer' || !win.Blob ? buf : new win.Blob([buf]);
      this.dispatchEvent(mkEvent('message', { data: data, origin: '' }));
    };
    SoloSocket.prototype._closed = function (code, reason, clean) {
      if (this._state === 3) return;
      var wasOpen = this._state;
      this._state = 3;
      if (!clean && wasOpen === 0) this.dispatchEvent(mkEvent('error', {}));
      this.dispatchEvent(mkEvent('close', { code: code, reason: reason, wasClean: !!clean }));
    };
    SoloSocket.__wuSolo = true;
    return SoloSocket;
  }

  // -- The bus to the worker --------------------------------------------------------------------------------------

  function Bus(win, opts) {
    this.win = win;
    this.opts = opts || {};
    this.conns = new Map();
    this.nextId = 1;
    this.backlog = [];
    this.waiting = [];
    this.ready = false;
    this.failed = null;
    this.stats = {};
    this.logs = [];
    this.store = opts.store || new Store(win);
    this.events = { opened: 0, closed: 0, toServer: 0, fromServer: 0, startedMs: 0, readyMs: 0, firstWorldMs: 0 };
  }

  Bus.prototype.start = function () {
    var self = this, win = this.win, t0 = win.performance ? win.performance.now() : Date.now();
    if (this.starting) return this.starting;
    this.events.startedMs = Math.round(t0);
    this.ring = Ring.create(this.opts.ringBytes || 1 << 20);
    this.starting = this.store.load().then(function (files) {
      return new Promise(function (ok, fail) {
        var w = self.worker = new win.Worker(self.opts.workerUrl);
        w.onmessage = function (e) { self.onWorker(e.data || {}, ok); };
        w.onerror = function (e) { self.fail('the worker failed: ' + ((e && e.message) || 'error')); fail(new Error(self.failed)); };
        var list = [];
        Object.keys(files).forEach(function (k) { list.push(files[k].buffer); });
        w.postMessage({ t: 'start', sab: self.ring.sab, files: files, env: self.opts.env || {}, engine: self.opts.engine }, list);
      });
    }).then(function () {
      self.ready = true;
      self.events.readyMs = Math.round((win.performance ? win.performance.now() : Date.now()) - t0);
      var q = self.waiting; self.waiting = [];
      q.forEach(function (s) { self.connect(s); });
      self.watchPage();
      return self;
    });
    this.starting.catch(function (e) { self.fail(String((e && e.message) || e)); });
    return this.starting;
  };

  Bus.prototype.fail = function (why) {
    if (this.failed) return;
    this.failed = why;
    this.conns.forEach(function (s) { s._closed(1006, 'solo stopped', false); });
    this.conns.clear();
    this.waiting.forEach(function (s) { s._closed(1006, 'solo stopped', false); });
    this.waiting = [];
    if (typeof this.opts.onFail === 'function') this.opts.onFail(why);
  };

  Bus.prototype.onWorker = function (m, ready) {
    var s;
    switch (m.t) {
      case 'ready': ready(); break;
      case 'd':
        s = this.conns.get(m.c);
        this.events.fromServer++;
        if (s) {
          if (s._port !== 3724 && !this.events.firstWorldMs) this.events.firstWorldMs = Math.round((this.win.performance ? this.win.performance.now() : Date.now()) - this.events.startedMs);
          s._message(m.b);
        }
        break;
      case 'c':
        s = this.conns.get(m.c);
        this.conns.delete(m.c);
        this.events.closed++;
        if (s) s._closed(1000, '', true);
        break;
      case 'files':
        this.store.apply(m.put, m.del).catch(function () { /* a save that didn't keep: the next one will */ });
        if (typeof this.opts.onSaved === 'function') this.opts.onSaved(Object.keys(m.put || {}));
        break;
      case 'drain': this.flush(); break;
      case 'stats': this.stats = m; break;
      case 'log':
        this.logs.push(m.line);
        if (this.logs.length > 200) this.logs.shift();
        break;
      case 'exit':
        if (m.code !== 0 || m.error) this.fail(m.error || ('Den Lite stopped (' + m.code + ')'));
        break;
    }
  };

  Bus.prototype.push = function (kind, id, bytes) {
    if (this.backlog.length || !this.ring.push(kind, id, bytes)) this.backlog.push([kind, id, bytes]);
  };
  Bus.prototype.flush = function () {
    while (this.backlog.length) {
      var r = this.backlog[0];
      if (!this.ring.push(r[0], r[1], r[2])) return;
      this.backlog.shift();
    }
  };

  Bus.prototype.open = function (sock) {
    if (this.failed) { setTimeout(function () { sock._closed(1006, 'solo stopped', false); }); return; }
    if (!sock._port) { setTimeout(function () { sock._closed(1006, 'not a game socket', false); }); return; }
    if (!this.ready) { this.waiting.push(sock); this.start(); return; }
    this.connect(sock);
  };
  Bus.prototype.connect = function (sock) {
    var id = this.nextId++;
    sock._id = id;
    this.conns.set(id, sock);
    this.events.opened++;
    this.push(KIND.OPEN, id, new Uint8Array([sock._port & 0xff, sock._port >> 8]));
    setTimeout(function () { sock._opened(); });
  };
  Bus.prototype.data = function (sock, u8) {
    if (!sock._id || !this.conns.has(sock._id)) return;
    this.events.toServer++;
    this.push(KIND.DATA, sock._id, u8);
  };
  Bus.prototype.closeFromClient = function (sock) {
    if (!sock._id || !this.conns.has(sock._id)) return;
    this.conns.delete(sock._id);
    this.push(KIND.CLOSE, sock._id, null);
  };

  /** Hidden page: the worker waits without beats; back: it goes on. Leaving the page: Den Lite saves and stops. */
  Bus.prototype.watchPage = function () {
    var self = this, doc = this.win.document;
    if (!doc || this.watching) return;
    this.watching = true;
    var vis = function () { if (self.ring) self.ring.setPaused(doc.visibilityState === 'hidden'); };
    doc.addEventListener('visibilitychange', vis);
    this.win.addEventListener('pagehide', function () { self.stop(); });
    vis();
  };
  Bus.prototype.stop = function () {
    if (!this.ring) return;
    this.ring.setPaused(false);
    this.push(KIND.STOP, 0, null);
  };
  /** Ends the worker now (a restore, a fresh start): the next socket starts a new one. */
  Bus.prototype.restart = function () {
    if (this.worker) this.worker.terminate();
    this.conns.forEach(function (s) { s._closed(1006, 'restarted', false); });
    this.conns.clear();
    this.worker = null; this.ring = null; this.ready = false; this.starting = null; this.failed = null; this.backlog = [];
    this.watching = this.watching || false;
  };

  Bus.prototype.status = function () {
    return { ready: this.ready, failed: this.failed, sockets: this.conns.size, backlog: this.backlog.length,
             events: Object.assign({}, this.events), stats: Object.assign({}, this.stats), logs: this.logs.slice(-20) };
  };

  // -- Choosing solo, and installing it -----------------------------------------------------------------------------

  /** Whether this page should play solo: ?solo=1, ?go=solo, or env.solo ("phone", "1"). ?solo=0 never. */
  function wanted(win) {
    var q = '';
    try { q = String(win.location.search || ''); } catch (e) { q = ''; }
    if (/[?&]solo=0\b/.test(q)) return false;
    if (/[?&]solo=1\b/.test(q) || /[?&]go=solo\b/.test(q)) return true;
    var env = win.__wenilla_env;
    return !!(env && (env.solo === 'phone' || env.solo === '1'));
  }

  /** What the browser needs: a Worker, shared memory (a cross-origin-isolated page) and Atomics. */
  function supported(win) {
    return typeof win.Worker === 'function' && typeof win.SharedArrayBuffer === 'function' && typeof win.Atomics === 'object' &&
      win.crossOriginIsolated !== false;
  }

  /**
   * Turns this page into solo play: the env signs the client in to the page's own Den Lite and seats the ready-made
   * character (no login form, no character list: webenv user/pass/char), and the client's game sockets go to it.
   * opts: {base (the solo folder's URL), engine, env (the worker's), store, onFail}.
   */
  function install(win, opts) {
    opts = opts || {};
    if (win.__wuSolo) return win.__wuSolo;
    var base = opts.base || new URL('./solo/', win.location.href).href;
    var hero = (opts.hero && /^[A-Za-z]{2,12}$/.test(opts.hero)) ? opts.hero : HERO;
    var env = win.__wenilla_env = win.__wenilla_env || {};
    // never a realm's login and never a hop here: this page's own server only
    delete env.hop; delete env.hop_ws; delete env.hop_sso; delete env.hop_home;
    env.user = ACCOUNT; env.pass = PASSWORD; env.char = hero; env.solo = env.solo || 'phone';
    var bus = new Bus(win, {
      workerUrl: base + 'den-lite-worker.js',
      engine: opts.engine || win.__wuSoloEngine || { kind: 'wasm', url: base + 'den-lite-web.wasm' },
      env: Object.assign({ DEN_LITE_PASSWORD: PASSWORD, DEN_LITE_NAME: hero, DEN_LITE_REALM: 'Solo' }, opts.workerEnv || {}),
      store: opts.store,
      onFail: function (why) { if (typeof opts.onFail === 'function') opts.onFail(why); emit(win, 'failed', { why: why }); },
      onSaved: function (paths) { emit(win, 'saved', { files: paths.length }); },
    });
    var Native = win.WebSocket;
    var Solo = makeSocketClass(win, bus);
    var host = '';
    try { host = win.location.host; } catch (e) { host = ''; }
    var Routed = function WebSocket(url, protocols) {
      var m = /^wss?:\/\/([^/?#]+)\/ws\/\d{1,5}(?:[?#].*)?$/.exec(String(url));
      if (m && (m[1] === host || !host)) return new Solo(url, protocols);
      return protocols === undefined ? new Native(url) : new Native(url, protocols);
    };
    Routed.prototype = Solo.prototype;
    ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED'].forEach(function (k, i) { Routed[k] = i; });
    Routed.__wuSolo = true;
    win.WebSocket = Routed;
    var api = {
      bus: bus,
      Socket: Solo,
      hero: hero,
      start: function () { return bus.start(); },
      status: function () { return bus.status(); },
      /** A save copy of this browser's solo world, as a file to keep (JSON). */
      exportSave: function () {
        return bus.store.load().then(function (files) { return JSON.stringify(packSave(files)); });
      },
      /** Opens a save copy: this browser's solo world is replaced (the page reloads into it). */
      importSave: function (text) {
        var files = unpackSave(text);
        bus.restart();
        return bus.store.replace(files).then(function () { return Object.keys(files).length; });
      },
      /** A fresh world: this browser's solo saves are gone. */
      reset: function () { bus.restart(); return bus.store.replace({}); },
    };
    win.__wuSolo = api;
    emit(win, 'installed', { hero: hero });
    // a fast start: Den Lite opens its world while the client downloads
    if (opts.eager !== false) bus.start().then(function () { emit(win, 'ready', bus.status().events); }, function () {});
    return api;
  }

  /** `leyline:solo` events on window (the touch layer and the page may listen; nothing depends on them). */
  function emit(win, what, detail) {
    try {
      if (typeof win.CustomEvent === 'function' && win.dispatchEvent) win.dispatchEvent(new win.CustomEvent('leyline:solo', { detail: Object.assign({ what: what }, detail || {}) }));
    } catch (e) { /* optional */ }
  }

  return { install: install, wanted: wanted, supported: supported, Bus: Bus, Store: Store, makeSocketClass: makeSocketClass,
           packSave: packSave, unpackSave: unpackSave, KIND: KIND, ACCOUNT: ACCOUNT, PASSWORD: PASSWORD, HERO: HERO };
});
