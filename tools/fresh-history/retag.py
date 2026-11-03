"""Release tags that still point at old commits by someone else, moved onto Orion's current main.

Owner 3 Nov: "still need to scrub chumpton as a contributor from the Leyline repo". After fresh-leyline, main is one
commit by Orion, but five release tags (leyline-addons, launcher-latest, launcher-v0.3.1, host-pack, host-kit) still
point at the old commits, so GitHub keeps counting their author. This moves each such tag to GitHub's main. The
releases stay (a release hangs on its tag's name, not its commit) and so do their files and download links.

Dry run by default: lists the tags it would move. With --go it needs --confirm: a file holding GitHub's main commit
(the owner writes it; Claude can't), and a backup bundle of every old tag goes to the Desktop first.
"""
import argparse
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fresh_history import OWNER, NOT_OWNER, Stop, run  # noqa: E402

ORION_EMAILS = ("orionopenwow@users.noreply.github.com",)


def retag(root, go=False, confirm_file=None, backup_dir=None, runner=run, out=print, now=time.strftime):
    git = lambda *a: runner(["git", "-C", root] + list(a))
    rc, who = runner(["gh", "api", "user", "--jq", ".login"])
    if rc != 0:
        raise Stop("the GitHub CLI is not signed in (gh auth login)")
    if who.strip().lower() in NOT_OWNER or who.strip() != OWNER:
        raise Stop(f"gh is signed in as {who.strip()}: it must be {OWNER} (gh auth switch)")
    out(f"[1/5] signed in as {who.strip()}")
    rc, txt = git("fetch", "-q", "--force", "--prune", "--tags", "origin")
    if rc != 0:
        raise Stop("git fetch failed: " + txt)
    rc, main = git("rev-parse", "refs/remotes/origin/main")
    if rc != 0:
        raise Stop("could not read GitHub's main")
    rc, author = git("log", "-1", "--format=%ae", main)
    if author.strip().lower() not in ORION_EMAILS:
        raise Stop(f"GitHub's main ({main[:7]}) is by {author.strip()}, not Orion: run fresh-leyline first")
    out(f"[2/5] GitHub's main is {main[:7]}, by Orion")
    rc, refs = git("ls-remote", "--tags", "origin")
    if rc != 0:
        raise Stop("could not list GitHub's tags: " + refs)
    tags = {}
    for line in refs.splitlines():
        sha, _, ref = line.partition("\t")
        if ref.startswith("refs/tags/"):
            name = ref[len("refs/tags/"):]
            if name.endswith("^{}"):
                tags[name[:-3]] = sha   # an annotated tag: the commit it points at
            else:
                tags.setdefault(name, sha)
    move = []
    for name, sha in sorted(tags.items()):
        rc, email = git("log", "-1", "--format=%ae", sha)
        if rc != 0:
            raise Stop(f"tag {name}: its commit {sha[:7]} isn't here after fetch")
        if email.strip().lower() not in ORION_EMAILS:
            move.append((name, sha, email.strip()))
    if not move:
        out("[3/5] every tag already points at a commit by Orion: nothing to do")
        out("RETAG OK")
        return 0
    for name, sha, email in move:
        out(f"[3/5] {name}: {sha[:7]} by someone else -> {main[:7]}")
    if not go:
        out(f"Dry run: {len(move)} tag(s) would move. To do it: a file tools\\fresh-history\\CONFIRM-retag holding "
            f"{main[:7]}, then the retag-leyline step.")
        return 0
    expected = open(confirm_file, encoding="utf-8").read().strip() if confirm_file and os.path.isfile(confirm_file) else ""
    if not expected:
        raise Stop(f"{confirm_file} is missing: nothing done")
    if not main.startswith(expected):
        raise Stop(f"GitHub's main is {main[:7]}, CONFIRM expects {expected[:7]}: nothing done")
    backup_dir = backup_dir or os.path.join(os.path.expanduser("~"), "Desktop")
    os.makedirs(backup_dir, exist_ok=True)
    bundle = os.path.join(backup_dir, "Leyline-tags-before-retag-" + now("%Y%m%d-%H%M%S") + ".bundle")
    rc, txt = git("bundle", "create", bundle, *[f"refs/tags/{n}" for n, _, _ in move])
    if rc != 0:
        raise Stop("the backup bundle failed: " + txt)
    out(f"[4/5] backup of the old tags: {bundle}")
    for name, _, _ in move:
        rc, txt = git("push", "-f", "origin", f"{main}:refs/tags/{name}")
        if rc != 0:
            raise Stop(f"moving {name} failed: {txt}")
        git("tag", "-f", name, main)
    out(f"[5/5] moved {len(move)} tag(s) onto {main[:7]}; releases and their files stay")
    out("RETAG OK")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--root", required=True)
    ap.add_argument("--go", action="store_true")
    ap.add_argument("--confirm")
    a = ap.parse_args(argv)
    try:
        return retag(a.root, go=a.go, confirm_file=a.confirm)
    except Stop as e:
        print("STOPPED: " + str(e))
        return 1


if __name__ == "__main__":
    sys.exit(main())
