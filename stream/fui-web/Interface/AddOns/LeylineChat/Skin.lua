--[[
    Skin.lua  (LeylineChat)
    The look. Styles the stock chat frames in place, never replacing them:

        panel     a themed face behind each chat frame (opacity slider), a 1 px light top edge, the theme's edge art
                  and a soft shadow underneath. The main window's panel also holds the input line.
        tabs      flat tabs joined to the panel's top; the active one has an accent underline and new lines show a
                  small dot (the stock tab glow no longer flashes). The game's dock still orders the tabs (and
                  scrolls the whisper windows' tabs); the World tab sits after the fixed tabs.
        input     the edit box inset into the panel, the channel name in its own colour.
        buttons   the stock button column (scroll arrows, menu) is hidden; a thin scrollbar shows on hover, a "jump
                  to latest" arrow when scrolled up, and the emote menu, Copy, Expand and settings sit on the tab bar.
        fade      after 20 s (or a minute) of quiet the frame chrome fades; hover or a new line brings it back.
        resize    the corner grip resizes; moves and resizes snap to a 4 px grid and to the screen edges.

    Forever UI: its chat skin is left loaded (its Alt+drag text selection still works), but the two pieces it draws
    over the chat, its scroll bar and its scroll-to-bottom button, stand down while this skin is on; our hooks run
    after its own, so our tab sizes and dock place win. Everything is undone by C:Unskin().
]]--

local C = LeylineChat
local NOOP = function() end
local floor = math.floor
local now = C.Now

C.PAD = { left = 6, right = 8, top = 4, bottom = 6, input = 28 }
C.TAB_H = 20
C.TAB_GAP = 1
C.BUTTONS_W = 64
C.skinned = {}

local STOCK_TEXTURES = { "Background", "TopLeftTexture", "BottomLeftTexture", "TopRightTexture",
    "BottomRightTexture", "LeftTexture", "RightTexture", "BottomTexture", "TopTexture" }
-- Stock buttons (the 3.3.5 button column and resize corner) and Forever UI's two pieces, by frame-name suffix.
local STOCK_FRAMES = { "ButtonFrame", "ResizeButton", "UpButton", "DownButton", "BottomButton",
    "ForeverScrollBar", "ForeverScrollToBottom" }
local TAB_PARTS = { "Left", "Middle", "Right", "SelectedLeft", "SelectedMiddle", "SelectedRight",
    "HighlightLeft", "HighlightMiddle", "HighlightRight", "Glow", "Flash" }

local function stockTextures()
    if type(CHAT_FRAME_TEXTURES) == "table" and CHAT_FRAME_TEXTURES[1] then return CHAT_FRAME_TEXTURES end
    return STOCK_TEXTURES
end

-- Hide a stock region or frame while the skin is on; the stock fades call Show(), so that is muted.
local function suppress(o)
    if not o or o.lcSuppressed then return end
    o.lcSuppressed = { shown = o:IsShown() and true or false }
    o:Hide()
    o.Show = NOOP
end
local function unsuppress(o)
    local s = o and o.lcSuppressed
    if not s then return end
    o.Show = nil
    o.lcSuppressed = nil
    if s.shown then o:Show() end
end
C.Suppress, C.Unsuppress = suppress, unsuppress

local function savePoints(f)
    local pts = {}
    for i = 1, (f.GetNumPoints and f:GetNumPoints() or 0) do
        local p, rel, rp, x, y = f:GetPoint(i)
        if p then table.insert(pts, { p, rel, rp, x, y }) end
    end
    return pts
end
local function loadPoints(f, pts)
    if not pts or not pts[1] then return end
    f:ClearAllPoints()
    for _, p in ipairs(pts) do f:SetPoint(p[1], p[2], p[3], p[4], p[5]) end
end
C.SavePoints, C.LoadPoints = savePoints, loadPoints

local function artName(kind, theme)
    return "Chat_" .. kind .. (theme.suffix or "")
end

-- -- 9-slice pieces (Chat_Edge*, Chat_Shadow: 64x64, 16 px corners) -------------------------------

local SLICES = {
    { "TL", 0, 0.25, 0, 0.25 }, { "T", 0.25, 0.75, 0, 0.25 }, { "TR", 0.75, 1, 0, 0.25 },
    { "L", 0, 0.25, 0.25, 0.75 }, { "R", 0.75, 1, 0.25, 0.75 },
    { "BL", 0, 0.25, 0.75, 1 }, { "B", 0.25, 0.75, 0.75, 1 }, { "BR", 0.75, 1, 0.75, 1 },
}

local function newSlice(parent, layer)
    local s = {}
    for _, d in ipairs(SLICES) do
        local t = parent:CreateTexture(nil, layer)
        s[d[1]] = t
    end
    return s
end

-- Lay the pieces around rect (outset o, corner size n, shifted dy). Returns true if the art loaded.
local function setSlice(s, rect, name, n, o, dy)
    local loaded = C:Art(s.TL, name, nil)
    for _, d in ipairs(SLICES) do
        local t = s[d[1]]
        if loaded then
            if t ~= s.TL then C:Art(t, name, nil) end
            t:SetTexCoord(d[2], d[3], d[4], d[5])
            t:Show()
        else
            t:Hide()
        end
    end
    if not loaded then return false end
    dy = dy or 0
    s.TL:ClearAllPoints(); s.TL:SetPoint("TOPLEFT", rect, "TOPLEFT", -o, o + dy); s.TL:SetWidth(n); s.TL:SetHeight(n)
    s.TR:ClearAllPoints(); s.TR:SetPoint("TOPRIGHT", rect, "TOPRIGHT", o, o + dy); s.TR:SetWidth(n); s.TR:SetHeight(n)
    s.BL:ClearAllPoints(); s.BL:SetPoint("BOTTOMLEFT", rect, "BOTTOMLEFT", -o, -o + dy); s.BL:SetWidth(n); s.BL:SetHeight(n)
    s.BR:ClearAllPoints(); s.BR:SetPoint("BOTTOMRIGHT", rect, "BOTTOMRIGHT", o, -o + dy); s.BR:SetWidth(n); s.BR:SetHeight(n)
    s.T:ClearAllPoints(); s.T:SetPoint("TOPLEFT", s.TL, "TOPRIGHT", 0, 0); s.T:SetPoint("BOTTOMRIGHT", s.TR, "BOTTOMLEFT", 0, 0)
    s.B:ClearAllPoints(); s.B:SetPoint("TOPLEFT", s.BL, "TOPRIGHT", 0, 0); s.B:SetPoint("BOTTOMRIGHT", s.BR, "BOTTOMLEFT", 0, 0)
    s.L:ClearAllPoints(); s.L:SetPoint("TOPLEFT", s.TL, "BOTTOMLEFT", 0, 0); s.L:SetPoint("BOTTOMRIGHT", s.BL, "TOPRIGHT", 0, 0)
    s.R:ClearAllPoints(); s.R:SetPoint("TOPLEFT", s.TR, "BOTTOMLEFT", 0, 0); s.R:SetPoint("BOTTOMRIGHT", s.BR, "TOPRIGHT", 0, 0)
    return true
