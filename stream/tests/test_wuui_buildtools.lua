-- texlua stream/tests/test_wuui_buildtools.lua [addon dir]
-- The Den builder's tools window (BuildTools.lua; Den Lite labs/den-lite src/buildtools.rs, tools.rs), 4 Nov. Fake
-- 3.3.5a API (as test_wuui_gizmo.lua), a fake Style and Builder, the real Gizmo.lua for the click hook: the tabs; Box
-- select's two clicks; the Path tool's clicks (point, move point n), its presets and Lay; Den Lite's answers (the
-- points, a laid path to edit, the stamp library and its picture, groups, history, measure); stamps placed with
-- the mouse, turned, mirrored, sized, kept with Enter or taken back with Esc; saving the selection as a stamp; the
-- layers' rows; history's undo-to; a right-click stops a tool; nothing outside the channel; no game-brand words.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"

local pass, fail = 0, 0
local function check(name, ok, why)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name .. (why and (" (" .. tostring(why) .. ")") or "")) end
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


UISpecialFrames = {}
function tinsert(t, v) table.insert(t, v) end
function Frame:SetWidth(w) self.w = w end
function Frame:SetHeight(h) self.h = h end
function Frame:IsVisible() return self.shown end
function Frame:CreateTexture() return setmetatable({ scripts = {}, attrs = {}, events = {}, shown = true }, Frame) end
function Frame:SetTexture(...) self.tex = { ... } end

