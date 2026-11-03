#!/usr/bin/env python3
"""The kill switch's server half (docs/KILL-SWITCH.md). Leyline Control copies it to /srv/wrath/killswitch/ over SSH
and runs it with python3; the message and choices come as JSON on stdin. Standard library only.

    python3 ks_server.py status
    python3 ks_server.py pause  [--dry-run]   < {"id", "title", "message", "parts": ["message", "site", "realms"]}
    python3 ks_server.py resume [--dry-run]   < {"id"}
    python3 ks_server.py backup [--dry-run]
    python3 ks_server.py wipe   [--dry-run]   < {"sha256"}

It reads and writes only: portal-data/leyline/{shutdown,news,version-policy}.json, portal-data/admin/relay-block.json
(the relay polls it), its own folder (the Undo state, backups/), and SOAP through soap-cmd.sh (announce, the message
of the day). It never touches Caddy, docker compose, rollback.sh or DNS. Every answer ends with one line
"KS-JSON {...}" that Control reads, then "KS OK" (exit 0) or "KS FAILED: why" (exit 1).
"""
import datetime as _dt
import hashlib
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

WU = os.environ.get("KS_WU_DIR", "/srv/wrath")
DATA = os.environ.get("KS_PORTAL_DATA") or os.path.join(WU, "portal-data")
HOME = os.environ.get("KS_HOME") or os.path.join(WU, "killswitch")
SOAP = os.environ.get("KS_SOAP") or os.path.join(WU, "soap-cmd.sh")
REALMS_URL = os.environ.get("KS_REALMS_URL", "https://leylinelauncher.com/wrath/realms.json")
REALM_ID_RE = re.compile(r"^[a-z2-7]{16}$")
ID_RE = re.compile(r"^[0-9A-Za-z-]{6,40}$")
# what a wipe leaves: the farewell page, the news post and the launcher's update notice
KEEP_ON_WIPE = ("leyline/shutdown.json", "leyline/news.json", "leyline/version-policy.json")
BACKUP_FRESH = 3600


class Refused(Exception):
    pass


def say(*a):
    print(*a, flush=True)


def lpath(name):
    return os.path.join(DATA, "leyline", name)


def apath(name):
    return os.path.join(DATA, "admin", name)


def iso(t=None):
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() if t is None else t))


def read_json(path, default):
    try:
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
        return doc if isinstance(doc, type(default)) else default
    except (OSError, ValueError):
        return default


def write_json(path, doc, dry, mode=0o644):
    """Atomic, and a file that is already there keeps its owner and mode (the portal's container reads them)."""
    if dry:
        say(f"  (dry run) would write {path}")
        return
    d = os.path.dirname(path)
    os.makedirs(d, exist_ok=True)
    try:
        st = os.stat(path)
    except OSError:
        st = None
    fd, tmp = tempfile.mkstemp(dir=d, prefix=".ks-")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(doc, f, indent=1, ensure_ascii=False)
            f.write("\n")
            f.flush()
            os.fsync(f.fileno())
        os.chmod(tmp, st.st_mode & 0o777 if st else mode)
        if st and hasattr(os, "chown"):
            try:
                os.chown(tmp, st.st_uid, st.st_gid)
            except OSError:
                pass
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def remove(path, dry):
    if dry:
        say(f"  (dry run) would remove {path}")
        return
    try:
        os.unlink(path)
    except FileNotFoundError:
        pass


def one_line(text, n):
    return " ".join(str(text or "").split())[:n]


def paragraphs(text, n=8, width=200):
    out = []
    for para in re.split(r"\n\s*\n|\n", str(text or "")):
        p = one_line(para, width)
        if p:
            out.append(p)
    return out[:n]


# ----------------------------------------------------------------------------------------------------- reading
def soap(cmd):
    if not os.path.exists(SOAP):
        return False, "soap-cmd.sh is not there"
    try:
        p = subprocess.run([SOAP, cmd], capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired) as e:
        return False, type(e).__name__
    return p.returncode == 0, one_line(p.stdout or p.stderr, 200)


