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
--   * GM (the owner and builders, never visitors; only inside the Den, all back to normal on leaving): speed 1x / 2x /
--     4x, Fly, Mark, Go to mark, Middle, Revive; the owner also Clear / Rain / Snow, Invisible and Summon a builder.
--     Each button's one-line hint shows under the row while the mouse is on it.
--   * The template shelf (edit mode, a Den's owner and builders): a slim row of picture tiles under the selection
--     card, the stamps (Inn, Small house, Garden, Market, Campfire, Dock) and, for the owner, the starter places.
--     Click a tile: ".den stamp <name>" marks it at your feet, and the mark follows you as you walk (".den stamp
--     here", at most every 1.5 seconds, only while you move); Q / E turn it ("stamp turn 15" / "-15"), Enter or
--     Place puts it down ("stamp place"), Esc cancels ("stamp cancel"), the same tile again cancels too. A starter
--     place's tile waits for Enter or Place (".den start <key>", "replace" when the Den has things in it; the realm
--     keeps a backup). The keys are override bindings (SetOverrideBindingClick, as Modes.lua) only while something is
--     being placed; a hover shows a one-line hint under the row. The same Q / E / Esc work while a Builder prop waits
--     for its ground click (EditMode.lua).
-- The realm checks everything (who you are in this Den, budgets, rates). OVMOD mod "wuden", whispered to yourself:
--   client -> server   cmd  start <key> [replace] | stamp <name> | stamp turn <deg> | stamp here|place|cancel |
--                           visit | knock | mail [clear] | gm speed 1|2|4 | gm fly on|off | mark | go | home |
--                           gm revive | gm weather clear|rain|snow | gm invisible on|off | gm summon <name>   (after .den)
--                      home                                       send me the home line
--                      note <text>                                a note for the mailbox
--   server -> client   home  started=..;door=0|1;mailbox=0|1;mail=<n>;role=..;stamp=<name|->;stamps=a,b;gm=<speed>,<fly>,<invisible>
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
    { key = "inn", name = "Inn", line = "A small inn: tables, kegs, lanterns, a mailbox and a fire outside." },
}
local STARTERS = { { "forest", "Forest glade" }, { "keep", "Dark keep" }, { "empty", "Empty" } }
-- the shelf's tiles: stock 3.3.5a icons (Interface\Icons\...), stamps first, then the owner's starter places
local ICON = "Interface\\Icons\\"
local SHELF_STAMPS = { "inn", "house", "garden", "market", "campfire", "dock" }
local SHELF_ICONS = { inn = "INV_Misc_Rune_01", house = "INV_Misc_Key_05", garden = "INV_Misc_Flower_02",
    market = "INV_Misc_Coin_01", campfire = "Spell_Fire_Fire", dock = "Trade_Fishing",
    forest = "Spell_Nature_Tranquility", keep = "Spell_Shadow_Twilight", empty = "INV_Misc_Note_01" }
local STARTER_LINES = { forest = "Trees, a brook, a pond, a cottage.", keep = "A ruined tower, mist, braziers.",
    empty = "The plain room, nothing in it." }
local TILE, TILE_GAP = 32, 4
local HERE_EVERY = 1.5   -- seconds between "stamp here" while you walk
local TURN_STEP = 15
local MARK_KEEP = 120    -- seconds: the realm's own mark lasts two minutes untouched (ov_den_home.lua)
local GRIDS = { 0, 0.5, 1, 2, 5 }
local NOTE_MAX = 200
-- the GM row: { key, label, width, command (or a function of the panel), hint }; owner = only the owner sees it
local GM_ALL = {
    { "s1", "1x", 32, "gm speed 1", "Normal run speed." },
    { "s2", "2x", 32, "gm speed 2", "Run twice as fast, here in the Den." },
    { "s4", "4x", 32, "gm speed 4", "Run four times as fast, here in the Den." },
    { "fly", "Fly: off", 62, function() return "gm fly " .. ((H.gm or {}).fly and "off" or "on") end,
        "Fly in the Den: jump, then hold Space to rise and X to sink." },
    { "mark", "Mark", 50, "mark", "Save this spot as your mark (one per character)." },
    { "go", "Go to mark", 76, "go", "Back to the spot you marked." },
    { "middle", "Middle", 56, "home", "Back to the middle of the room." },
    { "revive", "Revive", 56, "gm revive", "Back on your feet if you died." },
}
local GM_OWNER = {
    { "clear", "Clear", 48, "gm weather clear", "Clear skies over the Den, for everyone here." },
    { "rain", "Rain", 48, "gm weather rain", "Rain over the Den, for everyone here." },
    { "snow", "Snow", 48, "gm weather snow", "Snow over the Den, for everyone here." },
    { "invisible", "Invisible: off", 96, function() return "gm invisible " .. ((H.gm or {}).invisible and "off" or "on") end,
        "Visitors can't see you while it's on." },
    { "summon", "Summon", 64, function(p)
        local n = (p.summonName:GetText() or ""):gsub("[^%a]", ""):sub(1, 12)
        if n == "" then H.SetStatus("Type the name of a friend who builds here."); return nil end
        return "gm summon " .. n
    end, "Call over a friend who builds here (type their name first)." },
}

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

