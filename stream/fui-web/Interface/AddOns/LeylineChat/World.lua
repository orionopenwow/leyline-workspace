--[[
    World.lua  (LeylineChat)
    The realm-wide World channel and the realm's Discord bridge (realm/custom/lua_scripts/ov_discord.lua).

        World tab      a tab of its own on the tab bar, on by default: every World line, and the lines from the linked
                       Discord channel marked [D] (a small Discord icon and a blue [D]). Right-click opens its menu
                       (Open, Discord, Join, Hide this tab; the chat menu, the settings or /lchat world bring a hidden
                       tab back); while it is hidden, World and [D] lines show in the main window as [W].
        [W]            World's short name in the other windows (Core.lua's ShortChannel)
        Channel cycle  Tab in an empty input line: Say > Party > Raid > Guild > World (Shift+Tab back)
        /world <text>  say something in World from anywhere (/world alone opens the tab)
        Settings       "World tab", "Show Discord messages", "Join World chat"

    The realm side: World is an ordinary custom channel named "World" (joined here a few seconds after login when
    "Join World chat" is on). Discord lines reach every player as a system broadcast, "|cff7289da[Discord]|r Name: text"
    (ov_discord.lua DISCORD_TAG, its "|" doubled); a message filter takes those out of the system lines and into the
    World tab. Your ignore list hides [D] names too. The server's other [Discord] lines (.discord on/off answers) have
    no "Name: " and stay where they are.
]]--

local C = LeylineChat
local UI = C.UI
local strfind, strsub, strlower, strlen, gsub, strmatch = string.find, string.sub, string.lower, string.len, string.gsub, string.match
local now = C.Now

C.WORLD = "World"
C.WORLD_MAX_LINES = 500
C.DISCORD_HEX = "7289da"
C.DISCORD_MARK = "|T" .. C.MEDIA .. "Chat_Discord:12:12:0:0|t|cff7289da[D]|r"
C.WORLD_SETTLE = 8          -- seconds after entering the world before "Join World chat" is reconciled
C.DISCORD_NAME_MAX = 32     -- ov_discord.lua MAX_NAME
C.world = C.world or { lines = {} }

-- -- State ---------------------------------------------------------------------------------------

function C:WorldId()
    if not GetChannelName then return 0 end
    local id = GetChannelName(self.WORLD)
    return tonumber(id) or 0
end

function C:InWorld() return self:WorldId() > 0 end

function C:IsWorldChannel(name)
    if type(name) ~= "string" then return false end
    name = strlower(name)
    if name == "world" then return true end
    return strfind(name, "^%d+%. world$") and true or false
end

function C:WorldTabShown() return (self.active and self:Get("worldTab")) and true or false end

function C:WorldColour()
    local info = ChatTypeInfo and (ChatTypeInfo["CHANNEL" .. self:WorldId()] or ChatTypeInfo.CHANNEL)
    if info then return info.r or 1, info.g or 0.75, info.b or 0.75 end
    return 1, 0.75, 0.75
end

local function ignored(name)
    if not name or not GetNumIgnores or not GetIgnoreName then return false end
    local low = strlower(name)
    for i = 1, (GetNumIgnores() or 0) do
        local n = GetIgnoreName(i)
        if n and strlower(n) == low then return true end
    end
    return false
end
C.IsIgnored = ignored

-- Text from the bridge, safe to print: the server doubles every "|" (shown as one); a stray single one is doubled
-- here, so no colour code, link or texture ever comes from Discord.
local function safe(s)
    s = gsub(s or "", "||", "\1")
    s = gsub(s, "|", "||")
    return (gsub(s, "\1", "||"))
end
C.SafeText = safe

-- "|cff7289da[Discord]|r Name: text" -> name, text (nil for any other line).
function C:ParseDiscord(msg)
    if type(msg) ~= "string" then return nil end
    local rest = strmatch(msg, "^|c%x%x%x%x%x%x%x%x%[Discord%]|r%s*(.*)$") or strmatch(msg, "^%[Discord%]%s+(.*)$")
    if not rest then return nil end
    local name, text = strmatch(rest, "^([^:|]+): (.+)$")
    if not name or strlen(name) > self.DISCORD_NAME_MAX then return nil end
    return name, text
