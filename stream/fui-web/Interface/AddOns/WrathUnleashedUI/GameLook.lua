-- GameLook.lua (3.3.5a API): the game's own interface look for the bots' frames (the Bots bar, its menu, the orders
-- bar, the first-time tips and the Bots window). Owner, 12 Nov: "make this companion window as you'd expect with
-- forever ui elements not leyline squarechunks".
--
-- Only the client's own templates and texture PATHS (the client draws them; no art is copied into this add-on):
--   windows      Forever UI's panel art (ForeverUI.SetPanelArt, as its bag windows) or dialog frame
--                (ForeverUI.Templates.DialogFrame); else the stock dialog: Interface\DialogFrame\UI-DialogBox-Border over
--                UI-DialogBox-Background-Dark, with the gold UI-DialogBox-Header banner holding the title.
--   buttons      UIPanelButtonTemplate (the red and gold panel button); with Forever UI its red three-slice button
--                (ForeverUI.Templates.ThreeSliceButton "128-redbutton", as its Esc menu).
--   close        UIPanelCloseButton; with Forever UI its red X (ForeverUI.Templates.CloseButton).
--   tabs         CharacterFrameTabButtonTemplate under the window, selected with PanelTemplates_SetTab.
--   boxes, tips  the tooltip's rounded edge (Interface\Tooltips\UI-Tooltip-Border over UI-Tooltip-Background) in the
--                tooltip's own colours; group boxes in the options panels' grey.
--   fonts        GameFontNormal (gold), GameFontHighlight (white), GameFontDisable (grey) and their Small sizes.
-- Every Forever UI call and every template is guarded: a client without one gets the stock look, never an error.

local W = WrathUnleashedUI
local G = {}
W.GameLook = G

G.DIALOG_BG = "Interface\\DialogFrame\\UI-DialogBox-Background-Dark"
G.DIALOG_EDGE = "Interface\\DialogFrame\\UI-DialogBox-Border"
G.DIALOG_HEADER = "Interface\\DialogFrame\\UI-DialogBox-Header"
G.TIP_BG = "Interface\\Tooltips\\UI-Tooltip-Background"
G.TIP_EDGE = "Interface\\Tooltips\\UI-Tooltip-Border"
G.STATUSBAR = "Interface\\TargetingFrame\\UI-StatusBar"
G.PARTY_FRAME = "Interface\\TargetingFrame\\UI-PartyFrame"
G.ROW_HILITE = "Interface\\QuestFrame\\UI-QuestTitleHighlight"
G.SLOT = "Interface\\Buttons\\UI-Quickslot2"
G.SLOT_DOWN = "Interface\\Buttons\\UI-Quickslot-Depress"
G.SQUARE_HILITE = "Interface\\Buttons\\ButtonHilight-Square"
G.CHECK_HILITE = "Interface\\Buttons\\CheckButtonHilight"
G.PLUS_UP = "Interface\\Buttons\\UI-PlusButton-Up"
G.PLUS_DOWN = "Interface\\Buttons\\UI-PlusButton-Down"
G.PLUS_HILITE = "Interface\\Buttons\\UI-PlusButton-Hilight"
G.GOLD = { 1, 0.82, 0 }          -- NORMAL_FONT_COLOR
G.GREY = { 0.5, 0.5, 0.5 }       -- GRAY_FONT_COLOR

G.DIALOG = { bgFile = G.DIALOG_BG, edgeFile = G.DIALOG_EDGE, tile = true, tileSize = 32, edgeSize = 32,
    insets = { left = 11, right = 12, top = 12, bottom = 11 } }
G.TOOLTIP = { bgFile = G.TIP_BG, edgeFile = G.TIP_EDGE, tile = true, tileSize = 16, edgeSize = 16,
    insets = { left = 5, right = 5, top = 5, bottom = 5 } }

