#!/usr/bin/env python3
"""Edit the Looks list (stream/looks.json; stream/looks.py has the rules, mods/SPEC.md "Looks" the reasoning).

    python3 -I tools/looks/looks.py check                     every entry against the rules; exit 1 on any problem
    python3 -I tools/looks/looks.py list                      id, published or waiting, one click or link, licence
    python3 -I tools/looks/looks.py approve look:x --by Owner [--notes "..."]
    python3 -I tools/looks/looks.py unapprove look:x
    python3 -I tools/looks/looks.py pin look:x --url https://github.com/<o>/<r>/releases/download/<tag>/<file>.zip|.MPQ
                                     [--file FILE]

pin makes a graphics pack one click: it downloads the author's own release file once (or reads FILE, a copy of that same
file), records its sha256, size and the sha256 of every .MPQ in it (the only files the launcher puts in the client), and
writes the install block. Nothing is kept or hosted: the download is deleted straight after. The entry still needs a
person's review (approve) before it shows. Only release files are pinned: they are byte-stable, commit archives are not.
Standard library only.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import importlib.util
import json
import os
import sys
import tempfile
import urllib.request
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
FILE = os.environ.get("WU_LOOKS", os.path.join(ROOT, "stream", "looks.json"))
MAX_DOWNLOAD = 1024 * 1024 * 1024 + 64 * 1024 * 1024   # one client patch (the launcher's cap) and a little


def rules():
    spec = importlib.util.spec_from_file_location("wu_looks_rules", os.path.join(ROOT, "stream", "looks.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def read(path: str) -> dict:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def write(path: str, doc: dict) -> None:
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(doc, f, indent=1, ensure_ascii=False)
        f.write("\n")
    os.replace(tmp, path)


def find(doc: dict, lid: str) -> dict:
    for e in doc.get("looks", []):
        if e.get("id") == lid:
            return e
    raise SystemExit(f"{lid}: not in the list")


def now() -> str:
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def sha_file(p: str) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def block_for(url: str, path: str, tag: str, files_digest) -> dict:
    """The install block for a downloaded release file at path: a bare .MPQ, or a zip whose .MPQ files are installed."""
    with open(path, "rb") as f:
        head = f.read(4)
    name = url.rsplit("/", 1)[-1]
    files, prefix, count = {}, "", 0
    if head[:2] == b"PK":
        with zipfile.ZipFile(path) as z:
            names = [i for i in z.infolist() if not i.is_dir()]
            count = len(names)
            tops = {n.filename.split("/", 1)[0] for n in names}
            if len(tops) == 1 and all("/" in n.filename for n in names):
                prefix = next(iter(tops)) + "/"
            for i in names:
                rel = i.filename[len(prefix):]
                if rel.lower().endswith(".mpq"):
                    if i.flag_bits & 1:
                        raise SystemExit(f"{rel}: password protected")
                    h = hashlib.sha256()
                    with z.open(i) as r:
                        for chunk in iter(lambda: r.read(1 << 20), b""):
                            h.update(chunk)
                    files[rel] = h.hexdigest()
        if not files:
            raise SystemExit("the zip has no .MPQ client patch in it")
    elif head[:3] == b"MPQ" and name.lower().endswith(".mpq"):
        files, count = {name: sha_file(path)}, 1
    else:
        raise SystemExit("the release file is neither a zip nor an .MPQ")
    return {"kind": "files", "from": "release-asset", "url": url, "rev": tag, "sha256": sha_file(path),
            "size": os.path.getsize(path), "file_count": count, "strip_prefix": prefix, "files": files,
            "files_digest": files_digest(files)}


def download(url: str, dest: str) -> None:
    req = urllib.request.Request(url, headers={"User-Agent": "Leyline-looks/1"})
    with urllib.request.urlopen(req, timeout=60) as r, open(dest, "wb") as f:   # noqa: S310 (https only, checked)
        total = 0
        for chunk in iter(lambda: r.read(1 << 20), b""):
            total += len(chunk)
            if total > MAX_DOWNLOAD:
                raise SystemExit("the release file is larger than one client patch")
            f.write(chunk)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Edit the Looks list (stream/looks.json).")
    ap.add_argument("--file", dest="list_file", default=FILE, help=argparse.SUPPRESS)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("check")
    sub.add_parser("list")
    a = sub.add_parser("approve")
    a.add_argument("id")
    a.add_argument("--by", required=True)
    a.add_argument("--notes", default="")
    u = sub.add_parser("unapprove")
    u.add_argument("id")
    p = sub.add_parser("pin")
    p.add_argument("id")
    p.add_argument("--url", required=True)
    p.add_argument("--file", dest="local", default="")
    args = ap.parse_args(argv)
    lk = rules()
    doc = read(args.list_file)

    if args.cmd == "check":
        bad = 0
        for e in doc.get("looks", []):
            for x in lk.problems(e):
                bad += 1
                print(f"{e.get('id')}: {x}")
        ids = [e.get("id") for e in doc.get("looks", [])]
        if len(ids) != len(set(ids)):
            bad += 1
            print("the same id twice")
        print("ok" if not bad else f"{bad} problem(s)")
        return 1 if bad else 0
    if args.cmd == "list":
        ups = lk.upstream_ids()
        for e in doc.get("looks", []):
            p = lk.problems(e)
            state = "BAD" if p else ("published" if lk.reviewed(e) else "waiting")
            way = "-" if p else lk.how(e, ups)
            print(f"{e.get('id'):<24} {state:<9} {way:<8} {e.get('licence') or 'no licence':<12} {e.get('name')}")
        return 0
    e = find(doc, args.id)
    if args.cmd == "approve":
        p = lk.problems(e)
        if p:
            raise SystemExit("not approved, fix these first:\n  " + "\n  ".join(p))
        e["review"] = {"by": args.by.strip(), "at": now(), "notes": args.notes.strip()}
    elif args.cmd == "unapprove":
        e["review"] = {"by": "", "at": "", "notes": e.get("review", {}).get("notes", "")}
    elif args.cmd == "pin":
        if e.get("look", {}).get("type") != "graphics-pack":
            raise SystemExit("only a graphics pack is pinned (a UI look installs through curated addons, a preset is ours)")
        tag = args.url.split("/releases/download/", 1)[-1].split("/", 1)[0] if "/releases/download/" in args.url else ""
        with tempfile.TemporaryDirectory(prefix="looks-pin-") as d:
            path = args.local
            if not path:
                if not args.url.startswith("https://github.com/"):
                    raise SystemExit("only GitHub release files over https")
                path = os.path.join(d, "download")
                download(args.url, path)
            inst = block_for(args.url, path, tag, lk.files_digest)
        trial = dict(e, install=inst)
        p = lk.problems(trial)
        if p:
            raise SystemExit("not pinned:\n  " + "\n  ".join(p))
        e["install"] = inst
        e["size"] = inst["size"]
        e["review"] = {"by": "", "at": "", "notes": f"pinned {now()}: needs a review"}
        print(f"pinned {args.id}: {len(inst['files'])} patch file(s), {inst['size']} bytes, sha256 {inst['sha256'][:12]}… (waits for a review)")
    write(args.list_file, doc)
    return 0


if __name__ == "__main__":
    sys.exit(main())
