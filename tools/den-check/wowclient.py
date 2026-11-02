"""A tiny headless 3.3.5a (build 12340) client for den-check, our own code (MIT): it signs in to a login server
(SRP6), reads the realm list, connects to the world server, asks for the character list, enters the world with the
first character and asks the Leyline UI's Den panel question (the addon channel), exactly the steps the real game
takes after the one-click sign-in. Only for Den Lite on this PC: it never talks to anything but 127.0.0.1.

    steps = run_login("127.0.0.1", 3724, "DEN", "den")   # [(step, ok, detail), ...]
"""
import hashlib
import hmac
import os
import socket
import struct
import time
import zlib

N_HEX = "894B645E89E1535BBDAD5B8B290650530801B18EBFBF5E8FAB3C82872A3E9BB7"
G = 7
S_ENC_SEED = bytes.fromhex("CC98AE04E897EACA12DDC09342915357")  # the server's headers (we decrypt with it)
C_ENC_SEED = bytes.fromhex("C2B3723CC6AED9B5343C53EE2F4367CE")  # our headers (we encrypt with it)
SMSG_AUTH_CHALLENGE, CMSG_AUTH_SESSION, SMSG_AUTH_RESPONSE = 0x1EC, 0x1ED, 0x1EE
CMSG_CHAR_ENUM, SMSG_CHAR_ENUM, CMSG_PLAYER_LOGIN, SMSG_LOGIN_VERIFY_WORLD = 0x037, 0x03B, 0x03D, 0x236
CMSG_MESSAGECHAT, SMSG_MESSAGECHAT = 0x095, 0x096


def sha1(*parts):
    h = hashlib.sha1()
    for p in parts:
        h.update(p)
    return h.digest()


def le(n, size=32):
    return n.to_bytes(size, "little")


def from_le(b):
    return int.from_bytes(b, "little")


def interleave(s_bytes):
    """The session key K from S (32 bytes, little endian): even and odd bytes hashed apart, then interleaved."""
    s = bytes(s_bytes)
    while len(s) >= 2 and s[0] == 0:
        s = s[2:]
    even, odd = sha1(s[0::2]), sha1(s[1::2])
    out = bytearray()
    for i in range(20):
        out += bytes([even[i], odd[i]])
    return bytes(out)


class Rc4:
    def __init__(self, key, drop=1024):
        S = list(range(256))
        j = 0
        for i in range(256):
            j = (j + S[i] + key[i % len(key)]) & 0xFF
            S[i], S[j] = S[j], S[i]
        self.S, self.i, self.j = S, 0, 0
        self.crypt(bytes(drop))

    def crypt(self, data):
        S, i, j = self.S, self.i, self.j
        out = bytearray()
        for b in data:
            i = (i + 1) & 0xFF
            j = (j + S[i]) & 0xFF
            S[i], S[j] = S[j], S[i]
            out.append(b ^ S[(S[i] + S[j]) & 0xFF])
        self.i, self.j = i, j
        return bytes(out)


def recv_exact(s, n):
    b = b""
    while len(b) < n:
        c = s.recv(n - len(b))
        if not c:
            raise ConnectionError("the server closed the connection")
        b += c
    return b


