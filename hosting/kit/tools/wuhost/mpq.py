"""MPQ archives, read and written with the Python standard library only (the Den's light data and packs:
denslice.py, packs.py). Reads the player's OWN game archives (1.12, 2.4.3 and 3.3.5a: format v1 and v2 headers,
the hash and block tables, encrypted files, sector CRCs ignored, zlib, bzip2 and PKWARE DCL implode); writes small
v1 archives (zlib sectors, a (listfile)) for the server's extractor and for client patch files. Ported from
stream/www-files/mpq.js (itself after StormLib, MIT, Ladislav Zezula). Not read: Huffman and ADPCM (audio only),
LZMA, PTCH patch files: those raise MPQError, so a pack never needs them (packs skip sound).

    python -m wuhost.mpq list <archive> [prefix]       the archive's (listfile), optionally filtered
"""
import bz2
import os
import struct
import sys
import zlib


class MPQError(Exception):
    pass


FLAG_IMPLODE = 0x00000100
FLAG_COMPRESS = 0x00000200
FLAG_ENCRYPTED = 0x00010000
FLAG_KEY_V2 = 0x00020000
FLAG_PATCH = 0x00100000
FLAG_SINGLE = 0x01000000
FLAG_DELETE = 0x02000000
FLAG_CRC = 0x04000000
FLAG_EXISTS = 0x80000000
HASH_EMPTY = 0xFFFFFFFF
HASH_DELETED = 0xFFFFFFFE
M32 = 0xFFFFFFFF


def _crypt_table():
    t = [0] * 0x500
    seed = 0x00100001
    for i1 in range(0x100):
        i2 = i1
        for _ in range(5):
            seed = (seed * 125 + 3) % 0x2AAAAB
            t1 = (seed & 0xFFFF) << 16
            seed = (seed * 125 + 3) % 0x2AAAAB
            t[i2] = t1 | (seed & 0xFFFF)
            i2 += 0x100
    return t


CRYPT = _crypt_table()


def hash_string(name, kind):
    """StormLib's HashString: kind 0 table offset, 1 name A, 2 name B, 3 file key."""
    s1, s2 = 0x7FED7FED, 0xEEEEEEEE
    for ch in name.encode("utf-8", "replace"):
        if ch == 0x2F:
            ch = 0x5C
        elif 0x61 <= ch <= 0x7A:
            ch -= 0x20
        s1 = (CRYPT[(kind << 8) + ch] ^ ((s1 + s2) & M32)) & M32
        s2 = (ch + s1 + s2 + (s2 << 5) + 3) & M32
    return s1


def _words(data):
    n = len(data) // 4
    return list(struct.unpack("<%dI" % n, data[:n * 4])), data[n * 4:]


def decrypt(data, key):
    words, rest = _words(data)
    seed = 0xEEEEEEEE
    for i, v in enumerate(words):
        seed = (seed + CRYPT[0x400 + (key & 0xFF)]) & M32
        ch = (v ^ ((key + seed) & M32)) & M32
        key = ((((~key) << 0x15) + 0x11111111) & M32) | (key >> 0x0B)
        seed = (ch + seed + (seed << 5) + 3) & M32
        words[i] = ch
    return struct.pack("<%dI" % len(words), *words) + rest


def encrypt(data, key):
    words, rest = _words(data)
    seed = 0xEEEEEEEE
    for i, v in enumerate(words):
        seed = (seed + CRYPT[0x400 + (key & 0xFF)]) & M32
        ch = (v ^ ((key + seed) & M32)) & M32
        key = ((((~key) << 0x15) + 0x11111111) & M32) | (key >> 0x0B)
        seed = (v + seed + (seed << 5) + 3) & M32
        words[i] = ch
    return struct.pack("<%dI" % len(words), *words) + rest


