-- ov_den_companion.lua: your companion in your Den (mod-ale, 3.3.5a). Only with OV_DEN=1 and OV_DEN_COMPANION=1
-- (realm/profiles/den.env turns it on), next to ov_den.lua (the room, roles, save files) and ov_build.lua (every
-- prop placed and checked). Owner decision 2026-11-01 (the Den plan, 3.5 and 7): the companion is Jaina, in game
-- only (the launcher and the website say "your companion", never her name or likeness). Leyline has no AI of its
-- own: what the owner says to her goes to the player's own assistant through the launcher's `Leyline mcp` (the
-- Agents page, approve-first; launcher/src-tauri/src/agents/den.rs), and what comes back is said here.
--
--   .den jaina                  (the owner) she comes and follows you at a short distance; again: back to your side
--   .den jaina away             she goes (she also goes when you log out or leave the Den)
--   .den companion [away]       the same (the word the launcher and the website use: they never say her name)
--   .den ask <words>            to her, for your assistant ("a well by the door")
--   "Jaina, <words>"            said in chat within 10 yards of her: the same
--
-- The creature: entry 32364, "Lady Jaina Proudmoore" (AzerothCore world database, creature_template, checked
-- 2026-11-01 against master): the game's own Jaina model (creature_template_model 30865, the one every 3.3.5a
-- Jaina uses), no AIName and no ScriptName (no boss fight, event or gossip script rides along), no gossip menu, no
-- npcflag (no quests, no vendor), and unit_flags 832 (immune to players and creatures: nobody fights her, she
-- fights nobody). Faction 35 on top (friendly to all). OV_DEN_COMPANION_ENTRY picks another entry. She is a
-- temporary summon (Player:SpawnCreature, TEMPSUMMON_MANUAL_DESPAWN; else PerformIngameSpawn, not saved): never
-- in the world database, gone at a restart.
--
-- Files in the Den's saves folder (OV_DEN_SAVES, <kit>/state/dens; JSON read by OVBuild.JsonDecode, never run):
--   companion-in.json     written here every 10 seconds and after each ask: the heartbeat (at), companion out or
--                         not, the owner's spot (yards from the room's middle, z from the floor, yaw in degrees),
--                         the room's budgets, the build in progress, the last line said (said), and the last 20
--                         asks: { n, at, text, where }. The launcher's den_status / den_messages read it.
--   companion-out.json    the launcher writes: { lines = { { n, at, text } }, seen, read_at }. Lines after `said`
--                         and under 5 minutes old are said, one every 3 seconds, while she is out. read_at: when the
--                         assistant last read the asks (within 10 minutes: "someone is listening").
--   companion-place.json  the launcher writes: { file, id, label }: a saved Den file to add to the Den. Taken (and
--                         removed) when the owner is in the Den; checked whole first (OVBuild.DenCheck, the budgets),
--                         then placed 20 pieces at a time through OVBuild.ImportPrefab (the builder role, the Den,
--                         budgets, the paste rate: one undo entry per part), her progress said as it goes.
-- Every line she says is plain text (no chat codes), at most 200 letters, and never a game-brand word.
-- mod-ale used: RegisterPlayerEvent 4 ON_LOGOUT, 18 ON_CHAT, 28 ON_MAP_CHANGE; CreateLuaEvent; Player:SpawnCreature /
-- GetMap / GetInstanceId; Map:GetWorldObject; Creature:SetFaction / MoveFollow / SendUnitSay / DespawnOrUnsummon /
-- NearTeleport (each in pcall); PerformIngameSpawn; io.open, os.rename, os.remove; OVDen (ov_den.lua) and OVBuild.

if os.getenv("OV_DEN") ~= "1" or os.getenv("OV_DEN_COMPANION") ~= "1" then return end

local TAG = "|cffc8a2ff[Den]|r "
local SAVES = os.getenv("OV_DEN_SAVES") or "/azerothcore/dens"

