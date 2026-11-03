-- A fake 3.3.5a widget API that keeps every widget's geometry, so the Den editor (WrathUnleashedUI DenBar.lua,
-- Builder.lua, ...) can be laid out and drawn without the game: tools/den-ui/render.cjs turns a dump into a picture,
-- and stream/tests/test_wuui_denlayout.lua checks it (sizes, overlaps with the game's own minimap, chat and bars).
-- Lua 5.1 (and 5.4 with unpack = table.unpack). Nothing here is game data: the stock frames are rectangles at the
-- stock positions (UI units), the icons are names only.
--
--   local F = dofile("tools/den-ui/fakeui.lua")
--   F.setup(w, h)          UIParent w x h UI units (1920x1080 at the addon's own fit scale 0.71: 1352 x 1081)
--   F.load(addonDir)       every file of the .toc, in order
--   F.fire(event, ...)     an event to every frame that registered it
--   F.tick(secs)           OnUpdate on every visible frame
--   F.layout()             absolute rects of every visible region: { {n, kind, name, x, y, w, h, ...}, ... }
--   F.dump(path, title)    the layout as JSON (render.cjs draws it)
local F = {}
F.created = {}
F.frames = {}
F.sent = {}
F.chat = {}
F.cvars = {}
F.overrides = {}
F.clock = 100

-- the stock fonts' sizes (FRIZQT 10 / 12 / 16), and the game's own font files by path
local FONTS = {
    GameFontHighlightSmall = 10, GameFontNormalSmall = 10, GameFontDisableSmall = 10, GameFontHighlightSmallLeft = 10,
    GameFontNormal = 12, GameFontHighlight = 12, GameFontDisable = 12, GameFontNormalLarge = 16, GameFontHighlightLarge = 16,
    ChatFontNormal = 14, NumberFontNormal = 12, GameFontNormalHuge = 20, SystemFont_Small = 10,
}

local Frame = {}
Frame.__index = function(self, k)
    local v = rawget(Frame, k)
    if v ~= nil then return v end
    if type(k) == "string" and k:match("^%u") then return function() end end -- other widget methods: no-ops
    return nil
end
local function new(kind, name, parent, extra)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, events = {}, shown = true,
        n = #F.created + 1, points = {}, alpha = 1, scale = 1 }, Frame)
    for k, v in pairs(extra or {}) do f[k] = v end
    if name and name ~= "" then _G[name] = f end
    F.created[#F.created + 1] = f
    return f
end
F.new = new

function Frame:SetScript(n, fn) self.scripts[n] = fn end
function Frame:GetScript(n) return self.scripts[n] end
function Frame:HookScript(n, fn)
    local old = self.scripts[n]
    self.scripts[n] = function(...) if old then old(...) end; return fn(...) end
end
function Frame:Fire(n, ...) local fn = self.scripts[n]; if fn then return fn(self, ...) end end
function Frame:Click(button) return self:Fire("OnClick", button or "LeftButton", true) end
function Frame:Show()
    local was = self:IsVisible(); self.shown = true
    if not was and self:IsVisible() then self:Fire("OnShow") end
end
function Frame:Hide()
    local was = self:IsVisible(); self.shown = false
    if was then self:Fire("OnHide") end
end
function Frame:SetShown(on) if on then self:Show() else self:Hide() end end
function Frame:IsShown() return self.shown end
function Frame:IsVisible()
    if not self.shown then return false end
    return self.parent == nil or self.parent._root or self.parent:IsVisible()
end
function Frame:SetText(t)
    self.text = t
    if self.kind == "EditBox" and self.scripts.OnTextChanged then self:Fire("OnTextChanged", true) end
end
function Frame:GetText() return self.text end
function Frame:Enable() self.enabled = true end
function Frame:Disable() self.enabled = false end
function Frame:IsEnabled() return self.enabled ~= false end
function Frame:SetFocus() self.focus = true end
function Frame:ClearFocus() self.focus = false end
function Frame:HasFocus() return self.focus == true end
function Frame:RegisterEvent(e) self.events[e] = true end
function Frame:UnregisterEvent(e) self.events[e] = nil end
function Frame:GetName() return self.name end
function Frame:GetParent() return self.parent end
function Frame:SetParent(p) self.parent = p end
function Frame:GetObjectType() return self.kind end
function Frame:SetWidth(w) self.w = w end
function Frame:SetHeight(h) self.h = h end
function Frame:SetSize(w, h) self.w, self.h = w, h end
local function now(self) return F.anyRect(self) end
function Frame:GetWidth() local r = now(self); return r and r.w or (self.w or 0) end
function Frame:GetHeight() local r = now(self); return r and r.h or (self.h or 0) end
function Frame:GetLeft() local r = now(self); return r and r.x end
function Frame:GetRight() local r = now(self); return r and (r.x + r.w) end
function Frame:GetTop() local r = now(self); return r and (r.y + r.h) end
function Frame:GetBottom() local r = now(self); return r and r.y end
function Frame:GetCenter() local r = now(self); if r then return r.x + r.w / 2, r.y + r.h / 2 end end
function Frame:SetScale(s) self.scale = tonumber(s) or 1 end
function Frame:GetScale() return self.scale or 1 end
function Frame:GetEffectiveScale() return 1 end
function Frame:SetAlpha(a) self.alpha = a end
function Frame:GetAlpha() return self.alpha end
function Frame:SetFrameStrata(s) self.strata = s end
function Frame:GetFrameStrata() return self.strata or (self.parent and self.parent.GetFrameStrata and self.parent:GetFrameStrata()) or "MEDIUM" end
function Frame:SetFrameLevel(l) self.level = l end
function Frame:GetFrameLevel() return self.level or ((self.parent and self.parent.GetFrameLevel and self.parent:GetFrameLevel() or 0) + 1) end
function Frame:SetPoint(p, a, b, c, d)
    local rel, rp, x, y
    if type(a) == "table" then rel, rp, x, y = a, b, c, d
    elseif type(a) == "string" and type(b) == "string" then rel, rp, x, y = _G[a], b, c, d
    elseif type(a) == "string" then rel, rp, x, y = nil, a, b, c
    else rel, rp, x, y = nil, p, a, b end
    for i, q in ipairs(self.points) do if q.p == p then table.remove(self.points, i) break end end
    self.points[#self.points + 1] = { p = p, rel = rel, rp = rp or p, x = tonumber(x) or 0, y = tonumber(y) or 0 }
end
function Frame:ClearAllPoints() self.points = {} end
function Frame:SetAllPoints(rel)
    rel = rel or self.parent
    self.points = { { p = "TOPLEFT", rel = rel, rp = "TOPLEFT", x = 0, y = 0 }, { p = "BOTTOMRIGHT", rel = rel, rp = "BOTTOMRIGHT", x = 0, y = 0 } }
end
function Frame:SetTexture(r, g, b, a)
    if type(r) == "number" then self.color = { r, g, b, a or 1 }; self.file = nil else self.file = r; self.color = nil end
end
function Frame:GetTexture() return self.file end
function Frame:SetTexCoord(...) self.texcoord = { ... } end
function Frame:SetVertexColor(r, g, b, a) self.vcolor = { r, g, b, a or 1 } end
function Frame:SetDesaturated(d) self.desat = d and true or nil end
function Frame:SetTextColor(r, g, b, a) self.tcolor = { r, g, b, a or 1 } end
function Frame:GetTextColor() local c = self.tcolor or { 1, 1, 1, 1 } return c[1], c[2], c[3], c[4] end
function Frame:SetBackdrop(b) self.backdrop = b end
function Frame:SetBackdropColor(r, g, b, a) self.bdcolor = { r, g, b, a or 1 } end
function Frame:SetBackdropBorderColor(r, g, b, a) self.bdborder = { r, g, b, a or 1 } end
function Frame:SetJustifyH(j) self.justify = j end
function Frame:SetJustifyV(j) self.justifyV = j end
function Frame:SetFontString(fs) self.fontString = fs end
function Frame:GetFontString() return self.fontString end
function Frame:SetFontObject(o) self.font = o end
function Frame:GetFontObject() return self.font end
function Frame:SetFont(path, size, flags) self.fontFile, self.fontSize, self.fontFlags = path, size, flags; return true end
function Frame:GetFont()
    local s = F.fontSize(self)
    return self.fontFile or "Fonts\\FRIZQT__.TTF", s, self.fontFlags
end
function Frame:SetMaxLetters(n) self.maxLetters = n end
function Frame:SetMultiLine(m) self.multi = m end
function Frame:SetWordWrap(m) self.wrap = m end
function Frame:GetStringWidth() return F.textWidth(self) end
function Frame:GetStringHeight() return F.fontSize(self) + 2 end
function Frame:GetRegions()
    local out = {}
    for _, f in ipairs(F.created) do if f.parent == self and (f.kind == "FontString" or f.kind == "Texture") then out[#out + 1] = f end end
    return unpack(out)
end
function Frame:GetChildren()
    local out = {}
    for _, f in ipairs(F.created) do if f.parent == self and f.kind ~= "FontString" and f.kind ~= "Texture" then out[#out + 1] = f end end
    return unpack(out)
end
function Frame:IsMouseOver() return false end
function Frame:SetModel(p) self.model = p end
function Frame:GetModel() return self.model end
function Frame:ClearModel() self.model = nil end
function Frame:SetFacing(f) self.facing = f end
function Frame:GetFacing() return self.facing or 0 end
function Frame:SetPosition(x, y, z) self.mpos = { x, y, z } end
function Frame:GetPosition() local p = self.mpos or { 0, 0, 0 } return p[1], p[2], p[3] end
function Frame:SetModelScale(s) self.mscale = s end
function Frame:GetModelScale() return self.mscale or 1 end
function Frame:SetCamera(c) self.camera = c end
function Frame:SetLight(...) self.light = { ... } end
function Frame:SetFogColor(...) self.fog = { ... } end
function Frame:SetCamDistanceScale(s) self.camDist = s end
function Frame:SetValue(v) self.value = v; self:Fire("OnValueChanged", v) end
function Frame:GetValue() return self.value or 0 end
function Frame:SetChecked(c) self.checked = c end
function Frame:GetChecked() return self.checked end
function Frame:CreateFontString(name, layer, font) return new("FontString", name, self, { layer = layer or "ARTWORK", font = font }) end
function Frame:CreateTexture(name, layer) return new("Texture", name, self, { layer = layer or "ARTWORK" }) end

function CreateFrame(kind, name, parent, template)
    local f = new(kind, name, parent or UIParent, { template = template })
    F.frames[#F.frames + 1] = f
    if template == "OptionsSliderTemplate" and name then
        new("FontString", name .. "Low", f); new("FontString", name .. "High", f); new("FontString", name .. "Text", f)
    end
    if template == "GameMenuButtonTemplate" then f.w, f.h = 144, 21 end
    return f
end

-- font objects: the stock ones, and CreateFont's
local FontObj = {}
FontObj.__index = FontObj
function FontObj:SetFont(path, size, flags) self.file, self.size, self.flags = path, size, flags end
function FontObj:GetFont() return self.file, self.size, self.flags end
function FontObj:SetTextColor(r, g, b, a) self.color = { r, g, b, a } end
function FontObj:SetShadowOffset() end
function FontObj:SetShadowColor() end
function FontObj:SetFontObject(o) if type(o) == "table" then self.size = o.size; self.file = o.file end end
function FontObj:SetJustifyH() end
local function fontObj(name, size) local o = setmetatable({ fontName = name, size = size, file = "Fonts\\FRIZQT__.TTF" }, FontObj); _G[name] = o; return o end
function CreateFont(name) return fontObj(name, 12) end

function F.fontSize(fs)
    if fs.fontSize then return fs.fontSize end
    local o = fs.font
    if type(o) == "string" then o = _G[o] end
    if type(o) == "table" and o.size then return o.size end
    if type(fs.font) == "string" and FONTS[fs.font] then return FONTS[fs.font] end
    return 12
end
local function plainText(t) return (tostring(t or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("|T.-|t", "  ")) end
function F.textWidth(fs)
    local s = plainText(fs.text)
    local n = 0
    for _ in s:gmatch("[%z\1-\127\194-\244][\128-\191]*") do n = n + 1 end
    return n * F.fontSize(fs) * 0.56
end

-- ---------------------------------------------------------------- the screen and the stock frames
function F.setup(w, h)
    for k in pairs(F.overrides) do F.overrides[k] = nil end
    UIParent = new("Frame", "UIParent", nil, { _root = true, w = w, h = h, level = 0 })
    WorldFrame = new("Frame", "WorldFrame", nil, { _root = true, w = w, h = h, level = 0 })
    F.screen = { w = w, h = h }
    -- the stock frames (positions of the 3.3.5a defaults, UI units), shown so overlaps can be seen and checked
    local function stock(name, w2, h2, p, x, y, label)
        local f = new("Frame", name, UIParent, { w = w2, h = h2, stock = label or name })
        f:SetPoint(p, UIParent, p, x, y)
        return f
    end
    stock("MinimapCluster", 192, 192, "TOPRIGHT", 0, 0, "minimap")
    stock("BuffFrame", 300, 40, "TOPRIGHT", -205, -13, "buffs")
    stock("PlayerFrame", 232, 100, "TOPLEFT", -19, -4, "player frame")
    stock("TargetFrame", 232, 100, "TOPLEFT", 250, -4, "target frame")
    TargetFrame.shown = false
    stock("ChatFrame1", 430, 120, "BOTTOMLEFT", 32, 95, "chat")
    stock("MainMenuBar", 1024, 53, "BOTTOM", 0, 0, "action bar")
    stock("MultiBarBottomLeft", 500, 42, "BOTTOM", -258, 53, "bar")
    stock("MultiBarBottomRight", 500, 42, "BOTTOM", 258, 53, "bar")
    stock("MultiBarRight", 42, 500, "BOTTOMRIGHT", -2, 98, "side bar")
    stock("MultiBarLeft", 42, 500, "BOTTOMRIGHT", -45, 98, "side bar")
    GameMenuFrame = new("Frame", "GameMenuFrame", UIParent, { w = 195, h = 300 }); GameMenuFrame.shown = false
    GameTooltip = new("GameTooltip", "GameTooltip", UIParent); GameTooltip.shown = false
    DEFAULT_CHAT_FRAME = ChatFrame1
    ChatFrame1.AddMessage = function(_, m) F.chat[#F.chat + 1] = m end
    for name, size in pairs(FONTS) do fontObj(name, size) end
end
F.STOCK = { "MinimapCluster", "BuffFrame", "PlayerFrame", "TargetFrame", "ChatFrame1", "MainMenuBar", "MultiBarBottomLeft",
    "MultiBarBottomRight", "MultiBarRight", "MultiBarLeft" }

UISpecialFrames = {}
SlashCmdList = {}
function tinsert(t, v, w) if w ~= nil then table.insert(t, v, w) else table.insert(t, v) end end
function tremove(t, i) return table.remove(t, i) end
function wipe(t) for k in pairs(t) do t[k] = nil end return t end
strtrim = function(s) return (tostring(s or ""):gsub("^%s+", ""):gsub("%s+$", "")) end
function hooksecurefunc(a, b, c)
    if type(a) == "table" then local old = a[b]; a[b] = function(...) local r = { old(...) }; c(...); return unpack(r) end
    else local old = _G[a]; _G[a] = function(...) local r = old and { old(...) } or {}; b(...); return unpack(r) end end
end
function GetScreenWidth() return F.screen.w end
function GetScreenHeight() return F.screen.h end
function InCombatLockdown() return false end
function SetBinding() return 1 end
function SetBindingClick() return 1 end
function SaveBindings() end
function GetCurrentBindingSet() return 1 end
function GetCVar(k) return F.cvars[k] end
function SetCVar(k, v) F.cvars[k] = tostring(v) end
function SetOverrideBindingClick(_, _, key, button) F.overrides[key] = button end
function ClearOverrideBindings() for k in pairs(F.overrides) do F.overrides[k] = nil end end
function GetBindingAction() return "" end
function GetBindingKey() return nil end
function GetUnitSpeed() return 0 end
function IsFalling() return false end
function HasAction() return false end
function UnitLevel() return 1 end
function UnitClass() return "Warrior", "WARRIOR" end
function UnitName() return "Wren" end
function UnitGUID() return "0x0000000000000001" end
function UnitExists() return false end
function PlaySound() end
function HideUIPanel(f) if f then f:Hide() end end
function ShowUIPanel(f) if f then f:Show() end end
F.realm = "Your Den"
function GetRealmName() return F.realm end
function GetTime() return F.clock end
function time() return 1762000000 + math.floor(F.clock) end
function date() return "12:00" end
function GetLocale() return "enUS" end
function GetBuildInfo() return "3.3.5", "12340", "Jun 24 2010", 30300 end
function IsShiftKeyDown() return false end
function IsAltKeyDown() return false end
function IsControlKeyDown() return false end
function GetCursorPosition() return 0, 0 end
function GetCurrentKeyBoardFocus() return nil end
function GetNumAddOns() return 0 end
function GetAddOnInfo() return nil end
function IsMacClient() return false end
function GetNumRaidMembers() return 0 end
function GetNumPartyMembers() return 0 end
function GetActionBarPage() return 1 end
function GetActionInfo() return nil end
function GetNumSpellTabs() return 0 end
function ChatFrame_AddMessageEventFilter(ev, fn) F.filters = F.filters or {}; F.filters[ev] = F.filters[ev] or {}; table.insert(F.filters[ev], fn) end
function ChatFrame_RemoveMessageEventFilter() end
function RegisterAddonMessagePrefix() return true end
function SendChatMessage(m) F.chat[#F.chat + 1] = "say: " .. tostring(m) end
function SendAddonMessage(prefix, msg, channel, target)
    F.sent[#F.sent + 1] = msg
    if F.server and prefix == "OVMOD" and target == UnitName("player") then F.server(msg) end
end

-- what the server says to the addon: an addon whisper from yourself
function F.toClient(msg)
    for _, f in ipairs(F.frames) do
        if f.events.CHAT_MSG_ADDON then f:Fire("OnEvent", "CHAT_MSG_ADDON", "OVMOD", msg, "WHISPER", UnitName("player")) end
    end
end
function F.fire(event, ...)
    for _, f in ipairs(F.frames) do if f.events[event] then f:Fire("OnEvent", event, ...) end end
end
-- a system line in the chat, through the chat filters (as ChatFrame_MessageEventHandler does)
function F.systemLine(text)
    for _, fn in ipairs((F.filters or {}).CHAT_MSG_SYSTEM or {}) do
        if fn(ChatFrame1, "CHAT_MSG_SYSTEM", text) then return false end
    end
    F.chat[#F.chat + 1] = text
    return true
end
function F.tick(secs)
    F.clock = F.clock + secs
    for _, f in ipairs(F.frames) do
        local fn = f.scripts.OnUpdate
        if fn and (f.parent == WorldFrame or f:IsVisible() or f.parent == UIParent) then fn(f, secs) end
    end
end

function F.load(dir)
    local files = {}
    for line in io.lines(dir .. "/WrathUnleashedUI.toc") do
        if line:match("%.lua%s*$") then files[#files + 1] = line:gsub("%s+$", "") end
    end
    for _, f in ipairs(files) do dofile(dir .. "/" .. f) end
    return files
end

-- ---------------------------------------------------------------- layout: every region's absolute rect
-- UI units, origin at the screen's bottom left (as the game: y up). Two points on opposite sides set a size; else
-- the frame's own width / height; a FontString without a width is as wide as its text.
local function anchorXY(r, p)
    local x = (p:find("LEFT") and r.x) or (p:find("RIGHT") and r.x + r.w) or (r.x + r.w / 2)
    local y = (p:find("TOP") and r.y + r.h) or (p:find("BOTTOM") and r.y) or (r.y + r.h / 2)
    return x, y
end
local cache
function F.rect(f, depth)
    if not cache then return nil end
    if cache[f] ~= nil then return cache[f] or nil end
    depth = depth or 0
    if depth > 60 then return nil end
    if f._root then cache[f] = { x = 0, y = 0, w = f.w or F.screen.w, h = f.h or F.screen.h }; return cache[f] end
    cache[f] = false
    local w, h = f.w, f.h
    if f.kind == "FontString" then
        f.autoW = nil
        if not w or w == 0 then w = F.textWidth(f); f.autoW = true end
        if not h or h == 0 then
            -- a FontString with a width and no height wraps: as many lines as its text needs
            local lines = (f.w and f.w > 0) and math.max(1, math.ceil(F.textWidth(f) / f.w)) or 1
            h = lines * (F.fontSize(f) + 3) - 1
        end
    end
    local left, right, top, bottom, cx, cy
    local pts = f.points
    if #pts == 0 then
        if f.parent and f.kind ~= "FontString" and f.kind ~= "Texture" then return nil end
        pts = { { p = "TOPLEFT", rel = f.parent, rp = "TOPLEFT", x = 0, y = 0 } }
        if f.kind == "Texture" and not f.w then pts[2] = { p = "BOTTOMRIGHT", rel = f.parent, rp = "BOTTOMRIGHT", x = 0, y = 0 } end
    end
    -- a scaled frame's offsets are in its own units: the parent's rect / scale
    for _, q in ipairs(pts) do
        local rel = q.rel or f.parent or UIParent
        local rr = F.rect(rel, depth + 1)
        if not rr then cache[f] = false; return nil end
        local ax, ay = anchorXY(rr, q.rp)
        ax, ay = ax + q.x, ay + q.y
        local p = q.p
        if p:find("LEFT") then left = ax elseif p:find("RIGHT") then right = ax else cx = ax end
        if p:find("TOP") then top = ay elseif p:find("BOTTOM") then bottom = ay else cy = ay end
    end
    w, h = w or 0, h or 0
    local x, y
    if left and right then f.autoW = nil end
    if left and right then x, w = left, right - left elseif left then x = left elseif right then x = right - w else x = (cx or 0) - w / 2 end
    if top and bottom then y, h = bottom, top - bottom elseif bottom then y = bottom elseif top then y = top - h else y = (cy or 0) - h / 2 end
    cache[f] = { x = x, y = y, w = w, h = h }
    return cache[f]
end

local STRATA = { BACKGROUND = 1, LOW = 2, MEDIUM = 3, HIGH = 4, DIALOG = 5, FULLSCREEN = 6, FULLSCREEN_DIALOG = 7, TOOLTIP = 8 }
local LAYER = { BACKGROUND = 1, BORDER = 2, ARTWORK = 3, OVERLAY = 4, HIGHLIGHT = 5 }
local function owner(f) if f.kind == "FontString" or f.kind == "Texture" then return f.parent end return f end

-- every visible region with its rect, in drawing order
function F.layout()
    cache = {}
    local out = {}
    for _, f in ipairs(F.created) do
        if f:IsVisible() and not f._root then
            local r = F.rect(f)
            if r and r.w > 0 and r.h > 0 then
                local o = owner(f)
                local alpha = 1
                local p = f
                while p and not p._root do alpha = alpha * (p.alpha or 1); p = p.parent end
                local root = o
                while root.parent and not root.parent._root do root = root.parent end
                out[#out + 1] = { f = f, n = f.n, kind = f.kind, name = f.name, stock = f.stock, x = r.x, y = r.y, w = r.w, h = r.h,
                    strata = STRATA[o:GetFrameStrata()] or 3, level = o:GetFrameLevel(), layer = LAYER[f.layer or "ARTWORK"] or 3,
                    alpha = alpha, root = root.name or ("#" .. root.n) }
            end
        end
    end
    table.sort(out, function(a, b)
        if a.strata ~= b.strata then return a.strata < b.strata end
        if a.level ~= b.level then return a.level < b.level end
        local ar, br = (a.kind == "Texture" or a.kind == "FontString") and a.layer or 0, (b.kind == "Texture" or b.kind == "FontString") and b.layer or 0
        if ar ~= br then return ar < br end
        return a.n < b.n
    end)
    return out
end
function F.endLayout() cache = nil end

-- a frame's rect now, shown or not (the getters: GetLeft, GetWidth...)
function F.anyRect(f)
    if cache then return F.rect(f) end
    cache = {}
    local r = F.rect(f)
    cache = nil
    return r
end

-- the rect of a named or given frame (nil when hidden)
function F.rectOf(f)
    if type(f) == "string" then f = _G[f] end
    if not f or not f:IsVisible() then return nil end
    cache = {}
    local r = F.rect(f)
    cache = nil
    return r
end

local function jstr(v)
    if v == nil then return "null" end
    return '"' .. tostring(v):gsub('[%c"\\]', function(c) return string.format("\\u%04x", c:byte()) end) .. '"'
end
local function jnum(v) return v and string.format("%.2f", v) or "null" end
local function jcol(c) return c and ("[" .. jnum(c[1]) .. "," .. jnum(c[2]) .. "," .. jnum(c[3]) .. "," .. jnum(c[4] or 1) .. "]") or "null" end

function F.dump(path, title, notes)
    local rows = {}
    for _, e in ipairs(F.layout()) do
        local f = e.f
        local text = f.kind == "FontString" and f.text or (f.kind == "EditBox" and f.text) or nil
        local font = (f.kind == "FontString" or f.kind == "EditBox") and F.fontSize(f) or nil
        rows[#rows + 1] = string.format('{"n":%d,"kind":%s,"name":%s,"stock":%s,"x":%s,"y":%s,"w":%s,"h":%s,"strata":%d,"level":%d,"layer":%d,"alpha":%s,'
            .. '"color":%s,"file":%s,"text":%s,"font":%s,"tcolor":%s,"justify":%s,"model":%s,"desat":%s,"root":%s,"autoW":%s}',
            f.n, jstr(f.kind), jstr(f.name), jstr(f.stock), jnum(e.x), jnum(e.y), jnum(e.w), jnum(e.h), e.strata, e.level, e.layer, jnum(e.alpha),
            jcol(f.color or (f.file and f.vcolor) or nil), jstr(f.file), jstr(text), font and tostring(font) or "null", jcol(f.tcolor), jstr(f.justify),
            jstr(f.kind == "Model" and f.model or nil), f.desat and "true" or "false", jstr(e.root), f.autoW and "true" or "false")
    end
    F.endLayout()
    local h = assert(io.open(path, "wb"))
    h:write(string.format('{"title":%s,"notes":%s,"screen":{"w":%s,"h":%s},"regions":[\n%s\n]}\n', jstr(title), jstr(notes or ""),
        jnum(F.screen.w), jnum(F.screen.h), table.concat(rows, ",\n")))
    h:close()
end

return F
