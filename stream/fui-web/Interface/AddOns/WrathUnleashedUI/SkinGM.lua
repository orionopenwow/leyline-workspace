-- SkinGM.lua (3.3.5a API): the Leyline look for the GM panel (AzerothAdmin, /aa) and MiniGM (/mgm).
-- Owner, 3 Nov (his shot: docs/den-ui/azerothadmin-before.png): "dramatically upgrade this UI too". AzerothAdmin is a
-- third-party GPL addon fetched as it is (addons/README.md); we never edit its files. This file restyles its window
-- from the outside once it exists: our dark panel with soft edges and a thin gold line, a small "LEYLINE GM" header
-- in place of the big logo, the tabs as pills with a gold underline, flat chips with hover and press states, the
-- On / Off columns as switches, slim sliders, the Parameter(s) box as a search field, and the GM tab's buttons
-- regrouped (Cheats & modes, Movement, Spells, Players & GMs, Characters & pets; the lookups along the bottom).
-- Every button keeps its own OnClick and tooltip: the skin only moves, sizes and paints. The window is 680 wide on
-- the GM tab (750 before); the other tabs keep their own layout (750) in the new look.
--
--   /aa skin off | on    the skin off (everything as AzerothAdmin drew it) or back on; /gmskin does the same
--   G.Apply() / G.Remove() / G.SetEnabled(on)          W.SkinGM, for the tests (stream/tests/test_wuui_gmskin.lua)
--
-- Names are AzerothAdmin v24's (tools/den-ui/azerothadmin-frames.lua lists them, read from its source). Nothing
-- here runs when AzerothAdmin isn't loaded. Style values are Style.lua's editor look (S.ED): one palette for the
-- Den editor and the GM panel. Only solid-colour textures and our own Media files.
local W = WrathUnleashedUI
if not W or not W.Style then return end
local S = W.Style
local ED = S.ED
local unpack = unpack or table.unpack

local G = { on = false, group = "main", tab = "main", state = {}, hooked = false }
W.SkinGM = G

-- sizes (UI units)
G.W_GM, G.W_WIDE = 680, 750      -- the window on the GM tab / on the other tabs (their own layouts need 750)
G.HEAD, G.TABS, G.FOOT = 36, 30, 64
G.MID_GM, G.MID = 336, 254       -- the content area's height on the GM tab / on the others (theirs)
G.CHIP_H = 22

local C = {
    panel = { 0.035, 0.037, 0.045, 0.95 },
    band = { 0.055, 0.058, 0.07, 0.97 },
    tile = ED.tile, over = { 0.13, 0.135, 0.155, 0.96 }, down = ED.goldFill,
    edge = ED.edge, gold = ED.gold, text = ED.text, muted = ED.muted, off = ED.off,
    dim = { 0.06, 0.062, 0.07, 0.9 }, field = { 0, 0, 0, 0.55 },
    pill = { 1, 1, 1, 0.035 }, pillOn = { 1, 1, 1, 0.08 },
}

local function say(msg)
    local f = DEFAULT_CHAT_FRAME
    if f and f.AddMessage then f:AddMessage("|cffe0a43aLeyline GM:|r " .. msg) end
end

