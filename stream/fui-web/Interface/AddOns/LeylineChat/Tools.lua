--[[
    Tools.lua  (LeylineChat)
        Copy chat      the last 200 lines of a tab in a selectable box (tab bar button, /lchat copy)
        Link box       clicking a web link in chat opens it, highlighted, ready for Ctrl+C
        Typed history  lines you sent come back with the up/down arrows, even after a reload (per character)

    The 1.12 addon's own whisper tabs are gone: 3.3.5 has whisper windows of its own (the "Whisper tabs" setting
    switches the game's whisperMode between pop-out and inline).
]]--

local C = LeylineChat
local UI = C.UI
local strsub, strfind = string.sub, string.find

C.COPY_LINES = 200

-- -- Copy chat ---------------------------------------------------------------------------------

function C:CopyWindow()
    if self.copyWin then return self.copyWin end
    local f = UI.Window("LeylineChatCopy", { width = 560, height = 380, title = "Copy chat" })
    local hint = UI.Label(f, 10, UI.MUTED)
    hint:SetPoint("TOPLEFT", f, "TOPLEFT", 14, -32)
    hint:SetText("The last 200 lines. Select what you need and press Ctrl+C.")
    f.hint = hint

    local well = CreateFrame("Frame", nil, f)
    UI.Skin(well, "inset")
    well:SetPoint("TOPLEFT", f, "TOPLEFT", 10, -48)
    well:SetPoint("BOTTOMRIGHT", f, "BOTTOMRIGHT", -10, 40)

    local ok, sf = pcall(CreateFrame, "ScrollFrame", "LeylineChatCopyScroll", well, "UIPanelScrollFrameTemplate")
    if not ok or not sf then sf = CreateFrame("ScrollFrame", "LeylineChatCopyScroll", well) end
    sf:SetPoint("TOPLEFT", well, "TOPLEFT", 6, -6)
    sf:SetPoint("BOTTOMRIGHT", well, "BOTTOMRIGHT", -26, 6)
    local box = CreateFrame("EditBox", "LeylineChatCopyBox", sf)
    box:SetMultiLine(true)
    box:SetMaxLetters(0)
    box:SetAutoFocus(false)
    box:SetWidth(500)
    box:SetHeight(280)
    box:SetFontObject(ChatFontNormal or GameFontHighlightSmall)
    box:SetScript("OnEscapePressed", function() f:Hide() end)
    box:SetScript("OnTextChanged", function(self)
        if ScrollingEdit_OnTextChanged then C.Try(ScrollingEdit_OnTextChanged, self, sf) end
        -- read-only: typing puts the text back
        if f.text and self:GetText() ~= f.text and not f.setting then
            f.setting = true
            self:SetText(f.text)
            f.setting = nil
        end
    end)
    box:SetScript("OnCursorChanged", function(self, x, y, w, h)
        if ScrollingEdit_OnCursorChanged then C.Try(ScrollingEdit_OnCursorChanged, self, x, y, w, h) end
    end)
    box:SetScript("OnUpdate", function(self, elapsed)
        if ScrollingEdit_OnUpdate then C.Try(ScrollingEdit_OnUpdate, self, elapsed, sf) end
        if f.toBottom then
            f.toBottom = f.toBottom - 1
            if sf.UpdateScrollChildRect then sf:UpdateScrollChildRect() end
            if sf.GetVerticalScrollRange then sf:SetVerticalScroll(sf:GetVerticalScrollRange() or 0) end
            if f.toBottom <= 0 then f.toBottom = nil end
        end
    end)
    sf:SetScrollChild(box)
    f.box, f.scroll = box, sf

    local all = UI.Button(f, "Select all", 110, 22, function()
        box:SetFocus()
        box:HighlightText()
    end)
    all:SetPoint("BOTTOMLEFT", f, "BOTTOMLEFT", 12, 10)
    local close = UI.Button(f, "Close", 80, 22, function() f:Hide() end)
    close:SetPoint("BOTTOMRIGHT", f, "BOTTOMRIGHT", -12, 10)
    f.count = UI.Label(f, 10, UI.MUTED)
    f.count:SetPoint("LEFT", all, "RIGHT", 10, 0)
    f:Hide()
    self.copyWin = f
    return f
end

