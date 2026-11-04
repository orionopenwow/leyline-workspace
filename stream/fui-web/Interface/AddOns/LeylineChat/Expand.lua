--[[
    Expand.lua  (LeylineChat)
    The expanded chat window: the whole history of the tab you are on (General, World, a whisper window...) in a large
    window you can maximize, with a search box that filters the lines, the newest line kept in view, links you can
    click, and new lines arriving live.

        Open           the expand button on the tab bar (left of Copy), the chat menu, or /lchat expand
        Maximize       the button top right toggles between a large window and nearly the whole screen
        Search         case-insensitive, colour codes ignored; Escape clears it, Escape again closes
        Mouse wheel    scrolls; Shift+wheel jumps to the top or the newest line
        Newest         back to the newest line;  Copy opens the copy window on the same tab

    Lines come from the buffers the chat keeps (frame.lcLines, up to C.MAX_BUFFER; the World tab's own buffer), in
    their own colours (the parchment theme's ink is for the parchment, so it is not used on this dark window).
]]--

local C = LeylineChat
local UI = C.UI
local strlower, strfind = string.lower, string.find

C.EXPAND_LINES = 1200          -- more than any buffer keeps (C.MAX_BUFFER is 1000)
C.EXPAND_SIZE = 0.7            -- share of the screen, normal
C.EXPAND_MAX_SIZE = 0.94       -- share of the screen, maximized
C.EXPAND_POLL = 0.4            -- seconds between checks for new lines

local function tabName(f)
    local t = f and f.GetName and f:GetName() and _G[f:GetName() .. "TabText"]
    local n = t and t.GetText and t:GetText()
    if n and n ~= "" then return n end
    return "Chat"
end

-- the lines of the tab you are on, and its name
function C:ExpandSource()
    if self.worldView and self.world then return self.world.lines, self.WORLD or "World", nil end
    local f = self:Selected() or ChatFrame1
    return (f and f.lcLines) or {}, tabName(f), f
end

local function size(win, share)
    local sw = (UIParent and UIParent:GetWidth()) or 1024
    local sh = (UIParent and UIParent:GetHeight()) or 768
    local w, h = math.floor(sw * share), math.floor(sh * share)
    if w < 620 then w = 620 end
    if h < 400 then h = 400 end
    win:SetWidth(w)
    win:SetHeight(h)
    win:ClearAllPoints()
    win:SetPoint("CENTER", UIParent, "CENTER", 0, 0)
end

function C:ExpandWindow()
    if self.expandWin then return self.expandWin end
    local f = UI.Window("LeylineChatExpanded", { width = 760, height = 500, title = "Chat history" })
    f:Hide()

    f.head = UI.Label(f, 11, UI.ACCENT)
    f.head:SetPoint("TOPLEFT", f, "TOPLEFT", 14, -34)
    f.count = UI.Label(f, 10, UI.MUTED)
    f.count:SetPoint("LEFT", f.head, "RIGHT", 10, 0)

    f.max = UI.Button(f, "Maximize", 90, 20, function() C:ToggleExpandSize() end)
    f.max:SetPoint("TOPRIGHT", f, "TOPRIGHT", -12, -30)
    f.search = UI.SearchBox(f, { name = "LeylineChatExpandedSearch", width = 200, placeholder = "Search this tab",
        onChange = function() if f:IsShown() then C:FillExpanded() end end })
    f.search:SetPoint("RIGHT", f.max, "LEFT", -8, 0)
    f.search:SetScript("OnEscapePressed", function(self)
        if (self:GetText() or "") ~= "" then self:SetText("") else f:Hide() end
    end)

    local well = CreateFrame("Frame", nil, f)
    UI.Skin(well, "inset")
    well:SetPoint("TOPLEFT", f, "TOPLEFT", 10, -56)
    well:SetPoint("BOTTOMRIGHT", f, "BOTTOMRIGHT", -10, 40)
    f.well = well

    local m = CreateFrame("ScrollingMessageFrame", "LeylineChatExpandedLines", well)
    m:SetPoint("TOPLEFT", well, "TOPLEFT", 8, -6)
    m:SetPoint("BOTTOMRIGHT", well, "BOTTOMRIGHT", -8, 6)
    m:SetFontObject(ChatFontNormal or GameFontHighlightSmall)
    if m.SetJustifyH then m:SetJustifyH("LEFT") end
    if m.SetFading then m:SetFading(false) end
    if m.SetMaxLines then m:SetMaxLines(C.EXPAND_LINES) end
    m:EnableMouse(true)
    if m.SetHyperlinksEnabled then m:SetHyperlinksEnabled(true) end
    if m.EnableMouseWheel then m:EnableMouseWheel(true) end
    m:SetScript("OnMouseWheel", function(self, delta)
        local up = (delta or 0) > 0
        if IsShiftKeyDown and IsShiftKeyDown() then
            if up then self:ScrollToTop() else self:ScrollToBottom() end
        elseif up then self:ScrollUp(); self:ScrollUp(); self:ScrollUp()
        else self:ScrollDown(); self:ScrollDown(); self:ScrollDown() end
    end)
    m:SetScript("OnHyperlinkClick", function(self, link, text, button)
        if ChatFrame_OnHyperlinkShow then ChatFrame_OnHyperlinkShow(self, link, text, button)
        elseif SetItemRef then SetItemRef(link, text, button, self) end
    end)
    f.lines = m

    f.empty = UI.Label(well, 11, UI.MUTED)
    f.empty:SetPoint("CENTER", well, "CENTER", 0, 0)

    f.newest = UI.Button(f, "Newest", 90, 22, function() m:ScrollToBottom() end)
    f.newest:SetPoint("BOTTOMLEFT", f, "BOTTOMLEFT", 12, 10)
    f.copy = UI.Button(f, "Copy", 80, 22, function() C:OpenCopy(f.frame) end)
    f.copy:SetPoint("LEFT", f.newest, "RIGHT", 6, 0)
    f.close = UI.Button(f, "Close", 80, 22, function() f:Hide() end)
    f.close:SetPoint("BOTTOMRIGHT", f, "BOTTOMRIGHT", -12, 10)
    f.hint = UI.Label(f, 9, UI.MUTED)
    f.hint:SetPoint("LEFT", f.copy, "RIGHT", 12, 0)
    f.hint:SetText("Mouse wheel scrolls; Shift+wheel jumps to the top or the newest line.")

    -- new lines arrive while it is open
    f.since = 0
    f:SetScript("OnUpdate", function(self, elapsed)
        self.since = self.since + (elapsed or 0)
        if self.since < C.EXPAND_POLL then return end
        self.since = 0
        local lines = C:ExpandSource()
        local n = #lines
        if lines ~= self.source or n ~= self.seenN or lines[n] ~= self.seenLast then C:FillExpanded() end
    end)
    f:SetScript("OnHide", function() if f.search.ClearFocus then f.search:ClearFocus() end end)
    size(f, C.EXPAND_SIZE)
    self.expandWin = f
    return f
end

-- (re)print the lines of the current tab that match the search
function C:FillExpanded()
    local f = self.expandWin
    if not f then return end
    local lines, name, frame = self:ExpandSource()
    local n = #lines
    local q = strlower(C.Trim(f.search:GetText() or ""))
    local m = f.lines
    m:Clear()
    local shown = 0
    for i = 1, n do
        local e = lines[i]
        if e and e.text and (q == "" or strfind(strlower(C.Strip(e.text) or ""), q, 1, true)) then
            m:AddMessage(e.text, e.r or 1, e.g or 1, e.b or 1)
            shown = shown + 1
        end
    end
    m:ScrollToBottom()
    f.source, f.seenN, f.seenLast, f.frame = lines, n, lines[n], frame
    f.head:SetText(name)
    if q == "" then f.count:SetText(n .. (n == 1 and " line" or " lines"))
    else f.count:SetText(shown .. " of " .. n .. " lines match") end
    if shown == 0 then
        f.empty:SetText(n == 0 and "Nothing here yet." or "No line matches your search.")
        f.empty:Show()
    else
        f.empty:Hide()
    end
    self.expandShown = shown
end

function C:ToggleExpandSize()
    local f = self:ExpandWindow()
    f.maximized = not f.maximized
    size(f, f.maximized and C.EXPAND_MAX_SIZE or C.EXPAND_SIZE)
    f.max:SetText(f.maximized and "Restore" or "Maximize")
end

function C:OpenExpanded()
    local f = self:ExpandWindow()
    if f:IsShown() then f:Hide() return f end
    f.search:SetText("")
    self:FillExpanded()
    f:Show()
    return f
end
