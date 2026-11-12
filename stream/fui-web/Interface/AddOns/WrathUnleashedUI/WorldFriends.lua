-- WorldFriends.lua (3.3.5a API): the Leyline tab of the game's own Friends window. Made with the game's own pieces (the
-- Friends window's tab template, its panel buttons, input box and fonts), so it looks like the Friends window it sits in;
-- with Forever UI loaded, its buttons take Forever UI's own button skin.
--
-- On a world on your own PC (Single player, the original world: the realm half is lua_scripts/ov_worldfriends.lua):
--   the host:   "Invite link": this world's invite link in a box, selected for Ctrl-C (the game can't write the
--               clipboard); "Invite a friend": asks the Leyline launcher for a new link (the launcher answers within
--               seconds while the world runs); "Friends with a character here": each friend's characters, online or not,
--               trusted or not. Friends keep their characters: their login here is theirs for good.
--   a friend:   that their character here is kept, and how to come back ("Join your friend again").
-- Everywhere: "Leyline friends" opens the Leyline Friends panel (Social.lua). On another realm the tab says what it is
-- for and keeps that button.
--
-- Transport: the Open Mod API (OVMOD, mod "wufriends", whispered to yourself): info / invite; the realm answers info,
-- ok, err (ov_worldfriends.lua has the format). Only the realm's whispers to you count.

local W = WrathUnleashedUI
local F = { status = nil, answered = false, line = "" }
W.WorldFriends = F

local MOD = "wufriends"
local ROWS = 7
local POLL = 4            -- seconds between info asks while the tab shows
local SEND_EVERY = 1
local NO_ANSWER = 4       -- seconds: no answer by then = not one of your own worlds

local function now() return (GetTime and GetTime()) or 0 end

-- ---------------------------------------------------------------- talking to the realm

local lastSend = -1e9
function F.Send(action)
    if not SendAddonMessage or not UnitName then return false end
    if action ~= "info" and now() - lastSend < SEND_EVERY then return false end
    if action ~= "info" then lastSend = now() end
    F.asked = F.asked or now()
    pcall(SendAddonMessage, "OVMOD", MOD .. "\t" .. action .. "\t1/1\t", "WHISPER", UnitName("player"))
    return true
end

