"""Play sessions survive a portal restart (live 1 Nov: a site deploy signed every player out mid-game, their /data
requests redirected and a world load waited forever). Kept in a 0600 file; expired ones are not taken back.

    python3 -I stream/tests/test_play_sessions.py stream/wrath_portal.py
"""
import importlib.util, json, os, stat, sys, tempfile, time

os.environ.setdefault("SOAP_USER", "X"); os.environ.setdefault("SOAP_PASS", "x")
spec = importlib.util.spec_from_file_location("wp", sys.argv[1])
wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)
fails = 0
def check(name, ok, got=None):
    global fails
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else f" {got!r}")); fails += 0 if ok else 1

d = tempfile.mkdtemp(); path = os.path.join(d, "play-sessions.json")
a = wp.Sessions(); check("no file yet: nothing kept", a.load(path) == 0)
code = a.create("wr000001", "streampass", "mini", 2)
sid = a.redeem(code)
a.set_hop(sid, {"id": "buildersrealm234", "name": "Builders' Realm"})
check("saved 0600", os.path.exists(path) and stat.S_IMODE(os.stat(path).st_mode) == 0o600, oct(os.stat(path).st_mode))
b = wp.Sessions()   # the restarted portal
check("a restart keeps the session", b.load(path) == 1 and (b.get(sid) or {}).get("user") == "WR000001"
      and b.get(sid)["hop"]["id"] == "buildersrealm234")
b.clear_hop(sid); c = wp.Sessions(); c.load(path)
check("going home is kept too", "hop" not in c.get(sid))
raw = json.load(open(path)); raw[sid]["exp"] = time.time() - 1; json.dump(raw, open(path, "w"))
check("an expired session is not taken back", wp.Sessions().load(path) == 0)
open(path, "w").write("{broken")
e = wp.Sessions(); check("a broken file: a fresh start", e.load(path) == 0 and e.get(sid) is None)
e.create("wr000002", "p", "mini", 1); check("then saved again", len(json.load(open(path))) == 1)
c.end_user("WR000001"); check("sign-out removes it from the file", sid not in json.load(open(path)))
print(f"{9 - fails}/9 passed" if not fails else f"{fails} failed"); sys.exit(1 if fails else 0)
