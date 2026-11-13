-- Your Den's editor keys, Noggit's way (owner 5 Nov: "every other convenient and noggit feature possible build right
-- into the den"). Only in build mode (Gizmo.lua G.build), never in play mode: every key here is an override binding of
-- this file's own frame, set again whenever Gizmo.lua sets its own (G.Bindings) and cleared when build mode ends, the
-- Den is left or the game reloads (the game drops override bindings on a reload). Den Lite's side: labs/den-lite
-- src/noggit.rs (dupby, selnear, slope, cam frame / mark / to, the flight's own speed, yaw set).
--
--   1 / 2 / 3          the tool: Move (click carries, drag slides), Turn (drag left / right turns, the wheel turns
--                      the selection), Size (drag up / down sizes, the wheel sizes the selection)
--   Ctrl+Z / Ctrl+Y    undo, redo (Ctrl+Shift+Z redo too)
--   Ctrl+C / Ctrl+V    copy the selection; paste it under the cursor (in hand, with its turn and size)
--   Ctrl+D             duplicate the selection; again carries on the row by the same offset
--   Del                delete the selection          Ctrl+A   select every prop within 30 yards of you
--   Ctrl+drag          box select (Ctrl+Shift+drag adds)
--   G / Shift+G        snap to the grid on or off / the grid's size (0.5, 1, 2, 5 yards)
--   R / Shift+R        the turn step (15, 45, 90) / scatter: a random turn and size for each prop placed
--   L / Shift+L        lie on the ground's slope / stand upright
--   F                  frame the selection (the far view round it)
--   Home, Numpad 0     the view back to your character (from the far view)
--   Ctrl+1..3          keep where the view is; Shift+1..3 go back to it (kept between sessions)
--   right mouse + wheel   the flight's speed (2 to 96 yards a second, shown in the status line)
-- The builder's comforts (12 Nov; Den Lite labs/den-lite src/qol.rs, the caps say which it has):
--   Numpad 8 / 2 / 4 / 6, Ctrl+arrows   nudge the selection away / back / left / right of the view, on the grid's axes
--   Numpad 9 / 3, Ctrl+PgUp / PgDn      nudge it up / down;  Numpad 7 / 1  turn it left / right by the turn step
--                      (Alt: fine, a tenth of a yard and 1 degree; Shift: five steps, a quarter turn)
--   M                  the magnet: in hand near another of its kind, it snaps edge to edge (walls, fences, rows)
--   Ctrl+L             follow the slope: what is placed or moved lies on the ground's slope (L does the selection now)
--   Alt+drag           circle select (Alt+Shift+drag adds); a selection dragged by one of its props moves as one
--   Alt held           while something is in hand: no grid and no magnet (the precise spot); Q / E turn by 1 degree
-- Esc is Gizmo.lua's (G.Escape): the first press cancels what is going on, the next deselects, the third is the game's.

local W = WrathUnleashedUI or {}
local K = { speedI = 4, scatter = false, bound = {} }
W.DenKeys = K

K.FLY_SPEEDS = { 2, 4, 8, 16, 24, 32, 48, 64, 96 }   -- yards a second; 16 is Den Lite's FLY_SPEED
K.FLY_DEFAULT = 4
K.FAST, K.SLOW = 3, 1 / 3                            -- Shift and Alt while flying
K.NEAR_YARDS = 30                                    -- Ctrl+A
K.GRIDS = { 0.5, 1, 2, 5 }
K.ANGLES = { 15, 45, 90 }
K.ROT_PX = 20                                        -- pixels of a Turn drag per turn step
K.SCALE_PX = 1.01                                    -- a Size drag: this much bigger each pixel up
K.SCATTER = { 0.85, 1.15 }                           -- scatter's sizes
K.MARKS = 3
K.NUDGE = 0.5                                        -- yards a nudge moves with the grid off (the grid's own step when on)
K.NUDGE_FINE, K.NUDGE_BIG = 0.1, 5                   -- Alt: a tenth of a yard; Shift: five steps
K.TURN_FINE, K.TURN_BIG = 1, 90                      -- Alt: 1 degree; Shift: a quarter turn
-- the nudge keys: forward (away from the view), right, up, turn (+ left)
K.NUDGE_KEYS = {
    NUMPAD8 = { 1, 0, 0, 0 }, NUMPAD2 = { -1, 0, 0, 0 }, NUMPAD4 = { 0, -1, 0, 0 }, NUMPAD6 = { 0, 1, 0, 0 },
    NUMPAD9 = { 0, 0, 1, 0 }, NUMPAD3 = { 0, 0, -1, 0 }, NUMPAD7 = { 0, 0, 0, 1 }, NUMPAD1 = { 0, 0, 0, -1 },
    ["CTRL-UP"] = { 1, 0, 0, 0 }, ["CTRL-DOWN"] = { -1, 0, 0, 0 }, ["CTRL-LEFT"] = { 0, -1, 0, 0 }, ["CTRL-RIGHT"] = { 0, 1, 0, 0 },
    ["CTRL-PAGEUP"] = { 0, 0, 1, 0 }, ["CTRL-PAGEDOWN"] = { 0, 0, -1, 0 },
}
K.CLICK_MOVE = 6
K.TOOLS = { "move", "rotate", "scale" }
K.TOOL_NAME = { move = "Move", rotate = "Turn", scale = "Size" }

local function G() return W.Gizmo end
local function B() return W.Builder end
local function now() return GetTime and GetTime() or 0 end
local function send(text)
    local g = G()
    if g and g.Send then return g.Send(text) end
    return false
end
-- a Builder verb (the Builder's own path: its status line and edit mode)
local function cmd(text)
    local b = B()
    if b and b.Command then return b.Command(text) end
    return send(text)
end
local function can(verb)
    local b = B()
    return (b and b.Can and b.Can(verb)) and true or false
end
local function note(text)
    local b = B()
    if b and b.PillNote then b.PillNote(text) elseif b and b.SetStatus then b.SetStatus(text) end
    K.lastNote = text
end
local function db()
    WrathUnleashedUIDB = type(WrathUnleashedUIDB) == "table" and WrathUnleashedUIDB or {}
    local d = WrathUnleashedUIDB
    d.den = type(d.den) == "table" and d.den or {}
    return d.den
end
K.db = db
local function refresh()
    local g = G()
    if g and g.Bindings then g.Bindings() else K.Bind() end
    if g and g.ShowTag then g.ShowTag() end
end

-- ---------------------------------------------------------------- what is selected (Den Lite's state line)

-- The selection as the state says it: `sel=<id>,<entry>,<name>;n=<count>;pos=<x>,<y>,<z>,<yaw>,<scale>;`.
function K.ParseState(data)
    local id, entry, name = data:match("sel=(%d+),(%d+),([^;]*);")
    if not id then K.sel = nil; return nil end
    local x, y, z, yaw, scale = data:match("pos=([%-%d%.]+),([%-%d%.]+),([%-%d%.]+),([%-%d%.]+),([%-%d%.]+);")
    K.sel = { id = tonumber(id), entry = tonumber(entry), name = name, n = tonumber(data:match("n=(%d+);") or "1") or 1,
        x = tonumber(x), y = tonumber(y), z = tonumber(z), yaw = tonumber(yaw), scale = tonumber(scale) }
    return K.sel
end

function K.SelId()
    local g, b = G(), B()
    return (g and g.sel) or (K.sel and K.sel.id) or (b and b.state and b.state.selId) or nil
end

-- ---------------------------------------------------------------- the tool (1 / 2 / 3)

-- The tool now: the Builder's mode when it is one of the three (the rail's Move and Rotate set it too), else Move.
function K.Tool()
    local b = B()
    local m = b and b.mode
    if m == "rotate" or m == "scale" then return m end
    return "move"
end
function K.SetTool(t)
    if not K.TOOL_NAME[t] then return false end
    local b = B()
    if b and b.SetMode then b.SetMode(t) else K.tool = t end
    if b then b.mode = t end
    note(K.TOOL_NAME[t] .. " tool (" .. (t == "move" and 1 or t == "rotate" and 2 or 3) .. ")")
    refresh()
    return t
end

-- The wheel with something selected and nothing in hand: Turn turns it by the turn step (Shift: 5°), Size sizes it.
-- Move: false (the wheel zooms, as always).
function K.ToolWheel(sign)
    local t = K.Tool()
    if t == "move" or not K.SelId() then return false end
    if t == "rotate" then
        local step = (IsShiftKeyDown and IsShiftKeyDown()) and 5 or ((B() and B().TurnStep and B().TurnStep()) or 15)
        return cmd("turn " .. (sign * step))
    end
    return cmd(string.format("grow %.3f", sign > 0 and 1.1 or (1 / 1.1)))
end

-- ---------------------------------------------------------------- a Turn or Size drag on a prop

local function cursorPx()
    local g = G()
    if g and g.fake then return g.fake[1] * 1000, g.fake[2] * 1000 end
    if GetCursorPosition then return GetCursorPosition() end
    return 0, 0
end

-- A press on a prop moved past the click's few pixels with the Turn or Size tool: the prop is selected (a plain
-- click there) and, for Turn, picked up in place (`preview move`: the copy turns live with `preview turn`, the let-go
-- puts it down with one undo; the cursor's place is not sent, so it stays where it stood). `d`: Gizmo.lua's press.
function K.DragStart(d)
    local t = K.Tool()
    if t == "move" or not d or not d.prop or not d.u then return false end
    local g = G()
    send(string.format("click %.4f %.4f %.4f %d", d.u, d.v, d.a or (16 / 9), (g and g.zoom) or 12))
    K.drag = { kind = t, x0 = d.x, y0 = d.y, deg = 0, k = 1 }
    if t == "rotate" then
        send("preview move")
        note("Turning: move the mouse left or right, let go to keep it, Esc puts it back")
    else
        note("Sizing: move the mouse up or down, let go to keep it, Esc leaves it")
    end
    return true
end

function K.DragUpdate()
    local dr = K.drag
    if not dr then return false end
    local x, y = cursorPx()
    if dr.kind == "rotate" then
        local step = (B() and B().TurnStep and B().TurnStep()) or 15
        local px = (x or 0) - dr.x0
        local steps = px >= 0 and math.floor(px / K.ROT_PX) or -math.floor(-px / K.ROT_PX)
        local deg = -steps * step            -- to the right turns clockwise
        if deg ~= dr.deg then
            send("preview turn " .. (deg - dr.deg))
            dr.deg = deg
            note(string.format("Turned %+d°: let go to keep it", deg))
            return true
        end
    else
        local k = math.max(0.2, math.min(5, K.SCALE_PX ^ ((y or 0) - dr.y0)))
        if math.abs(k - dr.k) > 0.005 then
            dr.k = k
            note(string.format("Size x%.2f: let go to keep it", k))
            return true
        end
    end
    return false
end

function K.DragEnd()
    local dr = K.drag
    K.drag = nil
    if not dr then return false end
    K.DragUpdate()
    if dr.kind == "rotate" then return send("commit") end
    if math.abs(dr.k - 1) > 0.01 then return cmd(string.format("grow %.3f", dr.k)) end
    return true
end
function K.DragCancel() K.drag = nil end

-- ---------------------------------------------------------------- copy, paste, duplicate

function K.Copy()
    local s = K.sel
    if s and s.entry then K.clip = { entry = s.entry, yaw = s.yaw or 0, scale = s.scale or 1, name = s.name } end
    cmd("copy")
    if K.clip then note("Copied " .. (K.clip.name or "it") .. ": Ctrl+V puts it under the cursor") end
    refresh()
    return true
end

-- Ctrl+V: the copied prop in hand under the cursor, with its turn (`yaw set`) and, once put down, its size. A Den Lite
-- without noggit.rs: the Builder's own Paste (in front of you).
function K.Paste()
    local c, g = K.clip, G()
    if not (c and c.entry and can("dupby") and g and g.Arm) then
        local b = B()
        if b and b.Paste then return b.Paste() end
        return cmd("paste")
    end
    send(string.format("yaw set %d", math.floor((c.yaw or 0) + 0.5)))
    K.pasting = c.scale or 1
    return g.Arm(c.entry)
end

-- Ctrl+D: the selection copied by the last offset (a yard along x the first time). Moving the copy before pressing
-- again changes the offset: the next copy goes as far from it as it went from its original (Noggit's way).
function K.Duplicate()
    local s = K.sel
    if not (can("dupby") and s and s.x) then return cmd("dup") end
    local off = K.dupOff or { 1, 0, 0 }
    if K.dupLast and K.dupLast == s.id and K.dupSrc then
        off = { s.x - K.dupSrc[1], s.y - K.dupSrc[2], s.z - K.dupSrc[3] }
    end
    if math.abs(off[1]) + math.abs(off[2]) + math.abs(off[3]) < 0.01 then off = { 1, 0, 0 } end
    K.dupOff, K.dupSrc, K.dupWait, K.dupAt = off, { s.x, s.y, s.z }, s.id, now()
    return cmd(string.format("dupby %.2f %.2f %.2f", off[1], off[2], off[3]))
end

-- ---------------------------------------------------------------- the camera: speed, frame, marks, home

function K.Speed() return K.FLY_SPEEDS[K.speedI] or 16 end
function K.SpeedStep(sign)
    K.speedI = math.max(1, math.min(#K.FLY_SPEEDS, K.speedI + (sign > 0 and 1 or -1)))
    db().flySpeed = K.speedI
    note(string.format("Flying at %d yards a second", K.Speed()))
    local g = G()
    if g and g.far and g.FlySend then g.FlySend() end
    if g and g.ShowTag then g.ShowTag() end
    return true
end
-- The speed word of `cam fly`: Den Lite with noggit.rs takes the speed itself (`v<yards a second>`, Shift and Alt in
-- it); before it, ` 1` (fast) or ` s` (slow).
function K.SpeedWord()
    local shift, alt = IsShiftKeyDown and IsShiftKeyDown(), IsAltKeyDown and IsAltKeyDown()
    if can("flyspeed") then
        local v = K.Speed() * (shift and K.FAST or alt and K.SLOW or 1)
        return string.format(" v%.1f", v)
    end
    return shift and " 1" or (alt and " s" or "")
end
function K.RightHeld()
    if not IsMouseButtonDown then return false end
    local ok, held = pcall(IsMouseButtonDown, "RightButton")
    return ok and held and true or false
end

function K.Mark(n)
    K.markWait = n
    return send("cam mark")
end
function K.Recall(n)
    local m = (db().camMarks or {})[n]
    if type(m) ~= "string" or not m:match("^[%-%d%.,]+$") then
        note("No view kept in " .. n .. ": Ctrl+" .. n .. " keeps where the view is")
        return false
    end
    return send("cam to " .. m:gsub(",", " "))
end

-- ---------------------------------------------------------------- the grid, the turn step, scatter

function K.GridToggle()
    local b = B()
    if not (b and b.SetSnap) then return false end
    local g = (b.Snap and b.Snap()) or 0
    if g > 0 then db().lastGrid = g; b.SetSnap(0); note("Snap off")
    else local to = db().lastGrid or 1; b.SetSnap(to); note("Snap to a " .. to .. " yard grid") end
    return true
end
function K.GridSize()
    local b = B()
    if not (b and b.SetSnap) then return false end
    local g = (b.Snap and b.Snap()) or 0
    local to = K.GRIDS[1]
    for i, v in ipairs(K.GRIDS) do if math.abs(v - g) < 0.01 then to = K.GRIDS[i % #K.GRIDS + 1] end end
    db().lastGrid = to
    b.SetSnap(to)
    note("Snap to a " .. to .. " yard grid")
    return to
end
function K.AngleNext()
    local b = B()
    if not (b and b.SetAngle) then return false end
    local a = (b.TurnStep and b.TurnStep()) or 15
    local to = K.ANGLES[1]
    for i, v in ipairs(K.ANGLES) do if v == a then to = K.ANGLES[i % #K.ANGLES + 1] end end
    b.SetAngle(to)
    note("Turn step " .. to .. "°")
    return to
end
function K.ScatterToggle()
    K.scatter = not K.scatter
    db().scatter = K.scatter
    note(K.scatter and "Scatter on: each prop placed gets a random turn and size" or "Scatter off")
    -- a card in hand turns at once
    local g = G()
    if K.scatter and g and g.armed == "place" then K.ScatterTurn() end
    refresh()
    return K.scatter
end
function K.ScatterTurn()
    local r = math.random and math.random(0, 23) or 7
    return send("preview turn " .. (r * 15))
end

-- ---------------------------------------------------------------- the builder's comforts (Den Lite's qol.rs)

local function alt() return IsAltKeyDown and IsAltKeyDown() and true or false end
local function shift() return IsShiftKeyDown and IsShiftKeyDown() and true or false end

-- One nudge of the selection: `v` = { forward, right, up, turn sign }. The step is the grid's (or K.NUDGE), Alt a
-- tenth of a yard (1 degree), Shift five steps (a quarter turn). A Den Lite without qol.rs: the Builder's own `step`.
function K.NudgeStep()
    if alt() then return K.NUDGE_FINE end
    local b = B()
    local g = (b and b.Snap and b.Snap()) or 0
    local s = (g and g > 0) and g or K.NUDGE
    return shift() and s * K.NUDGE_BIG or s
end
function K.TurnStepNow()
    if alt() then return K.TURN_FINE end
    if shift() then return K.TURN_BIG end
    return (B() and B().TurnStep and B().TurnStep()) or 15
end
function K.Nudge(v)
    if not K.SelId() then note("Select a prop first: the nudge keys move the selection"); return false end
    local s, deg = K.NudgeStep(), (v[4] or 0) * K.TurnStepNow()
    if can("nudge") then
        K.lastNudge = string.format("nudge %.2f %.2f %.2f %d", v[1] * s, v[2] * s, v[3] * s, deg)
        return send(K.lastNudge)
    end
    if deg ~= 0 then return cmd("turn " .. deg) end
    local dir = (v[1] > 0 and "fwd") or (v[1] < 0 and "back") or (v[2] > 0 and "right") or (v[2] < 0 and "left") or (v[3] > 0 and "up") or "down"
    return cmd(string.format("step %s %.2f", dir, s))
end

-- The magnet (M) and following the slope (Ctrl+L): kept in the saved settings, said to Den Lite on entering build
-- mode (K.OnBuild) and when changed.
function K.MagnetToggle()
    K.magnet = not K.magnet
    db().magnet = K.magnet
    if can("magnet") then send("gizmo magnet " .. (K.magnet and "on" or "off")) end
    note(K.magnet and "Magnet on: a prop in hand snaps edge to edge with one of its kind (Alt: free)" or "Magnet off")
    if not K.magnet then K.snapped = nil end
    refresh()
    return K.magnet
end
function K.SlopeToggle()
    K.slope = not K.slope
    db().slope = K.slope
    if can("slopefollow") then send("gizmo slope " .. (K.slope and "on" or "off")) end
    note(K.slope and "Follow the slope on: what you place or move lies on the ground" or "Follow the slope off: props stand upright")
    refresh()
    return K.slope
end
function K.OnBuild()
    if can("magnet") then send("gizmo magnet " .. (K.magnet and "on" or "off")) end
    if can("slopefollow") then send("gizmo slope " .. (K.slope and "on" or "off")) end
end
-- Alt held with something in hand: this hover is free (no grid, no magnet). Gizmo.lua adds the word to its hover.
function K.FreeWord()
    if alt() and can("free") then return " free" end
    return ""
end

-- Circle select (Alt+drag): pressed in the middle, let go at the edge; the ring drawn on the screen while dragging.
local circleFrame
local function circleShow(x0, y0, x1, y1)
    if not circleFrame or not UIParent then return end
    local s = (UIParent.GetEffectiveScale and UIParent:GetEffectiveScale()) or 1
    local r = math.sqrt((x1 - x0) ^ 2 + (y1 - y0) ^ 2) / s
    circleFrame:ClearAllPoints()
    circleFrame:SetPoint("CENTER", UIParent, "BOTTOMLEFT", x0 / s, y0 / s)
    circleFrame:SetWidth(math.max(2, 2 * r)); circleFrame:SetHeight(math.max(2, 2 * r))
    circleFrame:Show()
end
function K.CircleDown()
    local g = G()
    if not (g and g.build and g.Cursor) then return false end
    local u, v, a = g.Cursor()
    if not u then return false end
    local x, y = cursorPx()
    K.circle = { u = u, v = v, a = a, x = x, y = y }
    return true
end
function K.CircleUpdate()
    if not K.circle then return false end
    local x, y = cursorPx()
    if math.abs(x - K.circle.x) > K.CLICK_MOVE or math.abs(y - K.circle.y) > K.CLICK_MOVE then circleShow(K.circle.x, K.circle.y, x, y) end
    return true
end
function K.CircleUp()
    local c, g = K.circle, G()
    K.circle = nil
    if circleFrame then circleFrame:Hide() end
    if not (c and g and g.Cursor) then return false end
    local u, v = g.Cursor()
    local x, y = cursorPx()
    if not u or (math.abs(x - c.x) <= K.CLICK_MOVE and math.abs(y - c.y) <= K.CLICK_MOVE) then return false end
    if not can("selcircle") then note("Circle select needs a newer Den Lite: Ctrl+drag a box"); return false end
    return send(string.format("selcircle %.4f %.4f %.4f %.4f %.4f %d%s", c.u, c.v, u, v, c.a or (16 / 9), g.zoom or 12, shift() and " add" or ""))
end

-- Fill (the Tools window's Arrange and these): copies of the first selected between it and the second, or over the
-- box the two make.
function K.Fill(kind, step)
    if not (K.sel and (K.sel.n or 1) >= 2) then
        note(kind == "box" and "Fill box: select two props at opposite corners first (Shift+click the second)"
            or "Fill gap: select two props first (Shift+click the second)")
        return false
    end
    if kind == "box" then return cmd(step and string.format("fill box %.2f", step) or "fill box") end
    return cmd("fill gap")
end

-- ---------------------------------------------------------------- box select (Ctrl+drag)

local boxFrame
local function boxShow(x1, y1, x2, y2)
    if not boxFrame or not UIParent then return end
    local s = (UIParent.GetEffectiveScale and UIParent:GetEffectiveScale()) or 1
    local l, r = math.min(x1, x2) / s, math.max(x1, x2) / s
    local bt, tp = math.min(y1, y2) / s, math.max(y1, y2) / s
    boxFrame:ClearAllPoints()
    boxFrame:SetPoint("BOTTOMLEFT", UIParent, "BOTTOMLEFT", l, bt)
    boxFrame:SetWidth(math.max(1, r - l)); boxFrame:SetHeight(math.max(1, tp - bt))
    boxFrame:Show()
end
function K.BoxDown()
    local g = G()
    if not (g and g.build and g.Cursor) then return false end
    local u, v, a = g.Cursor()
    if not u then return false end
    local x, y = cursorPx()
    K.box = { u = u, v = v, a = a, x = x, y = y, t = now() }
    return true
end
function K.BoxUpdate()
    if not K.box then return false end
    local x, y = cursorPx()
    if math.abs(x - K.box.x) > K.CLICK_MOVE or math.abs(y - K.box.y) > K.CLICK_MOVE then boxShow(K.box.x, K.box.y, x, y) end
    return true
end
function K.BoxUp()
    local bx, g = K.box, G()
    K.box = nil
    if boxFrame then boxFrame:Hide() end
    if not (bx and g and g.Cursor) then return false end
    local u, v = g.Cursor()
    local x, y = cursorPx()
    local add = IsShiftKeyDown and IsShiftKeyDown()
    if not u or (math.abs(x - bx.x) <= K.CLICK_MOVE and math.abs(y - bx.y) <= K.CLICK_MOVE) then
        -- a still Ctrl+click: the prop there joins the selection (as Shift+click)
        return send(string.format("click %.4f %.4f %.4f %d add", bx.u, bx.v, bx.a or (16 / 9), g.zoom or 12))
    end
    return send(string.format("tool box %.4f %.4f %.4f %.4f %.4f %d%s", bx.u, bx.v, u, v, bx.a or (16 / 9), g.zoom or 12, add and " add" or ""))
end

-- ---------------------------------------------------------------- the keys

K.ACTIONS = {
    undo = function() return cmd("undo") end,
    redo = function() return cmd("redo") end,
    tool1 = function() return K.SetTool("move") end,
    tool2 = function() return K.SetTool("rotate") end,
    tool3 = function() return K.SetTool("scale") end,
    angle = function() return K.AngleNext() end,
    scatter = function() return K.ScatterToggle() end,
    grid = function() return K.GridToggle() end,
    gridsize = function() return K.GridSize() end,
    selnear = function() return cmd(can("selnear") and ("selnear " .. K.NEAR_YARDS) or "selall") end,
    paste = function() return K.Paste() end,
    delete = function() return cmd("delete") end,
    copy = function() return K.Copy() end,
    dup = function() return K.Duplicate() end,
    frame = function() return send("cam frame") end,
    slope = function() return cmd(can("slope") and "slope" or "ground") end,
    upright = function() return cmd("upright") end,
    home = function() return send("cam off") end,
    magnet = function() return K.MagnetToggle() end,
    slopefollow = function() return K.SlopeToggle() end,
}
for key, v in pairs(K.NUDGE_KEYS) do
    K.ACTIONS["nudge_" .. key] = function() return K.Nudge(v) end
end
for i = 1, K.MARKS do
    K.ACTIONS["mark" .. i] = function() return K.Mark(i) end
    K.ACTIONS["recall" .. i] = function() return K.Recall(i) end
end

local owner
local function button(act)
    local name = "LeylineDenKey_" .. act
    if _G[name] then return name end
    if not CreateFrame or not UIParent then return name end
    local b = CreateFrame("Button", name, UIParent)
    if b.EnableMouse then b:EnableMouse(false) end
    if b.RegisterForClicks then b:RegisterForClicks("AnyDown") end
    b:SetScript("OnClick", function(_, _, down)
        if down == false then return end
        local fn = K.ACTIONS[act]
        if fn then fn() end
        refresh()
    end)
    b:Hide()
    return name
end
K.Button = button

-- Every key of this file for the moment (build mode only), as `K.bound[key] = action`.
function K.Wanted()
    local g = G()
    local out = {}
    if not (g and g.build) then return out end
    out["CTRL-Z"], out["CTRL-Y"], out["CTRL-SHIFT-Z"] = "undo", "redo", "redo"
    for i = 1, 3 do out[tostring(i)] = "tool" .. i end
    for i = 1, K.MARKS do out["CTRL-" .. i], out["SHIFT-" .. i] = "mark" .. i, "recall" .. i end
    out.R, out["SHIFT-R"] = "angle", "scatter"
    -- G is the exact click while a prop is in hand (Gizmo.lua's)
    if g.armed ~= "place" and g.armed ~= "move" then out.G = "grid" end
    out["SHIFT-G"] = "gridsize"
    out["CTRL-A"] = "selnear"
    out["CTRL-BUTTON1"], out["CTRL-SHIFT-BUTTON1"] = "box", "box"
    out["ALT-BUTTON1"], out["ALT-SHIFT-BUTTON1"] = "circle", "circle"
    out.M, out["CTRL-L"] = "magnet", "slopefollow"
    if K.clip or (B() and B().state and B().state.clip == "1") then out["CTRL-V"] = "paste" end
    if K.SelId() then
        out.DELETE, out["CTRL-C"], out["CTRL-D"] = "delete", "copy", "dup"
        out.F, out.L, out["SHIFT-L"] = "frame", "slope", "upright"
        -- the nudge keys, with Alt (fine) and Shift (big) as the same key (the action reads the modifier)
        for key in pairs(K.NUDGE_KEYS) do
            local act = "nudge_" .. key
            local ctrl = key:match("^CTRL%-(.+)$")
            out[key] = act
            if ctrl then
                out["ALT-CTRL-" .. ctrl], out["CTRL-SHIFT-" .. ctrl] = act, act
            else
                out["ALT-" .. key], out["SHIFT-" .. key] = act, act
            end
        end
    end
    if g.far then out.HOME, out.NUMPAD0 = "home", "home" end
    return out
end

function K.Bind()
    if not owner or not SetOverrideBindingClick or not ClearOverrideBindings then return false end
    if InCombatLockdown and InCombatLockdown() then K.pending = true; return false end
    K.pending = nil
    ClearOverrideBindings(owner)
    K.bound = {}
    for key, act in pairs(K.Wanted()) do
        local name = (act == "box" and "LeylineDenKeyBox") or (act == "circle" and "LeylineDenKeyCircle") or button(act)
        SetOverrideBindingClick(owner, true, key, name)
        K.bound[key] = act
    end
    return true
end

-- ---------------------------------------------------------------- the hint line (the status line's left part)

function K.Hint()
    local t = K.Tool()
    local head
    if t == "rotate" then
        head = "Turn (2): drag a prop left or right  ·  wheel turns the selection  ·  R step"
    elseif t == "scale" then
        head = "Size (3): drag a prop up or down  ·  wheel sizes the selection"
    else
        head = "Move (1): click a prop to carry it, drag to slide it  ·  Shift adds  ·  Ctrl+drag box, Alt+drag circle  ·  Q E turn"
    end
    local tail = "1 2 3 tools  ·  Del  ·  Ctrl+Z / Y  ·  Ctrl+C / V / D  ·  G grid  ·  F frame  ·  Numpad nudges"
    if K.scatter then tail = "scatter on  ·  " .. tail end
    if K.magnet then tail = "magnet on (M)  ·  " .. tail end
    if K.slope then tail = "on the slope (Ctrl+L)  ·  " .. tail end
    return head .. "  ·  " .. tail
end
-- The hover tip's second line: what the mouse does to the prop under it with the tool now.
function K.TipHint()
    local t = K.Tool()
    if t == "rotate" then return "drag left / right: turn  ·  wheel: turn the selection" end
    if t == "scale" then return "drag up / down: size  ·  wheel: size the selection" end
    local n = K.sel and K.sel.n or 0
    if n >= 2 then return "click: carry  ·  drag: slide all " .. n .. "  ·  Shift+click: add  ·  Q E: turn" end
    return "click: carry  ·  drag: slide  ·  Shift+click: add  ·  Q E: turn  ·  right-click: menu"
end
function K.SpeedText()
    return string.format("fly %d yd/s (right mouse + wheel)", K.Speed())
end

-- ---------------------------------------------------------------- the cursor (an editor's while building)

-- In build mode over the world: the casting cursor with something in hand, the hand over a prop; the game's own
-- otherwise, and always in play mode.
K.CURSOR_HAND, K.CURSOR_PLACE = "Interface\\Cursor\\Interact", "Interface\\Cursor\\Cast"
function K.CursorWant()
    local g = G()
    if not (g and g.build) then return nil end
    if IsMouselooking and IsMouselooking() then return nil end
    if g.OverWorld and not g.OverWorld() then return nil end
    if K.drag or g.dragging or g.armed then return K.CURSOR_PLACE end
    if g.hovered then return K.CURSOR_HAND end
    return nil
end
function K.CursorTick()
    local want = K.CursorWant()
    if want then
        if SetCursor then pcall(SetCursor, want) end
    elseif K.cursor and ResetCursor then
        pcall(ResetCursor)
    end
    K.cursor = want
end

-- ---------------------------------------------------------------- Den Lite's answers

function K.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= "wubuild" then return end
    local g = G()
    if name == "state" then
        local was = K.SelId()
        local s = K.ParseState(data)
        -- the copy Ctrl+D made (the answer comes at once; a selection later is the player's own)
        if K.dupWait and now() - (K.dupAt or 0) > 3 then K.dupWait = nil end
        if K.dupWait and s and s.id ~= K.dupWait then K.dupLast, K.dupWait = s.id, nil end
        if (was ~= nil) ~= (s ~= nil) then refresh() end
    elseif name == "snapped" then
        local was = K.snapped
        K.snapped = data == "1"
        -- said at once in the status line's left part (the build hint under it shows it too, once notes fade)
        if K.snapped and not was then note("Snapped edge to edge with its neighbour (Alt: free, M: magnet off)") end
        if g and g.ShowTag then g.ShowTag() end
    elseif name == "nudged" then
        local dx, dy, dz = data:match("^([%-%d%.]+),([%-%d%.]+),([%-%d%.]+)")
        if dx then note(string.format("Nudged %+.2f, %+.2f, %+.2f yd (Alt fine, Shift big)", tonumber(dx), tonumber(dy), tonumber(dz))) end
    elseif name == "cammark" then
        local n = K.markWait
        K.markWait = nil
        if n and data:match("^[%-%d%.,]+$") then
            local d = db()
            d.camMarks = type(d.camMarks) == "table" and d.camMarks or {}
            d.camMarks[n] = data
            note("View kept in " .. n .. ": Shift+" .. n .. " goes back to it")
        end
    elseif name == "placed" then
        -- a paste keeps the copied size; scatter sizes each prop at random (each one more undo step)
        if K.pasting and math.abs(K.pasting - 1) > 0.01 then cmd(string.format("setscale %.2f", K.pasting)) end
        if K.scatter then
            local lo, hi = K.SCATTER[1], K.SCATTER[2]
            local r = math.random and math.random() or 0.5
            cmd(string.format("grow %.3f", lo + (hi - lo) * r))
        end
    elseif name == "aim" then
        local kind = data:match("^(%a+)")
        if kind ~= "place" then K.pasting = nil end
        if kind == "off" then K.snapped = nil end
        -- scatter: each new card (and the next one after Shift+click) starts at a random turn
        if kind == "place" and K.scatter and g then K.ScatterTurn() end
    end
end

-- ---------------------------------------------------------------- set up

local function setup()
    if owner or not CreateFrame or not UIParent then return end
    owner = CreateFrame("Frame", "LeylineDenKeys", UIParent)
    local d = db()
    K.speedI = math.max(1, math.min(#K.FLY_SPEEDS, tonumber(d.flySpeed) or K.FLY_DEFAULT))
    K.scatter = d.scatter == true
    K.magnet = d.magnet == true
    K.slope = d.slope == true
    -- Ctrl+drag: the box's press and let-go
    local bb = CreateFrame("Button", "LeylineDenKeyBox", UIParent)
    if bb.RegisterForClicks then bb:RegisterForClicks("AnyDown", "AnyUp") end
    bb:SetScript("OnClick", function(_, _, down) if down == false then K.BoxUp() else K.BoxDown() end end)
    bb:Hide()
    -- Alt+drag: the circle's press and let-go, and the ring drawn while dragging (our own ring, Media\Icons\circle)
    local cb = CreateFrame("Button", "LeylineDenKeyCircle", UIParent)
    if cb.RegisterForClicks then cb:RegisterForClicks("AnyDown", "AnyUp") end
    cb:SetScript("OnClick", function(_, _, down) if down == false then K.CircleUp() else K.CircleDown() end end)
    cb:Hide()
    circleFrame = CreateFrame("Frame", "LeylineDenSelectCircle", UIParent)
    circleFrame:SetFrameStrata("TOOLTIP")
    if circleFrame.EnableMouse then circleFrame:EnableMouse(false) end
    if circleFrame.CreateTexture then
        local ring = circleFrame:CreateTexture(nil, "OVERLAY")
        if ring then
            ring:SetAllPoints(circleFrame)
            ring:SetTexture("Interface\\AddOns\\WrathUnleashedUI\\Media\\Icons\\circle")
            ring:SetVertexColor(0.95, 0.72, 0.25, 0.9)
        end
    end
    circleFrame:Hide()
    boxFrame = CreateFrame("Frame", "LeylineDenSelectBox", UIParent)
    boxFrame:SetFrameStrata("TOOLTIP")
    if boxFrame.EnableMouse then boxFrame:EnableMouse(false) end
    if boxFrame.CreateTexture then
        local fill = boxFrame:CreateTexture(nil, "BACKGROUND")
        if fill then fill:SetAllPoints(boxFrame); fill:SetTexture(0.95, 0.72, 0.25, 0.12) end
        for _, p in ipairs({ { "TOPLEFT", "TOPRIGHT", nil, 1 }, { "BOTTOMLEFT", "BOTTOMRIGHT", nil, 1 }, { "TOPLEFT", "BOTTOMLEFT", 1, nil }, { "TOPRIGHT", "BOTTOMRIGHT", 1, nil } }) do
            local t = boxFrame:CreateTexture(nil, "OVERLAY")
            if t then
                t:SetTexture(0.95, 0.72, 0.25, 0.9)
                t:SetPoint(p[1], boxFrame, p[1], 0, 0); t:SetPoint(p[2], boxFrame, p[2], 0, 0)
                if p[3] then t:SetWidth(p[3]) end
                if p[4] then t:SetHeight(p[4]) end
            end
        end
    end
    boxFrame:Hide()
    owner:SetScript("OnUpdate", function()
        if K.box then K.BoxUpdate() end
        if K.circle then K.CircleUpdate() end
        if K.drag then K.DragUpdate() end
        K.CursorTick()
    end)
    owner:SetScript("OnEvent", function(_, ev, ...)
        if ev == "CHAT_MSG_ADDON" then K.OnMessage(...)
        elseif ev == "PLAYER_REGEN_ENABLED" then
            -- keys asked for in combat (the game refuses them then): now
            local g = G()
            if g and g.bindPending and g.Bindings then g.Bindings() elseif K.pending then K.Bind() end
        end
    end)
    owner:RegisterEvent("CHAT_MSG_ADDON")
    owner:RegisterEvent("PLAYER_REGEN_ENABLED")
end
K.Setup = setup

if CreateFrame and UIParent then
    local ok = pcall(setup)
    if not ok then owner = nil end
end
