-- Dojo (3.3.5a API): the Den's training grounds, one small panel (owner 3 Nov: "the Dojo in the Den ... all built in
-- with clever UI sections"). The Den's tool rail has a Dojo button (DenBar.lua); it opens this panel beside the rail,
-- in the editor's look (Style.lua S.Ed*: near-black, gold for what is on). It stays open in play mode (training is
-- playing) and closes when you leave the Den.
--
--   Training      Ring (the training ring: on Den Lite your companion builds it), Dummies (three training dummies),
--                 Clear (the dummies, the boss, the ring)
--   Moving        Fly (on / off), Speed 1 / 2 / 4
--   Jaina builds  Garden, Campfire, Market, Inn (Den Lite: she walks over and builds it piece by piece; the full Den:
--                 the stamp at once)
--   Full Den      Boss (pick one, then Fight), Duel ring (on / off), New look; on Den Lite these say they need the
--                 full Den (no combat engine there)
--   the readout   your damage per second on the dummies (full Den), and your best boss time
--
-- Every button is one Den command over the realm's addon channel (mod "wuden", "cmd", "dojo <...>": EditMode.lua
-- DenCmd), the same on both servers: Den Lite (labs/den-lite dojo.rs) and the full Den (ov_dojo.lua) answer
--   server -> client   dojo  server=lite|full;fly=0|1;speed=<n>;dummies=<n>;ring=0|1[;building=0|1][;boss=<key|->;duel=0|1;best=<secs|->]
--                      dps   dps=<n>;total=<n>;secs=<s>;dummy=<name>[;done=1]
-- Their lines come back as the Den bar's toasts. Nothing the server sends is run; texts are shown plain.

local W = WrathUnleashedUI
local J = { line = {}, dps = nil, bossAt = 1 }
local MOD = "wuden"
J.STAMPS = { "garden", "campfire", "market", "inn" }
J.BOSSES = { "hogger", "vancleef", "azuregos", "kazzak", "doomwalker" }
J.SPEEDS = { 1, 2, 4 }
J.FULL_ONLY = "Needs the full Den (Leyline > Host > Your Den): Den Lite has no combat."

local panel

