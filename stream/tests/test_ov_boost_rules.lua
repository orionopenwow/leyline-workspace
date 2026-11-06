-- lua5.1 stream/tests/test_ov_boost_rules.lua realm/custom/lua_scripts/ov_main.lua realm/custom/lua_scripts/ov_kits.lua
-- The boost to 80's rules (ov_main.lua doBoost / onSelect / .boost) under a fake mod-ale, with the real spec kits:
-- once per character, the account's kit cap (boosts past it level and teach but give no items), deleted characters
-- still count toward the cap, the level limits (below 10; Death Knights below 58), no boost in combat, dead, or from
-- a battleground or arena, and gossip picks a client could forge (a secondary profession, a third primary, a
-- profession the character already has, "boost now" with no spec picked). (Review 4 Nov; texlua works too.)
local mainFile = assert(arg[1], "usage: lua5.1 test_ov_boost_rules.lua <ov_main.lua> [ov_kits.lua]")
local kitsFile = arg[2] or mainFile:gsub("ov_main%.lua$", "ov_kits.lua")

local pass, fail = 0, 0
local realPrint = print
local function check(name, ok, extra)
    if ok then pass = pass + 1; realPrint("PASS " .. name)
    else fail = fail + 1; realPrint("FAIL " .. name .. (extra ~= nil and ("  " .. tostring(extra)) or "")) end
end
print = function() end
newproxy = newproxy or function() return io.stdout end   -- Lua 5.2+ (texlua): any userdata will do

-- ---- fake mod-ale ------------------------------------------------------------------------------
local realGetenv = os.getenv
local env = { OV_MAIN = "1", OV_START_KIT = "0", OV_START_MOUNT = "0", OV_BOOST_KITS_PER_ACCOUNT = "2" }
os.getenv = function(k) if env[k] ~= nil then return env[k] end return realGetenv(k) end
local events, gossip = {}, nil
function RegisterPlayerEvent(e, f) events[e] = events[e] or {}; table.insert(events[e], f) end
function RegisterPlayerGossipEvent(menu, e, f) gossip = f end

local boosts = {}     -- char guid -> account   (characters.ov_boosts)
local choices = {}
function CharDBExecute(sql)
    local g, a = sql:match("^INSERT IGNORE INTO ov_boosts %(char_guid, account_id, boosted_at%) VALUES %((%d+), (%d+),")
    if g then if not boosts[tonumber(g)] then boosts[tonumber(g)] = tonumber(a) end return end
    local guid, choice = sql:match("^INSERT INTO ov_main_choice %(.-%) VALUES %((%d+), %d+, '(%w+)'")
    if guid then choices[tonumber(guid)] = choice end
end
local function row(v) return { GetUInt32 = function() return tonumber(v) or 0 end, GetString = function() return tostring(v) end,
    NextRow = function() return false end } end
function CharDBQuery(sql)
    local g = sql:match("SELECT choice FROM ov_main_choice WHERE guid = (%d+)")
    if g then return choices[tonumber(g)] and row(choices[tonumber(g)]) or nil end
    g = sql:match("FROM ov_boosts WHERE char_guid = (%d+)")
    if g then return row(boosts[tonumber(g)] and 1 or 0) end
    local a = sql:match("FROM ov_boosts WHERE account_id = (%d+)")
    if a then
        local n = 0
        for _, acc in pairs(boosts) do if acc == tonumber(a) then n = n + 1 end end
        return row(n)
    end
    return nil
end
function WorldDBQuery() return nil end
local mails = 0
function SendMail() mails = mails + 1 end

