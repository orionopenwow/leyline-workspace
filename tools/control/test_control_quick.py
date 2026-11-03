#!/usr/bin/env python3
"""Quick Claude edits (tools/control/quick.py, the apply-check / apply steps): a manifest of files with their sha256
and one deploy step; every file inside the Leyline folder, no secrets, no game data, byte for byte what the manifest
says, under the folders its deploy step ships; anything else refused with nothing deployed; the deploy is the same
.bat Control runs for that step; a log line per apply; Control registers both steps (apply-check read-only) and hands
each step its queue id and note.

    python3 -I tools/control/test_control_quick.py
"""
import ast, hashlib, importlib.util, io, json, os, contextlib, tempfile, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("quick", HERE / "quick.py")
quick = importlib.util.module_from_spec(spec)
spec.loader.exec_module(quick)


def control_steps():
    tree = ast.parse((HERE / "control.py").read_text(encoding="utf-8"))
    out = {}
    for node in tree.body:
        if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name) and node.targets[0].id in ("STEPS", "READ_ONLY"):
            out[node.targets[0].id] = ast.literal_eval(node.value)
    return out


class Rig(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="leyline-quick-"))
        quick.QUICK = self.root / "control" / "quick"
        quick.APPLIED = quick.QUICK / "applied.jsonl"
        self.ran = []

    def put(self, rel, data=b"print('hi')\n"):
        p = self.root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data)
        return {"path": rel, "sha256": hashlib.sha256(data).hexdigest()}

    def manifest(self, name, doc):
        p = self.root / "control" / "quick" / f"{name}.json"
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps(doc) if not isinstance(doc, str) else doc, encoding="utf-8")

    def run_quick(self, *argv, rc=0):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            got = quick.main(list(argv), runner=lambda bat: (self.ran.append(bat), rc)[1], root=self.root)
        return got, out.getvalue()

    def applied(self):
        try:
            return [json.loads(l) for l in quick.APPLIED.read_text(encoding="utf-8").splitlines()]
        except FileNotFoundError:
            return []


class Apply(Rig):
    def test_check_then_apply_runs_the_named_step_once(self):
        f = self.put("stream/forum.py")
        self.manifest("1311a", {"files": [f], "deploy": "signin", "note": "Forum: a typo"})
        rc, out = self.run_quick("check", "--id", "1311a")
        self.assertEqual(rc, 0, out)
        self.assertIn("QUICK CHECK OK (nothing deployed)", out)
        self.assertEqual(self.ran, [], "check deploys nothing")
        self.assertEqual(self.applied(), [], "check logs nothing")
        rc, out = self.run_quick("apply", "--id", "1311b", "--note", "quick:1311a  Forum: a typo")
        self.assertEqual(rc, 0, out)
        self.assertEqual(self.ran, ["stream/Update sign-in and warm-up.bat"])
        self.assertIn(f"stream/forum.py  {f['sha256'][:12]}", out)
        self.assertNotIn("print('hi')", out, "never a file's contents")
        log = self.applied()
        self.assertEqual((log[0]["id"], log[0]["deploy"], log[0]["ok"]), ("1311a", "signin", True))
        self.assertEqual(log[0]["files"], [["stream/forum.py", f["sha256"][:12]]])

    def test_the_deploy_steps_are_controls_own(self):
        steps = control_steps()["STEPS"]
        self.assertEqual(quick.DEPLOY["signin"][0], steps["signin"][0])
        self.assertEqual(quick.DEPLOY["addons"][0], steps["addons"][0])
        self.assertEqual(quick.DEPLOY["scripts"][0], steps["scripts"][0])
        self.assertEqual(set(quick.DEPLOY), {"signin", "addons", "scripts", "none"})

    def test_none_checks_and_logs_only(self):
        self.manifest("n1", {"files": [self.put("docs/NOTE.md", b"# hi\n")], "deploy": "none"})
        rc, out = self.run_quick("apply", "--id", "n1")
        self.assertEqual(rc, 0, out)
        self.assertEqual(self.ran, [])
        self.assertTrue(self.applied()[0]["ok"])

    def test_a_failed_step_is_said_and_logged(self):
        self.manifest("f1", {"files": [self.put("realm/custom/lua_scripts/ov_x.lua", b"-- x\n")], "deploy": "scripts"})
        rc, out = self.run_quick("apply", "--id", "f1", rc=3)
        self.assertEqual(rc, 3)
        self.assertIn("failed (exit 3)", out)
        self.assertFalse(self.applied()[0]["ok"])

    def test_addons_ships_addons_and_the_addon_ui(self):
        self.manifest("a1", {"files": [self.put("addons/OVQoL/OVQoL.lua"), self.put("stream/fui-web/Interface/AddOns/WrathUnleashedUI/Den.lua")],
                             "deploy": "addons"})
        self.assertEqual(self.run_quick("apply", "--id", "a1")[0], 0)
        self.assertEqual(self.ran, ["addons/pack/Publish Leyline addons.bat"])


