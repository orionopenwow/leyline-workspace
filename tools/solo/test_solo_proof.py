#!/usr/bin/env python3
"""Leyline Solo, Phase 0 (docs/LEYLINE-SOLO-PHASE0.md): solo_proof.py and mpq112.py on fake folders.

    python3 -I tools/solo/test_solo_proof.py

The SRP6 rows against the core's own sample accounts, the MPQ writer through two readers (ours and den-check's), the
player's archives (encrypted, single-unit, implode, bzip2), the first-era detection, check being read-only, the
engine zip's checks, the server settings, the database steps (with a fake database), the sign-in on and off
(restoring every byte), a slot that isn't ours never touched, the sign-in add-on (Lua 5.0 only, our words, run
against a fake login screen when a Lua is on this PC), and the passwords never in anything printed.
"""
import contextlib
import hashlib
import io
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
import zipfile
import zlib
import bz2
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / "tools" / "den-check"))
import mpq112  # noqa: E402
import solo_proof as sp  # noqa: E402
import mpqlite  # noqa: E402


def fake_exe(major, minor, patch, build):
    b = b"MZ" + bytes(200) + struct.pack("<I", 0xFEEF04BD) + struct.pack("<I", 0x00010000)
    b += struct.pack("<II", (major << 16) | minor, (patch << 16) | build) + bytes(64)
    return b


TOC = "## Interface: 11200\r\nGlueLocalization.lua\r\nGlueParent.xml\r\nAccountLogin.xml\r\nCharacterSelect.xml\r\n"


def make_game(root, toc=TOC, exe=(1, 12, 1, 5875), config=True, realmlist=True, patch_toc=None):
    g = Path(root) / "Game"
    (g / "Data").mkdir(parents=True)
    (g / "WoW.exe").write_bytes(fake_exe(*exe))
    for n in ("dbc.MPQ", "terrain.MPQ", "model.MPQ", "texture.MPQ", "sound.MPQ"):
        (g / "Data" / n).write_bytes(mpq112.build([("x\\%s.txt" % n, b"placeholder")]))
    (g / "Data" / "interface.MPQ").write_bytes(mpq112.build([(sp.TOC, toc.encode("latin-1"))]))
    if patch_toc is not None:
        (g / "Data" / "patch.MPQ").write_bytes(mpq112.build([(sp.TOC, patch_toc.encode("latin-1"))]))
    if realmlist:
        (g / "realmlist.wtf").write_bytes(b"set realmlist logon.example.org\r\n")
    if config:
        (g / "WTF").mkdir()
        (g / "WTF" / "Config.wtf").write_bytes(b'SET gxResolution "1024x768"\r\nSET accountName "OLDNAME"\r\n')
    return g


def snapshot(folder):
    out = {}
    for p in sorted(Path(folder).rglob("*")):
        if p.is_file():
            st = p.stat()
            out[str(p.relative_to(folder))] = (hashlib.sha256(p.read_bytes()).hexdigest(), int(st.st_mtime))
    return out


@contextlib.contextmanager
def captured():
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        yield buf


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.home = Path(self.tmp) / "home"
        self.env = mock.patch.dict(os.environ, {"LEYLINE_SOLO_DIR": str(self.home),
                                                "LEYLINE_APP_DIR": str(Path(self.tmp) / "app")})
        self.env.start()
        self.folder_txt = mock.patch.object(sp, "HERE", Path(self.tmp) / "toolhome")
        # the tool's own files (glue, conf) stay where they are; only folder.txt is looked up under HERE
        (Path(self.tmp) / "toolhome").mkdir()
        shutil.copytree(HERE / "glue", Path(self.tmp) / "toolhome" / "glue")
        self.folder_txt.start()
        sp._SECRETS.clear()

    def tearDown(self):
        self.folder_txt.stop()
        self.env.stop()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def set_up_state(self, **kw):
        st = {"setup_done": True, "root_pw": "ROOTPW1234567890ABCD", "db_pw": "DBPW1234567890ABCDEF",
              "solo_pw": "SOLOPW7777777777", "admin_pw": "ADMINPW888888888"}
        st.update(kw)
        sp.save_state(st)
        return st


