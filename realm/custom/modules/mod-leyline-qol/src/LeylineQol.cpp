/*
 * mod-leyline-qol: small quality-of-life rules for Leyline's Wrath Unleashed realm (README.md).
 *
 * 1. Instant logout in Dalaran. The owner (5 Nov 2026): "make dalaran an instant log out zone too". The core logs a
 *    player out at once only when they are resting (an inn, a capital city, a rest zone) and out of combat, or on a
 *    flight path (WorldSession::HandleLogoutRequestOpcode). Dalaran rests only in parts, so most of the city gave the
 *    20-second countdown. This module answers CMSG_LOGOUT_REQUEST itself (ServerScript::CanPacketReceive, which the
 *    session calls just before the opcode's handler, in the same thread) when the player stands in a listed area
 *    (LeylineQol.Dalaran.LogoutAreas: the city, zone 4395, and every area under it; and the forest under it, 2817)
 *    and nothing that would stop an instant logout applies (combat, falling, a duel, the GM freeze, the AFK rules).
 *    It does what the core does for a resting player: it releases loot, sends SMSG_LOGOUT_RESPONSE with result 0 and
 *    the instant flag (so the client shows no countdown) and calls LogoutPlayer(true) (save, SMSG_LOGOUT_COMPLETE).
 *    In every other case it lets the core's own handler answer, with the usual reasons and the countdown. It never
 *    sets the resting flag, so rested XP is unchanged.
 *
 * 2. Duels anywhere in Dalaran. The core refuses a duel when the caster's or the target's area lacks
 *    AREA_FLAG_ALLOW_DUELS (Spell::EffectDuel: "Dueling isn't allowed here"). At start-up (and on a config reload)
 *    this module sets that flag on every AreaTable row of map 571 under LeylineQol.Dalaran.DuelAreas (default the
 *    city, 4395: every street, Krasus' Landing, the Underbelly), the way the core's own SpellInfoCorrections sets area
 *    flags. Only the duel flag changes: the sanctuary flag stays, so attacking anyone else is still refused
 *    (Unit::_IsValidAttackTarget lets a duel's two players hit each other before its sanctuary check, and nothing
 *    else). Guards don't join duels (a duel is not PvP combat). A reload with the setting off puts the rows back.
 *
 * Settings: conf/mod_leyline_qol.conf.dist; the realm runs on environment settings (AC_LEYLINE_QOL_..., set in
 * realm/profiles/main.env). The defaults are built in, so a realm without the file or the env lines has both on.
 *
 * Unverified: written against the pinned core (mod-playerbots/azerothcore-wotlk 3f2969dd) by reading its sources and
 * checked with g++ -fsyntax-only against its headers; not run in game yet.
 */

#include "QolLogic.h"

#include "CharacterPackets.h"
#include "Common.h"
#include "Config.h"
#include "DBCStores.h"
#include "Log.h"
#include "Opcodes.h"
#include "Player.h"
#include "ScriptMgr.h"
#include "World.h"
#include "WorldConfig.h"
#include "WorldPacket.h"
#include "WorldSession.h"

#include <vector>

namespace
{
    constexpr uint32 AURA_GM_FREEZE = 9454;

    struct Settings
    {
        bool enable = true;
        bool instantLogout = true;
        bool duelsAnywhere = true;
        uint32 mapId = LeylineQol::MAP_NORTHREND;
        std::vector<uint32_t> logoutAreas{ LeylineQol::ZONE_DALARAN, LeylineQol::ZONE_CRYSTALSONG };
        std::vector<uint32_t> duelAreas{ LeylineQol::ZONE_DALARAN };
    };

    Settings g_cfg;
    std::vector<uint32> g_duelPatched;   // AreaTable rows this module gave FLAG_ALLOW_DUELS (put back on a reload)
    bool g_dbcReady = false;             // the AreaTable is loaded (OnBeforeWorldInitialized has run)

    void LoadSettings()
    {
        Settings s;
        s.enable = sConfigMgr->GetOption<bool>("LeylineQol.Enable", true);
        s.instantLogout = sConfigMgr->GetOption<bool>("LeylineQol.Dalaran.InstantLogout", true);
        s.duelsAnywhere = sConfigMgr->GetOption<bool>("LeylineQol.Dalaran.DuelsAnywhere", true);
        s.mapId = sConfigMgr->GetOption<uint32>("LeylineQol.Dalaran.MapId", LeylineQol::MAP_NORTHREND);
        s.logoutAreas = LeylineQol::ParseIdList(sConfigMgr->GetOption<std::string>("LeylineQol.Dalaran.LogoutAreas", "4395,2817"));
        s.duelAreas = LeylineQol::ParseIdList(sConfigMgr->GetOption<std::string>("LeylineQol.Dalaran.DuelAreas", "4395"));
        g_cfg = s;
    }

    uint32_t ParentOf(uint32_t id)
    {
        AreaTableEntry const* a = sAreaTableStore.LookupEntry(id);
        return a ? a->zone : 0;
    }