-- ---------------------------------------------------------------- remembering what the skin changes
-- Each change to one of AzerothAdmin's (or MiniGM's) own parts first remembers the old value, once; G.Remove puts
-- them all back in reverse order. Our own textures, texts and frames are listed in `ours` and hidden.
local undo, ours = {}, {}
local function keep(obj, key, get, set)
    if not obj then return end
    local kept = rawget(obj, "lgmKept")
    if not kept then kept = {}; rawset(obj, "lgmKept", kept) end
    if kept[key] then return end
    kept[key] = true
    local v = { get(obj) }
    undo[#undo + 1] = function() kept[key] = nil; set(obj, unpack(v)) end
end
local function own(x) x.lgmOwn = true; ours[#ours + 1] = x; return x end

local function setAlpha(o, a)
    if not o or not o.SetAlpha then return end
    keep(o, "alpha", function(x) return x:GetAlpha() end, function(x, v) x:SetAlpha(v or 1) end)
    o:SetAlpha(a)
end
local function setText(o, t)
    if not o or not o.SetText then return end
    keep(o, "text", function(x) return x:GetText() end, function(x, v) x:SetText(v or "") end)
    o:SetText(t)
end
local function keepPoints(f, size)
    keep(f, "points", function(o)
        local p = {}
        local n = o.GetNumPoints and o:GetNumPoints() or 0
        for i = 1, n do p[i] = { o:GetPoint(i) } end
        return p
    end, function(o, p)
        o:ClearAllPoints()
        for _, q in ipairs(p or {}) do o:SetPoint(unpack(q)) end
    end)
    if size then
        keep(f, "size", function(o) return o:GetWidth(), o:GetHeight() end, function(o, w, h)
            if w and w > 0 then o:SetWidth(w) end
            if h and h > 0 then o:SetHeight(h) end
        end)
    end
end
-- f:SetPoint(...) after remembering its points (and size when w / h are given)
local function place(f, w, h, ...)
    if not f then return end
    keepPoints(f, w ~= nil or h ~= nil)
    f:ClearAllPoints()
    f:SetPoint(...)
    if w then f:SetWidth(w) end
    if h then f:SetHeight(h) end
end
local function textColor(fs, c)
    if not fs or not fs.SetTextColor then return end
    keep(fs, "tcolor", function(x) return x:GetTextColor() end, function(x, r, g, b, a) if r then x:SetTextColor(r, g, b, a or 1) end end)
    fs:SetTextColor(c[1], c[2], c[3], c[4] or 1)
end

-- our fonts: the game's own font file at our sizes and colours (one font object each, made once)
local fonts = {}
local function font(key, size, c)
    if fonts[key] then return fonts[key] end
    local o = CreateFont and CreateFont("LeylineGM" .. key)
    if o then
        local file = "Fonts\\FRIZQT__.TTF"
        local base = _G.GameFontHighlight
        if base and base.GetFont then local f = base:GetFont(); if type(f) == "string" and f ~= "" then file = f end end
        if o.SetFont then o:SetFont(file, size, "") end
        if o.SetShadowOffset then o:SetShadowOffset(1, -1) end
        if o.SetShadowColor then o:SetShadowColor(0, 0, 0, 0.9) end
        if o.SetTextColor and c then o:SetTextColor(c[1], c[2], c[3]) end
    end
    fonts[key] = o or GameFontHighlightSmall
    return fonts[key]
end
local function F_CHIP() return font("Chip", 12, C.text) end
local function F_CHIPHI() return font("ChipHi", 12, C.gold) end
local function F_CHIPOFF() return font("ChipOff", 12, C.off) end
local function F_TEXT() return font("Text", 13, C.text) end
local function F_SMALL() return font("Small", 11, C.muted) end

local function setFont(fs, o)
    if not fs or not fs.SetFontObject then return end
    keep(fs, "font", function(x) return x.GetFontObject and x:GetFontObject() end, function(x, v) if v then x:SetFontObject(v) end end)
    fs:SetFontObject(o)
end

-- a solid-colour texture of ours
local function solid(parent, layer, c, sub)
    local t = parent:CreateTexture(nil, layer or "BACKGROUND", nil, sub)
    t:SetTexture(unpack(c))
    return own(t)
end
local function edges(parent, c, layer, out)
    local e = {}
    out = out or 0
    for i, side in ipairs({ { "TOPLEFT", "TOPRIGHT", 0, -out, out }, { "BOTTOMLEFT", "BOTTOMRIGHT", 0, out, out },
                            { "TOPLEFT", "BOTTOMLEFT", 1, out, out }, { "TOPRIGHT", "BOTTOMRIGHT", 1, out, out } }) do
        local t = solid(parent, layer or "BORDER", c)
        if side[3] == 1 then
            local x = (i == 3) and -out or out
            t:SetPoint(side[1], parent, side[1], x, out); t:SetPoint(side[2], parent, side[2], x, -out); t:SetWidth(1)
        else
            local y = (i == 1) and out or -out
            t:SetPoint(side[1], parent, side[1], -out, y); t:SetPoint(side[2], parent, side[2], out, y); t:SetHeight(1)
        end
        e[i] = t
    end
    return e
end
local function text(parent, size, c, layer)
    local fs = parent:CreateFontString(nil, layer or "OVERLAY")
    fs:SetFontObject(size == 11 and F_SMALL() or (size == 12 and F_CHIP() or F_TEXT()))
    c = c or C.text
    fs:SetTextColor(c[1], c[2], c[3])
    fs:SetJustifyH("LEFT")
    return own(fs)
end

-- the upstream parts' own fills (FrameLib's colour texture, a template's art) go clear
local function clearFills(f)
    if not f or not f.GetRegions then return end
    for _, r in ipairs({ f:GetRegions() }) do
        if type(r) == "table" and not r.lgmOwn and r.GetObjectType and r:GetObjectType() == "Texture" then setAlpha(r, 0) end
    end
end
local function clearBackdrop(f)
    if not f or not f.SetBackdropColor then return end
    if f.GetBackdrop and not f:GetBackdrop() then return end
    keep(f, "backdrop", function(x)
        local a = { x.GetBackdropColor and x:GetBackdropColor() }
        local b = { x.GetBackdropBorderColor and x:GetBackdropBorderColor() }
        return a, b
    end, function(x, a, b)
        if a and a[1] then x:SetBackdropColor(unpack(a)) end
        if b and b[1] and x.SetBackdropBorderColor then x:SetBackdropBorderColor(unpack(b)) end
    end)
    f:SetBackdropColor(0, 0, 0, 0)
    if f.SetBackdropBorderColor then f:SetBackdropBorderColor(0, 0, 0, 0) end
end

-- ---------------------------------------------------------------- panels
-- A window: the near-black fill, a 1px subtle edge, a soft shadow (three fading rings outside) and the gold line
-- (under the header at `lineY`, or along the top).
local function panel(f, lineY)
    if not f then return end
    clearFills(f)
    clearBackdrop(f)
    local d = f.lgmPanel
    if not d then
        d = { all = {} }
        f.lgmPanel = d
        d.fill = solid(f, "BACKGROUND", C.panel, -8)
        d.fill:SetAllPoints(f)
        for i, a in ipairs({ 0.32, 0.18, 0.08 }) do
            for _, t in ipairs(edges(f, { 0, 0, 0, a }, "BACKGROUND", i)) do d.all[#d.all + 1] = t end
        end
        for _, t in ipairs(edges(f, C.edge, "BORDER")) do d.all[#d.all + 1] = t end
        d.line = solid(f, "ARTWORK", { C.gold[1], C.gold[2], C.gold[3], 0.85 })
        d.line:SetHeight(1)
        d.all[#d.all + 1] = d.fill
        d.all[#d.all + 1] = d.line
    end
    d.line:ClearAllPoints()
    d.line:SetPoint("TOPLEFT", f, "TOPLEFT", 1, -(lineY or 0) - 1)
    d.line:SetPoint("TOPRIGHT", f, "TOPRIGHT", -1, -(lineY or 0) - 1)
    for _, t in ipairs(d.all) do t:Show() end
    return d
end

-- ---------------------------------------------------------------- chips (AzerothAdmin's and MiniGM's buttons)
local function enabled(b) return not b.IsEnabled or (b:IsEnabled() and true or false) end
local function paintChip(b)
    local d = b.lgm
    if not d or not G.on then return end
    local en = enabled(b)
    local fs = b.GetFontString and b:GetFontString()
    if d.kind == "half" then
        if d.row then G.PaintSwitch(d.row) end
        return
    end
    local c, e, tc
    if d.kind == "tab" then
        c = (d.active or b.lgmOver) and C.pillOn or C.pill
        e = d.active and { C.gold[1], C.gold[2], C.gold[3], 0.45 } or C.edge
        tc = d.active and C.gold or (b.lgmOver and C.text or C.muted)
        if d.under then if d.active then d.under:Show() else d.under:Hide() end end
    else
        c = (not en) and C.dim or ((d.active or b.lgmDown) and C.down or (b.lgmOver and C.over or C.tile))
        e = (en and (d.active or b.lgmDown)) and C.gold or C.edge
        tc = (not en) and C.off or ((d.active or b.lgmOver) and C.gold or C.text)
    end
    d.bg:SetTexture(unpack(c))
    for _, t in ipairs(d.edge) do t:SetTexture(unpack(e)) end
    if fs then fs:SetTextColor(tc[1], tc[2], tc[3]) end
end
G.PaintChip = paintChip

local function hookStates(b)
    if b.lgmHooked then return end
    b.lgmHooked = true
    if b.HookScript then
        b:HookScript("OnEnter", function(self) self.lgmOver = true; paintChip(self) end)
        b:HookScript("OnLeave", function(self) self.lgmOver, self.lgmDown = nil, nil; paintChip(self) end)
        b:HookScript("OnMouseDown", function(self) self.lgmDown = true; paintChip(self) end)
        b:HookScript("OnMouseUp", function(self) self.lgmDown = nil; paintChip(self) end)
    end
    if hooksecurefunc then
        if b.Enable then hooksecurefunc(b, "Enable", paintChip) end
        if b.Disable then hooksecurefunc(b, "Disable", paintChip) end
    end
end

-- the textures a button brought with it: FrameLib's "<name>_texture", a template's normal / pushed / highlight art
local function buttonArt(b)
    local out = {}
    local n = b.GetName and b:GetName()
    if n and _G[n .. "_texture"] then out[#out + 1] = _G[n .. "_texture"] end
    -- a template's art only when it is the stock panel button's (a close button's or an icon button's art stays)
    local function panelArt(t)
        local file = type(t) == "table" and not t.lgmOwn and t.GetTexture and t:GetTexture()
        return type(file) == "string" and file:find("UI%-Panel%-Button") ~= nil
    end
    local seen = {}
    for _, get in ipairs({ "GetNormalTexture", "GetPushedTexture", "GetHighlightTexture", "GetDisabledTexture" }) do
        local t = b[get] and b[get](b)
        if panelArt(t) and not seen[t] then seen[t] = true; out[#out + 1] = t end
    end
    if b.GetRegions then
        for _, r in ipairs({ b:GetRegions() }) do
            if r.GetObjectType and r:GetObjectType() == "Texture" and panelArt(r) and not seen[r] then seen[r] = true; out[#out + 1] = r end
        end
    end
    return out
end

-- b as one of our chips (kind nil), a tab pill ("tab"), or half of a switch ("half": no fill of its own)
local function chip(b, kind, label)
    if not b or b.lgmOwn then return end
    local d = b.lgm
    if not d then
        d = { all = {} }
        b.lgm = d
        d.bg = solid(b, "BACKGROUND", C.tile, 1)
        d.bg:SetAllPoints(b)
        d.edge = edges(b, C.edge, "BORDER")
        d.all[1] = d.bg
        for _, t in ipairs(d.edge) do d.all[#d.all + 1] = t end
        if kind == "tab" then
            d.under = solid(b, "OVERLAY", C.gold)
            d.under:SetHeight(2)
            d.under:SetPoint("TOPLEFT", b, "BOTTOMLEFT", 6, -3)
            d.under:SetPoint("TOPRIGHT", b, "BOTTOMRIGHT", -6, -3)
        end
    end
    d.kind = kind
    for _, t in ipairs(buttonArt(b)) do setAlpha(t, 0) end
    for _, t in ipairs(d.all) do if kind == "half" then t:Hide() else t:Show() end end
    if d.under then d.under:Hide() end
    if b.SetNormalFontObject then
        keep(b, "fonts", function(x)
            return x.GetNormalFontObject and x:GetNormalFontObject(), x.GetHighlightFontObject and x:GetHighlightFontObject(),
                x.GetDisabledFontObject and x:GetDisabledFontObject()
        end, function(x, n, h, dis)
            if n then x:SetNormalFontObject(n) end
            if h and x.SetHighlightFontObject then x:SetHighlightFontObject(h) end
            if dis and x.SetDisabledFontObject then x:SetDisabledFontObject(dis) end
        end)
        b:SetNormalFontObject(F_CHIP())
        if b.SetHighlightFontObject then b:SetHighlightFontObject(F_CHIPHI()) end
        if b.SetDisabledFontObject then b:SetDisabledFontObject(F_CHIPOFF()) end
    end
    local fs = b.GetFontString and b:GetFontString()
    if fs then
        setFont(fs, F_CHIP())
        textColor(fs, C.text)
    end
    if label then setText(b, label) end
    hookStates(b)
    paintChip(b)
    return b
end
G.Chip = chip

-- an EditBox as our field: the template's three textures and any backdrop clear, our dark fill and edge (gold
-- while typing)
local function field(e)
    if not e or e.lgmOwn then return end
    local n = e.GetName and e:GetName()
    for _, k in ipairs({ "Left", "Middle", "Right", "Mid" }) do if n and _G[n .. k] then setAlpha(_G[n .. k], 0) end end
    clearBackdrop(e)
    local d = e.lgm
    if not d then
        d = { all = {} }
        e.lgm = d
        d.bg = solid(e, "BACKGROUND", C.field, 1)
        d.bg:SetAllPoints(e)
        d.edge = edges(e, C.edge, "BORDER")
        d.all[1] = d.bg
        for _, t in ipairs(d.edge) do d.all[#d.all + 1] = t end
        if e.HookScript then
            local function edge(self, on)
                if not G.on then return end
                for _, t in ipairs(self.lgm.edge) do t:SetTexture(unpack(on and C.gold or C.edge)) end
                if self.lgm.hint then G.PaintHint(self) end
            end
            e:HookScript("OnEditFocusGained", function(self) self.lgmFocus = true; edge(self, true) end)
            e:HookScript("OnEditFocusLost", function(self) self.lgmFocus = nil; edge(self, false) end)
            e:HookScript("OnTextChanged", function(self) if self.lgm.hint then G.PaintHint(self) end end)
        end
    end
    for _, t in ipairs(d.all) do t:Show() end
    if e.SetFontObject then
        keep(e, "font", function(x) return x.GetFontObject and x:GetFontObject() end, function(x, v) if v then x:SetFontObject(v) end end)
        e:SetFontObject(F_TEXT())
    end
    return e
end

function G.PaintHint(e)
    local d = e.lgm
    if not d or not d.hint then return end
    local t = e.GetText and e:GetText()
    if G.on and (t == nil or t == "") and not e.lgmFocus then d.hint:Show() else d.hint:Hide() end
end

-- a Slider: the template's backdrop and labels clear, a 4px track, the filled part and the thumb in gold
local function slider(s)
    if not s or s.lgmOwn then return end
    clearBackdrop(s)
    local n = s.GetName and s:GetName()
    if n then
        setAlpha(_G[n .. "Low"], 0); setAlpha(_G[n .. "High"], 0)
        local t = _G[n .. "Text"]
        if t then setFont(t, F_CHIP()); textColor(t, C.text) end
    end
    local thumb = s.GetThumbTexture and s:GetThumbTexture()
    if thumb then
        keep(thumb, "thumb", function(x) return x:GetTexture(), x:GetWidth(), x:GetHeight() end, function(x, f, w, h)
            if f then x:SetTexture(f) end
            if w and w > 0 then x:SetWidth(w) end
            if h and h > 0 then x:SetHeight(h) end
        end)
        thumb:SetTexture(unpack(C.gold)); thumb:SetWidth(8); thumb:SetHeight(14)
    end
    local d = s.lgm
    if not d then
        d = { all = {} }
        s.lgm = d
        d.track = solid(s, "BACKGROUND", C.tile, 1)
        d.track:SetPoint("LEFT", s, "LEFT", 0, 0); d.track:SetPoint("RIGHT", s, "RIGHT", 0, 0); d.track:SetHeight(4)
        d.fill = solid(s, "BORDER", { C.gold[1], C.gold[2], C.gold[3], 0.7 })
        d.fill:SetPoint("LEFT", s, "LEFT", 0, 0); d.fill:SetHeight(4)
        d.all = { d.track, d.fill }
        if s.HookScript then s:HookScript("OnValueChanged", function(self) G.PaintSlider(self) end) end
    end
    for _, t in ipairs(d.all) do t:Show() end
    G.PaintSlider(s)
end
function G.PaintSlider(s)
    local d = s.lgm
    if not d or not G.on then return end
    local lo, hi = 0, 1
    if s.GetMinMaxValues then lo, hi = s:GetMinMaxValues() end
    lo, hi = tonumber(lo) or 0, tonumber(hi) or 1
    local v = tonumber(s.GetValue and s:GetValue()) or lo
    local k = (hi > lo) and math.max(0, math.min(1, (v - lo) / (hi - lo))) or 0
    local w = tonumber(s.GetWidth and s:GetWidth()) or 0
    d.fill:SetWidth(math.max(1, w * k))
end

-- ---------------------------------------------------------------- switches
-- The GM tab's On / Off pairs: one switch per row. The Off button is the track's left half, the On button its
-- right half (both keep their own commands); the knob shows what was last sent. Before that the knob sits in the
-- middle: the panel can't know the server's state.
local function paintSwitch(row)
    if not row or not G.on then return end
    local st = row.check and (row.check:GetChecked() and "on" or "off") or G.state[row.key]
    local over = (row.on and row.on.lgmOver) or (row.off and row.off.lgmOver) or (row.check and row.check.lgmOver)
    local fill = st == "on" and { C.gold[1], C.gold[2], C.gold[3], 0.28 } or C.tile
    local e = st == "on" and C.gold or (over and C.muted or C.edge)
    row.track:SetTexture(unpack(fill))
    for _, t in ipairs(row.edge) do t:SetTexture(unpack(e)) end
    row.knob:ClearAllPoints()
    if st == "on" then
        row.knob:SetPoint("RIGHT", row.track, "RIGHT", -2, 0); row.knob:SetTexture(unpack(C.gold))
    elseif st == "off" then
        row.knob:SetPoint("LEFT", row.track, "LEFT", 2, 0); row.knob:SetTexture(unpack(C.off))
    else
        row.knob:SetPoint("CENTER", row.track, "CENTER", 0, 0); row.knob:SetTexture(C.muted[1], C.muted[2], C.muted[3], 0.6)
    end
    if row.label then
        local c = st == "on" and C.text or C.muted
        if over then c = C.text end
        row.label:SetTextColor(c[1], c[2], c[3])
    end
end
G.PaintSwitch = paintSwitch

-- the track, knob (and label) of a switch, drawn on `holder`, the track's box at the given point
local function switchArt(row, holder, w, h, ...)
    row.track = solid(holder, "ARTWORK", C.tile)
    row.track:SetWidth(w); row.track:SetHeight(h)
    row.track:SetPoint(...)
    row.edge = edges(holder, C.edge, "OVERLAY")
    -- the edges were made around the holder: move them onto the track
    for i, t in ipairs(row.edge) do
        t:ClearAllPoints()
        if i == 1 then t:SetPoint("TOPLEFT", row.track, "TOPLEFT"); t:SetPoint("TOPRIGHT", row.track, "TOPRIGHT")
        elseif i == 2 then t:SetPoint("BOTTOMLEFT", row.track, "BOTTOMLEFT"); t:SetPoint("BOTTOMRIGHT", row.track, "BOTTOMRIGHT")
        elseif i == 3 then t:SetPoint("TOPLEFT", row.track, "TOPLEFT"); t:SetPoint("BOTTOMLEFT", row.track, "BOTTOMLEFT")
        else t:SetPoint("TOPRIGHT", row.track, "TOPRIGHT"); t:SetPoint("BOTTOMRIGHT", row.track, "BOTTOMRIGHT") end
    end
    row.knob = solid(holder, "OVERLAY", C.muted, 2)
    row.knob:SetWidth(h - 4); row.knob:SetHeight(h - 4)
end

-- a CheckButton as a switch (it keeps its own click and label)
local function checkSwitch(cb)
    if not cb or cb.lgmOwn then return end
    local row = cb.lgmRow
    if not row then
        row = { check = cb }
        cb.lgmRow = row
        switchArt(row, cb, 34, 16, "LEFT", cb, "LEFT", 1, 0)
        row.all = { row.track, row.knob }
        for _, t in ipairs(row.edge) do row.all[#row.all + 1] = t end
        if cb.HookScript then
            cb:HookScript("OnClick", function() paintSwitch(row) end)
            cb:HookScript("OnEnter", function(self) self.lgmOver = true; paintSwitch(row) end)
            cb:HookScript("OnLeave", function(self) self.lgmOver = nil; paintSwitch(row) end)
        end
        if hooksecurefunc and cb.SetChecked then hooksecurefunc(cb, "SetChecked", function() paintSwitch(row) end) end
    end
    for _, get in ipairs({ "GetNormalTexture", "GetPushedTexture", "GetHighlightTexture", "GetCheckedTexture",
                           "GetDisabledCheckedTexture", "GetDisabledTexture" }) do
        local t = cb[get] and cb[get](cb)
        if type(t) == "table" and not t.lgmOwn then setAlpha(t, 0) end
    end
    for _, t in ipairs(row.all) do t:Show() end
    local n = cb.GetName and cb:GetName()
    local label = n and _G[n .. "Text"]
    if label then setFont(label, F_TEXT()); textColor(label, C.text); row.label = label end
    paintSwitch(row)
    return row
end

-- ---------------------------------------------------------------- AzerothAdmin: the GM tab's layout
-- (relative to ma_midframe's top left; y down is negative)
local TOGGLES = {   -- key, On button, Off button, label (the order: what a GM reaches for first)
    { "gm", "ma_gmonbutton", "ma_gmoffbutton", "GM mode" },
    { "fly", "ma_flyonbutton", "ma_flyoffbutton", "Fly" },
    { "spectator", "ma_hoveronbutton", "ma_hoveroffbutton", "Spectator" },
    { "whisper", "ma_whisperonbutton", "ma_whisperoffbutton", "Whispers" },
    { "invisible", "ma_invisibleonbutton", "ma_invisibleoffbutton", "Invisible" },
    { "god", "ma_cheatgodonbutton", "ma_cheatgodoffbutton", "God mode" },
    { "cooldown", "ma_cheatcooldownonbutton", "ma_cheatcooldownoffbutton", "No cooldowns" },
    { "casttime", "ma_cheatcasttimeonbutton", "ma_cheatcasttimeoffbutton", "No cast times" },
    { "power", "ma_cheatpoweronbutton", "ma_cheatpoweroffbutton", "No power cost" },
    { "waterwalk", "ma_waterwalkonbutton", "ma_waterwalkoffbutton", "Water walking" },
    { "taxi", "ma_cheattaxionbutton", "ma_cheattaxioffbutton", "All flight paths" },
}
G.TOGGLES = TOGGLES
-- the chip sections to the right: header, y, { button, label } in rows of five
local SECTIONS = {
    { "Movement", -42, {
        { "ma_goxybutton", "Go XY" }, { "ma_goxyzbutton", "Go XYZ" }, { "ma_gozonexybutton", "Go zone XY" },
        { "ma_gotriggerbutton", "Go trigger" }, { "ma_gotaxinodebutton", "Go taxi node" },
        { "ma_lookuptaxibutton", "Lookup taxi" }, { "ma_lookupzonebutton", "Lookup zone" },
        { "ma_teleaddbutton", "Tele add" }, { "ma_teledelbutton", "Tele del" } } },
    { "Spells", -138, {
        { "ma_castbutton", "Cast" }, { "ma_castbackbutton", "Cast back" }, { "ma_castdistbutton", "Cast dist" },
        { "ma_castselfbutton", "Cast self" }, { "ma_casttargetbutton", "Cast target" } } },
    { "Players & GMs", -186, {
        { "ma_acctaddonbutton", "Acct addon" }, { "ma_acctgmlvlbutton", "Acct GM lvl" }, { "ma_acctchangebutton", "Acct change" },
        { "ma_displaylevelbutton", "My acct lvl" }, { "ma_gmlistbutton", "GM list" },
        { "ma_gmingamebutton", "GMs online" }, { "ma_gmnotifybutton", "GM notify" }, { "ma_setjail_a_button", "Set jail A" },
        { "ma_setjail_h_button", "Set jail H" }, { "ma_screenshotbutton", "Screenshot" } } },
    { "Characters & pets", -258, {
        { "ma_listitembutton", "List item" }, { "ma_charlistdeletedbutton", "Deleted list" },
        { "ma_chardeletedrestorebutton", "Restore char" }, { "ma_cheatstatusbutton", "Cheat status" },
        { "ma_petcreatebutton", "Pet create" }, { "ma_petlearnbutton", "Pet learn" }, { "ma_petunlearnbutton", "Pet unlearn" },
        { "ma_pettpbutton", "Pet TP" } } },
}
G.SECTIONS = SECTIONS
local LOOKUPS = {
    { "ma_itembutton", "Items" }, { "ma_itemsetbutton", "Item sets" }, { "ma_spellbutton", "Spells" },
    { "ma_skillbutton", "Skills" }, { "ma_questbutton", "Quests" }, { "ma_objectbutton", "Objects" },
    { "ma_creaturebutton", "Creatures" }, { "ma_telesearchbutton", "Teleports" }, { "ma_sendmailbutton", "Mail" },
}
G.LOOKUPS = LOOKUPS
local TABS = { "main", "char", "npc", "go", "tele", "ticket", "misc", "server" }
local AREA_X, AREA_W, COL_X, COL_W = 220, 448, 12, 196
local CHIP_W, GAP = 86, 4

local function sectionHeader(holder, label, x, y, w)
    local fs = text(holder, 11, C.muted)
    fs:SetText(label:upper())
    fs:SetPoint("TOPLEFT", ma_midframe, "TOPLEFT", x, y)
    local line = solid(holder, "ARTWORK", C.edge)
    line:SetHeight(1)
    line:SetPoint("LEFT", fs, "RIGHT", 8, 0)
    line:SetPoint("RIGHT", ma_midframe, "TOPLEFT", x + w, y - 6)
    return fs
end

local function layoutGMTab()
    local mid = ma_midframe
    local deco = G.deco
    if not deco then
        -- a frame inside the GM tab (a child of one of its buttons, so it shows and hides with the tab) for the
        -- section headers, the switch rows' labels and tracks; it takes no clicks
        deco = own(CreateFrame("Frame", nil, ma_displaylevelbutton))
        G.deco = deco
        deco:SetAllPoints(mid)
        sectionHeader(deco, "Cheats & modes", COL_X, -10, COL_W)
        for _, s in ipairs(SECTIONS) do sectionHeader(deco, s[1], AREA_X, s[2], AREA_W) end
        G.rows = {}
        for i, t in ipairs(TOGGLES) do
            local y = -28 - (i - 1) * 19
            local row = { key = t[1] }
            row.label = text(deco, 13, C.muted)
            row.label:SetText(t[4])
            row.label:SetPoint("LEFT", mid, "TOPLEFT", COL_X + 2, y - 9)
            switchArt(row, deco, 36, 16, "TOPRIGHT", mid, "TOPLEFT", COL_X + COL_W, y - 1)
            G.rows[t[1]] = row
        end
        -- the parameter field's hint and search mark
        G.hint = text(deco, 13, C.off)
        G.hint:SetText("Parameters: a name, an id or a value")
        G.mark = own(deco:CreateTexture(nil, "OVERLAY"))
        G.mark:SetWidth(14); G.mark:SetHeight(14)
        -- the search mark from our own icon set (Media\\Icons, tools/den-ui/icons); without it, no mark
        G.markOk = G.mark:SetTexture(S.MEDIA .. "Icons\\search") and true or false
        G.mark:SetVertexColor(C.muted[1], C.muted[2], C.muted[3])
        if not G.markOk then G.mark.lgmHidden = true; G.mark:Hide() end
    end
    deco:Show()
    -- the switches: Off is the track's left half, On its right half
    for i, t in ipairs(TOGGLES) do
        local row = G.rows[t[1]]
        local on, off = _G[t[2]], _G[t[3]]
        row.on, row.off = on, off
        if on and off then
            for _, b in ipairs({ off, on }) do
                chip(b, "half")
                b.lgm.row = row
                setText(b, "")
            end
            place(off, 18, 16, "LEFT", row.track, "LEFT", 0, 0)
            place(on, 18, 16, "RIGHT", row.track, "RIGHT", 0, 0)
            if not on.lgmSwitch then
                on.lgmSwitch, off.lgmSwitch = true, true
                if on.HookScript then
                    on:HookScript("OnClick", function() G.state[row.key] = "on"; paintSwitch(row) end)
                    off:HookScript("OnClick", function() G.state[row.key] = "off"; paintSwitch(row) end)
                end
            end
        end
        paintSwitch(row)
    end
    -- insta-kill: the last switch row, its CheckButton as the track
    if ma_instantkillbutton then
        local y = -28 - #TOGGLES * 19
        checkSwitch(ma_instantkillbutton)
        place(ma_instantkillbutton, 36, 16, "TOPRIGHT", mid, "TOPLEFT", COL_X + COL_W, y - 1)
        local label = _G.ma_instantkillbuttonText
        if label then
            place(label, nil, nil, "LEFT", mid, "TOPLEFT", COL_X + 2, y - 9)
            setText(label, "Insta-kill creatures")
        end
    end
    -- speed and scale: the label above a slim track, Reset beside it
    for i, s in ipairs({ { "ma_speedslider", "ma_speedsliderreset_button" }, { "ma_scaleslider", "ma_scalesliderreset_button" } }) do
        local sl, reset = _G[s[1]], _G[s[2]]
        local y = -282 - (i - 1) * 32      -- the track's top; its label just above
        if sl then
            slider(sl)
            place(sl, 140, 14, "TOPLEFT", mid, "TOPLEFT", COL_X, y)
            local t = _G[s[1] .. "Text"]
            if t then place(t, nil, nil, "BOTTOMLEFT", sl, "TOPLEFT", 0, 2) end
            G.PaintSlider(sl)
        end
        if reset then chip(reset, nil, "Reset"); place(reset, 50, 20, "TOPLEFT", mid, "TOPLEFT", COL_X + COL_W - 50, y + 3) end
    end
    -- the parameter field: full width over the chips, Clear inside its right end
    if ma_parameter then
        field(ma_parameter)
        place(ma_parameter, AREA_W, 24, "TOPLEFT", mid, "TOPLEFT", AREA_X, -8)
        if ma_parameter.SetTextInsets then
            keep(ma_parameter, "insets", function(x) if x.GetTextInsets then return x:GetTextInsets() end end,
                function(x, l, r, t, b) x:SetTextInsets(l or 4, r or 4, t or 2, b or 2) end)
            ma_parameter:SetTextInsets(G.markOk and 26 or 9, 64, 0, 0)
        end
        ma_parameter.lgm.hint = G.hint
        G.hint:ClearAllPoints(); G.hint:SetPoint("LEFT", ma_parameter, "LEFT", G.markOk and 26 or 9, 0)
        G.mark:ClearAllPoints(); G.mark:SetPoint("LEFT", ma_parameter, "LEFT", 7, 0)
        G.PaintHint(ma_parameter)
    end
    if ma_parametertext then setAlpha(ma_parametertext, 0) end
    if ma_gmclearbutton then
        chip(ma_gmclearbutton, nil, "Clear")
        place(ma_gmclearbutton, 54, 18, "RIGHT", ma_parameter or mid, "RIGHT", -3, 0)
    end
    -- the chip sections
    for _, s in ipairs(SECTIONS) do
        for i, e in ipairs(s[3]) do
            local b = _G[e[1]]
            if b then
                local col, r = (i - 1) % 5, math.floor((i - 1) / 5)
                chip(b, nil, e[2])
                place(b, CHIP_W, G.CHIP_H, "TOPLEFT", mid, "TOPLEFT", AREA_X + col * (CHIP_W + GAP), s[2] - 16 - r * 24)
            end
        end
    end
    -- the grid navigator: a row under Movement (its step, then N S E W)
    local gy = -42 - 16 - 2 * 24
    if ma_gridnavigatortext then
        setFont(ma_gridnavigatortext, F_SMALL()); textColor(ma_gridnavigatortext, C.muted)
        setText(ma_gridnavigatortext, "GRID STEP")
        place(ma_gridnavigatortext, nil, nil, "LEFT", mid, "TOPLEFT", AREA_X, gy - 11)
    end
    if ma_gridnavieditbox then
        field(ma_gridnavieditbox)
        place(ma_gridnavieditbox, 44, G.CHIP_H, "TOPLEFT", mid, "TOPLEFT", AREA_X + 74, gy)
    end
    for i, n in ipairs({ "ma_gridnaviaheadbutton", "ma_gridnavibackbutton", "ma_gridnavirightbutton", "ma_gridnavileftbutton" }) do
        local b = _G[n]
        if b then
            chip(b)
            place(b, 28, G.CHIP_H, "TOPLEFT", mid, "TOPLEFT", AREA_X + 126 + (i - 1) * 32, gy)
        end
    end
end

-- ---------------------------------------------------------------- AzerothAdmin: the window
local function layoutFooter(w)
    local inner = w - 24
    if ma_leftframe then
        clearFills(ma_leftframe)
        place(ma_leftframe, nil, 24, "BOTTOMLEFT", ma_bgframe, "BOTTOMLEFT", 12, 34)
        ma_leftframe:SetPoint("BOTTOMRIGHT", ma_bgframe, "BOTTOMRIGHT", -12, 34)
        if not G.lookLabel then
            G.lookLabel = text(ma_leftframe, 11, C.muted)
            G.lookLabel:SetText("LOOKUP")
            G.lookLabel:SetPoint("LEFT", ma_leftframe, "LEFT", 0, 0)
        end
        G.lookLabel:Show()
        local cw = math.floor((inner - 52 - (#LOOKUPS - 1) * GAP) / #LOOKUPS)
        for i, e in ipairs(LOOKUPS) do
            local b = _G[e[1]]
            if b then
                chip(b, nil, e[2])
                place(b, cw, G.CHIP_H, "LEFT", ma_leftframe, "LEFT", 52 + (i - 1) * (cw + GAP), 0)
            end
        end
    end
    if ma_rightframe then
        clearFills(ma_rightframe)
        place(ma_rightframe, nil, 20, "BOTTOMLEFT", ma_bgframe, "BOTTOMLEFT", 12, 8)
        ma_rightframe:SetPoint("BOTTOMRIGHT", ma_bgframe, "BOTTOMRIGHT", -12, 8)
        if ma_inforefreshbutton then chip(ma_inforefreshbutton, nil, "Refresh"); place(ma_inforefreshbutton, 64, 20, "RIGHT", ma_rightframe, "RIGHT", 0, 0) end
        local prev
        for _, n in ipairs({ "ma_infoonlinetext", "ma_infomaxonlinetext", "ma_infouptimetext", "ma_inforevisiontext" }) do
            local fs = _G[n]
            if fs then
                setFont(fs, F_SMALL()); textColor(fs, C.muted)
                if prev then place(fs, nil, nil, "LEFT", prev, "RIGHT", 16, 0) else place(fs, nil, nil, "LEFT", ma_rightframe, "LEFT", 0, 0) end
                prev = fs
            end
        end
        if _G.ma_inforevisiontext and ma_inforefreshbutton then
            _G.ma_inforevisiontext:SetPoint("RIGHT", ma_inforefreshbutton, "LEFT", -10, 0)
        end
    end
    if G.footBand then
        G.footBand:ClearAllPoints()
        G.footBand:SetPoint("BOTTOMLEFT", ma_bgframe, "BOTTOMLEFT", 1, 1)
        G.footBand:SetPoint("BOTTOMRIGHT", ma_bgframe, "BOTTOMRIGHT", -1, 1)
        G.footBand:SetHeight(G.FOOT - 2)
    end
end

-- the window's size for a tab: compact on the GM tab, as wide as the other tabs' own layouts need elsewhere. It
-- grows to the right and down from where it is (its top left stays put).
function G.Size(group)
    if not G.on or not ma_bgframe then return end
    local gm = group == "main"
    local w, mid = gm and G.W_GM or G.W_WIDE, gm and G.MID_GM or G.MID
    keepPoints(ma_bgframe, true)
    local l, t = ma_bgframe:GetLeft(), ma_bgframe:GetTop()
    if l and t then
        ma_bgframe:ClearAllPoints()
        ma_bgframe:SetPoint("TOPLEFT", UIParent, "BOTTOMLEFT", l, t)
    end
    ma_bgframe:SetWidth(w)
    ma_bgframe:SetHeight(G.HEAD + G.TABS + mid + G.FOOT)
    if ma_midframe then ma_midframe:SetHeight(mid) end
    layoutFooter(w)
end

function G.PaintTabs(group)
    G.tab = group or G.tab
    for _, g in ipairs(TABS) do
        local b = _G["ma_tabbutton_" .. g]
        if b and b.lgm then
            b.lgm.active = (g == G.tab)
            paintChip(b)
        end
    end
end

-- AzerothAdmin's header text with its colour codes taken out while the skin is on (it sets them again itself)
local function plain(s) return (tostring(s or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", "")) end
local function quietHeader(fs)
    if not fs or fs.lgmQuiet or not hooksecurefunc then return end
    fs.lgmQuiet = true
    hooksecurefunc(fs, "SetText", function(self, t)
        if self.lgmBusy then return end
        self.lgmRaw = t
        if G.on and type(t) == "string" and t:find("|c") then
            self.lgmBusy = true; self:SetText(plain(t)); self.lgmBusy = nil
        end
    end)
end

local function skinTree(f, depth)
    -- every other part of the window (the other tabs, the popups): chips, fields, sliders, switches
    depth = depth or 0
    if not f or depth > 6 or not f.GetChildren then return end
    for _, c in ipairs({ f:GetChildren() }) do
        if type(c) == "table" and not c.lgmOwn and c.GetObjectType then
            local kind = c:GetObjectType()
            local n = c.GetName and c:GetName()
            if kind == "CheckButton" then checkSwitch(c)
            elseif kind == "Button" and n and _G[n .. "_texture"] then
                if not c.lgm then chip(c) elseif c.lgm.kind ~= "half" then chip(c, c.lgm.kind) end
            elseif kind == "EditBox" then field(c)
            elseif kind == "Slider" then slider(c)
            end
            -- a dropdown (UIDropDownMenuTemplate) keeps its own styling (AzerothAdmin's DropDownStyle.lua)
            if not (n and _G[n .. "Button"] and _G[n .. "Middle"]) then skinTree(c, depth + 1) end
        end
    end
    if f.GetRegions then
        for _, r in ipairs({ f:GetRegions() }) do
            if type(r) == "table" and not r.lgmOwn and r.GetObjectType and r:GetObjectType() == "FontString" and not rawget(r, "lgmKept") then
                -- the stock gold labels in our text colour
                local cr, cg, cb = r:GetTextColor()
                if type(cr) == "number" and math.abs(cr - 1) < 0.06 and math.abs(cg - 0.82) < 0.06 and cb < 0.06 then textColor(r, C.text) end
            end
        end
    end
end

local function skinWindow()
    local bg = ma_bgframe
    panel(bg, G.HEAD)
    for _, n in ipairs({ "ma_menubgframe", "ma_topframe", "ma_midframe", "ma_leftframe", "ma_rightframe" }) do clearFills(_G[n]) end
    if ma_logoframe then setAlpha(ma_logoframe, 0) end
    -- the header: our mark and "LEYLINE GM" where the big logo was; the character / realm lines, Reload UI and the
    -- close button on the right
    place(ma_topframe, nil, G.HEAD, "TOPLEFT", bg, "TOPLEFT", 0, 0)
    ma_topframe:SetPoint("TOPRIGHT", bg, "TOPRIGHT", 0, 0)
    if not G.head then
        G.head = own(CreateFrame("Frame", nil, ma_topframe))
        G.head:SetAllPoints(ma_topframe)
        -- the band on the header frame itself, under AzerothAdmin's own header texts (G.head is drawn over them)
        G.headBand = solid(ma_topframe, "BACKGROUND", C.band, 1)
        G.headBand:SetPoint("TOPLEFT", ma_topframe, "TOPLEFT", 1, -1); G.headBand:SetPoint("BOTTOMRIGHT", ma_topframe, "BOTTOMRIGHT", -1, 0)
        local mark = own(G.head:CreateTexture(nil, "ARTWORK"))
        mark:SetTexture(S.MARK); mark:SetWidth(20); mark:SetHeight(20)
        mark:SetPoint("LEFT", ma_topframe, "LEFT", 12, 0)
        G.title = text(G.head, 13, C.text)
        G.title:SetText("LEYLINE |cffe0a43aGM|r")
        G.title:SetPoint("LEFT", mark, "RIGHT", 8, 0)
        G.footBand = solid(bg, "BACKGROUND", C.band, -7)
        G.footLine = solid(bg, "ARTWORK", C.edge)
        G.footLine:SetHeight(1)
        G.footLine:SetPoint("BOTTOMLEFT", bg, "BOTTOMLEFT", 1, G.FOOT - 1)
        G.footLine:SetPoint("BOTTOMRIGHT", bg, "BOTTOMRIGHT", -1, G.FOOT - 1)
        G.tabLine = solid(bg, "ARTWORK", C.edge)
        G.tabLine:SetHeight(1)
        G.tabLine:SetPoint("TOPLEFT", bg, "TOPLEFT", 1, -(G.HEAD + G.TABS))
        G.tabLine:SetPoint("TOPRIGHT", bg, "TOPRIGHT", -1, -(G.HEAD + G.TABS))
    end
    G.head:Show()
    if ma_revtext then
        setFont(ma_revtext, F_SMALL()); textColor(ma_revtext, C.muted)
        setText(ma_revtext, "AzerothAdmin " .. tostring(MINOR_VERSION or ""):gsub("^0$", ""))
        place(ma_revtext, nil, nil, "LEFT", G.title, "RIGHT", 10, -1)
    end
    if ma_closebutton then chip(ma_closebutton, nil, "x"); place(ma_closebutton, 24, 22, "RIGHT", ma_topframe, "RIGHT", -8, 0) end
    if ma_languagebutton then chip(ma_languagebutton, nil, "Reload UI"); place(ma_languagebutton, 78, 22, "RIGHT", ma_closebutton or ma_topframe, "LEFT", -6, 0) end
    for i, n in ipairs({ "ma_toptext", "ma_top2text" }) do
        local fs = _G[n]
        if fs then
            quietHeader(fs)
            setFont(fs, F_SMALL()); textColor(fs, C.muted)
            local raw = rawget(fs, "lgmRaw") or fs:GetText()
            if type(raw) == "string" and raw:find("|c") then fs.lgmBusy = true; setText(fs, plain(raw)); fs.lgmBusy = nil end
            place(fs, nil, nil, "RIGHT", ma_languagebutton or ma_topframe, "LEFT", -12, i == 1 and 7 or -7)
            if fs.SetJustifyH then fs:SetJustifyH("RIGHT") end
        end
    end
    -- the tab strip: inside the window under the header, pills from the left
    place(ma_menubgframe, nil, G.TABS, "TOPLEFT", bg, "TOPLEFT", 0, -G.HEAD)
    ma_menubgframe:SetPoint("TOPRIGHT", bg, "TOPRIGHT", 0, -G.HEAD)
    local prev
    for _, g in ipairs(TABS) do
        local b = _G["ma_tabbutton_" .. g]
        if b then
            chip(b, "tab")
            local fs = b.GetFontString and b:GetFontString()
            local tw = fs and fs.GetStringWidth and tonumber((fs:GetStringWidth())) or 40
            local w = math.max(40, math.floor(tw + 22))
            if prev then place(b, w, 22, "LEFT", prev, "RIGHT", 4, 0)
            else place(b, w, 22, "LEFT", ma_menubgframe, "LEFT", 12, 1) end
            prev = b
        end
    end
    -- the content area
    place(ma_midframe, nil, G.MID, "TOPLEFT", bg, "TOPLEFT", 0, -(G.HEAD + G.TABS))
    ma_midframe:SetPoint("TOPRIGHT", bg, "TOPRIGHT", 0, -(G.HEAD + G.TABS))
    for _, t in ipairs({ G.footBand, G.footLine, G.tabLine, G.headBand }) do t:Show() end
end

-- ---------------------------------------------------------------- MiniGM
local MINI = { "MiniGMFrame", "MiniGMClassMenu", "MiniGMModifyMenu", "MiniGMAltMenu", "MiniGMTelePicker", "MiniGMSelfWarn" }
local function skinMiniTree(f, depth)
    depth = depth or 0
    if not f or depth > 6 or not f.GetChildren then return end
    for _, c in ipairs({ f:GetChildren() }) do
        if type(c) == "table" and not c.lgmOwn and c.GetObjectType then
            local kind = c:GetObjectType()
            if kind == "Button" and #buttonArt(c) > 0 and not (c.lgm and c.lgm.kind == "half") then chip(c)
            elseif kind == "EditBox" then field(c)
            elseif kind == "Slider" then slider(c)
            end
            skinMiniTree(c, depth + 1)
        end
    end
end
function G.SkinMini()
    if not G.on or not _G.MiniGMFrame then return false end
    if InCombatLockdown and InCombatLockdown() then G.miniLater = true; return false end
    for _, n in ipairs(MINI) do
        local f = _G[n]
        if f then
            panel(f, 0)
            if f.GetRegions then
                for _, r in ipairs({ f:GetRegions() }) do
                    local file = type(r) == "table" and not r.lgmOwn and r.GetTexture and r:GetObjectType() == "Texture" and r:GetTexture()
                    if type(file) == "string" and file:find("DialogBox%-Header") then setAlpha(r, 0) end
                end
            end
            skinMiniTree(f)
        end
    end
    return true
end

-- ---------------------------------------------------------------- on / off
function G.Present() return type(_G.AzerothAdmin) == "table" and _G.ma_bgframe ~= nil and _G.ma_midframe ~= nil end

function G.Apply()
    G.on = true
    local done = false
    for _, x in ipairs(ours) do if x.Show and not x.lgmHidden then x:Show() end end
    if G.Present() then
        if ma_displaylevelbutton and ma_displaylevelbutton.IsShown and ma_displaylevelbutton:IsShown() then G.group = "main" end
        skinWindow()
        layoutGMTab()
        skinTree(ma_bgframe)
        for _, n in ipairs({ "ma_popupframe", "ma_popup2frame", "ma_popmenubgframe" }) do
            local f = _G[n]
            if f then panel(f, 0) end
        end
        for _, n in ipairs({ "ma_popuptopframe", "ma_popupmidframe", "ma_popupbottomframe", "ma_popup2cframe" }) do clearFills(_G[n]) end
        for _, n in ipairs({ "ma_popupframe", "ma_popup2frame" }) do skinTree(_G[n]) end
        G.tab = G.group
        G.Size(G.group)
        G.PaintTabs(G.tab)
        done = true
    end
    if G.SkinMini() then done = true end
    return done
end

function G.Remove()
    G.on = false
    for i = #undo, 1, -1 do undo[i]() end
    undo = {}
    for _, x in ipairs(ours) do if x.Hide then x:Hide() end end
    if G.Present() then
        -- the header lines as AzerothAdmin last set them
        for _, n in ipairs({ "ma_toptext", "ma_top2text" }) do
            local fs = _G[n]
            if fs and rawget(fs, "lgmRaw") then fs.lgmBusy = true; fs:SetText(fs.lgmRaw); fs.lgmBusy = nil end
        end
    end
end

function G.Enabled()
    local db = WrathUnleashedUIDB
    return not (type(db) == "table" and db.gmSkin == false)
end
function G.SetEnabled(on)
    WrathUnleashedUIDB = WrathUnleashedUIDB or {}
    WrathUnleashedUIDB.gmSkin = on and true or false
    if on and not G.on then G.Apply() elseif not on and G.on then G.Remove() end
end

function G.Command(msg)
    local arg = tostring(msg or ""):lower():gsub("^%s*skin%s*", ""):gsub("%s+$", "")
    local on
    if arg == "off" or arg == "0" then on = false
    elseif arg == "on" or arg == "1" then on = true
    else on = not G.Enabled() end
    G.SetEnabled(on)
    say(on and "the Leyline look is on (/aa skin off for the original)." or "the original look is back (/aa skin on for the Leyline look).")
end

-- AzerothAdmin's own functions, wrapped once: the tab switch repaints the pills and sizes the window; the skin's
-- command rides on /aa ("/aa skin off"); frames made later (a reload of its frames) get the skin when shown.
function G.Hook()
    local AA = _G.AzerothAdmin
    if G.hooked or type(AA) ~= "table" then return end
    G.hooked = true
    if hooksecurefunc then
        if AA.ToggleTabButton then hooksecurefunc(AA, "ToggleTabButton", function(_, group) if G.on then G.PaintTabs(group) end end) end
        if AA.ToggleContentGroup then
            hooksecurefunc(AA, "ToggleContentGroup", function(_, group)
                G.group = group
                if G.on then G.Size(group); if group ~= "main" then skinTree(ma_midframe) end end
            end)
        end
    end
    local click = AA.OnClick
    if click then
        AA.OnClick = function(self, input, ...)
            if type(input) == "string" and input:lower():match("^%s*skin") then return G.Command(input) end
            return click(self, input, ...)
        end
    end
    local function watch()
        if G.watching or not _G.ma_bgframe or not ma_bgframe.HookScript then return end
        G.watching = true
        ma_bgframe:HookScript("OnShow", function() if G.Enabled() and G.ready then G.Apply() end end)
    end
    watch()
    -- frames made after us: watched from then on, skinned when first shown (not inside CreateFrames: AzerothAdmin
    -- sets its buttons' scripts right after it, which would drop our hover hooks)
    if hooksecurefunc and AA.CreateFrames then hooksecurefunc(AA, "CreateFrames", watch) end
end

SLASH_LEYLINEGMSKIN1 = "/gmskin"
SlashCmdList = SlashCmdList or {}
SlashCmdList.LEYLINEGMSKIN = function(msg) G.Command(msg) end

local ev = CreateFrame("Frame")
ev:RegisterEvent("ADDON_LOADED")
ev:RegisterEvent("PLAYER_LOGIN")
ev:RegisterEvent("PLAYER_REGEN_ENABLED")
ev:SetScript("OnEvent", function(_, event, name)
    if event == "ADDON_LOADED" then
        if name == "WrathUnleashedUI" or name == "AzerothAdmin" then G.Hook() end
    elseif event == "PLAYER_LOGIN" then
        G.Hook()
        G.ready = true
        if G.Enabled() then G.Apply() end
        if _G.MiniGMFrame and MiniGMFrame.HookScript then
            MiniGMFrame:HookScript("OnShow", function() if G.on then G.SkinMini() end end)
        end
    elseif event == "PLAYER_REGEN_ENABLED" and G.miniLater then
        G.miniLater = nil
        G.SkinMini()
    end
end)
