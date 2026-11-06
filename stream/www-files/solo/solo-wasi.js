// solo-wasi.js — the small WASI (preview 1) the page's own Den Lite runs on (docs/PHONE-SOLO.md): an in-memory file
// system with one preopened folder "/", the clocks, random bytes, sleep, stdout/stderr to the console, args and env.
// Written for Den Lite's needs (Rust std on wasm32-wasip1: files, folders, rename, metadata, read_dir), from the WASI
// preview 1 witx definitions (github.com/WebAssembly/WASI, Apache-2.0 / the W3C licence; no code taken). Anything
// it doesn't do answers ENOSYS, so std reports "unsupported" instead of the worker throwing.
//
// The file system tells its owner what changed (onChange(path, bytes) after a written file closes or is renamed in,
// onRemove(path)), so the worker can hand the Dens folder to the page, which keeps it in IndexedDB.
(function (root, factory) {
  'use strict';
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.LeylineSoloWasi = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';
  var E = { SUCCESS: 0, BADF: 8, EXIST: 20, INVAL: 28, ISDIR: 31, NOENT: 44, NOSYS: 52, NOTDIR: 54, NOTEMPTY: 55, SPIPE: 70 };
  var FT = { CHAR: 2, DIR: 3, FILE: 4 };
  var O = { CREAT: 1, DIRECTORY: 2, EXCL: 4, TRUNC: 8 };
  var RIGHT_WRITE = 64n;
  var ALL = 0xFFFFFFFFFFFFFFFFn;
  var enc = new TextEncoder(), dec = new TextDecoder();

  function norm(p) {
    var out = [];
    String(p).split('/').forEach(function (s) {
      if (!s || s === '.') return;
      if (s === '..') out.pop(); else out.push(s);
    });
    return '/' + out.join('/');
  }
  function parent(p) { var i = p.lastIndexOf('/'); return i <= 0 ? '/' : p.slice(0, i); }
  function base(p) { return p.slice(p.lastIndexOf('/') + 1); }

  /** Files (path -> {data: Uint8Array, size, mtime}) and folders (a Set of paths), all absolute. */
  function MemFS(files, opts) {
    this.files = new Map();
    this.dirs = new Set(['/']);
    this.ino = new Map();
    this.nextIno = 2;
    this.opts = opts || {};
    var self = this;
    if (files) Object.keys(files).forEach(function (p) { self.writeFile(p, files[p]); });
  }
  MemFS.prototype.mkdirp = function (p) {
    p = norm(p);
    if (p === '/') return;
    this.mkdirp(parent(p));
    this.dirs.add(p);
  };
  MemFS.prototype.writeFile = function (p, bytes) {
    p = norm(p);
    this.mkdirp(parent(p));
    var b = bytes instanceof Uint8Array ? bytes.slice() : new Uint8Array(bytes);
    this.files.set(p, { data: b, size: b.length, mtime: Date.now() });
  };
  MemFS.prototype.readFile = function (p) {
    var f = this.files.get(norm(p));
    return f ? f.data.slice(0, f.size) : null;
  };
  MemFS.prototype.inode = function (p) {
    if (!this.ino.has(p)) this.ino.set(p, this.nextIno++);
    return this.ino.get(p);
  };
  MemFS.prototype.children = function (dir) {
    var pre = dir === '/' ? '/' : dir + '/', out = [];
    var add = function (p, type) { if (p.indexOf(pre) === 0 && p.length > pre.length && p.slice(pre.length).indexOf('/') < 0) out.push({ name: p.slice(pre.length), type: type, path: p }); };
    this.dirs.forEach(function (p) { add(p, FT.DIR); });
    this.files.forEach(function (_, p) { add(p, FT.FILE); });
    out.sort(function (a, b) { return a.name < b.name ? -1 : a.name > b.name ? 1 : 0; });
    return out;
  };
  /** Every file under `prefix`, as {path: bytes copy}. */
  MemFS.prototype.snapshot = function (prefix) {
    var out = {};
    this.files.forEach(function (f, p) { if (!prefix || p.indexOf(prefix) === 0) out[p] = f.data.slice(0, f.size); });
    return out;
  };

  function ExitError(code) { this.code = code; this.message = 'exit ' + code; }

  /**
   * The imports for "wasi_snapshot_preview1". opts: {fs, args, env, memory() -> WebAssembly.Memory, onChange,
   * onRemove, log(stream, line), sleep(ms)}.
   */
  function Wasi(opts) {
    this.fs = opts.fs || new MemFS();
    this.args = opts.args || ['den-lite-web'];
    this.env = opts.env || {};
    this.memory = opts.memory || function () { return null; };
    this.onChange = opts.onChange || function () {};
    this.onRemove = opts.onRemove || function () {};
    this.log = opts.log || function (fd, line) { (fd === 2 ? console.warn : console.log)(line); };
    var sleepCell = typeof SharedArrayBuffer !== 'undefined' ? new Int32Array(new SharedArrayBuffer(4)) : null;
    this.sleep = opts.sleep || function (ms) {
      if (sleepCell && ms > 0) { try { Atomics.wait(sleepCell, 0, 0, ms); return; } catch (e) { /* main thread */ } }
      var end = Date.now() + ms; while (Date.now() < end) { /* only where Atomics.wait isn't allowed */ }
    };
    this.fds = new Map();
    this.fds.set(0, { kind: 'std' }); this.fds.set(1, { kind: 'std' }); this.fds.set(2, { kind: 'std' });
    this.fds.set(3, { kind: 'dir', path: '/', pre: '/' });
    this.nextFd = 4;
    this.lines = { 1: '', 2: '' };
    this.imports = this.build();
  }

  Wasi.prototype.view = function () { return new DataView(this.memory().buffer); };
  Wasi.prototype.bytes = function () { return new Uint8Array(this.memory().buffer); };
  Wasi.prototype.str = function (ptr, len) { return dec.decode(this.bytes().slice(ptr, ptr + len)); };
  Wasi.prototype.at = function (fd, ptr, len) {
    var d = this.fds.get(fd);
    if (!d || d.kind !== 'dir') return null;
    var rel = this.str(ptr, len);
    return norm(rel.charAt(0) === '/' ? rel : d.path + '/' + rel);
  };
  Wasi.prototype.stat = function (path, ptr) {
    var v = this.view(), f = this.fs.files.get(path), isDir = this.fs.dirs.has(path);
    if (!f && !isDir) return E.NOENT;
    for (var i = 0; i < 64; i++) v.setUint8(ptr + i, 0);
    v.setBigUint64(ptr, 1n, true);
    v.setBigUint64(ptr + 8, BigInt(this.fs.inode(path)), true);
    v.setUint8(ptr + 16, isDir ? FT.DIR : FT.FILE);
    v.setBigUint64(ptr + 24, 1n, true);
    v.setBigUint64(ptr + 32, BigInt(f ? f.size : 0), true);
    var t = BigInt(f ? f.mtime : Date.now()) * 1000000n;
    v.setBigUint64(ptr + 40, t, true); v.setBigUint64(ptr + 48, t, true); v.setBigUint64(ptr + 56, t, true);
    return E.SUCCESS;
  };
  Wasi.prototype.flushLine = function (fd, text) {
    var s = this.lines[fd] + text, i;
    while ((i = s.indexOf('\n')) >= 0) { this.log(fd, s.slice(0, i)); s = s.slice(i + 1); }
    this.lines[fd] = s.length > 4096 ? (this.log(fd, s), '') : s;
  };
  /** Room for `size` bytes in an open file's buffer. */
  function grow(f, size) {
    if (size <= f.data.length) return;
    var n = new Uint8Array(Math.max(size, f.data.length * 2, 256));
    n.set(f.data.subarray(0, f.size));
    f.data = n;
  }
  Wasi.prototype.write = function (d, chunks, at) {
    var f = this.fs.files.get(d.path);
    if (!f) return -E.BADF;
    var pos = at === undefined ? (d.append ? f.size : d.pos) : at, n = 0;
    chunks.forEach(function (c) {
      grow(f, pos + c.length);
      f.data.set(c, pos);
      pos += c.length; n += c.length;
      if (pos > f.size) f.size = pos;
    });
    if (at === undefined) d.pos = pos;
    f.mtime = Date.now();
    d.dirty = true;
    var cap = this.fs.opts.cap && this.fs.opts.cap(d.path);
    if (cap && f.size > cap) {   // a log that only grows (the events): its older half goes
      var keep = f.data.slice(f.size - Math.floor(cap / 2), f.size);
      f.data = keep; f.size = keep.length; d.pos = f.size;
    }
    return n;
  };
  Wasi.prototype.iovs = function (ptr, len) {
    var v = this.view(), out = [];
    for (var i = 0; i < len; i++) out.push({ buf: v.getUint32(ptr + i * 8, true), len: v.getUint32(ptr + i * 8 + 4, true) });
    return out;
  };
  Wasi.prototype.changed = function (path) {
    var b = this.fs.readFile(path);
    if (b) this.onChange(path, b);
  };

  Wasi.prototype.build = function () {
    var w = this;
    var putStrings = function (list, ptrs, buf) {
      var v = w.view(), m = w.bytes();
      list.forEach(function (s) {
        var b = enc.encode(s + '\0');
        v.setUint32(ptrs, buf, true); ptrs += 4;
        m.set(b, buf); buf += b.length;
      });
      return E.SUCCESS;
    };
    var sizes = function (list, countPtr, sizePtr) {
      var v = w.view();
      v.setUint32(countPtr, list.length, true);
      v.setUint32(sizePtr, list.reduce(function (a, s) { return a + enc.encode(s).length + 1; }, 0), true);
      return E.SUCCESS;
    };
    var envList = function () { return Object.keys(w.env).map(function (k) { return k + '=' + w.env[k]; }); };
    var fns = {
      args_sizes_get: function (c, s) { return sizes(w.args, c, s); },
      args_get: function (p, b) { return putStrings(w.args, p, b); },
      environ_sizes_get: function (c, s) { return sizes(envList(), c, s); },
      environ_get: function (p, b) { return putStrings(envList(), p, b); },
      clock_res_get: function (id, ptr) { w.view().setBigUint64(ptr, 1000n, true); return E.SUCCESS; },
      clock_time_get: function (id, precision, ptr) {
        var ns = id === 0 ? BigInt(Date.now()) * 1000000n
          : BigInt(Math.round((typeof performance !== 'undefined' ? performance.now() : Date.now()) * 1e6));
        w.view().setBigUint64(ptr, ns, true);
        return E.SUCCESS;
      },
      random_get: function (ptr, len) {
        var m = w.bytes();
        for (var i = 0; i < len; i += 65536) crypto.getRandomValues(m.subarray(ptr + i, ptr + Math.min(len, i + 65536)));
        return E.SUCCESS;
      },
      sched_yield: function () { return E.SUCCESS; },
      proc_exit: function (code) { throw new ExitError(code); },
      proc_raise: function () { return E.NOSYS; },
      poll_oneoff: function (inp, out, n, neventsPtr) {
        var v = w.view(), wait = 0, k;
        for (k = 0; k < n; k++) {
          var s = inp + k * 48;
          if (v.getUint8(s + 8) === 0) {
            var t = Number(v.getBigUint64(s + 24, true)) / 1e6;
            if (v.getUint16(s + 40, true) & 1) t -= Date.now();   // an absolute time (realtime clock)
            wait = Math.max(wait, t);
          }
        }
        if (wait > 0) w.sleep(wait);
        for (k = 0; k < n; k++) {
          var e = out + k * 32, src = inp + k * 48;
          v.setBigUint64(e, v.getBigUint64(src, true), true);
          v.setUint16(e + 8, 0, true);
          v.setUint8(e + 10, v.getUint8(src + 8));
          for (var z = 11; z < 32; z++) v.setUint8(e + z, 0);
        }
        v.setUint32(neventsPtr, n, true);
        return E.SUCCESS;
      },
      fd_prestat_get: function (fd, ptr) {
        var d = w.fds.get(fd);
        if (!d || !d.pre) return E.BADF;
        var v = w.view();
        v.setUint8(ptr, 0);
        v.setUint32(ptr + 4, enc.encode(d.pre).length, true);
        return E.SUCCESS;
      },
      fd_prestat_dir_name: function (fd, ptr, len) {
        var d = w.fds.get(fd);
        if (!d || !d.pre) return E.BADF;
        w.bytes().set(enc.encode(d.pre).subarray(0, len), ptr);
        return E.SUCCESS;
      },
      fd_fdstat_get: function (fd, ptr) {
        var d = w.fds.get(fd);
        if (!d) return E.BADF;
        var v = w.view();
        for (var i = 0; i < 24; i++) v.setUint8(ptr + i, 0);
        v.setUint8(ptr, d.kind === 'std' ? FT.CHAR : d.kind === 'dir' ? FT.DIR : FT.FILE);
        v.setUint16(ptr + 2, d.append ? 1 : 0, true);
        v.setBigUint64(ptr + 8, ALL, true);
        v.setBigUint64(ptr + 16, ALL, true);
        return E.SUCCESS;
      },
      fd_fdstat_set_flags: function (fd, flags) {
        var d = w.fds.get(fd);
        if (!d) return E.BADF;
        d.append = !!(flags & 1);
        return E.SUCCESS;
      },
      fd_fdstat_set_rights: function () { return E.SUCCESS; },
      fd_filestat_get: function (fd, ptr) {
        var d = w.fds.get(fd);
        if (!d) return E.BADF;
        if (d.kind === 'std') {
          var v = w.view();
          for (var i = 0; i < 64; i++) v.setUint8(ptr + i, 0);
          v.setUint8(ptr + 16, FT.CHAR);
          return E.SUCCESS;
        }
        return w.stat(d.path, ptr);
      },
      fd_filestat_set_size: function (fd, size) {
        var d = w.fds.get(fd), f = d && w.fs.files.get(d.path);
        if (!f) return E.BADF;
        var n = Number(size);
        grow(f, n);
        if (n < f.size) f.data.fill(0, n, f.size);
        f.size = n; d.dirty = true;
        return E.SUCCESS;
      },
      fd_filestat_set_times: function () { return E.SUCCESS; },
      fd_advise: function () { return E.SUCCESS; },
      fd_allocate: function () { return E.SUCCESS; },
      fd_sync: function () { return E.SUCCESS; },
      fd_datasync: function () { return E.SUCCESS; },
      fd_close: function (fd) {
        var d = w.fds.get(fd);
        if (!d) return E.BADF;
        if (fd > 3) w.fds.delete(fd);
        if (d.kind === 'file' && d.dirty) w.changed(d.path);
        return E.SUCCESS;
      },
      fd_renumber: function (from, to) {
        var d = w.fds.get(from);
        if (!d) return E.BADF;
        w.fds.set(to, d); w.fds.delete(from);
        return E.SUCCESS;
      },
      fd_seek: function (fd, offset, whence, ptr) {
        var d = w.fds.get(fd), f = d && w.fs.files.get(d.path);
        if (!d || d.kind === 'std') return E.SPIPE;
        if (!f) return E.BADF;
        var o = Number(offset), pos = whence === 0 ? o : whence === 1 ? d.pos + o : f.size + o;
        if (pos < 0) return E.INVAL;
        d.pos = pos;
        w.view().setBigUint64(ptr, BigInt(pos), true);
        return E.SUCCESS;
      },
      fd_tell: function (fd, ptr) {
        var d = w.fds.get(fd);
        if (!d || d.kind !== 'file') return E.BADF;
        w.view().setBigUint64(ptr, BigInt(d.pos), true);
        return E.SUCCESS;
      },
      fd_write: function (fd, iovs, n, outPtr) {
        var d = w.fds.get(fd);
        if (!d) return E.BADF;
        var m = w.bytes(), chunks = w.iovs(iovs, n).map(function (c) { return m.slice(c.buf, c.buf + c.len); });
        var total = chunks.reduce(function (a, c) { return a + c.length; }, 0);
        if (d.kind === 'std') {
          if (fd !== 0) w.flushLine(fd, chunks.map(function (c) { return dec.decode(c); }).join(''));
        } else if (d.kind === 'file') {
          if (!d.write) return E.BADF;
          var r = w.write(d, chunks);
          if (r < 0) return -r;
        } else return E.ISDIR;
        w.view().setUint32(outPtr, total, true);
        return E.SUCCESS;
      },
      fd_pwrite: function (fd, iovs, n, offset, outPtr) {
        var d = w.fds.get(fd);
        if (!d || d.kind !== 'file' || !d.write) return E.BADF;
        var m = w.bytes(), chunks = w.iovs(iovs, n).map(function (c) { return m.slice(c.buf, c.buf + c.len); });
        var r = w.write(d, chunks, Number(offset));
        if (r < 0) return -r;
        w.view().setUint32(outPtr, r, true);
        return E.SUCCESS;
      },
      fd_read: function (fd, iovs, n, outPtr) {
        var d = w.fds.get(fd);
        if (!d) return E.BADF;
        if (d.kind === 'std') { w.view().setUint32(outPtr, 0, true); return E.SUCCESS; }
        if (d.kind !== 'file') return E.ISDIR;
        var f = w.fs.files.get(d.path);
        if (!f) return E.BADF;
        var m = w.bytes(), got = 0;
        w.iovs(iovs, n).forEach(function (c) {
          var k = Math.max(0, Math.min(c.len, f.size - d.pos));
          m.set(f.data.subarray(d.pos, d.pos + k), c.buf);
          d.pos += k; got += k;
        });
        w.view().setUint32(outPtr, got, true);
        return E.SUCCESS;
      },
      fd_pread: function (fd, iovs, n, offset, outPtr) {
        var d = w.fds.get(fd), f = d && w.fs.files.get(d.path);
        if (!f) return E.BADF;
        var m = w.bytes(), pos = Number(offset), got = 0;
        w.iovs(iovs, n).forEach(function (c) {
          var k = Math.max(0, Math.min(c.len, f.size - pos));
          m.set(f.data.subarray(pos, pos + k), c.buf);
          pos += k; got += k;
        });
        w.view().setUint32(outPtr, got, true);
        return E.SUCCESS;
      },
      fd_readdir: function (fd, buf, bufLen, cookie, usedPtr) {
        var d = w.fds.get(fd);
        if (!d || d.kind !== 'dir') return E.BADF;
        var list = [{ name: '.', type: FT.DIR, path: d.path }, { name: '..', type: FT.DIR, path: parent(d.path) }].concat(w.fs.children(d.path));
        var v = w.view(), m = w.bytes(), used = 0;
        for (var i = Number(cookie); i < list.length && used < bufLen; i++) {
          var name = enc.encode(list[i].name), ent = new Uint8Array(24 + name.length), ev = new DataView(ent.buffer);
          ev.setBigUint64(0, BigInt(i + 1), true);
          ev.setBigUint64(8, BigInt(w.fs.inode(list[i].path)), true);
          ev.setUint32(16, name.length, true);
          ev.setUint8(20, list[i].type);
          ent.set(name, 24);
          var k = Math.min(ent.length, bufLen - used);
          m.set(ent.subarray(0, k), buf + used);
          used += k;
        }
        v.setUint32(usedPtr, used, true);
        return E.SUCCESS;
      },
      path_open: function (dirfd, dirflags, ptr, len, oflags, rightsBase, rightsInh, fdflags, fdPtr) {
        var p = w.at(dirfd, ptr, len);
        if (p === null) return E.BADF;
        var isDir = w.fs.dirs.has(p), f = w.fs.files.get(p);
        var wantWrite = (BigInt(rightsBase) & RIGHT_WRITE) !== 0n || !!(oflags & O.TRUNC) || !!(fdflags & 1);
        if (isDir) {
          if (oflags & (O.CREAT | O.EXCL) && oflags & O.EXCL) return E.EXIST;
          if (wantWrite && !(oflags & O.DIRECTORY)) return E.ISDIR;
          var dfd = w.nextFd++;
          w.fds.set(dfd, { kind: 'dir', path: p });
          w.view().setUint32(fdPtr, dfd, true);
          return E.SUCCESS;
        }
        if (oflags & O.DIRECTORY) return f ? E.NOTDIR : E.NOENT;
        if (f && oflags & O.CREAT && oflags & O.EXCL) return E.EXIST;
        var dirty = false;
        if (!f) {
          if (!(oflags & O.CREAT)) return E.NOENT;
          if (!w.fs.dirs.has(parent(p))) return E.NOENT;
          if (w.fs.files.has(parent(p))) return E.NOTDIR;
          f = { data: new Uint8Array(0), size: 0, mtime: Date.now() };
          w.fs.files.set(p, f);
          dirty = true;
        } else if (oflags & O.TRUNC) {
          f.size = 0; f.mtime = Date.now(); dirty = true;
        }
        var fd = w.nextFd++;
        w.fds.set(fd, { kind: 'file', path: p, pos: 0, append: !!(fdflags & 1), write: wantWrite, dirty: dirty });
        w.view().setUint32(fdPtr, fd, true);
        return E.SUCCESS;
      },
      path_create_directory: function (fd, ptr, len) {
        var p = w.at(fd, ptr, len);
        if (p === null) return E.BADF;
        if (w.fs.dirs.has(p) || w.fs.files.has(p)) return E.EXIST;
        if (!w.fs.dirs.has(parent(p))) return E.NOENT;
        w.fs.dirs.add(p);
        return E.SUCCESS;
      },
      path_filestat_get: function (fd, flags, ptr, len, out) {
        var p = w.at(fd, ptr, len);
        if (p === null) return E.BADF;
        return w.stat(p, out);
      },
      path_filestat_set_times: function () { return E.SUCCESS; },
      path_rename: function (fd, op, ol, nfd, np, nl) {
        var a = w.at(fd, op, ol), b = w.at(nfd, np, nl);
        if (a === null || b === null) return E.BADF;
        if (!w.fs.dirs.has(parent(b))) return E.NOENT;
        var f = w.fs.files.get(a);
        if (f) {
          if (w.fs.dirs.has(b)) return E.ISDIR;
          w.fs.files.delete(a);
          w.fs.files.set(b, f);
          w.fds.forEach(function (d) { if (d.path === a) d.path = b; });
          w.onRemove(a);
          w.changed(b);
          return E.SUCCESS;
        }
        if (!w.fs.dirs.has(a)) return E.NOENT;
        if (w.fs.files.has(b)) return E.NOTDIR;
        if (w.fs.dirs.has(b) && w.fs.children(b).length) return E.NOTEMPTY;
        var moved = [];
        w.fs.dirs.forEach(function (p) { if (p === a || p.indexOf(a + '/') === 0) moved.push(['d', p]); });
        w.fs.files.forEach(function (_, p) { if (p.indexOf(a + '/') === 0) moved.push(['f', p]); });
        moved.forEach(function (m) {
          var to = b + m[1].slice(a.length);
          if (m[0] === 'd') { w.fs.dirs.delete(m[1]); w.fs.dirs.add(to); }
          else { var x = w.fs.files.get(m[1]); w.fs.files.delete(m[1]); w.fs.files.set(to, x); w.onRemove(m[1]); w.changed(to); }
        });
        return E.SUCCESS;
      },
      path_unlink_file: function (fd, ptr, len) {
        var p = w.at(fd, ptr, len);
        if (p === null) return E.BADF;
        if (w.fs.dirs.has(p)) return E.ISDIR;
        if (!w.fs.files.delete(p)) return E.NOENT;
        w.onRemove(p);
        return E.SUCCESS;
      },
      path_remove_directory: function (fd, ptr, len) {
        var p = w.at(fd, ptr, len);
        if (p === null) return E.BADF;
        if (!w.fs.dirs.has(p)) return w.fs.files.has(p) ? E.NOTDIR : E.NOENT;
        if (w.fs.children(p).length) return E.NOTEMPTY;
        w.fs.dirs.delete(p);
        return E.SUCCESS;
      },
      path_readlink: function () { return E.INVAL; },
    };
    // anything else Rust's std may import (links, sockets): "not supported here"
    return new Proxy(fns, { get: function (t, k) { return k in t ? t[k] : function () { return E.NOSYS; }; } });
  };

  /** Runs a WASI command (its _start) until it returns or calls proc_exit; the exit code. */
  Wasi.prototype.start = function (instance) {
    try {
      instance.exports._start();
      return 0;
    } catch (e) {
      if (e instanceof ExitError) return e.code;
      throw e;
    } finally {
      if (this.lines[1]) this.flushLine(1, '\n');
      if (this.lines[2]) this.flushLine(2, '\n');
    }
  };

  return { Wasi: Wasi, MemFS: MemFS, ExitError: ExitError, errno: E, norm: norm, base: base, parent: parent };
});
