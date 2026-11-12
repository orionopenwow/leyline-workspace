-- The Bots window (3.3.5a API): your bots at a glance and your orders in one click. Tabs:
--   Your bots  the roster and the orders below; Party (Fill my party with a class and spec per slot), Raid (fill a
--   10-player raid), Your alts (bring your own characters, save them as your usual party), Buddy (one levelling
--   buddy), Dungeons (CompanionsTabs.lua: pick a dungeon for your level, bots fill the party and meet you at the
--   door). The bar of your bots, the orders bar and the first-time tips are BotBar.lua.
--   Roster:  each bot's role icon, name in its class colour (always tagged a bot), spec, item level, health and
--            mana, and what it is doing (following, staying, passive, dead). Click a bot for its own menu:
--            a spec from the realm's premade specs, a role swap (tank, healer, damage), fresh gear, follow, stay,
--            dismiss. Your alts keep their own talents and gear.
--   Orders:  follow, stay, attack my target, passive / fight, assist the tank, crowd control, a formation and
--            "Prepare for raid" (spells, gear, buffs).
--   Group:   fill a party or a raid, bring your usual party, dismiss everyone.
-- Opened by /companions, /comp and the minimap button (shown once the realm says it has bots).
--
-- Nothing goes through chat. The realm (realm/custom/lua_scripts/ov_companions.lua) does everything and checks
-- everything (your bot, combat, cooldowns); this file only asks. OVMOD channel, mod "wucomp":
--   client -> server   roster | order <what> <name|all> | formation <name> | prepare | fill <party|raid>[\t<picks>] |
--                      usual | dismiss <name|all> | spec <name>\t<spec> | role <name> <tank|healer|dps> | gear <name> |
--                      alts | alt <name> | save | buddy <class|any|off> | dungeon <id>[\t<picks>]
--   server -> client   roster (H/B/S lines, see ov_companions.lua) | alts (A/U lines) | msg <text> |
--                      welcome <text> (single player: bots joined a new character by themselves; this file says it,
--                      with how to change them: the window's key and the minimap button)
-- The window's key (Esc > Key Bindings > Bots > Bots window) is set to Ctrl-B the first time the realm says it has
-- bots, only if that key is free and the window has no key yet, and never again (a player who clears it keeps it so).
-- Health and mana come from the game's own unit functions (party1-4, raid1-40), so they move live.
--
-- Bot chatter: the core tells everyone nearby when a bot earns an achievement, and bots answer orders in
-- whispers. With "Hide bot chatter" on (the default), achievement lines from your bots are hidden, and so are
-- your bots' whispers for a few seconds after an order from this window or after they join.

local W = WrathUnleashedUI
local PREFIX, MOD = "OVMOD", "wucomp"
local WIN_W, WIN_H = 540, 622
local PAGE_TOP = 8    -- the pages start under the title; the tabs hang under the window (the game's bottom tabs)
local ROWS, ROW_H = 9, 36
local QUIET_ORDER, QUIET_JOIN = 8, 25   -- seconds of hidden bot whispers after an order / after a bot joins
local ASK_DEBOUNCE, ANSWER_WAIT = 1.5, 6
local TAG = "|cff66ccff[Bots]|r "

local CLASS_TOKEN = { [1] = "WARRIOR", [2] = "PALADIN", [3] = "HUNTER", [4] = "ROGUE", [5] = "PRIEST", [6] = "DEATHKNIGHT",
    [7] = "SHAMAN", [8] = "MAGE", [9] = "WARLOCK", [11] = "DRUID" }
local CLASS_NAME = { [1] = "Warrior", [2] = "Paladin", [3] = "Hunter", [4] = "Rogue", [5] = "Priest", [6] = "Death Knight",
    [7] = "Shaman", [8] = "Mage", [9] = "Warlock", [11] = "Druid" }
local ROLE_LABEL = { tank = "Tank", healer = "Healer", dps = "Damage" }
local ROLE_ICON = "Interface\\LFGFrame\\UI-LFG-ICON-ROLES"
local ROLE_COORDS = { tank = { 0, 19 / 64, 22 / 64, 41 / 64 }, healer = { 20 / 64, 39 / 64, 1 / 64, 20 / 64 },
    dps = { 20 / 64, 39 / 64, 22 / 64, 41 / 64 } }
local STATUS = { follow = { "Following", 0.45, 0.9, 0.45 }, stay = { "Staying", 1, 0.82, 0.2 },
    passive = { "Passive", 0.6, 0.72, 0.95 }, dead = { "Dead", 1, 0.32, 0.32 } }
local KIND_TAG = { bot = "bot", alt = "alt bot", buddy = "buddy bot", dungeon = "dungeon bot" }
local FORMATIONS = { { "chaos", "Loose" }, { "near", "Close" }, { "melee", "Melee" }, { "line", "Line" },
    { "circle", "Circle" }, { "arrow", "Arrow" }, { "shield", "Shield" }, { "queue", "Single file" } }
local FORMATION_LABEL = {}
for _, f in ipairs(FORMATIONS) do FORMATION_LABEL[f[1]] = f[2] end
-- The game's own look (GameLook.lua; owner 12 Nov: "as you'd expect with forever ui elements not leyline
-- squarechunks"): its dialog, buttons, tabs and fonts, and its gold for headings.
local G = W.GameLook
local ACCENT = { 1, 0.82, 0 }

-- State, and the functions the tests reach (W.Companions).
local C = { bots = {}, specs = {}, names = {}, count = 0, cap = 4, raid = false, formation = "chaos", assist = false,
    cc = false, state = nil, status = "", quietUntil = 0, offset = 0, buddyOn = true, raidsOn = true,
    alts = {}, usual = {}, hooks = {}, tabs = {} }
W.Companions = C

local win, menu, picker
local build   -- the window (below)

local function now() return (GetTime and GetTime()) or 0 end
local function tidy(s) return (tostring(s or ""):gsub("|", "")) end
local function specLabel(spec)
    local s = tostring(spec or ""):gsub(" pve$", "")
    return (s:gsub("^%l", string.upper))
end

-- ---------------------------------------------------------------- talking to the realm

local function send(name, data)
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, PREFIX, MOD .. "\t" .. name .. "\t1/1\t" .. (data or ""), "WHISPER", UnitName("player"))
    return true
end

local function status(text)
    C.status = text or ""
    if win then win.status:SetText(C.status) end
end
C.SetStatus = status

-- A short answer when the window is closed (an order from the orders bar or a key): the screen's own error line,
-- not the chat.
function C.Notice(text)
    status(text)
    if win and win:IsShown() then return end
    if UIErrorsFrame and UIErrorsFrame.AddMessage then UIErrorsFrame:AddMessage(text, ACCENT[1], ACCENT[2], ACCENT[3], 1, 3) end
end

function C.Ask()
    C.askedAt = now()
    return send("roster", "")
end

-- Every order: the bots' whispered answers stay hidden for a moment; the realm's answer lands in the window.
local function act(name, data, note, fromBar)
    C.quietUntil = math.max(C.quietUntil, now() + QUIET_ORDER)
    C.waiting, C.actedAt = true, now()
    C.lastMsg, C.fromBar = nil, fromBar and now() or nil
    status(note or "Asking the realm...")
    return send(name, data)
end

function C.Order(what, target, fromBar) return act("order", what .. " " .. (target or "all"), nil, fromBar) end
function C.Formation(f) return act("formation", f) end
function C.Prepare() return act("prepare", "", "Getting your bots ready...") end
function C.Fill(kind, picks)
    return act("fill", kind .. ((picks and picks ~= "") and ("\t" .. picks) or ""), "Asking for bots...")
end
function C.Usual() return act("usual", "", "Bringing your usual party...") end
function C.Dismiss(name) return act("dismiss", name or "all") end
function C.SetSpec(name, spec) return act("spec", name .. "\t" .. spec) end
function C.SetRole(name, role) return act("role", name .. " " .. role) end
function C.Gear(name) return act("gear", name) end
function C.AskAlts() return send("alts", "") end
function C.Alt(name) return act("alt", name, "Bringing " .. tidy(name) .. "...") end
function C.Save() return act("save", "", "Saving your usual party...") end
function C.Buddy(arg) return act("buddy", arg or "any", arg == "off" and "Sending your buddy home..." or "Asking for a buddy...") end
function C.Dungeon(id, picks)
    return act("dungeon", id .. ((picks and picks ~= "") and ("\t" .. picks) or ""), "Getting your party ready...")
end
-- From the orders bar or a key: the answer shows on screen (C.Notice) rather than in the chat when the window is shut.
function C.BarOrder(what)
    if (what == "attack" or what == "pull") and UnitExists and not UnitExists("target") then
        return C.Notice("Target something first.")
    end
    return C.Order(what, "all", true)
end

-- ---------------------------------------------------------------- reading the realm's answers

local function split(line)
    local f = {}
    for v in (tostring(line or "") .. "\t"):gmatch("([^\t]*)\t") do f[#f + 1] = v end
    return f
end

function C.ParseRoster(data)
    local bots, specs, names = {}, {}, {}
    local head
    for line in tostring(data or ""):gmatch("[^\n]+") do
        local f = split(line)
        if f[1] == "H" then
            head = f
        elseif f[1] == "B" and #bots < 25 then
            local cls = tonumber(f[3])
            local b = { name = tidy(f[2]):sub(1, 24), cls = cls, role = ROLE_LABEL[f[4]] and f[4] or "dps",
                kind = KIND_TAG[f[5]] and f[5] or "bot", spec = tidy(f[6]):sub(1, 32), ilvl = tonumber(f[7]) or 0,
                status = STATUS[f[8]] and f[8] or "follow", level = tonumber(f[9]) or 0 }
            if b.name ~= "" and cls then bots[#bots + 1] = b; names[b.name] = true end
        elseif f[1] == "S" then
            local cls, list = tonumber(f[2]), {}
            for spec, role in tostring(f[3] or ""):gmatch("([%a ]+)=(%a+)") do
                list[#list + 1] = { spec = spec, role = ROLE_LABEL[role] and role or "dps" }
            end
            if cls then specs[cls] = list end
        end
    end
    if not head then return false end
    -- bots that just joined chatter while they learn spells and gear up
    for n in pairs(names) do
        if not C.names[n] then C.quietUntil = math.max(C.quietUntil, now() + QUIET_JOIN) end
    end
    C.bots, C.names = bots, names
    for cls, list in pairs(specs) do C.specs[cls] = list end
    C.count, C.cap = tonumber(head[2]) or #bots, tonumber(head[3]) or 4
    C.raid = head[4] == "1"
    C.formation = FORMATION_LABEL[head[5] or ""] and head[5] or "chaos"
    C.assist, C.cc = head[6] == "1", head[7] == "1"
    C.state = head[8] or "on"
    C.buddyOn, C.raidsOn = head[9] ~= "0", head[10] ~= "0"
    C.askedAt = nil
    if C.offset > math.max(0, #bots - ROWS) then C.offset = math.max(0, #bots - ROWS) end
    return true
end

-- The Your alts tab's list: A\t<name>\t<level>\t<class>\t<with you>\t<online> and U\t<name>.
function C.ParseAlts(data)
    local alts, usual, seen = {}, {}, {}
    for line in tostring(data or ""):gmatch("[^\n]+") do
        local f = split(line)
        if f[1] == "A" and #alts < 20 then
            local nm = tidy(f[2]):sub(1, 24)
            if nm ~= "" and not seen[nm] then
                seen[nm] = true
                alts[#alts + 1] = { name = nm, level = tonumber(f[3]) or 0, cls = tonumber(f[4]), with = f[5] == "1",
                    online = f[6] == "1" }
            end
        elseif f[1] == "U" and #usual < 10 then
            usual[#usual + 1] = tidy(f[2]):sub(1, 24)
        end
    end
    C.alts, C.usual = alts, usual
    return true
end

-- Other files (BotBar.lua, the tabs) hear about every new roster here.
function C.OnRoster(fn) C.hooks[#C.hooks + 1] = fn end
local function fireHooks()
    for _, fn in ipairs(C.hooks) do pcall(fn) end
end

-- ---------------------------------------------------------------- the window's key and the welcome

C.DEFAULT_KEY = "CTRL-B"

-- The window's key as the player reads it ("Ctrl-B"), or "" when it has none.
function C.WindowKey()
    if not GetBindingKey then return "" end
    local ok, k = pcall(GetBindingKey, "WUBOTS_WINDOW")
    if not ok or type(k) ~= "string" or k == "" then return "" end
    return (k:gsub("CTRL%-", "Ctrl-"):gsub("SHIFT%-", "Shift-"):gsub("ALT%-", "Alt-"))
end

-- Once per account: the window gets Ctrl-B when that key is free and the window has no key (not in combat: the
-- client refuses binding changes then; the next roster tries again).
function C.OfferKey()
    if type(WrathUnleashedUIDB) ~= "table" then WrathUnleashedUIDB = {} end
    local d = WrathUnleashedUIDB
    if d.botsKeyOffered or not (SetBinding and GetBindingAction) then return false end
    if InCombatLockdown and InCombatLockdown() then return false end
    d.botsKeyOffered = true
    if C.WindowKey() ~= "" then return false end
    local ok, action = pcall(GetBindingAction, C.DEFAULT_KEY)
    if not ok or (action or "") ~= "" then return false end
    if not pcall(SetBinding, C.DEFAULT_KEY, "WUBOTS_WINDOW") then return false end
    if SaveBindings then pcall(SaveBindings, (GetCurrentBindingSet and GetCurrentBindingSet()) or 1) end
    return true
end

-- How to change your bots, in plain words, with the key the player really has.
function C.HowTo()
    local key = C.WindowKey()
    if key ~= "" then return "Change them in the Bots window: press " .. key .. " or click the Bots button by the minimap." end
    return "Change them in the Bots window: click the Bots button by the minimap."
end

function C.Welcome(text)
    C.OfferKey()
    if C.ShowMinimapButton then C.ShowMinimapButton() end
    local line = TAG .. tidy(text):sub(1, 220) .. " " .. C.HowTo()
    if DEFAULT_CHAT_FRAME then DEFAULT_CHAT_FRAME:AddMessage(line) end
    local key = C.WindowKey()
    if UIErrorsFrame and UIErrorsFrame.AddMessage then
        UIErrorsFrame:AddMessage(key ~= "" and ("Your bots are here. Bots window: " .. key) or "Your bots are here.",
            ACCENT[1], ACCENT[2], ACCENT[3], 1, 5)
    end
    C.lastWelcome = line
    return line
end

function C.Dispatch(name, data)
    if name == "welcome" then return C.Welcome(data) end
    if name == "roster" then
        if not C.ParseRoster(data) then return end
        if C.state == "on" then C.OfferKey() end
        if C.state == "on" and C.ShowMinimapButton then C.ShowMinimapButton() end
        if menu and menu:IsShown() and not C.names[menu.botName or ""] then menu:Hide() end
        C.Refresh()
        fireHooks()
    elseif name == "alts" then
        C.ParseAlts(data)
        C.Refresh()
    elseif name == "msg" then
        local text = tidy(data):sub(1, 200)
        C.waiting = nil
        -- two answers in a row (a fill, then the trip) both stay readable
        local shown = text
        if C.lastMsg and C.lastMsgAt and now() - C.lastMsgAt < 3 then shown = C.lastMsg .. "\n" .. text end
        C.lastMsg, C.lastMsgAt = text, now()
        if win and win:IsShown() then
            status(shown)
        elseif C.fromBar and now() - C.fromBar < 4 then
            C.Notice(text)
        elseif DEFAULT_CHAT_FRAME then
            DEFAULT_CHAT_FRAME:AddMessage(TAG .. text)
        end
    end
end

function C.OnMessage(prefix, msg, channel, sender)
    if prefix ~= PREFIX or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, i, n, data = msg:match("^([^\t]+)\t([^\t]+)\t(%d+)/(%d+)\t(.*)$")
    if mod ~= MOD then return end
    i, n = tonumber(i), tonumber(n)
    if n < 1 or n > 64 or i < 1 or i > n then return end
    C.parts = C.parts or {}
    if n == 1 then C.parts[name] = nil; return C.Dispatch(name, data) end
    local buf = C.parts[name]
    if i == 1 or not buf or buf.n ~= n then buf = { n = n, got = 0 }; C.parts[name] = buf end
    if not buf[i] then buf[i] = data; buf.got = buf.got + 1 end
    if buf.got == n then
        C.parts[name] = nil
        C.Dispatch(name, table.concat(buf, "", 1, n))
    end
end

-- ---------------------------------------------------------------- helpers the window and the tests use

function C.Find(name) for _, b in ipairs(C.bots) do if b.name == name then return b end end end

-- The roles a class can take with the realm's specs: { tank = "bear pve", healer = "resto pve", dps = ... }
function C.RolesFor(cls)
    local out = {}
    for _, s in ipairs(C.specs[cls] or {}) do if not out[s.role] then out[s.role] = s.spec end end
    return out
end

-- The unit token of a bot in your party or raid, for live health and mana.
function C.UnitFor(name)
    if not UnitName then return nil end
    if GetNumRaidMembers and (GetNumRaidMembers() or 0) > 0 then
        for i = 1, 40 do local u = "raid" .. i; if UnitName(u) == name then return u end end
    end
    for i = 1, 4 do local u = "party" .. i; if UnitName(u) == name then return u end end
    return nil
end

local function db()
    if type(WrathUnleashedUIDB) ~= "table" then WrathUnleashedUIDB = {} end
    return WrathUnleashedUIDB
end
function C.HideChatter() return not db().botsChatter end
function C.SetHideChatter(on) db().botsChatter = (not on) or nil end

-- Chat filter: true hides the line. Only your own bots (the roster's names), only achievements and whispers.
function C.ChatFilter(_, event, msg, sender)
    local who = tostring(sender or ""):match("^([^%-]+)") or ""
    if not C.names[who] or not C.HideChatter() then return false end
    if event == "CHAT_MSG_ACHIEVEMENT" or event == "CHAT_MSG_GUILD_ACHIEVEMENT" then return true end
    if event == "CHAT_MSG_WHISPER" or event == "CHAT_MSG_WHISPER_INFORM" then return now() < C.quietUntil end
    return false
end

-- ---------------------------------------------------------------- widgets

local function label(parent, text, font, ...)
    local fs = parent:CreateFontString(nil, "OVERLAY", font or "GameFontHighlight")
    fs:SetPoint(...)
    fs:SetJustifyH("LEFT")
    fs:SetText(text)
    return fs
end

local function button(parent, text, w, onClick, ...)
    return G.Button(parent, text, w, onClick, ...)
end

local function fill(frame, r, g, b, a, layer)
    local t = frame:CreateTexture(nil, layer or "BACKGROUND")
    t:SetTexture(r, g, b, a)
    t:SetAllPoints(frame)
    return t
end

-- A window or popup in the game's dialog dress (GameLook.lua): Forever UI's dialog frame, else the stock dialog.
local function panel(parent, name, w, h)
    local f = CreateFrame("Frame", name, parent)
    f:SetWidth(w); f:SetHeight(h)
    G.Dialog(f)
    return f
end

-- A heading in the game's gold (GameFontNormal), as in its options panels.
local function section(parent, text, y)
    return label(parent, text, "GameFontNormal", "TOPLEFT", 20, y)
end

local function bar(parent, w, h, ...)
    return G.StatusBar(parent, w, h, ...)
end

local function classColour(cls)
    local c = RAID_CLASS_COLORS and RAID_CLASS_COLORS[CLASS_TOKEN[cls] or ""]
    if c then return c.r, c.g, c.b end
    return 1, 1, 1
end

local function hex(r, g, b)
    local function c(v) return math.max(0, math.min(255, math.floor((tonumber(v) or 1) * 255 + 0.5))) end
    return string.format("|cff%02x%02x%02x", c(r), c(g), c(b))
end

-- What the other files build with (CompanionsTabs.lua, BotBar.lua): one look everywhere.
C.UI = { label = label, button = button, fill = fill, panel = panel, section = section, bar = bar, look = G,
    classColour = classColour, hex = hex, now = now, tidy = tidy, specLabel = specLabel, db = db,
    ACCENT = ACCENT, CLASS_TOKEN = CLASS_TOKEN, CLASS_NAME = CLASS_NAME, ROLE_LABEL = ROLE_LABEL,
    ROLE_ICON = ROLE_ICON, ROLE_COORDS = ROLE_COORDS, KIND_TAG = KIND_TAG, WIN_W = WIN_W }

-- ---------------------------------------------------------------- the per-bot menu

-- The game's own right-click menu (UIDropDownMenu, as a party member's portrait menu): the bot's name as the title,
-- its level and spec, the orders, Spec and Role as sub-menus, Gear up, Send home and Cancel. A client without
-- UIDropDownMenu gets the same choices as a small dialog with panel buttons (below).
local dropdown
function C.MenuMode()
    if UIDropDownMenu_Initialize and UIDropDownMenu_AddButton and ToggleDropDownMenu then return "dropdown" end
    return "panel"
end

local function menuItems(b, level, value)
    local own = b.kind ~= "alt"
    local items = {}
    local function add(t) items[#items + 1] = t end
    local name = b.name
    if level == 2 and value == "spec" then
        for _, s in ipairs(C.specs[b.cls] or {}) do
            add({ text = specLabel(s.spec), checked = s.spec == b.spec, disabled = (not own) or s.spec == b.spec,
                func = function() C.SetSpec(name, s.spec) end })
        end
        return items
    elseif level == 2 and value == "role" then
        local roles = C.RolesFor(b.cls)
        for _, r in ipairs({ "tank", "healer", "dps" }) do
            add({ text = ROLE_LABEL[r], checked = r == b.role, disabled = (not own) or (not roles[r]) or r == b.role,
                func = function() C.SetRole(name, r) end })
        end
        return items
    end
    local r, g, bl = classColour(b.cls)
    add({ text = hex(r, g, bl) .. name .. "|r  |cff9d9d9d" .. KIND_TAG[b.kind] .. "|r", isTitle = true, notCheckable = true })
    add({ text = string.format("Level %d %s, %s", b.level, CLASS_NAME[b.cls] or "", b.spec ~= "" and specLabel(b.spec)
        or ROLE_LABEL[b.role]), disabled = true, notCheckable = true })
    add({ text = "Follow", notCheckable = true, func = function() C.Order("follow", name) end })
    add({ text = "Stay", notCheckable = true, func = function() C.Order("stay", name) end })
    add({ text = "Attack", notCheckable = true, func = function()
        if UnitExists and not UnitExists("target") then return status("Target something first.") end
        C.Order("attack", name)
    end })
    local passive = b.status == "passive"
    add({ text = passive and "Fight" or "Passive", notCheckable = true,
        func = function() C.Order(passive and "active" or "passive", name) end })
    add({ text = "Spec", value = "spec", hasArrow = true, notCheckable = true, disabled = not own })
    add({ text = "Role", value = "role", hasArrow = true, notCheckable = true, disabled = not own })
    add({ text = "Gear up", notCheckable = true, disabled = not own, func = function() C.Gear(name) end })
    if not own then add({ text = "Your own character keeps its own talents and gear.", disabled = true, notCheckable = true }) end
    add({ text = "Send home", notCheckable = true, func = function() C.Dismiss(name) end })
    add({ text = rawget(_G, "CANCEL") or "Cancel", notCheckable = true, func = function() end })
    return items
end
C.MenuItems = menuItems

local function dropdownInit(self, level)
    local b = C.Find(self.botName or "")
    if not b then return end
    for _, info in ipairs(menuItems(b, level or 1, rawget(_G, "UIDROPDOWNMENU_MENU_VALUE"))) do
        UIDropDownMenu_AddButton(info, level)
    end
end

local MENU_W = 250
local function buildMenu()
    menu = panel(UIParent, "WrathUnleashedBotMenu", MENU_W, 300)
    menu:SetFrameStrata("FULLSCREEN_DIALOG")
    menu:EnableMouse(true)
    menu:SetClampedToScreen(true)
    menu.title = label(menu, "", "GameFontNormal", "TOPLEFT", 20, -18)
    menu.sub = label(menu, "", "GameFontDisableSmall", "TOPLEFT", 20, -34)
    menu.close = G.Close(menu)
    menu.specHead = section(menu, "Spec", -52)
    menu.specBtns = {}
    for i = 1, 6 do
        local col, row = (i - 1) % 2, math.floor((i - 1) / 2)
        local b = button(menu, "", 102, function(self)
            if menu.botName and self.spec then C.SetSpec(menu.botName, self.spec); menu:Hide() end
        end, "TOPLEFT", 20 + col * 106, -72 - row * 24)
        menu.specBtns[i] = b
    end
    menu.roleHead = section(menu, "Role", -148)
    menu.roleBtns = {}
    for i, r in ipairs({ "tank", "healer", "dps" }) do
        local b = button(menu, ROLE_LABEL[r], 68, function()
            if menu.botName then C.SetRole(menu.botName, r); menu:Hide() end
        end, "TOPLEFT", 20 + (i - 1) * 70, -168)
        b.role = r
        menu.roleBtns[i] = b
    end
    menu.note = label(menu, "", "GameFontDisableSmall", "TOPLEFT", 20, -196)
    menu.note:SetWidth(MENU_W - 40)
    section(menu, "Orders", -212)
    menu.follow = button(menu, "Follow", 68, function() C.Order("follow", menu.botName); menu:Hide() end, "TOPLEFT", 20, -232)
    menu.stay = button(menu, "Stay", 68, function() C.Order("stay", menu.botName); menu:Hide() end, "TOPLEFT", 90, -232)
    menu.attack = button(menu, "Attack", 72, function()
        if UnitExists and not UnitExists("target") then status("Target something first."); return menu:Hide() end
        C.Order("attack", menu.botName); menu:Hide()
    end, "TOPLEFT", 160, -232)
    menu.passive = button(menu, "Passive", 68, function(self)
        C.Order(self.what or "passive", menu.botName); menu:Hide()
    end, "TOPLEFT", 20, -258)
    menu.gear = button(menu, "Gear up", 68, function() C.Gear(menu.botName); menu:Hide() end, "TOPLEFT", 90, -258)
    menu.dismiss = button(menu, "Send home", 72, function() C.Dismiss(menu.botName); menu:Hide() end, "TOPLEFT", 160, -258)
    tinsert(UISpecialFrames, "WrathUnleashedBotMenu")
    menu:Hide()
end

function C.Dropdown() return dropdown end

function C.OpenMenu(name, anchor)
    local b = C.Find(name)
    if not b then return end
    if C.MenuMode() == "dropdown" then
        if not dropdown then
            dropdown = CreateFrame("Frame", "WrathUnleashedBotMenuDropDown", UIParent, "UIDropDownMenuTemplate")
            dropdown.gameLook = "UIDropDownMenuTemplate"
        end
        if menu then menu:Hide() end
        if CloseDropDownMenus then pcall(CloseDropDownMenus) end
        dropdown.botName = b.name
        UIDropDownMenu_Initialize(dropdown, dropdownInit, "MENU")
        ToggleDropDownMenu(1, nil, dropdown, "cursor", 0, 0)
        return dropdown
    end
    if not menu then buildMenu() end
    menu.botName = b.name
    local r, g, bl = classColour(b.cls)
    menu.title:SetText(hex(r, g, bl) .. b.name .. "|r  |cff9d9d9d" .. KIND_TAG[b.kind] .. "|r")
    menu.sub:SetText(string.format("Level %d %s, %s", b.level, CLASS_NAME[b.cls] or "", b.spec ~= "" and specLabel(b.spec) or ROLE_LABEL[b.role]))
    local own = b.kind ~= "alt"
    local specs, roles = C.specs[b.cls] or {}, C.RolesFor(b.cls)
    for i, sb in ipairs(menu.specBtns) do
        local s = specs[i]
        sb.spec = s and s.spec
        if s then
            sb:SetText(specLabel(s.spec))
            G.Select(sb, s.spec == b.spec)
            sb:Show()
            if own and s.spec ~= b.spec then sb:Enable() else sb:Disable() end
        else
            sb:Hide()
        end
    end
    for _, rb in ipairs(menu.roleBtns) do
        rb:SetText(ROLE_LABEL[rb.role])
        G.Select(rb, rb.role == b.role)
        if own and roles[rb.role] and rb.role ~= b.role then rb:Enable() else rb:Disable() end
    end
    if own then menu.gear:Enable() else menu.gear:Disable() end
    menu.passive.what = b.status == "passive" and "active" or "passive"
    menu.passive:SetText(b.status == "passive" and "Fight" or "Passive")
    menu.note:SetText(own and "" or "Your own character keeps its own talents and gear.")
    menu:ClearAllPoints()
    if anchor then menu:SetPoint("TOPLEFT", anchor, "TOPRIGHT", 8, 0) else menu:SetPoint("CENTER") end
    menu:Show()
    return menu
end

-- ---------------------------------------------------------------- the formation picker

local function buildPicker()
    picker = panel(UIParent, "WrathUnleashedBotFormations", 150, 44 + #FORMATIONS * 22)
    picker:SetFrameStrata("FULLSCREEN_DIALOG")
    picker:EnableMouse(true)
    label(picker, "Formation", "GameFontNormalSmall", "TOPLEFT", 18, -16)
    picker.btns = {}
    for i, f in ipairs(FORMATIONS) do
        picker.btns[i] = button(picker, f[2], 114, function() C.Formation(f[1]); picker:Hide() end, "TOPLEFT", 18, -32 - (i - 1) * 22)
        picker.btns[i].key = f[1]
    end
    tinsert(UISpecialFrames, "WrathUnleashedBotFormations")
    picker:Hide()
end

function C.OpenFormations(anchor)
    if not picker then buildPicker() end
    for _, b in ipairs(picker.btns) do b:SetText((b.key == C.formation and "|cff7fff7f" or "") .. FORMATION_LABEL[b.key]) end
    picker:ClearAllPoints()
    if anchor then picker:SetPoint("BOTTOMLEFT", anchor, "TOPLEFT", 0, 4) else picker:SetPoint("CENTER") end
    picker:Show()
end

-- ---------------------------------------------------------------- the window

local function buildRow(i)
    local row = CreateFrame("Button", nil, win.page)
    row:SetWidth(WIN_W - 40); row:SetHeight(ROW_H - 2)
    row:SetPoint("TOPLEFT", 20, -78 - (i - 1) * ROW_H)
    row.hover = G.RowHighlight(row)
    row.role = row:CreateTexture(nil, "OVERLAY")
    row.role:SetWidth(20); row.role:SetHeight(20)
    row.role:SetPoint("LEFT", 6, 0)
    row.role:SetTexture(ROLE_ICON)
    row.name = label(row, "", "GameFontNormal", "TOPLEFT", 34, -3)
    row.name:SetWidth(200)
    row.info = label(row, "", "GameFontHighlightSmall", "TOPLEFT", 34, -19)
    row.info:SetWidth(200)
    row.hp = bar(row, 120, 8, "TOPLEFT", 252, -7)
    row.hp:SetStatusBarColor(0.2, 0.8, 0.3)
    row.mp = bar(row, 120, 5, "TOPLEFT", 252, -20)
    row.state = label(row, "", "GameFontHighlightSmall", "RIGHT", -8, 0)
    row.state:SetJustifyH("RIGHT")
    row:RegisterForClicks("LeftButtonUp", "RightButtonUp")
    row:SetScript("OnClick", function(self) if self.bot then C.OpenMenu(self.bot.name, self) end end)
    row:SetScript("OnEnter", function(self)
        self.hover:Show()
        if GameTooltip and self.bot then
            GameTooltip:SetOwner(self, "ANCHOR_RIGHT")
            GameTooltip:SetText(self.bot.name .. " (" .. KIND_TAG[self.bot.kind] .. ")")
            GameTooltip:AddLine("Click for spec, role, gear and orders.", 1, 1, 1)
            GameTooltip:Show()
        end
    end)
    row:SetScript("OnLeave", function(self) self.hover:Hide(); if GameTooltip then GameTooltip:Hide() end end)
    row:Hide()
    return row
end

-- live health and mana from the unit functions
local function updateBars()
    if not win then return end
    for _, row in ipairs(win.rows) do
        local b = row.bot
        if b then
            local u = C.UnitFor(b.name)
            local hp, hpMax = u and UnitHealth and UnitHealth(u), u and UnitHealthMax and UnitHealthMax(u)
            local mp, mpMax = u and UnitPower and UnitPower(u), u and UnitPowerMax and UnitPowerMax(u)
            row.hp:SetValue((hp and hpMax and hpMax > 0) and hp / hpMax or 0)
            row.mp:SetValue((mp and mpMax and mpMax > 0) and mp / mpMax or 0)
            local _, token = nil, nil
            if u and UnitPowerType then _, token = UnitPowerType(u) end
            local pc = PowerBarColor and PowerBarColor[token or "MANA"]
            if pc then row.mp:SetStatusBarColor(pc.r, pc.g, pc.b) else row.mp:SetStatusBarColor(0.25, 0.45, 1) end
            local dead = u and UnitIsDeadOrGhost and UnitIsDeadOrGhost(u)
            local st = STATUS[dead and "dead" or b.status]
            row.state:SetText(st[1]); row.state:SetTextColor(st[2], st[3], st[4])
        end
    end
end
C.UpdateBars = updateBars

function C.Refresh()
    if not win then return end
    local n = #C.bots
    if C.state == "soon" then
        win.sub:SetText("Bots are coming soon to this realm.")
    elseif C.state == "off" then
        win.sub:SetText("Bots are off on this realm.")
    else
        win.sub:SetText(string.format("%d of %d bots in your %s", n, C.cap, C.raid and "raid" or "party"))
    end
    for i, row in ipairs(win.rows) do
        local b = C.bots[i + C.offset]
        row.bot = b
        if b then
            local r, g, bl = classColour(b.cls)
            row.name:SetText(hex(r, g, bl) .. b.name .. "|r  |cff9d9d9d" .. KIND_TAG[b.kind] .. "|r")
            local spec = b.spec ~= "" and specLabel(b.spec) or ROLE_LABEL[b.role]
            row.info:SetText(string.format("%s %s, item level %d, level %d", spec, CLASS_NAME[b.cls] or "", b.ilvl, b.level))
            local c = ROLE_COORDS[b.role] or ROLE_COORDS.dps
            row.role:SetTexCoord(c[1], c[2], c[3], c[4])
            row:Show()
        else
            row:Hide()
        end
    end
    win.empty:SetText(n == 0 and (C.state == "on" or C.state == nil)
        and "No bots with you. Fill your group below, or bring your usual party." or "")
    win.more:SetText(n > ROWS and string.format("%d-%d of %d (scroll)", C.offset + 1, math.min(n, C.offset + ROWS), n) or "")
    local allPassive = n > 0
    for _, b in ipairs(C.bots) do if b.status ~= "passive" then allPassive = false end end
    win.passive:SetText(allPassive and "Fight" or "Passive")
    win.passive.what = allPassive and "active" or "passive"
    win.assist:SetText((C.assist and "|cff7fff7f" or "") .. "Assist tank")
    win.cc:SetText((C.cc and "|cff7fff7f" or "") .. "Crowd control")
    win.formation:SetText("Formation: " .. FORMATION_LABEL[C.formation])
    local has = n > 0
    for _, b in ipairs(win.needBots) do if has then b:Enable() else b:Disable() end end
    if win.fillRaid then if C.raidsOn then win.fillRaid:Enable() else win.fillRaid:Disable() end end
    if win.quiet then win.quiet:SetChecked(C.HideChatter()) end
    updateBars()
    local t = C.tabs[C.tab or 1]
    if t and t.page and t.refresh and t.key ~= "bots" then pcall(t.refresh, t.page) end
end

-- Tabs: CompanionsTabs.lua adds Party, Raid, Your alts, Buddy and Dungeons with C.AddTab before the window is
-- first built. build(page) makes the tab's page once; refresh(page) runs on show and on each new roster.
function C.AddTab(key, text, w, buildFn, refreshFn, onShow)
    for _, t in ipairs(C.tabs) do if t.key == key then return t end end
    local t = { key = key, text = text, w = w or 80, build = buildFn, refresh = refreshFn, onShow = onShow }
    C.tabs[#C.tabs + 1] = t
    return t
end

function C.ShowTab(key)
    if not win then build() end
    local at = 1
    for i, t in ipairs(C.tabs) do if t.key == key or i == key then at = i end end
    C.tab = at
    for i, t in ipairs(C.tabs) do
        if i == at then t.page:Show() else t.page:Hide() end
    end
    G.SelectTab(win, at, #C.tabs, C.tabs)
    local t = C.tabs[at]
    if t.onShow then pcall(t.onShow, t.page) end
    C.Refresh()
    return t.page
end
function C.Tab() local t = C.tabs[C.tab or 1]; return t and t.key end

local function newPage()
    local page = CreateFrame("Frame", nil, win)
    page:SetPoint("TOPLEFT", 0, -PAGE_TOP); page:SetPoint("BOTTOMRIGHT", 0, 0)
    page:Hide()
    return page
end

-- The Your bots tab: the roster, the orders, the group buttons (the window as it always was).
local function buildBots(page)
    win.page = page
    win.sub = label(page, "", "GameFontHighlightSmall", "TOPLEFT", 22, -40)
    win.refresh = button(page, "Refresh", 76, function() C.Ask() end, "TOPRIGHT", -20, -34)

    section(page, "Your bots", -60)
    win.rows = {}
    for i = 1, ROWS do win.rows[i] = buildRow(i) end
    page:EnableMouseWheel(true)
    page:SetScript("OnMouseWheel", function(_, delta)
        C.offset = math.max(0, math.min(math.max(0, #C.bots - ROWS), C.offset - delta)); C.Refresh()
    end)
    local listBottom = -78 - ROWS * ROW_H
    win.empty = label(page, "", "GameFontDisable", "TOP", 0, -140)
    win.more = label(page, "", "GameFontDisableSmall", "TOPRIGHT", -24, listBottom + 2)

    -- orders for everyone
    local y = listBottom - 10
    section(page, "Orders", y)
    win.needBots = {}
    local function order(text, w, x, yy, fn)
        local b = button(page, text, w, fn, "TOPLEFT", x, yy)
        win.needBots[#win.needBots + 1] = b
        return b
    end
    order("Follow", 76, 20, y - 22, function() C.Order("follow", "all") end)
    order("Stay", 76, 98, y - 22, function() C.Order("stay", "all") end)
    order("Attack my target", 128, 176, y - 22, function()
        if UnitExists and not UnitExists("target") then return status("Target something first.") end
        C.Order("attack", "all")
    end)
    win.passive = order("Passive", 76, 306, y - 22, function(self) C.Order(self.what or "passive", "all") end)
    win.assist = order("Assist tank", 110, 384, y - 22, function() C.Order(C.assist and "noassist" or "assist", "all") end)
    win.cc = order("Crowd control", 128, 20, y - 48, function() C.Order(C.cc and "nocc" or "cc", "all") end)
    win.formation = order("Formation: Loose", 150, 150, y - 48, function(self) C.OpenFormations(self) end)
    win.prepare = order("Prepare for raid", 142, 302, y - 48, function() C.Prepare() end)

    -- the group
    y = y - 82
    section(page, "Group", y)
    button(page, "Fill party", 96, function() C.Fill("party", C.Picks and C.Picks()) end, "TOPLEFT", 20, y - 22)
    win.fillRaid = button(page, "Fill raid", 96, function() C.Fill("raid") end, "TOPLEFT", 118, y - 22)
    button(page, "My usual party", 120, function() C.Usual() end, "TOPLEFT", 216, y - 22)
    order("Dismiss all", 110, 338, y - 22, function() C.Dismiss("all") end)
end
C.AddTab("bots", "Your bots", 84, buildBots)

build = function()
    win = panel(UIParent, "WrathUnleashedBotsFrame", WIN_W, WIN_H)
    win:SetPoint("CENTER")
    win:SetFrameStrata("DIALOG")
    win:EnableMouse(true)
    win:SetMovable(true)
    win:RegisterForDrag("LeftButton")
    win:SetScript("OnDragStart", win.StartMoving)
    win:SetScript("OnDragStop", win.StopMovingOrSizing)
    tinsert(UISpecialFrames, "WrathUnleashedBotsFrame")   -- Esc closes it

    win.title = G.Window(win, "Bots", "Interface\\Icons\\INV_Misc_GroupNeedMore")
    win.close = G.Close(win)

    -- the game's bottom tabs (CharacterFrameTabButtonTemplate), each tab's page in the window
    for i, t in ipairs(C.tabs) do
        t.btn = G.Tab(win, i, t.text, function() C.ShowTab(i) end)
        t.btn.tabKey = t.key
        t.page = newPage()
        local ok, err = pcall(t.build, t.page)
        if not ok and DEFAULT_CHAT_FRAME then DEFAULT_CHAT_FRAME:AddMessage(TAG .. "a tab failed to load: " .. tostring(err)) end
    end
    win.tabs = C.tabs

    -- footer: chatter switch and the realm's answers (every tab)
    local quiet = CreateFrame("CheckButton", nil, win, "UICheckButtonTemplate")
    quiet:SetWidth(22); quiet:SetHeight(22)
    quiet:SetPoint("BOTTOMLEFT", 18, 38)
    quiet:SetScript("OnClick", function(self) C.SetHideChatter(self:GetChecked() and true or false) end)
    win.quiet = quiet
    label(win, "Hide bot chatter (their achievements, and whispers right after an order)", "GameFontHighlightSmall",
        "LEFT", quiet, "RIGHT", 2, 0)
    win.status = label(win, C.status, "GameFontHighlightSmall", "BOTTOMLEFT", 22, 14)
    win.status:SetWidth(WIN_W - 44)

    win:SetScript("OnUpdate", function(self, elapsed)
        self.tick = (self.tick or 0) + (elapsed or 0)
        if self.tick < 0.25 then return end
        self.tick = 0
        updateBars()
        if C.waiting and C.actedAt and now() - C.actedAt > ANSWER_WAIT then
            C.waiting = nil
            status("No answer. Bots may not be on this realm.")
        end
    end)
    win:SetScript("OnShow", function()
        if PlaySound then pcall(PlaySound, "igCharacterInfoOpen") end
        C.Ask()
        C.ShowTab(C.tab or 1)
    end)
    win:SetScript("OnHide", function()
        if menu then menu:Hide() end
        if picker then picker:Hide() end
        if C.HideChooser then C.HideChooser() end
    end)
    win:Hide()
end

function C.Window() return win end
function C.Toggle(tab)
    if not win then build() end
    if tab then
        if not win:IsShown() then C.tab = nil; for i, t in ipairs(C.tabs) do if t.key == tab then C.tab = i end end; win:Show()
        else C.ShowTab(tab) end
        return
    end
    if win:IsShown() then win:Hide() else win:Show() end
end
W.ToggleBots = C.Toggle

-- ---------------------------------------------------------------- the minimap button

function C.PlaceMinimapButton(b)
    local angle = math.rad(tonumber(db().botsAngle) or 200)
    b:ClearAllPoints()
    b:SetPoint("CENTER", Minimap, "CENTER", math.cos(angle) * 80, math.sin(angle) * 80)
end

function C.ShowMinimapButton()
    if not Minimap or db().botsNoMinimap then return end
    local b = C.mini
    if not b then
        b = CreateFrame("Button", "WrathUnleashedBotsMinimapButton", Minimap)
        b:SetWidth(31); b:SetHeight(31)
        b:SetFrameStrata("MEDIUM"); b:SetFrameLevel(8)
        b:SetHighlightTexture("Interface\\Minimap\\UI-Minimap-ZoomButton-Highlight")
        local icon = b:CreateTexture(nil, "BACKGROUND")
        icon:SetTexture("Interface\\Icons\\INV_Misc_GroupNeedMore")
        icon:SetWidth(20); icon:SetHeight(20)
        icon:SetPoint("TOPLEFT", 7, -5)
        local border = b:CreateTexture(nil, "OVERLAY")
        border:SetTexture("Interface\\Minimap\\MiniMap-TrackingBorder")
        border:SetWidth(53); border:SetHeight(53)
        border:SetPoint("TOPLEFT")
        b:RegisterForDrag("LeftButton")
        b:SetScript("OnClick", function() C.Toggle() end)
        b:SetScript("OnDragStart", function(self)
            self:SetScript("OnUpdate", function()
                if not (GetCursorPosition and Minimap.GetCenter) then return end
                local mx, my = Minimap:GetCenter()
                local cx, cy = GetCursorPosition()
                local s = Minimap:GetEffectiveScale()
                if not (mx and cx and s and s > 0) then return end
                db().botsAngle = math.floor(math.deg(math.atan2(cy / s - my, cx / s - mx)) + 0.5)
                C.PlaceMinimapButton(self)
            end)
        end)
        b:SetScript("OnDragStop", function(self) self:SetScript("OnUpdate", nil) end)
        b:SetScript("OnEnter", function(self)
            if not GameTooltip then return end
            GameTooltip:SetOwner(self, "ANCHOR_LEFT")
            GameTooltip:SetText("Bots")
            GameTooltip:AddLine("Your bots and your orders. /comp", 1, 1, 1)
            GameTooltip:Show()
        end)
        b:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
        C.mini = b
    end
    C.PlaceMinimapButton(b)
    b:Show()
end

-- ---------------------------------------------------------------- events, slash, chat filters

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:RegisterEvent("PLAYER_ENTERING_WORLD")
listener:RegisterEvent("PARTY_MEMBERS_CHANGED")
listener:RegisterEvent("RAID_ROSTER_UPDATE")
listener:SetScript("OnEvent", function(_, event, prefix, msg, channel, sender)
    if event == "CHAT_MSG_ADDON" then return C.OnMessage(prefix, msg, channel, sender) end
    -- the group changed: ask for the roster a moment later (one ask for a burst of changes)
    C.askAt = now() + (event == "PLAYER_ENTERING_WORLD" and 3 or ASK_DEBOUNCE)
end)
listener:SetScript("OnUpdate", function()
    if C.askAt and now() >= C.askAt then C.askAt = nil; C.Ask() end
end)
C.listener = listener

-- The bot module's "configured with N bots" login line counts only its roaming bots, never the ones players bring
-- (and reads "0 bots" on a realm full of them): hidden. Its credit line (the module's source link) stays, as its
-- licence asks.
function C.SystemFilter(_, _, msg)
    msg = tostring(msg or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", "")
    return msg:find("^Playerbots: The server is configured with %d+ bots%.?$") ~= nil
        -- the balancing module's own login line (seen in game 31 Oct): noise for players
        or msg:find("^This server is running the AutoBalance module%.?$") ~= nil
end

if ChatFrame_AddMessageEventFilter then
    pcall(ChatFrame_AddMessageEventFilter, "CHAT_MSG_SYSTEM", C.SystemFilter)
    for _, e in ipairs({ "CHAT_MSG_ACHIEVEMENT", "CHAT_MSG_GUILD_ACHIEVEMENT", "CHAT_MSG_WHISPER", "CHAT_MSG_WHISPER_INFORM" }) do
        pcall(ChatFrame_AddMessageEventFilter, e, C.ChatFilter)
    end
end

-- /comp opens the window; /comp party|raid|alts|buddy|dungeons opens that tab; /comp bar and /comp orders switch the
-- bar of your bots and the orders bar on or off; /comp tips shows the first-time tips again (BotBar.lua).
local SLASH_TABS = { party = "party", raid = "raid", alts = "alts", alt = "alts", buddy = "buddy", dungeons = "dungeons",
    dungeon = "dungeons", bots = "bots" }
SLASH_WUCOMPANIONS1, SLASH_WUCOMPANIONS2 = "/companions", "/comp"
SlashCmdList["WUCOMPANIONS"] = function(msg)
    local word = tostring(msg or ""):lower():match("^%s*(%S*)")
    if SLASH_TABS[word] then return C.Toggle(SLASH_TABS[word]) end
    if word ~= "" and C.Slash and C.Slash(word) then return end
    C.Toggle()
end
