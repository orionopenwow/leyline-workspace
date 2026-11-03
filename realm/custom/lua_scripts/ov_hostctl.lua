-- ov_hostctl.lua: "Your realm" in the in-game Host window (WrathUnleashedUI/Host.lua), the realm half (mod-ale).
--
-- The realm's GM runs their realm from the game: status, announce, an on-screen notice, message of the day, kick, mute /
-- unmute, send to Dalaran, ban / unban the ACCOUNT (a ban covers the player's whole account, as on Leyline HQ), an allow
-- list (only listed characters may log in), and a restart with a five-minute warning. Everything the realm can't do
-- from here (bots, rates, sign-up, the Leyline list, the invite link with its secret) the window hands to the
-- launcher's Host page (leyline://host) or the website, never to this script.
--
-- Who may: a player whose account's GM level on this realm is at least OV_HOSTCTL_GM (default 3: the host kit gives
-- the host's own account level 3, hosting/kit/tools/wuhost/agent.py grant_gm). Everyone else gets only
-- "status" with gm=0 (so the window says "only this realm's GM") and every other action is refused and logged.
-- OV_HOSTCTL=0 turns the whole script off.
--
-- Transport: the Open Mod API (ov_modapi.lua, prefix OVMOD, whispered to yourself, never relayed). mod "wuhost":
--   client -> server   status                       (data empty)
--                      announce   <text>            to everyone online, as "[Realm] <text>"
--                      notify     <text>            an on-screen notice to everyone online (.notify)
--                      motd       <text>            the message of the day (.server set motd)
--                      kick       <name>
--                      mute       <name>\t<minutes>\t<reason>   1-10080 minutes (.mute)
--                      unmute     <name>
--                      rescue     <name>            send to Dalaran (.tele name <name> Dalaran)
--                      ban        <name>\t<hours>\t<reason>     the character's ACCOUNT, 1-8760 hours or 0 = permanent
--                                                   (.ban account <account>; the same words and limits as Leyline HQ)
--                      unban      <name>            the character's account (.unban account)
--                      allow      on|off | add\t<name> | del\t<name>
--                      restart    (data empty)      a restart in RESTART_DELAY seconds (never sooner), with warnings
--                      cancel     (data empty)      cancels a pending restart
--   server -> client   status     key=value lines (\n): gm, up, players, bots, restart, allow, allowed, vis,
--                                 listed, signup, botsCfg, herokey, rates, invite, motd
--                      ok         <text>            done
--                      err        <text>            refused (and why)
-- Rate limits: one action per ACTION_COOLDOWN seconds per player, an announce per ANNOUNCE_COOLDOWN seconds, a
-- restart per RESTART_COOLDOWN seconds. Text from the client is cleaned (no colour codes, no pipes, no control
-- characters, brand names out) and capped; names are letters only, at most 12.
--
-- The host kit's facts for status (hosting/kit/tools/wuhost/setup.py writes them next to the scripts, data only):
--   host       <kit>/realm-files/lua_scripts/host-realm.dat
--   container  /azerothcore/lua_scripts/host-realm.dat     (OV_HOST_FILE overrides)
--   return { realm_id = "...", visibility = "public", signup = "open", bots = 40, herokey = false, rates = "3/1/2",
--            join = "https://.../join/<id>" }        (join only for a public realm: a private invite holds a secret)
-- No file (the operator's own realm): those lines say "-" and the window points at the launcher.
--
-- The launcher's host console (launcher/src-tauri/src/host/console.rs) runs the same actions from the host's own PC,
-- out of game: it adds a row to ov_host_queue in the characters database (docker exec into the realm's own database
-- container; the kit has no SOAP), and this script runs it as the console within QUEUE_EVERY ms and writes the answer
-- back ("ok:<text>", "err:<text>" or "status:<lines>"). The same checks, limits and five-minute restart as the window;
-- a row older than QUEUE_MAX_AGE seconds is answered "err" and never run late. Only on a host kit's realm (the facts
-- file above is there when the script loads); the operator's own realm never polls.
--
-- mod-ale used: RegisterPlayerEvent 3 ON_LOGIN, 4 ON_LOGOUT; CreateLuaEvent (the console queue); Player:GetGMRank / GetName / GetGUIDLow / KickPlayer /
-- SendBroadcastMessage / RegisterEvent / GetAccountId; GetPlayerByName, GetPlayersInWorld, SendWorldMessage, RunCommand,
-- CharDBQuery / CharDBExecute, AuthDBQuery, GetRealmID.

if (os.getenv("OV_HOSTCTL") or "1") == "0" then return end
local OVMod = require("ov_modapi")

local MOD = "wuhost"
local GM_LEVEL = tonumber(os.getenv("OV_HOSTCTL_GM") or "") or 3
local FILE = os.getenv("OV_HOST_FILE") or "/azerothcore/lua_scripts/host-realm.dat"
local RESTART_DELAY = 300
local QUEUE_EVERY, QUEUE_MAX_AGE = 3000, 120
local ACTION_COOLDOWN, ANNOUNCE_COOLDOWN, RESTART_COOLDOWN = 2, 10, 600
local MAX_TEXT, MAX_ALLOW, MAX_REASON = 200, 500, 160
local BAN_HOURS, MUTE_MINUTES = 8760, 10080
local TAG = "|cff5b8def[Realm]|r "
local LOADED_AT = os.time()

local H = {}
OVHostCtl = H

local function try(obj, method, ...)
    if not obj then return nil end
    local okf, fn = pcall(function() return obj[method] end)
    if not okf or type(fn) ~= "function" then return nil end
    local ok, a = pcall(fn, obj, ...)
    if ok then return a end
end

local function gmLevel(player) return tonumber(try(player, "GetGMRank")) or 0 end
function H.IsGM(player) return player ~= nil and gmLevel(player) >= GM_LEVEL end

local function log(text) print("[ov_hostctl] " .. text) end

-- ---------------------------------------------------------------- cleaning

local function anyCase(word)
    return (word:gsub("%a", function(c) return "[" .. c:lower() .. c:upper() .. "]" end))
end
local BRANDS = {}
for _, w in ipairs({ "world of warcraft", "warcraft", "lich king", "blizzard", "wow" }) do
    BRANDS[#BRANDS + 1] = "%f[%w]" .. anyCase(w):gsub(" ", "%%s+") .. "%f[%W]"
end

function H.CleanText(s)
    if type(s) ~= "string" then return nil end
    s = s:gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("[%z\1-\31\127|]", " ")
    for _, p in ipairs(BRANDS) do s = s:gsub(p, "") end
    s = s:gsub("%s+", " "):gsub("^ ", ""):gsub(" $", "")
    if #s > MAX_TEXT then s = s:sub(1, MAX_TEXT):gsub("[\128-\191]*$", ""):gsub("[\192-\255]$", "") end
    return s ~= "" and s or nil
end

-- A reason for a mute or ban (the same rule as stream/hq.py game_reason): one line, letters, digits and . , ! ? ' ( ) : / -
-- only, at most MAX_REASON, at least 3 letters.
function H.CleanReason(s)
    s = H.CleanText(s)
    if not s then return nil end
    s = s:gsub("_", " "):gsub("[^%w%s%.,!%?'%(%):/%-\128-\255]", ""):gsub("%s+", " "):gsub("^ ", ""):gsub(" $", "")
    if #s > MAX_REASON then s = s:sub(1, MAX_REASON):gsub("[\128-\191]*$", ""):gsub("[\192-\255]$", ""):gsub(" $", "") end
    return #s >= 3 and s or nil
end

function H.CleanName(s)
    if type(s) ~= "string" or not s:match("^%a+$") or #s < 2 or #s > 12 then return nil end
    return s:sub(1, 1):upper() .. s:sub(2):lower()
end

-- ---------------------------------------------------------------- the host kit's facts (data only)

local DATA_WORDS = { ["return"] = true, ["true"] = true, ["false"] = true, realm_id = true, visibility = true,
    signup = true, bots = true, herokey = true, rates = true, join = true, listed = true }
local cache = { at = -1e9 }
function H.ReadHost()
    local now = os.time()
    if now - cache.at < 30 then return cache.doc end
    cache.at, cache.doc = now, nil
    local f = io.open(FILE, "rb")
    if not f then return nil end
    local text = f:read(4097)
    f:close()
    if not text or #text > 4096 then return nil end
    -- the writer escapes quotes and backslashes as \ddd: anything else after a backslash, or any word outside the
    -- strings that is not one of DATA_WORDS, and the file is refused (it can never run code)
    if text:gsub("\\%d%d%d", ""):find("\\", 1, true) then return nil end
    for word in text:gsub('"[^"\n]*"', '""'):gmatch("[%a_][%w_]*") do
        if not DATA_WORDS[word] then return nil end
    end
    local chunk
    if setfenv then
        chunk = loadstring(text, "=host-realm")
        if chunk then setfenv(chunk, {}) end
    else
        chunk = load(text, "=host-realm", "t", {})
    end
    if not chunk then return nil end
    local ok, d = pcall(chunk)
    if not ok or type(d) ~= "table" then return nil end
    local function one(v, set) return set[v] and v or nil end
    local doc = {
        realm_id = type(d.realm_id) == "string" and d.realm_id:match("^[a-z2-7]+$") and #d.realm_id == 16 and d.realm_id or nil,
        visibility = one(d.visibility, { public = true, private = true }),
        signup = one(d.signup, { open = true, invite = true, off = true }),
        bots = tonumber(d.bots) and math.max(0, math.min(1000, math.floor(tonumber(d.bots)))) or nil,
        herokey = d.herokey == true,
        rates = type(d.rates) == "string" and d.rates:match("^[%d%-]+/[%d%-]+/[%d%-]+$") and d.rates or nil,
        listed = d.listed,
    }
    -- the public join page only: https, no fragment (a private realm's invite carries its secret after #)
    if type(d.join) == "string" and d.join:match("^https://[%w%.%-]+/join/[a-z2-7]+$") and #d.join < 160 then
        doc.join = d.join
    end
    cache.doc = doc
    return doc
end
function H.ForgetHost() cache.at = -1e9 end

-- ---------------------------------------------------------------- state

local last, lastAnnounce = {}, {}   -- guid -> os.time()
local restart = { at = nil, by = nil, last = -1e9 }
local allow = { on = false, names = {}, n = 0, loaded = false }

local function charQuery(sql)
    if not CharDBQuery then return nil end
    local ok, q = pcall(CharDBQuery, sql)
    if ok then return q end
end
local function charExec(sql) if CharDBExecute then pcall(CharDBExecute, sql) end end
local function authQuery(sql)
    if not AuthDBQuery then return nil end
    local ok, q = pcall(AuthDBQuery, sql)
    if ok then return q end
end

-- The account behind a character (a ban covers the whole account): { id, name, gm } or nil. The name is already
-- cleaned (letters only) and the account name is checked before it reaches a command.
function H.AccountOf(name)
    local q = charQuery("SELECT account FROM characters WHERE name = '" .. name .. "'")
    local id = q and tonumber(q:GetString(0))
    if not id then return nil end
    id = math.floor(id)
    local a = authQuery("SELECT username FROM account WHERE id = " .. id)
    local user = a and a:GetString(0)
    if type(user) ~= "string" or not user:match("^[%w_%.%-]+$") or #user > 32 then return nil end
    local g = authQuery("SELECT MAX(gmlevel) FROM account_access WHERE id = " .. id)
    return { id = id, name = user:upper(), gm = tonumber(g and g:GetString(0)) or 0 }
end

local function loadAllow()
    if allow.loaded then return end
    allow.loaded = true
    charExec("CREATE TABLE IF NOT EXISTS ov_host_allow (name VARCHAR(12) NOT NULL PRIMARY KEY) ENGINE=InnoDB")
    charExec("CREATE TABLE IF NOT EXISTS ov_host_settings (k VARCHAR(32) NOT NULL PRIMARY KEY, v VARCHAR(64) NOT NULL) ENGINE=InnoDB")
    local q = charQuery("SELECT name FROM ov_host_allow")
    if q then
        repeat
            local n = H.CleanName(q:GetString(0))
            if n and not allow.names[n] then allow.names[n] = true; allow.n = allow.n + 1 end
        until not q:NextRow()
    end
    local s = charQuery("SELECT v FROM ov_host_settings WHERE k = 'allow'")
    allow.on = s ~= nil and s:GetString(0) == "on"
end

local function setAllowOn(on)
    allow.on = on
    charExec("REPLACE INTO ov_host_settings (k, v) VALUES ('allow', '" .. (on and "on" or "off") .. "')")
end

local function isBot(p)
    for _, m in ipairs({ "IsBot", "IsPlayerbot" }) do
        if try(p, m) == true then return true end
    end
    return false
end

local function counts()
    local people, bots = 0, 0
    local ok, list = pcall(GetPlayersInWorld)
    if ok and type(list) == "table" then
        for _, p in ipairs(list) do if isBot(p) then bots = bots + 1 else people = people + 1 end end
    end
    return people, bots
end

-- The world server's start: the auth database's uptime row for this realm, else when this script was loaded.
local function startedAt()
    if AuthDBQuery then
        local rid = tonumber((GetRealmID and select(2, pcall(GetRealmID))) or 1) or 1
        local ok, q = pcall(AuthDBQuery, "SELECT starttime FROM uptime WHERE realmid = " .. math.floor(rid)
            .. " ORDER BY starttime DESC LIMIT 1")
        if ok and q then
            local t = tonumber(q:GetString(0))
            if t and t > 0 then return t end
        end
    end
    return LOADED_AT
end

local motd = nil

-- The launcher's host console stands in for a player: the realm's owner, above every GM level; its answer is kept here.
local CONSOLE = { console = true }
function CONSOLE:GetGUIDLow() return 0 end
function CONSOLE:GetName() return "the host console" end
function CONSOLE:GetGMRank() return 255 end

-- ---------------------------------------------------------------- replies

local function reply(player, kind, data)
    if player == CONSOLE then CONSOLE.answer = { kind, data or "" }; return end
    OVMod.Send(player, MOD, kind, data or "")
end
local function ok(player, text) reply(player, "ok", text) end
local function err(player, text) reply(player, "err", text) end

function H.Status(player)
    if not H.IsGM(player) then return reply(player, "status", "gm=0") end
    loadAllow()
    local people, bots = counts()
    local host = H.ReadHost() or {}
    local lines = {
        "gm=1",
        "up=" .. math.max(0, os.time() - startedAt()),
        "players=" .. people,
        "bots=" .. bots,
        "restart=" .. (restart.at and math.max(0, restart.at - os.time()) or 0),
        "allow=" .. (allow.on and "on" or "off"),
        "allowed=" .. allow.n,
        "vis=" .. (host.visibility or "-"),
        "listed=" .. (host.visibility == "public" and "1" or host.visibility == "private" and "0" or "-"),
        "signup=" .. (host.signup or "-"),
        "botsCfg=" .. (host.bots and tostring(host.bots) or "-"),
        "herokey=" .. (host.realm_id and (host.herokey and "on" or "off") or "-"),
        "rates=" .. (host.rates or "-"),
        "invite=" .. (host.join or "-"),
        "motd=" .. (motd or "-"),
    }
    reply(player, "status", table.concat(lines, "\n"))
end

-- ---------------------------------------------------------------- actions (GM only)

local ACTIONS = {}

function ACTIONS.announce(player, data)
    local t = H.CleanText(data)
    if not t then return err(player, "Write something to announce.") end
    local g, now = player:GetGUIDLow(), os.time()
    if lastAnnounce[g] and now - lastAnnounce[g] < ANNOUNCE_COOLDOWN then
        return err(player, "One announcement every " .. ANNOUNCE_COOLDOWN .. " seconds.")
    end
    lastAnnounce[g] = now
    if SendWorldMessage then pcall(SendWorldMessage, TAG .. t) end
    ok(player, "Announced to everyone online.")
end

function ACTIONS.notify(player, data)
    local t = H.CleanText(data)
    if not t then return err(player, "Write the notice.") end
    local g, now = player:GetGUIDLow(), os.time()
    if lastAnnounce[g] and now - lastAnnounce[g] < ANNOUNCE_COOLDOWN then
        return err(player, "One announcement every " .. ANNOUNCE_COOLDOWN .. " seconds.")
    end
    if not RunCommand then return err(player, "This realm can't show a notice from here.") end
    lastAnnounce[g] = now
    pcall(RunCommand, "notify " .. t)
    ok(player, "Notice shown to everyone online.")
end

function ACTIONS.motd(player, data)
    local t = H.CleanText(data)
    if not t then return err(player, "Write the message of the day.") end
    if t:match("^%d") then t = "- " .. t end   -- a leading number would be read as a realm id
    if not RunCommand then return err(player, "This realm can't set the message from here.") end
    pcall(RunCommand, "server set motd " .. t)
    motd = t
    ok(player, "Message of the day set.")
end

local function target(player, data)
    local name = H.CleanName(data)
    if not name then return nil, "A character name: letters only." end
    if name:lower() == tostring(try(player, "GetName") or ""):lower() then return nil, "Not yourself." end
    return name
end

function ACTIONS.kick(player, data)
    local name, why = target(player, data)
    if not name then return err(player, why) end
    local p = GetPlayerByName and GetPlayerByName(name)
    if not p then return err(player, name .. " isn't online.") end
    if gmLevel(p) >= gmLevel(player) then return err(player, "Not a GM of your level or higher.") end
    try(p, "SendBroadcastMessage", TAG .. "You were removed from this realm by its GM.")
    try(p, "KickPlayer")
    ok(player, name .. " was kicked.")
end

-- A player someone may act on: on this realm, not yourself, not your own account, not a GM of your level or higher.
local function account(player, name)
    local acct = H.AccountOf(name)
    if not acct then return nil, "No character called " .. name .. " on this realm." end
    local mine = tonumber(try(player, "GetAccountId"))
    if mine and mine == acct.id then return nil, "Not your own account." end
    local p = GetPlayerByName and GetPlayerByName(name)
    if acct.gm >= gmLevel(player) or (p and gmLevel(p) >= gmLevel(player)) then
        return nil, "Not a GM of your level or higher."
    end
    return acct
end

local function hoursText(h) return h == 0 and "for good" or ("for " .. h .. (h == 1 and " hour" or " hours")) end

function ACTIONS.mute(player, data)
    local who, minutes, why = (data or ""):match("^(%a+)\t(%d+)\t?(.*)$")
    local name, bad = target(player, who)
    if not name then return err(player, bad) end
    minutes = tonumber(minutes)
    if not minutes or minutes < 1 or minutes > MUTE_MINUTES then
        return err(player, "Minutes: a number from 1 to " .. MUTE_MINUTES .. " (a week).")
    end
    local reason = H.CleanReason(why)
    if not reason then return err(player, "Say why (3 letters or more).") end
    local _, no = account(player, name)
    if no then return err(player, no) end
    if not RunCommand then return err(player, "This realm can't mute from here.") end
    pcall(RunCommand, string.format("mute %s %d %s", name, minutes, reason))
    ok(player, name .. " can't talk for " .. minutes .. " minutes.")
end

function ACTIONS.unmute(player, data)
    local name, why = target(player, data)
    if not name then return err(player, why) end
    if not RunCommand then return err(player, "This realm can't unmute from here.") end
    pcall(RunCommand, "unmute " .. name)
    ok(player, name .. " can talk again.")
end

function ACTIONS.rescue(player, data)
    local name, why = target(player, data)
    if not name then return err(player, why) end
    local _, no = account(player, name)
    if no then return err(player, no) end
    if not RunCommand then return err(player, "This realm can't move players from here.") end
    pcall(RunCommand, "tele name " .. name .. " Dalaran")
    ok(player, name .. " is on the way to Dalaran.")
end

-- Ban account: the character's whole account (every character on it), as on Leyline HQ. 1-8760 hours, 0 = permanent.
function ACTIONS.ban(player, data)
    local who, hours, why = (data or ""):match("^(%a+)\t(%d+)\t?(.*)$")
    local name, bad = target(player, who)
    if not name then return err(player, bad) end
    hours = tonumber(hours)
    if not hours or hours < 0 or hours > BAN_HOURS then return err(player, "Hours: 1 to 8760, or 0 for a permanent ban.") end
    local reason = H.CleanReason(why)
    if not reason then return err(player, "Say why (3 letters or more).") end
    local acct, no = account(player, name)
    if not acct then return err(player, no) end
    if not RunCommand then return err(player, "This realm can't ban from here.") end
    pcall(RunCommand, string.format("ban account %s %s %s", acct.name, hours > 0 and (hours .. "h") or "-1", reason))
    local p = GetPlayerByName and GetPlayerByName(name)
    if p then try(p, "KickPlayer") end
    log("ban account " .. acct.name .. " (" .. name .. ") " .. hoursText(hours) .. " by " .. tostring(try(player, "GetName")))
    ok(player, name .. "'s account is banned " .. hoursText(hours) .. ".")
end

function ACTIONS.unban(player, data)
    local name, why = target(player, data)
    if not name then return err(player, why) end
    local acct = H.AccountOf(name)
    if not acct then return err(player, "No character called " .. name .. " on this realm.") end
    if not RunCommand then return err(player, "This realm can't unban from here.") end
    pcall(RunCommand, "unban account " .. acct.name)
    ok(player, name .. "'s account is no longer banned.")
end

function ACTIONS.allow(player, data)
    loadAllow()
    data = data or ""
    if data == "on" or data == "off" then
        setAllowOn(data == "on")
        return ok(player, data == "on" and "Only characters on your list (and GMs) can log in now."
            or "Everyone can log in again.")
    end
    local verb, who = data:match("^(%a+)\t(%a+)$")
    local name = H.CleanName(who)
    if not name or (verb ~= "add" and verb ~= "del") then return err(player, "A character name: letters only.") end
    if verb == "add" then
        if allow.names[name] then return ok(player, name .. " is already on the list.") end
        if allow.n >= MAX_ALLOW then return err(player, "The list is full (" .. MAX_ALLOW .. ").") end
        allow.names[name], allow.n = true, allow.n + 1
        charExec("REPLACE INTO ov_host_allow (name) VALUES ('" .. name .. "')")
        return ok(player, name .. " is on the list.")
    end
    if allow.names[name] then allow.names[name], allow.n = nil, allow.n - 1 end
    charExec("DELETE FROM ov_host_allow WHERE name = '" .. name .. "'")
    ok(player, name .. " is off the list.")
end

-- A restart always gives everyone RESTART_DELAY seconds: the core counts down in chat itself, and we say why.
function ACTIONS.restart(player)
    local now = os.time()
    if restart.at and restart.at > now then
        return err(player, "A restart is already coming in " .. (restart.at - now) .. " seconds.")
    end
    if now - restart.last < RESTART_COOLDOWN then
        return err(player, "One restart every " .. math.floor(RESTART_COOLDOWN / 60) .. " minutes.")
    end
    if not RunCommand then return err(player, "Restart from the launcher's Host page.") end
    restart.at, restart.by, restart.last = now + RESTART_DELAY, try(player, "GetName"), now
    pcall(RunCommand, "server restart " .. RESTART_DELAY)
    if SendWorldMessage then
        pcall(SendWorldMessage, TAG .. "The realm restarts in 5 minutes. Finish what you're doing; it is back a "
            .. "minute or two later.")
    end
    log("restart in " .. RESTART_DELAY .. " s by " .. tostring(restart.by))
    ok(player, "Restart in 5 minutes. Everyone online was told.")
end

function ACTIONS.cancel(player)
    if not (restart.at and restart.at > os.time()) then return err(player, "No restart is coming.") end
    restart.at = nil
    if RunCommand then pcall(RunCommand, "server restart cancel") end
    if SendWorldMessage then pcall(SendWorldMessage, TAG .. "The restart is cancelled.") end
    ok(player, "Restart cancelled.")
end

-- ---------------------------------------------------------------- the GM command center's sandbox (console only)
-- The launcher's GM command center (launcher/ui/gmcenter.js, host/console.rs sandbox_data) for the host's own world:
-- "sandbox" with data "<verb>\t<arg>\t<name>" (name empty: the realm's GM who is online). Fixed verbs, each with a
-- checked argument; nothing typed is ever run as a GM command. Not on the in-game window's channel (no OVMOD handler):
-- only the console's queue, which only the realm's own PC can reach. Spawns are temporary (half an hour) and "clear"
-- takes back every one this made.
local SANDBOX_TELE = { dalaran = "Dalaran", stormwind = "Stormwind", orgrimmar = "Orgrimmar", ironforge = "Ironforge",
    undercity = "Undercity", thunderbluff = "ThunderBluff", darnassus = "Darnassus", silvermoon = "Silvermoon",
    exodar = "Exodar", shattrath = "Shattrath", gmisland = "GMIsland" }
local SANDBOX_WEATHER = { clear = { 0, 0 }, rain = { 1, 0.7 }, snow = { 2, 0.7 }, storm = { 3, 0.7 } }
local SPAWN_MS, FIND_MAX = 1800000, 15
local spawned = {}   -- { kind = "creature" | "object", guid = <GetGUID()> }, newest last, at most 200
H.SANDBOX_TELE = SANDBOX_TELE

local function worldQuery(sql)
    if not WorldDBQuery then return nil end
    local okq, q = pcall(WorldDBQuery, sql)
    if okq then return q end
end

-- The search words: letters, digits, spaces and dashes only, 2 to 30 (the console checked them too).
function H.SandboxWords(s)
    local w = tostring(s or ""):gsub("[^%w %-]", ""):gsub("%s+", " "):gsub("^ ", ""):gsub(" $", "")
    if #w < 2 or #w > 30 then return nil end
    return w
end

-- Whom a control acts on: the named character (online), else the realm's GM who is online (the host).
local function sandboxWho(player, name)
    if name and name ~= "" then
        local n = H.CleanName(name)
        if not n then return nil, "A character name: letters only." end
        local p = GetPlayerByName and GetPlayerByName(n)
        if not p then return nil, n .. " isn't online." end
        if player ~= CONSOLE and p ~= player and gmLevel(p) >= gmLevel(player) then return nil, "Not a GM of your level or higher." end
        return p
    end
    if player ~= CONSOLE then return player end
    local okl, list = pcall(GetPlayersInWorld)
    if okl and type(list) == "table" then
        for _, p in ipairs(list) do
            if gmLevel(p) >= GM_LEVEL and not isBot(p) then return p end
        end
    end
    return nil, "Log in to your world with your GM character first (or type a character's name)."
end

local function found(sql, what)
    local q = worldQuery(sql)
    local rows = {}
    if q then
        repeat
            local entry, name = tonumber(q:GetString(0)), tostring(q:GetString(1) or ""):gsub("[%c|\t]", " ")
            if entry then rows[#rows + 1] = math.floor(entry) .. "\t" .. name:sub(1, 48) end
        until #rows >= FIND_MAX or not q:NextRow()
    end
    if #rows == 0 then return nil, "No " .. what .. " by that name." end
    return table.concat(rows, "\n")
end

local function remember(kind, obj)
    local g = obj and try(obj, "GetGUID")
    if g == nil then return end
    spawned[#spawned + 1] = { kind = kind, guid = g }
    if #spawned > 200 then table.remove(spawned, 1) end
end

-- the game's clock for everyone online (SMSG_LOGIN_SETTIMESPEED, 0x042): the hour asked for, until they log in again
local function sendTime(hour)
    if not CreatePacket then return false end
    local t = os.date("*t")
    local packed = (t.year - 2000) * 16777216 + (t.month - 1) * 1048576 + (t.day - 1) * 16384 + (t.wday - 1) * 2048 + hour * 64
    local okl, list = pcall(GetPlayersInWorld)
    if not (okl and type(list) == "table") then return false end
    for _, p in ipairs(list) do
        local okp, pk = pcall(CreatePacket, 0x042, 12)
        if okp and pk then
            try(pk, "WriteULong", packed)
            try(pk, "WriteFloat", 0.01666667)
            try(pk, "WriteULong", 0)
            try(p, "SendPacket", pk)
        end
    end
    return true
end

local SANDBOX = {}
function SANDBOX.tele(player, p, arg)
    local to = SANDBOX_TELE[arg]
    if not to then return err(player, "Pick a place from the list.") end
    if not RunCommand then return err(player, "This realm can't teleport from here.") end
    pcall(RunCommand, "tele name " .. p:GetName() .. " " .. to)
    ok(player, p:GetName() .. " is on the way to " .. to .. ".")
end
function SANDBOX.weather(player, p, arg)
    local w = SANDBOX_WEATHER[arg]
    if not w then return err(player, "Clear, rain, snow or storm.") end
    local map = try(p, "GetMap")
    if not map then return err(player, "This realm can't change the weather.") end
    try(map, "SetWeather", try(p, "GetZoneId"), w[1], w[2])
    ok(player, "The weather where " .. p:GetName() .. " stands: " .. arg .. ".")
end
function SANDBOX.time(player, p, arg)
    local h = tonumber(arg)
    if not h or h < 0 or h > 23 or h % 1 ~= 0 then return err(player, "An hour from 0 to 23.") end
    if not sendTime(h) then return err(player, "This realm can't set the time.") end
    ok(player, string.format("It is %02d:00 for everyone online (until they log in again).", h))
end
function SANDBOX.speed(player, p, arg)
    local n = tonumber(arg)
    if not n or n < 1 or n > 5 or n % 1 ~= 0 then return err(player, "Speed: 1 to 5 times.") end
    for _, kind in ipairs({ 1, 2, 3, 6 }) do try(p, "SetSpeed", kind, n, true) end   -- run, run back, swim, flight
    ok(player, p:GetName() .. " moves at " .. n .. "x.")
end
function SANDBOX.fly(player, p, arg)
    if arg ~= "on" and arg ~= "off" then return err(player, "Fly: on or off.") end
    try(p, "SetCanFly", arg == "on")
    ok(player, p:GetName() .. (arg == "on" and " can fly (jump, then Space rises)." or " is back on the ground."))
end
function SANDBOX.level(player, p, arg)
    local n = tonumber(arg)
    if not n or n < 1 or n > 80 or n % 1 ~= 0 then return err(player, "A level from 1 to 80.") end
    if not RunCommand then return err(player, "This realm can't set levels from here.") end
    pcall(RunCommand, "character level " .. p:GetName() .. " " .. n)
    ok(player, p:GetName() .. " is level " .. n .. ".")
end
function SANDBOX.item(player, p, arg)
    local e = tonumber(arg)
    local q = e and worldQuery("SELECT name FROM item_template WHERE entry = " .. math.floor(e))
    if not q then return err(player, "No item with that number.") end
    if not try(p, "AddItem", math.floor(e), 1) then return err(player, p:GetName() .. "'s bags are full.") end
    ok(player, p:GetName() .. " got " .. tostring(q:GetString(0)) .. ".")
end
function SANDBOX.revive(player, p)
    if not try(p, "IsDead") then return ok(player, p:GetName() .. " is alive.") end
    try(p, "ResurrectPlayer", 100)
    ok(player, p:GetName() .. " is back on their feet.")
end
function SANDBOX.heal(player, p)
    local max = try(p, "GetMaxHealth")
    if max then try(p, "SetHealth", max) end
    ok(player, p:GetName() .. " is at full health.")
end
function SANDBOX.kill(player, p)
    local t = try(p, "GetSelection")
    if not t or try(t, "GetTypeId") ~= 3 then return err(player, "Target a creature in the game first (players can't be killed from here).") end
    if try(t, "IsDead") then return ok(player, "It is already dead.") end
    try(p, "Kill", t)
    ok(player, tostring(try(t, "GetName") or "It") .. " is dead.")
end
function SANDBOX.clear(player, p)
    local map, n = try(p, "GetMap"), 0
    for i = #spawned, 1, -1 do
        local s = spawned[i]
        local obj = map and try(map, "GetWorldObject", s.guid)
        if obj then
            if s.kind == "creature" then try(obj, "DespawnOrUnsummon", 0) else
                if try(obj, "Despawn") == nil then try(obj, "RemoveFromWorld", false) end
            end
            n = n + 1
        end
        table.remove(spawned, i)
    end
    ok(player, n == 0 and "Nothing of yours to clear here." or ("Cleared " .. n .. " thing" .. (n == 1 and "" or "s") .. " you spawned."))
end
SANDBOX["find-creature"] = function(player, p, arg)
    local w = H.SandboxWords(arg)
    if not w then return err(player, "Type 2 to 30 letters to search for.") end
    local list, no = found("SELECT entry, name FROM creature_template WHERE name LIKE '%" .. w .. "%' ORDER BY entry LIMIT " .. FIND_MAX, "creature")
    if not list then return err(player, no) end
    ok(player, list)
end
SANDBOX["find-object"] = function(player, p, arg)
    local w = H.SandboxWords(arg)
    if not w then return err(player, "Type 2 to 30 letters to search for.") end
    -- doors, plain objects, chairs, spell foci only: never chests, traps or anything that runs a script
    local list, no = found("SELECT entry, name FROM gameobject_template WHERE name LIKE '%" .. w .. "%' AND type IN (0, 5, 7, 8) "
        .. "AND ScriptName = '' AND AIName = '' ORDER BY entry LIMIT " .. FIND_MAX, "object")
    if not list then return err(player, no) end
    ok(player, list)
end
SANDBOX["find-item"] = function(player, p, arg)
    local w = H.SandboxWords(arg)
    if not w then return err(player, "Type 2 to 30 letters to search for.") end
    local list, no = found("SELECT entry, name FROM item_template WHERE name LIKE '%" .. w .. "%' ORDER BY Quality DESC, entry LIMIT " .. FIND_MAX, "item")
    if not list then return err(player, no) end
    ok(player, list)
end
SANDBOX["spawn-creature"] = function(player, p, arg)
    local e = tonumber(arg)
    local q = e and worldQuery("SELECT name FROM creature_template WHERE entry = " .. math.floor(e))
    if not q then return err(player, "No creature with that number.") end
    local x, y, z, o = try(p, "GetX"), try(p, "GetY"), try(p, "GetZ"), try(p, "GetO")
    local c = x and try(p, "SpawnCreature", math.floor(e), x + 3 * math.cos(o or 0), y + 3 * math.sin(o or 0), z, (o or 0) + math.pi, 3, SPAWN_MS)
    if not c then return err(player, "It couldn't be placed here.") end
    remember("creature", c)
    ok(player, tostring(q:GetString(0)) .. " is beside " .. p:GetName() .. " for half an hour (Clear takes it back).")
end
SANDBOX["spawn-object"] = function(player, p, arg)
    local e = tonumber(arg)
    local q = e and worldQuery("SELECT name FROM gameobject_template WHERE entry = " .. math.floor(e) .. " AND type IN (0, 5, 7, 8) AND ScriptName = '' AND AIName = ''")
    if not q then return err(player, "No object like that with that number.") end
    local x, y, z, o = try(p, "GetX"), try(p, "GetY"), try(p, "GetZ"), try(p, "GetO")
    local g = x and try(p, "SummonGameObject", math.floor(e), x + 3 * math.cos(o or 0), y + 3 * math.sin(o or 0), z, o or 0, SPAWN_MS / 1000)
    if not g then return err(player, "It couldn't be placed here.") end
    remember("object", g)
    ok(player, tostring(q:GetString(0)) .. " is beside " .. p:GetName() .. " for half an hour (Clear takes it back).")
end
H.SANDBOX_VERBS = {}
for k in pairs(SANDBOX) do H.SANDBOX_VERBS[#H.SANDBOX_VERBS + 1] = k end
table.sort(H.SANDBOX_VERBS)

function ACTIONS.sandbox(player, data)
    local verb, arg, name = (data or ""):match("^([%a%-]+)\t([^\t]*)\t?(.*)$")
    local fn = verb and SANDBOX[verb]
    if not fn then return err(player, "That is not a sandbox control.") end
    -- searches need nobody online; everything else acts on a character
    if verb:match("^find%-") then return fn(player, nil, arg) end
    local p, why = sandboxWho(player, name)
    if not p then return err(player, why) end
    log("sandbox " .. verb .. " on " .. tostring(try(p, "GetName")) .. " by " .. tostring(try(player, "GetName")))
    fn(player, p, arg)
end

-- One entry for every action: GM check first, then the rate limit, then the action.
function H.Handle(player, action, data)
    if not player then return false end
    if action == "status" then return H.Status(player) end
    local fn = ACTIONS[action]
    if not fn then return false end
    if not H.IsGM(player) then
        log(string.format("refused %s from %s (GM level %d)", action, tostring(try(player, "GetName")), gmLevel(player)))
        err(player, "Only this realm's GM can do that.")
        return false
    end
    local g, now = player:GetGUIDLow(), os.time()
    if last[g] and now - last[g] < ACTION_COOLDOWN then
        err(player, "One moment: one action every " .. ACTION_COOLDOWN .. " seconds.")
        return false
    end
    last[g] = now
    fn(player, data)
    return true
end

H.ACTION_NAMES = { "status", "announce", "notify", "motd", "kick", "mute", "unmute", "rescue", "ban", "unban", "allow",
    "restart", "cancel" }
for _, a in ipairs(H.ACTION_NAMES) do
    OVMod.On(MOD, a, function(player, data) H.Handle(player, a, data) end)
end

-- The allow list at login: a character not on it (and not a GM) is told and sent off a moment later.
local function onLogin(event, player)
    if not player then return end
    loadAllow()
    if not allow.on or gmLevel(player) > 0 or isBot(player) then return end
    local name = H.CleanName(try(player, "GetName"))
    if name and allow.names[name] then return end
    try(player, "SendBroadcastMessage", TAG .. "This realm is invite-only right now: ask its GM to add you.")
    local okReg = pcall(function()
        player:RegisterEvent(function(_, _, _, p) if p then try(p, "KickPlayer") end end, 3000, 1)
    end)
    if not okReg then try(player, "KickPlayer") end
end

local function onLogout(event, player)
    if not player then return end
    local g = player:GetGUIDLow()
    last[g], lastAnnounce[g] = nil, nil
end

RegisterPlayerEvent(3, onLogin)    -- PLAYER_EVENT_ON_LOGIN
RegisterPlayerEvent(4, onLogout)   -- PLAYER_EVENT_ON_LOGOUT

-- ---------------------------------------------------------------- the launcher's host console (ov_host_queue)

local QUEUE_TABLE = "CREATE TABLE IF NOT EXISTS ov_host_queue (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, "
    .. "action VARCHAR(16) NOT NULL, data VARBINARY(255) NOT NULL, at INT UNSIGNED NOT NULL, done TINYINT NOT NULL "
    .. "DEFAULT 0, result VARBINARY(1024) NULL) ENGINE=InnoDB"
local queueSeen, lastClean = {}, 0

local function unhex(h)
    if type(h) ~= "string" or #h % 2 ~= 0 or h:find("[^%x]") then return nil end
    return (h:gsub("%x%x", function(x) return string.char(tonumber(x, 16)) end))
end
local function hex(s) return (s:gsub(".", function(c) return string.format("%02X", c:byte()) end)) end

-- One look at the queue: runs what waits (at most 5), answers each row once.
function H.RunQueue()
    local q = charQuery("SELECT id, action, HEX(data), at FROM ov_host_queue WHERE done = 0 ORDER BY id LIMIT 5")
    local n = 0
    if q then
        repeat
            local id = tonumber(q:GetString(0))
            if id and not queueSeen[id] then
                queueSeen[id] = true
                local action, data, at = q:GetString(1), unhex(q:GetString(2) or ""), tonumber(q:GetString(3)) or 0
                CONSOLE.answer = nil
                if os.time() - at > QUEUE_MAX_AGE then
                    CONSOLE.answer = { "err", "That waited too long, so it was not done." }
                elseif not data or not (action == "status" or ACTIONS[action]) then
                    CONSOLE.answer = { "err", "The realm doesn't know that control." }
                else
                    H.Handle(CONSOLE, action, data)
                    log("console: " .. action)
                end
                local a = CONSOLE.answer or { "err", "No answer." }
                local text = (a[1] .. ":" .. a[2]):sub(1, 1000)
                charExec(string.format("UPDATE ov_host_queue SET done = 1, result = UNHEX('%s') WHERE id = %d", hex(text), id))
                n = n + 1
            end
        until not q:NextRow()
    end
    local now = os.time()
    if n == 0 and now - lastClean > 3600 then
        lastClean = now
        charExec("DELETE FROM ov_host_queue WHERE at < " .. math.floor(now - 86400))
    end
    return n
end

do
    local hostFile = io.open(FILE, "rb")
    if hostFile then
        hostFile:close()
        charExec(QUEUE_TABLE)
        if CreateLuaEvent then CreateLuaEvent(function() H.RunQueue() end, QUEUE_EVERY, 0) end
    end
end

-- for tests
H._restart, H._allow, H.RESTART_DELAY, H.GM_LEVEL = restart, allow, RESTART_DELAY, GM_LEVEL
H.CONSOLE, H.QUEUE_MAX_AGE = CONSOLE, QUEUE_MAX_AGE
H.OnLogin = onLogin
return H
