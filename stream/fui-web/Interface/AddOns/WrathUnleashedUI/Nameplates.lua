-- Nameplates.lua (3.3.5a API): compact, tidy nameplates. Owner 3 Nov, a neutral "Young Wolf" plate: the name floating
-- well above a thick, wide yellow bar in its gold frame with the level badge on the right: "fix nameplate height and
-- size". Nothing styled plates before this (Forever UI has no nameplate code; it draws the client's stock plate).
--
-- Now: a slim bar (small: 110 x 6, normal: 128 x 8) in a 1px dark edge instead of the gold frame, the name 2px above
-- it in a smaller outlined font, the level (or the skull) just right of the bar, the cast bar under it at the same
-- width with its spell icon at the left of both bars, the raid mark above the name. The frame's threat glow and the
-- cast bar's "can't interrupt" shield are drawn as the edge's colour (red / orange for threat, light grey for the
-- shield) instead of their frame-shaped art. The plate frame itself (the clickable area) is the client's; it only
-- grows if it were ever smaller than the new layout.
--
--   /plates small | normal | off     saved in WrathUnleashedUIDB.plates (default small); off puts every plate back
--                                    exactly as the client made it.
--
-- How plates are found: the client makes each plate an unnamed child of WorldFrame and reuses it, so a cheap scan
-- (every 0.2 s, only when WorldFrame's child count changes) looks at new children once. A plate is the one whose
-- second region is the texture "Interface\Tooltips\Nameplate-Border". Its regions, in order: threat glow, health
-- border, cast border, cast shield, spell icon, highlight, name, level, skull, raid icon, elite icon; its children:
-- the health bar and the cast bar. A plate is restyled again on each show (the client reuses it for another unit) and
-- the cast bar again when the client moves or resizes it at a cast's start.
--
-- Other nameplate addons win: with one of P.OTHERS loaded, or a Forever UI that styles plates itself, nothing here
-- touches a plate; nor is a plate touched whose border another addon has already hidden.

local W = WrathUnleashedUI
local P = {}
W.Plates = P

P.BORDER = "Interface\\Tooltips\\Nameplate-Border"
P.TICK = 0.2
P.DEFAULT = "small"
P.SIZES = {
    small  = { w = 110, h = 6, name = 9,  level = 8, cast = 5, gap = 2, castGap = 3, raid = 14 },
    normal = { w = 128, h = 8, name = 10, level = 9, cast = 6, gap = 2, castGap = 3, raid = 16 },
}
P.OTHERS = { "TidyPlates", "Aloft", "caelNameplates", "shNameplates", "dNameplates", "Kui_Nameplates", "KuiNameplates",
    "nPlates", "ElvUI", "TukUI", "Tukui", "Nameplates", "EKplates", "rNamePlates", "mNameplates", "Healium_Nameplates" }
P.EDGE = { 0, 0, 0, 0.85 }
P.SHIELD = { 0.75, 0.75, 0.75, 1 }

local plates = {}      -- list of adopted plates' states
local byFrame = {}     -- plate frame -> its state
local seen = {}        -- every WorldFrame child already looked at
local lastN = -1
local started = false

local function db()
    if type(WrathUnleashedUIDB) ~= "table" then WrathUnleashedUIDB = {} end
    return WrathUnleashedUIDB
end

function P.Mode()
    local m = db().plates
    if m == "off" or P.SIZES[m] then return m end
    return P.DEFAULT
end

local function loaded(name)
    if not IsAddOnLoaded then return false end
    local ok, v = pcall(IsAddOnLoaded, name)
    return ok and v and true or false
end

-- Another addon owns the plates (looked at when the world is entered: every addon has loaded by then).
function P.Blocked()
    for _, name in ipairs(P.OTHERS) do
        if loaded(name) then return name end
    end
    local F = ForeverUI
    if type(F) == "table" and (F.Nameplates or F.NamePlates) then return "ForeverUI" end
    return nil
end

function P.IsPlate(f)
    if not f or f.GetName == nil or f:GetName() then return false end
    if not f.GetRegions then return false end
    local _, border = f:GetRegions()
    return border ~= nil and border.GetObjectType ~= nil and border:GetObjectType() == "Texture"
        and border:GetTexture() == P.BORDER
end

-- ------------------------------------------------------------------ the client's own layout, kept for "off"
local function keep(r)
    if not r then return nil end
    local k = { points = {}, alpha = r.GetAlpha and r:GetAlpha() or 1 }
    if r.GetNumPoints then
        for i = 1, r:GetNumPoints() do k.points[i] = { r:GetPoint(i) } end
    end
    if r.GetWidth then k.w, k.h = r:GetWidth(), r:GetHeight() end
    if r.GetFont then k.font = { r:GetFont() } end
    return k
end

local function putBack(r, k)
    if not r or not k then return end
    if #k.points > 0 then
        r:ClearAllPoints()
        for _, p in ipairs(k.points) do r:SetPoint(unpack(p)) end
    end
    if k.w and k.w > 0 then r:SetWidth(k.w) end
    if k.h and k.h > 0 then r:SetHeight(k.h) end
    if k.font and k.font[1] then r:SetFont(k.font[1], k.font[2], k.font[3]) end
    if r.SetAlpha then r:SetAlpha(k.alpha) end
end

local REGIONS = { "glow", "border", "castBorder", "shield", "icon", "highlight", "name", "level", "skull", "raid", "elite" }

function P.Adopt(f)
    if byFrame[f] then return byFrame[f] end
    local st = { frame = f }
    local regions = { f:GetRegions() }
    for i, key in ipairs(REGIONS) do st[key] = regions[i] end
    st.health, st.cast = f:GetChildren()
    -- another addon already took this plate's frame away: leave it alone
    if not st.health or (st.border and st.border.GetAlpha and st.border:GetAlpha() == 0) then
        st.foreign = true
        byFrame[f] = st
        return st
    end
    st.kept = { frame = { w = f:GetWidth(), h = f:GetHeight() }, health = keep(st.health), cast = keep(st.cast) }
    for _, key in ipairs(REGIONS) do st.kept[key] = keep(st[key]) end
    byFrame[f] = st
    plates[#plates + 1] = st
    if f.HookScript then
        f:HookScript("OnShow", function() P.Apply(st) end)
    end
    if st.cast and st.cast.HookScript then
        st.cast:HookScript("OnShow", function() P.PlaceCast(st) end)
        st.cast:HookScript("OnSizeChanged", function(bar)
            local cfg = P.SIZES[P.Mode()]
            if cfg and st.styled and math.abs((bar:GetHeight() or 0) - cfg.cast) > 0.5 then P.PlaceCast(st) end
        end)
    end
    P.Apply(st)
    return st
end

local function edge(bar)
    local t = bar:CreateTexture(nil, "BACKGROUND")
    t:SetPoint("TOPLEFT", bar, "TOPLEFT", -1, 1)
    t:SetPoint("BOTTOMRIGHT", bar, "BOTTOMRIGHT", 1, -1)
    t:SetTexture(unpack(P.EDGE))
    return t
end

local function font(fs, kept, size)
    if not fs or not fs.SetFont then return end
    local path = kept and kept.font and kept.font[1] or "Fonts\\FRIZQT__.TTF"
    fs:SetFont(path, size, "OUTLINE")
    if fs.SetShadowOffset then fs:SetShadowOffset(0, 0) end
end

function P.PlaceCast(st)
    local cfg = P.SIZES[P.Mode()]
    local cast, health = st.cast, st.health
    if not cfg or not cast or not st.styled then return end
    cast:ClearAllPoints()
    cast:SetPoint("TOPLEFT", health, "BOTTOMLEFT", 0, -cfg.castGap)
    cast:SetPoint("TOPRIGHT", health, "BOTTOMRIGHT", 0, -cfg.castGap)
    cast:SetWidth(cfg.w)
    cast:SetHeight(cfg.cast)
    if not st.castEdge then st.castEdge = edge(cast) end
    st.castEdge:Show()
    if st.castBorder then st.castBorder:SetAlpha(0) end
    if st.shield then st.shield:SetAlpha(0) end
    if st.icon then
        -- the spell icon at the left of both bars, as tall as the two together
        local s = cfg.h + cfg.castGap + cfg.cast
        st.icon:ClearAllPoints()
        st.icon:SetPoint("TOPRIGHT", health, "TOPLEFT", -3, 0)
        st.icon:SetWidth(s)
        st.icon:SetHeight(s)
    end
end

function P.Restore(st)
    if not st.styled then return end
    st.styled = false
    local k = st.kept
    putBack(st.health, k.health)
    putBack(st.cast, k.cast)
    for _, key in ipairs(REGIONS) do putBack(st[key], k[key]) end
    -- only the frame's size: the client places the frame itself (over the unit's head) every frame
    if st.grew then st.frame:SetWidth(k.frame.w); st.frame:SetHeight(k.frame.h); st.grew = nil end
    if st.healthEdge then st.healthEdge:Hide() end
    if st.castEdge then st.castEdge:Hide() end
end

function P.Apply(st)
    if st.foreign then return end
    local mode = P.Mode()
    local cfg = P.SIZES[mode]
    if not cfg then P.Restore(st); return end
    st.styled = true
    local f, health, k = st.frame, st.health, st.kept

    health:ClearAllPoints()
    health:SetPoint("CENTER", f, "CENTER", 0, -2)
    health:SetWidth(cfg.w)
    health:SetHeight(cfg.h)
    if not st.healthEdge then st.healthEdge = edge(health) end
    st.healthEdge:Show()

    -- the frame-shaped art goes; the edge carries threat (P.Tint)
    for _, key in ipairs({ "border", "glow", "elite" }) do
        if st[key] then st[key]:SetAlpha(0) end
    end
    if st.highlight then
        st.highlight:ClearAllPoints()
        st.highlight:SetAllPoints(health)
    end
    if st.name then
        st.name:ClearAllPoints()
        st.name:SetPoint("BOTTOM", health, "TOP", 0, cfg.gap)
        font(st.name, k.name, cfg.name)
    end
    if st.level then
        st.level:ClearAllPoints()
        st.level:SetPoint("LEFT", health, "RIGHT", 3, 0)
        font(st.level, k.level, cfg.level)
    end
    if st.skull then
        st.skull:ClearAllPoints()
        st.skull:SetPoint("LEFT", health, "RIGHT", 2, 0)
        st.skull:SetWidth(cfg.level + 4)
        st.skull:SetHeight(cfg.level + 4)
    end
    if st.raid then
        st.raid:ClearAllPoints()
        st.raid:SetPoint("BOTTOM", st.name or health, "TOP", 0, 2)
        st.raid:SetWidth(cfg.raid)
        st.raid:SetHeight(cfg.raid)
    end
    P.PlaceCast(st)

    -- the clickable area: the client's frame, grown only if it were smaller than the new layout
    local needW = cfg.w + 2 * (cfg.h + cfg.castGap + cfg.cast + 4)
    local needH = 2 * (2 + cfg.h / 2 + cfg.gap + cfg.name + 2)
    local fw, fh = f:GetWidth() or 0, f:GetHeight() or 0
    if fw < needW or fh < needH then
        st.grew = true
        if fw < needW then f:SetWidth(needW) end
        if fh < needH then f:SetHeight(needH) end
    end
    P.Tint(st)
end

-- The edge's colour: the threat glow's colour while the client shows it, light grey while a cast can't be
-- interrupted (on the cast bar's edge), dark otherwise.
function P.Tint(st)
    if not st.styled then return end
    local c = P.EDGE
    if st.glow and st.glow.IsShown and st.glow:IsShown() then
        local r, g, b = st.glow:GetVertexColor()
        c = { r or 1, g or 0, b or 0, 1 }
    end
    if st.healthEdge then st.healthEdge:SetTexture(unpack(c)) end
    if st.castEdge then
        local s = st.shield and st.shield.IsShown and st.shield:IsShown()
        st.castEdge:SetTexture(unpack(s and P.SHIELD or P.EDGE))
    end
end

function P.Scan()
    local WF = WorldFrame
    if not WF or not WF.GetNumChildren then return end
    local n = WF:GetNumChildren()
    if n ~= lastN then
        lastN = n
        local kids = { WF:GetChildren() }
        for i = 1, #kids do
            local f = kids[i]
            if not seen[f] then
                seen[f] = true
                if P.IsPlate(f) then P.Adopt(f) end
            end
        end
    end
    for _, st in ipairs(plates) do
        if st.frame:IsShown() then P.Tint(st) end
    end
end

function P.Plates() return plates end

function P.SetMode(m)
    if m ~= "off" and not P.SIZES[m] then return false end
    db().plates = m
    for _, st in ipairs(plates) do P.Apply(st) end
    return true
end

local driver = CreateFrame("Frame")
P.driver = driver
local acc = 0
local function onUpdate(_, elapsed)
    acc = acc + (elapsed or 0)
    if acc < P.TICK then return end
    acc = 0
    P.Scan()
end

function P.Start()
    if started then return end
    local other = P.Blocked()
    if other then P.blockedBy = other; return end
    started = true
    driver:SetScript("OnUpdate", onUpdate)
end

driver:RegisterEvent("PLAYER_ENTERING_WORLD")
driver:SetScript("OnEvent", function() P.Start() end)

local function say(text)
    if DEFAULT_CHAT_FRAME then DEFAULT_CHAT_FRAME:AddMessage("|cff3592cfNameplates:|r " .. text) end
end

SLASH_LEYLINEPLATES1 = "/plates"
SlashCmdList = SlashCmdList or {}
SlashCmdList["LEYLINEPLATES"] = function(msg)
    local m = (msg or ""):lower():gsub("^%s+", ""):gsub("%s+$", "")
    if P.blockedBy then
        say("left to " .. P.blockedBy .. ", which draws them.")
        return
    end
    if m == "" then
        say(P.Mode() .. ". /plates small | normal | off")
    elseif P.SetMode(m) then
        say(m == "off" and "the game's own again." or m .. ".")
    else
        say("/plates small | normal | off")
    end
end
