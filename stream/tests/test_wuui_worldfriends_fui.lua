-- lua5.1 stream/tests/test_wuui_worldfriends_fui.lua [addon dir] [Forever UI's Social.lua]
-- The Leyline tab of the Friends window when Forever UI is loaded (WrathUnleashedUI/WorldFriends.lua). Forever UI's
-- Social window (ForeverUI.Social) is a child of FriendsFrame that hides every other child of it on each update, our
-- tab and panel included, and draws its own tabs for the game's five. So the Leyline tab must be one of its tabs, and
-- the panel its page:
--   a sixth tab in Forever UI's tab art, after Raid; the game's own Leyline tab hidden;
--   the tab shows our panel as Forever UI's page, under its title, and asks the world;
--   another of its tabs hides ours, ours comes back; the window opening again on our tab shows it;
--   /together opens it there; the panel fits Forever UI's shorter window (6 friend rows);
--   long answers come in parts (n/m) and are put back together.
-- Without arg[2] it runs against a fake of Forever UI's Social.lua contract (registerPage, createTab, selectTab,
-- tabWidth, apply, update, tabs, frame, G). With arg[2] (Forever UI's own Social.lua) it loads the real file over
-- stub atlas and template helpers.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local realSocial = arg[2]

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- fake widgets (as test_wuui_worldfriends.lua)
local frames, created = {}, {}
local Frame = {}
Frame.__index = function(self, k)
    local v = rawget(Frame, k)
    if v ~= nil then return v end
    if type(k) == "string" and k:match("^%u") then return function() end end
    return nil
end
function Frame:SetScript(n, fn) self.scripts[n] = fn end
function Frame:GetScript(n) return self.scripts[n] end
function Frame:HookScript(n, fn) local old = self.scripts[n]; self.scripts[n] = function(...) if old then old(...) end fn(...) end end
function Frame:Fire(n, ...) local fn = self.scripts[n]; if fn then return fn(self, ...) end end
local function visible(f)
    if not f.shown then return false end
    return f.parent == nil or visible(f.parent)
end
function Frame:Show()
    local was = visible(self); self.shown = true
    if not was and visible(self) then
        self:Fire("OnShow")
        local function down(f) for _, c in ipairs(f.children) do if c.shown then c:Fire("OnShow"); down(c) end end end
        down(self)
    end
end
function Frame:Hide()
    local was = visible(self); self.shown = false
    if was then
        self:Fire("OnHide")
        local function down(f) for _, c in ipairs(f.children) do if c.shown then c:Fire("OnHide"); down(c) end end end
        down(self)
    end
end
function Frame:IsShown() return self.shown end
function Frame:IsVisible() return visible(self) end
function Frame:SetText(t)
    local was = self.label; self.label = t
    if self.kind == "EditBox" and was ~= t then self:Fire("OnTextChanged") end
end
function Frame:GetText() return self.label end   -- kept apart from .text (Forever UI's own field for a tab's FontString)
function Frame:Enable() self.enabled = true end
function Frame:Disable() self.enabled = false end
function Frame:IsEnabled() return self.enabled and 1 or nil end
function Frame:RegisterEvent(e) self.events[e] = true end
function Frame:UnregisterEvent(e) self.events[e] = nil end
function Frame:GetName() return self.name end
function Frame:GetParent() return self.parent end
function Frame:SetParent(p)
    if self.parent then
        for i, c in ipairs(self.parent.children) do if c == self then table.remove(self.parent.children, i); break end end
    end
    self.parent = p
    if p then p.children[#p.children + 1] = self end
end
function Frame:GetChildren() local out = {}; for _, c in ipairs(self.children) do if not c.region then out[#out + 1] = c end end; return unpack(out) end
function Frame:GetRegions() local out = {}; for _, c in ipairs(self.children) do if c.region then out[#out + 1] = c end end; return unpack(out) end
function Frame:GetHeight() return self.h or 0 end
function Frame:SetHeight(h) self.h = h end
function Frame:SetWidth(w) self.w = w end
function Frame:GetWidth() return self.w or 0 end
function Frame:SetID(i) self.id = i end
function Frame:GetID() return self.id or 0 end
function Frame:GetFrameLevel() return 1 end
function Frame:SetPoint(...) self.points = self.points or {}; self.points[#self.points + 1] = { ... } end
function Frame:ClearAllPoints() self.points = {} end
function Frame:SetFocus() self.focus = true end
function Frame:ClearFocus() self.focus = false end
function Frame:HighlightText() self.highlighted = true end
function Frame:GetStringWidth() return #(self.label or "") * 6 end
local function new(kind, name, parent)
    local f = setmetatable({ kind = kind, name = name, scripts = {}, events = {}, shown = true, enabled = true, children = {} }, Frame)
    if parent then f:SetParent(parent) end
    if name then _G[name] = f end
    created[#created + 1] = f
    return f
end
function Frame:CreateFontString() local f = new("FontString", nil, self); f.region = true; return f end
function Frame:CreateTexture() local t = new("Texture", nil, self); t.region = true; return t end
function CreateFrame(kind, name, parent, template)
    local f = new(kind, name, parent); frames[#frames + 1] = f; f.template = template
    if template == "OptionsSliderTemplate" and name then new("FontString", name .. "Low", f); new("FontString", name .. "High", f) end
    return f
end

UIParent = new("Frame", "UIParent"); UIParent.top = true
function UIParent:GetScale() return 1 end
GameMenuFrame = new("Frame", "GameMenuFrame"); GameMenuFrame.top = true; GameMenuFrame.h = 300
UISpecialFrames = {}
function tinsert(t, v) table.insert(t, v) end
SlashCmdList = {}
DEFAULT_CHAT_FRAME = { AddMessage = function() end }
function GetScreenHeight() return 981 end
function InCombatLockdown() return false end
function SetBinding() return 1 end
function SetBindingClick() return 1 end
function SaveBindings() end
function SetCVar() end
function HasAction() return false end
function UnitLevel() return 80 end
function UnitClass() return "Mage", "MAGE" end
function UnitName() return "Tester" end
function GetRealmName() return "Single player" end
function PlaySound() end
function HideUIPanel(f) f:Hide() end
function ShowUIPanel(f) f:Show() end
local clock = 100
function GetTime() return clock end
function hooksecurefunc(a, b, c)
    local t, name, fn = _G, a, b
    if type(a) == "table" then t, name, fn = a, b, c end
    local old = t[name]
    t[name] = function(...) local r = old and old(...); fn(...); return r end
end
unpack = unpack or table.unpack

-- the game's Friends window (3.3.5: five tabs, Friends Who Guild Chat Raid) and its panels
FriendsFrame = new("Frame", "FriendsFrame"); FriendsFrame.top = true; FriendsFrame.numTabs = 5; FriendsFrame.selectedTab = 1
FriendsFrame:Hide()
for i = 1, 5 do local t = new("Button", "FriendsFrameTab" .. i, FriendsFrame); t.id = i end
FRIENDSFRAME_SUBFRAMES = { "FriendsListFrame", "IgnoreListFrame", "PendingListFrame", "WhoFrame", "GuildFrame", "ChannelFrame", "RaidFrame" }
for _, n in ipairs(FRIENDSFRAME_SUBFRAMES) do new("Frame", n, FriendsFrame).shown = (n == "FriendsListFrame") end
FriendsTabHeader = new("Frame", "FriendsTabHeader", FriendsFrame)
FriendsFrameCloseButton = new("Button", "FriendsFrameCloseButton", FriendsFrame)
function FriendsFrame_Update()
    for _, n in ipairs(FRIENDSFRAME_SUBFRAMES) do _G[n].shown = false end
    if FriendsFrame.selectedTab == 1 then FriendsListFrame.shown = true; FriendsTabHeader.shown = true
    elseif FriendsFrame.selectedTab == 2 then WhoFrame.shown = true end
end
FriendsFrame:SetScript("OnShow", function() FriendsFrame_Update() end)
function PanelTemplates_SetTab(f, id) f.selectedTab = id end
function PanelTemplates_SetNumTabs(f, n) f.numTabs = n end
function PanelTemplates_TabResize() end
-- the client's tab click (what Forever UI's secure tabs click)
for i = 1, 5 do
    _G["FriendsFrameTab" .. i]:SetScript("OnClick", function() PanelTemplates_SetTab(FriendsFrame, i); FriendsFrame_Update() end)
end

-- ---------------------------------------------------------------- Forever UI's Social window
if realSocial then
    -- the real file over stubs: anything Forever UI's helpers return is a frame or a callable stub
    local function stub()
        return setmetatable({}, { __index = function(t, k) local v = stub(); rawset(t, k, v); return v end,
            __call = function() return CreateFrame("Frame") end })
    end
    ForeverUI = stub()
    ForeverUI.Templates = stub()
    ForeverUI.AtlasEntry = function() return nil end
    ForeverUI.SetAtlas = function() return true end
    ForeverUI.CreateInset = function(parent, name) return CreateFrame("Frame", name, parent) end
    ForeverUI.CreatePanelButton = function(parent, text, w, h, name) local b = CreateFrame("Button", name, parent); b:SetText(text); return b end
    ForeverUI.CreateScrollBar = function(name, parent) return CreateFrame("Slider", name, parent) end
    SQUELCH_TYPE_IGNORE, FRIENDS_BUTTON_TYPE_WOW = 1, 1
    function GetNumFriends() return 0, 0 end
    function GetNumIgnores() return 0 end
    function GetSelectedFriend() return 0 end
    function GetSelectedIgnore() return 0 end
    function FriendsFrame_SelectFriend() end
    function FriendsFrame_SelectSquelched() end
    function ShowFriends() end
    dofile(realSocial)
else
    -- the fake: Forever UI's contract, as its Social.lua (the window on FriendsFrame, pages per tab, its own tabs)
    ForeverUI = { Templates = {} }
    local S = { pages = {}, tabs = {}, G = { tabGap = 3 } }
    ForeverUI.Social = S
    local registered = {}
    function S.registerPage(id, def) registered[id] = def end
    local function page(id)
        if S.pages[id] then return S.pages[id] end
        local def = registered[id]
        if not def then return nil end
        local frame = CreateFrame("Frame", "ForeverUISocialPage" .. id, S.frame)
        frame:Hide()
        local pg = { id = id, frame = frame, def = def }
        S.pages[id] = pg
        def.build(frame)
        return pg
    end
    function S.createTab(parent, name, atTop, template)
        local b = CreateFrame("Button", name, parent, template)
        b.art = {}; b.foreverTab = true
        return b
    end
    function S.selectTab(b, selected, active) b.selected = selected; if selected or not active then b:Disable() else b:Enable() end end
    function S.tabWidth(b) return 40 + #(b.label or "") * 6 end
    function S.update()
        for _, o in ipairs(S.tabs) do S.selectTab(o.button, o.id == FriendsFrame.selectedTab, true) end
        local pg = S.pages[FriendsFrame.selectedTab or 1]
        if pg and pg.frame:IsShown() then
            if pg.def.title then S.frame.title:SetText(pg.def.title() or "") end
            if pg.def.update then pg.def.update() end
        end
    end
    local function suppress()
        for _, c in ipairs({ FriendsFrame:GetChildren() }) do if c ~= S.frame and c:IsShown() then c:Hide() end end
    end
    function S.apply()
        if not S.frame or not FriendsFrame:IsShown() then return end
        local selected = FriendsFrame.selectedTab or 1
        page(selected)
        suppress()
        if not S.frame:IsShown() then S.frame:Show() end
        for id, pg in pairs(S.pages) do if id ~= selected and pg.frame:IsShown() then pg.frame:Hide() end end
        local pg = page(selected)
        if pg then pg.frame:Show() end
        S.update()
    end
    S.frame = CreateFrame("Frame", "ForeverUISocialFrame", FriendsFrame)
    S.frame.title = S.frame:CreateFontString()
    S.registerPage(1, { build = function() end, title = function() return "Friends List" end })
    S.registerPage(2, { build = function() end, title = function() return "Who" end })
    local prev
    for i, word in ipairs({ "Friends", "Who", "Guild", "Chat", "Raid" }) do
        local b = S.createTab(S.frame, "ForeverUISocialTab" .. i, false, "SecureActionButtonTemplate")
        b:SetText(word)
        b:SetScript("OnClick", function() _G["FriendsFrameTab" .. i]:Fire("OnClick") end)   -- its secure "click"
        if prev then b:SetPoint("TOPLEFT", prev, "TOPRIGHT", 3, 0) end
        S.tabs[i] = { id = i, button = b }
        prev = b
    end
    hooksecurefunc("FriendsFrame_Update", S.apply)
    FriendsFrame:HookScript("OnShow", S.apply)
end
local S = assert(ForeverUI.Social)

-- ---------------------------------------------------------------- the realm's answers (the host's world)
local sent = {}
local info = "role=host\nsharing=1\nlink=https://relay.leylinelauncher.com/join/abc#XYZ\nasking=0"
function SendAddonMessage(prefix, msg, channel, target)
    sent[#sent + 1] = { prefix, msg, channel, target }
end
local function fireClient(prefix, msg, channel, sender)
    for _, f in ipairs(frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", prefix, msg, channel, sender) end
    end
end
local function answer(data, parts)
    parts = parts or 1
    local size = math.ceil(#data / parts)
    for i = 1, parts do
        fireClient("OVMOD", "wufriends\tinfo\t" .. i .. "/" .. parts .. "\t" .. data:sub((i - 1) * size + 1, i * size), "WHISPER", "Tester")
    end
end

-- ---------------------------------------------------------------- the addon, in .toc order
local files = {}
for line in io.lines(dir .. "/WrathUnleashedUI.toc") do
    if line:match("%.lua%s*$") then files[#files + 1] = line:gsub("%s+$", "") end
end
for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
local W = assert(WrathUnleashedUI)
local F = assert(W.WorldFriends, "W.WorldFriends")
local panel = F.Panel()

local tab = _G.WrathUnleashedForeverSocialTabLeyline
-- a tab drawn selected: Forever UI's active art (the fake marks it)
local function sel(b) if b.art and b.art.activeLeft then return b.art.activeLeft:IsShown() end return b.selected == true end
check("a sixth tab on Forever UI's window, in its tab art, after Raid", tab and tab.parent == S.frame and tab:GetText() == "Leyline"
    and (realSocial or tab.foreverTab) and tab.points and tab.points[1][2] == S.tabs[#S.tabs].button)
check("the game's own Leyline tab (under Forever UI's window) is hidden", F.tab and F.tab:GetID() == 6 and not F.tab:IsShown())

ShowUIPanel(FriendsFrame)
check("the window opens on Friends: our tab not selected, our page not shown", FriendsFrame.selectedTab == 1 and not sel(tab)
    and not panel:IsVisible())
local n0 = #sent
tab:Fire("OnClick")
check("the tab shows our panel as Forever UI's page", FriendsFrame.selectedTab == 6 and panel:IsVisible()
    and panel.parent and panel.parent.parent == S.frame)
check("under Forever UI's title: Play together", S.frame.title:GetText() == "Play together" and not panel.title:IsShown())
check("our tab drawn selected, Forever UI's not", sel(tab) and not sel(S.tabs[1].button))
check("and it asks the world", #sent > n0 and sent[#sent][2] == "wufriends\tinfo\t1/1\t")

-- a long answer in parts, as ov_modapi.lua sends it
local friends = {}
for i = 1, 9 do friends[#friends + 1] = "f=G" .. string.rep(string.char(64 + i), 15) .. "\t0\t" .. (i % 2) .. "\tWren" .. string.char(96 + i) .. ":12" end
answer(info .. "\n" .. table.concat(friends, "\n"), 3)
check("an answer in three parts is put back together", F.status and F.status.host and #F.status.friends == 9
    and panel.link:GetText() == "https://relay.leylinelauncher.com/join/abc#XYZ")
local rows = 0
for _, r in ipairs(panel.rows) do if r:IsShown() then rows = rows + 1 end end
check("Forever UI's shorter window: six friend rows at most", rows == 6)
check("half an answer changes nothing", (function()
    local before = F.status
    fireClient("OVMOD", "wufriends\tinfo\t1/2\trole=friend", "WHISPER", "Tester")
    return F.status == before end)())

-- another of Forever UI's tabs, then back
FriendsFrameTab2:Fire("OnClick")   -- what Forever UI's secure Who tab clicks
check("Who: our page hides, our tab not selected", FriendsFrame.selectedTab == 2 and not panel:IsVisible() and not sel(tab)
    and sel(S.tabs[2].button))
check("the game's Friends panels stay hidden under Forever UI's window", not FriendsListFrame:IsShown() and not FriendsTabHeader:IsShown())
tab:Fire("OnClick")
check("back on Leyline", panel:IsVisible() and sel(tab))
HideUIPanel(FriendsFrame)
check("closed with the window", not panel:IsVisible())
ShowUIPanel(FriendsFrame)
check("opened again on our tab: our page again", FriendsFrame.selectedTab == 6 and panel:IsVisible())
HideUIPanel(FriendsFrame)
FriendsFrame.selectedTab = 1

-- /together
SlashCmdList.WUWORLDFRIENDS("")
check("/together opens the Friends window on the Leyline tab", FriendsFrame:IsShown() and FriendsFrame.selectedTab == 6
    and panel:IsVisible())
check("the Leyline friends button sits on the window's bottom row", panel.social:IsVisible()
    and panel.social.points[#panel.social.points][2] == S.frame)

-- once only
check("hooking again adds nothing", F.HookForever() == tab)

-- no brand names in anything shown
local bad = false
for _, f in ipairs(created) do
    local t = tostring(f.label or ""):lower()
    if t:find("warcraft") or t:find("blizzard") or t:find("lich king") or t:find("%f[%a]wow%f[%A]") then bad = true end
end
check("no brand names", not bad)

print(string.format("%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
