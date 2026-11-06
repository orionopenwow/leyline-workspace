// g++ -std=c++17 -Wall -Wextra -Werror -I../src test_qol_logic.cpp -o t && ./t   (no core needed)
#include "QolLogic.h"

#include <cstdio>

using namespace LeylineQol;

static int g_pass = 0, g_fail = 0;
static void check(char const* name, bool ok)
{
    if (ok) { ++g_pass; std::printf("PASS %s\n", name); }
    else { ++g_fail; std::printf("FAIL %s\n", name); }
}

// A small made-up area tree in the shape of the real one (the ids of the real sub-areas don't matter here):
// 4395 the city (map 571) > 100 a street > 101 an inn inside it; 102 the landing (already allows duels);
// 103 the sewers; 2817 the forest (map 571, already allows duels) > 200 a camp; 4378 an arena zone on map 617;
// 65 another zone (map 571) > 300 an area in it.
static std::vector<AreaRow> Rows()
{
    return {
        { 4395, 571, 0, 0x0800 | 0x0100 },
        { 100, 571, 4395, 0x0800 },
        { 101, 571, 100, 0x0800 },
        { 102, 571, 4395, 0x0800 | FLAG_ALLOW_DUELS },
        { 103, 571, 4395, 0 },
        { 2817, 571, 0, FLAG_ALLOW_DUELS },
        { 200, 571, 2817, 0 },
        { 4378, 617, 0, 0 },
        { 500, 617, 4378, 0 },
        { 65, 571, 0, FLAG_ALLOW_DUELS },
        { 300, 571, 65, 0 },
    };
}

static uint32_t Parent(uint32_t id)
{
    for (AreaRow const& r : Rows())
        if (r.id == id)
            return r.zone;
    return 0;
}

static bool Same(std::vector<uint32_t> const& a, std::vector<uint32_t> const& b) { return a == b; }

int main()
{
    // ParseIdList
    check("parse: commas and spaces", Same(ParseIdList("4395, 2817"), { 4395, 2817 }));
    check("parse: semicolons, tabs, extra separators", Same(ParseIdList(" 4395;;\t2817 , "), { 4395, 2817 }));
    check("parse: empty", ParseIdList("").empty() && ParseIdList(" , ").empty());
    check("parse: junk tokens and zero are skipped", Same(ParseIdList("43x5, 0, -1, abc, 4395"), { 4395 }));
    check("parse: duplicates once", Same(ParseIdList("4395,4395,2817"), { 4395, 2817 }));
    check("parse: too large is skipped", Same(ParseIdList("99999999999, 7"), { 7 }));
    check("parse: the defaults", Same(ParseIdList("4395,2817"), { ZONE_DALARAN, ZONE_CRYSTALSONG }));

    // InAreas: the zone itself, its sub-areas at any depth, not other zones
    std::vector<uint32_t> city{ ZONE_DALARAN };
    check("in: the city zone", InAreas(4395, city, Parent));
    check("in: a street", InAreas(100, city, Parent));
    check("in: an inn inside a street (two levels)", InAreas(101, city, Parent));
    check("in: the landing and the sewers", InAreas(102, city, Parent) && InAreas(103, city, Parent));
    check("not in: the forest, unless listed", !InAreas(2817, city, Parent) && !InAreas(200, city, Parent));
    check("in: the forest when listed", InAreas(200, { ZONE_DALARAN, ZONE_CRYSTALSONG }, Parent));
    check("not in: another zone", !InAreas(300, city, Parent) && !InAreas(65, city, Parent));
    check("not in: area 0 or an empty list", !InAreas(0, city, Parent) && !InAreas(100, {}, Parent));
    check("in: a single area may be listed alone", InAreas(101, { 100 }, Parent) && !InAreas(103, { 100 }, Parent));
    check("a parent loop ends", !InAreas(7, { 9 }, [](uint32_t id) { return id == 7 ? 8u : 7u; }));

    // InstantLogout
    LogoutAsk ok;
    ok.enabled = ok.inWorld = ok.onMap = ok.inArea = true;
    check("logout: in the city, calm: instant", InstantLogout(ok));
    LogoutAsk a = ok; a.enabled = false;   check("logout: setting off: the core", !InstantLogout(a));
    a = ok; a.onMap = false;               check("logout: other map (the arena): the core", !InstantLogout(a));
    a = ok; a.inArea = false;              check("logout: outside the areas: the core", !InstantLogout(a));
    a = ok; a.inWorld = false;             check("logout: not in world: the core", !InstantLogout(a));
    a = ok; a.inCombat = true;             check("logout: in combat: the core (it refuses)", !InstantLogout(a));
    a = ok; a.falling = true;              check("logout: falling: the core (it refuses)", !InstantLogout(a));
    a = ok; a.dueling = true;              check("logout: in a duel: the core (it refuses)", !InstantLogout(a));
    a = ok; a.frozen = true;               check("logout: GM freeze: the core (it refuses)", !InstantLogout(a));
    a = ok; a.afkBlocked = true;           check("logout: AFK rule: the core (it refuses)", !InstantLogout(a));

    // DuelAreas: every row under the city on map 571 that lacks the flag, nothing else
    std::vector<AreaRow> rows = Rows();
    check("duels: the city's rows that lack the flag", Same(DuelAreas(rows, city, MAP_NORTHREND), { 4395, 100, 101, 103 }));
    check("duels: the landing already has it (left alone)", !Listed(DuelAreas(rows, city, MAP_NORTHREND), 102));
    check("duels: other zones untouched", !Listed(DuelAreas(rows, city, MAP_NORTHREND), 300)
        && !Listed(DuelAreas(rows, city, MAP_NORTHREND), 200));
    check("duels: the forest when listed (its camp only; the zone has it)",
        Same(DuelAreas(rows, { ZONE_DALARAN, ZONE_CRYSTALSONG }, MAP_NORTHREND), { 4395, 100, 101, 103, 200 }));
    check("duels: another map's rows never (the arena, map 617)", DuelAreas(rows, { 4378 }, MAP_NORTHREND).empty()
        && Same(DuelAreas(rows, { 4378 }, 617), { 4378, 500 }));
    check("duels: empty list: nothing", DuelAreas(rows, {}, MAP_NORTHREND).empty());

    check("join", JoinIds({ 4395, 2817 }) == "4395,2817" && JoinIds({}).empty());

    std::printf("%d passed, %d failed\n", g_pass, g_fail);
    return g_fail ? 1 : 0;
}
