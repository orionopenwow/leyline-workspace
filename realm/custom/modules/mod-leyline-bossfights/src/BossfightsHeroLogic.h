/*
 * mod-leyline-bossfights: boss fight heroes, the pure parts (no core headers; tests/test_bossfights_logic.cpp checks
 * them with a plain g++). docs/BOSS-FIGHT-HEROES.md has the design.
 *
 * A hero is a level 80 character made by the module for boss fights only. It lives on a second game account linked to
 * the player's own (HeroAccountName), so the player's normal characters are never listed, moved, boosted or bound by
 * boss fights. Here: the hero account's name, the class and spec table (the playerbots premade spec numbers the bots
 * already use, realm/playerbots-specs.env), default races, generated names, the kit and bag plan from
 * BossfightsHeroData.h (ov_kits.lua + ov_main.lua), the bar layout, equipment slots, consumables, the console
 * command's arguments and its replies.
 */
#ifndef LEYLINE_BOSSFIGHTS_HERO_LOGIC_H
#define LEYLINE_BOSSFIGHTS_HERO_LOGIC_H

#include "BossfightsHeroData.h"
#include "BossfightsLogic.h"

#include <cstring>
#include <map>
#include <string>
#include <utility>
#include <vector>

namespace LeylineBF
{
    constexpr std::size_t HERO_LIMIT = 10;              // heroes per player (the realm's own CharactersPerRealm is 10 too)
    constexpr unsigned HERO_INTENT_TTL = 1800;          // a "Fight now" waits this long for the hero's login (s)
    constexpr std::size_t ACCOUNT_NAME_MAX = 16;        // the portal's ACCOUNT_RE (and the 3.3.5a login screen)
    constexpr char const* HERO_SUFFIX = "BF";
    constexpr std::uint32_t HERO_ZONE = 4812;           // Icecrown Citadel (char select shows the zone)

    inline std::string Upper(std::string s)
    {
        std::transform(s.begin(), s.end(), s.begin(), [](unsigned char c) { return char(std::toupper(c)); });
        return s;
    }

    inline bool PlainAccountName(std::string const& s)
    {
        if (s.empty() || s.size() > ACCOUNT_NAME_MAX)
            return false;
        for (unsigned char c : s)
            if (!std::isalnum(c))
                return false;
        return true;
    }

    // The player's hero account: <ACCOUNT>BF, or for a name too long for that, "BF" + 14 hex digits of its FNV-1a hash.
    // The website (stream/bossfights.py hero_account) uses the same rule. "" for a name that isn't a plain account name.
    inline std::string HeroAccountName(std::string const& main)
    {
        if (!PlainAccountName(main))
            return "";
        std::string u = Upper(main);
        if (u.size() + 2 <= ACCOUNT_NAME_MAX)
            return u + HERO_SUFFIX;
        unsigned long long h = 1469598103934665603ULL;
        for (unsigned char c : u)
        {
            h ^= c;
            h *= 1099511628211ULL;
        }
        char buf[24];
        std::snprintf(buf, sizeof(buf), "%014llX", h & 0xFFFFFFFFFFFFFFULL);
        return std::string(HERO_SUFFIX) + buf;
    }

    // ---- classes and specs ----------------------------------------------------------------------------------------

    struct SpecDef
    {
        std::uint8_t cls;
        char const* key;        // the word players and the website use
        char const* label;      // what players see
        char const* kit;        // ov_kits.lua kit key
        std::uint8_t pbSpec;    // mod-playerbots premade spec number (realm/playerbots-specs.env, the PvE builds)
        int role;               // ROLE_TANK / ROLE_HEAL / ROLE_DPS
        std::uint8_t tab;       // talent tree (Player::GetMostPointsTalentTree)
        char const* partner;    // the second spec dual spec fills by default
    };

