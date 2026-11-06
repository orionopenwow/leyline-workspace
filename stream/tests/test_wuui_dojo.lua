-- lua5.1 stream/tests/test_wuui_dojo.lua [addon dir]
-- The Dojo's panel (Dojo.lua; owner 3 Nov: "the Dojo in the Den ... all built in with clever UI sections"). Fake
-- 3.3.5a API (as test_wuui_denbar.lua) and every file of WrathUnleashedUI in .toc order, with a fake server answering
-- the panel's Den commands as Den Lite (labs/den-lite dojo.rs) or the full Den (ov_dojo.lua) do: the rail's Dojo
-- button opens the panel and asks for its line; each button is one "dojo ..." command; on Den Lite what needs a
-- combat engine is off and says why; on the full Den all of it is on; the damage per second shows in the readout and
-- the last numbers as a toast; Fly and the speed light up from the server's line; leaving the Den closes it; /dojo;
-- nothing from the server is run; no game-brand names on screen.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- fake widgets (as test_wuui_denhome.lua)
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
function Frame:HookScript(n, fn)
    local old = self.scripts[n]
    self.scripts[n] = function(...) if old then old(...) end; return fn(...) end
end
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
function Frame:SetText(t) self.text = t; if self.scripts.OnTextChanged and self.kind == "EditBox" then self:Fire("OnTextChanged") end end
function Frame:GetText() return self.text end
function Frame:Enable() self.enabled = true end
function Frame:Disable() self.enabled = false end
function Frame:SetFocus() self.focus = true end
function Frame:ClearFocus() self.focus = false end
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
GameMenuFrame.shown = false
UISpecialFrames = {}
function tinsert(t, v) table.insert(t, v) end
SlashCmdList = {}
local chatLines = {}
DEFAULT_CHAT_FRAME = { AddMessage = function(_, m) chatLines[#chatLines + 1] = m end }
function GetScreenHeight() return 981 end
function InCombatLockdown() return false end
function SetBinding() return 1 end
function SetBindingClick() return 1 end
function SaveBindings() end
local cv = {}
function GetCVar(k) return cv[k] end
function SetCVar(k, v) cv[k] = tostring(v) end
local overrides = {}
function SetOverrideBindingClick(owner, prio, key, button) overrides[key] = button end
function ClearOverrideBindings() overrides = {} end
local bound = {}   -- the player's own bindings
function GetBindingAction(key) return bound[key] or "" end
function GetUnitSpeed() return 0 end
function HasAction() return false end
function UnitLevel() return 1 end
function UnitClass() return "Warrior", "WARRIOR" end
function UnitName() return "Wren" end
function PlaySound() end
function HideUIPanel(f) f:Hide() end
local realm = "Your Den"
function GetRealmName() return realm end
local clock = 100
function GetTime() return clock end

-- ---------------------------------------------------------------- a fake server: Den Lite, or the full Den
local function fireClient(msg)
    for _, f in ipairs(frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", "OVMOD", msg, "WHISPER", "Wren") end
    end
end
local sent, cmds, jcmds = {}, {}, {}
-- Jaina's brain on Den Lite (jaina.rs): her line after each word
local jaina = { talk = "normal", doing = "follow", offer = "-", ask = "-" }
local function jainaLine()
    return string.format("talk=%s;doing=%s;mood=calm;offer=%s;ask=%s;mine=0;critters=0;placed=0;fav=-", jaina.talk, jaina.doing, jaina.offer, jaina.ask)
end
local server = "lite"
local state = { fly = 0, speed = 1, dummies = 0, ring = 0, combat = 0, fight = 0, god = 0 }
local function line()
    if server == "lite" then
        local extra = state.combat == 1 and string.format(";combat=1;class=mage;level=80;fight=%d;parse=-;best=-;god=%d", state.fight, state.god) or ""
        return string.format("server=lite;fly=%d;speed=%d;dummies=%d;ring=%d;building=0", state.fly, state.speed, state.dummies, state.ring) .. extra
    end
    return string.format("server=full;fly=%d;speed=%d;dummies=%d;ring=%d;boss=-;duel=0;best=42.5", state.fly, state.speed, state.dummies, state.ring)
end
function SendAddonMessage(prefix, msg, channel, target)
    sent[#sent + 1] = msg
    if prefix ~= "OVMOD" or target ~= "Wren" then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= "wuden" then return end
    if name == "hello" then
        -- only a Den answers (elsewhere nobody does, so the bar stays shut)
        if realm == "Your Den" then fireClient("wuden\tstatus\t1/1\trole=owner;props=0;saved=-") end
    elseif name == "cmd" and data:match("^jaina") then
        jcmds[#jcmds + 1] = data
        local w = data:match("^jaina (%S+)") or ""
        if server ~= "lite" then return end
        if w == "quiet" then jaina.talk = "quiet" elseif w == "talk" then jaina.talk = "normal"
        elseif w == "stay" then jaina.doing = "stay" elseif w == "follow" then jaina.doing = "follow"
        elseif w == "idea" then jaina.offer = "corner"; jaina.ask = "offer"
        elseif w == "yes" or w == "no" then jaina.offer = "-"; jaina.ask = "-"
        elseif w == "tidy" then jaina.ask = "tidy4" end
        fireClient("wuden\tjaina\t1/1\t" .. jainaLine())
    elseif name == "cmd" and data:match("^dojo") then
        cmds[#cmds + 1] = data
        local w = data:match("^dojo (%S+)") or ""
        if w == "fly" then state.fly = 1 - state.fly
        elseif w == "speed" then state.speed = tonumber(data:match("speed (%d)"))
        elseif w == "dummies" then state.dummies = 3
        elseif w == "ring" then state.ring = 1
        elseif w == "clear" then state.dummies = 0
        elseif w == "fight" then state.fight = data:match("fight on") and 1 or 0
        elseif w == "invulnerable" then state.god = data:match("invulnerable on") and 1 or 0 end
        if state.combat == 1 and (w == "status" or w == "class" or w == "bars" or w == "gearset") then
            fireClient("wuden\tloadout\t1/1\tclass=mage;spec=fire;specs=arcane,fire,frost;bars=aoe;saved=single,aoe;set=t10;tiers=1,10;mine=raid")
        end
        fireClient("wuden\tdojo\t1/1\t" .. line())
        if w == "status" and server == "lite" then fireClient("wuden\tjaina\t1/1\t" .. jainaLine()) end
    end
end
local function lastJ() return jcmds[#jcmds] end
local function lastCmd() return cmds[#cmds] end
local function fire(event, ...)
    for _, f in ipairs(frames) do if f.events[event] then f:Fire("OnEvent", event, ...) end end
end

-- ---------------------------------------------------------------- load the addon in .toc order
local files = {}
for line in io.lines(dir .. "/WrathUnleashedUI.toc") do
    if line:match("%.lua%s*$") then files[#files + 1] = line:gsub("%s+$", "") end
end
local at = {}
for i, f in ipairs(files) do at[f] = i end
check(".toc lists Dojo.lua after DenBar.lua and Style.lua", at["Dojo.lua"] and at["Dojo.lua"] > at["DenBar.lua"] and at["Dojo.lua"] > at["Style.lua"])
for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
local W = assert(WrathUnleashedUI)
local J = assert(W.Dojo, "W.Dojo")
local D = assert(W.DenBar, "W.DenBar")
check("nothing of the Dojo before it is opened", not J.IsOpen() and _G.LeylineDojoPanel == nil)

local before
-- ---------------------------------------------------------------- 1. Den Lite: the rail's Dojo button
fire("PLAYER_ENTERING_WORLD")
local bar = D.Bar()
check("the rail has a Dojo button", bar.dojo and bar.dojo:GetText() == "Dojo" and bar.dojo.iconPath == W.Style.Icon("dojo") and bar.dojo.edIcon)
bar.dojo:Fire("OnClick")
local p = J.Panel()
check("it opens the Dojo panel beside the rail", J.IsOpen() and p:IsShown() and p.head:GetText() == "DOJO" and bar.dojo:IsActive())
check("and asks for the Dojo's line", lastCmd() == "dojo status" and J.line.server == "lite")
check("Den Lite: its note", p.note:GetText():find("^Den Lite: the ring, dummies"))
check("the sections: Training, Moving, Builds, Full Den", p.training:GetText() == "TRAINING" and p.moving:GetText() == "MOVING"
    and p.builds:GetText() == "BUILDS" and p.full:GetText() == "FULL DEN")
-- owner 5 Nov: "FULL DEN ONLY" greyed buttons on Den Lite were clutter; the row is not there at all
check("Den Lite: the full Den's row hidden, not greyed", not p.full:IsShown() and not p.bossPick:IsShown() and not p.look:IsShown()
    and p.fullRow.hidden == true)
check("five tabs in their own row, under the title", (function()
    local n = 0
    for _, t in ipairs(J.TABS) do if p.tabs[t[1]] then n = n + 1 end end
    return n == 5 and p.tabs.jaina:GetText() == "Jaina" and p.tabs.practice:GetText() == "Practice" end)())
-- (the label column measured: test_wuui_denlayout.lua, whose fake measures text)
check("on Den Lite what needs combat is off, saying why", p.bossPick.enabled == false and p.fight.enabled == false and p.duel.enabled == false
    and p.look.enabled == false and p.fight.edLabel:find("full Den") and p.ring.enabled ~= false and p.fly.enabled ~= false)
check("speed 1 is on", p.speeds[1]:IsActive() and not p.speeds[2]:IsActive())

-- each button: one "dojo ..." command
p.ring:Fire("OnClick")
check("Ring: dojo ring", lastCmd() == "dojo ring" and p.ring:IsActive())
p.dummies:Fire("OnClick")
check("Dummies: dojo dummies 3", lastCmd() == "dojo dummies 3" and p.dummies:IsActive())
p.fly:Fire("OnClick")
check("Fly: dojo fly, and it lights up from the server's line", lastCmd() == "dojo fly" and p.fly:IsActive())
p.speeds[3]:Fire("OnClick")
check("4x: dojo speed 4", lastCmd() == "dojo speed 4" and p.speeds[3]:IsActive() and not p.speeds[1]:IsActive())
p.stamps[1]:Fire("OnClick")
check("Jaina builds a garden: dojo build garden", lastCmd() == "dojo build garden" and p.stamps[1]:GetText() == "Garden")
p.clear:Fire("OnClick")
check("Clear: dojo clear", lastCmd() == "dojo clear" and not p.dummies:IsActive())

-- Jaina's rows (owner 4 Nov: "the player can talk to her via the Den panel (buttons)")
check("the Jaina rows", p.jainaRow:GetText() == "ORDERS" and p.ideasRow:GetText() == "HER IDEAS"
    and p.jFollow:GetText() == "Follow" and p.jIdea:GetText() == "Idea" and p.jYes:GetText() == "Keep")
check("her line came with the Dojo's: following, talking", J.jaina.talk == "normal" and p.jFollow:IsActive() and not p.jStay:IsActive()
    and not p.jQuiet:IsActive())
check("nothing to answer: Keep and No off", p.jYes.enabled == false and p.jNo.enabled == false and p.jIdea.enabled ~= false)
before = #cmds
p.jStay:Fire("OnClick")
check("Stay: .den jaina stay (not a dojo command), lit from her line", lastJ() == "jaina stay" and #cmds == before and p.jStay:IsActive()
    and not p.jFollow:IsActive())
p.jFollow:Fire("OnClick")
check("Follow: jaina follow", lastJ() == "jaina follow" and p.jFollow:IsActive())
p.jQuiet:Fire("OnClick")
check("Quiet: jaina quiet, and it lights up", lastJ() == "jaina quiet" and p.jQuiet:IsActive())
p.jQuiet:Fire("OnClick")
check("Quiet again: jaina talk", lastJ() == "jaina talk" and not p.jQuiet:IsActive())
p.jIdea:Fire("OnClick")
check("Idea: jaina idea; Keep and No on, the idea in the readout", lastJ() == "jaina idea" and p.jYes.enabled == true and p.jNo.enabled == true
    and p.readout:GetText():find("Jaina's idea: Corner"))
p.jYes:Fire("OnClick")
check("Keep: jaina yes; nothing open again", lastJ() == "jaina yes" and p.jYes.enabled == false and not (p.readout:GetText() or ""):find("idea"))
p.jTidy:Fire("OnClick")
check("Tidy: jaina tidy; she asks first", lastJ() == "jaina tidy" and p.jYes.enabled == true and p.readout:GetText():find("tidy up%?"))
p.jNo:Fire("OnClick")
check("No: jaina no", lastJ() == "jaina no" and p.jNo.enabled == false)
p.jPet:Fire("OnClick")
check("Pet: jaina pet", lastJ() == "jaina pet")
fireClient("wuden\tjaina\t1/1\ttalk=loud;doing=|cffff0000x")
check("a line with no talk word is left out", J.jaina.talk == "normal")
before = #cmds
check("Next boss cycles the pick, sending nothing", J.NextBoss() == "vancleef" and p.bossPick:GetText() == "Vancleef" and #cmds == before)

-- ---------------------------------------------------------------- 1b. Den Lite with its sandbox combat (combat=1)
check("without combat=1 the practice rows are off", p.practiceChips[1].enabled == false and p.sessionChips[1].enabled == false and p.classBe.enabled == false)
state.combat = 1
J.Cmd("status")
check("with combat=1: Practice, Session and Class on", J.Combat() and p.practiceChips[1].enabled == true and p.sessionChips[1].enabled == true
    and p.classBe.enabled == true and p.practice:GetText() == "PRACTICE" and p.session:GetText() == "SESSION")
check("the note says what Den Lite does now", p.note:GetText():find("any class"))
check("the readout: the class and level", p.readout:GetText():find("Mage 80"))
local verbs = {}
for _, b in ipairs(p.practiceChips) do b:Fire("OnClick"); verbs[#verbs + 1] = lastCmd() end
check("Pack, Mover, Healer, Tank, Boss: their dojo commands", table.concat(verbs, ",") == "dojo pack,dojo mover,dojo healer,dojo tank,dojo boss")
verbs = {}
for _, b in ipairs(p.sessionChips) do b:Fire("OnClick"); verbs[#verbs + 1] = lastCmd() end
check("1m 3m 5m Reset Refill Fight", table.concat(verbs, ",") == "dojo parse 1,dojo parse 3,dojo parse 5,dojo reset,dojo refill,dojo fight on")
check("Fight lights up from the line", p.fightBack:IsActive())
before = #cmds
check("the class picker starts at what you are (the loadout line)", J.Class() == "mage" and p.classPick:GetText() == "Mage")
check("Next class cycles, sending nothing", J.NextClass() == "warlock" and p.classPick:GetText() == "Warlock" and #cmds == before)
p.classBe:Fire("OnClick")
check("Be: dojo class <the pick> <its spec>", lastCmd() == "dojo class warlock affliction")
p.mounts:Fire("OnClick")
check("Mounts: dojo mounts", lastCmd() == "dojo mounts")

-- ---------------------------------------------------------------- 1c. the pages, the range, the loadout (owner 3 Nov:
-- "spellbook placement loadout instant on bars, gear sets selector ... test range dojo features")
check("the tabs: Train shown, Loadout hidden", p.pages.train:IsShown() and not p.pages.loadout:IsShown()
    and p.tabs.train:IsActive() and not p.tabs.loadout:IsActive() and p.tabs.train:GetText() == "Train")
check("Train's rows are visible, Practice's and Loadout's are not", p.ring:IsVisible() and not p.practiceChips[1]:IsVisible() and not p.classBe:IsVisible())
p.tabs.practice:Fire("OnClick")
check("the Practice tab: the practice dummies and the range", J.page == "practice" and p.practiceChips[1]:IsVisible() and p.safe:IsVisible()
    and not p.ring:IsVisible() and p.tabs.practice:IsActive())
p.tabs.session:Fire("OnClick")
check("the Session tab: the parses", J.page == "session" and p.sessionChips[1]:IsVisible() and p.fightBack:IsVisible() and not p.practiceChips[1]:IsVisible())
p.tabs.jaina:Fire("OnClick")
check("the Jaina tab: her builds, orders, ideas", J.page == "jaina" and p.stamps[1]:IsVisible() and p.jFollow:IsVisible() and p.jIdea:IsVisible())
p.tabs.loadout:Fire("OnClick")
check("the Loadout tab: its page", J.page == "loadout" and p.pages.loadout:IsShown() and not p.pages.train:IsShown()
    and p.tabs.loadout:IsActive() and p.classBe:IsVisible() and not p.practiceChips[1]:IsVisible())
check("its sections: Class, Bars, Gear, Name", p.classRow:GetText() == "CLASS" and p.barsRow:GetText() == "BARS"
    and p.gearRow:GetText() == "GEAR" and p.nameRow:GetText() == "NAME")
J.Cmd("status")
check("the loadout line: the pickers follow what you are", J.loadout.class == "mage" and J.loadout.spec == "fire"
    and J.Class() == "mage" and J.Spec() == "fire" and p.specPick:GetText() == "Fire" and p.classBe:IsActive())
check("your saved bars, the one on first", p.barPick:GetText() == "aoe" and p.barUse.enabled ~= false)
check("gear: the tiers your game has, then your sets", table.concat(J.GearChoices(), ",") == "t1,t10,raid" and p.gearPick:GetText() == "Tier 1")
check("Next spec cycles, sending nothing", (function() local n = #cmds; return J.NextSpec() == "frost" and p.specPick:GetText() == "Frost" and #cmds == n end)())
check("another spec isn't what you are", not p.classBe:IsActive())
p.classBe:Fire("OnClick")
check("Be: dojo class mage frost", lastCmd() == "dojo class mage frost")
p.barPick:Fire("OnClick")
check("the next saved bar", J.Bar() == "single" and p.barPick:GetText() == "single")
p.barUse:Fire("OnClick")
check("Use: dojo bars use single", lastCmd() == "dojo bars use single")
p.nameBox:SetText("Boss Fight!")
p.barSave:Fire("OnClick")
check("Save by the name in the box (plain)", lastCmd() == "dojo bars save boss fight")
p.nameBox:SetText("")
p.barSave:Fire("OnClick")
check("Save with no name: bar <n>", lastCmd() == "dojo bars save bar 3")
p.barReset:Fire("OnClick")
check("Reset: dojo bars reset", lastCmd() == "dojo bars reset")
p.gearWear:Fire("OnClick")
check("Wear: dojo gearset t1", lastCmd() == "dojo gearset t1")
J.NextGear(); J.NextGear()
p.gearWear:Fire("OnClick")
check("your own set: dojo gearset raid", lastCmd() == "dojo gearset raid" and p.gearPick:GetText() == "raid")
p.gearSave:Fire("OnClick")
check("Save gear: dojo gearset save set 2", lastCmd() == "dojo gearset save set 2")
p.gearOff:Fire("OnClick")
check("Off: dojo gearset off", lastCmd() == "dojo gearset off")
fireClient("wuden\tloadout\t1/1\tclass=bard;spec=x")
check("a loadout line for no class is left out", J.loadout.class == "mage")
fireClient("wuden\tloadout\t1/1\tclass=mage;spec=fire;specs=arcane,fire,frost;bars=-;saved=;set=-;tiers=;mine=|cffff0000x|r")
check("no bars, no sets: Use and Wear off, words plain", p.barPick:GetText() == "No saved bar" and p.barUse.enabled == false
    and table.concat(J.GearChoices(), ",") == "x")
p.tabs.train:Fire("OnClick")
check("back to Train", J.page == "train" and p.pages.train:IsShown())
check("the Range row", p.rangeRow:GetText() == "RANGE" and #p.rangeChips == 5)
verbs = {}
for _, b in ipairs(p.rangeChips) do b:Fire("OnClick"); verbs[#verbs + 1] = lastCmd() end
check("Levels, Group, Range, Run, Safe: their dojo commands", table.concat(verbs, ",") == "dojo levels,dojo group,dojo range,dojo demo 30,dojo invulnerable on")
check("Safe lights up from the line", p.safe:IsActive())
p.safe:Fire("OnClick")
check("Safe again: off", lastCmd() == "dojo invulnerable off" and not p.safe:IsActive())
fireClient("wuden\tdps\t1/1\tdps=900;total=9000;secs=10.0;dummy=Wounded Friend;hps=1200;healed=12000;done=1")
check("healing per second beside the damage", p.readout:GetText():find("900 DPS") and p.readout:GetText():find("1200 HPS"))
state.combat = 0

-- ---------------------------------------------------------------- 2. the full Den
server = "full"
J.Cmd("status")
J.jaina = {}
J.Paint()
check("without her line (the full Den): Follow and Stay only", p.jFollow.enabled ~= false and p.jStay.enabled ~= false and p.jIdea.enabled == false
    and p.jPet.enabled == false and p.jYes.enabled == false and p.jIdea.edLabel:find("Den Lite"))
J.Page("train")
check("the full Den: everything on, its row shown", J.line.server == "full" and p.fight.enabled == true and p.look.enabled == true
    and p.full:GetText() == "FULL DEN" and p.full:IsShown() and p.bossPick:IsShown() and p.note:GetText():find("count your damage"))
p.fight:Fire("OnClick")
check("Fight: dojo boss <the pick>", lastCmd() == "dojo boss vancleef")
p.duel:Fire("OnClick")
check("Duels: dojo duel on", lastCmd() == "dojo duel on")
p.look:Fire("OnClick")
check("Look: dojo look", lastCmd() == "dojo look")
check("the best boss time in the readout", p.readout:GetText():find("Best boss time: 42.5 s"))
fireClient("wuden\tdps\t1/1\tdps=1500;total=3000;secs=2.0;dummy=Grandmaster's Training Dummy")
check("the damage per second in the readout", p.readout:GetText():find("1500 DPS") and p.readout:GetText():find("3000 in 2.0 s"))
fireClient("wuden\tdps\t1/1\tdps=1500;total=3000;secs=2.0;dummy=Grandmaster's Training Dummy;done=1")
local function toasted(pat)
    if D.current and D.current.text:find(pat) then return true end
    for _, t in ipairs(D.toasts) do if t.text:find(pat) then return true end end
end
check("the last numbers: done, and a toast in the Den bar", p.readout:GetText():find("%(done%)") and toasted("1500 damage per second"))
fireClient("wuden\tdps\t1/1\tdps=lots;total=|cffff0000x")
check("numbers only: anything else is left out", p.readout:GetText():find("1500 DPS"))
fireClient("wuden\tdojo\t1/1\tserver=evil;fly=1")
check("a line from neither server is left out", J.line.server == "full")
fireClient("wuden\tdojo\t1/1\tserver=full;fly=|cffff0000|Hx|h1")
check("texts plain (colour codes and links out)", not tostring(J.line.fly):find("|", 1, true))

-- ---------------------------------------------------------------- 3. closing, /dojo, leaving the Den
p.close:Fire("OnClick")
check("x closes it, the rail's button goes back", not J.IsOpen() and not bar.dojo:IsActive())
SlashCmdList.LEYLINEDOJO("")
check("/dojo opens it", J.IsOpen())
realm = "Somewhere else"
fire("PLAYER_ENTERING_WORLD")
check("leaving the Den closes it", not J.IsOpen())

-- ---------------------------------------------------------------- 4. plain
local brands = { "warcraft", "lich king", "blizzard", "world of" }
check("no game-brand names on screen", (function()
    for _, f in ipairs(created) do
        if type(f.text) == "string" then
            local t = f.text:lower()
            for _, b in ipairs(brands) do if t:find(b, 1, true) then return false end end
            if t:find("%f[%a]wow%f[%A]") then return false end
        end
        if type(f.edLabel) == "string" and f.edLabel:lower():find("warcraft", 1, true) then return false end
    end
    return true
end)())
check("nothing in the chat", (function()
    for _, l in ipairs(chatLines) do if l:find("Dojo") or l:find("dojo") then return false end end
    return true end)())

print(string.format("%d passed, %d failed", pass, fail))
if fail > 0 then os.exit(1) end
