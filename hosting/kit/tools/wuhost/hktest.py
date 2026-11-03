"""Test Hero Key sign-in: a full dry run on this realm, with nothing left behind (docs/leyline/join-network.md).

    python -m wuhost.hktest [--state /state] [--json]

Runs where the agent runs (it needs the realm's database settings: the kit's wu-agent service, or the linked agent's
state/link-db.env). Each step prints one plain line; the exit code is 0 only when every step passed.

  1. switched on   host.json "herokey": true (Network tab, or `leyline herokey on`)
  2. site key      the website's key is pinned (the kit's own, or state/herokey-site-keys.json from join-network)
  3. database      the agent reaches the login database, the server is for the 3.3.5a client, it may add accounts
  4. assertion     a test assertion signed with a throwaway key (never the website's) passes the same check the
                   agent runs for a real sign-in; a replay, another realm's assertion and a changed byte are refused
  5. account       inside one database transaction that is rolled back: the HK account is made, its one-time login
                   set and read back, and an SRP6 login with the one-time password is checked against it
  6. relay         the relay says this realm takes Hero Key sign-in (GET /api/realms/<id>)

Never prints the database password, the one-time password or any key's secret half.
"""
import argparse
import json
import os
import secrets
import sys
import urllib.request

from wunet import herokey, keys, srp6

from .agent import read_json

OFFERED_BUILD = 12340


def _assertion(secret, rid, sub, now, ttl=120):
    pub = keys.public_key(secret)
    pw = secrets.token_urlsafe(12)[:16]
    user = herokey.account_name(sub)
    while True:
        salt, ver = srp6.make_verifier(user, pw)
        if salt[-1] != 0:      # a salt every core keeps at 32 bytes (coredb.salt_ok)
            break
    a = {"v": 1, "iss": "selftest.invalid", "kid": herokey.key_id(pub), "aud": rid, "sub": sub, "salt": salt.hex(),
         "verifier": ver.hex(), "login_ttl": 120, "nonce": secrets.token_hex(16), "iat": int(now), "exp": int(now) + ttl}
    return a, keys.sign_doc(secret, a, herokey.CTX), user, pw


def srp_login_ok(user, password, salt_hex, ver_hex):
    """A whole SRP6 exchange (wunet/srp6.py's client and the server half the fake authserver uses, mirroring the core)
    with the stored salt and verifier: True when the password logs in."""
    salt, ver = bytes.fromhex(salt_hex), bytes.fromhex(ver_hex)
    server = srp6.Server(user, salt, ver)
    client = srp6.Client(user, password)
    A, M1 = client.process(server.B, salt)
    got = server.verify(A, M1)
    return got is not None and client.check_server(got[1])


