/*
 * mod-leyline-qol: pure helpers (no core headers), so tests/test_qol_logic.cpp can check them without a core.
 *
 *   ParseIdList   "4395, 2817" -> {4395, 2817} (commas, spaces or semicolons; junk and zeros are skipped)
 *   InAreas       is an area (or any area above it: sub-area -> area -> zone) in a list of zone/area ids
 *   InstantLogout the logout decision: the module answers a logout request at once only when the core would
 *                 log out at once if the player were resting; otherwise the core's own handler answers
 *                 (combat, falling, a duel, the GM freeze, the AFK rules: the usual reasons and the 20 s countdown)
 *   DuelAreas     which AreaTable rows get the "duels allowed" flag (rows of the given map under a listed id)
 */
#ifndef MOD_LEYLINE_QOL_LOGIC_H
#define MOD_LEYLINE_QOL_LOGIC_H

#include <cstdint>
#include <string>
#include <vector>

namespace LeylineQol
{
    constexpr uint32_t ZONE_DALARAN = 4395;            // the floating city (every street, Krasus' Landing, the sewers)
    constexpr uint32_t ZONE_CRYSTALSONG = 2817;        // the forest under it
    constexpr uint32_t MAP_NORTHREND = 571;            // the Dalaran arena (map 617) is another map and never counts
    constexpr uint32_t FLAG_ALLOW_DUELS = 0x00000040;  // AREA_FLAG_ALLOW_DUELS (DBCEnums.h)
    constexpr int MAX_DEPTH = 8;                       // sub-area chains are 1-2 deep; this only stops a loop

    inline std::vector<uint32_t> ParseIdList(std::string const& text)
    {
        std::vector<uint32_t> out;
        uint64_t cur = 0;
        bool inNum = false, bad = false;
        auto flush = [&]() {
            if (inNum && !bad && cur > 0 && cur <= 0xFFFFFFFFull)
            {
                bool dup = false;
                for (uint32_t v : out)
                    dup = dup || v == cur;
                if (!dup)
                    out.push_back(static_cast<uint32_t>(cur));
            }
            cur = 0;
            inNum = bad = false;
        };
        for (char c : text)
        {
            if (c >= '0' && c <= '9')
            {
                if (cur < 0x100000000ull)
                    cur = cur * 10 + static_cast<uint64_t>(c - '0');
                inNum = true;
            }
            else if (c == ',' || c == ' ' || c == ';' || c == '\t')
                flush();
            else
            {
                inNum = true;   // "43x5" is one bad token, skipped whole
                bad = true;
            }
        }
        flush();
        return out;
    }

    inline bool Listed(std::vector<uint32_t> const& ids, uint32_t id)
    {
        for (uint32_t v : ids)
            if (v == id)
                return true;
        return false;
    }

    // parentOf(id) gives the AreaTable "zone" field of id (0 = it is a zone itself, or unknown).
    template <typename ParentOf>
    bool InAreas(uint32_t areaId, std::vector<uint32_t> const& ids, ParentOf parentOf)
    {
        for (int depth = 0; areaId && depth < MAX_DEPTH; ++depth)
        {
            if (Listed(ids, areaId))
                return true;
            areaId = parentOf(areaId);
        }
        return false;
    }

    struct LogoutAsk
    {
        bool enabled = false;      // LeylineQol.Enable and LeylineQol.Dalaran.InstantLogout
        bool onMap = false;        // the player's map is LeylineQol.Dalaran.MapId
        bool inArea = false;       // InAreas(the player's area, LeylineQol.Dalaran.LogoutAreas)
        bool inWorld = false;
        bool inCombat = false;
        bool falling = false;      // MOVEMENTFLAG_FALLING | MOVEMENTFLAG_FALLING_FAR
        bool dueling = false;      // player->duel (a request or a fight)
        bool frozen = false;       // aura 9454, the GM .freeze
        bool afkBlocked = false;   // the core's AFK rules (AFK.PreventLogout) would refuse the logout
    };

    // true: the module sends the instant logout answer and logs the player out; false: the core handles the request.
    inline bool InstantLogout(LogoutAsk const& a)
    {
        return a.enabled && a.inWorld && a.onMap && a.inArea
            && !a.inCombat && !a.falling && !a.dueling && !a.frozen && !a.afkBlocked;
    }

    struct AreaRow
    {
        uint32_t id = 0;
        uint32_t map = 0;
        uint32_t zone = 0;         // the parent (0 for a zone)
        uint32_t flags = 0;
    };

    // The rows that should get FLAG_ALLOW_DUELS (rows that already have it are left out: nothing to change).
    inline std::vector<uint32_t> DuelAreas(std::vector<AreaRow> const& rows, std::vector<uint32_t> const& ids, uint32_t map)
    {
        auto parentOf = [&rows](uint32_t id) -> uint32_t {
            for (AreaRow const& r : rows)
                if (r.id == id)
                    return r.zone;
            return 0;
        };
        std::vector<uint32_t> out;
        for (AreaRow const& r : rows)
            if (r.map == map && !(r.flags & FLAG_ALLOW_DUELS) && InAreas(r.id, ids, parentOf))
                out.push_back(r.id);
        return out;
    }

    inline std::string JoinIds(std::vector<uint32_t> const& ids)
    {
        std::string s;
        for (uint32_t v : ids)
        {
            if (!s.empty())
                s += ",";
            s += std::to_string(v);
        }
        return s;
    }
}

#endif
