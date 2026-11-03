-- lua5.1 stream/tests/test_ov_buildrealm.lua [ov_buildrealm.lua]
-- The Builders' Realm's door (OV_BUILDREALM=1, realm/profiles/build.env) under a fake mod-ale: GMs, the main
-- realm's builders (a cross-database read of acore_characters.mod_builder_roles) and accounts allowed here come in
-- and get the builder role; everyone else is sent away (or, with OV_BUILDREALM_VISITORS=1, looks around and loses a
-- leftover role); .buildrealm allow / deny / list are GM only; account names can't carry SQL; off without the switch;
-- no game-brand names in anything players see.
local file = arg[1] or "realm/custom/lua_scripts/ov_buildrealm.lua"

local pass, fail = 0, 0
local realPrint = print
local function check(name, ok)
    if ok then pass = pass + 1; realPrint("PASS " .. name) else fail = fail + 1; realPrint("FAIL " .. name) end
end

local env
os.getenv = function(k) return env[k] end
os.time = function() return 1000 end

local events, sql, said, kicked
local mainBuilders, allowed, accounts, builders, online
local function result(rows)
    if #rows == 0 then return nil end
    local r = { rows = rows, i = 1 }
    function r:GetUInt32(c) return tonumber(self.rows[self.i][c + 1]) end
    function r:GetString(c) return tostring(self.rows[self.i][c + 1]) end
    function r:NextRow() self.i = self.i + 1; return self.i <= #self.rows end
    return r
end

