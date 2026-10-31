-- Leyline (3.3.5a API): the in-game window of the Leyline network, in the Mods & Workshop panel's look.
--   Realms:  every Leyline realm (name, ruleset, players, online/offline), this one first. "Copy link" puts the
--            realm's link (PUBLIC_BASE/r/<id>) in the box to copy; "Join" hops there: the browser game reloads
--            on that realm. The home button (always at the bottom: "Back to Wrath", or "Back to Wrath Unleashed" with the Leyline brand on) hops home.
--   Friends across realms: the place for the global friends list (coming).
--   Your game folder: "Streamed: Dalaran, capitals, starting areas" or "Whole world: your game folder", and
--            "Use my game folder", which asks the page for the folder picker (GameFolder.lua).
-- Opened by Esc > Realm List, /leyline and /realms. "Host your own" opens the Host window (Host.lua): set up a realm,
-- and "Your realm" for its GM.
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
--                                                                    V verified, N new, S sample hangout, E region
--                                                                    estimated)
--                      N:<realms>:<players>:<bots>                   the network line
--                      E:<count>                                     the list is complete
--                      U:<text>                                      no list (e.g. "Realm list unavailable")
--                      S:<id>:<live 0|1>                             this realm's own relay listing (left out of the
--                                                                    R lines); before E
--
-- The order (the same on every Leyline surface): this realm first when it is a community realm; featured realms;
-- the Leyline Sample Hangout (our own realm, temporary); every other live realm by the network's fair score;
-- offline ones. Filters (build, rules tag, region, language, has players now) narrow the community realms; the
-- sample stays. "Host your own" and "Report" put the website's link in the box to copy.
--   client -> server   H:<id|wrath>                                  hop there
--   server -> client   H:<id|wrath>                                  echoed back
--
-- Hopping: the addon can't navigate, so it asks the page. Join sends H:<id> to the realm, which echoes it; the
-- browser page (stream/www-files/wrath-page.js) watches CHAT_MSG_ADDON through the game bridge and reloads the
-- game at /wrath?go=r:<id>. It also sets the global WUUI_LEYLINE_HOP, which the page reads (and clears) through
-- the bridge, so a hop works on a realm whose server doesn't echo (a Beacon without ov_leyline.lua). The desktop
-- client has no page: after HOP_WAIT seconds the window says to open the link instead.
--
-- The realm browser (2026-10-31): in the browser game the page hands the window the whole network's list, paged, and
-- one realm's preview (stream/www-files/wrath-page.js leyline-browse): WUUI_LEYLINE_ASK = "list|<page>|<filters>|
-- <search>" or "preview|<id>" or "launch|<id>", answered in WUUI_LEYLINE_NET / WUUI_LEYLINE_PREVIEW and a call to
-- WUUI_LEYLINE_ONNET. The page says it can (WUUI_LEYLINE_BROWSE = 1); else the window keeps the realm's list above.
-- Then: a search box and the filters Plays in browser, Own files, Has players, Version; a row's View opens the
-- preview beside the window (the host's words and house rules, who's on when the realm shares it, its mods,
-- required and optional, the ones on your side marked, its version chip, uptime, Hero Key sign-in) and its Join
-- hops. A realm that needs another game version is never hopped to: Join hands it to the Leyline launcher
-- (leyline://realm/<id>, the page shows the button). The preview also works on the realm's list (fewer details).

local W = WrathUnleashedUI
-- The public site. Links are PUBLIC_BASE .. "/r/<id>"; a future Leyline domain is this one line.
local PUBLIC_BASE = "https://play.openvanilla.app"
local THIS_REALM_PATH = "/wrath"
local PREFIX = "WULY"
local HOME_REALM = "WrathUnleashed"       -- GetRealmName() on Wrath itself (W.Brand.home while the brand is on)
-- The Leyline brand switch (WrathUnleashedUI.lua W.Brand, from the realm at login): the home realm's name.
local function brand() return W.Brand or { on = false, home = "Wrath", back = "Back to Wrath" } end
local HOP_WAIT = 8                        -- seconds: still here after this = no page took the hop
local WIN_W, WIN_H, ROW_H, ROWS = 560, 632, 34, 7
local SAMPLE_NAME = "Leyline Sample Hangout"
local SAMPLE_NOTE = "temporary"
local LIST_TOP = -98
local ASK_EVERY, ANSWER_WAIT = 30, 6     -- seconds: refresh at most this often; no answer after this = unavailable
local UNAVAILABLE = "Realm list unavailable"

local RULESETS = {
    main = "Level 1 or boost to 80", brawl = "Brawl: everyone 80", progression = "Progression",
    casual = "Casual", dalaran = "Dalaran hangout", custom = "Custom rules",
}

local TAGS = { blizzlike = "Original rules", boosted = "Boosted", custom = "Custom", pvp = "PvP", rp = "RP",
    hangout = "Hangout", bots = "Bots" }
local TAG_ORDER = { "blizzlike", "boosted", "custom", "pvp", "rp", "hangout", "bots" }
local BUILDS = { [5875] = "1.12.1", [8606] = "2.4.3", [12340] = "3.3.5a", [15595] = "4.3.4", [18414] = "5.4.8" }
local REGIONS = { ["na-east"] = "NA east", ["na-west"] = "NA west", sa = "S. America", eu = "Europe", africa = "Africa",
    me = "Middle East", asia = "Asia", oce = "Oceania" }

local win
-- state and the functions the tests reach (W.Leyline). all: every realm as sent; realms: what the window shows.
local L = { realms = {}, all = {}, status = "", offset = 0, filter = {} }

local function label(parent, text, font, ...)
    local fs = parent:CreateFontString(nil, "OVERLAY", font or "GameFontHighlight")
    fs:SetPoint(...)
    fs:SetJustifyH("LEFT")
    fs:SetText(text)
    return fs
end

local function button(parent, text, w, onClick, ...)
    if W.Style then local b = W.Style.Button(parent, text, w, onClick, nil, ...); b:SetHeight(22); return b end
    local b = CreateFrame("Button", nil, parent, "UIPanelButtonTemplate")
    b:SetWidth(w); b:SetHeight(22)
    b:SetText(text)
    b:SetPoint(...)
    b:SetScript("OnClick", onClick)
    return b
end

local function now() return (GetTime and GetTime()) or 0 end

function L.Link(id)
    if id == "this" then return PUBLIC_BASE .. THIS_REALM_PATH end
    return PUBLIC_BASE .. "/r/" .. id
end

-- The browser game's page publishes WUUI_GAMEFOLDER = { page = true, ... } (wrath-page.js); the desktop client has none.
function L.HasPage() return WUUI_PAGE == true or (type(WUUI_GAMEFOLDER) == "table" and WUUI_GAMEFOLDER.page == true) end

-- A row's link: this realm's is the /wrath page in the browser, its relay link in the desktop client when it is listed.
function L.RowLink(r)
    if r.id == "this" and r.relayId and not L.HasPage() then return L.Link(r.relayId) end
    return L.Link(r.id)
end

-- The Join button's text: "You're here" on this realm's row; "Join through the relay" only where that is how this
-- player would come back in (this realm listed on the relay, live, and no browser page: the desktop client), else Join.
function L.JoinText(r)
    if r.net then return r.here and "You're here" or "View" end
    if r.id ~= "this" then return "Join" end
    if r.relayId and r.relayLive and not L.HasPage() then return "Join by link" end
    return "You're here"
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

local function realmLine(rest)
    local id, live, players, bots, ruleset, name = rest:match("^([%w]+):([01]):([%d%-]+):([%d%-]+):(%a+):(.*)$")
    if not id or not (id == "this" or id:match("^[a-z2-7]+$")) then return nil end
    name = name:gsub("|", ""):sub(1, 40)
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
        sample = flags:find("S") ~= nil, est = flags:find("E") ~= nil, score = tonumber(score) or 0,
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
    local function k(x) return (x:lower():gsub("[^%w]", "")) end
    return k(a) ~= "" and k(a) == k(b)
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

function L.View()
    local this, sample, featured, rest, offline = nil, nil, {}, {}, {}
    for _, r in ipairs(L.all) do if r.id == "this" then this = r end end
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
    L.realms = out
    return out
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

local function networkLine(n)
    return string.format("%d realm%s · %d player%s online", n.realms, n.realms == 1 and "" or "s", n.players,
        n.players == 1 and "" or "s") .. (n.bots > 0 and (" · " .. n.bots .. " bots") or "")
end
L.NetworkLine = networkLine

function L.OnMessage(prefix, msg, channel, sender)
    if prefix ~= PREFIX or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local kind, rest = msg:match("^(%u):(.*)$")
    if kind == "B" then
        L.incoming, L.network, L.selfId, L.selfLive = {}, nil, nil, nil
        L.age = tonumber(rest:match(":(%d+)$") or "")
    elseif kind == "M" and L.incoming then
        local id, m = metaLine(rest)
        local last = L.incoming[#L.incoming]
        if id and last and last.id == id then
            for k, v in pairs(m) do last[k] = v end
        end
    elseif kind == "S" and L.incoming then
        local id, live = rest:match("^([a-z2-7]+):([01])$")
        if id and #id == 16 then L.selfId, L.selfLive = id, live == "1" end
    elseif kind == "N" and L.incoming then
        local n, p, b = rest:match("^(%d+):(%d+):(%d+)$")
        if n then L.network = { realms = tonumber(n), players = tonumber(p), bots = tonumber(b) } end
    elseif kind == "R" and L.incoming then
        local r = realmLine(rest)
        if r and #L.incoming < 100 then L.incoming[#L.incoming + 1] = r end
    elseif kind == "E" and L.incoming then
        L.all, L.incoming, L.waiting = L.incoming, nil, nil
        L.View()
        local n = 0
        for _, r in ipairs(L.realms) do if r.id ~= "this" then n = n + 1 end end
        local age = L.age and L.age >= 120 and string.format(" (updated %d min ago)", math.floor(L.age / 60)) or ""
        if L.network then
            setStatus(networkLine(L.network) .. "." .. age)
        else
            setStatus((n == 0 and "No other realms are open right now." or (n == 1 and "1 other realm" or (n .. " other realms")) .. " on Leyline.") .. age)
        end
        L.Refresh()
    elseif kind == "H" and L.hopping and rest == L.hopping.target then
        L.hopping.echoed = true   -- the page reloads the game on it now
    elseif kind == "U" then
        L.incoming, L.waiting, L.failed = nil, nil, true
        L.realms, L.all = {}, {}
        setStatus(rest ~= "" and rest:gsub("|", "") or UNAVAILABLE)
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
        mods = tonumber(r.mods) or 0, desc = type(r.desc) == "string" and r.desc or "", ruleset = "custom", net = true }
end

function L.OnNet(kind)
    if kind == "list" then
        local n = WUUI_LEYLINE_NET
        if type(n) ~= "table" then return end
        L.browseAt = nil
        if type(n.err) == "string" then
            L.browse, L.realms = nil, {}
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
        L.realms, L.offset = out, L.backPage and math.max(0, #out - ROWS) or 0
        L.backPage = nil
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
        for _, r in ipairs(L.realms) do if r.build then seen[r.build] = true end end
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

local function players(r)
    if not r.live then return "" end
    if not r.players then return "" end
    return r.players .. " playing" .. ((r.bots and r.bots > 0) and (" +" .. r.bots .. " bots") or "")
end

local function copy(text)
    local box = win.copy
    box.fixed = text; box:SetText(text); box:SetFocus(); box:HighlightText()
end
L.Copy = copy

-- The row's texts: the name (with the sample's label and the Featured / Verified / New badges), the rules line
-- (ruleset, then tags, build, region, language when the realm has them) and the state (with the uptime badge).
function L.Texts(r)
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
    local name = r.name
    if r.sample then name = (brand().on and brand().home or SAMPLE_NAME) .. " · " .. SAMPLE_NOTE end
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

function L.Refresh()
    if not win then return end
    if win.filters then
        local f = L.filter
        win.filters.build:SetText("Build: " .. (f.build and BUILDS[f.build] or "any"))
        win.filters.tag:SetText("Rules: " .. (f.tag and TAGS[f.tag] or "any"))
        win.filters.region:SetText("Region: " .. (f.region and REGIONS[f.region] or "any"))
        win.filters.lang:SetText("Lang: " .. (f.lang or "any"))
        win.filters.now:SetText(f.now and "Has players" or "Everyone")
        local browsing = L.Browsing()
        for _, b in pairs(win.filters) do if browsing then b:Hide() else b:Show() end end
        if win.bfilters then
            local bf = L.bfilter
            win.bfilters.browser:SetText(bf.browser and "In browser: on" or "In browser")
            win.bfilters.own:SetText(bf.own and "Own files: on" or "Own files")
            win.bfilters.players:SetText(bf.players and "Has players" or "Everyone")
            win.bfilters.build:SetText("Version: " .. (bf.build and BUILDS[bf.build] or "any"))
            for _, b in pairs(win.bfilters) do if browsing then b:Show() else b:Hide() end end
            if browsing then win.search:Show(); win.searchHint:Show() else win.search:Hide(); win.searchHint:Hide() end
            if (L.query or "") ~= "" then win.searchHint:Hide() end
        end
    end
    local n = #L.realms
    if L.offset > math.max(0, n - ROWS) then L.offset = math.max(0, n - ROWS) end
    for i, row in ipairs(win.rows) do
        local r = L.realms[L.offset + i]
        row.realm = r
        if r then
            local name, rules, state = L.Texts(r)
            row.name:SetText(name)
            row.rules:SetText(rules)
            row.players:SetText(players(r))
            row.state:SetText(state)
            -- this realm's row: "You're here" (or "Join through the relay", which also puts the link in the box, so
            -- that row has no Copy link of its own)
            local jt = L.JoinText(r)
            row.join:SetText(jt)
            row.join:SetWidth(jt == "Join by link" and 164 or jt == "You're here" and 80 or 56)
            if jt == "Join by link" then row.copyBtn:Hide() else row.copyBtn:Show() end
            row:Show()
        else
            row:Hide()
        end
    end
    win.count:SetText(n > ROWS and string.format("%d-%d of %d", L.offset + 1, math.min(n, L.offset + ROWS), n) or "")
    local b = L.browse
    if L.Browsing() and b and (tonumber(b.pages) or 1) > 1 then
        win.count:SetText(string.format("Page %d of %d · %d realms", tonumber(b.page) or 1, tonumber(b.pages) or 1, tonumber(b.total) or n))
    end
