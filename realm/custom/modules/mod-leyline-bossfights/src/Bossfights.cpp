/*
 * mod-leyline-bossfights: Leyline's "Boss fights" mode, realm side (docs/BOSS-FIGHTS.md).
 *
 * A player types (or the website's go link runs) ".bossfight join frozen-throne normal bots" and, a few seconds
 * later, stands with a 10-player raid on the platform of the ICC raid's final fight, in a fresh instance, with every
 * earlier boss already DONE. The clock starts when the boss engages (its boss state goes IN_PROGRESS) and stops when
 * it starts its 10% cast (spell 72350); a wipe (FAIL) is recorded and the raid can try again in the same instance.
 * Every attempt goes into characters.leyline_bossfight_runs, which realm/custom/bossfights-export.py turns into the
 * website's boards. Players see "The Frozen Throne"; the boss's own name is never written anywhere.
 *
 * Modes:  bots        the player and 9 bots (mod-playerbots addclass bots, given a role's talents and gear of one
 *                     fixed item level, so times compare); starts at once.
 *         group       a lobby of real players who asked for the same boss and difficulty; full (10) starts at once,
 *                     else after LeylineBossfight.LobbyWaitSec the empty seats are filled with bots.
 *         group-only  the same lobby, but the leader (the first in it) wants no bots: it starts when it is full.
 *         A player who leads a group of their own starts with that group at once (bots fill unless group-only).
 *
 * Bots: the official realm runs with Playerbots.MaxAddedBots = 0 (owner, 31 Oct: no bots). This module adds its own
 * bots anyway, but only for its own fights: it raises the cap for the moment of its own AddPlayerBot call and puts it
 * back at once, so no other path can add a bot. LeylineBossfight.Bots (env AC_LEYLINE_BOSSFIGHT_BOTS, or plain
 * LEYLINE_BOSSFIGHT_BOTS) = 0 turns this off (bots and group modes then wait for real players). Bots are logged out
 * when the fight is cleared, when the run ends, or when their leader leaves.
 *
 * Threads: the boss-state and spell hooks run inside map updates (map threads). They only queue an event (under a
 * mutex) for an instance this module made; everything else (teleports, groups, bots, SQL) runs in
 * WorldScript::OnUpdate, which World::Update calls after the map updates have finished.
 *
 * Unverified: written against the pinned core (mod-playerbots/azerothcore-wotlk 3f2969dd) and mod-playerbots
 * (e88cc409) by reading their sources; not compiled here. The wu-images workflow's realm job compiles it.
 */

#include "BossfightsLogic.h"

#include "CharacterCache.h"
#include "Chat.h"
#include "CommandScript.h"
#include "Config.h"
#include "DatabaseEnv.h"
#include "GameTime.h"
#include "Group.h"
#include "GroupMgr.h"
#include "InstanceSaveMgr.h"
#include "InstanceScript.h"
#include "Log.h"
#include "Map.h"
#include "MapMgr.h"
#include "ObjectAccessor.h"
#include "ObjectMgr.h"
#include "Player.h"
#include "ScriptMgr.h"
#include "SpellInfo.h"
#include "World.h"
#include "WorldPacket.h"
#include "WorldSession.h"

#if __has_include("PlayerbotMgr.h") && __has_include("PlayerbotFactory.h") && __has_include("RandomPlayerbotMgr.h")
#define LEYLINE_BF_PLAYERBOTS 1
#include "PlayerbotAI.h"
#include "PlayerbotAIConfig.h"
#include "PlayerbotFactory.h"
#include "PlayerbotMgr.h"
#include "RandomPlayerbotMgr.h"
#else
#define LEYLINE_BF_PLAYERBOTS 0
#endif

#include <chrono>
#include <cstdlib>
#include <functional>
#include <map>
#include <mutex>
#include <set>
#include <unordered_map>
#include <unordered_set>

using namespace Acore::ChatCommands;
using namespace LeylineBF;

namespace
{
    constexpr char const* TAG = "|cff9fd8ff[Boss fights]|r ";
    constexpr uint32 SPELL_TICK_MS = 250;

    struct BfSettings
    {
        bool enable = true;
        bool bots = true;              // LeylineBossfight.Bots / LEYLINE_BOSSFIGHT_BOTS
        uint32 minLevel = 80;
        uint32 lobbyWaitSec = 180;     // group lobby: bots fill after this
        uint32 lobbyMaxWaitSec = 1800; // group-only lobby: given up after this
        uint32 botGearLevel = 251;     // bots' item level (10-player tier 10)
        uint32 botWaitSec = 45;        // how long the start waits for bots to log in
        uint32 returnAfterClearSec = 120;
        uint32 wipeIdleSec = 900;      // a wiped raid that never tries again is sent home
        std::vector<Seat> seats = DefaultSeats();
    };

    enum class BfPhase { Forming, PortLeader, Prepare, Ready, Engaged, Wiped, Cleared };

    struct BfMember
    {
        ObjectGuid guid;
        std::string name;
        uint8 cls = 0;
        bool bot = false;
        bool present = false;    // seen inside the run's instance
        bool geared = false;     // bots: talents and gear done
        uint64 lastMoveMs = 0;   // our last teleport of this member (paces retries)
        uint8 spec = 0;          // bots: the seat's premade spec
        bool hasReturn = false;
        uint32 retMap = 0;
        float retX = 0, retY = 0, retZ = 0, retO = 0;
    };

    struct BfRun
    {
        uint32 id = 0;
        BossDef const* boss = nullptr;
        int diff = DIFF_NORMAL;
        int mode = MODE_BOTS;
        ObjectGuid leader;
        ObjectGuid groupGuid;
        std::vector<BfMember> members;
        std::vector<Seat> wantBots;        // seats still to fill
        BfPhase phase = BfPhase::Forming;
        uint64 phaseSince = 0;             // steady ms
        uint32 instanceId = 0;
        uint64 engageMs = 0;               // steady ms of the boss's IN_PROGRESS
        uint32 engageUnix = 0;
        uint64 lastTickMs = 0;
        uint32 wipes = 0;
        bool gm = false;
        bool botBuffs = false;
        std::string roster;                // at engage
        uint32 sizeAtEngage = 0;
        uint32 botsAtEngage = 0;
        bool botsDismissed = false;
    };

    struct LobbyEntry
    {
        ObjectGuid guid;
        bool groupOnly = false;
        uint64 joinedMs = 0;
        TeamId team = TEAM_NEUTRAL;
    };

    enum EventType { EV_ENGAGE, EV_WIPE, EV_DONE, EV_CLEAR, EV_REASSERT };

    struct BfEvent
    {
        EventType type;
        uint32 instanceId;
        uint64 ms;
    };

    BfSettings g_cfg;
    uint32 g_nextRunId = 1;
    std::map<uint32, BfRun> g_runs;                          // run id -> run
    std::unordered_map<ObjectGuid, uint32> g_memberRun;    // member (player or bot) -> run id
    std::unordered_map<uint32, uint32> g_instanceRun;      // instance id -> run id
    std::unordered_set<uint32> g_knownInstances;           // every instance this module ever made (persisted)
    std::map<std::string, std::vector<LobbyEntry>> g_lobbies;   // "boss|diff[|team]" -> waiting players
    std::map<std::string, uint64> g_lobbySent;             // last Q broadcast per lobby
    std::unordered_set<ObjectGuid> g_pendingUnbind;        // unbind from the boss maps once out of them
    uint32 g_updateAcc = 0;
    bool g_inLogout = false;                               // a master logging out: the bot module logs his bots out

    // shared with the map threads
    std::mutex g_eventLock;
    std::vector<BfEvent> g_events;
    std::unordered_set<uint32> g_liveInstances;            // instances with a run (copy of g_instanceRun's keys)

    uint64 NowMs()
    {
        return uint64(std::chrono::duration_cast<std::chrono::milliseconds>(
            std::chrono::steady_clock::now().time_since_epoch()).count());
    }

    uint32 NowUnix() { return uint32(GameTime::GetGameTime().count()); }

    void SetLive(uint32 instanceId, bool on)
    {
        std::lock_guard<std::mutex> guard(g_eventLock);
        if (on)
            g_liveInstances.insert(instanceId);
        else
            g_liveInstances.erase(instanceId);
    }

    void PushEvent(EventType type, uint32 instanceId)
    {
        uint64 ms = NowMs();
        std::lock_guard<std::mutex> guard(g_eventLock);
        if (g_liveInstances.count(instanceId))
            g_events.push_back({ type, instanceId, ms });
    }

