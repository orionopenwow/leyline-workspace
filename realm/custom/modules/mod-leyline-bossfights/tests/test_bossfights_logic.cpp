// g++ -std=c++17 -Wall -Wextra -Werror -I../src test_bossfights_logic.cpp -o t && ./t   (no core needed)
#include "BossfightsLogic.h"

#include <cstdio>
#include <cstring>

using namespace LeylineBF;

static int g_pass = 0, g_fail = 0;
static void check(char const* name, bool ok)
{
    if (ok) { ++g_pass; std::printf("PASS %s\n", name); }
    else { ++g_fail; std::printf("FAIL %s\n", name); }
}

static bool hasBrandWord(std::string s)
{
    s = Lower(s);
    for (char const* w : { "lich king", "lichking", "warcraft", "blizzard", "wow", "arthas" })
        if (s.find(w) != std::string::npos)
            return true;
    return false;
}

int main()
{
    // the bosses table
    BossDef const* ft = FindBoss("frozen-throne");
    check("frozen-throne is listed", ft != nullptr);
    check("aliases", FindBoss("FT") == ft && FindBoss("throne") == ft && FindBoss("Frozen-Throne") == ft);
    check("unknown boss", FindBoss("onyxia") == nullptr && FindBoss("") == nullptr);
    check("map, entry, boss id, end spell, size", ft->mapId == 631 && ft->bossEntry == 36597 && ft->bossData == 12
        && ft->endSpell == 72350 && ft->size == 10);
    check("prerequisites: bosses 0-11 and the Sindragosa gauntlet (13), never the fight itself",
        ft->prereq.size() == 13 && IsPrereq(*ft, 0) && IsPrereq(*ft, 11) && IsPrereq(*ft, 13) && !IsPrereq(*ft, 12)
        && !IsPrereq(*ft, 14));
    check("the raid lands on the platform, inside its own fence",
        (ft->x - ft->fenceX) * (ft->x - ft->fenceX) + (ft->y - ft->fenceY) * (ft->y - ft->fenceY) < ft->fenceR * ft->fenceR
        && ft->z > ft->fenceMinZ);
    check("FindBossByMap", FindBossByMap(631) == ft && FindBossByMap(0) == nullptr);
    bool clean = true;
    for (BossDef const& b : Bosses())
        clean = clean && !hasBrandWord(b.title) && !hasBrandWord(b.key);
    check("no brand words in titles or keys", clean);

    // arguments
    check("difficulty", ParseDifficulty("normal") == DIFF_NORMAL && ParseDifficulty("HEROIC") == DIFF_HEROIC
        && ParseDifficulty("10h") == DIFF_HEROIC && ParseDifficulty("25") == DIFF_NONE && ParseDifficulty("") == DIFF_NONE);
    check("mode", ParseMode("bots") == MODE_BOTS && ParseMode("group") == MODE_GROUP && ParseMode("group-only") == MODE_GROUP_ONLY
        && ParseMode("Group-Only") == MODE_GROUP_ONLY && ParseMode("x") == MODE_NONE);
    check("mode names round-trip", ParseMode(ModeName(MODE_BOTS)) == MODE_BOTS && ParseMode(ModeName(MODE_GROUP)) == MODE_GROUP
        && ParseMode(ModeName(MODE_GROUP_ONLY)) == MODE_GROUP_ONLY);
    JoinArgs j = ParseJoin("frozen-throne", "heroic", "group-only");
    check("join args", j.error.empty() && j.boss == ft && j.diff == DIFF_HEROIC && j.mode == MODE_GROUP_ONLY);
    j = ParseJoin("", "", "");
    check("join defaults: frozen-throne normal bots", j.error.empty() && j.boss == ft && j.diff == DIFF_NORMAL && j.mode == MODE_BOTS);
    check("join errors", !ParseJoin("nope", "normal", "bots").error.empty() && !ParseJoin("ft", "mythic", "bots").error.empty()
        && !ParseJoin("ft", "normal", "pets").error.empty());

    // roles and seats
    check("roles", RoleOf(1, 2) == ROLE_TANK && RoleOf(1, 0) == ROLE_DPS && RoleOf(2, 0) == ROLE_HEAL && RoleOf(2, 1) == ROLE_TANK
        && RoleOf(5, 1) == ROLE_HEAL && RoleOf(5, 2) == ROLE_DPS && RoleOf(6, 0) == ROLE_TANK && RoleOf(8, 0) == ROLE_DPS
        && RoleOf(11, 2) == ROLE_HEAL && RoleOf(7, 2) == ROLE_HEAL);
    std::vector<Seat> def = DefaultSeats();
    int tanks = 0, heals = 0, dps = 0;
    for (Seat const& s : def) (s.role == ROLE_TANK ? tanks : s.role == ROLE_HEAL ? heals : dps)++;
    check("default seats: 2 tanks, 3 healers, 5 damage", def.size() == 10 && tanks == 2 && heals == 3 && dps == 5);
    std::vector<Seat> b = BotSeats(def, { ROLE_DPS }, 10);
    check("one damage player: 9 bots, both tanks and all healers stay", b.size() == 9 && b[0].role == ROLE_TANK
        && b[1].role == ROLE_TANK && b[2].role == ROLE_HEAL);
    b = BotSeats(def, { ROLE_TANK }, 10);
    check("a tank player: one bot tank", b.size() == 9 && std::count_if(b.begin(), b.end(), [](Seat const& s) { return s.role == ROLE_TANK; }) == 1);
    b = BotSeats(def, { ROLE_TANK, ROLE_TANK, ROLE_TANK }, 10);
    check("three tanks: the third takes a damage seat", b.size() == 7
        && std::count_if(b.begin(), b.end(), [](Seat const& s) { return s.role == ROLE_TANK; }) == 0
        && std::count_if(b.begin(), b.end(), [](Seat const& s) { return s.role == ROLE_DPS; }) == 4);
    b = BotSeats(def, std::vector<int>(10, ROLE_DPS), 10);
    check("a full raid of players: no bots", b.empty());
    b = BotSeats(def, std::vector<int>(12, ROLE_HEAL), 10);
    check("more players than seats: no bots, no crash", b.empty());
    check("seats from config", ParseSeats("1:2:tank, 5:1:heal,8:2:dps").size() == 3 && ParseSeats("1:2:tank,5:1:heal").at(1).cls == 5);
    check("bad seats config: the default", ParseSeats("1:2:wizard").size() == 10 && ParseSeats("").size() == 10
        && ParseSeats("10:0:dps").size() == 10 && ParseSeats("x").size() == 10);

    // the lobby
    LobbyDecision d = DecideLobby(10, 10, 0, 180000, 1800000, false, true);
    check("lobby: a full raid starts at once", d.action == LOBBY_START);
    d = DecideLobby(3, 10, 60000, 180000, 1800000, false, true);
    check("lobby: waits, eta 120 s", d.action == LOBBY_WAIT && d.etaSec == 120);
    d = DecideLobby(3, 10, 180000, 180000, 1800000, false, true);
    check("lobby: after the wait, bots fill", d.action == LOBBY_START_FILL);
    d = DecideLobby(3, 10, 900000, 180000, 1800000, true, true);
    check("lobby: group-only keeps waiting, no estimate", d.action == LOBBY_WAIT && d.etaSec == -1);
    d = DecideLobby(3, 10, 1800000, 180000, 1800000, true, true);
    check("lobby: group-only expires", d.action == LOBBY_EXPIRE);
    d = DecideLobby(3, 10, 400000, 180000, 1800000, false, false);
    check("lobby: no bots on this realm means group-only", d.action == LOBBY_WAIT && d.etaSec == -1);
    check("lobby: empty", DecideLobby(0, 10, 0, 1, 1, false, true).action == LOBBY_WAIT);

    // roster and messages
    check("roster text", RosterText({ { "Orion", 6, false }, { "Bo:t,x", 2, true } }) == "Orion:deathknight:player,Botx:paladin:bot");
    check("Q", MsgQueued("frozen-throne", DIFF_NORMAL, MODE_GROUP, 3, 7, 120) == "Q\tfrozen-throne\tnormal\tgroup\t3\t7\t120");
    check("Q group-only", MsgQueued("frozen-throne", DIFF_HEROIC, MODE_GROUP_ONLY, 1, 9, -1) == "Q\tfrozen-throne\theroic\tgroup-only\t1\t9\t-1");
    check("S", MsgStarting("frozen-throne", DIFF_HEROIC) == "S\tfrozen-throne\theroic");
    check("T", MsgTimer(65000) == "T\t65000");
    check("R clear", MsgResult(true, 312345, 3) == "R\tclear\t312345\t3");
    check("R wipe", MsgResult(false, 1000, 0) == "R\twipe\t1000\t-");
    check("L", MsgLeft() == "L");
    check("messages fit an addon message (255 with the prefix)",
        std::strlen(ADDON_PREFIX) + 1 + MsgQueued("frozen-throne", 1, 2, 10, 10, 1800).size() < 255);
    check("time format", FormatMs(312345) == "5:12.3" && FormatMs(0) == "0:00.0" && FormatMs(3600000) == "60:00.0");

    std::printf("%d passed, %d failed\n", g_pass, g_fail);
    return g_fail ? 1 : 0;
}
