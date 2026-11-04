--[[
    Skin.lua  (LeylineChat)
    The look. Styles the stock chat frames in place, never replacing them:

        panel     a themed face behind each chat frame (opacity slider), a 1 px light top edge, the theme's edge art
                  and a soft shadow underneath. The main window's panel also holds the input line.
        tabs      flat tabs joined to the panel's top; the active one has an accent underline and new lines show a
                  count badge (orange when you were mentioned; the stock tab glow no longer flashes); opening the tab
                  clears it. The game's dock still orders the tabs (and scrolls the whisper windows' tabs); the World
                  tab sits after the fixed tabs.
        input     the panel's well at rest: where Enter will talk ("Say", "Guild", "World") as a coloured label,
                  click it for the next channel (right-click back, like Tab / Shift+Tab), click the well to start
                  typing. While typing, the edit box sits in the well with the stock header ("Say:") in the
                  channel's colour, and clicking the header cycles too.
        buttons   the stock button column (scroll arrows, menu) is hidden; a thin scrollbar shows on hover, a "jump
                  to latest" arrow when scrolled up ("3 new lines" when lines came in below), and the emote menu,
                  Copy, Expand and settings sit on the tab bar.
        fade      after 10 s (20 s, a minute) of quiet everything we draw fades out: panel, shadow, tabs, tab bar,
                  input well, jump arrow, and the edit box's own backdrop (it hangs off the screen, not the panel,
                  so it is faded on its own: shown only while you type). Hover, typing or a new line brings it back.
                  The lines themselves stay two minutes, as stock.
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
C.LINE_SECONDS = 120        -- how long a line stays before it fades (the stock time), when fading is on
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
    -- clicking the well starts typing; the label at its left says where Enter will talk, and clicking it cycles
    p.wellBtn = CreateFrame("Button", name .. "LCWell", p)
    p.wellBtn:SetAllPoints(p.well)
    p.wellBtn:SetScript("OnClick", function() C:OpenInput(f) end)
    local pill = CreateFrame("Button", name .. "LCChannel", p)
    pill:SetHeight(16)
    pill:SetWidth(40)
    pill:SetPoint("LEFT", p.well, "LEFT", 3, 0)
    pill:RegisterForClicks("LeftButtonUp", "RightButtonUp")
    pill.bg = pill:CreateTexture(nil, "BACKGROUND")
    pill.bg:SetAllPoints(pill)
    pill.text = pill:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    pill.text:SetPoint("CENTER", pill, "CENTER", 0, 0)
    local phl = pill:CreateTexture(nil, "HIGHLIGHT")
    phl:SetAllPoints(pill)
    phl:SetTexture(1, 1, 1, 0.10)
    pill:SetScript("OnClick", function(self, button) C:CycleInput(f, button == "RightButton") end)
    pill:SetScript("OnEnter", function(self)
        C.lastActivity = now()
        if GameTooltip then
            GameTooltip:SetOwner(self, "ANCHOR_TOP")
            GameTooltip:SetText("Talking in " .. (self.text:GetText() or "Say"), 1, 1, 1)
            GameTooltip:AddLine("Click for the next channel, right-click for the one before (Tab and Shift+Tab while typing).", 0.85, 0.85, 0.85, 1)
            GameTooltip:Show()
        end
    end)
    pill:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
    p.pill = pill
    p.hint = p:CreateFontString(nil, "ARTWORK", "GameFontHighlightSmall")
    p.hint:SetPoint("LEFT", pill, "RIGHT", 6, 0)
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

    -- jump to latest ("3 new lines" beside the arrow when lines came in below)
    k.jump = CreateFrame("Button", name .. "LCJump", f)
    k.jump:SetWidth(20)
    k.jump:SetHeight(20)
    k.jump:SetPoint("BOTTOMRIGHT", f, "BOTTOMRIGHT", 2, 0)
    k.jump.bg = k.jump:CreateTexture(nil, "BACKGROUND")
    k.jump.bg:SetAllPoints(k.jump)
    k.jump.bg:Hide()
    k.jump.icon = k.jump:CreateTexture(nil, "ARTWORK")
    k.jump.icon:SetWidth(20)
    k.jump.icon:SetHeight(20)
    k.jump.icon:SetPoint("RIGHT", k.jump, "RIGHT", 0, 0)
    k.jump.label = k.jump:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    k.jump.label:SetPoint("RIGHT", k.jump.icon, "LEFT", 0, 0)
    k.jump.label:Hide()
    k.jump:SetScript("OnClick", function(self)
        if f.ScrollToBottom then f:ScrollToBottom() end
        f.lcBelow = 0
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

    if docked then
        p.well:Show(); p.wellLine:Show(); p.hint:Show()
    else
        p.well:Hide(); p.wellLine:Hide(); p.hint:Hide(); p.pill:Hide(); p.wellBtn:Hide()
    end
    if p.pill.text.SetFont then p.pill.text:SetFont("Fonts\\FRIZQT__.TTF", 10, theme.flags) end
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
    k.jump.bg:SetTexture(col(theme.inset, 0.9))
    if k.jump.label.SetFont then k.jump.label:SetFont("Fonts\\FRIZQT__.TTF", 10, theme.flags) end
    k.jump.label:SetTextColor(col(theme.accent))
    k.jump.lcShown = nil
    if not self:Art(k.grip.icon, "Chat_Grip", nil) then k.grip.icon:SetTexture(col(theme.muted, 0.5)) end

    self:ApplyFont(f)

    -- fading text: lines stay the stock two minutes (the "fade" setting is the window's chrome, see C:Update);
    -- "Never" keeps them for good
    if f.lcFading == nil then
        f.lcFading = (f.GetFading and f:GetFading()) and true or false
        f.lcTimeVisible = f.GetTimeVisible and f:GetTimeVisible() or 120
    end
    local fade = self:Get("fade")
    if fade == "never" then
        if f.SetFading then f:SetFading(false) end
    else
        if f.SetFading then f:SetFading(true) end
        if f.SetTimeVisible then f:SetTimeVisible(self.LINE_SECONDS) end
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
    -- the well's button and label sit over the resting edit box (lvl + 3), which the im chat style keeps shown
    k.panel.wellBtn:SetFrameLevel(lvl + 4)
    k.panel.pill:SetFrameLevel(lvl + 5)
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
        tab.lcBadge = tab:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
        tab.lcBadge:Hide()
        tab.lcOwn = { [tab.lcBg] = true, [tab.lcLine] = true, [tab.lcDot] = true, [tab.lcBadge] = true }
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
    -- the label's alpha is ours: Forever UI dims every tab region at its own rest (1 s after the mouse leaves),
    -- which left our lit tabs without their names; ours follow the window's fade (the tab frame's alpha)
    if fs and not fs.lcSetAlpha then
        local origFsAlpha = fs.SetAlpha
        fs.lcSetAlpha = function(a) if origFsAlpha then origFsAlpha(fs, a) end end
        fs.SetAlpha = NOOP
    end
    if fs and fs.lcSetAlpha then fs.lcSetAlpha(1) end
    if fs and fs.SetFont then fs:SetFont("Fonts\\FRIZQT__.TTF", 10, self:Theme().flags) end
    if fs then
        fs:ClearAllPoints()
        fs:SetPoint("CENTER", tab, "CENTER", 0, 1)
        if fs.SetWidth then fs:SetWidth(0) end
        local w = (fs.GetStringWidth and fs:GetStringWidth() or 30) + 20
        if tab.conversationIcon then w = w + 14 end
        if w < 40 then w = 40 end
        tab.lcBase = w
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
    if tab.lcBadge then tab.lcBadge:Hide() end
    if o.fs and o.fs.lcSetAlpha then o.fs.SetAlpha = nil; o.fs.lcSetAlpha = nil end
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
    tab.lcWant, tab.lcBase = nil, nil
    tab.lcUnread, tab.lcMention, tab.lcCount = nil, nil, nil
end

-- Unread badge (replaces the stock tab glow). counted: one more line (the stock alert hooks only mark it unread,
-- so a line is never counted twice).
function C:MarkUnread(f, mention, counted)
    if not self.active then return end
    local tab = tabOf(f)
    if not tab or not tab.lcBg then return end
    if f == self:Selected() and f:IsVisible() then return end
    tab.lcUnread = true
    if counted then tab.lcCount = (tab.lcCount or 0) + 1 end
    if mention then tab.lcMention = true end
    self:StyleTab(tab, false)
end

-- The badge: the dot alone (unread, no count known), or the count in a small pill at the tab's right end; the tab
-- grows to fit it (the tabs are chained, so the ones after it move along).
local function styleBadge(tab, unread, theme)
    local dot, badge = tab.lcDot, tab.lcBadge
    local n = unread and (tab.lcCount or 0) or 0
    local extra = 0
    dot:ClearAllPoints()
    if n > 0 and badge then
        badge:SetText(n > 99 and "99+" or tostring(n))
        if badge.SetFont then badge:SetFont("Fonts\\FRIZQT__.TTF", 9, "") end
        local bw = math.max(13, floor((badge:GetStringWidth() or 6) + 7))
        dot:SetWidth(bw)
        dot:SetHeight(12)
        dot:SetPoint("RIGHT", tab, "RIGHT", -4, 1)
        badge:ClearAllPoints()
        badge:SetPoint("CENTER", dot, "CENTER", 0, 0)
        local r, g, b
        if tab.lcMention then r, g, b = 1, 0.62, 0.18 else r, g, b = col(theme.accent) end
        -- dark figures on a light badge, white on a dark one
        if 0.3 * r + 0.59 * g + 0.11 * b > 0.5 then badge:SetTextColor(0.07, 0.07, 0.08) else badge:SetTextColor(1, 1, 1) end
        badge:Show()
        extra = bw + 3
    else
        dot:SetWidth(6)
        dot:SetHeight(6)
        dot:SetPoint("TOPRIGHT", tab, "TOPRIGHT", -3, -3)
        if badge then badge:Hide() end
    end
    dot:SetAlpha(unread and 1 or 0)
    if tab.lcBase then
        local want = tab.lcBase + extra
        if tab.lcWant ~= want then
            tab.lcWant = want
            tab:SetWidth(want)
        end
        local fs = (tab.lcOrig and tab.lcOrig.fs) or (tab.lcCentred and tab.label)
        if fs then
            fs:ClearAllPoints()
            fs:SetPoint("CENTER", tab, "CENTER", -floor(extra / 2), 1)
        end
    end
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
    styleBadge(tab, (tab.lcUnread and not selected) and true or false, theme)
    local fs = tab.lcOrig and tab.lcOrig.fs or tab.label
    if fs then
        if selected then fs:SetTextColor(col(theme.text)) else fs:SetTextColor(col(theme.muted)) end
        if fs.lcSetAlpha then fs.lcSetAlpha(1) elseif fs.SetAlpha then fs:SetAlpha(1) end
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
            if selected then tab.lcUnread, tab.lcMention, tab.lcCount = nil, nil, nil end
            self:StyleTab(tab, selected, theme)
        end
    end
    if self.StyleWorldTab then self:StyleWorldTab(theme) end
    self:RefreshInputLabels()
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
        -- the header ("Say:") is clickable while typing: the next channel (right-click: the one before)
        if o.header and not eb.lcHeadBtn then
            local hb = CreateFrame("Button", (eb:GetName() or "LeylineChatEdit") .. "LCHeader", eb)
            hb:SetAllPoints(o.header)
            hb:RegisterForClicks("LeftButtonUp", "RightButtonUp")
            hb:SetScript("OnClick", function(_, button)
                if C.CycleChannel then C:CycleChannel(eb, button == "RightButton", true) end
                if eb.SetFocus then eb:SetFocus() end
            end)
            hb:Hide()
            eb.lcHeadBtn = hb
        end
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
    -- its backdrop shows only while you type (C:StyleInputs): at rest the panel's well (which fades with the
    -- window) stands in. The box hangs off the screen, so the panel's fade never reached these two textures;
    -- with the im chat style (and Forever UI) the box stays shown at rest, and they were the grey bar left
    -- across the bottom of a faded chat.
    eb.lcBgA = math.min(1, (mainSkin.opacity or 0.85) + 0.15)
    eb.lcInputA = nil
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
    eb.lcInputA = nil
    if eb.lcHeadBtn then eb.lcHeadBtn:Hide() end
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
    self:RefreshInputLabels()