local function stampOf(key) for _, s in ipairs(STAMPS) do if s.key == key then return s end end end
local function clock() return (GetTime and GetTime()) or 0 end

-- A stamp picked here or in the spawn menu: the realm marks where it goes (nothing placed until Place). From the
-- shelf (or the spawn menu in edit mode) the mark also follows you and the placing keys are on; from the panel it
-- stays where it was marked (walk around it, then Here), and the keys only in edit mode.
function H.Stamp(key, fromShelf)
    local s = stampOf(key)
    if not s then return false end
    denCmd("stamp " .. key)
    H.armed = { kind = "stamp", key = key, name = s.name, at = clock(), follow = fromShelf and true or nil }
    H.lastHere, H.moved = clock(), false
    if not fromShelf then H.Show(true) end
    H.SetStatus(s.name .. ": marked at your feet. Turn it, then Place.")
    H.Say(s.name .. ": Q / E turn, Enter places, Esc cancels.")
    H.UpdateKeys()
    return true
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
    -- the GM row: the owner and builders; the owner's buttons only for the owner
    local sp, fl, iv = tostring(h.gm or ""):match("^(%d+),(%d),(%d)$")
    H.gm = { speed = tonumber(sp) or 1, fly = fl == "1", invisible = iv == "1" }
    local gmOn = owner or h.role == "builder"
    for _, f in ipairs(panel.gmAll) do if gmOn then f:Show() else f:Hide() end end
    for _, f in ipairs(panel.gmOwner) do if owner then f:Show() else f:Hide() end end
    panel.gm.fly:SetText(H.gm.fly and "Fly: on" or "Fly: off")
    panel.gm.invisible:SetText(H.gm.invisible and "Invisible: on" or "Invisible: off")
    for _, k in ipairs({ 1, 2, 4 }) do
        local b = panel.gm["s" .. k]
        b:SetText((H.gm.speed == k and "[" .. k .. "x]") or (k .. "x"))
    end
end

function H.GmClick(key)
    if not panel then return nil end
    for _, list in ipairs({ GM_ALL, GM_OWNER }) do
        for _, g in ipairs(list) do
            if g[1] == key then
                local c = g[4]
                if type(c) == "function" then c = c(panel) end
                if type(c) == "string" then denCmd(c) end
                return c
            end
        end
    end
end