class Refuse(Rig):
    def refused(self, doc, why, name="r1"):
        self.manifest(name, doc)
        rc, out = self.run_quick("apply", "--id", name)
        self.assertEqual(rc, 1, out)
        self.assertIn("Refused, nothing deployed", out)
        self.assertRegex(out, why)
        self.assertEqual(self.ran, [], "nothing ran")
        return out

    def test_a_changed_file(self):
        f = self.put("stream/forum.py")
        (self.root / "stream/forum.py").write_bytes(b"print('changed')\n")
        self.refused({"files": [f], "deploy": "signin"}, r"isn't what Claude sent")
        self.assertFalse(self.applied()[0]["ok"], "the refusal is logged")

    def test_paths_outside_or_odd(self):
        good = self.put("stream/a.py")
        for p, why in [("../outside.py", r"no '\.', '\.\.'"), ("/etc/passwd", "relative path"), ("C:/Windows/x.py", "relative path"),
                       ("stream\\a.py", "forward slashes"), ("stream//a.py", "empty parts"), ("stream/./a.py", r"no '\.'"),
                       (".git/config", "not a file Control deploys"), ("control/autopilot.json", "not a file Control deploys"),
                       ("logs/x.log", "not a file Control deploys"), ("", "1 to 260")]:
            self.refused({"files": [{"path": p, "sha256": good["sha256"]}], "deploy": "none"}, why)

    def test_secrets_and_game_data(self):
        for p in ["stream/.env", "hosting/discord/deploy/discord.env", "tools/discord-leyline/token.txt", "keys/release.key",
                  "certs/site.pem", "stream/my_password.txt", "stream/client_secret.json", "credentials.json", "client/Data/patch-4.MPQ",
                  "tools/x.exe", "tools/x.dll"]:
            f = self.put(p, b"x")
            self.refused({"files": [f], "deploy": "none"}, r"secret|game data|program")

    def test_a_link_out_of_the_folder(self):
        outside = Path(tempfile.mkdtemp()) / "x.py"
        outside.write_bytes(b"x")
        (self.root / "stream").mkdir(parents=True, exist_ok=True)
        os.symlink(outside, self.root / "stream" / "link.py")
        self.refused({"files": [{"path": "stream/link.py", "sha256": hashlib.sha256(b"x").hexdigest()}], "deploy": "signin"}, "outside the Leyline folder")

    def test_the_wrong_step_for_the_folder(self):
        self.refused({"files": [self.put("realm/custom/lua_scripts/ov_x.lua")], "deploy": "signin"}, "deploys only stream/")
        self.refused({"files": [self.put("stream/x.py")], "deploy": "scripts"}, "deploys only realm/custom/", "r2")

    def test_shape(self):
        f = self.put("stream/a.py")
        self.refused({"files": [f], "deploy": "ship"}, "deploy must be one of")
        self.refused({"files": [f], "deploy": "signin", "run": "rm -rf /"}, "takes only files, deploy, note", "r2")
        self.refused({"files": [], "deploy": "signin"}, "a list of 1 to", "r3")
        self.refused({"files": [dict(f, extra=1)], "deploy": "signin"}, "nothing else", "r4")
        self.refused({"files": [f, f], "deploy": "signin"}, "listed twice", "r5")
        self.refused({"files": [{"path": "stream/a.py", "sha256": "ABC"}], "deploy": "signin"}, "64 lowercase hex", "r6")
        self.refused({"files": [{"path": "stream/none.py", "sha256": "0" * 64}], "deploy": "signin"}, "not on this PC", "r7")
        self.refused("not json {", "not JSON", "r8")

    def test_no_manifest_or_a_bad_name(self):
        rc, out = self.run_quick("apply", "--id", "nothing-here")
        self.assertEqual(rc, 1)
        self.assertIn("No manifest at control/quick/nothing-here.json", out)
        rc, out = self.run_quick("apply", "--id", "../../etc")
        self.assertIn("No manifest name", out)
        rc, out = self.run_quick("apply", "--id", "x", "--note", "quick:../x")
        self.assertIn("No manifest name", out)
        self.assertEqual(self.ran, [])

    def test_unknown_arguments(self):
        self.assertEqual(self.run_quick("deploy")[0], 2)
        self.assertEqual(self.run_quick("apply", "--force")[0], 2)


class Control(unittest.TestCase):
    def test_both_steps_registered_check_read_only(self):
        s = control_steps()
        self.assertEqual(s["STEPS"]["apply-check"][0], "tools/control/Quick apply - check.bat")
        self.assertEqual(s["STEPS"]["apply"][0], "tools/control/Quick apply.bat")
        self.assertIn("apply-check", s["READ_ONLY"])
        self.assertNotIn("apply", s["READ_ONLY"], "a deploy waits for Launch all when Ask before deploys is on")
        root = HERE.parents[1]
        for bat, verb in [("Quick apply - check.bat", "check"), ("Quick apply.bat", "apply")]:
            raw = (HERE / bat).read_bytes()
            self.assertIn(b"\r\n", raw, "CRLF")
            self.assertNotIn(b"\n", raw.replace(b"\r\n", b""), "CRLF only")
            self.assertIn(f"tools\\control\\quick.py {verb}".encode(), raw)
            self.assertNotIn(b"pause", raw.lower())
        self.assertTrue((root / "tools/control/README.md").exists())

    def test_control_hands_the_step_its_id_and_note(self):
        src = (HERE / "control.py").read_text(encoding="utf-8")
        self.assertIn('env["LEYLINE_CONTROL_ID"] = str(item.get("id") or "")', src)
        self.assertIn('env["LEYLINE_CONTROL_NOTE"] = str(item.get("note") or "")[:200]', src)


if __name__ == "__main__":
    unittest.main(verbosity=2)
