-- Teleports (3.3.5a API): go anywhere from Your Den, in one window (owner 4 Nov: "teleport anywhere from the Den,
-- fast ... a Teleports window with categories, search, favourites, recent, 'Back to my Den' always one click/key").
-- In the Leyline look (Style.lua: S.Window, S.Button, S.Input).
--
--   the top     a search box (any part of a name: "storm", "icc", "arena"), Back to my Den (always there), Back (the
--               spot before your last teleport; its name from the server)
--   the left    Favourites, Recent, then the list's categories (Cities, Towns, Starting areas, Zones, Dungeons, Raids,
--               Battlegrounds, Arenas, Sights, Your Den) and My Dens (the Dens you keep at other places)
--   the list    a place per row: click it to go; the star keeps it in Favourites. The mouse wheel scrolls.
--   keys        Back to my Den: Shift-Home unless you set your own (Esc > Key Bindings > Leyline travel); the window
--               has a key there too. /tp or /teleports opens it.
--
-- The places are TeleportsData.lua (names, titles, categories; written by tools/den-places/gen_teleports.py from the
-- same list Den Lite has). The window knows no coordinates: each place is one Den command over the realm's addon
-- channel (EditMode.lua DenCmd, "wuden cmd"): "go <name>", "home", "back", "places status". Den Lite (labs/den-lite
-- places.rs, world.rs) answers with the trip and, for the window, one line:
--   server -> client   places   den=<your Den's place>;map=<the map you are on>;back=<where Back goes|->;kept=<a,b>
-- Nothing the server sends is run; its texts are shown plain. Favourites and Recent are kept in WrathUnleashedUIDB.

local W = WrathUnleashedUI
local T = { tab = "city", query = "", offset = 0, line = {} }
local MOD = "wuden"
T.ROWS = 12
T.RECENT = 12
T.FAVS = 40
T.HOME_KEY = "SHIFT-HOME"

local frame, keyOwner