def realm_ids():
    """Every realm id the network lists (official and community)."""
    try:
        with urllib.request.urlopen(urllib.request.Request(REALMS_URL, headers={"User-Agent": "Leyline-killswitch"}),
                                    timeout=15) as r:
            doc = json.loads(r.read(4_000_000).decode("utf-8"))
    except Exception as e:   # noqa: BLE001
        raise Refused(f"couldn't read the realm list ({type(e).__name__})")
    rows = doc.get("realms") if isinstance(doc, dict) else None
    return sorted({str(r.get("id")) for r in rows or [] if isinstance(r, dict) and REALM_ID_RE.fullmatch(str(r.get("id")))})


def active_state():
    st = read_json(os.path.join(HOME, "active.json"), {})
    sid = st.get("id")
    if not (isinstance(sid, str) and ID_RE.fullmatch(sid)):
        return None
    return read_json(os.path.join(HOME, f"state-{sid}.json"), {}) or None


def data_bytes():
    total = 0
    for root, _dirs, files in os.walk(DATA):
        for f in files:
            try:
                total += os.path.getsize(os.path.join(root, f))
            except OSError:
                pass
    return total


def cmd_status(_payload, _dry):
    news = read_json(lpath("news.json"), {})
    posts = news.get("posts") if isinstance(news.get("posts"), list) else []
    rb = read_json(apath("relay-block.json"), {})
    try:
        ids = realm_ids()
        realms_err = ""
    except Refused as e:
        ids, realms_err = [], str(e)
    ok, soap_said = soap("server info")
    st = active_state()
    out = {"shutdown": read_json(lpath("shutdown.json"), {}) or None,
           "news_posts": len(posts), "news_top": (posts[0] or {}).get("id") if posts and isinstance(posts[0], dict) else None,
           "version_notice": str(read_json(lpath("version-policy.json"), {}).get("notice") or ""),
           "nodes_off": rb.get("nodes_off") is True,
           "blocked": len(rb.get("blocked") or {}) if isinstance(rb.get("blocked"), dict) else 0,
           "realms": ids, "realms_error": realms_err, "soap": ok, "soap_said": soap_said if not ok else "",
           "data_dir": DATA, "data_exists": os.path.isdir(DATA), "data_bytes": data_bytes(),
           "active": {"id": st.get("id"), "at": st.get("at"), "parts": st.get("parts")} if st and not st.get("resumed") else None}
    for k, v in out.items():
        if k != "realms":
            say(f"  {k}: {v}")
    say(f"  realms: {len(ids)}")
    return out


# ----------------------------------------------------------------------------------------------------- pause / resume
def check_payload(p, need_message=True):
    if not isinstance(p, dict) or not ID_RE.fullmatch(str(p.get("id") or "")):
        raise Refused("no run id")
    if need_message and not one_line(p.get("message"), 10):
        raise Refused("no message")
    return p


