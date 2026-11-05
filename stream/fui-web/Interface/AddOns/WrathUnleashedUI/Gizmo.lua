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
--   * The mouse, picked by Den Lite (owner 3 Nov: "clicking objects, having them highlighted, moving them around
--     easily"; Den Lite's caps say `click`): the game never tells the realm about a left click on an object, so in
--     build mode this file sends the cursor's place even with nothing in hand and Den Lite casts the ray itself
--     (labs/den-lite src/pick.rs). The prop under the cursor glows and its name shows by the cursor (`hovered`). A
--     short, still left click (under 0.3 s, under 6 px) selects it (`click`), or clears the selection on empty
--     ground; Shift+click adds it. Pressing on a prop and moving more than 6 px drags it (`drag`, then the cursor's
--     place; the spot you grabbed stays under the cursor); letting go drops it (`commit`, one undo); Esc puts it
--     back; the wheel or Q / E turn it.
--   * The build view: entering build mode puts the view up and back over you (`cam build`: Den Lite's camera object,
--     a pose it knows exactly, so the ray is true), with a BUILD frame round the screen and the cursor always free;
--     leaving gives your own camera back. WASD and the arrows pan the view while it is out; the wheel zooms.
-- How the mouse is shared with the camera (3.3.5a): WorldFrame's OnMouseDown / OnMouseUp hooks see every press on the
-- world but can't stop the game's own left-drag (CAMERAORSELECTORMOVE turns the camera; CameraOrSelectOrMoveStart /
-- Stop are protected). So while the cursor is over a prop (Den Lite said `hovered`), or a prop is being dragged,
-- BUTTON1 is override-bound to this file's own button (LeylineGizmoGrab, down and up): the press is ours and the
-- camera stays. Over empty ground BUTTON1 is the game's again (left-drag turns the camera, right-drag as always). The
-- binding never changes while a button is held, and a lost mouse-up is caught by IsMouseButtonDown.
-- The override bindings (Esc, G, the wheel, the snap key, BUTTON1 over a prop, Q / E with a selection, WASD and the
-- arrows while the view is out) exist only in build mode and only while needed, and are cleared after.
-- Talks over OVMOD as the Builder does (`wubuild cmd ...`); answers come as
-- `wubuild aim|axis|picked|placed|drag|moved|cam|mode|ghost|state|hovered|selected|grabbed`.

local W = WrathUnleashedUI or {}
local G = { build = false, armed = nil, axis = nil, sel = nil, scale = 1, lastSend = 0, lastU = nil, lastV = nil, zoom = 12, far = nil }
W.Gizmo = G

G.PLACE_SPELL = 1543            -- gizmo.rs PLACE_SPELL
G.VIEW_KEEP = 5                 -- the player's own camera is kept in this saved view while building
G.VIEW_BUILD = 2                -- the build camera starts from this view (reset to the game's default first)
G.CAM_DIST = 12                 -- yards from the player, the distance Den Lite's ray uses (gizmo.rs Camera.dist)
G.HOVER_EVERY = 0.1             -- seconds between hovers (about 10 a second)
G.HOVER_FAST = 0.03             -- while carrying or dragging: about 30 a second, so the prop keeps up (owner 4 Nov)
G.CLICK_TIME, G.CLICK_MOVE = 0.3, 6   -- a click, not a drag: shorter than this, moved less than this many pixels
G.AIM_KEY = "G"
G.ZOOM_STEP = 2                 -- yards a wheel notch zooms (this file zooms, so it knows the distance)
G.ZOOM_MIN = 2
G.ZOOM_HARD = 50                -- the game's own camera never goes further than this
G.SNAP_KEYS = { "J", "K", "F9", "F10" }   -- snap out: the first that nothing else uses
G.PAN = 4                       -- yards an arrow key pans the far view
G.PAN_EVERY = 0.2               -- seconds between pans while a pan key is held
G.SETTLE = 0.15                 -- the cursor came to rest: its place once more after this (Den Lite's glow keeps up)
G.TURN = 15                     -- degrees Q / E (and the wheel while dragging) turn the prop

local PREFIX, MOD = "OVMOD", "wubuild"

local function now() return GetTime and GetTime() or 0 end

local sent = { n = 0, t = 0, rate = 0 }
local function send(text)
    if not SendAddonMessage or not UnitName then return false end
    sent.n = sent.n + 1
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
-- Den Lite's state not here yet (it comes with the gallery's catalog, which may not have been asked for): unknown,
-- which is not "no" (4 Nov: den-move's build mode was switched off again half a second after it went on)
local function stateKnown()
    local B = W.Builder
    return B and B.state and B.state.lite ~= nil or false
end
function G.Wanted()
    local D = W.DenBar
    if not (D and D.active and not D.play) then return false end
    if lite() then return true end
    if not stateKnown() then return G.build and true or false end
    return false
end

-- Ask Den Lite for its state (the Builder's `state` message: the answer fills the Builder's state, `lite=1` with it).
function G.AskState()
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, PREFIX, MOD .. "\tstate\t1/1\t", "WHISPER", UnitName("player"))
    return true
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

-- Den Lite picks with the mouse itself (its caps say `click`): a Den Lite from before only knew right-clicks.
function G.Picks()
    local B = W.Builder
    if B and B.Can then return B.Can("click") and true or false end
    return (B and B.state and B.state.caps and B.state.caps:find("click", 1, true)) and true or false
end

-- One hover when the cursor moved (at most every G.HOVER_EVERY seconds): while something is in hand or dragged, or
-- in build mode with nothing in hand (Den Lite picks: the prop under the cursor glows). The cursor at rest: its place
-- once more after G.SETTLE (Den Lite changes the glow at most every 100 ms, so the last change is never lost).
function G.Tick(force)
    if not (G.armed or G.dragging or (G.build and G.Picks())) or not overWorld() then return false end
    local t = now()
    local every = (G.armed or G.dragging) and G.HOVER_FAST or G.HOVER_EVERY
    if not force and t - G.lastSend < every then return false end
    local u, v, a = G.Cursor()
    if not u then return false end
    if not force and G.lastU and math.abs(u - G.lastU) < 0.002 and math.abs(v - G.lastV) < 0.002 then
        if G.armed or G.dragging or G.settled or t - G.lastSend < G.SETTLE then return false end
        G.settled = true
    else
        G.settled = false
    end
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
    -- the cursor always free in build mode
    if IsMouselooking and IsMouselooking() and MouselookStop then pcall(MouselookStop) end
    if SaveView then pcall(SaveView, G.VIEW_KEEP) end
    -- as far out as the game goes (given back on leaving)
    if GetCVar and SetCVar then
        G.cvars = { max = GetCVar("cameraDistanceMax"), factor = GetCVar("cameraDistanceMaxFactor") }
        pcall(SetCVar, "cameraDistanceMax", tostring(G.ZOOM_HARD))
    end
    G.zoom = G.CAM_DIST
    G.Pose()
    send("cam zoom " .. G.zoom)
    -- the editor's own view: up and back over you, a pose Den Lite knows (a Den Lite from before: the build camera)
    if G.Picks() then send("cam build"); G.buildView = true end
end

function G.CameraOut()
    if not G.camera then return end
    G.camera = nil
    if G.far or G.buildView then send("cam off"); G.far = nil end
    G.buildView = nil
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

local owner, aimBtn, cancelBtn, wheelUp, wheelDown, snapBtn, tag, menu, grabBtn, tip, frame
local PAN_KEYS = { UP = "fwd", DOWN = "back", LEFT = "left", RIGHT = "right", W = "fwd", S = "back", A = "left", D = "right" }
local TURN_KEYS = { Q = 1, E = -1 }
local down = {}

-- The snap-out key: the first of G.SNAP_KEYS nothing else uses (the game's own bindings and the addon's keep theirs).
function G.SnapKey()
    for _, k in ipairs(G.SNAP_KEYS) do
        local ok, a = true, nil
        if GetBindingAction then ok, a = pcall(GetBindingAction, k) end
        if not ok or a == nil or a == "" then return k end
    end
    return nil
end

-- BUTTON1 is ours while the cursor is over a prop or one is dragged (never changed while the button is held).
function G.WantsButton1()
    if down.LeftButton then return G.b1 and true or false end
    return (G.build and G.Picks() and not G.armed and (G.hovered ~= nil or G.dragging)) and true or false
end

local function bindings()
    if not owner or not SetOverrideBindingClick or not ClearOverrideBindings then return end
    if InCombatLockdown and InCombatLockdown() then return end
    ClearOverrideBindings(owner)
    G.b1 = G.WantsButton1()
    if G.b1 and grabBtn then SetOverrideBindingClick(owner, true, "BUTTON1", "LeylineGizmoGrab") end
    if G.build and (G.sel or G.armed == "move" or G.armed == "place" or G.dragging) then
        for k in pairs(TURN_KEYS) do SetOverrideBindingClick(owner, true, k, "LeylineGizmoTurn" .. k) end
    end
    if G.armed or G.axis or G.dragging or (menu and menu:IsShown()) then
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
    if G.far and G.build then
        for k in pairs(PAN_KEYS) do SetOverrideBindingClick(owner, true, k, "LeylineGizmoPan" .. k) end
    end
end
G.Bindings = bindings

local AXIS_TEXT = { x = "X (red)", y = "Y (green)", z = "Z (blue, up)" }
local function handName()
    local B = W.Builder
    return (B and B.inHand and B.inHand.name) or (B and B.pick and B.pick.name) or "It"
end
-- The hint line in build mode with nothing in hand.
function G.Hint()
    local D = W.DenBar
    local key = (D and D.keys and D.keys.build) or "F7"
    if not G.Picks() then return "Right-click a prop to select it  ·  " .. key .. " to play" end
    return "Click a prop to select  ·  drag to move  ·  Shift adds  ·  Q E turn  ·  Del deletes  ·  " .. key .. " leaves Build"
end

local function showTag()
    if not tag then return end
    local parts = {}
    if G.dragging then
        parts[#parts + 1] = "Moving: let go to drop it  ·  wheel or Q E turns it  ·  Esc puts it back"
    elseif G.armed == "place" then
        parts[#parts + 1] = handName() .. " in hand: click the ground  ·  Shift+click places more  ·  Esc puts it away"
    elseif G.armed == "move" then
        parts[#parts + 1] = "Moving: click the ground  ·  " .. G.AIM_KEY .. ": exact spot  ·  Esc puts it back"
    elseif G.armed == "axis" then
        parts[#parts + 1] = "Dragging along " .. (AXIS_TEXT[G.axis] or "") .. ": move the mouse, click drops it  ·  wheel steps  ·  Esc puts it back"
    elseif G.axis then
        parts[#parts + 1] = "Axis " .. (AXIS_TEXT[G.axis] or G.axis) .. ": the mouse wheel moves it"
    end
    if #parts == 0 and G.build then parts[#parts + 1] = G.Hint() end
    if G.far then
        if G.far.snap then
            parts[#parts + 1] = "Overview: " .. (G.snapKey and (G.snapKey .. " or ") or "") .. "zoom in to come back  ·  WASD pans"
        else
            parts[#parts + 1] = "WASD pans  ·  wheel zooms"
        end
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
G.ShowTag = showTag

-- Q / E: turn what is held (in hand, dragged) or the selection, by G.TURN degrees (Q: left).
function G.Turn(sign)
    if not G.build then return false end
    local deg = sign * G.TURN
    if G.dragging or G.armed == "move" or G.armed == "place" then return send("preview turn " .. deg) end
    if G.sel then return send("turn " .. deg) end
    return false
end

-- The wheel: dragging a prop, it turns; dragging a handle, a step along it; an axis picked, the Builder's move; else
-- the zoom (up: nearer).
function G.Wheel(sign)
    local B = W.Builder
    if G.dragging then return G.Turn(sign) end
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
    if G.armed == "axis" then send("grab off") elseif G.armed or G.dragging then send("preview off") end
    G.armed = nil
    G.dragging = nil
    -- the press is spent: its let-go starts nothing
    if down.LeftButton then down.LeftButton.prop = nil end
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
        G.hovered = nil
    end
    if frame then if on then frame:Show() else frame:Hide() end end
    if tip and not on then tip:Hide() end
    bindings(); showTag()
end

-- ---------------------------------------------------------------- the mouse on the world

-- Where the cursor is now: the pixels (for the click's 6 px) and Den Lite's u, v, aspect.
local function at()
    local x, y = 0, 0
    if GetCursorPosition then x, y = GetCursorPosition() end
    local u, v, a = G.Cursor()
    return x or 0, y or 0, u, v, a
end

-- The mouse over a gameobject the game names (its tooltip, no unit): a left click there isn't "empty ground" (a Den
-- Lite from before picking only).
function G.OverObject()
    if not GameTooltip or not GameTooltip.IsShown or not GameTooltip:IsShown() then return false end
    return not (UnitExists and UnitExists("mouseover"))
end

-- A button pressed on the world (WorldFrame's hook, or BUTTON1's own binding over a prop; the second of the two is
-- left out). A left press over a prop may become a drag.
function G.MouseDown(button)
    if down[button] then return false end
    local x, y, u, v, a = at()
    down[button] = { t = now(), x = x, y = y, u = u, v = v, a = a, prop = G.hovered and G.hovered.id or nil }
    return true
end

-- Held and moved more than G.CLICK_MOVE pixels from a press on a prop: Den Lite grabs what is under where it was
-- pressed (the grabbed spot stays under the cursor), then follows the cursor (G.Tick).
function G.DragCheck()
    local d = down.LeftButton
    if not d or G.dragging or G.armed or not G.build or not d.prop or not d.u or not G.Picks() then return false end
    local x, y = at()
    if math.abs(x - d.x) <= G.CLICK_MOVE and math.abs(y - d.y) <= G.CLICK_MOVE then return false end
    G.dragging = true
    if tip then tip:Hide() end
    send(string.format("drag %.4f %.4f %.4f %d", d.u, d.v, d.a or (16 / 9), G.zoom))
    G.lastU, G.lastV = nil, nil
    G.Tick(true)
    bindings(); showTag()
    return true
end

-- A button let go: a drag drops the prop (one undo); a click (short, still) on the world: left places what is in
-- hand, or selects what is under the cursor (Shift adds; empty ground clears); right stops placing.
function G.MouseUp(button)
    -- a quick flick (pressed, moved and let go within one frame) is a drag too
    if button == "LeftButton" and not G.dragging then G.DragCheck() end
    local d = down[button]
    down[button] = nil
    if not G.build or not d then bindings(); return false end
    if button == "LeftButton" and G.dragging then
        G.Tick(true)
        G.dragging = nil
        send("commit")
        bindings(); showTag()
        return true
    end
    local x, y = at()
    local click = (now() - d.t) <= G.CLICK_TIME and math.abs(x - d.x) <= G.CLICK_MOVE and math.abs(y - d.y) <= G.CLICK_MOVE
    if not click then
        -- a camera drag while placing, dragging or the view is out: the build camera's pose again (the ray needs it)
        if (G.armed or G.far) and SetView then pcall(SetView, G.VIEW_BUILD) end
        bindings()
        return false
    end
    local done = false
    if button == "LeftButton" then
        if SpellIsTargeting and SpellIsTargeting() then return false end   -- the exact click: Den Lite gets the cast
        local shift = IsShiftKeyDown and IsShiftKeyDown()
        if G.armed then
            G.Tick(true)
            -- Shift+click: placed, and another in hand
            local keep = G.armed == "place" and shift
            done = send(keep and "commit keep" or "commit")
        elseif menu and menu:IsShown() then
            menu:Hide(); done = true
        elseif G.Picks() and d.u then
            -- a click picks the prop up (owner 4 Nov): it follows the cursor over the ground, the next click puts it
            -- down; a click on an arrow grabs that axis; Shift adds to the selection instead (a Den Lite from before
            -- carrying reads the word as a plain select)
            done = send(string.format("click %.4f %.4f %.4f %d%s", d.u, d.v, d.a or (16 / 9), G.zoom, shift and " add" or " carry"))
        elseif not G.OverObject() then
            done = send("selclear")
        end
    elseif button == "RightButton" then
        if G.armed then G.Cancel(); done = true end
    end
    bindings()
    return done
end

-- A mouse-up the game kept from us (let go over a window, or a binding's up never came): the button is up, so it is
-- let go here.
function G.UpCheck()
    if not IsMouseButtonDown then return false end
    for button in pairs(down) do
        local ok, held = pcall(IsMouseButtonDown, button)
        if ok and not held then G.MouseUp(button); return true end
    end
    return false
end

-- The name of the prop under the cursor, by the cursor (not while dragging, placing or over a window).
function G.TipUpdate()
    if not tip then return end
    if not (G.build and G.hovered and G.hovered.name ~= "" and not G.dragging and not G.armed and overWorld()) then tip:Hide(); return end
    local x, y = 0, 0
    if GetCursorPosition then x, y = GetCursorPosition() end
    local s = (UIParent.GetEffectiveScale and UIParent:GetEffectiveScale()) or 1
    tip:ClearAllPoints()
    tip:SetPoint("BOTTOMLEFT", UIParent, "BOTTOMLEFT", (x or 0) / s + 16, (y or 0) / s + 8)
    tip.fs:SetText(G.hovered.name)
    tip:Show()
end

-- Pan keys held: the view keeps panning (one step at once, then every G.PAN_EVERY while held).
local held = {}
function G.PanKey(dir, isDown)
    if isDown == false then held[dir] = nil; return false end
    held[dir] = now()
    return G.Pan(dir)
end
function G.PanHeld()
    if not G.far then for k in pairs(held) do held[k] = nil end; return false end
    local t, sent = now(), false
    for dir, last in pairs(held) do
        if t - last >= G.PAN_EVERY then held[dir] = t; sent = G.Pan(dir) or sent end
    end
    return sent
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
        if not G.armed then G.carried = nil end
        -- something in hand: the prop that was under the cursor is the one in hand now (Den Lite stops its hover
        -- without a `hovered -`), so the hover is let go here: no stale name by the cursor, BUTTON1 back to the
        -- camera once it is put down (review 4 Nov)
        if G.armed then G.hovered = nil; if tip then tip:Hide() end end
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
        -- Shift+right-click: the one selected before stays selected with it
        local prev = G.sel
        G.Picked(data)
        if prev and prev ~= G.sel and IsShiftKeyDown and IsShiftKeyDown() then send("seladd " .. prev) end
    elseif name == "placed" then
        -- placed: the new prop is the selection (Den Lite's state follows and fills the panel, which stays open)
        G.sel = tonumber(data)
        local B = W.Builder
        if B and B.HandDone then B.HandDone(true) end
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
    elseif name == "hovered" then
        -- `<id> <name>` or `-`: the prop under the cursor (it glows); BUTTON1 is ours while there is one
        local id, nm = data:match("^(%d+)%s*(.*)$")
        G.hovered = id and { id = tonumber(id), name = nm or "" } or nil
        G.TipUpdate()
        bindings()
    elseif name == "selected" then
        -- a left click selected this prop (or `-`: cleared); the Builder's state fills the panel
        G.sel = tonumber(data)
        bindings(); showTag()
    elseif name == "grabbed" then
        -- the drag's press found nothing to grab (or a locked prop): the drag is over
        if data == "-" then
            G.dragging = nil
            if down.LeftButton then down.LeftButton.prop = nil end
            bindings(); showTag()
        end
    elseif name == "carried" then
        -- picked up by a click (`<id>`, or `axis x|y|z` for an arrow): it follows the cursor; a click puts it down
        G.carried = data
        -- the carried prop is the selection (Den Lite selected it): Q / E turn it after it is put down
        if tonumber(data) then G.sel = tonumber(data) end
        local B = W.Builder
        if B and B.PillNote then
            B.PillNote(data:find("^axis") and ("Moving along " .. data:sub(6):upper() .. ": click to drop it, Esc puts it back")
                or "Carrying: click to put it down, Esc puts it back")
        end
        bindings(); showTag()
    elseif name == "debug" then
        G.dbgLine = data
        G.DebugDraw()
    elseif name == "ghost" then
        G.ghost = data
    elseif name == "state" then
        local sc = data:match("pos=[^;]-,([%d%.]+);")
        G.scale = tonumber(sc) or G.scale
        if data:find("sel=-;", 1, true) then G.sel = nil end
    end
end

-- ---------------------------------------------------------------- the debug overlay (/wudebug; owner 4 Nov: "with debug ability")
-- Top left: what this file thinks (build mode, armed, carried, hovered, the selection, BUTTON1 ours, the cursor,
-- hovers sent a second) and what Den Lite sees (its `debug` line: armed, where the copy is, the cursor's ray and
-- where it meets the ground, the ground's height, the handle under the cursor, the redraw pace). Asked twice a second
-- while shown. Nothing here changes the editor.
local dbg
function G.DebugDraw()
    if not dbg or not dbg:IsShown() then return end
    local u, v = G.Cursor()
    local lines = {
        string.format("|cffffd27fLeyline build debug|r  (/wudebug to hide)"),
        string.format("build=%s armed=%s carried=%s axis=%s sel=%s", G.build and "on" or "off", G.armed or "-", G.carried or "-",
            G.axis or "-", tostring(G.sel or "-")),
        string.format("hovered=%s button1=%s dragging=%s zoom=%s far=%s", G.hovered and (tostring(G.hovered.id) .. " " .. (G.hovered.name or "")) or "-",
            G.b1 and "ours" or "game", G.dragging and "yes" or "no", tostring(G.zoom), G.far and string.format("%.0f", G.far.back) or "-"),
        string.format("cursor=%s,%s  sent=%d/s  hover every %.0f ms", u and string.format("%.3f", u) or "-", v and string.format("%.3f", v) or "-",
            sent.rate, ((G.armed or G.dragging) and G.HOVER_FAST or G.HOVER_EVERY) * 1000),
        "|cff9fd0ffDen Lite|r",
    }
    for part in (G.dbgLine or "no answer yet: is Den Lite from 4 Nov or later?"):gmatch("[^;]+") do lines[#lines + 1] = part end
    dbg.fs:SetText(table.concat(lines, "\n"))
end

function G.Debug(on)
    if not dbg and CreateFrame and UIParent then
        dbg = CreateFrame("Frame", "LeylineGizmoDebug", UIParent)
        dbg:SetFrameStrata("TOOLTIP")
        dbg:SetWidth(460); dbg:SetHeight(300)
        dbg:SetPoint("TOPLEFT", UIParent, "TOPLEFT", 12, -120)
        if dbg.SetBackdrop then
            dbg:SetBackdrop({ bgFile = "Interface\\Tooltips\\UI-Tooltip-Background", tile = true, tileSize = 16 })
            if dbg.SetBackdropColor then dbg:SetBackdropColor(0, 0, 0, 0.7) end
        end
        if dbg.EnableMouse then dbg:EnableMouse(false) end
        dbg.fs = dbg.CreateFontString and dbg:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall") or { SetText = function() end }
        if dbg.fs.SetPoint then dbg.fs:SetPoint("TOPLEFT", dbg, "TOPLEFT", 8, -8) end
        if dbg.fs.SetJustifyH then dbg.fs:SetJustifyH("LEFT") end
        dbg:Hide()
    end
    if not dbg then return false end
    if on == nil then on = not dbg:IsShown() end
    if on then dbg:Show(); send("debug") else dbg:Hide() end
    G.DebugDraw()
    return on
end

-- each half second while shown: Den Lite's view again, and the send rate
function G.DebugTick(el)
    sent.t = sent.t + (el or 0)
    if sent.t < 0.5 then return end
    sent.rate = math.floor(sent.n / sent.t + 0.5)
    sent.n, sent.t = 0, 0
    if dbg and dbg:IsShown() then send("debug"); G.DebugDraw() end
end

if SlashCmdList then
    SLASH_LEYLINEBUILDDEBUG1 = "/wudebug"
    SlashCmdList.LEYLINEBUILDDEBUG = function() G.Debug() end
end

-- ---------------------------------------------------------------- the den-test (DenBar.lua D.TestAction "gizmo ...")

function G.TestAction(w)
    local a = w[2] or ""
    if a == "build" then
        local on = w[3] ~= "off"
        if on and not stateKnown() then G.AskState() end
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
    -- the mouse, as if it were there: a click at (u, v) (`add`: Shift), a press-drag-let-go from one point to another,
    -- or its three steps (a picture can be taken while it is held)
    if a == "click" then
        local u, v = tonumber(w[3]), tonumber(w[4])
        if not u or not v then return false end
        return send(string.format("click %.4f %.4f %.4f %d%s", u, v, 16 / 9, G.zoom, w[5] == "add" and " add" or ""))
    end
    if a == "carry" then
        -- a click that picks the prop (or an arrow) up at (u, v)
        local u, v = tonumber(w[3]), tonumber(w[4])
        if not u or not v then return false end
        return send(string.format("click %.4f %.4f %.4f %d carry", u, v, 16 / 9, G.zoom))
    end
    if a == "debug" then G.Debug(w[3] ~= "off"); return send("debug") end
    if a == "press" then
        local u, v = tonumber(w[3]), tonumber(w[4])
        if not u or not v then return false end
        G.dragging = true
        bindings(); showTag()
        return send(string.format("drag %.4f %.4f %.4f %d", u, v, 16 / 9, G.zoom))
    end
    if a == "moveto" then
        local u, v = tonumber(w[3]), tonumber(w[4])
        if not u or not v then return false end
        return send(string.format("hover %.4f %.4f %.4f %d", u, v, 16 / 9, G.zoom))
    end
    if a == "release" then
        G.dragging = nil
        bindings(); showTag()
        return send("commit")
    end
    if a == "drag" then
        local u1, v1, u2, v2 = tonumber(w[3]), tonumber(w[4]), tonumber(w[5]), tonumber(w[6])
        if not (u1 and v1 and u2 and v2) then return false end
        G.TestAction({ "gizmo", "press", w[3], w[4] })
        G.TestAction({ "gizmo", "moveto", w[5], w[6] })
        return G.TestAction({ "gizmo", "release" })
    end
    if a == "turn" then return G.Turn(tonumber(w[3]) or 1) end
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
        local D, B = W.DenBar, W.Builder
        report(string.format("why wanted=%d active=%d play=%d lite=%s denrealm=%d",
            G.Wanted() and 1 or 0, (D and D.active) and 1 or 0, (D and D.play) and 1 or 0,
            tostring(B and B.state and B.state.lite or "-"), (W.InDen and W.InDen()) and 1 or 0))
        report(string.format("gizmo build=%d armed=%s axis=%s sel=%s camera=%d menu=%d cursor=%s,%s,%s ghost=%s zoom=%d far=%s snapkey=%s moved=%s hovered=%s dragging=%d button1=%d frame=%d",
            G.build and 1 or 0, G.armed or "-", G.axis or "-", tostring(G.sel or "-"), G.camera and 1 or 0,
            (menu and menu:IsShown()) and 1 or 0, u and string.format("%.3f", u) or "-", v and string.format("%.3f", v) or "-",
            asp and string.format("%.3f", asp) or "-", G.ghost or "-", G.zoom or 0,
            G.far and string.format("%.1f%s", G.far.back, G.far.snap and "s" or "") or "-", G.snapKey or "-", ((G.lastMove or "-"):gsub(" ", "_")),
            G.hovered and tostring(G.hovered.id) or "-", G.dragging and 1 or 0, G.b1 and 1 or 0, (frame and frame:IsShown()) and 1 or 0))
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
    -- a pan key: down pans at once and keeps panning while held; up stops (a click from elsewhere: one step)
    for k, dir in pairs(PAN_KEYS) do
        local b = CreateFrame("Button", "LeylineGizmoPan" .. k, UIParent)
        if b.RegisterForClicks then b:RegisterForClicks("AnyDown", "AnyUp") end
        b:SetScript("OnClick", function(_, _, isDown) G.PanKey(dir, isDown) end)
        b:Hide()
    end
    for k, sign in pairs(TURN_KEYS) do
        local b = CreateFrame("Button", "LeylineGizmoTurn" .. k, UIParent)
        if b.RegisterForClicks then b:RegisterForClicks("AnyDown") end
        b:SetScript("OnClick", function(_, _, isDown) if isDown ~= false then G.Turn(sign) end end)
        b:Hide()
    end
    -- BUTTON1 over a prop: the press and the let-go are ours (the camera doesn't turn)
    grabBtn = CreateFrame("Button", "LeylineGizmoGrab", UIParent)
    if grabBtn.RegisterForClicks then grabBtn:RegisterForClicks("AnyDown", "AnyUp") end
    grabBtn:SetScript("OnClick", function(_, _, isDown)
        if isDown == false then G.MouseUp("LeftButton") else G.MouseDown("LeftButton") end
    end)
    grabBtn:Hide()
    -- the prop's name by the cursor
    tip = CreateFrame("Frame", "LeylineGizmoHoverTip", UIParent)
    tip:SetFrameStrata("TOOLTIP")
    tip:SetWidth(160); tip:SetHeight(20)
    tip.fs = tip.CreateFontString and tip:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall") or { SetText = function() end }
    if tip.fs.SetPoint then tip.fs:SetPoint("LEFT", tip, "LEFT", 6, 0) end
    tip:Hide()
    -- build mode's frame round the screen and its word, so it is plain which mode you are in
    frame = CreateFrame("Frame", "LeylineGizmoBuildFrame", UIParent)
    frame:SetAllPoints(UIParent)
    frame:SetFrameStrata("BACKGROUND")
    if frame.EnableMouse then frame:EnableMouse(false) end
    local EDGE, R, Gc, Bc, A = 4, 0.95, 0.72, 0.25, 0.85
    local function edge(p1, p2, w, h)
        local t = frame.CreateTexture and frame:CreateTexture(nil, "OVERLAY")
        if not t then return end
        t:SetTexture(R, Gc, Bc, A)
        t:SetPoint(p1, frame, p1, 0, 0); t:SetPoint(p2, frame, p2, 0, 0)
        if w then t:SetWidth(w) end
        if h then t:SetHeight(h) end
    end
    edge("TOPLEFT", "TOPRIGHT", nil, EDGE); edge("BOTTOMLEFT", "BOTTOMRIGHT", nil, EDGE)
    edge("TOPLEFT", "BOTTOMLEFT", EDGE, nil); edge("TOPRIGHT", "BOTTOMRIGHT", EDGE, nil)
    frame.word = frame.CreateFontString and frame:CreateFontString(nil, "OVERLAY", "GameFontNormalLarge") or { SetText = function() end }
    if frame.word.SetPoint then frame.word:SetPoint("TOP", frame, "TOP", 0, -10) end
    frame.word:SetText("BUILD")
    if frame.word.SetTextColor then frame.word:SetTextColor(R, Gc, Bc) end
    frame:Hide()
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
            local D = W.DenBar
            if D and D.active and not D.play and not stateKnown() then
                G.asked = (G.asked or 0) + 1
                if G.asked % 4 == 1 then G.AskState() end   -- at once, then every 2 s until it comes
            else
                G.asked = 0
            end
            G.SetBuild(G.Wanted())
        end
        G.UpCheck()
        G.DragCheck()
        if G.armed or G.dragging or G.build then G.Tick(false) end
        G.TipUpdate()
        G.PanHeld()
        G.DebugTick(el)
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