end

local function sliceAlpha(s, a)
    for _, d in ipairs(SLICES) do s[d[1]]:SetAlpha(a) end
end

-- -- Which frames, which tabs ----------------------------------------------------------------------

local function tabOf(f) return f and f.GetName and f:GetName() and _G[f:GetName() .. "Tab"] end
C.TabOf = tabOf

function C:Docked()
    local dock = GENERAL_CHAT_DOCK
    local list = dock and ((FCFDock_GetChatFrames and C.Try(FCFDock_GetChatFrames, dock)) or dock.DOCKED_CHAT_FRAMES)
    if type(list) ~= "table" or not list[1] then list = DOCKED_CHAT_FRAMES end
    local out = {}
    if type(list) == "table" and list[1] then
        for _, f in ipairs(list) do table.insert(out, f) end
        return out
    end
    for i = 1, (NUM_CHAT_WINDOWS or 10) do
        local f = _G["ChatFrame" .. i]
        local t = tabOf(f)
        if f and (i == 1 or (t and t:IsShown() and f.isDocked)) then table.insert(out, f) end
    end
    return out
end

function C:IsDocked(f)
    for _, d in ipairs(self:Docked()) do if d == f then return true end end
    return false
end

function C:Selected()
    if self.worldView then return nil end
    local dock = GENERAL_CHAT_DOCK
    local sel = dock and FCFDock_GetSelectedWindow and C.Try(FCFDock_GetSelectedWindow, dock)
    return sel or SELECTED_DOCK_FRAME or SELECTED_CHAT_FRAME or DEFAULT_CHAT_FRAME or ChatFrame1
end

-- -- Building the pieces ---------------------------------------------------------------------------

local function iconButton(parent, name, art, fallbackText, tip, onClick)
    local b = CreateFrame("Button", name, parent)
    b:SetWidth(18)
    b:SetHeight(18)
    b.icon = b:CreateTexture(nil, "ARTWORK")
    b.icon:SetAllPoints(b)
    b.lcArt = art
    b.label = b:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    b.label:SetPoint("CENTER", b, "CENTER", 0, 1)
    b.label:SetText(fallbackText)
    local hl = b:CreateTexture(nil, "HIGHLIGHT")
    hl:SetAllPoints(b)
    hl:SetTexture(1, 1, 1, 0.12)
    b:RegisterForClicks("LeftButtonUp", "RightButtonUp")
    b:SetScript("OnClick", function(self, button) onClick(self, button) end)
    b:SetScript("OnEnter", function(self)
        C.lastActivity = now()
        if GameTooltip then
            GameTooltip:SetOwner(self, "ANCHOR_TOP")
            GameTooltip:SetText(tip, 1, 1, 1)
            GameTooltip:Show()
        end
    end)
    b:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
    return b
end

-- The strip over the main window's panel: the dock (the chat tabs) and the World tab sit on it, the menu, Copy and
-- Expand buttons at its right end.
function C:Bar()
    if self.bar then return self.bar end
    local b = CreateFrame("Frame", "LeylineChatBar", UIParent)
    b:SetHeight(self.TAB_H)
    b:SetWidth(44)
    b.menu = iconButton(b, "LeylineChatMenuButton", "Chat_Menu", "...", "Chat menu", function(btn) C:OpenMenu(btn) end)
    b.menu:SetPoint("BOTTOMRIGHT", b, "BOTTOMRIGHT", -2, 1)
    b.copy = iconButton(b, "LeylineChatCopyButton", "Chat_Copy", "C", "Copy chat", function() C:OpenCopy() end)
    b.copy:SetPoint("RIGHT", b.menu, "LEFT", -2, 0)
    b.expand = iconButton(b, "LeylineChatExpandButton", "Chat_Expand", "+", "Expand chat: the whole history, with search", function() C:OpenExpanded() end)
    b.expand:SetPoint("RIGHT", b.copy, "LEFT", -2, 0)
    b:Hide()
    self.bar = b
    return b
end

