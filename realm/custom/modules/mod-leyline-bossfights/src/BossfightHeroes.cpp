/*
 * mod-leyline-bossfights: boss fight heroes (docs/BOSS-FIGHT-HEROES.md).
 *
 * A hero is a level 80 character the module makes for boss fights only, on a second game account linked to the
 * player's own (<ACCOUNT>BF, BossfightsHeroLogic.h HeroAccountName). The player's normal characters are never listed,
 * moved, boosted or bound by boss fights: the hero account's character list holds heroes and nothing else.
 *
 *   Instant: ".bossfight hero go <ACCOUNT> <boss> <difficulty> <mode> [class [spec [spec2]]] [alliance|horde]" (the
 *   website's Fight now, over SOAP) makes the hero at once if there is none for that pick: Player::Create on a session
 *   with no socket (as mod-playerbots makes its bots), level 80, the class's trainer spells, the spec's premade talents
 *   (the bots' builds), dual spec with the second spec filled, the spec's PvE kit worn, every other kit of the class
 *   (PvE and PvP) in four bags, reagents, a flask and food, both specs' action bars, saved in one transaction.
 *   Its saved place is the platform of the fight, in no instance, so its login makes a fresh instance there.
 *
 *   First frame on the platform: at login (Player::LoadFromDB) a raid map with no instance would put the character
 *   at the raid's entrance, and a raid needs a raid group. The hero is unbound from the boss maps while offline, so a
 *   fresh instance is made; OnPlayerLoadFromDB (after the map is chosen, before the player is added to it) gives it a
 *   raid group of its own and puts it on the platform. The core's login check (CheckInstanceLoginValid) then passes:
 *   the group is a raid, and the heroic entry requirement is waived for heroes (OnPlayerNotAvoidSatisfy).
 *   OnPlayerLogin queues the waiting join (the website's pick, else the hero's last one).
 *
 *   A fresh instance for the next run (after a clear, or another difficulty): a far teleport within the map
 *   (TeleportTo with newInstance) with the hero's own bind dropped once it has left the old one. A hero joining
 *   another player's run the same way lands in its leader's instance (InstanceSaveMgr: a grouped player goes where
 *   its leader is bound). Only a loading screen; nothing of the open world.
 *
 *   QoL: no XP; no trade, mail, auction bids, Dungeon Finder, battleground or arena queues; heroes stay on the boss
 *   maps (a teleport anywhere else is refused, a released ghost stays at its body); loot of a heroes-only run is
 *   cleared; a spec switch puts that spec's kit on; free respec and an instant spec swap out of combat; on a restart
 *   cooldowns, durability, health and power come back.
 *
 * Tables (characters DB, made at startup; data/sql/db-characters/base/leyline_bossfight_heroes.sql is the same):
 *   leyline_bf_hero_accounts  hero account -> the player's account, the current hero, the last pick, the waiting go
 *   leyline_bf_heroes         guid -> hero account, class, spec, second spec
 *   leyline_bossfight_live    one row the module refreshes every minute (ov_go.lua checks it before a "bf:" go)
 *
 * Unverified: written against the pinned core (3f2969dd) and mod-playerbots (e88cc409) by reading their sources.
 */

#include "BossfightHeroes.h"

#include "AccountMgr.h"
#include "CharacterCache.h"
#include "Chat.h"
#include "Config.h"
#include "DatabaseEnv.h"
#include "GameTime.h"
#include "Group.h"
#include "GroupMgr.h"
#include "InstanceSaveMgr.h"
#include "Item.h"
#include "Log.h"
#include "LootMgr.h"
#include "Map.h"
#include "MapMgr.h"
#include "ObjectAccessor.h"
#include "ObjectMgr.h"
#include "Player.h"
#include "ScriptMgr.h"
#include "SpellInfo.h"
#include "SpellMgr.h"
#include "Trainer.h"
#include "World.h"
#include "WorldSession.h"

#if __has_include("PlayerbotFactory.h") && __has_include("PlayerbotAIConfig.h")
#define LEYLINE_BF_HERO_TALENTS 1
#include "PlayerbotFactory.h"
#else
#define LEYLINE_BF_HERO_TALENTS 0
#endif

#include <chrono>
#include <random>
#include <set>
#include <sstream>
#include <unordered_map>
#include <unordered_set>

using namespace LeylineBF;

namespace
{
    constexpr char const* TAG = "|cff9fd8ff[Boss fights]|r ";
    constexpr std::uint32_t SOUL_SHARD = 6265;
    constexpr std::uint32_t SPELL_DUAL_WIELD = 674, SPELL_TITANS_GRIP = 46917;
    constexpr std::uint32_t SPELL_ACTIVATE_PRIMARY = 63645, SPELL_ACTIVATE_SECONDARY = 63644;
    constexpr std::uint8_t ACTION_SPELL = 0x00, ACTION_ITEM = 0x80;
    constexpr std::uint32_t ARROWS = 41165, BULLETS = 41164;

    struct HeroRow
    {
        std::uint32_t account = 0;
        std::uint8_t cls = 0;
        std::string spec, spec2;
    };

    struct AccountRow
    {
        std::uint32_t main = 0;
        std::uint32_t current = 0;     // guid low of the current hero
        std::uint8_t lastClass = 0;
        std::string lastSpec, lastSpec2;
        int lastFaction = -1;
        std::string lastGo = "bf:frozen-throne:normal:bots";
        std::string pendingGo;
        std::uint32_t pendingUntil = 0;
    };

    struct LoginJoin
    {
        ObjectGuid guid;
        JoinArgs args;
        std::uint64_t at = 0;          // steady ms
        std::uint64_t until = 0;
    };

    std::unordered_map<std::uint32_t, HeroRow> g_heroes;       // guid low -> hero
    std::unordered_map<std::uint32_t, AccountRow> g_accounts;  // hero account id -> link
    std::vector<LoginJoin> g_joins;                            // joins waiting for a login or an arrival
    std::unordered_set<std::uint32_t> g_offlineUnbind;         // heroes to unbind from the boss maps once logged out
    std::uint64_t g_lastBeat = 0;

    std::uint64_t NowMs()
    {
        return std::uint64_t(std::chrono::duration_cast<std::chrono::milliseconds>(
            std::chrono::steady_clock::now().time_since_epoch()).count());
    }

    std::uint32_t NowUnix() { return std::uint32_t(GameTime::GetGameTime().count()); }

    BossDef const& Featured() { return Bosses().front(); }

    std::string Esc(std::string s)
    {
        CharacterDatabase.EscapeString(s);
        return s;
    }

    void SaveAccount(std::uint32_t heroAccount)
    {
        auto it = g_accounts.find(heroAccount);
        if (it == g_accounts.end())
            return;
        AccountRow const& a = it->second;
        CharacterDatabase.Execute("REPLACE INTO leyline_bf_hero_accounts (hero_account_id, main_account_id, current_guid, last_class, "
            "last_spec, last_spec2, last_faction, last_go, pending_go, pending_until) VALUES ({}, {}, {}, {}, '{}', '{}', {}, '{}', '{}', {})",
            heroAccount, a.main, a.current, a.lastClass, Esc(a.lastSpec), Esc(a.lastSpec2), a.lastFaction, Esc(a.lastGo),
            Esc(a.pendingGo), a.pendingUntil);
    }

    HeroRow const* HeroOf(std::uint32_t guidLow)
    {
        auto it = g_heroes.find(guidLow);
        return it == g_heroes.end() ? nullptr : &it->second;
    }

    void Say(Player* p, std::string const& text) { LeylineBFCore::Say(p, text); }

    // ---- accounts -------------------------------------------------------------------------------------------------

    struct Link
    {
        std::uint32_t hero = 0, main = 0;
        std::string heroName, error;
    };