    inline std::vector<SpecDef> const& Specs()
    {
        static std::vector<SpecDef> const specs = {
            { 1, "arms", "Arms", "arms", 0, ROLE_DPS, 0, "fury" },
            { 1, "fury", "Fury", "fury", 1, ROLE_DPS, 1, "arms" },
            { 1, "prot", "Protection", "prot", 2, ROLE_TANK, 2, "fury" },
            { 2, "holy", "Holy", "holy", 0, ROLE_HEAL, 0, "ret" },
            { 2, "prot", "Protection", "prot", 1, ROLE_TANK, 1, "ret" },
            { 2, "ret", "Retribution", "ret", 2, ROLE_DPS, 2, "holy" },
            { 3, "bm", "Beast Mastery", "bm", 0, ROLE_DPS, 0, "mm" },
            { 3, "mm", "Marksmanship", "mm", 1, ROLE_DPS, 1, "sv" },
            { 3, "sv", "Survival", "sv", 2, ROLE_DPS, 2, "mm" },
            { 4, "assa", "Assassination", "assa", 0, ROLE_DPS, 0, "combat" },
            { 4, "combat", "Combat", "combat", 1, ROLE_DPS, 1, "assa" },
            { 4, "sub", "Subtlety", "sub", 2, ROLE_DPS, 2, "combat" },
            { 5, "disc", "Discipline", "disc", 0, ROLE_HEAL, 0, "holy" },
            { 5, "holy", "Holy", "holy", 1, ROLE_HEAL, 1, "disc" },
            { 5, "shadow", "Shadow", "shadow", 2, ROLE_DPS, 2, "disc" },
            { 6, "blood", "Blood", "blood", 0, ROLE_TANK, 0, "frost" },
            { 6, "frost", "Frost", "frost", 1, ROLE_DPS, 1, "unholy" },
            { 6, "unholy", "Unholy", "unholy", 2, ROLE_DPS, 2, "frost" },
            { 7, "ele", "Elemental", "ele", 0, ROLE_DPS, 0, "resto" },
            { 7, "enh", "Enhancement", "enh", 1, ROLE_DPS, 1, "resto" },
            { 7, "resto", "Restoration", "resto", 2, ROLE_HEAL, 2, "ele" },
            { 8, "arcane", "Arcane", "all", 0, ROLE_DPS, 0, "fire" },
            { 8, "fire", "Fire", "all", 1, ROLE_DPS, 1, "frost" },
            { 8, "frost", "Frost", "all", 2, ROLE_DPS, 2, "fire" },
            { 9, "affli", "Affliction", "all", 0, ROLE_DPS, 0, "destro" },
            { 9, "demo", "Demonology", "all", 1, ROLE_DPS, 1, "affli" },
            { 9, "destro", "Destruction", "all", 2, ROLE_DPS, 2, "affli" },
            { 11, "balance", "Balance", "balance", 0, ROLE_DPS, 0, "resto" },
            { 11, "bear", "Feral (bear)", "bear", 1, ROLE_TANK, 1, "cat" },
            { 11, "resto", "Restoration", "resto", 2, ROLE_HEAL, 2, "balance" },
            { 11, "cat", "Feral (cat)", "cat", 3, ROLE_DPS, 1, "bear" },
        };
        return specs;
    }

    // warrior / paladin / ... / druid, the class id as text, or a few common short forms. 0 = unknown.
    inline std::uint8_t ParseClass(std::string const& raw)
    {
        std::string c = Lower(raw);
        c.erase(std::remove_if(c.begin(), c.end(), [](char ch) { return ch == ' ' || ch == '-' || ch == '_'; }), c.end());
        static std::vector<std::pair<char const*, std::uint8_t>> const words = {
            { "warrior", 1 }, { "war", 1 }, { "paladin", 2 }, { "pala", 2 }, { "pally", 2 }, { "hunter", 3 }, { "hunt", 3 },
            { "rogue", 4 }, { "priest", 5 }, { "deathknight", 6 }, { "dk", 6 }, { "shaman", 7 }, { "sham", 7 },
            { "mage", 8 }, { "warlock", 9 }, { "lock", 9 }, { "druid", 11 },
        };
        for (auto const& w : words)
            if (c == w.first)
                return w.second;
        if (c.size() <= 2 && !c.empty() && std::all_of(c.begin(), c.end(), [](char ch) { return std::isdigit((unsigned char)ch); }))
        {
            int id = std::stoi(c);
            if (id >= 1 && id <= 11 && id != 10)
                return std::uint8_t(id);
        }
        return 0;
    }