local function reset(e)
    env = e
    events, sql, said, kicked = {}, {}, {}, {}
    mainBuilders, allowed, builders, online = {}, {}, {}, {}
    accounts = { ANN = 11, BOB = 12, CAT = 13 }
    _G.OVBuild = {
        Builders = function() local o = {}; for n in pairs(builders) do o[#o + 1] = n end; return o end,
        SetBuilder = function(by, name, on) builders[name] = on or nil; return true end,
    }
    _G.OV_BUILDREALM_TEST = nil
    function RegisterPlayerEvent(id, fn) events[id] = fn end
    function CharDBExecute(s)
        sql[#sql + 1] = s
        local a, u = s:match("^REPLACE INTO mod_buildrealm_access %(account, username, by_name, at%) VALUES %((%d+), '(%w+)'")
        if a then allowed[tonumber(a)] = u end
        local d = s:match("^DELETE FROM mod_buildrealm_access WHERE account = (%d+)$")
        if d then allowed[tonumber(d)] = nil end
    end
    function CharDBQuery(s)
        sql[#sql + 1] = s
        local a = s:match("FROM mod_buildrealm_access WHERE account = (%d+)$")
        if a then return allowed[tonumber(a)] and result({ { 1 } }) or nil end
        local m = s:match("^SELECT 1 FROM `acore_characters`%.mod_builder_roles r JOIN `acore_characters`%.characters c ON c%.guid = r%.guid WHERE c%.account = (%d+) LIMIT 1$")
        if m then return mainBuilders[tonumber(m)] and result({ { 1 } }) or nil end
        if s:match("^SELECT username FROM mod_buildrealm_access ORDER BY username$") then
            local rows = {}
            for _, u in pairs(allowed) do rows[#rows + 1] = { u } end
            table.sort(rows, function(x, y) return x[1] < y[1] end)
            return result(rows)
        end
        return nil
    end
    function AuthDBQuery(s)
        sql[#sql + 1] = s
        local n = s:match("^SELECT id, username FROM account WHERE username = '(%w+)'$")
        if n and accounts[n] then return result({ { accounts[n], n } }) end
        return nil
    end
    function GetPlayersInWorld() return online end
    local f = assert(loadfile(file))
    f()
end

local function player(name, acct, gm)
    local p = { name = name, acct = acct, gm = gm or 0 }
    function p:GetName() return self.name end
    function p:GetAccountId() return self.acct end
    function p:GetGMRank() return self.gm end
    function p:SendBroadcastMessage(t) said[#said + 1] = { self.name, t } end
    function p:KickPlayer() kicked[self.name] = true end
    function p:RegisterEvent(fn, delay, rep) fn(nil, delay, rep, self) return 1 end
    return p
end
local function lastSaid(name) for i = #said, 1, -1 do if said[i][1] == name then return said[i][2] end end return "" end
local function cmd(p, text) return events[42](42, p, text) end

-- off without the switch
reset({})
check("off: no events without OV_BUILDREALM=1", events[3] == nil and events[42] == nil and #sql == 0)

reset({ OV_BUILDREALM = "1" })
check("on: login and command events, its own table", events[3] and events[42]
    and sql[1]:match("^CREATE TABLE IF NOT EXISTS mod_buildrealm_access"))

-- a GM comes in, no role needed
local gm = player("Keeper", 1, 3)
events[3](3, gm)
check("GM: welcomed, not kicked, no role row", not kicked.Keeper and builders.Keeper == nil and lastSaid("Keeper"):find("Welcome"))

-- a builder on the main realm: in, and gets the role here
mainBuilders[11] = true
local ann = player("Ann", 11)
events[3](3, ann)
check("main realm builder: in, builder role here", not kicked.Ann and builders.Ann == true and lastSaid("Ann"):find("F7"))
check("main realm builder: the cross-database read names acore_characters", (function()
    for _, s in ipairs(sql) do if s:find("`acore_characters`.mod_builder_roles", 1, true) then return true end end
    return false end)())

-- a stranger: told and logged out
local bob = player("Bob", 12)
events[3](3, bob)
check("stranger: sent away", kicked.Bob == true and lastSaid("Bob"):find("approved builders") and builders.Bob == nil)

-- the owner allows Bob's account; Bob online gets the role at once
online = { bob }
kicked = {}
cmd(gm, ".buildrealm allow bob")
check("allow: stored by account, Bob builds now", allowed[12] == "BOB" and builders.Bob == true and lastSaid("Keeper"):find("BOB may build"))
events[3](3, bob)
check("allowed account: in at the next login", not kicked.Bob and builders.Bob == true)
cmd(gm, ".buildrealm list")
check("list: the allowed accounts", lastSaid("Keeper"):find("Allowed here: BOB"))

-- deny: role gone, sent away
cmd(gm, ".buildrealm deny BOB")
check("deny: row gone, role gone, Bob sent away", allowed[12] == nil and builders.Bob == nil and kicked.Bob == true)

-- not for players
local before = #sql
check("players can't allow", cmd(ann, ".buildrealm allow cat") == false and allowed[13] == nil and lastSaid("Ann"):find("Only the realm's team"))
cmd(ann, ".buildrealm")
check(".buildrealm alone: your standing", lastSaid("Ann"):find("builder on the main realm"))

-- account names can't carry SQL
local n = #sql
cmd(gm, ".buildrealm allow x';DROP")
check("bad account names: refused, no query", #sql == n and lastSaid("Keeper"):find("Usage"))
cmd(gm, ".buildrealm allow nobody")
check("unknown account: says so", lastSaid("Keeper"):find("No account called NOBODY"))
check("other commands pass through", cmd(gm, ".gm on") == nil)

-- publish: the room as a saved build on the main realm, under a code
OVBuild.DenLayout = function() return { props = {
    { entry = 19006, x = 0, y = 24, z = 0, yaw = 90 },
    { entry = 2413, x = 3, y = -3.5, z = 1.25, yaw = 0, scale = 1.5 } }, paths = {} } end
local n0 = #sql
cmd(ann, ".buildrealm publish house")
check("publish: GM only", #sql == n0 or not sql[#sql]:find("mod_builder_builds", 1, true))
cmd(gm, ".buildrealm publish house")
local ins
for _, s in ipairs(sql) do if s:find("^INSERT INTO `acore_characters`%.mod_builder_builds") then ins = s end end
check("publish: one row on the main realm, owner 0, a 6-letter code", ins and ins:find("VALUES %(0, 'Builders', 'house', '[A-HJ-NP-Z2-9]+', ")
    and ins:match("'([A-HJ-NP-Z2-9]+)', '") and #ins:match("'house', '([A-HJ-NP-Z2-9]+)'") == 6)
check("publish: props as entry,x,y,z,turn (radians)", ins and ins:find("'19006,0.00,24.00,0.00,1.571;2413,3.00,-3.50,1.25,0.000'", 1, true))
check("publish: says the code to load there", lastSaid("Keeper"):find("%.build load [A-HJ-NP-Z2-9]+") and lastSaid("Keeper"):find("2 props"))
cmd(gm, ".buildrealm publish bad'name")
check("publish: bad names refused", lastSaid("Keeper"):find("Usage"))
OVBuild.DenLayout = function() return { props = {}, paths = {} } end
cmd(gm, ".buildrealm publish empty")
check("publish: an empty room says so", lastSaid("Keeper"):find("empty"))

-- visitors on: a stranger looks around, a leftover role is taken back
reset({ OV_BUILDREALM = "1", OV_BUILDREALM_VISITORS = "1" })
builders.Cat = true
local cat = player("Cat", 13)
events[3](3, cat)
check("visitors on: Cat stays, without the builder role", not kicked.Cat and builders.Cat == nil and lastSaid("Cat"):find("visiting"))

-- a bad database name falls back
reset({ OV_BUILDREALM = "1", OV_BUILDREALM_MAIN_CHARS = "x`; DROP" })
check("bad main database name: the default", OV_BUILDREALM_TEST.MAIN_CHARS == "acore_characters")

-- words players see
local src = assert(io.open(file)):read("*a")
local shown = {}
for s in src:gmatch('"([^"]*)"') do shown[#shown + 1] = s end
local txt = table.concat(shown, " ")
check("no game-brand names in player text", not txt:find("World of Warcraft") and not txt:find("Warcraft")
    and not txt:find("WoW") and not txt:find("Blizzard") and not txt:find("Lich King"))

realPrint(string.format("%d passed, %d failed", pass, fail))
os.exit(fail == 0 and 0 or 1)
