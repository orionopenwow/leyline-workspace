"""A small read-only MPQ reader for den-check (our own code, MIT; no game data is read beyond what is asked).

It opens an archive's header and its two encrypted tables, looks a name up by the format's hashes (no file needs to
be unpacked for that), and reads files stored the way the launcher's writer stores them (launcher/src-tauri/src/
mpq.rs: sectors, each zlib or stored) or bzip2 sectors. Encrypted or imploded files are refused, never guessed.

    a = Archive("Data/patch-Z.MPQ"); a.has("Interface\\GlueXML\\GlueXML.toc"); a.read("(listfile)")
"""
import bz2
import struct
import zlib

FLAG_IMPLODE = 0x00000100
FLAG_COMPRESS = 0x00000200
FLAG_ENCRYPTED = 0x00010000
FLAG_SINGLE_UNIT = 0x01000000
FLAG_SECTOR_CRC = 0x04000000
FLAG_EXISTS = 0x80000000


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


def hash_name(name: str, kind: int) -> int:
    s1, s2 = 0x7FED7FED, 0xEEEEEEEE
    for ch in name.replace("/", "\\").upper().encode("ascii", "replace"):
        s1 = (CRYPT[kind * 0x100 + ch] ^ ((s1 + s2) & 0xFFFFFFFF)) & 0xFFFFFFFF
        s2 = (ch + s1 + s2 + (s2 << 5) + 3) & 0xFFFFFFFF
    return s1


def decrypt(words, key):
    out = []
    s2 = 0xEEEEEEEE
    for w in words:
        s2 = (s2 + CRYPT[0x400 + (key & 0xFF)]) & 0xFFFFFFFF
        v = (w ^ ((key + s2) & 0xFFFFFFFF)) & 0xFFFFFFFF
        out.append(v)
        key = ((((~key) << 0x15) + 0x11111111) | (key >> 0x0B)) & 0xFFFFFFFF
        s2 = (v + s2 + (s2 << 5) + 3) & 0xFFFFFFFF
    return out


class MpqError(Exception):
    pass


class Archive:
    def __init__(self, path, data=None):
        self.path = path
        self._f = path
        if data is None:
            with open(path, "rb") as f:
                data = f.read(1 << 20)
        self._head = data
        self._load()

    def _bytes(self, pos, n):
        if pos + n <= len(self._head):
            return self._head[pos:pos + n]
        with open(self._f, "rb") as f:
            f.seek(pos)
            return f.read(n)

    def _load(self):
        b = self._head
        # the header may sit at any 512-byte boundary (user data before it is skipped)
        off = 0
        while off + 32 <= len(b) and b[off:off + 4] != b"MPQ\x1a":
            if b[off:off + 4] == b"MPQ\x1b":
                off += struct.unpack_from("<I", b, off + 8)[0]
                continue
            off += 512
        if off + 32 > len(b) or b[off:off + 4] != b"MPQ\x1a":
            raise MpqError("not an MPQ archive")
        (hsize, self.size, self.version, shift, hpos, bpos, hcount, bcount) = struct.unpack_from("<IIHHIIII", b, off + 4)
        self.offset = off
        self.header_size = hsize
        self.sector = 512 << shift
        if hcount == 0 or hcount & (hcount - 1) or hcount > (1 << 20) or bcount > (1 << 20):
            raise MpqError("its tables don't look right")
        ht = struct.unpack("<%dI" % (hcount * 4), self._bytes(off + hpos, hcount * 16))
        bt = struct.unpack("<%dI" % (bcount * 4), self._bytes(off + bpos, bcount * 16))
        h = decrypt(ht, hash_name("(hash table)", 3))
        bl = decrypt(bt, hash_name("(block table)", 3))
        self.hashes = [tuple(h[i:i + 4]) for i in range(0, len(h), 4)]
        self.blocks = [tuple(bl[i:i + 4]) for i in range(0, len(bl), 4)]

    def _find(self, name):
        n = len(self.hashes)
        a, b, start = hash_name(name, 1), hash_name(name, 2), hash_name(name, 0) & (n - 1)
        for k in range(n):
            e = self.hashes[(start + k) & (n - 1)]
            if e[3] == 0xFFFFFFFF:
                return None
            if e[0] == a and e[1] == b and e[3] < len(self.blocks):
                return self.blocks[e[3]]
        return None

    def has(self, name):
        blk = self._find(name)
        return bool(blk and blk[3] & FLAG_EXISTS)

    def read(self, name):
        blk = self._find(name)
        if not blk or not blk[3] & FLAG_EXISTS:
            return None
        pos, csize, size, flags = blk
        if flags & (FLAG_ENCRYPTED | FLAG_IMPLODE):
            raise MpqError("%s is stored in a way this reader doesn't open" % name)
        raw = self._bytes(self.offset + pos, csize)
        if size == 0:
            return b""
        if flags & FLAG_SINGLE_UNIT:
            return self._unpack(raw, size) if flags & FLAG_COMPRESS and csize < size else raw[:size]
        if not flags & FLAG_COMPRESS:
            return raw[:size]
        n = (size + self.sector - 1) // self.sector
        offs = struct.unpack_from("<%dI" % (n + 1), raw, 0)
        out = bytearray()
        for i in range(n):
            want = min(self.sector, size - i * self.sector)
            part = raw[offs[i]:offs[i + 1]]
            out += self._unpack(part, want) if len(part) < want else part
        if len(out) != size:
            raise MpqError("%s doesn't unpack to its size" % name)
        return bytes(out)

    @staticmethod
    def _unpack(part, want):
        kind = part[0]
        if kind == 0x02:
            return zlib.decompress(part[1:])
        if kind == 0x10:
            return bz2.decompress(part[1:])
        raise MpqError("a compression this reader doesn't open (0x%02x)" % kind)

    def names(self):
        lf = self.read("(listfile)")
        if lf is None:
            return []
        return [n for n in lf.decode("utf-8", "replace").replace("\r", "").split("\n") if n]
