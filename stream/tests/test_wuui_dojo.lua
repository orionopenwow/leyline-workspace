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
local sent, cmds = {}, {}
local server = "lite"
local state = { fly = 0, speed = 1, dummies = 0, ring = 0 }
local function line()
    if server == "lite" then
        return string.format("server=lite;fly=%d;speed=%d;dummies=%d;ring=%d;building=0", state.fly, state.speed, state.dummies, state.ring)
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
    elseif name == "cmd" and data:match("^dojo") then
        cmds[#cmds + 1] = data
        local w = data:match("^dojo (%S+)") or ""
        if w == "fly" then state.fly = 1 - state.fly
        elseif w == "speed" then state.speed = tonumber(data:match("speed (%d)"))
        elseif w == "dummies" then state.dummies = 3
        elseif w == "ring" then state.ring = 1
        elseif w == "clear" then state.dummies = 0 end
        fireClient("wuden\tdojo\t1/1\t" .. line())
    end
end
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

-- ---------------------------------------------------------------- 1. Den Lite: the rail's Dojo button
fire("PLAYER_ENTERING_WORLD")
local bar = D.Bar()
check("the rail has a Dojo button", bar.dojo and bar.dojo:GetText() == "Dojo" and bar.dojo.iconPath == W.Style.Icon("dojo") and bar.dojo.edIcon)
bar.dojo:Fire("OnClick")
local p = J.Panel()
check("it opens the Dojo panel beside the rail", J.IsOpen() and p:IsShown() and p.head:GetText() == "DOJO" and bar.dojo:IsActive())
check("and asks for the Dojo's line", lastCmd() == "dojo status" and J.line.server == "lite")
check("Den Lite: its note", p.note:GetText():find("^Den Lite: the ring, dummies"))
check("the sections: Training, Moving, Jaina builds, Full Den", p.training:GetText() == "TRAINING" and p.moving:GetText() == "MOVING"
    and p.builds:GetText() == "JAINA BUILDS" and p.full:GetText() == "FULL DEN ONLY")
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
local before = #cmds
check("Next boss cycles the pick, sending nothing", J.NextBoss() == "vancleef" and p.bossPick:GetText() == "Vancleef" and #cmds == before)

-- ---------------------------------------------------------------- 2. the full Den
server = "full"
J.Cmd("status")
check("the full Den: everything on", J.line.server == "full" and p.fight.enabled == true and p.look.enabled == true
    and p.full:GetText() == "FULL DEN" and p.note:GetText():find("count your damage"))
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