local function try(fn, ...)
    if type(fn) ~= "function" then return false end
    local ok, a, b = pcall(fn, ...)
    if not ok then return false end
    return true, a, b
end
G.Try = try

-- Forever UI, when it is loaded and has the template table its own windows use.
function G.FUI()
    local F = rawget(_G, "ForeverUI")
    if type(F) == "table" and type(F.Templates) == "table" then return F end
    return nil
end

local function backdrop(frame, bd)
    if not frame.SetBackdrop then return false end
    frame:SetBackdrop(bd)
    return true
end

-- The tooltip's colours (the client's TOOLTIP_DEFAULT_* tables, else their stock values).
local function tooltipColours(frame, alpha)
    local bg = rawget(_G, "TOOLTIP_DEFAULT_BACKGROUND_COLOR") or { r = 0.09, g = 0.09, b = 0.19 }
    local e = rawget(_G, "TOOLTIP_DEFAULT_COLOR") or { r = 1, g = 1, b = 1 }
    if frame.SetBackdropColor then frame:SetBackdropColor(bg.r, bg.g, bg.b, alpha or 0.92) end
    if frame.SetBackdropBorderColor then frame:SetBackdropBorderColor(e.r, e.g, e.b, 1) end
end

-- A dialog: Forever UI's dialog frame, else the stock dialog border on the dark dialog background. A dark wash from
-- the tooltip's background file (tinted) keeps text readable where the dialog background draws faint.
function G.Dialog(frame)
    local F = G.FUI()
    if F and try(F.Templates.DialogFrame, frame) then
        frame.gameLook = "forever-dialog"
        return frame
    end
    backdrop(frame, G.DIALOG)
    if frame.SetBackdropColor then frame:SetBackdropColor(1, 1, 1, 1) end
    if frame.SetBackdropBorderColor then frame:SetBackdropBorderColor(1, 1, 1, 1) end
    if not frame.gameWash then
        local t = frame:CreateTexture(nil, "BACKGROUND")
        t:SetTexture(G.TIP_BG)
        if t.SetVertexColor then t:SetVertexColor(0, 0, 0, 0.55) end
        t:SetPoint("TOPLEFT", 11, -12); t:SetPoint("BOTTOMRIGHT", -12, 11)
        frame.gameWash = t
    end
    frame.gameLook = "dialog"
    return frame
end

-- A window with a title: Forever UI's panel art (the metal frame, the portrait ring, the title band as its bags),
-- else its dialog frame, else the stock dialog with the gold header banner. Returns the title FontString.
function G.Window(frame, title, portraitIcon)
    local F = G.FUI()
    local fs
    if F and try(F.SetPanelArt, frame) then
        frame.gameLook = "forever-panel"
        local p = frame:CreateTexture(nil, "BORDER")
        p:SetWidth(28); p:SetHeight(28)
        p:SetPoint("CENTER", frame, "TOPLEFT", 14, -17)
        if not (portraitIcon and SetPortraitToTexture and try(SetPortraitToTexture, p, portraitIcon)) then
            p:SetTexture(portraitIcon or "Interface\\Icons\\INV_Misc_GroupNeedMore")
            if p.SetTexCoord then p:SetTexCoord(4 / 64, 60 / 64, 4 / 64, 60 / 64) end
        end
        frame.gamePortrait = p
        fs = frame:CreateFontString(nil, "OVERLAY", "GameFontNormal")
        fs:SetPoint("TOP", frame, "TOP", 0, -6)
    else
        G.Dialog(frame)
        if frame.gameLook == "dialog" then
            local h = frame:CreateTexture(nil, "ARTWORK")
            h:SetTexture(G.DIALOG_HEADER)
            h:SetWidth(256); h:SetHeight(64)
            h:SetPoint("TOP", frame, "TOP", 0, 12)
            frame.gameHeader = h
            fs = frame:CreateFontString(nil, "OVERLAY", "GameFontNormal")
            fs:SetPoint("TOP", h, "TOP", 0, -14)
        else
            fs = frame:CreateFontString(nil, "OVERLAY", "GameFontNormal")
            fs:SetPoint("TOP", frame, "TOP", 0, -8)
        end
    end
    fs:SetText(title or "")
    frame.gameTitle = fs
    return fs
