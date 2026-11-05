-- lua5.1 stream/tests/test_wuui_tourney.lua [addon dir]   (texlua runs it too)
-- The Tournament window (DenTourney.lua; owner 4 Nov: "duel tournaments in the Den ... a bracket window in the UI
-- (ForeverUI style)"). Fake 3.3.5a API (as test_wuui_dojo.lua) and every file of WrathUnleashedUI in .toc order,
-- with a fake server answering the window's "tourney ..." commands as Den Lite does (labs/den-lite tourney.rs: the
-- tourney line, the sign-ups and the bracket in parts): the Den bar's Arena button opens it; the host's buttons send
-- new/ring/twist/start/win/cancel; Join, Leave, Yield; the bracket by round with the match being fought marked; a
-- friend (not the host) gets no host buttons; parts are put back together; nothing from the server is run; names are
-- letters only; no game-brand names on screen.
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

-- ---------------------------------------------------------------- a fake server: Den Lite's tourney.rs
local function fireClient(msg)
    for _, f in ipairs(frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", "OVMOD", msg, "WHISPER", "Wren") end
    end
end
local cmds = {}
local me = "host"
local st = nil   -- the tournament
local function send()
    if not st then fireClient("wuden\ttourney\t1/1\tphase=none"); return end
    fireClient(string.format("wuden\ttourney\t1/1\tfmt=%s;bo=%d;phase=%s;n=%d;a=%s;b=%s;wa=%d;wb=%d;champ=%s;ring=%d;me=%s;in=%d;twist=%s;title=%s",
        st.fmt, st.bo, st.phase, #st.players, st.a or "-", st.b or "-", st.wa, st.wb, st.champ or "-", st.ring, me, #st.players > 0 and 1 or 0, st.twist, "Spring Cup"))
    fireClient("wuden\ttsign\t1/1\t1/1|" .. table.concat(st.players, ","))
    -- the bracket in two parts, the second first (they may come in any order)
    local rows = {}
    for _, m in ipairs(st.matches) do rows[#rows + 1] = table.concat({ m[1], m[2], m[3], m[4], m[5], m[6] }, ",") end
    local p1 = table.concat({ rows[1], rows[2], rows[3] }, ";")
    local rest = {}
    for i = 4, #rows do rest[#rest + 1] = rows[i] end
    if #rest > 0 then
        fireClient("wuden\ttbr\t1/1\t1/2|" .. p1)
        fireClient("wuden\ttbr\t1/1\t2/2|" .. table.concat(rest, ";"))
    else
        fireClient("wuden\ttbr\t1/1\t1/1|" .. p1)
    end
end
function SendAddonMessage(prefix, msg, channel, target)
    if prefix ~= "OVMOD" or target ~= "Wren" then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= "wuden" then return end
    if name == "hello" then
        if realm == "Your Den" then fireClient("wuden\tstatus\t1/1\trole=" .. (me == "host" and "owner" or "visitor") .. ";props=0;saved=-") end
    elseif name == "cmd" and data:match("^tourney") then
        cmds[#cmds + 1] = data
        local w = data:match("^tourney (%S+)") or "status"
        if w == "new" then
            st = { fmt = data:match("new (%a+)"), bo = tonumber(data:match("bo(%d)")) or 1, phase = "signup", players = {}, matches = {}, wa = 0, wb = 0, ring = 0, twist = "-" }
        elseif w == "join" then table.insert(st.players, "Wren")
        elseif w == "ring" then st.ring = 1
        elseif w == "twist" then st.twist = data:match("twist (%a+)")
        elseif w == "start" then
            st.phase = "fight"; st.a, st.b = "Rowan", "Fen"
            st.matches = { { 1, "Rowan", "Fen", 0, 0, "-" }, { 1, "Briar", "Ash", 0, 0, "-" }, { 1, "Wren", "-", 0, 0, "Wren" }, { 1, "Moss", "Reed", 2, 1, "Moss" } }
        elseif w == "win" then st.wa = st.wa + 1
        elseif w == "cancel" then st = nil end
        send()
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
check(".toc lists DenTourney.lua after DenBar.lua and Style.lua", at["DenTourney.lua"] and at["DenTourney.lua"] > at["DenBar.lua"] and at["DenTourney.lua"] > at["Style.lua"])
for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
local W = assert(WrathUnleashedUI)
local T = assert(W.Tourney, "W.Tourney")
local D = assert(W.DenBar, "W.DenBar")
check("nothing of the window before it is opened", not T.IsOpen() and _G.LeylineTourneyPanel == nil)

-- ---------------------------------------------------------------- 1. the host: the Arena button, a new tournament
fire("PLAYER_ENTERING_WORLD")
local bar = D.Bar()
check("the rail has an Arena button", bar.arena and bar.arena:GetText() == "Arena" and bar.arena.iconPath == W.Style.Icon("banners"))
bar.arena:Fire("OnClick")
local p = T.Panel()
check("it opens the Tournament window and asks for the state", T.IsOpen() and p:IsShown() and lastCmd() == "tourney status" and bar.arena:IsActive())
check("no tournament yet: it says how", p.title:GetText() == "No tournament yet" and p.note:GetText():find("presses New"))
check("the host's buttons are on; Start and Cancel wait for a tournament", p.new.enabled ~= false and p.start.enabled == false and p.cancel.enabled == false)
p.formats[2]:Fire("OnClick")
p.bests[2]:Fire("OnClick")
check("the format and best of are picked here, sending nothing", T.format == "robin" and T.bo == 3 and p.formats[2]:IsActive() and lastCmd() == "tourney status")
p.formats[1]:Fire("OnClick")
p.new:Fire("OnClick")
check("New: tourney new single bo3", lastCmd() == "tourney new single bo3")
check("the window follows the line", p.title:GetText():find("Spring Cup") and p.title:GetText():find("Single elimination, best of 3")
    and p.note:GetText():find("Sign%-ups open") and p.start.enabled ~= false)
p.join:Fire("OnClick")
check("Join: tourney join; then Join is lit and Leave is on", lastCmd() == "tourney join" and p.join:IsActive() and p.join.enabled == false and p.leave.enabled ~= false)
check("the sign-ups' names", p.signed:GetText() == "Signed up: Wren")
p.ring:Fire("OnClick")
check("Ring here: tourney ring, lit from the line", lastCmd() == "tourney ring" and p.ring:IsActive() and p.note:GetText():find("in the ring"))
check("Rule cycles the house rules, sending nothing", T.NextTwist() == "ringout" and p.rule:GetText() == "Ring out" and lastCmd() == "tourney ring")
T.NextTwist(); T.NextTwist(); T.NextTwist(); T.NextTwist(); T.NextTwist(); T.NextTwist(); T.NextTwist()
check("Sudden death", T.Twist() == "sudden" and p.rule:GetText() == "Sudden death")
p.ruleSet:Fire("OnClick")
check("Set: tourney twist sudden", lastCmd() == "tourney twist sudden" and p.ruleSet:IsActive() and p.note:GetText():find("rule: Sudden death"))

-- ---------------------------------------------------------------- 2. it runs: the bracket, the match, the host's call
p.start:Fire("OnClick")
check("Start: tourney start", lastCmd() == "tourney start")
check("the match being fought, with its score", p.vs:GetText():find("Rowan") and p.vs:GetText():find("0 %- 0") and p.vs:GetText():find("Fen"))
check("the bracket put back together from its two parts", #T.matches == 4 and T.matches[4].a == "Moss")
check("round 1 in the first column", p.cols[1].head:GetText() == "ROUND 1" and p.cols[2].head:GetText() == "")
check("the match being fought is marked; a bye says so; a played one has its winner and score",
    p.cols[1].lines[1]:GetText():find("> ") and p.cols[1].lines[1]:GetText():find("Rowan %- Fen")
    and p.cols[1].lines[3]:GetText() == "Wren (bye)" and p.cols[1].lines[4]:GetText():find("Moss") and p.cols[1].lines[4]:GetText():find("2%-1"))
check("the host's call: <name> won, for each fighter", p.winA:IsShown() and p.winA:GetText() == "Rowan won" and p.winB:GetText() == "Fen won")
p.winA:Fire("OnClick")
check("Rowan won: tourney win Rowan", lastCmd() == "tourney win Rowan" and p.vs:GetText():find("1 %- 0"))
check("New waits while it runs; Start too", p.new.enabled == false and p.start.enabled == false)
check("Yield only for a fighter (Wren isn't fighting)", p.yield.enabled == false)
p.bracket:Fire("OnClick")
check("In chat: tourney bracket", lastCmd() == "tourney bracket")

-- ---------------------------------------------------------------- 3. a friend: no host buttons
me = "in"
send()
check("a friend: the host's buttons are off, saying why", p.new.enabled == false and p.ring.enabled == false and p.start.enabled == false
    and p.ring.edLabel:find("host runs") and p.hostRow:GetText() == "THE HOST'S")
check("and no host's call", not p.winA:IsShown())
st.a = "Wren"
send()
check("a fighter can Yield", p.yield.enabled ~= false)
p.yield:Fire("OnClick")
check("Yield: tourney yield", lastCmd() == "tourney yield")
me = "host"

-- ---------------------------------------------------------------- 4. careful with what comes in
fireClient("wuden\ttbr\t1/1\t1/1|1,Ro|cffff0000wan,Fen,0,0,-;x,evil,,;2,<b>,Ash,0,0,-")
check("names are letters only; a broken row is left out", #T.matches == 2 and T.matches[1].a == "Rocffffwan" and T.matches[2].a == "b")
fireClient("wuden\ttourney\t1/1\tfmt=chess;phase=fight")
check("a line for no format is left out", T.line.fmt == "single")
fireClient("wuden\ttbr\t1/1\t2/3|1,A,B,0,0,-")
check("a part on its own waits for the others", #T.matches == 2)
p.cancel:Fire("OnClick")
check("Cancel: tourney cancel; the window is empty again", lastCmd() == "tourney cancel" and p.title:GetText() == "No tournament yet" and #T.matches == 0)

-- ---------------------------------------------------------------- 5. closing, /tourney, leaving the Den
p.close:Fire("OnClick")
check("x closes it, the Arena button goes back", not T.IsOpen() and not bar.arena:IsActive())
SlashCmdList.LEYLINETOURNEY("")
check("/tourney opens it", T.IsOpen())
realm = "Somewhere else"
fire("PLAYER_ENTERING_WORLD")
check("leaving the Den closes it", not T.IsOpen())

local brands = { "warcraft", "lich king", "blizzard", "world of" }
check("no game-brand names on screen", (function()
    for _, f in ipairs(created) do
        if type(f.text) == "string" then
            local t = f.text:lower()
            for _, b in ipairs(brands) do if t:find(b, 1, true) then return false end end
            if t:find("%f[%a]wow%f[%A]") then return false end
        end
    end
    return true
end)())

print(string.format("%d passed, %d failed", pass, fail))
if fail > 0 then os.exit(1) end
