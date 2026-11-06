// stub-engine.js — a stand-in for Den Lite's wasm in the solo tests (docs/PHONE-SOLO.md), when the wasm isn't built
// in this checkout: the same engine contract den-lite-worker.js gives Den Lite (records from the page's ring, bytes
// back per socket), with the smallest server that lets the client in: SRP6 login on 3724, a realm list of one realm
// at 127.0.0.1:8085, and on the world port the auth session (header encryption on), one character, entering the world
// and ping. It also keeps a file in the Dens folder (each time in the world), so the page's saves are tested too.
// Not Den Lite: nothing here is played. CI runs the same page test against the real wasm (den-lite-web.yml).
/* global importScripts, Wire335 */
'use strict';
// next to this file (a worker resolves importScripts against its own address, so the engine's is made absolute)
importScripts(self.__engineUrl && self.location ? new URL('./wire335.js', new URL(self.__engineUrl, self.location.href)).href : './wire335.js');

self.LeylineSoloEngine = {
  run: function (ly) {
    var W = Wire335, keys = new Map(), conns = new Map();
    var user = 'SOLO', pass = (ly.env && ly.env.DEN_LITE_PASSWORD) || 'den', hero = (ly.env && ly.env.DEN_LITE_NAME) || 'Wren';
    var VISITS = '/dens/stub-visits.json';
    var sendWorld = function (c, op, body) {
      body = body || new Uint8Array(0);
      var size = body.length + 2, h = new Uint8Array([size >> 8, size & 255, op & 255, op >> 8]);
      if (c.crypt) c.crypt.out.apply(h);
      ly.send(c.id, W.cat(h, body));
    };
    var login = function (c) {
      for (;;) {
        var b = c.buf;
        if (c.state === 'challenge') {
          if (b.length < 4) return;
          var size = b[2] | (b[3] << 8);
          if (b.length < 4 + size) return;
          c.buf = b.slice(4 + size);
          c.srp = W.srpServer(user, pass);
          ly.send(c.id, W.cat(new Uint8Array([0, 0, 0]), c.srp.B, new Uint8Array([1, 7, 32]), W.N_LE, c.srp.salt, new Uint8Array(16), new Uint8Array([0])));
          c.state = 'proof';
        } else if (c.state === 'proof') {
          if (b.length < 75) return;
          c.buf = b.slice(75);
          var ok = c.srp.check(b.subarray(1, 33), b.subarray(33, 53));
          if (!ok) { ly.send(c.id, new Uint8Array([1, 4, 3, 0])); ly.close(c.id); c.state = 'done'; return; }
          keys.set(user, ok.K);
          ly.send(c.id, W.cat(new Uint8Array([1, 0]), ok.M2, W.u32le(0), W.u32le(0), W.u16le(0)));
          c.state = 'realms';
        } else if (c.state === 'realms') {
          if (b.length < 5) return;
          c.buf = b.slice(5);
          var enc = new TextEncoder(), realm = W.cat(new Uint8Array([1, 0, 0]), enc.encode('Solo\0'), enc.encode('127.0.0.1:8085\0'),
            new Uint8Array(new Float32Array([0]).buffer), new Uint8Array([1, 1, 1]));
          var body = W.cat(W.u32le(0), W.u16le(1), realm, new Uint8Array([0x10, 0]));
          ly.send(c.id, W.cat(new Uint8Array([0x10]), W.u16le(body.length), body));
        } else return;
      }
    };
    var world = function (c) {
      for (;;) {
        var b = c.buf;
        if (b.length < 6) return;
        var h = b.slice(0, 6);
        if (c.crypt && !c.pending) { c.crypt.in.apply(h); c.pending = h; }
        if (c.pending) h = c.pending;
        var size = (h[0] << 8) | h[1], op = h[2] | (h[3] << 8);
        if (b.length < 2 + size) return;
        var body = b.slice(6, 2 + size);
        c.buf = b.slice(2 + size);
        c.pending = null;
        if (op === W.OP.CMSG_AUTH_SESSION) {
          var nm = W.readCString(body, 8), name = nm[0], o = nm[1] + 4, clientSeed = body.subarray(o, o + 4);
          var proof = body.subarray(o + 4 + 12 + 8, o + 4 + 12 + 8 + 20), K = keys.get(name);
          var want = K && W.sha1(new TextEncoder().encode(name), W.u32le(0), clientSeed, c.seed, K);
          if (!want || want.some(function (x, i) { return x !== proof[i]; })) { sendWorld(c, W.OP.SMSG_AUTH_RESPONSE, new Uint8Array([0x15])); ly.close(c.id); return; }
          // as Den Lite: the encryption is on from the auth response itself
          c.crypt = W.headerCrypto(K, 'server');
          sendWorld(c, W.OP.SMSG_AUTH_RESPONSE, W.cat(new Uint8Array([0x0C]), W.u32le(0), new Uint8Array([0]), W.u32le(0), new Uint8Array([2])));
        } else if (op === W.OP.CMSG_CHAR_ENUM) {
          sendWorld(c, W.OP.SMSG_CHAR_ENUM, W.cat(new Uint8Array([1]), W.bigToLe(1n, 8), new TextEncoder().encode(hero + '\0'), new Uint8Array(40)));
        } else if (op === W.OP.CMSG_PLAYER_LOGIN) {
          var v = new DataView(new ArrayBuffer(20));
          v.setUint32(0, 1, true); v.setFloat32(4, 16226.2, true); v.setFloat32(8, 16257.0, true); v.setFloat32(12, 13.25, true); v.setFloat32(16, 1.57, true);
          sendWorld(c, W.OP.SMSG_LOGIN_VERIFY_WORLD, new Uint8Array(v.buffer));
          sendWorld(c, W.OP.SMSG_UPDATE_OBJECT, W.u32le(0));
          var visits = 0;
          try { visits = JSON.parse(new TextDecoder().decode(ly.fs.readFile(VISITS) || new Uint8Array(0)) || '{}').visits || 0; } catch (e) { visits = 0; }
          ly.fs.writeFile(VISITS, new TextEncoder().encode(JSON.stringify({ visits: visits + 1 })));
          ly.changed(VISITS);
        } else if (op === W.OP.CMSG_PING) {
          sendWorld(c, W.OP.SMSG_PONG, body.subarray(0, 4));
        }
      }
    };
    for (;;) {
      var r = ly.recv(1000);
      if (!r) continue;
      if (r.kind === 4) return 0;
      var c = conns.get(r.conn);
      if (r.kind === 1) {
        var port = r.data[0] | (r.data[1] << 8);
        c = { id: r.conn, port: port, buf: new Uint8Array(0), state: 'challenge', crypt: null, pending: null };
        conns.set(r.conn, c);
        if (port !== 3724) {
          c.seed = W.rand(4);
          sendWorld(c, W.OP.SMSG_AUTH_CHALLENGE, W.cat(W.u32le(1), c.seed, W.rand(32)));
        }
      } else if (r.kind === 2 && c) {
        c.buf = W.cat(c.buf, r.data);
        if (c.port === 3724) login(c); else world(c);
      } else if (r.kind === 3) {
        conns.delete(r.conn);
      }
    }
  },
};
