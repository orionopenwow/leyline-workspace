"""discord_read.py against a fake Discord: GET only, oldest first, the newest id kept per channel, a channel the bot
can't read skipped, the token never printed.   python3 tools/discord-leyline/test_discord_read.py"""
import contextlib, io, json, sys, tempfile
from pathlib import Path
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import discord_read as dr  # noqa: E402
import discord_leyline as dl  # noqa: E402

TOKEN = "abc.def.ghi-secret"
calls = []


class Fake:
    def __init__(self, *a, **k): pass

    def request(self, m, path, body=None, query=None):
        calls.append(m)
        if m != "GET":
            raise AssertionError("wrote")
        if path == "/users/@me/guilds": return [{"id": "1", "name": "Leyline"}]
        if path == "/guilds/1": return {"name": "Leyline"}
        if path == "/guilds/1/channels":
            return [{"id": "9", "type": 4, "name": "PLAY"}, {"id": "10", "type": 0, "name": "general", "parent_id": "9"},
                    {"id": "11", "type": 0, "name": "staff"}]
        if path == "/guilds/1/threads/active": return {"threads": []}
        if path == "/channels/10/messages":
            if query["after"] == "500": return []
            return [{"id": "500", "content": "stuck at loading", "timestamp": "2026-11-04T02:50:00", "author": {"username": "a"}},
                    {"id": "400", "content": "hi " + TOKEN, "timestamp": "2026-11-04T02:40:00", "author": {"username": "b", "global_name": "Bee"}}]
        if path == "/channels/11/messages": raise dl.ApiError(403, "Missing Access", 50001)
        raise AssertionError(path)


def main():
    tmp = Path(tempfile.mkdtemp())
    dr.LIVE, dr.STATE, dl.Discord = tmp, tmp / "read-state.json", Fake
    tok = tmp / "token.txt"; tok.write_text(TOKEN)
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        assert dr.main(["--from-control", "--token-file", str(tok)]) == 0
    t = (tmp / "inbox-latest.md").read_text()
    checks = [
        ("oldest first, display names", t.index("**Bee**") < t.index("stuck at loading")),
        ("the newest id kept", json.loads(dr.STATE.read_text())["10"] == "500"),
        ("a channel it can't read is skipped", "can't read" in out.getvalue()),
        ("the token never printed", TOKEN not in out.getvalue()),
        ("GET only", set(calls) == {"GET"}),
        ("ends OK", "DISCORD READ OK" in out.getvalue()),
    ]
    with contextlib.redirect_stdout(io.StringIO()):
        dr.main(["--from-control", "--token-file", str(tok)])
    checks.append(("a second read: nothing new", "Nothing new" in (tmp / "inbox-latest.md").read_text()))
    bad = [n for n, ok in checks if not ok]
    for n, ok in checks:
        print(("PASS " if ok else "FAIL ") + n)
    print(f"{len(checks) - len(bad)}/{len(checks)} passed")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
