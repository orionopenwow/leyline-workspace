-- lua5.1 tests/test_ov_companions.lua ../realm/custom/lua_scripts/ov_companions.lua
local file = assert(arg[1], "ov_companions.lua")
local events, printed = {}, {}
local clock = 1000000
os.time = function() return clock end
local env = {}
os.getenv = function(k) return env[k] end
function RegisterPlayerEvent(id, fn) events[id] = fn end
-- the OVMOD channel (ov_modapi.lua, next to the script) for the Bots window
local onServerAddon
function RegisterServerEvent(id, fn) if id == 30 then onServerAddon = fn end end
package.path = (arg[1]:match("^(.*/)[^/]+$") or "./") .. "?.lua;" .. package.path
local realPrint = print
print = function(s) printed[#printed + 1] = tostring(s) end
math.randomseed(7)

-- A fake characters database: characters and mod_companions_usual.
local sqlLog = {}
local chars = {}   -- { guid, account, name, level, class, online }
local usual = {}   -- [account] = { { slot, guid, name } }
local owned = {}   -- [bot guid] = { owner, name, role, spec, kind }
local autos = {}   -- [character guid] = { mode, party }: mod_companions_auto (single player)
local function result(rows)
    if not rows or #rows == 0 then return nil end
    local r = { rows = rows, i = 1 }
    function r:GetUInt32(c) return tonumber(self.rows[self.i][c + 1]) end
    function r:GetString(c) return tostring(self.rows[self.i][c + 1] or "") end
    function r:NextRow() self.i = self.i + 1; return self.i <= #self.rows end
    return r
end
function CharDBExecute(sql)
    sqlLog[#sqlLog + 1] = sql
    local ob, oo, on, orl, osp, ok = sql:match("^REPLACE INTO mod_companions_owned %(bot, owner, name, role, spec, kind, at%) VALUES %((%d+), (%d+), '([%a\128-\255]+)', '(%a+)', '([%a ]*)', '(%a+)', %d+%)$")
    if ob then owned[tonumber(ob)] = { owner = tonumber(oo), name = on, role = orl, spec = osp, kind = ok } end
    local ag, am, ap = sql:match("^REPLACE INTO mod_companions_auto %(guid, mode, party, at%) VALUES %((%d+), '(%a+)', '([%a =:,]*)', %d+%)$")
    if ag then autos[tonumber(ag)] = { mode = am, party = ap } end
    local db = sql:match("^DELETE FROM mod_companions_owned WHERE bot = (%d+)$")
    if db then owned[tonumber(db)] = nil end
    local dow = sql:match("^DELETE FROM mod_companions_owned WHERE owner = (%d+)$")
    if dow then for k, v in pairs(owned) do if v.owner == tonumber(dow) then owned[k] = nil end end end
    local acc = sql:match("^DELETE FROM mod_companions_usual WHERE account = (%d+)$")
    if acc then usual[tonumber(acc)] = nil end
    local a, s, g, n = sql:match("^INSERT INTO mod_companions_usual %(account, slot, guid, name, saved_at%) VALUES %((%d+), (%d+), (%d+), '([%a\128-\255]+)', %d+%)$")
    if a then
        a = tonumber(a); usual[a] = usual[a] or {}
        table.insert(usual[a], { slot = tonumber(s), guid = tonumber(g), name = n })
    end
end
function CharDBQuery(sql)
    sqlLog[#sqlLog + 1] = sql
    local au = sql:match("^SELECT mode, party FROM mod_companions_auto WHERE guid = (%d+)$")
    if au then local r = autos[tonumber(au)]; return r and result({ { r.mode, r.party } }) or nil end
    local ow = sql:match("^SELECT bot, name, role, spec, kind FROM mod_companions_owned WHERE owner = (%d+)$")
    if ow then
        local rows = {}
        for b, v in pairs(owned) do if v.owner == tonumber(ow) then rows[#rows + 1] = { b, v.name, v.role, v.spec, v.kind } end end
        return result(rows)
    end
    local acc, me = sql:match("^SELECT guid, name, level, class FROM characters WHERE account = (%d+) AND online = 0 AND guid <> (%d+)")
    if acc then
        local rows = {}
        for _, c in ipairs(chars) do
            if c.account == tonumber(acc) and c.online == 0 and c.guid ~= tonumber(me) then rows[#rows + 1] = { c.guid, c.name, c.level, c.class } end
        end
        return result(rows)
    end
    local acc2, nm = sql:match("^SELECT guid, name, level, class, online FROM characters WHERE account = (%d+) AND name = '([^']*)' LIMIT 1$")
    if acc2 then
        for _, c in ipairs(chars) do
            if c.account == tonumber(acc2) and c.name:lower() == nm:lower() then return result({ { c.guid, c.name, c.level, c.class, c.online } }) end
        end
        return nil
    end
    local acc3 = sql:match("FROM mod_companions_usual u JOIN characters c ON c.guid = u.guid AND c.account = u.account WHERE u.account = (%d+) ORDER BY u.slot$")
    if acc3 then
        local rows = {}
        for _, u in ipairs(usual[tonumber(acc3)] or {}) do
            for _, c in ipairs(chars) do
                if c.guid == u.guid and c.account == tonumber(acc3) then rows[#rows + 1] = { c.guid, c.name, c.level, c.class, c.online } end
            end
        end
        return result(rows)
    end
    return nil
end

local T_SRC = assert(io.open(file)):read("*a")
dofile(file)
local T = assert(OV_COMPANIONS_TEST)

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; realPrint("PASS " .. name) else fail = fail + 1; realPrint("FAIL " .. name) end
end

check("usual-party table made at load, mod_ prefix",
    sqlLog[1] and sqlLog[1]:match("^CREATE TABLE IF NOT EXISTS mod_companions_usual") ~= nil)
check("single player's table made at load, mod_ prefix", (function()
    for _, q in ipairs(sqlLog) do if q:match("^CREATE TABLE IF NOT EXISTS mod_companions_auto") then return true end end
    return false end)())
check("ownership table made at load, mod_ prefix",
    sqlLog[2] and sqlLog[2]:match("^CREATE TABLE IF NOT EXISTS mod_companions_owned") ~= nil)
check("the window channel is there (ov_modapi.lua loaded through require)", T.window == true and onServerAddon ~= nil)

local BANNED = { "world of warcraft", "wow", "warcraft", "lich king", "blizzard", "openvanilla", "open vanilla", "openwow",
    "leyline", "wrathunleashed", "playerbot", "azerothcore", "mod-ale", "eluna" }
local allMsgs = {}

local function player(over)
    local p = { msgs = {}, cmds = {}, whispers = {}, queue = {}, teleports = {}, combat = false, dead = false }
    for k, v in pairs(over or {}) do p[k] = v end
    function p:SendBroadcastMessage(m) self.msgs[#self.msgs + 1] = m; allMsgs[#allMsgs + 1] = m end
    function p:GetGUIDLow() return self.guid end
    function p:GetGUID() return self.guid end
    function p:GetAccountId() return self.account or 42 end
    function p:GetAccountName() return self.accName or "PLAYER" end
    function p:GetName() return self.name or "Tester" end
    function p:GetLevel() return self.level or 20 end
    function p:GetClass() return self.class or 1 end
    function p:IsInCombat() return self.combat end
    function p:IsDead() return self.dead end
    function p:InBattleground() return self.bg or false end
    function p:InArena() return false end
    function p:GetMap() local s = self; return { IsDungeon = function() return (s.dungeon or s.raidmap) or false end,
        IsRaid = function() return s.raidmap or false end } end
    function p:GetMapId() return self.map or 0 end
    function p:GetInstanceId() return self.inst or 0 end
    function p:GetX() return 1 end
    function p:GetY() return 2 end
    function p:GetZ() return 3 end
    function p:GetO() return 4 end
    function p:GetDistance() return self.dist or 5 end
    function p:GetGroup() return self.group end
    function p:Teleport(...) self.teleports[#self.teleports + 1] = { ... } end
    function p:RegisterEvent(fn, delay, rep) self.queue[#self.queue + 1] = { fn = fn, delay = delay, rep = rep } end
    function p:Whisper(text, lang, to)
        self.whispers[#self.whispers + 1] = { text = text, to = to, via = "whisper" }
        if text == "leave" and to and to.group then to.group:RemoveMember(to.guid) end
    end
    function p:SendAddonMessage(prefix, msg, kind, to)
        self.addon = self.addon or {}
        self.addon[#self.addon + 1] = { prefix = prefix, msg = msg, kind = kind, to = to }
    end
    function p:GetSelection() return self.sel end
    function p:GetEquippedItemBySlot(slot)
        if not self.ilvl or slot == 3 or slot == 18 then return nil end
        local lv = self.ilvl
        return { GetItemLevel = function() return lv end }
    end
    -- mod-playerbots' PlayerbotAI through mod-ale (bots only; noai = an older mod-ale without GetBotAI)
    do
        function p:GetBotAI()
            if not self.bot or self.noai then return nil end
            local bot = self
            self.ai = self.ai or {
                HandleCommand = function(_, kind, text, from)
                    from.whispers[#from.whispers + 1] = { text = text, to = bot, via = "ai", kind = kind }
                    if text == "leave" and bot.group then bot.group:RemoveMember(bot.guid) end
                end,
                ChangeStrategy = function(_, change, state)
                    bot.strategies = bot.strategies or {}
                    bot.strategies[#bot.strategies + 1] = state .. ":" .. change
                end,
                GetMaster = function() return bot.master end,
            }
            return self.ai
        end
    end
    if not (over and over.built == false) then
        function p:IsPlayerbot() return self.bot == true end
        function p:IsAddclassBot() return self.addclass == true end
        function p:IsTank(bySpec) return bySpec == true and self.spec == "tank" end
        function p:IsHealer(bySpec) return bySpec == true and self.spec == "healer" end
        function p:RunCommand(c) self.cmds[#self.cmds + 1] = c end
    end
    return p
end

-- A group the fake mod-playerbots adds bots to.
local function group(leader, members)
    local g = { members = members or { leader }, raid = false, converted = 0 }
    function g:GetMembers() return self.members end
    function g:GetMembersCount() return #self.members end
    function g:IsLeader(guid) return guid == leader.guid end
    function g:IsRaidGroup() return self.raid end
    function g:ConvertToRaid() self.raid = true; self.converted = self.converted + 1 end
    function g:IsLFGGroup() return self.lfg or false end
    function g:IsBGGroup() return false end
    function g:RemoveMember(guid)
        for i, m in ipairs(self.members) do if m.guid == guid then table.remove(self.members, i); m.group = nil; return true end end
        return false
    end
    leader.group = g
    for _, m in ipairs(g.members) do m.group = g end
    return g
end

local nextBot = 1000
-- What mod-playerbots does for each "addclass <class>" / "add <name>" the player ran: the bot joins the group.
local CLS = { warrior = 1, paladin = 2, hunter = 3, rogue = 4, priest = 5, dk = 6, shaman = 7, mage = 8, warlock = 9, druid = 11 }
local function serve(p, opts)
    opts = opts or {}
    local made = {}
    p.served = p.served or {}
    for i, c in ipairs(p.cmds) do
        local cls = c:match("^playerbots bot addclass (%a+)$")
        local alt = c:match("^playerbots bot add (%S+)$")
        if (cls or alt) and not p.served[i] and not (opts.skip and opts.skip[cls or alt]) then
            p.served[i] = true
            local b
            if cls then
                nextBot = nextBot + 1
                b = player({ guid = nextBot, account = 900, accName = "RNDBOT" .. nextBot, bot = true, addclass = true,
                    class = CLS[cls], level = opts.level or p:GetLevel(), name = "Bot" .. string.char(64 + (nextBot % 26) + 1) .. "x",
                    map = opts.map or 0, inst = opts.inst or 0, dist = opts.dist })
            else
                local row
                for _, ch in ipairs(chars) do if ch.name == alt then row = ch end end
                b = player({ guid = row.guid, account = row.account, accName = "OWNER", bot = true, addclass = false,
                    class = row.class, level = row.level, name = row.name, spec = row.spec, map = opts.map or 0,
                    inst = opts.inst or 0, dist = opts.dist })
                row.online = 1
            end
            b.master = p
            if opts.noai then b.noai = true end
            if not p.group then group(p) end
            table.insert(p.group.members, b); b.group = p.group
            if #p.group.members > 5 then p.group.raid = true end   -- the module converts at the 6th member
            made[#made + 1] = b
        end
    end
    return made
end

local function run(p, n)
    local ticks = 0
    while #p.queue > 0 and (not n or ticks < n) do
        local e = table.remove(p.queue, 1)
        for k = 1, e.rep do ticks = ticks + 1; e.fn(ticks, e.delay, e.rep - k, p) end
    end
    return ticks
end
local function last(p) return p.msgs[#p.msgs] or "" end
local function cmd(p, c) return events[42](42, p, c) end
local function count(p, pat) local n = 0; for _, c in ipairs(p.cmds) do if c:match(pat) then n = n + 1 end end; return n end
local function whispered(p, to, pat)
    for _, w in ipairs(p.whispers) do if w.to == to and w.text:match(pat) then return true end end
    return false
end
local function n(t) local k = 0; for _ in pairs(t or {}) do k = k + 1 end; return k end

check("events registered (logout, level, command)", events[4] and events[13] and events[42])

-- composition math ------------------------------------------------------------------------------------------
local P = T.planRoles
local function eq(a, t, h, d) return a.tank == t and a.healer == h and a.dps == d end
check("plan: solo damage player, raid of 10 -> 2 tanks, 3 healers, 4 damage", eq(P({ dps = 1 }, T.SHAPE[10], 9), 2, 3, 4))
check("plan: solo tank, raid of 10 -> 1 tank, 3 healers, 5 damage", eq(P({ tank = 1 }, T.SHAPE[10], 9), 1, 3, 5))
check("plan: solo healer, party of 5 -> 1 tank, 0 healers, 3 damage", eq(P({ healer = 1 }, T.SHAPE[5], 4), 1, 0, 3))
check("plan: tanks and healers first when slots are short", eq(P({ dps = 1 }, T.SHAPE[10], 4), 2, 2, 0))
check("plan: extra slots go to damage", eq(P({ tank = 3, dps = 1 }, T.SHAPE[10], 6), 0, 3, 3))
check("plan: never negative", eq(P({ tank = 5, healer = 5, dps = 9 }, T.SHAPE[10], 0), 0, 0, 0))

-- help -----------------------------------------------------------------------------------------------------
local h = player({ guid = 1 })
check(".companions handled", cmd(h, ".companions") == false)
local help = table.concat(h.msgs, "\n")
check("help lists party, raid, roles, alts, save, buddy, dismiss and the limits",
    help:match("%.recruit party") and help:match("%.recruit raid") and help:match("%.recruit tank")
    and help:match("%.recruit alt") and help:match("%.companions save") and help:match("%.recruit usual")
    and help:match("%.buddy") and help:match("%.dismiss") and help:match("4 bots in a party") and help:match("9 bots in a raid")
    and help:match("every 30 s") and help:match("pass on loot"))
check("other commands untouched", cmd(h, "summon") == nil and cmd(h, "qol") == nil and cmd(h, "playerbots bot list") == nil)

-- recruit a tank ---------------------------------------------------------------------------------------------
local a = player({ guid = 10, level = 30 })
check(".recruit handled", cmd(a, "recruit tank") == false)
check("one addclass of a tank class", #a.cmds == 1 and (a.cmds[1]:match("addclass warrior$") or a.cmds[1]:match("addclass paladin$")
    or a.cmds[1]:match("addclass druid$") or a.cmds[1]:match("addclass dk$")))
check("no dk below 55", count(a, "addclass dk") == 0)
check("pending, says a few seconds", #T.pending[10] == 1 and last(a):match("Recruiting 1 tank bot"))
check("arrival timer queued", #a.queue == 1 and a.queue[1].delay == 2000)
local bots = serve(a)
run(a)
local tank = bots[1]
check("claimed into companions", T.companions[10] and T.companions[10][tank.guid] ~= nil and #T.pending[10] == 0)
check("tank spec whispered", whispered(a, tank, "^talents spec %a+ pve$") and (whispered(a, tank, "prot pve") or whispered(a, tank, "bear pve")))
check("maintenance and autogear whispered", whispered(a, tank, "^maintenance$") and whispered(a, tank, "^autogear$"))
check("loot list reset whispered", whispered(a, tank, "^ll normal$"))
check("follow whispered", whispered(a, tank, "^follow$"))
check("same level: no init", count(a, "init=auto") == 0)
check("joins message calls it a bot", a.msgs[#a.msgs]:match("joins as a tank bot"))

-- cooldown
cmd(a, "recruit healer")
check("cooldown", count(a, "addclass") == 1 and last(a):match("recruit again in 30 s"))
clock = clock + 31

-- three dps, five asked: party of 5 -> room for 3
cmd(a, "recruit dps 5")
check("capped at the party: 3 more (you + tank + 3 = 5)", count(a, "addclass") == 4 and last(a):match("3 of the 5 asked"))
check("dps classes vary", (function()
    local seen = {}; for i = 2, 4 do seen[a.cmds[i]] = true end; return n(seen) == 3 end)())
check("realm count includes pending", T.realmCount() == 4)
local dps = serve(a, { level = 25, map = 1 })
run(a)
check("all four claimed", n(T.companions[10]) == 4)
check("open world, other map: moved to the player", #dps[1].teleports == 1 and dps[1].teleports[1][2] == 1)
check("level mismatch: init=auto by name", count(a, "^playerbots bot init=auto Bot%a+$") == 3)
clock = clock + 31
cmd(a, "recruit healer")
check("party cap: 4 bots", count(a, "addclass") == 4 and last(a):match("the most allowed in a party"))

-- dismiss
local before = #a.cmds
cmd(a, "dismiss"); run(a)
check(".dismiss alone with several: asks which", #a.cmds == before and last(a):match("Dismiss which"))
cmd(a, "dismiss " .. tank.name:lower()); run(a)
check(".dismiss <name> removes that one", a.cmds[#a.cmds] == "playerbots bot remove " .. tank.name and T.companions[10][tank.guid] == nil)
cmd(a, "dismiss all"); run(a)
check(".dismiss all removes the rest", count(a, "^playerbots bot remove ") == 4 and next(T.companions[10]) == nil and last(a):match("home"))
cmd(a, "dismiss"); run(a)
check(".dismiss with none", last(a):match("no bots"))

-- a recruit that never arrives
clock = clock + 31
local n0 = player({ guid = 11, level = 30 })
cmd(n0, "recruit healer priest")
check("class pick inside the role", n0.cmds[1] == "playerbots bot addclass priest")
check("wrong class for the role explains", (function() clock = clock + 31; cmd(n0, "recruit tank mage"); return last(n0):match("can be:") end)())
clock = clock + 20
run(n0)
check("never arrived: expires with a message", #T.pending[11] == 0 and last(n0):match("didn't arrive"))

-- party fill -------------------------------------------------------------------------------------------------
clock = clock + 31
local pf = player({ guid = 70, level = 40, spec = "healer" })
cmd(pf, "recruit party")
check(".recruit party: 4 bots for a healer (1 tank, 3 damage, no healer)", count(pf, "addclass") == 4
    and #T.pending[70] == 4 and (function() local t, hl = 0, 0
        for _, p in ipairs(T.pending[70]) do if p.role == "tank" then t = t + 1 elseif p.role == "healer" then hl = hl + 1 end end
        return t == 1 and hl == 0 end)())
check(".recruit party message", last(pf):match("Recruiting 4 bots for a 5%-player group: 1 tank, 3 damage"))
serve(pf); run(pf)
check("party fill: no raid", pf.group.raid == false and pf.group.converted == 0)
clock = clock + 31
cmd(pf, "recruit party")
check("party full", last(pf):match("full") or last(pf):match("most allowed"))

-- raid fill (10) ---------------------------------------------------------------------------------------------
clock = clock + 31
local rd = player({ guid = 80, level = 80 })
local mate = player({ guid = 81, level = 80, spec = "tank" })
group(rd, { rd, mate })
cmd(rd, "recruit raid")
local tanks, heals, dmg = 0, 0, 0
for _, p in ipairs(T.pending[80]) do
    if p.role == "tank" then tanks = tanks + 1 elseif p.role == "healer" then heals = heals + 1 else dmg = dmg + 1 end
end
check(".recruit raid: 8 bots fill to 10 (a real tank already there)", count(rd, "addclass") == 8 and tanks == 1 and heals == 3 and dmg == 4)
check(".recruit raid: party converted to a raid", rd.group.raid == true and rd.group.converted == 1)
check(".recruit raid message", last(rd):match("Recruiting 8 bots for a 10%-player raid: 1 tank, 3 healers, 4 damage"))
serve(rd); run(rd)
check("raid: all 8 claimed", n(T.companions[80]) == 8)
check("raid: specs, maintenance, autogear for each", (function()
    for bg in pairs(T.companions[80]) do
        local m; for _, x in ipairs(rd.group.members) do if x.guid == bg then m = x end end
        if not (whispered(rd, m, "^talents spec ") and whispered(rd, m, "^maintenance$") and whispered(rd, m, "^autogear$")
            and whispered(rd, m, "^follow$")) then return false end
    end
    return true end)())
clock = clock + 31
cmd(rd, "recruit dps")
check("raid full at 10", last(rd):match("full %(10 players%)") or last(rd):match("most allowed in a raid"))
cmd(rd, "dismiss all"); run(rd)
check("raid: dismiss all sends 8 home", count(rd, "^playerbots bot remove ") == 8)
check("raid: dismissed bots leave the group (no offline members filling it)", #rd.group.members == 2)
check("raid: dismiss never calls the group's RemoveMember from the script (crashed the realm 2026-10-26)",
    not T_SRC:find("RemoveMember%s*\""))

-- raid caps: solo raid gets 9 (per player in a raid), not 4
clock = clock + 31
local solo = player({ guid = 90, level = 80 })
cmd(solo, "recruit raid")
check("solo .recruit raid: 9 bots (2 tanks, 3 healers, 4 damage)", count(solo, "addclass") == 9
    and last(solo):match("2 tanks, 3 healers, 4 damage"))
serve(solo); run(solo)
check("solo raid: group became a raid", solo.group.raid == true)
clock = clock + 31
cmd(solo, "recruit healer")
check("raid cap: 9 bots", count(solo, "addclass") == 9 and last(solo):match("most allowed in a raid"))
cmd(solo, "dismiss all"); run(solo)

-- 25 behind the switch
clock = clock + 31
local r25 = player({ guid = 91, level = 80 })
cmd(r25, "recruit raid 25")
check("25-player off by default", #r25.cmds == 0 and last(r25):match("25%-player raids are off"))
env.OV_COMPANIONS_RAID25 = "1"; env.OV_COMPANIONS_GROUP_MAX = "25"; env.OV_COMPANIONS_PER_PLAYER_RAID = "24"
cmd(r25, "recruit raid 25")
check("OV_COMPANIONS_RAID25=1: fills 25", count(r25, "addclass") == 24)
env.OV_COMPANIONS_RAID25, env.OV_COMPANIONS_GROUP_MAX, env.OV_COMPANIONS_PER_PLAYER_RAID = nil, nil, nil
events[4](4, r25)
env.OV_COMPANIONS_PER_PLAYER_RAID = "30"
local r10 = player({ guid = 92, level = 80 })
cmd(r10, "recruit raid")
check("raid size capped at 10 without the switch, whatever the env", count(r10, "addclass") == 9)
env.OV_COMPANIONS_PER_PLAYER_RAID = nil
events[4](4, r10)
env.OV_COMPANIONS_GROUP_MAX = "5"
local nr = player({ guid = 93 }); cmd(nr, "recruit raid")
check("GROUP_MAX=5: raids off", #nr.cmds == 0 and last(nr):match("off"))
env.OV_COMPANIONS_GROUP_MAX = nil

-- refusals and the instance rules -----------------------------------------------------------------------------
local c = player({ guid = 12, combat = true })
cmd(c, "recruit dps")
check("refused in combat", #c.cmds == 0 and last(c):match("Not in combat"))
local dd = player({ guid = 120, dead = true })
cmd(dd, "recruit dps")
check("refused while dead", #dd.cmds == 0 and last(dd):match("dead"))
local bgp = player({ guid = 121, bg = true })
cmd(bgp, "recruit party")
check("refused in battlegrounds", #bgp.cmds == 0 and last(bgp):match("battlegrounds or arenas"))
local d = player({ guid = 13, dungeon = true, map = 36, inst = 5 })
cmd(d, "recruit dps")
check("allowed inside a dungeon out of combat", count(d, "addclass") == 1)
local inb = serve(d, { map = 36, inst = 5, dist = 80 })[1]
local outb = serve(d, {})   -- nothing more pending
run(d)
check("in instance, same map and instance, far: moved to the player", #inb.teleports == 1 and not whispered(d, inb, "^summon$"))
clock = clock + 31
cmd(d, "recruit dps")
local other = serve(d, { map = 0, inst = 0 })[1]
run(d)
check("in instance, bot elsewhere: playerbots summon, no teleport", #other.teleports == 0 and whispered(d, other, "^summon$"))
clock = clock + 31
cmd(d, "recruit dps")
local wrongInst = serve(d, { map = 36, inst = 9, dist = 80 })[1]
run(d)
check("in instance, same map other instance: summon, no teleport", #wrongInst.teleports == 0 and whispered(d, wrongInst, "^summon$"))
clock = clock + 31
other.combat = true
cmd(d, "recruit dps")
check("in instance, group member in combat: refused", last(d):match("out of combat") and count(d, "addclass") == 3)
other.combat = false
cmd(d, "recruit raid")
check("raid fill refused inside a 5-player dungeon", last(d):match("use %.recruit party"))
local rr = player({ guid = 14, raidmap = true, map = 533, inst = 7, level = 80 })
cmd(rr, "recruit raid")
check("raid fill allowed inside a raid instance", count(rr, "addclass") == 9)
events[4](4, rr)
local lead = player({ guid = 15 }); local follower = player({ guid = 16 })
group(lead, { lead, follower })
cmd(follower, "recruit dps")
check("only the group leader", #follower.cmds == 0 and last(follower):match("leader"))
cmd(lead, "recruit")
check("usage without a role", last(lead):match("Usage"))
local lfg = player({ guid = 17 }); group(lfg).lfg = true
cmd(lfg, "recruit raid")
check("no raid fill in a Dungeon Finder group", #lfg.cmds == 0 and last(lfg):match("Dungeon Finder"))

-- realm cap
clock = clock + 31
env.OV_COMPANIONS_REALM_MAX = tostring(T.realmCount() + 2)
local r1 = player({ guid = 18 }); cmd(r1, "recruit dps 2")
local r2 = player({ guid = 19 }); cmd(r2, "recruit dps")
check("realm cap", count(r1, "addclass") == 2 and #r2.cmds == 0 and last(r2):match("out with someone"))
env.OV_COMPANIONS_REALM_MAX = nil
local beforeLogout = T.realmCount()
events[4](4, r1)
check("logout frees the slots", T.pending[18] == nil and T.realmCount() == beforeLogout - 2)

-- alts ------------------------------------------------------------------------------------------------------
chars = {
    { guid = 500, account = 7, name = "Me", level = 70, class = 1, online = 1 },
    { guid = 501, account = 7, name = "Healie", level = 68, class = 5, online = 0, spec = "healer" },
    { guid = 502, account = 7, name = "Tanky", level = 70, class = 1, online = 0, spec = "tank" },
    { guid = 503, account = 7, name = "Busy", level = 60, class = 8, online = 1 },
    { guid = 600, account = 8, name = "Stranger", level = 80, class = 2, online = 0 },
}
clock = clock + 31
local me = player({ guid = 500, account = 7, name = "Me", level = 70 })
cmd(me, "companions alts")
check(".companions alts lists own offline characters only", (function()
    local all = table.concat(me.msgs, "\n")
    return all:match("Healie %(68 Priest%)") and all:match("Tanky %(70 Warrior%)") and not all:match("Busy")
        and not all:match("Stranger") and not all:match("Me %(") end)())
check(".companions alts query is by account id", (function()
    for _, s in ipairs(sqlLog) do if s:match("FROM characters WHERE account = 7 AND online = 0 AND guid <> 500") then return true end end
    return false end)())
cmd(me, "recruit alt Stranger")
check("another account's character refused", #me.cmds == 0 and last(me):match("no character called Stranger"))
cmd(me, "recruit alt Busy")
check("an online alt refused", #me.cmds == 0 and last(me):match("online already"))
cmd(me, "recruit alt Me")
check("yourself refused", #me.cmds == 0 and last(me):match("That's you"))
cmd(me, "recruit alt Ta'nky; drop")
check("odd names refused, no SQL", #me.cmds == 0 and last(me):match("Usage"))
cmd(me, "recruit alt tanky")
check(".recruit alt: playerbots bot add with the stored name, no addclass", me.cmds[1] == "playerbots bot add Tanky" and count(me, "addclass") == 0)
check(".recruit alt message", last(me):match("Bringing Tanky"))
local tk = serve(me)[1]
run(me)
check("alt claimed as a bot", T.companions[500][502] and T.companions[500][502].alt == true and last(me):match("Tanky %(your Warrior%) joins as a bot"))
check("alt: own gear, spec, level: no init, talents, maintenance or autogear",
    count(me, "init=auto") == 0 and not whispered(me, tk, "talents") and not whispered(me, tk, "maintenance")
    and not whispered(me, tk, "autogear"))
check("alt: loot reset and follow", whispered(me, tk, "^ll normal$") and whispered(me, tk, "^follow$"))
check("alt role from its spec", T.companions[500][502].role == "tank")
clock = clock + 31
cmd(me, "recruit alt Tanky")
check("alt already with you", last(me):match("already with you"))
cmd(me, "recruit alt Healie")
serve(me); run(me)
check("second alt in", T.companions[500][501] ~= nil)
-- alts count toward the cap
clock = clock + 31
cmd(me, "recruit dps 5")
check("alts count toward the party cap (2 alts + 2 more = 4)", count(me, "addclass") == 2)
serve(me); run(me)
cmd(me, "companions save")
check(".companions save stores the alts, not the class bots", usual[7] and #usual[7] == 2 and usual[7][1].name == "Healie"
    and usual[7][2].name == "Tanky" and last(me):match("Saved your usual party: Healie, Tanky"))
cmd(me, "dismiss all"); run(me)
check(".dismiss all handles alts", count(me, "^playerbots bot remove Tanky$") == 1 and count(me, "^playerbots bot remove Healie$") == 1)
for _, ch in ipairs(chars) do if ch.guid == 501 or ch.guid == 502 then ch.online = 0 end end
me.group.members = { me }   -- the module logs them out of the group
clock = clock + 31
local adds = count(me, "^playerbots bot add ")
cmd(me, "recruit usual")
check(".recruit usual re-adds the saved alts", count(me, "^playerbots bot add ") == adds + 2
    and last(me):match("Bringing your usual party: Healie, Tanky"))
serve(me); run(me)
cmd(me, "dismiss healie"); run(me)
check(".dismiss <alt name>", me.cmds[#me.cmds] == "playerbots bot remove Healie")
local other7 = player({ guid = 700, account = 9 })
cmd(other7, "recruit usual")
check(".recruit usual with nothing saved", #other7.cmds == 0 and last(other7):match("No usual party saved"))
local noalts = player({ guid = 701, account = 10 })
cmd(noalts, "companions save")
check(".companions save with no alts", last(noalts):match("No alts with you"))
check("the usual list is read by account, joined to characters on that account", (function()
    for _, s in ipairs(sqlLog) do if s:match("JOIN characters c ON c.guid = u.guid AND c.account = u.account WHERE u.account = 7") then return true end end
    return false end)())

-- buddy
clock = clock + 31
local b = player({ guid = 20, level = 12 })
check(".buddy handled", cmd(b, "buddy") == false)
check(".buddy addclass, never a dk at 12", count(b, "addclass") == 1 and count(b, "addclass dk") == 0 and last(b):match("on the way"))
cmd(b, "buddy")
check(".buddy while on the way", last(b):match("on the way") and count(b, "addclass") == 1)
local bb = serve(b)[1]
run(b)
check("buddy claimed, follow, no spec forced", T.companions[20][bb.guid].buddy and whispered(b, bb, "^follow$")
    and not whispered(b, bb, "talents"))
cmd(b, "buddy")
check(".buddy shows the buddy", last(b):match(bb.name) and last(b):match("keeps to your level"))
b.level = 13
events[13](13, b, 12)
check("level up queues the catch-up", #b.queue == 1)
run(b)
check("buddy re-levelled with init=auto", b.cmds[#b.cmds] == "playerbots bot init=auto " .. bb.name and last(b):match("level 13"))
bb.level = 13
b.level = 14; bb.combat = true
events[13](13, b, 13); run(b)
check("in combat: catches up later", last(b):match("after the fight"))
clock = clock + 31
cmd(b, "buddy warrior")
check("one buddy only", count(b, "addclass") == 1)
cmd(b, "buddy off"); run(b)
check(".buddy off dismisses", b.cmds[#b.cmds] == "playerbots bot remove " .. bb.name and T.companions[20][bb.guid] == nil)
local bl = player({ guid = 21, level = 10 }); cmd(bl, "buddy dk")
check(".buddy dk below 55 refused", #bl.cmds == 0 and last(bl):match("from level 55"))
local bq = player({ guid = 22, level = 60 }); cmd(bq, "buddy dk")
check(".buddy dk at 60", bq.cmds[1] == "playerbots bot addclass dk")

-- a kicked companion is forgotten
clock = clock + 31
local k = player({ guid = 30 })
cmd(k, "recruit dps")
serve(k); run(k)
table.remove(k.group.members)   -- the player kicked it
cmd(k, "dismiss"); run(k)
check("left the group: pruned", last(k):match("no bots"))

-- switches
env.OV_COMPANIONS = "0"
local o = player({ guid = 40 }); cmd(o, "recruit dps")
check("OV_COMPANIONS=0: off", #o.cmds == 0 and last(o):match("off on this realm"))
env.OV_COMPANIONS = nil
env.OV_COMPANIONS_BUDDY = "0"
local ob = player({ guid = 41 }); cmd(ob, "buddy")
check("OV_COMPANIONS_BUDDY=0: off", #ob.cmds == 0 and last(ob):match("off"))
env.OV_COMPANIONS_BUDDY = nil
env.OV_COMPANIONS_PER_PLAYER = "2"
local small = player({ guid = 42 }); cmd(small, "recruit party")
check("OV_COMPANIONS_PER_PLAYER honoured", count(small, "addclass") == 2)
env.OV_COMPANIONS_PER_PLAYER = nil
env.OV_COMPANIONS_COOLDOWN = "0"
local fast = player({ guid = 43 }); cmd(fast, "recruit dps"); cmd(fast, "recruit dps")
check("OV_COMPANIONS_COOLDOWN honoured", count(fast, "addclass") == 2)
env.OV_COMPANIONS_COOLDOWN = nil

-- bots never drive this
local botp = player({ guid = 50, bot = true, accName = "RNDBOT50" })
check("bots: handled silently", cmd(botp, "recruit dps") == false and #botp.cmds == 0 and #botp.msgs == 0)

-- no mod-playerbots in the build: coming soon
local stock = player({ guid = 60, built = false })
stock.IsPlayerbot, stock.RunCommand = nil, nil
check("no bot module: handled", cmd(stock, "recruit tank") == false)
check("no bot module: coming soon, no brand", last(stock):match("Bots are coming soon to this realm"))
cmd(stock, "companions"); cmd(stock, "buddy"); cmd(stock, "dismiss all")
check("no bot module: every command says so", #stock.msgs == 4)

-- loot: every arrival got the per-bot loot reset; the realm keys are checked in the profile test below
check("every claimed bot got 'll normal'", (function()
    for _, p in ipairs({ a, pf, rd, me }) do
        for _, w in ipairs(p.whispers) do
            if w.text == "follow" and not whispered(p, w.to, "^ll normal$") then return false end
        end
    end
    return true end)())

-- the realm profile and serverpass carry the always-pass loot keys and the caps
local function slurp(path) local f = io.open(path, "rb"); if not f then return nil end; local s = f:read("*a"); f:close(); return s end
local root = file:match("^(.*)realm/custom/lua_scripts/ov_companions%.lua$")
if root then
    local prof = slurp(root .. "realm/profiles/main.env") or ""
    local sp = slurp(root .. "realm/serverpass.sh") or ""
    local function both(kv) return prof:find("\n" .. kv .. "\n", 1, true) ~= nil and sp:find(kv, 1, true) ~= nil end
    check("loot: need -> pass", both("AC_PLAYERBOTS_LOOT_NEED_ROLL_LEVEL=0"))
    check("loot: greed -> pass", both("AC_PLAYERBOTS_LOOT_GREED_ROLL_LEVEL=0"))
    check("loot: no disenchant, no recipe rolls", both("AC_PLAYERBOTS_LOOT_ROLL_DISENCHANT=0") and both("AC_PLAYERBOTS_LOOT_ROLL_RECIPE=0"))
    check("loot: no looting under free-for-all", both("AC_PLAYERBOTS_FREE_METHOD_LOOT=0"))
    -- owner 2026-10-31 "remove all live bots for now": the module cap is 0 while OV_COMPANIONS=0 (it was 9, you + 9)
    check("caps: module 0 while bots are off (9 with them on)", both(prof:find("\nOV_COMPANIONS=0\n", 1, true) and "AC_PLAYERBOTS_MAX_ADDED_BOTS=0" or "AC_PLAYERBOTS_MAX_ADDED_BOTS=9"))
    check("caps: realm 60, group 10, raid 9, 25 off", both("OV_COMPANIONS_REALM_MAX=60")
        and both("OV_COMPANIONS_GROUP_MAX=10") and both("OV_COMPANIONS_PER_PLAYER_RAID=9") and both("OV_COMPANIONS_RAID25=0"))
else
    realPrint("SKIP profile checks (run with the repo path to ov_companions.lua)")
end

-- bots are told through their PlayerbotAI, not whispered (no "To <bot>:" lines in the owner's chat) ------------
check("orders go through PlayerbotAI:HandleCommand as whispers (type 7), never Player:Whisper, when mod-ale has it",
    (function()
        local n = 0
        for _, p in ipairs({ a, pf, rd, me }) do
            for _, w in ipairs(p.whispers) do
                if w.via ~= "ai" or w.kind ~= 7 then return false end
                n = n + 1
            end
        end
        return n > 20 end)())
clock = clock + 31
local oldai = player({ guid = 150, level = 30 })
cmd(oldai, "recruit dps")
local oa = serve(oldai, { noai = true })[1]
run(oldai)
check("without GetBotAI: whispers, as before", whispered(oldai, oa, "^follow$") and oldai.whispers[#oldai.whispers].via == "whisper")

-- ownership survives a crash or a script reload ------------------------------------------------------------------
local function forget(guid) for k in pairs(T.companions[guid] or {}) do T.companions[guid][k] = nil end end
local function ownedBy(guid) local k = 0; for _, v in pairs(owned) do if v.owner == guid then k = k + 1 end end; return k end
clock = clock + 31
local cr = player({ guid = 160, level = 80 })
cmd(cr, "recruit raid")
serve(cr); run(cr)
check("ownership rows saved for every arrival", ownedBy(160) == 9)
forget(160)   -- the realm crashed and came back: the module logged the 9 bots back into the raid, our table is empty
cmd(cr, "dismiss all"); run(cr)
check("after a crash: .dismiss all finds the 9 bots in the raid (their master is you) and sends them home",
    count(cr, "^playerbots bot remove ") == 9 and last(cr):match("head home") and #cr.group.members == 1)
check("dismissed: ownership rows gone", ownedBy(160) == 0)

clock = clock + 31
local nd = player({ guid = 161, level = 40 })
cmd(nd, "recruit dps 2")
serve(nd, { noai = true }); run(nd)
forget(161)
cmd(nd, "dismiss all"); run(nd)
check("without GetBotAI: the ownership table brings them back", count(nd, "^playerbots bot remove ") == 2)

local stranger = player({ guid = 162 })
local outsider = player({ guid = 163, level = 40 })
group(outsider)
local theirs = player({ guid = 1990, account = 900, accName = "RNDBOT1990", bot = true, addclass = true, name = "Theirs" })
theirs.master = stranger
table.insert(outsider.group.members, theirs); theirs.group = outsider.group
cmd(outsider, "dismiss all"); run(outsider)
check("someone else's bot in your group is not yours", last(outsider):match("no bots") and count(outsider, "remove") == 0)

clock = clock + 31
local lg = player({ guid = 164, level = 40 })
cmd(lg, "recruit dps")
serve(lg); run(lg)
forget(164)
lg.queue = {}
events[3](3, lg)
check("login: a look for your bots is queued (15 s, 3 times)", #lg.queue == 1 and lg.queue[1].delay == 15000 and lg.queue[1].rep == 3)
run(lg)
check("login after a crash: 'back with you' once", last(lg):match("1 bot is back with you") and n(T.companions[164]) == 1
    and (function() local k = 0; for _, m in ipairs(lg.msgs) do if m:match("back with you") then k = k + 1 end end; return k == 1 end)())
events[4](4, lg)
check("a normal logout clears the ownership rows", ownedBy(164) == 0)
local bl2 = player({ guid = 165, bot = true, accName = "RNDBOT165" })
bl2.queue = {}
events[3](3, bl2)
check("bots logging in look for nothing", #bl2.queue == 0)

-- premade specs ---------------------------------------------------------------------------------------------------
check("specs: the fallback pve list without env", table.concat(T.specsFor(11), ",") == "balance pve,bear pve,resto pve,cat pve")
env.AC_PLAYERBOTS_PREMADE_SPEC_NAME_8__0 = "arcane pve"; env.AC_PLAYERBOTS_PREMADE_SPEC_NAME_8__1 = "fire pvp"
env.AC_PLAYERBOTS_PREMADE_SPEC_NAME_8__2 = "frost pve"; env.AC_PLAYERBOTS_PREMADE_SPEC_NAME_8__3 = "bad'; drop pve"
check("specs: the realm's env, pve only, odd names dropped", table.concat(T.specsFor(8), ",") == "arcane pve,frost pve")
for i = 0, 3 do env["AC_PLAYERBOTS_PREMADE_SPEC_NAME_8__" .. i] = nil end
check("spec roles", T.specRole("prot pve") == "tank" and T.specRole("bear pve") == "tank" and T.specRole("blood pve") == "tank"
    and T.specRole("resto pve") == "healer" and T.specRole("disc pve") == "healer" and T.specRole("double aura blood pve") == "dps"
    and T.specRole("frost pve") == "dps")

-- the Bots window (OVMOD, mod wucomp) -------------------------------------------------------------------------------
local function win(p, name, data, sameSecond)
    if not sameSecond then clock = clock + 1 end
    return onServerAddon(30, p, 7, "OVMOD", "wucomp\t" .. name .. "\t1/1\t" .. (data or ""), p)
end
local winPlayers = {}
local function replies(p, name)
    local out, buf = {}, nil
    for _, x in ipairs(p.addon or {}) do
        local mod, nm, i, k, data = x.msg:match("^([^\t]+)\t([^\t]+)\t(%d+)/(%d+)\t(.*)$")
        if x.prefix == "OVMOD" and mod == "wucomp" and nm == name then
            if i == "1" then buf = { data } else buf[#buf + 1] = data end
            if i == k then out[#out + 1] = table.concat(buf) end
        end
    end
    return out
end
local function said(p) local r = replies(p, "msg"); return r[#r] or "" end
local function roster(p)
    local r = { bots = {}, specs = {}, by = {} }
    local all = replies(p, "roster")
    for line in (all[#all] or ""):gmatch("[^\n]+") do
        local f = {}
        for v in (line .. "\t"):gmatch("([^\t]*)\t") do f[#f + 1] = v end
        if f[1] == "H" then
            r.count, r.cap, r.raid, r.formation, r.assist, r.cc, r.state = tonumber(f[2]), tonumber(f[3]), f[4], f[5], f[6], f[7], f[8]
        elseif f[1] == "B" then
            local b = { name = f[2], cls = tonumber(f[3]), role = f[4], kind = f[5], spec = f[6], ilvl = tonumber(f[7]),
                status = f[8], level = tonumber(f[9]) }
            r.bots[#r.bots + 1] = b; r.by[b.name] = b
        elseif f[1] == "S" then r.specs[tonumber(f[2])] = f[3] end
    end
    return r
end
local function told(p, bot, text) for _, w in ipairs(p.whispers) do if w.to == bot and w.text == text then return true end end; return false end
local function strat(bot, change) for _, x in ipairs(bot.strategies or {}) do if x == change then return true end end; return false end

clock = clock + 31
local wp = player({ guid = 200, level = 80, spec = "dps" })
winPlayers[#winPlayers + 1] = wp
win(wp, "fill", "party")
check("window: Fill the group answers in the window, not the chat", #wp.msgs == 0 and said(wp):match("Recruiting 4 bots for a 5%-player group"))
local wbots = serve(wp)
for _, b in ipairs(wbots) do b.ilvl = 200 end
run(wp)
check("window: the arrivals go to the window too", #wp.msgs == 0 and said(wp):match("joins as a %a+ bot"))
clock = clock + 16
win(wp, "roster", "")
local r = roster(wp)
check("roster: 4 bots, party cap 4, not a raid, bots on", r.count == 4 and #r.bots == 4 and r.cap == 4 and r.raid == "0" and r.state == "on")
check("roster: tank first, every row a bot with class, spec, item level, level and status", (function()
    if r.bots[1].role ~= "tank" then return false end
    for _, b in ipairs(r.bots) do
        if b.kind ~= "bot" or not b.cls or b.spec == "" or b.ilvl ~= 200 or b.level ~= 80 or b.status ~= "follow" then return false end
    end
    return true end)())
check("roster: the premade specs of every class there", (function()
    for _, b in ipairs(r.bots) do if not (r.specs[b.cls] or ""):match(" pve=") then return false end end
    return true end)())
local tankBot, dmgBot
for _, b in ipairs(wbots) do
    local row = r.by[b.name]
    if row.role == "tank" then tankBot = b elseif row.role == "dps" and not dmgBot then dmgBot = b end
end

win(wp, "order", "stay all")
r = roster(wp)
check("order stay: every bot told 'stay', status staying", (function()
    for _, b in ipairs(wbots) do if not told(wp, b, "stay") or r.by[b.name].status ~= "stay" then return false end end
    return said(wp) == "Staying here." end)())
win(wp, "order", "follow " .. tankBot.name:lower())
r = roster(wp)
check("order follow <name>: only that one", r.by[tankBot.name].status == "follow" and r.by[dmgBot.name].status == "stay"
    and said(wp):match("^" .. tankBot.name .. ": Following you%.$"))
win(wp, "order", "passive all")
r = roster(wp)
check("order passive: silent strategy change in and out of combat", strat(dmgBot, "0:+passive") and strat(dmgBot, "1:+passive")
    and r.by[dmgBot.name].status == "passive" and r.by[tankBot.name].status == "passive")
win(wp, "order", "active all")
check("order active: -passive, back to what they did (staying, following)", strat(dmgBot, "0:-passive")
    and roster(wp).by[dmgBot.name].status == "stay" and roster(wp).by[tankBot.name].status == "follow")
win(wp, "order", "attack all")
check("attack without a target: refused", said(wp) == "Target something first." and not told(wp, dmgBot, "attack"))
wp.sel = {}
win(wp, "order", "attack all")
check("attack my target", told(wp, dmgBot, "attack") and told(wp, tankBot, "attack"))
win(wp, "order", "dance all")
check("unknown order refused", said(wp):match("isn't known"))
win(wp, "order", "follow theirs")
check("an order to a bot that isn't yours", said(wp):match("None of your bots"))
win(wp, "order", "assist all")
r = roster(wp)
check("assist tank: the tank marks, damage focuses the skull", strat(tankBot, "0:+mark rti") and strat(dmgBot, "0:+dps assist")
    and told(wp, dmgBot, "rti skull") and not strat(tankBot, "0:+dps assist") and r.assist == "1")
win(wp, "order", "cc all")
check("crowd control on", strat(dmgBot, "0:+cc") and strat(tankBot, "1:+cc") and roster(wp).cc == "1")
win(wp, "order", "nocc all")
check("crowd control off", strat(dmgBot, "0:-cc") and roster(wp).cc == "0")
win(wp, "formation", "arrow")
check("formation: every bot told, the roster says so", told(wp, dmgBot, "formation arrow") and told(wp, tankBot, "formation arrow")
    and roster(wp).formation == "arrow")
win(wp, "formation", "pyramid; drop")
check("formation: only known ones", said(wp):match("isn't known") and roster(wp).formation == "arrow")

wp.combat = true
win(wp, "prepare", "")
check("prepare refused in combat", said(wp):match("out of combat") and not told(wp, dmgBot, "rebuff"))
wp.combat = false
dmgBot.combat = true
win(wp, "prepare", "")
check("prepare refused while a group member fights", said(wp):match("out of combat"))
dmgBot.combat = false
local before = #wp.whispers
win(wp, "prepare", "")
check("prepare for raid: maintenance, autogear and rebuff for each", (function()
    local seen = {}
    for i = before + 1, #wp.whispers do local w = wp.whispers[i]; seen[w.to.name .. ":" .. w.text] = true end
    for _, b in ipairs(wbots) do
        if not (seen[b.name .. ":maintenance"] and seen[b.name .. ":autogear"] and seen[b.name .. ":rebuff"]) then return false end
    end
    return said(wp):match("Getting ready") ~= nil end)())
win(wp, "prepare", "")
check("prepare: a cooldown", said(wp):match("Again in %d+ s"))

-- spec and role on a known class
clock = clock + 31
local wd = player({ guid = 210, level = 80 })
winPlayers[#winPlayers + 1] = wd
cmd(wd, "recruit tank druid")
local bear = serve(wd)[1]
run(wd)
win(wd, "spec", bear.name .. "\tcat pve")
check("spec: one of the realm's premade specs", told(wd, bear, "talents spec cat pve") and told(wd, bear, "autogear")
    and roster(wd).by[bear.name].spec == "cat pve" and roster(wd).by[bear.name].role == "dps" and said(wd):match("learns cat"))
check("spec change saved for a crash", owned[bear.guid] and owned[bear.guid].spec == "cat pve" and owned[bear.guid].role == "dps")
win(wd, "spec", bear.name .. "\tresto pve")
check("spec: one change per bot every 10 s", said(wd):match("still busy") and not told(wd, bear, "talents spec resto pve"))
clock = clock + 11
win(wd, "spec", bear.name .. "\tfrost pvp")
check("spec: not one of the list: refused", said(wd):match("isn't one") and not told(wd, bear, "talents spec frost pvp"))
win(wd, "spec", bear.name .. "\tcat pve; .server shutdown")
check("spec: odd text refused", not said(wd):match("learns"))
win(wd, "role", bear.name:lower() .. " healer")
check("role swap: healer gives a druid resto", told(wd, bear, "talents spec resto pve") and roster(wd).by[bear.name].role == "healer"
    and said(wd):match("now a healer"))
clock = clock + 11
win(wd, "role", bear.name:lower() .. " tank")
check("role swap: tank gives bear", told(wd, bear, "talents spec bear pve") and roster(wd).by[bear.name].role == "tank")
clock = clock + 11
win(wd, "gear", bear.name)
check("gear refresh: autogear", said(wd):match("picking better gear"))
clock = clock + 31
cmd(wd, "recruit dps mage")
local mage = serve(wd)[1]
run(wd)
clock = clock + 11
win(wd, "role", mage.name:lower() .. " tank")
check("role swap: a mage can't tank", said(wd):match("A Mage can't be a tank") and not told(wd, mage, "talents spec prot pve"))
win(wd, "dismiss", mage.name:lower())
run(wd)
check("window dismiss one", count(wd, "^playerbots bot remove " .. mage.name .. "$") == 1 and said(wd):match("heads home"))

-- alts keep their own talents and gear
winPlayers[#winPlayers + 1] = me
local tk2
for _, m in ipairs(me.group.members) do if m.name == "Tanky" then tk2 = m end end
win(me, "spec", "Tanky\tarms pve")
check("alts: no spec change", said(me):match("your own character") and not told(me, tk2, "talents spec arms pve"))
win(me, "gear", "Tanky")
check("alts: no gear change", said(me):match("your own character"))
local mb = #me.whispers
win(me, "prepare", "")
check("alts: prepare only rebuffs", (function()
    for i = mb + 1, #me.whispers do local w = me.whispers[i]; if w.to == tk2 and (w.text == "maintenance" or w.text == "autogear") then return false end end
    return told(me, tk2, "rebuff") end)())
check("alts: kind alt in the roster", roster(me).by.Tanky and roster(me).by.Tanky.kind == "alt")

-- the realm checks everything
local fl = 0
local fz = player({ guid = 220 })
for i = 1, 6 do win(fz, "roster", "", true) end
check("window: at most 4 requests a second", #replies(fz, "roster") == 4)
local wb = player({ guid = 221, bot = true, accName = "RNDBOT221" })
win(wb, "fill", "party")
check("window: bots can't drive it", #wb.cmds == 0 and #replies(wb, "msg") == 0)
local ws = player({ guid = 222, built = false })
ws.IsPlayerbot, ws.RunCommand = nil, nil
win(ws, "roster", "")
check("window on a realm without bots: roster says coming soon", roster(ws).state == "soon")
win(ws, "fill", "party")
check("window on a realm without bots: the answer says so", said(ws):match("coming soon"))
env.OV_COMPANIONS = "0"
win(wp, "roster", "")
check("window with bots off: roster says off", roster(wp).state == "off")
env.OV_COMPANIONS = nil
local chatBefore = #wp.msgs
cmd(wp, "companions")
check("a typed command answers in the chat again", #wp.msgs > chatBefore and #replies(wp, "msg") > 0)
win(wp, "dismiss", "all"); run(wp)
check("window dismiss all", count(wp, "^playerbots bot remove ") == 4 and roster(wp).count == 0)
check("window answers never go to the chat while the window drives", (function()
    for _, p in ipairs(winPlayers) do
        for _, x in ipairs(replies(p, "msg")) do if x:find("|cff66ccff[Bots]", 1, true) then return false end end
    end
    return true end)())

-- the window's tabs: picks, pull, alts, buddy, dungeons ------------------------------------------------------------
local function head(p)
    local all = replies(p, "roster")
    for line in (all[#all] or ""):gmatch("[^\n]+") do
        if line:sub(1, 2) == "H\t" then local f = {}; for v in (line .. "\t"):gmatch("([^\t]*)\t") do f[#f + 1] = v end; return f end
    end
    return {}
end
local PK = T.parsePicks("tank=druid:bear pve,healer=shaman,dps=mage:fire pve,dps=any,tank=mage,healer=priest:shadow pve,dps=bard,heals=paladin")
check("picks: class and spec per slot", PK.tank[1].word == "druid" and PK.tank[1].spec == "bear pve" and PK.healer[1].word == "shaman"
    and PK.healer[1].spec == nil and PK.dps[1].word == "mage" and PK.dps[1].spec == "fire pve" and PK.dps[2].word == nil)
check("picks: a class that can't take the role, a spec of another role and unknown words are dropped",
    PK.tank[2].word == nil and PK.healer[2].word == "priest" and PK.healer[2].spec == nil and PK.dps[3].word == nil
    and PK.healer[3].word == "paladin")
check("picks: odd text gives no picks", #T.parsePicks("tank=warrior; drop table").tank == 0 and #T.parsePicks("").dps == 0)

clock = clock + 31
local wq = player({ guid = 230, level = 80, spec = "dps" })
winPlayers[#winPlayers + 1] = wq
win(wq, "fill", "party\ttank=druid:bear pve,healer=shaman,dps=mage:fire pve,dps=any")
check("fill with picks: the picked classes are recruited", count(wq, "^playerbots bot addclass druid$") == 1
    and count(wq, "^playerbots bot addclass shaman$") == 1 and count(wq, "^playerbots bot addclass mage$") >= 1
    and count(wq, "addclass") == 4 and said(wq):match("Recruiting 4 bots"))
local qb = serve(wq)
run(wq)
local qmage, qdruid, qsham
for _, x in ipairs(qb) do
    if x.class == 8 and not qmage then qmage = x elseif x.class == 11 then qdruid = x elseif x.class == 7 then qsham = x end
end
check("fill with picks: the picked specs are taught", told(wq, qdruid, "talents spec bear pve") and told(wq, qmage, "talents spec fire pve")
    and told(wq, qsham, "talents spec resto pve"))
win(wq, "roster", "")
local hf = head(wq)
check("roster head: buddy on and raids on", hf[9] == "1" and hf[10] == "1")
env.OV_COMPANIONS_BUDDY, env.OV_COMPANIONS_GROUP_MAX = "0", "5"
win(wq, "roster", "")
hf = head(wq)
check("roster head: buddy off and raids off", hf[9] == "0" and hf[10] == "0")
env.OV_COMPANIONS_BUDDY, env.OV_COMPANIONS_GROUP_MAX = nil, nil
win(wq, "order", "pull all")
check("pull without a target: refused", said(wq) == "Target something first.")
wq.sel = {}
local wb0 = #wq.whispers
win(wq, "order", "pull all")
check("pull: only the tank opens on your target", (function()
    for i = wb0 + 1, #wq.whispers do
        local w = wq.whispers[i]
        if w.text == "attack" and w.to ~= qdruid then return false end
    end
    return told(wq, qdruid, "attack") and said(wq) == "Pulling your target."
end)())
win(wq, "dismiss", "all"); run(wq)
local solo = player({ guid = 231, level = 80, spec = "dps" })
clock = clock + 31
cmd(solo, "recruit dps mage"); local sm = serve(solo)[1]; run(solo)
solo.sel = {}
win(solo, "order", "pull all")
check("pull without a tank bot: everyone attacks", told(solo, sm, "attack"))
winPlayers[#winPlayers + 1] = solo

-- Your alts tab
table.insert(chars, { guid = 801, account = 11, name = "Alto", level = 60, class = 11, online = 0 })
table.insert(chars, { guid = 802, account = 11, name = "Bassa", level = 65, class = 7, online = 0 })
clock = clock + 31
local ma = player({ guid = 800, account = 11, name = "Mains", level = 70 })
winPlayers[#winPlayers + 1] = ma
win(ma, "alts", "")
local al = replies(ma, "alts")[1] or ""
check("alts: the account's offline characters, none with you, no usual party", al:find("A\tAlto\t60\t11\t0\t0", 1, true)
    and al:find("A\tBassa\t65\t7\t0\t0", 1, true) and not al:find("U\t", 1, true) and not al:find("Stranger", 1, true))
win(ma, "alt", "Stranger")
check("alt: another account's character refused", #ma.cmds == 0 and said(ma):match("no character called Stranger"))
win(ma, "alt", "Alto")
check("alt: brought through the module's add", ma.cmds[1] == "playerbots bot add Alto" and said(ma):match("Bringing Alto"))
serve(ma); run(ma)
check("alt: joined, and the list is sent again", T.companions[800][801] and #replies(ma, "alts") >= 2)
win(ma, "save", "")
al = replies(ma, "alts")[#replies(ma, "alts")]
check("save: the usual party saved and shown; Alto shows as with you", usual[11] and usual[11][1].name == "Alto"
    and al:find("U\tAlto", 1, true) and al:find("A\tAlto\t0\t11\t1\t1", 1, true) and said(ma):match("Saved your usual party"))
win(ma, "dismiss", "all"); run(ma)

-- Buddy tab
clock = clock + 31
local mb = player({ guid = 810, level = 20 })
winPlayers[#winPlayers + 1] = mb
win(mb, "buddy", "bard")
check("buddy: an unknown class: usage", #mb.cmds == 0 and said(mb):match("Usage"))
win(mb, "buddy", "mage")
check("buddy: the picked class", mb.cmds[1] == "playerbots bot addclass mage" and said(mb):match("on the way"))
local mbb = serve(mb)[1]; run(mb)
win(mb, "roster", "")
check("buddy: in the roster as the buddy", roster(mb).by[mbb.name] and roster(mb).by[mbb.name].kind == "buddy")
win(mb, "buddy", "any")
check("buddy: asking again shows the buddy", said(mb):match(mbb.name) and #mb.cmds == 1)
win(mb, "buddy", "off"); run(mb)
check("buddy off: it leaves and logs out", told(mb, mbb, "leave") and mb.cmds[#mb.cmds] == "playerbots bot remove " .. mbb.name
    and said(mb):match("heads home"))
env.OV_COMPANIONS_BUDDY = "0"
win(mb, "buddy", "mage")
check("buddy off on the realm: says so", said(mb):match("buddy is off") and #mb.cmds == 2)
env.OV_COMPANIONS_BUDDY = nil

-- Dungeons tab
local nd = 0
for _ in pairs(T.DUNGEONS) do nd = nd + 1 end
check("dungeons: the list loaded through require, with doors and levels", nd >= 50 and T.DUNGEONS.deadmines
    and T.DUNGEONS.deadmines.map == 36 and T.DUNGEONS.deadmines.min == 10 and T.DUNGEONS.deadmines.pos[1] == 36)
clock = clock + 31
local md = player({ guid = 820, level = 15, spec = "dps" })
winPlayers[#winPlayers + 1] = md
function md:Teleport(...)
    local t = { ... }
    self.teleports[#self.teleports + 1] = t
    if self.refuse then return false end
    self.map, self.dungeon = t[1], true
    return true
end
win(md, "dungeon", "atlantis")
check("dungeon: not on the list", said(md):match("isn't on the list") and #md.teleports == 0)
md.combat = true
win(md, "dungeon", "deadmines")
check("dungeon: not in combat", said(md) == "Not in combat." and #md.cmds == 0 and #md.teleports == 0)
md.combat, md.bg = false, true
win(md, "dungeon", "deadmines")
check("dungeon: not from a battleground", said(md):match("battlegrounds") and #md.teleports == 0)
md.bg = false
local low = player({ guid = 821, level = 8 })
winPlayers[#winPlayers + 1] = low
win(low, "dungeon", "deadmines")
check("dungeon: below its level: refused", said(low):match("opens at level 10") and #low.cmds == 0 and #low.teleports == 0)
win(md, "dungeon", "deadmines\ttank=warrior:prot pve")
check("dungeon: the party is filled (with the picks)", count(md, "addclass") == 4 and count(md, "^playerbots bot addclass warrior$") == 1)
check("dungeon: taken to the door", #md.teleports == 1 and md.teleports[1][1] == 36 and md.teleports[1][2] == -16.4
    and said(md):match("^To The Deadmines: your bots meet you at the door%.$"))
local mdb = serve(md)   -- they log in out in the world
run(md)
check("dungeon: the bots that join are fetched into the instance (summon)", (function()
    for _, x in ipairs(mdb) do if not told(md, x, "summon") then return false end end
    return #mdb == 4 end)())
win(md, "dungeon", "deadmines")
check("dungeon: already inside it: no trip, says so", #md.teleports == 1 and said(md):match("already"))
win(md, "dungeon", "wailing-caverns")
check("dungeon: inside another one: leave first", said(md):match("Leave this dungeon first") and #md.teleports == 1)
md.map, md.dungeon = 0, false
win(md, "dungeon", "wailing-caverns")
check("dungeon: one trip every 5 minutes", said(md):match("travel to a dungeon again in %d+ s") and #md.teleports == 1)
clock = clock + 301
env.OV_COMPANIONS_DUNGEON_TRAVEL = "0"
win(md, "dungeon", "wailing-caverns")
check("dungeon: travel off: the party only", #md.teleports == 1 and said(md):match("^Head to Wailing Caverns"))
env.OV_COMPANIONS_DUNGEON_TRAVEL = nil
md.refuse = true
win(md, "dungeon", "wailing-caverns")
check("dungeon: the core refuses the door: says so", said(md):match("closed to you"))
md.refuse = nil
local before = #md.teleports
win(md, "dungeon", "wailing-caverns")
check("dungeon: the bots already with you are brought after the trip", (function()
    local n0 = #md.whispers
    run(md)
    for _, x in ipairs(mdb) do
        local got = false
        for i = n0 + 1, #md.whispers do if md.whispers[i].to == x and md.whispers[i].text == "summon" then got = true end end
        if not got then return false end
    end
    return #md.teleports == before + 1 end)())
local mr = player({ guid = 822, level = 30 })
winPlayers[#winPlayers + 1] = mr
group(mr).raid = true
win(mr, "dungeon", "deadmines")
check("dungeon: not as a raid", said(mr):match("You are in a raid"))
local lead = player({ guid = 823, level = 30 })
local mf = player({ guid = 824, level = 30 })
winPlayers[#winPlayers + 1] = mf
group(lead, { lead, mf })
win(mf, "dungeon", "deadmines")
check("dungeon: group leader only", said(mf):match("Only the group leader") and #mf.teleports == 0)
win(md, "dismiss", "all"); run(md)
for _, p in ipairs({ wq, solo, ma, mb, md, low, mr, mf }) do for _, x in ipairs(replies(p, "msg")) do allMsgs[#allMsgs + 1] = x end end
for _, p in ipairs({ wp, wd, me, ws }) do for _, x in ipairs(replies(p, "msg")) do allMsgs[#allMsgs + 1] = x end end

-- single player: bots that join by themselves (OV_COMPANIONS_AUTO, _FOLLOW, _DUNGEON_AUTO) ------------------------
local function addclasses(p)
    local out = {}
    for _, c in ipairs(p.cmds) do local w = c:match("^playerbots bot addclass (%a+)$"); if w then out[#out + 1] = w end end
    return out
end
local function saw(p, pat) for _, m in ipairs(p.msgs) do if m:match(pat) then return true end end; return false end
local function isRole(word, role) for _, pk in ipairs(T.ROLES[role].picks) do if pk[1] == word then return true end end; return false end
local function login(p) p.queue = {}; events[3](3, p); return run(p, 1) end   -- the login's 6 s wait, then the ask

-- off unless the profile says so: a realm without the keys (and the official one) never does any of it
clock = clock + 31
local off = player({ guid = 320, level = 10 })
login(off); run(off)
check("auto: off by default: nobody joins by themselves", #addclasses(off) == 0)
off.dungeon, off.map = true, 36
events[28](28, off); run(off)
check("auto: off by default: entering a dungeon fills nothing", #addclasses(off) == 0)
do
    local saved = {}
    for k, v in pairs(env) do saved[k] = v end
    local f = assert(io.open((file:match("^(.*/)custom/lua_scripts/[^/]+$") or "../realm/") .. "profiles/main.env"))
    for line in f:lines() do local k, v = line:match("^([%w_]+)=(.*)$"); if k then env[k] = v end end
    f:close()
    local o = player({ guid = 321, level = 10 })
    login(o); run(o)
    o.dungeon, o.map = true, 36
    events[28](28, o); run(o)
    check("auto: the official realm's main.env: nothing joins, nothing fills", #addclasses(o) == 0)
    env.OV_COMPANIONS_AUTO, env.OV_COMPANIONS_DUNGEON_AUTO = "1", "1"
    local o2 = player({ guid = 322, level = 10 })
    login(o2); run(o2)
    check("auto: even if main.env named them, bots off there (OV_COMPANIONS=0) wins", #addclasses(o2) == 0)
    for k in pairs(env) do env[k] = nil end
    for k, v in pairs(saved) do env[k] = v end
end

env.OV_COMPANIONS_AUTO, env.OV_COMPANIONS_DUNGEON_AUTO = "1", "1"

-- first entry: a damage dealer gets a healer and a tank, with one welcome line
local fe = player({ guid = 300, level = 10, spec = "dps" })
login(fe)
local w1 = addclasses(fe)
check("first entry: 2 bots asked for by themselves (OV_COMPANIONS_FOLLOW default 2)", #w1 == 2)
check("first entry: damage dealer -> a tank and a healer", isRole(w1[1], "tank") and isRole(w1[2], "healer"))
check("first entry: nothing said before they arrive", #fe.msgs == 0)
check("first entry: the choice is on record ('new') before they arrive", autos[300] and autos[300].mode == "new")
serve(fe); run(fe)
check("first entry: both joined, kept to your level (ownership kind 'auto')", n(T.companions[300]) == 2
    and (function() for b, v in pairs(owned) do if v.owner == 300 and v.kind ~= "auto" then return false end end; return true end)())
local welcome = table.concat(fe.msgs, "\n")
check("first entry: one welcome line names them and their roles", welcome:match("Your bots are here: %a+ the %a+ %(tank%) and %a+ the %a+ %(healer%)%. They follow you")
    and (function() local k = 0; for _, m in ipairs(fe.msgs) do if m:match("Your bots are here") then k = k + 1 end end; return k == 1 end)())
check("first entry: no per-bot 'joins as' lines", not welcome:match("joins as"))
check("first entry without the Bots window: how to change them", welcome:match("Change them any time: %.companions"))
check("first entry: the party is remembered", autos[300].mode == "on" and autos[300].party:match("^tank=%a+:[%a ]+ pve,healer=%a+:[%a ]+ pve$"))
check("first entry: no cooldown taken from the player", (function()
    local before = #fe.cmds; cmd(fe, "recruit dps"); local ok = #fe.cmds == before + 1
    serve(fe); run(fe); cmd(fe, "dismiss " .. T.companions[300][fe.group.members[#fe.group.members].guid].name:lower()); run(fe)
    return ok end)())

-- they keep to your level, with their role's spec
fe.level = 11
events[13](13, fe, 10); run(fe)
check("level up: both re-levelled (init=auto) and re-specced", count(fe, "^playerbots bot init=auto ") == 2
    and (function() local k = 0; for _, w in ipairs(fe.whispers) do if w.text:match("^talents spec ") then k = k + 1 end end; return k >= 4 end)())
check("level up: one line", saw(fe, "Your bots keep up: level 11"))

-- logout and back: the same party returns
local party1 = autos[300].party
events[4](4, fe)
check("logout: the party is kept", autos[300].mode == "on" and autos[300].party == party1)
local fe2 = player({ guid = 300, level = 11, spec = "dps" })
login(fe2)
local w2 = addclasses(fe2)
check("login: the same classes come back", #w2 == 2 and party1:match("^tank=" .. w2[1] .. ":") and party1:match(",healer=" .. w2[2] .. ":"))
serve(fe2); run(fe2)
check("login: 'Your bots are back', no welcome again", saw(fe2, "Your bots are back: ") and not saw(fe2, "Your bots are here")
    and not saw(fe2, "%.companions"))
check("login: they get their saved spec", (function()
    local k = 0
    for item in party1:gmatch("[^,]+") do
        local spec = item:match(":(.*)$")
        for _, w in ipairs(fe2.whispers) do if w.text == "talents spec " .. spec then k = k + 1; break end end
    end
    return k == 2 end)())

-- sent home on purpose: kept, nothing joins by itself
cmd(fe2, "dismiss all"); run(fe2)
check("dismiss all: remembered as a choice", autos[300].mode == "off" and saw(fe2, "won't join you by themselves"))
events[4](4, fe2)
check("logout after dismissing: still off", autos[300].mode == "off")
local fe3 = player({ guid = 300, level = 11, spec = "dps" })
login(fe3); run(fe3)
check("login after dismissing: nobody joins", #addclasses(fe3) == 0)
-- bringing bots again turns it back on
clock = clock + 31
cmd(fe3, "recruit tank"); serve(fe3); run(fe3)
events[4](4, fe3)
check("a recruit later: kept again ('on', just what was with you)", autos[300].mode == "on" and autos[300].party:match("^tank=%a+:[%a ]+ pve$"))
local fe4 = player({ guid = 300, level = 11, spec = "dps" })
login(fe4)
check("login: that one comes back (no more than you had)", #addclasses(fe4) == 1 and isRole(addclasses(fe4)[1], "tank"))
serve(fe4); run(fe4)

-- dungeons: the free slots fill on entry, and only those go home on leaving
fe4.dungeon, fe4.map = true, 36
events[28](28, fe4)
check("dungeon: nothing until the loading screen is over (4 s)", #addclasses(fe4) == 1 and fe4.queue[#fe4.queue].delay == 4000)
run(fe4, 1)
local dw = addclasses(fe4)
check("dungeon: 3 dungeon bots fill the party (a healer and damage: the tank is with you)", #dw == 4
    and isRole(dw[2], "healer") and isRole(dw[3], "dps") and isRole(dw[4], "dps"))
serve(fe4, { map = 36 }); run(fe4)
check("dungeon: one line, and they are marked dungeon bots", saw(fe4, "Dungeon bots join you: .+%. They head home when you leave%.")
    and (function() local k = 0; for _, v in pairs(owned) do if v.owner == 300 and v.kind == "temp" then k = k + 1 end end; return k == 3 end)())
check("dungeon: full party", #fe4.group.members == 5)
events[28](28, fe4); run(fe4)
check("dungeon: a second entry (same dungeon) adds nobody", #addclasses(fe4) == 4)
local removes = count(fe4, "^playerbots bot remove ")
fe4.dungeon, fe4.map, fe4.dead = false, 0, true
events[28](28, fe4); run(fe4)
check("dungeon: a ghost running back keeps them", count(fe4, "^playerbots bot remove ") == removes and n(T.companions[300]) == 4)
fe4.dead = false
events[28](28, fe4); run(fe4)
check("dungeon: leaving sends just the dungeon bots home", count(fe4, "^playerbots bot remove ") == removes + 3
    and n(T.companions[300]) == 1 and saw(fe4, "Your dungeon bots head home"))
events[4](4, fe4)
check("dungeon bots are never remembered", autos[300].party:match("^tank=%a+:[%a ]+ pve$"))

-- a new world's defaults (realm/profiles/solo.env, owner 12 Nov: no party by itself, dungeon buddies on), read from
-- the file itself; then the two switches each on their own
do
    local saved = {}
    for k, v in pairs(env) do saved[k] = v end
    local f = assert(io.open((file:match("^(.*/)custom/lua_scripts/[^/]+$") or "../realm/") .. "profiles/solo.env"))
    for line in f:lines() do local k, v = line:match("^([%w_]+)=(.*)$"); if k then env[k] = v end end
    f:close()
    check("solo.env: a new world has OV_COMPANIONS_AUTO=0 and OV_COMPANIONS_DUNGEON_AUTO=1", env.OV_COMPANIONS_AUTO == "0"
        and env.OV_COMPANIONS_DUNGEON_AUTO == "1")
    env.OV_COMPANIONS_REALM_MAX = "2000"   -- this test's realm already has many bots out; the cap isn't what's tested
    clock = clock + 31
    local nw = player({ guid = 340, level = 12, spec = "dps" })
    login(nw); run(nw)
    check("AUTO=0: the first entry brings no party, says nothing, keeps no record", #addclasses(nw) == 0 and #nw.msgs == 0
        and autos[340] == nil)
    events[4](4, nw)
    check("AUTO=0: logout keeps no record either", autos[340] == nil)
    -- a character that had a party while AUTO was 1 (300: mode on, a tank): AUTO=0 now, so it doesn't come back
    local was = autos[300] and autos[300].mode
    local back = player({ guid = 300, level = 11, spec = "dps" })
    login(back); run(back)
    check("AUTO=0: a party remembered from before doesn't come back by itself", was == "on" and #addclasses(back) == 0)
    events[4](4, back)
    check("AUTO=0: and its record is left as it was", autos[300].mode == "on")
    -- dungeon buddies still on: walking into a 5-player dungeon fills the party
    nw = player({ guid = 341, level = 12, spec = "dps" })
    login(nw); run(nw)
    nw.dungeon, nw.map = true, 36
    events[28](28, nw); run(nw, 1)
    check("AUTO=0, DUNGEON_AUTO=1: a dungeon still fills the party (4 dungeon bots)", #addclasses(nw) == 4)
    -- the other way round: a party on entry, but no dungeon buddies
    env.OV_COMPANIONS_AUTO, env.OV_COMPANIONS_FOLLOW, env.OV_COMPANIONS_DUNGEON_AUTO = "1", "2", "0"
    clock = clock + 31
    local d0 = player({ guid = 342, level = 12, spec = "dps" })
    login(d0)
    check("AUTO=1: the first entry's party is asked for", #addclasses(d0) == 2)
    serve(d0); run(d0)
    d0.dungeon, d0.map = true, 36
    events[28](28, d0); run(d0)
    check("DUNGEON_AUTO=0: entering a dungeon adds no dungeon bots", #addclasses(d0) == 2)
    for k in pairs(env) do env[k] = nil end
    for k, v in pairs(saved) do env[k] = v end
end

-- the party covers what you are not; the count; the first era has no Death Knights
local tk = player({ guid = 330, level = 70, spec = "tank" })
local roles = {}
for _, p in ipairs(T.freshEntries(tk, 2)) do roles[#roles + 1] = p.role end
table.sort(roles)
check("you tank -> a healer and damage", table.concat(roles, ",") == "dps,healer")
local hl = player({ guid = 331, level = 70, spec = "healer" })
roles = {}
for _, p in ipairs(T.freshEntries(hl, 2)) do roles[#roles + 1] = p.role end
table.sort(roles)
check("you heal -> a tank and damage", table.concat(roles, ",") == "dps,tank")
env.OV_COMPANIONS_FOLLOW = "4"
local f4 = player({ guid = 332, level = 20 })
login(f4)
check("OV_COMPANIONS_FOLLOW=4: four (1 tank, 1 healer, 2 damage)", #addclasses(f4) == 4)
serve(f4); run(f4)
check("four names: the welcome stays short (two lines)", saw(f4, "^.-Your bots are here: .+%.$") and saw(f4, "They follow you"))
env.OV_COMPANIONS_FOLLOW = "0"
local f0 = player({ guid = 333, level = 20 })
login(f0); run(f0)
check("OV_COMPANIONS_FOLLOW=0: nobody", #addclasses(f0) == 0)
env.OV_COMPANIONS_FOLLOW = "9"
local f9 = player({ guid = 334, level = 20 })
login(f9)
check("OV_COMPANIONS_FOLLOW past 4: four (a party)", #addclasses(f9) == 4)
serve(f9); run(f9)
env.OV_COMPANIONS_FOLLOW = nil
env.AC_MAX_PLAYER_LEVEL = "60"
check("the original world (cap 60): never a Death Knight", (function()
    local era = player({ guid = 335, level = 58 })
    for _ = 1, 60 do for _, p in ipairs(T.freshEntries(era, 4)) do if p.word == "dk" then return false end end end
    return #T.partyEntries(era, "tank=dk:blood pve,healer=priest:holy pve") == 1
end)())
env.AC_MAX_PLAYER_LEVEL = nil

-- in a fight at login: they wait, then come
local cb = player({ guid = 336, level = 20, combat = true })
login(cb)
check("in combat: nobody yet, asked again in 10 s", #addclasses(cb) == 0 and cb.queue[#cb.queue].delay == 10000)
cb.combat = false
run(cb, 4)
check("out of combat: they come", #addclasses(cb) == 2)
serve(cb); run(cb)

-- the Bots window is there: it says the welcome (with its key)
local ww = player({ guid = 337, level = 20 })
win(ww, "roster")
login(ww); serve(ww); run(ww)
check("with the window: the welcome goes to the add-on, not the chat", #replies(ww, "welcome") == 1
    and replies(ww, "welcome")[1]:match("^Your bots are here: .+ They follow you") and not saw(ww, "Your bots are here"))

-- bots logging in, and after a crash
local bb = player({ guid = 1500, bot = true, accName = "RNDBOT1500", level = 20 })
bb.queue = {}; events[3](3, bb)
check("auto: bots logging in ask for nothing", #bb.queue == 0)
local cr2 = player({ guid = 338, level = 20 })
owned[4242] = { owner = 338, name = "Crashy", role = "tank", spec = "", kind = "auto" }
login(cr2); run(cr2)
check("auto: after a crash (ownership rows left) nothing new is asked for: they are adopted", #addclasses(cr2) == 0)
owned[4242] = nil

-- the remembered party is checked on the way back in
check("remembered party: a class that can't take the role is dropped",
    #T.partyEntries(player({ guid = 339, level = 80 }), "tank=mage:frost pve,healer=priest:holy pve,buddy=rogue") == 2)
env.OV_COMPANIONS_AUTO, env.OV_COMPANIONS_DUNGEON_AUTO = nil, nil

-- names: no brand names, and companions are called bots
-- raw module commands from players: the same limits as .recruit --------------------------------------------
local rw = player({ guid = 700, level = 30 })
check("raw: list passes through", cmd(rw, "playerbots bot list") == nil and cmd(rw, ".playerbots bot remove all") == nil)
check("raw: other module commands untouched", cmd(rw, "playerbots rndbot stats") == nil)
check("raw: addclass handled, not passed through", cmd(rw, ".playerbots bot addclass mage") == false)
check("raw: addclass runs one addclass of that class through the queue", count(rw, "^playerbots bot addclass mage$") == 1
    and #T.pending[700] == 1 and last(rw):match("Recruiting a Mage bot"))
check("raw: the cooldown applies", cmd(rw, "playerbots bot addclass priest") == false and count(rw, "addclass") == 1
    and last(rw):match("recruit again in"))
clock = clock + 31
for _ = 1, 3 do cmd(rw, "bot addclass warrior"); clock = clock + 31 end
check("raw: .bot alias and the per-player cap (4 in a party)", count(rw, "addclass") == 4 and #T.pending[700] == 4)
cmd(rw, "playerbots bot addclass rogue")
check("raw: past the cap: refused with the reason", count(rw, "addclass") == 4 and last(rw):match("the most allowed"))
check("raw: addaccount refused, friendly", cmd(rw, "playerbots bot addaccount SOMEONE") == false
    and count(rw, "addaccount") == 0 and last(rw):match("%.recruit"))
check("raw: init=epic refused", cmd(rw, "playerbot bot init=epic all") == false and count(rw, "init=epic") == 0)
check("raw: add a list refused", cmd(rw, "playerbots bot add Aa,Bb") == false and count(rw, " add Aa") == 0)
check("raw: addclass of an unknown class: usage", cmd(rw, "playerbots bot addclass bard") == false and last(rw):match("Usage"))
local rdk = player({ guid = 701, level = 30 })
check("raw: no dk below its level", cmd(rdk, "playerbots bot addclass dk") == false and count(rdk, "addclass") == 0
    and last(rdk):match("Death Knights join"))
local gm = player({ guid = 702, level = 30 })
function gm:GetGMRank() return 2 end
check("raw: GMs pass straight through", cmd(gm, "playerbots bot addclass mage") == nil and #gm.cmds == 0)
local off = player({ guid = 703, built = false })
check("raw: without bots built: coming soon", cmd(off, "playerbots bot addclass mage") == false and last(off):match("coming soon"))
env.OV_COMPANIONS = "0"
local rq = player({ guid = 704, level = 30 })
check("raw: bots off: refused", cmd(rq, "playerbots bot addclass mage") == false and count(rq, "addclass") == 0
    and last(rq):match("Bots are off"))
env.OV_COMPANIONS = nil
check("raw: our own adds aren't caught by the guard", (function()
    local own = player({ guid = 705, level = 30 })
    function own:RunCommand(c) self.cmds[#self.cmds + 1] = c; self.hook = events[42](42, self, c) end
    clock = clock + 31
    cmd(own, "recruit tank")
    return #own.cmds == 1 and own.hook == nil
end)())

check("no brand names in anything players saw", (function()
    for _, m in ipairs(allMsgs) do
        local l = m:lower()
        for _, w in ipairs(BANNED) do
            if l:find(w, 1, true) and not (w == "wow" and not l:find("%f[%a]wow%f[%A]")) then return false end
        end
    end
    return true end)())
check("players only ever hear 'bots' (the .companions command aside)", (function()
    for _, m in ipairs(allMsgs) do
        if m:lower():gsub("%.companions?", ""):find("companion") then return false end
    end
    return true end)())
check("messages are short", (function()
    for _, m in ipairs(allMsgs) do if #m:gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", "") > 170 then realPrint("  long: " .. m); return false end end
    return true end)())

realPrint(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
