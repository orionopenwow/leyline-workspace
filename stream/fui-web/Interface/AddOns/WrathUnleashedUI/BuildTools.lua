-- BuildTools (3.3.5a API): the Den builder's tools window (grand release brief, 4 Nov: "make the Den builder's tools
-- complete and top tier"), WrathUnleashedToolsFrame, opened by the rail's Tools, Path and Stamps (Den Lite) and
-- /tools. Its tabs, in the editor's look (Style.lua S.Ed*):
--   Select   Box select (two clicks on the screen: its corners), the selection's group, all of its kind; snap to the
--            grid and the turn step; Land (on what is under it: surface snap); Solid on / off / as the game makes it;
--            lock, hide, show all; Measure; Replace with the gallery's pick (one, or all of its kind).
--   Arrange  align X / Y / Z / bottoms / tops, distribute X / Y / Z; an array (n copies a step apart) and a ring (n
--            copies round it or round you, facing in, out, along or as it is); copy, paste, duplicate.
--   Path     the gallery's pick laid along clicks on the ground: every n yards, smoothed or straight, along or across
--            the path, on the middles (fences), on the line or one or both sides; the points shown as stakes, undo a
--            point, move point n (its number, then a click), delete it; Lay; Edit a laid path (its points come back,
--            the next Lay lays it again in place); presets Fence, Lamps, Road.
--   Stamps   the stamp library by category (Leyline's own, yours, places copied from your game) with each one's
--            top-down picture and credit; turn, mirror and size it, then Place with the mouse: a click on the ground
--            puts it there, another click moves it, Enter keeps it, Esc takes it back; save the selection as a stamp.
--   Layers   the Den's groups: select, hide / show, lock / unlock each; group the selection by a name; ungroup.
--   History  the undo steps by name, the newest first: a click goes back to that point.
-- Everything is a `.build` command over the Builder's channel (Builder.lua B.Command; Den Lite's buildtools.rs
-- answers): `wubuild point | pathpts | pathedit | staged | stamps | stampplan | groups | history | measure` come back.
-- A click on the world while a tool waits for one (Gizmo.lua's MouseUp asks T.Click first) goes to the tool.
-- The full Den (realm) has its own align, group, hide and measure (the Builder's More); this window is Den Lite's.

local W = WrathUnleashedUI
local T = { tab = "select", mode = nil, pts = {}, len = 0, lay = { every = 3, smooth = true, face = "along", mid = false, sides = 0, offset = 2 },
    xf = { turn = 0, mirror = "none", scale = 1 }, stamps = {}, plans = {}, cat = "all", page = 0, groups = {}, history = {}, parts = {} }
local PREFIX, MOD = "OVMOD", "wubuild"
local WIN_W, PAD, ROW = 440, 12, 34
local TABS = { { "select", "Select" }, { "arrange", "Arrange" }, { "path", "Path" }, { "stamps", "Stamps" }, { "layers", "Layers" }, { "history", "History" } }
local STAMP_ROWS = 6
local PLAN_PX = 128
-- the stamp picture's colours by class (tools.rs plan): building or floor, furniture, small things, plants, lights
local PLAN_COLOURS = { [0] = { 0.62, 0.55, 0.45, 0.9 }, { 0.75, 0.55, 0.30, 0.95 }, { 0.55, 0.60, 0.70, 0.95 }, { 0.35, 0.65, 0.35, 0.95 }, { 0.95, 0.80, 0.35, 1 } }
local CAT_TEXT = { all = "All", buildings = "Buildings", gardens = "Gardens", markets = "Markets", camps = "Camps", water = "Water",
    rooms = "Rooms", paths = "Paths", mine = "Mine", places = "Places" }
local PRESETS = {
    fence = { every = 2, smooth = true, face = "along", mid = true, sides = 0, offset = 0 },
    lamps = { every = 6, smooth = true, face = "across", mid = false, sides = 3, offset = 3 },
    road = { every = 2, smooth = true, face = "along", mid = false, sides = 0, offset = 0 },
}
local win

local function S() return W.Style end
local function B() return W.Builder end
local function cmd(text) local b = B(); return b and b.Command(text) or false end
local function status(text)
    T.status = text
    if win then win.status:SetText(text or "") end
    local b = B()
    if b and b.SetStatus then b.SetStatus(text) end
end
local function chip(parent, text, w, fn, ...) return S().EdChip(parent, text, w, fn, ...) end
local function lite() local b = B(); return b and b.state and b.state.lite == "1" or false end

-- ---------------------------------------------------------------- what Den Lite says

local function split(s, sep)
    local out = {}
    for part in (s .. sep):gmatch("(.-)" .. sep:gsub("%p", "%%%0")) do out[#out + 1] = part end
    return out
end

local function dispatch(name, data)
    if name == "pathpts" then
        local n, g, len, pts = data:match("^(%d+)|(%d+)|([%d%.]+)|(.*)$")
        T.pts = {}
        for x, y in (pts or ""):gmatch("(%-?[%d%.]+),(%-?[%d%.]+)") do T.pts[#T.pts + 1] = { tonumber(x), tonumber(y) } end
        T.editing = tonumber(g or "0") ~= 0 and tonumber(g) or nil
        T.len = tonumber(len or "0") or 0
    elseif name == "pathedit" then
        local f = split(data, "|")
        if #f >= 8 then
            T.editing = tonumber(f[1])
            T.pathEntry = tonumber(f[2])
            T.lay = { every = tonumber(f[3]) or 3, smooth = f[4] == "1", face = f[5] == "across" and "across" or "along", mid = f[6] == "1",
                sides = tonumber(f[7]) or 0, offset = tonumber(f[8]) or 0 }
            T.Show("path")
        end
    elseif name == "staged" then
        if data == "-" then T.staged = nil else
            local n, turn, m, sc = data:match("^([%w_%-]+)|(%-?%d+)|(%a+)|([%d%.]+)$")
            T.staged = n and { name = n, turn = tonumber(turn), mirror = m, scale = tonumber(sc) } or nil
        end
        T.BindStageKeys(T.staged ~= nil)
    elseif name == "stamps" then
        local list = {}
        for item in data:gmatch("[^;]+") do
            local key, title, cat, n, origin, credit = item:match("^([%w_%-]+),([^,]*),(%a+),(%d+),(%a+),(.*)$")
            if key and #list < 80 then list[#list + 1] = { key = key, name = title ~= "" and title or key, cat = cat, count = tonumber(n), origin = origin, credit = credit } end
        end
        T.stamps = list
        local b = B()
        if b and b.tab == "stamps" and b.Filter then b.Filter() end
    elseif name == "stampplan" then
        local key, plan = data:match("^([%w_%-]+)|(.*)$")
        if key then
            local rects = {}
            for cx, cy, w, h, c in plan:gmatch("(%-?%d+),(%-?%d+),(%d+),(%d+),(%d)") do rects[#rects + 1] = { tonumber(cx), tonumber(cy), tonumber(w), tonumber(h), tonumber(c) } end
            T.plans[key] = rects
        end
    elseif name == "groups" then
        local list = {}
        for item in data:gmatch("[^;]+") do
            local id, gname, n, hidden, locked = item:match("^(%d+),([^,]*),(%d+),([01]),([01])$")
            if id then list[#list + 1] = { id = tonumber(id), name = gname, count = tonumber(n), hidden = hidden == "1", locked = locked == "1" } end
        end
        T.groups = list
    elseif name == "history" then
        local u, r, names = data:match("^(%d+)|(%d+)|(.*)$")
        T.undo, T.redo = tonumber(u or "0"), tonumber(r or "0")
        T.history = {}
        for n in (names or ""):gmatch("[^;]+") do T.history[#T.history + 1] = n end
    elseif name == "measure" then
        T.measured = {}
        for k, v in data:gmatch("(%a+)=([%-%d%.]+)") do T.measured[k] = tonumber(v) end
    elseif name == "state" then
        -- the Builder's own state: Solid and the group of the selection (buildtools.rs state_extra)
    else
        return false
    end
    T.Refresh()
    return true
end
T.Dispatch = dispatch

function T.OnMessage(prefix, msg, channel, sender)
    if prefix ~= PREFIX or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, i, n, data = msg:match("^([^\t]+)\t([^\t]+)\t(%d+)/(%d+)\t(.*)$")
    if mod ~= MOD then return end
    i, n = tonumber(i), tonumber(n)
    if n < 1 or n > 32 or i < 1 or i > n then return end
    if n == 1 then T.parts[name] = nil; return dispatch(name, data) end
    local buf = T.parts[name]
    if i == 1 or not buf or buf.n ~= n then buf = { n = n, got = 0 }; T.parts[name] = buf end
    if not buf[i] then buf[i] = data; buf.got = buf.got + 1 end
    if buf.got == n then
        T.parts[name] = nil
        dispatch(name, table.concat(buf, "", 1, n))
    end
end

-- ---------------------------------------------------------------- the mouse: the tool that waits for a click

local MODE_TEXT = { point = "Path: click the ground to add a point (right-click stops)", move = "Path: click where point %d goes",
    box1 = "Box select: click one corner", box2 = "Box select: click the other corner", stamp = "Stamp: click the ground where it goes" }
function T.SetMode(m, arg)
    T.mode, T.moveIndex = m, (m == "move") and arg or nil
    if m ~= "box2" then T.corner = nil end
    local text = m and MODE_TEXT[m]
    if text then status(text:format(arg or 0)) end
    T.Refresh()
end

local function zoomOf() local g = W.Gizmo; return (g and g.zoom) or 12 end

-- A short, still left click on the world (Gizmo.lua asks here first): taken when a tool waits for one.
function T.Click(u, v, a, zoom, shift)
    if not T.mode or not u then return false end
    a, zoom = a or (16 / 9), zoom or zoomOf()
    if T.mode == "point" then
        cmd(string.format("tool point %.4f %.4f %.4f %d", u, v, a, zoom))
    elseif T.mode == "move" then
        cmd(string.format("tool point %.4f %.4f %.4f %d %d", u, v, a, zoom, (T.moveIndex or 1) - 1))
        T.SetMode("point")
    elseif T.mode == "box1" then
        T.corner = { u, v }
        T.SetMode("box2")
        T.corner = { u, v }
    elseif T.mode == "box2" then
        local c = T.corner or { u, v }
        cmd(string.format("tool box %.4f %.4f %.4f %.4f %.4f %d%s", c[1], c[2], u, v, a, zoom, shift and " add" or ""))
        T.SetMode(nil)
    elseif T.mode == "stamp" then
        local x = T.xf
        cmd(string.format("tool stamp %s %.4f %.4f %.4f %d %d %s %.2f", T.stampKey or "", u, v, a, zoom, x.turn, x.mirror, x.scale))
    end
    return true
end

-- A right-click on the world: the waiting tool stops (a staged stamp stays: Enter keeps it, Esc takes it back).
function T.RightClick()
    if not T.mode then return false end
    T.SetMode(nil)
    status("")
    return true
end

-- Enter keeps a staged stamp, Esc takes it back (override bindings only while one waits, given back after).
local stageOwner
function T.BindStageKeys(on)
    if not SetOverrideBindingClick or not ClearOverrideBindings then return false end
    if not stageOwner then
        stageOwner = CreateFrame("Frame", "LeylineToolsKeys", UIParent)
        local keep = CreateFrame("Button", "LeylineToolsKeep", UIParent)
        keep:SetScript("OnClick", function() T.Keep() end)
        local back = CreateFrame("Button", "LeylineToolsCancel", UIParent)
        back:SetScript("OnClick", function() T.CancelStamp() end)
    end
    ClearOverrideBindings(stageOwner)
    T.stageKeys = nil
    if on and not (InCombatLockdown and InCombatLockdown()) then
        SetOverrideBindingClick(stageOwner, true, "ENTER", "LeylineToolsKeep")
        SetOverrideBindingClick(stageOwner, true, "NUMPADENTER", "LeylineToolsKeep")
        SetOverrideBindingClick(stageOwner, true, "ESCAPE", "LeylineToolsCancel")
        T.stageKeys = true
    end
    return T.stageKeys == true
end

-- ---------------------------------------------------------------- what the buttons do

function T.Lay()
    local b = B()
    local it = (b and b.pick and not b.pick.kind) and b.pick or nil
    local entry = it and it.id or T.pathEntry
    if not entry then status("Pick the prop the path is made of in the gallery first (a fence, a lamp, a road tile...)."); return false end
    if #T.pts < 1 then status("Add points first: Add points, then click the ground along the way."); return false end
    local l = T.lay
    T.SetMode(nil)
    return cmd(string.format("path lay %d %s %d %s %d %d %s", entry, tostring(l.every), l.smooth and 1 or 0, l.face, l.mid and 1 or 0, l.sides, tostring(l.offset)))
end

function T.Preset(name)
    local p = PRESETS[name]
    if not p then return false end
    T.lay = { every = p.every, smooth = p.smooth, face = p.face, mid = p.mid, sides = p.sides, offset = p.offset }
    T.Refresh()
    return true
end

function T.SetLay(k, v)
    if k == "every" or k == "offset" then
        v = tonumber(v)
        if not v then return false end
        v = math.max(k == "every" and 0.25 or 0, math.min(k == "every" and 50 or 20, v))
    end
    T.lay[k] = v
    T.Refresh()
    return true
end

function T.StampItems()
    local out = {}
    for _, s in ipairs(T.stamps) do
        if T.cat == "all" or s.cat == T.cat or (T.cat == "mine" and s.origin == "mine") then
            out[#out + 1] = { kind = "stamp", key = s.key, id = "stamp:" .. s.key, name = s.name, cat = s.cat, origin = s.origin, credit = s.credit, count = s.count,
                line = string.format("%d pieces%s", s.count or 0, (s.credit and s.credit ~= "") and (" · by " .. s.credit) or "") }
        end
    end
    return out
end

function T.Categories()
    local seen, out = {}, { "all" }
    for _, s in ipairs(T.stamps) do if not seen[s.cat] then seen[s.cat] = true; out[#out + 1] = s.cat end end
    if not seen.mine then for _, s in ipairs(T.stamps) do if s.origin == "mine" then out[#out + 1] = "mine"; break end end end
    return out
end

function T.PickStamp(key)
    T.stampKey = key
    if not T.plans[key] then cmd("stampplan " .. key) end
    T.Refresh()
    return true
end

-- Place with the mouse: the picked stamp waits for a click on the ground.
function T.PlaceStamp(key)
    if key then T.PickStamp(key) end
    if not T.stampKey then status("Pick a stamp first."); return false end
    T.SetMode("stamp")
    return true
end

function T.Turn(deg)
    T.xf.turn = (T.xf.turn + deg) % 360
    if T.xf.turn > 180 then T.xf.turn = T.xf.turn - 360 end
    T.Again()
    T.Refresh()
end
function T.Mirror(m) T.xf.mirror = m; T.Again(); T.Refresh() end
function T.Size(d)
    T.xf.scale = math.floor(math.max(0.25, math.min(3, T.xf.scale + d)) * 100 + 0.5) / 100
    T.Again()
    T.Refresh()
end
-- a stamp that is down and waiting: put down again with the new turn, mirror and size
function T.Again()
    if not T.staged then return false end
    return cmd(string.format("stamp again %d %s %.2f", T.xf.turn, T.xf.mirror, T.xf.scale))
end
function T.Keep()
    if T.mode == "stamp" then T.SetMode(nil) end
    T.staged = nil
    T.BindStageKeys(false)
    return cmd("stamp keep")
end
function T.CancelStamp()
    if T.mode == "stamp" then T.SetMode(nil) end
    T.staged = nil
    T.BindStageKeys(false)
    return cmd("stamp cancel")
end

local function nameOk(n) return n and n:match("^[%w_%-]+$") and #n <= 24 end
function T.SaveStamp(name, cat, title)
    name = tostring(name or ""):lower():gsub("%s+", "-")
    if not nameOk(name) then status("Stamp names are letters, digits, - and _ (up to 24)."); return false end
    cat = (cat and CAT_TEXT[cat] and cat ~= "all") and cat or "mine"
    return cmd(("stamp save " .. name .. " " .. cat .. " " .. tostring(title or ""):gsub("[^%w%s%-']", "")):sub(1, 100))
end

function T.Array(n, dx, dy, dz)
    n = tonumber(n)
    if not n or n < 1 then status("How many copies?"); return false end
    return cmd(string.format("array %d %s %s %s", n, tostring(tonumber(dx) or 0), tostring(tonumber(dy) or 0), tostring(tonumber(dz) or 0)))
end
function T.Ring(n, r, face, me)
    n, r = tonumber(n), tonumber(r)
    if not n or not r then status("How many copies, and how far out?"); return false end
    return cmd(string.format("ring %d %s %s%s", n, tostring(r), face or "in", me and " me" or ""))
end

function T.Replace(all)
    local b = B()
    return b and b.Swap and b.Swap(all) or false
end

-- ---------------------------------------------------------------- the window

local function section(parent, text, y) return S().EdLabel(parent, text, false, "TOPLEFT", PAD, y) end
local function box(parent, w, text, ...)
    local e = S().EdInput(parent, w, 24)
    e:SetPoint(...)
    e:SetText(text or "")
    return e
end
-- chips in a row, wrapping: { {text, fn, key, tip}, ... }
local function row(parent, list, y, into)
    local x = PAD
    for _, it in ipairs(list) do
        local c = chip(parent, it[1], nil, it[2])
        local w = tonumber((c:GetWidth())) or 70
        if x > PAD and x + w > WIN_W - PAD then x, y = PAD, y - ROW end
        c:SetPoint("TOPLEFT", parent, "TOPLEFT", x, y)
        c.edLabel = it[4]
        x = x + w + 6
        if into then into[it[3] or it[1]] = c end
    end
    return y - ROW
end

local function page(name)
    local pg = CreateFrame("Frame", nil, win)
    pg:SetPoint("TOPLEFT", win, "TOPLEFT", 0, -78); pg:SetPoint("BOTTOMRIGHT", win, "BOTTOMRIGHT", 0, 30)
    win.pages[name] = pg
    pg.c = {}
    return pg
end

local function buildSelect(pg)
    local y = -4
    section(pg, "Select", y)
    y = row(pg, {
        { "Box", function() T.SetMode("box1") end, "box", "Box select: click two corners on the screen (Shift on the second adds)" },
        { "Group", function() cmd("selgroup") end, "selgroup", "Every prop in the selection's group (a path, a stamp, a group you made)" },
        { "Of its kind", function() cmd("selkind") end, "kind", "Every prop like the selected one" },
        { "Clear", function() local b = B(); if b then b.Deselect() end end, "clear" },
    }, y - 18, pg.c)
    section(pg, "Snap", y)
    local snaps = {}
    for _, g in ipairs({ 0, 0.5, 1, 2 }) do
        snaps[#snaps + 1] = { g > 0 and (g .. " yd") or "Grid off", function() local b = B(); if b then b.SetSnap(g) end; T.Refresh() end, "grid" .. g, "Snap what you place to a " .. g .. " yard grid" }
    end
    for _, a in ipairs({ 15, 45, 90 }) do
        snaps[#snaps + 1] = { a .. "°", function() local b = B(); if b then b.SetAngle(a) end; T.Refresh() end, "angle" .. a, "Turn by " .. a .. "°" }
    end
    snaps[#snaps + 1] = { "Land", function() cmd("land") end, "land", "Surface snap: put it on what is under it (a table's top, a floor, the ground)" }
    y = row(pg, snaps, y - 18, pg.c)
    section(pg, "Solid (walk into, stand on)", y)
    y = row(pg, {
        { "Solid", function() cmd("solid on") end, "solidOn", "Stand on it and jump onto it: Den Lite keeps you on its top" },
        { "Walk through", function() cmd("solid off") end, "solidOff", "Walk through it (the game is told it is an open door)" },
        { "As the game", function() cmd("solid auto") end, "solidAuto", "As its model makes it" },
    }, y - 18, pg.c)
    pg.solidText = S().EdText(pg, S().ED_SIZE.small)
    pg.solidText:SetPoint("TOPLEFT", PAD, y + 6); pg.solidText:SetWidth(WIN_W - 2 * PAD); pg.solidText:SetJustifyH("LEFT")
    y = y - 22
    section(pg, "Keep", y)
    y = row(pg, {
        { "Lock", function() cmd("lock") end, "lock" }, { "Unlock", function() cmd("unlock") end, "unlock" },
        { "Hide", function() cmd("hide") end, "hide" }, { "Show all", function() cmd("show") end, "show" },
        { "Measure", function() cmd("measure") end, "measure", "From you, or between two selected; its size" },
    }, y - 18, pg.c)
    pg.measureText = S().EdText(pg, S().ED_SIZE.small)
    pg.measureText:SetPoint("TOPLEFT", PAD, y + 6); pg.measureText:SetWidth(WIN_W - 2 * PAD); pg.measureText:SetJustifyH("LEFT")
    y = y - 22
    section(pg, "Replace with the gallery's pick", y)
    row(pg, {
        { "This one", function() T.Replace(false) end, "swap" }, { "All of its kind", function() T.Replace(true) end, "swapkind" },
    }, y - 18, pg.c)
end

local function buildArrange(pg)
    local y = -4
    section(pg, "Line up", y)
    y = row(pg, {
        { "Align X", function() cmd("align x") end, "ax" }, { "Align Y", function() cmd("align y") end, "ay" }, { "Align Z", function() cmd("align z") end, "az" },
        { "Bottoms", function() cmd("align bottom") end, "abottom" }, { "Tops", function() cmd("align top") end, "atop" },
    }, y - 18, pg.c)
    y = row(pg, {
        { "Spread X", function() cmd("distribute x") end, "dx" }, { "Spread Y", function() cmd("distribute y") end, "dy" }, { "Spread Z", function() cmd("distribute z") end, "dz" },
    }, y, pg.c)
    section(pg, "Array: copies in a row", y)
    y = y - 18
    pg.n = box(pg, 44, "4", "TOPLEFT", PAD, y)
    pg.dx = box(pg, 56, "2", "TOPLEFT", PAD + 52, y)
    pg.dy = box(pg, 56, "0", "TOPLEFT", PAD + 114, y)
    pg.dz = box(pg, 56, "0", "TOPLEFT", PAD + 176, y)
    pg.c.array = chip(pg, "Make", nil, function() T.Array(pg.n:GetText(), pg.dx:GetText(), pg.dy:GetText(), pg.dz:GetText()) end, "TOPLEFT", PAD + 240, y)
    pg.c.array.edLabel = "copies · x · y · z yards apart"
    y = y - ROW - 4
    section(pg, "Ring: copies round a circle", y)
    y = y - 18
    pg.rn = box(pg, 44, "8", "TOPLEFT", PAD, y)
    pg.rr = box(pg, 56, "5", "TOPLEFT", PAD + 52, y)
    pg.face = "in"
    local fx = PAD + 116
    pg.faces = {}
    for _, f in ipairs({ "in", "out", "along", "keep" }) do
        local c = chip(pg, f:sub(1, 1):upper() .. f:sub(2), nil, function() pg.face = f; T.Refresh() end, "TOPLEFT", fx, y)
        fx = fx + (tonumber((c:GetWidth())) or 50) + 4
        pg.faces[f] = c
    end
    y = y - ROW
    y = row(pg, {
        { "Round it", function() T.Ring(pg.rn:GetText(), pg.rr:GetText(), pg.face, false) end, "ring", "Round the selected prop" },
        { "Round me", function() T.Ring(pg.rn:GetText(), pg.rr:GetText(), pg.face, true) end, "ringme", "Round where you stand" },
    }, y, pg.c)
    section(pg, "Copy", y)
    row(pg, {
        { "Copy", function() cmd("copy") end, "copy" }, { "Paste", function() local b = B(); if b then b.Paste() end end, "paste" },
        { "Duplicate", function() cmd("dup") end, "dup" },
    }, y - 18, pg.c)
end

local function buildPath(pg)
    local y = -4
    section(pg, "Made of", y)
    pg.pick = S().EdText(pg, S().ED_SIZE.text)
    pg.pick:SetPoint("TOPLEFT", PAD + 80, y); pg.pick:SetWidth(WIN_W - 2 * PAD - 80); pg.pick:SetJustifyH("LEFT")
    y = row(pg, {
        { "Fence", function() T.Preset("fence") end, "fence", "End to end on the middles, along the path, every 2 yd" },
        { "Lamps", function() T.Preset("lamps") end, "lamps", "Both sides, facing the path, every 6 yd" },
        { "Road", function() T.Preset("road") end, "road", "On the line, along it, every 2 yd" },
    }, y - 20, pg.c)
    section(pg, "Every (yd)", y + 4)
    pg.every = box(pg, 56, "3", "TOPLEFT", PAD + 90, y + 8)
    pg.every:SetScript("OnEnterPressed", function(self) T.SetLay("every", self:GetText()); self:ClearFocus() end)
    pg.c.smooth = chip(pg, "Smooth", nil, function() T.SetLay("smooth", not T.lay.smooth) end, "TOPLEFT", PAD + 156, y + 8)
    pg.c.mid = chip(pg, "On the middles", nil, function() T.SetLay("mid", not T.lay.mid) end, "TOPLEFT", PAD + 236, y + 8)
    y = y - ROW + 8
    y = row(pg, {
        { "Along", function() T.SetLay("face", "along") end, "along" }, { "Across", function() T.SetLay("face", "across") end, "across" },
        { "Line", function() T.SetLay("sides", 0) end, "side0" }, { "Left", function() T.SetLay("sides", 1) end, "side1" },
        { "Right", function() T.SetLay("sides", 2) end, "side2" }, { "Both", function() T.SetLay("sides", 3) end, "side3" },
    }, y, pg.c)
    section(pg, "Off the line (yd)", y + 4)
    pg.offset = box(pg, 56, "2", "TOPLEFT", PAD + 120, y + 8)
    pg.offset:SetScript("OnEnterPressed", function(self) T.SetLay("offset", self:GetText()); self:ClearFocus() end)
    y = y - ROW + 4
    section(pg, "Points", y)
    pg.ptsText = S().EdText(pg, S().ED_SIZE.small)
    pg.ptsText:SetPoint("TOPLEFT", PAD + 60, y); pg.ptsText:SetWidth(WIN_W - 2 * PAD - 60); pg.ptsText:SetJustifyH("LEFT")
    y = row(pg, {
        { "Add points", function() if T.mode == "point" then T.SetMode(nil) else T.SetMode("point"); cmd("path show") end end, "add", "Then click the ground along the way (right-click stops)" },
        { "Undo point", function() cmd("path undo") end, "undo" }, { "Clear", function() cmd("path clear") end, "clear" },
        { "Edit a laid path", function() cmd("path edit") end, "edit", "Select one of its props first: its points come back" },
    }, y - 18, pg.c)
    pg.idx = box(pg, 44, "1", "TOPLEFT", PAD, y)
    pg.c.move = chip(pg, "Move point", nil, function() T.SetMode("move", tonumber(pg.idx:GetText()) or 1) end, "TOPLEFT", PAD + 52, y)
    pg.c.del = chip(pg, "Delete point", nil, function() cmd("path del " .. ((tonumber(pg.idx:GetText()) or 1) - 1)) end, "TOPLEFT", PAD + 160, y)
    y = y - ROW
    pg.c.lay = chip(pg, "Lay", 120, function()
        T.SetLay("every", pg.every:GetText()); T.SetLay("offset", pg.offset:GetText()); T.Lay()
    end, "TOPLEFT", PAD, y)
    pg.c.lay.edLabel = "Put the props along the path (one undo)"
end

local function buildStamps(pg)
    local y = -4
    pg.cats = CreateFrame("Frame", nil, pg)
    pg.cats:SetPoint("TOPLEFT", pg, "TOPLEFT", 0, y); pg.cats:SetWidth(WIN_W); pg.cats:SetHeight(ROW * 2)
    pg.catChips = {}
    y = y - ROW * 2
    -- the list (left) and the picture (right)
    pg.rows = {}
    for i = 1, STAMP_ROWS do
        local c = chip(pg, "", 250, function() local it = pg.rowItems and pg.rowItems[i]; if it then T.PickStamp(it.key) end end, "TOPLEFT", PAD, y - (i - 1) * ROW)
        if c.edText then c.edText:SetJustifyH("LEFT") end
        pg.rows[i] = c
    end
    pg.c.prev = chip(pg, "<", 28, function() T.page = math.max(0, T.page - 1); T.Refresh() end, "TOPLEFT", PAD, y - STAMP_ROWS * ROW)
    pg.c.next = chip(pg, ">", 28, function() T.page = T.page + 1; T.Refresh() end, "TOPLEFT", PAD + 34, y - STAMP_ROWS * ROW)
    pg.pageText = S().EdText(pg, S().ED_SIZE.small)
    pg.pageText:SetPoint("TOPLEFT", PAD + 72, y - STAMP_ROWS * ROW - 6)
    pg.plan = CreateFrame("Frame", nil, pg)
    pg.plan:SetWidth(PLAN_PX); pg.plan:SetHeight(PLAN_PX)
    pg.plan:SetPoint("TOPRIGHT", pg, "TOPRIGHT", -PAD - 10, y)
    S().EdPanel(pg.plan, 0.7, S().ED.tile)
    pg.planTex = {}
    pg.planName = S().EdText(pg, S().ED_SIZE.small)
    pg.planName:SetPoint("TOPRIGHT", pg, "TOPRIGHT", -PAD, y - PLAN_PX - 4); pg.planName:SetWidth(PLAN_PX + 40); pg.planName:SetJustifyH("RIGHT")
    y = y - STAMP_ROWS * ROW - ROW - 4
    section(pg, "Before it goes down", y)
    pg.xfText = S().EdText(pg, S().ED_SIZE.small)
    pg.xfText:SetPoint("TOPLEFT", PAD + 170, y); pg.xfText:SetWidth(WIN_W - 2 * PAD - 170); pg.xfText:SetJustifyH("LEFT")
    y = row(pg, {
        { "Turn -15", function() T.Turn(-15) end, "turnL" }, { "Turn +15", function() T.Turn(15) end, "turnR" },
        { "No mirror", function() T.Mirror("none") end, "mnone" }, { "Mirror L/R", function() T.Mirror("lr") end, "mlr" }, { "Mirror F/B", function() T.Mirror("fb") end, "mfb" },
        { "Smaller", function() T.Size(-0.25) end, "smaller" }, { "Bigger", function() T.Size(0.25) end, "bigger" },
    }, y - 18, pg.c)
    y = row(pg, {
        { "Place with the mouse", function() T.PlaceStamp() end, "place", "Click the ground: it goes there; click again to move it" },
        { "Keep (Enter)", function() T.Keep() end, "keep" }, { "Take back (Esc)", function() T.CancelStamp() end, "cancel" },
    }, y, pg.c)
    section(pg, "Save the selection as a stamp", y)
    y = y - 18
    pg.saveName = box(pg, 120, "", "TOPLEFT", PAD, y)
    pg.saveTitle = box(pg, 150, "", "TOPLEFT", PAD + 128, y)
    pg.c.save = chip(pg, "Save", nil, function() T.SaveStamp(pg.saveName:GetText(), T.cat, pg.saveTitle:GetText()) end, "TOPLEFT", PAD + 286, y)
    pg.c.save.edLabel = "A name (letters, digits, - and _) and a title; in the category shown, else Mine. Your name is its credit."
    pg.c.del = chip(pg, "Delete", nil, function() if T.stampKey then cmd("stamp delete " .. T.stampKey) end end, "TOPLEFT", PAD + 346, y)
    pg.c.del.edLabel = "Delete the picked stamp (only one you saved)"
end

local function buildList(pg, n, onRow)
    pg.rows = {}
    for i = 1, n do
        local c = chip(pg, "", WIN_W - 2 * PAD, function() onRow(i) end, "TOPLEFT", PAD, -4 - (i - 1) * ROW)
        if c.edText then c.edText:SetJustifyH("LEFT") end
        pg.rows[i] = c
    end
end

local function buildLayers(pg)
    pg.rows = {}
    pg.btns = {}
    for i = 1, 7 do
        local y = -4 - (i - 1) * ROW
        local c = chip(pg, "", 230, function() local g = T.groups[i]; if g then cmd("layer " .. g.id .. " select") end end, "TOPLEFT", PAD, y)
        if c.edText then c.edText:SetJustifyH("LEFT") end
        local hb = chip(pg, "Hide", 58, function() local g = T.groups[i]; if g then cmd("layer " .. g.id .. (g.hidden and " show" or " hide")) end end, "TOPLEFT", PAD + 236, y)
        local lb = chip(pg, "Lock", 64, function() local g = T.groups[i]; if g then cmd("layer " .. g.id .. (g.locked and " unlock" or " lock")) end end, "TOPLEFT", PAD + 300, y)
        pg.rows[i], pg.btns[i] = c, { hide = hb, lock = lb }
    end
    local y = -4 - 7 * ROW
    pg.name = box(pg, 160, "", "TOPLEFT", PAD, y)
    pg.c.group = chip(pg, "Group the selection", nil, function() cmd(("group " .. (pg.name:GetText() or "")):gsub("%s+$", "")) end, "TOPLEFT", PAD + 168, y)
    pg.c.ungroup = chip(pg, "Ungroup", nil, function() cmd("ungroup") end, "TOPLEFT", PAD + 168 + 150, y)
    pg.empty = S().EdText(pg, S().ED_SIZE.small)
    pg.empty:SetPoint("TOPLEFT", PAD, -8); pg.empty:SetWidth(WIN_W - 2 * PAD); pg.empty:SetJustifyH("LEFT")
    pg.empty:SetText("No groups yet: select some props (the Box) and Group them. Paths and stamps are groups by themselves.")
end

local function build()
    win = CreateFrame("Frame", "WrathUnleashedToolsFrame", UIParent)
    win:SetWidth(WIN_W); win:SetHeight(560)
    win:SetPoint("TOPLEFT", UIParent, "TOPLEFT", 80, -112)
    win:SetFrameStrata("DIALOG")
    win:EnableMouse(true)
    win:SetMovable(true)
    win:RegisterForDrag("LeftButton")
    win:SetScript("OnDragStart", win.StartMoving)
    win:SetScript("OnDragStop", win.StopMovingOrSizing)
    S().EdPanel(win, 0.92, S().ED.panel2)
    S().EdLabel(win, "Tools", true, "TOPLEFT", PAD, -12)
    win.close = chip(win, "x", 28, function() win:Hide() end, "TOPRIGHT", -8, -6)
    if UISpecialFrames then tinsert(UISpecialFrames, "WrathUnleashedToolsFrame") end
    win.tabs, win.pages = {}, {}
    local x = PAD
    for _, t in ipairs(TABS) do
        local c = chip(win, t[2], nil, function() T.Show(t[1]) end, "TOPLEFT", x, -40)
        x = x + (tonumber((c:GetWidth())) or 70) + 4
        win.tabs[t[1]] = c
    end
    buildSelect(page("select"))
    buildArrange(page("arrange"))
    buildPath(page("path"))
    buildStamps(page("stamps"))
    local hp = page("history")
    buildList(hp, 12, function(i) if T.history[i] then cmd("undoto " .. i) end end)
    buildLayers(page("layers"))
    win.status = S().EdText(win, S().ED_SIZE.small)
    win.status:SetPoint("BOTTOMLEFT", PAD, 10); win.status:SetWidth(WIN_W - 2 * PAD); win.status:SetJustifyH("LEFT")
    local m = S().ED.muted
    win.status:SetTextColor(m[1], m[2], m[3])
    win:SetScript("OnShow", function() T.Refresh() end)
    -- closed: a waiting tool stops (a stamp that is down stays: Enter keeps it), the path's stakes go (its points stay)
    win:SetScript("OnHide", function()
        if T.mode and T.mode ~= "stamp" then T.SetMode(nil) end
        if #T.pts > 0 then cmd("path hide") end
    end)
    win:Hide()
end

local function activeIf(c, on) if c and c.SetActive then c:SetActive(on and true or false) end end

local function paintPlan(pg, key)
    for _, t in ipairs(pg.planTex) do t:Hide() end
    local rects = key and T.plans[key] or nil
    pg.planName:SetText(key and (T.stampName or key) or "")
    if not rects then return end
    local k = PLAN_PX / 64
    for i, r in ipairs(rects) do
        local t = pg.planTex[i]
        if not t then t = pg.plan:CreateTexture(nil, "ARTWORK"); pg.planTex[i] = t end
        local c = PLAN_COLOURS[r[5]] or PLAN_COLOURS[2]
        t:SetTexture(c[1], c[2], c[3], c[4])
        t:ClearAllPoints()
        t:SetWidth(math.max(2, r[3] * k)); t:SetHeight(math.max(2, r[4] * k))
        t:SetPoint("CENTER", pg.plan, "BOTTOMLEFT", r[1] * k, r[2] * k)
        t:Show()
    end
end

function T.Refresh()
    if not win or not win:IsShown() then return end
    for k, pg in pairs(win.pages) do if k == T.tab then pg:Show() else pg:Hide() end end
    for k, c in pairs(win.tabs) do activeIf(c, k == T.tab) end
    local b = B()
    local st = (b and b.state) or {}
    -- Select
    local p = win.pages.select
    activeIf(p.c.box, T.mode == "box1" or T.mode == "box2")
    for _, g in ipairs({ 0, 0.5, 1, 2 }) do activeIf(p.c["grid" .. g], b and b.Snap and b.Snap() == g) end
    for _, a in ipairs({ 15, 45, 90 }) do activeIf(p.c["angle" .. a], b and b.TurnStep and b.TurnStep() == a) end
    activeIf(p.c.solidOn, st.solid == "on"); activeIf(p.c.solidOff, st.solid == "off"); activeIf(p.c.solidAuto, st.solid == "auto")
    local coll = ({ model = "its model is solid in your game", none = "its model has no collision: Solid keeps you on its top only",
        unknown = "your game decides from its model: try it" })[st.coll or ""]
    p.solidText:SetText(st.selId and coll and ("This one: " .. coll .. ".") or "Select a prop to set it.")
    local mz = T.measured
    p.measureText:SetText(mz and string.format("%.2f yd (%.2f on the flat, %.2f up) · it is %.1f x %.1f x %.1f yd", mz.d or 0, mz.flat or 0, mz.up or 0, mz.w or 0, mz.dp or 0, mz.h or 0) or "")
    -- Arrange
    local a = win.pages.arrange
    for f, c in pairs(a.faces) do activeIf(c, a.face == f) end
    -- Path
    local pp = win.pages.path
    local pick = b and b.pick and not b.pick.kind and b.pick or nil
    pp.pick:SetText(pick and pick.name or (T.pathEntry and ("The path's own (" .. T.pathEntry .. ")")) or "Pick one in the gallery (a fence, a lamp, a road tile)")
    local l = T.lay
    if not pp.every.HasFocus or not pp.every:HasFocus() then pp.every:SetText(tostring(l.every)) end
    if not pp.offset.HasFocus or not pp.offset:HasFocus() then pp.offset:SetText(tostring(l.offset)) end
    activeIf(pp.c.smooth, l.smooth); activeIf(pp.c.mid, l.mid)
    activeIf(pp.c.along, l.face == "along"); activeIf(pp.c.across, l.face == "across")
    for i = 0, 3 do activeIf(pp.c["side" .. i], l.sides == i) end
    activeIf(pp.c.add, T.mode == "point"); activeIf(pp.c.move, T.mode == "move")
    local n = #T.pts
    pp.ptsText:SetText(n == 0 and "None yet." or string.format("%d point%s · %.0f yd%s", n, n == 1 and "" or "s", T.len or 0,
        T.editing and " · editing a laid path (Lay puts it in place of the old one)" or ""))
    pp.c.lay:SetText(T.editing and "Lay again" or "Lay")
    -- Stamps
    local sp = win.pages.stamps
    local cats = T.Categories()
    local cx, cy = PAD, 0
    for i, c in ipairs(cats) do
        local ch = sp.catChips[i]
        if not ch then
            ch = chip(sp.cats, "", nil, function() T.cat = ch.cat; T.page = 0; T.Refresh(); if b and b.Filter then b.Filter() end end)
            sp.catChips[i] = ch
        end
        ch.cat = c
        ch:SetText(CAT_TEXT[c] or c)
        local w = tonumber((ch:GetWidth())) or 70
        if cx > PAD and cx + w > WIN_W - PAD then cx, cy = PAD, cy - ROW end
        ch:ClearAllPoints(); ch:SetPoint("TOPLEFT", sp.cats, "TOPLEFT", cx, cy)
        cx = cx + w + 4
        activeIf(ch, T.cat == c)
        ch:Show()
    end
    for i = #cats + 1, #sp.catChips do sp.catChips[i]:Hide() end
    local items = T.StampItems()
    local pages = math.max(1, math.ceil(#items / STAMP_ROWS))
    if T.page >= pages then T.page = pages - 1 end
    sp.rowItems = {}
    for i = 1, STAMP_ROWS do
        local it = items[T.page * STAMP_ROWS + i]
        sp.rowItems[i] = it
        local c = sp.rows[i]
        if it then
            local tag = it.origin == "mine" and " · yours" or (it.origin == "copied" and " · copied" or "")
            c:SetText(string.format("%s  (%d%s)", it.name, it.count or 0, tag))
            c.edLabel = it.line
            activeIf(c, T.stampKey == it.key)
            if T.stampKey == it.key then T.stampName = it.name end
            c:Show()
        else
            c:Hide()
        end
    end
    sp.pageText:SetText(#items == 0 and "No stamps yet." or string.format("%d-%d of %d", T.page * STAMP_ROWS + 1, math.min(#items, (T.page + 1) * STAMP_ROWS), #items))
    paintPlan(sp, T.stampKey)
    local x = T.xf
    sp.xfText:SetText(string.format("turned %d° · %s · size %.2f", x.turn, ({ none = "not mirrored", lr = "mirrored left to right", fb = "mirrored front to back" })[x.mirror] or "", x.scale))
    activeIf(sp.c.mnone, x.mirror == "none"); activeIf(sp.c.mlr, x.mirror == "lr"); activeIf(sp.c.mfb, x.mirror == "fb")
    activeIf(sp.c.place, T.mode == "stamp")
    activeIf(sp.c.keep, T.staged ~= nil); activeIf(sp.c.cancel, T.staged ~= nil)
    -- Layers
    local lp = win.pages.layers
    for i, c in ipairs(lp.rows) do
        local g = T.groups[i]
        if g then
            c:SetText(string.format("%s  (%d)", g.name ~= "" and g.name or ("Group " .. g.id), g.count or 0))
            lp.btns[i].hide:SetText(g.hidden and "Show" or "Hide"); lp.btns[i].lock:SetText(g.locked and "Unlock" or "Lock")
            c:Show(); lp.btns[i].hide:Show(); lp.btns[i].lock:Show()
        else
            c:Hide(); lp.btns[i].hide:Hide(); lp.btns[i].lock:Hide()
        end
    end
    if #T.groups == 0 then lp.empty:Show() else lp.empty:Hide() end
    -- History
    local hp = win.pages.history
    for i, c in ipairs(hp.rows) do
        local h = T.history[i]
        if h then c:SetText(string.format("%d. %s", i, h)); c.edLabel = "Go back to before this (" .. i .. " step" .. (i == 1 and "" or "s") .. ")"; c:Show() else c:Hide() end
    end
end

-- Open the window on a tab (and ask Den Lite for what that tab shows).
function T.Show(tab)
    if not win then build() end
    if tab then T.tab = tab end
    win:Show()
    if T.tab == "stamps" then cmd("stamps"); if T.stampKey then cmd("stampplan " .. T.stampKey) end end
    if T.tab == "layers" then cmd("groups") end
    if T.tab == "history" then cmd("history") end
    if T.tab == "path" then cmd("path show") end
    T.Refresh()
    return true
end
function T.Hide() if win then win:Hide() end end
function T.Toggle(tab)
    if win and win:IsShown() and (not tab or tab == T.tab) then win:Hide(); return false end
    return T.Show(tab)
end
function T.Window() if not win then build() end return win end
-- Den Lite's (its caps say `tool`): the rail's Path and Stamps open this window there
function T.Here()
    local b = B()
    return b and b.Can and b.Can("tool") or false
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:SetScript("OnEvent", function(_, _, prefix, msg, channel, sender) T.OnMessage(prefix, msg, channel, sender) end)

SLASH_WUTOOLS1 = "/tools"
SlashCmdList["WUTOOLS"] = function(msg) T.Toggle((msg and msg ~= "") and msg:lower() or nil) end

T.TABS, T.PRESETS = TABS, PRESETS
W.BuildTools = T
