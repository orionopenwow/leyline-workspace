/*
 * mod-leyline-bossfights: the pure parts (no core headers), so tests/test_bossfights_logic.cpp can check them
 * with a plain g++. Everything here works on plain numbers and strings: the bosses table, argument parsing, the
 * bot seat plan, the lobby decision, the roster text and the LYBF addon messages (docs/BOSS-FIGHTS.md).
 *
 * Player-facing words: the featured fight is "The Frozen Throne"; Boss Rush's other fights have descriptive names too. The boss's own name never appears here or in
 * any message (stream/hub.py BANNED_RE: no game, publisher or boss brand words).
 */
#ifndef LEYLINE_BOSSFIGHTS_LOGIC_H
#define LEYLINE_BOSSFIGHTS_LOGIC_H

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <cstdio>
#include <string>
#include <vector>

namespace LeylineBF
{
    constexpr std::uint32_t MODULE_VERSION = 1;
    constexpr char const* ADDON_PREFIX = "LYBF";

    enum Difficulty : int { DIFF_NONE = -1, DIFF_NORMAL = 0, DIFF_HEROIC = 1 };
    enum Mode : int { MODE_NONE = -1, MODE_BOTS = 0, MODE_GROUP = 1, MODE_GROUP_ONLY = 2 };
    enum Role : int { ROLE_TANK = 0, ROLE_HEAL = 1, ROLE_DPS = 2 };

    // A boss fight: the real encounter on its real map, played with the core's own boss script. v1 had one (the
    // final fight of the ICC raid, map 631); Boss Rush (docs/BOSS-RUSH.md, the launcher's local "exact" engine) adds
    // the others below. Only the featured (first) fight is offered unless LeylineBossfight.AllFights is on, so the
    // official realm keeps exactly what it had.
    struct BossDef
    {
        char const* key;              // the id players, the website, the launcher and the addon use
        char const* title;            // what players see (never the boss's own name)
        char const* raid;             // the raid's own place name (a zone name, not a brand word), for the launcher
        std::uint32_t mapId;
        std::uint32_t zone;           // AreaTable zone id (the character list shows it for a hero saved there)
        std::uint32_t bossEntry;      // creature entry
        std::uint32_t bossData;       // the instance script's boss id (DATA_*)
        std::uint32_t endSpell;       // the clock stops when the boss starts casting this; 0 = when the boss is DONE
        std::uint8_t size;            // seats
        bool heroic;                  // the game has a heroic 10 for this raid (else normal only)
        float x, y, z, o;             // where the raid is put: on the platform, in front of the boss (or the NPC that starts it)
        float fenceX, fenceY, fenceR, fenceMinZ;  // outside this circle (or below the floor) = off the platform
        std::vector<std::uint32_t> prereq;        // boss ids set DONE quietly (the base SetBossState) before the fight
        std::vector<std::uint32_t> scripted;      // boss ids set DONE through the raid script's own SetBossState, once,
                                                  // because the script does what the fight needs then (an intro, a spawn)
        std::vector<std::uint32_t> despawn;       // creature entries taken away at the start (e.g. the drakes of a solo fight)
        char const* startHint;        // how the fight begins, said when the raid is ready
        // Boss Rush "start anywhere" (docs/BOSS-RUSH.md "Start anywhere"); every row may leave these out:
        int scope = 0;                            // SCOPE_BOSS, SCOPE_WING or SCOPE_RAID (BossfightsStarts.h builds the last two)
        std::vector<std::uint32_t> chain = {};    // a wing or the whole raid: its bosses' ids in order; the run is cleared by
                                                  // bossData's DONE (or endSpell), and a wipe on any of them is a wipe
        std::vector<std::pair<std::uint32_t, std::uint32_t>> data = {};   // the raid script's SetData(id, value), once at
                                                  // the start (the plague wing's door: the trap event done)
        std::vector<std::uint32_t> kill = {};     // creature entries killed through the core at the start, so the script
                                                  // reacts as to a real kill (the frost wyrms: their deaths call the queen)
        float hx = 0, hy = 0, hz = 0, ho = 0;     // the horde's start when it isn't the same (the gunship: each side its own ship)
    };

    enum Scope : int { SCOPE_BOSS = 0, SCOPE_WING = 1, SCOPE_RAID = 2 };

    // The wing and whole-raid starts (BossfightsStarts.h, included at the end of this file).
    inline std::vector<BossDef> const& Scopes();