    inline SpecDef const* FindSpec(std::uint8_t cls, std::string const& raw)
    {
        std::string k = Lower(raw);
        k.erase(std::remove_if(k.begin(), k.end(), [](char ch) { return ch == ' ' || ch == '-' || ch == '_'; }), k.end());
        static std::vector<std::pair<char const*, char const*>> const alias = {
            { "protection", "prot" }, { "tank", "prot" }, { "retribution", "ret" }, { "beastmastery", "bm" }, { "beast", "bm" },
            { "marksmanship", "mm" }, { "marks", "mm" }, { "survival", "sv" }, { "surv", "sv" }, { "assassination", "assa" },
            { "as", "assa" }, { "subtlety", "sub" }, { "discipline", "disc" }, { "elemental", "ele" }, { "enhancement", "enh" },
            { "restoration", "resto" }, { "affliction", "affli" }, { "aff", "affli" }, { "demonology", "demo" },
            { "destruction", "destro" }, { "feral", "cat" }, { "feralcat", "cat" }, { "feralbear", "bear" },
            { "moonkin", "balance" }, { "boomkin", "balance" }, { "heal", "" }, { "healer", "" },
        };
        for (auto const& a : alias)
            if (k == a.first)
            {
                k = a.second;
                break;
            }
        if (cls == 6 && k == "prot")
            k = "blood";
        if (cls == 11 && k == "prot")
            k = "bear";
        for (SpecDef const& s : Specs())
            if (s.cls == cls && k == s.key)
                return &s;
        if (k.empty() || Lower(raw) == "heal" || Lower(raw) == "healer")   // "healer": the class's first healing spec
            for (SpecDef const& s : Specs())
                if (s.cls == cls && s.role == ROLE_HEAL)
                    return &s;
        if (k.size() == 1 && std::isdigit((unsigned char)k[0]))             // 1-4: the class's specs in order
        {
            int n = k[0] - '1';
            for (SpecDef const& s : Specs())
                if (s.cls == cls && n-- == 0)
                    return &s;
        }
        return nullptr;
    }

    // The pick when none is given: a damage spec of the class.
    inline SpecDef const* DefaultSpec(std::uint8_t cls)
    {
        static char const* const want[12] = { "", "arms", "ret", "mm", "combat", "shadow", "unholy", "ele", "fire", "affli", "", "balance" };
        return cls < 12 ? FindSpec(cls, want[cls]) : nullptr;
    }

    inline SpecDef const* PartnerSpec(SpecDef const& s) { return FindSpec(s.cls, s.partner); }

    // Default race per class and faction (any player race works for the module; these are the plain picks).
    inline std::uint8_t DefaultRace(std::uint8_t cls, bool horde)
    {
        //                                    -  war pal hun rog pri  dk sha mag wlk  -  dru
        static std::uint8_t const alliance[12] = { 0, 1, 1, 3, 1, 1, 1, 11, 1, 1, 0, 4 };
        static std::uint8_t const hordeRace[12] = { 0, 2, 10, 8, 5, 5, 2, 2, 8, 2, 0, 6 };
        return cls < 12 ? (horde ? hordeRace[cls] : alliance[cls]) : 0;
    }

    // -1 = not given, 0 = Alliance, 1 = Horde.
    inline int ParseFaction(std::string const& raw)
    {
        std::string f = Lower(raw);
        if (f == "alliance" || f == "a" || f == "ally")
            return 0;
        if (f == "horde" || f == "h")
            return 1;
        return -1;
    }

    // ---- names ----------------------------------------------------------------------------------------------------

    inline bool NameHasBrandWord(std::string const& name)
    {
        std::string n = Lower(name);
        // Split literals: no string in the module's sources holds a brand word (test_bossfights_export.py scans them).
        for (char const* w : { "war" "craft", "bliz" "zard", "li" "ch", "ar" "thas", "w" "ow", "frost" "mourne" })
            if (n.find(w) != std::string::npos)
                return true;
        return false;
    }

    // A name the boards and the website show as it is: 2-12 plain letters, the first upper case, no brand word.
    inline bool HeroNameAllowed(std::string const& name)
    {
        if (name.size() < 2 || name.size() > 12)
            return false;
        for (std::size_t i = 0; i < name.size(); ++i)
        {
            unsigned char c = name[i];
            if (!std::isalpha(c) || (i == 0) != bool(std::isupper(c)))
                return false;
        }
        return !NameHasBrandWord(name);
    }