# ------------------------------------------------------------------------------------------------ accounts
class Srp6(unittest.TestCase):
    def test_matches_the_cores_sample_accounts(self):
        """realmd.sql's sample rows (their passwords are their names) are what srp6_row makes for their salts."""
        for user, s, v in (
                ("ADMINISTRATOR", "8EB5DE915AA3D805FA7099CF61C0BB8A77990EA869078A0C5B9EEE55828F4505",
                 "312B99EEF1C0196BB73B79D114CE161C5D089319E6EF54FAA6117DAB8B672C14"),
                ("PLAYER", "EBA23AF194D89B8061CA7FEBA06D336B1C38D8FBDABA76F2C51D45141362D881",
                 "3738EC7E7C731FD431C716990C6D97CA5C1D50EF0DA7DE9819076DE1D03AA891")):
            self.assertEqual(sp.srp6_row(user, user.lower(), int(s, 16)), (s, v))

    def test_new_salt_has_a_top_byte(self):
        for _ in range(20):
            s, v = sp.srp6_row("SOLO", "ABC")
            self.assertEqual(len(s), 64)
            self.assertNotEqual(s[:2], "00")

    def test_passwords_fit_the_game(self):
        for _ in range(20):
            p = sp.new_password()
            self.assertRegex(p, r"^[A-Z0-9]{16}$")


# ------------------------------------------------------------------------------------------------ MPQ
class Mpq(unittest.TestCase):
    FILES = [("Interface/GlueXML/GlueXML.toc", TOC.encode()), ("Interface\\GlueXML\\Big.lua", os.urandom(9000) + b"a" * 20000),
             ("Empty.txt", b""), ("Small.txt", b"hi")]

    def test_round_trip_through_both_readers(self):
        b = mpq112.build(self.FILES)
        with tempfile.NamedTemporaryFile(delete=False, suffix=".MPQ") as f:
            f.write(b)
        try:
            other = mpqlite.Archive(f.name)
            ours = mpq112.Archive(b)
            for n, d in self.FILES:
                self.assertEqual(ours.read(n), d, n)
                self.assertEqual(ours.read(n.lower()), d, n)
                if d:
                    self.assertEqual(other.read(n.replace("/", "\\")), d, n)
            self.assertEqual(ours.names(), [n.replace("/", "\\") for n, _ in self.FILES])
            self.assertFalse(ours.has("Nope.txt"))
        finally:
            os.unlink(f.name)

    def test_refuses_bad_and_repeated_names(self):
        for bad in ("..\\x", "\\abs", "a:b", "", "(listfile)", "x" * 201):
            with self.assertRaises(mpq112.MpqError, msg=bad):
                mpq112.build([(bad, b"x")])
        with self.assertRaises(mpq112.MpqError):
            mpq112.build([("A.txt", b"1"), ("a.TXT", b"2")])

    def test_reads_the_launchers_own_archive(self):
        """den-check's sample is launcher/src-tauri/src/mpq.rs's own output: the same subset, read the same way."""
        a = mpq112.Archive(str(ROOT / "tools" / "den-check" / "fixtures" / "patch-Z-sample.bin"))
        names = a.names()
        self.assertIn("Interface\\GlueXML\\GlueXML.toc", names)
        for n in names:
            a.read(n)

    def test_explode_vector(self):
        """zlib's contrib/blast example stream."""
        self.assertEqual(mpq112.explode(bytes([0x00, 0x04, 0x82, 0x24, 0x25, 0x8F, 0x80, 0x7F])), b"AIAIAIAIAIAIA")

    def _archive(self, name, sectors_or_unit, flags, fsize):
        """An archive holding one file stored as given (the player's archives' ways)."""
        body = sectors_or_unit
        hdr = 32
        pos = hdr
        size = 16
        table = [[mpq112.M32] * 4 for _ in range(size)]
        at = mpq112.hash_name(name, 0) & (size - 1)
        table[at] = [mpq112.hash_name(name, 1), mpq112.hash_name(name, 2), 0, 0]
        blocks = [[pos, len(body), fsize, flags]]
        hw = mpq112.encrypt_words([w for e in table for w in e], mpq112.hash_name("(hash table)", 3))
        bw = mpq112.encrypt_words([w for e in blocks for w in e], mpq112.hash_name("(block table)", 3))
        out = bytearray(hdr) + body
        hp = len(out)
        out += struct.pack("<%dI" % len(hw), *hw)
        bp = len(out)
        out += struct.pack("<%dI" % len(bw), *bw)
        out[:32] = b"MPQ\x1a" + struct.pack("<IIHHIIII", 32, len(out), 0, 3, hp, bp, size, 1)
        return bytes(out), pos

    def _enc(self, data, key):
        data = data + bytes(-len(data) % 4)
        n = len(data) // 4
        return struct.pack("<%dI" % n, *mpq112.encrypt_words(struct.unpack("<%dI" % n, data), key))

    def test_encrypted_compressed_sectors_with_fixed_key(self):
        name = "Interface\\GlueXML\\GlueXML.toc"
        text = (TOC * 400).encode()
        sec = mpq112.SECTOR_SIZE
        chunks = [text[i:i + sec] for i in range(0, len(text), sec)]
        comp = [b"\x02" + zlib.compress(c) for c in chunks]
        # the file key: the base name's, fixed by the block's position and size
        base = mpq112.hash_name("GlueXML.toc", 3)
        key = ((base + 32) ^ len(text)) & mpq112.M32
        start = 4 * (len(comp) + 1)
        offs, at = [], start
        for c in comp:
            offs.append(at)
            at += len(c)
        offs.append(at)
        enc_table = struct.pack("<%dI" % len(offs), *mpq112.encrypt_words(offs, (key - 1) & mpq112.M32))
        body = enc_table + b"".join(self._enc_exact(c, (key + i) & mpq112.M32) for i, c in enumerate(comp))
        b, _ = self._archive(name, body, mpq112.FLAG_EXISTS | mpq112.FLAG_COMPRESS | mpq112.FLAG_ENCRYPTED | mpq112.FLAG_FIX_KEY,
                             len(text))
        self.assertEqual(mpq112.Archive(b).read(name), text)

    def _enc_exact(self, data, key):
        n = len(data) // 4
        return struct.pack("<%dI" % n, *mpq112.encrypt_words(struct.unpack("<%dI" % n, data[:n * 4]), key)) + data[n * 4:]

    def test_single_unit_bzip2_and_implode_sectors(self):
        name = "A.txt"
        text = b"hello first era " * 50
        b, _ = self._archive(name, b"\x10" + bz2.compress(text),
                             mpq112.FLAG_EXISTS | mpq112.FLAG_COMPRESS | mpq112.FLAG_SINGLE_UNIT, len(text))
        self.assertEqual(mpq112.Archive(b).read(name), text)
        imp = bytes([0x00, 0x04, 0x82, 0x24, 0x25, 0x8F, 0x80, 0x7F])
        body = struct.pack("<II", 8, 8 + len(imp)) + imp
        b, _ = self._archive(name, body, mpq112.FLAG_EXISTS | mpq112.FLAG_IMPLODE, 13)
        self.assertEqual(mpq112.Archive(b).read(name), b"AIAIAIAIAIAIA")

    def test_not_an_archive(self):
        with self.assertRaises(mpq112.MpqError):
            mpq112.Archive(b"hello" * 100)


