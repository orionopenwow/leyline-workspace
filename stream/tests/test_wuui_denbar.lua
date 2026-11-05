-- lua5.1 stream/tests/test_wuui_denbar.lua [addon dir]
-- Den mode (DenBar.lua; owner 1 Nov 23:46: "There was no interface to deal with the den ... make it make sense, not
-- spam the chat"). Fake 3.3.5a API (as test_wuui_denhome.lua) and every file of WrathUnleashedUI in .toc order, with
-- a fake Den Lite answering the bar's addon messages as labs/den-lite notice.rs / ovmod.rs do: on entering Your Den
-- the bar opens by itself and says hello; the first time the four intro cards show (Next / Skip, remembered in the
-- saved variables, again from Help); Build opens the Builder; Shape ground, Save (a name) and Invite send the Den's
-- commands; the invite link lands in the copy box; toasts show one at a time, then "Saved 12 s ago"; play mode hides
-- every editor window and back (F8, the pill, Esc > Back to building); F7 / F8 are bound only in the Den and only
-- when free; elsewhere no bar, no keys; nothing goes to the chat; no game-brand names on screen. 2026-11-03: the slim
-- strip (Undo n / Redo n, Menu, Snap, 15 / 45 / 90, Files, x), the tool rail and the status pill.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- fake widgets (as test_wuui_denhome.lua)
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
function Frame:HookScript(n, fn)
    local old = self.scripts[n]
    self.scripts[n] = function(...) if old then old(...) end; return fn(...) end
end
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
function Frame:UnregisterEvent(e) self.events[e] = nil end
function Frame:GetName() return self.name end
function Frame:GetParent() return self.parent end
function Frame:GetHeight() return self.h or 0 end
function Frame:SetHeight(h) self.h = h end
local function new(kind, name, parent)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, events = {}, shown = true }, Frame)
    if name then _G[name] = f end
    created[#created + 1] = f
    return f
end
function Frame:GetEffectiveScale() return 1 end
function Frame:CreateFontString() return new("FontString", nil, self) end
function Frame:CreateTexture() return new("Texture", nil, self) end
function CreateFrame(kind, name, parent, template)
    local f = new(kind, name, parent); frames[#frames + 1] = f
    if template == "OptionsSliderTemplate" and name then
        new("FontString", name .. "Low", f); new("FontString", name .. "High", f)
    end
    return f
end

UIParent = new("Frame", "UIParent"); UIParent.top = true
WorldFrame = new("Frame", "WorldFrame"); WorldFrame.top = true
function UIParent:GetScale() return 1 end
GameMenuFrame = new("Frame", "GameMenuFrame"); GameMenuFrame.top = true; GameMenuFrame.h = 300
GameMenuFrame.shown = false
UISpecialFrames = {}
function tinsert(t, v) table.insert(t, v) end
SlashCmdList = {}
local chatLines = {}
DEFAULT_CHAT_FRAME = { AddMessage = function(_, m) chatLines[#chatLines + 1] = m end }
function GetScreenHeight() return 981 end
function InCombatLockdown() return false end
function SetBinding() return 1 end
function SetBindingClick() return 1 end
function SaveBindings() end
local cv = {}
function GetCVar(k) return cv[k] end
function SetCVar(k, v) cv[k] = tostring(v) end
local overrides = {}
function SetOverrideBindingClick(owner, prio, key, button) overrides[key] = button end
function ClearOverrideBindings() overrides = {} end
local bound = {}   -- the player's own bindings
function GetBindingAction(key) return bound[key] or "" end
function GetUnitSpeed() return 0 end
function HasAction() return false end
function UnitLevel() return 1 end
function UnitClass() return "Warrior", "WARRIOR" end
function UnitName() return "Wren" end
function PlaySound() end
function HideUIPanel(f) f:Hide() end
local realm = "Your Den"
function GetRealmName() return realm end
local clock = 100
function GetTime() return clock end

-- ---------------------------------------------------------------- a fake Den Lite (notice.rs, ovmod.rs)
local function fireClient(msg)
    for _, f in ipairs(frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", "OVMOD", msg, "WHISPER", "Wren") end
    end
end
local sent = {}
local denLite = true   -- false: another server, nobody answers
function SendAddonMessage(prefix, msg, channel, target)
    sent[#sent + 1] = msg
    if not denLite or prefix ~= "OVMOD" or target ~= "Wren" then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= "wuden" then return end
    if name == "hello" then
        fireClient("wuden\tstatus\t1/1\trole=owner;props=0;saved=-")
        fireClient("wuden\ttoast\t1/1\tcheer|Welcome home. This room is yours: build anything.")
        fireClient("wuden\ttoast\t1/1\tjaina|I'm with you.")
    elseif name == "cmd" then
        local w = data:match("^(%S+)")
        if w == "save" then
            local n = data:match("^save (%S+)")
            fireClient("wuden\tsaved\t1/1\t" .. n)
            fireClient("wuden\ttoast\t1/1\tinfo|Saved to a file (0 props).")
        elseif w == "invite" then
            fireClient("wuden\ttoast\t1/1\tinfo|Making an invite link for one friend...")
        elseif w == "forge" then
            fireClient("wuden\ttoast\t1/1\tinfo|Opening Forge to shape your Den's ground...")
        end
    end
end
local function sentHas(text) for _, m in ipairs(sent) do if m == text then return true end end end
local function fire(event, ...)
    for _, f in ipairs(frames) do if f.events[event] then f:Fire("OnEvent", event, ...) end end
end
local function tick(secs)
    clock = clock + secs
    for _, f in ipairs(frames) do
        local fn = f.scripts.OnUpdate
        if fn and f.parent == WorldFrame then fn(f, secs) end
    end
end

-- ---------------------------------------------------------------- load the addon in .toc order
local files = {}
for line in io.lines(dir .. "/WrathUnleashedUI.toc") do
    if line:match("%.lua%s*$") then files[#files + 1] = line:gsub("%s+$", "") end
end
local at = {}
for i, f in ipairs(files) do at[f] = i end
check(".toc lists DenBar.lua after DenHome.lua, EditMode.lua and Builder.lua", at["DenBar.lua"] and at["DenBar.lua"] > at["DenHome.lua"]
    and at["DenBar.lua"] > at["EditMode.lua"] and at["DenBar.lua"] > at["Builder.lua"] and at["DenBar.lua"] > at["Style.lua"])
for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
local W = assert(WrathUnleashedUI)
local D = assert(W.DenBar, "W.DenBar")
check("nothing shown before entering the world", not D.IsActive() and not (LeylineDenBar and LeylineDenBar:IsShown()))

-- ---------------------------------------------------------------- 1. entering Your Den: the bar opens by itself
fire("PLAYER_ENTERING_WORLD")
local bar = D.Bar()
check("in Your Den the bar opens by itself", D.IsActive() and bar:IsShown())
check("and says hello to Den Lite", sentHas("wuden\thello\t1/1\t" .. D.VERSION))
check("Den Lite's status arrived", D.status and D.status.role == "owner" and D.status.props == "0")
check("the slim strip: Undo / Redo, Menu, Snap, the turn steps, Play, ?, Files, Invite, x",
    bar.undo:GetText() == "Undo 0" and bar.redo:GetText() == "Redo 0" and bar.menuBtn:GetText() == "Menu"
    and bar.snap:GetText() == "Snap: off" and bar.rot[15] and bar.rot[45] and bar.rot[90] and bar.rot[15]:IsActive()
    and bar.playBtn:GetText() == "Play (F7)" and bar.help:GetText() == "?" and bar.save:GetText() == "Files"
    and bar.invite:GetText() == "Invite" and bar.close:GetText() == "x" and bar.title:GetText() == "BUILD")
check("build mode's mark: a thin gold edge round the screen", _G.LeylineDenBuildEdge:IsShown() and #_G.LeylineDenBuildEdge.lines == 4)
check("the tool rail: Select, Build (F7), Move, Rotate, Stamps, Path, Ground, Dojo, View (snap out); no step stack (owner's shot)", D.Rail():IsShown()
    and bar.view and bar.view:GetText() == "View" and bar.view.edLabel:find("Snap out")
    and bar.select:GetText() == "Select" and bar.build:GetText() == "Build" and bar.move:GetText() == "Move"
    and bar.rotate:GetText() == "Rotate" and bar.ground:GetText() == "Shape ground" and bar.stamps:GetText() == "Stamps"
    and bar.path:GetText() == "Path" and #bar.steps == 0 and bar.build.iconPath:find("^Interface\\AddOns\\WrathUnleashedUI\\Media\\Icons\\", 1, false) and not bar.select.iconPath:find("Interface\\Icons", 1, true)
    and bar.ground.edText:GetText() == "Ground" and bar.build.edLabel:find("gallery"))
check("square icon buttons of one size on the rail", (function()
    for _, k in ipairs({ "select", "build", "move", "rotate", "ground", "stamps", "path" }) do if not bar[k].edIcon then return false end end
    return true end)())
check("the status pill at the bottom", _G.LeylineEditorStatus and _G.LeylineEditorStatus:IsShown()
    and _G.LeylineEditorStatus.line:GetText():find("^Place · snap off · 15°"))
check("F7 and F8 bound in the Den (free: a stock game binds neither)", overrides.F7 == "LeylineDenKeyBuild" and overrides.F8 == "LeylineDenKeyPlay")

-- the intro, the first time
local intro = D.Intro()
check("the first time: the intro's first card", intro and intro:IsShown() and intro.head:GetText() == "This is your Den"
    and intro.body:GetText():find("empty white world that's yours") and intro.count:GetText() == "1 / 5")
check("four short cards and the keys", #D.CARDS == 5 and (function()
    for i, c in ipairs(D.CARDS) do if (i < 5 and #c[2] > 160) or #c[2] > 220 or #c[1] > 20 then return false end end
    return D.CARDS[2][1] == "Build mode" and D.CARDS[2][2]:find("F7") and D.CARDS[3][1] == "Shape ground" and D.CARDS[4][2]:find("Invite") and D.CARDS[5][1] == "Keys"
end)())
intro.next:Fire("OnClick")
check("Next: Build mode", intro.head:GetText() == "Build mode" and intro.count:GetText() == "2 / 5")
intro.next:Fire("OnClick"); intro.next:Fire("OnClick"); intro.next:Fire("OnClick")
check("the last card (the keys): Start building, no Skip", intro.head:GetText() == "Keys" and intro.next:GetText() == "Start building" and not intro.skip:IsShown())
intro.next:Fire("OnClick")
check("done: the intro goes and is remembered", not intro:IsShown() and WrathUnleashedUIDB.den.introSeen == true)

-- toasts: one at a time, then the saved line
check("the welcome toast first", bar.line:GetText():find("Welcome home") ~= nil)
tick(D.TOAST_SECS + 0.1)
check("then Jaina's one greeting, as a toast", bar.line:GetText():find("Jaina: I'm with you") ~= nil)
tick(D.TOAST_SECS + 0.1)
check("then the Den's own line", bar.line:GetText():find("saves itself") ~= nil)
check("the same toast twice in a row shows once", D.Toast("info", "Hi.") and not D.Toast("info", "Hi."))
tick(D.TOAST_SECS + 0.1)

-- ---------------------------------------------------------------- 2. the buttons
bar.build:Fire("OnClick")
local B = W.Builder
check("Build opens the Builder (place, move, turn, delete, the catalog with search)", B.Window() and B.Window():IsShown())
bar.ground:Fire("OnClick")
check("Shape ground asks the launcher for Forge (.den forge)", sentHas("wuden\tcmd\t1/1\tforge"))
check("its answer is a toast in the bar, and in the Builder's own line", bar.line:GetText():find("Opening Forge") ~= nil and B.status:find("Opening Forge") ~= nil)
tick(D.TOAST_SECS + 0.1)

bar.save:Fire("OnClick")
local files = W.Builder.Files()
check("Files: this Den's save by name, builds, prefabs and the plot", files:IsShown() and files.den:IsShown() and files.den:GetText() == "my-den"
    and files.code and files.prefab and files.claim)
files.den:SetText("castle by the lake")
files.denSave:Fire("OnClick")
check("a name with spaces is made a plain name and saved", sentHas("wuden\tcmd\t1/1\tsave castle-by-the-lake"))
check("the bar hears it: Saved as ...", D.saved and D.saved.name == "castle-by-the-lake")
files.close:Fire("OnClick")
tick(D.TOAST_SECS + 0.1)
tick(12)
check("then: Saved as castle-by-the-lake 12 s ago (the line counts on)", bar.line:GetText():find("Saved as castle%-by%-the%-lake 1%d s ago") ~= nil)
D.OpenSave()
local dr = D.Drawer()
check("Save by the drawer too: a name box under the strip", dr:IsShown() and D.drawerKind == "save" and dr.box.focus and dr.box:GetText() == "castle-by-the-lake")
dr.box:SetText("no;way")
check("a bad name: not sent, says why", not D.Save(dr.box:GetText()) and bar.line:GetText():find("letters, numbers") ~= nil)
D.CloseDrawer()
tick(D.TOAST_SECS + 0.1)
fireClient("wuden\tsaved\t1/1\tauto")
check("the autosave: Saved 0 s ago", bar.line:GetText():find("Saved 0 s ago") ~= nil)

bar.invite:Fire("OnClick")
check("Invite asks for the link (.den invite) and opens the copy box", sentHas("wuden\tcmd\t1/1\tinvite") and dr:IsShown() and D.drawerKind == "invite")
local link = "https://relay.openvanilla.app/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ~~GABCDE2345.ABCDEFGH23456789"
fireClient("wuden\tinvite\t1/1\t" .. link)
check("the link lands in the copy box, focused to copy", dr.box:GetText() == link and dr.box.focus and dr.label:GetText() == "INVITE ONE FRIEND")
dr.box:SetText(link .. "x")
check("a key typed doesn't change the link", dr.box:GetText() == link)
fireClient("wuden\tinvite\t1/1\tjavascript:alert(1)")
check("anything that isn't an invite link is ignored", D.link == link)
local den = W.EditMode.DenPanel()
check("the hidden Den panel doesn't take the keyboard", not (den and den.link.focus))
dr.close:Fire("OnClick")

bar.help:Fire("OnClick")
check("?: the keys card", intro:IsShown() and intro.count:GetText() == "5 / 5" and intro.head:GetText() == "Keys")
intro.next:Fire("OnClick")
check("Skip closes it", not intro:IsShown())

-- ---------------------------------------------------------------- 2b. the editor's frame (2026-11-03)
local Bd = W.Builder
bar.menuBtn:Fire("OnClick")
local mn = D.Menu()
check("Menu: a list under the strip, the budget on top", mn and mn:IsShown() and mn.meter:GetText() ~= nil and #mn.rows == #D.MENU)
check("the menu holds what the old rows of buttons had", (function()
    local want = { "Select mine", "Copy", "Paste", "Duplicate", "Delete", "Group", "Ungroup", "Put away", "Storage", "Drop to the floor",
        "Properties", "Den panel", "Den home", "Reload the props", "Keys and tips" }
    local have = {}
    for _, r in ipairs(mn.rows) do have[r:GetText()] = r:IsShown() end
    for _, w in ipairs(want) do if not have[w] then return false end end
    return have["Box select (edit mode)"] == false end)())
local nm = #sent
for _, r in ipairs(mn.rows) do if r:GetText() == "Select mine" then r:Fire("OnClick") end end
check("a menu row does it and the menu closes", sent[nm + 1] == "wubuild\tcmd\t1/1\tselall" and not mn:IsShown())
bar.snap:Fire("OnClick")
local sm = D.SnapMenu()
check("Snap: off, 0.5, 1, 2, 5 yd", sm:IsShown() and #sm.rows == 5 and sm.rows[1]:GetText() == "off" and sm.rows[5]:GetText() == "5 yd")
sm.rows[3]:Fire("OnClick")
check("Snap 1 yd: the grid at your feet and edit mode's grid", bar.snap:GetText() == "Snap: 1 yd" and sentHas("wubuild\tcmd\t1/1\tgrid 1")
    and cv.leylineEditGrid == "1" and not sm:IsShown())
bar.rot[45]:Fire("OnClick")
check("45°: the turn step for Q / E and the popover", Bd.TurnStep() == 45 and bar.rot[45]:IsActive() and not bar.rot[15]:IsActive()
    and cv.leylineEditAngle == "45")
bar.rot[15]:Fire("OnClick")
fireClient("wubuild\tstate\t1/1\tsel=-;mine=0/200;area=Your Den;code=-;n=0;pos=-;undo=3;redo=1")
check("Undo 3 / Redo 1: the realm's counts", bar.undo:GetText() == "Undo 3" and bar.redo:GetText() == "Redo 1")
nm = #sent
bar.undo:Fire("OnClick")
check("Undo sends undo", sent[nm + 1] == "wubuild\tcmd\t1/1\tundo")
bar.move:Fire("OnClick")
check("Move: the selection's popover, the tool lit gold", Bd.Panel():IsShown() and bar.move:IsActive() and Bd.mode == "move")
bar.rotate:Fire("OnClick")
check("Rotate: the same popover", Bd.Panel():IsShown() and bar.rotate:IsActive() and not bar.move:IsActive())
bar.stamps:Fire("OnClick")
check("Stamps: the gallery on its Stamps tab", Bd.Window():IsShown() and Bd.tab == "stamps" and bar.stamps:IsActive())
nm = #sent
bar.select:Fire("OnClick")
check("Select on your own game: the prop nearest you", sent[nm + 1] == "wubuild\tcmd\t1/1\tnear" and bar.select:IsActive())
Bd.ShowTab("all")
Bd.ShowProps(false)
bar.close:Fire("OnClick")
check("x in the Den: play mode (the tools away)", D.play == true and not bar:IsShown() and not D.Rail():IsShown())
D.SetPlay(false)
check("and back: the strip and the rail", bar:IsShown() and D.Rail():IsShown())

-- ---------------------------------------------------------------- 3. play mode
B.ShowBuilder(true)
W.EditMode.ShowDen(true)
W.DenHome.Show(true)
B.ShowProps(true)
-- one right-hand inspector (DenLayout.lua, owner 5 Nov): the properties, opened last, put Den home away until they close
check("before: the Builder, the Den panel and the properties are open; Den home waits for the properties", B.Window():IsShown()
    and W.EditMode.DenPanel():IsShown() and B.Panel():IsShown() and (not W.DenLayout or (not W.DenHome.Panel():IsShown()
    and W.DenLayout.Waiting(W.DenHome.Panel()))))
_G.LeylineDenKeyPlay:Fire("OnClick", "LeftButton", true)
_G.LeylineDenKeyPlay:Fire("OnClick", "LeftButton", false)
check("F8: play mode (one press, down and up, counts once)", D.play == true)
check("every editor window goes", not B.Window():IsShown() and not W.EditMode.DenPanel():IsShown() and not W.DenHome.Panel():IsShown()
    and not B.Panel():IsShown())
check("play mode: no strip, rail, pill or edge; the corner chip and the play bar (Dojo, Class, Invite, Files)", not bar:IsShown()
    and not D.Rail():IsShown() and not _G.LeylineEditorStatus:IsShown() and not _G.LeylineDenBuildEdge:IsShown() and bar.pill:IsShown()
    and bar.pill.back:GetText() == "Your Den · Build (F7)" and bar.pill.play.dojo and bar.pill.play.class and bar.pill.play.invite
    and bar.pill.play.files)
check("the mode's banner: Play mode", _G.LeylineDenModeBanner:IsShown() and _G.LeylineDenModeBanner.label:GetText() == "Play mode")
bar.pill.play.class:Fire("OnClick")
check("Class: the ten classes", _G.LeylineDenClassMenu:IsShown() and #_G.LeylineDenClassMenu.rows == 10
    and _G.LeylineDenClassMenu.rows[6]:GetText() == "Death Knight")
local nc = #sent
_G.LeylineDenClassMenu.rows[8]:Fire("OnClick")
check("a class: .den class mage (the Den's command), the list closed", sent[nc + 1] == "wuden\tcmd\t1/1\tclass mage"
    and not _G.LeylineDenClassMenu:IsShown())
check("a toast says how back", (function()
    for _, t in ipairs(D.toasts) do if t.text:find("Back to building") then return true end end
    return bar.pill.line:GetText():find("Back to building") ~= nil end)())
local mb = D.MenuButton()
check("Esc menu: Back to building", mb and mb:IsShown() and mb:GetText() == "Back to building")
GameMenuFrame:Show()
mb:Fire("OnClick")
check("from the Esc menu: back to building, the menu closed", D.play == false and bar:IsShown() and not bar.pill:IsShown() and not GameMenuFrame:IsShown()
    and mb:GetText() == "Play your Den")
bar.playBtn:Fire("OnClick")
check("the Play button too", D.play == true)
bar.pill.back:Fire("OnClick")
check("and the pill's button back", D.play == false and bar:IsShown())
D.SetPlay(true)
_G.LeylineDenKeyBuild:Fire("OnClick", "LeftButton", true)
check("F7 in play mode: back to building, the Builder opens, the banner says Build mode", D.play == false and B.Window():IsShown()
    and _G.LeylineDenModeBanner.label:GetText() == "Build mode" and _G.LeylineDenBuildEdge:IsShown())
_G.LeylineDenKeyBuild:Fire("OnClick", "LeftButton", true)
check("F7 in build mode: play mode (one key both ways)", D.play == true)
_G.LeylineDenKeyBuild:Fire("OnClick", "LeftButton", true)

-- ---------------------------------------------------------------- 4. keys only when free, and nowhere else
bound.F8 = "TOGGLEAUTORUN"
D.BindKeys()
check("F8 taken by the player: Ctrl+F8 (F7 still the mode's key, the button says F7)", overrides.F8 == nil and overrides["CTRL-F8"] == "LeylineDenKeyPlay"
    and bar.playBtn:GetText() == "Play (F7)")
bound.F8 = nil

realm = "Wrath Unleashed"
denLite = false
fire("PLAYER_ENTERING_WORLD")
check("on another realm: no bar, no keys, no Esc button", not D.IsActive() and not bar:IsShown() and not bar.pill:IsShown() and next(overrides) == nil
    and not mb:IsShown())
check("and play mode can't be turned on there", D.SetPlay(true) == false)
W.ToggleBuilder()
check("elsewhere, the Builder brings the frame: BUILD, no Den buttons", bar:IsShown() and D.Rail():IsShown() and bar.title:GetText() == "BUILD"
    and not bar.invite:IsShown() and not bar.playBtn:IsShown() and not bar.ground:IsShown())
bar.close:Fire("OnClick")
check("and x shuts it all", not W.Builder.Window():IsShown() and not bar:IsShown() and not D.Rail():IsShown())
realm = "Your Den"
denLite = true
fire("PLAYER_ENTERING_WORLD")
check("back in the Den: the bar again, no intro (seen)", D.IsActive() and bar:IsShown() and not intro:IsShown())

-- ---------------------------------------------------------------- 5. quiet and plain
check("the bar wrote nothing to the chat", #chatLines == 0 or (function()
    for _, l in ipairs(chatLines) do if l:find("Den") then return false end end
    return true
end)())
local brands = { "warcraft", "lich king", "blizzard", "world of" }
local function onScreen()
    local out = {}
    for _, f in ipairs(created) do if type(f.text) == "string" then out[#out + 1] = f.text:lower() end end
    for _, c in ipairs(D.CARDS) do out[#out + 1] = (c[1] .. " " .. c[2]):lower() end
    return out
end
check("no game-brand names on screen", (function()
    for _, t in ipairs(onScreen()) do
        for _, b in ipairs(brands) do if t:find(b, 1, true) then return false end end
        if t:find("%f[%a]wow%f[%A]") then return false end
    end
    return true
end)())
check("toasts are plain text (colour codes and links taken out)", D.Toast("info", "|cffff0000red|r |Hitem:1|h[x]|h") and (function()
    tick(D.TOAST_SECS + 0.1)
    for _ = 1, 6 do if D.current and D.current.text:find("red") then return D.current.text == "red [x]" end; tick(D.TOAST_SECS + 0.1) end
    return false
end)())

-- the den-test's before/after (3 Nov: still no walking): Den Lite asks, the addon answers with its whole view
do
    local n0 = #sent
    fireClient("wuden\tdiag\t1/1\tbefore")
    local said = table.concat({ select(n0 + 1, unpack(sent)) }, "\n")
    check("diag: the addon answers Den Lite's ask with its view, tagged", said:find("wudiag\tat:before", 1, true) ~= nil
        and said:find("wudiag\tme:speed=", 1, true) ~= nil and said:find("wudiag\tme2:mounted=", 1, true) ~= nil
        and said:find("W*=", 1, true) ~= nil)
end

-- the showcase (labs/den-lite showcase.rs): a share's pictures, taken by the game itself, the interface hidden
do
    local shots, turns, zoomed = 0, {}, 0
    Screenshot = function() shots = shots + 1; check("showcase: the interface is hidden for each picture", not UIParent:IsShown()) end
    FlipCameraYaw = function(a) turns[#turns + 1] = a end
    CameraZoomOut = function(n) zoomed = n end
    cv.screenshotFormat = "tga"
    UIParent:Show()
    fireClient("wuden\tshowcase\t1/1\t4|1800|24")
    local f = D.shootFrame
    check("showcase: it starts, zoomed out to fit the build, as JPEGs", f:IsShown() and zoomed == 24 and cv.screenshotFormat == "jpeg")
    check("showcase: one at a time", D.Showcase(4, 1.8, 24) == false)
    for _ = 1, 60 do if f:IsShown() then f:Fire("OnUpdate", 0.5) end end
    check("showcase: four pictures, a quarter turn after each, back where it began", shots == 4 and #turns == 4
        and turns[1] == 90 and turns[4] == 90)
    check("showcase: then the interface and the player's own settings are back", not f:IsShown() and UIParent:IsShown()
        and cv.screenshotFormat == "tga")
    fireClient("wuden\tshowcase\t1/1\tnot numbers")
    check("showcase: a bad line does nothing", not f:IsShown())
end

print(string.format("\n%d/%d passed", pass, pass + fail))
if fail > 0 then os.exit(1) end
