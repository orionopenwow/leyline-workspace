-- Your first build: a short tour of Your Den's editor, one step at a time (owner 12 Nov: "makes it more user
-- intuitive"; docs/DEN-NEXT.md "The builder's comforts"). A small card at the right of the screen says what to do next
-- and moves on by itself when you have done it: it watches what the editor sends Den Lite (SendAddonMessage, hooked:
-- `wubuild cmd ...`) and what Den Lite answers (`wubuild aim|placed|carried|grabbed|state`). Nothing it does changes
-- the Den.
--
--   1 Build mode   F7                         6 Copy it        Ctrl+D
--   2 Pick a prop  a gallery card             7 Take it back   Ctrl+Z
--   3 Put it down  click the ground           8 Select several Shift+click, Ctrl+drag, Alt+drag
--   4 Move it      click to carry / drag      9 Play it        F7 again
--   5 Turn it      Q / E
--
-- It shows the first time you are in Your Den after the intro cards, once a tour was never finished or ended. Skip
-- step, End the tour; /dentour starts it again, /dentour off ends it. Kept in WrathUnleashedUIDB.den.coach.
-- TODO(owner): a voice pass on the steps' words.

local W = WrathUnleashedUI or {}
local C = { step = 1, shown = false, dismissed = false }
W.DenCoach = C

C.STEPS = {
    { id = "build", title = "Build mode", text = "Press F7 to start building. A thin gold edge and the tools show you're in Build mode." },
    { id = "pick", title = "Pick a prop", text = "Open the gallery (the Build button on the left) and click a card. The prop follows your mouse." },
    { id = "place", title = "Put it down", text = "Move the mouse over the ground and left-click. Shift+click puts down another." },
    { id = "move", title = "Move it", text = "Click your prop to pick it up, move the mouse, click again to put it down. Or press on it and drag." },
    { id = "turn", title = "Turn it", text = "Point at it and press Q or E. With it in hand, the mouse wheel turns it too. Hold Alt for fine turns." },
    { id = "dup", title = "Copy it", text = "With it selected, press Ctrl+D: a copy beside it. Press again and the row carries on." },
    { id = "undo", title = "Take it back", text = "Ctrl+Z undoes the last change, Ctrl+Y does it again. Every change is one step." },
    { id = "many", title = "Select several", text = "Shift+click adds a prop. Ctrl+drag draws a box, Alt+drag a circle. Drag one and they all move." },
    { id = "play", title = "Play it", text = "Press F7 again to play: walk round, sit on a chair. F7 brings you back to building." },
}
C.DONE_TEXT = "That's the whole of it. The ? on the top strip lists every key; /dentour runs this again."

local function S() return W.Style end
local function db()
    WrathUnleashedUIDB = type(WrathUnleashedUIDB) == "table" and WrathUnleashedUIDB or {}
    local d = WrathUnleashedUIDB
    d.den = type(d.den) == "table" and d.den or {}
    d.den.coach = type(d.den.coach) == "table" and d.den.coach or {}
    return d.den.coach
end
C.db = db

local card

-- ---------------------------------------------------------------- what was done

-- An editor event: `kind` is "out" (sent to Den Lite) or "in" (its answer), `name` the answer's name, `text` the data.
-- Moves the tour on when it is what the step waits for.
function C.Saw(kind, name, text)
    text = tostring(text or "")
    local id = C.STEPS[C.step] and C.STEPS[C.step].id
    if kind == "in" and (name == "carried" or name == "grabbed") and text:match("^%d+$") then C.holding = true end
    if kind == "out" and text:match("^preview off") then C.holding = nil end
    local hit = false
    if kind == "out" then
        if text:match("^mode build") then hit = id == "build"
        elseif text:match("^mode play") then hit = id == "play"
        elseif text:match("^commit") then
            if C.holding then hit = id == "move"; C.holding = nil end
        elseif text:match("^turn") or text:match("^preview turn") then hit = id == "turn"
        elseif text:match("^nudge") then
            local deg = tonumber(text:match("^nudge %S+ %S+ %S+ ([%-%d]+)") or "0") or 0
            hit = id == "turn" and deg ~= 0
        elseif text:match("^dupby") or text == "dup" then hit = id == "dup"
        elseif text:match("^undo") then hit = id == "undo"
        end
    elseif kind == "in" then
        if name == "aim" and text:match("^place") then hit = id == "pick"
        elseif name == "placed" then hit = id == "place"
        elseif name == "state" then
            local n = tonumber(text:match("n=(%d+);") or "0") or 0
            hit = id == "many" and n >= 2
        end
    end
    if hit then C.Next(true) end
    return hit
