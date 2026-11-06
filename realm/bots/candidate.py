#!/usr/bin/env python3
"""The canary's proposal (bots-canary.yml, propose): realm/bots-candidate.json (stdout) and the pull request's text.

    python3 realm/bots/candidate.py --smoke success|skipped|failure [--report smoke.json] [--compat compat.json]
                                    [--images canary-abc1234] [--run URL] [--body pr-body.md] > realm/bots-candidate.json

The candidate file says what was tested and how; the owner merging the pull request is the only promotion. Reads
realm/bots.json (already the canary pins in the proposal's working copy). Standard library only.
"""
import argparse
import datetime
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))


def load(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError, TypeError):
        return None


def candidate(pins, smoke, report=None, compat=None, images="", run="", today=None):
    state = {"success": "passed", "skipped": "pending", "failure": "failed"}.get(smoke, "pending")
    if state == "passed" and not (report or {}).get("ok"):
        state = "failed"          # a green job must also have a green report
    return {
        "format": 1,
        "playerbots": pins.get("playerbots"),
        "core": pins.get("core"),
        "changes": pins.get("changes"),
        "images": images,
        "tested": (today or datetime.date.today()).isoformat(),
        "smoke": state,
        "checks": [{"name": c.get("name"), "ok": bool(c.get("ok")), "detail": str(c.get("detail", ""))[:200]}
                   for c in (report or {}).get("checks", [])],
        "compat": {"ok": bool((compat or {}).get("ok")), "missing": (compat or {}).get("missing", [])},
        "run": run,
    }


def body(c):
    s = c["playerbots"] or ""
    lines = [f"Moves **mod-playerbots** to `{s[:7]}` and its core to `{(c['core'] or '')[:7]}` "
             f"({c.get('changes') if c.get('changes') is not None else '?'} upstream changes since the current pin).", ""]
    if c["smoke"] == "passed":
        lines.append("The bot smoke test passed on the canary images (`" + c["images"] + "`):")
    elif c["smoke"] == "pending":
        lines += ["**Smoke test pending**: no smoke runner here (repo variable `BOTS_SMOKE_RUNNER`). Run Control's "
                  "`bots-smoke` step on the PC against `" + c["images"] + "` first, then mark this ready.", ""]
    else:
        lines.append("**The smoke test FAILED**: do not merge.")
    if c["checks"]:
        lines += ["", "| check | | detail |", "|---|---|---|"]
        lines += [f"| {x['name']} | {'pass' if x['ok'] else 'FAIL'} | {x['detail'].replace('|', '/')} |" for x in c["checks"]]
    lines += ["", "Compat guard (`realm/bots/compat.py`): " + ("OK" if c["compat"]["ok"] else "FAILED"), "",
              "Merging is the promotion: `wu-images.yml` then builds `:stable` from main, and players' launchers offer "
              "**Update the bots** on the Single player page (never automatic). Nothing here merges by itself.",
              "", "Run: " + (c.get("run") or "-")]
    return "\n".join(lines) + "\n"


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--smoke", required=True)
    ap.add_argument("--report")
    ap.add_argument("--compat")
    ap.add_argument("--images", default="")
    ap.add_argument("--run", default="")
    ap.add_argument("--body")
    ap.add_argument("--pins", default=os.path.join(REPO, "realm", "bots.json"))
    a = ap.parse_args(argv)
    pins = load(a.pins) or {}
    c = candidate(pins, a.smoke, load(a.report), load(a.compat), a.images, a.run)
    if a.body:
        with open(a.body, "w", encoding="utf-8") as f:
            f.write(body(c))
    json.dump(c, sys.stdout, indent=1)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