    std::string RandomPassword()
    {
        static char const chars[] = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
        std::random_device rd;
        std::mt19937 gen(rd());
        std::uniform_int_distribution<int> pick(0, int(sizeof(chars)) - 2);
        std::string s;
        for (int i = 0; i < 16; ++i)
            s += chars[pick(gen)];
        return s;
    }

    // The hero account of a player's own account (made when missing, with a random password; the website sets the
    // player's own password on it before it signs the browser game in).
    Link LinkForMain(std::uint32_t mainId, std::string const& mainName, bool create)
    {
        Link l;
        if (!mainId)
        {
            l.error = "no such account";
            return l;
        }
        if (g_accounts.count(mainId))
        {
            l.error = "that is a boss fight hero account";
            return l;
        }
        l.main = mainId;
        l.heroName = HeroAccountName(mainName);
        if (l.heroName.empty())
        {
            l.error = "that account name can't have heroes";
            return l;
        }
        l.hero = AccountMgr::GetId(l.heroName);
        if (!l.hero && create)
        {
            if (sAccountMgr->CreateAccount(l.heroName, RandomPassword()) == AOR_OK)
                l.hero = AccountMgr::GetId(l.heroName);
        }
        if (!l.hero)
        {
            l.error = create ? "the hero account couldn't be made" : "no heroes yet";
            return l;
        }
        auto it = g_accounts.find(l.hero);
        if (it != g_accounts.end() && it->second.main != mainId)
        {
            l.error = "that hero account belongs to another player";
            l.hero = 0;
            return l;
        }
        if (it == g_accounts.end())
        {
            AccountRow row;
            row.main = mainId;
            g_accounts[l.hero] = row;
            SaveAccount(l.hero);
        }
        return l;
    }

    Link LinkForPlayer(Player* p, bool create)
    {
        std::uint32_t acct = p->GetSession()->GetAccountId();
        auto it = g_accounts.find(acct);
        if (it != g_accounts.end())
        {
            Link l;
            l.hero = acct;
            l.main = it->second.main;
            AccountMgr::GetName(acct, l.heroName);
            return l;
        }
        std::string name;
        AccountMgr::GetName(acct, name);
        return LinkForMain(acct, name, create);
    }

    std::vector<std::uint32_t> HeroesOf(std::uint32_t heroAccount)
    {
        std::vector<std::uint32_t> out;
        std::vector<std::uint32_t> gone;
        for (auto const& kv : g_heroes)
        {
            if (kv.second.account != heroAccount)
                continue;
            if (!sCharacterCache->GetCharacterCacheByGuid(ObjectGuid::Create<HighGuid::Player>(kv.first)))
                gone.push_back(kv.first);
            else
                out.push_back(kv.first);
        }
        for (std::uint32_t g : gone)
        {
            g_heroes.erase(g);
            CharacterDatabase.Execute("DELETE FROM leyline_bf_heroes WHERE guid = {}", g);
        }
        std::sort(out.begin(), out.end());
        return out;
    }

    std::string NameOf(std::uint32_t guidLow)
    {
        std::string n;
        sCharacterCache->GetCharacterNameByGuid(ObjectGuid::Create<HighGuid::Player>(guidLow), n);
        return n;
    }

    // Alliance or Horde for a new hero: the pick, else the hero account's last one, else the side most of the player's
    // own characters are on (Alliance when there are none).
    bool HordeFor(Link const& l, int pickFaction)
    {
        if (pickFaction >= 0)
            return pickFaction == 1;
        auto it = g_accounts.find(l.hero);
        if (it != g_accounts.end() && it->second.lastFaction >= 0)
            return it->second.lastFaction == 1;
        int horde = 0, alliance = 0;
        if (QueryResult r = CharacterDatabase.Query("SELECT race FROM characters WHERE account = {} LIMIT 50", l.main))
        {
            do
            {
                std::uint8_t race = (*r)[0].Get<std::uint8_t>();
                bool h = race == 2 || race == 5 || race == 6 || race == 8 || race == 10;
                (h ? horde : alliance)++;
            } while (r->NextRow());
        }
        return horde > alliance;
    }

    // ---- making a hero (offline: the Player is never added to a map) ----------------------------------------------

    bool FactionOk(std::uint32_t spell, bool horde)
    {
        auto const& other = horde ? HeroAllianceOnly() : HeroHordeOnly();
        return std::find(other.begin(), other.end(), spell) == other.end();
    }

    // A trainer's "teach" spell, learned the way casting it would: its learn-spell effects (never a profession).
    void LearnTaught(Player* p, std::uint32_t spellId, bool horde)
    {
        SpellInfo const* si = sSpellMgr->GetSpellInfo(spellId);
        if (!si)
            return;
        for (std::uint8_t i = 0; i < MAX_SPELL_EFFECTS; ++i)
        {
            if (si->Effects[i].Effect != SPELL_EFFECT_LEARN_SPELL || !si->Effects[i].TriggerSpell)
                continue;
            std::uint32_t t = si->Effects[i].TriggerSpell;
            SpellInfo const* ti = sSpellMgr->GetSpellInfo(t);
            if (!ti || ti->Effects[0].Effect == SPELL_EFFECT_TRADE_SKILL || !FactionOk(t, horde))
                continue;
            if (!p->HasSpell(t))
                p->learnSpell(t, false);
        }
    }

    // Every class trainer spell the hero may learn now (the core's ".learn all my trainer", without casting).
    void TrainerPass(Player* p, bool horde)
    {
        std::vector<Trainer::Trainer const*> const* trainers = nullptr;
        try
        {
            trainers = &sObjectMgr->GetClassTrainers(p->getClass());
        }
        catch (std::exception const&)
        {
            return;
        }
        for (int round = 0; round < 10; ++round)
        {
            bool again = false;
            for (Trainer::Trainer const* t : *trainers)
            {
                if (!t || !t->IsTrainerValidForPlayer(p))
                    continue;
                for (Trainer::Spell const& s : t->GetSpells())
                {
                    if (!FactionOk(s.SpellId, horde) || !t->CanTeachSpell(p, &s))
                        continue;
                    if (s.IsCastable())
                        LearnTaught(p, s.SpellId, horde);
                    else
                        p->learnSpell(s.SpellId, false);
                    if (!t->CanTeachSpell(p, &s))
                        again = true;
                }
            }
            if (!again)
                break;
        }
    }

    void SetSpecTalents(Player* p, SpecDef const& spec, bool reset)
    {
#if LEYLINE_BF_HERO_TALENTS
        PlayerbotFactory::InitTalentsBySpecNo(p, spec.pbSpec, reset);
#else
        (void)p; (void)spec; (void)reset;   // without mod-playerbots: the points stay free for the player
#endif
    }

    void DualWieldFlags(Player* p)
    {
        if (p->HasSpell(SPELL_DUAL_WIELD))
            p->SetCanDualWield(true);
        if (p->HasSpell(SPELL_TITANS_GRIP))
            p->SetCanTitanGrip(true);
    }

    std::uint32_t HighestKnown(Player* p, std::uint32_t rank1)
    {
        for (std::uint32_t s = sSpellMgr->GetLastSpellInChain(rank1); s; s = sSpellMgr->GetPrevSpellInChain(s))
            if (p->HasSpell(s))
                return s;
        return p->HasSpell(rank1) ? rank1 : 0;
    }

    struct Button
    {
        std::uint8_t button;
        std::uint32_t action;
        std::uint8_t type;
    };