    // icecrown_citadel.h (pinned core): DATA_LORD_MARROWGAR 0 ... DATA_SINDRAGOSA 11, DATA_THE_LICH_KING 12,
    // DATA_SINDRAGOSA_GAUNTLET 13. CheckRequiredBosses(DATA_THE_LICH_KING) walks the Plagueworks (Putricide, with
    // Festergut and Rotface), the Crimson Hall (Blood-Queen, with the Council), the Frostwing Halls (Sindragosa,
    // with Valithria and the gauntlet) and the Lower Spire (Saurfang, the gunship, Deathwhisper, Marrowgar).
    // Platform: boss_the_lich_king.cpp CenterPosition (503.6282, -2124.655, 840.8569) and TirionSpawn (505.2, -2124.4):
    // the raid lands 12 yards east of the NPC, facing him (west, pi). Spell 72350 is the fight's 10% cast
    // (SPELL_FURY_OF_FROSTMOURNE), the same end point the public ICC speedrun rules use.
    //
    // The other fights (Boss Rush, docs/BOSS-RUSH.md "The catalog"): the boss's spot is its row in the pinned core's
    // data/sql/base/db_world/creature.sql; the raid lands 25 yards in front of it (along its facing), turned to it.
    // The ids are each raid's own header (ruby_sanctum.h, obsidian_sanctum.h, onyxias_lair.h, vault_of_archavon.h,
    // eye_of_eternity.h). Nothing here has been played yet: docs/BOSS-RUSH.md "Not verified".
    inline std::vector<BossDef> const& Bosses()
    {
        static std::vector<BossDef> const bosses = {
            { "frozen-throne", "The Frozen Throne", "Icecrown Citadel", 631, 4812, 36597, 12, 72350, 10, true,
              517.0f, -2124.6f, 840.86f, 3.141593f,
              503.63f, -2124.66f, 75.0f, 820.0f,
              { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13 }, {}, {},
              "speak to the paladin in front of the throne to begin" },
            // Marrowgar (36612) at (-401.37, 2211.14, 42.08), facing east (0): the first hall of the citadel. Start
            // anywhere (12 Nov): the raid stands 8 yards outside the hall's door (gameobject 201857 at -338.09,
            // 2211.47, 42.97, the world database's own row), facing in.
            { "bone-lord", "The Bone Lord", "Icecrown Citadel", 631, 4812, 36612, 0, 0, 10, true,
              -330.09f, 2211.47f, 42.97f, 3.141593f,
              -401.37f, 2211.14f, 85.0f, 30.0f,
              {}, {}, {},
              "walk into the hall and pull the boss to begin" },
            // Deathwhisper (36855) at (-634.69, 2211.39, 51.98) on her dais; the cult's doors (SummonPositions) at
            // x -579 to -619, y 2154 and 2269, z 51.0. Marrowgar DONE opens her hall as in the real raid.
            // Start anywhere: 8 yards outside the oratory's door (gameobject 201563 at -520.44, 2211.47, 63.14; the
            // floor there is 62.8: gameobjects 202245 and 185879 stand on it), facing in.
            { "cult-mistress", "The Cult Mistress", "Icecrown Citadel", 631, 4812, 36855, 1, 0, 10, true,
              -512.44f, 2211.47f, 63.0f, 3.141593f,
              -610.0f, 2211.4f, 110.0f, 40.0f,
              { 0 }, {}, {},
              "walk into the oratory and up to the dais to begin" },
            // Halion (39863) at (3156.0, 533.81, 72.99). Baltharus (0) and Saviana (2) quietly; Zarithrian (1) through
            // the script's own SetBossState: that is what starts Halion's intro (instance_ruby_sanctum.cpp).
            { "twilight-destroyer", "The Twilight Destroyer", "The Ruby Sanctum", 724, 4987, 39863, 6, 0, 10, true,
              3131.0f, 533.37f, 72.99f, 0.017500f,
              3156.0f, 533.81f, 70.0f, 55.0f,
              { 0, 2 }, { 1 }, {},
              "wait for the boss to land, then pull to begin" },
            // Sartharion (28860) at (3246.57, 551.26, 58.62) facing south. Tenebron 1, Vesperon 2, Shadron 3: DONE
            // drakes are left out of the fight (boss_sartharion.cpp JustEngagedWith), and their creatures go away.
            { "obsidian-sanctum", "The Obsidian Sanctum", "The Obsidian Sanctum", 615, 4493, 28860, 0, 0, 10, false,
              3245.26f, 526.30f, 58.62f, 1.518400f,
              3246.57f, 551.26f, 75.0f, 40.0f,
              { 1, 2, 3 }, {}, { 30449, 30451, 30452 },
              "pull the boss on the island to begin" },
            // The same with all three drakes up: they join the fight one by one (the game's hardest version of it).
            { "obsidian-three", "The Obsidian Sanctum, three drakes", "The Obsidian Sanctum", 615, 4493, 28860, 0, 0, 10, false,
              3245.26f, 526.30f, 58.62f, 1.518400f,
              3246.57f, 551.26f, 75.0f, 40.0f,
              {}, {}, {},
              "pull the boss on the island to begin" },
            // Onyxia (10184) at (-10.62, -219.36, -87.73) at the back of her lair, facing the way in.
            { "brood-mother", "The Brood Mother's Lair", "Onyxia's Lair", 249, 2159, 10184, 0, 0, 10, false,
              -35.48f, -216.72f, -87.73f, 6.177400f,
              -10.62f, -219.36f, 80.0f, -110.0f,
              {}, {}, {},
              "pull the dragon to begin" },
            // The vault's four watchers (vault_of_archavon.h EVENT_*): Archavon 31125, Emalon 33993, Koralon 35013,
            // Toravon 38433, each in its own room. The realm's Wintergrasp lock is off in Boss Rush's profile.
            { "stone-watcher", "The Stone Watcher", "Vault of Archavon", 624, 4603, 31125, 0, 0, 10, false,
              113.89f, -101.09f, 91.40f, 6.265700f,
              138.89f, -101.53f, 55.0f, 70.0f,
              {}, {}, {},
              "pull the watcher to begin" },
            { "storm-watcher", "The Storm Watcher", "Vault of Archavon", 624, 4603, 33993, 1, 0, 10, false,
              -219.16f, -263.57f, 91.55f, 4.729800f,
              -218.73f, -288.57f, 55.0f, 70.0f,
              {}, {}, {},
              "pull the watcher to begin" },
            { "flame-watcher", "The Flame Watcher", "Vault of Archavon", 624, 4603, 35013, 2, 0, 10, false,
              -218.96f, 79.39f, 91.83f, 1.553300f,
              -218.52f, 104.39f, 55.0f, 70.0f,
              {}, {}, {},
              "pull the watcher to begin" },
            { "frost-watcher", "The Frost Watcher", "Vault of Archavon", 624, 4603, 38433, 3, 0, 10, false,
              -43.77f, -263.71f, 91.25f, 4.729800f,
              -43.33f, -288.71f, 55.0f, 70.0f,
              {}, {}, {},
              "pull the watcher to begin" },
            // Malygos (28859) circles above the disc; the disc's middle is eye_of_eternity.h CenterPos
            // (754.40, 1301.27, 266.10). The raid lands 20 yards west of it, facing the middle.
            { "spell-weaver", "The Spellweaver's Eye", "The Eye of Eternity", 616, 4500, 28859, 0, 0, 10, false,
              734.40f, 1301.27f, 266.20f, 0.0f,
              754.40f, 1301.27f, 60.0f, 250.0f,
              {}, {}, {},
              "walk to the middle of the disc to begin" },

            // ---- the rest of the citadel (start anywhere, owner 12 Nov: "start in front of any boss or any raid wing
            // or the entire raid itself"). Each boss's start is 8 yards outside its own room's door, facing in: the
            // doors are the pinned core's world database rows (data/sql/base/db_world/gameobject.sql, the entries of
            // icecrown_citadel.h's doorData), the bosses its creature.sql rows, the earlier bosses the instance
            // script's own checks (instance_icecrown_citadel.cpp CheckRequiredBosses, doorData). Not stood on yet:
            // docs/BOSS-RUSH.md "Not verified".
            //
            // The gunship (boss id 2): each side boards its own ship. The spots are the world database's game_tele
            // rows 1958 (the alliance's captain) and 1957 (the horde's). Deathwhisper DONE through the script
            // (scripted) brings the ships in (SpawnGunship). No fence: the decks move. The entry is the alliance's ship
            // (37540), for the record only: the fight ends on the boss id's DONE.
            { "icc-gunship", "The Gunship Battle", "Icecrown Citadel", 631, 4812, 37540, 2, 0, 10, true,
              -467.294f, 2466.7f, 190.462f, 3.103320f,
              0.0f, 0.0f, 0.0f, 0.0f,
              { 0 }, { 1 }, {},
              "speak to your ship's captain to set sail",
              SCOPE_BOSS, {}, {}, {},
              -474.192f, 1916.34f, 216.332f, 3.174010f },
            // Saurfang (37813) at (-461.51, 2211.11, 541.20) on his rise; the raid lands where the rise's teleporter
            // sets you down (game_tele 1959), 98 yards west of him, facing east.
            { "icc-deathbringer", "The Deathbringer", "Icecrown Citadel", 631, 4812, 37813, 3, 0, 10, true,
              -559.589f, 2211.55f, 539.286f, 6.272410f,
              -461.51f, 2211.11f, 110.0f, 520.0f,
              { 0, 1, 2 }, {}, {},
              "walk up to the boss to begin" },
            // Festergut (36626) at (4267.92, 3137.26, 360.59); his door 201371 at (4267.71, 3085.06, 359.46).
            { "icc-bloat", "The Bloated Horror", "Icecrown Citadel", 631, 4812, 36626, 4, 0, 10, true,
              4267.71f, 3077.06f, 359.5f, 1.570796f,
              4267.92f, 3137.26f, 75.0f, 340.0f,
              { 0, 1, 2, 3 }, {}, {},
              "walk through the door and pull the boss to begin" },
            // Rotface (36627) at (4445.87, 3137.31, 360.59); his door 201370 at (4445.72, 3085.06, 359.46).
            { "icc-ooze", "The Ooze Horror", "Icecrown Citadel", 631, 4812, 36627, 5, 0, 10, true,
              4445.72f, 3077.06f, 359.5f, 1.570796f,
              4445.87f, 3137.31f, 75.0f, 340.0f,
              { 0, 1, 2, 3 }, {}, {},
              "walk through the door and pull the boss to begin" },
            // Putricide (36678) at (4356.19, 3262.90, 389.48); the lab's door 201372 at (4356.76, 3155.91, 388.31).
            // The door opens on the trap event's DONE (SetData(DATA_PUTRICIDE_TRAP_STATE 254, DONE 3)).
            { "icc-professor", "The Mad Professor", "Icecrown Citadel", 631, 4812, 36678, 6, 0, 10, true,
              4356.76f, 3147.91f, 388.4f, 1.570796f,
              4356.19f, 3262.9f, 125.0f, 370.0f,
              { 0, 1, 2, 3, 4, 5 }, {}, {},
              "walk into the lab and pull the boss to begin",
              SCOPE_BOSS, {}, { { 254, 3 } }, {} },
            // The three princes (37970, 37972, 37973) at x 4680, y 2755-2783, z 364.17; the hall's door 201376 at
            // (4581.03, 2769.33, 361.21), opened by the trash before it (boss id 14) as in the real raid.
            { "icc-council", "The Blood Council", "Icecrown Citadel", 631, 4812, 37970, 7, 0, 10, true,
              4573.03f, 2769.33f, 361.3f, 0.0f,
              4680.29f, 2769.24f, 120.0f, 340.0f,
              { 0, 1, 2, 3, 14 }, {}, {},
              "walk into the hall and wake the princes to begin" },
            // Lana'thel (37955) at (4624.88, 2768.31, 402.19); her door 201746 at (4534.99, 2769.85, 403.65).
            { "icc-blood-queen", "The Blood Queen", "Icecrown Citadel", 631, 4812, 37955, 8, 0, 10, true,
              4526.99f, 2769.85f, 403.8f, 0.0f,
              4624.88f, 2768.31f, 110.0f, 390.0f,
              { 0, 1, 2, 3, 14, 7 }, {}, {},
              "walk up to the queen to begin" },
            // Valithria (36789) at (4203.65, 2483.89, 364.96); her hall's door 201375 at (4292.61, 2484.49, 366.30),
            // opened by the ambush before it (boss id 9) as in the real raid.
            { "icc-dreamwalker", "The Dreamwalker", "Icecrown Citadel", 631, 4812, 36789, 10, 0, 10, true,
              4300.61f, 2484.49f, 366.4f, 3.141593f,
              4203.65f, 2483.89f, 110.0f, 340.0f,
              { 0, 1, 2, 3, 9 }, {}, {},
              "step into the hall: keep the dreamer alive and heal her to full to win" },
            // Sindragosa (36853) is summoned when the two frost wyrms (37533, 37534) die (instance_icecrown_citadel.cpp
            // OnUnitDeath), then lands at boss_sindragosa.cpp SindragosaLandPos (4419.19, 2484.57, 203.38). The start
            // kills the wyrms through the core and sets the gauntlet (13) DONE; her door 201373 at (4274.20, 2484.56,
            // 210.46).
            { "icc-frost-queen", "The Frost Queen", "Icecrown Citadel", 631, 4812, 36853, 11, 0, 10, true,
              4266.2f, 2484.56f, 210.5f, 0.0f,
              4419.19f, 2484.57f, 165.0f, 190.0f,
              { 0, 1, 2, 3, 9, 10, 13 }, {}, {},
              "wait for her to land, then pull to begin",
              SCOPE_BOSS, {}, {}, { 37533, 37534 } },
        };
        return bosses;
    }