# ------------------------------------------------------------------------------------------------ folders, check
class Folders(Base):
    def test_first_era_by_program_and_by_archives(self):
        g = make_game(self.tmp)
        self.assertTrue(sp.first_era(g)[0])
        (g / "WoW.exe").write_bytes(fake_exe(3, 3, 5, 12340))
        self.assertFalse(sp.first_era(g)[0])
        (g / "WoW.exe").unlink()
        self.assertTrue(sp.first_era(g)[0])           # by the first era's archives
        (g / "Data" / "common.MPQ").write_bytes(b"x")
        self.assertFalse(sp.first_era(g)[0])

    def test_folder_from_argument_folder_txt_then_launcher(self):
        app = Path(self.tmp) / "app"
        app.mkdir()
        (app / "settings.json").write_text(json.dumps({"selected_client": "b", "clients": [
            {"id": "w", "build": "3.3.5a", "path": "C:/Wrath"}, {"id": "a", "build": "1.12.1", "path": "C:/One", "name": "Vanilla 1.12.1"},
            {"id": "b", "build": "1.12.1", "path": "C:/Two"}]}))
        self.assertEqual(sp.game_folder()[0], Path("C:/Two"))             # the selected first-era one
        (sp.HERE / "folder.txt").write_text("# my game\n\"D:\\Games\\First\"\n")
        self.assertEqual(sp.game_folder()[0], Path("D:\\Games\\First"))
        self.assertEqual(sp.game_folder("E:/x")[0], Path("E:/x"))

    def test_check_is_read_only_and_passes(self):
        g = make_game(self.tmp, patch_toc=TOC + "Extra.xml\r\n")
        before = snapshot(g)
        with mock.patch.object(sp, "port_free", return_value=True), mock.patch.object(sp, "free_gb", return_value=50.0), \
                captured() as buf:
            rc = sp.main(["check", str(g)])
        self.assertEqual(rc, 0, buf.getvalue())
        self.assertIn("CHECK OK", buf.getvalue())
        self.assertIn("read from your patch.MPQ", buf.getvalue())
        self.assertEqual(snapshot(g), before)
        self.assertFalse(sp.state_path().exists())

    def test_check_fails_on_a_later_client(self):
        g = make_game(self.tmp, exe=(3, 3, 5, 12340))
        with mock.patch.object(sp, "port_free", return_value=True), mock.patch.object(sp, "free_gb", return_value=50.0), \
                captured() as buf:
            self.assertEqual(sp.main(["check", str(g)]), 1)
        self.assertIn("FAIL first-era client", buf.getvalue())

    def test_toc_from_the_archive_read_last(self):
        g = make_game(self.tmp, patch_toc=TOC + "FromPatch.xml\r\n")
        (g / "Data" / "patch-2.MPQ").write_bytes(mpq112.build([(sp.TOC, (TOC + "FromPatch2.xml\r\n").encode())]))
        (g / "Data" / "patch-Z.MPQ").write_bytes(mpq112.build([(sp.TOC, b"ours, never read for the list")]))
        toc, src = sp.player_toc(g, "patch-Z.MPQ")
        self.assertEqual(src, "patch-2.MPQ")
        self.assertIn("FromPatch2.xml", toc)
        self.assertEqual(sp.slot_order(["patch.MPQ", "interface.MPQ", "patch-3.MPQ", "patch-A.MPQ", "dbc.MPQ", "patch-Z.MPQ"],
                                       "patch-Z.MPQ"), ["patch-A.MPQ", "patch-3.MPQ", "patch.MPQ", "interface.MPQ"])