    // The spec's layout with what this hero knows in its active spec (BossfightsHeroLogic.h BarLayout): spells at their
    // highest known rank, the racial, the PvP medallion, the flask and the food where the boost had its mounts.
    std::vector<Button> ResolveBars(Player* p, SpecDef const& spec, bool horde, std::uint32_t flask)
    {
        std::vector<Button> out;
        for (auto const& kv : BarLayout(p->getClass(), spec.key))
        {
            for (std::int32_t alt : kv.second)
            {
                std::uint32_t action = 0;
                std::uint8_t type = ACTION_SPELL;
                if (alt > 0)
                    action = HighestKnown(p, std::uint32_t(alt));
                else if (alt == BAR_RACIAL)
                {
                    for (std::uint32_t r : HeroRacials())
                        if (p->HasSpell(r))
                        {
                            action = r;
                            break;
                        }
                }
                else if (alt == BAR_PVP)
                    action = Medallion(horde), type = ACTION_ITEM;
                else if (alt == BAR_GROUND)
                    action = flask, type = ACTION_ITEM;
                else if (alt == BAR_FLY)
                    action = FISH_FEAST, type = ACTION_ITEM;
                if (action)
                {
                    out.push_back({ kv.first, action, type });
                    break;
                }
            }
        }
        return out;
    }

    void ApplyBarsInMemory(Player* p, std::vector<Button> const& bars)
    {
        for (std::uint8_t b = 0; b < MAX_ACTION_BUTTONS; ++b)
            p->removeActionButton(b);
        for (Button const& b : bars)
            p->addActionButton(b.button, b.action, b.type);
    }

    // Wears the kit's pieces from new items (creation) or from the bags (a spec switch): each in the next free slot its
    // InventoryType allows (both rings, both trinkets, the off hand with dual wield or titan's grip). Pieces that can't
    // be worn are returned.
    std::vector<std::uint32_t> WearKit(Player* p, HeroKit const& kit, bool fromBags)
    {
        std::vector<std::uint32_t> left;
        std::set<std::uint8_t> used;
        for (std::uint32_t id : kit.pve)
        {
            ItemTemplate const* proto = sObjectMgr->GetItemTemplate(id);
            if (!proto)
                continue;
            std::vector<std::uint8_t> slots = EquipSlotsFor(proto->InventoryType, p->CanDualWield(), p->CanTitanGrip());
            bool done = false;
            for (std::uint8_t s : slots)   // already worn there
            {
                Item* cur = p->GetItemByPos(INVENTORY_SLOT_BAG_0, s);
                if (!used.count(s) && cur && cur->GetEntry() == id)
                {
                    used.insert(s);
                    done = true;
                    break;
                }
            }
            for (std::size_t i = 0; !done && i < slots.size(); ++i)
            {
                std::uint8_t s = slots[i];
                if (used.count(s))
                    continue;
                std::uint16_t dest = 0;
                if (!fromBags)
                {
                    if (p->CanEquipNewItem(s, dest, id, false) == EQUIP_ERR_OK && p->EquipNewItem(dest, id, true))
                        used.insert(s), done = true;
                    continue;
                }
                Item* item = nullptr;
                for (std::uint8_t b = INVENTORY_SLOT_ITEM_START; b < INVENTORY_SLOT_ITEM_END && !item; ++b)
                    if (Item* it = p->GetItemByPos(INVENTORY_SLOT_BAG_0, b); it && it->GetEntry() == id)
                        item = it;
                for (std::uint8_t bag = INVENTORY_SLOT_BAG_START; bag < INVENTORY_SLOT_BAG_END && !item; ++bag)
                    if (Bag* pb = p->GetBagByPos(bag))
                        for (std::uint32_t j = 0; j < pb->GetBagSize() && !item; ++j)
                            if (Item* it = pb->GetItemByPos(std::uint8_t(j)); it && it->GetEntry() == id)
                                item = it;
                if (!item)
                    break;
                if (p->CanEquipItem(s, dest, item, true) == EQUIP_ERR_OK)
                {
                    p->SwapItem(item->GetPos(), dest);
                    used.insert(s);
                    done = true;
                }
            }
            if (!done && !fromBags)
                left.push_back(id);
        }
        if (fromBags)
            p->AutoUnequipOffhandIfNeed();
        return left;
    }

    void Store(Player* p, std::uint32_t id, std::uint32_t count)
    {
        ItemPosCountVec dest;
        if (count && p->CanStoreNewItem(NULL_BAG, NULL_SLOT, dest, id, count) == EQUIP_ERR_OK)
            p->StoreNewItem(dest, id, true);
    }

    struct Made
    {
        std::uint32_t guid = 0;
        std::string name;
        std::string error;
    };

    // Makes and saves a hero on the hero account. Never adds it to the world.
    Made CreateHero(Link const& l, SpecDef const& spec, SpecDef const& spec2, bool horde)
    {
        Made out;
        std::uint8_t cls = spec.cls;
        if (HeroesOf(l.hero).size() >= HERO_LIMIT ||
            AccountMgr::GetCharactersCount(l.hero) >= sWorld->getIntConfig(CONFIG_CHARACTERS_PER_REALM))
        {
            out.error = "you have " + std::to_string(HERO_LIMIT) + " heroes: delete one first (.bf hero delete <name>)";
            return out;
        }
        std::uint8_t race = DefaultRace(cls, horde);
        if (!sObjectMgr->GetPlayerInfo(race, cls))
        {
            out.error = "that class can't be made on this realm";
            return out;
        }
        std::string name;
        std::uint32_t seed = l.hero * 7919u + NowUnix();
        for (std::uint32_t a = 0; a < 400 && name.empty(); ++a)
        {
            std::string n = HeroName(seed, a);
            if (ObjectMgr::CheckPlayerName(n, true) == CHAR_NAME_SUCCESS && !sObjectMgr->IsReservedName(n) &&
                sCharacterCache->GetCharacterGuidByName(n).IsEmpty())
                name = n;
        }
        if (name.empty())
        {
            out.error = "no free name was found; try again";
            return out;
        }

        WorldSession* session = new WorldSession(l.hero, "", 0, nullptr, SEC_PLAYER, EXPANSION_WRATH_OF_THE_LICH_KING,
            time_t(0), LOCALE_enUS, 0, false, false, 0);
        CharacterCreateInfo info(name, race, cls, GENDER_MALE, 0, 0, 0, 0, 0);
        Player* p = new Player(session);
        p->GetMotionMaster()->Initialize();
        if (!p->Create(sObjectMgr->GetGenerator<HighGuid::Player>().Generate(), &info))
        {
            p->CleanupsBeforeDelete();
            delete p;
            delete session;
            out.error = "the hero couldn't be made";
            return out;
        }
        p->setCinematic(1);
        p->SetAtLoginFlag(AT_LOGIN_NONE);

        // the starting outfit goes; heroes carry only their kits
        for (std::uint8_t s = EQUIPMENT_SLOT_START; s < INVENTORY_SLOT_ITEM_END; ++s)
            if (p->GetItemByPos(INVENTORY_SLOT_BAG_0, s))
                p->DestroyItem(INVENTORY_SLOT_BAG_0, s, true);

        // level 80 (GiveLevel's own steps, without its level-up hooks: the realm's Lua reacts to those)
        p->SetUInt32Value(UNIT_FIELD_LEVEL, 80);
        p->SetUInt32Value(PLAYER_NEXT_LEVEL_XP, sObjectMgr->GetXPForLevel(80));
        p->InitStatsForLevel();
        p->InitTaxiNodesForLevel();
        p->InitGlyphsForLevel();
        p->InitTalentForLevel();

        // spells: weapons, the class quests' spells, the trainers; the spec's talents, the trainers again for the
        // talents' ranks; dual spec with the second spec's talents and ranks
        for (std::uint32_t s : HeroWeaponSpells(cls))
            if (!p->HasSpell(s))
                p->learnSpell(s, false);
        for (std::uint32_t s : HeroQuestSpells(cls))
            if (FactionOk(s, horde) && !p->HasSpell(s))
                p->learnSpell(s, false);
        TrainerPass(p, horde);
        // The points of a level 80 character, set by hand: a new Death Knight still stands in its starting zone here,
        // where CalculateTalentsPoints counts only quest points (25). At login, on the boss map, the core counts again.
        std::uint32_t points = std::uint32_t((80 - 9) * sWorld->getRate(RATE_TALENT));
        p->SetFreeTalentPoints(points);
        SetSpecTalents(p, spec, false);
        TrainerPass(p, horde);
        p->learnSpell(SPELL_ACTIVATE_PRIMARY, false);
        p->learnSpell(SPELL_ACTIVATE_SECONDARY, false);
        p->SetSpecsCount(2);
        p->SetActiveSpec(1);
        p->SetFreeTalentPoints(points);
        SetSpecTalents(p, spec2, false);
        TrainerPass(p, horde);
        HeroKit const* kit = KitFor(cls, spec.kit);
        HeroKit const* kit2 = KitFor(cls, spec2.kit);
        std::uint32_t flask = FlaskFor(kit ? kit->role : "");
        std::uint32_t flask2 = FlaskFor(kit2 ? kit2->role : "");
        std::vector<Button> bars2 = ResolveBars(p, spec2, horde, flask2);
        p->SetActiveSpec(0);
        p->SetFreeTalentPoints(0);
        p->UpdateSkillsForLevel();
        p->UpdateSkillsToMaxSkillsForLevel();
        DualWieldFlags(p);
        ApplyBarsInMemory(p, ResolveBars(p, spec, horde, flask));

        // gear: four bags, the spec's kit worn, everything else of the class packed, then the supplies
        for (std::uint8_t b = INVENTORY_SLOT_BAG_START; b < INVENTORY_SLOT_BAG_END; ++b)
        {
            std::uint16_t dest = 0;
            if (p->CanEquipNewItem(b, dest, HERO_BAG, false) == EQUIP_ERR_OK)
                p->EquipNewItem(dest, HERO_BAG, true);
        }
        std::vector<std::uint32_t> left = kit ? WearKit(p, *kit, false) : std::vector<std::uint32_t>();
        for (std::uint32_t id : left)
            Store(p, id, 1);
        for (auto const& kv : HeroBagItems(cls, spec.kit, horde))
            Store(p, kv.first, kv.second);
        for (auto const& kv : HeroSupplies(cls, kit ? kit->role : ""))
            Store(p, kv.first, kv.second);
        if (flask2 != flask)
            Store(p, flask2, CONSUMABLE_STACK);
        if (cls == 9)
            Store(p, SOUL_SHARD, 10);
        if (cls == 3)
            if (Item* ranged = p->GetItemByPos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_RANGED))
                p->SetAmmo(ranged->GetTemplate()->SubClass == ITEM_SUBCLASS_WEAPON_GUN ? BULLETS : ARROWS);
        p->SetFullHealth();
        if (p->getPowerType() == POWER_MANA)
            p->SetPower(POWER_MANA, p->GetMaxPower(POWER_MANA));

