-- ov_events.lua: the events clock for Wrath Unleashed (mod-ale, 3.3.5a). Off unless OV_EVENTS=1 (main.env).
--
-- What it does
--   * Stock events, announced shortly before they start: the arena chest in Stranglethorn (game_event 16, every 3 h),
--     the fishing contests (the Kalu'ak derby, the Stranglethorn one), Call to Arms weekends, and holidays when they
--     begin. Times come from the world database's game_event rows (start_time + n * occurence), read at load and
--     every hour; holidays (game_event.holiday <> 0) are announced by the GAME_EVENT_START hook. OV_EVENTS_STOCK.
--   * Our own small events, bragging rights only (no gold, no items; owner 2026-10-31):
--       Trivia (OV_EVENTS_TRIVIA): a few questions from our own set below (general fantasy and this realm, no game
--         data); answer with .ans <answer> or by saying "#ans <answer>". First right answer scores; most points wins.
--       Hide and seek (OV_EVENTS_HIDE): written by us from how the familiar event is described (a creature hides
--         somewhere in Dalaran, hints come every few minutes, the first player to reach it and talk to it wins); none
--         of that script's code. The hider is automatic (one of the SPOTS below) or a GM's: .events hide here [hint].
--   * A weekly board of wins: .events top (characters.ov_event_wins). Bots never win.
--   * Announcements are rate-limited (ANNOUNCE_GAP_S apart, ANNOUNCE_PER_HOUR at most); .events off mutes them for you.
--
-- Player commands: .events (what's on next), .events top, .events off | on, .ans <answer>.
-- GM (rank 2+): .events trivia | hide [here [hint]] | stop.
-- Env (all off unless set; realm/profiles/main.env turns them on): OV_EVENTS, OV_EVENTS_STOCK, OV_EVENTS_TRIVIA,
--   OV_EVENTS_HIDE; OV_EVENTS_TRIVIA_EVERY_H (2) and _AT (15: minute), OV_EVENTS_HIDE_EVERY_H (2), _OFFSET_H (1) and
--   _AT (45); OV_EVENTS_LEAD_MIN (5: the "starting soon" lead); OV_EVENTS_MIN_PLAYERS (1: real players online for
--   our own events to run); OV_EVENTS_HIDER (721: the hider's creature entry; TODO(check in game)).
-- mod-ale (84b85cc): RegisterServerEvent 34 GAME_EVENT_START (event, id); RegisterPlayerEvent 18 ON_CHAT
--   (event, player, msg, type, lang), 42 ON_COMMAND; RegisterCreatureGossipEvent(entry, 1 ON_HELLO, fn);
--   CreateLuaEvent; PerformIngameSpawn(1, entry, map, instance, x, y, z, o, save, despawnMs); GetPlayersInWorld;
--   WorldDBQuery; CharDBQuery / CharDBExecute.

local function flag(name) return os.getenv(name) == "1" end
local function num(name, default)
    local v = tonumber(os.getenv(name) or "")
    return v or default
end

local ON = flag("OV_EVENTS")
local TAG = "|cffffcc00[Events]|r "
local DALARAN_MAP, DALARAN_ZONE = 571, 4395
local ANNOUNCE_GAP_S = 120
local ANNOUNCE_PER_HOUR = 10
local TICK_MS = 30000          -- the schedule
local FAST_MS = 2000           -- a running event (questions, hints, who reached the hider)
local HOST_LINE = "Want your own event night? Host your own world: |cffffff00/host|r"

local E = {}           -- state, exposed for tests as OV_EVENTS_TEST
E.muted = {}           -- player guid -> true (.events off)
E.sent = {}            -- announcement times (rate limit)
E.soon = {}            -- "<kind>:<start>" -> true once its "starting soon" went out
E.stock = {}           -- { id, name, start, every, len, stop } periodic stock events
E.holiday = {}         -- game_event id -> description (holidays)
E.active = nil         -- the running own event: { kind = "trivia" | "hide", ... }
E.lastAns = {}         -- player guid -> time of the last answer
E.hostDay = nil        -- the day the host line went out after an event

local function now() return os.time() end
local function try(obj, method, ...)
    if not obj then return nil end
    local ok, fn = pcall(function() return obj[method] end)
    if not ok or type(fn) ~= "function" then return nil end
    local ok2, a = pcall(fn, obj, ...)
    if ok2 then return a end
    return nil
end
-- A player in Dalaran can report Crystalsong Forest (2817) as the zone (seen in game 31 Oct): the city's box counts too.
function E.inDalaran(p)
    if try(p, "GetZoneId") == DALARAN_ZONE or try(p, "GetAreaId") == DALARAN_ZONE then return true end
    if try(p, "GetMapId") ~= DALARAN_MAP then return false end
    local x, y, z = try(p, "GetX"), try(p, "GetY"), try(p, "GetZ")
    return type(x) == "number" and type(y) == "number" and type(z) == "number"
        and x > 5500 and x < 6100 and y > 250 and y < 1050 and z > 550
end
local function isBot(p)
    if try(p, "IsBot") == true or try(p, "IsPlayerbot") == true then return true end
    local name = try(p, "GetAccountName")
    return type(name) == "string" and name:upper():find("^RNDBOT") ~= nil
end
local function say(p, text) p:SendBroadcastMessage(TAG .. text) end
local function players()
    local ok, list = pcall(GetPlayersInWorld)
    local out = {}
    for _, p in ipairs(ok and list or {}) do if not isBot(p) then out[#out + 1] = p end end
    return out
end
local function esc(s) return (tostring(s or ""):gsub("[%z\1-\31]", ""):gsub("\\", "\\\\"):gsub("'", "''")) end
local BANNED = { "world of warcraft", "warcraft", "lich king", "blizzard" }
local function brandSafe(s)
    local l = (s or ""):lower()
    for _, b in ipairs(BANNED) do if l:find(b, 1, true) then return false end end
    return not l:find("%f[%a]wow%f[%A]")
end

-- To everyone who hasn't muted events. Announcements (kind "announce") are rate-limited; lines inside a running
-- event (kind "event": questions, hints, the result) are not, since the event itself bounds them.
local function broadcast(text, kind)
    if kind == "announce" then
        local t, kept = now(), {}
        for _, s in ipairs(E.sent) do if t - s < 3600 then kept[#kept + 1] = s end end
        E.sent = kept
        if #kept >= ANNOUNCE_PER_HOUR or (kept[#kept] and t - kept[#kept] < ANNOUNCE_GAP_S) then return false end
        E.sent[#E.sent + 1] = t
    end
    for _, p in ipairs(players()) do
        if not E.muted[p:GetGUIDLow()] then say(p, text) end
    end
    return true
end

-- the weekly board ------------------------------------------------------------------------------------------------
CharDBExecute("CREATE TABLE IF NOT EXISTS ov_event_wins ("
    .. "week CHAR(7) NOT NULL, guid INT UNSIGNED NOT NULL, name VARCHAR(12) NOT NULL, kind VARCHAR(8) NOT NULL, "
    .. "wins INT UNSIGNED NOT NULL DEFAULT 0, PRIMARY KEY (week, guid, kind))")
local function week() return os.date("%Y-%W", now()) end
local function addWin(p, kind)
    CharDBExecute(string.format("INSERT INTO ov_event_wins (week, guid, name, kind, wins) VALUES ('%s', %d, '%s', '%s', 1) "
        .. "ON DUPLICATE KEY UPDATE wins = wins + 1, name = VALUES(name)", week(), p:GetGUIDLow(), esc(p:GetName()), kind))
end
local function showTop(p)
    local q = CharDBQuery(string.format("SELECT name, SUM(wins) w FROM ov_event_wins WHERE week = '%s' "
        .. "GROUP BY guid, name ORDER BY w DESC, name LIMIT 5", week()))
    if not q then return say(p, "No wins yet this week. Be the first: |cffffff00.events|r shows what's next.") end
    local rows, i = {}, 1
    repeat
        rows[#rows + 1] = string.format("%d. %s %d", i, q:GetString(0), q:GetUInt32(1))
        i = i + 1
    until not q:NextRow()
    say(p, "This week's event wins (bragging rights): " .. table.concat(rows, "   "))
end
local function endLine()
    local day = os.date("%Y-%m-%d", now())
    if E.hostDay == day then return end
    E.hostDay = day
    broadcast(HOST_LINE, "event")
end

-- trivia: our own questions (general fantasy and this realm; no game data, no brand words) --------------------------------
local QUESTIONS = {
    { q = "How many legs does a spider have?", a = { "8", "eight" } },
    { q = "Mix blue and yellow paint. What color do you get?", a = { "green" } },
    { q = "What metal is a dragon's hoard mostly made of?", a = { "gold" } },
    { q = "What creature is half eagle and half lion?", a = { "griffin", "gryphon", "griffon" } },
    { q = "What creature is half horse and half human?", a = { "centaur" } },
    { q = "Which mythical bird rises again from its own ashes?", a = { "phoenix" } },
    { q = "What metal is said to hurt werewolves?", a = { "silver" } },
    { q = "What giant of old tales has a single eye?", a = { "cyclops" } },
    { q = "What do you call a group of wolves?", a = { "pack" } },
    { q = "What do you call a group of crows?", a = { "murder" } },
    { q = "How many horns does a unicorn have?", a = { "1", "one" } },
    { q = "In chess, which piece moves in an L shape?", a = { "knight" } },
    { q = "How many sides does a hexagon have?", a = { "6", "six" } },
    { q = "What gem is famous for being red?", a = { "ruby", "garnet" } },
    { q = "What color is a sapphire, most of the time?", a = { "blue" } },
    { q = "What is the hardest natural gem?", a = { "diamond" } },
    { q = "Which planet is called the red planet?", a = { "mars" } },
    { q = "What sweet thing do bees make?", a = { "honey" } },
    { q = "What sea creature has eight arms?", a = { "octopus" } },
    { q = "Which animal is called the king of the jungle?", a = { "lion" } },
    { q = "What tool does a miner swing?", a = { "pick", "pickaxe", "pick axe" } },
    { q = "What do you call magic that raises the dead?", a = { "necromancy" } },
    { q = "What stringed instrument with a round body does a bard often play?", a = { "lute", "lyre", "harp" } },
    { q = "In old tales, what creature waits under a bridge and asks for a toll?", a = { "troll" } },
    { q = "What do you call the part of a sword you hold?", a = { "hilt", "grip", "handle" } },
    { q = "What do you call someone who forges iron at an anvil?", a = { "blacksmith", "smith" } },
    { q = "What season comes after autumn?", a = { "winter" } },
    { q = "How many days are in a week?", a = { "7", "seven" } },
    { q = "What shape has eight sides?", a = { "octagon" } },
    { q = "Frozen water is called what?", a = { "ice" } },
    { q = "What small winged being of folklore is also called a fairy? (starts with P)", a = { "pixie", "pixy" } },
    { q = "What do you call the moon when it is fully lit?", a = { "full moon", "full" } },
    { q = "On this realm, which city floats in the sky above the forest?", a = { "dalaran" } },
    { q = "On this realm, what do you type to report a bug? (with the dot)", a = { ".bug", "bug" } },
    { q = "On this realm, what do you type for the house rules? (with the dot)", a = { ".rules", "rules" } },
    { q = "On this realm, what do you type to see where to go at level 80? (with the dot)", a = { ".guide", "guide" } },
    { q = "On this realm, which game do you start with /golf?", a = { "minigolf", "mini golf", "golf" } },
    { q = "How many players fight in a 1v1 arena match?", a = { "2", "two" } },
    { q = "On this realm, where is the duel corner? (two words: the landing's name)", a = { "krasus landing", "krasus' landing", "krasuss landing" } },
    { q = "What do you call a dragon's young, just out of the egg? (starts with H)", a = { "hatchling" } },
}
local TRIVIA_ROUND = 5
local QUESTION_S = 60
local PAUSE_S = 10

local function norm(s)
    s = tostring(s or ""):lower():gsub("[^%w%s%.']", " "):gsub("'", ""):gsub("%s+", " "):gsub("^%s+", ""):gsub("%s+$", "")
    s = s:gsub("^the ", ""):gsub("^an? ", "")
    return s
end
local function rightAnswer(q, text)
    local t = norm(text)
    for _, a in ipairs(q.a) do
        if t == norm(a) then return true end
    end
    return false
end
local deck = {}
local function nextQuestion()
    if #deck == 0 then
        for i = 1, #QUESTIONS do deck[i] = i end
        for i = #deck, 2, -1 do local j = math.random(i); deck[i], deck[j] = deck[j], deck[i] end
    end
    return QUESTIONS[table.remove(deck)]
end

local function finish(text)
    local a = E.active
    E.active = nil
    if a and a.kind == "hide" and a.guid and rawget(_G, "GetMapById") then   -- the hider goes (it would anyway, timed)
        local ok, m = pcall(GetMapById, DALARAN_MAP, 0)
        local c = ok and m and try(m, "GetWorldObject", a.guid)
        if c then try(c, "DespawnOrUnsummon", 0) end
    end
    if text then broadcast(text, "event") end
    endLine()
end

local function askNext()
    local a = E.active
    if not a or a.kind ~= "trivia" then return end
    a.n = a.n + 1
    if a.n > a.total then
        local best, name = 0, nil
        for guid, s in pairs(a.score) do
            if s.points > best or (s.points == best and name and s.name < name) then best, name, a.winner = s.points, s.name, guid end
        end
        local w = a.winner and a.score[a.winner]
        if w then addWin(w.player, "trivia") end
        return finish(w and string.format("Trivia is over. %s wins with %d right! Bragging rights: |cffffff00.events top|r.",
            w.name, w.points) or "Trivia is over. Nobody got one this time.")
    end
    a.q = nextQuestion()
    a.asked = now()
    a.open = true
    broadcast(string.format("Question %d of %d: %s  (|cffffff00.ans|r your answer)", a.n, a.total, a.q.q), "event")
end

local function trivia(by)
    if E.active then return false end
    E.active = { kind = "trivia", n = 0, total = TRIVIA_ROUND, score = {}, started = now() }
    broadcast(string.format("Trivia starts now: %d questions. Answer with |cffffff00.ans|r <answer> or say |cffffff00#ans|r "
        .. "<answer>. First right answer scores.", TRIVIA_ROUND), "event")
    E.active.wait = now() + 5
    return true
end

local function answer(p, text)
    local a = E.active
    if not a or a.kind ~= "trivia" or not a.open then return say(p, "No question is open right now. |cffffff00.events|r shows the next trivia.") end
    if isBot(p) then return end
    local guid, t = p:GetGUIDLow(), now()
    if E.lastAns[guid] and t - E.lastAns[guid] < 2 then return end   -- one try every 2 s
    E.lastAns[guid] = t
    if not rightAnswer(a.q, text) then return end
    a.open = false
    local s = a.score[guid] or { points = 0, name = p:GetName(), player = p }
    s.points, s.player = s.points + 1, p
    a.score[guid] = s
    broadcast(string.format("%s got it: %s.", s.name, a.q.a[1]), "event")
    a.wait = t + PAUSE_S
end

-- hide and seek (our own; a creature hides at one of these spots in Dalaran, from the core's game_tele points) -----------------
-- TODO(check in game): each spot is open ground a player can walk to (none inside the factions' guarded enclaves).
local SPOTS = {
    { 5890.33, 621.467, 649.383, 5.62, "It wants a new look.", "Near the barber's chair." },
    { 5885.77, 606.631, 650.294, 5.62, "Thirsty?", "By the shop that sells drinks, east of the square." },
    { 5848.54, 566.409, 652.657, 5.63, "It is hiding with the other pets.", "The pet shop." },
    { 5778.72, 556.013, 651.639, 4.03, "Bows and arrows nearby.", "The hunters' shop, south-west of the square." },
    { 5947.99, 664.679, 641.227, 1.21, "By a statue of an old archmage.", "The memorial in the east gardens." },
    { 5958.08, 617.286, 650.627, 5.90, "It is counting coins.", "The bank on the east side." },
    { 5654.86, 682.2, 652.006, 2.69, "Counting coins, on the far side of town.", "The bank on the west side." },
    { 5700.25, 641.774, 646.28, 3.52, "It likes trees.", "The park on the west side." },
    { 5812.08, 689.954, 647.046, 4.45, "It likes toys.", "The toy shop north of the square." },
    { 5769.84, 632.804, 661.076, 3.19, "Odd things for sale, up a few steps.", "The curio shop west of the square." },
    { 5860.97, 702.897, 643.276, 0.96, "Ink and paper.", "The scribes' shop." },
    { 5903.79, 752.506, 641.048, 1.34, "It smells of leather.", "The leather shop in the north-east." },
    { 5761.53, 736.624, 641.698, 2.64, "Robes and wands.", "The robe shop on the north side." },
    { 5840.61, 647.844, 658.404, 0.09, "Up the stairs where people sit and talk.", "The lounge just east of the square." },
    { 5813.84, 449.122, 658.752, 1.16, "Where the flyers land.", "The duel corner, at the city's southern tip." },
    { 5915.89, 694.852, 642.426, 5.74, "Hammers on anvils.", "The smithy in the north-east." },
}
local HIDE_S = 600
local HINT1_S, HINT2_S = 180, 360
local FIND_YARDS = 4
local HIDER = math.floor(num("OV_EVENTS_HIDER", 721))

local function hide(by, here, hint)
    if E.active then return false end
    local spot
    if here then
        local x, y, z, o = by:GetX(), by:GetY(), by:GetZ(), by:GetO()
        local h = (hint and hint ~= "") and hint or "A staff member hid it. Look around Dalaran."
        spot = { x, y, z, o, h, h }
    else
        spot = SPOTS[math.random(#SPOTS)]
    end
    local guid, c = nil, nil
    if PerformIngameSpawn then
        local ok, made = pcall(PerformIngameSpawn, 1, HIDER, DALARAN_MAP, 0, spot[1], spot[2], spot[3], spot[4], false, HIDE_S * 1000)
        c = ok and made or nil
    end
    if c then
        guid = try(c, "GetGUID")
        try(c, "SetFaction", 35)          -- friendly to everyone: nobody can kill it
        try(c, "SetNPCFlags", 1)          -- gossip: talk to it to win
        try(c, "SetWanderRadius", 0)
        try(c, "MoveIdle")
    end
    E.active = { kind = "hide", spot = spot, started = now(), guid = guid, hints = 0, by = here and by:GetName() or nil }
    broadcast("Hide and seek in Dalaran: a rabbit is hiding somewhere in the city. Find it and talk to it (or walk "
        .. "right up to it). First hint in 3 minutes. Bragging rights only.", "event")
    return true
end

local function found(p)
    local a = E.active
    if not a or a.kind ~= "hide" or isBot(p) then return false end
    addWin(p, "hide")
    finish(string.format("%s found it! Hide and seek is over. Wins this week: |cffffff00.events top|r.", p:GetName()))
    return true
end

local function hideTick()
    local a = E.active
    local t = now() - a.started
    if t >= HIDE_S then return finish("Nobody found it this time. It was here: " .. a.spot[6]) end
    if a.hints == 0 and t >= HINT1_S then a.hints = 1; broadcast("Hide and seek hint: " .. a.spot[5], "event")
    elseif a.hints == 1 and t >= HINT2_S then a.hints = 2; broadcast("Last hint: " .. a.spot[6], "event") end
    for _, p in ipairs(players()) do
        if try(p, "GetMapId") == DALARAN_MAP and not try(p, "IsDead") then
            local d = try(p, "GetDistance", a.spot[1], a.spot[2], a.spot[3])
            if d and d <= FIND_YARDS then return found(p) end
        end
    end
end

-- stock events from game_event ---------------------------------------------------------------------------------------------
-- Our own names (plain words, no brand); anything else periodic is left out. Holidays use the row's description.
local STOCK_NAMES = {
    [16] = "The arena chest in Stranglethorn: the big chest drops in the arena, fight for it",
    [63] = "The Kalu'ak Fishing Derby: catch the most, turn in at Dalaran",
    [64] = "The Kalu'ak Fishing Derby: catch the most, turn in at Dalaran",
    [14] = "The Stranglethorn Fishing Extravaganza",
    [15] = "The Stranglethorn Fishing Extravaganza",
    [18] = "Call to Arms: a battleground with bonus honor this weekend",
    [19] = "Call to Arms: a battleground with bonus honor this weekend",
    [20] = "Call to Arms: a battleground with bonus honor this weekend",
    [21] = "Call to Arms: a battleground with bonus honor this weekend",
    [53] = "Call to Arms: a battleground with bonus honor this weekend",
    [54] = "Call to Arms: a battleground with bonus honor this weekend",
}

function E.loadStock()
    E.stock, E.holiday = {}, {}
    local q = WorldDBQuery("SELECT eventEntry, UNIX_TIMESTAMP(start_time), UNIX_TIMESTAMP(end_time), occurence, length, "
        .. "holiday, description FROM game_event")
    if not q then return end
    repeat
        local id, start, stop, every, len, hol, desc = q:GetUInt32(0), q:GetUInt32(1), q:GetUInt32(2), q:GetUInt32(3),
            q:GetUInt32(4), q:GetUInt32(5), q:GetString(6)
        if STOCK_NAMES[id] and every > 0 then
            E.stock[#E.stock + 1] = { id = id, name = STOCK_NAMES[id], start = start, every = every * 60, len = len * 60, stop = stop }
        elseif hol > 0 and brandSafe(desc) and desc ~= "" then
            E.holiday[id] = desc
        end
    until not q:NextRow()
end

-- The next start of a periodic stock event at or after time t (nil when it has ended).
function E.nextStart(s, t)
    local n = s.start
    if t > n then n = s.start + math.ceil((t - s.start) / s.every) * s.every end
    if s.stop and s.stop > 0 and n > s.stop then return nil end
    return n
end

-- our own schedule: the next start of each kind at or after t (server local time, on the hour plus a minute)
local function nextOwn(every, offset, at, t)
    local d = os.date("*t", t)
    d.min, d.sec = at, 0
    for _ = 0, 48 do
        local s = os.time(d)
        if s >= t and (d.hour - offset) % every == 0 then return s end
        d.hour = d.hour + 1
        d = os.date("*t", os.time(d))
        d.min, d.sec = at, 0
    end
    return nil
end
function E.upcoming(t)
    local list = {}
    if flag("OV_EVENTS_TRIVIA") then
        list[#list + 1] = { kind = "trivia", name = "Trivia", start = nextOwn(num("OV_EVENTS_TRIVIA_EVERY_H", 2), 0,
            num("OV_EVENTS_TRIVIA_AT", 15), t) }
    end
    if flag("OV_EVENTS_HIDE") then
        list[#list + 1] = { kind = "hide", name = "Hide and seek in Dalaran", start = nextOwn(num("OV_EVENTS_HIDE_EVERY_H", 2),
            num("OV_EVENTS_HIDE_OFFSET_H", 1), num("OV_EVENTS_HIDE_AT", 45), t) }
    end
    if flag("OV_EVENTS_STOCK") then
        local seen = {}
        for _, s in ipairs(E.stock) do
            local n = E.nextStart(s, t)
            if n and not seen[s.name .. n] then
                seen[s.name .. n] = true
                list[#list + 1] = { kind = "stock", name = s.name, start = n, id = s.id }
            end
        end
    end
    local out = {}
    for _, e in ipairs(list) do if e.start then out[#out + 1] = e end end
    table.sort(out, function(a, b) return a.start < b.start end)
    return out
end

local function realPlayers() return #players() end
local function whenText(s, t)
    local m = math.max(0, math.floor((s - t) / 60 + 0.5))
    if m < 1 then return "now" end
    if m < 60 then return "in " .. m .. " min" end
    return string.format("in %dh %02dm", math.floor(m / 60), m % 60)
end

-- A running event, every FAST_MS.
function E.fast()
    local a = E.active
    if not ON or not a then return end
    local t = now()
    if a.kind == "trivia" then
        if a.wait and t >= a.wait then a.wait = nil; askNext()
        elseif a.open and t - a.asked >= QUESTION_S then
            a.open = false
            broadcast("Time's up. It was: " .. a.q.a[1] .. ".", "event")
            a.wait = t + PAUSE_S
        end
    elseif a.kind == "hide" then
        hideTick()
    end
end

-- The schedule, every TICK_MS: "starting soon" before each start, and our own events at their start.
function E.tick()
    if not ON then return end
    local t = now()
    local lead = num("OV_EVENTS_LEAD_MIN", 5) * 60
    local enough = realPlayers() >= num("OV_EVENTS_MIN_PLAYERS", 1)
    for _, e in ipairs(E.upcoming(t - 59)) do   -- from a minute ago: a start just passed is still seen once
        local key = e.kind .. ":" .. (e.id or "") .. ":" .. e.start
        if e.start > t then
            if e.start - t <= lead and not E.soon[key] and (e.kind == "stock" or enough) then
                if broadcast(e.name .. " starts " .. whenText(e.start, t) .. ".", "announce") then E.soon[key] = true end
            end
        elseif e.kind ~= "stock" and not E.soon["go:" .. key] then
            E.soon["go:" .. key] = true
            if enough and not E.active then
                if e.kind == "trivia" then trivia() else hide() end
            end
        end
    end
end

local function onGameEventStart(event, id)
    if not ON or not flag("OV_EVENTS_STOCK") then return end
    local d = E.holiday[id]
    if d then broadcast(d .. " has begun.", "announce") end
end

-- commands ------------------------------------------------------------------------------------------------------------------
local function showNext(p)
    if not ON then return say(p, "Events are off on this realm.") end
    local t, lines = now(), {}
    if E.active then lines[#lines + 1] = (E.active.kind == "trivia" and "Trivia" or "Hide and seek") .. " is on now" end
    for i, e in ipairs(E.upcoming(t)) do
        if i > 3 then break end
        lines[#lines + 1] = e.name:gsub(":.*$", "") .. " " .. whenText(e.start, t)
    end
    if #lines == 0 then return say(p, "Nothing scheduled right now.") end
    say(p, "Next: " .. table.concat(lines, ".  ") .. ".  Wins: |cffffff00.events top|r. Quiet: |cffffff00.events off|r.")
end

local function onCommand(event, p, command)
    if not p then return end
    local raw = (command or ""):gsub("^[%.!]", "")
    local verb, rest = raw:match("^(%S+)%s*(.*)$")
    verb = (verb or ""):lower()
    if verb == "ans" or verb == "answer" then
        if not ON then return end
        answer(p, rest)
        return false
    end
    if verb ~= "events" and verb ~= "event" then return end
    local sub, arg = rest:match("^(%S*)%s*(.*)$")
    sub = (sub or ""):lower()
    local gm = (try(p, "GetGMRank") or 0) >= 2
    if sub == "" or sub == "next" then showNext(p)
    elseif sub == "top" then showTop(p)
    elseif sub == "off" then E.muted[p:GetGUIDLow()] = true; say(p, "Event announcements off for you (|cffffff00.events on|r turns them back on).")
    elseif sub == "on" then E.muted[p:GetGUIDLow()] = nil; say(p, "Event announcements on.")
    elseif gm and sub == "trivia" then if not trivia(p) then say(p, "An event is already running (|cffffff00.events stop|r).") end
    elseif gm and sub == "hide" then
        local here, hint = arg:match("^(here)%s*(.*)$")
        if here and not E.inDalaran(p) then say(p, "Stand in Dalaran to hide it here.")
        elseif not hide(p, here ~= nil, hint) then say(p, "An event is already running (|cffffff00.events stop|r).") end
    elseif gm and sub == "stop" then
        if E.active then finish("The event was stopped by staff.") else say(p, "Nothing is running.") end
    else showNext(p) end
    return false
end

-- "#ans <answer>" said in chat (say, yell, party, raid, guild): the familiar way to answer
local function onChat(event, p, msg)
    if not ON or not E.active or E.active.kind ~= "trivia" then return end
    local a = (msg or ""):match("^#ans%s+(.+)$")
    if a then answer(p, a) end
end

local function onGossip(event, p, creature)
    local a = E.active
    if not a or a.kind ~= "hide" or not a.guid then return end
    if try(creature, "GetGUID") == a.guid then
        found(p)
        try(p, "GossipComplete")
        return false
    end
end

if ON then
    math.randomseed(os.time())
    E.loadStock()
    -- a tip for ov_qol.lua's login tips (they take turns)
    OV_EXTRA_TIPS = rawget(_G, "OV_EXTRA_TIPS") or {}
    table.insert(OV_EXTRA_TIPS, { text = "|cffffff00.events|r shows what's on next: trivia, hide and seek, the arena chest, fishing." })
    if CreateLuaEvent then
        CreateLuaEvent(function() E.tick() end, TICK_MS, 0)
        CreateLuaEvent(function() E.fast() end, FAST_MS, 0)
        CreateLuaEvent(function() E.loadStock() end, 3600000, 0)
    end
    RegisterServerEvent(34, onGameEventStart)      -- GAME_EVENT_START
    RegisterPlayerEvent(18, onChat)                -- PLAYER_EVENT_ON_CHAT
    if RegisterCreatureGossipEvent then RegisterCreatureGossipEvent(HIDER, 1, onGossip) end   -- GOSSIP_EVENT_ON_HELLO
end
RegisterPlayerEvent(42, onCommand)                 -- PLAYER_EVENT_ON_COMMAND (says "off" when the clock is off)

E.QUESTIONS, E.SPOTS, E.STOCK_NAMES, E.HOST_LINE = QUESTIONS, SPOTS, STOCK_NAMES, HOST_LINE
E.trivia, E.hide, E.answer, E.rightAnswer, E.norm, E.broadcast, E.found, E.onCommand, E.onChat, E.onGossip =
    trivia, hide, answer, rightAnswer, norm, broadcast, found, onCommand, onChat, onGossip
E.TICK_MS, E.FAST_MS, E.HIDE_S, E.HINT1_S, E.HINT2_S, E.QUESTION_S, E.PAUSE_S = TICK_MS, FAST_MS, HIDE_S, HINT1_S, HINT2_S, QUESTION_S, PAUSE_S
E.onGameEventStart, E.ON = onGameEventStart, ON
OV_EVENTS_TEST = E
