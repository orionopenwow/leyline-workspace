-- lua5.1 stream/tests/test_wuui_denquiet.lua stream/fui-web/Interface/AddOns/WrathUnleashedUI/WrathUnleashedUI.lua
-- Your Den only (owner's den-test, 1 Nov): the game's "Couldn't load Blizzard_CombatLog: Banned" box over the world
-- is closed again in the Den (realm "Your Den"), and nowhere else; any other message stays. Fake widgets only.
local file = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI/WrathUnleashedUI.lua"
local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

local frames = {}
local function newFrame()
    local f = { events = {}, scripts = {} }
    function f:RegisterEvent(e) self.events[e] = true end
    function f:UnregisterEvent(e) self.events[e] = nil end
    function f:SetScript(k, fn) self.scripts[k] = fn end
    frames[#frames + 1] = f
    return f
end
function CreateFrame() return newFrame() end
SlashCmdList = {}
local realm = "Your Den"
function GetRealmName() return realm end
-- the game's box and message() (FrameXML BasicControls.xml)
BasicMessageDialog = { shown = false }
function BasicMessageDialog:IsShown() return self.shown end
function BasicMessageDialog:Show() self.shown = true end
function BasicMessageDialog:Hide() self.shown = false end
BasicMessageDialogText = { GetText = function(self) return self.t end, SetText = function(self, t) self.t = t end }
function message(text)
    if not BasicMessageDialog:IsShown() then
        BasicMessageDialogText:SetText(text)
        BasicMessageDialog:Show()
    end
end
local secure = 0
function hooksecurefunc(name, post)
    local orig = _G[name]
    _G[name] = function(...) local r = orig(...); secure = secure + 1; post(...); return r end
end
-- only the box's own frame (the one that also waits for PLAYER_LOGIN): the bars and the brand aren't this test's
local function fire(event, ...)
    for _, f in ipairs(frames) do
        if f.events[event] and f.events.PLAYER_LOGIN and f.scripts.OnEvent then f.scripts.OnEvent(f, event, ...) end
    end
end

-- the box shown before the addon loaded: closed at login, in the Den
message("Couldn't load Blizzard_CombatLog: Banned")
dofile(file)
local W = WrathUnleashedUI
check("in the Den: knows it", W.InDen())
fire("PLAYER_LOGIN")
check("a box up before the addon loaded is closed at login", not BasicMessageDialog:IsShown())

-- shown after: closed at once by the post-hook
message("Couldn't load Blizzard_CombatLog: Banned")
check("shown after: closed at once", not BasicMessageDialog:IsShown() and secure == 1)

-- any other message stays, also in the Den
message("Couldn't load Questie: Missing")
check("another message stays", BasicMessageDialog:IsShown())
BasicMessageDialog:Hide()

-- on any other realm it stays
realm = "Wrath Unleashed"
message("Couldn't load Blizzard_CombatLog: Banned")
check("not in the Den: it stays", BasicMessageDialog:IsShown() and not W.InDen())
fire("PLAYER_ENTERING_WORLD")
check("not in the Den: still there after entering the world", BasicMessageDialog:IsShown())
BasicMessageDialog:Hide()

-- no realm name at all (a client without the call): nothing is hidden
GetRealmName = nil
check("no realm name: not the Den", not W.InDen() and not W.DenQuiet("Couldn't load Blizzard_CombatLog: Banned"))

print(string.format("\n%d/%d passed", pass, pass + fail))
if fail > 0 then os.exit(1) end
