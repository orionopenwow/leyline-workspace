-- texlua stream/tests/test_wuui_gizmo.lua [addon dir]
-- Your Den's placement aids (Gizmo.lua; Den Lite labs/den-lite src/gizmo.rs, farsight.rs). Fake 3.3.5a API and a fake Builder /
-- Den bar: build mode starts in Your Den on Den Lite (mode build, gizmo on, the build camera from a reset view at a
-- known zoom, the player's own view kept and put back); nothing is bound until something is armed; a gallery pick
-- arms the live preview and the cursor's place on the screen goes to Den Lite (u, v, aspect, distance; at most 10 a
-- second; not over a window); a left click places, a drag doesn't (the camera's pose comes back), the exact click's
-- targeting is left to the game; right-click and Esc stop; a picked axis binds the wheel to the Builder's move; a
-- right-clicked prop opens the menu at the cursor; a left click on empty ground clears the selection; play mode
-- gives everything back; no game-brand words.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- fake widgets
local Frame = {}
Frame.__index = function(self, k)
    local v = rawget(Frame, k)
    if v ~= nil then return v end
    if type(k) == "string" and k:match("^%u") then return function() end end
    return nil
end
function Frame:SetScript(n, fn) self.scripts[n] = fn end
function Frame:HookScript(n, fn)
    local old = self.scripts[n]
    self.scripts[n] = function(...) if old then old(...) end; return fn(...) end
end
function Frame:Fire(n, ...) local fn = self.scripts[n]; if fn then return fn(self, ...) end end
function Frame:Show() self.shown = true end
function Frame:Hide() self.shown = false end
function Frame:IsShown() return self.shown end
function Frame:SetText(t) self.text = t end
function Frame:GetText() return self.text end
function Frame:SetAttribute(k, v) self.attrs[k] = v end
function Frame:RegisterEvent(e) self.events[e] = true end
function Frame:SetPoint(...) self.point = { ... } end
function Frame:CreateFontString() return setmetatable({ scripts = {}, attrs = {}, events = {}, shown = true }, Frame) end
function Frame:GetWidth() return self.w or 0 end
function Frame:GetHeight() return self.h or 0 end
function Frame:GetEffectiveScale() return self.scale or 1 end
local named = {}
local function new(kind, name, parent, template)
    local f = setmetatable({ kind = kind, name = name, parent = parent, template = template, scripts = {}, attrs = {}, events = {}, shown = true }, Frame)
    if name then named[name] = f; _G[name] = f end
    return f
end
function CreateFrame(kind, name, parent, template) return new(kind, name, parent, template) end
UIParent = new("Frame", "UIParent")
UIParent.w, UIParent.h, UIParent.scale = 1920, 1080, 1
WorldFrame = new("Frame", "WorldFrame")
GameTooltip = new("GameTooltip", "GameTooltip")
GameTooltip.shown = false

