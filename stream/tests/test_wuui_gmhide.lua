-- texlua stream/tests/test_wuui_gmhide.lua stream/fui-web/Interface/AddOns/WrathUnleashedUI/GMHide.lua
-- The GM bars (AzerothAdmin, MiniGM) are hidden outside Your Den and stay hidden; /gmbar shows them; in the Den they stay.
local file = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI/GMHide.lua"
local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end
local frames = {}
local function frame(name)
    local f = { shown = true, hooks = {}, events = {} }
    function f:IsShown() return self.shown end
    function f:Show() self.shown = true; for _, h in ipairs(self.hooks) do h(self) end end
    function f:Hide() self.shown = false end
    function f:HookScript(k, fn) if k == "OnShow" then self.hooks[#self.hooks + 1] = fn end end
    function f:RegisterEvent(e) self.events[e] = true end
    function f:SetScript(k, fn) self[k] = fn end
    if name then _G[name] = f end
    frames[#frames + 1] = f
    return f
end
function CreateFrame() return frame() end
SlashCmdList = {}
print = function() end
local realm = "Wrath Unleashed"
WrathUnleashedUI = { InDen = function() return realm == "Your Den" end }
local aa, mini, side = frame("ma_bgframe"), frame("MiniGMFrame"), frame("ma_minibgframe")
local mmButton, mmOther = frame("LibDBIcon10_AzerothAdmin"), frame("LibDBIcon10_Questie")
Minimap = frame("Minimap")
function Minimap:GetChildren() return mmButton, mmOther end
function frame_name(f, n) f.GetName = function() return n end end
frame_name(mmButton, "LibDBIcon10_AzerothAdmin"); frame_name(mmOther, "LibDBIcon10_Questie")
dofile(file)
local ev; for _, f in ipairs(frames) do if f.events.PLAYER_ENTERING_WORLD then ev = f end end
ev.OnEvent(ev, "PLAYER_ENTERING_WORLD")
check("official realm: AzerothAdmin hidden", not aa.shown)
check("official realm: MiniGM hidden", not mini.shown)
check("official realm: side bar hidden", not side.shown)
check("official realm: its minimap button hidden, another addon's kept", not mmButton.shown and mmOther.shown)
side:Show()
check("shown again by the addon: hidden at once", not side.shown)
SlashCmdList.WUGMBAR("")
check("/gmbar shows the bars", side.shown and mini.shown)
SlashCmdList.WUGMBAR("off")
check("/gmbar off hides them", not side.shown and not mini.shown)
realm = "Your Den"
side:Show(); mini:Show()
ev.OnEvent(ev, "PLAYER_ENTERING_WORLD")
check("in the Den: they stay", side.shown and mini.shown)
print = _G.io and function(s) io.write(s, "\n") end
io.write(string.format("\n%d/%d passed\n", pass, pass + fail))
if fail > 0 then os.exit(1) end