end

-- Up / Down: through the rows, then the page's next (or previous) page of the list.
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

local function info(text) if win then win.info:SetText(text) end end

function L.OnWrath()
    if not GetRealmName then return true end
    local ok, name = pcall(GetRealmName)
    -- Home is our realm under any of its names: today's, the Sample Hangout's, the Leyline brand's, the official name
    -- and the one it had before (Dalaran Hangout, until 2026-10-30).
    return not ok or name == nil or name == HOME_REALM or name == "Leyline Sample Hangout"
        or name == W.OFFICIAL_NAME or name == "Dalaran Hangout" or (brand().on and name == brand().home)
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

-- "Host your own": the three steps, and the website's page in the box to copy.
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
    if not r or r.id == "this" or r.sample then
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

local function buildRow(i)
    local row = CreateFrame("Button", nil, win.list)
    row:SetWidth(WIN_W - 40); row:SetHeight(ROW_H - 2)
    row:SetPoint("TOPLEFT", 0, -(i - 1) * ROW_H)
    if W.Style then
        local hl = row:CreateTexture(nil, "HIGHLIGHT")
        hl:SetTexture(W.Style.C.accent[1], W.Style.C.accent[2], W.Style.C.accent[3], 0.10)
        hl:SetAllPoints()
        local line = row:CreateTexture(nil, "BACKGROUND")
        line:SetTexture(W.Style.Unpack(W.Style.C.border))
        line:SetPoint("BOTTOMLEFT", 0, -1); line:SetPoint("BOTTOMRIGHT", 0, -1); line:SetHeight(1)
    else
        row:SetHighlightTexture("Interface\\QuestFrame\\UI-QuestTitleHighlight", "ADD")
    end
    row.name = label(row, "", "GameFontNormal", "TOPLEFT", 4, -2)
    row.name:SetWidth(230)
    row.rules = label(row, "", "GameFontHighlightSmall", "TOPLEFT", row.name, "BOTTOMLEFT", 0, -2)
    row.rules:SetWidth(230)
    row.players = label(row, "", "GameFontHighlightSmall", "TOPLEFT", 244, -4)
    row.players:SetWidth(100)
    row.state = label(row, "", "GameFontHighlightSmall", "TOPLEFT", 244, -18)
    row.copyBtn = button(row, "Copy link", 80, function(self)
        local r = self:GetParent().realm
        if r then L.selected = r; win.info:SetText("The link to " .. r.name .. ". Ctrl+C copies it."); copy(L.RowLink(r)) end
    end, "RIGHT", -64, 0)
    row.join = button(row, "Join", 56, function(self)
        local r = self:GetParent().realm
        if r and r.net and not r.here then L.Preview(r) elseif r then L.Join(r) end
    end, "RIGHT", -4, 0)
    row.copyBtn:ClearAllPoints()
    row.copyBtn:SetPoint("RIGHT", row.join, "LEFT", -4, 0)   -- left of Join, whatever its width
    row:SetScript("OnClick", function(self)
        if self.realm then copy(L.RowLink(self.realm)); L.Preview(self.realm) end
    end)
    row:Hide()
    return row