# ---------------------------------------------------------------------------------------------- PKWARE DCL explode
_PK_DistBits = (
    0x02, 0x04, 0x04, 0x05, 0x05, 0x05, 0x05, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06,
    0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07,
    0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07,
    0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08,
)
_PK_DistCode = (
    0x03, 0x0d, 0x05, 0x19, 0x09, 0x11, 0x01, 0x3e, 0x1e, 0x2e, 0x0e, 0x36, 0x16, 0x26, 0x06, 0x3a,
    0x1a, 0x2a, 0x0a, 0x32, 0x12, 0x22, 0x42, 0x02, 0x7c, 0x3c, 0x5c, 0x1c, 0x6c, 0x2c, 0x4c, 0x0c,
    0x74, 0x34, 0x54, 0x14, 0x64, 0x24, 0x44, 0x04, 0x78, 0x38, 0x58, 0x18, 0x68, 0x28, 0x48, 0x08,
    0xf0, 0x70, 0xb0, 0x30, 0xd0, 0x50, 0x90, 0x10, 0xe0, 0x60, 0xa0, 0x20, 0xc0, 0x40, 0x80, 0x00,
)
_PK_ExLenBits = (
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
)
_PK_LenBase = (
    0x0000, 0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x000a, 0x000e, 0x0016, 0x0026, 0x0046, 0x0086, 0x0106,
)
_PK_LenBits = (
    0x03, 0x02, 0x03, 0x03, 0x04, 0x04, 0x04, 0x05, 0x05, 0x05, 0x05, 0x06, 0x06, 0x06, 0x07, 0x07,
)
_PK_LenCode = (
    0x05, 0x03, 0x01, 0x06, 0x0a, 0x02, 0x0c, 0x14, 0x04, 0x18, 0x08, 0x30, 0x10, 0x20, 0x40, 0x00,
)
_PK_ChBitsAsc = (
    0x0b, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x08, 0x07, 0x0c, 0x0c, 0x07, 0x0c, 0x0c,
    0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0d, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c,
    0x04, 0x0a, 0x08, 0x0c, 0x0a, 0x0c, 0x0a, 0x08, 0x07, 0x07, 0x08, 0x09, 0x07, 0x06, 0x07, 0x08,
    0x07, 0x06, 0x07, 0x07, 0x07, 0x07, 0x08, 0x07, 0x07, 0x08, 0x08, 0x0c, 0x0b, 0x07, 0x09, 0x0b,
    0x0c, 0x06, 0x07, 0x06, 0x06, 0x05, 0x07, 0x08, 0x08, 0x06, 0x0b, 0x09, 0x06, 0x07, 0x06, 0x06,
    0x07, 0x0b, 0x06, 0x06, 0x06, 0x07, 0x09, 0x08, 0x09, 0x09, 0x0b, 0x08, 0x0b, 0x09, 0x0c, 0x08,
    0x0c, 0x05, 0x06, 0x06, 0x06, 0x05, 0x06, 0x06, 0x06, 0x05, 0x0b, 0x07, 0x05, 0x06, 0x05, 0x05,
    0x06, 0x0a, 0x05, 0x05, 0x05, 0x05, 0x08, 0x07, 0x08, 0x08, 0x0a, 0x0b, 0x0b, 0x0c, 0x0c, 0x0c,
    0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d,
    0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d,
    0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d,
    0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c,
    0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c,
    0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c,
    0x0d, 0x0c, 0x0d, 0x0d, 0x0d, 0x0c, 0x0d, 0x0d, 0x0d, 0x0c, 0x0d, 0x0d, 0x0d, 0x0d, 0x0c, 0x0d,
    0x0d, 0x0d, 0x0c, 0x0c, 0x0c, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d, 0x0d,
)
_PK_ChCodeAsc = (
    0x0490, 0x0fe0, 0x07e0, 0x0be0, 0x03e0, 0x0de0, 0x05e0, 0x09e0, 0x01e0, 0x00b8, 0x0062, 0x0ee0, 0x06e0, 0x0022, 0x0ae0, 0x02e0,
    0x0ce0, 0x04e0, 0x08e0, 0x00e0, 0x0f60, 0x0760, 0x0b60, 0x0360, 0x0d60, 0x0560, 0x1240, 0x0960, 0x0160, 0x0e60, 0x0660, 0x0a60,
    0x000f, 0x0250, 0x0038, 0x0260, 0x0050, 0x0c60, 0x0390, 0x00d8, 0x0042, 0x0002, 0x0058, 0x01b0, 0x007c, 0x0029, 0x003c, 0x0098,
    0x005c, 0x0009, 0x001c, 0x006c, 0x002c, 0x004c, 0x0018, 0x000c, 0x0074, 0x00e8, 0x0068, 0x0460, 0x0090, 0x0034, 0x00b0, 0x0710,
    0x0860, 0x0031, 0x0054, 0x0011, 0x0021, 0x0017, 0x0014, 0x00a8, 0x0028, 0x0001, 0x0310, 0x0130, 0x003e, 0x0064, 0x001e, 0x002e,
    0x0024, 0x0510, 0x000e, 0x0036, 0x0016, 0x0044, 0x0030, 0x00c8, 0x01d0, 0x00d0, 0x0110, 0x0048, 0x0610, 0x0150, 0x0060, 0x0088,
    0x0fa0, 0x0007, 0x0026, 0x0006, 0x003a, 0x001b, 0x001a, 0x002a, 0x000a, 0x000b, 0x0210, 0x0004, 0x0013, 0x0032, 0x0003, 0x001d,
    0x0012, 0x0190, 0x000d, 0x0015, 0x0005, 0x0019, 0x0008, 0x0078, 0x00f0, 0x0070, 0x0290, 0x0410, 0x0010, 0x07a0, 0x0ba0, 0x03a0,
    0x0240, 0x1c40, 0x0c40, 0x1440, 0x0440, 0x1840, 0x0840, 0x1040, 0x0040, 0x1f80, 0x0f80, 0x1780, 0x0780, 0x1b80, 0x0b80, 0x1380,
    0x0380, 0x1d80, 0x0d80, 0x1580, 0x0580, 0x1980, 0x0980, 0x1180, 0x0180, 0x1e80, 0x0e80, 0x1680, 0x0680, 0x1a80, 0x0a80, 0x1280,
    0x0280, 0x1c80, 0x0c80, 0x1480, 0x0480, 0x1880, 0x0880, 0x1080, 0x0080, 0x1f00, 0x0f00, 0x1700, 0x0700, 0x1b00, 0x0b00, 0x1300,
    0x0da0, 0x05a0, 0x09a0, 0x01a0, 0x0ea0, 0x06a0, 0x0aa0, 0x02a0, 0x0ca0, 0x04a0, 0x08a0, 0x00a0, 0x0f20, 0x0720, 0x0b20, 0x0320,
    0x0d20, 0x0520, 0x0920, 0x0120, 0x0e20, 0x0620, 0x0a20, 0x0220, 0x0c20, 0x0420, 0x0820, 0x0020, 0x0fc0, 0x07c0, 0x0bc0, 0x03c0,
    0x0dc0, 0x05c0, 0x09c0, 0x01c0, 0x0ec0, 0x06c0, 0x0ac0, 0x02c0, 0x0cc0, 0x04c0, 0x08c0, 0x00c0, 0x0f40, 0x0740, 0x0b40, 0x0340,
    0x0300, 0x0d40, 0x1d00, 0x0d00, 0x1500, 0x0540, 0x0500, 0x1900, 0x0900, 0x0940, 0x1100, 0x0100, 0x1e00, 0x0e00, 0x0140, 0x1600,
    0x0600, 0x1a00, 0x0e40, 0x0640, 0x0a40, 0x0a00, 0x1200, 0x0200, 0x1c00, 0x0c00, 0x1400, 0x0400, 0x1800, 0x0800, 0x1000, 0x0000,
)


