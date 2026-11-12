-- The Bots window's other tabs (3.3.5a API), after Companions.lua's "Your bots":
--   Party     Fill my party: 1 tank, 1 healer and damage, with a class and a spec per slot (or Any). The picks are
--             kept per character and also used by "Fill party" on Your bots and by the Dungeons tab.
--   Raid      fill a 10-player raid (2 tanks, 3 healers, 5 damage), the raid's shape as role icons, prepare, dismiss.
--   Your alts your own characters as bots (their own gear, spec and level); save who is with you as your usual
--             party, bring it back.
--   Buddy     one levelling buddy (a class or Any), on and off.
--   Dungeons  the dungeons for your level (or all of them), each with the game's own Dungeon Finder art by path:
--             Go fills the party for it and meets everyone at the door (ov_companions.lua "dungeon <id>").
-- The realm decides everything (limits, cooldowns, refusals) and its answer is the window's status line.

local W = WrathUnleashedUI
local C = W.Companions
if not C or not C.UI then return end
local U = C.UI
local label, button, section, fill, panel = U.label, U.button, U.section, U.fill, U.panel
local G = U.look   -- the game's own look (GameLook.lua)

-- class words the realm knows (ov_companions.lua CLASS) and the classes each role can be (its ROLES picks)
local WORD_ID = { warrior = 1, paladin = 2, hunter = 3, rogue = 4, priest = 5, dk = 6, shaman = 7, mage = 8, warlock = 9,
    druid = 11 }
local ROLE_CLASSES = {
    tank = { "warrior", "paladin", "druid", "dk" },
    healer = { "priest", "shaman", "druid", "paladin" },
    dps = { "mage", "hunter", "rogue", "warlock", "shaman", "priest", "warrior", "dk" },
}
local BUDDY_CLASSES = { "warrior", "paladin", "hunter", "rogue", "priest", "shaman", "mage", "warlock", "druid", "dk" }
-- the realm's specs when its roster hasn't listed a class yet (ov_companions.lua FALLBACK_SPECS); it checks anyway
local FALLBACK_SPECS = {
    [1] = { "arms pve", "fury pve", "prot pve" }, [2] = { "holy pve", "prot pve", "ret pve" },
    [3] = { "bm pve", "mm pve", "surv pve" }, [4] = { "as pve", "combat pve", "subtlety pve" },
    [5] = { "disc pve", "holy pve", "shadow pve" }, [6] = { "blood pve", "frost pve", "unholy pve" },
    [7] = { "ele pve", "enh pve", "resto pve" }, [8] = { "arcane pve", "fire pve", "frost pve", "frostfire pve" },
    [9] = { "affli pve", "demo pve", "destro pve" }, [11] = { "balance pve", "bear pve", "resto pve", "cat pve" },
}
local TANK_SPECS = { ["prot pve"] = true, ["bear pve"] = true, ["blood pve"] = true }
local HEAL_SPECS = { ["holy pve"] = true, ["resto pve"] = true, ["disc pve"] = true }
local function specRole(spec)
    if TANK_SPECS[spec] then return "tank" end
    if HEAL_SPECS[spec] then return "healer" end
    return "dps"
end
local SLOTS = { "tank", "healer", "dps", "dps" }
local SPEC_NAMES = { as = "Assassination", bm = "Beast Mastery", mm = "Marksmanship", surv = "Survival", ele = "Elemental",
    enh = "Enhancement", resto = "Restoration", affli = "Affliction", demo = "Demonology", destro = "Destruction",
    prot = "Protection", ret = "Retribution", disc = "Discipline", bear = "Feral (bear)", cat = "Feral (cat)" }

local function className(word) return U.CLASS_NAME[WORD_ID[word] or 0] or "Any" end
local function colourWord(word)
    if not word then return "Any" end
    local r, g, b = U.classColour(WORD_ID[word])
    return U.hex(r, g, b) .. className(word) .. "|r"