end

-- -- The World view (a message frame over the main window) ---------------------------------------

function C:WorldFrame()
    if self.worldFrame then return self.worldFrame end
    local f = CreateFrame("ScrollingMessageFrame", "LeylineChatWorldFrame", UIParent)
    f:SetPoint("TOPLEFT", ChatFrame1, "TOPLEFT", 0, 0)
    f:SetPoint("BOTTOMRIGHT", ChatFrame1, "BOTTOMRIGHT", 0, 0)
    if ChatFrame1.GetFont and f.SetFont then
        local path, size, flags = ChatFrame1:GetFont()
        f:SetFont(path or "Fonts\\ARIALN.TTF", size or 14, flags or "")
    end
    if f.SetJustifyH then f:SetJustifyH("LEFT") end
    if f.SetMaxLines then f:SetMaxLines(self.WORLD_MAX_LINES) end
    if ChatFrame1.GetFrameStrata then f:SetFrameStrata(ChatFrame1:GetFrameStrata() or "LOW") end
    f:EnableMouse(true)
    if f.SetHyperlinksEnabled then f:SetHyperlinksEnabled(true) end
    if f.EnableMouseWheel then f:EnableMouseWheel(true) end
    f:SetScript("OnMouseWheel", C.OnWheel)
    f:SetScript("OnHyperlinkClick", function(self, link, text, button)
        if ChatFrame_OnHyperlinkShow then ChatFrame_OnHyperlinkShow(self, link, text, button)
        elseif SetItemRef then SetItemRef(link, text, button, self) end
    end)
    f:Hide()
    self.worldFrame = f
    return f
end

function C:RenderWorld()
    local f = self.worldFrame
    if not f then return end
    f:Clear()
    for _, e in ipairs(self.world.lines) do self:Emit(f, e) end
end

function C:ViewWorld()
    if not self:WorldTabShown() then return end
    local f = self:WorldFrame()
    if not self.worldView then
        self.hiddenFrame = self:Selected()
        if self.hiddenFrame and self.hiddenFrame ~= f then self.hiddenFrame:Hide() end
    end
    self.worldView = true
    self.world.unread, self.world.mention = nil, nil
    if self.SkinFrame then self:SkinFrame(f) end
    f:Show()
    self:RenderWorld()
    self:RefreshTabs()
end

function C:LeaveWorld()
    if not self.worldView then return end
    self.worldView = nil
    if self.worldFrame then self.worldFrame:Hide() end
    if self.hiddenFrame then self.hiddenFrame:Show() end
    self.hiddenFrame = nil
    self:RefreshTabs()
end

-- One line into the World view's buffer (timestamped like every other line).
function C:AddWorldLine(text, r, g, b, mention)
    text = self:StampText(text)
    local e = { text = text, r = r, g = g, b = b }
    local lines = self.world.lines
    table.insert(lines, e)
    if #lines > self.WORLD_MAX_LINES then table.remove(lines, 1) end
    if self.worldView and self.worldFrame then
        self:Emit(self.worldFrame, e)
    else
        self.world.unread = true
        if mention then self.world.mention = true end
        self:StyleWorldTab()
    end
    self:Touch(self:WorldFrame(), mention)
    return e
end

-- -- Lines -----------------------------------------------------------------------------------------

-- A player's line in World (CHAT_MSG_CHANNEL): kept for the World tab.
function C:OnWorldLine(msg, author, flag)
    if not self:WorldTabShown() or type(msg) ~= "string" then return end
    local me = UnitName and UnitName("player")
    local mine = author and author == me
    if not mine and self:Get("goldFilter") and self:IsGoldSpam(msg) then
        self.filtered = self.filtered + 1
        return
    end
    author = author or "?"
    local text = "|Hplayer:" .. author .. "|h[" .. author .. "]|h: " .. msg
    if flag == "GM" then text = self:GmTag(text, author) end
    if self:Get("classColors") then text = self:ColourName(text, author) end
    local mention = false
    if not mine and self:MentionsOn(self:WorldFrame()) and self:Mentions(msg) then
        text = self:Highlight(text)
        mention = true
    end
    if self:Get("links") then text = self:Linkify(text) end
    local r, g, b = self:WorldColour()
    return self:AddWorldLine(text, r, g, b, mention)
