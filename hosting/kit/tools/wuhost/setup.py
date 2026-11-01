"""Host setup and small admin commands. Runs inside the tools container with the kit folder at
/kit (so the host's PC needs Docker only, no Python).

  python -m wuhost.setup init --name "Frost Haven" --profile main --visibility private
         [--image ghcr.io/... --tag stable] [--relay relay.openvanilla.app] [--site https://openvanilla.app]
         [--bots 40]
  python -m wuhost.setup show                       the invite link, login, status (plain text)
  python -m wuhost.setup get                        earlier answers as KEY=VALUE (for the .bat)
  python -m wuhost.setup friend NAME [PASSWORD]     SQL for a friend's account (piped into mysql)
  python -m wuhost.setup new-invite                 a new invite secret (old links stop working)
  python -m wuhost.setup visibility public|private
  python -m wuhost.setup claim TOKEN                the token the website gave your Hero Key account
  python -m wuhost.setup signup invite|open|off     who may make accounts from the join page
  python -m wuhost.setup browser on|off             browser players: on turns the core's Warden anti-cheat off so
                                                    the Leyline browser client (no Warden module) can join; off
                                                    turns Warden on (stock clients only). AzerothCore rulesets.
  python -m wuhost.setup options [--type pvp --xp 5 --loot 2 --rep 3 --start level1|boost]
                                                    the realm's own settings: type, rates, start (state/realm.env,
                                                    read by the world server at its next start) and its
                                                    realm-list type; also what a seed's "options" set (the
                                                    website's More settings, the in-game Host window)
  python -m wuhost.setup mods [list | add ID[@VERSION] ... | remove ID ...]
                                                    the mods this realm runs or asks players to have, on the realm
                                                    list (docs/GAME-MODS.md): Workshop ids (mod:<id>, up:<slug>) or
                                                    Leyline Modes (mode:<id>). Names only: players' launchers offer
                                                    the ones in the Workshop catalog, checked, when they join
  python -m wuhost.setup rules                      re-read state/rules.txt and state/beacon.json (the
                                                    realm list text) after you edited them
  python -m wuhost.setup init --seed state/seed.b64 a realm made on the website (/wrath/host): its links,
                                                    claim token and answers, from the per-realm download
                                                    (base64 JSON). Used once, then deleted. The realm key
                                                    is made here, on this PC (seed v2); an older v1 seed
                                                    carried a key made by the website.
  python -m wuhost.setup link --name "Frost Haven" --auth-service 127.0.0.1:3724 --world-service 127.0.0.1:8085
         [--build 5875|8606|12340] [--core vmangos] [--type pvp] [--tags ...] [--tagline ...] [--status-url URL]
                                                    a server you ALREADY run (any core) on the Leyline realm list:
                                                    only the agent runs (leyline link); it never touches that
                                                    server's realm list, accounts or GM rights. The key is made here.
  python -m wuhost.setup join-network --name "Frost Haven" --core azerothcore --auth-service 127.0.0.1:3724
         --world-service 127.0.0.1:8085 --db-host 127.0.0.1 [--db-user leyline] [--auth-db acore_auth]
         [--chars-db acore_characters]           (password: WU_JOIN_DB_PASSWORD or --db-password-file, never printed)
                                                    an existing AzerothCore or TrinityCore 3.3.5a server joins the
                                                    Leyline network with Hero Key sign-in: link + the login database
                                                    for HK accounts (state/link-db.env, 0600) + the website's key
                                                    pinned + checks (docs/leyline/join-network.md)
  python -m wuhost.setup herokey on|off|pin|status  Hero Key sign-in for this realm; pin fetches the website's key once
  python -m wuhost.setup share-names on|off         list who is online (character names) on the realm list; off by default
  python -m wuhost.setup official --name "Wrath Unleashed" --relay-url ws://... --listing-url http://...
         --auth-service ov-ac-authserver:3724 --world-service ov-ac-worldserver:8085 [--status-url URL]
                                                    the operator's OWN realm as a Beacon (hosting/hangout):
                                                    public, no signup, its realm list, accounts and GM
                                                    rights left alone. The key is made here, on that box.

Re-running init keeps the realm key, passwords and invite (it only changes what you pass), so
running "Host a Wrath realm" again is always safe.
"""
import argparse
import base64
import json
import os
import re
import secrets
import string
import sys

from wunet import domains, keys, srp6

try:   # the one blocklist for names (wunet/namecheck.py, a copy of stream/namecheck.py with its blocklist.json)
    from wunet import namecheck
except ImportError:
    namecheck = None

from . import cores

KIT = os.environ.get("WU_KIT", "/kit")
STATE = os.path.join(KIT, "state")
NAME_RE = re.compile(r"^[A-Za-z0-9 _'-]{1,32}$")
GM_RE = re.compile(r"^[A-Za-z0-9]{3,16}$")
PROFILES = {"main": "Leyline main: normal Wrath from level 1, or a one-time boost to 80",
            "brawl": "Brawl: everyone starts at 80 in Dalaran with everything unlocked",
            "progression": "Progression: blizzlike 1x, Vanilla then TBC then Wrath per character",
            "dalaran": "Dalaran hangout: light on your PC, no bots, boost to 80 straight to Dalaran",
            "vanillaplus": "Vanilla+: the original world only (level 60 cap), 1.5x XP, made to stay up for good",
            "den": "My Den: your own room to build in, private, for you and the friends you invite",
            "solo": "Solo leveling: a private realm on your PC for levelling alone, with helper bots, dungeons scaled for one"}
# Per-profile settings for .env (docker-compose.host.yml): a Dalaran hangout runs on a small PC.
# A Den (realm/profiles/den.env, docs/DEN.md) is one room on the host's PC: the smallest of all.
PROFILE_ENV = {"dalaran": {"WU_DB_BUFFER": "256M", "WU_PLAYER_LIMIT": "40"},
               "den": {"WU_DB_BUFFER": "256M", "WU_PLAYER_LIMIT": "10"},
               "solo": {"WU_PLAYER_LIMIT": "5"}}
# Solo leveling (realm/profiles/solo.env): a few roaming bots so the world isn't empty; its helper bots are the point.
PROFILE_BOTS = {"dalaran": 0, "vanillaplus": 0, "den": 0, "solo": 20}   # the bot count a new realm starts with
NO_BOTS = ("dalaran", "den")                       # rulesets that never run bots (and the other cores: none yet)
# The text the realm list and the join page show; the host edits these two files (then: Realm tools > rules).
EDITABLE = ("tagline", "summary", "type", "discord", "tags", "region", "language", "max_players", "direct", "mods")
# The realm's mod list (docs/GAME-MODS.md; the same rules as stream/gamemods.py and the relay's listing.py clean_mods).
# Every AzerothCore realm this kit makes runs the Leyline realm scripts (make-kit.sh copies realm/custom/lua_scripts), so
# it lists the built-in Leyline Modes by itself; the host adds Workshop mods with `leyline mods add <id>`.
MOD_ID_RE = re.compile(r"^(?:mod:[a-z][a-z0-9-]{2,31}|up:[a-z0-9][a-z0-9._-]{0,63}|mode:[a-z][a-z0-9-]{1,31})$")
MOD_VERSION_RE = re.compile(r"^[0-9A-Za-z][0-9A-Za-z.+-]{0,31}$")
MAX_MODS = 32
BUILTIN_MODES = ("minigolf", "bumper-balls", "coin-dash", "community")
# The realm list's filters (hosting/relay/wurelay/listing.py checks them again). Tags a profile starts with; the
# host changes them in state/beacon.json or the launcher's Host page. "bots" follows the bot count by itself.
TAGS = ("blizzlike", "boosted", "custom", "pvp", "rp", "hangout", "bots")
REGIONS = ("na-east", "na-west", "sa", "eu", "africa", "me", "asia", "oce")
LANG_RE = re.compile(r"^[a-z]{2,3}(-[A-Z]{2})?$")
PROFILE_TAGS = {"main": ["boosted"], "brawl": ["boosted", "pvp", "custom"], "progression": ["blizzlike"],
                "dalaran": ["hangout", "boosted"], "vanillaplus": ["blizzlike", "custom"], "den": ["custom"],
                "solo": ["custom"]}
PROFILE_MAX = {"dalaran": 40, "den": 10, "solo": 5}
CORE, BUILD = "azerothcore", 12340   # what the host kit runs
DEFAULT_RULES = {"dalaran": [
    "Be kind: no harassment, hate speech or spam.",
    "Duel anyone who agrees; no griefing in Dalaran.",
    "No cheats, bots or exploits. Report bugs with .bug",
    "The host's word is final: the host is the GM here.",
], "den": [
    "This is the host's Den: look around, and build only if the host made you a builder.",
    "Be kind: no harassment, hate speech or spam.",
], "solo": [
    "This is the host's own realm for levelling alone, with helper bots.",
]}
DEFAULT_TAGLINE = {"dalaran": "A Dalaran hangout: boost to 80, duel, queue, meet.",
                   "vanillaplus": "Vanilla+: the original world to level 60, always on.",
                   "den": "A Den: a room to build in, for friends with the invite link.",
                   "solo": "Solo leveling: one player, helper bots, dungeons scaled for one."}
# Host any client: the profiles on other cores (cores.py; their files in profiles/<id>/, steps in
# profiles/host-profile.sh). (core, client build, the login and world services in their compose file)
PROFILE_CORE = {k: (p["core"], p["build"], cores.LOGON, cores.WORLD) for k, p in cores.PROFILES.items()}
for _k, _p in cores.PROFILES.items():
    PROFILES[_k] = _p["label"]
    PROFILE_BOTS[_k] = 0
    PROFILE_TAGS[_k] = list(_p["tags"])
    DEFAULT_TAGLINE[_k] = _p["tagline"]
RULES_HELP = ("# Your realm's rules: one per line (up to 10, 120 characters each). Lines starting with # are\n"
              "# ignored. Shown on the realm list and your join page. After editing: Realm tools > rules.\n")
ALNUM = string.ascii_uppercase + string.digits
B32 = "ABCDEFGHJKMNPQRSTVWXYZ23456789"


SECRET_RE = re.compile(r"^[A-Z2-9]{16,40}$")
CLAIM_RE = re.compile(r"^[A-Za-z0-9_-]{16,64}$")