end
local function specText(spec)
    if not spec then return "Any" end
    local short = U.specLabel(spec)
    local first = short:match("^(%a+)")
    local long = first and SPEC_NAMES[first:lower()]
    return long and (long .. short:sub(#first + 1)) or short
end

-- What a class may learn for a role: the realm's list (the roster's S lines) or the fallback.
function C.SpecsFor(word, role)
    local id = WORD_ID[word or ""]
    if not id then return {} end
    local out, list = {}, C.specs[id]
    if list and #list > 0 then
        for _, s in ipairs(list) do if s.role == role then out[#out + 1] = s.spec end end
    else
        for _, s in ipairs(FALLBACK_SPECS[id] or {}) do if specRole(s) == role then out[#out + 1] = s end end
    end
    return out
end

local function charDB()
    if type(WrathUnleashedUIChar) ~= "table" then WrathUnleashedUIChar = {} end
    return WrathUnleashedUIChar
end

-- ---------------------------------------------------------------- the picks (per character)

function C.GetPicks()
    local db = charDB()
    if type(db.botPicks) ~= "table" then db.botPicks = {} end
    for i = 1, #SLOTS do if type(db.botPicks[i]) ~= "table" then db.botPicks[i] = {} end end
    return db.botPicks
end

-- "tank=warrior:prot pve,healer=any,dps=mage,dps=any" for the realm; "" when every slot is Any.
function C.Picks()
    local parts, any = {}, false
    for i, role in ipairs(SLOTS) do
        local p = C.GetPicks()[i]
        local word = WORD_ID[p.word or ""] and p.word or nil
        if word then any = true end
        local spec = word and p.spec and tostring(p.spec):match("^[%a ]+$") and (":" .. p.spec) or ""
        parts[#parts + 1] = role .. "=" .. (word or "any") .. spec
    end
    return any and table.concat(parts, ",") or ""
end

function C.SetPick(i, word, spec)
    local p = C.GetPicks()[i]
    if not p then return end
    if word ~= p.word then p.spec = nil end
    p.word = word
    if spec ~= nil then p.spec = spec or nil end
end

function C.ResetPicks() charDB().botPicks = nil end

-- ---------------------------------------------------------------- the chooser (a small list under a button)

local chooser
local function buildChooser()
    chooser = panel(UIParent, "WrathUnleashedBotChooser", 176, 60)
    chooser:SetFrameStrata("FULLSCREEN_DIALOG")
    chooser:EnableMouse(true)
    chooser:SetClampedToScreen(true)
    chooser.title = label(chooser, "", "GameFontNormalSmall", "TOPLEFT", 16, -12)
    chooser.btns = {}
    tinsert(UISpecialFrames, "WrathUnleashedBotChooser")
    chooser:Hide()
end

-- options: { { key, text } }; onPick(key). Two columns past 6 options.
function C.Choose(anchor, title, options, current, onPick)
    if not chooser then buildChooser() end
    local cols = #options > 6 and 2 or 1
    local rows = math.ceil(#options / cols)
    chooser:SetWidth(cols == 2 and 300 or 176); chooser:SetHeight(40 + rows * 24)
    chooser.title:SetText(title or "")
    for i = 1, math.max(#options, #chooser.btns) do
        local b = chooser.btns[i]
        local o = options[i]
        if o and not b then
            b = button(chooser, "", 136, function(self)
                chooser:Hide()
                if chooser.onPick then chooser.onPick(self.key) end
            end, "TOPLEFT", 16, -30)
            chooser.btns[i] = b
        end
        if b and o then
            local col, row = (i - 1) % cols, math.floor((i - 1) / cols)
            b:ClearAllPoints(); b:SetPoint("TOPLEFT", 16 + col * 140, -30 - row * 24)
            b.key = o[1]
            b:SetText(o[2])
            G.Select(b, o[1] == current)
            b:Show()
        elseif b then
            b:Hide()
        end
    end
    chooser.onPick = onPick
    chooser:ClearAllPoints()
    if anchor then chooser:SetPoint("TOPLEFT", anchor, "BOTTOMLEFT", 0, -2) else chooser:SetPoint("CENTER") end
    chooser:Show()
    return chooser
end
function C.HideChooser() if chooser then chooser:Hide() end end
function C.Chooser() return chooser end

local function roleIcon(parent, size, role, ...)
    local t = parent:CreateTexture(nil, "OVERLAY")
    t:SetWidth(size); t:SetHeight(size)
    t:SetPoint(...)
    t:SetTexture(U.ROLE_ICON)
    local c = U.ROLE_COORDS[role] or U.ROLE_COORDS.dps
    t:SetTexCoord(c[1], c[2], c[3], c[4])
    return t
end

local function para(parent, text, y, w)
    local fs = label(parent, text, "GameFontHighlightSmall", "TOPLEFT", 22, y)
    fs:SetWidth(w or (U.WIN_W - 44))
    return fs
end

-- ---------------------------------------------------------------- Party

C.AddTab("party", "Party", 64, function(page)
    section(page, "Fill my party", -40)
    para(page, "One tank, one healer and damage, minus what your group already has. Pick a class and a spec for "
        .. "each slot, or leave Any. Bots come at your level and gear up.", -60)
    page.slots = {}
    for i, role in ipairs(SLOTS) do
        local y = -100 - (i - 1) * 40
        local row = CreateFrame("Frame", nil, page)
        row:SetWidth(U.WIN_W - 40); row:SetHeight(34)
        row:SetPoint("TOPLEFT", 20, y)
        row.icon = roleIcon(row, 24, role, "LEFT", 6, 0)
        row.role = label(row, U.ROLE_LABEL[role], "GameFontNormal", "LEFT", 38, 0)
        row.cls = button(row, "", 160, function(self)
            local opts = { { "any", "Any" } }
            for _, w in ipairs(ROLE_CLASSES[role]) do opts[#opts + 1] = { w, colourWord(w) } end
            C.Choose(self, U.ROLE_LABEL[role] .. ": class", opts, C.GetPicks()[i].word or "any", function(key)
                C.SetPick(i, key ~= "any" and key or nil); C.Refresh()
            end)
        end, "LEFT", 120, 0)
        row.spec = button(row, "", 180, function(self)
            local p = C.GetPicks()[i]
            if not p.word then return end
            local opts = { { "any", "Any" } }
            for _, sp in ipairs(C.SpecsFor(p.word, role)) do opts[#opts + 1] = { sp, specText(sp) } end
            C.Choose(self, className(p.word) .. ": spec", opts, p.spec or "any", function(key)
                C.SetPick(i, p.word, key ~= "any" and key or false); C.Refresh()
            end)
        end, "LEFT", 288, 0)
        row.slotRole = role
        page.slots[i] = row
    end
    page.fill = button(page, "Fill my party", 140, function() C.Fill("party", C.Picks()) end, "TOPLEFT", 20, -270)
    page.reset = button(page, "Every slot Any", 120, function() C.ResetPicks(); C.Refresh() end, "TOPLEFT", 166, -270)
    page.dismiss = button(page, "Dismiss all", 100, function() C.Dismiss("all") end, "TOPLEFT", 292, -270)
    para(page, "Up to 4 bots in a party. They always pass on loot. The Dungeons tab uses these picks too.", -304)
end, function(page)
    local picks = C.GetPicks()
    for i, row in ipairs(page.slots) do
        local p = picks[i]
        row.cls:SetText("Class: " .. colourWord(p.word))
        row.spec:SetText("Spec: " .. specText(p.word and p.spec or nil))
        if p.word then row.spec:Enable() else row.spec:Disable() end
    end
    if C.state == "on" or C.state == nil then page.fill:Enable() else page.fill:Disable() end
    if #C.bots > 0 then page.dismiss:Enable() else page.dismiss:Disable() end
end)

-- ---------------------------------------------------------------- Raid

local RAID_SHAPE = { "tank", "tank", "healer", "healer", "healer", "dps", "dps", "dps", "dps", "dps" }

C.AddTab("raid", "Raid", 60, function(page)
    section(page, "Fill a 10-player raid", -40)
    para(page, "Two tanks, three healers and five damage, minus what your group already has. Your party becomes a raid. "
        .. "Not from inside a 5-player dungeon.", -60)
    page.icons = {}
    for i, role in ipairs(RAID_SHAPE) do
        local slot = CreateFrame("Frame", nil, page)
        slot:SetWidth(42); slot:SetHeight(42)
        slot:SetPoint("TOPLEFT", 26 + (i - 1) * 48, -100)
        G.Inset(slot)   -- the options panels' group box; its edge turns gold for a role you have
        slot.icon = roleIcon(slot, 30, role, "CENTER", 0, 0)
        page.icons[i] = slot
    end
    page.shape = para(page, "", -152)
    page.fill = button(page, "Fill raid", 120, function() C.Fill("raid") end, "TOPLEFT", 20, -180)
    page.prepare = button(page, "Prepare for raid", 140, function() C.Prepare() end, "TOPLEFT", 146, -180)
    page.dismiss = button(page, "Dismiss all", 100, function() C.Dismiss("all") end, "TOPLEFT", 292, -180)
    page.note = para(page, "", -214)
    para(page, "Up to 9 bots in a raid. Raids are 10-player. Bots know the boss fights of the raids listed in the guide, "
        .. "and they always pass on loot.", -240)
end, function(page)
    -- the raid's shape: you and your bots by role, against 2 / 3 / 5
    local have = { tank = 0, healer = 0, dps = 0 }
    for _, b in ipairs(C.bots) do have[b.role] = (have[b.role] or 0) + 1 end
    local used = { tank = 0, healer = 0, dps = 0 }
    for i, slot in ipairs(page.icons) do
        local role = RAID_SHAPE[i]
        used[role] = used[role] + 1
        local on = used[role] <= (have[role] or 0)
        slot.icon:SetAlpha(on and 1 or 0.3)
        G.InsetOn(slot, on)
    end
    page.shape:SetText(string.format("With you now: %d %s (%d tank%s, %d healer%s, %d damage).", #C.bots,
        #C.bots == 1 and "bot" or "bots", have.tank, have.tank == 1 and "" or "s", have.healer, have.healer == 1 and "" or "s",
        have.dps))
    if C.raidsOn and (C.state == "on" or C.state == nil) then page.fill:Enable() else page.fill:Disable() end
    if #C.bots > 0 then page.prepare:Enable(); page.dismiss:Enable() else page.prepare:Disable(); page.dismiss:Disable() end
    page.note:SetText(C.raidsOn and "" or "Raids with bots are off on this realm.")
end)

-- ---------------------------------------------------------------- Your alts

local ALT_ROWS = 8
C.AddTab("alts", "Your alts", 78, function(page)
    section(page, "Your alts", -40)
    page.refresh = button(page, "Refresh", 70, function() C.AskAlts() end, "TOPRIGHT", -20, -36)
    para(page, "Bring your own characters as bots, with their own gear, spec and level. They pass on loot too.", -60)
    page.rows = {}
    for i = 1, ALT_ROWS do
        local row = CreateFrame("Frame", nil, page)
        row:SetWidth(U.WIN_W - 40); row:SetHeight(28)
        row:SetPoint("TOPLEFT", 20, -84 - (i - 1) * 30)
        row.name = label(row, "", "GameFontNormal", "LEFT", 10, 0)
        row.name:SetWidth(150)
        row.info = label(row, "", "GameFontHighlightSmall", "LEFT", 170, 0)
        row.info:SetWidth(170)
        row.bring = button(row, "Bring", 90, function(self) if self.alt then C.Alt(self.alt) end end, "RIGHT", -6, 0)
        row:Hide()
        page.rows[i] = row
    end
    page.empty = label(page, "", "GameFontDisable", "TOPLEFT", 30, -96)
    page.usual = para(page, "", -334)
    page.bringUsual = button(page, "Bring my usual party", 160, function() C.Usual() end, "TOPLEFT", 20, -356)
    page.save = button(page, "Save who is with me as my usual party", 250, function() C.Save() end, "TOPLEFT", 186, -356)
end, function(page)
    for i, row in ipairs(page.rows) do
        local a = C.alts[i]
        if a then
            local r, g, b = U.classColour(a.cls)
            row.name:SetText(U.hex(r, g, b) .. a.name .. "|r")
            row.info:SetText(a.with and "with you as a bot" or string.format("Level %d %s", a.level, U.CLASS_NAME[a.cls] or ""))
            row.bring.alt = a.name
            if a.with or a.online then row.bring:Disable(); row.bring:SetText(a.with and "With you" or "Online") else
                row.bring:Enable(); row.bring:SetText("Bring") end
            row:Show()
        else
            row:Hide()
        end
    end
    page.empty:SetText(#C.alts == 0 and "No other characters on this account to bring." or "")
    page.usual:SetText(#C.usual > 0 and ("Your usual party: " .. table.concat(C.usual, ", ") .. ".")
        or "No usual party saved yet. Bring your alts, then save them.")
    if #C.usual > 0 then page.bringUsual:Enable() else page.bringUsual:Disable() end
    local alts = false
    for _, b in ipairs(C.bots) do if b.kind == "alt" then alts = true end end
    if alts then page.save:Enable() else page.save:Disable() end
end, function() C.AskAlts() end)

-- ---------------------------------------------------------------- Buddy

C.AddTab("buddy", "Buddy", 64, function(page)
    section(page, "Levelling buddy", -40)
    para(page, "One bot that keeps to your level and follows you. Each time you level up, it catches up. It counts as one "
        .. "of your party's bots.", -60)
    local card = CreateFrame("Frame", nil, page)
    card:SetWidth(U.WIN_W - 40); card:SetHeight(64)
    card:SetPoint("TOPLEFT", 20, -100)
    G.Inset(card)
    card.portrait = card:CreateTexture(nil, "ARTWORK")
    card.portrait:SetWidth(48); card.portrait:SetHeight(48)
    card.portrait:SetPoint("LEFT", 8, 0)
    card.portrait:SetTexture("Interface\\Icons\\INV_Misc_GroupNeedMore")
    card.name = label(card, "", "GameFontNormalLarge", "TOPLEFT", 66, -12)
    card.info = label(card, "", "GameFontHighlightSmall", "TOPLEFT", 66, -36)
    page.card = card
    page.cls = button(page, "", 170, function(self)
        local opts = { { "any", "Any" } }
        for _, w in ipairs(BUDDY_CLASSES) do opts[#opts + 1] = { w, colourWord(w) } end
        C.Choose(self, "Buddy: class", opts, charDB().buddyClass or "any", function(key)
            charDB().buddyClass = key ~= "any" and key or nil; C.Refresh()
        end)
    end, "TOPLEFT", 20, -180)
    page.toggle = button(page, "Bring a buddy", 150, function(self)
        if self.off then C.Buddy("off") else C.Buddy(charDB().buddyClass or "any") end
    end, "TOPLEFT", 196, -180)
    page.note = para(page, "", -214)
end, function(page)
    local buddy
    for _, b in ipairs(C.bots) do if b.kind == "buddy" then buddy = b end end
    local card = page.card
    if buddy then
        local r, g, b = U.classColour(buddy.cls)
        card.name:SetText(U.hex(r, g, b) .. buddy.name .. "|r  |cff9d9d9dbuddy bot|r")
        card.info:SetText(string.format("Level %d %s, keeping to your level", buddy.level, U.CLASS_NAME[buddy.cls] or ""))
        local unit = C.UnitFor(buddy.name)
        if unit and SetPortraitTexture then pcall(SetPortraitTexture, card.portrait, unit) end
    else
        card.name:SetText("No buddy with you")
        card.info:SetText("Pick a class, or leave Any for a surprise.")
        card.portrait:SetTexture("Interface\\Icons\\INV_Misc_GroupNeedMore")
    end
    page.cls:SetText("Class: " .. colourWord(charDB().buddyClass))
    page.toggle.off = buddy ~= nil
    page.toggle:SetText(buddy and "Send buddy home" or "Bring a buddy")
    if C.buddyOn and (C.state == "on" or C.state == nil) then page.toggle:Enable() else page.toggle:Disable() end
    if buddy then page.cls:Disable() else page.cls:Enable() end
    page.note:SetText(C.buddyOn and "" or "The levelling buddy is off on this realm.")
end)

-- ---------------------------------------------------------------- Dungeons

local DUNGEON_ROWS, DUNGEON_ROW_H = 6, 46
local FALLBACK_ICON = "Interface\\Icons\\INV_Misc_GroupNeedMore"
local function norm(s) return (tostring(s or ""):lower():gsub("^the ", ""):gsub("[^%a]", "")) end

-- The game's own Dungeon Finder art for a dungeon: the client's own name for it when GetLFGDungeonInfo knows the
-- dungeon by name, else CompanionsData.lua's name. Paths only; the art is the client's.
local lfgNames
function C.DungeonIcon(d)
    if not lfgNames then
        lfgNames = {}
        if GetLFGDungeonInfo then
            for id = 1, 300 do
                local ok, name, _, _, _, _, _, _, _, _, tex = pcall(GetLFGDungeonInfo, id)
                if ok and type(name) == "string" and type(tex) == "string" and tex ~= "" then
                    lfgNames[norm(name)] = lfgNames[norm(name)] or tex
                end
            end
        end
    end
    local tex = lfgNames[norm(d.name)] or (d.icon ~= "" and d.icon) or nil
    return tex and ("Interface\\LFGFrame\\LFGIcon-" .. tex) or FALLBACK_ICON
end

-- The dungeons to show: those whose range holds your level (or all of them), in level order.
function C.DungeonList(all)
    local lvl = (UnitLevel and UnitLevel("player")) or 1
    local out = {}
    for _, d in ipairs(W.CompanionsDungeons or {}) do
        if all or (lvl >= d.min and lvl <= d.max) then out[#out + 1] = d end
    end
    return out
end

C.AddTab("dungeons", "Dungeons", 84, function(page)
    section(page, "Dungeon buddies", -40)
    para(page, "Pick a dungeon: bots fill your party (your Party tab picks) and everyone meets at the door.", -60, 380)
    local all = CreateFrame("CheckButton", nil, page, "UICheckButtonTemplate")
    all:SetWidth(22); all:SetHeight(22)
    all:SetPoint("TOPRIGHT", -112, -56)
    all:SetScript("OnClick", function(self) C.dungeonAll = self:GetChecked() and true or false; C.dungeonOffset = 0; C.Refresh() end)
    label(page, "Every dungeon", "GameFontHighlightSmall", "LEFT", all, "RIGHT", 2, 0)
    page.all = all
    page.rows = {}
    for i = 1, DUNGEON_ROWS do
        local row = CreateFrame("Frame", nil, page)
        row:SetWidth(U.WIN_W - 40); row:SetHeight(DUNGEON_ROW_H - 4)
        row:SetPoint("TOPLEFT", 20, -84 - (i - 1) * DUNGEON_ROW_H)
        row.icon = row:CreateTexture(nil, "ARTWORK")
        row.icon:SetWidth(36); row.icon:SetHeight(36)
        row.icon:SetPoint("LEFT", 4, 0)
        row.name = label(row, "", "GameFontNormal", "TOPLEFT", 48, -6)
        row.name:SetWidth(300)
        row.info = label(row, "", "GameFontHighlightSmall", "TOPLEFT", 48, -23)
        row.info:SetWidth(300)
        row.go = button(row, "Go", 90, function(self) if self.id then C.Dungeon(self.id, C.Picks()) end end, "RIGHT", -6, 0)
        row:Hide()
        page.rows[i] = row
    end
    page:EnableMouseWheel(true)
    page:SetScript("OnMouseWheel", function(_, delta)
        local n = #C.DungeonList(C.dungeonAll)
        C.dungeonOffset = math.max(0, math.min(math.max(0, n - DUNGEON_ROWS), (C.dungeonOffset or 0) - delta))
        C.Refresh()
    end)
    page.empty = label(page, "", "GameFontDisable", "TOPLEFT", 30, -100)
    page.empty:SetWidth(U.WIN_W - 60)
    page.more = label(page, "", "GameFontDisableSmall", "TOPRIGHT", -24, -84 - DUNGEON_ROWS * DUNGEON_ROW_H)
    para(page, "Not in combat, not as a raid, and only the group leader. One trip to a door every few minutes.",
        -104 - DUNGEON_ROWS * DUNGEON_ROW_H)
end, function(page)
    local list = C.DungeonList(C.dungeonAll)
    local lvl = (UnitLevel and UnitLevel("player")) or 1
    local off = C.dungeonOffset or 0
    if off > math.max(0, #list - DUNGEON_ROWS) then off = math.max(0, #list - DUNGEON_ROWS); C.dungeonOffset = off end
    page.all:SetChecked(C.dungeonAll and true or false)
    for i, row in ipairs(page.rows) do
        local d = list[i + off]
        if d then
            -- SetTexture says nil when the client has no such file: the group icon then
            if not row.icon:SetTexture(C.DungeonIcon(d)) then row.icon:SetTexture(FALLBACK_ICON) end
            row.name:SetText(d.name)
            local fits = lvl >= d.min and lvl <= d.max
            row.info:SetText(string.format("Levels %d to %d%s", d.min, d.max,
                lvl < d.min and ("  |cffff7f76opens at " .. d.min .. "|r") or (fits and "  |cff4fd18bfor your level|r" or "")))
            row.go.id = d.id
            if lvl >= d.min and (C.state == "on" or C.state == nil) then row.go:Enable() else row.go:Disable() end
            row:Show()
        else
            row:Hide()
        end
    end
    page.empty:SetText(#list == 0 and "No dungeon fits your level yet. The first ones open at level 10: tick Every dungeon to "
        .. "see them all." or "")
    page.more:SetText(#list > DUNGEON_ROWS and string.format("%d-%d of %d (scroll)", off + 1, math.min(#list, off + DUNGEON_ROWS),
        #list) or "")
end)