def _pk_table(codes, lens, bits):
    t = [-1] * (1 << bits)
    for s, code in enumerate(codes):
        step = 1 << lens[s]
        k = code & (step - 1)
        while k < len(t):
            t[k] = s
            k += step
    return t


_PK_T = {}


def explode(src, max_out):
    """PKWARE Data Compression Library 'implode' streams (StormLib's explode, via mpq.js)."""
    if len(src) <= 4:
        raise MPQError("pkware: bad data")
    ctype, dbits = src[0], src[1]
    if dbits < 4 or dbits > 6 or ctype not in (0, 1):
        raise MPQError("pkware: bad header")
    if not _PK_T:
        _PK_T["len"] = _pk_table(_PK_LenCode, _PK_LenBits, 7)
        _PK_T["dist"] = _pk_table(_PK_DistCode, _PK_DistBits, 8)
        _PK_T["lit"] = _pk_table(_PK_ChCodeAsc, _PK_ChBitsAsc, 13)
    T = _PK_T
    st = {"p": 2, "buf": 0, "cnt": 0}
    n = len(src)

    def need(k):
        while st["cnt"] < k:
            st["buf"] |= (src[st["p"]] if st["p"] < n else 0) << st["cnt"]
            st["p"] += 1
            st["cnt"] += 8

    def peek(k):
        need(k)
        return st["buf"] & ((1 << k) - 1)

    def skip(k):
        need(k)
        st["buf"] >>= k
        st["cnt"] -= k

    def take(k):
        v = peek(k)
        skip(k)
        return v

    out = bytearray()
    while len(out) < max_out:
        if st["p"] > n + 2:
            break
        if take(1):
            code = T["len"][peek(7)]
            skip(_PK_LenBits[code])
            extra = _PK_ExLenBits[code]
            ln = _PK_LenBase[code] + (take(extra) if extra else 0)
            if ln == 0x205:
                break
            rep = ln + 2
            dcode = T["dist"][peek(8)]
            skip(_PK_DistBits[dcode])
            dist = ((dcode << 2) | take(2) if rep == 2 else (dcode << dbits) | take(dbits)) + 1
            for _ in range(rep):
                if len(out) >= max_out:
                    break
                o = len(out)
                out.append(out[o - dist] if o - dist >= 0 else 0)
        elif ctype == 0:
            out.append(take(8))
        else:
            s = T["lit"][peek(13)]
            if s < 0:
                raise MPQError("pkware: bad literal code")
            skip(_PK_ChBitsAsc[s])
            out.append(s)
    return bytes(out)


