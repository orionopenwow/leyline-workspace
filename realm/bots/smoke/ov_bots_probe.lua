-- ov_bots_probe.lua: the bot smoke test's eyes inside the realm (realm/bots/bots_smoke.py, docs/BOTS-UPDATES.md).
-- Loaded ONLY by the throwaway smoke realm (realm/bots/docker-compose.smoke.yml mounts it beside ov_companions.lua);
-- it is never in the host kit or on a player's realm. It reads state, spawns one practice mob, and says what it
-- saw as one "OVSMOKE {json}" system line to the typing player (and the server log). It never moves a bot and never
-- sends a bot command: what the bots do is mod-playerbots' and ov_companions.lua's own work, which is what's tested.
--
--   .ovsmoke report            the player and their group: per member name, bot?, map, instance, alive, in combat
--   .ovsmoke compat A,B,C      which of these mod-ale methods a bot (and its PlayerbotAI) answers to
--   .ovsmoke pull [entry]      one hostile practice mob (default 38) a few yards in front, attacking the player
--   .ovsmoke fight             the practice mob: alive, health %, which bots are fighting it
--
-- GM accounts only (the smoke test's own account is made GM through the console).

local MOB_DEFAULT = 38          -- a level 10 humanoid of the starting lands (creature_template), any hostile one works
local FACTION_HOSTILE = 14      -- hostile to everyone
local TEMPSUMMON_TIMED_DESPAWN = 3
local pulled = {}               -- player guid low -> { guid = creature guid, map = map id }

local function try(obj, method, ...)
    if not obj then return nil end
    local ok, fn = pcall(function() return obj[method] end)
    if not ok or type(fn) ~= "function" then return nil end
    local ok2, a = pcall(fn, obj, ...)
    if ok2 then return a end
    return nil
end

local function has(obj, method)
    local ok, fn = pcall(function() return obj[method] end)
    return ok and type(fn) == "function"
end

-- a tiny JSON writer (strings, numbers, booleans, arrays, objects with string keys)
local function enc(v)
    local t = type(v)
    if t == "nil" then return "null" end
    if t == "boolean" then return v and "true" or "false" end
    if t == "number" then
        if v ~= v or v == math.huge or v == -math.huge then return "null" end
        if math.floor(v) == v then return string.format("%d", v) end
        return string.format("%.2f", v)
    end
    if t == "string" then
        return '"' .. v:gsub('[%c"\\]', function(c) return string.format("\\u%04x", c:byte()) end) .. '"'
    end
    if t == "table" then
        if #v > 0 or next(v) == nil then
            local out = {}
            for i = 1, #v do out[i] = enc(v[i]) end
            return "[" .. table.concat(out, ",") .. "]"
        end
        local keys, out = {}, {}
        for k in pairs(v) do keys[#keys + 1] = tostring(k) end
        table.sort(keys)
        for _, k in ipairs(keys) do out[#out + 1] = enc(k) .. ":" .. enc(v[k]) end
        return "{" .. table.concat(out, ",") .. "}"
    end
    return "null"
end

local function answer(player, what, data)
    data.t = what
    local line = "OVSMOKE " .. enc(data)
    print(line)
    player:SendBroadcastMessage(line)
end

local function isBot(m) return try(m, "IsPlayerbot") == true end

local function member(m, player)
    return {
        name = try(m, "GetName"), bot = isBot(m), level = try(m, "GetLevel"), class = try(m, "GetClass"),
        map = try(m, "GetMapId"), instance = try(m, "GetInstanceId"), alive = try(m, "IsDead") == false,
        combat = try(m, "IsInCombat") == true, x = try(m, "GetX"), y = try(m, "GetY"), z = try(m, "GetZ"), o = try(m, "GetO"),
        dist = (m ~= player) and try(m, "GetDistance", player) or 0,
        master = (function() local ai = try(m, "GetBotAI"); local ma = ai and try(ai, "GetMaster"); return ma and try(ma, "GetName") or nil end)(),
    }
end

local function members(player)
    local g = try(player, "GetGroup")
    local list = g and try(g, "GetMembers") or { player }
    return g, list
end

local function report(player)
    local g, list = members(player)
    local out = {}
    for _, m in ipairs(list) do out[#out + 1] = member(m, player) end
    answer(player, "report", { me = member(player, player), grouped = g ~= nil, raid = g and try(g, "IsRaidGroup") == true or false,
        members = out })
end

local function compat(player, csv)
    local names = {}
    for n in tostring(csv or ""):gmatch("[%a]+") do names[#names + 1] = n end
    local _, list = members(player)
    local bot
    for _, m in ipairs(list) do if isBot(m) then bot = m; break end end
    if not bot then return answer(player, "compat", { error = "no bot in the group to ask" }) end
    local ai = try(bot, "GetBotAI")
    local missing, ok = {}, {}
    for _, n in ipairs(names) do
        if has(bot, n) or (ai and has(ai, n)) then ok[#ok + 1] = n else missing[#missing + 1] = n end
    end
    answer(player, "compat", { bot = try(bot, "GetName"), ai = ai ~= nil, ok = ok, missing = missing })
end

local function pull(player, entry)
    entry = tonumber(entry or "") or MOB_DEFAULT
    local x, y, z, o = player:GetX(), player:GetY(), player:GetZ(), player:GetO()
    local c = try(player, "SpawnCreature", entry, x + 6 * math.cos(o), y + 6 * math.sin(o), z, o, TEMPSUMMON_TIMED_DESPAWN, 240000)
    if not c then return answer(player, "pull", { error = "could not spawn creature " .. entry }) end
    try(c, "SetFaction", FACTION_HOSTILE)
    try(c, "AttackStart", player)
    pulled[player:GetGUIDLow()] = { guid = c:GetGUID(), map = player:GetMapId() }
    answer(player, "pull", { entry = entry, name = try(c, "GetName"), level = try(c, "GetLevel") })
end

local function fight(player)
    local p = pulled[player:GetGUIDLow()]
    if not p then return answer(player, "fight", { error = "nothing pulled" }) end
    local map = try(player, "GetMap")
    local c = map and try(map, "GetWorldObject", p.guid)
    local _, list = members(player)
    local on = {}
    for _, m in ipairs(list) do
        if isBot(m) and try(m, "IsInCombat") == true then on[#on + 1] = try(m, "GetName") end
    end
    if not c then return answer(player, "fight", { gone = true, alive = false, bots_in_combat = on }) end
    answer(player, "fight", { alive = try(c, "IsDead") == false, hp = try(c, "GetHealthPct"), bots_in_combat = on,
        victim = (function() local v = try(c, "GetVictim"); return v and try(v, "GetName") or nil end)() })
end

local function onCommand(event, player, command)
    if not player then return end
    local words = {}
    for w in tostring(command or ""):gsub("^[%.!]", ""):gmatch("%S+") do words[#words + 1] = w end
    if (words[1] or ""):lower() ~= "ovsmoke" then return end
    if (tonumber(try(player, "GetGMRank") or 0) or 0) < 1 then return false end
    local sub = (words[2] or "report"):lower()
    local ok, err = pcall(function()
        if sub == "report" then report(player)
        elseif sub == "compat" then compat(player, words[3])
        elseif sub == "pull" then pull(player, words[3])
        elseif sub == "fight" then fight(player)
        else answer(player, "error", { error = "unknown: " .. sub }) end
    end)
    if not ok then answer(player, "error", { error = tostring(err) }) end
    return false
end

RegisterPlayerEvent(42, onCommand)   -- PLAYER_EVENT_ON_COMMAND
print("OVSMOKE {\"t\":\"loaded\"}")