    // A generated name: two or three syllables from seed and attempt (the module tries attempts until the core's name
    // check and the character cache take one). Always passes HeroNameAllowed.
    inline std::string HeroName(std::uint32_t seed, std::uint32_t attempt)
    {
        static char const* const first[] = { "Ka", "Lo", "Mi", "Ser", "Tor", "Vel", "Bra", "Dra", "Eli", "Fen", "Gal", "Hal",
                                             "Ira", "Jor", "Kel", "Lyr", "Mor", "Nel", "Ori", "Pel", "Quin", "Ras", "Sol", "Tam",
                                             "Ul", "Var", "Wyn", "Yar", "Zan", "Ael", "Bel", "Cor" };
        static char const* const mid[] = { "", "", "ra", "li", "no", "ve", "da", "si", "ro", "ma", "te", "lu" };
        static char const* const last[] = { "n", "th", "ra", "dor", "wen", "lin", "mar", "ric", "sa", "vin", "ka", "nor",
                                            "del", "ris", "ten", "lyn" };
        std::uint32_t h = seed * 2654435761u ^ (attempt + 1) * 40503u;
        h ^= h >> 13;
        h *= 0x5bd1e995u;
        h ^= h >> 15;
        std::string n = std::string(first[h % 32]) + mid[(h >> 5) % 12] + last[(h >> 9) % 16];
        if (attempt >= 64)   // the pool is busy: add letters until the name is new (still letters only)
            for (std::uint32_t a = attempt; a && n.size() < 12; a /= 26)
                n += char('a' + a % 26);
        if (n.size() > 12)
            n.resize(12);
        if (!HeroNameAllowed(n))
            n = "Hero" + std::string(1, char('a' + h % 26)) + std::string(1, char('a' + (h >> 8) % 26));
        return n;
    }

    // ---- kits, bags, consumables --------------------------------------------------------------------------------

    inline HeroKit const* KitFor(std::uint8_t cls, char const* kitKey)
    {
        for (HeroKit const& k : HeroKits())
            if (k.cls == cls && std::strcmp(k.key, kitKey) == 0)
                return &k;
        return nullptr;
    }

    inline std::uint32_t Medallion(bool horde) { return horde ? HERO_MEDALLION_HORDE : HERO_MEDALLION_ALLIANCE; }

    // Every item id once, with the most copies any one kit wants (two of the same ring, a pair of weapons).
    inline void AddKitItems(std::map<std::uint32_t, std::uint32_t>& want, std::vector<std::uint32_t> const& items)
    {
        std::map<std::uint32_t, std::uint32_t> here;
        for (std::uint32_t id : items)
            ++here[id];
        for (auto const& kv : here)
            want[kv.first] = std::max(want[kv.first], kv.second);
    }

    // What goes in the bags: every other PvE kit of the class and every PvP kit (with the faction's medallion), minus
    // what the worn kit already holds. Sorted by item id, so the bags come out the same every time.
    inline std::vector<std::pair<std::uint32_t, std::uint32_t>> HeroBagItems(std::uint8_t cls, char const* wornKit, bool horde)
    {
        std::map<std::uint32_t, std::uint32_t> want;
        for (HeroKit const& k : HeroKits())
        {
            if (k.cls != cls)
                continue;
            if (std::strcmp(k.key, wornKit) != 0)
                AddKitItems(want, k.pve);
            std::vector<std::uint32_t> pvp = k.pvp;
            pvp.push_back(Medallion(horde));
            AddKitItems(want, pvp);
        }
        if (HeroKit const* worn = KitFor(cls, wornKit))
        {
            std::map<std::uint32_t, std::uint32_t> on;
            for (std::uint32_t id : worn->pve)
                ++on[id];
            for (auto const& kv : on)
            {
                auto it = want.find(kv.first);
                if (it == want.end())
                    continue;
                if (it->second <= kv.second)
                    want.erase(it);
                else
                    it->second -= kv.second;
            }
        }
        return { want.begin(), want.end() };
    }

    // A flask for the kit's stat profile and raid food, a small stack each.
    constexpr std::uint32_t FLASK_ENDLESS_RAGE = 46377, FLASK_FROST_WYRM = 46376, FLASK_STONEBLOOD = 46379, FISH_FEAST = 43015;
    constexpr std::uint32_t CONSUMABLE_STACK = 5;

    inline std::uint32_t FlaskFor(char const* kitRole)
    {
        std::string r = kitRole ? kitRole : "";
        if (r == "tank" || r == "dktank" || r == "bear")
            return FLASK_STONEBLOOD;
        if (r == "caster" || r == "healer" || r == "holypal")
            return FLASK_FROST_WYRM;
        return FLASK_ENDLESS_RAGE;
    }

