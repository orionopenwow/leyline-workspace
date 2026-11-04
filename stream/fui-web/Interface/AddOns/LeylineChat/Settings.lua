--[[
    Settings.lua  (LeylineChat)
    The chat settings window (/lchat, the chat menu's "All chat settings...", the tab menu): every option of
    C.SCHEMA in two columns. A switch flips on click; a choice steps on click (right-click steps back); a number has
    - and + buttons. Hover a row for what it does. Changes apply at once and are saved (LeylineChatDB).
    (The 1.12 addon's options lived in the shared Addons window of its core addon; this window stands in for it.)
]]--

local C = LeylineChat
local UI = C.UI
local format = string.format

C.SETTINGS_ROW_H = 22
C.SETTINGS_COL_W = 300

local function choiceIndex(opt, value)
    for i, c in ipairs(opt.choices) do if c.value == value then return i end end
    return 1
end

local function valueText(opt)
    local v = C:Get(opt.key)
    if opt.type == "choice" then
        local c = opt.choices[choiceIndex(opt, v)]
        return c and c.label or tostring(v)
    elseif opt.type == "slider" then
        return format(opt.fmt or "%d", math.floor((tonumber(v) or opt.min or 0) + 0.5))
    end
    return v and "On" or "Off"
end

function C:StepOption(opt, dir)
    if opt.type == "choice" then
        local i = choiceIndex(opt, self:Get(opt.key)) + (dir or 1)
        if i > #opt.choices then i = 1 elseif i < 1 then i = #opt.choices end
        self:SetOption(opt.key, opt.choices[i].value)
    elseif opt.type == "slider" then
        local v = (tonumber(self:Get(opt.key)) or opt.min) + (dir or 1) * (opt.step or 1)
        if v < opt.min then v = opt.min elseif v > opt.max then v = opt.max end
        self:SetOption(opt.key, v)
    else
        self:SetOption(opt.key, not self:Get(opt.key))
    end
end

local function row(parent, opt)
    local r = CreateFrame("Button", nil, parent)
    r:SetWidth(C.SETTINGS_COL_W - 8)
    r:SetHeight(C.SETTINGS_ROW_H - 2)
    r:RegisterForClicks("LeftButtonUp", "RightButtonUp")
    local hl = r:CreateTexture(nil, "HIGHLIGHT")
    hl:SetAllPoints(r)
    hl:SetTexture(1, 0.82, 0, 0.08)
    r.label = UI.Label(r, 11, UI.TEXT)
    r.label:SetPoint("LEFT", r, "LEFT", 24, 0)
    r.label:SetText(opt.label)
    r.value = UI.Label(r, 11, UI.ACCENT)
    r.value:SetPoint("RIGHT", r, "RIGHT", -6, 0)
    if not opt.type then
        -- a switch: a box with a tick
        r.box = r:CreateTexture(nil, "ARTWORK")
        r.box:SetWidth(14); r.box:SetHeight(14)
        r.box:SetPoint("LEFT", r, "LEFT", 4, 0)
        r.box:SetTexture(UI.INSET[1], UI.INSET[2], UI.INSET[3], 1)
        r.check = r:CreateTexture(nil, "OVERLAY")
        r.check:SetWidth(18); r.check:SetHeight(18)
        r.check:SetPoint("CENTER", r.box, "CENTER", 1, 1)
        r.check:SetTexture("Interface\\Buttons\\UI-CheckBox-Check")
    elseif opt.type == "slider" then
        r.minus = UI.Button(r, "-", 20, 18, function() C:StepOption(opt, -1) end)
        r.minus:SetPoint("LEFT", r, "LEFT", 0, 0)
        r.label:ClearAllPoints()
        r.label:SetPoint("LEFT", r.minus, "RIGHT", 4, 0)
        r.plus = UI.Button(r, "+", 20, 18, function() C:StepOption(opt, 1) end)
        r.plus:SetPoint("RIGHT", r, "RIGHT", 0, 0)
        r.value:ClearAllPoints()
        r.value:SetPoint("RIGHT", r.plus, "LEFT", -4, 0)
    end
    r:SetScript("OnClick", function(self, button)
        if opt.type == "slider" then return end
        C:StepOption(opt, button == "RightButton" and -1 or 1)
    end)
    r:SetScript("OnEnter", function(self)
        if GameTooltip and (opt.sub or opt.type == "choice") then
            GameTooltip:SetOwner(self, "ANCHOR_RIGHT")
            GameTooltip:SetText(opt.label, 1, 1, 1)
            if opt.sub then GameTooltip:AddLine(opt.sub, 0.85, 0.85, 0.85, true) end
            if opt.type == "choice" then GameTooltip:AddLine("Click for the next choice, right-click for the one before.", 0.6, 0.6, 0.6, true) end
            GameTooltip:Show()
        end
    end)
    r:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
    r.opt = opt
    return r
end

function C:SettingsWindow()
    if self.settingsWin then return self.settingsWin end
    local opts = self.SCHEMA.options
    local perCol = math.ceil(#opts / 2)
    local f = UI.Window("LeylineChatSettings", { width = 2 * C.SETTINGS_COL_W + 24,
        height = 96 + perCol * C.SETTINGS_ROW_H + 40, title = "Chat settings" })
    f.desc = UI.Label(f, 10, UI.MUTED)
    f.desc:SetPoint("TOPLEFT", f, "TOPLEFT", 14, -32)
    f.desc:SetWidth(2 * C.SETTINGS_COL_W - 8)
    f.desc:SetText(self.SCHEMA.desc)
    -- on / off for the whole addon
    f.power = row(f, { key = "enabled", label = "Use " .. C.NAME, sub = "Off puts the game's own chat back exactly as it was." })
    f.power:SetPoint("TOPLEFT", f, "TOPLEFT", 10, -56)
    f.rows = {}
    for i, opt in ipairs(opts) do
        local r = row(f, opt)
        local colIndex = (i <= perCol) and 0 or 1
        local line = (i <= perCol) and (i - 1) or (i - 1 - perCol)
        r:SetPoint("TOPLEFT", f, "TOPLEFT", 10 + colIndex * (C.SETTINGS_COL_W + 4), -84 - line * C.SETTINGS_ROW_H)
        table.insert(f.rows, r)
    end
    f.copy = UI.Button(f, "Copy chat", 100, 22, function() C:OpenCopy() end)
    f.copy:SetPoint("BOTTOMLEFT", f, "BOTTOMLEFT", 12, 10)
    f.expand = UI.Button(f, "Chat history", 110, 22, function() C:OpenExpanded() end)
    f.expand:SetPoint("LEFT", f.copy, "RIGHT", 6, 0)
    f.done = UI.Button(f, "Close", 80, 22, function() f:Hide() end)
    f.done:SetPoint("BOTTOMRIGHT", f, "BOTTOMRIGHT", -12, 10)
    f:SetScript("OnShow", function() C:RefreshSettings() end)
    f:Hide()
    self.settingsWin = f
    return f
end

local function paint(r)
    local opt = r.opt
    if not opt.type then
        local on = C:Get(opt.key) and true or false
        if on then r.check:Show() else r.check:Hide() end
        r.value:SetText("")
    else
        r.value:SetText(valueText(opt))
    end
    local dim = (opt.key ~= "enabled") and not C:Get("enabled")
    local c = dim and UI.MUTED or UI.TEXT
    r.label:SetTextColor(c[1], c[2], c[3])
end

function C:RefreshSettings()
    local f = self.settingsWin
    if not f or not f:IsShown() then return end
    paint(f.power)
    for _, r in ipairs(f.rows) do paint(r) end
end

function C:ToggleSettings(show)
    local f = self:SettingsWindow()
    if show == nil then show = not f:IsShown() end
    if show then f:Show(); self:RefreshSettings() else f:Hide() end
    return f
end