-- Plain text of the last n lines of a chat tab (or the World view).
function C:CopyText(frame, n)
    n = n or self.COPY_LINES
    local lines
    if not frame and self.worldView and self.world then
        lines = self.world.lines
    else
        frame = frame or self:Selected() or ChatFrame1
        lines = frame and frame.lcLines
    end
    lines = lines or {}
    local total = #lines
    local out = {}
    for i = math.max(1, total - n + 1), total do
        table.insert(out, C.Strip(lines[i].text))
    end
    return table.concat(out, "\n"), #out
end

function C:OpenCopy(frame)
    local f = self:CopyWindow()
    local text, count = self:CopyText(frame)
    f.text = text
    f.setting = true
    f.box:SetText(text)
    f.setting = nil
    f.count:SetText(count == 1 and "1 line" or (count .. " lines"))
    f.toBottom = 3
    f:Show()
    f.box:SetFocus()
    f.box:HighlightText()
    return f
end

-- -- Link box ----------------------------------------------------------------------------------

function C:LinkWindow()
    if self.linkWin then return self.linkWin end
    local f = UI.Window("LeylineChatLink", { width = 400, height = 96, title = "Copy link" })
    local e = CreateFrame("EditBox", "LeylineChatLinkBox", f)
    UI.Skin(e, "inset")
    e:SetPoint("TOPLEFT", f, "TOPLEFT", 12, -34)
    e:SetPoint("TOPRIGHT", f, "TOPRIGHT", -12, -34)
    e:SetHeight(22)
    e:SetAutoFocus(false)
    e:SetMaxLetters(0)
    e:SetFontObject(ChatFontNormal or GameFontHighlightSmall)
    if e.SetTextInsets then e:SetTextInsets(6, 6, 0, 0) end
    e:SetTextColor(0.55, 0.78, 1)
    e:SetScript("OnEscapePressed", function() f:Hide() end)
    e:SetScript("OnEnterPressed", function() f:Hide() end)
    e:SetScript("OnTextChanged", function(self)
        if f.url and self:GetText() ~= f.url and not f.setting then
            f.setting = true
            self:SetText(f.url)
            self:HighlightText()
            f.setting = nil
        end
    end)
    local hint = UI.Label(f, 10, UI.MUTED)
    hint:SetPoint("TOPLEFT", e, "BOTTOMLEFT", 2, -8)
    hint:SetText("Press Ctrl+C to copy it, then Escape. Links open outside the game.")
    f.box = e
    f:Hide()
    self.linkWin = f
    return f
end

function C:OpenLink(url)
    local f = self:LinkWindow()
    f.url = url
    f.setting = true
    f.box:SetText(url)
    f.setting = nil
    f:Show()
    f.box:SetFocus()
    f.box:HighlightText()
    return f
end

-- "url:" links never reach the stock code (its tooltip has no such link type).
local origSetItemRef = SetItemRef
function SetItemRef(link, text, button, chatFrame)
    if type(link) == "string" and strsub(link, 1, 4) == "url:" then
        C:OpenLink(strsub(link, 5))
        return
    end
    if origSetItemRef then return origSetItemRef(link, text, button, chatFrame) end
end

-- -- Typed history ---------------------------------------------------------------------------------
-- The stock edit box keeps what you send for the up/down arrows, but forgets it on a reload. Lines added to its
-- history are also saved per character (LeylineChatCharDB.history) and put back at login.

C.MAX_HISTORY = 50

function C:History() return self:CharDB().history end

function C:Remember(line)
    if not self.active or not self:Get("typedHistory") then return end
    if type(line) ~= "string" or line == "" or strfind(line, "^%s*$") then return end
    local h = self:History()
    if h[#h] == line then return end
    table.insert(h, line)
    while #h > self.MAX_HISTORY do table.remove(h, 1) end
end

C.editBoxHooks.history = function(eb)
    if not eb.AddHistoryLine then return end
    local origAdd = eb.AddHistoryLine
    eb.lcAddHistoryStock = origAdd
    eb.AddHistoryLine = function(self, line)
        C:Remember(line)
        return origAdd(self, line)
    end
end

function C:RestoreHistory()
    if self.historyLoaded or not self:Get("typedHistory") then return end
    local eb = C.MainEditBox()
    if not eb then return end
    self.historyLoaded = 0
    local add = eb.lcAddHistoryStock or eb.AddHistoryLine
    for _, line in ipairs(self:History()) do
        if add then add(eb, line) end
        self.historyLoaded = self.historyLoaded + 1
    end
end

-- -- Apply -----------------------------------------------------------------------------------------

function C:ApplyTools()
    self:HookEditBoxes()
    if self.active then self:RestoreHistory() end
end
