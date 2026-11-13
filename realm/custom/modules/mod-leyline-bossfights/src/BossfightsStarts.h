/*
 * mod-leyline-bossfights: Boss Rush's "start anywhere" (docs/BOSS-RUSH.md "Start anywhere"; owner 12 Nov: "Boss Rush
 * should allow you to start in front of any boss or any raid wing or the entire raid itself with companions or with
 * friends in a one-click UI"). Pure, like BossfightsLogic.h (which includes this file at its end), so
 * tests/test_bossfights_logic.cpp checks it with a plain g++.
 *
 * The tree: a raid, its wings (in the order players know them), each wing's bosses. A boss is a row of Bosses() (its
 * start, its earlier bosses, how it begins). A wing starts at its first boss's start and runs until its last boss is
 * down; the whole raid starts at the raid's entrance (the world database's areatrigger_teleport row) and runs until
 * its final boss is down. Scopes() turns each wing and whole raid into a row of its own (the same BossDef the run
 * code plays), so a wing or the raid is joined, recorded and retried like any fight.
 *
 * Retry after a wipe: the raid goes back to the first boss of the run still standing (its own start); a step with no
 * start of its own (the frost queen's gauntlet, the sanctum's lesser dragons) goes back to the nearest earlier step
 * that has one, else the run's own start.
 *
 * Player-facing words are ours (no boss names, no brand words: stream/tests/test_bossfights_export.py checks every
 * string here).
 */
#ifndef LEYLINE_BOSSFIGHTS_STARTS_H
#define LEYLINE_BOSSFIGHTS_STARTS_H

#include <cstdint>
#include <string>
#include <vector>

namespace LeylineBF
{
    // One boss of a wing or a raid: its id in the raid's instance script, and the fight (a Bosses() key) whose start
    // is in front of it ("" = none of its own: a retry goes to the nearest earlier step that has one).
    struct StepDef
    {
        std::uint32_t bossId;
        char const* fight;
    };

    struct WingDef
    {
        char const* key;              // the wing's key (a join's boss word); a wing of one boss has none ("") and plays the boss
        char const* title;            // its place name, as players know it
        char const* from;             // the fight whose start the wing starts at (and whose earlier bosses are set DONE)
        std::vector<std::uint32_t> skip;   // boss ids set DONE quietly besides (the gunship: companions can't cross ships)
        std::vector<StepDef> steps;
        char const* hint;
    };

    struct RaidDef
    {
        char const* key;              // the raid's own key (the launcher's picker)
        char const* title;            // its place name
        std::uint32_t mapId;
        std::uint32_t zone;
        float ex, ey, ez, eo;         // the entrance: the world database's areatrigger_teleport row (the raid's way in)
        std::uint32_t areatrigger;    // that row's id, for the record
        bool heroic;
        std::vector<char const*> fights;   // its fights (Bosses() keys) in the picker's order
        std::vector<WingDef> wings;
        char const* whole;            // the whole raid's key ("" = none: a raid of one fight)
        std::vector<std::uint32_t> wholeSkip;
        std::vector<StepDef> wholeSteps;
        char const* wholeHint;
    };