local function S() return W.Style end
local function plain(s) return (tostring(s or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("|", ""):gsub("%c", " ")) end
local function title(s) return (s:gsub("^%l", string.upper)) end

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

function J.Lite() return J.line.server == "lite" end
function J.Boss() return J.BOSSES[J.bossAt] end

-- ---------------------------------------------------------------- the panel
local function chip(parent, text, w, fn, ...) return S().EdChip(parent, text, w, fn, ...) end
local function label(parent, text, ...) return S().EdLabel(parent, text, false, ...) end

local function build()
    panel = CreateFrame("Frame", "LeylineDojoPanel", UIParent)
    panel:SetWidth(318); panel:SetHeight(236)
    -- right of the Den's tool rail, under the player frame (not over it)
    panel:SetPoint("TOPLEFT", UIParent, "TOPLEFT", 80, -118)
    panel:SetFrameStrata("MEDIUM")
    panel:EnableMouse(true)
    S().EdPanel(panel, 0.9)
    panel.head = S().EdLabel(panel, "Dojo", true, "TOPLEFT", 10, -9)
    panel.close = chip(panel, "x", 24, function() J.Show(false) end, "TOPRIGHT", -8, -8)
    panel.note = panel:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    panel.note:SetPoint("TOPLEFT", 10, -28); panel.note:SetWidth(298); panel.note:SetJustifyH("LEFT")
    panel.note.styleKeep = true

    local y = -50
    local function row(name)
        local l = label(panel, name, "TOPLEFT", 10, y - 5)
        y = y - 34
        return l
    end
    -- chips as wide as their words (nothing cut), the panel as wide as its widest row
    local widest = 0
    local function chips(list, x0, at)
        local x = x0 or 104
        for _, c in ipairs(list) do
            local b = chip(panel, c[2], nil, c[4], "TOPLEFT", x, at)
            if c[1] == "bossPick" then b:SetWidth(math.max(tonumber((b:GetWidth())) or 0, 96)) end
            b.edLabel = c[5]
            panel[c[1]] = b
            x = x + (tonumber((b:GetWidth())) or c[3]) + 4
        end
        widest = math.max(widest, x)
        return x
    end
    local at = y
    panel.training = row("Training")
    chips({
        { "ring", "Ring", 52, function() J.Cmd("ring") end, "A training ring beside you (on Den Lite your companion builds it)" },
        { "dummies", "Dummies", 70, function() J.Cmd("dummies 3") end, "Three training dummies (in the ring when there is one)" },
        { "clear", "Clear", 52, function() J.Cmd("clear") end, "The dummies and the boss go (the ring too on the full Den)" },
    }, nil, at)
    at = y
    panel.moving = row("Moving")
    local x = chips({ { "fly", "Fly", 52, function() J.Cmd("fly") end, "Fly: jump, then Space to rise and X to sink" } }, nil, at) + 8
    panel.speeds = {}
    for _, n in ipairs(J.SPEEDS) do
        local b = chip(panel, n .. "x", nil, function() J.Cmd("speed " .. n) end, "TOPLEFT", x, at)
        b.edLabel = n == 1 and "Normal speed" or ("Run and fly " .. n .. " times as fast")
        b.speed = n
        panel.speeds[#panel.speeds + 1] = b
        x = x + (tonumber((b:GetWidth())) or 34) + 4
    end
    widest = math.max(widest, x)
    at = y
    panel.builds = row("Jaina builds")
    panel.stamps = {}
    x = 104
    for _, s in ipairs(J.STAMPS) do
        local b = chip(panel, title(s), nil, function() J.Cmd("build " .. s) end, "TOPLEFT", x, at)
        b.edLabel = "Jaina builds a " .. s .. " in front of you"
        b.stamp = s
        panel.stamps[#panel.stamps + 1] = b
        x = x + (tonumber((b:GetWidth())) or 52) + 4
    end
    widest = math.max(widest, x)
    at = y
    panel.full = row("Full Den")
    chips({
        { "bossPick", title(J.Boss()), 78, function() J.NextBoss() end, "Which boss (click for the next)" },
        { "fight", "Fight", 46, function() J.Cmd("boss " .. J.Boss()) end, "The boss, 12 yards ahead: beat your best time" },
        { "duel", "Duels", 50, function() J.Cmd("duel " .. (J.line.duel == "1" and "off" or "on")) end, "Duels in the Den become timed spars" },
        { "look", "Look", 44, function() J.Cmd("look") end, "A new look at the character list" },
    }, nil, at)
    panel.readout = panel:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    local pw = math.max(318, widest + 10)
    panel:SetWidth(pw)
    panel:SetHeight(-y + 46)
    panel.note:SetWidth(pw - 20)
    panel.readout:SetPoint("TOPLEFT", 10, y - 6); panel.readout:SetWidth(pw - 20); panel.readout:SetJustifyH("LEFT")
    panel.readout.styleKeep = true
    panel:Hide()
    J.Paint()
end

function J.Panel() if not panel then build() end return panel end
function J.IsOpen() return panel ~= nil and panel:IsShown() end

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

-- What is on (gold), what this server can't do (off, saying why), the readout.
function J.Paint()
    if not panel then return end
    local l = J.line
    local lite = J.Lite()
    if l.server == "lite" then
        panel.note:SetText("Den Lite: the ring, dummies to practise on, flying, speed and Jaina's builds.")
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
    panel.full:SetText(lite and "FULL DEN ONLY" or "FULL DEN")
    local parts = {}
    if J.dps then
        parts[#parts + 1] = string.format("%s%d DPS|r  %d in %s s%s", S().Hex("accent"), J.dps.dps, J.dps.total, J.dps.secs, J.dps.done and " (done)" or "")
    end
    if l.best and l.best ~= "-" then parts[#parts + 1] = "Best boss time: " .. l.best .. " s" end
    if l.building == "1" then parts[#parts + 1] = "Jaina is building..." end
    panel.readout:SetText(#parts > 0 and table.concat(parts, "   ") or (lite and "" or "Hit a dummy: your damage per second shows here."))
end

-- ---------------------------------------------------------------- the server's answers
function J.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= MOD then return end
    if name == "dojo" then
        local l = {}
        for k, v in data:gmatch("(%a+)=([^;]*)") do l[k] = plain(v):sub(1, 24) end
        if l.server ~= "lite" and l.server ~= "full" then return end
        J.line = l
        J.Paint()
        if W.DenBar and W.DenBar.Paint then W.DenBar.Paint() end
    elseif name == "dps" then
        local t = {}
        for k, v in data:gmatch("(%a+)=([^;]*)") do t[k] = plain(v) end
        local dps, total = tonumber(t.dps or ""), tonumber(t.total or "")
        if not (dps and total) then return end
        J.dps = { dps = dps, total = total, secs = (t.secs or "?"):sub(1, 8), dummy = (t.dummy or ""):sub(1, 40), done = t.done == "1" }
        if J.dps.done and W.DenBar and W.DenBar.Toast then
            W.DenBar.Toast("ok", string.format("%d damage per second (%d in %s s).", dps, total, J.dps.secs))
        end
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
