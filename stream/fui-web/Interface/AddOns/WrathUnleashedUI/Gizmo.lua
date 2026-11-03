-- Your Den's placement aids (Den Lite: labs/den-lite src/gizmo.rs, farsight.rs). Owner, 3 Nov: "it should function
-- like this ... until it works identically" (his own editor's video), "the 'snap out' camera mode working for sure too
-- and infinite camera zoom out from character".
--
-- The stock client has no cursor-to-world call, so:
--   * Build mode (in Your Den, not in play mode, on Den Lite: the Builder's state says lite=1): Den Lite shows the
--     props as objects the game highlights under the mouse; a right-click on one selects it (`wubuild picked`) and
--     this file opens a menu at the cursor. The camera is put in a known pose (the build camera: the player's own view
--     kept in view 5 and put back on leaving) and this file keeps count of its zoom, so Den Lite's ray stays true.
--   * A card picked in the gallery is in hand (`preview <id>`): while the cursor is over the world this file sends
--     where it is on the screen (`hover <u> <v> <aspect> <zoom>`, about 10 a second) and Den Lite shows the real prop
--     there, on the ground. One left click places it and ends placing: the new prop is selected at once (its glow,
--     the handles, the panel). Shift+click places it and keeps it in hand; a right click or Esc puts it away.
--   * The handles (red X, green Y, blue up): right-click one and the selection follows the mouse along that axis
--     ("Moved Cottage +4.45 yd"); a left click drops it (one undo), Esc or a right click puts it back; the wheel steps.
--   * The exact click: while placing, G (or this file's button) starts the ground-target cursor of the one spell Den
--     Lite taught (Flare): the game's own circle follows the mouse and the click lands exactly (Den Lite never casts
--     it). Each such click is also a calibration sample for the build camera (den-check "hover calibration").
--   * The camera: the wheel zooms as always (this file does it, to keep count); past the game's own limit Den Lite
--     carries on (`cam far`: the view goes to an invisible camera object of Den Lite's, further back each notch, and
--     back to you when you zoom in again). The snap-out key (J, or the first free of K, F9, F10) puts the view up and
--     back over where you stand, and back; while it is out the arrow keys pan it.
-- Nothing here takes WASD or the mouse's camera drags. The override bindings (Esc, G, the wheel, the snap key, the
-- arrows while snapped out) exist only in build mode and only while needed, and are cleared after.
-- Talks over OVMOD as the Builder does (`wubuild cmd ...`); answers come as
-- `wubuild aim|axis|picked|placed|drag|moved|cam|mode|ghost|state`.

local W = WrathUnleashedUI or {}
local G = { build = false, armed = nil, axis = nil, sel = nil, scale = 1, lastSend = 0, lastU = nil, lastV = nil, zoom = 12, far = nil }
W.Gizmo = G

G.PLACE_SPELL = 1543            -- gizmo.rs PLACE_SPELL
G.VIEW_KEEP = 5                 -- the player's own camera is kept in this saved view while building
G.VIEW_BUILD = 2                -- the build camera starts from this view (reset to the game's default first)
G.CAM_DIST = 12                 -- yards from the player, the distance Den Lite's ray uses (gizmo.rs Camera.dist)
G.HOVER_EVERY = 0.1             -- seconds between hovers (about 10 a second)
G.CLICK_TIME, G.CLICK_MOVE = 0.3, 6   -- a click, not a drag: shorter than this, moved less than this many pixels
G.AIM_KEY = "G"
G.ZOOM_STEP = 2                 -- yards a wheel notch zooms (this file zooms, so it knows the distance)
G.ZOOM_MIN = 2
G.ZOOM_HARD = 50                -- the game's own camera never goes further than this
G.SNAP_KEYS = { "J", "K", "F9", "F10" }   -- snap out: the first that nothing else uses
G.PAN = 4                       -- yards an arrow key pans the far view

local PREFIX, MOD = "OVMOD", "wubuild"

local function now() return GetTime and GetTime() or 0 end

local function send(text)
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, PREFIX, MOD .. "\tcmd\t1/1\t" .. text, "WHISPER", UnitName("player"))
    return true
end
G.Send = send

