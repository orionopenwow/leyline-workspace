-- lua5.1 stream/tests/test_wuui_leyline_browse.lua [addon dir] [list chunk file] [preview chunk file] [--dump out.json]
-- The in-game realm browser (WrathUnleashedUI/Leyline.lua with the page's list, wrath-page.js leyline-browse): with
-- the page there (WUUI_LEYLINE_BROWSE = 1) the window asks the page, not the realm; the rows, search, the filters
-- (Plays in browser, Own files, Has players, Version), paging, the preview beside the window (rules, who's on only
-- when shared, mods required / optional / on your side, version chip, uptime, Hero Key), Join from the preview (a hop),
-- a realm needing another game version handed to the launcher (never a hop), offline, the page not answering (back
-- to the realm's list), the desktop client's own version check, and no game-brand words or version numbers in prose.
-- With the chunk files (written by stream/tests/browse.page.test.cjs from the real wrath-page.js), the page's own
-- answers are fed in instead of the ones below. --dump writes every shown widget's geometry for the screenshot
-- harness (the window and the preview drawn in a browser).
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local listFile, previewFile, dumpFile
do
    local rest = {}
    local i = 2
    while arg[i] do
        if arg[i] == "--dump" then dumpFile = arg[i + 1]; i = i + 2 else rest[#rest + 1] = arg[i]; i = i + 1 end
    end
    listFile, previewFile = rest[1], rest[2]
end

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

-- ---------------------------------------------------------------- fake widgets that keep their geometry
local created = {}
local Frame = {}
Frame.__index = function(self, k)
    local v = rawget(Frame, k)
    if v ~= nil then return v end
    if type(k) == "string" and k:match("^%u") then return function() end end
    return nil
end
function Frame:SetScript(n, fn) self.scripts[n] = fn end
function Frame:GetScript(n) return self.scripts[n] end
function Frame:HookScript(n, fn) local old = self.scripts[n]; self.scripts[n] = function(...) if old then old(...) end fn(...) end end
function Frame:Fire(n, ...) local fn = self.scripts[n]; if fn then return fn(self, ...) end end
function Frame:Show() local was = self:IsVisible(); self.shown = true; if not was and self:IsVisible() then self:Fire("OnShow") end end
function Frame:Hide() local was = self:IsVisible(); self.shown = false; if was then self:Fire("OnHide") end end
function Frame:IsShown() return self.shown end
function Frame:IsVisible()
    if not self.shown then return false end
    return self.parent == nil or self.parent.top or self.parent:IsVisible()
end
function Frame:SetText(t) self.text = t end
function Frame:GetText() return self.text end
function Frame:Enable() self.enabled = true end
function Frame:Disable() self.enabled = false end
function Frame:IsEnabled() return self.enabled ~= false end
function Frame:RegisterEvent(e) self.events[e] = true end
function Frame:GetName() return self.name end
function Frame:GetParent() return self.parent end
function Frame:GetChildren() local out = {} for _, f in ipairs(created) do if f.parent == self and f.kind ~= "FontString" and f.kind ~= "Texture" then out[#out + 1] = f end end return unpack(out) end
function Frame:SetWidth(w) self.w = w end
function Frame:SetHeight(h) self.h = h end
function Frame:GetWidth() return self.w or 0 end
function Frame:GetHeight() return self.h or 0 end
function Frame:SetSize(w, h) self.w, self.h = w, h end
function Frame:SetPoint(p, a, b, c, d)
    local rel, rp, x, y
    if type(a) == "table" then rel, rp, x, y = a, b, c, d
    elseif type(a) == "string" and type(b) == "string" then rel, rp, x, y = _G[a], b, c, d   -- by name
    elseif type(a) == "string" then rel, rp, x, y = nil, a, b, c
    else rel, rp, x, y = nil, p, a, b end
    self.points = self.points or {}
    for i, q in ipairs(self.points) do if q.p == p then table.remove(self.points, i) break end end
    self.points[#self.points + 1] = { p = p, rel = rel, rp = rp or p, x = x or 0, y = y or 0 }
end
function Frame:ClearAllPoints() self.points = {} end
function Frame:SetAllPoints(rel) self.points = { { p = "TOPLEFT", rel = rel, rp = "TOPLEFT", x = 0, y = 0 }, { p = "BOTTOMRIGHT", rel = rel, rp = "BOTTOMRIGHT", x = 0, y = 0 } } end
function Frame:SetTexture(r, g, b, a) if type(r) == "number" then self.color = { r, g, b, a or 1 } else self.file = r end end
function Frame:SetVertexColor(r, g, b, a) self.vcolor = { r, g, b, a or 1 } end
function Frame:SetTextColor(r, g, b, a) self.tcolor = { r, g, b, a or 1 } end
function Frame:GetTextColor() local c = self.tcolor or { 1, 1, 1, 1 } return c[1], c[2], c[3], c[4] end
function Frame:SetBackdrop(b) self.backdrop = b end
function Frame:SetBackdropColor(r, g, b, a) self.bdcolor = { r, g, b, a or 1 } end
function Frame:SetBackdropBorderColor(r, g, b, a) self.bdborder = { r, g, b, a or 1 } end
function Frame:SetJustifyH(j) self.justify = j end
function Frame:SetFontString(fs) self.fontString = fs end
function Frame:GetFontString() return self.fontString end
function Frame:GetStringWidth() return #(self.text or "") * 6 end
function Frame:SetFrameStrata(s) self.strata = s end
function Frame:SetAlpha(a) self.alpha = a end
function Frame:SetMaxLetters(n) self.maxLetters = n end
local function new(kind, name, parent, font, layer)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, events = {}, shown = true, font = font, layer = layer,
        n = #created + 1 }, Frame)
    if name then _G[name] = f end
    created[#created + 1] = f
    return f
end
function Frame:CreateFontString(_, layer, font) return new("FontString", nil, self, font, layer) end
function Frame:CreateTexture(_, layer) return new("Texture", nil, self, nil, layer) end
function CreateFrame(kind, name, parent) return new(kind, name, parent) end

UIParent = new("Frame", "UIParent"); UIParent.top = true; UIParent.w, UIParent.h = 1229, 768
function UIParent:GetScale() return 1 end
GameMenuFrame = new("Frame", "GameMenuFrame"); GameMenuFrame.top = true; GameMenuFrame.h = 300
UISpecialFrames = {}
function tinsert(t, v) table.insert(t, v) end
SlashCmdList = {}
DEFAULT_CHAT_FRAME = { AddMessage = function() end }
function GetScreenHeight() return 768 end
function InCombatLockdown() return false end
function SetBinding() return 1 end
function SetBindingClick() return 1 end
function SaveBindings() end
function SetCVar() end
function HasAction() return false end
function UnitLevel() return 80 end
function UnitClass() return "Mage", "MAGE" end
function UnitName() return "Tester" end
function GetRealmName() return "WrathUnleashed" end
function PlaySound() end
function HideUIPanel(f) f:Hide() end
local clock = 100
function GetTime() return clock end
local build = "12340"
function GetBuildInfo() return "3.3.5", build, "Jun 24 2010", 30300 end
local sent = {}
function SendAddonMessage(prefix, msg, channel, target) sent[#sent + 1] = { prefix, msg, channel, target } end

-- ---------------------------------------------------------------- load the addon in .toc order
for line in io.lines(dir .. "/WrathUnleashedUI.toc") do
    if line:match("%.lua%s*$") then dofile(dir .. "/" .. line:gsub("%s+$", "")) end
end
local W = assert(WrathUnleashedUI)
local L = assert(W.Leyline)
check("the window answers the page's call (WUUI_LEYLINE_ONNET)", type(WUUI_LEYLINE_ONNET) == "function")

-- the page is there (wrath-page.js sets these on its polls)
WUUI_PAGE = true
WUUI_LEYLINE_BROWSE = 1
W.ToggleLeyline()
local win = L.Window()
check("opening asks the page for page 1, not the realm", WUUI_LEYLINE_ASK == "list|1||" and #sent == 0)
check("the browse filters and search show, the realm-list filters don't", win.bfilters.browser:IsVisible() and win.search:IsVisible()
    and not win.filters.tag:IsVisible() and not win.filters.build:IsVisible())

-- the page's answer (wrath-page.js listLua)
local A, B, C, D = "aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb", "cccccccccccccccc", "dddddddddddddddd"
local function feed(file, fallback)
    WUUI_LEYLINE_ASK = nil
    if file then
        local f = assert(io.open(file, "rb")); local text = f:read("*a"); f:close()
        assert(loadstring(text))()
    else
        fallback()
        WUUI_LEYLINE_ONNET(WUUI_LEYLINE_PREVIEW_KIND or "list")
    end
end
feed(listFile, function()
    WUUI_LEYLINE_NET = { v = 1, page = 1, pages = 1, total = 4, all = 4, sso = true, mine = 12340,
        network = { realms = 3, players = 14, bots = 10 }, rows = {
        { id = "hhhhhhhhhhhhhhhh", name = "Wrath Unleashed", live = true, online = 4, bots = 10, build = 12340, chip = "3.3.5a", browser = true, own = false, here = true, mods = 4, desc = "" },
        { id = A, name = "Frost Haven", live = true, online = 7, bots = 0, build = 12340, chip = "3.3.5a", browser = true, own = false, hk = true, up = 99, featured = true, mods = 3, desc = "A cosy place" },
        { id = B, name = "Old Days of fun", live = true, online = 0, bots = 0, build = 5875, chip = "1.12.1", browser = false, own = true, mods = 0, desc = "" },
        { id = C, name = "Quiet Meadow", live = false, online = 0, bots = 0, build = 12340, chip = "3.3.5a", browser = true, own = false, mods = 0, desc = "" },
    } }
end)
local function shown()
    local out = {}
    for _, r in ipairs(win.rows) do if r:IsShown() then out[#out + 1] = r end end
    return out
end
local rows = shown()
check("four rows from the page, in its order", #rows == 4 and rows[2].realm.id == A and L.realms[3].id == B)
check("the network line", win.status:GetText() == "3 realms · 14 players online · 10 bots.")
check("this realm's row: you are here", rows[1].name:GetText():find("you are here") and rows[1].join:GetText() == "You're here")
check("a row: name, badges; chips line with the version chip, browser, Hero Key, mods", rows[2].name:GetText():find("^Frost Haven")
    and rows[2].name:GetText():find("Featured") and rows[2].rules:GetText() == "3.3.5a · Plays in browser · Hero Key · 3 mods"
    and rows[2].players:GetText() == "7 playing" and rows[2].state:GetText():find("99%% up"))
check("another version's row says own game files", rows[3].rules:GetText() == "1.12.1 · Own game files")
check("rows open the preview (View), not a hop", rows[2].join:GetText() == "View")

-- search: asked after a short pause, with the words
win.search:SetText("frost"); win.search:Fire("OnTextChanged")
win:Fire("OnUpdate", 0.1)
check("search waits for the typing to stop", WUUI_LEYLINE_ASK == nil)
clock = clock + 1; win:Fire("OnUpdate", 0.1)
check("then asks page 1 with the words", WUUI_LEYLINE_ASK == "list|1||frost")
win.search:SetText(""); win.search:Fire("OnTextChanged"); clock = clock + 1; win:Fire("OnUpdate", 0.1)
-- filters
win.bfilters.browser:Fire("OnClick")
check("Plays in browser", WUUI_LEYLINE_ASK == "list|1|b|" and win.bfilters.browser:GetText() == "In browser: on")
win.bfilters.players:Fire("OnClick")
check("and Has players", WUUI_LEYLINE_ASK == "list|1|b,p|")
win.bfilters.browser:Fire("OnClick"); win.bfilters.players:Fire("OnClick")
win.bfilters.own:Fire("OnClick")
check("Own files", WUUI_LEYLINE_ASK == "list|1|o|")
win.bfilters.own:Fire("OnClick")
win.bfilters.build:Fire("OnClick")
check("Version: the first one listed (a chip, not prose)", WUUI_LEYLINE_ASK == "list|1|v12340|" and win.bfilters.build:GetText() == "Version: 3.3.5a")
win.bfilters.build:Fire("OnClick")
check("Version: the next one", WUUI_LEYLINE_ASK == "list|1|v5875|")
win.bfilters.build:Fire("OnClick")
check("Version: back to any", WUUI_LEYLINE_ASK == "list|1||")

-- paging
WUUI_LEYLINE_NET = { page = 1, pages = 3, total = 45, network = { realms = 40, players = 9, bots = 0 }, rows = {} }
for i = 1, 20 do WUUI_LEYLINE_NET.rows[i] = { id = string.rep("a", 15) .. string.char(96 + (i % 26) + 1), name = "Realm " .. i, live = true, online = i, build = 12340, chip = "3.3.5a", browser = true, mods = 0 } end
WUUI_LEYLINE_ONNET("list")
check("20 rows, 7 shown, a page count", #L.realms == 20 and #shown() == 7 and win.count:GetText() == "Page 1 of 3 · 45 realms")
WUUI_LEYLINE_ASK = nil
L.Scroll(1); L.Scroll(1)
check("Down moves through the rows first", L.offset == 13 and WUUI_LEYLINE_ASK == nil)
L.Scroll(1)
check("then asks the next page", WUUI_LEYLINE_ASK == "list|2||")
L.Scroll(-1); L.Scroll(-1); L.Scroll(-1)
check("Up back to the top of the page", L.offset == 0)

-- back to the four realms, then the preview
feed(listFile, function()
    WUUI_LEYLINE_NET = { page = 1, pages = 1, total = 4, network = { realms = 3, players = 14, bots = 10 }, rows = {
        { id = "hhhhhhhhhhhhhhhh", name = "Wrath Unleashed", live = true, online = 4, bots = 10, build = 12340, chip = "3.3.5a", browser = true, here = true, mods = 4 },
        { id = A, name = "Frost Haven", live = true, online = 7, bots = 0, build = 12340, chip = "3.3.5a", browser = true, hk = true, up = 99, featured = true, mods = 3, desc = "A cosy place" },
        { id = B, name = "Old Days of fun", live = true, online = 0, build = 5875, chip = "1.12.1", browser = false, own = true, mods = 0 },
        { id = C, name = "Quiet Meadow", live = false, online = 0, build = 12340, chip = "3.3.5a", browser = true, mods = 0 } } }
end)
rows = shown()
local frost
for _, r in ipairs(rows) do if r.realm.id == A then frost = r end end
frost.join:Fire("OnClick")
local panel = win.panel
check("View opens the preview beside the window and asks the page for the details", panel and panel:IsShown() and WUUI_LEYLINE_ASK == "preview|" .. A
    and panel.title:GetText() == "Frost Haven" and panel.who:GetText() == "Loading...")
check("no hop yet", WUUI_LEYLINE_HOP == nil)
WUUI_LEYLINE_PREVIEW_KIND = "preview"
feed(previewFile, function()
    WUUI_LEYLINE_PREVIEW = { id = A, name = "Frost Haven", desc = "A cosy place", rules = "Be kind. No spam.", live = true, online = 7, bots = 0,
        who = { "Mira", "Tolan" }, mods = { { name = "Minigolf", req = true, client = false }, { name = "Skada", req = false, client = true },
        { name = "Dalaran dice" } }, build = 12340, chip = "3.3.5a", up = 99, hk = "ready", browser = true, own = false, verdict = "ok", why = "" }
end)
WUUI_LEYLINE_PREVIEW_KIND = nil
check("preview: name, chips (version chip, browser, Hero Key)", panel.title:GetText() == "Frost Haven"
    and panel.chips:GetText():find("3.3.5a") and panel.chips:GetText():find("Plays in browser") and panel.chips:GetText():find("Hero Key sign%-in"))
check("preview: players online now, uptime, the host's words", panel.players:GetText() == "7 players online now"
    and panel.up:GetText() == "99% up lately" and panel.desc:GetText() == "A cosy place")
check("preview: house rules", panel.rules:GetText() == "Be kind. No spam.")
check("preview: who's on (the realm shares it)", panel.who:GetText() == "Mira, Tolan")
check("preview: mods, required and optional, the ones on your side marked", panel.mods:GetText():find("^Required: Minigolf")
    and panel.mods:GetText():find("Optional: Skada |cff5b8def%(on your side%)|r") and panel.mods:GetText():find("Uses: Dalaran dice"))
check("preview: Join", panel.join:GetText() == "Join" and panel.join:IsEnabled())
local shotFrost = true
if dumpFile then L.dumpAt = "frost" end
local function dump(path)
    local function num(v) return v and string.format("%.2f", v) or "null" end
    local function str(v)
        if v == nil then return "null" end
        return '"' .. tostring(v):gsub('[%c"\\]', function(c) return string.format("\\u%04x", c:byte()) end) .. '"'
    end
    local function color(c) return c and ("[" .. num(c[1]) .. "," .. num(c[2]) .. "," .. num(c[3]) .. "," .. num(c[4]) .. "]") or "null" end
    local out = {}
    for _, f in ipairs(created) do
        if f:IsVisible() and (f == win or f == panel or (function() local p = f.parent; while p do if p == win or p == panel then return true end p = p.parent end end)()) then
            local pts = {}
            for _, q in ipairs(f.points or {}) do
                pts[#pts + 1] = string.format('{"p":%s,"rel":%s,"rp":%s,"x":%s,"y":%s}', str(q.p), q.rel and q.rel.n or "null", str(q.rp), num(q.x), num(q.y))
            end
            out[#out + 1] = string.format('{"n":%d,"kind":%s,"parent":%s,"points":[%s],"w":%s,"h":%s,"text":%s,"font":%s,"color":%s,"tcolor":%s,"bd":%s,"justify":%s,"layer":%s,"file":%s,"enabled":%s,"edit":%s}',
                f.n, str(f.kind), f.parent and f.parent.n or "null", table.concat(pts, ","), num(f.w), num(f.h), str(f.text), str(f.font),
                color(f.color or f.vcolor), color(f.tcolor), color(f.bdcolor), str(f.justify), str(f.layer), str(f.file), tostring(f.enabled ~= false),
                tostring(f.kind == "EditBox"))
        end
    end
    local h = assert(io.open(path, "wb"))
    h:write('{"screen":{"w":1229,"h":768},"win":' .. win.n .. ',"frames":[' .. table.concat(out, ",\n") .. "]}\n")
    h:close()
end
if dumpFile then dump(dumpFile:gsub("%.json$", "") .. "-preview.json") end

panel.join:Fire("OnClick")
check("Join from the preview: the hop (the page's fast hop takes it)", WUUI_LEYLINE_HOP == A and panel.info:GetText() == "Going to Frost Haven...")
WUUI_LEYLINE_HOP, L.hopping = nil, nil

-- another game version: never a hop; the launcher
local old
for _, r in ipairs(shown()) do if r.realm.id == B then old = r end end
old.join:Fire("OnClick")
WUUI_LEYLINE_PREVIEW = { id = B, name = "Old Days of fun", desc = "", rules = "", live = true, online = 0, who = nil, mods = {}, build = 5875, chip = "1.12.1",
    hk = "unknown", browser = false, own = true, verdict = "launcher", why = "This realm needs a different game version than this one. Open it in the Leyline launcher." }
WUUI_LEYLINE_ONNET("preview")
check("another version: says so plainly, Open in launcher", panel.why:GetText():find("needs a different game version") and panel.join:GetText() == "Open in launcher"
    and panel.who:GetText() == "This realm doesn't share who's on." and panel.mods:GetText() == "No mods listed."
    and panel.rules:GetText() == "The host hasn't written house rules.")
if dumpFile then dump(dumpFile:gsub("%.json$", "") .. "-launcher.json") end
WUUI_LEYLINE_ASK = nil
panel.join:Fire("OnClick")
check("its Join hands it to the launcher, never a hop", WUUI_LEYLINE_ASK == "launch|" .. B and WUUI_LEYLINE_HOP == nil
    and win.copy:GetText() == "leyline://realm/" .. B and panel.info:GetText():find("Leyline launcher"))

-- offline
local quiet
for _, r in ipairs(shown()) do if r.realm.id == C then quiet = r end end
quiet:Fire("OnClick")
check("a row click opens the preview too (and puts the link in the box)", panel.title:GetText() == "Quiet Meadow" and win.copy:GetText():find(C))
WUUI_LEYLINE_PREVIEW = { id = C, name = "Quiet Meadow", live = false, mods = {}, build = 12340, chip = "3.3.5a", verdict = "offline", why = "This realm is offline right now." }
WUUI_LEYLINE_ONNET("preview")
check("offline: Join is off", panel.join:GetText() == "Offline" and not panel.join:IsEnabled() and panel.players:GetText():find("Offline"))
-- a late preview for another realm is ignored
WUUI_LEYLINE_PREVIEW = { id = A, name = "Frost Haven", verdict = "ok" }
WUUI_LEYLINE_ONNET("preview")
check("a preview for another realm than the one open is ignored", panel.title:GetText() == "Quiet Meadow")

-- the list as it looks with the preview closed, for the screenshot
if dumpFile then
    panel:Hide()
    dump(dumpFile:gsub("%.json$", "") .. "-list.json")
    panel:Show()
end

-- the page stops answering: back to the realm's own list
L.AskList(1)
clock = clock + 7
win:Fire("OnUpdate", 0.1)
check("the page didn't answer: the realm's own list (WULY L)", sent[#sent] and sent[#sent][2] == "L" and WUUI_LEYLINE_BROWSE == nil)
check("and the realm-list filters are back", win.filters.tag:IsVisible() and not win.bfilters.browser:IsVisible())

-- the desktop client (no page): the preview from the realm's list, and the version check is its own
WUUI_PAGE, WUUI_LEYLINE_BROWSE = nil, nil
local r = { id = B, name = "Old Days", live = true, players = 2, build = 5875, ruleset = "main" }
local t = L.PreviewTexts(r, nil)
check("desktop client: another version -> the launcher, said plainly", t.verdict == "launcher" and t.join == "Open in launcher"
    and t.why:find("different game version") and not t.why:find("%d%.%d"))
check("desktop client: the details are the browser game's", t.who == "Shown in the browser game." and t.mods == "Shown in the browser game.")
build = "5875"
check("same version as the realm: Join", L.PreviewTexts(r, nil).verdict == "ok")
build = "12340"
L.selected, L.detail = r, nil
WUUI_LEYLINE_ASK = nil
check("desktop client: its Join gives the launcher link, no page ask", L.PreviewJoin() == "launcher" and WUUI_LEYLINE_ASK == nil
    and win.copy:GetText() == "leyline://realm/" .. B)

-- words: no game brand, no version numbers outside the chips
local bad = 0
for _, f in ipairs(created) do
    local s = tostring(f.text or ""):lower()
    if s:find("warcraft") or s:find("%f[%a]wow%f[%A]") or s:find("blizzard") or s:find("lich king") then bad = bad + 1 end
    if f.kind == "FontString" and f ~= panel.chips and not (f.parent and f.parent.parent == win.list) and not (f.parent and f.parent.kind == "Button")
        and s:find("%d+%.%d+%.%d") then bad = bad + 1; print("  version in prose: " .. s) end
end
check("no game-brand names, no version numbers in prose", bad == 0)

print(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
