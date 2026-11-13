-- lua5.1 stream/tests/test_wuui_companions.lua [addon dir] [ov_companions.lua] [ov_modapi.lua]
-- Fake 3.3.5a API: loads every file of WrathUnleashedUI in .toc order, opens the Bots window (/comp) and runs real
-- round trips through ov_companions.lua and ov_modapi.lua (fake mod-ale, fake mod-playerbots): the roster after
-- a party fill, live health bars, the orders (stay, attack, passive, assist, crowd control, formation, prepare),
-- the per-bot menu (spec, role), dismiss, the minimap button, the chat filters (bot achievements, whispers after
-- an order), answers in the window and not the chat, and no game-brand names on screen.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local server = arg[2] or "realm/custom/lua_scripts/ov_companions.lua"
local modapi = arg[3] or server:gsub("ov_companions%.lua$", "ov_modapi.lua")

local pass, fail = 0, 0
math.randomseed(7)   -- the realm picks classes at random
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- fake widgets (as test_wuui_builder.lua)
local frames, created = {}, {}
local Frame = {}
Frame.__index = function(self, k)
    local v = rawget(Frame, k)
    if v ~= nil then return v end
    if type(k) == "string" and k:match("^%u") then return function() end end -- other widget methods: no-ops
    return nil
end
function Frame:SetScript(n, fn) self.scripts[n] = fn end
function Frame:GetScript(n) return self.scripts[n] end
function Frame:Fire(n, ...) local fn = self.scripts[n]; if fn then return fn(self, ...) end end
function Frame:Show()
    local was = self:IsVisible(); self.shown = true
    if not was and self:IsVisible() then self:Fire("OnShow") end
end
function Frame:Hide()
    local was = self:IsVisible(); self.shown = false
    if was then self:Fire("OnHide") end
end
function Frame:IsShown() return self.shown end
function Frame:IsVisible()
    if not self.shown then return false end
    return self.parent == nil or self.parent.top or self.parent:IsVisible()
