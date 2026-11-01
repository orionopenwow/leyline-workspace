-- lua5.1 stream/tests/test_ov_den_home.lua [lua_scripts folder]
-- A Den's home (ov_den_home.lua) under a fake mod-ale, with ov_build.lua, ov_den.lua and ov_modapi.lua: the starter
-- places (the shipped files, every prop a verified catalog entry, the owner's first-visit choice, start / replace /
-- empty), stamps (the files, the mark before placing, turning on 15 degree steps, here, place as one undo, ground),
-- the grid at your feet (.build grid), the undo stack (50 deep), who may do what (owner, builder, visitor), the front
-- door's knock, the mailbox (notes cleaned, one a minute, the owner's mail, kept in the save file and back on open),
-- .den visit, and no game-brand words in what players see.
local dir = arg[1] or "realm/custom/lua_scripts"
local buildFile, denFile, homeFile, modapi = dir .. "/ov_build.lua", dir .. "/ov_den.lua", dir .. "/ov_den_home.lua", dir .. "/ov_modapi.lua"

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
function PerformIngameSpawn(kind, entry, map, inst, x, y, z, o, save)
    if kind ~= 2 or save then return nil end
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
    function p:GetX() return self.x end
    function p:GetY() return self.y end
    function p:GetZ() return self.z end
    function p:GetO() return self.o end
    function p:Teleport(m, x, y, z, o) self.map, self.x, self.y, self.z, self.o = m, x, y, z, o; self.teleports = self.teleports + 1; return true end
    function p:SetBindPoint() end
    function p:RegisterEvent(fn) fn(1, 0, 1, self); return 1 end
    online[p.guid] = p
    return p
end
local function cmd(p, text) return fire(42, p, text) end
local function said(p, pat) for _, m in ipairs(p.msgs) do if m:find(pat) then return true end end end
local function addonHas(p, pat) for _, m in ipairs(p.addon) do if m:find(pat) then return true end end end
local function sqlHas(pat) for _, s in ipairs(sqlLog) do if s:find(pat) then return true end end end
local function clear(p) p.msgs, p.addon = {}, {} end

package.preload["ov_modapi"] = function() return dofile(modapi) end
dofile(buildFile)
dofile(denFile)
dofile(homeFile)
local T, D, H = assert(OV_BUILD_TEST), assert(OV_DEN_TEST), assert(OV_DEN_HOME_TEST, "ov_den_home.lua loads with OV_DEN=1")
local DEN = T.DEN
local OVMod = rawget(_G, "OVMod")

local function decor(owner)
    local n = 0
    for _, p in pairs(T.props) do if p.owner ~= 0 and (not owner or p.owner == owner) then n = n + 1 end end
    return n
end
local function stakes() local n = 0; for _, o in pairs(objects) do if o.entry == H.MARKER then n = n + 1 end end; return n end
local function catalogIds() local ids = {}; for _, c in ipairs(T.CATALOG()) do ids[c.id] = true end; return ids end
local function tick(ms) for _, e in ipairs(luaEvents) do if e.delay == ms then e.fn() end end end
local function near(a, b) return math.abs(a - b) < 0.01 end