function C:SkinFor(f)
    if f.lcSkin then return f.lcSkin end
    local k = { frame = f }
    local name = f:GetName() or "LeylineChatAnon"

    k.back = CreateFrame("Frame", name .. "LCShadow", f)
    k.shadow = newSlice(k.back, "BACKGROUND")

    local p = CreateFrame("Frame", name .. "LCPanel", f)
    k.panel = p
    p.face = p:CreateTexture(nil, "BACKGROUND")
    p.face:SetAllPoints(p)
    p.edge = newSlice(p, "BORDER")
    p.lines = {}
    for _, side in ipairs({ "TOP", "BOTTOM", "LEFT", "RIGHT" }) do
        local t = p:CreateTexture(nil, "BORDER")
        if side == "TOP" or side == "BOTTOM" then
            t:SetHeight(1)
            t:SetPoint(side .. "LEFT", p, side .. "LEFT", 0, 0)
            t:SetPoint(side .. "RIGHT", p, side .. "RIGHT", 0, 0)
        else
            t:SetWidth(1)
            t:SetPoint("TOP" .. side, p, "TOP" .. side, 0, 0)
            t:SetPoint("BOTTOM" .. side, p, "BOTTOM" .. side, 0, 0)
        end
        p.lines[side] = t
    end
    p.top = p:CreateTexture(nil, "ARTWORK")
    p.top:SetHeight(1)
    p.top:SetPoint("TOPLEFT", p, "TOPLEFT", 1, -1)
    p.top:SetPoint("TOPRIGHT", p, "TOPRIGHT", -1, -1)

    -- the input line's well (docked frames), with a hint while you are not typing
    p.well = p:CreateTexture(nil, "BORDER")
    p.well:SetPoint("BOTTOMLEFT", p, "BOTTOMLEFT", 4, 4)
    p.well:SetPoint("BOTTOMRIGHT", p, "BOTTOMRIGHT", -4, 4)
    p.well:SetHeight(20)
    p.wellLine = p:CreateTexture(nil, "ARTWORK")
    p.wellLine:SetHeight(1)
    p.wellLine:SetPoint("BOTTOMLEFT", p.well, "TOPLEFT", 0, 0)
    p.wellLine:SetPoint("BOTTOMRIGHT", p.well, "TOPRIGHT", 0, 0)
    p.hint = p:CreateFontString(nil, "ARTWORK", "GameFontHighlightSmall")
    p.hint:SetPoint("LEFT", p.well, "LEFT", 8, 0)
    p.hint:SetText("Press Enter to chat")

    -- thin scrollbar on hover
    k.bar = CreateFrame("Frame", name .. "LCScrollBar", f)
    k.bar:SetWidth(3)
    k.bar:SetPoint("TOPLEFT", f, "TOPRIGHT", 2, -2)
    k.bar:SetPoint("BOTTOMLEFT", f, "BOTTOMRIGHT", 2, 2)
    k.bar.track = k.bar:CreateTexture(nil, "BACKGROUND")
    k.bar.track:SetAllPoints(k.bar)
    k.bar.thumb = k.bar:CreateTexture(nil, "ARTWORK")
    k.bar.thumb:SetWidth(3)
    k.bar:Hide()

    -- jump to latest
    k.jump = CreateFrame("Button", name .. "LCJump", f)
    k.jump:SetWidth(20)
    k.jump:SetHeight(20)
    k.jump:SetPoint("BOTTOMRIGHT", f, "BOTTOMRIGHT", 2, 0)
    k.jump.icon = k.jump:CreateTexture(nil, "ARTWORK")
    k.jump.icon:SetAllPoints(k.jump)
    k.jump:SetScript("OnClick", function(self)
        if f.ScrollToBottom then f:ScrollToBottom() end
        self:Hide()
    end)
    k.jump:SetScript("OnEnter", function(self)
        if GameTooltip then
            GameTooltip:SetOwner(self, "ANCHOR_LEFT")
            GameTooltip:SetText("Jump to the latest line", 1, 1, 1)
            GameTooltip:Show()
        end
    end)
    k.jump:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
    k.jump:Hide()

    -- resize grip (bottom-right of the panel), shown on hover when the window is unlocked
    k.grip = CreateFrame("Button", name .. "LCGrip", p)
    k.grip:SetWidth(12)
    k.grip:SetHeight(12)
    k.grip:SetPoint("BOTTOMRIGHT", p, "BOTTOMRIGHT", -1, 1)
    k.grip.icon = k.grip:CreateTexture(nil, "OVERLAY")
    k.grip.icon:SetAllPoints(k.grip)
    k.grip:SetScript("OnMouseDown", function()
        if f.SetResizable then f:SetResizable(true) end
        if f.SetMinResize then f:SetMinResize(200, 60) end
        if f.StartSizing then f:StartSizing("BOTTOMRIGHT") end
        k.sizing = true
    end)
    k.grip:SetScript("OnMouseUp", function()
        if f.StopMovingOrSizing then f:StopMovingOrSizing() end
        k.sizing = nil
        C:Snap(f, true)
    end)
    k.grip:Hide()

    f.lcSkin = k
    table.insert(self.skinned, f)
    return k
end

-- -- Applying a theme ---------------------------------------------------------------------------

local function col(t, a) return t[1], t[2], t[3], a or 1 end
C.Col = col

function C:ApplyFont(f)
    if not f or not f.lcSkin then return end
    if not f.lcFont and f.GetFont then
        local path, size, flags = f:GetFont()
        f.lcFont = { path or "Fonts\\ARIALN.TTF", size or 14, flags or "" }
        f.lcSpacing = f.GetSpacing and f:GetSpacing() or 0
    end
    local theme = self:Theme()
    if f.SetFont and f.lcFont then f:SetFont(f.lcFont[1], tonumber(self:Get("fontSize")) or 13, theme.flags) end
    if f.SetSpacing then f:SetSpacing(1) end
    if f.SetShadowColor then
        if theme.ink then f:SetShadowColor(0, 0, 0, 0) else f:SetShadowColor(0, 0, 0, 0.9) end
    end
    if f.SetShadowOffset then f:SetShadowOffset(1, -1) end
end

function C:ThemeSkin(k)
    local theme = self:Theme()
    local f, p = k.frame, k.panel
    local op = (tonumber(self:Get("opacity")) or 85) / 100
    if op < 0.4 then op = 0.4 elseif op > 1 then op = 1 end
    k.opacity = op

    local docked = (f == self.worldFrame) or self:IsDocked(f)
    k.docked = docked
    local bottom = docked and self.PAD.input or self.PAD.bottom
    p:ClearAllPoints()
    p:SetPoint("TOPLEFT", f, "TOPLEFT", -self.PAD.left, self.PAD.top)
    p:SetPoint("BOTTOMRIGHT", f, "BOTTOMRIGHT", self.PAD.right, -bottom)
    k.back:ClearAllPoints()
    k.back:SetAllPoints(p)

    -- the face: panel art (or a flat colour) at the opacity setting
    k.art = self:Art(p.face, artName("Panel", theme), { col(theme.face, 1) })
    p.face:SetTexCoord(0, 1, 0, 1)
    p.face:SetAlpha(op * ((not k.art and theme.faceAlpha) or 1))

    -- the edge: theme art, or thin flat lines
    local edged = setSlice(p.edge, p, artName("Edge", theme), theme.edgeSize, theme.edgeOut, 0)
    k.edged = edged
    for _, t in pairs(p.lines) do
        t:SetTexture(col(theme.edge, theme.edgeAlpha or 1))
        if edged then t:Hide() else t:Show() end
    end
    if theme.topLine or not edged then
        p.top:SetTexture(col(theme.top, theme.topAlpha or 0.55))
        p.top:Show()
    else
        p.top:Hide()
    end

    -- the soft shadow underneath
    if theme.shadow and setSlice(k.shadow, p, "Chat_Shadow", 16, 10 + theme.edgeOut, -4) then
        sliceAlpha(k.shadow, 0.55 + 0.45 * op)
    else
        for _, d in ipairs(SLICES) do k.shadow[d[1]]:Hide() end
    end

    if docked then p.well:Show(); p.wellLine:Show(); p.hint:Show() else p.well:Hide(); p.wellLine:Hide(); p.hint:Hide() end
    p.well:SetTexture(col(theme.inset, 1))
    p.well:SetAlpha(math.min(1, op + 0.1) * ((theme.faceAlpha and 0.8) or 1))
    p.wellLine:SetTexture(col(theme.edge, (theme.edgeAlpha or 0.8)))
    if p.hint.SetFont then p.hint:SetFont("Fonts\\FRIZQT__.TTF", 10, theme.flags) end
    p.hint:SetTextColor(col(theme.muted, 0.75))
    local bar = self:Bar()
    for _, b in ipairs({ bar.menu, bar.copy, bar.expand }) do
        if self:Art(b.icon, b.lcArt, nil) then
            b.icon:SetVertexColor(col(theme.muted))
            b.label:Hide()
        else
            b.label:Show()
            b.label:SetTextColor(col(theme.muted))
        end
    end

    k.bar.track:SetTexture(col(theme.edge, 0.35))
    k.bar.thumb:SetTexture(col(theme.accent, 0.85))
    if not self:Art(k.jump.icon, "Chat_Arrow", nil) then k.jump.icon:SetTexture(col(theme.accent, 0.9)) end
    k.jump.icon:SetVertexColor(1, 1, 1)
    if not self:Art(k.grip.icon, "Chat_Grip", nil) then k.grip.icon:SetTexture(col(theme.muted, 0.5)) end

    self:ApplyFont(f)

    -- fading text
    if f.lcFading == nil then
        f.lcFading = (f.GetFading and f:GetFading()) and true or false
        f.lcTimeVisible = f.GetTimeVisible and f:GetTimeVisible() or 120
    end
    local fade = self:Get("fade")
    if fade == "never" then
        if f.SetFading then f:SetFading(false) end
    else
        if f.SetFading then f:SetFading(true) end
        if f.SetTimeVisible then f:SetTimeVisible(tonumber(fade) or 20) end
    end
