#!/usr/bin/env python3
"""Our own copy of every Leyline release, so installed launchers keep updating and people can keep hosting when
GitHub is down (docs/RESILIENCE.md). The Leyline Control step "release-mirror", run after a ship.

    py -3 tools\\release-mirror\\release_mirror.py check            what it would copy (read-only)
    py -3 tools\\release-mirror\\release_mirror.py build --out DIR  fetch and stage the new files in DIR
    py -3 tools\\release-mirror\\release_mirror.py upload           build, then put them on the server (SSH)

What it copies, from the public GitHub releases of the Leyline repo (no sign-in needed) and from the upstream
projects at the commits our host profiles pin:
  * the newest launcher releases (tags launcher-v*, KEEP_LAUNCHERS of them): every file, and latest.json with its
    downloads pointed at our mirror (the signatures inside are untouched: they are over the files, and every
    launcher checks them against its built-in key, so the mirror can't push a bad build);
  * the Host pack (release host-pack, from its hostpack.json) and the Leyline addons (release leyline-addons);
  * the upstream server SOURCE at each pin in hosting/kit/profiles/*/profile.lock (LL_CORE_REPO/LL_CORE_COMMIT,
    LL_DB_REPO/LL_DB_COMMIT): the project's own tarball of that commit, checked inside: a tarball with anything
    on the refusal list (game data, a game client) is refused and nothing of it is copied.
Each copied file gets its sha256 (index.json, SHA256SUMS) and a .torrent (web seeds: our mirror, then GitHub).

The release log ("the chain", hosting/kit/tools/wunet/releaselog.py): when the release key file is there
(LEYLINE_RELEASE_KEY, default %USERPROFILE%\\.leyline\\release-log.key), one signed entry per new item is added to
the owner's copy (beside the key: release-log-chain.json) and published with the mirror as chain.json. No key yet:
the mirror still goes up and the log waits.

Server layout: /srv/wrath/portal-data/leyline/releases (wu-portal's /data/leyline/releases), served at
https://play.openvanilla.app/wrath/leyline/releases/ (stream/leyline_net.py release_file). Only new files are
uploaded (the live index.json says what is there). Never prints a secret. Standard library only.
Exit codes: 0 ok, 1 failed or refused, 2 usage.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "hosting", "kit", "tools"))
from wunet import keys, releaselog  # noqa: E402

BRANDING = os.path.join(ROOT, "launcher", "branding.json")
PROFILES = os.path.join(ROOT, "hosting", "kit", "profiles")
KEEP_LAUNCHERS = 3
PIECE = 256 * 1024
MAX_FILE = 1024 * 1024 * 1024
UA = "leyline-release-mirror/1"
HOST = "root@play.openvanilla.app"
REMOTE = "/srv/wrath/portal-data/leyline/releases"
_TAG = re.compile(r"^launcher-v([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.]+)?)$")
_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]{0,119}$")
_HEX40 = re.compile(r"^[0-9a-f]{40}$")


class Refused(Exception):
    pass


def branding() -> dict:
    with open(BRANDING, encoding="utf-8") as f:
        return json.load(f)


def default_key_path() -> str:
    return os.environ.get("LEYLINE_RELEASE_KEY") or os.path.join(os.path.expanduser("~"), ".leyline", "release-log.key")


# ------------------------------------------------------------------------------------------------ fetching
def http_get(url: str, timeout: float = 60) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/json, */*"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        data = r.read(MAX_FILE + 1)
    if len(data) > MAX_FILE:
        raise Refused(f"{url}: more than {MAX_FILE >> 20} MB")
    return data


def version_key(v: str):
    core, _, pre = v.partition("-")
    nums = tuple(int(x) for x in core.split("."))
    return nums + ((1,) if not pre else (0, pre))


def refusal(name: str):
    return releaselog.refused_name(name)


def check_tarball(data: bytes, name: str) -> None:
    """Refuse a source tarball with game data, a game client, links or odd entries inside."""
    try:
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as t:
            n = 0
            for m in t:
                n += 1
                if n > 200_000:
                    raise Refused(f"{name}: too many files")
                base = m.name.rsplit("/", 1)[-1]
                why = refusal(base)
                if why:
                    raise Refused(f"{name}: {why} inside")
                if m.issym() or m.islnk():
                    continue                     # upstream trees have symlinks; the launcher never unpacks these
                if not (m.isfile() or m.isdir() or m.type in (tarfile.XHDTYPE, tarfile.XGLTYPE)):
                    raise Refused(f"{name}: an odd entry inside ({m.name})")
                if m.isfile() and m.size:
                    head = t.extractfile(m).read(4)
                    if head[:4] in (b"MPQ\x1a", b"MPQ\x1b", b"BLP1", b"BLP2", b"MD20", b"MD21", b"WDBC", b"WDB2", b"REVM"):
                        raise Refused(f"{name}: game data inside ({m.name}, by its contents)")
    except (tarfile.TarError, OSError, EOFError) as e:
        raise Refused(f"{name}: not a readable tarball ({e})")


def bencode(v) -> bytes:
    if isinstance(v, int):
        return b"i%de" % v
    if isinstance(v, str):
        v = v.encode()
    if isinstance(v, bytes):
        return b"%d:%s" % (len(v), v)
    if isinstance(v, list):
        return b"l" + b"".join(bencode(x) for x in v) + b"e"
    return b"d" + b"".join(bencode(k) + bencode(v[k]) for k in sorted(v)) + b"e"


def make_torrent(name: str, data: bytes, web_seeds: list[str]) -> tuple[bytes, str]:
    """The same single-file torrent as launcher/src-tauri/src/p2p/share.rs torrent_of_file (256 KiB pieces)."""
    pieces = b"".join(hashlib.sha1(data[i:i + PIECE]).digest() for i in range(0, len(data), PIECE))
    info = {"length": len(data), "name": name, "piece length": PIECE, "pieces": pieces}
    top = {"info": info}
    if web_seeds:
        top["url-list"] = web_seeds
    return bencode(top), hashlib.sha1(bencode(info)).hexdigest()


# ------------------------------------------------------------------------------------------------ what to copy
def profile_pins(profiles_dir: str = PROFILES) -> list[dict]:
    """[{repo, commit, profile}] from every profile.lock (KEY=VALUE lines; nothing in them is run)."""
    out, seen = [], set()
    for prof in sorted(os.listdir(profiles_dir)) if os.path.isdir(profiles_dir) else []:
        p = os.path.join(profiles_dir, prof, "profile.lock")
        if not os.path.isfile(p):
            continue
        kv = {}
        with open(p, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    k, v = line.split("=", 1)
                    kv[k.strip()] = v.strip()
        for rk, ck in (("LL_CORE_REPO", "LL_CORE_COMMIT"), ("LL_DB_REPO", "LL_DB_COMMIT")):
            repo, commit = kv.get(rk, ""), kv.get(ck, "").lower()
            m = re.match(r"^https://github\.com/([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+?)(?:\.git)?/?$", repo)
            if m and _HEX40.match(commit) and (repo, commit) not in seen:
                seen.add((repo, commit))
                out.append({"repo": repo, "owner": m.group(1), "name": m.group(2), "commit": commit, "profile": prof})
    return out


def plan(releases: list, gh_download: str, pins: list[dict], keep: int = KEEP_LAUNCHERS) -> list[dict]:
    """Items to mirror: {kind, name, version, dir, files: [{name, url}], source?}."""
    items = []
    launchers = []
    for r in releases if isinstance(releases, list) else []:
        if not isinstance(r, dict) or r.get("draft"):
            continue
        tag = r.get("tag_name") or ""
        assets = [a for a in r.get("assets") or [] if isinstance(a, dict) and _NAME.match(str(a.get("name", "")))]
        names = {a["name"] for a in assets}
        m = _TAG.match(tag)
        if m and "latest.json" in names:
            launchers.append((m.group(1), tag, assets))
        elif tag == "host-pack":
            zips = sorted((a["name"] for a in assets if a["name"].startswith("leyline-host-pack-") and a["name"].endswith(".zip")),
                          key=lambda n: version_key(n[len("leyline-host-pack-"):-4]) if re.match(r"^leyline-host-pack-[0-9.]+\.zip$", n) else (0,))
            if zips:
                z = zips[-1]
                ver = z[len("leyline-host-pack-"):-4]
                items.append({"kind": "hostpack", "name": "Host pack", "version": ver, "dir": "host-pack",
                              "files": [{"name": z, "url": f"{gh_download}{tag}/{z}"}]})
        elif tag == "leyline-addons":
            zips = sorted((a["name"] for a in assets if re.match(r"^leyline-addons-[0-9.]+\.zip$", a["name"])),
                          key=lambda n: version_key(n[len("leyline-addons-"):-4]))
            if zips:
                z = zips[-1]
                items.append({"kind": "addons", "name": "Leyline addons", "version": z[len("leyline-addons-"):-4], "dir": "addons",
                              "files": [{"name": z, "url": f"{gh_download}{tag}/{z}"}]})
    launchers.sort(key=lambda x: version_key(x[0]), reverse=True)
    for ver, tag, assets in launchers[:keep]:
        files = [{"name": a["name"], "url": f"{gh_download}{tag}/{a['name']}"} for a in assets
                 if not refusal(a["name"])]
        items.append({"kind": "launcher", "name": "Leyline", "version": ver, "dir": tag, "files": files})
    for p in pins:
        fname = f"{p['name']}-{p['commit']}.tar.gz"
        items.append({"kind": "source", "name": p["name"], "version": p["commit"][:12], "dir": "sources",
                      "files": [{"name": fname, "url": f"https://codeload.github.com/{p['owner']}/{p['name']}/tar.gz/{p['commit']}"}],
                      "source": {"repo": p["repo"], "commit": p["commit"]}})
    return items


def rewrite_manifest(data: bytes, gh_download: str, mirror: str) -> bytes:
    """latest.json with every download pointed at the mirror (same path below it); the signatures are kept."""
    doc = json.loads(data)
    for k, a in (doc.get("platforms") or {}).items():
        u = a.get("url", "")
        if not u.startswith(gh_download):
            raise Refused(f"latest.json: {k} downloads from {u}, not from the release")
        a["url"] = mirror + u[len(gh_download):]
    return (json.dumps(doc, indent=1, ensure_ascii=False) + "\n").encode("utf-8")


# ------------------------------------------------------------------------------------------------ build
def build(out: str, *, api: str, gh_download: str, mirror: str, have: dict, pins: list[dict], key_path: str | None,
          chain_path: str | None, server_chain=None, fetch=http_get, now: int | None = None, log=print) -> dict:
    """Stage new files under out/releases/<dir>/<name>, plus index.json, SHA256SUMS, latest.json, chain.json.
    have: the live index's files ({path: {size, sha256}}). Returns a report."""
    rel = os.path.join(out, "releases")
    os.makedirs(rel, exist_ok=True)
    releases = json.loads(fetch(api))
    items = plan(releases, gh_download, pins)
    index = dict(have)
    new_files, done = [], []
    newest_launcher = None
    for it in items:
        refs = []
        for f in it["files"]:
            path = f"{it['dir']}/{f['name']}"
            if refusal(f["name"]):
                raise Refused(f"{f['name']}: {refusal(f['name'])}")
            if path in have:                                     # already on the mirror: never fetched again
                refs.append({"name": f["name"], "size": have[path]["size"], "sha256": have[path]["sha256"]})
                continue
            data = fetch(f["url"])
            if it["kind"] == "source":
                check_tarball(data, f["name"])
            if f["name"] == "latest.json":
                data = rewrite_manifest(data, gh_download, mirror)
            os.makedirs(os.path.join(rel, it["dir"]), exist_ok=True)
            with open(os.path.join(rel, it["dir"], f["name"]), "wb") as w:
                w.write(data)
            ref = {"name": f["name"], "size": len(data), "sha256": hashlib.sha256(data).hexdigest()}
            refs.append(ref)
            index[path] = {"size": ref["size"], "sha256": ref["sha256"]}
            new_files.append(path)
            # a .torrent for each payload (web seeds: our mirror, then where it came from)
            if not f["name"].endswith((".json", ".sig", ".torrent")):
                t, ih = make_torrent(f["name"], data, [f"{mirror}{path}", f["url"]])
                tname = f["name"] + ".torrent"
                with open(os.path.join(rel, it["dir"], tname), "wb") as w:
                    w.write(t)
                index[f"{it['dir']}/{tname}"] = {"size": len(t), "sha256": hashlib.sha256(t).hexdigest(), "infohash": ih}
                index[path]["infohash"] = ih
                new_files.append(f"{it['dir']}/{tname}")
        it["refs"] = refs
        done.append(it)
        if it["kind"] == "launcher" and (newest_launcher is None or version_key(it["version"]) > version_key(newest_launcher["version"])):
            newest_launcher = it
    # the top latest.json: the newest launcher's (already rewritten)
    if newest_launcher:
        src = os.path.join(rel, newest_launcher["dir"], "latest.json")
        if not os.path.isfile(src):
            data = rewrite_manifest(fetch(next(f["url"] for f in newest_launcher["files"] if f["name"] == "latest.json")), gh_download, mirror)
            os.makedirs(os.path.dirname(src), exist_ok=True)
            with open(src, "wb") as w:
                w.write(data)
        shutil.copyfile(src, os.path.join(rel, "latest.json"))
        with open(src, "rb") as r:
            b = r.read()
        index["latest.json"] = {"size": len(b), "sha256": hashlib.sha256(b).hexdigest()}
    # launcher versions no longer kept leave the index (the server removes their folders)
    keep_dirs = {it["dir"] for it in done}
    index = {p: m for p, m in index.items() if "/" not in p or p.split("/", 1)[0] in keep_dirs}
    # the release log
    chain_note = "no release key yet: the release log waits (docs/RESILIENCE.md)"
    chain = None
    if key_path and os.path.isfile(key_path):
        secret = releaselog.read_key(key_path)
        pub = keys.public_key(secret).hex()
        want_pub = (branding().get("release_log") or {}).get("public_key") or ""
        if want_pub and want_pub != pub:
            raise Refused("the release key on this PC is not the one launcher/branding.json trusts")
        chain = releaselog.load(chain_path) if chain_path else None
        chain = chain if chain and chain.get("entries") is not None else releaselog.empty(pub)
        releaselog.verify(chain, pub)
        if server_chain:
            chain, what = releaselog.choose(chain, server_chain, pub)
            if what.startswith("refused"):
                log(f"  the server's chain.json was not taken ({what})")
        bodies = releaselog.verify(chain, pub)
        known = {(b["kind"], b["name"], b["version"], tuple(sorted(f["sha256"] for f in b["files"]))) for b in bodies}
        added = 0
        for it in done:
            files = sorted(it["refs"], key=lambda r: r["name"])
            k = (it["kind"], it["name"], it["version"], tuple(sorted(f["sha256"] for f in files)))
            if k in known or not files:
                continue
            payloads = [f for f in files if not f["name"].endswith((".json", ".sig", ".torrent"))]
            ih = index.get(f"{it['dir']}/{payloads[0]['name']}", {}).get("infohash") if len(payloads) == 1 else None
            chain = releaselog.append(chain, secret, kind=it["kind"], name=it["name"], version=it["version"], files=files,
                                      mirrors=[mirror], infohash=ih, source=it.get("source"), now=now)
            added += 1
        if chain_path:
            releaselog.save(chain_path, chain)
        with open(os.path.join(rel, "chain.json"), "w", encoding="utf-8") as w:
            json.dump(chain, w, ensure_ascii=False, indent=0)
        chain_note = f"release log: {added} new entries, {len(chain['entries'])} in all, head {releaselog.head(chain)[:16]}"
    with open(os.path.join(rel, "index.json"), "w", encoding="utf-8") as w:
        json.dump({"format": 1, "updated": int(time.time() if now is None else now), "files": index}, w, indent=1, sort_keys=True)
    with open(os.path.join(rel, "SHA256SUMS"), "w", encoding="utf-8") as w:
        for p in sorted(index):
            w.write(f"{index[p]['sha256']}  {p}\n")
    return {"items": [(it["kind"], it["name"], it["version"]) for it in done], "new": new_files, "chain": chain_note,
            "latest": newest_launcher["version"] if newest_launcher else None, "chain_doc": chain}


