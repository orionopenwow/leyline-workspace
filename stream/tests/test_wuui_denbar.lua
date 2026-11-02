-- lua5.1 stream/tests/test_wuui_denbar.lua [addon dir]
-- Den mode (DenBar.lua; owner 1 Nov 23:46: "There was no interface to deal with the den ... make it make sense, not
-- spam the chat"). Fake 3.3.5a API (as test_wuui_denhome.lua) and every file of WrathUnleashedUI in .toc order, with
-- a fake Den Lite answering the bar's addon messages as labs/den-lite notice.rs / ovmod.rs do: on entering Your Den
-- the bar opens by itself and says hello; the first time the four intro cards show (Next / Skip, remembered in the
-- saved variables, again from Help); Build opens the Builder; Shape ground, Save (a name) and Invite send the Den's
-- commands; the invite link lands in the copy box; toasts show one at a time, then "Saved 12 s ago"; play mode hides
-- every editor window and back (F8, the pill, Esc > Back to building); F7 / F8 are bound only in the Den and only
-- when free; elsewhere no bar, no keys; nothing goes to the chat; no game-brand names on screen.
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
check("six big buttons: Build, Shape ground, Play, Save, Invite, Help",
    bar.build:GetText() == "Build (F7)" and bar.ground:GetText() == "Shape ground" and bar.playBtn:GetText() == "Play (F8)"
    and bar.save:GetText() == "Save" and bar.invite:GetText() == "Invite" and bar.help:GetText() == "Help")
check("F7 and F8 bound in the Den (free: a stock game binds neither)", overrides.F7 == "LeylineDenKeyBuild" and overrides.F8 == "LeylineDenKeyPlay")

-- the intro, the first time
local intro = D.Intro()
check("the first time: the intro's first card", intro and intro:IsShown() and intro.head:GetText() == "This is your Den"
    and intro.body:GetText():find("empty white world that's yours") and intro.count:GetText() == "1 / 4")
check("four short cards", #D.CARDS == 4 and (function()
    for _, c in ipairs(D.CARDS) do if #c[2] > 160 or #c[1] > 20 then return false end end
    return D.CARDS[2][1] == "Build" and D.CARDS[3][1] == "Shape ground" and D.CARDS[4][2]:find("Invite")
end)())
intro.next:Fire("OnClick")
check("Next: Build", intro.head:GetText() == "Build" and intro.count:GetText() == "2 / 4")
intro.next:Fire("OnClick"); intro.next:Fire("OnClick")
check("the last card: Start building, no Skip", intro.head:GetText() == "Play and invite" and intro.next:GetText() == "Start building" and not intro.skip:IsShown())
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
local dr = D.Drawer()
check("Save: a name box under the bar", dr:IsShown() and D.drawerKind == "save" and dr.box.focus and dr.box:GetText() == "my-den")
dr.box:SetText("castle by the lake")
dr.ok:Fire("OnClick")
check("a name with spaces is made a plain name and saved", sentHas("wuden\tcmd\t1/1\tsave castle-by-the-lake") and not dr:IsShown())
check("the bar hears it: Saved as ...", D.saved and D.saved.name == "castle-by-the-lake")
tick(D.TOAST_SECS + 0.1)
tick(12)
check("then: Saved as castle-by-the-lake 12 s ago (the line counts on)", bar.line:GetText():find("Saved as castle%-by%-the%-lake 1%d s ago") ~= nil)
bar.save:Fire("OnClick")
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
check("the link lands in the copy box, focused to copy", dr.box:GetText() == link and dr.box.focus and dr.label:GetText() == "Invite link:")
dr.box:SetText(link .. "x")
check("a key typed doesn't change the link", dr.box:GetText() == link)
fireClient("wuden\tinvite\t1/1\tjavascript:alert(1)")
check("anything that isn't an invite link is ignored", D.link == link)
local den = W.EditMode.DenPanel()
check("the hidden Den panel doesn't take the keyboard", not (den and den.link.focus))
dr.close:Fire("OnClick")

bar.help:Fire("OnClick")
check("Help: the intro again", intro:IsShown() and intro.count:GetText() == "1 / 4")
intro.skip:Fire("OnClick")
check("Skip closes it", not intro:IsShown())

-- ---------------------------------------------------------------- 3. play mode
W.EditMode.ShowDen(true)
W.DenHome.Show(true)
B.ShowProps(true)
check("before: the Builder, the Den panel, Den home and the properties are open", B.Window():IsShown() and W.EditMode.DenPanel():IsShown()
    and W.DenHome.Panel():IsShown() and B.Panel():IsShown())
_G.LeylineDenKeyPlay:Fire("OnClick", "LeftButton", true)
_G.LeylineDenKeyPlay:Fire("OnClick", "LeftButton", false)
check("F8: play mode (one press, down and up, counts once)", D.play == true)
check("every editor window goes", not B.Window():IsShown() and not W.EditMode.DenPanel():IsShown() and not W.DenHome.Panel():IsShown()
    and not B.Panel():IsShown())
check("the bar shrinks to one button", not bar:IsShown() and bar.pill:IsShown() and bar.pill.back:GetText() == "Back to building (F8)")
check("a toast says how back", bar.pill.line:GetText():find("Back to building") ~= nil)
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
check("F7 in play mode: back to building, the Builder opens", D.play == false and B.Window():IsShown())

-- ---------------------------------------------------------------- 4. keys only when free, and nowhere else
bound.F8 = "TOGGLEAUTORUN"
D.BindKeys()
check("F8 taken by the player: Ctrl+F8, and the button says so", overrides.F8 == nil and overrides["CTRL-F8"] == "LeylineDenKeyPlay"
    and bar.playBtn:GetText() == "Play (Ctrl+F8)")
bound.F8 = nil

realm = "Wrath Unleashed"
denLite = false
fire("PLAYER_ENTERING_WORLD")
check("on another realm: no bar, no keys, no Esc button", not D.IsActive() and not bar:IsShown() and not bar.pill:IsShown() and next(overrides) == nil
    and not mb:IsShown())
check("and play mode can't be turned on there", D.SetPlay(true) == false)
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

print(string.format("\n%d/%d passed", pass, pass + fail))
if fail > 0 then os.exit(1) end