def login(host, port, account, password, timeout=10):
    """SRP6 sign-in and the realm list: (K, [(name, address)])."""
    account, password = account.upper(), password.upper()
    s = socket.create_connection((host, port), timeout=timeout)
    try:
        acct = account.encode()
        body = (b"WoW\0" + bytes([3, 3, 5]) + struct.pack("<H", 12340) + b"68x\0" + b"niW\0" + b"SUne"
                + struct.pack("<I", 0) + bytes([127, 0, 0, 1]) + bytes([len(acct)]) + acct)
        s.sendall(bytes([0x00, 0x08]) + struct.pack("<H", len(body)) + body)
        head = recv_exact(s, 3)
        if head[2] != 0:
            raise RuntimeError("the login server refused the account (code %d)" % head[2])
        B = recv_exact(s, 32)
        glen = recv_exact(s, 1)[0]
        g = from_le(recv_exact(s, glen))
        nlen = recv_exact(s, 1)[0]
        N = from_le(recv_exact(s, nlen))
        salt = recv_exact(s, 32)
        recv_exact(s, 16)  # crc salt
        flags = recv_exact(s, 1)[0]
        if flags:
            raise RuntimeError("the login server asks for an authenticator")
        x = from_le(sha1(salt, sha1((account + ":" + password).encode())))
        a = from_le(os.urandom(19))
        A = pow(g, a, N)
        Bn = from_le(B)
        u = from_le(sha1(le(A), B))
        S = pow((Bn - 3 * pow(g, x, N)) % N, a + u * x, N)
        K = interleave(le(S))
        hn, hg = sha1(le(N, nlen)), sha1(le(g, glen))
        M1 = sha1(bytes(p ^ q for p, q in zip(hn, hg)), sha1(account.encode()), salt, le(A), B, K)
        s.sendall(bytes([0x01]) + le(A) + M1 + bytes(20) + bytes([0, 0]))
        r = recv_exact(s, 2)
        if r[1] != 0:
            raise RuntimeError("wrong password (code %d)" % r[1])
        M2 = recv_exact(s, 20)
        recv_exact(s, 4 + 4 + 2)
        if M2 != sha1(le(A), M1, K):
            raise RuntimeError("the server's proof doesn't match")
        s.sendall(bytes([0x10]) + bytes(4))
        h = recv_exact(s, 3)
        size = struct.unpack_from("<H", h, 1)[0]
        data = recv_exact(s, size)
        count = struct.unpack_from("<H", data, 4)[0]
        p, realms = 6, []
        for _ in range(count):
            flags_r = data[p + 2]
            p += 3
            e = data.index(b"\0", p)
            name = data[p:e].decode("utf-8", "replace")
            p = e + 1
            e = data.index(b"\0", p)
            addr = data[p:e].decode()
            p = e + 1 + 4 + 3
            if flags_r & 0x04:
                p += 5
            realms.append((name, addr))
        return K, realms
    finally:
        s.close()


class World:
    def __init__(self, address, account, K, timeout=10):
        host, port = address.rsplit(":", 1)
        if host not in ("127.0.0.1", "localhost"):
            raise RuntimeError("den-check only talks to a Den on this PC (the realm list said %s)" % address)
        self.s = socket.create_connection((host, int(port)), timeout=timeout)
        size, op = struct.unpack(">H", recv_exact(self.s, 2))[0], struct.unpack("<H", recv_exact(self.s, 2))[0]
        body = recv_exact(self.s, size - 2)
        if op != SMSG_AUTH_CHALLENGE:
            raise RuntimeError("no auth challenge")
        server_seed = body[4:8]
        client_seed = os.urandom(4)
        acct = account.upper().encode()
        digest = sha1(acct, bytes(4), client_seed, server_seed, K)
        plain = struct.pack("<I", 0) + struct.pack("<I", 0)
        addons = struct.pack("<I", len(plain)) + zlib.compress(plain)
        auth = (struct.pack("<II", 12340, 0) + acct + b"\0" + struct.pack("<I", 0) + client_seed
                + struct.pack("<III", 0, 0, 1) + struct.pack("<Q", 0) + digest + addons)
        self.s.sendall(struct.pack(">H", len(auth) + 4) + struct.pack("<I", CMSG_AUTH_SESSION) + auth)
        self.dec = Rc4(hmac.new(S_ENC_SEED, K, hashlib.sha1).digest())
        self.enc = Rc4(hmac.new(C_ENC_SEED, K, hashlib.sha1).digest())

    def send(self, op, body=b""):
        h = struct.pack(">H", len(body) + 4) + struct.pack("<I", op)
        self.s.sendall(self.enc.crypt(h) + body)

    def frame(self):
        b0 = self.dec.crypt(recv_exact(self.s, 1))
        if b0[0] & 0x80:
            rest = self.dec.crypt(recv_exact(self.s, 4))
            size = ((b0[0] & 0x7F) << 16) | (rest[0] << 8) | rest[1]
            op = struct.unpack("<H", rest[2:4])[0]
        else:
            rest = self.dec.crypt(recv_exact(self.s, 3))
            size = (b0[0] << 8) | rest[0]
            op = struct.unpack("<H", rest[1:3])[0]
        return op, recv_exact(self.s, size - 2)

    def until(self, op, seconds=10):
        end = time.time() + seconds
        while time.time() < end:
            o, b = self.frame()
            if o == op:
                return b
        raise TimeoutError("no answer 0x%03x" % op)

    def addon(self, me, prefix, text):
        body = struct.pack("<II", 7, 0xFFFFFFFF) + me.encode() + b"\0" + (prefix + "\t" + text).encode() + b"\0"
        self.send(CMSG_MESSAGECHAT, body)

    def addon_answer(self, want, seconds=10):
        end = time.time() + seconds
        while time.time() < end:
            o, b = self.frame()
            if o == SMSG_MESSAGECHAT and b[1:5] == b"\xff\xff\xff\xff":
                n = struct.unpack_from("<I", b, 25)[0]
                t = b[29:29 + n - 1].decode("utf-8", "replace")
                if t.startswith(want):
                    return t
        raise TimeoutError("no addon answer")

    def close(self):
        self.s.close()


