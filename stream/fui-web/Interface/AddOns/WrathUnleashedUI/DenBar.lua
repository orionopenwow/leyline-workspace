-- DenBar (3.3.5a API): Den mode, the one place to deal with Your Den (owner, 1 Nov 23:46: "There was no interface to
-- deal with the den. Nothing made sense ... make it make sense, not spam the chat"), redone 3 Nov as a game editor's
-- frame (owner: "very unintuitive, bulky, two square-like UI"; Style.lua S.Ed*: near-black, gold for what is on).
--
-- On entering Your Den (the launcher's own Den on this PC: realm "Your Den", W.InDen; or any Den whose server answers
-- the bar's hello) the editor's frame opens by itself (and anywhere else while the Builder is open):
--   the top strip (LeylineDenBar): "YOUR DEN" (elsewhere "BUILD"), Undo n / Redo n, Menu (the budget, select mine,
--                   copy, paste, duplicate, delete, group, put away, storage, floor, properties, the Den panel, Den
--                   home, edit mode, reload, help), Snap: off / 0.5 / 1 / 2 / 5 yd, the turn step 15° / 45° / 90°,
--                   the line (Den Lite's toasts, "Saved 12 s ago"), then Play (F8), ?, Files (the Den's save by
--                   name, builds, prefabs, the plot: Builder.lua), Invite and x (play mode; out of edit mode).
--   the tool rail (LeylineDenRail), down the left: Select, Build (F7: the gallery of props, Builder.lua), Move and
--                   Rotate (the selection's popover), Shape ground (asks the launcher to open Forge on the Den's
--                   ground: ".den forge"), Stamps, Path, and the move step's presets (0.1, 0.25, 1, 3 yd).
--   Play (F8)       play mode: every editor window, the shelf and the handles go away and you just walk your Den;
--                   the strip shrinks to one button. F8 again, that button, or Esc > Back to building returns.
--   Save            a save by name (".den save <name>", Files > This Den); the autosave keeps the Den anyway, and the
--                   line says when: "Saved 12 s ago".
--   Invite          one link for one friend (".den invite"); it lands in a copy box under the strip (Ctrl+C).
--   ?               the short intro again.
-- The first time (remembered in WrathUnleashedUIDB.den.introSeen) a short intro of four cards shows; Next / Skip.
--
-- Den Lite (labs/den-lite notice.rs) talks to the bar over the realm's addon channel, mod "wuden", whispered:
--   client -> server   hello <version>     the bar is here: Den Lite's own lines become toasts from now on
--                      status              send the status line again
--                      cmd <text>          a .den command (save, invite, forge), as the Den panel's
--   server -> client   status  role=..;props=<n>;saved=<seconds ago>|-
--                      toast   <kind>|<text>   kind: info, ok, warn, jaina, cheer: one line under the bar for a few
--                                              seconds, one at a time (a short queue, no repeats)
--                      saved   auto|<name>     the Den was just kept (the autosave, or a save by name)
--                      invite  <link>          the invite link (checked as EditMode.lua checks it)
-- Nothing the server sends is run; texts are shown plain (colour codes taken out).
--
-- Keys (override bindings, only while in the Den, given back on leaving): F7 Build and F8 Play when the player hasn't
-- bound them to something (a stock game binds neither); else Ctrl+F7 / Ctrl+F8; in the browser client F7 is its own
-- (the building camera), so only Play is bound.

local W = WrathUnleashedUI
local D = { toasts = {}, play = false, active = false, saved = nil, keys = {} }
local MOD = "wuden"
local VERSION = "1"
local TOAST_SECS = 5
local TOAST_QUEUE = 4
local NAME_MAX = 24
local BAR_W, BAR_H, RAIL_BTN = 900, 32, 30
local KEY_CHOICES = { build = { "F7", "CTRL-F7" }, play = { "F8", "CTRL-F8" } }
local KIND_COLOUR = { info = "text", ok = "ok", warn = "warn", jaina = "violet", cheer = "accent" }

-- TODO(owner): a voice pass on the intro cards, his words.
local CARDS = {
    { "This is your Den", "An empty white world that's yours. Everything you do here keeps itself: it saves as you go." },
    { "Build", "Build (F7, the box on the left) opens a gallery of props: double-click one to place it, then move, turn or delete it." },
    { "Shape ground", "Opens the world editor on your Den's ground: raise hills, dig, paint the ground. Your game shows it after a restart." },
    { "Play and invite", "Play (F8) hides the tools so you just walk your Den. Invite makes one link for a friend: send it, and they join you here." },
}
D.CARDS = CARDS

local bar, rail, menu, snapMenu, drawer, intro, menuButton, keyOwner

local function clock() return (GetTime and GetTime()) or 0 end
local function S() return W.Style end
local function plain(s)
    return (tostring(s or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("|H.-|h", ""):gsub("|h", ""):gsub("|T.-|t", ""):gsub("|", ""):gsub("%c", " "))
end
local function hex(name) local st = S(); return st and st.Hex(name) or "|cffffffff" end

local function send(name, data)
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, "OVMOD", MOD .. "\t" .. name .. "\t1/1\t" .. (data or ""), "WHISPER", UnitName("player"))
    return true
end
local function denCmd(text)
    local E = W.EditMode
    if E and E.DenCmd then return E.DenCmd(text) end
    return send("cmd", (tostring(text or ""):gsub("[^%w%s%-_]", ""):sub(1, 60)))
end

local function db()
    WrathUnleashedUIDB = type(WrathUnleashedUIDB) == "table" and WrathUnleashedUIDB or {}
    local d = WrathUnleashedUIDB
    d.den = type(d.den) == "table" and d.den or {}
    return d.den
end

-- ---------------------------------------------------------------- the status line: toasts, and "Saved 12 s ago"
function D.Ago(secs)
    secs = math.max(0, math.floor(tonumber(secs) or 0))
    if secs < 60 then return secs .. " s ago" end
    if secs < 3600 then return math.floor(secs / 60) .. " min ago" end
    return math.floor(secs / 3600) .. " h ago"
end

function D.SavedLine()
    local s = D.saved
    if not s then return "Your Den saves itself as you build." end
    local what = (s.name and s.name ~= "auto") and ("Saved as " .. s.name .. " ") or "Saved "
    return what .. D.Ago(clock() - s.at)
end

local function showLine()
    if not bar then return end
    local t = D.current
    local text = t and (hex(KIND_COLOUR[t.kind] or "text") .. (t.kind == "jaina" and "Jaina: " or "") .. t.text .. "|r") or (hex("muted") .. D.SavedLine() .. "|r")
    bar.line:SetText(text)
    if bar.pill then bar.pill.line:SetText(text) end
end

-- One line in the bar for TOAST_SECS, one at a time; the same line twice in a row shows once. `now`: before the
-- others (the bar's own answer to a click).
function D.Toast(kind, text, now)
    text = plain(text):gsub("^%s+", ""):gsub("%s+$", "")
    if text == "" then return false end
    kind = KIND_COLOUR[kind or ""] and kind or "info"
    local last = D.toasts[#D.toasts] or D.current
    if last and last.text == text then return false end
    if #D.toasts >= TOAST_QUEUE then table.remove(D.toasts, 1) end
    if now then
        table.insert(D.toasts, 1, { kind = kind, text = text })
        D.NextToast()
    else
        D.toasts[#D.toasts + 1] = { kind = kind, text = text }
        if not D.current then D.NextToast() end
    end
    -- the Builder's own status line says it too, when its window is open (its answers used to be in the chat)
    local B = W.Builder
    local win = B and B.Window and B.Window()
    if win and win:IsShown() and B.SetStatus and kind ~= "jaina" and kind ~= "cheer" then B.SetStatus(text) end
    return true
end

function D.NextToast()
    D.current = table.remove(D.toasts, 1)
    if D.current then D.current.until_ = clock() + TOAST_SECS end
    showLine()
end

function D.Tick()
    if D.current and clock() >= D.current.until_ then D.NextToast() end
    if not D.current then showLine() end   -- "Saved 12 s ago" counts on
end

-- ---------------------------------------------------------------- the top strip and the tool rail
local function ED() return S().ED end
local function chip(parent, text, w, onClick, ...) return S().EdChip(parent, text, w, onClick, ...) end
local function icon(parent, path, text, onClick, ...) return S().EdIcon(parent, RAIL_BTN, "Interface\\Icons\\" .. path, text, onClick, ...) end
local function B() return W.Builder end
local function E() return W.EditMode end
local function editing() local e = E(); return e and e.IsOn and e.IsOn() end

local function keyText(which) local k = D.keys[which]; return k and (" (" .. k:gsub("CTRL%-", "Ctrl+") .. ")") or "" end

function D.Labels()
    if not bar then return end
    bar.build:SetText("Build" .. keyText("build"))
    bar.playBtn:SetText("Play" .. keyText("play"))
    if bar.pill then bar.pill.back:SetText("Back to building" .. keyText("play")) end
end

-- The menu (the strip's Menu): everything that isn't a tool of its own, one row each.
local MENU = {
    { "Select mine", function() local b = B(); if b then b.Command("selall") end end },
    { "Select nearest", function() local b = B(); if b then b.Command("near") end end },
    { "Clear selection", function() local b = B(); if b then b.Command("selclear") end end },
    { "Box select (edit mode)", function() local e = E(); if e and editing() then e.StartBox() end end, "edit" },
    { "Copy", function() local b = B(); if b then b.Command("copy") end end },
    { "Paste", function() local b = B(); if b then b.Paste() end end },
    { "Duplicate", function() local b = B(); if b then b.Command("dup") end end },
    { "Delete", function() local b = B(); if b then b.Command("delete") end end },
    { "Group", function() local b = B(); if b then b.Command("group") end end },
    { "Ungroup", function() local b = B(); if b then b.Command("ungroup") end end },
    { "Put away", function() local b = B(); if b then b.Command("store") end end },
    { "Storage", function() local b = B(); if b then b.ShowBuilder(true); b.ShowTab("storage") end end },
    { "Floor", function() local b = B(); if b then b.Command("floor") end end },
    { "Properties", function() local b = B(); if b then b.ToggleProps() end end },
    { "Den panel", function() local e = E(); if e and e.ToggleDen then e.ToggleDen() end end, "den" },
    { "Den home", function() if W.DenHome and W.DenHome.Toggle then W.DenHome.Toggle() end end, "den" },
    { "Edit mode (F7)", function() local e = E(); if e and e.Toggle then e.Toggle() end end, "browser" },
    { "Reload the props", function() local b = B(); if b then b.RequestCatalog() end end },
    { "Builder help", function() local b = B(); if b then b.Command("help") end end },
}
D.MENU = MENU

local function closeMenus(except)
    if menu and menu ~= except then menu:Hide() end
    if snapMenu and snapMenu ~= except then snapMenu:Hide() end
end

local function buildMenu()
    menu = CreateFrame("Frame", "LeylineDenMenu", bar)
    menu:SetWidth(190)
    menu:SetPoint("TOPLEFT", bar.menuBtn, "BOTTOMLEFT", 0, -4)
    menu:SetFrameStrata("DIALOG")
    menu:EnableMouse(true)
    S().EdPanel(menu, 0.96, ED().panel2)
    -- the budget first: props and points (or the Den's room and yard)
    menu.meter = menu:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    menu.meter:SetPoint("TOPLEFT", 8, -8); menu.meter:SetWidth(176); menu.meter:SetJustifyH("LEFT")
    menu.meter.styleKeep = true
    menu.rows = {}
    local y = -26
    for i, it in ipairs(MENU) do
        local r = chip(menu, it[1], 176, function() closeMenus(); it[2]() end, "TOPLEFT", 7, y)
        if r.edText then r.edText:ClearAllPoints(); r.edText:SetPoint("LEFT", 8, 0); r.edText:SetJustifyH("LEFT") end
        r.only = it[3]
        menu.rows[i] = r
        y = y - 23
    end
    menu.bottom = y
    menu:Hide()
end

function D.ShowMenu(show)
    D.Bar()
    if not menu then buildMenu() end
    if show == nil then show = not menu:IsShown() end
    closeMenus(menu)
    if not show then menu:Hide(); return false end
    local b = B()
    local s = (b and b.state) or {}
    menu.meter:SetText(s.den and string.format("Room %s   Yard %s", s.room or "?", s.yard or "?") or (b and b.Meter and (b.Meter()) or ""))
    local e = E()
    local browser = e and e.Desktop and not e.Desktop()
    local y = -26
    for _, r in ipairs(menu.rows) do
        local on = not r.only or (r.only == "den" and D.active) or (r.only == "edit" and editing()) or (r.only == "browser" and browser)
        if on then r:ClearAllPoints(); r:SetPoint("TOPLEFT", menu, "TOPLEFT", 7, y); r:Show(); y = y - 23 else r:Hide() end
    end
    menu:SetHeight(-y + 6)
    menu:Show()
    return true
end
function D.Menu() return menu end

local function snapText(g) return (g and g > 0) and ("Snap: " .. g .. " yd") or "Snap: off" end
local function buildSnapMenu()
    snapMenu = CreateFrame("Frame", "LeylineDenSnapMenu", bar)
    snapMenu:SetWidth(92)
    snapMenu:SetPoint("TOPLEFT", bar.snap, "BOTTOMLEFT", 0, -4)
    snapMenu:SetFrameStrata("DIALOG")
    snapMenu:EnableMouse(true)
    S().EdPanel(snapMenu, 0.96, ED().panel2)
    snapMenu.rows = {}
    local list = (B() and B().SNAPS) or { 0, 0.5, 1, 2, 5 }
    for i, g in ipairs(list) do
        local r = chip(snapMenu, g > 0 and (g .. " yd") or "off", 80, function()
            closeMenus()
            local b = B()
            if b then b.SetSnap(g) end
            D.Refresh()
        end, "TOPLEFT", 6, -6 - (i - 1) * 23)
        r.grid = g
        snapMenu.rows[i] = r
    end
    snapMenu:SetHeight(#list * 23 + 10)
    snapMenu:Hide()
end
function D.ShowSnap(show)
    D.Bar()
    if not snapMenu then buildSnapMenu() end
    if show == nil then show = not snapMenu:IsShown() end
    closeMenus(snapMenu)
    if show then snapMenu:Show() else snapMenu:Hide() end
    return show
end
function D.SnapMenu() D.Bar(); if not snapMenu then buildSnapMenu() end return snapMenu end

-- The strip's close: out of edit mode; in a Den, play mode (the tools away); elsewhere the editor's windows shut.
function D.Close()
    closeMenus()
    local e = E()
    if editing() and e.Set then e.Set(false); return "edit" end
    if D.active then D.SetPlay(true); return "play" end
    local b = B()
    if b then
        if b.Window() then b.Window():Hide() end
        if b.Panel() then b.Panel():Hide() end
        if b.Files then local f = b.Files(); if f:IsShown() then f:Hide() end end
    end
    D.Refresh()
    return "closed"
end

-- The rail's tools.
function D.Tool(which)
    local b = B()
    if not b then return end
    closeMenus()
    if which == "select" then
        local e = E()
        if editing() then
            if b.mode == "select" and e.StartBox then e.StartBox() elseif e.Cancel then e.Cancel("Click a prop to select it; Select again for a box.") end
        else
            b.Command("near")
            b.SetStatus("Selected the prop nearest you. Shift-click in edit mode adds more.")
        end
        b.SetMode("select")
    elseif which == "place" then
        return D.Build()
    elseif which == "move" or which == "rotate" then
        b.Focus(which)
    elseif which == "stamps" then
        b.ShowBuilder(true); b.ShowTab("stamps"); b.SetMode("stamps")
    elseif which == "path" then
        b.Path()
    end
    D.Refresh()
end

local function buildBar()
    -- the strip: across the top, near-black and translucent
    bar = CreateFrame("Frame", "LeylineDenBar", UIParent)
    bar:SetHeight(BAR_H)
    bar:SetPoint("TOPLEFT", UIParent, "TOPLEFT", 0, 0)
    bar:SetPoint("TOPRIGHT", UIParent, "TOPRIGHT", 0, 0)
    bar:SetWidth(BAR_W)
    bar:SetFrameStrata("MEDIUM")
    bar:EnableMouse(true)
    S().EdPanel(bar, 0.84)
    bar.mark = bar:CreateTexture(nil, "ARTWORK")
    bar.mark:SetTexture(S().MARK)
    bar.mark:SetWidth(18); bar.mark:SetHeight(18)
    bar.mark:SetPoint("LEFT", 10, 0)
    bar.title = S().EdLabel(bar, "Your Den", true, "LEFT", 34, 0)
    local x = 120
    local function add(key, text, w, fn)
        local b = chip(bar, text, w, fn, "LEFT", x, 0)
        x = x + w + 4
        bar[key] = b
        return b
    end
    add("undo", "Undo 0", 60, function() local b = B(); if b then b.Command("undo") end end)
    add("redo", "Redo 0", 60, function() local b = B(); if b then b.Command("redo") end end)
    add("menuBtn", "Menu", 52, function() D.ShowMenu() end)
    x = x + 8
    add("snap", "Snap: off", 78, function() D.ShowSnap() end)
    bar.rot = {}
    for _, a in ipairs({ 15, 45, 90 }) do
        bar.rot[a] = add("rot" .. a, a .. "°", 36, function() local b = B(); if b then b.SetAngle(a) end; D.Refresh() end)
        bar.rot[a].edLabel = "Q / E turn by " .. a .. " degrees"
    end
    -- the line: Den Lite's toasts, one at a time, and "Saved 12 s ago"
    bar.line = bar:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    bar.line:SetPoint("LEFT", x + 12, 0)
    bar.line:SetPoint("RIGHT", bar, "RIGHT", -300, 0)
    bar.line:SetJustifyH("CENTER")
    bar.line.styleKeep = true
    -- the right: Play, ?, Files, Invite, close
    bar.close = chip(bar, "x", 24, function() D.Close() end, "RIGHT", -6, 0)
    bar.invite = chip(bar, "Invite", 56, function() D.Invite() end, "RIGHT", -34, 0)
    bar.save = chip(bar, "Files", 52, function() local b = B(); if b then b.ShowFiles() end end, "RIGHT", -94, 0)
    bar.help = chip(bar, "?", 24, function() D.ShowIntro(1) end, "RIGHT", -150, 0)
    bar.playBtn = chip(bar, "Play", 78, function() D.SetPlay(true) end, "RIGHT", -178, 0)

    -- the rail: down the left, square icon buttons, the tool that is on in gold
    rail = CreateFrame("Frame", "LeylineDenRail", UIParent)
    rail:SetWidth(RAIL_BTN + 8)
    rail:SetPoint("TOPLEFT", UIParent, "TOPLEFT", 8, -(BAR_H + 10))
    rail:SetFrameStrata("MEDIUM")
    rail:EnableMouse(true)
    S().EdPanel(rail, 0.84)
    local y = -4
    local function tool(key, path, text, fn)
        local b = icon(rail, path, text, fn, "TOPLEFT", 4, y)
        y = y - RAIL_BTN - 4
        bar[key] = b
        return b
    end
    tool("select", "INV_Misc_Spyglass_02", "Select", function() D.Tool("select") end)
    tool("build", "INV_Crate_01", "Build", function() D.Tool("place") end)
    tool("move", "Ability_Rogue_Sprint", "Move", function() D.Tool("move") end)
    tool("rotate", "Ability_Whirlwind", "Rotate", function() D.Tool("rotate") end)
    tool("ground", "Spell_Nature_Earthquake", "Shape ground", function() D.ShapeGround() end)
    tool("stamps", "INV_Misc_Map_01", "Stamps", function() D.Tool("stamps") end)
    tool("path", "Ability_Tracking", "Path", function() D.Tool("path") end)
    bar.build.edLabel = "Build"
    y = y - 4
    bar.stepLabel = S().EdLabel(rail, "Step", false, "TOP", rail, "TOP", 0, y)
    y = y - 14
    bar.steps = {}
    local Bm = B()
    for i, si in ipairs((Bm and Bm.STEP_PRESETS) or {}) do
        local yd = Bm.STEPS[si]
        local b = chip(rail, tostring(yd), RAIL_BTN, function() local b2 = B(); if b2 then b2.SetStep(si) end; D.Refresh() end, "TOPLEFT", 4, y)
        b.edLabel = "Move " .. yd .. " yd a step"
        b.step = si
        bar.steps[i] = b
        y = y - 26
    end
    rail:SetHeight(-y + 2)
    for _, k in ipairs({ "select", "build", "move", "rotate", "ground", "stamps", "path" }) do
        bar[k].edTipAt, bar[k].edTipTo = "LEFT", "RIGHT"
    end
    rail:Hide()
    bar.rail = rail

    -- play mode: one small button and the line
    local pill = CreateFrame("Frame", "LeylineDenPlayPill", UIParent)
    pill:SetWidth(330); pill:SetHeight(30)
    pill:SetPoint("TOP", UIParent, "TOP", 0, -6)
    pill:EnableMouse(true)
    S().EdPanel(pill, 0.72)
    pill.back = chip(pill, "Back to building", 150, function() D.SetPlay(false) end, "LEFT", 4, 0)
    pill.line = pill:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    pill.line:SetPoint("LEFT", 160, 0)
    pill.line:SetWidth(166); pill.line:SetJustifyH("LEFT")
    pill.line.styleKeep = true
    pill:Hide()
    bar.pill = pill

    -- the drawer under the strip: a name for Save, or the invite link to copy
    drawer = CreateFrame("Frame", "LeylineDenDrawer", bar)
    drawer:SetWidth(500); drawer:SetHeight(34)
    drawer:SetPoint("TOPRIGHT", bar, "BOTTOMRIGHT", -6, -4)
    drawer:EnableMouse(true)
    S().EdPanel(drawer, 0.95, ED().panel2)
    drawer.label = S().EdLabel(drawer, "", false, "LEFT", 10, 0)
    drawer.label:SetWidth(100)
    drawer.box = S().EdInput(drawer, 260, 20)
    drawer.box:SetPoint("LEFT", 112, 0)
    drawer.box:SetScript("OnEscapePressed", function(self) self:ClearFocus(); D.CloseDrawer() end)
    drawer.box:SetScript("OnEnterPressed", function() if D.drawerKind == "save" then D.Save(drawer.box:GetText()) else D.CloseDrawer() end end)
    drawer.box:SetScript("OnEditFocusGained", function(self) if D.drawerKind == "invite" then self:HighlightText() end end)
    -- the link stays as it came (a typed key doesn't change it)
    drawer.box:SetScript("OnTextChanged", function(self)
        if D.drawerKind == "invite" and D.link and self:GetText() ~= D.link then self:SetText(D.link); self:HighlightText() end
    end)
    drawer.ok = chip(drawer, "Save", 56, function() if D.drawerKind == "save" then D.Save(drawer.box:GetText()) else D.CloseDrawer() end end, "LEFT", 378, 0)
    drawer.ok:SetActive(true)
    drawer.close = chip(drawer, "Close", 52, function() D.CloseDrawer() end, "LEFT", 438, 0)
    drawer:Hide()

    bar:Hide()
    D.Labels()
end
function D.Bar() if not bar then buildBar() end return bar end
function D.Rail() D.Bar(); return rail end
function D.Drawer() D.Bar(); return drawer end

-- The strip and the rail say what is on now: the title, Undo n / Redo n, Snap, the turn step, the tool, the step.
function D.Paint()
    if not bar then return end
    local b = B()
    local s = (b and b.state) or {}
    bar.title:SetText(D.active and "YOUR DEN" or "BUILD")
    bar.undo:SetText("Undo " .. (tonumber(s.undo or "") or 0))
    bar.redo:SetText("Redo " .. (tonumber(s.redo or "") or 0))
    if b then
        bar.snap:SetText(snapText(b.Snap()))
        local a = b.TurnStep()
        for deg, c in pairs(bar.rot) do c:SetActive(deg == a) end
        local win = b.Window and b.Window()
        local gallery = win and win:IsShown()
        bar.build:SetActive(gallery and (b.mode == "place" or b.mode == nil))
        bar.select:SetActive(b.mode == "select")
        bar.move:SetActive(b.mode == "move")
        bar.rotate:SetActive(b.mode == "rotate")
        bar.stamps:SetActive(gallery and b.tab == "stamps")
        bar.path:SetActive(b.mode == "path")
        for _, c in ipairs(bar.steps) do c:SetActive(c.step == b.step) end
    end
    -- what is only for a Den: the ground, Play, Invite
    for _, k in ipairs({ "ground", "playBtn", "invite" }) do if D.active then bar[k]:Show() else bar[k]:Hide() end end
end

-- In the browser's edit mode the UI is hidden: the strip, the rail and their menus go on the world frame.
function D.Dock(toWorld)
    local b = B()
    if not (b and b.Dock) then return end
    D.Bar()
    for _, f in ipairs({ bar, rail, bar.pill }) do b.Dock(f, toWorld) end
end

function D.OpenDrawer(kind)
    D.Bar()
    D.drawerKind = kind
    if kind == "save" then
        drawer.label:SetText("Name this save:")
        drawer.ok:SetText("Save")
        drawer.box:SetMaxLetters(NAME_MAX)
        drawer.box:SetText(D.lastName or "my-den")
        drawer.box:SetFocus()
        drawer.box:HighlightText()
    else
        drawer.label:SetText("Invite link:")
        drawer.ok:SetText("Done")
        drawer.box:SetMaxLetters(0)
        drawer.box:SetText(D.link or "")
        if D.link then drawer.box:SetFocus(); drawer.box:HighlightText() end
    end
    drawer:Show()
end
function D.CloseDrawer()
    if not drawer then return end
    D.drawerKind = nil
    drawer.box:ClearFocus()
    drawer:Hide()
end

-- ---------------------------------------------------------------- the buttons
function D.Build()
    if D.play then D.SetPlay(false) end
    local E = W.EditMode
    -- the browser client's free building camera (the gallery comes with it); on a player's own game the gallery
    if E and E.Desktop and not E.Desktop() and E.Toggle and not E.IsOn() then E.Toggle(); return "camera" end
    local B = W.Builder
    if B and B.SetMode then B.SetMode("place") end
    if W.ToggleBuilder then W.ToggleBuilder() end
    D.Refresh()
    return "builder"
end

function D.ShapeGround()
    denCmd("forge")
    return true
end

function D.OpenSave() D.OpenDrawer("save") end

function D.Save(name)
    local n = tostring(name or ""):gsub("^%s+", ""):gsub("%s+$", ""):gsub("%s+", "-")
    if n == "" or #n > NAME_MAX or not n:match("^[%w_%-]+$") then
        D.Toast("warn", "A save's name: letters, numbers, - and _ (up to " .. NAME_MAX .. ").", true)
        return false
    end
    D.lastName = n
    denCmd("save " .. n)
    D.CloseDrawer()
    return true
end

function D.Invite()
    D.link = nil
    denCmd("invite")
    D.OpenDrawer("invite")
    drawer.box:SetText("")
    drawer.label:SetText("Making a link...")
    return true
end

-- What play mode hides: every editor window, the shelf (with edit mode), what is being placed, the selection.
local function editorFrames()
    local out = {}
    local B, E, H = W.Builder, W.EditMode, W.DenHome
    local function add(f) if f then out[#out + 1] = f end end
    if B then add(B.Window and B.Window()); add(B.Panel and B.Panel()); add(B.Files and B.Files()) end
    if E then add(E.DenPanel and E.DenPanel()) end
    if H then add(H.Panel and H.Panel()) end
    add(intro); add(drawer)
    return out
end

function D.SetPlay(on)
    on = on and true or false
    if not D.active then on = false end
    D.play = on
    if on then
        local E, H, B = W.EditMode, W.DenHome, W.Builder
        if H and H.Placing and H.Placing() and H.Cancel then H.Cancel(true) end
        if E and E.IsOn and (E.IsOn() or E.allowed or E.asking) and E.Leave then E.Leave() end
        for _, f in ipairs(editorFrames()) do if f.IsShown and f:IsShown() then f:Hide() end end
        D.CloseDrawer()
        closeMenus()
        -- no handles on a selected prop while you walk
        if B and B.state and B.state.selId and B.Command then B.Command("selclear") end
        if LeylineEdit then pcall(LeylineEdit, "group") end
        D.Toast("info", "Play mode: just you and your Den. " .. (D.keys.play and (D.keys.play:gsub("CTRL%-", "Ctrl+") .. " or ") or "") .. "Esc > Back to building.", true)
    end
    D.Refresh()
    return D.play
end
function D.TogglePlay() return D.SetPlay(not D.play) end

-- ---------------------------------------------------------------- the intro
local function buildIntro()
    intro = CreateFrame("Frame", "LeylineDenIntro", UIParent)
    intro:SetWidth(400); intro:SetHeight(190)
    intro:SetPoint("CENTER", UIParent, "CENTER", 0, 80)
    intro:SetFrameStrata("DIALOG")
    intro:EnableMouse(true)
    if S() then S().Window(intro, "Your Den") end
    intro.head = intro:CreateFontString(nil, "OVERLAY", "GameFontNormalLarge")
    intro.head:SetPoint("TOPLEFT", 16, -44)
    intro.head:SetJustifyH("LEFT")
    intro.body = intro:CreateFontString(nil, "OVERLAY", "GameFontHighlight")
    intro.body:SetPoint("TOPLEFT", 16, -72)
    intro.body:SetWidth(368); intro.body:SetJustifyH("LEFT")
    intro.count = intro:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    intro.count:SetPoint("BOTTOMLEFT", 16, 16)
    intro.skip = chip(intro, "Skip", 70, function() D.EndIntro() end, "BOTTOMRIGHT", -104, 10)
    intro.next = chip(intro, "Next", 86, function() D.NextCard() end, "BOTTOMRIGHT", -12, 10)
    intro.next:SetActive(true)
    if UISpecialFrames then tinsert(UISpecialFrames, "LeylineDenIntro") end
    intro:SetScript("OnHide", function() if D.card then D.EndIntro() end end)
    intro:Hide()
end

function D.ShowIntro(n)
    if not intro then buildIntro() end
    if D.play then D.SetPlay(false) end
    D.card = math.max(1, math.min(#CARDS, tonumber(n) or 1))
    local c = CARDS[D.card]
    intro.head:SetText(c[1])
    intro.body:SetText(c[2])
    intro.count:SetText(D.card .. " / " .. #CARDS)
    intro.next:SetText(D.card == #CARDS and "Start building" or "Next")
    if D.card == #CARDS then intro.skip:Hide() else intro.skip:Show() end
    intro:Show()
    return D.card
end
function D.NextCard()
    if not D.card then return nil end
    if D.card >= #CARDS then D.EndIntro(); return nil end
    return D.ShowIntro(D.card + 1)
end
function D.EndIntro()
    D.card = nil
    db().introSeen = true
    if intro and intro:IsShown() then intro:Hide() end
end
function D.Intro() return intro end

-- ---------------------------------------------------------------- keys and the Esc menu
local keyDownSeen, keyHalf = false, {}
local function keyButton(which, fn)
    local name = which == "build" and "LeylineDenKeyBuild" or "LeylineDenKeyPlay"
    if _G[name] then return name end
    local b = CreateFrame("Button", name, UIParent)
    b:EnableMouse(false)
    if b.RegisterForClicks then b:RegisterForClicks("AnyDown", "AnyUp") end
    -- a click binding clicks as the key goes down and as it comes up (DenHome.lua's way): once per press
    b:SetScript("OnClick", function(_, _, down)
        if down == true then keyDownSeen = true end
        if keyDownSeen then
            if down == true then fn() end
        else
            keyHalf[which] = not keyHalf[which]
            if keyHalf[which] then fn() end
        end
    end)
    return name
end

local function free(key)
    if not GetBindingAction then return true end
    local ok, a = pcall(GetBindingAction, key)
    return not ok or a == nil or a == ""
end

function D.BindKeys()
    if not keyOwner then keyOwner = CreateFrame("Frame", "LeylineDenBarKeys", UIParent) end
    if ClearOverrideBindings then ClearOverrideBindings(keyOwner) end
    D.keys = {}
    if D.active and SetOverrideBindingClick and not (InCombatLockdown and InCombatLockdown()) then
        local E = W.EditMode
        local browser = E and E.Desktop and not E.Desktop()
        for _, which in ipairs({ "build", "play" }) do
            if not (which == "build" and browser) then
                for _, k in ipairs(KEY_CHOICES[which]) do
                    if free(k) then
                        SetOverrideBindingClick(keyOwner, false, k, keyButton(which, which == "build" and D.Build or D.TogglePlay))
                        D.keys[which] = k
                        break
                    end
                end
            elseif browser then
                D.keys.build = "F7"   -- the browser client's own key
            end
        end
    end
    D.Labels()
    return D.keys
end

local function menuRefresh()
    if not GameMenuFrame then return end
    if not menuButton then
        menuButton = CreateFrame("Button", "GameMenuButtonLeylineDen", GameMenuFrame, "GameMenuButtonTemplate")
        menuButton:SetScript("OnClick", function()
            if HideUIPanel then HideUIPanel(GameMenuFrame) end
            D.TogglePlay()
        end)
    end
    menuButton:SetText(D.play and "Back to building" or "Play your Den")
    if D.active then menuButton:Show() else menuButton:Hide() end
    if GameMenuFrame:IsShown() and W.LayoutGameMenu then W.LayoutGameMenu() end
end
function D.MenuButton() return menuButton end

-- ---------------------------------------------------------------- on and off
-- The frame shows in a Den (not in play mode), and anywhere while an editor window of the Builder is open.
function D.Refresh()
    if D.refreshing then return end
    D.refreshing = true
    D.Bar()
    local B = W.Builder
    local open = B and B.EditorOpen and B.EditorOpen()
    local want = (D.active and not D.play) or (open and not D.play) or false
    D.Dock(editing() and true or false)
    if want then bar:Show(); rail:Show() else bar:Hide(); rail:Hide(); closeMenus() end
    if D.active and D.play then bar.pill:Show() else bar.pill:Hide() end
    if B and B.ShowPill then B.ShowPill(want) end
    D.Paint()
    menuRefresh()
    showLine()
    D.refreshing = nil
end

function D.SetActive(on)
    on = on and true or false
    if D.active == on then return end
    D.active = on
    if not on then
        D.play = false; D.CloseDrawer(); if intro then intro:Hide() end
        -- leaving the Den: its editor windows go with it
        for _, f in ipairs(editorFrames()) do if f.IsShown and f:IsShown() then f:Hide() end end
    end
    D.BindKeys()
    D.Refresh()
    if on and not db().introSeen then D.ShowIntro(1) end
end
function D.IsActive() return D.active end

-- Entering the world: in Your Den the bar opens and says hello; Den Lite answers with the status (any Den that
-- answers gets the bar too). Elsewhere it stays shut (and gives the keys back).
function D.Enter()
    local inDen = W.InDen and W.InDen() or false
    D.SetActive(inDen)
    send("hello", VERSION)
end

-- ---------------------------------------------------------------- the server's answers
local function inviteOk(data)
    return data:match("^https://[%w%.%-]+[:%d]*/join/[a-z2-7]+#[A-Z2-9]+$")
        or data:match("^https://[%w%.%-]+[:%d]*/join/[a-z2-7]+#[A-Z2-9]+~~G[A-Z0-9]+%.[A-Z0-9]+$")
end

function D.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= MOD then return end
    if name == "status" then
        local s = {}
        for k, v in data:gmatch("(%a+)=([^;]*)") do s[k] = plain(v) end
        D.status = s
        local ago = tonumber(s.saved or "")
        if ago then D.saved = { at = clock() - ago, name = (D.saved and D.saved.name) or "auto" } end
        D.SetActive(true)
        showLine()
    elseif name == "toast" then
        local kind, text = data:match("^(%a+)|(.*)$")
        if kind then D.Toast(kind, text) end
    elseif name == "saved" then
        local n = plain(data):gsub("[^%w_%-]", ""):sub(1, NAME_MAX)
        D.saved = { at = clock(), name = n ~= "" and n or "auto" }
        showLine()
    elseif name == "invite" and inviteOk(data) then
        D.link = data
        if D.active then
            D.OpenDrawer("invite")
            drawer.label:SetText("Invite link:")
            drawer.box:SetText(data)
            drawer.box:SetFocus()
            drawer.box:HighlightText()
        end
    end
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:RegisterEvent("PLAYER_ENTERING_WORLD")
listener:SetScript("OnEvent", function(_, event, prefix, msg, channel, sender)
    if event == "CHAT_MSG_ADDON" then return D.OnMessage(prefix, msg, channel, sender) end
    if event == "PLAYER_ENTERING_WORLD" then return D.Enter() end
end)
local ticker = CreateFrame("Frame", nil, WorldFrame or UIParent)
local acc = 0
ticker:SetScript("OnUpdate", function(_, elapsed)
    acc = acc + (elapsed or 0)
    if acc < 0.5 then return end
    acc = 0
    if D.active then D.Tick() end
end)

SLASH_LEYLINEDENBAR1 = "/denbar"
SlashCmdList["LEYLINEDENBAR"] = function(msg)
    msg = tostring(msg or ""):lower()
    if msg == "play" then D.TogglePlay() elseif msg == "help" then D.ShowIntro(1) else D.SetActive(not D.active) end
end

D.VERSION, D.TOAST_SECS, D.KEY_CHOICES = VERSION, TOAST_SECS, KEY_CHOICES
W.DenBar = D
