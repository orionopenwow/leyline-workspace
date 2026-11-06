-- Dojo (3.3.5a API): the Den's training grounds, one small panel with five tabs (Train, Loadout, Practice, Session,
-- Jaina; 5 Nov, it was two tall pages) (owner 3 Nov: "the Dojo in the Den
-- ... all built in with clever UI sections"; "spellbook placement loadout instant on bars, gear sets selector ...
-- better combat and test range dojo features"). The Den's tool rail has a Dojo button (DenBar.lua); it opens this
-- panel beside the rail, in the editor's look (Style.lua S.Ed*: near-black, gold for what is on). It stays open in
-- play mode (training is playing) and closes when you leave the Den.
--
-- Train (the first page)
--   Training      Ring (the training ring: on Den Lite your companion builds it), Dummies (three training dummies),
--                 Clear (the dummies, the boss, the ring)
--   Moving        Fly (on / off), Speed 1 / 2 / 4
--   Jaina builds  Garden, Campfire, Market, Inn (Den Lite: she walks over and builds it piece by piece; the full Den:
--                 the stamp at once)
--   Jaina         Follow, Stay, Quiet (she says nothing unasked; again: she chimes in now and then), Pet (a critter)
--   Her ideas     Idea (a small set in front of you as a sparkling copy), Keep, No (her question: keep the set, tidy
--                 up), Tidy (she asks, then takes away only what she put down and you haven't moved). Den Lite's
--                 brain (labs/den-lite jaina.rs, owner 4 Nov); each is one ".den jaina <word>" (EditMode.lua DenCmd).
--                 On the full Den only Follow and Stay (its ov_den_companion.lua knows those).
--   Practice      Pack, Mover, Healer, Tank, Boss: the sandbox combat's dummies (Den Lite, labs/den-lite combat.rs:
--                 on when its line says combat=1)
--   Range         Levels (dummies three levels below and above you, light and heavy armour), Group (the healing
--                 range), Range (a dummy 40 yards out, markers at 5/10/20/30), Run (your main bar cast for you for
--                 30 s: a practice parse), Safe (nothing hurts you)
--   Session       1m / 3m / 5m (a timed parse), Reset (the dummies, your cooldowns, the meters), Refill, Fight
--                 (dummies hit back)
--   Full Den      Boss (pick one, then Fight), Duel ring (on / off), New look; on Den Lite these say they need the
--                 full Den
-- Loadout (the second page; Den Lite)
--   Class         the class and its spec (click for the next), Be (that class and spec at level 80: its spellbook,
--                 talents and every bar at once: ".den class <class> <spec>"), Mounts (every mount on the right bar)
--   Bars          your saved bars for this spec (click for the next), Use, Save (by the name in the box, or "bar n"),
--                 Reset (the spec's own layout)
--   Gear          the class and spec's raid tiers (T1-T10, the ones your game has) and your own sets, Wear, Save,
--                 Off (your own outfit)
-- The readout: your damage and healing per second, your best boss time.
--
-- Every button is one Den command over the realm's addon channel (mod "wuden", "cmd", "dojo <...>": EditMode.lua
-- DenCmd), the same on both servers: Den Lite (labs/den-lite dojo.rs, sandbox.rs) and the full Den (ov_dojo.lua)
-- answer
--   server -> client   dojo     server=lite|full;fly=0|1;speed=<n>;dummies=<n>;ring=0|1[;building=0|1][;boss=<key|->;duel=0|1;best=<secs|->]
--                               [;combat=1;class=<word|->;level=<n>;fight=0|1;parse=<minutes|->;best=<secs|->;god=0|1]
--                      dps      dps=<n>;total=<n>;secs=<s>;dummy=<name>[;hps=<n>;healed=<n>][;done=1]
--                      loadout  class=<word>;spec=<word>;specs=<a,b,c>;bars=<name|->;saved=<a,b>;set=<name|->;tiers=<1,2>;mine=<a,b>
--                      jaina    talk=quiet|normal|chatty;doing=<word>;mood=<word>;offer=<set|->;ask=offer|tidy<n>|-;mine=<n>;critters=<n>;...
-- Their lines come back as the Den bar's toasts. Nothing the server sends is run; texts are shown plain.

local W = WrathUnleashedUI
local J = { line = {}, jaina = {}, dps = nil, bossAt = 1, classAt = 1, specAt = 1, barAt = 1, gearAt = 1, page = "train",
    loadout = { saved = {}, tiers = {}, mine = {} } }
local MOD = "wuden"
J.STAMPS = { "garden", "campfire", "market", "inn" }
J.BOSSES = { "hogger", "vancleef", "azuregos", "kazzak", "doomwalker" }
J.SPEEDS = { 1, 2, 4 }
J.PRACTICE = { { "pack", "Pack", "Five dummies close together: try your area spells" },
    { "mover", "Mover", "A dummy walking a circle: keep it in range" },
    { "healer", "Healer", "A wounded friendly dummy losing health: heal it" },
    { "tank", "Tank", "A dummy that hits back once you hit it (you never fall)" },
    { "boss", "Boss", "The practice boss: move out of its circles, interrupt its cast" } }
J.RANGE = { { "levels", "Levels", "Dummies three levels below and above you (light and heavy armour: misses count against the higher)" },
    { "group", "Group", "The healing range: five friends losing health, one faster" },
    { "range", "Range", "A dummy 40 yards out, markers at 5, 10, 20 and 30 yards from it" },
    { "demo 30", "Run", "Your main bar cast for you at the nearest dummy for 30 seconds: a practice parse" } }
J.SESSION = { { "parse 1", "1m", "A one minute parse from your first hit (your best per class and spec is kept)" },
    { "parse 3", "3m", "A three minute parse" }, { "parse 5", "5m", "A five minute parse" },
    { "reset", "Reset", "Every dummy full again, your cooldowns ready, your health and power full" },
    { "refill", "Refill", "Your health and power full" } }
J.CLASSES = { "warrior", "paladin", "hunter", "rogue", "priest", "deathknight", "shaman", "mage", "warlock", "druid" }
J.SPECS = { warrior = { "arms", "fury", "protection" }, paladin = { "holy", "protection", "retribution" },
    hunter = { "beastmastery", "marksmanship", "survival" }, rogue = { "assassination", "combat", "subtlety" },
    priest = { "discipline", "holy", "shadow" }, deathknight = { "blood", "frost", "unholy" },
    shaman = { "elemental", "enhancement", "restoration" }, mage = { "arcane", "fire", "frost" },
    warlock = { "affliction", "demonology", "destruction" }, druid = { "balance", "feral", "restoration" } }
J.FULL_ONLY = "Needs the full Den (Leyline > Host > Your Den): Den Lite has no combat."
J.LITE_ONLY = "Den Lite's sandbox: update Your Den"

local panel

local function S() return W.Style end
local function plain(s) return (tostring(s or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("|", ""):gsub("%c", " ")) end
local function title(s) return (s:gsub("^%l", string.upper)) end
local function short(s, n) s = tostring(s or ""); if #s > n then return s:sub(1, n - 2) .. ".." end return s end
-- a word list from the server ("a,b,c"): letters, digits, spaces and dashes only
local function list(s)
    local out = {}
    for w in tostring(s or ""):gmatch("[^,]+") do
        w = w:gsub("[^%w%s%-_]", "")
        if w ~= "" and w ~= "-" and #out < 24 then out[#out + 1] = w:sub(1, 24) end
    end
    return out
end

local function send(name, data)
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, "OVMOD", MOD .. "\t" .. name .. "\t1/1\t" .. (data or ""), "WHISPER", UnitName("player"))
    return true
end

-- One Den command: "dojo <text>".
function J.Cmd(text)
    local t = "dojo" .. (text and text ~= "" and (" " .. text) or "")
    local E = W.EditMode
    if E and E.DenCmd then return E.DenCmd(t) end
    return send("cmd", (t:gsub("[^%w%s%-_]", ""):sub(1, 60)))
end

-- One word for Jaina: ".den jaina <word>" (her brain on Den Lite: jaina.rs `word`; follow and stay on both servers).
function J.Jaina(word)
    local t = "jaina " .. tostring(word or "")
    local E = W.EditMode
    if E and E.DenCmd then return E.DenCmd(t) end
    return send("cmd", (t:gsub("[^%w%s%-_]", ""):sub(1, 60)))
end
J.JAINA_LITE = "Den Lite's companion (Your Den in the launcher)"

function J.Lite() return J.line.server == "lite" end
function J.Combat() return J.line.combat == "1" end
function J.Boss() return J.BOSSES[J.bossAt] end
function J.Class() return J.CLASSES[J.classAt] end
function J.Specs() return J.SPECS[J.Class()] or {} end
function J.Spec() return J.Specs()[J.specAt] or J.Specs()[1] end
function J.NextClass()
    J.classAt = J.classAt % #J.CLASSES + 1
    J.specAt = 1
    J.Paint()
    return J.Class()
end
function J.NextSpec()
    J.specAt = J.specAt % math.max(1, #J.Specs()) + 1
    J.Paint()
    return J.Spec()
end
-- the gear choices: the tiers your game has for the spec ("t7"), then your own sets
function J.GearChoices()
    local out = {}
    for _, t in ipairs(J.loadout.tiers or {}) do out[#out + 1] = "t" .. t end
    for _, n in ipairs(J.loadout.mine or {}) do out[#out + 1] = n end
    return out
end
function J.Gear() return J.GearChoices()[J.gearAt] end
function J.NextGear()
    J.gearAt = J.gearAt % math.max(1, #J.GearChoices()) + 1
    J.PaintPicks()
    return J.Gear()
end
function J.Bar() return (J.loadout.saved or {})[J.barAt] end
function J.NextBar()
    J.barAt = J.barAt % math.max(1, #(J.loadout.saved or {})) + 1
    J.PaintPicks()
    return J.Bar()
end
-- the name typed for a save, else "<what> <n>"
local function saveName(what, have)
    local t = panel and panel.nameBox and panel.nameBox:GetText() or ""
    t = tostring(t or ""):lower():gsub("[^%w%s%-_]", ""):gsub("^%s+", ""):gsub("%s+$", ""):sub(1, 24)
    if t ~= "" then return t end
    return what .. " " .. (#(have or {}) + 1)
end
function J.SaveBars() return J.Cmd("bars save " .. saveName("bar", J.loadout.saved)) end
function J.SaveGear() return J.Cmd("gearset save " .. saveName("set", J.loadout.mine)) end

-- ---------------------------------------------------------------- the panel
local function label(parent, text, ...) return S().EdLabel(parent, text, false, ...) end

-- The panel (owner 5 Nov, live: "the ui is a mess": its title sat over the Train tab, "FULL DEN ONLY" ran under its
-- row's buttons, the panel covered play mode's corner). Now the right-hand inspector (DenLayout.lua), a header row
-- (the title, x), the tabs in their own row, then one page: the label column as wide as the longest label (measured),
-- the buttons after it as wide as their words. Five short pages instead of one tall grid.
J.TABS = { { "train", "Train", "Dummies, moving, the boss" }, { "loadout", "Loadout", "Class and spec, your bars, gear sets" },
    { "practice", "Practice", "Practice dummies and the range" }, { "session", "Session", "Timed parses, reset, refill" },
    { "jaina", "Jaina", "Jaina: her builds, her ideas" } }
local PAD, ROW, GAP = 12, 34, 4

local function build()
    panel = CreateFrame("Frame", "LeylineDojoPanel", UIParent)
    panel:SetWidth(360); panel:SetHeight(300)
    panel:SetPoint("TOPRIGHT", UIParent, "TOPRIGHT", -198, -54)
    panel:SetFrameStrata("MEDIUM")
    panel:EnableMouse(true)
    S().EdPanel(panel, 0.9)
    -- the header row: the title, the close button
    panel.head = S().EdLabel(panel, "Dojo", true, "TOPLEFT", PAD, -14)
    panel.close = S().EdChip(panel, "x", 28, function() J.Show(false) end, "TOPRIGHT", -8, -6)
    panel.close.edLabel = "Close the Dojo"
    -- the tabs, their own row
    panel.tabs = {}
    local tx = PAD - 2
    for _, t in ipairs(J.TABS) do
        local b = S().EdChip(panel, t[2], nil, function() J.Page(t[1]) end, "TOPLEFT", tx, -40)
        b.edLabel = t[3]; b.page = t[1]
        panel.tabs[t[1]] = b
        tx = tx + (tonumber((b:GetWidth())) or 60) + GAP
    end
    local tabsW = tx - GAP + PAD - 2
    panel.note = S().EdText(panel, S().ED_SIZE.small)
    panel.note:SetPoint("TOPLEFT", PAD, -76); panel.note:SetJustifyH("LEFT"); panel.note:SetJustifyV("TOP")
    local m = S().ED.muted
    panel.note:SetTextColor(m[1], m[2], m[3])
    panel.pages = {}
    for _, t in ipairs(J.TABS) do
        local pg = CreateFrame("Frame", nil, panel)
        pg:SetAllPoints(panel)
        panel.pages[t[1]] = pg
    end

    -- every page's rows first (labels and buttons), placed once the label column's width is known
    local rows = {}
    local function row(pg, key, name, list)
        local r = { pg = pg, label = label(pg, name), chips = {} }
        panel[key] = r.label
        for _, c in ipairs(list or {}) do
            local b = S().EdChip(pg, c[2], nil, c[4])
            if c[6] then b:SetWidth(math.max(tonumber((b:GetWidth())) or 0, c[6])) end
            b.edLabel = c[5]
            if c[1] then panel[c[1]] = b end
            r.chips[#r.chips + 1] = b
        end
        rows[#rows + 1] = r
        return r
    end
    local function verbs(r, key, list)
        panel[key] = {}
        for _, c in ipairs(list) do
            local b = S().EdChip(r.pg, c[2], nil, function() J.Cmd(c[1]) end)
            b.edLabel = c[3]; b.verb = c[1]
            panel[key][#panel[key] + 1] = b
            r.chips[#r.chips + 1] = b
        end
    end
    local P = panel.pages

    -- Train: the ring and dummies, moving, the full Den's boss (that row only on the full Den)
    row(P.train, "training", "Training", {
        { "ring", "Ring", 52, function() J.Cmd("ring") end, "A training ring beside you (on Den Lite your companion builds it)" },
        { "dummies", "Dummies", 70, function() J.Cmd("dummies 3") end, "Three training dummies (in the ring when there is one)" },
        { "clear", "Clear", 52, function() J.Cmd("clear") end, "The dummies and the boss go (the ring too on the full Den)" } })
    local mv = row(P.train, "moving", "Moving", {
        { "fly", "Fly", 52, function() J.Cmd("fly") end, "Fly: jump, then Space to rise and X to sink" } })
    panel.speeds = {}
    for _, n in ipairs(J.SPEEDS) do
        local b = S().EdChip(P.train, n .. "x", nil, function() J.Cmd("speed " .. n) end)
        b.edLabel = n == 1 and "Normal speed" or ("Run and fly " .. n .. " times as fast")
        b.speed = n
        panel.speeds[#panel.speeds + 1] = b
        mv.chips[#mv.chips + 1] = b
    end
    panel.fullRow = row(P.train, "full", "Full Den", {
        { "bossPick", title(J.Boss()), 78, function() J.NextBoss() end, "Which boss (click for the next)", 96 },
        { "fight", "Fight", 46, function() J.Cmd("boss " .. J.Boss()) end, "The boss, 12 yards ahead: beat your best time" },
        { "duel", "Duels", 50, function() J.Cmd("duel " .. (J.line.duel == "1" and "off" or "on")) end, "Duels in the Den become timed spars" },
        { "look", "Look", 44, function() J.Cmd("look") end, "A new look at the character list" } })

    -- Loadout: class and spec, bars, gear, a name for Save
    row(P.loadout, "classRow", "Class", {
        { "classPick", title(J.Class()), 86, function() J.NextClass() end, "Which class (click for the next)", 100 },
        { "specPick", title(J.Spec()), 96, function() J.NextSpec() end, "Which spec (click for the next)", 124 },
        { "classBe", "Be", 34, function() J.Cmd("class " .. J.Class() .. " " .. J.Spec()) end, "Become that class and spec at level 80: its spellbook, talents and every bar at once" },
        { "mounts", "Mounts", 58, function() J.Cmd("mounts") end, "Every mount in your spellbook, on the right bar (flying ones fly with Fly on)" } })
    row(P.loadout, "barsRow", "Bars", {
        { "barPick", "No saved bar", 96, function() J.NextBar() end, "Your saved bars for this spec (click for the next)", 110 },
        { "barUse", "Use", 40, function() if J.Bar() then J.Cmd("bars use " .. J.Bar()) end end, "Put that bar on" },
        { "barSave", "Save", 44, function() J.SaveBars() end, "Keep your bar as it is now (by the name in the box)" },
        { "barReset", "Reset", 48, function() J.Cmd("bars reset") end, "The spec's own layout again" } })
    row(P.loadout, "gearRow", "Gear", {
        { "gearPick", "No sets", 96, function() J.NextGear() end, "Raid tiers for your class and spec, then your own sets (click for the next)", 110 },
        { "gearWear", "Wear", 44, function() if J.Gear() then J.Cmd("gearset " .. J.Gear()) end end, "Put that whole look on (everyone in the Den sees it)" },
        { "gearSave", "Save", 44, function() J.SaveGear() end, "Keep what you wear as a set (by the name in the box)" },
        { "gearOff", "Off", 40, function() J.Cmd("gearset off") end, "Your own outfit again" } })
    local nr = row(P.loadout, "nameRow", "Name")
    panel.nameBox = S().EdInput(P.loadout, 180, 26)
    panel.nameBox.edLabel = "A name for Save (empty: bar 1, set 1...)"
    if panel.nameBox.SetMaxLetters then panel.nameBox:SetMaxLetters(24) end
    nr.chips[1] = panel.nameBox

    -- Practice: the sandbox combat's dummies, the range
    verbs(row(P.practice, "practice", "Practice"), "practiceChips", J.PRACTICE)
    local rr = row(P.practice, "rangeRow", "Range")
    verbs(rr, "rangeChips", J.RANGE)
    panel.safe = S().EdChip(P.practice, "Safe", nil, function() J.Cmd("invulnerable " .. (J.line.god == "1" and "off" or "on")) end)
    panel.safe.edLabel = "Nothing hurts you (again: as before)"
    panel.rangeChips[#panel.rangeChips + 1] = panel.safe
    rr.chips[#rr.chips + 1] = panel.safe

    -- Session: timed parses, reset, refill, the dummies hitting back
    local sr = row(P.session, "session", "Session")
    verbs(sr, "sessionChips", J.SESSION)
    panel.fightBack = S().EdChip(P.session, "Fight", nil, function() J.Cmd("fight " .. (J.line.fight == "1" and "off" or "on")) end)
    panel.fightBack.edLabel = "The dummies hit back (lightly)"
    panel.sessionChips[#panel.sessionChips + 1] = panel.fightBack
    sr.chips[#sr.chips + 1] = panel.fightBack

    -- Jaina: her builds, her orders, her ideas
    local br = row(P.jaina, "builds", "Builds")
    panel.stamps = {}
    for _, s in ipairs(J.STAMPS) do
        local b = S().EdChip(P.jaina, title(s), nil, function() J.Cmd("build " .. s) end)
        b.edLabel = "Jaina builds a " .. s .. " in front of you"
        b.stamp = s
        panel.stamps[#panel.stamps + 1] = b
        br.chips[#br.chips + 1] = b
    end
    row(P.jaina, "jainaRow", "Orders", {
        { "jFollow", "Follow", 54, function() J.Jaina("follow") end, "She follows you" },
        { "jStay", "Stay", 44, function() J.Jaina("stay") end, "She waits where she is" },
        { "jQuiet", "Quiet", 50, function() J.Jaina(J.jaina.talk == "quiet" and "talk" or "quiet") end,
            "Quiet: she says nothing unasked (gestures only). Again: she chimes in now and then" },
        { "jPet", "Pet", 40, function() J.Jaina("pet") end, "A small critter by you (three at most; they go when you leave)" } })
    row(P.jaina, "ideasRow", "Her ideas", {
        { "jIdea", "Idea", 44, function() J.Jaina("idea") end, "She suggests a small set in front of you: a sparkling copy you keep or send away" },
        { "jYes", "Keep", 46, function() J.Jaina("yes") end, "Keep her set (one undo takes it back), or yes to her question" },
        { "jNo", "No", 36, function() J.Jaina("no") end, "Send her set away, or no to her question" },
        { "jTidy", "Tidy", 44, function() J.Jaina("tidy") end, "She asks first, then takes away only what she put down and you haven't moved" } })
    for _, k in ipairs({ "jIdea", "jTidy", "jPet", "jQuiet" }) do panel[k].edLabel0 = panel[k].edLabel end

    -- the label column: as wide as the longest label on any page (so the pages line up)
    local labels = {}
    for _, r in ipairs(rows) do labels[#labels + 1] = r.label end
    local col = PAD + (W.DenLayout and W.DenLayout.Column(labels, 60, 14) or 110)
    panel.col = col
    local widest = tabsW
    for _, r in ipairs(rows) do
        local x = col
        for _, b in ipairs(r.chips) do x = x + (tonumber((b:GetWidth())) or 40) + GAP end
        r.width = x - GAP + PAD
        if r.width > widest then widest = r.width end
    end
    panel.rows = rows
    panel:SetWidth(widest)
    panel.note:SetWidth(widest - 2 * PAD)
    panel.readout = S().EdText(panel, S().ED_SIZE.small)
    panel.readout:SetJustifyH("LEFT"); panel.readout:SetJustifyV("TOP")
    panel.readout:SetWidth(widest - 2 * PAD)
    panel:Hide()
    if W.DenLayout then W.DenLayout.Dock(panel, "inspector", { name = "dojo" }) end
    J.Page(J.page)
    J.Paint()
end

-- The rows of each page placed top down (a row that is hidden takes no room), the note's height measured; the panel
-- as tall as its page, the readout under it.
function J.Layout()
    if not panel then return end
    local nh
    -- the note wraps: as many lines as its words need in its width
    local nw = tonumber((panel.note:GetStringWidth())) or 0
    local lines = math.max(1, math.ceil(nw / math.max(1, (tonumber((panel.note:GetWidth())) or 300))))
    if (panel.note:GetText() or "") == "" then lines = 0 end
    nh = lines * (S().ED_SIZE.small + 3)
    panel.note:SetHeight(math.max(1, nh))
    local top = -76 - nh - (lines > 0 and 10 or 0)
    local ys = {}
    for _, r in ipairs(panel.rows) do
        local y = ys[r.pg] or top
        local on = not r.hidden
        if on then
            r.label:ClearAllPoints(); r.label:SetPoint("TOPLEFT", r.pg, "TOPLEFT", PAD, y - 7)
            local x = panel.col
            for _, b in ipairs(r.chips) do
                b:ClearAllPoints(); b:SetPoint("TOPLEFT", r.pg, "TOPLEFT", x, y)
                x = x + (tonumber((b:GetWidth())) or 40) + GAP
            end
            y = y - ROW
        end
        if on then r.label:Show() else r.label:Hide() end
        for _, b in ipairs(r.chips) do if on then b:Show() else b:Hide() end end
        ys[r.pg] = y
    end
    local y = ys[panel.pages[J.page]] or top
    local text = panel.readout:GetText() or ""
    local rh = 0
    if text ~= "" then
        local rw = tonumber((panel.readout:GetStringWidth())) or 0
        rh = math.max(1, math.ceil(rw / math.max(1, (tonumber((panel.readout:GetWidth())) or 300)))) * (S().ED_SIZE.small + 3)
    end
    panel.readout:ClearAllPoints(); panel.readout:SetPoint("TOPLEFT", panel, "TOPLEFT", PAD, y - 6)
    panel.readout:SetHeight(math.max(1, rh))
    panel:SetHeight(-y + (rh > 0 and rh + 14 or 0) + 8)
    if panel:IsShown() and W.DenLayout then W.DenLayout.Layout() end
end

function J.Panel() if not panel then build() end return panel end
function J.IsOpen() return panel ~= nil and panel:IsShown() end

-- A page: "train" or "loadout" (the tab lights up).
function J.Page(which)
    local ok = false
    for _, t in ipairs(J.TABS) do if t[1] == which then ok = true end end
    J.page = ok and which or "train"
    if not panel then return J.page end
    for k, pg in pairs(panel.pages) do if k == J.page then pg:Show() else pg:Hide() end end
    for k, b in pairs(panel.tabs) do b:SetActive(k == J.page) end
    J.Paint()
    return J.page
end

function J.Show(on)
    J.Panel()
    if on then
        panel:Show()
        J.Cmd("status")
    else
        panel:Hide()
    end
    J.Paint()
    if W.DenBar and W.DenBar.Paint then W.DenBar.Paint() end
    return J.IsOpen()
end
function J.Toggle() return J.Show(not J.IsOpen()) end

function J.NextBoss()
    J.bossAt = J.bossAt % #J.BOSSES + 1
    if panel then panel.bossPick:SetText(title(J.Boss())) end
    return J.Boss()
end

-- The pickers' words (shortened to fit their chips).
function J.PaintPicks()
    if not panel then return end
    panel.classPick:SetText(title(J.Class()))
    panel.specPick:SetText(short(title(J.Spec() or "-"), 13))
    local bar = J.Bar()
    panel.barPick:SetText(bar and short(bar, 12) or "No saved bar")
    if bar then panel.barUse:Enable() else panel.barUse:Disable() end
    local g = J.Gear()
    panel.gearPick:SetText(g and short(g:gsub("^t(%d+)$", "Tier %1"), 12) or "No sets")
    if g then panel.gearWear:Enable() else panel.gearWear:Disable() end
end

-- What is on (gold), what this server can't do (off, saying why), the readout.
function J.Paint()
    if not panel then return end
    local l = J.line
    local lite = J.Lite()
    if l.server == "lite" then
        panel.note:SetText(J.Combat() and "Den Lite: any class and spec, dummies that take your spells, parses and a boss."
            or "Den Lite: the ring, dummies to practise on, flying, speed and Jaina's builds.")
    elseif l.server == "full" then
        panel.note:SetText("The full Den: dummies count your damage, bosses keep your best time.")
    else
        panel.note:SetText("Training grounds in your Den.")
    end
    panel.fly:SetActive(l.fly == "1")
    for _, b in ipairs(panel.speeds) do b:SetActive(tostring(b.speed) == (l.speed or "1")) end
    panel.ring:SetActive(l.ring == "1")
    panel.dummies:SetActive((tonumber(l.dummies or "") or 0) > 0)
    panel.duel:SetActive(l.duel == "1")
    panel.fight:SetActive(l.boss ~= nil and l.boss ~= "-")
    for _, k in ipairs({ "bossPick", "fight", "duel", "look" }) do
        local b = panel[k]
        if lite then b:Disable(); b.edLabel = J.FULL_ONLY else b:Enable() end
    end
    -- the full Den's row: not shown at all on Den Lite (owner 5 Nov: greyed "FULL DEN ONLY" buttons were clutter)
    panel.fullRow.hidden = lite
    -- the sandbox combat's rows and the loadout page: on when the server says it has it (Den Lite's combat=1)
    local combat = J.Combat()
    for _, list in ipairs({ panel.practiceChips, panel.rangeChips, panel.sessionChips }) do
        for _, b in ipairs(list) do
            if combat then b:Enable() else b:Disable(); b.edLabel = "Needs Den Lite's sandbox combat (update Your Den)" end
        end
    end
    for _, k in ipairs({ "classPick", "specPick", "classBe", "mounts", "barPick", "barSave", "barReset", "gearPick", "gearSave", "gearOff" }) do
        if combat then panel[k]:Enable() else panel[k]:Disable() end
    end
    panel.fightBack:SetActive(l.fight == "1")
    panel.safe:SetActive(l.god == "1")
    J.PaintPicks()
    if not combat then panel.barUse:Disable(); panel.gearWear:Disable() end
    local lo = J.loadout
    panel.classBe:SetActive(lo.class ~= nil and lo.class == J.Class() and lo.spec == J.Spec())
    local parts = {}
    if J.dps then
        local d = J.dps
        local s = string.format("%s%d DPS|r  %d in %s s", S().Hex("accent"), d.dps, d.total, d.secs)
        if (d.hps or 0) > 0 then s = s .. string.format("   %s%d HPS|r  %d healed", S().Hex("accent"), d.hps, d.healed or 0) end
        parts[#parts + 1] = s .. (d.done and " (done)" or "")
    end
    if l.best and l.best ~= "-" then parts[#parts + 1] = "Best boss time: " .. l.best .. " s" end
    if l.building == "1" then parts[#parts + 1] = "Jaina is building..." end
    -- Jaina's row: lit from her line (Den Lite); without it only Follow and Stay (the full Den knows those)
    local jl = J.jaina or {}
    local brain = jl.talk ~= nil
    panel.jFollow:SetActive(jl.doing ~= nil and jl.doing ~= "stay")
    panel.jStay:SetActive(jl.doing == "stay")
    panel.jQuiet:SetActive(jl.talk == "quiet")
    for _, k in ipairs({ "jIdea", "jTidy", "jPet", "jQuiet" }) do
        local b = panel[k]
        if brain then b:Enable(); b.edLabel = b.edLabel0 else b:Disable(); b.edLabel = J.JAINA_LITE end
    end
    local asking = brain and jl.ask ~= nil and jl.ask ~= "-"
    for _, k in ipairs({ "jYes", "jNo" }) do if asking then panel[k]:Enable() else panel[k]:Disable() end end
    if brain and jl.offer and jl.offer ~= "-" then parts[#parts + 1] = "Jaina's idea: " .. title(jl.offer) .. " (Keep or No)"
    elseif brain and jl.ask and jl.ask:match("^tidy") then parts[#parts + 1] = "Jaina asks: tidy up? (Keep or No)" end
    if combat and l.class and l.class ~= "-" then
        parts[#parts + 1] = title(l.class) .. " " .. (l.level or "") .. ((lo.spec and lo.class == l.class) and (" " .. title(lo.spec)) or "")
    end
    if combat and lo.set and lo.set ~= "-" then parts[#parts + 1] = "Gear: " .. lo.set end
    if combat and l.parse and l.parse ~= "-" then parts[#parts + 1] = l.parse .. " min parse" end
    panel.readout:SetText(#parts > 0 and table.concat(parts, "   ") or ((lite and not combat) and "" or "Hit a dummy: your damage per second shows here."))
    -- without the sandbox combat: the Practice and Session pages say why their buttons are off
    if not combat and (J.page == "practice" or J.page == "session" or J.page == "loadout") then
        panel.note:SetText("These need Den Lite's sandbox combat: update Your Den in the launcher.")
    end
    J.Layout()
end

-- ---------------------------------------------------------------- the server's answers
local function fields(data, n)
    local t = {}
    for k, v in data:gmatch("(%a+)=([^;]*)") do t[k] = plain(v):sub(1, n) end
    return t
end

function J.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= MOD then return end
    if name == "dojo" then
        local l = fields(data, 24)
        if l.server ~= "lite" and l.server ~= "full" then return end
        J.line = l
        J.Paint()
        if W.DenBar and W.DenBar.Paint then W.DenBar.Paint() end
    elseif name == "jaina" then
        local t = fields(data, 24)
        if t.talk ~= "quiet" and t.talk ~= "normal" and t.talk ~= "chatty" then return end
        J.jaina = t
        J.Paint()
    elseif name == "dps" then
        local t = fields(data, 40)
        local dps, total = tonumber(t.dps or ""), tonumber(t.total or "")
        if not (dps and total) then return end
        J.dps = { dps = dps, total = total, secs = (t.secs or "?"):sub(1, 8), dummy = (t.dummy or ""):sub(1, 40), done = t.done == "1",
            hps = tonumber(t.hps or "") or 0, healed = tonumber(t.healed or "") or 0 }
        if J.dps.done and W.DenBar and W.DenBar.Toast then
            local s = string.format("%d damage per second (%d in %s s).", dps, total, J.dps.secs)
            if J.dps.hps > 0 then s = s .. string.format(" %d healing per second.", J.dps.hps) end
            W.DenBar.Toast("ok", s)
        end
        J.Paint()
    elseif name == "loadout" then
        local t = fields(data, 200)
        local class = (t.class or ""):lower():gsub("[^%a]", "")
        if not J.SPECS[class] then return end
        local lo = { class = class, spec = (t.spec or ""):lower():gsub("[^%a]", ""), bars = list(t.bars)[1], saved = list(t.saved),
            set = list(t.set)[1] or "-", tiers = {}, mine = list(t.mine) }
        for n in tostring(t.tiers or ""):gmatch("%d+") do
            if #lo.tiers < 10 and tonumber(n) >= 1 and tonumber(n) <= 10 then lo.tiers[#lo.tiers + 1] = tonumber(n) end
        end
        J.loadout = lo
        -- the pickers follow what you are
        for i, c in ipairs(J.CLASSES) do if c == class then J.classAt = i end end
        for i, s in ipairs(J.Specs()) do if s == lo.spec then J.specAt = i end end
        J.barAt = 1
        for i, n in ipairs(lo.saved) do if n == lo.bars then J.barAt = i end end
        J.gearAt = math.min(J.gearAt, math.max(1, #J.GearChoices()))
        J.Paint()
    end
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:RegisterEvent("PLAYER_ENTERING_WORLD")
listener:SetScript("OnEvent", function(_, event, prefix, msg, channel, sender)
    if event == "CHAT_MSG_ADDON" then return J.OnMessage(prefix, msg, channel, sender) end
    -- leaving the Den: the panel goes with it (DenBar.lua decides what a Den is; it loads first)
    if event == "PLAYER_ENTERING_WORLD" and panel and not (W.DenBar and W.DenBar.IsActive and W.DenBar.IsActive()) then
        panel:Hide()
    end
end)

SLASH_LEYLINEDOJO1 = "/dojo"
SlashCmdList["LEYLINEDOJO"] = function() J.Toggle() end

W.Dojo = J