# ------------------------------------------------------------------------------------------------ upload
def ssh_opts(key: str) -> list[str]:
    return ["-i", key, "-o", "StrictHostKeyChecking=accept-new", "-o", "ServerAliveInterval=30"]


def upload(out: str, host: str, key: str, run=subprocess.run) -> None:
    tgz = os.path.join(out, "release-mirror.tgz")
    shutil.copyfile(os.path.join(os.path.dirname(os.path.abspath(__file__)), "install-release-mirror.sh"),
                    os.path.join(out, "install-release-mirror.sh"))
    with tarfile.open(tgz, "w:gz") as t:
        t.add(os.path.join(out, "install-release-mirror.sh"), "install-release-mirror.sh")
        t.add(os.path.join(out, "releases"), "releases")
    r = run(["scp", *ssh_opts(key), tgz, f"{host}:/tmp/leyline-release-mirror.tgz"])
    if r.returncode:
        raise Refused("the upload failed (scp). If it timed out, wait 2 minutes and run it again.")
    cmd = ("rm -rf /tmp/leyline-release-mirror && mkdir -p /tmp/leyline-release-mirror && "
           "tar -xzf /tmp/leyline-release-mirror.tgz -C /tmp/leyline-release-mirror && rm -f /tmp/leyline-release-mirror.tgz && "
           "sed -i 's/\\r$//' /tmp/leyline-release-mirror/install-release-mirror.sh && "
           "sh /tmp/leyline-release-mirror/install-release-mirror.sh; r=$?; rm -rf /tmp/leyline-release-mirror; exit $r")
    r = run(["ssh", *ssh_opts(key), host, cmd])
    if r.returncode:
        raise Refused("the server did not report success (see above); it keeps the mirror it had.")