    void ApplyDuelAreas()
    {
        // put back what an earlier pass changed, then apply the current list
        for (uint32 id : g_duelPatched)
            if (AreaTableEntry* a = const_cast<AreaTableEntry*>(sAreaTableStore.LookupEntry(id)))
                a->flags &= ~uint32(AREA_FLAG_ALLOW_DUELS);
        g_duelPatched.clear();

        if (!g_cfg.enable || !g_cfg.duelsAnywhere || g_cfg.duelAreas.empty())
            return;

        std::vector<LeylineQol::AreaRow> rows;
        for (uint32 i = 0; i < sAreaTableStore.GetNumRows(); ++i)
            if (AreaTableEntry const* a = sAreaTableStore.LookupEntry(i))
                rows.push_back({ a->ID, a->mapid, a->zone, a->flags });

        for (uint32_t id : LeylineQol::DuelAreas(rows, g_cfg.duelAreas, g_cfg.mapId))
            if (AreaTableEntry* a = const_cast<AreaTableEntry*>(sAreaTableStore.LookupEntry(id)))
            {
                a->flags |= AREA_FLAG_ALLOW_DUELS;
                g_duelPatched.push_back(id);
            }
    }

    void Announce()
    {
        LOG_INFO("server.loading", "mod-leyline-qol: {}; Dalaran instant logout {} (areas {}); duels anywhere {} (areas {}, {} rows opened to duels)",
            g_cfg.enable ? "on" : "off",
            (g_cfg.enable && g_cfg.instantLogout) ? "on" : "off", LeylineQol::JoinIds(g_cfg.logoutAreas),
            (g_cfg.enable && g_cfg.duelsAnywhere) ? "on" : "off", LeylineQol::JoinIds(g_cfg.duelAreas),
            g_duelPatched.size());
    }
}

class LeylineQolWorldScript : public WorldScript
{
public:
    LeylineQolWorldScript() : WorldScript("LeylineQolWorldScript", {
        WORLDHOOK_ON_AFTER_CONFIG_LOAD,
        WORLDHOOK_ON_BEFORE_WORLD_INITIALIZED,
    }) { }

    void OnAfterConfigLoad(bool reload) override
    {
        LoadSettings();
        if (reload && g_dbcReady)   // the first load runs before the AreaTable is read; OnBeforeWorldInitialized applies it
        {
            ApplyDuelAreas();
            Announce();
        }
    }

    void OnBeforeWorldInitialized() override
    {
        g_dbcReady = true;
        LoadSettings();     // again: cheap, and right even if a loader ever adds scripts after the first config load
        ApplyDuelAreas();
        Announce();
    }
};

class LeylineQolServerScript : public ServerScript
{
public:
    LeylineQolServerScript() : ServerScript("LeylineQolServerScript", { SERVERHOOK_CAN_PACKET_RECEIVE }) { }

    bool CanPacketReceive(WorldSession* session, WorldPacket const& packet) override
    {
        if (packet.GetOpcode() != CMSG_LOGOUT_REQUEST || !session)
            return true;   // every other packet, untouched (this hook also runs in map threads; nothing else is read)

        Player* player = session->GetPlayer();
        if (!player)
            return true;

        LeylineQol::LogoutAsk ask;
        ask.enabled = g_cfg.enable && g_cfg.instantLogout;
        if (!ask.enabled)
            return true;
        ask.inWorld = player->IsInWorld();
        ask.onMap = player->GetMapId() == g_cfg.mapId;
        ask.inArea = LeylineQol::InAreas(player->GetAreaId(), g_cfg.logoutAreas, ParentOf);
        ask.inCombat = player->IsInCombat();
        ask.falling = player->m_movementInfo.HasMovementFlag(MOVEMENTFLAG_FALLING | MOVEMENTFLAG_FALLING_FAR);
        ask.dueling = player->duel != nullptr;
        ask.frozen = player->HasAura(AURA_GM_FREEZE);
        // the core's AFK rules (MiscHandler.cpp): AFK.PreventLogout 1 = not while AFK in a sanctuary, 2 = never while AFK
        uint32 afkRule = sWorld->getIntConfig(CONFIG_AFK_PREVENT_LOGOUT);
        AreaTableEntry const* area = sAreaTableStore.LookupEntry(player->GetAreaId());
        bool sanctuary = area && area->IsSanctuary();
        ask.afkBlocked = player->isAFK() && ((afkRule == 1 && sanctuary) || afkRule == 2 || session->HasAccountFlag(ACCOUNT_FLAG_NOKICK));

        if (!LeylineQol::InstantLogout(ask))
            return true;   // the core answers: countdown, or the reason it can't

        // what HandleLogoutRequestOpcode does for a resting player
        if (ObjectGuid lguid = player->GetLootGUID())
            session->DoLootRelease(lguid);

        WorldPackets::Character::LogoutResponse logoutResponse;
        logoutResponse.LogoutResult = 0;
        logoutResponse.Instant = true;
        session->SendPacket(logoutResponse.Write());

        session->LogoutPlayer(true);
        return false;      // handled: the core's handler is not called for this packet
    }
};

void AddLeylineQolScripts()
{
    new LeylineQolWorldScript();
    new LeylineQolServerScript();
}