def cmd_pause(p, dry):
    check_payload(p)
    parts = [x for x in p.get("parts") or [] if x in ("message", "site", "realms")]
    if not parts:
        raise Refused("no server parts ticked")
    sid = p["id"]
    st = active_state()
    if st and st.get("resumed"):
        st = None
    if st:   # paused before (a second run after a problem): what it saved first is what Undo puts back
        say(f"  an earlier pause ({st.get('id')}) is still on: its saved state is kept and used for Undo")
    before = dict((st or {}).get("before") or {})
    done, problems = [], []
    title = one_line(p.get("title") or "Leyline is closing", 80)
    message = str(p.get("message") or "")[:1800]
    today = time.strftime("%Y-%m-%d")

    shutdown = read_json(lpath("shutdown.json"), {}) or None
    before.setdefault("shutdown", shutdown)
    mode = "closed" if "site" in parts else "banner"
    if "message" in parts or "site" in parts:
        doc = {"format": 1, "mode": mode, "title": title, "message": message, "at": iso(), "id": sid}
        write_json(lpath("shutdown.json"), doc, dry)
        done.append(f"website: {'the farewell page on every page' if mode == 'closed' else 'a banner on every page'}")

    if "message" in parts:
        news = read_json(lpath("news.json"), {})
        posts = [x for x in (news.get("posts") if isinstance(news.get("posts"), list) else []) if isinstance(x, dict)]
        pid = f"farewell-{sid}"
        posts = [x for x in posts if x.get("id") != pid]
        posts.insert(0, {"id": pid, "date": today, "tag": "Notice", "title": title, "items": paragraphs(message)})
        write_json(lpath("news.json"), {"format": 1, "updated_at": iso(), "posts": posts[:200]}, dry)
        before.setdefault("news_post", pid)
        done.append("launcher: the News post")
        pol = read_json(lpath("version-policy.json"), {})
        before.setdefault("version_notice", str(pol.get("notice") or ""))
        write_json(lpath("version-policy.json"), dict(pol, notice=one_line(message, 200), by="killswitch", at=iso()), dry)
        done.append("launcher: the update notice")
        line = one_line(message, 200)
        for what, cmd in (("announce", f"announce {line}"), ("message of the day", f"server set motd {line}")):
            if dry:
                say(f"  (dry run) would send the realm: {what}")
                continue
            ok, said = soap(cmd)
            (done if ok else problems).append(f"realm: {what}" + ("" if ok else f" did not answer ({said})"))

    if "realms" in parts:
        ids = realm_ids()
        rb = read_json(apath("relay-block.json"), {})
        blocked = dict(rb.get("blocked")) if isinstance(rb.get("blocked"), dict) else {}
        before.setdefault("nodes_off", rb.get("nodes_off") is True)
        added = list(before.get("blocked_added") or [])
        for rid in ids:
            if rid not in blocked:
                blocked[rid] = {"at": iso(), "by": "killswitch", "reason": "Leyline shut down"}
                added.append(rid)
        before["blocked_added"] = sorted(set(added))
        write_json(apath("relay-block.json"), dict(rb, format=1, updated_at=iso(), nodes_off=True, blocked=blocked), dry,
                   0o640)
        done.append(f"relay: every node stopped, {len(ids)} realm(s) blocked")

    state = {"id": sid, "at": iso(), "parts": sorted(set(parts) | set((st or {}).get("parts") or [])),
             "before": before, "done": done, "problems": problems, "resumed": False}
    if not dry:
        write_json(os.path.join(HOME, f"state-{sid}.json"), state, False, 0o600)
        write_json(os.path.join(HOME, "active.json"), {"id": sid, "at": iso()}, False, 0o600)
    for d in done:
        say(f"  done: {d}")
    for x in problems:
        say(f"  problem: {x}")
    return {"done": done, "problems": problems, "parts": state["parts"]}


def cmd_resume(p, dry):
    check_payload(p, need_message=False)
    st = active_state()
    if not st or st.get("resumed"):
        raise Refused("nothing is paused")
    b = st.get("before") or {}
    done, manual = [], []
    if b.get("shutdown"):
        write_json(lpath("shutdown.json"), b["shutdown"], dry)
    else:
        remove(lpath("shutdown.json"), dry)
    done.append("website: the farewell page and banner off")
    if b.get("news_post"):
        news = read_json(lpath("news.json"), {})
        posts = [x for x in (news.get("posts") if isinstance(news.get("posts"), list) else [])
                 if isinstance(x, dict) and not str(x.get("id", "")).startswith("farewell-")]
        write_json(lpath("news.json"), dict(news, format=1, updated_at=iso(), posts=posts), dry)
        done.append("launcher: the News post taken back")
    if "version_notice" in b:
        pol = read_json(lpath("version-policy.json"), {})
        write_json(lpath("version-policy.json"), dict(pol, notice=b["version_notice"], by="killswitch", at=iso()), dry)
        done.append("launcher: the update notice put back")
        manual.append("realm: set the message of the day back (the realm's config, or .server set motd)")
    if "nodes_off" in b or b.get("blocked_added"):
        rb = read_json(apath("relay-block.json"), {})
        blocked = dict(rb.get("blocked")) if isinstance(rb.get("blocked"), dict) else {}
        for rid in b.get("blocked_added") or []:
            blocked.pop(rid, None)
        write_json(apath("relay-block.json"), dict(rb, format=1, updated_at=iso(), nodes_off=bool(b.get("nodes_off")),
                                                   blocked=blocked), dry, 0o640)
        done.append(f"relay: nodes back as they were, {len(b.get('blocked_added') or [])} block(s) taken off")
    if not dry:
        st.update(resumed=True, resumed_at=iso())
        write_json(os.path.join(HOME, f"state-{st['id']}.json"), st, False, 0o600)
        remove(os.path.join(HOME, "active.json"), False)
    for d in done:
        say(f"  done: {d}")
    return {"done": done, "manual": manual, "id": st["id"]}


