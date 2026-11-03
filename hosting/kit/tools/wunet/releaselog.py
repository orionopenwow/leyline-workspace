"""The Leyline release log ("the chain"): an append-only, hash-chained, signed list of everything Leyline publishes.

Not money, not tokens, not mining: a log in which every entry names the entry before it by its sha256, and every
entry is signed with one Ed25519 key (the release key, which the owner keeps offline; docs/RESILIENCE.md). Anyone
can keep a copy, check all of it, and hand it to anyone else; nobody without the key can add, change or drop an
entry without being caught. The launcher (launcher/src-tauri/src/releaselog.rs), every relay and host agent with
the mesh on (wurelay/meshnode.py) and the server keep a copy and swap it.

What an entry names: a Leyline launcher release, a Host pack, the Leyline addons, or a pinned upstream server SOURCE
tarball (open-source code at a git commit), each with its files (name, size, sha256), optionally the torrent's
info-hash / magnet, and the mirrors that serve it. Never game data: a file name on the refusal list is refused.

Wire format (one JSON document):

    {"format": 1, "key": "<hex public key>", "entries": [{"entry": "<text>", "sig": "<hex>"}, ...]}

where <text> is the canonical JSON (sorted keys, no spaces, ensure_ascii=False) of the body

    {"v": 1, "seq": 0, "prev": "<64 zeros for the first, else sha256 hex of the previous <text>>",
     "kind": "launcher" | "hostpack" | "addons" | "source" | "note", "name": "...", "version": "0.3.2",
     "files": [{"name": "Leyline_0.3.2_x64-setup.exe", "size": 123, "sha256": "<hex>"}],
     "infohash": "<hex, optional>", "magnet": "magnet:?..., optional", "mirrors": ["https://.../"],
     "time": 1761900000, "source": {"repo": "https://github.com/...", "commit": "<40 hex>"} (kind source only)}

and <sig> is Ed25519 over b"leyline-release-log-v1\\n" + <text>. The entry's hash is sha256(<text>); the head is
the last entry's hash. A chain is good only when every entry verifies, seq counts 0, 1, 2 ... and each prev is the
hash before it. Between two good chains the longer wins (a peer's shorter or equal copy is kept out); a peer's
chain that is not signed by the key, broken, reordered or truncated in the middle is refused whole.

Command line (standard library only; the `cryptography` package is used when it is there, else pure Python):

    python3 -m wunet.releaselog keygen --out PATH          make a release key (prints the PUBLIC key only)
    python3 -m wunet.releaselog public --key PATH          print the public key of a key file
    python3 -m wunet.releaselog append --log chain.json --key PATH --kind launcher --name Leyline --version 0.3.2 \\
            --file Leyline_0.3.2_x64-setup.exe [--file ...] [--mirror https://.../] [--infohash H] [--magnet M] \\
            [--source-repo URL --source-commit SHA]
    python3 -m wunet.releaselog verify --log chain.json [--pub HEX]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import tempfile
import time

from . import keys

FORMAT = 1
CTX = b"leyline-release-log-v1\n"
ZERO = "0" * 64
KINDS = ("launcher", "hostpack", "addons", "source", "note")
MAX_ENTRIES = 100_000
MAX_TEXT = 16_384
MAX_FILES = 64
MAX_MIRRORS = 16
# A dev key for TESTS ONLY (everyone can see this seed: it signs nothing real). The launcher and the mesh trust
# only the public key in launcher/branding.json, which is never this one.
DEV_TEST_SEED = bytes.fromhex("6c65796c696e652d6465762d746573742d6b65792d6e6f742d666f722d757365")  # "leyline-dev-test-key-not-for-use"

_HEX64 = re.compile(r"^[0-9a-f]{64}$")
_HEX40 = re.compile(r"^[0-9a-f]{40}$")
_VER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9.+-]{0,47}$")
_FILE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._ +-]{0,119}$")
# The same floor as the launcher's p2p/checker.rs: game data and game clients are never named in the log.
_GAME_EXT = (".mpq", ".blp", ".m2", ".wmo", ".adt", ".dbc", ".db2", ".skin", ".anim", ".wdt", ".wdl", ".bls", ".casc")
_CLIENT = ("wow.exe", "wow-64.exe", "wowb.exe", "wowt.exe", "wowclassic.exe", "battle.net.exe")


HERE = os.path.dirname(os.path.abspath(__file__))
# The release key's PUBLIC half for relays and host agents (the same key as launcher/branding.json release_log.public_key;
# launcher/scripts/rebrand.py writes both). Empty until the owner makes the key: then no node keeps a chain.
KEY_FILE = os.path.join(HERE, "release-log.json")
# A copy of the chain shipped inside a Host pack (make-hostpack.py --chain), so a new host starts with it.
CHAIN_FILE = os.path.join(HERE, "release-log-chain.json")


class Bad(ValueError):
    pass


def shipped_key(path: str = KEY_FILE) -> str:
    try:
        with open(path, encoding="utf-8") as f:
            k = json.load(f).get("public_key") or ""
    except (OSError, ValueError, AttributeError):
        return ""
    return k if _HEX64.match(k) else ""


def shipped_chain(pub_hex: str, path: str = CHAIN_FILE):
    """The chain shipped beside this module, if there is one and it verifies; else None."""
    try:
        with open(path, encoding="utf-8") as f:
            d = json.load(f)
        verify(d, pub_hex)
        return d
    except (OSError, ValueError):
        return None


def canonical(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def entry_hash(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _https_dir(u) -> bool:
    return (isinstance(u, str) and u.startswith("https://") and u.endswith("/") and len(u) <= 300
            and not any(c.isspace() for c in u) and ".." not in u and "@" not in u.split("/")[2])


def refused_name(name: str) -> str | None:
    low = name.lower()
    if low in _CLIENT or (low.startswith("wow") and low.endswith(".exe")):
        return f"{name}: a game client file"
    if low.endswith(_GAME_EXT):
        return f"{name}: game data"
    return None


def check_body(b, seq: int, prev: str) -> None:
    """Raise Bad unless b is a well-formed body at position seq after prev."""
    if not isinstance(b, dict) or b.get("v") != 1:
        raise Bad("not a release log entry")
    if b.get("seq") != seq or isinstance(b.get("seq"), bool):
        raise Bad(f"entry {seq}: out of order")
    if b.get("prev") != prev:
        raise Bad(f"entry {seq}: does not follow the entry before it")
    if b.get("kind") not in KINDS:
        raise Bad(f"entry {seq}: unknown kind")
    if not isinstance(b.get("name"), str) or not 0 < len(b["name"]) <= 80:
        raise Bad(f"entry {seq}: name")
    if not isinstance(b.get("version"), str) or not _VER.match(b["version"]):
        raise Bad(f"entry {seq}: version")
    if not isinstance(b.get("time"), int) or isinstance(b["time"], bool) or b["time"] < 0:
        raise Bad(f"entry {seq}: time")
    files = b.get("files")
    if not isinstance(files, list) or len(files) > MAX_FILES:
        raise Bad(f"entry {seq}: files")
    for f in files:
        if not (isinstance(f, dict) and isinstance(f.get("name"), str) and _FILE.match(f["name"]) and ".." not in f["name"]
                and isinstance(f.get("size"), int) and not isinstance(f["size"], bool) and f["size"] >= 0
                and isinstance(f.get("sha256"), str) and _HEX64.match(f["sha256"])):
            raise Bad(f"entry {seq}: a file is not name/size/sha256")
        why = refused_name(f["name"])
        if why:
            raise Bad(f"entry {seq}: refused, {why}")
    mirrors = b.get("mirrors", [])
    if not isinstance(mirrors, list) or len(mirrors) > MAX_MIRRORS or not all(_https_dir(m) for m in mirrors):
        raise Bad(f"entry {seq}: mirrors must be https folders ending in /")
    ih = b.get("infohash")
    if ih is not None and not (isinstance(ih, str) and (_HEX40.match(ih) or _HEX64.match(ih))):
        raise Bad(f"entry {seq}: infohash")
    mg = b.get("magnet")
    if mg is not None and not (isinstance(mg, str) and mg.startswith("magnet:?") and len(mg) <= 2048):
        raise Bad(f"entry {seq}: magnet")
    src = b.get("source")
    if b["kind"] == "source":
        if not (isinstance(src, dict) and isinstance(src.get("repo"), str) and src["repo"].startswith("https://")
                and isinstance(src.get("commit"), str) and _HEX40.match(src["commit"])):
            raise Bad(f"entry {seq}: a source entry needs source.repo (https) and source.commit (40 hex)")
    elif src is not None:
        raise Bad(f"entry {seq}: only source entries name a source")


def verify(doc, pub_hex: str) -> list[dict]:
    """The bodies of a good chain signed by pub_hex, or raise Bad. pub_hex must be a real key."""
    if not (isinstance(pub_hex, str) and _HEX64.match(pub_hex)):
        raise Bad("no release key to check against")
    if not isinstance(doc, dict) or doc.get("format") != FORMAT:
        raise Bad("not a release log")
    if doc.get("key") not in (None, pub_hex):
        raise Bad("signed with another key")
    ents = doc.get("entries")
    if not isinstance(ents, list) or len(ents) > MAX_ENTRIES:
        raise Bad("no entries")
    pub = bytes.fromhex(pub_hex)
    prev, out = ZERO, []
    for i, e in enumerate(ents):
        if not isinstance(e, dict) or not isinstance(e.get("entry"), str) or not isinstance(e.get("sig"), str):
            raise Bad(f"entry {i}: not entry/sig")
        text = e["entry"]
        if len(text.encode("utf-8")) > MAX_TEXT:
            raise Bad(f"entry {i}: too long")
        try:
            sig = bytes.fromhex(e["sig"])
        except ValueError:
            raise Bad(f"entry {i}: signature is not hex")
        if not keys.verify(pub, CTX + text.encode("utf-8"), sig):
            raise Bad(f"entry {i}: the signature does not verify")
        try:
            body = json.loads(text)
        except ValueError:
            raise Bad(f"entry {i}: not JSON")
        check_body(body, i, prev)
        if out and body["time"] + 300 < out[-1]["time"]:
            raise Bad(f"entry {i}: earlier than the entry before it")
        out.append(body)
        prev = entry_hash(text)
    return out


def head(doc) -> str:
    ents = doc.get("entries") or []
    return entry_hash(ents[-1]["entry"]) if ents else ZERO


def empty(pub_hex: str) -> dict:
    return {"format": FORMAT, "key": pub_hex, "entries": []}


def file_ref(path: str, name: str | None = None) -> dict:
    h, n = hashlib.sha256(), 0
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
            n += len(chunk)
    return {"name": name or os.path.basename(path), "size": n, "sha256": h.hexdigest()}


def append(doc: dict, secret: bytes, *, kind: str, name: str, version: str, files: list[dict],
           mirrors: list[str] | None = None, infohash: str | None = None, magnet: str | None = None,
           source: dict | None = None, now: int | None = None) -> dict:
    """A new chain with one more signed entry (doc is not changed). Raises Bad for a malformed entry."""
    pub_hex = keys.public_key(secret).hex()
    bodies = verify(doc, pub_hex) if doc.get("entries") else []
    seq = len(bodies)
    body = {"v": 1, "seq": seq, "prev": head(doc), "kind": kind, "name": name, "version": version,
            "files": files, "mirrors": list(mirrors or []), "time": int(time.time() if now is None else now)}
    if bodies and body["time"] < bodies[-1]["time"]:
        body["time"] = bodies[-1]["time"]
    if infohash:
        body["infohash"] = infohash.lower()
    if magnet:
        body["magnet"] = magnet
    if source:
        body["source"] = source
    check_body(body, seq, body["prev"])
    text = canonical(body)
    if len(text.encode("utf-8")) > MAX_TEXT:
        raise Bad("entry too long")
    sig = keys.sign(secret, CTX + text.encode("utf-8")).hex()
    return {"format": FORMAT, "key": pub_hex, "entries": list(doc.get("entries") or []) + [{"entry": text, "sig": sig}]}


def choose(ours: dict | None, theirs, pub_hex: str):
    """(the chain to keep, what happened): 'adopted' (theirs extends ours), 'replaced' (a longer good fork),
    'kept' (theirs is not longer) or 'refused: <why>' (theirs does not verify). ours must already be good."""
    try:
        tb = verify(theirs, pub_hex)
    except Bad as e:
        return ours, f"refused: {e}"
    ol = len((ours or {}).get("entries") or [])
    if len(tb) <= ol:
        return ours, "kept"
    mine = (ours or {}).get("entries") or []
    if theirs["entries"][:ol] == mine:
        return {"format": FORMAT, "key": pub_hex, "entries": theirs["entries"]}, "adopted"
    return {"format": FORMAT, "key": pub_hex, "entries": theirs["entries"]}, "replaced"


def extend(ours: dict, more: list, pub_hex: str):
    """ours plus entries a peer sent after our head (the gossip's cheap path); choose() decides."""
    if not isinstance(more, list):
        return ours, "refused: not a list"
    return choose(ours, {"format": FORMAT, "key": pub_hex, "entries": list(ours.get("entries") or []) + more}, pub_hex)


class Log:
    """One node's copy, kept in a file (written only when it changes, atomically)."""

    def __init__(self, path: str, pub_hex: str):
        self.path, self.pub = path, pub_hex
        self.doc = empty(pub_hex)
        try:
            with open(path, encoding="utf-8") as f:
                d = json.load(f)
            verify(d, pub_hex)
            self.doc = d
        except (OSError, ValueError):
            pass

    def __len__(self):
        return len(self.doc["entries"])

    def head(self) -> dict:
        return {"seq": len(self) - 1, "head": head(self.doc), "key": self.pub}

    def since(self, n: int, limit: int = 500) -> list:
        n = max(0, min(int(n), len(self)))
        return self.doc["entries"][n:n + limit]

    def offer(self, doc) -> str:
        new, what = choose(self.doc, doc, self.pub)
        if what in ("adopted", "replaced"):
            self.doc = new
            self.save()
        return what

    def offer_more(self, more) -> str:
        new, what = extend(self.doc, more, self.pub)
        if what in ("adopted", "replaced"):
            self.doc = new
            self.save()
        return what

    def save(self):
        d = os.path.dirname(os.path.abspath(self.path))
        os.makedirs(d, exist_ok=True)
        fd, tmp = tempfile.mkstemp(dir=d, prefix=".chain-")
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(self.doc, f, ensure_ascii=False, indent=0)
        os.replace(tmp, self.path)


# ------------------------------------------------------------------------------------------------ key files
def write_key(path: str) -> str:
    """Make a new release key at path (refuses to overwrite). Returns the PUBLIC key hex; the secret never prints."""
    if os.path.exists(path):
        raise Bad(f"{path} already exists: a release key is made once (docs/RESILIENCE.md)")
    secret = keys.new_secret()
    pub = keys.public_key(secret).hex()
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(json.dumps({"kind": "leyline-release-log-key", "public": pub, "secret": secret.hex()}) + "\n")
    return pub


def read_key(path: str) -> bytes:
    with open(path, encoding="utf-8") as f:
        d = json.load(f)
    if d.get("kind") != "leyline-release-log-key":
        raise Bad(f"{path} is not a release log key")
    secret = bytes.fromhex(d["secret"])
    if keys.public_key(secret).hex() != d.get("public"):
        raise Bad(f"{path}: the key does not match its public half")
    return secret


def load(path: str):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except FileNotFoundError:
        return None


def save(path: str, doc: dict):
    d = os.path.dirname(os.path.abspath(path))
    os.makedirs(d, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=d, prefix=".chain-")
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, indent=0)
    os.replace(tmp, path)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(prog="python3 -m wunet.releaselog", description="The Leyline release log.")
    sub = ap.add_subparsers(dest="cmd", required=True)
    k = sub.add_parser("keygen")
    k.add_argument("--out", required=True)
    p = sub.add_parser("public")
    p.add_argument("--key", required=True)
    a = sub.add_parser("append")
    a.add_argument("--log", required=True)
    a.add_argument("--key", required=True)
    a.add_argument("--kind", required=True, choices=KINDS)
    a.add_argument("--name", required=True)
    a.add_argument("--version", required=True)
    a.add_argument("--file", action="append", default=[])
    a.add_argument("--mirror", action="append", default=[])
    a.add_argument("--infohash")
    a.add_argument("--magnet")
    a.add_argument("--source-repo")
    a.add_argument("--source-commit")
    v = sub.add_parser("verify")
    v.add_argument("--log", required=True)
    v.add_argument("--pub")
    o = ap.parse_args(argv)
    try:
        if o.cmd == "keygen":
            print(write_key(o.out))
            print("Public key above: put it in launcher/branding.json release_log.public_key. Back up the key file offline.",
                  file=sys.stderr)
        elif o.cmd == "public":
            print(keys.public_key(read_key(o.key)).hex())
        elif o.cmd == "append":
            secret = read_key(o.key)
            doc = load(o.log) or empty(keys.public_key(secret).hex())
            src = {"repo": o.source_repo, "commit": o.source_commit} if o.source_repo else None
            new = append(doc, secret, kind=o.kind, name=o.name, version=o.version,
                         files=[file_ref(f) for f in o.file], mirrors=o.mirror, infohash=o.infohash, magnet=o.magnet,
                         source=src)
            save(o.log, new)
            print(f"entry {len(new['entries']) - 1}: {o.kind} {o.name} {o.version}, head {head(new)[:16]}")
        elif o.cmd == "verify":
            doc = load(o.log)
            if doc is None:
                raise Bad(f"{o.log} not found")
            bodies = verify(doc, o.pub or doc.get("key") or "")
            print(f"good: {len(bodies)} entries, head {head(doc)[:16]}")
    except (Bad, OSError, ValueError) as e:
        print(f"release log: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
