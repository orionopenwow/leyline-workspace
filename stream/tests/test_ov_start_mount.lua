-- lua5.1 stream/tests/test_ov_start_mount.lua realm/custom/lua_scripts/ov_main.lua
-- Riding from level 1 on the main realm (ov_main.lua, "riding from level 1"; owner 2026-11-03: "Start level 1s with
-- initial ground mounts and riding"): on the first login every race gets Apprentice Riding and its own basic ground
-- mount as spells (no bag item), one chat line, a log line; Death Knights, bots, OV_MAIN off and OV_START_MOUNT=0 get
-- nothing; a second login learns nothing and says nothing; a character made before (below 20) is caught up quietly at
-- login, one at 20 or above is left to the trainer; .mount casts the best mount the character knows (a normal cast),
-- gets down when mounted, and is left to the core on other realms.
local mainFile = assert(arg[1], "usage: lua5.1 test_ov_start_mount.lua <ov_main.lua>")

local pass, fail = 0, 0
local realPrint = print
local function check(name, ok, extra)
    if ok then pass = pass + 1; realPrint("PASS " .. name)
    else fail = fail + 1; realPrint("FAIL " .. name .. (extra ~= nil and ("  " .. tostring(extra)) or "")) end
end
local logged = {}
print = function(...)
    local t = {}
    for i = 1, select("#", ...) do t[#t + 1] = tostring(select(i, ...)) end
    logged[#logged + 1] = table.concat(t, " ")
end

-- ---- fake mod-ale ------------------------------------------------------------------------------
local ENV = { OV_MAIN = "1", OV_START_KIT = "0" }
local realGetenv = os.getenv
os.getenv = function(k) if ENV[k] ~= nil then return ENV[k] or nil end return realGetenv(k) end
local events = {}
function RegisterPlayerEvent(e, f) events[e] = events[e] or {}; table.insert(events[e], f) end
function RegisterPlayerGossipEvent() end
function CharDBExecute() end
function CharDBQuery(sql)
    if sql:find("FROM ov_boosts", 1, true) then
        return { GetUInt32 = function() return 0 end, NextRow = function() return false end }
    end
    return nil
end
function WorldDBQuery() return nil end
function SendMail() end

local function newPlayer(guid, race, class, level, opts)
    opts = opts or {}
    local p = { guid = guid, race = race, class = class or 1, level = level or 1, spells = {}, learns = {},
        timers = {}, said = {}, casts = {}, mounted = false, menus = 0 }
    for _, s in ipairs(opts.spells or {}) do p.spells[s] = true end
    function p:GetGUIDLow() return self.guid end
    function p:GetAccountId() return 7 end
    function p:GetAccountName() return opts.account or "PLAYER" end
    function p:GetName() return "Rider" .. self.guid end
    function p:GetClass() return self.class end
    function p:GetRace() return self.race end
    function p:GetLevel() return self.level end
    function p:IsBot() return opts.bot == true end
    function p:HasSpell(s) return self.spells[s] == true end
    function p:LearnSpell(s) self.spells[s] = true; self.learns[#self.learns + 1] = s end
    function p:HasSkill() return false end
    function p:SendBroadcastMessage(t) self.said[#self.said + 1] = t end
    function p:RegisterEvent(fn, ms) self.timers[#self.timers + 1] = fn end
    function p:GossipClearMenu() end
    function p:GossipMenuAddItem() end
    function p:GossipSendMenu() self.menus = self.menus + 1 end
    function p:GossipComplete() end
    function p:GetEquippedItemBySlot() return nil end
    function p:HasItem() return true end
    function p:IsMounted() return self.mounted end
    function p:Dismount() self.mounted = false; self.dismounted = true end
    function p:CastSpell(target, spell, triggered)
        self.casts[#self.casts + 1] = { target = target, spell = spell, triggered = triggered }
        self.mounted = true
    end
    return p
end
local function fire(e, ...) local r for _, f in ipairs(events[e] or {}) do r = f(e, ...) end return r end
local function runTimers(p) local t = p.timers; p.timers = {}; for _, fn in ipairs(t) do fn(nil, nil, nil, p) end end
-- a new character, as the core does it: ON_LOGIN first, then ON_FIRST_LOGIN (CharacterHandler.cpp)
local function firstLogin(p) fire(3, p); fire(30, p); runTimers(p) end
local function login(p) fire(3, p); runTimers(p) end
local function command(p, text) return fire(42, p, text) end
local function joined(p) return table.concat(p.said, "\n") end

OV_KITS = {}
dofile(mainFile)
local T = assert(OV_MAIN_TEST, "OV_MAIN_TEST")
local RIDING = 33388

-- 1. the data: the ten races of the game, each with its level-20 vendor mount's spell
local EXPECT = { [1] = 458, [2] = 580, [3] = 6899, [4] = 10793, [5] = 17464, [6] = 18990, [7] = 10873, [8] = 8395,
    [10] = 34795, [11] = 34406 }
local n = 0
for race, spell in pairs(T.START_MOUNT) do
    n = n + 1
    check("race " .. race .. ": mount spell " .. spell, EXPECT[race] == spell, EXPECT[race])
end
check("ten races, no more", n == 10, n)
check("Apprentice Riding is 33388", T.APPRENTICE_RIDING == RIDING)

-- 2. first login, every race: riding and its own mount, as spells, one line, a log line
local guid = 100
for race, spell in pairs(EXPECT) do
    guid = guid + 1
    local p = newPlayer(guid, race, race == 10 and 8 or 1)   -- a blood elf mage, everyone else a warrior
    local logs = #logged
    firstLogin(p)
    check("race " .. race .. ": Apprentice Riding at level 1", p.spells[RIDING] == true)
    check("race " .. race .. ": its own mount at level 1", p.spells[spell] == true)
    check("race " .. race .. ": nothing else of the kind learned", #p.learns == 2, #p.learns)
    local lines = 0
    for _, s in ipairs(p.said) do if s:find("ride from level 1", 1, true) then lines = lines + 1 end end
    check("race " .. race .. ": told once, with .mount", lines == 1 and joined(p):find(".mount", 1, true) ~= nil, joined(p))
    local found = false
    for i = logs + 1, #logged do if logged[i]:find("[ov_main] start mount:", 1, true) then found = true end end
    check("race " .. race .. ": logged", found)
end

-- 3. once only: the next login learns nothing and says nothing
local h = newPlayer(200, 1, 1)
firstLogin(h)
local learned, said = #h.learns, #h.said
login(h); login(h)
check("relog: nothing learned again", #h.learns == learned, #h.learns)
check("relog: nothing said again", #h.said == said, joined(h))
fire(30, h); runTimers(h)
check("a stray second first-login event: nothing learned twice", #h.learns == learned)

-- 4. who gets nothing
local dk = newPlayer(201, 1, 6, 55)
firstLogin(dk)
check("Death Knight: no riding or race mount (the Ebon Hold story has its own)", #dk.learns == 0 and not dk.spells[RIDING])
local bot = newPlayer(202, 2, 1, 1, { bot = true })
firstLogin(bot)
check("bot: nothing", #bot.learns == 0)
local rnd = newPlayer(203, 2, 1, 1, { account = "RNDBOT0042" })
firstLogin(rnd)
check("random bot account: nothing", #rnd.learns == 0)
ENV.OV_START_MOUNT = "0"
local offp = newPlayer(204, 1, 1)
firstLogin(offp)
check("OV_START_MOUNT=0: nothing", #offp.learns == 0 and not joined(offp):find("ride from level 1", 1, true))
ENV.OV_START_MOUNT = nil
ENV.OV_MAIN = "0"
local other = newPlayer(205, 1, 1)
firstLogin(other)
check("another realm (OV_MAIN off): nothing", #other.learns == 0)
check("another realm: .mount left to the core", command(other, ".mount") == nil and #other.casts == 0)
ENV.OV_MAIN = "1"

-- 5. a character made before: caught up quietly below 20, left to the trainer from 20
local old = newPlayer(206, 3, 1, 12)
login(old)
check("made before, level 12: riding and the ram at login", old.spells[RIDING] and old.spells[6899])
check("made before: quietly (no first-login line)", not joined(old):find("ride from level 1", 1, true), joined(old))
local old20 = newPlayer(207, 3, 1, 20)
login(old20)
check("made before, level 20: left to the riding trainer", #old20.learns == 0)
local half = newPlayer(208, 4, 1, 5, { spells = { RIDING } })
login(half)
check("already has riding: only the mount is learned", #half.learns == 1 and half.learns[1] == 10793)

-- 6. .mount
local r = newPlayer(209, 7, 1)
firstLogin(r)
check(".mount is handled (no 'no such command')", command(r, ".mount") == false)
check(".mount: a normal (not triggered) cast of the gnome's mechanostrider on itself",
    #r.casts == 1 and r.casts[1].spell == 10873 and r.casts[1].triggered == false and r.casts[1].target == r)
command(r, "mount")
check(".mount when mounted: gets down, no new cast", r.dismounted == true and #r.casts == 1)
local boosted = newPlayer(210, 1, 1, 80, { spells = { RIDING, 458, T.MOUNTS[1], T.SMART_MOUNT } })
command(boosted, ".mount")
check(".mount after a boost: the Celestial Steed (rides and flies)", boosted.casts[1] and boosted.casts[1].spell == T.SMART_MOUNT)
local dkm = newPlayer(211, 2, 6, 58, { spells = { T.DK_MOUNT } })
command(dkm, ".mount")
check(".mount, Death Knight: the Deathcharger", dkm.casts[1] and dkm.casts[1].spell == T.DK_MOUNT)
local late = newPlayer(212, 5, 1, 7)
command(late, ".mount")
check(".mount, level 7 made before: learns and casts the skeletal horse", late.spells[RIDING] and late.casts[1]
    and late.casts[1].spell == 17464)
local walker = newPlayer(213, 5, 1, 30)
command(walker, ".mount")
check(".mount at 30 without riding: told where riding comes from", #walker.casts == 0
    and joined(walker):find("riding trainer", 1, true) ~= nil, joined(walker))

-- 7. player-facing words
local all = {}
for _, p in ipairs({ h, old, r, walker }) do all[#all + 1] = joined(p) end
local text = table.concat(all, "\n")
for _, word in ipairs({ "World of Warcraft", "Warcraft", "WoW", "Lich King", "Blizzard" }) do
    check("no '" .. word .. "' in what players read", not text:find(word, 1, true))
end

print = realPrint
realPrint(string.format("%d passed, %d failed", pass, fail))
os.exit(fail == 0 and 0 or 1)