-- ---------------------------------------------------------------- the shipped files
local ids = catalogIds()
check("six stamps ship: garden, market, campfire, dock, house, inn", (function()
    for _, n in ipairs({ "garden", "market", "campfire", "dock", "house", "inn" }) do if not H.stamp(n) then return false end end
    return #H.STAMPS == 6 end)())
check("no stamp outside the list is read (no paths from players)", H.stamp("../ov_build") == nil and H.stamp("nothing") == nil)
check("every stamp piece is a verified catalog entry", (function()
    for _, n in ipairs(H.STAMPS) do for _, e in ipairs(H.stamp(n).props) do if not ids[e.entry] then return false end end end
    return true end)())
check("the inn stamp has an inn, a door and a mailbox", (function()
    local r, inn = {}, false
    for _, e in ipairs(H.stamp("inn").props) do if e.role then r[e.role] = true end; if e.entry == 19007 then inn = true end end
    return inn and r.house and r.door and r.mailbox end)())
check("the house stamp has a house, a door and a mailbox", (function()
    local r = {}
    for _, e in ipairs(H.stamp("house").props) do if e.role then r[e.role] = true end end
    return r.house and r.door and r.mailbox end)())
check("two starter places ship, every prop a verified catalog entry", (function()
    for _, s in ipairs(H.STARTERS) do
        if s.file then
            local doc = H.starterDoc(s)
            if not doc or #doc.props < 20 then return false end
            for _, e in ipairs(doc.props) do if not ids[e.entry] then return false end end
        end
    end
    return true end)())
check("the starter places pass the Den's own check (inside the Den, within both budgets, nothing left out)", (function()
    DEN.floorZ = GROUND
    for _, s in ipairs(H.STARTERS) do
        if s.file then
            local c = T.X.DenCheck(H.starterDoc(s), false)
            if not c or c.skipped ~= 0 then return false end
        end
    end
    return true end)())
check("a smaller room still takes them (OV_DEN_SIZE 12: nothing outside the Den)", (function()
    local half, r = DEN.half, DEN.r
    DEN.half, DEN.r = 12, 26
    local ok = true
    for _, s in ipairs(H.STARTERS) do if s.file then local c = T.X.DenCheck(H.starterDoc(s), false); ok = ok and c and c.skipped == 0 end end
    DEN.half, DEN.r = half, r
    return ok end)())

-- ---------------------------------------------------------------- the math
check("facings snap to 15 degree steps", near(H.step(math.rad(7)), 0) and near(H.step(math.rad(8)), math.rad(15))
    and near(H.step(math.rad(359)), 0) and near(H.step(math.rad(-20)), math.rad(345)))
check("a local offset turns with the stamp (x ahead, y left)", (function()
    local x, y = H.turn(2, 0, math.pi / 2)
    local x2, y2 = H.turn(0, 1, math.pi)
    return near(x, 0) and near(y, 2) and near(x2, 0) and near(y2, -1) end)())
check("a stamp's pieces: turned around the mark, facings added", (function()
    local st = { props = { { entry = 1, x = 3, y = 0, z = 0, yaw = 90 } } }
    local p = H.pieces(st, { x = 10, y = 20, z = 5, o = math.pi / 2 })[1]
    return near(p.x, 10) and near(p.y, 23) and near(p.z, 5) and near(p.o, math.pi) end)())
check("the mark: four corners and one ahead", (function()
    local m = H.markPoints({ props = { { x = -2, y = -1 }, { x = 2, y = 1 } } }, { x = 0, y = 0, o = 0 })
    return #m == 5 and near(m[1].x, -3) and near(m[1].y, -2) and near(m[3].x, 3) and near(m[3].y, 2) and near(m[5].x, 4.5) and near(m[5].y, 0) end)())

-- ---------------------------------------------------------------- the owner's first visit
local own = player({ guid = 1, name = "Owner", gm = 3 })
fire(30, own)
fire(3, own)
check("first visit, empty Den: the owner is asked how it starts", said(own, "Pick how your Den starts") and addonHas(own, "^wuden\tstart\t1/1\tforest,Forest glade;keep,Dark keep;empty,Empty"))
fire(28, own)
check("asked once a session", (function() local n = 0; for _, m in ipairs(own.msgs) do if m:find("Pick how your Den starts") then n = n + 1 end end; return n == 1 end)())
check("the home line goes to the addon", addonHas(own, "^wuden\thome\t1/1\tstarted=%-;door=0;mailbox=0;mail=0;role=owner;stamp=%-;stamps=garden,market,campfire,dock,house,inn"))

local bud = player({ guid = 2, name = "Buddy" })
cmd(bud, ".den start forest")
check("only the owner picks the start", said(bud, "Only the Den's owner") and decor() == 0)
cmd(own, ".den start")
check(".den start lists the three", said(own, "Forest glade") and said(own, "Dark keep") and said(own, "Empty"))
cmd(own, ".den start forest")
local forestN = #H.starterDoc(H.STARTERS[1]).props
check("Forest glade: every prop placed around the Den's middle", decor() == forestN and decor(1) == forestN)
check("Forest glade: the room turns wood", D.settings.theme == "wood" and D.settings.started == "forest")
check("Forest glade: the front door and mailbox are set", H.spot("door") and H.spot("mailbox")
    and near(H.spot("door").x, DEN.x + 1.5) and near(H.spot("door").y, DEN.y + 2.2))
check("the home line says so", addonHas(own, "started=forest;door=1;mailbox=1"))
clock = clock + 10
cmd(own, ".den start keep")
check("with decor there it asks first", said(own, "start keep replace") and D.settings.started == "forest")
cmd(own, ".den start keep replace")
local keepN = #H.starterDoc(H.STARTERS[2]).props
check("Dark keep replaces it, a copy kept as backup", decor() == keepN and D.settings.theme == "stone"
    and readFile(SAVES .. "/backup.den.json") ~= nil)
cmd(own, "build undo")
check("one undo takes the starter away", decor() == 0)
clock = clock + 10
cmd(own, ".den start empty")
check("Empty: nothing placed, remembered", decor() == 0 and D.settings.started == "empty")
fire(3, own)
check("not asked again once started", (function() local n = 0; for _, m in ipairs(own.msgs) do if m:find("Pick how your Den starts") then n = n + 1 end end; return n == 1 end)())

-- ---------------------------------------------------------------- stamps: the mark, turning, placing
cmd(own, ".den allow Buddy build")
local vis = player({ guid = 3, name = "Visitor" })
clear(own)
cmd(own, ".den stamps")
check(".den stamps lists them", said(own, "garden") and said(own, "Small house"))
cmd(vis, ".den stamp garden")
check("a visitor can't stamp", said(vis, "You're visiting") and stakes() == 0 and H.pending[3] == nil)
bud.x, bud.y, bud.o = DEN.x + 5, DEN.y - 6, math.rad(97)
cmd(bud, ".den stamp garden")
local pend = H.pending[2]
check("a builder marks a stamp: five stakes, nothing placed", pend and stakes() == 5 and decor() == 0)
check("the mark faces the builder's way, on a 15 degree step", near(pend.o, math.rad(90)))
check("stakes aren't props (never saved)", (function() for _, p in pairs(T.props) do if p.entry == H.MARKER then return false end end; return true end)())
cmd(bud, ".den stamp turn")
check("turn: 15 degrees by default", near(H.pending[2].o, math.rad(105)) and stakes() == 5)
cmd(bud, ".den stamp turn -45")
check("turn -45", near(H.pending[2].o, math.rad(60)))
cmd(bud, ".den stamp turn 7")
check("turns stay on 15 degree steps", near(H.pending[2].o, math.rad(60)) or near(H.pending[2].o, math.rad(75)))
bud.x = DEN.x + 6
cmd(bud, ".den stamp here")
check("here: the mark moves to you, keeping its facing", near(H.pending[2].x, DEN.x + 6) and stakes() == 5
    and (near(H.pending[2].o, math.rad(60)) or near(H.pending[2].o, math.rad(75))))
cmd(bud, ".den stamp cancel")
check("cancel: the stakes go, nothing placed", stakes() == 0 and H.pending[2] == nil and decor() == 0)
cmd(bud, ".den stamp place")
check("place with nothing marked says how", said(bud, "Nothing marked"))
clock = clock + 10
bud.o = 0
local doorBefore = H.spot("door")
cmd(bud, ".den stamp garden")
cmd(bud, ".den stamp place")
local gardenN = #H.stamp("garden").props
check("place: the garden is down, the builder's props", decor(2) == gardenN and stakes() == 0 and H.pending[2] == nil)
check("each piece on the floor (the room's)", (function()
    for _, p in pairs(T.props) do if p.owner == 2 and not near(p.z, GROUND) then return false end end
    return true end)())
check("a piece lands where the stamp says (x ahead, y left)", (function()
    for _, p in pairs(T.props) do
        if p.owner == 2 and p.entry == 24538 then return near(p.x, DEN.x + 6 - 4.5) and near(p.y, DEN.y - 6) end
    end
    return false end)())
check("a builder's stamp doesn't move the owner's door", doorBefore and near(H.spot("door").x, doorBefore.x) and near(H.spot("door").y, doorBefore.y))
cmd(bud, "build undo")
check("one undo takes the whole stamp back", decor(2) == 0)
clock = clock + 10
cmd(bud, ".den stamp campfire now 30")
check("now: placed at once, turned", decor(2) == #H.stamp("campfire").props)
clock = clock + 10
own.x, own.y, own.o = DEN.x - 8, DEN.y + 2, 0
cmd(own, ".den stamp house now")
check("the owner's house stamp sets the front door and mailbox", H.spot("door") and near(H.spot("door").x, DEN.x - 8 + 2)
    and near(H.spot("door").y, DEN.y + 2 + 1.5) and H.spot("mailbox") ~= nil)
clock = clock + 10
cmd(bud, ".den stamp nope")
check("an unknown stamp says so", said(bud, "No stamp called 'nope'"))
H.pending[2] = { name = "garden", map = 1, x = DEN.x, y = DEN.y, z = GROUND, o = 0, stakes = {}, at = clock }
H.mark(bud, H.pending[2])
clock = clock + H.MARK_KEEP + 1
tick(5000)
check("a forgotten mark goes by itself", H.pending[2] == nil and stakes() == 0)

-- ---------------------------------------------------------------- the grid at your feet
clock = clock + 10
bud.x, bud.y, bud.z, bud.o = DEN.x + 3.3, DEN.y - 4.8, GROUND + 0.4, math.rad(50)
cmd(bud, ".build grid 2")
check(".build grid 2", said(bud, "Grid 2 yd"))
cmd(bud, ".build grid ground on")
cmd(bud, ".build spawn chair")
local last
for _, p in pairs(T.props) do if p.owner == 2 and (not last or p.id > last.id) then last = p end end
check("at your feet on the grid (lined up with the Den's middle), turned to 45, on the floor", last and near(last.x, DEN.x + 4)
    and near(last.y, DEN.y - 4) and near(last.o, math.rad(45)) and near(last.z, GROUND))
cmd(bud, ".build grid 5")
cmd(bud, ".build grid 3")
check("only 0.5, 1, 2, 5 or off", said(bud, "Grid: 5 yd"))
cmd(bud, ".build grid off")
cmd(bud, ".build grid ground off")
clock = clock + 2
cmd(bud, ".build spawn chair")
local last2
for _, p in pairs(T.props) do if p.owner == 2 and (not last2 or p.id > last2.id) then last2 = p end end
check("grid off: exactly where you stand", last2 and near(last2.x, bud.x) and near(last2.z, GROUND + 0.4))

-- ---------------------------------------------------------------- undo: the last 50 actions
local u = player({ guid = 4, name = "Undoer", gm = 1, x = DEN.x + 1, y = DEN.y + 1 })
for i = 1, 55 do clock = clock + 1; u.x = DEN.x + (i % 10) - 5; cmd(u, ".build spawn chair") end
check("55 props placed", decor(4) == 55)
check("the undo stack keeps the last 50", #T.undo[4] == 50)
for i = 1, 50 do cmd(u, ".build undo") end
check("50 undos take 50 back", decor(4) == 5)
clear(u)
cmd(u, ".build undo")
check("the 51st: nothing to undo", said(u, "Nothing to undo"))
for i = 1, 50 do cmd(u, ".build redo") end
check("and redo brings all 50 back", decor(4) == 55)
online[4] = nil

-- ---------------------------------------------------------------- the front door and the mailbox
local door = H.spot("door")
vis.x, vis.y = DEN.x + 14, DEN.y - 14
clear(own); clear(vis)
cmd(vis, ".den knock")
check("knocking away from the door: go up to it first", said(vis, "Go up to the front door"))
cmd(vis, ".den visit")
check(".den visit: to the front door", near(vis.x, door.x) and near(vis.y, door.y))
cmd(vis, ".den knock")
check("a knock: the owner hears it (chat and the addon)", said(own, "Visitor knocks at your front door") and addonHas(own, "^wuden\tknock\t1/1\tVisitor$")
    and said(vis, "The owner heard it"))
clear(vis)
cmd(vis, ".den knock")
check("one knock every 30 seconds", said(vis, "a moment ago"))
cmd(own, ".den knock")
check("the owner doesn't knock on their own door", said(own, "your own door"))
online[1] = nil
clock = clock + 31
clear(vis)
cmd(vis, ".den knock")
check("nobody home: leave a note", said(vis, "Nobody's home"))
online[1] = own

cmd(vis, ".den note Lovely place! |cffff0000red|r it's 'great'\\ ")
check("a note goes in the mailbox, cleaned (no colour codes, quotes or backslashes)", #H.notes == 1 and H.notes[1].from == "Visitor"
    and not H.notes[1].text:find("|", 1, true) and not H.notes[1].text:find("'", 1, true) and not H.notes[1].text:find("\\", 1, true)
    and H.notes[1].text:find("Lovely place!", 1, true) and sqlHas("^INSERT INTO mod_den_notes"))
check("the owner hears about it", said(own, "Visitor left a note"))
cmd(vis, ".den note again")
check("one note a minute", said(vis, "One note a minute") and #H.notes == 1)
clock = clock + 61
OVMod.Dispatch(vis, "wuden", "note", string.rep("x", 250))
check("from the addon too, cut to 200 letters", #H.notes == 2 and #H.notes[2].text == 200)
clock = clock + 61
cmd(vis, ".den note    ")
check("an empty note says how", said(vis, "Usage: |cffffff00.den note"))
vis.map = 0
clock = clock + 61
cmd(vis, ".den note from outside")
check("notes only from inside the Den", said(vis, "Notes go in the mailbox") and #H.notes == 2)
vis.map, vis.x, vis.y = 1, door.x, door.y
cmd(vis, ".den mail")
check("only the owner reads the mail", said(vis, "The mailbox is the owner's"))
check("the home line counts the unread notes for the owner", (function() clear(own); H.sendHome(own); return addonHas(own, "mail=2;role=owner") end)())
cmd(own, ".den mail")
check("the owner reads the notes, newest first; the addon gets each", said(own, "Visitor, ") and addonHas(own, "^wuden\tmail\t1/1\t%d+;Visitor;%d+;Lovely place!")
    and H.unread() == 0)
check("read notes are marked seen", sqlHas("^UPDATE mod_den_notes SET seen = 1"))

-- ---------------------------------------------------------------- the save file keeps them
clock = clock + 10
cmd(own, ".den save home1")
local text = readFile(SAVES .. "/home1.den.json") or ""
check("the save file carries the notes, the door and the mailbox", text:find('"notes"', 1, true) and text:find("Lovely place!", 1, true)
    and text:find('"door"', 1, true) and text:find('"mailbox"', 1, true))
cmd(own, ".den mail clear")
check("mail clear empties the mailbox", #H.notes == 0 and sqlHas("^DELETE FROM mod_den_notes"))
for _, k in ipairs({ "doorx", "doory", "doorz" }) do D.settings[k] = nil end
check("the door is gone before loading", H.spot("door") == nil)
clock = clock + 10
cmd(own, ".den load home1 replace")
check("opening the file brings the notes and the door back", #H.notes == 2 and H.spot("door") and near(H.spot("door").x, door.x))
clock = clock + 10
cmd(own, ".den load home1 replace")
check("opening it again doesn't double the notes", #H.notes == 2)

-- ---------------------------------------------------------------- the launcher's Den setup: open.json names a starter place
do
    local f = assert(io.open(SAVES .. "/forest-glade.den.json", "w"))
    f:write(readFile(dir .. "/den-starters/forest-glade.den.json")); f:close()
    f = assert(io.open(SAVES .. "/open.json", "w")); f:write('{"file": "forest-glade", "id": "abc123"}'); f:close()
    D.settings.started = nil
    clock = clock + 10
    own.map, own.x, own.y, own.z = 1, DEN.x, DEN.y, GROUND
    for _, e in ipairs(luaEvents) do if e.delay == 2000 then for _ = 1, 5 do e.fn() end end end
    check("the launcher's starter place opens in the Den and counts as the pick", D.settings.started == "forest"
        and readFile(SAVES .. "/open.json") == nil)
end

-- ---------------------------------------------------------------- help, words, SQL
clear(vis)
cmd(vis, ".den")
check(".den's help names the visitor's things", said(vis, "%.den knock") and said(vis, "%.den note"))
clear(own)
cmd(own, ".den")
check(".den's help names the owner's", said(own, "%.den start forest") and said(own, "%.den stamps") and said(own, "%.den mail"))
check("no game-brand words in anything players see", (function()
    local function bad(s) local l = s:lower(); return l:find("warcraft") or l:find("blizzard") or l:find("lich king") or l:find("%f[%a]wow%f[%A]") end
    for _, m in ipairs(allMsgs) do if bad(m) then return false end end
    for _, f in ipairs({ "den-starters/forest-glade", "den-starters/dark-keep", "den-stamps/garden", "den-stamps/market", "den-stamps/inn",
        "den-stamps/campfire", "den-stamps/dock", "den-stamps/house" }) do
        if bad(readFile(dir .. "/" .. f .. ".den.json") or "") then return false end
    end
    return true end)())
check("SQL only on the scripts' own tables", (function()
    for _, s in ipairs(sqlLog) do
        local t = s:match("INTO%s+(%S+)") or s:match("^UPDATE%s+(%S+)") or s:match("FROM%s+(%S+)") or s:match("EXISTS%s+(%S+)")
        if not t or not (t:match("^mod_builder_") or t:match("^mod_den_")) then return false end
    end
    return true end)())

-- ---------------------------------------------------------------- not a Den
env.OV_DEN = nil
OV_DEN_HOME_TEST = nil
events = {}
dofile(homeFile)
check("OV_DEN unset: ov_den_home.lua does nothing", OV_DEN_HOME_TEST == nil and events[3] == nil)

os.execute("rm -rf '" .. SAVES .. "'")
realPrint(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
