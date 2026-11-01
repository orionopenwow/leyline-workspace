-- ov_den_home.lua: a Den's starter places, stamps, front door and mailbox (mod-ale, 3.3.5a). Only with OV_DEN=1,
-- next to ov_den.lua (the room, roles, save files) and ov_build.lua (every prop placed and checked). docs/DEN.md.
--
--   .den start                         (owner) the starter places: Forest glade, Dark keep, Empty
--   .den start forest|keep|empty [replace]   puts one in the Den (with decor there: asks; replace keeps a "backup")
--   .den stamps                        the stamps: garden, market, campfire, dock, house
--   .den stamp <name> [degrees]        (builders) marks where it goes, at your feet, facing your way: stakes at its
--                                      corners and one ahead (nothing placed yet)
--   .den stamp turn [degrees]          turns the marked stamp (15 by default; it stays on 15 degree steps)
--   .den stamp here | place | cancel   moves the mark to you / puts the stamp down (one undo) / takes the mark away
--   .den stamp <name> now [degrees]    puts it down at once
--   .den door | .den mailbox           (owner) the front door / the mailbox is where you stand (a house stamp and the
--                                      starter places set both)
--   .den knock                         at the front door: tells the owner (they hear a knock); nobody home: leave a note
--   .den note <text>                   a note in the Den's mailbox (anyone here, one a minute, up to 200 letters)
--   .den mail [clear]                  (owner) the notes, newest first; clear empties the mailbox
--   .den visit                         to the front door (or the room's doorway)
--
-- Files (read only, never run; JSON read by OVBuild.JsonDecode): <scripts>/den-starters/<name>.den.json (also copied
-- to the kit's state/dens by setup.py, where the owner's own copy wins) and <scripts>/den-stamps/<name>.den.json.
-- A stamp's props are relative to where you stand: x ahead, y to your left, z from the ground; "role": "door",
-- "mailbox" or "house" marks a piece. Every stamp goes down through OVBuild.ImportPrefab (the builder role, the
-- Den, budgets, the paste rate, one undo entry), each piece set on the ground or floor under it (within 8 yards).
-- Notes: mod_den_notes; the save file carries them ("notes") and the door and mailbox ("door", "mailbox").
-- Addon (WrathUnleashedUI/DenHome.lua), OVMOD mod "wuden":
--   client -> server   home                       send me the home line
--                      note    <text>             a note (the text as typed; cleaned here)
--   server -> client   home    started=<key|->;door=0|1;mailbox=0|1;mail=<unread>;role=<role>;stamp=<name|->;stamps=<a,b>
--                      start   forest,Forest glade;keep,Dark keep;empty,Empty      the owner's first visit: pick one
--                      knock   <name>             someone is at your door
--                      mail    <id>;<from>;<at>;<text>    one note (after .den mail)

if os.getenv("OV_DEN") ~= "1" then return end

local TAG = "|cffc8a2ff[Den]|r "
local MOD = "wuden"
local SHIPPED = os.getenv("OV_DEN_SHIPPED") or "/azerothcore/lua_scripts"
local SAVES = os.getenv("OV_DEN_SAVES") or "/azerothcore/dens"

local H = {
    STARTERS = { { key = "forest", file = "forest-glade", name = "Forest glade" }, { key = "keep", file = "dark-keep", name = "Dark keep" },
        { key = "empty", name = "Empty" } },
    ALIASES = { forest = "forest", glade = "forest", ["forest-glade"] = "forest", keep = "keep", dark = "keep", ["dark-keep"] = "keep",
        empty = "empty", none = "empty", plain = "empty" },
    STAMPS = { "garden", "market", "campfire", "dock", "house" },
    STAMP_MAX = 60, STAMP_SPAN = 40, FILE_MAX = 512 * 1024,
    MARKER = 180007,          -- Excavation Stake (the catalog's): the stamp's corners, spawned unsaved, never a prop
    MARK_KEEP = 120,          -- seconds a mark stays without a word
    KNOCK_EVERY = 30, NOTE_EVERY = 60, NOTE_LEN = 200, NOTES_MAX = 100, DOOR_RANGE = 15, GROUND_RANGE = 8,
    pending = {}, lastKnock = {}, lastNote = {}, notes = {}, offered = {}, cache = {},
}

local OVMod
do
    local ok, m = pcall(function() return require("ov_modapi") end)
    if ok and type(m) == "table" then OVMod = m end
    if not OVMod and type(rawget(_G, "OVMod")) == "table" then OVMod = rawget(_G, "OVMod") end
end

local function say(player, text) player:SendBroadcastMessage(TAG .. text) end
local function send(player, name, data) if OVMod and player then pcall(OVMod.Send, player, MOD, name, data) end end
local function try(obj, method, ...)
    if not obj then return nil end
    local ok, fn = pcall(function() return obj[method] end)
    if not ok or type(fn) ~= "function" then return nil end
    local ok2, a, b = pcall(fn, obj, ...)
    if ok2 then return a, b end
    return nil
end
local function finite(v) return type(v) == "number" and v == v and v ~= math.huge and v ~= -math.huge end
local function query(sql)
    if not CharDBQuery then return nil end
    local ok, q = pcall(CharDBQuery, sql)
    if ok then return q end
    return nil
end
local function exec(sql) if CharDBExecute then pcall(CharDBExecute, sql) end end

-- ov_den.lua and ov_build.lua may load before or after this file: looked up at each use.
local function D() local d = rawget(_G, "OVDen"); return (type(d) == "table" and d.Files) and d or nil end
local function B() local b = rawget(_G, "OVBuild"); return (type(b) == "table" and b.DEN) and b or nil end
local function den() local b = B(); return b and b.DEN end
local function role(player) local d = D(); return d and d.role(player) or "visitor" end
local function isOwner(player) local d = D(); return d and d.isOwner(player) or false end
local function setting(k) local d = D(); return d and d.settings[k] end
local function set(k, v) local d = D(); if d then d.setSetting(k, v) end end

------------------------------------------------------------------------------------------------
-- files: read as text, decoded by the realm's strict JSON reader, checked here; nothing in them is ever run
------------------------------------------------------------------------------------------------
function H.readText(path)
    local f = io and io.open and io.open(path, "r")
    if not f then return nil end
    local text = f:read(H.FILE_MAX + 1) or ""
    f:close()
    if #text > H.FILE_MAX then return nil end
    return (text:gsub("^\239\187\191", ""))
end

function H.decode(text)
    local b = B()
    if not (b and text) then return nil end
    local doc = b.JsonDecode(text, H.FILE_MAX)
    if type(doc) ~= "table" or doc.format ~= "leyline-den" or type(doc.props) ~= "table" then return nil end
    return doc
end

-- A stamp: { name, title, line, props = { { entry, x, y, z, yaw, role } } }, checked; cached once read.
function H.stamp(name)
    name = tostring(name or ""):lower()
    local known = false
    for _, s in ipairs(H.STAMPS) do if s == name then known = true end end
    if not known then return nil end
    if H.cache[name] then return H.cache[name] end
    local doc = H.decode(H.readText(SHIPPED .. "/den-stamps/" .. name .. ".den.json"))
    if not doc or #doc.props < 1 or #doc.props > H.STAMP_MAX then return nil end
    local out = { name = name, title = type(doc.title) == "string" and doc.title:sub(1, 40) or name,
        line = type(doc.line) == "string" and doc.line:sub(1, 120) or "", props = {} }
    for _, e in ipairs(doc.props) do
        if type(e) ~= "table" then return nil end
        local x, y, z, yaw = tonumber(e.x), tonumber(e.y), tonumber(e.z or 0), tonumber(e.yaw or 0)
        local entry = tonumber(e.entry)
        if not (entry and finite(x) and finite(y) and finite(z) and finite(yaw)) then return nil end
        if math.abs(x) > H.STAMP_SPAN or math.abs(y) > H.STAMP_SPAN or math.abs(z) > H.STAMP_SPAN then return nil end
        out.props[#out.props + 1] = { entry = entry, x = x, y = y, z = z, yaw = yaw,
            role = (e.role == "door" or e.role == "mailbox" or e.role == "house") and e.role or nil }
    end
    H.cache[name] = out
    return out
end

-- A starter place's file: the owner's copy in the saves folder first (setup.py puts the shipped one there), else the
-- shipped one. Returns doc, the name it was read under.
function H.starterDoc(s)
    local doc = H.decode(H.readText(SAVES .. "/" .. s.file .. ".den.json"))
    if doc then return doc end
    return H.decode(H.readText(SHIPPED .. "/den-starters/" .. s.file .. ".den.json"))
end

------------------------------------------------------------------------------------------------
-- the stamp's math: a local offset (x ahead, y left) turned by o, around (x0, y0); facings on 15 degree steps
------------------------------------------------------------------------------------------------
local STEP = math.pi / 12
function H.step(o) return (math.floor((o or 0) / STEP + 0.5) * STEP) % (2 * math.pi) end
function H.turn(dx, dy, o)
    local c, s = math.cos(o), math.sin(o)
    return dx * c - dy * s, dx * s + dy * c
end

-- The world pieces of a stamp marked at p { x, y, z, o }: { entry, x, y, z, o, role }.
function H.pieces(st, p)
    local out = {}
    for _, e in ipairs(st.props) do
        local wx, wy = H.turn(e.x, e.y, p.o)
        out[#out + 1] = { entry = e.entry, x = p.x + wx, y = p.y + wy, z = p.z + e.z, o = (p.o + math.rad(e.yaw)) % (2 * math.pi), role = e.role }
    end
    return out
end

-- The mark: four stakes at the stamp's corners (its pieces' box, 1 yard out) and one ahead of its middle.
function H.markPoints(st, p)
    local x1, y1, x2, y2 = math.huge, math.huge, -math.huge, -math.huge
    for _, e in ipairs(st.props) do
        if e.x < x1 then x1 = e.x end
        if e.x > x2 then x2 = e.x end
        if e.y < y1 then y1 = e.y end
        if e.y > y2 then y2 = e.y end
    end
    x1, y1, x2, y2 = x1 - 1, y1 - 1, x2 + 1, y2 + 1
    local out = {}
    for _, c in ipairs({ { x1, y1 }, { x2, y1 }, { x2, y2 }, { x1, y2 }, { x2 + 1.5, (y1 + y2) / 2 } }) do
        local wx, wy = H.turn(c[1], c[2], p.o)
        out[#out + 1] = { x = p.x + wx, y = p.y + wy }
    end
    return out
end

------------------------------------------------------------------------------------------------
-- the mark (a ghost before placing): stakes spawned unsaved, never props; gone on place, cancel or after a while
------------------------------------------------------------------------------------------------
function H.unmark(player, pend)
    if not pend then return end
    local map = (player and try(player, "GetMap")) or (GetMapById and pend.map and GetMapById(pend.map))
    for _, g in ipairs(pend.stakes or {}) do
        local go = map and try(map, "GetWorldObject", g)
        if go and try(go, "RemoveFromWorld", false) == nil then try(go, "Despawn") end
    end
    pend.stakes = {}
end

function H.mark(player, pend)
    H.unmark(player, pend)
    local st = H.stamp(pend.name)
    if not (st and PerformIngameSpawn) then return 0 end
    local b = B()
    for _, m in ipairs(H.markPoints(st, pend)) do
        local z = (b and b.FloorAt and b.FloorAt(player, m.x, m.y, pend.z)) or pend.z
        if math.abs(z - pend.z) > H.GROUND_RANGE then z = pend.z end
        local ok, go = pcall(PerformIngameSpawn, 2, H.MARKER, pend.map, 0, m.x, m.y, z, pend.o, false, 0, 1)
        if ok and go then pend.stakes[#pend.stakes + 1] = try(go, "GetGUID") end
    end
    pend.at = os.time()
    return #pend.stakes
end

local function markText(pend)
    return string.format("Marked: %s, facing %d degrees.", (H.stamp(pend.name) or {}).title or pend.name, math.floor(math.deg(pend.o) + 0.5) % 360)
end

function H.sweep()
    local now = os.time()
    for g, pend in pairs(H.pending) do
        if now - (pend.at or 0) > H.MARK_KEEP then
            H.unmark(nil, pend)
            H.pending[g] = nil
        end
    end
end

------------------------------------------------------------------------------------------------
-- door and mailbox: kept relative to the Den's middle and floor (mod_den_settings), so they move with the Den
------------------------------------------------------------------------------------------------
function H.spot(kind)
    local d = den()
    local x, y, z = tonumber(setting(kind .. "x") or ""), tonumber(setting(kind .. "y") or ""), tonumber(setting(kind .. "z") or "")
    if not (d and x and y and z) then return nil end
    local fz = D() and D().floorZ() or d.z
    return { x = d.x + x, y = d.y + y, z = fz + z }
end

function H.setSpot(kind, x, y, z)
    local d = den()
    if not d then return end
    local fz = D() and D().floorZ() or d.z
    set(kind .. "x", string.format("%.2f", x - d.x))
    set(kind .. "y", string.format("%.2f", y - d.y))
    set(kind .. "z", string.format("%.2f", z - fz))
end

-- The pieces marked door / mailbox in a list of world pieces set the Den's spots (a house: the door is its door).
function H.takeRoles(list)
    for _, e in ipairs(list) do
        if e.role == "door" or e.role == "mailbox" then H.setSpot(e.role, e.x, e.y, e.z) end
    end
end

------------------------------------------------------------------------------------------------
-- notes (the mailbox): mod_den_notes, and the save file's "notes"
------------------------------------------------------------------------------------------------
exec("CREATE TABLE IF NOT EXISTS mod_den_notes (id INT UNSIGNED NOT NULL PRIMARY KEY, sender VARCHAR(12) NOT NULL, "
    .. "text VARCHAR(200) NOT NULL, at INT UNSIGNED NOT NULL DEFAULT 0, seen TINYINT UNSIGNED NOT NULL DEFAULT 0)")
H.nextNote = 0
do
    local q = query("SELECT id, sender, text, at, seen FROM mod_den_notes ORDER BY id")
    if q then
        repeat
            local n = { id = q:GetUInt32(0), from = q:GetString(1), text = q:GetString(2), at = q:GetUInt32(3), seen = q:GetUInt32(4) == 1 }
            H.notes[#H.notes + 1] = n
            if n.id > H.nextNote then H.nextNote = n.id end
        until not q:NextRow()
    end
end

-- A note's text: printable letters only, no colour codes or links, no quotes or backslashes for SQL; nil when empty.
function H.cleanNote(s)
    s = tostring(s or ""):gsub("|", ""):gsub("[%c\\]", " "):gsub("'", "\226\128\153"):gsub("%s+", " ")
    s = s:gsub("^ ", ""):gsub(" $", "")
    if #s > H.NOTE_LEN then s = s:sub(1, H.NOTE_LEN) end
    s = s:gsub("[\192-\255][\128-\191]*$", function(t) local n = t:byte(1) >= 240 and 4 or t:byte(1) >= 224 and 3 or 2; return #t == n and t or "" end)
    return s ~= "" and s or nil
end

function H.addNote(from, text, at, seen)
    H.nextNote = H.nextNote + 1
    local n = { id = H.nextNote, from = (tostring(from or ""):gsub("[^%w]", "")):sub(1, 12), text = text, at = at or os.time(), seen = seen and true or false }
    H.notes[#H.notes + 1] = n
    exec(string.format("INSERT INTO mod_den_notes (id, sender, text, at, seen) VALUES (%d, '%s', '%s', %d, %d)", n.id, n.from, n.text, n.at, n.seen and 1 or 0))
    while #H.notes > H.NOTES_MAX do
        local old = table.remove(H.notes, 1)
        exec(string.format("DELETE FROM mod_den_notes WHERE id = %d", old.id))
    end
    return n
end

function H.unread()
    local n = 0
    for _, x in ipairs(H.notes) do if not x.seen then n = n + 1 end end
    return n
end

local function owners()
    local out = {}
    if not GetPlayersInWorld then return out end
    local ok, list = pcall(GetPlayersInWorld)
    if not ok or type(list) ~= "table" then return out end
    for _, pl in ipairs(list) do if isOwner(pl) then out[#out + 1] = pl end end
    return out
end

------------------------------------------------------------------------------------------------
-- the home line for the addon
------------------------------------------------------------------------------------------------
function H.sendHome(player)
    if not player then return end
    local p = H.pending[player:GetGUIDLow()]
    send(player, "home", string.format("started=%s;door=%d;mailbox=%d;mail=%d;role=%s;stamp=%s;stamps=%s", setting("started") or "-",
        H.spot("door") and 1 or 0, H.spot("mailbox") and 1 or 0, isOwner(player) and H.unread() or 0, role(player),
        p and p.name or "-", table.concat(H.STAMPS, ",")))
end

------------------------------------------------------------------------------------------------
-- commands
------------------------------------------------------------------------------------------------
local C = {}

function H.starterOf(word)
    local key = H.ALIASES[tostring(word or ""):lower()]
    for _, s in ipairs(H.STARTERS) do if s.key == key then return s end end
    return nil
end

function C.start(player, words)
    if not isOwner(player) then return say(player, "Only the Den's owner picks its starting place.") end
    local s = H.starterOf(words[2])
    if not s then
        local names = {}
        for _, x in ipairs(H.STARTERS) do names[#names + 1] = "|cffffff00" .. x.key .. "|r (" .. x.name .. ")" end
        return say(player, "Start your Den as: " .. table.concat(names, ", ") .. ". Type |cffffff00.den start forest|r, for one.")
    end
    if s.key == "empty" then
        set("started", "empty")
        H.sendHome(player)
        return say(player, "An empty Den it is: the plain room, yours to fill. |cffffff00.den stamps|r has ready-made areas.")
    end
    local d = D()
    local have = d.decorCount()
    if have > 0 and (words[3] or ""):lower() ~= "replace" then
        return say(player, string.format("Your Den has %d props. |cffffff00.den start %s replace|r puts %s in their place (a copy of "
            .. "yours is saved as 'backup').", have, s.key, s.name))
    end
    local doc = H.starterDoc(s)
    if not doc then return say(player, s.name .. " isn't on this realm (the Host pack needs an update).") end
    local c, why = B().DenCheck(doc, false)
    if not c then return say(player, why or "That starting place doesn't fit this Den.") end
    d.Files.open(player, s.file, c, doc, true)
    set("started", s.key)
    say(player, s.name .. " is yours: change anything. The front door and the mailbox are by the house.")
    H.sendHome(player)
end

function C.stamps(player)
    local parts = {}
    for _, n in ipairs(H.STAMPS) do
        local st = H.stamp(n)
        if st then parts[#parts + 1] = "|cffffff00" .. n .. "|r (" .. st.title .. ", " .. #st.props .. ")" end
    end
    if #parts == 0 then return say(player, "No stamps on this realm (the Host pack needs an update).") end
    say(player, "Stamps, ready-made areas: " .. table.concat(parts, ", ") .. ".")
    say(player, "|cffffff00.den stamp garden|r marks where it goes at your feet; |cffffff00turn|r, |cffffff00here|r, |cffffff00place|r, |cffffff00cancel|r.")
end

local function canStamp(player)
    if role(player) == "visitor" then say(player, "You're visiting: the owner can make you a builder."); return false end
    return true
end

local function markAt(player, name, deg)
    local guid = player:GetGUIDLow()
    local x, y, z, o = player:GetX(), player:GetY(), player:GetZ(), player:GetO()
    local b = B()
    if b and b.SnapPoint then x, y, z, o = b.SnapPoint(player, x, y, z, o) end
    local old = H.pending[guid]
    H.unmark(player, old)
    local pend = { name = name, map = player:GetMapId(), x = x, y = y, z = z, o = H.step(o + math.rad(deg or 0)), stakes = {} }
    H.pending[guid] = pend
    H.mark(player, pend)
    return pend
end

function H.place(player, pend)
    local st = H.stamp(pend.name)
    local b = B()
    if not (st and b and b.ImportPrefab) then say(player, "That stamp isn't on this realm."); return nil end
    local list = H.pieces(st, pend)
    local props = {}
    for _, e in ipairs(list) do
        local h, dz = b.FloorAt and b.FloorAt(player, e.x, e.y, pend.z), e.z - pend.z   -- each piece on the ground under it
        if h and math.abs(h - pend.z) <= H.GROUND_RANGE then e.z = h + dz end
        props[#props + 1] = { e = e.entry, x = e.x - pend.x, y = e.y - pend.y, z = e.z - pend.z, o = e.o, s = 1 }
    end
    local ok, ids = b.ImportPrefab({ v = 1, name = pend.name, props = props }, player, pend.x, pend.y, pend.z, 0)
    if not ok then return nil end
    H.unmark(player, pend)
    H.pending[player:GetGUIDLow()] = nil
    if isOwner(player) then H.takeRoles(list) end
    say(player, string.format("%s is down (%d props). Ctrl+Z (|cffffff00.build undo|r) takes it back.", st.title, #ids))
    return ids
end

function C.stamp(player, words)
    local guid = player:GetGUIDLow()
    local w = (words[2] or ""):lower()
    if w == "" then return C.stamps(player) end
    if not canStamp(player) then return end
    local pend = H.pending[guid]
    if w == "turn" or w == "here" or w == "place" or w == "cancel" then
        if not pend then return say(player, "Nothing marked: |cffffff00.den stamp <name>|r first (|cffffff00.den stamps|r lists them).") end
        if w == "cancel" then H.unmark(player, pend); H.pending[guid] = nil; return say(player, "The mark is gone; nothing was placed.") end
        if w == "place" then return H.place(player, pend) end
        if w == "here" then
            local o = pend.o
            pend = markAt(player, pend.name, 0)
            pend.o = o
            H.mark(player, pend)
        else
            local deg = tonumber(words[3] or "15")
            if not deg or not finite(deg) or math.abs(deg) > 360 then return say(player, "Turn by up to 360 degrees.") end
            pend.o = H.step(pend.o + math.rad(deg))
            H.mark(player, pend)
        end
        return say(player, markText(pend) .. " |cffffff00.den stamp place|r puts it down.")
    end
    local st = H.stamp(w)
    if not st then return say(player, "No stamp called '" .. w .. "' (|cffffff00.den stamps|r lists them).") end
    local now, deg = (words[3] or ""):lower() == "now", nil
    deg = tonumber(now and words[4] or words[3] or "0") or 0
    if not finite(deg) or math.abs(deg) > 360 then return say(player, "Turn by up to 360 degrees.") end
    pend = markAt(player, w, deg)
    if now then return H.place(player, pend) end
    say(player, markText(pend) .. " The stakes show its corners; the lone one is its front. "
        .. "|cffffff00.den stamp turn|r (15 degrees), |cffffff00here|r, |cffffff00place|r, |cffffff00cancel|r.")
end

local function spotCommand(kind, label)
    return function(player)
        if not isOwner(player) then return say(player, "Only the Den's owner moves the " .. label .. ".") end
        if not (D() and D().inside(player)) then return say(player, "Stand in your Den first.") end
        H.setSpot(kind, player:GetX(), player:GetY(), player:GetZ())
        say(player, "The " .. label .. " is here now.")
    end
end
C.door = spotCommand("door", "front door")
C.mailbox = spotCommand("mailbox", "mailbox")

local function dist(a, x, y) local dx, dy = a.x - x, a.y - y; return math.sqrt(dx * dx + dy * dy) end

function C.knock(player)
    if isOwner(player) then return say(player, "It's your own door. Visitors knock with |cffffff00.den knock|r.") end
    local door = H.spot("door")
    if door and dist(door, player:GetX(), player:GetY()) > H.DOOR_RANGE then
        return say(player, "Go up to the front door first (|cffffff00.den visit|r takes you there).")
    end
    local g, now = player:GetGUIDLow(), os.time()
    if H.lastKnock[g] and now - H.lastKnock[g] < H.KNOCK_EVERY then return say(player, "You knocked a moment ago.") end
    H.lastKnock[g] = now
    local list = owners()
    if #list == 0 then return say(player, "Nobody's home. Leave a note: |cffffff00.den note <your words>|r.") end
    for _, o in ipairs(list) do
        o:SendBroadcastMessage("|cffff80ff" .. player:GetName() .. " knocks at your front door.|r")
        send(o, "knock", player:GetName())
    end
    say(player, "You knock. The owner heard it.")
end

function C.note(player, words, text)
    local body = tostring(text or ""):gsub("^%s*%S+%s*", "")
    return H.leaveNote(player, body)
end

function H.leaveNote(player, body)
    if not (D() and D().inside(player)) then return say(player, "Notes go in the mailbox of the Den you're in.") end
    local t = H.cleanNote(body)
    if not t then return say(player, "Usage: |cffffff00.den note <your words>|r (up to " .. H.NOTE_LEN .. " letters).") end
    local g, now = player:GetGUIDLow(), os.time()
    if H.lastNote[g] and now - H.lastNote[g] < H.NOTE_EVERY then return say(player, "One note a minute.") end
    H.lastNote[g] = now
    H.addNote(player:GetName(), t, now, isOwner(player))
    say(player, "Your note is in the mailbox.")
    for _, o in ipairs(owners()) do
        if o ~= player then o:SendBroadcastMessage(TAG .. player:GetName() .. " left a note in your mailbox (|cffffff00.den mail|r)."); H.sendHome(o) end
    end
end

function C.mail(player, words)
    if not isOwner(player) then return say(player, "The mailbox is the owner's. Leave a note: |cffffff00.den note <your words>|r.") end
    if (words[2] or ""):lower() == "clear" then
        local n = #H.notes
        H.notes = {}
        exec("DELETE FROM mod_den_notes")
        H.sendHome(player)
        return say(player, string.format("The mailbox is empty (%d note%s gone).", n, n == 1 and "" or "s"))
    end
    if #H.notes == 0 then return say(player, "No notes in your mailbox yet.") end
    local shown = 0
    for i = #H.notes, math.max(1, #H.notes - 9), -1 do
        local n = H.notes[i]
        say(player, string.format("%s%s, %s: %s", n.seen and "" or "|cffffd100new|r ", n.from, os.date("!%d %b %H:%M", n.at), n.text))
        send(player, "mail", string.format("%d;%s;%d;%s", n.id, n.from, n.at, n.text))
        if not n.seen then n.seen = true; exec(string.format("UPDATE mod_den_notes SET seen = 1 WHERE id = %d", n.id)) end
        shown = shown + 1
    end
    if #H.notes > shown then say(player, string.format("(%d older notes kept; |cffffff00.den mail clear|r empties the mailbox.)", #H.notes - shown)) end
    H.sendHome(player)
end

function C.visit(player)
    local d = den()
    if not d then return end
    local door = H.spot("door")
    local fz = D().floorZ()
    if door then try(player, "Teleport", d.map, door.x, door.y, door.z + 1, player:GetO())
    else try(player, "Teleport", d.map, d.x, d.y - d.half - 2, fz + 1, math.pi / 2) end
    say(player, door and "At the front door. |cffffff00.den knock|r lets the owner know you're here." or "At the Den's doorway.")
end

------------------------------------------------------------------------------------------------
-- the owner's first visit: pick a starting place (once, while the Den is empty and nothing is on its way)
------------------------------------------------------------------------------------------------
function H.offer(player)
    local d = D()
    if not (d and B() and isOwner(player) and d.inside(player)) or setting("started") then return end
    if d.decorCount() > 0 then set("started", "own"); return end   -- a Den built before starter places existed
    if d.Files.pending and d.Files.pending() then return end        -- the launcher is opening a file (its own pick)
    local g = player:GetGUIDLow()
    if H.offered[g] then return end
    H.offered[g] = true
    say(player, "Pick how your Den starts: |cffffff00.den start forest|r (Forest glade), |cffffff00.den start keep|r (Dark keep) "
        .. "or |cffffff00.den start empty|r. The Den panel (|cffffff00/den|r) has the same choice.")
    local parts = {}
    for _, s in ipairs(H.STARTERS) do parts[#parts + 1] = s.key .. "," .. s.name end
    send(player, "start", table.concat(parts, ";"))
end

------------------------------------------------------------------------------------------------
-- the save file: notes, door and mailbox go with the Den (ov_den.lua's Files.doc / Files.open, wrapped)
------------------------------------------------------------------------------------------------
function H.wrapFiles()
    local d = D()
    if not d or d.Files.homeWrapped then return end
    d.Files.homeWrapped = true
    local doc0, open0 = d.Files.doc, d.Files.open
    d.Files.doc = function(name)
        local doc = doc0(name)
        local dd, fz = den(), d.floorZ()
        for _, kind in ipairs({ "door", "mailbox" }) do
            local s = H.spot(kind)
            if s and dd then doc[kind] = { math.floor((s.x - dd.x) * 100 + 0.5) / 100, math.floor((s.y - dd.y) * 100 + 0.5) / 100,
                math.floor((s.z - fz) * 100 + 0.5) / 100 } end
        end
        if #H.notes > 0 then
            doc.notes = {}
            for _, n in ipairs(H.notes) do doc.notes[#doc.notes + 1] = { from = n.from, text = n.text, at = n.at } end
        end
        return doc
    end
    d.Files.open = function(player, name, c, doc, replace)
        local n = open0(player, name, c, doc, replace)
        H.fromDoc(doc)
        if player then H.sendHome(player) end
        return n
    end
end

-- A file's door, mailbox (its fields, else its pieces marked so) and notes (added when not here yet).
function H.fromDoc(doc)
    local dd = den()
    if not (dd and type(doc) == "table") then return end
    local fz = D().floorZ()
    for _, kind in ipairs({ "door", "mailbox" }) do
        local v = doc[kind]
        if type(v) == "table" and finite(v[1]) and finite(v[2]) and finite(v[3]) and math.abs(v[1]) <= 1000 and math.abs(v[2]) <= 1000 then
            H.setSpot(kind, dd.x + v[1], dd.y + v[2], fz + v[3])
        elseif type(doc.props) == "table" then
            for _, e in ipairs(doc.props) do
                if type(e) == "table" and e.role == kind and finite(e.x) and finite(e.y) and finite(e.z or 0) then
                    H.setSpot(kind, dd.x + e.x, dd.y + e.y, fz + (e.z or 0))
                end
            end
        end
    end
    if type(doc.notes) == "table" then
        local have = {}
        for _, n in ipairs(H.notes) do have[n.from .. "@" .. n.at] = true end
        for i = 1, math.min(#doc.notes, H.NOTES_MAX) do
            local n = doc.notes[i]
            local t = type(n) == "table" and H.cleanNote(n.text)
            local from = type(n) == "table" and tostring(n.from or ""):gsub("[^%w]", ""):sub(1, 12)
            local at = type(n) == "table" and tonumber(n.at)
            if t and from and from ~= "" and at and finite(at) and not have[from .. "@" .. at] then
                H.addNote(from, t, math.floor(at), true); have[from .. "@" .. at] = true
            end
        end
    end
end

------------------------------------------------------------------------------------------------
-- help, events, the addon
------------------------------------------------------------------------------------------------
local function help(player, r)
    if r == "visitor" then
        say(player, "  |cffffff00.den visit|r (the front door)  |cffffff00.den knock|r  |cffffff00.den note <your words>|r (the mailbox)")
        return
    end
    say(player, "  Stamps: |cffffff00.den stamps|r, |cffffff00.den stamp garden|r then |cffffff00place|r (|cffffff00turn|r, |cffffff00here|r, |cffffff00cancel|r)."
        .. " Grid: |cffffff00.build grid 1|2|5|off|r, |cffffff00.build grid ground on|r.")
    if r == "owner" then
        say(player, "  |cffffff00.den start forest|keep|empty|r  |cffffff00.den mail|r  |cffffff00.den door|r / |cffffff00mailbox|r (where you stand)")
    end
end

local function install()
    OVDen = rawget(_G, "OVDen") or {}
    OVDen.extra = OVDen.extra or {}
    for k, fn in pairs(C) do
        OVDen.extra[k] = function(player, words, text)
            H.wrapFiles()
            fn(player, words, text)
            H.sendHome(player)
        end
    end
    OVDen.help = help
    OVDen.Home = H
end
install()

local function arrive(event, player)
    if not player then return end
    H.wrapFiles()
    pcall(H.offer, player)
    if isOwner(player) and H.unread() > 0 then
        say(player, string.format("%d new note%s in your mailbox: |cffffff00.den mail|r.", H.unread(), H.unread() == 1 and "" or "s"))
    end
    H.sendHome(player)
end
local function logout(event, player)
    if not player then return end
    local g = player:GetGUIDLow()
    H.unmark(player, H.pending[g])
    H.pending[g], H.offered[g] = nil, nil
end

RegisterPlayerEvent(3, arrive)    -- PLAYER_EVENT_ON_LOGIN
RegisterPlayerEvent(4, logout)    -- PLAYER_EVENT_ON_LOGOUT
RegisterPlayerEvent(28, arrive)   -- PLAYER_EVENT_ON_MAP_CHANGE
if CreateLuaEvent then CreateLuaEvent(function() pcall(H.sweep) end, 5000, 0) end

if OVMod then
    OVMod.On(MOD, "home", function(player) H.wrapFiles(); pcall(H.offer, player); H.sendHome(player) end)
    OVMod.On(MOD, "note", function(player, data)
        if #(data or "") > 600 then return end
        H.wrapFiles()
        H.leaveNote(player, data)
        H.sendHome(player)
    end)
end

-- for stream/tests/test_ov_den_home.lua (read only)
OV_DEN_HOME_TEST = H
