-- Leyline (3.3.5a API): the in-game realm list of the Leyline network, in the look of the game's own Realm Selection
-- screen (the list the player sees before entering the world), built from in-game templates and texture paths only:
--   the dialog box with the gold border and its header ("Realm Selection"), the column headers Realm Name, Type,
--   Characters, Population and Leyline's Players · Bots (click one to sort; again to reverse), rows with the quest
--   log's highlight and a lit selection, the realm categories as tabs along the bottom (All, Official, Community,
--   Single player & Den, Friends' worlds, Favourites), and the original's Okay and Cancel with Leyline's Copy link,
--   Host your own and the home button ("Back to Wrath", or "Back to Wrath Unleashed" with the Leyline brand on).
--   A search box (name, rules tag, region, language, build) and the filters (build, rules tag, region, language, has
--   players now) in a small dropdown; Refresh asks again; "Updated N s ago" from the list's age. Offline realms are
--   greyed and last; the current realm is marked and picked when the list comes. The star by a row keeps it in
--   Favourites (WrathUnleashedUIDB.leyline, per account). Keys while it is open: Enter = Okay, Esc = Cancel,
--   Up / Down move the selection, typing goes to the search. Double-click a row = Okay. With Forever UI its red
--   three-slice buttons skin ours (ForeverUI.Templates.ThreeSliceButton, as ModsPanel.lua's game menu).
--   Under the list: the link box, the friends summary (Social.lua adopts it) and your game folder (GameFolder.lua).
-- Opened by Esc > Realm List, /leyline and /realms. "Host your own" opens the Host window (Host.lua).
-- This realm is one row: its own relay listing (the Beacon of the same realm, e.g. the official realm's) is not a
-- second row. The realm says which listing that is (S:<id>:<live>, below); an older realm doesn't, so a listed realm
-- with this realm's own name is taken as it too. The row then keeps the listing's id: "Copy link" gives the relay
-- link in the desktop client (where the relay is how other clients come in) and the /wrath link in the browser.
--
-- The client can't make HTTP calls, so the realm sends the list (realm/custom/lua_scripts/ov_leyline.lua) as
-- addon messages, prefix WULY, whispered to yourself:
--   client -> server   L                                             send me the list
--   server -> client   B:<count>:<age seconds>                       a list begins
--                      R:<id>:<live 0|1>:<players|->:<bots|->:<ruleset>:<name>   one realm (id "this" = this realm)
--                      M:<id>:<flags>:<score>:<up|->:<build>:<region|->:<lang|->:<max|->:<tags>   the network fields
--                                                                    of the realm before it (flags F featured,
--                                                                    V verified, N new, O official, S sample
--                                                                    hangout, E region estimated)
--                      N:<realms>:<players>:<bots>                   the network line
--                      W:<title>:<by>:<line>                         Den of the week (staff pick it on the site):
--                                                                    said after the network line, as text
--                      V:<n>                                         v2: the protocol version (a v1 realm sends none)
--                      X:<id>:<kind>:<chars|->                       v2: a realm's kind (official, community, local =
--                                                                    Single player on this PC, den) and your
--                                                                    characters there (the realm counts its own)
--                      E:<count>                                     the list is complete
--                      U:<text>                                      no list (e.g. "Realm list unavailable")
--                      S:<id>:<live 0|1>                             this realm's own relay listing (left out of the
--                                                                    R lines); before E
-- Without V/X (a v1 realm) the window still fills: kinds come from the O and S flags and, for this realm, from where
-- you are (Your Den, the home realm, else community); characters show "-" unless remembered from an earlier visit.
-- Friends' worlds come from the Leyline friends list (Social.lua: "<character> on <realm>" for a friend in game).
--
-- The order (the same on every Leyline surface): this realm first when it is a community realm; featured realms;
-- the Leyline Sample Hangout (our own realm, temporary); every other live realm by the network's fair score;
-- offline ones. Filters narrow the community realms; the sample stays. A column sort replaces that order (offline
-- still last). "Host your own" and "Report" put the website's link in the box to copy.
--   client -> server   H:<id|wrath>                                  hop there
--   server -> client   H:<id|wrath>                                  echoed back
--
-- Hopping: the addon can't navigate, so it asks the page. Okay sends H:<id> to the realm, which echoes it; the
-- browser page (stream/www-files/wrath-page.js) watches CHAT_MSG_ADDON through the game bridge and reloads the
-- game at /wrath?go=r:<id>. It also sets the global WUUI_LEYLINE_HOP, which the page reads (and clears) through
-- the bridge, so a hop works on a realm whose server doesn't echo (a Beacon without ov_leyline.lua). The desktop
-- client has no page: after HOP_WAIT seconds the window says to open the link instead.
--
-- The realm browser (2026-10-31): in the browser game the page hands the window the whole network's list, paged, and
-- one realm's preview (stream/www-files/wrath-page.js leyline-browse): WUUI_LEYLINE_ASK = "list|<page>|<filters>|
-- <search>" or "preview|<id>" or "launch|<id>", answered in WUUI_LEYLINE_NET / WUUI_LEYLINE_PREVIEW and a call to
-- WUUI_LEYLINE_ONNET. The page says it can (WUUI_LEYLINE_BROWSE = 1); else the window keeps the realm's list above.
-- Then the search goes to the page and the dropdown has its filters (Plays in browser, Own files, Has players,
-- Version); picking a row opens the preview beside the window (the host's words and house rules, who's on when the
-- realm shares it, its mods, the version chip, uptime, Hero Key sign-in) and Okay joins through it. A realm that
-- needs another game version is never hopped to: Okay hands it to the Leyline launcher (leyline://realm/<id>).

local W = WrathUnleashedUI
-- The public site. Links are PUBLIC_BASE .. "/r/<id>"; a future Leyline domain is this one line.
local PUBLIC_BASE = "https://leylinelauncher.com"
local THIS_REALM_PATH = "/wrath"
local PREFIX = "WULY"
local HOME_REALM = "WrathUnleashed"       -- GetRealmName() on Wrath itself (W.Brand.home while the brand is on)
-- The Leyline brand switch (WrathUnleashedUI.lua W.Brand, from the realm at login): the home realm's name.
local function brand() return W.Brand or { on = false, home = "Wrath", back = "Back to Wrath" } end
local HOP_WAIT = 8                        -- seconds: still here after this = no page took the hop
local WIN_W, WIN_H, ROW_H, ROWS = 680, 620, 18, 14
local SAMPLE_NAME = "Leyline Sample Hangout"
local SAMPLE_NOTE = "temporary"
local ASK_EVERY, ANSWER_WAIT = 30, 6     -- seconds: refresh at most this often; no answer after this = unavailable
local UNAVAILABLE = "Realm list unavailable"
local POP_MEDIUM, POP_HIGH = 10, 50       -- players (people, not bots) for Medium / High when a realm has no maximum

local RULESETS = {
    main = "Level 1 or boost to 80", brawl = "Brawl: everyone 80", progression = "Progression",
    casual = "Casual", dalaran = "Dalaran hangout", custom = "Custom rules",
}
-- The Type column: the original's words (Normal, PvP, RP, RP PvP) where they fit, else the ruleset's short name.
local RULE_TYPE = { main = "Normal", brawl = "Brawl", progression = "Progression", casual = "Casual",
    dalaran = "Hangout", custom = "Custom" }

local TAGS = { blizzlike = "Original rules", boosted = "Boosted", custom = "Custom", pvp = "PvP", rp = "RP",
    hangout = "Hangout", bots = "Bots" }
local TAG_ORDER = { "blizzlike", "boosted", "custom", "pvp", "rp", "hangout", "bots" }
local BUILDS = { [5875] = "1.12.1", [8606] = "2.4.3", [12340] = "3.3.5a", [15595] = "4.3.4", [18414] = "5.4.8" }
local REGIONS = { ["na-east"] = "NA east", ["na-west"] = "NA west", sa = "S. America", eu = "Europe", africa = "Africa",
    me = "Middle East", asia = "Asia", oce = "Oceania" }
local KINDS = { official = true, community = true, ["local"] = true, den = true }

-- The realm categories, as tabs along the bottom of the list.
local TABS = {
    { "all", "All" }, { "official", "Official" }, { "community", "Community" },
    { "mine", "Single player & Den" }, { "friends", "Friends' worlds" }, { "fav", "Favourites" },
}
-- The columns: key (sort), header, width. The first is the favourite star.
local COLS = {
    { nil, "", 22 }, { "name", "Realm Name", 224 }, { "type", "Type", 90 }, { "chars", "Characters", 80 },
    { "pop", "Population", 92 }, { "players", "Players · Bots", 104 },
}

local TEX = {
    border = "Interface\\DialogFrame\\UI-DialogBox-Border",
    background = "Interface\\DialogFrame\\UI-DialogBox-Background",
    header = "Interface\\DialogFrame\\UI-DialogBox-Header",
    insetBg = "Interface\\Tooltips\\UI-Tooltip-Background",
    insetEdge = "Interface\\Tooltips\\UI-Tooltip-Border",
    hover = "Interface\\QuestFrame\\UI-QuestTitleHighlight",
    selected = "Interface\\QuestFrame\\UI-QuestLogTitleHighlight",
    columnTabs = "Interface\\FriendsFrame\\WhoFrame-ColumnTabs",
    sortArrow = "Interface\\Buttons\\UI-SortArrow",
    star = "Interface\\TargetingFrame\\UI-RaidTargetingIcon_1",
}

local win
-- state and the functions the tests reach (W.Leyline). all: every realm as sent; ordered: the network's order after
-- the filters; realms: what the window shows (the tab, the search and the sort on top).
local L = { realms = {}, all = {}, ordered = {}, status = "", offset = 0, filter = {}, tab = "all", sort = {}, proto = 1 }

local function label(parent, text, font, ...)
    local fs = parent:CreateFontString(nil, "OVERLAY", font or "GameFontHighlight")
    fs:SetPoint(...)
    fs:SetJustifyH("LEFT")
    fs:SetText(text)
    return fs
end

-- A frame from an in-game template, or a plain one where this client has no such template.
local function make(kind, name, parent, template)
    if template then
        local ok, f = pcall(CreateFrame, kind, name, parent, template)
        if ok and f then return f end
    end
    return CreateFrame(kind, name, parent)
end

-- Forever UI's red three-slice button (its game menu's look), when Forever UI is loaded.
local function forever()
    return type(ForeverUI) == "table" and type(ForeverUI.Templates) == "table"
        and type(ForeverUI.Templates.ThreeSliceButton) == "function"
end
local function skin(b)
    if not forever() or b.foreverThreeSlice then return end
    pcall(ForeverUI.Templates.ThreeSliceButton, b, "128-redbutton", { GameFontHighlight, GameFontHighlight, GameFontDisable })
end
L.Forever = forever