    // The featured fight (the official realm's only one) is the first; the rest (and every wing and whole-raid start)
    // need LeylineBossfight.AllFights.
    inline bool Offered(BossDef const& b, bool allFights) { return allFights || &b == &Bosses().front(); }

    // The fight on this map whose boss id is `bossData` (several fights share a map: the citadel has three).
    inline BossDef const* FindBossOnMap(std::uint32_t mapId, std::uint32_t bossData)
    {
        for (BossDef const& b : Bosses())
            if (b.mapId == mapId && b.bossData == bossData)
                return &b;
        return nullptr;
    }

    inline std::string Lower(std::string s)
    {
        std::transform(s.begin(), s.end(), s.begin(), [](unsigned char c) { return char(std::tolower(c)); });
        return s;
    }

    // A fight, a wing or the whole raid by its key (Boss Rush's "start anywhere" keys are in BossfightsStarts.h).
    inline BossDef const* FindBoss(std::string const& raw)
    {
        std::string k = Lower(raw);
        if (k == "ft" || k == "throne" || k == "frozenthrone" || k == "frozen_throne" || k == "icc")
            k = "frozen-throne";
        for (BossDef const& b : Bosses())
            if (k == b.key)
                return &b;
        for (BossDef const& b : Scopes())
            if (k == b.key)
                return &b;
        return nullptr;
    }

