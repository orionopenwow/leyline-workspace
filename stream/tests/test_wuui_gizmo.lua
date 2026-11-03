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
check("in hand: Esc and the exact-click key bound too, nothing else", G.armed == "place" and bindings.ESCAPE == "LeylineGizmoCancel"
    and bindings.G == "LeylineGizmoAim" and count(bindings) == 5)
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
tick(0.05)
check("at most 10 a second", #cmds(n) == 0)
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
check("the tag goes", not named.LeylineGizmoTag.shown)

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
check("an arrow: cam pan", cmds(n)[1] == "cam pan fwd 4")
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

-- brand words
local words = { "World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard" }
local shown = table.concat(labels, " ") .. " " .. tostring(named.LeylineGizmoTag.fs.text)
local clean = true
for _, w in ipairs(words) do if shown:find(w, 1, true) then clean = false end end
check("no game-brand words on screen", clean)

print(string.format("%d/%d passed", pass, pass + fail))
if fail > 0 then os.exit(1) end
