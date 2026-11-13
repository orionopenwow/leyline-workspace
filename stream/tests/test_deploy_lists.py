"""Every file the portal loads from beside itself is shipped by the signin step.

The portal (wrath_portal.py) and its modules load their siblings with _load_module(name, "x.py"),
importlib's spec_from_file_location(..., "x.py") or open(os.path.join(HERE, "x.json")). A missing file is quietly
None, so a module the deploy forgets never fails a test: the feature is just not live (audit 12 Nov: weekly.py,
weekly.json, denweek.py, denmarket.py, gamemods.py and brandfile.py were in no deploy list).

This test fails when a file that the stream code loads by name is missing from any of:
  * "Update sign-in and warm-up.bat": the files packed into the upload (LINKS and friends),
  * the same .bat's `sed -i 's/\\r$//'` list (every shipped .py loses Windows line ends),
  * fastloop.sh: the files it installs into the portal, backs up and puts back on a rollback.

    python3 -I stream/tests/test_deploy_lists.py
"""
import os
import re
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from test_fastloop import installed_files, put_back_body  # noqa: E402

BAT = os.path.join(STREAM, "Update sign-in and warm-up.bat")
FASTLOOP = os.path.join(STREAM, "fastloop.sh")

# loaded by name but deliberately not shipped by the signin step (name -> why)
NOT_SHIPPED = {
    "feedback_scorer.py": "an optional scorer path given on the command line, not a sibling",
}


def loaded_siblings():
    """file -> the stream modules that load it, for every sibling .py/.json named in stream/*.py that exists in stream/."""
    names = set(os.listdir(STREAM))
    out = {}
    pats = [
        re.compile(r'_load_module\(\s*"[^"]+"\s*,\s*"([\w.-]+\.py)"'),
        re.compile(r'spec_from_file_location\(\s*"[^"]+"\s*,\s*os\.path\.join\([^)]*?"([\w.-]+\.py)"\)', re.S),
        re.compile(r'os\.path\.join\(\s*(?:HERE|os\.path\.dirname\(os\.path\.abspath\(__file__\)\)|_HERE|BASE)\s*,\s*"([\w.-]+\.(?:py|json))"'),
    ]
    for fn in sorted(names):
        if not fn.endswith(".py"):
            continue
        with open(os.path.join(STREAM, fn), encoding="utf-8") as fh:
            src = fh.read()
        for p in pats:
            for m in p.finditer(src):
                f = m.group(1)
                if f in names and f != fn:
                    out.setdefault(f, set()).add(fn)
    return out


def portal_closure():
    """The files reachable from wrath_portal.py through the loads above (what the live portal can try to load)."""
    graph = {}
    for f, users in loaded_siblings().items():
        for u in users:
            graph.setdefault(u, set()).add(f)
    seen, todo = set(), ["wrath_portal.py"]
    while todo:
        cur = todo.pop()
        for nxt in graph.get(cur, ()):
            if nxt not in seen:
                seen.add(nxt)
                todo.append(nxt)
    return seen


def bat_packed(src):
    """Every stream file name the .bat names on a LINKS / DOCS / tar line."""
    out = set()
    for ln in src.splitlines():
        if "LINKS=" in ln or "DOCS=" in ln or ln.startswith("tar "):
            out.update(re.findall(r"[\w-]+\.(?:py|json)", ln))
    return out


def fastloop_installed(src):
    """installed_files() plus the `for f in a b c; do ... install -m 644 "$f" ... done` loops."""
    out = set(installed_files(src))
    for m in re.finditer(r"for f in ([^;\n]+); do(.*?)\bdone\b", src, re.S):
        if re.search(r'install -m 644 "\$f"', m.group(2)):
            out.update(re.findall(r"[\w-]+\.(?:py|json)", m.group(1)))
    return out


def bat_sed_list(src):
    m = re.search(r"sed -i 's/\\r\$//' (.*?)2>/dev/null", src)
    assert m, "the .bat's sed line"
    return set(re.findall(r"[\w-]+\.py", m.group(1)))


class DeployLists(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with open(BAT, encoding="utf-8", errors="replace") as f:
            cls.bat = f.read()
        with open(FASTLOOP, encoding="utf-8") as f:
            cls.fl = f.read()
        cls.loaded = portal_closure() - set(NOT_SHIPPED)

    def test_parser_found_the_loads(self):
        # the known ones, so a regex that stops matching fails here and not silently
        for f in ("docs.py", "hub.py", "weekly.py", "weekly.json", "denweek.py", "gamemods.py", "ratelimit.py",
                  "playmodes.json"):
            self.assertIn(f, self.loaded)
        self.assertGreater(len(self.loaded), 45)

    def test_bat_packs_every_loaded_file(self):
        missing = sorted(self.loaded - bat_packed(self.bat))
        self.assertEqual(missing, [], "the portal loads these but the signin .bat never uploads them")

    def test_bat_strips_crlf_from_every_shipped_py(self):
        shipped = {f for f in bat_packed(self.bat) if f.endswith(".py")}
        missing = sorted(shipped - bat_sed_list(self.bat) - {"wrath_portal.py"})
        self.assertEqual(missing, [], "shipped .py files the server never strips CR from")

    def test_fastloop_installs_every_loaded_file(self):
        missing = sorted(self.loaded - fastloop_installed(self.fl))
        self.assertEqual(missing, [], "the portal loads these but fastloop.sh never installs them")

    def test_fastloop_rolls_back_every_loaded_file(self):
        body = put_back_body(self.fl)
        missing = sorted(f for f in self.loaded if f not in body)
        self.assertEqual(missing, [], "installed but not put back by put_back()")


if __name__ == "__main__":
    unittest.main(verbosity=1)