-- A player whose every unknown method is a harmless no-op (doBoost's steps call many), with the ones the rules read.
local function newPlayer(guid, opts)
    opts = opts or {}
    local p = { guid = guid, account = opts.account or 7, class = opts.class or 1, level = opts.level or 1,
        race = opts.race or 1, combat = opts.combat, dead = opts.dead, bg = opts.bg, arena = opts.arena,
        skills = {}, spells = {}, said = {}, items = {}, added = 0, teleports = 0, timers = {} }
    for _, s in ipairs(opts.skills or {}) do p.skills[s] = true end
    local m = {}
    function m:GetGUIDLow() return self.guid end
    function m:GetAccountId() return self.account end
    function m:GetName() return "Hero" .. self.guid end
    function m:GetClass() return self.class end
    function m:GetRace() return self.race end
    function m:GetLevel() return self.level end
    function m:SetLevel(l) self.level = l end
    function m:GetTeam() return 0 end
    function m:IsInCombat() return self.combat == true end
    function m:IsDead() return self.dead == true end
    function m:InBattleground() return self.bg == true end
    function m:InArena() return self.arena == true end
    function m:HasSkill(s) return self.skills[s] == true end
    function m:HasSpell(s) return self.spells[s] == true end
    function m:LearnSpell(s) self.spells[s] = true end
    function m:SetSkill(s) self.skills[s] = true end
    function m:AddItem() self.added = self.added + 1; return newproxy(false) end
    function m:EquipItem() return nil end
    function m:GetEquippedItemBySlot() return nil end
    function m:Teleport() self.teleports = self.teleports + 1 end
    function m:GossipClearMenu() self.items = {} end
    function m:GossipMenuAddItem(_, text, _, intid) self.items[#self.items + 1] = { text = text, intid = intid } end
    function m:GossipSendMenu() end
    function m:GossipComplete() end
    function m:SendBroadcastMessage(t) self.said[#self.said + 1] = t end
    function m:RegisterEvent(fn) self.timers[#self.timers + 1] = fn end
    function m:IsBot() return false end
    setmetatable(p, { __index = function(_, k) return m[k] or function() return nil end end })
    return p
end
local function fire(e, ...) local r for _, f in ipairs(events[e] or {}) do r = f(e, ...) end return r end
local function pick(p, intid) return gossip(2, p, p, 0, intid) end
local function last(p) return p.said[#p.said] or "" end
local function boosted(p) return p.level == 80 end

dofile(kitsFile)
dofile(mainFile)
check("the real kits loaded (Warrior: Arms, Fury, Protection)", OV_KITS and OV_KITS.order and #OV_KITS.order[1] == 3)

-- 1. a warrior boosts as Arms with no professions: level 80, Dalaran, kit items, recorded
local a = newPlayer(201)
fire(30, a)
pick(a, 2)                                  -- Boost: the spec menu
check("the spec menu lists the class's specs", #a.items == 4 and a.items[1].intid == 11, #a.items)
pick(a, 11)                                 -- Arms: the professions menu
local profIds = {}
for _, it in ipairs(a.items) do profIds[#profIds + 1] = it.intid end
check("the professions menu: 11 primaries and 'boost now'", #a.items == 12 and a.items[#a.items].intid == 999, #a.items)
pick(a, 999)
check("boost now: level 80", boosted(a))
check("boost now: in Dalaran", a.teleports == 1)
check("boost now: kit items given", a.added > 20, a.added)
check("boost now: recorded in ov_boosts", boosts[201] == 7)

-- 2. once per character: the menu, .boost and a forged pick all refuse
local before = a.added
a.level = 1                                 -- even if a GM sets it back to level 1
check(".boost on a boosted character: refused", fire(42, a, "boost") == false and last(a):find("already been boosted", 1, true))
pick(a, 11); pick(a, 999)
check("a forged pick on a boosted character gives nothing", a.added == before and a.teleports == 1)

-- 3. the account's kit cap (2 here): the third boost levels and teaches, no items
local b = newPlayer(202); fire(30, b); pick(b, 2); pick(b, 12); pick(b, 999)
check("second boost on the account: items", boosted(b) and b.added > 20)
local c = newPlayer(203); fire(30, c); pick(c, 2); pick(c, 13); pick(c, 999)
check("third boost (cap 2): level 80 all the same", boosted(c))
check("third boost: no kit items", c.added == 0, c.added)
check("third boost: told why", (table.concat(c.said, "\n")):find("had its 2 boost kits", 1, true) ~= nil)

-- 4. deleting a boosted character doesn't give its kit back
fire(2, 202)
local d = newPlayer(204); fire(30, d); pick(d, 2); pick(d, 11); pick(d, 999)
check("after a delete, the cap still counts the deleted character", boosted(d) and d.added == 0, d.added)

-- 5. level limits
local l10 = newPlayer(301, { account = 8, level = 10 })
check(".boost at level 10: refused", fire(42, l10, "boost") == false and last(l10):find("below level 10", 1, true))
local l9 = newPlayer(302, { account = 8, level = 9 })
fire(42, l9, "boost")
check(".boost at level 9: the spec menu", #l9.items == 4)
local dk57 = newPlayer(303, { account = 8, class = 6, level = 57 })
fire(42, dk57, "boost")
check(".boost on a level 57 Death Knight: the spec menu", #dk57.items == 4)
local dk58 = newPlayer(304, { account = 8, class = 6, level = 58 })
check(".boost on a level 58 Death Knight: refused", fire(42, dk58, "boost") == false and last(dk58):find("below 58", 1, true))

-- 6. not in combat, not dead, not from a battleground or arena (a level 55-57 Death Knight can be in one)
local function tryBoost(guid, opts)
    local p = newPlayer(guid, opts); fire(42, p, "boost"); pick(p, 11); pick(p, 999); return p
end
local inCombat = tryBoost(401, { account = 9, combat = true })
check("in combat: no boost", not boosted(inCombat) and last(inCombat):find("combat", 1, true) and not boosts[401])
local dead = tryBoost(402, { account = 9, dead = true })
check("dead: no boost (a ghost would land in Dalaran without its body)", not boosted(dead) and not boosts[402], last(dead))
local bg = tryBoost(403, { account = 9, class = 6, level = 55, bg = true })
check("in a battleground: no boost (it would teleport out mid-match)", not boosted(bg) and not boosts[403], last(bg))
local arena = tryBoost(404, { account = 9, class = 6, level = 56, arena = true })
check("in an arena: no boost", not boosted(arena) and not boosts[404], last(arena))
check("refused boosts don't use a kit or the character's one boost", tryBoost(401, { account = 9 }).level == 80)

-- 7. forged professions picks
local f = newPlayer(501, { account = 10, skills = { 186 } })   -- already a miner
fire(42, f, "boost"); pick(f, 11)
local hasMining = false
for _, it in ipairs(f.items) do if it.intid == 1186 then hasMining = true end end
check("a profession the character has isn't offered again", not hasMining)
pick(f, 1185)                                -- Cooking (secondary): not a primary pick
pick(f, 1186)                                -- Mining again
pick(f, 1171)                                -- Alchemy: the second primary -> boosts (two known)
check("forged secondary / known picks ignored; the second primary boosts", boosted(f))
check("the boost taught Alchemy and kept Mining", f.spells[51304] == true and f.spells[50310] == true)
local f3 = newPlayer(502, { account = 10 })
fire(42, f3, "boost"); pick(f3, 11)
pick(f3, 1171); pick(f3, 1164)
check("two primaries picked: the boost runs at once (no third)", boosted(f3) and not f3.spells[51313])
local f4 = newPlayer(503, { account = 10 })
fire(30, f4)
pick(f4, 999)
check("'boost now' with no spec picked: nothing happens", not boosted(f4) and not boosts[503])
pick(f4, 50)
check("a spec number past the class's list: nothing happens", not boosted(f4) and not boosts[503])

-- 8. OV_MAIN off: .boost is left to the core, no menu
env.OV_MAIN = "0"
local off = newPlayer(601, { account = 11 })
check("OV_MAIN=0: .boost not handled here", fire(42, off, "boost") == nil and #off.items == 0)
env.OV_MAIN = "1"

realPrint(string.format("%d passed, %d failed", pass, fail))
os.exit(fail == 0 and 0 or 1)