def read_seed(rel):
    """The website's per-realm seed (base64 of a JSON object), checked field by field; None when the
    file is not there (already used). It never holds a password or an account: only this realm's invite
    and host links, the claim token for the host's Hero Key account (also the pairing token: the website
    finds this realm's id by its hash, wunet/keys.pair_hash) and the answers. v2 (current) has no key: the
    realm key is made on this PC. v1 (the first website build) carried a key the website made."""
    p = rel if os.path.isabs(rel) else os.path.join(KIT, rel)
    try:
        with open(p, encoding="ascii") as f:
            doc = json.loads(base64.b64decode("".join(f.read().split()), validate=True).decode("utf-8"))
    except OSError:
        return None
    except (ValueError, UnicodeDecodeError):
        sys.exit("The realm file from the website is damaged: download it again.")
    v = doc.get("v") if isinstance(doc, dict) else None
    ok = (isinstance(doc, dict) and v in (1, 2)
          and (v == 2 or (isinstance(doc.get("realm_key"), str) and re.fullmatch(r"[0-9a-f]{64}", doc["realm_key"])))
          and (v == 1 or ("realm_key" not in doc and "realm_id" not in doc))
          and all(isinstance(doc.get(k), str) and SECRET_RE.match(doc[k]) for k in ("invite_secret", "host_secret"))
          and (doc.get("claim") is None or (isinstance(doc["claim"], str) and CLAIM_RE.match(doc["claim"])))
          and (v == 1 or isinstance(doc.get("claim"), str))
          and isinstance(doc.get("name"), str) and NAME_RE.match(doc["name"])
          and doc.get("profile") in PROFILES and doc.get("visibility") in ("public", "private")
          and isinstance(doc.get("gm_name"), str) and GM_RE.match(doc["gm_name"])
          and isinstance(doc.get("tagline", ""), str)
          and isinstance(doc.get("rules", []), list) and all(isinstance(r, str) for r in doc.get("rules", [])))
    if not ok:
        sys.exit("The realm file from the website is damaged: download it again.")
    if "options" in doc:
        doc["options"] = check_options(doc["options"], doc["profile"])
        if doc["options"] is None:
            sys.exit("The realm file from the website is damaged: download it again.")
    if v == 1 and keys.realm_id(keys.public_key(bytes.fromhex(doc["realm_key"]))) != doc.get("realm_id"):
        sys.exit("The realm file from the website does not match its realm id: download it again.")
    doc["_path"] = p
    return doc


ANOTHER = ("This kit folder already hosts another realm (state/realm.key). "
           "Use a fresh copy of the host kit for this one.")


def apply_seed(seed, a, cfg):
    """v2: the realm key is made on this PC (init does it); refused when this kit already hosts another realm
    (another claim token). v1: put the website's key in state/realm.key (refused when this kit holds another).
    Then use the seed's answers wherever this run did not pass its own."""
    kp = path("realm.key")
    if seed["v"] == 2:
        if cfg.get("realm_id") and cfg.get("claim") != seed["claim"]:
            sys.exit(ANOTHER)
        cfg["claim"] = seed["claim"]
    else:
        want = bytes.fromhex(seed["realm_key"])
        if os.path.exists(kp):
            if keys.load_or_create(kp) != want:
                sys.exit(ANOTHER)
        else:
            os.makedirs(STATE, exist_ok=True)
            fd = os.open(kp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, "w", encoding="ascii") as f:
                f.write(want.hex() + "\n")
    for k in ("name", "profile", "visibility", "gm_name"):
        if getattr(a, k) is None and not cfg.get(k):
            setattr(a, k, seed[k])
    apply_options(seed.get("options"), a, cfg)
    if a.tagline is None and seed.get("tagline") and not (cfg.get("record") or {}).get("tagline"):
        a.tagline = seed["tagline"]
    cfg.setdefault("invite_secret", seed["invite_secret"])
    cfg.setdefault("host_secret", seed["host_secret"])
    if seed.get("claim") and not cfg.get("claim"):
        cfg["claim"] = seed["claim"]
    if not os.path.exists(path("rules.txt")) and seed.get("rules"):
        with open(path("rules.txt"), "w", encoding="utf-8") as f:
            f.write(RULES_HELP + "".join(" ".join(r.split())[:120] + "\n" for r in seed["rules"][:10] if r.strip()))


# The realm's own settings ("options": the website's More settings, the in-game Host window, `setup.py options`). The
# same names and rules as stream/hostflow.py options(), WrathUnleashedUI/Host.lua and the launcher's deeplink.rs.
TYPES = {"normal": ("PvE", 0), "pvp": ("PvP", 1), "rp": ("RP", 6), "rppvp": ("RP-PvP", 8)}   # list type, GameType
REALM_ENV = "realm.env"


def check_options(o, profile):
    """A seed's options, checked against the profile: the clean dict, or None when anything is wrong."""
    if not isinstance(o, dict) or len(o) > 16:
        return None
    ac = profile not in cores.PROFILES
    out = {}

    def num(k, lo, hi):
        v = o[k]
        return v if isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi else None
    for k in o:
        if k == "type" and o[k] in TYPES:
            out[k] = o[k]
        elif k in ("xp", "loot", "rep") and ac and num(k, 1, 10) is not None:
            out[k] = o[k]
        elif k == "start" and profile == "main" and o[k] in ("level1", "boost"):
            out[k] = o[k]
        elif k == "bots" and num(k, 0, 200) is not None and (o[k] == 0 or (ac and profile not in NO_BOTS)):
            out[k] = o[k]
        elif k == "signup" and o[k] in ("open", "invite", "off"):
            out[k] = o[k]
        elif k == "herokey" and o[k] in ("on", "off"):
            out[k] = o[k]
        elif k == "region" and o[k] in REGIONS:
            out[k] = o[k]
        elif k == "lang" and isinstance(o[k], str) and LANG_RE.match(o[k]):
            out[k] = o[k]
        elif k == "tags" and isinstance(o[k], list) and all(t in TAGS and t != "bots" for t in o[k]):
            out[k] = [t for t in TAGS if t in o[k]]
        elif k == "max" and num(k, 0, 10000) is not None:
            out[k] = o[k]
        elif k == "browser" and o[k] in ("on", "off") and (ac or o[k] == "off"):
            out[k] = o[k]
        elif k not in OPTION_NAMES and isinstance(k, str) and re.fullmatch(r"[a-z_]{1,16}", k):
            continue   # a setting a newer website knows and this kit does not yet: left out, not "damaged"
        else:
            return None
    return out


OPTION_NAMES = ("type", "xp", "loot", "rep", "start", "bots", "signup", "herokey", "region", "lang", "tags", "max", "browser")


def apply_options(o, a, cfg):
    """A seed's options wherever this run did not pass its own: bots, sign-up, Hero Key, the realm list fields (type,
    tags, region, language, max players) and the world server's settings (rates, start, type: state/realm.env)."""
    if not o:
        return
    if a.bots is None and "bots" in o:
        a.bots = o["bots"]
    if a.signup is None and "signup" in o and not cfg.get("signup"):
        a.signup = o["signup"]
    if getattr(a, "browser", None) is None and "browser" in o and "browser" not in cfg:
        a.browser = o["browser"]
    if "herokey" in o and "herokey" not in cfg:
        cfg["herokey"] = o["herokey"] == "on"
    rec = cfg.setdefault("record", {})
    if "type" in o:
        rec["type"] = TYPES[o["type"]][0]
    for k, rk in (("tags", "tags"), ("region", "region"), ("lang", "language"), ("max", "max_players")):
        if k in o:
            rec[rk] = o[k] if k != "max" else (o[k] or None)
    if "max" in o:
        cfg["player_limit"] = o["max"] or None   # the server's own cap too (write_env: WU_PLAYER_LIMIT)
    env = cfg.setdefault("realm_env", {})
    for k in ("type", "xp", "loot", "rep", "start"):
        if k in o:
            env[k] = o[k]


def realm_env_lines(cfg):
    """state/realm.env: the world server's own settings over the profile's (docker-compose.host.yml reads it last)."""
    e = cfg.get("realm_env") or {}
    out = ["# Your realm's own settings over its ruleset's (written by setup.py; read when the world server starts).",
           "# Change them with: leyline options ... (or the launcher's Host page), then restart the realm."]
    if cfg.get("profile") in cores.PROFILES:
        return out + ["# (this core takes its ruleset's settings)"]
    if e.get("type") in TYPES:
        out.append(f"AC_GAME_TYPE={TYPES[e['type']][1]}")
    if e.get("xp"):
        out += [f"AC_RATE_XP_{k}={e['xp']}" for k in ("KILL", "QUEST", "QUEST_DF", "EXPLORE")]
    if e.get("loot"):
        out += [f"AC_RATE_DROP_ITEM_{k}={e['loot']}" for k in ("POOR", "NORMAL", "UNCOMMON", "RARE", "EPIC")]
        out.append(f"AC_RATE_DROP_MONEY={e['loot']}")
    if e.get("rep"):
        out.append(f"AC_RATE_REPUTATION_GAIN={e['rep']}")
    if e.get("start") in ("level1", "boost") and cfg.get("profile") == "main":
        out.append("OV_MAIN=" + ("1" if e["start"] == "boost" else "0"))   # the boost menu (ov_main.lua)
    return out


def write_realm_env(cfg):
    with open(path(REALM_ENV), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(realm_env_lines(cfg)) + "\n")


def lua_str(v):
    """A Lua string literal ov_hostctl.lua reads as data only: quotes, backslashes and controls as \\ddd."""
    return '"' + "".join(c if (32 <= ord(c) < 127 and c not in '"\\') else "".join(f"\\{b:03d}" for b in c.encode())
                         for c in str(v)) + '"'


def write_host_dat(cfg):
    """realm-files/lua_scripts/host-realm.dat: this realm's facts for its GM's "Your realm" in game (ov_hostctl.lua):
    no secrets (a private realm's invite carries its secret, so only a public realm's join page is in it)."""
    d = os.path.join(KIT, "realm-files", "lua_scripts")
    if not os.path.isdir(d) or cfg.get("official") or cfg.get("linked"):
        return
    e = cfg.get("realm_env") or {}
    rates = "/".join(str(e.get(k) or "-") for k in ("xp", "loot", "rep"))
    fields = [("realm_id", lua_str(cfg["realm_id"])), ("visibility", lua_str(cfg["visibility"])),
              ("signup", lua_str(cfg.get("signup") or "invite")), ("bots", str(int(cfg.get("bots") or 0))),
              ("herokey", "true" if cfg.get("herokey") else "false"), ("rates", lua_str(rates))]
    if cfg["visibility"] == "public":
        fields.append(("join", lua_str(invite_link(cfg))))
    tmp = os.path.join(d, "host-realm.dat.tmp")
    try:
        with open(tmp, "w", encoding="ascii", newline="\n") as f:
            f.write("return { " + ", ".join(f"{k} = {v}" for k, v in fields) + " }\n")
        os.replace(tmp, os.path.join(d, "host-realm.dat"))
    except OSError:
        pass