    // The raids Boss Rush plays. Entrances: areatrigger_teleport 5670 (the citadel), 5869 (the Ruby Sanctum), 5258 (the
    // vault), 5243 (the Obsidian Sanctum), 2848 (the brood mother's lair), 5290 (the Eye) in the pinned core's
    // data/sql/base/db_world/areatrigger_teleport.sql.
    inline std::vector<RaidDef> const& Raids()
    {
        static std::vector<RaidDef> const raids = {
            { "icc", "Icecrown Citadel", 631, 4812, 76.8638f, 2211.37f, 30.0f, 3.149650f, 5670, true,
              { "bone-lord", "cult-mistress", "icc-gunship", "icc-deathbringer", "icc-bloat", "icc-ooze", "icc-professor",
                "icc-council", "icc-blood-queen", "icc-dreamwalker", "icc-frost-queen", "frozen-throne" },
              {
                  { "icc-spire", "The Lower Spire", "bone-lord", { 2 },
                    { { 0, "bone-lord" }, { 1, "cult-mistress" }, { 3, "icc-deathbringer" } },
                    "the lower spire is yours: three bosses, the ships already fought" },
                  { "icc-plague", "The Plagueworks", "icc-bloat", {},
                    { { 4, "icc-bloat" }, { 5, "icc-ooze" }, { 6, "icc-professor" } },
                    "the plague wing is yours: the two horrors in either order, then the lab" },
                  { "icc-crimson", "The Crimson Hall", "icc-council", {},
                    { { 7, "icc-council" }, { 8, "icc-blood-queen" } },
                    "the crimson hall is yours: the council, then the queen above it" },
                  { "icc-frostwing", "The Frostwing Halls", "icc-dreamwalker", {},
                    { { 10, "icc-dreamwalker" }, { 13, "" }, { 11, "icc-frost-queen" } },
                    "the frostwing halls are yours: the dreamer, the gauntlet, then the queen" },
                  { "", "The Frozen Throne", "frozen-throne", {}, { { 12, "frozen-throne" } }, "" },
              },
              "icc-raid", { 2 },
              { { 0, "bone-lord" }, { 1, "cult-mistress" }, { 3, "icc-deathbringer" }, { 4, "icc-bloat" }, { 5, "icc-ooze" },
                { 6, "icc-professor" }, { 7, "icc-council" }, { 8, "icc-blood-queen" }, { 9, "" }, { 10, "icc-dreamwalker" },
                { 13, "" }, { 11, "icc-frost-queen" }, { 12, "frozen-throne" } },
              "the whole citadel from its gate: every boss, the ships already fought, the throne last" },
            // ruby_sanctum.h: Baltharus 0, Zarithrian 1, Saviana 2, Halion 6
            { "rs", "The Ruby Sanctum", 724, 4987, 3274.0f, 533.531f, 87.665f, 3.160000f, 5869, true,
              { "twilight-destroyer" },
              { { "", "The Ruby Sanctum", "twilight-destroyer", {}, { { 6, "twilight-destroyer" } }, "" } },
              "rs-raid", {},
              { { 0, "" }, { 2, "" }, { 1, "" }, { 6, "twilight-destroyer" } },
              "the whole sanctum from its door: the three guardians, then the dragon" },
            { "os", "The Obsidian Sanctum", 615, 4493, 3228.58f, 385.86f, 65.549f, 1.578000f, 5243, false,
              { "obsidian-sanctum", "obsidian-three" },
              { { "", "The Obsidian Sanctum", "obsidian-sanctum", {}, { { 0, "obsidian-sanctum" } }, "" } },
              "", {}, {}, "" },
            { "ony", "Onyxia's Lair", 249, 2159, 29.1607f, -71.3372f, -8.18032f, 4.580000f, 2848, false,
              { "brood-mother" },
              { { "", "Onyxia's Lair", "brood-mother", {}, { { 0, "brood-mother" } }, "" } },
              "", {}, {}, "" },
            // vault_of_archavon.h: the four watchers 0-3, each in its own room, any order
            { "voa", "Vault of Archavon", 624, 4603, -505.96f, -103.353f, 157.0f, 0.0f, 5258, false,
              { "stone-watcher", "storm-watcher", "flame-watcher", "frost-watcher" },
              { { "", "Vault of Archavon", "stone-watcher", {}, { { 0, "stone-watcher" } }, "" } },
              "voa-raid", {},
              { { 0, "stone-watcher" }, { 1, "storm-watcher" }, { 2, "flame-watcher" }, { 3, "frost-watcher" } },
              "the whole vault from its door: the four watchers, in any order" },
            { "eoe", "The Eye of Eternity", 616, 4500, 728.055f, 1329.03f, 275.0f, 5.515240f, 5290, false,
              { "spell-weaver" },
              { { "", "The Eye of Eternity", "spell-weaver", {}, { { 0, "spell-weaver" } }, "" } },
              "", {}, {}, "" },
        };
        return raids;
    }

