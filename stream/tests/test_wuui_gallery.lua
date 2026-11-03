-- lua5.1 stream/tests/test_wuui_gallery.lua [addon dir]
-- The Den editor's gallery (Builder.lua, 2026-11-03; owner: "a gallery of spinning 3D objects that I can place"),
-- with a fake 3.3.5a API whose Model frames record SetModel, SetFacing and ClearModel, and catalogs sent straight to
-- the addon as the realm (ov_build.lua) and Den Lite (ovmod.rs) send them: 20 tiles made once and reused across
-- pages ("1-20 of 53"), the model path (the optional 6th field) set on the tile's Model frame (as .mdx, the client's
-- own way), the category's icon where there is none (a WMO, a bad path, an old 5-field catalog), a display id in the
-- 6th field, "catmore" adding to the catalog, search, A-Z, one click picks (the gold edge), a double-click and a drag
-- out place, Spin turns the models (off: they rest), the saved Spin, and no game-brand names on screen.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- fake widgets
local frames, created = {}, {}
local Frame = {}
Frame.__index = function(self, k)
    local v = rawget(Frame, k)
    if v ~= nil then return v end
    if type(k) == "string" and k:match("^%u") then return function() end end -- other widget methods: no-ops
    return nil
end
function Frame:SetScript(n, fn) self.scripts[n] = fn end
function Frame:GetScript(n) return self.scripts[n] end
function Frame:Fire(n, ...) local fn = self.scripts[n]; if fn then return fn(self, ...) end end
function Frame:Show()
    local was = self:IsVisible(); self.shown = true
    if not was and self:IsVisible() then self:Fire("OnShow") end
end
function Frame:Hide()
    local was = self:IsVisible(); self.shown = false
    if was then self:Fire("OnHide") end
end
function Frame:IsShown() return self.shown end
function Frame:IsVisible()
    if not self.shown then return false end
    return self.parent == nil or self.parent.top or self.parent:IsVisible()