        BossDef const& b = Featured();
        std::uint32_t guid = p->GetGUID().GetCounter();
        CharacterDatabaseTransaction trans = CharacterDatabase.BeginTransaction();
        p->SaveToDB(trans, true, false);
        trans->Append("UPDATE characters SET map = {}, position_x = {}, position_y = {}, position_z = {}, orientation = {}, "
            "zone = {}, instance_id = 0, instance_mode_mask = 0, cinematic = 1, at_login = 0 WHERE guid = {}",
            b.mapId, b.x, b.y, b.z, b.o, HERO_ZONE, guid);
        for (Button const& btn : bars2)
        {
            CharacterDatabasePreparedStatement* stmt = CharacterDatabase.GetPreparedStatement(CHAR_INS_CHAR_ACTION);
            stmt->SetData(0, guid);
            stmt->SetData(1, std::uint8_t(1));
            stmt->SetData(2, btn.button);
            stmt->SetData(3, btn.action);
            stmt->SetData(4, btn.type);
            trans->Append(stmt);
        }
        trans->Append("REPLACE INTO leyline_bf_heroes (guid, hero_account_id, class, spec, spec2, created_at) VALUES ({}, {}, {}, '{}', '{}', {})",
            guid, l.hero, cls, spec.key, spec2.key, NowUnix());
        CharacterDatabase.DirectCommitTransaction(trans);   // the browser game asks for the character list seconds later
        sCharacterCache->AddCharacterCacheEntry(p->GetGUID(), l.hero, name, p->getGender(), race, cls, 80);
        p->CleanupsBeforeDelete();
        delete p;
        delete session;

