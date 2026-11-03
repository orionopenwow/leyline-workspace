-- lua5.1 stream/tests/test_den_smoke.lua [lua_scripts folder]
-- The Den smoke test (owner, 31 Oct: "place buildings and paths and doodads that save to a file and can relaunch into
-- it and invite a friend"), end to end under a FAKE realm: ov_build.lua + ov_den.lua + ov_modapi.lua on a fake mod-ale
-- whose characters database is a small in-memory SQL table store (the statements the scripts send, with MySQL's
-- FLOAT precision), so a "restart" really reloads every script from the database alone. Not a real server: no
-- worldserver, no client, no relay (docs/DEN.md has the owner's real-PC test).
--   place: a building (a map object, the "buildings" catalog), doodads, a path on foot and a path by two clicks, a
--   group, a lock, a hidden prop, a size; restart: all of it comes back from the database, spawned where it was;
--   .den save: the versioned JSON file in the saves folder; .den load: fresh, replace (a backup kept), add, the ask;
--   the launcher's open.json: opened once when the owner is in the room; bad files refused or trimmed, never run;
--   invite a friend: the link from den-invite.dat, the friend lands in the room, the owner gives and takes building.
local dir = arg[1] or "realm/custom/lua_scripts"
local buildFile, denFile, modapi = dir .. "/ov_build.lua", dir .. "/ov_den.lua", dir .. "/ov_modapi.lua"

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
    OV_BUILD_NEW_HOURS = "0", OV_BUILD_SCALE_MIN = "0.1", OV_BUILD_SCALE_MAX = "5", OV_DEN_SAVES = SAVES }
os.getenv = function(k) return env[k] end
local function readFile(p) local f = io.open(p, "r"); if not f then return nil end; local t = f:read("*a"); f:close(); return t end
local function writeFile(p, t) local f = assert(io.open(p, "w")); f:write(t); f:close() end

-- ---------------------------------------------------------------- the fake characters database
local DB, unknownSql = {}, {}
local function f32(v)   -- MySQL FLOAT is single precision
    if type(v) ~= "number" or v == 0 or v ~= v then return v end
    local m, e = math.frexp(v)
    return math.ldexp(math.floor(m * 2 ^ 24 + 0.5) / 2 ^ 24, e)
end
local function splitTop(s)   -- commas outside quotes and brackets
    local out, depth, cur, q = {}, 0, {}, false
    local i = 1
    while i <= #s do
        local c = s:sub(i, i)
        if q then
            cur[#cur + 1] = c
            if c == "'" then if s:sub(i + 1, i + 1) == "'" then cur[#cur + 1] = "'"; i = i + 1 else q = false end end
        elseif c == "'" then q = true; cur[#cur + 1] = c
        elseif c == "(" then depth = depth + 1; cur[#cur + 1] = c
        elseif c == ")" then depth = depth - 1; cur[#cur + 1] = c
        elseif c == "," and depth == 0 then out[#out + 1] = table.concat(cur):match("^%s*(.-)%s*$"); cur = {}
        else cur[#cur + 1] = c end
        i = i + 1
    end
    if #cur > 0 then out[#out + 1] = table.concat(cur):match("^%s*(.-)%s*$") end
    return out
end
local function val(t)
    t = t:match("^%s*(.-)%s*$")
    if t:sub(1, 1) == "'" then return (t:sub(2, -2):gsub("''", "'")) end
    if t == "NULL" then return nil end
    return tonumber(t)
end
local function create(name, body)
    if DB[name] then return end
    local t = { cols = {}, float = {}, pk = {}, rows = {}, n = 0 }
    for _, part in ipairs(splitTop(body)) do
        local pk = part:match("^PRIMARY KEY %((.-)%)$")
        if pk then for c in pk:gmatch("[%w_]+") do t.pk[#t.pk + 1] = c end
        elseif not part:match("^UNIQUE KEY") and not part:match("^KEY ") then
            local col, ty = part:match("^([%w_]+)%s+(%u+)")
            t.cols[#t.cols + 1] = col
            if ty == "FLOAT" then t.float[col] = true end
            if part:find("AUTO_INCREMENT") then t.auto = col end
            if part:find("PRIMARY KEY") then t.pk[#t.pk + 1] = col end
        end
    end
    DB[name] = t
end
local function matches(row, where)
    if not where or where == "" then return true end
    for cond in (where .. " AND "):gmatch("(.-) AND ") do
        local c, op, v = cond:match("^%s*([%w_]+)%s*([=<>]+)%s*(.-)%s*$")
        v = val(v)
        local x = row[c]
        if op == "=" and x ~= v then return false end
        if op == ">" and not (tonumber(x) and x > v) then return false end
        if op == "<" and not (tonumber(x) and x < v) then return false end
    end
    return true
end
local function samePk(t, a, b)
    if #t.pk == 0 then return false end
    for _, c in ipairs(t.pk) do if a[c] ~= b[c] then return false end end
    return true
end
local function result(rows, cols)
    if #rows == 0 then return nil end
    local r = { i = 1 }
    local function get(i) return rows[r.i][cols[i + 1]] end
    function r:GetUInt32(i) return math.floor(tonumber(get(i)) or 0) end
    function r:GetInt32(i) return math.floor(tonumber(get(i)) or 0) end
    function r:GetString(i) local v = get(i); return v == nil and "" or tostring(v) end
    function r:GetFloat(i) return tonumber(get(i)) or 0 end
    function r:NextRow() r.i = r.i + 1; return r.i <= #rows end
    return r
end
local function sql(s)
    local name, body = s:match("^CREATE TABLE IF NOT EXISTS (%S+) %((.*)%)$")
    if name then return create(name, body) end
    local verb, tn, cols, vals = s:match("^(%u+) INTO (%S+) %((.-)%) VALUES (.*)$")
    if verb == "INSERT" or verb == "REPLACE" then
        local t = DB[tn]
        if not t then unknownSql[#unknownSql + 1] = s; return end
        local names = splitTop(cols)
        for tuple in vals:gmatch("%b()") do
            local row, vs = {}, splitTop(tuple:sub(2, -2))
            for i, c in ipairs(names) do local v = val(vs[i]); row[c] = t.float[c] and f32(v) or v end
            if t.auto and row[t.auto] == nil then t.n = t.n + 1; row[t.auto] = t.n end
            local keep = {}
            local dup = false
            for _, old in ipairs(t.rows) do
                if samePk(t, old, row) then dup = true; if verb == "INSERT" then keep[#keep + 1] = old end else keep[#keep + 1] = old end
            end
            if not (dup and verb == "INSERT") then keep[#keep + 1] = row end
            t.rows = keep
        end
        return
    end
    local ut, sets, where = s:match("^UPDATE (%S+) SET (.-) WHERE (.*)$")
    if ut then
        local t = DB[ut]
        if not t then unknownSql[#unknownSql + 1] = s; return end
        for _, row in ipairs(t.rows) do
            if matches(row, where) then
                for _, a in ipairs(splitTop(sets)) do local c, v = a:match("^([%w_]+)%s*=%s*(.*)$"); v = val(v); row[c] = t.float[c] and f32(v) or v end
            end
        end
        return
    end
    local dt, rest = s:match("^DELETE FROM (%S+)%s*(.*)$")
    if dt then
        local t = DB[dt]
        if not t then unknownSql[#unknownSql + 1] = s; return end
        local w = rest:match("^WHERE (.*)$")
        local keep = {}
        for _, row in ipairs(t.rows) do if not matches(row, w) then keep[#keep + 1] = row end end
        t.rows = keep
        return
    end
    unknownSql[#unknownSql + 1] = s
end
local function select(s)
    local cols, tn, rest = s:match("^SELECT (.-) FROM (%S+)(.*)$")
    local t = tn and DB[tn]
    if not t then unknownSql[#unknownSql + 1] = s; return nil end
    local where = rest:match("WHERE (.-)%s*ORDER BY") or rest:match("WHERE (.-)%s*LIMIT") or rest:match("WHERE (.*)$")
    local order, desc = rest:match("ORDER BY ([%w_]+)%s*(%u*)")
    local limit = tonumber(rest:match("LIMIT (%d+)") or "")
    local rows = {}
    for _, row in ipairs(t.rows) do if matches(row, where) then rows[#rows + 1] = row end end
    local mx = cols:match("^MAX%(([%w_]+)%)$")
    if mx then
        local m = 0
        for _, row in ipairs(rows) do m = math.max(m, tonumber(row[mx]) or 0) end
        return result({ { m = m } }, { "m" })
    end
    if order then table.sort(rows, function(a, b) if desc == "DESC" then return (a[order] or 0) > (b[order] or 0) end return tostring(a[order]) < tostring(b[order]) end) end
    if limit then local cut = {}; for i = 1, math.min(limit, #rows) do cut[i] = rows[i] end; rows = cut end
    local names = splitTop(cols)
    return result(rows, names)
end
local sqlLog = {}
function CharDBExecute(s) sqlLog[#sqlLog + 1] = s; sql(s) end
function CharDBQuery(s) sqlLog[#sqlLog + 1] = s; return select(s) end
function WorldDBQuery(s)
    local list = s:match("IN %(([%d,]+)%)")
    if not list or s:find("AIName") then return nil end   -- the "any object" lookups: nothing beyond the catalog here
    local rows = {}
    for id in list:gmatch("%d+") do rows[#rows + 1] = { id } end
    local r = { rows = rows, i = 1 }
    function r:GetUInt32(c) return tonumber(self.rows[self.i][c + 1]) end
    function r:NextRow() self.i = self.i + 1; return self.i <= #self.rows end
    return r
end

-- ---------------------------------------------------------------- the fake world (wiped by a restart)
local events, luaEvents, objects, nextGuid, maps = {}, {}, {}, 9000, {}
local GROUND, DX = 12.6, 16226.2
local function heightAt(x) return GROUND + 0.02 * (x - DX) end   -- the island slopes a little, east to west
function RegisterPlayerEvent(id, fn) events[id] = events[id] or {}; table.insert(events[id], fn) end
function RegisterServerEvent() end
function CreateLuaEvent(fn, delay, rep) luaEvents[#luaEvents + 1] = { fn = fn, delay = delay, rep = rep } end
local function fire(id, ...) local r; for _, fn in ipairs(events[id] or {}) do local x = fn(id, ...); if x ~= nil then r = x end end; return r end
local function mapObj(id)
    maps[id] = maps[id] or { id = id, GetWorldObject = function(self, g) local o = objects[g]; if o and o.map == self.id then return o end end,
        IsDungeon = function() return false end, GetHeight = function(_, x) return heightAt(x) end }
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
local online, allMsgs = {}, {}
function GetPlayerByName(n) for _, p in pairs(online) do if p.name:lower() == n:lower() then return p end end end
function GetPlayersInWorld() local t = {}; for _, p in pairs(online) do t[#t + 1] = p end; table.sort(t, function(a, b) return a.guid < b.guid end); return t end
local function player(over)
    local p = { msgs = {}, addon = {}, gm = 0, gmOn = false, map = 0, x = 0, y = 0, z = 0, o = 0 }
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
    function p:Teleport(m, x, y, z, o) self.map, self.x, self.y, self.z, self.o = m, x, y, z, o; return true end
    function p:SetBindPoint(x, y, z, m, a) self.bind = { x = x, y = y, z = z, map = m, area = a } end
    function p:RegisterEvent(fn) fn(1, 0, 1, self); return 1 end
    online[p.guid] = p
    return p
end
local function last(p) return p.msgs[#p.msgs] or "" end
local function said(p, pat) for _, m in ipairs(p.msgs) do if m:find(pat) then return true end end end
local function cmd(p, text) clock = clock + 6; return fire(42, p, text) end   -- each command a few seconds apart (the rates)

local T, D, DEN
-- (Re)start the realm: every script loads again; only the database (and the saves folder) survive.
local function boot()
    events, luaEvents, objects = {}, {}, {}
    package.loaded["ov_modapi"] = nil
    package.preload["ov_modapi"] = function() return dofile(modapi) end
    OVMod, OVBuild, OV_BUILD_TEST, OV_DEN_TEST = nil, nil, nil, nil
    dofile(buildFile)
    dofile(denFile)
    T, D = assert(OV_BUILD_TEST), assert(OV_DEN_TEST, "ov_den.lua loads with OV_DEN=1")
    DEN = T.DEN
end
local function restart()
    for _, p in pairs(online) do fire(4, p) end
    online = {}
    boot()
end
local function tick(n) for _ = 1, n or 1 do clock = clock + 2; for _, e in ipairs(luaEvents) do if e.delay == 2000 then e.fn() end end end end
local function decor() local l = {}; for _, p in pairs(T.props) do if p.owner ~= 0 then l[#l + 1] = p end end; table.sort(l, function(a, b) return a.id < b.id end); return l end
local function roomCount() local n = 0; for _, p in pairs(T.props) do if p.owner == 0 then n = n + 1 end end; return n end
local function liveCount() local n = 0; for _ in pairs(objects) do n = n + 1 end; return n end
-- The Den as comparable lines (positions from the Den's middle, 2 decimals; groups as "which props share one").
local function shape()
    local out, gi, gn = {}, {}, 0
    for _, p in ipairs(decor()) do
        local g = T.X.groupOf[p.id]
        if g and not gi[g] then gn = gn + 1; gi[g] = gn end
        out[#out + 1] = string.format("%d %.2f %.2f %.2f %.2f s%.2f p%.2f r%.2f %s%s g%s", p.entry, p.x - DEN.x, p.y - DEN.y, p.z, p.o,
            p.s or 1, p.pt or 0, p.rl or 0, T.X.locked[p.id] and "L" or "-", T.X.hidden[p.id] and "H" or "-", g and gi[g] or "-")
    end
    table.sort(out)
    return table.concat(out, "\n")
end
local function pathsCount() local n = 0; for _ in pairs(T.X.paths) do n = n + 1 end; return n end

boot()
check("a Den: the room 32 yards wide (den.env OV_DEN_SIZE=16)", DEN and DEN.half == 16)
check("buildings are in a Den's catalog", T.X.known(19006) and T.X.known(19006).cat == "buildings" and T.costOf(19006) == 10)

-- ---------------------------------------------------------------- the owner's first login: the white room
local own = player({ guid = 1, name = "Owner", gm = 3 })
fire(30, own); fire(3, own)
local ROOM = roomCount()
check("first login: in the room, the room goes up", own.map == 1 and ROOM > 40 and said(own, "Welcome to the Den"))

-- ---------------------------------------------------------------- place: a building, doodads, paths
own.x, own.y, own.z = DEN.x, DEN.y + DEN.half + 8, heightAt(DEN.x)     -- out in the yard, north of the room
cmd(own, "build spawn house")
local house = T.props[T.selected[1]]
check("a building: the House (a map object) in the yard", house and house.entry == 19006 and T.live[house.id] and objects[T.live[house.id]].entry == 19006)
own.x, own.y, own.z = DEN.x + 3, DEN.y + 3, GROUND
cmd(own, "build spawn chair")
local chair = T.props[T.selected[1]]
own.x = DEN.x - 3
cmd(own, "build spawn table")
local tbl = T.props[T.selected[1]]
own.y = DEN.y - 4
cmd(own, "build spawn lantern")
local lantern = T.props[T.selected[1]]
cmd(own, "build spawn barrel")
local barrel = T.props[T.selected[1]]
check("doodads: chair, table, lantern, barrel", chair and tbl and lantern and barrel and #decor() == 5)
-- a path on foot: start, walk, end, 4 yards apart, down the island's slope
own.x, own.y, own.z = DEN.x - 10, DEN.y + DEN.half + 4, heightAt(DEN.x - 10)
cmd(own, "build path platformsmall from")
check(".build path <prop> from: the start marked", last(own):find("Path start marked"))
own.x = DEN.x + 10
cmd(own, "build path platformsmall to 4")
local walk = {}
for _, id in ipairs(T.group[1] or {}) do walk[#walk + 1] = T.props[id] end
table.sort(walk, function(a, b) return a.x < b.x end)
check(".build path ... to 4: six pieces 4 yards apart, from one end to the other", #walk == 6 and math.abs(walk[1].x - (DEN.x - 10)) < 1e-6
    and math.abs(walk[6].x - (DEN.x + 10)) < 1e-6 and math.abs(walk[2].x - walk[1].x - 4) < 1e-6)
check("each piece down on the ground (the slope), facing along the path", (function()
    for _, p in ipairs(walk) do if math.abs(p.z - heightAt(p.x)) > 1e-6 or math.abs(p.o) > 1e-6 then return false end end
    return true end)())
check("the path is one group and is kept for the save file", T.X.groupOf[walk[1].id] and T.X.groupOf[walk[1].id] == T.X.groupOf[walk[6].id]
    and T.X.paths[T.X.groupOf[walk[1].id]] and T.X.paths[T.X.groupOf[walk[1].id]].every == 4 and #DB.mod_builder_paths.rows == 1)
check("the message", said(own, "Path laid: 6 props"))
-- a path by two clicks (edit mode sends the ends): lanterns down the room, on the room's floor
own.x, own.y = DEN.x, DEN.y
cmd(own, string.format("build path lantern %.2f %.2f %.2f %.2f %.2f %.2f 5", DEN.x - 10, DEN.y - 10, GROUND + 3, DEN.x - 10, DEN.y + 10, GROUND + 3))
local clicked = {}
for _, id in ipairs(T.group[1] or {}) do clicked[#clicked + 1] = T.props[id] end
check("a path by two clicks: 5 lanterns on the room's floor, facing north", #clicked == 5 and (function()
    for _, p in ipairs(clicked) do if math.abs(p.z - DEN.floorZ) > 1e-6 or math.abs(p.o - math.pi / 2) > 1e-6 then return false end end
    return true end)())
cmd(own, "build undo")
check("undo takes the whole path back, in one step", #decor() == 11 and last(own):find("the path %(5 props%) is gone"))
cmd(own, "build redo")
check("redo lays it again", #decor() == 16)
-- the path's limits and checks
cmd(own, string.format("build path lantern %.2f %.2f %.2f %.2f %.2f %.2f 1", DEN.x - 20, DEN.y, GROUND, DEN.x + 25, DEN.y, GROUND))
check("too many pieces: refused, says why", last(own):find("needs 46 pieces") and #decor() == 16)
cmd(own, "build path lantern from")
own.x = DEN.x + 300
cmd(own, "build path lantern to")
check("the far end outside the Den: nothing laid", #decor() == 16)
own.x = DEN.x
cmd(own, "build path nosuchprop from")
check("an unknown prop: the usage", last(own):find("Usage: "))
cmd(own, string.format("build path lantern %.2f %.2f %.2f %.2f %.2f %.2f 99", DEN.x, DEN.y, GROUND, DEN.x + 5, DEN.y, GROUND))
check("spacing 1 to 20 yards", last(own):find("1 to 20 yards") and #decor() == 16)
cmd(own, "build path lantern 1 2 3 4 5 6")
check("ends far from you: refused", last(own):find("too far") and #decor() == 16)

-- groups, a lock, a hidden prop, a size
cmd(own, "build pick " .. T.live[chair.id])
cmd(own, "build pick " .. T.live[tbl.id] .. " add")
cmd(own, "build group")
cmd(own, "build pick " .. T.live[lantern.id])
cmd(own, "build lock")
cmd(own, "build pick " .. T.live[barrel.id])
cmd(own, "build hide")
cmd(own, "build pick " .. T.live[tbl.id])
cmd(own, "build ungroup")      -- and group again with the size set, to see the group survive a change
cmd(own, "build pick " .. T.live[chair.id])
cmd(own, "build pick " .. T.live[tbl.id] .. " add")
cmd(own, "build group")
cmd(own, "build pick " .. T.live[tbl.id])
cmd(own, "build setscale 1.5")
tbl = T.props[tbl.id]
check("group, lock, hide, size", T.X.groupOf[chair.id] and T.X.groupOf[chair.id] == T.X.groupOf[tbl.id] and T.X.locked[lantern.id]
    and T.X.hidden[barrel.id] and not T.live[barrel.id] and tbl.s == 1.5)
local S1, N1 = shape(), #decor()

-- ---------------------------------------------------------------- restart: everything comes back from the database
restart()
check("restart: the props are read back from the database", #decor() == N1 and roomCount() == ROOM)
check("restart: nothing in the world until someone is near", liveCount() == 0)
own = player({ guid = 1, name = "Owner", gm = 3, map = 1, x = DEN.x, y = DEN.y, z = GROUND + 1 })
fire(3, own)
check("restart: the owner logs in inside the Den, the room isn't built twice", roomCount() == ROOM)
check("restart: every prop spawned again (the hidden one stays out)", liveCount() == N1 + ROOM - 1 and not T.live[barrel.id])
check("restart: positions, turns, size, lock, hidden and groups as they were (FLOAT precision)", (function()
    local a, b = {}, {}
    for l in S1:gmatch("[^\n]+") do a[#a + 1] = l end
    for l in shape():gmatch("[^\n]+") do b[#b + 1] = l end
    if #a ~= #b then return false end
    for i = 1, #a do if a[i] ~= b[i] then realPrint("  was: " .. a[i] .. "\n  now: " .. b[i]); return false end end
    return true end)())
check("restart: the spawned objects stand where the props are", (function()
    for _, p in ipairs(decor()) do
        local o = T.live[p.id] and objects[T.live[p.id]]
        if not T.X.hidden[p.id] and not (o and math.abs(o.x - p.x) < 1e-6 and math.abs(o.z - p.z) < 1e-6 and o.entry == p.entry) then return false end
    end
    return true end)())
check("restart: the paths are kept", pathsCount() == 2)

-- ---------------------------------------------------------------- .den save: the file
local friend = player({ guid = 2, name = "Bea", map = 0, x = 0, y = 0, z = 0 })
cmd(own, "den save house")
local text = readFile(SAVES .. "/house.den.json")
local doc = text and OVBuild.JsonDecode(text, 600000)
check(".den save: house.den.json in the saves folder", doc and last(own):find("Saved to a file too %(" .. N1 .. " props%)"))
check("the file: format, version, name, time, the room", doc and doc.format == "leyline-den" and doc.version == 1 and doc.name == "house"
    and doc.saved:match("^%d%d%d%d%-%d%d%-%d%dT") and doc.room.look == "white" and doc.room.size == 16)
check("the file: every prop with entry, place from the middle, turn, flags, groups", doc and #doc.props == N1 and (function()
    local seen = {}
    for _, e in ipairs(doc.props) do
        seen[e.entry] = (seen[e.entry] or 0) + 1
        if not (e.x and e.y and e.z and e.yaw) then return false end
    end
    local hidden, locked, groups = 0, 0, {}
    for _, e in ipairs(doc.props) do
        if e.hidden then hidden = hidden + 1 end
        if e.locked then locked = locked + 1 end
        if e.group then groups[e.group] = (groups[e.group] or 0) + 1 end
    end
    local g = 0; for _ in pairs(groups) do g = g + 1 end
    return seen[19006] == 1 and seen[190023] == 6 and hidden == 1 and locked == 1 and g == 3 end)())
check("the file: the paths, from the Den's middle and floor", doc and #doc.paths == 2 and doc.paths[1].every and #doc.paths[1].from == 3)
check("the file: the size kept", text and text:find('"scale":1.5'))
check("the file: no room pieces in it (the room is its look)", doc and (function()
    for _, e in ipairs(doc.props) do if e.entry == D.THEMES.white.floor or e.entry == D.THEMES.white.light then return false end end
    return true end)())
check("mod_den_files remembers it", DB.mod_den_files.rows[1] and DB.mod_den_files.rows[1].name == "house")
cmd(own, "den files")
check(".den files", last(own):find("house %(" .. N1 .. "%)"))

-- ---------------------------------------------------------------- .den load: the ask, fresh, replace, add
cmd(own, "den load house")
check(".den load with decor here: asks, changes nothing", last(own):find("replace") and last(own):find("add") and #decor() == N1)
own.x, own.y = DEN.x, DEN.y
for _ = 1, 3 do cmd(own, "build unlock"); cmd(own, "build selall 40"); cmd(own, "build unlock"); cmd(own, "build clear") end
check("cleared the Den (the room stays)", #decor() == 0 and roomCount() == ROOM)
cmd(own, "den load house")
check(".den load into an empty Den: everything back as it was", shape() == S1 and last(own):find("Undo") == nil and said(own, "Opened 'house': " .. N1 .. " props"))
check("and spawned (hidden stays out)", (function() local n = 0; for _, p in ipairs(decor()) do if T.live[p.id] then n = n + 1 end end; return n == N1 - 1 end)())
check("the paths again", pathsCount() == 2)
cmd(own, "den load house replace")
check(".den load <name> replace: the same Den, not twice", shape() == S1 and #decor() == N1)
local backup = readFile(SAVES .. "/backup.den.json")
check("replace keeps a copy of what was there: backup.den.json", backup and OVBuild.JsonDecode(backup, 600000).name == "backup"
    and said(own, "saved as 'backup'"))
check("old paths of removed props are dropped, new ones kept", pathsCount() == 2)
cmd(own, "den load house add")
check(".den load <name> add: next to what's there", #decor() == 2 * N1)
cmd(own, "build undo")
check("undo takes the added Den back", #decor() == N1)
cmd(own, "den load house replace")
cmd(own, "build undo")
cmd(own, "build undo")
check("undo after replace: what was there comes back", #decor() == N1)

-- after a load, a restart keeps it (the file's props are the realm's own props now)
local S2 = shape()
restart()
own = player({ guid = 1, name = "Owner", gm = 3, map = 1, x = DEN.x, y = DEN.y, z = GROUND + 1 })
fire(3, own)
check("restart after a load: the same Den", shape() == S2)

-- ---------------------------------------------------------------- the launcher's "Open a saved Den" (open.json)
for _ = 1, 3 do cmd(own, "build selall 40"); cmd(own, "build unlock"); cmd(own, "build clear") end
check("cleared again", #decor() == 0)
writeFile(SAVES .. "/open.json", '{"file": "house", "id": "k7Q2xz"}')
own.msgs = {}
tick(4)
check("open.json: not before the next look (every 10 s)", #decor() == 0)
tick(1)
check("open.json: the Den opens with the file once the owner is in the room", shape() == S1 and said(own, "Opening 'house' from the launcher"))
check("open.json: removed, and its id kept so it opens once", readFile(SAVES .. "/open.json") == nil and D.settings.opened == "k7Q2xz")
writeFile(SAVES .. "/open.json", '{"file": "house", "id": "k7Q2xz"}')
tick(10)
check("the same id again: not opened twice", #decor() == N1)
os.remove(SAVES .. "/open.json")
-- relaunch: the realm is off, the launcher writes open.json, the realm starts, the owner logs in
restart()
writeFile(SAVES .. "/open.json", '{"file": "nofile", "id": "z9"}')
own = player({ guid = 1, name = "Owner", gm = 3, map = 1, x = DEN.x, y = DEN.y, z = GROUND + 1 })
fire(3, own)
check("relaunch into a missing file: says so, the Den stays", said(own, "isn't there") and #decor() == N1)
writeFile(SAVES .. "/open.json", '{"file": "house", "id": "r2"}')
for _ = 1, 3 do cmd(own, "build selall 40"); cmd(own, "build unlock"); cmd(own, "build clear") end
restart()
own = player({ guid = 1, name = "Owner", gm = 3, map = 1, x = DEN.x, y = DEN.y, z = GROUND + 1 })
fire(3, own)
check("relaunch into a file: the Den opens with it at login", shape() == S1)
check("a visitor's presence never opens it", (function()
    writeFile(SAVES .. "/open.json", '{"file": "house", "id": "v1"}')
    local vis = player({ guid = 7, name = "Vis", map = 1, x = DEN.x, y = DEN.y, z = GROUND })
    online[1] = nil
    tick(5)
    online[1] = own
    online[7] = nil
    os.remove(SAVES .. "/open.json")
    return #decor() == N1 and D.settings.opened == "r2" and vis end)())

-- ---------------------------------------------------------------- bad files: refused or trimmed, never run
local function try(name, body)
    writeFile(SAVES .. "/" .. name .. ".den.json", body)
    own.msgs = {}
    cmd(own, "den load " .. name .. " add")
    return last(own), #decor()
end
local base = '{"format":"leyline-den","version":1,"name":"t","room":{"look":"white","size":16},"paths":[],"props":%s}'
local n0 = #decor()
local m = try("bad1", "return { format = 'leyline-den' }; os.exit(1)")
check("not JSON (a script): refused, nothing run", m:find("can't be read") and #decor() == n0)
m = try("bad2", '{"format":"something","version":1,"props":[]}')
check("not a Den file: refused", m:find("isn't a Den file"))
m = try("bad3", '{"format":"leyline-den","version":2,"props":[]}')
check("a newer version: refused, says to update", m:find("newer"))
m = try("bad4", base:format('[{"entry":"2413","x":0,"y":0,"z":0}]'))
check("an entry as text: refused", m:find("has no entry"))
m = try("bad5", base:format('[{"entry":2413,"x":0,"y":0,"z":0,"locked":"yes"}]'))
check("bad flags: refused", m:find("bad flags"))
m = try("bad6", base:format('[{"entry":999999,"x":1,"y":1,"z":0},{"entry":2413,"x":500,"y":0,"z":0},{"entry":2413,"x":2,"y":2,"z":0,"scale":99,"pitch":10}]'))
local last1 = decor()[#decor()]
check("unknown entries and props outside the Den left out, the rest placed (size kept in limits)", said(own, "2 left out")
    and #decor() == n0 + 1 and last1.entry == 2413 and last1.s == 5 and math.abs(last1.pt - math.rad(10)) < 1e-6)
cmd(own, "build undo")
local many = {}
for i = 1, 31 do many[i] = string.format('{"entry":19006,"x":%d,"y":25,"z":0}', i % 7 - 3) end
m = try("bad7", base:format("[" .. table.concat(many, ",") .. "]"))
check("over the yard's budget: refused whole, says why", m:find("yard's 300 decor points") and #decor() == n0)
local huge = {}
for i = 1, 801 do huge[i] = '{"entry":2413,"x":0,"y":0,"z":0}' end
m = try("bad8", base:format("[" .. table.concat(huge, ",") .. "]"))
check("over 800 props: refused", m:find("up to 800 props"))
m = try("bad9", base:format('[{"entry":2413,"x":1,"y":1,"z":0}]'):gsub('"name":"t"', '"name":"x\\"); os.exit(1) --"'))
check("text in a file is only text", said(own, "Opened 'bad9'") and #decor() == n0 + 1)
cmd(own, "build undo")
check("a friend can't open the owner's files", (function()
    friend.map, friend.x, friend.y, friend.z = 1, DEN.x, DEN.y, GROUND
    cmd(friend, "den load house replace")
    return #decor() == n0 and not said(friend, "Opened") end)())
cmd(own, "den load ../../etc/passwd")
check("a file name is letters, digits, - and _ only", not said(own, "Opened 'etcpasswd'") and #decor() == n0)

-- ---------------------------------------------------------------- invite a friend
writeFile(SAVES .. "/den-invite.dat", 'return { invite = "https://relay.example.app:8443/join/abcdefghij234567#ABCDEFGHJKMNPQRSTVWX" }\n')
env.OV_DEN_INVITE_FILE = SAVES .. "/den-invite.dat"
boot()
own = online[1]
own.msgs, own.addon = {}, {}
cmd(own, "den invite")
check("Invite a friend: the owner gets the link from den-invite.dat (a relay with a port too)", said(own, "relay.example.app:8443/join/abcdefghij234567#")
    and (function() for _, a in ipairs(own.addon) do if a:find("^wuden\tinvite\t1/1\thttps://") then return true end end end)())
-- the friend opens the link, makes an account on the join page and logs in for the first time: into the room
local pal = player({ guid = 5, name = "Pal", map = 0, x = -8949, y = -132, z = 83 })
fire(30, pal); fire(3, pal)
check("the friend lands in the Den", pal.map == 1 and math.abs(pal.x - DEN.x) < 0.01 and said(pal, "Welcome to the Den"))
cmd(pal, "build spawn chair")
check("and only looks at first", last(pal):find("builder role"))
cmd(own, "den allow Pal build")
check("the owner lets them build", said(pal, "You can build here now"))
pal.x, pal.y, pal.z = DEN.x - 8, DEN.y - 8, GROUND
cmd(pal, "build path platformsmall from")
pal.x = DEN.x + 8
cmd(pal, "build path platformsmall to 4")
check("the friend lays a path", said(pal, "Path laid: 5 props"))
cmd(own, "den allow Pal view")
cmd(pal, "build undo")
check("view only again: the friend can't undo it any more", last(pal):find("Can't undo") or last(pal):find("builder role"))

-- ---------------------------------------------------------------- packs: a Den file lists the packs it needs
check("a Den with no packs: its file has no packs list", doc and doc.packs == nil)
writeFile(SAVES .. "/packs.json", '{"packs": [{"id": "goldshire", "title": "Goldshire", "version": 1}]}')
writeFile(SAVES .. "/trip.den.json", '{"format": "leyline-den", "version": 1, "name": "trip", "props": [], '
    .. '"packs": ["goldshire", "mounts", "Bad Id", "goldshire"]}')
own.x, own.y, own.z = DEN.x, DEN.y, GROUND
cmd(own, "den load trip add")
check("opening a file with packs: the ones not made here are named, with how to make them",
    said(own, "packs you haven't made yet: mounts%.") and said(own, "leyline pack build mounts"))
check("the Den remembers its packs (good ids only, once each)", D.settings.packs == "goldshire.mounts")
cmd(own, "den save trip2")
local doc2 = OVBuild.JsonDecode(readFile(SAVES .. "/trip2.den.json") or "", 1000000)
check("its file lists them for a friend", doc2 and doc2.version == 1 and doc2.packs and #doc2.packs == 2
    and doc2.packs[1] == "goldshire" and doc2.packs[2] == "mounts")
writeFile(SAVES .. "/packs.json", '{"packs": [{"id": "goldshire"}, {"id": "mounts"}]}')
local before = #own.msgs
cmd(own, "den load trip2 add")
check("every pack made: nothing to say about packs", (function()
    for i = before + 1, #own.msgs do if own.msgs[i]:find("packs you haven't made") then return false end end
    return true end)())

-- ---------------------------------------------------------------- the fake database saw only what it knows
check("every SQL statement was one the fake database understands", #unknownSql == 0 or (function() for _, s in ipairs(unknownSql) do realPrint("  ?? " .. s) end end)())
check("SQL only on the scripts' own tables", (function()
    for _, s in ipairs(sqlLog) do
        local t = s:match("INTO%s+(%S+)") or s:match("^UPDATE%s+(%S+)") or s:match("FROM%s+(%S+)") or s:match("EXISTS%s+(%S+)")
        if not t or not (t:match("^mod_builder_") or t:match("^mod_den_")) then return false end
    end
    return true end)())
local BANNED = { "world of warcraft", "warcraft", "lich king", "blizzard" }
check("no game-brand names in anything players saw", (function()
    for _, msg in ipairs(allMsgs) do
        local l = msg:lower()
        for _, w in ipairs(BANNED) do if l:find(w, 1, true) then return false end end
        if l:find("%f[%a]wow%f[%A]") then return false end
    end
    return true end)())

os.execute("rm -rf '" .. SAVES .. "'")
realPrint(string.format("\n%d/%d passed", pass, pass + fail))
if fail > 0 then os.exit(1) end