local function build()
    panel = CreateFrame("Frame", "LeylineDenHomePanel", UIParent)
    panel:SetWidth(470); panel:SetHeight(560)
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
    panel.turnL = bld(button(panel, "Turn left", 84, function() denCmd("stamp turn " .. TURN_STEP) end, "TOPLEFT", 12, -180))
    panel.turnR = bld(button(panel, "Turn right", 84, function() denCmd("stamp turn -" .. TURN_STEP) end, "TOPLEFT", 100, -180))
    panel.here = bld(button(panel, "Here", 60, function() denCmd("stamp here") end, "TOPLEFT", 188, -180))
    panel.place = bld(button(panel, "Place", 70, function() H.Place(true) end, "TOPLEFT", 252, -180))
    panel.cancel = bld(button(panel, "Cancel", 70, function() H.Cancel(true) end, "TOPLEFT", 326, -180))
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

    -- GM controls (the owner and builders; the realm checks who, where and how often)
    panel.gm, panel.gmAll, panel.gmOwner = {}, {}, {}
    local gmLabel = label(panel, "GM, here in the Den (back to normal when you leave):", nil, "TOPLEFT", 14, -424)
    panel.gmAll[#panel.gmAll + 1] = gmLabel
    panel.gmHint = label(panel, "", nil, "TOPLEFT", 14, -494)
    panel.gmHint:SetWidth(440)
    panel.gmAll[#panel.gmAll + 1] = panel.gmHint
    local function gmRow(list, into, y, gx)
        for _, g in ipairs(list) do
            local key, hint = g[1], g[5]
            local b = button(panel, g[2], g[3], function() H.GmClick(key) end, "TOPLEFT", gx, y)
            local enter, leave = b:GetScript("OnEnter"), b:GetScript("OnLeave")
            b:SetScript("OnEnter", function(self, ...) if enter then enter(self, ...) end; panel.gmHint:SetText(hint) end)
            b:SetScript("OnLeave", function(self, ...) if leave then leave(self, ...) end; panel.gmHint:SetText("") end)
            b.hint = hint
            panel.gm[key] = b
            into[#into + 1] = b
            gx = gx + g[3] + 4
            if key == "invisible" then   -- the summon name box sits before Summon
                panel.summonName = (W.Style and W.Style.Input and W.Style.Input(panel)) or CreateFrame("EditBox", nil, panel, "InputBoxTemplate")
                panel.summonName:SetWidth(96); panel.summonName:SetHeight(20)
                panel.summonName:SetPoint("TOPLEFT", gx + 4, y - 1)
                panel.summonName:SetAutoFocus(false)
                panel.summonName:SetMaxLetters(12)
                panel.summonName:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
                into[#into + 1] = panel.summonName
                gx = gx + 108
            end
        end
    end
    gmRow(GM_ALL, panel.gmAll, -440, 12)
    gmRow(GM_OWNER, panel.gmOwner, -466, 12)

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


-- ---------------------------------------------------------------- the template shelf (edit mode)
local shelf
local function E() return W.EditMode end
local function editing() local e = E(); return e and e.IsOn and e.IsOn() end
local function role()   -- the home line's, else the Builder state's (both the realm's)
    local B = W.Builder
    return H.home.role or (B and B.state and B.state.den)
end
local function denHasThings()
    if H.home.started and H.home.started ~= "-" then return true end
    local B = W.Builder
    local st = (B and B.state) or {}
    for _, k in ipairs({ "room", "yard" }) do
        local used = tonumber(tostring(st[k] or ""):match("^(%d+)/") or "")
        if used and used > 0 then return true end
    end
    return false
end

function H.ShelfItems()
    local out = {}
    for _, key in ipairs(SHELF_STAMPS) do
        local s = stampOf(key)
        if s then out[#out + 1] = { kind = "stamp", key = key, name = s.name, hint = s.name .. ": " .. s.line, icon = ICON .. SHELF_ICONS[key] } end
    end
    for _, st in ipairs(STARTERS) do
        out[#out + 1] = { kind = "start", key = st[1], name = st[2], owner = true, icon = ICON .. SHELF_ICONS[st[1]],
            hint = st[2] .. " (a whole Den): " .. STARTER_LINES[st[1]] }
    end
    return out
end

-- The line under the tiles: what is being placed, or the hovered tile's hint.
function H.Say(text)
    H.shelfLine = text or ""
    if shelf then shelf.hint:SetText(H.shelfLine) end
end

local function disarm(note)
    H.armed = nil
    H.Say(note or "")
    H.UpdateKeys()
    H.ShelfRefresh()
end

-- A tile clicked: a stamp is marked (and follows you), a starter place waits for Enter or Place. The same tile
-- again cancels.
function H.ShelfClick(key)
    if H.armed and H.armed.key == key then return H.Cancel() end
    for _, it in ipairs(H.ShelfItems()) do
        if it.key == key then
            if it.kind == "stamp" then
                local e = E()
                if e and e.pending and e.Cancel then e.Cancel() end
                return H.Stamp(key, true)
            end
            if role() ~= "owner" then return false end
            if H.armed and H.armed.kind == "stamp" then denCmd("stamp cancel") end
            H.armed = { kind = "start", key = key, name = it.name, replace = denHasThings(), at = clock() }
            H.Say(it.name .. (H.armed.replace and ": replaces your Den (a backup is kept). Enter, or Esc." or ": Enter to start, Esc cancels."))
            H.UpdateKeys()
            H.ShelfRefresh()
            return true
        end
    end
    return false
end

-- Place (the shelf's, the panel's, Enter): the marked stamp goes down, or the starter place starts.
function H.Place(fromPanel)
    local a = H.armed
    if a and a.kind == "start" then
        denCmd("start " .. a.key .. (a.replace and " replace" or ""))
        disarm(a.name .. ": on its way.")
        return true
    end
    if not a and not fromPanel then return false end
    denCmd("stamp place")
    disarm(a and (a.name .. " placed. Ctrl+Z takes it back.") or nil)
    return true
end

function H.Cancel(fromPanel)
    local a = H.armed
    if a and a.kind == "start" then disarm("Kept your Den as it is."); return true end
    if not a and not fromPanel then
        local e = E()
        if e and e.pending and e.Cancel then e.Cancel("Cancelled."); H.UpdateKeys(); return true end
        return false
    end
    denCmd("stamp cancel")
    disarm(a and "Cancelled." or nil)
    return true
end

function H.Turn(deg)
    local a = H.armed
    if a and a.kind == "stamp" then a.at = clock(); denCmd("stamp turn " .. deg); return true end
    if a then return false end
    local e = E()
    return e and e.TurnPending and e.TurnPending(deg) or false
end

-- Something being placed: a stamp or starter from here, or a Builder prop waiting for its ground click.
function H.Placing()
    if H.armed then return true end
    local e = E()
    return (e and e.pending) and true or false
end

-- The keys while placing. key: Q, E, ENTER, ESCAPE. src: who called (the binding, or the client's own Q hook):
-- one press seen by both counts once.
H.lastKey = {}
function H.Key(key, src)
    src = src or "client"
    local last, now = H.lastKey[key], clock()
    if last and last.src ~= src and now - last.at < 0.2 then return false end
    H.lastKey[key] = { src = src, at = now }
    if key == "Q" then return H.Turn(TURN_STEP)
    elseif key == "E" then return H.Turn(-TURN_STEP)
    elseif key == "ENTER" then return H.Place()
    elseif key == "ESCAPE" then return H.Cancel() end
    return false
end

-- Override bindings to hidden buttons while placing (Modes.lua's way: a click binding gets one click as the key
-- goes down and one as it comes up; a client that says which is believed once it said "down").
local keyOwner, keyButtons, keyHalf, keyDownSeen = nil, {}, {}, false
local function keyButton(key)
    local name = "LeylineDenKey" .. key
    if keyButtons[key] then return name end
    local b = CreateFrame("Button", name, UIParent)
    b:EnableMouse(false)
    b:RegisterForClicks("AnyDown", "AnyUp")
    b:SetScript("OnClick", function(_, _, down)
        if down == true then keyDownSeen = true end
        if keyDownSeen then
            if down == true then H.Key(key, "bind") end
        else
            keyHalf[key] = not keyHalf[key]
            if keyHalf[key] then H.Key(key, "bind") end
        end
    end)
    keyButtons[key] = b
    return name
end
H.keysOn = ""
function H.UpdateKeys()
    local want = {}
    local e = E()
    if H.armed and (H.armed.follow or H.armed.kind == "start" or editing()) then want = { "Q", "E", "ENTER", "ESCAPE" }
    elseif e and e.pending then
        want = e.pending.kind == "prop" and { "Q", "E", "ESCAPE" } or { "ESCAPE" }
    end
    local sig = table.concat(want, " ")
    if sig == H.keysOn then return end
    if not keyOwner then keyOwner = CreateFrame("Frame", "LeylineDenKeys", UIParent) end
    if ClearOverrideBindings then ClearOverrideBindings(keyOwner) end
    keyHalf = {}
    H.keysOn = sig
    if not SetOverrideBindingClick then return end
    for _, k in ipairs(want) do SetOverrideBindingClick(keyOwner, true, k, keyButton(k)) end
end

local function tile(parent, it)
    local b = CreateFrame("Button", nil, parent)
    b:SetWidth(TILE); b:SetHeight(TILE)
    b.icon = b:CreateTexture(nil, "ARTWORK")
    b.icon:SetAllPoints(b)
    b.icon:SetTexture(it.icon)
    b.icon:SetTexCoord(0.08, 0.92, 0.08, 0.92)
    b.icon.path = it.icon
    b:SetHighlightTexture("Interface\\Buttons\\ButtonHilight-Square", "ADD")
    b.on = b:CreateTexture(nil, "OVERLAY")
    b.on:SetTexture("Interface\\Buttons\\CheckButtonHilight")
    b.on:SetBlendMode("ADD")
    b.on:SetAllPoints(b)
    b.on:Hide()
    b.item, b.hint = it, it.hint
    b:SetScript("OnClick", function() H.ShelfClick(it.key) end)
    b:SetScript("OnEnter", function() if shelf then shelf.hint:SetText(it.hint) end end)
    b:SetScript("OnLeave", function() if shelf then shelf.hint:SetText(H.shelfLine or "") end end)
    return b
end

-- Built with the edit HUD (EditMode.lua): on the world frame, under the selection card.
function H.BuildShelf(hud, card, scale)
    if shelf then return shelf end
    shelf = CreateFrame("Frame", "LeylineDenShelf", hud)
    shelf:SetScale(scale or 1)
    shelf:SetPoint("TOPLEFT", card, "BOTTOMLEFT", 0, -8)
    local items = H.ShelfItems()
    local nStamps = 0
    for _, it in ipairs(items) do if it.kind == "stamp" then nStamps = nStamps + 1 end end
    local width = 16 + #items * (TILE + TILE_GAP) + 10
    shelf:SetWidth(width); shelf:SetHeight(TILE + 44)
    if W.Style and W.Style.Box then W.Style.Box(shelf, 0.88)
    else
        local bg = shelf:CreateTexture(nil, "BACKGROUND")
        bg:SetTexture(0.06, 0.07, 0.09, 0.88); bg:SetAllPoints(shelf)
    end
    shelf.tiles, shelf.starters = {}, {}
    local x = 8
    for i, it in ipairs(items) do
        if i == nStamps + 1 then x = x + 10 end   -- a small gap before the starter places
        local b = tile(shelf, it)
        b:SetPoint("TOPLEFT", x, -8)
        x = x + TILE + TILE_GAP
        shelf.tiles[it.key] = b
        if it.kind == "start" then shelf.starters[#shelf.starters + 1] = b end
    end
    shelf.hint = shelf:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    shelf.hint:SetPoint("TOPLEFT", 8, -TILE - 16)
    shelf.hint:SetWidth(width - 140); shelf.hint:SetJustifyH("LEFT")
    shelf.hint:SetText(H.shelfLine or "")
    shelf.place = button(shelf, "Place", 56, function() H.Place() end, "TOPRIGHT", -70, -TILE - 12)
    shelf.cancel = button(shelf, "Cancel", 60, function() H.Cancel() end, "TOPRIGHT", -8, -TILE - 12)
    shelf:Hide()
    H.ShelfRefresh()
    return shelf
end
function H.Shelf() return shelf end

-- Shown only in edit mode, in a Den, for its owner and builders; the starter places for the owner only.
function H.ShelfRefresh()
    if not shelf then return end
    local r = role()
    local on = editing() and (r == "owner" or r == "builder")
    if not on then if shelf:IsShown() then shelf:Hide() end; return end
    shelf:Show()
    for _, b in ipairs(shelf.starters) do if r == "owner" then b:Show() else b:Hide() end end
    for key, b in pairs(shelf.tiles) do if H.armed and H.armed.key == key then b.on:Show() else b.on:Hide() end end
    if H.Placing() then shelf.cancel:Show() else shelf.cancel:Hide() end
    if H.armed then shelf.place:Show() else shelf.place:Hide() end   -- a prop goes down with its ground click
end

-- The mark follows you: while you walk (GetUnitSpeed), "stamp here" at most every HERE_EVERY seconds, and once
-- more after you stop. Edit mode roots your character, so there it stays put until you step out (F7) and walk.
function H.Tick(elapsed)
    H.tickAcc = (H.tickAcc or 0) + (elapsed or 0)
    if H.tickAcc >= 0.25 then
        H.tickAcc = 0
        H.UpdateKeys()
        H.ShelfRefresh()
    end
    local a = H.armed
    if not a then return end
    -- the realm takes an untouched mark away after two minutes; so does the shelf (and gives the keys back)
    if clock() - (a.at or 0) > MARK_KEEP then return disarm("The mark timed out.") end
    if a.kind ~= "stamp" or not a.follow then return end
    if GetUnitSpeed then
        local ok, speed = pcall(GetUnitSpeed, "player")
        if ok and (tonumber(speed) or 0) > 0 then H.moved = true end
    end
    if H.moved and clock() - (H.lastHere or 0) >= HERE_EVERY then
        H.lastHere, H.moved, a.at = clock(), false, clock()
        denCmd("stamp here")
    end
end
local ticker = CreateFrame("Frame", nil, WorldFrame or UIParent)
ticker:SetScript("OnUpdate", function(_, elapsed) H.Tick(elapsed) end)

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
        -- the shelf's mark: seen on the realm, then gone (placed, cancelled, two minutes passed)
        local a = H.armed
        if a and a.kind == "stamp" then
            if h.stamp == a.key then a.seen = true
            elseif a.seen then disarm("") end
        end
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

H.STAMPS, H.STARTERS, H.GRIDS, H.GM_ALL, H.GM_OWNER = STAMPS, STARTERS, GRIDS, GM_ALL, GM_OWNER
H.SHELF_ICONS, H.HERE_EVERY = SHELF_ICONS, HERE_EVERY
W.DenHome = H
