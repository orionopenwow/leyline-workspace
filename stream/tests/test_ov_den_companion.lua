-- lua5.1 stream/tests/test_ov_den_companion.lua [lua_scripts folder]
-- Your companion in your Den (ov_den_companion.lua) under a fake mod-ale, with ov_build.lua, ov_den.lua,
-- ov_den_home.lua and ov_modapi.lua: off unless OV_DEN_COMPANION=1; .den jaina (the owner only: the creature entry,
-- friendly, following at a short distance, back to your side after a teleport, gone on leaving the Den, on .den
-- jaina away and on logout); what the owner tells her (.den ask, "Jaina, ..." within 10 yards: cleaned, numbered, where
-- they stood, rate-limited) written to companion-in.json for the launcher; the launcher's lines from companion-out.json
-- said in order, one every few seconds, old ones and brand words never; a build from companion-place.json checked
-- whole, then placed part by part with her progress; restored after a reload; and no game-brand words anywhere.
local dir = arg[1] or "realm/custom/lua_scripts"
local buildFile, denFile, homeFile, compFile, modapi = dir .. "/ov_build.lua", dir .. "/ov_den.lua", dir .. "/ov_den_home.lua",
    dir .. "/ov_den_companion.lua", dir .. "/ov_modapi.lua"

local pass, fail = 0, 0
local realPrint = print
local function check(name, ok)
    if ok then pass = pass + 1; realPrint("PASS " .. name) else fail = fail + 1; realPrint("FAIL " .. name) end
end
print = function() end

local clock = 2000000
os.time = function() return clock end
local SAVES = os.tmpname()
os.remove(SAVES)
os.execute("mkdir -p '" .. SAVES .. "'")
local env = { OV_DEN = "1", OV_DEN_SIZE = "16", OV_BUILD_PER_PLAYER = "400", OV_BUILD_POINTS = "1500", OV_BUILD_PER_AREA = "800",
    OV_BUILD_NEW_HOURS = "0", OV_BUILD_SCALE_MIN = "0.1", OV_BUILD_SCALE_MAX = "5", OV_DEN_SAVES = SAVES, OV_DEN_SHIPPED = dir,
    OV_DEN_INSIDE_POINTS = "1000", OV_DEN_OUTSIDE_POINTS = "300" }
os.getenv = function(k) return env[k] end
local function readFile(p) local f = io.open(p, "r"); if not f then return nil end; local t = f:read("*a"); f:close(); return t end
local function writeFile(p, t) local f = io.open(p, "w"); f:write(t); f:close() end

