-- lua5.1 stream/tests/test_ov_dojo.lua [realm/custom/lua_scripts/ov_dojo.lua] [labs/den-lite/src/dojo.rs]
-- The Dojo on the full Den (ov_dojo.lua, OV_DEN=1) under a fake mod-ale and a stand-in for ov_den.lua's OVDen (its
-- roles, the edge, the floor, the GM controls, the stamps): the ring (the same layout as Den Lite's DOJO_STAMP),
-- the training dummies and the damage per second they report to the Den bar, a boss from the list (level, health,
-- the 10 minutes) and the best times kept per character, the duel ring's spars, fly and speed through the GM
-- controls (owner only for flying), a new look, a stamp, clear, who may do what, the panel's line, .den dojo through
-- OVDen.extra, and no game-brand names anywhere players read.
local dojoFile = arg[1] or "realm/custom/lua_scripts/ov_dojo.lua"
local rustFile = arg[2] or "labs/den-lite/src/dojo.rs"

local pass, fail = 0, 0
local realPrint = print
local function check(name, ok)
    if ok then pass = pass + 1; realPrint("PASS " .. name) else fail = fail + 1; realPrint("FAIL " .. name) end
end
print = function() end

local clock = 5000000
function GetCurrTime() return clock end
local env = {}
os.getenv = function(k) return env[k] end

-- ---------------------------------------------------------------- fake world
local events, creatureEvents, luaEvents = {}, {}, {}
function RegisterPlayerEvent(id, fn) events[id] = events[id] or {}; table.insert(events[id], fn) end
function RegisterCreatureEvent(entry, id, fn) creatureEvents[entry .. ":" .. id] = fn end
function CreateLuaEvent(fn, delay, rep) luaEvents[#luaEvents + 1] = { fn = fn, delay = delay, rep = rep } end
local function fire(id, ...) local r; for _, fn in ipairs(events[id] or {}) do local x = fn(id, ...); if x ~= nil then r = x end end; return r end
local function tick() for _, e in ipairs(luaEvents) do if e.delay == 1000 then e.fn() end end end

local objects, spawns, nextGuid = {}, {}, 100
local maps = {}
local function mapObj(id)
    maps[id] = maps[id] or { id = id, GetWorldObject = function(self, g) local o = objects[g]; if o and o.map == self.id then return o end end }
    return maps[id]
end
function GetMapById(id) return mapObj(id) end
function PerformIngameSpawn(kind, entry, map, inst, x, y, z, o, save, ms)
    if save then return nil end
    nextGuid = nextGuid + 1
    local s = { kind = kind, entry = entry, map = map, x = x, y = y, z = z, o = o, ms = ms, low = nextGuid, guid = 0xF13000000000 + nextGuid,
        level = 1, maxHealth = 1000, health = 1000 }
    function s:GetGUID() return self.guid end
    function s:GetGUIDLow() return self.low end
    function s:GetEntry() return self.entry end
    function s:SetLevel(l) self.level = l end
    function s:GetMaxHealth() return self.maxHealth end
    function s:SetMaxHealth(h) self.maxHealth = h end
    function s:SetHealth(h) self.health = h end
    function s:DespawnOrUnsummon() self.gone = true; objects[self.guid] = nil end
    function s:RemoveFromWorld() self.gone = true; objects[self.guid] = nil end
    objects[s.guid] = s
    spawns[#spawns + 1] = s
    return s
end

-- the characters database: mod_dojo_best as REPLACE INTO says, read back by SELECT
local sqlLog, bestRows = {}, {}
function CharDBExecute(sql)
    sqlLog[#sqlLog + 1] = sql
    local g, boss, ms = sql:match("^REPLACE INTO mod_dojo_best %(guid, boss, ms, at%) VALUES %((%d+), '(%w+)', (%d+), %d+%)")
    if g then bestRows[tonumber(g)] = bestRows[tonumber(g)] or {}; bestRows[tonumber(g)][boss] = tonumber(ms) end
end
function CharDBQuery(sql)
    local g = tonumber(sql:match("FROM mod_dojo_best WHERE guid = (%d+)") or "")
    if not g or not bestRows[g] then return nil end
    local rows = {}
    for b, ms in pairs(bestRows[g]) do rows[#rows + 1] = { b, ms } end
    if #rows == 0 then return nil end
    local r = { rows = rows, i = 1 }
    function r:GetString(c) return tostring(self.rows[self.i][c + 1]) end
    function r:GetUInt32(c) return tonumber(self.rows[self.i][c + 1]) end
    function r:NextRow() self.i = self.i + 1; return self.i <= #self.rows end
    return r
end

local allMsgs = {}
local function player(over)
    local p = { msgs = {}, addon = {}, map = 1, x = 16226.2, y = 16257.0, z = 12.6, o = 0, inside = true, roleName = "visitor",
        maxHealth = 5000, health = 5000 }
    for k, v in pairs(over or {}) do p[k] = v end
    function p:SendBroadcastMessage(m) self.msgs[#self.msgs + 1] = m; allMsgs[#allMsgs + 1] = m end
    function p:GetGUIDLow() return self.guid end
    function p:GetName() return self.name end
    function p:GetMapId() return self.map end
    function p:GetInstanceId() return 0 end
    function p:GetX() return self.x end
    function p:GetY() return self.y end
    function p:GetZ() return self.z end
    function p:GetO() return self.o end
    function p:ToPlayer() return self end
    function p:GetMaxHealth() return self.maxHealth end
    function p:SetHealth(h) self.health = h end
    function p:SetAtLoginFlag(f) self.atLogin = f end
    function p:SaveToDB() self.saved = true end
    return p
end
local function last(p) return p.msgs[#p.msgs] or "" end
local function said(p, pat) for _, m in ipairs(p.msgs) do if m:find(pat) then return true end end end
local function cmd(p, text) return fire(42, p, text) end
local function addonLast(p, name)
    for i = #p.addon, 1, -1 do local n, d = p.addon[i]:match("^wuden\t(%w+)\t(.*)$"); if n == name then return d end end
end

-- the Open Mod API: what is sent to whom
local handlers = {}
package.preload["ov_modapi"] = function()
    return {
        On = function(mod, name, fn) handlers[mod .. "/" .. name] = fn end,
        Send = function(pl, mod, name, data) pl.addon[#pl.addon + 1] = mod .. "\t" .. name .. "\t" .. data end,
    }
end

-- ov_den.lua's OVDen, as the Dojo uses it
local GROUND = 12.6
local gmCalls, stampCalls, prevHelp = {}, {}, {}
OVDen = {
    extra = { stamp = function(pl, w, text) stampCalls[#stampCalls + 1] = { pl = pl, w = w, text = text } end },
    help = function(pl, r) prevHelp[#prevHelp + 1] = r end,
    role = function(pl) return pl.roleName end,
    inside = function(pl) return pl.inside end,
    floorZ = function() return GROUND end,
    GM = { state = {}, rated = function() return true end },
}
function OVDen.GM.command(pl, w)
    gmCalls[#gmCalls + 1] = w
    local s = OVDen.GM.state[pl:GetGUIDLow()] or { speed = 1 }
    OVDen.GM.state[pl:GetGUIDLow()] = s
    if w[2] == "fly" then s.fly = w[3] == "on" elseif w[2] == "speed" then s.speed = tonumber(w[3]) end
end

-- ---------------------------------------------------------------- loading
dofile(dojoFile)
check("off without OV_DEN=1: nothing registered", OV_DOJO_TEST == nil and next(events) == nil)
env.OV_DEN = "1"
dofile(dojoFile)
local D = assert(OV_DOJO_TEST, "ov_dojo.lua loads with OV_DEN=1")
check("events: logout, duel start and end, commands", events[4] and events[10] and events[11] and events[42])
check("every dummy reports damage taken; every boss its fight and its death", (function()
    for _, d in ipairs(D.DUMMIES) do if not creatureEvents[d.entry .. ":9"] then return false end end
    for _, b in ipairs(D.BOSSES) do if not (creatureEvents[b.entry .. ":1"] and creatureEvents[b.entry .. ":4"]) then return false end end
    return true end)())
check("a tick every second", (function() for _, e in ipairs(luaEvents) do if e.delay == 1000 and e.rep == 0 then return true end end end)())
check("the table is made before anything reads it", sqlLog[1] and sqlLog[1]:find("^CREATE TABLE IF NOT EXISTS mod_dojo_best"))
check("the Den bar's Dojo panel can ask for its line", handlers["wuden/dojo"] ~= nil)
check(".den dojo goes to the Dojo (OVDen.extra.dojo), and OVDen.Dojo is here", type(OVDen.extra.dojo) == "function" and OVDen.Dojo == D)

-- the ring is Den Lite's DOJO_STAMP, piece for piece
check("the ring matches Den Lite's DOJO_STAMP (labs/den-lite/src/dojo.rs)", (function()
    local f = io.open(rustFile, "r")
    if not f then return false end
    local text = f:read("*a"); f:close()
    local stamp = text:match('DOJO_STAMP: &str = r#"(.-)"#;')
    if not stamp then return false end
    local rust = {}
    for e, x, y, yaw in stamp:gmatch('{"entry": (%d+), "x": ([%-%d%.]+), "y": ([%-%d%.]+), "z": 0, "yaw": (%d+)}') do
        rust[#rust + 1] = { tonumber(e), tonumber(x), tonumber(y), tonumber(yaw) }
    end
    if #rust ~= #D.RING or #rust < 20 then return false end
    for i, r in ipairs(rust) do
        local l = D.RING[i]
        for k = 1, 4 do if math.abs(r[k] - l[k]) > 1e-6 then return false end end
    end
    -- and the dummies' spots
    local line = text:match("pub const DUMMY_SPOTS: &%[%(f32, f32%)%] = &%[(.-)%];")
    if not line then return false end
    local spots = {}
    for x, y in line:gmatch("%(([%-%d%.]+), ([%-%d%.]+)%)") do spots[#spots + 1] = { tonumber(x), tonumber(y) } end
    if #spots ~= #D.DUMMY_SPOTS then return false end
    for i, sp in ipairs(spots) do
        if math.abs(sp[1] - D.DUMMY_SPOTS[i][1]) > 1e-6 or math.abs(sp[2] - D.DUMMY_SPOTS[i][2]) > 1e-6 then return false end
    end
    -- and how far ahead the ring's middle goes (dojo::reach of the stamp: its farthest piece and 1.5)
    local far = 0
    for _, r in ipairs(rust) do far = math.max(far, math.sqrt(r[2] * r[2] + r[3] * r[3])) end
    return math.abs(far + 1.5 - D.RING_REACH) < 0.01 end)())

-- ---------------------------------------------------------------- who may
local own = player({ guid = 1, name = "Owner", roleName = "owner" })
local bea = player({ guid = 2, name = "Bea", roleName = "builder", x = 16228, y = 16259 })
local vic = player({ guid = 3, name = "Vic" })
check(".dojo: what this is, for anyone", cmd(vic, ".dojo") == false and said(vic, "training grounds"))
check("another command isn't the Dojo's", cmd(vic, ".den home") == nil)
cmd(vic, ".dojo ring")
check("a visitor can't put the ring up", last(vic):find("owner and the builders") and #spawns == 0)
own.inside = false
cmd(own, ".dojo ring")
check("only inside the Den", last(own):find("Only inside the Den") and #spawns == 0)
own.inside = true

-- ---------------------------------------------------------------- the ring and the dummies
cmd(own, ".dojo ring")
check(".dojo ring: its pieces as gameobjects, not saved", #spawns == #D.RING and spawns[1].kind == 2 and last(own):find("ring is up %(22 pieces%)"))
check("the ring's middle 10.23 yards ahead: the first fence 6 further, on the ground", math.abs(spawns[1].x - (own.x + 16.23)) < 1e-3
    and math.abs(spawns[1].y - own.y) < 1e-3 and spawns[1].z == GROUND and spawns[1].entry == 19434)
check("the panel hears the ring", addonLast(own, "dojo") == "server=full;fly=0;speed=1;dummies=0;ring=1;boss=-;duel=0;best=-")
local ringN = #spawns
cmd(bea, ".dojo dummies")
local d1 = spawns[ringN + 1]
check(".dojo dummies (a builder): three of the game's training dummies", #spawns == ringN + 3 and d1.kind == 1 and d1.entry == 31144)
check("in the ring, 3 yards in from its middle, facing its opening", math.abs(d1.x - (own.x + 13.23)) < 1e-3 and math.abs(d1.y - own.y) < 1e-3
    and math.abs(d1.o - math.pi) < 1e-6 and d1.z == GROUND)
cmd(bea, ".dojo dummies 9 raider")
check("at most five; a kind by name; the old ones go first", #spawns == ringN + 8 and spawns[#spawns].entry == 31146 and d1.gone
    and last(bea):find("5 Raider's Training Dummies up"))
cmd(bea, ".dojo dummies 2 squishy")
check("an unknown kind says the kinds", last(bea):find("grandmaster %(80%)"))
cmd(own, ".dojo dummies 3")
local dummies = {}
for i = #spawns - 2, #spawns do dummies[#dummies + 1] = spawns[i] end

-- ---------------------------------------------------------------- damage per second
local onDamage = creatureEvents["31144:9"]
onDamage(9, dummies[1], own, 500)
check("the first hit: the pill hears it at once", addonLast(own, "dps") == "dps=500;total=500;secs=1.0;dummy=Grandmaster's Training Dummy")
clock = clock + 500
onDamage(9, dummies[2], own, 500)
check("at most once a second", addonLast(own, "dps") == "dps=500;total=500;secs=1.0;dummy=Grandmaster's Training Dummy")
clock = clock + 500
local pet = { GetOwner = function() return own end }
onDamage(9, dummies[1], pet, 1000)
check("a pet's damage is its owner's", addonLast(own, "dps") == "dps=2000;total=2000;secs=1.0;dummy=Grandmaster's Training Dummy")
clock = clock + 1000
onDamage(9, dummies[3], own, 1000)
check("a second later: over two seconds", addonLast(own, "dps") == "dps=1500;total=3000;secs=2.0;dummy=Grandmaster's Training Dummy")
local stranger = { GetGUIDLow = function() return 99999 end }
onDamage(9, stranger, own, 100000)
check("a dummy that isn't the Dojo's doesn't count", D.dps[1].total == 3000)
cmd(own, ".dojo dps")
check(".dojo dps", last(own):find("dps 1500, total 3000, secs 2.0"))
clock = clock + 7000
tick()
check("a pause ends the count: the last numbers, done", addonLast(own, "dps") == "dps=1500;total=3000;secs=2.0;dummy=Grandmaster's Training Dummy;done=1")
onDamage(9, dummies[1], own, 300)
check("the next hit starts a new count", D.dps[1].total == 300)

-- ---------------------------------------------------------------- bosses and best times
cmd(bea, ".dojo boss kazzak")
check("a boss is the owner's", last(bea):find("Only the Den's owner"))
cmd(own, ".dojo boss")
check(".dojo boss alone: the list", last(own):find("kazzak") and last(own):find("hogger"))
cmd(own, ".dojo boss kazzak 90")
check("a level from 1 to 83", last(own):find("1 to 83"))
cmd(own, ".dojo boss kazzak 80 300")
check("health 1 to 200 per cent", last(own):find("1 to 200"))
cmd(own, ".dojo boss kazzak 80 50")
local boss = spawns[#spawns]
check(".dojo boss kazzak 80 50: at level 80, half its health, gone in 10 minutes", boss.entry == 18728 and boss.kind == 1 and boss.level == 80
    and boss.maxHealth == 500 and boss.health == 500 and boss.ms == 600000 and last(own):find("Doom Lord Kazzak is here %(level 80, 50%% health%)"))
check("12 yards ahead, facing you", math.abs(boss.x - (own.x + 12)) < 1e-3 and math.abs(boss.o - math.pi) < 1e-6)
check("the panel: the boss", addonLast(own, "dojo"):find(";boss=kazzak;"))
creatureEvents["18728:1"](1, boss, own)
clock = clock + 42000
creatureEvents["18728:4"](4, boss, own)
check("killed: the time from the fight's start, the first best", last(own):find("Doom Lord Kazzak down in 42%.0 s%. Your first time")
    and bestRows[1] and bestRows[1].kazzak == 42000 and D.boss == nil)
check("the panel: the best", addonLast(own, "dojo"):find(";best=42%.0$"))
cmd(own, ".dojo boss kazzak")
local b2 = spawns[#spawns]
check("its own level and full health when not given", b2.level == 73 and b2.maxHealth == 1000)
creatureEvents["18728:1"](1, b2, own)
clock = clock + 30000
creatureEvents["18728:4"](4, b2, { GetOwner = function() return own end })
check("a better time: a new best (a pet's kill is its owner's)", last(own):find("A new best %(was 42%.0 s%)") and bestRows[1].kazzak == 30000)
cmd(own, ".dojo boss kazzak")
local b3 = spawns[#spawns]
local world = PerformIngameSpawn(1, 18728, 1, 0, 0, 0, 0, 0, false, 0)
creatureEvents["18728:4"](4, world, own)
check("a boss that isn't the Dojo's: nothing", D.boss ~= nil and D.boss.low == b3.low)
creatureEvents["18728:1"](1, b3, own)
clock = clock + 50000
creatureEvents["18728:4"](4, b3, own)
check("slower: your best stays", last(own):find("Your best is 30%.0 s") and bestRows[1].kazzak == 30000)
fire(4, own)
cmd(own, ".dojo best")
check(".dojo best: kept per character (read back after a logout)", last(own):find("Doom Lord Kazzak 30%.0 s"))
cmd(own, ".dojo boss hogger")
local b4 = spawns[#spawns]
clock = clock + 606000
tick()
check("after 10 minutes the boss goes", b4.gone and D.boss == nil)
cmd(own, ".dojo boss hogger")
local b5 = spawns[#spawns]
fire(4, own)
check("its owner logging out takes it away", b5.gone and D.boss == nil)

-- ---------------------------------------------------------------- spars
local fred = player({ guid = 4, name = "Fred", roleName = "builder" })
fire(10, own, fred)
own.health, fred.health = 100, 0
fire(11, own, fred, 1)
check("the duel ring off: a duel is just a duel", not said(fred, "spar") and fred.health == 0)
cmd(bea, ".dojo duel on")
check("the duel ring is the owner's", last(bea):find("Only the Den's owner"))
cmd(own, ".dojo duel on")
check(".dojo duel on", last(own):find("duel ring is on") and D.duel)
fire(10, own, fred)
check("a duel in the Den: a spar", said(fred, "A spar in the Dojo's ring: fight!"))
clock = clock + 12000
own.health, fred.health = 100, 0
fire(11, own, fred, 1)
check("its end: timed, both back to full", last(fred):find("Owner wins the spar in 12%.0 s") and own.health == 5000 and fred.health == 5000)
fred.inside = false
fire(10, own, fred)
check("not when one of them is outside the Den", not D.spars[1])
cmd(own, ".dojo duel off")

-- ---------------------------------------------------------------- fly, speed, look, build
cmd(bea, ".dojo fly on")
check("only the owner flies", last(bea):find("Only the Den's owner flies") and #gmCalls == 0)
cmd(own, ".dojo fly")
check(".dojo fly (the owner): the GM controls' fly, on", gmCalls[1] and gmCalls[1][2] == "fly" and gmCalls[1][3] == "on")
check("the panel: flying", addonLast(own, "dojo"):find("^server=full;fly=1;"))
cmd(own, ".dojo fly")
check("again: off", gmCalls[2][3] == "off")
cmd(bea, ".dojo speed 2")
check(".dojo speed 2 (builders too): the GM controls' speed", gmCalls[3][2] == "speed" and gmCalls[3][3] == "2")
cmd(vic, ".dojo look")
check(".dojo look: the game's customize screen at the character list", vic.atLogin == 8 and vic.saved and last(vic):find("character list"))
cmd(bea, ".dojo build garden")
check(".dojo build garden: the stamp at once, through .den stamp", stampCalls[1] and stampCalls[1].w[2] == "garden" and stampCalls[1].w[3] == "now")
cmd(bea, ".dojo build")
check("build what?", last(bea):find("Build what"))

-- ---------------------------------------------------------------- .den dojo, clear, help
OVDen.extra.dojo(own, { "dojo", "clear" })
check(".den dojo clear: the dummies and the ring go", dummies[1].gone and spawns[1].gone and last(own):find("Cleared: 3 dummies, the ring")
    and D.ring == nil)
check("the panel after clear", addonLast(own, "dojo") == "server=full;fly=0;speed=1;dummies=0;ring=0;boss=-;duel=0;best=30.0")
handlers["wuden/dojo"](own)
check("the panel's ask answers the line", addonLast(own, "dojo"):find("^server=full;"))
OVDen.help(own, "owner")
check(".den's help: the others' lines, then the Dojo's", prevHelp[1] == "owner" and last(own):find("The Dojo"))

-- ---------------------------------------------------------------- brand words
local brand = { "world of warcraft", "warcraft", "lich king", "blizzard" }
check("no game-brand names in anything said", (function()
    for _, m in ipairs(allMsgs) do
        local l = m:lower()
        for _, b in ipairs(brand) do if l:find(b, 1, true) then return false end end
        if (" " .. l .. " "):find("[^%a]wow[^%a]") then return false end
    end
    return #allMsgs > 30 end)())

realPrint(string.format("%d passed, %d failed", pass, fail))
if fail > 0 then os.exit(1) end