        HeroRow row;
        row.account = l.hero;
        row.cls = cls;
        row.spec = spec.key;
        row.spec2 = spec2.key;
        g_heroes[guid] = row;
        LOG_INFO("module", "mod-leyline-bossfights: hero {} ({} {}, {}) made on account {}", name, ClassToken(cls), spec.key, spec2.key, l.hero);
        out.guid = guid;
        out.name = name;
        return out;
    }

    // ---- the hero's place before its next login ------------------------------------------------------------------

    void UnbindOffline(std::uint32_t guidLow)
    {
        ObjectGuid g = ObjectGuid::Create<HighGuid::Player>(guidLow);
        sInstanceSaveMgr->PlayerCreateBoundInstancesMaps(g);   // the bind storage must exist for an offline unbind
        for (BossDef const& b : Bosses())
            for (std::uint8_t d = 0; d < MAX_RAID_DIFFICULTY; ++d)
                if (sInstanceSaveMgr->PlayerGetBoundInstance(g, b.mapId, ::Difficulty(d)))
                    sInstanceSaveMgr->PlayerUnbindInstance(g, b.mapId, ::Difficulty(d), true, nullptr);
    }

    // An offline hero: out of any group left over (a stale raid would change its difficulty at login and send it to
    // its homebind), unbound, and saved on the platform at the waiting go's difficulty.
    void PrepareOffline(std::uint32_t guidLow, int diff)
    {
        ObjectGuid g = ObjectGuid::Create<HighGuid::Player>(guidLow);
        if (ObjectGuid groupGuid = sCharacterCache->GetCharacterGroupGuidByGuid(g))
            if (Group* group = sGroupMgr->GetGroupByGUID(groupGuid.GetCounter()))
                if (group->IsMember(g))
                    group->RemoveMember(g);
        UnbindOffline(guidLow);
        BossDef const& b = Featured();
        CharacterDatabase.Execute("UPDATE characters SET map = {}, position_x = {}, position_y = {}, position_z = {}, orientation = {}, "
            "zone = {}, instance_id = 0, instance_mode_mask = {} WHERE guid = {} AND online = 0",
            b.mapId, b.x, b.y, b.z, b.o, HERO_ZONE, InstanceModeMask(diff), guidLow);
    }

    void QueueJoin(ObjectGuid guid, JoinArgs const& a, std::uint64_t delayMs, std::uint64_t waitMs)
    {
        g_joins.erase(std::remove_if(g_joins.begin(), g_joins.end(), [guid](LoginJoin const& j) { return j.guid == guid; }), g_joins.end());
        LoginJoin j;
        j.guid = guid;
        j.args = a;
        j.at = NowMs() + delayMs;
        j.until = j.at + waitMs;
        g_joins.push_back(j);
    }

    // ---- the commands ---------------------------------------------------------------------------------------------

    std::vector<std::string> Words(std::string const& s)
    {
        std::vector<std::string> out;
        std::istringstream in(s);
        std::string w;
        while (in >> w && out.size() < 16)
            out.push_back(w);
        return out;
    }

    void Line(ChatHandler* h, std::string const& text, bool console)
    {
        h->SendSysMessage(console ? text : std::string(TAG) + text);
    }

    // The hero for a pick: an existing hero of that class and spec, else a new one. No class in the pick: the current
    // hero, else the last pick, else a fire mage.
    Made HeroForPick(Link const& l, HeroPick const& pick, bool& made)
    {
        made = false;
        AccountRow& acc = g_accounts[l.hero];
        std::uint8_t cls = pick.cls;
        SpecDef const* spec = pick.spec;
        SpecDef const* spec2 = pick.spec2;
        std::vector<std::uint32_t> mine = HeroesOf(l.hero);
        if (!cls)
        {
            if (acc.current && std::find(mine.begin(), mine.end(), acc.current) != mine.end())
            {
                Made m;
                m.guid = acc.current;
                m.name = NameOf(acc.current);
                return m;
            }
            cls = acc.lastClass ? acc.lastClass : 8;
            spec = FindSpec(cls, acc.lastSpec);
            if (!spec)
                spec = DefaultSpec(cls);
            spec2 = FindSpec(cls, acc.lastSpec2);
            if (!spec2 || spec2 == spec)
                spec2 = PartnerSpec(*spec);
        }
        for (std::uint32_t g : mine)
        {
            HeroRow const* h = HeroOf(g);
            if (h && h->cls == cls && h->spec == spec->key)
            {
                Made m;
                m.guid = g;
                m.name = NameOf(g);
                acc.current = g;
                return m;
            }
        }
        bool horde = HordeFor(l, pick.faction);
        Made m = CreateHero(l, *spec, *spec2, horde);
        if (m.error.empty())
        {
            made = true;
            acc.current = m.guid;
            acc.lastClass = cls;
            acc.lastSpec = spec->key;
            acc.lastSpec2 = spec2->key;
            acc.lastFaction = horde ? 1 : 0;
        }
        return m;
    }

    std::string OkLine(Link const& l, Made const& m, bool made)
    {
        HeroRow const* h = HeroOf(m.guid);
        SpecDef const* s = h ? FindSpec(h->cls, h->spec) : nullptr;
        SpecDef const* s2 = h ? FindSpec(h->cls, h->spec2) : nullptr;
        if (!h || !s || !s2)
            return ReplyHeroNo("the hero couldn't be read back");
        return ReplyHeroOk(m.name, h->cls, *s, *s2, made, l.heroName);
    }

    void ListTo(ChatHandler* h, Link const& l, bool console)
    {
        std::vector<std::uint32_t> mine = HeroesOf(l.hero);
        AccountRow const& acc = g_accounts[l.hero];
        if (console)
            h->SendSysMessage("bfhero list " + std::to_string(mine.size()) + " " + l.heroName);
        else if (mine.empty())
            Line(h, "No heroes yet. .bf hero make <class> [spec] makes one (level 80, geared, both specs ready), "
                "or pick one on the website's Boss fights page.", false);
        else
            Line(h, "Your heroes (" + std::to_string(mine.size()) + " of " + std::to_string(HERO_LIMIT) + ", account " + l.heroName + "):", false);
        for (std::uint32_t g : mine)
        {
            HeroRow const* r = HeroOf(g);
            if (!r)
                continue;
            if (console)
                h->SendSysMessage(ReplyHeroLine(NameOf(g), r->cls, r->spec, r->spec2, acc.current == g));
            else
            {
                SpecDef const* s = FindSpec(r->cls, r->spec);
                SpecDef const* s2 = FindSpec(r->cls, r->spec2);
                Line(h, NameOf(g) + ": " + (s ? s->label : r->spec) + " / " + (s2 ? s2->label : r->spec2) + " " + ClassToken(r->cls) +
                    (acc.current == g ? " (current)" : ""), false);
            }
        }
    }

    bool DeleteHero(Link const& l, std::string const& rawName, std::string& why)
    {
        std::string name = rawName;
        if (!normalizePlayerName(name))
        {
            why = "no such hero";
            return false;
        }
        for (std::uint32_t g : HeroesOf(l.hero))
        {
            if (NameOf(g) != name)
                continue;
            if (ObjectAccessor::FindConnectedPlayer(ObjectGuid::Create<HighGuid::Player>(g)))
            {
                why = name + " is logged in: log out first";
                return false;
            }
            Player::DeleteFromDB(g, l.hero, true, true);
            g_heroes.erase(g);
            CharacterDatabase.Execute("DELETE FROM leyline_bf_heroes WHERE guid = {}", g);
            AccountRow& acc = g_accounts[l.hero];
            if (acc.current == g)
                acc.current = 0;
            SaveAccount(l.hero);
            return true;
        }
        why = "no hero called " + name;
        return false;
    }

    // Free respec of the active spec (out of combat, not fighting): the spec's talents, its trainer ranks, its kit
    // and its bars.
    std::string Respec(Player* p, SpecDef const& spec)
    {
        if (p->IsInCombat() || LeylineBFCore::Fighting(p->GetGUID()))
            return "not in combat";
        bool horde = p->GetTeamId(true) == TEAM_HORDE;
        p->resetTalents(true);
        SetSpecTalents(p, spec, false);
        TrainerPass(p, horde);
        DualWieldFlags(p);
        if (HeroKit const* kit = KitFor(p->getClass(), spec.kit))
        {
            WearKit(p, *kit, true);
            ApplyBarsInMemory(p, ResolveBars(p, spec, horde, FlaskFor(kit->role)));
            p->SendActionButtons(1);
        }
        HeroRow& r = g_heroes[p->GetGUID().GetCounter()];
        (p->GetActiveSpec() == 0 ? r.spec : r.spec2) = spec.key;
        CharacterDatabase.Execute("UPDATE leyline_bf_heroes SET spec = '{}', spec2 = '{}' WHERE guid = {}", r.spec, r.spec2,
            p->GetGUID().GetCounter());
        return "";
    }

    bool ConsoleCommand(ChatHandler* h, std::vector<std::string> const& w)
    {
        std::string verb = w.empty() ? "" : Lower(w[0]);
        if (verb == "live")   // the website's check that this module is running
        {
            h->SendSysMessage("bfhero live " + std::to_string(MODULE_VERSION));
            return true;
        }
        if (w.size() < 2 || (verb != "go" && verb != "make" && verb != "list" && verb != "delete"))
        {
            h->SendSysMessage("bfhero usage: hero go <account> <boss> <normal|heroic> <bots|group|group-only> [class [spec [spec2]]] "
                "[alliance|horde] | hero make <account> <class> [spec [spec2]] [alliance|horde] | hero list <account> | "
                "hero delete <account> <name> | hero live");
            return true;
        }
        if (!LeylineBFCore::Enabled())
        {
            h->SendSysMessage(ReplyHeroNo("boss fights are off on this realm"));
            return true;
        }
        std::string mainName = Upper(w[1]);
        if (!PlainAccountName(mainName))
        {
            h->SendSysMessage(ReplyHeroNo("bad account name"));
            return true;
        }
        Link l = LinkForMain(AccountMgr::GetId(mainName), mainName, verb == "go" || verb == "make");
        if (!l.error.empty())
        {
            h->SendSysMessage(verb == "list" ? "bfhero list 0 -" : ReplyHeroNo(l.error));
            return true;
        }
        if (verb == "list")
        {
            ListTo(h, l, true);
            return true;
        }
        if (verb == "delete")
        {
            std::string why;
            h->SendSysMessage(w.size() >= 3 && DeleteHero(l, w[2], why) ? "bfhero ok deleted" : ReplyHeroNo(why.empty() ? "which hero?" : why));
            return true;
        }
        JoinArgs a;
        std::size_t pickFrom = 2;
        if (verb == "go")
        {
            if (w.size() < 5)
            {
                h->SendSysMessage(ReplyHeroNo("hero go needs a boss, a difficulty and a mode"));
                return true;
            }
            a = ParseJoin(w[2], w[3], w[4]);
            if (!a.error.empty())
            {
                h->SendSysMessage(ReplyHeroNo(a.error));
                return true;
            }
            pickFrom = 5;
        }
        HeroPick pick = ParseHeroPick(std::vector<std::string>(w.begin() + std::min(pickFrom, w.size()), w.end()));
        if (!pick.error.empty() || (verb == "make" && !pick.cls))
        {
            h->SendSysMessage(ReplyHeroNo(pick.error.empty() ? "hero make needs a class" : pick.error));
            return true;
        }
        bool made = false;
        Made m = HeroForPick(l, pick, made);
        if (!m.error.empty())
        {
            h->SendSysMessage(ReplyHeroNo(m.error));
            return true;
        }
        AccountRow& acc = g_accounts[l.hero];
        acc.current = m.guid;
        if (verb == "go")
        {
            acc.pendingGo = acc.lastGo = HeroGo(*a.boss, a.diff, a.mode);
            acc.pendingUntil = NowUnix() + HERO_INTENT_TTL;
            Player* online = ObjectAccessor::FindConnectedPlayer(ObjectGuid::Create<HighGuid::Player>(m.guid));
            if (online && online->IsInWorld())
            {
                acc.pendingGo.clear();
                QueueJoin(online->GetGUID(), a, 500, 60000);
            }
            else if (!online)
                PrepareOffline(m.guid, a.diff);
        }
        SaveAccount(l.hero);
        h->SendSysMessage(OkLine(l, m, made));
        return true;
    }

    bool PlayerCommand(ChatHandler* h, Player* p, std::vector<std::string> const& w)
    {
        std::string verb = w.empty() ? "list" : Lower(w[0]);
        bool hero = LeylineBFHeroes::IsHeroPlayer(p);
        if (verb == "list" || verb == "heroes")
        {
            Link l = LinkForPlayer(p, false);
            if (!l.error.empty())
                Line(h, "No heroes yet. .bf hero make <class> [spec] makes one (level 80, geared, both specs ready).", false);
            else
                ListTo(h, l, false);
            Line(h, ".bf hero make <class> [spec [second spec]] [alliance|horde], .bf hero delete <name>" +
                std::string(hero ? ", .bf hero spec <spec> (free respec), .bf hero swap (the other spec, now)" : "") + ".", false);
            return true;
        }
        if (verb == "make" || verb == "new")
        {
            HeroPick pick = ParseHeroPick(std::vector<std::string>(w.begin() + 1, w.end()));
            if (!pick.error.empty() || !pick.cls)
            {
                Line(h, "Usage: .bf hero make <class> [spec [second spec]] [alliance|horde]" +
                    (pick.error.empty() ? std::string() : " (" + pick.error + ")"), false);
                return true;
            }
            Link l = LinkForPlayer(p, true);
            if (!l.error.empty())
            {
                Line(h, "Not now: " + l.error + ".", false);
                return true;
            }
            bool made = false;
            Made m = HeroForPick(l, pick, made);
            if (!m.error.empty())
            {
                Line(h, "Not now: " + m.error + ".", false);
                return true;
            }
            g_accounts[l.hero].current = m.guid;
            SaveAccount(l.hero);
            PrepareOffline(m.guid, DIFF_NORMAL);
            Line(h, (made ? "Made " : "You already have ") + m.name + ", level 80 and ready. " +
                (hero ? "Log out to the character list to play it." :
                        "Play it from the website's Boss fights page (Fight now), which signs in to your hero account."), false);
            return true;
        }
        if (verb == "delete" || verb == "remove")
        {
            Link l = LinkForPlayer(p, false);
            std::string why;
            if (w.size() < 2)
                Line(h, "Usage: .bf hero delete <name>", false);
            else if (l.error.empty() && DeleteHero(l, w[1], why))
                Line(h, "Deleted.", false);
            else
                Line(h, "Not deleted: " + (l.error.empty() ? why : l.error) + ".", false);
            return true;
        }
        if (!hero)
        {
            Line(h, "That works on a boss fight hero.", false);
            return true;
        }
        HeroRow const* r = HeroOf(p->GetGUID().GetCounter());
        if (verb == "spec" || verb == "respec")
        {
            SpecDef const* s = w.size() > 1 && r ? FindSpec(r->cls, w[1]) : nullptr;
            if (!s)
            {
                Line(h, "Usage: .bf hero spec <spec> (free; changes the spec you have on now)", false);
                return true;
            }
            std::string why = Respec(p, *s);
            LeylineBFHeroes::SendState(p);
            Line(h, why.empty() ? std::string("Now ") + s->label + ": talents, gear and bars are set." : "Not now: " + why + ".", false);
            return true;
        }
        if (verb == "swap")
        {
            if (p->IsInCombat() || LeylineBFCore::Fighting(p->GetGUID()) || p->GetSpecsCount() < 2)
            {
                Line(h, "Not now: not in combat.", false);
                return true;
            }
            p->ActivateSpec(p->GetActiveSpec() == 0 ? 1 : 0);
            return true;
        }
        Line(h, "Unknown: .bf hero (list), make, delete, spec, swap.", false);
        return true;
    }

    // ---- QoL hooks --------------------------------------------------------------------------------------------------

    bool OnBossMap(std::uint32_t mapId) { return FindBossByMap(mapId) != nullptr; }

    bool Exempt(Player* p) { return p->IsGameMaster() || (p->GetSession() && p->GetSession()->GetSecurity() > SEC_PLAYER); }

    class LeylineBossfightHeroesPlayerScript : public PlayerScript
    {
    public:
        LeylineBossfightHeroesPlayerScript() : PlayerScript("LeylineBossfightHeroesPlayerScript", {
            PLAYERHOOK_ON_LOAD_FROM_DB,
            PLAYERHOOK_ON_BEFORE_TELEPORT,
            PLAYERHOOK_NOT_AVOID_SATISFY,
            PLAYERHOOK_CAN_REPOP_AT_GRAVEYARD,
            PLAYERHOOK_ON_GIVE_EXP,
            PLAYERHOOK_CAN_INIT_TRADE,
            PLAYERHOOK_CAN_SET_TRADE_ITEM,
            PLAYERHOOK_CAN_SEND_MAIL,
            PLAYERHOOK_CAN_PLACE_AUCTION_BID,
            PLAYERHOOK_CAN_JOIN_LFG,
            PLAYERHOOK_CAN_JOIN_IN_BATTLEGROUND_QUEUE,
            PLAYERHOOK_CAN_JOIN_IN_ARENA_QUEUE,
            PLAYERHOOK_ON_BEFORE_SEND_LOOT,
            PLAYERHOOK_ON_AFTER_SPEC_SLOT_CHANGED,
            PLAYERHOOK_ON_DELETE
        }) { }

        void OnPlayerLoadFromDB(Player* player) override { LeylineBFHeroes::OnLoadFromDB(player); }

        bool OnPlayerBeforeTeleport(Player* player, uint32 mapid, float, float, float, float, uint32, Unit*) override
        {
            if (!player || !LeylineBFHeroes::IsHeroPlayer(player) || OnBossMap(mapid) || Exempt(player))
                return true;
            Say(player, "Heroes stay at The Frozen Throne. Log out to the character list to leave.");
            return false;
        }

        bool OnPlayerNotAvoidSatisfy(Player* player, DungeonProgressionRequirements const*, uint32 target_map, bool) override
        {
            return !(player && OnBossMap(target_map) && LeylineBFHeroes::IsHeroPlayer(player));   // false = requirements waived
        }

        bool OnPlayerCanRepopAtGraveyard(Player* player) override
        {
            return !(player && LeylineBFHeroes::IsHeroPlayer(player) && OnBossMap(player->GetMapId()));   // the ghost stays at its body
        }

        void OnPlayerGiveXP(Player* player, uint32& amount, Unit*, uint8) override
        {
            if (player && LeylineBFHeroes::IsHeroPlayer(player))
                amount = 0;
        }

        bool OnPlayerCanInitTrade(Player* player, Player* target) override
        {
            return !((player && LeylineBFHeroes::IsHeroPlayer(player)) || (target && LeylineBFHeroes::IsHeroPlayer(target)));
        }

        bool OnPlayerCanSetTradeItem(Player* player, Item*, uint8) override { return !(player && LeylineBFHeroes::IsHeroPlayer(player)); }

        bool OnPlayerCanSendMail(Player* player, ObjectGuid receiverGuid, ObjectGuid, std::string&, std::string&, uint32, uint32, Item*) override
        {
            return !((player && LeylineBFHeroes::IsHeroPlayer(player)) || LeylineBFHeroes::IsHero(receiverGuid));
        }

        bool OnPlayerCanPlaceAuctionBid(Player* player, AuctionEntry*) override { return !(player && LeylineBFHeroes::IsHeroPlayer(player)); }

        bool OnPlayerCanJoinLfg(Player* player, uint8, std::set<uint32>&, std::string const&) override
        {
            return !(player && LeylineBFHeroes::IsHeroPlayer(player));
        }

        bool OnPlayerCanJoinInBattlegroundQueue(Player* player, ObjectGuid, BattlegroundTypeId, uint8, GroupJoinBattlegroundResult&) override
        {
            return !(player && LeylineBFHeroes::IsHeroPlayer(player));
        }

        bool OnPlayerCanJoinInArenaQueue(Player* player, ObjectGuid, uint8, BattlegroundTypeId, uint8, uint8, GroupJoinBattlegroundResult&) override
        {
            return !(player && LeylineBFHeroes::IsHeroPlayer(player));
        }

        // A heroes-only run leaves no loot: nothing to carry, sell or clutter the bags with.
        void OnPlayerBeforeSendLoot(Player* player, ObjectGuid, Loot* loot) override
        {
            if (player && loot && LeylineBFHeroes::IsHeroPlayer(player) && LeylineBFCore::RunAllHeroes(player->GetGUID()))
                loot->clear();
        }

        // The talent pane's spec switch (or .bf hero swap): that spec's kit goes on.
        void OnPlayerAfterSpecSlotChanged(Player* player, uint8 newSlot) override
        {
            if (!player || !LeylineBFHeroes::IsHeroPlayer(player) || player->IsInCombat())
                return;
            HeroRow const* r = HeroOf(player->GetGUID().GetCounter());
            SpecDef const* s = r ? FindSpec(r->cls, newSlot == 0 ? r->spec : r->spec2) : nullptr;
            if (HeroKit const* kit = s ? KitFor(r->cls, s->kit) : nullptr)
                WearKit(player, *kit, true);
            if (player->getPowerType() == POWER_MANA)
                player->SetPower(POWER_MANA, player->GetMaxPower(POWER_MANA));
            LeylineBFHeroes::SendState(player);
        }

        void OnPlayerDelete(ObjectGuid guid, uint32) override
        {
            if (g_heroes.erase(guid.GetCounter()))
                CharacterDatabase.Execute("DELETE FROM leyline_bf_heroes WHERE guid = {}", guid.GetCounter());
        }
    };
}