    // The fence: outside this circle (or below the floor) before the pull, a member is brought back to the start.
    // A start with no fence (radius 0: the gunship's decks, a wing's or the raid's start) brings back only the dead.
    inline bool HasFence(BossDef const& b) { return b.fenceR > 0.0f; }

    inline bool InsideFence(BossDef const& b, float x, float y, float z)
    {
        if (!HasFence(b))
            return true;
        float dx = x - b.fenceX, dy = y - b.fenceY;
        return dx * dx + dy * dy <= b.fenceR * b.fenceR && z >= b.fenceMinZ;
    }

    // Is this boss id one the run is about: the fight's own, or one of its wing's / raid's chain.
    inline bool InChain(BossDef const& b, std::uint32_t bossId)
    {
        return bossId == b.bossData || std::find(b.chain.begin(), b.chain.end(), bossId) != b.chain.end();
    }

    inline BossDef const* FindBossByMap(std::uint32_t mapId)
    {
        for (BossDef const& b : Bosses())
            if (b.mapId == mapId)
                return &b;
        return nullptr;
    }

    inline bool IsPrereq(BossDef const& b, std::uint32_t bossId)
    {
        return std::find(b.prereq.begin(), b.prereq.end(), bossId) != b.prereq.end();
    }

    // The difficulty word. Boss Rush (start anywhere) also takes the 25-player raid: "normal25", "heroic25" (the word
    // a hero's waiting go keeps), "25", "25n", "25h", "25-normal", "25-heroic"; ParseSize says which.
    inline int ParseDifficulty(std::string const& raw)
    {
        std::string d = Lower(raw);
        if (d == "normal" || d == "n" || d == "10" || d == "10n" || d == "10-normal" || d == "normal25" || d == "25" || d == "25n" || d == "25-normal")
            return DIFF_NORMAL;
        if (d == "heroic" || d == "h" || d == "hc" || d == "10h" || d == "10-heroic" || d == "heroic25" || d == "25h" || d == "25-heroic")
            return DIFF_HEROIC;
        return DIFF_NONE;
    }

