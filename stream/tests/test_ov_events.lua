-- ov_events.lua (the events clock: stock events announced, trivia, hide and seek, the weekly board) under a fake mod-ale.
--   lua5.1 stream/tests/test_ov_events.lua realm/custom/lua_scripts/ov_events.lua
local file = assert(arg[1], "usage: lua5.1 test_ov_events.lua <ov_events.lua>")

local pass, fail = 0, 0
local function check(name, ok, extra)
    if ok then pass = pass + 1; print("PASS " .. name)
    else fail = fail + 1; print("FAIL " .. name .. (extra and ("  " .. tostring(extra)) or "")) end
end

-- a fixed clock: Saturday 2026-10-31 13:00:00 server time
local clock = os.time({ year = 2026, month = 10, day = 31, hour = 13, min = 0, sec = 0 })
local realTime = os.time
os.time = function(t) if t then return realTime(t) end return clock end
local env = {}
os.getenv = function(k) return env[k] end
math.random = function(n) return n and 1 or 0.5 end   -- predictable: the first spot, the first card left

local function rows(list)
    if #list == 0 then return nil end
    local i, q = 1, {}
    function q:GetString(c) return tostring(list[i][c + 1]) end
    function q:GetUInt32(c) return tonumber(list[i][c + 1]) end
    function q:NextRow() i = i + 1; return i <= #list end
    return q
end

-- the world (game_event) and characters (ov_event_wins) databases
local GAME_EVENT = {
    -- id, start, end, occurence (min), length (min), holiday, description
    { 16, realTime({ year = 2010, month = 1, day = 1, hour = 0 }), realTime({ year = 2030, month = 12, day = 31, hour = 0 }), 180, 120, 0, "Gurubashi Arena Booty Run" },
    { 63, realTime({ year = 2010, month = 1, day = 2, hour = 14 }), realTime({ year = 2030, month = 12, day = 31, hour = 0 }), 10080, 60, 0, "Kalu'ak Fishing Derby" },
    { 12, realTime({ year = 2010, month = 10, day = 18, hour = 2 }), realTime({ year = 2030, month = 12, day = 31, hour = 0 }), 525600, 20160, 324, "Hallow's End" },
    { 99, realTime({ year = 2010, month = 1, day = 1, hour = 0 }), realTime({ year = 2030, month = 12, day = 31, hour = 0 }), 60, 5, 0, "Some internal thing" },
    { 77, realTime({ year = 2010, month = 1, day = 1, hour = 0 }), realTime({ year = 2030, month = 12, day = 31, hour = 0 }), 525600, 60, 999, "A Warcraft brand thing" },
}
function WorldDBQuery(sql)
    if sql:find("FROM game_event", 1, true) then return rows(GAME_EVENT) end
    return nil
