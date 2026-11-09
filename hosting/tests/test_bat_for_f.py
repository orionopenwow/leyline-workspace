"""The kit's .bat files: no bare = , ; inside a FOR /F command.

cmd turns an unquoted =, comma or semicolon in a `for /f ... in (`command`)` into a space. On 9 Nov a host saw
"docker: invalid reference format" after "== Your realm": `-e WU_KIT=/kit` became `-e WU_KIT /kit`, so docker took
/kit as the image name and the earlier answers were never read. Quote such arguments ("WU_KIT=/kit") or escape them (^=).
"""
import glob
import os
import re
import unittest

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BATS = sorted(glob.glob(os.path.join(ROOT, "hosting", "**", "*.bat"), recursive=True)
              + glob.glob(os.path.join(ROOT, "tools", "**", "*.bat"), recursive=True)
              + glob.glob(os.path.join(ROOT, "*.bat")))
FORF = re.compile(r"for\s+/f\b[^(]*\(\s*([`'])(.*)\1\s*\)", re.I)


def bare(cmd: str) -> list[str]:
    """The = , ; outside double quotes and not escaped with ^."""
    out, q, i = [], False, 0
    while i < len(cmd):
        c = cmd[i]
        if c == '"':
            q = not q
        elif c == "^":
            i += 1
        elif not q and c in "=,;":
            out.append(c)
        i += 1
    return out


class ForF(unittest.TestCase):
    def test_no_bare_delimiters_in_for_f_commands(self):
        bad = []
        for p in BATS:
            for n, line in enumerate(open(p, encoding="utf-8", errors="replace"), 1):
                m = FORF.search(line)
                if m and bare(m.group(2)):
                    bad.append(f"{os.path.relpath(p, ROOT)}:{n}: {line.strip()[:160]}")
        self.assertEqual(bad, [], "quote or ^-escape = , ; inside these FOR /F commands:\n" + "\n".join(bad))

    def test_the_rule_itself(self):
        self.assertEqual(bare('docker run -v "a=b" -e WU_KIT=/kit x'), ["="])
        self.assertEqual(bare('docker run -v "a=b" -e "WU_KIT=/kit" x'), [])
        self.assertEqual(bare('wmic os where name^="x" get a^,b'), [])


if __name__ == "__main__":
    unittest.main()
