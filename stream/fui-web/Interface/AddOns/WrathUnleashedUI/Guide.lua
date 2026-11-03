-- Guide (3.3.5a API): the Den guide, a paged help window in the editor's look (Style.lua S.Ed*), with our own icons
-- (Media\Icons, tools/den-ui/icons/make_icons.py). Owner 3 Nov: "an in-game Den guide". Open it with Guide on the
-- Den bar (and in play mode's corner), /den guide, /denguide, or .den help in the chat (Den Lite answers `wuden
-- guide open`). Left: the pages; right: the page, in plain short sentences; Back and Next at the foot. Esc closes it.
-- Every verb here is one Den Lite answers (labs/den-lite README.md, "In the game"; world.rs, dojo.rs, sandbox.rs).

local W = WrathUnleashedUI
local G = { page = 1 }
local MOD = "wuden"
local WIN_W, WIN_H, LIST_W, PAD = 640, 400, 168, 14

-- { key, the page's name in the list, its icon, its heading, its lines }
G.PAGES = {
    { "start", "Your Den", "build", "Your Den", {
        "Your Den is your own place. You build it, then you play in it.",
        "It saves by itself as you go. Files keeps copies by name.",
        "F7 switches between Build mode and Play mode.",
        "Use Back and Next below, or pick a page on the left." } },
    { "modes", "Build and Play", "play", "Build mode and Play mode", {
        "Build mode shows the tools: the bar at the top, the tools down the left, the gallery.",
        "In Build mode your props glow under the mouse. Right-click one to pick it.",
        "Play mode hides the tools. You just walk your Den. Props work as they should: a chair is a seat.",
        "In Play mode the corner keeps Dojo, Class, Invite, Files and this guide.",
        "Press F7, or Play on the bar, to switch." } },
    { "gallery", "The gallery", "objects", "The gallery", {
        "Build opens the gallery: every prop you can place, by kind on the left. Search finds one by name.",
        "Each tile shows the prop from the side. Spin turns them all slowly.",
        "Hold the right mouse button on a tile and drag to turn it. The wheel over it zooms in and out.",
        "Shift and the wheel turns the page. A right-click on a tile makes it a favourite.",
        "Click a tile to pick it. Double-click, or Enter, to place it. You can also drag a tile out of the window." } },
    { "place", "Placing", "place", "Placing a prop", {
        "Pick a prop in the gallery. A copy of it stands in front of you.",
        "Move the mouse over the ground. The copy goes where you point.",
        "Click to put it there. Keep clicking to place more. Esc stops.",
        "G shows a circle on the ground for the exact spot. Click to place it there.",
        "Snap on the bar lines props up on a grid. 15, 45 and 90 set how far Q and E turn." } },
    { "select", "Selecting", "select", "Selecting and changing", {
        "Right-click a prop to select it. A menu opens: Delete, Duplicate, Move, Rotate, Bigger, Smaller, Snap to floor, Lock, Copy, Properties.",
        "Three flags stand around it: red for X, green for Y, blue for Z.",
        "Right-click a flag, then roll the wheel. The prop moves along that line.",
        "The selection panel shows its numbers. Type a number and press Enter. Its preview turns with a right-drag and zooms with the wheel.",
        "Undo and Redo are at the top. Lock keeps a prop from moving by accident." } },
    { "stamps", "Stamps and Jaina", "stamp", "Stamps and Jaina", {
        "Stamps are whole places at once: campfire, inn, market, garden, dock, house, smithy, nook, shrine.",
        "Say \"Jaina, build a garden\" or type .den jaina build garden. She walks over and builds it piece by piece.",
        "Type .den stamp garden now to put one down at once. .build undo takes it back.",
        "A stamp always goes inside your Den. A bigger room: .den grow.",
        ".den companion calls her. Say \"Jaina, stay\", \"Jaina, follow\" or \"Jaina, tips\"." } },
    { "dojo", "The Dojo", "dojo", "The Dojo", {
        "Dojo on the bar opens the training panel. .den dojo builds a ring with three training dummies.",
        "Practice: Pack, Mover, Healer, Tank, Boss (move out of its circles, interrupt its cast). Your numbers show as you hit.",
        "Range: Levels (dummies below and above you), Group (heal five), Range (40 yards), Run (your bar cast for you), Safe.",
        "Session: 1m, 3m or 5m for a timed parse. Reset fills everything again. .den dojo best shows your bests.",
        ".den fly on lets you fly: jump, then Space to rise and X to sink. .den speed 1 to 5 runs faster." } },
    { "class", "Class, mounts, looks", "class", "Class, mounts and looks", {
        ".den class mage fire 80: a Fire mage at level 80, its spells, talents and every bar at once. .den spec frost: another spec.",
        ".den bars save aoe keeps your bar; .den bars use aoe puts it back. The Dojo's Loadout page has all of it.",
        ".den gearset t7: your class's raid set. .den gearset save mine keeps what you wear. .den gearset off: your own outfit.",
        ".den mounts puts every mount in your spellbook. Cast one to ride. .den mounts off ends it.",
        ".den gear and an item's name puts its look on you. .den gear reset takes the looks off." } },
    { "invite", "Invite a friend", "invite", "Invite a friend", {
        "Press Invite on the Your Den card in the launcher. Send the link to one friend.",
        "Invite on the Den bar makes a link too.",
        "Stop sharing in the launcher takes the link back.",
        "The launcher's Invite guide has the rest." } },
    { "keys", "Keys", "keys", "Keys", {
        "F7: Build or Play.  Enter: place the picked prop.  Esc: let go.",
        "Arrows: move.  Page Up and Page Down: lift and lower.",
        "Q and E: turn.  + and -: size.  Tab: the next prop.",
        "Del: delete.  Ctrl+D: duplicate.  Ctrl+Z and Ctrl+Y: undo and redo.",
        "G: the exact spot.  Right-drag in a model window: turn.  Wheel there: zoom." } },
    { "files", "Saving", "files", "Saving and opening", {
        "Your Den saves by itself every few seconds.",
        "Files on the bar saves your Den by name, and opens one again.",
        "In the chat: .den save mine, then .den load mine.",
        ".den commands lists every command in the chat." } },
}

local win

local function S() return W.Style end

function G.Index(key)
    for i, p in ipairs(G.PAGES) do if p[1] == key then return i end end
    return nil
end

function G.Window() return win end
function G.IsOpen() return win ~= nil and win:IsShown() end

local function paint()
    if not win then return end
    local p = G.PAGES[G.page]
    win.heading:SetText(p[4])
    S().SetIcon(win.icon, p[3], S().ED.gold)
    win.body:SetText(table.concat(p[5], "\n\n"))
    win.count:SetText(G.page .. " of " .. #G.PAGES)
    for i, b in ipairs(win.list) do b:SetActive(i == G.page) end
    if G.page > 1 then win.back:Enable() else win.back:Disable() end
    if G.page < #G.PAGES then win.next:Enable() else win.next:Disable() end
end

local function build()
    win = CreateFrame("Frame", "LeylineDenGuide", UIParent)
    win:SetWidth(WIN_W); win:SetHeight(WIN_H)
    win:SetPoint("CENTER", UIParent, "CENTER", 0, 60)
    win:SetFrameStrata("DIALOG")
    win:EnableMouse(true)
    win:SetMovable(true)
    win:RegisterForDrag("LeftButton")
    win:SetScript("OnDragStart", win.StartMoving)
    win:SetScript("OnDragStop", win.StopMovingOrSizing)
    S().EdPanel(win, 0.95, S().ED.panel2)
    if UISpecialFrames then tinsert(UISpecialFrames, "LeylineDenGuide") end
    win.title = S().EdLabel(win, "Den guide", true, "TOPLEFT", PAD, -12)
    win.close = S().EdChip(win, "x", 28, function() G.Show(false) end, "TOPRIGHT", -8, -8)
    win.close.edLabel = "Close (Esc)"
    -- the pages, one row each
    win.list = {}
    for i, p in ipairs(G.PAGES) do
        local b = S().EdChip(win, p[2], LIST_W, function() G.Go(i) end, "TOPLEFT", PAD, -40 - (i - 1) * 30)
        b.edText:ClearAllPoints(); b.edText:SetPoint("LEFT", 10, 0); b.edText:SetPoint("RIGHT", -6, 0)
        b.edText:SetJustifyH("LEFT")
        win.list[i] = b
    end
    -- the page
    local x0 = PAD + LIST_W + PAD
    local bw = WIN_W - x0 - PAD
    win.icon = win:CreateTexture(nil, "ARTWORK")
    win.icon:SetWidth(28); win.icon:SetHeight(28)
    win.icon:SetPoint("TOPLEFT", x0, -40)
    win.heading = S().EdText(win, S().ED_SIZE.title)
    win.heading:SetPoint("TOPLEFT", x0 + 36, -46)
    win.heading:SetWidth(bw - 36); win.heading:SetHeight(18); win.heading:SetJustifyH("LEFT")
    local g = S().ED.gold
    win.heading:SetTextColor(g[1], g[2], g[3])
    win.body = S().EdText(win, S().ED_SIZE.text)
    win.body:SetPoint("TOPLEFT", x0, -80)
    win.body:SetWidth(bw); win.body:SetHeight(WIN_H - 80 - 52)
    win.body:SetJustifyH("LEFT"); if win.body.SetJustifyV then win.body:SetJustifyV("TOP") end
    -- the foot
    win.next = S().EdChip(win, "Next", 70, function() G.Go(G.page + 1) end, "BOTTOMRIGHT", -PAD, 12)
    win.back = S().EdChip(win, "Back", 70, function() G.Go(G.page - 1) end, "BOTTOMRIGHT", -PAD - 76, 12)
    win.count = S().EdText(win, S().ED_SIZE.small)
    win.count:SetPoint("BOTTOMLEFT", x0, 19)
    win.count:SetWidth(120); win.count:SetJustifyH("LEFT")
    local m = S().ED.muted
    win.count:SetTextColor(m[1], m[2], m[3])
    win:SetScript("OnShow", function() paint(); if W.DenBar and W.DenBar.Paint then W.DenBar.Paint() end end)
    win:SetScript("OnHide", function() if W.DenBar and W.DenBar.Paint then W.DenBar.Paint() end end)
    win:Hide()
end

-- Page n (1 .. #G.PAGES, or a page's key), shown.
function G.Go(n)
    if type(n) == "string" then n = G.Index(n) or 1 end
    G.page = math.max(1, math.min(#G.PAGES, tonumber(n) or 1))
    if not win then build() end
    paint()
    return G.page
end

-- Open (on true; at `page` when given) or close it.
function G.Show(on, page)
    if not win then build() end
    if page then G.Go(page) end
    if on then win:Show(); paint() else win:Hide() end
    return win:IsShown()
end
function G.Toggle() return G.Show(not G.IsOpen()) end

-- What the guide shows now (the den-test's report): open, the page, how many.
function G.Report()
    return string.format("guide open=%d page=%d pages=%d key=%s", G.IsOpen() and 1 or 0, G.page, #G.PAGES, G.PAGES[G.page][1])
end

-- Den Lite's `.den help` opens it (wuden guide open; a page key opens that page).
function G.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= MOD or name ~= "guide" then return end
    if data == "close" then G.Show(false) else G.Show(true, G.Index(data or "") or nil) end
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:SetScript("OnEvent", function(_, _, prefix, msg, channel, sender) G.OnMessage(prefix, msg, channel, sender) end)

SLASH_LEYLINEDENGUIDE1 = "/denguide"
SlashCmdList["LEYLINEDENGUIDE"] = function(msg)
    msg = tostring(msg or ""):lower():gsub("^%s+", ""):gsub("%s+$", "")
    G.Show(true, msg ~= "" and (G.Index(msg) or tonumber(msg)) or nil)
end

W.DenGuide = G