end

-- The next step (done: said with a tick for a moment); past the last, the tour is over.
function C.Next(done)
    C.lastDone = done and C.STEPS[C.step] and C.STEPS[C.step].title or nil
    C.step = C.step + 1
    local d = db()
    d.step = C.step
    if C.step > #C.STEPS then d.done = true end
    C.Draw()
    return C.step
end

-- Steps already done when the card shows (in Build mode already): moved past.
function C.CatchUp()
    local G = W.Gizmo
    if C.STEPS[C.step] and C.STEPS[C.step].id == "build" and G and G.build then C.Next(true) end
end

-- ---------------------------------------------------------------- the card

local function text(parent, size, ...)
    local st = S()
    local fs = st and st.EdText and st.EdText(parent, size) or parent:CreateFontString(nil, "OVERLAY", "GameFontHighlight")
    if select("#", ...) > 0 then fs:SetPoint(...) end
    if fs.SetJustifyH then fs:SetJustifyH("LEFT") end
    return fs
end

local function build()
    if card or not CreateFrame or not UIParent then return card end
    local st = S()
    card = CreateFrame("Frame", "LeylineDenCoach", UIParent)
    card:SetFrameStrata("HIGH")
    card:SetWidth(310); card:SetHeight(170)
    card:SetPoint("TOPRIGHT", UIParent, "TOPRIGHT", -24, -160)
    card:EnableMouse(true)
    if st and st.EdPanel then st.EdPanel(card, 0.92, st.ED.panel2); if st.EdEdge then st.EdEdge(card, st.ED.gold) end end
    card.icon = card:CreateTexture(nil, "ARTWORK")
    card.icon:SetWidth(20); card.icon:SetHeight(20)
    card.icon:SetPoint("TOPLEFT", card, "TOPLEFT", 12, -10)
    if st and st.SetIcon then st.SetIcon(card.icon, "coach", st.ED.gold) end
    card.label = st and st.EdLabel and st.EdLabel(card, "Your first build", true, "TOPLEFT", 38, -14) or text(card, 13, "TOPLEFT", 38, -14)
    card.count = text(card, 13, "TOPRIGHT", card, "TOPRIGHT", -12, -14)
    if st and st.Muted then st.Muted(card.count) end
    card.done = text(card, 13, "TOPLEFT", card, "TOPLEFT", 12, -38)
    if card.done.SetTextColor then card.done:SetTextColor(0.31, 0.82, 0.55) end
    card.head = text(card, 15, "TOPLEFT", card, "TOPLEFT", 12, -56)
    card.body = text(card, 13, "TOPLEFT", card, "TOPLEFT", 12, -78)
    card.body:SetWidth(286)
    if card.body.SetJustifyV then card.body:SetJustifyV("TOP") end
    -- the steps as dots: gold done, white this one, grey to come
    card.dots = {}
    for i = 1, #C.STEPS do
        local t = card:CreateTexture(nil, "ARTWORK")
        t:SetWidth(14); t:SetHeight(4)
        t:SetPoint("BOTTOMLEFT", card, "BOTTOMLEFT", 12 + (i - 1) * 18, 44)
        card.dots[i] = t
    end
    local chip = st and st.EdChip
    if chip then
        card.skip = chip(card, "Skip step", nil, function() C.Next(false) end, "BOTTOMLEFT", card, "BOTTOMLEFT", 10, 10)
        card.close = chip(card, "End the tour", nil, function() C.End() end, "BOTTOMRIGHT", card, "BOTTOMRIGHT", -10, 10)
    end
    card:Hide()
    return card
