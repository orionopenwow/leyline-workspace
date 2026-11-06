-- Boss fights (3.3.5a API): Leyline's instant boss fights, in the editor's look (Style.lua S.Ed*: near-black, gold for
-- what is picked). The featured fight is the last fight of the citadel, shown to players as "The Frozen Throne" (its
-- boss is never named on screen). Three parts:
--   the window   /bf, /bossfights, Esc > Boss fights (Leyline realms), the phone tray's Menu > Boss fights
--                (stream/www-files/touch.js): the fight, Normal / Heroic, "Go now with companions" (bots fill the
--                raid at once), "Queue with players" (+ "Players only": no bots fill the seats)
--   the bar      a small strip at the top middle while you are queued, starting or fighting: what is happening, the
--                fight's live timer (mm:ss.t, run here between the realm's ticks) and Leave. Under the unit frames'
--                row, so it covers neither them, the buffs nor the minimap. DenLayout.lua's slots are the Den
--                editor's (nothing is docked outside the Den); in the Den the bar sits under the strip and the
--                toasts; with touch controls on, under touch.js's player and target frames
--   the card     the result: a clear (the time, the hint, the board's address) or a wipe (Try again / Leave)
--
-- Every button is one realm command typed as chat (".bug" in ModsPanel.lua does the same), never run here:
--   .bossfight join <boss> <normal|heroic> <bots|group|group-only>   .bossfight leave   .bossfight status
-- The realm answers with addon messages, prefix LYBF, whispered to yourself, tab-separated:
--   Q <boss> <diff> <mode> <waiting> <needed> <eta_s>   queued (only the bots' ETA is ever shown, no other numbers)
--   S <boss> <diff>                                     starting (teleporting)
--   T <ms>                                              the fight's time (at the pull, then every 5 s)
--   R <clear|wipe> <ms> <rank_hint|->                   the result
--   L                                                   left / cancelled
-- After /reload (or any loading screen) the addon asks ".bossfight status" and the realm's answer puts the bar back.
-- Your last pick is kept per character (WrathUnleashedUIChar.bossfights), so Try again works after a reload too.
-- Nothing from the realm is run; texts are shown plain.

local W = WrathUnleashedUI
local BF = { state = "idle", boss = "frozen-throne", diff = "normal", mode = "bots", playersOnly = false,
    queue = nil, timer = nil, result = nil, askedAt = nil }
W.BossFights = BF

BF.PREFIX = "LYBF"
BF.BOARD = "leylinelauncher.com/wrath/bossfights"
BF.BOSSES = { { id = "frozen-throne", name = "The Frozen Throne",
    -- TODO(owner): a line about the fight in your own words (shown as the boss tile's tip)
    tip = "The last fight at the top of the citadel: a ten-player raid fight" } }
BF.DIFFS = { normal = "Normal", heroic = "Heroic" }
BF.MODES = { bots = true, group = true, ["group-only"] = true }
BF.ASK_WAIT = 8        -- seconds for the realm to answer a join before the bar gives up
BF.TOUCH_PX = 44       -- the smallest control on a phone, in CSS pixels
BF.CTRL = 36           -- the smallest control here, in UI units (chips, inputs, the close button)

local win, bar, card, menuButton

local function S() return W.Style end
local function now() return (GetTime and tonumber((GetTime()))) or 0 end
-- no colour codes, no control characters, no game-brand words in a text from the realm
local BANNED = { "world of warcraft", "warcraft", "lich king", "blizzard", "%f[%a]wow%f[%A]" }
function BF.Clean(s, n)
    s = tostring(s or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("|", ""):gsub("%c", " ")
    local low = s:lower()
    for _, b in ipairs(BANNED) do if low:find(b) then return "" end end
    return s:sub(1, n or 60)
end
function BF.BossName(id)
    for _, b in ipairs(BF.BOSSES) do if b.id == id then return b.name end end
    return "Boss fight"
end
function BF.DiffName(d) return BF.DIFFS[d] or "Normal" end

-- mm:ss.t
function BF.Format(ms)
    ms = math.max(0, math.floor((tonumber(ms) or 0) + 0.5))
    local tenths = math.floor(ms / 100)
    return string.format("%02d:%02d.%d", math.floor(tenths / 600), math.floor((tenths % 600) / 10), tenths % 10)
end
-- the fight's time now: the realm's last tick plus the time since (stopped at the result)
function BF.Elapsed()
    local t = BF.timer
    if not t then return 0 end
    if not t.running then return t.ms end
    return t.ms + math.max(0, now() - t.at) * 1000
end
function BF.Eta()
    local q = BF.queue
    if not q then return 0 end
    return math.max(0, math.ceil(q.eta - (now() - q.at)))
end

local function char()
    WrathUnleashedUIChar = type(WrathUnleashedUIChar) == "table" and WrathUnleashedUIChar or {}
    local c = WrathUnleashedUIChar
    if type(c.bossfights) ~= "table" then c.bossfights = {} end
    return c.bossfights
end
local function remember()
    local c = char()
    c.boss, c.diff, c.mode, c.playersOnly = BF.boss, BF.diff, BF.mode, BF.playersOnly and true or nil
end
local function restore()
    local c = char()
    if c.boss then BF.boss = tostring(c.boss) end
    if BF.DIFFS[c.diff or ""] then BF.diff = c.diff end
    if BF.MODES[c.mode or ""] then BF.mode = c.mode end
    BF.playersOnly = c.playersOnly == true
end

-- ---------------------------------------------------------------- talking to the realm
function BF.Command(text)
    if not SendChatMessage then return false end
    pcall(SendChatMessage, ".bossfight " .. text, "SAY")
    return true
end
function BF.Join(boss, diff, mode)
    boss = boss or BF.boss; diff = diff or BF.diff; mode = mode or BF.mode
    if not BF.DIFFS[diff] or not BF.MODES[mode] or not tostring(boss):match("^[%w%-]+$") then return false end
    BF.boss, BF.diff, BF.mode = boss, diff, mode
    remember()
    BF.state = "asking"
    BF.askedAt = now()
    BF.result = nil
    BF.Command("join " .. boss .. " " .. diff .. " " .. mode)
    if win then win:Hide() end
    if card then card:Hide() end
    BF.Paint()
    return true
end
function BF.GoNow() return BF.Join(BF.boss, BF.diff, "bots") end
function BF.QueueUp() return BF.Join(BF.boss, BF.diff, BF.playersOnly and "group-only" or "group") end
function BF.TryAgain() return BF.Join(BF.boss, BF.diff, BF.mode) end
function BF.Leave()
    BF.Command("leave")
    return true
end
function BF.Status()
    BF.Command("status")
    return true
end

-- ---------------------------------------------------------------- the realm's answers
local function split(msg)
    local out = {}
    for f in (msg .. "\t"):gmatch("([^\t]*)\t") do out[#out + 1] = f; if #out > 8 then break end end
    return out
end
local function bossId(s) s = tostring(s or ""); if s:match("^[%w%-]+$") and #s <= 32 then return s end end

function BF.OnMessage(prefix, msg, channel, sender)
    if prefix ~= BF.PREFIX or type(msg) ~= "string" then return end
    if channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local f = split(msg)
    local kind = f[1]
    if kind == "Q" then
        local boss, diff, mode = bossId(f[2]), f[3], f[4]
        if not (boss and BF.DIFFS[diff] and BF.MODES[mode]) then return end
        BF.boss, BF.diff, BF.mode = boss, diff, mode
        BF.state = "queued"
        BF.queue = { eta = math.max(0, math.min(3600, tonumber(f[7] or "") or 0)), at = now() }
        BF.timer, BF.result = nil, nil
    elseif kind == "S" then
        local boss, diff = bossId(f[2]), f[3]
        if not (boss and BF.DIFFS[diff]) then return end
        BF.boss, BF.diff = boss, diff
        BF.state = "starting"
        BF.queue, BF.timer, BF.result = nil, nil, nil
    elseif kind == "T" then
        local ms = tonumber(f[2] or "")
        if not ms or ms < 0 then return end
        BF.state = "engaged"
        BF.queue, BF.result = nil, nil
        BF.timer = { ms = ms, at = now(), running = true }
    elseif kind == "R" then
        local how, ms = f[2], tonumber(f[3] or "")
        if (how ~= "clear" and how ~= "wipe") or not ms or ms < 0 then return end
        local hint = BF.Clean(f[4], 48)
        if hint == "-" then hint = "" end
        BF.state = how
        BF.queue = nil
        BF.timer = { ms = ms, at = now(), running = false }
        BF.result = { how = how, ms = ms, hint = hint }
        BF.ShowCard()
    elseif kind == "L" then
        BF.state = "idle"
        BF.queue, BF.timer, BF.result = nil, nil, nil
        if card then card:Hide() end
    else
        return
    end
    BF.askedAt = nil
    remember()
    BF.Paint()
end

-- ---------------------------------------------------------------- touch: big enough for a thumb
-- touch.js sets __OWTOUCH.applied while its controls are on and __OWTOUCH.cssH, the canvas's CSS height: UIParent's
-- height over it is UI units per CSS pixel. Off: nil.
function BF.TouchUnits()
    local T = rawget(_G, "__OWTOUCH")
    if type(T) ~= "table" or not T.applied then return nil end
    local css = tonumber(T.cssH)
    local h = UIParent and tonumber((UIParent:GetHeight()))
    if not (css and css > 0 and h and h > 0) then return nil end
    return h / css
end
-- a frame's scale: 1 on a computer; with touch on, its smallest control (minH units) at least 44 CSS px, unless that
-- would make the frame wider than the screen
function BF.ScaleFor(minH, width)
    local u = BF.TouchUnits()
    if not u then return 1 end
    local s = math.max(1, BF.TOUCH_PX * u / (minH or BF.CTRL))
    local sw = UIParent and tonumber((UIParent:GetWidth()))
    if width and sw and sw > 0 then s = math.min(s, (sw - 16) / width) end
    return s
end

-- ---------------------------------------------------------------- the window
local PAD, GAP = 12, 8
local WIN_W = 340

local function chip(parent, text, w, onClick, tip)
    local b = S().EdChip(parent, text, w, onClick)
    b:SetHeight(BF.CTRL)
    b.edLabel = tip
    return b
end

local function buildWindow()
    win = CreateFrame("Frame", "LeylineBossFights", UIParent)
    win:SetWidth(WIN_W); win:SetHeight(280)
    win:SetPoint("CENTER", UIParent, "CENTER", 0, 40)
    win:SetFrameStrata("DIALOG")
    win:EnableMouse(true)
    S().EdPanel(win, 0.94)
    win.head = S().EdLabel(win, "Boss fights", true, "TOPLEFT", PAD, -14)
    win.close = chip(win, "x", BF.CTRL, function() BF.Show(false) end, "Close")
    win.close:SetPoint("TOPRIGHT", win, "TOPRIGHT", -6, -6)
    local inner = WIN_W - 2 * PAD
    local y = -6 - BF.CTRL - GAP
    -- the fight (one for now: a tile per fight, the picked one gold)
    win.bosses = {}
    for _, b in ipairs(BF.BOSSES) do
        local t = chip(win, b.name, inner, function() BF.Pick(b.id) end, b.tip)
        t:SetHeight(40)
        t:SetPoint("TOPLEFT", win, "TOPLEFT", PAD, y)
        t.boss = b.id
        win.bosses[#win.bosses + 1] = t
        y = y - 40 - GAP
    end
    -- Normal / Heroic
    local half = (inner - GAP) / 2
    win.normal = chip(win, "Normal", half, function() BF.SetDiff("normal") end, "The fight as it was made")
    win.normal:SetPoint("TOPLEFT", win, "TOPLEFT", PAD, y)
    win.heroic = chip(win, "Heroic", half, function() BF.SetDiff("heroic") end, "Harder: more health, harder hits, a faster clock")
    win.heroic:SetPoint("TOPLEFT", win, "TOPLEFT", PAD + half + GAP, y)
    y = y - BF.CTRL - GAP
    -- go
    win.go = chip(win, "Go now with companions", inner, function() BF.GoNow() end,
        "In at once: your companions fill the raid")
    win.go:SetHeight(40)
    win.go:SetPoint("TOPLEFT", win, "TOPLEFT", PAD, y)
    y = y - 40 - GAP
    local qw = math.floor(inner * 0.58)
    win.queue = chip(win, "Queue with players", qw, function() BF.QueueUp() end,
        "Wait for other players; bots fill the seats still empty after a short wait")
    win.queue:SetPoint("TOPLEFT", win, "TOPLEFT", PAD, y)
    win.only = chip(win, "Players only", inner - qw - GAP, function() BF.SetPlayersOnly(not BF.playersOnly) end,
        "Wait for a whole group of players: no bots")
    win.only:SetPoint("TOPLEFT", win, "TOPLEFT", PAD + qw + GAP, y)
    y = y - BF.CTRL - GAP
    win.status = S().EdText(win, S().ED_SIZE.small)
    win.status:SetPoint("TOPLEFT", win, "TOPLEFT", PAD, y - 2)
    win.status:SetWidth(inner); win.status:SetJustifyH("LEFT")
    local m = S().ED.muted
    win.status:SetTextColor(m[1], m[2], m[3])
    win:SetHeight(-y + 16 + PAD)
    win:Hide()
    if UISpecialFrames then tinsert(UISpecialFrames, "LeylineBossFights") end
end

function BF.Pick(id) if bossId(id) then BF.boss = id; remember() end BF.Paint(); return BF.boss end
function BF.SetDiff(d) if BF.DIFFS[d] then BF.diff = d; remember() end BF.Paint(); return BF.diff end
function BF.SetPlayersOnly(on) BF.playersOnly = on and true or false; remember(); BF.Paint(); return BF.playersOnly end

function BF.Window() if not win then buildWindow() end return win end
function BF.IsOpen() return win ~= nil and win:IsShown() and true or false end
function BF.Show(on)
    BF.Window()
    if on then
        win:SetScale(BF.ScaleFor(BF.CTRL, WIN_W))
        win:Show()
    else
        win:Hide()
    end
    BF.Paint()
    return BF.IsOpen()
end
function BF.Toggle() return BF.Show(not BF.IsOpen()) end

-- ---------------------------------------------------------------- the bar
local BAR_W, BAR_H = 420, 48
local function buildBar()
    bar = CreateFrame("Frame", "LeylineBossFightBar", UIParent)
    bar:SetWidth(BAR_W); bar:SetHeight(BAR_H)
    bar:SetFrameStrata("MEDIUM")
    bar:EnableMouse(true)
    S().EdPanel(bar, 0.88)
    bar.title = S().EdText(bar, S().ED_SIZE.small)
    bar.title:SetPoint("TOPLEFT", bar, "TOPLEFT", 10, -7)
    bar.title:SetWidth(BAR_W - 170); bar.title:SetJustifyH("LEFT")
    local g = S().ED.gold
    bar.title:SetTextColor(g[1], g[2], g[3])
    bar.line = S().EdText(bar, S().ED_SIZE.small)
    bar.line:SetPoint("TOPLEFT", bar, "TOPLEFT", 10, -25)
    bar.line:SetWidth(BAR_W - 86); bar.line:SetJustifyH("LEFT")
    if bar.line.SetWordWrap then bar.line:SetWordWrap(false) end
    bar.clock = S().EdText(bar, 18)
    bar.clock:SetPoint("RIGHT", bar, "RIGHT", -78, 0)
    bar.clock:SetJustifyH("RIGHT")
    bar.leave = chip(bar, "Leave", 64, function() BF.Leave() end, "Leave the boss fight (or the queue)")
    bar.leave:SetPoint("RIGHT", bar, "RIGHT", -6, 0)
    local acc = 0
    bar:SetScript("OnUpdate", function(self, elapsed)
        acc = acc + (tonumber(elapsed) or 0)
        if acc < 0.1 then return end
        acc = 0
        BF.Tick()
    end)
    bar:Hide()
end

-- where the bar goes: the top middle, just under the unit frames' row (clear of the buffs and the minimap, far from the
-- raid frames on the left and the boss frames on the right); in the Den under its strip and toasts; with touch under
-- touch.js's unit frames
function BF.PlaceBar()
    if not bar then return end
    local s = BF.ScaleFor(BF.CTRL, BAR_W)
    bar:SetScale(s)
    -- under the row of the player frame, the target frame, the buffs and the minimap's top (104 high at the top)
    local y = -108
    local u = BF.TouchUnits()
    local L, D = W.DenLayout, W.DenBar
    if u then
        y = -(12 * u + 112)
    elseif D and D.active and L then
        y = math.min(y, -(L.BAR_TOP + L.BAR_H + L.GAP) - (L.StackHeight and L.StackHeight("toast") or 0))
    end
    bar:ClearAllPoints()
    bar:SetPoint("TOP", UIParent, "TOP", 0, y / s)
end
function BF.Bar() if not bar then buildBar() end return bar end

-- the bar's words for each state (the queue shows the bots' ETA only: never how many are waiting or needed)
function BF.BarText()
    local st = BF.state
    if st == "asking" then return "Asking the realm..." end
    if st == "queued" then
        if BF.mode == "bots" then return "Your companions are getting ready..." end
        if BF.mode == "group-only" then return "Waiting for players..." end
        local eta = BF.Eta()
        if eta <= 0 then return "Waiting for players... bots fill the rest any moment" end
        return string.format("Waiting for players... bots fill the rest in ~%ds", eta)
    end
    if st == "starting" then return "Starting: on your way in..." end
    if st == "engaged" then return "Fighting" end
    if st == "clear" then return "Cleared" end
    if st == "wipe" then return "Wiped: try again or leave" end
    return ""
end

function BF.Tick()
    if BF.state == "asking" and BF.askedAt and now() - BF.askedAt > BF.ASK_WAIT then
        BF.state = "idle"
        BF.askedAt = nil
        if W.Say then W.Say("Boss fights: no answer from the realm. Try again in a moment.") end
        BF.Paint()
        return
    end
    if not bar or not bar:IsShown() then return end
    -- touch controls turned on or off while the bar is up: its size and place again
    if math.abs(BF.ScaleFor(BF.CTRL, BAR_W) - (tonumber((bar:GetScale())) or 1)) > 0.01 then BF.PlaceBar() end
    bar.line:SetText(BF.BarText())
    -- the line runs up to Leave, or up to the clock while there is one
    if BF.timer then bar.clock:SetText(BF.Format(BF.Elapsed())); bar.line:SetWidth(BAR_W - 170)
    else bar.clock:SetText(""); bar.line:SetWidth(BAR_W - 86) end
end

-- ---------------------------------------------------------------- the result card
local CARD_W = 360
local function buildCard()
    card = CreateFrame("Frame", "LeylineBossFightResult", UIParent)
    card:SetWidth(CARD_W); card:SetHeight(250)
    card:SetFrameStrata("DIALOG")
    card:EnableMouse(true)
    S().EdPanel(card, 0.95)
    local inner = CARD_W - 2 * PAD
    card.head = S().EdLabel(card, "Cleared", true, "TOP", 0, -16)
    card.boss = S().EdText(card, S().ED_SIZE.text)
    card.boss:SetPoint("TOP", card, "TOP", 0, -40)
    card.time = S().EdText(card, 30)
    card.time:SetPoint("TOP", card, "TOP", 0, -64)
    card.hint = S().EdText(card, S().ED_SIZE.small)
    card.hint:SetPoint("TOP", card, "TOP", 0, -106)
    card.board = S().EdText(card, S().ED_SIZE.small)
    card.board:SetPoint("TOP", card, "TOP", 0, -128)
    card.board:SetWidth(inner)
    card.link = S().EdInput(card, inner, BF.CTRL)
    card.link:SetHeight(BF.CTRL)
    card.link:SetPoint("TOP", card, "TOP", 0, -150)
    card.link:SetText("https://" .. BF.BOARD)
    card.link:SetScript("OnEditFocusGained", function(self) if self.HighlightText then self:HighlightText() end end)
    card.link:SetScript("OnTextChanged", function(self, user) if user then self:SetText("https://" .. BF.BOARD) end end)
    card.link.edLabel = "Copy the board's address"
    local half = (inner - GAP) / 2
    card.again = chip(card, "Try again", half, function() BF.TryAgain() end, "Again from the start, here in the instance")
    card.again:SetPoint("BOTTOMLEFT", card, "BOTTOMLEFT", PAD, PAD)
    card.leave = chip(card, "Leave", half, function() if card then card:Hide() end BF.Leave() end, "Leave the boss fight")
    card.leave:SetPoint("BOTTOMRIGHT", card, "BOTTOMRIGHT", -PAD, PAD)
    card.done = chip(card, "Done", half, function() card:Hide() end, "Close this card")
    card.done:SetPoint("BOTTOMLEFT", card, "BOTTOMLEFT", PAD, PAD)
    card:Hide()
    if UISpecialFrames then tinsert(UISpecialFrames, "LeylineBossFightResult") end
end
function BF.Card() if not card then buildCard() end return card end
function BF.ShowCard()
    BF.Card()
    local r = BF.result
    if not r then card:Hide(); return end
    local clear = r.how == "clear"
    card.head:SetText((clear and "Cleared" or "Wiped"):upper())
    card.boss:SetText(BF.BossName(BF.boss) .. " - " .. BF.DiffName(BF.diff))
    card.time:SetText(BF.Format(r.ms))
    card.hint:SetText(r.hint or "")
    if clear then
        card.board:SetText("See the board: " .. BF.BOARD)
        card.board:Show(); card.link:Show(); card.done:Show(); card.again:Hide()
        card:SetHeight(250)
    else
        card.board:SetText("")
        card.board:Hide(); card.link:Hide(); card.done:Hide(); card.again:Show()
        card:SetHeight(176)
    end
    card.leave:Show()
    local s = BF.ScaleFor(BF.CTRL, CARD_W)
    card:SetScale(s)
    card:ClearAllPoints()
    card:SetPoint("CENTER", UIParent, "CENTER", 0, 60 / s)
    card:Show()
end

-- ---------------------------------------------------------------- painting
function BF.Busy() return BF.state == "asking" or BF.state == "queued" or BF.state == "starting" or BF.state == "engaged" end
function BF.Paint()
    -- the bar: from asking to the result (a wipe or a clear keeps it, with Leave, until you leave)
    if BF.state ~= "idle" then
        BF.Bar()
        bar.title:SetText(BF.BossName(BF.boss) .. " - " .. BF.DiffName(BF.diff))
        BF.PlaceBar()
        bar:Show()
        BF.Tick()
    elseif bar then
        bar:Hide()
    end
    if menuButton then if W.LeylineRealm then menuButton:Show() else menuButton:Hide() end end
    if not win then return end
    for _, t in ipairs(win.bosses) do t:SetActive(t.boss == BF.boss) end
    win.normal:SetActive(BF.diff == "normal")
    win.heroic:SetActive(BF.diff == "heroic")
    win.only:SetActive(BF.playersOnly)
    win.only:SetText(BF.playersOnly and "Players only: on" or "Players only")
    local on = W.LeylineRealm and not BF.Busy()
    for _, b in ipairs({ win.go, win.queue }) do if on then b:Enable() else b:Disable() end end
    if not W.LeylineRealm then
        win.status:SetText("Boss fights are on Leyline realms.")
    elseif BF.Busy() then
        win.status:SetText("You're in a boss fight: Leave is on the bar at the top.")
    elseif BF.playersOnly then
        win.status:SetText("Players only: you wait until a whole group is in.")
    else
        win.status:SetText("Your best clear goes on the board.")
    end
end

-- ---------------------------------------------------------------- Esc > Boss fights
local function menuRefresh()
    if not GameMenuFrame then return end
    if not menuButton then
        menuButton = CreateFrame("Button", "GameMenuButtonLeylineBossFights", GameMenuFrame, "GameMenuButtonTemplate")
        menuButton:SetText("Boss fights")
        menuButton:SetScript("OnClick", function()
            if HideUIPanel then HideUIPanel(GameMenuFrame) end
            BF.Show(true)
        end)
    end
    if W.LeylineRealm then menuButton:Show() else menuButton:Hide() end
end
function BF.MenuButton() menuRefresh(); return menuButton end
if GameMenuFrame and GameMenuFrame.HookScript then GameMenuFrame:HookScript("OnShow", function() menuRefresh(); if W.LayoutGameMenu then W.LayoutGameMenu() end end) end

-- ---------------------------------------------------------------- events, /bf
restore()
local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:RegisterEvent("PLAYER_ENTERING_WORLD")
local asked = false
listener:SetScript("OnEvent", function(self, event, prefix, msg, channel, sender)
    if event == "CHAT_MSG_ADDON" then return BF.OnMessage(prefix, msg, channel, sender) end
    if event == "PLAYER_ENTERING_WORLD" then
        -- the first time (login, /reload): once the realm says it is a Leyline realm; after a loading screen: at once
        if not asked then
            asked = true
            if W.WhenLeyline then W.WhenLeyline(function() menuRefresh(); BF.Status() end) end
        elseif W.LeylineRealm then
            BF.Status()
        end
        if bar and bar:IsShown() then BF.PlaceBar() end
    end
end)

SLASH_LEYLINEBOSSFIGHTS1, SLASH_LEYLINEBOSSFIGHTS2 = "/bf", "/bossfights"
SlashCmdList = SlashCmdList or {}
SlashCmdList.LEYLINEBOSSFIGHTS = function(msg)
    local word = tostring(msg or ""):lower():match("^%s*(%S*)")
    if word == "leave" then return BF.Leave() end
    if word == "status" then return BF.Status() end
    if word == "go" then return BF.GoNow() end
    if word == "queue" then return BF.QueueUp() end
    return BF.Toggle()
end