def _get_json(url, timeout=6):
    req = urllib.request.Request(url, headers={"User-Agent": "wu-hktest/1", "Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read(65536).decode("utf-8"))


def run(state_dir, db, get_json=_get_json, now=None, out=print):
    """[(step, ok, line)], and print each line. `db` is the agent's database object (agent.RealmDB or coredb.*)."""
    import time
    now = int(time.time() if now is None else now)
    cfg = read_json(os.path.join(state_dir, "host.json"), {}) or {}
    rid = cfg.get("realm_id") or ""
    rows = []

    def step(name, ok, line):
        rows.append((name, bool(ok), line))
        out(("  OK      " if ok else "  PROBLEM ") + line)

    step("on", cfg.get("herokey") is True, "Hero Key sign-in is switched on for this realm." if cfg.get("herokey") is True
         else "Hero Key sign-in is off for this realm: turn it on in the Network tab (or: leyline herokey on).")
    if cfg.get("official") is True:
        step("on", False, "This is the operator's own realm: it keeps its own sign-in.")
    pinned = herokey.pinned_keys(state_dir=state_dir)
    step("key", bool(pinned), f"The website's key is pinned ({', '.join(sorted(pinned))})." if pinned else
         "The website's key is not pinned yet: run join-network again, or `leyline herokey pin`, once the website "
         "has Hero Key sign-in on.")
    probe = None
    if db is None:
        step("db", False, "The agent has no database settings: run join-network with your login database details.")
    else:
        try:
            probe = db.probe()
        except Exception as e:   # noqa: BLE001: the reason is the line
            step("db", False, f"Could not reach the login database ({type(e).__name__}): check the address, user and "
                              "that this PC may connect to it.")
        if probe is not None:
            b = probe.get("build")
            step("db", b == OFFERED_BUILD and probe.get("can_write"),
                 f"Login database reached: realm \"{probe.get('realm_name') or '?'}\", {probe.get('accounts')} accounts."
                 + ("" if b == OFFERED_BUILD else f" Its realm list is for another client (build {b}): Hero Key sign-in is "
                                                    "for servers of the Wrath client for now.")
                 + ("" if probe.get("can_write") else " The database user may not add accounts: give it INSERT and "
                                                      "UPDATE on the login database."))
    # 4: the same check a real sign-in goes through, with a throwaway key
    test_secret = keys.new_secret()
    test_keys = {herokey.key_id(keys.public_key(test_secret)): keys.public_key(test_secret)}
    sub = "".join(secrets.choice("abcdefghijklmnopqrstuvwxyz234567") for _ in range(26))
    a, sig, user, pw = _assertion(test_secret, rid or "aaaaaaaaaaaaaaaa", sub, now)
    seen = herokey.ReplayCache()
    good, why = herokey.check(a, sig, rid, test_keys, seen, now=now)
    _, replay = herokey.check(a, sig, rid, test_keys, seen, now=now)
    other, osig, _, _ = _assertion(test_secret, "bbbbbbbbbbbbbbbb" if rid != "bbbbbbbbbbbbbbbb" else "cccccccccccccccc",
                                   sub, now)
    _, wrong_aud = herokey.check(other, osig, rid, test_keys, herokey.ReplayCache(), now=now)
    _, tampered = herokey.check(dict(a, nonce=secrets.token_hex(16)), sig, rid, test_keys, herokey.ReplayCache(), now=now)
    ok4 = good is not None and replay == "replay" and wrong_aud == "aud" and tampered == "sig"
    step("assertion", ok4, "A test sign-in passes the check, and a replay, another realm's and a changed one are refused."
         if ok4 else f"The sign-in check did not behave: {why or ''} {replay} {wrong_aud} {tampered}".strip())
    # 5: the account, rolled back
    if db is not None and probe is not None and hasattr(db, "dry_run"):
        try:
            with db.dry_run():
                r = db.create_account(user, a["salt"], a["verifier"])
                back = db.get_login(user) if r == "ok" else None
            ok5 = r == "ok" and back is not None and srp_login_ok(user, pw, back[0], back[1])
            step("account", ok5, f"A test account {user} was made, its one-time login checked, and all of it undone."
                 if ok5 else f"Making the test account failed ({r}): Hero Key players could not get in.")
        except Exception as e:   # noqa: BLE001
            step("account", False, f"The test account could not be made ({type(e).__name__}): the database user needs "
                                   "INSERT and UPDATE on the login database.")
    else:
        step("account", False, "Skipped the account test: the database is not reachable.")
    # 6: the relay's view
    relay = cfg.get("relay")
    if relay and rid:
        try:
            info = get_json(f"https://{relay}/api/realms/{rid}")
            on = isinstance(info, dict) and info.get("herokey") is True
            step("relay", on, "The relay offers Hero Key sign-in for this realm." if on else
                 "The relay does not offer Hero Key sign-in for this realm yet: the agent tells it within a minute of the "
                 "steps above passing (restart the agent if it stays this way).")
        except Exception as e:   # noqa: BLE001
            step("relay", False, f"Could not ask the relay ({type(e).__name__}): is this PC online?")
    return rows


def main(argv=None):
    ap = argparse.ArgumentParser(description="Test Hero Key sign-in (a dry run)")
    ap.add_argument("--state", default=os.environ.get("WU_STATE", "/state"))
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args(argv)
    from .coredb import open_db
    rows = run(a.state, open_db(), out=(lambda s: None) if a.json else print)
    if a.json:
        print(json.dumps({"ok": all(r[1] for r in rows), "steps": [{"id": r[0], "ok": r[1], "text": r[2]} for r in rows]}))
    return 0 if rows and all(r[1] for r in rows) else 1


if __name__ == "__main__":
    sys.exit(main())