local C = {
    ENTRY = tonumber(os.getenv("OV_DEN_COMPANION_ENTRY") or "") or 32364,
    NAME = "Jaina",
    FOLLOW = 2.5, ANGLE = math.pi * 0.75,   -- yards behind you, a little to the left
    FAR = 30,                               -- further than this (a teleport, .den home): she comes back to your side
    HEAR = 10,                              -- "Jaina, ..." within this many yards
    TICK_MS = 2000, BEAT = 10,              -- seconds between two heartbeats
    ASK_EVERY = 4, ASKS_KEEP = 20, LEN = 200,
    SAY_EVERY = 3, LINE_AGE = 300, LISTENING = 600,
    CHUNK = tonumber(os.getenv("OV_DEN_COMPANION_CHUNK") or "") or 20, PLACE_EVERY = 6,
    FILE_MAX = 512 * 1024,
    BRAND = { "world of warcraft", "warcraft", "lich king", "blizzard" },
    out = {},        -- owner guid -> { guid = creature guid, map }
    asks = {}, nextAsk = 1, said = 0, queue = {}, lastSay = 0, lastAsk = {}, lastBeat = 0,
    placing = nil,   -- { label, done, of, state, why, chunks, at, owner }
    lastPlace = nil, -- what the heartbeat says about the last build
}

local function D() local d = rawget(_G, "OVDen"); return (type(d) == "table" and d.Files) and d or nil end
local function B() local b = rawget(_G, "OVBuild"); return (type(b) == "table" and b.DEN) and b or nil end
local function den() local b = B(); return b and b.DEN end

local function try(obj, method, ...)
    if not obj then return nil end
    local ok, fn = pcall(function() return obj[method] end)
    if not ok or type(fn) ~= "function" then return nil end
    local ok2, a, b = pcall(fn, obj, ...)
    if ok2 then return a, b end
    return nil
end
local function finite(v) return type(v) == "number" and v == v and v ~= math.huge and v ~= -math.huge end
local function r1(v) return math.floor(v * 10 + 0.5) / 10 end
local function say(player, text) player:SendBroadcastMessage(TAG .. text) end
local function isOwner(player) local d = D(); return d ~= nil and d.isOwner(player) end
local function inside(player) local d = D(); return d ~= nil and d.inside(player) end

------------------------------------------------------------------------------------------------
-- plain words
------------------------------------------------------------------------------------------------
-- One line of plain text: no chat codes (|), no control characters, single spaces, at most C.LEN letters.
function C.clean(s)
    s = tostring(s or ""):gsub("[%c|]", " "):gsub("%s+", " "):gsub("^ ", ""):gsub(" $", "")
    if #s > C.LEN then s = s:sub(1, C.LEN) end
    return s
end

-- The game's brand stays out of everything she says.
function C.branded(s)
    local l = " " .. tostring(s or ""):lower():gsub("[^%w]+", " ") .. " "
    for _, w in ipairs(C.BRAND) do if l:find(w, 1, true) then return true end end
    return l:find(" wow ", 1, true) ~= nil
end

