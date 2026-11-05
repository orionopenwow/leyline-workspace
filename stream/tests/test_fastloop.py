"""stream/fastloop.sh (the signin step's server half), run against fakes: no server, no Docker, no network.

  * every portal file the script installs is also backed up and put back on a rollback (review 4 Nov: looks.py,
    looks.json, upstream_addons.py, upstream-addons.json, leyline_net.py and hostpack.json were not);
  * a portal that doesn't answer after the update gets its old files back (the real put_back, with fake docker);
  * a healthy run ends "FASTLOOP OK"; a signed-out 200 on /data (game data not gated) fails the step;
  * old backups are pruned to the last ten.

    python3 -I stream/tests/test_fastloop.py
"""
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
SCRIPT = os.path.join(STREAM, "fastloop.sh")


def installed_files(src):
    out = set()
    for ln in src.splitlines():
        if "install -m 644" not in ln or "$WU/portal" not in ln:
            continue
        for m in re.finditer(r'install -m 644 ([^&;|]*?)"\$WU/portal/([\w.-]*)"', ln):
            for f in m.group(1).split() + ([m.group(2)] if m.group(2) else []):
                f = f.split("/")[-1]
                if re.fullmatch(r"[\w-]+\.(py|json)", f):
                    out.add(f)
    return out


def put_back_body(src):
    i = src.index("put_back() {")
    return src[i:src.index("\n}\n", i)]


FAKE_DOCKER = r"""#!/bin/sh
echo "docker $*" >> "$FAKE_LOG"
case "$1" in
  inspect) printf '/data\n/hosted\n'; exit 0;;
  exec) [ -f "$FAKE_DIR/portal-down" ] && exit 1; exit 0;;
  *) exit 0;;
esac
"""
FAKE_CURL = r"""#!/bin/sh
for a in "$@"; do last="$a"; done
case "$last" in
  */data/__index) cat "$FAKE_DIR/data-code" 2>/dev/null || printf 302;;
  *) printf 200;;
esac
"""


class Static(unittest.TestCase):
    def setUp(self):
        self.src = open(SCRIPT, encoding="utf-8").read()

    def test_syntax(self):
        subprocess.run(["bash", "-n", SCRIPT], check=True)

    def test_every_installed_file_is_rolled_back(self):
        files = installed_files(self.src)
        self.assertGreater(len(files), 40, "the parser found the install lines")
        missing = sorted(f for f in files if f not in put_back_body(self.src))
        self.assertEqual(missing, [], "installed by fastloop.sh but never put back by put_back()")

    def test_gate_check_and_prune(self):
        self.assertIn('if [ "$c2" = 200 ]', self.src)
        self.assertIn("head -n -10", self.src)


class Run(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="fastloop-")
        self.wu = os.path.join(self.tmp, "wu")
        os.makedirs(os.path.join(self.wu, "portal"))
        open(os.path.join(self.wu, "portal.env"), "w").close()
        self.bin = os.path.join(self.tmp, "bin")
        os.makedirs(self.bin)
        for name, body in (("docker", FAKE_DOCKER), ("curl", FAKE_CURL)):
            p = os.path.join(self.bin, name)
            with open(p, "w") as f:
                f.write(body)
            os.chmod(p, 0o755)
        self.cf = os.path.join(self.tmp, "Caddyfile")
        with open(self.cf, "w") as f:
            f.write("example.test {\n\thandle {\n\t\treverse_proxy ovlive-realm:8090\n\t}\n}\n")
        self.bk = os.path.join(self.tmp, "backups")
        # the server's portal as it was before: an old looks.py and wrath_portal.py
        for n in ("looks.py", "wrath_portal.py", "leyline_net.py"):
            with open(os.path.join(self.wu, "portal", n), "w") as f:
                f.write(f"# old {n}\n")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def run_script(self):
        env = dict(os.environ, PATH=self.bin + os.pathsep + os.environ["PATH"], WU_LIB="/nonexistent/stream-lib.sh",
                   WU_DIR=self.wu, WU_CADDYFILE=self.cf, WU_CADDY_DIR=self.tmp, WU_BACKUPS=self.bk,
                   FAKE_DIR=self.tmp, FAKE_LOG=os.path.join(self.tmp, "calls"))
        # caddy_reload runs `docker compose exec` in WU_CADDY_DIR: the fake docker answers 0
        p = subprocess.run(["bash", SCRIPT], env=env, capture_output=True, text=True, timeout=300)
        return p.returncode, p.stdout + p.stderr

    def test_portal_down_puts_old_files_back(self):
        open(os.path.join(self.tmp, "portal-down"), "w").close()
        rc, out = self.run_script()
        self.assertEqual(rc, 1, out[-2000:])
        self.assertIn("the old one (and its art) is back", out)
        for n in ("looks.py", "wrath_portal.py", "leyline_net.py"):
            self.assertEqual(open(os.path.join(self.wu, "portal", n)).read(), f"# old {n}\n", n)
        # files that weren't there before are gone again
        self.assertFalse(os.path.exists(os.path.join(self.wu, "portal", "looks.json")))
        self.assertFalse(os.path.exists(os.path.join(self.wu, "portal", "herokeys.py")))

    def test_healthy_run_and_gate(self):
        rc, out = self.run_script()
        self.assertEqual(rc, 0, out[-2000:])
        self.assertTrue(out.rstrip().endswith("FASTLOOP OK"), out[-500:])
        self.assertNotEqual(open(os.path.join(self.wu, "portal", "looks.py")).read(), "# old looks.py\n")
        with open(os.path.join(self.tmp, "data-code"), "w") as f:
            f.write("200")
        rc, out = self.run_script()
        self.assertEqual(rc, 1, out[-1000:])
        self.assertIn("not behind the sign-in gate", out)
        self.assertNotIn("FASTLOOP OK", out)

    def test_old_backups_pruned(self):
        for i in range(14):
            os.makedirs(os.path.join(self.bk, f"20260101-0000{i:02d}"))
        open(os.path.join(self.tmp, "portal-down"), "w").close()
        self.run_script()
        self.assertEqual(len(os.listdir(self.bk)), 10)


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]] + [a for a in sys.argv[1:] if a.startswith("-")])