end

-- -- Where you are talking: the well's label ----------------------------------------------------------

local CHAT_NAMES = {
    SAY = "Say", YELL = "Yell", EMOTE = "Emote", PARTY = "Party", RAID = "Raid", RAID_WARNING = "Raid warning",
    BATTLEGROUND = "Battleground", GUILD = "Guild", OFFICER = "Officer", WHISPER = "Whisper", BN_WHISPER = "Whisper",
    AFK = "Away", DND = "Busy",
}

-- "Say", "Guild", "World", "Trade", "To Bob", with the channel's colour.
function C:ChannelLabel(eb)
    if not eb then return "Say", 1, 1, 1 end
    local t = C.EditAttr(eb, "chatType") or "SAY"
    local label = CHAT_NAMES[t]
    local info = ChatTypeInfo and ChatTypeInfo[t]
    if t == "CHANNEL" then
        local id = tonumber(C.EditAttr(eb, "channelTarget")) or 0
        local _, name = C.Try(GetChannelName, id)
        if type(name) == "string" and name ~= "" then
            label = string.match(name, "^(.-)%s+%-%s+.*$") or name      -- "Trade - City" -> "Trade"
        else
            label = "Channel " .. id
        end
        info = ChatTypeInfo and (ChatTypeInfo["CHANNEL" .. id] or info)
    elseif t == "WHISPER" or t == "BN_WHISPER" then
        local who = C.EditAttr(eb, "tellTarget")
        if type(who) == "string" and who ~= "" then label = "To " .. who end
    end
    if not label then label = string.upper(string.sub(t, 1, 1)) .. string.lower(string.sub(t, 2)) end
    local r, g, b = 1, 1, 1
    if info then r, g, b = info.r or 1, info.g or 1, info.b or 1 end
    if self.inked then r, g, b = C.InkRGB(r, g, b) end
    return label, r, g, b
