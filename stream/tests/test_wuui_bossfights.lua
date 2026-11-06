-- texlua stream/tests/test_wuui_bossfights.lua [addon dir] [tools/den-ui dir]
-- Boss fights (BossFights.lua): the window, the bar, the result card. The whole addon in .toc order on
-- tools/den-ui/fakeui.lua, with a fake realm answering the hello (so it is a Leyline realm) and the LYBF lines:
--   * /bf and /bossfights open the window; Esc > Boss fights on a Leyline realm; Normal / Heroic, Go now, Queue,
--     Players only each type the right ".bossfight join ..." line;
--   * Q (the bots' ETA only, never the waiting / needed numbers), S, T (a live mm:ss.t between ticks), R clear (the
--     card, the time, the board), R wipe (Try again stays, Leave), L (all gone);
--   * a join with no answer gives up; lines from someone else or another prefix are ignored; colour codes and
--     game-brand words from the realm are not shown;
--   * /reload: ".bossfight status" once the realm says hello, and on each loading screen; the realm's answer puts the
--     bar and the running timer back; the last pick is kept;
--   * the bar clears the unit frames, the buffs and the minimap; touch on: every control at least 44 CSS px;
--   * no game-brand word in any text on screen.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local uidir = arg[2] or "tools/den-ui"

local pass, fail = 0, 0
local function check(name, ok, extra)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name .. (extra and ("  " .. tostring(extra)) or "")) end
end

local F
local function boot(charDB)
    WrathUnleashedUI, WrathUnleashedUIDB, WrathUnleashedUIChar = nil, { den = { introSeen = true } }, charDB
    __OWTOUCH = nil
    F = dofile(uidir .. "/fakeui.lua")
    F.setup(1352, 1081)
    F.cvars.uiScale = "0.71"
    F.server = function(msg)
        if msg:match("^wuhello\task\t") then F.toClient("wuhello\thi\t1/1\t") end
    end
    F.load(dir)
    F.fire("PLAYER_LOGIN")
    F.fire("PLAYER_ENTERING_WORLD")
    F.tick(0.2)
    return WrathUnleashedUI.BossFights
