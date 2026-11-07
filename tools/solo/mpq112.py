"""MPQ archives for Leyline Solo (our own code, MIT; stdlib only). Docs: docs/LEYLINE-SOLO-PHASE0.md.

* `build(files)`: the archive bytes for our own few files, the subset launcher/src-tauri/src/mpq.rs writes (format
  version 0, the one the first-era client reads): the 32-byte header, each file in 4 KB sectors with a sector
  offset table, each sector deflated (compression byte 0x02) when that is smaller, else stored; the hash and block
  tables encrypted as the format requires; a `(listfile)`. No file encryption, no single-unit files, no CRCs.
* `Archive(path)`: a reader for one file of an archive, enough for the player's OWN archives on their own PC
  (encrypted files, single-unit files, sectors compressed with zlib, bzip2 or the PKWARE "implode" method). Used
  only to read the name list of the player's login screen (Interface\\GlueXML\\GlueXML.toc) and to tell our archive
  from anyone else's. Nothing read here is copied anywhere but into the player's own patch archive on their PC.

Names are stored with `/` turned into `\\`; lookups are without regard to case, as in the client.
"""
import bz2
import struct
import zlib

SECTOR_SHIFT = 3
SECTOR_SIZE = 512 << SECTOR_SHIFT
HEADER_SIZE = 32
LISTFILE = "(listfile)"

FLAG_IMPLODE = 0x00000100
FLAG_COMPRESS = 0x00000200
FLAG_ENCRYPTED = 0x00010000
FLAG_FIX_KEY = 0x00020000
FLAG_SINGLE_UNIT = 0x01000000
FLAG_SECTOR_CRC = 0x04000000
FLAG_EXISTS = 0x80000000
M32 = 0xFFFFFFFF


class MpqError(Exception):
    pass


def _crypt_table():
    t = [0] * 0x500
    seed = 0x00100001
    for i1 in range(0x100):
        i2 = i1
        for _ in range(5):
            seed = (seed * 125 + 3) % 0x2AAAAB
            hi = (seed & 0xFFFF) << 16
            seed = (seed * 125 + 3) % 0x2AAAAB
            t[i2] = hi | (seed & 0xFFFF)
            i2 += 0x100
    return t


CRYPT = _crypt_table()


def hash_name(name, kind):
    """The format's string hash: 0 table index, 1 and 2 the two name checks, 3 the encryption key."""
    s1, s2 = 0x7FED7FED, 0xEEEEEEEE
    for ch in name.replace("/", "\\").upper().encode("ascii", "replace"):
        s1 = (CRYPT[kind * 0x100 + ch] ^ ((s1 + s2) & M32)) & M32
        s2 = (ch + s1 + s2 + (s2 << 5) + 3) & M32
    return s1


def encrypt_words(words, key):
    out = []
    s2 = 0xEEEEEEEE
    for w in words:
        s2 = (s2 + CRYPT[0x400 + (key & 0xFF)]) & M32
        out.append((w ^ ((key + s2) & M32)) & M32)
        key = ((((~key) << 0x15) + 0x11111111) | (key >> 0x0B)) & M32
        s2 = (w + s2 + (s2 << 5) + 3) & M32
    return out


def decrypt_words(words, key):
    out = []
    s2 = 0xEEEEEEEE
    for w in words:
        s2 = (s2 + CRYPT[0x400 + (key & 0xFF)]) & M32
        v = (w ^ ((key + s2) & M32)) & M32
        out.append(v)
        key = ((((~key) << 0x15) + 0x11111111) | (key >> 0x0B)) & M32
        s2 = (v + s2 + (s2 << 5) + 3) & M32
    return out


def _decrypt_bytes(data, key):
    n = len(data) // 4
    words = decrypt_words(struct.unpack("<%dI" % n, data[:n * 4]), key)
    return struct.pack("<%dI" % n, *words) + data[n * 4:]


def name_ok(name):
    """A name the archive may hold: a relative game path, printable ASCII, no `..` part, at most 200 bytes."""
    n = name.replace("/", "\\")
    if not n or len(n) > 200 or n.startswith("\\"):
        return False
    if any(not (0x20 <= ord(c) < 0x7F) or c in ':"' for c in n):
        return False
    return all(p not in ("", ".", "..") for p in n.split("\\"))