end

local function buildFriends(parent)
    local box = CreateFrame("Frame", nil, parent)
    box:SetPoint("BOTTOMLEFT", 16, 44); box:SetPoint("BOTTOMRIGHT", -16, 44)
    box:SetHeight(84)
    if W.Style then W.Style.Backdrop(box, W.Style.C.bg2) else box:SetBackdrop({ bgFile = "Interface\\Tooltips\\UI-Tooltip-Background",
        edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border", tile = true, tileSize = 16, edgeSize = 16,
        insets = { left = 4, right = 4, top = 4, bottom = 4 } }) end
    box:SetBackdropColor(0, 0, 0, 0.45)
    label(box, "Friends across realms", "GameFontNormal", "TOPLEFT", 12, -10)
    box.soon = label(box, "Coming", "GameFontDisableSmall", "TOPRIGHT", -12, -12)
    box.text = label(box, "Your Leyline friends on every realm in one list: who's online, where they are, "
        .. "and a click to join them.", "GameFontHighlightSmall", "TOPLEFT", 12, -30)
    box.text:SetWidth(WIN_W - 60)
    box.list = CreateFrame("Frame", nil, box)   -- the rows go here when friends arrive
    box.list:SetPoint("TOPLEFT", 12, -50); box.list:SetPoint("BOTTOMRIGHT", -12, 8)
    box.add = button(box, "Add a friend", 110, function() end, "BOTTOMRIGHT", -10, 8)
    box.add:Disable()
    if W.Social and W.Social.Adopt then W.Social.Adopt(box) end   -- Leyline friends (Social.lua)
    return box