end
local wins = {}   -- week|guid|kind -> { name, wins }
local charSql = {}
function CharDBExecute(sql)
    charSql[#charSql + 1] = sql
    local wk, guid, name, kind = sql:match("VALUES %('([^']+)', (%d+), '([^']*)', '(%a+)', 1%)")
    if wk then
        local k = wk .. "|" .. guid .. "|" .. kind
        wins[k] = wins[k] or { name = name, wins = 0, guid = tonumber(guid), week = wk }
        wins[k].wins = wins[k].wins + 1
    end
end
function CharDBQuery(sql)
    local wk = sql:match("week = '([^']+)'")
    if not wk then return nil end
    local by = {}
    for _, v in pairs(wins) do
        if v.week == wk then by[v.guid] = by[v.guid] or { v.name, 0 }; by[v.guid][2] = by[v.guid][2] + v.wins end
    end
    local list = {}
    for _, v in pairs(by) do list[#list + 1] = v end
    table.sort(list, function(a, b) return a[2] > b[2] end)
    return rows(list)
end

local events, server, gossip, timers, spawns = {}, {}, {}, {}, {}
function RegisterPlayerEvent(id, fn) events[id] = fn end
function RegisterServerEvent(id, fn) server[id] = fn end
function RegisterCreatureGossipEvent(entry, ev, fn) gossip[entry] = fn end
function CreateLuaEvent(fn, ms, rep) timers[#timers + 1] = { fn = fn, ms = ms }; return #timers end
local hiderGuid = "GUID-HIDER"
function PerformIngameSpawn(kind, entry, map, inst, x, y, z, o, save, dur)
    local c = { entry = entry, x = x, y = y, z = z, flags = {} }
    function c:GetGUID() return hiderGuid end
    function c:SetFaction(f) self.flags.faction = f end
    function c:SetNPCFlags(f) self.flags.npc = f end
    spawns[#spawns + 1] = { kind = kind, entry = entry, map = map, x = x, y = y, z = z, save = save, dur = dur, c = c }
    return c
end

local online = {}
function GetPlayersInWorld() return online end
local all = {}
local function player(o)
    local p = { msgs = {}, guid = 1, name = "Alice", gm = 0, map = 571, zone = 4395, x = 0, y = 0, z = 0, acc = "ALICE" }
    for k, v in pairs(o or {}) do p[k] = v end
    function p:SendBroadcastMessage(m) self.msgs[#self.msgs + 1] = m; all[#all + 1] = m end
    function p:GetGUIDLow() return self.guid end
    function p:GetName() return self.name end
    function p:GetAccountName() return self.acc end
    function p:GetGMRank() return self.gm end
    function p:GetMapId() return self.map end
    function p:GetZoneId() return self.zone end
    function p:IsDead() return false end
    function p:GetX() return self.x end
    function p:GetY() return self.y end
    function p:GetZ() return self.z end
    function p:GetO() return 1 end
    function p:GetDistance(x, y, z) return math.sqrt((self.x - x) ^ 2 + (self.y - y) ^ 2 + (self.z - z) ^ 2) end
    function p:GossipComplete() self.closed = true end
    return p
end
local function plain(m) return (m or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", "") end
local function last(p) return plain(p.msgs[#p.msgs]) end
local function cmd(p, c) return events[42](42, p, c) end
local function has(p, s) for _, m in ipairs(p.msgs) do if plain(m):find(s, 1, true) then return true end end return false end

-- off by default ---------------------------------------------------------------------------------------------------------
dofile(file)
local E = OV_EVENTS_TEST
check("off unless OV_EVENTS=1: no timers, no hooks but the command", not E.ON and #timers == 0 and not server[34] and not events[18] and events[42])
local q = player()
check("off: .events says so", cmd(q, "events") == false and last(q):find("off on this realm", 1, true))
check("off: .ans is not ours", cmd(q, "ans 8") == nil)

-- on, as main.env sets it -----------------------------------------------------------------------------------------------
env = { OV_EVENTS = "1", OV_EVENTS_STOCK = "1", OV_EVENTS_TRIVIA = "1", OV_EVENTS_HIDE = "1" }
events, server, gossip, timers = {}, {}, {}, {}
OV_EXTRA_TIPS = nil
dofile(file)
E = OV_EVENTS_TEST
check("on: the schedule and the fast tick", #timers == 3 and timers[1].ms == 30000 and timers[2].ms == 2000)
check("on: hooks (game event start, chat, command, the hider's gossip)", server[34] and events[18] and events[42] and gossip[721])
check("a login tip for ov_qol.lua", OV_EXTRA_TIPS and OV_EXTRA_TIPS[1].text:find(".events", 1, true))
check("stock: the arena chest and the derby, our own names", #E.stock == 2 and E.stock[1].name:find("arena chest", 1, true)
    and not E.stock[1].name:find("Gurubashi", 1, true))
check("stock: the internal event left out", (function() for _, s in ipairs(E.stock) do if s.id == 99 then return false end end return true end)())
check("holidays: by description, brand-checked", E.holiday[12] == "Hallow's End" and E.holiday[77] == nil)
check("next start: every 3 h from midnight", E.nextStart(E.stock[1], clock + 1) == clock + 2 * 3600)   -- 15:00
check("next start of the derby: Saturday 14:00", E.nextStart(E.stock[2], clock) == clock + 3600)

local up = E.upcoming(clock)
check("upcoming: sorted", (function() for i = 2, #up do if up[i].start < up[i - 1].start then return false end end return #up >= 4 end)())
check("trivia next at 14:15 (even hours)", (function() for _, e in ipairs(up) do if e.kind == "trivia" then return e.start == clock + 3600 + 900 end end end)())
check("hide and seek next at 13:45 (odd hours)", (function() for _, e in ipairs(up) do if e.kind == "hide" then return e.start == clock + 2700 end end end)())

-- .events --------------------------------------------------------------------------------------------------------------
local alice = player()
online = { alice }
cmd(alice, "events")
check(".events: the next three, plain", last(alice):find("Next: Hide and seek in Dalaran in 45 min", 1, true) and last(alice):find(".events top", 1, true))

-- "starting soon": 5 minutes ahead, once, rate-limited ----------------------------------------------------------------------
clock = clock + 40 * 60 + 5     -- 13:40:05
E.tick()
check("hide and seek: 'starts in 5 min' once", has(alice, "Hide and seek in Dalaran starts in 5 min."))
local n = #alice.msgs
E.tick()
check("... not twice", #alice.msgs == n)
-- the derby at 14:00 is due at 13:55 too, but announcements keep 2 minutes apart
clock = clock + 15 * 60 + 25    -- 13:55:30
E.tick()
check("the derby: 'starts in 5 min'", has(alice, "Kalu'ak Fishing Derby: catch the most, turn in at Dalaran starts in 5 min."))

-- bots don't count and never get anything
local bot = player({ guid = 99, name = "Botty", acc = "RNDBOT0001" })
online = { alice, bot }

-- hide and seek at 13:45 ------------------------------------------------------------------------------------------------------
clock = realTime({ year = 2026, month = 10, day = 31, hour = 13, min = 45, sec = 10 })
E.tick()
check("hide and seek started at its time", E.active and E.active.kind == "hide" and has(alice, "a rabbit is hiding"))
check("bots hear nothing", #bot.msgs == 0)
local sp = spawns[#spawns]
check("the hider: an unsaved, timed spawn in Dalaran at a spot", sp and sp.kind == 1 and sp.entry == 721 and sp.map == 571 and sp.save == false
    and sp.dur == E.HIDE_S * 1000 and sp.x == E.SPOTS[1][1])
check("the hider: friendly and talkable", sp.c.flags.faction == 35 and sp.c.flags.npc == 1)
E.tick()
check("one event at a time (the clock doesn't start it twice)", #spawns == 1)
clock = clock + E.HINT1_S
E.fast()
check("first hint after 3 min", has(alice, "Hide and seek hint: " .. E.SPOTS[1][5]))
-- a bot walks right up to it: no win
bot.x, bot.y, bot.z = E.SPOTS[1][1], E.SPOTS[1][2], E.SPOTS[1][3]
E.fast()
check("a bot at the spot never wins", E.active ~= nil)
alice.x, alice.y, alice.z = E.SPOTS[1][1] + 2, E.SPOTS[1][2], E.SPOTS[1][3]
E.fast()
check("Alice walks up to it: she wins", E.active == nil and has(alice, "Alice found it!"))
check("a win on this week's board", wins[os.date("%Y-%W", clock) .. "|1|hide"] and wins[os.date("%Y-%W", clock) .. "|1|hide"].wins == 1)
check("after an event: the host-your-own line, once a day", has(alice, "Host your own world: /host"))

-- trivia (GM start) -------------------------------------------------------------------------------------------------------------
local gmp = player({ guid = 7, name = "Staff", gm = 2, acc = "STAFF" })
local bob = player({ guid = 2, name = "Bob", acc = "BOB" })
online = { alice, bob, gmp, bot }
check("players can't start events", cmd(alice, "events trivia") == false and E.active == nil)
cmd(gmp, "events trivia")
check("GM: trivia starts", E.active and E.active.kind == "trivia" and has(bob, "Trivia starts now"))
clock = clock + 6; E.fast()
local q1 = E.active.q
check("question 1 asked", q1 and has(bob, "Question 1 of 5: " .. q1.q))
cmd(bob, "ans something wrong")
check("a wrong answer: nothing", E.active.open)
clock = clock + 1
cmd(alice, "ans " .. q1.a[1]:upper() .. "!")
check("first right answer scores (case and punctuation don't matter)", not E.active.open and has(bob, "Alice got it"))
for i = 2, 5 do
    clock = clock + E.PAUSE_S + 1; E.fast()
    local qi = E.active.q
    if i == 3 then
        clock = clock + E.QUESTION_S + 1; E.fast()
        check("time's up: the answer is shown", has(bob, "Time's up. It was: " .. qi.a[1]))
    else
        clock = clock + 3
        events[18](18, bob, "#ans " .. qi.a[1], 1, 0)   -- the familiar way, said in chat
    end
end
check("#ans in chat counts", E.active.score[2] and E.active.score[2].points == 3)
clock = clock + E.PAUSE_S + 1; E.fast()
check("trivia over: Bob wins 3 to 1", E.active == nil and has(alice, "Bob wins with 3 right!"))
check("the host line not again the same day", (function() local c = 0; for _, m in ipairs(alice.msgs) do if plain(m):find("/host", 1, true) then c = c + 1 end end return c == 1 end)())

-- the board -----------------------------------------------------------------------------------------------------------------
cmd(alice, "events top")
check(".events top: this week's wins", last(alice):find("This week's event wins (bragging rights): ", 1, true)
    and last(alice):find("Alice 1", 1, true) and last(alice):find("Bob 1", 1, true))
check("the SQL is escaped", (function()
    local evil = player({ guid = 5, name = "O'Brien", acc = "X" })
    E.found(evil)   -- no event: ignored
    E.active = { kind = "hide", spot = E.SPOTS[2], started = clock, hints = 0 }
    E.found(evil)
    return charSql[#charSql]:find("'O''Brien'", 1, true) ~= nil
end)())

-- mute, rate limits, holidays ------------------------------------------------------------------------------------------------------
cmd(bob, "events off")
local before = #bob.msgs
check("muted: an announcement doesn't reach Bob", E.broadcast("x", "event") and #bob.msgs == before)
cmd(bob, "events on")
E.sent = {}
check("rate limit: two announcements 2 min apart at most", E.broadcast("a", "announce") and not E.broadcast("b", "announce"))
clock = clock + 121
check("... then again", E.broadcast("c", "announce"))
E.sent = {}
for i = 1, 10 do clock = clock + 121; E.broadcast("x" .. i, "announce") end
clock = clock + 121
check("... and 10 an hour at most", not E.broadcast("eleventh", "announce"))
clock = clock + 3600; E.sent = {}
server[34](34, 12)
check("a holiday start is announced", has(alice, "Hallow's End has begun."))
local m0 = #alice.msgs
server[34](34, 99)
check("other game events are not", #alice.msgs == m0)

-- GM hide here, with a hint ------------------------------------------------------------------------------------------------------
E.active = nil
gmp.x, gmp.y, gmp.z = 5800, 600, 650
cmd(gmp, "events hide here By the big fountain")
check("GM hides it where they stand, with their hint", E.active and E.active.kind == "hide" and spawns[#spawns].x == 5800
    and E.active.spot[5] == "By the big fountain")
online = { alice }
alice.x, alice.y = 0, 0
check("talking to it wins (gossip)", gossip[721](1, alice, spawns[#spawns].c) == false and E.active == nil and alice.closed)
gmp.zone, gmp.map = 1, 1
cmd(gmp, "events hide here")
check("GM outside Dalaran: refused", E.active == nil and last(gmp):find("Stand in Dalaran", 1, true))
cmd(gmp, "events trivia"); cmd(gmp, "events stop")
check("GM stop", E.active == nil and has(alice, "stopped by staff"))

-- no players: our own events don't run (the stock ones are still announced) ---------------------------------------------------------
online = {}
local t0 = realTime({ year = 2026, month = 11, day = 1, hour = 14, min = 15, sec = 5 })
clock = t0; E.tick()
check("nobody online: trivia doesn't start", E.active == nil)

-- content checks ------------------------------------------------------------------------------------------------------------------
check("our own question set: 30 or more, each with an answer", #E.QUESTIONS >= 30 and (function()
    for _, x in ipairs(E.QUESTIONS) do if type(x.q) ~= "string" or #x.a == 0 then return false end end return true end)())
check("every answer matches itself", (function() for _, x in ipairs(E.QUESTIONS) do if not E.rightAnswer(x, x.a[1]) then return false end end return true end)())
check("answers forgive 'the', case and spaces", E.rightAnswer({ a = { "full moon" } }, "  The FULL   moon ") and not E.rightAnswer({ a = { "ice" } }, "rice"))
check("hide spots: 12 or more in Dalaran, each with two hints", #E.SPOTS >= 12 and (function()
    for _, s in ipairs(E.SPOTS) do
        if not (s[1] > 5600 and s[1] < 6000 and s[2] > 400 and s[2] < 800 and s[3] > 630 and s[3] < 670 and s[5] and s[6]) then return false end
    end
    return true
end)())
local text = { E.HOST_LINE }
for _, x in ipairs(E.QUESTIONS) do text[#text + 1] = x.q; for _, a in ipairs(x.a) do text[#text + 1] = a end end
for _, s in ipairs(E.SPOTS) do text[#text + 1] = s[5]; text[#text + 1] = s[6] end
for _, nm in pairs(E.STOCK_NAMES) do text[#text + 1] = nm end
for _, m in ipairs(all) do text[#text + 1] = plain(m) end
check("no brand words anywhere players read", (function()
    for _, t in ipairs(text) do
        local l = t:lower()
        if l:find("warcraft") or l:find("blizzard") or l:find("lich king") or l:find("%f[%a]wow%f[%A]") then return false, t end
    end
    return true
end)())
check("no gold or item prizes: the script never gives money, items or mail", (function()
    local f = assert(io.open(file, "rb")); local src = f:read("*a"); f:close()
    return not (src:find("AddItem", 1, true) or src:find("ModifyMoney", 1, true) or src:find("SendMail", 1, true)
        or src:find("AddTitle", 1, true) or src:find("SetKnownTitle", 1, true))
end)())

print(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
