--[[
    Kit.lua  (LeylineChat)
    The few shared helpers the chat needs, folded in from the old core addon so LeylineChat stands on its own:

        settings     LeylineChatDB (account) and LeylineChatCharDB (per character: what you typed, World routing)
        Print/Trim   one line in the main window; whitespace trimmed
        Art          our own art from Media\ (TGA), or a flat colour when it does not load
        UI           a small window kit: Window, Button, Label, Skin (panel / inset), SearchBox, Menu

    3.3.5a API, Lua 5.1 (no table.getn, getglobal, this/arg1: handlers take their arguments).
]]--

LeylineChat = LeylineChat or {}
local C = LeylineChat

C.ID = "LeylineChat"
C.NAME = "Leyline chat"
C.MEDIA = "Interface\\AddOns\\LeylineChat\\Media\\"

local strmatch = string.match

function C.Print(msg)
    local f = DEFAULT_CHAT_FRAME
    if f and f.AddMessage then f:AddMessage("|cff7fb2ffLeyline|r " .. tostring(msg)) end
end

function C.Trim(s)
    return strmatch(s or "", "^%s*(.-)%s*$") or ""
end

function C.Now() return GetTime and GetTime() or 0 end

-- -- Saved settings --------------------------------------------------------------------------------
-- The game fills LeylineChatDB / LeylineChatCharDB just before ADDON_LOADED; until then these make
-- empty tables (the defaults apply), which the saved ones then replace.

function C:DB()
    if type(LeylineChatDB) ~= "table" then LeylineChatDB = {} end
    local db = LeylineChatDB
    if type(db.settings) ~= "table" then db.settings = {} end
    if type(db.classes) ~= "table" then db.classes = {} end
    return db
end

function C:CharDB()
    if type(LeylineChatCharDB) ~= "table" then LeylineChatCharDB = {} end
    local db = LeylineChatCharDB
    if type(db.history) ~= "table" then db.history = {} end
    return db
end

function C:Settings() return self:DB().settings end

-- -- Art -------------------------------------------------------------------------------------------
-- tex:SetTexture(our art) or, failing that, fallback (a path, or {r, g, b, a}). True when our art loaded
-- (3.3.5 SetTexture answers 1 or nil).

function C:Art(tex, name, fallback)
    if not tex then return false end
    if name and tex:SetTexture(self.MEDIA .. name) then return true end
    if type(fallback) == "table" then
        tex:SetTexture(fallback[1], fallback[2], fallback[3], fallback[4] or 1)
    elseif fallback then
        tex:SetTexture(fallback)
    else
        tex:SetTexture(nil)
    end
    return false
end

-- Call a function the client may lack (or that may fail): nil instead of an error.
function C.Try(fn, ...)
    if type(fn) ~= "function" then return nil end
    local ok, a, b, c = pcall(fn, ...)
    if ok then return a, b, c end
    return nil
end

-- hooksecurefunc on a global function, only when the client has it.
function C.Hook(name, fn)
    if type(_G[name]) ~= "function" or not hooksecurefunc then return false end
    hooksecurefunc(name, fn)
    return true
end

-- Edit box hooks (typed history, the Tab channel cycle): each file adds one; every edit box gets each once.
C.editBoxHooks = C.editBoxHooks or {}
function C:HookEditBox(eb)
    if not eb then return end
    eb.lcHooks = eb.lcHooks or {}
    for name, fn in pairs(self.editBoxHooks) do
        if not eb.lcHooks[name] then eb.lcHooks[name] = true; fn(eb) end
    end
end
function C:HookEditBoxes()
    for _, eb in ipairs(self.EditBoxes()) do self:HookEditBox(eb) end
end

-- -- The window kit --------------------------------------------------------------------------------

local UI = {}
C.UI = UI

UI.FONT = "Fonts\\FRIZQT__.TTF"
UI.WHITE = "Interface\\Buttons\\WHITE8X8"
UI.SLATE = { 0.07, 0.075, 0.09, 0.94 }      -- panel face
UI.INSET = { 0.035, 0.04, 0.05, 0.85 }      -- recessed areas (lists, inputs)
UI.EDGE = { 0.36, 0.38, 0.44, 1 }           -- thin border
UI.TEXT = { 0.93, 0.93, 0.95 }
UI.MUTED = { 0.62, 0.64, 0.70 }             -- secondary text
UI.ACCENT = { 1, 0.82, 0 }                  -- gold
UI.THIN = { bgFile = UI.WHITE, edgeFile = UI.WHITE, edgeSize = 1, tile = false,
    insets = { left = 1, right = 1, top = 1, bottom = 1 } }

