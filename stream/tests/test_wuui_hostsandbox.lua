-- lua5.1 stream/tests/test_wuui_hostsandbox.lua [lua_scripts dir]
-- The GM command center's sandbox controls in realm/custom/lua_scripts/ov_hostctl.lua (the launcher's ui/gmcenter.js,
-- host/console.rs sandbox_data), under a fake mod-ale: only through the console's queue (never the in-game window's
-- channel); fixed verbs only; the GM who is online when no name is given; a named character only when online;
-- teleport presets by name only; searches with letters only and at most 15 rows; spawns beside the character, tracked,
-- and Clear takes them back; level through the realm's own command; fly, speed, weather, the time packet; kill only a
-- targeted creature; revive and heal; and a player who is not the GM refused in game.
local scripts = arg[1] or "realm/custom/lua_scripts"

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- a fake mod-ale
local tmp = os.tmpname()
local hostFile = tmp .. ".dat"
local function write(path, text) local f = assert(io.open(path, "wb")); f:write(text); f:close() end
write(hostFile, 'return { realm_id = "abcdefghijklmnop", visibility = "private", signup = "invite", bots = 0 }\n')
local realGetenv = os.getenv
os.getenv = function(k) if k == "OV_HOST_FILE" then return hostFile end return realGetenv(k) end
local serverClock = 1000000
os.time = function() return serverClock end