def gen(n, alphabet=ALNUM):
    return "".join(secrets.choice(alphabet) for _ in range(n))


def path(*p):
    return os.path.join(STATE, *p)


def load():
    try:
        with open(path("host.json"), encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def save(cfg):
    os.makedirs(STATE, exist_ok=True)
    tmp = path("host.json.tmp")
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(cfg, f, indent=1, sort_keys=True)
    os.replace(tmp, path("host.json"))


def write_private(name, text):
    p = path(name)
    fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.write(text)


def invite_link(cfg):
    base = f"{cfg.get('join_base') or cfg['site']}/join/{cfg['realm_id']}"
    return base + (f"#{cfg['invite_secret']}" if cfg["visibility"] == "private" else "")


def host_link(cfg):
    """The host's own join link: the invite plus the host key, so the host (only) can make the GM
    account from the join page. Never shared."""
    return f"{cfg.get('join_base') or cfg['site']}/join/{cfg['realm_id']}#{cfg['invite_secret']}~{cfg['host_secret']}"


def write_links(cfg):
    with open(path("invite.txt"), "w", encoding="utf-8") as f:
        f.write(invite_link(cfg) + "\n")
    if cfg.get("host_secret"):
        write_private("host-link.txt", host_link(cfg) + "\n")
    write_den_invite(cfg)
    make_den_saves(cfg)


def make_den_saves(cfg):
    """state/dens: a Den's save files (ov_den.lua writes <name>.den.json there, the launcher's Saved Dens reads them
    and writes open.json). The world server mounts it (docker-compose.host.yml) and runs as its own user, so for a
    Den the folder is open to it (it holds only layout files; nothing in it is ever run). Made for every realm so
    Docker never makes it as root."""
    d = path("dens")
    try:
        os.makedirs(d, exist_ok=True)
        if cfg.get("profile") == "den" and os.name == "posix":
            os.chmod(d, 0o777)
    except OSError:
        pass


def write_den_invite(cfg):
    """A Den's in-game "Invite a friend" (ov_den.lua, shown to the realm's GM only): its invite link, next to the
    scripts as data only. Only for the den profile, whose owner asked for the link in game; other rulesets keep the
    private link out of the realm's files (write_host_dat), and leaving the den profile removes the file."""
    d = os.path.join(KIT, "realm-files", "lua_scripts")
    if not os.path.isdir(d) or cfg.get("official") or cfg.get("linked"):
        return
    f = os.path.join(d, "den-invite.dat")
    if cfg.get("profile") != "den":
        try:
            os.remove(f)
        except OSError:
            pass
        return
    tmp = f + ".tmp"
    try:
        fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w", encoding="ascii", newline="\n") as out:
            out.write("return { invite = " + lua_str(invite_link(cfg)) + " }\n")
        os.replace(tmp, f)
    except OSError:
        pass


def read_rules():
    """state/rules.txt: one rule per line; None when the file is not there."""
    try:
        with open(path("rules.txt"), encoding="utf-8-sig") as f:
            lines = [ln.strip() for ln in f]
    except OSError:
        return None
    return [ln[:120] for ln in lines if ln and not ln.startswith("#")][:10]


def read_beacon_edits():
    """The fields the host may edit in state/beacon.json (name and ruleset come from setup)."""
    try:
        with open(path("beacon.json"), encoding="utf-8-sig") as f:
            doc = json.load(f)
    except (OSError, ValueError):
        return {}
    return {k: doc[k] for k in EDITABLE if isinstance(doc, dict) and k in doc} if isinstance(doc, dict) else {}


def beacon_record(cfg):
    """What the listing will show (it rebuilds the rest itself); kept in state/beacon.json so the
    host can see and edit the text (tagline, summary, type, discord), and the rules in state/rules.txt."""
    rec = cfg.get("record") or {}
    rec.update(read_beacon_edits())
    rules = read_rules()
    if rules is not None:
        rec["rules"] = rules
    if not rec.get("tagline"):
        rec["tagline"] = DEFAULT_TAGLINE.get(cfg["profile"], "")
    rec.setdefault("tagline", "")
    rec.setdefault("summary", "")
    rec.setdefault("rules", [])
    rec.setdefault("discord", None)
    if cfg.get("linked"):   # a server the host already runs: its own core and client, no ruleset of ours
        rec.update(name=cfg["name"], type=rec.get("type", "PvE"), ruleset="custom", warden=None, signup="off")
        rec["mods"] = realm_mods(rec.get("mods"), builtin=False)
        return linked_fields(rec)
    rec.update(name=cfg["name"], type=rec.get("type", "PvE"), ruleset=cfg["profile"],
               warden=cfg["warden"], signup=cfg.get("signup") or "invite")
    rec["mods"] = realm_mods(rec.get("mods"), builtin=cfg.get("profile") not in cores.PROFILES)
    return listing_fields(rec, cfg)


def clean_mods(v):
    """[{"id", "version"?}], checked, each id once, at most MAX_MODS (a bad entry is dropped)."""
    out, seen = [], set()
    for m in (v if isinstance(v, list) else [])[:MAX_MODS * 2]:
        if isinstance(m, str):
            m = {"id": m}
        mid = m.get("id") if isinstance(m, dict) else None
        if not isinstance(mid, str) or not MOD_ID_RE.fullmatch(mid) or mid in seen:
            continue
        seen.add(mid)
        one = {"id": mid}
        if isinstance(m.get("version"), str) and MOD_VERSION_RE.fullmatch(m["version"]):
            one["version"] = m["version"]
        # the Network tab's mods editor: a short label, required or optional, a client mod or a server-side one
        name = " ".join(str(m.get("name") or "").split())[:40]
        if name and not BRAND_RE.search(name):
            one["name"] = name
        if isinstance(m.get("required"), bool):
            one["required"] = m["required"]
        if isinstance(m.get("client"), bool):
            one["client"] = m["client"]
        out.append(one)
        if len(out) >= MAX_MODS:
            break
    return out


def realm_mods(v, builtin):
    """The realm's list: the built-in Leyline Modes first (a realm with the Leyline realm scripts runs them), then the host's."""
    return clean_mods(([{"id": "mode:" + m} for m in BUILTIN_MODES] if builtin else []) + (v if isinstance(v, list) else []))


def parse_mod(arg):
    """ID or ID@VERSION from the command line, checked; exits with a plain message when it is not a mod id."""
    mid, _, ver = arg.strip().partition("@")
    if not MOD_ID_RE.fullmatch(mid):
        sys.exit(f"Not a mod id: {arg!r}. Use the id from the Workshop, like mod:dalaran-dice, up:skada or mode:minigolf.")
    if ver and not MOD_VERSION_RE.fullmatch(ver):
        sys.exit(f"Not a version: {ver!r}.")
    return {"id": mid, **({"version": ver} if ver else {})}


def cmd_mods(a):
    """The realm's mod list on the realm list (docs/GAME-MODS.md). add/remove change state/beacon.json too (the host's
    copy of the list text); the agent sends the list with its next heartbeat. Names only: nothing is downloaded here."""
    cfg = load()
    if not cfg:
        sys.exit("Run Host a Wrath realm first.")
    builtin = not cfg.get("linked") and cfg.get("profile") not in cores.PROFILES
    if a.action == "set":   # the whole list at once (the launcher's Network tab): JSON [{"id", "name", "required", "client"}]
        try:
            want = json.loads(a.json or "[]")
        except ValueError:
            sys.exit("mods set --json: a JSON list of {id, name, required, client}")
        if not isinstance(want, list) or len(want) > MAX_MODS:
            sys.exit(f"mods set --json: a list of at most {MAX_MODS} mods")
        for m in want:
            mid = m.get("id") if isinstance(m, dict) else m
            if not isinstance(mid, str) or not MOD_ID_RE.fullmatch(mid):
                sys.exit(f"Not a mod id: {mid!r}. Use the id from the Workshop, like mod:dalaran-dice or up:skada.")
        _set_mods([m for m in clean_mods(want) if not (builtin and m["id"].startswith("mode:") and m["id"][5:] in BUILTIN_MODES)])
    elif a.action in ("add", "remove"):
        if not a.ids:
            sys.exit(f"Usage: leyline mods {a.action} ID [ID ...]")
        _mods_add_remove(a, cfg, builtin)
    _mods_report(a)


def _set_mods(cur):
    def put(c):
        c.setdefault("record", {})["mods"] = cur
        try:
            with open(path("beacon.json"), encoding="utf-8-sig") as f:
                b = json.load(f)
        except (OSError, ValueError):
            b = None
        if isinstance(b, dict):   # beacon.json is the host's copy of the text: keep it in step
            b["mods"] = cur
            with open(path("beacon.json"), "w", encoding="utf-8") as f:
                json.dump(b, f, indent=1)
    _edit(put)


def _mods_add_remove(a, cfg, builtin):
    want = [parse_mod(x) for x in a.ids]
    cur = [m for m in clean_mods((cfg.get("record") or {}).get("mods")) if not (builtin and m["id"][5:] in BUILTIN_MODES
                                                                                and m["id"].startswith("mode:"))]
    if a.action == "add":
        ids = {m["id"] for m in want}
        cur = [m for m in cur if m["id"] not in ids] + want
        if len(cur) + (len(BUILTIN_MODES) if builtin else 0) > MAX_MODS:
            sys.exit(f"A realm lists at most {MAX_MODS} mods.")
    else:
        gone = {m["id"] for m in want}
        if builtin and any(i.startswith("mode:") and i[5:] in BUILTIN_MODES for i in gone):
            sys.exit("The Leyline Modes come with this realm's scripts, so they stay on its list.")
        cur = [m for m in cur if m["id"] not in gone]
    _set_mods(cur)


def _mods_report(a):
    rec = (load() or {}).get("record") or {}
    mods = rec.get("mods") or []
    sys.stderr.write((f"This realm lists {len(mods)} mod{'' if len(mods) == 1 else 's'}:\n" if mods else "This realm lists no mods.\n")
                     + "".join(f"  {m['id']}{' ' + m['version'] if m.get('version') else ''}\n" for m in mods)
                     + ("The realm list shows changes within a minute. Players' launchers offer the ones in the Workshop "
                        "when they join.\n" if a.action != "list" else ""))


def listing_fields(rec, cfg):
    """The realm list's filter fields, checked: tags, region, language, max players, direct (off unless the host
    turns it on), and the core and build the kit runs."""
    tags = rec.get("tags") if isinstance(rec.get("tags"), list) else list(PROFILE_TAGS.get(cfg.get("profile"), []))
    tags = {t.strip().lower() for t in tags if isinstance(t, str)}
    tags.discard("bots")
    if (cfg.get("bots") or 0) > 0:
        tags.add("bots")
    rec["tags"] = [t for t in TAGS if t in tags]
    rec["region"] = rec.get("region") if rec.get("region") in REGIONS else None
    lang = rec.get("language")
    rec["language"] = lang if isinstance(lang, str) and LANG_RE.match(lang) else None
    mp = rec.get("max_players", PROFILE_MAX.get(cfg.get("profile")))
    rec["max_players"] = mp if isinstance(mp, int) and not isinstance(mp, bool) and 1 <= mp <= 10000 else None
    rec["direct"] = rec.get("direct") is True
    core, build = PROFILE_CORE.get(cfg.get("profile"), (CORE, BUILD))[:2]
    rec.update(core=core, build=build)
    return rec


def linked_fields(rec):
    """listing_fields for a linked server: the same checks, but the core and client build are the host's."""
    tags = rec.get("tags") if isinstance(rec.get("tags"), list) else []
    tags = {t.strip().lower() for t in tags if isinstance(t, str)}
    rec["tags"] = [t for t in TAGS if t in tags]
    rec["region"] = rec.get("region") if rec.get("region") in REGIONS else None
    lang = rec.get("language")
    rec["language"] = lang if isinstance(lang, str) and LANG_RE.match(lang) else None
    mp = rec.get("max_players")
    rec["max_players"] = mp if isinstance(mp, int) and not isinstance(mp, bool) and 1 <= mp <= 10000 else None
    rec["direct"] = False
    rec["core"] = rec.get("core") if rec.get("core") in LINK_CORES else "other"
    rec["build"] = rec.get("build") if rec.get("build") in link_builds() else BUILD
    return rec


def csv_tags(v):
    if v is None:
        return None
    want = [t.strip().lower() for t in v.split(",") if t.strip()]
    bad = [t for t in want if t not in TAGS]
    if bad:
        sys.exit("Tags: " + ", ".join(TAGS) + f" (not {', '.join(bad)})")
    return want


def write_env(cfg, image, tag, tools_image):
    env = {"WU_IMAGE": image, "WU_TAG": tag, "DB_ROOT_PASSWORD": cfg["db_password"], "WU_PROFILE": cfg["profile"],
           "WU_WARDEN": "1" if cfg["warden"] else "0", "WU_BOTS": str(cfg["bots"]), "WU_TOOLS_IMAGE": tools_image,
           "COMPOSE_PROJECT_NAME": "wu-host"}
    for k in ("WU_UID", "WU_GID"):   # Linux/macOS: the agent writes state/ as the host's own user
        if os.environ.get(k, "").isdigit():
            env[k] = os.environ[k]
    keep = {}
    try:
        with open(os.path.join(KIT, ".env"), encoding="utf-8") as f:
            for ln in f:
                if "=" in ln and not ln.startswith("#"):
                    k, v = ln.rstrip("\r\n").split("=", 1)
                    keep[k] = v
    except OSError:
        pass
    for prof_env in PROFILE_ENV.values():   # another profile's settings go when the profile changes
        for k, v in prof_env.items():
            if keep.get(k) == v and k not in PROFILE_ENV.get(cfg["profile"], {}):
                del keep[k]
    keep.update(PROFILE_ENV.get(cfg["profile"], {}))
    keep.update(env)
    player_limit_env(keep, cfg)
    fd = os.open(os.path.join(KIT, ".env"), os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as f:
        f.write("# Written by Host a Wrath realm. Keep it private: it holds the database password.\n")
        for k in sorted(keep):
            f.write(f"{k}={keep[k]}\n")


def player_limit_env(env, cfg):
    """WU_PLAYER_LIMIT (the world server's PlayerLimit: docker-compose.host.yml AC_PLAYER_LIMIT, the CMaNGOS and
    TrinityCore profiles' run-server.sh) from the host's max players. Without one: the ruleset's own (PROFILE_ENV),
    else the compose file's default; a value the host typed into .env by hand is kept."""
    lim = cfg.get("player_limit")
    ours = cfg.get("player_limit_env")
    if isinstance(lim, int) and not isinstance(lim, bool) and 1 <= lim <= 10000:
        env["WU_PLAYER_LIMIT"] = cfg["player_limit_env"] = str(lim)
        return
    if ours and env.get("WU_PLAYER_LIMIT") == ours:
        env.pop("WU_PLAYER_LIMIT")
        env.update({k: v for k, v in PROFILE_ENV.get(cfg.get("profile"), {}).items() if k == "WU_PLAYER_LIMIT"})
    cfg.pop("player_limit_env", None)


def cmd_init(a):
    cfg = load() or {}
    if cfg.get("official"):
        sys.exit("This folder runs the operator's own realm (setup.py official), not a host kit.")
    if cfg.get("linked"):
        sys.exit(LINKED)
    seed = read_seed(a.seed) if getattr(a, "seed", None) else None
    if seed:
        if seed["v"] == 1 and cfg.get("realm_id") and cfg["realm_id"] != seed["realm_id"]:
            sys.exit(ANOTHER.replace("state/realm.key", "state/host.json"))
        apply_seed(seed, a, cfg)
    name = a.name if a.name is not None else cfg.get("name")
    if not name or not NAME_RE.match(name.strip()):
        sys.exit("Realm name: 1-32 letters, digits, spaces, _ ' or -")
    if name_refused(name):
        sys.exit(NAME_REFUSED)
    profile = a.profile or cfg.get("profile", "main")
    if profile not in PROFILES:
        sys.exit("Profile must be " + ", ".join(sorted(k for k in PROFILES if cores.offered(k))))
    if not cores.offered(profile):   # 4.3.4: not offered for now (cores.MAX_OFFERED_BUILD)
        sys.exit(cores.not_offered(profile))
    vis = a.visibility or cfg.get("visibility", "private")
    if vis not in ("public", "private"):
        sys.exit("Choose private or public")
    os.makedirs(STATE, exist_ok=True)
    secret = keys.load_or_create(path("realm.key"))
    rid = keys.realm_id(keys.public_key(secret))
    first = not cfg
    relay = a.relay or cfg.get("relay", domains.RELAY)
    cfg.update(
        realm_id=rid, name=name.strip(), profile=profile, visibility=vis,
        relay=relay, site=(a.site or cfg.get("site", domains.SITE)).rstrip("/"),
        # Join pages are served by the relay itself (https://<relay>/join/<id>): no website needed.
        join_base=(a.join_base or (f"https://{a.relay}" if a.relay else None) or cfg.get("join_base")
                   or (domains.JOIN if relay == domains.RELAY else f"https://{relay}")).rstrip("/"),
        relay_url=a.relay_url or f"wss://{relay}/agent",
        listing_url=a.listing_url or f"https://{relay}/api/listing/heartbeat",
        auth_service=PROFILE_CORE.get(profile, (0, 0, "ac-authserver:3724"))[2],
        world_service=PROFILE_CORE.get(profile, (0, 0, 0, "ac-worldserver:8085"))[3],
        warden=warden_for(a, cfg, profile, vis),
        bots=(a.bots if a.bots is not None
              else cfg["bots"] if cfg.get("profile") == profile and "bots" in cfg
              else PROFILE_BOTS.get(profile, 40)),
        # A listed realm is for strangers, so its join page makes accounts ("open"); a friends' realm
        # only from the invite link. An explicit --signup (or an earlier choice) wins.
        signup=a.signup or cfg.get("signup") or ("open" if vis == "public" else "invite"),
    )
    cfg.setdefault("invite_secret", gen(20, B32))
    cfg.setdefault("host_secret", gen(24, B32))   # only in the host's own link (state/host-link.txt)
    cfg.setdefault("db_password", gen(24))
    if a.gm_name is not None:
        if not GM_RE.match(a.gm_name):
            sys.exit("GM account name: 3-16 letters or digits")
        cfg["gm_name"] = a.gm_name.upper()
    if a.tagline is not None:
        cfg.setdefault("record", {})["tagline"] = " ".join(a.tagline.split())[:80]
        b = read_beacon_edits()
        if b:   # beacon.json is the host's copy of the text: keep it in step
            b_full = json.load(open(path("beacon.json"), encoding="utf-8-sig"))
            b_full["tagline"] = cfg["record"]["tagline"]
            with open(path("beacon.json"), "w", encoding="utf-8") as f:
                json.dump(b_full, f, indent=1)
    listing_args(a, cfg)
    if "gm" not in cfg and not cfg.get("gm_name"):
        # Legacy kit flow (Host a Wrath realm.bat): a generated GM login. The leyline command never
        # does this: it asks for the operator's account name and they make the account themselves.
        user, pw = "HOST" + gen(4), gen(12)
        salt, ver = srp6.make_verifier(user, pw)
        cfg["gm"] = {"username": user, "salt": salt.hex(), "verifier": ver.hex()}
        write_private("host-login.txt", f"Your own game login on your realm (a GM account):\n"
                                        f"  account:  {user}\n  password: {pw}\n")
    if not os.path.exists(path("rules.txt")):
        with open(path("rules.txt"), "w", encoding="utf-8") as f:
            f.write(RULES_HELP + "".join(r + "\n" for r in DEFAULT_RULES.get(profile, [])))
    cfg["record"] = beacon_record(cfg)
    save(cfg)
    write_beacon(cfg)
    write_env(cfg, a.image, a.tag, a.tools_image)
    save(cfg)   # write_env notes the player cap it wrote
    write_realm_env(cfg)
    write_links(cfg)
    write_host_dat(cfg)
    if seed:   # used: the key and links live in state/ now
        try:
            os.remove(seed["_path"])
        except OSError:
            pass
    print(json.dumps({"realm_id": rid, "first_run": first, "invite": invite_link(cfg)}))


def browser_ok(profile):
    """The Leyline browser client is a 3.3.5a client without a Warden module: it can join the AzerothCore rulesets
    once Warden is off. The other cores run other clients (cores.PROFILES), so the switch does not apply there."""
    return profile not in cores.PROFILES


def warden_for(a, cfg, profile, vis):
    """Warden (the core's client anti-cheat) from the "Browser players" switch (cfg["browser"]). An explicit --warden
    or --browser wins and is kept; else an earlier choice; else, on the AzerothCore rulesets, browser players ON
    (Warden off) for every new realm, public or private: the operator's own public Hangout runs that way, the core's
    server-side checks (movement, spells, loot, trades) all stay on, and the browser client cannot join otherwise.
    A host who wants stock clients only turns it off (leyline browser off). Other cores: Warden on when public."""
    if a.warden is not None:
        cfg["browser"] = a.warden == "off"
    elif getattr(a, "browser", None) is not None:
        cfg["browser"] = a.browser == "on"
    if not browser_ok(profile):
        cfg.pop("browser", None)
        return (vis == "public") if a.warden is None else a.warden == "on"
    cfg.setdefault("browser", True)
    return not cfg["browser"]


def listing_args(a, cfg):
    """--tags --region --language --max-players --direct: into the record and the host's beacon.json."""
    ch = {}
    if getattr(a, "tags", None) is not None:
        ch["tags"] = csv_tags(a.tags)
    if getattr(a, "region", None) is not None:
        if a.region not in REGIONS + ("none",):
            sys.exit("Region: " + ", ".join(REGIONS) + " or none")
        ch["region"] = None if a.region == "none" else a.region
    if getattr(a, "language", None) is not None:
        if a.language != "none" and not LANG_RE.match(a.language):
            sys.exit("Language: a code like en, de or pt-BR (or none)")
        ch["language"] = None if a.language == "none" else a.language
    if getattr(a, "max_players", None) is not None:
        if not 0 <= a.max_players <= 10000:
            sys.exit("Max players: 1-10000 (0 = no limit)")
        ch["max_players"] = a.max_players or None
        cfg["player_limit"] = a.max_players or None   # the server's own cap too (write_env: WU_PLAYER_LIMIT)
    if getattr(a, "direct", None) is not None:
        ch["direct"] = a.direct == "on"
    if not ch:
        return
    cfg.setdefault("record", {}).update(ch)
    try:
        with open(path("beacon.json"), encoding="utf-8-sig") as f:
            b_full = json.load(f)
    except (OSError, ValueError):
        return
    if isinstance(b_full, dict):   # beacon.json is the host's copy of the text: keep it in step
        b_full.update(ch)
        with open(path("beacon.json"), "w", encoding="utf-8") as f:
            json.dump(b_full, f, indent=1)


def cmd_listing(a):
    """Change the realm list details (tags, region, language, max players, direct); sent with the next heartbeat.
    Max players is the world server's cap too (.env WU_PLAYER_LIMIT, from its next start)."""
    _edit(lambda c: listing_args(a, c))
    if getattr(a, "max_players", None) is not None:
        sync_env(lambda env, c: player_limit_env(env, c))
        sys.stderr.write("Saved. The realm list shows it within a minute; the server's cap applies at the realm's next "
                         "start (leyline stop, then leyline host).\n")


def sync_env(fn):
    """Change the kit's .env (0600, the rest kept) with fn(env, cfg); host.json saved after (fn may note things)."""
    p = os.path.join(KIT, ".env")
    try:
        with open(p, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except OSError:
        return
    head = [ln for ln in lines if ln.startswith("#") or "=" not in ln]
    env = dict(ln.split("=", 1) for ln in lines if "=" in ln and not ln.startswith("#"))
    cfg = load() or {}
    fn(env, cfg)
    save(cfg)
    fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as f:
        f.write("".join(ln + "\n" for ln in head) + "".join(f"{k}={env[k]}\n" for k in sorted(env)))


def cmd_browser(a):
    """The "Browser players" switch: on = Warden off (the browser client and stock clients join), off = Warden on
    (stock clients only). The world server reads WU_WARDEN at its next start; the realm list shows it at once."""
    cfg = load()
    if not cfg:
        sys.exit("Run Host a Wrath realm first.")
    if cfg.get("linked") or cfg.get("official"):
        sys.exit("A linked server keeps its own anti-cheat settings: change Warden in your server's own config.")
    if not browser_ok(cfg.get("profile")):
        sys.exit("Browser players need an AzerothCore ruleset: the browser client is a 3.3.5a client.")
    _edit(lambda c: c.update(browser=a.state == "on", warden=a.state == "off"))
    set_env_value("WU_WARDEN", "0" if a.state == "on" else "1")
    sys.stderr.write(("Browser players on: Warden is off, so the Leyline browser client can join. "
                      if a.state == "on" else "Browser players off: Warden is on, stock clients only. ")
                     + "Restart your realm (leyline stop, then leyline host) for the server to pick it up.\n")


def add_listing_args(p):
    p.add_argument("--tags", help="comma list: " + ",".join(TAGS))
    p.add_argument("--region", help="where the realm runs: " + ", ".join(REGIONS) + " or none")
    p.add_argument("--language", help="the realm's language: en, de, pt-BR, ... or none")
    p.add_argument("--max-players", type=int, help="the player limit: the world server's cap (AzerothCore, CMaNGOS, "
                   "TrinityCore) and the realm list's (0 = the ruleset's own, not shown)")
    p.add_argument("--direct", choices=["on", "off"], help="allow direct connections (off by default; the network "
                   "only lists it when its operator turns direct connections on)")


def cmd_official(a):
    """The operator's own realm as a Beacon (hosting/hangout/hangout-up.sh runs this on the realm's box): a
    public Leyline record, its key made here, signup off (that realm makes its accounts itself, from the website's
    sign-in), and "official" so the agent never touches its realm list, accounts or GM rights. Safe to run again:
    the key and realm id stay; the text and addresses follow the arguments."""
    cfg = load() or {}
    if cfg and not cfg.get("official"):
        sys.exit("This folder holds a host kit's realm, not the operator's own realm: use another folder.")
    if not NAME_RE.match(a.name.strip()) or BRAND_RE.search(a.name):
        sys.exit("Realm name: 1-32 letters, digits, spaces, _ ' or -, no game or publisher brand names")
    for k in ("relay_url", "listing_url"):
        v = getattr(a, k)
        if v and not re.match(r"^(wss?|https?)://[A-Za-z0-9.:\-\[\]]+(/[A-Za-z0-9/._-]*)?$", v):
            sys.exit(f"--{k.replace('_', '-')}: a plain URL")
    os.makedirs(STATE, exist_ok=True)
    secret = keys.load_or_create(path("realm.key"))
    rid = keys.realm_id(keys.public_key(secret))
    relay = a.relay or cfg.get("relay", domains.RELAY)
    rules = []
    if a.rules_file:
        with open(a.rules_file, encoding="utf-8-sig") as f:
            rules = [" ".join(ln.split())[:120] for ln in f if ln.strip() and not ln.lstrip().startswith("#")][:10]
    text = " ".join([a.name, a.tagline or "", a.summary or ""] + rules)
    if BRAND_RE.search(text):
        sys.exit("Leave game and publisher brand names out of the realm list text.")
    # The operator's other official realms (hosting/build-realm: the Builders' Realm) say what they are with
    # --ruleset / --tags; the defaults are the official realm's own (the hangout step passes neither).
    ruleset = a.ruleset or "dalaran"
    tags = [t.strip().lower() for t in (a.tags or "hangout,boosted").split(",") if t.strip()]
    if ruleset not in ("dalaran", "den", "custom") or not tags or any(t not in TAGS for t in tags):
        sys.exit(f"--ruleset: dalaran, den or custom; --tags: some of {', '.join(TAGS)}")
    cfg.update(realm_id=rid, official=True, name=a.name.strip(), profile=ruleset, visibility="public", signup="off",
               relay=relay, relay_url=a.relay_url or f"wss://{relay}/agent",
               listing_url=a.listing_url or f"https://{relay}/api/listing/heartbeat",
               join_base=(a.join_base or f"https://{relay}").rstrip("/"), site=(a.site or domains.SITE).rstrip("/"),
               auth_service=a.auth_service, world_service=a.world_service, realmlist="keep",
               status_url=a.status_url or None, warden=False, bots=0)
    for k in ("invite_secret", "host_secret", "gm", "gm_name", "db_password", "claim"):
        cfg.pop(k, None)   # nothing to invite to, no GM account to make, no database of its own
    cfg["record"] = {"name": cfg["name"], "tagline": " ".join((a.tagline or "").split())[:80],
                     "summary": " ".join((a.summary or "").split())[:400], "type": "PvE", "ruleset": ruleset,
                     "rules": rules, "discord": a.discord or None, "warden": None, "signup": "off",
                     "tags": tags, "max_players": a.max_players or None, "direct": False}
    cfg["record"] = listing_fields(cfg["record"], cfg)
    save(cfg)
    print(json.dumps({"realm_id": rid, "name": cfg["name"]}))


# ------------------------------------------------------------------------------------------- link
# A server the host ALREADY runs (any core, on this PC, a VPS or their LAN) on the Leyline realm list. Only the agent
# runs (`leyline link`, "Link my server.bat"): one outbound tunnel to the relay, the signed heartbeat, and the
# relay's players passed to the server's own login server. Hands off like the operator's own realm: never its
# realm list, accounts or GM rights (no database login at all).
LINKED = ("This kit folder links a server you already run (leyline link). Use a fresh copy of the host kit to "
          "host a new realm, or: leyline link remove")
LINK_CORES = ("azerothcore", "trinitycore", "cmangos", "vmangos", "mangos", "other")
LINK_TYPES = {"normal": "PvE", "pvp": "PvP", "rp": "RP", "rppvp": "RP-PvP"}
HP_RE = re.compile(r"^(?:[A-Za-z0-9](?:[A-Za-z0-9.-]{0,251}[A-Za-z0-9])?|\[[0-9A-Fa-f:]{2,45}\]):\d{1,5}$")
HTTP_URL_RE = re.compile(r"^https?://[A-Za-z0-9.:\-\[\]]+(/[A-Za-z0-9/._~%?=&-]*)?$")
DISCORD_RE = re.compile(r"^https://(discord\.gg|discord\.com/invite)/[A-Za-z0-9-]{2,32}$")


def link_builds():
    """The client builds a linked server may say it runs: the ones Leyline offers now (cores.MAX_OFFERED_BUILD)."""
    return tuple(b for b in sorted(cores.BUILDS) if b <= cores.MAX_OFFERED_BUILD or cores.all_clients())


def host_port(v, what):
    if not isinstance(v, str) or not HP_RE.match(v.strip()) or not 0 < int(v.strip().rsplit(":", 1)[1]) < 65536:
        sys.exit(f"{what}: a host and port, like 127.0.0.1:3724 or my-server.example:3724")
    return v.strip()


def cmd_link(a):
    """Put a server the host already runs on the Leyline realm list. Safe to run again: the key and realm id stay;
    the text and addresses follow the arguments (an argument not passed keeps its earlier value)."""
    cfg = load() or {}
    if cfg and not cfg.get("linked"):
        sys.exit("This kit folder already hosts a realm (state/host.json). Use a fresh copy of the host kit to link "
                 "a server you already run.")
    old = cfg.get("record") or {}
    name = (a.name if a.name is not None else cfg.get("name") or "").strip()
    if not NAME_RE.match(name) or BRAND_RE.search(name):
        sys.exit("Realm name: 1-32 letters, digits, spaces, _ ' or -, no game or publisher brand names")
    if name_refused(name):
        sys.exit(NAME_REFUSED)
    auth = host_port(a.auth_service or cfg.get("auth_service") or "", "Login server address")
    world = host_port(a.world_service or cfg.get("world_service") or "", "World server address")
    build = a.build if a.build is not None else old.get("build", BUILD)
    if build not in link_builds():
        sys.exit("Client: " + ", ".join(f"{b} ({cores.BUILDS[b]})" for b in link_builds()))
    core = a.core or old.get("core") or ("azerothcore" if build == BUILD else "other")
    if core not in LINK_CORES:
        sys.exit("Core: " + ", ".join(LINK_CORES))
    rtype = LINK_TYPES[a.type] if a.type else old.get("type", "PvE")
    status_url = a.status_url if a.status_url is not None else cfg.get("status_url")
    if status_url and not HTTP_URL_RE.match(status_url):
        sys.exit("--status-url: a plain http(s) URL (players and bots as JSON), or leave it out")
    discord = a.discord if a.discord is not None else old.get("discord")
    if discord and not DISCORD_RE.match(discord):
        sys.exit("--discord: an invite link like https://discord.gg/abc123")
    rules = old.get("rules") or []
    if a.rules_file:
        with open(a.rules_file, encoding="utf-8-sig") as f:
            rules = [" ".join(ln.split())[:120] for ln in f if ln.strip() and not ln.lstrip().startswith("#")][:10]
    tagline = " ".join((a.tagline if a.tagline is not None else old.get("tagline", "")).split())[:80]
    summary = " ".join((a.summary if a.summary is not None else old.get("summary", "")).split())[:400]
    if BRAND_RE.search(" ".join([name, tagline, summary] + rules)):
        sys.exit("Leave game and publisher brand names out of the realm list text.")
    os.makedirs(STATE, exist_ok=True)
    secret = keys.load_or_create(path("realm.key"))
    rid = keys.realm_id(keys.public_key(secret))
    relay = a.relay or cfg.get("relay", domains.RELAY)
    cfg.update(realm_id=rid, linked=True, name=name, profile="linked", visibility="public", signup="off",
               relay=relay, relay_url=a.relay_url or cfg.get("relay_url") or f"wss://{relay}/agent",
               listing_url=a.listing_url or cfg.get("listing_url") or f"https://{relay}/api/listing/heartbeat",
               join_base=(cfg.get("join_base") or (domains.JOIN if relay == domains.RELAY else f"https://{relay}")).rstrip("/"),
               site=(cfg.get("site") or domains.SITE).rstrip("/"),
               auth_service=auth, world_service=world, realmlist="keep", status_url=status_url or None,
               warden=None, bots=0)
    for k in ("invite_secret", "host_secret", "gm", "gm_name", "db_password", "official"):
        cfg.pop(k, None)   # nothing to invite to, no GM account to make, no database login
    rec = dict(old, name=name, tagline=tagline, summary=summary, type=rtype, ruleset="custom", rules=rules,
               discord=discord or None, warden=None, core=core, build=build, signup="off")
    if a.tags is not None:
        rec["tags"] = csv_tags(a.tags)
    listing_args(argparse.Namespace(region=a.region, language=a.language), {"record": rec})
    cfg["record"] = linked_fields(rec)
    save(cfg)
    write_beacon(cfg)
    if not os.path.exists(path("rules.txt")) or a.rules_file:
        with open(path("rules.txt"), "w", encoding="utf-8") as f:
            f.write(RULES_HELP + "".join(r + "\n" for r in rules))
    print(json.dumps({"realm_id": rid, "name": name, "page": f"https://play.{domains.DOMAIN}/wrath/r/{rid}"}))


BRAND_RE = re.compile(r"(?i)world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard")
NAME_REFUSED = ("That realm name is not allowed on the Leyline list (slurs, rude words and names that pass for the "
                "Leyline team are refused): pick another.")


def name_refused(name) -> bool:
    """What the relay's realm list refuses in a name (wunet/namecheck.py realm_name_hit), checked here first."""
    try:
        return bool(namecheck and namecheck.realm_name_hit(str(name or "")))
    except Exception:   # noqa: BLE001: the relay checks again
        return False


def write_beacon(cfg):
    with open(path("beacon.json"), "w", encoding="utf-8") as f:
        json.dump(cfg["record"], f, indent=1)


def cmd_show(a):
    cfg = load()
    if not cfg:
        sys.exit("This PC is not hosting a realm yet: run Host a Wrath realm first.")
    rs = {}
    try:
        with open(path("relay-state.json"), encoding="utf-8") as f:
            rs = json.load(f)
    except (OSError, ValueError):
        pass
    print(f"Realm:      {cfg['name']}  ({cfg['profile']}, {cfg['visibility']})")
    print(f"Realm id:   {cfg['realm_id']}")
    print(f"Invite:     {invite_link(cfg)}")
    rules = (cfg.get("record") or {}).get("rules") or []
    print(f"Rules:      {len(rules)} line(s), from state/rules.txt" + (f" (first: {rules[0]})" if rules else ""))
    if cfg["visibility"] == "private":
        print("            Only people with this link can join. Keep the part after # private.")
    else:
        print("            Public: listed in Beacons (unclaimed until you claim it with your Hero Key).")
    if rs.get("connected"):
        print(f"Relay:      connected. Players' realmlist: {realmlist_line(cfg, rs)}")
    else:
        print(f"Relay:      not connected yet{(' (' + rs['last_error'] + ')') if rs.get('last_error') else ''}")
    if cfg.get("gm_name"):
        ok = rs.get("gm_ok") and rs.get("gm_account") == cfg["gm_name"]
        print(f"GM account: {cfg['gm_name']}  " + (
            "(has GM rights on this realm)" if ok else
            "(not made yet: open your own link below and make this account with your own password; "
            "it gets GM rights by itself)"))
        if cfg.get("host_secret"):
            print(f"Your link:  {host_link(cfg)}")
            print("            Yours only (it can make the GM account). Friends get the invite link above.")
        return
    try:
        with open(path("host-login.txt"), encoding="utf-8") as f:
            print(f.read().rstrip())
    except OSError:
        pass


def read_relay_state():
    try:
        with open(path("relay-state.json"), encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def realmlist_line(cfg, rs):
    """What players type. The shared port 3724 works only where the relay runs it (off on a box whose
    own authserver has 3724); otherwise the realm's own relay port."""
    if rs.get("shared_auth_port") == 3724:
        return f"set realmlist {cfg['relay']}   (or {cfg['relay']}:{rs.get('auth_port')})"
    if rs.get("auth_port"):
        return f"set realmlist {cfg['relay']}:{rs['auth_port']}"
    return "(shown once the realm is connected to the relay)"


def cmd_realmlist(a):
    cfg = load()
    if cfg:
        print(realmlist_line(cfg, read_relay_state()).split("   ")[0])


def cmd_gm(a):
    """Name the account that gets GM rights. The operator makes that account; nothing is created here."""
    if not GM_RE.match(a.name):
        sys.exit("GM account name: 3-16 letters or digits")
    _edit(lambda c: c.update(gm_name=a.name.upper()))


def cmd_get(a):
    """Earlier answers as KEY=VALUE lines (for the .bat's defaults); nothing when not set up yet."""
    cfg = load() or {}
    for k, var in (("name", "WU_OLD_NAME"), ("profile", "WU_OLD_PROFILE"), ("visibility", "WU_OLD_VIS")):
        if cfg.get(k):
            print(f"{var}={cfg[k]}")


def cmd_friend(a):
    """SQL that makes a friend's account; the .bat pipes it into the database container."""
    if (load() or {}).get("profile") in PROFILE_CORE:
        sys.exit("On this realm friends make their own account on your join page (send them your invite link).")
    user = a.name.upper()
    if not re.match(r"^[A-Z0-9]{3,16}$", user):
        sys.exit("Account names: 3-16 letters or digits")
    pw = (a.password or gen(10)).upper()
    salt, ver = srp6.make_verifier(user, pw)
    sys.stderr.write(f"Account for your friend:  {user}  password: {pw}\n")
    print(f"INSERT INTO account (username, salt, verifier, expansion) VALUES ('{user}', 0x{salt.hex()}, 0x{ver.hex()}, 2);")


def _edit(fn):
    cfg = load()
    if not cfg:
        sys.exit("Run Host a Wrath realm first.")
    fn(cfg)
    cfg["record"] = beacon_record(cfg)
    save(cfg)
    write_beacon(cfg)
    write_links(cfg)
    write_realm_env(cfg)
    write_host_dat(cfg)
    print(invite_link(cfg))


def cmd_new_invite(a):
    _edit(lambda c: c.update(invite_secret=gen(20, B32)))


def cmd_visibility(a):
    _edit(lambda c: c.update(visibility=a.visibility))


def cmd_claim(a):
    if not re.match(r"^[A-Za-z0-9_-]{16,64}$", a.token):
        sys.exit("That doesn't look like a claim token (copy it from the website).")
    _edit(lambda c: c.update(claim=a.token))


def cmd_signup(a):
    _edit(lambda c: c.update(signup=a.mode))


def cmd_herokey(a):
    """Hero Key sign-in (docs/HERO-KEY-SSO.md): players signed in on the Leyline website get an account here
    without the join page. Off by default; it only works once the website's key is pinned (the kit's own,
    wunet/herokey.py SITE_KEYS, or `herokey pin`, which fetches it once from the website over HTTPS)."""
    from wunet import herokey
    if a.state == "pin":
        kids = pin_site_key((load() or {}).get("site") or domains.SITE)
        sys.stderr.write(("Pinned the website's Hero Key sign-in key: " + ", ".join(kids) + "\n") if kids else
                         "The website does not offer Hero Key sign-in yet: nothing pinned. Try again once it does.\n")
        return
    if a.state == "status":
        cfg = load() or {}
        pinned = herokey.pinned_keys(state_dir=STATE)
        print(json.dumps({"on": cfg.get("herokey") is True, "pinned": sorted(pinned),
                          "database": bool(cfg.get("linked") is not True or os.path.exists(path(LINK_DB_ENV)))}))
        return
    _edit(lambda c: c.update(herokey=a.state == "on"))
    if a.state == "on" and not herokey.pinned_keys(state_dir=STATE):
        sys.stderr.write("Saved. Hero Key sign-in starts once this kit knows the website's key: `herokey pin` (once the "
                         "website has it on), or a kit update.\n")


def pin_site_key(site, get=None):
    """Fetch the website's public Hero Key key(s) once over HTTPS and pin them in state/herokey-site-keys.json.
    Returns the key ids, or [] when the website does not offer Hero Key sign-in (its herokey.json is 404)."""
    from wunet import herokey
    import urllib.error
    import urllib.request
    url = site.rstrip("/") + "/wrath/leyline/herokey.json"
    if not url.startswith("https://") and not os.environ.get("WU_ALLOW_HTTP_SITE"):
        sys.exit("The website address must be https://")

    def _get(u):
        req = urllib.request.Request(u, headers={"User-Agent": "wu-host-setup/1", "Accept": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=10) as r:
                return json.loads(r.read(65536).decode("utf-8"))
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return None
            raise
    doc = (get or _get)(url)
    if not isinstance(doc, dict):
        return []
    pubs = [k.get("pub") for k in doc.get("keys") or [] if isinstance(k, dict) and k.get("use") in ("current", "next")]
    os.makedirs(STATE, exist_ok=True)
    try:
        return herokey.pin_state_keys(STATE, pubs, source=url)
    except ValueError:
        return []


def cmd_share_names(a):
    """The realm list shows who is online (character names, at most 50; never GM or bot characters). Off by default."""
    _edit(lambda c: c.update(share_names=a.state == "on"))
    sys.stderr.write("Saved. The realm list shows who is online within a minute.\n" if a.state == "on" else
                     "Saved. The realm list shows only how many are online.\n")


LINK_DB_ENV = "link-db.env"
JOIN_CORES = {"azerothcore": ("acore_auth", "acore_characters"), "trinitycore": ("auth", "characters")}
NOT_YET_CORES = ("cmangos", "vmangos", "mangos", "other")
DB_NAME_RE = re.compile(r"^[A-Za-z0-9_$]{1,64}$")
DB_USER_RE = re.compile(r"^[A-Za-z0-9_.@-]{1,80}$")


def db_env_lines(core, host, port, user, password, auth, chars, realm_db_id=1):
    """state/link-db.env for the linked agent (docker run --env-file): its login database, for Hero Key accounts."""
    for v in (host, user, password, auth, chars):
        if "\n" in str(v) or "\r" in str(v):
            sys.exit("Database settings can't hold line breaks.")
    return "".join(f"{k}={v}\n" for k, v in (
        ("DB_HOST", host), ("DB_PORT", port), ("DB_USER", user), ("DB_PASSWORD", password), ("WU_CORE", core),
        ("WU_DB_AUTH", auth), ("WU_DB_CHARS", chars), ("WU_EXPANSION", 2), ("WU_REALM_DB_ID", realm_db_id)))


def cmd_join_network(a):
    """An existing AzerothCore or TrinityCore 3.3.5a server joins the Leyline network with Hero Key sign-in
    (docs/leyline/join-network.md): link it (cmd_link), give the agent the login database for Hero Key accounts,
    check the database and the server's client, pin the website's key, and switch Hero Key sign-in on."""
    core = (a.core or "").lower()
    if core in NOT_YET_CORES:
        sys.exit(f"{core}: not yet. Hero Key sign-in for existing servers is for AzerothCore and TrinityCore servers "
                 "of the Wrath client for now. You can still put your server on the realm list with: link")
    if core not in JOIN_CORES:
        sys.exit("--core azerothcore or --core trinitycore")
    password = os.environ.get("WU_JOIN_DB_PASSWORD", "")
    if a.db_password_file:
        with open(a.db_password_file, encoding="utf-8") as f:
            password = f.read().strip()
    if not a.db_host or not HP_RE.match(f"{a.db_host}:{a.db_port}") or not 0 < a.db_port < 65536:
        sys.exit("--db-host: the login database's host name or address (and --db-port, default 3306)")
    user = a.db_user or "root"
    auth = a.auth_db or JOIN_CORES[core][0]
    chars = a.chars_db or JOIN_CORES[core][1]
    if not DB_USER_RE.match(user) or not DB_NAME_RE.match(auth) or not DB_NAME_RE.match(chars):
        sys.exit("--db-user, --auth-db, --chars-db: plain names")
    env = {"DB_HOST": a.db_host, "DB_PORT": str(a.db_port), "DB_USER": user, "DB_PASSWORD": password, "WU_CORE": core,
           "WU_DB_AUTH": auth, "WU_DB_CHARS": chars, "WU_EXPANSION": "2", "WU_REALM_DB_ID": str(a.realm_db_id)}
    problems = []
    if not a.skip_check:
        from .coredb import open_db
        try:
            probe = open_db(env).probe()
        except Exception as e:   # noqa: BLE001: a plain line, never the password
            sys.exit(f"Could not reach the login database at {a.db_host}:{a.db_port} as {user} ({type(e).__name__}). "
                     "Check the address and user, and that this PC may connect to it.")
        if probe.get("build") != BUILD:
            sys.exit(f"The realm list of this server is for another client (build {probe.get('build')}): Hero Key "
                     "sign-in is for servers of the Wrath client for now. Use link to list it without Hero Key sign-in.")
        if not probe.get("can_write"):
            problems.append("The database user may not add accounts: give it INSERT and UPDATE on the login database.")
        sys.stderr.write(f"Login database reached: realm \"{probe.get('realm_name')}\", {probe.get('accounts')} accounts.\n")
    a.build, a.core = BUILD, core
    cmd_link(a)
    with open(path(LINK_DB_ENV) + ".tmp", "w", encoding="utf-8") as f:
        os.chmod(path(LINK_DB_ENV) + ".tmp", 0o600)
        f.write(db_env_lines(core, a.db_host, a.db_port, user, password, auth, chars, a.realm_db_id))
    os.replace(path(LINK_DB_ENV) + ".tmp", path(LINK_DB_ENV))
    kids = [] if a.no_pin else pin_site_key((load() or {}).get("site") or domains.SITE)
    cfg = load()
    cfg.update(herokey=True, db_core=core)
    save(cfg)
    sys.stderr.write(("The website's Hero Key key is pinned (" + ", ".join(kids) + ").\n") if kids else
                     "The website does not offer Hero Key sign-in yet: your server is listed now and takes Hero Key "
                     "sign-ins once it does (then run: herokey pin).\n")
    for p_ in problems:
        sys.stderr.write("PROBLEM: " + p_ + "\n")
    sys.stderr.write("Next: start the agent with state/link-db.env, then run the self-test (Test Hero Key sign-in).\n")


def cmd_options(a):
    """The realm's own settings (type, rates, start): state/realm.env for the next start, and the list's type."""
    o = {k: getattr(a, k) for k in ("type", "xp", "loot", "rep", "start", "bots") if getattr(a, k, None) is not None}
    cfg = load()
    if not cfg:
        sys.exit("Run Host a Wrath realm first.")
    clean = check_options(o, cfg.get("profile"))
    if clean is None:
        sys.exit("Options: --type normal|pvp|rp|rppvp, --xp/--loot/--rep 1-10 (AzerothCore rulesets), "
                 "--start level1|boost (Classic levelling), --bots 0-200 (not the Dalaran hangout or other cores)")

    def ch(c):
        if "bots" in clean:   # the bot count is the compose file's WU_BOTS (.env), not realm.env
            c["bots"] = clean.pop("bots")
            set_env_value("WU_BOTS", str(c["bots"]))
        c.setdefault("realm_env", {}).update(clean)
        if "type" in clean:
            c.setdefault("record", {})["type"] = TYPES[clean["type"]][0]
            try:   # beacon.json is the host's copy of the list text: keep it in step
                with open(path("beacon.json"), encoding="utf-8-sig") as f:
                    b_full = json.load(f)
                if isinstance(b_full, dict):
                    b_full["type"] = TYPES[clean["type"]][0]
                    with open(path("beacon.json"), "w", encoding="utf-8") as f:
                        json.dump(b_full, f, indent=1)
            except (OSError, ValueError):
                pass
    _edit(ch)
    sys.stderr.write("Saved. Restart your realm (leyline stop, then leyline host) for it to take effect.\n")


def set_env_value(key, value):
    """One KEY=value line of the kit's .env (the rest kept; 0600 as write_env makes it)."""
    p = os.path.join(KIT, ".env")
    try:
        with open(p, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except OSError:
        return
    lines = [ln for ln in lines if not ln.startswith(key + "=")] + [f"{key}={value}"]
    fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")


def cmd_rules(a):
    """Re-read state/rules.txt and state/beacon.json; the agent sends the new text with its next heartbeat."""
    _edit(lambda c: None)
    rec = load()["record"]
    sys.stderr.write(f"Realm list text updated: {len(rec.get('rules') or [])} rule(s), tagline: {rec.get('tagline') or '(none)'}\n")


VOLUNTEER_ENV = "volunteer-relay.env"


def cmd_volunteer_relay(a):
    """The community relay profile (docs/LEYLINE-MESH.md): writes state/volunteer-relay.env for
    docker-compose.volunteer-relay.yml. It relays for others only while that compose file runs; --off removes it."""
    from wuhost import meshpick
    p = path(VOLUNTEER_ENV)
    if a.off:
        if os.path.exists(p):
            os.remove(p)
        sys.stderr.write("Community relay settings removed. Stop it with: docker compose -f docker-compose.volunteer-relay.yml down\n")
        return
    env = meshpick.volunteer_env(a.host, region=a.region or "", max_realms=a.max_realms, kbps=a.kbps,
                                 daily_gb=a.daily_gb, bootstrap=a.bootstrap or f"https://{domains.RELAY}",
                                 max_players=a.max_players, total_kbps=a.total_kbps)
    os.makedirs(path("node"), exist_ok=True)   # shared with the relay: status.json out, a "pause" file in
    os.makedirs(STATE, exist_ok=True)
    with open(p, "w", encoding="utf-8") as f:
        f.write(env)
    sys.stderr.write(f"Community relay settings saved ({a.host}). Start it with: "
                     "docker compose -f docker-compose.volunteer-relay.yml up -d\n")


def cmd_relay_pick(a):
    """Choose the relay this realm dials, from the mesh (signed relay records, verified here): our seed relays,
    or with --volunteers the least loaded community relay that has room. Writes host.json; restart the agent."""
    from wuhost import meshpick
    cfg = load() or {}
    if not cfg.get("realm_id"):
        sys.exit("Run init first.")
    host = meshpick.pick_relay(meshpick.fetch_relays(a.bootstrap or f"https://{domains.RELAY}"),
                               volunteers=a.volunteers, seeds={domains.RELAY})
    if not host:
        sys.exit("No relay with room was found: keeping " + cfg.get("relay", domains.RELAY))
    cfg.update(relay=host, relay_url=f"wss://{host}/agent", listing_url=f"https://{host}/api/listing/heartbeat",
               join_base=f"https://{host}")
    save(cfg)
    sys.stderr.write(f"This realm now uses the relay {host}. Restart the realm's agent.\n")


def main(argv=None):
    ap = argparse.ArgumentParser(description="Leyline host setup")
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("init")
    p.add_argument("--name")
    p.add_argument("--profile", choices=sorted(PROFILES))
    p.add_argument("--visibility", choices=["public", "private"])
    p.add_argument("--image", default="ghcr.io/orionopenwow/wu-ac-wotlk")
    p.add_argument("--tag", default="stable")
    p.add_argument("--tools-image", default="wu-host-tools:1")
    p.add_argument("--relay")
    p.add_argument("--site")
    p.add_argument("--bots", type=int)
    p.add_argument("--relay-url", help="override wss://<relay>/agent (tests, self-hosted relays)")
    p.add_argument("--listing-url", help="override https://<relay>/api/listing/heartbeat")
    p.add_argument("--warden", choices=["on", "off"])
    p.add_argument("--browser", choices=["on", "off"], help="browser players (Warden off); on by default on AzerothCore")
    p.add_argument("--join-base", help="where join pages live (default https://<relay>)")
    p.add_argument("--gm-name", help="the operator's own account name that gets GM rights (they make it)")
    p.add_argument("--tagline", help="one line for the realm list")
    p.add_argument("--signup", choices=["invite", "open", "off"])
    p.add_argument("--seed", help="the per-realm file from the website's host page (state/seed.b64)")
    add_listing_args(p)
    p.set_defaults(fn=cmd_init)
    p = sub.add_parser("listing", help="the realm list details: tags, region, language, max players, direct")
    add_listing_args(p)
    p.set_defaults(fn=cmd_listing)
    p = sub.add_parser("gm")
    p.add_argument("name")
    p.set_defaults(fn=cmd_gm)
    p = sub.add_parser("official", help="the operator's own realm as a Beacon (hosting/hangout)")
    p.add_argument("--name", required=True)
    p.add_argument("--relay")
    p.add_argument("--relay-url")
    p.add_argument("--listing-url")
    p.add_argument("--join-base")
    p.add_argument("--site")
    p.add_argument("--auth-service", required=True, help="host:port of the realm's authserver")
    p.add_argument("--world-service", required=True, help="host:port of the realm's worldserver")
    p.add_argument("--status-url", help="players and bots from this status document (openvanilla-status-1)")
    p.add_argument("--tagline")
    p.add_argument("--summary")
    p.add_argument("--rules-file")
    p.add_argument("--discord")
    p.add_argument("--ruleset", help="dalaran (default), den or custom")
    p.add_argument("--tags", help="comma list of realm list tags (default hangout,boosted)")
    p.add_argument("--max-players", type=int)
    p.set_defaults(fn=cmd_official)
    p = sub.add_parser("link", help="a server you already run, on the Leyline realm list (only the agent runs)")
    p.add_argument("--name")
    p.add_argument("--auth-service", help="host:port of your login server (from the agent's point of view)")
    p.add_argument("--world-service", help="host:port of your world server")
    p.add_argument("--build", type=int, help="the client your server runs: 5875, 8606 or 12340")
    p.add_argument("--core", choices=LINK_CORES)
    p.add_argument("--type", choices=sorted(LINK_TYPES))
    p.add_argument("--tags", help="comma list: " + ",".join(TAGS))
    p.add_argument("--region")
    p.add_argument("--language")
    p.add_argument("--tagline")
    p.add_argument("--summary")
    p.add_argument("--rules-file")
    p.add_argument("--discord")
    p.add_argument("--status-url", help="players and bots as JSON (format openvanilla-status-1); optional")
    p.add_argument("--relay")
    p.add_argument("--relay-url")
    p.add_argument("--listing-url")
    p.set_defaults(fn=cmd_link)
    sub.add_parser("realmlist").set_defaults(fn=cmd_realmlist)
    sub.add_parser("show").set_defaults(fn=cmd_show)
    sub.add_parser("get").set_defaults(fn=cmd_get)
    p = sub.add_parser("friend")
    p.add_argument("name")
    p.add_argument("password", nargs="?")
    p.set_defaults(fn=cmd_friend)
    sub.add_parser("new-invite").set_defaults(fn=cmd_new_invite)
    p = sub.add_parser("visibility")
    p.add_argument("visibility", choices=["public", "private"])
    p.set_defaults(fn=cmd_visibility)
    p = sub.add_parser("claim")
    p.add_argument("token")
    p.set_defaults(fn=cmd_claim)
    p = sub.add_parser("signup")
    p.add_argument("mode", choices=["invite", "open", "off"])
    p.set_defaults(fn=cmd_signup)
    p = sub.add_parser("browser", help="browser players: on = Warden off so the browser client can join (AzerothCore)")
    p.add_argument("state", choices=["on", "off"])
    p.set_defaults(fn=cmd_browser)
    p = sub.add_parser("herokey", help="Hero Key sign-in: players from the Leyline website, no join page (off by default)")
    p.add_argument("state", choices=["on", "off", "pin", "status"])
    p.set_defaults(fn=cmd_herokey)
    p = sub.add_parser("share-names", help="list who is online (character names) on the realm list (off by default)")
    p.add_argument("state", choices=["on", "off"])
    p.set_defaults(fn=cmd_share_names)
    p = sub.add_parser("join-network", help="an existing AzerothCore or TrinityCore 3.3.5a server joins with Hero Key sign-in")
    for k in ("--name", "--auth-service", "--world-service", "--type", "--tags", "--region", "--language", "--tagline",
              "--summary", "--rules-file", "--discord", "--status-url", "--relay", "--relay-url", "--listing-url"):
        p.add_argument(k)
    p.add_argument("--core", required=True, help="azerothcore or trinitycore (others: not yet)")
    p.add_argument("--db-host")
    p.add_argument("--db-port", type=int, default=3306)
    p.add_argument("--db-user")
    p.add_argument("--db-password-file", help="a file holding the password (or WU_JOIN_DB_PASSWORD); never printed")
    p.add_argument("--auth-db")
    p.add_argument("--chars-db")
    p.add_argument("--realm-db-id", type=int, default=1)
    p.add_argument("--skip-check", action="store_true", help="tests only: no database check")
    p.add_argument("--no-pin", action="store_true", help="do not fetch the website's key now")
    p.set_defaults(fn=cmd_join_network, build=None)
    sub.add_parser("rules").set_defaults(fn=cmd_rules)
    p = sub.add_parser("mods", help="the mods this realm runs or asks players to have (docs/GAME-MODS.md)")
    p.add_argument("action", nargs="?", choices=["list", "add", "remove", "set"], default="list")
    p.add_argument("--json", help="mods set: the whole list, JSON [{id, name, required, client}]")
    p.add_argument("ids", nargs="*", help="mod:<id>, up:<slug> or mode:<id>, optionally @VERSION")
    p.set_defaults(fn=cmd_mods)
    p = sub.add_parser("options", help="the realm's own settings: type, rates, start (state/realm.env)")
    p.add_argument("--type", choices=sorted(TYPES))
    p.add_argument("--xp", type=int)
    p.add_argument("--loot", type=int)
    p.add_argument("--rep", type=int)
    p.add_argument("--start", choices=["level1", "boost"])
    p.add_argument("--bots", type=int, help="random bots, 0-200 (not the Dalaran hangout or other cores)")
    p.set_defaults(fn=cmd_options)
    p = sub.add_parser("volunteer-relay", help="relay for others (the Leyline mesh's community relay; off by default)")
    p.add_argument("--host", help="this PC's public DNS name (needed for TLS)")
    p.add_argument("--region", choices=["na-east", "na-west", "sa", "eu", "africa", "me", "asia", "oce"])
    p.add_argument("--max-realms", type=int, default=25)
    p.add_argument("--kbps", type=int, default=256, help="per realm, both directions")
    p.add_argument("--daily-gb", type=int, default=5, help="per realm per day")
    p.add_argument("--max-players", type=int, help="players this PC carries at most, all realms together")
    p.add_argument("--total-kbps", type=int, help="this PC's upload for everyone it carries, KB/s")
    p.add_argument("--bootstrap", help="mesh nodes to start from (comma separated https URLs)")
    p.add_argument("--off", action="store_true")
    p.set_defaults(fn=cmd_volunteer_relay)
    p = sub.add_parser("relay-pick", help="choose this realm's relay from the Leyline mesh")
    p.add_argument("--volunteers", action="store_true", help="community relays may be chosen too")
    p.add_argument("--bootstrap")
    p.set_defaults(fn=cmd_relay_pick)
    a = ap.parse_args(argv)
    if a.cmd == "volunteer-relay" and not a.off and not a.host:
        ap.error("volunteer-relay needs --host (or --off)")
    a.fn(a)
    return 0


if __name__ == "__main__":
    sys.exit(main())