    // 25 for a 25-player word, else 10.
    inline int ParseSize(std::string const& raw)
    {
        std::string d = Lower(raw);
        return d == "normal25" || d == "heroic25" || d == "25" || d == "25n" || d == "25h" || d == "25-normal" || d == "25-heroic" ? 25 : 10;
    }

    inline char const* DifficultyName(int d) { return d == DIFF_HEROIC ? "heroic" : "normal"; }

    // The word kept in a go and a run's record: "normal" / "heroic" for 10 (as before), "normal25" / "heroic25".
    inline std::string DiffToken(int diff, int size) { return std::string(DifficultyName(diff)) + (size == 25 ? "25" : ""); }

    // The core's raid difficulty (0 10 normal, 1 25 normal, 2 10 heroic, 3 25 heroic): RAID_DIFFICULTY_* and the
    // characters table's instance_mode_mask >> 4.
    inline int RaidDifficultyIndex(int diff, int size) { return (diff == DIFF_HEROIC ? 2 : 0) + (size == 25 ? 1 : 0); }

    inline int ParseMode(std::string const& raw)
    {
        std::string m = Lower(raw);
        if (m == "bots" || m == "bot" || m == "solo")
            return MODE_BOTS;
        if (m == "group" || m == "lobby" || m == "queue")
            return MODE_GROUP;
        if (m == "group-only" || m == "grouponly" || m == "group_only" || m == "players" || m == "no-bots")
            return MODE_GROUP_ONLY;
        return MODE_NONE;
    }