local addonHandlers, playerEvents, luaEvents = {}, {}, {}
function RegisterServerEvent(id, fn) if id == 30 then addonHandlers[#addonHandlers + 1] = fn end end
function RegisterPlayerEvent(id, fn) playerEvents[id] = playerEvents[id] or {}; table.insert(playerEvents[id], fn) end
function CreateLuaEvent(fn, ms, times) luaEvents[#luaEvents + 1] = { fn = fn, ms = ms, times = times } end

local world, commands, packets = {}, {}, {}
local mapObjects, weatherSet = {}, nil
local map = {}
function map:SetWeather(zone, kind, grade) weatherSet = { zone, kind, grade } end
function map:GetWorldObject(g) return mapObjects[g] end

local nextGuid = 100
local function newObject(kind, entry)
    nextGuid = nextGuid + 1
    local o = { kind = kind, entry = entry, guid = nextGuid, gone = false }
    function o:GetGUID() return self.guid end
    function o:DespawnOrUnsummon() self.gone = true; mapObjects[self.guid] = nil end
    function o:Despawn() self.gone = true; mapObjects[self.guid] = nil; return true end
    mapObjects[o.guid] = o
    return o
end

local function fakePlayer(name, gm, guid)
    local p = { name = name, gm = gm, guid = guid, chat = {}, speeds = {}, items = {}, dead = false, health = 10, fly = nil }
    function p:GetName() return self.name end
    function p:GetGMRank() return self.gm end
    function p:GetGUIDLow() return self.guid end
    function p:GetAccountId() return self.guid * 10 end
    function p:SendBroadcastMessage(m) self.chat[#self.chat + 1] = m end
    function p:GetMap() return map end
    function p:GetZoneId() return 4395 end
    function p:GetX() return 10 end
    function p:GetY() return 20 end
    function p:GetZ() return 30 end
    function p:GetO() return 0 end
    function p:SetSpeed(kind, rate) self.speeds[kind] = rate end
    function p:SetCanFly(on) self.fly = on end
    function p:AddItem(entry, count) if self.full then return nil end self.items[#self.items + 1] = entry; return {} end
    function p:IsDead() return self.dead end
    function p:ResurrectPlayer() self.dead = false end
    function p:GetMaxHealth() return 100 end
    function p:SetHealth(h) self.health = h end
    function p:GetSelection() return self.target end
    function p:Kill(t) t.dead = true end
    function p:SendPacket(pk) packets[#packets + 1] = { to = self.name, pk = pk } end
    function p:SpawnCreature(entry, x, y, z, o, kind, ms) self.spawn = { entry, x, y, z, o, kind, ms }; return newObject("creature", entry) end
    function p:SummonGameObject(entry, x, y, z, o, secs) self.summon = { entry, secs }; return newObject("object", entry) end
    function p:RegisterEvent(fn) fn(nil, nil, nil, self) end
    function p:KickPlayer() self.kicked = true end
    function p:SendAddonMessage() end
    world[#world + 1] = p
    return p
end
local friend = fakePlayer("Friendly", 0, 2)
local host = fakePlayer("Hosty", 3, 1)
function GetPlayersInWorld() return world end
function GetPlayerByName(n) for _, p in ipairs(world) do if p.name:lower() == n:lower() then return p end end end
function RunCommand(c) commands[#commands + 1] = c end
function SendWorldMessage() end
function GetRealmID() return 1 end
function AuthDBQuery() return nil end
function CreatePacket(op, size)
    local pk = { op = op, size = size, data = {} }
    function pk:WriteULong(v) self.data[#self.data + 1] = v end
    function pk:WriteFloat(v) self.data[#self.data + 1] = v end
    return pk
end

local function rows(list)
    local i = 1
    return { GetString = function(_, c) return list[i][c + 1] end, NextRow = function() i = i + 1; return list[i] ~= nil end }
end
local worldSql = {}
function WorldDBQuery(q)
    worldSql[#worldSql + 1] = q
    if q:find("FROM creature_template WHERE name LIKE '%%Guard%%'") then
        local l = {}
        for i = 1, 20 do l[i] = { tostring(1000 + i), "Guard " .. i .. "|cff" } end
        return rows(l)
    end
    if q:find("FROM creature_template WHERE entry = 68$") then return rows({ { "City Guard" } }) end
    if q:find("FROM gameobject_template WHERE entry = 500 ") then return rows({ { "Chair" } }) end
    if q:find("FROM item_template WHERE entry = 6948$") then return rows({ { "Hearthstone" } }) end
    return nil
end

-- the queue in the characters database: rows the console added, answers written back
local queue, answers = {}, {}
local function hex(s) return (s:gsub(".", function(c) return string.format("%02X", c:byte()) end)) end
local function unhex(h) return (h:gsub("%x%x", function(x) return string.char(tonumber(x, 16)) end)) end
function CharDBQuery(q)
    if q:find("^SELECT id, action, HEX%(data%), at FROM ov_host_queue WHERE done = 0") then
        local l = {}
        for _, r in ipairs(queue) do if not r.done then l[#l + 1] = { tostring(r.id), r.action, hex(r.data), tostring(r.at) } end end
        if #l == 0 then return nil end
        return rows(l)
    end
    return nil
end
function CharDBExecute(q)
    local h, id = q:match("^UPDATE ov_host_queue SET done = 1, result = UNHEX%('(%x*)'%) WHERE id = (%d+)$")
    if id then
        for _, r in ipairs(queue) do if r.id == tonumber(id) then r.done = true end end
        answers[tonumber(id)] = unhex(h)
    end
end

package.preload["ov_modapi"] = function() return dofile(scripts .. "/ov_modapi.lua") end
dofile(scripts .. "/ov_modapi.lua")
dofile(scripts .. "/ov_hostctl.lua")
local HC = assert(OVHostCtl)
os.getenv = realGetenv

local nextId = 0
local function console(data)
    serverClock = serverClock + 3   -- past the console's one action every 2 seconds
    nextId = nextId + 1
    queue[#queue + 1] = { id = nextId, action = "sandbox", data = data, at = serverClock }
    HC.RunQueue()
    return answers[nextId] or "none"
end

-- ---------------------------------------------------------------- the checks
check("the console's queue polls on a host kit's realm", #luaEvents == 1)
check("the verbs, all of them", table.concat(HC.SANDBOX_VERBS, ",") ==
    "clear,find-creature,find-item,find-object,fly,heal,item,kill,level,revive,spawn-creature,spawn-object,speed,tele,time,weather")

check("teleport: the GM online when no name is given", console("tele\tdalaran\t") == "ok:Hosty is on the way to Dalaran."
    and commands[#commands] == "tele name Hosty Dalaran")
check("teleport: a named friend", console("tele\tshattrath\tfriendly") == "ok:Friendly is on the way to Shattrath."
    and commands[#commands] == "tele name Friendly Shattrath")
local before = #commands
check("teleport: presets only", console("tele\tDalaran; .server shutdown\t"):sub(1, 4) == "err:" and #commands == before)
check("a name not online is said", console("tele\tdalaran\tNobody") == "err:Nobody isn't online.")
check("a name that is not a name is refused", console("tele\tdalaran\tRob') DROP"):sub(1, 4) == "err:")
check("an unknown verb is refused", console("run\t.server shutdown\t") == "err:That is not a sandbox control.")

check("level through the realm's own command", console("level\t80\t") == "ok:Hosty is level 80."
    and commands[#commands] == "character level Hosty 80")
check("level out of range refused", console("level\t81\t"):sub(1, 4) == "err:")
check("speed on run, run back, swim and flight", console("speed\t4\t") == "ok:Hosty moves at 4x."
    and host.speeds[1] == 4 and host.speeds[2] == 4 and host.speeds[3] == 4 and host.speeds[6] == 4)
check("speed out of range refused", console("speed\t9\t"):sub(1, 4) == "err:" and host.speeds[1] == 4)
check("fly on and off", console("fly\ton\t"):sub(1, 3) == "ok:" and host.fly == true
    and console("fly\toff\t"):sub(1, 3) == "ok:" and host.fly == false)
check("weather where the character stands", console("weather\tsnow\t"):sub(1, 3) == "ok:"
    and weatherSet[1] == 4395 and weatherSet[2] == 2)
check("weather: the four kinds only", console("weather\tfog\t"):sub(1, 4) == "err:")
local was = #packets
check("time of day: one packet to everyone online", console("time\t22\t"):find("^ok:It is 22:00") ~= nil and #packets == was + #world
    and packets[#packets].pk.op == 0x042 and packets[#packets].pk.data[1] % 2048 == 22 * 64)
check("time of day: 0 to 23 only", console("time\t24\t"):sub(1, 4) == "err:")

check("revive a dead character", (function() host.dead = true; return console("revive\t\t") end)() == "ok:Hosty is back on their feet."
    and host.dead == false)
check("heal to full", console("heal\t\t"):sub(1, 3) == "ok:" and host.health == 100)
check("kill: nothing targeted is refused", console("kill\t\t"):sub(1, 4) == "err:")
local wolf = { dead = false }
function wolf:GetTypeId() return 3 end
function wolf:IsDead() return self.dead end
function wolf:GetName() return "Wolf" end
host.target = wolf
check("kill: the targeted creature", console("kill\t\t") == "ok:Wolf is dead." and wolf.dead)
host.target = friend
function friend:GetTypeId() return 4 end
check("kill: never a player", console("kill\t\t"):sub(1, 4) == "err:")
host.target = nil

local found = console("find-creature\tGuard\t")
local n = select(2, found:gsub("\n", "")) + 1
check("search: 15 rows at most, entry and name, no colour codes", found:sub(1, 3) == "ok:" and n == 15
    and found:find("1001\tGuard 1 cff") ~= nil and not found:find("|", 1, true))
check("search: letters only reach the SQL", console("find-creature\tGu'a;rd\t"):sub(1, 3) == "ok:"
    and worldSql[#worldSql]:find("LIKE '%%Guard%%'") ~= nil)
check("search: too short refused", console("find-item\tx\t") == "err:Type 2 to 30 letters to search for.")
check("search: objects of safe kinds only", (function() console("find-object\tchair\t"); return worldSql[#worldSql] end)()
    :find("type IN %(0, 5, 7, 8%) AND ScriptName = '' AND AIName = ''") ~= nil)

check("spawn a creature beside the character, temporary", console("spawn-creature\t68\t"):find("^ok:City Guard is beside Hosty") ~= nil
    and host.spawn[1] == 68 and host.spawn[6] == 3 and host.spawn[7] == 1800000)
check("spawn: an unknown number refused", console("spawn-creature\t99\t") == "err:No creature with that number.")
check("spawn an object", console("spawn-object\t500\t"):find("^ok:Chair is beside Hosty") ~= nil and host.summon[1] == 500)
check("give an item", console("item\t6948\t") == "ok:Hosty got Hearthstone." and host.items[1] == 6948)
host.full = true
check("full bags said", console("item\t6948\t") == "err:Hosty's bags are full.")
host.full = nil
local live = 0
for _, o in pairs(mapObjects) do if not o.gone then live = live + 1 end end
check("clear takes back what it spawned", live == 2 and console("clear\t\t") == "ok:Cleared 2 things you spawned."
    and next(mapObjects) == nil and console("clear\t\t") == "ok:Nothing of yours to clear here.")

check("not on the in-game window's channel", (function()
    local okd = pcall(function() return OVMod and OVMod.Dispatch(friend, "wuhost", "sandbox", "level\t80\t") end)
    return okd and commands[#commands] ~= "character level Friendly 80"
end)())
check("a player who is not the GM is refused in game", (function()
    HC.Handle(friend, "sandbox", "level\t80\t")
    return commands[#commands] ~= "character level Friendly 80"
end)())

table.remove(world, 2)   -- the host logs out
check("nobody to act on: says to log in first", console("level\t10\t"):find("^err:Log in to your world") ~= nil)

os.remove(tmp); os.remove(hostFile)
print(string.format("%d passed, %d failed", pass, fail))
if fail > 0 then os.exit(1) end