    bool IsLive(uint32 instanceId)
    {
        std::lock_guard<std::mutex> guard(g_eventLock);
        return g_liveInstances.count(instanceId) != 0;
    }

    void LoadSettings()
    {
        BfSettings s;
        s.enable = sConfigMgr->GetOption<bool>("LeylineBossfight.Enable", true, false);
        s.bots = sConfigMgr->GetOption<bool>("LeylineBossfight.Bots", true, false);
        if (char const* env = std::getenv("LEYLINE_BOSSFIGHT_BOTS"))
            s.bots = std::string(env) != "0" && Lower(env) != "false" && Lower(env) != "off";
        s.minLevel = sConfigMgr->GetOption<uint32>("LeylineBossfight.MinLevel", 80, false);
        s.lobbyWaitSec = sConfigMgr->GetOption<uint32>("LeylineBossfight.LobbyWaitSec", 180, false);
        s.lobbyMaxWaitSec = sConfigMgr->GetOption<uint32>("LeylineBossfight.LobbyMaxWaitSec", 1800, false);
        s.botGearLevel = sConfigMgr->GetOption<uint32>("LeylineBossfight.BotGearLevel", 251, false);
        s.botWaitSec = sConfigMgr->GetOption<uint32>("LeylineBossfight.BotWaitSec", 45, false);
        s.returnAfterClearSec = sConfigMgr->GetOption<uint32>("LeylineBossfight.ReturnAfterClearSec", 120, false);
        s.wipeIdleSec = sConfigMgr->GetOption<uint32>("LeylineBossfight.WipeIdleSec", 900, false);
        std::string seats = sConfigMgr->GetOption<std::string>("LeylineBossfight.BotSeats", "", false);
        s.seats = seats.empty() ? DefaultSeats() : ParseSeats(seats);
#if !LEYLINE_BF_PLAYERBOTS
        s.bots = false;
#endif
        g_cfg = s;
    }

    bool BotsOn() { return g_cfg.bots && LEYLINE_BF_PLAYERBOTS; }

    // Playerbots.EnableICCBuffs (the bot module's heroic "assist" auras for the whole raid). Read from the same
    // config store; recorded with every run so the boards can tell assisted runs apart.
    bool BotBuffsOn()
    {
#if LEYLINE_BF_PLAYERBOTS
        return sPlayerbotAIConfig.EnableICCBuffs;
#else
        return sConfigMgr->GetOption<bool>("Playerbots.EnableICCBuffs", true, false);
#endif
    }

    void Say(Player* p, std::string const& text)
    {
        if (p && p->GetSession())
            ChatHandler(p->GetSession()).SendSysMessage(std::string(TAG) + text);
    }

    // LYBF addon message: a whisper from the player to themself, LANG_ADDON, "LYBF\t<payload>" (the client splits
    // the prefix at the first tab and fires CHAT_MSG_ADDON with distribution WHISPER, sender = the player).
    void Addon(Player* p, std::string const& payload)
    {
        if (!p || !p->GetSession() || !p->IsInWorld())
            return;
        WorldPacket data;
        ChatHandler::BuildChatPacket(data, CHAT_MSG_WHISPER, LANG_ADDON, p, p, std::string(ADDON_PREFIX) + "\t" + payload);
        p->GetSession()->SendPacket(&data);
    }

    bool IsBotPlayer(Player* p)
    {
#if LEYLINE_BF_PLAYERBOTS
        if (!p)
            return false;
        PlayerbotAI* ai = sPlayerbotsMgr.GetPlayerbotAI(p);
        return ai && !IsRealPlayer(p);
#else
        (void)p;
        return false;
#endif
    }

    BfRun* RunOf(ObjectGuid guid)
    {
        auto it = g_memberRun.find(guid);
        if (it == g_memberRun.end())
            return nullptr;
        auto r = g_runs.find(it->second);
        return r == g_runs.end() ? nullptr : &r->second;
    }

    BfMember* MemberOf(BfRun& run, ObjectGuid guid)
    {
        for (BfMember& m : run.members)
            if (m.guid == guid)
                return &m;
        return nullptr;
    }

    void ForHumans(BfRun& run, std::function<void(Player*, BfMember&)> fn)
    {
        for (BfMember& m : run.members)
            if (!m.bot)
                if (Player* p = ObjectAccessor::FindConnectedPlayer(m.guid))
                    if (p->IsInWorld())
                        fn(p, m);
    }

    void AddonRun(BfRun& run, std::string const& payload)
    {
        ForHumans(run, [&payload](Player* p, BfMember&) { Addon(p, payload); });
    }

    void SayRun(BfRun& run, std::string const& text)
    {
        ForHumans(run, [&text](Player* p, BfMember&) { Say(p, text); });
    }

    InstanceScript* ScriptOf(BfRun const& run)
    {
        if (!run.instanceId || !run.boss)
            return nullptr;
        Map* map = sMapMgr->FindMap(run.boss->mapId, run.instanceId);
        InstanceMap* im = map ? map->ToInstanceMap() : nullptr;
        return im ? im->GetInstanceScript() : nullptr;
    }

    // Every earlier boss DONE, through the base InstanceScript::SetBossState (a qualified, non-virtual call): the
    // state, the doors and the save, without the raid script's own extras (no gunship spawn, no unlocked loot
    // chests, no weekly quest, and the heroic "final fight available" flag left alone). A boss still at
    // TO_BE_DECIDED (its creature never loaded) takes NOT_STARTED first, since the first set from that state is
    // silent. Called at the start, each tick while the raid is not fighting, and when a boss's AI resets it.
    void EnsurePrereqs(BfRun& run)
    {
        InstanceScript* script = ScriptOf(run);
        if (!script)
            return;
        for (uint32 id : run.boss->prereq)
        {
            EncounterState st = script->GetBossState(id);
            if (st == DONE)
                continue;
            if (st == TO_BE_DECIDED)
                script->InstanceScript::SetBossState(id, NOT_STARTED);
            script->InstanceScript::SetBossState(id, DONE);
        }
    }

    void UnbindNow(Player* p, uint32 mapId)
    {
        for (uint8 d = 0; d < MAX_RAID_DIFFICULTY; ++d)
            if (sInstanceSaveMgr->PlayerGetBoundInstance(p->GetGUID(), mapId, Difficulty(d)))
                sInstanceSaveMgr->PlayerUnbindInstance(p->GetGUID(), mapId, Difficulty(d), true, p);
    }

    // Unbinding while inside the map is not safe (the core's own .instance unbind skips the current map): later.
    void Unbind(Player* p)
    {
        if (!p)
            return;
        bool inside = false;
        for (BossDef const& b : Bosses())
            inside = inside || p->GetMapId() == b.mapId;
        if (inside || p->IsBeingTeleportedFar())
        {
            g_pendingUnbind.insert(p->GetGUID());
            return;
        }
        for (BossDef const& b : Bosses())
            UnbindNow(p, b.mapId);
        g_pendingUnbind.erase(p->GetGUID());
    }

    void SendHome(Player* p, BfMember const* m)
    {
        if (!p)
            return;
        if (p->isDead())
        {
            p->ResurrectPlayer(1.0f);
            p->SpawnCorpseBones();
        }
        if (m && m->hasReturn)
            p->TeleportTo(m->retMap, m->retX, m->retY, m->retZ, m->retO);
        else
            p->TeleportTo(p->m_homebindMapId, p->m_homebindX, p->m_homebindY, p->m_homebindZ, p->GetOrientation());
        g_pendingUnbind.insert(p->GetGUID());
    }

    void Arrive(Player* p, BfRun const& run, uint32 n)
    {
        // a little spread so ten players don't stand in one spot
        float dx = float(n % 3) * 2.5f, dy = (float(n / 3) - 1.5f) * 2.5f;
        BossDef const& b = *run.boss;
        if (p->GetMapId() == b.mapId && p->GetInstanceId() == run.instanceId)
            p->NearTeleportTo(b.x + dx, b.y + dy, b.z, b.o);
        else
            p->TeleportTo(b.mapId, b.x + dx, b.y + dy, b.z, b.o, TELE_TO_GM_MODE);
    }

    // ---- bots -------------------------------------------------------------------------------------------------

