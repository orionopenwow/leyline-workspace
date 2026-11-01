-- DenHome (3.3.5a API): a Den's home panel, next to the Den panel (EditMode.lua). /denhome, or Den panel > Home.
-- What the realm's ov_den_home.lua does, as buttons:
--   * Start your Den (the owner): Forest glade, Dark keep, Empty. The realm asks on the owner's first visit to an
--     empty Den ("start"), and the panel opens on that choice.
--   * Stamps, ready-made areas: Garden, Market square, Campfire circle, Dock, Small house. A pick marks where it goes
--     at your feet (stakes at its corners and one ahead: the ghost before placing); Turn left / right (15 degrees),
--     Here, Place, Cancel. The Builder's spawn menu has the same stamps in its Stamps tab.
--   * Placing at your feet: Grid off / 0.5 / 1 / 2 / 5 yards and Ground on / off (.build grid; kept per character).
--   * Visiting: Go to the front door, Knock, and a note for the mailbox (up to 200 letters).
--   * Mailbox (the owner): how many new notes, Read mail (the latest notes show here and in chat), Empty it.
-- The realm checks everything (who you are in this Den, budgets, rates). OVMOD mod "wuden", whispered to yourself:
--   client -> server   cmd  start <key> [replace] | stamp <name> | stamp turn <deg> | stamp here|place|cancel |
--                           visit | knock | mail [clear]          (as typed after .den)
--                      home                                       send me the home line
--                      note <text>                                a note for the mailbox
--   server -> client   home  started=..;door=0|1;mailbox=0|1;mail=<n>;role=..;stamp=<name|->;stamps=a,b
--                      start forest,Forest glade;...  |  knock <name>  |  mail <id>;<from>;<at>;<text>
-- Nothing the realm sends is run; names and notes are shown as text, colour codes taken out.

local W = WrathUnleashedUI
local H = { home = {}, mail = {}, gridAt = 1 }
local MOD = "wuden"
local STAMPS = {
    { key = "garden", name = "Garden", line = "A fenced flower bed with a bench and a lantern." },
    { key = "market", name = "Market square", line = "Two stalls, a counter, crates and barrels." },
    { key = "campfire", name = "Campfire circle", line = "A campfire with benches around it." },
    { key = "dock", name = "Dock", line = "Boards out in front of you, barrels and a fishing chair." },
    { key = "house", name = "Small house", line = "A furnished cottage with a front door and a mailbox." },
}
local STARTERS = { { "forest", "Forest glade" }, { "keep", "Dark keep" }, { "empty", "Empty" } }
local GRIDS = { 0, 0.5, 1, 2, 5 }
local NOTE_MAX = 200

local panel

local function plain(s)
    return (tostring(s or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("|H.-|h", ""):gsub("|T.-|t", ""):gsub("|", ""))
end

local function sendRaw(name, data)
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, "OVMOD", MOD .. "\t" .. name .. "\t1/1\t" .. (data or ""), "WHISPER", UnitName("player"))
    return true
end
local function denCmd(text)
    if W.EditMode and W.EditMode.DenCmd then return W.EditMode.DenCmd(text) end
    return sendRaw("cmd", (tostring(text or ""):gsub("[^%w%s%-_]", ""):sub(1, 60)))
end
local function buildCmd(text)
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, "OVMOD", "wubuild\tcmd\t1/1\t" .. text, "WHISPER", UnitName("player"))
    return true
end

-- per character: the grid and ground snap for placing at your feet
local function saved()
    WrathUnleashedUIChar = WrathUnleashedUIChar or {}
    local c = WrathUnleashedUIChar
    c.denhome = type(c.denhome) == "table" and c.denhome or {}
    return c.denhome
end

