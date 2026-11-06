-- The bar of your bots, the orders bar and the first-time tips (3.3.5a API). Both bars show only while bots are in
-- your group (the roster the Bots window asks for, Companions.lua) and keep where you drag them.
--
--   Bot bar     one slot per bot: its portrait (SetPortraitTexture), name in its class colour, role icon, health and
--               mana, and one word for what it is doing: following, fighting, resting, staying, passive, dead.
--               Left-click targets it (a SecureUnitButtonTemplate slot, so it works in combat too); right-click
--               opens its menu (follow, stay, attack my target, passive, gear up, spec and role, send home).
--               Slots are laid out out of combat only (the client's rule for secure frames); a change in combat
--               waits for the fight's end.
--   Orders bar  Follow, Stay, Attack my target, Passive (then Fight) and Pull (your tank bot opens on your target)
--               for every bot. Each has a key binding (Esc > Key Bindings > Bots; Bindings.xml), unbound until you
--               set one; its key shows on the button.
--   Tips        three short tips the first time a character of this account has bots; never again after
--               (/comp tips shows them again).
-- The icons are the game's own (the pet bar's follow, stay, attack and passive art), by path only.
-- /comp bar and /comp orders switch the bars off and on.

local W = WrathUnleashedUI
local C = W.Companions
if not C or not C.UI then return end
local U = C.UI
local B = {}
C.Bar = B

local SLOT_W, SLOT_H, HEAD_H, MAX_SLOTS = 200, 46, 16, 9
local ORDER_SIZE, ORDER_GAP = 34, 6

BINDING_HEADER_WUBOTS = "Bots"
BINDING_NAME_WUBOTS_FOLLOW = "Bots: follow me"
BINDING_NAME_WUBOTS_STAY = "Bots: stay here"
BINDING_NAME_WUBOTS_ATTACK = "Bots: attack my target"
BINDING_NAME_WUBOTS_PASSIVE = "Bots: passive / fight"
BINDING_NAME_WUBOTS_PULL = "Bots: pull my target"
BINDING_NAME_WUBOTS_WINDOW = "Bots window"

local ORDERS = {
    { key = "follow", label = "Follow", bind = "WUBOTS_FOLLOW", icon = PET_FOLLOW_TEXTURE or "Interface\\Icons\\Ability_Tracking",
        tip = "Every bot follows you." },
    { key = "stay", label = "Stay", bind = "WUBOTS_STAY", icon = PET_WAIT_TEXTURE or "Interface\\Icons\\Spell_Nature_TimeStop",
        tip = "Every bot stays where it is." },
    { key = "attack", label = "Attack my target", bind = "WUBOTS_ATTACK",
        icon = PET_ATTACK_TEXTURE or "Interface\\Icons\\Ability_GhoulFrenzy", tip = "Every bot attacks your target." },
    { key = "passive", label = "Passive", bind = "WUBOTS_PASSIVE", icon = PET_PASSIVE_TEXTURE or "Interface\\Icons\\Ability_Seal",
        tip = "Bots stop attacking (press again: they fight)." },
    { key = "pull", label = "Pull", bind = "WUBOTS_PULL", icon = "Interface\\Icons\\Ability_Warrior_Charge",
        tip = "Your tank bot opens the fight on your target; the others join in." },
}

local ACTIVITY = { following = { "Following", 0.45, 0.9, 0.45 }, fighting = { "Fighting", 1, 0.5, 0.3 },
    resting = { "Resting", 0.45, 0.7, 1 }, staying = { "Staying", 1, 0.82, 0.2 }, passive = { "Passive", 0.7, 0.62, 0.95 },
    dead = { "Dead", 1, 0.32, 0.32 } }

local function db() return U.db() end
local function lockdown() return InCombatLockdown and InCombatLockdown() end
local function allPassive()
    if #C.bots == 0 then return false end
    for _, b in ipairs(C.bots) do if b.status ~= "passive" then return false end end
    return true
end

-- ---------------------------------------------------------------- orders and keys