    inline char const* ModeName(int m)
    {
        switch (m)
        {
            case MODE_BOTS: return "bots";
            case MODE_GROUP_ONLY: return "group-only";
            default: return "group";
        }
    }

    // Class ids (3.3.5a ChrClasses) to the roster's word.
    inline char const* ClassToken(std::uint8_t cls)
    {
        switch (cls)
        {
            case 1: return "warrior";
            case 2: return "paladin";
            case 3: return "hunter";
            case 4: return "rogue";
            case 5: return "priest";
            case 6: return "deathknight";
            case 7: return "shaman";
            case 8: return "mage";
            case 9: return "warlock";
            case 11: return "druid";
            default: return "unknown";
        }
    }

    // A player's raid role from class and the talent tab with the most points (Player::GetMostPointsTalentTree).
    // Feral druids count as damage (bear or cat can't be told apart from the tab alone).
    inline int RoleOf(std::uint8_t cls, std::uint8_t tab)
    {
        switch (cls)
        {
            case 1: return tab == 2 ? ROLE_TANK : ROLE_DPS;                   // protection
            case 2: return tab == 1 ? ROLE_TANK : (tab == 0 ? ROLE_HEAL : ROLE_DPS);
            case 5: return tab == 2 ? ROLE_DPS : ROLE_HEAL;                    // discipline, holy
            case 6: return tab == 0 ? ROLE_TANK : ROLE_DPS;                    // blood
            case 7: return tab == 2 ? ROLE_HEAL : ROLE_DPS;                    // restoration
            case 11: return tab == 2 ? ROLE_HEAL : ROLE_DPS;                   // restoration
            default: return ROLE_DPS;
        }
    }

    // One bot seat: class, mod-playerbots premade spec number (realm/playerbots-specs.env order), role.
    struct Seat
    {
        std::uint8_t cls;
        std::uint8_t spec;
        int role;
    };

    // The default 10-player raid: 2 tanks, 3 healers, 5 damage.
    inline std::vector<Seat> DefaultSeats()
    {
        return {
            { 1, 2, ROLE_TANK },   // warrior, prot pve
            { 2, 1, ROLE_TANK },   // paladin, prot pve
            { 2, 0, ROLE_HEAL },   // paladin, holy pve
            { 5, 0, ROLE_HEAL },   // priest, disc pve
            { 7, 2, ROLE_HEAL },   // shaman, resto pve
            { 6, 2, ROLE_DPS },    // death knight, unholy pve
            { 8, 1, ROLE_DPS },    // mage, fire pve
            { 9, 0, ROLE_DPS },    // warlock, affli pve
            { 3, 1, ROLE_DPS },    // hunter, mm pve
            { 4, 1, ROLE_DPS },    // rogue, combat pve
        };
    }

