-- lua5.1 stream/tests/test_wuui_plates.lua [addon dir]
-- Compact nameplates (Nameplates.lua; owner 3 Nov: "fix nameplate height and size"). A fake 3.3.5a WorldFrame with
-- stock plates (unnamed children whose second region is the Nameplate-Border texture, the regions and the two bars in
-- the client's order): plates are found as they appear, the bar, name, level, cast bar and icons get their sizes and
-- anchors, applying again changes nothing, the client's resets are undone, /plates normal | off | small, and off puts
-- back exactly the client's own layout. Another nameplate addon loaded = nothing touched.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end
unpack = unpack or table.unpack

-- ------------------------------------------------------------------ fake widgets
local O = {}
O.__index = O
local made = 0
local function obj(kind, name, parent)
    made = made + 1
    local o = setmetatable({ kind = kind, name = name, parent = parent, points = {}, w = 0, h = 0, alpha = 1,
        shown = true, scripts = {}, regions = {}, kids = {} }, O)
    if name then _G[name] = o end
    return o
end
function O:GetName() return self.name end
function O:GetObjectType() return self.kind end
function O:SetPoint(...) self.points[#self.points + 1] = { ... } end
function O:ClearAllPoints() self.points = {} end
function O:SetAllPoints(r) self.points = { { "TOPLEFT", r, "TOPLEFT", 0, 0 }, { "BOTTOMRIGHT", r, "BOTTOMRIGHT", 0, 0 } } end
function O:GetNumPoints() return #self.points end
function O:GetPoint(i) return unpack(self.points[i]) end
function O:SetWidth(w)
    local old = self.w; self.w = w
    if old ~= w and self.scripts.OnSizeChanged then self.scripts.OnSizeChanged(self, self.w, self.h) end
end
function O:SetHeight(h)
    local old = self.h; self.h = h
    if old ~= h and self.scripts.OnSizeChanged then self.scripts.OnSizeChanged(self, self.w, self.h) end
end
function O:GetWidth() return self.w end
function O:GetHeight() return self.h end
function O:SetAlpha(a) self.alpha = a end
function O:GetAlpha() return self.alpha end
function O:Show() local was = self.shown; self.shown = true; if not was and self.scripts.OnShow then self.scripts.OnShow(self) end end
function O:Hide() self.shown = false end
function O:IsShown() return self.shown end
function O:SetScript(n, fn) self.scripts[n] = fn end
function O:HookScript(n, fn)
    local old = self.scripts[n]
    self.scripts[n] = function(...) if old then old(...) end fn(...) end
end
function O:RegisterEvent(e) self.events = self.events or {}; self.events[e] = true end
function O:GetRegions() return unpack(self.regions) end
function O:GetChildren() return unpack(self.kids) end
function O:GetNumChildren() return #self.kids end
function O:SetTexture(a, ...) if type(a) == "number" then self.color = { a, ... } else self.tex = a end end
function O:GetTexture() return self.tex end
function O:SetVertexColor(r, g, b) self.vc = { r, g, b } end
function O:GetVertexColor() return unpack(self.vc or { 1, 1, 1 }) end
function O:SetFont(p, s, f) self.font = { p, s, f } end
function O:GetFont() return unpack(self.font or {}) end
function O:SetShadowOffset() end
function O:CreateTexture(name, layer)
    local t = obj("Texture", name, self); t.layer = layer
    self.made = self.made or {}; self.made[#self.made + 1] = t
    return t
end
function O:CreateFontString(name) return obj("FontString", name, self) end

-- ------------------------------------------------------------------ fake client
local function newWorld(addons)
    made = 0
    WorldFrame = obj("Frame", "WorldFrame")
    UIParent = obj("Frame", "UIParent")
    WrathUnleashedUI = {}
    WrathUnleashedUIDB = nil
    SlashCmdList = {}
    ForeverUI = nil
    local chat = {}
    DEFAULT_CHAT_FRAME = { AddMessage = function(_, t) chat[#chat + 1] = t end }
    IsAddOnLoaded = function(n) return addons and addons[n] or false end
    local frames = {}
    CreateFrame = function(kind, name, parent)
        local f = obj(kind, name, parent)
        frames[#frames + 1] = f
        return f
    end
    local f, err = loadfile(dir .. "/Nameplates.lua")
    assert(f, err)
    f()
    local P = WrathUnleashedUI.Plates
    local function fire(e) for _, fr in ipairs(frames) do if fr.events and fr.events[e] and fr.scripts.OnEvent then fr.scripts.OnEvent(fr, e) end end end
    local function tick() local d = P.driver; if d.scripts.OnUpdate then d.scripts.OnUpdate(d, 0.25) end end
    return P, fire, tick, chat
end

-- a stock plate as the client builds it (sizes from the stock layout)
local function stockPlate()
    local f = obj("Frame", nil, WorldFrame)
    f.w, f.h = 156, 39
    f:SetPoint("BOTTOM", WorldFrame, "BOTTOMLEFT", 400, 300)
    local function tex(path, w, h, point)
        local t = obj("Texture", nil, f); t.tex = path; t.w, t.h = w, h; t:SetPoint(unpack(point)); return t
    end
    local health = obj("StatusBar", nil, f); health.w, health.h = 116, 11
    health:SetPoint("BOTTOMLEFT", f, "BOTTOMLEFT", 5, 5)
    local cast = obj("StatusBar", nil, f); cast.w, cast.h = 116, 10; cast.shown = false
    cast:SetPoint("TOP", health, "BOTTOM", 0, -6)
    local glow = tex("Interface\\TargetingFrame\\UI-TargetingFrame-Flash", 172, 44, { "CENTER", f, "CENTER", 0, 0 }); glow.shown = false
    local border = tex("Interface\\Tooltips\\Nameplate-Border", 156, 39, { "CENTER", f, "CENTER", 0, 0 })
    local castBorder = tex("Interface\\Tooltips\\Nameplate-CastBar", 156, 39, { "CENTER", cast, "CENTER", 0, 0 })
    local shield = tex("Interface\\Tooltips\\Nameplate-CastBar-Shield", 156, 39, { "CENTER", cast, "CENTER", 0, 0 }); shield.shown = false
    local icon = tex(nil, 16, 16, { "CENTER", cast, "LEFT", -4, 0 })
    local hl = tex("Interface\\Tooltips\\Nameplate-Glow", 156, 39, { "CENTER", f, "CENTER", 0, 0 })
    local name = obj("FontString", nil, f); name.font = { "Fonts\\FRIZQT__.TTF", 14, "" }; name:SetPoint("BOTTOM", f, "TOP", 0, -14)
    local level = obj("FontString", nil, f); level.font = { "Fonts\\FRIZQT__.TTF", 12, "" }; level:SetPoint("CENTER", f, "BOTTOMRIGHT", -12, 11)
    local skull = tex("Interface\\TargetingFrame\\UI-TargetingFrame-Skull", 18, 18, { "CENTER", level, "CENTER", 0, 0 })
    local raid = tex("Interface\\TargetingFrame\\UI-RaidTargetingIcons", 32, 32, { "BOTTOM", f, "TOP", 0, 0 })
    local elite = tex("Interface\\Tooltips\\EliteNameplateIcon", 64, 32, { "RIGHT", f, "RIGHT", 10, 0 })
    f.regions = { glow, border, castBorder, shield, icon, hl, name, level, skull, raid, elite }
    f.kids = { health, cast }
    WorldFrame.kids[#WorldFrame.kids + 1] = f
    return f, { health = health, cast = cast, glow = glow, border = border, castBorder = castBorder, shield = shield,
        icon = icon, hl = hl, name = name, level = level, skull = skull, raid = raid, elite = elite }
end

local function snapshot(r)
    local s = { w = r.w, h = r.h, alpha = r.alpha, font = r.font and { unpack(r.font) } }
    s.points = {}
    for i, p in ipairs(r.points) do s.points[i] = { unpack(p) } end
    return s
end
local function same(a, b)
    if a.w ~= b.w or a.h ~= b.h or a.alpha ~= b.alpha or #a.points ~= #b.points then return false end
    for i = 1, #a.points do
        for j = 1, 5 do if a.points[i][j] ~= b.points[i][j] then return false end end
    end
    if (a.font == nil) ~= (b.font == nil) then return false end
    if a.font then for j = 1, 3 do if a.font[j] ~= b.font[j] then return false end end end
    return true
end
local function pt(r, i) return r.points[i] or {} end

-- ------------------------------------------------------------------ found and restyled
local P, fire, tick, chat = newWorld()
check("no scanning before the world is entered", P.driver.scripts.OnUpdate == nil)
local named = obj("Frame", "SomeNamedFrame", WorldFrame); WorldFrame.kids[#WorldFrame.kids + 1] = named
local other = obj("Frame", nil, WorldFrame); other.regions = { obj("Texture"), obj("Texture") }
other.regions[2].tex = "Interface\\Something"; WorldFrame.kids[#WorldFrame.kids + 1] = other
local f1, r1 = stockPlate()
local orig = {}
for k, r in pairs(r1) do orig[k] = snapshot(r) end
local origFrame = snapshot(f1)
fire("PLAYER_ENTERING_WORLD")
tick()
check("one plate found among WorldFrame's children", #P.Plates() == 1 and P.Plates()[1].frame == f1)
check("default mode small", P.Mode() == "small")
local c = P.SIZES.small
check("health bar 110 x 6", r1.health.w == 110 and r1.health.h == 6 and c.w == 110 and c.h == 6)
check("health bar centred on the plate", pt(r1.health, 1)[1] == "CENTER" and pt(r1.health, 1)[2] == f1 and #r1.health.points == 1)
check("name 2px above the bar", pt(r1.name, 1)[1] == "BOTTOM" and pt(r1.name, 1)[2] == r1.health and pt(r1.name, 1)[3] == "TOP" and pt(r1.name, 1)[5] == 2)
check("name font smaller, outlined, the client's own font", r1.name.font[1] == "Fonts\\FRIZQT__.TTF" and r1.name.font[2] == 9 and r1.name.font[3] == "OUTLINE")
check("level just right of the bar", pt(r1.level, 1)[1] == "LEFT" and pt(r1.level, 1)[2] == r1.health and pt(r1.level, 1)[3] == "RIGHT" and r1.level.font[2] == 8)
check("skull in the level's place", pt(r1.skull, 1)[2] == r1.health and pt(r1.skull, 1)[3] == "RIGHT")
check("gold frame, glow and elite art hidden", r1.border.alpha == 0 and r1.glow.alpha == 0 and r1.elite.alpha == 0)
check("highlight covers the bar", pt(r1.hl, 1)[2] == r1.health and pt(r1.hl, 2)[2] == r1.health)
check("cast bar under the bar, same width", pt(r1.cast, 1)[1] == "TOPLEFT" and pt(r1.cast, 1)[2] == r1.health and pt(r1.cast, 1)[3] == "BOTTOMLEFT"
    and pt(r1.cast, 2)[1] == "TOPRIGHT" and pt(r1.cast, 2)[3] == "BOTTOMRIGHT" and r1.cast.h == 5 and r1.cast.w == 110)
check("cast frame art hidden", r1.castBorder.alpha == 0 and r1.shield.alpha == 0)
check("spell icon left of both bars, as tall as both", pt(r1.icon, 1)[2] == r1.health and pt(r1.icon, 1)[3] == "TOPLEFT" and r1.icon.h == 6 + 3 + 5)
check("raid mark above the name", pt(r1.raid, 1)[2] == r1.name and r1.raid.w == 14)
check("dark 1px edge behind each bar", r1.health.made and #r1.health.made == 1 and r1.health.made[1].layer == "BACKGROUND"
    and r1.health.made[1].color[4] == 0.85 and r1.cast.made and #r1.cast.made == 1)
check("the plate's clickable frame kept (already big enough)", f1.w == 156 and f1.h == 39)
check("the plate frame's own position untouched", same(snapshot(f1), origFrame))
check("other children left alone", named.made == nil and other.made == nil)

-- ------------------------------------------------------------------ idempotent, cheap, client resets
local before = {}
for k, r in pairs(r1) do before[k] = snapshot(r) end
P.Apply(P.Plates()[1]); tick(); tick()
local still = true
for k, r in pairs(r1) do if not same(before[k], snapshot(r)) then still = false; print("  changed: " .. k) end end
check("applying again changes nothing", still)
check("no second edge texture", #r1.health.made == 1 and #r1.cast.made == 1)
local adopted = 0
local realAdopt = P.Adopt
P.Adopt = function(...) adopted = adopted + 1; return realAdopt(...) end
tick(); tick()
check("no new children = nothing looked at again", adopted == 0)
local f2, r2 = stockPlate()
tick()
check("a new plate is found on the next tick", #P.Plates() == 2 and adopted == 1 and r2.health.w == 110)
P.Adopt = realAdopt

-- the client reuses a plate for another unit: hidden, reset, shown
f1:Hide(); r1.health:ClearAllPoints(); r1.health:SetPoint("BOTTOMLEFT", f1, "BOTTOMLEFT", 5, 5); r1.health.w, r1.health.h = 116, 11
f1:Show()
check("reshown plate restyled", r1.health.w == 110 and r1.health.h == 6 and pt(r1.health, 1)[1] == "CENTER")
-- a cast starts: the client moves and resizes the cast bar
r1.cast:ClearAllPoints(); r1.cast:SetPoint("TOP", r1.health, "BOTTOM", 0, -6); r1.cast:SetHeight(10)
check("cast bar resize undone", r1.cast.h == 5 and pt(r1.cast, 1)[1] == "TOPLEFT")
r1.cast:Hide(); r1.cast:ClearAllPoints(); r1.cast:Show()
check("cast bar placed again on show", #r1.cast.points == 2 and pt(r1.cast, 1)[2] == r1.health)

-- threat and the shield as the edge's colour
r1.glow.shown = true; r1.glow.vc = { 1, 0, 0 }
tick()
check("threat: the edge takes the glow's colour", r1.health.made[1].color[1] == 1 and r1.health.made[1].color[2] == 0 and r1.health.made[1].color[4] == 1)
r1.glow.shown = false; r1.shield.shown = true
tick()
check("no threat: dark edge again", r1.health.made[1].color[1] == 0 and r1.health.made[1].color[4] == 0.85)
check("can't interrupt: light grey cast edge", r1.cast.made[1].color[1] == 0.75)
r1.shield.shown = false; tick()

-- ------------------------------------------------------------------ /plates
local slash = SlashCmdList["LEYLINEPLATES"]
check("/plates registered", SLASH_LEYLINEPLATES1 == "/plates" and type(slash) == "function")
slash("normal")
check("/plates normal: 128 x 8, saved", r1.health.w == 128 and r1.health.h == 8 and r2.health.w == 128 and WrathUnleashedUIDB.plates == "normal")
slash("  OFF ")
local back = true
for k, r in pairs(r1) do if not same(orig[k], snapshot(r)) then back = false; print("  not restored: " .. k) end end
check("/plates off: every region exactly as the client made it", back)
check("/plates off: edges hidden, saved", not r1.health.made[1].shown and not r1.cast.made[1].shown and WrathUnleashedUIDB.plates == "off")
check("/plates off: plate frame as it was", same(snapshot(f1), origFrame))
f1:Hide(); f1:Show(); tick()
check("off stays off when a plate is reused", r1.health.w == 116 and r1.border.alpha == 1)
slash("small")
check("/plates small again", r1.health.w == 110 and r1.border.alpha == 0 and r1.health.made[1].shown and #r1.health.made == 1)
slash("huge")
check("an unknown size isn't saved", WrathUnleashedUIDB.plates == "small" and chat[#chat]:find("/plates small | normal | off", 1, true))
slash("")
check("/plates alone says the mode", chat[#chat]:find("small", 1, true) ~= nil)

-- saved off: plates found later stay stock
P, fire, tick, chat = newWorld()
WrathUnleashedUIDB = { plates = "off" }
local f3, r3 = stockPlate()
fire("PLAYER_ENTERING_WORLD"); tick()
check("saved off: plate found but left stock", #P.Plates() == 1 and r3.health.w == 116 and r3.border.alpha == 1 and r3.health.made == nil)

-- a tiny plate frame grows to hold the layout, and shrinks back on off
P, fire, tick, chat = newWorld()
local f4, r4 = stockPlate(); f4.w, f4.h = 60, 10
fire("PLAYER_ENTERING_WORLD"); tick()
check("a frame smaller than the layout grows", f4.w >= 110 and f4.h >= 6 + 9 + 2)
SlashCmdList["LEYLINEPLATES"]("off")
check("and its size comes back on off", f4.w == 60 and f4.h == 10)

-- ------------------------------------------------------------------ other nameplate addons win
P, fire, tick, chat = newWorld({ TidyPlates = true })
local f5, r5 = stockPlate()
fire("PLAYER_ENTERING_WORLD"); tick()
check("another nameplate addon: nothing touched", P.blockedBy == "TidyPlates" and #P.Plates() == 0 and r5.health.w == 116 and r5.border.alpha == 1)
SlashCmdList["LEYLINEPLATES"]("small")
check("/plates says who draws them", chat[#chat]:find("TidyPlates", 1, true) ~= nil and r5.health.w == 116)
P, fire, tick, chat = newWorld()
local f6, r6 = stockPlate(); r6.border.alpha = 0
fire("PLAYER_ENTERING_WORLD"); tick()
check("a plate whose frame another addon hid is left alone", #P.Plates() == 0 and r6.health.w == 116)
P, fire, tick, chat = newWorld()
ForeverUI = { Nameplates = {} }
local f7, r7 = stockPlate()
fire("PLAYER_ENTERING_WORLD"); tick()
check("a Forever UI that styles plates itself wins", P.blockedBy == "ForeverUI" and r7.health.w == 116)

-- ------------------------------------------------------------------ the words players read
local src = io.open(dir .. "/Nameplates.lua"):read("*a")
local said = {}
for s in src:gmatch('say%((.-)%)\n') do said[#said + 1] = s end
local clean = true
for _, s in ipairs(said) do
    for _, w in ipairs({ "World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard" }) do
        if s:find(w, 1, true) then clean = false end
    end
end
check("no brand words in what players read", #said >= 3 and clean)

print(("%d passed, %d failed"):format(pass, fail))
if fail > 0 then os.exit(1) end