end

-- A line from Discord.
function C:OnDiscord(name, text)
    if not name or name == "" then return end
    if not self:Get("discord") or ignored(name) then return end
    name, text = safe(name), safe(text)
    self.discordLines = (self.discordLines or 0) + 1
    local line = self.DISCORD_MARK .. " " .. name .. ": " .. text
    local mention = self:MentionsOn(self:WorldFrame()) and self:Mentions(text)
    if mention then line = self:Highlight(line) end
    if self:Get("links") then line = self:Linkify(line) end
    local r, g, b = self:WorldColour()
    if self:WorldTabShown() then
        self:AddWorldLine(line, r, g, b, mention)
    else
        local id = self:WorldId()
        local label = (self:Get("channelNames") == "full") and ("[" .. (id > 0 and (id .. ". ") or "") .. self.WORLD .. "] ") or "[W] "
        local f = DEFAULT_CHAT_FRAME or ChatFrame1
        if f then f:AddMessage(label .. line, r, g, b) end
    end
end

-- The message filter on system lines: a Discord line goes to OnDiscord once (every chat window that shows system
-- lines runs the filter for the same line) and out of the system lines. Off, the stock chat shows the server's line.
function C.DiscordFilter(frame, event, msg)
    if not C.active then return false end
    local name, text = C:ParseDiscord(msg)
    if not name then return false end
    local key = msg .. "\1" .. tostring(now())
    if key ~= C.lastDiscordKey then
        C.lastDiscordKey = key
        C:OnDiscord(name, text)
    end
    return true
end

if ChatFrame_AddMessageEventFilter then ChatFrame_AddMessageEventFilter("CHAT_MSG_SYSTEM", C.DiscordFilter) end

-- -- Join World chat: the setting against the channel --------------------------------------------

function C:SyncWorldJoin()
    if not self.worldReady or not self.active then return end
    if now() - (self.worldAsked or -100) < 5 then return end
    local want = self:Get("worldJoin") and true or false
    local inWorld = self:InWorld()
    if want and not inWorld and JoinChannelByName then
        self.worldAsked = now()
        C.Try(JoinChannelByName, self.WORLD)
    elseif not want and inWorld and LeaveChannelByName then
        self.worldAsked = now()
        C.Try(LeaveChannelByName, self.WORLD)
    end
end

