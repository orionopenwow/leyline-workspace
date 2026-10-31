-- ov_duels.lua: duel ratings and points for Wrath Unleashed (mod-ale, 3.3.5a). Off unless OV_DUELS=1 (main.env).
-- Owner 2026-10-31: "Working in game duels points, leaderboards on site".
--
-- What it does
--   * Counts finished duels (the duel-start and duel-end hooks): who won, who lost, how it ended and how long it took.
--   * A rating per character (Elo: everyone starts at 1500; a win against a higher rating is worth more).
--   * Duel points per week (the season: Monday 00:00 UTC) and all time: a win 3, a loss 1 (only rated duels).
--   * Anti-farming. A duel changes nothing (no rating, no points, no win or loss) when:
--       - it was interrupted (someone else stepped in, a zone change, a logout), or a player fled (ran out of the
--         flag's range) before OV_DUELS_FLED_MIN_S of fighting;
--       - it was won in under OV_DUELS_MIN_S seconds (a /yield or an instant kill on a fresh alt);
--       - both characters are on the same account (an alt farming its main);
--       - the same two characters already had OV_DUELS_PAIR_PER_DAY rated duels today (UTC);
--       - either one is below OV_DUELS_MIN_LEVEL (80), or they are in different level brackets (when the minimum is
--         lower: 10-19, 20-29 ... 70-79, 80);
--       - either one is a bot, or a GM with GM mode on.
--     The players are told why, in one short line.
--   * Win streaks: a short line to the players in the same zone at 3, 5, 10, 15 ... wins in a row (rate-limited).
--   * The site's leaderboards (/wrath/leaderboards) read these tables through realm/custom/duels-export.py.
--
-- Player commands: .duel (help), .duel stats [name], .duel top (this week's points), .duel top all (ratings),
--   .duel quiet | loud (streak lines off or on for you).
-- Env (all off unless set; realm/profiles/main.env turns OV_DUELS on): OV_DUELS; OV_DUELS_K (32: the K-factor),
--   OV_DUELS_MIN_S (15), OV_DUELS_FLED_MIN_S (30), OV_DUELS_PAIR_PER_DAY (3), OV_DUELS_MIN_LEVEL (80),
--   OV_DUELS_WIN_POINTS (3), OV_DUELS_LOSS_POINTS (1), OV_DUELS_STREAKS (1: the streak lines; 0 turns them off).
-- Tables (characters database, made here with CREATE TABLE IF NOT EXISTS before anything reads them; this script
--   never reads a table it did not make: a query on a missing table aborts the whole world server, MySQL 1146):
--   ov_duel_stats (all time, one row per character), ov_duel_week (points per week), ov_duel_log (every duel).
-- mod-ale (84b85cc): RegisterPlayerEvent 10 ON_DUEL_START (event, player1, player2), 11 ON_DUEL_END (event, winner,
--   loser, type: 0 interrupted, 1 won, 2 fled), 42 ON_COMMAND; Player:GetAccountId, GetGUIDLow, GetName, GetClass,
--   GetLevel, IsGM, GetZoneId; GetPlayersInWorld; CharDBQuery / CharDBExecute.

local function flag(name) return os.getenv(name) == "1" end
local function num(name, default)
    local v = tonumber(os.getenv(name) or "")
    return v or default
end

local ON = flag("OV_DUELS")
local TAG = "|cffff8040[Duels]|r "
local START_RATING = 1500
local MIN_RATING = 100
local PLACED_AFTER = 3          -- rated duels before a character is on the all-time board
local STREAK_GAP_S = 30         -- streak lines at least this far apart, realm-wide
local SITE = "play.openvanilla.app/wrath/leaderboards"
local DUEL_INTERRUPTED, DUEL_WON, DUEL_FLED = 0, 1, 2
local KIND_NAME = { [DUEL_INTERRUPTED] = "interrupted", [DUEL_WON] = "won", [DUEL_FLED] = "fled" }

local D = {}           -- state, exposed for tests as OV_DUELS_TEST
D.started = {}         -- pair key -> os.time() the fight began
D.stats = {}           -- guid -> all-time row (loaded once, then kept here; writes carry absolute values)
D.week = {}            -- "<week>|<guid>" -> { points, wins, losses }
D.pairs = {}           -- "<day>|<pair>" -> rated duels today
D.quiet = {}           -- guid -> true (.duel quiet)
D.lastStreak = 0
D.ready = false

local function now() return os.time() end
local function try(obj, method, ...)
    if not obj then return nil end
    local ok, fn = pcall(function() return obj[method] end)
    if not ok or type(fn) ~= "function" then return nil end
    local ok2, a = pcall(fn, obj, ...)
    if ok2 then return a end
    return nil
end
local function isBot(p)
    if try(p, "IsBot") == true or try(p, "IsPlayerbot") == true then return true end
    local name = try(p, "GetAccountName")
    return type(name) == "string" and name:upper():find("^RNDBOT") ~= nil
end
local function say(p, text) if p then try(p, "SendBroadcastMessage", TAG .. text) end end
local function esc(s) return (tostring(s or ""):gsub("[%z\1-\31]", ""):gsub("\\", "\\\\"):gsub("'", "''")) end
local function weekKey(t) return os.date("!%Y-%W", t or now()) end      -- the season: Monday 00:00 UTC
local function dayKey(t) return os.date("!%Y-%m-%d", t or now()) end
local function pairKey(a, b)
    if a > b then a, b = b, a end
    return a .. ":" .. b
end

-- tables: made first, and checked before any read ----------------------------------------------------------------
local SCHEMA = {
    "CREATE TABLE IF NOT EXISTS ov_duel_stats (guid INT UNSIGNED NOT NULL PRIMARY KEY, name VARCHAR(12) NOT NULL, "
        .. "class TINYINT UNSIGNED NOT NULL DEFAULT 0, rating SMALLINT UNSIGNED NOT NULL DEFAULT 1500, "
        .. "best SMALLINT UNSIGNED NOT NULL DEFAULT 1500, wins INT UNSIGNED NOT NULL DEFAULT 0, "
        .. "losses INT UNSIGNED NOT NULL DEFAULT 0, points INT UNSIGNED NOT NULL DEFAULT 0, "
        .. "streak SMALLINT UNSIGNED NOT NULL DEFAULT 0, best_streak SMALLINT UNSIGNED NOT NULL DEFAULT 0, "
        .. "last_at INT UNSIGNED NOT NULL DEFAULT 0, KEY name (name), KEY rating (rating))",
    "CREATE TABLE IF NOT EXISTS ov_duel_week (week CHAR(7) NOT NULL, guid INT UNSIGNED NOT NULL, name VARCHAR(12) NOT NULL, "
        .. "class TINYINT UNSIGNED NOT NULL DEFAULT 0, points INT UNSIGNED NOT NULL DEFAULT 0, "
        .. "wins INT UNSIGNED NOT NULL DEFAULT 0, losses INT UNSIGNED NOT NULL DEFAULT 0, PRIMARY KEY (week, guid), "
        .. "KEY week_points (week, points))",
    "CREATE TABLE IF NOT EXISTS ov_duel_log (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, at INT UNSIGNED NOT NULL, "
        .. "day CHAR(10) NOT NULL, winner INT UNSIGNED NOT NULL, loser INT UNSIGNED NOT NULL, pair VARCHAR(24) NOT NULL, "
        .. "kind TINYINT UNSIGNED NOT NULL, secs INT UNSIGNED NOT NULL DEFAULT 0, rated TINYINT UNSIGNED NOT NULL DEFAULT 0, "
        .. "delta SMALLINT NOT NULL DEFAULT 0, KEY pair_day (pair, day))",
}
local TABLES = { "ov_duel_stats", "ov_duel_week", "ov_duel_log" }
local function createTables() for _, sql in ipairs(SCHEMA) do CharDBExecute(sql) end end

-- true once all three tables exist. Asks information_schema (always there), never the tables themselves: a read of a
-- missing table would take the whole world server down. CharDBExecute is queued, so the first check may come back
-- short; then the tables are asked for again and the duel is left uncounted.
function D.isReady()
    if D.ready then return true end
    local ok, q = pcall(CharDBQuery, "SELECT COUNT(*) FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() "
        .. "AND TABLE_NAME IN ('" .. table.concat(TABLES, "', '") .. "')")
    if ok and q and tonumber(q:GetUInt32(0)) == #TABLES then D.ready = true; return true end
    createTables()
    return false
end

-- rating math ------------------------------------------------------------------------------------------------------
function D.expected(ra, rb) return 1 / (1 + 10 ^ ((rb - ra) / 400)) end
-- The points a win moves from the loser to the winner: at least 1, K at most.
function D.delta(winnerRating, loserRating, k)
    k = k or num("OV_DUELS_K", 32)
    local d = math.floor(k * (1 - D.expected(winnerRating, loserRating)) + 0.5)
    if d < 1 then d = 1 end
    if d > k then d = k end
    return d
end

-- rows (memory first; one read per character per load) ---------------------------------------------------------------
local function loadStats(guid)
    local s = D.stats[guid]
    if s then return s end
    s = { rating = START_RATING, best = START_RATING, wins = 0, losses = 0, points = 0, streak = 0, best_streak = 0 }
    local q = CharDBQuery(string.format("SELECT rating, best, wins, losses, points, streak, best_streak FROM ov_duel_stats "
        .. "WHERE guid = %d", guid))
    if q then
        s.rating, s.best, s.wins, s.losses = q:GetUInt32(0), q:GetUInt32(1), q:GetUInt32(2), q:GetUInt32(3)
        s.points, s.streak, s.best_streak = q:GetUInt32(4), q:GetUInt32(5), q:GetUInt32(6)
    end
    D.stats[guid] = s
    return s
end
local function loadWeek(guid, wk)
    local key = wk .. "|" .. guid
    local w = D.week[key]
    if w then return w end
    w = { points = 0, wins = 0, losses = 0 }
    local q = CharDBQuery(string.format("SELECT points, wins, losses FROM ov_duel_week WHERE week = '%s' AND guid = %d", wk, guid))
    if q then w.points, w.wins, w.losses = q:GetUInt32(0), q:GetUInt32(1), q:GetUInt32(2) end
    D.week[key] = w
    return w
end
local function pairToday(pair, day)
    local key = day .. "|" .. pair
    local n = D.pairs[key]
    if n then return n end
    local q = CharDBQuery(string.format("SELECT COUNT(*) FROM ov_duel_log WHERE pair = '%s' AND day = '%s' AND rated = 1", pair, day))
    n = q and q:GetUInt32(0) or 0
    D.pairs[key] = n
    return n
end
local function saveStats(guid, p, s)
    CharDBExecute(string.format("INSERT INTO ov_duel_stats (guid, name, class, rating, best, wins, losses, points, streak, "
        .. "best_streak, last_at) VALUES (%d, '%s', %d, %d, %d, %d, %d, %d, %d, %d, %d) ON DUPLICATE KEY UPDATE name = VALUES(name), "
        .. "class = VALUES(class), rating = VALUES(rating), best = VALUES(best), wins = VALUES(wins), losses = VALUES(losses), "
        .. "points = VALUES(points), streak = VALUES(streak), best_streak = VALUES(best_streak), last_at = VALUES(last_at)",
        guid, esc(p:GetName()), try(p, "GetClass") or 0, s.rating, s.best, s.wins, s.losses, s.points, s.streak, s.best_streak, now()))
end
local function saveWeek(guid, p, wk, w)
    CharDBExecute(string.format("INSERT INTO ov_duel_week (week, guid, name, class, points, wins, losses) VALUES "
        .. "('%s', %d, '%s', %d, %d, %d, %d) ON DUPLICATE KEY UPDATE name = VALUES(name), class = VALUES(class), "
        .. "points = VALUES(points), wins = VALUES(wins), losses = VALUES(losses)",
        wk, guid, esc(p:GetName()), try(p, "GetClass") or 0, w.points, w.wins, w.losses))
end
local function logDuel(w, l, kind, secs, rated, delta, t)
    CharDBExecute(string.format("INSERT INTO ov_duel_log (at, day, winner, loser, pair, kind, secs, rated, delta) VALUES "
        .. "(%d, '%s', %d, %d, '%s', %d, %d, %d, %d)", t, dayKey(t), w, l, pairKey(w, l), kind, secs, rated and 1 or 0, delta))
end

-- the rules --------------------------------------------------------------------------------------------------------
local function bracket(level)
    if level >= 80 then return 8 end
    return math.floor(level / 10)
end

-- Why a finished duel doesn't count (nil when it does). kind: DUEL_*; secs: seconds of fighting (nil: unknown).
function D.whyNot(w, l, kind, secs, t)
    if kind == DUEL_INTERRUPTED then return "The duel was interrupted, so it doesn't count." end
    if secs == nil then return "That duel started before the scores were on, so it doesn't count." end
    if kind == DUEL_FLED and secs < num("OV_DUELS_FLED_MIN_S", 30) then
        return "Someone left the duel area early, so it doesn't count."
    end
    if secs < num("OV_DUELS_MIN_S", 15) then
        return string.format("Under %d seconds: too short to count.", num("OV_DUELS_MIN_S", 15))
    end
    if try(w, "IsGM") == true or try(l, "IsGM") == true then return "GM mode is on, so it doesn't count." end
    local aw, al = try(w, "GetAccountId"), try(l, "GetAccountId")
    if aw ~= nil and aw == al then return "Two characters on one account: it doesn't count." end
    local lw, ll = try(w, "GetLevel") or 0, try(l, "GetLevel") or 0
    local min = num("OV_DUELS_MIN_LEVEL", 80)
    if lw < min or ll < min then
        return string.format("Ratings are for level %d%s only.", min, min >= 80 and "" or " and up")
    end
    if bracket(lw) ~= bracket(ll) then return "Different level brackets: it doesn't count." end
    local cap = num("OV_DUELS_PAIR_PER_DAY", 3)
    if pairToday(pairKey(w:GetGUIDLow(), l:GetGUIDLow()), dayKey(t)) >= cap then
        return string.format("You two had %d rated duels today. More tomorrow (UTC).", cap)
    end
    return nil
end

local function zoneLine(p, text)
    local ok, list = pcall(GetPlayersInWorld)
    local zone = try(p, "GetZoneId")
    for _, o in ipairs(ok and list or {}) do
        if not isBot(o) and try(o, "GetZoneId") == zone and not D.quiet[o:GetGUIDLow()] then say(o, text) end
    end
end
local function streakLine(w, s, l, lostStreak)
    if os.getenv("OV_DUELS_STREAKS") == "0" then return end
    local t = now()
    if t - D.lastStreak < STREAK_GAP_S then return end
    local n = s.streak
    local text
    if lostStreak >= 5 then
        text = string.format("%s ended %s's streak of %d wins!", w:GetName(), l:GetName(), lostStreak)
    elseif n == 3 or n == 5 or (n >= 10 and n % 5 == 0) then
        text = string.format("%s has won %d duels in a row!", w:GetName(), n)
    end
    if not text then return end
    D.lastStreak = t
    zoneLine(w, text)
end

-- One finished duel. Returns true when it was rated.
function D.record(w, l, kind, secs)
    local t = now()
    local wg, lg = w:GetGUIDLow(), l:GetGUIDLow()
    local why = D.whyNot(w, l, kind, secs, t)
    if why then
        if kind ~= DUEL_INTERRUPTED and secs ~= nil then logDuel(wg, lg, kind, secs, false, 0, t) end
        say(w, why); say(l, why)
        return false
    end
    local ws, ls = loadStats(wg), loadStats(lg)
    local wk = weekKey(t)
    local ww, lw = loadWeek(wg, wk), loadWeek(lg, wk)
    local d = D.delta(ws.rating, ls.rating)
    local lostStreak = ls.streak
    ws.rating = ws.rating + d
    ls.rating = math.max(MIN_RATING, ls.rating - d)
    if ws.rating > ws.best then ws.best = ws.rating end
    ws.wins, ls.losses = ws.wins + 1, ls.losses + 1
    ws.streak, ls.streak = ws.streak + 1, 0
    if ws.streak > ws.best_streak then ws.best_streak = ws.streak end
    local wp, lp = num("OV_DUELS_WIN_POINTS", 3), num("OV_DUELS_LOSS_POINTS", 1)
    ws.points, ls.points = ws.points + wp, ls.points + lp
    ww.points, ww.wins = ww.points + wp, ww.wins + 1
    lw.points, lw.losses = lw.points + lp, lw.losses + 1
    local pk = dayKey(t) .. "|" .. pairKey(wg, lg)
    D.pairs[pk] = (D.pairs[pk] or 0) + 1
    saveStats(wg, w, ws); saveStats(lg, l, ls)
    saveWeek(wg, w, wk, ww); saveWeek(lg, l, wk, lw)
    logDuel(wg, lg, kind, secs, true, d, t)
    say(w, string.format("You won: rating %d (+%d), +%d points (%d this week).", ws.rating, d, wp, ww.points))
    say(l, string.format("You lost: rating %d (-%d), +%d point%s (%d this week).", ls.rating, d, lp, lp == 1 and "" or "s", lw.points))
    streakLine(w, ws, l, lostStreak)
    return true
end

-- hooks ------------------------------------------------------------------------------------------------------------
local function onDuelStart(event, p1, p2)
    if not ON or not p1 or not p2 then return end
    D.started[pairKey(p1:GetGUIDLow(), p2:GetGUIDLow())] = now()
end

local function onDuelEnd(event, winner, loser, kind)
    if not ON or not winner or not loser then return end
    local key = pairKey(winner:GetGUIDLow(), loser:GetGUIDLow())
    local began = D.started[key]
    D.started[key] = nil
    if isBot(winner) or isBot(loser) then return end
    if not D.isReady() then return end
    D.record(winner, loser, kind, began and (now() - began) or nil)
end

-- commands ---------------------------------------------------------------------------------------------------------
local function record(s) return string.format("%d-%d", s.wins, s.losses) end
local function showStats(p, name)
    if not D.isReady() then return say(p, "The duel scores are starting up. Try again in a minute.") end
    if name == "" or name:lower() == p:GetName():lower() then
        local s, w = loadStats(p:GetGUIDLow()), loadWeek(p:GetGUIDLow(), weekKey())
        if s.wins + s.losses == 0 then
            return say(p, string.format("No rated duels yet. Duel another level %d player for at least %d seconds: "
                .. "|cffffff00.hub duels|r.", num("OV_DUELS_MIN_LEVEL", 80), num("OV_DUELS_MIN_S", 15)))
        end
        return say(p, string.format("Rating %d (best %d). Wins-losses %s. Streak %d (best %d). This week: %d points, %s.",
            s.rating, s.best, record(s), s.streak, s.best_streak, w.points, record(w)))
    end
    if not name:match("^%a+$") or #name < 2 or #name > 12 then return say(p, "Type a character name: |cffffff00.duel stats|r <name>") end
    local q = CharDBQuery(string.format("SELECT name, rating, best, wins, losses, best_streak FROM ov_duel_stats WHERE name = '%s'", esc(name)))
    if not q then return say(p, name .. " has no rated duels yet.") end
    say(p, string.format("%s: rating %d (best %d). Wins-losses %d-%d. Best streak %d.", q:GetString(0), q:GetUInt32(1),
        q:GetUInt32(2), q:GetUInt32(3), q:GetUInt32(4), q:GetUInt32(5)))
end

local function showTop(p, all)
    if not D.isReady() then return say(p, "The duel scores are starting up. Try again in a minute.") end
    local q
    if all then
        q = CharDBQuery(string.format("SELECT name, rating, wins, losses FROM ov_duel_stats WHERE wins + losses >= %d "
            .. "ORDER BY rating DESC, wins DESC, name LIMIT 5", PLACED_AFTER))
    else
        q = CharDBQuery(string.format("SELECT name, points, wins, losses FROM ov_duel_week WHERE week = '%s' "
            .. "ORDER BY points DESC, wins DESC, name LIMIT 5", weekKey()))
    end
    if not q then
        return say(p, all and "Nobody is on the rating board yet." or "No duel points yet this week. Be the first: |cffffff00.hub duels|r.")
    end
    local rows, i = {}, 1
    repeat
        rows[#rows + 1] = string.format("%d. %s %d", i, q:GetString(0), q:GetUInt32(1))
        i = i + 1
    until not q:NextRow()
    say(p, (all and "Top ratings: " or "This week's duel points: ") .. table.concat(rows, "   ") .. "   More: " .. SITE)
end

local function onCommand(event, p, command)
    if not p then return end
    local raw = (command or ""):gsub("^[%.!]", "")
    local verb, rest = raw:match("^(%S+)%s*(.*)$")
    verb = (verb or ""):lower()
    if verb ~= "duel" and verb ~= "duels" then return end
    if not ON then say(p, "Duel ratings are off on this realm."); return false end
    local sub, arg = rest:match("^(%S*)%s*(.-)%s*$")
    sub = (sub or ""):lower()
    if sub == "stats" or sub == "me" then showStats(p, arg or "")
    elseif sub == "top" then showTop(p, (arg or ""):lower() == "all")
    elseif sub == "quiet" then D.quiet[p:GetGUIDLow()] = true; say(p, "Streak lines off for you (|cffffff00.duel loud|r turns them back on).")
    elseif sub == "loud" then D.quiet[p:GetGUIDLow()] = nil; say(p, "Streak lines on.")
    else
        say(p, "Duels at level 80 count for a rating and points. |cffffff00.duel stats|r [name], |cffffff00.duel top|r (this week), "
            .. "|cffffff00.duel top all|r. Duel corner: |cffffff00.hub duels|r. Boards: " .. SITE)
    end
    return false
end

if ON then
    createTables()
    OV_EXTRA_TIPS = rawget(_G, "OV_EXTRA_TIPS") or {}
    table.insert(OV_EXTRA_TIPS, { text = "Duels at level 80 earn a rating and points: |cffffff00.duel top|r, |cffffff00.hub duels|r.", min = 80 })
    RegisterPlayerEvent(10, onDuelStart)       -- PLAYER_EVENT_ON_DUEL_START
    RegisterPlayerEvent(11, onDuelEnd)         -- PLAYER_EVENT_ON_DUEL_END
end
RegisterPlayerEvent(42, onCommand)             -- PLAYER_EVENT_ON_COMMAND (says "off" when the scores are off)

D.SCHEMA, D.TABLES, D.START_RATING, D.PLACED_AFTER, D.KIND_NAME = SCHEMA, TABLES, START_RATING, PLACED_AFTER, KIND_NAME
D.onDuelStart, D.onDuelEnd, D.onCommand, D.pairKey, D.weekKey, D.dayKey, D.ON = onDuelStart, onDuelEnd, onCommand, pairKey, weekKey, dayKey, ON
OV_DUELS_TEST = D