# ------------------------------------------------------------------------------------------------ engine, settings, database
class Engine(Base):
    def make_zip(self, extra=None):
        z = Path(self.tmp) / sp.ZIP
        with zipfile.ZipFile(z, "w") as zf:
            zf.writestr("solo-engine.json", json.dumps({"name": "leyline-solo-engine", "version": "2026.11.07-abc"}))
            zf.writestr("bin/conf-dist/mangosd.conf.dist", "[MangosdConf]\nDataDir = \".\"\nRate.XP.Kill    = 1\nBindIP = \"0.0.0.0\"\n")
            for k, v in (extra or {}).items():
                zf.writestr(k, v)
        (Path(str(z) + ".sha256")).write_text(sp.sha256_file(z) + "  " + sp.ZIP + "\n")
        return z

    def test_local_zip_checked_and_unpacked_once(self):
        z = self.make_zip()
        st = {}
        with captured():
            info = sp.get_engine(st, str(z))
        self.assertEqual(info["version"], "2026.11.07-abc")
        self.assertTrue((self.home / "engine" / "bin" / "conf-dist" / "mangosd.conf.dist").is_file())
        with captured() as buf:
            sp.get_engine(st, str(z))
        self.assertIn("already unpacked", buf.getvalue())

    def test_zip_that_does_not_match_its_sum_is_refused(self):
        z = self.make_zip()
        Path(str(z) + ".sha256").write_text("0" * 64 + "  x\n")
        with self.assertRaises(SystemExit), captured():
            sp.get_engine({}, str(z))

    def test_zip_with_a_path_outside_is_refused(self):
        z = self.make_zip({"../evil.txt": "x"})
        with self.assertRaises(SystemExit), captured():
            sp.get_engine({}, str(z))
        self.assertFalse((Path(self.tmp) / "evil.txt").exists())

    def test_apply_conf(self):
        dist = "[X]\nA = 1\n# A = commented\nB.c = \"x\"\n"
        self.assertEqual(sp.apply_conf(dist, {"A": "2"}, {"D": "4", "B.c": "\"y\""}),
                         "[X]\nA = 2\n# A = commented\nB.c = \"y\"\nD = 4\n")
        o = sp.overrides((HERE / "conf" / "solo.conf").read_text())
        self.assertEqual(o["MaxPlayerLevel"], "60")
        self.assertEqual(o["BindIP"], "\"127.0.0.1\"")
        self.assertEqual(o["Rate.XP.Kill"], "2")
        bots = sp.overrides((HERE / "conf" / "solo-aiplayerbot.conf").read_text())
        for k in ("AiPlayerbot.RandomBotAutologin", "AiPlayerbot.RandomBotAutoCreate", "AiPlayerbot.MaxRandomBots",
                  "AiPlayerbot.RandomBotJoinBG", "AiPlayerbot.LLMEnabled"):
            self.assertEqual(bots[k], "0", k)
        self.assertEqual(bots["AiPlayerbot.Enabled"], "1")
        self.assertEqual(sp.overrides((HERE / "conf" / "solo-anticheat.conf").read_text())["Warden.Enable"], "0")

    def test_write_confs(self):
        eng = Path(self.tmp) / "eng"
        (eng / "bin" / "conf-dist").mkdir(parents=True)
        for n in ("mangosd", "realmd", "aiplayerbot", "anticheat"):
            (eng / "bin" / "conf-dist" / ("%s.conf.dist" % n)).write_text("[Conf]\nBindIP = \"0.0.0.0\"\nDataDir = \".\"\n")
        shutil.copytree(HERE / "conf", eng / "conf")
        (eng / "bin" / "legacy.dll").write_bytes(b"MZ provider")
        st = self.set_up_state()
        sp.write_confs(st, eng=eng)
        self.assertEqual((self.home / "run" / "legacy.dll").read_bytes(), b"MZ provider")   # beside the servers
        m = (self.home / "run" / "mangosd.conf").read_text()
        self.assertIn('BindIP = "127.0.0.1"', m)
        self.assertIn('DataDir = "%s"' % sp.fwd(self.home / "data"), m)
        self.assertIn("WorldDatabaseInfo = \"127.0.0.1;%d;solo;%s;classicmangos\"" % (sp.PORTS["database"], st["db_pw"]), m)
        self.assertIn("mmap.enabled = 0", m)
        self.assertIn('SOAP.IP = "127.0.0.1"', m)
        self.assertIn("MaxPlayerLevel = 60", m)
        self.assertNotIn(st["root_pw"], m)
        r = (self.home / "run" / "realmd.conf").read_text()
        self.assertIn("RealmServerPort = 3724", r)
        self.assertIn("AiPlayerbot.MaxRandomBots = 0", (self.home / "run" / "aiplayerbot.conf").read_text())
        self.assertIn("Warden.Enable = 0", (self.home / "run" / "anticheat.conf").read_text())

    def test_db_init_with_a_fake_database_never_prints_a_password(self):
        sqls = []

        def fake_mysql(st, sql=None, db=None, stdin_path=None, eng=None, check=True):
            sqls.append((db, sql, stdin_path))
            return "5123\n" if sql and "COUNT(*)" in sql else ""
        (self.home / "db" / "mysql").mkdir(parents=True)
        st = {"db_imported": True}
        with mock.patch.object(sp, "mysql", fake_mysql), mock.patch.object(sp, "db_start", return_value=None), \
                mock.patch.object(sp, "db_stop"), captured() as buf:
            sp.db_init(st, eng=Path(self.tmp) / "eng")
        text = buf.getvalue()
        for k in ("root_pw", "db_pw", "solo_pw", "admin_pw"):
            self.assertTrue(st[k])
            self.assertNotIn(st[k], text)
        joined = "\n".join(s or "" for _, s, _ in sqls)
        self.assertIn("CREATE USER IF NOT EXISTS 'solo'@'127.0.0.1'", joined)
        self.assertIn("INSERT INTO account (username, gmlevel, v, s, joindate) VALUES ('SOLO', 0,", joined)
        self.assertIn("VALUES ('SOLOSTOP', 3,", joined)
        self.assertIn("'Leyline Solo', '127.0.0.1', 8085", joined)
        self.assertNotIn(st["solo_pw"], joined)       # accounts are verifiers, never the password
        saved = json.loads(sp.state_path().read_text())
        self.assertEqual(saved["solo_pw"], st["solo_pw"])

    def test_mysql_sends_statements_on_stdin(self):
        st = self.set_up_state()
        calls = []

        def fake_run(args, **kw):
            calls.append((args, kw))
            return subprocess.CompletedProcess(args, 0, b"1\n", b"")
        with mock.patch.object(sp.subprocess, "run", fake_run):
            sp.mysql(st, "ALTER USER 'solo' IDENTIFIED BY '%s'" % st["db_pw"], eng=Path(self.tmp) / "eng")
        args, kw = calls[0]
        self.assertFalse(any(st["db_pw"] in a or st["root_pw"] in a for a in args))
        self.assertIn(st["db_pw"].encode(), kw["input"])
        cnf = (self.home / "run" / "client.cnf").read_text()
        self.assertIn("password=" + st["root_pw"], cnf)

    @unittest.skipIf(os.name == "nt", "the fake extractors are shell scripts")
    def test_extract_runs_the_tools_on_the_players_folder(self):
        g = make_game(self.tmp)
        tools = self.home / "engine" / "tools"
        tools.mkdir(parents=True)
        log = Path(self.tmp) / "calls.txt"
        for n in ("ad", "vmap_extractor", "vmap_assembler", "MoveMapGen"):
            p = tools / n
            p.write_text("#!/bin/sh\necho %s \"$@\" >> '%s'\nmkdir -p Buildings\n" % (n, log))
            p.chmod(0o755)
        st = {}
        with captured() as buf:
            sp.extract(st, g, mmaps=False)
        calls = log.read_text()
        self.assertIn("ad -i %s -o %s" % (g, self.home / "data"), calls)
        self.assertIn("vmap_extractor -d %s -o %s" % (g / "Data", self.home / "data"), calls)
        self.assertIn("vmap_assembler", calls)
        self.assertNotIn("MoveMapGen", calls)
        self.assertIn("monster paths: skipped", buf.getvalue())
        self.assertFalse(st["mmaps"])
        log.write_text("")
        with captured():
            sp.extract(st, g, mmaps=True)              # maps and vmaps kept; only the paths run
        self.assertEqual(log.read_text().split()[0], "MoveMapGen")
        self.assertTrue(st["mmaps"])


