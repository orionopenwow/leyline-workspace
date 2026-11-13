-- ov_companions.lua: Companions, bot party members for dungeons and 10-player raids (mod-ale, 3.3.5a).
-- Players always see them called bots.
--
--   .companions                     help, your bots and the limits (.companions list is the same)
--   .companions alts                the other characters on your account you can bring as bots
--   .companions save                remembers the alts with you as your usual party (per account)
--   .recruit party                  fills a 5-player group: 1 tank, 1 healer, damage
--   .recruit raid                   fills a 10-player raid: 2 tanks, 3 healers, 5 damage (minus the roles the
--                                   group already has), converting the party to a raid
--   .recruit tank|healer|dps [n]    recruits n (default 1); .recruit dps mage picks the class
--   .recruit alt <name>             one of your own characters, with its own gear, spec and level
--   .recruit usual                  brings back the alts saved with .companions save
--   .buddy [class]                  one levelling buddy: your level, follows you, re-matched when you level up
--   .dismiss [name|all]             sends bots home (alts too)
--
-- Owner decisions (final): 10-player raids only (25 behind OV_COMPANIONS_RAID25, off); bots ALWAYS pass on
-- loot, alts included. The rolls are decided by mod-playerbots' global loot keys, set in the realm profile
-- (realm/serverpass.sh, profiles/main.env): Playerbots.LootNeedRollLevel = 0 turns every NEED into PASS,
-- LootGreedRollLevel = 0 turns every GREED into PASS, LootRollDisenchant = 0 (no disenchant votes),
-- LootRollRecipe = 0, FreeMethodLoot = 0 (no looting under free-for-all). Under master loot and free-for-all
-- the bots always vote PASS (LootRollAction.cpp, e88cc40). Per bot, on arrival, we also whisper "ll normal"
-- (the bot's own loot list setting: an alt may carry "all" from an earlier session). What can't be
-- prevented: items below the group's loot threshold (grey and white by default) go round-robin, and a bot
-- picks up its turn's share like any member. Free-for-all avoids that: bots never loot under it.
--
-- This drives mod-playerbots (the realm image's core is its fork, custom/sources.lock); none of the bot AI is
-- here. Everything goes through the player's own chat commands (Player:RunCommand, run as if typed, with the
-- player's own security) and whispers to the bot (Player:Whisper reaches mod-playerbots' OnPlayerCanUseChat):
--   .playerbots bot addclass <class>    an addclass-pool character of the player's faction logs in, is levelled
--                                       and geared to the player when 4+ levels apart, and joins the player's
--                                       group (a 6th member converts the party to a raid; PlayerbotMgr.cpp)
--   .playerbots bot add <name>          an alt of the player's own account logs in as a bot (AllowAccountBots)
--   .playerbots bot init=auto <name>    re-levels / re-gears an addclass bot to the player (gear-score capped)
--   .playerbots bot remove <name>       logs it out
--   whisper "talents spec <premade>"    the role's premade spec (Playerbots.PremadeSpecName.<class>.<n>)
--   whisper "maintenance", "autogear"   spells, talents, consumables, repairs; gear up to AutoGearQualityLimit
--   whisper "summon", "follow", "ll normal"
-- Alts keep their own gear, spec and level: they get no init=auto, talents, maintenance or autogear.
-- The bot logs in asynchronously, so .recruit only queues; a timer on the player (every 2 s, 8 times) claims the
-- bots that join the group, brings them over, matches the level, sets the spec.
-- Recruits that never arrive (no free character of that class in the pool) expire with a message.
--
-- Where: not in battlegrounds or arenas, not while dead or in combat. Inside a dungeon or raid only while
-- nobody in the group is in combat (refill a slot between pulls); there a bot on the same map and instance
-- is moved to the player, any other is fetched with mod-playerbots' own "summon".
--
-- Without mod-playerbots (the stock image: no Player:IsPlayerbot method) every command answers "coming soon";
-- realm/companions-README.md lists what the build needs.
--
-- Talking to a bot: through its own PlayerbotAI:HandleCommand(CHAT_MSG_WHISPER, text, owner) when mod-ale has it
-- (the same queue a whisper reaches, without the "To <bot>: maintenance" lines in the owner's chat), else
-- Player:Whisper. Strategy switches (passive, crowd control, assist) use PlayerbotAI:ChangeStrategy, which says
-- nothing back. Never remove group members from Lua (see dismissCommand).
--
-- Who owns a bot survives a crash or a script reload: the bots in the player's group whose PlayerbotAI master is
-- the player are adopted back (adopt, on every command and 15 s after login); without GetBotAI, the table
-- mod_companions_owned (bot guid -> owner) decides. Rows go on dismiss and on a normal logout.
--
-- The Bots window (WrathUnleashedUI/Companions.lua, /companions, /comp) talks over the OVMOD channel
-- (ov_modapi.lua), mod "wucomp". Nothing goes through chat. Every request is checked here: the bot must be the
-- player's own, heavy orders wait for the end of combat, and each has a cooldown.
--   client -> server   roster                                  send me my bots
--                      order <what> <name|all>                 what: follow stay attack passive active assist
--                                                              noassist cc nocc pull (tanks attack the
--                                                              owner's target; without a tank bot, everyone)
--                                                              regroup (follow; a far bot is summoned) flee
--                                                              (mod-playerbots' "flee": follow and passive)
--                                                              drink (bots with mana drink now)
--                                                              tactics, each with its "no" form, kept per owner
--                                                              and given to bots that join later:
--                                                              waitpull (damage waits for the tank: +wait for
--                                                              attack, +threat, "wait for attack time 3"),
--                                                              healtank (healers "focus heal <the tank>", the
--                                                              owner when no tank bot), savemana (healers +save
--                                                              mana), aoe (damage +dps aoe; off: +dps assist)
--                      inspect <name>                          send me that bot's gear and the upgrades in its bags
--                      equip <name>\t<item entry>              wear that item from its bags ("e |Hitem:<entry>:")
--                      upgrade <name>                          wear every upgrade in its bags ("equip upgrade")
--                      formation <near|melee|line|circle|arrow|shield|queue|chaos>
--                      prepare                                 maintenance, autogear (not alts), rebuff
--                      fill <party|raid>[\t<picks>]  usual  dismiss <name|all>
--                      spec <name>\t<premade spec>             role <name> <tank|healer|dps>   gear <name>
--                      alts                                    send me my alts and my usual party
--                      alt <name>   save                       .recruit alt <name>, .companions save
--                      buddy <class|any|off>                   .buddy [class], .buddy off
--                      dungeon <id>[\t<picks>]                 fill the party for that dungeon (ov_companions_dungeons.lua)
--                                                              and meet at its door (see dungeonCommand)
--                      <picks>: the window's per-slot choices, "tank=warrior:prot pve,healer=any,dps=mage,dps=any"
--                      (role=class[:premade spec]); a pick that doesn't fit the role is ignored (the usual pick)
--   server -> client   roster  H\t<count>\t<cap>\t<raid 0|1>\t<formation>\t<assist 0|1>\t<cc 0|1>\t<state>
--                              \t<buddy on 0|1>\t<raids on 0|1>\t<waitpull 0|1>\t<healtank 0|1>\t<savemana 0|1>
--                              \t<aoe 0|1>
--                              B\t<name>\t<class id>\t<role>\t<kind bot|alt|buddy>\t<spec>\t<item level>\t<status>\t<level>
--                              S\t<class id>\t<spec>=<role>,...     (the realm's premade specs per class present)
--                      msg <text>                              the answer the chat would have had
--                      alts    A\t<name>\t<level>\t<class id>\t<with you 0|1>\t<online 0|1>   U\t<name> (the usual party)
--                      gear    G\t<name>\t<class id>\t<level>\t<item level>\t<kind>
--                              I\t<slot>\t<entry>\t<item level>\t<quality>\t<item name>      (what it wears, slots 0-17)
--                              U\t<slot>\t<entry>\t<item level>\t<quality>\t<item name>\t<now> (the best upgrade in
--                              its bags for that slot, against the item level it wears there now)
-- After a window request, the answers to it (and the arrivals it starts) go to the window, not the chat; a typed
-- command switches them back to the chat.
--
-- Env (os.getenv at each use; "1" on, "0" off):
--   OV_COMPANIONS                 on/off                                                   default 1
--   OV_COMPANIONS_PER_PLAYER      bots one player may have in a party (buddy, alts included) default 4
--   OV_COMPANIONS_PER_PLAYER_RAID bots one player may have in a raid group                 default 9
--   OV_COMPANIONS_REALM_MAX       bots on the whole realm at once                          default 60
--   OV_COMPANIONS_GROUP_MAX       the largest group .recruit fills (5..10; ..25 with RAID25) default 10
--   OV_COMPANIONS_RAID25          1 lets .recruit raid 25 fill a 25-player raid            default 0
--   OV_COMPANIONS_COOLDOWN        seconds between two .recruit / .buddy                    default 30
--   OV_COMPANIONS_BUDDY           .buddy on/off                                            default 1
--   OV_COMPANIONS_DUNGEON_TRAVEL  the Dungeons tab takes you to the door                   default 1
--   OV_COMPANIONS_DUNGEON_COOLDOWN seconds between two trips to a dungeon door             default 300
-- Single player (realm/profiles/solo.env, solovanilla.env; the launcher may override them in the world's realm.env,
-- docs/SINGLE-PLAYER-ONE-CLICK.md "Companions settings"). OFF unless the profile turns them on: main.env never does.
--   OV_COMPANIONS_AUTO            bots join a character by themselves: the first time it enters the world (with a
--                                 welcome line), and again at each login unless the player sent them home default 0
--   OV_COMPANIONS_FOLLOW          how many join by themselves (0..4; never past OV_COMPANIONS_PER_PLAYER) default 2
--   OV_COMPANIONS_DUNGEON_AUTO    entering a 5-player dungeon (on foot, from the Bots window or the Dungeon Finder)
--                                 fills the party with dungeon bots; leaving sends just those home          default 0
-- See "single player" below for the party they make and how the choice is kept per character.
-- mod-playerbots' own Playerbots.MaxAddedBots (AC_PLAYERBOTS_MAX_ADDED_BOTS, 9) is the hard cap behind these.
-- Characters database: mod_companions_usual (account, slot, guid, name, saved_at), mod_companions_owned
-- (bot, owner, name, role, spec, kind, at) and mod_companions_auto (guid, mode, party, at: single player's choice per
-- character), made here if missing.
-- mod-ale events: 3 ON_LOGIN, 4 ON_LOGOUT, 13 ON_LEVEL_CHANGE, 28 ON_MAP_CHANGE, 42 ON_COMMAND (return false = handled).
--
-- Achievement lines ("<bot> has earned the achievement ...") come from the core (AchievementMgr::
-- SendAchievementEarned tells everyone in say range) when a bot is levelled, geared and taught spells and mounts
-- (init=auto, maintenance). Neither mod-playerbots (e88cc40) nor mod-ale can stop the line before it is sent, so
-- the Bots window's addon hides achievement lines from the player's own bots on the player's screen.

local OVMod
do
    -- ALE loads scripts in name order, so ov_modapi.lua has not run yet: require loads it once. Without it the
    -- window is off; every chat command still works.
    local ok, m = pcall(function() return require("ov_modapi") end)
    OVMod = (ok and type(m) == "table" and m) or _G.OVMod
end
local MOD = "wucomp"

-- The Dungeons tab's list (ov_companions_dungeons.lua, generated by stream/modes_data.py): id -> { name, map, min,
-- max, pos = { map, x, y, z, o } }. Without the file the tab's requests answer "not on the list".
local DUNGEONS = {}
do
    local ok, list = pcall(require, "ov_companions_dungeons")
    if ok and type(list) == "table" then
        for _, d in ipairs(list) do
            if type(d) == "table" and type(d.id) == "string" and d.id:match("^[%w%-]+$") and type(d.name) == "string"
                and tonumber(d.map) and tonumber(d.min) then
                DUNGEONS[d.id] = d
            end
        end
    end
end

local TAG = "|cff66ccff[Bots]|r "

------------------------------------------------------------------------------------------------
-- env
------------------------------------------------------------------------------------------------
local function flag(name, default)
    local v = os.getenv(name)
    if v == "1" then return true end
    if v == "0" then return false end
    return default
end

local function num(name, default, lo, hi)
    local v = tonumber(os.getenv(name) or "")
    if not v then v = default end
    v = math.floor(v)
    if v < lo then v = lo end
    if v > hi then v = hi end
    return v
end

local function onSwitch()      return flag("OV_COMPANIONS", true) end
local function buddyOn()       return flag("OV_COMPANIONS_BUDDY", true) end
local function raid25()        return flag("OV_COMPANIONS_RAID25", false) end
local function raidSizeMax()   return raid25() and 25 or 10 end
local function perPlayer()     return num("OV_COMPANIONS_PER_PLAYER", 4, 0, 4) end
local function perPlayerRaid() return num("OV_COMPANIONS_PER_PLAYER_RAID", 9, 0, raidSizeMax() - 1) end
local function realmMax()      return num("OV_COMPANIONS_REALM_MAX", 60, 0, 2000) end
local function groupMax()      return num("OV_COMPANIONS_GROUP_MAX", 10, 2, raidSizeMax()) end
local function cooldown()      return num("OV_COMPANIONS_COOLDOWN", 30, 0, 3600) end
local function dungeonTravel() return flag("OV_COMPANIONS_DUNGEON_TRAVEL", true) end
local function travelCooldown() return num("OV_COMPANIONS_DUNGEON_COOLDOWN", 300, 0, 86400) end
-- single player: off by default, so a realm whose profile doesn't name them (the official one) never does this
local function autoOn()        return onSwitch() and flag("OV_COMPANIONS_AUTO", false) end
local function autoCount()     return num("OV_COMPANIONS_FOLLOW", 2, 0, 4) end
local function dungeonAutoOn() return onSwitch() and flag("OV_COMPANIONS_DUNGEON_AUTO", false) end

local PARTY = 5

local replyTo = {}   -- [ownerGuid] = true while the answers go to the Bots window

local function say(player, text)
    if replyTo[player:GetGUIDLow()] and OVMod then
        OVMod.Send(player, MOD, "msg", text)
        return
    end
    player:SendBroadcastMessage(TAG .. text)
end

local function try(obj, method, ...)
    if not obj then return nil end
    local ok, fn = pcall(function() return obj[method] end)
    if not ok or type(fn) ~= "function" then return nil end
    local ok2, a, b = pcall(fn, obj, ...)
    if ok2 then return a, b end
    return nil
end

local function hasMethod(obj, method)
    local ok, fn = pcall(function() return obj[method] end)
    return ok and type(fn) == "function"
end

-- mod-ale registers IsPlayerbot only when built with MOD_PLAYERBOTS.
local function botsBuilt(player) return hasMethod(player, "IsPlayerbot") and hasMethod(player, "RunCommand") end

-- The bot's PlayerbotAI (mod-ale's Player:GetBotAI), or nil.
local function botAI(m)
    local ai = try(m, "GetBotAI")
    if type(ai) ~= "table" and type(ai) ~= "userdata" then return nil end
    return ai
end

local CHAT_MSG_WHISPER = 7
local BOT_STATE_COMBAT, BOT_STATE_NON_COMBAT = 0, 1

local function isBot(p)
    if try(p, "IsPlayerbot") == true or try(p, "IsBot") == true then return true end
    local name = try(p, "GetAccountName")
    return type(name) == "string" and name:upper():find("^RNDBOT") ~= nil
end

-- A bot of the addclass pool (what .recruit <role> makes); alt bots of the player's own account are not.
local function isAddclassBot(owner, m)
    local v = try(m, "IsAddclassBot")
    if v ~= nil then return v == true end
    return isBot(m) and try(m, "GetAccountId") ~= owner:GetAccountId()
end

local function inPvp(player)
    if try(player, "InBattleground") or try(player, "InArena") then return true end
    local map = try(player, "GetMap")
    return map ~= nil and (try(map, "IsBattlegroundOrArena") or try(map, "IsBattleground") or try(map, "IsBattleArena")) == true
end

-- "dungeon", "raid" or nil (open world)
local function instanceKind(player)
    local map = try(player, "GetMap")
    if not map then return nil end
    if try(map, "IsRaid") == true then return "raid" end
    if try(map, "IsDungeon") == true then return "dungeon" end
    return nil
end

local function plural(n, word) return n .. " " .. word .. (n == 1 and "" or "s") end

------------------------------------------------------------------------------------------------
-- roles
------------------------------------------------------------------------------------------------
local CLASS = {
    warrior = 1, paladin = 2, hunter = 3, rogue = 4, priest = 5, dk = 6, shaman = 7, mage = 8, warlock = 9, druid = 11,
}
local CLASS_ALIAS = { deathknight = "dk", ["death-knight"] = "dk", lock = "warlock", pally = "paladin",
    pal = "paladin", war = "warrior", sham = "shaman", hunt = "hunter" }
local CLASS_LABEL = { [1] = "Warrior", [2] = "Paladin", [3] = "Hunter", [4] = "Rogue", [5] = "Priest",
    [6] = "Death Knight", [7] = "Shaman", [8] = "Mage", [9] = "Warlock", [11] = "Druid" }

-- class -> premade spec name (mod-playerbots' playerbots.conf.dist PremadeSpecName.<class>.<n>, pve)
local ROLES = {
    tank   = { label = "tank",   picks = { { "warrior", "prot pve" }, { "paladin", "prot pve" },
                                           { "druid", "bear pve" }, { "dk", "blood pve" } } },
    healer = { label = "healer", picks = { { "priest", "holy pve" }, { "shaman", "resto pve" },
                                           { "druid", "resto pve" }, { "paladin", "holy pve" } } },
    dps    = { label = "damage", picks = { { "mage", "frost pve" }, { "hunter", "mm pve" },
                                           { "rogue", "combat pve" }, { "warlock", "affli pve" },
                                           { "shaman", "ele pve" }, { "priest", "shadow pve" },
                                           { "warrior", "fury pve" }, { "dk", "frost pve" } } },
}
local ROLE_ORDER = { "tank", "healer", "dps" }
local ROLE_ALIAS = { heal = "healer", heals = "healer", healers = "healer", tanks = "tank", damage = "dps",
    dd = "dps", melee = "dps", caster = "dps" }
local BUDDY_CLASSES = { "warrior", "paladin", "hunter", "rogue", "priest", "shaman", "mage", "warlock", "druid", "dk" }

-- What a full group of each size wants.
local SHAPE = {
    [5]  = { tank = 1, healer = 1, dps = 3 },
    [10] = { tank = 2, healer = 3, dps = 5 },
    [25] = { tank = 3, healer = 6, dps = 16 },
}

-- The premade specs a bot may be given, per class: the realm's own (AC_PLAYERBOTS_PREMADE_SPEC_NAME_<class>__<n>,
-- realm/playerbots-specs.env), pve only; these if the env has none (the pinned playerbots.conf.dist's pve names).
local FALLBACK_SPECS = {
    [1] = { "arms pve", "fury pve", "prot pve" }, [2] = { "holy pve", "prot pve", "ret pve" },
    [3] = { "bm pve", "mm pve", "surv pve" }, [4] = { "as pve", "combat pve", "subtlety pve" },
    [5] = { "disc pve", "holy pve", "shadow pve" }, [6] = { "blood pve", "frost pve", "unholy pve", "double aura blood pve" },
    [7] = { "ele pve", "enh pve", "resto pve" }, [8] = { "arcane pve", "fire pve", "frost pve", "frostfire pve" },
    [9] = { "affli pve", "demo pve", "destro pve" }, [11] = { "balance pve", "bear pve", "resto pve", "cat pve" },
}
local TANK_SPECS = { ["prot pve"] = true, ["bear pve"] = true, ["blood pve"] = true }
local HEAL_SPECS = { ["holy pve"] = true, ["resto pve"] = true, ["disc pve"] = true }

local function specRole(spec)
    if TANK_SPECS[spec] then return "tank" end
    if HEAL_SPECS[spec] then return "healer" end
    return "dps"
end

local function specsFor(cls)
    local out = {}
    for n = 0, 9 do
        local v = os.getenv("AC_PLAYERBOTS_PREMADE_SPEC_NAME_" .. tostring(cls) .. "__" .. n)
        if v and v:match("^[%a ]+ pve$") and #v <= 32 then out[#out + 1] = v end
    end
    if #out == 0 then for _, v in ipairs(FALLBACK_SPECS[cls] or {}) do out[#out + 1] = v end end
    return out
end

local function dkLevel()
    return tonumber(os.getenv("AC_START_HEROIC_PLAYER_LEVEL") or "") or 55
end

-- A world held to the first era (level cap 60 or under: the original world, solovanilla.env) has no Death Knights.
local function dkInWorld() return (tonumber(os.getenv("AC_MAX_PLAYER_LEVEL") or "") or 80) > 60 end
local function dkOk(player) return dkInWorld() and player:GetLevel() >= dkLevel() end
local function dkRefusal()
    if not dkInWorld() then return "Death Knights aren't in this world." end
    return "Death Knights join from level " .. dkLevel() .. "."
end

local function classOf(word)
    word = CLASS_ALIAS[word] or word
    return CLASS[word] and word or nil
end

-- How many of each role to recruit: what the shape wants minus what the group has, tanks first, then
-- healers, then damage; n slots at most, any left over go to damage.
local function planRoles(have, shape, n)
    local out = { tank = 0, healer = 0, dps = 0 }
    for _, r in ipairs(ROLE_ORDER) do
        local need = math.max(0, (shape[r] or 0) - (have[r] or 0))
        local take = math.min(need, n)
        out[r] = take
        n = n - take
    end
    out.dps = out.dps + math.max(0, n)
    return out
end

------------------------------------------------------------------------------------------------
-- state (per world server run; mod-playerbots logs a player's bots out when the player logs out)
------------------------------------------------------------------------------------------------
local companions = {}   -- [ownerGuid] = { [botGuid] = { name, cls, role, spec, buddy, alt, order, passive } }
local orders = {}       -- [ownerGuid] = { formation, assist, cc, waitpull, healtank, savemana, aoe }: the window's
                        -- group-wide switches (the tactics are given to bots that join later too: applyTactics)
local pending = {}      -- [ownerGuid] = { { cls, word, spec, role, buddy, alt, guid, name, at }, ... }
local lastRecruit = {}  -- [ownerGuid] = os.time()
local ticking = {}      -- [ownerGuid] = the running arrival timer's number
local raidWanted = {}   -- [ownerGuid] = true while a raid fill is arriving
local autoJoined = {}   -- [ownerGuid] = { kind = "first"|"back"|"dungeon", names = { "Anna the Priest (healer)" } }
local hasWindow = {}    -- [ownerGuid] = true once the player's Bots window has asked for something (it is there)

local ARRIVE_TICK_MS, ARRIVE_TICKS, ARRIVE_TIMEOUT = 2000, 8, 15
local TELEPORT_DISTANCE = 40

local function countOf(t) local n = 0; for _ in pairs(t or {}) do n = n + 1 end; return n end
local function mine(guid) return countOf(companions[guid]) + #(pending[guid] or {}) end
local function realmCount()
    local n = 0
    for g in pairs(companions) do n = n + countOf(companions[g]) end
    for g in pairs(pending) do n = n + #pending[g] end
    return n
end

local function groupMembers(player)
    local group = try(player, "GetGroup")
    return group, (group and try(group, "GetMembers")) or { player }
end

local function groupSize(player)
    local group = try(player, "GetGroup")
    if not group then return 1 end
    return tonumber(try(group, "GetMembersCount")) or #(try(group, "GetMembers") or {})
end

local function isRaidGroup(player)
    local group = try(player, "GetGroup")
    return group ~= nil and try(group, "IsRaidGroup") == true
end

local function groupInCombat(player)
    local _, members = groupMembers(player)
    for _, m in ipairs(members) do
        if try(m, "IsInCombat") then return true end
    end
    return false
end

local function buddyOf(guid)
    for bg, c in pairs(companions[guid] or {}) do if c.buddy then return bg, c end end
    for _, p in ipairs(pending[guid] or {}) do if p.buddy then return nil, p end end
    return nil
end

local adopt, ownForget   -- below, with the database

-- Drop companions that left the group or logged out (the player kicked them, or a server hiccup), and take back
-- the player's bots we lost track of (a crash or a script reload empties the tables; the bots stay).
local function prune(player)
    local guid = player:GetGUIDLow()
    adopt(player)
    local list = companions[guid]
    if not list then return end
    local inGroup = {}
    local _, members = groupMembers(player)
    for _, m in ipairs(members) do inGroup[m:GetGUIDLow()] = m end
    for bg in pairs(list) do
        if not inGroup[bg] then list[bg] = nil; ownForget(bg) end
    end
end

-- Character names: letters only (UTF-8 letters pass as bytes 128-255), at most 12 characters (24 bytes).
-- Never holds a quote, space or semicolon, so it is safe in a command line and an SQL string.
local function safeName(n)
    n = tostring(n or "")
    if n == "" or #n > 24 or n:find("[^%a\128-\255]") then return "" end
    return n
end

local selfCommand = false   -- true while we run our own bot command (so the raw-command guard lets it through)
local function botCommand(owner, text)
    selfCommand = true
    local ok, err = pcall(owner.RunCommand, owner, "playerbots bot " .. text)
    selfCommand = false
    if not ok then error(err, 0) end
end
local function whisper(owner, bot, text) try(owner, "Whisper", text, 0, bot) end

-- A command to one bot, as if the owner whispered it: straight into the bot's command queue when mod-ale has
-- PlayerbotAI:HandleCommand (no whisper lines in the owner's chat), else a real whisper.
local function tell(owner, bot, text)
    local ai = botAI(bot)
    if ai and hasMethod(ai, "HandleCommand") then
        local ok = pcall(function() ai:HandleCommand(CHAT_MSG_WHISPER, text, owner) end)
        if ok then return end
    end
    whisper(owner, bot, text)
end

-- A strategy change for combat and out of combat ("+passive", "-cc,+dps assist"): silent through
-- PlayerbotAI:ChangeStrategy, else the bot's own "co"/"nc" commands.
local function strategy(owner, bot, change)
    local ai = botAI(bot)
    if ai and hasMethod(ai, "ChangeStrategy") then
        local ok = pcall(function()
            ai:ChangeStrategy(change, BOT_STATE_COMBAT)
            ai:ChangeStrategy(change, BOT_STATE_NON_COMBAT)
        end)
        if ok then return end
    end
    tell(owner, bot, "co " .. change)
    tell(owner, bot, "nc " .. change)
end

-- The role a group member covers: our record for our bots, else the spec (mod-playerbots' IsTank/IsHealer
-- by talents, which works for real players too), else damage.
local function roleOf(owner, m)
    local c = (companions[owner:GetGUIDLow()] or {})[m:GetGUIDLow()]
    if c and ROLES[c.role or ""] then return c.role end
    if try(m, "IsTank", true) == true then return "tank" end
    if try(m, "IsHealer", true) == true then return "healer" end
    return "dps"
end

local function rolesInGroup(player)
    local have = { tank = 0, healer = 0, dps = 0 }
    local _, members = groupMembers(player)
    for _, m in ipairs(members) do
        local r = roleOf(player, m)
        have[r] = have[r] + 1
    end
    for _, p in ipairs(pending[player:GetGUIDLow()] or {}) do
        local r = ROLES[p.role or ""] and p.role or "dps"
        have[r] = have[r] + 1
    end
    return have
end

------------------------------------------------------------------------------------------------
-- arrival: claim the bots that joined, set them up
------------------------------------------------------------------------------------------------
local ownSave, pushRoster, autoArrived, autoDismissed, applyTactics   -- below

local function claimed(botGuid)
    for _, list in pairs(companions) do if list[botGuid] then return true end end
    return false
end

-- Open world: move a far bot to the player. Inside an instance: only a bot already on the same map and
-- instance is moved; any other is fetched with mod-playerbots' own summon (its checks, its instance rules).
local function bringOver(owner, m)
    local sameMap = try(m, "GetMapId") == try(owner, "GetMapId")
    local d = tonumber(try(m, "GetDistance", owner))
    local far = not sameMap or (d ~= nil and d > TELEPORT_DISTANCE)
    if not far then return end
    if not instanceKind(owner) then
        try(m, "Teleport", owner:GetMapId(), owner:GetX(), owner:GetY(), owner:GetZ(), owner:GetO())
        return
    end
    local mi, oi = try(m, "GetInstanceId"), try(owner, "GetInstanceId")
    if sameMap and mi ~= nil and mi == oi then
        try(m, "Teleport", owner:GetMapId(), owner:GetX(), owner:GetY(), owner:GetZ(), owner:GetO())
    else
        tell(owner, m, "summon")
    end
end

local function setUp(owner, m, p)
    local name = safeName(m:GetName())
    bringOver(owner, m)
    if not p.alt then
        if m:GetLevel() ~= owner:GetLevel() and name ~= "" then
            botCommand(owner, "init=auto " .. name)
        end
        if p.spec then tell(owner, m, "talents spec " .. p.spec) end
        if not p.buddy then
            tell(owner, m, "maintenance")
            tell(owner, m, "autogear")
        end
    end
    tell(owner, m, "ll normal")
    tell(owner, m, "follow")
    if applyTactics then applyTactics(owner, m, ROLES[p.role or ""] and p.role or roleOf(owner, m)) end
end

local function convertIfNeeded(owner)
    local guid = owner:GetGUIDLow()
    if not raidWanted[guid] then return end
    local group = try(owner, "GetGroup")
    if group and try(group, "IsRaidGroup") == false then try(group, "ConvertToRaid") end
end

local function arrivals(owner)
    local guid = owner:GetGUIDLow()
    local list = pending[guid]
    if not list or #list == 0 then return end
    convertIfNeeded(owner)
    local joined = false
    local _, members = groupMembers(owner)
    for _, m in ipairs(members) do
        local bg = m:GetGUIDLow()
        if bg ~= guid and isBot(m) and not claimed(bg) then
            local at
            if isAddclassBot(owner, m) then
                local cls = m:GetClass()
                for i, p in ipairs(list) do if not p.alt and p.cls == cls then at = i; break end end
            elseif try(m, "GetAccountId") == owner:GetAccountId() then
                for i, p in ipairs(list) do if p.alt and p.guid == bg then at = i; break end end
            end
            if at then
                local p = table.remove(list, at)
                local cls = m:GetClass()
                local role = p.role
                if p.alt then role = roleOf(owner, m) end
                companions[guid] = companions[guid] or {}
                companions[guid][bg] = { name = m:GetName(), cls = cls, role = role, spec = p.spec, buddy = p.buddy,
                    alt = p.alt, order = "follow", keep = p.auto ~= nil or nil, dungeon = p.dungeon or nil }
                ownSave(guid, bg, companions[guid][bg])
                setUp(owner, m, p)
                joined = true
                if p.auto or p.dungeon then
                    autoArrived(owner, p, m)   -- one line for the whole party, once they are all here
                elseif p.buddy then
                    say(owner, string.format("%s the %s is your levelling buddy bot. Say 'stay' or 'follow' in party chat.",
                        m:GetName(), CLASS_LABEL[cls] or "bot"))
                elseif p.alt then
                    say(owner, string.format("%s (your %s) joins as a bot.", m:GetName(), CLASS_LABEL[cls] or "alt"))
                else
                    say(owner, string.format("%s the %s joins as a %s bot.", m:GetName(), CLASS_LABEL[cls] or "bot",
                        ROLES[role] and ROLES[role].label or role))
                end
            end
        end
    end
    if #list == 0 then raidWanted[guid] = nil end
    if joined then pushRoster(owner) end
    if #list == 0 then autoArrived(owner) end
end

local function expire(owner, final)
    local guid = owner:GetGUIDLow()
    local list = pending[guid]
    if not list then return end
    local now, kept, lost, alts, autoLost = os.time(), {}, 0, {}, 0
    for _, p in ipairs(list) do
        if final or now - p.at >= ARRIVE_TIMEOUT then
            if p.alt then alts[#alts + 1] = p.name
            elseif p.auto or p.dungeon then autoLost = autoLost + 1
            else lost = lost + 1 end
        else
            kept[#kept + 1] = p
        end
    end
    pending[guid] = kept
    if #kept == 0 then raidWanted[guid] = nil end
    if lost > 0 then
        say(owner, string.format("%s didn't arrive (none free of that class right now). Try another class or role.",
            lost == 1 and "One bot" or (lost .. " bots")))
    end
    if #alts > 0 then
        say(owner, table.concat(alts, ", ") .. " didn't arrive. Try again in a minute.")
    end
    if autoLost > 0 then
        autoArrived(owner)
        owner:SendBroadcastMessage(TAG .. string.format("%s couldn't join right now (none free). The Bots window can "
            .. "bring more later.", autoLost == 1 and "One bot" or (autoLost .. " bots")))
    end
end

-- One timer per player at a time; a newer one (a later .recruit) takes over, the older one's ticks do nothing.
local timerGen = 0
local function watchArrivals(player)
    local guid = player:GetGUIDLow()
    timerGen = timerGen + 1
    local gen, ticks = timerGen, 0
    ticking[guid] = gen
    player:RegisterEvent(function(_, _, _, p)
        if ticking[guid] ~= gen then return end
        ticks = ticks + 1
        if not p then ticking[guid] = nil; return end
        local ok, err = pcall(arrivals, p)
        if not ok then print("[ov_companions] arrival check failed: " .. tostring(err)) end
        local last = ticks >= ARRIVE_TICKS or #(pending[guid] or {}) == 0
        if last then ticking[guid] = nil end
        expire(p, ticks >= ARRIVE_TICKS)
    end, ARRIVE_TICK_MS, ARRIVE_TICKS)
end

------------------------------------------------------------------------------------------------
-- database: the usual party (alts) per account
------------------------------------------------------------------------------------------------
local function dbExec(sql)
    if not CharDBExecute then return false end
    return (pcall(CharDBExecute, sql))
end

local function dbRows(sql, cols)
    if not CharDBQuery then return {} end
    local ok, q = pcall(CharDBQuery, sql)
    if not ok or not q then return {} end
    local rows = {}
    repeat
        local row = {}
        for i, kind in ipairs(cols) do
            if kind == "n" then row[i] = tonumber(q:GetUInt32(i - 1)) else row[i] = q:GetString(i - 1) end
        end
        rows[#rows + 1] = row
    until not q:NextRow()
    return rows
end

dbExec([[CREATE TABLE IF NOT EXISTS mod_companions_usual (
  account INT UNSIGNED NOT NULL, slot TINYINT UNSIGNED NOT NULL, guid INT UNSIGNED NOT NULL,
  name VARCHAR(24) NOT NULL, saved_at INT UNSIGNED NOT NULL, PRIMARY KEY (account, slot)
)]])

-- Who owns which bot, so a crash or a script reload doesn't orphan them (the module logs them back in with
-- their master; the in-memory tables start empty).
dbExec([[CREATE TABLE IF NOT EXISTS mod_companions_owned (
  bot INT UNSIGNED NOT NULL, owner INT UNSIGNED NOT NULL, name VARCHAR(24) NOT NULL, role VARCHAR(8) NOT NULL,
  spec VARCHAR(32) NOT NULL, kind VARCHAR(6) NOT NULL, at INT UNSIGNED NOT NULL, PRIMARY KEY (bot), KEY (owner)
)]])

local ROLE_WORDS = { tank = true, healer = true, dps = true, alt = true, buddy = true }

-- Single player's choice per character: mode "new" (bots asked for, never arrived yet), "on" (they come back at
-- each login: party is what was with the player, "tank=warrior:prot pve,healer=priest:holy pve,buddy=mage") or
-- "off" (the player sent them home: nothing joins by itself until the player brings bots again).
dbExec([[CREATE TABLE IF NOT EXISTS mod_companions_auto (
  guid INT UNSIGNED NOT NULL, mode VARCHAR(4) NOT NULL, party VARCHAR(255) NOT NULL, at INT UNSIGNED NOT NULL,
  PRIMARY KEY (guid)
)]])

ownSave = function(owner, bg, c)
    local name = safeName(c.name)
    local role = ROLE_WORDS[c.role or ""] and c.role or "dps"
    local spec = tostring(c.spec or "")
    if not spec:match("^[%a ]*$") or #spec > 32 then spec = "" end
    -- auto: joined by itself (keeps to the owner's level); temp: a dungeon bot (goes home when the owner leaves)
    local kind = c.buddy and "buddy" or c.alt and "alt" or c.dungeon and "temp" or c.keep and "auto" or "bot"
    if name == "" then return end
    dbExec(string.format("REPLACE INTO mod_companions_owned (bot, owner, name, role, spec, kind, at) "
        .. "VALUES (%d, %d, '%s', '%s', '%s', '%s', %d)", bg, owner, name, role, spec, kind, os.time()))
end

ownForget = function(bg) dbExec(string.format("DELETE FROM mod_companions_owned WHERE bot = %d", bg)) end

local function ownForgetOwner(owner)
    dbExec(string.format("DELETE FROM mod_companions_owned WHERE owner = %d", owner))
end

local function ownRows(owner)
    local out = {}
    for _, r in ipairs(dbRows(string.format(
        "SELECT bot, name, role, spec, kind FROM mod_companions_owned WHERE owner = %d", owner), { "n", "s", "s", "s", "s" })) do
        if r[1] then out[r[1]] = { name = r[2], role = r[3], spec = r[4], kind = r[5] } end
    end
    return out
end

-- Takes back the bots in the player's group that are the player's: PlayerbotAI:GetMaster() is the player, or
-- (without GetBotAI) mod_companions_owned says so. Not while recruits are on the way (arrivals claims those).
-- Returns how many came back.
adopt = function(player)
    local guid = player:GetGUIDLow()
    if #(pending[guid] or {}) > 0 or not try(player, "GetGroup") then return 0 end
    local _, members = groupMembers(player)
    local rows, n = nil, 0
    for _, m in ipairs(members) do
        local bg = m:GetGUIDLow()
        if bg ~= guid and isBot(m) and not claimed(bg) then
            local mine
            local ai = botAI(m)
            local master = ai and try(ai, "GetMaster")
            if master then mine = try(master, "GetGUIDLow") == guid end
            rows = rows or ownRows(guid)
            local row = rows[bg]
            if mine == nil then mine = row ~= nil end
            if mine then
                local alt = try(m, "GetAccountId") == player:GetAccountId()
                local buddy = row ~= nil and row.kind == "buddy"
                local role = row and ROLE_WORDS[row.role or ""] and row.role or nil
                if not role then role = roleOf(player, m) end
                local spec = row and row.spec ~= "" and row.spec or nil
                companions[guid] = companions[guid] or {}
                local c = { name = m:GetName(), cls = m:GetClass(), role = role, spec = spec, buddy = buddy, alt = alt,
                    order = "follow", keep = (row ~= nil and row.kind == "auto") or nil,
                    dungeon = (row ~= nil and row.kind == "temp") or nil }
                companions[guid][bg] = c
                ownSave(guid, bg, c)
                n = n + 1
            end
        end
    end
    return n
end

-- The other characters on the player's account that are offline: { guid, name, level, class }.
local function accountAlts(player)
    local rows = dbRows(string.format(
        "SELECT guid, name, level, class FROM characters WHERE account = %d AND online = 0 AND guid <> %d "
        .. "ORDER BY level DESC, name LIMIT 20", player:GetAccountId(), player:GetGUIDLow()), { "n", "s", "n", "n" })
    local out = {}
    for _, r in ipairs(rows) do out[#out + 1] = { guid = r[1], name = r[2], level = r[3], cls = r[4] } end
    return out
end

-- One character of the player's own account, by name (the database compares names without case).
local function ownAlt(player, name)
    local rows = dbRows(string.format(
        "SELECT guid, name, level, class, online FROM characters WHERE account = %d AND name = '%s' LIMIT 1",
        player:GetAccountId(), name), { "n", "s", "n", "n", "n" })
    local r = rows[1]
    if not r then return nil end
    return { guid = r[1], name = r[2], level = r[3], cls = r[4], online = r[5] }
end

------------------------------------------------------------------------------------------------
-- commands
------------------------------------------------------------------------------------------------
local function comingSoon(player)
    say(player, "Bots are coming soon to this realm.")
end

-- Shared checks for every .recruit and .buddy. opts.raid: this command makes a raid; opts.size: the group
-- size it fills to; opts.auto: single player's own joins (no cooldown, nothing said: they try again later).
-- Returns how many may be recruited now (0 = refused, already said why) and the cap used.
local function room(player, want, opts)
    opts = opts or {}
    local function no(text) if not opts.auto then say(player, text) end; return 0 end
    if not onSwitch() then return no("Bots are off on this realm.") end
    if try(player, "IsDead") then return no("Not while dead.") end
    if try(player, "IsInCombat") then return no("Not in combat.") end
    if inPvp(player) then return no("Not in battlegrounds or arenas.") end
    if instanceKind(player) and groupInCombat(player) then
        return no("Wait until your group is out of combat.")
    end
    local group = try(player, "GetGroup")
    if group and try(group, "IsLeader", player:GetGUID()) == false then
        return no("Only the group leader can recruit.")
    end
    local guid, now = player:GetGUIDLow(), os.time()
    local cd = cooldown()
    if not opts.auto and lastRecruit[guid] and now - lastRecruit[guid] < cd then
        return no(string.format("You can recruit again in %d s.", cd - (now - lastRecruit[guid])))
    end
    prune(player)
    local raiding = opts.raid or isRaidGroup(player)
    local cap = raiding and perPlayerRaid() or perPlayer()
    local size = opts.size or (raiding and groupMax() or math.min(PARTY, groupMax()))
    local have = mine(guid)
    local slots = size - groupSize(player) - #(pending[guid] or {})
    local realmLeft = realmMax() - realmCount()
    local n = math.min(want, cap - have, slots, realmLeft)
    if n <= 0 then
        if opts.auto then return 0 end
        if cap - have <= 0 then
            say(player, string.format("You have %s, the most allowed %s (.dismiss to make room).",
                plural(have, "bot"), raiding and "in a raid" or "in a party"))
        elseif slots <= 0 then
            say(player, string.format("Your group is full (%d players).", size))
        else
            say(player, "Every bot on the realm is out with someone right now. Try again in a few minutes.")
        end
        return 0
    end
    return n, raiding
end

local function queue(player, list, auto)
    local guid = player:GetGUIDLow()
    pending[guid] = pending[guid] or {}
    if not auto then lastRecruit[guid] = os.time() end
    for _, p in ipairs(list) do
        p.at = os.time()
        table.insert(pending[guid], p)
        if p.alt then botCommand(player, "add " .. p.name) else botCommand(player, "addclass " .. p.word) end
    end
    watchArrivals(player)
end

-- The class picks for a role at the player's level (no Death Knight below its start level).
local function picksFor(player, roleWord, onlyClass)
    local picks = {}
    for _, pk in ipairs(ROLES[roleWord].picks) do
        if (not onlyClass or pk[1] == onlyClass) and (pk[1] ~= "dk" or dkOk(player)) then
            picks[#picks + 1] = pk
        end
    end
    return picks
end

-- wanted: the window's picks for this role, in slot order ({ word = "mage" | nil, spec = "fire pve" | nil }), used
-- first; the rest are the usual picks, round the list from a random start.
local function addRole(player, list, roleWord, n, onlyClass, wanted)
    local picks = picksFor(player, roleWord, onlyClass)
    if #picks == 0 or n <= 0 then return end
    local start = math.random(#picks)
    local w = 1
    for i = 1, n do
        local pk = picks[(start + i - 2) % #picks + 1]
        local word, spec = pk[1], pk[2]
        local want = wanted and wanted[w]
        w = w + 1
        if want and want.word then
            for _, p in ipairs(picks) do
                if p[1] == want.word then word, spec = p[1], p[2] end
            end
            if word == want.word and want.spec then spec = want.spec end
        end
        list[#list + 1] = { cls = CLASS[word], word = word, spec = spec, role = roleWord }
    end
end

-- The window's per-slot picks: "tank=warrior:prot pve,healer=any,dps=mage" -> { tank = { { word, spec } }, ... }.
-- A class that can't take the role, or a spec that isn't one of the realm's for that class and role, is dropped
-- (that slot gets the usual pick). At most 25 picks.
local function parsePicks(text)
    local out = { tank = {}, healer = {}, dps = {} }
    local n = 0
    for item in tostring(text or ""):lower():gmatch("[^,]+") do
        n = n + 1
        if n > 25 then break end
        local role, cls, spec = item:match("^%s*(%a+)=([%a%-]+):?([%a ]*)%s*$")
        role = ROLE_ALIAS[role or ""] or role
        if ROLES[role or ""] then
            local word = cls ~= "any" and classOf(cls or "") or nil
            local fits = false
            if word then for _, pk in ipairs(ROLES[role].picks) do if pk[1] == word then fits = true end end end
            if not fits then word = nil end
            local ok
            if word and spec ~= "" then
                for _, sp in ipairs(specsFor(CLASS[word])) do
                    if sp == spec and specRole(sp) == role then ok = sp end
                end
            end
            table.insert(out[role], { word = word, spec = ok })
        end
    end
    return out
end

-- Before a raid fill: a party group becomes a raid now when it will pass 5 (without a group yet, the
-- module converts it itself when the 6th member joins).
local function goRaid(player, total)
    raidWanted[player:GetGUIDLow()] = true
    local group = try(player, "GetGroup")
    if group and total > PARTY and try(group, "IsRaidGroup") == false then try(group, "ConvertToRaid") end
end

local function roleList(counts)
    local parts = {}
    if counts.tank > 0 then parts[#parts + 1] = plural(counts.tank, "tank") end
    if counts.healer > 0 then parts[#parts + 1] = plural(counts.healer, "healer") end
    if counts.dps > 0 then parts[#parts + 1] = counts.dps .. " damage" end
    return table.concat(parts, ", ")
end

local function fillCommand(player, kind, arg, picks)
    local size, raid = PARTY, false
    if kind == "raid" then
        if groupMax() <= PARTY then return say(player, "Raids with bots are off on this realm.") end
        size, raid = math.min(10, groupMax()), true
        if arg == "25" then
            if not raid25() then return say(player, "25-player raids are off here. .recruit raid fills a 10-player raid.") end
            size = math.min(25, groupMax())
        elseif arg and arg ~= "10" then
            return say(player, "Usage: |cffffff00.recruit raid|r (a 10-player raid)")
        end
        if instanceKind(player) == "dungeon" then
            return say(player, "Inside a 5-player dungeon, use .recruit party.")
        end
        local group = try(player, "GetGroup")
        if group and (try(group, "IsLFGGroup") == true or try(group, "IsBGGroup") == true) then
            return say(player, "Not in a Dungeon Finder group. Leave it first.")
        end
    elseif isRaidGroup(player) then
        return say(player, "You are in a raid. Use .recruit raid.")
    end
    local n = room(player, size, { raid = raid, size = size })
    if n == 0 then return end
    local counts = planRoles(rolesInGroup(player), SHAPE[size] or SHAPE[10], n)
    local list = {}
    for _, r in ipairs(ROLE_ORDER) do addRole(player, list, r, counts[r], nil, picks and picks[r]) end
    if #list == 0 then return say(player, "Your group already has what it needs.") end
    if raid then goRaid(player, groupSize(player) + #(pending[player:GetGUIDLow()] or {}) + #list) end
    queue(player, list)
    local msg = string.format("Recruiting %s for a %d-player %s: %s. They join in a few seconds.",
        plural(#list, "bot"), size, raid and "raid" or "group", roleList(counts))
    local free = size - groupSize(player) - #(pending[player:GetGUIDLow()] or {})
    if free > 0 then msg = msg .. " (The limits here leave " .. plural(free, "slot") .. " open.)" end
    say(player, msg)
end

local function roleCommand(player, roleWord, extra)
    roleWord = ROLE_ALIAS[roleWord or ""] or roleWord
    local role = ROLES[roleWord or ""]
    if not role then
        return say(player, "Usage: |cffffff00.recruit party|r, |cffffff00.recruit raid|r, "
            .. "|cffffff00.recruit tank|healer|dps [n]|r or |cffffff00.recruit alt <name>|r")
    end
    local want, onlyClass = 1, nil
    if extra then
        if tonumber(extra) then
            want = math.floor(tonumber(extra))
        else
            onlyClass = classOf(extra)
            local fits = false
            for _, pk in ipairs(role.picks) do if pk[1] == onlyClass then fits = true end end
            if not fits then
                local names = {}
                for _, pk in ipairs(role.picks) do names[#names + 1] = pk[1] end
                return say(player, "A " .. role.label .. " bot can be: " .. table.concat(names, ", ") .. ".")
            end
        end
    end
    if want < 1 then return say(player, "Recruit at least one.") end
    if #picksFor(player, roleWord, onlyClass) == 0 then return say(player, dkRefusal()) end
    local n = room(player, want)
    if n == 0 then return end
    local list = {}
    addRole(player, list, roleWord, n, onlyClass)
    queue(player, list)
    local msg = string.format("Recruiting %s: they join your group in a few seconds.", plural(n, role.label .. " bot"))
    if n < want then msg = msg .. string.format(" (%d of the %d asked: the limits here.)", n, want) end
    say(player, msg)
end

local function alreadyComing(guid, charGuid)
    if (companions[guid] or {})[charGuid] then return true end
    for _, p in ipairs(pending[guid] or {}) do if p.alt and p.guid == charGuid then return true end end
    return false
end

local function altsCommand(player)
    local alts = accountAlts(player)
    if #alts == 0 then return say(player, "No other characters on your account to bring.") end
    local names = {}
    for _, a in ipairs(alts) do
        names[#names + 1] = string.format("%s (%d %s)", a.name, a.level or 0, CLASS_LABEL[a.cls] or "?")
    end
    say(player, "Your other characters: " .. table.concat(names, ", "))
    say(player, "Bring one as a bot: |cffffff00.recruit alt <name>|r")
end

-- Queues alts (rows from ownAlt / the usual list) within the limits; returns the names queued.
local function queueAlts(player, alts, label)
    local raiding = isRaidGroup(player)
    local total = groupSize(player) + #(pending[player:GetGUIDLow()] or {}) + #alts
    if not raiding and total > PARTY and groupMax() > PARTY then raiding = true end
    local n = room(player, #alts, { raid = raiding })
    if n == 0 then return {} end
    local list, names = {}, {}
    for i = 1, n do
        local a = alts[i]
        list[#list + 1] = { alt = true, guid = a.guid, name = a.name, cls = a.cls, role = "alt" }
        names[#names + 1] = a.name
    end
    if raiding then goRaid(player, groupSize(player) + #(pending[player:GetGUIDLow()] or {}) + #list) end
    queue(player, list)
    local msg = label .. table.concat(names, ", ") .. ". They join in a few seconds."
    if n < #alts then msg = msg .. string.format(" (%d of %d: the limits here.)", n, #alts) end
    say(player, msg)
    return names
end

local function altCommand(player, rawName)
    if not rawName then return altsCommand(player) end
    local name = safeName(rawName)
    if name == "" then return say(player, "Usage: |cffffff00.recruit alt <name>|r (.companions alts lists them)") end
    local a = ownAlt(player, name)
    if not a then return say(player, "You have no character called " .. name .. " on this account.") end
    if a.guid == player:GetGUIDLow() then return say(player, "That's you.") end
    if alreadyComing(player:GetGUIDLow(), a.guid) then return say(player, a.name .. " is already with you.") end
    if a.online ~= 0 then return say(player, a.name .. " is online already.") end
    queueAlts(player, { a }, "Bringing ")
end

local function saveCommand(player)
    local guid, account = player:GetGUIDLow(), player:GetAccountId()
    prune(player)
    local alts = {}
    for bg, c in pairs(companions[guid] or {}) do
        if c.alt and safeName(c.name) ~= "" then alts[#alts + 1] = { guid = bg, name = c.name } end
    end
    if #alts == 0 then
        return say(player, "No alts with you to save. Bring them first: .recruit alt <name>")
    end
    table.sort(alts, function(x, y) return x.name < y.name end)
    dbExec(string.format("DELETE FROM mod_companions_usual WHERE account = %d", account))
    local names = {}
    for i, a in ipairs(alts) do
        dbExec(string.format("INSERT INTO mod_companions_usual (account, slot, guid, name, saved_at) VALUES (%d, %d, %d, '%s', %d)",
            account, i, a.guid, a.name, os.time()))
        names[#names + 1] = a.name
    end
    say(player, "Saved your usual party: " .. table.concat(names, ", ") .. ". |cffffff00.recruit usual|r brings them back.")
end

local function usualCommand(player)
    local account = player:GetAccountId()
    local rows = dbRows(string.format(
        "SELECT c.guid, c.name, c.level, c.class, c.online FROM mod_companions_usual u JOIN characters c "
        .. "ON c.guid = u.guid AND c.account = u.account WHERE u.account = %d ORDER BY u.slot", account),
        { "n", "s", "n", "n", "n" })
    if #rows == 0 then
        return say(player, "No usual party saved. Bring alts with .recruit alt <name>, then .companions save.")
    end
    local guid, ready, busy = player:GetGUIDLow(), {}, {}
    for _, r in ipairs(rows) do
        local a = { guid = r[1], name = r[2], level = r[3], cls = r[4], online = r[5] }
        if a.guid ~= guid and not alreadyComing(guid, a.guid) then
            if a.online == 0 then ready[#ready + 1] = a else busy[#busy + 1] = a.name end
        end
    end
    if #ready == 0 then
        if #busy > 0 then return say(player, table.concat(busy, ", ") .. " are online already.") end
        return say(player, "Your usual party is already with you.")
    end
    queueAlts(player, ready, "Bringing your usual party: ")
    if #busy > 0 then say(player, "Online already: " .. table.concat(busy, ", ") .. ".") end
end

local function buddyCommand(player, arg)
    if not buddyOn() then return say(player, "The levelling buddy is off on this realm. Try .recruit.") end
    local guid = player:GetGUIDLow()
    prune(player)
    local bg, b = buddyOf(guid)
    if arg == "off" or arg == "dismiss" or arg == "bye" then
        if not bg then return say(player, "You have no buddy with you.") end
        local _, members = groupMembers(player)
        for _, m in ipairs(members) do
            if m:GetGUIDLow() == bg then tell(player, m, "leave"); break end
        end
        local nm = safeName(b.name)
        if nm ~= "" then
            player:RegisterEvent(function(_, _, _, p) if p then botCommand(p, "remove " .. nm) end end, 2500, 1)
        end
        companions[guid][bg] = nil
        ownForget(bg)
        say(player, b.name .. " heads home.")
        return autoDismissed(player)
    end
    if b then
        if not bg then return say(player, "Your buddy is on the way.") end
        return say(player, string.format("Your buddy is %s the %s, a bot that keeps to your level. (.buddy off to send it home)",
            b.name, CLASS_LABEL[b.cls] or "bot"))
    end
    local word
    if arg then
        word = classOf(arg)
        if not word then return say(player, "Usage: |cffffff00.buddy [class]|r  (warrior, paladin, hunter, rogue, priest, "
            .. "shaman, mage, warlock, druid, dk)") end
        if word == "dk" and not dkOk(player) then
            return say(player, dkRefusal())
        end
    else
        local all = {}
        for _, w in ipairs(BUDDY_CLASSES) do
            if w ~= "dk" or dkOk(player) then all[#all + 1] = w end
        end
        word = all[math.random(#all)]
    end
    if room(player, 1) == 0 then return end
    queue(player, { { cls = CLASS[word], word = word, role = "buddy", buddy = true } })
    say(player, "Your levelling buddy bot is on the way.")
end

local function dismissCommand(player, arg)
    local guid = player:GetGUIDLow()
    prune(player)
    local list = companions[guid] or {}
    local n = countOf(list)
    if n == 0 then
        pending[guid] = nil
        raidWanted[guid] = nil
        return say(player, "You have no bots with you.")
    end
    local gone = {}
    -- Each bot leaves the group itself first (its own "leave" command, the module's LeaveOrDisbandGroup), then
    -- logs out 2.5 s later. A bot that only logs out stays behind as an offline member and fills the slot
    -- ("Your group is full"). Never remove members from the group in Lua: on 2026-10-26 calling the group's
    -- RemoveMember next to the module's own logout clean-up crashed the world server.
    local _, members = groupMembers(player)
    local byGuid = {}
    for _, m in ipairs(members) do byGuid[m:GetGUIDLow()] = m end
    local later = {}
    local function send(bg, c)
        gone[#gone + 1] = c.name; list[bg] = nil; ownForget(bg)
        if byGuid[bg] then tell(player, byGuid[bg], "leave") end
        local nm = safeName(c.name)
        if nm ~= "" then later[#later + 1] = nm end
    end
    if arg == "all" then
        for bg, c in pairs(list) do send(bg, c) end
        pending[guid] = nil
        raidWanted[guid] = nil
    elseif arg == "buddy" then
        return buddyCommand(player, "off")
    elseif arg or n == 1 then
        for bg, c in pairs(list) do
            if not arg or c.name:lower() == arg then send(bg, c); break end
        end
        if #gone == 0 then return say(player, "None of your bots is called " .. arg .. ".") end
    else
        local names = {}
        for _, c in pairs(list) do names[#names + 1] = c.name end
        table.sort(names)
        return say(player, "Dismiss which? |cffffff00.dismiss <name>|r or |cffffff00.dismiss all|r: " .. table.concat(names, ", "))
    end
    if #later > 0 then
        player:RegisterEvent(function(_, _, _, p)
            if not p then return end
            for _, nm in ipairs(later) do botCommand(p, "remove " .. nm) end
            pushRoster(p)
        end, 2500, 1)
    end
    table.sort(gone)
    say(player, table.concat(gone, ", ") .. (#gone == 1 and " heads" or " head") .. " home.")
    autoDismissed(player)
end

local function showHelp(player)
    say(player, "Bots: party members who fight, heal and follow you. They always pass on loot.")
    say(player, "  |cffffff00.recruit party|r  fill a 5-player group (1 tank, 1 healer, damage)")
    if groupMax() > PARTY then
        say(player, "  |cffffff00.recruit raid|r  fill a 10-player raid (2 tanks, 3 healers, 5 damage)")
    end
    say(player, "  |cffffff00.recruit tank|healer|dps [n]|r  add bots of one role (.recruit dps mage)")
    say(player, "  |cffffff00.recruit alt <name>|r  bring one of your own characters (|cffffff00.companions alts|r lists them)")
    say(player, "  |cffffff00.companions save|r  remember the alts with you; |cffffff00.recruit usual|r brings them back")
    if buddyOn() then say(player, "  |cffffff00.buddy [class]|r  a levelling buddy that keeps to your level") end
    say(player, "  |cffffff00.dismiss [name|all]|r  send bots home")
    say(player, string.format("  Limits: %s in a party, %s in a raid, .recruit every %d s.",
        plural(perPlayer(), "bot"), plural(perPlayerRaid(), "bot"), cooldown()))
    say(player, "  In party chat: follow, stay, attack, summon.")
    prune(player)
    local names = {}
    for _, c in pairs(companions[player:GetGUIDLow()] or {}) do
        names[#names + 1] = c.name .. " (" .. (c.buddy and "buddy" or c.alt and "alt" or (ROLES[c.role] and ROLES[c.role].label or c.role)) .. ")"
    end
    table.sort(names)
    if #names > 0 then say(player, "  With you: " .. table.concat(names, ", ")) end
    if OVMod then say(player, "  The Bots window: |cffffff00/comp|r") end
end

------------------------------------------------------------------------------------------------
-- the Bots window (WrathUnleashedUI/Companions.lua) over OVMOD, mod "wucomp"
------------------------------------------------------------------------------------------------
local FORMATIONS = { near = true, melee = true, line = true, circle = true, arrow = true, shield = true, queue = true,
    chaos = true }
local PREPARE_COOLDOWN = 60   -- seconds between two "Prepare for raid"
local BOT_COOLDOWN = 10       -- seconds between two spec / role / gear changes of one bot
local PER_SECOND = 4          -- window requests answered per player per second
local ILVL_CACHE = 15         -- seconds an item level is reused
local GEAR_SLOTS = { 0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17 }   -- not the shirt (3) or tabard (18)

local rate, lastPrepare, botBusy, ilvlCache = {}, {}, {}, {}

local function clean(s) return (tostring(s or ""):gsub("[\t\n|]", "")) end

local function flooding(guid)
    local now = os.time()
    local r = rate[guid]
    if not r or r.at ~= now then rate[guid] = { at = now, n = 1 }; return false end
    r.n = r.n + 1
    return r.n > PER_SECOND
end

-- Average item level of what the bot wears (cached a few seconds; 17 slots per bot).
local function itemLevel(m)
    local bg, now = m:GetGUIDLow(), os.time()
    local hit = ilvlCache[bg]
    if hit and now - hit.at < ILVL_CACHE then return hit.v end
    local sum, n = 0, 0
    for _, slot in ipairs(GEAR_SLOTS) do
        local it = try(m, "GetEquippedItemBySlot", slot)
        local lv = it and tonumber(try(it, "GetItemLevel"))
        if lv and lv > 0 then sum = sum + lv; n = n + 1 end
    end
    local v = n > 0 and math.floor(sum / n + 0.5) or 0
    ilvlCache[bg] = { v = v, at = now }
    return v
end

local function statusOf(m, c)
    if try(m, "IsDead") then return "dead" end
    if c.passive then return "passive" end
    if c.order == "stay" then return "stay" end
    return "follow"
end

-- The player's bots that are in the group now, tanks first, then healers, then damage, then by name.
local RANK = { tank = 1, healer = 2, dps = 3 }
local function myBots(player)
    prune(player)
    local _, members = groupMembers(player)
    local byGuid = {}
    for _, m in ipairs(members) do byGuid[m:GetGUIDLow()] = m end
    local out = {}
    for bg, c in pairs(companions[player:GetGUIDLow()] or {}) do
        if byGuid[bg] then out[#out + 1] = { bg = bg, c = c, m = byGuid[bg], role = roleOf(player, byGuid[bg]) } end
    end
    table.sort(out, function(a, b)
        local ra, rb = RANK[a.role] or 4, RANK[b.role] or 4
        if ra ~= rb then return ra < rb end
        return a.c.name < b.c.name
    end)
    return out
end

local function findBot(player, name)
    name = tostring(name or ""):lower()
    if name == "" then return nil end
    for _, e in ipairs(myBots(player)) do if e.c.name:lower() == name then return e end end
    return nil
end

pushRoster = function(player)
    if not OVMod or not player then return end
    local guid = player:GetGUIDLow()
    local state = (not botsBuilt(player)) and "soon" or (not onSwitch()) and "off" or "on"
    local list = state == "on" and myBots(player) or {}
    local o = orders[guid] or {}
    local raiding = isRaidGroup(player)
    local lines = { table.concat({ "H", #list, raiding and perPlayerRaid() or perPlayer(), raiding and 1 or 0,
        o.formation or "chaos", o.assist and 1 or 0, o.cc and 1 or 0, state, buddyOn() and 1 or 0,
        groupMax() > PARTY and 1 or 0, o.waitpull and 1 or 0, o.healtank and 1 or 0, o.savemana and 1 or 0,
        o.aoe and 1 or 0 }, "\t") }
    local classes, seen = {}, {}
    for _, e in ipairs(list) do
        local c, m = e.c, e.m
        local cls = c.cls or m:GetClass()
        lines[#lines + 1] = table.concat({ "B", clean(c.name), cls, e.role,
            c.buddy and "buddy" or c.alt and "alt" or c.dungeon and "dungeon" or "bot",
            clean(c.spec or ""), itemLevel(m), statusOf(m, c), m:GetLevel() }, "\t")
        if not seen[cls] then seen[cls] = true; classes[#classes + 1] = cls end
    end
    table.sort(classes)
    for _, cls in ipairs(classes) do
        local parts = {}
        for _, s in ipairs(specsFor(cls)) do parts[#parts + 1] = s .. "=" .. specRole(s) end
        lines[#lines + 1] = "S\t" .. cls .. "\t" .. table.concat(parts, ",")
    end
    OVMod.Send(player, MOD, "roster", table.concat(lines, "\n"))
end

local function later(player, ms, fn)
    player:RegisterEvent(function(_, _, _, p) if p then fn(p) end end, ms, 1)
end

-- Every window action: answers go to the window from now on; bots on this realm, and on.
local function windowReady(player)
    if isBot(player) then return false end
    hasWindow[player:GetGUIDLow()] = true
    if flooding(player:GetGUIDLow()) then return false end
    replyTo[player:GetGUIDLow()] = true
    if not botsBuilt(player) then comingSoon(player); return false end
    if not onSwitch() then say(player, "Bots are off on this realm."); return false end
    return true
end

-- Orders that teach, re-gear or re-spec wait for a quiet moment.
local function calm(player)
    if try(player, "IsDead") then say(player, "Not while dead."); return false end
    if try(player, "IsInCombat") or groupInCombat(player) then say(player, "Wait until your group is out of combat."); return false end
    return true
end

local function busy(player, e)
    local now = os.time()
    if botBusy[e.bg] and now - botBusy[e.bg] < BOT_COOLDOWN then
        say(player, string.format("%s is still busy. Try again in %d s.", e.c.name, BOT_COOLDOWN - (now - botBusy[e.bg])))
        return true
    end
    botBusy[e.bg] = now
    return false
end

-- order <what> <name|all>
local ORDERS = {
    follow  = { said = "Following you.", fn = function(p, e) tell(p, e.m, "follow"); e.c.order = "follow"; e.c.passive = nil end },
    stay    = { said = "Staying here.", fn = function(p, e) tell(p, e.m, "stay"); e.c.order = "stay"; e.c.passive = nil end },
    attack  = { said = "Attacking your target.", fn = function(p, e) tell(p, e.m, "attack") end },
    passive = { said = "Passive: they won't attack.", fn = function(p, e) strategy(p, e.m, "+passive"); e.c.passive = true end },
    active  = { said = "They fight again.", fn = function(p, e) strategy(p, e.m, "-passive"); e.c.passive = nil end },
    -- Tanks mark their target with the skull; damage focuses one target at a time (dps assist, not dps aoe) and
    -- the skull first (mod-playerbots' DpsTargetValue takes the RTI target first).
    assist  = { said = "Damage bots focus the tank's skull target.", group = "assist", on = true, fn = function(p, e)
        if e.role == "tank" then strategy(p, e.m, "+mark rti")
        elseif e.role == "dps" then strategy(p, e.m, "+dps assist"); tell(p, e.m, "rti skull") end
    end },
    noassist = { said = "Assist tank off.", group = "assist", on = false, fn = function(p, e)
        if e.role == "tank" then strategy(p, e.m, "-mark rti") end
    end },
    cc      = { said = "Crowd control on: bots that can will keep extra enemies busy.", group = "cc", on = true,
        fn = function(p, e) strategy(p, e.m, "+cc") end },
    nocc    = { said = "Crowd control off.", group = "cc", on = false, fn = function(p, e) strategy(p, e.m, "-cc") end },
    -- back to the owner: follow, and a bot that is far away (or elsewhere) is fetched with mod-playerbots' summon
    regroup = { said = "Regrouping on you.", fn = function(p, e)
        tell(p, e.m, "follow"); e.c.order = "follow"
        local sameMap = try(e.m, "GetMapId") == try(p, "GetMapId")
        local d = tonumber(try(e.m, "GetDistance", p))
        if not sameMap or (d ~= nil and d > TELEPORT_DISTANCE) then tell(p, e.m, "summon") end
    end },
    -- mod-playerbots' own flee: it stops, follows and turns passive (Fight brings it back)
    flee    = { said = "Running with you: they stop fighting. Press Fight when you are safe.", fn = function(p, e)
        tell(p, e.m, "flee"); e.c.order = "follow"; e.c.passive = true
    end },
    drink   = { said = "Bots with mana drink up.", fn = function(p, e) tell(p, e.m, "drink") end },
    -- tactics (applyTactics gives the group's ones to bots that join later)
    waitpull   = { said = "Damage bots wait for the tank to hold the first enemy, and watch their threat.",
        group = "waitpull", on = true, fn = function(p, e) applyTactics(p, e.m, e.role, "waitpull", true) end },
    nowaitpull = { said = "Damage bots open the fight with the tank.", group = "waitpull", on = false,
        fn = function(p, e) applyTactics(p, e.m, e.role, "waitpull", false) end },
    healtank   = { said = "Healers heal the tank first.", group = "healtank", on = true,
        fn = function(p, e) applyTactics(p, e.m, e.role, "healtank", true) end },
    nohealtank = { said = "Healers heal whoever needs it most.", group = "healtank", on = false,
        fn = function(p, e) applyTactics(p, e.m, e.role, "healtank", false) end },
    savemana   = { said = "Healers save mana between fights.", group = "savemana", on = true,
        fn = function(p, e) applyTactics(p, e.m, e.role, "savemana", true) end },
    nosavemana = { said = "Healers spend mana freely.", group = "savemana", on = false,
        fn = function(p, e) applyTactics(p, e.m, e.role, "savemana", false) end },
    aoe        = { said = "Damage bots spread their damage over many enemies.", group = "aoe", on = true,
        fn = function(p, e) applyTactics(p, e.m, e.role, "aoe", true) end },
    noaoe      = { said = "Damage bots focus one enemy at a time.", group = "aoe", on = false,
        fn = function(p, e) applyTactics(p, e.m, e.role, "aoe", false) end },
    -- the tank bots open the fight on the owner's target, the others join in as they assist; no tank bot: everyone
    pull    = { said = "Pulling your target.", target = true, fn = function(p, e) tell(p, e.m, "attack") end,
        pick = function(list)
            local tanks = {}
            for _, e in ipairs(list) do if e.role == "tank" then tanks[#tanks + 1] = e end end
            return #tanks > 0 and tanks or list
        end },
}

local function onOrder(player, data)
    if not windowReady(player) then return end
    local what, target = tostring(data or ""):lower():match("^(%a+) (%S+)$")
    local o = ORDERS[what or ""]
    if not o then return say(player, "That order isn't known.") end
    if (what == "attack" or o.target) and not try(player, "GetSelection") then return say(player, "Target something first.") end
    local list = myBots(player)
    if #list == 0 then return say(player, "You have no bots with you.") end
    if o.pick and target == "all" then list = o.pick(list) end
    local done
    for _, e in ipairs(list) do
        if target == "all" or e.c.name:lower() == target then o.fn(player, e); done = done or e.c.name end
    end
    if not done then return say(player, "None of your bots is called " .. clean(target) .. ".") end
    if o.group and target == "all" then
        local guid = player:GetGUIDLow()
        orders[guid] = orders[guid] or {}
        orders[guid][o.group] = o.on
    end
    say(player, target == "all" and o.said or (done .. ": " .. o.said))
    pushRoster(player)
end

local function onFormation(player, data)
    if not windowReady(player) then return end
    local f = tostring(data or ""):lower():match("^(%a+)$")
    if not FORMATIONS[f or ""] then return say(player, "That formation isn't known.") end
    local list = myBots(player)
    if #list == 0 then return say(player, "You have no bots with you.") end
    for _, e in ipairs(list) do tell(player, e.m, "formation " .. f) end
    local guid = player:GetGUIDLow()
    orders[guid] = orders[guid] or {}
    orders[guid].formation = f
    say(player, "Formation: " .. f .. ".")
    pushRoster(player)
end

-- Maintenance (spells, talents, consumables, repairs) and autogear for recruited bots; alts keep their own gear;
-- everyone rebuffs.
local function onPrepare(player)
    if not windowReady(player) or not calm(player) then return end
    local guid, now = player:GetGUIDLow(), os.time()
    if lastPrepare[guid] and now - lastPrepare[guid] < PREPARE_COOLDOWN then
        return say(player, string.format("They are getting ready. Again in %d s.", PREPARE_COOLDOWN - (now - lastPrepare[guid])))
    end
    local list = myBots(player)
    if #list == 0 then return say(player, "You have no bots with you.") end
    lastPrepare[guid] = now
    for _, e in ipairs(list) do
        if not e.c.alt then
            tell(player, e.m, "maintenance")
            tell(player, e.m, "autogear")
            ilvlCache[e.bg] = nil
        end
        tell(player, e.m, "rebuff")
    end
    say(player, "Getting ready: spells, gear and buffs. Give them a few seconds.")
    later(player, 5000, function(p) for _, e in ipairs(list) do ilvlCache[e.bg] = nil end; pushRoster(p) end)
end

-- A premade spec (one of the realm's for that class), then maintenance and autogear for the new talents.
local function learnSpec(player, e, spec)
    tell(player, e.m, "talents spec " .. spec)
    tell(player, e.m, "maintenance")
    tell(player, e.m, "autogear")
    e.c.spec = spec
    if not e.c.buddy then e.c.role = specRole(spec) end
    ownSave(player:GetGUIDLow(), e.bg, e.c)
    ilvlCache[e.bg] = nil
    pushRoster(player)
    later(player, 4000, function(p) ilvlCache[e.bg] = nil; pushRoster(p) end)   -- the new gear's item level
end

local function specLabel(spec) return (spec:gsub(" pve$", "")) end

local function changeable(player, e)
    if not e then say(player, "That bot isn't with you."); return false end
    if e.c.alt then say(player, e.c.name .. " is your own character: it keeps its own talents and gear."); return false end
    return true
end

local function onSpec(player, data)
    if not windowReady(player) then return end
    local name, spec = tostring(data or ""):match("^(%S+)\t([%a ]+)$")
    local e = findBot(player, name)
    if not changeable(player, e) then return end
    local fits = false
    for _, s in ipairs(specsFor(e.c.cls)) do if s == spec then fits = true end end
    if not fits then return say(player, "That spec isn't one " .. e.c.name .. " can learn.") end
    if not calm(player) or busy(player, e) then return end
    learnSpec(player, e, spec)
    say(player, string.format("%s learns %s.", e.c.name, specLabel(spec)))
end

local function onRole(player, data)
    if not windowReady(player) then return end
    local name, role = tostring(data or ""):lower():match("^(%S+) (%a+)$")
    role = ROLE_ALIAS[role or ""] or role
    if not ROLES[role or ""] then return say(player, "Pick tank, healer or damage.") end
    local e = findBot(player, name)
    if not changeable(player, e) then return end
    -- the class's usual pick for the role (ROLES), else any of the realm's specs for that role
    local word, spec
    for w, id in pairs(CLASS) do if id == e.c.cls then word = w end end
    local have = {}
    for _, s in ipairs(specsFor(e.c.cls)) do have[s] = specRole(s) == role end
    for _, pk in ipairs(ROLES[role].picks) do if pk[1] == word and have[pk[2]] then spec = pk[2] end end
    if not spec then for _, s in ipairs(specsFor(e.c.cls)) do if not spec and have[s] then spec = s end end end
    if not spec then
        return say(player, string.format("A %s can't be a %s.", CLASS_LABEL[e.c.cls] or "bot", ROLES[role].label))
    end
    if not calm(player) or busy(player, e) then return end
    learnSpec(player, e, spec)
    say(player, string.format("%s is now a %s (%s).", e.c.name, ROLES[role].label, specLabel(spec)))
end

local function onGear(player, data)
    if not windowReady(player) then return end
    local e = findBot(player, tostring(data or ""):match("^(%S+)$"))
    if not changeable(player, e) or not calm(player) or busy(player, e) then return end
    tell(player, e.m, "autogear")
    ilvlCache[e.bg] = nil
    say(player, e.c.name .. " is picking better gear.")
    later(player, 4000, function(p) ilvlCache[e.bg] = nil; pushRoster(p) end)
end

-- The tactics (the window's Tactics tab), by role, through mod-playerbots' own strategies and commands:
--   waitpull  damage: +wait for attack (waits after the tank engages), +threat (stops near the tank's threat), and
--             "wait for attack time 3"; off: -wait for attack, -threat
--   healtank  healers: "focus heal <tank>" (the first tank bot, else the owner, who then tanks); off: "focus heal none"
--   savemana  healers: +save mana / -save mana
--   aoe       damage: +dps aoe (spread); off: +dps assist (focus; the two replace each other)
-- With no `what`: a bot that just joined gets every tactic its owner has on.
local function groupTank(owner)
    for _, e in ipairs(myBots(owner)) do if e.role == "tank" then return e.c.name end end
    return safeName(owner:GetName())
end
applyTactics = function(owner, m, role, what, on)
    if not what then
        local o = orders[owner:GetGUIDLow()] or {}
        for _, k in ipairs({ "waitpull", "healtank", "savemana", "aoe" }) do
            if o[k] then applyTactics(owner, m, role, k, true) end
        end
        return
    end
    if what == "waitpull" and role == "dps" then
        strategy(owner, m, (on and "+" or "-") .. "wait for attack")
        strategy(owner, m, (on and "+" or "-") .. "threat")
        if on then tell(owner, m, "wait for attack time 3") end
    elseif what == "healtank" and role == "healer" then
        local tank = on and groupTank(owner) or ""
        tell(owner, m, "focus heal " .. ((on and tank ~= "") and tank:lower() or "none"))
    elseif what == "savemana" and role == "healer" then
        strategy(owner, m, (on and "+" or "-") .. "save mana")
    elseif what == "aoe" and role == "dps" then
        strategy(owner, m, on and "+dps aoe" or "+dps assist")
    end
end

-- The gear window (inspect, equip, upgrade). What each inventory type can go in (equipment slots 0-17).
local INV_SLOTS = { [1] = { 0 }, [2] = { 1 }, [3] = { 2 }, [5] = { 4 }, [20] = { 4 }, [6] = { 5 }, [7] = { 6 }, [8] = { 7 },
    [9] = { 8 }, [10] = { 9 }, [11] = { 10, 11 }, [12] = { 12, 13 }, [16] = { 14 }, [13] = { 15, 16 }, [17] = { 15 },
    [21] = { 15 }, [14] = { 16 }, [22] = { 16 }, [23] = { 16 }, [15] = { 17 }, [25] = { 17 }, [26] = { 17 }, [28] = { 17 } }
local BACKPACK, BAG_SLOTS = 255, { 19, 20, 21, 22 }

-- Every item in the bot's backpack and bags (Player:GetItemByPos: bag 255 slots 23-38, bags 19-22 slots 0-35).
local function bagItems(m)
    local out = {}
    for slot = 23, 38 do
        local it = try(m, "GetItemByPos", BACKPACK, slot)
        if it then out[#out + 1] = it end
    end
    for _, bag in ipairs(BAG_SLOTS) do
        local b = try(m, "GetItemByPos", BACKPACK, bag)
        local size = b and tonumber(try(b, "GetBagSize")) or 0
        for slot = 0, math.min(size, 36) - 1 do
            local it = try(m, "GetItemByPos", bag, slot)
            if it then out[#out + 1] = it end
        end
    end
    return out
end

local function itemFields(it)
    local entry = tonumber(try(it, "GetEntry")) or 0
    return entry, tonumber(try(it, "GetItemLevel")) or 0, tonumber(try(it, "GetQuality")) or 1, clean(try(it, "GetName") or ""):sub(1, 60)
end

-- slot -> { entry, ilvl, quality, name }: what the bot wears
local function worn(m)
    local out = {}
    for _, slot in ipairs(GEAR_SLOTS) do
        local it = try(m, "GetEquippedItemBySlot", slot)
        if it then
            local entry, lv, q, nm = itemFields(it)
            out[slot] = { entry = entry, ilvl = lv, quality = q, name = nm }
        end
    end
    return out
end

-- slot -> { entry, ilvl, quality, name, now }: the best item in its bags for each slot that beats what it wears there
-- (the lower of the two rings, trinkets or hands), and that the bot can wear (Player:CanEquipItem when mod-ale has it)
local function upgrades(m, wearing)
    wearing = wearing or worn(m)
    local best = {}
    for _, it in ipairs(bagItems(m)) do
        local entry, lv, q, nm = itemFields(it)
        local slots = INV_SLOTS[tonumber(try(it, "GetInventoryType")) or 0]
        if entry > 0 and lv > 0 and slots then
            local pick, low
            for _, slot in ipairs(slots) do
                local cur = wearing[slot] and wearing[slot].ilvl or 0
                if (low == nil or cur < low) and try(m, "CanEquipItem", it, slot) ~= false then pick, low = slot, cur end
            end
            if pick and lv > low and (not best[pick] or lv > best[pick].ilvl) then
                best[pick] = { entry = entry, ilvl = lv, quality = q, name = nm, now = low }
            end
        end
    end
    return best
end

local function pushGear(player, e)
    if not OVMod then return end
    local m = e.m
    local wearing = worn(m)
    local lines = { table.concat({ "G", clean(e.c.name), e.c.cls or m:GetClass(), m:GetLevel(), itemLevel(m),
        e.c.buddy and "buddy" or e.c.alt and "alt" or e.c.dungeon and "dungeon" or "bot" }, "\t") }
    for _, slot in ipairs(GEAR_SLOTS) do
        local w = wearing[slot]
        if w then lines[#lines + 1] = table.concat({ "I", slot, w.entry, w.ilvl, w.quality, w.name }, "\t") end
    end
    local up = upgrades(m, wearing)
    for _, slot in ipairs(GEAR_SLOTS) do
        local u = up[slot]
        if u then lines[#lines + 1] = table.concat({ "U", slot, u.entry, u.ilvl, u.quality, u.name, u.now }, "\t") end
    end
    OVMod.Send(player, MOD, "gear", table.concat(lines, "\n"))
end

local function onInspect(player, data)
    if not windowReady(player) then return end
    local e = findBot(player, tostring(data or ""):match("^(%S+)$"))
    if not e then return say(player, "That bot isn't with you.") end
    pushGear(player, e)
end

-- One item from its bags: only an upgrade the bags really hold (the gear window's Equip). Alts too: it is their own
-- item, worn because the player asked.
local function onEquip(player, data)
    if not windowReady(player) then return end
    local name, entry = tostring(data or ""):match("^(%S+)\t(%d+)$")
    local e = findBot(player, name)
    if not e then return say(player, "That bot isn't with you.") end
    entry = tonumber(entry)
    local held
    for _, it in ipairs(bagItems(e.m)) do if tonumber(try(it, "GetEntry")) == entry then held = it end end
    if not held then return say(player, e.c.name .. " doesn't carry that item.") end
    if not calm(player) or busy(player, e) then return end
    tell(player, e.m, "e |Hitem:" .. entry .. ":0|h")
    ilvlCache[e.bg] = nil
    say(player, e.c.name .. " puts on " .. clean(try(held, "GetName") or "the item") .. ".")
    later(player, 1500, function(p)
        ilvlCache[e.bg] = nil
        local again = findBot(p, e.c.name)
        if again then pushGear(p, again) end
        pushRoster(p)
    end)
end

-- Everything better in its bags (mod-playerbots' "equip upgrade"): never new gear, so alts may too.
local function onUpgrade(player, data)
    if not windowReady(player) then return end
    local e = findBot(player, tostring(data or ""):match("^(%S+)$"))
    if not e then return say(player, "That bot isn't with you.") end
    local n = 0
    for _ in pairs(upgrades(e.m)) do n = n + 1 end
    if n == 0 then return say(player, e.c.name .. " has nothing better in its bags.") end
    if not calm(player) or busy(player, e) then return end
    tell(player, e.m, "equip upgrade")
    ilvlCache[e.bg] = nil
    say(player, string.format("%s puts on %s from its bags.", e.c.name, plural(n, "upgrade")))
    later(player, 1500, function(p)
        ilvlCache[e.bg] = nil
        local again = findBot(p, e.c.name)
        if again then pushGear(p, again) end
        pushRoster(p)
    end)
end

-- The Your alts tab: the account's other characters (the ones .companions alts lists, plus which are with you
-- already) and the saved usual party.
local function pushAlts(player)
    if not OVMod then return end
    local guid = player:GetGUIDLow()
    local lines = {}
    for _, a in ipairs(accountAlts(player)) do
        local nm = safeName(a.name)
        if nm ~= "" then
            lines[#lines + 1] = table.concat({ "A", nm, tonumber(a.level) or 0, tonumber(a.cls) or 0,
                alreadyComing(guid, a.guid) and 1 or 0, 0 }, "\t")
        end
    end
    -- alts with you are online (as bots), so the offline list above misses them
    for bg, c in pairs(companions[guid] or {}) do
        if c.alt and safeName(c.name) ~= "" then
            lines[#lines + 1] = table.concat({ "A", safeName(c.name), 0, tonumber(c.cls) or 0, 1, 1 }, "\t")
        end
    end
    for _, r in ipairs(dbRows(string.format(
        "SELECT c.guid, c.name, c.level, c.class, c.online FROM mod_companions_usual u JOIN characters c "
        .. "ON c.guid = u.guid AND c.account = u.account WHERE u.account = %d ORDER BY u.slot", player:GetAccountId()),
        { "n", "s", "n", "n", "n" })) do
        local nm = safeName(r[2])
        if nm ~= "" then lines[#lines + 1] = "U\t" .. nm end
    end
    OVMod.Send(player, MOD, "alts", table.concat(lines, "\n"))
end

-- The Dungeons tab: pick a dungeon, and bots fill the party (the window's picks, else 1 tank, 1 healer, damage,
-- minus what the group has) and everyone meets at its door: the player is taken to the spot just inside the
-- instance (ov_companions_dungeons.lua, the same doors as ov_go.lua), the bots that join find the player there
-- (bringOver: inside an instance mod-playerbots' own summon), and the bots already with you are brought a few
-- seconds later. The same rules as .recruit (not in combat, not dead, not in battlegrounds or arenas, group
-- leader only), and: a party, not a raid; the dungeon's lowest level; not from inside another instance; one
-- trip every OV_COMPANIONS_DUNGEON_COOLDOWN seconds (the fill itself keeps .recruit's own cooldown). The core's
-- own entry rules still apply at the door (Player:Teleport says false when it refuses).
local lastTravel = {}
local function dungeonCommand(player, id, picks)
    local d = DUNGEONS[tostring(id or ""):lower()]
    if not d then return say(player, "That dungeon isn't on the list.") end
    if try(player, "IsDead") then return say(player, "Not while dead.") end
    if try(player, "IsInCombat") then return say(player, "Not in combat.") end
    if inPvp(player) then return say(player, "Not in battlegrounds or arenas.") end
    local group = try(player, "GetGroup")
    if group and try(group, "IsLeader", player:GetGUID()) == false then
        return say(player, "Only the group leader can pick a dungeon.")
    end
    if isRaidGroup(player) then return say(player, "You are in a raid. Dungeons are for a 5-player party.") end
    if player:GetLevel() < tonumber(d.min) then
        return say(player, string.format("%s opens at level %d.", d.name, d.min))
    end
    local here = try(player, "GetMapId") == tonumber(d.map)
    if instanceKind(player) and not here then return say(player, "Leave this dungeon first.") end
    if groupInCombat(player) then return say(player, "Wait until your group is out of combat.") end
    local guid = player:GetGUIDLow()
    -- 1. the party (fillCommand says what it recruits, or why not)
    if PARTY - groupSize(player) - #(pending[guid] or {}) > 0 then fillCommand(player, "party", nil, picks) end
    -- 2. the door
    if here then return say(player, "You are in " .. d.name .. " already: your bots join you here.") end
    if not dungeonTravel() or type(d.pos) ~= "table" then
        return say(player, "Head to " .. d.name .. ": your bots follow you.")
    end
    local now, cd = os.time(), travelCooldown()
    if lastTravel[guid] and now - lastTravel[guid] < cd then
        return say(player, string.format("You can travel to a dungeon again in %d s. Your bots stay with you.",
            cd - (now - lastTravel[guid])))
    end
    local p = d.pos
    if try(player, "Teleport", p[1], p[2], p[3], p[4], p[5]) == false then
        return say(player, "The way into " .. d.name .. " is closed to you right now.")
    end
    lastTravel[guid] = now
    say(player, "To " .. d.name .. ": your bots meet you at the door.")
    later(player, 5000, function(pl)
        for _, e in ipairs(myBots(pl)) do bringOver(pl, e.m) end
        pushRoster(pl)
    end)
end

------------------------------------------------------------------------------------------------
-- single player: bots that join by themselves (OV_COMPANIONS_AUTO, OV_COMPANIONS_FOLLOW, OV_COMPANIONS_DUNGEON_AUTO)
------------------------------------------------------------------------------------------------
-- A new player never lands alone and never needs a chat command:
--   First entry   the first time a character enters the world, OV_COMPANIONS_FOLLOW bots (default 2) join it, ~6 s
--                 after login, with one welcome line saying how to change them (the Bots window; the window's
--                 add-on adds its key, Companions.lua "welcome"). The party covers what the player is not:
--                     you deal damage   -> a healer, then a tank, then damage
--                     you tank          -> a healer, then damage
--                     you heal          -> a tank, then damage
--                 (your role: your talents through mod-playerbots' IsTank / IsHealer; with none yet, damage). Two
--                 bots for a damage dealer = a healer that keeps you up and a tank that holds what you pull: the
--                 most forgiving open-world group, and a dungeon only needs two more. They keep to your level as
--                 you level (as the buddy does), with their role's premade spec.
--   Each login    the same party comes back (the classes and specs that were with you at logout, at most FOLLOW),
--                 unless you sent them all home (.dismiss, the window's Dismiss, the buddy's off): then nothing joins
--                 by itself until you bring bots again (any fill, buddy or dungeon pick turns it back on at logout).
--   Dungeons      entering a 5-player dungeon (walking in, the Dungeons tab, or the Dungeon Finder alone through
--                 mod-solo-lfg) fills the free slots with dungeon bots (1 tank, 1 healer, damage, minus what the
--                 group has); leaving to the open world alive sends just those home and brings your party over.
-- Nothing here runs unless the realm's profile turns it on (main.env never does), and never for bots themselves.
local AUTO_LOGIN_MS, AUTO_DUNGEON_MS, AUTO_RETRY_MS, AUTO_TRIES = 6000, 4000, 10000, 4
-- what the bots that join by themselves should cover, by the player's own role
local AUTO_ORDER = {
    dps    = { "healer", "tank", "dps", "dps" },
    tank   = { "healer", "dps", "dps", "dps" },
    healer = { "tank", "dps", "dps", "dps" },
}
local CLASS_WORD = {}
for w, id in pairs(CLASS) do CLASS_WORD[id] = w end

local function autoRow(guid)
    local r = dbRows(string.format("SELECT mode, party FROM mod_companions_auto WHERE guid = %d", guid), { "s", "s" })[1]
    if not r or (r[1] ~= "new" and r[1] ~= "on" and r[1] ~= "off") then return nil end
    return { mode = r[1], party = r[2] or "" }
end

local function autoSet(guid, mode, party)
    party = tostring(party or "")
    if not party:match("^[%a =:,]*$") or #party > 255 then party = "" end
    dbExec(string.format("REPLACE INTO mod_companions_auto (guid, mode, party, at) VALUES (%d, '%s', '%s', %d)",
        guid, mode, party, os.time()))
end

-- The party to remember: the player's bots that aren't alts or dungeon bots, as "role=class:spec" / "buddy=class".
local function partyOf(guid)
    local items = {}
    for _, c in pairs(companions[guid] or {}) do
        local word = CLASS_WORD[c.cls or 0]
        if word and not c.alt and not c.dungeon then
            if c.buddy then items[#items + 1] = "buddy=" .. word
            elseif ROLES[c.role or ""] then
                local spec = tostring(c.spec or "")
                items[#items + 1] = c.role .. "=" .. word .. (spec:match("^[%a ]+$") and (":" .. spec) or "")
            end
        end
    end
    table.sort(items, function(x, y)
        local rx, ry = RANK[x:match("^%a+")] or 4, RANK[y:match("^%a+")] or 4
        if rx ~= ry then return rx < ry end
        return x < y
    end)
    return table.concat(items, ",")
end

-- A remembered party back into recruit entries (a class that can't take the role, or isn't in this world: dropped).
local function partyEntries(player, party)
    local out = {}
    for item in tostring(party or ""):gmatch("[^,]+") do
        local role, word, spec = item:match("^(%a+)=(%a+):?([%a ]*)$")
        word = word and classOf(word)
        if word and (word ~= "dk" or dkOk(player)) then
            if role == "buddy" then
                if buddyOn() then out[#out + 1] = { cls = CLASS[word], word = word, role = "buddy", buddy = true } end
            elseif ROLES[role or ""] then
                local fits
                for _, pk in ipairs(ROLES[role].picks) do if pk[1] == word then fits = pk[2] end end
                if fits then
                    for _, sp in ipairs(specsFor(CLASS[word])) do
                        if sp == spec and specRole(sp) == role then fits = sp end
                    end
                    out[#out + 1] = { cls = CLASS[word], word = word, spec = fits, role = role }
                end
            end
        end
    end
    return out
end

-- The party for a character with none remembered: n bots covering what the player is not (AUTO_ORDER).
local function freshEntries(player, n)
    local counts, order = { tank = 0, healer = 0, dps = 0 }, AUTO_ORDER[roleOf(player, player)] or AUTO_ORDER.dps
    for i = 1, math.min(n, #order) do counts[order[i]] = counts[order[i]] + 1 end
    local list = {}
    for _, r in ipairs(ROLE_ORDER) do addRole(player, list, r, counts[r]) end
    return list
end

local function labelOf(m, c)
    local role = c.buddy and "buddy" or (ROLES[c.role or ""] and ROLES[c.role].label) or "bot"
    return string.format("%s the %s (%s)", m:GetName(), CLASS_LABEL[m:GetClass()] or "bot", role)
end

local function joinNames(names)
    if #names <= 1 then return names[1] or "" end
    return table.concat(names, ", ", 1, #names - 1) .. " and " .. names[#names]
end

-- Called by arrivals for each bot that joined by itself (p, m), and with no bot once the arrivals are over: then,
-- when no bot of the kind is still on the way, one line for all of them.
autoArrived = function(owner, p, m)
    local guid = owner:GetGUIDLow()
    if p then
        local a = autoJoined[guid] or { names = {} }
        autoJoined[guid] = a
        a.kind = p.dungeon and "dungeon" or p.auto
        local c = (companions[guid] or {})[m:GetGUIDLow()] or {}
        a.names[#a.names + 1] = labelOf(m, c)
        return
    end
    local a = autoJoined[guid]
    if not a then return end
    for _, q in ipairs(pending[guid] or {}) do if q.auto or q.dungeon then return end end
    autoJoined[guid] = nil
    if #a.names == 0 then return end
    local who = joinNames(a.names)
    if a.kind == "dungeon" then
        owner:SendBroadcastMessage(TAG .. "Dungeon bots join you: " .. who .. ". They head home when you leave.")
    elseif a.kind == "back" then
        owner:SendBroadcastMessage(TAG .. "Your bots are back: " .. who .. ".")
    else
        local here, rest = "Your bots are here: " .. who .. ".", "They follow you, fight with you and leave the loot to you."
        if hasWindow[guid] and OVMod then
            OVMod.Send(owner, MOD, "welcome", here .. " " .. rest)   -- the add-on says it, with the Bots window's key
        else
            if #here + #rest < 150 then owner:SendBroadcastMessage(TAG .. here .. " " .. rest)
            else owner:SendBroadcastMessage(TAG .. here); owner:SendBroadcastMessage(TAG .. rest) end
            owner:SendBroadcastMessage(TAG .. "Change them any time: .companions")
        end
    end
    if a.kind ~= "dungeon" and autoOn() then autoSet(guid, "on", partyOf(guid)) end
end

-- After the player sends bots home: none left with them (dungeon bots aside) = they chose to play alone, kept.
autoDismissed = function(player)
    if not autoOn() then return end
    local guid = player:GetGUIDLow()
    for _, c in pairs(companions[guid] or {}) do if not c.dungeon then return end end
    local row = autoRow(guid)
    if row and row.mode == "off" then return end
    autoSet(guid, "off", "")
    say(player, "They won't join you by themselves now. Bring bots back any time from the Bots window.")
end

-- n bots join the player by themselves (kind "first" / "back"), or dungeon bots fill the party (kind "dungeon").
-- Not now (combat, dead, a recruit on the way): again in 10 s, AUTO_TRIES times.
local autoTry
local function autoJoin(player, kind, tries)
    local guid = player:GetGUIDLow()
    local function again()
        if tries < AUTO_TRIES then
            later(player, AUTO_RETRY_MS, function(p) autoTry(p, kind, tries + 1) end)
        end
    end
    if #(pending[guid] or {}) > 0 or try(player, "IsInCombat") or try(player, "IsDead") then return again() end
    local list
    if kind == "dungeon" then
        if not dungeonAutoOn() or instanceKind(player) ~= "dungeon" or isRaidGroup(player) then return end
        prune(player)
        local free = PARTY - groupSize(player)
        if free <= 0 then return end
        local n = room(player, free, { auto = true, size = PARTY })
        if n == 0 then return again() end
        local counts = planRoles(rolesInGroup(player), SHAPE[PARTY], n)
        list = {}
        for _, r in ipairs(ROLE_ORDER) do addRole(player, list, r, counts[r]) end
        for _, p in ipairs(list) do p.dungeon = true end
    else
        if not autoOn() then return end
        prune(player)
        local row = autoRow(guid)
        if row and row.mode == "off" then return end
        local want = math.min(autoCount(), perPlayer()) - mine(guid)
        if want <= 0 then return end
        local n = room(player, want, { auto = true })
        if n == 0 then return again() end
        local saved = row and row.mode == "on" and partyEntries(player, row.party) or {}
        list = {}
        if #saved > 0 then
            for i = 1, math.min(n, #saved) do list[#list + 1] = saved[i] end
        else
            list = freshEntries(player, n)
        end
        if not row then autoSet(guid, "new", "") end
        for _, p in ipairs(list) do p.auto = kind end
    end
    if #list == 0 then return end
    queue(player, list, true)
end
autoTry = function(p, kind, tries)
    if not p or isBot(p) or not botsBuilt(p) then return end
    local ok, err = pcall(autoJoin, p, kind, tries)
    if not ok then print("[ov_companions] auto join failed: " .. tostring(err)) end
end

-- Login: the first entry's party, or the remembered one. Not after a crash: the module brings those bots back by
-- itself and onLogin's looks adopt them (their ownership rows are still there).
local function autoLogin(player)
    if not (autoOn() or dungeonAutoOn()) or isBot(player) or not botsBuilt(player) then return end
    local guid = player:GetGUIDLow()
    if next(ownRows(guid)) then return end
    later(player, AUTO_LOGIN_MS, function(p)
        if autoOn() then
            local row = autoRow(guid)
            autoTry(p, (not row or row.mode == "new") and "first" or "back", 1)
        end
        -- logged in inside a dungeon: its bots too, once the party is here (autoJoin waits for the arrivals)
        if dungeonAutoOn() and instanceKind(p) == "dungeon" then
            later(p, AUTO_DUNGEON_MS, function(q) autoTry(q, "dungeon", 1) end)
        end
    end)
end

-- Map change: into a 5-player dungeon, its bots; out to the open world (alive: a ghost running back keeps them),
-- the dungeon bots go home and the rest of the party is brought over.
local function onMapChange(event, player)
    if not player or isBot(player) or not botsBuilt(player) or not onSwitch() then return end
    local guid = player:GetGUIDLow()
    local kind = instanceKind(player)
    if kind == "dungeon" then
        if dungeonAutoOn() then later(player, AUTO_DUNGEON_MS, function(p) autoTry(p, "dungeon", 1) end) end
        return
    end
    if kind or try(player, "IsDead") then return end
    local temps = {}
    for bg, c in pairs(companions[guid] or {}) do if c.dungeon then temps[#temps + 1] = bg end end
    if #temps == 0 then return end
    local _, members = groupMembers(player)
    local byGuid = {}
    for _, m in ipairs(members) do byGuid[m:GetGUIDLow()] = m end
    local names = {}
    for _, bg in ipairs(temps) do
        local c = companions[guid][bg]
        companions[guid][bg] = nil
        ownForget(bg)
        if byGuid[bg] then tell(player, byGuid[bg], "leave") end
        local nm = safeName(c.name)
        if nm ~= "" then names[#names + 1] = nm end
    end
    say(player, "Your dungeon bots head home.")
    later(player, 2500, function(p)
        for _, nm in ipairs(names) do botCommand(p, "remove " .. nm) end
        for _, e in ipairs(myBots(p)) do bringOver(p, e.m) end
        pushRoster(p)
    end)
end

-- Logout: remember the party that was with the player (an empty one changes nothing: a party that never arrived,
-- or one the module already logged out; sending them home on purpose was kept at the time, autoDismissed).
local function autoLogout(player)
    if not autoOn() or isBot(player) then return end
    local guid = player:GetGUIDLow()
    local party = partyOf(guid)
    if party ~= "" then autoSet(guid, "on", party) end
end

if OVMod then
    OVMod.On(MOD, "roster", function(player)
        if isBot(player) then return end
        hasWindow[player:GetGUIDLow()] = true
        if flooding(player:GetGUIDLow()) then return end
        pushRoster(player)
    end)
    OVMod.On(MOD, "order", onOrder)
    OVMod.On(MOD, "formation", onFormation)
    OVMod.On(MOD, "prepare", onPrepare)
    OVMod.On(MOD, "spec", onSpec)
    OVMod.On(MOD, "role", onRole)
    OVMod.On(MOD, "gear", onGear)
    OVMod.On(MOD, "inspect", onInspect)
    OVMod.On(MOD, "equip", onEquip)
    OVMod.On(MOD, "upgrade", onUpgrade)
    OVMod.On(MOD, "fill", function(player, data)
        if not windowReady(player) then return end
        local kind, picks = tostring(data or ""):match("^(%a+)\t?(.*)$")
        if kind ~= "party" and kind ~= "raid" then return say(player, "Fill a party or a raid.") end
        fillCommand(player, kind, nil, (picks or "") ~= "" and parsePicks(picks) or nil)
    end)
    OVMod.On(MOD, "alts", function(player)
        if isBot(player) or flooding(player:GetGUIDLow()) then return end
        if botsBuilt(player) then pushAlts(player) end
    end)
    OVMod.On(MOD, "alt", function(player, data)
        if not windowReady(player) then return end
        local name = tostring(data or ""):match("^(%S+)$")
        if not name then return say(player, "Which character?") end
        altCommand(player, name)
        later(player, 3000, pushAlts)
    end)
    OVMod.On(MOD, "save", function(player)
        if not windowReady(player) then return end
        saveCommand(player)
        pushAlts(player)
    end)
    OVMod.On(MOD, "buddy", function(player, data)
        if not windowReady(player) then return end
        local arg = tostring(data or ""):lower():match("^(%a+)$")
        if arg == "any" then arg = nil end
        if not buddyOn() then return say(player, "The levelling buddy is off on this realm. Try a party fill.") end
        if not arg and buddyOf(player:GetGUIDLow()) then return buddyCommand(player, nil) end
        buddyCommand(player, arg)
        if arg == "off" then later(player, 3000, pushRoster) end
    end)
    OVMod.On(MOD, "dungeon", function(player, data)
        if not windowReady(player) then return end
        local id, picks = tostring(data or ""):match("^([%w%-]+)\t?(.*)$")
        dungeonCommand(player, id, (picks or "") ~= "" and parsePicks(picks) or nil)
    end)
    OVMod.On(MOD, "usual", function(player) if windowReady(player) then usualCommand(player) end end)
    OVMod.On(MOD, "dismiss", function(player, data)
        if not windowReady(player) then return end
        local arg = tostring(data or ""):lower():match("^(%S+)$")
        if not arg then return say(player, "Dismiss whom?") end
        dismissCommand(player, arg)
        pushRoster(player)
    end)
end

------------------------------------------------------------------------------------------------
-- events
------------------------------------------------------------------------------------------------
local COMMANDS = { companions = true, companion = true, recruit = true, buddy = true, dismiss = true }

-- The module's own chat commands (.playerbots bot add <name> / addclass <class> / addaccount / init=...) from a
-- player who isn't a GM would skip every limit above (per player, per realm, group size, cooldown). They go
-- through the same checks as .recruit instead; the rest (list, remove, logout, help) passes through.
local RAW_BOT = { playerbots = true, playerbot = true, bot = true }
local RAW_PASS = { list = true, ls = true, remove = true, rm = true, logout = true, help = true }

local function isGM(player)
    local rank = tonumber(try(player, "GetGMRank") or 0) or 0
    return rank > 0 or try(player, "IsGM") == true
end

local function rawBotCommand(player, words, rawWords)
    local sub, at = nil, nil
    if words[1] == "bot" then sub, at = words[2], 3
    elseif words[2] == "bot" then sub, at = words[3], 4
    else return nil end                                 -- other module commands keep their own security levels
    if selfCommand or not sub or RAW_PASS[sub] or isGM(player) or isBot(player) then return nil end
    replyTo[player:GetGUIDLow()] = nil
    if not botsBuilt(player) then comingSoon(player); return false end
    if not onSwitch() then say(player, "Bots are off on this realm."); return false end
    if sub == "add" and rawWords[at] and not rawWords[at + 1] and not rawWords[at]:find(",", 1, true) then
        altCommand(player, rawWords[at])              -- one of the player's own characters, as .recruit alt
    elseif sub == "addclass" and words[at] and not words[at + 1] then
        local cls = classOf(words[at])
        if not cls then return say(player, "Usage: |cffffff00.recruit tank|healer|dps <class>|r") or false end
        if cls == "dk" and not dkOk(player) then
            say(player, dkRefusal()); return false
        end
        local role, spec
        for _, r in ipairs(ROLE_ORDER) do
            for _, pk in ipairs(ROLES[r].picks) do
                if not role and pk[1] == cls then role, spec = r, pk[2] end
            end
        end
        if not role then return say(player, "Usage: |cffffff00.recruit tank|healer|dps <class>|r") or false end
        if room(player, 1) == 0 then return false end
        queue(player, { { cls = CLASS[cls], word = cls, spec = spec, role = role } })
        say(player, string.format("Recruiting a %s bot: it joins your group in a few seconds.", CLASS_LABEL[CLASS[cls]] or cls))
    else
        say(player, "Bring bots with |cffffff00.recruit|r (party, raid, tank, healer, dps, alt <name>) or the Bots window.")
    end
    return false
end

local function onCommand(event, player, command)
    if not player then return end
    local raw = (command or ""):gsub("^[%.!]", "")
    local text = raw:lower()
    local words, rawWords = {}, {}
    for w in text:gmatch("%S+") do words[#words + 1] = w end
    for w in raw:gmatch("%S+") do rawWords[#rawWords + 1] = w end
    local cmd = words[1]
    if RAW_BOT[cmd] then return rawBotCommand(player, words, rawWords) end
    if not COMMANDS[cmd] then return end
    if isBot(player) then return false end
    replyTo[player:GetGUIDLow()] = nil   -- typed: the answers go to the chat
    if not botsBuilt(player) then comingSoon(player); return false end
    if cmd == "companions" or cmd == "companion" then
        if words[2] == "alts" then altsCommand(player)
        elseif words[2] == "save" then saveCommand(player)
        else showHelp(player) end
    elseif not onSwitch() then
        say(player, "Bots are off on this realm.")
    elseif cmd == "recruit" then
        local w = words[2]
        if w == "party" or w == "raid" then fillCommand(player, w, words[3])
        elseif w == "alt" then altCommand(player, rawWords[3])
        elseif w == "usual" then usualCommand(player)
        else roleCommand(player, w, words[3]) end
    elseif cmd == "buddy" then
        buddyCommand(player, words[2])
    elseif cmd == "dismiss" then
        dismissCommand(player, words[2])
    end
    return false
end

-- The buddy keeps to the player's level (mod-playerbots re-levels and re-gears it with init=auto), and so do the
-- bots that joined by themselves (single player: c.keep), which then get their role's spec back.
local function keepers(guid)
    for _, c in pairs(companions[guid] or {}) do if c.buddy or c.keep then return true end end
    return false
end
local function onLevelChange(event, player, oldLevel)
    if not player or isBot(player) then return end
    local guid = player:GetGUIDLow()
    if not keepers(guid) then return end
    player:RegisterEvent(function(_, _, _, p)
        if not p then return end
        prune(p)
        local _, members = groupMembers(p)
        local byGuid = {}
        for _, m in ipairs(members) do byGuid[m:GetGUIDLow()] = m end
        local caught, waiting = 0, false
        for bg, c in pairs(companions[guid] or {}) do
            local m = byGuid[bg]
            if m and (c.buddy or c.keep) and not c.alt and m:GetLevel() ~= p:GetLevel() then
                if try(p, "IsInCombat") or try(m, "IsInCombat") then
                    if c.buddy then say(p, c.name .. " catches up to your level after the fight (.buddy, or level again).")
                    else waiting = true end
                else
                    botCommand(p, "init=auto " .. safeName(c.name))
                    if c.spec and not c.buddy then tell(p, m, "talents spec " .. c.spec) end
                    tell(p, m, "follow")
                    if c.buddy then say(p, string.format("%s keeps up: level %d.", c.name, p:GetLevel()))
                    else caught = caught + 1 end
                end
            end
        end
        if caught > 0 then say(p, string.format("Your bots keep up: level %d.", p:GetLevel())) end
        if waiting then say(p, "Your bots catch up to your level after the fight (or when you level again).") end
    end, 3000, 1)
end

-- After a crash the module logs the player's bots back in with the player as their master (seen live
-- 2026-10-26: a 10-player raid came back, our tables were empty). Look for them a few times after login.
local LOGIN_LOOKS, LOGIN_LOOK_MS = 3, 15000
local function onLogin(event, player)
    if not player or isBot(player) or not botsBuilt(player) then return end
    autoLogin(player)
    local looks, found = 0, false
    player:RegisterEvent(function(_, _, _, p)
        looks = looks + 1
        if not p or found or isBot(p) then return end
        local n = adopt(p)
        if n > 0 then
            found = true
            say(p, plural(n, "bot") .. (n == 1 and " is" or " are") .. " back with you. (.dismiss all sends them home)")
            pushRoster(p)
        end
    end, LOGIN_LOOK_MS, LOGIN_LOOKS)
end

local function onLogout(event, player)
    if not player then return end
    local guid = player:GetGUIDLow()
    pcall(autoLogout, player)
    companions[guid], pending[guid], ticking[guid], raidWanted[guid] = nil, nil, nil, nil
    autoJoined[guid], hasWindow[guid] = nil, nil
    orders[guid], replyTo[guid], lastPrepare[guid], rate[guid] = nil, nil, nil, nil
    -- A normal logout: the module logs the bots out with the player, so the ownership rows go. (A crash never
    -- gets here: the rows stay for adopt.)
    if not isBot(player) then ownForgetOwner(guid) end
end

RegisterPlayerEvent(3, onLogin)         -- PLAYER_EVENT_ON_LOGIN
RegisterPlayerEvent(4, onLogout)        -- PLAYER_EVENT_ON_LOGOUT
RegisterPlayerEvent(13, onLevelChange)  -- PLAYER_EVENT_ON_LEVEL_CHANGE
RegisterPlayerEvent(28, onMapChange)    -- PLAYER_EVENT_ON_MAP_CHANGE
RegisterPlayerEvent(42, onCommand)      -- PLAYER_EVENT_ON_COMMAND

-- for stream/tests/test_ov_companions.lua (read only)
OV_COMPANIONS_TEST = { companions = companions, pending = pending, realmCount = realmCount, ROLES = ROLES,
    planRoles = planRoles, SHAPE = SHAPE, specsFor = specsFor, specRole = specRole, adopt = adopt, orders = orders,
    pushRoster = pushRoster, window = OVMod ~= nil, DUNGEONS = DUNGEONS, parsePicks = parsePicks,
    AUTO_ORDER = AUTO_ORDER, partyOf = partyOf, partyEntries = partyEntries, freshEntries = freshEntries,
    upgrades = upgrades, INV_SLOTS = INV_SLOTS, applyTactics = function(...) return applyTactics(...) end }