-- The game's red panel button (UIPanelButtonTemplate), skinned by Forever UI when it is there.
local function button(parent, text, w, onClick, ...)
    local b = make("Button", nil, parent, "UIPanelButtonTemplate")
    b:SetWidth(w); b:SetHeight(22)
    b:SetText(text)
    b:SetPoint(...)
    b:SetScript("OnClick", onClick)
    skin(b)
    return b
end

-- The dialog box: the gold border on the dark dialog background.
local function dialog(f)
    if f.SetBackdrop then
        f:SetBackdrop({ bgFile = TEX.background, edgeFile = TEX.border, tile = true, tileSize = 32, edgeSize = 32,
            insets = { left = 11, right = 12, top = 12, bottom = 11 } })
        f:SetBackdropColor(0, 0, 0, 1)
    end
    local bg = f:CreateTexture(nil, "BACKGROUND")   -- the browser client draws the dialog fill see-through
    bg:SetTexture(0, 0, 0, 0.8)
    bg:SetPoint("TOPLEFT", 11, -12); bg:SetPoint("BOTTOMRIGHT", -12, 11)
end

-- The dialog's header plate with its title, as the game's own dialogs.
local function header(f, text, width)
    local h = f:CreateTexture(nil, "ARTWORK")
    h:SetTexture(TEX.header)
    h:SetWidth(width or 360); h:SetHeight(64)
    h:SetPoint("TOP", 0, 12)
    local t = f:CreateFontString(nil, "OVERLAY", "GameFontNormal")
    t:SetPoint("TOP", h, "TOP", 0, -14)
    t:SetText(text)
    return t, h
end

local function now() return (GetTime and GetTime()) or 0 end

-- A script this client's widget may not have (OnChar, OnArrowPressed): set when it can be, never an error.
local function on(f, script, fn) return (pcall(f.SetScript, f, script, fn)) end

local function norm(x) return (tostring(x or ""):lower():gsub("[^%w]", "")) end

function L.Link(id)
    if id == "this" then return PUBLIC_BASE .. THIS_REALM_PATH end
    return PUBLIC_BASE .. "/r/" .. id
end

-- The browser game's page publishes WUUI_GAMEFOLDER = { page = true, ... } (wrath-page.js); the desktop client has none.
function L.HasPage() return WUUI_PAGE == true or (type(WUUI_GAMEFOLDER) == "table" and WUUI_GAMEFOLDER.page == true) end

-- A row's link: this realm's is the /wrath page in the browser, its relay link in the desktop client when it is listed.
-- Your own worlds remembered from another visit have none (the launcher starts them).
function L.RowLink(r)
    if r.remembered then return nil end
    if r.id == "this" and r.relayId and not L.HasPage() then return L.Link(r.relayId) end
    return L.Link(r.id)
end

-- What Okay does on a row, in words: "You're here" on this realm's row; "Join by link" only where that is how this
-- player would come back in (this realm listed on the relay, live, and no browser page: the desktop client), else Join.
function L.JoinText(r)
    if r.net then return r.here and "You're here" or "View" end
    if r.id ~= "this" then return "Join" end
    if r.relayId and r.relayLive and not L.HasPage() then return "Join by link" end
    return "You're here"
end

-- ---------------------------------------------------------------- Saved (per account): favourites, characters, worlds

function L.DB()
    if type(WrathUnleashedUIDB) ~= "table" then WrathUnleashedUIDB = {} end
    local d = WrathUnleashedUIDB.leyline
    if type(d) ~= "table" then d = {}; WrathUnleashedUIDB.leyline = d end
    for _, k in ipairs({ "fav", "chars", "worlds" }) do if type(d[k]) ~= "table" then d[k] = {} end end
    return d
end