    void RequestBots(BfRun& run, Player* leader)
    {
#if LEYLINE_BF_PLAYERBOTS
        if (run.wantBots.empty() || !leader)
            return;
        PlayerbotMgr* mgr = sPlayerbotsMgr.GetPlayerbotMgr(leader);
        if (!mgr)
        {
            Say(leader, "Bots can't join you right now; the fight starts with the players there are.");
            run.wantBots.clear();
            return;
        }
        bool alliance = leader->GetTeamId(true) == TEAM_ALLIANCE;
        int32 cap = sPlayerbotAIConfig.MaxAddedBots;
        // Only for this call: the realm's own cap (0 on the official realm) stays for every other path.
        sPlayerbotAIConfig.MaxAddedBots = std::max<int32>(cap, int32(mgr->GetPlayerbotsCount() + run.wantBots.size() + 1));
        std::vector<Seat> left;
        for (Seat const& seat : run.wantBots)
        {
            auto& pool = sRandomPlayerbotMgr.addclassCache[RandomPlayerbotMgr::GetTeamClassIdx(alliance, seat.cls)];
            ObjectGuid pick;
            for (ObjectGuid const& g : pool)
            {
                if (g_memberRun.count(g) || ObjectAccessor::FindConnectedPlayer(g))
                    continue;
                if (sCharacterCache->GetCharacterGuildIdByGuid(g))
                    continue;
                pick = g;
                break;
            }
            if (pick.IsEmpty())
            {
                left.push_back(seat);
                continue;
            }
            BfMember m;
            m.guid = pick;
            m.bot = true;
            m.cls = seat.cls;
            m.spec = seat.spec;
            sCharacterCache->GetCharacterNameByGuid(pick, m.name);
            run.members.push_back(m);
            g_memberRun[pick] = run.id;
            mgr->AddPlayerBot(pick, leader->GetSession()->GetAccountId());
        }
        sPlayerbotAIConfig.MaxAddedBots = cap;
        if (!left.empty())
            Say(leader, std::to_string(left.size()) + " bot seat(s) couldn't be filled (no free bot of that class).");
        run.wantBots.clear();
#else
        (void)leader;
        run.wantBots.clear();
#endif
    }

    // The seat's premade talents (realm/playerbots-specs.env), then gear of one fixed item level, every time:
    // bots in one board fight with the same kit whoever brings them.
    void PrepareBot(Player* bot, BfMember& m)
    {
#if LEYLINE_BF_PLAYERBOTS
        if (m.geared)
            return;
        m.geared = true;
        PlayerbotFactory::InitTalentsBySpecNo(bot, m.spec, true);
        PlayerbotFactory::DestroyEquippedGear(bot);
        PlayerbotFactory::AutoGear(bot, ITEM_QUALITY_EPIC, g_cfg.botGearLevel, false, false, true);
        bot->SetFullHealth();
        if (PlayerbotAI* ai = sPlayerbotsMgr.GetPlayerbotAI(bot))
            ai->ResetStrategies();
#else
        (void)bot;
        m.geared = true;
#endif
    }

    void DismissBots(BfRun& run)
    {
        if (run.botsDismissed)
            return;
        run.botsDismissed = true;
#if LEYLINE_BF_PLAYERBOTS
        Player* leader = ObjectAccessor::FindConnectedPlayer(run.leader);
        PlayerbotMgr* mgr = leader && !g_inLogout ? sPlayerbotsMgr.GetPlayerbotMgr(leader) : nullptr;
        Group* group = sGroupMgr->GetGroupByGUID(run.groupGuid.GetCounter());
        for (BfMember const& m : run.members)
        {
            if (!m.bot)
                continue;
            g_memberRun.erase(m.guid);
            if (group && group->IsMember(m.guid))
                group = group->RemoveMember(m.guid) ? sGroupMgr->GetGroupByGUID(run.groupGuid.GetCounter()) : nullptr;
            if (mgr && mgr->GetPlayerBot(m.guid))
                mgr->LogoutPlayerBot(m.guid);
        }
#endif
        run.members.erase(std::remove_if(run.members.begin(), run.members.end(), [](BfMember const& m) { return m.bot; }),
            run.members.end());
    }

    // ---- records ----------------------------------------------------------------------------------------------

    void SnapshotRoster(BfRun& run)
    {
        std::vector<RosterEntry> roster;
        uint32 bots = 0;
        for (BfMember& m : run.members)
        {
            Player* p = ObjectAccessor::FindConnectedPlayer(m.guid);
            if (!p || !p->IsInWorld() || p->GetMapId() != run.boss->mapId || p->GetInstanceId() != run.instanceId)
                continue;
            roster.push_back({ p->GetName(), p->getClass(), m.bot });
            bots += m.bot ? 1 : 0;
        }
        run.roster = RosterText(roster);
        run.sizeAtEngage = uint32(roster.size());
        run.botsAtEngage = bots;
    }

    // The place this time would take on its board (clears of the same boss, difficulty and kind, GM runs left out).
    long long RankHint(BfRun const& run, uint64 ms)
    {
        if (run.gm)
            return 0;
        QueryResult r = CharacterDatabase.Query("SELECT COUNT(*) FROM leyline_bossfight_runs WHERE boss = '{}' AND difficulty = '{}' "
            "AND result = 'clear' AND gm = 0 AND tier = 0 AND (bots > 0) = {} AND ms < {}",
            run.boss->key, DifficultyName(run.diff), run.botsAtEngage > 0 ? 1 : 0, ms);
        return r ? (long long)(*r)[0].Get<uint64>() + 1 : 0;
    }

    void Record(BfRun& run, char const* result, uint64 ms)
    {
        std::string roster = run.roster;
        CharacterDatabase.EscapeString(roster);
        CharacterDatabase.Execute("INSERT INTO leyline_bossfight_runs (boss, difficulty, mode, size, ms, started_at, ended_at, "
            "leader_guid, instance_id, roster, bots, bot_buffs, gm, tier, wipes_before, result) VALUES "
            "('{}', '{}', '{}', {}, {}, {}, {}, {}, {}, '{}', {}, {}, {}, 0, {}, '{}')",
            run.boss->key, DifficultyName(run.diff), ModeName(run.mode), run.sizeAtEngage, ms, run.engageUnix, NowUnix(),
            run.leader.GetCounter(), run.instanceId, roster, run.botsAtEngage, run.botBuffs ? 1 : 0, run.gm ? 1 : 0,
            run.wipes, result);
    }

    // ---- ending -----------------------------------------------------------------------------------------------

    void SetPhase(BfRun& run, BfPhase p)
    {
        run.phase = p;
        run.phaseSince = NowMs();
    }

    void EndRun(uint32 runId, std::string const& why, bool sendHome)
    {
        auto it = g_runs.find(runId);
        if (it == g_runs.end())
            return;
        BfRun& run = it->second;
        if (run.phase == BfPhase::Engaged)
            Record(run, "abandon", NowMs() - run.engageMs);
        DismissBots(run);
        ForHumans(run, [&](Player* p, BfMember& m)
        {
            Addon(p, MsgLeft());
            if (!why.empty())
                Say(p, why);
            if (sendHome && run.instanceId && p->GetMapId() == run.boss->mapId && p->GetInstanceId() == run.instanceId)
                SendHome(p, &m);
            else
                Unbind(p);
        });
        for (BfMember const& m : run.members)
        {
            g_memberRun.erase(m.guid);
            if (!m.bot)
                g_pendingUnbind.insert(m.guid);
        }
        if (run.instanceId)
        {
            g_instanceRun.erase(run.instanceId);
            SetLive(run.instanceId, false);
        }
        g_runs.erase(it);
    }

    // One player leaves (.bossfight leave, logout, left the group or the instance). The leader leaving ends it.
    void LeaveRun(Player* p, ObjectGuid guid, bool sendHome, std::string const& why)
    {
        BfRun* run = RunOf(guid);
        if (!run)
            return;
        if (run->leader == guid)
        {
            EndRun(run->id, why.empty() ? "The raid's leader left: the boss fight is over." : why, true);
            return;
        }
        BfMember* m = MemberOf(*run, guid);
        BfMember copy = m ? *m : BfMember();
        g_memberRun.erase(guid);
        run->members.erase(std::remove_if(run->members.begin(), run->members.end(), [guid](BfMember const& x) { return x.guid == guid; }),
            run->members.end());
        if (p)
        {
            Addon(p, MsgLeft());
            if (!why.empty())
                Say(p, why);
            if (sendHome && p->GetMapId() == run->boss->mapId && p->GetInstanceId() == run->instanceId)
                SendHome(p, &copy);
            else
                Unbind(p);
            if (Group* g = p->GetGroup())
                if (g->GetGUID() == run->groupGuid)
                    p->RemoveFromGroup();
        }
        g_pendingUnbind.insert(guid);
        SayRun(*run, copy.name + " left the boss fight.");
    }

