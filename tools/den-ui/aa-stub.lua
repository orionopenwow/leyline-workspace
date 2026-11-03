-- Stand-ins for AzerothAdmin's window (the GM panel, /aa), built on whatever fake 3.3.5a API is loaded
-- (tools/den-ui/fakeui.lua): every frame, button, text, field, slider and switch of its window under the same name,
-- parent, size and anchor as AzerothAdmin v24 makes them (tools/den-ui/azerothadmin-frames.lua, read from its
-- source by aa-frames.py), with its default grey look, so the Leyline skin (WrathUnleashedUI SkinGM.lua) can be
-- tested (stream/tests/test_wuui_gmskin.lua) and drawn (tools/den-ui/gm-scene.lua) without the addon, which is
-- third-party GPL code we don't copy. Also a small MiniGM stand-in. None of AzerothAdmin's code is here.
--
--   local A = dofile("tools/den-ui/aa-stub.lua")
--   A.build(rows)          the window (rows: dofile("tools/den-ui/azerothadmin-frames.lua")); AzerothAdmin's
--                          tab switch, /aa and CreateFrames as small stand-ins; A.clicks[name] counts OnClick runs,
--                          A.tips[name] the tooltip (OnEnter) runs
--   A.buildMini()          a MiniGM window: panel buttons, a close button, a field
--   A.snapshot(names)      points, size, alpha, text and font of the named parts (to compare after the skin is off)
local A = {}
local unpack = unpack or table.unpack

local GREY = { 0.62, 0.62, 0.64 }
A.CONTENT = { "main", "char", "npc", "go", "tele", "ticket", "misc", "server" }

