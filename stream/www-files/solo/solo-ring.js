// solo-ring.js — the byte ring between the page and its own Den Lite (docs/PHONE-SOLO.md). One writer (the page:
// solo.js, the client's sockets), one reader (the worker: den-lite-worker.js, Den Lite's ly_recv). The worker blocks
// on it with Atomics.wait, which is why it is shared memory and not postMessage: a worker running Den Lite's loop never
// returns to its event loop, so messages could not reach it. The other way (Den Lite -> the client) is postMessage.
//
// Layout (one SharedArrayBuffer): 32 bytes of Int32 control, then the ring.
//   ctl[0] seq       bumped (and notified) on every write, on pause/resume and on stop
//   ctl[1] head      bytes written, ever (wrapping u32)
//   ctl[2] tail      bytes read, ever (wrapping u32)
//   ctl[3] paused    1: the page is hidden; the worker waits without beats until it is 0 again
//   ctl[4] drain     1: the page has records waiting for room; the reader says "drain" when it took some
// A record: u32 length (little endian), then that many bytes: kind u8, connection u32, the rest.
// Records never split: a writer with no room keeps the record (solo.js's backlog) until the reader drained.
(function (root, factory) {
  'use strict';
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.LeylineSoloRing = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';
  var CTL = 32;
  var SEQ = 0, HEAD = 1, TAIL = 2, PAUSED = 3, DRAIN = 4;
  var KIND = { OPEN: 1, DATA: 2, CLOSE: 3, STOP: 4 };

  function Ring(sab) {
    this.sab = sab;
    this.ctl = new Int32Array(sab, 0, 8);
    this.data = new Uint8Array(sab, CTL);
    this.size = this.data.length;
  }

  /** A new ring with room for `bytes` (rounded up to a whole page). */
  Ring.create = function (bytes) {
    var n = Math.max(4096, Math.ceil((bytes || 1 << 20) / 4096) * 4096);
    return new Ring(new SharedArrayBuffer(CTL + n));
  };

  Ring.prototype.used = function () {
    return (Atomics.load(this.ctl, HEAD) - Atomics.load(this.ctl, TAIL)) >>> 0;
  };

  Ring.prototype._put = function (at, bytes) {
    var i = at % this.size, first = Math.min(bytes.length, this.size - i);
    this.data.set(bytes.subarray(0, first), i);
    if (first < bytes.length) this.data.set(bytes.subarray(first), 0);
  };

  Ring.prototype._get = function (at, into, len) {
    var i = at % this.size, first = Math.min(len, this.size - i);
    into.set(this.data.subarray(i, i + first), 0);
    if (first < len) into.set(this.data.subarray(0, len - first), first);
  };

  Ring.prototype.bump = function () {
    Atomics.add(this.ctl, SEQ, 1);
    Atomics.notify(this.ctl, SEQ);
  };

  /** Writes one record; false (nothing written, the drain flag set) when there is no room for it now. */
  Ring.prototype.push = function (kind, conn, payload) {
    var body = payload ? (payload instanceof Uint8Array ? payload : new Uint8Array(payload)) : new Uint8Array(0);
    var len = 5 + body.length, need = 4 + len;
    if (need > this.size) throw new RangeError('record bigger than the ring');
    if (this.size - this.used() < need) {
      Atomics.store(this.ctl, DRAIN, 1);
      return false;
    }
    var head = Atomics.load(this.ctl, HEAD) >>> 0;
    var hdr = new Uint8Array(9);
    var dv = new DataView(hdr.buffer);
    dv.setUint32(0, len, true);
    hdr[4] = kind;
    dv.setUint32(5, conn >>> 0, true);
    this._put(head, hdr);
    this._put(head + 9, body);
    Atomics.store(this.ctl, HEAD, (head + need) | 0);
    this.bump();
    return true;
  };

  /** The next record's length (kind + connection + rest), or -1 when the ring is empty. */
  Ring.prototype.peek = function () {
    if (this.used() < 4) return -1;
    var b = new Uint8Array(4);
    this._get(Atomics.load(this.ctl, TAIL) >>> 0, b, 4);
    return new DataView(b.buffer).getUint32(0, true);
  };

  /** Copies the next record (without its length) into `into` at `offset` and takes it off the ring; its length.
   *  True in `this.drained` when the page asked to hear about room. */
  Ring.prototype.take = function (into, offset) {
    var len = this.peek();
    if (len < 0) return -1;
    var tail = Atomics.load(this.ctl, TAIL) >>> 0;
    this._get(tail + 4, offset ? into.subarray(offset) : into, len);
    Atomics.store(this.ctl, TAIL, (tail + 4 + len) | 0);
    this.drained = Atomics.compareExchange(this.ctl, DRAIN, 1, 0) === 1;
    return len;
  };

  /** The reader's wait: until a record comes, the ring is resumed, or `ms` passed (Atomics.wait: workers only).
   *  While paused it waits for the resume, however long, and then reports no record (so no beat is due early). */
  Ring.prototype.wait = function (ms) {
    for (;;) {
      var seq = Atomics.load(this.ctl, SEQ);
      if (Atomics.load(this.ctl, PAUSED) === 1) { Atomics.wait(this.ctl, SEQ, seq); continue; }
      if (this.peek() >= 0) return true;
      if (!(ms > 0)) return false;
      var r = Atomics.wait(this.ctl, SEQ, seq, ms);
      if (r === 'timed-out') return this.peek() >= 0;
      ms = 0;   // woken: look once more, without waiting again (the caller's clock decides the rest)
      if (Atomics.load(this.ctl, PAUSED) === 1) continue;
      return this.peek() >= 0;
    }
  };

  Ring.prototype.setPaused = function (on) {
    Atomics.store(this.ctl, PAUSED, on ? 1 : 0);
    this.bump();
  };

  Ring.prototype.paused = function () { return Atomics.load(this.ctl, PAUSED) === 1; };

  /** A record as solo.js writes it, read back (tests, the script engine): {kind, conn, data} or null. */
  Ring.prototype.next = function () {
    var len = this.peek();
    if (len < 0) return null;
    var b = new Uint8Array(len);
    this.take(b, 0);
    var dv = new DataView(b.buffer);
    return { kind: b[0], conn: dv.getUint32(1, true), data: b.subarray(5) };
  };

  Ring.KIND = KIND;
  return Ring;
});