end
C.Build = build

function C.Draw()
    if not card then return end
    local n = #C.STEPS
    local s = C.STEPS[C.step]
    if s then
        card.count:SetText(C.step .. " / " .. n)
        card.head:SetText(s.title)
        card.body:SetText(s.text)
        if card.skip then card.skip:Show() end
        if card.close and card.close.SetText then card.close:SetText("End the tour") end
    else
        card.count:SetText(n .. " / " .. n)
        card.head:SetText("Your Den is yours")
        card.body:SetText(C.DONE_TEXT)
        if card.skip then card.skip:Hide() end
        if card.close and card.close.SetText then card.close:SetText("Close") end
    end
    card.done:SetText(C.lastDone and ("Done: " .. C.lastDone) or "")
    for i, t in ipairs(card.dots) do
        if i < C.step then t:SetTexture(0.878, 0.643, 0.227, 1)
        elseif i == C.step then t:SetTexture(0.92, 0.92, 0.94, 1)
        else t:SetTexture(0.36, 0.37, 0.40, 0.8) end
    end
end

function C.Show()
    build()
    if not card then return false end
    C.shown = true
    C.CatchUp()
    C.Draw()
    card:Show()
    return true
end
function C.Hide()
    C.shown = false
    if card then card:Hide() end
end
-- End the tour (and Close after the last step): not shown again by itself.
function C.End()
    db().done = true
    C.dismissed = true
    C.Hide()
end
-- /dentour: from the first step again.
function C.Restart()
    local d = db()
    d.done, d.step = false, 1
    C.step, C.lastDone, C.dismissed, C.holding = 1, nil, false, nil
    return C.Show()
end

-- Shown by itself: in Your Den, the intro cards seen, the tour never finished or ended, not closed this session.
function C.Wanted()
    local D = W.DenBar
    if not (D and D.active) then return false end
    local den = WrathUnleashedUIDB and WrathUnleashedUIDB.den
    if not (den and den.introSeen) then return false end
    local intro = D.Intro and D.Intro()
    if intro and intro.IsShown and intro:IsShown() then return false end
    local d = db()
    return not d.done and not C.dismissed
end

function C.Tick()
    local want = C.Wanted()
    if want and not C.shown then C.Show()
    elseif not want and C.shown and not (C.step > #C.STEPS) then C.Hide()
    elseif C.shown then C.CatchUp() end
end

-- ---------------------------------------------------------------- set up

local function onAddon(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= "wubuild" then return end
    if C.shown then C.Saw("in", name, data) end
end

local function onSend(prefix, msg)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or not C.shown then return end
    local data = msg:match("^wubuild\tcmd\t1/1\t(.*)$")
    if data then C.Saw("out", "cmd", data) end
end
C.OnSend = onSend

local function setup()
    if C.frame or not CreateFrame or not UIParent then return end
    local d = db()
    C.step = math.max(1, math.min(#C.STEPS + 1, tonumber(d.step) or 1))
    if C.step > #C.STEPS and not d.done then C.step = 1 end
    local f = CreateFrame("Frame", "LeylineDenCoachWatch", UIParent)
    C.frame = f
    local acc = 0
    f:SetScript("OnUpdate", function(_, el)
        acc = acc + (el or 0)
        if acc < 0.5 then return end
        acc = 0
        C.Tick()
    end)
    f:SetScript("OnEvent", function(_, ev, ...) if ev == "CHAT_MSG_ADDON" then onAddon(...) end end)
    f:RegisterEvent("CHAT_MSG_ADDON")
    if hooksecurefunc then pcall(hooksecurefunc, "SendAddonMessage", onSend) end
    SLASH_LEYLINEDENTOUR1 = "/dentour"
    SlashCmdList = SlashCmdList or {}
    SlashCmdList.LEYLINEDENTOUR = function(msg)
        if tostring(msg or ""):lower():match("off") then C.End() else C.Restart() end
    end
end
C.Setup = setup

if CreateFrame and UIParent then
    local ok = pcall(setup)
    if not ok then C.frame = nil end
end