    void LeaveLobby(ObjectGuid guid)
    {
        for (auto& kv : g_lobbies)
            kv.second.erase(std::remove_if(kv.second.begin(), kv.second.end(), [guid](LobbyEntry const& e) { return e.guid == guid; }),
                kv.second.end());
    }

    std::string LobbyKeyOf(ObjectGuid guid)
    {
        for (auto const& kv : g_lobbies)
            for (LobbyEntry const& e : kv.second)
                if (e.guid == guid)
                    return kv.first;
        return "";
    }

    // ---- starting ---------------------------------------------------------------------------------------------

    // Why this player can't start or queue right now ("" = fine).
    std::string Blocked(Player* p)
    {
        if (!p->IsInWorld() || p->IsBeingTeleported())
            return "wait a moment (you're on the move)";
        if (p->GetLevel() < g_cfg.minLevel)
            return "boss fights need level " + std::to_string(g_cfg.minLevel);
        if (p->IsInCombat())
            return "not in combat";
        if (!p->IsAlive())
            return "not while dead";
        if (p->InBattleground() || p->InArena() || p->InBattlegroundQueue())
            return "not from a battleground, an arena or their queue";
        if (p->GetMap() && (p->GetMap()->IsDungeon() || p->GetMap()->IsBattlegroundOrArena()))
            return "not from inside an instance";
        return "";
    }

    int RoleOfPlayer(Player* p) { return RoleOf(p->getClass(), p->GetMostPointsTalentTree()); }

    // humans[0] is the leader. fill: bots take the empty seats.
    uint32 StartRun(std::vector<Player*> const& humans, BossDef const* boss, int diff, int mode, bool fill, bool gm)
    {
        if (humans.empty() || !boss)
            return 0;
        Player* leader = humans[0];
        BfRun run;
        run.id = g_nextRunId++;
        run.boss = boss;
        run.diff = diff;
        run.mode = mode;
        run.leader = leader->GetGUID();
        run.gm = gm;
        SetPhase(run, BfPhase::Forming);

        // the raid group: the leader's own (a premade), else a new one
        Group* group = leader->GetGroup();
        if (!group)
        {
            group = new Group();
            if (!group->Create(leader))
            {
                delete group;
                Say(leader, "The raid group couldn't be made. Try again in a moment.");
                return 0;
            }
            sGroupMgr->AddGroup(group);
        }
        if (!group->isRaidGroup())
            group->ConvertToRaid();
        run.groupGuid = group->GetGUID();

        std::vector<int> roles;
        for (Player* p : humans)
        {
            if (p != leader && p->GetGroup() != group && !group->IsMember(p->GetGUID()))
            {
                if (p->GetGroup() || !group->AddMember(p))
                {
                    Say(p, "You couldn't be added to the raid (are you in another group?).");
                    continue;
                }
            }
            BfMember m;
            m.guid = p->GetGUID();
            m.name = p->GetName();
            m.cls = p->getClass();
            m.hasReturn = true;
            m.retMap = p->GetMapId();
            m.retX = p->GetPositionX();
            m.retY = p->GetPositionY();
            m.retZ = p->GetPositionZ();
            m.retO = p->GetOrientation();
            run.members.push_back(m);
            roles.push_back(RoleOfPlayer(p));
            g_memberRun[m.guid] = run.id;
            LeaveLobby(m.guid);
            if (p->GetSession()->GetSecurity() > SEC_PLAYER || p->IsGameMaster())
                run.gm = true;
            for (BossDef const& b : Bosses())
                UnbindNow(p, b.mapId);   // checked above: nobody here is inside an instance
        }
        group->SetRaidDifficulty(diff == DIFF_HEROIC ? RAID_DIFFICULTY_10MAN_HEROIC : RAID_DIFFICULTY_10MAN_NORMAL);

        if (fill && BotsOn())
            run.wantBots = BotSeats(g_cfg.seats, roles, boss->size);

        uint32 id = run.id;
        BfRun& stored = g_runs.emplace(id, std::move(run)).first->second;
        AddonRun(stored, MsgStarting(boss->key, diff));
        SayRun(stored, std::string("Starting ") + boss->title + " (" + DifficultyName(diff) + ")" +
            (stored.wantBots.empty() ? "" : ", with " + std::to_string(stored.wantBots.size()) + " bots") + "...");
        RequestBots(stored, leader);
        return id;
    }

    // After a wipe (or to line up again): everyone back on the platform, alive, in this instance.
    void Restart(BfRun& run)
    {
        EnsurePrereqs(run);
        uint32 n = 0;
        for (BfMember& m : run.members)
        {
            Player* p = ObjectAccessor::FindConnectedPlayer(m.guid);
            if (!p || !p->IsInWorld())
                continue;
            if (p->isDead())
            {
                p->ResurrectPlayer(1.0f);
                p->SpawnCorpseBones();
            }
            p->SetFullHealth();
            Arrive(p, run, n++);
        }
        SetPhase(run, BfPhase::Ready);
        AddonRun(run, MsgStarting(run.boss->key, run.diff));
        SayRun(run, "Ready again. Speak to the paladin in front of the throne to begin.");
    }

    // ---- events from the map threads --------------------------------------------------------------------------

    void HandleEvent(BfEvent const& ev)
    {
        auto ir = g_instanceRun.find(ev.instanceId);
        if (ir == g_instanceRun.end())
            return;
        auto rr = g_runs.find(ir->second);
        if (rr == g_runs.end())
            return;
        BfRun& run = rr->second;
        switch (ev.type)
        {
            case EV_REASSERT:
                if (run.phase != BfPhase::Engaged)
                    EnsurePrereqs(run);
                break;
            case EV_ENGAGE:
                if (run.phase == BfPhase::Ready || run.phase == BfPhase::Wiped || run.phase == BfPhase::Prepare)
                {
                    SetPhase(run, BfPhase::Engaged);
                    run.engageMs = ev.ms;
                    run.engageUnix = NowUnix();
                    run.lastTickMs = ev.ms;
                    run.botBuffs = BotBuffsOn();
                    SnapshotRoster(run);
                    AddonRun(run, MsgTimer(0));
                }
                break;
            case EV_CLEAR:
            case EV_DONE:
                if (run.phase == BfPhase::Engaged)
                {
                    uint64 ms = ev.ms > run.engageMs ? ev.ms - run.engageMs : 0;
                    long long rank = RankHint(run, ms);
                    Record(run, "clear", ms);
                    SetPhase(run, BfPhase::Cleared);
                    AddonRun(run, MsgResult(true, ms, rank));
                    SayRun(run, std::string("Cleared in ") + FormatMs(ms) + (rank > 0 ? " (#" + std::to_string(rank) + " on its board)" : "") +
                        (run.gm ? ". A GM was in this raid, so it won't go on the boards." : ".") +
                        " You go back in " + std::to_string(g_cfg.returnAfterClearSec) + " s, or type .bossfight leave.");
                }
                break;
            case EV_WIPE:
                if (run.phase == BfPhase::Engaged)
                {
                    uint64 ms = ev.ms > run.engageMs ? ev.ms - run.engageMs : 0;
                    Record(run, "wipe", ms);
                    ++run.wipes;
                    SetPhase(run, BfPhase::Wiped);
                    AddonRun(run, MsgResult(false, ms, 0));
                    SayRun(run, "Wiped at " + FormatMs(ms) + ". Try again: .bossfight join " + run.boss->key + " " +
                        DifficultyName(run.diff) + " " + ModeName(run.mode) + " (or .bossfight leave).");
                }
                break;
        }
    }

    // ---- the tick -------------------------------------------------------------------------------------------

