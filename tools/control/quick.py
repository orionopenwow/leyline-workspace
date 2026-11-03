#!/usr/bin/env python3
"""Quick Claude edits: one safe Control step pair instead of a new .bat for every small change (owner 3 Nov: "quick
Claude edits instead of rigorous bat files").

Claude writes the changed files onto the PC (as always: the PC copy checked against git first), then drops a
manifest beside them in control/quick/<id>.json and queues one line in UPDATES-PENDING.txt:

    1311a apply-check  what it brings      (read-only: checks the manifest, deploys nothing; Autopilot may run it)
    1311b apply  quick:1311a  what it brings   (checks again, then runs the one deploy step it names)

The manifest is found by the queue line's id (control/quick/<id>.json), or by a "quick:<name>" first word in its
note. It says exactly which files and how they are deployed:

    {"files": [{"path": "stream/forum.py", "sha256": "<64 hex>"}], "deploy": "signin", "note": "Forum: typo"}

`apply` refuses the whole manifest (and deploys nothing) unless every file is:
  * a plain relative path with forward slashes inside the Leyline folder (no "..", no drive, no absolute path, no
    link out of the folder), and not in .git, control/, logs/, .repos/ or _local/;
  * not a secret or settings file (.env files, token.txt, keys, certificates, credentials, anything named password
    or secret) and not game data or a program (.mpq, .exe, .dll);
  * there, at most 5 MB, and byte for byte the sha256 the manifest gives (so what deploys is what Claude meant);
  * under the folders its deploy step ships: signin: stream/; addons: addons/ or stream/fui-web/; scripts:
    realm/custom/; none: anywhere (checked and logged only; the next ship carries it).
Then it runs that existing step's own .bat (the same one Control runs for "signin", "addons" or "scripts"), with its
backup, self-check and rollback, and appends one line to control/quick/applied.jsonl. It prints paths and short
hashes only, never a file's contents. Any other key, deploy name or shape is refused.

    python3 -I tools/control/quick.py check|apply [--id ID] [--note NOTE]     (Control passes both in the environment)
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[2]
QUICK = ROOT / "control" / "quick"
APPLIED = QUICK / "applied.jsonl"
MAX_FILES = 200
MAX_BYTES = 5 * 1024 * 1024
# deploy -> (the step's .bat, as in control.py STEPS, the folders it ships)
DEPLOY = {
    "signin": ("stream/Update sign-in and warm-up.bat", ("stream/",)),
    "addons": ("addons/pack/Publish Leyline addons.bat", ("addons/", "stream/fui-web/")),
    "scripts": ("realm/Server pass - finder 10 and bug reports.bat", ("realm/custom/",)),
    "none": (None, ("",)),
}
DENY_DIRS = (".git/", "control/", "logs/", ".repos/", "_local/")
SECRET = re.compile(r"(?i)(^|/)(\.env(\.[^/]*)?|[^/]*\.env|token\.txt|control-token|id_(rsa|dsa|ecdsa|ed25519)(\.pub)?|"
                    r"[^/]*\.(pem|key|pfx|p12|crt|kdbx|keystore|jks)|credentials?(\.[^/]*)?|[^/]*(password|passwd|secret)[^/]*)$")
NO_SHIP = re.compile(r"(?i)\.(mpq|exe|dll|so|dylib)$")
ID_OK = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$")
HEX64 = re.compile(r"^[0-9a-f]{64}$")


class Refused(Exception):
    pass


def manifest_name(item_id: str, note: str) -> str:
    first = (note or "").split()[:1]
    if first and first[0].startswith("quick:"):
        name = first[0][len("quick:"):]
    else:
        name = item_id or ""
    if not ID_OK.match(name):
        raise Refused(f"No manifest name: queue the line as '<id> apply' with control/quick/<id>.json beside it (got {name!r}).")
    return name


def clean_path(p) -> str:
    if not isinstance(p, str) or not p or len(p) > 260:
        raise Refused("A file path must be text, 1 to 260 characters.")
    if "\\" in p or p.startswith("/") or re.match(r"^[A-Za-z]:", p) or any(c in p for c in "\0\r\n\t*?<>|\""):
        raise Refused(f"{p!r}: a relative path with forward slashes only.")
    parts = PurePosixPath(p).parts
    if not parts or any(x in ("", ".", "..") for x in p.split("/")):
        raise Refused(f"{p!r}: no '.', '..' or empty parts.")
    if any((p + "/").startswith(d) or p == d.rstrip("/") for d in DENY_DIRS):
        raise Refused(f"{p}: not a file Control deploys (.git, control, logs, .repos, _local).")
    if SECRET.search(p):
        raise Refused(f"{p}: looks like a secret or settings file. Those are never deployed this way.")
    if NO_SHIP.search(p):
        raise Refused(f"{p}: game data or a program. Never deployed this way.")
    return p


def load(path: Path) -> dict:
    try:
        raw = path.read_bytes()
    except OSError:
        raise Refused(f"No manifest at control/quick/{path.name}.")
    if len(raw) > 256 * 1024:
        raise Refused("The manifest is over 256 KB.")
    try:
        m = json.loads(raw.decode("utf-8-sig"))
    except (ValueError, UnicodeDecodeError):
        raise Refused("The manifest is not JSON.")
    if not isinstance(m, dict) or set(m) - {"files", "deploy", "note", "id"}:
        raise Refused("The manifest takes only files, deploy, note (and id).")
    if m.get("deploy") not in DEPLOY:
        raise Refused(f"deploy must be one of {', '.join(DEPLOY)}.")
    if not isinstance(m.get("note", ""), str) or len(m.get("note", "")) > 300:
        raise Refused("note: text, at most 300 characters.")
    files = m.get("files")
    if not isinstance(files, list) or not 1 <= len(files) <= MAX_FILES:
        raise Refused(f"files: a list of 1 to {MAX_FILES}.")
    return m


def verify(m: dict, root: Path = None) -> list[dict]:
    """Every file checked (see the module's text). Returns [{path, sha256, bytes}] or raises Refused."""
    root = (root or ROOT).resolve()
    _, under = DEPLOY[m["deploy"]]
    seen, out = set(), []
    for f in m["files"]:
        if not isinstance(f, dict) or set(f) != {"path", "sha256"}:
            raise Refused("Each file is {\"path\": ..., \"sha256\": ...} and nothing else.")
        p = clean_path(f["path"])
        if p in seen:
            raise Refused(f"{p}: listed twice.")
        seen.add(p)
        want = f["sha256"]
        if not isinstance(want, str) or not HEX64.match(want):
            raise Refused(f"{p}: sha256 must be 64 lowercase hex digits.")
        if not any(p.startswith(u) for u in under):
            raise Refused(f"{p}: '{m['deploy']}' deploys only {', '.join(under)}. Use the deploy step that ships it.")
        full = root / p
        try:
            real = full.resolve(strict=True)
        except (OSError, RuntimeError):
            raise Refused(f"{p}: not on this PC.")
        if root not in real.parents:
            raise Refused(f"{p}: leads outside the Leyline folder.")
        if not real.is_file():
            raise Refused(f"{p}: not a file.")
        size = real.stat().st_size
        if size > MAX_BYTES:
            raise Refused(f"{p}: over 5 MB.")
        got = hashlib.sha256(real.read_bytes()).hexdigest()
        if got != want:
            raise Refused(f"{p}: its sha256 is {got[:12]}…, the manifest says {want[:12]}…. Not deployed: the file isn't what Claude sent.")
        out.append({"path": p, "sha256": got, "bytes": size})
    return out


def record(name: str, m: dict, files: list, ok: bool, why: str = "") -> None:
    try:
        QUICK.mkdir(parents=True, exist_ok=True)
        with open(APPLIED, "a", encoding="utf-8") as f:
            f.write(json.dumps({"at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "id": name, "deploy": m.get("deploy"),
                                "files": [[x["path"], x["sha256"][:12]] for x in files], "ok": ok, "why": why[:300]}) + "\n")
    except OSError:
        pass


def run_bat(bat: str) -> int:
    """The deploy step's own .bat, as Control runs it (its output straight through, so Control logs and scrubs it)."""
    p = subprocess.Popen(["cmd", "/c", "call", str(ROOT / bat)], cwd=str(ROOT), stdin=subprocess.PIPE,
                         creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    try:
        p.stdin.write(b"\n" * 8)
        p.stdin.close()
    except OSError:
        pass
    return p.wait()


def main(argv=None, runner=run_bat, root: Path = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if not argv or argv[0] not in ("check", "apply"):
        print(__doc__.strip().splitlines()[0])
        print("usage: quick.py check|apply [--id ID] [--note NOTE]")
        return 2
    what = argv.pop(0)
    item_id = os.environ.get("LEYLINE_CONTROL_ID", "")
    note = os.environ.get("LEYLINE_CONTROL_NOTE", "")
    while argv:
        k = argv.pop(0)
        if k == "--id" and argv:
            item_id = argv.pop(0)
        elif k == "--note" and argv:
            note = argv.pop(0)
        else:
            print(f"!! unknown argument {k!r}")
            return 2
    base = (root or ROOT)
    m, files, name = {}, [], item_id
    try:
        name = manifest_name(item_id, note)
        m = load(base / "control" / "quick" / f"{name}.json")
        files = verify(m, base)
    except Refused as e:
        print(f"!! Refused, nothing deployed: {e}")
        if what == "apply":
            record(name, m, files, False, str(e))
        return 1
    print(f"Manifest control/quick/{name}.json: {len(files)} file(s), deploy: {m['deploy']}" + (f" ({m['note']})" if m.get("note") else ""))
    for x in files:
        print(f"  ok  {x['path']}  {x['sha256'][:12]}  {x['bytes']} bytes")
    if what == "check":
        print("QUICK CHECK OK (nothing deployed)")
        return 0
    bat = DEPLOY[m["deploy"]][0]
    if not bat:
        record(name, m, files, True, "checked; no deploy step")
        print("QUICK APPLY OK: checked and logged (no deploy step; the next ship carries it)")
        return 0
    print(f"== running the '{m['deploy']}' step ({bat})")
    rc = runner(bat)
    record(name, m, files, rc == 0, "" if rc == 0 else f"the {m['deploy']} step failed (exit {rc})")
    if rc != 0:
        print(f"!! The '{m['deploy']}' step failed (exit {rc}): its own rollback ran. See the lines above.")
        return rc
    print(f"QUICK APPLY OK: {len(files)} file(s) deployed by '{m['deploy']}'")
    return 0


if __name__ == "__main__":
    sys.exit(main())
