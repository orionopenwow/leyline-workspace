-- ov_go.lua's .hub (the Dalaran spots: duel corner, 1v1 arena, transmog, keystone dungeons, how Hardcore starts) under
-- a fake mod-ale, and the NPC positions realm/serverpass.sh step 2b writes kept equal to the HUB table.
--   lua5.1 stream/tests/test_ov_go_hub.lua realm/custom/lua_scripts/ov_go.lua [realm/serverpass.sh] [ov_qol.lua]
local file = assert(arg[1], "usage: lua5.1 test_ov_go_hub.lua <ov_go.lua> [serverpass.sh] [ov_qol.lua]")
local events = {}
local clock = 1000000
os.time = function() return clock end
local env = {}
local realGetenv = os.getenv
os.getenv = function(k) if env[k] ~= nil then return env[k] end return nil end
function RegisterPlayerEvent(id, fn) events[id] = events[id] or {}; table.insert(events[id], fn) end
function AuthDBExecute() end
function AuthDBQuery() return nil end
function WorldDBQuery() return nil end
function GetPlayersInWorld() return {} end

dofile(file)
local T = assert(OV_GO_TEST)

local pass, fail = 0, 0
local function check(name, ok, extra)
    if ok then pass = pass + 1; print("PASS " .. name)
    else fail = fail + 1; print("FAIL " .. name .. (extra and ("  " .. tostring(extra)) or "")) end
end

