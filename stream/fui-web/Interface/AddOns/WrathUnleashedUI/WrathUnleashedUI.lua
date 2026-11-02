-- WrathUnleashed UI: the browser realm's defaults (3.3.5a API).
--  1. Every action bar on: bottom left, bottom right, right, right 2 (Esc > Interface > ActionBars).
--     Done once per character; turning a bar off afterwards is respected.
--  2. A level 80 gets its empty buttons on those bars filled from the spellbook: every active spell
--     not already on a bar, highest rank, in spellbook order. Once per character; /wubars does it again.
--     Not for a character the realm lays out itself (the main realm's boost, ov_main.lua): see "the realm's layout".
-- Nothing is removed or moved: only empty buttons are used.
-- Leyline realms only (owner 2026-10-30): every one-time default (camera, auto loot, talent preview, the bars, the
-- keys and the page keys, the level 80 fill, Shift-P in Collections.lua) and the instant quests wait for our realm's
-- answer at login (W.WhenLeyline, "the realm's hello" below). The Leyline launcher installs this addon into players'
-- own game folders too, so on any other server the windows load but nothing is rebound or changed.

local ADDON = "WrathUnleashedUI"
local BOOK = BOOKTYPE_SPELL or "spell"
-- Action slots (1-based) of the extra bars, in the order they fill: bottom left, bottom right, right, right 2.
local SLOTS = {}
for _, first in ipairs({ 61, 49, 25, 37 }) do
    for i = 0, 11 do SLOTS[#SLOTS + 1] = first + i end
end

WrathUnleashedUI = WrathUnleashedUI or {}
local W = WrathUnleashedUI

-- The Leyline brand switch (owner 2026-10-30): while the realm has it on (ov_modapi.lua OVMod.Brand, realm/brand.sh)
-- it tells this addon at login over OVMOD (mod "wubrand", name "brand", data "leyline\t<realm name>"), and the
-- windows say "Leyline" and call the main realm by the name it sends (W.OFFICIAL_NAME when it sends none). No word
-- from the realm = off: every text is the
-- old one. Only what players read changes: the addon's folder, saved variables, frame names and prefixes stay.
-- Kept in memory only (never saved), so a realm that turns it off is off at the next login.
W.Brand = { on = false, name = "WrathUnleashed", home = "Wrath", back = "Back to Wrath" }
-- The official realm's name (owner 2026-10-30 23:32: "Wrath Unleashed" for now; the realm's own copy is
-- ov_modapi.lua OVMod.OFFICIAL_NAME). Leyline.lua reads it from here.
W.OFFICIAL_NAME = "Wrath Unleashed"
local brandWatchers = {}
function W.OnBrand(fn) brandWatchers[#brandWatchers + 1] = fn end
function W.SetBrand(realm)
    realm = type(realm) == "string" and realm:gsub("|", ""):gsub("[%c]", ""):sub(1, 32) or ""
    if realm == "" then realm = W.OFFICIAL_NAME end
    W.Brand = { on = true, name = "Leyline", home = realm, back = "Back to " .. realm }
    for _, fn in ipairs(brandWatchers) do pcall(fn, W.Brand) end
end

-- ---------------------------------------------------------------- UI size
-- Owner 1 Nov 20:56, the desktop game in Your Den and on Wrath Unleashed: "the UI is completely destroyed ... the UI
-- scale, the client scale to immediately adapt to player screens and not jump all over the place". His shot
-- (1896x1071): Forever UI's bottom row (action bar, micro menu, bags, ~1440 UI units with its gryphons) at an
-- effective scale of ~1.2, wider than the screen: the bar's left end off screen and both gryphons, which Forever UI
-- keeps on screen, pushed in over the bar instead of sitting at its ends. What did it, all desktop-only:
--  * the old login line `UIParent:SetScale(1)` whenever UIParent's scale wasn't 1. In the browser that is always
--    1; on the desktop UIParent's scale IS the game's UI size (0.9 by itself above 853 px), so every login on a
--    Leyline realm made the UI bigger a moment after entering the world, and the game put its own size back at the
--    next window or scale change: the jumps;
--  * Esc > Mods & Workshop wrote its slider's value back as the player's choice whenever the page opened (SetValue
--    runs OnValueChanged), and "Modern size" computed 768 / UIParent's height, the browser's math (its UIParent is
--    the window's height in pixels): on the desktop that is always 1.0, and the slider goes to 1.2;
--  * nothing chose a size for the screen, so a 16:9 screen at 1.0 or more can't hold the row.
-- Now: one size from the real screen (W.FitScale), chosen when the addon loads, before the world is drawn,
-- through the game's own UI scale (never UIParent:SetScale: the browser draws that only partly, live 2026-10-25),
-- set only when it differs, so the game lays out once; after a window resize again, once, only if the size changes.
-- A size the player picks (the slider, or the game's own Interface options) is theirs and stays. In the browser the
-- client already sizes the UI from the window every frame (its "pixel" mode), so the addon leaves it alone there.
W.SCALE_MIN, W.SCALE_MAX = 0.5, 1.2     -- the slider's range
W.FIT_MIN, W.FIT_MAX = 0.64, 1.0         -- the automatic size: today's games' range (the game's own slider's)
W.ROW_HALF = 724                         -- half of Forever UI's bottom row with its gryphons, when it can't be measured
W.FIT_MARGIN = 8                         -- UI units kept free at each screen edge

local function scaleDB()
    WrathUnleashedUIDB = WrathUnleashedUIDB or {}
    return WrathUnleashedUIDB
end
local function cvar(name)
    if not GetCVar then return nil end
    local ok, v = pcall(GetCVar, name)
    if ok then return v end
end

-- The browser client has its own "uiScaleMode" setting (pixel / manual); the desktop game has no such setting.
function W.IsBrowser()
    local v = cvar("uiScaleMode")
    return type(v) == "string" and v ~= ""
end

-- Half the width the bottom row needs, in UI units from the screen's centre: Forever UI's measured row
-- (ForeverUI.BottomRow, every micro button included) plus the gryphons' overhang, else the stock main bar.
function W.RowHalf()
    local F = ForeverUI
    local half
    local row = type(F) == "table" and F.BottomRow
    if type(row) == "table" and tonumber(row.left) and tonumber(row.right) then
        local over = 124                                  -- a 154-unit gryphon, 30 of it over the bar
        local caps = F.ActionBarEndCaps
        if type(caps) == "table" and caps.left and caps.left.GetWidth and tonumber(caps.inset) then
            local w = caps.left:GetWidth()
            if w and w > 0 then over = w - caps.inset end
        end
        half = math.max(-row.left, row.right) + over
    elseif MainMenuBar and MainMenuBar.GetWidth then
        half = (MainMenuBar:GetWidth() or 0) / 2
    end
    if not half or half <= 0 then half = W.ROW_HALF end
    return half + W.FIT_MARGIN
end

-- The UI size for a window w x h pixels: as today's games (one UI unit per pixel, 768 / height, between 0.64 and
-- 1.0), and never so big that a row `half` UI units each side of the centre leaves the screen. Two decimals,
-- rounded down.
function W.FitScale(w, h, half)
    w, h = tonumber(w), tonumber(h)
    if not w or not h or w <= 0 or h <= 0 then return nil end
    half = tonumber(half) or (W.ROW_HALF + W.FIT_MARGIN)
    local k = 768 / h
    if k < W.FIT_MIN then k = W.FIT_MIN elseif k > W.FIT_MAX then k = W.FIT_MAX end
    local fit = 768 * (w / h) / (2 * half)               -- the screen is 768 * w / h / k units wide
    if fit < k then k = fit end
    k = math.floor(k * 100 + 1e-6) / 100
    if k < W.SCALE_MIN then k = W.SCALE_MIN end
    return k
end

-- The window in pixels: gxResolution (the browser answers its canvas's size there too) when it has the window's
-- shape (GetScreenWidth / GetScreenHeight, in UI units). A maximized window can keep an old setting: then the
-- window's shape at the height the game's own automatic size (0.9) stands for.
function W.ScreenPixels()
    local sw = GetScreenWidth and GetScreenWidth()
    local sh = GetScreenHeight and GetScreenHeight()
    local aspect = (sw and sh and sw > 0 and sh > 0) and sw / sh or nil
    local rw, rh = tostring(cvar("gxResolution") or ""):match("^(%d+)x(%d+)")
    rw, rh = tonumber(rw), tonumber(rh)
    if rw and rh and rw > 0 and rh > 0 then
        if not aspect or math.abs(rw / rh - aspect) / aspect < 0.03 then return rw, rh end
    end
    if aspect then return aspect * 853, 853 end
    return nil
end

-- The size in use now: the player's own, else the game's setting (the browser's pixel mode: 768 / its height).
function W.CurrentScale()
    local own = scaleDB().uiScaleOwn
    if type(own) == "number" then return own end
    if W.IsBrowser() and cvar("uiScaleMode") == "pixel" then
        local _, h = W.ScreenPixels()
        if h then return math.max(0.3, math.min(1, math.floor(768 / h * 100 + 0.5) / 100)) end
    end
    return tonumber(cvar("uiScale") or "") or 1
end
-- The size that fits this screen (the old name, kept for callers).
function W.ModernScale()
    local w, h = W.ScreenPixels()
    return W.FitScale(w, h, W.RowHalf()) or 1
end

local applying, lastSet = false, nil
local function setScale(k)
    if not SetCVar then return end
    applying, lastSet = true, k
    pcall(SetCVar, "useUiScale", "1")
    pcall(SetCVar, "uiScale", string.format("%.2f", k))
    applying = false
end

local fitPending = false
-- The fitting size, set only when it isn't the one already in use (desktop, the fit on, no size of the player's).
-- True when it changed something.
function W.ApplyFit()
    if W.IsBrowser() then return false end
    local db = scaleDB()
    if db.uiScaleOwn ~= nil or not db.uiFit then return false end
    if InCombatLockdown and InCombatLockdown() then fitPending = true; return false end
    fitPending = false
    local w, h = W.ScreenPixels()
    local k = W.FitScale(w, h, W.RowHalf())
    if not k then return false end
    db.uiScaleAuto = k
    local now = tonumber(cvar("uiScale") or "")
    if cvar("useUiScale") == "1" and now and math.abs(now - k) < 0.005 then return false end   -- already: nothing moves
    setScale(k)
    return true
end

-- The player's own size (the slider): kept as it is at any window size, until "Fit to screen".
function W.SetOwnScale(k)
    k = tonumber(k)
    if not k then return false end
    if InCombatLockdown and InCombatLockdown() then return false end
    if k < W.SCALE_MIN then k = W.SCALE_MIN elseif k > W.SCALE_MAX then k = W.SCALE_MAX end
    k = math.floor(k * 100 + 0.5) / 100
    scaleDB().uiScaleOwn = k
    setScale(k)
    return true
end
W.ApplyScale = W.SetOwnScale

-- "Fit to screen": back to the size from the screen (desktop), or to the browser client's own pixel mode.
function W.FitToScreen()
    local db = scaleDB()
    db.uiScaleOwn = nil
    if W.IsBrowser() then
        if SetCVar then pcall(SetCVar, "uiScaleMode", "pixel") end
        return true
    end
    db.uiFit = true
    W.ApplyFit()
    return true
end

-- A size changed in the game's own Interface options (or by a /console line) while the fit was on is the player's.
local function noticePlayer()
    if applying or W.IsBrowser() then return end
    local db = scaleDB()
    if db.uiScaleOwn ~= nil or not db.uiFit then return end
    if cvar("useUiScale") == "0" then db.uiScaleOwn = "game"; return end   -- "Use UI Scale" unticked: the game's own
    local now = tonumber(cvar("uiScale") or "")
    local mine = lastSet or db.uiScaleAuto
    if now and mine and math.abs(now - mine) >= 0.005 then db.uiScaleOwn = now end
end

local scaleFrame = CreateFrame("Frame")
for _, e in ipairs({ "ADDON_LOADED", "DISPLAY_SIZE_CHANGED", "UI_SCALE_CHANGED", "CVAR_UPDATE", "PLAYER_REGEN_ENABLED" }) do
    scaleFrame:RegisterEvent(e)
end
local function fitNextFrame()
    -- a resize sends several events: one look, a frame later
    scaleFrame:SetScript("OnUpdate", function(self)
        self:SetScript("OnUpdate", nil)
        W.ApplyFit()
    end)
end
scaleFrame:SetScript("OnEvent", function(self, event, arg1)
    if event == "ADDON_LOADED" then
        if arg1 ~= ADDON then return end
        local db = scaleDB()
        -- Before this version a size was saved whenever the Mods page opened: not a choice, so it goes.
        if (tonumber(db.uiScaleV) or 0) < 2 then db.uiScale, db.uiScaleV = nil, 2 end
        if W.IsBrowser() then return end
        if db.uiFit then
            W.ApplyFit()              -- the loading screen: before anything is drawn
        elseif db.uiScaleOwn == nil and W.WhenLeyline then
            -- the first time, on a Leyline realm only (another server's game is left as it is)
            W.WhenLeyline(function()
                local d = scaleDB()
                if d.uiScaleOwn == nil and not d.uiFit then d.uiFit = true; W.ApplyFit() end
            end)
        end
    elseif event == "DISPLAY_SIZE_CHANGED" then
        fitNextFrame()
    elseif event == "UI_SCALE_CHANGED" or event == "CVAR_UPDATE" then
        noticePlayer()
    elseif event == "PLAYER_REGEN_ENABLED" then
        if fitPending then W.ApplyFit() end
    end
end)

-- Chat lines wait a few seconds after login for the realm's brand word (W.SettleBrand), so the tag is the right one.
local held, settled = {}, false
local function say(msg)
    if not settled then held[#held + 1] = msg; return end
    if DEFAULT_CHAT_FRAME then DEFAULT_CHAT_FRAME:AddMessage("|cff4fc3f7[" .. W.Brand.name .. "]|r " .. msg) end
end
function W.SettleBrand()
    if settled then return end
    settled = true
    local list = held
    held = {}
    for _, m in ipairs(list) do say(m) end
end
W.Say = say

-- The realm's hello: the addon asks at login (OVMOD "wuhello" "ask"); a Leyline realm answers "wuhello" "hi"
-- (ov_modapi.lua). Its other answers prove it too: the brand ("wubrand" "brand") and the bars word ("wubars"
-- "layout", ov_main.lua). Only answers count, never the ask (a whisper to oneself comes back on any server). Kept for
-- this session only: the next login asks again.
W.LeylineRealm = false
local leylineWaiting = {}
-- fn now on a Leyline realm, else once its hello arrives (never on another server).
function W.WhenLeyline(fn)
    if W.LeylineRealm then pcall(fn) else leylineWaiting[#leylineWaiting + 1] = fn end
end
function W.SetLeyline()
    if W.LeylineRealm then return end
    W.LeylineRealm = true
    local list = leylineWaiting
    leylineWaiting = {}
    for _, fn in ipairs(list) do pcall(fn) end
end
local function isHello(msg)
    return msg:find("^wuhello\thi\t%d+/%d+\t") or msg:find("^wubrand\tbrand\t%d+/%d+\t") or msg:find("^wubars\tlayout\t%d+/%d+\t")
end

local BRAND_WAIT = 3   -- seconds after entering the world: no word from the realm by then = off (the ask is answered at once)
local brandFrame = CreateFrame("Frame")
brandFrame:RegisterEvent("CHAT_MSG_ADDON")
brandFrame:RegisterEvent("PLAYER_ENTERING_WORLD")
brandFrame:SetScript("OnEvent", function(self, event, prefix, msg, channel)
    if event == "PLAYER_ENTERING_WORLD" then
        self:UnregisterEvent("PLAYER_ENTERING_WORLD")
        if SendAddonMessage and UnitName then
            pcall(SendAddonMessage, "OVMOD", "wuhello\task\t1/1\t", "WHISPER", UnitName("player"))
            pcall(SendAddonMessage, "OVMOD", "wubrand\task\t1/1\t", "WHISPER", UnitName("player"))
        end
        local wait = 0
        self:SetScript("OnUpdate", function(f, elapsed)
            wait = wait + elapsed
            if wait < BRAND_WAIT then return end
            f:SetScript("OnUpdate", nil)
            W.SettleBrand()
        end)
        return
    end
    if prefix ~= "OVMOD" or channel ~= "WHISPER" or type(msg) ~= "string" then return end
    if isHello(msg) then W.SetLeyline() end
    local data = msg:match("^wubrand\tbrand\t1/1\t(.*)$")
    if not data then return end
    local kind, realm = data:match("^(%a+)\t?(.*)$")
    if kind == "leyline" then
        W.SetBrand(realm)
        W.SettleBrand()
    end
end)

local function barsOn()
    -- (also W.BarsOn for the Mods panel)
    if SetActionBarToggles then SetActionBarToggles(1, 1, 1, 1, nil) end
    SHOW_MULTI_ACTIONBAR_1, SHOW_MULTI_ACTIONBAR_2, SHOW_MULTI_ACTIONBAR_3, SHOW_MULTI_ACTIONBAR_4 = 1, 1, 1, 1
    ALWAYS_SHOW_MULTI_ACTIONBAR = nil   -- empty buttons stay hidden (shown while dragging a spell)
    if MultiActionBar_Update then MultiActionBar_Update() end
    if UIParent_ManageFramePositions then UIParent_ManageFramePositions() end
end

-- Names of the spells already on any of the 120 buttons.
local function placedNames()
    local names = {}
    for slot = 1, 120 do
        if HasAction(slot) then
            local kind, id = GetActionInfo(slot)
            if kind == "spell" and id then
                local name = GetSpellInfo(id)
                if name then names[name] = true end
            elseif kind == "macro" then
                names["macro:" .. tostring(id)] = true
            end
        end
    end
    return names
end

-- Active spells in spellbook order, one entry per name (the last listed = the highest rank).
local function bookSpells()
    local order, index = {}, {}
    local tabs = GetNumSpellTabs and GetNumSpellTabs() or 0
    for t = 1, tabs do
        local _, _, offset, count = GetSpellTabInfo(t)
        for i = offset + 1, offset + count do
            local name = GetSpellName(i, BOOK)
            if name and not IsPassiveSpell(i, BOOK) then
                if not index[name] then
                    order[#order + 1] = name
                end
                index[name] = i
            end
        end
    end
    return order, index
end

local function fillBars(quiet)
    if InCombatLockdown and InCombatLockdown() then
        if not quiet then say("Not in combat: try /wubars again after the fight.") end
        return 0
    end
    local have = placedNames()
    local order, index = bookSpells()
    local placed, s = 0, 1
    for _, name in ipairs(order) do
        if not have[name] then
            while SLOTS[s] and HasAction(SLOTS[s]) do s = s + 1 end
            local slot = SLOTS[s]
            if not slot then break end
            ClearCursor()
            PickupSpell(index[name], BOOK)
            PlaceAction(slot)
            ClearCursor()
            have[name] = true
            placed = placed + 1
            s = s + 1
        end
    end
    if not quiet then
        say(placed > 0 and (placed .. " more spells put on your action bars.") or "Your action bars already have every spell.")
    end
    return placed
end

-- Key bindings everyone knows: no reaching for 8 9 0 - =.
--   main bar         1-7, then F G T Z X for buttons 8-12
--   bottom left bar  Shift + the same keys (it sits on top of the main bar)
--   bottom right bar Ctrl + the same keys
--   right bar        Alt + the same keys
--   right bar 2      no keys (click it, or bind in Esc > Key Bindings)
-- 8 9 0 - = are cleared. Done once per character; Esc > Mods & Workshop > Default keys does it again.
local KEYS = { "1", "2", "3", "4", "5", "6", "7", "F", "G", "T", "Z", "X" }
-- The stock keys that turn the main bar to its next / previous page. Shift is the bottom left bar's key here, and
-- Shift + wheel turns a carried prop in edit mode, so a Shift held while scrolling (zooming, building) flipped the
-- main bar to an empty or other page and it looked broken ("bars are broken sometimes", 2026-10-30). Unbound with
-- the default keys, and once for characters that had them already (only while they still flip pages).
local PAGE_KEYS = { "SHIFT-UP", "SHIFT-DOWN", "SHIFT-MOUSEWHEELUP", "SHIFT-MOUSEWHEELDOWN" }
local PAGE_FLIPS = { NEXTACTIONPAGE = true, PREVIOUSACTIONPAGE = true }
local function unbindPageFlips()
    if not SetBinding or not GetBindingAction then return false end
    if InCombatLockdown and InCombatLockdown() then return false end
    local n = 0
    for _, key in ipairs(PAGE_KEYS) do
        if PAGE_FLIPS[GetBindingAction(key) or ""] then pcall(SetBinding, key); n = n + 1 end
    end
    if n > 0 and SaveBindings then pcall(SaveBindings, (GetCurrentBindingSet and GetCurrentBindingSet()) or 2) end
    return true
end

-- The main bar back on its first page (after edit mode, a mode or minigolf, which take the keys and the wheel).
-- Stance, form, vehicle and possess bars are the client's own state, not a page: untouched.
function W.MainBarHome()
    if InCombatLockdown and InCombatLockdown() then return false end
    if type(GetActionBarPage) ~= "function" or type(ChangeActionBarPage) ~= "function" then return false end
    local page = GetActionBarPage()
    if page and page ~= 1 then pcall(ChangeActionBarPage, 1); return true end
    return false
end
local BAR_KEYS = {
    { "ACTIONBUTTON", "" },
    { "MULTIACTIONBAR1BUTTON", "SHIFT-" },
    { "MULTIACTIONBAR2BUTTON", "CTRL-" },
    { "MULTIACTIONBAR3BUTTON", "ALT-" },
}
local function defaultKeys(quiet)
    if not SetBinding then return false end
    if InCombatLockdown and InCombatLockdown() then
        if not quiet then say("Not in combat: try again after the fight.") end
        return false
    end
    for _, key in ipairs({ "8", "9", "0", "-", "=" }) do pcall(SetBinding, key) end
    for _, key in ipairs(PAGE_KEYS) do pcall(SetBinding, key) end
    for _, bar in ipairs(BAR_KEYS) do
        for i, key in ipairs(KEYS) do
            -- Alt+Z is the screenshot key (hide the interface, as in the classic game), not a right-bar slot
            if bar[2] .. key ~= "ALT-Z" then pcall(SetBinding, bar[2] .. key, bar[1] .. i) end
        end
    end
    if W.ShotKey then W.ShotKey() end
    if SaveBindings then pcall(SaveBindings, (GetCurrentBindingSet and GetCurrentBindingSet()) or 2) end
    if not quiet then
        say("Keys set: main bar 1-7 and F G T Z X; the bar above it Shift + those keys; the next Ctrl +; the right bar Alt +.")
    end
    return true
end

-- The realm's layout (2026-10-30, "bars are broken sometimes"). The main realm's boost (ov_main.lua) writes a layout for
-- the class and spec a few seconds after the boost, or after the login while it is still pending, and then syncs the
-- server's copy with a talent spec switch. This addon used to fill a level 80's empty buttons 4 s after
-- PLAYER_LEVEL_UP, which the boost fires: right in that window, on the very bars the layout writes (bottom left,
-- bottom right, right), so the bars came out as the layout, a mix or spellbook order (the mount buttons gone)
-- depending on the timing, and the buttons the layout keeps free for talents were taken. Now:
--   * a jump of more than one level (a boost) never fills: the realm lays the bars out;
--   * the realm says "boost" (whisper, OVMOD "wubars" "layout") when it boosts, and answers "boost" or "none" to
--     the ask sent at login; "boost" stops any fill for the character, "none" fills (at least 4 s after login);
--   * no answer in ASK_WAIT seconds (a realm without the boost) fills as before;
--   * a fill that falls in combat waits for the end of the fight (it used to be skipped for good).
local FILL_DELAY, ASK_WAIT = 4, 20
local fill = { at = nil, since = 0, word = nil, waitCombat = false }
local lastLevel

local function char()
    WrathUnleashedUIChar = WrathUnleashedUIChar or {}
    return WrathUnleashedUIChar
end

local filler = CreateFrame("Frame")
local function runFill()
    local c = char()
    fill.at = nil
    if c.filled80 then return end
    if InCombatLockdown and InCombatLockdown() then fill.waitCombat = true; return end
    fill.waitCombat = false
    c.filled80 = true
    local n = fillBars(true)
    if n > 0 then say(n .. " more spells put on your action bars. /wubars fills new ones later.") end
end
-- in `seconds` (from now, or sooner if one is already due earlier)
local function fillIn(seconds)
    if char().filled80 then return end
    local due = fill.since + seconds
    if not fill.at or due < fill.at then fill.at = due end
end
local function leaveToRealm()
    char().filled80 = true
    fill.at, fill.waitCombat = nil, false
end
filler:SetScript("OnUpdate", function(_, elapsed)
    fill.since = fill.since + (elapsed or 0)
    if fill.at and fill.since >= fill.at then runFill() end
end)

function W.OnBarsWord(word)
    fill.word = word
    if word == "boost" then
        leaveToRealm()
    elseif word == "none" and fill.at then
        fillIn(math.max(0, FILL_DELAY - fill.since))
    end
end

local f = CreateFrame("Frame")
f:RegisterEvent("PLAYER_ENTERING_WORLD")
f:RegisterEvent("PLAYER_LEVEL_UP")
f:RegisterEvent("PLAYER_REGEN_ENABLED")
f:RegisterEvent("CHAT_MSG_ADDON")
f:SetScript("OnEvent", function(_, event, level, msg, channel)
    local c = char()
    if event == "CHAT_MSG_ADDON" then
        if level ~= "OVMOD" or channel ~= "WHISPER" or type(msg) ~= "string" then return end
        local word = msg:match("^wubars\tlayout\t1/1\t(%a+)$")
        if word then W.OnBarsWord(word) end
        return
    end
    if event == "PLAYER_REGEN_ENABLED" then
        if fill.waitCombat then runFill() end
        return
    end
    if event == "PLAYER_ENTERING_WORLD" then
        f:UnregisterEvent("PLAYER_ENTERING_WORLD")
        lastLevel = UnitLevel("player")
        W.WhenLeyline(function() W.LeylineDefaults() end)
        return
    end
    -- PLAYER_LEVEL_UP
    local lvl = tonumber(level) or UnitLevel("player")
    local jump = lastLevel and lvl and lvl - lastLevel > 1
    lastLevel = lvl
    if not W.LeylineRealm then return end   -- another server: no fill
    if not lvl or lvl < 80 or c.filled80 then return end
    if jump or fill.word == "boost" then leaveToRealm(); return end   -- a boost: the realm lays the bars out
    fillIn(FILL_DELAY)
end)

-- The plain destroy prompt (grey and white items: one Accept) becomes the typed one (DELETE) the game uses for blue
-- and better items. Cancel or Esc leaves the item on the cursor as the game does; nothing else changes.
function W.SafeDestroy()
    local d = StaticPopupDialogs
    if type(d) ~= "table" or type(d.DELETE_GOOD_ITEM) ~= "table" then return false end
    if d.DELETE_ITEM ~= d.DELETE_GOOD_ITEM then
        W.plainDestroy = W.plainDestroy or d.DELETE_ITEM
        d.DELETE_ITEM = d.DELETE_GOOD_ITEM
    end
    return true
end

-- The one-time defaults, on a Leyline realm only (W.WhenLeyline at login).
function W.LeylineDefaults()
    local c = char()
    WrathUnleashedUIDB = WrathUnleashedUIDB or {}
    -- Defaults everyone wants (once per character, so a player's own change sticks):
    -- the farthest camera (and start zoomed out), auto loot.
    if not c.defaults then
        c.defaults = true
        if SetCVar then
            pcall(SetCVar, "cameraDistanceMax", "50")
            pcall(SetCVar, "cameraDistanceMaxFactor", "4")
            pcall(SetCVar, "autoLootDefault", "1")
        end
        if CameraZoomOut then pcall(CameraZoomOut, 50) end
    end
    -- Items are never lost to one click (owner 1 Nov: a starting bow went to the world-drop prompt): every
    -- destroy asks for the word DELETE, as the game already does for blue and better items, and the action
    -- bars are locked once per character (Shift-drag still moves a button), so a click never lifts one.
    W.SafeDestroy()
    if not c.barLock then
        c.barLock = true
        if SetCVar then pcall(SetCVar, "lockActionBars", "1") end
        if LOCK_ACTIONBAR ~= nil then LOCK_ACTIONBAR = "1" end
    end
    -- Talent planner: 3.3.5's own preview mode (plan points, then Learn), on for everyone.
    if SetCVar then pcall(SetCVar, "previewTalents", "1"); pcall(SetCVar, "previewPetTalents", "1") end
    -- (No UIParent:SetScale here any more: on the desktop UIParent's scale is the game's UI size, and setting it
    -- back to 1 made the UI jump bigger after every login; the size is the game's own setting, see "UI size".)
    WrathUnleashedUIDB.scale = nil
    if not c.barsOn then
        barsOn()
        c.barsOn = true
    end
    if not c.keys then
        c.keys = defaultKeys(true) and true or nil
        if c.keys then c.pageKeys = true; say("Every action bar is on, with keys: 1-7 and F G T Z X, plus Shift, Ctrl and Alt for the bars above. Esc > Mods & Workshop to change.") end
    end
    if c.keys and not c.pageKeys then c.pageKeys = unbindPageFlips() or nil end
    if not c.shotKey and W.ShotKey then c.shotKey = W.ShotKey() or nil end
    lastLevel = UnitLevel("player")
    if lastLevel and lastLevel >= 80 and not c.filled80 then
        if SendAddonMessage and UnitName then
            pcall(SendAddonMessage, "OVMOD", "wubars\task\t1/1\t", "WHISPER", UnitName("player"))
        end
        if fill.word == "boost" then leaveToRealm()
        elseif fill.word == "none" then fillIn(FILL_DELAY)
        else fillIn(ASK_WAIT) end
    end
end

W.BarsOn, W.FillBars, W.DefaultKeys = barsOn, fillBars, defaultKeys

-- Instant quests: accept, hand in and take the reward when there is nothing to choose. Shift skips it
-- (to read the text); a reward choice and a quest that needs gold or items handed over always wait.
local q = CreateFrame("Frame")
for _, e in ipairs({ "QUEST_DETAIL", "QUEST_PROGRESS", "QUEST_COMPLETE", "QUEST_ACCEPT_CONFIRM" }) do q:RegisterEvent(e) end
q:SetScript("OnEvent", function(_, event)
    if not W.LeylineRealm then return end   -- never on another server
    if (WrathUnleashedUIDB and WrathUnleashedUIDB.questsOff) or (IsShiftKeyDown and IsShiftKeyDown()) then return end
    if event == "QUEST_DETAIL" then
        if AcceptQuest then AcceptQuest() end
    elseif event == "QUEST_ACCEPT_CONFIRM" then
        if ConfirmAcceptQuest then ConfirmAcceptQuest() end
        if StaticPopup_Hide then StaticPopup_Hide("QUEST_ACCEPT") end
    elseif event == "QUEST_PROGRESS" then
        if IsQuestCompletable and IsQuestCompletable() and (not GetQuestMoneyToGet or GetQuestMoneyToGet() == 0) then
            CompleteQuest()
        end
    elseif event == "QUEST_COMPLETE" then
        if (GetNumQuestChoices and GetNumQuestChoices() or 0) <= 1 and GetQuestReward then GetQuestReward(1) end
    end
end)

SLASH_WUBARS1 = "/wubars"
SlashCmdList["WUBARS"] = function()
    barsOn()
    fillBars(false)
end

-- ---------------------------------------------------------------- screenshot mode
-- Alt+Z does not reach the browser client, and UIParent:Hide() alone leaves no way back. /shot (or /hideui) hides
-- the interface; Esc, Alt+Z or a click on the small hint brings it back. Print Screen still takes a screenshot.
-- The catcher is not a child of UIParent, so it stays when the interface is hidden.
local shot
local function shotEnd()
    if shot then shot:Hide() end
    if not UIParent:IsShown() then UIParent:Show() end
end
function W.ScreenshotMode(on)
    if on == nil then on = UIParent:IsShown() end
    if not on then return shotEnd() end
    if not shot then
        shot = CreateFrame("Button", "WrathUnleashedShotMode", nil)
        shot:SetFrameStrata("TOOLTIP")
        shot:SetWidth(260); shot:SetHeight(18)
        shot:SetPoint("TOP", WorldFrame, "TOP", 0, -6)
        local fs = shot:CreateFontString(nil, "OVERLAY", "GameFontDisableSmall")
        fs:SetAllPoints(shot)
        fs:SetText("Screenshot mode: Esc to bring the interface back")
        shot.text = fs
        shot:EnableKeyboard(true)
        shot:SetScript("OnKeyDown", function(_, key)
            if key == "ESCAPE" or (key == "Z" and IsAltKeyDown and IsAltKeyDown()) then return shotEnd() end
            if key == "PRINTSCREEN" and Screenshot then Screenshot() end
        end)
        shot:SetScript("OnClick", shotEnd)
        -- the hint fades after 3 s so the screenshot is clean; the keys keep working
        shot:SetScript("OnUpdate", function(self, el)
            self.left = (self.left or 0) - (el or 0)
            self.text:SetAlpha(self.left > 0 and 1 or 0)
        end)
    end
    shot.left = 3
    UIParent:Hide()
    shot:Show()
end
-- Alt+Z (owner 31 Oct, the live check: it only fired the right bar's Alt+Z slot): a key binding that clicks a hidden
-- button toggling screenshot mode. Bindings work while the interface is hidden; the catcher's Alt+Z and Esc bring it back.
local shotKeyBtn = CreateFrame("Button", "WrathUnleashedShotKey", nil)
shotKeyBtn:SetScript("OnClick", function() W.ScreenshotMode() end)
function W.ShotKey()
    if not SetBindingClick then return false end
    if InCombatLockdown and InCombatLockdown() then return false end
    if not pcall(SetBindingClick, "ALT-Z", "WrathUnleashedShotKey") then return false end
    if SaveBindings then pcall(SaveBindings, (GetCurrentBindingSet and GetCurrentBindingSet()) or 2) end
    return true
end
SLASH_WUSHOT1, SLASH_WUSHOT2 = "/shot", "/hideui"
SlashCmdList["WUSHOT"] = function() W.ScreenshotMode() end

-- ---------------------------------------------------------------- Your Den: one message kept quiet
-- In Your Den (the launcher's own Den on this PC, realm "Your Den") the owner's den-test (1 Nov) showed the game's
-- "Couldn't load Blizzard_CombatLog: Banned" box over the world at login: the game refused its own combat log addon
-- (most likely a loose copy of it in the game folder's Interface\AddOns; tools/den-check names those). A Den has no
-- combat, so in the Den only that one box is closed again; anywhere else, and any other message, it stays.
W.DEN_REALM = "Your Den"
function W.InDen()
    if not GetRealmName then return false end
    local ok, name = pcall(GetRealmName)
    return ok and name == W.DEN_REALM
end
function W.DenQuiet(text)
    return type(text) == "string" and text:find("Blizzard_CombatLog", 1, true) ~= nil and W.InDen()
end
local function denQuietBox()
    local d = BasicMessageDialog
    local t = d and (d.Text or _G["BasicMessageDialogText"])
    if d and d:IsShown() and t and t.GetText and W.DenQuiet(t:GetText()) then d:Hide() end
end
-- message() shows the box (FrameXML BasicControls.xml); a secure post-hook taints nothing that calls it
if hooksecurefunc and type(message) == "function" then
    hooksecurefunc("message", function(text) if W.DenQuiet(text) then denQuietBox() end end)
end
-- and when the box was up before this addon loaded
local denQuiet = CreateFrame("Frame")
denQuiet:RegisterEvent("PLAYER_LOGIN")
denQuiet:RegisterEvent("PLAYER_ENTERING_WORLD")
denQuiet:SetScript("OnEvent", denQuietBox)
