-- ov_duels.lua (duel ratings, points, anti-farming, .duel commands) under a fake mod-ale.
--   lua5.1 stream/tests/test_ov_duels.lua realm/custom/lua_scripts/ov_duels.lua
local file = assert(arg[1], "usage: lua5.1 test_ov_duels.lua <ov_duels.lua>")

local pass, fail = 0, 0
local function check(name, ok, extra)
    if ok then pass = pass + 1; print("PASS " .. name)
    else fail = fail + 1; print("FAIL " .. name .. (extra and ("  " .. tostring(extra)) or "")) end
end

-- a fixed clock: Saturday 2026-10-31 13:00:00 UTC
local clock = os.time({ year = 2026, month = 10, day = 31, hour = 13, min = 0, sec = 0 })
local realTime = os.time
os.time = function(t) if t then return realTime(t) end return clock end
local env = {}
os.getenv = function(k) return env[k] end

local function rows(list)
    if not list or #list == 0 then return nil end
    local i, q = 1, {}
    function q:GetString(c) return tostring(list[i][c + 1]) end
    function q:GetUInt32(c) return tonumber(list[i][c + 1]) end
    function q:NextRow() i = i + 1; return i <= #list end
    return q
end

-- The characters database. CharDBExecute is queued like the real one (flush() runs the queue). Any query on a table
-- that doesn't exist yet aborts, as the core does (MySQL 1146), and any read of a table this script didn't make too.
local db = { tables = {}, stats = {}, week = {}, log = {} }
local queue, executed, queried, aborted = {}, {}, {}, nil
local function apply(sql)
    executed[#executed + 1] = sql
    local t = sql:match("^CREATE TABLE IF NOT EXISTS (%S+)")
    if t then db.tables[t] = true; return end
    local function need(name) if not db.tables[name] then aborted = aborted or ("write to missing " .. name) end end
    if sql:find("^INSERT INTO ov_duel_stats") then
        need("ov_duel_stats")
        local g, n, c, r, b, w, l, p, s, bs = sql:match("VALUES %((%d+), '(.-)', (%d+), (%d+), (%d+), (%d+), (%d+), (%d+), (%d+), (%d+), %d+%)")
        db.stats[tonumber(g)] = { name = n, class = tonumber(c), rating = tonumber(r), best = tonumber(b), wins = tonumber(w),
            losses = tonumber(l), points = tonumber(p), streak = tonumber(s), best_streak = tonumber(bs) }
    elseif sql:find("^INSERT INTO ov_duel_week") then
        need("ov_duel_week")
        local wk, g, n, c, p, w, l = sql:match("VALUES %('([^']+)', (%d+), '(.-)', (%d+), (%d+), (%d+), (%d+)%)")
        db.week[wk .. "|" .. g] = { week = wk, guid = tonumber(g), name = n, points = tonumber(p), wins = tonumber(w), losses = tonumber(l) }
    elseif sql:find("^INSERT INTO ov_duel_log") then
        need("ov_duel_log")
        local at, day, w, l, pair, kind, secs, rated, delta = sql:match(
            "VALUES %((%d+), '([^']+)', (%d+), (%d+), '([^']+)', (%d+), (%d+), (%d+), (%-?%d+)%)")
        db.log[#db.log + 1] = { day = day, pair = pair, kind = tonumber(kind), secs = tonumber(secs), rated = tonumber(rated), delta = tonumber(delta) }
    else
        aborted = aborted or ("unexpected write " .. sql)
    end
end
function CharDBExecute(sql) queue[#queue + 1] = sql end
local function flush() local q = queue; queue = {}; for _, s in ipairs(q) do apply(s) end end
local MINE = { ov_duel_stats = true, ov_duel_week = true, ov_duel_log = true }
function CharDBQuery(sql)
    queried[#queried + 1] = sql
    if sql:find("information_schema.TABLES", 1, true) then
        local n = 0
        for t in pairs(MINE) do if db.tables[t] then n = n + 1 end end
        return rows({ { n } })
    end
    local t = sql:match("FROM (%S+)")
    if not MINE[t] then aborted = aborted or ("read of a table not ours: " .. tostring(t)); return nil end
    if not db.tables[t] then aborted = aborted or ("read of missing " .. t); return nil end
    if t == "ov_duel_stats" then
        local g = sql:match("WHERE guid = (%d+)")
        if g then
            local s = db.stats[tonumber(g)]
            return s and rows({ { s.rating, s.best, s.wins, s.losses, s.points, s.streak, s.best_streak } })
        end
        local n = sql:match("WHERE name = '([^']*)'")
        if n then
            for _, s in pairs(db.stats) do
                if s.name:lower() == n:lower() then return rows({ { s.name, s.rating, s.best, s.wins, s.losses, s.best_streak } }) end
            end
            return nil
        end
        local min = tonumber(sql:match("wins %+ losses >= (%d+)"))
        local list = {}
        for _, s in pairs(db.stats) do if s.wins + s.losses >= min then list[#list + 1] = s end end
        table.sort(list, function(a, b) return a.rating > b.rating end)
        local out = {}
        for i = 1, math.min(5, #list) do out[i] = { list[i].name, list[i].rating, list[i].wins, list[i].losses } end
        return rows(out)
    elseif t == "ov_duel_week" then
        local wk, g = sql:match("week = '([^']+)' AND guid = (%d+)")
        if wk then
            local w = db.week[wk .. "|" .. g]
            return w and rows({ { w.points, w.wins, w.losses } })
        end
        wk = sql:match("week = '([^']+)'")
        local list = {}
        for _, w in pairs(db.week) do if w.week == wk then list[#list + 1] = w end end
        table.sort(list, function(a, b) return a.points > b.points end)
        local out = {}
        for i = 1, math.min(5, #list) do out[i] = { list[i].name, list[i].points, list[i].wins, list[i].losses } end
        return rows(out)
    elseif t == "ov_duel_log" then
        local pair, day = sql:match("pair = '([^']+)' AND day = '([^']+)'")
        local n = 0
        for _, e in ipairs(db.log) do if e.pair == pair and e.day == day and e.rated == 1 then n = n + 1 end end
        return rows({ { n } })
    end
end

local events = {}
function RegisterPlayerEvent(id, fn) events[id] = fn end
local online = {}
function GetPlayersInWorld() return online end
local function player(o)
    local p = { msgs = {}, guid = 1, name = "Alice", acct = 10, level = 80, class = 1, zone = 4395, gm = false, accName = "ALICE" }
    for k, v in pairs(o or {}) do p[k] = v end
    function p:SendBroadcastMessage(m) self.msgs[#self.msgs + 1] = m end
    function p:GetGUIDLow() return self.guid end
    function p:GetName() return self.name end
    function p:GetAccountId() return self.acct end
    function p:GetAccountName() return self.accName end
    function p:GetLevel() return self.level end
    function p:GetClass() return self.class end
    function p:GetZoneId() return self.zone end
    function p:IsGM() return self.gm end
    return p
end
local function plain(m) return (m or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", "") end
local function last(p) return plain(p.msgs[#p.msgs]) end
local function cmd(p, c) return events[42](42, p, c) end
local function duel(w, l, secs, kind)
    events[10](10, w, l)
    clock = clock + secs
    events[11](11, w, l, kind or 1)
    flush()
end

-- off by default ------------------------------------------------------------------------------------------------
dofile(file)
local D = OV_DUELS_TEST
check("off unless OV_DUELS=1: only the command hook, no tables", not D.ON and events[42] and not events[10] and not events[11] and #queue == 0)
local q = player()
check("off: .duel says so", cmd(q, "duel top") == false and last(q):find("off on this realm", 1, true))
check("off: other commands are not ours", cmd(q, "events") == nil)

-- on --------------------------------------------------------------------------------------------------------------
env = { OV_DUELS = "1" }
events = {}
OV_EXTRA_TIPS = nil
dofile(file)
D = OV_DUELS_TEST
check("on: duel start, duel end and command hooks", D.ON and events[10] and events[11] and events[42])
check("on: CREATE TABLE IF NOT EXISTS for all three tables, at load", #queue == 3 and queue[1]:find("^CREATE TABLE IF NOT EXISTS ov_duel_stats")
    and queue[2]:find("ov_duel_week") and queue[3]:find("ov_duel_log"))
check("a login tip for level 80s", OV_EXTRA_TIPS and OV_EXTRA_TIPS[1].min == 80 and OV_EXTRA_TIPS[1].text:find(".duel top", 1, true))

-- the queue hasn't run yet: a duel now must not read the tables
local alice = player()
local bob = player({ guid = 2, name = "Bob", acct = 20, class = 8 })
online = { alice, bob }
events[10](10, alice, bob); clock = clock + 40; events[11](11, alice, bob, 1)
check("before the tables exist: nothing read, the duel left uncounted", aborted == nil and D.ready == false
    and (function() for _, s in ipairs(queried) do if not s:find("information_schema", 1, true) then return false end end return true end)())
check("... and the tables are asked for again", #queue == 6)
flush()
check("tables made", db.tables.ov_duel_stats and db.tables.ov_duel_week and db.tables.ov_duel_log)

-- rating math -------------------------------------------------------------------------------------------------------
check("Elo: equal ratings, K 32: 16", D.delta(1500, 1500, 32) == 16)
check("Elo: beating a stronger player is worth more", D.delta(1400, 1600, 32) == 24 and D.delta(1600, 1400, 32) == 8)
check("Elo: never under 1, never over K", D.delta(3000, 100, 32) == 1 and D.delta(100, 3000, 32) == 32)
check("Elo: expected scores add up to 1", math.abs(D.expected(1550, 1480) + D.expected(1480, 1550) - 1) < 1e-9)
env.OV_DUELS_K = "20"
check("OV_DUELS_K sets the K-factor", D.delta(1500, 1500) == 10)
env.OV_DUELS_K = nil

-- a rated duel ----------------------------------------------------------------------------------------------------------
duel(alice, bob, 40)
check("a 40 s duel at 80: rated", aborted == nil and db.stats[1] and db.stats[1].rating == 1516 and db.stats[2].rating == 1484)
check("wins, losses, streaks", db.stats[1].wins == 1 and db.stats[1].streak == 1 and db.stats[2].losses == 1 and db.stats[2].streak == 0)
local wk = D.weekKey(clock)
check("weekly points: a win 3, a loss 1", db.week[wk .. "|1"].points == 3 and db.week[wk .. "|2"].points == 1 and db.stats[1].points == 3)
check("the week is Monday-based, UTC", wk == os.date("!%Y-%W", clock))
check("the log: rated, with the change", db.log[#db.log].rated == 1 and db.log[#db.log].delta == 16 and db.log[#db.log].secs == 40)
check("both are told", last(alice):find("You won: rating 1516 (+16), +3 points (3 this week).", 1, true)
    and last(bob):find("You lost: rating 1484 (-16), +1 point (1 this week).", 1, true))
check("names are escaped in SQL", (function()
    local evil = player({ guid = 9, name = "O'Neil", acct = 90 })
    D.record(evil, bob, 1, 40); flush()
    for _, s in ipairs(executed) do if s:find("'O''Neil'", 1, true) then return true end end
    return false
end)())
local before = db.stats[1].rating

-- anti-farming -----------------------------------------------------------------------------------------------------------
duel(alice, bob, 5)
check("under 15 s: no rating change", db.stats[1].rating == before and last(alice):find("too short", 1, true))
check("... but logged as unrated", db.log[#db.log].rated == 0)
duel(alice, bob, 20, 2)
check("fled under 30 s: nothing", db.stats[1].rating == before and last(bob):find("left the duel area", 1, true))
duel(alice, bob, 200, 0)
check("interrupted: nothing, whatever the length", db.stats[1].rating == before and last(alice):find("interrupted", 1, true))
local nlog = #db.log
check("interrupted duels aren't logged", db.log[nlog].kind ~= 0)
duel(alice, bob, 35, 2)
check("fled after 30 s: counts as a win", db.stats[1].rating > before and db.stats[1].wins == 2)

local alt = player({ guid = 3, name = "Altie", acct = 10 })
local r0 = db.stats[1].rating
duel(alice, alt, 60)
check("same account: nothing", db.stats[1].rating == r0 and not db.stats[3] and last(alt):find("one account", 1, true))

local low = player({ guid = 4, name = "Lowbie", acct = 40, level = 79 })
duel(alice, low, 60)
check("level 80 only", db.stats[1].rating == r0 and not db.stats[4] and last(low):find("level 80 only", 1, true))

local gm = player({ guid = 5, name = "Staff", acct = 50, gm = true })
duel(gm, alice, 60)
check("GM mode on: nothing", not db.stats[5] and last(gm):find("GM mode", 1, true))

local botty = player({ guid = 6, name = "Botty", acct = 60, accName = "RNDBOT0001" })
local m0 = #alice.msgs
duel(alice, botty, 60)
check("bots: nothing, not a word", not db.stats[6] and #alice.msgs == m0)

-- same pair: 3 rated duels a day (Alice and Bob already had 2 rated today)
duel(bob, alice, 40)
check("third rated duel of the pair today: counts", db.stats[2].wins == 1)
local rb = db.stats[2].rating
duel(bob, alice, 40)
check("fourth: no rating change", db.stats[2].rating == rb and last(bob):find("3 rated duels today", 1, true))
check("the pair count survives a reload (read from the log)", (function()
    D.pairs = {}
    return D.whyNot(bob, alice, 1, 40, clock) ~= nil
end)())
clock = realTime({ year = 2026, month = 11, day = 1, hour = 1, min = 0, sec = 0 })
duel(bob, alice, 40)
check("next day (UTC): counts again", db.stats[2].rating > rb)
check("no start seen (a reload mid-duel): uncounted", (function()
    local r = db.stats[2].rating
    events[11](11, bob, alice, 1); flush()
    return db.stats[2].rating == r and last(bob):find("started before", 1, true)
end)())

-- brackets when the minimum level is lower
env.OV_DUELS_MIN_LEVEL = "10"
local a70 = player({ guid = 11, name = "Seventy", acct = 110, level = 72 })
local b75 = player({ guid = 12, name = "Seventyf", acct = 120, level = 75 })
local c80 = player({ guid = 13, name = "Eighty", acct = 130, level = 80 })
duel(a70, b75, 30)
check("brackets: 72 vs 75 counts", db.stats[11] and db.stats[11].wins == 1)
duel(c80, b75, 30)
check("brackets: 80 vs 75 doesn't", not db.stats[13] and last(c80):find("brackets", 1, true))
env.OV_DUELS_MIN_LEVEL = nil

-- streaks ------------------------------------------------------------------------------------------------------------------
local carol = player({ guid = 21, name = "Carol", acct = 210 })
local others = {}
for i = 1, 6 do others[i] = player({ guid = 30 + i, name = "Foe" .. string.char(64 + i), acct = 300 + i }) end
online = { alice, bob, carol }
local far = player({ guid = 40, name = "Far", acct = 400, zone = 1 })
online[#online + 1] = far
for i = 1, 3 do clock = clock + 60; duel(carol, others[i], 30) end
check("3 in a row: a line to the zone", (function() for _, m in ipairs(alice.msgs) do if plain(m):find("Carol has won 3 duels in a row!", 1, true) then return true end end end)())
check("... not to other zones", (function() for _, m in ipairs(far.msgs) do if plain(m):find("in a row", 1, true) then return false end end return true end)())
cmd(bob, "duel quiet")
for i = 4, 5 do clock = clock + 60; duel(carol, others[i], 30) end
check("5 in a row: told, but not to a player who went quiet", (function()
    local a, b = false, false
    for _, m in ipairs(alice.msgs) do if plain(m):find("Carol has won 5 duels in a row!", 1, true) then a = true end end
    for _, m in ipairs(bob.msgs) do if plain(m):find("5 duels in a row", 1, true) then b = true end end
    return a and not b
end)())
check("best streak kept", db.stats[21].best_streak == 5)
clock = clock + 60; duel(others[6], carol, 30)
check("ending a streak of 5 is told", (function() for _, m in ipairs(alice.msgs) do if plain(m):find("FoeF ended Carol's streak of 5 wins!", 1, true) then return true end end end)()
    and db.stats[21].streak == 0 and db.stats[21].best_streak == 5)

-- commands ---------------------------------------------------------------------------------------------------------------------
check(".duel: help with the site", cmd(alice, "duel") == false and last(alice):find("play.openvanilla.app/wrath/leaderboards", 1, true))
cmd(alice, "duel stats")
check(".duel stats: your own", last(alice):find("^%[Duels%] Rating %d+ %(best %d+%)%. Wins%-losses %d+%-%d+%. Streak") ~= nil, last(alice))
cmd(alice, "duel stats carol")
check(".duel stats <name>", last(alice):find("Carol: rating", 1, true) and last(alice):find("Best streak 5", 1, true))
cmd(alice, "duel stats nobody")
check(".duel stats <unknown>", last(alice):find("nobody has no rated duels yet", 1, true))
cmd(alice, "duel stats x'; DROP TABLE")
check(".duel stats: names are letters only", last(alice):find("Type a character name", 1, true))
local newbie = player({ guid = 50, name = "Newbie", acct = 500 })
cmd(newbie, "duel stats")
check(".duel stats with none yet", last(newbie):find("No rated duels yet", 1, true))
cmd(alice, "duel top")
check(".duel top: this week's points", last(alice):find("This week's duel points: 1. Carol 16", 1, true), last(alice))   -- 5 wins and a loss
cmd(alice, "duel top all")
check(".duel top all: ratings, placed after 3 rated duels", last(alice):find("Top ratings: 1. ", 1, true)
    and not last(alice):find("Seventy ", 1, true))
check("commands with a dot or a bang", cmd(alice, ".duel top") == false and cmd(alice, "!duels") == false)

-- the rules for every read ------------------------------------------------------------------------------------------------------
check("never a read of a missing table or one not ours", aborted == nil, aborted)
check("the first query of all asks information_schema, not our tables", queried[1]:find("information_schema.TABLES", 1, true) ~= nil)
check("the script reads no other table (source check)", (function()
    local f = assert(io.open(file, "rb")); local src = f:read("*a"); f:close()
    for t in src:gmatch("FROM ([%w_%.]+)") do
        if t ~= "ov_duel_stats" and t ~= "ov_duel_week" and t ~= "ov_duel_log" and t ~= "information_schema.TABLES" then return false, t end
    end
    return true
end)())
check("no prizes: the script never gives money, items, mail or titles", (function()
    local f = assert(io.open(file, "rb")); local src = f:read("*a"); f:close()
    return not (src:find("AddItem", 1, true) or src:find("ModifyMoney", 1, true) or src:find("SendMail", 1, true)
        or src:find("SetKnownTitle", 1, true))
end)())
check("no brand words in anything players read", (function()
    local all = {}
    for _, p in ipairs({ alice, bob, carol, far, newbie, low, alt, gm, c80 }) do for _, m in ipairs(p.msgs) do all[#all + 1] = plain(m) end end
    for _, t in ipairs(all) do
        local l = t:lower()
        if l:find("warcraft") or l:find("blizzard") or l:find("lich king") or l:find("%f[%a]wow%f[%A]") then return false, t end
    end
    return #all > 20
end)())

print(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