def _decompress(data, want):
    mask, body = data[0], data[1:]
    if mask & ~(0x02 | 0x08 | 0x10):
        raise MPQError("compression 0x%02x is not read here (audio or LZMA)" % mask)
    # StormLib's decompression order: bzip2, then implode, then zlib
    if mask & 0x10:
        body = bz2.decompress(body)
    if mask & 0x08:
        body = explode(body, want)
    if mask & 0x02:
        body = zlib.decompress(body)
    return body


# ---------------------------------------------------------------------------------------------- reading
class Archive:
    """One MPQ archive, read only. names(): the (listfile); read(name) -> bytes or None."""

    def __init__(self, path):
        self.path = path
        self.f = open(path, "rb")
        self.base = self._find_header()
        f = self.f
        f.seek(self.base)
        hdr = f.read(44)
        (_magic, hsize, _asize, version, shift, hpos, bpos, hcount, bcount) = struct.unpack("<4sIIHHIIII", hdr[:32])
        hi_bt, hpos_hi, bpos_hi = 0, 0, 0
        if version >= 1 and hsize >= 44 and len(hdr) >= 44:
            hi_bt, hpos_hi, bpos_hi = struct.unpack("<QHH", hdr[32:44])
        self.sector = 512 << shift
        hpos += hpos_hi << 32
        bpos += bpos_hi << 32
        f.seek(self.base + hpos)
        ht = decrypt(f.read(hcount * 16), hash_string("(hash table)", 3))
        f.seek(self.base + bpos)
        bt = decrypt(f.read(bcount * 16), hash_string("(block table)", 3))
        self.hashes = [struct.unpack_from("<IIHHI", ht, i * 16) for i in range(hcount)]
        self.blocks = [list(struct.unpack_from("<IIII", bt, i * 16)) for i in range(bcount)]
        if hi_bt:
            f.seek(self.base + hi_bt)
            his = struct.unpack("<%dH" % bcount, f.read(bcount * 2))
            for b, h in zip(self.blocks, his):
                b[0] += h << 32
        self._names = None

    def _find_header(self):
        f = self.f
        off = 0
        size = os.fstat(f.fileno()).st_size
        while off < size:
            f.seek(off)
            head = f.read(16)
            if head[:4] == b"MPQ\x1a":
                return off
            if head[:4] == b"MPQ\x1b" and len(head) >= 12:
                return off + struct.unpack("<I", head[8:12])[0]
            off += 512
        raise MPQError("%s is not an MPQ archive" % self.path)

    def close(self):
        self.f.close()

    def __enter__(self):
        return self

    def __exit__(self, *a):
        self.close()

    def _find(self, name):
        n = len(self.hashes)
        if not n:
            return None
        i = hash_string(name, 0) % n
        a, b = hash_string(name, 1), hash_string(name, 2)
        best = None
        for _ in range(n):
            ha, hb, locale, _plat, bi = self.hashes[i]
            if bi == HASH_EMPTY:
                break
            if bi != HASH_DELETED and ha == a and hb == b and bi < len(self.blocks):
                if locale == 0:
                    return self.blocks[bi]
                best = best or self.blocks[bi]
            i = (i + 1) % n
        return best

    def has(self, name):
        b = self._find(name)
        return bool(b and b[3] & FLAG_EXISTS and not b[3] & FLAG_DELETE)

    def read(self, name):
        b = self._find(name)
        if not b:
            return None
        pos, csize, fsize, flags = b
        if not flags & FLAG_EXISTS or flags & FLAG_DELETE:
            return None
        if flags & FLAG_PATCH:
            raise MPQError("%s is a patch file (not read here)" % name)
        key = 0
        if flags & FLAG_ENCRYPTED:
            key = hash_string(name.replace("/", "\\").split("\\")[-1], 3)
            if flags & FLAG_KEY_V2:
                key = ((key + (pos & M32)) ^ fsize) & M32
        f = self.f
        f.seek(self.base + pos)
        if flags & FLAG_SINGLE:
            raw = f.read(csize)
            if flags & FLAG_ENCRYPTED:
                raw = decrypt(raw, key)
            if csize < fsize:
                raw = explode(raw, fsize) if (flags & FLAG_IMPLODE and not flags & FLAG_COMPRESS) else _decompress(raw, fsize)
            return raw[:fsize]
        if not flags & (FLAG_COMPRESS | FLAG_IMPLODE):
            nsec = (fsize + self.sector - 1) // self.sector
            raw = f.read(fsize)
            if flags & FLAG_ENCRYPTED:
                raw = b"".join(decrypt(raw[i * self.sector:(i + 1) * self.sector], (key + i) & M32) for i in range(nsec))
            return raw
        nsec = (fsize + self.sector - 1) // self.sector
        count = nsec + 1 + (1 if flags & FLAG_CRC else 0)
        table = f.read(count * 4)
        if flags & FLAG_ENCRYPTED:
            table = decrypt(table, (key - 1) & M32)
        offs = struct.unpack("<%dI" % count, table)
        out = []
        for i in range(nsec):
            f.seek(self.base + pos + offs[i])
            raw = f.read(offs[i + 1] - offs[i])
            if flags & FLAG_ENCRYPTED:
                raw = decrypt(raw, (key + i) & M32)
            want = min(self.sector, fsize - i * self.sector)
            if len(raw) < want:
                raw = explode(raw, want) if (flags & FLAG_IMPLODE and not flags & FLAG_COMPRESS) else _decompress(raw, want)
            out.append(raw[:want])
        return b"".join(out)

    def names(self):
        """The archive's (listfile): its file names as written there (backslashes)."""
        if self._names is None:
            text = self.read("(listfile)") or b""
            self._names = [ln.strip().replace("/", "\\") for ln in text.decode("utf-8", "replace").splitlines() if ln.strip()]
        return self._names