end

function C:SkinFrame(f)
    local k = self:SkinFor(f)
    -- the chat frame stays above its panel and shadow
    if not f.lcLevel then
        f.lcLevel = f:GetFrameLevel() or 1
        if f.lcLevel < 3 then f:SetFrameLevel(3) end
    end
    local lvl = f:GetFrameLevel() or 3
    k.back:SetFrameLevel(math.max(0, lvl - 2))
    k.panel:SetFrameLevel(math.max(0, lvl - 1))
    k.bar:SetFrameLevel(lvl + 1)
    k.jump:SetFrameLevel(lvl + 2)
    k.grip:SetFrameLevel(lvl + 2)
    local name = f:GetName() or ""
    for _, s in ipairs(stockTextures()) do suppress(_G[name .. s]) end
    for _, s in ipairs(STOCK_FRAMES) do suppress(_G[name .. s]) end
    if f.buttonFrame then suppress(f.buttonFrame) end
    if f.resizeButton then suppress(f.resizeButton) end
    k.back:Show()
    k.panel:Show()
    self:ThemeSkin(k)
end

function C:UnskinFrame(f)
    local k = f.lcSkin
    if not k then return end
    k.back:Hide()
    k.panel:Hide()
    k.bar:Hide()
    k.jump:Hide()
    local name = f:GetName() or ""
    for _, s in ipairs(stockTextures()) do unsuppress(_G[name .. s]) end
    for _, s in ipairs(STOCK_FRAMES) do unsuppress(_G[name .. s]) end
    if f.buttonFrame then unsuppress(f.buttonFrame) end
    if f.resizeButton then unsuppress(f.resizeButton) end
    if f.lcFont and f.SetFont then f:SetFont(f.lcFont[1], f.lcFont[2], f.lcFont[3]) end
    if f.SetSpacing then f:SetSpacing(f.lcSpacing or 0) end
    if f.SetShadowColor then f:SetShadowColor(0, 0, 0, 1) end
    f.lcFont = nil
    if f.lcFading ~= nil then
        if f.SetFading then f:SetFading(f.lcFading) end
        if f.SetTimeVisible then f:SetTimeVisible(f.lcTimeVisible or 120) end
        f.lcFading = nil
    end
    if f.lcLevel then f:SetFrameLevel(f.lcLevel); f.lcLevel = nil end
end

-- -- Tabs --------------------------------------------------------------------------------------

local function tabText(tab)
    return _G[(tab:GetName() or "") .. "Text"] or (tab.GetFontString and tab:GetFontString())
end

function C:SkinTab(tab, f)
    if not tab then return end
    if not tab.lcBg then
        tab.lcBg = tab:CreateTexture(nil, "BACKGROUND")
        tab.lcBg:SetAllPoints(tab)
        tab.lcLine = tab:CreateTexture(nil, "ARTWORK")
        tab.lcLine:SetHeight(2)
        tab.lcLine:SetPoint("BOTTOMLEFT", tab, "BOTTOMLEFT", 3, 0)
        tab.lcLine:SetPoint("BOTTOMRIGHT", tab, "BOTTOMRIGHT", -3, 0)
        tab.lcDot = tab:CreateTexture(nil, "OVERLAY")
        tab.lcDot:SetWidth(6)
        tab.lcDot:SetHeight(6)
        tab.lcDot:SetPoint("TOPRIGHT", tab, "TOPRIGHT", -3, -3)
        tab.lcOwn = { [tab.lcBg] = true, [tab.lcLine] = true, [tab.lcDot] = true }
    end
    tab.lcFrame = f
    if not tab.lcOrig then
        local fs = tabText(tab)
        local o = { w = tab:GetWidth(), h = tab:GetHeight(), points = savePoints(tab), fs = fs, regions = {} }
        if fs then o.fsPoints = savePoints(fs) end
        if fs and fs.GetFont then o.font = { fs:GetFont() } end
        if fs and fs.GetTextColor then o.color = { fs:GetTextColor() } end
        for _, r in ipairs({ tab:GetRegions() }) do
            if r.GetObjectType and r:GetObjectType() == "Texture" and not tab.lcOwn[r] then table.insert(o.regions, r) end
        end
        -- leaving the World view comes first, then the stock click selects the tab's window
        o.click = tab:GetScript("OnClick")
        tab:SetScript("OnClick", function(self, button, ...)
            if button ~= "RightButton" and C.LeaveWorld then C:LeaveWorld() end
            if o.click then o.click(self, button, ...) end
            C:RefreshTabs()
        end)
        local origSetAlpha = tab.SetAlpha
        tab.lcSetAlpha = function(a) origSetAlpha(tab, a) end
        tab.lcOrig = o
    end
    for _, r in ipairs(tab.lcOrig.regions) do suppress(r) end
    local tname = tab:GetName() or ""
    for _, n in ipairs(TAB_PARTS) do suppress(_G[tname .. n]) end
    if tab.glow then suppress(tab.glow) end
    -- the stock code fades the tabs out a moment after the mouse leaves; ours stay with the panel
    tab.SetAlpha = function(self)
        if UIFrameFadeRemoveFrame then UIFrameFadeRemoveFrame(self) end
    end
    tab.lcSetAlpha(self.chromeAlpha or 1)
    tab.lcBg:Show(); tab.lcLine:Show(); tab.lcDot:Show()
    tab:SetHeight(self.TAB_H)
    if tab.lcOrig.level == nil then tab.lcOrig.level = tab:GetFrameLevel() or 1 end
    if f and f.GetFrameLevel and (tab:GetFrameLevel() or 0) <= (f:GetFrameLevel() or 1) then tab:SetFrameLevel((f:GetFrameLevel() or 1) + 1) end
    local fs = tab.lcOrig.fs
    if fs and fs.SetFont then fs:SetFont("Fonts\\FRIZQT__.TTF", 10, self:Theme().flags) end
    if fs then
        fs:ClearAllPoints()
        fs:SetPoint("CENTER", tab, "CENTER", 0, 1)
        if fs.SetWidth then fs:SetWidth(0) end
        local w = (fs.GetStringWidth and fs:GetStringWidth() or 30) + 20
        if tab.conversationIcon then w = w + 14 end
        if w < 40 then w = 40 end
        tab.lcWant = w
        tab:SetWidth(w)
    end
