// g++ -std=c++17 -Wall -Wextra -Werror -I../src test_bossfights_heroes.cpp -o th && ./th   (no core needed)
// Boss fight heroes, the pure parts (src/BossfightsHeroLogic.h and the generated src/BossfightsHeroData.h).
#include "BossfightsHeroLogic.h"

#include <cstdio>
#include <set>

using namespace LeylineBF;

static int g_pass = 0, g_fail = 0;
static void check(char const* name, bool ok)
{
    if (ok) { ++g_pass; std::printf("PASS %s\n", name); }
    else { ++g_fail; std::printf("FAIL %s\n", name); }
}

static std::vector<std::uint8_t> const CLASSES = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 11 };

int main()
{
    // the hero account
    check("hero account: the account name and BF", HeroAccountName("wr000123") == "WR000123BF");
    check("hero account: 14 letters still fit", HeroAccountName("ABCDEFGHIJKLMN") == "ABCDEFGHIJKLMNBF");
    std::string longName = HeroAccountName("ABCDEFGHIJKLMNO");
    check("hero account: a long name hashes to BF + 14 hex", longName.size() == 16 && longName.rfind("BF", 0) == 0
        && longName != HeroAccountName("ABCDEFGHIJKLMNP") && longName == HeroAccountName("abcdefghijklmno"));
    check("hero account: nothing for a bad name", HeroAccountName("") == "" && HeroAccountName("a b") == ""
        && HeroAccountName("ABCDEFGHIJKLMNOPQ") == "" && HeroAccountName("x;DROP") == "");
    check("hero account: always a plain account name", PlainAccountName(HeroAccountName("WR1")) && PlainAccountName(longName));

    // classes and specs
    check("classes by word", ParseClass("warrior") == 1 && ParseClass("Death Knight") == 6 && ParseClass("death-knight") == 6
        && ParseClass("DK") == 6 && ParseClass("druid") == 11 && ParseClass("8") == 8);
    check("classes: unknown", ParseClass("monk") == 0 && ParseClass("10") == 0 && ParseClass("") == 0 && ParseClass("12") == 0);
    bool everyClass = true, everyPartner = true, kitsExist = true, threeOrFour = true;
    for (std::uint8_t c : CLASSES)
    {
        int n = 0;
        for (SpecDef const& s : Specs())
            if (s.cls == c)
            {
                ++n;
                SpecDef const* p = PartnerSpec(s);
                everyPartner = everyPartner && p && p != &s && p->cls == c;
                kitsExist = kitsExist && KitFor(c, s.kit) != nullptr;
            }
        threeOrFour = threeOrFour && (n == 3 || (c == 11 && n == 4));
        everyClass = everyClass && DefaultSpec(c) != nullptr;
    }
    check("every class has its specs (druid 4)", threeOrFour);
    check("every class has a default spec", everyClass);
    check("every spec has a different second spec of its class", everyPartner);
    check("every spec has its kit in ov_kits.lua", kitsExist);
    check("spec aliases", FindSpec(1, "Protection") == FindSpec(1, "prot") && FindSpec(6, "prot") == FindSpec(6, "blood")
        && FindSpec(11, "feral") == FindSpec(11, "cat") && FindSpec(11, "prot") == FindSpec(11, "bear")
        && FindSpec(3, "beast mastery") == FindSpec(3, "bm") && FindSpec(5, "healer") == FindSpec(5, "disc"));
    check("spec by number", FindSpec(8, "2") == FindSpec(8, "fire") && FindSpec(11, "4") == FindSpec(11, "cat"));
    check("spec: wrong class", FindSpec(8, "prot") == nullptr && FindSpec(4, "healer") == nullptr && FindSpec(1, "fire") == nullptr);
    check("premade spec numbers match realm/playerbots-specs.env", FindSpec(11, "cat")->pbSpec == 3 && FindSpec(11, "bear")->pbSpec == 1
        && FindSpec(6, "unholy")->pbSpec == 2 && FindSpec(2, "ret")->pbSpec == 2 && FindSpec(9, "destro")->pbSpec == 2);
    check("roles", FindSpec(1, "prot")->role == ROLE_TANK && FindSpec(5, "disc")->role == ROLE_HEAL && FindSpec(8, "fire")->role == ROLE_DPS
        && RoleOf(11, FindSpec(11, "resto")->tab) == ROLE_HEAL && RoleOf(2, FindSpec(2, "prot")->tab) == ROLE_TANK);
    bool races = true;
    for (std::uint8_t c : CLASSES)
        races = races && DefaultRace(c, false) && DefaultRace(c, true);
    check("a default race for every class and faction", races && DefaultRace(7, false) == 11 && DefaultRace(11, true) == 6
        && DefaultRace(2, true) == 10);
    check("faction words", ParseFaction("Horde") == 1 && ParseFaction("alliance") == 0 && ParseFaction("x") == -1);

    // names
    std::set<std::string> names;
    bool allowed = true;
    for (std::uint32_t a = 0; a < 200; ++a)
    {
        std::string n = HeroName(12345, a);
        allowed = allowed && HeroNameAllowed(n);
        names.insert(n);
    }
    check("generated names: 2-12 letters, capitalised, no brand word", allowed);
    check("generated names: many different ones", names.size() > 120);
    check("generated names: the same seed and attempt give the same name", HeroName(7, 3) == HeroName(7, 3));
    check("name rules", HeroNameAllowed("Orion") && !HeroNameAllowed("orion") && !HeroNameAllowed("O") && !HeroNameAllowed("Or1on")
        && !HeroNameAllowed("Abcdefghijklm") && !HeroNameAllowed("Wowza") && !HeroNameAllowed("Lichy"));

    // kits and bags
    HeroKit const* fury = KitFor(1, "fury");
    check("the fury kit", fury && fury->pve.size() >= 15 && !fury->pvp.empty());
    auto bags = HeroBagItems(1, "fury", false);
    std::map<std::uint32_t, std::uint32_t> bag(bags.begin(), bags.end());
    bool noneWorn = true;   // a piece of the worn kit is packed only when another kit wants more copies of it
    for (std::uint32_t id : fury->pve)
    {
        long worn = std::count(fury->pve.begin(), fury->pve.end(), id), most = 0;
        for (HeroKit const& k : HeroKits())
            if (k.cls == 1 && std::string(k.key) != "fury")
                most = std::max(most, (long)std::count(k.pve.begin(), k.pve.end(), id));
        noneWorn = noneWorn && (bag.count(id) ? long(bag[id]) <= std::max(0L, most - worn) || most > worn : true);
    }
    check("bags: the worn kit isn't packed again", noneWorn && !bag.count(fury->pve[0]));
    check("bags: the other PvE kits are packed", bag.count(KitFor(1, "prot")->pve[0]) == 1);
    check("bags: the PvP kits and the faction's medallion", bag.count(KitFor(1, "arms")->pvp[0]) == 1
        && bag.count(HERO_MEDALLION_ALLIANCE) == 1 && bag.count(HERO_MEDALLION_HORDE) == 0);
    auto hbags = HeroBagItems(1, "fury", true);
    check("bags: the horde medallion for horde", hbags.size() == bags.size()
        && std::map<std::uint32_t, std::uint32_t>(hbags.begin(), hbags.end()).count(HERO_MEDALLION_HORDE));
    std::size_t most = 0;
    for (std::uint8_t c : CLASSES)
        for (SpecDef const& s : Specs())
            if (s.cls == c)
            {
                std::size_t slots = 0;
                for (auto const& kv : HeroBagItems(c, s.kit, true))
                    slots += kv.second;
                slots += HeroSupplies(c, KitFor(c, s.kit)->role).size();
                most = std::max(most, slots);
            }
    check("bags: everything fits in the backpack and four 20-slot bags (96)", most <= 96);
    std::printf("     (the fullest hero needs %zu of 96 slots)\n", most);
    check("supplies: a flask for the role and food", FlaskFor("tank") == FLASK_STONEBLOOD && FlaskFor("healer") == FLASK_FROST_WYRM
        && FlaskFor("str") == FLASK_ENDLESS_RAGE && HeroSupplies(8, "caster").back().first == FISH_FEAST);
    check("supplies: the shaman's totems come along", HeroSupplies(7, "enh").size() >= 7);

    // bars
    auto ret = BarLayout(2, "ret");
    check("bars: the spec's main bar", ret.count(0) && ret[0][0] == 35395);
    check("bars: the class rows stay", ret.count(60) && ret.count(48) && ret.count(36));
    check("bars: alternatives kept (faction seals)", ret[49].size() == 2);
    check("bars: the PvP medallion marker", ret[71].size() == 1 && ret[71][0] == BAR_PVP);
    auto fire = BarLayout(8, "fire");
    check("bars: mage has one layout for every spec", fire.count(0) && fire[0][0] == 133 && fire.count(12));
    check("bars: never past the last button", BarLayout(11, "cat").rbegin()->first < 144);
    check("bars: a spec row replaces the class row", BarLayout(11, "cat")[72][0] == 33876 && BarLayout(11, "balance")[72][0] == 1082);

    // equipment slots
    check("slots: rings and trinkets have two", EquipSlotsFor(11, false, false).size() == 2 && EquipSlotsFor(12, false, false).size() == 2);
    check("slots: a one-hander goes to the off hand only with dual wield", EquipSlotsFor(13, false, false).size() == 1
        && EquipSlotsFor(13, true, false).size() == 2);
    check("slots: a two-hander in the off hand only with titan's grip", EquipSlotsFor(17, true, false).size() == 1
        && EquipSlotsFor(17, true, true).size() == 2);
    check("slots: relics and ranged", EquipSlotsFor(28, false, false) == std::vector<std::uint8_t>{ 17 } && EquipSlotsFor(15, false, false)[0] == 17);
    check("slots: bags and ammo are not equipment here", EquipSlotsFor(18, false, false).empty() && EquipSlotsFor(24, false, false).empty());
    check("instance mode mask", InstanceModeMask(DIFF_NORMAL) == 0 && InstanceModeMask(DIFF_HEROIC) == 0x20);

    // the console command
    HeroPick p = ParseHeroPick({ "mage", "fire" });
    check("pick: class and spec, second spec by default", p.error.empty() && p.cls == 8 && p.spec == FindSpec(8, "fire") && p.spec2 == FindSpec(8, "frost"));
    p = ParseHeroPick({ "druid", "bear", "resto", "horde" });
    check("pick: second spec and faction", p.error.empty() && p.spec2 == FindSpec(11, "resto") && p.faction == 1);
    p = ParseHeroPick({ "horde", "priest" });
    check("pick: faction first, default spec", p.error.empty() && p.cls == 5 && p.spec == DefaultSpec(5) && p.faction == 1);
    p = ParseHeroPick({ "paladin", "holy", "holy" });
    check("pick: the same spec twice gets its partner", p.spec2 == FindSpec(2, "ret"));
    check("pick: nothing = keep the last pick", ParseHeroPick({}).cls == 0 && ParseHeroPick({}).error.empty());
    check("pick: errors", !ParseHeroPick({ "monk" }).error.empty() && !ParseHeroPick({ "mage", "holy" }).error.empty()
        && !ParseHeroPick({ "mage", "fire", "frost", "arcane" }).error.empty());
    SpecDef const& f = *FindSpec(8, "fire");
    check("reply: ok", ReplyHeroOk("Kalin", 8, f, *FindSpec(8, "frost"), true, "WR1BF") == "bfhero ok Kalin mage fire frost made WR1BF");
    check("reply: no, plain words, one line", ReplyHeroNo("no\tsuch\naccount") == "bfhero no nosuchaccount");
    check("reply: list line", ReplyHeroLine("Kalin", 6, "blood", "frost", true) == "bfhero hero Kalin deathknight blood frost current");
    BossDef const* ft = FindBoss("frozen-throne");
    check("go text", HeroGo(*ft, DIFF_HEROIC, MODE_GROUP_ONLY) == "bf:frozen-throne:heroic:group-only");
    JoinArgs g = ParseHeroGo("bf:frozen-throne:heroic:group");
    check("go text back", g.error.empty() && g.boss == ft && g.diff == DIFF_HEROIC && g.mode == MODE_GROUP);
    check("go text: bad ones", !ParseHeroGo("arena:1v1").error.empty() && !ParseHeroGo("bf:frozen-throne:normal").error.empty()
        && !ParseHeroGo("bf:onyxia:normal:bots").error.empty() && !ParseHeroGo("bf:frozen-throne:mythic:bots").error.empty());

    std::printf("%d passed, %d failed\n", g_pass, g_fail);
    return g_fail ? 1 : 0;
}