# ------------------------------------------------------------------------------------------------ sign-in
class Signin(Base):
    def test_on_then_off_restores_every_byte(self):
        g = make_game(self.tmp, patch_toc=TOC + "Patched.xml\r\n")
        before = snapshot(g)
        st = self.set_up_state()
        with captured() as buf:
            self.assertEqual(sp.main(["signin", str(g)]), 0)
        text = buf.getvalue()
        self.assertIn("SIGNIN ON", text)
        self.assertEqual((g / "realmlist.wtf").read_bytes(), b"set realmlist 127.0.0.1\r\n")
        cfg = (g / "WTF" / "Config.wtf").read_bytes()
        self.assertIn(b'SET accountName "SOLO"\r\n', cfg)
        self.assertIn(b'SET realmName "Leyline Solo"\r\n', cfg)
        self.assertIn(b'SET gxResolution "1024x768"\r\n', cfg)
        self.assertNotIn(b"OLDNAME", cfg)
        arc = mpq112.Archive(str(g / "Data" / "patch-Z.MPQ"))
        toc = arc.read(sp.TOC).decode()
        self.assertTrue(toc.startswith(TOC + "Patched.xml\r\n"))       # the player's own list, read from patch.MPQ
        self.assertTrue(toc.endswith("LeylineSolo.xml\r\n"))
        self.assertEqual(arc.read(sp.OUR_LUA), (HERE / "glue" / "LeylineSolo.lua").read_bytes())
        sess = arc.read(sp.SESSION).decode()
        self.assertNotIn(st["solo_pw"], sess)                           # escaped bytes, not plain text
        self.assertIn(sp.lua_bytes(st["solo_pw"]), sess)
        self.assertTrue(sp.is_ours(g / "Data" / "patch-Z.MPQ"))
        pwf = sp.password_file()
        self.assertIn(st["solo_pw"], pwf.read_text())
        self.assertNotIn(st["solo_pw"], text)
        # again: idempotent, the first backups kept
        with captured():
            self.assertEqual(sp.main(["signin", str(g)]), 0)
        with captured() as buf:
            self.assertEqual(sp.main(["signin", "--off"]), 0)
        self.assertIn("SIGNIN OFF: your game folder is as it was.", buf.getvalue())
        self.assertEqual(snapshot(g), before)
        self.assertFalse(pwf.exists())
        self.assertEqual(list((self.home / "signin-backup").iterdir()), [])

    def test_files_that_were_not_there_are_removed_again(self):
        g = make_game(self.tmp, config=False, realmlist=False)
        before = snapshot(g)
        self.set_up_state()
        with captured():
            self.assertEqual(sp.main(["signin", str(g)]), 0)
        self.assertTrue((g / "WTF" / "Config.wtf").is_file())
        self.assertTrue((g / "realmlist.wtf").is_file())
        with captured():
            self.assertEqual(sp.main(["signin", "--off"]), 0)
        self.assertEqual(snapshot(g), before)

    def test_a_slot_that_isnt_ours_is_never_touched(self):
        g = make_game(self.tmp)
        theirs = mpq112.build([("Textures\\Hd.txt", b"someone else's mod")])
        (g / "Data" / "patch-Z.MPQ").write_bytes(theirs)
        self.set_up_state()
        with captured() as buf:
            self.assertEqual(sp.main(["signin", str(g)]), 1)
        self.assertIn("isn't ours", buf.getvalue())
        self.assertEqual((g / "Data" / "patch-Z.MPQ").read_bytes(), theirs)
        with captured():
            self.assertEqual(sp.main(["signin", "--off"]), 0)
        self.assertEqual((g / "Data" / "patch-Z.MPQ").read_bytes(), theirs)
        # another slot works
        with captured():
            self.assertEqual(sp.main(["signin", str(g), "--slot", "patch-Y.MPQ"]), 0)
        self.assertTrue(sp.is_ours(g / "Data" / "patch-Y.MPQ"))
        with captured():
            self.assertEqual(sp.main(["signin", "--off"]), 0)
        self.assertFalse((g / "Data" / "patch-Y.MPQ").exists())

    def test_stop_takes_the_password_out(self):
        g = make_game(self.tmp)
        st = self.set_up_state()
        with captured():
            sp.main(["signin", str(g), "--char", "Wren"])
        sess = mpq112.Archive(str(g / "Data" / "patch-Z.MPQ")).read(sp.SESSION).decode()
        self.assertIn(sp.lua_bytes("Wren"), sess)
        with captured():
            self.assertEqual(sp.main(["stop"]), 0)
        arc = mpq112.Archive(str(g / "Data" / "patch-Z.MPQ"))
        self.assertEqual(arc.read(sp.SESSION).decode(), sp.NO_SESSION)
        self.assertNotIn(sp.lua_bytes(st["solo_pw"]).encode(), arc.read(sp.SESSION))
        with captured():
            self.assertEqual(sp.main(["signin", "--off"]), 0)       # still recognised as ours after the rewrite
        self.assertFalse((g / "Data" / "patch-Z.MPQ").exists())

    def test_no_addon_only_prefills(self):
        g = make_game(self.tmp)
        self.set_up_state()
        with captured() as buf:
            self.assertEqual(sp.main(["signin", str(g), "--no-addon"]), 0)
        self.assertFalse((g / "Data" / "patch-Z.MPQ").exists())
        self.assertIn("type the password once", buf.getvalue())
        with captured():
            sp.main(["signin", "--off"])

    def test_refusals(self):
        g = make_game(self.tmp)
        with captured() as buf:
            self.assertEqual(sp.main(["signin", str(g)]), 1)          # not set up
        self.assertIn("solo-setup first", buf.getvalue())
        self.set_up_state()
        with captured():
            self.assertEqual(sp.main(["signin", str(g), "--char", "x1"]), 1)
            self.assertEqual(sp.main(["signin", str(g), "--slot", "patch-ZZ.MPQ"]), 1)
        with self.assertRaises(ValueError):
            sp.session_lua("SOLO", "has space", "Leyline Solo")
        with self.assertRaises(ValueError):
            sp.session_lua("SOLO", "ABC", "Leyline Solo", "lowercase")

    def test_scrub(self):
        sp._SECRETS.append("SECRETSECRET1234")
        self.assertEqual(sp.scrub("db said SECRETSECRET1234 no"), "db said [hidden] no")