end

function C:UnskinTab(tab)
    if not tab or not tab.lcOrig then return end
    local o = tab.lcOrig
    tab.SetAlpha = nil
    for _, r in ipairs(o.regions) do unsuppress(r) end
    local tname = tab:GetName() or ""
    for _, n in ipairs(TAB_PARTS) do unsuppress(_G[tname .. n]) end
    if tab.glow then unsuppress(tab.glow) end
    tab.lcBg:Hide(); tab.lcLine:Hide(); tab.lcDot:Hide()
    tab:SetScript("OnClick", o.click)
    if o.level then tab:SetFrameLevel(o.level) end
    if o.w and o.w > 0 then tab:SetWidth(o.w) end
    if o.h and o.h > 0 then tab:SetHeight(o.h) end
    loadPoints(tab, o.points)
    local fs = o.fs
    if fs then
        if o.font and o.font[1] and fs.SetFont then fs:SetFont(o.font[1], o.font[2], o.font[3]) end
        if o.color and o.color[1] then fs:SetTextColor(o.color[1], o.color[2], o.color[3]) end
        loadPoints(fs, o.fsPoints)
    end
    tab.lcOrig = nil
    tab.lcWant = nil
    tab.lcUnread, tab.lcMention = nil, nil
end

-- Unread dot (replaces the stock tab glow).
function C:MarkUnread(f, mention)
    if not self.active then return end
    local tab = tabOf(f)
    if not tab or not tab.lcBg then return end
    if f == self:Selected() and f:IsVisible() then return end
    tab.lcUnread = true
    if mention then tab.lcMention = true end
    self:StyleTab(tab, false)
end

function C:StyleTab(tab, selected, theme)
    if not tab or not tab.lcBg then return end
    theme = theme or self:Theme()
    if not self:Art(tab.lcBg, artName("Tab", theme), { col(theme.tab, 1) }) then
        tab.lcBg:SetTexture(col(theme.tab, theme.faceAlpha or 1))
    end
    local op = (tonumber(self:Get("opacity")) or 85) / 100
    tab.lcBg:SetAlpha((selected and 1 or 0.72) * math.max(op, 0.55))
    tab.lcBg:SetVertexColor(selected and 1 or 0.82, selected and 1 or 0.82, selected and 1 or 0.82)
    tab.lcLine:SetTexture(col(theme.accent, 1))
    tab.lcLine:SetAlpha(selected and 1 or 0)
    if not self:Art(tab.lcDot, "Chat_Dot", nil) then tab.lcDot:SetTexture(1, 1, 1, 1) end
    if tab.lcMention then tab.lcDot:SetVertexColor(1, 0.62, 0.18) else tab.lcDot:SetVertexColor(col(theme.accent)) end
    tab.lcDot:SetAlpha((tab.lcUnread and not selected) and 1 or 0)
    local fs = tab.lcOrig and tab.lcOrig.fs or tab.label
    if fs then
        if selected then fs:SetTextColor(col(theme.text)) else fs:SetTextColor(col(theme.muted)) end
        if fs.SetAlpha then fs:SetAlpha(1) end
    end
end

function C:RefreshTabs()
    if not self.active then return end
    local theme = self:Theme()
    local sel = self:Selected()
    for _, f in ipairs(C.Frames()) do
        local tab = tabOf(f)
        if tab and tab.lcBg and tab.lcOrig then
            local selected
            if self:IsDocked(f) then selected = (f == sel) else selected = not self.worldView end
            if selected then tab.lcUnread, tab.lcMention = nil, nil end
            self:StyleTab(tab, selected, theme)
        end
    end
    if self.StyleWorldTab then self:StyleWorldTab(theme) end
end