end
function Frame:SetText(t) self.text = t; if self.scripts.OnTextChanged and self.kind == "EditBox" then self:Fire("OnTextChanged") end end
function Frame:GetText() return self.text end
function Frame:Enable() self.enabled = true end
function Frame:Disable() self.enabled = false end
function Frame:SetFocus() self.focus = true end
function Frame:ClearFocus() self.focus = false end
function Frame:RegisterEvent(e) self.events[e] = true end
function Frame:GetName() return self.name end
function Frame:GetParent() return self.parent end
function Frame:SetParent(p) self.parent = p end
function Frame:GetHeight() return self.h or 0 end
function Frame:SetHeight(h) self.h = h end
function Frame:GetWidth() return self.w or 0 end
function Frame:SetWidth(w) self.w = w end
function Frame:GetEffectiveScale() return self.escale or 1 end
function Frame:SetTexture(a, b, c, d) self.tex = b and { a, b, c, d } or a end
function Frame:IsMouseOver() return self.mouseOver == true end
-- Model frames
function Frame:SetModel(path) self.modelFile = path; self.setModelCalls = (self.setModelCalls or 0) + 1 end
function Frame:SetLight(on, ...) self.light = on end
function Frame:SetPosition(x, y, z) self.pos = { x, y, z } end
function Frame:SetModelScale(v) self.mscale = v end
function Frame:ClearModel() self.modelFile = nil end
function Frame:SetFacing(f) self.facing = f end
function Frame:SetCamera(c) self.camera = c end
local function new(kind, name, parent)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, events = {}, shown = true }, Frame)
    if name then _G[name] = f end
    created[#created + 1] = f
    return f
end
function Frame:CreateFontString() return new("FontString", nil, self) end
function Frame:CreateTexture() return new("Texture", nil, self) end
local models = 0
function CreateFrame(kind, name, parent)
    local f = new(kind, name, parent); frames[#frames + 1] = f
    if kind == "Model" then models = models + 1 end
    return f
end

UIParent = new("Frame", "UIParent"); UIParent.top = true
WorldFrame = new("Frame", "WorldFrame"); WorldFrame.top = true
GameMenuFrame = new("Frame", "GameMenuFrame"); GameMenuFrame.top = true; GameMenuFrame.shown = false
UISpecialFrames = {}
function tinsert(t, v) table.insert(t, v) end
SlashCmdList = {}
DEFAULT_CHAT_FRAME = { AddMessage = function() end }
local screenW = 1745
function GetScreenWidth() return screenW end
function GetScreenHeight() return 981 end
function InCombatLockdown() return false end
function SetBinding() return 1 end
function SetBindingClick() return 1 end
function SaveBindings() end
function SetCVar() end
function GetCVar() return nil end
function HasAction() return false end
function UnitLevel() return 80 end
function UnitClass() return "Mage", "MAGE" end
function UnitName() return "Tess" end
function GetRealmName() return "Elsewhere" end
function PlaySound() end
local clock = 100
function GetTime() return clock end
local sent = {}
function SendAddonMessage(prefix, msg, channel, target) sent[#sent + 1] = msg end
local function lastCmd() return (sent[#sent] or ""):match("^wubuild\tcmd\t1/1\t(.*)$") end
local function fireClient(msg)
    for _, f in ipairs(frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", "OVMOD", msg, "WHISPER", "Tess") end
    end
end

-- ---------------------------------------------------------------- load the addon in .toc order
local files = {}
for line in io.lines(dir .. "/WrathUnleashedUI.toc") do
    if line:match("%.lua%s*$") then files[#files + 1] = line:gsub("%s+$", "") end
end
for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
local W = assert(WrathUnleashedUI)
local B = assert(W.Builder)

-- ---------------------------------------------------------------- a catalog of 53: models, old lines, odd paths
local items = {}
for i = 1, 53 do
    local id = 1000 + i
    local name = string.format("Barrel %02d", 54 - i)   -- the realm's order is not A-Z
    local cat = (i % 2 == 0) and "barrels" or "crates"
    local line = string.format("%d,barrel%d,%s,%s,%d", id, i, cat, name, (i % 5 == 0) and 2 or 1)
    if i == 1 then line = line .. ",World\\Generic\\Human\\Passive Doodads\\Barrels\\Barrel01.m2"
    elseif i == 2 then line = line .. ",World/Generic/Human/Passive Doodads/Crates/Crate01.mdx"
    elseif i == 3 then line = line .. ",World\\Generic\\Human\\Passive Doodads\\Lights\\Lamp01"
    elseif i == 4 then line = line .. ",World\\wmo\\Azeroth\\Buildings\\Inn.wmo"
    elseif i == 5 then line = line .. ",..\\..\\evil.m2"
    elseif i == 6 then line = line .. ",4523"                                   -- catalog v2: a display id
    elseif i <= 30 then line = line .. string.format(",World\\Generic\\Props\\Prop%02d.m2", i) end   -- 31..53: old 5-field lines
    items[#items + 1] = line
end
local data = table.concat(items, ";")
-- in two parts, as the realm sends a long catalog
local half = math.floor(#data / 2)
W.ToggleBuilder()
local win = B.Window()
check("/builder opens the gallery and asks for the catalog", win:IsShown() and sent[1] == "wubuild\tcatalog\t1/1\t")
fireClient("wubuild\tcatalog\t1/2\t" .. data:sub(1, half))
fireClient("wubuild\tcatalog\t2/2\t" .. data:sub(half + 1))
check("53 props arrive", #B.catalog == 53)
check("20 Model frames, made once", models == 20 and #win.tiles == 20)

-- the model paths
local byId = {}
for _, c in ipairs(B.catalog) do byId[c.id] = c end
check("a .m2 path is given to SetModel as .mdx", byId[1001].model == "World\\Generic\\Human\\Passive Doodads\\Barrels\\Barrel01.mdx")
check("/ becomes \\ and .mdx stays", byId[1002].model == "World\\Generic\\Human\\Passive Doodads\\Crates\\Crate01.mdx")
check("no ending: .mdx", byId[1003].model == "World\\Generic\\Human\\Passive Doodads\\Lights\\Lamp01.mdx")
check("a WMO can't show in a tile: no model", byId[1004] and byId[1004].model == nil)
check("a path that climbs out (..) is refused", byId[1005] and byId[1005].model == nil)
check("a bare number is a display id (catalog v2), no path yet", byId[1006].display == 4523 and byId[1006].model == nil)
check("old 5-field lines still read, with their points", byId[1035] and byId[1035].model == nil and byId[1035].cost == 2 and byId[1035].name == "Barrel 19")

-- page 1: the tiles' Model frames
local function tileOf(id) for _, t in ipairs(win.tiles) do if t:IsShown() and t.item and t.item.id == id then return t end end end
check("1-20 of 53", win.count:GetText() == "1-20 of 53")
local t1 = tileOf(1001)
check("a tile with a model: SetModel(path), shown, the icon hidden", t1.model.modelFile == byId[1001].model and t1.model:IsShown() and not t1.icon:IsShown())
check("a tile's model is lit (a Model frame without a light draws black)", t1.model.light == 1)
-- the framing (the old wiki's Model:SetPosition: (0,0,0) the frame's bottom-left, one unit the screen's diagonal)
local diag = math.sqrt(1745 ^ 2 + 981 ^ 2)
local TW = 88 / diag
check("a model's tile: in the tile's middle, the frame's own units (about 0.044 across)", t1.model.pos
    and math.abs(t1.model.pos[1] - TW / 2) < 1e-9 and t1.model.pos[3] == 0 and t1.model.mscale < TW, table.concat(t1.model.pos or {}, ","))
B.SetFit(0.5, 0.25, 0, 0.5, "height", "1", 0)
local s1 = t1.model.mscale
check("SetFit re-frames the tiles shown: by a part of the tile, the unit, positions over the scale, a camera",
    t1.model.pos and math.abs(t1.model.pos[1] - 0.75 * (88 / 981) / s1) < 1e-9 and math.abs(t1.model.pos[3] - 0.5 * (88 / 981) / s1) < 1e-9
    and t1.model.camera == 0 and WrathUnleashedUIDB.den.fit.fill == 0.5 and WrathUnleashedUIDB.den.fit.scaledPos == true)
B.SetFit("reset")
check("SetFit reset: the default again", WrathUnleashedUIDB.den.fit == nil and math.abs(t1.model.pos[1] - TW / 2) < 1e-9)
-- framed again once laid out: before its frame has a size there is nothing to frame by; shown or sized, it is framed
-- on the next frames, and again when the screen changes
do
    local m = t1.model
    m.pos = nil; m.w = 0
    B.RefitSoon(t1)
    check("no size yet: not framed (nothing to frame by)", m.pos == nil)
    m.w = 88
    win.grid:Fire("OnUpdate", 0.01)
    check("the next frame, laid out: framed", m.pos and math.abs(m.pos[1] - TW / 2) < 1e-9)
    m.pos = nil
    win.grid:Fire("OnUpdate", 0.01)
    check("and once more the frame after (the model loads late)", m.pos ~= nil)
    m.pos = nil
    win.grid:Fire("OnUpdate", 0.01)
    check("then left alone while nothing changes", m.pos == nil)
    m:Hide(); m:Show()
    check("shown again: framed on show", m.pos ~= nil)
    m.pos = nil
    m.w, m.h = 120, 120; m:Fire("OnSizeChanged", 120, 120)
    check("resized: framed by its new size", m.pos and math.abs(m.pos[1] - 60 / diag) < 1e-9)
    m.w, m.h = 88, 88
    win.grid:Fire("OnUpdate", 0.01); win.grid:Fire("OnUpdate", 0.01); win.grid:Fire("OnUpdate", 0.01)
    m.pos = nil
    screenW = 1365
    win.grid:Fire("OnUpdate", 0.01)
    local W2 = 88 / math.sqrt(1365 ^ 2 + 981 ^ 2)
    check("the screen's size or UI scale changed: framed again", m.pos and math.abs(m.pos[1] - W2 / 2) < 1e-9)
    screenW = 1745
    win.grid:Fire("OnUpdate", 0.01)
end
local t4 = tileOf(1004)
check("a WMO's tile: the building icon on purpose, and why", not t4.model:IsShown() and t4.icon:IsShown() and t4.icon.path == "Interface\\Icons\\INV_Misc_Rune_01"
    and t4.why:GetText() == "building")
check("the name under the tile", t1.name:GetText() == "Barrel 53")
check("a prop of 2 points says so on its tile", tileOf(1005).cost:GetText() == "2" and t1.cost:GetText() == "")
local m1 = t1.model
win.next:Fire("OnClick")
check("Next: 21-40 of 53", win.count:GetText() == "21-40 of 53" and tileOf(1021) ~= nil)
check("the same 20 frames, reused (no new Model frame)", models == 20 and win.tiles[1].model == m1)
check("a reused tile's model is the new prop's", win.tiles[1].model.modelFile == byId[1021].model)
win.next:Fire("OnClick")
check("41-53 of 53, the rest of the tiles hidden", win.count:GetText() == "41-53 of 53" and win.tiles[13]:IsShown() and not win.tiles[14]:IsShown())
check("the old lines' tiles: icons, and the reused frames' models cleared", tileOf(1041) and not tileOf(1041).model:IsShown()
    and tileOf(1041).model.modelFile == nil)
check("Next is off on the last page", win.next.enabled == false)
win.prev:Fire("OnClick"); win.prev:Fire("OnClick")

-- search, A-Z
win.search:SetText("barrel 0"); win.search:Fire("OnTextChanged")
check("search: Barrel 01-09", #B.shown == 9 and win.count:GetText() == "1-9 of 9")
win.search:SetText(""); win.search:Fire("OnTextChanged")
win.az:Fire("OnClick")
check("A-Z: by name", B.az and win.az:IsActive() and win.tiles[1].item.name == "Barrel 01" and win.tiles[2].item.name == "Barrel 02")
win.az:Fire("OnClick")
check("A-Z off: the realm's order", not B.az and win.tiles[1].item.id == 1001)
for _, t in ipairs(win.tabs) do if t.tab == "barrels" then t:Fire("OnClick") end end
check("a category: only its props", #B.shown == 26 and win.title:GetText() == "BARRELS")
for _, t in ipairs(win.tabs) do if t.tab == "all" then t:Fire("OnClick") end end

-- selection: one click, the gold edge
local gold = W.Style.ED.gold
local function goldEdge(t) local e = t.edges[1].tex; return type(e) == "table" and e[1] == gold[1] and e[2] == gold[2] end
win.tiles[2]:Fire("OnClick", "LeftButton")
check("a click picks it: the gold edge, Place names it", B.pick == win.tiles[2].item and goldEdge(win.tiles[2]) and not goldEdge(win.tiles[1])
    and win.spawn:GetText() == "Place " .. win.tiles[2].item.name)
check("one click sends nothing", lastCmd() == nil)
win.tiles[3]:Fire("OnClick", "LeftButton")
check("another click moves the edge", goldEdge(win.tiles[3]) and not goldEdge(win.tiles[2]))
win.tiles[3]:Fire("OnEnter")
check("hover: the tile lights and the status says how", win.tiles[3].hover:IsShown() and B.status:find("Double%-click"))
win.tiles[3]:Fire("OnLeave")
win.tiles[3]:Fire("OnDoubleClick")
check("a double-click places it at your feet", lastCmd() == "spawn " .. win.tiles[3].item.id)
win.tiles[5]:Fire("OnDragStart")
win.mouseOver = true
win.tiles[5]:Fire("OnDragStop")
check("a drag let go inside the window places nothing", lastCmd() == "spawn " .. win.tiles[3].item.id)
win.mouseOver = false
win.tiles[5]:Fire("OnDragStart")
win.tiles[5]:Fire("OnDragStop")
check("dragged out of the window: placed", lastCmd() == "spawn " .. win.tiles[5].item.id)
win.tiles[1]:Fire("OnClick", "RightButton")
check("right-click: a favourite, a gold star", WrathUnleashedUIChar.builder.favs[1001] == true and win.tiles[1].fav:GetText() == "*")

-- spin
check("Spin is on by default", B.spin and win.spinBtn:IsActive())
local f0 = win.tiles[1].model.facing
win.grid:Fire("OnUpdate", 0.5)
local f1 = win.tiles[1].model.facing
check("each frame turns the models", f0 and f1 and math.abs(f1 - f0) > 0.1 and win.tiles[2].model.facing == f1)
check("a tile without a model isn't turned", win.tiles[4].model.facing == nil or win.tiles[4].model.facing ~= f1)
win.spinBtn:Fire("OnClick")
local rest = win.tiles[1].model.facing
win.grid:Fire("OnUpdate", 0.5)
check("Spin off: the models rest at a three-quarter view", not B.spin and not win.spinBtn:IsActive() and win.tiles[1].model.facing == rest
    and math.abs(rest - 0.6) < 1e-6)
check("and it is kept per character", WrathUnleashedUIChar.builder.spin == false)
win.spinBtn:Fire("OnClick")

-- catmore: more of the catalog (Den Lite sends what one message can't carry)
fireClient("wubuild\tcatmore\t1/1\t2001,lamp,lights,Lamp,1,World\\Generic\\Lamp.m2;1001,barrel1,barrels,Duplicate,1")
check("catmore adds to the catalog, an id twice once", #B.catalog == 54 and byId[1001].name == "Barrel 53")
-- an old realm: a 5-field catalog replaces it, and the gallery still works
fireClient("wubuild\tcatalog\t1/1\t2413,chair,seating,Wooden Chair,1;181075,table,tables,Table,2")
check("an old 5-field catalog: two props, icons only", #B.catalog == 2 and win.count:GetText() == "1-2 of 2"
    and not win.tiles[1].model:IsShown() and win.tiles[1].icon.path == "Interface\\Icons\\Spell_Nature_Sleep")
win.tiles[2]:Fire("OnDoubleClick")
check("and places as before", lastCmd() == "spawn 181075")
-- found objects carry a model too
for _, t in ipairs(win.tabs) do if t.tab == "objects" then t:Fire("OnClick") end end
fireClient("wubuild\tfound\t1/1\tobjects,1,0;500006,t500006,objects,Garden Bench,1,World\\Generic\\Bench.mdx")
check("a found object's tile shows its model", win.tiles[1].item.name == "Garden Bench" and win.tiles[1].model.modelFile == "World\\Generic\\Bench.mdx")

-- the status pill
check("the status pill: Place, the pick, snap, the turn step", _G.LeylineEditorStatus:IsShown() and _G.LeylineEditorStatus.line:GetText():find("^Place · "))

-- nothing on screen names the game or its maker
local bad
for _, f in ipairs(created) do
    local t = type(f.text) == "string" and f.text or nil
    if t and (t:lower():find("warcraft") or t:lower():find("%f[%w]wow%f[%W]") or t:lower():find("lich king") or t:lower():find("blizzard")) then bad = t end
end
check("no game-brand names on screen", bad == nil)

print(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