-- A flat 1 px border and face. style: "panel" | "inset" | "raised"
function UI.Skin(f, style)
    local face = (style == "inset") and UI.INSET or UI.SLATE
    if f.SetBackdrop then
        f:SetBackdrop(UI.THIN)
        f:SetBackdropColor(face[1], face[2], face[3], face[4])
        if style == "raised" then
            f:SetBackdropBorderColor(0.50, 0.52, 0.58, 1)
        else
            f:SetBackdropBorderColor(UI.EDGE[1], UI.EDGE[2], UI.EDGE[3], UI.EDGE[4])
        end
    end
    return f
end

function UI.Label(parent, size, color, flags, layer)
    local fs = parent:CreateFontString(nil, layer or "OVERLAY", "GameFontHighlightSmall")
    if fs.SetFont then fs:SetFont(UI.FONT, size or 11, flags or "") end
    if color then fs:SetTextColor(color[1], color[2], color[3], color[4] or 1) end
    if fs.SetJustifyH then fs:SetJustifyH("LEFT") end
    return fs
end

-- The stock panel button; a plain bordered one if the template is missing.
function UI.Button(parent, text, width, height, onClick)
    local ok, b = pcall(CreateFrame, "Button", nil, parent, "UIPanelButtonTemplate")
    if not ok or not b then
        b = CreateFrame("Button", nil, parent)
        UI.Skin(b, "raised")
        local fs = UI.Label(b, 10, UI.TEXT)
        fs:SetPoint("CENTER", b, "CENTER", 0, 0)
        b.label = fs
        b.SetText = function(self, t) self.label:SetText(t) end
    end
    b:SetWidth(width or 80)
    b:SetHeight(height or 20)
    b:SetText(text or "")
    if onClick then b:SetScript("OnClick", function(self, button) onClick(self, button) end) end
    return b
end

-- opts: width, height, title, strata
function UI.Window(name, opts)
    opts = opts or {}
    local f = CreateFrame("Frame", name, UIParent)
    f:SetWidth(opts.width or 220)
    f:SetHeight(opts.height or 120)
    f:SetPoint("CENTER", UIParent, "CENTER", 0, 0)
    f:SetFrameStrata(opts.strata or "DIALOG")
    UI.Skin(f, "panel")
    f:SetMovable(true)
    f:EnableMouse(true)
    if f.SetClampedToScreen then f:SetClampedToScreen(true) end

    local bar = f:CreateTexture(nil, "ARTWORK")
    bar:SetPoint("TOPLEFT", f, "TOPLEFT", 1, -1)
    bar:SetPoint("TOPRIGHT", f, "TOPRIGHT", -1, -1)
    bar:SetHeight(22)
    bar:SetTexture(0.12, 0.125, 0.15, 1)
    f.titleBar = bar
    local rule = f:CreateTexture(nil, "ARTWORK")
    rule:SetPoint("TOPLEFT", bar, "BOTTOMLEFT", 0, 0)
    rule:SetPoint("TOPRIGHT", bar, "BOTTOMRIGHT", 0, 0)
    rule:SetHeight(1)
    rule:SetTexture(UI.ACCENT[1], UI.ACCENT[2], UI.ACCENT[3], 0.5)

    f.title = UI.Label(f, 12, UI.TEXT, "OUTLINE")
    f.title:SetPoint("LEFT", bar, "LEFT", 8, 0)
    f.title:SetText(opts.title or "")

    local ok, close = pcall(CreateFrame, "Button", nil, f, "UIPanelCloseButton")
    if not ok or not close then close = UI.Button(f, "x", 20, 18) end
    close:SetPoint("RIGHT", bar, "RIGHT", 4, 0)
    close:SetScript("OnClick", function() f:Hide() end)
    f.closeButton = close

    -- the title strip is the drag handle
    local h = CreateFrame("Button", nil, f)
    h:SetPoint("TOPLEFT", bar, "TOPLEFT", 0, 0)
    h:SetPoint("BOTTOMRIGHT", bar, "BOTTOMRIGHT", -24, 0)
    h:RegisterForDrag("LeftButton")
    h:SetScript("OnDragStart", function() f:StartMoving() end)
    h:SetScript("OnDragStop", function() f:StopMovingOrSizing() end)
    f.dragHandle = h

    if UISpecialFrames and name then table.insert(UISpecialFrames, name) end   -- Escape closes it
    return f