-- One key or button press: follow, stay, attack, passive (toggles with fight), pull; "window" opens the window.
function C.Key(what)
    if what == "window" then return C.Toggle() end
    if #C.bots == 0 then return C.Notice("You have no bots with you.") end
    if what == "passive" then what = allPassive() and "active" or "passive" end
    return C.BarOrder(what)
end

-- What a bot is doing, in one word (the roster's status, and the unit functions for what moves live).
function C.Activity(b, unit)
    if not b then return "following" end
    if unit and UnitIsDeadOrGhost and UnitIsDeadOrGhost(unit) then return "dead" end
    if b.status == "dead" then return "dead" end
    if unit and UnitAffectingCombat and UnitAffectingCombat(unit) then return "fighting" end
    if b.status == "stay" then return "staying" end
    if b.status == "passive" then return "passive" end
    if unit and UnitHealth and UnitHealthMax then
        local hp, hpMax = UnitHealth(unit) or 0, UnitHealthMax(unit) or 0
        local low = hpMax > 0 and hp < hpMax * 0.9
        if not low and UnitPowerType and UnitPower and UnitPowerMax then
            local kind = UnitPowerType(unit)
            local mp, mpMax = UnitPower(unit) or 0, UnitPowerMax(unit) or 0
            low = kind == 0 and mpMax > 0 and mp < mpMax * 0.8
        end
        if low then return "resting" end
    end
    return "following"
end

-- ---------------------------------------------------------------- saving where a bar is

local function place(frame, key, point, rel, x, y)
    local p = db()[key]
    frame:ClearAllPoints()
    if type(p) == "table" and type(p[1]) == "string" and tonumber(p[3]) and tonumber(p[4]) then
        frame:SetPoint(p[1], UIParent, p[2] or p[1], tonumber(p[3]), tonumber(p[4]))
    else
        frame:SetPoint(point, UIParent, rel, x, y)
    end
end

local function remember(frame, key)
    if not frame.GetPoint then return end
    local p, _, rp, x, y = frame:GetPoint()
    if p and x and y then db()[key] = { p, rp or p, math.floor(x + 0.5), math.floor(y + 0.5) } end
end

local function draggable(frame, handle, key)
    handle:EnableMouse(true)
    handle:RegisterForDrag("LeftButton")
    frame:SetMovable(true)
    frame:SetClampedToScreen(true)
    handle:SetScript("OnDragStart", function()
        if lockdown() and frame.secure then return end   -- secure slots can't move in combat
        frame:StartMoving()
    end)
    handle:SetScript("OnDragStop", function()
        frame:StopMovingOrSizing()
        remember(frame, key)
    end)
end

-- ---------------------------------------------------------------- the bot bar

local bar
local function buildSlot(i)
    local s = CreateFrame("Button", "WrathUnleashedBotBarSlot" .. i, bar, "SecureUnitButtonTemplate")
    s:SetWidth(SLOT_W); s:SetHeight(SLOT_H - 2)
    s:SetPoint("TOPLEFT", 0, -HEAD_H - (i - 1) * SLOT_H)
    s:RegisterForClicks("AnyUp")
    s:SetAttribute("type1", "target")   -- left-click: the client's own target action
    U.fill(s, U.BG[1], U.BG[2], U.BG[3], 0.82)
    s.hover = U.fill(s, U.ACCENT[1], U.ACCENT[2], U.ACCENT[3], 0.12, "ARTWORK"); s.hover:Hide()
    s.edge = s:CreateTexture(nil, "ARTWORK")
    s.edge:SetWidth(2); s.edge:SetPoint("TOPLEFT"); s.edge:SetPoint("BOTTOMLEFT")
    s.portrait = s:CreateTexture(nil, "ARTWORK")
    s.portrait:SetWidth(38); s.portrait:SetHeight(38)
    s.portrait:SetPoint("LEFT", 5, 0)
    s.role = s:CreateTexture(nil, "OVERLAY")
    s.role:SetWidth(16); s.role:SetHeight(16)
    s.role:SetPoint("BOTTOMLEFT", s.portrait, "BOTTOMLEFT", -3, -3)
    s.role:SetTexture(U.ROLE_ICON)
    s.name = U.label(s, "", "GameFontNormalSmall", "TOPLEFT", 48, -5)
    s.name:SetWidth(96)
    s.activity = U.label(s, "", "GameFontHighlightSmall", "TOPRIGHT", -6, -5)
    s.activity:SetJustifyH("RIGHT")
    s.hp = U.bar(s, SLOT_W - 56, 9, "TOPLEFT", 48, -21)
    s.hp:SetStatusBarColor(0.2, 0.8, 0.3)
    s.mp = U.bar(s, SLOT_W - 56, 5, "TOPLEFT", 48, -34)
    s:SetScript("PostClick", function(self, button)
        if button == "RightButton" and self.botName then C.OpenMenu(self.botName, self) end
    end)
    s:SetScript("OnEnter", function(self)
        self.hover:Show()
        if GameTooltip and self.unit then
            GameTooltip:SetOwner(self, "ANCHOR_RIGHT")
            if GameTooltip.SetUnit then GameTooltip:SetUnit(self.unit) else GameTooltip:SetText(self.botName or "") end
            GameTooltip:AddLine("Left-click: target. Right-click: orders, spec, role, gear.", 1, 1, 1)
            GameTooltip:Show()
        end
    end)
    s:SetScript("OnLeave", function(self) self.hover:Hide(); if GameTooltip then GameTooltip:Hide() end end)
    s:Hide()
    return s
end

local function buildBar()
    bar = CreateFrame("Frame", "WrathUnleashedBotBar", UIParent)
    bar:SetWidth(SLOT_W); bar:SetHeight(HEAD_H + SLOT_H)
    bar:SetFrameStrata("MEDIUM")
    bar.secure = true
    local head = CreateFrame("Frame", nil, bar)
    head:SetPoint("TOPLEFT"); head:SetPoint("TOPRIGHT"); head:SetHeight(HEAD_H - 2)
    U.fill(head, U.BG[1], U.BG[2], U.BG[3], 0.9)
    local line = head:CreateTexture(nil, "ARTWORK")
    line:SetTexture(U.ACCENT[1], U.ACCENT[2], U.ACCENT[3], 0.8)
    line:SetHeight(1); line:SetPoint("BOTTOMLEFT"); line:SetPoint("BOTTOMRIGHT")
    bar.title = U.label(head, "Bots", "GameFontNormalSmall", "LEFT", 6, 0)
    bar.title:SetTextColor(U.ACCENT[1], U.ACCENT[2], U.ACCENT[3])
    bar.count = U.label(head, "", "GameFontDisableSmall", "RIGHT", -22, 0)
    bar.count:SetJustifyH("RIGHT")
    local open = U.button(head, "+", 16, function() C.Toggle() end, "RIGHT", -2, 0)
    open:SetHeight(14)
    bar.open = open
    head:SetScript("OnEnter", function(self)
        if not GameTooltip then return end
        GameTooltip:SetOwner(self, "ANCHOR_TOP")
        GameTooltip:SetText("Your bots")
        GameTooltip:AddLine("Drag to move. + opens the Bots window.", 1, 1, 1)
        GameTooltip:Show()
    end)
    head:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
    bar.head = head
    draggable(bar, head, "botBarPos")
    bar.slots = {}
    for i = 1, MAX_SLOTS do bar.slots[i] = buildSlot(i) end
    place(bar, "botBarPos", "LEFT", "LEFT", 24, -60)
    bar:SetScript("OnUpdate", function(self, elapsed)
        self.tick = (self.tick or 0) + (elapsed or 0)
        if self.tick < 0.2 then return end
        self.portraitTick = (self.portraitTick or 0) + self.tick
        self.tick = 0
        B.Update(self.portraitTick > 3)
        if self.portraitTick > 3 then self.portraitTick = 0 end
    end)
    bar:Hide()
end

function B.Frame() return bar end

-- Slots and their units (out of combat only: the client won't change secure frames in a fight).
function B.Layout()
    local want = #C.bots > 0 and not db().botBarOff
    if not bar then
        if not want then return end
        buildBar()
    end
    if lockdown() then B.dirty = true; return false end
    B.dirty = nil
    local n = 0
    for _, b in ipairs(C.bots) do
        local unit = C.UnitFor(b.name)
        if unit and n < MAX_SLOTS then
            n = n + 1
            local s = bar.slots[n]
            s.unit, s.botName, s.bot = unit, b.name, b
            s:SetAttribute("unit", unit)
            local r, g, bl = U.classColour(b.cls)
            s.name:SetText(U.hex(r, g, bl) .. b.name .. "|r")
            s.edge:SetTexture(r, g, bl, 0.9)
            local c = U.ROLE_COORDS[b.role] or U.ROLE_COORDS.dps
            s.role:SetTexCoord(c[1], c[2], c[3], c[4])
            if SetPortraitTexture then pcall(SetPortraitTexture, s.portrait, unit) end
            s:Show()
        end
    end
    for i = n + 1, MAX_SLOTS do
        local s = bar.slots[i]
        s.unit, s.botName, s.bot = nil, nil, nil
        s:SetAttribute("unit", nil)
        s:Hide()
    end
    bar:SetHeight(HEAD_H + math.max(1, n) * SLOT_H)
    bar.count:SetText(n > 0 and (n .. (n == 1 and " bot" or " bots")) or "")
    if want and n > 0 then bar:Show() else bar:Hide() end
    B.Update(false)
    return true
end

-- Health, mana and the activity word, live.
function B.Update(portraits)
    if not bar then return end
    for _, s in ipairs(bar.slots) do
        local u = s.unit
        if u and s:IsShown() then
            local hp, hpMax = UnitHealth and UnitHealth(u) or 0, UnitHealthMax and UnitHealthMax(u) or 0
            local mp, mpMax = UnitPower and UnitPower(u) or 0, UnitPowerMax and UnitPowerMax(u) or 0
            s.hp:SetValue(hpMax > 0 and hp / hpMax or 0)
            s.mp:SetValue(mpMax > 0 and mp / mpMax or 0)
            local token
            if UnitPowerType then local _, t = UnitPowerType(u); token = t end
            local pc = PowerBarColor and PowerBarColor[token or "MANA"]
            if pc then s.mp:SetStatusBarColor(pc.r, pc.g, pc.b) else s.mp:SetStatusBarColor(0.25, 0.45, 1) end
            local a = ACTIVITY[C.Activity(s.bot, u)]
            s.activity:SetText(a[1]); s.activity:SetTextColor(a[2], a[3], a[4])
            if s.portrait.SetDesaturated then s.portrait:SetDesaturated(a == ACTIVITY.dead) end
            if portraits and SetPortraitTexture then pcall(SetPortraitTexture, s.portrait, u) end
        end
    end
end

-- ---------------------------------------------------------------- the orders bar

local orders
local function hotkey(bind)
    if not GetBindingKey then return "" end
    local ok, k = pcall(GetBindingKey, bind)
    if not ok or not k then return "" end
    return (tostring(k):gsub("SHIFT%-", "S-"):gsub("CTRL%-", "C-"):gsub("ALT%-", "A-"))
end

local function buildOrders()
    orders = CreateFrame("Frame", "WrathUnleashedBotOrders", UIParent)
    orders:SetWidth(14 + #ORDERS * (ORDER_SIZE + ORDER_GAP)); orders:SetHeight(ORDER_SIZE + 10)
    orders:SetFrameStrata("MEDIUM")
    if W.Style then W.Style.Box(orders, 0.82) else U.fill(orders, U.BG[1], U.BG[2], U.BG[3], 0.82) end
    local grip = CreateFrame("Frame", nil, orders)
    grip:SetWidth(10); grip:SetPoint("TOPLEFT", 2, -2); grip:SetPoint("BOTTOMLEFT", 2, 2)
    local dots = grip:CreateTexture(nil, "ARTWORK")
    dots:SetTexture(U.ACCENT[1], U.ACCENT[2], U.ACCENT[3], 0.5)
    dots:SetWidth(2); dots:SetPoint("TOP", 0, -6); dots:SetPoint("BOTTOM", 0, 6)
    grip:SetScript("OnEnter", function(self)
        if not GameTooltip then return end
        GameTooltip:SetOwner(self, "ANCHOR_TOP")
        GameTooltip:SetText("Bot orders")
        GameTooltip:AddLine("Drag to move. Keys: Esc > Key Bindings > Bots.", 1, 1, 1)
        GameTooltip:Show()
    end)
    grip:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
    draggable(orders, grip, "botOrdersPos")
    orders.btns = {}
    for i, o in ipairs(ORDERS) do
        local b = CreateFrame("Button", "WrathUnleashedBotOrder" .. o.key, orders)
        b:SetWidth(ORDER_SIZE); b:SetHeight(ORDER_SIZE)
        b:SetPoint("LEFT", 14 + (i - 1) * (ORDER_SIZE + ORDER_GAP), 0)
        b.icon = b:CreateTexture(nil, "ARTWORK")
        b.icon:SetAllPoints(b)
        b.icon:SetTexture(o.icon)
        b.icon:SetTexCoord(0.07, 0.93, 0.07, 0.93)
        b.on = b:CreateTexture(nil, "OVERLAY")
        b.on:SetTexture(U.ACCENT[1], U.ACCENT[2], U.ACCENT[3], 0.35)
        b.on:SetAllPoints(b); b.on:Hide()
        if b.SetHighlightTexture then b:SetHighlightTexture("Interface\\Buttons\\ButtonHilight-Square", "ADD") end
        b.hotkey = U.label(b, "", "NumberFontNormalSmallGray", "TOPRIGHT", -1, -2)
        b.hotkey:SetJustifyH("RIGHT")
        b.order, b.bind = o.key, o.bind
        b:SetScript("OnClick", function() C.Key(o.key) end)
        b:SetScript("OnEnter", function(self)
            if not GameTooltip then return end
            GameTooltip:SetOwner(self, "ANCHOR_TOP")
            local k = hotkey(o.bind)
            GameTooltip:SetText(self.label or o.label)
            GameTooltip:AddLine(o.tip, 1, 1, 1)
            GameTooltip:AddLine(k ~= "" and ("Key: " .. k) or "No key yet: Esc > Key Bindings > Bots.", 0.6, 0.62, 0.66)
            GameTooltip:Show()
        end)
        b:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
        orders.btns[i] = b
        orders.btns[o.key] = b
    end
    place(orders, "botOrdersPos", "BOTTOM", "BOTTOM", 0, 172)
    orders:Hide()
end

function B.Orders() return orders end

function B.RefreshOrders()
    local want = #C.bots > 0 and not db().botOrdersOff
    if not orders then
        if not want then return end
        buildOrders()
    end
    local passive = allPassive()
    for _, b in ipairs(orders.btns) do
        b.hotkey:SetText(hotkey(b.bind))
        if b.order == "passive" then
            b.label = passive and "Fight" or "Passive"
            if passive then b.on:Show() else b.on:Hide() end
        end
    end
    if want then orders:Show() else orders:Hide() end
end

-- ---------------------------------------------------------------- the first-time tips

local TIPS = {
    { "Your bots", "This bar shows your bots. Left-click one to target it; right-click it for orders, its spec, its role "
        .. "and new gear.", "bar" },
    { "Orders for everyone", "Follow, Stay, Attack, Passive and Pull for all your bots at once. Give them keys in Esc > Key "
        .. "Bindings > Bots.", "orders" },
    { "More bots", "Open the Bots window (/comp, or the minimap button) to fill a party or a raid, bring your alts or a "
        .. "levelling buddy, or pick a dungeon.", nil },
}

local tips
local function buildTips()
    tips = U.panel(UIParent, "WrathUnleashedBotTips", 300, 128)
    tips:SetFrameStrata("DIALOG")
    tips:EnableMouse(true)
    tips:SetClampedToScreen(true)
    tips.head = U.label(tips, "", "GameFontDisableSmall", "TOPLEFT", 16, -12)
    tips.title = U.label(tips, "", "GameFontNormal", "TOPLEFT", 16, -28)
    tips.title:SetTextColor(U.ACCENT[1], U.ACCENT[2], U.ACCENT[3])
    tips.text = U.label(tips, "", "GameFontHighlightSmall", "TOPLEFT", 16, -48)
    tips.text:SetWidth(268)
    tips.next = U.button(tips, "Next", 80, function() B.Tip((tips.step or 1) + 1) end, "BOTTOMRIGHT", -12, 10)
    if W.Style then W.Style.SetKind(tips.next, "primary") end
    tips.skip = U.button(tips, "Skip tips", 80, function() tips:Hide() end, "BOTTOMLEFT", 12, 10)
    tinsert(UISpecialFrames, "WrathUnleashedBotTips")
    tips:Hide()
end

function B.Tips() return tips end

function B.Tip(step)
    if step > #TIPS then if tips then tips:Hide() end; return end
    if not tips then buildTips() end
    local t = TIPS[step]
    tips.step = step
    tips.head:SetText(string.format("Tip %d of %d", step, #TIPS))
    tips.title:SetText(t[1])
    tips.text:SetText(t[2])
    tips.next:SetText(step == #TIPS and "Got it" or "Next")
    tips:ClearAllPoints()
    local anchor = (t[3] == "bar" and bar) or (t[3] == "orders" and orders) or nil
    if anchor and anchor:IsShown() then
        if t[3] == "bar" then tips:SetPoint("LEFT", anchor, "RIGHT", 12, 0)
        else tips:SetPoint("BOTTOM", anchor, "TOP", 0, 12) end
    else
        tips:SetPoint("CENTER", UIParent, "CENTER", 0, 120)
    end
    tips:Show()
end

-- The first time bots are with you (any character on this account): the tips, once.
function B.MaybeTips()
    if db().botsTipsSeen or #C.bots == 0 then return false end
    db().botsTipsSeen = true
    B.Tip(1)
    return true
end

-- ---------------------------------------------------------------- wiring

function B.Refresh()
    B.Layout()
    B.RefreshOrders()
    B.MaybeTips()
end
C.OnRoster(B.Refresh)

function C.Slash(word)
    if word == "bar" then
        db().botBarOff = (not db().botBarOff) or nil
        B.Layout()
        C.Notice(db().botBarOff and "The bar of your bots is off (/comp bar)." or "The bar of your bots is on.")
        return true
    elseif word == "orders" then
        db().botOrdersOff = (not db().botOrdersOff) or nil
        B.RefreshOrders()
        C.Notice(db().botOrdersOff and "The orders bar is off (/comp orders)." or "The orders bar is on.")
        return true
    elseif word == "tips" then
        B.Tip(1)
        return true
    end
    return false
end

local ev = CreateFrame("Frame")
ev:RegisterEvent("PLAYER_REGEN_ENABLED")
ev:RegisterEvent("PARTY_MEMBERS_CHANGED")
ev:RegisterEvent("RAID_ROSTER_UPDATE")
ev:RegisterEvent("UNIT_PORTRAIT_UPDATE")
ev:RegisterEvent("UPDATE_BINDINGS")
ev:SetScript("OnEvent", function(_, event, unit)
    if event == "PLAYER_REGEN_ENABLED" then
        if B.dirty then B.Layout() end
    elseif event == "UNIT_PORTRAIT_UPDATE" then
        if bar and SetPortraitTexture then
            for _, s in ipairs(bar.slots) do if s.unit and s.unit == unit then pcall(SetPortraitTexture, s.portrait, unit) end end
        end
    elseif event == "UPDATE_BINDINGS" then
        if orders then B.RefreshOrders() end
    else
        B.Layout()   -- party / raid unit numbers moved
    end
end)
B.events = ev
