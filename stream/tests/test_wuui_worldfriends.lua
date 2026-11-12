-- lua5.1 stream/tests/test_wuui_worldfriends.lua [addon dir] [lua_scripts dir]
-- The Leyline tab of the game's Friends window (WrathUnleashedUI/WorldFriends.lua) and its realm half
-- (realm/custom/lua_scripts/ov_worldfriends.lua, under a fake mod-ale with ov_modapi.lua and a fake characters database
-- standing in for the launcher's side, host/worldfriends.rs):
--   the tab: the game's own tab template on the Friends window (after its own tabs), its panel in place of the game's,
--     the older Leyline button gone, back to the game's panels on another tab;
--   the host: the info ask, "No link yet", Invite a friend (an ask row for the launcher, one at a time), the launcher's
--     answer and link, the link selected for Ctrl-C (read only), friends with their characters (online, trusted);
--   a friend: told their character is kept and how to come back, never sent the link, can't invite;
--   another realm (no answer): what the tab is for; only the realm's whispers to you count; the script is off unless
--   OV_WORLDFRIENDS=1; no brand names.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local scripts = arg[2] or "realm/custom/lua_scripts"

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- fake widgets (as test_wuui_host.lua)
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
function Frame:SetText(t)
    local was = self.text; self.text = t
    if self.kind == "EditBox" and was ~= t then self:Fire("OnTextChanged") end
end
function Frame:GetText() return self.text end
function Frame:Enable() self.enabled = true end
function Frame:Disable() self.enabled = false end
function Frame:RegisterEvent(e) self.events[e] = true end
function Frame:UnregisterEvent(e) self.events[e] = nil end
function Frame:GetName() return self.name end
function Frame:GetParent() return self.parent end
function Frame:GetHeight() return self.h or 0 end
function Frame:SetHeight(h) self.h = h end
function Frame:SetID(i) self.id = i end
function Frame:GetID() return self.id or 0 end
function Frame:SetFocus() self.focus = true end
function Frame:ClearFocus() self.focus = false end
function Frame:HighlightText() self.highlighted = true end
local function new(kind, name, parent)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, events = {}, shown = true, enabled = true }, Frame)
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
function hooksecurefunc(name, fn)
    local old = _G[name]
    _G[name] = function(...) local r = old and old(...); fn(...); return r end
end

-- the game's own Friends window: four tabs and its panels
FriendsFrame = new("Frame", "FriendsFrame"); FriendsFrame.top = true; FriendsFrame.numTabs = 4; FriendsFrame.selectedTab = 1
for i = 1, 4 do local t = new("Button", "FriendsFrameTab" .. i, FriendsFrame); t.id = i end
FRIENDSFRAME_SUBFRAMES = { "FriendsListFrame", "IgnoreListFrame", "PendingListFrame", "WhoFrame", "ChannelFrame", "RaidFrame" }
for _, n in ipairs(FRIENDSFRAME_SUBFRAMES) do new("Frame", n, FriendsFrame).shown = (n == "FriendsListFrame") end
FriendsTabHeader = new("Frame", "FriendsTabHeader", FriendsFrame)
function FriendsFrame_Update()
    for _, n in ipairs(FRIENDSFRAME_SUBFRAMES) do _G[n].shown = false end
    if FriendsFrame.selectedTab == 1 then FriendsListFrame.shown = true; FriendsTabHeader.shown = true
    elseif FriendsFrame.selectedTab == 2 then WhoFrame.shown = true end
end
function PanelTemplates_SetTab(f, id) f.selectedTab = id end
function PanelTemplates_SetNumTabs(f, n) f.numTabs = n end
function PanelTemplates_TabResize() end

