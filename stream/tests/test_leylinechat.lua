-- lua5.1 stream/tests/test_leylinechat.lua [addon dir]     (texlua works too)
-- Fake 3.3.5a chat API: loads LeylineChat (every file in .toc order) with Forever UI's two chat pieces present, enters
-- the world, and checks the skin on ChatFrame1, the World tab (a World channel line, a "[Discord] Name: text"
-- broadcast as a [D] line), timestamps, short channel names, class colours, the gold filter, the Tab channel cycle,
-- /world, /lchat (the settings window, themes, off and on again), typed history, web links, the Lua 5.1 rules and
-- the brand words; the fade (every texture and text the addon draws is hidden or at alpha 0 once the chat is faded,
-- the input line's backdrop included), the channel label on the input line, unread counts on the tabs, "new lines
-- below", the mouse wheel and clickable names.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/LeylineChat"

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end
local unpack = unpack or table.unpack

local function readFile(p)
    local f = io.open(p, "rb")
    if not f then return nil end
    local s = f:read("*a")
    f:close()
    return s
end

-- ---------------------------------------------------------------- fake widgets
local frames = {}
local missingArt = {}
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
function Frame:SetText(t) self.text = t; if self.scripts.OnTextChanged then self:Fire("OnTextChanged") end end
function Frame:GetText() return self.text end
function Frame:GetStringWidth() return #(self.text or "") * 6 end
function Frame:SetTexture(t, ...)
    self.texture = t
    if type(t) == "string" and t:find("^Interface\\AddOns\\LeylineChat\\Media\\") then
        local name = t:sub(#"Interface\\AddOns\\LeylineChat\\Media\\" + 1)
        if readFile(dir .. "/Media/" .. name .. ".tga") then return 1 end
        missingArt[name] = true
        return nil
    end
    return 1
end
function Frame:GetTexture() return self.texture end
function Frame:SetAlpha(a) self.alpha = a end
function Frame:GetAlpha() return self.alpha or 1 end
function Frame:RegisterEvent(e) self.events[e] = true end
function Frame:UnregisterEvent(e) self.events[e] = nil end
function Frame:GetName() return self.name end
function Frame:GetParent() return self.parent end
function Frame:SetParent(p) self.parent = p end
function Frame:GetObjectType() return self.kind end
function Frame:GetFrameLevel() return self.level or 1 end
function Frame:SetFrameLevel(l) self.level = l end
function Frame:GetRegions() return unpack(self.regions) end
function Frame:SetAttribute(k, v) self.attrs[k] = v end
function Frame:GetAttribute(k) return self.attrs[k] end
function Frame:SetFont(p, s, f) self.font = { p, s, f } end
function Frame:GetFont() if self.font then return self.font[1], self.font[2], self.font[3] end end
function Frame:SetTextColor(r, g, b) self.color = { r, g, b } end
function Frame:ClearAllPoints() self.points = {} end
function Frame:SetPoint(...) self.points = self.points or {}; table.insert(self.points, { ... }) end
function Frame:GetNumPoints() return #(self.points or {}) end
function Frame:GetPoint(i) local p = (self.points or {})[i]; if p then return unpack(p) end end
function Frame:SetWidth(w) self.width = w end
function Frame:GetWidth() return self.width or 100 end
function Frame:SetHeight(h) self.height = h end
function Frame:GetHeight() return self.height or 100 end
-- ScrollingMessageFrame
function Frame:AddMessage(text, r, g, b) table.insert(self.messages, { text = text, r = r, g = g, b = b }) end
function Frame:Clear() self.messages = {} end
function Frame:GetNumMessages() return #self.messages end
function Frame:GetCurrentScroll() return 0 end
function Frame:AtBottom() return true end
-- EditBox
function Frame:HasFocus() return self.focus end
function Frame:SetFocus() self.focus = true end
function Frame:ClearFocus() self.focus = false end

local function new(kind, name, parent)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, events = {}, shown = true,
        regions = {}, attrs = {}, messages = {} }, Frame)
    if name then _G[name] = f end
    return f
