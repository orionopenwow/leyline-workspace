"""The server, over the PC's existing SSH key (the same login the check step uses), read only: the world server,
its Lua scripts, the streamed slice, the relay, the GM console and the bug manifest (server/audit-server.sh). Also
the audit account for the gameplay checks (audit-server.sh account) and, from this PC, the network list."""
from __future__ import annotations

import base64
import os
import shutil
import subprocess
from pathlib import Path

from common import FAIL, PASS, ROOT, SKIP, HERE, short

F = "Server"
CHECKS = {   # key: (what it checks, the one next action when it fails)
    "world": ("World server up", "Run the worldlog step (why the world server stopped) and send Claude its log."),
    "world_restart": ("No world server restart in the last hour", "Run the worldlog step and send Claude its log."),
    "oom": ("No out-of-memory kills in the last 24 h", "Run the lean step, or tell Claude the server ran out of memory."),
    "soap": ("The realm answers its console (SOAP)", "Run the check step and send Claude logs/server-check-latest.txt."),
    "lua": ("Lua scripts there, no Lua errors", "Tell Claude the Lua error in this line."),
    "lua_loaded": ("The Lua engine loaded the scripts", "Run the scripts step (live reload)."),
    "slice": ("The streamed game files are whole (slice health)", "Run the slice-health step and send Claude logs/slice-health-latest.txt."),
    "relay": ("Relay running", "Run the relay step (Deploy relay)."),
    "gmconsole": ("GM console answers", "Run the gm step (GM console update)."),
    "bugs": ("Bug manifest reachable", "Run the gm step, then the bugs step."),
    "memory": ("Memory to spare", "Run the lean step, or tell Claude."),
    "disk": ("Disk space to spare", "Run tidy-check, then tidy."),
    "account": ("The audit's test account", "Run the check step and send Claude logs/server-check-latest.txt."),
}


def parse(text: str) -> tuple[list[tuple[str, str, str]], str, bool]:
    """audit-server.sh's output: [(key, ok|bad|skip, detail)], the secret (or ""), and whether it finished."""
    rows, secret, done = [], "", False
    for line in text.splitlines():
        line = line.rstrip("\r")
        if line.startswith("AUDIT_SECRET "):
            secret = line.split(" ", 1)[1].strip()
        elif line.startswith("AUDIT DONE"):
            done = True
        elif line.startswith("AUDIT "):
            parts = line.split(" ", 3)
            if len(parts) >= 3:
                rows.append((parts[1], parts[2], parts[3] if len(parts) > 3 else ""))
    return rows, secret, done


def payload(mode: str) -> str:
    script = (HERE / "server" / "audit-server.sh").read_text(encoding="utf-8")
    pre = ""
    if mode == "checks":
        sh = ROOT / "stream" / "server" / "slice-health.sh"
        if sh.exists():
            pre = "export SLICE_HEALTH_B64='" + base64.b64encode(sh.read_bytes()).decode() + "'\n"
    return pre + script


def ssh(ctx, mode: str, timeout: float = 240) -> tuple[int, str]:
    """Runs audit-server.sh <mode> on the server; (exit code, output). -1: this PC can't (no ssh or no key)."""
    fx = ctx.fake.get("ssh_" + mode)
    if fx is not None:
        return 0, Path(fx).read_text(encoding="utf-8") if os.path.exists(str(fx)) else str(fx)
    if not shutil.which("ssh"):
        return -1, "no ssh on this PC"
    key = Path(os.path.expanduser(ctx.cfg["ssh"]["key"]))
    if not key.exists():
        return -1, f"no SSH key at {key} (only the owner's PC has it)"
    host = ctx.host(ctx.cfg["ssh"].get("host", "realmlist"))
    cmd = ["ssh", "-i", str(key), "-o", "StrictHostKeyChecking=accept-new", "-o", "BatchMode=yes",
           "-o", "ConnectTimeout=20", "-o", "ServerAliveInterval=30", f"{ctx.cfg['ssh']['user']}@{host}",
           f"T=$(mktemp) && sed 's/\\r$//' > $T && bash $T {mode}; rc=$?; rm -f $T; exit $rc"]
    try:
        p = subprocess.run(cmd, input=payload(mode).encode(), capture_output=True, timeout=timeout,
                           creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    except (OSError, subprocess.TimeoutExpired) as e:
        return 1, short(e)
    out = p.stdout.decode("utf-8", "replace")
    if p.returncode == 255:
        return 1, "SSH could not log in: " + p.stderr.decode("utf-8", "replace").strip()[-200:]
    return p.returncode, out


def run(ctx, rec):
    rc, out = ssh(ctx, "checks")
    if rc == -1:
        rec.add(F, "Server checks over SSH", SKIP, out)
        return
    rows, _, done = parse(out)
    if not rows:
        rec.add(F, "Server checks over SSH", FAIL, out.strip()[-200:] or f"exit {rc}",
                "Run the check step; if it fails too, the SSH key or the server is the problem.")
        return
    for key, st, detail in rows:
        name, action = CHECKS.get(key, (key, "Tell Claude this line."))
        rec.add(F, name, {"ok": PASS, "bad": FAIL}.get(st, SKIP), detail, action)
    if not done:
        rec.add(F, "Server checks finished", FAIL, "the script stopped early", "Run the check step and send Claude its log.")


def audit_account(ctx, rec) -> str:
    """The AUDIT account made or refreshed on the server; returns its password ("" when it can't be)."""
    rc, out = ssh(ctx, "account", timeout=90)
    if rc == -1:
        rec.add("Wrath Unleashed", "The audit's test account", SKIP, out)
        return ""
    rows, secret, _ = parse(out)
    rec.hide(secret)
    row = next((r for r in rows if r[0] == "account"), None)
    if not secret or not row or row[1] != "ok":
        rec.add("Wrath Unleashed", "The audit's test account", FAIL, (row[2] if row else out.strip()[-200:]) or f"exit {rc}",
                CHECKS["account"][1])
        return ""
    rec.add("Wrath Unleashed", "The audit's test account", PASS, row[2])
    return secret


def network_list(ctx, rec):
    """From this PC, as the launcher reads it: the relay's listing, with Wrath Unleashed on it."""
    r = ctx.get(ctx.url("relay", "/api/listing/realms.json"), timeout=20)
    data = r.json() if r.status == 200 else None
    if not r.status:
        rec.add("Hosting", "The network list (relay listing) loads", FAIL, f"{ctx.base('relay')}: {r.error}",
                "Check this PC's internet, then run the relay step (Deploy relay).")
        return None
    if not isinstance(data, dict) or not isinstance(data.get("realms"), list):
        rec.add("Hosting", "The network list (relay listing) loads", FAIL, f"HTTP {r.status or r.error}",
                "Run the relay step (Deploy relay).")
        return None
    realms = data["realms"]
    rec.add("Hosting", "The network list (relay listing) loads", PASS, f"{len(realms)} realms")
    want = ctx.cfg["realm_name"].lower()
    wu = next((x for x in realms if want in str(x.get("name", "")).lower()), None)
    if wu:
        rec.add("Hosting", f"{ctx.cfg['realm_name']} on the network list", PASS,
                f"live: {wu.get('live')}, players: {wu.get('players')}")
    else:
        rec.add("Hosting", f"{ctx.cfg['realm_name']} on the network list", FAIL, "not listed",
                "Run the hangout step (Wrath Unleashed on Leyline).")
    ctx.cache["listing"] = realms
    return wu