-- -- The tab strip ---------------------------------------------------------------------------------
-- The game's dock (GENERAL_CHAT_DOCK) sits on our strip, from its left end to the buttons; its fixed tabs go left
-- to right at their natural width, the World tab right after them, then the dock's scrolling list of whisper tabs.
-- Run after every FCFDock_UpdateTabs (and after Forever UI's own hook on it), so this layout is the last word.

function C:AnchorDock()
    local dock = GENERAL_CHAT_DOCK
    if not dock or not self.bar then return end
    if not self.dockPoints then self.dockPoints = savePoints(dock) end
    dock:ClearAllPoints()
    dock:SetPoint("BOTTOMLEFT", self.bar, "BOTTOMLEFT", 2, 0)
    dock:SetPoint("BOTTOMRIGHT", self.bar, "BOTTOMRIGHT", -self.BUTTONS_W, 0)
end

function C:LayoutTabs()
    if self.PrepWorldTab then self:PrepWorldTab() end
    if not self.active or not self.skinOn then return end
    local dock = GENERAL_CHAT_DOCK
    local lastStatic
    for _, f in ipairs(self:Docked()) do
        local tab = tabOf(f)
        if tab and tab.lcOrig then
            if tab.lcWant then tab:SetWidth(tab.lcWant) end
            tab:SetHeight(self.TAB_H)
            -- the fixed tabs (a whisper window's tab stays in the dock's scrolling list)
            if not dock or f.isStaticDocked or (f.isStaticDocked == nil and not f.isTemporary) then
                tab:ClearAllPoints()
                if lastStatic then
                    tab:SetPoint("LEFT", lastStatic, "RIGHT", self.TAB_GAP, 0)
                else
                    tab:SetPoint("BOTTOMLEFT", dock or self.bar, "BOTTOMLEFT", dock and 0 or 2, 0)
                end
                lastStatic = tab
            end
        end
    end
    local wt = LeylineChatWorldTab
    local after = lastStatic
    if wt and wt:IsShown() then
        wt:ClearAllPoints()
        if lastStatic then
            wt:SetPoint("LEFT", lastStatic, "RIGHT", self.TAB_GAP, 0)
        else
            wt:SetPoint("BOTTOMLEFT", self.bar, "BOTTOMLEFT", 2, 0)
        end
        after = wt
    end
    if dock and dock.scrollFrame and after then
        dock.scrollFrame:SetPoint("LEFT", after, "RIGHT", self.TAB_GAP, 0)
    end
    -- undocked windows keep the stock place for their tab (over their own panel)
    self.layouts = (self.layouts or 0) + 1
end

-- The tabs' wanted widths changed (a rename, another addon resized one): lay out again.
function C:TabsDrifted()
    for _, f in ipairs(self:Docked()) do
        local tab = tabOf(f)
        if tab and tab.lcWant and math.abs((tab:GetWidth() or 0) - tab.lcWant) > 0.5 then return true end
    end
    return false
end

-- -- The input line -------------------------------------------------------------------------------

function C:SkinEditBox(eb, main)
    local mainSkin = ChatFrame1 and ChatFrame1.lcSkin
    if not eb or not mainSkin then return end
    local theme = self:Theme()
    if not eb.lcOrig then
        local o = { points = savePoints(eb), h = eb:GetHeight(), regions = {}, parent = eb.GetParent and eb:GetParent() }
        if eb.GetFont then o.font = { eb:GetFont() } end
        eb.lcBg = eb:CreateTexture(nil, "BACKGROUND")
        eb.lcBg:SetAllPoints(eb)
        eb.lcEdge = eb:CreateTexture(nil, "BORDER")
        eb.lcEdge:SetHeight(1)
        eb.lcEdge:SetPoint("TOPLEFT", eb, "TOPLEFT", 0, 0)
        eb.lcEdge:SetPoint("TOPRIGHT", eb, "TOPRIGHT", 0, 0)
        local own = { [eb.lcBg] = true, [eb.lcEdge] = true }
        o.texts = {}
        for _, r in ipairs({ eb:GetRegions() }) do
            local kind = r.GetObjectType and r:GetObjectType()
            if kind == "Texture" and not own[r] then table.insert(o.regions, r)
            elseif kind == "FontString" then table.insert(o.texts, r) end
        end
        o.header = _G[(eb:GetName() or "") .. "Header"]
        if o.header and o.header.GetFont then o.headerFont = { o.header:GetFont() } end
        eb.lcOrig = o
        if self.HookEditBox then self:HookEditBox(eb) end
    end
    for _, r in ipairs(eb.lcOrig.regions) do suppress(r) end
    -- Forever UI fades the edit box's words at rest (and lights them while you type); readable once skinned
    for _, r in ipairs(eb.lcOrig.texts) do if r.SetAlpha then r:SetAlpha(1) end end
    -- the main box hangs off the screen, not the window, so it stays when the World view hides the window
    if main and eb.SetParent and eb.lcOrig.parent and eb.lcOrig.parent ~= UIParent then
        eb:SetParent(UIParent)
        eb.lcReparented = true
        if eb.SetFrameStrata and ChatFrame1.GetFrameStrata then eb:SetFrameStrata(ChatFrame1:GetFrameStrata() or "LOW") end
        if eb.SetFrameLevel then eb:SetFrameLevel((ChatFrame1:GetFrameLevel() or 3) + 3) end
    end
    eb.lcBg:Show()
    eb.lcEdge:Show()
    eb.lcBg:SetTexture(col(theme.inset, 1))
    eb.lcBg:SetAlpha(math.min(1, (mainSkin.opacity or 0.85) + 0.15))
    eb.lcEdge:SetTexture(col(theme.accent, 0.55))
    eb:ClearAllPoints()
    eb:SetPoint("BOTTOMLEFT", ChatFrame1, "BOTTOMLEFT", -self.PAD.left + 4, -self.PAD.input + 4)
    eb:SetPoint("BOTTOMRIGHT", ChatFrame1, "BOTTOMRIGHT", self.PAD.right - 4, -self.PAD.input + 4)
    eb:SetHeight(20)
    local size = tonumber(self:Get("fontSize")) or 13
    if eb.SetFont and eb.lcOrig.font then eb:SetFont(eb.lcOrig.font[1] or "Fonts\\ARIALN.TTF", size, theme.flags) end
    if eb.SetTextColor then eb:SetTextColor(col(theme.text)) end
    local h = eb.lcOrig.header
    if h and h.SetFont then h:SetFont("Fonts\\FRIZQT__.TTF", size - 2, theme.flags) end
    self:ColourHeader(eb)
end

function C:UnskinEditBox(eb)
    if not eb or not eb.lcOrig then return end
    local o = eb.lcOrig
    for _, r in ipairs(o.regions) do unsuppress(r) end
    eb.lcBg:Hide()
    eb.lcEdge:Hide()
    if eb.lcReparented and o.parent then eb:SetParent(o.parent); eb.lcReparented = nil end
    loadPoints(eb, o.points)
    if o.h and o.h > 0 then eb:SetHeight(o.h) end
    if o.font and o.font[1] and eb.SetFont then eb:SetFont(o.font[1], o.font[2], o.font[3]) end
    if o.header and o.headerFont and o.headerFont[1] then o.header:SetFont(o.headerFont[1], o.headerFont[2], o.headerFont[3]) end
    if ChatEdit_UpdateHeader and eb:IsShown() then C.Try(ChatEdit_UpdateHeader, eb) end
end

-- The edit boxes of the docked windows go into the main window's well.
function C:SkinEditBoxes()
    local main = C.MainEditBox()
    for _, f in ipairs(self:Docked()) do
        local eb = f.editBox or _G[(f:GetName() or "") .. "EditBox"]
        if eb then self:SkinEditBox(eb, eb == main) end
    end
    if main and not main.lcBg then self:SkinEditBox(main, true) end
end

-- "Say:", "Guild:", "2. Trade:" in the colour of the channel being typed into.
function C:ColourHeader(eb)
    if not self.active or not eb or not eb.lcOrig then return end
    local h = eb.lcOrig.header
    if not h or not ChatTypeInfo then return end
    local t = C.EditAttr(eb, "chatType") or "SAY"
    local info = ChatTypeInfo[t]
    local target = C.EditAttr(eb, "channelTarget")
    if t == "CHANNEL" and target then info = ChatTypeInfo["CHANNEL" .. target] or info end
    if info then
        local r, g, b = info.r or 1, info.g or 1, info.b or 1
        if self.inked then r, g, b = C.InkRGB(r, g, b) end
        h:SetTextColor(r, g, b)
        if eb.SetTextColor then eb:SetTextColor(r, g, b) end
    end
end

-- -- Menu on the tab bar -------------------------------------------------------------------------

function C:OpenMenu(anchor)
    local f = self:Selected() or ChatFrame1
    local items = { { text = "Chat", title = true } }
    if ChatFrame_ToggleMenu or ChatFrame_OpenMenu or ChatMenu then
        table.insert(items, { text = "Emotes and speech", func = function()
            if ChatFrame_ToggleMenu then C.Try(ChatFrame_ToggleMenu)
            elseif ChatFrame_OpenMenu then C.Try(ChatFrame_OpenMenu)
            elseif ChatMenu then ChatMenu:Show() end
            if ChatMenu then
                ChatMenu:ClearAllPoints()
                ChatMenu:SetPoint("BOTTOMRIGHT", anchor, "TOPRIGHT", 0, 2)
            end
        end })
    end
    table.insert(items, { text = "Expanded view (whole history)", func = function() C:OpenExpanded() end })
    table.insert(items, { text = "Copy chat", func = function() C:OpenCopy((self.worldView) and nil or f) end })
    if not self.worldView then
        table.insert(items, { text = "Highlight my name in this tab", checked = self:MentionsOn(f), func = function()
            local s = C:Settings()
            if type(s.mentionOff) ~= "table" then s.mentionOff = {} end
            local key = f:GetName()
            if s.mentionOff[key] then s.mentionOff[key] = nil else s.mentionOff[key] = true end
        end })
    end
    table.insert(items, { text = "Tabs", title = true })
    table.insert(items, { text = "Show the World tab", checked = self:Get("worldTab") and true or false, func = function() C:SetOption("worldTab", not C:Get("worldTab")) end })
    table.insert(items, { text = "Whispers in their own tabs", checked = self:Get("whisperTabs") and true or false, func = function() C:SetOption("whisperTabs", not C:Get("whisperTabs")) end })
    table.insert(items, { text = "Theme", title = true })
    for _, key in ipairs(self.THEME_ORDER) do
        local k = key
        table.insert(items, { text = self.THEMES[k].label, checked = (self:Get("theme") == k), func = function() C:SetOption("theme", k) end })
    end
    table.insert(items, { text = "All chat settings...", func = function() C:OpenSettings() end })
    self.menu = C.UI.Menu(anchor, items)
    if self.menu and anchor then
        self.menu:ClearAllPoints()
        self.menu:SetPoint("BOTTOMRIGHT", anchor, "TOPRIGHT", 0, 2)
    end
    return self.menu
end

-- -- Moving and resizing: snap to a 4 px grid and the screen edges ------------------------------

function C:Snap(f, sized)
    if not f or not f.GetLeft or not self.active then return end
    local l, b = f:GetLeft(), f:GetBottom()
    if not l or not b then return end
    local w, h = f:GetWidth() or 0, f:GetHeight() or 0
    local sw = UIParent and UIParent:GetWidth() or 1024
    local sh = UIParent and UIParent:GetHeight() or 768
    local function grid(v) return floor(v / 4 + 0.5) * 4 end
    if sized then w, h = grid(w), grid(h) end
    l, b = grid(l), grid(b)
    local near = 16
    local bottomPad = (f == ChatFrame1) and self.PAD.input or self.PAD.bottom
    if l - self.PAD.left < near then l = self.PAD.left + 4 end
    if b - bottomPad < near then b = bottomPad + 4 end
    if sw - (l + w + self.PAD.right) < near then l = sw - w - self.PAD.right - 4 end
    if sh - (b + h + self.PAD.top + self.TAB_H) < near then b = sh - h - self.PAD.top - self.TAB_H - 4 end
    f:ClearAllPoints()
    f:SetPoint("BOTTOMLEFT", UIParent, "BOTTOMLEFT", l, b)
    if w > 0 and h > 0 then f:SetWidth(w); f:SetHeight(h) end
    -- the game keeps chat windows' places itself
    if FCF_SavePositionAndDimensions then C.Try(FCF_SavePositionAndDimensions, f) end
    self.snapped = { l, b, w, h }
    return l, b
end

-- -- Hooks into the stock chat code (post-hooks only, so nothing of the stock code is replaced) ------

local hooked = false
function C:HookStock()
    if hooked then return end
    hooked = true
    -- the stock tab glow: our unread dot instead
    C.Hook("FCF_StartAlertFlash", function(f)
        if not C.active then return end
        if FCF_StopAlertFlash then C.Try(FCF_StopAlertFlash, f) end
        C:MarkUnread(f)
    end)
    C.Hook("FCF_FlashTab", function(f)
        if C.active and f and f.GetName then C:MarkUnread(f) end
    end)
    -- the dock re-places its tabs (a window docked, undocked, renamed, a whisper window opened): our layout after it
    C.Hook("FCFDock_UpdateTabs", function(dock)
        if C.active and C.skinOn and (dock == nil or dock == GENERAL_CHAT_DOCK) then C:LayoutTabs() end
    end)
    C.Hook("FCFDock_SetPrimary", function(dock)
        if C.active and C.skinOn and dock == GENERAL_CHAT_DOCK then C:AnchorDock() end
    end)
    C.Hook("FCF_DockFrame", function() if C.active and C.skinOn then C:Skin() end end)
    C.Hook("FCF_UnDockFrame", function() if C.active and C.skinOn then C:Skin() end end)
    C.Hook("FCF_DockUpdate", function() if C.active and C.skinOn then C:Skin() end end)
    C.Hook("FCF_OpenTemporaryWindow", function()
        C:HookFrames()
        if C.active and C.skinOn then C:Skin() end
    end)
    C.Hook("FCF_SetWindowName", function(f)
        if C.active and C.skinOn and f then
            local tab = tabOf(f)
            if tab and tab.lcOrig then C:SkinTab(tab, f); C:LayoutTabs(); C:RefreshTabs() end
        end
    end)
    C.Hook("FCF_SetChatWindowFontSize", function(_, f)
        f = f or (FCF_GetCurrentChatFrame and C.Try(FCF_GetCurrentChatFrame))
        if C.active and C.skinOn and f and f.lcSkin then C:ApplyFont(f) end
    end)
    C.Hook("FCFTab_UpdateColors", function() if C.active and C.skinOn then C:RefreshTabs() end end)
    C.Hook("PanelTemplates_TabResize", function(tab)
        if C.active and C.skinOn and tab and tab.lcOrig and tab.lcWant then tab:SetWidth(tab.lcWant) end
    end)
    C.Hook("FCF_StopDragging", function(f)
        if not (C.active and C.skinOn and f) then return end
        if C:IsDocked(f) then f = C:Docked()[1] or f end
        C:Snap(f)
    end)
    C.Hook("ChatEdit_UpdateHeader", function(eb) C:ColourHeader(eb) end)
end

-- -- Apply / undo ----------------------------------------------------------------------------------

function C:SkinAllTabs()
    for _, f in ipairs(C.Frames()) do
        local tab = tabOf(f)
        if tab and (f:IsShown() or tab:IsShown() or self:IsDocked(f)) then self:SkinTab(tab, f) end
    end
end

function C:Skin()
    self:HookStock()
    for _, f in ipairs(C.Frames()) do self:SkinFrame(f) end
    self:SkinAllTabs()
    suppress(ChatFrameMenuButton)
    local bar = self:Bar()
    bar:ClearAllPoints()
    bar:SetPoint("BOTTOMRIGHT", ChatFrame1, "TOPRIGHT", self.PAD.right, self.PAD.top + self:TabLift())
    bar:SetPoint("BOTTOMLEFT", ChatFrame1, "TOPLEFT", -self.PAD.left, self.PAD.top + self:TabLift())
    bar:SetFrameLevel((ChatFrame1:GetFrameLevel() or 3) + 1)
    bar:Show()
    self:SkinEditBoxes()
    self.skinOn = true
    self:AnchorDock()
    self:LayoutTabs()
    self:RefreshTabs()
end

-- Tabs sit on top of a thick frame (Wildwood's carved wood) rather than under it.
function C:TabLift()
    local out = self:Theme().edgeOut or 0
    return math.max(0, out - 2)
end

function C:Unskin()
    if not self.skinOn then return end
    self.skinOn = nil
    for _, f in ipairs(self.skinned) do self:UnskinFrame(f) end
    for _, f in ipairs(C.Frames()) do self:UnskinTab(tabOf(f)) end
    unsuppress(ChatFrameMenuButton)
    if self.bar then self.bar:Hide() end
    for _, f in ipairs(C.Frames()) do
        local eb = f.editBox or _G[(f:GetName() or "") .. "EditBox"]
        if eb then self:UnskinEditBox(eb) end
    end
    if ChatFrameEditBox then self:UnskinEditBox(ChatFrameEditBox) end
    local dock = GENERAL_CHAT_DOCK
    if dock and self.dockPoints then loadPoints(dock, self.dockPoints); self.dockPoints = nil end
    if dock and FCFDock_UpdateTabs then C.Try(FCFDock_UpdateTabs, dock, true) end
    if FriendsMicroButton and FriendsMicroButton.SetAlpha then FriendsMicroButton:SetAlpha(1) end
end

function C:ApplySkin()
    if self.active then self:Skin() else self:Unskin() end
end

-- -- Per-frame upkeep: scrollbar, jump arrow, grip, fade, tab styles --------------------------------

local function over(fr)
    return fr and fr:IsVisible() and MouseIsOver and MouseIsOver(fr) and true or false
end
C.Over = over

C.chromeAlpha = 1
C.lastActivity = 0

function C:Hovered()
    for _, f in ipairs(self.skinned) do
        local k = f.lcSkin
        if k and f:IsVisible() and (over(k.panel) or over(tabOf(f)) or k.sizing) then return true end
    end
    if self.bar and over(self.bar) then return true end
    if self.worldFrame and over(self.worldFrame) then return true end
    return false
end

function C:UpdateScroll(f)
    local k = f.lcSkin
    if not k then return end
    if C.AtBottom(f) then k.jump:Hide() elseif f:IsVisible() then k.jump:Show() end
    local hovered = over(k.panel)
    local n = f.GetNumMessages and f:GetNumMessages() or 0
    local size = tonumber(self:Get("fontSize")) or 13
    local fit = floor((f:GetHeight() or 100) / (size + 1))
    if hovered and n > fit and fit > 0 then
        local off = (f.GetCurrentScroll and f:GetCurrentScroll()) or 0
        local h = k.bar:GetHeight() or (f:GetHeight() or 100)
        local th = math.max(10, h * fit / n)
        local range = math.max(1, n - fit)
        local pos = (h - th) * (1 - math.min(off, range) / range)
        k.bar.thumb:SetHeight(th)
        k.bar.thumb:ClearAllPoints()
        k.bar.thumb:SetPoint("TOPLEFT", k.bar, "TOPLEFT", 0, -pos)
        k.bar:Show()
    else
        k.bar:Hide()
    end
    local locked = f.isLocked
    if (hovered or k.sizing) and not locked then k.grip:Show() else k.grip:Hide() end
end

function C:SetChrome(a)
    self.chromeAlpha = a
    for _, f in ipairs(self.skinned) do
        local k = f.lcSkin
        if k then
            k.panel:SetAlpha(a)
            k.back:SetAlpha(a)
        end
        local tab = tabOf(f)
        if tab and tab.lcSetAlpha and tab.lcOrig then tab.lcSetAlpha(a) end
    end
    if self.bar then self.bar:SetAlpha(a) end
end

function C:Typing()
    for _, eb in ipairs(C.EditBoxes()) do
        if eb:IsVisible() and (eb == ACTIVE_CHAT_EDIT_BOX or (eb.HasFocus and eb:HasFocus())) then return true end
    end
    return false
end

function C:Update(elapsed)
    self.ctxFrame = nil                          -- a stale chat note never outlives a frame
    if not self.active or not self.skinOn then return end
    local t = now()
    if self:Typing() or self:Hovered() then self.lastActivity = t end
    for _, f in ipairs(self.skinned) do
        if f:IsVisible() then self:UpdateScroll(f) end
    end
    local fade = self:Get("fade")
    local target = 1
    if fade ~= "never" and t - (self.lastActivity or 0) > (tonumber(fade) or 20) then target = 0 end
    local a = self.chromeAlpha or 1
    if a ~= target then
        local step = (elapsed or 0.05) * ((target > a) and 5 or 0.8)
        if target > a then a = math.min(target, a + step) else a = math.max(target, a - step) end
        self:SetChrome(a)
    end
    self.tick = (self.tick or 0) + (elapsed or 0)
    if self.tick > 0.25 then
        self.tick = 0
        local sel = self:Selected()
        if sel ~= self.lastSelected then
            self.lastSelected = sel
            self:RefreshTabs()
        end
        if self:TabsDrifted() then self:LayoutTabs() end
        self:StockStamps()
        if self.WorldTick then self:WorldTick() end
    end
end

local upkeep = CreateFrame("Frame", "LeylineChatUpkeep", UIParent)
upkeep:SetScript("OnUpdate", function(self, elapsed) C:Update(elapsed) end)
C.upkeep = upkeep