end
function Frame:SetText(t) self.text = t end
function Frame:GetText() return self.text end
function Frame:Enable() self.enabled = true end
function Frame:Disable() self.enabled = false end
function Frame:RegisterEvent(e) self.events[e] = true end
function Frame:GetName() return self.name end
function Frame:SetChecked(v) self.checked = v end
function Frame:GetChecked() return self.checked end
function Frame:SetValue(v) self.value = v end
function Frame:SetTexCoord(...) self.coords = { ... } end
function Frame:SetTextColor(r, g, b) self.color = { r, g, b } end
function Frame:SetAttribute(k, v) self.attrs = self.attrs or {}; self.attrs[k] = v end
function Frame:GetAttribute(k) return self.attrs and self.attrs[k] end
function Frame:SetPoint(...) self.point = { ... } end
function Frame:GetPoint() if self.point then return unpack(self.point) end end
function Frame:SetTexture(t) self.tex = t; return 1 end
function Frame:GetTexture() return self.tex end
function Frame:SetAlpha(a) self.alpha = a end
function Frame:SetFrameStrata(s) self.strata = s end
function Frame:SetBackdrop(t) self.backdrop = t end
function Frame:SetBackdropBorderColor(r, g, b) self.edgeColour = { r, g, b } end
function Frame:SetStatusBarTexture(t) self.barTex = t end
function Frame:SetNormalTexture(t) self.normalTex = t end
function Frame:LockHighlight() self.locked = true end
function Frame:UnlockHighlight() self.locked = nil end
local function new(kind, name, parent)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, events = {}, shown = true }, Frame)
    if name then _G[name] = f end
    created[#created + 1] = f
    return f
end
function Frame:CreateFontString() return new("FontString", nil, self) end
function Frame:CreateTexture() return new("Texture", nil, self) end
function CreateFrame(kind, name, parent, template)
    local f = new(kind, name, parent); f.template = template; frames[#frames + 1] = f; return f
end
unpack = unpack or table.unpack
local errorsLines = {}
UIErrorsFrame = { AddMessage = function(_, m) errorsLines[#errorsLines + 1] = m end }
local inCombat = false
local bindings = {}
function GetBindingKey(b) return bindings[b] end
local unitCombat = {}
function UnitAffectingCombat(u) return unitCombat[u] or false end
local portraits = {}
function SetPortraitTexture(tex, unit) portraits[#portraits + 1] = unit; tex.portraitOf = unit end
function LFGGetDungeonInfoByID(id)   -- the 3.3.5 client's call (GetLFGDungeonInfo is a later client's)
    if id == 5 then return "Utgarde Keep", 1, 70, 72, 70, 69, 80, 2, 0, "UtgardeKeepClient" end
    return nil
end

UIParent = new("Frame", "UIParent"); UIParent.top = true
Minimap = new("Frame", "Minimap"); Minimap.top = true
GameMenuFrame = new("Frame", "GameMenuFrame"); GameMenuFrame.top = true; GameMenuFrame.h = 300
function Frame:GetHeight() return self.h or 0 end
function Frame:SetHeight(h) self.h = h end
UISpecialFrames = {}
function tinsert(t, v) table.insert(t, v) end
SlashCmdList = {}
local chatLines = {}
DEFAULT_CHAT_FRAME = { AddMessage = function(_, m) chatLines[#chatLines + 1] = m end }
local filters = {}
function ChatFrame_AddMessageEventFilter(event, fn) filters[event] = fn end
RAID_CLASS_COLORS = { MAGE = { r = 0.41, g = 0.8, b = 0.94 }, WARRIOR = { r = 0.78, g = 0.61, b = 0.43 },
    DRUID = { r = 1, g = 0.49, b = 0.04 }, PRIEST = { r = 1, g = 1, b = 1 } }
PowerBarColor = { MANA = { r = 0, g = 0, b = 1 }, RAGE = { r = 1, g = 0, b = 0 } }
function GetScreenHeight() return 981 end
function InCombatLockdown() return inCombat end
function SetBinding() return 1 end
function SetBindingClick() return 1 end
function SaveBindings() end
function SetCVar() end
function HasAction() return false end
function UnitLevel() return 80 end
function UnitClass() return "Mage", "MAGE" end
function PlaySound() end
function HideUIPanel(f) f:Hide() end
local clock = 100
function GetTime() return clock end
local hasTarget = false
function UnitExists(u) return u == "target" and hasTarget end
function GetNumRaidMembers() return 0 end
local partyUnits = {}   -- party1.. -> bot
function UnitName(u)
    if u == "player" then return "Tester" end
    local b = partyUnits[u]
    return b and b.name or nil
end
function UnitHealth(u) local b = partyUnits[u]; return b and b.hp or 0 end
function UnitHealthMax(u) return partyUnits[u] and 1000 or 0 end
function UnitPower(u) local b = partyUnits[u]; return b and b.mp or 0 end
function UnitPowerMax(u) return partyUnits[u] and 500 or 0 end
function UnitPowerType() return 0, "MANA" end
function UnitIsDeadOrGhost(u) local b = partyUnits[u]; return b and b.dead or false end

-- ---------------------------------------------------------------- the realm (ov_companions.lua + ov_modapi.lua, fake mod-ale)
local realGetenv, realTime = os.getenv, os.time
local serverClock = 1000000
os.time = function() return serverClock end
os.getenv = function() return nil end
local onServerAddon
function RegisterServerEvent(id, fn) if id == 30 then onServerAddon = fn end end
local playerEvents = {}
function RegisterPlayerEvent(id, fn) playerEvents[id] = fn end
function CharDBExecute() end
function CharDBQuery() return nil end
local function fireClient(prefix, msg, channel, sender)
    for _, f in ipairs(frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", prefix, msg, channel, sender) end
    end
end

local serverChat = {}
local function unit(o)
    local u = { whispers = {}, queue = {}, cmds = {}, level = 80, combat = false }
    for k, v in pairs(o) do u[k] = v end
    function u:GetGUIDLow() return self.guid end
    function u:GetGUID() return self.guid end
    function u:GetName() return self.name end
    function u:GetLevel() return self.level end
    function u:GetClass() return self.class end
    function u:GetAccountId() return self.account end
    function u:GetAccountName() return self.accName end
    function u:IsInCombat() return self.combat end
    function u:IsDead() return self.dead or false end
    function u:InBattleground() return false end
    function u:InArena() return false end
    function u:GetMap() return { IsDungeon = function() return false end, IsRaid = function() return false end } end
    function u:GetMapId() return 0 end
    function u:GetInstanceId() return 0 end
    function u:GetX() return 0 end
    function u:GetY() return 0 end
    function u:GetZ() return 0 end
    function u:GetO() return 0 end
    function u:GetDistance() return 5 end
    function u:GetGroup() return self.group end
    function u:Teleport() end
    function u:GetSelection() return hasTarget and {} or nil end
    function u:IsPlayerbot() return self.bot == true end
    function u:IsAddclassBot() return self.bot == true end
    function u:IsTank(bySpec) return false end
    function u:IsHealer(bySpec) return false end
    function u:RunCommand(c) self.cmds[#self.cmds + 1] = c end
    function u:RegisterEvent(fn, delay, rep) self.queue[#self.queue + 1] = { fn = fn, delay = delay, rep = rep } end
    function u:SendBroadcastMessage(m) serverChat[#serverChat + 1] = m end
    function u:SendAddonMessage(prefix, msg) fireClient(prefix, msg, "WHISPER", "Tester") end
    function u:GetEquippedItemBySlot(slot)
        if not self.bot then return nil end
        return { GetItemLevel = function() return 187 end, GetEntry = function() return 40000 + slot end,
            GetQuality = function() return 3 end, GetName = function() return "Worn Thing " .. slot end }
    end
    -- bags (the gear window's upgrades): self.bags["<bag>:<slot>"]
    function u:GetItemByPos(bag, slot) return self.bags and self.bags[bag .. ":" .. slot] end
    function u:GetBotAI()
        if not self.bot then return nil end
        local me = self
        return {
            HandleCommand = function(_, kind, text, from)
                from.whispers[#from.whispers + 1] = { to = me, text = text }
                if text == "leave" and me.group then
                    for i, m in ipairs(me.group.members) do if m == me then table.remove(me.group.members, i) end end
                    me.group = nil
                end
            end,
            ChangeStrategy = function(_, change, state) me.strategies = (me.strategies or "") .. state .. change .. ";" end,
            GetMaster = function() return me.master end,
        }
    end
    return u
end

local player = unit({ guid = 7, name = "Tester", class = 8, account = 1, accName = "TESTER" })
package.preload["ov_modapi"] = function() return dofile(modapi) end
package.path = (server:match("^(.*/)[^/]+$") or "./") .. "?.lua;" .. package.path   -- ov_companions_dungeons.lua
dofile(server)
local T = assert(OV_COMPANIONS_TEST)
check("the realm script has the window channel", T.window == true and onServerAddon ~= nil)

local sent = {}
function SendAddonMessage(prefix, msg, channel, target)
    sent[#sent + 1] = { prefix, msg, channel, target }
    serverClock = serverClock + 1
    if target == UnitName("player") and onServerAddon then onServerAddon(30, player, 7, prefix, msg, player) end
end

-- what mod-playerbots does for each addclass: a bot joins the group
local CLS = { warrior = 1, paladin = 2, hunter = 3, rogue = 4, priest = 5, dk = 6, shaman = 7, mage = 8, warlock = 9, druid = 11 }
local nextBot, served = 100, {}
local function serve()
    local made = {}
    for i, c in ipairs(player.cmds) do
        local cls = c:match("^playerbots bot addclass (%a+)$")
        if cls and not served[i] then
            served[i] = true
            nextBot = nextBot + 1
            local b = unit({ guid = nextBot, name = "Bot" .. string.char(64 + nextBot - 100), class = CLS[cls], account = 900,
                accName = "RNDBOT" .. nextBot, bot = true, master = player, hp = 1000, mp = 500 })
            if not player.group then
                player.group = { members = { player } }
                local g = player.group
                function g:GetMembers() return self.members end
                function g:GetMembersCount() return #self.members end
                function g:IsLeader(guid) return guid == player.guid end
                function g:IsRaidGroup() return false end
                function g:IsLFGGroup() return false end
                function g:IsBGGroup() return false end
            end
            table.insert(player.group.members, b); b.group = player.group
            made[#made + 1] = b
        end
    end
    for i, b in ipairs(made) do partyUnits["party" .. i] = b end
    return made
end
local function runServer()
    while #player.queue > 0 do
        local e = table.remove(player.queue, 1)
        for k = 1, e.rep do e.fn(k, e.delay, e.rep - k, player) end
    end
end

-- ---------------------------------------------------------------- load the addon in .toc order
local files = {}
for line in io.lines(dir .. "/WrathUnleashedUI.toc") do
    if line:match("%.lua%s*$") then files[#files + 1] = line:gsub("%s+$", "") end
end
local at = {}
for i, f in ipairs(files) do at[f] = i end
check(".toc lists Companions.lua (before Leyline.lua, which stays last)", at["Companions.lua"] and at["Companions.lua"] < at["Leyline.lua"])
for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
os.getenv = realGetenv
local W = assert(WrathUnleashedUI)
local C = assert(W.Companions, "W.Companions")
check("builds nothing until opened", C.Window() == nil and not _G.WrathUnleashedBotsMinimapButton)
check("bot chatter hidden by default", C.HideChatter() == true)
check(".toc: the dungeon list, the tabs and the bars after Companions.lua", at["CompanionsData.lua"] < at["Companions.lua"]
    and at["CompanionsTabs.lua"] > at["Companions.lua"] and at["BotBar.lua"] > at["Companions.lua"] and at["BotBar.lua"] < at["Leyline.lua"])
check("/companions and /comp", SLASH_WUCOMPANIONS1 == "/companions" and SLASH_WUCOMPANIONS2 == "/comp" and SlashCmdList.WUCOMPANIONS)
check("chat filters for bot achievements and whispers", filters.CHAT_MSG_ACHIEVEMENT and filters.CHAT_MSG_GUILD_ACHIEVEMENT
    and filters.CHAT_MSG_WHISPER and filters.CHAT_MSG_WHISPER_INFORM)
check("the bot module's 'configured with N bots' login line is hidden; its credit line stays", filters.CHAT_MSG_SYSTEM
    and filters.CHAT_MSG_SYSTEM(nil, "CHAT_MSG_SYSTEM", "|cff00ff00Playerbots:|r The server is configured with 0 bots.") == true
    and filters.CHAT_MSG_SYSTEM(nil, "CHAT_MSG_SYSTEM", "|cff00ff00This server runs with |cff00ccffmod-playerbots|r |cffcccccchttps://github.com/mod-playerbots/mod-playerbots|r") == false
    and filters.CHAT_MSG_SYSTEM(nil, "CHAT_MSG_SYSTEM", "You have learned a new spell.") == false
    and filters.CHAT_MSG_SYSTEM(nil, "CHAT_MSG_SYSTEM", "This server is running the |cff4CFF00AutoBalance|r module.") == true)

-- entering the world: a roster ask a moment later; the realm has bots, so the minimap button shows
C.listener:Fire("OnEvent", "PLAYER_ENTERING_WORLD")
check("no ask at once", #sent == 0)
clock = clock + 4
C.listener:Fire("OnUpdate", 0.1)
check("then the roster is asked over OVMOD, whispered to self", sent[1] and sent[1][1] == "OVMOD" and sent[1][2] == "wucomp\troster\t1/1\t"
    and sent[1][3] == "WHISPER" and sent[1][4] == "Tester")
check("the realm answered: bots on, none with you", C.state == "on" and #C.bots == 0 and C.cap == 4)
local mini = _G.WrathUnleashedBotsMinimapButton
check("minimap button shown once the realm has bots", mini and mini:IsShown() and mini.parent == Minimap)

-- open it
SlashCmdList.WUCOMPANIONS("")
local win = C.Window()
check("/comp opens the window; Esc closes it", win and win:IsShown() and UISpecialFrames[#UISpecialFrames] == "WrathUnleashedBotsFrame")
check("empty: says so and offers the group buttons", win.empty:GetText():find("No bots with you") and win.sub:GetText() == "0 of 4 bots in your party")
check("orders wait for bots", win.prepare.enabled == false and win.passive.enabled == false)

-- a widget with that text anywhere inside parent (the window's tabs keep their widgets on pages)
local function inside(f, parent)
    local p = f.parent
    while p do if p == parent then return true end; p = p.parent end
    return false
end
local function find(parent, text)
    for _, f in ipairs(created) do if inside(f, parent) and f.text == text then return f end end
end
find(win, "Fill party"):Fire("OnClick")
check("Fill party: the realm recruits 4", #player.cmds == 4 and win.status:GetText():find("Recruiting 4 bots"))
check("the answer is in the window, not the chat", #serverChat == 0)
local bots = serve()
runServer()
check("they join: 4 rows, tank first", #C.bots == 4 and win.rows[1]:IsShown() and win.rows[4]:IsShown()
    and not win.rows[5]:IsShown() and C.bots[1].role == "tank")
check("every row says bot, its class colour, spec, item level and level", (function()
    for i = 1, 4 do
        local r = win.rows[i]
        if not (r.name:GetText():find("bot|r") and r.name:GetText():find("^|cff%x%x%x%x%x%x")
            and r.info:GetText():find("item level 187, level 80")) then return false end
    end
    return true end)())
check("role icons from the role sheet", win.rows[1].role.coords and win.rows[1].role.coords[1] == 0)
check("joined: shown in the window, not the chat", win.status:GetText():find("joins as a") and #serverChat == 0)
check("orders on now", win.prepare.enabled == true)

-- live bars
bots[1].hp = 500; bots[1].mp = 250
C.UpdateBars()
local r1
for _, r in ipairs(win.rows) do if r.bot and r.bot.name == bots[1].name then r1 = r end end
check("health and mana from the unit functions", r1.hp.value == 0.5 and r1.mp.value == 0.5)
bots[1].dead = true
C.UpdateBars()
check("dead shows Dead", r1.state:GetText() == "Dead")
bots[1].dead = false

-- bot chatter
local quietAch = filters.CHAT_MSG_ACHIEVEMENT(nil, "CHAT_MSG_ACHIEVEMENT", "%s has earned the achievement [Big Love Rocket]!", bots[2].name)
check("a bot's achievement line is hidden", quietAch == true)
check("someone else's achievement stays", filters.CHAT_MSG_ACHIEVEMENT(nil, "CHAT_MSG_ACHIEVEMENT", "x", "Friend") == false)
check("a bot's whisper right after it joined is hidden", filters.CHAT_MSG_WHISPER(nil, "CHAT_MSG_WHISPER", "Equipping gear", bots[2].name) == true)
clock = clock + 60
check("later, a bot's whisper shows (you asked it something)", filters.CHAT_MSG_WHISPER(nil, "CHAT_MSG_WHISPER", "Hi", bots[2].name) == false)
check("a friend's whisper always shows", filters.CHAT_MSG_WHISPER(nil, "CHAT_MSG_WHISPER", "Hi", "Friend") == false)

-- orders
local before = #player.whispers
find(win, "Stay"):Fire("OnClick")
check("Stay: every bot told stay through its AI", #player.whispers == before + 4 and player.whispers[#player.whispers].text == "stay")
check("rows say Staying", win.rows[2].state:GetText() == "Staying" and win.status:GetText() == "Staying here.")
check("whispers right after an order are hidden", filters.CHAT_MSG_WHISPER(nil, "CHAT_MSG_WHISPER", "Staying", bots[3].name) == true)
local sends = #sent
find(win, "Attack my target"):Fire("OnClick")
check("Attack without a target: asks for one, sends nothing", #sent == sends and win.status:GetText() == "Target something first.")
hasTarget = true
find(win, "Attack my target"):Fire("OnClick")
check("Attack my target", player.whispers[#player.whispers].text == "attack")
win.passive:Fire("OnClick")
check("Passive: silent strategy change; the button turns into Fight", bots[1].strategies:find("0%+passive") and win.passive:GetText() == "Fight")
win.passive:Fire("OnClick")
check("Fight: -passive", bots[1].strategies:find("0%-passive") and win.passive:GetText() == "Passive")
win.assist:Fire("OnClick")
check("Assist tank: on, shown green", C.assist and win.assist:GetText():find("^|cff7fff7f"))
win.cc:Fire("OnClick")
check("Crowd control: on", C.cc and win.cc:GetText():find("^|cff7fff7f"))
win.formation:Fire("OnClick")
local pick = _G.WrathUnleashedBotFormations
check("formation picker opens", pick and pick:IsShown())
find(pick, "Arrow"):Fire("OnClick")
check("Arrow: told, picker closed, button says so", player.whispers[#player.whispers].text == "formation arrow" and not pick:IsShown()
    and win.formation:GetText() == "Formation: Arrow")
win.prepare:Fire("OnClick")
check("Prepare for raid", win.status:GetText():find("Getting ready"))

-- the per-bot menu
local mage
for _, r in ipairs(win.rows) do if r.bot and r.bot.cls == 8 then mage = r end end
if not mage then for _, r in ipairs(win.rows) do if r.bot and r.bot.role == "dps" then mage = r end end end
mage:Fire("OnClick", "LeftButton")
local menu = _G.WrathUnleashedBotMenu
check("click a bot: its menu", menu and menu:IsShown() and menu.title:GetText():find(mage.bot.name))
local specBtn
for _, b in ipairs(menu.specBtns) do if b:IsShown() and b.enabled and b.spec then specBtn = b end end
check("specs from the realm's list, the current one not clickable", specBtn ~= nil)
local spec = specBtn.spec
local mageName = mage.bot.name
specBtn:Fire("OnClick")
check("pick a spec: the realm teaches it, the roster shows it", player.whispers[#player.whispers - 2].text == "talents spec " .. spec
    and not menu:IsShown() and C.Find(mageName).spec == spec)
for _, r in ipairs(win.rows) do if r.bot and r.bot.name == mageName then mage = r end end
local roles = C.RolesFor(mage.bot.cls)
mage:Fire("OnClick", "LeftButton")
check("role buttons only for roles the class can take", (function()
    for _, rb in ipairs(menu.roleBtns) do
        local can = roles[rb.role] ~= nil and rb.role ~= mage.bot.role
        if (rb.enabled == true) ~= can then return false end
    end
    return true end)())
menu:Hide()

-- ---------------------------------------------------------------- the bar of your bots
local Bar = C.Bar
local bf = Bar.Frame()
check("bot bar: built and shown once bots are with you", bf and bf:IsShown() and bf:GetName() == "WrathUnleashedBotBar")
local s1 = bf.slots[1]
check("bot bar: one secure slot per bot, the client's own target on left-click", s1.template == "SecureUnitButtonTemplate"
    and s1:GetAttribute("type1") == "target" and s1:GetAttribute("unit") == "party1" and bf.slots[4]:IsShown()
    and not bf.slots[5]:IsShown())
check("bot bar: portrait, name in its class colour, role icon", s1.portrait.portraitOf == "party1"
    and s1.name:GetText():find("^|cff%x%x%x%x%x%x" .. s1.botName) and s1.role.coords ~= nil)
check("bot bar: says how many", bf.count:GetText() == "4 bots")
check("bot bar and orders bar: the unit frames' layer (LOW), under the game's windows", bf.strata == "LOW"
    and Bar.Orders() and Bar.Orders().strata == "LOW")
Bar.Update(false)
check("bot bar: health and mana bars", s1.hp.value ~= nil and s1.mp.value ~= nil)
local function slotOf(bot) for _, s in ipairs(bf.slots) do if s.botName == bot.name then return s end end end
local sa = slotOf(bots[3])
bots[3].hp = 1000; bots[3].mp = 500; hasTarget = false
C.Order("follow", "all"); Bar.Update(false)
check("activity: following", sa.activity:GetText() == "Following")
bots[3].mp = 100; Bar.Update(false)
check("activity: resting (low on mana out of a fight)", sa.activity:GetText() == "Resting")
bots[3].mp = 500
unitCombat[sa.unit] = true; Bar.Update(false)
check("activity: fighting", sa.activity:GetText() == "Fighting")
unitCombat[sa.unit] = nil
bots[3].dead = true; Bar.Update(false)
check("activity: dead", sa.activity:GetText() == "Dead")
bots[3].dead = false
sa:Fire("PostClick", "RightButton")
local bmenu = _G.WrathUnleashedBotMenu
check("bot bar: right-click opens that bot's menu (orders, spec, role, gear)", bmenu:IsShown() and bmenu.botName == sa.botName
    and bmenu.attack and bmenu.passive and bmenu.gear and bmenu.dismiss)
local w0 = #player.whispers
bmenu.passive:Fire("OnClick")
check("menu: passive for that bot only", bots[1].strategies and #player.whispers == w0 and not bmenu:IsShown()
    and C.Find(sa.botName).status == "passive")
sa:Fire("PostClick", "RightButton")
check("menu: then it offers Fight", bmenu.passive:GetText() == "Fight")
bmenu.passive:Fire("OnClick")
sa:Fire("PostClick", "LeftButton")
check("bot bar: left-click doesn't open the menu", not bmenu:IsShown())
-- moving it, and keeping where it is
bf.head:Fire("OnDragStart")
bf.point = { "TOPLEFT", UIParent, "TOPLEFT", 300.4, -200.2 }
bf.head:Fire("OnDragStop")
check("bot bar: dragged, the spot is kept", WrathUnleashedUIDB.botBarPos and WrathUnleashedUIDB.botBarPos[1] == "TOPLEFT"
    and WrathUnleashedUIDB.botBarPos[3] == 300 and WrathUnleashedUIDB.botBarPos[4] == -200)
-- in combat, slots wait for the end of the fight
inCombat = true
local keepUnit = s1:GetAttribute("unit")
partyUnits.party1, partyUnits.party2 = partyUnits.party2, partyUnits.party1
check("in combat: no layout (secure frames)", Bar.Layout() == false and Bar.dirty == true and s1:GetAttribute("unit") == keepUnit)
inCombat = false
Bar.events:Fire("OnEvent", "PLAYER_REGEN_ENABLED")
check("after the fight: laid out again", Bar.dirty == nil and slotOf(partyUnits.party1).unit == "party1")
partyUnits.party1, partyUnits.party2 = partyUnits.party2, partyUnits.party1
Bar.Layout()

-- ---------------------------------------------------------------- the orders bar
local ob = Bar.Orders()
check("orders bar: shown with bots, seven orders with the game's own icons", ob and ob:IsShown() and #ob.btns == 7
    and ob.btns.follow and ob.btns.stay and ob.btns.attack and ob.btns.passive and ob.btns.pull and ob.btns.regroup and ob.btns.flee
    and ob.btns.stay.icon:GetTexture():find("^Interface\\Icons\\"))
check("key bindings: a header and a name for each order", BINDING_HEADER_WUBOTS == "Bots" and BINDING_NAME_WUBOTS_FOLLOW
    and BINDING_NAME_WUBOTS_PULL and BINDING_NAME_WUBOTS_WINDOW)
local bx = assert(io.open(dir .. "/Bindings.xml")):read("*a")
check("Bindings.xml: each binding calls C.Key", bx:find('name="WUBOTS_FOLLOW" header="WUBOTS"', 1, true)
    and bx:find('WrathUnleashedUI.Companions.Key("pull")', 1, true) and bx:find('WrathUnleashedUI.Companions.Key("window")', 1, true))
bindings.WUBOTS_FOLLOW = "ALT-1"
Bar.events:Fire("OnEvent", "UPDATE_BINDINGS")
check("orders bar: a set key shows on its button", ob.btns.follow.hotkey:GetText() == "A-1" and ob.btns.stay.hotkey:GetText() == "")
win:Hide()
local chat0 = #chatLines
ob.btns.stay:Fire("OnClick")
check("orders bar: Stay for everyone; the answer on screen, not in the chat", player.whispers[#player.whispers].text == "stay"
    and errorsLines[#errorsLines] == "Staying here." and #chatLines == chat0)
hasTarget = false
ob.btns.attack:Fire("OnClick")
check("orders bar: Attack without a target says so", errorsLines[#errorsLines] == "Target something first.")
hasTarget = true
C.Key("attack")
check("key: attack my target", player.whispers[#player.whispers].text == "attack")
C.Key("passive")
check("key: passive, then the button says Fight", bots[1].strategies:find("passive") and ob.btns.passive.label == "Fight"
    and ob.btns.passive.on:IsShown())
C.Key("passive")
check("key: again, they fight", ob.btns.passive.label == "Passive")
local w1 = #player.whispers
C.Key("pull")
check("key: pull (the tank opens)", (function()
    local tankName = C.bots[1].role == "tank" and C.bots[1].name
    for i = w1 + 1, #player.whispers do
        if player.whispers[i].text == "attack" and player.whispers[i].to.name ~= tankName then return false end
    end
    return #player.whispers == w1 + 1 and errorsLines[#errorsLines] == "Pulling your target." end)())
C.Key("window")
check("key: the window", win:IsShown())

-- ---------------------------------------------------------------- first-time tips
local tipsF = Bar.Tips()
check("tips: shown the first time bots are with you, at the bar", tipsF and tipsF:IsShown() and tipsF.step == 1
    and tipsF.head:GetText() == "Tip 1 of 3" and WrathUnleashedUIDB.botsTipsSeen == true)
tipsF.next:Fire("OnClick")
check("tips: 2 of 3, the orders bar", tipsF.step == 2 and tipsF.title:GetText() == "Orders for everyone")
tipsF.next:Fire("OnClick")
check("tips: 3 of 3 ends with Got it", tipsF.step == 3 and tipsF.next:GetText() == "Got it")
tipsF.next:Fire("OnClick")
check("tips: done", not tipsF:IsShown())
C.Ask()
check("tips: never again", not tipsF:IsShown() and Bar.MaybeTips() == false)
SlashCmdList.WUCOMPANIONS("tips")
check("/comp tips shows them again", tipsF:IsShown() and tipsF.step == 1)
tipsF:Hide()

-- ---------------------------------------------------------------- the tabs
check("tabs: Your bots, Party, Raid, Your alts, Buddy, Dungeons, Tactics", (function()
    local want = { "bots", "party", "raid", "alts", "buddy", "dungeons", "tactics" }
    for i, k in ipairs(want) do if not (win.tabs[i] and win.tabs[i].key == k) then return false end end
    return #win.tabs == 7 end)())
local party = C.ShowTab("party")
check("Party tab: shown alone, the roster page hidden", party:IsShown() and not win.page:IsShown() and C.Tab() == "party")
check("Party tab: 4 slots (tank, healer, damage, damage), every slot Any", #party.slots == 4 and party.slots[1].slotRole == "tank"
    and party.slots[4].slotRole == "dps" and party.slots[1].cls:GetText() == "Class: Any" and party.slots[1].spec.enabled == false)
party.slots[1].cls:Fire("OnClick")
local ch = C.Chooser()
check("class chooser: Any and the classes that can tank", ch:IsShown() and find(ch, "Any")
    and find(ch, "|cffff7d0aDruid|r") and not find(ch, "|cff69ccf0Mage|r"))
find(ch, "|cffff7d0aDruid|r"):Fire("OnClick")
check("picked a druid: saved per character, the spec opens", C.GetPicks()[1].word == "druid" and WrathUnleashedUIChar.botPicks
    and party.slots[1].spec.enabled == true and not ch:IsShown())
party.slots[1].spec:Fire("OnClick")
check("spec chooser: only the druid's tank specs", find(ch, "Feral (bear)") and not find(ch, "Restoration"))
find(ch, "Feral (bear)"):Fire("OnClick")
check("picks string for the realm", C.Picks() == "tank=druid:bear pve,healer=any,dps=any,dps=any")
serverClock = serverClock + 31
party.fill:Fire("OnClick")
check("Fill my party sends the picks", sent[#sent][2] == "wucomp\tfill\t1/1\tparty\ttank=druid:bear pve,healer=any,dps=any,dps=any"
    and win.status:GetText():find("most allowed"))
party.reset:Fire("OnClick")
check("Every slot Any", C.Picks() == "" and party.slots[1].cls:GetText() == "Class: Any")

local raid = C.ShowTab("raid")
check("Raid tab: the shape lights up what you have", raid.shape:GetText():find("With you now: 4 bots %(1 tank, 1 healer, 2 damage%)")
    and raid.icons[1].icon.alpha == 1 and raid.icons[2].icon.alpha == 0.3 and raid.fill.enabled == true)
raid.fill:Fire("OnClick")
check("Fill raid", sent[#sent][2] == "wucomp\tfill\t1/1\traid")

local nsent = #sent
local alts = C.ShowTab("alts")
check("Your alts tab: asks the realm for your alts", sent[nsent + 1] and sent[nsent + 1][2] == "wucomp\talts\t1/1\t")
check("no alts: says so", alts.empty:GetText():find("No other characters") and alts.bringUsual.enabled == false)
C.Dispatch("alts", "A\tAlto\t60\t11\t0\t0\nA\tBassa\t0\t7\t1\t1\nU\tAlto")
check("alts listed: class colour, level, who is with you", alts.rows[1]:IsShown() and alts.rows[1].name:GetText():find("Alto")
    and alts.rows[1].info:GetText() == "Level 60 Druid" and alts.rows[2].info:GetText() == "with you as a bot"
    and alts.rows[2].bring.enabled == false and not alts.rows[3]:IsShown())
check("the usual party shown", alts.usual:GetText() == "Your usual party: Alto." and alts.bringUsual.enabled == true)
alts.rows[1].bring:Fire("OnClick")
check("Bring an alt", sent[#sent][2] == "wucomp\talt\t1/1\tAlto")
alts.bringUsual:Fire("OnClick")
check("Bring my usual party", sent[#sent][2] == "wucomp\tusual\t1/1\t")

local bud = C.ShowTab("buddy")
check("Buddy tab: none with you", bud.card.name:GetText() == "No buddy with you" and bud.toggle:GetText() == "Bring a buddy")
bud.cls:Fire("OnClick")
find(C.Chooser(), "|cff69ccf0Mage|r"):Fire("OnClick")
check("buddy class picked", WrathUnleashedUIChar.buddyClass == "mage" and bud.cls:GetText():find("Mage"))
serverClock = serverClock + 31
bud.toggle:Fire("OnClick")
check("Bring a buddy asks for that class; the realm's answer shows", sent[#sent][2] == "wucomp\tbuddy\t1/1\tmage"
    and win.status:GetText():find("most allowed"))

local dg = C.ShowTab("dungeons")
check("Dungeons tab: the ones for your level (80)", dg.rows[1]:IsShown() and (function()
    for _, r in ipairs(dg.rows) do
        if r:IsShown() and not r.info:GetText():find("for your level") then return false end
    end
    return true end)())
local uk
for _, r in ipairs(dg.rows) do if r:IsShown() and r.name:GetText() == "Utgarde Keep" then uk = r end end
check("the game's own Dungeon Finder art: the client's name when it knows the dungeon", uk
    and uk.icon:GetTexture() == "Interface\\LFGFrame\\LFGIcon-UtgardeKeepClient")
check("else the list's own name", (function()
    for _, r in ipairs(dg.rows) do
        if r:IsShown() and r ~= uk and not r.icon:GetTexture():find("^Interface\\LFGFrame\\LFGIcon%-%a+$") then return false end
    end
    return true end)())
check("more than fit: scroll", dg.more:GetText():find("of %d+ %(scroll%)"))
local n80 = #C.DungeonList(false)
dg.all:SetChecked(true); dg.all:Fire("OnClick")
check("Every dungeon: all of them", #C.DungeonList(true) == #W.CompanionsDungeons and #W.CompanionsDungeons > n80)
dg:Fire("OnMouseWheel", -1)
check("scrolls", C.dungeonOffset == 1)
dg.all:SetChecked(false); dg.all:Fire("OnClick")
uk.go:Fire("OnClick")
check("Go: the realm fills the party and takes you to the door", sent[#sent][2] == "wucomp\tdungeon\t1/1\tutgarde-keep"
    and win.status:GetText():find("To Utgarde Keep: your bots meet you at the door"))
C.ShowTab("bots")
check("back to Your bots", win.page:IsShown() and not dg:IsShown())

-- answers while the window is closed land in the chat frame, tagged
win:Hide()
C.Order("follow", "all")
check("closed window: the realm's answer goes to the chat frame", chatLines[#chatLines] == "|cff66ccff[Bots]|r Following you.")
win:Show()

-- the switch
win.quiet:SetChecked(false); win.quiet:Fire("OnClick")
check("Hide bot chatter off: achievements show", filters.CHAT_MSG_ACHIEVEMENT(nil, "CHAT_MSG_ACHIEVEMENT", "x", bots[2].name) == false
    and WrathUnleashedUIDB.botsChatter == true)
win.quiet:SetChecked(true); win.quiet:Fire("OnClick")
check("on again", filters.CHAT_MSG_ACHIEVEMENT(nil, "CHAT_MSG_ACHIEVEMENT", "x", bots[2].name) == true)

-- ---------------------------------------------------------------- more: rows, orders, tactics, gear, ready, saved parties
do
    -- every button on Your bots says what it does
    check("Your bots: every order has a tooltip", (function()
        for _, b in ipairs(win.needBots) do if not (b.tipTitle and b.tipText and b:GetScript("OnEnter")) then return false end end
        return win.tactics.tipText ~= nil and win.ready.tipText ~= nil and #win.needBots >= 11 end)())
    -- rows: the bot's face, and a word when it needs you
    local rr
    for _, r in ipairs(win.rows) do if r.bot and r.bot.name == bots[2].name then rr = r end end
    C.Refresh()
    check("rows: each bot's portrait (SetPortraitTexture on its party unit)", rr.portrait.portraitOf ~= nil
        and rr.portrait.portraitOf:find("^party%d$"))
    bots[2].hp = 200
    C.UpdateBars()
    check("rows: a hurt bot says so (its order word stays)", rr.mood:GetText() == "Hurt" and rr.state:GetText() ~= "Hurt")
    bots[2].hp = 1000; bots[2].mp = 50
    C.UpdateBars()
    check("rows: low on mana", rr.mood:GetText() == "Low on mana")
    bots[2].mp = 500
    function UnitInRange(u) return u ~= rr.portrait.portraitOf and 1 or nil end
    function CheckInteractDistance() return false end
    C.UpdateBars()
    check("rows: far away says Regroup", rr.mood:GetText() == "Far away: Regroup")
    UnitInRange, CheckInteractDistance = nil, nil
    C.UpdateBars()
    check("rows: nothing to say when all is well", rr.mood:GetText() == "")

    -- Regroup, Flee and Drink from the window, through the realm
    serverClock = serverClock + 5
    win.regroup:Fire("OnClick")
    check("Regroup: the realm tells every bot to follow", player.whispers[#player.whispers].text == "follow"
        and win.status:GetText() == "Regrouping on you.")
    win.flee:Fire("OnClick")
    check("Flee: mod-playerbots' flee; they turn passive (the button says Fight)", player.whispers[#player.whispers].text == "flee"
        and win.passive:GetText() == "Fight")
    win.passive:Fire("OnClick")
    win.drink:Fire("OnClick")
    check("Drink", player.whispers[#player.whispers].text == "drink" and win.status:GetText():find("drink up"))

    -- the orders bar's new orders and keys
    local ob = Bar.Orders()
    ob.btns.regroup:Fire("OnClick")
    check("orders bar: Regroup", sent[#sent][2] == "wucomp\torder\t1/1\tregroup all")
    C.Key("flee")
    check("key: Flee", sent[#sent][2] == "wucomp\torder\t1/1\tflee all")
    C.Key("passive")
    C.Key("drink")
    check("key: Drink", sent[#sent][2] == "wucomp\torder\t1/1\tdrink all")
    local bx = assert(io.open(dir .. "/Bindings.xml")):read("*a")
    check("Bindings.xml and names for the new keys", (function()
        for _, k in ipairs({ "REGROUP", "FLEE", "DRINK", "READY", "GEAR", "TACTICS" }) do
            if not (bx:find('name="WUBOTS_' .. k .. '"', 1, true) and _G["BINDING_NAME_WUBOTS_" .. k]) then return false end
        end
        return true end)())

    -- Tactics tab
    local tt = C.ShowTab("tactics")
    check("Tactics tab: the switches, each in the game's option box with a check box and a line", #tt.toggles == 7
        and tt.toggles.waitpull.check.template == "UICheckButtonTemplate" and tt.toggles.waitpull.backdrop
        and tt.toggles.healtank.text:GetText():find("tank alive first"))
    tt.toggles.waitpull.check:SetChecked(true); tt.toggles.waitpull.check:Fire("OnClick")
    check("Tank pulls first: on for every bot, ticked and gold-edged from the realm's roster", sent[#sent][2] == "wucomp\torder\t1/1\twaitpull all"
        and C.waitPull == true and tt.toggles.waitpull.check:GetChecked() == true and tt.toggles.waitpull.gameOn == true)
    tt.toggles.healtank.check:SetChecked(true); tt.toggles.healtank.check:Fire("OnClick")
    check("Heal the tank first: the healer bot told to focus the tank", C.healTank == true and (function()
        for _, w in ipairs(player.whispers) do if w.text:find("^focus heal ") then return true end end
        return false end)())
    tt.toggles.aoe.check:SetChecked(true); tt.toggles.aoe.check:Fire("OnClick")
    tt.toggles.aoe.check:SetChecked(false); tt.toggles.aoe.check:Fire("OnClick")
    check("Spread damage on, then off", sent[#sent][2] == "wucomp\torder\t1/1\tnoaoe all" and C.aoe == false
        and tt.toggles.aoe.check:GetChecked() == false)
    tt.toggles.waitpull.check:SetChecked(false); tt.toggles.waitpull.check:Fire("OnClick")
    check("Tank pulls first off", C.waitPull == false)
    check("Tactics: each formation explained, the current one picked", #tt.formations == 8 and tt.formations[1].tipText ~= nil)
    tt.formations[8]:Fire("OnClick")
    check("Tactics: Single file", player.whispers[#player.whispers].text == "formation queue" and tt.formations[8].gameSelected == true)
    check("Tactics: loot explained", find(tt, "Loot") ~= nil)

    -- the gear window: what it wears, upgrades in its bags, Equip
    local gb = bots[3]
    gb.bags = { ["255:23"] = { GetEntry = function() return 50001 end, GetItemLevel = function() return 200 end,
        GetInventoryType = function() return 1 end, GetQuality = function() return 4 end, GetName = function() return "Shiny Helm" end } }
    function GetItemIcon(id) return "Interface\\Icons\\Item" .. id end
    ITEM_QUALITY_COLORS = { [3] = { hex = "|cff0070dd" }, [4] = { hex = "|cffa335ee" } }
    local gw = C.OpenGear(gb.name)
    check("gear window: asks the realm, shows its answer", sent[#sent][2] == "wucomp\tinspect\t1/1\t" .. gb.name and gw:IsShown()
        and C.gear and C.gear.name == gb.name and gw.title:GetText() == gb.name .. "'s gear")
    check("gear window: 17 slots, each item's icon, quality colour, item level", #gw.slots == 17
        and gw.slots[1].icon:GetTexture() == "Interface\\Icons\\Item40000" and gw.slots[1].name:GetText() == "|cff0070ddWorn Thing 0|r"
        and gw.slots[1].lvl:GetText() == "Head, item level 187" and gw.sub:GetText():find("item level 187"))
    check("gear window: the upgrade in its bags, with Equip", gw.ups[1]:IsShown() and gw.ups[1].name:GetText():find("Shiny Helm")
        and gw.ups[1].lvl:GetText() == "Head: item level 200, now 187 (+13)" and not gw.ups[2]:IsShown() and gw.all.enabled == true)
    gw.ups[1].equip:Fire("OnClick")
    check("Equip: the bot is told to wear it; the answer in the gear window", player.whispers[#player.whispers].text == "e |Hitem:50001:0|h"
        and gw.status:GetText():find("puts on Shiny Helm"))
    gb.bags = nil
    gw.refresh:Fire("OnClick")
    check("Refresh: nothing better now", not gw.ups[1]:IsShown() and gw.noUps:GetText():find("Nothing better") and gw.all.enabled == false)
    check("gear window: the game's dialog, slot rings, panel buttons", gw.backdrop and gw.backdrop.edgeFile == "Interface\\DialogFrame\\UI-DialogBox-Border"
        and gw.slots[1].normalTex == "Interface\\Buttons\\UI-Quickslot2" and gw.trade.template == "UIPanelButtonTemplate"
        and UISpecialFrames[#UISpecialFrames] == "WrathUnleashedBotGear")
    gw:Hide()
    C.Dispatch("gear", "G\tNobody\t1\t80\t1\tbot\nI\t0\t1\t1\t1\tx")
    check("gear: an answer for a bot not in the window changes nothing on screen", not gw:IsShown())
    check("gear: odd lines ignored", C.ParseGear("I\t0\t1") == false and C.ParseGear("G\t\t1") == false)
    check("bot menu: Gear and upgrades", (function()
        for _, it in ipairs(C.MenuItems(C.bots[1], 1)) do if it.text == "Gear and upgrades" then return true end end
        return false end)())
    C.Slash = C.Slash
    SlashCmdList.WUCOMPANIONS("gear " .. bots[2].name:lower())
    check("/comp gear <name>", gw:IsShown() and gw.botName == bots[2].name)
    gw:Hide()

    -- Ready?
    local function checks() local out = {}; for _, c in ipairs(C.ReadyChecks()) do out[c.key] = c end; return out end
    C.preparedAt = nil   -- Prepare for raid was pressed earlier in this test
    local ck = checks()
    check("Ready?: you (damage) and your bots make tank, healer and damage", ck.roles.ok == true and ck.roles.detail:find("you: damage"))
    check("Ready?: not prepared until you press Prepare", ck.prepared.ok == false)
    check("Ready?: what the client can't tell waits (no repair or bag functions here)", ck.repair.ok == nil and ck.food.ok == nil)
    bots[1].dead = true
    ck = checks()
    check("Ready?: a dead bot named", ck.alive.ok == false and ck.alive.detail:find(bots[1].name))
    bots[1].dead = false
    function GetInventoryItemDurability(slot) if slot == 5 then return 10, 100 end return 90, 100 end
    function GetContainerNumSlots(bag) return bag == 0 and 2 or 0 end
    function GetContainerItemLink(bag, slot) return slot == 1 and "|Hitem:1|h" or nil end
    function GetItemInfo() return "Bread", nil, 1, 5, 1, "Consumable", "Food & Drink" end
    function GetLFGRoles() return true, true, false, false end
    ck = checks()
    check("Ready?: your worn gear named with its durability", ck.repair.ok == false and ck.repair.detail:find("10%%"))
    check("Ready?: food found in your bags", ck.food.ok == true)
    check("Ready?: you as tank (the role check) count", ck.roles.detail:find("you: tank") and ck.roles.detail:find("^2 tank"))
    local rw = C.OpenReady()
    check("Ready? window: right-click on the minimap button opens it too", (function()
        rw:Hide()
        _G.WrathUnleashedBotsMinimapButton:Fire("OnClick", "RightButton")
        return rw:IsShown() end)())
    check("Ready? window: the raid frames' ready marks", rw.rows[1].mark:GetTexture() == "Interface\\RaidFrame\\ReadyCheck-Ready"
        and (function() for _, r in ipairs(rw.rows) do if r.mark:GetTexture() == "Interface\\RaidFrame\\ReadyCheck-NotReady" then return true end end end)())
    serverClock = serverClock + 400
    rw.prepare:Fire("OnClick")
    check("Ready? Prepare bots: asks the realm, then counts as prepared", sent[#sent][2] == "wucomp\tprepare\t1/1\t"
        and checks().prepared.ok == true and rw.status:GetText() ~= "")
    check("Ready? the game's ready check waits for a group leader", rw.check.enabled == false)
    rw:Hide()
    GetInventoryItemDurability, GetContainerNumSlots, GetContainerItemLink, GetItemInfo, GetLFGRoles = nil, nil, nil, nil, nil

    -- saved parties
    local pt = C.ShowTab("party")
    C.SetPick(1, "warrior", "prot pve"); C.SetPick(3, "mage")
    pt.presets[2].save:Fire("OnClick")
    check("saved party: kept per character, shown with class and spec", WrathUnleashedUIChar.botPresets[2]
        and pt.presets[2].text:GetText():find("Warrior") and pt.presets[2].text:GetText():find("%(Prot") and pt.presets[1].text:GetText():find("Empty")
        and pt.presets[1].bring.enabled == false and pt.presets[2].bring.enabled == true)
    C.ResetPicks()
    serverClock = serverClock + 31
    pt.presets[2].bring:Fire("OnClick")
    check("saved party: Bring puts the picks back and fills with them", sent[#sent][2] == "wucomp\tfill\t1/1\tparty\ttank=warrior:prot pve,healer=any,dps=mage,dps=any"
        and C.GetPicks()[1].word == "warrior")
    C.ResetPicks()
    C.ShowTab("bots")

    -- chat: "Hide everything your bots say"
    check("bot party chat shows by default", filters.CHAT_MSG_PARTY(nil, "CHAT_MSG_PARTY", "Need mana", bots[2].name) == false)
    win.mute:SetChecked(true); win.mute:Fire("OnClick")
    check("mute: their whispers and party chat hidden, friends still shown", filters.CHAT_MSG_PARTY(nil, "CHAT_MSG_PARTY", "Need mana", bots[2].name) == true
        and filters.CHAT_MSG_WHISPER(nil, "CHAT_MSG_WHISPER", "Hi", bots[2].name) == true
        and filters.CHAT_MSG_PARTY(nil, "CHAT_MSG_PARTY", "hi", "Friend") == false and WrathUnleashedUIDB.botsMute == true)
    win.mute:SetChecked(false); win.mute:Fire("OnClick")
    check("mute off", filters.CHAT_MSG_PARTY(nil, "CHAT_MSG_PARTY", "Need mana", bots[2].name) == false)
end

-- ---------------------------------------------------------------- the game's own look (owner 12 Nov: "forever ui
-- elements not leyline squarechunks"): templates, backdrops and textures by the client's own names
do
    local G = W.GameLook
    local function under(f, root)
        local p = f
        for _ = 1, 20 do if p == nil then return false end; if p == root then return true end; p = p.parent end
        return false
    end
    local function flat(root)
        for _, f in ipairs(created) do
            if f ~= root and under(f, root) or f == root then
                if f.styleBg or f.styleFill or f.styleEdge or f.edRound then return "Style.lua helper on " .. tostring(f.kind) end
                if f.kind == "Texture" and type(f.tex) == "number" then return "a solid-colour texture" end
                if f.backdrop and (tostring(f.backdrop.bgFile):find("white8x8", 1, true)
                    or tostring(f.backdrop.edgeFile):find("white8x8", 1, true)) then return "the flat white8x8 backdrop" end
            end
        end
        return nil
    end
    check("look: GameLook.lua loads before the bots' files", at["GameLook.lua"] and at["GameLook.lua"] < at["CompanionsData.lua"])
    -- the window
    check("look: Bots window is the stock dialog (dialog border, dark dialog background) with the gold header banner",
        win.backdrop and win.backdrop.edgeFile == "Interface\\DialogFrame\\UI-DialogBox-Border"
        and win.backdrop.bgFile == "Interface\\DialogFrame\\UI-DialogBox-Background-Dark"
        and win.gameHeader and win.gameHeader.tex == "Interface\\DialogFrame\\UI-DialogBox-Header"
        and win.title:GetText() == "Bots" and win.close.template == "UIPanelCloseButton")
    check("look: the window's tabs are the game's bottom tabs, named for PanelTemplates", (function()
        for i, t in ipairs(win.tabs) do
            if t.btn.template ~= "CharacterFrameTabButtonTemplate" or t.btn:GetName() ~= "WrathUnleashedBotsFrameTab" .. i then return false end
        end
        return win.numTabs == 7 end)())
    check("look: the window's buttons are UIPanelButtonTemplate", find(win, "Fill party").template == "UIPanelButtonTemplate"
        and win.passive.template == "UIPanelButtonTemplate" and win.rows[1].hp.barTex == "Interface\\TargetingFrame\\UI-StatusBar")
    check("look: roster rows hover with the game's list highlight", win.rows[1].hover.tex == "Interface\\QuestFrame\\UI-QuestTitleHighlight")
    -- the bot bar
    local bf = Bar.Frame()
    local s1 = bf.slots[1]
    check("look: each bot is a party member frame (party art round a 37 px portrait, 70 x 7 stock bars)",
        s1.art.tex == "Interface\\TargetingFrame\\UI-PartyFrame" and s1.hp.barTex == "Interface\\TargetingFrame\\UI-StatusBar"
        and s1.mp.barTex == "Interface\\TargetingFrame\\UI-StatusBar" and s1.portrait.h == 37)
    check("look: the bar's header is a tooltip box with the game's plus button",
        bf.head.backdrop and bf.head.backdrop.edgeFile == "Interface\\Tooltips\\UI-Tooltip-Border"
        and bf.open.normalTex == "Interface\\Buttons\\UI-PlusButton-Up")
    local ob = Bar.Orders()
    check("look: the orders bar is a tooltip box, its orders dressed as action buttons",
        ob.backdrop and ob.backdrop.edgeFile == "Interface\\Tooltips\\UI-Tooltip-Border"
        and ob.btns.follow.normalTex == "Interface\\Buttons\\UI-Quickslot2" and ob.btns.passive.on.tex == "Interface\\Buttons\\CheckButtonHilight")
    -- the tips
    Bar.Tip(1)
    local tf = Bar.Tips()
    check("look: the tips are a tooltip box, panel buttons", tf.backdrop and tf.backdrop.bgFile == "Interface\\Tooltips\\UI-Tooltip-Background"
        and tf.backdrop.edgeFile == "Interface\\Tooltips\\UI-Tooltip-Border" and tf.next.template == "UIPanelButtonTemplate"
        and tf.skip.template == "UIPanelButtonTemplate" and tf.head:GetText() == "Tip 1 of 3")
    tf.next:Fire("OnClick")
    check("tips: Next still advances", tf.step == 2 and tf:IsShown())
    tf.skip:Fire("OnClick")
    check("tips: Skip tips still closes them", not tf:IsShown())
    -- the fallback menu (no UIDropDownMenu in this client)
    C.OpenMenu(C.bots[1].name)
    local pm = _G.WrathUnleashedBotMenu
    check("look: the menu without UIDropDownMenu is a dialog with panel buttons", C.MenuMode() == "panel" and pm:IsShown()
        and pm.backdrop and pm.backdrop.edgeFile == "Interface\\DialogFrame\\UI-DialogBox-Border"
        and pm.follow.template == "UIPanelButtonTemplate" and pm.close.template == "UIPanelCloseButton")
    pm:Hide()
    check("look: no flat squares (Style.lua helpers, solid-colour textures, white8x8 backdrops) in the bar, the orders, "
        .. "the tips, the menu, the window", (function()
        for _, root in ipairs({ bf, ob, tf, pm, win, C.Chooser(), _G.WrathUnleashedBotFormations, C.GearWindow(), C.ReadyWindow() }) do
            local why = root and flat(root)
            if why then print("  " .. tostring(root.name) .. ": " .. why); return false end
        end
        return true end)())

    -- the game's own right-click menu (UIDropDownMenu), when the client has it
    local added, inited = {}, nil
    UIDROPDOWNMENU_MENU_VALUE = nil
    function UIDropDownMenu_Initialize(frame, fn, mode) inited = { frame = frame, fn = fn, mode = mode } end
    function UIDropDownMenu_AddButton(info, level) added[#added + 1] = { info = info, level = level or 1 } end
    local toggled
    function ToggleDropDownMenu(level, value, frame, anchor)
        toggled = { level = level, frame = frame, anchor = anchor }
        added = {}
        UIDROPDOWNMENU_MENU_VALUE = value
        inited.fn(frame, level)
    end
    CANCEL = "Cancel"
    local b1 = C.bots[1]
    C.OpenMenu(b1.name)
    local function item(text) for _, a in ipairs(added) do if a.info.text == text then return a.info end end end
    local dd = C.Dropdown()
    check("menu: the game's own UIDropDownMenu at the cursor", C.MenuMode() == "dropdown" and dd and dd.template == "UIDropDownMenuTemplate"
        and inited.mode == "MENU" and toggled.anchor == "cursor" and toggled.level == 1)
    check("menu: its title is the bot (class colour, tagged a bot)", added[1].info.isTitle and added[1].info.text:find(b1.name, 1, true)
        and added[1].info.text:find("bot", 1, true))
    check("menu: orders, spec, role and gear are in it", item("Follow") and item("Stay") and item("Attack")
        and (item("Passive") or item("Fight")) and item("Gear up") and item("Send home") and item("Cancel")
        and item("Spec") and item("Spec").hasArrow and item("Role") and item("Role").hasArrow)
    ToggleDropDownMenu(2, "spec", dd)
    local nSpec = 0
    for _, a in ipairs(added) do if a.level == 2 then nSpec = nSpec + 1 end end
    check("menu: Spec opens the class's specs, the current one ticked and not clickable", nSpec == #(C.specs[b1.cls] or {}) and nSpec > 0
        and (function() for _, a in ipairs(added) do if a.info.checked and not a.info.disabled then return false end end return true end)())
    ToggleDropDownMenu(2, "role", dd)
    check("menu: Role opens Tank, Healer, Damage", item("Tank") and item("Healer") and item("Damage"))
    ToggleDropDownMenu(1, nil, dd)
    local w0 = #player.whispers
    item("Follow").func()
    runServer()
    check("menu: Follow orders that bot only", #player.whispers == w0 + 1 and player.whispers[#player.whispers].text == "follow"
        and player.whispers[#player.whispers].to.name == b1.name)
    UIDropDownMenu_Initialize, UIDropDownMenu_AddButton, ToggleDropDownMenu, CANCEL = nil, nil, nil, nil

    -- with Forever UI loaded: its own dialog, panel art, red buttons and close button
    local fui = {}
    ForeverUI = { SetPanelArt = function(f) fui.panel = f; f.foreverPanel = true end,
        Templates = { DialogFrame = function(f) fui.dialog = f end, ThreeSliceButton = function(b, tex) fui.button = tex end,
            CloseButton = function(b) fui.close = b end } }
    local d = CreateFrame("Frame", "WULookDialog", UIParent); G.Dialog(d)
    local p = CreateFrame("Frame", "WULookPanel", UIParent); G.Window(p, "Bots")
    local pb = G.Button(UIParent, "Next", 80)
    local pc = G.Close(p)
    check("Forever UI: its dialog frame, its panel art, its red three-slice button, its close button",
        fui.dialog == d and d.gameLook == "forever-dialog" and fui.panel == p and p.gameLook == "forever-panel"
        and fui.button == "128-redbutton" and pb.gameLook == "forever-redbutton" and fui.close == pc)
    ForeverUI = { Templates = { DialogFrame = function() error("boom") end, ThreeSliceButton = function() error("boom") end } }
    local d2 = CreateFrame("Frame", "WULookDialog2", UIParent); G.Dialog(d2)
    local b2 = G.Button(UIParent, "Next", 80)
    check("Forever UI: a failing call falls back to the stock look", d2.gameLook == "dialog"
        and d2.backdrop.edgeFile == "Interface\\DialogFrame\\UI-DialogBox-Border" and b2.gameLook == "UIPanelButtonTemplate")
    ForeverUI = nil
end

-- dismiss
find(win, "Dismiss all"):Fire("OnClick")
runServer()
check("Dismiss all: they leave, the window empties", #player.group.members == 1 and #C.bots == 0 and not win.rows[1]:IsShown()
    and win.sub:GetText() == "0 of 4 bots in your party")
check("nothing in the chat from any of it", #serverChat == 0)
check("no bots: the bot bar and the orders bar hide", not Bar.Frame():IsShown() and not Bar.Orders():IsShown())

-- single player: the welcome (the realm's "welcome" when bots joined a new character by themselves) ----------------
do
    local actions = { ["B"] = "OPENALLBAGS" }
    function GetBindingAction(k) return actions[k] or "" end
    function SetBinding(k, a) actions[k] = a; bindings[a] = k; return 1 end
    local saved = 0
    function SaveBindings() saved = saved + 1 end
    WrathUnleashedUIDB.botsKeyOffered = nil
    bindings.WUBOTS_WINDOW = nil
    inCombat = true
    check("welcome: no key change in combat (the client refuses)", C.OfferKey() == false and bindings.WUBOTS_WINDOW == nil
        and not WrathUnleashedUIDB.botsKeyOffered)
    inCombat = false
    local before = #chatLines
    fireClient("OVMOD", "wucomp\twelcome\t1/1\tYour bots are here: Anna the Priest (healer) and Bo the Warrior (tank). "
        .. "They follow you, fight with you and leave the loot to you.", "WHISPER", "Tester")
    local line = chatLines[#chatLines] or ""
    check("welcome: one chat line", #chatLines == before + 1)
    check("welcome: the window gets Ctrl-B (it was free) and keeps it", bindings.WUBOTS_WINDOW == "CTRL-B" and saved == 1
        and WrathUnleashedUIDB.botsKeyOffered == true)
    check("welcome: names them and says how to change them, with the key", line:find("Your bots are here: Anna the Priest", 1, true)
        and line:find("Change them in the Bots window: press Ctrl-B or click the Bots button by the minimap.", 1, true))
    check("welcome: no chat command in it", not line:find("/comp", 1, true) and not line:find("%.companions"))
    check("welcome: a short line on screen too", (errorsLines[#errorsLines] or ""):find("Bots window: Ctrl-B", 1, true))
    check("welcome: the minimap button shows", _G.WrathUnleashedBotsMinimapButton and _G.WrathUnleashedBotsMinimapButton:IsShown())
    -- a player who cleared the key keeps it cleared
    actions["CTRL-B"], bindings.WUBOTS_WINDOW = nil, nil
    fireClient("OVMOD", "wucomp\twelcome\t1/1\tYour bots are here: Anna the Priest (healer).", "WHISPER", "Tester")
    check("welcome: the key is offered once only", bindings.WUBOTS_WINDOW == nil and saved == 1
        and (chatLines[#chatLines] or ""):find("Change them in the Bots window: click the Bots button by the minimap.", 1, true))
    -- a key the player uses for something else is never taken
    WrathUnleashedUIDB.botsKeyOffered = nil
    actions["CTRL-B"] = "TOGGLEBAG1"
    check("welcome: Ctrl-B taken: left alone", C.OfferKey() == false and actions["CTRL-B"] == "TOGGLEBAG1" and bindings.WUBOTS_WINDOW == nil)
    -- dungeon bots are tagged as such
    check("roster: a dungeon bot's tag", C.ParseRoster("H\t1\t4\t0\tchaos\t0\t0\ton\t1\t1\nB\tDuno\t5\thealer\tdungeon\tholy pve\t10\tfollow\t20")
        and C.bots[1].kind == "dungeon" and C.UI.KIND_TAG.dungeon == "dungeon bot")
    C.ParseRoster("H\t0\t4\t0\tchaos\t0\t0\ton\t1\t1")
end

-- a realm without bots: the window says so
player.IsPlayerbot = nil
C.Ask()
check("no bot module: 'coming soon'", C.state == "soon" and win.sub:GetText():find("coming soon"))

-- names on screen
local BANNED = { "world of warcraft", "warcraft", "lich king", "blizzard", "playerbot", "azerothcore", "companion" }
check("no brand names (or 'companion') on screen", (function()
    for _, f in ipairs(created) do
        local t = type(f.text) == "string" and f.text:lower() or ""
        for _, w in ipairs(BANNED) do if t:find(w, 1, true) then print("  on screen: " .. f.text); return false end end
        if t:find("%f[%a]wow%f[%A]") then return false end
    end
    return true end)())

os.time = realTime
print(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
