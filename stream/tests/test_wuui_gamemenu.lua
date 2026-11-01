-- lua5.1 stream/tests/test_wuui_gamemenu.lua
-- The Esc menu as one column (ModsPanel.lua W.LayoutGameMenu, owner 1 Nov 13:40: Realm List, Customize UI and Help
-- Request sat on top of each other): the game's buttons, Mods & Workshop, Realm List, then any other button on the
-- menu (Forever UI's Customize UI, Help Request) in the order they sat, a gap, Logout and Exit, a gap, Return to Game;
-- the frame's height follows. Fake widgets only.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

local Frame = {}
Frame.__index = function(self, k)
    local v = rawget(Frame, k)
    if v ~= nil then return v end
    if type(k) == "string" and k:match("^%u") then return function() end end
    return nil
end
local function new(kind, name, parent)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, shown = true, kids = {}, h = 21 }, Frame)
    if name then _G[name] = f end
    if parent and parent.kids then parent.kids[#parent.kids + 1] = f end
    return f
end
function Frame:SetScript(n, fn) self.scripts[n] = fn end
function Frame:HookScript(n, fn) local old = self.scripts[n]; self.scripts[n] = function(...) if old then old(...) end fn(...) end end
function Frame:Show() self.shown = true; if self.scripts.OnShow then self.scripts.OnShow(self) end end
function Frame:Hide() self.shown = false end
function Frame:IsShown() return self.shown end
function Frame:SetText(t) self.text = t end
function Frame:GetText() return self.text end
function Frame:GetHeight() return self.h end
function Frame:SetHeight(h) self.h = h end
function Frame:ClearAllPoints() self.point = nil end
function Frame:SetPoint(a, rel, b, x, y) self.point = { a, rel, b, x or 0, y or 0 } end
function Frame:GetPoint() local p = self.point or { "TOP", nil, "TOP", 0, -37 }; return p[1], p[2], p[3], p[4], p[5] end
function Frame:GetTop() return self.top or 0 end
function Frame:IsObjectType(t) return self.kind == t end
function Frame:GetChildren() return unpack(self.kids) end
function Frame:CreateFontString() return new("FontString", nil, self) end
function Frame:CreateTexture() return new("Texture", nil, self) end
function CreateFrame(kind, name, parent) return new(kind, name, parent) end
UIParent = new("Frame", "UIParent")
GameMenuFrame = new("Frame", "GameMenuFrame"); GameMenuFrame.h = 300
UISpecialFrames = {}
StaticPopupDialogs = {}
function tinsert(t, v) table.insert(t, v) end
SlashCmdList = {}
function PlaySound() end
function HideUIPanel(f) f:Hide() end

-- the menu as the game and Forever UI made it: Customize UI and Help Request both under Mods & Workshop
local y = 500
for _, n in ipairs({ "GameMenuButtonOptions", "GameMenuButtonSoundOptions", "GameMenuButtonUIOptions",
    "GameMenuButtonKeybindings", "GameMenuButtonMacros" }) do
    local b = new("Button", n, GameMenuFrame); b.text = n; b.top = y; y = y - 22
end
GameMenuButtonOptions:SetPoint("TOP", GameMenuFrame, "TOP", 0, -37)
local custom = new("Button", nil, GameMenuFrame); custom.text = "Customize UI"; custom.top = 380
local help = new("Button", "GameMenuButtonHelp", GameMenuFrame); help.text = "Help Request"; help.top = 379
local hidden = new("Button", nil, GameMenuFrame); hidden.text = "Ratings"; hidden.shown = false
for _, n in ipairs({ "GameMenuButtonLogout", "GameMenuButtonQuit", "GameMenuButtonContinue" }) do
    local b = new("Button", n, GameMenuFrame); b.text = n
end

WrathUnleashedUI = {}
local ok, err = pcall(dofile, dir .. "/ModsPanel.lua")
check("ModsPanel.lua loads" .. (ok and "" or (": " .. tostring(err))), ok)
local W = WrathUnleashedUI
GameMenuFrame:Show()
check("another server: no Report a bug (it has no .bug)", GameMenuButtonBugReport and not GameMenuButtonBugReport.shown)
W.LeylineRealm = true
GameMenuFrame:Show()
local function below(b, other, gap) return b.point and b.point[2] == other and b.point[3] == "BOTTOM" and b.point[5] == gap end
check("Mods & Workshop under Macros", below(GameMenuButtonAddOns, GameMenuButtonMacros, -1))
check("Realm List under Mods & Workshop", below(GameMenuButtonRealmList, GameMenuButtonAddOns, -1))
check("Report a bug under Realm List", below(GameMenuButtonBugReport, GameMenuButtonRealmList, -1))
check("Customize UI under Report a bug (no longer on top of Realm List)", below(custom, GameMenuButtonBugReport, -1))
check("Help Request under Customize UI", below(help, custom, -1))
check("a hidden button is left alone", hidden.point == nil)
check("Logout a gap below the last one", below(GameMenuButtonLogout, help, -16))
check("Exit under Logout, Return to Game a gap below", below(GameMenuButtonQuit, GameMenuButtonLogout, -1)
    and below(GameMenuButtonContinue, GameMenuButtonQuit, -16))
-- 37 + 10 buttons * 21 + 9 + (21+16) + (21+1) + (21+16) + 22
check("the frame fits every button", GameMenuFrame.h == 37 + 10 * 21 + 9 + 37 + 22 + 37 + 22, GameMenuFrame.h)
GameMenuFrame:Show()
check("opening it again lays out the same", below(help, custom, -1) and GameMenuFrame.h == 37 + 10 * 21 + 9 + 37 + 22 + 37 + 22)
-- the box: Send files it with .bug, one line, capped
local said
function SendChatMessage(msg, kind) said = { msg, kind } end
check("Send: .bug with the text", W.SendBug("  the wolf\nhas no bar ") and said[1] == ".bug the wolf has no bar" and said[2] == "SAY")
said = nil
check("Send: nothing for an empty box", W.SendBug("  ") == false and said == nil)
W.SendBug(string.rep("x", 400))
check("Send: capped to one chat line", #said[1] == 255)
check("the box is a popup with a text field", StaticPopupDialogs and StaticPopupDialogs.WU_BUG_REPORT.hasEditBox == 1)
print(pass .. "/" .. (pass + fail) .. " passed")
if fail > 0 then os.exit(1) end
