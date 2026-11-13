// g++ -std=c++17 -Wall -Wextra -Werror -I../src test_bossfights_logic.cpp -o t && ./t   (no core needed)
#include "BossfightsLogic.h"

#include <cstdio>
#include <cmath>
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
    check("FindBossByMap: the featured fight first on its map", FindBossByMap(631) == ft && FindBossByMap(0) == nullptr && FindBossByMap(249) == FindBoss("brood-mother"));
    bool clean = true;
    for (BossDef const& b : Bosses())
        clean = clean && !hasBrandWord(b.title) && !hasBrandWord(b.key);
    check("no brand words in titles or keys", clean);

    // Boss Rush's catalog (docs/BOSS-RUSH.md): every fight lands inside its own fence, on its own map, with no
    // brand word; keys are unique; the citadel's three fights are told apart by their boss id
    bool fenced = true, unique = true, sized = true;
    for (BossDef const& b : Bosses())
    {
        fenced = fenced && InsideFence(b, b.x, b.y, b.z) && (!HasFence(b) || b.z > b.fenceMinZ)
            && (!b.hx || InsideFence(b, b.hx, b.hy, b.hz));
        sized = sized && b.size == 10 && b.mapId != 0 && b.bossEntry != 0 && b.zone != 0 && b.startHint && *b.startHint;
        for (BossDef const& c : Bosses())
            if (&b != &c && std::string(b.key) == c.key)
                unique = false;
        clean = clean && !hasBrandWord(b.raid) && !hasBrandWord(b.startHint);
    }
    check("catalog: at least 10 fights", Bosses().size() >= 10);
    check("catalog: everyone lands inside the fence", fenced);
    check("catalog: unique keys", unique);
    check("catalog: 10 seats, a map, a boss, a zone and a start hint each", sized);
    check("catalog: no brand words in raid names or hints", clean);
    check("the featured fight stays first", &Bosses().front() == ft);
    check("FindBossOnMap tells the citadel's fights apart", FindBossOnMap(631, 12) == ft && FindBossOnMap(631, 0) == FindBoss("bone-lord")
        && FindBossOnMap(631, 1) == FindBoss("cult-mistress") && FindBossOnMap(631, 7) == FindBoss("icc-council")
        && FindBossOnMap(631, 14) == nullptr && FindBossOnMap(724, 6) == FindBoss("twilight-destroyer"));
    check("Halion's intro goes through the script (Zarithrian, 1), the others quietly", FindBoss("twilight-destroyer")->scripted == std::vector<std::uint32_t>{ 1 }
        && FindBoss("twilight-destroyer")->prereq == std::vector<std::uint32_t>({ 0, 2 }));
    check("the solo Sanctum leaves the drakes out; the three-drake one keeps them", FindBoss("obsidian-sanctum")->prereq.size() == 3
        && FindBoss("obsidian-sanctum")->despawn.size() == 3 && FindBoss("obsidian-three")->prereq.empty() && FindBoss("obsidian-three")->despawn.empty());
    check("only the featured fight is offered without AllFights", Offered(*ft, false) && !Offered(*FindBoss("bone-lord"), false)
        && Offered(*FindBoss("bone-lord"), true));
    check("join: another fight needs AllFights", !ParseJoin("bone-lord", "normal", "bots", false).error.empty()
        && ParseJoin("bone-lord", "heroic", "bots", true).error.empty());
    check("join: heroic only where the raid has one", !ParseJoin("brood-mother", "heroic", "bots").error.empty()
        && ParseJoin("brood-mother", "normal", "bots").error.empty() && ParseJoin("twilight-destroyer", "heroic", "bots").error.empty());

    // arguments
    check("difficulty", ParseDifficulty("normal") == DIFF_NORMAL && ParseDifficulty("HEROIC") == DIFF_HEROIC
        && ParseDifficulty("10h") == DIFF_HEROIC && ParseDifficulty("25") == DIFF_NORMAL && ParseDifficulty("heroic25") == DIFF_HEROIC
        && ParseDifficulty("mythic") == DIFF_NONE && ParseDifficulty("") == DIFF_NONE);
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


    // ---- start anywhere (BossfightsStarts.h; docs/BOSS-RUSH.md "Start anywhere") ----------------------------------
    {
        // every citadel boss has a start (the gauntlet and the ambush are steps, not starts)
        char const* icc[] = { "bone-lord", "cult-mistress", "icc-gunship", "icc-deathbringer", "icc-bloat", "icc-ooze",
                              "icc-professor", "icc-council", "icc-blood-queen", "icc-dreamwalker", "icc-frost-queen", "frozen-throne" };
        std::uint32_t ids[] = { 0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12 };
        bool all = true;
        for (int i = 0; i < 12; ++i)
            all = all && FindBoss(icc[i]) && FindBoss(icc[i])->bossData == ids[i] && FindBoss(icc[i])->mapId == 631 && !IsPrereq(*FindBoss(icc[i]), ids[i]);
        check("start anywhere: every citadel boss is a start, on its own boss id, never its own prerequisite", all);
        // each boss's earlier bosses: the instance script's checks (CheckLowerSpire / Plagueworks / CrimsonHalls / FrostwingHalls)
        auto has = [](char const* k, std::vector<std::uint32_t> want) {
            BossDef const* b = FindBoss(k);
            if (!b) return false;
            for (std::uint32_t w : want) if (!IsPrereq(*b, w)) return false;
            return true;
        };
        check("start anywhere: earlier bosses as the raid script checks them",
            has("icc-deathbringer", { 0, 1, 2 }) && has("icc-bloat", { 0, 1, 2, 3 }) && has("icc-professor", { 0, 1, 2, 3, 4, 5 })
            && has("icc-council", { 0, 1, 2, 3, 14 }) && has("icc-blood-queen", { 0, 1, 2, 3, 14, 7 })
            && has("icc-dreamwalker", { 0, 1, 2, 3, 9 }) && has("icc-frost-queen", { 0, 1, 2, 3, 9, 10, 13 })
            && FindBoss("bone-lord")->prereq.empty());
        check("start anywhere: the lab's door (trap event done), the wyrms killed for the queen, the ships brought in",
            FindBoss("icc-professor")->data.size() == 1 && FindBoss("icc-professor")->data[0] == std::make_pair(254u, 3u)
            && FindBoss("icc-frost-queen")->kill == std::vector<std::uint32_t>({ 37533, 37534 })
            && FindBoss("icc-gunship")->scripted == std::vector<std::uint32_t>{ 1 } && FindBoss("icc-gunship")->hx != 0.0f
            && !HasFence(*FindBoss("icc-gunship")));
        // the starts stand outside their rooms: each one farther from its boss than the door is (the boss's spot is the fence's middle)
        bool outside = true;
        for (char const* k : { "bone-lord", "icc-bloat", "icc-ooze", "icc-professor", "icc-council", "icc-blood-queen", "icc-dreamwalker", "icc-frost-queen", "icc-deathbringer" })
        {
            BossDef const* b = FindBoss(k);
            float dx = b->x - b->fenceX, dy = b->y - b->fenceY;
            outside = outside && dx * dx + dy * dy > 40.0f * 40.0f && InsideFence(*b, b->x, b->y, b->z);
        }
        check("start anywhere: the bosses' starts are outside their rooms (40+ yards) and inside the fence", outside);
        // facing the boss: the start's orientation points at the fence's middle (within 10 degrees)
        bool facing = true;
        for (BossDef const& b : Bosses())
        {
            if (!HasFence(b) || std::string(b.key) == "frozen-throne" || std::string(b.key) == "spell-weaver")
                continue;
            float want = std::atan2(b.fenceY - b.y, b.fenceX - b.x);
            float d = std::fabs(std::remainder(want - b.o, 6.2831853f));
            facing = facing && d < 0.18f;
            if (d >= 0.18f) std::printf("     facing: %s %.2f\n", b.key, d);
        }
        check("start anywhere: everyone lands facing the boss", facing);

        // the tree
        RaidDef const& r = Raids().front();
        check("raids: the citadel first, with its five wings and its gate", std::string(r.key) == "icc" && r.wings.size() == 5
            && r.areatrigger == 5670 && std::string(r.whole) == "icc-raid");
        bool fightsKnown = true, keysOk = true, clean2 = true;
        for (RaidDef const& rr : Raids())
        {
            for (char const* f : rr.fights)
                fightsKnown = fightsKnown && FindFight(f) && FindFight(f)->mapId == rr.mapId;
            for (WingDef const& w : rr.wings)
            {
                fightsKnown = fightsKnown && FindFight(w.from);
                for (StepDef const& st : w.steps)
                    fightsKnown = fightsKnown && (!*st.fight || FindFight(st.fight));
                keysOk = keysOk && std::strlen(w.key) <= 17;
                clean2 = clean2 && !hasBrandWord(w.title) && !hasBrandWord(w.hint);
            }
            keysOk = keysOk && std::strlen(rr.whole) <= 17;
            clean2 = clean2 && !hasBrandWord(rr.title) && !hasBrandWord(rr.wholeHint);
        }
        std::size_t inRaids = 0;
        for (RaidDef const& rr : Raids())
            inRaids += rr.fights.size();
        check("raids: every fight is in exactly one raid's list, on its map", fightsKnown && inRaids == Bosses().size());
        check("raids: keys fit a hero's waiting go, no brand words", keysOk && clean2);
        // the go text of the longest key and difficulty fits the table's column (64)
        std::size_t longest = 0;
        for (BossDef const& b : Bosses()) longest = std::max(longest, std::strlen(b.key));
        for (BossDef const& b : Scopes()) longest = std::max(longest, std::strlen(b.key));
        check("a waiting go fits its column", 3 + longest + 1 + 8 + 1 + 10 <= 64);

        // the scopes
        BossDef const* spire = FindBoss("icc-spire");
        BossDef const* plague = FindBoss("icc-plague");
        BossDef const* frost = FindBoss("icc-frostwing");
        BossDef const* raid = FindBoss("icc-raid");
        check("scopes: four citadel wings and three whole raids", Scopes().size() == 7 && spire && plague && frost && FindBoss("icc-crimson")
            && raid && FindBoss("rs-raid") && FindBoss("voa-raid") && !FindBoss("os-raid"));
        check("scopes: a wing starts at its first boss with its earlier bosses (and the ships) done, ends on its last",
            spire->scope == SCOPE_WING && spire->x == FindBoss("bone-lord")->x && IsPrereq(*spire, 2) && !IsPrereq(*spire, 0)
            && spire->bossData == 3 && spire->chain == std::vector<std::uint32_t>({ 0, 1, 3 }) && !HasFence(*spire)
            && plague->prereq == FindBoss("icc-bloat")->prereq && plague->bossData == 6 && plague->data.empty()
            && frost->chain == std::vector<std::uint32_t>({ 10, 13, 11 }) && IsPrereq(*frost, 9) && frost->kill.empty());
        check("scopes: the whole raid from the gate, ending on the final fight's end cast",
            raid->scope == SCOPE_RAID && raid->x == r.ex && raid->prereq == std::vector<std::uint32_t>{ 2 } && raid->bossData == 12
            && raid->endSpell == 72350 && raid->bossEntry == 36597 && raid->chain.size() == 13 && raid->chain.back() == 12
            && InChain(*raid, 9) && !InChain(*raid, 2) && raid->scripted.empty() && raid->heroic);
        check("scopes: the sanctum's whole raid keeps its intro to the script (no quiet prerequisites)",
            FindBoss("rs-raid")->prereq.empty() && FindBoss("rs-raid")->scripted.empty() && FindBoss("rs-raid")->bossData == 6);
        check("scopes: only with AllFights, and joinable", !ParseJoin("icc-raid", "normal", "bots", false).error.empty()
            && ParseJoin("icc-plague", "heroic25", "group", true).error.empty() && ParseJoin("icc-plague", "heroic25", "group", true).size == 25
            && ParseJoin("voa-raid", "heroic", "bots", true).error == "Vault of Archavon has no heroic: play it on normal");

        // retry: the first boss still standing, else the nearest earlier step with a start
        std::vector<std::uint32_t> down;
        auto done = [&down](std::uint32_t id) { return std::find(down.begin(), down.end(), id) != down.end(); };
        check("retry: a wing's first boss when nothing is down", RetrySpot(*plague, done) == FindBoss("icc-bloat"));
        down = { 4 };
        check("retry: the next boss standing", RetrySpot(*plague, done) == FindBoss("icc-ooze"));
        down = { 10 };
        check("retry: the gauntlet (no start) goes back to the dreamer's", RetrySpot(*frost, done) == FindBoss("icc-dreamwalker"));
        down = { 10, 13 };
        check("retry: then the queen", RetrySpot(*frost, done) == FindBoss("icc-frost-queen"));
        down = {};
        check("retry: the sanctum's lesser dragons have no starts: the run's own (the door)", RetrySpot(*FindBoss("rs-raid"), done) == nullptr);
        check("retry: a single fight stays at its own start", RetrySpot(*FindBoss("icc-council"), done) == nullptr);
        down = { 0, 1, 3, 4, 5, 6, 7, 8, 9, 10, 13, 11 };
        check("retry: the raid's last step: the throne", RetrySpot(*raid, done) == FindBoss("frozen-throne"));
        check("progress: done of the chain", Progress(*raid, done) == std::make_pair(std::size_t(12), std::size_t(13))
            && Progress(*FindBoss("icc-council"), done) == std::make_pair(std::size_t(1), std::size_t(1)));

        // sizes and seats
        check("size: 25 from its words, else 10", ParseSize("heroic25") == 25 && ParseSize("25") == 25 && ParseSize("10h") == 10 && ParseSize("heroic") == 10
            && DiffToken(DIFF_HEROIC, 25) == "heroic25" && DiffToken(DIFF_NORMAL, 10) == "normal");
        check("size: the core's raid difficulty", RaidDifficultyIndex(DIFF_NORMAL, 10) == 0 && RaidDifficultyIndex(DIFF_NORMAL, 25) == 1
            && RaidDifficultyIndex(DIFF_HEROIC, 10) == 2 && RaidDifficultyIndex(DIFF_HEROIC, 25) == 3);
        RoleCount c25 = CountRoles(DefaultSeats25()), c10 = CountRoles(DefaultSeats());
        check("seats: 25 is 3 tanks, 6 healers, 16 damage; 10 is 2, 3, 5", DefaultSeats25().size() == 25 && c25.tank == 3 && c25.heal == 6 && c25.dps == 16
            && c10.tank == 2 && c10.heal == 3 && c10.dps == 5);
        std::vector<Seat> b25 = BotSeats(SeatsFor(25, DefaultSeats()), { ROLE_HEAL }, 25);
        RoleCount cb = CountRoles(b25);
        check("seats: a healer and 24 companions", b25.size() == 24 && cb.heal == 5 && cb.tank == 3 && cb.dps == 16);
        check("seats: two friends (a tank and damage) and 8 companions", BotSeats(DefaultSeats(), { ROLE_TANK, ROLE_DPS }, 10).size() == 8);

        // the playtest's marks and status line
        check("phase marks: the final fight's transitions and its end", PhaseMarks(36597).size() == 4 && PhaseMarks(36597).back().spell == 72350
            && PhaseMarks(36612).empty());
        RushState st;
        st.key = "frozen-throne"; st.diff = "normal"; st.phase = "ready"; st.leader = "Kalinra"; st.map = 631; st.instance = 7;
        st.x = 517.0f; st.y = -2124.6f; st.z = 840.9f; st.dist = 1.2f; st.members = 10; st.bots = 9; st.inside = 10; st.step = "frozen-throne";
        st.done = 0; st.of = 1; st.marks = "transition1,quake"; st.ms = 1234;
        std::string line = RushLine(st);
        check("status line", line == "bfrush run key=frozen-throne diff=normal phase=ready leader=Kalinra map=631 inst=7 x=517.0 y=-2124.6 z=840.9 dist=1.2 "
            "members=10 bots=9 inside=10 step=frozen-throne done=0/1 boss=0 wipes=0 clears=0 marks=transition1,quake last=- ms=1234");
        st.leader = "a b=c";
        check("status line: a value never breaks the words", RushLine(st).find("leader=a_b_c ") != std::string::npos);
    }

    std::printf("%d passed, %d failed\n", g_pass, g_fail);
    return g_fail ? 1 : 0;
}