-- the widget methods fakeui.lua leaves out, on one object
function A.extend(o)
    local function def(k, fn) if rawget(o, k) == nil then o[k] = fn end end
    def("GetNumPoints", function(self) return #(self.points or {}) end)
    def("GetPoint", function(self, i)
        local q = (self.points or {})[i or 1]
        if q then return q.p, q.rel or self.parent, q.rp, q.x, q.y end
    end)
    def("GetBackdrop", function(self) return self.backdrop end)
    def("GetBackdropColor", function(self) if self.bdcolor then return unpack(self.bdcolor) end end)
    def("GetBackdropBorderColor", function(self) if self.bdborder then return unpack(self.bdborder) end end)
    for _, k in ipairs({ "Normal", "Highlight", "Disabled" }) do
        def("Set" .. k .. "FontObject", function(self, f) self["font" .. k] = f end)
        def("Get" .. k .. "FontObject", function(self) return self["font" .. k] end)
    end
    for _, k in ipairs({ "Normal", "Pushed", "Highlight", "Checked", "DisabledChecked", "Disabled", "Thumb" }) do
        def("Get" .. k .. "Texture", function(self) return self["tex" .. k] end)
    end
    def("SetTextInsets", function(self, l, r, t, b) self.insets = { l, r, t, b } end)
    def("GetTextInsets", function(self) if self.insets then return unpack(self.insets) end end)
    def("SetMinMaxValues", function(self, lo, hi) self.lo, self.hi = lo, hi end)
    def("GetMinMaxValues", function(self) return self.lo or 0, self.hi or 1 end)
    return o
end

local function tex(parent, name, layer, file, c)
    local t = A.extend(parent:CreateTexture(name, layer))
    if file then t:SetTexture(file) elseif c then t:SetTexture(unpack(c)) end
    return t
end

-- a button as FrameLib makes it: a colour texture "<name>_texture", the text in the stock gold
local function button(r, parent)
    local b = A.extend(CreateFrame(r.type or "Button", r.name, parent, r.inherits))
    if r.w then b:SetWidth(r.w) end
    if r.h then b:SetHeight(r.h) end
    if r.type == "CheckButton" then
        b:SetWidth(32); b:SetHeight(32)
        b.texNormal = tex(b, nil, "BACKGROUND", "Interface\\Buttons\\UI-CheckBox-Up")
        b.texPushed = tex(b, nil, "BACKGROUND", "Interface\\Buttons\\UI-CheckBox-Down"); b.texPushed:SetAlpha(0)
        b.texHighlight = tex(b, nil, "HIGHLIGHT", "Interface\\Buttons\\UI-CheckBox-Highlight"); b.texHighlight:SetAlpha(0)
        b.texChecked = tex(b, nil, "ARTWORK", "Interface\\Buttons\\UI-CheckBox-Check"); b.texChecked:SetAlpha(0)
        local label = A.extend(b:CreateFontString(r.name .. "Text", "ARTWORK", "GameFontHighlight"))
        label:SetPoint("LEFT", b, "RIGHT", -2, 1)
        label:SetText(r.text or "")
        label:SetTextColor(1, 1, 1)
        b.SetChecked = function(self, c) self.checked = c and 1 or nil; self.texChecked:SetAlpha(c and 1 or 0) end
    else
        if r.tex then tex(b, r.tex, "BACKGROUND", nil, { 0.93, 0.93, 0.93, 1 }):SetAllPoints(b) end
        local fs = A.extend(b:CreateFontString(nil, "ARTWORK", "GameFontNormal"))
        fs:SetPoint("CENTER", b, "CENTER", 0, 0)
        fs:SetTextColor(1, 0.82, 0)
        b:SetFontString(fs)
        b.fontNormal, b.fontHighlight, b.fontDisabled = "GameFontNormal", "GameFontHighlight", "GameFontDisable"
        b.SetText = function(self, t) fs:SetText(t) end
        b.GetText = function(self) return fs:GetText() end
        if r.text then b:SetText(r.text) end
    end
    b:SetPoint(r.pos or "CENTER", (r.rel and _G[r.rel]) or parent, r.relPos or r.pos or "CENTER", r.x or 0, r.y or 0)
    return b
end

local function frame(r, parent)
    local f = A.extend(CreateFrame(r.type or "Frame", r.name, parent, r.inherits))
    if r.w then f:SetWidth(r.w) end
    if r.h then f:SetHeight(r.h) end
    if r.pos then f:SetPoint(r.pos, (r.rel and _G[r.rel]) or parent, r.relPos or r.pos, r.x or 0, r.y or 0) end
    local alpha = ({ ma_bgframe = 0.5, ma_popupframe = 0.5, ma_popmenubgframe = 0.5, ma_popup2frame = 0.5 })[r.name] or 0.7
    if r.type == "Slider" then
        f:SetHeight(17)
        f:SetBackdrop({ bgFile = "Interface\\Buttons\\UI-SliderBar-Background", edgeFile = "Interface\\Buttons\\UI-SliderBar-Border" })
        f:SetBackdropColor(1, 1, 1, 1); f:SetBackdropBorderColor(1, 1, 1, 1)
        local low = A.extend(f:CreateFontString(r.name .. "Low", "ARTWORK", "GameFontHighlightSmall"))
        low:SetPoint("TOPLEFT", f, "BOTTOMLEFT", 2, 3); low:SetText("Low"); low:SetTextColor(1, 1, 1)
        local high = A.extend(f:CreateFontString(r.name .. "High", "ARTWORK", "GameFontHighlightSmall"))
        high:SetPoint("TOPRIGHT", f, "BOTTOMRIGHT", -2, 3); high:SetText("High"); high:SetTextColor(1, 1, 1)
        local text = A.extend(f:CreateFontString(r.name .. "Text", "ARTWORK", "GameFontHighlight"))
        text:SetPoint("BOTTOM", f, "TOP", 0, 0); text:SetTextColor(1, 1, 1)
        f.lo, f.hi, f.value = 0.1, 10, 1
        local label = r.name:match("speed") and "Speed" or (r.name:match("scale") and "Scale" or "Value")
        text:SetText(label .. ": 1.0")
        f.texThumb = tex(f, nil, "OVERLAY", "Interface\\Buttons\\UI-SliderBar-Button-Horizontal")
        f.texThumb:SetWidth(32); f.texThumb:SetHeight(32)
        local function thumb()
            local k = ((f.value or 0) - f.lo) / (f.hi - f.lo)
            f.texThumb:ClearAllPoints()
            f.texThumb:SetPoint("CENTER", f, "LEFT", (f.w or 100) * k, 0)
        end
        local setW, setV = f.SetWidth, f.SetValue
        f.SetWidth = function(self, w) setW(self, w); thumb() end
        f.SetValue = function(self, v) setV(self, v); thumb() end
        thumb()
    elseif r.type == "EditBox" then
        f:SetFontObject("ChatFontNormal")
        f:SetText("")
        if r.inherits == "InputBoxTemplate" then
            for _, k in ipairs({ "Left", "Middle", "Right" }) do
                local t = tex(f, r.name .. k, "BACKGROUND", "Interface\\Common\\Common-Input-Border")
                if k == "Left" then t:SetPoint("LEFT", f, "LEFT", -5, 0); t:SetWidth(8); t:SetHeight(20)
                elseif k == "Right" then t:SetPoint("RIGHT", f, "RIGHT", 0, 0); t:SetWidth(8); t:SetHeight(20)
                else t:SetPoint("LEFT", f, "LEFT", 3, 0); t:SetPoint("RIGHT", f, "RIGHT", -8, 0); t:SetHeight(20) end
            end
        elseif r.tex then
            tex(f, r.tex, "BACKGROUND", nil, { 0.93, 0.93, 0.93, 1 }):SetAllPoints(f)
        end
    elseif r.inherits == "UIDropDownMenuTemplate" then
        for _, k in ipairs({ "Left", "Middle", "Right" }) do tex(f, r.name .. k, "ARTWORK", "Interface\\Glues\\CharacterCreate\\CharacterCreate-LabelFrame") end
        A.extend(CreateFrame("Button", r.name .. "Button", f))
    elseif r.name == "ma_logoframe" then
        tex(f, nil, "BACKGROUND", "Interface\\AddOns\\AzerothAdmin\\Textures\\logo.tga"):SetAllPoints(f)
    elseif r.type == nil or r.type == "Frame" or r.type == "ScrollFrame" then
        local c = r.type == "ScrollFrame" and { 0, 0, 0, 0.7 } or { GREY[1], GREY[2], GREY[3], alpha }
        tex(f, nil, "BACKGROUND", nil, c):SetAllPoints(f)
    end
    return f
end

-- the hand-made EditBoxes (the GM tab's parameter field, the grid step): AzerothAdmin's backdrop look
local function editbox(r, parent)
    local e = A.extend(CreateFrame("EditBox", r.name, parent))
    e:SetWidth(r.w or 80); e:SetHeight(r.h or 20)
    e:SetPoint(r.pos or "TOPLEFT", (r.rel and _G[r.rel]) or parent, r.relPos or r.pos or "TOPLEFT", r.x or 0, r.y or 0)
    e:SetFontObject("ChatFontNormal")
    e:SetText("")
    e:SetTextInsets(4, 4, 2, 2)
    e:SetBackdrop({ bgFile = "Interface\\ChatFrame\\ChatFrameBackground", edgeFile = "Interface\\Buttons\\WHITE8X8", edgeSize = 2 })
    e:SetBackdropColor(0, 0, 0, 0.6); e:SetBackdropBorderColor(0.35, 0.35, 0.35, 1)
    -- the fake draws no backdrop: a texture for the "before" picture only
    tex(e, nil, "BACKGROUND", nil, { 0, 0, 0, 0.6 }):SetAllPoints(e)
    return e
end

local function fontstring(r, parent)
    local fs = A.extend(parent:CreateFontString(r.name, "ARTWORK", "GameFontNormal"))
    fs:SetPoint(r.pos or "CENTER", (r.rel and _G[r.rel]) or parent, r.relPos or r.pos or "CENTER", r.x or 0, r.y or 0)
    fs:SetTextColor(1, 0.82, 0)
    if r.text then fs:SetText(r.text) end
    return fs
end

function A.build(list)
    A.clicks, A.tips, A.groups, A.rows, A.all = {}, {}, {}, {}, {}
    local KEYS = { "kind", "name", "group", "parent", "type", "inherits", "w", "h", "pos", "rel", "relPos", "x", "y", "text", "tex" }
    MAJOR_VERSION = "|cFF00FF00AzerothAdmin-3.3.5|r"
    MINOR_VERSION = 24
    local AA = { db = { profile = { style = { framestrata = "MEDIUM", showtooltips = true } }, char = {} } }
    AzerothAdmin = AA
    function AA:CreateFrames()
        for _, v in ipairs(list) do
            local r = {}
            for i, k in ipairs(KEYS) do r[k] = v[i] end
            if r.name then
                local parent = (r.parent and _G[r.parent]) or UIParent
                local o
                if r.kind == "button" then o = button(r, parent)
                elseif r.kind == "frame" then o = frame(r, parent)
                elseif r.kind == "editbox" then o = editbox(r, parent)
                elseif r.kind == "fontstring" then o = fontstring(r, parent)
                end
                if o then
                    A.groups[r.group or "?"] = A.groups[r.group or "?"] or {}
                    table.insert(A.groups[r.group or "?"], o)
                    A.rows[#A.rows + 1] = r
                    A.all[r.name] = o
                end
            end
        end
        ma_toptext:SetText("|cFF00FF00Char:|r Wren |cFF00FF00Guid:|r 1")
        ma_top2text:SetText("|cFF00FF00Realm:|r Your Den")
        ma_revtext:SetText(MAJOR_VERSION .. " " .. MINOR_VERSION)
        ma_infoonlinetext:SetText("Players Online: 3")
        ma_infomaxonlinetext:SetText("Maximum Online: 5")
        ma_infouptimetext:SetText("Server Uptime: 2 hours 14 minutes")
        ma_inforevisiontext:SetText("AzerothCore Revision: rev. 5f8e1c2 2026-10-30")
        if ma_bgframe.SetFrameStrata then ma_bgframe:SetFrameStrata("MEDIUM") end
        for _, g in ipairs({ "char", "npc", "go", "tele", "ticket", "misc", "server", "popup", "popup2", "minimenu", "hidden", "server_special", "noneyet" }) do
            for _, o in ipairs(A.groups[g] or {}) do o:Hide() end
        end
    end
    -- PrepareScript: every button's command (counted here) and tooltip
    function AA:InitButtons()
        for _, r in ipairs(A.rows) do
            if r.kind == "button" then
                local b = _G[r.name]
                local n = r.name
                b:SetScript("OnClick", function(self)
                    if self.SetChecked then self:SetChecked(not self:GetChecked()) end
                    A.clicks[n] = (A.clicks[n] or 0) + 1
                end)
                b:SetScript("OnEnter", function() A.tips[n] = (A.tips[n] or 0) + 1 end)
                b:SetScript("OnLeave", function() end)
            end
        end
    end
    function AA:HideAllGroups()
        for _, g in ipairs(A.CONTENT) do for _, o in ipairs(A.groups[g] or {}) do o:Hide() end end
    end
    function AA:ToggleTabButton(group)
        for _, g in ipairs(A.CONTENT) do
            local t = _G["ma_tabbutton_" .. g .. "_texture"]
            if t then t:SetTexture(0.93, 0.93, 0.93, g == group and 1 or 0.7) end
        end
    end
    function AA:ToggleContentGroup(group)
        self:HideAllGroups()
        for _, o in ipairs(A.groups[group] or {}) do o:Show() end
    end
    function AA:InstantGroupToggle(group)
        for _, o in ipairs(A.groups.bg or {}) do o:Show() end
        self:ToggleTabButton(group)
        self:ToggleContentGroup(group)
    end
    function AA:OnClick()
        if ma_bgframe:IsShown() then for _, o in ipairs(A.groups.bg) do o:Hide() end
        else for _, o in ipairs(A.groups.bg) do o:Show() end end
    end
    -- AzerothAdmin's OnInitialize: the frames, then the buttons' scripts; /aa
    AA:CreateFrames()
    AA:InitButtons()
    AA:ToggleTabButton("main")
    SlashCmdList = SlashCmdList or {}
    SlashCmdList.ACECONSOLE_AA = function(input) AzerothAdmin.OnClick(AzerothAdmin, input) end
    return AA
end

-- a MiniGM window as MiniGM.lua makes it (the dialog box backdrop and header, panel buttons, a close button, a field)
function A.buildMini()
    A.miniClicks = {}
    local f = A.extend(CreateFrame("Frame", "MiniGMFrame", UIParent))
    f:SetWidth(300); f:SetHeight(220); f:SetPoint("CENTER", UIParent, "CENTER", 520, 0)
    f:SetBackdrop({ bgFile = "Interface\\DialogFrame\\UI-DialogBox-Background", edgeFile = "Interface\\DialogFrame\\UI-DialogBox-Border" })
    f:SetBackdropColor(1, 1, 1, 1); f:SetBackdropBorderColor(1, 1, 1, 1)
    local header = tex(f, nil, "ARTWORK", "Interface\\DialogFrame\\UI-DialogBox-Header")
    header:SetWidth(200); header:SetHeight(64); header:SetPoint("TOP", f, "TOP", 0, 12)
    A.miniHeader = header
    local title = A.extend(f:CreateFontString(nil, "OVERLAY", "GameFontNormal"))
    title:SetPoint("TOP", f, "TOP", 0, -2); title:SetText("MiniGM")
    local close = A.extend(CreateFrame("Button", "MiniGMStubClose", f, "UIPanelCloseButton"))
    close:SetWidth(32); close:SetHeight(32); close:SetPoint("TOPRIGHT", f, "TOPRIGHT", -8, -8)
    close.texNormal = tex(close, nil, "ARTWORK", "Interface\\Buttons\\UI-Panel-MinimizeButton-Up")
    close:SetScript("OnClick", function() A.miniClicks.close = (A.miniClicks.close or 0) + 1 end)
    A.miniClose = close
    A.miniButtons = {}
    for i, label in ipairs({ "Fly on", "Fly off", "Speed 1.5x", "God on", "Revive", "Add bot" }) do
        local b = A.extend(CreateFrame("Button", "MiniGMStubButton" .. i, f, "UIPanelButtonTemplate"))
        b:SetWidth(130); b:SetHeight(22)
        b:SetPoint("TOPLEFT", f, "TOPLEFT", 14 + ((i - 1) % 2) * 138, -34 - math.floor((i - 1) / 2) * 25)
        b.texNormal = tex(b, nil, "BACKGROUND", "Interface\\Buttons\\UI-Panel-Button-Up"); b.texNormal:SetAllPoints(b)
        b.texHighlight = tex(b, nil, "HIGHLIGHT", "Interface\\Buttons\\UI-Panel-Button-Highlight"); b.texHighlight:SetAllPoints(b)
        local fs = A.extend(b:CreateFontString(nil, "ARTWORK", "GameFontNormal"))
        fs:SetPoint("CENTER", b, "CENTER", 0, 0); fs:SetText(label)
        b:SetFontString(fs)
        b.SetText = function(self, t) fs:SetText(t) end
        b.GetText = function(self) return fs:GetText() end
        b:SetScript("OnClick", function() A.miniClicks[label] = (A.miniClicks[label] or 0) + 1 end)
        A.miniButtons[i] = b
    end
    local e = A.extend(CreateFrame("EditBox", "MiniGMStubBox", f, "InputBoxTemplate"))
    e:SetWidth(120); e:SetHeight(18); e:SetPoint("BOTTOMLEFT", f, "BOTTOMLEFT", 20, 16)
    e:SetFontObject("ChatFontNormal")
    for _, k in ipairs({ "Left", "Middle", "Right" }) do tex(e, "MiniGMStubBox" .. k, "BACKGROUND", "Interface\\Common\\Common-Input-Border") end
    A.miniBox = e
    return f
end

-- what the skin may change on a part, to compare once it is off
function A.snapshot(names)
    local out = {}
    for _, n in ipairs(names) do
        local o = _G[n]
        if o then
            local s = { alpha = o.GetAlpha and o:GetAlpha(), text = o.GetText and o:GetText(), w = o.w, h = o.h,
                font = o.GetFontObject and o:GetFontObject(), tcolor = o.tcolor and table.concat(o.tcolor, ","), points = {} }
            for i = 1, o:GetNumPoints() do
                local p, rel, rp, x, y = o:GetPoint(i)
                s.points[i] = table.concat({ p, tostring(rel and (rel.name or rel.n)), rp, string.format("%.1f", x or 0), string.format("%.1f", y or 0) }, " ")
            end
            s.points = table.concat(s.points, " | ")
            out[n] = s
        end
    end
    return out
end

return A
