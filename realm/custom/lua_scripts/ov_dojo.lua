-- ov_dojo.lua: the Dojo, a Den's training grounds (mod-ale, 3.3.5a). Only with OV_DEN=1 (realm/profiles/den.env),
-- next to ov_den.lua (the room, roles, the GM controls this uses for flying and speed). docs/DEN.md, "The Dojo".
-- Owner 3 Nov: "Den - Training Room built out: able to duel in Den, customize character, fly around ... the Dojo in
-- the Den ... initial training dummy and npc area, spawn bosses". Den Lite (labs/den-lite dojo.rs) has the parts
-- that need no combat engine (the ring, display-only dummies, fly, speed, Jaina building); this is the rest.
--
--   .dojo  (or .den dojo)          what this is
--   .dojo ring                     (owner, builders) a training ring of fences, torches, banners, racks and benches in
--                                  front of you (the same layout as Den Lite's; spawned, not saved; .dojo clear takes it)
--   .dojo dummies [1-5] [kind]     (owner, builders) the game's own training dummies in front of you (in the ring when
--                                  there is one): hit them and the Den bar's pill shows your damage per second
--                                  (kind: grandmaster 80, master 70, expert 60, raider: a boss-level dummy)
--   .dojo dps                      your last numbers on the dummies
--   .dojo boss [name] [level] [health %]   (owner) one boss from a short list, at a level (1-83) and a share of its
--                                  health (1-200 %); it goes after 10 minutes, on .dojo clear, or when it dies.
--                                  Killed: your time against your best (per character, kept)
--   .dojo bosses | .dojo best      the list / your best times
--   .dojo duel on|off              (owner) the duel ring: a duel inside the Den is a spar: timed, both healed at the end
--   .dojo fly [on|off]             (owner only, inside the Den) fly without GM mode (ov_den.lua's GM controls:
--                                  off again when you leave the Den or log out)
--   .dojo speed 1|2|4              (owner, builders) run faster here (ov_den.lua's GM controls)
--   .dojo look                     a new look: the game's own customize screen at your next visit to the character list
--   .dojo build <stamp>            a stamp at once where you stand (ov_den_home.lua's .den stamp <name> now)
--   .dojo clear                    the dummies, the boss and the ring go
--
-- The Den bar's Dojo panel (WrathUnleashedUI/Dojo.lua) talks over the OVMOD channel, mod "wuden":
--   client -> server   cmd   dojo <the rest>        (ov_den.lua's cmd: OVDen.extra.dojo)
--   server -> client   dojo  server=full;fly=0|1;speed=<n>;dummies=<n>;ring=0|1;boss=<name|->;duel=0|1;best=<secs|->
--                      dps   dps=<n>;total=<n>;secs=<s>;dummy=<name>      (at most once a second while you hit)
-- Table (characters database, made here before anything reads it): mod_dojo_best (guid, boss, ms, at).
-- mod-ale used: RegisterPlayerEvent 4 ON_LOGOUT, 10 ON_DUEL_START, 11 ON_DUEL_END, 42 ON_COMMAND;
-- RegisterCreatureEvent 1 ON_ENTER_COMBAT, 4 ON_DIED, 9 ON_DAMAGE_TAKEN; CreateLuaEvent; PerformIngameSpawn (1
-- creature, 2 gameobject; not saved); Map:GetWorldObject; Creature:SetLevel / SetMaxHealth / GetMaxHealth / SetHealth /
-- DespawnOrUnsummon; GameObject:RemoveFromWorld; Player:SetAtLoginFlag / SaveToDB / SetHealth / GetMaxHealth;
-- Object:ToPlayer, Unit:GetOwner; GetCurrTime; CharDBQuery / CharDBExecute.

if os.getenv("OV_DEN") ~= "1" then return end

local TAG = "|cffffc04d[Dojo]|r "
local MOD = "wuden"
local BOSS_MS = 10 * 60 * 1000     -- a boss goes after 10 minutes
local DPS_IDLE_MS = 6000           -- no hit for this long: the next hit starts a new count
local DPS_EVERY_MS = 1000          -- the pill's numbers at most once a second
local MAX_DUMMIES = 5
local AT_LOGIN_CUSTOMIZE = 0x08    -- the game's customize screen at the next character list (ov_services.lua)
local NA = "That isn't available on this realm."

local OVMod
do
    local ok, m = pcall(function() return require("ov_modapi") end)
    if ok and type(m) == "table" then OVMod = m end
    if not OVMod and type(rawget(_G, "OVMod")) == "table" then OVMod = rawget(_G, "OVMod") end
end

local D = {
    -- the game's training dummies (creature_template); kind -> entry. TODO(owner): which ones to offer.
    DUMMIES = {
        { key = "grandmaster", entry = 31144, name = "Grandmaster's Training Dummy" },
        { key = "master", entry = 32667, name = "Master's Training Dummy" },
        { key = "expert", entry = 32666, name = "Expert's Training Dummy" },
        { key = "raider", entry = 31146, name = "Raider's Training Dummy" },
    },
    -- bosses whose scripts don't need their own instance (world bosses and simple ones). TODO(owner): the list.
    BOSSES = {
        { key = "hogger", entry = 448, name = "Hogger", level = 11 },
        { key = "vancleef", entry = 639, name = "Edwin VanCleef", level = 21 },
        { key = "azuregos", entry = 6109, name = "Azuregos", level = 63 },
        { key = "kazzak", entry = 18728, name = "Doom Lord Kazzak", level = 73 },
        { key = "doomwalker", entry = 17711, name = "Doomwalker", level = 73 },
    },
    -- the ring: x ahead, y to the left of its middle, as Den Lite's DOJO_STAMP (labs/den-lite/src/dojo.rs; the test
    -- checks they match): { entry, x, y, yaw degrees }
    RING = {
        { 19434, 6.0, 0.0, 90 }, { 19434, 5.196, 3.0, 120 }, { 19434, 3.0, 5.196, 150 }, { 19434, 0.0, 6.0, 0 },
        { 19434, -3.0, 5.196, 30 }, { 19434, -5.196, 3.0, 60 }, { 19434, -5.196, -3.0, 120 }, { 19434, -3.0, -5.196, 150 },
        { 19434, 0.0, -6.0, 0 }, { 19434, 3.0, -5.196, 30 }, { 19434, 5.196, -3.0, 60 },
        { 180043, 4.95, 4.95, 0 }, { 180043, -4.95, 4.95, 0 }, { 180043, -4.95, -4.95, 0 }, { 180043, 4.95, -4.95, 0 },
        { 180773, -6.9, 1.6, 180 }, { 180774, -6.9, -1.6, 180 },
        { 19635, 2.5, 3.6, 270 }, { 19635, 2.5, -3.6, 90 },
        { 180042, 7.6, 0.0, 180 },
        { 24538, -8.0, 3.5, 0 }, { 24538, -8.0, -3.5, 0 },
    },
    RING_REACH = 10.23,                                   -- the ring's middle this far ahead of you (Den Lite's too)
    DUMMY_SPOTS = { { 3.0, 0.0 }, { 3.0, 1.8 }, { 3.0, -1.8 }, { 1.2, 2.6 }, { 1.2, -2.6 } },
    dummies = {},      -- guidLow -> { guid, map, entry, name }
    ringObjs = {},     -- { guid, map }
    ring = nil,        -- { map, x, y, z, o }
    boss = nil,        -- { guid, low, map, key, name, by (guidLow), byName, at (ms), started (ms) }
    dps = {},          -- player guidLow -> { first, last, total, sent, name }
    best = {},         -- player guidLow -> { [key] = ms }
    duel = false,
    spars = {},        -- player guidLow -> started (ms)
}

local function say(player, text) player:SendBroadcastMessage(TAG .. text) end
local function nowMs()
    if GetCurrTime then local ok, t = pcall(GetCurrTime); if ok and tonumber(t) then return tonumber(t) end end
    return os.time() * 1000
end

-- obj:method(...) in pcall: its first result, or nil (a missing method or an error)
local function try(obj, method, ...)
    if not obj then return nil end
    local ok, fn = pcall(function() return obj[method] end)
    if not ok or type(fn) ~= "function" then return nil end
    local ok2, r = pcall(fn, obj, ...)
    if ok2 then return r end
    return nil
end
local function call(obj, method, ...)
    if not obj then return false end
    local ok, fn = pcall(function() return obj[method] end)
    if not ok or type(fn) ~= "function" then return false end
    return (pcall(fn, obj, ...))
end

local function den() return rawget(_G, "OVDen") end
local function role(player)
    local d = den()
    if d and d.role then local ok, r = pcall(d.role, player); if ok then return r end end
    return (tonumber(try(player, "GetGMRank")) or 0) > 0 and "owner" or "visitor"
end
local function inside(player)
    local d = den()
    if d and d.inside then local ok, r = pcall(d.inside, player); if ok then return r end end
    return true
end
local function floorZ(player)
    local d = den()
    if d and d.floorZ then local ok, z = pcall(d.floorZ); if ok and tonumber(z) then return tonumber(z) end end
    return player:GetZ()
end

-- who may: "builder" (the owner and builders) or "owner"; always inside the Den
local function may(player, need)
    local r = role(player)
    if r == "visitor" then return nil, "The Dojo is for the Den's owner and the builders they allow." end
    if need == "owner" and r ~= "owner" then return nil, "Only the Den's owner can do that." end
    if not inside(player) then return nil, "Only inside the Den: |cffffff00.den home|r takes you back." end
    return r
end

local function exec(sql) if CharDBExecute then pcall(CharDBExecute, sql) end end
local function query(sql)
    if not CharDBQuery then return nil end
    local ok, q = pcall(CharDBQuery, sql)
    if ok then return q end
end

------------------------------------------------------------------------------------------------
-- places: ahead of a player, and (x ahead, y left) of a spot
------------------------------------------------------------------------------------------------
local function facing(player) return tonumber(try(player, "GetO")) or 0 end
local function ahead(player, yards)
    local o = facing(player)
    return player:GetX() + math.cos(o) * yards, player:GetY() + math.sin(o) * yards, o
end
local function local2(cx, cy, o, x, y)
    local c, s = math.cos(o), math.sin(o)
    return cx + x * c - y * s, cy + x * s + y * c
end
D.local2 = local2

local function worldObject(mapId, guid)
    if not (GetMapById and guid) then return nil end
    local ok, m = pcall(GetMapById, mapId)
    if not ok or not m then return nil end
    local ok2, o = pcall(function() return m:GetWorldObject(guid) end)
    if ok2 then return o end
end

local function spawn(kind, entry, player, x, y, z, o, ms)
    if not PerformIngameSpawn then return nil end
    local ok, obj = pcall(PerformIngameSpawn, kind, entry, player:GetMapId(), tonumber(try(player, "GetInstanceId")) or 0,
        x, y, z, o, false, ms or 0)
    if ok then return obj end
end

------------------------------------------------------------------------------------------------
-- the panel's line
------------------------------------------------------------------------------------------------
local function count(t) local n = 0; for _ in pairs(t) do n = n + 1 end; return n end
local function bestOf(g)
    local b, low = D.best[g], nil
    for _, ms in pairs(b or {}) do if not low or ms < low then low = ms end end
    return low
end
function D.line(player)
    local g = player:GetGUIDLow()
    local gm = den() and den().GM
    local s = gm and gm.state and gm.state[g] or {}
    local low = bestOf(g)
    return string.format("server=full;fly=%d;speed=%d;dummies=%d;ring=%d;boss=%s;duel=%d;best=%s", s.fly and 1 or 0,
        s.speed or 1, count(D.dummies), D.ring and 1 or 0, D.boss and D.boss.key or "-", D.duel and 1 or 0,
        low and string.format("%.1f", low / 1000) or "-")
end
function D.send(player)
    if OVMod then pcall(OVMod.Send, player, MOD, "dojo", D.line(player)) end
end

------------------------------------------------------------------------------------------------
-- the ring and the dummies
------------------------------------------------------------------------------------------------
function D.clearRing()
    for _, r in ipairs(D.ringObjs) do
        local o = worldObject(r.map, r.guid)
        if o then call(o, "RemoveFromWorld", false) end
    end
    D.ringObjs, D.ring = {}, nil
end

function D.placeRing(player)
    D.clearRing()
    local cx, cy, o = ahead(player, D.RING_REACH)
    local z = floorZ(player)
    local made = 0
    for _, p in ipairs(D.RING) do
        local x, y = local2(cx, cy, o, p[2], p[3])
        local go = spawn(2, p[1], player, x, y, z, (o + math.rad(p[4])) % (2 * math.pi))
        if go then
            made = made + 1
            D.ringObjs[#D.ringObjs + 1] = { guid = try(go, "GetGUID"), map = player:GetMapId() }
        end
    end
    if made == 0 then return say(player, NA) end
    D.ring = { map = player:GetMapId(), x = cx, y = cy, z = z, o = o }
    say(player, string.format("The Dojo's ring is up (%d pieces): |cffffff00.dojo dummies|r puts training dummies in it.", made))
end

function D.clearDummies()
    local n = 0
    for _, d in pairs(D.dummies) do
        local c = worldObject(d.map, d.guid)
        if c then call(c, "DespawnOrUnsummon", 0) end
        n = n + 1
    end
    D.dummies = {}
    return n
end

local function dummyKind(word)
    word = tostring(word or ""):lower()
    for _, d in ipairs(D.DUMMIES) do if d.key == word then return d end end
end

function D.placeDummies(player, n, kindWord)
    local kind = dummyKind(kindWord) or D.DUMMIES[1]
    if kindWord and kindWord ~= "" and not dummyKind(kindWord) then
        return say(player, "Dummies: grandmaster (80), master (70), expert (60), raider (a boss-level one).")
    end
    n = math.max(1, math.min(MAX_DUMMIES, tonumber(n) or 3))
    D.clearDummies()
    local spots = {}
    if D.ring and D.ring.map == player:GetMapId() then
        local r = D.ring
        for i = 1, n do
            local s = D.DUMMY_SPOTS[i]
            local x, y = local2(r.x, r.y, r.o, s[1], s[2])
            spots[i] = { x, y, r.z, (r.o + math.pi) % (2 * math.pi) }
        end
    else
        local cx, cy, o = ahead(player, 5)
        for i = 1, n do
            local x, y = local2(cx, cy, o, 0, (i - 1 - (n - 1) / 2) * 1.8)
            spots[i] = { x, y, floorZ(player), (o + math.pi) % (2 * math.pi) }
        end
    end
    local made = 0
    for _, s in ipairs(spots) do
        local c = spawn(1, kind.entry, player, s[1], s[2], s[3], s[4], 0)
        if c then
            made = made + 1
            local low = try(c, "GetGUIDLow")
            if low then D.dummies[low] = { guid = try(c, "GetGUID"), map = player:GetMapId(), entry = kind.entry, name = kind.name } end
        end
    end
    if made == 0 then return say(player, NA) end
    local name = made == 1 and kind.name or (kind.name:gsub("y$", "ies"))
    say(player, string.format("%d %s up. Hit them: the Den bar shows your damage per second.", made, name))
end

------------------------------------------------------------------------------------------------
-- damage per second on the dummies
------------------------------------------------------------------------------------------------
local function playerOf(unit)
    if not unit then return nil end
    local p = try(unit, "ToPlayer")
    if p then return p end
    local owner = try(unit, "GetOwner")
    return owner and try(owner, "ToPlayer") or nil
end

function D.dpsLine(t, now)
    local secs = math.max(1, (t.last - t.first) / 1000)
    if now and now - t.last < DPS_IDLE_MS then secs = math.max(secs, (now - t.first) / 1000) end
    return string.format("dps=%d;total=%d;secs=%.1f;dummy=%s", math.floor(t.total / secs + 0.5), t.total, secs, t.name)
end

function D.onDamage(event, creature, attacker, damage)
    local d = creature and D.dummies[try(creature, "GetGUIDLow") or -1]
    if not d then return end
    local player = playerOf(attacker)
    if not player then return end
    local g, now = player:GetGUIDLow(), nowMs()
    local t = D.dps[g]
    if not t or now - t.last > DPS_IDLE_MS then
        t = { first = now, last = now, total = 0, sent = 0, name = d.name, player = player }
        D.dps[g] = t
    end
    t.total, t.last, t.player = t.total + (tonumber(damage) or 0), now, player
    if now - t.sent >= DPS_EVERY_MS then
        t.sent = now
        if OVMod then pcall(OVMod.Send, player, MOD, "dps", D.dpsLine(t, now)) end
    end
end

------------------------------------------------------------------------------------------------
-- bosses and best times
------------------------------------------------------------------------------------------------
exec("CREATE TABLE IF NOT EXISTS mod_dojo_best (guid INT UNSIGNED NOT NULL, boss VARCHAR(16) NOT NULL, ms INT UNSIGNED NOT NULL, "
    .. "at INT UNSIGNED NOT NULL, PRIMARY KEY (guid, boss))")

local function bestFor(g)
    if D.best[g] then return D.best[g] end
    local b = {}
    local q = query(string.format("SELECT boss, ms FROM mod_dojo_best WHERE guid = %d", g))
    if q then repeat b[q:GetString(0)] = q:GetUInt32(1) until not q:NextRow() end
    D.best[g] = b
    return b
end

local function bossOf(word)
    word = tostring(word or ""):lower()
    for _, b in ipairs(D.BOSSES) do if b.key == word then return b end end
end

function D.listBosses(player)
    local parts = {}
    for _, b in ipairs(D.BOSSES) do parts[#parts + 1] = string.format("|cffffff00%s|r (%d)", b.key, b.level) end
    say(player, "Bosses: " .. table.concat(parts, ", ") .. ". |cffffff00.dojo boss kazzak 80 50|r: at level 80, half its health.")
end

function D.clearBoss()
    local b = D.boss
    if not b then return false end
    local c = worldObject(b.map, b.guid)
    if c then call(c, "DespawnOrUnsummon", 0) end
    D.boss = nil
    return true
end

function D.spawnBoss(player, word, level, hp)
    local b = bossOf(word)
    if not b then return D.listBosses(player) end
    level = tonumber(level) or b.level
    hp = tonumber(hp) or 100
    if level ~= math.floor(level) or level < 1 or level > 83 then return say(player, "A level from 1 to 83.") end
    if hp < 1 or hp > 200 then return say(player, "Its health: 1 to 200 (per cent of its own).") end
    D.clearBoss()
    local x, y, o = ahead(player, 12)
    local c = spawn(1, b.entry, player, x, y, floorZ(player), (o + math.pi) % (2 * math.pi), BOSS_MS)
    if not c then return say(player, NA) end
    call(c, "SetLevel", level)
    local max = tonumber(try(c, "GetMaxHealth"))
    if max and hp ~= 100 then
        max = math.max(1, math.floor(max * hp / 100))
        call(c, "SetMaxHealth", max)
    end
    if max then call(c, "SetHealth", max) end
    D.boss = { guid = try(c, "GetGUID"), low = try(c, "GetGUIDLow"), map = player:GetMapId(), key = b.key, name = b.name,
        by = player:GetGUIDLow(), byName = player:GetName(), byPlayer = player, at = nowMs() }
    local best = bestFor(player:GetGUIDLow())[b.key]
    say(player, string.format("%s is here (level %d%s). The clock starts when the fight does%s. It leaves in 10 minutes or with |cffffff00.dojo clear|r.",
        b.name, level, hp ~= 100 and string.format(", %d%% health", hp) or "",
        best and string.format("; your best: %.1f s", best / 1000) or ""))
end

local function ours(creature)
    local b = D.boss
    return b and creature and try(creature, "GetGUIDLow") == b.low
end

function D.onBossCombat(event, creature)
    if ours(creature) and not D.boss.started then D.boss.started = nowMs() end
end

function D.onBossDied(event, creature, killer)
    if not ours(creature) then return end
    local b = D.boss
    D.boss = nil
    local ms = nowMs() - (b.started or b.at)
    local who = playerOf(killer) or b.byPlayer
    if not who then return end
    local g = who:GetGUIDLow()
    local best = bestFor(g)
    local old = best[b.key]
    local line = string.format("%s down in %.1f s.", b.name, ms / 1000)
    if not old or ms < old then
        best[b.key] = ms
        exec(string.format("REPLACE INTO mod_dojo_best (guid, boss, ms, at) VALUES (%d, '%s', %d, %d)", g, b.key, ms, os.time()))
        line = line .. (old and string.format(" A new best (was %.1f s)!", old / 1000) or " Your first time: that's your best.")
    else
        line = line .. string.format(" Your best is %.1f s.", old / 1000)
    end
    say(who, line)
    D.send(who)
end

function D.showBest(player)
    local best = bestFor(player:GetGUIDLow())
    local parts = {}
    for _, b in ipairs(D.BOSSES) do
        if best[b.key] then parts[#parts + 1] = string.format("%s %.1f s", b.name, best[b.key] / 1000) end
    end
    say(player, #parts == 0 and "No best times yet: |cffffff00.dojo boss|r." or ("Your best: " .. table.concat(parts, ", ") .. "."))
end

------------------------------------------------------------------------------------------------
-- spars: the duel ring
------------------------------------------------------------------------------------------------
function D.onDuelStart(event, a, b)
    if not (D.duel and a and b and inside(a) and inside(b)) then return end
    local now = nowMs()
    D.spars[a:GetGUIDLow()], D.spars[b:GetGUIDLow()] = now, now
    say(a, "A spar in the Dojo's ring: fight!")
    say(b, "A spar in the Dojo's ring: fight!")
end

function D.onDuelEnd(event, winner, loser, how)
    if not (winner and loser) then return end
    local started = D.spars[winner:GetGUIDLow()] or D.spars[loser:GetGUIDLow()]
    D.spars[winner:GetGUIDLow()], D.spars[loser:GetGUIDLow()] = nil, nil
    if not started then return end
    for _, p in ipairs({ winner, loser }) do
        local max = tonumber(try(p, "GetMaxHealth"))
        if max then call(p, "SetHealth", max) end
    end
    local line = string.format("%s wins the spar in %.1f s. Both back to full health.", winner:GetName(), (nowMs() - started) / 1000)
    say(winner, line)
    say(loser, line)
end

------------------------------------------------------------------------------------------------
-- the commands
------------------------------------------------------------------------------------------------
local HELP = {
    "The Dojo: training grounds in your Den. |cffffff00.dojo ring|r, |cffffff00.dojo dummies [1-5]|r (hit them: your damage per second in the Den bar), |cffffff00.dojo clear|r.",
    "|cffffff00.dojo boss|r (the list; owner): a boss to beat against your best time. |cffffff00.dojo duel on|r (owner): duels here are timed spars.",
    "|cffffff00.dojo fly|r (owner), |cffffff00.dojo speed 1|2|4|r, |cffffff00.dojo look|r (a new look at the character list), |cffffff00.dojo build garden|r.",
}

local function onOff(w) w = tostring(w or ""):lower(); if w == "on" then return true elseif w == "off" then return false end end

local function gm(player, words)
    local d = den()
    if not (d and d.GM and d.GM.command) then return say(player, NA) end
    return d.GM.command(player, words)
end

function D.run(player, args)
    local sub = (args[1] or ""):lower()
    if sub == "" or sub == "help" then
        for _, l in ipairs(HELP) do say(player, l) end
        return
    end
    if sub == "dps" then
        local t = D.dps[player:GetGUIDLow()]
        return say(player, t and ("On the dummies: " .. D.dpsLine(t):gsub(";dummy=.*$", ""):gsub("=", " "):gsub(";", ", "))
            or "No hits yet: |cffffff00.dojo dummies|r, then hit one.")
    elseif sub == "bosses" then return D.listBosses(player)
    elseif sub == "best" then return D.showBest(player)
    elseif sub == "status" then return
    elseif sub == "look" or sub == "barber" then
        if not call(player, "SetAtLoginFlag", AT_LOGIN_CUSTOMIZE) then return say(player, NA) end
        call(player, "SaveToDB")
        return say(player, "A new look is ready: log out to the character list and the game's customize screen opens (hair, face, skin; your name stays).")
    end
    local need = (sub == "boss" or sub == "duel") and "owner" or "builder"
    local r, why = may(player, need)
    if not r then return say(player, why) end
    local rated = den() and den().GM and den().GM.rated
    if rated and not rated(player) then return say(player, "One moment: that's a lot at once.") end
    if sub == "ring" then return D.placeRing(player)
    elseif sub == "dummies" or sub == "dummy" then return D.placeDummies(player, args[2], args[3])
    elseif sub == "boss" then return D.spawnBoss(player, args[2], args[3], args[4])
    elseif sub == "clear" then
        local n = D.clearDummies()
        local boss = D.clearBoss()
        local ring = D.ring ~= nil
        D.clearRing()
        return say(player, string.format("Cleared: %d dumm%s%s%s.", n, n == 1 and "y" or "ies", boss and ", the boss" or "", ring and ", the ring" or ""))
    elseif sub == "duel" or sub == "duels" then
        local on = onOff(args[2])
        if on == nil then return say(player, "Usage: |cffffff00.dojo duel on|r or |cffffff00off|r (now " .. (D.duel and "on" or "off") .. ").") end
        D.duel = on
        return say(player, on and "The duel ring is on: a duel inside the Den is a spar, timed, both healed at the end."
            or "The duel ring is off.")
    elseif sub == "fly" then
        if r ~= "owner" then return say(player, "Only the Den's owner flies here.") end
        local w = args[2]
        if not w or w == "" then
            local g = den() and den().GM
            local gs = g and g.state and g.state[player:GetGUIDLow()]
            w = (gs and gs.fly) and "off" or "on"
        end
        return gm(player, { "gm", "fly", w })
    elseif sub == "speed" then return gm(player, { "gm", "speed", args[2] })
    elseif sub == "build" or sub == "stamp" then
        local stamp = den() and den().extra and den().extra.stamp
        if not stamp then return say(player, NA) end
        local name = tostring(args[2] or ""):lower():gsub("[^%w_%-]", "")
        if name == "" then return say(player, "Build what? |cffffff00.dojo build garden|r (|cffffff00.den stamps|r lists them).") end
        return stamp(player, { "stamp", name, "now" }, "stamp " .. name .. " now")
    end
    for _, l in ipairs(HELP) do say(player, l) end
end

local function words(text)
    local w = {}
    for x in tostring(text or ""):gmatch("%S+") do if #w < 6 then w[#w + 1] = x:sub(1, 24) end end
    return w
end

-- .dojo ... (and .den dojo ..., through ov_den.lua: OVDen.extra.dojo, also the Den bar's cmd)
local function onCommand(event, player, command)
    if not player then return end
    local text = (command or ""):gsub("^[%.!]", "")
    local verb, rest = text:match("^(%S+)%s*(.*)$")
    if (verb or ""):lower() ~= "dojo" then return end
    D.run(player, words(rest))
    D.send(player)
    return false
end

local function install()
    OVDen = rawget(_G, "OVDen") or {}
    OVDen.extra = OVDen.extra or {}
    OVDen.extra.dojo = function(player, w)
        local args = {}
        for i = 2, #w do args[#args + 1] = w[i] end
        D.run(player, args)
        D.send(player)
    end
    -- one line in .den's help, after the others (ov_den_home.lua's)
    local prev = OVDen.help
    if not (prev and OVDen.dojoHelp) then
        OVDen.help = function(player, r)
            if prev then pcall(prev, player, r) end
            if r ~= "visitor" then say(player, "  The Dojo (training grounds): |cffffff00.dojo|r") end
        end
        OVDen.dojoHelp = true
    end
    OVDen.Dojo = D
end
install()

-- every second: the pill's last numbers when a count ends, the boss's 10 minutes
function D.tick()
    local now = nowMs()
    for g, t in pairs(D.dps) do
        if now - t.last > DPS_IDLE_MS and not t.done then
            t.done = true
            if t.player and OVMod then pcall(OVMod.Send, t.player, MOD, "dps", D.dpsLine(t) .. ";done=1") end
        end
    end
    if D.boss and now - D.boss.at > BOSS_MS + 5000 then D.clearBoss() end
end

local function onLogout(event, player)
    if not player then return end
    local g = player:GetGUIDLow()
    D.dps[g], D.spars[g], D.best[g] = nil, nil, nil
    if D.boss and D.boss.by == g then D.clearBoss() end
end

RegisterPlayerEvent(4, onLogout)          -- PLAYER_EVENT_ON_LOGOUT
RegisterPlayerEvent(10, D.onDuelStart)    -- PLAYER_EVENT_ON_DUEL_START
RegisterPlayerEvent(11, D.onDuelEnd)      -- PLAYER_EVENT_ON_DUEL_END
RegisterPlayerEvent(42, onCommand)        -- PLAYER_EVENT_ON_COMMAND
if RegisterCreatureEvent then
    for _, d in ipairs(D.DUMMIES) do RegisterCreatureEvent(d.entry, 9, D.onDamage) end     -- CREATURE_EVENT_ON_DAMAGE_TAKEN
    for _, b in ipairs(D.BOSSES) do
        RegisterCreatureEvent(b.entry, 1, D.onBossCombat)                                  -- CREATURE_EVENT_ON_ENTER_COMBAT
        RegisterCreatureEvent(b.entry, 4, D.onBossDied)                                    -- CREATURE_EVENT_ON_DIED
    end
end
if CreateLuaEvent then CreateLuaEvent(function() pcall(D.tick) end, 1000, 0) end

if OVMod then
    OVMod.On(MOD, "dojo", function(player) D.send(player) end)
end

-- for stream/tests/test_ov_dojo.lua (read only)
OV_DOJO_TEST = D