function H.StampItems()
    local out = {}
    for _, s in ipairs(STAMPS) do out[#out + 1] = { kind = "stamp", key = s.key, id = "stamp:" .. s.key, name = s.name, line = s.line } end
    return out
end

-- A stamp picked here or in the spawn menu: the realm marks where it goes (nothing placed until Place).
function H.Stamp(key)
    for _, s in ipairs(STAMPS) do
        if s.key == key then
            denCmd("stamp " .. key)
            H.Show(true)
            H.SetStatus(s.name .. ": marked at your feet. Turn it, then Place.")
            return true
        end
    end
    return false
end

function H.Grid() local g = tonumber(saved().grid) or 0; return g end
function H.SetGrid(g, quiet)
    saved().grid = g
    if not quiet then buildCmd(g > 0 and ("grid " .. g) or "grid off") end
    H.Refresh()
end
function H.SetGround(on, quiet)
    saved().ground = on and true or nil
    if not quiet then buildCmd("grid ground " .. (on and "on" or "off")) end
    H.Refresh()
end

function H.LeaveNote(text)
    local t = plain(text):gsub("[%c]", " "):gsub("^%s+", ""):gsub("%s+$", "")
    if t == "" then H.SetStatus("Write a few words first."); return false end
    if #t > NOTE_MAX then t = t:sub(1, NOTE_MAX) end
    sendRaw("note", t)
    if panel then panel.note:SetText("") end
    return true
end

-- ---------------------------------------------------------------- the panel
local function button(parent, text, w, onClick, ...)
    local b
    if W.Style then b = W.Style.Button(parent, text, w, onClick, nil, ...); b:SetHeight(22); return b end
    b = CreateFrame("Button", nil, parent, "UIPanelButtonTemplate")
    b:SetWidth(w); b:SetHeight(22)
    b:SetText(text)
    b:SetPoint(...)
    b:SetScript("OnClick", onClick)
    return b
end
local function label(parent, text, font, ...)
    local fs = parent:CreateFontString(nil, "OVERLAY", font or "GameFontHighlightSmall")
    fs:SetPoint(...)
    fs:SetJustifyH("LEFT")
    fs:SetText(text)
    return fs
end

function H.SetStatus(t)
    H.status = t or ""
    if panel then panel.status:SetText(H.status) end
end

local function gridText() local g = H.Grid(); return g > 0 and ("Grid " .. g .. " yd") or "Grid off" end
local function groundText() return saved().ground and "Ground on" or "Ground off" end

function H.Refresh()
    if not panel then return end
    local h = H.home
    local owner, visitor = h.role == "owner", h.role == "visitor"
    local started = h.started and h.started ~= "-" and h.started or nil
    local names = { forest = "Forest glade", keep = "Dark keep", empty = "Empty", own = "your own" }
    panel.who:SetText(owner and ("Your Den" .. (started and (": " .. (names[started] or started)) or ", not started yet"))
        or h.role == "builder" and "You build in this Den." or "You're visiting this Den.")
    for _, b in ipairs(panel.ownerOnly) do if owner then b:Enable() else b:Disable() end end
    for _, b in ipairs(panel.builders) do if visitor then b:Disable() else b:Enable() end end
    panel.replace:SetText(H.replace and "Replacing: yes" or "Replacing: no")
    panel.grid:SetText(gridText())
    panel.ground:SetText(groundText())
    local n = tonumber(h.mail or "0") or 0
    panel.mailLine:SetText(owner and (n > 0 and (n .. " new note" .. (n == 1 and "" or "s")) or "No new notes") or "Notes go to the owner.")
    panel.marked:SetText((h.stamp and h.stamp ~= "-") and ("Marked: " .. h.stamp) or "")
    for i, fs in ipairs(panel.notes) do
        local m = H.mail[i]
        fs:SetText(m and (m.from .. ": " .. m.text) or "")
    end
end

local function build()
    panel = CreateFrame("Frame", "LeylineDenHomePanel", UIParent)
    panel:SetWidth(470); panel:SetHeight(470)
    panel:SetPoint("LEFT", UIParent, "LEFT", 456, 0)
    panel:SetFrameStrata("DIALOG")
    panel:EnableMouse(true)
    if W.Style then W.Style.Window(panel, "Den: home")
    else
        panel:SetBackdrop({ bgFile = "Interface\\Tooltips\\UI-Tooltip-Background", edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border",
            tile = true, tileSize = 16, edgeSize = 16, insets = { left = 4, right = 4, top = 4, bottom = 4 } })
        label(panel, "Den: home", "GameFontNormal", "TOPLEFT", 14, -12)
    end
    panel.ownerOnly, panel.builders = {}, {}
    local function own(b) panel.ownerOnly[#panel.ownerOnly + 1] = b; return b end
    local function bld(b) panel.builders[#panel.builders + 1] = b; return b end
    panel.who = label(panel, "", "GameFontHighlight", "TOPLEFT", 14, -36)

    -- start your Den
    label(panel, "Start your Den as:", nil, "TOPLEFT", 14, -60)
    panel.start = {}
    local x = 12
    for _, s in ipairs(STARTERS) do
        local key = s[1]
        panel.start[key] = own(button(panel, s[2], 100, function() denCmd("start " .. key .. (H.replace and " replace" or "")) end, "TOPLEFT", x, -76))
        x = x + 104
    end
    panel.replace = own(button(panel, "Replacing: no", 110, function() H.replace = not H.replace; H.Refresh() end, "TOPLEFT", x + 6, -76))

    -- stamps
    label(panel, "Stamps (ready-made areas): pick one, turn it, Place.", nil, "TOPLEFT", 14, -108)
    panel.stamp = {}
    for i, s in ipairs(STAMPS) do
        local col, row = (i - 1) % 3, math.floor((i - 1) / 3)
        panel.stamp[s.key] = bld(button(panel, s.name, 140, function() H.Stamp(s.key) end, "TOPLEFT", 12 + col * 146, -124 - row * 26))
    end
    panel.turnL = bld(button(panel, "Turn left", 84, function() denCmd("stamp turn 15") end, "TOPLEFT", 12, -180))
    panel.turnR = bld(button(panel, "Turn right", 84, function() denCmd("stamp turn -15") end, "TOPLEFT", 100, -180))
    panel.here = bld(button(panel, "Here", 60, function() denCmd("stamp here") end, "TOPLEFT", 188, -180))
    panel.place = bld(button(panel, "Place", 70, function() denCmd("stamp place") end, "TOPLEFT", 252, -180))
    panel.cancel = bld(button(panel, "Cancel", 70, function() denCmd("stamp cancel") end, "TOPLEFT", 326, -180))
    panel.marked = label(panel, "", nil, "TOPLEFT", 14, -206)

    -- placing at your feet
    label(panel, "Placing at your feet:", nil, "TOPLEFT", 14, -226)
    panel.grid = bld(button(panel, gridText(), 100, function()
        local g, nxt = H.Grid(), GRIDS[1]
        for i, v in ipairs(GRIDS) do if math.abs(v - g) < 0.001 then nxt = GRIDS[i % #GRIDS + 1] end end
        H.SetGrid(nxt)
    end, "TOPLEFT", 150, -222))
    panel.ground = bld(button(panel, groundText(), 100, function() H.SetGround(not saved().ground) end, "TOPLEFT", 254, -222))

    -- visiting
    label(panel, "Visiting:", nil, "TOPLEFT", 14, -256)
    panel.visit = button(panel, "Go to the front door", 150, function() denCmd("visit") end, "TOPLEFT", 80, -252)
    panel.knock = button(panel, "Knock", 70, function() denCmd("knock") end, "TOPLEFT", 234, -252)
    panel.note = (W.Style and W.Style.Input and W.Style.Input(panel)) or CreateFrame("EditBox", nil, panel, "InputBoxTemplate")
    panel.note:SetWidth(300); panel.note:SetHeight(20)
    panel.note:SetPoint("TOPLEFT", 16, -282)
    panel.note:SetAutoFocus(false)
    panel.note:SetMaxLetters(NOTE_MAX)
    panel.note:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
    panel.note:SetScript("OnEnterPressed", function(self) self:ClearFocus(); H.LeaveNote(self:GetText()) end)
    panel.leave = button(panel, "Leave a note", 120, function() H.LeaveNote(panel.note:GetText()) end, "TOPLEFT", 326, -280)

    -- the mailbox
    label(panel, "Mailbox:", nil, "TOPLEFT", 14, -314)
    panel.mailLine = label(panel, "", nil, "TOPLEFT", 80, -314)
    panel.read = own(button(panel, "Read mail", 90, function() H.mail = {}; denCmd("mail") end, "TOPLEFT", 240, -310))
    panel.empty = own(button(panel, "Empty it", 80, function() H.mail = {}; denCmd("mail clear") end, "TOPLEFT", 334, -310))
    panel.notes = {}
    for i = 1, 5 do
        local fs = label(panel, "", nil, "TOPLEFT", 16, -338 - (i - 1) * 16)
        fs:SetWidth(440)
        panel.notes[i] = fs
    end

    panel.status = label(panel, H.status or "", nil, "BOTTOMLEFT", 14, 14)
    panel.status:SetWidth(360)
    button(panel, "Close", 70, function() panel:Hide() end, "BOTTOMRIGHT", -12, 10)
    if UISpecialFrames then tinsert(UISpecialFrames, "LeylineDenHomePanel") end
    panel:Hide()
end

function H.Show(show)
    if not panel then build() end
    if show then
        H.Refresh()
        panel:Show()
        sendRaw("home", "")
    else
        panel:Hide()
    end
end
function H.Toggle() H.Show(not (panel and panel:IsShown())) end
function H.Panel() return panel end

-- ---------------------------------------------------------------- the realm's answers
function H.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= MOD then return end
    if name == "home" then
        local h = {}
        for k, v in data:gmatch("(%a+)=([^;]*)") do h[k] = plain(v) end
        H.home = h
        H.Refresh()
    elseif name == "start" then
        H.home.role = "owner"
        H.Show(true)
        H.SetStatus("Pick how your Den starts: Forest glade, Dark keep or Empty.")
    elseif name == "knock" then
        local who = plain(data):gsub("[^%w]", ""):sub(1, 12)
        if who == "" then return end
        if PlaySound then pcall(PlaySound, "ReadyCheck") end
        if UIErrorsFrame and UIErrorsFrame.AddMessage then pcall(UIErrorsFrame.AddMessage, UIErrorsFrame, who .. " knocks at your front door.", 1, 0.5, 1) end
        H.SetStatus(who .. " knocked at your front door.")
    elseif name == "mail" then
        local id, from, at, text = data:match("^(%d+);([%w]*);(%d+);(.*)$")
        if not id then return end
        table.insert(H.mail, { id = tonumber(id), from = from, at = tonumber(at), text = plain(text):sub(1, NOTE_MAX) })
        while #H.mail > 5 do table.remove(H.mail) end
        H.Refresh()
    end
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:RegisterEvent("PLAYER_ENTERING_WORLD")
listener:SetScript("OnEvent", function(_, event, prefix, msg, channel, sender)
    if event == "CHAT_MSG_ADDON" then return H.OnMessage(prefix, msg, channel, sender) end
    -- the grid and ground snap are the realm's per login: say them again (only when set, so nothing else hears)
    if event == "PLAYER_ENTERING_WORLD" and not H.told then
        H.told = true
        if H.Grid() > 0 then buildCmd("grid " .. H.Grid()) end
        if saved().ground then buildCmd("grid ground on") end
    end
end)

SLASH_LEYLINEDENHOME1 = "/denhome"
SlashCmdList["LEYLINEDENHOME"] = function() H.Toggle() end

H.STAMPS, H.STARTERS, H.GRIDS = STAMPS, STARTERS, GRIDS
W.DenHome = H