-- key=value lines; f= lines are friends: login, trusted, online, Name:level,...
function F.Parse(data)
    local s = { friends = {} }
    for line in (data or ""):gmatch("[^\n]+") do
        local k, v = line:match("^(%w+)=(.*)$")
        if k == "f" then
            local user, trusted, online, chars = v:match("^([^\t]*)\t([01])\t([01])\t(.*)$")
            if user then
                local list = {}
                for name, level in chars:gmatch("(%a+):(%d+)") do list[#list + 1] = { name = name, level = tonumber(level) } end
                s.friends[#s.friends + 1] = { user = user, trusted = trusted == "1", online = online == "1", chars = list }
            end
        elseif k then
            s[k] = v
        end
    end
    s.host = s.role == "host"
    s.sharing = s.sharing == "1"
    s.asking = s.asking == "1"
    return s
end

local refresh   -- set once the panel is made

function F.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or channel ~= "WHISPER" or type(msg) ~= "string" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local kind, data = msg:match("^wufriends\t(%a+)\t1/1\t(.*)$")
    if not kind then return end
    F.answered = true
    if kind == "info" then
        F.status = F.Parse(data)
    elseif kind == "ok" or kind == "err" then
        F.line = (data:gsub("|", ""))
        F.lineKind = kind
        F.Send("info")
    end
    if refresh then refresh() end
end

-- ---------------------------------------------------------------- the look: the game's own templates, Forever UI's skin

local function make(kind, name, parent, templates)
    for _, t in ipairs(templates) do
        local ok, f = pcall(CreateFrame, kind, name, parent, t)
        if ok and f then return f end
    end
    return CreateFrame(kind, name, parent)
end

local fuiFonts
local function skin(b)
    if type(ForeverUI) == "table" and type(ForeverUI.Templates) == "table" and type(ForeverUI.Templates.ThreeSliceButton) == "function" then
        fuiFonts = fuiFonts or { GameFontHighlight, GameFontHighlight, GameFontDisable }
        pcall(ForeverUI.Templates.ThreeSliceButton, b, "128-redbutton", fuiFonts)
    end
end

local function button(parent, name, text, w, onClick)
    local b = make("Button", name, parent, { "UIPanelButtonTemplate" })
    b:SetWidth(w); b:SetHeight(22)
    b:SetText(text)
    b:SetScript("OnClick", onClick)
    skin(b)
    return b
end

local function label(parent, font, text)
    local fs = parent:CreateFontString(nil, "OVERLAY", font)
    fs:SetJustifyH("LEFT")
    fs:SetText(text or "")
    return fs
end

-- ---------------------------------------------------------------- the panel

local panel
F.Panel = function() return panel end

local function linkBox(parent)
    local e = make("EditBox", "WrathUnleashedWorldFriendsLink", parent, { "InputBoxTemplate" })
    e:SetWidth(290); e:SetHeight(20)
    e:SetAutoFocus(false)
    e:SetFontObject(GameFontHighlightSmall)
    e.fixed = ""
    -- read only: whatever is typed goes back to the link
    e:SetScript("OnTextChanged", function(self) if self:GetText() ~= self.fixed then self:SetText(self.fixed); self:HighlightText() end end)
    e:SetScript("OnEditFocusGained", function(self) self:HighlightText() end)
    e:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
    e:SetScript("OnEnterPressed", function(self) self:ClearFocus() end)
    return e
end

-- Put the link in the box, selected: Ctrl-C copies it.
function F.SelectLink()
    if not panel then return false end
    local e = panel.link
    if (e.fixed or "") == "" then return false end
    e:SetText(e.fixed); e:SetFocus(); e:HighlightText()
    F.line = "Press Ctrl-C to copy the link, then send it to your friend."
    F.lineKind = "ok"
    if refresh then refresh() end
    return true
end

function F.Invite()
    if not F.Send("invite") then return false end
    F.line = "Asking the Leyline launcher for a new invite..."
    F.lineKind = "ok"
    if refresh then refresh() end
    return true
end

local function charText(f)
    local names = {}
    for _, c in ipairs(f.chars) do names[#names + 1] = c.name .. " (" .. c.level .. ")" end
    if #names == 0 then return "No character yet" end
    return table.concat(names, ", ")
end

local function build()
    if panel or not FriendsFrame then return panel end
    panel = CreateFrame("Frame", "WrathUnleashedWorldFriendsPanel", FriendsFrame)
    panel:SetPoint("TOPLEFT", FriendsFrame, "TOPLEFT", 22, -76)
    panel:SetPoint("BOTTOMRIGHT", FriendsFrame, "BOTTOMRIGHT", -42, 84)
    panel:Hide()

    panel.title = label(panel, "GameFontNormalLarge", "Play together")
    panel.title:SetPoint("TOPLEFT", panel, "TOPLEFT", 4, -2)
    panel.sub = label(panel, "GameFontHighlightSmall", "")
    panel.sub:SetPoint("TOPLEFT", panel.title, "BOTTOMLEFT", 0, -6)
    panel.sub:SetWidth(300)

    -- the host's: the link, select it, invite
    panel.linkLabel = label(panel, "GameFontNormal", "Invite link")
    panel.linkLabel:SetPoint("TOPLEFT", panel.sub, "BOTTOMLEFT", 0, -12)
    panel.link = linkBox(panel)
    panel.link:SetPoint("TOPLEFT", panel.linkLabel, "BOTTOMLEFT", 6, -4)
    panel.select = button(panel, "WrathUnleashedWorldFriendsSelect", "Select link", 110, function() F.SelectLink() end)
    panel.select:SetPoint("TOPLEFT", panel.link, "BOTTOMLEFT", -6, -6)
    panel.invite = button(panel, "WrathUnleashedWorldFriendsInvite", "Invite a friend", 130, function() F.Invite() end)
    panel.invite:SetPoint("LEFT", panel.select, "RIGHT", 8, 0)
    panel.line = label(panel, "GameFontHighlightSmall", "")
    panel.line:SetPoint("TOPLEFT", panel.select, "BOTTOMLEFT", 0, -6)
    panel.line:SetWidth(300)

    -- friends with a character here
    panel.listLabel = label(panel, "GameFontNormal", "Friends with a character here")
    panel.listLabel:SetPoint("TOPLEFT", panel.line, "BOTTOMLEFT", 0, -12)
    panel.rows = {}
    local prev = panel.listLabel
    for i = 1, ROWS do
        local r = CreateFrame("Frame", nil, panel)
        r:SetWidth(300); r:SetHeight(26)
        r:SetPoint("TOPLEFT", prev, "BOTTOMLEFT", 0, i == 1 and -4 or 0)
        r.name = label(r, "GameFontHighlight", "")
        r.name:SetPoint("TOPLEFT", r, "TOPLEFT", 4, 0)
        r.name:SetWidth(296)
        r.state = label(r, "GameFontHighlightSmall", "")
        r.state:SetPoint("TOPLEFT", r.name, "BOTTOMLEFT", 0, -1)
        r:Hide()
        panel.rows[i] = r
        prev = r
    end

    -- everywhere: the Leyline Friends panel
    panel.social = button(panel, "WrathUnleashedWorldFriendsSocial", "Leyline friends", 130, function()
        if W.ToggleSocial then W.ToggleSocial() end
    end)
    panel.social:SetPoint("BOTTOMLEFT", panel, "BOTTOMLEFT", 0, 0)

    local acc = 0
    panel:SetScript("OnShow", function() F.asked = nil; F.Send("info"); acc = 0; refresh() end)
    panel:SetScript("OnUpdate", function(_, elapsed)
        acc = acc + (elapsed or 0)
        if acc >= POLL then acc = 0; F.Send("info"); refresh() end
    end)
    refresh()
    return panel
end
F.Build = build

refresh = function()
    if not panel then return end
    local s = F.status
    local own = s ~= nil
    local host = own and s.host
    for _, k in ipairs({ "linkLabel", "link", "select", "invite", "line", "listLabel" }) do
        if host then panel[k]:Show() else panel[k]:Hide() end
    end
    if not own then
        if F.asked and now() - F.asked >= NO_ANSWER and not F.answered then
            panel.sub:SetText("This tab is for your own Leyline worlds (Single player, the original world): invite a friend "
                .. "and they keep their character there. Your Leyline friends are a click away below.")
        else
            panel.sub:SetText("Asking this world...")
        end
    elseif host then
        panel.sub:SetText(s.sharing and "Your world is shared: friends with your link can come in. Each friend's character here is theirs for good."
            or "Invite a friend to play in your world. Each friend keeps their character here for next time.")
    else
        panel.sub:SetText("You're on your friend's world, and your character here is kept. To come back: open your friend's "
            .. "link again, or press Join again on their world in the Leyline launcher (Single player, Friends' worlds).")
    end
    for _, r in ipairs(panel.rows) do r:Hide() end
    if not host then return end
    local link = s.link or ""
    panel.link.fixed = link
    panel.link:SetText(link ~= "" and link or "")
    if link == "" then panel.select:Disable() else panel.select:Enable() end
    if s.asking then panel.invite:Disable() else panel.invite:Enable() end
    local line = F.line
    if s.asking then line = "Asking the Leyline launcher for a new invite..." end
    if line == "" and s.answer and s.answer ~= "" then line = s.answer:gsub("^ok:", ""):gsub("^err:", "") end
    if line == "" and link == "" then line = "No link yet: press Invite a friend." end
    panel.line:SetText(line)
    local n = 0
    for _, f in ipairs(s.friends) do
        n = n + 1
        local r = panel.rows[n]
        if not r then break end
        r.name:SetText(charText(f))
        local state = f.online and "|cff40ff40Online|r" or "|cff9d9d9dOffline|r"
        if f.trusted then state = state .. "  Trusted: comes back any time" end
        r.state:SetText(state)
        r:Show()
    end
    if n == 0 then
        panel.rows[1].name:SetText("No friends here yet.")
        panel.rows[1].state:SetText("A friend who joins with your link keeps their character for next time.")
        panel.rows[1]:Show()
    end
end
F.Refresh = function() if refresh then refresh() end end

-- ---------------------------------------------------------------- the tab on the Friends window

local function others(show)
    for _, name in ipairs(type(FRIENDSFRAME_SUBFRAMES) == "table" and FRIENDSFRAME_SUBFRAMES
        or { "FriendsListFrame", "IgnoreListFrame", "PendingListFrame", "WhoFrame", "GuildFrame", "ChannelFrame", "RaidFrame" }) do
        local f = _G[name]
        if f and f.Hide and not show then f:Hide() end
    end
    if FriendsTabHeader then
        if show then FriendsTabHeader:Show() else FriendsTabHeader:Hide() end
    end
end

function F.Select()
    if not (F.tab and build()) then return false end
    if PanelTemplates_SetTab then PanelTemplates_SetTab(FriendsFrame, F.tab:GetID()) end
    FriendsFrame.selectedTab = F.tab:GetID()
    others(false)
    panel:Show()
    if PlaySound then PlaySound("igCharacterInfoTab") end
    return true
end

function F.Open()
    if not FriendsFrame then return false end
    if not FriendsFrame:IsShown() then
        if ShowUIPanel then ShowUIPanel(FriendsFrame) else FriendsFrame:Show() end
    end
    return F.Select()
end

function F.Hook()
    if F.tab or not FriendsFrame then return F.tab end
    local n = (tonumber(FriendsFrame.numTabs) or 4) + 1
    while _G["FriendsFrameTab" .. n] do n = n + 1 end
    local tab = make("Button", "FriendsFrameTab" .. n, FriendsFrame, { "FriendsFrameTabTemplate", "CharacterFrameTabButtonTemplate" })
    tab:SetID(n)
    tab:SetText("Leyline")
    local prev = _G["FriendsFrameTab" .. (n - 1)]
    if prev then tab:SetPoint("LEFT", prev, "RIGHT", -14, 0) else tab:SetPoint("TOPLEFT", FriendsFrame, "BOTTOMLEFT", 14, 76) end
    tab:SetScript("OnClick", function() F.Select() end)
    if PanelTemplates_TabResize then pcall(PanelTemplates_TabResize, tab, 0) end
    if PanelTemplates_SetNumTabs then PanelTemplates_SetNumTabs(FriendsFrame, n) end
    FriendsFrame.numTabs = n
    F.tab = tab
    -- another tab picked: ours hides, the game's own panels come back (FriendsFrame_Update shows the right one)
    local function update()
        if FriendsFrame.selectedTab ~= tab:GetID() then
            if panel and panel:IsShown() then panel:Hide(); others(FriendsFrame.selectedTab == 1) end
        elseif panel then   -- the window opened again on this tab
            others(false); panel:Show()
        end
    end
    if hooksecurefunc and FriendsFrame_Update then hooksecurefunc("FriendsFrame_Update", update) end
    if FriendsFrame.HookScript then FriendsFrame:HookScript("OnHide", function() if panel then panel:Hide() end end) end
    F.update = update
    -- the older Leyline button on the window: this tab has it now
    local old = _G["WrathUnleashedSocialFriendsButton"]
    if old then old:Hide() end
    build()
    return tab
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:RegisterEvent("PLAYER_ENTERING_WORLD")
listener:SetScript("OnEvent", function(_, event, prefix, msg, channel, sender)
    if event == "CHAT_MSG_ADDON" then F.OnMessage(prefix, msg, channel, sender) else F.Hook() end
end)
F.Hook()

SLASH_WUWORLDFRIENDS1, SLASH_WUWORLDFRIENDS2 = "/together", "/invitefriend"
SlashCmdList["WUWORLDFRIENDS"] = function() F.Open() end