    void TickRun(BfRun& run)
    {
        uint64 now = NowMs();
        Player* leader = ObjectAccessor::FindConnectedPlayer(run.leader);
        if (!leader)
        {
            EndRun(run.id, "The raid's leader left: the boss fight is over.", true);
            return;
        }
        BossDef const& b = *run.boss;

        // GMs: a member who is (or turns into) a GM marks the run; it never reaches the boards
        ForHumans(run, [&run](Player* p, BfMember&)
        {
            if (p->GetSession()->GetSecurity() > SEC_PLAYER || p->IsGameMaster())
                run.gm = true;
        });

        // members who left the raid group or the instance (after they were in it) are out
        if (run.phase >= BfPhase::Ready)
        {
            std::vector<ObjectGuid> gone;
            for (BfMember& m : run.members)
            {
                if (m.bot)
                    continue;
                Player* p = ObjectAccessor::FindConnectedPlayer(m.guid);
                if (!p || !p->IsInWorld() || p->IsBeingTeleported())
                    continue;
                bool inside = p->GetMapId() == b.mapId && p->GetInstanceId() == run.instanceId;
                bool grouped = p->GetGroup() && p->GetGroup()->GetGUID() == run.groupGuid;
                if (inside)
                    m.present = true;
                else if (!grouped || (m.present && p->IsAlive()))
                    gone.push_back(m.guid);   // a released ghost outside stays: it is brought back when the raid isn't fighting
                if (inside && !grouped)
                    gone.push_back(m.guid);
            }
            for (ObjectGuid g : gone)
            {
                if (g == run.leader)
                {
                    EndRun(run.id, "The raid's leader left: the boss fight is over.", true);
                    return;
                }
                LeaveRun(ObjectAccessor::FindConnectedPlayer(g), g, false, "You left the boss fight.");
            }
        }

        switch (run.phase)
        {
            case BfPhase::Forming:
            {
                bool allIn = true;
                for (BfMember const& m : run.members)
                {
                    if (!m.bot)
                        continue;
                    Player* bot = ObjectAccessor::FindConnectedPlayer(m.guid);
                    Group* g = bot ? bot->GetGroup() : nullptr;
                    if (!bot || !bot->IsInWorld() || !g || g->GetGUID() != run.groupGuid)
                        allIn = false;
                }
                if (!allIn && now - run.phaseSince < uint64(g_cfg.botWaitSec) * 1000)
                    break;
                if (!allIn)
                {
                    // the bots that never came are dropped (and logged out if they did log in)
                    std::vector<ObjectGuid> late;
                    for (BfMember const& m : run.members)
                    {
                        if (!m.bot)
                            continue;
                        Player* bot = ObjectAccessor::FindConnectedPlayer(m.guid);
                        Group* g = bot ? bot->GetGroup() : nullptr;
                        if (!bot || !bot->IsInWorld() || !g || g->GetGUID() != run.groupGuid)
                            late.push_back(m.guid);
                    }
#if LEYLINE_BF_PLAYERBOTS
                    if (PlayerbotMgr* mgr = sPlayerbotsMgr.GetPlayerbotMgr(leader))
                        for (ObjectGuid g : late)
                            if (mgr->GetPlayerBot(g))
                                mgr->LogoutPlayerBot(g);
#endif
                    for (ObjectGuid g : late)
                    {
                        g_memberRun.erase(g);
                        run.members.erase(std::remove_if(run.members.begin(), run.members.end(), [g](BfMember const& x) { return x.guid == g; }),
                            run.members.end());
                    }
                    Say(leader, std::to_string(late.size()) + " bot(s) didn't arrive in time; starting without them.");
                }
                if (Group* g = leader->GetGroup())
                    g->SetRaidDifficulty(run.diff == DIFF_HEROIC ? RAID_DIFFICULTY_10MAN_HEROIC : RAID_DIFFICULTY_10MAN_NORMAL);
                std::string why = Blocked(leader);
                if (!why.empty())
                {
                    EndRun(run.id, "The boss fight couldn't start: " + why + ".", false);
                    return;
                }
                Arrive(leader, run, 0);
                SetPhase(run, BfPhase::PortLeader);
                break;
            }
            case BfPhase::PortLeader:
            {
                if (leader->IsInWorld() && !leader->IsBeingTeleported() && leader->GetMapId() == b.mapId && leader->GetInstanceId())
                {
                    run.instanceId = leader->GetInstanceId();
                    if (g_instanceRun.count(run.instanceId))
                    {
                        EndRun(run.id, "That instance is already in use; try again.", true);
                        return;
                    }
                    g_instanceRun[run.instanceId] = run.id;
                    g_knownInstances.insert(run.instanceId);
                    SetLive(run.instanceId, true);
                    CharacterDatabase.Execute("REPLACE INTO leyline_bossfight_instances (instance_id, boss, created_at) VALUES ({}, '{}', {})",
                        run.instanceId, b.key, NowUnix());
                    if (MemberOf(run, run.leader))
                        MemberOf(run, run.leader)->present = true;
                    SetPhase(run, BfPhase::Prepare);
                }
                else if (now - run.phaseSince > 30000)
                {
                    EndRun(run.id, "The raid couldn't enter (level, lockout or group rule). Nothing was recorded.", true);
                    return;
                }
                break;
            }
            case BfPhase::Prepare:
            {
                EnsurePrereqs(run);
                uint32 n = 1;
                for (BfMember& m : run.members)
                {
                    if (m.guid == run.leader)
                        continue;
                    Player* p = ObjectAccessor::FindConnectedPlayer(m.guid);
                    if (!p || !p->IsInWorld())
                        continue;
                    if (m.bot)
                        PrepareBot(p, m);
                    Arrive(p, run, n++);
                }
                SetPhase(run, BfPhase::Ready);
                SayRun(run, std::string(b.title) + " (" + DifficultyName(run.diff) + "): speak to the paladin in front of the throne to begin. "
                    "The clock starts when the fight does.");
                break;
            }
            case BfPhase::Ready:
            {
                EnsurePrereqs(run);
                // the fence: anyone off the platform (fallen, released, sent down by the script) comes back, alive
                uint32 n = 0;
                for (BfMember& m : run.members)
                {
                    Player* p = ObjectAccessor::FindConnectedPlayer(m.guid);
                    if (!p || !p->IsInWorld() || p->IsBeingTeleported())
                        continue;
                    bool inside = p->GetMapId() == b.mapId && p->GetInstanceId() == run.instanceId;
                    if (!inside)
                    {
                        // a bot that logged in late, a player whose teleport didn't happen, a released ghost outside
                        if (now - m.lastMoveMs < 10000)
                            continue;
                        m.lastMoveMs = now;
                        if (m.bot && !m.geared)
                            PrepareBot(p, m);
                        if (p->isDead())
                        {
                            p->ResurrectPlayer(1.0f);
                            p->SpawnCorpseBones();
                        }
                        Arrive(p, run, n++);
                        continue;
                    }
                    float dx = p->GetPositionX() - b.fenceX, dy = p->GetPositionY() - b.fenceY;
                    if (dx * dx + dy * dy > b.fenceR * b.fenceR || p->GetPositionZ() < b.fenceMinZ || p->isDead())
                    {
                        if (p->isDead())
                        {
                            p->ResurrectPlayer(1.0f);
                            p->SpawnCorpseBones();
                        }
                        Arrive(p, run, n++);
                    }
                }
                break;
            }
            case BfPhase::Engaged:
            {
                if (now - run.lastTickMs >= 5000)
                {
                    run.lastTickMs = now;
                    AddonRun(run, MsgTimer(now - run.engageMs));
                }
                break;
            }
            case BfPhase::Wiped:
            {
                if (now - run.phaseSince > uint64(g_cfg.wipeIdleSec) * 1000)
                {
                    EndRun(run.id, "Nobody tried again for a while: the boss fight is over.", true);
                    return;
                }
                break;
            }
            case BfPhase::Cleared:
            {
                if (!run.botsDismissed && now - run.phaseSince > 15000)
                    DismissBots(run);
                if (now - run.phaseSince > uint64(g_cfg.returnAfterClearSec) * 1000)
                {
                    EndRun(run.id, "", true);
                    return;
                }
                break;
            }
        }
    }

    std::string LobbyKey(BossDef const* boss, int diff, TeamId team)
    {
        std::string k = std::string(boss->key) + "|" + DifficultyName(diff);
        if (!sWorld->getBoolConfig(CONFIG_ALLOW_TWO_SIDE_INTERACTION_GROUP))
            k += team == TEAM_ALLIANCE ? "|a" : "|h";
        return k;
    }

    void SendQueued(std::string const& key, std::vector<LobbyEntry> const& list, long long eta)
    {
        std::string bossKey = key.substr(0, key.find('|'));
        BossDef const* boss = FindBoss(bossKey);
        if (!boss)
            return;
        int diff = key.find("|heroic") != std::string::npos ? DIFF_HEROIC : DIFF_NORMAL;
        bool leaderOnly = !list.empty() && list.front().groupOnly;
        for (LobbyEntry const& e : list)
            if (Player* p = ObjectAccessor::FindConnectedPlayer(e.guid))
                Addon(p, MsgQueued(boss->key, diff, leaderOnly ? MODE_GROUP_ONLY : MODE_GROUP, list.size(),
                    list.size() >= boss->size ? 0 : boss->size - list.size(), eta));
        g_lobbySent[key] = NowMs();
    }