------------------------------------------------------------------------------------------------
-- files: JSON in the saves folder (the realm's strict reader; written whole, then renamed)
------------------------------------------------------------------------------------------------
local function path(name) return SAVES .. "/" .. name end

function C.readJson(name, max)
    local f = io and io.open and io.open(path(name), "r")
    if not f then return nil end
    local text = f:read((max or 65536) + 1) or ""
    f:close()
    if #text > (max or 65536) then return nil end
    local b = B()
    if not (b and b.JsonDecode) then return nil end
    local v = b.JsonDecode(text:gsub("^\239\187\191", ""), max or 65536)
    if type(v) ~= "table" then return nil end
    return v
end

function C.writeJson(name, v)
    local d = D()
    if not (d and d.Files and d.Files.encode) then return false end
    local tmp = path(name) .. ".tmp"
    local f = io and io.open and io.open(tmp, "w")
    if not f then return false end
    f:write(d.Files.encode(v) .. "\n")
    f:close()
    if not (os.rename and os.rename(tmp, path(name))) then
        if os.remove then pcall(os.remove, tmp) end
        return false
    end
    return true
end

------------------------------------------------------------------------------------------------
-- her: called, following, gone
------------------------------------------------------------------------------------------------
-- Her creature, on the map she was called on (the owner may have left it).
function C.find(player)
    local o = player and C.out[player:GetGUIDLow()]
    if not o then return nil end
    local map
    if player:GetMapId() == o.map then map = try(player, "GetMap") end
    if not map and GetMapById then
        local ok, m = pcall(GetMapById, o.map, o.instance or 0)
        if ok then map = m end
    end
    return map and try(map, "GetWorldObject", o.guid) or nil
end

function C.isOut(player) return C.find(player) ~= nil end

local function spot(player)
    local o = tonumber(try(player, "GetO")) or 0
    return player:GetX() + math.cos(o + C.ANGLE) * C.FOLLOW, player:GetY() + math.sin(o + C.ANGLE) * C.FOLLOW, player:GetZ(), o
end

function C.summon(player)
    local x, y, z, o = spot(player)
    local c = try(player, "SpawnCreature", C.ENTRY, x, y, z, o, 8, 0)   -- TEMPSUMMON_MANUAL_DESPAWN
    if not c and PerformIngameSpawn then
        local ok, made = pcall(PerformIngameSpawn, 1, C.ENTRY, player:GetMapId(), tonumber(try(player, "GetInstanceId")) or 0,
            x, y, z, o, false, 0)
        if ok then c = made end
    end
    if not c then return nil end
    try(c, "SetFaction", 35)          -- friendly to everyone
    try(c, "SetWanderRadius", 0)
    try(c, "MoveFollow", player, C.FOLLOW, C.ANGLE)
    C.out[player:GetGUIDLow()] = { guid = try(c, "GetGUID"), map = player:GetMapId(), instance = tonumber(try(player, "GetInstanceId")) or 0 }
    return c
end

function C.dismiss(player, quiet)
    local c = C.find(player)
    if c and not quiet then C.speak(player, "I'll be close by. Call me when you need me.") end
    if c then
        if not try(c, "DespawnOrUnsummon", 0) then try(c, "RemoveFromWorld") end
    end
    local had = C.out[player:GetGUIDLow()] ~= nil
    C.out[player:GetGUIDLow()] = nil
    return had
end

-- She says one line: in her own voice when she is out (everyone near hears it), else as the Den's line to the owner.
-- Brand words: never said.
function C.speak(player, text)
    text = C.clean(text)
    if text == "" or C.branded(text) then return false end
    local c = C.find(player)
    if c then
        local ok, fn = pcall(function() return c.SendUnitSay end)
        if ok and type(fn) == "function" and pcall(fn, c, text, 0) then return true end   -- LANG_UNIVERSAL
    end
    say(player, C.NAME .. ": " .. text)
    return true
end

------------------------------------------------------------------------------------------------
-- asks: what the owner says to her, for their assistant
------------------------------------------------------------------------------------------------
local function listening()
    local o = C.readJson("companion-out.json")
    local t = o and tonumber(o.read_at)
    return t ~= nil and os.time() - t <= C.LISTENING
end

function C.ask(player, text)
    if not isOwner(player) then return say(player, "Only the Den's owner talks to their companion.") end
    if not C.isOut(player) then return say(player, "Call your companion first: |cffffff00.den jaina|r") end
    text = C.clean(text)
    if text == "" then return say(player, "Usage: |cffffff00.den ask <your words>|r, or say \"Jaina, ...\" near her.") end
    local g, now = player:GetGUIDLow(), os.time()
    if C.lastAsk[g] and now - C.lastAsk[g] < C.ASK_EVERY then
        return C.speak(player, "One moment, I'm still on the last one.")
    end
    C.lastAsk[g] = now
    local d, fz = den(), D().floorZ()
    local where = d and { x = r1(player:GetX() - d.x), y = r1(player:GetY() - d.y), z = r1(player:GetZ() - fz),
        yaw = r1(math.deg(tonumber(try(player, "GetO")) or 0) % 360) } or nil
    C.asks[#C.asks + 1] = { n = C.nextAsk, at = now, text = text, where = where }
    C.nextAsk = C.nextAsk + 1
    while #C.asks > C.ASKS_KEEP do table.remove(C.asks, 1) end
    C.beat(true)
    if listening() then
        C.speak(player, "Let me think about that.")
    else
        C.speak(player, "I've written that down. Nobody is reading my notes right now: open the Agents page in Leyline and "
            .. "ask your assistant to read what you told me.")
    end
    return true
end

------------------------------------------------------------------------------------------------
-- lines from the launcher
------------------------------------------------------------------------------------------------
function C.readLines()
    local o = C.readJson("companion-out.json")
    if not o or type(o.lines) ~= "table" then return end
    local now = os.time()
    for _, l in ipairs(o.lines) do
        local n, at = tonumber(type(l) == "table" and l.n), tonumber(type(l) == "table" and l.at)
        if n and n == math.floor(n) and n > C.said then
            C.said = n
            if at and now - at <= C.LINE_AGE and type(l.text) == "string" then C.queue[#C.queue + 1] = l.text end
        end
    end
    while #C.queue > C.ASKS_KEEP do table.remove(C.queue, 1) end
end

local function owners()
    local out = {}
    if not GetPlayersInWorld then return out end
    local ok, list = pcall(GetPlayersInWorld)
    if not ok or type(list) ~= "table" then return out end
    for _, p in ipairs(list) do if isOwner(p) then out[#out + 1] = p end end
    return out
end

-- The owner she is out for (the first), else nil.
local function withHer()
    for _, p in ipairs(owners()) do if C.isOut(p) then return p end end
    return nil
end

function C.sayNext()
    if #C.queue == 0 or os.time() - C.lastSay < C.SAY_EVERY then return end
    local p = withHer()
    if not p then return end
    while #C.queue > 0 do   -- a line she won't say (empty, a brand word) doesn't take a turn
        if C.speak(p, table.remove(C.queue, 1)) then C.lastSay = os.time(); return end
    end
end

------------------------------------------------------------------------------------------------
-- building: a saved Den file added to the Den, part by part
------------------------------------------------------------------------------------------------
-- The pieces of a checked Den file as prefab parts: { origin = {x, y, z}, props = { {e, x, y, z, o, s} } }.
function C.parts(doc)
    local d, fz = den(), D().floorZ()
    local list = {}
    for _, e in ipairs(doc.props) do list[#list + 1] = e end
    local parts = {}
    for i = 1, #list, C.CHUNK do
        local chunk, x0, y0, x1, y1 = {}, math.huge, math.huge, -math.huge, -math.huge
        for j = i, math.min(#list, i + C.CHUNK - 1) do
            local e = list[j]
            chunk[#chunk + 1] = e
            x0, y0, x1, y1 = math.min(x0, e.x), math.min(y0, e.y), math.max(x1, e.x), math.max(y1, e.y)
        end
        local cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
        local props = {}
        for _, e in ipairs(chunk) do
            props[#props + 1] = { e = e.entry, x = e.x - cx, y = e.y - cy, z = e.z or 0, o = math.rad(e.yaw or 0), s = e.scale or 1 }
        end
        parts[#parts + 1] = { origin = { d.x + cx, d.y + cy, fz }, props = props }
    end
    return parts
end

function C.status(state, why)
    local p = C.placing
    C.lastPlace = { label = p.label, done = p.done, of = p.of, state = state, why = why, file = p.file }
    if state ~= "placing" then C.placing = nil end
    C.beat(true)
end

-- Starts a build from companion-place.json (the owner in the Den).
function C.takePlace(player)
    if C.placing then return end
    local req = C.readJson("companion-place.json", 4096)
    if not req then return end
    local d = D()
    local name = d.Files.clean(req.file)
    local id = tostring(req.id or ""):gsub("[^%w]", ""):sub(1, 32)
    local label = C.clean(type(req.label) == "string" and req.label or (name or "the build")):sub(1, 40)
    if os.remove then pcall(os.remove, path("companion-place.json")) end
    if not name or id == "" then return end
    local doc = C.readJson(name .. ".den.json", C.FILE_MAX)
    local b = B()
    C.placing = { label = label, done = 0, of = 0, file = name, id = id, owner = player:GetGUIDLow(), at = 0 }
    if not doc or doc.format ~= "leyline-den" or type(doc.props) ~= "table" then
        C.tell(player, "I couldn't read the plans for " .. label .. ".")
        return C.status("failed", "the file can't be read")
    end
    local c, why = b.DenCheck(doc, true)
    if not c then
        C.tell(player, "That won't fit: " .. C.clean(why or "") .. " Nothing was placed.")
        return C.status("failed", why)
    end
    C.placing.parts = C.parts(doc)
    C.placing.of = #doc.props
    C.tell(player, string.format("Placing %s: %d pieces.", label, #doc.props))
    C.status("placing")
end

-- The next part of the build in progress (one paste every C.PLACE_EVERY seconds: ImportPrefab's rate).
function C.placeNext(player)
    local p = C.placing
    if not (p and p.parts) or os.time() - p.at < C.PLACE_EVERY then return end
    if player:GetGUIDLow() ~= p.owner then return end
    if not inside(player) then return end   -- waits while the owner is out of the Den
    p.at = os.time()
    local part = table.remove(p.parts, 1)
    if not part then return end
    local ok, ids = B().ImportPrefab({ v = 1, name = "companion", props = part.props }, player, part.origin[1], part.origin[2], part.origin[3], 0)
    if not ok then
        C.tell(player, string.format("I stopped placing %s at %d of %d pieces: %s", p.label, p.done, p.of, C.clean(ids or "it didn't fit")))
        return C.status("stopped", ids)
    end
    p.done = p.done + #ids
    if #p.parts == 0 then
        C.tell(player, string.format("Done: %s is in, %d pieces. Ctrl+Z takes it back, a part at a time.", p.label, p.done))
        return C.status("done")
    end
    C.tell(player, string.format("Placing %s: %d of %d pieces.", p.label, p.done, p.of))
    C.status("placing")
end

-- Progress in her voice when she's out, else the Den's line.
function C.tell(player, text)
    if not C.speak(player, text) then say(player, C.clean(text)) end
end

------------------------------------------------------------------------------------------------
-- the heartbeat: companion-in.json
------------------------------------------------------------------------------------------------
function C.beat(now)
    if not now and os.time() - C.lastBeat < C.BEAT then return end
    C.lastBeat = os.time()
    local b, d = B(), den()
    local doc = { format = "leyline-companion", version = 1, at = os.time(), said = C.said, asks = C.asks, companion = false }
    local o = owners()[1]
    if o and d then
        local fz = D().floorZ()
        doc.owner = { online = true, inside = inside(o), x = r1(o:GetX() - d.x), y = r1(o:GetY() - d.y), z = r1(o:GetZ() - fz),
            yaw = r1(math.deg(tonumber(try(o, "GetO")) or 0) % 360) }
        doc.companion = withHer() ~= nil
    else
        doc.owner = { online = false }
    end
    if b and b.DenUsage and b.DenLayout then
        local ok, i, ic, yo, yc = pcall(b.DenUsage)
        local okL, lay = pcall(b.DenLayout)
        if ok and okL and type(lay) == "table" then
            doc.room = { props = #lay.props, room_points = i, room_cap = ic, yard_points = yo, yard_cap = yc, half = d and d.half }
        end
    end
    if C.placing then
        doc.placing = { label = C.placing.label, done = C.placing.done, of = C.placing.of, state = "placing", file = C.placing.file }
    elseif C.lastPlace then
        doc.placing = C.lastPlace
    end
    C.writeJson("companion-in.json", doc)
end

-- After a restart (or a Lua reload): the asks and the last line said come back, nothing is said twice.
function C.restore()
    if C.restored or not B() then return end
    C.restored = true
    local v = C.readJson("companion-in.json", 256 * 1024)
    if not v then return end
    if tonumber(v.said) then C.said = math.floor(tonumber(v.said)) end
    if type(v.asks) == "table" then
        for _, a in ipairs(v.asks) do
            if type(a) == "table" and tonumber(a.n) and type(a.text) == "string" then
                C.asks[#C.asks + 1] = { n = tonumber(a.n), at = tonumber(a.at) or 0, text = C.clean(a.text), where = type(a.where) == "table" and a.where or nil }
                C.nextAsk = math.max(C.nextAsk, tonumber(a.n) + 1)
            end
        end
        while #C.asks > C.ASKS_KEEP do table.remove(C.asks, 1) end
    end
end

------------------------------------------------------------------------------------------------
-- the tick: follow, lines, building, heartbeat
------------------------------------------------------------------------------------------------
function C.follow(player)
    local o = C.out[player:GetGUIDLow()]
    if not o then return end
    local c = C.find(player)
    if not inside(player) or player:GetMapId() ~= o.map then
        C.dismiss(player, true)
        return say(player, "Your companion stays in your Den: |cffffff00.den jaina|r calls her again there.")
    end
    if not c then C.out[player:GetGUIDLow()] = nil; C.summon(player); return end
    local dx, dy = (tonumber(try(c, "GetX")) or player:GetX()) - player:GetX(), (tonumber(try(c, "GetY")) or player:GetY()) - player:GetY()
    if math.sqrt(dx * dx + dy * dy) > C.FAR then
        local x, y, z, ori = spot(player)
        if not try(c, "NearTeleport", x, y, z, ori) then C.dismiss(player, true); C.summon(player); return end
        try(c, "MoveFollow", player, C.FOLLOW, C.ANGLE)
    end
end

function C.tick()
    C.install()
    if not (B() and D()) then return end
    C.restore()
    pcall(C.readLines)
    for _, p in ipairs(owners()) do
        pcall(C.follow, p)
        if inside(p) then pcall(C.takePlace, p); pcall(C.placeNext, p) end
    end
    pcall(C.sayNext)
    pcall(C.beat)
end

------------------------------------------------------------------------------------------------
-- commands, chat, events
------------------------------------------------------------------------------------------------
function C.jaina(player, words)
    if not isOwner(player) then return say(player, "Only the Den's owner calls their companion.") end
    local w = (words[2] or ""):lower()
    if w == "away" or w == "dismiss" or w == "bye" or w == "go" then
        if not C.dismiss(player) then say(player, "Your companion isn't here.") end
        return
    end
    if not inside(player) then return say(player, "Your companion stays in your Den: |cffffff00.den home|r first.") end
    if C.isOut(player) then
        C.dismiss(player, true)
        C.summon(player)
        return C.speak(player, "Right here.")
    end
    if not C.summon(player) then return say(player, "Your companion can't come on this realm.") end
    C.speak(player, "Hello! Tell me what you'd like to build: say \"Jaina, ...\" near me, or |cffffff00.den ask|r.")
    say(player, "Your companion follows you. What you tell her goes to your own assistant through the Leyline launcher "
        .. "(Agents page); it builds only after you press Approve there. |cffffff00.den jaina away|r sends her off.")
end

-- "Jaina, <words>" said within C.HEAR yards of her (said aloud as usual; she hears it too).
function C.onChat(event, player, msg, msgType)
    if not player or (msgType ~= nil and msgType ~= 1) then return end   -- CHAT_MSG_SAY
    local text = tostring(msg or ""):match("^%s*[Jj][Aa][Ii][Nn][Aa]%s*[,:!]%s*(.+)$")
    if not text or not isOwner(player) then return end
    local c = C.find(player)
    if not c then return end
    local dx, dy = (tonumber(try(c, "GetX")) or player:GetX()) - player:GetX(), (tonumber(try(c, "GetY")) or player:GetY()) - player:GetY()
    if math.sqrt(dx * dx + dy * dy) > C.HEAR then return end
    pcall(C.ask, player, text)
end

local function help(player, r)
    if r ~= "owner" then return end
    say(player, "  Your companion: |cffffff00.den jaina|r (|cffffff00away|r sends her off); |cffffff00.den ask <words>|r, or say "
        .. "\"Jaina, ...\" near her: your own assistant answers and builds, after you approve in the launcher.")
end

-- .den jaina / .den ask (ov_den.lua's OVDen.extra) and a line in .den's help: put back if another script replaced them.
function C.install()
    OVDen = rawget(_G, "OVDen") or {}
    OVDen.extra = OVDen.extra or {}
    if OVDen.extra.jaina ~= C.cmdJaina then OVDen.extra.jaina = C.cmdJaina end
    if OVDen.extra.companion ~= C.cmdJaina then OVDen.extra.companion = C.cmdJaina end
    if OVDen.extra.ask ~= C.cmdAsk then OVDen.extra.ask = C.cmdAsk end
    if OVDen.help ~= C.helpWrap then
        local prev = OVDen.help
        C.helpWrap = function(player, r)
            if prev then pcall(prev, player, r) end
            help(player, r)
        end
        OVDen.help = C.helpWrap
    end
    OVDen.Companion = C
end
C.cmdJaina = function(player, words) C.jaina(player, words) end
C.cmdAsk = function(player, words, text) C.ask(player, tostring(text or ""):gsub("^%s*%S+%s*", "")) end

local function logout(event, player) if player then C.dismiss(player, true); C.lastAsk[player:GetGUIDLow()] = nil end end
local function mapChange(event, player) if player and C.out[player:GetGUIDLow()] then pcall(C.follow, player) end end

C.restore()
C.install()
RegisterPlayerEvent(4, logout)       -- PLAYER_EVENT_ON_LOGOUT
RegisterPlayerEvent(18, C.onChat)    -- PLAYER_EVENT_ON_CHAT
RegisterPlayerEvent(28, mapChange)   -- PLAYER_EVENT_ON_MAP_CHANGE
if CreateLuaEvent then CreateLuaEvent(function() pcall(C.tick) end, C.TICK_MS, 0) end

-- for stream/tests/test_ov_den_companion.lua (read only)
OV_DEN_COMPANION_TEST = C