end

-- An inset edit box with a grey placeholder. opts: name, width, placeholder, onChange(text)
function UI.SearchBox(parent, opts)
    local e = CreateFrame("EditBox", opts.name, parent)
    UI.Skin(e, "inset")
    e:SetWidth(opts.width or 140)
    e:SetHeight(18)
    e:SetAutoFocus(false)
    e:SetFontObject(ChatFontNormal or GameFontHighlightSmall)
    if e.SetTextInsets then e:SetTextInsets(6, 6, 0, 0) end
    local ph = UI.Label(e, 10, UI.MUTED)
    ph:SetPoint("LEFT", e, "LEFT", 6, 0)
    ph:SetText(opts.placeholder or "Search")
    e:SetScript("OnTextChanged", function(self)
        local t = self:GetText() or ""
        if t == "" then ph:Show() else ph:Hide() end
        if opts.onChange then opts.onChange(t) end
    end)
    e:SetScript("OnEscapePressed", function(self) self:SetText(""); self:ClearFocus() end)
    e:SetScript("OnEnterPressed", function(self) self:ClearFocus() end)
    e.placeholder = ph
    return e
end

-- One small context menu (not UIDropDownMenu, so it never fights the stock menus).
-- items = { { text =, func =, checked = ?, disabled = ?, title = ? }, ... }
local menu
function UI.Menu(anchor, items)
    if not menu then
        menu = CreateFrame("Frame", "LeylineChatMenu", UIParent)
        UI.Skin(menu, "raised")
        menu:SetFrameStrata("FULLSCREEN_DIALOG")
        if menu.SetClampedToScreen then menu:SetClampedToScreen(true) end
        menu:EnableMouse(true)
        menu.buttons = {}
        if UISpecialFrames then table.insert(UISpecialFrames, "LeylineChatMenu") end
        menu:SetScript("OnUpdate", function(self, elapsed)
            -- close after the mouse has been away for a moment
            if MouseIsOver and not MouseIsOver(self) then
                self.away = (self.away or 0) + (elapsed or 0)
                if self.away > 1.2 then self:Hide() end
            else
                self.away = 0
            end
        end)
    end
    local width = 120
    for i, it in ipairs(items) do
        local b = menu.buttons[i]
        if not b then
            b = CreateFrame("Button", nil, menu)
            b:SetHeight(16)
            b.text = UI.Label(b, 10)
            b.text:SetPoint("LEFT", b, "LEFT", 16, 0)
            b.check = b:CreateTexture(nil, "OVERLAY")
            b.check:SetWidth(12)
            b.check:SetHeight(12)
            b.check:SetPoint("LEFT", b, "LEFT", 2, 0)
            b.check:SetTexture("Interface\\Buttons\\UI-CheckBox-Check")
            local hl = b:CreateTexture(nil, "HIGHLIGHT")
            hl:SetAllPoints(b)
            hl:SetTexture(1, 0.82, 0, 0.12)
            menu.buttons[i] = b
        end
        b.text:SetText(it.text or "")
        local c = it.color or (it.title and UI.ACCENT) or (it.disabled and UI.MUTED) or { 1, 1, 1 }
        b.text:SetTextColor(c[1], c[2], c[3])
        if it.checked then b.check:Show() else b.check:Hide() end
        local item = it
        b:SetScript("OnClick", function()
            if item.title or item.disabled then return end
            menu:Hide()
            if item.func then item.func() end
        end)
        width = math.max(width, (b.text:GetStringWidth() or 60) + 28)
        b:ClearAllPoints()
        b:SetPoint("TOPLEFT", menu, "TOPLEFT", 4, -4 - (i - 1) * 16)
        b:Show()
    end
    for i = #items + 1, #menu.buttons do menu.buttons[i]:Hide() end
    for _, b in ipairs(menu.buttons) do b:SetWidth(width) end
    menu.items = items
    menu:SetWidth(width + 8)
    menu:SetHeight(#items * 16 + 8)
    menu:ClearAllPoints()
    if anchor then
        menu:SetPoint("BOTTOMLEFT", anchor, "TOPLEFT", 0, 2)
    else
        menu:SetPoint("CENTER", UIParent, "CENTER", 0, 0)
    end
    menu.away = 0
    menu:Show()
    return menu
end
