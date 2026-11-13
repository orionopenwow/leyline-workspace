-- ov_boards.lua: what the signed leaderboards need from the realm that nothing else records (mod-ale, 3.3.5a).
-- Off unless OV_BOARDS=1 (main.env; Boss Rush's profile). docs/LEADERBOARDS.md.
--
-- What it does
--   * Deaths in a boss fight: every player or companion killed by a creature in a boss fight's raid (the maps of Boss
--     Rush's catalog) is one row of ov_board_deaths (the time, the instance, bot or not). The board export counts a
--     run's deaths from them: the rows in the run's own instance between its pull and its end
--     (realm/custom/boards-export.py count_deaths). Nothing about the run itself changes.
--   * 1v1 arena results: when an arena with one player a side ends, the winner and the loser are one row of
--     ov_arena_log (the week, the day, how long it took). The export makes this week's 1v1 arena board from it, at most
--     3 wins a day against the same opponent. Not counted: two characters of the same account, a bot (RNDBOT*), a GM
--     in GM mode, a game with no clear winner (both alive at the end: a draw or a timeout), spectators.
--   * .boards (or .board): where the boards are, and this week's featured fight.
--
-- Env: OV_BOARDS (1 = on), OV_BOARDS_FIGHTS (the fights this realm offers, in Boss Rush's catalog order, comma
--   separated; default frozen-throne: the official realm's one fight; the week's featured fight turns through them).
-- Tables (characters database, made here with CREATE TABLE IF NOT EXISTS; this script never reads a table at all:
--   the export does, read only): ov_board_deaths, ov_arena_log. boards-export.py --schema has the same text.
-- mod-ale (84b85cc): RegisterPlayerEvent 8 ON_KILLED_BY_CREATURE (event, killer, killed), 42 ON_COMMAND;
--   RegisterBGEvent 2 BG_EVENT_ON_END (event, bg, bgId, instanceId, winner); BattleGround:GetMap,
--   GetMaxPlayersPerTeam, GetMapId; Map:IsArena, GetPlayers, GetInstanceId, GetMapId; Player:GetGUIDLow, GetName,
--   GetClass, GetAccountId, IsGM, IsSpectator; Unit:IsDead. Every call goes through try(): a missing method is a
--   skipped row, never an error in the world server.

local function flag(name) return os.getenv(name) == "1" end

local ON = flag("OV_BOARDS")
local TAG = "|cff66ccff[Boards]|r "
local SITE = "leylinelauncher.com/wrath/boards"
local WEEK0 = 4 * 86400            -- Monday 5 Jan 1970 00:00 UTC (boards-export.py WEEK0)
local WEEK = 7 * 86400
-- Boss Rush's catalog (launcher/src-tauri/src/host/bossrush.rs FIGHTS): key, title and raid map. Our own titles; a
-- boss's own name never shows.
local CATALOG = {
    { "frozen-throne", "The Frozen Throne", 631 }, { "bone-lord", "The Bone Lord", 631 },
    { "cult-mistress", "The Cult Mistress", 631 }, { "twilight-destroyer", "The Twilight Destroyer", 724 },
    { "obsidian-sanctum", "The Obsidian Sanctum", 615 }, { "obsidian-three", "The Obsidian Sanctum, three drakes", 615 },
    { "brood-mother", "The Brood Mother's Lair", 249 }, { "stone-watcher", "The Stone Watcher", 624 },
    { "storm-watcher", "The Storm Watcher", 624 }, { "flame-watcher", "The Flame Watcher", 624 },
    { "frost-watcher", "The Frost Watcher", 624 }, { "spell-weaver", "The Spellweaver's Eye", 616 },
}

local B = {}            -- exposed for tests as OV_BOARDS_TEST
B.rows = 0

local function now() return os.time() end
local function try(obj, method, ...)
    if not obj then return nil end
    local ok, fn = pcall(function() return obj[method] end)
    if not ok or type(fn) ~= "function" then return nil end
    local ok2, a = pcall(fn, obj, ...)
    if ok2 then return a end
    return nil
end
local function isBot(p)
    if try(p, "IsBot") == true or try(p, "IsPlayerbot") == true then return true end
    local name = try(p, "GetAccountName")
    return type(name) == "string" and name:upper():find("^RNDBOT") ~= nil
end
local function esc(s) return (tostring(s or ""):gsub("[%z\1-\31]", ""):gsub("\\", "\\\\"):gsub("'", "''")) end
local function say(p, text) if p then try(p, "SendBroadcastMessage", TAG .. text) end end

local SCHEMA = {
    "CREATE TABLE IF NOT EXISTS ov_board_deaths (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, "
        .. "at INT UNSIGNED NOT NULL, instance_id INT UNSIGNED NOT NULL, map SMALLINT UNSIGNED NOT NULL, "
        .. "guid INT UNSIGNED NOT NULL, bot TINYINT UNSIGNED NOT NULL DEFAULT 0, KEY inst (instance_id, at))",
    "CREATE TABLE IF NOT EXISTS ov_arena_log (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, "
        .. "at INT UNSIGNED NOT NULL, week CHAR(10) NOT NULL, day CHAR(10) NOT NULL, bracket TINYINT UNSIGNED NOT NULL, "
        .. "map SMALLINT UNSIGNED NOT NULL, secs INT UNSIGNED NOT NULL DEFAULT 0, "
        .. "winner INT UNSIGNED NOT NULL, winner_name VARCHAR(12) NOT NULL, winner_class TINYINT UNSIGNED NOT NULL, "
        .. "loser INT UNSIGNED NOT NULL, loser_name VARCHAR(12) NOT NULL, loser_class TINYINT UNSIGNED NOT NULL, "
        .. "KEY week (week), KEY pair_day (day, winner, loser))",
}

-- the week and the featured fight ----------------------------------------------------------------------------------
function B.weekStart(t) return WEEK0 + math.floor((t - WEEK0) / WEEK) * WEEK end
function B.weekKey(t) return os.date("!%Y-%m-%d", B.weekStart(t)) end
function B.dayKey(t) return os.date("!%Y-%m-%d", t) end

-- The fights this realm offers, in catalog order (unknown keys left out; none known: the first fight).
function B.fights(list)
    local want, out = {}, {}
    for k in tostring(list or ""):gmatch("[^,%s]+") do want[k] = true end
    for _, f in ipairs(CATALOG) do if want[f[1]] then out[#out + 1] = f end end
    if #out == 0 then out[1] = CATALOG[1] end
    return out
end
local FIGHTS = B.fights(os.getenv("OV_BOARDS_FIGHTS") or "frozen-throne")

-- This week's featured fight: the offered fights in turn, one a week (boards-export.py featured, bossrush.rs
-- featured_fight: the same rule).
function B.featured(t, fights)
    fights = fights or FIGHTS
    local n = math.floor((t - WEEK0) / WEEK)
    return fights[(n % #fights) + 1]
end

local BOSS_MAPS = {}
for _, f in ipairs(CATALOG) do BOSS_MAPS[f[3]] = true end
B.BOSS_MAPS = BOSS_MAPS

-- deaths in a boss fight -------------------------------------------------------------------------------------------
function B.onKilled(event, killer, killed)
    if not killed then return end
    local map = try(killed, "GetMapId")
    if not map or not BOSS_MAPS[map] then return end
    local m = try(killed, "GetMap")
    local inst = try(killed, "GetInstanceId") or try(m, "GetInstanceId")
    if not inst or inst == 0 then return end
    local guid = try(killed, "GetGUIDLow")
    if not guid then return end
    CharDBExecute(string.format("INSERT INTO ov_board_deaths (at, instance_id, map, guid, bot) VALUES (%d, %d, %d, %d, %d)",
        now(), inst, map, guid, isBot(killed) and 1 or 0))
    B.rows = B.rows + 1
end

-- 1v1 arena results ------------------------------------------------------------------------------------------------
-- Why a finished arena isn't counted (nil: it is).
function B.whyNot(a, b)
    if not a or not b then return "not two players" end
    if a.account == b.account then return "same account" end
    if a.bot or b.bot then return "a bot" end
    if a.gm or b.gm then return "a GM" end
    if a.dead == b.dead then return "no clear winner" end
    return nil
end

local function who(p)
    return { guid = try(p, "GetGUIDLow") or 0, name = try(p, "GetName") or "", class = try(p, "GetClass") or 0,
             account = try(p, "GetAccountId") or -1, bot = isBot(p), gm = try(p, "IsGM") == true,
             dead = try(p, "IsDead") == true }
end

B.started = {}          -- arena instance id -> os.time() at its start

function B.onStart(event, bg, bgId, instanceId)
    if instanceId then B.started[instanceId] = now() end
end

function B.onEnd(event, bg, bgId, instanceId, winner)
    local m = try(bg, "GetMap")
    if not m or try(m, "IsArena") ~= true then return end
    if (try(bg, "GetMaxPlayersPerTeam") or 0) ~= 1 then return end
    local list = try(m, "GetPlayers") or {}
    local players = {}
    for _, p in pairs(list) do
        if try(p, "IsSpectator") ~= true then players[#players + 1] = who(p) end
    end
    local t = now()
    local secs = B.started[instanceId or -1] and (t - B.started[instanceId]) or 0
    B.started[instanceId or -1] = nil
    if #players ~= 2 then return "not two players" end
    local a, b = players[1], players[2]
    local why = B.whyNot(a, b)
    if why then return why end
    local w, l = a, b
    if w.dead then w, l = b, a end
    CharDBExecute(string.format("INSERT INTO ov_arena_log (at, week, day, bracket, map, secs, winner, winner_name, "
        .. "winner_class, loser, loser_name, loser_class) VALUES (%d, '%s', '%s', 1, %d, %d, %d, '%s', %d, %d, '%s', %d)",
        t, B.weekKey(t), B.dayKey(t), try(bg, "GetMapId") or try(m, "GetMapId") or 0, secs,
        w.guid, esc(w.name):sub(1, 12), w.class, l.guid, esc(l.name):sub(1, 12), l.class))
    B.rows = B.rows + 1
    return nil
end

-- .boards ----------------------------------------------------------------------------------------------------------
function B.onCommand(event, p, command)
    if not p then return end
    local verb = ((command or ""):gsub("^[%.!]", "")):match("^(%S+)")
    verb = (verb or ""):lower()
    if verb ~= "boards" and verb ~= "board" and verb ~= "leaderboards" then return end
    if not ON then say(p, "The signed leaderboards are off on this realm."); return false end
    local f = B.featured(now())
    say(p, "This week's featured fight: |cffffff00" .. f[2] .. "|r. Clear times, duel ratings and 1v1 arena wins, "
        .. "signed by this realm: " .. SITE .. " (and the launcher's Modes page).")
    return false
end

if ON then
    for _, sql in ipairs(SCHEMA) do CharDBExecute(sql) end
    RegisterPlayerEvent(8, B.onKilled)        -- PLAYER_EVENT_ON_KILLED_BY_CREATURE
    if RegisterBGEvent then
        RegisterBGEvent(1, B.onStart)          -- BG_EVENT_ON_START
        RegisterBGEvent(2, B.onEnd)            -- BG_EVENT_ON_END
    end
end
RegisterPlayerEvent(42, B.onCommand)          -- PLAYER_EVENT_ON_COMMAND (says "off" when the boards are off)

B.SCHEMA, B.CATALOG, B.FIGHTS, B.ON = SCHEMA, CATALOG, FIGHTS, ON
OV_BOARDS_TEST = B