# ------------------------------------------------------------------------------------------------ the add-on
class Glue(unittest.TestCase):
    BRAND = ("World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard")

    def strip_strings(self, lua):
        import re
        lua = re.sub(r"--[^\n]*", "", lua)
        return re.sub(r'"(\\.|[^"\\])*"', '""', lua)

    def test_lua_50_only(self):
        lua = (HERE / "glue" / "LeylineSolo.lua").read_text()
        code = self.strip_strings(lua)
        for bad in ("#", "%", "string.match", ":match(", "gmatch", "select(", "...", "goto ", "//"):
            self.assertNotIn(bad, code, bad)

    def test_our_words(self):
        for f in ("LeylineSolo.lua", "LeylineSolo.xml"):
            text = (HERE / "glue" / f).read_text()
            for w in self.BRAND:
                if w == "WoW":
                    self.assertNotRegex(text.replace("blizzard.com/wow/ui", ""), r"\bWoW\b", f)
                else:
                    self.assertNotIn(w, text.replace("http://www.blizzard.com/wow/ui/", ""), f)

    def test_xml_loads_session_then_code(self):
        x = (HERE / "glue" / "LeylineSolo.xml").read_text()
        self.assertLess(x.index('Script file="LeylineSoloSession.lua"'), x.index('Script file="LeylineSolo.lua"'))
        self.assertIn("LeylineSolo_OnUpdate(arg1)", x)

    @unittest.skipUnless(shutil.which("texlua") or shutil.which("lua") or shutil.which("lua5.1"), "no Lua on this PC")
    def test_drives_a_fake_login_screen(self):
        lua = shutil.which("lua") or shutil.which("lua5.1") or shutil.which("texlua")
        session = sp.session_lua("SOLO", "PW12", "Leyline Solo", "Wren")
        harness = r'''
local calls = {}
local function frame(shown) return { s = shown, IsShown = function(self) return self.s end } end
AccountLogin, CharacterSelect, RealmList, GlueDialog = frame(true), frame(false), frame(false), frame(false)
LeylineSoloNote = { SetText = function(self, t) calls.note = t end, SetFont = function() end }
LeylineSoloDriver = { Show = function() end }
function DefaultServerLogin(u, p) calls.login = u .. "/" .. p; AccountLogin.s = false; CharacterSelect.s = true end
function GetNumCharacters() return 2 end
function GetCharacterInfo(i) if i == 1 then return "Bob" else return "Wren" end end
function SelectCharacter(i) calls.select = i end
function EnterWorld() calls.enter = true end
dofile(arg[1]); dofile(arg[2])
assert(LEYLINE_SOLO_SESSION == nil, "the global is cleared")
for i = 1, 40 do LeylineSolo_OnUpdate(0.2) end
assert(calls.login == "SOLO/PW12", tostring(calls.login))
assert(calls.select == 2, "picked by name")
assert(calls.enter, "entered the world")
assert(LeylineSolo.state == "entering", LeylineSolo.state)
for k, v in pairs(LeylineSolo) do assert(v ~= "PW12", "no password kept") end
-- without a session nothing runs
LEYLINE_SOLO_SESSION = nil; calls = {}
AccountLogin.s = true
dofile(arg[2])
for i = 1, 10 do LeylineSolo_OnUpdate(0.5) end
assert(calls.login == nil)
-- an error dialog stops it and says why
LEYLINE_SOLO_SESSION = { user = "SOLO", pass = "X", realm = "Leyline Solo" }
function DefaultServerLogin(u, p) AccountLogin.s = false; GlueDialog.s = true; GlueDialog.which = "OKAY" end
calls = {}
dofile(arg[2])
for i = 1, 5 do LeylineSolo_OnUpdate(0.2) end
assert(LeylineSolo.state == "done" and LeylineSolo.why == "dialog", tostring(LeylineSolo.why))
assert(string.find(calls.note, "stopped"), calls.note)
print("LUA OK")
'''
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "s.lua").write_text(session)
            (Path(d) / "h.lua").write_text(harness)
            p = subprocess.run([lua, str(Path(d) / "h.lua"), str(Path(d) / "s.lua"), str(HERE / "glue" / "LeylineSolo.lua")],
                               capture_output=True, text=True)
        self.assertIn("LUA OK", p.stdout + p.stderr, p.stdout + p.stderr)