def run_login(host, port, account, password, timeout=10):
    """The exact steps after the one-click sign-in, each (step, ok, detail); stops at the first failure."""
    steps = []

    def step(name, fn):
        t = time.time()
        try:
            d = fn()
            steps.append((name, True, "%s (%d ms)" % (d or "ok", (time.time() - t) * 1000)))
            return True
        except Exception as e:  # noqa: BLE001 - each step reports its own failure in plain words
            steps.append((name, False, str(e)))
            return False

    st = {}

    def do_login():
        st["K"], st["realms"] = login(host, port, account, password, timeout)
        return "signed in as %s" % account.upper()

    def do_realms():
        names = [r[0] for r in st["realms"]]
        if not names:
            raise RuntimeError("the realm list is empty")
        st["address"] = st["realms"][0][1]
        return "%s at %s" % (names[0], st["address"])

    def do_world():
        st["w"] = World(st["address"], account, st["K"], timeout)
        b = st["w"].until(SMSG_AUTH_RESPONSE)
        if b[0] != 0x0C:
            raise RuntimeError("the world server said no (code %d)" % b[0])
        return "session accepted"

    def do_chars():
        st["w"].send(CMSG_CHAR_ENUM)
        b = st["w"].until(SMSG_CHAR_ENUM)
        if b[0] < 1:
            raise RuntimeError("no character")
        st["guid"] = struct.unpack_from("<Q", b, 1)[0]
        e = b.index(b"\0", 9)
        st["name"] = b[9:e].decode("utf-8", "replace")
        return "%d character(s): %s" % (b[0], st["name"])

    def do_enter():
        st["w"].send(CMSG_PLAYER_LOGIN, struct.pack("<Q", st["guid"]))
        b = st["w"].until(SMSG_LOGIN_VERIFY_WORLD)
        m, x, y, z = struct.unpack_from("<Ifff", b, 0)
        return "map %d at %.0f %.0f %.0f" % (m, x, y, z)

    def do_panel():
        st["w"].addon(st["name"], "OVMOD", "wuden\tinfo\t1/1\t")
        t = st["w"].addon_answer("OVMOD\twuden\tinfo\t")
        return t.split("\t", 4)[-1]

    for name, fn in (("sign in", do_login), ("realm list", do_realms), ("world server", do_world),
                     ("character list", do_chars), ("enter the world", do_enter), ("Den panel answers", do_panel)):
        if not step(name, fn):
            break
    if "w" in st:
        st["w"].close()
    return steps