    // The 25-player raid: 3 tanks, 6 healers, 16 damage (the same premade specs; a class may come more than once).
    inline std::vector<Seat> DefaultSeats25()
    {
        return {
            { 1, 2, ROLE_TANK },   // warrior, prot pve
            { 2, 1, ROLE_TANK },   // paladin, prot pve
            { 6, 0, ROLE_TANK },   // death knight, blood pve
            { 2, 0, ROLE_HEAL },   // paladin, holy pve
            { 5, 0, ROLE_HEAL },   // priest, disc pve
            { 7, 2, ROLE_HEAL },   // shaman, resto pve
            { 5, 1, ROLE_HEAL },   // priest, holy pve
            { 11, 2, ROLE_HEAL },  // druid, resto pve
            { 7, 2, ROLE_HEAL },   // shaman, resto pve
            { 6, 2, ROLE_DPS },    // death knight, unholy pve
            { 8, 1, ROLE_DPS },    // mage, fire pve
            { 9, 0, ROLE_DPS },    // warlock, affli pve
            { 3, 1, ROLE_DPS },    // hunter, mm pve
            { 4, 1, ROLE_DPS },    // rogue, combat pve
            { 1, 1, ROLE_DPS },    // warrior, fury pve
            { 2, 2, ROLE_DPS },    // paladin, ret pve
            { 5, 2, ROLE_DPS },    // priest, shadow pve
            { 7, 0, ROLE_DPS },    // shaman, ele pve
            { 7, 1, ROLE_DPS },    // shaman, enh pve
            { 8, 2, ROLE_DPS },    // mage, frost pve
            { 9, 2, ROLE_DPS },    // warlock, destro pve
            { 3, 2, ROLE_DPS },    // hunter, surv pve
            { 4, 0, ROLE_DPS },    // rogue, as pve
            { 6, 1, ROLE_DPS },    // death knight, frost pve
            { 11, 0, ROLE_DPS },   // druid, balance pve
        };
    }

    // The seats of a raid of this size: the realm's own (BotSeats, for 10) or the 25-player plan.
    inline std::vector<Seat> SeatsFor(int size, std::vector<Seat> const& ten)
    {
        return size == 25 ? DefaultSeats25() : ten;
    }

    // How many tanks, healers and damage dealers the bots bring for a player of this role (the launcher's icons).
    struct RoleCount { int tank = 0, heal = 0, dps = 0; };
    inline RoleCount CountRoles(std::vector<Seat> const& seats)
    {
        RoleCount c;
        for (Seat const& s : seats)
            (s.role == ROLE_TANK ? c.tank : s.role == ROLE_HEAL ? c.heal : c.dps) += 1;
        return c;
    }

    // "1:2:tank,2:1:tank,..." (class:spec:role). Anything unreadable gives the default seats.
    inline std::vector<Seat> ParseSeats(std::string const& text)
    {
        std::vector<Seat> out;
        std::string cur;
        auto flush = [&out](std::string const& item) -> bool
        {
            if (item.empty())
                return true;
            unsigned cls = 0, spec = 0;
            char role[16] = { 0 };
            if (std::sscanf(item.c_str(), "%u:%u:%15s", &cls, &spec, role) != 3)
                return false;
            std::string r = Lower(role);
            int ro = r == "tank" ? ROLE_TANK : r == "heal" || r == "healer" ? ROLE_HEAL : r == "dps" ? ROLE_DPS : -1;
            if (ro < 0 || cls < 1 || cls > 11 || cls == 10 || spec > 9)
                return false;
            out.push_back({ std::uint8_t(cls), std::uint8_t(spec), ro });
            return true;
        };
        for (char c : text)
        {
            if (c == ',' || c == ';')
            {
                if (!flush(cur))
                    return DefaultSeats();
                cur.clear();
            }
            else if (!std::isspace(static_cast<unsigned char>(c)))
                cur += c;
        }
        if (!flush(cur) || out.empty())
            return DefaultSeats();
        return out;
    }

    // The seats bots fill once the real players' roles are taken out: each player removes the first seat of
    // their role; a player whose role has no seat left removes the last damage seat (then the last seat of any
    // kind). Never more than `size - players` seats come back.
    inline std::vector<Seat> BotSeats(std::vector<Seat> seats, std::vector<int> const& playerRoles, std::size_t size)
    {
        for (int role : playerRoles)
        {
            if (seats.empty())
                break;
            auto it = std::find_if(seats.begin(), seats.end(), [role](Seat const& s) { return s.role == role; });
            if (it == seats.end())
            {
                auto rit = std::find_if(seats.rbegin(), seats.rend(), [](Seat const& s) { return s.role == ROLE_DPS; });
                it = rit != seats.rend() ? std::next(rit).base() : std::prev(seats.end());
            }
            seats.erase(it);
        }
        std::size_t want = size > playerRoles.size() ? size - playerRoles.size() : 0;
        if (seats.size() > want)
            seats.resize(want);
        return seats;
    }

    // The group lobby: what to do with the players waiting for one boss and difficulty.
    enum LobbyAction { LOBBY_WAIT = 0, LOBBY_START = 1, LOBBY_START_FILL = 2, LOBBY_EXPIRE = 3 };

    struct LobbyDecision
    {
        LobbyAction action;
        long long etaSec;   // seconds until bots fill the seats; -1 = no estimate (group-only waits for a full raid)
    };