def _pack(data):
    n = (len(data) + SECTOR_SIZE - 1) // SECTOR_SIZE
    start = 4 * (n + 1)
    table, body = [], bytearray()
    for i in range(n):
        chunk = data[i * SECTOR_SIZE:(i + 1) * SECTOR_SIZE]
        table.append(start + len(body))
        z = zlib.compress(chunk, 9)
        if 1 + len(z) < len(chunk):
            body += b"\x02" + z
        else:
            body += chunk
    table.append(start + len(body))
    return struct.pack("<%dI" % len(table), *table) + bytes(body)


def build(files):
    """The archive bytes for `files` [(name, bytes)], in the order given, plus a `(listfile)`. Refuses a bad or
    repeated name (without regard to case), or more than 4095 files."""
    seen = set()
    for n, _ in files:
        if not name_ok(n) or n.lower() == LISTFILE:
            raise MpqError("not a name an archive can hold: %r" % n)
        k = n.replace("/", "\\").upper()
        if k in seen:
            raise MpqError("the same file twice: %s" % n)
        seen.add(k)
    if len(files) >= 4096:
        raise MpqError("too many files for one archive")
    allf = [(n.replace("/", "\\"), bytes(d)) for n, d in files]
    allf.append((LISTFILE, "".join(n + "\r\n" for n, _ in allf).encode("ascii")))

    out = bytearray(HEADER_SIZE)
    blocks = []
    for _, d in allf:
        pos = len(out)
        if not d:
            blocks.append((pos, 0, 0, FLAG_EXISTS))
            continue
        p = _pack(d)
        out += p
        blocks.append((pos, len(p), len(d), FLAG_EXISTS | FLAG_COMPRESS))
    size = 16
    while size < len(allf) * 2:
        size *= 2
    table = [[M32, M32, M32, M32] for _ in range(size)]
    for i, (n, _) in enumerate(allf):
        at = hash_name(n, 0) & (size - 1)
        while table[at][3] != M32:
            at = (at + 1) & (size - 1)
        # name A, name B, locale 0 (neutral) and platform 0, block index
        table[at] = [hash_name(n, 1), hash_name(n, 2), 0, i]
    hw = encrypt_words([w for e in table for w in e], hash_name("(hash table)", 3))
    bw = encrypt_words([w for e in blocks for w in e], hash_name("(block table)", 3))
    hash_pos = len(out)
    out += struct.pack("<%dI" % len(hw), *hw)
    block_pos = len(out)
    out += struct.pack("<%dI" % len(bw), *bw)
    out[:32] = b"MPQ\x1a" + struct.pack("<IIHHIIII", HEADER_SIZE, len(out), 0, SECTOR_SHIFT, hash_pos, block_pos,
                                        size, len(blocks))
    return bytes(out)


# ------------------------------------------------------------------------------------------------ PKWARE explode
# The PKWARE Data Compression Library's "implode" format, decoded as zlib's contrib/blast/blast.c (Mark Adler,
# zlib licence) describes it: a literal/length/distance code with fixed Huffman tables, codes stored bit-inverted.

_LITLEN = [11, 124, 8, 7, 28, 7, 188, 13, 76, 4, 10, 8, 12, 10, 12, 10, 8, 23, 8, 9, 7, 6, 7, 8, 7, 6, 55, 8, 23, 24,
           12, 11, 7, 9, 11, 12, 6, 7, 22, 5, 7, 24, 6, 11, 9, 6, 7, 22, 7, 11, 38, 7, 9, 8, 25, 11, 8, 11, 9, 12, 8,
           12, 5, 38, 5, 38, 5, 11, 7, 5, 6, 21, 6, 10, 53, 8, 7, 24, 10, 27, 44, 253, 253, 253, 252, 252, 252, 13, 12,
           45, 12, 45, 12, 61, 12, 45, 44, 173]