    // A fight of Bosses() only (not a wing or a raid).
    inline BossDef const* FindFight(std::string const& key)
    {
        for (BossDef const& b : Bosses())
            if (key == b.key)
                return &b;
        return nullptr;
    }

    inline std::vector<std::uint32_t> StepIds(std::vector<StepDef> const& steps)
    {
        std::vector<std::uint32_t> out;
        for (StepDef const& s : steps)
            out.push_back(s.bossId);
        return out;
    }

    inline std::vector<std::uint32_t> Merged(std::vector<std::uint32_t> a, std::vector<std::uint32_t> const& b)
    {
        for (std::uint32_t x : b)
            if (std::find(a.begin(), a.end(), x) == a.end())
                a.push_back(x);
        return a;
    }

    // Every wing of more than one boss, and every whole raid, as a row the run code plays.
    inline std::vector<BossDef> const& Scopes()
    {
        static std::vector<BossDef> const scopes = []
        {
            std::vector<BossDef> out;
            for (RaidDef const& r : Raids())
            {
                for (WingDef const& w : r.wings)
                {
                    BossDef const* from = FindFight(w.from);
                    BossDef const* last = w.steps.empty() ? nullptr : FindFight(w.steps.back().fight);
                    if (!*w.key || !from || !last)
                        continue;
                    BossDef d = *from;
                    d.key = w.key;
                    d.title = w.title;
                    d.raid = r.title;
                    d.bossEntry = last->bossEntry;
                    d.bossData = last->bossData;
                    d.endSpell = last->endSpell;
                    d.heroic = r.heroic;
                    d.fenceX = d.fenceY = d.fenceR = d.fenceMinZ = 0.0f;   // no fence: the raid walks the wing
                    d.prereq = Merged(from->prereq, w.skip);
                    d.startHint = w.hint;
                    d.scope = SCOPE_WING;
                    d.chain = StepIds(w.steps);
                    out.push_back(d);
                }
                BossDef const* last = r.wholeSteps.empty() ? nullptr : FindFight(r.wholeSteps.back().fight);
                if (!*r.whole || !last)
                    continue;
                BossDef d = *last;
                d.key = r.whole;
                d.title = r.title;
                d.raid = r.title;
                d.mapId = r.mapId;
                d.zone = r.zone;
                d.heroic = r.heroic;
                d.x = r.ex;
                d.y = r.ey;
                d.z = r.ez;
                d.o = r.eo;
                d.hx = d.hy = d.hz = d.ho = 0.0f;
                d.fenceX = d.fenceY = d.fenceR = d.fenceMinZ = 0.0f;
                d.prereq = r.wholeSkip;
                d.scripted.clear();
                d.despawn.clear();
                d.data.clear();
                d.kill.clear();
                d.startHint = r.wholeHint;
                d.scope = SCOPE_RAID;
                d.chain = StepIds(r.wholeSteps);
                out.push_back(d);
            }
            return out;
        }();
        return scopes;
    }

    inline RaidDef const* RaidOf(BossDef const& b)
    {
        for (RaidDef const& r : Raids())
            if (r.mapId == b.mapId)
                return &r;
        return nullptr;
    }

    // The steps of a wing or a whole raid (empty for a single fight).
    inline std::vector<StepDef> StepsOf(BossDef const& b)
    {
        RaidDef const* r = RaidOf(b);
        if (!r || b.scope == SCOPE_BOSS)
            return {};
        if (b.scope == SCOPE_RAID)
            return r->wholeSteps;
        for (WingDef const& w : r->wings)
            if (std::string(w.key) == b.key)
                return w.steps;
        return {};
    }

