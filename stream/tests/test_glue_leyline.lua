-- lua5.1 stream/tests/test_glue_leyline.lua [GlueXML dir] [--dump out.json]
-- Leyline realms on character select (GlueXML/ForeverUIGlueLeyline.lua, the browser game's glue): Forever UI's REALMS
-- button opens the Leyline realm browser while the page offers it, and does what it always did otherwise (and with
-- Shift); without that button, a Realms button of our own. The list from the page, search and filters, the preview
-- (rules, who's on only when shared, mods, version chip, Hero Key), Join = a hop the page takes, another game version =
-- the launcher (never a hop), offline, and a failure inside never breaks character select.
local dir = arg[1] or "stream/fui-web/Interface/GlueXML"
local dumpFile
for i = 1, #arg do if arg[i] == "--dump" then dumpFile = arg[i + 1] end end

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


local clock = 100
function GetTime() return clock end
local shift = false
function IsShiftKeyDown() return shift end
GlueFontNormal, GlueFontHighlightSmall, GlueFontNormalLarge, GlueFontHighlight, GlueFontDisableSmall = {}, {}, {}, {}, {}

local function setup(withNav)
    created = {}
    CharacterSelect = new("Frame", "CharacterSelect"); CharacterSelect.top = true; CharacterSelect.w, CharacterSelect.h = 1229, 768
    CharacterSelectUI = new("Frame", "CharacterSelectUI", CharacterSelect); CharacterSelectUI.w, CharacterSelectUI.h = 1229, 768
    CharacterSelectUI:SetAllPoints(CharacterSelect)
    local native = 0
    if withNav then
        ForeverUICharacterSelectNavBar = new("Frame", "ForeverUICharacterSelectNavBar", CharacterSelectUI)
        local tray = new("Frame", nil, ForeverUICharacterSelectNavBar)
        local realms = new("Button", nil, tray); realms.text = "REALMS"
        realms:SetScript("OnClick", function() native = native + 1 end)
        local menu = new("Button", nil, tray); menu.text = "MENU"
        ForeverUICharacterSelectNavBar.realms = realms
    else
        ForeverUICharacterSelectNavBar = nil
    end
    WUUI_LEYLINE_BROWSE, WUUI_LEYLINE_ASK, WUUI_LEYLINE_HOP, LeylineGlue, WUUI_LEYLINE_ONNET_GLUE = nil, nil, nil, nil, nil
    dofile(dir .. "/ForeverUIGlueLeyline.lua")
    return function() return native end
end

-- no page (the desktop client): REALMS does what it always did
local native = setup(true)
local nav = ForeverUICharacterSelectNavBar.realms
nav:Fire("OnClick")
check("no page: REALMS opens this server's realm list, as before", native() == 1 and not (LeylineGlue.win and LeylineGlue.win:IsShown()))
-- the page is there
WUUI_LEYLINE_BROWSE = 1
nav:Fire("OnClick")
local LG = LeylineGlue
local w = LG.win
check("with the page: REALMS opens the Leyline realm browser and asks for the list", native() == 1 and w and w:IsShown()
    and WUUI_LEYLINE_ASK == "list|1||" and type(WUUI_LEYLINE_ONNET_GLUE) == "function")
shift = true; w:Hide(); nav:Fire("OnClick"); shift = false
check("Shift+REALMS: still this server's realm list", native() == 2 and not w:IsShown())
nav:Fire("OnClick")

local A, B, C = "aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb", "cccccccccccccccc"
WUUI_LEYLINE_NET = { page = 1, pages = 2, total = 24, network = { realms = 3, players = 14, bots = 10 }, rows = {
    { id = "hhhhhhhhhhhhhhhh", name = "Wrath Unleashed", live = true, online = 4, build = 12340, chip = "3.3.5a", browser = true, here = true },
    { id = A, name = "Frost Haven", live = true, online = 7, build = 12340, chip = "3.3.5a", browser = true, hk = true, up = 99, featured = true, desc = "A cosy place" },
    { id = B, name = "Old Days of fun", live = true, online = 0, build = 5875, chip = "1.12.1", browser = false, own = true },
    { id = C, name = "Quiet Meadow", live = false, online = 0, build = 12340, chip = "3.3.5a", browser = true } } }
WUUI_LEYLINE_ONNET_GLUE("list")
local rows = {}
for _, r in ipairs(w.rowsUi) do if r:IsShown() then rows[#rows + 1] = r end end
check("the rows: name, chips, players", #rows == 4 and rows[2].name:GetText():find("^Frost Haven") and rows[2].sub:GetText() == "3.3.5a · Plays in browser · Hero Key"
    and rows[2].state:GetText():find("7 playing") and rows[1].name:GetText():find("you are here") and rows[4].state:GetText():find("offline"))
check("the network line, the page count", w.status:GetText() == "3 realms · 14 players online" and w.count:GetText() == "Page 1 of 2")
LG.Scroll(1)
check("Down past the rows asks the next page", WUUI_LEYLINE_ASK == "list|2||")
w.fb:Fire("OnClick")
check("filter: Plays in browser", WUUI_LEYLINE_ASK == "list|1|b|")
w.fb:Fire("OnClick")
if w.search then
    w.search:SetText("frost"); w.search:Fire("OnTextChanged")
    clock = clock + 1; w:Fire("OnUpdate", 0.1)
    check("search, after the typing stops", WUUI_LEYLINE_ASK == "list|1||frost")
end
check("before a pick: a word on what the preview shows", w.empty:IsShown() and not w.pv:IsShown())
rows[2]:Fire("OnClick")
check("a pick asks for its preview; Loading meanwhile", WUUI_LEYLINE_ASK == "preview|" .. A and w.pv:IsShown() and w.pv.who:GetText() == "Loading...")
WUUI_LEYLINE_PREVIEW = { id = A, name = "Frost Haven", desc = "A cosy place", rules = "Be kind. No spam.", live = true, online = 7, who = { "Mira", "Tolan" },
    mods = { { name = "Minigolf", req = true, client = false }, { name = "Skada", req = false, client = true } }, build = 12340, chip = "3.3.5a",
    up = 99, hk = "ready", browser = true, verdict = "ok", why = "" }
WUUI_LEYLINE_ONNET_GLUE("preview")
local pv = w.pv
check("preview: chips with the version chip and Hero Key", pv.chips:GetText():find("3.3.5a") and pv.chips:GetText():find("Hero Key sign%-in"))
check("preview: players, uptime, rules, who, mods", pv.players:GetText() == "7 players online now  ·  99% up lately" and pv.rules:GetText() == "Be kind. No spam."
    and pv.who:GetText() == "Mira, Tolan" and pv.mods:GetText() == "Required: Minigolf\nOptional: Skada |cff5b8def(on your side)|r")
local function dump(path)
    local function num(v) return v and string.format("%.2f", v) or "null" end
    local function str(v)
        if v == nil then return "null" end
        return '"' .. tostring(v):gsub('[%c"\\]', function(c) return string.format("\\u%04x", c:byte()) end) .. '"'
    end
    local function color(c) return c and ("[" .. num(c[1]) .. "," .. num(c[2]) .. "," .. num(c[3]) .. "," .. num(c[4]) .. "]") or "null" end
    local out = {}
    for _, f in ipairs(created) do
        local inside = false
        local p = f
        while p do if p == w then inside = true end p = p.parent end
        if inside and f:IsVisible() then
            local pts = {}
            for _, q in ipairs(f.points or {}) do
                pts[#pts + 1] = string.format('{"p":%s,"rel":%s,"rp":%s,"x":%s,"y":%s}', str(q.p), q.rel and q.rel.n or (f.parent and f.parent.n or "null"), str(q.rp), num(q.x), num(q.y))
            end
            out[#out + 1] = string.format('{"n":%d,"kind":%s,"parent":%s,"points":[%s],"w":%s,"h":%s,"text":%s,"font":%s,"color":%s,"tcolor":%s,"bd":null,"justify":%s,"layer":%s,"file":null,"enabled":%s,"edit":%s}',
                f.n, str(f.kind), (f.parent and f.parent ~= CharacterSelect) and f.parent.n or "null", table.concat(pts, ","), num(f.w), num(f.h), str(f.text),
                str(f.font and f.font:gsub("^Glue", "Game")), color(f.color or f.vcolor), color(f.tcolor), str(f.justify), str(f.layer), tostring(f.enabled ~= false), tostring(f.kind == "EditBox"))
        end
    end
    local h = assert(io.open(path, "wb"))
    h:write('{"screen":{"w":1229,"h":768},"frames":[' .. table.concat(out, ",\n") .. "]}\n")
    h:close()
end
if dumpFile then dump(dumpFile) end
pv.join:Fire("OnClick")
check("Join: the hop the page takes (straight to that realm's characters with Hero Key)", WUUI_LEYLINE_HOP == A and w.info:GetText() == "Going to Frost Haven...")
WUUI_LEYLINE_HOP = nil
rows[3]:Fire("OnClick")
WUUI_LEYLINE_PREVIEW = { id = B, name = "Old Days of fun", live = true, mods = {}, build = 5875, chip = "1.12.1", browser = false, own = true,
    verdict = "launcher", why = "This realm needs a different game version than this one. Open it in the Leyline launcher." }
WUUI_LEYLINE_ONNET_GLUE("preview")
check("another version: said plainly, Open in launcher", pv.why:GetText():find("different game version") and pv.join.label:GetText() == "Open in launcher")
pv.join:Fire("OnClick")
check("its Join asks for the launcher bar, never a hop", WUUI_LEYLINE_ASK == "launch|" .. B and WUUI_LEYLINE_HOP == nil)
rows[4]:Fire("OnClick")
WUUI_LEYLINE_PREVIEW = { id = C, name = "Quiet Meadow", live = false, mods = {}, chip = "3.3.5a", verdict = "offline", why = "This realm is offline right now." }
WUUI_LEYLINE_ONNET_GLUE("preview")
check("offline: Join off", pv.join.label:GetText() == "Offline" and not pv.join:IsEnabled())
rows[1]:Fire("OnClick")
check("this realm: You're here, no hop", pv.join.label:GetText() == "You're here" and not pv.join:IsEnabled())
w.native:Fire("OnClick")
check("This server's realms: the old list", native() == 3 and not w:IsShown())
-- a broken answer never throws into the glue
WUUI_LEYLINE_NET = { rows = { 5, { id = 7 }, { id = "short" } } }
check("junk from the page is ignored, no error", pcall(WUUI_LEYLINE_ONNET_GLUE, "list"))
-- no REALMS button in the top bar: our own Realms button
setup(false)
check("no top bar: our own Realms button", LeylineGlue.own and LeylineGlue.own.label:GetText() == "Realms")
LeylineGlue.own:Fire("OnClick")
check("it does nothing without the page", not LeylineGlue.win)
WUUI_LEYLINE_BROWSE = 1
LeylineGlue.own:Fire("OnClick")
check("and opens the browser with it", LeylineGlue.win and LeylineGlue.win:IsShown())
-- a failing client call while building never breaks character select
setup(true)
WUUI_LEYLINE_BROWSE = 1
local realCF = CreateFrame
CreateFrame = function() error("no frames today") end
local ok = pcall(function() ForeverUICharacterSelectNavBar.realms:Fire("OnClick") end)
CreateFrame = realCF
check("a failure building the browser falls back to the old realm list", ok)
-- words
local bad = 0
for _, f in ipairs(created) do
    local s = tostring(f.text or ""):lower()
    if s:find("warcraft") or s:find("%f[%a]wow%f[%A]") or s:find("blizzard") or s:find("lich king") then bad = bad + 1 end
end
check("no game-brand names", bad == 0)
-- the stock realm wizard never shows: it goes straight on to character select
do
    local screens = {}
    function SetGlueScreen(n) screens[#screens + 1] = n end
    RealmWizard = new("Frame", "RealmWizard")
    setup(true)
    RealmWizard.shown = true
    RealmWizard.scripts.OnShow(RealmWizard)
    check("the stock realm wizard is skipped: straight on to character create (no loop back through character select)", screens[#screens] == "charcreate" and LeylineGlue.skipWizard == true)
    RealmWizard, SetGlueScreen = nil, nil
end
print(string.format("\n%d/%d passed", pass, pass + fail))
os.exit(fail == 0 and 0 or 1)