def github_chain(out: str, slug: str, run=subprocess.run) -> str:
    """Attach chain.json to the public release "release-log" (made once) with the owner's gh sign-in."""
    p = os.path.join(out, "releases", "chain.json")
    if not os.path.isfile(p):
        return "no chain.json to publish"
    if shutil.which("gh") is None:
        return "gh is not installed: chain.json is on our server only"
    if run(["gh", "release", "view", "release-log", "-R", slug], capture_output=True).returncode:
        run(["gh", "release", "create", "release-log", "-R", slug, "--title", "Leyline release log",
             "--notes", "The signed release log (docs/RESILIENCE.md). Launchers check every entry against the release key.",
             "--latest=false"], capture_output=True)
    r = run(["gh", "release", "upload", "release-log", p, "-R", slug, "--clobber"], capture_output=True)
    return "chain.json on GitHub" if r.returncode == 0 else "could not put chain.json on GitHub (gh)"


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("cmd", choices=("check", "build", "upload"))
    ap.add_argument("--out")
    ap.add_argument("--key", default=default_key_path(), help="the release key file (default: %(default)s)")
    ap.add_argument("--host", default=HOST)
    ap.add_argument("--ssh-key", default=os.path.join(os.path.expanduser("~"), ".ssh", "openwow"))
    ap.add_argument("--no-github", action="store_true", help="do not attach chain.json to the GitHub release")
    o = ap.parse_args(argv)
    b = branding()
    mirror = b["updates"]["mirrors"][0]
    gh_download = b["updates"]["github_downloads"]
    api = b["updates"]["github_api"]
    site = b["site"].rstrip("/")
    try:
        try:
            have = json.loads(http_get(mirror + "index.json", 30)).get("files") or {}
        except Exception:                     # first run, or the mirror is down: copy everything
            have = {}
        try:
            server_chain = json.loads(http_get(site + "/wrath/leyline/chain.json", 30))
        except Exception:
            server_chain = None
        pins = profile_pins()
        if o.cmd == "check":
            items = plan(json.loads(http_get(api)), gh_download, pins)
            for it in items:
                todo = [f["name"] for f in it["files"] if f"{it['dir']}/{f['name']}" not in have]
                print(f"  {it['kind']:8} {it['name']} {it['version']}: {len(todo)} new file(s) of {len(it['files'])}")
            print(f"  release key: {'there' if os.path.isfile(o.key) else 'not made yet (docs/RESILIENCE.md)'}")
            return 0
        out = o.out or tempfile.mkdtemp(prefix="leyline-release-mirror-")
        chain_path = os.path.join(os.path.dirname(o.key), "release-log-chain.json")
        print(f"[1/3] Copying from GitHub and the upstream projects into {out} ...")
        rep = build(out, api=api, gh_download=gh_download, mirror=mirror, have=have, pins=pins, key_path=o.key,
                    chain_path=chain_path, server_chain=server_chain)
        for k, n, v in rep["items"]:
            print(f"  {k:8} {n} {v}")
        print(f"  {len(rep['new'])} new file(s); newest launcher {rep['latest']}; {rep['chain']}")
        if o.cmd == "build":
            return 0
        print("[2/3] Uploading the new files ...")
        upload(out, o.host, o.ssh_key)
        print("[3/3] " + ("chain.json not on GitHub (--no-github)" if o.no_github else github_chain(out, b["repo"])))
        shutil.rmtree(out, ignore_errors=True)
        print(f"Done: {mirror}latest.json")
    except (Refused, releaselog.Bad, OSError, ValueError) as e:
        print(f"release mirror: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
