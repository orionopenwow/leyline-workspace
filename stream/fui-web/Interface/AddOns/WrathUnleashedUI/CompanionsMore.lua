-- More for the player who plays with a few bots (3.3.5a API), after Companions.lua and CompanionsTabs.lua:
--   Tactics tab  how they fight and heal, as the game's options panels: Tank pulls first, Focus the tank's target,
--                Spread damage, Crowd control, Passive; Heal the tank first, Save mana; the formation (how close they
--                follow, each one explained); loot (bots always pass). Each switch is one realm order for every bot
--                (ov_companions.lua applyTactics), kept by the realm and given to bots that join later.
--   Gear window  one bot's gear (/comp gear, the bot's menu "Gear and upgrades"): every slot with the item's icon,
--                name in its quality colour and item level (the game's own item tooltip on hover), then the better
--                items in its bags with Equip, and Equip all upgrades, Gear up (new gear; not for your alts), Trade
--                (give it something from your bags, then Equip it here) and Inspect (the game's own inspect window).
--   Ready?       a checklist before a dungeon (/comp ready, the Ready? button, right-click the minimap button): a
--                tank, a healer and damage (with your own role from the Dungeon Finder's role check), everyone alive,
--                everyone close by, bots at your level, bots prepared, healers with mana, your gear repaired, food
--                and drink in your bags. Fill missing roles, Prepare bots, Regroup and the game's own Ready check.
-- The look is the game's own (GameLook.lua): dialogs, panel buttons, option check boxes, inset group boxes, the
-- action button's slot ring round each item, the raid frames' ready-check marks. Paths only; no art is copied.

local W = WrathUnleashedUI
local C = W.Companions
if not C or not C.UI then return end
local U = C.UI
local G = U.look
local label, button, section, panel, tip = U.label, U.button, U.section, U.panel, U.tip

local READY_ICON = { [true] = "Interface\\RaidFrame\\ReadyCheck-Ready", [false] = "Interface\\RaidFrame\\ReadyCheck-NotReady",
    wait = "Interface\\RaidFrame\\ReadyCheck-Waiting" }

local function para(parent, text, x, y, w)
    local fs = label(parent, text, "GameFontHighlightSmall", "TOPLEFT", x, y)
    fs:SetWidth(w or (U.WIN_W - 44))
    return fs
end

-- ---------------------------------------------------------------- Tactics

-- key: the realm's order (its "no" form turns it off, Companions.lua C.Tactic); state: where the roster keeps it
C.TACTICS = {
    { head = "How they fight" },
    { key = "waitpull", title = "Tank pulls first", state = "waitPull",
        text = "Damage bots let the tank hold the first enemy, and watch their threat." },
    { key = "assist", title = "Focus the tank's target", state = "assist",
        text = "The tank bot marks its target with a skull; damage bots kill that one first." },
    { key = "aoe", title = "Spread damage", state = "aoe",
        text = "Damage bots hit many enemies at once. Off: one enemy at a time." },
    { key = "cc", title = "Crowd control", state = "cc",
        text = "Bots that can keep extra enemies out of the fight (sleep, trap, sap, banish)." },
    { key = "passive", title = "Passive", state = "allPassive",
        text = "Bots don't attack; they follow and heal. Untick to let them fight." },
    { head = "How they heal" },
    { key = "healtank", title = "Heal the tank first", state = "healTank",
        text = "Healers keep the tank alive first: your tank bot, or you when you tank." },
    { key = "savemana", title = "Save mana", state = "saveMana",
        text = "Healers hold back between fights so they have mana for the next pull." },
}

-- the formations and what each means for the player (how close, which shape)
C.FORMATION_HELP = {
    { "near", "Close", "Right behind you, a few steps away." },
    { "chaos", "Loose", "Spread out round you, each where it likes." },
    { "melee", "Melee", "Close in, ready to fight at arm's length." },
    { "line", "Line", "Side by side in a line behind you." },
    { "circle", "Circle", "A ring round you." },
    { "arrow", "Arrow", "Tanks ahead, healers and ranged behind." },
    { "shield", "Shield", "A wall in front of you." },
    { "queue", "Single file", "One behind the other: narrow paths and bridges." },
}

function C.TacticOn(t)
    if t.state == "allPassive" then
        if #C.bots == 0 then return false end
        for _, b in ipairs(C.bots) do if b.status ~= "passive" then return false end end
        return true
    end
    return C[t.state] and true or false
end

local TOGGLE_H = 38
C.AddTab("tactics", "Tactics", 70, function(page)
    page.toggles = {}
    local y = -38
    for _, t in ipairs(C.TACTICS) do
        if t.head then
            section(page, t.head, y)
            y = y - 18
        else
            local box = CreateFrame("Frame", nil, page)
            box:SetWidth(U.WIN_W - 40); box:SetHeight(TOGGLE_H - 3)
            box:SetPoint("TOPLEFT", 20, y)
            G.Inset(box)   -- the options panels' group box; its edge turns gold while the tactic is on
            local cb = CreateFrame("CheckButton", nil, box, "UICheckButtonTemplate")
            cb:SetWidth(24); cb:SetHeight(24)
            cb:SetPoint("LEFT", 6, 0)
            cb:SetScript("OnClick", function(self)
                local on = self:GetChecked() and true or false
                C.Tactic(t.key, on)
            end)
            tip(cb, t.title, t.text, "For every bot with you, and bots that join later.")
            box.check = cb
            box.title = label(box, t.title, "GameFontNormal", "TOPLEFT", 36, -5)
            box.text = label(box, t.text, "GameFontHighlightSmall", "TOPLEFT", 36, -20)
            box.text:SetWidth(U.WIN_W - 90)
            box.tactic = t
            page.toggles[#page.toggles + 1] = box
            page.toggles[t.key] = box
            y = y - TOGGLE_H
        end
    end
    y = y - 4
    section(page, "Formation: how they follow you", y)
    page.formations = {}
    for i, f in ipairs(C.FORMATION_HELP) do
        local col, row = (i - 1) % 4, math.floor((i - 1) / 4)
        local b = button(page, f[2], 118, function() C.Formation(f[1]) end, "TOPLEFT", 20 + col * 124, y - 20 - row * 24)
        tip(b, f[2], f[3])
        b.key = f[1]
        page.formations[i] = b
    end
    y = y - 76
    section(page, "Loot", y)
    para(page, "Bots always pass: every roll is yours. Grey and white items still go round the group; set loot to Free for All "
        .. "to keep them all.", 22, y - 18)
    page.none = para(page, "", 22, y - 48)
end, function(page)
    local has = #C.bots > 0
    for _, box in ipairs(page.toggles) do
        local on = C.TacticOn(box.tactic)
        box.check:SetChecked(on)
        G.InsetOn(box, on)
        if has then box.check:Enable() else box.check:Disable() end
    end
    for _, b in ipairs(page.formations) do
        G.Select(b, b.key == C.formation)
        if has then b:Enable() else b:Disable() end
    end
    page.none:SetText(has and "" or "Bring bots first: the tactics are for the bots with you, and the ones who join later.")
end)

-- ---------------------------------------------------------------- the gear window

-- equipment slots 0-17 as the character window lists them, with the empty slot's own art (by path)
local SLOT_INFO = {
    { 0, "Head", "Head" }, { 1, "Neck", "Neck" }, { 2, "Shoulder", "Shoulder" }, { 14, "Back", "Chest" }, { 4, "Chest", "Chest" },
    { 8, "Wrist", "Wrists" }, { 9, "Hands", "Hands" }, { 5, "Waist", "Waist" }, { 6, "Legs", "Legs" }, { 7, "Feet", "Feet" },
    { 10, "Finger", "Finger" }, { 11, "Finger", "Finger" }, { 12, "Trinket", "Trinket" }, { 13, "Trinket", "Trinket" },
    { 15, "Main hand", "MainHand" }, { 16, "Off hand", "SecondaryHand" }, { 17, "Ranged", "Ranged" },
}
C.GEAR_SLOTS = SLOT_INFO
local SLOT_NAME = {}
for _, s in ipairs(SLOT_INFO) do SLOT_NAME[s[1]] = s[2] end
local GEAR_W, GEAR_H, UP_ROWS = 392, 590, 4

local function qualityHex(q)
    local c = rawget(_G, "ITEM_QUALITY_COLORS") and ITEM_QUALITY_COLORS[q]
    if c and c.hex then return c.hex end
    local fall = { [0] = "|cff9d9d9d", [1] = "|cffffffff", [2] = "|cff1eff00", [3] = "|cff0070dd", [4] = "|cffa335ee", [5] = "|cffff8000" }
    return fall[q] or "|cffffffff"
end
C.QualityHex = qualityHex

local function itemIcon(entry)
    if GetItemIcon then
        local ok, t = pcall(GetItemIcon, entry)
        if ok and t then return t end
    end
    if GetItemInfo then
        local ok, _, _, _, _, _, _, _, _, _, t = pcall(GetItemInfo, entry)
        if ok and t then return t end
    end
    return "Interface\\Icons\\INV_Misc_QuestionMark"
end

local function itemTip(owner, entry)
    if not (GameTooltip and entry) then return end
    GameTooltip:SetOwner(owner, "ANCHOR_RIGHT")
    if GameTooltip.SetHyperlink and pcall(GameTooltip.SetHyperlink, GameTooltip, "item:" .. entry .. ":0:0:0:0:0:0:0") then
        GameTooltip:Show()
    end
end

local gear
local function slotButton(parent, size, ...)
    local b = CreateFrame("Button", nil, parent)
    b:SetWidth(size); b:SetHeight(size)
    b:SetPoint(...)
    b.icon = b:CreateTexture(nil, "BACKGROUND")
    b.icon:SetAllPoints(b)
    b.icon:SetTexCoord(0.07, 0.93, 0.07, 0.93)
    G.ActionButton(b, size)   -- the action button's slot ring and highlight
    b:SetScript("OnEnter", function(self) if self.entry then itemTip(self, self.entry) end end)
    b:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
    return b
end

local function buildGear()
    gear = panel(UIParent, "WrathUnleashedBotGear", GEAR_W, GEAR_H)
    gear:SetPoint("CENTER", 300, 0)
    gear:SetFrameStrata("DIALOG")
    gear:EnableMouse(true)
    gear:SetMovable(true)
    gear:RegisterForDrag("LeftButton")
    gear:SetScript("OnDragStart", gear.StartMoving)
    gear:SetScript("OnDragStop", gear.StopMovingOrSizing)
    gear:SetClampedToScreen(true)
    tinsert(UISpecialFrames, "WrathUnleashedBotGear")
    gear.title = G.Window(gear, "Gear", "Interface\\Icons\\INV_Chest_Chain")
    G.Close(gear)
    gear.sub = label(gear, "", "GameFontHighlightSmall", "TOPLEFT", 20, -40)
    gear.sub:SetWidth(GEAR_W - 40)
    gear.slots = {}
    for i, s in ipairs(SLOT_INFO) do
        local col, row = (i - 1) % 2, math.floor((i - 1) / 2)
        local x, y = 20 + col * 178, -58 - row * 32
        local b = slotButton(gear, 28, "TOPLEFT", x, y)
        b.slot, b.empty = s[1], "Interface\\PaperDoll\\UI-PaperDoll-Slot-" .. s[3]
        b.name = label(gear, "", "GameFontHighlightSmall", "TOPLEFT", x + 34, y - 1)
        b.name:SetWidth(138)
        b.lvl = label(gear, "", "GameFontDisableSmall", "TOPLEFT", x + 34, y - 15)
        gear.slots[i] = b
    end
    local y = -58 - 9 * 32 - 4
    section(gear, "Better items in its bags", y)
    gear.ups = {}
    for i = 1, UP_ROWS do
        local ry = y - 20 - (i - 1) * 30
        local b = slotButton(gear, 24, "TOPLEFT", 20, ry)
        b.name = label(gear, "", "GameFontHighlightSmall", "TOPLEFT", 52, ry - 1)
        b.name:SetWidth(230)
        b.lvl = label(gear, "", "GameFontDisableSmall", "TOPLEFT", 52, ry - 13)
        b.lvl:SetWidth(230)
        b.equip = button(gear, "Equip", 70, function(self)
            if gear.botName and self.entry then C.EquipItem(gear.botName, self.entry) end
        end, "TOPRIGHT", -20, ry)
        tip(b.equip, "Equip", "Your bot puts this on now, from its own bags. Out of combat only.")
        gear.ups[i] = b
    end
    gear.noUps = para(gear, "", 22, y - 24, GEAR_W - 44)
    y = y - 20 - UP_ROWS * 30 - 6
    gear.all = button(gear, "Equip all upgrades", 140, function() if gear.botName then C.Upgrade(gear.botName) end end,
        "TOPLEFT", 20, y)
    tip(gear.all, "Equip all upgrades", "Your bot puts on every better item it carries.")
    gear.gearUp = button(gear, "Gear up", 90, function() if gear.botName then C.Gear(gear.botName) end end, "TOPLEFT", 164, y)
    tip(gear.gearUp, "Gear up", "Your bot gets new gear for its level and spec. Not for your own characters: they keep theirs.")
    gear.refresh = button(gear, "Refresh", 90, function() if gear.botName then C.Inspect(gear.botName) end end,
        "TOPLEFT", 258, y)
    gear.trade = button(gear, "Trade", 90, function()
        local u = gear.botName and C.UnitFor(gear.botName)
        if u and InitiateTrade then pcall(InitiateTrade, u) end
    end, "TOPLEFT", 20, y - 26)
    tip(gear.trade, "Trade", "Give your bot an item from your bags. Then press Refresh: it shows above, ready to Equip.",
        "Stand next to it.")
    gear.inspect = button(gear, "Inspect", 90, function()
        local u = gear.botName and C.UnitFor(gear.botName)
        if u and InspectUnit then pcall(InspectUnit, u) end
    end, "TOPLEFT", 114, y - 26)
    tip(gear.inspect, "Inspect", "The game's own inspect window: its gear, talents and more.", "Stand close to it.")
    gear.status = label(gear, "", "GameFontHighlightSmall", "BOTTOMLEFT", 22, 16)
    gear.status:SetWidth(GEAR_W - 44)
    gear:SetScript("OnShow", function() if PlaySound then pcall(PlaySound, "igCharacterInfoOpen") end end)
    gear:Hide()
end

function C.GearWindow() return gear end

function C.RefreshGear()
    if not gear then return end
    local g = C.gear
    local mine = g and g.name == gear.botName
    local b = C.Find(gear.botName or "")
    local r, gg, bl = U.classColour((g and g.cls) or (b and b.cls))
    gear.title:SetText((gear.botName or "") .. "'s gear")
    local tag = "  |cff9d9d9d" .. (U.KIND_TAG[(mine and g.kind) or (b and b.kind) or "bot"]) .. "|r"
    if mine then
        gear.sub:SetText(U.hex(r, gg, bl) .. string.format("Level %d %s|r, item level %d", g.level, U.CLASS_NAME[g.cls] or "", g.ilvl) .. tag)
    else
        gear.sub:SetText("Asking the realm...")
    end
    for _, s in ipairs(gear.slots) do
        local it = mine and g.worn[s.slot] or nil
        s.entry = it and it.entry or nil
        if it then
            s.icon:SetTexture(itemIcon(it.entry))
            s.name:SetText(qualityHex(it.quality) .. it.name .. "|r")
            s.lvl:SetText(SLOT_NAME[s.slot] .. ", item level " .. it.ilvl)
        else
            s.icon:SetTexture(s.empty)
            s.name:SetText("|cff9d9d9d" .. SLOT_NAME[s.slot] .. "|r")
            s.lvl:SetText(mine and "empty" or "")
        end
    end
    local ups = mine and g.ups or {}
    for i, row in ipairs(gear.ups) do
        local u = ups[i]
        row.entry, row.equip.entry = u and u.entry or nil, u and u.entry or nil
        if u then
            row.icon:SetTexture(itemIcon(u.entry))
            row.name:SetText(qualityHex(u.quality) .. u.name .. "|r")
            row.lvl:SetText(string.format("%s: item level %d, now %d (+%d)", SLOT_NAME[u.slot] or "", u.ilvl, u.now, u.ilvl - u.now))
            row:Show(); row.equip:Show(); row.name:Show(); row.lvl:Show()
        else
            row:Hide(); row.equip:Hide(); row.name:Hide(); row.lvl:Hide()
        end
    end
    gear.noUps:SetText(mine and #ups == 0 and "Nothing better in its bags. Trade it something, then Refresh." or "")
    if mine and #ups > 0 then gear.all:Enable() else gear.all:Disable() end
    local own = not ((mine and g.kind or (b and b.kind)) == "alt")
    if own then gear.gearUp:Enable() else gear.gearUp:Disable() end
    gear.status:SetText(C.status or "")
end
C.OnGear(function(g) if gear and gear:IsShown() and g.name == gear.botName then C.RefreshGear() end end)
C.OnRoster(function() if gear and gear:IsShown() then C.RefreshGear() end end)

-- Open a bot's gear: by name, else your target when it is one of your bots, else your first bot.
function C.OpenGear(name)
    name = name and U.tidy(name) or nil
    if name and name ~= "" and not C.Find(name) then
        for _, b in ipairs(C.bots) do if b.name:lower() == name:lower() then name = b.name end end
    end
    if not (name and C.Find(name)) then
        local t = UnitName and UnitName("target")
        name = (t and C.Find(t)) and t or (C.bots[1] and C.bots[1].name)
    end
    if not name then return C.Notice("You have no bots with you.") end
    if not gear then buildGear() end
    gear.botName = name
    if not (C.gear and C.gear.name == name) then C.gear = nil end
    C.Inspect(name)
    gear:Show()
    C.RefreshGear()
    return gear
end

-- ---------------------------------------------------------------- Ready?

local function partyUnitOf(b) return C.UnitFor(b.name) end

-- Your own role: the Dungeon Finder's role check (GetLFGRoles: leader, tank, healer, damage), else damage.
function C.MyRole()
    if GetLFGRoles then
        local ok, _, tank, heal, dps = pcall(GetLFGRoles)
        if ok then
            if tank then return "tank" end
            if heal then return "healer" end
            if dps then return "dps" end
        end
    end
    return "dps"
end

local function names(list)
    if #list <= 2 then return table.concat(list, " and ") end
    return table.concat(list, ", ", 1, #list - 1) .. " and " .. list[#list]
end

-- Your bags: food and drink (the item's type, as the game names it), or nil when the client can't tell.
local function foodInBags()
    if not (GetContainerNumSlots and GetContainerItemLink and GetItemInfo) then return nil end
    local n, seen = 0, false
    for bag = 0, 4 do
        for slot = 1, (GetContainerNumSlots(bag) or 0) do
            local link = GetContainerItemLink(bag, slot)
            if link then
                local _, _, _, _, _, kind, sub = GetItemInfo(link)
                if kind then seen = true end
                if sub == "Food & Drink" then n = n + 1 end
            end
        end
    end
    if n > 0 then return true end
    return seen and false or nil
end

-- Your gear's lowest durability, 0..1, or nil.
local function lowestDurability()
    if not GetInventoryItemDurability then return nil end
    local low
    for slot = 1, 18 do
        local cur, max = GetInventoryItemDurability(slot)
        if cur and max and max > 0 then
            local p = cur / max
            if not low or p < low then low = p end
        end
    end
    return low
end

-- The checklist: { key, ok (true, false, or nil for "can't tell"), text, detail }. Pure, for the window and the tests.
function C.ReadyChecks()
    local out = {}
    local function add(key, ok, text, detail) out[#out + 1] = { key = key, ok = ok, text = text, detail = detail or "" } end
    local bots = C.bots
    local me = C.MyRole()
    local have = { tank = 0, healer = 0, dps = 0 }
    have[me] = have[me] + 1
    for _, b in ipairs(bots) do have[b.role] = (have[b.role] or 0) + 1 end
    local missing = {}
    if have.tank == 0 then missing[#missing + 1] = "a tank" end
    if have.healer == 0 then missing[#missing + 1] = "a healer" end
    if have.dps == 0 then missing[#missing + 1] = "damage" end
    add("roles", #missing == 0, "A tank, a healer and damage",
        #missing > 0 and ("Missing: " .. names(missing) .. ". Fill missing roles brings them.")
        or string.format("%d tank, %d healer, %d damage (you: %s).", have.tank, have.healer, have.dps, U.ROLE_LABEL[me]:lower()))
    if #bots == 0 then
        add("bots", false, "Bots with you", "None yet: Fill missing roles brings a party.")
    else
        local dead, far, low, behind, thirsty = {}, {}, {}, {}, {}
        local myLevel = (UnitLevel and UnitLevel("player")) or 0
        local farKnown = false
        for _, b in ipairs(bots) do
            local u = partyUnitOf(b)
            local mood = C.Mood(b, u)
            if (u and UnitIsDeadOrGhost and UnitIsDeadOrGhost(u)) or b.status == "dead" then dead[#dead + 1] = b.name end
            if UnitInRange and u then farKnown = true end
            if mood == "far" then far[#far + 1] = b.name end
            if b.level > 0 and myLevel > 0 and b.level < myLevel - 2 then behind[#behind + 1] = b.name end
            if b.role == "healer" and u and UnitPower and UnitPowerMax and UnitPowerType and UnitPowerType(u) == 0 then
                local mp, mpMax = UnitPower(u) or 0, UnitPowerMax(u) or 0
                if mpMax > 0 and mp < mpMax * 0.6 then thirsty[#thirsty + 1] = b.name end
            end
            if mood == "hurt" then low[#low + 1] = b.name end
        end
        add("alive", #dead == 0, "Everyone is alive", #dead > 0 and ("Dead: " .. names(dead) .. ".") or "")
        local closeOk = nil   -- nil: the client can't tell
        if #far > 0 then closeOk = false elseif farKnown then closeOk = true end
        add("close", closeOk, "Everyone is close by",
            #far > 0 and ("Far away: " .. names(far) .. ". Press Regroup.") or "")
        add("level", #behind == 0, "Bots at your level", #behind > 0 and ("Behind: " .. names(behind) .. ". Prepare bots catches them up.") or "")
        local fresh = C.preparedAt and (U.now() - C.preparedAt) < 900
        add("prepared", fresh and true or false, "Bots prepared (spells, gear, buffs)",
            fresh and "" or "Press Prepare bots (out of combat).")
        add("mana", #thirsty == 0 and #low == 0, "Healers have mana, nobody is hurt",
            (#thirsty > 0 and ("Low on mana: " .. names(thirsty) .. ". Press Drink. ") or "")
            .. (#low > 0 and ("Hurt: " .. names(low) .. ".") or ""))
    end
    local dur = lowestDurability()
    local repaired = nil
    if dur then repaired = dur >= 0.25 end
    add("repair", repaired, "Your gear is repaired",
        dur and dur < 0.25 and string.format("Your most worn item is at %d%%: repair at a vendor.", math.floor(dur * 100)) or "")
    local food = foodInBags()
    add("food", food, "Food and drink in your bags", food == false and "None in your bags: buy some from an innkeeper." or "")
    return out
end

local ready
local READY_W, READY_H, READY_ROWS = 380, 420, 9
local function buildReady()
    ready = panel(UIParent, "WrathUnleashedBotReady", READY_W, READY_H)
    ready:SetPoint("CENTER", 0, 60)
    ready:SetFrameStrata("DIALOG")
    ready:EnableMouse(true)
    ready:SetMovable(true)
    ready:RegisterForDrag("LeftButton")
    ready:SetScript("OnDragStart", ready.StartMoving)
    ready:SetScript("OnDragStop", ready.StopMovingOrSizing)
    ready:SetClampedToScreen(true)
    tinsert(UISpecialFrames, "WrathUnleashedBotReady")
    G.Window(ready, "Ready?", "Interface\\RaidFrame\\ReadyCheck-Ready")
    G.Close(ready)
    ready.rows = {}
    for i = 1, READY_ROWS do
        local y = -40 - (i - 1) * 32
        local mark = ready:CreateTexture(nil, "ARTWORK")
        mark:SetWidth(18); mark:SetHeight(18)
        mark:SetPoint("TOPLEFT", 20, y)
        local text = label(ready, "", "GameFontNormal", "TOPLEFT", 46, y)
        local detail = label(ready, "", "GameFontHighlightSmall", "TOPLEFT", 46, y - 14)
        detail:SetWidth(READY_W - 66)
        ready.rows[i] = { mark = mark, text = text, detail = detail }
    end
    local y = -40 - READY_ROWS * 32 - 4
    ready.fill = button(ready, "Fill missing roles", 140, function() C.Fill("party", C.Picks and C.Picks()) end, "TOPLEFT", 20, y)
    tip(ready.fill, "Fill missing roles", "Bots fill your party to five with the roles it lacks (your Party tab picks).")
    ready.prepare = button(ready, "Prepare bots", 110, function() C.Prepare() end, "TOPLEFT", 164, y)
    tip(ready.prepare, "Prepare bots", "Spells, better gear, food and buffs for every bot. Out of combat only.")
    ready.regroup = button(ready, "Regroup", 86, function() C.Order("regroup", "all") end, "TOPLEFT", 278, y)
    tip(ready.regroup, "Regroup", "Every bot comes back to you; one far away is brought over.")
    ready.drink = button(ready, "Drink", 86, function() C.Order("drink", "all") end, "TOPLEFT", 20, y - 26)
    tip(ready.drink, "Drink", "Bots with mana drink now.")
    ready.check = button(ready, "Ready check", 110, function()
        if DoReadyCheck then pcall(DoReadyCheck) end
    end, "TOPLEFT", 110, y - 26)
    tip(ready.check, "Ready check", "The game's own ready check for your group: your bots answer too.", "Group leader only.")
    ready.status = label(ready, "", "GameFontHighlightSmall", "BOTTOMLEFT", 22, 16)
    ready.status:SetWidth(READY_W - 44)
    ready:SetScript("OnUpdate", function(self, elapsed)
        self.tick = (self.tick or 0) + (elapsed or 0)
        if self.tick < 1 then return end
        self.tick = 0
        C.RefreshReady()
    end)
    ready:Hide()
end

function C.ReadyWindow() return ready end

function C.RefreshReady()
    if not ready then return end
    local checks = C.ReadyChecks()
    for i, row in ipairs(ready.rows) do
        local c = checks[i]
        if c then
            row.mark:SetTexture(READY_ICON[c.ok == nil and "wait" or c.ok])
            row.text:SetText(c.text)
            row.detail:SetText(c.detail)
            row.mark:Show(); row.text:Show(); row.detail:Show()
        else
            row.mark:Hide(); row.text:Hide(); row.detail:Hide()
        end
    end
    local has = #C.bots > 0
    for _, b in ipairs({ ready.prepare, ready.regroup, ready.drink }) do if has then b:Enable() else b:Disable() end end
    local leader = (IsPartyLeader and IsPartyLeader()) or (IsRaidLeader and IsRaidLeader())
    local grouped = (GetNumPartyMembers and (GetNumPartyMembers() or 0) > 0) or (GetNumRaidMembers and (GetNumRaidMembers() or 0) > 0)
    if leader and grouped then ready.check:Enable() else ready.check:Disable() end
    ready.status:SetText(C.status or "")
    return checks
end
C.OnRoster(function() if ready and ready:IsShown() then C.RefreshReady() end end)

function C.OpenReady()
    if not ready then buildReady() end
    if ready:IsShown() then ready:Hide(); return ready end
    C.Ask()
    ready:Show()
    C.RefreshReady()
    return ready
end

-- The realm's answers land in whichever of these is open when the Bots window isn't (Companions.lua C.Dispatch).
function C.SubShown()
    return (gear and gear:IsShown()) or (ready and ready:IsShown()) or false
end
function C.SubStatus()
    if gear and gear:IsShown() then gear.status:SetText(C.status or "") end
    if ready and ready:IsShown() then ready.status:SetText(C.status or "") end
end
