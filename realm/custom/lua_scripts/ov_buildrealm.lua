-- ov_buildrealm.lua: the Builders' Realm's door (mod-ale, 3.3.5a). Only on a realm with OV_BUILDREALM=1
-- (realm/profiles/build.env; docs/BUILD-REALM.md). The realm itself is a big Den (ov_den.lua) where ov_build.lua
-- does all the building; this script decides who comes in and who builds.
--
-- Approved builders (all of these, checked at every login, so a change counts at the next login):
--   * GMs (GM level 1 or more for this realm or for all realms: account_access, the same as on the main realm);
--   * every account with a character that has the builder role on the main realm (".build grant <name>" there,
--     acore_characters.mod_builder_roles; OV_BUILDREALM_MAIN_CHARS names that database, read only);
--   * accounts the owner allows here: ".buildrealm allow <account>" (mod_buildrealm_access, this realm's own
--     characters database).
-- An approved builder gets the builder role on each character here as it logs in (OVBuild.SetBuilder). Anyone
-- else: with OV_BUILDREALM_VISITORS=1 they may look around (no building; a builder role left over from earlier is
-- taken back); with 0 (the default) they are told this realm is for approved builders and logged out after a few
-- seconds.
--
--   .buildrealm                    who may build here, and your own standing
--   .buildrealm allow <account>    (GM) let an account build here (its characters on this realm)
--   .buildrealm deny <account>     (GM) take that back (characters online stop building at once)
--   .buildrealm list               (GM) the accounts allowed here (the main realm's builders come on top)
--   .buildrealm publish <name>     (GM) the whole room as a saved build on the main realm, with a 6-letter code:
--                                  there, ".build load <code>" in a build area pastes it (see "Publish" below)
--
-- Env (os.getenv when the script loads): OV_BUILDREALM (1 = on), OV_BUILDREALM_VISITORS (0/1),
-- OV_BUILDREALM_MAIN_CHARS (default acore_characters), OV_BUILDREALM_NAME (default "Builders' Realm"),
-- OV_BUILDREALM_KICK_MS (default 6000).
-- mod-ale used: RegisterPlayerEvent 3 ON_LOGIN, 42 ON_COMMAND; Player:GetAccountId / GetGMRank / GetName /
-- KickPlayer / RegisterEvent / SendBroadcastMessage; GetPlayersInWorld; CharDBQuery / CharDBExecute; AuthDBQuery.

if os.getenv("OV_BUILDREALM") ~= "1" then return end

local TAG = "|cff66ccff[Builders]|r "
local NAME = os.getenv("OV_BUILDREALM_NAME") or "Builders' Realm"   -- TODO(owner): the final name
local VISITORS = os.getenv("OV_BUILDREALM_VISITORS") == "1"
local MAIN_CHARS = os.getenv("OV_BUILDREALM_MAIN_CHARS") or "acore_characters"
if not MAIN_CHARS:match("^[A-Za-z0-9_]+$") then MAIN_CHARS = "acore_characters" end
local KICK_MS = tonumber(os.getenv("OV_BUILDREALM_KICK_MS") or "") or 6000

local function say(player, text) player:SendBroadcastMessage(TAG .. text) end

local function try(obj, method, ...)
    if not obj then return nil end
    local ok, fn = pcall(function() return obj[method] end)
    if not ok or type(fn) ~= "function" then return nil end
    local ok2, a, b = pcall(fn, obj, ...)
    if ok2 then return a, b end
    return nil
end

local function charQuery(sql)
    if not CharDBQuery then return nil end
    local ok, q = pcall(CharDBQuery, sql)
    if ok then return q end
    return nil
end
local function charExec(sql) if CharDBExecute then pcall(CharDBExecute, sql) end end
local function authQuery(sql)
    if not AuthDBQuery then return nil end
    local ok, q = pcall(AuthDBQuery, sql)
    if ok then return q end
    return nil
end

local function B() local b = rawget(_G, "OVBuild"); if type(b) == "table" then return b end end

charExec("CREATE TABLE IF NOT EXISTS mod_buildrealm_access ("
    .. "account INT UNSIGNED NOT NULL PRIMARY KEY, username VARCHAR(20) NOT NULL, "
    .. "by_name VARCHAR(12) NOT NULL DEFAULT '', at INT UNSIGNED NOT NULL DEFAULT 0)")

local function isGM(player) return (tonumber(try(player, "GetGMRank")) or 0) > 0 end

-- Why an account may build here: "gm", "main" (a builder on the main realm), "allowed" (here), or nil.
local function reason(player)
    if isGM(player) then return "gm" end
    local acct = tonumber(try(player, "GetAccountId"))
    if not acct then return nil end
    if charQuery(string.format("SELECT 1 FROM mod_buildrealm_access WHERE account = %d", acct)) then return "allowed" end
    local q = charQuery(string.format("SELECT 1 FROM `%s`.mod_builder_roles r JOIN `%s`.characters c ON c.guid = r.guid "
        .. "WHERE c.account = %d LIMIT 1", MAIN_CHARS, MAIN_CHARS, acct))
    if q then return "main" end
    return nil
end

local function isBuilderHere(name)
    local b = B()
    if not b or not b.Builders then return false end
    local l = name:lower()
    for _, n in ipairs(b.Builders()) do if n:lower() == l then return true end end
    return false
end

local function grant(player)
    local b, name = B(), player:GetName()
    if b and b.SetBuilder and not isBuilderHere(name) then b.SetBuilder(nil, name, true) end
end

local function revoke(player)
    local b, name = B(), player:GetName()
    if b and b.SetBuilder and isBuilderHere(name) then b.SetBuilder(nil, name, false) end
end

local function sendAway(player)
    say(player, NAME .. " is for approved builders. Ask the realm's team to add you, then come back.")
    local function go(_, _, _, pl)
        pl = pl or player
        if pl then try(pl, "KickPlayer") end
    end
    if not try(player, "RegisterEvent", go, KICK_MS, 1) then go() end
end

local function onLogin(event, player)
    if not player then return end
    local why = reason(player)
    if why then
        if why ~= "gm" then grant(player) end
        say(player, "Welcome to " .. NAME .. ". Press F7 to build (or type /builder). Save your work with .den save <name>.")
        return
    end
    revoke(player)
    if VISITORS then
        say(player, "You're visiting " .. NAME .. ": look around as much as you like. Building is for approved builders.")
    else
        sendAway(player)
    end
end

-- ------------------------------------------------------------------------------------------------ commands
local function cleanAccount(s)
    s = tostring(s or "")
    if s:match("^[A-Za-z0-9]+$") and #s <= 20 then return s:upper() end
    return nil
end

local function accountByName(name)
    local q = authQuery(string.format("SELECT id, username FROM account WHERE username = '%s'", name))
    if not q then return nil end
    return q:GetUInt32(0), q:GetString(1)
end

local function onlineOf(acct)
    local out = {}
    if not GetPlayersInWorld then return out end
    for _, pl in ipairs(GetPlayersInWorld() or {}) do
        if tonumber(try(pl, "GetAccountId")) == acct then out[#out + 1] = pl end
    end
    return out
end

local function cleanName(s) return (tostring(s or ""):gsub("[^%a]", ""):sub(1, 12)) end

-- Publish (GM): the whole room as a saved build on the main realm, owned by nobody there, under a 6-letter code.
-- The main realm's ov_build.lua reads a code it hasn't seen from its database (getByCode), so nothing restarts or
-- reloads: on the main realm the team stands in a build area and types ".build load <code>"; it pastes at their feet
-- and facing with every check a paste has (area, budgets, the realm's own list of allowed props). Positions are from
-- the room's middle and floor; sizes and tilts don't carry (the main realm's saved-build format has neither).
local CODE_ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"   -- ov_build.lua's K.CODE_ALPHABET
local function newCode()
    for _ = 1, 20 do
        local t = {}
        for i = 1, 6 do local k = math.random(#CODE_ALPHABET); t[i] = CODE_ALPHABET:sub(k, k) end
        local code = table.concat(t)
        if not charQuery(string.format("SELECT 1 FROM `%s`.mod_builder_builds WHERE code = '%s'", MAIN_CHARS, code)) then
            return code
        end
    end
    return nil
end

local function publishText(lay)
    local out = {}
    for _, p in ipairs(lay.props or {}) do
        local e, x, y, z = tonumber(p.entry), tonumber(p.x), tonumber(p.y), tonumber(p.z)
        if e and x and y and z then
            out[#out + 1] = string.format("%d,%.2f,%.2f,%.2f,%.3f", e, x, y, z, math.rad(tonumber(p.yaw) or 0) % (2 * math.pi))
        end
    end
    return table.concat(out, ";"), #out
end

local function publish(player, arg)
    local name = tostring(arg or ""):match("^[%w_%-]+$") and arg:sub(1, 24) or nil
    if not name then return say(player, "Usage: |cffffff00.buildrealm publish <name>|r (letters, digits, - and _)") end
    local b = B()
    local lay = b and b.DenLayout and b.DenLayout()
    if not lay then return say(player, "There's no room to publish here.") end
    local text, n = publishText(lay)
    if n == 0 then return say(player, "The room is empty: nothing to publish.") end
    local code
    local q = charQuery(string.format("SELECT code FROM `%s`.mod_builder_builds WHERE owner = 0 AND name = '%s'", MAIN_CHARS, name))
    if q then code = q:GetString(0) end
    if not code or not code:match("^[A-HJ-NP-Z2-9]+$") or #code ~= 6 then code = newCode() end
    if not code then return say(player, "Couldn't make a code just now; try again.") end
    charExec(string.format("INSERT INTO `%s`.mod_builder_builds (owner, owner_name, name, code, props, created_at) "
        .. "VALUES (0, 'Builders', '%s', '%s', '%s', %d) ON DUPLICATE KEY UPDATE props = VALUES(props), "
        .. "code = VALUES(code), created_at = VALUES(created_at)", MAIN_CHARS, name, code, text, os.time()))
    say(player, string.format("Published '%s' (%d props) to the main realm. There, stand in a build area and type "
        .. "|cffffff00.build load %s|r.", name, n, code))
end

local function run(player, rest)
    local words = {}
    for w in (rest or ""):gmatch("%S+") do words[#words + 1] = w end
    local sub = (words[1] or ""):lower()
    if sub == "" or sub == "help" then
        local why = reason(player)
        local mine = why == "gm" and "you run this realm" or why == "main" and "you're a builder on the main realm"
            or why == "allowed" and "the team added you here" or "you're visiting"
        say(player, NAME .. ": approved builders build here; " .. mine .. ".")
        if isGM(player) then
            say(player, "|cffffff00.buildrealm allow <account>|r  |cffffff00deny <account>|r  |cffffff00list|r  |cffffff00publish <name>|r. "
                .. "Builders on the main realm (.build grant there) may always come.")
        end
        return
    end
    if not isGM(player) then return say(player, "Only the realm's team can do that.") end
    if sub == "allow" or sub == "deny" then
        local name = cleanAccount(words[2])
        if not name then return say(player, "Usage: |cffffff00.buildrealm " .. sub .. " <account name>|r") end
        local id, uname = accountByName(name)
        if not id then return say(player, "No account called " .. name .. ".") end
        if sub == "allow" then
            charExec(string.format("REPLACE INTO mod_buildrealm_access (account, username, by_name, at) VALUES (%d, '%s', '%s', %d)",
                id, uname, cleanName(player:GetName()), os.time()))
            for _, pl in ipairs(onlineOf(id)) do grant(pl) end
            return say(player, uname .. " may build here now.")
        end
        charExec(string.format("DELETE FROM mod_buildrealm_access WHERE account = %d", id))
        for _, pl in ipairs(onlineOf(id)) do
            if not reason(pl) then
                revoke(pl)
                if not VISITORS then sendAway(pl) end
            end
        end
        return say(player, uname .. " isn't allowed here any more (a builder on the main realm still is: revoke it there).")
    end
    if sub == "publish" then return publish(player, words[2]) end
    if sub == "list" then
        local q = charQuery("SELECT username FROM mod_buildrealm_access ORDER BY username")
        local names = {}
        if q then repeat names[#names + 1] = q:GetString(0) until not q:NextRow() end
        return say(player, (#names > 0 and ("Allowed here: " .. table.concat(names, ", ")) or "Nobody added here yet")
            .. ". Builders on the main realm come too.")
    end
    say(player, "Try |cffffff00.buildrealm|r.")
end

local function onCommand(event, player, command)
    if not player then return end
    local text = (command or ""):gsub("^[%.!]", "")
    local verb, rest = text:match("^(%S+)%s*(.*)$")
    if (verb or ""):lower() ~= "buildrealm" then return end
    run(player, rest)
    return false
end

RegisterPlayerEvent(3, onLogin)     -- PLAYER_EVENT_ON_LOGIN
RegisterPlayerEvent(42, onCommand)  -- PLAYER_EVENT_ON_COMMAND

-- for stream/tests/test_ov_buildrealm.lua (read only)
OV_BUILDREALM_TEST = { reason = reason, run = run, onLogin = onLogin, NAME = NAME, MAIN_CHARS = MAIN_CHARS }
