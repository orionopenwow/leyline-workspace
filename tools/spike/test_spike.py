#!/usr/bin/env python3
"""tools/spike/spike_test.py against local fakes (review 4 Nov): no server is touched.

A fake website program (/__auth answers 401, /wrath/realms 200) and a fake wu-play (/data/__index and a few files);
part D finds nothing on 127.0.0.1:443 (or whatever is there) and is still summarised. Checks the summary lines, the
SPIKE RESULT JSON, the bytes counted for game files, and the plain SPIKE WARN lines for failing or busy parts.

    python3 -I tools/spike/test_spike.py
"""
import http.server, importlib.util, json, subprocess, sys, threading, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("spike", HERE / "spike_test.py")
spike = importlib.util.module_from_spec(spec)
spec.loader.exec_module(spike)


class Portal(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        code = 401 if self.path.startswith("/__auth") else 200 if self.path == "/wrath/realms" else 404
        b = b"<html>realms</html>" if code == 200 else b""
        self.send_response(code); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)


class Play(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        if self.path == "/data/__index":
            b = json.dumps(["World/a.blp", "World/b.m2", "readme.txt"]).encode()
        elif self.path.startswith("/data/"):
            b = b"x" * 65536
        else:
            b = b""
        self.send_response(200); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)


def serve(h):
    s = http.server.ThreadingHTTPServer(("127.0.0.1", 0), h)
    s.daemon_threads = True
    threading.Thread(target=s.serve_forever, daemon=True).start()
    return s


class Spike(unittest.TestCase):
    def test_against_fakes(self):
        p, q = serve(Portal), serve(Play)
        r = subprocess.run([sys.executable, "-I", str(HERE / "spike_test.py"), "--portal",
                            f"http://127.0.0.1:{p.server_address[1]}", "--play", f"http://127.0.0.1:{q.server_address[1]}",
                            "--site", "play.example.test", "--workers", "4", "--seconds", "5"],
                           capture_output=True, text=True, timeout=180)
        out = r.stdout
        self.assertEqual(r.returncode, 0, out + r.stderr)
        self.assertIn("A. game-file check (/__auth):", out)
        self.assertIn("B. website page (/wrath/realms):", out)
        self.assertIn("C. game files (wu-play):", out)
        res = json.loads(next(ln for ln in out.splitlines() if ln.startswith("SPIKE RESULT "))[13:])
        by = {x["name"][0]: x for x in res}
        self.assertEqual(by["A"]["errors"], 0)
        self.assertEqual(set(by["A"]["codes"]), {"401"})
        self.assertGreater(by["C"]["mb_s"], 0)
        self.assertTrue(out.rstrip().endswith("SPIKE TEST OK"))

    def test_verdicts(self):
        good = {"name": "A", "codes": {"200": 100}, "errors": 0, "p95_ms": 40}
        self.assertEqual(spike.verdicts([good, {}]), [])
        bad = {"name": "B", "codes": {"200": 80, "503": 20}, "errors": 20, "p95_ms": 2500}
        w = spike.verdicts([bad])
        self.assertEqual(len(w), 3, w)
        self.assertIn("20% failed", w[0])


if __name__ == "__main__":
    unittest.main()