class LauncherGlue(unittest.TestCase):
    """The launcher's copy of the add-on (launcher/src-tauri/glue/, packed by host/classicsign.rs) is this one, and its
    stand-in login screen tests (pick, Quick start's make, enter) pass on this copy too."""
    LAUNCHER = ROOT / "launcher" / "src-tauri" / "glue"

    def test_same_files_as_the_launchers(self):
        for f in ("LeylineSolo.lua", "LeylineSolo.xml"):
            self.assertEqual((HERE / "glue" / f).read_bytes(), (self.LAUNCHER / f).read_bytes(), f)

    @unittest.skipUnless(shutil.which("texlua") or shutil.which("lua") or shutil.which("lua5.1"), "no Lua on this PC")
    def test_the_launchers_login_screen_tests(self):
        lua = shutil.which("lua5.1") or shutil.which("lua") or shutil.which("texlua")
        p = subprocess.run([lua, str(self.LAUNCHER / "test_solo_glue.lua"), str(HERE / "glue" / "LeylineSolo.lua")],
                           capture_output=True, text=True)
        self.assertIn("all passed", p.stdout, p.stdout + p.stderr)


class ControlSteps(unittest.TestCase):
    def test_bats_call_the_tool_with_crlf(self):
        for name, cmd in (("Solo check.bat", "check"), ("Solo setup.bat", "setup"), ("Solo start.bat", "start"),
                          ("Solo stop.bat", "stop"), ("Solo sign-in.bat", "signin"), ("Solo sign-in off.bat", "signin --off"),
                          ("Solo status.bat", "status")):
            b = (HERE / name).read_bytes()
            self.assertIn(b"\r\n", b, name)
            self.assertNotIn(b"\n", b.replace(b"\r\n", b""), name)
            self.assertIn(('solo_proof.py" %s' % cmd).encode(), b, name)


if __name__ == "__main__":
    unittest.main()