    // Where a retry stands: the fight in front of the first step still standing (done(id) says which are down), or the
    // nearest earlier step's fight when that step has no start of its own. nullptr = the run's own start (a single
    // fight, or nothing earlier with a start).
    template <typename Done>
    BossDef const* RetrySpot(BossDef const& b, Done done)
    {
        std::vector<StepDef> steps = StepsOf(b);
        if (steps.empty())
            return nullptr;
        std::size_t at = steps.size() - 1;
        for (std::size_t i = 0; i < steps.size(); ++i)
            if (!done(steps[i].bossId))
            {
                at = i;
                break;
            }
        for (std::size_t i = at + 1; i-- > 0;)
            if (*steps[i].fight)
                return FindFight(steps[i].fight);
        return nullptr;
    }

    // How many of a run's steps are down.
    template <typename Done>
    std::pair<std::size_t, std::size_t> Progress(BossDef const& b, Done done)
    {
        std::vector<std::uint32_t> ids = b.chain.empty() ? std::vector<std::uint32_t>{ b.bossData } : b.chain;
        std::size_t n = 0;
        for (std::uint32_t id : ids)
            n += done(id) ? 1 : 0;
        return { n, ids.size() };
    }

    // The boss's casts that mark its phases (the playtest's timeline; docs/BOSS-RUSH.md "The boss playtest"). The final
    // fight: boss_the_lich_king.cpp SPELL_REMORSELESS_WINTER_1 / _2 (the transitions at 70% and 40%), SPELL_QUAKE (the
    // platform's edge breaks: the next phase), SPELL_FURY_OF_FROSTMOURNE (10%: the end). Ids for the other difficulties
    // are matched through the core's SpellDifficulty table where the module looks them up.
    struct PhaseMark
    {
        std::uint32_t spell;
        char const* label;   // one word, no spaces (the status line's marks=)
    };

    inline std::vector<PhaseMark> PhaseMarks(std::uint32_t bossEntry)
    {
        if (bossEntry == 36597)
            return { { 68981, "transition1" }, { 72259, "transition2" }, { 72262, "quake" }, { 72350, "end" } };
        return {};
    }

    // The console's answer to "bossfight rush status <ACCOUNT>" (the launcher's page and its playtest read it): one
    // line of key=value words, no spaces inside a value.
    struct RushState
    {
        std::string key, diff, phase, step, leader, marks, last;
        std::uint32_t map = 0, instance = 0, members = 0, bots = 0, inside = 0, wipes = 0, clears = 0, bossState = 0;
        std::size_t done = 0, of = 0;
        float x = 0, y = 0, z = 0, dist = 0;
        unsigned long long ms = 0;
    };

    inline std::string Word(std::string const& s)
    {
        std::string out;
        for (char c : s)
            out += (c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '=') ? '_' : c;
        return out.empty() ? "-" : out;
    }

    inline std::string RushLine(RushState const& s)
    {
        char pos[96];
        std::snprintf(pos, sizeof(pos), "x=%.1f y=%.1f z=%.1f dist=%.1f", s.x, s.y, s.z, s.dist);
        return "bfrush run key=" + Word(s.key) + " diff=" + Word(s.diff) + " phase=" + Word(s.phase) + " leader=" + Word(s.leader) +
            " map=" + std::to_string(s.map) + " inst=" + std::to_string(s.instance) + " " + pos +
            " members=" + std::to_string(s.members) + " bots=" + std::to_string(s.bots) + " inside=" + std::to_string(s.inside) +
            " step=" + Word(s.step) + " done=" + std::to_string(s.done) + "/" + std::to_string(s.of) +
            " boss=" + std::to_string(s.bossState) + " wipes=" + std::to_string(s.wipes) + " clears=" + std::to_string(s.clears) +
            " marks=" + Word(s.marks) + " last=" + Word(s.last) + " ms=" + std::to_string(s.ms);
    }
}

#endif