# ---------------------------------------------------------------------------------------------- writing
def write_archive(path, files, sector_shift=3):
    """A v1 archive at path holding files {name: bytes} (names with backslashes), each zlib-compressed by sector
    (stored when that doesn't help), plus a (listfile). No file encryption. Returns the archive's size."""
    files = dict(files)
    files["(listfile)"] = ("\r\n".join(sorted(n for n in files if n != "(listfile)")) + "\r\n").encode("utf-8")
    sector = 512 << sector_shift
    hcount = 16
    while hcount < 2 * len(files):
        hcount *= 2
    body = bytearray()
    blocks = []
    for name, data in files.items():
        pos = 32 + len(body)
        nsec = max(1, (len(data) + sector - 1) // sector)
        secs = []
        for i in range(nsec):
            chunk = data[i * sector:(i + 1) * sector]
            z = b"\x02" + zlib.compress(chunk, 9)
            secs.append(z if len(z) < len(chunk) else chunk)
        offs = [(nsec + 1) * 4]
        for s in secs:
            offs.append(offs[-1] + len(s))
        blob = struct.pack("<%dI" % (nsec + 1), *offs) + b"".join(secs)
        body += blob
        blocks.append((pos, len(blob), len(data), FLAG_EXISTS | FLAG_COMPRESS))
    table = [(HASH_EMPTY, HASH_EMPTY, 0xFFFF, 0xFFFF, HASH_EMPTY)] * hcount
    for bi, name in enumerate(files):
        i = hash_string(name, 0) % hcount
        while table[i][4] != HASH_EMPTY:
            i = (i + 1) % hcount
        table[i] = (hash_string(name, 1), hash_string(name, 2), 0, 0, bi)
    ht = b"".join(struct.pack("<IIHHI", *e) for e in table)
    bt = b"".join(struct.pack("<IIII", *b) for b in blocks)
    hpos = 32 + len(body)
    bpos = hpos + len(ht)
    size = bpos + len(bt)
    hdr = struct.pack("<4sIIHHIIII", b"MPQ\x1a", 32, size, 0, sector_shift, hpos, bpos, hcount, len(blocks))
    tmp = path + ".tmp"
    with open(tmp, "wb") as f:
        f.write(hdr)
        f.write(body)
        f.write(encrypt(ht, hash_string("(hash table)", 3)))
        f.write(encrypt(bt, hash_string("(block table)", 3)))
    os.replace(tmp, path)
    return size


# ---------------------------------------------------------------------------------------------- a game folder
LOCALES = ("enUS", "enGB", "deDE", "esES", "esMX", "frFR", "koKR", "zhCN", "zhTW", "enCN", "enTW", "ruRU")


def _patch_rank(base):
    """patch.MPQ = 1, patch-2.MPQ = 2, ... patch-9 = 9, patch-A..Z after; anything else 0."""
    b = base.lower()
    for pre in ("patch-",):
        pass
    if b == "patch.mpq":
        return 1
    if b.startswith("patch-") and b.endswith(".mpq"):
        tag = b[6:-4].split("-")[-1]
        if tag.isdigit():
            return int(tag)
        if len(tag) == 1 and tag.isalpha():
            return 10 + ord(tag) - ord("a")
        if tag == b[6:-4]:   # patch-enUS.MPQ (a locale's first patch)
            return 1
    return 0


class GameData:
    """A game folder's archives in priority order (a later archive wins): every base archive, then the patches in
    order with each locale patch right after the general one of the same number. read(name) -> bytes or None from
    the highest archive holding it; names(prefix) the union of their listfiles. Works for 1.12 (dbc.MPQ, model.MPQ,
    ...), 2.4.3 and 3.3.5a folders alike."""

    def __init__(self, game_dir):
        data = os.path.join(game_dir, "Data")
        if not os.path.isdir(data):
            raise MPQError("no Data folder in %s" % game_dir)
        self.locale = None
        found = []
        for name in sorted(os.listdir(data)):
            p = os.path.join(data, name)
            if os.path.isfile(p) and name.lower().endswith(".mpq"):
                found.append((_patch_rank(name), 0, name, p))
            elif os.path.isdir(p) and name in LOCALES:
                self.locale = self.locale or name
                for sub in sorted(os.listdir(p)):
                    q = os.path.join(p, sub)
                    if os.path.isfile(q) and sub.lower().endswith(".mpq"):
                        found.append((_patch_rank(sub), 1, sub, q))
        if not found:
            raise MPQError("no game archives in %s" % data)
        found.sort(key=lambda t: (t[0], t[1], t[2].lower()))
        self.archives = []
        for _rank, _loc, _name, p in found:
            try:
                self.archives.append(Archive(p))
            except (OSError, MPQError, struct.error):
                continue   # an unreadable archive is skipped, not fatal
        self.base = [a for a, t in zip(self.archives, found) if t[0] == 0]

    def close(self):
        for a in self.archives:
            a.close()

    def __enter__(self):
        return self

    def __exit__(self, *a):
        self.close()

    def read(self, name):
        name = name.replace("/", "\\")
        for a in reversed(self.archives):
            if a.has(name):
                return a.read(name)
        return None

    def has(self, name):
        name = name.replace("/", "\\")
        return any(a.has(name) for a in self.archives)

    def names(self, prefix=""):
        p = prefix.replace("/", "\\").lower()
        out = {}
        for a in self.archives:
            for n in a.names():
                if n.lower().startswith(p):
                    out.setdefault(n.lower(), n)
        return sorted(out.values(), key=str.lower)

    def build(self):
        """The client build from component.wow-<locale>.txt (3.3.5a: 12340), or None (older clients have none)."""
        if not self.locale:
            return None
        t = self.read("component.wow-%s.txt" % self.locale)
        if not t:
            return None
        m = t.decode("latin-1").split('version="')
        if len(m) > 1 and m[1].split('"')[0].isdigit():
            return int(m[1].split('"')[0])
        return None


def main(argv=None):
    a = list(sys.argv[1:] if argv is None else argv)
    if len(a) >= 2 and a[0] == "list":
        with Archive(a[1]) as ar:
            for n in ar.names():
                if len(a) < 3 or n.lower().startswith(a[2].lower()):
                    print(n)
        return 0
    print(__doc__.strip().splitlines()[-1].strip())
    return 2


if __name__ == "__main__":
    sys.exit(main())
