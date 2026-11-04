-- texlua stream/tests/test_wuui_realmguide.lua [addon dir]
-- Fake 3.3.5a API: the realm guide (RealmGuide.lua, RealmGuideData.lua) and its Leyline minimap button.
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
function Frame:SetText(t) self.text = t end
function Frame:GetText() return self.text end
function Frame:SetTexture(t) self.texture = t end
function Frame:Enable() self.enabled = true end
function Frame:Disable() self.enabled = false end
function Frame:RegisterEvent(e) self.events[e] = true end
function Frame:UnregisterEvent(e) self.events[e] = nil end
function Frame:GetName() return self.name end
function Frame:GetParent() return self.parent end
function Frame:SetCreature(id) self.creature_set = id end
local function new(kind, name, parent)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, events = {}, shown = true }, Frame)
    if name then _G[name] = f end
    created[#created + 1] = f
    return f
end
function Frame:CreateFontString() return new("FontString", nil, self) end
function Frame:CreateTexture() return new("Texture", nil, self) end
function CreateFrame(kind, name, parent) local f = new(kind, name, parent); frames[#frames + 1] = f; return f end

UIParent = new("Frame", "UIParent"); UIParent.top = true
UIParent.s = 1
function UIParent:SetScale(k) self.s = k end
function UIParent:GetScale() return self.s end
UISpecialFrames = {}
function tinsert(t, v) table.insert(t, v) end
SlashCmdList = {}
DEFAULT_CHAT_FRAME = { AddMessage = function() end }
function GetScreenHeight() return 981 end
function InCombatLockdown() return false end
local binds, clicks = {}, {}
function SetBinding(k, a) binds[k] = a or false; return 1 end
function SetBindingClick(k, b) clicks[k] = b; return 1 end
local saved = 0
function SaveBindings() saved = saved + 1 end
function GetCurrentBindingSet() return 2 end
function SetCVar() end
function SetActionBarToggles() end
function HasAction() return false end
function UnitLevel() return 80 end
function PlaySound() end
function GetCursorPosition() return 0, 0 end
Minimap = new("Frame", "Minimap")
function Minimap:GetCenter() return 100, 100 end
function Minimap:GetEffectiveScale() return 1 end
local opened = {}
ChatFrame_OpenChat = function(t) opened.chat = t end

dofile(dir .. "/WrathUnleashedUI.lua")
dofile(dir .. "/Style.lua")
dofile(dir .. "/RealmGuideData.lua")
dofile(dir .. "/RealmGuide.lua")
local W = assert(WrathUnleashedUI)
local R = assert(W.RealmGuide, "W.RealmGuide")
check("the guide's pages loaded (8 or more)", #(W.REALM_GUIDE or {}) >= 8)
local pages = R.Pages()
check("plus All commands and More", pages[#pages - 1].key == "commands" and pages[#pages].key == "more")
check("every page's icon is one of ours", (function()
    for _, p in ipairs(pages) do if W.Style.Icon(p.icon):find("noimage", 1, true) then return false, p.icon end end return true end)())

-- the minimap button: on PLAYER_ENTERING_WORLD (W.WhenLeyline may wait for the realm's hello; run it now)
W.WhenLeyline = function(fn) fn() end
for _, f in ipairs(frames) do if f.events.PLAYER_ENTERING_WORLD and f.scripts.OnEvent then f:Fire("OnEvent", "PLAYER_ENTERING_WORLD") end end
local b = _G.LeylineGuideMinimapButton
check("a round minimap button with the Leyline mark and the tracking border", b and b.parent == Minimap and b.icon.texture:find("leyline%-mark")
    and (function() for _, c in ipairs(created) do if c.parent == b and c.texture == "Interface\\Minimap\\MiniMap-TrackingBorder" then return true end end end)())
b:Fire("OnClick")
local win = _G.LeylineRealmGuide
check("click opens the guide on Welcome", win and win:IsShown() and R.page == 1 and win.heading.text:find("Welcome", 1, true))
check("rounded corners from our own corner texture (four, flipped into place), no square backdrop", win.edRoundPanel
    and (function() local n = 0; for _, c in ipairs(created) do if c.parent == win and c.texture and c.texture:find("round%-corner") then n = n + 1 end end return n == 8 end)())
check("the page shows its commands in gold", win.body.text:find("|cffe0a43a.wu|r", 1, true) ~= nil)
check("Esc closes it (UISpecialFrames)", (function() for _, n in ipairs(UISpecialFrames) do if n == "LeylineRealmGuide" then return true end end end)())
R.Go("commands")
check("All commands lists every page's commands", win.body.text:find(".solo", 1, true) and win.body.text:find(".transmog portable", 1, true))
R.Go("more")
check("More shows its buttons", win.more[1]:IsShown() and win.more[1].text == "Collections")
win.more[5]:Fire("OnClick")
check("Report a bug opens the chat with .bug", opened.chat == ".bug ")
R.Go(1)
check("the buttons hide on other pages", not win.more[1]:IsShown())
b:Fire("OnClick")
check("click again closes it", not win:IsShown())
SlashCmdList.WUREALMGUIDE("pvp")
check("/guide pvp opens that page", win:IsShown() and pages[R.page].key == "pvp")
SlashCmdList.WUREALMGUIDE("button off")
check("/wuguide button off hides the button and remembers", not b:IsShown() and WrathUnleashedUIDB.guideNoMinimap)
SlashCmdList.WUREALMGUIDE("button on")
check("/wuguide button on brings it back", b:IsShown() and not WrathUnleashedUIDB.guideNoMinimap)

print(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