# ----------------------------------------------------------------------------------------------------- backup / wipe
def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def cmd_backup(_p, dry):
    if not os.path.isdir(DATA):
        raise Refused(f"{DATA} is not there")
    out_dir = os.path.join(HOME, "backups")
    name = f"portal-data-{_dt.datetime.now().strftime('%Y%m%d-%H%M%S')}.tar.gz"
    path = os.path.join(out_dir, name)
    if dry:
        say(f"  (dry run) would pack {DATA} ({data_bytes()} bytes) into {path}")
        return {"path": path, "sha256": "", "bytes": 0, "dry": True}
    os.makedirs(out_dir, mode=0o700, exist_ok=True)
    with tarfile.open(path, "w:gz") as tar:
        tar.add(DATA, arcname="portal-data")
    os.chmod(path, 0o600)
    sha = sha256_file(path)
    with open(path + ".sha256", "w", encoding="utf-8") as f:
        f.write(f"{sha}  {name}\n")
    size = os.path.getsize(path)
    say(f"  packed {name}: {size} bytes, sha256 {sha}")
    return {"path": path, "sha256": sha, "bytes": size}


def cmd_wipe(p, dry):
    want = str((p or {}).get("sha256") or "")
    if not re.fullmatch(r"[0-9a-f]{64}", want):
        raise Refused("no backup sha256 given")
    if read_json(lpath("shutdown.json"), {}).get("mode") != "closed":
        raise Refused("the website is not paused (no farewell page): pause it first")
    out_dir = os.path.join(HOME, "backups")
    found = None
    for f in sorted(os.listdir(out_dir)) if os.path.isdir(out_dir) else []:
        full = os.path.join(out_dir, f)
        if f.endswith(".tar.gz") and time.time() - os.path.getmtime(full) < BACKUP_FRESH and sha256_file(full) == want:
            found = full
    if not found:
        raise Refused("no backup from the last hour with that sha256: back up first")
    keep = {os.path.normpath(os.path.join(DATA, k)) for k in KEEP_ON_WIPE}
    removed = 0
    for root, dirs, files in os.walk(DATA, topdown=False):
        for f in files:
            full = os.path.normpath(os.path.join(root, f))
            if full in keep:
                continue
            removed += 1
            remove(full, dry)
        for d in dirs:
            full = os.path.join(root, d)
            if not dry:
                try:
                    os.rmdir(full)
                except OSError:
                    pass   # not empty: it holds a kept file
    say(f"  {'would remove' if dry else 'removed'} {removed} file(s); kept the farewell page, the news and the notice")
    return {"removed": removed, "backup": os.path.basename(found), "dry": dry}


COMMANDS = {"status": cmd_status, "pause": cmd_pause, "resume": cmd_resume, "backup": cmd_backup, "wipe": cmd_wipe}


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    dry = "--dry-run" in argv
    argv = [a for a in argv if a != "--dry-run"]
    if not argv or argv[0] not in COMMANDS:
        say(__doc__.strip().splitlines()[0])
        say("KS FAILED: unknown command")
        return 2
    payload = {}
    if argv[0] in ("pause", "resume", "wipe"):
        raw = sys.stdin.read(64_000)
        try:
            payload = json.loads(raw) if raw.strip() else {}
        except ValueError:
            say("KS FAILED: bad JSON on stdin")
            return 2
    try:
        out = COMMANDS[argv[0]](payload, dry)
    except Refused as e:
        say("KS-JSON " + json.dumps({"error": str(e)}))
        say(f"KS FAILED: {e}")
        return 1
    say("KS-JSON " + json.dumps(out, ensure_ascii=False))
    say("KS OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