    void TickLobbies()
    {
        uint64 now = NowMs();
        for (auto it = g_lobbies.begin(); it != g_lobbies.end();)
        {
            std::vector<LobbyEntry>& list = it->second;
            std::size_t before = list.size();
            list.erase(std::remove_if(list.begin(), list.end(), [](LobbyEntry const& e)
            {
                Player* p = ObjectAccessor::FindConnectedPlayer(e.guid);
                if (!p)
                    return true;
                Group* g = p->GetGroup();
                return g && g->GetMembersCount() > 1;   // joined someone's group meanwhile
            }), list.end());
            if (list.empty())
            {
                g_lobbySent.erase(it->first);
                it = g_lobbies.erase(it);
                continue;
            }
            std::string key = it->first;
            BossDef const* boss = FindBoss(key.substr(0, key.find('|')));
            int diff = key.find("|heroic") != std::string::npos ? DIFF_HEROIC : DIFF_NORMAL;
            LobbyDecision d = DecideLobby(list.size(), boss->size, now - list.front().joinedMs,
                uint64(g_cfg.lobbyWaitSec) * 1000, uint64(g_cfg.lobbyMaxWaitSec) * 1000, list.front().groupOnly, BotsOn());
            if (d.action == LOBBY_START || d.action == LOBBY_START_FILL)
            {
                std::vector<Player*> humans;
                std::vector<ObjectGuid> waiting;
                for (LobbyEntry const& e : list)
                {
                    if (humans.size() >= boss->size)
                        break;
                    Player* p = ObjectAccessor::FindConnectedPlayer(e.guid);
                    if (!p)
                        continue;
                    std::string why = Blocked(p);
                    if (!why.empty())
                    {
                        Say(p, "Your boss fight is starting, but " + why + ": you stay in the queue.");
                        continue;
                    }
                    humans.push_back(p);
                }
                if (!humans.empty())
                {
                    int mode = list.front().groupOnly ? MODE_GROUP_ONLY : MODE_GROUP;
                    StartRun(humans, boss, diff, mode, d.action == LOBBY_START_FILL, false);   // takes them out of the lobby
                    ++it;
                    continue;
                }
            }
            else if (d.action == LOBBY_EXPIRE)
            {
                for (LobbyEntry const& e : list)
                    if (Player* p = ObjectAccessor::FindConnectedPlayer(e.guid))
                    {
                        Addon(p, MsgLeft());
                        Say(p, "Not enough players came for a full raid; the queue is closed. Try again later, or with bots.");
                    }
                g_lobbySent.erase(key);
                it = g_lobbies.erase(it);
                continue;
            }
            auto sent = g_lobbySent.find(key);
            if (list.size() != before || sent == g_lobbySent.end() || now - sent->second >= 15000)
                SendQueued(key, list, d.etaSec);
            ++it;
        }
    }

    void TickUnbinds()
    {
        for (auto it = g_pendingUnbind.begin(); it != g_pendingUnbind.end();)
        {
            Player* p = ObjectAccessor::FindConnectedPlayer(*it);
            if (!p || !p->IsInWorld() || p->IsBeingTeleported() || RunOf(*it))
            {
                if (RunOf(*it))
                    it = g_pendingUnbind.erase(it);
                else
                    ++it;
                continue;
            }
            bool inside = false;
            for (BossDef const& b : Bosses())
                inside = inside || p->GetMapId() == b.mapId;
            if (inside)
            {
                ++it;
                continue;
            }
            for (BossDef const& b : Bosses())
                UnbindNow(p, b.mapId);
            it = g_pendingUnbind.erase(it);
        }
    }

    void Tick()
    {
        std::vector<BfEvent> events;
        {
            std::lock_guard<std::mutex> guard(g_eventLock);
            events.swap(g_events);
        }
        for (BfEvent const& ev : events)
            HandleEvent(ev);
        TickLobbies();
        std::vector<uint32> ids;
        for (auto const& kv : g_runs)
            ids.push_back(kv.first);
        for (uint32 id : ids)
        {
            auto it = g_runs.find(id);
            if (it != g_runs.end())
                TickRun(it->second);
        }
        TickUnbinds();
    }

    // ---- commands ---------------------------------------------------------------------------------------------

    std::string Arg(Optional<std::string> const& a) { return a ? *a : std::string(); }

    void StatusTo(ChatHandler* handler, Player* p)
    {
        if (BfRun* run = RunOf(p->GetGUID()))
        {
            std::string phase;
            switch (run->phase)
            {
                case BfPhase::Forming: case BfPhase::PortLeader: case BfPhase::Prepare: phase = "starting"; break;
                case BfPhase::Ready: phase = "ready: speak to the paladin in front of the throne"; break;
                case BfPhase::Engaged: phase = "fighting, " + FormatMs(NowMs() - run->engageMs); break;
                case BfPhase::Wiped: phase = "wiped (" + std::to_string(run->wipes) + "): .bossfight join to try again"; break;
                case BfPhase::Cleared: phase = "cleared"; break;
            }
            uint32 bots = 0;
            for (BfMember const& m : run->members)
                bots += m.bot ? 1 : 0;
            handler->SendSysMessage(std::string(TAG) + run->boss->title + " (" + DifficultyName(run->diff) + ", " + ModeName(run->mode) +
                "): " + phase + ". " + std::to_string(run->members.size() - bots) + " player(s), " + std::to_string(bots) + " bot(s)." +
                (run->gm ? " Not for the boards (a GM is in it)." : ""));
            if (run->phase == BfPhase::Engaged)
                Addon(p, MsgTimer(NowMs() - run->engageMs));
            else if (run->phase != BfPhase::Cleared)
                Addon(p, MsgStarting(run->boss->key, run->diff));
            return;
        }
        std::string key = LobbyKeyOf(p->GetGUID());
        if (!key.empty())
        {
            auto const& list = g_lobbies[key];
            handler->SendSysMessage(std::string(TAG) + "Waiting for players: " + std::to_string(list.size()) + " in the queue. .bossfight leave to stop.");
            g_lobbySent.erase(key);   // the next tick sends Q again
            return;
        }
        handler->SendSysMessage(std::string(TAG) + "You're not in a boss fight. .bossfight list shows them; "
            ".bossfight join frozen-throne normal bots starts one.");
    }

    bool HandleJoin(ChatHandler* handler, Optional<std::string> bossArg, Optional<std::string> diffArg, Optional<std::string> modeArg)
    {
        Player* p = handler->GetSession() ? handler->GetSession()->GetPlayer() : nullptr;
        if (!p)
            return true;
        if (!g_cfg.enable)
        {
            handler->SendSysMessage(std::string(TAG) + "Boss fights are off on this realm.");
            return true;
        }
        JoinArgs a = ParseJoin(Arg(bossArg), Arg(diffArg), Arg(modeArg));
        if (!a.error.empty())
        {
            handler->SendSysMessage(std::string(TAG) + a.error + ". Usage: .bossfight join <boss> <normal|heroic> <bots|group|group-only>");
            return true;
        }
        // already in one: after a wipe, the same join means "again, here" (the in-game panel's Try again)
        if (BfRun* run = RunOf(p->GetGUID()))
        {
            if (run->boss != a.boss || run->diff != a.diff)
            {
                handler->SendSysMessage(std::string(TAG) + "You're already in a boss fight. .bossfight leave first.");
                return true;
            }
            if (run->phase == BfPhase::Wiped || run->phase == BfPhase::Ready)
                Restart(*run);
            else if (run->phase == BfPhase::Engaged)
                handler->SendSysMessage(std::string(TAG) + "The fight is on!");
            else
                StatusTo(handler, p);
            return true;
        }
        if (!LobbyKeyOf(p->GetGUID()).empty())
        {
            StatusTo(handler, p);
            return true;
        }
        std::string why = Blocked(p);
        if (!why.empty())
        {
            handler->SendSysMessage(std::string(TAG) + "Not now: " + why + ".");
            return true;
        }
        bool fill = a.mode != MODE_GROUP_ONLY;
        if (a.mode == MODE_BOTS && !BotsOn())
        {
            handler->SendSysMessage(std::string(TAG) + "Bots aren't on for boss fights on this realm: join with group (or group-only) "
                "to raid with other players.");
            return true;
        }
        // a group of their own: its leader starts with it at once
        if (Group* g = p->GetGroup(); g && g->GetMembersCount() > 1)
        {
            if (!g->IsLeader(p->GetGUID()))
            {
                handler->SendSysMessage(std::string(TAG) + "You're in a group: its leader starts the boss fight for all of you.");
                return true;
            }
            if (g->isLFGGroup() || g->isBGGroup() || g->isBFGroup())
            {
                handler->SendSysMessage(std::string(TAG) + "Not with a Dungeon Finder or battleground group.");
                return true;
            }
            std::vector<Player*> humans{ p };
            for (GroupReference* ref = g->GetFirstMember(); ref; ref = ref->next())
            {
                Player* m = ref->GetSource();
                if (!m || m == p || IsBotPlayer(m))
                    continue;
                std::string mw = Blocked(m);
                if (!mw.empty() || RunOf(m->GetGUID()))
                {
                    handler->SendSysMessage(std::string(TAG) + m->GetName() + " can't come: " + (mw.empty() ? "already in a boss fight" : mw) + ".");
                    return true;
                }
                humans.push_back(m);
            }
            if (humans.size() > a.boss->size)
            {
                handler->SendSysMessage(std::string(TAG) + "Your group is bigger than this fight's " + std::to_string(a.boss->size) + " seats.");
                return true;
            }
            StartRun(humans, a.boss, a.diff, a.mode, fill, false);
            return true;
        }
        if (a.mode == MODE_BOTS)
        {
            StartRun({ p }, a.boss, a.diff, a.mode, true, false);
            return true;
        }
        std::string key = LobbyKey(a.boss, a.diff, p->GetTeamId(true));
        LobbyEntry e;
        e.guid = p->GetGUID();
        e.groupOnly = a.mode == MODE_GROUP_ONLY;
        e.joinedMs = NowMs();
        e.team = p->GetTeamId(true);
        g_lobbies[key].push_back(e);
        g_lobbySent.erase(key);
        std::vector<LobbyEntry> const& list = g_lobbies[key];
        bool leaderOnly = list.front().groupOnly;
        handler->SendSysMessage(std::string(TAG) + "In the queue for " + a.boss->title + " (" + DifficultyName(a.diff) + "): " +
            std::to_string(list.size()) + " of " + std::to_string(a.boss->size) + ". " +
            (leaderOnly || !BotsOn() ? "It starts when the raid is full." :
                "If it isn't full in " + std::to_string(g_cfg.lobbyWaitSec / 60) + " min, bots take the empty seats.") +
            " .bossfight leave to stop.");
        return true;
    }

