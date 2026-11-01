-- lua5.1 stream/tests/test_wuui_denhome.lua [addon dir] [lua_scripts dir]
-- Fake 3.3.5a API (as test_wuui_builder.lua): loads every file of WrathUnleashedUI in .toc order and runs real round
-- trips through ov_build.lua, ov_den.lua, ov_den_home.lua and ov_modapi.lua (fake mod-ale, a Den): the Den home
-- panel (/denhome, the Den panel's Home), the owner's start choice (the realm's "start" opens it), Forest glade
-- placed, the stamps (pick, turn left and right, Place, Cancel; the spawn menu's Stamps tab), the grid and ground at
-- your feet, Knock, a note for the mailbox and Read mail, the GM row (shown by role, its hints, round trips), and no
-- game-brand names on screen. 2026-11-01: the template shelf in edit mode (owner and builders only, a stock icon and a
-- short hint per tile; a click marks the stamp, the mark follows you as you walk, throttled; Q / E turn, Enter places,
-- Esc cancels, all through override key bindings, in order; a starter place waits for Enter), the same keys for a
-- Builder prop waiting for its ground click, and the Den bar's one-line help with the rest behind "?".
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local scripts = arg[2] or "realm/custom/lua_scripts"

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- fake widgets (as test_wuui_leyline.lua)
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
function Frame:UnregisterEvent(e) self.events[e] = nil end
function Frame:GetName() return self.name end
function Frame:GetParent() return self.parent end
function Frame:GetHeight() return self.h or 0 end
function Frame:SetHeight(h) self.h = h end
local function new(kind, name, parent)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, events = {}, shown = true }, Frame)
    if name then _G[name] = f end
    created[#created + 1] = f
    return f
end
function Frame:GetEffectiveScale() return 1 end
function Frame:CreateFontString() return new("FontString", nil, self) end
function Frame:CreateTexture() return new("Texture", nil, self) end
function CreateFrame(kind, name, parent, template)
    local f = new(kind, name, parent); frames[#frames + 1] = f
    if template == "OptionsSliderTemplate" and name then
        new("FontString", name .. "Low", f); new("FontString", name .. "High", f)
    end
    return f
end

UIParent = new("Frame", "UIParent"); UIParent.top = true
WorldFrame = new("Frame", "WorldFrame"); WorldFrame.top = true
function UIParent:GetScale() return 1 end
GameMenuFrame = new("Frame", "GameMenuFrame"); GameMenuFrame.top = true; GameMenuFrame.h = 300
UISpecialFrames = {}
function tinsert(t, v) table.insert(t, v) end
SlashCmdList = {}
DEFAULT_CHAT_FRAME = { AddMessage = function() end }
function GetScreenHeight() return 981 end
function InCombatLockdown() return false end
function SetBinding() return 1 end
function SetBindingClick() return 1 end
function SaveBindings() end
local cv = { leylineEditMode = "0" }
function GetCVar(k) return cv[k] end
function SetCVar(k, v) cv[k] = tostring(v) end
local bindings = {}
function SetOverrideBindingClick(owner, prio, key, button) bindings[key] = button end
function ClearOverrideBindings() bindings = {} end
local speed = 0
function GetUnitSpeed() return speed end
function HasAction() return false end
function UnitLevel() return 80 end
function UnitClass() return "Mage", "MAGE" end
function UnitName() return "Tester" end
function PlaySound() end
function HideUIPanel(f) f:Hide() end
local clock = 100
function GetTime() return clock end


-- ---------------------------------------------------------------- the realm: a Den (fake mod-ale)
local realGetenv = os.getenv
local serverClock = 1000000
os.time = function() return serverClock end
local SAVES = os.tmpname(); os.remove(SAVES); os.execute("mkdir -p '" .. SAVES .. "'")
local env = { OV_DEN = "1", OV_DEN_SIZE = "16", OV_BUILD_PER_PLAYER = "400", OV_BUILD_POINTS = "1500", OV_BUILD_PER_AREA = "800",
    OV_BUILD_NEW_HOURS = "0", OV_DEN_SAVES = SAVES, OV_DEN_SHIPPED = scripts }
os.getenv = function(k) return env[k] end
local onServerAddon
local playerEvents = {}
function RegisterServerEvent(id, fn) if id == 30 then onServerAddon = fn end end
function RegisterPlayerEvent(id, fn) playerEvents[id] = playerEvents[id] or {}; table.insert(playerEvents[id], fn) end
function CreateLuaEvent() end
function CharDBExecute() end
function CharDBQuery() return nil end
local objects, nextGuid = {}, 1
local GROUND = 12.6
local map1 = { GetWorldObject = function(_, g) return objects[g] end, GetHeight = function() return GROUND end, IsDungeon = function() return false end }
function GetMapById() return map1 end
function PerformIngameSpawn(kind, entry, map, inst, x, y, z, o, save)
    nextGuid = nextGuid + 1
    local go = { guid = nextGuid, entry = entry, x = x, y = y, z = z, o = o }
    function go:GetGUID() return self.guid end
    function go:RemoveFromWorld() objects[self.guid] = nil end
    function go:SetScale(sc) self.scale = sc end
    objects[go.guid] = go
    return go
end
function GetGUIDLow(g) return g end
local function fireClient(prefix, msg, channel, sender)
    for _, f in ipairs(frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", prefix, msg, channel, sender) end
    end
end
local chat = {}
local player = { x = 16226.2 + 4, y = 16257.0 - 3, z = GROUND, o = 0 }
function player:GetGUIDLow() return 7 end
function player:GetName() return "Tester" end
function player:GetGMRank() return 1 end
function player:IsGM() return false end
function player:GetMapId() return 1 end
function player:GetMap() return map1 end
function player:GetX() return self.x end
function player:GetY() return self.y end
function player:GetZ() return self.z end
function player:GetO() return self.o end
function player:Teleport(m, x, y, z, o) self.x, self.y, self.z, self.o = x, y, z, o; return true end
function player:SendBroadcastMessage(m) chat[#chat + 1] = m end
function player:SendAddonMessage(prefix, msg) fireClient(prefix, msg, "WHISPER", "Tester") end
function player:SetSpeed(t, r) self.speeds = self.speeds or {}; self.speeds[t] = r end
function player:SetCanFly(on) self.canFly = on end
function player:SetGMVisible(on) self.gmVisible = on end
function player:SetGameMaster() end
function player:IsDead() return false end
function player:GetZoneId() return 876 end
local visitor = { x = 16226.2, y = 16257.0 - 10, z = GROUND, o = 0, msgs = {} }
for k, v in pairs(player) do if type(v) == "function" then visitor[k] = v end end
function visitor:GetGUIDLow() return 8 end
function visitor:GetName() return "Ana" end
function visitor:GetGMRank() return 0 end
function visitor:SendBroadcastMessage(m) self.msgs[#self.msgs + 1] = m end
function visitor:SendAddonMessage() end
function GetPlayersInWorld() return { player, visitor } end
function GetPlayerByName(n) if n == "Tester" then return player elseif n == "Ana" then return visitor end end
package.preload["ov_modapi"] = function() return dofile(scripts .. "/ov_modapi.lua") end
dofile(scripts .. "/ov_build.lua")
dofile(scripts .. "/ov_den.lua")
dofile(scripts .. "/ov_den_home.lua")
local T, D, HS = assert(OV_BUILD_TEST), assert(OV_DEN_TEST), assert(OV_DEN_HOME_TEST)
T.DEN.floorZ = GROUND
local OVMod = rawget(_G, "OVMod")

local sent = {}
function SendAddonMessage(prefix, msg, channel, target)
    sent[#sent + 1] = { prefix, msg, channel, target }
    if target == UnitName("player") and onServerAddon then onServerAddon(30, player, 7, prefix, msg, player) end
end
local function lastSent() return sent[#sent] and sent[#sent][2] end
local function sentHas(text) for _, m in ipairs(sent) do if m[2] == text then return true end end end
local function decor() local n = 0; for _, p in pairs(T.props) do if p.owner ~= 0 then n = n + 1 end end; return n end
local function chatHas(pat) for _, m in ipairs(chat) do if m:find(pat) then return true end end end

-- ---------------------------------------------------------------- load the addon in .toc order
local files = {}
for line in io.lines(dir .. "/WrathUnleashedUI.toc") do
    if line:match("%.lua%s*$") then files[#files + 1] = line:gsub("%s+$", "") end
end
local at = {}
for i, f in ipairs(files) do at[f] = i end
check(".toc lists DenHome.lua after EditMode.lua and before Leyline.lua", at["DenHome.lua"] and at["DenHome.lua"] > at["EditMode.lua"]
    and at["DenHome.lua"] < at["Leyline.lua"])
for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
os.getenv = function(k) return env[k] end
local W = assert(WrathUnleashedUI)
local H = assert(W.DenHome, "W.DenHome")
check("builds nothing until asked", H.Panel() == nil)
check("/denhome", SLASH_LEYLINEDENHOME1 == "/denhome" and SlashCmdList.LEYLINEDENHOME)

-- the owner logs in: the realm asks how the Den starts, and the panel opens on that choice
for _, fn in ipairs(playerEvents[3] or {}) do fn(3, player) end
local p = H.Panel()
check("the realm's start choice opens the panel", p and p:IsShown() and p.status:GetText():find("Pick how your Den starts"))
check("the home line arrived: your Den, not started", H.home.role == "owner" and H.home.started == "-" and p.who:GetText():find("not started yet"))
check("the owner's buttons are on", p.start.forest.enabled ~= false and p.read.enabled ~= false)
p.start.forest:Fire("OnClick")
check("Forest glade: sent as .den start forest", sentHas("wuden\tcmd\t1/1\tstart forest"))
check("and the realm put it in the Den", decor() > 30 and D.settings.started == "forest" and H.home.started == "forest"
    and p.who:GetText():find("Forest glade"))
local placed = decor()
p.replace:Fire("OnClick")
check("Replacing: yes", p.replace:GetText() == "Replacing: yes")
serverClock = serverClock + 10
p.start.keep:Fire("OnClick")
check("Dark keep, replacing", sentHas("wuden\tcmd\t1/1\tstart keep replace") and D.settings.started == "keep")
p.replace:Fire("OnClick")

-- stamps
serverClock = serverClock + 10
local before = decor()
p.stamp.garden:Fire("OnClick")
check("a stamp: marked at your feet, nothing placed yet", sentHas("wuden\tcmd\t1/1\tstamp garden") and HS.pending[7]
    and decor() == before and H.home.stamp == "garden" and p.marked:GetText():find("garden"))
check("a stamp from the panel, outside edit mode: no keys taken (Q, E and Enter stay yours)", next(bindings) == nil)
local o0 = HS.pending[7].o
p.turnL:Fire("OnClick")
check("Turn left: 15 degrees", lastSent() == "wuden\tcmd\t1/1\tstamp turn 15" and math.abs(HS.pending[7].o - (o0 + math.rad(15)) % (2 * math.pi)) < 1e-6)
p.turnR:Fire("OnClick"); p.turnR:Fire("OnClick")
check("Turn right twice: 15 the other way", math.abs(HS.pending[7].o - (o0 - math.rad(15)) % (2 * math.pi)) < 1e-6)
p.place:Fire("OnClick")
check("Place: the garden is down", decor() == before + #HS.stamp("garden").props and HS.pending[7] == nil and H.home.stamp == "-")
serverClock = serverClock + 10
p.stamp.dock:Fire("OnClick")
p.cancel:Fire("OnClick")
check("Cancel: the mark goes, nothing placed", HS.pending[7] == nil and decor() == before + #HS.stamp("garden").props)

-- the spawn menu's Stamps tab
local B = W.Builder
B.ShowMenu(true)
local menu = _G.WrathUnleashedSpawnMenu
local stampsTab
for _, t in ipairs(menu.tabs) do if t.tab == "stamps" then stampsTab = t end end
stampsTab:Fire("OnClick")
check("the spawn menu's Stamps tab: six stamps", #B.menuItems == 6 and B.menuItems[1].kind == "stamp")
local cell
for _, c in ipairs(menu.cells) do if c.item and c.item.key == "campfire" then cell = c end end
cell:Fire("OnEnter")
check("a stamp cell says what it is", B.status:find("Campfire circle") and B.status:find("mark where it goes"))
cell:Fire("OnClick", "RightButton")
check("stamps aren't favourites", not chatHas("favourite"))
cell:Fire("OnClick", "LeftButton")
check("a click marks it", HS.pending[7] and HS.pending[7].name == "campfire")
p.cancel:Fire("OnClick")
B.ShowMenu(false)

-- the grid and ground at your feet
p.grid:Fire("OnClick")
check("Grid 0.5 yd: told to the realm", p.grid:GetText() == "Grid 0.5 yd" and lastSent() == "wubuild\tcmd\t1/1\tgrid 0.5" and T.X.grid[7] == 0.5)
p.grid:Fire("OnClick"); p.grid:Fire("OnClick"); p.grid:Fire("OnClick")
check("then 1, 2, 5 yards", p.grid:GetText() == "Grid 5 yd" and T.X.grid[7] == 5)
p.grid:Fire("OnClick")
check("and off", p.grid:GetText() == "Grid off" and T.X.grid[7] == nil and lastSent() == "wubuild\tcmd\t1/1\tgrid off")
p.ground:Fire("OnClick")
check("Ground on", p.ground:GetText() == "Ground on" and T.X.ground[7] == true and WrathUnleashedUIChar.denhome.ground == true)
p.ground:Fire("OnClick")

-- knock and the mailbox
local door = HS.spot("door")
visitor.x, visitor.y = door.x, door.y
OVMod.Dispatch(visitor, "wuden", "cmd", "knock")
check("a visitor's knock: the owner's panel says so", p.status:GetText():find("Ana knocked at your front door"))
OVMod.Dispatch(visitor, "wuden", "note", "Love the |cffff0000keep|r!")
check("the visitor's note is in the mailbox", #HS.notes == 1 and HS.notes[1].from == "Ana")
check("the owner's panel counts it", H.home.mail == "1" and p.mailLine:GetText() == "1 new note")
p.read:Fire("OnClick")
check("Read mail: the latest notes show, colour codes gone", lastSent() == "wuden\tcmd\t1/1\tmail" and H.mail[1] and H.mail[1].from == "Ana"
    and p.notes[1]:GetText() == "Ana: Love the keep!" and p.mailLine:GetText() == "No new notes")
serverClock = serverClock + 61
p.note:SetText("Back soon")
p.leave:Fire("OnClick")
check("Leave a note sends the words as they are", lastSent() == "wuden\tnote\t1/1\tBack soon" and #HS.notes == 2 and p.note:GetText() == "")
local n0 = #sent
p.note:SetText("   ")
p.leave:Fire("OnClick")
check("an empty note never leaves the panel", #sent == n0 and p.status:GetText():find("Write a few words"))
p.visit:Fire("OnClick")
check("Go to the front door", lastSent() == "wuden\tcmd\t1/1\tvisit" and math.abs(player.x - door.x) < 0.01)

-- the GM row: the owner sees it all
local function shown(list) for _, f in ipairs(list) do if not f:IsShown() then return false end end; return true end
local function hidden(list) for _, f in ipairs(list) do if f:IsShown() then return false end end; return true end
check("GM row: the owner sees every button, the owner's too", p.gm.s2 and shown(p.gmAll) and shown(p.gmOwner)
    and p.gm.summon:IsShown() and p.gm.invisible:IsShown())
serverClock = serverClock + 10
p.gm.s2:Fire("OnClick")
check("GM 2x: sent as .den gm speed 2, the realm sets it, the button shows it", lastSent() == "wuden\tcmd\t1/1\tgm speed 2"
    and player.speeds[1] == 2 and H.home.gm == "2,0,0" and p.gm.s2:GetText() == "[2x]")
p.gm.fly:Fire("OnClick")
check("GM Fly: on, and the button says so", sentHas("wuden\tcmd\t1/1\tgm fly on") and player.canFly == true and p.gm.fly:GetText() == "Fly: on")
p.gm.fly:Fire("OnClick")
check("GM Fly again: off", lastSent() == "wuden\tcmd\t1/1\tgm fly off" and player.canFly == false and p.gm.fly:GetText() == "Fly: off")
p.gm.fly:Fire("OnEnter")
check("GM: a button's one-line hint shows under the row", p.gmHint:GetText():find("hold Space"))
p.gm.fly:Fire("OnLeave")
check("and goes on leaving it", p.gmHint:GetText() == "")
check("GM: every button has a hint", (function()
    for _, g in ipairs(H.GM_ALL) do if not (p.gm[g[1]].hint or ""):find("%a") then return false end end
    for _, g in ipairs(H.GM_OWNER) do if not (p.gm[g[1]].hint or ""):find("%a") then return false end end
    return true end)())
serverClock = serverClock + 10
local n1 = #sent
p.summonName:SetText("")
p.gm.summon:Fire("OnClick")
check("GM Summon with no name: nothing sent", #sent == n1 and p.status:GetText():find("name of a friend"))
p.summonName:SetText("Ana!")
p.gm.summon:Fire("OnClick")
check("GM Summon: someone not allowed in the Den is refused", lastSent() == "wuden\tcmd\t1/1\tgm summon Ana" and chatHas("isn't allowed"))
p.gm.s1:Fire("OnClick")
check("GM 1x: normal speed", player.speeds[1] == 1 and p.gm.s1:GetText() == "[1x]")

-- ---------------------------------------------------------------- the template shelf (edit mode)
local E = W.EditMode
local function tick(dt) for _, f in ipairs(frames) do if f.scripts.OnUpdate then f:Fire("OnUpdate", dt or 0.3) end end end
local function press(key)   -- a key with an override binding: one click down, one up (Modes.lua's way)
    local b = bindings[key] and _G[bindings[key]]
    if not b then return false end
    b:Fire("OnClick", "LeftButton", true); b:Fire("OnClick", "LeftButton", false)
    return true
end
local function denCmds(from)
    local out = {}
    for i = from + 1, #sent do local t = sent[i][2]:match("^wuden\tcmd\t1/1\t(.*)$"); if t then out[#out + 1] = t end end
    return table.concat(out, " | ")
end
tick()
check("shelf: not outside edit mode", H.Shelf() == nil or not H.Shelf():IsShown())
E.asking = clock; E.OnSnap("on"); tick()
local shelf = H.Shelf()
check("shelf: in edit mode, for the owner", E.IsOn() and shelf and shelf:IsShown())
check("shelf: nine tiles, stamps first (Inn, house, garden, market, campfire, dock), then the starter places", (function()
    local items = H.ShelfItems()
    local want = { "inn", "house", "garden", "market", "campfire", "dock", "forest", "keep", "empty" }
    for i, k in ipairs(want) do if not (items[i] and items[i].key == k and shelf.tiles[k]) then return false end end
    return #items == #want end)())
check("shelf: every tile a stock game icon and a short hint", (function()
    for key, b in pairs(shelf.tiles) do
        if not (b.icon.path or ""):match("^Interface\\Icons\\[%w_]+$") then return false end
        if not (b.hint and #b.hint > 8 and #b.hint <= 80) then return false end
    end
    return true end)())
check("shelf: the owner sees the starter places", shelf.tiles.forest:IsShown() and shelf.tiles.keep:IsShown())
check("shelf: no Place or Cancel until something is picked", not shelf.place:IsShown() and not shelf.cancel:IsShown())
local line0 = shelf.hint:GetText()
shelf.tiles.garden:Fire("OnEnter")
check("shelf: a hover hint under the row", shelf.hint:GetText():find("^Garden: "))
shelf.tiles.garden:Fire("OnLeave")
check("and gone after", shelf.hint:GetText() == line0)

-- a stamp from the shelf: mark, follow, Q, E, Enter, in that order
serverClock = serverClock + 10
local before = decor()
local n0 = #sent
shelf.tiles.inn:Fire("OnClick")
tick()
check("shelf click: the Inn marked at your feet, nothing placed", HS.pending[7] and HS.pending[7].name == "inn" and decor() == before
    and H.armed and H.armed.key == "inn" and shelf.tiles.inn.on:IsShown())
check("shelf: Q, E, Enter and Esc bound while placing", bindings.Q and bindings.E and bindings.ENTER and bindings.ESCAPE)
check("shelf: Place and Cancel show", shelf.place:IsShown() and shelf.cancel:IsShown() and shelf.hint:GetText():find("Esc"))
local x0 = HS.pending[7].x
speed = 7; player.x = player.x + 5
clock = clock + 2; tick()
check("walking: the mark follows (stamp here)", denCmds(n0) == "stamp inn | stamp here" and math.abs(HS.pending[7].x - x0) > 4)
tick(); tick()
check("throttled: no second here within 1.5 seconds", denCmds(n0) == "stamp inn | stamp here")
speed = 0; clock = clock + 2; tick()
check("stopping: one last here where you stopped", denCmds(n0) == "stamp inn | stamp here | stamp here")
clock = clock + 2; tick()
check("standing still: no more", denCmds(n0) == "stamp inn | stamp here | stamp here")
player.x = player.x - 5
local o0 = HS.pending[7].o
press("Q")
check("Q: turns it 15 degrees (one press, one turn)", denCmds(n0) == "stamp inn | stamp here | stamp here | stamp turn 15"
    and math.abs(HS.pending[7].o - (o0 + math.rad(15)) % (2 * math.pi)) < 1e-6)
press("E")
check("E: back the other way", denCmds(n0) == "stamp inn | stamp here | stamp here | stamp turn 15 | stamp turn -15"
    and math.abs(HS.pending[7].o - o0) < 1e-6)
clock = clock + 1
E.SpawnMenu(true); E.SpawnMenu(false)
check("the client's hold-Q while a stamp is marked: turns, no spawn menu", denCmds(n0):find("stamp turn %-15 | stamp turn 15$")
    and not (_G.WrathUnleashedSpawnMenu and _G.WrathUnleashedSpawnMenu:IsShown()))
local n1 = #sent
press("Q")
check("the same Q press heard by the binding too counts once", #sent == n1)
clock = clock + 1
press("E")
press("ENTER")
tick()
check("Enter: the Inn is down", denCmds(n0) == "stamp inn | stamp here | stamp here | stamp turn 15 | stamp turn -15 | stamp turn 15 | stamp turn -15 | stamp place"
    and decor() == before + #HS.stamp("inn").props and HS.pending[7] == nil)
check("and the keys go back", H.armed == nil and next(bindings) == nil and not shelf.place:IsShown())

-- Esc cancels; the same tile twice cancels too
serverClock = serverClock + 10
before = decor()
n0 = #sent
shelf.tiles.dock:Fire("OnClick"); tick()
press("ESCAPE"); tick()
check("Esc: the mark goes, nothing placed", denCmds(n0) == "stamp dock | stamp cancel" and HS.pending[7] == nil and decor() == before
    and H.armed == nil and next(bindings) == nil)
shelf.tiles.campfire:Fire("OnClick"); shelf.tiles.campfire:Fire("OnClick"); tick()
check("the same tile again: cancelled", denCmds(n0) == "stamp dock | stamp cancel | stamp campfire | stamp cancel" and HS.pending[7] == nil)
shelf.tiles.garden:Fire("OnClick"); shelf.tiles.market:Fire("OnClick"); tick()
check("another tile: the mark moves to that stamp", HS.pending[7] and HS.pending[7].name == "market" and H.armed.key == "market")
shelf.cancel:Fire("OnClick"); tick()
check("Cancel button", HS.pending[7] == nil and H.armed == nil)

-- a starter place waits for Enter (replacing: the Den has things in it)
serverClock = serverClock + 10
n0 = #sent
shelf.tiles.forest:Fire("OnClick"); tick()
check("a starter tile sends nothing yet and says it replaces", denCmds(n0) == "" and H.armed and H.armed.kind == "start"
    and shelf.hint:GetText():find("replaces"))
press("Q")
check("Q does nothing to a starter", denCmds(n0) == "")
press("ENTER"); tick()
check("Enter: start forest replace", denCmds(n0) == "start forest replace" and D.settings.started == "forest" and H.armed == nil)
shelf.tiles.keep:Fire("OnClick"); press("ESCAPE"); tick()
check("Esc: the Den stays as it is", denCmds(n0) == "start forest replace" and D.settings.started == "forest")

-- a Builder prop: the same click-to-place flow, Q / E turn it, Esc cancels
local B = W.Builder
local item
for _, c in ipairs(B.catalog) do if c.cat == "seating" then item = c; break end end
B.pick = item
B.Spawn(); tick()
check("a Builder prop waits for its ground click; Q, E and Esc bound (not Enter: chat stays)", E.pending and E.pending.kind == "prop"
    and bindings.Q and bindings.E and bindings.ESCAPE and not bindings.ENTER)
press("Q"); press("Q")
check("Q twice: it will land turned 30 degrees", E.pending.turn == 30 and E.note:find("30"))
local nb = #sent
serverClock = serverClock + 10
E.Clicked(player.x + 2, player.y + 1, player.z)
local seq = {}
for i = nb + 1, #sent do local t = sent[i][2]:match("^wubuild\tcmd\t1/1\t(.*)$"); if t then seq[#seq + 1] = t end end
check("the click places it, then turns it", seq[1] and seq[1]:find("^spawnat " .. item.id) and seq[2] == "turn 30")
tick()
check("and the keys go back", next(bindings) == nil)
B.Spawn(); tick()
nb = #sent
press("ESCAPE"); tick()
check("Esc: the prop isn't placed", E.pending == nil and #sent == nb and next(bindings) == nil)

-- the Den bar: one line of help, the long help behind "?"
local hud = _G.LeylineEditModeHUD
check("Den bar: the help is one line, the keys behind ?", (function()
    local help
    for _, f in ipairs(created) do if f.text and f.text:find("^Click select") then help = f.text end end
    return help and #help <= 70 and E and hud end)())
local helpBtn
for _, f in ipairs(created) do if f.text == "?" and f.tip and #f.tip.lines >= 8 then helpBtn = f end end
check("? keeps the long keys, hidden until the mouse is on it", helpBtn and not helpBtn.tip:IsShown()
    and (helpBtn:Fire("OnEnter") or true) and helpBtn.tip:IsShown() and (helpBtn:Fire("OnLeave") or true) and not helpBtn.tip:IsShown())
check("shelf and help: no game-brand words", (function()
    local texts = {}
    for _, it in ipairs(H.ShelfItems()) do texts[#texts + 1] = it.hint; texts[#texts + 1] = it.name end
    for _, l in ipairs(helpBtn and helpBtn.tip.lines or {}) do texts[#texts + 1] = l:GetText() end
    for _, t in ipairs(texts) do
        local l = t:lower()
        if l:find("warcraft") or l:find("blizzard") or l:find("lich king") or t:find("%f[%a]WoW%f[%A]") or l:find("%f[%a]wow%f[%A]") then return false end
    end
    return #texts > 10 end)())

-- a builder: the row, without the owner's buttons
fireClient("OVMOD", "wuden\thome\t1/1\tstarted=keep;door=1;mailbox=1;mail=0;role=builder;stamp=-;stamps=garden;gm=1,0,0", "WHISPER", "Tester")
check("GM row: a builder sees speed, fly, mark, go, middle, revive, not weather, invisible or summon", shown(p.gmAll)
    and hidden(p.gmOwner) and not p.summonName:IsShown())
tick()
check("shelf: a builder sees the stamps, not the starter places", shelf:IsShown() and shelf.tiles.inn:IsShown()
    and not shelf.tiles.forest:IsShown())

-- a visitor's panel: the owner's and builders' buttons are off
fireClient("OVMOD", "wuden\thome\t1/1\tstarted=keep;door=1;mailbox=1;mail=0;role=visitor;stamp=-;stamps=garden", "WHISPER", "Tester")
check("a visitor: start, stamps and mail buttons off", p.start.forest.enabled == false and p.stamp.garden.enabled == false
    and p.read.enabled == false and p.who:GetText() == "You're visiting this Den.")
check("a visitor: no GM row at all", hidden(p.gmAll) and hidden(p.gmOwner))
tick()
check("shelf: a visitor never sees it", not shelf:IsShown())
E.Leave(); tick()
check("shelf: gone with edit mode", not E.IsOn() and not shelf:IsShown())
fireClient("OVMOD", "wuden\thome\t1/1\trole=owner;started=forest", "WHISPER", "Someone")
check("lines from anyone else are ignored", H.home.role == "visitor")
fireClient("OVMOD", "wuden\tknock\t1/1\t|cff00ff00Eve|r<script>", "WHISPER", "Tester")
check("a knock shows only the letters of a name", p.status:GetText() == "Evescript knocked at your front door.")

-- the Den panel's Home button
E.ShowDen(true)
p:Hide()
E.DenPanel().home:Fire("OnClick")
check("Den panel > Home opens it", p:IsShown())
E.ShowDen(false)

check("no game-brand names on screen", (function()
    for _, f in ipairs(created) do
        local t = (f.text or ""):lower()
        if t:find("warcraft") or t:find("blizzard") or t:find("lich king") or t:find("%f[%a]wow%f[%A]") then return false end
    end
    return true end)())

os.execute("rm -rf '" .. SAVES .. "'")
os.getenv = realGetenv
print(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
