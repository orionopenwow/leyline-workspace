-- lua5.1 stream/tests/test_wuui_gamemenu_fui.lua
-- The Esc menu with Forever UI (ModsPanel.lua W.LayoutForeverMenu, owner 3 Nov 01:28: "fix the 2 out of style buttons
-- into the foreverui style"): Realm List and Report a bug are Forever UI's red 200 x 36 buttons in its column under
-- Mods & Workshop; the Mac options button (not a Mac) and a hidden Ratings leave no hole; Forever UI's own layout
-- running again on a button's OnShow is followed by ours. Fake widgets only.
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


-- Forever UI: its template marks a skinned button; its own Layout puts every button it knows at its own place
local skinned = {}
ForeverUI = { Templates = { ThreeSliceButton = function(b, tex) b.foreverThreeSlice = true; skinned[b] = tex end } }
ForeverUI.GameMenu = { Layout = function() for _, n in ipairs({ "GameMenuButtonOptions", "GameMenuButtonUIOptions" }) do
    local b = _G[n]; if b then b:ClearAllPoints(); b:SetPoint("TOPLEFT", GameMenuFrame, "TOPLEFT", 28, -999) end end end }
function IsMacClient() return false end
function Frame:SetWidth(w) self.w = w end
for _, n in ipairs({ "GameMenuButtonOptions", "GameMenuButtonSoundOptions", "GameMenuButtonMacOptions", "GameMenuButtonUIOptions",
    "GameMenuButtonKeybindings", "GameMenuButtonMacros", "ForeverUIGameMenuButtonCustomize", "ForeverUIGameMenuButtonHelp",
    "GameMenuButtonLogout", "GameMenuButtonQuit", "GameMenuButtonContinue" }) do
    local b = new("Button", n, GameMenuFrame); b.text = n
end
local ratings = new("Button", "GameMenuButtonRatings", GameMenuFrame); ratings.text = "Ratings"; ratings.shown = false

WrathUnleashedUI = {}
local ok, err = pcall(dofile, dir .. "/ModsPanel.lua")
check("ModsPanel.lua loads" .. (ok and "" or (": " .. tostring(err))), ok)
local W = WrathUnleashedUI
W.LeylineRealm = true
GameMenuFrame:Show()
local function at(b) return b.point and b.point[1] == "TOPLEFT" and b.point[2] == GameMenuFrame and b.point[4] == 28 and -b.point[5] end
check("Realm List: Forever UI's red button, 200 x 36", skinned[GameMenuButtonRealmList] == "128-redbutton" and GameMenuButtonRealmList.w == 200 and GameMenuButtonRealmList.h == 36)
check("Report a bug: the same", skinned[GameMenuButtonBugReport] == "128-redbutton" and GameMenuButtonBugReport.w == 200 and GameMenuButtonBugReport.h == 36)
check("the Mac options button is hidden on this PC (no hole under Interface)", GameMenuButtonMacOptions.shown == false)
check("Video, Sound, Interface, Key Bindings, Macros one under the other", at(GameMenuButtonOptions) == 48 and at(GameMenuButtonSoundOptions) == 84
    and at(GameMenuButtonUIOptions) == 120 and at(GameMenuButtonKeybindings) == 156 and at(GameMenuButtonMacros) == 192, at(GameMenuButtonKeybindings))
check("Mods & Workshop, Realm List, Report a bug, Customize UI next", at(GameMenuButtonAddOns) == 228 and at(GameMenuButtonRealmList) == 264
    and at(GameMenuButtonBugReport) == 300 and at(ForeverUIGameMenuButtonCustomize) == 336)
check("Help Request, Logout, Return to Game each after a 20 gap", at(ForeverUIGameMenuButtonHelp) == 392 and at(GameMenuButtonLogout) == 448
    and at(GameMenuButtonQuit) == 484 and at(GameMenuButtonContinue) == 540)
check("the frame fits: 28 + 200 + 28 wide, last + 36 + 34 high", GameMenuFrame.w == 256 and GameMenuFrame.h == 540 + 36 + 34, GameMenuFrame.h)
-- Forever UI's own layout again (a button's OnShow): ours after it
ForeverUI.GameMenu.Layout(); GameMenuButtonOptions:Show()
check("after Forever UI lays out again, the column is ours again", at(GameMenuButtonOptions) == 48 and at(GameMenuButtonUIOptions) == 120)
W.LeylineRealm = false
GameMenuFrame:Show()
check("another server: no Report a bug, Customize UI moves up", GameMenuButtonBugReport.shown == false and at(ForeverUIGameMenuButtonCustomize) == 300)

-- the real 3.3.5 client answers IsShown with 1 or nil, not true/false (owner 4 Nov 11:41: the frame shrank to its
-- header and the buttons hung below it)
local realIsShown = Frame.IsShown
Frame.IsShown = function(self) if self.shown then return 1 end return nil end
W.LeylineRealm = true
GameMenuFrame:Show()
check("1/nil answers: every shown button placed, the frame fits them", at(GameMenuButtonContinue) == 540 and GameMenuFrame.h == 540 + 36 + 34, GameMenuFrame.h)
check("1/nil answers: a hidden button (Ratings) still leaves no hole", at(GameMenuButtonAddOns) == 228)
Frame.IsShown = realIsShown

print(string.format("%d passed, %d failed", pass, fail))
os.exit(fail == 0 and 0 or 1)