-- A realm's keys: its relay id when it has one, and its name (the same realm seen from inside has no relay id).
local function keys(r)
    local out = {}
    local id = r.id == "this" and r.relayId or (not r.remembered and r.id ~= "this" and r.id) or nil
    if id then out[#out + 1] = id end
    if norm(r.name) ~= "" then out[#out + 1] = "n:" .. norm(r.name) end
    return out
end
L.Keys = keys

function L.IsFav(r)
    if not r then return false end
    local fav = L.DB().fav
    for _, k in ipairs(keys(r)) do if fav[k] then return true end end
    return false
end

function L.ToggleFav(r)
    if not r then return false end
    local fav, on = L.DB().fav, not L.IsFav(r)
    for _, k in ipairs(keys(r)) do fav[k] = on or nil end
    if L.tab == "fav" then L.Apply() end
    L.Refresh()
    return on
end

-- Your characters on a realm: what the realm said (this realm only), else what it said on an earlier visit.
function L.Chars(r)
    if r.chars then return r.chars end
    local chars = L.DB().chars
    for _, k in ipairs(keys(r)) do if tonumber(chars[k]) then return tonumber(chars[k]) end end
    return nil
end

-- Remember this realm's character count, and this realm itself when it is one of your own worlds.
local function remember(this)
    if not this then return end
    local d = L.DB()
    if this.chars then for _, k in ipairs(keys(this)) do d.chars[k] = this.chars end end
    local kind = L.Kind(this)
    if (kind == "local" or kind == "den") and norm(this.name) ~= "" then
        local n = 0
        for _ in pairs(d.worlds) do n = n + 1 end
        local k = "n:" .. norm(this.name)
        if d.worlds[k] or n < 20 then d.worlds[k] = { name = tostring(this.name):sub(1, 40), kind = kind } end
    end
end

-- ---------------------------------------------------------------- Data

local function setStatus(text)
    L.status = text or ""
    if win then win.status:SetText(L.status) end
end

function L.Request(force)
    if not force and L.askedAt and now() - L.askedAt < ASK_EVERY and not L.failed then return false end
    if not SendAddonMessage or not UnitName then setStatus(UNAVAILABLE); return false end
    L.askedAt, L.waiting, L.failed = now(), true, nil
    setStatus("Asking the realm for the realm list...")
    pcall(SendAddonMessage, PREFIX, "L", "WHISPER", UnitName("player"))
    return true
end

-- Text from the realm, safe to show: no escape sequences (a lone "|" starts one: colours, links, textures), no
-- control characters, at most n bytes and never a cut UTF-8 character at the end.
local function plain(s, n)
    s = tostring(s or ""):gsub("|", ""):gsub("[%z\1-\31\127]", " ")
    if #s > n then
        s = s:sub(1, n)
        local lead, tail = s:match("()([\192-\255][\128-\191]*)$")
        if lead then
            local b = tail:byte(1)
            local need = b >= 240 and 4 or b >= 224 and 3 or 2
            if #tail < need then s = s:sub(1, lead - 1) end
        end
    end
    return s
end
L.Plain = plain

local function realmLine(rest)
    local id, live, players, bots, ruleset, name = rest:match("^([%w]+):([01]):([%d%-]+):([%d%-]+):(%a+):(.*)$")
    if not id or not (id == "this" or id:match("^[a-z2-7]+$")) then return nil end
    name = plain(name, 40)   -- the name is the last field: a ":" in it (an older realm's) stays part of it
    return { id = id, live = live == "1", players = tonumber(players), bots = tonumber(bots),
        ruleset = RULESETS[ruleset] and ruleset or "custom", name = name ~= "" and name or "Unnamed realm" }
end

local function metaLine(rest)
    local id, flags, score, up, build, region, lang, maxp, tags =
        rest:match("^([%w]+):([%u%-]+):(%d+):([%d%-]+):(%d+):([%w%-]+):([%a%-]+):([%d%-]+):([%a,]*)$")
    if not id then return nil end
    local t = {}
    for x in tags:gmatch("%a+") do if TAGS[x] then t[#t + 1] = x end end
    return id, { featured = flags:find("F") ~= nil, verified = flags:find("V") ~= nil, new = flags:find("N") ~= nil,
        sample = flags:find("S") ~= nil, est = flags:find("E") ~= nil, official = flags:find("O") ~= nil,
        score = tonumber(score) or 0,
        up = tonumber(up), build = BUILDS[tonumber(build) or 0] and tonumber(build) or 12340,
        region = REGIONS[region] and region or nil, lang = (lang:match("^%l%l%l?$") or lang:match("^%l%l%-%u%u$")) and lang or nil,
        maxp = tonumber(maxp), tags = t }
end

-- The filters: every value a community realm in the list has, in order.
local function values(key)
    local seen, out = {}, {}
    for _, r in ipairs(L.all) do
        if r.id ~= "this" then
            local vs = key == "tag" and (r.tags or {}) or { r[key] }
            for _, v in ipairs(vs) do
                if v ~= nil and not seen[v] then seen[v] = true; out[#out + 1] = v end
            end
        end
    end
    if key == "tag" then
        local ordered = {}
        for _, t in ipairs(TAG_ORDER) do if seen[t] then ordered[#ordered + 1] = t end end
        return ordered
    end
    table.sort(out, function(a, b)
        if type(a) == "number" and type(b) == "number" then return a < b end
        return tostring(a) < tostring(b)
    end)
    return out
end
L.Values = values

local function matches(r)
    local f = L.filter
    if f.build and (r.build or 12340) ~= f.build then return false end
    if f.region and r.region ~= f.region then return false end
    if f.lang and (r.lang or ""):sub(1, 2) ~= f.lang:sub(1, 2) then return false end
    if f.now and not (r.live and (r.players or 0) > 0) then return false end
    if f.tag then
        local has = false
        for _, t in ipairs(r.tags or {}) do if t == f.tag then has = true end end
        if not has then return false end
    end
    return true
end

-- What the window shows, in the network's order (see the top of this file).
local function sameName(a, b)
    if type(a) ~= "string" or type(b) ~= "string" then return false end
    return norm(a) ~= "" and norm(a) == norm(b)
end
L.SameName = sameName

-- This realm's own relay listing (the realm's S line, or a non-sample listing with this realm's name): merged into
-- the "this" row, never a row of its own.
local function isSelf(r, this)
    if not this or r.id == "this" or r.sample then return false end
    if L.selfId then return r.id == L.selfId end
    return sameName(r.name, this.name)
end
L.IsSelf = isSelf

function L.OnWrath()
    if not GetRealmName then return true end
    local ok, name = pcall(GetRealmName)
    -- Home is our realm under any of its names: today's, the Sample Hangout's, the Leyline brand's, the official name
    -- and the one it had before (Dalaran Hangout, until 2026-10-30).
    return not ok or name == nil or name == HOME_REALM or name == "Leyline Sample Hangout"
        or name == W.OFFICIAL_NAME or name == "Dalaran Hangout" or (brand().on and name == brand().home)
end

local function inDen() return W.InDen and W.InDen() or false end

-- A realm's category: what the realm said (X, v2), else from its flags (O official, S our own hangout) and, for this
-- realm, from where you are.
function L.Kind(r)
    if r.kind and KINDS[r.kind] then return r.kind end
    if r.id == "this" or r.here then
        if inDen() then return "den" end
        if r.sample or r.official or L.OnWrath() then return "official" end
        return "community"
    end
    if r.official or r.sample then return "official" end
    return "community"
end

function L.View()
    local this, sample, featured, rest, offline = nil, nil, {}, {}, {}
    for _, r in ipairs(L.all) do if r.id == "this" then this = r end end
    -- Your Den (Den Lite has no realm list of its own): still one row, this one.
    if not this and inDen() then
        this = { id = "this", name = W.DEN_REALM or "Your Den", live = true, ruleset = "custom", kind = "den" }
        table.insert(L.all, 1, this)
    end
    local thisSample = this and this.sample
    if this then this.relayId, this.relayLive = L.selfId, L.selfLive end
    for _, r in ipairs(L.all) do
        if r.id == "this" then
            -- placed below
        elseif isSelf(r, this) then
            this.relayId, this.relayLive = r.id, r.live
        elseif r.sample then
            if not thisSample then sample = r end   -- on the sample itself, its relay record is this realm again
        elseif matches(r) then
            if not r.live then offline[#offline + 1] = r
            elseif r.featured then featured[#featured + 1] = r
            else rest[#rest + 1] = r end
        end
    end
    local out = {}
    local function add(list) for _, r in ipairs(list) do out[#out + 1] = r end end
    if this and not thisSample then out[#out + 1] = this end
    add(featured)
    if thisSample then out[#out + 1] = this elseif sample then out[#out + 1] = sample end
    add(rest); add(offline)
    remember(this)
    L.ordered = out
    return L.Apply()
end

-- ---------------------------------------------------------------- Tabs, search, sort

-- Your Leyline friends in a world now (Social.lua's list: "<character> on <realm>"), or nil without that list.
function L.FriendsInWorlds()
    local S = W.Social
    if type(S) ~= "table" or type(S.rows) ~= "table" then return nil end
    local out = {}
    for _, f in ipairs(S.rows) do
        if type(f) == "table" and f.status == "g" and type(f.where) == "string" then
            local realm = f.where:match(" on (.+)$")
            if realm then out[#out + 1] = { name = tostring(f.name or ""), realm = realm } end
        end
    end
    return out
end

-- The names of your friends on realm r.
function L.FriendsOn(r)
    local out = {}
    local here = r.id == "this" or r.here
    local myRealm = here and GetRealmName and select(2, pcall(GetRealmName)) or nil
    for _, f in ipairs(L.FriendsInWorlds() or {}) do
        if sameName(f.realm, r.name) or (myRealm and sameName(f.realm, myRealm)) then out[#out + 1] = f.name end
    end
    return out
end

function L.InTab(r, tab)
    tab = tab or L.tab
    if tab == "all" then return true end
    local kind = L.Kind(r)
    if tab == "official" then return kind == "official" end
    if tab == "community" then return kind == "community" end
    if tab == "mine" then return kind == "local" or kind == "den" end
    if tab == "friends" then return #L.FriendsOn(r) > 0 end
    if tab == "fav" then return L.IsFav(r) end
    return true
end

-- Search: the name, the type, the ruleset, the rules tags, the region, the language and the build.
function L.Matches(r, q)
    q = tostring(q or ""):lower():gsub("^%s+", ""):gsub("%s+$", "")
    if q == "" then return true end
    local hay = { r.name, L.Type(r), RULESETS[r.ruleset], r.region, r.region and REGIONS[r.region], r.lang,
        r.build and tostring(r.build), r.build and BUILDS[r.build], r.chip, L.Kind(r) }
    for _, t in ipairs(r.tags or {}) do hay[#hay + 1] = t; hay[#hay + 1] = TAGS[t] end
    for _, h in ipairs(hay) do
        if type(h) == "string" and h:lower():find(q, 1, true) then return true end
    end
    return false
end

-- The Type column.
function L.Type(r)
    local kind = L.Kind(r)
    if kind == "den" then return "Den" end
    if kind == "local" then return "Single player" end
    if r.net then return r.browser and "In browser" or "Own files" end
    local pvp, rp = false, false
    for _, t in ipairs(r.tags or {}) do if t == "pvp" then pvp = true elseif t == "rp" then rp = true end end
    if pvp and rp then return "RP PvP" end
    if pvp then return "PvP" end
    if rp then return "RP" end
    return RULE_TYPE[r.ruleset] or "Custom"
end

-- The Population column, coloured as the original: Low green, Medium yellow, High and Full red; Offline grey.
-- Against the realm's maximum when it has one (a quarter, 60 %, full), else POP_MEDIUM / POP_HIGH players.
local POP_RANK = { Low = 1, Medium = 2, High = 3, Full = 4 }
function L.Population(r)
    if r.remembered then return "On this PC", 0.62, 0.62, 0.62 end
    if not r.live then return "Offline", 0.5, 0.5, 0.5 end
    local p = r.players
    if not p then return "-", 0.62, 0.62, 0.62 end
    local level
    if r.maxp and r.maxp > 0 then
        local f = p / r.maxp
        level = f >= 1 and "Full" or f >= 0.6 and "High" or f >= 0.25 and "Medium" or "Low"
    else
        level = p >= POP_HIGH and "High" or p >= POP_MEDIUM and "Medium" or "Low"
    end
    if level == "Low" then return level, 0.1, 1, 0.1 end
    if level == "Medium" then return level, 1, 1, 0.1 end
    return level, 1, 0.1, 0.1
end

function L.PlayersText(r)
    if not r.live or r.remembered then return "-" end
    if not r.players and not r.bots then return "-" end
    return (r.players and tostring(r.players) or "-") .. " · " .. (r.bots and tostring(r.bots) or "0")
end

local function sortValue(r, key)
    if key == "name" then return tostring(L.RowName(r) or ""):lower() end
    if key == "type" then return L.Type(r):lower() end
    if key == "chars" then return L.Chars(r) or -1 end
    if key == "pop" then local p = L.Population(r); return POP_RANK[p] or 0 end
    if key == "players" then return (r.players or -1) * 1000 + (r.bots or 0) end
    return 0
end

-- Click a column header: sort by it; again: the other way; a third time: back to the network's order.
function L.SortBy(key)
    local s = L.sort
    if s.key ~= key then L.sort = { key = key, desc = (key == "pop" or key == "players" or key == "chars") }
    elseif s.desc == (key == "pop" or key == "players" or key == "chars") then L.sort = { key = key, desc = not s.desc }
    else L.sort = {} end
    L.Apply()
    L.Refresh()
    return L.sort
end

-- Your own worlds this client has seen (Single player, Your Den) other than this one, for the Single player & Den tab.
local function rememberedWorlds()
    local out, this = {}, nil
    for _, r in ipairs(L.ordered) do if r.id == "this" then this = r end end
    for k, w in pairs(L.DB().worlds) do
        if type(w) == "table" and type(w.name) == "string" and not (this and sameName(w.name, this.name)) then
            out[#out + 1] = { id = "mine", key = k, name = w.name, kind = KINDS[w.kind] and w.kind or "local",
                remembered = true, live = false, ruleset = "custom" }
        end
    end
    table.sort(out, function(a, b) return a.name:lower() < b.name:lower() end)
    return out
end

-- The tab, the search (on the realm's list; the page searches its own) and the sort on top of the network's order.
function L.Apply()
    local out, browsing = {}, L.Browsing()
    local list = {}
    for i, r in ipairs(L.ordered) do list[#list + 1] = r; r._i = i end
    if L.tab == "mine" and not browsing then
        for _, r in ipairs(rememberedWorlds()) do list[#list + 1] = r; r._i = #list end
    end
    for _, r in ipairs(list) do
        if L.InTab(r) and (browsing or L.Matches(r, L.query)) then out[#out + 1] = r end
    end
    local key, desc = L.sort.key, L.sort.desc
    table.sort(out, function(a, b)
        -- plain booleans: a nil beside a false would break the sort's order (table.sort: "invalid order function")
        local la, lb = (a.live and not a.remembered) and true or false, (b.live and not b.remembered) and true or false
        if la ~= lb then return la end   -- offline last, whatever the sort
        if key then
            local va, vb = sortValue(a, key), sortValue(b, key)
            if va ~= vb then if desc then return va > vb end return va < vb end
        end
        return a._i < b._i
    end)
    L.realms = out
    if L.selected then   -- the selection stays when it is still listed
        local still
        for _, r in ipairs(out) do if r == L.selected or (r.id == L.selected.id and r.id ~= "mine") then still = r end end
        L.selected = still
    end
    return out
end

function L.SetTab(tab)
    for _, t in ipairs(TABS) do
        if t[1] == tab then
            L.tab, L.offset = tab, 0
            if tab == "friends" and W.Social and W.Social.Request then pcall(W.Social.Request, false) end
            L.Apply()
            L.Refresh()
            return true
        end
    end
    return false
end

-- The search box's words: the realm's list is searched here at once; the page's list is asked after a pause.
function L.SetSearch(q)
    q = tostring(q or ""):gsub("[|%c]", ""):sub(1, 32)
    if q == L.query then return end
    L.query, L.offset = q, 0
    if L.Browsing() then L.searchAt = now() else L.Apply() end
    L.Refresh()
end

-- What the list says when it shows nothing.
function L.EmptyText()
    if #L.realms > 0 then return "" end
    if L.tab == "fav" then return "No favourites yet. Click the star by a realm to keep it here." end
    if L.tab == "mine" then
        return "Your own worlds show here: Single player and Your Den. Start them from the Leyline launcher."
    end
    if L.tab == "friends" then
        if not W.Social then return "Friends' worlds are coming: the realms your Leyline friends are on show here." end
        return "None of your Leyline friends are in a world right now."
    end
    if (L.query or "") ~= "" or next(L.filter) or next(L.bfilter) then return "No realm matches. Try fewer words or filters." end
    if L.waiting or L.browseAt then return "" end
    return "No realms are listed right now."
end

function L.SetFilter(key, value)
    L.filter[key] = value
    L.offset = 0
    L.View()
    L.Refresh()
end

function L.ClearFilters()
    L.filter = {}
    if L.Browsing() and next(L.bfilter) then L.bfilter = {}; L.AskList(1) end
    L.offset = 0
    L.View()
    L.Refresh()
end

function L.Cycle(key)
    if key == "now" then
        L.filter.now = not L.filter.now or nil
    else
        local vs, cur, nextv = values(key), L.filter[key], nil
        if cur == nil then nextv = vs[1] else
            for i, v in ipairs(vs) do if v == cur then nextv = vs[i + 1] end end
        end
        L.filter[key] = nextv
    end
    L.offset = 0
    L.View()
    L.Refresh()
end

function L.FilterCount()
    local n = 0
    for _ in pairs(L.Browsing() and L.bfilter or L.filter) do n = n + 1 end
    return n
end

local FILTER_NAMES = { build = "Build", tag = "Rules", region = "Region", lang = "Language" }
local function valueName(key, v)
    if key == "build" then return BUILDS[v] or tostring(v) end
    if key == "tag" then return TAGS[v] or v end
    if key == "region" then return REGIONS[v] or v end
    return tostring(v)
end
L.ValueName = valueName

-- The filters dropdown's lines (UIDropDownMenu info tables), level 1 and the value lists of level 2.
function L.MenuItems(level, value)
    local items = {}
    local function close() if CloseDropDownMenus then pcall(CloseDropDownMenus) end end
    if L.Browsing() then
        local bf = L.bfilter
        if level == 2 and value == "build" then
            items[#items + 1] = { text = "Any", checked = bf.build == nil, func = function() bf.build = nil; L.AskList(1); L.Refresh(); close() end }
            for _, b in ipairs({ 12340, 5875, 8606, 15595, 18414 }) do
                items[#items + 1] = { text = BUILDS[b], checked = bf.build == b,
                    func = function() bf.build = b; L.AskList(1); L.Refresh(); close() end }
            end
            return items
        end
        items[#items + 1] = { text = "Filters", isTitle = true, notCheckable = true }
        for _, k in ipairs({ { "browser", "Plays in browser" }, { "own", "Own game files" }, { "players", "Has players now" } }) do
            items[#items + 1] = { text = k[2], checked = bf[k[1]] == true, keepShownOnClick = true,
                func = function() L.BCycle(k[1]) end }
        end
        items[#items + 1] = { text = "Version", hasArrow = true, value = "build", notCheckable = true }
        items[#items + 1] = { text = "Clear filters", notCheckable = true, func = function() L.ClearFilters(); close() end }
        return items
    end
    if level == 2 and FILTER_NAMES[value] then
        items[#items + 1] = { text = "Any", checked = L.filter[value] == nil, func = function() L.SetFilter(value, nil); close() end }
        for _, v in ipairs(values(value)) do
            items[#items + 1] = { text = valueName(value, v), checked = L.filter[value] == v,
                func = function() L.SetFilter(value, v); close() end }
        end
        return items
    end
    items[#items + 1] = { text = "Filters", isTitle = true, notCheckable = true }
    for _, k in ipairs({ "build", "tag", "region", "lang" }) do
        local cur = L.filter[k]
        items[#items + 1] = { text = FILTER_NAMES[k] .. ": " .. (cur ~= nil and valueName(k, cur) or "any"),
            hasArrow = true, value = k, notCheckable = true }
    end
    items[#items + 1] = { text = "Has players now", checked = L.filter.now == true, keepShownOnClick = true,
        func = function() L.Cycle("now") end }
    items[#items + 1] = { text = "Clear filters", notCheckable = true, func = function() L.ClearFilters(); close() end }
    return items
end

local function networkLine(n)
    return string.format("%d realm%s · %d player%s online", n.realms, n.realms == 1 and "" or "s", n.players,
        n.players == 1 and "" or "s") .. (n.bots > 0 and (" · " .. n.bots .. " bots") or "")
end
L.NetworkLine = networkLine

-- "Updated N s ago" from the list's age (the B line) and the time since it came.
function L.UpdatedText()
    if not L.listAt then return "" end
    local s = math.max(0, math.floor(now() - L.listAt))
    if s < 60 then return string.format("Updated %d s ago", s) end
    if s < 7200 then return string.format("Updated %d min ago", math.floor(s / 60)) end
    return string.format("Updated %d h ago", math.floor(s / 3600))
end

-- The current realm, picked when the list comes and nothing else is.
local function pickCurrent()
    if L.selected then return end
    for _, r in ipairs(L.realms) do if r.id == "this" or r.here then L.selected = r; return end end
end

function L.OnMessage(prefix, msg, channel, sender)
    if prefix ~= PREFIX or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local kind, rest = msg:match("^(%u):(.*)$")
    if kind == "B" then
        L.incoming, L.network, L.selfId, L.selfLive, L.denWeek, L.incomingProto = {}, nil, nil, nil, nil, nil
        L.age = tonumber(rest:match(":(%d+)$") or "")
    elseif kind == "M" and L.incoming then
        local id, m = metaLine(rest)
        local last = L.incoming[#L.incoming]
        if id and last and last.id == id then
            for k, v in pairs(m) do last[k] = v end
        end
    elseif kind == "V" and L.incoming then
        L.incomingProto = tonumber(rest:match("^(%d+)$") or "")
    elseif kind == "X" and L.incoming then
        local id, k, chars = rest:match("^([%w]+):(%a+):([%d%-]+)$")
        if id and KINDS[k] then
            for _, r in ipairs(L.incoming) do
                if r.id == id then r.kind = k; r.chars = tonumber(chars) end
            end
        end
    elseif kind == "S" and L.incoming then
        local id, live = rest:match("^([a-z2-7]+):([01])$")
        if id and #id == 16 then L.selfId, L.selfLive = id, live == "1" end
    elseif kind == "N" and L.incoming then
        local n, p, b = rest:match("^(%d+):(%d+):(%d+)$")
        if n then L.network = { realms = tonumber(n), players = tonumber(p), bots = tonumber(b) } end
    elseif kind == "W" and L.incoming then
        local t, by, line = rest:match("^([^:]+):([^:]+):(.*)$")   -- the line is last: a ":" in it stays
        if t then L.denWeek = { title = plain(t, 40), by = plain(by, 24), line = plain(line, 140) } end
    elseif kind == "R" and L.incoming then
        local r = realmLine(rest)
        if r and #L.incoming < 100 then L.incoming[#L.incoming + 1] = r end
    elseif kind == "E" and L.incoming then
        L.all, L.incoming, L.waiting = L.incoming, nil, nil
        L.proto = L.incomingProto or 1
        L.listAt = now() - (L.age or 0)
        L.View()
        pickCurrent()
        local n = 0
        for _, r in ipairs(L.ordered) do if r.id ~= "this" then n = n + 1 end end
        local dw = L.denWeek and (" Den of the week: " .. L.denWeek.title .. " by " .. L.denWeek.by .. ".") or ""
        if L.network then
            setStatus(networkLine(L.network) .. "." .. dw)
        else
            setStatus(n == 0 and "No other realms are open right now." or (n == 1 and "1 other realm" or (n .. " other realms")) .. " on Leyline.")
        end
        L.Refresh()
    elseif kind == "H" and L.hopping and rest == L.hopping.target then
        L.hopping.echoed = true   -- the page reloads the game on it now
    elseif kind == "U" then
        L.incoming, L.waiting, L.failed = nil, nil, true
        L.all = {}
        L.View()
        setStatus(rest ~= "" and plain(rest, 120) or UNAVAILABLE)
        L.Refresh()
    end
end

-- ---------------------------------------------------------------- The realm browser (the page's list)

local PANEL_W = 340
local SEARCH_WAIT = 0.5                   -- seconds of no typing before the search is asked
L.bfilter, L.query = {}, ""

function L.Browsing() return WUUI_LEYLINE_BROWSE == 1 end

-- The running game's build (GetBuildInfo's second value), 12340 when unknown.
function L.MyBuild()
    if not GetBuildInfo then return 12340 end
    local ok, _, b = pcall(GetBuildInfo)
    return (ok and tonumber(b)) or 12340
end

function L.AskList(page)
    local f = {}
    if L.bfilter.browser then f[#f + 1] = "b" end
    if L.bfilter.own then f[#f + 1] = "o" end
    if L.bfilter.players then f[#f + 1] = "p" end
    if L.bfilter.build then f[#f + 1] = "v" .. L.bfilter.build end
    L.netPage = math.max(1, math.floor(tonumber(page) or 1))
    local q = (L.query or ""):gsub("[|%c]", ""):sub(1, 32)
    WUUI_LEYLINE_ASK = "list|" .. L.netPage .. "|" .. table.concat(f, ",") .. "|" .. q
    L.browseAt = now()
    setStatus("Asking for the realm list...")
    return WUUI_LEYLINE_ASK
end

-- One row of the page's list, in the window's row shape.
function L.FromNet(r)
    return { id = r.id, name = tostring(r.name or ""):gsub("|", ""):sub(1, 40), live = r.live == true,
        players = tonumber(r.online), bots = tonumber(r.bots), build = BUILDS[tonumber(r.build) or 0] and tonumber(r.build) or 12340,
        chip = type(r.chip) == "string" and r.chip or nil, browser = r.browser == true, own = r.own == true, hk = r.hk,
        up = tonumber(r.up), featured = r.featured == true, verified = r.verified == true, here = r.here == true,
        official = r.official == true, maxp = tonumber(r.max),
        mods = tonumber(r.mods) or 0, desc = type(r.desc) == "string" and r.desc or "", ruleset = "custom", net = true }
end

function L.OnNet(kind)
    if kind == "list" then
        local n = WUUI_LEYLINE_NET
        if type(n) ~= "table" then return end
        L.browseAt = nil
        if type(n.err) == "string" then
            L.browse, L.ordered = nil, {}
            L.Apply()
            setStatus(n.err:gsub("|", ""))
            L.Refresh()
            return
        end
        L.browse = n
        local out = {}
        for _, r in ipairs(type(n.rows) == "table" and n.rows or {}) do
            if type(r) == "table" and type(r.id) == "string" and r.id:match("^[a-z2-7]+$") and #r.id == 16 and #out < 50 then
                out[#out + 1] = L.FromNet(r)
            end
        end
        L.ordered = out
        L.Apply()
        L.offset = L.backPage and math.max(0, #L.realms - ROWS) or 0
        L.backPage = nil
        L.listAt = now()
        pickCurrent()
        local net = type(n.network) == "table" and n.network or nil
        local line = net and networkLine({ realms = tonumber(net.realms) or 0, players = tonumber(net.players) or 0,
            bots = tonumber(net.bots) or 0 }) .. "." or ""
        if (tonumber(n.total) or 0) == 0 then
            line = (L.query ~= "" or next(L.bfilter)) and "No realm matches. Try fewer filters." or "No realms are open right now."
        end
        setStatus(line)
        L.Refresh()
    elseif kind == "preview" then
        local p = WUUI_LEYLINE_PREVIEW
        if type(p) == "table" and L.selected and p.id == L.selected.id then
            L.detail = p
            L.ShowPreview(L.selected)
        end
    end
end
WUUI_LEYLINE_ONNET = function(kind) L.OnNet(kind) end

-- Plays in browser, Own files, Has players: on/off. Version: every version the list shows, then any.
function L.BCycle(key)
    if key == "build" then
        local seen, order = {}, { 12340, 5875, 8606, 15595, 18414 }
        for _, r in ipairs(L.ordered) do if r.build then seen[r.build] = true end end
        if L.bfilter.build then seen[L.bfilter.build] = true end
        local vs = {}
        for _, b in ipairs(order) do if seen[b] then vs[#vs + 1] = b end end
        local cur, nextv = L.bfilter.build, nil
        if cur == nil then nextv = vs[1] else for i, v in ipairs(vs) do if v == cur then nextv = vs[i + 1] end end end
        L.bfilter.build = nextv
    else
        L.bfilter[key] = not L.bfilter[key] or nil
    end
    L.AskList(1)
    L.Refresh()
end

-- What Join does here: ok, own (the page switches to own game files), launcher (another game version: the Leyline
-- launcher), offline. The page's preview says it; without one, from what the row knows.
function L.Verdict(r, p)
    if p and type(p.verdict) == "string" then return p.verdict, type(p.why) == "string" and p.why or "" end
    if not r.live then return "offline", "This realm is offline right now." end
    if r.build and r.build ~= L.MyBuild() then
        return "launcher", "This realm needs a different game version than this one. Open it in the Leyline launcher."
    end
    return "ok", ""
end

local function chipOf(r) return r.chip or BUILDS[r.build or 12340] or "?" end

-- The preview's words (pure: the tests read it). p: the page's WUUI_LEYLINE_PREVIEW for r, or nil.
function L.PreviewTexts(r, p)
    local t = {}
    t.title = (p and p.name) or r.name
    local chips = { "|cff8b5cf6" .. chipOf(r) .. "|r" }
    local browser = p and p.browser or r.browser
    if r.net or p then chips[#chips + 1] = browser and "Plays in browser" or "Own game files" end
    local hk = p and p.hk or (r.hk == true and "yes" or r.hk == false and "no" or nil)
    if hk == "ready" or hk == "yes" then chips[#chips + 1] = "|cff4cc38aHero Key sign-in|r"
    elseif hk == "no" then chips[#chips + 1] = "Own accounts" end
    t.chips = table.concat(chips, "  ·  ")
    local online = p and tonumber(p.online) or r.players
    local bots = p and tonumber(p.bots) or r.bots
    local live = (p and p.live ~= nil) and p.live or r.live
    if not live then t.players = "|cff9aa3b5Offline|r"
    elseif online then t.players = online .. (online == 1 and " player" or " players") .. " online now"
        .. ((bots and bots > 0) and (" · " .. bots .. " bots") or "")
    else t.players = "|cff4cc38aOnline|r" end
    local up = p and tonumber(p.up) or r.up
    t.up = up and (up .. "% up lately") or "Uptime: new"
    t.desc = (p and p.desc ~= "" and p.desc) or r.desc or ""
    t.rules = (p and p.rules ~= "" and p.rules) or ((not p and r.net) and "Loading..." or "")
    if t.rules == "" then t.rules = "The host hasn't written house rules." end
    if p and type(p.who) == "table" then
        local names = {}
        for i, n in ipairs(p.who) do if i <= 50 and type(n) == "string" then names[#names + 1] = n end end
        t.who = #names > 0 and table.concat(names, ", ") or "Nobody right now."
    elseif p then
        t.who = "This realm doesn't share who's on."
    else
        t.who = r.net and "Loading..." or "Shown in the browser game."
    end
    if p and type(p.mods) == "table" then
        local req, opt, other = {}, {}, {}
        for _, m in ipairs(p.mods) do
            if type(m) == "table" and type(m.name) == "string" then
                local n = m.name .. (m.client == true and " |cff5b8def(on your side)|r" or "")
                if m.req == true then req[#req + 1] = n elseif m.req == false then opt[#opt + 1] = n else other[#other + 1] = n end
            end
        end
        local lines = {}
        if #req > 0 then lines[#lines + 1] = "Required: " .. table.concat(req, ", ") end
        if #opt > 0 then lines[#lines + 1] = "Optional: " .. table.concat(opt, ", ") end
        if #other > 0 then lines[#lines + 1] = "Uses: " .. table.concat(other, ", ") end
        t.mods = #lines > 0 and table.concat(lines, "\n") or "No mods listed."
    else
        t.mods = r.net and "Loading..." or "Shown in the browser game."
    end
    local v, why = L.Verdict(r, p)
    t.verdict, t.why = v, why
    if r.here or r.id == "this" then t.join, t.why = "You're here", "You're on this realm now."
    elseif v == "launcher" then t.join = "Open in launcher"
    elseif v == "offline" then t.join = "Offline"
    else t.join = "Join" end
    return t
end

function L.Preview(r)
    if not r then return false end
    L.selected, L.detail = r, nil
    if r.id ~= "this" and L.Browsing() then WUUI_LEYLINE_ASK = "preview|" .. r.id end
    L.ShowPreview(r)
    return true
end

-- The preview's Join: never a broken hop.
function L.PreviewJoin()
    local r = L.selected
    if not r then return false end
    local t = L.PreviewTexts(r, L.detail)
    if t.join == "You're here" then return L.Join(r) end
    if t.verdict == "launcher" then
        if L.Browsing() then WUUI_LEYLINE_ASK = "launch|" .. r.id end
        L.panelInfo(r.name .. " needs another game version. " .. (L.HasPage() and "The bar at the top of the page opens it in the Leyline launcher."
            or "Open it in the Leyline launcher: the link is in the box."))
        if win and L.Copy then L.Copy("leyline://realm/" .. r.id) end
        return "launcher"
    end
    if t.verdict == "offline" then L.panelInfo(r.name .. " is offline right now. Try again later."); return false end
    L.panelInfo("Going to " .. r.name .. "...")
    return L.Hop(r.id, r.name)
end

-- ---------------------------------------------------------------- Window

local function copy(text)
    if not win or not text then return end
    local box = win.copy
    box.fixed = text; box:SetText(text); box:SetFocus(); box:HighlightText()
end
L.Copy = copy

-- The row's long texts (the tooltip; the tests read them): the name (with the sample's label and the Featured /
-- Verified / New badges), the rules line (ruleset, then tags, build, region, language when the realm has them) and
-- the state (with the uptime badge).
function L.Texts(r)
    if r.remembered then
        return r.name, (r.kind == "den" and "Your Den" or "Single player") .. " on this PC", "|cff9aa3b5not running here|r"
    end
    if r.net then
        local name = r.name .. (r.here and "  |cff8b5cf6(you are here)|r" or "")
        local badges = {}
        if r.featured then badges[#badges + 1] = "|cff8b5cf6Featured|r" end
        if r.verified then badges[#badges + 1] = "|cff4cc38aVerified|r" end
        if #badges > 0 then name = name .. "  " .. table.concat(badges, " ") end
        local chips = { chipOf(r), r.browser and "Plays in browser" or "Own game files" }
        if r.hk == true then chips[#chips + 1] = "Hero Key" end
        if r.mods and r.mods > 0 then chips[#chips + 1] = r.mods .. (r.mods == 1 and " mod" or " mods") end
        local state = r.live and "|cff4cc38aonline|r" or "|cff9aa3b5offline|r"
        if r.up then state = state .. " · " .. r.up .. "% up" end
        return name, table.concat(chips, " · "), state
    end
    local name = L.RowName(r)
    if r.id == "this" then name = name .. "  |cff8b5cf6(you are here)|r" end
    local badges = {}
    if r.featured then badges[#badges + 1] = "|cff8b5cf6Featured|r" end
    if r.verified then badges[#badges + 1] = "|cff4cc38aVerified|r" end
    if r.new then badges[#badges + 1] = "|cff5b8defNew|r" end
    if #badges > 0 then name = name .. "  " .. table.concat(badges, " ") end
    local extra = {}
    for _, t in ipairs(r.tags or {}) do if r.id ~= "this" or not r.sample then extra[#extra + 1] = TAGS[t] end end
    if r.build and r.build ~= 12340 then extra[#extra + 1] = BUILDS[r.build] end
    if r.region then extra[#extra + 1] = REGIONS[r.region] .. (r.est and "?" or "") end
    if r.lang then extra[#extra + 1] = r.lang end
    local rules = RULESETS[r.ruleset] .. (#extra > 0 and (" · " .. table.concat(extra, ", ")) or "")
    local state = r.live and "|cff4cc38aonline|r" or "|cff9aa3b5offline|r"
    if r.up then state = state .. " · " .. r.up .. "% up" end
    return name, rules, state
end

-- The name in the Realm Name column (the sample hangout's label; the badges are in the tooltip).
function L.RowName(r)
    if r.sample and not r.net then return (brand().on and brand().home or SAMPLE_NAME) .. " · " .. SAMPLE_NOTE end
    return r.name
end

-- The columns of a row, plain (the tests read them): name, type, characters, population (+ colour), players · bots.
function L.Columns(r)
    local name = L.RowName(r)
    if r.id == "this" or r.here then name = name .. " |cffffd100(current realm)|r" end
    local chars = L.Chars(r)
    local pop, pr, pg, pb = L.Population(r)
    return { name = name, type = L.Type(r), chars = chars and tostring(chars) or "-", pop = pop, popColor = { pr, pg, pb },
        players = L.PlayersText(r) }
end

local function info(text) if win then win.info:SetText(text) end end

local function isSel(r) return L.selected ~= nil and r ~= nil and (L.selected == r or (r.id ~= "mine" and L.selected.id == r.id)) end

function L.Refresh()
    if not win then return end
    local n = #L.realms
    if L.offset > math.max(0, n - ROWS) then L.offset = math.max(0, n - ROWS) end
    for i, row in ipairs(win.rows) do
        local r = L.realms[L.offset + i]
        row.realm = r
        if r then
            local c = L.Columns(r)
            row.name:SetText(c.name)
            row.type:SetText(c.type)
            row.chars:SetText(c.chars)
            row.pop:SetText(c.pop)
            row.pop:SetTextColor(c.popColor[1], c.popColor[2], c.popColor[3])
            row.players:SetText(c.players)
            local grey = not r.live or r.remembered
            for _, fs in ipairs({ row.name, row.type, row.chars, row.players }) do
                if grey then fs:SetTextColor(0.5, 0.5, 0.5) else fs:SetTextColor(1, 1, 1) end
            end
            if (r.id == "this" or r.here) and not grey then row.name:SetTextColor(1, 0.82, 0) end
            row.fav = L.IsFav(r)
            row.star.icon:SetAlpha(row.fav and 1 or 0.2)
            if row.star.icon.SetDesaturated then row.star.icon:SetDesaturated(not row.fav) end
            if isSel(r) then row.sel:Show(); row:LockHighlight() else row.sel:Hide(); row:UnlockHighlight() end
            row:Show()
        else
            row.sel:Hide()
            row:Hide()
        end
    end
    win.empty:SetText(L.EmptyText())
    win.count:SetText(n > ROWS and string.format("%d-%d of %d", L.offset + 1, math.min(n, L.offset + ROWS), n) or "")
    local b = L.browse
    if L.Browsing() and b and (tonumber(b.pages) or 1) > 1 then
        win.count:SetText(string.format("Page %d of %d · %d realms", tonumber(b.page) or 1, tonumber(b.pages) or 1, tonumber(b.total) or n))
    end
    for i, t in ipairs(TABS) do
        local tab = win.tabs[i]
        tab.on = (t[1] == L.tab)
        if PanelTemplates_SetTab and tab.on then pcall(PanelTemplates_SetTab, win, i) end
        if not PanelTemplates_SetTab then if tab.on then tab:LockHighlight() else tab:UnlockHighlight() end end
    end
    for _, h in ipairs(win.headers) do
        if h.key and L.sort.key == h.key then
            h.arrow:Show()
            if L.sort.desc then h.arrow:SetTexCoord(0, 0.5625, 0, 1) else h.arrow:SetTexCoord(0, 0.5625, 1, 0) end
        else
            h.arrow:Hide()
        end
    end
    local fc = L.FilterCount()
    win.filterBtn:SetText(fc > 0 and ("Filters (" .. fc .. ")") or "Filters")
    if (L.query or "") ~= "" or win.search.focused then win.searchHint:Hide() else win.searchHint:Show() end
    win.updated:SetText(L.UpdatedText())
    local sel = L.selected
    if sel and sel.remembered then win.okay:Disable() else win.okay:Enable() end
end

-- Pick a row (the Okay, Copy link and Report buttons act on it); on the page's list it opens the preview too.
function L.Select(r)
    if not r then return false end
    L.selected = r
    if r.net then L.Preview(r) elseif win and win.panel then win.panel:Hide() end
    L.Refresh()
    return true
end

-- Up / Down: the selection, keeping it in view; past the end of the page's list, its next (or previous) page.
function L.Move(dir)
    local n = #L.realms
    if n == 0 then return false end
    local at = 0
    for i, r in ipairs(L.realms) do if isSel(r) then at = i end end
    local to = at == 0 and (dir > 0 and 1 or n) or at + dir
    if to < 1 or to > n then
        local b = L.browse
        if L.Browsing() and b then L.Scroll(dir) end
        return false
    end
    if to <= L.offset then L.offset = to - 1 elseif to > L.offset + ROWS then L.offset = to - ROWS end
    L.Select(L.realms[to])
    return true
end

-- Up / Down buttons and the wheel: through the rows, then the page's next (or previous) page of the list.
function L.Scroll(dir)
    local b = L.browse
    if dir > 0 then
        if L.offset + ROWS < #L.realms then L.offset = L.offset + ROWS
        elseif L.Browsing() and b and (tonumber(b.page) or 1) < (tonumber(b.pages) or 1) then L.AskList((tonumber(b.page) or 1) + 1) end
    else
        if L.offset > 0 then L.offset = math.max(0, L.offset - ROWS)
        elseif L.Browsing() and b and (tonumber(b.page) or 1) > 1 then L.backPage = true; L.AskList((tonumber(b.page) or 1) - 1) end
    end
    L.Refresh()
end

-- Ask the page to reload the game on target ("wrath" or a relay realm id). False for anything else.
function L.Hop(target, name)
    if type(target) ~= "string" or not (target == "wrath" or (#target == 16 and target:match("^[a-z2-7]+$"))) then
        return false
    end
    WUUI_LEYLINE_HOP = target
    L.hopping = { target = target, name = name or brand().home, at = now() }
    if SendAddonMessage and UnitName then pcall(SendAddonMessage, PREFIX, "H:" .. target, "WHISPER", UnitName("player")) end
    if target == "wrath" then
        info(brand().back .. "... the game reloads there in a moment.")
    elseif L.Browsing() then   -- the page's fast hop: straight to that realm's characters
        local d = L.detail and L.detail.id == target and L.detail or nil
        local hk = d and (d.hk == "ready" or d.hk == "yes")
        info("Going to " .. (name or "that realm") .. "... the game reloads there in a moment. "
            .. (hk and "You sign in with your Hero Key." or "The first time, you make an account there (or sign in to yours)."))
    else
        info("Hopping to " .. (name or "that realm") .. "... the game reloads there in a moment. The first time, you "
            .. "make an account on that realm (or sign in to yours); " .. (W.OFFICIAL_NAME or "our realm") .. " uses your "
            .. (brand().on and "own" or "Wrath") .. " login.")
    end
    return true
end

-- "Host your own": the Host window (Host.lua), else the three steps and the website's page in the box to copy.
L.HOST_LINK = PUBLIC_BASE .. "/wrath/realms#host-your-own"
function L.HostYourOwn()
    if W.Host and W.Host.Show then   -- Host.lua: set up a realm, and "Your realm" for its GM
        if win then win:Hide() end
        W.Host.Show(1)
        return true
    end
    info("Host your own realm: 1. Get the Leyline launcher and Docker. 2. Open Host, pick a ruleset and your rules. "
        .. "3. Make it public: it shows up here. The link has the details.")
    copy(L.HOST_LINK)
end

function L.ReportLink(id) return PUBLIC_BASE .. "/wrath/realms/report?realm=" .. id end
function L.Report(r)
    r = r or L.selected
    if not r or r.id == "this" or r.sample or r.remembered then
        info("Pick a community realm in the list (click its row), then Report.")
        return false
    end
    info("Open this link to report " .. r.name .. " to Leyline staff (you sign in on the website).")
    copy(L.ReportLink(r.id))
    return true
end

-- The brand can arrive after the window was made: relabel its home button then.
if W.OnBrand then
    W.OnBrand(function(b)
        if win and win.back then win.back:SetText(b.back); win.back:SetWidth(170) end
    end)
end

function L.Back()
    if L.OnWrath() then info("You're on " .. brand().home .. " now."); return false end
    return L.Hop("wrath", brand().home)
end

function L.Join(r)
    local link = L.RowLink(r)
    if r.id == "this" then
        if L.JoinText(r) == "Join by link" then
            info("You're on " .. r.name .. " now. Other players join with the link above (or "
                .. "leyline://realm/" .. r.relayId .. " in the Leyline launcher).")
        else
            info("You're on " .. r.name .. " now. Share the link above to bring friends here.")
        end
    elseif not r.live then
        info(r.name .. " is offline right now. Its link is above: try again later.")
    else
        L.Hop(r.id, r.name)
    end
    copy(link)
end

-- Okay: join the picked realm (the page's list: through the preview, never a broken hop).
function L.Okay()
    local r = L.selected
    if not r then info("Pick a realm in the list first."); return false end
    if r.remembered then
        info(r.name .. " runs on this PC: start it from the Leyline launcher (" .. (r.kind == "den" and "Your Den" or "Single player") .. ").")
        return false
    end
    if r.net and not r.here then return L.PreviewJoin() end
    return L.Join(r)
end

-- Copy link: the picked realm's link in the box.
function L.CopySelected()
    local r = L.selected
    if not r then info("Pick a realm in the list, then Copy link."); return false end
    local link = L.RowLink(r)
    if not link then info(r.name .. " runs on this PC: it has no link to share."); return false end
    info("The link to " .. L.RowName(r) .. ". Ctrl+C copies it. Anyone with the link can play there in their browser.")
    copy(link)
    return link
end

-- The keyboard while the window is open: Enter = Okay, Esc = Cancel, Up / Down = the selection.
function L.OnKey(key)
    if win and not L.keys then return false end
    if key == "ENTER" then L.Okay(); return true end
    if key == "ESCAPE" then if win then win:Hide() end; return true end
    if key == "UP" then L.Move(-1); return true end
    if key == "DOWN" then L.Move(1); return true end
    return false
end

-- The window takes the keyboard only while it is the window in use: taken when it opens or is clicked; let go when
-- it closes (Esc, Cancel, the X, a loading screen), when another panel opens over it (Esc menu, map, character...),
-- in combat, and when it is hidden any other way (Alt-Z hides the whole interface). A focused box (search, link) lets
-- go of its focus too: a hidden box with the focus would keep every key, movement included.
function L.TakeKeys(on)
    on = on and true or false
    if on and InCombatLockdown and InCombatLockdown() then on = false end
    if on and not (win and win:IsVisible()) then on = false end
    L.keys = on
    if win and win.EnableKeyboard then win:EnableKeyboard(on) end
    if not on and win then
        for _, box in ipairs({ win.search, win.copy }) do
            if box and box.ClearFocus then box:ClearFocus() end
        end
    end
    return on
end

-- Typing a letter: it goes to the search box.
function L.OnChar(text)
    if not win or not L.keys or type(text) ~= "string" or not text:match("^[%w%s%-]$") then return false end
    win.search:SetFocus()
    win.search:SetText((win.search:GetText() or "") .. text)
    L.SetSearch(win.search:GetText())
    return true
end

local function tooltip(row)
    local r = row.realm
    if not r or not GameTooltip then return end
    local name, rules, state = L.Texts(r)
    GameTooltip:SetOwner(row, "ANCHOR_RIGHT")
    GameTooltip:SetText(name)
    GameTooltip:AddLine(rules, 1, 1, 1)
    GameTooltip:AddLine(state, 1, 1, 1)
    local friends = L.FriendsOn(r)
    if #friends > 0 then GameTooltip:AddLine("Friends here: " .. table.concat(friends, ", "), 0.4, 0.8, 1) end
    GameTooltip:AddLine("Double-click to join.", 0.6, 0.6, 0.6)
    GameTooltip:Show()
end

local function buildRow(i)
    local row = CreateFrame("Button", nil, win.list)
    row:SetWidth(win.listW); row:SetHeight(ROW_H)
    row:SetPoint("TOPLEFT", 0, -(i - 1) * ROW_H)
    row:SetHighlightTexture(TEX.hover, "ADD")
    row.sel = row:CreateTexture(nil, "BACKGROUND")
    row.sel:SetTexture(TEX.selected)
    row.sel:SetVertexColor(1, 0.82, 0, 0.6)
    row.sel:SetAllPoints()
    row.sel:Hide()
    local x = 0
    local fields = {}
    for c, col in ipairs(COLS) do
        if c > 1 then
            local fs = row:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
            fs:SetPoint("LEFT", x + 6, 0)
            fs:SetWidth(col[3] - 10); fs:SetHeight(ROW_H)
            fs:SetJustifyH(c == 4 and "CENTER" or "LEFT")
            fields[col[1]] = fs
        end
        x = x + col[3]
    end
    row.name, row.type, row.chars, row.pop, row.players = fields.name, fields.type, fields.chars, fields.pop, fields.players
    row.star = CreateFrame("Button", nil, row)
    row.star:SetWidth(14); row.star:SetHeight(14)
    row.star:SetPoint("LEFT", 4, 0)
    row.star.icon = row.star:CreateTexture(nil, "ARTWORK")
    row.star.icon:SetTexture(TEX.star)
    row.star.icon:SetAllPoints()
    row.star:SetScript("OnClick", function(self) L.ToggleFav(self:GetParent().realm) end)
    row:SetScript("OnClick", function(self) if self.realm then L.Select(self.realm) end end)
    on(row, "OnDoubleClick", function(self) if self.realm then L.Select(self.realm); L.Okay() end end)
    row:SetScript("OnEnter", tooltip)
    row:SetScript("OnLeave", function() if GameTooltip then GameTooltip:Hide() end end)
    row:Hide()
    return row
end

-- A column header, as the game's own lists (the Who list's WhoFrameColumnHeaderTemplate), with the sort arrow.
local function buildHeader(i, col, x, y)
    local name = "WrathUnleashedLeylineColumn" .. i
    local h = make("Button", name, win, "WhoFrameColumnHeaderTemplate")
    h:SetPoint("TOPLEFT", x, y)
    h:SetHeight(24)
    if WhoFrameColumn_SetWidth and h.GetName and h:GetName() and _G[name .. "Middle"] then
        pcall(WhoFrameColumn_SetWidth, h, col[3])
    else
        h:SetWidth(col[3])
        if not _G[name .. "Middle"] then   -- no template: the same art, drawn here
            local l = h:CreateTexture(nil, "BACKGROUND"); l:SetTexture(TEX.columnTabs)
            l:SetWidth(5); l:SetHeight(24); l:SetPoint("TOPLEFT"); l:SetTexCoord(0, 0.078125, 0, 0.75)
            local r = h:CreateTexture(nil, "BACKGROUND"); r:SetTexture(TEX.columnTabs)
            r:SetWidth(4); r:SetHeight(24); r:SetPoint("TOPRIGHT"); r:SetTexCoord(0.90625, 0.96875, 0, 0.75)
            local m = h:CreateTexture(nil, "BACKGROUND"); m:SetTexture(TEX.columnTabs)
            m:SetHeight(24); m:SetPoint("LEFT", l, "RIGHT"); m:SetPoint("RIGHT", r, "LEFT"); m:SetTexCoord(0.078125, 0.90625, 0, 0.75)
            h:SetHighlightTexture("Interface\\PaperDollInfoFrame\\UI-Character-Tab-Highlight", "ADD")
            local fs = h:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
            fs:SetPoint("LEFT", 8, 0)
            if h.SetFontString then h:SetFontString(fs) end
        end
    end
    h:SetText(col[2])
    h.key = col[1]
    h.arrow = h:CreateTexture(nil, "OVERLAY")
    h.arrow:SetTexture(TEX.sortArrow)
    h.arrow:SetWidth(9); h.arrow:SetHeight(8)
    h.arrow:SetPoint("RIGHT", -6, 0)
    h.arrow:Hide()
    h:SetScript("OnClick", function(self)
        if self.key then L.SortBy(self.key) end
        if PlaySound then pcall(PlaySound, "igMainMenuOptionCheckBoxOn") end
    end)
    if not col[1] then h:Disable() end
    return h
end

-- A realm category tab along the bottom of the list (the character sheet's tabs, CharacterFrameTabButtonTemplate).
local function buildTab(i, t)
    local tab = make("Button", "WrathUnleashedLeylineFrameTab" .. i, win, "CharacterFrameTabButtonTemplate")
    if tab.SetID then tab:SetID(i) end
    tab:SetText(t[2])
    tab.key = t[1]
    if PanelTemplates_TabResize then pcall(PanelTemplates_TabResize, tab, 0) else tab:SetWidth(30 + #t[2] * 6); tab:SetHeight(32) end
    if i == 1 then tab:SetPoint("TOPLEFT", win.listBox, "BOTTOMLEFT", 6, 2)
    else tab:SetPoint("LEFT", win.tabs[i - 1], "RIGHT", -15, 0) end
    tab:SetScript("OnClick", function(self)
        L.SetTab(self.key)
        if PlaySound then pcall(PlaySound, "igCharacterInfoTab") end
    end)
    -- the template's own tooltip names a character sheet key binding: ours says what the tab holds
    tab:SetScript("OnEnter", nil)
    tab:SetScript("OnLeave", nil)
    tab:SetScript("OnShow", function(self) if PanelTemplates_TabResize then pcall(PanelTemplates_TabResize, self, 0) end end)
    return tab
end

local function buildFriends(parent, y)
    local box = CreateFrame("Frame", nil, parent)
    box:SetPoint("TOPLEFT", 20, y); box:SetPoint("TOPRIGHT", -20, y)
    box:SetHeight(40)
    if box.SetBackdrop then
        box:SetBackdrop({ bgFile = TEX.insetBg, edgeFile = TEX.insetEdge, tile = true, tileSize = 16, edgeSize = 16,
            insets = { left = 4, right = 4, top = 4, bottom = 4 } })
        box:SetBackdropColor(0, 0, 0, 0.5)
    end
    label(box, "Friends across realms", "GameFontNormalSmall", "TOPLEFT", 10, -8)
    box.soon = label(box, "Coming", "GameFontDisableSmall", "TOPLEFT", 150, -8)
    box.text = label(box, "Your Leyline friends on every realm in one list: who's online, where they are, "
        .. "and a click to join them.", "GameFontHighlightSmall", "TOPLEFT", 10, -22)
    box.text:SetWidth(WIN_W - 180); box.text:SetHeight(12)
    box.list = CreateFrame("Frame", nil, box)   -- the rows go here when friends arrive
    box.list:SetPoint("TOPLEFT", 12, -40); box.list:SetPoint("BOTTOMRIGHT", -12, 0)
    box.add = button(box, "Add a friend", 110, function() end, "RIGHT", -8, 0)
    box.add:Disable()
    if W.Social and W.Social.Adopt then W.Social.Adopt(box) end   -- Leyline friends (Social.lua)
    return box
end

local function openFilters(anchor)
    local menu = win.filterMenu
    if UIDropDownMenu_Initialize and ToggleDropDownMenu and UIDropDownMenu_AddButton then
        UIDropDownMenu_Initialize(menu, function(_, level)
            level = level or 1
            for _, it in ipairs(L.MenuItems(level, UIDROPDOWNMENU_MENU_VALUE)) do
                local item = UIDropDownMenu_CreateInfo and UIDropDownMenu_CreateInfo() or {}
                for k, v in pairs(it) do item[k] = v end
                UIDropDownMenu_AddButton(item, level)
            end
        end, "MENU")
        ToggleDropDownMenu(1, nil, menu, anchor, 0, 0)
        return true
    end
    -- no dropdown in this client: the first filter that has values steps on, as before
    L.Cycle("tag")
    return false
end
L.OpenFilters = function() if win then return openFilters(win.filterBtn) end end

local function build()
    win = CreateFrame("Frame", "WrathUnleashedLeylineFrame", UIParent)
    win:SetWidth(WIN_W); win:SetHeight(WIN_H)
    win:SetPoint("CENTER")
    win:SetFrameStrata("DIALOG")
    win:SetToplevel(true)
    win:EnableMouse(true)
    win:SetMovable(true)
    win:RegisterForDrag("LeftButton")
    win:SetScript("OnDragStart", win.StartMoving)
    win:SetScript("OnDragStop", win.StopMovingOrSizing)
    dialog(win)
    win.title = header(win, "Realm Selection")
    local close = make("Button", nil, win, "UIPanelCloseButton")
    close:SetPoint("TOPRIGHT", -4, -4)
    close:SetScript("OnClick", function() win:Hide() end)
    tinsert(UISpecialFrames, "WrathUnleashedLeylineFrame")   -- Esc closes it
    win:EnableKeyboard(false)   -- taken on show (L.TakeKeys), never left on while hidden
    on(win, "OnMouseDown", function() L.TakeKeys(true) end)   -- a click on the window takes the keys back
    L.charScript = on(win, "OnChar", function(_, text) L.OnChar(text) end)
    on(win, "OnKeyDown", function(_, key)
        -- without OnChar, a letter key itself goes to the search
        if not L.OnKey(key) and not L.charScript and type(key) == "string" and #key == 1 then L.OnChar(key:lower()) end
    end)

    -- the top line: search, Filters, Refresh, Report; when the list came
    local search = make("EditBox", "WrathUnleashedLeylineSearch", win, "InputBoxTemplate")
    search:SetWidth(190); search:SetHeight(20)
    search:SetPoint("TOPLEFT", 30, -34)
    search:SetAutoFocus(false)
    search:SetMaxLetters(32)
    search:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
    search:SetScript("OnEnterPressed", function(self)
        self:ClearFocus()
        if L.Browsing() then L.searchAt = now() - SEARCH_WAIT end
    end)
    search:SetScript("OnEditFocusGained", function(self) self.focused = true; win.searchHint:Hide() end)
    search:SetScript("OnEditFocusLost", function(self) self.focused = nil; L.Refresh() end)
    on(search, "OnArrowPressed", function(_, key) if key == "UP" then L.Move(-1) elseif key == "DOWN" then L.Move(1) end end)
    search:SetScript("OnTextChanged", function(self) L.SetSearch(self:GetText() or "") end)
    win.search = search
    win.searchHint = label(search, "Search realms", "GameFontDisableSmall", "LEFT", search, "LEFT", 2, 0)
    win.filterBtn = button(win, "Filters", 84, function(self) openFilters(self) end, "LEFT", search, "RIGHT", 8, 0)
    win.filterMenu = make("Frame", "WrathUnleashedLeylineFilterMenu", win, "UIDropDownMenuTemplate")
    win.filterMenu:Hide()
    win.refresh = button(win, "Refresh", 80, function()
        if L.Browsing() then L.AskList(L.netPage or 1) else L.Request(true) end
        L.Refresh()
    end, "LEFT", win.filterBtn, "RIGHT", 4, 0)
    win.report = button(win, "Report", 70, function() L.Report() end, "LEFT", win.refresh, "RIGHT", 4, 0)
    win.updated = label(win, "", "GameFontDisableSmall", "TOPRIGHT", -24, -38)
    win.updated:SetJustifyH("RIGHT")

    win.status = label(win, L.status, "GameFontHighlightSmall", "TOPLEFT", 24, -60)
    win.status:SetWidth(WIN_W - 190); win.status:SetHeight(12)
    win.count = label(win, "", "GameFontHighlightSmall", "TOPRIGHT", -24, -60)
    win.count:SetJustifyH("RIGHT")

    -- the column headers and the list in its inset box, the scroll arrows beside it
    local listX, headY = 20, -76
    win.listW = 0
    for _, col in ipairs(COLS) do win.listW = win.listW + col[3] end
    win.headers = {}
    local x = listX
    for i, col in ipairs(COLS) do
        win.headers[i] = buildHeader(i, col, x, headY)
        x = x + col[3]
    end
    win.listBox = CreateFrame("Frame", nil, win)
    win.listBox:SetPoint("TOPLEFT", listX - 4, headY - 24)
    win.listBox:SetWidth(win.listW + 8); win.listBox:SetHeight(ROWS * ROW_H + 10)
    if win.listBox.SetBackdrop then
        win.listBox:SetBackdrop({ bgFile = TEX.insetBg, edgeFile = TEX.insetEdge, tile = true, tileSize = 16, edgeSize = 16,
            insets = { left = 4, right = 4, top = 4, bottom = 4 } })
        win.listBox:SetBackdropColor(0, 0, 0, 0.6)
    end
    win.list = CreateFrame("Frame", nil, win.listBox)
    win.list:SetWidth(win.listW); win.list:SetHeight(ROWS * ROW_H)
    win.list:SetPoint("TOPLEFT", 4, -5)
    win.list:EnableMouseWheel(true)
    win.list:SetScript("OnMouseWheel", function(_, delta)
        if L.Browsing() then L.Scroll(-delta) else L.offset = math.max(0, L.offset - delta); L.Refresh() end
    end)
    win.rows = {}
    for i = 1, ROWS do win.rows[i] = buildRow(i) end
    win.empty = label(win.list, "", "GameFontDisable", "CENTER", 0, 0)
    win.empty:SetJustifyH("CENTER"); win.empty:SetWidth(win.listW - 60)
    win.up = make("Button", nil, win, "UIPanelScrollUpButtonTemplate")
    win.up:SetPoint("TOPLEFT", win.listBox, "TOPRIGHT", 2, -2)
    win.up:SetScript("OnClick", function() L.Scroll(-1) end)
    win.down = make("Button", nil, win, "UIPanelScrollDownButtonTemplate")
    win.down:SetPoint("BOTTOMLEFT", win.listBox, "BOTTOMRIGHT", 2, 2)
    win.down:SetScript("OnClick", function() L.Scroll(1) end)

    -- the realm categories along the bottom of the list
    win.tabs = {}
    for i, t in ipairs(TABS) do win.tabs[i] = buildTab(i, t) end
    win.numTabs = #TABS
    if PanelTemplates_SetNumTabs then pcall(PanelTemplates_SetNumTabs, win, #TABS) end

    -- the link box and what the window says
    local y = headY - 24 - (ROWS * ROW_H + 10) - 40
    label(win, "Link:", "GameFontNormalSmall", "TOPLEFT", 24, y - 4)
    local box = make("EditBox", nil, win, "InputBoxTemplate")
    box:SetWidth(WIN_W - 100); box:SetHeight(20)
    box:SetPoint("TOPLEFT", 66, y)
    box:SetAutoFocus(false)
    box:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
    box:SetScript("OnTextChanged", function(self) if self.fixed and self:GetText() ~= self.fixed then self:SetText(self.fixed) end end)
    win.copy = box
    win.info = label(win, "Pick a realm and press Okay to go there. Copy link puts its link in the box; Ctrl+C copies it.",
        "GameFontHighlightSmall", "TOPLEFT", 24, y - 26)
    win.info:SetWidth(WIN_W - 48)

    win.friends = buildFriends(win, y - 56)

    -- Your game folder (GameFolder.lua): streamed places, or the whole world from the player's own folder. Its
    -- button is the game's own here (GameFolder.lua draws a Leyline one when Style.lua is there).
    if W.GameFolder then
        local style = W.Style
        W.Style = nil
        local ok, row = pcall(W.GameFolder.Attach, win, y - 104, WIN_W)
        W.Style = style
        if ok then win.folder = row; if row and row.button then skin(row.button) end end
    end

    -- the original's buttons, and Leyline's
    win.cancel = button(win, "Cancel", 90, function() win:Hide() end, "BOTTOMRIGHT", -20, 18)
    win.okay = button(win, "Okay", 90, function() L.Okay() end, "RIGHT", win.cancel, "LEFT", -6, 0)
    win.copyBtn = button(win, "Copy link", 90, function() L.CopySelected() end, "RIGHT", win.okay, "LEFT", -6, 0)
    win.back = button(win, brand().back, brand().on and 170 or 130, function() L.Back() end, "BOTTOMLEFT", 20, 18)
    win.host = button(win, "Host your own", 120, function() L.HostYourOwn() end, "LEFT", win.back, "RIGHT", 6, 0)

    -- No answer from the realm in time: say so (and let Refresh try again).
    local tick = 0
    win:SetScript("OnUpdate", function(_, elapsed)
        if L.browseAt and now() - L.browseAt > ANSWER_WAIT then   -- the page didn't answer: the realm's own list
            L.browseAt = nil
            WUUI_LEYLINE_BROWSE = nil
            L.Request(true)
            L.Refresh()
        end
        if L.searchAt and now() - L.searchAt >= SEARCH_WAIT then
            L.searchAt = nil
            if L.Browsing() then L.AskList(1) end
        end
        if L.waiting and L.askedAt and now() - L.askedAt > ANSWER_WAIT then
            L.waiting, L.failed, L.incoming = nil, true, nil
            setStatus(UNAVAILABLE)
            L.View()
            L.Refresh()
        end
        if L.hopping and now() - L.hopping.at > HOP_WAIT then   -- no page took it (the desktop client)
            local h = L.hopping
            L.hopping, WUUI_LEYLINE_HOP = nil, nil
            info(h.target == "wrath" and "Hopping works in the browser game. Open " .. L.Link("this") .. " to play on " .. brand().home .. "."
                or "Hopping works in the browser game. Open the link above to play on " .. h.name .. ".")
        end
        tick = tick + (tonumber(elapsed) or 0)
        if tick >= 1 then tick = 0; win.updated:SetText(L.UpdatedText()) end
    end)
    win:SetScript("OnShow", function()
        L.TakeKeys(true)
        if PlaySound then pcall(PlaySound, "igCharacterInfoOpen") end
        if L.Browsing() then L.AskList(L.netPage or 1) else L.Request(false) end
        L.Refresh()
    end)
    win:SetScript("OnHide", function()
        L.TakeKeys(false)
        if win.panel then win.panel:Hide() end
        if CloseDropDownMenus then pcall(CloseDropDownMenus) end
        if PlaySound then pcall(PlaySound, "igCharacterInfoClose") end
    end)
    win:Hide()
end

-- The preview, beside the window (the window moves left to make room): the same dialog box.
local function buildPanel()
    local p = CreateFrame("Frame", "WrathUnleashedLeylinePreview", win)
    p:SetWidth(PANEL_W); p:SetHeight(WIN_H)
    p:SetPoint("TOPLEFT", win, "TOPRIGHT", -4, 0)
    p:EnableMouse(true)
    dialog(p)
    header(p, "Realm details", 260)
    local close = make("Button", nil, p, "UIPanelCloseButton")
    close:SetPoint("TOPRIGHT", -4, -4)
    close:SetScript("OnClick", function() p:Hide() end)
    local wide = PANEL_W - 44
    p.title = label(p, "", "GameFontNormalLarge", "TOPLEFT", 22, -40); p.title:SetWidth(wide)
    p.chips = label(p, "", "GameFontHighlightSmall", "TOPLEFT", p.title, "BOTTOMLEFT", 0, -6); p.chips:SetWidth(wide)
    p.players = label(p, "", "GameFontHighlight", "TOPLEFT", p.chips, "BOTTOMLEFT", 0, -8); p.players:SetWidth(wide)
    p.up = label(p, "", "GameFontHighlightSmall", "TOPLEFT", p.players, "BOTTOMLEFT", 0, -3); p.up:SetWidth(wide)
    p.desc = label(p, "", "GameFontHighlightSmall", "TOPLEFT", p.up, "BOTTOMLEFT", 0, -8); p.desc:SetWidth(wide)
    local function section(name, anchor)
        local h = label(p, name, "GameFontNormal", "TOPLEFT", anchor, "BOTTOMLEFT", 0, -12)
        local b = label(p, "", "GameFontHighlightSmall", "TOPLEFT", h, "BOTTOMLEFT", 0, -4)
        b:SetWidth(wide)
        return h, b
    end
    p.rulesH, p.rules = section("House rules", p.desc)
    p.whoH, p.who = section("Who's on", p.rules)
    p.modsH, p.mods = section("Mods", p.who)
    p.why = label(p, "", "GameFontHighlightSmall", "BOTTOMLEFT", 22, 74); p.why:SetWidth(wide)
    p.info = label(p, "", "GameFontHighlightSmall", "BOTTOMLEFT", 22, 50); p.info:SetWidth(wide)
    p.join = button(p, "Join", 150, function() L.PreviewJoin() end, "BOTTOMLEFT", 20, 18)
    p.copyBtn = button(p, "Copy link", 90, function()
        if L.selected then copy(L.RowLink(L.selected)) end
    end, "BOTTOMRIGHT", -20, 18)
    p:SetScript("OnShow", function() win:ClearAllPoints(); win:SetPoint("CENTER", -PANEL_W / 2, 0) end)
    p:SetScript("OnHide", function() win:ClearAllPoints(); win:SetPoint("CENTER") end)
    p:Hide()
    win.panel = p
    return p
end

function L.panelInfo(text) if win and win.panel then win.panel.info:SetText(text or "") end end

function L.ShowPreview(r)
    if not win then return end
    local p = win.panel or buildPanel()
    local t = L.PreviewTexts(r, L.detail)
    p.title:SetText(t.title)
    p.chips:SetText(t.chips)
    p.players:SetText(t.players)
    p.up:SetText(t.up)
    p.desc:SetText(t.desc)
    p.rules:SetText(t.rules)
    p.who:SetText(t.who)
    p.mods:SetText(t.mods)
    p.why:SetText(t.why ~= "" and ((t.verdict == "launcher" and "|cfff0d389" or "") .. t.why .. (t.verdict == "launcher" and "|r" or "")) or "")
    p.join:SetText(t.join)
    if t.join == "Offline" then p.join:Disable() else p.join:Enable() end
    p.info:SetText("")
    p:Show()
end

function W.ToggleLeyline()
    if not win then build() end
    if win:IsShown() then win:Hide() else win:Show() end
end

-- Esc > Realm List (ModsPanel.lua's game menu button): always shows the window.
function W.ShowRealmList()
    if not win then build() end
    win:Show()
end

-- Another panel opening over the window takes the keyboard back for the game (it goes through ShowUIPanel, or is
-- the Esc menu or the map).
function L.OtherPanel(frame)
    if win and frame ~= win and win:IsShown() and L.keys then L.TakeKeys(false) end
end
if hooksecurefunc and ShowUIPanel then pcall(hooksecurefunc, "ShowUIPanel", function(frame) L.OtherPanel(frame) end) end
for _, name in ipairs({ "GameMenuFrame", "WorldMapFrame", "InterfaceOptionsFrame", "VideoOptionsFrame", "HelpFrame" }) do
    local f = rawget(_G, name)
    if type(f) == "table" and f.HookScript then pcall(f.HookScript, f, "OnShow", function(self) L.OtherPanel(self) end) end
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:RegisterEvent("PLAYER_LEAVING_WORLD")    -- a loading screen (zoning, a hop, logging out): the window closes
listener:RegisterEvent("PLAYER_REGEN_DISABLED")   -- combat: the keys go back to the game at once
listener:SetScript("OnEvent", function(_, event, prefix, msg, channel, sender)
    if event == "PLAYER_LEAVING_WORLD" then
        if win and win:IsShown() then win:Hide() end
        L.TakeKeys(false)
    elseif event == "PLAYER_REGEN_DISABLED" then
        L.TakeKeys(false)
    else
        L.OnMessage(prefix, msg, channel, sender)
    end
end)

SLASH_WULEYLINE1, SLASH_WULEYLINE2 = "/leyline", "/realms"
SlashCmdList["WULEYLINE"] = function() W.ToggleLeyline() end

W.Leyline = L
L.Window, L.PUBLIC_BASE, L.PREFIX, L.HOP_WAIT = function() return win end, PUBLIC_BASE, PREFIX, HOP_WAIT
L.ROWS, L.PANEL_W, L.TABS, L.COLS, L.TEX = ROWS, PANEL_W, TABS, COLS, TEX