end

-- The close button: Forever UI's red X, else UIPanelCloseButton. Hides its window.
function G.Close(frame)
    local b = CreateFrame("Button", nil, frame, "UIPanelCloseButton")
    local F = G.FUI()
    b:ClearAllPoints()
    if F and try(F.Templates.CloseButton, b, frame) then
        b:ClearAllPoints()
        b:SetPoint("TOPRIGHT", frame, "TOPRIGHT", 1, 0)
        b.gameLook = "forever-close"
    else
        b:SetPoint("TOPRIGHT", frame, "TOPRIGHT", -3, -3)
        b.gameLook = "UIPanelCloseButton"
    end
    b:SetScript("OnClick", function(self) self:GetParent():Hide() end)
    return b
end

-- A panel button: UIPanelButtonTemplate (or Forever UI's red three-slice button on it). 22 high.
local fuiFonts
function G.Button(parent, text, w, onClick, ...)
    local b = CreateFrame("Button", nil, parent, "UIPanelButtonTemplate")
    b:SetWidth(w or 90); b:SetHeight(22)
    b.gameLook = "UIPanelButtonTemplate"
    local F = G.FUI()
    if F then
        if not fuiFonts and GameFontHighlightSmall then
            fuiFonts = { GameFontHighlightSmall, GameFontHighlightSmall, GameFontDisableSmall or GameFontHighlightSmall }
        end
        if try(F.Templates.ThreeSliceButton, b, "128-redbutton", fuiFonts) then b.gameLook = "forever-redbutton" end
    end
    b:SetText(text or "")
    if select("#", ...) > 0 then b:SetPoint(...) end
    if onClick then b:SetScript("OnClick", onClick) end
    return b
end

-- "This one is picked" on a panel button: the button's own highlight, held.
function G.Select(b, on)
    if not b then return end
    if on then if b.LockHighlight then b:LockHighlight() end
    else if b.UnlockHighlight then b:UnlockHighlight() end end
    b.gameSelected = on and true or nil
end

-- A tooltip-styled box (the tips, the bar's header, the orders bar).
function G.Tooltip(frame, alpha)
    backdrop(frame, G.TOOLTIP)
    tooltipColours(frame, alpha)
    frame.gameLook = "tooltip"
    return frame
end

-- A group box inside a window (the options panels' OptionsBoxTemplate look).
function G.Inset(frame)
    backdrop(frame, G.TOOLTIP)
    if frame.SetBackdropColor then frame:SetBackdropColor(0.1, 0.1, 0.1, 0.5) end
    if frame.SetBackdropBorderColor then frame:SetBackdropBorderColor(0.4, 0.4, 0.4, 1) end
    frame.gameLook = "inset"
    return frame
end
-- its edge in gold (what is on) or back to grey
function G.InsetOn(frame, on)
    if frame.SetBackdropBorderColor then
        if on then frame:SetBackdropBorderColor(G.GOLD[1], G.GOLD[2], G.GOLD[3], 1)
        else frame:SetBackdropBorderColor(0.4, 0.4, 0.4, 1) end
    end
    frame.gameOn = on and true or nil
end

-- The list row highlight the game's lists use (quest log, friends list), shown on hover.
function G.RowHighlight(row)
    local t = row:CreateTexture(nil, "HIGHLIGHT")
    t:SetTexture(G.ROW_HILITE)
    if t.SetBlendMode then t:SetBlendMode("ADD") end
    t:SetAllPoints(row)
    if t.SetAlpha then t:SetAlpha(0.6) end
    t:Hide()
    return t
end

-- A status bar on the game's own bar texture, with a dark trough from the same file.
function G.StatusBar(parent, w, h, ...)
    local back = parent:CreateTexture(nil, "BORDER")
    back:SetTexture(G.STATUSBAR)
    if back.SetVertexColor then back:SetVertexColor(0, 0, 0, 0.55) end
    back:SetWidth(w); back:SetHeight(h)
    back:SetPoint(...)
    local s = CreateFrame("StatusBar", nil, parent)
    s:SetWidth(w); s:SetHeight(h)
    s:SetPoint("CENTER", back, "CENTER", 0, 0)
    s:SetStatusBarTexture(G.STATUSBAR)
    s:SetMinMaxValues(0, 1); s:SetValue(1)
    s.back = back
    return s
end

-- An icon button in the action button's dress: the empty-slot ring, the pressed ring, the square highlight, and the
-- checked glow for "on".
function G.ActionButton(b, size)
    if b.SetNormalTexture then b:SetNormalTexture(G.SLOT) end
    local nt = b.GetNormalTexture and b:GetNormalTexture()
    if nt then
        nt:ClearAllPoints()
        nt:SetWidth(size * 66 / 36); nt:SetHeight(size * 66 / 36)
        nt:SetPoint("CENTER", b, "CENTER", 0, -1)
    end
    if b.SetPushedTexture then b:SetPushedTexture(G.SLOT_DOWN) end
    if b.SetHighlightTexture then b:SetHighlightTexture(G.SQUARE_HILITE, "ADD") end
    local on = b:CreateTexture(nil, "OVERLAY")
    on:SetTexture(G.CHECK_HILITE)
    if on.SetBlendMode then on:SetBlendMode("ADD") end
    on:SetAllPoints(b)
    on:Hide()
    b.gameLook = "action"
    return on
end

-- The bottom tabs of a game window: CharacterFrameTabButtonTemplate, named <window>Tab<i> (the template and
-- PanelTemplates find their parts by name). The window must have a name.
function G.Tab(win, i, text, onClick)
    local name = (win.GetName and win:GetName() or "WrathUnleashedLookWindow") .. "Tab" .. i
    local b = CreateFrame("Button", name, win, "CharacterFrameTabButtonTemplate")
    if b.SetID then b:SetID(i) end
    b:SetText(text or "")
    b.gameLook = "CharacterFrameTabButtonTemplate"
    b:SetScript("OnClick", function(self)
        if PlaySound then try(PlaySound, "igCharacterInfoTab") end
        if onClick then onClick(self) end
    end)
    local function fit(self)
        if PanelTemplates_TabResize then try(PanelTemplates_TabResize, self, 0) end
        local hl = _G[name .. "HighlightTexture"]
        if hl and hl.SetWidth and self.GetTextWidth then try(hl.SetWidth, hl, (tonumber((self:GetTextWidth())) or 40) + 31) end
    end
    fit(b)
    b:SetScript("OnShow", fit)
    b:ClearAllPoints()
    if i == 1 then b:SetPoint("TOPLEFT", win, "BOTTOMLEFT", 12, 7)
    else b:SetPoint("LEFT", _G[(win:GetName() or "") .. "Tab" .. (i - 1)] or win, "RIGHT", -16, 0) end
    return b
end

function G.SelectTab(win, at, n, tabs)
    win.numTabs = n
    if PanelTemplates_SetNumTabs then try(PanelTemplates_SetNumTabs, win, n) end
    if PanelTemplates_SetTab and try(PanelTemplates_SetTab, win, at) then
        win.gameTab = at
        return true
    end
    -- no PanelTemplates: the picked tab bright, the others dimmed
    for i, t in ipairs(tabs or {}) do
        local b = t.btn or t
        if b and b.SetAlpha then b:SetAlpha(i == at and 1 or 0.6) end
    end
    win.gameTab = at
    return false
end
