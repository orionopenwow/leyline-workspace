-- Your Den's placement aids (Den Lite: labs/den-lite src/gizmo.rs). Owner, 3 Nov 10:29 - 10:36: "when mousing over
-- ground in game, it shows a preview of where it will land", "show the actual prop moving around ... a live preview",
-- "the 3 directional sphere movement arrows", "hovering over an object makes it glow; you can click it, select it ...
-- right-click options to delete".
--
-- The stock client has no cursor-to-world call, so:
--   * Build mode (in Your Den, not in play mode, on Den Lite: the Builder's state says lite=1): Den Lite shows the
--     props as objects the game highlights under the mouse; a right-click on one selects it (`wubuild picked`) and
--     this file opens a menu at the cursor (Delete, Duplicate, Move, Rotate, Scale, Snap to floor, Lock, Copy,
--     Properties). The camera is put in a known pose (the build camera: the player's own view kept in view 5 and put
--     back on leaving).
--   * A prop picked in the gallery is armed (`preview <id>`): Den Lite shows a copy of it, and while the cursor is
--     over the world this file sends where it is on the screen (`hover <u> <v> <aspect> <distance>`, about 10 a
--     second); Den Lite casts the ray from the build camera and moves the copy there. A left click on the world
--     places it (`commit`) and stays armed; a right click or Esc stops (`preview off`). Move in the menu arms the
--     selected prop the same way.
--   * The exact click: while armed, G (or this file's button) starts the ground-target cursor of the one spell Den
--     Lite taught (Flare): the game's own circle follows the mouse and the click lands exactly (Den Lite never casts
--     it). Each such click is also a calibration sample for the build camera (den-check "hover calibration").
--   * The gizmo: three flags around the selected prop (red X, green Y, blue Z). Right-click a flag (or X / Y / Z in
--     the axis tag) to pick that axis; then the mouse wheel moves the prop along it by the Builder's step.
-- Nothing here holds the keyboard: the override bindings (G, Esc, the wheel) are set only while armed or while an
-- axis is picked, and cleared after. WASD and the mouse's camera drags are never taken.
-- Talks over OVMOD as the Builder does (`wubuild cmd ...`); answers come as `wubuild aim|axis|picked|mode|ghost|state`.

local W = WrathUnleashedUI or {}
local G = { build = false, armed = nil, axis = nil, sel = nil, scale = 1, lastSend = 0, lastU = nil, lastV = nil }
W.Gizmo = G

G.PLACE_SPELL = 1543            -- gizmo.rs PLACE_SPELL
G.VIEW_KEEP = 5                 -- the player's own camera is kept in this saved view while building
G.VIEW_BUILD = 2                -- the build camera starts from this view (reset to the game's default first)
G.CAM_DIST = 12                 -- yards from the player, the distance Den Lite's ray uses (gizmo.rs Camera.dist)
G.HOVER_EVERY = 0.1             -- seconds between hovers (about 10 a second)
G.CLICK_TIME, G.CLICK_MOVE = 0.3, 6   -- a click, not a drag: shorter than this, moved less than this many pixels
G.AIM_KEY = "G"

local PREFIX, MOD = "OVMOD", "wubuild"

local function now() return GetTime and GetTime() or 0 end

local function send(text)
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, PREFIX, MOD .. "\tcmd\t1/1\t" .. text, "WHISPER", UnitName("player"))
    return true
end
G.Send = send

local function report(text)
    if not SendAddonMessage or not UnitName then return end
    pcall(SendAddonMessage, PREFIX, "wuden\treport\t1/1\t" .. text, "WHISPER", UnitName("player"))
end

-- Den Lite (its state says lite=1), in Your Den, not in play mode.
local function lite()
    local B = W.Builder
    return B and B.state and B.state.lite == "1" or false
end
function G.Wanted()
    local D = W.DenBar
    return (D and D.active and not D.play and lite()) and true or false
end

-- ---------------------------------------------------------------- the cursor on the screen

-- The cursor as Den Lite's ray wants it: u, v from -1 to 1 (right and up), and the screen's width over its height.
function G.Cursor()
    if not GetCursorPosition or not UIParent then return nil end
    local x, y = GetCursorPosition()
    local s = (UIParent.GetEffectiveScale and UIParent:GetEffectiveScale()) or 1
    local w, h = UIParent:GetWidth() or 0, UIParent:GetHeight() or 0
    if not x or w <= 0 or h <= 0 or s <= 0 then return nil end
    local u, v = 2 * (x / s) / w - 1, 2 * (y / s) / h - 1
    if u < -1 or u > 1 or v < -1 or v > 1 then return nil end
    return u, v, w / h
end

-- Over the 3D world, not over a window.
local function overWorld()
    if not GetMouseFocus or not WorldFrame then return true end
    local f = GetMouseFocus()
    return f == nil or f == WorldFrame
end

-- One hover when the cursor moved (at most every G.HOVER_EVERY seconds).
function G.Tick(force)
    if not G.armed or not overWorld() then return false end
    local t = now()
    if not force and t - G.lastSend < G.HOVER_EVERY then return false end
    local u, v, a = G.Cursor()
    if not u then return false end
    if not force and G.lastU and math.abs(u - G.lastU) < 0.002 and math.abs(v - G.lastV) < 0.002 then return false end
    G.lastSend, G.lastU, G.lastV = t, u, v
    return send(string.format("hover %.4f %.4f %.4f %d", u, v, a, G.CAM_DIST))
end

-- ---------------------------------------------------------------- the build camera

function G.CameraIn()
    if G.camera then return end
    G.camera = true
    if SaveView then pcall(SaveView, G.VIEW_KEEP) end
    if ResetView then pcall(ResetView, G.VIEW_BUILD) end
    if SetView then pcall(SetView, G.VIEW_BUILD) end
    -- a known distance: all the way in, then out by the build distance
    if CameraZoomIn then pcall(CameraZoomIn, 50) end
    if CameraZoomOut then pcall(CameraZoomOut, G.CAM_DIST) end
end

function G.CameraOut()
    if not G.camera then return end
    G.camera = nil
    if SetView then pcall(SetView, G.VIEW_KEEP) end
end

-- ---------------------------------------------------------------- the keys: only while needed

local owner, aimBtn, cancelBtn, wheelUp, wheelDown, tag, menu

local function bindings()
    if not owner or not SetOverrideBindingClick or not ClearOverrideBindings then return end
    if InCombatLockdown and InCombatLockdown() then return end
    ClearOverrideBindings(owner)
    if G.armed or G.axis or (menu and menu:IsShown()) then
        SetOverrideBindingClick(owner, true, "ESCAPE", "LeylineGizmoCancel")
    end
    if G.armed and aimBtn then
        SetOverrideBindingClick(owner, true, G.AIM_KEY, "LeylineGizmoAim")
    end
    if G.axis then
        SetOverrideBindingClick(owner, true, "MOUSEWHEELUP", "LeylineGizmoWheelUp")
        SetOverrideBindingClick(owner, true, "MOUSEWHEELDOWN", "LeylineGizmoWheelDown")
    end
end
G.Bindings = bindings

local AXIS_TEXT = { x = "X (red)", y = "Y (green)", z = "Z (blue)" }
local function showTag()
    if not tag then return end
    local parts = {}
    if G.armed then
        parts[#parts + 1] = (G.armed == "move" and "Moving" or "Placing") .. ": left-click the ground  ·  " .. G.AIM_KEY .. ": exact spot  ·  Esc stops"
    end
    if G.axis then
        parts[#parts + 1] = "Axis " .. (AXIS_TEXT[G.axis] or G.axis) .. ": the mouse wheel moves it"
    end
    if #parts == 0 then tag:Hide(); return end
    tag.fs:SetText(table.concat(parts, "   |   "))
    tag:Show()
end

function G.SetAxis(a)
    a = (a == "x" or a == "y" or a == "z") and a or nil
    if a ~= G.axis then
        G.axis = a
        send("axis " .. (a or "off"))
    end
    bindings(); showTag()
end

function G.Wheel(sign)
    if not G.axis then return false end
    local B = W.Builder
    if B and B.Move then return B.Move(G.axis, sign) end
    return send(string.format("move %s %s", G.axis, tostring(0.25 * sign)))
end

-- ---------------------------------------------------------------- arming

-- A gallery pick (Builder.lua B.Choose) in build mode: its copy follows the cursor.
function G.Arm(id)
    id = tonumber(id)
    if not id or not G.build then return false end
    G.lastU, G.lastV = nil, nil
    send("preview " .. id)
    G.armed = "place"            -- until Den Lite's aim says otherwise
    G.Tick(true)
    return true
end

function G.MoveSelected()
    if not G.build then return false end
    G.lastU, G.lastV = nil, nil
    send("preview move")
    G.armed = "move"
    G.Tick(true)
    return true
end

function G.Cancel()
    if menu and menu:IsShown() then menu:Hide() end
    if G.armed then send("preview off") end
    G.armed = nil
    if G.axis then G.SetAxis(nil) end
    bindings(); showTag()
end

-- ---------------------------------------------------------------- build mode

function G.SetBuild(on)
    on = on and true or false
    if on == G.build then return end
    G.build = on
    if on then
        send("mode build")
        send("gizmo on")
        G.CameraIn()
    else
        G.Cancel()
        send("gizmo off")
        send("mode play")
        G.CameraOut()
    end
end

-- ---------------------------------------------------------------- clicks on the world

local down = {}
function G.MouseDown(button)
    local x, y = 0, 0
    if GetCursorPosition then x, y = GetCursorPosition() end
    down[button] = { t = now(), x = x or 0, y = y or 0 }
end

-- The mouse over a gameobject the game names (its tooltip, no unit): a left click there isn't "empty ground".
local function overObject()
    if not GameTooltip or not GameTooltip.IsShown or not GameTooltip:IsShown() then return false end
    return not (UnitExists and UnitExists("mouseover"))
end

-- A click (short, still) on the world: left places (armed) or clears the selection; right stops.
function G.MouseUp(button)
    local d = down[button]
    down[button] = nil
    if not G.build or not d then return false end
    local x, y = 0, 0
    if GetCursorPosition then x, y = GetCursorPosition() end
    local click = (now() - d.t) <= G.CLICK_TIME and math.abs((x or 0) - d.x) <= G.CLICK_MOVE and math.abs((y or 0) - d.y) <= G.CLICK_MOVE
    if not click then
        -- a camera drag while armed: the build camera's pose again (the ray needs it)
        if button == "LeftButton" and G.armed and SetView then pcall(SetView, G.VIEW_BUILD) end
        return false
    end
    if button == "LeftButton" then
        if SpellIsTargeting and SpellIsTargeting() then return false end   -- the exact click: Den Lite gets the cast
        if G.armed then G.Tick(true); return send("commit") end
        if menu and menu:IsShown() then menu:Hide(); bindings(); return true end
        if not overObject() then return send("selclear") end
        return false
    elseif button == "RightButton" then
        if G.armed then G.Cancel(); return true end
    end
    return false
end

-- ---------------------------------------------------------------- the menu at the cursor

local MENU = {
    { "Delete", function() send("delete") end },
    { "Duplicate", function() send("dup") end },
    { "Move (follows the mouse)", function() G.MoveSelected() end },
    { "Rotate 15°", function() send("turn 15") end },
    { "Bigger", function() send(string.format("setscale %.2f", math.min(5, (G.scale or 1) * 1.25))) end },
    { "Smaller", function() send(string.format("setscale %.2f", math.max(0.1, (G.scale or 1) / 1.25))) end },
    { "Snap to floor", function() send("floor") end },
    { "Lock", function() send("lock") end },
    { "Copy", function() send("copy") end },
    { "Properties", function() local B = W.Builder; if B and B.ShowProps then B.ShowProps(true) end end },
}
G.MENU = MENU

local function buildMenu()
    if menu or not CreateFrame or not UIParent then return menu end
    menu = CreateFrame("Frame", "LeylineGizmoMenu", UIParent)
    menu:SetFrameStrata("DIALOG")
    menu:SetWidth(190); menu:SetHeight(#MENU * 22 + 12)
    if menu.SetBackdrop then
        menu:SetBackdrop({ bgFile = "Interface\\Tooltips\\UI-Tooltip-Background", edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border",
            tile = true, tileSize = 16, edgeSize = 14, insets = { left = 3, right = 3, top = 3, bottom = 3 } })
        if menu.SetBackdropColor then menu:SetBackdropColor(0.06, 0.07, 0.1, 0.95) end
    end
    menu.rows = {}
    for i, item in ipairs(MENU) do
        local b = CreateFrame("Button", "LeylineGizmoMenu" .. i, menu)
        b:SetWidth(178); b:SetHeight(20)
        b:SetPoint("TOPLEFT", menu, "TOPLEFT", 6, -6 - (i - 1) * 22)
        local fs = b.CreateFontString and b:CreateFontString(nil, "OVERLAY", "GameFontHighlight")
        if fs then fs:SetPoint("LEFT", b, "LEFT", 6, 0); fs:SetText(item[1]); b.label = fs end
        if b.SetHighlightTexture then b:SetHighlightTexture("Interface\\QuestFrame\\UI-QuestTitleHighlight", "ADD") end
        b:SetScript("OnClick", function() menu:Hide(); item[2](); bindings() end)
        menu.rows[i] = b
    end
    menu:Hide()
    return menu
end

-- Den Lite says a right-click selected this prop: the menu where the cursor is.
function G.Picked(id)
    G.sel = tonumber(id)
    local m = buildMenu()
    if not m then return end
    local x, y = 0, 0
    if GetCursorPosition then x, y = GetCursorPosition() end
    local s = (UIParent.GetEffectiveScale and UIParent:GetEffectiveScale()) or 1
    m:ClearAllPoints()
    m:SetPoint("TOPLEFT", UIParent, "BOTTOMLEFT", (x or 0) / s + 8, (y or 0) / s - 8)
    m:Show()
    bindings()
end

-- ---------------------------------------------------------------- Den Lite's answers

function G.OnMessage(prefix, msg, channel, sender)
    if prefix ~= PREFIX or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, i, n, data = msg:match("^([^\t]+)\t([^\t]+)\t(%d+)/(%d+)\t(.*)$")
    if mod ~= MOD or i ~= "1" or n ~= "1" then return end
    if name == "aim" then
        local kind = data:match("^(%a+)")
        G.armed = (kind == "place" or kind == "move") and kind or nil
        bindings(); showTag()
    elseif name == "axis" then
        G.axis = (data == "x" or data == "y" or data == "z") and data or nil
        bindings(); showTag()
    elseif name == "picked" then
        G.Picked(data)
    elseif name == "ghost" then
        G.ghost = data
    elseif name == "state" then
        local sc = data:match("pos=[^;]-,([%d%.]+);")
        G.scale = tonumber(sc) or G.scale
        if data:find("sel=-;", 1, true) then G.sel = nil end
    end
end

-- ---------------------------------------------------------------- the den-test (DenBar.lua D.TestAction "gizmo ...")

function G.TestAction(w)
    local a = w[2] or ""
    if a == "build" then
        local on = w[3] ~= "off"
        -- already so: say the mode again (Den Lite answers the look each time; the den-test reads that)
        if on == G.build then send(on and "mode build" or "mode play") else G.SetBuild(on) end
        return true
    end
    if a == "arm" then return G.Arm(w[3]) end
    if a == "move" then return G.MoveSelected() end
    if a == "hover" then
        -- the test's own point on the screen (u v), as if the cursor were there
        local u, v = tonumber(w[3]), tonumber(w[4])
        if not u or not v then return false end
        local _, _, asp = G.Cursor()
        return send(string.format("hover %.4f %.4f %.4f %d", u, v, asp or (16 / 9), G.CAM_DIST))
    end
    if a == "tick" then return G.Tick(true) end
    if a == "commit" then return send("commit") end
    if a == "cancel" then G.Cancel(); return true end
    if a == "axis" then G.SetAxis(w[3]); return true end
    if a == "wheel" then return G.Wheel(tonumber(w[3]) or 1) end
    if a == "menu" then
        local i = tonumber(w[3])
        local m = buildMenu()
        if i and MENU[i] then MENU[i][2](); if m then m:Hide() end; return true end
        return false
    end
    if a == "report" then
        local u, v, asp = G.Cursor()
        report(string.format("gizmo build=%d armed=%s axis=%s sel=%s camera=%d menu=%d cursor=%s,%s,%s ghost=%s",
            G.build and 1 or 0, G.armed or "-", G.axis or "-", tostring(G.sel or "-"), G.camera and 1 or 0,
            (menu and menu:IsShown()) and 1 or 0, u and string.format("%.3f", u) or "-", v and string.format("%.3f", v) or "-",
            asp and string.format("%.3f", asp) or "-", G.ghost or "-"))
        return true
    end
    return false
end

-- ---------------------------------------------------------------- set up

local function setup()
    if owner or not CreateFrame or not UIParent then return end
    owner = CreateFrame("Frame", "LeylineGizmo", UIParent)
    -- the exact click: a secure button that starts the spell's ground-target cursor (a click or the G key)
    local name = GetSpellInfo and GetSpellInfo(G.PLACE_SPELL)
    if name then
        local ok, b = pcall(CreateFrame, "Button", "LeylineGizmoAim", UIParent, "SecureActionButtonTemplate")
        if ok and b then
            aimBtn = b
            b:SetAttribute("type", "spell")
            b:SetAttribute("spell", name)
            -- shown (a hidden button may not take its binding's click) but 1 x 1, clear and out of the way
            b:SetWidth(1); b:SetHeight(1); b:SetAlpha(0)
            b:SetPoint("BOTTOMLEFT", UIParent, "BOTTOMLEFT", 0, 0)
        end
    end
    cancelBtn = CreateFrame("Button", "LeylineGizmoCancel", UIParent)
    cancelBtn:SetScript("OnClick", function() G.Cancel() end)
    cancelBtn:Hide()
    wheelUp = CreateFrame("Button", "LeylineGizmoWheelUp", UIParent)
    wheelUp:SetScript("OnClick", function() G.Wheel(1) end)
    wheelUp:Hide()
    wheelDown = CreateFrame("Button", "LeylineGizmoWheelDown", UIParent)
    wheelDown:SetScript("OnClick", function() G.Wheel(-1) end)
    wheelDown:Hide()
    -- the tag above the status pill: what is armed, which axis
    tag = CreateFrame("Frame", "LeylineGizmoTag", UIParent)
    tag:SetWidth(520); tag:SetHeight(22)
    local pill = _G and _G.LeylineEditorStatus
    if pill then tag:SetPoint("BOTTOM", pill, "TOP", 0, 6) else tag:SetPoint("BOTTOM", UIParent, "BOTTOM", 0, 150) end
    tag.fs = tag.CreateFontString and tag:CreateFontString(nil, "OVERLAY", "GameFontHighlight") or { SetText = function() end }
    if tag.fs.SetPoint then tag.fs:SetPoint("CENTER", tag, "CENTER", 0, 0) end
    tag:Hide()
    -- the clicks on the world (the hook never takes a click from the game: camera drags and movement go on)
    if WorldFrame and WorldFrame.HookScript then
        pcall(WorldFrame.HookScript, WorldFrame, "OnMouseDown", function(_, b) G.MouseDown(b) end)
        pcall(WorldFrame.HookScript, WorldFrame, "OnMouseUp", function(_, b) G.MouseUp(b) end)
    end
    -- the Builder's gallery pick arms the preview in build mode
    if hooksecurefunc and W.Builder and W.Builder.Choose then
        pcall(hooksecurefunc, W.Builder, "Choose", function(item) if item and not item.kind then G.Arm(item.id) end end)
    end
    local acc = 0
    owner:SetScript("OnUpdate", function(_, el)
        acc = acc + (el or 0)
        if acc >= 0.5 then
            acc = 0
            G.SetBuild(G.Wanted())
        end
        if G.armed then G.Tick(false) end
    end)
    owner:SetScript("OnEvent", function(_, ev, ...)
        if ev == "CHAT_MSG_ADDON" then G.OnMessage(...) end
    end)
    owner:RegisterEvent("CHAT_MSG_ADDON")
end
G.Setup = setup

if CreateFrame and UIParent then
    local ok = pcall(setup)
    if not ok then owner = nil end
end
