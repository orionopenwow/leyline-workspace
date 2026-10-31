-- lua5.1 stream/tests/test_ov_names.lua [ov_names.lua]
-- The realm's name guard (realm/custom/lua_scripts/ov_names.lua) under a fake mod-ale: names that trip the list (look-
-- alikes, digits, separators, run-together words) and the ones that must pass (Scunthorpe, Cassandra, Grape,
-- Therapist...), a new character and a login flagged and set to rename (nothing deleted), a name that passes for the
-- team flagged for a GM only (and a GM's own character left alone), a guild flagged, public chat starred (item links
-- kept whole, whispers untouched), the console (list, rename, done, scan, check, reload; players never reach it),
-- the owner's words from ov_blocked_words, quotes never reaching SQL, OV_NAMES=0 off.
local file = arg[1] or "realm/custom/lua_scripts/ov_names.lua"

local pass, fail = 0, 0
local realPrint = print
local function check(name, ok)
    if ok then pass = pass + 1; realPrint("PASS " .. name) else fail = fail + 1; realPrint("FAIL " .. name) end
end
print = function() end

local clock = 1800000000
os.time = function() return clock end
local env = {}
os.getenv = function(k) return env[k] end

local events, guildEvents, luaEvents = {}, {}, {}
function RegisterPlayerEvent(id, fn) events[id] = events[id] or {}; table.insert(events[id], fn) end
function RegisterGuildEvent(id, fn) guildEvents[id] = fn end
function CreateLuaEvent(fn, delay, rep) luaEvents[#luaEvents + 1] = { fn = fn, delay = delay } end

local sql = {}
local blocked = { { "grumbo", "word" } }
local chars = { { 11, "Bob" }, { 12, "N1gg3r" }, { 13, "Cassandra" }, { 14, "LeylineGM" }, { 15, "Grape" } }
local function result(rows)
    if not rows or #rows == 0 then return nil end
    local r = { rows = rows, i = 1 }
    function r:GetUInt32(c) return tonumber(self.rows[self.i][c + 1]) end
    function r:GetString(c) return tostring(self.rows[self.i][c + 1] or "") end
    function r:NextRow() self.i = self.i + 1; return self.i <= #self.rows end
    return r
end
local flags = {}
function CharDBExecute(s) sql[#sql + 1] = s end
function CharDBQuery(q)
    if q:find("FROM ov_blocked_words", 1, true) then return result(blocked) end
    if q:find("FROM ov_name_flags", 1, true) then return result(flags) end
    if q:find("SELECT name FROM characters WHERE guid = 12", 1, true) then return result({ { "N1gg3r" } }) end
    if q:find("SELECT guid, name FROM characters", 1, true) then return result(chars) end
    return nil
end
local online = {}
function GetPlayerByName(n) return online[n] end

local function player(name, gm)
    local p = { name = name, guid = 0, gm = gm or 0, msgs = {}, flags = 0, saved = 0 }
    function p:GetName() return self.name end
    function p:GetGUIDLow() return self.guid end
    function p:GetGMRank() return self.gm end
    function p:SetAtLoginFlag(f) self.flags = self.flags + f end
    function p:SaveToDB() self.saved = self.saved + 1 end
    function p:SendBroadcastMessage(t) self.msgs[#self.msgs + 1] = t end
    return p
end
local function fire(id, ...)
    local a, b
    for _, fn in ipairs(events[id] or {}) do local x, y = fn(id, ...); if x ~= nil then a = x end; if y ~= nil then b = y end end
    return a, b
end
local function lastSql() return sql[#sql] or "" end
local function anySql(pat) for _, s in ipairs(sql) do if s:find(pat) then return true end end return false end

local N = dofile(file)
check("loads, a global for the other scripts (OVNames)", type(N) == "table" and OVNames == N)
check("makes its two tables (CREATE TABLE IF NOT EXISTS)", anySql("CREATE TABLE IF NOT EXISTS ov_blocked_words")
    and anySql("CREATE TABLE IF NOT EXISTS ov_name_flags"))

-- ---------------------------------------------------------------- names
local bad = { ["Nigger"] = "slur", ["N1gg3r"] = "slur", ["Niiiggerrr"] = "slur", ["Fagz"] = "slur", ["Kike"] = "slur",
    ["Shithead"] = "profanity", ["Fuuuck"] = "profanity", ["Therapistrapist"] = "sexual", ["TheRapist"] = "sexual",
    ["Admin"] = "staff", ["Adm1n"] = "staff", ["Admiiin"] = "staff", ["\208\144dmin"] = "staff", ["Moderat0r"] = "staff",
    ["LeylineStaff"] = "staff", ["L3yl1n3"] = "staff", ["GMBob"] = "staff", ["BobGM"] = "staff", ["Gamemaster"] = "staff",
    ["\195\129dm\195\173n"] = "staff", ["Le\226\128\139yline"] = "staff", ["Owner"] = "staff", ["Warcraft"] = "brand",
    ["Grumbo"] = "custom", ["Dildomage"] = "sexual", ["Pornstar"] = "sexual", ["Cocks"] = "sexual" }
for name, kind in pairs(bad) do
    local k = N.NameHit(name)
    check("refused: " .. name:gsub("[\128-\255]", "?") .. " (" .. kind .. ")", k == kind)
end
for _, name in ipairs({ "Scunthorpe", "Cassandra", "Grape", "Therapist", "Bob", "Arthas", "Niger", "Snigger", "Badminton",
        "Gmork", "Staffan", "Dickens", "Peacock", "Hancock", "Sussex", "Analyst", "Shiitake", "Penistone", "Toshitaka",
        "Spices", "Cocker", "Officially", "Thrall", "Jaina", "Classic", "Assassin", "Raccoon", "Japan", "Kassandra" }) do
    check("passes: " .. name, N.NameHit(name) == nil)
end

-- ---------------------------------------------------------------- chat
local a, m = fire(18, player("Ann"), "what the fuck is this", 1, 0)
check("say: a word starred out, the same length, not blocked", a == nil and m == "what the **** is this")
a, m = fire(18, player("Ann"), "I love grape juice in Scunthorpe", 1, 0)
check("say: a clean line is left alone (nothing returned)", a == nil and m == nil)
a, m = fire(22, player("Ann"), "selling |cff0070dd|Hitem:1234:0:0:0|h[Shitty Sword]|h|r cheap, n1gg3r", 1, 0, 2)
check("channel: an item link kept whole, the rest starred",
    m == "selling |cff0070dd|Hitem:1234:0:0:0|h[Shitty Sword]|h|r cheap, ******")
a, m = fire(22, player("Ann"), "f u c k you", 1, 0, 2)
check("channel: spaced-out letters starred", m == "* * * * you")
check("whispers, party and guild chat untouched (no hook for 19, 20, 21)", events[19] == nil and events[20] == nil and events[21] == nil)

-- ---------------------------------------------------------------- create and login
sql = {}
local p = player("N1gg3r"); p.guid = 501
fire(1, p)
check("create: flagged in ov_name_flags", anySql("INSERT INTO ov_name_flags .*'char', 501, 'N1gg3r', 'slur'"))
check("create: the rename flag set, and again in the database a moment later", p.flags == 1 and #luaEvents == 1)
luaEvents[1].fn()
check("create: the delayed UPDATE sets at_login's rename bit (nothing deleted)",
    lastSql():find("UPDATE characters SET at_login = at_login | 1 WHERE guid = 501", 1, true) ~= nil and not anySql("DELETE"))

sql = {}
p = player("Shithead"); p.guid = 502
fire(3, p)
check("login: flagged, rename set and saved, told why", anySql("'char', 502, 'Shithead', 'profanity'") and p.flags == 1
    and p.saved == 1 and #p.msgs == 1 and p.msgs[1]:find("pick a new name", 1, true) ~= nil)
check("login: the message names no word", not p.msgs[1]:lower():find("shit", 1, true))

sql = {}
p = player("LeylineGM"); p.guid = 503
fire(3, p)
check("login: a team-looking name is flagged for a GM only (no rename, no message)",
    anySql("'char', 503, 'LeylineGM', 'staff'") and p.flags == 0 and #p.msgs == 0)
sql = {}
p = player("LeylineGM", 3); p.guid = 504
fire(3, p)
check("login: a GM's own character with a team name is left alone", #sql == 0 and p.flags == 0)
sql = {}
p = player("Cassandra"); p.guid = 505
fire(3, p)
check("login: a clean name: nothing written", #sql == 0 and p.flags == 0)

sql = {}
local guild = { GetId = function() return 77 end }
local leader = player("Bob")
guildEvents[5](5, guild, leader, "Rapist Club")
check("guild create: flagged for a GM, the leader told", anySql("'guild', 77, 'Rapist Club', 'sexual'") and #leader.msgs == 1)

-- ---------------------------------------------------------------- the console
local out = {}
local handler = { SendSysMessage = function(_, line) out[#out + 1] = line end }
local function console(c) out = {}; local r = fire(42, nil, c, handler); return r end
flags = { { "char", 502, "Shithead", "profanity", clock, "open" } }
check("console: handled (false)", console("ovnames list") == false)
check("console list: a ROW and the end line", out[1] == "ROW\tchar\t502\tShithead\tprofanity\t" .. clock .. "\topen"
    and out[2] == "ovadmin end 1")
console("ovnames list bogus")
check("console list: a bad state refused", out[1]:find("^ovadmin error") ~= nil)
sql = {}
local live = player("N1gg3r"); online["N1gg3r"] = live
console("ovnames rename 12")
check("console rename: at_login set, the online character told, status renamed",
    anySql("UPDATE characters SET at_login = at_login | 1 WHERE guid = 12") and live.flags == 1 and #live.msgs == 1
    and anySql("status = 'renamed'") and out[#out] == "ovadmin end 0")
console("ovnames rename 12; DROP TABLE characters")
check("console rename: junk after the guid ignored (tonumber)", not anySql("DROP"))
console("ovnames rename abc")
check("console rename: not a number refused", out[1]:find("^ovadmin error") ~= nil)
sql = {}
console("ovnames done char 502")
check("console done", lastSql():find("SET status = 'done' WHERE what = 'char' AND id = 502", 1, true) ~= nil)
console("ovnames done table 5")
check("console done: only char or guild", out[1]:find("^ovadmin error") ~= nil)
sql = {}
console("ovnames scan 0")
check("console scan: flags the bad names among the made ones, says where to go on",
    anySql("'char', 12, 'N1gg3r', 'slur'") and anySql("'char', 14, 'LeylineGM', 'staff'") and not anySql("'Cassandra'")
    and not anySql("'Grape'") and out[1] == "ovadmin next 15" and out[2] == "ovadmin end 2")
console("ovnames check Adm1n")
check("console check", out[1] == "ovadmin name staff")
console("ovnames check Cassandra")
check("console check: ok", out[1] == "ovadmin name ok")
blocked = { { "grumbo", "word" }, { "zorb", "inside" } }
console("ovnames reload")
check("console reload: the owner's words read again", out[1] == "ovadmin ok: 2 of the owner's words" and N.NameHit("Xzorbx") == "custom")
out = {}
check("a player never reaches the console", fire(42, player("Bob"), "ovnames list", handler) == nil and #out == 0)
check("other console commands are not ours", console("ovadmin ping") == nil and #out == 0)

sql = {}
p = player("Fu'ck"); p.guid = 506
fire(3, p)
check("a quote in a name never reaches SQL", anySql("'char', 506, 'Fu ck'") and not anySql("Fu'ck"))

-- ---------------------------------------------------------------- off
env.OV_NAMES = "0"
events, guildEvents = {}, {}
local N2 = dofile(file)
sql = {}
p = player("N1gg3r"); p.guid = 507
fire(3, p)
local x, y = fire(18, player("Ann"), "fuck", 1, 0)
check("OV_NAMES=0: nothing flagged, chat left alone", #sql == 0 and p.flags == 0 and y == nil)

realPrint(string.format("%d passed, %d failed", pass, fail))
os.exit(fail == 0 and 0 or 1)