-- a fake Style (the editor's look): chips with their text, active state and tooltip
local chips = {}
local Style = { ED = { muted = { 0.6, 0.6, 0.6, 1 }, tile = { 0, 0, 0, 1 }, panel2 = { 0, 0, 0, 1 }, gold = { 1, 0.6, 0.2, 1 } }, ED_SIZE = { small = 13, text = 13, label = 13, title = 15, chip = 28 } }
function Style.EdPanel(f) return f end
function Style.EdText(parent) return parent:CreateFontString() end
function Style.EdLabel(parent, text, gold, ...) local fs = parent:CreateFontString(); fs:SetText(tostring(text):upper()); return fs end
function Style.EdInput(parent, w, h) local e = CreateFrame("EditBox", nil, parent); e.text = ""; return e end
function Style.EdChip(parent, text, w, onClick, ...)
    local b = CreateFrame("Button", nil, parent)
    b.w = w or (#tostring(text or "") * 7 + 22)
    b.edText = b:CreateFontString()
    b:SetText(text or "")
    b:SetScript("OnClick", onClick)
    b.SetActive = function(self, on) self.active = on and true or nil end
    chips[#chips + 1] = b
    return b
end

-- the Builder as BuildTools sees it: its state (Den Lite: caps with the tools), its commands, the gallery's pick
local said, swaps = {}, {}
local W
W = { Builder = { state = { lite = "1", can = { tool = true }, solid = "off", coll = "none", selId = 5 }, tab = "all",
    Command = function(t) said[#said + 1] = t; return true end, SetStatus = function(t) W.status = t end,
    Snap = function() return 1 end, SetSnap = function(g) W.snap = g end, TurnStep = function() return 45 end, SetAngle = function(a) W.angle = a end,
    Swap = function(all) swaps[#swaps + 1] = all; return true end, Paste = function() said[#said + 1] = "paste"; return true end,
    Deselect = function() said[#said + 1] = "deselect" end, Can = function(v) return v == "tool" end },
    DenBar = { active = true, play = false }, Style = Style }
W.Builder.state.lite = "1"
WrathUnleashedUI = W
SlashCmdList = SlashCmdList or {}
dofile(dir .. "/Gizmo.lua")
dofile(dir .. "/BuildTools.lua")
local T = W.BuildTools
local G = W.Gizmo
check("loaded", T and G and SlashCmdList.WUTOOLS)

local function last() return said[#said] end
local function answer(name, data)
    T.OnMessage("OVMOD", "wubuild\t" .. name .. "\t1/1\t" .. data, "WHISPER", "Wren")
end
local function chipText(c) return c and c.edText and c.edText.text or (c and c.text) end
local function find(text)
    for _, c in ipairs(chips) do if c.text == text and c.shown ~= false then return c end end
end

-- the window and its tabs
T.Show("select")
local win = named.WrathUnleashedToolsFrame
check("the window opens on Select, the other pages hidden", win and win.shown and win.pages.select.shown and not win.pages.path.shown)
local names = {}
for _, t in ipairs(T.TABS) do names[#names + 1] = t[2] end
check("the tabs: Select, Arrange, Path, Stamps, Layers, History", table.concat(names, ",") == "Select,Arrange,Path,Stamps,Layers,History")
check("Esc closes it (UISpecialFrames)", UISpecialFrames[#UISpecialFrames] == "WrathUnleashedToolsFrame")
check("the snap chips show the Builder's grid and turn step", win.pages.select.c.grid1.active and win.pages.select.c.angle45.active
    and not win.pages.select.c.grid2.active)
check("Solid's state and what the game does with its model, in words", win.pages.select.c.solidOff.active
    and win.pages.select.solidText.text:find("no collision"))
win.pages.select.c.solidOn:Fire("OnClick")
check("Solid sends solid on", last() == "solid on")
win.pages.select.c.land:Fire("OnClick")
check("Land: surface snap", last() == "land")
win.pages.select.c.swapkind:Fire("OnClick")
check("Replace: all of its kind with the gallery's pick (the Builder's Swap)", swaps[1] == true)

-- Box select: two clicks on the world, through Gizmo.lua's click
G.build = true
T.SetMode("box1")
cursor = { 480, 270 }
WorldFrame:Fire("OnMouseDown", "LeftButton"); clock = clock + 0.05; WorldFrame:Fire("OnMouseUp", "LeftButton")
check("the first click is a corner (no carry click sent)", T.mode == "box2" and #sent == 0)
cursor = { 1440, 810 }
WorldFrame:Fire("OnMouseDown", "LeftButton"); clock = clock + 0.05; WorldFrame:Fire("OnMouseUp", "LeftButton")
check("the second: the box to Den Lite, corners in screen units", last() == "tool box -0.5000 -0.5000 0.5000 0.5000 1.7778 12" and T.mode == nil, last())
check("and Gizmo.lua sent nothing of its own", #sent == 0)

-- the Path tool
T.Show("path")
local pp = win.pages.path
check("Path shows the gallery's pick to pick first", pp.pick.text:find("Pick one in the gallery"))
W.Builder.pick = { id = 19457, name = "Lamp Post" }
pp.c.lamps:Fire("OnClick")
check("Lamps: every 6, both sides, across, 3 off the line", T.lay.every == 6 and T.lay.sides == 3 and T.lay.face == "across" and T.lay.offset == 3
    and pp.c.side3.active and pp.c.across.active and pp.pick.text == "Lamp Post")
pp.c.add:Fire("OnClick")
check("Add points: waits for clicks, the stakes asked for", T.mode == "point" and last() == "path show")
cursor = { 960, 540 }
WorldFrame:Fire("OnMouseDown", "LeftButton"); clock = clock + 0.05; WorldFrame:Fire("OnMouseUp", "LeftButton")
check("a click: the point under the cursor", last() == "tool point 0.0000 0.0000 1.7778 12")
answer("pathpts", "2|0|9.0|16226.20,16257.00;16235.20,16257.00")
check("the points back: two, 9 yd", #T.pts == 2 and pp.ptsText.text == "2 points · 9 yd")
pp.idx:SetText("2")
pp.c.move:Fire("OnClick")
WorldFrame:Fire("OnMouseDown", "LeftButton"); clock = clock + 0.05; WorldFrame:Fire("OnMouseUp", "LeftButton")
check("Move point 2: the next click moves it (index from 0), then back to adding", last() == "tool point 0.0000 0.0000 1.7778 12 1" and T.mode == "point")
pp.c.undo:Fire("OnClick")
check("Undo point", last() == "path undo")
pp.c.lay:Fire("OnClick")
check("Lay: the pick, every, smooth, face, mid, sides, offset", last() == "path lay 19457 6 1 across 0 3 3" and T.mode == nil, last())
answer("pathedit", "7|190023|2|1|along|1|0|0")
check("a laid path to edit: its numbers back, the Path tab, Lay again", T.editing == 7 and T.lay.mid and T.lay.every == 2 and T.tab == "path"
    and pp.c.lay.text == "Lay again")
-- a right click stops a tool
T.SetMode("point")
WorldFrame:Fire("OnMouseDown", "RightButton"); clock = clock + 0.05; WorldFrame:Fire("OnMouseUp", "RightButton")
check("a right click stops the tool", T.mode == nil)

-- stamps: the library, its picture, the mouse
T.Show("stamps")
check("Stamps asks for the library", last() == "stamps")
answer("stamps", "garden,Garden,gardens,24,ours,;inn,Inn,buildings,34,ours,;porch,My porch,gardens,2,mine,Orion;")
local sp = win.pages.stamps
local cats = {}
for _, c in ipairs(sp.catChips) do if c.shown ~= false then cats[#cats + 1] = c.text end end
check("categories: All and each one there, Mine for yours", table.concat(cats, ",") == "All,Gardens,Buildings,Mine", table.concat(cats, ","))
check("the rows: name, pieces, yours", sp.rows[1].text == "Garden  (24)" and sp.rows[3].text == "My porch  (2 · yours)" and sp.rows[3].edLabel:find("by Orion"))
local items = T.StampItems()
check("the gallery's Stamps tab gets the library", #items == 3 and items[1].kind == "stamp" and items[1].id == "stamp:garden")
sp.rows[1]:Fire("OnClick")
check("a row picks it and asks for its picture", T.stampKey == "garden" and last() == "stampplan garden")
answer("stampplan", "garden|32,32,40,30,0;10,50,4,4,3")
check("its picture: a box per piece in the 128 square, coloured by kind", #sp.planTex == 2 and sp.planTex[1].w == 80 and sp.planTex[2].tex[2] > 0.6)
sp.c.mlr:Fire("OnClick"); sp.c.turnR:Fire("OnClick"); sp.c.bigger:Fire("OnClick")
check("turn, mirror and size before it goes down", T.xf.turn == 15 and T.xf.mirror == "lr" and T.xf.scale == 1.25 and sp.xfText.text:find("mirrored left to right"))
sp.c.place:Fire("OnClick")
check("Place with the mouse waits for a click", T.mode == "stamp")
cursor = { 960, 540 }
WorldFrame:Fire("OnMouseDown", "LeftButton"); clock = clock + 0.05; WorldFrame:Fire("OnMouseUp", "LeftButton")
check("the click: the stamp there, turned, mirrored and sized", last() == "tool stamp garden 0.0000 0.0000 1.7778 12 15 lr 1.25", last())
answer("staged", "garden|15|lr|1.25")
check("down and waiting: Enter keeps it, Esc takes it back", T.staged and bindings.ENTER == "LeylineToolsKeep" and bindings.ESCAPE == "LeylineToolsCancel")
sp.c.turnL:Fire("OnClick")
check("turned again while it waits: put down again", last() == "stamp again 0 lr 1.25")
named.LeylineToolsKeep:Fire("OnClick")
check("Enter keeps it: the keys given back, the mouse free", last() == "stamp keep" and not T.staged and bindings.ENTER == nil and T.mode == nil)
answer("staged", "garden|0|lr|1.25")
named.LeylineToolsCancel:Fire("OnClick")
check("Esc takes it back", last() == "stamp cancel")
sp.saveName:SetText("My Porch 2")
sp.saveTitle:SetText("Porch & steps")
T.cat = "gardens"
sp.c.save:Fire("OnClick")
check("save the selection as a stamp: its name made a file name, the category, the title", last() == "stamp save my-porch-2 gardens Porch  steps", last())
sp.saveName:SetText("bad/name")
local n = #said
sp.c.save:Fire("OnClick")
check("a bad name isn't sent", #said == n and W.status:find("letters, digits"))

-- layers, history, measure
T.Show("layers")
check("Layers asks for the groups", last() == "groups")
answer("groups", "3,Porch,2,0,1;7,garden,24,1,0;")
local lp = win.pages.layers
check("a row per group: name, count, Hide or Show, Lock or Unlock", lp.rows[1].text == "Porch  (2)" and lp.btns[1].lock.text == "Unlock"
    and lp.btns[2].hide.text == "Show" and not lp.rows[3].shown and not lp.empty.shown)
lp.btns[2].hide:Fire("OnClick")
check("Show a hidden layer", last() == "layer 7 show")
lp.rows[1]:Fire("OnClick")
check("a click selects the layer", last() == "layer 3 select")
lp.name:SetText("Fence run")
lp.c.group:Fire("OnClick")
check("group the selection by a name", last() == "group Fence run")
T.Show("history")
answer("history", "3|1|Placed 12;Changed Wooden Chair;Placed Barrel")
local hp = win.pages.history
check("history: the newest first, numbered", hp.rows[1].text == "1. Placed 12" and hp.rows[3].text == "3. Placed Barrel" and not hp.rows[4].shown)
hp.rows[2]:Fire("OnClick")
check("a click goes back to before that step", last() == "undoto 2")
T.Show("select")
answer("measure", "d=5.00;flat=4.00;up=3.00;w=1.00;dp=0.50;h=1.20;n=1")
check("measure in words", win.pages.select.measureText.text == "5.00 yd (4.00 on the flat, 3.00 up) · it is 1.0 x 0.5 x 1.2 yd")

-- arrays and rings
T.Show("arrange")
local ap = win.pages.arrange
ap.n:SetText("5"); ap.dx:SetText("2.5")
ap.c.array:Fire("OnClick")
check("an array: n copies a step apart", last() == "array 5 2.5 0 0")
ap.faces.out:Fire("OnClick")
ap.c.ringme:Fire("OnClick")
check("a ring round me, facing out", last() == "ring 8 5 out me" and ap.faces.out.active)
ap.c.abottom:Fire("OnClick")
check("bottoms lined up", last() == "align bottom")

-- other mods' messages and other people's are left alone
local before = #T.history
T.OnMessage("OVMOD", "wuden\thistory\t1/1\t1|0|x", "WHISPER", "Wren")
T.OnMessage("OVMOD", "wubuild\thistory\t1/1\t1|0|x", "WHISPER", "Someone")
check("only Den Lite's own whisper, only wubuild", #T.history == before)
-- a long answer in parts
T.OnMessage("OVMOD", "wubuild\tgroups\t1/2\t1,A,1,0,0;", "WHISPER", "Wren")
T.OnMessage("OVMOD", "wubuild\tgroups\t2/2\t2,B,1,0,0;", "WHISPER", "Wren")
check("a long answer put back together", #T.groups == 2 and T.groups[2].name == "B")

-- the words on screen
local bad = {}
for _, c in ipairs(chips) do
    for _, word in ipairs({ "world of warcraft", "warcraft", "lich king", "blizzard", "wow", "sign in" }) do
        local t = ((c.text or "") .. " " .. (c.edLabel or "")):lower()
        if t:find(word, 1, true) then bad[#bad + 1] = c.text end
    end
end
check("no game-brand words on screen", #bad == 0)
check("nothing went anywhere but the Builder's channel", #sent == 0)

print(string.format("%d/%d passed", pass, pass + fail))
if fail > 0 then os.exit(1) end