local clock = 100
function GetTime() return clock end
local cursor = { 960, 540 }
function GetCursorPosition() return cursor[1], cursor[2] end
local focus = WorldFrame
function GetMouseFocus() return focus end
local targeting = false
function SpellIsTargeting() return targeting end
local mouseoverUnit = false
function UnitExists(u) return u == "mouseover" and mouseoverUnit end
function UnitName() return "Wren" end
function InCombatLockdown() return false end
local shift = false
function IsShiftKeyDown() return shift end
local cvars = { cameraDistanceMax = "15", cameraDistanceMaxFactor = "2" }
function GetCVar(k) return cvars[k] end
function SetCVar(k, v) cvars[k] = tostring(v) end
local gameBinds = { J = "TOGGLEFOO" }
function GetBindingAction(k) return gameBinds[k] or "" end
function GetSpellInfo(id) return id == 1543 and "Flare" or nil end
local sent = {}
function SendAddonMessage(prefix, text, channel, target) sent[#sent + 1] = { prefix, text, channel, target } end
local bindings = {}
function SetOverrideBindingClick(owner, prio, key, button) bindings[key] = button end
function ClearOverrideBindings() bindings = {} end
local camera = {}
function SaveView(n) camera[#camera + 1] = "save " .. n end
function ResetView(n) camera[#camera + 1] = "reset " .. n end
function SetView(n) camera[#camera + 1] = "set " .. n end
function CameraZoomIn(n) camera[#camera + 1] = "in " .. n end
function CameraZoomOut(n) camera[#camera + 1] = "out " .. n end
function hooksecurefunc(t, name, fn)
    local old = t[name]
    t[name] = function(...) local r = { old(...) }; fn(...); return unpack and unpack(r) or table.unpack(r) end
end

-- the Builder and the Den bar as Gizmo.lua sees them
local moves, notes, handDone = {}, {}, 0
WrathUnleashedUI = {
    Builder = { state = { lite = "1", selName = "Cottage" }, Choose = function(item) return true end, Move = function(a, s) moves[#moves + 1] = a .. s; return true end,
        ShowProps = function(show) WrathUnleashedUI.propsShown = show end, STEPS = { 0.01, 0.1, 0.25, 0.5, 1, 3, 5, 10 }, step = 5,
        PillNote = function(t) notes[#notes + 1] = t end, HandDone = function() handDone = handDone + 1 end },
    DenBar = { active = true, play = false },
}
local W = WrathUnleashedUI

SlashCmdList = SlashCmdList or {}
dofile(dir .. "/Gizmo.lua")
local G = W.Gizmo
check("loaded and set up", G and named.LeylineGizmo and named.LeylineGizmoAim)
check("the exact click: a secure spell button for the taught spell, by its own name",
    named.LeylineGizmoAim.template == "SecureActionButtonTemplate" and named.LeylineGizmoAim.attrs.type == "spell" and named.LeylineGizmoAim.attrs.spell == "Flare")

local owner = named.LeylineGizmo
local function tick(dt) clock = clock + (dt or 0.5); owner:Fire("OnUpdate", dt or 0.5) end
local function cmds(from)
    local out = {}
    for i = (from or 0) + 1, #sent do
        local t = sent[i][2]:match("^wubuild\tcmd\t1/1\t(.*)$")
        if t then out[#out + 1] = t end
    end
    return out
end
local function answer(name, data)
    owner:Fire("OnEvent", "CHAT_MSG_ADDON", "OVMOD", "wubuild\t" .. name .. "\t1/1\t" .. data, "WHISPER", "Wren")
end
local function has(list, want) for _, v in ipairs(list) do if v == want then return true end end return false end
local function count(t) local n = 0; for _ in pairs(t) do n = n + 1 end; return n end

-- build mode
tick()
local c = cmds()
check("in the Den on Den Lite: build mode and the gizmo", G.build and has(c, "mode build") and has(c, "gizmo on"))
check("the build camera: the player's view kept, a reset view, all the way in, out by the build distance, kept in view 2",
    table.concat(camera, ",") == "save 5,reset 2,set 2,in 50,out 12,save 2", table.concat(camera, ","))
check("as far out as the game goes: its zoom limit raised (and its own value kept)", cvars.cameraDistanceMax == "50" and G.cvars.max == "15")
check("the zoom told to Den Lite", has(c, "cam zoom 12"))
check("build mode binds only the wheel (the zoom, kept count of) and the snap-out key: J is the game's, so K",
    bindings.MOUSEWHEELUP == "LeylineGizmoWheelUp" and bindings.MOUSEWHEELDOWN == "LeylineGizmoWheelDown" and bindings.K == "LeylineGizmoSnap"
    and not bindings.J and count(bindings) == 3)
check("never a movement key", not (bindings.W or bindings.A or bindings.S or bindings.D or bindings.Q or bindings.E or bindings.UP or bindings.LEFT))

-- a gallery pick arms the preview
local n = #sent
W.Builder.Choose({ id = 2413, name = "Wooden Chair" })
c = cmds(n)
check("a gallery pick arms the preview, and the cursor goes at once", c[1] == "preview 2413" and c[2] and c[2]:find("^hover "))
answer("aim", "place,2413,1543")
check("in hand: Esc, the exact-click key and Q / E (turn it) bound too, nothing else", G.armed == "place" and bindings.ESCAPE == "LeylineGizmoCancel"
    and bindings.G == "LeylineGizmoAim" and bindings.Q == "LeylineGizmoTurnQ" and bindings.E == "LeylineGizmoTurnE" and count(bindings) == 7)
check("the tag says how", named.LeylineGizmoTag.shown and named.LeylineGizmoTag.fs.text:find("in hand: click the ground")
    and named.LeylineGizmoTag.fs.text:find("Shift%+click places more"))

-- the cursor's place on the screen
cursor = { 1440, 270 }
n = #sent
tick(0.15)
c = cmds(n)
check("hover: u 0.5, v -0.5, the screen's aspect, the build distance", c[1] == "hover 0.5000 -0.5000 1.7778 12")
cursor = { 1450, 270 }
n = #sent
tick(0.02)
check("in hand: at most about 30 a second (owner 4 Nov: it keeps up)", #cmds(n) == 0)
tick(0.1)
check("then again", (cmds(n)[1] or ""):find("^hover 0%.5104"))
n = #sent
tick(0.2)
check("the cursor still: nothing sent", #cmds(n) == 0)
focus = named.LeylineGizmoTag
cursor = { 100, 100 }
tick(0.2)
check("over a window: nothing sent", #cmds(n) == 0)
focus = WorldFrame
UIParent.scale = 0.8
cursor = { 768, 432 }
n = #sent
tick(0.2)
check("the UI's scale is taken out", cmds(n)[1] == "hover 0.0000 0.0000 1.7778 12")
UIParent.scale = 1

-- clicks
local function click(button, dt, dx)
    WorldFrame:Fire("OnMouseDown", button)
    clock = clock + (dt or 0.1)
    cursor = { cursor[1] + (dx or 0), cursor[2] }
    WorldFrame:Fire("OnMouseUp", button)
end
n = #sent
click("LeftButton")
c = cmds(n)
check("a left click places it where the copy is (one click: placed, then selected)", c[#c] == "commit")
shift = true
n = #sent
click("LeftButton")
check("Shift+click: placed, and another in hand", cmds(n)[#cmds(n)] == "commit keep")
shift = false
n = #sent
camera = {}
click("LeftButton", 1.2, 80)
check("a drag (the camera) places nothing; the build camera's pose comes back", not has(cmds(n), "commit") and camera[1] == "set 2")
targeting = true
n = #sent
click("LeftButton")
check("the exact click's targeting: left to the game (Den Lite gets the cast)", #cmds(n) == 0)
targeting = false
n = #sent
click("RightButton")
c = cmds(n)
check("a right click stops: preview off, its keys given back (the wheel and snap-out stay in build mode)", c[1] == "preview off" and G.armed == nil
    and count(bindings) == 3 and handDone > 0)
check("the tag goes back to build mode's hint (a Den Lite from before: right-click selects)",
    named.LeylineGizmoTag.shown and named.LeylineGizmoTag.fs.text:find("Right%-click a prop to select it"))

-- the gizmo's axis and the wheel
answer("axis", "y")
check("an axis: the wheel bound (and Esc), nothing else", G.axis == "y" and bindings.MOUSEWHEELUP == "LeylineGizmoWheelUp"
    and bindings.MOUSEWHEELDOWN == "LeylineGizmoWheelDown" and bindings.ESCAPE and not bindings.G)
named.LeylineGizmoWheelUp:Fire("OnClick")
named.LeylineGizmoWheelDown:Fire("OnClick")
check("the wheel moves the prop along the axis by the Builder's step", moves[1] == "y1" and moves[2] == "y-1")
n = #sent
named.LeylineGizmoCancel:Fire("OnClick")
check("Esc lets the axis go: the wheel zooms again", G.axis == nil and cmds(n)[1] == "axis off" and count(bindings) == 3)

-- a right-clicked prop: the menu at the cursor
cursor = { 500, 400 }
answer("state", "mine=1/800;sel=5,2413,Wooden Chair;n=1;pos=1.00,2.00,3.00,0,2.00;")
answer("picked", "5")
local menu = named.LeylineGizmoMenu
check("the menu opens where the cursor is", menu and menu.shown and menu.point[4] == 508 and menu.point[5] == 392 and G.sel == 5)
check("Esc closes it", bindings.ESCAPE == "LeylineGizmoCancel")
local labels = {}
for i, r in ipairs(menu.rows) do labels[i] = r.label.text end
check("the menu: delete, duplicate, move, rotate, size, floor, lock, copy, properties",
    table.concat(labels, "|") == "Delete|Duplicate|Move (follows the mouse)|Rotate 15°|Bigger|Smaller|Snap to floor|Lock|Copy|Properties")
n = #sent
menu.rows[5]:Fire("OnClick")
check("Bigger: a quarter more than its size now", cmds(n)[1] == "setscale 2.50" and not menu.shown)
G.Picked(5)
n = #sent
menu.rows[1]:Fire("OnClick")
check("Delete", cmds(n)[1] == "delete")
G.Picked(5)
n = #sent
menu.rows[3]:Fire("OnClick")
check("Move: the selected prop follows the mouse", cmds(n)[1] == "preview move")
answer("aim", "move,5,1543")
check("moving: armed", G.armed == "move" and bindings.G)
G.Cancel()
G.Picked(5)
menu.rows[10]:Fire("OnClick")
check("Properties opens the Builder's panel", W.propsShown == true)

-- Shift+right-click on another prop: both selected (seladd the one before)
shift = true
n = #sent
answer("picked", "6")
check("Shift+right-click adds: the one before stays selected", G.sel == 6 and cmds(n)[1] == "seladd 5")
shift = false
G.Cancel()

-- a left click on empty ground clears the selection; not on a named object
n = #sent
click("LeftButton")
check("a left click on empty ground: the selection cleared", cmds(n)[1] == "selclear")
GameTooltip.shown = true
n = #sent
click("LeftButton")
check("on an object the game names: kept", #cmds(n) == 0)
GameTooltip.shown = false

-- one click places it and ends placing: Den Lite says placed, then aim off; the panel's "in hand" ends
W.Builder.Choose({ id = 2413, name = "Cottage" })
answer("aim", "place,2413,1543")
local hd = handDone
answer("placed", "7")
answer("aim", "off")
check("placed: the new prop is the selection, nothing in hand, the in-hand keys given back", G.sel == 7 and G.armed == nil and handDone > hd
    and not bindings.G and not bindings.ESCAPE)

-- a handle grabbed: the prop follows the cursor along its axis; the wheel steps; a click drops it
answer("aim", "axis,7,x")
check("dragging along x: armed, Esc and the wheel bound", G.armed == "axis" and G.axis == "x" and bindings.ESCAPE and bindings.MOUSEWHEELUP)
check("the tag says so", named.LeylineGizmoTag.fs.text:find("Dragging along X %(red%)"))
cursor = { 1200, 400 }
n = #sent
tick(0.2)
check("the cursor goes to Den Lite (the drag follows it)", (cmds(n)[1] or ""):find("^hover "))
n = #sent
named.LeylineGizmoWheelUp:Fire("OnClick")
check("the wheel: a step along the axis (the Builder's step)", cmds(n)[1] == "grab nudge 1")
answer("drag", "x,+4.45")
check("the status: Moving Cottage +4.45 yd along X", notes[#notes] == "Moving Cottage +4.45 yd along X")
n = #sent
click("LeftButton")
check("a left click drops it", cmds(n)[#cmds(n)] == "commit")
answer("moved", "x,+4.45")
answer("aim", "off")
answer("axis", "-")
check("dropped: Moved Cottage +4.45 yd along X", notes[#notes] == "Moved Cottage +4.45 yd along X" and G.armed == nil)
answer("aim", "axis,7,z")
n = #sent
named.LeylineGizmoCancel:Fire("OnClick")
check("Esc while dragging: puts it back (grab off)", cmds(n)[1] == "grab off" and G.armed == nil)
n = #sent
check("test: grab a handle", G.TestAction({ "gizmo", "grab", "y" }) and cmds(n)[1] == "grab y")

-- the wheel: the game's zoom, kept count of; past the game's limit Den Lite's far view; and back
camera = {}
n = #sent
named.LeylineGizmoWheelDown:Fire("OnClick")
check("wheel down: the game zooms out 2 yards, kept count of, told to Den Lite", camera[1] == "out 2" and G.zoom == 14 and cmds(n)[1] == "cam zoom 14")
local hovered = false
for _ = 1, 30 do named.LeylineGizmoWheelDown:Fire("OnClick") end
c = cmds(n)
check("the game's own limit (50): no further than that", G.zoom == 50 and has(c, "cam zoom 50"))
check("past it: Den Lite's far view, a notch at a time", c[#c] == "cam far 1")
answer("cam", "far,8.0,0,0")
check("the far view: the build camera posed again round Den Lite's camera object", G.far and G.far.back == 8 and camera[#camera] == "save 2")
check("the arrows pan it while it is out", bindings.UP == "LeylineGizmoPanUP" and bindings.RIGHT == "LeylineGizmoPanRIGHT")
n = #sent
named.LeylineGizmoPanUP:Fire("OnClick")
check("an arrow: the free flight forward", (cmds(n)[1] or ""):find("^cam fly 1 0 0 ") ~= nil)
n = #sent
named.LeylineGizmoWheelUp:Fire("OnClick")
check("wheel up while far: nearer (Den Lite)", cmds(n)[1] == "cam far -1")
answer("cam", "near")
check("back to the player's view: the arrows given back", G.far == nil and not bindings.UP)
n = #sent
named.LeylineGizmoWheelUp:Fire("OnClick")
check("then the game's own zoom again", G.zoom == 48 and cmds(n)[1] == "cam zoom 48")

-- snap out: one key, and back
n = #sent
named.LeylineGizmoSnap:Fire("OnClick")
check("the snap-out key: cam snap", cmds(n)[1] == "cam snap")
answer("cam", "far,35.0,1,90")
check("snapped out: the overview, the tag says how back", G.far and G.far.snap and named.LeylineGizmoTag.fs.text:find("Overview: K"))
camera = {}
click("LeftButton", 1.2, 80)
check("a camera drag in the overview: the pose comes back (the ray needs it)", camera[1] == "set 2")
n = #sent
check("test: snap", G.TestAction({ "gizmo", "snap" }) and cmds(n)[1] == "cam snap")
answer("cam", "near")
n = #sent
check("test: zoom and pan", G.TestAction({ "gizmo", "zoom", "-2" }) and has(cmds(n), "cam zoom 44") and not G.TestAction({ "gizmo", "pan", "left" }))

-- the den-test's actions
n = #sent
check("test: hover at a point", G.TestAction({ "gizmo", "hover", "0.25", "-0.4" }) and cmds(n)[1] == "hover 0.2500 -0.4000 1.7778 12")
n = #sent
G.TestAction({ "gizmo", "report" })
check("test: the report", sent[#sent][2]:find("^wuden\treport\t1/1\tgizmo build=1 armed=%- axis=%-") and sent[#sent][2]:find("zoom=44 far=%- snapkey=K"))

-- play mode: everything given back
W.Builder.Choose({ id = 2413 })
answer("aim", "place,2413,1543")
camera = {}
W.DenBar.play = true
n = #sent
tick()
c = cmds(n)
check("play mode: the preview stops, the gizmo goes, the props are plain again", has(c, "preview off") and has(c, "gizmo off") and has(c, "mode play") and not G.build)
check("the player's own camera back, and the game's zoom limit", camera[#camera] == "set 5" and cvars.cameraDistanceMax == "15")
check("nothing bound", count(bindings) == 0)
n = #sent
W.Builder.Choose({ id = 2413 })
check("in play mode a pick arms nothing", #cmds(n) == 0)

-- not Den Lite (the full Den): no build mode
W.DenBar.play = false
W.Builder.state.lite = nil
tick()
check("on the full Den: no build mode", not G.build)

-- ---------------------------------------------------------------- the mouse, picked by Den Lite (its caps: click)
local mouseHeld = {}
function IsMouseButtonDown(b) return mouseHeld[b] and true or false end
local looking = true
function IsMouselooking() return looking end
function MouselookStop() looking = false end
W.Builder.state.lite = "1"
W.Builder.Can = function(v) return v == "click" or v == "drag" end
W.DenBar.play = false
W.DenBar.keys = { build = "F7", play = "F8" }
answer("state", "mine=1/800;sel=-;n=1;")
local function at(list, want) for i, v in ipairs(list) do if v == want then return i end end return nil end
camera = {}
n = #sent
tick()
c = cmds(n)
check("build mode with a picking Den Lite: the editor's own view (cam build) after the zoom", G.build and has(c, "mode build")
    and at(c, "cam build") and at(c, "cam zoom 12") and at(c, "cam build") > at(c, "cam zoom 12"))
check("the cursor is freed (no mouselook) on entering", looking == false)
check("a BUILD frame round the screen", named.LeylineGizmoBuildFrame.shown and named.LeylineGizmoBuildFrame.word.text == "BUILD")
check("the hint line", named.LeylineGizmoTag.shown and named.LeylineGizmoTag.fs.text:find("Click a prop to pick it up, click again to put it down  ·  Shift adds  ·  Q E turn  ·  Del deletes  ·  F7 leaves Build", 1, true))
answer("cam", "far,14.0,0,0")
check("the view out: WASD and the arrows pan it, Q / E not yet (nothing selected), BUTTON1 still the game's",
    bindings.W == "LeylineGizmoPanW" and bindings.A == "LeylineGizmoPanA" and bindings.UP == "LeylineGizmoPanUP" and not bindings.Q and not bindings.BUTTON1)
check("the tag: how to pan", named.LeylineGizmoTag.fs.text:find("WASD pans  ·  wheel zooms", 1, true))

-- the hover stream with nothing in hand
cursor = { 1440, 270 }
n = #sent
tick(0.15)
check("in build mode the cursor's place goes even with nothing in hand", cmds(n)[1] == "hover 0.5000 -0.5000 1.7778 12")
n = #sent
tick(0.2)
check("at rest: once more (Den Lite's glow keeps up)", cmds(n)[1] == "hover 0.5000 -0.5000 1.7778 12")
n = #sent
tick(0.2); tick(0.2)
check("then nothing", #cmds(n) == 0)

-- a prop under the cursor: its name by the cursor, BUTTON1 ours
answer("hovered", "5 Wooden Chair")
check("hovered: the name by the cursor", G.hovered and G.hovered.id == 5 and named.LeylineGizmoHoverTip.shown and named.LeylineGizmoHoverTip.fs.text == "Wooden Chair")
check("hovered: BUTTON1 is ours (the camera doesn't turn on a press on a prop)", bindings.BUTTON1 == "LeylineGizmoGrab" and G.b1)
local grab = named.LeylineGizmoGrab
local function press(both) mouseHeld.LeftButton = true; if both then WorldFrame:Fire("OnMouseDown", "LeftButton") end; grab:Fire("OnClick", "LeftButton", true) end
local function release(both) mouseHeld.LeftButton = nil; grab:Fire("OnClick", "LeftButton", false); if both then WorldFrame:Fire("OnMouseUp", "LeftButton") end end
n = #sent
press(true); clock = clock + 0.1; release(true)
c = cmds(n)
check("a click on it (the hook and the binding both saw it): picked up once, at the press point", #c == 1 and c[1] == "click 0.5000 -0.5000 1.7778 12 carry")
answer("selected", "5")
check("selected: Q / E turn it", G.sel == 5 and bindings.Q == "LeylineGizmoTurnQ" and bindings.E == "LeylineGizmoTurnE")
n = #sent
named.LeylineGizmoTurnQ:Fire("OnClick", "LeftButton", true)
named.LeylineGizmoTurnE:Fire("OnClick", "LeftButton", true)
check("Q turns it left, E right", cmds(n)[1] == "turn 15" and cmds(n)[2] == "turn -15")
shift = true
n = #sent
press(); clock = clock + 0.1; release()
check("Shift+click adds it", cmds(n)[1] == "click 0.5000 -0.5000 1.7778 12 add")
shift = false

-- press and drag: the drag starts past 6 px, from where it was pressed; the wheel turns it; letting go drops it
n = #sent
press()
cursor = { 1444, 272 }
tick(0.05)
local function drags(list) local k = 0; for _, x in ipairs(list) do if x:find("^drag ") then k = k + 1 end end; return k end
check("held, moved 4 px: still a click", not G.dragging and drags(cmds(n)) == 0)
n = #sent
cursor = { 1500, 300 }
tick(0.05)
c = cmds(n)
check("moved past 6 px: Den Lite grabs what was under the press, then the cursor's place", G.dragging and c[1] == "drag 0.5000 -0.5000 1.7778 12" and (c[2] or ""):find("^hover 0%.5625 %-0%.4444"))
check("dragging: the name tip hides, the tag says how", not named.LeylineGizmoHoverTip.shown and named.LeylineGizmoTag.fs.text:find("Moving: let go to drop it"))
answer("aim", "move,5,1543")
answer("hovered", "-")
check("the binding stays ours while the button is held", bindings.BUTTON1 == "LeylineGizmoGrab")
cursor = { 1520, 320 }
n = #sent
tick(0.15)
check("the prop follows the cursor", (cmds(n)[1] or ""):find("^hover "))
n = #sent
named.LeylineGizmoWheelUp:Fire("OnClick")
check("the wheel while dragging turns it", cmds(n)[1] == "preview turn 15")
n = #sent
release()
c = cmds(n)
check("let go: dropped (commit), the drag over", c[#c] == "commit" and not G.dragging)
answer("aim", "off")
check("over empty ground after: BUTTON1 the game's again", not bindings.BUTTON1)

-- Esc while dragging puts it back; the let-go after does nothing
answer("hovered", "5 Wooden Chair")
n = #sent
press(); cursor = { 1400, 300 }; tick(0.05)
answer("aim", "move,5,1543")
named.LeylineGizmoCancel:Fire("OnClick")
release()
c = cmds(n)
check("Esc while dragging: put back (preview off), no drop after", has(c, "preview off") and not has(c, "commit") and not G.dragging)
answer("aim", "off")

-- a quick flick (pressed, moved and let go before the next frame): a drag and a drop
answer("hovered", "5 Wooden Chair")
cursor = { 1400, 300 }
n = #sent
press(); cursor = { 1440, 330 }; release()
c = cmds(n)
check("a quick flick on a prop: dragged and dropped, not a camera drag", (c[1] or ""):find("^drag 0%.4583") and c[#c] == "commit" and not G.dragging)
answer("aim", "off")

-- a press on a prop that was gone by then: Den Lite says nothing was grabbed
press(); cursor = { 1300, 300 }; tick(0.05)
answer("grabbed", "-")
check("nothing grabbed: the drag is over", not G.dragging)
release()

-- a lost let-go (over a window): caught by the button's state
answer("hovered", "5 Wooden Chair")
n = #sent
press()
mouseHeld.LeftButton = nil
clock = clock + 0.05
owner:Fire("OnUpdate", 0.05)
check("a let-go the game kept from us: caught (a click; still over the prop, BUTTON1 still ours)", (cmds(n)[1] or ""):find("^click ") and G.b1 == true)

-- a click on empty ground (BUTTON1 the game's: the hook sees it): Den Lite clears the selection
answer("hovered", "-")
n = #sent
WorldFrame:Fire("OnMouseDown", "LeftButton"); clock = clock + 0.1; WorldFrame:Fire("OnMouseUp", "LeftButton")
check("a click on empty ground: click (Den Lite clears the selection), never the old selclear", (cmds(n)[1] or ""):find("^click ") and not has(cmds(n), "selclear"))

-- a fly key held: said at once and again while held (Den Lite glides ahead), one let-go when released; Space goes up
n = #sent
named.LeylineGizmoPanW:Fire("OnClick", "LeftButton", true)
tick(0.25)
named.LeylineGizmoPanSPACE:Fire("OnClick", "LeftButton", true)
named.LeylineGizmoPanW:Fire("OnClick", "LeftButton", false)
named.LeylineGizmoPanSPACE:Fire("OnClick", "LeftButton", false)
tick(0.25)
tick(0.25)
c = cmds(n)
local fwd, ups, stops = 0, 0, 0
for _, x in ipairs(c) do
    if x:find("^cam fly 1 0 0 ") then fwd = fwd + 1 end
    if x:find("^cam fly 0 0 1 ") or x:find("^cam fly 1 0 1 ") then ups = ups + 1 end
    if x:find("^cam fly 0 0 0 ") then stops = stops + 1 end
end
check("W held: the view flies on, Space lifts it, and one let-go when released", fwd >= 2 and ups >= 1 and stops >= 1 and (c[#c] or ""):find("^cam fly 0 0 0 ") ~= nil)

-- the den-test's mouse
n = #sent
G.TestAction({ "gizmo", "click", "0.1", "-0.2" })
G.TestAction({ "gizmo", "drag", "0.1", "-0.2", "0.3", "-0.25" })
c = cmds(n)
check("test: click, and press-move-let-go", c[1] == "click 0.1000 -0.2000 1.7778 12" and c[2] == "drag 0.1000 -0.2000 1.7778 12"
    and c[3] == "hover 0.3000 -0.2500 1.7778 12" and c[4] == "commit" and not G.dragging)
G.TestAction({ "gizmo", "report" })
check("test: the report says the mouse's state", sent[#sent][2]:find("hovered=%- dragging=0 button1=0 frame=1"))

-- click to carry (owner 4 Nov): the test's own carry click, Den Lite's answer, the hint, and the next click puts it down
n = #sent
G.TestAction({ "gizmo", "carry", "0.1", "-0.2" })
c = cmds(n)
check("test: a carry click", c[1] == "click 0.1000 -0.2000 1.7778 12 carry")
answer("aim", "move,5,1543")
answer("carried", "5")
check("carried: armed to move, the prop known", G.armed == "move" and G.carried == "5")
answer("aim", "off")
check("put down: nothing carried", G.armed == nil and G.carried == nil)
answer("carried", "axis y")
check("an arrow clicked: carried along Y", G.carried == "axis y")
answer("aim", "off")

-- review 4 Nov: picking a hovered prop up lets the hover go (no stale tooltip, BUTTON1 back to the camera after the
-- drop), and the carried prop is the selection (Q / E turn it once it is down)
answer("selected", "-")
answer("hovered", "7 Garden Bench")
check("carry: hovered first, BUTTON1 ours", G.hovered and G.hovered.id == 7 and bindings.BUTTON1 == "LeylineGizmoGrab")
answer("aim", "move,7,1543")
answer("carried", "7")
check("carry: the hover let go once it is in hand (no name by the cursor)", G.hovered == nil and not named.LeylineGizmoHoverTip.shown)
check("carry: the carried prop is the selection", G.sel == 7)
check("carry: Esc and Q / E bound while carried", bindings.ESCAPE == "LeylineGizmoCancel" and bindings.Q == "LeylineGizmoTurnQ")
answer("aim", "off")
check("carry: put down on empty ground: BUTTON1 back to the camera, Q / E still turn it",
    bindings.BUTTON1 == nil and not G.b1 and G.sel == 7 and bindings.Q == "LeylineGizmoTurnQ")
answer("carried", "axis x")
check("carry: an arrow's carry leaves the selection as it is", G.sel == 7 and G.carried == "axis x")
answer("aim", "off")

-- the debug overlay (/wudebug): shown and hidden, asks Den Lite, shows its line split up
n = #sent
check("debug: the slash command is there", SlashCmdList and SlashCmdList.LEYLINEBUILDDEBUG ~= nil)
G.Debug(true)
c = cmds(n)
check("debug: shown, Den Lite asked", named.LeylineGizmoDebug and named.LeylineGizmoDebug.shown and has(c, "debug"))
answer("debug", "armed=off;at=-;cursor=0.100,-0.200;ground=1.00,2.00,3.00;floor=3.00;handle=-;draw_ms=33")
local txt = tostring(named.LeylineGizmoDebug.fs.text)
check("debug: its own state and Den Lite's, one per line", txt:find("build=on", 1, true) and txt:find("\nground=1.00,2.00,3.00", 1, true) and txt:find("draw_ms=33", 1, true))
n = #sent
tick(0.6)
check("debug: asked again twice a second", has(cmds(n), "debug"))
G.Debug(false)
check("debug: hidden", not named.LeylineGizmoDebug.shown)
n = #sent
tick(0.6)
check("debug: hidden asks nothing", not has(cmds(n), "debug"))

-- Den Lite's state not here yet (4 Nov den-move): build mode stays on, and the state is asked for
local keepState = W.Builder.state
W.Builder.state = {}
local n0 = #sent
G.TestAction({ "gizmo", "build", "on" })
tick(0.6); tick(0.6)
local asked = false
for i = n0 + 1, #sent do if sent[i][2] == "wubuild\tstate\t1/1\t" then asked = true end end
check("no state yet: build mode stays on, the state asked for", G.build and asked)
W.Builder.state = { lite = "0" }
tick(0.6)
check("a full Den's state (lite=0): build mode off", not G.build)
W.Builder.state = keepState
tick(0.6)
check("Den Lite's state back: build mode on again", G.build)

-- leaving build mode: the view back, the frame and every binding gone
W.DenBar.play = true
n = #sent
tick()
c = cmds(n)
check("play mode: the view given back (cam off), nothing bound, no frame", has(c, "cam off") and count(bindings) == 0 and not named.LeylineGizmoBuildFrame.shown)
n = #sent
cursor = { 900, 500 }
tick(0.2)
check("play mode: no hover stream", #cmds(n) == 0)

-- brand words
local words = { "World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard" }
local shown = table.concat(labels, " ") .. " " .. tostring(named.LeylineGizmoTag.fs.text) .. " " .. G.Hint() .. " " .. tostring(named.LeylineGizmoBuildFrame.word.text)
local clean = true
for _, w in ipairs(words) do if shown:find(w, 1, true) then clean = false end end
check("no game-brand words on screen", clean)

print(string.format("%d/%d passed", pass, pass + fail))
if fail > 0 then os.exit(1) end