end

-- The edit box Enter opens from this window (the stock choice when the client has it).
function C:InputFor(f)
    local own = (f and f ~= self.worldFrame) and f or nil
    local eb = ChatEdit_ChooseBoxForSend and C.Try(ChatEdit_ChooseBoxForSend, own)
    if not eb and own and own.editBox and GetCVar and C.Try(GetCVar, "chatStyle") == "im" then eb = own.editBox end
    return eb or C.MainEditBox()
end

function C:OpenInput(f)
    self.lastActivity = now()
    if ChatFrame_OpenChat then C.Try(ChatFrame_OpenChat, "", (f and f ~= self.worldFrame) and f or nil) end
end

function C:CycleInput(f, back)
    self.lastActivity = now()
    local eb = self:InputFor(f)
    if eb and self.CycleChannel then self:CycleChannel(eb, back, true) end
    self:RefreshInputLabels()
end

-- Each docked panel's well: the label (and the click-to-type button) at rest, nothing over the box while typing.
function C:RefreshInputLabels()
    if not self.active or not self.skinOn then return end
    local typing = self:Typing()
    for _, f in ipairs(self.skinned) do
        local k = f.lcSkin
        local p = k and k.panel
        if p and p.pill then
            if k.docked and not typing then
                local label, r, g, b = self:ChannelLabel(self:InputFor(f))
                local pill = p.pill
                if pill.text:GetText() ~= label then pill.text:SetText(label) end
                pill.text:SetTextColor(r, g, b)
                pill.bg:SetTexture(r, g, b, 0.16)
                pill:SetWidth(math.max(28, floor((pill.text:GetStringWidth() or 24) + 12)))
                pill:Show()
                p.wellBtn:Show()
                p.hint:Show()
            else
                p.pill:Hide()
                p.wellBtn:Hide()
                if k.docked then p.hint:Hide() end
            end
        end
    end