local all = {}
local function player(over)
    local p = { msgs = {}, queue = {}, teleports = {}, zone = 4395, guid = 7, gm = 0 }
    for k, v in pairs(over or {}) do p[k] = v end
    function p:SendBroadcastMessage(m) self.msgs[#self.msgs + 1] = m; all[#all + 1] = m end
    function p:GetGUIDLow() return self.guid end
    function p:GetAccountId() return 42 end
    function p:GetLevel() return 80 end
    function p:GetGMRank() return self.gm end
    function p:GetZoneId() return self.zone end
    function p:GetAreaId() return self.area or self.zone end
    function p:GetMapId() return self.map or 0 end
    function p:GetX() return self.x end
    function p:GetY() return self.y end
    function p:GetZ() return self.z end
    function p:IsInCombat() return self.combat or false end
    function p:IsDead() return self.dead or false end
    function p:Teleport(...) self.teleports[#self.teleports + 1] = { ... }; return true end
    function p:RegisterEvent(fn, delay) self.queue[#self.queue + 1] = fn end
    return p
end
local function run(p) while #p.queue > 0 do table.remove(p.queue, 1)(1, 100, 0, p) end end
local function plain(m) return (m or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", "") end
local function last(p) return plain(p.msgs[#p.msgs]) end
local function cmd(p, c)
    local r
    for _, fn in ipairs(events[42]) do local x = fn(42, p, c, nil); if x == false then r = false end end
    return r
end

-- the table ---------------------------------------------------------------------------------------------------------
check("five spots in order", table.concat(T.HUB_ORDER, ",") == "duels,arena,transmog,keys,hardcore")
check("the duel corner is h:duels (Krasus' Landing)", T.HUB.duels.pos == T.PLACES["h:duels"].pos
    and math.abs(T.PLACES["h:duels"].pos[2] - 5813.84) < 0.01 and T.PLACES["h:duels"].pos[1] == 571)
for _, k in ipairs({ "arena", "transmog", "keys" }) do
    local h = T.HUB[k]
    check(k .. ": an NPC on map 571 near the city centre", h.npc and h.pos[1] == 571
        and math.abs(h.pos[2] - 5805) < 30 and h.pos[3] > 400 and h.pos[3] < 700)
end
check("hardcore has no spot (it starts at level 1), only the way", T.HUB.hardcore.pos == nil and T.HUB.hardcore.tip:find("level 1", 1, true))
check("the NPC entries from the modules' SQL", T.HUB.arena.npc == 999991 and T.HUB.transmog.npc == 190010 and T.HUB.keys.npc == 200005)

-- .hub alone, a spot, aliases -----------------------------------------------------------------------------------------
local p = player()
check(".hub is handled", cmd(p, "hub") == false)
check(".hub lists the five", last(p):find(".hub duels", 1, true) and last(p):find(".hub hardcore", 1, true)
    and last(p):find(".hub keys", 1, true) and not last(p):find(".qol hub", 1, true))
check(".hub transmog: handled, deferred", cmd(p, "hub transmog") == false and #p.teleports == 0 and #p.queue == 1)
run(p)
local t = p.teleports[1]
check("... then the hop to the Warpweaver's spot", t and t[1] == 571 and t[2] == T.HUB.transmog.pos[2] and t[3] == T.HUB.transmog.pos[3])
check("... and the tip", last(p):find("Warpweaver", 1, true))
check("cooldown: again at once is refused", cmd(p, "hub keys") == false and #p.queue == 0 and last(p):find("Again in 10s", 1, true))
clock = clock + 11
check("alias .hub 1v1 -> the arena", cmd(p, ".hub 1v1") == false and #p.queue == 1); run(p)
check("... the battlemaster's spot and .q1v1", p.teleports[2][2] == T.HUB.arena.pos[2] and last(p):find(".q1v1", 1, true))
clock = clock + 11
check(".wuhub keys works too", cmd(p, "wuhub keys") == false and #p.queue == 1); run(p)
check(".hub hardcore: the way, no teleport", cmd(p, "hub hardcore") == false and #p.queue == 0 and last(p):find("Shrine of Challenge", 1, true))
check(".hub something-else: the list (travel hubs off here)", cmd(p, "hub gurubashi") == false and last(p):find(".hub duels", 1, true))

-- refusals ------------------------------------------------------------------------------------------------------------
clock = clock + 11
-- in game (31 Oct): standing in Dalaran reported zone 2817 (Crystalsong Forest): the city's box counts
local floating = player({ zone = 2817, map = 571, x = 5807.9, y = 588.4, z = 660.9, guid = 31 })
cmd(floating, "hub duels"); run(floating)
check("in Dalaran but the zone says Crystalsong: still goes", #floating.teleports == 1)
local below = player({ zone = 2817, map = 571, x = 5807.9, y = 588.4, z = 140, guid = 32 })
cmd(below, "hub duels")
check("Crystalsong Forest below the city: told to come to the city", #below.teleports == 0 and last(below):find("Come to the city", 1, true))
local away = player({ zone = 1519, guid = 8 })
check("outside Dalaran: no hop, where it is", cmd(away, "hub duels") == false and #away.queue == 0
    and last(away):find("is in Dalaran", 1, true))
local busy = player({ combat = true, guid = 9 })
check("in combat: no hop", cmd(busy, "hub transmog") == false and #busy.queue == 0 and last(busy):find("not in combat", 1, true))
local dead = player({ dead = true, guid = 10 })
check("dead: no hop", cmd(dead, "hub transmog") == false and #dead.queue == 0 and last(dead):find("not while dead", 1, true))
check("other commands untouched", cmd(p, "go cancel") == false and cmd(p, "server info") == nil and cmd(p, "hubx") == nil)

-- serverpass.sh step 2b writes the same NPC positions ------------------------------------------------------------------
if arg[2] then
    local f = assert(io.open(arg[2], "rb")); local sp = f:read("*a"); f:close()
    for _, k in ipairs({ "arena", "transmog", "keys" }) do
        local h = T.HUB[k]
        local want = string.format('"%%d+ %d %s %s %s %s"', h.npc, tostring(h.pos[2]), tostring(h.pos[3]), tostring(h.pos[4]), tostring(h.pos[5]))
        check("serverpass.sh places " .. k .. " at the HUB position", sp:find(want:gsub("%.", "%%.")) ~= nil, want)
    end
    check("serverpass.sh: idempotent (deletes its own guids, checks the template)", sp:find("DELETE FROM creature WHERE guid IN ($OV_NPC_GUIDS)", 1, true)
        and sp:find("FROM creature_template t WHERE t.entry = $2", 1, true))
end

-- with ov_qol.lua loaded too (Brawl, travel hubs on): no double answers ---------------------------------------------------
if arg[3] then
    events = {}
    env = { OV_BRAWL = "1" }
    function CharDBQuery() return nil end
    function CharDBExecute() end
    function WorldDBExecute() end
    function GetGameTime() return clock end
    function CreateLuaEvent() return 1 end
    dofile(file)
    local okq = pcall(dofile, arg[3])
    check("ov_qol.lua loads beside it", okq)
    if okq then
        local b = player({ guid = 20 })
        cmd(b, "hub")
        check("Brawl: .hub alone is one answer, naming .qol hub", #b.msgs == 1 and plain(b.msgs[1]):find(".qol hub", 1, true), #b.msgs)
        b.msgs = {}
        cmd(b, "hub transmog")
        check("Brawl: .hub transmog is ov_go's alone", #b.msgs == 0 and #b.queue == 1, #b.msgs)
        b.queue = {}
        cmd(b, "hub gurubashi")
        check("Brawl: .hub gurubashi is ov_qol's alone (no Dalaran list)", #b.msgs >= 1 and not plain(b.msgs[1]):find(".hub duels", 1, true))
    end
end

local clean = true
for _, m in ipairs(all) do
    local l = plain(m):lower()
    if l:find("warcraft") or l:find("blizzard") or l:find("lich king") or l:find("%f[%a]wow%f[%A]") then clean = false end
end
check("no brand words in any message", clean)
os.getenv = realGetenv
print(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