local function report(text)
    if not SendAddonMessage or not UnitName then return end
    pcall(SendAddonMessage, PREFIX, "wuden\treport\t1/1\t" .. text, "WHISPER", UnitName("player"))
end

-- Den Lite (its state says lite=1), in Your Den, not in play mode.
local function lite()
    local B = W.Builder
    return B and B.state and B.state.lite == "1" or false
end
function G.Wanted()
    local D = W.DenBar
    return (D and D.active and not D.play and lite()) and true or false
end

-- ---------------------------------------------------------------- the cursor on the screen

-- The cursor as Den Lite's ray wants it: u, v from -1 to 1 (right and up), and the screen's width over its height.
function G.Cursor()
    if not GetCursorPosition or not UIParent then return nil end
    local x, y = GetCursorPosition()
    local s = (UIParent.GetEffectiveScale and UIParent:GetEffectiveScale()) or 1
    local w, h = UIParent:GetWidth() or 0, UIParent:GetHeight() or 0
    if not x or w <= 0 or h <= 0 or s <= 0 then return nil end
    local u, v = 2 * (x / s) / w - 1, 2 * (y / s) / h - 1
    if u < -1 or u > 1 or v < -1 or v > 1 then return nil end
    return u, v, w / h
end

-- Over the 3D world, not over a window.
local function overWorld()
    if not GetMouseFocus or not WorldFrame then return true end
    local f = GetMouseFocus()
    return f == nil or f == WorldFrame
end

-- One hover when the cursor moved (at most every G.HOVER_EVERY seconds).
function G.Tick(force)
    if not G.armed or not overWorld() then return false end
    local t = now()
    if not force and t - G.lastSend < G.HOVER_EVERY then return false end
    local u, v, a = G.Cursor()
    if not u then return false end
    if not force and G.lastU and math.abs(u - G.lastU) < 0.002 and math.abs(v - G.lastV) < 0.002 then return false end
    G.lastSend, G.lastU, G.lastV = t, u, v
    return send(string.format("hover %.4f %.4f %.4f %d", u, v, a, G.zoom))
end

-- ---------------------------------------------------------------- the build camera

-- The game's own zoom limit (yards): its two CVars, at most G.ZOOM_HARD.
function G.ZoomMax()
    local mx = tonumber(GetCVar and GetCVar("cameraDistanceMax") or "") or 15
    local k = tonumber(GetCVar and GetCVar("cameraDistanceMaxFactor") or "") or 1
    return math.max(G.ZOOM_MIN, math.min(G.ZOOM_HARD, mx * k))
end

