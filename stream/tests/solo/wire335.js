// wire335.js — the 3.3.5a (12340) login and world handshakes in JavaScript, both sides, for the solo tests
// (docs/PHONE-SOLO.md): a test client that does what the browser client does on its sockets (SRP6 login, the realm
// list, the world's auth session with its header encryption, the character list, entering the world), and the small
// stand-in server (stub-engine.js) the page test runs when the Den Lite wasm isn't built here.
//
// Written from the public protocol notes (the wowdev wiki's SRP6 and packet pages) and checked against what Den Lite
// does (labs/den-lite: wow_srp, wow_login_messages, wow_world_messages; AzerothCore's AuthSession / WorldSocket as the
// reference only). Synchronous SHA-1 so it also runs in a worker. UMD: node (require), workers (importScripts), pages.
(function (root, factory) {
  'use strict';
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.Wire335 = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  // -- SHA-1, HMAC-SHA1, RC4 --------------------------------------------------------------------------------------
  function sha1() {
    var parts = Array.prototype.slice.call(arguments), len = 0, i;
    parts = parts.map(function (p) { return typeof p === 'string' ? new TextEncoder().encode(p) : p; });
    parts.forEach(function (p) { len += p.length; });
    var total = ((len + 9 + 63) >> 6) << 6, m = new Uint8Array(total), off = 0;
    parts.forEach(function (p) { m.set(p, off); off += p.length; });
    m[len] = 0x80;
    var dv = new DataView(m.buffer);
    dv.setUint32(total - 4, (len * 8) >>> 0);
    dv.setUint32(total - 8, Math.floor(len / 0x20000000));
    var h0 = 0x67452301, h1 = 0xEFCDAB89, h2 = 0x98BADCFE, h3 = 0x10325476, h4 = 0xC3D2E1F0, w = new Int32Array(80);
    for (var b = 0; b < total; b += 64) {
      for (i = 0; i < 16; i++) w[i] = dv.getInt32(b + i * 4);
      for (i = 16; i < 80; i++) { var x = w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]; w[i] = (x << 1) | (x >>> 31); }
      var a = h0, bb = h1, c = h2, d = h3, e = h4;
      for (i = 0; i < 80; i++) {
        var f, k;
        if (i < 20) { f = (bb & c) | (~bb & d); k = 0x5A827999; }
        else if (i < 40) { f = bb ^ c ^ d; k = 0x6ED9EBA1; }
        else if (i < 60) { f = (bb & c) | (bb & d) | (c & d); k = 0x8F1BBCDC; }
        else { f = bb ^ c ^ d; k = 0xCA62C1D6; }
        var t = (((a << 5) | (a >>> 27)) + f + e + k + w[i]) | 0;
        e = d; d = c; c = (bb << 30) | (bb >>> 2); bb = a; a = t;
      }
      h0 = (h0 + a) | 0; h1 = (h1 + bb) | 0; h2 = (h2 + c) | 0; h3 = (h3 + d) | 0; h4 = (h4 + e) | 0;
    }
    var out = new Uint8Array(20), ov = new DataView(out.buffer);
    [h0, h1, h2, h3, h4].forEach(function (h, j) { ov.setInt32(j * 4, h); });
    return out;
  }
  function hmacSha1(key, msg) {
    if (key.length > 64) key = sha1(key);
    var k = new Uint8Array(64); k.set(key);
    var ip = k.map(function (x) { return x ^ 0x36; }), op = k.map(function (x) { return x ^ 0x5c; });
    return sha1(op, sha1(ip, msg));
  }
  function Rc4(key, drop) {
    var s = new Uint8Array(256), i, j = 0, t;
    for (i = 0; i < 256; i++) s[i] = i;
    for (i = 0; i < 256; i++) { j = (j + s[i] + key[i % key.length]) & 255; t = s[i]; s[i] = s[j]; s[j] = t; }
    this.s = s; this.i = 0; this.j = 0;
    this.apply(new Uint8Array(drop || 0));
  }
  Rc4.prototype.apply = function (data) {
    var s = this.s, i = this.i, j = this.j, t;
    for (var n = 0; n < data.length; n++) {
      i = (i + 1) & 255; j = (j + s[i]) & 255; t = s[i]; s[i] = s[j]; s[j] = t;
      data[n] ^= s[(s[i] + s[j]) & 255];
    }
    this.i = i; this.j = j;
    return data;
  };

  // -- Numbers, bytes -----------------------------------------------------------------------------------------------
  function leToBig(u8) { var n = 0n; for (var i = u8.length - 1; i >= 0; i--) n = (n << 8n) | BigInt(u8[i]); return n; }
  function bigToLe(n, len) { var out = new Uint8Array(len); for (var i = 0; i < len; i++) { out[i] = Number(n & 255n); n >>= 8n; } return out; }
  function modpow(b, e, m) { var r = 1n; b %= m; while (e > 0n) { if (e & 1n) r = (r * b) % m; b = (b * b) % m; e >>= 1n; } return r; }
  function cat() {
    var parts = Array.prototype.slice.call(arguments), n = 0, o = 0;
    parts.forEach(function (p) { n += p.length; });
    var out = new Uint8Array(n);
    parts.forEach(function (p) { out.set(p, o); o += p.length; });
    return out;
  }
  function rand(n) { var b = new Uint8Array(n); crypto.getRandomValues(b); return b; }
  function u16le(v) { return new Uint8Array([v & 255, (v >> 8) & 255]); }
  function u32le(v) { return new Uint8Array([v & 255, (v >>> 8) & 255, (v >>> 16) & 255, (v >>> 24) & 255]); }
  function rd32(b, o) { return (b[o] | (b[o + 1] << 8) | (b[o + 2] << 16) | (b[o + 3] << 24)) >>> 0; }

  // -- SRP6 as the game does it -------------------------------------------------------------------------------------
  var N_HEX = '894B645E89E1535BBDAD5B8B290650530801B18EBFBF5E8FAB3C82872A3E9BB7';
  var N = BigInt('0x' + N_HEX), G = 7n, K3 = 3n;
  var N_LE = bigToLe(N, 32);
  function xOf(user, pass, salt) {
    return leToBig(sha1(salt, sha1(user.toUpperCase() + ':' + pass.toUpperCase())));
  }
  function interleave(S) {
    var s = bigToLe(S, 32), p = 0;
    while (p < 32 && s[p] === 0) p++;
    if (p & 1) p++;
    p /= 2;
    var half = 16 - p, e = new Uint8Array(half), o = new Uint8Array(half);
    for (var i = 0; i < half; i++) { e[i] = s[2 * (i + p)]; o[i] = s[2 * (i + p) + 1]; }
    var he = sha1(e), ho = sha1(o), k = new Uint8Array(40);
    for (i = 0; i < 20; i++) { k[2 * i] = he[i]; k[2 * i + 1] = ho[i]; }
    return k;
  }
  function m1Of(user, salt, A, B, K) {
    var hn = sha1(N_LE), hg = sha1(new Uint8Array([7])), x = new Uint8Array(20);
    for (var i = 0; i < 20; i++) x[i] = hn[i] ^ hg[i];
    return sha1(x, sha1(user.toUpperCase()), salt, A, B, K);
  }
  /** The client's side: {A, M1, K, M2} from the server's B and salt. */
  function srpClient(user, pass, Bbytes, salt) {
    var a = leToBig(rand(19)), A = modpow(G, a, N), B = leToBig(Bbytes);
    var Ab = bigToLe(A, 32), u = leToBig(sha1(Ab, Bbytes)), x = xOf(user, pass, salt);
    var base = ((B - K3 * modpow(G, x, N)) % N + N) % N;
    var S = modpow(base, a + u * x, N), K = interleave(S), M1 = m1Of(user, salt, Ab, Bbytes, K);
    return { A: Ab, M1: M1, K: K, M2: sha1(Ab, M1, K) };
  }
  /** The server's side: {salt, B}, then check(A, M1) -> {K, M2} or null. */
  function srpServer(user, pass) {
    var salt = rand(32), v = modpow(G, xOf(user, pass, salt), N), b = leToBig(rand(19));
    var B = (K3 * v + modpow(G, b, N)) % N, Bb = bigToLe(B, 32);
    return {
      salt: salt, B: Bb,
      check: function (Ab, M1) {
        var A = leToBig(Ab);
        if (A % N === 0n) return null;
        var u = leToBig(sha1(Ab, Bb)), S = modpow(A * modpow(v, u, N), b, N), K = interleave(S);
        var want = m1Of(user, salt, Ab, Bb, K);
        for (var i = 0; i < 20; i++) if (want[i] !== M1[i]) return null;
        return { K: K, M2: sha1(Ab, M1, K) };
      },
    };
  }

  // -- The world's header encryption ------------------------------------------------------------------------------
  var S_ENC = new Uint8Array([0xC2, 0xB3, 0x72, 0x3C, 0xC6, 0xAE, 0xD9, 0xB5, 0x34, 0x3C, 0x53, 0xEE, 0x2F, 0x43, 0x67, 0xCE]);
  var S_DEC = new Uint8Array([0xCC, 0x98, 0xAE, 0x04, 0xE8, 0x97, 0xEA, 0xCA, 0x12, 0xDD, 0xC0, 0x93, 0x42, 0x91, 0x53, 0x57]);
  /** The two RC4 streams of a session: `side` 'client' or 'server'. {out, in} each an Rc4. */
  function headerCrypto(K, side) {
    var toClient = new Rc4(hmacSha1(S_ENC, K), 1024), toServer = new Rc4(hmacSha1(S_DEC, K), 1024);
    return side === 'client' ? { out: toServer, in: toClient } : { out: toClient, in: toServer };
  }

  // -- A byte stream over message events (what the client does with its socket) ----------------------------------
  function Reader() { this.buf = new Uint8Array(0); this.waiters = []; this.closed = false; }
  Reader.prototype.push = function (u8) { this.buf = cat(this.buf, u8); this.wake(); };
  Reader.prototype.end = function () { this.closed = true; this.wake(); };
  Reader.prototype.wake = function () {
    var w = this.waiters; this.waiters = [];
    w.forEach(function (f) { f(); });
  };
  Reader.prototype.read = function (n, ms) {
    var self = this;
    return new Promise(function (ok, fail) {
      var timer = setTimeout(function () { fail(new Error('timed out waiting for ' + n + ' bytes')); }, ms || 8000);
      var go = function () {
        if (self.buf.length >= n) { clearTimeout(timer); var out = self.buf.slice(0, n); self.buf = self.buf.slice(n); ok(out); }
        else if (self.closed) { clearTimeout(timer); fail(new Error('the socket closed')); }
        else self.waiters.push(go);
      };
      go();
    });
  };

  function socketStream(ws) {
    var r = new Reader();
    ws.binaryType = 'arraybuffer';
    ws.onmessage = function (e) { r.push(new Uint8Array(e.data)); };
    ws.onclose = function () { r.end(); };
    var opened = new Promise(function (ok, fail) { ws.onopen = function () { ok(); }; ws.onerror = function () { fail(new Error('socket error')); }; });
    return { opened: opened, read: function (n, ms) { return r.read(n, ms); }, write: function (u8) { ws.send(u8); }, close: function () { ws.close(); }, reader: r };
  }

  // -- Login (port 3724) --------------------------------------------------------------------------------------------
  function challengeMessage(user) {
    var name = new TextEncoder().encode(user.toUpperCase());
    var body = cat(new Uint8Array([0x57, 0x6f, 0x57, 0x00, 3, 3, 5]), u16le(12340), new Uint8Array([0x36, 0x38, 0x78, 0x00]),
      new Uint8Array([0x6e, 0x69, 0x57, 0x00]), new Uint8Array([0x53, 0x55, 0x6e, 0x65]), u32le(0), new Uint8Array([127, 0, 0, 1]),
      new Uint8Array([name.length]), name);
    return cat(new Uint8Array([0x00, 8]), u16le(body.length), body);
  }
  function readCString(b, o) { var e = o; while (b[e] !== 0) e++; return [new TextDecoder().decode(b.subarray(o, e)), e + 1]; }

  /** The whole login: {K, realms: [{name, address}]}. `s` is a socketStream. */
  async function login(s, user, pass) {
    s.write(challengeMessage(user));
    var h = await s.read(3);
    if (h[0] !== 0 || h[2] !== 0) throw new Error('login challenge refused (' + h[2] + ')');
    var B = await s.read(32), gl = (await s.read(1))[0];
    await s.read(gl);
    var nl = (await s.read(1))[0];
    await s.read(nl);
    var salt = await s.read(32);
    await s.read(16);
    var flags = (await s.read(1))[0];
    if (flags) throw new Error('security flags not handled: ' + flags);
    var c = srpClient(user, pass, B, salt);
    s.write(cat(new Uint8Array([0x01]), c.A, c.M1, new Uint8Array(20), new Uint8Array([0, 0])));
    var p = await s.read(2);
    if (p[0] !== 1 || p[1] !== 0) throw new Error('login proof refused (' + p[1] + ')');
    var rest = await s.read(20 + 4 + 4 + 2), M2 = rest.subarray(0, 20);
    for (var i = 0; i < 20; i++) if (M2[i] !== c.M2[i]) throw new Error('the server proof is wrong');
    s.write(cat(new Uint8Array([0x10]), u32le(0)));
    var rh = await s.read(3);
    if (rh[0] !== 0x10) throw new Error('expected the realm list, got ' + rh[0]);
    var body = await s.read(rh[1] | (rh[2] << 8)), count = body[4] | (body[5] << 8), o = 6, realms = [];
    for (var r = 0; r < count; r++) {
      o += 3;
      var nm = readCString(body, o); o = nm[1];
      var ad = readCString(body, o); o = ad[1];
      o += 4 + 3;
      realms.push({ name: nm[0], address: ad[0] });
    }
    return { K: c.K, realms: realms };
  }

  // -- World (any other port) ---------------------------------------------------------------------------------------
  var OP = { SMSG_AUTH_CHALLENGE: 0x1EC, CMSG_AUTH_SESSION: 0x1ED, SMSG_AUTH_RESPONSE: 0x1EE, CMSG_CHAR_ENUM: 0x037, SMSG_CHAR_ENUM: 0x03B,
             CMSG_PLAYER_LOGIN: 0x03D, SMSG_LOGIN_VERIFY_WORLD: 0x236, SMSG_UPDATE_OBJECT: 0x0A9, SMSG_COMPRESSED_UPDATE_OBJECT: 0x1F6,
             CMSG_PING: 0x1DC, SMSG_PONG: 0x1DD, SMSG_ADDON_INFO: 0x2EF, SMSG_TIME_SYNC_REQ: 0x390 };

  /** A world connection as the client: frames in and out, with the header encryption once `crypt` is set. */
  function WorldClient(s) { this.s = s; this.crypt = null; }
  WorldClient.prototype.recv = async function () {
    var h = await this.s.read(4);
    if (this.crypt) this.crypt.in.apply(h);
    var size;
    if (h[0] & 0x80) {
      var more = await this.s.read(1);
      if (this.crypt) this.crypt.in.apply(more);
      size = ((h[0] & 0x7f) << 16) | (h[1] << 8) | h[2];
      var op = h[3] | (more[0] << 8);
      return { op: op, body: await this.s.read(size - 2) };
    }
    size = (h[0] << 8) | h[1];
    return { op: h[2] | (h[3] << 8), body: size > 2 ? await this.s.read(size - 2) : new Uint8Array(0) };
  };
  WorldClient.prototype.send = function (op, body) {
    body = body || new Uint8Array(0);
    var size = body.length + 4, h = new Uint8Array([size >> 8, size & 255, op & 255, (op >> 8) & 255, 0, 0]);
    if (this.crypt) this.crypt.out.apply(h);
    this.s.write(cat(h, body));
  };
  /** Waits for one of `ops` (skipping the rest, kept in `seen`). */
  WorldClient.prototype.until = async function (ops, seen) {
    for (;;) {
      var m = await this.recv();
      if (seen) seen.push(m.op);
      if (ops.indexOf(m.op) >= 0) return m;
    }
  };

  /** The world's handshake and entering the world: {guid, verify: {map, x, y, z}, seen: [opcodes]}. */
  async function enterWorld(s, user, K) {
    var w = new WorldClient(s), seen = [];
    var ch = await w.recv();
    if (ch.op !== OP.SMSG_AUTH_CHALLENGE) throw new Error('expected the auth challenge, got ' + ch.op.toString(16));
    var serverSeed = ch.body.subarray(4, 8), clientSeed = rand(4), name = new TextEncoder().encode(user.toUpperCase());
    var proof = sha1(name, u32le(0), clientSeed, serverSeed, K);
    w.send(OP.CMSG_AUTH_SESSION, cat(u32le(12340), u32le(0), name, new Uint8Array([0]), u32le(0), clientSeed, u32le(0), u32le(0), u32le(1),
      new Uint8Array(8), proof, u32le(0)));
    w.crypt = headerCrypto(K, 'client');
    var ar = await w.until([OP.SMSG_AUTH_RESPONSE], seen);
    if (ar.body[0] !== 0x0C) throw new Error('auth response ' + ar.body[0]);
    w.send(OP.CMSG_CHAR_ENUM);
    var ce = await w.until([OP.SMSG_CHAR_ENUM], seen);
    if (!ce.body[0]) throw new Error('no character');
    var guidBytes = ce.body.subarray(1, 9), name2 = readCString(ce.body, 9)[0];
    w.send(OP.CMSG_PLAYER_LOGIN, guidBytes);
    var v = await w.until([OP.SMSG_LOGIN_VERIFY_WORLD], seen), dv = new DataView(v.body.buffer, v.body.byteOffset);
    var verify = { map: dv.getUint32(0, true), x: dv.getFloat32(4, true), y: dv.getFloat32(8, true), z: dv.getFloat32(12, true) };
    var upd = await w.until([OP.SMSG_UPDATE_OBJECT, OP.SMSG_COMPRESSED_UPDATE_OBJECT], seen);
    w.send(OP.CMSG_PING, cat(u32le(7), u32le(20)));
    await w.until([OP.SMSG_PONG], seen);
    return { guid: leToBig(guidBytes), name: name2, verify: verify, update: upd.op, seen: seen, world: w };
  }

  return { sha1: sha1, hmacSha1: hmacSha1, Rc4: Rc4, leToBig: leToBig, bigToLe: bigToLe, modpow: modpow, cat: cat, u16le: u16le, u32le: u32le, rd32: rd32,
           srpClient: srpClient, srpServer: srpServer, headerCrypto: headerCrypto, Reader: Reader, socketStream: socketStream,
           login: login, enterWorld: enterWorld, challengeMessage: challengeMessage, readCString: readCString, OP: OP, N_LE: N_LE, rand: rand };
});