    // waiting: real players in the lobby; waitedMs: how long the first (the leader) has waited; leaderGroupOnly:
    // the leader picked group-only; bots: bots are allowed on this realm.
    inline LobbyDecision DecideLobby(std::size_t waiting, std::size_t size, unsigned long long waitedMs,
                                     unsigned long long fillAfterMs, unsigned long long expireMs,
                                     bool leaderGroupOnly, bool bots)
    {
        if (waiting == 0)
            return { LOBBY_WAIT, -1 };
        if (waiting >= size)
            return { LOBBY_START, 0 };
        bool fill = bots && !leaderGroupOnly;
        if (fill)
        {
            if (waitedMs >= fillAfterMs)
                return { LOBBY_START_FILL, 0 };
            return { LOBBY_WAIT, (long long)((fillAfterMs - waitedMs + 999) / 1000) };
        }
        if (expireMs && waitedMs >= expireMs)
            return { LOBBY_EXPIRE, -1 };
        return { LOBBY_WAIT, -1 };
    }

    struct RosterEntry
    {
        std::string name;
        std::uint8_t cls;
        bool bot;
    };

    // "Name:class:bot|player", comma separated (the leyline_bossfight_runs.roster column).
    inline std::string RosterText(std::vector<RosterEntry> const& roster)
    {
        std::string out;
        for (RosterEntry const& r : roster)
        {
            if (!out.empty())
                out += ',';
            std::string name;
            for (char c : r.name)
                if (c != ',' && c != ':' && c != '\'' && c != '"' && c != '\\' && c != '\t' && c != '\n')
                    name += c;
            out += name + ':' + ClassToken(r.cls) + ':' + (r.bot ? "bot" : "player");
        }
        return out;
    }

    // The LYBF addon messages (docs/BOSS-FIGHTS.md). The prefix and a tab go in front when sent.
    inline std::string MsgQueued(char const* boss, int diff, int mode, std::size_t waiting, std::size_t needed, long long eta)
    {
        return std::string("Q\t") + boss + '\t' + DifficultyName(diff) + '\t' + ModeName(mode) + '\t' +
            std::to_string(waiting) + '\t' + std::to_string(needed) + '\t' + std::to_string(eta);
    }

    inline std::string MsgStarting(char const* boss, int diff)
    {
        return std::string("S\t") + boss + '\t' + DifficultyName(diff);
    }

    inline std::string MsgTimer(unsigned long long ms) { return "T\t" + std::to_string(ms); }

    inline std::string MsgResult(bool clear, unsigned long long ms, long long rank)
    {
        return std::string("R\t") + (clear ? "clear" : "wipe") + '\t' + std::to_string(ms) + '\t' +
            (rank > 0 ? std::to_string(rank) : std::string("-"));
    }

    inline std::string MsgLeft() { return "L"; }

    // m:ss.t for chat lines.
    inline std::string FormatMs(unsigned long long ms)
    {
        unsigned long long tenths = ms / 100;
        char buf[32];
        std::snprintf(buf, sizeof(buf), "%llu:%02llu.%llu", tenths / 600, (tenths / 10) % 60, tenths % 10);
        return buf;
    }

    // The arguments of ".bossfight join <boss> <difficulty> <mode>". Mode defaults to bots, difficulty to normal.
    struct JoinArgs
    {
        BossDef const* boss = nullptr;
        int diff = DIFF_NONE;
        int mode = MODE_NONE;
        int size = 10;   // 10, or 25 (Boss Rush only: AllFights)
        std::string error;
    };

    inline JoinArgs ParseJoin(std::string const& boss, std::string const& diff, std::string const& mode, bool allFights = true)
    {
        JoinArgs a;
        a.boss = FindBoss(boss.empty() ? "frozen-throne" : boss);
        if (a.boss && !Offered(*a.boss, allFights))
            a.boss = nullptr;
        a.diff = ParseDifficulty(diff.empty() ? "normal" : diff);
        a.size = ParseSize(diff);
        a.mode = ParseMode(mode.empty() ? "bots" : mode);
        if (!a.boss)
            a.error = "Unknown boss fight. Type .bossfight list";
        else if (a.diff == DIFF_NONE)
            a.error = "Difficulty is normal or heroic";
        else if (a.mode == MODE_NONE)
            a.error = "Mode is bots, group or group-only";
        else if (a.diff == DIFF_HEROIC && !a.boss->heroic)
            a.error = std::string(a.boss->title) + " has no heroic: play it on normal";
        else if (a.size == 25 && !allFights)
            a.error = "Boss fights here are for 10 players";
        return a;
    }
}

#include "BossfightsStarts.h"

#endif
