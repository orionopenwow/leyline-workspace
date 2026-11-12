-- lua5.1 stream/tests/test_wuui_leyline.lua [addon dir] [ov_leyline.lua]
-- Fake 3.3.5a API: loads every file of WrathUnleashedUI in .toc order, opens the Leyline window (the realm list in the
-- look of the game's Realm Selection) from Esc > Realm List, and runs a real round trip through ov_leyline.lua (fake
-- mod-ale): the WULY request, the chunked answer (v2: V and X lines), the columns (Realm Name, Type, Characters,
-- Population, Players · Bots), the population thresholds, the tabs, the search, the dropdown filters, the column sort,
-- favourites kept per account, an old (v1) realm still filling the window, Copy link, Okay (the hop: H:<id> to the
-- realm, echoed back, and the WUUI_LEYLINE_HOP global the page reads), the keyboard, Back to Wrath, Your Den, the
-- friends box, "Realm list unavailable", the in-game templates and textures, Forever UI's buttons, and no game-brand
-- names in anything the window shows.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local server = arg[2] or "realm/custom/lua_scripts/ov_leyline.lua"

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- fake widgets (as test_wuui_collections.lua)
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
function Frame:SetBackdrop(b) self.backdrop = b end
function Frame:SetTexture(t) self.tex = t end
function Frame:SetTextColor(r, g, b) self.color = { r, g, b } end
function Frame:SetFocus() self.focus = true end
function Frame:ClearFocus() self.focus = false end
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
function Frame:CreateFontString() return new("FontString", nil, self) end
function Frame:CreateTexture() return new("Texture", nil, self) end
function CreateFrame(kind, name, parent, template)
    local f = new(kind, name, parent); frames[#frames + 1] = f
    f.template = template
    if template == "OptionsSliderTemplate" and name then  -- the template's Low / High labels
        new("FontString", name .. "Low", f); new("FontString", name .. "High", f)
    end
    return f
end

UIParent = new("Frame", "UIParent"); UIParent.top = true
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
function SetCVar() end
function HasAction() return false end
function UnitLevel() return 80 end
function UnitClass() return "Mage", "MAGE" end
function UnitName() return "Tester" end
local realmName = "WrathUnleashed"
function GetRealmName() return realmName end
function PlaySound() end
function HideUIPanel(f) f:Hide() end
local clock = 100
function GetTime() return clock end

-- ---------------------------------------------------------------- the realm (ov_leyline.lua under fake mod-ale)
local tmp = os.tmpname()
local datFile = tmp .. ".dat"
local function writeDat(text) local f = assert(io.open(datFile, "wb")); f:write(text); f:close() end
writeDat('return { updated = 999000, realms = {\n'
    .. '  { id = "abcdefghijklmnop", name = "Frost Haven", ruleset = "dalaran", live = true, players = 7, bots = 2 },\n'
    .. '  { id = "qrstuvwxyz234567", name = "Quiet Corner", ruleset = "main", live = false, players = 0, bots = 0 },\n'
    .. '} }\n')
local realGetenv = os.getenv
os.getenv = function(k) if k == "OV_LEYLINE_FILE" then return datFile end return realGetenv(k) end
local serverClock = 1000000
os.time = function() return serverClock end
local onServerAddon
function RegisterServerEvent(id, fn) if id == 30 then onServerAddon = fn end end
function RegisterPlayerEvent() end
function GetPlayersInWorld() return { {}, {} } end
local function fireClient(prefix, msg, channel, sender)
    for _, f in ipairs(frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", prefix, msg, channel, sender) end
    end
end
local player = { GetGUIDLow = function() return 1 end }
function player:SendAddonMessage(prefix, msg) fireClient(prefix, msg, "WHISPER", "Tester") end
dofile(server)
os.getenv = realGetenv   -- os.time stays fake: the realm's 5 s rate limit runs on serverClock

local sent = {}
function SendAddonMessage(prefix, msg, channel, target)
    sent[#sent + 1] = { prefix, msg, channel, target }
    if target == UnitName("player") and onServerAddon then onServerAddon(30, player, 7, prefix, msg, player) end
end

-- ---------------------------------------------------------------- load the addon in .toc order
local files = {}
for line in io.lines(dir .. "/WrathUnleashedUI.toc") do
    if line:match("%.lua%s*$") then files[#files + 1] = line:gsub("%s+$", "") end
end
check(".toc lists Leyline.lua last", files[#files] == "Leyline.lua")
for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
local W = assert(WrathUnleashedUI)
local L = assert(W.Leyline, "W.Leyline")
check("builds nothing until opened", L.Window() == nil)
check("/leyline and /realms", SLASH_WULEYLINE1 == "/leyline" and SLASH_WULEYLINE2 == "/realms" and SlashCmdList.WULEYLINE)
check("one public base for links", L.PUBLIC_BASE == "https://leylinelauncher.com"
    and L.Link("abcdefghijklmnop") == "https://leylinelauncher.com/r/abcdefghijklmnop")

-- Esc > Realm List (its own button, independent of Mods & Workshop)
local realmBtn = _G.GameMenuButtonRealmList
check("the Esc menu has Realm List and keeps Mods & Workshop", realmBtn and realmBtn.text == "Realm List"
    and GameMenuButtonAddOns and GameMenuButtonAddOns.text == "Mods & Workshop" and realmBtn.parent == GameMenuFrame)
GameMenuButtonAddOns:Fire("OnClick")
local mods = _G.WrathUnleashedModsFrame
local dup
for _, f in ipairs(created) do if f.parent == mods and f.text == "Leyline realms" then dup = f end end
check("Mods & Workshop no longer has its own Leyline realms button", mods and mods:IsShown() and not dup)
mods:Hide()
GameMenuFrame:Show()
realmBtn:Fire("OnClick")
local win = L.Window()
check("Realm List opens the Leyline window and closes the game menu", win and win:IsShown() and not GameMenuFrame:IsShown())
realmBtn:Fire("OnClick")
check("Realm List again keeps it open (it shows, never toggles off)", win:IsShown())
check("Esc closes it", UISpecialFrames[#UISpecialFrames] == "WrathUnleashedLeylineFrame")
check("asked the realm once (WULY L, whispered to self)", #sent == 1 and sent[1][1] == "WULY" and sent[1][2] == "L"
    and sent[1][3] == "WHISPER" and sent[1][4] == "Tester")

local function shown()
    local out = {}
    for _, r in ipairs(win.rows) do if r:IsShown() then out[#out + 1] = r end end
    return out
end
local function list(lines)
    for _, m in ipairs(lines) do fireClient("WULY", m, "WHISPER", "Tester") end
end
local function ids()
    local out = {}
    for _, r in ipairs(L.realms) do out[#out + 1] = r.id end
    return table.concat(out, " ")
end

-- ---------------------------------------------------------------- the look: the game's own Realm Selection
check("the dialog box: the gold border on the dialog background", win.backdrop
    and win.backdrop.edgeFile == "Interface\\DialogFrame\\UI-DialogBox-Border"
    and win.backdrop.bgFile == "Interface\\DialogFrame\\UI-DialogBox-Background")
check("the title: Realm Selection on the dialog header", win.title and win.title:GetText() == "Realm Selection" and (function()
    for _, f in ipairs(created) do if f.parent == win and f.tex == "Interface\\DialogFrame\\UI-DialogBox-Header" then return true end end
end)())
local heads = {}
for i, h in ipairs(win.headers) do heads[i] = h:GetText() end
check("the column headers, as the original plus Players · Bots", table.concat(heads, "|") == "|Realm Name|Type|Characters|Population|Players · Bots"
    and win.headers[2].template == "WhoFrameColumnHeaderTemplate")
check("rows light up with the quest log's highlight; the selection with its title highlight",
    win.rows[1].sel.tex == "Interface\\QuestFrame\\UI-QuestLogTitleHighlight")
local tabs = {}
for i, t in ipairs(win.tabs) do tabs[i] = t:GetText() end
check("the realm categories as tabs along the bottom", table.concat(tabs, "|") == "All|Official|Community|Single player & Den|Friends' worlds|Favourites"
    and win.tabs[1].template == "CharacterFrameTabButtonTemplate" and win.tabs[1]:GetName() == "WrathUnleashedLeylineFrameTab1")
check("the original's Okay and Cancel, and Leyline's Copy link, Host your own and home", win.okay:GetText() == "Okay"
    and win.cancel:GetText() == "Cancel" and win.copyBtn:GetText() == "Copy link" and win.host:GetText() == "Host your own"
    and win.back:GetText() == "Back to Wrath" and win.okay.template == "UIPanelButtonTemplate")
check("search box and the filters dropdown", win.search.template == "InputBoxTemplate" and win.filterBtn:GetText() == "Filters"
    and win.filterMenu.template == "UIDropDownMenuTemplate")

-- ---------------------------------------------------------------- the rows
local rows = shown()
check("three realms, this one first, marked the current realm", #rows == 3 and rows[1].realm.id == "this"
    and rows[1].name:GetText():find("^WrathUnleashed") and rows[1].name:GetText():find("current realm"))
check("columns: name, type, characters, population, players · bots", rows[2].name:GetText() == "Frost Haven"
    and rows[2].type:GetText() == "Hangout" and rows[2].chars:GetText() == "-" and rows[2].pop:GetText() == "Low"
    and rows[2].players:GetText() == "7 · 2")
check("population coloured: Low green", rows[2].pop.color[1] < 0.2 and rows[2].pop.color[2] == 1)
check("offline realm: Offline, greyed, last", rows[3].pop:GetText() == "Offline" and rows[3].players:GetText() == "-"
    and rows[3].type:GetText() == "Normal" and rows[3].name.color[1] == 0.5)
check("v2: this realm's kind from the realm; characters - (no account in the fake realm)", L.proto == 2
    and rows[1].realm.kind == "official" and rows[1].chars:GetText() == "-")
check("the current realm is picked when the list comes", L.selected and L.selected.id == "this" and rows[1].sel:IsShown())
check("status: the network line (this realm + the live ones)", win.status:GetText():find("^2 realms · 9 players online · 2 bots"))
check("updated N ago, from the list's age", win.updated:GetText() == "Updated 16 min ago")
check("the tooltip's words: ruleset, state", select(2, L.Texts(rows[2].realm)) == "Dalaran hangout"
    and select(3, L.Texts(rows[2].realm)):find("online"))

rows[2]:Fire("OnClick")
check("a click picks the row", L.selected == rows[2].realm and rows[2].sel:IsShown() and not rows[1].sel:IsShown())
win.copyBtn:Fire("OnClick")
check("Copy link fills the box with /r/<id>", win.copy:GetText() == "https://leylinelauncher.com/r/abcdefghijklmnop")
win.copy:SetText("typed over"); win.copy:Fire("OnTextChanged")
check("the box can't be typed over", win.copy:GetText() == "https://leylinelauncher.com/r/abcdefghijklmnop")
local before = #sent
rows[3]:Fire("OnClick"); win.okay:Fire("OnClick")
check("Okay on an offline realm: says so, no hop", win.info:GetText():find("^Quiet Corner is offline")
    and win.copy:GetText() == "https://leylinelauncher.com/r/qrstuvwxyz234567" and #sent == before and WUUI_LEYLINE_HOP == nil
    and L.hopping == nil)
rows[1]:Fire("OnClick"); win.okay:Fire("OnClick")
check("Okay on this realm: you're here", win.info:GetText():find("^You're on WrathUnleashed")
    and win.copy:GetText() == "https://leylinelauncher.com/wrath")

local function says(parent, pat)
    for _, f in ipairs(created) do
        if f.parent == parent and f.kind == "FontString" and (f.text or ""):find(pat) then return true end
    end
end
check("Friends across realms: Leyline friends (Social.lua) take the box", says(win.friends, "^Friends across realms$")
    and not says(win.friends, "^Coming$") and win.friends.list and win.friends.add.enabled == true
    and win.friends.add:GetText() == "Friends" and W.Social.adopted == win.friends)
check("your game folder: the game's own button here", win.folder and win.folder.button
    and win.folder.button.template == "UIPanelButtonTemplate")

-- reopening soon after doesn't ask again; Refresh does
win:Hide(); win:Show()
check("reopening within 30 s doesn't ask again", #sent == 1)
win.refresh:Fire("OnClick")
check("Refresh asks again (the realm rate-limits to 5 s)", #sent == 2)

-- ---------------------------------------------------------------- the hop
local echoes = {}
local listener
for _, f in ipairs(frames) do if f.events.CHAT_MSG_ADDON then listener = f end end
local realOnEvent = listener:GetScript("OnEvent")
listener:SetScript("OnEvent", function(self, ev, prefix, msg, ...)    -- what the page sees (CHAT_MSG_ADDON)
    if prefix == "WULY" and msg:sub(1, 2) == "H:" then echoes[#echoes + 1] = msg end
    return realOnEvent(self, ev, prefix, msg, ...)
end)
rows = shown()
rows[2]:Fire("OnClick"); win.okay:Fire("OnClick")
local last = sent[#sent]
check("Okay on a live realm: H:<id> whispered to self", last[1] == "WULY" and last[2] == "H:abcdefghijklmnop"
    and last[3] == "WHISPER" and last[4] == "Tester")
check("Okay: the realm echoes it back (the page's cue)", echoes[1] == "H:abcdefghijklmnop" and L.hopping and L.hopping.echoed)
check("Okay: the page's fallback global is set", WUUI_LEYLINE_HOP == "abcdefghijklmnop")
check("Okay: says it's hopping, and the account step", win.info:GetText():find("^Hopping to Frost Haven")
    and win.info:GetText():find("make an account"))
check("Okay still puts the link in the box", win.copy:GetText() == "https://leylinelauncher.com/r/abcdefghijklmnop")
check("the realm drops a malformed hop", (function()
    local n = #echoes
    serverClock = serverClock + 5
    SendAddonMessage("WULY", "H:../../etc", "WHISPER", "Tester")
    SendAddonMessage("WULY", "H:ABCDEFGHIJKLMNOP", "WHISPER", "Tester")
    return #echoes == n end)())
check("the addon refuses a malformed hop", L.Hop("not-a-realm") == false and L.Hop("ABCDEFGHIJKLMNOP") == false
    and WUUI_LEYLINE_HOP == "abcdefghijklmnop")

-- no page took it (the desktop client): after HOP_WAIT the window says so and clears the request
clock = clock + L.HOP_WAIT + 1
win:Fire("OnUpdate", 0.1)
check("no page: the hop is dropped and the window says why", L.hopping == nil and WUUI_LEYLINE_HOP == nil
    and win.info:GetText():find("^Hopping works in the browser game"))
serverClock = serverClock + 5
rows[2]:Fire("OnDoubleClick")
check("double-click a row = Okay (the same H:<id>)", sent[#sent][2] == "H:abcdefghijklmnop" and WUUI_LEYLINE_HOP == "abcdefghijklmnop")
WUUI_LEYLINE_HOP, L.hopping = nil, nil

-- Back to Wrath: always there; on Wrath it says so, on a community realm it hops home
before = #sent
win.back:Fire("OnClick")
check("Back to Wrath on Wrath: you're here, no hop", win.info:GetText() == "You're on Wrath now." and #sent == before
    and WUUI_LEYLINE_HOP == nil)
realmName = "Frost Haven"
serverClock = serverClock + 5
win.back:Fire("OnClick")
check("Back to Wrath on a community realm: H:wrath", sent[#sent][2] == "H:wrath" and WUUI_LEYLINE_HOP == "wrath"
    and echoes[#echoes] == "H:wrath" and win.info:GetText():find("^Back to Wrath"))
WUUI_LEYLINE_HOP, L.hopping, realmName = nil, nil, "WrathUnleashed"
listener:SetScript("OnEvent", realOnEvent)

-- the client ignores messages that aren't the realm's answer to us
fireClient("WULY", "B:1:0", "WHISPER", "Someone")
fireClient("WULY", "R:zzzz:1:1:1:main:Spoof", "WHISPER", "Someone")
fireClient("WULY", "E:1", "WHISPER", "Someone")
check("ignores other senders", #shown() == 3)
fireClient("WULY", "R:abcdefghijklmnop:1:1:1:main:Stray", "WHISPER", "Tester")
fireClient("WULY", "X:this:den:9", "WHISPER", "Tester")
check("ignores rows and kinds outside a list", #shown() == 3 and L.realms[1].kind == "official")

-- ---------------------------------------------------------------- the keyboard
L.selected = nil
L.OnKey("DOWN")
check("Down picks the first row, Down again the next", L.selected == L.realms[1] and L.OnKey("DOWN") and L.selected == L.realms[2])
win:Fire("OnKeyDown", "UP")
check("Up moves back", L.selected == L.realms[1])
win:Fire("OnKeyDown", "ENTER")
check("Enter = Okay", win.info:GetText():find("^You're on WrathUnleashed"))
win:Fire("OnChar", "q")
check("typing goes to the search", win.search:GetText() == "q" and win.search.focus == true and L.query == "q"
    and ids() == "qrstuvwxyz234567")
win.search:SetText(""); win.search:Fire("OnTextChanged")
check("clearing the search shows every realm again", #L.realms == 3)
win:Fire("OnKeyDown", "ESCAPE")
check("Esc = Cancel", not win:IsShown())
win:Show()
win.cancel:Fire("OnClick")
check("Cancel closes it", not win:IsShown())
win:Show()

-- missing file on the realm: Realm list unavailable
os.remove(datFile)
OV_LEYLINE_TEST.cache.at = -1e9
serverClock = serverClock + 10
win.refresh:Fire("OnClick")
check("missing file: Realm list unavailable", win.status:GetText() == "Realm list unavailable" and #shown() == 0)

-- no answer at all (no server script): unavailable after a few seconds
onServerAddon = nil
win.refresh:Fire("OnClick")
check("waiting...", win.status:GetText():find("^Asking"))
clock = clock + 10
win:Fire("OnUpdate", 0.1)
check("no answer: Realm list unavailable", win.status:GetText() == "Realm list unavailable")

-- many realms: paging and the mouse wheel
local msgs = { "B:20:0" }
for i = 1, 20 do msgs[#msgs + 1] = string.format("R:%s:1:%d:0:casual:Realm %d", string.rep(string.char(96 + i), 16), i, i) end
msgs[#msgs + 1] = "E:20"
list(msgs)
check("a page of 14 of 20", #shown() == 14 and win.count:GetText() == "1-14 of 20")
check("v1 realm (no V line): protocol 1", L.proto == 1)
win.list:Fire("OnMouseWheel", -1)
check("mouse wheel scrolls", shown()[1].name:GetText() == "Realm 2")
L.selected = L.realms[14]; L.offset = 0
L.OnKey("DOWN")
check("Down past the page keeps the selection in view", L.selected == L.realms[15] and L.offset == 1)

-- ---------------------------------------------------------------- population
local function pop(r) local t, red, g = L.Population(r); return t .. ":" .. (red > 0.5 and "r" or "") .. (g > 0.5 and "g" or "") end
check("population: Low green under 10 players", pop({ live = true, players = 0 }) == "Low:g" and pop({ live = true, players = 9 }) == "Low:g")
check("population: Medium yellow from 10", pop({ live = true, players = 10 }) == "Medium:rg" and pop({ live = true, players = 49 }) == "Medium:rg")
check("population: High red from 50", pop({ live = true, players = 50 }) == "High:r")
check("population: against the realm's maximum", pop({ live = true, players = 24, maxp = 100 }) == "Low:g"
    and pop({ live = true, players = 25, maxp = 100 }) == "Medium:rg" and pop({ live = true, players = 60, maxp = 100 }) == "High:r"
    and pop({ live = true, players = 100, maxp = 100 }) == "Full:r")
check("population: bots don't count, offline grey, unknown -", pop({ live = true, players = 2, bots = 80 }) == "Low:g"
    and L.Population({ live = false, players = 5 }) == "Offline" and L.Population({ live = true }) == "-")

-- ---------------------------------------------------------------- the network fields: order, badges, filters
list({ "B:6:0", "R:this:1:3:0:main:WrathUnleashed", "M:this:S:0:-:12340:-:-:-:hangout",
    "R:cccccccccccccccc:1:9:0:main:Plain", "M:cccccccccccccccc:-:30:88:12340:na-east:en:-:pvp",
    "R:dddddddddddddddd:1:0:0:main:Picked", "M:dddddddddddddddd:FV:12:97:5875:eu:de:100:pvp,rp",
    "R:eeeeeeeeeeeeeeee:1:2:0:dalaran:Hangout relay", "M:eeeeeeeeeeeeeeee:S:50:-:12340:-:-:-:hangout",
    "R:ffffffffffffffff:1:0:0:custom:Far Away", "M:ffffffffffffffff:NE:5:-:12340:oce:-:-:",
    "R:gggggggggggggggg:0:0:0:main:Sleepy", "M:gggggggggggggggg:-:0:40:12340:eu:-:-:rp",
    "N:4:14:0", "E:6" })
check("order: featured, this realm (the sample), then by score, offline last; its relay record not twice",
    ids() == "dddddddddddddddd this cccccccccccccccc ffffffffffffffff gggggggggggggggg")
rows = shown()
local n1, r1, s1 = L.Texts(rows[1].realm)
check("featured row: the name column plain, the badges and the rules in its tooltip", rows[1].name:GetText() == "Picked"
    and n1:find("Featured") and n1:find("Verified") and r1 == "Level 1 or boost to 80 · PvP, RP, 1.12.1, Europe, de"
    and s1:find("97%% up"))
check("Type: RP PvP from the tags, PvP, the ruleset otherwise", rows[1].type:GetText() == "RP PvP" and rows[3].type:GetText() == "PvP"
    and rows[4].type:GetText() == "Custom")
check("the sample: Leyline Sample Hangout · temporary (current realm)", rows[2].name:GetText():find("^Leyline Sample Hangout · temporary")
    and rows[2].name:GetText():find("current realm"))
check("estimated region marked, new badge", select(2, L.Texts(rows[4].realm)):find("Oceania%?") and L.Texts(rows[4].realm):find("New"))
check("status: the network line", win.status:GetText():find("^4 realms · 14 players online"))
check("no Den of the week: none said", not win.status:GetText():find("Den of the week"))
list({ "B:1:0", "R:this:1:3:0:main:WrathUnleashed", "N:1:3:0", "W:Mossy Hollow:Ana:A pond and lanterns", "E:1" })
check("Den of the week: said after the network line", win.status:GetText():find("Den of the week: Mossy Hollow by Ana%.$") ~= nil
    and L.denWeek and L.denWeek.line == "A pond and lanterns")
list({ "B:1:0", "R:this:1:3:0:main:WrathUnleashed", "N:1:3:0", "W:broken", "E:1" })
check("a broken W line: none said", not win.status:GetText():find("Den of the week") and L.denWeek == nil)
local NET = { "B:6:0", "R:this:1:3:0:main:WrathUnleashed", "M:this:S:0:-:12340:-:-:-:hangout",
    "R:cccccccccccccccc:1:9:0:main:Plain", "M:cccccccccccccccc:-:30:88:12340:na-east:en:-:pvp",
    "R:dddddddddddddddd:1:0:0:main:Picked", "M:dddddddddddddddd:FV:12:97:5875:eu:de:100:pvp,rp",
    "R:eeeeeeeeeeeeeeee:1:2:0:dalaran:Hangout relay", "M:eeeeeeeeeeeeeeee:S:50:-:12340:-:-:-:hangout",
    "R:ffffffffffffffff:1:0:0:custom:Far Away", "M:ffffffffffffffff:NE:5:-:12340:oce:-:-:",
    "R:gggggggggggggggg:0:0:0:main:Sleepy", "M:gggggggggggggggg:-:0:40:12340:eu:-:-:rp",
    "N:4:14:0", "E:6" }
list(NET)
L.Cycle("tag")
check("filter Rules: PvP (the first tag any realm has)", L.filter.tag == "pvp" and #L.realms == 3
    and L.realms[2].id == "this" and win.filterBtn:GetText() == "Filters (1)")
L.Cycle("tag"); L.Cycle("tag"); L.Cycle("tag")
check("filter cycles back to any", L.filter.tag == nil and win.filterBtn:GetText() == "Filters")
L.Cycle("region")
check("filter Region: Europe", L.filter.region == "eu" and #L.realms == 3)
L.Cycle("region"); L.Cycle("region"); L.Cycle("region")
L.Cycle("build")
check("filter Build: 1.12.1", L.filter.build == 5875 and #L.realms == 2)
L.filter = {}
L.Cycle("now")
check("filter Has players: only realms with people now (the sample stays)", L.filter.now == true
    and #L.realms == 2 and L.realms[1].id == "this" and L.realms[2].id == "cccccccccccccccc")
L.Cycle("now")
check("and back to everyone", #L.realms == 5)
-- the dropdown's lines
local menu = L.MenuItems(1)
local mt = {}
for _, it in ipairs(menu) do mt[#mt + 1] = it.text end
check("the filters dropdown: Build, Rules, Region, Language (submenus), Has players now, Clear",
    table.concat(mt, "|") == "Filters|Build: any|Rules: any|Region: any|Language: any|Has players now|Clear filters"
    and menu[3].hasArrow and menu[3].value == "tag")
local sub = L.MenuItems(2, "tag")
local st = {}
for _, it in ipairs(sub) do st[#st + 1] = it.text end
check("a submenu: Any, then every value the list has", table.concat(st, "|") == "Any|PvP|RP|Hangout" and sub[1].checked == true)
sub[3].func()
check("picking a value filters", L.filter.tag == "rp" and #L.realms == 3 and L.MenuItems(1)[3].text == "Rules: RP")
L.MenuItems(1)[7].func()
check("Clear filters", next(L.filter) == nil and #L.realms == 5)
OPENED = nil
UIDropDownMenu_Initialize = function(frame, init, kind) frame.init, frame.kind = init, kind end
ToggleDropDownMenu = function(level, value, frame, anchor) OPENED = { frame = frame, anchor = anchor } end
UIDropDownMenu_AddButton = function(item, level) ADDED = ADDED or {}; ADDED[#ADDED + 1] = item.text end
win.filterBtn:Fire("OnClick")
win.filterMenu.init(win.filterMenu, 1)
check("the Filters button opens the game's dropdown under it", OPENED and OPENED.frame == win.filterMenu
    and OPENED.anchor == win.filterBtn and win.filterMenu.kind == "MENU" and ADDED and ADDED[2] == "Build: any")

-- ---------------------------------------------------------------- search
local function search(q) win.search:SetText(q); win.search:Fire("OnTextChanged") end
search("plain")
check("search: by name", ids() == "cccccccccccccccc")
search("europe")
check("search: by region", ids() == "dddddddddddddddd gggggggggggggggg")
search("1.12")
check("search: by build", ids() == "dddddddddddddddd")
search("rp")
check("search: by rules tag", ids() == "dddddddddddddddd gggggggggggggggg")
search("en")
check("search: by language (and anything else with those letters)", ids():find("cccccccccccccccc") ~= nil)
search("nothing like it")
check("search: nothing matches, the list says so", #L.realms == 0 and win.empty:GetText():find("^No realm matches"))
search("")
check("search cleared", #L.realms == 5 and win.empty:GetText() == "")

-- ---------------------------------------------------------------- sort
win.headers[2]:Fire("OnClick")
check("sort by Realm Name (as shown), offline still last", ids() == "ffffffffffffffff this dddddddddddddddd cccccccccccccccc gggggggggggggggg"
    and L.sort.key == "name" and not L.sort.desc and win.headers[2].arrow:IsShown())
win.headers[2]:Fire("OnClick")
check("again: the other way", ids() == "cccccccccccccccc dddddddddddddddd this ffffffffffffffff gggggggggggggggg" and L.sort.desc)
win.headers[2]:Fire("OnClick")
check("a third time: the network's order", ids() == "dddddddddddddddd this cccccccccccccccc ffffffffffffffff gggggggggggggggg"
    and L.sort.key == nil and not win.headers[2].arrow:IsShown())
win.headers[6]:Fire("OnClick")
check("sort by Players · Bots: most first", ids() == "cccccccccccccccc this dddddddddddddddd ffffffffffffffff gggggggggggggggg")
win.headers[5]:Fire("OnClick")
check("sort by Population", L.sort.key == "pop" and L.realms[#L.realms].id == "gggggggggggggggg")
win.headers[3]:Fire("OnClick")
check("sort by Type", L.sort.key == "type" and L.realms[1].id == "ffffffffffffffff")
L.sort = {}; L.Apply(); L.Refresh()

-- ---------------------------------------------------------------- tabs
list({ "B:4:0", "R:this:1:3:0:main:Mira's place", "M:this:-:0:-:12340:-:-:-:",
    "R:vrvrvrvrvrvrvrvr:1:20:0:main:Vanilla Remastered · Official", "M:vrvrvrvrvrvrvrvr:O:0:99:5875:-:-:-:",
    "R:cccccccccccccccc:1:9:0:main:Plain", "M:cccccccccccccccc:-:30:88:12340:na-east:en:-:pvp",
    "R:hhhhhhhhhhhhhhhh:1:1:0:main:Hollow", "M:hhhhhhhhhhhhhhhh:-:3:-:12340:-:-:-:",
    "V:2", "X:this:local:4", "N:4:33:0", "E:4" })
check("v2: this realm is your Single player world, 4 characters", L.realms[1].kind == "local" and shown()[1].chars:GetText() == "4"
    and shown()[1].type:GetText() == "Single player")
win.tabs[2]:Fire("OnClick")
check("Official: realms flagged O", L.tab == "official" and ids() == "vrvrvrvrvrvrvrvr" and win.tabs[2].on)
win.tabs[3]:Fire("OnClick")
check("Community: the rest", ids() == "cccccccccccccccc hhhhhhhhhhhhhhhh")
win.tabs[4]:Fire("OnClick")
check("Single player & Den: this world", ids() == "this")
W.Social.rows = { { status = "g", name = "Ana", where = "Tolan on Plain" }, { status = "l", name = "Bo", where = "In the launcher" },
    { status = "g", name = "Cy", where = "Cyra on mira's  place" } }
win.tabs[5]:Fire("OnClick")
check("Friends' worlds: the realms your friends are in (the friends list's where)", ids() == "this cccccccccccccccc"
    and table.concat(L.FriendsOn(L.realms[2]), ",") == "Ana")
W.Social.rows = {}
L.SetTab("friends")
check("Friends' worlds: none in a world, the list says so", #L.realms == 0 and win.empty:GetText():find("None of your Leyline friends"))
win.tabs[6]:Fire("OnClick")
check("Favourites: empty, says how", #L.realms == 0 and win.empty:GetText():find("Click the star"))
L.SetTab("all")
shown()[3].star:Fire("OnClick")
check("the star keeps a realm", L.IsFav(L.realms[3]) and WrathUnleashedUIDB.leyline.fav.cccccccccccccccc == true)
L.SetTab("fav")
check("Favourites: the starred realm", ids() == "cccccccccccccccc")
list({ "B:2:0", "R:this:1:3:0:main:Mira's place", "R:cccccccccccccccc:1:9:0:main:Plain", "E:2" })
check("favourites survive a new list (saved per account)", ids() == "cccccccccccccccc")
shown()[1].star:Fire("OnClick")
check("and the star again lets it go", #L.realms == 0 and WrathUnleashedUIDB.leyline.fav.cccccccccccccccc == nil)
L.SetTab("all")

-- ---------------------------------------------------------------- an old (v1) realm still fills the window
check("v1 list: no V line, protocol 1; this realm still listed", L.proto == 1 and L.realms[1].id == "this")
check("v1 list: your characters remembered from the last time this realm said", shown()[1].chars:GetText() == "4")
check("v1 list: this realm's kind from where you are (not home: community)", (function()
    realmName = "Mira's place"
    local k = L.Kind(L.realms[1])
    realmName = "WrathUnleashed"
    return k == "community" and L.Kind(L.realms[1]) == "official" end)())

-- ---------------------------------------------------------------- Your Den (no realm list there)
realmName = "Your Den"
list({ "U:Realm list unavailable" })
check("Your Den: one row, this world, a Den", #L.realms == 1 and L.realms[1].name == "Your Den" and shown()[1].type:GetText() == "Den"
    and L.Kind(L.realms[1]) == "den")
realmName = "WrathUnleashed"
list({ "B:1:0", "R:this:1:3:0:main:WrathUnleashed", "E:1" })
L.SetTab("mine")
local mine = {}
for _, r in ipairs(L.realms) do mine[#mine + 1] = r.name end
check("Single player & Den: your own worlds this client has seen", table.concat(mine, ",") == "Mira's place,Your Den"
    and shown()[1].pop:GetText() == "On this PC")
before = #sent
shown()[2]:Fire("OnClick")
win.okay:Fire("OnClick")
check("Okay on one of them: start it from the launcher, no hop", #sent == before and win.info:GetText():find("Leyline launcher")
    and win.okay.enabled == false)
check("and it has no link", L.CopySelected() == false)
L.SetTab("all")

-- Host your own and Report
list(NET)
win.host:Fire("OnClick")
check("Host your own: opens the Host window (Host.lua), the realm window steps aside",
    _G.WrathUnleashedHostFrame and _G.WrathUnleashedHostFrame:IsShown() and not win:IsShown() and W.Host.tab == 1)
_G.WrathUnleashedHostFrame:Hide(); win:Show()
L.selected = nil
win.report:Fire("OnClick")
check("Report with nothing picked: says how", win.info:GetText():find("^Pick a community realm"))
shown()[3]:Fire("OnClick")
win.report:Fire("OnClick")
check("Report: the website's report link for the picked realm", win.copy:GetText() ==
    "https://leylinelauncher.com/wrath/realms/report?realm=cccccccccccccccc" and win.info:GetText():find("^Open this link to report Plain"))
check("no Report for the sample", L.Report(L.realms[2]) == false)
check("a malformed M line is ignored", (function()
    list({ "B:1:0", "R:hhhhhhhhhhhhhhhh:1:1:0:main:X", "M:hhhhhhhhhhhhhhhh:<script>:1:1:1:x:y:z:w", "E:1" })
    return #L.realms == 1 and not L.realms[1].featured and L.realms[1].build == nil end)())
L.filter = {}

-- Forever UI: its red three-slice buttons (the details panel is made now, so it takes them)
ForeverUI = { Templates = { ThreeSliceButton = function(b, tex) b.foreverThreeSlice = tex end } }
L.Preview(L.realms[1])
check("with Forever UI: its red buttons", win.panel.join.foreverThreeSlice == "128-redbutton" and L.Forever())
ForeverUI = nil
win.panel:Hide()

-- this realm's own relay listing is the same realm: one row (by the realm's S line, or by name from an older realm)
list({ "B:3:0", "R:this:1:4:0:main:Wrath Unleashed", "M:this:-:0:-:12340:-:-:-:",
    "R:kkkkkkkkkkkkkkkk:1:4:0:dalaran:Wrath Unleashed", "M:kkkkkkkkkkkkkkkk:N:40:-:12340:-:-:-:hangout",
    "R:cccccccccccccccc:1:9:0:main:Plain", "M:cccccccccccccccc:-:30:88:12340:na-east:en:-:pvp", "E:3" })
check("by name: Wrath Unleashed (current realm) once, its listing not a second row",
    ids() == "this cccccccccccccccc" and L.realms[1].relayId == "kkkkkkkkkkkkkkkk")
check("this row: You're here in the browser game (a page)", (function()
    WUUI_GAMEFOLDER = { page = true }
    local t = L.JoinText(shown()[1].realm)
    WUUI_GAMEFOLDER = nil
    return t == "You're here" end)())
check("this row: Join by link in the desktop client (listed, live, no page)", L.JoinText(shown()[1].realm) == "Join by link")
shown()[1]:Fire("OnClick"); win.copyBtn:Fire("OnClick")
check("desktop: Copy link gives the relay link", win.copy:GetText() == "https://leylinelauncher.com/r/kkkkkkkkkkkkkkkk")
list({ "B:3:0", "R:this:1:4:0:main:Wrath Unleashed", "M:this:-:0:-:12340:-:-:-:",
    "R:kkkkkkkkkkkkkkkk:0:0:0:dalaran:Wrath Unleashed", "M:kkkkkkkkkkkkkkkk:N:40:-:12340:-:-:-:hangout", "E:3" })
check("listing offline: no Join by link", #L.realms == 1 and L.JoinText(shown()[1].realm) == "You're here")
list({ "B:3:0", "R:this:1:4:0:main:Wrath Unleashed", "M:this:-:0:-:12340:-:-:-:", "S:mmmmmmmmmmmmmmmm:1",
    "R:kkkkkkkkkkkkkkkk:1:4:0:dalaran:Wrath Unleashed", "M:kkkkkkkkkkkkkkkk:N:40:-:12340:-:-:-:hangout", "E:3" })
check("the realm's S line wins: only that id is this realm (a same-name realm elsewhere stays)",
    #L.realms == 2 and L.realms[1].relayId == "mmmmmmmmmmmmmmmm" and L.realms[2].id == "kkkkkkkkkkkkkkkk")
check("names match without case, spaces or punctuation", L.SameName("Dalaran  hangout!", "dalaran hangout")
    and not L.SameName("Dalaran", "Wrath Unleashed") and not L.SameName("", ""))

-- nothing the window shows names the game or its maker
local bad
for _, f in ipairs(created) do
    local t = (f.kind == "FontString" or f.kind == "Button") and f.text or nil
    if t and (t:lower():find("warcraft") or t:lower():find("%f[%w]wow%f[%W]") or t:lower():find("lich king")
        or t:lower():find("blizzard") or t:find("Open ?Vanilla") or t:find("OpenWoW")) then bad = t end
end
check("no game-brand or old project names on screen", bad == nil)
if bad then print("  shown: " .. bad) end

os.remove(tmp)
print(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