    bool HandleLeave(ChatHandler* handler)
    {
        Player* p = handler->GetSession() ? handler->GetSession()->GetPlayer() : nullptr;
        if (!p)
            return true;
        if (RunOf(p->GetGUID()))
        {
            LeaveRun(p, p->GetGUID(), true, "You left the boss fight.");
            return true;
        }
        if (!LobbyKeyOf(p->GetGUID()).empty())
        {
            LeaveLobby(p->GetGUID());
            Addon(p, MsgLeft());
            handler->SendSysMessage(std::string(TAG) + "You left the queue.");
            return true;
        }
        handler->SendSysMessage(std::string(TAG) + "You're not in a boss fight or its queue.");
        return true;
    }

    bool HandleStatus(ChatHandler* handler)
    {
        if (Player* p = handler->GetSession() ? handler->GetSession()->GetPlayer() : nullptr)
            StatusTo(handler, p);
        return true;
    }

    bool HandleList(ChatHandler* handler)
    {
        for (BossDef const& b : Bosses())
            handler->SendSysMessage(std::string(TAG) + b.key + ": " + b.title + " (" + std::to_string(b.size) + " players, normal or heroic). "
                ".bossfight join " + b.key + " normal bots");
        handler->SendSysMessage(std::string(TAG) + "Modes: bots (you and 9 bots, now), group (a raid of players; bots fill after " +
            std::to_string(g_cfg.lobbyWaitSec / 60) + " min), group-only (players only).");
        return true;
    }

    // GM / console: .bossfight start <leader> <boss> <difficulty> <bots|group|group-only>: the leader's own group
    // (if any) comes along; bots fill unless group-only. Started by a GM in game: the run is marked gm.
    bool HandleStart(ChatHandler* handler, Optional<std::string> nameArg, Optional<std::string> bossArg, Optional<std::string> diffArg,
        Optional<std::string> modeArg)
    {
        std::string name = Arg(nameArg);
        Player* leader = !name.empty() && normalizePlayerName(name) ? ObjectAccessor::FindPlayerByName(name) : nullptr;
        if (!leader)
        {
            handler->SendSysMessage("bossfight start: no such character online. Usage: .bossfight start <leader> <boss> <normal|heroic> <bots|group|group-only>");
            return true;
        }
        JoinArgs a = ParseJoin(Arg(bossArg), Arg(diffArg), Arg(modeArg));
        if (!a.error.empty())
        {
            handler->SendSysMessage("bossfight start: " + a.error);
            return true;
        }
        if (RunOf(leader->GetGUID()))
        {
            handler->SendSysMessage("bossfight start: " + leader->GetName() + " is already in a boss fight");
            return true;
        }
        std::string why = Blocked(leader);
        if (!why.empty())
        {
            handler->SendSysMessage("bossfight start: " + leader->GetName() + ": " + why);
            return true;
        }
        std::vector<Player*> humans{ leader };
        if (Group* g = leader->GetGroup())
            for (GroupReference* ref = g->GetFirstMember(); ref; ref = ref->next())
            {
                Player* m = ref->GetSource();
                if (m && m != leader && !IsBotPlayer(m) && Blocked(m).empty() && !RunOf(m->GetGUID()) && humans.size() < a.boss->size)
                    humans.push_back(m);
            }
        if (leader->GetGroup() && !leader->GetGroup()->IsLeader(leader->GetGUID()))
        {
            handler->SendSysMessage("bossfight start: " + leader->GetName() + " is in a group but doesn't lead it");
            return true;
        }
        bool gm = handler->GetSession() != nullptr;   // a GM in game, not the console / SOAP
        uint32 id = StartRun(humans, a.boss, a.diff, a.mode, a.mode != MODE_GROUP_ONLY && BotsOn(), gm);
        handler->SendSysMessage(id ? "bossfight start: ok, run " + std::to_string(id) : std::string("bossfight start: failed"));
        return true;
    }

    // GM / console: .bossfight reset <char>: out of any boss fight or queue, sent home, unbound from the boss maps.
    bool HandleReset(ChatHandler* handler, Optional<std::string> nameArg)
    {
        std::string name = Arg(nameArg);
        Player* p = !name.empty() && normalizePlayerName(name) ? ObjectAccessor::FindPlayerByName(name) : nullptr;
        if (!p)
        {
            handler->SendSysMessage("bossfight reset: no such character online. Usage: .bossfight reset <character>");
            return true;
        }
        if (RunOf(p->GetGUID()))
            LeaveRun(p, p->GetGUID(), true, "A GM took you out of the boss fight.");
        LeaveLobby(p->GetGUID());
        bool inside = false;
        for (BossDef const& b : Bosses())
            inside = inside || p->GetMapId() == b.mapId;
        if (inside)
            SendHome(p, nullptr);
        else
            Unbind(p);
        handler->SendSysMessage("bossfight reset: ok, " + p->GetName() + (inside ? " sent home (unbound once out)" : " unbound"));
        return true;
    }
}

class LeylineBossfightsCommandScript : public CommandScript
{
public:
    LeylineBossfightsCommandScript() : CommandScript("LeylineBossfightsCommandScript") { }

    ChatCommandTable GetCommands() const override
    {
        static ChatCommandTable bossfightTable =
        {
            { "join",   HandleJoin,   SEC_PLAYER,     Console::No },
            { "leave",  HandleLeave,  SEC_PLAYER,     Console::No },
            { "status", HandleStatus, SEC_PLAYER,     Console::No },
            { "list",   HandleList,   SEC_PLAYER,     Console::Yes },
            { "start",  HandleStart,  SEC_GAMEMASTER, Console::Yes },
            { "reset",  HandleReset,  SEC_GAMEMASTER, Console::Yes },
        };
        // .bf is also the core's battlefield (Wintergrasp) GM table (cs_bf.cpp: start, stop, switch, timer, enable,
        // queue). Sub-commands merge by name, so the alias carries only the player words that don't clash:
        // a second "start" would be a duplicate command and stop the server at load.
        static ChatCommandTable bfTable =
        {
            { "join",   HandleJoin,   SEC_PLAYER, Console::No },
            { "leave",  HandleLeave,  SEC_PLAYER, Console::No },
            { "status", HandleStatus, SEC_PLAYER, Console::No },
            { "list",   HandleList,   SEC_PLAYER, Console::Yes },
        };
        static ChatCommandTable commandTable =
        {
            { "bossfight", bossfightTable },
            { "bf",        bfTable },
        };
        return commandTable;
    }
};