namespace LeylineBFHeroes
{
    void LoadTables()
    {
        CharacterDatabase.DirectExecute("CREATE TABLE IF NOT EXISTS leyline_bf_hero_accounts (hero_account_id INT UNSIGNED NOT NULL PRIMARY KEY, "
            "main_account_id INT UNSIGNED NOT NULL, current_guid INT UNSIGNED NOT NULL DEFAULT 0, last_class TINYINT UNSIGNED NOT NULL DEFAULT 0, "
            "last_spec VARCHAR(12) NOT NULL DEFAULT '', last_spec2 VARCHAR(12) NOT NULL DEFAULT '', last_faction TINYINT NOT NULL DEFAULT -1, "
            "last_go VARCHAR(40) NOT NULL DEFAULT 'bf:frozen-throne:normal:bots', pending_go VARCHAR(40) NOT NULL DEFAULT '', "
            "pending_until INT UNSIGNED NOT NULL DEFAULT 0, UNIQUE KEY main (main_account_id))");
        CharacterDatabase.DirectExecute("CREATE TABLE IF NOT EXISTS leyline_bf_heroes (guid INT UNSIGNED NOT NULL PRIMARY KEY, "
            "hero_account_id INT UNSIGNED NOT NULL, class TINYINT UNSIGNED NOT NULL, spec VARCHAR(12) NOT NULL, spec2 VARCHAR(12) NOT NULL, "
            "created_at INT UNSIGNED NOT NULL DEFAULT 0, KEY account (hero_account_id))");
        CharacterDatabase.DirectExecute("CREATE TABLE IF NOT EXISTS leyline_bossfight_live (id TINYINT UNSIGNED NOT NULL PRIMARY KEY, "
            "version INT UNSIGNED NOT NULL DEFAULT 0, beat INT UNSIGNED NOT NULL DEFAULT 0)");
        g_heroes.clear();
        g_accounts.clear();
        if (QueryResult r = CharacterDatabase.Query("SELECT guid, hero_account_id, class, spec, spec2 FROM leyline_bf_heroes"))
        {
            do
            {
                Field* f = r->Fetch();
                HeroRow h;
                h.account = f[1].Get<std::uint32_t>();
                h.cls = f[2].Get<std::uint8_t>();
                h.spec = f[3].Get<std::string>();
                h.spec2 = f[4].Get<std::string>();
                g_heroes[f[0].Get<std::uint32_t>()] = h;
            } while (r->NextRow());
        }
        if (QueryResult r = CharacterDatabase.Query("SELECT hero_account_id, main_account_id, current_guid, last_class, last_spec, last_spec2, "
            "last_faction, last_go, pending_go, pending_until FROM leyline_bf_hero_accounts"))
        {
            do
            {
                Field* f = r->Fetch();
                AccountRow a;
                a.main = f[1].Get<std::uint32_t>();
                a.current = f[2].Get<std::uint32_t>();
                a.lastClass = f[3].Get<std::uint8_t>();
                a.lastSpec = f[4].Get<std::string>();
                a.lastSpec2 = f[5].Get<std::string>();
                a.lastFaction = f[6].Get<std::int8_t>();
                a.lastGo = f[7].Get<std::string>();
                a.pendingGo = f[8].Get<std::string>();
                a.pendingUntil = f[9].Get<std::uint32_t>();
                g_accounts[f[0].Get<std::uint32_t>()] = a;
            } while (r->NextRow());
        }
        CharacterDatabase.DirectExecute("REPLACE INTO leyline_bossfight_live (id, version, beat) VALUES (1, {}, UNIX_TIMESTAMP())", MODULE_VERSION);
        g_lastBeat = NowMs();
        LOG_INFO("module", "mod-leyline-bossfights: {} heroes on {} hero accounts (talents {})", g_heroes.size(), g_accounts.size(),
            LEYLINE_BF_HERO_TALENTS ? "from the bots' premade specs" : "left free: mod-playerbots missing");
    }

