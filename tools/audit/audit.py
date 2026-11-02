#!/usr/bin/env python3
"""Leyline's release audit: every feature checked the way a new player meets it, in one run (docs/AUDIT.md).

    py -3 tools\\audit\\audit.py                 on the owner's PC (Leyline Control: Release audit, or the step "audit")
    python3 tools/audit/audit.py --fake          anywhere else: the site's own code and fake servers on 127.0.0.1
    ... --only site,downloads,server,hosting,wrath,den,launcher   --no-boost   --logs <folder>

Prints one PASS / FAIL / SKIP line per check and writes logs/audit-latest.txt and logs/audit-latest.html (green or
red per feature, what failed, the one next action, and the short list only a person can check). Read only, except
the audit's own test account and characters on the realm, and a Den Lite on this PC in a temporary folder (removed
after). Anything this PC can't do is a SKIP with the reason. Exit code: 0 nothing failed, 1 something failed,
2 the audit itself broke. Standard library only.
"""
from __future__ import annotations

import argparse
import shutil
import sys
import tempfile
import time
import traceback
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import FAIL, ROOT, Ctx, Recorder, load_config, short  # noqa: E402
import report  # noqa: E402

SECTIONS = ["site", "downloads", "server", "hosting", "wrath", "den", "launcher"]


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Leyline's release audit")
    ap.add_argument("--fake", action="store_true", help="fake servers on 127.0.0.1 (CI, or away from the owner's PC)")
    ap.add_argument("--only", default="", help="comma list of sections: " + ",".join(SECTIONS))
    ap.add_argument("--no-boost", action="store_true", help="skip the level-80 boost test character")
    ap.add_argument("--logs", default=str(ROOT / "logs"))
    a = ap.parse_args(argv)
    only = [s for s in a.only.split(",") if s] or SECTIONS
    cfg = load_config()
    cfg["no_boost"] = a.no_boost
    rec = Recorder()
    temp = Path(tempfile.mkdtemp(prefix="leyline-audit-"))
    fakes = None
    fake, rewrite = {}, []
    t0 = time.time()
    print(f"Leyline release audit ({'fake servers' if a.fake else 'live'}): {', '.join(only)}", flush=True)
    try:
        if a.fake:
            import fakes as fk
            fakes = fk.Fakes()
            fake, rewrite = fakes.ctx_args()
        ctx = Ctx(cfg, rec, fake, rewrite, temp)
        wu = None
        for name in only:
            print(f"== {name}", flush=True)
            t = time.time()
            try:
                if name == "site":
                    import web
                    web.run(ctx, rec)
                elif name == "downloads":
                    import web
                    web.run_downloads(ctx, rec)
                elif name == "server":
                    import server
                    server.run(ctx, rec)
                elif name == "hosting":
                    import server
                    wu = server.network_list(ctx, rec)
                    import web
                    web.check_hosting(ctx, rec, wu)
                elif name == "wrath":
                    import server
                    import wrath
                    pw = server.audit_account(ctx, rec)
                    wrath.run(ctx, rec, cfg["audit_account"], pw)
                elif name == "den":
                    import den
                    den.run(ctx, rec)
                elif name == "launcher":
                    import pc
                    pc.run(ctx, rec)
            except Exception as e:  # noqa: BLE001 - one section never stops the others
                rec.add(name.capitalize(), f"The {name} checks ran", FAIL, short(e), "Tell Claude this line.")
                traceback.print_exc()
            print(f"   ({time.time() - t:.0f} s)", flush=True)
        where = ", ".join(f"{k}: {ctx._hosts[k]}" for k in sorted(ctx._hosts)) or ("127.0.0.1" if a.fake else "")
        meta = {"secs": time.time() - t0, "mode": "fake servers" if a.fake else "live", "where": where,
                "appendix": ctx.cache.get("appendix", [])}
        t, h = report.write(rec.results, meta, Path(a.logs))
        c = rec.counts()
        print(f"\nRESULT: {c['PASS']} passed, {c['FAIL']} failed, {c['SKIP']} skipped in {meta['secs']:.0f} s")
        print(f"Next: {report.next_action(rec.results)}")
        print(f"Report: {h}")
        print("AUDIT " + ("GREEN" if not c["FAIL"] else "RED"))
        return 1 if c["FAIL"] else 0
    except Exception as e:  # noqa: BLE001
        print(f"The audit itself broke: {short(e)}")
        traceback.print_exc()
        return 2
    finally:
        if fakes:
            fakes.close()
        shutil.rmtree(temp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