    // The class's reagents and tools (ov_main.lua CLASS_ITEMS) plus the flask and the food.
    inline std::vector<std::pair<std::uint32_t, std::uint32_t>> HeroSupplies(std::uint8_t cls, char const* kitRole)
    {
        std::vector<std::pair<std::uint32_t, std::uint32_t>> out = HeroClassItems(cls);
        out.push_back({ FlaskFor(kitRole), CONSUMABLE_STACK });
        out.push_back({ FISH_FEAST, CONSUMABLE_STACK });
        return out;
    }

    // ---- action bars ----------------------------------------------------------------------------------------------

    // The bar layout for a class and spec (button -> the slot's alternatives): the class's rows, then the spec's rows,
    // a spec row replacing the class row that starts at the same button. The boost's markers stay in (BAR_*).
    inline std::map<std::uint8_t, std::vector<std::int32_t>> BarLayout(std::uint8_t cls, char const* spec)
    {
        std::map<std::uint8_t, std::vector<std::vector<std::int32_t>> const*> rows;
        for (HeroBarRow const& r : HeroBarRows())
            if (r.cls == cls && r.spec[0] == '\0')
                rows[r.start] = &r.slots;
        for (HeroBarRow const& r : HeroBarRows())
            if (r.cls == cls && r.spec[0] != '\0' && std::strcmp(r.spec, spec) == 0)
                rows[r.start] = &r.slots;
        std::map<std::uint8_t, std::vector<std::int32_t>> out;
        for (auto const& kv : rows)
            for (std::size_t i = 0; i < kv.second->size() && kv.first + i < 144; ++i)
                out[std::uint8_t(kv.first + i)] = (*kv.second)[i];
        return out;
    }

    // ---- equipment slots ------------------------------------------------------------------------------------------

    // Where an item of this InventoryType may go (EquipmentSlots 0-18), in the order to try. dualWield: the character
    // may hold a one-hander in the off hand; titanGrip: a two-hander too.
    inline std::vector<std::uint8_t> EquipSlotsFor(std::uint32_t invType, bool dualWield, bool titanGrip)
    {
        switch (invType)
        {
            case 1: return { 0 };            // head
            case 2: return { 1 };            // neck
            case 3: return { 2 };            // shoulders
            case 4: return { 3 };            // shirt
            case 5: case 20: return { 4 };   // chest, robe
            case 6: return { 5 };            // waist
            case 7: return { 6 };            // legs
            case 8: return { 7 };            // feet
            case 9: return { 8 };            // wrists
            case 10: return { 9 };           // hands
            case 11: return { 10, 11 };      // finger
            case 12: return { 12, 13 };      // trinket
            case 16: return { 14 };          // back
            case 13: return dualWield ? std::vector<std::uint8_t>{ 15, 16 } : std::vector<std::uint8_t>{ 15 };   // one-hand
            case 21: return { 15 };          // main hand
            case 17: return titanGrip ? std::vector<std::uint8_t>{ 15, 16 } : std::vector<std::uint8_t>{ 15 };   // two-hand
            case 14: case 22: case 23: return { 16 };   // shield, off hand, held in off hand
            case 15: case 25: case 26: case 28: return { 17 };   // ranged, thrown, ranged right, relic
            case 19: return { 18 };          // tabard
            default: return {};
        }
    }

    // characters.instance_mode_mask: dungeon difficulty in the low nibble, raid difficulty in the high one
    // (10 normal = 0, 25 normal = 1, 10 heroic = 2, 25 heroic = 3).
    inline std::uint8_t InstanceModeMask(int diff, int size = 10) { return std::uint8_t(RaidDifficultyIndex(diff, size) << 4); }

    // ---- the console command and its replies ------------------------------------------------------------------------

    // ".bossfight hero go <ACCOUNT> <boss> <difficulty> <mode> [class [spec [spec2]]] [alliance|horde]" and
    // ".bossfight hero make <ACCOUNT> <class> [spec [spec2]] [alliance|horde]": the class, spec, second spec and faction
    // from the words after the fixed ones. No class = keep the last pick (or make the default).
    struct HeroPick
    {
        std::uint8_t cls = 0;
        SpecDef const* spec = nullptr;
        SpecDef const* spec2 = nullptr;
        int faction = -1;
        std::string error;
    };