end

-- The edit boxes' own backdrop: shown while that box is typed in, gone at rest. Answers whether anyone is typing.
function C:StyleInputs()
    local typing = false
    for _, eb in ipairs(C.EditBoxes()) do
        if eb.lcBg and eb.lcOrig then
            local on = (eb:IsVisible() and (eb == ACTIVE_CHAT_EDIT_BOX or (eb.HasFocus and eb:HasFocus()))) and true or false
            if on then typing = true end
            local a = on and 1 or 0
            if eb.lcInputA ~= a then
                eb.lcInputA = a
                eb.lcBg:SetAlpha(on and (eb.lcBgA or 1) or 0)
                eb.lcEdge:SetAlpha(a)
                if eb.lcHeadBtn then if on then eb.lcHeadBtn:Show() else eb.lcHeadBtn:Hide() end end
            end
        end
    end
    if typing ~= self.wasTyping then
        self.wasTyping = typing
        self:RefreshInputLabels()
    end
    return typing
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
    self.wasTyping = nil
    self:StyleInputs()
    self:RefreshInputLabels()
    self:SetChrome(self.chromeAlpha or 1)
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
    if C.AtBottom(f) then
        f.lcBelow = 0
        k.jump:Hide()
    elseif f:IsVisible() then
        local j = k.jump
        local n = f.lcBelow or 0
        if j.lcShown ~= n then
            j.lcShown = n
            if n > 0 then
                j.label:SetText((n > 99 and "99+" or tostring(n)) .. (n == 1 and " new line" or " new lines"))
                j.label:Show()
                j.bg:Show()
                j:SetWidth(20 + floor((j.label:GetStringWidth() or 40) + 8))
            else
                j.label:Hide()
                j.bg:Hide()
                j:SetWidth(20)
            end
        end
        j:Show()
    end
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
            k.jump:SetAlpha(a)
            k.bar:SetAlpha(a)
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
    if self:StyleInputs() or self:Hovered() then self.lastActivity = t end
    for _, f in ipairs(self.skinned) do
        if f:IsVisible() then self:UpdateScroll(f) end
    end
    local fade = self:Get("fade")
    local target = 1
    if fade ~= "never" and t - (self.lastActivity or 0) > (tonumber(fade) or 10) then target = 0 end
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
        self:RefreshInputLabels()
        if self.WorldTick then self:WorldTick() end
    end
end

local upkeep = CreateFrame("Frame", "LeylineChatUpkeep", UIParent)
upkeep:SetScript("OnUpdate", function(self, elapsed) C:Update(elapsed) end)
C.upkeep = upkeep
