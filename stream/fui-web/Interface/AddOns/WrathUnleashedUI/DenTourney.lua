-- Tournament (3.3.5a API): duel tournaments in the Den, one window in the editor's look (owner 4 Nov: "duel
-- tournaments in the Den ... a bracket window in the UI (ForeverUI style) ... guests sign up in-game from a Den
-- panel"; the formats and the ideas list are docs/DEN-TOURNAMENTS.md). The Den bar has an Arena button (DenBar.lua:
-- the rail, and the play-mode corner); /tourney opens it too.
--
-- Everyone      Join / Leave (the sign-ups), Yield (you lose the round you're fighting), the bracket and the match
--               being fought (the names, the score, the countdown's state), the sign-ups' names
-- The host      Single / Robin / King (the format), Bo1 / Bo3 / Bo5 (rounds a match), New, Ring here (the arena where
--               you stand: step out and you lose the round), Rule (a house rule, click for the next), Start, Cancel,
--               and for the match being fought "<name> won" (the host's call)
--
-- Every button is one Den command over the realm's addon channel (mod "wuden", "cmd", "tourney <...>": EditMode.lua
-- DenCmd), answered by Den Lite (labs/den-lite tourney.rs):
--   server -> client   tourney  fmt=single|robin|king;bo=<n>;phase=signup|waiting|called|countdown|fight|done;n=<players>;
--                               a=<name|->;b=<name|->;wa=<n>;wb=<n>;champ=<name|->;ring=0|1;me=host|in|out;in=0|1;twist=<word|->;title=<text>
--                               (phase=none: there is no tournament now)
--                      tsign    <i>/<n>|<name>,<name>,...        the sign-ups, in parts
--                      tbr      <i>/<n>|<round>,<a>,<b|->,<wa>,<wb>,<winner|->;...   the matches, in parts
-- What everyone hears (the calls, the countdown, "Fight!") comes as system lines. Nothing the server sends is run;
-- names are letters only, texts are shown plain.

local W = WrathUnleashedUI
local T = { line = {}, signups = {}, matches = {}, parts = { tsign = {}, tbr = {} }, format = "single", bo = 3, twistAt = 0 }
local MOD = "wuden"
T.FORMATS = { { "single", "Single", "Single elimination: lose a match and you're out" },
    { "robin", "Robin", "Round robin: everyone fights everyone, the most wins take it (up to 10 players)" },
    { "king", "King", "King of the hill: the winner stays on; three wins in a row take the crown" } }
T.BESTOF = { 1, 3, 5 }
-- labs/den-lite tourney.rs TWISTS (said with each call; the ring keeps "ringout")
T.TWISTS = { "ringout", "classlock", "normalised", "nopotions", "melee", "handicap", "mirror", "sudden", "mounted", "costume" }
T.TWIST_WORDS = { ringout = "Ring out", classlock = "Class-locked", normalised = "Same gear", nopotions = "No potions", melee = "Melee only",
    handicap = "Handicap", mirror = "Mirror match", sudden = "Sudden death", mounted = "Mounted joust", costume = "Costumes" }
T.PHASE_WORDS = { signup = "Sign-ups open", waiting = "Next match soon", called = "Fighters to the ring", countdown = "Get set...",
    fight = "Fight!", done = "Over" }
T.FORMAT_WORDS = { single = "Single elimination", robin = "Round robin", king = "King of the hill" }
-- the bracket shows this many rounds side by side, and this many matches in each
T.COLS, T.ROWS = 5, 8

local panel

local function S() return W.Style end
local function plain(s) return (tostring(s or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("|", ""):gsub("%c", " ")) end
-- a character's name from the server: letters only, 12 at most ("-": none)
local function name(s)
    s = tostring(s or "")
    if s == "-" or s == "" then return nil end
    s = s:gsub("[^%a]", ""):sub(1, 12)
    if s == "" then return nil end
    return s
end

local function send(data)
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, "OVMOD", MOD .. "\tcmd\t1/1\t" .. data, "WHISPER", UnitName("player"))
    return true
end

-- One Den command: "tourney <text>".
function T.Cmd(text)
    local t = "tourney" .. (text and text ~= "" and (" " .. text) or "")
    local E = W.EditMode
    if E and E.DenCmd then return E.DenCmd(t) end
    return send((t:gsub("[^%w%s%-_]", ""):sub(1, 80)))
end

-- the host: the tournament's line says so; with no tournament yet, the Den bar's role (DenBar.lua status: owner)
function T.Host()
    if T.line.me then return T.line.me == "host" end
    local st = W.DenBar and W.DenBar.status
    return st == nil or st.role == "owner"
end
function T.Signed() return T.line["in"] == "1" or T.line.me == "in" end
function T.Live() return T.line.phase ~= nil and T.line.phase ~= "signup" and T.line.phase ~= "done" end
function T.Twist() return T.TWISTS[T.twistAt] end
function T.NextTwist()
    T.twistAt = (T.twistAt + 1) % (#T.TWISTS + 1)
    T.Paint()
    return T.Twist()
end
function T.SetFormat(f) T.format = f; T.Paint() return f end
function T.SetBestOf(n) T.bo = n; T.Paint() return n end
function T.New() return T.Cmd("new " .. T.format .. " bo" .. T.bo) end
function T.Rule() local t = T.Twist(); return T.Cmd("twist " .. (t or "off")) end

-- ---------------------------------------------------------------- the window
local function label(parent, text, ...) return S().EdLabel(parent, text, false, ...) end

local function build()
    panel = CreateFrame("Frame", "LeylineTourneyPanel", UIParent)
    panel:SetWidth(560); panel:SetHeight(420)
    panel:SetPoint("TOPLEFT", UIParent, "TOPLEFT", 80, -118)
    panel:SetFrameStrata("MEDIUM")
    panel:EnableMouse(true)
    if panel.SetMovable then panel:SetMovable(true) end
    S().EdPanel(panel, 0.92)
    panel.head = S().EdLabel(panel, "Tournament", true, "TOPLEFT", 10, -9)
    panel.close = S().EdChip(panel, "x", 24, function() T.Show(false) end, "TOPRIGHT", -8, -8)
    panel.title = panel:CreateFontString(nil, "OVERLAY", "GameFontNormal")
    panel.title:SetPoint("TOPLEFT", 10, -36); panel.title:SetWidth(540); panel.title:SetJustifyH("LEFT")
    panel.title.styleKeep = true
    panel.note = panel:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    panel.note:SetPoint("TOPLEFT", 10, -54); panel.note:SetWidth(540); panel.note:SetJustifyH("LEFT")
    panel.note.styleKeep = true

    local function chips(row, list, x, y)
        for _, c in ipairs(list) do
            local b = S().EdChip(panel, c[2], nil, c[3], "TOPLEFT", x, y)
            b.edLabel = c[4]
            panel[c[1]] = b
            x = x + (tonumber((b:GetWidth())) or 52) + 4
        end
        return x
    end
    -- the host's row: the format, best of, New, Ring here, Rule, Start, Cancel
    panel.hostRow = label(panel, "Host", "TOPLEFT", 10, -81)
    panel.formats = {}
    local x = 70
    for _, f in ipairs(T.FORMATS) do
        local b = S().EdChip(panel, f[2], nil, function() T.SetFormat(f[1]) end, "TOPLEFT", x, -76)
        b.edLabel = f[3]; b.format = f[1]
        panel.formats[#panel.formats + 1] = b
        x = x + (tonumber((b:GetWidth())) or 52) + 4
    end
    x = x + 6
    panel.bests = {}
    for _, n in ipairs(T.BESTOF) do
        local b = S().EdChip(panel, "Bo" .. n, nil, function() T.SetBestOf(n) end, "TOPLEFT", x, -76)
        b.edLabel = n == 1 and "One round a match" or ("Best of " .. n .. ": first to " .. (math.floor(n / 2) + 1) .. " rounds wins the match")
        b.bo = n
        panel.bests[#panel.bests + 1] = b
        x = x + (tonumber((b:GetWidth())) or 40) + 4
    end
    x = chips(panel.hostRow, { { "new", "New", function() T.New() end, "A new tournament with this format: sign-ups open" } }, x + 6, -76)
    chips(panel.hostRow, {
        { "ring", "Ring here", function() T.Cmd("ring") end, "The arena: a ring 16 yards across where you stand. Step out during a fight and you lose the round" },
        { "rule", "No rule", function() T.NextTwist() end, "A house rule (click for the next), said with every call; Set sends it" },
        { "ruleSet", "Set", function() T.Rule() end, "Use that house rule" },
        { "start", "Start", function() T.Cmd("start") end, "Close the sign-ups and call the first match" },
        { "cancel", "Cancel", function() T.Cmd("cancel") end, "Call the tournament off" },
    }, 70, -110)
    -- everyone's row
    panel.youRow = label(panel, "You", "TOPLEFT", 10, -149)
    chips(panel.youRow, {
        { "join", "Join", function() T.Cmd("join") end, "Sign up" },
        { "leave", "Leave", function() T.Cmd("leave") end, "Off the list (once it runs: your matches go to the other fighter)" },
        { "yield", "Yield", function() T.Cmd("yield") end, "Give up the round you're fighting" },
        { "bracket", "In chat", function() T.Cmd("bracket") end, "The bracket in the chat as well" },
    }, 70, -144)
    -- the match being fought: the names and the score; the host's call
    panel.vs = panel:CreateFontString(nil, "OVERLAY", "GameFontNormalLarge")
    panel.vs:SetPoint("TOPLEFT", 10, -184); panel.vs:SetWidth(360); panel.vs:SetJustifyH("LEFT")
    panel.vs.styleKeep = true
    panel.winA = S().EdChip(panel, "A won", nil, function() local a = name(T.line.a); if a then T.Cmd("win " .. a) end end, "TOPLEFT", 380, -180)
    panel.winA.edLabel = "The host's call: this round to them"
    panel.winB = S().EdChip(panel, "B won", nil, function() local b = name(T.line.b); if b then T.Cmd("win " .. b) end end, "TOPLEFT", 470, -180)
    panel.winB.edLabel = "The host's call: this round to them"
    -- the bracket: a column per round, a line per match
    panel.cols = {}
    for c = 1, T.COLS do
        local col = { head = panel:CreateFontString(nil, "OVERLAY", "GameFontNormalSmall"), lines = {} }
        col.head:SetPoint("TOPLEFT", 10 + (c - 1) * 108, -218); col.head:SetWidth(104); col.head:SetJustifyH("LEFT")
        col.head.styleKeep = true
        for r = 1, T.ROWS do
            local l = panel:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
            l:SetPoint("TOPLEFT", 10 + (c - 1) * 108, -218 - r * 22); l:SetWidth(104); l:SetJustifyH("LEFT")
            l.styleKeep = true
            col.lines[r] = l
        end
        panel.cols[c] = col
    end
    panel.more = panel:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    panel.more:SetPoint("TOPLEFT", 10, -218 - (T.ROWS + 1) * 22); panel.more:SetWidth(540); panel.more:SetJustifyH("LEFT")
    panel.more.styleKeep = true
    panel.signed = panel:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    panel.signed:SetPoint("TOPLEFT", 10, -218 - (T.ROWS + 2) * 22); panel.signed:SetWidth(540); panel.signed:SetJustifyH("LEFT")
    panel.signed.styleKeep = true
    panel:SetHeight(218 + (T.ROWS + 3) * 22 + 10)
    panel:Hide()
    T.Paint()
end

function T.Panel() if not panel then build() end return panel end
function T.IsOpen() return panel ~= nil and panel:IsShown() end

function T.Show(on)
    T.Panel()
    if on then
        panel:Show()
        T.Cmd("status")
    else
        panel:Hide()
    end
    T.Paint()
    if W.DenBar and W.DenBar.Paint then W.DenBar.Paint() end
    return T.IsOpen()
end
function T.Toggle() return T.Show(not T.IsOpen()) end

-- The bracket's columns: by round (king of the hill: one column of its matches, the newest last).
function T.Columns()
    local rounds, order = {}, {}
    local king = T.line.fmt == "king"
    for _, m in ipairs(T.matches) do
        local r = king and 1 or m.round
        if not rounds[r] then rounds[r] = {}; order[#order + 1] = r end
        table.insert(rounds[r], m)
    end
    table.sort(order)
    local cols = {}
    for _, r in ipairs(order) do cols[#cols + 1] = { round = r, matches = rounds[r] } end
    return cols
end

local function matchText(m, now)
    if not m.b then return m.a .. " (bye)" end
    local s = m.a .. " - " .. m.b
    if m.winner then
        local hi, lo = math.max(m.wa, m.wb), math.min(m.wa, m.wb)
        return S().Hex("accent") .. m.winner .. "|r " .. hi .. "-" .. lo .. "  " .. (m.winner == m.a and m.b or m.a)
    end
    return (now and (S().Hex("accent") .. "> |r") or "") .. s
end

function T.Paint()
    if not panel then return end
    local l = T.line
    local none = l.phase == nil
    if none then
        panel.title:SetText("No tournament yet")
        panel.note:SetText("The host picks a format and presses New; then everyone presses Join.")
    else
        panel.title:SetText(plain(l.title or ""):sub(1, 40) .. "  ·  " .. (T.FORMAT_WORDS[l.fmt] or "") .. ", best of " .. (tonumber(l.bo or "") or 1))
        local champ = name(l.champ)
        local bits = { T.PHASE_WORDS[l.phase] or "", (tonumber(l.n or "") or 0) .. " players" }
        if l.ring == "1" then bits[#bits + 1] = "in the ring" end
        if l.twist and l.twist ~= "-" and T.TWIST_WORDS[l.twist] then bits[#bits + 1] = "rule: " .. T.TWIST_WORDS[l.twist] end
        if champ then bits = { "Champion: " .. champ, (tonumber(l.n or "") or 0) .. " players" } end
        panel.note:SetText(table.concat(bits, "  ·  "))
    end
    -- the host's buttons: theirs only (a friend sees the format and the bracket)
    local host = T.Host()
    for _, b in ipairs(panel.formats) do b:SetActive(b.format == T.format); if host then b:Enable() else b:Disable() end end
    for _, b in ipairs(panel.bests) do b:SetActive(b.bo == T.bo); if host then b:Enable() else b:Disable() end end
    for _, k in ipairs({ "new", "ring", "rule", "ruleSet", "start", "cancel" }) do
        if host then panel[k]:Enable() else panel[k]:Disable(); panel[k].edLabel = "Your host runs the tournament" end
    end
    panel.hostRow:SetText(host and "HOST" or "THE HOST'S")
    panel.ring:SetActive(l.ring == "1")
    panel.rule:SetText(T.Twist() and T.TWIST_WORDS[T.Twist()] or "No rule")
    panel.ruleSet:SetActive(l.twist ~= nil and l.twist ~= "-" and l.twist == T.Twist())
    if none or l.phase ~= "signup" then panel.start:Disable() end
    if none or l.phase == "done" then panel.cancel:Disable() end
    if T.Live() then panel.new:Disable() end
    -- yours
    local signup = l.phase == "signup"
    if signup and not T.Signed() then panel.join:Enable() else panel.join:Disable() end
    if T.Signed() and (signup or T.Live()) then panel.leave:Enable() else panel.leave:Disable() end
    panel.join:SetActive(T.Signed())
    local me = UnitName and UnitName("player") or ""
    local a, b = name(l.a), name(l.b)
    local fighting = T.Live() and (me == a or me == b)
    if fighting then panel.yield:Enable() else panel.yield:Disable() end
    if none then panel.bracket:Disable() else panel.bracket:Enable() end
    -- the match
    if a and b then
        panel.vs:SetText(string.format("%s  %s%d - %d|r  %s", a, S().Hex("accent"), tonumber(l.wa or "") or 0, tonumber(l.wb or "") or 0, b))
        panel.winA:SetText(a .. " won"); panel.winB:SetText(b .. " won")
        if host and T.Live() then panel.winA:Show(); panel.winB:Show() else panel.winA:Hide(); panel.winB:Hide() end
    else
        panel.vs:SetText(name(l.champ) and ("Champion: " .. name(l.champ)) or "")
        panel.winA:Hide(); panel.winB:Hide()
    end
    -- the bracket
    local cols = T.Columns()
    local first = math.max(1, #cols - T.COLS + 1)   -- the latest rounds when there are more than fit
    local hidden = 0
    for c = 1, T.COLS do
        local col = panel.cols[c]
        local data = cols[first + c - 1]
        col.head:SetText(data and (l.fmt == "king" and "MATCHES" or ("ROUND " .. data.round)) or "")
        local list = data and data.matches or {}
        local from = (l.fmt == "king") and math.max(1, #list - T.ROWS + 1) or 1
        for r = 1, T.ROWS do
            local m = list[from + r - 1]
            local now = m and not m.winner and m.b and m.a == a and m.b == b
            col.lines[r]:SetText(m and matchText(m, now) or "")
        end
        if #list > T.ROWS then hidden = hidden + #list - T.ROWS end
    end
    panel.more:SetText(hidden > 0 and (hidden .. " more matches: In chat shows them all.") or "")
    panel.signed:SetText(#T.signups > 0 and ("Signed up: " .. table.concat(T.signups, ", ")) or (none and "" or "Nobody signed up yet."))
end

-- ---------------------------------------------------------------- the server's answers
local function fields(data, n)
    local t = {}
    for k, v in data:gmatch("(%a+)=([^;]*)") do t[k] = plain(v):sub(1, n) end
    return t
end

-- "<i>/<n>|<body>": parts kept until all n are in, then the whole list
local function part(kind, data)
    local i, n, body = data:match("^(%d+)/(%d+)|(.*)$")
    i, n = tonumber(i), tonumber(n)
    if not (i and n) or i < 1 or n > 40 or i > n then return nil end
    local p = T.parts[kind]
    if i == 1 then for k in pairs(p) do p[k] = nil end end
    p[i] = body
    for k = 1, n do if p[k] == nil then return nil end end
    local all = {}
    for k = 1, n do all[#all + 1] = p[k] end
    return all
end

function T.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, nm, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= MOD then return end
    if nm == "tourney" then
        local l = fields(data, 40)
        if l.phase == "none" then
            T.line, T.signups, T.matches = {}, {}, {}
        elseif T.PHASE_WORDS[l.phase or ""] and T.FORMAT_WORDS[l.fmt or ""] then
            T.line = l
            T.format = l.fmt
            T.bo = tonumber(l.bo or "") or T.bo
        else
            return
        end
        T.Paint()
        if W.DenBar and W.DenBar.Paint then W.DenBar.Paint() end
    elseif nm == "tsign" then
        local all = part("tsign", data)
        if not all then return end
        local out = {}
        for _, body in ipairs(all) do
            for w in body:gmatch("[^,]+") do local n2 = name(w); if n2 and #out < 32 then out[#out + 1] = n2 end end
        end
        T.signups = out
        T.Paint()
    elseif nm == "tbr" then
        local all = part("tbr", data)
        if not all then return end
        local out = {}
        for _, body in ipairs(all) do
            for row in body:gmatch("[^;]+") do
                local r, a, b, wa, wb, w = row:match("^(%d+),([^,]*),([^,]*),(%d+),(%d+),([^,]*)$")
                if r and name(a) and #out < 64 then
                    out[#out + 1] = { round = tonumber(r), a = name(a), b = name(b), wa = tonumber(wa), wb = tonumber(wb), winner = name(w) }
                end
            end
        end
        T.matches = out
        T.Paint()
    end
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:RegisterEvent("PLAYER_ENTERING_WORLD")
listener:SetScript("OnEvent", function(_, event, prefix, msg, channel, sender)
    if event == "CHAT_MSG_ADDON" then return T.OnMessage(prefix, msg, channel, sender) end
    -- leaving the Den: the window goes with it (DenBar.lua decides what a Den is; it loads first)
    if event == "PLAYER_ENTERING_WORLD" and panel and not (W.DenBar and W.DenBar.IsActive and W.DenBar.IsActive()) then
        panel:Hide()
    end
end)

SLASH_LEYLINETOURNEY1 = "/tourney"
SLASH_LEYLINETOURNEY2 = "/arena"
SlashCmdList["LEYLINETOURNEY"] = function() T.Toggle() end

W.Tourney = T