-- The build camera at the zoom kept count of: view 2 reset (behind, the game's own pitch), all the way in, out by the
-- zoom, and kept in view 2 (a camera drag puts it back: SetView 2).
function G.Pose()
    if ResetView then pcall(ResetView, G.VIEW_BUILD) end
    if SetView then pcall(SetView, G.VIEW_BUILD) end
    if CameraZoomIn then pcall(CameraZoomIn, 50) end
    if CameraZoomOut then pcall(CameraZoomOut, G.zoom) end
    if SaveView then pcall(SaveView, G.VIEW_BUILD) end
end

function G.CameraIn()
    if G.camera then return end
    G.camera = true
    if SaveView then pcall(SaveView, G.VIEW_KEEP) end
    -- as far out as the game goes (given back on leaving)
    if GetCVar and SetCVar then
        G.cvars = { max = GetCVar("cameraDistanceMax"), factor = GetCVar("cameraDistanceMaxFactor") }
        pcall(SetCVar, "cameraDistanceMax", tostring(G.ZOOM_HARD))
    end
    G.zoom = G.CAM_DIST
    G.Pose()
    send("cam zoom " .. G.zoom)
end

function G.CameraOut()
    if not G.camera then return end
    G.camera = nil
    if G.far then send("cam off"); G.far = nil end
    if G.cvars and SetCVar then
        if G.cvars.max then pcall(SetCVar, "cameraDistanceMax", G.cvars.max) end
        if G.cvars.factor then pcall(SetCVar, "cameraDistanceMaxFactor", G.cvars.factor) end
        G.cvars = nil
    end
    if SetView then pcall(SetView, G.VIEW_KEEP) end
end

-- The wheel in build mode: the game's zoom, kept count of; past its limit (and while the view is out) Den Lite's far
-- view, further each notch; zooming back in past it gives the view back. `out` > 0: further.
function G.Zoom(out)
    if out > 0 then
        if G.far or G.zoom + G.ZOOM_STEP > G.ZoomMax() + 0.01 then return send("cam far 1") end
        if CameraZoomOut then pcall(CameraZoomOut, G.ZOOM_STEP) end
        G.zoom = G.zoom + G.ZOOM_STEP
    else
        if G.far then return send("cam far -1") end
        if G.zoom - G.ZOOM_STEP < G.ZOOM_MIN then return false end
        if CameraZoomIn then pcall(CameraZoomIn, G.ZOOM_STEP) end
        G.zoom = G.zoom - G.ZOOM_STEP
    end
    if SaveView then pcall(SaveView, G.VIEW_BUILD) end
    return send("cam zoom " .. G.zoom)
end

-- Snap out: the view up and back over where you stand (Den Lite's camera object), and back.
function G.Snap()
    if not G.build then return false end
    return send("cam snap")
end
function G.Pan(dir)
    if not G.far then return false end
    return send("cam pan " .. dir .. " " .. G.PAN)
end

-- ---------------------------------------------------------------- the keys: only while needed

local owner, aimBtn, cancelBtn, wheelUp, wheelDown, snapBtn, tag, menu
local PAN_KEYS = { UP = "fwd", DOWN = "back", LEFT = "left", RIGHT = "right" }

-- The snap-out key: the first of G.SNAP_KEYS nothing else uses (the game's own bindings and the addon's keep theirs).
function G.SnapKey()
    for _, k in ipairs(G.SNAP_KEYS) do
        local ok, a = true, nil
        if GetBindingAction then ok, a = pcall(GetBindingAction, k) end
        if not ok or a == nil or a == "" then return k end
    end
    return nil
end

local function bindings()
    if not owner or not SetOverrideBindingClick or not ClearOverrideBindings then return end
    if InCombatLockdown and InCombatLockdown() then return end
    ClearOverrideBindings(owner)
    if G.armed or G.axis or (menu and menu:IsShown()) then
        SetOverrideBindingClick(owner, true, "ESCAPE", "LeylineGizmoCancel")
    end
    if (G.armed == "place" or G.armed == "move") and aimBtn then
        SetOverrideBindingClick(owner, true, G.AIM_KEY, "LeylineGizmoAim")
    end
    -- the wheel: in build mode the zoom (kept count of), or the picked axis
    if G.build or G.axis then
        SetOverrideBindingClick(owner, true, "MOUSEWHEELUP", "LeylineGizmoWheelUp")
        SetOverrideBindingClick(owner, true, "MOUSEWHEELDOWN", "LeylineGizmoWheelDown")
    end
    if G.build then
        G.snapKey = G.snapKey or G.SnapKey()
        if G.snapKey then SetOverrideBindingClick(owner, true, G.snapKey, "LeylineGizmoSnap") end
    end
    if G.far then
        for k in pairs(PAN_KEYS) do SetOverrideBindingClick(owner, true, k, "LeylineGizmoPan" .. k) end
    end
end
G.Bindings = bindings

local AXIS_TEXT = { x = "X (red)", y = "Y (green)", z = "Z (blue, up)" }
local function handName()
    local B = W.Builder
    return (B and B.inHand and B.inHand.name) or (B and B.pick and B.pick.name) or "It"
end
local function showTag()
    if not tag then return end
    local parts = {}
    if G.armed == "place" then
        parts[#parts + 1] = handName() .. " in hand: click the ground  ·  Shift+click places more  ·  Esc puts it away"
    elseif G.armed == "move" then
        parts[#parts + 1] = "Moving: click the ground  ·  " .. G.AIM_KEY .. ": exact spot  ·  Esc puts it back"
    elseif G.armed == "axis" then
        parts[#parts + 1] = "Dragging along " .. (AXIS_TEXT[G.axis] or "") .. ": move the mouse, click drops it  ·  wheel steps  ·  Esc puts it back"
    elseif G.axis then
        parts[#parts + 1] = "Axis " .. (AXIS_TEXT[G.axis] or G.axis) .. ": the mouse wheel moves it"
    end
    if G.far then
        parts[#parts + 1] = (G.far.snap and "Overview" or "Far view") .. ": " .. (G.snapKey and (G.snapKey .. " or ") or "") .. "zoom in to come back  ·  arrows pan"
    end
    if #parts == 0 then tag:Hide(); return end
    tag.fs:SetText(table.concat(parts, "   |   "))
    tag:Show()
end

function G.SetAxis(a)
    a = (a == "x" or a == "y" or a == "z") and a or nil
    if a ~= G.axis then
        G.axis = a
        send("axis " .. (a or "off"))
    end
    bindings(); showTag()
end

-- The wheel: dragging a handle, a step along it; an axis picked, the Builder's move; else the zoom (up: nearer).
function G.Wheel(sign)
    local B = W.Builder
    if G.armed == "axis" then
        local step = (B and B.STEPS and B.STEPS[B.step]) or 0.5
        return send("grab nudge " .. tostring(sign * step))
    end
    if G.axis then
        if B and B.Move then return B.Move(G.axis, sign) end
        return send(string.format("move %s %s", G.axis, tostring(0.25 * sign)))
    end
    if G.build then return G.Zoom(-sign) end
    return false
end

-- ---------------------------------------------------------------- arming

-- A gallery pick (Builder.lua B.Choose) in build mode: its copy follows the cursor.
function G.Arm(id)
    id = tonumber(id)
    if not id or not G.build then return false end
    G.lastU, G.lastV = nil, nil
    send("preview " .. id)
    G.armed = "place"            -- until Den Lite's aim says otherwise
    G.Tick(true)
    return true
end

function G.MoveSelected()
    if not G.build then return false end
    G.lastU, G.lastV = nil, nil
    send("preview move")
    G.armed = "move"
    G.Tick(true)
    return true
end

function G.Cancel()
    if menu and menu:IsShown() then menu:Hide() end
    if G.armed == "axis" then send("grab off") elseif G.armed then send("preview off") end
    G.armed = nil
    if G.axis then G.SetAxis(nil) end
    local B = W.Builder
    if B and B.HandDone then B.HandDone() end
    bindings(); showTag()
end

-- Grab a handle of the selection without the mouse (the den-test; a right-click on a handle does it in the game).
function G.Grab(axis)
    if not G.build or not (axis == "x" or axis == "y" or axis == "z") then return false end
    G.lastU, G.lastV = nil, nil
    return send("grab " .. axis)
end

-- ---------------------------------------------------------------- build mode

function G.SetBuild(on)
    on = on and true or false
    if on == G.build then return end
    G.build = on
    if on then
        send("mode build")
        send("gizmo on")
        G.CameraIn()
    else
        G.Cancel()
        send("gizmo off")
        send("mode play")
        G.CameraOut()
    end
    bindings(); showTag()
end

-- ---------------------------------------------------------------- clicks on the world

local down = {}
function G.MouseDown(button)
    local x, y = 0, 0
    if GetCursorPosition then x, y = GetCursorPosition() end
    down[button] = { t = now(), x = x or 0, y = y or 0 }
end

-- The mouse over a gameobject the game names (its tooltip, no unit): a left click there isn't "empty ground".
local function overObject()
    if not GameTooltip or not GameTooltip.IsShown or not GameTooltip:IsShown() then return false end
    return not (UnitExists and UnitExists("mouseover"))
end

-- A click (short, still) on the world: left places (armed) or clears the selection; right stops.
function G.MouseUp(button)
    local d = down[button]
    down[button] = nil
    if not G.build or not d then return false end
    local x, y = 0, 0
    if GetCursorPosition then x, y = GetCursorPosition() end
    local click = (now() - d.t) <= G.CLICK_TIME and math.abs((x or 0) - d.x) <= G.CLICK_MOVE and math.abs((y or 0) - d.y) <= G.CLICK_MOVE
    if not click then
        -- a camera drag while placing, dragging or snapped out: the build camera's pose again (the ray needs it)
        if (G.armed or G.far) and SetView then pcall(SetView, G.VIEW_BUILD) end
        return false
    end
    if button == "LeftButton" then
        if SpellIsTargeting and SpellIsTargeting() then return false end   -- the exact click: Den Lite gets the cast
        if G.armed then
            G.Tick(true)
            -- Shift+click: placed, and another in hand
            local keep = G.armed == "place" and IsShiftKeyDown and IsShiftKeyDown()
            return send(keep and "commit keep" or "commit")
        end
        if menu and menu:IsShown() then menu:Hide(); bindings(); return true end
        if not overObject() then return send("selclear") end
        return false
    elseif button == "RightButton" then
        if G.armed then G.Cancel(); return true end
    end
    return false
end

-- ---------------------------------------------------------------- the menu at the cursor

local MENU = {
    { "Delete", function() send("delete") end },
    { "Duplicate", function() send("dup") end },
    { "Move (follows the mouse)", function() G.MoveSelected() end },
    { "Rotate 15°", function() send("turn 15") end },
    { "Bigger", function() send(string.format("setscale %.2f", math.min(5, (G.scale or 1) * 1.25))) end },
    { "Smaller", function() send(string.format("setscale %.2f", math.max(0.1, (G.scale or 1) / 1.25))) end },
    { "Snap to floor", function() send("floor") end },
    { "Lock", function() send("lock") end },
    { "Copy", function() send("copy") end },
    { "Properties", function() local B = W.Builder; if B and B.ShowProps then B.ShowProps(true) end end },
}
G.MENU = MENU

local function buildMenu()
    if menu or not CreateFrame or not UIParent then return menu end
    menu = CreateFrame("Frame", "LeylineGizmoMenu", UIParent)
    menu:SetFrameStrata("DIALOG")
    menu:SetWidth(190); menu:SetHeight(#MENU * 22 + 12)
    if menu.SetBackdrop then
        menu:SetBackdrop({ bgFile = "Interface\\Tooltips\\UI-Tooltip-Background", edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border",
            tile = true, tileSize = 16, edgeSize = 14, insets = { left = 3, right = 3, top = 3, bottom = 3 } })
        if menu.SetBackdropColor then menu:SetBackdropColor(0.06, 0.07, 0.1, 0.95) end
    end
    menu.rows = {}
    for i, item in ipairs(MENU) do
        local b = CreateFrame("Button", "LeylineGizmoMenu" .. i, menu)
        b:SetWidth(178); b:SetHeight(20)
        b:SetPoint("TOPLEFT", menu, "TOPLEFT", 6, -6 - (i - 1) * 22)
        local fs = b.CreateFontString and b:CreateFontString(nil, "OVERLAY", "GameFontHighlight")
        if fs then fs:SetPoint("LEFT", b, "LEFT", 6, 0); fs:SetText(item[1]); b.label = fs end
        if b.SetHighlightTexture then b:SetHighlightTexture("Interface\\QuestFrame\\UI-QuestTitleHighlight", "ADD") end
        b:SetScript("OnClick", function() menu:Hide(); item[2](); bindings() end)
        menu.rows[i] = b
    end
    menu:Hide()
    return menu
end

-- Den Lite says a right-click selected this prop: the menu where the cursor is.
function G.Picked(id)
    G.sel = tonumber(id)
    local m = buildMenu()
    if not m then return end
    local x, y = 0, 0
    if GetCursorPosition then x, y = GetCursorPosition() end
    local s = (UIParent.GetEffectiveScale and UIParent:GetEffectiveScale()) or 1
    m:ClearAllPoints()
    m:SetPoint("TOPLEFT", UIParent, "BOTTOMLEFT", (x or 0) / s + 8, (y or 0) / s - 8)
    m:Show()
    bindings()
end

-- ---------------------------------------------------------------- Den Lite's answers

function G.OnMessage(prefix, msg, channel, sender)
    if prefix ~= PREFIX or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, i, n, data = msg:match("^([^\t]+)\t([^\t]+)\t(%d+)/(%d+)\t(.*)$")
    if mod ~= MOD or i ~= "1" or n ~= "1" then return end
    if name == "aim" then
        local kind, rest = data:match("^(%a+),?(.*)$")
        G.armed = (kind == "place" or kind == "move" or kind == "axis") and kind or nil
        if kind == "axis" then G.axis = rest:match(",(%a)$") or G.axis end
        if not G.armed then
            local B = W.Builder
            if B and B.HandDone then B.HandDone() end
        end
        bindings(); showTag()
    elseif name == "axis" then
        G.axis = (data == "x" or data == "y" or data == "z") and data or nil
        bindings(); showTag()
    elseif name == "picked" then
        G.Picked(data)
    elseif name == "placed" then
        -- placed: the new prop is the selection (Den Lite's state follows and opens the panel)
        G.sel = tonumber(data)
        local B = W.Builder
        if B and B.HandDone then B.HandDone() end
    elseif name == "drag" or name == "moved" then
        local axis, d = data:match("^(%a),([%+%-][%d%.]+)$")
        local B = W.Builder
        local what = (B and B.state and B.state.selName) or "it"
        G.lastMove = d and string.format("%s %s %s yd along %s", name == "drag" and "Moving" or "Moved", what, d, (axis or ""):upper()) or nil
        if G.lastMove and B and B.PillNote then B.PillNote(G.lastMove) end
    elseif name == "cam" then
        local back, snap = data:match("^far,([%d%.]+),(%d)")
        local was = G.far
        G.far = back and { back = tonumber(back), snap = snap == "1" } or nil
        -- the view moved to Den Lite's camera object (or back to you): the build camera's pose again round it
        if (was == nil) ~= (G.far == nil) then G.Pose() end
        bindings(); showTag()
    elseif name == "ghost" then
        G.ghost = data
    elseif name == "state" then
        local sc = data:match("pos=[^;]-,([%d%.]+);")
        G.scale = tonumber(sc) or G.scale
        if data:find("sel=-;", 1, true) then G.sel = nil end
    end
end

-- ---------------------------------------------------------------- the den-test (DenBar.lua D.TestAction "gizmo ...")

function G.TestAction(w)
    local a = w[2] or ""
    if a == "build" then
        local on = w[3] ~= "off"
        -- already so: say the mode again (Den Lite answers the look each time; the den-test reads that)
        if on == G.build then send(on and "mode build" or "mode play") else G.SetBuild(on) end
        return true
    end
    if a == "arm" then return G.Arm(w[3]) end
    if a == "move" then return G.MoveSelected() end
    if a == "hover" then
        -- the test's own point on the screen (u v), as if the cursor were there
        local u, v = tonumber(w[3]), tonumber(w[4])
        if not u or not v then return false end
        local _, _, asp = G.Cursor()
        return send(string.format("hover %.4f %.4f %.4f %d", u, v, asp or (16 / 9), G.CAM_DIST))
    end
    if a == "tick" then return G.Tick(true) end
    if a == "commit" then return send(w[3] == "keep" and "commit keep" or "commit") end
    if a == "grab" then return G.Grab(w[3]) end
    if a == "snap" then return G.Snap() end
    if a == "zoom" then local n = tonumber(w[3]) or 1; for _ = 1, math.min(40, math.abs(n)) do G.Zoom(n > 0 and 1 or -1) end; return true end
    if a == "pan" then return G.Pan(w[3] or "fwd") end
    if a == "cancel" then G.Cancel(); return true end
    if a == "axis" then G.SetAxis(w[3]); return true end
    if a == "wheel" then return G.Wheel(tonumber(w[3]) or 1) end
    if a == "menu" then
        local i = tonumber(w[3])
        local m = buildMenu()
        if i and MENU[i] then MENU[i][2](); if m then m:Hide() end; return true end
        return false
    end
    if a == "report" then
        local u, v, asp = G.Cursor()
        report(string.format("gizmo build=%d armed=%s axis=%s sel=%s camera=%d menu=%d cursor=%s,%s,%s ghost=%s zoom=%d far=%s snapkey=%s moved=%s",
            G.build and 1 or 0, G.armed or "-", G.axis or "-", tostring(G.sel or "-"), G.camera and 1 or 0,
            (menu and menu:IsShown()) and 1 or 0, u and string.format("%.3f", u) or "-", v and string.format("%.3f", v) or "-",
            asp and string.format("%.3f", asp) or "-", G.ghost or "-", G.zoom or 0,
            G.far and string.format("%.1f%s", G.far.back, G.far.snap and "s" or "") or "-", G.snapKey or "-", (G.lastMove or "-"):gsub(" ", "_")))
        return true
    end
    return false
end

-- ---------------------------------------------------------------- set up

local function setup()
    if owner or not CreateFrame or not UIParent then return end
    owner = CreateFrame("Frame", "LeylineGizmo", UIParent)
    -- the exact click: a secure button that starts the spell's ground-target cursor (a click or the G key)
    local name = GetSpellInfo and GetSpellInfo(G.PLACE_SPELL)
    if name then
        local ok, b = pcall(CreateFrame, "Button", "LeylineGizmoAim", UIParent, "SecureActionButtonTemplate")
        if ok and b then
            aimBtn = b
            b:SetAttribute("type", "spell")
            b:SetAttribute("spell", name)
            -- shown (a hidden button may not take its binding's click) but 1 x 1, clear and out of the way
            b:SetWidth(1); b:SetHeight(1); b:SetAlpha(0)
            b:SetPoint("BOTTOMLEFT", UIParent, "BOTTOMLEFT", 0, 0)
        end
    end
    cancelBtn = CreateFrame("Button", "LeylineGizmoCancel", UIParent)
    cancelBtn:SetScript("OnClick", function() G.Cancel() end)
    cancelBtn:Hide()
    wheelUp = CreateFrame("Button", "LeylineGizmoWheelUp", UIParent)
    wheelUp:SetScript("OnClick", function() G.Wheel(1) end)
    wheelUp:Hide()
    wheelDown = CreateFrame("Button", "LeylineGizmoWheelDown", UIParent)
    wheelDown:SetScript("OnClick", function() G.Wheel(-1) end)
    wheelDown:Hide()
    snapBtn = CreateFrame("Button", "LeylineGizmoSnap", UIParent)
    snapBtn:SetScript("OnClick", function() G.Snap() end)
    snapBtn:Hide()
    for k, dir in pairs(PAN_KEYS) do
        local b = CreateFrame("Button", "LeylineGizmoPan" .. k, UIParent)
        b:SetScript("OnClick", function() G.Pan(dir) end)
        b:Hide()
    end
    -- the tag above the status pill: what is armed, which axis
    tag = CreateFrame("Frame", "LeylineGizmoTag", UIParent)
    tag:SetWidth(520); tag:SetHeight(22)
    local pill = _G and _G.LeylineEditorStatus
    if pill then tag:SetPoint("BOTTOM", pill, "TOP", 0, 6) else tag:SetPoint("BOTTOM", UIParent, "BOTTOM", 0, 150) end
    tag.fs = tag.CreateFontString and tag:CreateFontString(nil, "OVERLAY", "GameFontHighlight") or { SetText = function() end }
    if tag.fs.SetPoint then tag.fs:SetPoint("CENTER", tag, "CENTER", 0, 0) end
    tag:Hide()
    -- the clicks on the world (the hook never takes a click from the game: camera drags and movement go on)
    if WorldFrame and WorldFrame.HookScript then
        pcall(WorldFrame.HookScript, WorldFrame, "OnMouseDown", function(_, b) G.MouseDown(b) end)
        pcall(WorldFrame.HookScript, WorldFrame, "OnMouseUp", function(_, b) G.MouseUp(b) end)
    end
    -- the Builder's gallery pick arms the preview in build mode
    if hooksecurefunc and W.Builder and W.Builder.Choose then
        pcall(hooksecurefunc, W.Builder, "Choose", function(item) if item and not item.kind then G.Arm(item.id) end end)
    end
    local acc = 0
    owner:SetScript("OnUpdate", function(_, el)
        acc = acc + (el or 0)
        if acc >= 0.5 then
            acc = 0
            G.SetBuild(G.Wanted())
        end
        if G.armed then G.Tick(false) end
    end)
    owner:SetScript("OnEvent", function(_, ev, ...)
        if ev == "CHAT_MSG_ADDON" then G.OnMessage(...) end
    end)
    owner:RegisterEvent("CHAT_MSG_ADDON")
end
G.Setup = setup

if CreateFrame and UIParent then
    local ok = pcall(setup)
    if not ok then owner = nil end
end
