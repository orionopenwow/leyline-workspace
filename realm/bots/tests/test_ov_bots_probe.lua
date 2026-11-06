-- lua5.1 realm/bots/tests/test_ov_bots_probe.lua realm/bots/smoke/ov_bots_probe.lua
-- The bot smoke test's probe (ov_bots_probe.lua) under a fake mod-ale: .ovsmoke report / compat / pull / fight answer
-- one "OVSMOKE {json}" system line each, with what the group looks like; GM only; other commands are left alone.
-- (texlua works too; hosting/tests/test_bots_updates.py runs this.)
local file = assert(arg[1], "usage: lua5.1 test_ov_bots_probe.lua <ov_bots_probe.lua>")
local pass, fail = 0, 0
local realPrint = print
local function check(name, ok, extra)
    if ok then pass = pass + 1; realPrint("PASS " .. name)
    else fail = fail + 1; realPrint("FAIL " .. name .. (extra ~= nil and ("  " .. tostring(extra)) or "")) end
end
local logged = {}
print = function(s) logged[#logged + 1] = s end

local onCommand
function RegisterPlayerEvent(e, f) if e == 42 then onCommand = f end end

local function unit(t)
    t.lines = {}
    function t:GetName() return self.name end
    function t:GetLevel() return self.level or 20 end
    function t:GetClass() return self.class or 1 end
    function t:GetMapId() return self.map or 36 end
    function t:GetInstanceId() return self.inst or 7 end
    function t:IsDead() return self.dead == true end
    function t:IsInCombat() return self.combat == true end
    function t:GetX() return self.x or 0 end
    function t:GetY() return self.y or 0 end
    function t:GetZ() return self.z or 0 end
    function t:GetO() return self.o or 0 end
    function t:GetDistance(o) return math.sqrt((self:GetX() - o:GetX()) ^ 2 + (self:GetY() - o:GetY()) ^ 2) end
    function t:GetGUIDLow() return self.guid or 1 end
    function t:GetGUID() return self.guid or 1 end
    return t
end

local master = unit({ name = "Smk", guid = 1, gm = 3 })
local ai = { GetMaster = function() return master end, HandleCommand = function() end, ChangeStrategy = function() end }
local bot = unit({ name = "Tankbot", guid = 2, x = 3, combat = false })
function bot:IsPlayerbot() return true end
function bot:GetBotAI() return ai end
function bot:RunCommand() end
function master:IsPlayerbot() return false end
function master:GetGMRank() return self.gm end
function master:SendBroadcastMessage(s) self.lines[#self.lines + 1] = s end
local group = { GetMembers = function() return { master, bot } end, IsRaidGroup = function() return false end }
function master:GetGroup() return group end

local mob = unit({ name = "Defias Thug", guid = 99, level = 10 })
function mob:GetHealthPct() return self.hp or 100 end
function mob:GetVictim() return master end
function mob:SetFaction(f) self.faction = f end
function mob:AttackStart(t) self.target = t end
local spawned
function master:SpawnCreature(entry, x, y, z, o, kind, ms) spawned = { entry = entry, kind = kind, ms = ms, x = x }; return mob end
local map = { GetWorldObject = function(_, guid) return guid == 99 and mob or nil end }
function master:GetMap() return map end

dofile(file)
check("registers the command hook", type(onCommand) == "function")
check("says it is loaded in the log", tostring(logged[1]):find('OVSMOKE {"t":"loaded"}', 1, true) ~= nil, logged[1])

local function run(cmd)
    master.lines = {}
    local r = onCommand(42, master, cmd)
    return r, master.lines[#master.lines] or ""
end

local r, line = run("ovsmoke report")
check("report: swallowed", r == false)
check("report: one OVSMOKE line", line:match("^OVSMOKE {") ~= nil, line)
check("report: type", line:find('"t":"report"', 1, true) ~= nil, line)
check("report: the bot, its master and where it is", line:find('"name":"Tankbot"', 1, true) and line:find('"master":"Smk"', 1, true)
    and line:find('"map":36', 1, true) and line:find('"instance":7', 1, true) and line:find('"bot":true', 1, true), line)
check("report: distances", line:find('"dist":3', 1, true) ~= nil, line)
check("report: grouped", line:find('"grouped":true', 1, true) ~= nil, line)
check("report: also in the log", logged[#logged] == line)

r, line = run(".ovsmoke compat GetBotAI,HandleCommand,ChangeStrategy,IsPlayerbot,NoSuchMethod")
check("compat: player and AI methods found", line:find('"ok":["GetBotAI","HandleCommand","ChangeStrategy","IsPlayerbot"]', 1, true) ~= nil, line)
check("compat: the missing one listed", line:find('"missing":["NoSuchMethod"]', 1, true) ~= nil, line)

r, line = run(".ovsmoke pull")
check("pull: default practice mob, timed despawn", spawned and spawned.entry == 38 and spawned.kind == 3 and spawned.ms == 240000)
check("pull: hostile and attacking the player", mob.faction == 14 and mob.target == master)
check("pull: answer", line:find('"name":"Defias Thug"', 1, true) ~= nil, line)
r, line = run(".ovsmoke pull 6")
check("pull: an entry can be given", spawned.entry == 6)

bot.combat = true
mob.hp = 40
r, line = run(".ovsmoke fight")
check("fight: alive, health, who fights", line:find('"alive":true', 1, true) and line:find('"hp":40', 1, true)
    and line:find('"bots_in_combat":["Tankbot"]', 1, true), line)
mob.dead = true
r, line = run(".ovsmoke fight")
check("fight: dead", line:find('"alive":false', 1, true) ~= nil, line)

master.gm = 0
r, line = run(".ovsmoke report")
check("not a GM: swallowed, no answer", r == false and line == "")
master.gm = 3
r = onCommand(42, master, ".recruit party")
check("other commands pass through", r == nil)
r = onCommand(42, nil, "ovsmoke report")
check("console: ignored", r == nil)

r, line = run(".ovsmoke nonsense")
check("unknown sub-command answered as an error", line:find('"t":"error"', 1, true) ~= nil, line)

-- strings with quotes stay valid JSON
bot.name = 'Q"uote'
r, line = run(".ovsmoke report")
check("quotes escaped", line:find('"Q\\u0022uote"', 1, true) ~= nil, line)

realPrint(string.format("%d passed, %d failed", pass, fail))
os.exit(fail == 0 and 0 or 1)