-- ---------------------------------------------------------------- fake world
local events, luaEvents = {}, {}
function RegisterPlayerEvent(id, fn) events[id] = events[id] or {}; table.insert(events[id], fn) end
function RegisterServerEvent() end
function CreateLuaEvent(fn, delay, rep) luaEvents[#luaEvents + 1] = { fn = fn, delay = delay, rep = rep } end
local function fire(id, ...) local r; for _, fn in ipairs(events[id] or {}) do local x = fn(id, ...); if x ~= nil then r = x end end; return r end

local objects, nextGuid, maps = {}, 9000, {}
local GROUND = 12.6
local function mapObj(id)
    maps[id] = maps[id] or { id = id, GetWorldObject = function(self, g) local o = objects[g]; if o and o.map == self.id then return o end end,
        IsDungeon = function() return false end, GetHeight = function() return GROUND end }
    return maps[id]
end
function GetMapById(id) return mapObj(id) end
local says = {}   -- what creatures said aloud
local function creature(entry, map, x, y, z, o)
    nextGuid = nextGuid + 1
    local c = { guid = nextGuid, entry = entry, map = map, x = x, y = y, z = z, o = o, kind = "creature" }
    function c:GetGUID() return self.guid end
    function c:GetX() return self.x end
    function c:GetY() return self.y end
    function c:SetFaction(f) self.faction = f end
    function c:SetWanderRadius(r) self.wander = r end
    function c:MoveFollow(t, d, a) self.follows, self.dist, self.angle = t, d, a end
    function c:SendUnitSay(text, lang) says[#says + 1] = text; self.lang = lang end
    function c:NearTeleport(nx, ny, nz, no) self.x, self.y, self.z, self.o = nx, ny, nz, no; self.teleported = true; return true end
    function c:DespawnOrUnsummon() objects[self.guid] = nil end
    objects[c.guid] = c
    return c
end
function PerformIngameSpawn(kind, entry, map, inst, x, y, z, o, save)
    if save then return nil end
    if kind == 1 then return creature(entry, map, x, y, z, o) end
    if kind ~= 2 then return nil end
    nextGuid = nextGuid + 1
    local go = { guid = nextGuid, entry = entry, map = map, x = x, y = y, z = z, o = o }
    function go:GetGUID() return self.guid end
    function go:RemoveFromWorld() objects[self.guid] = nil end
    function go:SetScale(s) self.scale = s end
    objects[go.guid] = go
    return go
end
function GetGUIDLow(g) return g end

local sqlLog = {}
function CharDBExecute(sql) sqlLog[#sqlLog + 1] = sql end
function CharDBQuery() return nil end
function WorldDBQuery(sql)
    local rows = {}
    local list = sql:match("IN %(([%d,]+)%)")
    if not list then return nil end
    for id in list:gmatch("%d+") do rows[#rows + 1] = { id } end
    local r = { rows = rows, i = 1 }
    function r:GetUInt32(c) return tonumber(self.rows[self.i][c + 1]) end
    function r:NextRow() self.i = self.i + 1; return self.i <= #self.rows end
    return r
end

local online, allMsgs = {}, {}
function GetPlayerByName(n) for _, p in pairs(online) do if p.name:lower() == n:lower() then return p end end end
function GetPlayersInWorld() local t = {}; for _, p in pairs(online) do t[#t + 1] = p end; table.sort(t, function(a, b) return a.guid < b.guid end); return t end
local function player(over)
    local p = { msgs = {}, addon = {}, gm = 0, gmOn = false, map = 1, x = 16226.2, y = 16257.0, z = GROUND, o = 0, teleports = 0 }
    for k, v in pairs(over or {}) do p[k] = v end
    function p:SendBroadcastMessage(m) self.msgs[#self.msgs + 1] = m; allMsgs[#allMsgs + 1] = m end
    function p:SendAddonMessage(prefix, msg) self.addon[#self.addon + 1] = msg end
    function p:GetGUIDLow() return self.guid end
    function p:GetName() return self.name end
    function p:GetGMRank() return self.gm end
    function p:IsGM() return self.gmOn end
    function p:GetMapId() return self.map end
    function p:GetMap() return mapObj(self.map) end
    function p:GetInstanceId() return 0 end
    function p:GetX() return self.x end
    function p:GetY() return self.y end
    function p:GetZ() return self.z end
    function p:GetO() return self.o end
    function p:Teleport(m, x, y, z, o) self.map, self.x, self.y, self.z, self.o = m, x, y, z, o; self.teleports = self.teleports + 1; return true end
    function p:SetBindPoint() end
    function p:RegisterEvent(fn) fn(1, 0, 1, self); return 1 end
    function p:SpawnCreature(entry, x, y, z, o, kind, dur) self.spawnKind = kind; return creature(entry, self.map, x, y, z, o) end
    online[p.guid] = p
    return p
end
local function cmd(p, text) return fire(42, p, text) end
local function said(p, pat) for _, m in ipairs(p.msgs) do if m:find(pat) then return true end end end
local function spoke(pat) for _, m in ipairs(says) do if m:find(pat) then return true end end end
local function clear(p) p.msgs, p.addon = {}, {} end
local function tick() for _, e in ipairs(luaEvents) do if e.delay == 2000 then e.fn() end end end
local function her() for _, o in pairs(objects) do if o.kind == "creature" then return o end end end
local function hers() local n = 0; for _, o in pairs(objects) do if o.kind == "creature" then n = n + 1 end end; return n end

package.preload["ov_modapi"] = function() return dofile(modapi) end
dofile(buildFile)
dofile(denFile)
dofile(compFile)
check("off unless OV_DEN_COMPANION=1", OV_DEN_COMPANION_TEST == nil and events[18] == nil)
env.OV_DEN_COMPANION = "1"
dofile(compFile)
dofile(homeFile)   -- loads after the companion (file order) and sets OVDen.help: the companion puts its line back
local T, D, C = assert(OV_BUILD_TEST), assert(OV_DEN_TEST), assert(OV_DEN_COMPANION_TEST, "loads with OV_DEN=1 and OV_DEN_COMPANION=1")
local DEN = T.DEN
local B = rawget(_G, "OVBuild")
local function decor() local n = 0; for _, p in pairs(T.props) do if p.owner ~= 0 then n = n + 1 end end; return n end
local function inDoc() return B.JsonDecode(readFile(SAVES .. "/companion-in.json") or "", 1e6) end

check("the creature: entry 32364, the game's own Jaina (no AI or script, immune, no gossip)", C.ENTRY == 32364)

-- ---------------------------------------------------------------- calling her
local own = player({ guid = 1, name = "Owner", gm = 3 })
local vis = player({ guid = 2, name = "Visitor", x = 16226.2 + 3, y = 16257.0 })
fire(3, own); fire(3, vis)
tick()
check("the room is up and the owner is in the Den", D.inside(own) and DEN.floorZ == GROUND)
cmd(vis, ".den jaina")
check("only the owner calls her", said(vis, "Only the Den's owner") and hers() == 0)
cmd(own, ".den jaina")
local c = her()
check(".den jaina: she comes, as a temporary summon", c and c.entry == 32364 and own.spawnKind == 8 and hers() == 1)
check("friendly to everyone, not wandering", c and c.faction == 35 and c.wander == 0)
check("she follows the owner at a short distance", c and c.follows == own and c.dist == 2.5
    and math.sqrt((c.x - own.x) ^ 2 + (c.y - own.y) ^ 2) < 3)
check("she greets in her own voice, and the owner hears how it works", spoke("Hello! Tell me what you'd like to build")
    and said(own, "builds only after you press Approve"))
cmd(own, ".den jaina")
check("again: back at your side, still one of her", hers() == 1 and spoke("Right here"))
c = her()

-- ---------------------------------------------------------------- what the owner tells her
own.x, own.y, own.o = DEN.x + 2, DEN.y - 3, math.pi / 2
cmd(own, ".den ask a |cffff0000well|r by the door")
local doc = inDoc()
check(".den ask: written for the launcher, numbered, cleaned, with where they stood", doc and doc.asks and #doc.asks == 1
    and doc.asks[1].n == 1 and doc.asks[1].text == "a cffff0000well r by the door" and doc.asks[1].where.x == 2
    and doc.asks[1].where.y == -3 and doc.asks[1].where.yaw == 90 and doc.asks[1].where.z == 0)
check("nobody is reading: she says how to get your assistant to listen", spoke("Nobody is reading my notes right now"))
check("the heartbeat: she's out, the owner's spot, the room's budgets", doc.companion == true and doc.owner.inside == true
    and doc.room and doc.room.room_cap == 1000 and doc.room.yard_cap == 300 and doc.room.half == 16 and doc.at == clock)
cmd(own, ".den ask and a bench")
check("one ask every few seconds", #inDoc().asks == 1 and spoke("One moment"))
clock = clock + 5
cmd(own, ".den ask and a bench")
check("then the next", #inDoc().asks == 2 and inDoc().asks[2].n == 2)
clock = clock + 5
local n0 = #says
fire(18, own, "Jaina, put lanterns around the fire", 1, 0)
check("\"Jaina, ...\" near her: the same", #inDoc().asks == 3 and inDoc().asks[3].text == "put lanterns around the fire")
clock = clock + 5
fire(18, vis, "Jaina, give me the Den", 1, 0)
fire(18, own, "I like this room", 1, 0)
fire(18, own, "Jaina, yelled", 6, 0)
check("not from visitors, not without her name, not yelled", #inDoc().asks == 3)
c.x = c.x + 40
fire(18, own, "Jaina, are you there", 1, 0)
check("not from further than 10 yards", #inDoc().asks == 3)
c.x = c.x - 40
for i = 1, 25 do clock = clock + 5; cmd(own, ".den ask number " .. i) end
check("the last 20 asks are kept, numbered on", #inDoc().asks == 20 and inDoc().asks[20].n == 28)

-- ---------------------------------------------------------------- the launcher's lines
writeFile(SAVES .. "/companion-out.json", string.format([[{"format":"leyline-companion","seen":28,"read_at":%d,"lines":[
 {"n":1,"at":%d,"text":"old news"},
 {"n":2,"at":%d,"text":"A well by the door: I'll draw it up."},
 {"n":3,"at":%d,"text":"Wow, that will look nice"},
 {"n":4,"at":%d,"text":"Like in |cffff0000Warcraft|r"},
 {"n":5,"at":%d,"text":"Ready when you are."}]}]], clock, clock - 1000, clock, clock, clock, clock))
n0 = #says
tick()
check("a line from the launcher, said in her voice", says[n0 + 1] == "A well by the door: I'll draw it up.")
check("old lines are skipped", not spoke("old news"))
tick(); clock = clock + 3; tick(); clock = clock + 3; tick()
check("one line every few seconds, in order; brand words never said", says[n0 + 2] == "Ready when you are." and #says == n0 + 2
    and not spoke("Wow, that") and not spoke("Warcraft"))
clock = clock + 11; tick()
check("the heartbeat says which lines were said", inDoc().said == 5)
clock = clock + 5
cmd(own, ".den ask a fountain")
check("someone is reading (read_at): she says she's on it", spoke("Let me think about that"))

-- ---------------------------------------------------------------- building
local props = {}
for i = 1, 30 do props[#props + 1] = string.format('{"entry":2413,"x":%d,"y":%d,"z":0,"yaw":90}', (i % 6) * 2 - 5, math.floor(i / 6) * 2 - 5) end
writeFile(SAVES .. "/tavern.den.json", '{"format":"leyline-den","version":1,"name":"tavern","kind":"agent","props":[' .. table.concat(props, ",") .. ']}')
writeFile(SAVES .. "/companion-place.json", '{"file":"tavern","id":"abc123","label":"the tavern"}')
local before = decor()
n0 = #says
clock = clock + 10
tick()
check("a build from the launcher: checked whole, then announced", spoke("Placing the tavern: 30 pieces") and readFile(SAVES .. "/companion-place.json") == nil)
check("the heartbeat shows it in progress", inDoc().placing.state == "placing" and inDoc().placing.of == 30)
check("part by part: 20 of 30, in her words", decor() == before + 20 and spoke("Placing the tavern: 20 of 30 pieces"))
clock = clock + 2; tick()
check("one part every few seconds (the paste rate)", decor() == before + 20)
clock = clock + 4; tick()
check("done: every piece in, where the file says", decor() == before + 30 and spoke("Done: the tavern is in, 30 pieces")
    and inDoc().placing.state == "done" and inDoc().placing.done == 30)
check("each piece where the file put it", (function()
    for _, p in pairs(T.props) do
        if p.owner == 1 and p.entry == 2413 and math.abs(p.x - (DEN.x + 1)) < 0.01 and math.abs(p.y - (DEN.y - 5)) < 0.01 and math.abs(p.z - GROUND) < 0.01 then return true end
    end end)())
-- too big for the yard: refused whole, nothing placed
props = {}
for i = 1, 40 do props[#props + 1] = string.format('{"entry":19006,"x":25,"y":%d,"z":0}', (i % 5) - 2) end
writeFile(SAVES .. "/houses.den.json", '{"format":"leyline-den","version":1,"name":"houses","props":[' .. table.concat(props, ",") .. ']}')
writeFile(SAVES .. "/companion-place.json", '{"file":"houses","id":"abc124","label":"the street"}')
before = decor()
clock = clock + 10; tick(); clock = clock + 6; tick()
check("too much for the budgets: said, and nothing placed", spoke("That won't fit") and decor() == before and inDoc().placing.state == "failed")
writeFile(SAVES .. "/companion-place.json", '{"file":"../../etc/passwd","id":"x1","label":"evil"}')
clock = clock + 10; tick()
check("only Den files by name", readFile(SAVES .. "/companion-place.json") == nil and decor() == before)

-- ---------------------------------------------------------------- her short commands and her own guide
check("the short commands are whole lines", C.command("Stay") == "stay" and C.command(" wait here! ") == "stay"
    and C.command("please follow me") == "follow" and C.command("come here, please") == "come" and C.command("what next?") == "tips"
    and C.command("go away") == "away" and C.command("stay by the door and add a bench") == nil)
check("her guide: plain answers to common questions", C.guide("how do I save this?"):find("^%.den save") ~= nil
    and C.guide("where can I level? quests?"):find("Solo leveling") ~= nil and C.guide("a well by the door") == nil)
os.remove(SAVES .. "/companion-out.json")   -- nobody reading her notes
c = her()
local asks0, lastN = #inDoc().asks, inDoc().asks[#inDoc().asks].n
clock = clock + 10
c.x = c.x + 20   -- far beyond hearing for asks: commands still reach her
fire(18, own, "Jaina, stay", 1, 0)
check("\"Jaina, stay\": she waits, from anywhere in the Den, and it isn't an ask", spoke("I'll wait here")
    and inDoc().asks[#inDoc().asks].n == lastN)
c.x = own.x + 45
clock = clock + 10; tick()
check("told to stay: a long way off doesn't pull her back", not c.teleported and hers() == 1)
c.x = own.x + 20
fire(18, own, "Jaina, come", 1, 0)
check("\"Jaina, come\": she walks over and follows again", spoke("On my way") and c.follows == own)
fire(18, own, "Jaina, tips", 1, 0)
check("\"Jaina, tips\": a tip in her voice", spoke("stamp inn now"))
c.x = own.x + 1
clock = clock + 10
fire(18, own, "Jaina, how do I save my den?", 1, 0)
check("no assistant reading: her own answer at once, and it is still written down", spoke("%.den save and a name")
    and inDoc().asks[#inDoc().asks].text == "how do I save my den?")
cmd(own, ".den jaina stay")
check(".den jaina stay: the same as saying it", spoke("I'll wait here"))
cmd(own, ".den jaina follow")
check(".den jaina follow", spoke("Right behind you"))

-- ---------------------------------------------------------------- following, leaving
own.x = DEN.x + 5
c = her()
c.x = c.x + 45
clock = clock + 10; tick()
check("far from her (a teleport): she comes back to your side", c.teleported and math.abs(c.x - own.x) < 3 and hers() == 1)
clear(own)
own.map = 0
fire(28, own)
check("out of the Den: she stays behind (gone)", hers() == 0)
own.map, own.x, own.y = 1, DEN.x, DEN.y
cmd(own, ".den jaina")
cmd(own, ".den jaina away")
check(".den jaina away: a goodbye, and gone", hers() == 0 and spoke("Call me when you need me"))
cmd(own, ".den companion")
check(".den companion: the same as .den jaina", hers() == 1)
fire(4, own)
check("logging out: gone", hers() == 0)

-- ---------------------------------------------------------------- help, reload, words
fire(3, own)
clear(own)
cmd(own, ".den")
check(".den's help names her for the owner (next to the home script's lines)", said(own, "%.den jaina") and said(own, "%.den stamps"))
clear(vis)
cmd(vis, ".den")
check("not for visitors", not said(vis, "%.den jaina"))
OV_DEN_COMPANION_TEST = nil
dofile(compFile)
local C2 = OV_DEN_COMPANION_TEST
check("after a reload: the asks and the last line said come back", C2.said == 5 and #C2.asks == 20 and C2.nextAsk == C.nextAsk and C.nextAsk == 31)
check("no game-brand words in anything players see or she says", (function()
    local function bad(s) local l = s:lower(); return l:find("warcraft") or l:find("blizzard") or l:find("lich king") or l:find("%f[%a]wow%f[%A]") end
    for _, m in ipairs(allMsgs) do if bad(m) then return false end end
    for _, m in ipairs(says) do if bad(m) then return false end end
    return true end)())
check("plain words: no chat codes in what she says", (function() for _, m in ipairs(says) do if m:find("|", 1, true) then return false end end; return true end)())

os.execute("rm -rf '" .. SAVES .. "'")
realPrint(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
