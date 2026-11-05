#!/usr/bin/env python3
"""tools/security/h3-off.sh against fakes (review 4 Nov): fake docker (caddy validate / reload) and fake curl.

  * off adds "servers { protocols h1 h2 }" to the global block (made when there is none), backs up, self-checks;
  * a Caddy that refuses it, or a site that stops answering, gets the old file back;
  * --undo takes out only those lines: a change made to the Caddyfile after h3-off survives (it used to be lost);
  * --undo on a file without the lines changes nothing; hand-edited lines are refused, not guessed.

    python3 -I tools/security/test_h3.py
"""
import os, shutil, subprocess, tempfile, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "h3-off.sh"
DOCKER = """#!/bin/sh
echo "$*" >> "$FAKE_DIR/calls"
case "$*" in *validate*) [ -f "$FAKE_DIR/refuse" ] && exit 1;; esac
exit 0
"""
CURL = """#!/bin/sh
case "$*" in *-D*) printf 'HTTP/2 200\\r\\n'; exit 0;; esac
[ -f "$FAKE_DIR/site-down" ] && { printf 502; exit 0; }
printf 200
"""
SITES = "leylinelauncher.com {\n\treverse_proxy ovlive-realm:8090\n}\n"
GLOBAL = "{\n\temail ops@example.test\n}\n\n" + SITES


class H3(unittest.TestCase):
    def setUp(self):
        self.t = Path(tempfile.mkdtemp(prefix="h3-"))
        (self.t / "bin").mkdir()
        for n, body in (("docker", DOCKER), ("curl", CURL)):
            p = self.t / "bin" / n
            p.write_text(body)
            p.chmod(0o755)
        self.cf = self.t / "Caddyfile"
        self.home = self.t / "root"

    def tearDown(self):
        shutil.rmtree(self.t, ignore_errors=True)

    def run_sh(self, *args):
        env = dict(os.environ, PATH=f"{self.t / 'bin'}:{os.environ['PATH']}", WU_CADDYFILE=str(self.cf),
                   WU_CADDY_DIR=str(self.t), FAKE_DIR=str(self.t), WU_H3_WAIT="0")
        src = SCRIPT.read_text().replace("BKROOT=/root/caddy-backups", f"BKROOT={self.home}/caddy-backups")
        (self.t / "h3.sh").write_text(src)
        p = subprocess.run(["bash", str(self.t / "h3.sh"), *args], env=env, capture_output=True, text=True, timeout=60)
        return p.returncode, p.stdout + p.stderr

    def test_off_then_undo_keeps_later_changes(self):
        for start in (SITES, GLOBAL):
            with self.subTest(global_block=start is GLOBAL):
                self.cf.write_text(start)
                rc, out = self.run_sh()
                self.assertEqual(rc, 0, out)
                self.assertIn("H3 OFF OK", out)
                txt = self.cf.read_text()
                self.assertIn("\t\tprotocols h1 h2", txt)
                self.assertEqual(txt.count("{\n"), start.count("{\n") + (2 if start is SITES else 1))
                # run again: nothing doubled
                rc, out = self.run_sh()
                self.assertEqual(self.cf.read_text().count("protocols h1 h2"), 1)
                # a later change (another step adds a redirect), then undo
                later = self.cf.read_text().replace("\treverse_proxy", "\tredir /login /wrath/signin 302\n\treverse_proxy")
                self.cf.write_text(later)
                rc, out = self.run_sh("--undo")
                self.assertEqual(rc, 0, out)
                self.assertIn("H3 UNDO OK", out)
                want = start.replace("\treverse_proxy", "\tredir /login /wrath/signin 302\n\treverse_proxy")
                self.assertEqual(self.cf.read_text(), want)

    def test_refused_or_site_down_rolls_back(self):
        self.cf.write_text(SITES)
        (self.t / "refuse").touch()
        rc, out = self.run_sh()
        self.assertEqual(rc, 1)
        self.assertEqual(self.cf.read_text(), SITES)
        (self.t / "refuse").unlink()
        (self.t / "site-down").touch()
        rc, out = self.run_sh()
        self.assertEqual(rc, 1)
        self.assertIn("old Caddyfile is back", out)
        self.assertEqual(self.cf.read_text(), SITES)

    def test_undo_without_lines_or_hand_edited(self):
        self.cf.write_text(SITES)
        rc, out = self.run_sh("--undo")
        self.assertEqual((rc, self.cf.read_text()), (0, SITES))
        self.run_sh() if not (self.t / "site-down").exists() else None
        edited = self.cf.read_text().replace("protocols h1 h2", "protocols h1 h2 h3")
        self.cf.write_text(edited)
        rc, out = self.run_sh("--undo")
        self.assertEqual(rc, 1, out)
        self.assertEqual(self.cf.read_text(), edited, "nothing guessed")


if __name__ == "__main__":
    unittest.main()