end
local function say() return F.chat[#F.chat] end
local function bf(prefix, msg, channel, sender)
    F.fire("CHAT_MSG_ADDON", prefix or "LYBF", msg, channel or "WHISPER", sender or UnitName("player"))
end
local function slash(cmd, msg)
    for k, fn in pairs(SlashCmdList) do
        local i = 1
        while _G["SLASH_" .. k .. i] do
            if _G["SLASH_" .. k .. i] == cmd then return fn(msg or "") end
            i = i + 1
        end
    end
    error("no slash command " .. cmd)
end
local function says(n)
    local out = {}
    for i = math.max(1, #F.chat - n + 1), #F.chat do out[#out + 1] = F.chat[i] end
    return out
end
local function count(pat)
    local n = 0
    for _, l in ipairs(F.chat) do if l:find(pat) then n = n + 1 end end
    return n
end

-- every text on screen now (visible FontStrings, buttons' words, inputs, tips)
local function ours(f)
    local p = f
    while p do
        local n = p.name
        if n == "LeylineBossFights" or n == "LeylineBossFightBar" or n == "LeylineBossFightResult" or n == "GameMenuButtonLeylineBossFights" then return true end
        p = p.parent
    end
end
local function visibleTexts()
    local out = {}
    for _, f in ipairs(F.created) do
        if f.IsVisible and f:IsVisible() and ours(f) then
            if type(f.text) == "string" and f.text ~= "" then out[#out + 1] = f.text end
            if type(f.edLabel) == "string" and f.edLabel ~= "" then out[#out + 1] = f.edLabel end
        end
    end
    return out
end
local BANNED = { "world of warcraft", "warcraft", "lich king", "blizzard", "%f[%a]wow%f[%A]", "icecrown" }
local allSeen = {}
local function sweep()
    for _, t in ipairs(visibleTexts()) do allSeen[#allSeen + 1] = t end
end
local function bannedIn(list)
    for _, t in ipairs(list) do
        local l = t:lower()
        for _, b in ipairs(BANNED) do if l:find(b) then return t end end
    end
end

-- ---------------------------------------------------------------- the window
local B = boot(nil)
check("loaded: W.BossFights", type(B) == "table" and B.state == "idle")
check("login: .bossfight status once the realm said hello", count("^say: %.bossfight status$") == 1, table.concat(F.chat, " | "))
slash("/bf")
local win = _G.LeylineBossFights
check("/bf opens the window", win and win:IsVisible())
sweep()
check("the window: The Frozen Throne picked, Normal picked", win.bosses[1]:GetText() == "The Frozen Throne" and win.bosses[1]:IsActive()
    and win.normal:IsActive() and not win.heroic:IsActive())
check("the window: Go now and Queue on (a Leyline realm)", win.go:IsEnabled() and win.queue:IsEnabled())
check("the window's words", win.go:GetText() == "Go now with companions" and win.queue:GetText() == "Queue with players"
    and win.only:GetText() == "Players only")
slash("/bf")
check("/bf again closes it", not win:IsVisible())
slash("/bossfights")
check("/bossfights opens it", win:IsVisible())
check("Esc closes it (UISpecialFrames)", (function() for _, n in ipairs(UISpecialFrames) do if n == "LeylineBossFights" then return true end end end)())

win.heroic:Click()
check("Heroic picks heroic", B.diff == "heroic" and win.heroic:IsActive() and not win.normal:IsActive())
win.go:Click()
check("Go now: .bossfight join frozen-throne heroic bots", say() == "say: .bossfight join frozen-throne heroic bots", say())
check("Go now closes the window, the bar asks", not win:IsVisible() and B.state == "asking")
local bar = _G.LeylineBossFightBar
check("the bar shows while asking", bar and bar:IsVisible() and bar.line:GetText() == "Asking the realm...")
sweep()

-- no answer: gives up
F.tick(5); F.tick(4)
check("no answer in 8 s: back to idle, the bar goes", B.state == "idle" and not bar:IsVisible())

-- ---------------------------------------------------------------- the queue
slash("/bf")
win.normal:Click()
win.queue:Click()
check("Queue: .bossfight join frozen-throne normal group", say() == "say: .bossfight join frozen-throne normal group", say())
bf(nil, "Q\tfrozen-throne\tnormal\tgroup\t3\t7\t30")
F.tick(0.2)
sweep()
check("Q group: Waiting for players... bots fill the rest in ~30s", bar:IsVisible()
    and bar.line:GetText() == "Waiting for players... bots fill the rest in ~30s", bar.line:GetText())
local function digitsSeen()
    for _, t in ipairs(visibleTexts()) do if t:find("%f[%d]3%f[%D]") or t:find("%f[%d]7%f[%D]") then return t end end
end
check("Q: never the waiting (3) or needed (7) numbers", digitsSeen() == nil, digitsSeen())
check("the bar's title: The Frozen Throne - Normal", bar.title:GetText() == "The Frozen Throne - Normal", bar.title:GetText())
check("the bar's Leave", bar.leave:IsVisible() and bar.leave:GetText() == "Leave")
F.tick(10.05)
check("the ETA counts down here", bar.line:GetText() == "Waiting for players... bots fill the rest in ~20s", bar.line:GetText())
F.tick(25)
check("ETA past: any moment", bar.line:GetText() == "Waiting for players... bots fill the rest any moment", bar.line:GetText())
check("no clock while queued", (bar.clock:GetText() or "") == "")
check("the window says you're in one, its buttons off", (function() slash("/bf"); local ok = not win.go:IsEnabled()
    and win.status:GetText():find("Leave is on the bar") ~= nil; slash("/bf"); return ok end)())

-- players only
bf(nil, "L")
check("L: idle, the bar goes", B.state == "idle" and not bar:IsVisible())
slash("/bf")
win.only:Click()
check("Players only on (gold)", B.playersOnly and win.only:IsActive())
sweep()
win.queue:Click()
check("Queue, players only: group-only", say() == "say: .bossfight join frozen-throne normal group-only", say())
bf(nil, "Q\tfrozen-throne\tnormal\tgroup-only\t2\t8\t0")
F.tick(0.2)
check("Q group-only: Waiting for players... (no bots line, no numbers)", bar.line:GetText() == "Waiting for players...", bar.line:GetText())
bf(nil, "Q\tfrozen-throne\tnormal\tbots\t1\t0\t0")
F.tick(0.2)
check("Q bots: your companions", bar.line:GetText() == "Your companions are getting ready...", bar.line:GetText())

-- ---------------------------------------------------------------- start, the timer
bf(nil, "S\tfrozen-throne\theroic")
F.tick(0.2)
check("S: starting, on your way in", B.state == "starting" and bar.line:GetText() == "Starting: on your way in..."
    and bar.title:GetText() == "The Frozen Throne - Heroic", bar.line:GetText())
bf(nil, "T\t0")
F.tick(0.1)
check("T 0: the clock starts", B.state == "engaged" and bar.clock:GetText() == "00:00.1", bar.clock:GetText())
F.tick(1.2); F.tick(1.2)
check("the clock runs between ticks (2.5 s)", bar.clock:GetText() == "00:02.5", bar.clock:GetText())
bf(nil, "T\t5000")
F.tick(0.1)
check("T 5000: back in step with the realm", bar.clock:GetText() == "00:05.1", bar.clock:GetText())
F.tick(60)
check("minutes: 01:05.1", bar.clock:GetText() == "01:05.1", bar.clock:GetText())
check("Format: 754321 ms = 12:34.3", B.Format(754321) == "12:34.3", B.Format(754321))
sweep()

-- ---------------------------------------------------------------- the result: a wipe
bf(nil, "R\twipe\t187400\t-")
F.tick(0.1)
local card = _G.LeylineBossFightResult
sweep()
check("R wipe: the card, Wiped, the time", card and card:IsVisible() and card.head:GetText() == "WIPED" and card.time:GetText() == "03:07.4",
    card and card.time:GetText())
check("R wipe: Try again and Leave, no board", card.again:IsVisible() and card.leave:IsVisible() and not card.done:IsVisible()
    and not card.board:IsVisible())
check("R wipe: the clock stops at the result", (function() F.tick(3); return bar.clock:GetText() == "03:07.4" end)(), bar.clock:GetText())
card.again:Click()
check("Try again: the same join (stays in the instance)", say() == "say: .bossfight join frozen-throne heroic bots" and not card:IsVisible(), say())

-- ---------------------------------------------------------------- the result: a clear
bf(nil, "T\t0")
bf(nil, "R\tclear\t412345\t|cffff0000#4 this week|r")
F.tick(0.1)
sweep()
check("R clear: Cleared, the boss, the time", card:IsVisible() and card.head:GetText() == "CLEARED" and card.time:GetText() == "06:52.3"
    and card.boss:GetText() == "The Frozen Throne - Heroic", card.time:GetText())
check("R clear: the hint, plain", card.hint:GetText() == "#4 this week", card.hint:GetText())
check("R clear: See the board", card.board:IsVisible() and card.board:GetText() == "See the board: leylinelauncher.com/wrath/bossfights"
    and card.link:GetText() == "https://leylinelauncher.com/wrath/bossfights")
check("R clear: Done and Leave, no Try again", card.done:IsVisible() and card.leave:IsVisible() and not card.again:IsVisible())
card.done:Click()
check("Done closes the card, the bar stays (Leave)", not card:IsVisible() and bar:IsVisible() and bar.line:GetText() == "Cleared")
bar.leave:Click()
check("Leave: .bossfight leave", say() == "say: .bossfight leave", say())
bf(nil, "L")
check("L after a clear: nothing left", not bar:IsVisible() and not card:IsVisible() and B.state == "idle")

-- ---------------------------------------------------------------- what is ignored
bf(nil, "T\t5000", "WHISPER", "Mallory")
check("a line from someone else: ignored", B.state == "idle")
bf("OVMOD", "T\t5000")
check("another prefix: ignored", B.state == "idle")
bf(nil, "Q\tfrozen-throne\tmythic\tgroup\t1\t1\t1")
check("a bad difficulty: ignored", B.state == "idle")
bf(nil, "Z\twhatever")
check("an unknown line: ignored", B.state == "idle")
bf(nil, "R\tclear\t1000\tWorld of Warcraft record")
check("a banned word from the realm is not shown", card.hint:GetText() == "", card.hint:GetText())
bf(nil, "L")

-- ---------------------------------------------------------------- /bf leave, /bf status
slash("/bf", "leave")
check("/bf leave", say() == "say: .bossfight leave")
slash("/bf", "status")
check("/bf status", say() == "say: .bossfight status")

-- ---------------------------------------------------------------- Esc > Boss fights
GameMenuFrame:Show()
local mb = _G.GameMenuButtonLeylineBossFights
check("Esc menu: Boss fights on a Leyline realm", mb and mb:IsShown() and mb:GetText() == "Boss fights")
mb:Click()
check("Esc > Boss fights opens the window", win:IsVisible() and not GameMenuFrame:IsShown())
win.close:Click()
check("x closes the window", not win:IsVisible())

-- ---------------------------------------------------------------- the bar's place
bf(nil, "T\t1000")
F.tick(0.1)
TargetFrame.shown = true
local L = WrathUnleashedUI.DenLayout
local br = L.Rect(bar)
local clear = true
local hits = {}
for _, n in ipairs({ "PlayerFrame", "TargetFrame", "BuffFrame", "MinimapCluster" }) do
    local o = _G[n]
    if o and L.Hit(br, L.Rect(o)) then clear = false; hits[#hits + 1] = n end
end
check("the bar clears the player and target frames, the buffs, the minimap", br and clear, table.concat(hits, ","))
check("the bar: top middle", br and math.abs((br.l + br.r) / 2 - 676) < 2 and br.t > 1081 - 140, br and (br.l .. "," .. br.t))
bf(nil, "L")

-- ---------------------------------------------------------------- /reload
local saved = WrathUnleashedUIChar
check("the last pick is kept per character", type(saved) == "table" and saved.bossfights and saved.bossfights.diff == "heroic"
    and saved.bossfights.boss == "frozen-throne")
-- engaged when the interface reloads
B = boot(saved)
check("reload: .bossfight status asked", count("^say: %.bossfight status$") == 1)
check("reload: the pick is back", B.diff == "heroic" and B.boss == "frozen-throne" and B.playersOnly == true)
bf(nil, "T\t65000")
F.tick(0.1)
bar = _G.LeylineBossFightBar
check("reload: the realm's T puts the bar and the clock back", bar and bar:IsVisible() and bar.clock:GetText() == "01:05.1",
    bar and bar.clock:GetText())
F.tick(2)
check("reload: the clock runs", bar.clock:GetText() == "01:07.1", bar.clock:GetText())
-- a loading screen (into the instance): asked again
F.fire("PLAYER_ENTERING_WORLD")
check("a loading screen: asked again", count("^say: %.bossfight status$") == 2)
sweep()
-- queued when it reloads
B = boot(saved)
bf(nil, "Q\tfrozen-throne\tnormal\tgroup\t1\t9\t45")
F.tick(0.2)
bar = _G.LeylineBossFightBar
check("reload while queued: the queue bar is back", bar:IsVisible() and bar.line:GetText() == "Waiting for players... bots fill the rest in ~45s",
    bar.line:GetText())

-- another server: no hello, no status, no menu button
WrathUnleashedUI, WrathUnleashedUIDB, WrathUnleashedUIChar = nil, { den = { introSeen = true } }, nil
F = dofile(uidir .. "/fakeui.lua")
F.setup(1352, 1081)
F.server = nil
F.load(dir)
F.fire("PLAYER_ENTERING_WORLD")
F.tick(4)
check("another server: no .bossfight line", count("bossfight") == 0, table.concat(F.chat, " | "))
WrathUnleashedUI.BossFights.Show(true)
check("another server: the window says so, its buttons off", not _G.LeylineBossFights.go:IsEnabled()
    and _G.LeylineBossFights.status:GetText() == "Boss fights are on Leyline realms.")
GameMenuFrame:Show()
check("another server: no Esc button", not (_G.GameMenuButtonLeylineBossFights and _G.GameMenuButtonLeylineBossFights:IsShown()))

-- ---------------------------------------------------------------- touch: 44 CSS px
-- touch.js on a phone: UIParent 560 units high over a 390 px canvas (1.436 units per CSS px)
B = boot(nil)
UIParent.w, UIParent.h = 1210, 560
F.screen = { w = 1210, h = 560 }
__OWTOUCH = { applied = true, cssH = 390 }
local u = 560 / 390
B.Show(true)
bf(nil, "T\t1000")
bf(nil, "R\twipe\t1000\t-")
F.tick(0.1)
local small, smallest = {}, 999
local frames = { _G.LeylineBossFights, _G.LeylineBossFightBar, _G.LeylineBossFightResult }
for _, top in ipairs(frames) do
    for _, f in ipairs(F.created) do
        if (f.kind == "Button" or f.kind == "EditBox") and f:IsVisible() then
            local p = f
            while p and p ~= top do p = p.parent end
            if p == top then
                local css = (tonumber(f.h) or 0) * f:GetEffectiveScale() / u
                if css < smallest then smallest = css end
                if css < 44 - 0.01 then small[#small + 1] = (f.text or f.edLabel or "?") .. "=" .. string.format("%.1f", css) end
            end
        end
    end
end
check("touch: every control of the window, the bar and the card at least 44 CSS px", #small == 0 and smallest < 999,
    table.concat(small, ", ") .. " smallest " .. smallest)
local fits = true
for _, top in ipairs(frames) do
    local w = (tonumber(top.w) or 0) * top:GetEffectiveScale()
    local h = (tonumber(top.h) or 0) * top:GetEffectiveScale()
    if w > 1210 or h > 560 then fits = false end
end
check("touch: each fits the phone's screen", fits)
local tb = L and WrathUnleashedUI.DenLayout.Rect(_G.LeylineBossFightBar)
check("touch: the bar under touch.js's unit frames", tb and tb.t <= 560 - (12 * u + 110), tb and tb.t)
__OWTOUCH = nil
B.PlaceBar()
check("touch off: scale 1 again", _G.LeylineBossFightBar:GetScale() == 1)
sweep()

-- ---------------------------------------------------------------- no game-brand words
check("no game-brand word in any text shown (" .. #allSeen .. " texts)", #allSeen > 40 and bannedIn(allSeen) == nil, bannedIn(allSeen))
local src = io.open(dir .. "/BossFights.lua"):read("*a")
local lits = {}
for q in src:gsub("%-%-[^\n]*", ""):gmatch('"([^"\n]*)"') do lits[#lits + 1] = q end
local okLits = {}
for _, q in ipairs(lits) do if not q:find("%%f") then okLits[#okLits + 1] = q end end
-- the banned list itself is the one place the words are written
local bad = nil
for _, q in ipairs(okLits) do
    local l = q:lower()
    if l ~= "world of warcraft" and l ~= "warcraft" and l ~= "lich king" and l ~= "blizzard" then
        for _, b in ipairs(BANNED) do if l:find(b) then bad = q end end
    end
end
check("BossFights.lua: no game-brand word in its strings", bad == nil, bad)

print(string.format("%d passed, %d failed", pass, fail))
if fail > 0 then os.exit(1) end