_LENLEN = [2, 35, 36, 53, 38, 23]
_DISTLEN = [2, 20, 53, 230, 247, 151, 248]
_BASE = [3, 2, 4, 5, 6, 7, 8, 9, 10, 12, 16, 24, 40, 72, 136, 264]
_EXTRA = [0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]
_MAXBITS = 13


def _huffman(rep):
    lengths = []
    for b in rep:
        lengths += [b & 15] * ((b >> 4) + 1)
    count = [0] * (_MAXBITS + 1)
    for n in lengths:
        count[n] += 1
    offs = [0] * (_MAXBITS + 2)
    for n in range(1, _MAXBITS + 1):
        offs[n + 1] = offs[n] + count[n]
    symbol = [0] * len(lengths)
    for s, n in enumerate(lengths):
        if n:
            symbol[offs[n]] = s
            offs[n] += 1
    return count, symbol


_LITCODE, _LENCODE, _DISTCODE = _huffman(_LITLEN), _huffman(_LENLEN), _huffman(_DISTLEN)


class _Bits:
    def __init__(self, data):
        self.data, self.pos, self.buf, self.cnt = data, 0, 0, 0

    def bit(self):
        if self.cnt == 0:
            if self.pos >= len(self.data):
                raise MpqError("implode stream ended early")
            self.buf, self.cnt = self.data[self.pos], 8
            self.pos += 1
        b = self.buf & 1
        self.buf >>= 1
        self.cnt -= 1
        return b

    def bits(self, n):
        v = 0
        for i in range(n):
            v |= self.bit() << i
        return v

    def decode(self, h):
        count, symbol = h
        code = first = index = 0
        for ln in range(1, _MAXBITS + 1):
            code |= self.bit() ^ 1
            c = count[ln]
            if code < first + c:
                return symbol[index + (code - first)]
            index += c
            first = (first + c) << 1
            code <<= 1
        raise MpqError("bad implode code")


def explode(data):
    s = _Bits(data)
    lit = s.bits(8)
    if lit > 1:
        raise MpqError("bad implode header")
    dict_bits = s.bits(8)
    if not 4 <= dict_bits <= 6:
        raise MpqError("bad implode dictionary size")
    out = bytearray()
    while True:
        if s.bit():
            sym = s.decode(_LENCODE)
            length = _BASE[sym] + s.bits(_EXTRA[sym])
            if length == 519:
                return bytes(out)
            shift = 2 if length == 2 else dict_bits
            dist = (s.decode(_DISTCODE) << shift) + s.bits(shift) + 1
            if dist > len(out):
                raise MpqError("implode distance too far back")
            for _ in range(length):
                out.append(out[-dist])
        else:
            out.append(s.decode(_LITCODE) if lit else s.bits(8))


def _decompress(sector, want):
    """One sector of a FLAG_COMPRESS file: the first byte names the methods (in the order they are undone)."""
    if len(sector) >= want:
        return sector[:want]
    mask, body = sector[0], sector[1:]
    if mask & ~(0x02 | 0x08 | 0x10):
        raise MpqError("compression %#x is not read here" % mask)
    if mask & 0x10:
        body = bz2.decompress(body)
    if mask & 0x08:
        body = explode(body)
    if mask & 0x02:
        body = zlib.decompress(body)
    return body


