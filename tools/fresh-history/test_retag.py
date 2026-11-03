"""retag.py on real throwaway git repos (a bare 'GitHub', the owner's folder); gh is faked."""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import retag as R  # noqa: E402
from fresh_history import Stop  # noqa: E402

BASE = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")
OLD = dict(BASE, GIT_AUTHOR_NAME="Someone", GIT_AUTHOR_EMAIL="old@x", GIT_COMMITTER_NAME="Someone",
           GIT_COMMITTER_EMAIL="old@x")
ORION = dict(BASE, GIT_AUTHOR_NAME="Orion", GIT_AUTHOR_EMAIL="orionopenwow@users.noreply.github.com",
             GIT_COMMITTER_NAME="Orion", GIT_COMMITTER_EMAIL="orionopenwow@users.noreply.github.com")


@unittest.skipUnless(shutil.which("git"), "git not installed")
class RetagTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.origin = os.path.join(self.tmp, "origin.git")
        self.pc = os.path.join(self.tmp, "pc")
        self.g(BASE, "init", "-q", "--bare", "-b", "main", self.origin)
        self.g(BASE, "clone", "-q", self.origin, self.pc)
        self.commit(OLD, "a.txt")
        self.g(OLD, "-C", self.pc, "tag", "launcher-latest")
        self.g(OLD, "-C", self.pc, "tag", "-a", "host-kit", "-m", "kit")
        self.old = self.g(BASE, "-C", self.pc, "rev-parse", "HEAD")
        # main rewritten as one commit by Orion (what fresh-leyline does)
        self.g(ORION, "-C", self.pc, "checkout", "-q", "--orphan", "new")
        self.g(ORION, "-C", self.pc, "commit", "-q", "-m", "Leyline")
        self.g(ORION, "-C", self.pc, "tag", "launcher-v0.3.45")
        self.g(BASE, "-C", self.pc, "push", "-q", "-f", "origin", "new:main", "--tags")
        self.main = self.g(BASE, "-C", self.pc, "rev-parse", "HEAD")
        self.lines = []
        self.confirm = os.path.join(self.tmp, "CONFIRM-retag")

    def g(self, env, *a):
        r = subprocess.run(["git"] + list(a), env=env, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        return r.stdout.strip()

    def commit(self, env, name):
        with open(os.path.join(self.pc, name), "w") as f:
            f.write("x")
        self.g(env, "-C", self.pc, "add", name)
        self.g(env, "-C", self.pc, "commit", "-q", "-m", name)

    def runner(self, login="orionopenwow"):
        def run(args):
            if args[:2] == ["gh", "api"]:
                return 0, login
            return R.run(args)
        return run

    def remote_tag(self, name):
        out = self.g(BASE, "ls-remote", "--tags", self.origin)
        tags = {}
        for line in out.splitlines():
            sha, ref = line.split("\t")
            tags[ref] = sha
        return tags.get(f"refs/tags/{name}^{{}}") or tags.get(f"refs/tags/{name}")

    def test_dry_run_lists_only_the_old_tags(self):
        self.assertEqual(R.retag(self.pc, runner=self.runner(), out=self.lines.append), 0)
        text = "\n".join(self.lines)
        self.assertIn("launcher-latest", text)
        self.assertIn("host-kit", text)
        self.assertNotIn("launcher-v0.3.45:", text)
        self.assertEqual(self.remote_tag("launcher-latest"), self.old, "a dry run moves nothing")

    def test_go_moves_them_after_a_backup(self):
        with open(self.confirm, "w") as f:
            f.write(self.main[:7])
        backups = os.path.join(self.tmp, "Desktop")
        R.retag(self.pc, go=True, confirm_file=self.confirm, backup_dir=backups, runner=self.runner(),
                out=self.lines.append)
        self.assertEqual(self.remote_tag("launcher-latest"), self.main)
        self.assertEqual(self.remote_tag("host-kit"), self.main)
        self.assertIn("RETAG OK", self.lines)
        self.assertEqual(len(os.listdir(backups)), 1, "the old tags kept in a bundle")
        # again: nothing left to move
        self.lines.clear()
        R.retag(self.pc, runner=self.runner(), out=self.lines.append)
        self.assertIn("nothing to do", "\n".join(self.lines))

    def test_stops_without_confirm_wrong_confirm_or_wrong_account(self):
        with self.assertRaises(Stop):
            R.retag(self.pc, go=True, confirm_file=self.confirm, runner=self.runner(), out=self.lines.append)
        with open(self.confirm, "w") as f:
            f.write("0000000")
        with self.assertRaises(Stop):
            R.retag(self.pc, go=True, confirm_file=self.confirm, runner=self.runner(), out=self.lines.append)
        with self.assertRaises(Stop):
            R.retag(self.pc, runner=self.runner("Chumpton"), out=self.lines.append)
        self.assertEqual(self.remote_tag("launcher-latest"), self.old)


if __name__ == "__main__":
    unittest.main()
