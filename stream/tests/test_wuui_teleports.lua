-- lua5.1 stream/tests/test_wuui_teleports.lua [addon dir]
-- The Teleports window (Teleports.lua; owner 4 Nov: "a Teleports window with categories, search, favourites, recent,
-- 'Back to my Den' always one click/key"). Fake 3.3.5a API (as test_wuui_dojo.lua) and every file of WrathUnleashedUI
-- in .toc order, with a fake Den Lite answering the window's Den commands as labs/den-lite world.rs does: /tp opens it
-- and asks for its line; the categories hold the list's places (TeleportsData.lua); a search finds by any part of a
-- name, best first, and Enter goes to the first; a row's click is one "go <name>" command and lands in Recent; the star
-- keeps a place in Favourites (kept in WrathUnleashedUIDB); Back to my Den is "home" and Shift-Home is its key in Your
-- Den unless the player set one; Back is "back" and is off until the server says where it goes; My Dens are the kept
-- Dens; nothing from the server is run; no game-brand names on screen.
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
function SetOverrideBinding(owner, prio, key, command) overrides[key] = command end
local ownKeys = {}
function GetBindingKey(command) return ownKeys[command] end
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

-- ---------------------------------------------------------------- a fake Den Lite: the window's commands
local function fireClient(msg)
    for _, f in ipairs(frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", "OVMOD", msg, "WHISPER", "Wren") end
    end
end
local cmds = {}
local backTo = "-"
function SendAddonMessage(prefix, msg, channel, target)
    if prefix ~= "OVMOD" or target ~= "Wren" then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= "wuden" or name ~= "cmd" then return end
    cmds[#cmds + 1] = data
    if data:match("^go ") then backTo = "Stormwind" end
    if data == "places status" or data:match("^go ") then
        fireClient("wuden\tplaces\t1/1\tden=the island;map=0;back=" .. backTo .. ";kept=Goldshire,My |cffff0000Fort|r")
    end
end
local function lastCmd() return cmds[#cmds] end
local function fire(event, ...)
    for _, f in ipairs(frames) do if f.events[event] then f:Fire("OnEvent", event, ...) end end
end

-- ---------------------------------------------------------------- load the addon in .toc order
local files = {}
for l in io.lines(dir .. "/WrathUnleashedUI.toc") do
    if l:match("%.lua%s*$") then files[#files + 1] = l:gsub("%s+$", "") end
end
local at = {}
for i, f in ipairs(files) do at[f] = i end
check(".toc lists TeleportsData.lua then Teleports.lua, after Style.lua and EditMode.lua", at["Teleports.lua"] and at["TeleportsData.lua"]
    and at["TeleportsData.lua"] < at["Teleports.lua"] and at["Teleports.lua"] > at["Style.lua"] and at["Teleports.lua"] > at["EditMode.lua"])
for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
local W = assert(WrathUnleashedUI)
local T = assert(W.Teleports, "W.Teleports")
check("nothing of the window before it is opened", not T.IsOpen() and _G.LeylineTeleports == nil)
check("the list: over 200 places in ten categories", #T.Places() > 200 and #T.Cats() == 10)

-- ---------------------------------------------------------------- 1. Back to my Den: one key, from the start
fire("PLAYER_ENTERING_WORLD")
check("in Your Den, Shift-Home is Back to my Den", overrides["SHIFT-HOME"] == "LEYLINE_DENHOME")
ownKeys.LEYLINE_DENHOME = "CTRL-H"
fire("PLAYER_ENTERING_WORLD")
check("unless the player set a key of their own", overrides["SHIFT-HOME"] == nil)
ownKeys.LEYLINE_DENHOME = nil
realm = "Somewhere"
fire("PLAYER_ENTERING_WORLD")
check("elsewhere the key is given back", overrides["SHIFT-HOME"] == nil)
realm = "Your Den"
fire("PLAYER_ENTERING_WORLD")
T.Home()
check("Back to my Den is one command: home", lastCmd() == "home")

-- ---------------------------------------------------------------- 2. the window
SlashCmdList["LEYLINETELEPORTS"]("")
local f = T.Frame()
check("/tp opens it and asks for its line", T.IsOpen() and f:IsShown() and lastCmd() == "places status" and T.line.map == "0")
check("Back is off until the server says where it goes", f.back.enabled == false)
check("its title and the always-there Back to my Den", f.styleTitle:GetText() == "Teleports" and f.home:GetText() == "Back to my Den")
f.home:Fire("OnClick")
check("Back to my Den: home", lastCmd() == "home")
-- the first tab: cities
check("it opens on Cities", T.tab == "city" and f.rows[1].entry.name == "stormwind" and f.rows[1]:GetText() == "Stormwind")
f.tabs.raid:Fire("OnClick")
local raids = T.Entries()
check("a tab: its places", T.tab == "raid" and #raids > 12 and raids[1].name == "moltencore")
check("twelve rows and a count", f.rows[12]:IsShown() and f.count:GetText():find("^1 to 12 of"))
T.Scroll(3)
check("the wheel scrolls", T.offset == 3 and f.rows[1].entry.name == raids[4].name)
T.Scroll(-99)
check("not past the top", T.offset == 0)
f.rows[1]:Fire("OnClick")
check("a row's click: go <name>", lastCmd() == "go moltencore")
check("and the server's line: Back goes to Stormwind", T.line.back == "Stormwind" and f.back.enabled ~= false and f.back.edLabel == "Back to Stormwind")
f.back:Fire("OnClick")
check("Back: back", lastCmd() == "back")

-- search
f.search:SetText("storm")
local hits = T.Entries()
check("a search finds by any part of a name, best first", hits[1].name == "stormwind" and #hits >= 3)
local names = {}
for _, h in ipairs(hits) do names[h.name] = true end
check("... the harbor, the keep, the peaks", names.stormwindharbor and names.stormwindkeep and names.stormpeaks)
f.search:SetText("icc")
check("a short name", T.Entries()[1].name == "icecrowncitadel")
f.search:Fire("OnEnterPressed")
check("Enter goes to the first", lastCmd() == "go icecrowncitadel")
f.search:SetText("Un'Goro")
check("titles with marks in them", T.Entries()[1].name == "ungoro")
f.search:SetText("qqqqzz")
check("nothing: says so", #T.Entries() == 0 and f.count:GetText() == "Nothing has that name." and not f.rows[1]:IsShown())
f.search:SetText("")

-- recent and favourites
f.tabs.recent:Fire("OnClick")
check("Recent: the newest first", T.Entries()[1].name == "icecrowncitadel" and T.Entries()[2].name == "moltencore")
f.rows[1].star:Fire("OnClick")
check("the star keeps it", T.IsFav("icecrowncitadel") and WrathUnleashedUIDB.teleFav[1] == "icecrowncitadel")
f.tabs.fav:Fire("OnClick")
check("Favourites", #T.Entries() == 1 and f.rows[1].entry.name == "icecrowncitadel" and f.rows[1].star.styleOn == true)
f.rows[1].star:Fire("OnClick")
check("the star again: gone", not T.IsFav("icecrowncitadel") and #T.Entries() == 0 and f.count:GetText():find("^No favourites yet"))
check("a name not on the list is never kept or sent", T.ToggleFav("nowhere") == false and T.Go("nowhere") == false and lastCmd() ~= "go nowhere")
for i = 1, 20 do T.Go(T.Places()[i][1]) end
check("Recent keeps twelve", #WrathUnleashedUIDB.teleRecent == T.RECENT)

-- your Dens kept at other places
f.tabs.kept:Fire("OnClick")
local kept = T.Entries()
check("My Dens: the server's names, plain", #kept == 2 and kept[1].title == "Goldshire" and kept[2].title == "My Fort")
f.rows[1]:Fire("OnClick")
check("one of them: go <its name>", lastCmd() == "go goldshire")
check("no star for a kept Den", not f.rows[1].star:IsShown())

-- the server's line: only a whisper from yourself, only with a map
local was = T.line
T.OnMessage("OVMOD", "wuden\tplaces\t1/1\tden=x;map=1", "WHISPER", "Somebody")
T.OnMessage("OVMOD", "wuden\tplaces\t1/1\tden=x", "WHISPER", "Wren")
check("nothing from others, nothing without a map", T.line == was)

-- /tp <place>: at once
SlashCmdList["LEYLINETELEPORTS"]("dalaran")
check("/tp dalaran goes", lastCmd() == "go dalaran")
SlashCmdList["LEYLINETELEPORTS"]("home")
check("/tp home", lastCmd() == "home")
f:Hide()
realm = "Somewhere"
T.Show(true)
fire("PLAYER_ENTERING_WORLD")
check("leaving Your Den closes it", not T.IsOpen())

-- ---------------------------------------------------------------- 3. plain
local brands = { "warcraft", "lich king", "blizzard", "world of" }
check("no game-brand names on screen or in the list", (function()
    for _, x in ipairs(created) do
        if type(x.text) == "string" then
            local t = x.text:lower()
            for _, b in ipairs(brands) do if t:find(b, 1, true) then return false end end
            if t:find("%f[%a]wow%f[%A]") then return false end
        end
    end
    for _, p in ipairs(T.Places()) do
        local t = p[2]:lower()
        for _, b in ipairs(brands) do if t:find(b, 1, true) then return false end end
    end
    return true
end)())
check("nothing in the chat", #chatLines == 0)

print(string.format("%d passed, %d failed", pass, fail))
if fail > 0 then os.exit(1) end