# ------------------------------------------------------------------------------------------------ reading
class Archive:
    """One MPQ archive (format version 0 or 1, the header at the start or at a 512-byte boundary)."""

    def __init__(self, path_or_bytes):
        if isinstance(path_or_bytes, (bytes, bytearray)):
            self.raw = bytes(path_or_bytes)
        else:
            with open(path_or_bytes, "rb") as f:
                self.raw = f.read()
        base = 0
        while base < min(len(self.raw), 1 << 20):
            if self.raw[base:base + 4] == b"MPQ\x1a":
                break
            base += 512
        else:
            raise MpqError("not an MPQ archive")
        if self.raw[base:base + 4] != b"MPQ\x1a":
            raise MpqError("not an MPQ archive")
        self.base = base
        (_hsize, _asize, self.version, shift, hpos, bpos, hn, bn) = struct.unpack_from("<IIHHIIII", self.raw, base + 4)
        if hn == 0 or hn & (hn - 1) or hn > 1 << 20 or bn > 1 << 20:
            raise MpqError("bad table sizes")
        self.sector = 512 << min(shift, 16)
        self.hashes = self._table(base + hpos, hn, "(hash table)")
        self.blocks = self._table(base + bpos, bn, "(block table)")

    def _table(self, pos, n, key):
        need = pos + 16 * n
        if need > len(self.raw):
            raise MpqError("table outside the archive")
        words = decrypt_words(struct.unpack_from("<%dI" % (4 * n), self.raw, pos), hash_name(key, 3))
        return [words[i:i + 4] for i in range(0, len(words), 4)]

    def _find(self, name):
        n = len(self.hashes)
        a, b = hash_name(name, 1), hash_name(name, 2)
        start = at = hash_name(name, 0) & (n - 1)
        hits = []
        while True:
            e = self.hashes[at]
            if e[3] == M32:
                break
            if e[0] == a and e[1] == b and e[3] < len(self.blocks):
                hits.append(e)
            at = (at + 1) & (n - 1)
            if at == start:
                break
        if not hits:
            return None
        hits.sort(key=lambda e: (e[2] & 0xFFFF) != 0)     # the neutral locale first
        return self.blocks[hits[0][3]]

    def has(self, name):
        blk = self._find(name)
        return bool(blk and blk[3] & FLAG_EXISTS)

    def read(self, name):
        blk = self._find(name.replace("/", "\\"))
        if not blk or not blk[3] & FLAG_EXISTS:
            raise KeyError(name)
        pos, csize, fsize, flags = blk
        start = self.base + pos
        data = self.raw[start:start + csize]
        if len(data) < csize:
            raise MpqError("file outside the archive")
        key = 0
        if flags & FLAG_ENCRYPTED:
            key = hash_name(name.replace("/", "\\").split("\\")[-1], 3)
            if flags & FLAG_FIX_KEY:
                key = ((key + pos) ^ fsize) & M32
        if fsize == 0:
            return b""
        if flags & FLAG_SINGLE_UNIT:
            if flags & FLAG_ENCRYPTED:
                data = _decrypt_bytes(data, key)
            if flags & FLAG_COMPRESS:
                return _decompress(data, fsize)
            if flags & FLAG_IMPLODE and csize < fsize:
                return explode(data)
            return data[:fsize]
        n = (fsize + self.sector - 1) // self.sector
        if not flags & (FLAG_COMPRESS | FLAG_IMPLODE):
            out = bytearray()
            for i in range(n):
                chunk = data[i * self.sector:(i + 1) * self.sector]
                out += _decrypt_bytes(chunk, (key + i) & M32) if flags & FLAG_ENCRYPTED else chunk
            return bytes(out[:fsize])
        tn = n + 1 + (1 if flags & FLAG_SECTOR_CRC else 0)
        if 4 * tn > len(data):
            raise MpqError("sector table outside the file")
        offs = list(struct.unpack_from("<%dI" % tn, data, 0))
        if flags & FLAG_ENCRYPTED:
            offs = decrypt_words(offs, (key - 1) & M32)
        out = bytearray()
        for i in range(n):
            a, b = offs[i], offs[i + 1]
            if not 0 <= a <= b <= len(data):
                raise MpqError("bad sector table (an encrypted file of another name?)")
            chunk = data[a:b]
            if flags & FLAG_ENCRYPTED:
                chunk = _decrypt_bytes(chunk, (key + i) & M32)
            want = min(self.sector, fsize - i * self.sector)
            if flags & FLAG_COMPRESS:
                out += _decompress(chunk, want)
            elif len(chunk) < want:
                out += explode(chunk)
            else:
                out += chunk[:want]
        if len(out) != fsize:
            raise MpqError("file size does not match")
        return bytes(out)

    def names(self):
        try:
            return [n for n in self.read(LISTFILE).decode("latin-1").replace("\r", "").split("\n") if n]
        except (KeyError, MpqError):
            return []
