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
--                   ground: ".den forge"), Stamps, Path, Dojo (the training grounds' panel: Dojo.lua), and the move
--                   step's presets (0.1, 0.25, 1, 3 yd).
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
local BAR_H, RAIL_W, RAIL_BTN = 40, 64, 30   -- the strip's height, the rail's width, a rail icon
local RAIL_TOP = 118                          -- under the game's player frame (104 high at the top left)
local RAIL_FLOOR = 232                        -- above the chat (its top is 215 at the stock place)
local KEY_CHOICES = { build = { "F7", "CTRL-F7" }, play = { "F8", "CTRL-F8" } }
local KIND_COLOUR = { info = "text", ok = "ok", warn = "warn", jaina = "violet", cheer = "accent" }

-- TODO(owner): a voice pass on the intro cards, his words.
local CARDS = {
    { "This is your Den", "An empty white world that's yours. Everything you do here keeps itself: it saves as you go." },
    { "Build", "Build (F7, the box on the left) opens a gallery of props: double-click one to place it, then move, turn or delete it." },
    { "Shape ground", "Opens the world editor on your Den's ground: raise hills, dig, paint the ground. Your game shows it after a restart." },
    { "Play and invite", "Play (F8) hides the tools so you just walk your Den. Invite makes one link for a friend: send it, and they join you here." },
    { "Keys", "F7 gallery · F8 play · double-click a tile to place · arrows move, PgUp / PgDn lift, Q / E turn · Del deletes · Ctrl+Z / Ctrl+Y undo, redo · Ctrl+C / V / D copy, paste, duplicate." },
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

-- The line: the current toast over the status pill (Builder.lua B.PillNote), and in play mode on the small pill.
local function showLine()
    if not bar then return end
    local t = D.current
    local text = t and (hex(KIND_COLOUR[t.kind] or "text") .. (t.kind == "jaina" and "Jaina: " or "") .. t.text .. "|r") or (hex("muted") .. D.SavedLine() .. "|r")
    bar.line:SetText(text)
    if bar.pill then bar.pill.line:SetText(text) end
    local b = W.Builder
    if t and b and b.PillNote and t ~= D.noted then
        D.noted = t
        local st = S()
        local c = t.kind == "cheer" and st.ED.gold or (st.C and (t.kind == "warn" or t.kind == "jaina") and st.C[KIND_COLOUR[t.kind]]) or nil
        b.PillNote((t.kind == "jaina" and "Jaina: " or "") .. t.text, c)
    end
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
local function B() return W.Builder end
local function E() return W.EditMode end
local function editing() local e = E(); return e and e.IsOn and e.IsOn() end

local function keyText(which) local k = D.keys[which]; return k and (" (" .. k:gsub("CTRL%-", "Ctrl+") .. ")") or "" end

function D.Labels()
    if not bar then return end
    bar.build:SetText("Build" .. keyText("build"))
    bar.build.edLabel = "Build: the gallery of props" .. keyText("build")
    bar.playBtn:SetText("Play" .. keyText("play"))
    if bar.pill then bar.pill.back:SetText("Back to building" .. keyText("play")) end
end

-- The menu (the strip's Menu): what isn't a tool of its own, in groups. A row's third field: "den" (only in a Den),
-- "edit" (the browser's edit mode), "browser", "full" (only the full Den: Den Lite's state says lite=1).
local MENU = {
    { "#", "Selection" },
    { "Select nearest", function() local b = B(); if b then b.Command("near") end end },
    { "Select mine", function() local b = B(); if b then b.Command("selall") end end },
    { "Clear selection", function() local b = B(); if b then b.Command("selclear") end end },
    { "Box select (edit mode)", function() local e = E(); if e and editing() then e.StartBox() end end, "edit" },
    { "Properties", function() local b = B(); if b then b.ToggleProps() end end },
    { "#", "Edit" },
    { "Copy", function() local b = B(); if b then b.Command("copy") end end },
    { "Paste", function() local b = B(); if b then b.Paste() end end },
    { "Duplicate", function() local b = B(); if b then b.Command("dup") end end },
    { "Drop to the floor", function() local b = B(); if b then b.Command("floor") end end },
    { "Delete", function() local b = B(); if b then b.Command("delete") end end },
    { "#", "Groups and storage", "full" },
    { "Group", function() local b = B(); if b then b.Command("group") end end, "full" },
    { "Ungroup", function() local b = B(); if b then b.Command("ungroup") end end, "full" },
    { "Put away", function() local b = B(); if b then b.Command("store") end end, "full" },
    { "Storage", function() local b = B(); if b then b.ShowBuilder(true); b.ShowTab("storage") end end, "full" },
    { "#", "Your Den", "den" },
    { "Den panel", function() local e = E(); if e and e.ToggleDen then e.ToggleDen() end end, "den" },
    { "Den home", function() if W.DenHome and W.DenHome.Toggle then W.DenHome.Toggle() end end, "den" },
    { "Edit mode (F7)", function() local e = E(); if e and e.Toggle then e.Toggle() end end, "browser" },
    { "#", "Help" },
    { "Keys and tips", function() D.ShowIntro(#CARDS) end },
    { "The intro again", function() D.ShowIntro(1) end },
    { "Reload the props", function() local b = B(); if b then b.RequestCatalog() end end },
}
D.MENU = MENU
local MENU_W, MENU_ROW = 220, 28

local function closeMenus(except)
    if menu and menu ~= except then menu:Hide() end
    if snapMenu and snapMenu ~= except then snapMenu:Hide() end
end

local function buildMenu()
    menu = CreateFrame("Frame", "LeylineDenMenu", bar)
    menu:SetWidth(MENU_W)
    menu:SetPoint("TOPRIGHT", bar.menuBtn, "BOTTOMRIGHT", 0, -6)
    menu:SetFrameStrata("DIALOG")
    menu:EnableMouse(true)
    S().EdPanel(menu, 0.96, ED().panel2)
    -- the budget first: props and points (or the Den's room and yard)
    menu.meter = S().EdText(menu, S().ED_SIZE.text)
    menu.meter:SetPoint("TOPLEFT", 10, -10); menu.meter:SetWidth(MENU_W - 20); menu.meter:SetJustifyH("LEFT")
    menu.rows = {}
    for i, it in ipairs(MENU) do
        local r
        if it[1] == "#" then
            r = S().EdLabel(menu, it[2], true)
            r.header = true
        else
            r = chip(menu, it[1], MENU_W - 16, function() closeMenus(); it[2]() end)
            r:SetHeight(MENU_ROW - 2)
            if r.edText then r.edText:ClearAllPoints(); r.edText:SetPoint("LEFT", 10, 0); r.edText:SetJustifyH("LEFT") end
        end
        r.only = it[3]
        menu.rows[i] = r
    end
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
    local lite = s.lite == "1"
    local y = -34
    for _, r in ipairs(menu.rows) do
        local on = not r.only or (r.only == "den" and D.active) or (r.only == "edit" and editing()) or (r.only == "browser" and browser)
            or (r.only == "full" and not lite)
        if on then
            r:ClearAllPoints()
            if r.header then y = y - 6; r:SetPoint("TOPLEFT", menu, "TOPLEFT", 10, y - 6); y = y - 22
            else r:SetPoint("TOPLEFT", menu, "TOPLEFT", 8, y); y = y - MENU_ROW end
            r:Show()
        else
            r:Hide()
        end
    end
    menu:SetHeight(-y + 8)
    menu:Show()
    return true
end
function D.Menu() return menu end

local function snapText(g) return (g and g > 0) and ("Snap: " .. g .. " yd") or "Snap: off" end
local function buildSnapMenu()
    snapMenu = CreateFrame("Frame", "LeylineDenSnapMenu", bar)
    snapMenu:SetWidth(110)
    snapMenu:SetPoint("TOPLEFT", bar.snap, "BOTTOMLEFT", 0, -6)
    snapMenu:SetFrameStrata("DIALOG")
    snapMenu:EnableMouse(true)
    S().EdPanel(snapMenu, 0.96, ED().panel2)
    snapMenu.rows = {}
    local list = (B() and B().SNAPS) or { 0, 0.5, 1, 2, 5 }
    for i, g in ipairs(list) do
        local r = chip(snapMenu, g > 0 and (g .. " yd") or "off", 94, function()
            closeMenus()
            local b = B()
            if b then b.SetSnap(g) end
            D.Refresh()
        end, "TOPLEFT", 8, -8 - (i - 1) * (MENU_ROW + 2))
        r.grid = g
        snapMenu.rows[i] = r
    end
    snapMenu:SetHeight(#list * (MENU_ROW + 2) + 14)
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
            b.SetStatus("Selected the prop nearest you. Its numbers are in the popover on the right.")
            if b.ShowProps then b.ShowProps(true); b.propsAuto = true end
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

-- The rail's tools: { key, icon (Interface\Icons\...), label under it, tooltip, what it does }
D.RAIL = {
    { "select", "Ability_Hunter_SniperShot", "Select", "Select: the prop nearest you", function() D.Tool("select") end },
    { "build", "INV_Crate_01", "Build", "Build: the gallery of props", function() D.Tool("place") end },
    { "move", "Spell_Arcane_Blink", "Move", "Move the selection (arrows, PgUp / PgDn)", function() D.Tool("move") end },
    { "rotate", "Ability_Whirlwind", "Rotate", "Turn the selection (Q / E)", function() D.Tool("rotate") end },
    { "stamps", "INV_Misc_Map_01", "Stamps", "Stamps: ready-made areas", function() D.Tool("stamps") end },
    { "path", "Ability_Tracking", "Path", "Path: a line of the picked prop", function() D.Tool("path") end },
    { "ground", "Spell_Nature_Earthquake", "Ground", "Shape the ground (opens Forge)", function() D.ShapeGround() end, "Shape ground" },
    { "dojo", "Ability_DualWield", "Dojo", "The training grounds", function() if W.Dojo then W.Dojo.Toggle() end end },
}

-- A rail button: the icon and its name under it (a short screen: the icon alone, the name in the tooltip).
local function railButton(t)
    local b = chip(rail, "", RAIL_W - 8, t[5])
    b.edIcon = b:CreateTexture(nil, "ARTWORK")
    b.edIcon:SetWidth(RAIL_BTN); b.edIcon:SetHeight(RAIL_BTN)
    b.edIcon:SetTexCoord(0.08, 0.92, 0.08, 0.92)
    b.edIcon:SetTexture("Interface\\Icons\\" .. t[2])
    b.iconPath = "Interface\\Icons\\" .. t[2]
    b.SetIcon = function(self, path) self.iconPath = path; self.edIcon:SetTexture(path) end
    b.edText:ClearAllPoints()
    b.edText:SetPoint("BOTTOMLEFT", 2, 4); b.edText:SetPoint("BOTTOMRIGHT", -2, 4)
    b.SetText = function(self, txt) self.text = txt; self.edText:SetText(t[3]) end
    b.GetText = function(self) return self.text end
    b:SetText(t[6] or t[3])
    b.edLabel = t[4]
    b.edTipAt, b.edTipTo = "LEFT", "RIGHT"
    return b
end

-- The rail's buttons for the screen's height: labelled (54 high) when they fit between the player frame and the chat,
-- else icon-only (40). Called on every Refresh (a resize, a new UI scale).
function D.LayoutRail()
    if not rail then return end
    local h = (GetScreenHeight and GetScreenHeight()) or 768
    local n = #D.RAIL
    local room = h - RAIL_TOP - RAIL_FLOOR
    local tall = room >= n * 58 + 8
    local bh = tall and 54 or math.max(34, math.min(44, math.floor((room - 8) / n) - 4))
    local y = -4
    for _, t in ipairs(D.RAIL) do
        local b = bar[t[1]]
        b:SetHeight(bh)
        b:ClearAllPoints(); b:SetPoint("TOPLEFT", rail, "TOPLEFT", 4, y)
        b.edIcon:ClearAllPoints()
        if tall then b.edIcon:SetPoint("TOP", b, "TOP", 0, -4); b.edText:Show()
        else b.edIcon:SetPoint("CENTER", b, "CENTER", 0, 0); b.edText:Hide() end
        y = y - bh - 4
    end
    rail:SetHeight(-y + 4)
    rail.compact = not tall
end

local function buildBar()
    -- the strip: a floating toolbar across the top middle, clear of the minimap (top right) and the player frame
    bar = CreateFrame("Frame", "LeylineDenBar", UIParent)
    bar:SetHeight(BAR_H)
    bar:SetPoint("TOP", UIParent, "TOP", 0, -6)
    bar:SetFrameStrata("MEDIUM")
    bar:EnableMouse(true)
    S().EdPanel(bar, 0.88)
    bar.mark = bar:CreateTexture(nil, "ARTWORK")
    bar.mark:SetTexture(S().MARK)
    bar.mark:SetWidth(22); bar.mark:SetHeight(22)
    bar.mark:SetPoint("LEFT", 10, 0)
    bar.title = S().EdText(bar, S().ED_SIZE.title)
    bar.title:SetPoint("LEFT", 38, 0)
    bar.title:SetTextColor(ED().gold[1], ED().gold[2], ED().gold[3])
    bar.title:SetText("YOUR DEN")
    local x = 136
    bar.chips = {}
    local function add(key, text, w, fn, tip)
        local b = chip(bar, text, w, fn, "LEFT", x, 0)
        bar.chips[#bar.chips + 1] = { b, x }
        x = x + (tonumber((b:GetWidth())) or w or 60) + 6
        bar[key] = b
        b.edLabel = tip
        b.edTipAt, b.edTipTo, b.edTipX, b.edTipY = "TOP", "BOTTOM", 0, -6
        return b
    end
    local function gap() x = x + 10 end
    add("undo", "Undo 0", 76, function() local b = B(); if b then b.Command("undo") end end, "Undo (Ctrl+Z with a prop selected)")
    add("redo", "Redo 0", 76, function() local b = B(); if b then b.Command("redo") end end, "Redo (Ctrl+Y)")
    gap()
    add("snap", "Snap: off", 104, function() D.ShowSnap() end, "Snap what you place to a grid")
    bar.rot = {}
    for _, a in ipairs({ 15, 45, 90 }) do
        bar.rot[a] = add("rot" .. a, a .. "°", 44, function() local b = B(); if b then b.SetAngle(a) end; D.Refresh() end, "Q / E turn by " .. a .. " degrees")
    end
    gap()
    add("playBtn", "Play", 96, function() D.SetPlay(true) end, "Play: hide the tools and walk your Den")
    add("invite", "Invite", 70, function() D.Invite() end, "Invite one friend: a link to send")
    add("save", "Files", 62, function() local b = B(); if b then b.ShowFiles() end end, "Save and open your Den by name")
    add("menuBtn", "Menu", 66, function() D.ShowMenu() end, "Copy, paste, delete, groups, help...")
    add("help", "?", 28, function() D.ShowIntro(#CARDS) end, "Keys and tips")
    add("close", "x", 28, function() D.Close() end, "Close the tools (Play)")
    bar.x = x
    bar.full = x + 4
    bar:SetWidth(bar.full)
    -- the strip's old line (kept for callers): the toasts now show over the status pill
    bar.line = S().EdText(bar, S().ED_SIZE.text)
    bar.line:Hide()

    -- the rail: down the left under the player frame, an icon and a name each, the tool that is on in gold
    rail = CreateFrame("Frame", "LeylineDenRail", UIParent)
    rail:SetWidth(RAIL_W)
    rail:SetPoint("TOPLEFT", UIParent, "TOPLEFT", 8, -RAIL_TOP)
    rail:SetFrameStrata("MEDIUM")
    rail:EnableMouse(true)
    S().EdPanel(rail, 0.88)
    for _, t in ipairs(D.RAIL) do bar[t[1]] = railButton(t) end
    bar.steps = {}   -- the move step lives in the popover now (owner: the rail's "STEP 0.1 0.25 1 3" stack)
    D.LayoutRail()
    rail:Hide()
    bar.rail = rail

    -- play mode: one small chip and the line
    local pill = CreateFrame("Frame", "LeylineDenPlayPill", UIParent)
    pill:SetWidth(420); pill:SetHeight(36)
    pill:SetPoint("TOP", UIParent, "TOP", 0, -6)
    pill:EnableMouse(true)
    S().EdPanel(pill, 0.78)
    pill.back = chip(pill, "Back to building", 170, function() D.SetPlay(false) end, "LEFT", 4, 0)
    pill.line = S().EdText(pill, S().ED_SIZE.text)
    pill.line:SetPoint("LEFT", 182, 0)
    pill.line:SetWidth(228); pill.line:SetJustifyH("LEFT")
    pill:Hide()
    bar.pill = pill

    -- the drawer under the strip: a name for Save, or the invite link to copy (a small card, not a bar)
    drawer = CreateFrame("Frame", "LeylineDenDrawer", bar)
    drawer:SetWidth(470); drawer:SetHeight(64)
    drawer:SetPoint("TOP", bar, "BOTTOM", 0, -8)
    drawer:EnableMouse(true)
    S().EdPanel(drawer, 0.95, ED().panel2)
    drawer.label = S().EdLabel(drawer, "", true, "TOPLEFT", 12, -9)
    drawer.label:SetWidth(300)
    drawer.box = S().EdInput(drawer, 330, 26)
    drawer.box:SetPoint("TOPLEFT", 12, -28)
    drawer.box:SetScript("OnEscapePressed", function(self) self:ClearFocus(); D.CloseDrawer() end)
    drawer.box:SetScript("OnEnterPressed", function() if D.drawerKind == "save" then D.Save(drawer.box:GetText()) else D.CloseDrawer() end end)
    drawer.box:SetScript("OnEditFocusGained", function(self) if D.drawerKind == "invite" then self:HighlightText() end end)
    -- the link stays as it came (a typed key doesn't change it)
    drawer.box:SetScript("OnTextChanged", function(self)
        if D.drawerKind == "invite" and D.link and self:GetText() ~= D.link then self:SetText(D.link); self:HighlightText() end
    end)
    drawer.hint = S().EdText(drawer, S().ED_SIZE.small)
    drawer.hint:SetPoint("TOPRIGHT", -12, -10)
    drawer.hint:SetTextColor(ED().muted[1], ED().muted[2], ED().muted[3])
    drawer.ok = chip(drawer, "Save", 56, function() if D.drawerKind == "save" then D.Save(drawer.box:GetText()) else D.CloseDrawer() end end, "TOPLEFT", 350, -28)
    drawer.ok:SetActive(true)
    drawer.close = chip(drawer, "x", 28, function() D.CloseDrawer() end, "TOPLEFT", 412, -28)
    drawer:Hide()

    bar:Hide()
    D.Labels()
end
function D.Bar() if not bar then buildBar() end return bar end
function D.Rail() D.Bar(); return rail end
function D.Drawer() D.Bar(); return drawer end

-- The strip's place: the screen's middle, but never over the player frame (top left, 222 wide with its margin) or the
-- minimap (top right, 200); on a narrow screen without its title, so it fits between them.
function D.LayoutBar()
    if not bar then return end
    local sw = (GetScreenWidth and tonumber((GetScreenWidth()))) or 1366
    local room = sw - 222 - 200
    local compact = bar.full > room
    local shift = compact and 98 or 0
    for _, c in ipairs(bar.chips) do c[1]:ClearAllPoints(); c[1]:SetPoint("LEFT", bar, "LEFT", c[2] - shift, 0) end
    if compact then bar.title:Hide() else bar.title:Show() end
    local w = bar.full - shift
    bar:SetWidth(w)
    local left = math.max(222, math.floor((sw - w) / 2))
    bar:ClearAllPoints()
    bar:SetPoint("TOPLEFT", UIParent, "TOPLEFT", left, -6)
    bar.compact = compact
end

-- The strip and the rail say what is on now: the title, Undo n / Redo n, Snap, the turn step, the tool.
function D.Paint()
    if not bar then return end
    local b = B()
    local s = (b and b.state) or {}
    bar.title:SetText(D.active and "YOUR DEN" or "BUILD")
    bar.undo:SetText("Undo " .. (tonumber(s.undo or "") or 0))
    bar.redo:SetText("Redo " .. (tonumber(s.redo or "") or 0))
    if (tonumber(s.undo or "") or 0) > 0 then bar.undo:Enable() else bar.undo:Disable() end
    if (tonumber(s.redo or "") or 0) > 0 then bar.redo:Enable() else bar.redo:Disable() end
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
    end
    bar.dojo:SetActive(W.Dojo and W.Dojo.IsOpen and W.Dojo.IsOpen() or false)
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
        drawer.label:SetText(("Name this save"):upper())
        drawer.hint:SetText("Enter saves")
        drawer.ok:SetText("Save")
        drawer.box:SetMaxLetters(NAME_MAX)
        drawer.box:SetText(D.lastName or "my-den")
        drawer.box:SetFocus()
        drawer.box:HighlightText()
    else
        drawer.label:SetText(("Invite one friend"):upper())
        drawer.hint:SetText(D.link and "Ctrl+C copies it, then send it" or "")
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

-- Files > This Den > Open: a save put back in place of the Den (".den load <name> replace"; Undo brings it back).
function D.Open(name)
    local n = tostring(name or ""):gsub("^%s+", ""):gsub("%s+$", ""):gsub("%s+", "-")
    if n == "" or #n > NAME_MAX or not n:match("^[%w_%-]+$") then
        D.Toast("warn", "Type the name of a save: letters, numbers, - and _.", true)
        return false
    end
    denCmd("load " .. n .. " replace")
    return true
end

function D.Invite()
    D.link = nil
    denCmd("invite")
    D.OpenDrawer("invite")
    drawer.box:SetText("")
    drawer.label:SetText(("Making a link..."):upper())
    drawer.hint:SetText("")
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
    intro:SetWidth(460); intro:SetHeight(220)
    intro:SetPoint("CENTER", UIParent, "CENTER", 0, 80)
    intro:SetFrameStrata("DIALOG")
    intro:EnableMouse(true)
    if S() then S().Window(intro, "Your Den") end
    intro.head = S().EdText(intro, 17)
    intro.head:SetPoint("TOPLEFT", 18, -46)
    intro.head:SetJustifyH("LEFT")
    intro.head:SetTextColor(ED().gold[1], ED().gold[2], ED().gold[3])
    intro.body = S().EdText(intro, 14)
    intro.body:SetPoint("TOPLEFT", 18, -76)
    intro.body:SetWidth(424); intro.body:SetJustifyH("LEFT")
    intro.count = S().EdText(intro, S().ED_SIZE.text)
    intro.count:SetPoint("BOTTOMLEFT", 18, 18)
    intro.skip = chip(intro, "Skip", 70, function() D.EndIntro() end, "BOTTOMRIGHT", -150, 12)
    intro.next = chip(intro, "Next", 130, function() D.NextCard() end, "BOTTOMRIGHT", -12, 12)
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
    D.LayoutRail()
    D.LayoutBar()
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
    elseif name == "diag" then
        -- the den-test asks what the game makes of the character now (WrathUnleashedUI.lua W.DenDiag)
        if W.DenDiag then pcall(W.DenDiag, plain(data)) end
    elseif name == "ui" then
        D.TestAction(data)
    elseif name == "invite" and inviteOk(data) then
        D.link = data
        if D.active then
            D.OpenDrawer("invite")
            drawer.box:SetText(data)
            drawer.box:SetFocus()
            drawer.box:HighlightText()
        end
    end
end

-- ---------------------------------------------------------------- the chat: the Den's system lines go to the pill
-- Owner's shot, 3 Nov: "the chat full of system lines". In a Den every system line (Den Lite's answers, its welcome)
-- shows over the status pill instead, one at a time; what the player typed themselves (a dot command in the last few
-- seconds) still answers in the chat, as before.
D.TYPED_SECS = 4
function D.ChatFilter(_, _, text)
    if not (D.active or (W.InDen and W.InDen())) then return false end
    if D.typedAt and clock() - D.typedAt <= D.TYPED_SECS then return false end
    D.Toast("info", text)
    return true
end
if ChatFrame_AddMessageEventFilter then ChatFrame_AddMessageEventFilter("CHAT_MSG_SYSTEM", D.ChatFilter) end
if hooksecurefunc and SendChatMessage then
    hooksecurefunc("SendChatMessage", function(msg) if type(msg) == "string" and msg:match("^%s*%.") then D.typedAt = clock() end end)
end

-- ---------------------------------------------------------------- the den-test's feature check
-- tools/den-check `featuretest` asks Den Lite (`.den ui <action>`, only with the test's channel), which asks the
-- addon (`wuden ui <action>`): each action is what a click on the editor does, through the same buttons and the same
-- Builder calls, so the test proves the clicks. Only in a Den. `report` answers `wuden report <line>` lines: the
-- gallery's tiles (with a model, an icon on purpose, empty), the catalog, the selection, the windows' places and
-- what they overlap of the game's own frames.
local function frameRect(f)
    if not (f and f.IsShown and f:IsShown() and f.GetLeft) then return nil end
    local l, b, w, h = f:GetLeft(), f:GetBottom(), f:GetWidth(), f:GetHeight()
    if not (l and b and w and h) then return nil end
    local k = 1
    if f.GetEffectiveScale and UIParent and UIParent.GetEffectiveScale then
        local fe, ue = tonumber(f:GetEffectiveScale()), tonumber(UIParent:GetEffectiveScale())
        if fe and ue and ue > 0 then k = fe / ue end
    end
    return { x = l * k, y = b * k, w = w * k, h = h * k }
end
D.EDITOR_FRAMES = { "LeylineDenBar", "LeylineDenRail", "WrathUnleashedBuilderFrame", "LeylineEditorStatus", "WrathUnleashedPropsFrame",
    "WrathUnleashedFilesFrame", "LeylineDenMenu", "LeylineDenDrawer" }
D.GAME_FRAMES = { "MinimapCluster", "ChatFrame1", "MainMenuBar", "MultiBarBottomLeft", "MultiBarBottomRight", "MultiBarRight",
    "MultiBarLeft", "PlayerFrame" }
-- Which shown editor windows overlap which shown game frames (by more than 2 UI units): { "A/B", ... }
function D.Overlaps()
    local out = {}
    for _, en in ipairs(D.EDITOR_FRAMES) do
        local a = frameRect(_G[en])
        if a then
            for _, gn in ipairs(D.GAME_FRAMES) do
                local g = frameRect(_G[gn])
                if g then
                    local ox = math.min(a.x + a.w, g.x + g.w) - math.max(a.x, g.x)
                    local oy = math.min(a.y + a.h, g.y + g.h) - math.max(a.y, g.y)
                    if ox > 2 and oy > 2 then out[#out + 1] = en .. "/" .. gn end
                end
            end
        end
    end
    return out
end
D.FrameRect = frameRect

local function report(line) send("report", tostring(line):gsub("[%c|]", " "):sub(1, 200)) end
function D.Report(what)
    local b = B()
    local lines = {}
    if b and b.TileReport then
        local r = b.TileReport()
        lines[#lines + 1] = string.format("gallery shown=%d tiles=%d model=%d icon=%d empty=%d rows=%d cols=%d tile=%d", r.shown, r.tiles,
            r.model, r.icon, r.empty, r.rows, r.cols, r.tile)
        lines[#lines + 1] = string.format("catalog n=%d model=%d boxed=%d wmo=%d none=%d", r.catalog, r.withModel, r.boxed, r.wmo, r.none)
        if #r.missing > 0 then lines[#lines + 1] = "nomodel " .. table.concat(r.missing, ","):sub(1, 180) end
    end
    local s = (b and b.state) or {}
    lines[#lines + 1] = string.format("state sel=%s n=%s undo=%s redo=%s clip=%s grid=%s lite=%s mode=%s tab=%s pick=%s", s.selName or "-",
        tostring(s.count or 0), s.undo or "?", s.redo or "?", s.clip or "?", s.grid or "?", s.lite or "0", tostring(b and b.mode),
        tostring(b and b.tab), tostring(b and b.pick and b.pick.id or "-"))
    local open = {}
    for _, n in ipairs(D.EDITOR_FRAMES) do if _G[n] and _G[n].IsShown and _G[n]:IsShown() then open[#open + 1] = n end end
    for _, n in ipairs({ "LeylineDenIntro", "LeylineDenPlayPill", "WrathUnleashedDenFrame", "LeylineDojoFrame" }) do
        if _G[n] and _G[n].IsShown and _G[n]:IsShown() then open[#open + 1] = n end
    end
    lines[#lines + 1] = "open " .. table.concat(open, ",")
    local ov = D.Overlaps()
    lines[#lines + 1] = "overlaps " .. (#ov > 0 and table.concat(ov, ",") or "none")
    local h = (GetScreenHeight and GetScreenHeight()) or 0
    local w = (GetScreenWidth and GetScreenWidth()) or 0
    lines[#lines + 1] = string.format("screen %.0fx%.0f uiscale=%s play=%s rail=%s", w, h, tostring(GetCVar and GetCVar("uiScale")), tostring(D.play),
        rail and (rail.compact and "compact" or "labelled") or "-")
    local p = b and b.Pill and b.Pill()
    if p and p.line then lines[#lines + 1] = "pill " .. tostring(p.line:GetText() or "") end
    if what then lines[#lines + 1] = "mark " .. tostring(what) end
    for _, l in ipairs(lines) do report(l) end
    return lines
end

local function find(id)
    local b = B()
    id = tonumber(id)
    for _, c in ipairs((b and b.catalog) or {}) do if c.id == id then return c end end
end
-- one test action, as the matching click: true when it was one
function D.TestAction(text)
    if not D.active then return false end
    local w = {}
    for x in tostring(text or ""):gmatch("%S+") do w[#w + 1] = x end
    local a, b = w[1] or "", B()
    if not b then return false end
    closeMenus()
    if a == "report" then D.Report(w[2]); return true end
    if a == "gallery" then if not (b.Window() and b.Window():IsShown()) then bar.build:Click() end; if w[2] then b.ShowTab(w[2]) end; return true end
    if a == "nogallery" then if b.Window() then b.Window():Hide() end; return true end
    if a == "tab" then return b.ShowTab(w[2] or "all") end
    if a == "fit" then return b.SetFit(w[2], w[3], w[4]) end
    if a == "page" then return b.Page(tonumber(w[2]) or 1) end
    if a == "pick" or a == "place" then
        local c = find(w[2])
        if not c then report("missing " .. tostring(w[2])); return false end
        if not (b.Window() and b.Window():IsShown()) then bar.build:Click() end
        b.ShowTab("all")
        b.Choose(c)
        if a == "place" then b.Window().spawn:Click() end
        return true
    end
    if a == "select" then bar.select:Click(); return true end
    if a == "props" then b.ShowProps(w[2] ~= "off"); return true end
    if a == "nudge" then
        local p = b.Panel()
        if not (p and p:IsShown()) then b.ShowProps(true); p = b.Panel() end
        local f = (w[2] or "x"):lower()
        local btn = ((tonumber(w[3]) or 1) < 0) and p.minus[f] or p.plus[f]
        if btn then btn:Click(); return true end
        return false
    end
    if a == "key" then local fn = b.KEYS[(w[2] or ""):upper()]; if fn then fn(); return true end; return false end
    if a == "undo" then bar.undo:Click(); return true end
    if a == "redo" then bar.redo:Click(); return true end
    if a == "snap" then local g = tonumber(w[2]); if g then b.SetSnap(g) else D.ShowSnap(true) end; D.Refresh(); return true end
    if a == "turnstep" then local r = bar.rot[tonumber(w[2]) or 15]; if r then r:Click() end; return r ~= nil end
    if a == "menu" then
        if not w[2] then D.ShowMenu(true); return true end
        local want = table.concat(w, " ", 2):lower()
        D.ShowMenu(true)
        for i, it in ipairs(MENU) do
            if it[1] ~= "#" and it[1]:lower() == want and menu.rows[i]:IsShown() then menu.rows[i]:Click(); return true end
        end
        D.ShowMenu(false)
        report("missing menu " .. want)
        return false
    end
    if a == "files" then b.ShowFiles(w[2] ~= "off"); return true end
    if a == "save" or a == "open" then
        b.ShowFiles(true)
        local f = b.Files()
        f.den:SetText(w[2] or "")
        if a == "save" then f.denSave:Click() else f.denOpen:Click() end
        return true
    end
    if a == "keys" then bar.help:Click(); return true end
    if a == "intro" then D.ShowIntro(tonumber(w[2]) or 1); return true end
    if a == "play" then bar.playBtn:Click(); return true end
    if a == "build" then bar.pill.back:Click(); return true end
    if a == "close" then
        D.CloseDrawer(); D.EndIntro()
        for _, f in ipairs({ b.Window(), b.Panel(), b.Files and b.Files() }) do if f and f:IsShown() then f:Hide() end end
        return true
    end
    if a == "rail" then local r = bar[w[2] or ""]; if r and r.Click then r:Click(); return true end; return false end
    return false
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