// Any GM command typed by a member of a run marks the run (it never reaches the boards).
class LeylineBossfightsAllCommandScript : public AllCommandScript
{
public:
    LeylineBossfightsAllCommandScript() : AllCommandScript("LeylineBossfightsAllCommandScript", {
        ALLCOMMANDHOOK_ON_TRY_EXECUTE_COMMAND
    }) { }

    bool OnTryExecuteCommand(ChatHandler& handler, std::string_view cmdStr) override
    {
        WorldSession* s = handler.GetSession();
        Player* p = s ? s->GetPlayer() : nullptr;
        if (!p || s->GetSecurity() <= SEC_PLAYER)
            return true;
        std::string c = Lower(std::string(cmdStr));
        bool ours = c.rfind("bossfight", 0) == 0 || c.rfind("bf join", 0) == 0 || c.rfind("bf leave", 0) == 0
            || c.rfind("bf status", 0) == 0 || c.rfind("bf list", 0) == 0;
        if (!ours)
            if (BfRun* run = RunOf(p->GetGUID()))
                run->gm = true;
        return true;
    }
};

// Map threads: only queue events for this module's instances.
class LeylineBossfightsGlobalScript : public GlobalScript
{
public:
    LeylineBossfightsGlobalScript() : GlobalScript("LeylineBossfightsGlobalScript", {
        GLOBALHOOK_ON_BEFORE_SET_BOSS_STATE
    }) { }

    void OnBeforeSetBossState(uint32 id, EncounterState newState, EncounterState oldState, Map* instance) override
    {
        if (!instance || newState == oldState || oldState == TO_BE_DECIDED)
            return;
        BossDef const* b = FindBossByMap(instance->GetId());
        if (!b || !IsLive(instance->GetInstanceId()))
            return;
        if (id == b->bossData)
        {
            if (newState == IN_PROGRESS)
                PushEvent(EV_ENGAGE, instance->GetInstanceId());
            else if (newState == FAIL)
                PushEvent(EV_WIPE, instance->GetInstanceId());
            else if (newState == DONE)
                PushEvent(EV_DONE, instance->GetInstanceId());
        }
        else if (oldState == DONE && newState != DONE && IsPrereq(*b, id))
            PushEvent(EV_REASSERT, instance->GetInstanceId());   // a boss's AI reset it (its creature loaded)
    }
};

class LeylineBossfightsSpellScript : public AllSpellScript
{
public:
    LeylineBossfightsSpellScript() : AllSpellScript("LeylineBossfightsSpellScript", {
        ALLSPELLHOOK_ON_PREPARE
    }) { }

    // The cast begins (Spell::prepare has passed its checks): the boss is at 10% and the outro starts.
    void OnSpellPrepare(Spell* /*spell*/, Unit* caster, SpellInfo const* spellInfo) override
    {
        if (!caster || !spellInfo || !caster->IsCreature())
            return;
        Map* map = caster->GetMap();
        if (!map)
            return;
        BossDef const* b = FindBossByMap(map->GetId());
        if (!b || spellInfo->Id != b->endSpell || caster->GetEntry() != b->bossEntry)
            return;
        PushEvent(EV_CLEAR, map->GetInstanceId());
    }
};

class LeylineBossfightsPlayerScript : public PlayerScript
{
public:
    LeylineBossfightsPlayerScript() : PlayerScript("LeylineBossfightsPlayerScript", {
        PLAYERHOOK_ON_LOGIN,
        PLAYERHOOK_ON_LOGOUT
    }) { }

    void OnPlayerLogin(Player* player) override
    {
        if (!g_cfg.enable || !player)
            return;
        ObjectGuid guid = player->GetGUID();
        if (BfRun* run = RunOf(guid))
        {
            // back in time (the run kept them): the panel gets its state again
            if (run->phase == BfPhase::Engaged)
                Addon(player, MsgTimer(NowMs() - run->engageMs));
            else
                Addon(player, MsgStarting(run->boss->key, run->diff));
            return;
        }
        // inside one of our old instances with no run (a restart, or the run ended while offline): home, unbound
        if (FindBossByMap(player->GetMapId()) && g_knownInstances.count(player->GetInstanceId()))
        {
            Say(player, "That boss fight is over.");
            SendHome(player, nullptr);
        }
    }

    void OnPlayerLogout(Player* player) override
    {
        if (!player)
            return;
        ObjectGuid guid = player->GetGUID();
        LeaveLobby(guid);
        BfRun* run = RunOf(guid);
        if (!run)
            return;
        BfMember* m = MemberOf(*run, guid);
        if (m && m->bot)
        {
            // a bot logged out (its leader left, or the bot module did it): drop its seat
            g_memberRun.erase(guid);
            run->members.erase(std::remove_if(run->members.begin(), run->members.end(), [guid](BfMember const& x) { return x.guid == guid; }),
                run->members.end());
            return;
        }
        if (run->leader == guid)
        {
            g_inLogout = true;
            EndRun(run->id, "The raid's leader logged out: the boss fight is over.", false);
            g_inLogout = false;
            return;
        }
        LeaveRun(nullptr, guid, false, "");
    }
};

class LeylineBossfightsWorldScript : public WorldScript
{
public:
    LeylineBossfightsWorldScript() : WorldScript("LeylineBossfightsWorldScript", {
        WORLDHOOK_ON_AFTER_CONFIG_LOAD,
        WORLDHOOK_ON_STARTUP,
        WORLDHOOK_ON_UPDATE
    }) { }

    void OnAfterConfigLoad(bool /*reload*/) override
    {
        LoadSettings();
    }

    void OnStartup() override
    {
        // The same tables as data/sql/db-characters/base/leyline_bossfights.sql (and bossfights-export.py --schema),
        // for a realm whose updater skipped it.
        CharacterDatabase.DirectExecute("CREATE TABLE IF NOT EXISTS leyline_bossfight_runs (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, "
            "boss VARCHAR(32) NOT NULL, difficulty VARCHAR(8) NOT NULL, mode VARCHAR(12) NOT NULL DEFAULT 'bots', "
            "size TINYINT UNSIGNED NOT NULL DEFAULT 0, ms INT UNSIGNED NOT NULL DEFAULT 0, started_at INT UNSIGNED NOT NULL DEFAULT 0, "
            "ended_at INT UNSIGNED NOT NULL DEFAULT 0, leader_guid INT UNSIGNED NOT NULL DEFAULT 0, instance_id INT UNSIGNED NOT NULL DEFAULT 0, "
            "roster TEXT NOT NULL, bots TINYINT UNSIGNED NOT NULL DEFAULT 0, bot_buffs TINYINT UNSIGNED NOT NULL DEFAULT 0, "
            "gm TINYINT UNSIGNED NOT NULL DEFAULT 0, tier TINYINT UNSIGNED NOT NULL DEFAULT 0, wipes_before SMALLINT UNSIGNED NOT NULL DEFAULT 0, "
            "result VARCHAR(8) NOT NULL, KEY board (boss, difficulty, result, ms), KEY ended (ended_at))");
        CharacterDatabase.DirectExecute("CREATE TABLE IF NOT EXISTS leyline_bossfight_instances (instance_id INT UNSIGNED NOT NULL PRIMARY KEY, "
            "boss VARCHAR(32) NOT NULL, created_at INT UNSIGNED NOT NULL DEFAULT 0)");
        CharacterDatabase.DirectExecute("DELETE FROM leyline_bossfight_instances WHERE created_at < UNIX_TIMESTAMP() - 14 * 86400");
        if (QueryResult r = CharacterDatabase.Query("SELECT instance_id FROM leyline_bossfight_instances"))
        {
            do
            {
                g_knownInstances.insert((*r)[0].Get<uint32>());
            } while (r->NextRow());
        }
        LOG_INFO("module", "mod-leyline-bossfights: {} (bots {}, bot buffs {}, playerbots {}, {} old instances)",
            g_cfg.enable ? "on" : "off", BotsOn() ? "on" : "off", BotBuffsOn() ? "on" : "off",
            LEYLINE_BF_PLAYERBOTS ? "built in" : "missing", g_knownInstances.size());
    }

    void OnUpdate(uint32 diff) override
    {
        if (!g_cfg.enable)
            return;
        g_updateAcc += diff;
        if (g_updateAcc < SPELL_TICK_MS)
            return;
        g_updateAcc = 0;
        Tick();
    }
};

void AddLeylineBossfightsScripts()
{
    new LeylineBossfightsCommandScript();
    new LeylineBossfightsAllCommandScript();
    new LeylineBossfightsGlobalScript();
    new LeylineBossfightsSpellScript();
    new LeylineBossfightsPlayerScript();
    new LeylineBossfightsWorldScript();
}