local function S() return W.Style end
local function plain(s) return (tostring(s or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("|", ""):gsub("%c", " ")) end
local function norm(s) return (tostring(s or ""):lower():gsub("[^%w]", "")) end

local function db()
    if type(WrathUnleashedUIDB) ~= "table" then WrathUnleashedUIDB = {} end
    local d = WrathUnleashedUIDB
    if type(d.teleFav) ~= "table" then d.teleFav = {} end
    if type(d.teleRecent) ~= "table" then d.teleRecent = {} end
    return d
end

-- ---------------------------------------------------------------- the places

local byName
function T.Places() return (W.TeleportsData and W.TeleportsData.places) or {} end
function T.Cats() return (W.TeleportsData and W.TeleportsData.cats) or {} end
-- a place by its name: { name, title, cat, words, map }
function T.Place(name)
    if not byName then
        byName = {}
        for _, p in ipairs(T.Places()) do byName[p[1]] = p end
    end
    return byName[name]
end

-- How well a search names a place (as places.rs `score`): 4 its name, a word or its title; 3 the start of its name;
-- 2 the start of a word or its title; 1 inside its title (4 letters or more); 0 not at all.
function T.Score(p, q)
    if q == "" then return 0 end
    local names = { norm(p[1]), norm(p[2]) }
    for w in tostring(p[4] or ""):gmatch("%S+") do names[#names + 1] = norm(w) end
    for _, n in ipairs(names) do if n == q then return 4 end end
    if #q >= 2 and names[1]:sub(1, #q) == q then return 3 end
    if #q >= 2 then for _, n in ipairs(names) do if n:sub(1, #q) == q then return 2 end end end
    if #q >= 4 then for _, n in ipairs(names) do if n:find(q, 1, true) then return 1 end end end
    return 0
end

-- What the list shows: a search (best first), else the tab: "fav", "recent", "kept" or a category.
-- Each entry: { name = <place or kept Den>, title = ..., kept = true for one of your Dens }.
function T.Entries()
    local out = {}
    local q = norm(T.query)
    if q ~= "" then
        local hits = {}
        for i, p in ipairs(T.Places()) do
            local s = T.Score(p, q)
            if s > 0 then hits[#hits + 1] = { s, i, p } end
        end
        table.sort(hits, function(a, b) if a[1] ~= b[1] then return a[1] > b[1] end return a[2] < b[2] end)
        for _, h in ipairs(hits) do out[#out + 1] = { name = h[3][1], title = h[3][2] } end
        return out
    end
    if T.tab == "fav" or T.tab == "recent" then
        for _, n in ipairs(T.tab == "fav" and db().teleFav or db().teleRecent) do
            local p = T.Place(n)
            if p then out[#out + 1] = { name = p[1], title = p[2] } end
        end
        return out
    end
    if T.tab == "kept" then
        for _, k in ipairs(T.Kept()) do out[#out + 1] = { name = k, title = k, kept = true } end
        return out
    end
    for _, p in ipairs(T.Places()) do
        if p[3] == T.tab then out[#out + 1] = { name = p[1], title = p[2] } end
    end
    return out
end

function T.Kept()
    local out = {}
    for k in tostring(T.line.kept or ""):gmatch("[^,]+") do
        k = k:gsub("[^%w%s'%-]", "")
        if k ~= "" and #out < 12 then out[#out + 1] = k:sub(1, 40) end
    end
    return out
end

-- ---------------------------------------------------------------- favourites, recent

function T.IsFav(name)
    for _, n in ipairs(db().teleFav) do if n == name then return true end end
    return false
end
function T.ToggleFav(name)
    if not T.Place(name) then return false end
    local f = db().teleFav
    for i, n in ipairs(f) do
        if n == name then table.remove(f, i); T.Paint(); return false end
    end
    if #f < T.FAVS then f[#f + 1] = name end
    T.Paint()
    return true
end
local function remember(name)
    local r = db().teleRecent
    for i = #r, 1, -1 do if r[i] == name then table.remove(r, i) end end
    table.insert(r, 1, name)
    while #r > T.RECENT do table.remove(r) end
end

-- ---------------------------------------------------------------- the commands

local function cmd(text)
    local E = W.EditMode
    if E and E.DenCmd then return E.DenCmd(text) end
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, "OVMOD", MOD .. "\tcmd\t1/1\t" .. (text:gsub("[^%w%s%-_]", ""):sub(1, 60)), "WHISPER", UnitName("player"))
    return true
end
T.Cmd = cmd

-- Go to a place of the list (a name from it only), or one of your Dens kept at a place (by its name).
function T.Go(name, kept)
    if kept then
        local k = tostring(name or ""):lower():gsub("[^%w%s%-]", "")
        if k == "" then return false end
        return cmd("go " .. k)
    end
    if not T.Place(name) then return false end
    remember(name)
    cmd("go " .. name)
    T.Paint()
    return true
end
function T.Home() return cmd("home") end
function T.Back() return cmd("back") end

-- ---------------------------------------------------------------- the window

local function build()
    local s = S()
    frame = CreateFrame("Frame", "LeylineTeleports", UIParent)
    frame:SetWidth(470); frame:SetHeight(410)
    frame:SetPoint("CENTER", UIParent, "CENTER", 0, 20)
    frame:SetFrameStrata("DIALOG")
    frame:EnableMouse(true)
    frame:SetMovable(true)
    frame:RegisterForDrag("LeftButton")
    frame:SetScript("OnDragStart", function(self) self:StartMoving() end)
    frame:SetScript("OnDragStop", function(self) self:StopMovingOrSizing() end)
    s.Window(frame, "Teleports")
    s.Close(frame)
    if UISpecialFrames then tinsert(UISpecialFrames, "LeylineTeleports") end

    frame.search = s.Input(frame, 200, 22)
    frame.search:SetPoint("TOPLEFT", 12, -40)
    frame.search.edLabel = "Search: any part of a name (storm, icc, arena)"
    frame.search:SetScript("OnTextChanged", function(self)
        T.query = tostring(self:GetText() or ""):sub(1, 40)
        T.offset = 0
        T.Paint()
    end)
    frame.search:SetScript("OnEnterPressed", function(self)
        local first = T.Entries()[1]
        self:ClearFocus()
        if first then T.Go(first.name, first.kept) end
    end)
    frame.home = s.Button(frame, "Back to my Den", 116, function() T.Home() end, "primary", "TOPLEFT", 218, -40)
    frame.back = s.Button(frame, "Back", 116, function() T.Back() end, "ghost", "TOPLEFT", 340, -40)

    -- the tabs
    frame.tabs = {}
    local tabs = { { "fav", "Favourites" }, { "recent", "Recent" } }
    for _, c in ipairs(T.Cats()) do tabs[#tabs + 1] = { c[1], c[2] } end
    tabs[#tabs + 1] = { "kept", "My Dens" }
    local y = -72
    for _, t in ipairs(tabs) do
        local b = s.Button(frame, t[2], 110, function() T.Tab(t[1]) end, "ghost", "TOPLEFT", 12, y)
        b.tab = t[1]
        frame.tabs[t[1]] = b
        y = y - 24
    end

    -- the rows: the place (click to go) and its star
    frame.list = CreateFrame("Frame", nil, frame)
    frame.list:SetPoint("TOPLEFT", 130, -72)
    frame.list:SetWidth(328); frame.list:SetHeight(T.ROWS * 24)
    frame.list:EnableMouseWheel(true)
    frame.list:SetScript("OnMouseWheel", function(_, delta) T.Scroll(delta > 0 and -3 or 3) end)
    frame.rows = {}
    for i = 1, T.ROWS do
        local r = s.Button(frame.list, "", 296, nil, "ghost", "TOPLEFT", 0, -(i - 1) * 24)
        r:SetScript("OnClick", function(self) if self.entry then T.Go(self.entry.name, self.entry.kept) end end)
        r.star = s.Button(frame.list, "*", 28, nil, "ghost", "TOPLEFT", 300, -(i - 1) * 24)
        r.star:SetScript("OnClick", function() if r.entry and not r.entry.kept then T.ToggleFav(r.entry.name) end end)
        frame.rows[i] = r
    end
    frame.count = frame:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    frame.count:SetPoint("TOPLEFT", 130, -72 - T.ROWS * 24 - 4)
    frame.count:SetWidth(328); frame.count:SetJustifyH("LEFT")
    frame.hint = frame:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    frame.hint:SetPoint("BOTTOMLEFT", 12, 10)
    frame.hint:SetWidth(446); frame.hint:SetJustifyH("LEFT")
    s.Muted(frame.hint)
    frame:Hide()
end

function T.Frame() if not frame then build() end return frame end
function T.IsOpen() return frame ~= nil and frame:IsShown() end

function T.Tab(which)
    T.tab = which
    T.offset = 0
    if frame and frame.search and (frame.search:GetText() or "") ~= "" then frame.search:SetText("") end
    T.query = ""
    T.Paint()
    return T.tab
end

function T.Scroll(by)
    local n = #T.Entries()
    T.offset = math.max(0, math.min(T.offset + by, math.max(0, n - T.ROWS)))
    T.Paint()
    return T.offset
end

function T.Paint()
    if not frame then return end
    for k, b in pairs(frame.tabs) do S().Tab(b, T.query == "" and k == T.tab) end
    local list = T.Entries()
    for i, r in ipairs(frame.rows) do
        local e = list[T.offset + i]
        r.entry = e
        if e then
            r:SetText(plain(e.title))
            r:Show(); r:Enable()
            if e.kept then r.star:Hide() else
                r.star:Show()
                r.star:SetText(T.IsFav(e.name) and "|cfff2c14e*|r" or "*")
                S().Tab(r.star, T.IsFav(e.name))
            end
        else
            r:SetText(""); r:Hide(); r.star:Hide()
        end
    end
    local empty = T.query ~= "" and "Nothing has that name." or (T.tab == "fav" and "No favourites yet: the star by a place keeps it here.")
        or (T.tab == "recent" and "Where you go shows here.") or (T.tab == "kept" and "Dens you keep at other places show here (.den here makes one).") or ""
    if #list == 0 then frame.count:SetText(empty)
    elseif #list > T.ROWS then frame.count:SetText(string.format("%d to %d of %d (scroll for more)", T.offset + 1, math.min(#list, T.offset + T.ROWS), #list))
    else frame.count:SetText("") end
    local back = T.line.back
    if back and back ~= "" and back ~= "-" then
        frame.back:SetText("Back"); frame.back:Enable()
        frame.back.edLabel = "Back to " .. back
    else
        frame.back:SetText("Back"); frame.back:Disable()
        frame.back.edLabel = "Where you were before a teleport"
    end
    local key = (GetBindingKey and GetBindingKey("LEYLINE_DENHOME")) or T.HOME_KEY
    frame.hint:SetText(string.format("%s: back to your Den from anywhere.  Type .go <place>, .back, .den home.  Your Den: %s",
        tostring(key), plain(T.line.den or "the island")))
end

function T.Show(on)
    T.Frame()
    if on then
        frame:Show()
        cmd("places status")
    else
        frame:Hide()
    end
    T.Paint()
    return T.IsOpen()
end
function T.Toggle() return T.Show(not T.IsOpen()) end

-- ---------------------------------------------------------------- Back to my Den: one key, always

-- In Your Den's realm, Shift-Home goes home unless the player set a key of their own for it (Esc > Key Bindings >
-- Leyline travel); elsewhere the key is given back.
function T.Keys(inDen)
    if not SetOverrideBinding or not ClearOverrideBindings then return false end
    keyOwner = keyOwner or CreateFrame("Frame", nil, UIParent)
    ClearOverrideBindings(keyOwner)
    local own = GetBindingKey and GetBindingKey("LEYLINE_DENHOME")
    if inDen and not own then
        SetOverrideBinding(keyOwner, false, T.HOME_KEY, "LEYLINE_DENHOME")
        return true
    end
    return false
end

-- ---------------------------------------------------------------- the server's line

function T.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= MOD or name ~= "places" then return end
    local l = {}
    for k, v in data:gmatch("(%a+)=([^;]*)") do l[k] = plain(v):sub(1, 200) end
    if not l.map or not tonumber(l.map) then return end
    T.line = l
    T.Paint()
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:RegisterEvent("PLAYER_ENTERING_WORLD")
listener:SetScript("OnEvent", function(_, event, prefix, msg, channel, sender)
    if event == "CHAT_MSG_ADDON" then return T.OnMessage(prefix, msg, channel, sender) end
    if event == "PLAYER_ENTERING_WORLD" then
        local inDen = W.InDen and W.InDen() or false
        T.Keys(inDen)
        if not inDen and frame then frame:Hide() end
        if inDen and T.IsOpen() then cmd("places status") end
    end
end)

-- the key bindings' names (Bindings.xml)
BINDING_HEADER_LEYLINETRAVEL = "Leyline travel"
BINDING_NAME_LEYLINE_DENHOME = "Back to my Den"
BINDING_NAME_LEYLINE_TELEPORTS = "Teleports window"
BINDING_NAME_LEYLINE_BACK = "Back (where I was)"

SLASH_LEYLINETELEPORTS1 = "/tp"
SLASH_LEYLINETELEPORTS2 = "/teleports"
SlashCmdList["LEYLINETELEPORTS"] = function(msg)
    msg = tostring(msg or ""):gsub("^%s+", ""):gsub("%s+$", "")
    if msg == "" then return T.Toggle() end
    if msg == "home" then return T.Home() end
    if msg == "back" then return T.Back() end
    -- "/tp stormwind": the best match, at once
    T.query = msg
    local first = T.Entries()[1]
    T.query = ""
    if first then return T.Go(first.name) end
    T.Show(true)
    if frame and frame.search then frame.search:SetText(msg) end
end

W.Teleports = T
