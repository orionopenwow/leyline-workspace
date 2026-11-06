/*
 * mod-leyline-bossfights: the seam between the boss fights (Bossfights.cpp) and boss fight heroes (BossfightHeroes.cpp).
 * docs/BOSS-FIGHT-HEROES.md has the design. Core headers are needed by both .cpp files; this one only declares.
 */
#ifndef LEYLINE_BOSSFIGHT_HEROES_H
#define LEYLINE_BOSSFIGHT_HEROES_H

#include "BossfightsHeroLogic.h"
#include "ObjectGuid.h"

#include <string>
#include <vector>

class ChatHandler;
class Player;

// Bossfights.cpp
namespace LeylineBFCore
{
    // What ".bossfight join" does for this player (start, queue, or try again). Returns the line to show when it
    // didn't go ahead ("" when it did: the module's own lines say what happens next).
    std::string Join(Player* p, LeylineBF::JoinArgs const& a);
    bool InRun(ObjectGuid guid);               // in a run (a lobby doesn't count)
    bool Fighting(ObjectGuid guid);            // in a run whose boss is engaged
    bool RunAllHeroes(ObjectGuid guid);        // in a run whose real players are all heroes
    bool InstanceUsed(std::uint32_t instanceId);  // an instance a run already used (a hero needs a fresh one)
    bool Enabled();
    void UnbindBossMaps(Player* p);            // now: only when the player isn't on a boss map's grid
    void Say(Player* p, std::string const& text);
    void Addon(Player* p, std::string const& payload);   // an LYBF message to the player's own panel
}

// BossfightHeroes.cpp
namespace LeylineBFHeroes
{
    void LoadTables();                         // startup: the tables and the in-memory hero list
    bool IsHero(ObjectGuid guid);
    bool IsHeroPlayer(Player* p);
    // A hero on a boss map that isn't in a run: it may start a run from there (its "waiting room").
    bool InWaitingRoom(Player* p);
    // Back on the platform (a hero never goes to a homebind): a near teleport, or a fresh instance if it isn't on the map.
    void ToPlatform(Player* p);
    // A hero that needs a fresh instance before this join (the last one was used, or the difficulty differs): moves it
    // there and runs the join on arrival. True = it took over (the caller stops).
    bool HopFirst(Player* p, LeylineBF::JoinArgs const& a);
    // Move a hero of a starting run into the run's instance (it waits in an instance of its own): a far teleport
    // within the map, its own bind dropped so it lands with its leader.
    void HopInto(Player* p, float x, float y, float z, float o);
    void OnLoadFromDB(Player* p);
    void OnLogin(Player* p);
    void OnLogout(Player* p);
    void Tick();
    // A restart between pulls (a wipe, Try again): cooldowns, repair, health and power for one member.
    void Refresh(Player* p);
    // ".bossfight hero ..." / ".bf hero ...": the console form (SOAP, the website) or the player's.
    bool Command(ChatHandler* handler, std::string const& args);
    void SendState(Player* p);                 // LYBF "H" to a hero's panel (login, .bossfight status, spec changes)
    void AddScripts();
}

#endif