end

local function build()
    win = CreateFrame("Frame", "WrathUnleashedLeylineFrame", UIParent)
    win:SetWidth(WIN_W); win:SetHeight(WIN_H)
    win:SetPoint("CENTER")
    win:SetFrameStrata("DIALOG")
    win:EnableMouse(true)
    win:SetMovable(true)
    win:RegisterForDrag("LeftButton")
    win:SetScript("OnDragStart", win.StartMoving)
    win:SetScript("OnDragStop", win.StopMovingOrSizing)
    if not W.Style then   -- Style.lua draws the window (S.Window, at its title)
        win:SetBackdrop({ bgFile = "Interface\\Tooltips\\UI-Tooltip-Background",
        edgeFile = "Interface\\DialogFrame\\UI-DialogBox-Border", tile = true, tileSize = 32, edgeSize = 32,
        insets = { left = 11, right = 12, top = 12, bottom = 11 } })
    end
    win:SetBackdropColor(0.05, 0.06, 0.08, 0.95)   -- solid: the dialog texture draws see-through in the browser client
    do   -- a colour fill the engine draws itself (no texture file; the backdrop fill needs one)
        local bg = win:CreateTexture(nil, "BACKGROUND")
        bg:SetTexture(0.094, 0.106, 0.137, 0.97)   -- #181b23 (Style.lua)
        bg:SetPoint("TOPLEFT", 10, -10); bg:SetPoint("BOTTOMRIGHT", -10, 10)
    end
    local _ = (W.Style and W.Style.Window(win, "Leyline") or label(win, "Leyline", "GameFontNormalLarge", "TOP", 0, -18))
    local close = (W.Style and W.Style.Close(win) or CreateFrame("Button", nil, win, "UIPanelCloseButton"))
    close:SetPoint("TOPRIGHT", -6, -6)
    button(win, "Done", 90, function() win:Hide() end, "BOTTOMRIGHT", -16, 14)
    win.back = button(win, brand().back, brand().on and 170 or 120, function() L.Back() end, "BOTTOMLEFT", 16, 14)
    win.host = button(win, "Host your own", 120, function() L.HostYourOwn() end, "BOTTOM", 0, 14)
    if W.Style then W.Style.SetKind(win.host, "primary") end
    tinsert(UISpecialFrames, "WrathUnleashedLeylineFrame")   -- Esc closes it

    label(win, "Realms", "GameFontNormal", "TOPLEFT", 20, -46)
    win.status = label(win, L.status, "GameFontHighlightSmall", "TOPLEFT", 84, -48)
    win.status:SetWidth(WIN_W - 200)
    win.refresh = button(win, "Refresh", 80, function() L.Request(true) end, "TOPRIGHT", -20, -42)

    -- filters: each button steps through the values the listed realms have, then back to "any"
    win.filters = {
        build = button(win, "Build: any", 104, function() L.Cycle("build") end, "TOPLEFT", 20, -68),
        tag = button(win, "Rules: any", 104, function() L.Cycle("tag") end, "TOPLEFT", 126, -68),
        region = button(win, "Region: any", 118, function() L.Cycle("region") end, "TOPLEFT", 232, -68),
        lang = button(win, "Lang: any", 90, function() L.Cycle("lang") end, "TOPLEFT", 352, -68),
        now = button(win, "Everyone", 86, function() L.Cycle("now") end, "TOPLEFT", 444, -68),
    }
    -- the realm browser's own (the page's list): search, then the filters
    local search = W.Style and W.Style.Input(win, 126, 20) or CreateFrame("EditBox", nil, win, "InputBoxTemplate")
    search:SetWidth(126); search:SetHeight(20)
    search:SetPoint("TOPLEFT", 24, -69)
    search:SetAutoFocus(false)
    search:SetMaxLetters(32)
    search:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
    search:SetScript("OnEnterPressed", function(self) self:ClearFocus(); L.searchAt = now() - SEARCH_WAIT end)
    search:SetScript("OnTextChanged", function(self)
        local q = (self:GetText() or ""):gsub("[|%c]", "")
        if q ~= L.query then L.query, L.searchAt = q, now() end
        if q == "" then win.searchHint:Show() else win.searchHint:Hide() end
    end)
    win.search = search
    win.searchHint = label(search, "Search realms", "GameFontDisableSmall", "LEFT", search, "LEFT", 6, 0)   -- on the box: above its fill
    win.bfilters = {
        browser = button(win, "In browser", 96, function() L.BCycle("browser") end, "TOPLEFT", 156, -68),
        own = button(win, "Own files", 88, function() L.BCycle("own") end, "TOPLEFT", 256, -68),
        players = button(win, "Everyone", 90, function() L.BCycle("players") end, "TOPLEFT", 348, -68),
        build = button(win, "Version: any", 98, function() L.BCycle("build") end, "TOPLEFT", 442, -68),
    }

    win.list = CreateFrame("Frame", nil, win)
    win.list:SetWidth(WIN_W - 40); win.list:SetHeight(ROWS * ROW_H)
    win.list:SetPoint("TOPLEFT", 20, LIST_TOP)
    win.list:EnableMouseWheel(true)
    win.list:SetScript("OnMouseWheel", function(_, delta)
        if L.Browsing() then L.Scroll(-delta) else L.offset = math.max(0, L.offset - delta); L.Refresh() end
    end)
    win.rows = {}
    for i = 1, ROWS do win.rows[i] = buildRow(i) end
    local listBottom = LIST_TOP - ROWS * ROW_H
    win.count = label(win, "", "GameFontHighlightSmall", "TOPLEFT", 20, listBottom - 8)
    button(win, "Up", 50, function() L.Scroll(-1) end, "TOPRIGHT", -76, listBottom - 4)
    button(win, "Down", 50, function() L.Scroll(1) end, "TOPRIGHT", -20, listBottom - 4)
    win.report = button(win, "Report", 64, function() L.Report() end, "TOPRIGHT", -132, listBottom - 4)

    local box = W.Style and W.Style.Input(win, WIN_W - 50, 20) or CreateFrame("EditBox", nil, win, "InputBoxTemplate")
    box:SetWidth(WIN_W - 50); box:SetHeight(20)
    box:SetPoint("TOPLEFT", 26, listBottom - 34)
    box:SetAutoFocus(false)
    box:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
    box:SetScript("OnTextChanged", function(self) if self.fixed and self:GetText() ~= self.fixed then self:SetText(self.fixed) end end)
    win.copy = box
    win.info = label(win, "Copy link puts a realm's link here; Ctrl+C copies it. Anyone with the link can play there "
        .. "in their browser.", "GameFontHighlightSmall", "TOPLEFT", 22, listBottom - 58)
    win.info:SetWidth(WIN_W - 44)

    -- Your game folder (GameFolder.lua): streamed places, or the whole world from the player's own folder.
    if W.GameFolder then win.folder = W.GameFolder.Attach(win, listBottom - 98, WIN_W) end

    win.friends = buildFriends(win)

    -- No answer from the realm in time: say so (and let Refresh try again).
    win:SetScript("OnUpdate", function()
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
        end
        if L.hopping and now() - L.hopping.at > HOP_WAIT then   -- no page took it (the desktop client)
            local h = L.hopping
            L.hopping, WUUI_LEYLINE_HOP = nil, nil
            info(h.target == "wrath" and "Hopping works in the browser game. Open " .. L.Link("this") .. " to play on " .. brand().home .. "."
                or "Hopping works in the browser game. Open the link above to play on " .. h.name .. ".")
        end
    end)
    win:SetScript("OnShow", function()
        if PlaySound then pcall(PlaySound, "igCharacterInfoOpen") end
        if L.Browsing() then L.AskList(L.netPage or 1) else L.Request(false) end
        L.Refresh()
    end)
    win:SetScript("OnHide", function() if win.panel then win.panel:Hide() end end)
    win:Hide()
