"""This PC's Leyline: the launcher that is on it against the latest release, and its settings file."""
from __future__ import annotations

import hashlib
import json
import os
import subprocess
from pathlib import Path

from common import FAIL, PASS, ROOT, SKIP, appdata, is_windows

F = "Launcher"


def installed_launchers() -> list[Path]:
    out = []
    env = os.environ.get("LEYLINE_EXE")
    if env:
        out.append(Path(env))
    out.append(ROOT / ".launcher" / "Leyline.exe")          # "Open Leyline Launcher.bat" keeps it here
    la = os.environ.get("LOCALAPPDATA")
    if la:
        out += [Path(la) / "Programs" / "Leyline" / "Leyline.exe", Path(la) / "Leyline" / "Leyline.exe"]
    return [p for p in out if p.is_file()]


def file_version(p: Path) -> str:
    if not is_windows():
        return ""
    try:
        r = subprocess.run(["powershell", "-NoProfile", "-Command", f"(Get-Item -LiteralPath '{p}').VersionInfo.ProductVersion"],
                           capture_output=True, text=True, timeout=20, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        return r.stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        return ""


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def settings_problems(doc) -> list[str]:
    """What is wrong with the launcher's settings.json (launcher/src-tauri/src/settings.rs), [] when it is sane."""
    if not isinstance(doc, dict):
        return ["not a JSON object"]
    out = []
    if doc.get("schema") != 1:
        out.append(f"schema {doc.get('schema')!r} (expected 1)")
    clients = doc.get("clients", [])
    if not isinstance(clients, list):
        return out + ["clients is not a list"]
    ids = [c.get("id") for c in clients if isinstance(c, dict)]
    sel = doc.get("selected_client")
    if sel and sel not in ids:
        out.append(f"the chosen game ({sel}) is not in the list")
    for c in clients:
        if isinstance(c, dict) and c.get("path") and not Path(c["path"]).exists():
            out.append(f"game folder {c['path']} is gone")
    return out


def run(ctx, rec):
    found = installed_launchers()
    latest = ctx.cache.get("launcher_sha")
    ver = ctx.cache.get("launcher_version") or "?"
    if not found:
        rec.add(F, "The launcher on this PC is the latest", SKIP,
                "no Leyline.exe on this PC (Open Leyline Launcher.bat gets it)" if is_windows() else "not a Windows PC")
    elif not latest:
        rec.add(F, "The launcher on this PC is the latest", SKIP, "the latest release didn't download, so nothing to compare")
    else:
        exe = found[0]
        same = sha256(exe) == latest
        have = file_version(exe)
        rec.add(F, "The launcher on this PC is the latest", PASS if same else FAIL,
                f"{exe}: " + ("the same file as release " + ver if same else f"version {have or 'older'}, the latest is {ver}"),
                "Double-click Open Leyline Launcher.bat (it fetches the latest), then run the audit again.")
    ad = appdata()
    sp = ad / "Leyline" / "settings.json" if ad else None
    if not sp or not sp.exists():
        rec.add(F, "Its settings file is sane", SKIP, "no %APPDATA%\\Leyline\\settings.json (the launcher hasn't run here)")
        return
    try:
        doc = json.loads(sp.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        rec.add(F, "Its settings file is sane", FAIL, f"{sp} doesn't read: {e}",
                "Close Leyline; it sets a broken settings file aside by itself at the next start.")
        return
    bad = settings_problems(doc)
    n = len(doc.get("clients") or [])
    rec.add(F, "Its settings file is sane", PASS if not bad else FAIL,
            f"{n} game folders, realm {doc.get('selected_realm') or 'not chosen'}" + (f"; {'; '.join(bad)}" if bad else ""),
            "Open Leyline > Settings and add the game folder again.")