end
-- every texture and text the addon creates (for the fade checks)
local trackOurs, ourRegions = false, {}
local function region(kind, name, parent)
    local r = new(kind, name, parent)
    table.insert(parent.regions, r)
    if trackOurs then ourRegions[#ourRegions + 1] = r end
    return r
end
function Frame:CreateFontString(name) return region("FontString", name, self) end
function Frame:CreateTexture(name) return region("Texture", name, self) end
function CreateFrame(kind, name, parent, template)
    local f = new(kind, name, parent)
    f.template = template
    frames[#frames + 1] = f
    return f
end

UIParent = new("Frame", "UIParent"); UIParent.top = true
UISpecialFrames = {}
SlashCmdList = {}
GameTooltip = new("GameTooltip", "GameTooltip")
function GetTime() return T end
T = 100
date = os.date
function PlaySound() end
function MouseIsOver() return false end
function UIFrameFadeRemoveFrame() end
local shift = false
function IsShiftKeyDown() return shift end
function UnitName(u) if u == "player" then return "Orion" end; return nil end
function UnitExists(u) return u == "player" end
function UnitIsPlayer() return true end
function UnitClass() return "Druid", "DRUID" end
function IsInGuild() return true end
function GetNumPartyMembers() return 0 end
function GetNumRaidMembers() return 0 end
function GetNumIgnores() return 1 end
function GetIgnoreName() return "Troll" end
local cvars = { whisperMode = "inline", guildMemberNotify = "1" }
function GetCVar(k) return cvars[k] end
function SetCVar(k, v) cvars[k] = v end
CHAT_TIMESTAMP_FORMAT = "%H:%M "
RAID_CLASS_COLORS = { MAGE = { r = 0.41, g = 0.8, b = 0.94 }, DRUID = { r = 1, g = 0.49, b = 0.04 } }
ChatTypeInfo = { SAY = { r = 1, g = 1, b = 1 }, GUILD = { r = 0.25, g = 1, b = 0.25 }, SYSTEM = { r = 1, g = 1, b = 0 },
    CHANNEL = { r = 1, g = 0.75, b = 0.75 }, CHANNEL5 = { r = 0.9, g = 0.7, b = 0.5 }, WHISPER = { r = 1, g = 0.5, b = 1 },
    CHANNEL1 = { r = 1, g = 0.75, b = 0.75 } }
local sent = {}
function SendChatMessage(msg, kind, lang, target) table.insert(sent, { msg, kind, target }) end
local joined = {}
local channels = { General = 1, World = 5 }
function GetChannelName(n)
    if type(n) == "number" then
        for name, id in pairs(channels) do if id == n then return id, name end end
        return 0
    end
    local id = channels[n]; if id then return id, n end; return 0
end
function JoinChannelByName(n) joined[#joined + 1] = n; channels[n] = 6 end
function LeaveChannelByName(n) channels[n] = nil end
local headerUpdates = 0
function ChatEdit_UpdateHeader(eb) headerUpdates = headerUpdates + 1 end
local opened = {}
function ChatFrame_OpenChat(text, cf) opened[#opened + 1] = text or "" end
function FCF_SavePositionAndDimensions() end
local refs = {}
function SetItemRef(link, text, button) refs[#refs + 1] = link end
function ChatFrame_OnHyperlinkShow(self, link, text, button) SetItemRef(link, text, button, self) end
function hooksecurefunc(name, fn)
    local orig = _G[name]
    _G[name] = function(...)
        local r = { orig(...) }
        fn(...)
        return unpack(r)
    end
end

-- the chat frames: 1 General, 2 Combat Log, 3 a third docked tab; each with its tab, edit box and stock art
NUM_CHAT_WINDOWS = 10
CHAT_FRAME_TEXTURES = { "Background", "TopLeftTexture", "BottomLeftTexture", "TopRightTexture", "BottomRightTexture",
    "LeftTexture", "RightTexture", "BottomTexture", "TopTexture", "ResizeButton" }
CHAT_FRAMES = {}
local names = { "General", "Combat Log", "Trade" }
for i = 1, 3 do
    local n = "ChatFrame" .. i
    local f = CreateFrame("ScrollingMessageFrame", n, UIParent)
    f.level = 2
    f:SetFont("Fonts\\ARIALN.TTF", 14, "")
    f.isDocked, f.isStaticDocked = true, true
    for _, s in ipairs(CHAT_FRAME_TEXTURES) do f:CreateTexture(n .. s) end
    f.buttonFrame = CreateFrame("Frame", n .. "ButtonFrame", f)
    local tab = CreateFrame("Button", n .. "Tab", UIParent)
    for _, s in ipairs({ "Left", "Middle", "Right", "SelectedLeft", "SelectedMiddle", "SelectedRight", "Glow" }) do tab:CreateTexture(n .. "Tab" .. s) end
    local tt = tab:CreateFontString(n .. "TabText")
    tt:SetText(names[i])
    tab.clicked = 0
    tab:SetScript("OnClick", function(self) self.clicked = self.clicked + 1 end)
    local eb = CreateFrame("EditBox", n .. "EditBox", f)
    eb:CreateTexture(n .. "EditBoxLeft")
    eb:CreateFontString(n .. "EditBoxHeader")
    eb:SetAttribute("chatType", "SAY")
    eb.AddHistoryLine = function(self, line) self.hist = self.hist or {}; table.insert(self.hist, line) end
    eb:SetScript("OnTabPressed", function(self) self.stockTab = (self.stockTab or 0) + 1 end)
    eb.shown = (i == 1)
    f.editBox = eb
    f.channelList = (i == 1) and { "General", "World" } or {}
    for _, e in ipairs({ "CHAT_MSG_SAY", "CHAT_MSG_GUILD", "CHAT_MSG_CHANNEL", "CHAT_MSG_SYSTEM", "CHAT_MSG_WHISPER" }) do
        f.events[e] = (i ~= 2)
    end
    CHAT_FRAMES[i] = n
end
ChatFrame2.events = { COMBAT_LOG_EVENT = true }
DEFAULT_CHAT_FRAME = ChatFrame1
SELECTED_CHAT_FRAME = ChatFrame1
COMBATLOG = ChatFrame2
function IsCombatLog(f) return f == ChatFrame2 end
GENERAL_CHAT_DOCK = CreateFrame("Frame", "GeneralDockManager", UIParent)
GENERAL_CHAT_DOCK.DOCKED_CHAT_FRAMES = { ChatFrame1, ChatFrame2, ChatFrame3 }
GENERAL_CHAT_DOCK.scrollFrame = CreateFrame("ScrollFrame", "GeneralDockManagerScrollFrame", GENERAL_CHAT_DOCK)
GENERAL_CHAT_DOCK.primary = ChatFrame1
local selectedWindow = ChatFrame1
function FCFDock_GetChatFrames(dock) return dock.DOCKED_CHAT_FRAMES end
function FCFDock_GetSelectedWindow() return selectedWindow end
local dockUpdates = 0
function FCFDock_UpdateTabs(dock) dockUpdates = dockUpdates + 1 end
function FCF_StartAlertFlash(f) f.alerting = true end
function FCF_StopAlertFlash(f) f.alerting = false end
function ChatFrame_RemoveChannel(f, name)
    for i, n in ipairs(f.channelList) do if n == name then table.remove(f.channelList, i); return end end
end
function ChatFrame_AddChannel(f, name) table.insert(f.channelList, name) end
local function listed(f, name) for _, n in ipairs(f.channelList) do if n == name then return true end end return false end

-- Forever UI's chat: its scroll bar and scroll-to-bottom button on each window (by name), and its dock hook
ForeverUI = { Chat = {} }
local fuiBar = CreateFrame("Frame", "ChatFrame1ForeverScrollBar", ChatFrame1)
local fuiBack = CreateFrame("Button", "ChatFrame1ForeverScrollToBottom", ChatFrame1)
hooksecurefunc("FCFDock_UpdateTabs", function(dock)
    -- Forever UI's placeDock: its tab places and its own sizes (ours must win, our hook runs after this one)
    for _, f in ipairs(dock.DOCKED_CHAT_FRAMES) do _G[f:GetName() .. "Tab"]:SetWidth(77) end
end)

-- the stock chat handler: filters, the frame's channel list, then a 3.3.5-shaped line
local filters = {}
function ChatFrame_AddMessageEventFilter(event, fn)
    filters[event] = filters[event] or {}
    table.insert(filters[event], fn)
end
local function stockLine(frame, event, ...)
    local a = { ... }
    for _, fn in ipairs(filters[event] or {}) do
        local drop = fn(frame, event, unpack(a, 1, 12))
        if drop then return end
    end
    local msg, author = a[1], a[2]
    local who = author and ("|Hplayer:" .. author .. ":1|h[" .. author .. "]|h") or ""
    if event == "CHAT_MSG_SAY" then
        frame:AddMessage(who .. " says: " .. msg, 1, 1, 1)
    elseif event == "CHAT_MSG_GUILD" then
        frame:AddMessage("|Hchannel:GUILD|h[Guild]|h " .. who .. ": " .. msg, 0.25, 1, 0.25)
    elseif event == "CHAT_MSG_CHANNEL" then
        if not listed(frame, a[9]) then return end
        frame:AddMessage("|Hchannel:channel:" .. a[8] .. "|h[" .. a[4] .. "]|h " .. who .. ": " .. msg, 1, 0.75, 0.75)
    elseif event == "CHAT_MSG_WHISPER" then
        frame:AddMessage(who .. " whispers: " .. msg, 1, 0.5, 1)
    elseif event == "CHAT_MSG_SYSTEM" then
        frame:AddMessage(msg, 1, 1, 0)
    end
end
-- one game event: the chat frames (stock handler) and every addon frame that registered it
local function event(name, ...)
    for i = 1, 3 do
        local f = _G["ChatFrame" .. i]
        if f.events[name] then stockLine(f, name, ...) end
    end
    for _, f in ipairs(frames) do
        if f.events[name] and f.scripts.OnEvent then f.scripts.OnEvent(f, name, ...) end
    end
end
local function lastText(f) local m = f.messages[#f.messages]; return m and m.text or "" end
local function has(f, needle)
    for _, m in ipairs(f.messages) do if m.text:find(needle, 1, true) then return true end end
    return false
end
local function tick(dt) T = T + dt; for _, f in ipairs(frames) do if f.scripts.OnUpdate then f.scripts.OnUpdate(f, dt) end end end
local function quiet(seconds) for _ = 1, seconds do tick(1) end end

-- What still shows of ours: every texture (with something set) and text (not empty) the addon created that is
-- visible with an alpha above 0 (its own alpha times its parents'). The separate dialogs (copy, link, settings,
-- menu, expanded view) are not part of the chat window and do not fade.
local DIALOGS = { LeylineChatLink = true, LeylineChatCopy = true, LeylineChatSettings = true, LeylineChatMenu = true,
    LeylineChatExpanded = true, LeylineChatExpand = true }
local function effAlpha(r)
    local a, p = r.alpha or 1, r.parent
    while p do a = a * (p.alpha or 1); p = p.parent end
    return a
end
local function where(r)
    local names, p = {}, r.parent
    while p and p ~= UIParent do names[#names + 1] = p.name or "?"; p = p.parent end
    return (r.name or r.kind) .. " in " .. table.concat(names, "<")
end
local function litPieces()
    local out = {}
    for _, r in ipairs(ourRegions) do
        local dialog, p = false, r.parent
        while p do if p.name and DIALOGS[p.name] then dialog = true end; p = p.parent end
        local drawn = (r.kind == "Texture" and r.texture ~= nil) or (r.kind == "FontString" and r.text ~= nil and r.text ~= "")
        if not dialog and drawn and r:IsVisible() and effAlpha(r) > 0.01 then out[#out + 1] = where(r) end
    end
    return out
end

-- ---------------------------------------------------------------- load in .toc order
local toc = readFile(dir .. "/LeylineChat.toc")
check("toc: ## Interface: 30300", toc and toc:find("## Interface: 30300", 1, true) ~= nil)
check("toc: saved settings LeylineChatDB, per-character LeylineChatCharDB",
    toc:find("## SavedVariables: LeylineChatDB", 1, true) and toc:find("## SavedVariablesPerCharacter: LeylineChatCharDB", 1, true) and true)
check("toc: no dependency on the old core addon", not toc:lower():find("wenillacore") and not toc:find("## Dependencies"))
local files = {}
for line in toc:gmatch("[^\r\n]+") do
    if not line:match("^%s*#") and line:match("%S") then files[#files + 1] = line:match("^%s*(.-)%s*$") end
end
local okAll, err = true, nil
trackOurs = true
for _, f in ipairs(files) do
    local ok, e = pcall(dofile, dir .. "/" .. f)
    if not ok then okAll, err = false, e end
end
check("loads every file in .toc order without an error" .. (err and (": " .. tostring(err)) or ""), okAll)
local C = LeylineChat
check("LeylineChat is the addon's table", type(C) == "table" and C.ID == "LeylineChat")

-- Lua 5.1 rules: no 5.2+ syntax and no 5.0 leftovers in the code (comments and strings aside)
local function code(src)
    src = src:gsub("%-%-%[(=*)%[.-%]%1%]", "")
    src = src:gsub("%-%-[^\n]*", "")
    src = src:gsub('"[^"\n]*"', '""'):gsub("'[^'\n]*'", "''")
    return src
end
local bad = {}
for _, f in ipairs(files) do
    local src = code(readFile(dir .. "/" .. f))
    for _, rx in ipairs({ "%f[%w]goto%f[%W]", "//", "%f[%w]utf8%.", "<<", ">>", "[^~=<>]~[^=]", "&", "\\z",
        "table%.getn", "getglobal", "%f[%w]this%f[%W]", "%f[%w]arg[1-9]%f[%W]", "string%.gfind", "math%.mod",
        "%f[%w]unpack%(", "loadstring", "setfenv", "table%.unpack" }) do
        if src:find(rx) then bad[#bad + 1] = f .. ": " .. rx end
    end
end
check("Lua 5.1 rules (no goto, //, bit operators, utf8, \\z; no this/arg1/table.getn)" .. (bad[1] and (": " .. table.concat(bad, ", ")) or ""), not bad[1])

-- brand words: nowhere in the addon (code, comments, toc); no old project names players could read
local brandHits = {}
local allText = toc
for _, f in ipairs(files) do allText = allText .. "\n" .. readFile(dir .. "/" .. f) end
for _, rx in ipairs({ "[Ww][Oo][Rr][Ll][Dd]%s+[Oo][Ff]%s+[Ww][Aa][Rr][Cc][Rr][Aa][Ff][Tt]", "[Ww][Aa][Rr][Cc][Rr][Aa][Ff][Tt]",
    "[Ll][Ii][Cc][Hh]%s*[Kk][Ii][Nn][Gg]", "[Bb][Ll][Ii][Zz][Zz][Aa][Rr][Dd]", "%f[%w]WoW%f[%W]",
    "[Oo]pen [Vv]anilla", "[Cc]lassic%s*[Pp]lus", "[Ww]enilla", "CPChat", "/cpchat" }) do
    if allText:find(rx) then brandHits[#brandHits + 1] = rx end
end
check("no brand words or old project names" .. (brandHits[1] and (": " .. table.concat(brandHits, ", ")) or ""), not brandHits[1])

-- ---------------------------------------------------------------- enter the world
LeylineChatDB, LeylineChatCharDB = nil, nil      -- the game's saved copies: none yet (a first login)
event("ADDON_LOADED", "LeylineChat")
event("PLAYER_ENTERING_WORLD")
check("on by default after entering the world", C.active == true and C.skinOn == true)
local k = ChatFrame1.lcSkin
check("ChatFrame1 is skinned: its panel and shadow are shown", k and k.panel:IsShown() and k.back:IsShown())
check("ChatFrame1 panel: the Slate panel art", k and k.panel.face.texture == C.MEDIA .. "Chat_Panel")
check("ChatFrame1 stock background and button column are hidden",
    not ChatFrame1Background:IsShown() and not ChatFrame1ButtonFrame:IsShown() and ChatFrame1Background.Show ~= Frame.Show)
check("ChatFrame1 text: our size (13) with an outline", ChatFrame1.font[2] == 13 and ChatFrame1.font[3] == "OUTLINE")
check("ChatFrame1's tab: flat, the stock tab art hidden", ChatFrame1Tab.lcBg ~= nil and not ChatFrame1TabLeft:IsShown() and not ChatFrame1TabGlow:IsShown())
check("the selected tab has the accent line, the others not", ChatFrame1Tab.lcLine.alpha == 1 and ChatFrame3Tab.lcLine.alpha == 0)
check("the tab bar (menu, copy, expand) is shown", LeylineChatBar and LeylineChatBar:IsShown() and LeylineChatMenuButton ~= nil)
check("the dock sits on our tab bar", GENERAL_CHAT_DOCK.points[1][2] == LeylineChatBar)
check("the edit box is in the panel's well, on the screen (not the window)",
    ChatFrame1EditBox.lcBg ~= nil and ChatFrame1EditBox:GetParent() == UIParent and not ChatFrame1EditBoxLeft:IsShown())
check("Forever UI's chat scroll bar and scroll-to-bottom button stand down", not fuiBar:IsShown() and not fuiBack:IsShown())
check("the game's own timestamps stand down while ours are on", CHAT_TIMESTAMP_FORMAT == nil)
check("every art name the code asked for ships in Media/", next(missingArt) == nil)
-- the dock lays out its tabs again (Forever UI's hook sizes them first): our sizes and the World tab after the fixed tabs
GENERAL_CHAT_DOCK.scrollFrame.points = {}
FCFDock_UpdateTabs(GENERAL_CHAT_DOCK)
check("after the dock's update our tab widths win over Forever UI's", ChatFrame1Tab.width == ChatFrame1Tab.lcWant and ChatFrame1Tab.width ~= 77)
local wt = LeylineChatWorldTab
check("the World tab is shown, after the last fixed tab", wt and wt:IsShown() and wt.points[1][2] == ChatFrame3Tab)
check("the dock's whisper-tab list starts after the World tab", GENERAL_CHAT_DOCK.scrollFrame.points[1][2] == wt)

-- ---------------------------------------------------------------- the World tab
check("World is taken out of the main window (its lines go to the tab)", not listed(ChatFrame1, "World")
    and LeylineChatCharDB.worldMain == "taken")
local before = #ChatFrame1.messages
event("CHAT_MSG_CHANNEL", "anyone up for Stratholme?", "Alice", "Common", "5. World", "", "", 0, 5, "World")
local wl = C.world.lines[#C.world.lines]
check("a World channel line goes to the World tab", wl and wl.text:find("[Alice]", 1, true) and wl.text:find("anyone up for Stratholme?", 1, true))
check("... and not to the main window", #ChatFrame1.messages == before)
check("... with a timestamp", wl and wl.text:find("^|cff8a909c%d+:%d%d|r ") ~= nil)
check("the World tab shows an unread dot", wt.lcUnread == true and wt.lcDot.alpha == 1)

-- a Discord line: the server's system broadcast "|cff7289da[Discord]|r Name: text"
local sysBefore1, sysBefore3 = #ChatFrame1.messages, #ChatFrame3.messages
local nWorld = #C.world.lines
event("CHAT_MSG_SYSTEM", "|cff7289da[Discord]|r Dana: hello from the Discord, a||b")
local dl = C.world.lines[#C.world.lines]
check("a [Discord] broadcast becomes one [D] line in the World tab", #C.world.lines == nWorld + 1
    and dl.text:find("|cff7289da[D]|r Dana: hello from the Discord", 1, true) ~= nil)
check("... marked with the small Discord icon", dl.text:find("|TInterface\\AddOns\\LeylineChat\\Media\\Chat_Discord:12:12:0:0|t", 1, true) ~= nil)
check("... the server's escaped | stays one escaped |", dl.text:find("a||b", 1, true) ~= nil and not dl.text:find("a||||b", 1, true))
check("... and leaves the system lines of every window", #ChatFrame1.messages == sysBefore1 and #ChatFrame3.messages == sysBefore3)
event("CHAT_MSG_SYSTEM", "|cff7289da[Discord]|r Discord lines hidden. |cffffff00.discord on|r shows them again.")
check("the server's other [Discord] lines (no \"Name: \") stay in the main window", has(ChatFrame1, "Discord lines hidden."))
nWorld = #C.world.lines
event("CHAT_MSG_SYSTEM", "|cff7289da[Discord]|r Troll: buy my stuff")
check("an ignored name's Discord line is hidden", #C.world.lines == nWorld)
event("CHAT_MSG_SYSTEM", "|cff7289da[Discord]|r Eve: |Hitem:1|h[x]|h")
check("a stray | from a server is escaped (no link or texture from Discord)", C.world.lines[#C.world.lines].text:find("Eve: ||Hitem", 1, true) ~= nil)

-- opening the World tab
ChatFrame1EditBox:SetAttribute("chatType", "SAY"); ChatFrame1EditBox:SetAttribute("stickyType", "SAY")
LeylineChatWorldTab:Fire("OnClick", "LeftButton")
check("opening the World tab points the input line at World at once (sticky), before any Enter",
    ChatFrame1EditBox:GetAttribute("chatType") == "CHANNEL" and ChatFrame1EditBox:GetAttribute("channelTarget") == 5
    and ChatFrame1EditBox:GetAttribute("stickyType") == "CHANNEL")
check("clicking the World tab opens the World view over the main window", C.worldView and LeylineChatWorldFrame:IsShown()
    and not ChatFrame1:IsShown() and #LeylineChatWorldFrame.messages == #C.world.lines)
check("the edit box stays (it hangs off the screen, not the hidden window)", ChatFrame1EditBox:GetParent() == UIParent)
ChatFrame_OpenChat("")
check("Enter in the World view writes to World", ChatFrame1EditBox:GetAttribute("chatType") == "CHANNEL"
    and ChatFrame1EditBox:GetAttribute("channelTarget") == 5)
ChatFrame3Tab:Fire("OnClick", "LeftButton")
check("clicking a chat tab leaves the World view, then the stock click runs", not C.worldView and not LeylineChatWorldFrame:IsShown()
    and ChatFrame1:IsShown() and ChatFrame3Tab.clicked == 1)
check("leaving the World tab puts Say back on the input line", ChatFrame1EditBox:GetAttribute("chatType") == "SAY"
    and ChatFrame1EditBox:GetAttribute("stickyType") == "SAY")
ChatFrame1EditBox:SetAttribute("chatType", "SAY")

-- ---------------------------------------------------------------- the line pipeline
event("CHAT_MSG_SAY", "hello there", "Bob")
local line = lastText(ChatFrame1)
check("timestamps on: a said line starts with the time", line:find("^|cff8a909c" .. os.date("%H:%M"):gsub("%p", "%%%0") .. "|r ") ~= nil)
C:SetOption("timestamps", "off")
event("CHAT_MSG_SAY", "no stamp now", "Bob")
check("timestamps off: no time", lastText(ChatFrame1):find("^|Hplayer:Bob") ~= nil)
check("... and the game's own timestamps come back", CHAT_TIMESTAMP_FORMAT == "%H:%M ")
C:SetOption("timestamps", "24")
check("timestamps back on: the game's stand down again", CHAT_TIMESTAMP_FORMAT == nil)
event("CHAT_MSG_GUILD", "guild news", "Bob")
check("short channel names: [Guild] -> [G] (inside the 3.3.5 channel link)", lastText(ChatFrame1):find("|Hchannel:GUILD|h[G]|h", 1, true) ~= nil)
C:Learn("Alice", "MAGE")
event("CHAT_MSG_SAY", "a mage speaks", "Alice")
check("class colours: a known mage's name in mage blue", lastText(ChatFrame1):find("[|cff69ccf0Alice|r]", 1, true) ~= nil)
event("CHAT_MSG_SAY", "hey Orion, look", "Bob")
check("a mention of my name is highlighted", lastText(ChatFrame1):find("|cffff9e2eOrion|r", 1, true) ~= nil)
event("CHAT_MSG_SAY", "see leylinelauncher.com for news", "Bob")
check("a web address becomes a link", lastText(ChatFrame1):find("|Hurl:leylinelauncher.com|h[leylinelauncher.com]|h", 1, true) ~= nil)
SetItemRef("url:leylinelauncher.com", "x", "LeftButton")
check("clicking a web link opens the copy-link box (not the stock tooltip)", LeylineChatLink and LeylineChatLink:IsShown()
    and LeylineChatLinkBox.text == "leylinelauncher.com" and #refs == 0)
SetItemRef("item:6948", "x", "LeftButton")
check("other links still reach the stock code", refs[1] == "item:6948")
local n1 = #ChatFrame1.lcLines
event("CHAT_MSG_SAY", "same again", "Bob")
event("CHAT_MSG_SAY", "same again", "Bob")
check("a repeated line collapses into x2", #ChatFrame1.lcLines == n1 + 1 and ChatFrame1.lcLines[#ChatFrame1.lcLines].text:find("\195\1512", 1, true) ~= nil)
C:SetOption("goldFilter", true)
local n2 = #ChatFrame1.messages
event("CHAT_MSG_SAY", "cheap gold at www.goldz.com", "Spammer")
check("the gold filter hides an advert", #ChatFrame1.messages == n2 and C.filtered >= 1)
C:SetOption("goldFilter", false)
local c2 = #ChatFrame2.messages
ChatFrame2:AddMessage("Bob hits Gnoll for 12.", 1, 1, 1)
check("the combat log is left as it is (no stamp)", #ChatFrame2.messages == c2 + 1 and lastText(ChatFrame2) == "Bob hits Gnoll for 12.")
selectedWindow = ChatFrame1
event("CHAT_MSG_SAY", "for tab three", "Bob")
check("a line in a tab you are not on shows its unread dot", ChatFrame3Tab.lcUnread == true and ChatFrame3Tab.lcDot.alpha == 1
    and ChatFrame1Tab.lcUnread ~= true)

-- ---------------------------------------------------------------- Tab cycles channels
local eb = ChatFrame1EditBox
eb.text = ""
eb:SetAttribute("chatType", "SAY")
eb.scripts.OnTabPressed(eb)
check("Tab in an empty line: Say -> Guild", eb:GetAttribute("chatType") == "GUILD")
eb.scripts.OnTabPressed(eb)
check("Tab: Guild -> World", eb:GetAttribute("chatType") == "CHANNEL" and eb:GetAttribute("channelTarget") == 5)
eb.scripts.OnTabPressed(eb)
check("Tab: World -> Say (round)", eb:GetAttribute("chatType") == "SAY")
shift = true
eb.scripts.OnTabPressed(eb)
shift = false
check("Shift+Tab steps back: Say -> World", eb:GetAttribute("chatType") == "CHANNEL")
check("the header follows the channel", headerUpdates >= 4)
eb.text = "/w Bob hi"
eb.scripts.OnTabPressed(eb)
check("with text in the line the stock Tab runs (completion)", eb.stockTab == 1)
eb.text = ""

-- ---------------------------------------------------------------- typed history, /world
eb:AddHistoryLine("/dance")
check("typed lines are kept per character (LeylineChatCharDB.history)", LeylineChatCharDB.history[#LeylineChatCharDB.history] == "/dance"
    and eb.hist[#eb.hist] == "/dance")
SlashCmdList.LEYLINECHATWORLD("hi all")
check("/world <text> says it in World", sent[#sent][1] == "hi all" and sent[#sent][2] == "CHANNEL" and sent[#sent][3] == 5)
SlashCmdList.LEYLINECHATWORLD("")
check("/world alone opens the World tab and the input line on it", C.worldView and opened[#opened] == "/5 ")
C:LeaveWorld()

-- ---------------------------------------------------------------- join World when not in it
channels.World = nil
C.worldReady, C.worldSettleAt, C.worldAsked = nil, nil, nil
event("PLAYER_ENTERING_WORLD")
tick(1)
check("not yet: World is joined a few seconds after login", #joined == 0)
tick(9)
check("World joined on login when not in it", joined[1] == "World")
table.insert(ChatFrame1.channelList, "World")     -- the game lists the joined channel in the main window
tick(0.3)
check("World listed in the main window again by the game goes back to its tab", not listed(ChatFrame1, "World"))

-- ---------------------------------------------------------------- the fade (owner 4 Nov 14:21: "chat bar improperly
-- still showing when faded"): the input line's backdrop hung off the screen and never faded
selectedWindow = ChatFrame1
C:LeaveWorld()
local meb = ChatFrame1EditBox
ACTIVE_CHAT_EDIT_BOX = nil; meb.focus = false; meb.text = ""
meb:SetAttribute("chatType", "SAY"); meb:SetAttribute("stickyType", "SAY")
check("the default: the chrome fades after 10 s of quiet", C.DEFAULTS.fade == "10" and C:Get("fade") == "10")
local timeVisible
ChatFrame1.SetTimeVisible = function(self, s) timeVisible = s end
C:ThemeSkin(ChatFrame1.lcSkin)
ChatFrame1.SetTimeVisible = nil
check("... while the lines themselves stay the stock two minutes", timeVisible == 120)
event("CHAT_MSG_SAY", "wake up", "Bob")
tick(0.5)
check("awake: the chrome is fully shown", C.chromeAlpha == 1 and k.panel.alpha == 1 and LeylineChatBar.alpha == 1)
check("at rest the edit box's own backdrop is off; the panel's well (which fades with it) stands in",
    meb:IsShown() and meb.lcBg.alpha == 0 and meb.lcEdge.alpha == 0 and k.panel.well:IsVisible())
quiet(14)
check("after 10 s of quiet the chrome fades out", C.chromeAlpha == 0)
local lit = litPieces()
check("faded: every texture and text we draw is hidden or at alpha 0 (panel, shadow, tabs, tab bar, well, label, edit box backdrop)"
    .. (lit[1] and (": " .. table.concat(lit, ", ")) or ""), not lit[1])
check("... the tabs, the tab bar, the shadow and the jump arrow too",
    ChatFrame1Tab.alpha == 0 and ChatFrame3Tab.alpha == 0 and LeylineChatBar.alpha == 0 and k.back.alpha == 0 and k.jump.alpha == 0)
ChatFrame1TabText:SetAlpha(0)      -- Forever UI dims every tab region at its own rest
check("Forever UI cannot dim our tab names behind our fade", ChatFrame1TabText:GetAlpha() == 1)
event("CHAT_MSG_SAY", "back again", "Bob")
tick(0.3)
check("a new line brings the chrome back", C.chromeAlpha == 1 and k.panel.alpha == 1 and ChatFrame1Tab.alpha == 1)
quiet(14)
MouseIsOver = function(fr) return fr == k.panel end
tick(0.3)
MouseIsOver = function() return false end
check("the mouse over the chat brings it back", C.chromeAlpha == 1)

-- ---------------------------------------------------------------- the input line says where you talk
tick(0.3)
local pill = ChatFrame1LCChannel
check("the well shows where Enter talks: Say, in Say's colour", pill and pill:IsVisible() and pill.text.text == "Say"
    and pill.text.color[1] == 1 and pill.text.color[2] == 1)
pill:Fire("OnClick", "LeftButton")
check("clicking the label steps to the next channel, kept for the next Enter", pill.text.text == "Guild"
    and meb:GetAttribute("chatType") == "GUILD" and meb:GetAttribute("stickyType") == "GUILD")
pill:Fire("OnClick", "LeftButton")
check("... then World", pill.text.text == "World" and meb:GetAttribute("chatType") == "CHANNEL" and meb:GetAttribute("channelTarget") == 6)
pill:Fire("OnClick", "RightButton")
check("right-click steps back (Guild)", pill.text.text == "Guild")
LeylineChatWorldTab:Fire("OnClick", "LeftButton")
tick(0.3)
check("in the World tab the label says World", LeylineChatWorldFrameLCChannel:IsVisible() and LeylineChatWorldFrameLCChannel.text.text == "World")
ChatFrame1Tab:Fire("OnClick", "LeftButton")
tick(0.3)
check("back on a chat tab it says what that line had (Guild)", pill:IsVisible() and pill.text.text == "Guild")
meb:SetAttribute("chatType", "WHISPER"); meb:SetAttribute("tellTarget", "Bob")
C:RefreshInputLabels()
check("a whisper reads To Bob", pill.text.text == "To Bob")
pill:Fire("OnClick", "LeftButton")
check("clicking it from a whisper steps to Say", meb:GetAttribute("chatType") == "SAY" and pill.text.text == "Say")
local nOpened = #opened
ChatFrame1LCWell:Fire("OnClick", "LeftButton")
check("clicking the well starts typing", #opened == nOpened + 1 and opened[#opened] == "")
ACTIVE_CHAT_EDIT_BOX = meb; meb.focus = true
tick(0.1)
check("typing: the edit box backdrop shows, the well's label steps aside, the header is clickable",
    meb.lcBg.alpha > 0.5 and meb.lcEdge.alpha == 1 and not pill:IsShown() and meb.lcHeadBtn:IsShown())
meb.text = "half a line"
ChatFrame1EditBoxLCHeader:Fire("OnClick", "LeftButton")
check("clicking the header (\"Say:\") steps to the next channel, the words kept", meb:GetAttribute("chatType") == "GUILD"
    and meb.text == "half a line" and meb.focus)
ACTIVE_CHAT_EDIT_BOX = nil; meb.focus = false; meb.text = ""
meb:SetAttribute("chatType", "SAY"); meb:SetAttribute("stickyType", "SAY")
tick(0.1)
check("done typing: the backdrop goes, the label is back", meb.lcBg.alpha == 0 and pill:IsShown() and not meb.lcHeadBtn:IsShown())
local altArrow
meb.SetAltArrowKeyMode = function(self, v) altArrow = v end
C:ApplyBehaviour()
meb.SetAltArrowKeyMode = nil
check("Up and Down recall what you typed without Alt", altArrow == false)

-- the World view fades the same way
LeylineChatWorldTab:Fire("OnClick", "LeftButton")
event("CHAT_MSG_CHANNEL", "world wakes", "Alice", "Common", "6. World", "", "", 0, 6, "World")
tick(0.3)
check("World view awake: its panel shows", LeylineChatWorldFrameLCPanel.alpha == 1 and LeylineChatWorldFrameLCPanel:IsVisible())
quiet(14)
lit = litPieces()
check("World view faded: nothing of ours left showing" .. (lit[1] and (": " .. table.concat(lit, ", ")) or ""),
    not lit[1] and LeylineChatWorldFrameLCPanel.alpha == 0)
local nRefs = #refs
LeylineChatWorldFrame:Fire("OnHyperlinkClick", "player:Alice", "[Alice]", "LeftButton")
check("a name in the World tab is clickable (the game's whisper / player menu link)", #refs == nRefs + 1 and refs[#refs] == "player:Alice")
check("the World view scrolls with the mouse wheel", LeylineChatWorldFrame:GetScript("OnMouseWheel") == C.OnWheel)
ChatFrame1Tab:Fire("OnClick", "LeftButton")

-- ---------------------------------------------------------------- unread counts on the tabs
selectedWindow = ChatFrame3; C:RefreshTabs(); selectedWindow = ChatFrame1; C:RefreshTabs()
check("opening a tab clears its count", ChatFrame3Tab.lcCount == nil and not ChatFrame3Tab.lcBadge:IsShown()
    and ChatFrame3Tab.width == ChatFrame3Tab.lcBase)
event("CHAT_MSG_SAY", "one", "Bob"); event("CHAT_MSG_SAY", "two", "Bob"); event("CHAT_MSG_SAY", "three", "Bob")
FCF_StartAlertFlash(ChatFrame3)
check("lines on a tab you are not on: a count badge (3), the stock alert not counted twice", ChatFrame3Tab.lcCount == 3
    and ChatFrame3Tab.lcBadge:IsShown() and ChatFrame3Tab.lcBadge.text == "3" and ChatFrame3Tab.lcDot.alpha == 1)
check("... the tab grows to fit it, its name moves left", ChatFrame3Tab.width > ChatFrame3Tab.lcBase
    and ChatFrame3Tab.width == ChatFrame3Tab.lcWant and ChatFrame3TabText.points[1][4] < 0)
check("... dark figures on the gold badge", ChatFrame3Tab.lcBadge.color[1] < 0.2)
check("the tab you are on gets no badge", not ChatFrame1Tab.lcBadge:IsShown() and ChatFrame1Tab.lcCount == nil)
selectedWindow = ChatFrame3; C:RefreshTabs(); selectedWindow = ChatFrame1; C:RefreshTabs()
check("opening it clears the badge and the tab shrinks back", ChatFrame3Tab.lcCount == nil and not ChatFrame3Tab.lcBadge:IsShown()
    and ChatFrame3Tab.width == ChatFrame3Tab.lcBase)
C:ViewWorld(); C:LeaveWorld()
event("CHAT_MSG_CHANNEL", "w1", "Alice", "Common", "6. World", "", "", 0, 6, "World")
event("CHAT_MSG_CHANNEL", "w2", "Alice", "Common", "6. World", "", "", 0, 6, "World")
check("the World tab counts too", LeylineChatWorldTab.lcBadge:IsShown() and LeylineChatWorldTab.lcBadge.text == "2")
LeylineChatWorldTab:Fire("OnClick", "LeftButton")
check("... and opening it clears", not LeylineChatWorldTab.lcBadge:IsShown() and C.world.count == nil)
ChatFrame1Tab:Fire("OnClick", "LeftButton")

-- ---------------------------------------------------------------- new lines below, the mouse wheel
ChatFrame1.GetCurrentScroll = function() return 4 end
event("CHAT_MSG_SAY", "below one", "Bob"); event("CHAT_MSG_SAY", "below two", "Bob")
tick(0.1)
check("scrolled up: the jump button says how many lines came in below", ChatFrame1LCJump:IsShown()
    and ChatFrame1LCJump.label.text == "2 new lines" and ChatFrame1LCJump.label:IsShown() and ChatFrame1LCJump.width > 20)
ChatFrame1.ScrollToBottom = function(self) self.bottomed = (self.bottomed or 0) + 1 end
ChatFrame1LCJump:Fire("OnClick", "LeftButton")
ChatFrame1.GetCurrentScroll = nil
tick(0.1)
check("clicking it goes to the newest line and clears the count", ChatFrame1.bottomed == 1 and ChatFrame1.lcBelow == 0
    and not ChatFrame1LCJump:IsShown())
ChatFrame1.ScrollToTop = function(self) self.topped = true end
ChatFrame1.ScrollUp = function(self) self.upped = true end
local wheel = ChatFrame1:GetScript("OnMouseWheel")
check("the chat scrolls with the mouse wheel", wheel == C.OnWheel)
wheel(ChatFrame1, 1)
shift = true; wheel(ChatFrame1, 1); wheel(ChatFrame1, -1); shift = false
check("... Shift+wheel goes to the top and the bottom", ChatFrame1.upped and ChatFrame1.topped and ChatFrame1.bottomed == 2)
ChatFrame1.ScrollToTop, ChatFrame1.ScrollUp, ChatFrame1.ScrollToBottom = nil, nil, nil

-- ---------------------------------------------------------------- /lchat
check("/lchat is a slash command (and does not clash with /chat)", SLASH_LEYLINECHAT1 == "/lchat" and SlashCmdList.LEYLINECHAT ~= nil)
SlashCmdList.LEYLINECHAT("")
check("/lchat opens the settings window", LeylineChatSettings and LeylineChatSettings:IsShown())
local rows = LeylineChatSettings.rows
check("the settings window lists every option", #rows == #C.SCHEMA.options)
local themeRow
for _, r in ipairs(rows) do if r.opt.key == "theme" then themeRow = r end end
themeRow:Fire("OnClick", "LeftButton")
check("clicking the theme row steps to the next theme (Parchment), saved", LeylineChatDB.settings.theme == "parchment" and themeRow.value.text == "Parchment")
check("Parchment: the ink is on (lines re-printed in dark ink)", C.inked == true and ChatFrame1.lcSkin.panel.face.texture == C.MEDIA .. "Chat_Panel_Parchment")
SlashCmdList.LEYLINECHAT("theme midnight")
check("/lchat theme midnight", C:Get("theme") == "midnight" and C.inked == false)
SlashCmdList.LEYLINECHAT("whisperTabs")
C:SetOption("whisperTabs", true)
check("whisper tabs: the game's own whisper windows (whisperMode popout)", cvars.whisperMode == "popout" and C:Get("whisperTabs") == true)

SlashCmdList.LEYLINECHAT("off")
check("/lchat off: the stock chat comes back", not C.active and ChatFrame1Background:IsShown() and not ChatFrame1.lcSkin.panel:IsShown()
    and ChatFrame1Tab.lcOrig == nil and ChatFrame1TabLeft:IsShown())
check("... Forever UI's chat pieces come back", fuiBar:IsShown() and fuiBack:IsShown())
check("... the edit box goes back to its window", ChatFrame1EditBox:GetParent() == ChatFrame1)
check("... the tab names' alpha is the stock code's again", rawget(ChatFrame1TabText, "SetAlpha") == nil
    and rawget(ChatFrame1TabText, "lcSetAlpha") == nil)
check("... World shows in the main window again, as before", listed(ChatFrame1, "World") and LeylineChatCharDB.worldMain == nil)
check("... the game's timestamps are back", CHAT_TIMESTAMP_FORMAT == "%H:%M ")
local plain = #ChatFrame1.messages
event("CHAT_MSG_SYSTEM", "|cff7289da[Discord]|r Dana: while off")
check("... and a Discord line is the server's own line in the main window", #ChatFrame1.messages == plain + 1 and has(ChatFrame1, "Dana: while off"))
SlashCmdList.LEYLINECHAT("on")
check("/lchat on: skinned again, World back in its tab", C.active and ChatFrame1.lcSkin.panel:IsShown() and not listed(ChatFrame1, "World"))

-- the tab menu rows (right-click a chat tab)
local added = {}
UIDropDownMenu_AddButton = function(info) added[#added + 1] = info.text end
function FCFOptionsDropDown_Initialize() end
C.menuHooked = nil
event("PLAYER_ENTERING_WORLD")
FCFOptionsDropDown_Initialize(nil, 1)
check("the chat tab menu gets our rows", added[1] == C.NAME and added[#added] == "All chat settings...")

-- ---------------------------------------------------------------- the World tab hidden
SlashCmdList.LEYLINECHAT("world")
check("/lchat world hides the World tab; World goes to the main window", not LeylineChatWorldTab:IsShown() and listed(ChatFrame1, "World"))
local m1 = #ChatFrame1.messages
event("CHAT_MSG_SYSTEM", "|cff7289da[Discord]|r Dana: tab hidden")
check("with the tab hidden a Discord line shows in the main window as [W] [D]", #ChatFrame1.messages == m1 + 1
    and lastText(ChatFrame1):find("[W] |T", 1, true) ~= nil and lastText(ChatFrame1):find("[D]|r Dana: tab hidden", 1, true) ~= nil)
SlashCmdList.LEYLINECHAT("world")
check("/lchat world brings the tab back", LeylineChatWorldTab:IsShown() and not listed(ChatFrame1, "World"))

print(string.format("%d passed, %d failed", pass, fail))
if fail > 0 then os.exit(1) end