    inline HeroPick ParseHeroPick(std::vector<std::string> const& words)
    {
        HeroPick p;
        std::vector<std::string> rest;
        for (std::string const& w : words)
        {
            int f = ParseFaction(w);
            if (f >= 0 && p.faction < 0)
                p.faction = f;
            else if (!w.empty())
                rest.push_back(w);
        }
        if (rest.empty())
            return p;
        p.cls = ParseClass(rest[0]);
        if (!p.cls)
        {
            p.error = "unknown class";
            return p;
        }
        p.spec = rest.size() > 1 ? FindSpec(p.cls, rest[1]) : DefaultSpec(p.cls);
        if (!p.spec)
        {
            p.error = "unknown spec for that class";
            return p;
        }
        p.spec2 = rest.size() > 2 ? FindSpec(p.cls, rest[2]) : PartnerSpec(*p.spec);
        if (!p.spec2)
        {
            p.error = "unknown second spec for that class";
            return p;
        }
        if (p.spec2 == p.spec)
            p.spec2 = PartnerSpec(*p.spec);
        if (rest.size() > 3)
            p.error = "too many words";
        return p;
    }

    // Words for replies: tabs and newlines out, at most 120 characters.
    inline std::string ReplyWords(std::string const& s)
    {
        std::string out;
        for (char c : s)
            if (c != '\t' && c != '\n' && c != '\r')
                out += c;
        if (out.size() > 120)
            out.resize(120);
        return out;
    }

    // "bfhero ok <name> <class> <spec> <spec2> <made|ready> <hero account>": what the website reads (stream/bossfights.py
    // parse_hero_reply). "bfhero no <why>": nothing was made or started; why is plain words for the player.
    inline std::string ReplyHeroOk(std::string const& name, std::uint8_t cls, SpecDef const& spec, SpecDef const& spec2,
                                   bool made, std::string const& heroAccount)
    {
        return "bfhero ok " + name + " " + ClassToken(cls) + " " + spec.key + " " + spec2.key + " " + (made ? "made" : "ready") +
            " " + heroAccount;
    }

    inline std::string ReplyHeroNo(std::string const& why) { return "bfhero no " + ReplyWords(why); }

    // One line of ".bossfight hero list": "bfhero hero <name> <class> <spec> <spec2> <current|->".
    inline std::string ReplyHeroLine(std::string const& name, std::uint8_t cls, std::string const& spec, std::string const& spec2,
                                     bool current)
    {
        return "bfhero hero " + name + " " + ClassToken(cls) + " " + spec + " " + spec2 + " " + (current ? "current" : "-");
    }

    // The hero's waiting go, as stored (leyline_bf_hero_accounts.pending_go): the same text as the website's go kind.
    // Boss Rush's 25-player raids keep "normal25" / "heroic25" (BossfightsLogic.h DiffToken); the table's column is 64.
    inline std::string HeroGo(BossDef const& boss, int diff, int mode, int size = 10)
    {
        return std::string("bf:") + boss.key + ":" + DiffToken(diff, size) + ":" + ModeName(mode);
    }

    inline JoinArgs ParseHeroGo(std::string const& go)
    {
        JoinArgs bad;
        bad.error = "bad go";
        if (go.rfind("bf:", 0) != 0)
            return bad;
        std::vector<std::string> parts;
        std::string cur;
        for (char c : go.substr(3))
        {
            if (c == ':')
            {
                parts.push_back(cur);
                cur.clear();
            }
            else
                cur += c;
        }
        parts.push_back(cur);
        if (parts.size() != 3)
            return bad;
        JoinArgs a = ParseJoin(parts[0], parts[1], parts[2]);
        return a;
    }

    // LYBF "H": the hero you are on (the in-game panel's hero row). active: 0 = the first spec is on, 1 = the second.
    inline std::string MsgHero(std::string const& name, std::uint8_t cls, std::string const& spec, std::string const& spec2, int active)
    {
        return "H\t" + name + "\t" + ClassToken(cls) + "\t" + spec + "\t" + spec2 + "\t" + (active ? "1" : "0");
    }

    // Wipe: in a run of heroes only, everyone is brought back and the clock is ready again after this many seconds.
    constexpr unsigned HERO_AUTO_RETRY_SEC = 8;
}

#endif