end

-- The preview, beside the window (the window moves left to make room).
local function buildPanel()
    local p = CreateFrame("Frame", "WrathUnleashedLeylinePreview", win)
    p:SetWidth(PANEL_W); p:SetHeight(WIN_H)
    p:SetPoint("TOPLEFT", win, "TOPRIGHT", -4, 0)
    p:EnableMouse(true)
    do
        local bg = p:CreateTexture(nil, "BACKGROUND")
        bg:SetTexture(0.094, 0.106, 0.137, 0.97)
        bg:SetPoint("TOPLEFT", 10, -10); bg:SetPoint("BOTTOMRIGHT", -10, 10)
    end
    if W.Style then W.Style.Window(p, "Realm preview") else label(p, "Realm preview", "GameFontNormalLarge", "TOP", 0, -18) end
    local close = (W.Style and W.Style.Close(p) or CreateFrame("Button", nil, p, "UIPanelCloseButton"))
    close:SetPoint("TOPRIGHT", -6, -6)
    close:SetScript("OnClick", function() p:Hide() end)
    local wide = PANEL_W - 44
    p.title = label(p, "", "GameFontNormalLarge", "TOPLEFT", 22, -48); p.title:SetWidth(wide)
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
    p.join = button(p, "Join", 150, function() L.PreviewJoin() end, "BOTTOMLEFT", 20, 16)
    if W.Style then W.Style.SetKind(p.join, "primary") end
    p.copyBtn = button(p, "Copy link", 90, function()
        if L.selected then copy(L.RowLink(L.selected)) end
    end, "BOTTOMRIGHT", -20, 16)
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

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:SetScript("OnEvent", function(_, _, prefix, msg, channel, sender) L.OnMessage(prefix, msg, channel, sender) end)

SLASH_WULEYLINE1, SLASH_WULEYLINE2 = "/leyline", "/realms"
SlashCmdList["WULEYLINE"] = function() W.ToggleLeyline() end

W.Leyline = L
L.Window, L.PUBLIC_BASE, L.PREFIX, L.HOP_WAIT = function() return win end, PUBLIC_BASE, PREFIX, HOP_WAIT
L.ROWS, L.PANEL_W = ROWS, PANEL_W
