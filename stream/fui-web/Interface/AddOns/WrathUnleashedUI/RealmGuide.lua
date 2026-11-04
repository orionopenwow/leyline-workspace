-- The realm guide (owner 4 Nov: "a guide to explain them all ... a new minimap circle icon with border with the
-- Leyline icon that opens our rounded addon window with the guide, commands, and more"). The pages are
-- RealmGuideData.lua (generated from docs/realm-guide/wrath-unleashed.json, the same text as the site's
-- /wrath/docs/wrath-unleashed). The window is the Den guide's look (Style.lua S.Ed*, the rounded panel): pages on the
-- left, the page on the right, then two of ours: Commands (every page's commands in one list) and More (buttons to
-- the other windows). Opened by the Leyline button on the minimap (drag it round the edge), /guide and /wuguide.
-- The minimap button shows on a Leyline realm (W.WhenLeyline); /wuguide button off hides it, /wuguide button on.

local W = WrathUnleashedUI
if not W then return end
local R = { page = 1 }
local WIN_W, WIN_H, LIST_W, PAD = 680, 440, 176, 14
local GOLD = "|cffe0a43a"
local MUTED = "|cff999ea8"
local EDGE_GOLD = { 0.878, 0.643, 0.227, 0.55 }   -- the rounded edge: the accent, soft

local function S() return W.Style end
local function db()
    WrathUnleashedUIDB = WrathUnleashedUIDB or {}
    return WrathUnleashedUIDB
end

-- The buttons on More: { label, what it opens (a function name on W, or a slash command's handler) }
R.MORE = {
    { "Collections", function() if W.ToggleCollections then W.ToggleCollections() end end },
    { "Leyline realms", function() if W.ToggleLeyline then W.ToggleLeyline() end end },
    { "Mods and Workshop", function() if W.ToggleMods then W.ToggleMods() end end },
    { "Minigames", function() local f = SlashCmdList and SlashCmdList["LEYLINEMODES"]; if f then f("") end end },
    { "Report a bug", function()
        if ChatFrame_OpenChat then ChatFrame_OpenChat(".bug ") end
    end },
}

-- every page's pages plus Commands and More
function R.Pages()
    local list = {}
    for _, p in ipairs(W.REALM_GUIDE or {}) do list[#list + 1] = p end
    list[#list + 1] = { key = "commands", name = "All commands", icon = "keys", heading = "All commands", all = true }
    list[#list + 1] = { key = "more", name = "More", icon = "menu", heading = "More", more = true }
    return list
end

function R.Index(key)
    for i, p in ipairs(R.Pages()) do if p.key == key then return i end end
    return nil
end

local function cmdLine(c) return GOLD .. c[1] .. "|r   " .. MUTED .. c[2] .. "|r" end

-- The body text of a page.
function R.Text(p)
    if p.all then
        local out = {}
        for _, q in ipairs(W.REALM_GUIDE or {}) do
            for _, c in ipairs(q.commands or {}) do out[#out + 1] = cmdLine(c) end
        end
        return table.concat(out, "\n")
    end
    if p.more then return "Open the other windows from here." end
    local t = table.concat(p.lines or {}, "\n\n")
    if p.commands and #p.commands > 0 then
        local c = {}
        for _, x in ipairs(p.commands) do c[#c + 1] = cmdLine(x) end
        t = t .. "\n\n" .. MUTED .. "COMMANDS|r\n" .. table.concat(c, "\n")
    end
    return t
end

local win

function R.Window() return win end
function R.IsOpen() return win ~= nil and win:IsShown() end

local function paint()
    if not win then return end
    local pages = R.Pages()
    local p = pages[R.page]
    win.heading:SetText(p.heading)
    S().SetIcon(win.icon, p.icon, S().ED.gold)
    win.body:SetText(R.Text(p))
    for i, b in ipairs(win.list) do b:SetActive(i == R.page) end
    for _, b in ipairs(win.more) do if p.more then b:Show() else b:Hide() end end
    if R.page > 1 then win.back:Enable() else win.back:Disable() end
    if R.page < #pages then win.next:Enable() else win.next:Disable() end
end

local function build()
    local pages = R.Pages()
    win = CreateFrame("Frame", "LeylineRealmGuide", UIParent)
    local h = math.max(WIN_H, 52 + #pages * 30 + 40)
    win:SetWidth(WIN_W); win:SetHeight(h)
    win:SetPoint("CENTER", UIParent, "CENTER", 0, 40)
    win:SetFrameStrata("DIALOG")
    win:EnableMouse(true)
    win:SetMovable(true)
    win:RegisterForDrag("LeftButton")
    win:SetScript("OnDragStart", win.StartMoving)
    win:SetScript("OnDragStop", win.StopMovingOrSizing)
    -- rounded corners in every client (owner 4 Nov 14:18), a soft gold edge
    if S().RoundPanel then S().RoundPanel(win, S().ED.panel2, EDGE_GOLD, 14, 1) else S().EdPanel(win, 0.95, S().ED.panel2) end
    if UISpecialFrames then tinsert(UISpecialFrames, "LeylineRealmGuide") end
    win.mark = win:CreateTexture(nil, "ARTWORK")
    win.mark:SetTexture(S().MARK)
    win.mark:SetWidth(18); win.mark:SetHeight(18)
    win.mark:SetPoint("TOPLEFT", PAD, -10)
    win.title = S().EdLabel(win, W.REALM_GUIDE_TITLE or "Guide", true, "TOPLEFT", PAD + 24, -12)
    win.close = S().EdChip(win, "x", 28, function() R.Show(false) end, "TOPRIGHT", -8, -8)
    win.close.edLabel = "Close (Esc)"
    win.list = {}
    for i, p in ipairs(pages) do
        local b = S().EdChip(win, p.name, LIST_W, function() R.Go(i) end, "TOPLEFT", PAD, -40 - (i - 1) * 30)
        b.edText:ClearAllPoints(); b.edText:SetPoint("LEFT", 10, 0); b.edText:SetPoint("RIGHT", -6, 0)
        b.edText:SetJustifyH("LEFT")
        win.list[i] = b
    end
    local x0 = PAD + LIST_W + PAD
    local bw = WIN_W - x0 - PAD
    -- a hairline under the title and one between the list and the page
    local function line(c) local t = win:CreateTexture(nil, "ARTWORK"); t:SetTexture(S().WHITE); t:SetVertexColor(c[1], c[2], c[3], c[4]); return t end
    win.rule = line({ 0.878, 0.643, 0.227, 0.30 })
    win.rule:SetPoint("TOPLEFT", win, "TOPLEFT", PAD, -34); win.rule:SetPoint("TOPRIGHT", win, "TOPRIGHT", -PAD, -34); win.rule:SetHeight(1)
    win.split = line({ 1, 1, 1, 0.08 })
    win.split:SetPoint("TOPLEFT", win, "TOPLEFT", PAD + LIST_W + PAD / 2, -44); win.split:SetPoint("BOTTOMLEFT", win, "BOTTOMLEFT", PAD + LIST_W + PAD / 2, 14)
    win.split:SetWidth(1)
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
    win.body:SetWidth(bw); win.body:SetHeight(h - 80 - 52)
    win.body:SetJustifyH("LEFT"); if win.body.SetJustifyV then win.body:SetJustifyV("TOP") end
    win.more = {}
    for i, m in ipairs(R.MORE) do
        local b = S().EdChip(win, m[1], 200, function() m[2]() end, "TOPLEFT", x0, -110 - (i - 1) * 34)
        b:Hide()
        win.more[i] = b
    end
    win.next = S().EdChip(win, "Next", 70, function() R.Go(R.page + 1) end, "BOTTOMRIGHT", -PAD, 12)
    win.back = S().EdChip(win, "Back", 70, function() R.Go(R.page - 1) end, "BOTTOMRIGHT", -PAD - 76, 12)
    win:SetScript("OnShow", paint)
    win:Hide()
end

function R.Go(n)
    local pages = R.Pages()
    if type(n) == "string" then n = R.Index(n) or 1 end
    R.page = math.max(1, math.min(#pages, tonumber(n) or 1))
    if not win then build() end
    paint()
    return R.page
end

function R.Show(on, page)
    if not win then build() end
    if page then R.Go(page) end
    if on then win:Show(); paint() else win:Hide() end
    return win:IsShown()
end
function R.Toggle() return R.Show(not R.IsOpen()) end
W.ToggleRealmGuide = R.Toggle

-- ---------------------------------------------------------------- the minimap button

function R.PlaceButton(b)
    local angle = math.rad(tonumber(db().guideAngle) or 225)
    b:ClearAllPoints()
    b:SetPoint("CENTER", Minimap, "CENTER", math.cos(angle) * 80, math.sin(angle) * 80)
end

function R.ShowButton()
    if not Minimap or db().guideNoMinimap then return end
    local b = R.mini
    if not b then
        b = CreateFrame("Button", "LeylineGuideMinimapButton", Minimap)
        b:SetWidth(31); b:SetHeight(31)
        b:SetFrameStrata("MEDIUM"); b:SetFrameLevel(8)
        b:SetHighlightTexture("Interface\\Minimap\\UI-Minimap-ZoomButton-Highlight")
        local bg = b:CreateTexture(nil, "BACKGROUND")
        bg:SetTexture("Interface\\Minimap\\UI-Minimap-Background")
        bg:SetWidth(20); bg:SetHeight(20)
        bg:SetPoint("TOPLEFT", 7, -5)
        local icon = b:CreateTexture(nil, "ARTWORK")
        icon:SetTexture(S().MARK)
        icon:SetWidth(18); icon:SetHeight(18)
        icon:SetPoint("TOPLEFT", 8, -6)
        b.icon = icon
        local border = b:CreateTexture(nil, "OVERLAY")
        border:SetTexture("Interface\\Minimap\\MiniMap-TrackingBorder")
        border:SetWidth(53); border:SetHeight(53)
        border:SetPoint("TOPLEFT")
        b:RegisterForDrag("LeftButton")
        b:SetScript("OnClick", function() R.Toggle() end)
        b:SetScript("OnDragStart", function(self)
            self:SetScript("OnUpdate", function()
                if not (GetCursorPosition and Minimap.GetCenter) then return end
                local mx, my = Minimap:GetCenter()
                local cx, cy = GetCursorPosition()
                local s = Minimap:GetEffectiveScale()
                if not (mx and cx and s and s > 0) then return end
                db().guideAngle = math.floor(math.deg(math.atan2(cy / s - my, cx / s - mx)) + 0.5)
                R.PlaceButton(self)
            end)
        end)
        b:SetScript("OnDragStop", function(self) self:SetScript("OnUpdate", nil) end)
        b:SetScript("OnEnter", function(self)
            if not GameTooltip then return end
            GameTooltip:SetOwner(self, "ANCHOR_LEFT")
            GameTooltip:SetText("Leyline")
            GameTooltip:AddLine("The realm guide, commands and more. /guide", 1, 1, 1)
            GameTooltip:Show()
        end)
        b:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
        R.mini = b
    end
    R.PlaceButton(b)
    b:Show()
end

local ev = CreateFrame("Frame")
ev:RegisterEvent("PLAYER_ENTERING_WORLD")
ev:SetScript("OnEvent", function(self)
    self:UnregisterEvent("PLAYER_ENTERING_WORLD")
    if W.WhenLeyline then W.WhenLeyline(R.ShowButton) else R.ShowButton() end
end)

SLASH_WUREALMGUIDE1, SLASH_WUREALMGUIDE2 = "/wuguide", "/guide"
SlashCmdList["WUREALMGUIDE"] = function(msg)
    msg = tostring(msg or ""):lower():gsub("^%s+", ""):gsub("%s+$", "")
    if msg == "button off" then
        db().guideNoMinimap = true
        if R.mini then R.mini:Hide() end
        print("|cff66ccffLeyline|r: the minimap button is hidden. /wuguide button on brings it back.")
        return
    elseif msg == "button on" then
        db().guideNoMinimap = nil
        R.ShowButton()
        return
    end
    R.Show(true, msg ~= "" and (R.Index(msg) or tonumber(msg)) or nil)
end

W.RealmGuide = R
