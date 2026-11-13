-- ov_boards.lua (deaths in boss fights, 1v1 arena results, .boards) under a fake mod-ale.
--   lua5.1 stream/tests/test_ov_boards.lua realm/custom/lua_scripts/ov_boards.lua     (texlua runs it too)
local file = assert(arg[1], "usage: lua5.1 test_ov_boards.lua <ov_boards.lua>")

local pass, fail = 0, 0
local function check(name, ok, extra)
    if ok then pass = pass + 1; print("PASS " .. name)
    else fail = fail + 1; print("FAIL " .. name .. (extra and ("  " .. tostring(extra)) or "")) end
end

-- a fixed clock: Thursday 2026-11-12 19:00:00 UTC
local clock = 1794510000   -- calendar.timegm((2026, 11, 12, 19, 0, 0))
local realTime = os.time
os.time = function(t) if t then return realTime(t) end return clock end
local env = {}
os.getenv = function(k) return env[k] end

local executed, queried, events, bgEvents = {}, {}, {}, {}
function CharDBExecute(sql) executed[#executed + 1] = sql end
function CharDBQuery(sql) queried[#queried + 1] = sql; return nil end
function RegisterPlayerEvent(id, fn) events[id] = fn end
function RegisterBGEvent(id, fn) bgEvents[id] = fn end

local function load(e)
    env = e or {}
    executed, queried, events, bgEvents = {}, {}, {}, {}
    OV_BOARDS_TEST = nil
    dofile(file)
    return OV_BOARDS_TEST
end

-- fakes
local function player(t)
    local p = { said = {} }
    function p:GetGUIDLow() return t.guid end
    function p:GetName() return t.name end
    function p:GetClass() return t.class or 1 end
    function p:GetAccountId() return t.account or t.guid end
    function p:GetAccountName() return t.accname or ("ACC" .. t.guid) end
    function p:IsGM() return t.gm or false end
    function p:IsDead() return t.dead or false end
    function p:IsSpectator() return t.spect or false end
    function p:GetMapId() return t.map or 0 end
    function p:GetInstanceId() return t.inst or 0 end
    function p:SendBroadcastMessage(m) self.said[#self.said + 1] = m end
    return p
end
local function arena(players, opts)
    opts = opts or {}
    local m = {}
    function m:IsArena() return opts.arena ~= false end
    function m:GetPlayers() return players end
    function m:GetMapId() return 617 end
    local bg = {}
    function bg:GetMap() return m end
    function bg:GetMaxPlayersPerTeam() return opts.per or 1 end
    function bg:GetMapId() return 617 end
    return bg
end

-- off: nothing registered but the command, nothing made
local B = load({})
check("off: no tables made", #executed == 0)
check("off: no death or arena hooks", events[8] == nil and bgEvents[2] == nil)
check("off: .boards says it is off", (function()
    local p = player({ guid = 1, name = "Orion" })
    local r = events[42](42, p, "boards")
    return r == false and p.said[1]:find("off") ~= nil
end)())
check("never reads a table", #queried == 0)

-- on
B = load({ OV_BOARDS = "1" })
check("on: makes its two tables", #executed == 2 and executed[1]:find("^CREATE TABLE IF NOT EXISTS ov_board_deaths")
    and executed[2]:find("^CREATE TABLE IF NOT EXISTS ov_arena_log"))
check("on: hooks deaths, the arena's start and end, the command", events[8] and bgEvents[1] and bgEvents[2] and events[42])

-- the week and the featured fight
check("week: Monday 9 Nov 2026 UTC", B.weekKey(clock) == "2026-11-09", B.weekKey(clock))
check("week: a Monday at 00:00 is its own week", B.weekKey(clock - 3 * 86400 - 19 * 3600) == "2026-11-09" and B.weekKey(clock - 3 * 86400 - 19 * 3600 - 1) == "2026-11-02")
check("week: Sunday night is the week before", B.weekKey(clock - 4 * 86400) == "2026-11-02")
check("fights: only the official one by default", #B.FIGHTS == 1 and B.FIGHTS[1][1] == "frozen-throne")
local all = B.fights("spell-weaver, frozen-throne,nope,bone-lord")
check("fights: catalog order, unknown left out", #all == 3 and all[1][1] == "frozen-throne" and all[2][1] == "bone-lord" and all[3][1] == "spell-weaver")
check("fights: none known is the first fight", B.fights("nope")[1][1] == "frozen-throne")
local wk = math.floor((clock - 4 * 86400) / (7 * 86400))
check("featured: the catalog in turn, one a week", B.featured(clock, B.CATALOG)[1] == B.CATALOG[(wk % 12) + 1][1]
    and B.featured(clock + 7 * 86400, B.CATALOG)[1] == B.CATALOG[((wk + 1) % 12) + 1][1])
check("featured: one fight is always that fight", B.featured(clock)[1] == "frozen-throne")
check("featured: no brand words in the titles", (function()
    for _, f in ipairs(B.CATALOG) do
        local t = f[2]:lower()
        if t:find("warcraft") or t:find("lich king") or t:find("blizzard") or t:find("%f[%a]wow%f[%A]") then return false end
    end
    return true
end)())

-- deaths
executed = {}
local cr = {}
events[8](8, cr, player({ guid = 7, name = "Orion", map = 631, inst = 55 }))
check("death: a death in a boss fight's raid is one row", #executed == 1
    and executed[1] == string.format("INSERT INTO ov_board_deaths (at, instance_id, map, guid, bot) VALUES (%d, 55, 631, 7, 0)", clock), executed[1])
events[8](8, cr, player({ guid = 8, name = "Botty", accname = "RNDBOT12", map = 724, inst = 56 }))
check("death: a companion's counts too, marked a bot", executed[2] and executed[2]:find("VALUES %(%d+, 56, 724, 8, 1%)"), executed[2])
events[8](8, cr, player({ guid = 9, name = "Elsewhere", map = 571, inst = 0 }))
events[8](8, cr, player({ guid = 9, name = "Noinst", map = 631, inst = 0 }))
check("death: elsewhere or outside an instance: nothing", #executed == 2)

-- the 1v1 arena
executed = {}
bgEvents[1](1, nil, 4, 900)
clock = clock + 95
local a = player({ guid = 21, name = "Alice", class = 8, account = 1 })
local b = player({ guid = 22, name = "Bob", class = 4, account = 2, dead = true })
local why = bgEvents[2](2, arena({ a, b }), 4, 900, 0)
check("arena: a clear win is one row", why == nil and #executed == 1, why)
check("arena: the winner, the loser, the week, the day, the time it took", executed[1] and executed[1] ==
    string.format("INSERT INTO ov_arena_log (at, week, day, bracket, map, secs, winner, winner_name, winner_class, loser, "
        .. "loser_name, loser_class) VALUES (%d, '2026-11-09', '2026-11-12', 1, 617, 95, 21, 'Alice', 8, 22, 'Bob', 4)", clock), executed[1])
local b2 = player({ guid = 22, name = "Bob", class = 4, account = 2 })
local a2 = player({ guid = 21, name = "Alice", class = 8, account = 1, dead = true })
bgEvents[2](2, arena({ a2, b2 }), 4, 901, 0)
check("arena: the living one wins, whichever comes first", executed[2] and executed[2]:find("22, 'Bob', 4, 21, 'Alice', 8%)"), executed[2])
executed = {}
check("arena: both alive is no clear winner", bgEvents[2](2, arena({ a, b2 }), 4, 902, 0) == "no clear winner")
check("arena: the same account is not counted", bgEvents[2](2, arena({ a, player({ guid = 23, name = "Alt", account = 1, dead = true }) }), 4, 903, 0) == "same account")
check("arena: a bot is not counted", bgEvents[2](2, arena({ a, player({ guid = 24, name = "Botb", accname = "RNDBOT3", account = 9, dead = true }) }), 4, 904, 0) == "a bot")
check("arena: a GM is not counted", bgEvents[2](2, arena({ player({ guid = 25, name = "Gm", account = 5, gm = true }), b }), 4, 905, 0) == "a GM")
check("arena: a spectator doesn't count as a player", bgEvents[2](2, arena({ a, b, player({ guid = 26, name = "Watcher", account = 6, spect = true }) }), 4, 906, 0) == nil)
check("arena: three players is not a 1v1", bgEvents[2](2, arena({ a, b, b2 }), 4, 907, 0) == "not two players")
local n = #executed
bgEvents[2](2, arena({ a, b }, { per = 2 }), 4, 908, 0)
bgEvents[2](2, arena({ a, b }, { arena = false }), 4, 909, 0)
check("arena: a 2v2 or a battleground: nothing", #executed == n)
executed = {}
bgEvents[2](2, arena({ a, player({ guid = 27, name = "O'Neil", account = 7, dead = true }) }), 4, 910, 0)
check("arena: a quote in a name is escaped", executed[1] and executed[1]:find("'O''Neil'", 1, true), executed[1])

-- .boards
local p = player({ guid = 1, name = "Orion" })
check(".boards: answered and swallowed", events[42](42, p, ".boards") == false)
check(".boards: the featured fight and the site", p.said[1]:find("The Frozen Throne", 1, true) and p.said[1]:find("leylinelauncher.com/wrath/boards", 1, true), p.said[1])
check(".board and .leaderboards too", events[42](42, p, "board") == false and events[42](42, p, "leaderboards") == false)
check("other commands pass", events[42](42, p, "duel top") == nil)
check("still never reads a table", #queried == 0)

-- a Boss Rush realm's rotation
B = load({ OV_BOARDS = "1", OV_BOARDS_FIGHTS = "frozen-throne,bone-lord,cult-mistress" })
check("rotation: three fights, one a week", B.featured(clock)[1] ~= B.featured(clock + 7 * 86400)[1]
    and B.featured(clock)[1] == B.featured(clock + 21 * 86400)[1])

print(string.format("%d passed, %d failed", pass, fail))
if fail > 0 then os.exit(1) end