-- ---------------------------------------------------------------- the realm: ov_modapi + ov_worldfriends, fake mod-ale
local realGetenv = os.getenv
local env = "0"
os.getenv = function(k) if k == "OV_WORLDFRIENDS" then return env end return realGetenv(k) end
local serverClock = 1000000
os.time = function() return serverClock end
local addonHandlers = {}
function RegisterServerEvent(id, fn) if id == 30 then addonHandlers[#addonHandlers + 1] = fn end end
function RegisterPlayerEvent() end
local function fireClient(prefix, msg, channel, sender)
    for _, f in ipairs(frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", prefix, msg, channel, sender) end
    end
end
local got = {}   -- what the realm whispered to the player
local me = { account = 1 }
function me:GetName() return "Tester" end
function me:GetGUIDLow() return 1 end
function me:GetAccountId() return self.account end
function me:SendAddonMessage(prefix, msg) got[#got + 1] = msg; fireClient(prefix, msg, "WHISPER", "Tester") end
function GetPlayersInWorld() return { me } end

-- the characters database the launcher and the script share (host/worldfriends.rs INGAME_TABLES)
local db = { share = {}, friends = {}, chars = {}, asks = {} }
local accounts = { [1] = "SOLO", [7] = "GABCDEFGHIJKLMNO", [8] = "GQRSTUVWXYZ23456", [9] = "GOLD234567" }
local executed = {}
local function result(rows)
    if #rows == 0 then return nil end
    local i = 1
    return { GetString = function(_, c) return tostring(rows[i][c + 1]) end, NextRow = function() i = i + 1; return i <= #rows end }
end
function CharDBExecute(q)
    executed[#executed + 1] = q
    if q:match("^INSERT INTO ov_world_asks") then db.asks[#db.asks + 1] = { id = #db.asks + 1, done = 0, answer = "" } end
end
function CharDBQuery(q)
    local k = q:match("^SELECT v FROM ov_world_share WHERE k = '(%w+)'$")
    if k then return db.share[k] and result({ { db.share[k] } }) end
    local id = tonumber(q:match("^SELECT 1 FROM ov_world_friends WHERE account = (%d+)$") or "")
    if id then for _, f in ipairs(db.friends) do if f[1] == id then return result({ { 1 } }) end end return nil end
    if q:match("^SELECT account, username, trusted FROM ov_world_friends") then return result(db.friends) end
    local a = tonumber(q:match("^SELECT name, level, online FROM characters WHERE account = (%d+)") or "")
    if a then return result(db.chars[a] or {}) end
    if q:match("^SELECT id FROM ov_world_asks WHERE done = 0") then
        for _, x in ipairs(db.asks) do if x.done == 0 then return result({ { x.id } }) end end
        return nil
    end
    if q:match("^SELECT answer FROM ov_world_asks WHERE done = 1") then
        for i = #db.asks, 1, -1 do if db.asks[i].done == 1 then return result({ { db.asks[i].answer } }) end end
        return nil
    end
    return nil
end
function AuthDBQuery(q)
    local id = tonumber(q:match("^SELECT username FROM account WHERE id = (%d+)$") or "")
    return id and accounts[id] and result({ { accounts[id] } })
end
package.preload["ov_modapi"] = function() return dofile(scripts .. "/ov_modapi.lua") end
dofile(scripts .. "/ov_modapi.lua")
dofile(scripts .. "/ov_worldfriends.lua")
check("the realm script is off unless OV_WORLDFRIENDS=1 (only the launcher's own worlds)", OVWorldFriends == nil and #executed == 0)
env = "1"
dofile(scripts .. "/ov_worldfriends.lua")
local T = assert(OVWorldFriends)
check("single player's login tips say how to invite a friend in game (Friends, the Leyline tab, /together)", (function()
    local n = 0
    for _, t in ipairs(OV_EXTRA_TIPS or {}) do
        if t.text:find("Invite a friend") and t.text:find("Leyline tab") and t.text:find("/together") then n = n + 1 end
    end
    return n == 1 end)())
check("it makes its tables when missing", #executed == 3 and executed[1]:find("CREATE TABLE IF NOT EXISTS ov_world_share"))
os.getenv = realGetenv

local sent = {}
function SendAddonMessage(prefix, msg, channel, target)
    sent[#sent + 1] = { prefix, msg, channel, target }
    if target == UnitName("player") then for _, h in ipairs(addonHandlers) do h(30, me, 7, prefix, msg, me) end end
end

-- ---------------------------------------------------------------- the addon, in .toc order
local files = {}
for line in io.lines(dir .. "/WrathUnleashedUI.toc") do
    if line:match("%.lua%s*$") then files[#files + 1] = line:gsub("%s+$", "") end
end
local at = {}
for i, f in ipairs(files) do at[f] = i end
check(".toc lists WorldFriends.lua after Social.lua, Leyline.lua still last", at["WorldFriends.lua"] and at["WorldFriends.lua"] > at["Social.lua"]
    and files[#files] == "Leyline.lua")
for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
local W = assert(WrathUnleashedUI)
local F = assert(W.WorldFriends, "W.WorldFriends")

-- ---------------------------------------------------------------- the tab
local tab = _G.FriendsFrameTab5
check("a fifth tab on the Friends window, the game's own tab template", tab and tab.template == "FriendsFrameTabTemplate"
    and tab:GetText() == "Leyline" and tab:GetID() == 5 and FriendsFrame.numTabs == 5)
check("the older Leyline button on the window is gone (the tab has it)", not _G.WrathUnleashedSocialFriendsButton
    or not _G.WrathUnleashedSocialFriendsButton:IsShown())
local panel = F.Panel()
check("its panel is made hidden, on the Friends window", panel and panel.parent == FriendsFrame and not panel:IsShown())
check("the buttons are the game's own panel buttons", panel.invite.template == "UIPanelButtonTemplate" and panel.link.template == "InputBoxTemplate")

local n0 = #sent
tab:Fire("OnClick")
check("the tab shows its panel in place of the game's", panel:IsShown() and FriendsFrame.selectedTab == 5
    and not FriendsListFrame:IsShown() and not FriendsTabHeader:IsShown())
check("and asks the world (OVMOD wufriends info, whispered to self)", #sent > n0 and sent[n0 + 1][1] == "OVMOD"
    and sent[n0 + 1][2] == "wufriends\tinfo\t1/1\t" and sent[n0 + 1][3] == "WHISPER" and sent[n0 + 1][4] == "Tester")
check("the host: invite, no link yet", F.status and F.status.host and panel.invite:IsShown() and panel.invite.enabled
    and not panel.select.enabled and panel.line:GetText() == "No link yet: press Invite a friend.")
check("no friends yet: what friends get", panel.rows[1]:IsShown() and panel.rows[1].name:GetText() == "No friends here yet.")

-- Invite a friend: an ask for the launcher
clock = clock + 2
panel.invite:Fire("OnClick")
check("Invite a friend: an ask row for the launcher", #db.asks == 1 and db.asks[1].done == 0)
check("the panel says the launcher is on it, Invite waits", panel.line:GetText():find("Asking the Leyline launcher") and not panel.invite.enabled)
clock = clock + 2
F.Invite()
check("one ask at a time", #db.asks == 1)

-- the launcher runs it: the link, sharing on, and two friends with characters
db.asks[1].done, db.asks[1].answer = 1, "ok:Your invite link is ready: copy it and send it to your friend."
local LINK = "https://relay.leylinelauncher.com/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ~~GABCDE2345.PASSWORD2345~~lan=192.168.1.5:3724~~lf1"
db.share.sharing, db.share.link = "1", LINK
db.friends = { { 7, "GABCDEFGHIJKLMNO", 0 }, { 9, "GOLD234567", 1 } }
db.chars[7] = { { "Wren", 12, 1 }, { "Ash", 80, 0 } }
db.chars[9] = { { "Bram", 30, 0 } }
F.line = ""
clock = clock + 5
panel:Fire("OnUpdate", 5)
check("the link arrives in the box", panel.link:GetText() == LINK and panel.select.enabled and panel.invite.enabled)
check("the launcher's answer shows", panel.line:GetText() == "Your invite link is ready: copy it and send it to your friend.")
check("friends with their characters, online first", panel.rows[1].name:GetText() == "Wren (12), Ash (80)"
    and panel.rows[1].state:GetText():find("Online") and panel.rows[1]:IsShown())
check("a trusted friend, offline", panel.rows[2].name:GetText() == "Bram (30)" and panel.rows[2].state:GetText():find("Offline")
    and panel.rows[2].state:GetText():find("Trusted"))
check("the sub line says it's shared", panel.sub:GetText():find("Your world is shared"))

-- the link, selected for Ctrl-C, read only
panel.select:Fire("OnClick")
check("Select link: focused and selected, with how to copy", panel.link.focus and panel.link.highlighted
    and panel.line:GetText():find("Ctrl%-C"))
panel.link:SetText("typed over")
check("the box is read only", panel.link:GetText() == LINK)

-- only the realm's whispers to me count
local before = F.status
fireClient("OVMOD", "wufriends\tinfo\t1/1\trole=host\nlink=https://evil.example/join/x#Y", "WHISPER", "Mallory")
fireClient("OVMOD", "wufriends\tinfo\t1/1\trole=host", "PARTY", "Tester")
check("anyone else's messages are ignored", F.status == before and panel.link:GetText() == LINK)

-- another tab: the game's own panel comes back
FriendsFrame.selectedTab = 1
FriendsFrame_Update()
check("another tab: ours hides, the game's come back", not panel:IsShown() and FriendsListFrame:IsShown() and FriendsTabHeader:IsShown())

-- a friend's side
me.account = 7
got = {}
SlashCmdList.WUWORLDFRIENDS("")
check("/together opens the Friends window on this tab", FriendsFrame:IsShown() and panel:IsShown() and FriendsFrame.selectedTab == 5)
check("a friend is told their character is kept and how to come back", F.status and not F.status.host
    and panel.sub:GetText():find("your character here is kept") and panel.sub:GetText():find("Join again"))
local leaked = false
for _, m in ipairs(got) do if m:find("link=") or m:find("PASSWORD2345") or m:find("f=") then leaked = true end end
check("a friend is never sent the link or the friends list", not leaked)
check("a friend sees no link or Invite", not panel.link:IsShown() and not panel.invite:IsShown() and panel.social:IsShown())
clock = clock + 5
got = {}
F.Send("invite")
check("a friend can't invite", #db.asks == 1 and got[1] and got[1]:find("^wufriends\terr\t"))
me.account = 9
check("a one-time friend's login is a friend too", T.IsFriend(me))
me.account = 1

-- another realm (no answer): what the tab is for
F.status, F.answered, F.asked = nil, false, clock - 10
F.Refresh()
check("no answer: the tab says what it is for", panel.sub:GetText():find("^This tab is for your own Leyline worlds"))

-- Leyline friends from the tab
local toggled = false
local realToggle = W.ToggleSocial
W.ToggleSocial = function() toggled = true end
panel.social:Fire("OnClick")
W.ToggleSocial = realToggle
check("Leyline friends opens the Leyline Friends panel", toggled)

-- the realm script's own checks
check("friend logins: one-time and lasting, never the host's", T.FriendLogin("GABCDE2345") and T.FriendLogin("GABCDEFGHIJKLMNO")
    and not T.FriendLogin("SOLO") and not T.FriendLogin("HKABCDEFGHIJKLMN") and not T.FriendLogin("G1"))
check("only an invite link's characters pass", T.LinkOk(LINK) and not T.LinkOk("https://x/join/y#|cffff0000") and not T.LinkOk("javascript:1"))

-- no brand names in anything shown
local bad = false
for _, f in ipairs(created) do
    local t = tostring(f.text or ""):lower()
    if t:find("warcraft") or t:find("blizzard") or t:find("lich king") or t:find("%f[%a]wow%f[%A]") then bad = true end
end
check("no brand names in the tab", not bad)

print(string.format("%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