    bool IsHero(ObjectGuid guid) { return guid.IsPlayer() && g_heroes.count(guid.GetCounter()) != 0; }

    bool IsHeroPlayer(Player* p) { return p && IsHero(p->GetGUID()); }

    bool InWaitingRoom(Player* p)
    {
        return IsHeroPlayer(p) && OnBossMap(p->GetMapId()) && !LeylineBFCore::InRun(p->GetGUID());
    }

    void ToPlatform(Player* p)
    {
        if (!p)
            return;
        BossDef const& b = Featured();
        if (p->isDead())
        {
            p->ResurrectPlayer(1.0f);
            p->SpawnCorpseBones();
        }
        if (p->GetMapId() == b.mapId)
            p->NearTeleportTo(b.x, b.y, b.z, b.o);
        else
            HopInto(p, b.x, b.y, b.z, b.o);
    }

    void HopInto(Player* p, float x, float y, float z, float o)
    {
        BossDef const& b = Featured();
        if (!p->TeleportTo(b.mapId, x, y, z, o, TELE_TO_GM_MODE, nullptr, true))
            return;
        // TeleportTo has taken the player off the old instance's grid: its own bind can go now, so the new map is the
        // leader's instance (a grouped player) or a fresh one. If the teleport was delayed, the tick does it.
        if (!p->IsInWorld())
            LeylineBFCore::UnbindBossMaps(p);
        else
            g_offlineUnbind.insert(p->GetGUID().GetCounter());
    }

