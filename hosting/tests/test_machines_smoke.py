"""The separate-machines smoke (tests/smoke/machines.py): relay, host's PC and players as their own processes and
addresses on the loopback /8, so a knock from one computer never lets another one in. About 20 s."""
import os
import subprocess
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))


@unittest.skipUnless(sys.platform.startswith("linux"), "needs the whole 127/8 on loopback (Linux)")
class SeparateMachines(unittest.TestCase):
    def test_private_then_public_between_computers(self):
        p = subprocess.run([sys.executable, os.path.join(HERE, "smoke", "machines.py")], capture_output=True,
                           text=True, timeout=600)
        self.assertEqual(p.returncode, 0, p.stdout[-3000:] + p.stderr[-2000:])
        self.assertIn('"ok": true', p.stdout)


if __name__ == "__main__":
    unittest.main()