-- Does this window show the channel? (3.3.5 keeps each window's channels in frame.channelList.)
local function shows(f, name)
    if type(f.channelList) ~= "table" then return nil end
    for _, n in pairs(f.channelList) do
        if type(n) == "string" and strlower(n) == strlower(name) then return true end
    end
    return false
end

-- While the World tab shows, World lines belong there only: the main window stops showing the channel (and gets it
-- back when the tab is hidden, or the addon turned off). LeylineChatCharDB.worldMain remembers what was changed:
-- "taken" (it showed World before), "added" (it did not).
function C:RouteWorld()
    local f = DEFAULT_CHAT_FRAME or ChatFrame1
    if not f or not self:InWorld() then return end
    local char = self:CharDB()
    local mark = char.worldMain
    local has = shows(f, self.WORLD)                -- true, false, or nil (the client does not say)
    local function add() if ChatFrame_AddChannel then C.Try(ChatFrame_AddChannel, f, self.WORLD) end end
    local function remove() if ChatFrame_RemoveChannel then C.Try(ChatFrame_RemoveChannel, f, self.WORLD) end end
    if self:WorldTabShown() then
        -- World lines go to the tab only
        if has == true or (has == nil and mark == "added") then
            remove()
            char.worldMain = (mark == "added") and nil or "taken"
        elseif has == false and mark == "added" then
            char.worldMain = nil
        end
    elseif self.active then
        -- the tab is hidden: World lines in the main window, as [W]
        if has == false or (has == nil and mark ~= "added") then
            add()
            char.worldMain = (mark == "taken") and nil or "added"
        end
    else
        -- the addon is off: the main window as it was before
        if mark == "taken" and has ~= true then add() end
        if mark == "added" and has ~= false then remove() end
        char.worldMain = nil
    end
end

-- The game may list World in the main window again (a join notice handled after ours): put it back in its tab.
function C:WorldTick()
    if not self.worldReady or not self:WorldTabShown() then return end
    local f = DEFAULT_CHAT_FRAME or ChatFrame1
    if f and shows(f, self.WORLD) and self:InWorld() then self:RouteWorld() end
end

-- -- The tab ---------------------------------------------------------------------------------------

function C:WorldTab()
    local t = LeylineChatWorldTab
    if t then return t end
    t = CreateFrame("Button", "LeylineChatWorldTab", self:Bar())
    t:SetHeight(self.TAB_H)
    t:RegisterForClicks("LeftButtonUp", "RightButtonUp")
    t.lcBg = t:CreateTexture(nil, "BACKGROUND")
    t.lcBg:SetAllPoints(t)
    t.lcLine = t:CreateTexture(nil, "ARTWORK")
    t.lcLine:SetHeight(2)
    t.lcLine:SetPoint("BOTTOMLEFT", t, "BOTTOMLEFT", 3, 0)
    t.lcLine:SetPoint("BOTTOMRIGHT", t, "BOTTOMRIGHT", -3, 0)
    t.lcDot = t:CreateTexture(nil, "OVERLAY")
    t.lcDot:SetWidth(6)
    t.lcDot:SetHeight(6)
    t.lcDot:SetPoint("TOPRIGHT", t, "TOPRIGHT", -3, -3)
    t.mark = t:CreateTexture("LeylineChatWorldTabDiscord", "OVERLAY")
    t.mark:SetWidth(10)
    t.mark:SetHeight(10)
    t.mark:SetPoint("LEFT", t, "LEFT", 6, 1)
    t.label = t:CreateFontString("LeylineChatWorldTabText", "OVERLAY", "GameFontHighlightSmall")
    t.label:SetPoint("CENTER", t, "CENTER", 0, 1)
    t.label:SetText(self.WORLD)
    t:SetScript("OnClick", function(self, button)
        if button == "RightButton" then
            if GameTooltip then GameTooltip:Hide() end
            C:WorldTabMenu(self)          -- a menu, like the stock chat tabs; hiding the tab is one choice in it
            return
        end
        C:ViewWorld()
    end)
    t:SetScript("OnEnter", function(self)
        C.lastActivity = now()
        if GameTooltip then
            GameTooltip:SetOwner(self, "ANCHOR_TOP")
            GameTooltip:SetText("World", 1, 1, 1)
            GameTooltip:AddLine("Realm-wide chat, both factions. Type /world <text>.", 0.85, 0.85, 0.85)
            if C:Get("discord") then GameTooltip:AddLine("Linked to the realm's Discord: [D] lines come from there.", 0.45, 0.54, 0.85) end
            if not C:InWorld() then GameTooltip:AddLine("You are not in World chat (Chat settings: Join World chat).", 1, 0.5, 0.3) end
            GameTooltip:AddLine("Right-click for options", 0.6, 0.6, 0.6)
            GameTooltip:Show()
        end
    end)
    t:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
    t:Hide()
    return t
end

-- Look and width of the World tab; Skin.lua's C:LayoutTabs places it after the fixed chat tabs.
function C:PrepWorldTab()
    local t = LeylineChatWorldTab
    if not self:WorldTabShown() then
        if t then t:Hide() end
        return
    end
    t = self:WorldTab()
    local discord = self:Get("discord") and true or false
    if discord then
        if not self:Art(t.mark, "Chat_Discord", nil) then t.mark:SetTexture(0.447, 0.537, 0.855, 1) end
        t.mark:Show()
    else
        t.mark:Hide()
    end
    if t.label.SetFont then t.label:SetFont("Fonts\\FRIZQT__.TTF", 10, self:Theme().flags) end
    if t.label.SetWidth then t.label:SetWidth(0) end
    t.label:ClearAllPoints()
    local tw = t.label:GetStringWidth() or 30
    if tw < 24 then tw = 30 end
    local w
    if discord then
        -- the mark, then the label right after it: never centred over the mark
        t.label:SetPoint("LEFT", t.mark, "RIGHT", 5, 0)
        w = 6 + 10 + 5 + tw + 12
    else
        t.label:SetPoint("CENTER", t, "CENTER", 0, 1)
        w = tw + 20
    end
    if w < 44 then w = 44 end
    t.lcWant = w
    t:SetWidth(w)
    t:Show()
    self:StyleWorldTab()
end

function C:StyleWorldTab(theme)
    local t = LeylineChatWorldTab
    if not t or not t:IsShown() or not self.active then return end
    t.lcUnread = self.world.unread
    t.lcMention = self.world.mention
    self:StyleTab(t, self.worldView and true or false, theme or self:Theme())
end

-- -- The channel cycle (Tab in an empty input line) ---------------------------------------------

function C:ChannelCycle()
    local list = { { "SAY" } }
    if GetNumPartyMembers and (GetNumPartyMembers() or 0) > 0 then table.insert(list, { "PARTY" }) end
    if GetNumRaidMembers and (GetNumRaidMembers() or 0) > 0 then table.insert(list, { "RAID" }) end
    if IsInGuild and IsInGuild() then table.insert(list, { "GUILD" }) end
    local id = self:WorldId()
    if id > 0 then table.insert(list, { "CHANNEL", id }) end
    return list
end

function C:CycleChannel(eb, back)
    if not eb then return false end
    local text = eb.GetText and eb:GetText() or ""
    if text and not strfind(text, "^%s*$") then return false end
    local t = C.EditAttr(eb, "chatType") or "SAY"
    if t == "WHISPER" or t == "BN_WHISPER" then return false end   -- the stock Tab steps through tell targets
    local list = self:ChannelCycle()
    local n = #list
    local cur = 0
    for i, c in ipairs(list) do
        if c[1] == t and (t ~= "CHANNEL" or tostring(c[2]) == tostring(C.EditAttr(eb, "channelTarget"))) then cur = i end
    end
    local nxt
    if back then nxt = cur - 1 else nxt = cur + 1 end
    if nxt < 1 then nxt = n elseif nxt > n then nxt = 1 end
    local pick = list[nxt]
    C.SetEditAttr(eb, "chatType", pick[1])
    if pick[1] == "CHANNEL" then C.SetEditAttr(eb, "channelTarget", pick[2]) end
    if ChatEdit_UpdateHeader then C.Try(ChatEdit_UpdateHeader, eb) end
    return true
end

C.editBoxHooks.cycle = function(eb)
    if not eb.GetScript then return end
    local origTab = eb:GetScript("OnTabPressed")
    eb:SetScript("OnTabPressed", function(self, ...)
        if C.active and C:CycleChannel(self, IsShiftKeyDown and IsShiftKeyDown()) then return end
        if origTab then return origTab(self, ...) end
    end)
end

-- Enter while the World tab is open: you are writing to World.
local openHooked = false
local function hookOpenChat()
    if openHooked then return end
    openHooked = C.Hook("ChatFrame_OpenChat", function(text)
        if C.active and C.worldView and (text == nil or text == "") and C:InWorld() then
            local eb = C.ActiveEditBox()
            if not eb then return end
            C.SetEditAttr(eb, "chatType", "CHANNEL")
            C.SetEditAttr(eb, "channelTarget", C:WorldId())
            if ChatEdit_UpdateHeader then C.Try(ChatEdit_UpdateHeader, eb) end
        end
    end)
end

-- -- /world ----------------------------------------------------------------------------------------

SLASH_LEYLINECHATWORLD1 = "/world"
SlashCmdList["LEYLINECHATWORLD"] = function(msg)
    msg = C.Trim(msg or "")
    local id = C:WorldId()
    if id == 0 then
        if C:Get("worldJoin") then
            C.Print("You are not in World chat yet. It joins a few seconds after you log in (or type /join World).")
        else
            C.Print("World chat is off. Turn on \"Join World chat\" in the chat settings (/lchat) to talk there.")
        end
        return
    end
    if msg == "" then
        if C:WorldTabShown() then C:ViewWorld() end
        if ChatFrame_OpenChat then ChatFrame_OpenChat("/" .. id .. " ") end
        return
    end
    if strlen(msg) > 255 then msg = strsub(msg, 1, 255) end
    SendChatMessage(msg, "CHANNEL", nil, id)
end

-- The World tab's right-click menu: open it, the Discord and join switches, hide the tab (asked, never by accident),
-- all settings.
function C:WorldTabMenu(anchor)
    local items = {
        { text = "World chat", title = true },
        { text = "Open World chat", func = function() C:ViewWorld() end },
        { text = "Show Discord messages", checked = self:Get("discord") and true or false, func = function() C:SetOption("discord", not C:Get("discord")) end },
        { text = "Join World chat", checked = self:Get("worldJoin") and true or false, func = function() C:SetOption("worldJoin", not C:Get("worldJoin")) end },
        { text = "Hide this tab", func = function()
            C:SetOption("worldTab", false)
            C.Print("World tab hidden; World chat shows in the main window as [W]. Tick \"Show the World tab\" in the chat menu (the ... button on the chat bar), or type /lchat world, to bring it back.")
        end },
        { text = "All chat settings...", func = function() C:OpenSettings() end },
    }
    self.menu = UI.Menu(anchor, items)
    return self.menu
end

-- -- Apply -----------------------------------------------------------------------------------------

function C:ApplyWorld()
    hookOpenChat()
    if not self:WorldTabShown() then
        self:LeaveWorld()
        if LeylineChatWorldTab then LeylineChatWorldTab:Hide() end
    else
        self:PrepWorldTab()
        if self.LayoutTabs then self:LayoutTabs() end
        if self.worldView and self.worldFrame and self.SkinFrame then self:SkinFrame(self.worldFrame) end
    end
    if self.worldFrame and not self.active and self.UnskinFrame then self:UnskinFrame(self.worldFrame) end
    self:RouteWorld()
    self:SyncWorldJoin()
end

local ev = CreateFrame("Frame", "LeylineChatWorldEvents")
ev:RegisterEvent("PLAYER_ENTERING_WORLD")
ev:RegisterEvent("CHAT_MSG_CHANNEL")
ev:RegisterEvent("CHAT_MSG_CHANNEL_NOTICE")
-- arg1 msg/notice, arg2 author, arg4 "5. World", arg6 flag, arg8 channel number, arg9 channel name
ev:SetScript("OnEvent", function(self, event, a1, a2, _, a4, _, a6, _, _, a9)
    if event == "CHAT_MSG_CHANNEL" then
        if C:IsWorldChannel(a9) or (not a9 and C:IsWorldChannel(a4)) then C:OnWorldLine(a1, a2, a6) end
    elseif event == "CHAT_MSG_CHANNEL_NOTICE" then
        if C:IsWorldChannel(a9) or (not a9 and C:IsWorldChannel(a4)) then
            if a1 == "YOU_JOINED" or a1 == "YOU_LEFT" or a1 == "YOU_CHANGED" then
                if C.active then C:RouteWorld(); C:PrepWorldTab(); if C.LayoutTabs then C:LayoutTabs() end end
            end
        end
    elseif event == "PLAYER_ENTERING_WORLD" then
        if not C.worldReady and not C.worldSettleAt then C.worldSettleAt = now() + C.WORLD_SETTLE end
    end
end)
ev:SetScript("OnUpdate", function()
    if C.worldSettleAt and now() >= C.worldSettleAt then
        C.worldSettleAt = nil
        C.worldReady = true
        C:RouteWorld()
        C:SyncWorldJoin()
    end
end)
C.worldEvents = ev