    bool HopFirst(Player* p, JoinArgs const& a)
    {
        if (!InWaitingRoom(p) || !a.boss)
            return false;
        Map* map = p->GetMap();
        bool usedInstance = map && LeylineBFCore::InstanceUsed(p->GetInstanceId());
        ::Difficulty want = a.diff == DIFF_HEROIC ? RAID_DIFFICULTY_10MAN_HEROIC : RAID_DIFFICULTY_10MAN_NORMAL;
        bool otherDifficulty = map && map->GetDifficulty() != want;
        if (!usedInstance && !otherDifficulty)
            return false;
        if (Group* g = p->GetGroup())
            if (g->IsLeader(p->GetGUID()))
                g->SetRaidDifficulty(want);
        p->SetRaidDifficulty(want);
        Say(p, "Getting a fresh raid ready...");
        HopInto(p, a.boss->x, a.boss->y, a.boss->z, a.boss->o);
        QueueJoin(p->GetGUID(), a, 1500, 45000);
        return true;
    }

    void OnLoadFromDB(Player* p)
    {
        if (!IsHeroPlayer(p))
            return;
        BossDef const& b = Featured();
        if (p->GetMapId() != b.mapId)
            return;   // somewhere else (a GM moved it): OnLogin brings it back
        // A raid of its own: the core lets nobody into a raid instance without one (CheckInstanceLoginValid).
        if (!p->GetGroup())
        {
            Group* g = new Group();
            if (g->Create(p))
            {
                sGroupMgr->AddGroup(g);
                g->ConvertToRaid();
            }
            else
                delete g;
        }
        // The fresh instance's place is the raid's entrance: the platform instead, before the first frame.
        p->Relocate(b.x, b.y, b.z, b.o);
        p->UpdatePositionData();
    }

    void OnLogin(Player* p)
    {
        if (!IsHeroPlayer(p))
            return;
        g_offlineUnbind.erase(p->GetGUID().GetCounter());
        // (off the boss maps, which the login path shouldn't allow: the queued join's start brings it to the platform;
        // no teleport from inside the login hook)
        auto it = g_accounts.find(p->GetSession()->GetAccountId());
        std::string go = "bf:frozen-throne:normal:bots";
        if (it != g_accounts.end())
        {
            AccountRow& acc = it->second;
            acc.current = p->GetGUID().GetCounter();
            if (!acc.pendingGo.empty() && acc.pendingUntil > NowUnix())
                go = acc.pendingGo;
            else if (!acc.lastGo.empty())
                go = acc.lastGo;
            acc.pendingGo.clear();
            acc.pendingUntil = 0;
            SaveAccount(it->first);
        }
        JoinArgs a = ParseHeroGo(go);
        if (!a.error.empty())
            a = ParseJoin("", "", "");
        SendState(p);
        QueueJoin(p->GetGUID(), a, 1500, 60000);
    }

    void OnLogout(Player* p)
    {
        if (!IsHeroPlayer(p))
            return;
        g_joins.erase(std::remove_if(g_joins.begin(), g_joins.end(), [p](LoginJoin const& j) { return j.guid == p->GetGUID(); }), g_joins.end());
        if (p->isDead())
            p->ResurrectPlayer(1.0f);   // saved alive: the next login enters the raid map
        if (p->GetGroup())
            p->RemoveFromGroup();
        g_offlineUnbind.insert(p->GetGUID().GetCounter());
    }

    void SendState(Player* p)
    {
        if (!IsHeroPlayer(p))
            return;
        HeroRow const* r = HeroOf(p->GetGUID().GetCounter());
        if (r)
            LeylineBFCore::Addon(p, MsgHero(p->GetName(), r->cls, r->spec, r->spec2, p->GetActiveSpec()));
    }

    void Refresh(Player* p)
    {
        if (!p)
            return;
        p->RemoveAllSpellCooldown();
        p->DurabilityRepairAll(false, 0.0f, false);
        p->SetFullHealth();
        if (p->getPowerType() == POWER_MANA)
            p->SetPower(POWER_MANA, p->GetMaxPower(POWER_MANA));
    }

    // A hero on a boss map with no group (its raid ended, the bots left, it left a run): a raid of its own again, at
    // once, so the core's "not in this instance's raid" timer never sends it out.
    void KeepSoloRaids()
    {
        for (auto const& kv : g_heroes)
        {
            Player* p = ObjectAccessor::FindConnectedPlayer(ObjectGuid::Create<HighGuid::Player>(kv.first));
            if (!p || !p->IsInWorld() || p->IsBeingTeleported() || p->GetGroup() || p->GetGroupInvite() || !OnBossMap(p->GetMapId()))
                continue;
            Group* g = new Group();
            if (!g->Create(p))
            {
                delete g;
                continue;
            }
            sGroupMgr->AddGroup(g);
            g->ConvertToRaid();
            g->SetRaidDifficulty(p->GetMap() ? p->GetMap()->GetDifficulty() : RAID_DIFFICULTY_10MAN_NORMAL);
        }
    }

    void Tick()
    {
        std::uint64_t now = NowMs();
        if (now - g_lastBeat >= 60000)
        {
            g_lastBeat = now;
            CharacterDatabase.Execute("REPLACE INTO leyline_bossfight_live (id, version, beat) VALUES (1, {}, UNIX_TIMESTAMP())", MODULE_VERSION);
        }
        static std::uint64_t lastSolo = 0;
        if (now - lastSolo >= 1000)
        {
            lastSolo = now;
            KeepSoloRaids();
        }
        for (auto it = g_offlineUnbind.begin(); it != g_offlineUnbind.end();)
        {
            ObjectGuid g = ObjectGuid::Create<HighGuid::Player>(*it);
            Player* p = ObjectAccessor::FindConnectedPlayer(g);
            if (!p)
            {
                UnbindOffline(*it);
                it = g_offlineUnbind.erase(it);
            }
            else if (!p->IsInWorld())
            {
                LeylineBFCore::UnbindBossMaps(p);
                it = g_offlineUnbind.erase(it);
            }
            else if (!p->IsBeingTeleportedFar())
                it = g_offlineUnbind.erase(it);   // arrived (or never left): nothing to drop now
            else
                ++it;
        }
        std::vector<LoginJoin> ready;
        g_joins.erase(std::remove_if(g_joins.begin(), g_joins.end(), [&](LoginJoin const& j)
        {
            if (now > j.until)
                return true;
            if (now < j.at)
                return false;
            Player* p = ObjectAccessor::FindConnectedPlayer(j.guid);
            if (!p)
                return true;
            if (!p->IsInWorld() || p->IsBeingTeleported())
                return false;
            ready.push_back(j);
            return true;
        }), g_joins.end());
        for (LoginJoin const& j : ready)
            if (Player* p = ObjectAccessor::FindConnectedPlayer(j.guid))
            {
                std::string why = LeylineBFCore::Join(p, j.args);
                if (!why.empty())
                    Say(p, why);
            }
    }

    bool Command(ChatHandler* handler, std::string const& args)
    {
        std::vector<std::string> w = Words(args);
        Player* p = handler->GetSession() ? handler->GetSession()->GetPlayer() : nullptr;
        if (!p)
            return ConsoleCommand(handler, w);
        return PlayerCommand(handler, p, w);
    }

    void AddScripts()
    {
        new LeylineBossfightHeroesPlayerScript();
    }
}
