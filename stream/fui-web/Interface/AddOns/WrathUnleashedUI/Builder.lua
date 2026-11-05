-- Builder (3.3.5a API): the Den editor's pieces for the realm's Builder (realm/custom/lua_scripts/ov_build.lua, and
-- Den Lite's labs/den-lite ovmod.rs). Owner, 3 Nov: "very unintuitive, bulky ... doesn't look like Noggit or a gallery
-- of spinning 3D objects that I can place." So, in a game editor's look (Style.lua S.Ed*: near-black, gold accent):
--   * the asset browser (WrathUnleashedBuilderFrame, bottom left): a column of category icons (Favourites, Recent,
--     Prefabs, Stamps, Storage, Everything and every category, Every object and Models), a search box, A-Z and Spin,
--     "1-20 of 53" paging, and a grid of 20 square tiles, each the prop's own 3D model turning slowly (a Model frame,
--     SetModel(<the catalog's model path>), SetFacing each frame while Spin is on; the category's icon where there is
--     no model). Click a tile: it is picked (a gold edge); double-click (or Place, or drag it out of the window): it is
--     placed where you stand, in edit mode where you click next; right-click: a favourite. Hold Q in edit mode (or
--     /spawnmenu) shows the same window: the spawn menu is this one gallery.
--   * the status pill (LeylineEditorStatus, bottom centre): "Place · Barrel · snap 1 yd · 15° · 3 props selected" and
--     under it what the realm last said.
--   * the properties popover (WrathUnleashedPropsFrame): the selection's exact numbers (x, y, z, turn, pitch, roll,
--     size) with - / + nudges, the move step and the turn step, Copy / Paste / Delete / Undo / Redo, and More (lock,
--     hide, measure, align, stack, select mine, group, put away, floor). While it is open the arrow keys move the
--     selection by the step, Page Up / Page Down lift and lower it, Q / E turn it (override bindings, given back when
--     it closes; not in the browser's edit mode, whose camera has its own keys).
--   * Files (WrathUnleashedFilesFrame): the Den's save by name (in a Den), builds (Save, Load, Share by name or code),
--     prefabs (Save, Paste, Share) and your plot (Claim, Info, Add, Visitor, Remove).
-- The top strip and the tool rail are DenBar.lua's (Undo n / Redo n, the menu, Snap, 15° / 45° / 90°, Files,
-- Invite; Select, Place, Move, Rotate, Shape ground, Stamps, Path and the step presets). Opened by the rail's Place,
-- F7 (on a player's own game), Esc > Mods & Workshop > Builder and /builder.
-- One prop's exact numbers go to the realm as one change: ".build move <id> x y z yaw [pitch roll]" and
-- ".build scale <id> size" (a group uses setpos / setturn / setscale). With mod-sandbox-core on the realm (core=1)
-- the prop changes in place and keeps its object id; tilt (pitch, roll) needs it.
--
-- Everything the buttons do is a ".build ..." command, sent over the realm's OVMOD addon channel (ov_modapi.lua),
-- whispered to yourself, mod "wubuild":
--   client -> server   wubuild \t cmd \t 1/1 \t <the text after .build>      e.g. "spawn 2413", "move z 0.5"
--                      wubuild \t catalog \t 1/1 \t                          send me the catalog
--   server -> client   wubuild \t catalog \t i/n \t id,key,category,name,points[,model];...   (the allowlist)
--                      wubuild \t catmore \t i/n \t (the same items)       more of the catalog, added to it (Den Lite
--                                                  sends what doesn't fit one message so; an older addon ignores it)
--                      wubuild \t state \t i/n \t sel=..;mine=..;area=..;code=..;n=..;pos=..;pts=..;tier=..;plot=..;clip=..;
--                                                  ids=..;rot=<yaw>,<pitch>,<roll>;gids=<object GUID counters>;core=0|1
--                      wubuild \t prefabs \t i/n \t id,name,code,count;...
--                      wubuild \t storage \t i/n \t entry,count;...         what you have put away (.build store)
--   model (optional, 2026-11-03): the prop's model in the player's own game files, from GameObjectDisplayInfo
--   (e.g. World\Generic\Human\Passive Doodads\Barrels\Barrel01.m2; SetModel is given it as .mdx, the client's own
--   way; WMOs are left out): only a path is sent, the client draws its own file. A bare number there is read as a
--   display id (the plan's catalog v2, docs/DEN-OVERHAUL.md), its path to come later. Catalogs without it (older realms) still work: the tile
--   then shows the category's icon.
--   The state also carries undo=<n>;redo=<n>;stored=<n>;grp=<0|1> and, in a Den, den=<owner|builder|visitor>;
--   room=<used>/<cap>;yard=<used>/<cap> (the Den's two decor budgets).
-- The realm checks everything (role, area, plot, budget, rate) and answers in chat as for the dot commands. The
-- window only ever offers props from the realm's catalog. Without the realm script nothing is sent anywhere
-- else and the window says the Builder isn't on this realm.

local W = WrathUnleashedUI
local PREFIX, MOD = "OVMOD", "wubuild"
local COLS, ROWS = 5, 4
local PAGE = COLS * ROWS                        -- tiles on a page: 20 frames, reused (fewer rows on a short screen)
local TILE, GAP, NAME_H = 92, 8, 20             -- owner 3 Nov 08:40 "UI too small": 92-unit tiles, 13 px names
local CAT_W = 128                               -- the category column: an icon and the name, one row each
local HEAD_H, FOOT_H, PAD = 76, 42, 10
local WIN_W = PAD + CAT_W + PAD + COLS * TILE + (COLS - 1) * GAP + PAD
local function winHeight(rows) return HEAD_H + rows * (TILE + NAME_H + GAP) - GAP + FOOT_H end
local WIN_H = winHeight(ROWS)
-- Where the editor's windows go, clear of the game's own frames (UI units): the rail under the player frame, the
-- gallery right of the rail and above the chat (its top is 215 at the stock place), the selection's panel left of the minimap and
-- left of the side bars, the status pill above the action bars.
local LAYOUT = { railX = 8, railTop = 118, railW = 64, galleryX = 80, galleryBottom = 232, topMargin = 56, galleryTop = 112,
    propsRight = 198, propsTop = 52, pillBottom = 152 }
local ANSWER_WAIT = 6
local STEPS = { 0.01, 0.1, 0.25, 0.5, 1, 3, 5, 10 }   -- the move step, in yards
local STEP_PRESETS = { 2, 4, 5, 7 }             -- the panel's presets (indices into STEPS): 0.1, 0.5, 1, 5 (the owner's editor)
local ANGLE_STEPS = { 1, 5, 15, 45, 90 }        -- the turn step, in degrees (the top strip offers 15, 45 and 90)
local SIZE_STEP = 0.1
local SPIN_SPEED = 0.8                          -- radians a second
local REST_FACING = 0.6                         -- a three-quarter view while Spin is off
-- "objects" and "models" aren't catalog categories: the realm searches them (.build find / .build models; a Den
-- places any safe object and any model of the host's own game files), and their lists are the realm's answers.
-- "buildings" (houses, inns, towers: a Den's catalog only) shows only when the realm's catalog has them.
local CATS = { "all", "seating", "tables", "barrels", "crates", "lights", "banners", "tents", "fires", "fences", "nature", "decor",
    "structure", "buildings", "furniture", "objects", "models", "mymodels" }
local SEARCHED = { objects = "find", models = "models" }
-- "furniture" (a house's palette) isn't a realm category either: the seating and tables, and the beds, rugs, the rack
-- and the book from decor. "stamps" are a Den's ready-made areas (DenHome.lua, .den stamp).
local FURNITURE_IDS = { [13948] = true, [19504] = true, [181396] = true, [181077] = true, [188346] = true, [188347] = true,
    [188348] = true, [19635] = true, [2695] = true }
local function inCat(c, want)
    return want == "all" or c.cat == want or (want == "furniture" and (c.cat == "seating" or c.cat == "tables" or FURNITURE_IDS[c.id] == true))
end
local RECENT_MAX = 12
-- A small picture per category: the editor's own line icons (Style.lua S.Icon, Media\Icons; owner 3 Nov: "Don't use
-- the game's icons for the UI"), for the column and for a tile without a model.
local CAT_ICONS = { seating = "seating", tables = "tables", barrels = "barrels", crates = "crates", lights = "lights",
    banners = "banners", tents = "tents", fires = "fires", fences = "fences", nature = "nature", decor = "decor",
    structure = "structure", buildings = "buildings", furniture = "furniture", objects = "objects", models = "models", mymodels = "models" }
-- A tile that can't show its model shows a picture on purpose: a building (a WMO: a Model frame can't draw one) the
-- house; a prop the player's game files have no model for, its category's icon.
local WMO_ICON = "buildings"
local TAB_ICONS = { favourites = "favourites", recent = "recent", prefabs = "prefabs", stamps = "stamp", storage = "storage", all = "all" }
local function iconName(cat) return CAT_ICONS[cat or ""] or TAB_ICONS[cat or ""] or "noimage" end
local function catIcon(cat) return W.Style.Icon(iconName(cat)) end

-- The column: Favourites, Recent, Prefabs, Stamps, Storage, Everything, then every category.
local TABS = { "favourites", "recent", "prefabs", "stamps", "storage", "all" }
for i = 2, #CATS do TABS[#TABS + 1] = CATS[i] end
local TAB_TEXT = { favourites = "Favourites", recent = "Recent", prefabs = "Prefabs", stamps = "Stamps", storage = "Storage",
    all = "Everything", objects = "Every object", models = "Models", mymodels = "My models" }
-- what only the full Den has (Den Lite's state says lite=1): hidden there, not offered and then refused
local FULL_TABS = { prefabs = true, storage = true, models = true }
local function tabText(t) return TAB_TEXT[t] or (t:sub(1, 1):upper() .. t:sub(2)) end

local win, panel, files, pill
local B = { catalog = {}, shown = {}, offset = 0, tab = "all", search = "", state = {}, parts = {}, prefabs = {},
    storage = {}, found = {}, angle = 3, spin = true, az = false, facing = REST_FACING, mode = "place", step = 5 }
-- "My models" (the player's own models dropped on the launcher's Your Den card; Den Lite's mymodels.rs): only when
-- the catalog has one
local function tabHidden(t, lite)
    if lite and FULL_TABS[t] then return true end
    if t ~= "mymodels" then return false end
    for _, c in ipairs(B.catalog or {}) do if c.cat == "mymodels" then return false end end
    return true
end

local function S() return W.Style end
local function chip(parent, text, w, onClick, ...) return S().EdChip(parent, text, w, onClick, ...) end
local function label(parent, text, font, ...)
    local fs = parent:CreateFontString(nil, "OVERLAY", font or "GameFontHighlightSmall")
    fs:SetPoint(...)
    fs:SetJustifyH("LEFT")
    fs:SetText(text)
    return fs
end
local function smallCaps(parent, text, gold, ...) return S().EdLabel(parent, text, gold, ...) end
local function editBox(parent, w, maxLetters, ...)
    local e = S().EdInput(parent, w, 20)
    e:SetPoint(...)
    if maxLetters then e:SetMaxLetters(maxLetters) end
    return e
end
local function solid(f, layer, c)
    local t = f:CreateTexture(nil, layer or "BACKGROUND")
    t:SetTexture(c[1], c[2], c[3], c[4] or 1)
    return t
end

-- A "?" button: the long help, shown while the mouse is on it (a click keeps it open). Its own small frame, not
-- GameTooltip: edit mode hides UIParent, and the tooltip with it.
function W.HelpButton(parent, lines, width, ...)
    local b = W.Style.EdChip(parent, "?", 28, nil, ...)
    local tip = CreateFrame("Frame", nil, b)
    width = width or 420
    tip:SetWidth(width); tip:SetHeight(16 + #lines * 18)
    tip:SetPoint("BOTTOMRIGHT", b, "TOPRIGHT", 0, 4)   -- above, to the left (b.tip can be put elsewhere)
    tip:SetFrameStrata("TOOLTIP")
    W.Style.EdPanel(tip, 0.96, W.Style.ED.panel2)
    tip.lines = {}
    for i, line in ipairs(lines) do
        local fs = W.Style.EdText(tip, W.Style.ED_SIZE.text)
        fs:SetPoint("TOPLEFT", 10, -8 - (i - 1) * 18)
        fs:SetWidth(width - 16); fs:SetJustifyH("LEFT")
        fs:SetText(line)
        tip.lines[i] = fs
    end
    tip:Hide()
    b.tip = tip
    local enter, leave = b:GetScript("OnEnter"), b:GetScript("OnLeave")
    b:SetScript("OnEnter", function(self, ...) if enter then enter(self, ...) end; tip:Show() end)
    b:SetScript("OnLeave", function(self, ...) if leave then leave(self, ...) end; if not self.pinned then tip:Hide() end end)
    b:SetScript("OnClick", function(self) self.pinned = not self.pinned; if self.pinned then tip:Show() else tip:Hide() end end)
    return b
end

local function now() return (GetTime and GetTime()) or 0 end
local function editing() return W.EditMode and W.EditMode.IsOn and W.EditMode.IsOn() end

local refreshPill
local function setStatus(text)
    B.status, B.statusAt = text or "", now()
    if pill then
        pill.note:SetText(B.status)
        local m = S().ED.muted
        pill.note:SetTextColor(m[1], m[2], m[3])
    end
    if panel then panel.status:SetText(B.status) end
    if files then files.status:SetText(B.status) end
end

-- favourites, recents, Spin and A-Z, per character (WrathUnleashedUIChar is the addon's per-character saved table)
local function store()
    WrathUnleashedUIChar = WrathUnleashedUIChar or {}
    local c = WrathUnleashedUIChar
    c.builder = type(c.builder) == "table" and c.builder or {}
    c.builder.favs = type(c.builder.favs) == "table" and c.builder.favs or {}
    c.builder.recent = type(c.builder.recent) == "table" and c.builder.recent or {}
    c.builder.items = type(c.builder.items) == "table" and c.builder.items or {}   -- found objects and models, by id
    return c.builder
end

-- ---------------------------------------------------------------- talking to the realm

local function send(name, data)
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, PREFIX, MOD .. "\t" .. name .. "\t1/1\t" .. (data or ""), "WHISPER", UnitName("player"))
    return true
end

-- Only letters, digits, -, _ , . and spaces reach the realm; the realm checks the rest.
function B.Command(text)
    text = tostring(text or ""):gsub("[^%w%s%.%-_]", ""):sub(1, 100)
    return send("cmd", text)
end

function B.RequestCatalog()
    B.askedAt, B.waiting = now(), true
    setStatus("Asking the realm for the props...")
    return send("catalog", "")
end

-- Asks the realm to search every object ("objects") or every model ("models") for q (page n).
function B.Find(kind, q, page)
    local verb = SEARCHED[kind]
    q = tostring(q or ""):gsub("[^%w%s%-]", ""):gsub("^%s+", ""):gsub("%s+$", ""):sub(1, 40)
    if not verb then return false end
    if #q < 2 then setStatus("Type two or more letters, then Enter."); return false end
    B.asked = B.asked or {}
    B.asked[kind] = q
    setStatus("Searching...")
    return B.Command(verb .. " " .. q .. ((page and page > 1) and (" " .. page) or ""))
end

-- Next at the end of a searched list asks for the next page.
function B.MoreFound(kind)
    local f = B.found[kind]
    if f and f.more and f.q then return B.Find(kind, f.q, f.page + 1) end
    return false
end

-- A found object or model, kept so Recent and Favourites still show it next time (ids only reach the realm).
local function remember(c)
    if c and (c.cat == "objects" or c.cat == "models") then
        store().items[c.id] = { key = c.key, cat = c.cat, name = c.name, cost = c.cost, model = c.model, size = c.size, wmo = c.wmo }
    end
end

-- The model path a tile may show: the player's own game file, by its path only. Letters, digits, spaces and
-- _ - . ' and \ (or /). The client's Model:SetModel takes the .mdx ending (it loads the .m2 file of that name), so
-- .m2 / .mdl / no ending become .mdx; a WMO (a building's own format) can't show in a tile.
function B.ModelPath(m)
    m = tostring(m or ""):gsub("/", "\\")
    if m == "" or #m > 160 or not m:match("^[%w_%-%. '\\]+$") or m:find("%.%.") or m:find("^\\") then return nil end
    local low = m:lower()
    if low:match("%.wmo$") then return nil end
    if low:match("%.md[xl]$") then m = m:sub(1, -5) .. ".mdx"
    elseif low:match("%.m2$") then m = m:sub(1, -4) .. ".mdx"
    else m = m .. ".mdx" end
    return m
end

local function parseCatalog(data, into)
    local list = into or {}
    local seen = {}
    for _, c in ipairs(list) do seen[c.id] = true end
    for item in data:gmatch("[^;]+") do
        local id, key, cat, name, cost, model = item:match("^(%d+),([%w]+),(%a+),([^,]*),(%d+),(.+)$")
        if not id then id, key, cat, name, cost = item:match("^(%d+),([%w]+),(%a+),(.-),(%d+)$") end
        if not id then id, key, cat, name = item:match("^(%d+),([%w]+),(%a+),(.+)$") end
        if id and #list < 800 and not seen[tonumber(id)] then
            seen[tonumber(id)] = true
            -- the 6th field: a model path (and, Den Lite 2026-11-03, its size: ",<w>x<d>x<h>x<bottom>" yards), "-wmo"
            -- (a building: no model in a tile), "-" (no model in the player's files), or (catalog v2,
            -- docs/DEN-OVERHAUL.md) a bare display id whose path comes later; an older 5-field line still reads
            local size
            if model then
                local path, sw, sd, sh, sz = model:match("^(.-),(%d+%.?%d*)x(%d+%.?%d*)x(%d+%.?%d*)x(%-?%d+%.?%d*)$")
                if path then model, size = path, { w = tonumber(sw), d = tonumber(sd), h = tonumber(sh), z = tonumber(sz) } end
            end
            local display = model and model:match("^%d+$") and tonumber(model) or nil
            local wmo = model == "-wmo" or (model and model:lower():match("%.wmo$") ~= nil) or nil
            local path = (not display and not wmo and model ~= "-") and B.ModelPath(model) or nil
            list[#list + 1] = { id = tonumber(id), key = key, cat = cat, name = name:gsub("|", ""), cost = tonumber(cost) or 1,
                model = path, display = display, size = path and size or nil, wmo = wmo,
                noModel = (model == "-" or (model ~= nil and not path and not display and not wmo)) or nil }
        end
    end
    return list
end
B.ParseCatalog = parseCatalog

local function pair(s)
    local a, b = tostring(s or ""):match("^(%d+)/(%d+)$")
    return tonumber(a), tonumber(b)
end

local function parseState(data)
    local s = {}
    for k, v in data:gmatch("(%a+)=([^;]*)") do s[k] = v:gsub("|", "") end
    local id, entry, name = (s.sel or ""):match("^(%d+),(%d+),(.*)$")
    s.selName = name
    s.selId = tonumber(id or "")
    s.selEntry = tonumber(entry or "")
    -- caps: the verbs a newer server adds (selnext, selkind, lock on Den Lite...), so the editor offers only what it does
    s.can = {}
    for v in (s.caps or ""):gmatch("[%w]+") do s.can[v:lower()] = true end
    s.count = tonumber(s.n or "") or (name and 1 or 0)
    local x, y, z, t, sc = (s.pos or ""):match("^(%-?[%d%.]+),(%-?[%d%.]+),(%-?[%d%.]+),(%-?[%d%.]+),([%d%.]+)$")
    if x then s.p = { x = tonumber(x), y = tonumber(y), z = tonumber(z), turn = tonumber(t), scale = tonumber(sc) } end
    local yaw, pitch, roll = (s.rot or ""):match("^(%-?[%d%.]+),(%-?[%d%.]+),(%-?[%d%.]+)$")
    if s.p then s.p.yaw, s.p.pitch, s.p.roll = tonumber(yaw) or s.p.turn, tonumber(pitch) or 0, tonumber(roll) or 0 end
    -- ids (prop ids) and gids (their objects' GUID counters, what edit mode clicks and drags), in the same order
    s.idList, s.gidList = {}, {}
    for v in (s.ids or ""):gmatch("%d+") do s.idList[#s.idList + 1] = tonumber(v) end
    for v in (s.gids or ""):gmatch("%d+") do s.gidList[#s.gidList + 1] = tonumber(v) end
    s.core = s.core == "1"
    s.used, s.cap = pair(s.mine)
    s.ptsUsed, s.ptsCap = pair(s.pts)
    local pid, owner, role, pn, pc, pp, ppc = (s.plot or ""):match("^(%d+),(%w*),(%a+),(%d+)/(%d+),(%d+)/(%d+)$")
    if pid then
        s.plotInfo = { id = tonumber(pid), owner = owner, role = role, n = tonumber(pn), ncap = tonumber(pc),
            pts = tonumber(pp), ptsCap = tonumber(ppc) }
    end
    return s
end

-- The selection in words: "Nothing selected", "Selected: Barrel", "Selected: 3 props (last: Barrel)", and the area.
function B.SelectionText(s)
    s = s or B.state
    local parts = {}
    if s.count and s.count > 1 then
        parts[#parts + 1] = "Selected: " .. s.count .. " props" .. (s.selName and (" (last: " .. s.selName .. ")") or "")
    else
        parts[#parts + 1] = s.selName and ("Selected: " .. s.selName) or "Nothing selected"
    end
    if s.mine then parts[#parts + 1] = "props " .. s.mine end
    if s.area and s.area ~= "-" then parts[#parts + 1] = s.area else parts[#parts + 1] = "not in a build area" end
    return table.concat(parts, "  |  ")
end

local ROLE_TEXT = { owner = "yours", builder = "you build here", visitor = "you visit: look and copy only",
    gm = "GM", none = "you can't build here" }

function B.PlotText(s)
    s = s or B.state
    local p = s.plotInfo
    if not p then return "No plot here. Claim one to build with friends." end
    return string.format("%s's plot (%s): %d/%d props, %d/%d points", p.owner, ROLE_TEXT[p.role] or p.role,
        p.n, p.ncap, p.pts, p.ptsCap)
end

-- The meter: props and points against your limits; the fuller of the two fills the bar.
function B.Meter()
    local s = B.state
    local frac = 0
    if s.used and s.cap and s.cap > 0 then frac = math.max(frac, s.used / s.cap) end
    if s.ptsUsed and s.ptsCap and s.ptsCap > 0 then frac = math.max(frac, s.ptsUsed / s.ptsCap) end
    if frac > 1 then frac = 1 end
    local text = string.format("Props %s   Points %s", s.mine or "?", s.pts or "?")
    if s.tier and s.tier ~= "" then text = text .. "   (" .. s.tier .. ")" end
    return text, frac
end

local function refreshMeter()
    local text, frac = B.Meter()
    B.meterFrac = frac
    if win then
        win.meter:SetText(text)
        win.bar:SetWidth(math.max(1, frac * 120))
        if frac >= 1 then win.bar:SetVertexColor(0.9, 0.2, 0.2)
        elseif frac >= 0.8 then win.bar:SetVertexColor(0.95, 0.75, 0.2)
        else win.bar:SetVertexColor(0.25, 0.8, 0.3) end
    end
    if files then files.plot:SetText(B.PlotText()) end
end

-- Why a spawn of item can't happen now (your own limits; the realm checks the rest), or nil.
function B.Blocked(item)
    local s = B.state
    if s.used and s.cap and s.used >= s.cap then
        return string.format("You're at your limit of %d props. Delete some to place more.", s.cap)
    end
    if item and s.ptsUsed and s.ptsCap and s.ptsUsed + (item.cost or 1) > s.ptsCap then
        return string.format("Not enough build points: that needs %d, you have %d left. Delete something first.",
            item.cost or 1, math.max(0, s.ptsCap - s.ptsUsed))
    end
    return nil
end

local function chromeRefresh()
    local D = W.DenBar
    if D and D.Refresh and D.IsActive then pcall(D.Refresh) end
end

local function dispatch(name, data)
    if name == "catalog" or name == "catmore" then
        B.catalog = parseCatalog(data, name == "catmore" and B.catalog or nil)
        B.waiting = nil
        B.Filter()
        setStatus(#B.catalog .. " props. Double-click one to place it.")
    elseif name == "state" then
        B.waiting = nil
        local was = B.state.selId
        B.state = parseState(data)
        -- the browser client (20261026a) carries the whole selection on screen while you drag or carry it; it
        -- knows objects by their GUID counter (gids), and puts the handles on the realm's selection
        if LeylineEdit then
            local g = B.state.gidList
            pcall(LeylineEdit, "group " .. table.concat(g, " "))
            local primary = B.ObjectOfProp(B.state.selId)
            if primary and primary ~= B.handlesOn then
                B.handlesOn = primary
                pcall(LeylineEdit, "select " .. primary)
            elseif not primary then
                B.handlesOn = nil
            end
        end
        if files and B.state.code and B.state.code ~= "-" then files.code:SetText(B.state.code) end
        refreshMeter()
        -- something new selected while the editor is up (the gallery open, just placed, or a Den in build mode): its
        -- panel pops up (and goes when nothing is)
        local D = W.DenBar
        local up = (win and win:IsShown()) or (B.placedAt and now() - B.placedAt < 8)
            or (D and D.IsActive and D.IsActive() and not D.play)
        if B.state.selId and B.state.selId ~= was and up and not (panel and panel:IsShown()) then
            B.ShowProps(true); B.propsAuto = true
        elseif not B.state.selId and B.propsAuto and panel and panel:IsShown() then
            B.ShowProps(false)
        end
        B.PropsFill()
        -- the selection's keys exist only while something is selected
        if panel and panel:IsShown() and (B.keysOn == true) ~= (B.state.selId ~= nil) then B.BindKeys(true) end
        if refreshPill then refreshPill() end
        chromeRefresh()
    elseif name == "prefabs" then
        local list = {}
        for item in data:gmatch("[^;]+") do
            local id, pname, code, n = item:match("^(%d+),([%w_%-]+),([%w%-]+),(%d+)$")
            if id and #list < 60 then list[#list + 1] = { kind = "prefab", id = tonumber(id), name = pname, code = code ~= "-" and code or nil, count = tonumber(n) } end
        end
        B.prefabs = list
        B.Filter()
    elseif name == "found" then
        -- a .build find / .build models answer: <kind>,<page>,<more>;id,key,category,name,points[,model];...
        local kind, page, more = data:match("^(%a+),(%d+),([01])")
        if SEARCHED[kind or ""] then
            local list = parseCatalog(data)
            if tonumber(page) > 1 and B.found[kind] and B.found[kind].page == tonumber(page) - 1 then
                local all = B.found[kind].list
                for _, c in ipairs(list) do all[#all + 1] = c end
                list = all
            end
            B.found[kind] = { list = list, page = tonumber(page), more = more == "1", q = B.asked and B.asked[kind] }
            for _, c in ipairs(list) do B.known = B.known or {}; B.known[c.id] = c end
            B.Filter()
            setStatus(#list == 0 and "Nothing matches." or string.format("%d found%s. Double-click one to place it.", #list,
                more == "1" and " (Next for more)" or ""))
        end
    elseif name == "storage" then
        -- what you have put away: [entry] = count (placing one from the Storage tab takes it out)
        local st = {}
        for item in data:gmatch("[^;]+") do
            local e, n = item:match("^(%d+),(%d+)$")
            if e then st[tonumber(e)] = tonumber(n) end
        end
        B.storage = st
        B.Filter()
    end
end

function B.OnMessage(prefix, msg, channel, sender)
    if prefix ~= PREFIX or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, i, n, data = msg:match("^([^\t]+)\t([^\t]+)\t(%d+)/(%d+)\t(.*)$")
    if mod ~= MOD then return end
    i, n = tonumber(i), tonumber(n)
    if n < 1 or n > 32 or i < 1 or i > n then return end
    if n == 1 then B.parts[name] = nil; return dispatch(name, data) end
    local buf = B.parts[name]
    if i == 1 or not buf or buf.n ~= n then buf = { n = n, got = 0 }; B.parts[name] = buf end
    if not buf[i] then buf[i] = data; buf.got = buf.got + 1 end
    if buf.got == n then
        B.parts[name] = nil
        dispatch(name, table.concat(buf, "", 1, n))
    end
end

-- ---------------------------------------------------------------- what a tab lists

-- The catalog's item with this entry (or one known from before), nil when there is none.
function B.ById(id)
    id = tonumber(id)
    for _, c in ipairs(B.catalog) do if c.id == id then return c end end
    return B.known and B.known[id] or nil
end

local function byIdMap()
    local m = {}
    for _, c in ipairs(B.catalog) do m[c.id] = c end
    return m
end

-- What a tab lists: only the realm's catalog (and your prefabs, a Den's stamps), never anything else.
function B.ListFor(tab, search)
    local q = tostring(search or ""):lower()
    -- by name, by its key, and by its category as a tag ("light", "fence", "seat": the gallery's search box)
    local function match(c)
        return q == "" or c.name:lower():find(q, 1, true) or (c.key and c.key:find(q, 1, true)) or (c.cat and #q >= 3 and c.cat:find(q, 1, true))
    end
    local out = {}
    if tab == "prefabs" then
        for _, f in ipairs(B.prefabs) do if match(f) then out[#out + 1] = f end end
        return out
    end
    if tab == "stamps" then
        -- Den Lite's library (BuildTools.lua: Leyline's own, yours, copied places; by category), else Den home's six
        local lib = W.BuildTools and W.BuildTools.StampItems and W.BuildTools.StampItems() or {}
        if #lib == 0 then lib = (W.DenHome and W.DenHome.StampItems and W.DenHome.StampItems()) or {} end
        for _, st in ipairs(lib) do if match(st) then out[#out + 1] = st end end
        return out
    end
    if SEARCHED[tab] then
        for _, c in ipairs((B.found[tab] or {}).list or {}) do out[#out + 1] = c end
        return out
    end
    local ids = byIdMap()
    for id, it in pairs(store().items) do   -- found objects and models, for Recent, Favourites and Storage
        if not ids[id] and type(it) == "table" and it.name then
            ids[id] = { id = id, key = it.key, cat = it.cat, name = it.name, cost = it.cost or 1, model = B.ModelPath(it.model),
                size = type(it.size) == "table" and it.size or nil, wmo = it.wmo }
        end
    end
    if tab == "storage" then
        for _, c in ipairs(B.catalog) do if (B.storage[c.id] or 0) > 0 and match(c) then out[#out + 1] = c end end
        for id, c in pairs(ids) do
            if (B.storage[id] or 0) > 0 and (c.cat == "objects" or c.cat == "models") and match(c) then out[#out + 1] = c end
        end
        return out
    end
    if tab == "recent" then
        for _, id in ipairs(store().recent) do local c = ids[id]; if c and match(c) then out[#out + 1] = c end end
        return out
    end
    local favs = store().favs
    for _, c in ipairs(B.catalog) do
        local inTab = inCat(c, tab) or (tab == "favourites" and favs[c.id])
        if inTab and match(c) then out[#out + 1] = c end
    end
    if tab == "favourites" then
        for id, c in pairs(ids) do if favs[id] and (c.cat == "objects" or c.cat == "models") and match(c) then out[#out + 1] = c end end
    end
    return out
end

-- The list the gallery shows: the tab, the search (the searched tabs search on Enter), A-Z when on.
function B.Filter()
    local list = B.ListFor(B.tab, SEARCHED[B.tab] and "" or B.search)
    if B.az and B.tab ~= "recent" then
        local sorted = {}
        for i, c in ipairs(list) do sorted[i] = c end
        table.sort(sorted, function(a, b)
            local x, y = a.name:lower(), b.name:lower()
            if x == y then return (a.id or 0) < (b.id or 0) end
            return x < y
        end)
        list = sorted
    end
    B.shown = list
    B.menuItems = list   -- the old spawn menu's name for the same list
    if B.pick then
        local still = false
        for _, c in ipairs(B.shown) do if c == B.pick then still = true end end
        if not still then B.pick = nil end
    end
    B.Refresh()
end

function B.ShowTab(tab)
    local ok = false
    for _, t in ipairs(TABS) do if t == tab then ok = true end end
    if not ok then return false end
    B.tab, B.offset = tab, 0
    B.Filter()
    return true
end

-- ---------------------------------------------------------------- the gallery's tiles

local function tileNameText(c)
    if c.kind == "prefab" then return c.name .. " (" .. (c.count or 0) .. ")" end
    if B.tab == "storage" then return c.name .. " x" .. (B.storage[c.id] or 0) end
    return c.name
end

local function tileIcon(c)
    if c.kind == "prefab" then return catIcon("prefabs") end
    if c.kind == "stamp" then return catIcon("stamps") end
    if c.wmo then return W.Style.Icon(WMO_ICON) end
    return catIcon(c.cat)
end
B.TileIcon = tileIcon

local function paintTile(t)
    local c = t.item
    local picked = c ~= nil and c == B.pick
    local e = picked and S().ED.gold or (t.over and { 1, 1, 1, 0.45 } or S().ED_EDGE)
    for _, x in ipairs(t.edges) do x:SetTexture(e[1], e[2], e[3], e[4]) end
    if t.over or picked then t.hover:Show() else t.hover:Hide() end
end
B.PaintTile = paintTile

-- How a tile shows its model: from the SIDE. The owner's PC sweep (3 Nov, den-features "fit-5-playermodel-own"): a
-- plain Model frame shows a doodad from straight above (its "up" toward you; no SetPitch on any 3.3.5a model frame),
-- PlayerModel and DressUpModel framed by hand came out empty, but a PlayerModel left to frame the model itself
-- (scale 1 at 0, 0, 0: its own camera) shows props from the side. So every tile, and the selection's larger
-- preview, is that: a PlayerModel, its own framing. SetFacing turns it about its up axis (Spin, or a right-drag).
-- A client where a PlayerModel can't be made falls back to a plain Model.
-- Owner 3 Nov: in a model window (a tile, the preview) hold the RIGHT mouse button and drag to turn the model; the
-- wheel over it zooms in and out (only over that window: the camera keeps the wheel everywhere else). Spin still
-- turns them all; a turn of your own is added to it. Shift + wheel over a tile pages the gallery as before.
B.VIEW_FRAME = "PlayerModel"
B.ZOOM_MIN, B.ZOOM_MAX, B.ZOOM_STEP = 0.5, 3.0, 1.15
B.TURN_PER_PX = 0.02                      -- radians a UI unit of right-drag
local DRAG_SLOP = 3                       -- UI units before a right-press is a drag (a click stays a click)

-- A model frame of the view's kind on `parent` (a PlayerModel; a plain Model where that can't be made).
local function newViewModel(parent)
    local ok, m = pcall(CreateFrame, B.VIEW_FRAME, nil, parent)
    if not ok or not m then ok, m = pcall(CreateFrame, "Model", nil, parent) end
    m.kind = (ok and m and m.kind) or B.VIEW_FRAME
    return m
end

-- A view's facing now: Spin's (or the resting three-quarter view) plus the player's own turn.
function B.ViewFacing(v)
    return ((B.spin and B.facing or REST_FACING) or REST_FACING) + ((v and v.turn) or 0)
end

-- The model of view `v` (a tile, the preview) framed by its own camera at the view's zoom, turned.
local function applyView(v)
    local m = v and v.model
    if not (m and v.modelPath) then return false end
    pcall(m.SetModelScale, m, v.zoom or 1)
    pcall(m.SetPosition, m, 0, 0, 0)
    pcall(m.SetFacing, m, B.ViewFacing(v))
    return true
end
B.ApplyView = applyView

-- Zoom view `v` by the wheel's `delta` (up: nearer), kept between B.ZOOM_MIN and B.ZOOM_MAX.
function B.ViewZoom(v, delta)
    if not (v and v.modelPath) then return false end
    local z = (v.zoom or 1) * ((tonumber(delta) or 0) > 0 and B.ZOOM_STEP or 1 / B.ZOOM_STEP)
    v.zoom = math.max(B.ZOOM_MIN, math.min(B.ZOOM_MAX, z))
    applyView(v)
    return true
end

-- Turn view `v` by `rad` (a right-drag; the test's `.den ui view turn`).
function B.ViewTurn(v, rad)
    if not (v and v.modelPath) then return false end
    v.turn = ((v.turn or 0) + (tonumber(rad) or 0)) % (2 * math.pi)
    applyView(v)
    return true
end

local function cursorX(f)
    local x = GetCursorPosition and GetCursorPosition()
    local s = (f and f.GetEffectiveScale and tonumber((f:GetEffectiveScale()))) or 1
    return x and (tonumber(x) or 0) / math.max(0.01, s) or nil
end

-- Right-drag to turn, the wheel to zoom, on `host` (the frame that gets the mouse) for view `v`. Returns a function
-- for the host's OnUpdate (the drag follows the cursor). `pageWheel(delta)`: what Shift + wheel does instead.
function B.AttachViewMouse(host, v, pageWheel)
    if host.EnableMouseWheel then host:EnableMouseWheel(true) end
    local function down(self, button)
        if button ~= "RightButton" or not v.modelPath then return end
        v.drag = { x = cursorX(self), turn = v.turn or 0 }
        v.dragged = nil
    end
    local function up(self, button)
        if button == "RightButton" and v.drag then v.drag = nil end
    end
    if host.HookScript and host:GetScript("OnMouseDown") then host:HookScript("OnMouseDown", down) else host:SetScript("OnMouseDown", down) end
    if host.HookScript and host:GetScript("OnMouseUp") then host:HookScript("OnMouseUp", up) else host:SetScript("OnMouseUp", up) end
    host:SetScript("OnMouseWheel", function(_, delta)
        if pageWheel and IsShiftKeyDown and IsShiftKeyDown() then pageWheel(delta); return end
        B.ViewZoom(v, delta)
    end)
    return function(self)
        local d = v.drag
        if not d then return end
        if IsMouseButtonDown and not IsMouseButtonDown("RightButton") then v.drag = nil; return end
        local x = cursorX(self)
        if not (x and d.x) then return end
        local dx = x - d.x
        if math.abs(dx) > DRAG_SLOP then v.dragged = true end
        if v.dragged then
            v.turn = (d.turn + dx * B.TURN_PER_PX) % (2 * math.pi)
            applyView(v)
        end
    end
end

-- The tile's model frame (made once).
local function tileModel(t)
    if not t.model then
        local m = newViewModel(t)
        m:SetWidth(TILE - 4); m:SetHeight(TILE - 4)
        m:SetPoint("TOPLEFT", 2, -2)
        m:Hide()
        m:SetScript("OnShow", function() applyView(t) end)
        t.model = m
    end
    if t.top and t.model.GetFrameLevel then
        local lv = t.model:GetFrameLevel()
        if type(lv) == "number" then t.top:SetFrameLevel(lv + 2) end
    end
    return t.model
end
B.TileModel = tileModel

-- Each frame (the grid's OnUpdate): Spin, and a right-drag on a tile.
function B.TileTick(elapsed)
    if not win then return end
    for _, t in ipairs(win.tiles) do
        if t.drag and t.dragTick then t.dragTick(t) end
    end
    B.Turntable(elapsed)
end

local function setTileModel(t, c)
    local path = c and c.model
    if path then
        -- shown first: a Model frame given a model while hidden may draw nothing (tiles are reused across pages)
        t.model:Show()
        if t.modelPath ~= path then
            t.modelPath = path
            t.turn, t.zoom, t.drag = 0, 1, nil      -- a new prop: as it comes, your own turn and zoom start again
            if t.model.ClearModel then pcall(t.model.ClearModel, t.model) end
            pcall(t.model.SetModel, t.model, path)
            -- a plain Model frame has no light of its own: without one every model draws black (3 Nov, the
            -- owner's gallery was black tiles). Ambient plus a key light from the front-left, as the
            -- character screens light theirs.
            if t.model.SetLight then pcall(t.model.SetLight, t.model, 1, 0, -0.5, -0.7, -0.5, 0.75, 1, 1, 1, 0.9, 1, 0.95, 0.85) end
            applyView(t)
        end
        t.icon:Hide()
        t.badge:Show()
        t.why:SetText("")
    else
        if t.modelPath then t.modelPath = nil; if t.model.ClearModel then pcall(t.model.ClearModel, t.model) end end
        t.model:Hide()
        t.icon:Show()
        t.badge:Hide()
        -- a picture on purpose, and why: a building, or no model in this game's files
        t.why:SetText(c and not c.kind and (c.wmo and "building" or "no preview") or "")
    end
end

-- What the gallery shows (the den-test's feature check, DenBar.lua D.Report): tiles on screen, with a model, with an
-- icon on purpose (buildings, props without a model), and the catalog's own counts.
function B.TileReport()
    local r = { shown = (win and win:IsShown()) and 1 or 0, tiles = 0, model = 0, icon = 0, empty = 0, wmo = 0, none = 0,
        rows = B.rows or ROWS, cols = COLS, tile = TILE, catalog = #B.catalog, withModel = 0, boxed = 0, missing = {} }
    for _, c in ipairs(B.catalog) do
        if c.model then r.withModel = r.withModel + 1; if c.size then r.boxed = r.boxed + 1 end
        elseif c.wmo then r.wmo = r.wmo + 1
        else r.none = r.none + 1; if #r.missing < 40 then r.missing[#r.missing + 1] = c.name end end
    end
    for _, t in ipairs((win and win.tiles) or {}) do
        if t:IsShown() and t.item then
            r.tiles = r.tiles + 1
            if t.modelPath and t.model:IsShown() then r.model = r.model + 1
            elseif t.icon:IsShown() and t.icon.path then r.icon = r.icon + 1
            else r.empty = r.empty + 1 end
        end
    end
    return r
end

local function emptyText(n)
    if n > 0 then return "" end
    local t = B.tab
    if t == "favourites" then return "No favourites yet: right-click a prop to add it." end
    if t == "recent" then return "Props you place show up here." end
    if t == "prefabs" then return "No prefabs yet: select props, then Save in Files > Prefab." end
    if t == "stamps" then return "Stamps are ready-made areas for a Den: a garden, a market, a campfire, a dock, a house." end
    if t == "storage" then return "Nothing put away: select props, then Put away (the selection's More)." end
    if SEARCHED[t] then return "Type a word in Search, then Enter: the realm searches every "
        .. (t == "models" and "model" or "object") .. " it has." end
    if #B.catalog == 0 then return B.waiting and "Asking the realm for the props..." or "No props yet." end
    return "Nothing matches."
end

function B.Refresh()
    if refreshPill then refreshPill() end
    if not win then return end
    local n = #B.shown
    local maxOff = math.max(0, math.floor((n - 1) / COLS) * COLS)   -- the last page may be part full
    if B.offset > maxOff then B.offset = maxOff end
    if B.offset < 0 then B.offset = 0 end
    local favs = store().favs
    local page = B.PageSize()
    for i, t in ipairs(win.tiles) do
        local c = i <= page and B.shown[B.offset + i] or nil
        t.item = c
        if c then
            t.elided = S().Elide(t.name, tileNameText(c), TILE - 4)
            t.fav:SetText(favs[c.id] and c.kind == nil and "*" or "")
            t.cost:SetText((c.cost or 1) > 1 and tostring(c.cost) or "")
            t.icon:SetTexture(tileIcon(c)); t.icon.path = tileIcon(c)
            t.badge:SetTexture(tileIcon(c))
            t:Show()
            setTileModel(t, c)
            paintTile(t)
        else
            t:Hide()
        end
    end
    win.title:SetText(tabText(B.tab):upper())
    for _, b in ipairs(win.tabs) do b:SetActive(b.tab == B.tab) end
    win.az:SetActive(B.az)
    win.spinBtn:SetActive(B.spin)
    local f = SEARCHED[B.tab] and B.found[B.tab]
    win.count:SetText(n > 0 and string.format("%d-%d of %d%s", B.offset + 1, math.min(n, B.offset + page), n,
        (f and f.more) and "+" or "") or "")
    win.empty:SetText(emptyText(n))
    if B.offset > 0 then win.prev:Enable() else win.prev:Disable() end
    if B.offset + page < n or (f and f.more) then win.next:Enable() else win.next:Disable() end
    -- what only the full Den has stays out of Den Lite's column
    local lite = B.state.lite == "1"
    local y = -PAD
    for _, b in ipairs(win.tabs) do
        if tabHidden(b.tab, lite) then b:Hide() else b:ClearAllPoints(); b:SetPoint("TOPLEFT", win, "TOPLEFT", PAD, y); b:Show(); y = y - win.tabStep end
    end
    win.spawn:SetText(B.pick and ("Place " .. B.pick.name) or "Place")
    if B.pick then win.spawn:Enable() else win.spawn:Disable() end
end

-- Paging: Next / Prev a page; the wheel a row. At the end of a searched list Next asks the realm for more.
function B.PageSize() return COLS * (B.rows or ROWS) end
function B.Page(dir)
    local page = B.PageSize()
    if dir > 0 and B.offset + page >= #B.shown then
        if SEARCHED[B.tab] then return B.MoreFound(B.tab) end
        return false
    end
    B.offset = math.max(0, B.offset + dir * page)
    B.Refresh()
    return true
end
function B.Scroll(rows)
    local full = math.max(0, math.ceil((#B.shown - B.PageSize()) / COLS) * COLS)   -- the wheel stops at a full last page
    B.offset = math.max(0, math.min(math.max(full, B.offset), B.offset + rows * COLS))
    B.Refresh()
end

-- Spin: the tiles' models turn slowly; off, they rest at a three-quarter view.
function B.SetSpin(on)
    B.spin = on and true or false
    store().spin = B.spin
    if not B.spin then
        B.facing = REST_FACING
        if win then for _, t in ipairs(win.tiles) do if t.modelPath then pcall(t.model.SetFacing, t.model, B.ViewFacing(t)) end end end
        if B.preview and B.preview.modelPath then applyView(B.preview) end
    end
    B.Refresh()
end
-- Spin: every model shown turns (the tiles and the preview), each with its own turn added.
function B.Turntable(elapsed)
    if not B.spin then return end
    B.facing = ((B.facing or REST_FACING) + (tonumber(elapsed) or 0) * SPIN_SPEED) % (2 * math.pi)
    for _, t in ipairs((win and win.tiles) or {}) do
        if t.modelPath and t:IsShown() then pcall(t.model.SetFacing, t.model, B.ViewFacing(t)) end
    end
    local v = B.preview
    if v and v.modelPath and v.model:IsShown() then pcall(v.model.SetFacing, v.model, B.ViewFacing(v)) end
end
function B.SetAZ(on) B.az = on and true or false; store().az = B.az; B.offset = 0; B.Filter() end

-- ---------------------------------------------------------------- picking and placing

local function addRecent(id)
    local r = store().recent
    for i = #r, 1, -1 do if r[i] == id then table.remove(r, i) end end
    table.insert(r, 1, id)
    while #r > RECENT_MAX do table.remove(r) end
end

function B.ToggleFavourite(c)
    if not c or c.kind == "prefab" or c.kind == "stamp" then return end
    local favs = store().favs
    favs[c.id] = (not favs[c.id]) or nil
    remember(c)
    setStatus(favs[c.id] and (c.name .. " is a favourite.") or (c.name .. " is no longer a favourite."))
    if B.tab == "favourites" then B.Filter() else B.Refresh() end
end

-- One click on a tile: picked (the gold edge) AND armed: the next placement is this prop, no other click needed.
-- The pill says "Place: Barrel — click Place or press Enter / double-click"; Enter places it in front of you (Place
-- and a double-click too); Esc or Clear lets it go. After placing, the realm selects the new prop and its panel opens.
function B.Choose(item)
    if not item then return false end
    B.pick = item
    B.mode = "place"
    B.Arm(item)
    -- Den Lite's build mode: the real prop follows the cursor (Gizmo.lua); the panel shows it in hand
    if W.Gizmo and W.Gizmo.build and not item.kind then B.InHand(item) end
    if item.kind == "prefab" then setStatus(string.format("%s: %d props. Enter or double-click pastes it.", item.name, item.count or 0))
    elseif item.kind == "stamp" then setStatus(item.name .. ": " .. (item.line or "") .. " Enter or double-click marks where it goes.")
    else setStatus(string.format("%s: %d build point%s. Enter or double-click places it; right-click: a favourite.",
        item.name, item.cost or 1, (item.cost or 1) == 1 and "" or "s")) end
    B.Refresh()
    return true
end

-- Armed: the picked prop waits for Enter (or Place, or a double-click). B.Arm(nil) lets it go.
function B.Arm(item)
    B.armed = item
    B.BindArmKeys(item ~= nil)
    if refreshPill then refreshPill() end
    return item ~= nil
end
function B.Armed() return B.armed end
function B.PlaceArmed()
    local it = B.armed
    if not it then B.BindArmKeys(false); return false end
    return B.Place(it)
end

-- Places the picked prop: in front of you (Den Lite: 3 yards ahead), or in edit mode where you click next.
function B.Spawn()
    if not B.pick then setStatus("Pick a prop first."); return false end
    if B.pick.kind then return B.Place(B.pick) end
    local why = B.Blocked(B.pick)
    if why then setStatus(why); return false end
    addRecent(B.pick.id)
    remember(B.pick)
    -- In edit mode the next click on the ground places it (EditMode.lua); otherwise at your feet.
    if editing() then
        W.EditMode.StartPlacing(B.pick)
        B.placedAt = now()
        B.Arm(nil)
        setStatus("Click the ground where it goes.")
        return true
    end
    B.placedAt = now()
    B.Arm(nil)
    return B.Command("spawn " .. B.pick.id)
end

-- Double-click, Place, or a tile dragged out of the window: a prop, a prefab (pasted) or a stamp (marked).
function B.Place(item)
    if not item then return false end
    local ok
    if item.kind == "prefab" then
        ok = B.PastePrefab(item.name)
    elseif item.kind == "stamp" then
        -- Den Lite: placed with the mouse (BuildTools.lua: a click on the ground puts it there, Enter keeps it)
        local T = W.BuildTools
        if T and T.Here() then ok = T.PlaceStamp(item.key); if ok then T.Show("stamps") end
        else ok = W.DenHome and W.DenHome.Stamp(item.key, editing()) end   -- in edit mode: as the shelf's tiles
    end
    if item.kind then
        B.Arm(nil)
    else
        B.pick = item
        B.Refresh()
        ok = B.Spawn()
    end
    -- Opened by holding Q in edit mode: the next click goes to the world, so the gallery gets out of the way.
    if ok and editing() and B.heldOpen and win then win:Hide() end
    return ok
end

-- The old spawn menu's click: right-click a favourite, else place.
function B.MenuPick(item, mouse)
    if not item then return false end
    if mouse == "RightButton" then B.ToggleFavourite(item); return true end
    return B.Place(item)
end
function B.MenuList() return B.shown end
function B.MenuRefresh() B.Filter() end

function B.Move(axis, sign)
    return B.Command(string.format("move %s %s", axis, tostring(sign * STEPS[B.step])))
end

function B.TurnStep() return ANGLE_STEPS[B.angle] end
function B.Turn(sign) return B.Command("turn " .. (sign * ANGLE_STEPS[B.angle])) end

-- The step (yards) and the turn step (degrees): the rail's presets, the strip's 15 / 45 / 90, the popover's chips.
function B.SetStep(i)
    i = tonumber(i)
    if not i or not STEPS[i] then return false end
    B.step = i
    if panel then panel.stepBtn:SetText("Step " .. STEPS[B.step] .. " yd"); for si, c in pairs(panel.stepChips or {}) do c:SetActive(si == B.step) end end
    if refreshPill then refreshPill() end
    chromeRefresh()
    return true
end
function B.SetAngle(deg)
    for i, a in ipairs(ANGLE_STEPS) do
        if a == tonumber(deg) then
            B.angle = i
            if SetCVar then pcall(SetCVar, "leylineEditAngle", tostring(a)) end   -- edit mode's Q / E follow
            if panel then panel.angle:SetText("Turn " .. a .. "°"); for d, c in pairs(panel.angleChips or {}) do c:SetActive(d == a) end end
            if refreshPill then refreshPill() end
            chromeRefresh()
            return true
        end
    end
    return false
end

-- Snap: the grid for what you place (off, 0.5, 1, 2 or 5 yards): Den home's grid at your feet (.build grid) and edit
-- mode's grid for clicks.
B.SNAPS = { 0, 0.5, 1, 2, 5 }
function B.Snap()
    local H = W.DenHome
    if H and H.Grid then return H.Grid() end
    return B.snap or 0
end
function B.SetSnap(g)
    g = tonumber(g) or 0
    B.snap = g
    local H = W.DenHome
    if H and H.SetGrid then H.SetGrid(g) else B.Command(g > 0 and ("grid " .. g) or "grid off") end
    if SetCVar then pcall(SetCVar, "leylineEditGrid", tostring(g)) end
    setStatus(g > 0 and ("What you place snaps to a " .. g .. " yard grid.") or "Snap off: props go exactly where you put them.")
    chromeRefresh()
    return g
end

-- The editor's mode (the rail): select, place, move, rotate; the pill says it.
function B.SetMode(m)
    B.mode = m
    if refreshPill then refreshPill() end
    chromeRefresh()
end

-- The prop id behind an object id (GUID counter) in the selection, and back; nil if it isn't selected.
function B.PropForObject(gid)
    local s = B.state
    gid = tonumber(gid)
    if not gid or gid == 0 or not s.gidList then return nil end
    for i, g in ipairs(s.gidList) do if g == gid then return s.idList[i] end end
    return nil
end
function B.ObjectOfProp(pid)
    local s = B.state
    pid = tonumber(pid)
    if not pid or not s.idList then return nil end
    for i, id in ipairs(s.idList) do
        if id == pid then local g = s.gidList[i]; return (g and g ~= 0) and g or nil end
    end
    return nil
end

-- One prop to an exact place and rotation (degrees), as one change: ".build move <id> x y z yaw [pitch roll]".
-- Tilt rides along only when the realm can show it (core=1) or the prop has one.
function B.MoveTo(pid, x, y, z, yaw, pitch, roll)
    if not (pid and x and y and z and yaw) then return false end
    local text = string.format("move %d %.3f %.3f %.3f %.1f", pid, x, y, z, yaw % 360)
    pitch, roll = pitch or 0, roll or 0
    if B.state.core or math.abs(pitch) > 0.05 or math.abs(roll) > 0.05 then
        text = text .. string.format(" %.1f %.1f", pitch, roll)
    end
    return B.Command(text)
end

-- ---------------------------------------------------------------- selecting
-- Does the realm do this verb? The full Den (ov_build.lua) does lock and the rest; Den Lite says what it adds in the
-- state's caps= (selnext: Tab cycles the props near you; selkind: every prop of the selection's kind; lock).
local FULL_VERBS = { lock = true, unlock = true }
function B.Can(verb)
    local s = B.state
    if s.can and s.can[verb] then return true end
    return s.lite ~= "1" and FULL_VERBS[verb] == true
end

-- Esc: nothing selected, the panel closed, a waiting pick let go.
function B.Deselect()
    B.Arm(nil)
    if B.state.selId then B.Command("selclear") end
    B.ShowProps(false)
    setStatus("Nothing selected.")
    return true
end

-- Tab: the next prop near you (the realm's selnext); a realm without it: the nearest (Near).
function B.SelectNext()
    if B.Can("selnext") then return B.Command("selnext") end
    setStatus("Selected the nearest prop (Tab cycles once Den Lite has selnext).")
    return B.Command("near")
end

-- Every prop of the selection's kind (the realm's selkind).
function B.SelectKind()
    if not B.state.selId then setStatus("Select a prop first."); return false end
    if not B.Can("selkind") then setStatus("Selecting all of a kind needs a Den update."); return false end
    return B.Command("selkind")
end

-- Lock: it can't be moved or deleted by accident (the full Den; Den Lite once it says caps=lock).
function B.Lock()
    if not B.Can("lock") then setStatus("Lock needs a Den update."); return false end
    return B.Command("lock")
end

-- The selection's size in yards: the catalog's box of its kind times its size (or nil when the box isn't known).
function B.SelSize(s)
    s = s or B.state
    local e = s.selEntry
    if not e then return nil end
    local c
    for _, it in ipairs(B.catalog) do if it.id == e then c = it; break end end
    if not c then local k = B.known and B.known[e]; c = k end
    local box = c and c.size
    if not box then return nil end
    local k = (s.p and s.p.scale) or 1
    return { w = box.w * k, d = box.d * k, h = box.h * k }
end
function B.SizeText(s)
    local z = B.SelSize(s)
    s = s or B.state
    local k = (s.p and s.p.scale) or 1
    if not z then return s.p and string.format("Size %.2f (its box isn't known)", k) or "" end
    return string.format("%.2f x %.2f x %.2f yd   (size %.2f)", z.w, z.d, z.h, k)
end

-- ---------------------------------------------------------------- Files: builds, prefabs, the plot

local function nameIn(box)
    local t = ((box and box:GetText()) or ""):gsub("%s+", "")
    if t == "" or not t:match("^[%w_%-]+$") or #t > 24 then return nil end
    return t
end
local buildFiles
local function boxName() if not files then buildFiles() end return nameIn(files.code) end

function B.Save()
    local n = boxName()
    if not n then setStatus("Type a name in the box (letters, digits, - and _)."); return false end
    return B.Command("save " .. n)
end

function B.Load()
    local n = boxName()
    if not n then setStatus("Type a build name or a code in the box."); return false end
    return B.Command("load " .. n)
end

function B.Share()
    local n = boxName()
    if not n then setStatus("Type the name of one of your builds in the box."); return false end
    return B.Command("share " .. n)
end

-- copy / paste: in edit mode the next ground click is the paste point
function B.Paste()
    if editing() and W.EditMode.StartPasting then W.EditMode.StartPasting(); setStatus("Click the ground where it goes."); return true end
    return B.Command("paste")
end

-- prefabs
function B.PastePrefab(name)
    if not name then return false end
    if editing() and W.EditMode.StartPrefab then
        W.EditMode.StartPrefab(name)
        setStatus("Click the ground where the prefab goes.")
        return true
    end
    return B.Command("prefab paste " .. name)
end

local function prefabName()
    local n = files and nameIn(files.prefab)
    if not n then setStatus("Type a prefab name in the box (letters, digits, - and _)."); return nil end
    return n
end

local function plotName()
    local n = files and nameIn(files.plotName)
    if not n then setStatus("Type a character name in the box."); return nil end
    return n
end

-- ---------------------------------------------------------------- docking (edit mode hides UIParent)

-- Puts frame on the world frame while editing (UIParent is hidden), back on UIParent after.
function B.Dock(frame, toWorld)
    if not frame then return end
    local parent = (toWorld and WorldFrame) or UIParent
    if frame:GetParent() ~= parent then
        frame:SetParent(parent)
        if toWorld and UIParent.GetEffectiveScale and WorldFrame and WorldFrame.GetEffectiveScale then
            local a, b = UIParent:GetEffectiveScale(), WorldFrame:GetEffectiveScale()
            frame:SetScale((tonumber(a) or 1) / math.max(0.01, tonumber(b) or 1))
        else
            frame:SetScale(1)
        end
        frame:SetFrameStrata("DIALOG")
    end
end

-- ---------------------------------------------------------------- the status pill

local MODE_TEXT = { select = "Select", place = "Place", move = "Move", rotate = "Rotate", path = "Path", stamps = "Stamps" }

-- "Place · Barrel · snap 1 yd · 15° · 3 props selected"
function B.StatusLine()
    local s = B.state
    if B.armed then
        return "Place: " .. B.armed.name .. " · Enter, double-click or Place"
    end
    local parts = { MODE_TEXT[B.mode or "place"] or "Place" }
    if B.pick and (B.mode == "place" or B.mode == nil) then parts[#parts + 1] = B.pick.name end
    if (B.mode == "move") then parts[#parts + 1] = "step " .. STEPS[B.step] .. " yd" end
    local g = B.Snap()
    parts[#parts + 1] = "snap " .. ((g and g > 0) and (g .. " yd") or "off")
    parts[#parts + 1] = ANGLE_STEPS[B.angle] .. "°"
    if s.count and s.count > 1 then parts[#parts + 1] = s.count .. " props selected" end
    if s.den and s.room then parts[#parts + 1] = "room " .. s.room
    elseif s.mine then parts[#parts + 1] = "props " .. s.mine end
    return table.concat(parts, " · ")
end

-- The pill: one line at the bottom centre, above the action bars, saying what the editor is doing; over it, for a
-- few seconds, the last thing the realm (Den Lite) said or the hint for what the mouse is on. Every answer goes here,
-- none to the chat (DenBar.lua filters the Den's system lines into it too).
local NOTE_SECS = 6
local function buildPill()
    pill = CreateFrame("Frame", "LeylineEditorStatus", UIParent)
    pill:SetWidth(560); pill:SetHeight(30)
    pill:SetPoint("BOTTOM", UIParent, "BOTTOM", 0, LAYOUT.pillBottom)
    pill:SetFrameStrata("MEDIUM")
    S().EdPanel(pill, 0.86)
    pill.line = S().EdText(pill, S().ED_SIZE.text)
    pill.line:SetPoint("CENTER", 0, 0)
    pill.line:SetHeight(18)   -- one line: a long one ends in "..."
    pill.note = S().EdText(pill, S().ED_SIZE.text)
    pill.note:SetPoint("BOTTOM", pill, "TOP", 0, 6)
    pill.note:SetWidth(720)
    pill.note.outside = true   -- over the pill, on purpose
    local m = S().ED.muted
    pill.note:SetTextColor(m[1], m[2], m[3])
    pill.note:SetText(B.status or "")
    -- the selection, boxed in gold at the pill's right: its name (and how many)
    pill.sel = CreateFrame("Button", nil, pill)
    pill.sel:SetHeight(30)
    pill.sel:SetPoint("LEFT", pill, "RIGHT", 6, 0)
    pill.sel.outside = true
    S().EdPanel(pill.sel, 0.9, S().ED.panel2)
    S().EdEdge(pill.sel, S().ED.gold)
    pill.sel.label = S().EdText(pill.sel, S().ED_SIZE.text)
    pill.sel.label:SetPoint("CENTER", 0, 0)
    local g = S().ED.gold
    pill.sel.label:SetTextColor(g[1], g[2], g[3])
    pill.sel:SetScript("OnClick", function() B.ShowProps(true) end)
    pill.sel.edLabel = "The selection: its panel"
    pill.sel:Hide()
    pill:SetScript("OnUpdate", function()
        if B.statusAt and now() - B.statusAt > NOTE_SECS and pill.note:GetText() ~= "" then pill.note:SetText("") end
    end)
    pill:Hide()
end
refreshPill = function()
    if not pill then return end
    local line = B.StatusLine()
    local D = W.DenBar
    if D and D.IsActive and D.IsActive() and D.SavedLine then line = line .. "  ·  " .. D.SavedLine() end
    -- never wider than the room between the chat (its right edge 462 at the stock place) and the screen's middle;
    -- a line too long loses the "Saved" note first, then ends in "..." (nothing cut mid-letter)
    local sw = (GetScreenWidth and tonumber((GetScreenWidth()))) or 1366
    local most = math.max(360, math.min(900, sw - 2 * 470))
    pill.line:SetText(line)
    local w = pill.line.GetStringWidth and tonumber((pill.line:GetStringWidth()))
    if w and w + 40 > most then
        line = B.StatusLine()
        pill.line:SetText(line)
        w = tonumber((pill.line:GetStringWidth()))
    end
    local pw = math.max(math.min(420, most), math.min(most, (w or (#line * 7)) + 40))
    pill:SetWidth(pw)
    pill.line:SetWidth(pw - 24)
    if w and w > pw - 24 then S().Elide(pill.line, line, pw - 24) end
    -- the selection's box, and the pill's edge gold while building in a Den (build mode), the subtle edge elsewhere
    local s = B.state
    local name = s.selName and ((s.count or 1) > 1 and (s.count .. " selected: " .. s.selName) or s.selName)
    if name then
        pill.sel.label:SetText(name)
        local tw = pill.sel.label.GetStringWidth and tonumber((pill.sel.label:GetStringWidth())) or (#name * 7)
        pill.sel:SetWidth(math.min(240, tw + 24))
        pill.sel.label:SetWidth(math.min(240, tw + 24) - 16)
        pill.sel:Show()
    else
        pill.sel:Hide()
    end
    local building = D and D.IsActive and D.IsActive() and not D.play
    S().EdEdge(pill, building and S().ED.gold or S().ED.edge)
end
function B.Pill() if not pill then buildPill() end return pill end
-- a note over the pill (a toast's colour, or muted): DenBar.lua's toasts and the Builder's own hints
function B.PillNote(text, colour)
    B.Pill()
    B.status, B.statusAt = text or "", now()
    pill.note:SetText(text or "")
    local c = colour or S().ED.muted
    pill.note:SetTextColor(c[1], c[2], c[3])
end
function B.ShowPill(show)
    B.Pill()
    B.Dock(pill, editing())
    if show then refreshPill(); pill:Show() else pill:Hide() end
end

-- ---------------------------------------------------------------- the properties popover

local FIELDS = { { "x", "X" }, { "y", "Y" }, { "z", "Z" }, { "turn", "Turn" }, { "pitch", "Pitch" }, { "roll", "Roll" },
    { "scale", "Size" } }
local ANGLE_FIELDS = { turn = true, pitch = true, roll = true }

-- The preview on top of the selection panel: the selected prop's model (the catalog's), or the pick in hand; its
-- category's icon when it has no model.
function B.PreviewFill()
    local pv = panel and panel.preview
    if not pv then return end
    local s = B.state
    local c = B.inHand or (s.p and s.selEntry and (B.ById and B.ById(s.selEntry)))
    local path = c and c.model
    pv:Show()
    if not path then
        if pv.modelPath then pv.modelPath = nil; if pv.model.ClearModel then pcall(pv.model.ClearModel, pv.model) end end
        pv.model:Hide()
        S().SetIcon(pv.icon, iconName(c and (c.wmo and WMO_ICON or c.cat) or "noimage"), S().ED.muted)
        pv.icon:Show()
        return
    end
    pv.icon:Hide()
    pv.model:Show()
    if pv.modelPath ~= path then
        pv.modelPath = path
        pv.turn, pv.zoom, pv.drag = 0, 1, nil
        if pv.model.ClearModel then pcall(pv.model.ClearModel, pv.model) end
        pcall(pv.model.SetModel, pv.model, path)
        if pv.model.SetLight then pcall(pv.model.SetLight, pv.model, 1, 0, -0.5, -0.7, -0.5, 0.75, 1, 1, 1, 0.9, 1, 0.95, 0.85) end
    end
    applyView(pv)
end

-- What the quick buttons need from the realm (Den Lite's caps); a realm without it: the button off.
local QUICK_VERBS = { Upright = "upright", Settle = "settle", Random = "random", Centre = "centre", Reset = "reset", Swap = "swap", SwapKind = "swapkind" }

function B.PropsFill()
    if not panel then return end
    local s = B.state
    local p = s.p
    B.PreviewFill()
    local hand = B.inHand
    if hand then
        panel.body:Hide(); panel.handText:Show()
        panel.title2:SetText(hand.name or "")
        panel.stateText:SetText("in hand · click the ground")
        return
    end
    panel.body:Show(); panel.handText:Hide()
    panel.title2:SetText(p and ((s.count or 1) > 1 and (s.count .. " props (numbers for the last one)") or (s.selName or "")) or "Nothing selected")
    panel.stateText:SetText(p and "selected" or "")
    panel.size:SetText(p and B.SizeText(s) or "Click a prop, Nearest, or place one from the gallery.")
    panel.selTitle:SetText(("Selection · " .. tostring(p and (s.count or 1) or 0)):upper())
    for si, c in pairs(panel.stepChips or {}) do c:SetActive(si == B.step) end
    for a, c in pairs(panel.angleChips or {}) do c:SetActive(a == ANGLE_STEPS[B.angle]) end
    local lockOk = B.Can("lock")
    if panel.actions.Lock then
        if lockOk then panel.actions.Lock:Enable() else panel.actions.Lock:Disable() end
        panel.actions.Lock.edLabel = lockOk and "Lock: it can't be moved or deleted by accident" or "Lock needs a Den update"
    end
    for key, verb in pairs(QUICK_VERBS) do
        local b = panel.actions[key] or panel.swap[key]
        if b then if B.Can(verb) then b:Enable() else b:Disable() end end
    end
    if panel.sel and panel.sel.Kind then
        if B.Can("selkind") then panel.sel.Kind:Enable() else panel.sel.Kind:Disable() end
        panel.sel.Kind.edLabel = B.Can("selkind") and "Every prop of this kind" or "Needs a Den update"
    end
    -- Swap with: the gallery's pick
    local pk = B.pick and not B.pick.kind and B.pick or nil
    S().SetIcon(panel.swapIcon, iconName(pk and (pk.wmo and WMO_ICON or pk.cat) or "noimage"), pk and S().ED.text or S().ED.muted)
    S().Elide(panel.swapName, pk and pk.name or "Click a card in the gallery", panel.swapNameW)
    if p then
        if panel.turnSlider then panel.turnSlider:Put((p.turn or 0) % 360); panel.turnText:SetText(string.format("%.0f°", (p.turn or 0) % 360)) end
        if panel.scaleSlider then panel.scaleSlider:Put(math.max(0.1, math.min(5, p.scale or 1))) end
    end
    if B.propsEditing then return end
    for _, f in ipairs(FIELDS) do
        local v = p and p[f[1]]
        panel.box[f[1]]:SetText(v and (ANGLE_FIELDS[f[1]] and string.format("%.0f", v) or string.format("%.2f", v)) or "")
    end
end

local function angleDiff(a, b)
    local d = (a - b) % 360
    if d > 180 then d = 360 - d end
    return d
end

function B.ApplyProps()
    local p = B.state.p
    B.propsEditing = nil
    if not p then setStatus("Select a prop first."); return false end
    local v = {}
    for _, f in ipairs(FIELDS) do
        v[f[1]] = tonumber((panel.box[f[1]]:GetText() or ""):match("^%s*(.-)%s*$"))
        if not v[f[1]] or v[f[1]] ~= v[f[1]] then setStatus("Type numbers in the boxes."); return false end
    end
    if math.abs(v.pitch) > 180 or math.abs(v.roll) > 180 then setStatus("Pitch and roll go from -180 to 180."); return false end
    local s = B.state
    local moved = math.abs(v.x - p.x) > 0.004 or math.abs(v.y - p.y) > 0.004 or math.abs(v.z - p.z) > 0.004
    local turned = angleDiff(v.turn, p.turn) > 0.5
    local tilted = math.abs(v.pitch - (p.pitch or 0)) > 0.5 or math.abs(v.roll - (p.roll or 0)) > 0.5
    local sized = math.abs(v.scale - p.scale) > 0.004
    local sent = 0
    if (s.count or 1) <= 1 and s.selId then
        -- one prop: position and rotation in one change, then the size
        -- unchanged boxes keep the realm's finer numbers (the boxes show whole degrees)
        local yaw = turned and v.turn or (p.yaw or p.turn)
        local pitch, roll = v.pitch, v.roll
        if not tilted then pitch, roll = p.pitch or 0, p.roll or 0 end
        if moved or turned or tilted then B.MoveTo(s.selId, v.x, v.y, v.z, yaw, pitch, roll); sent = sent + 1 end
        if sized then B.Command(string.format("scale %d %.2f", s.selId, v.scale)); sent = sent + 1 end
    else
        if moved then B.Command(string.format("setpos %.2f %.2f %.2f", v.x, v.y, v.z)); sent = sent + 1 end
        if turned then B.Command(string.format("setturn %.0f", v.turn % 360)); sent = sent + 1 end
        if sized then B.Command(string.format("setscale %.2f", v.scale)); sent = sent + 1 end
        if tilted then setStatus("Tilt one prop at a time."); return sent > 0 end
    end
    if sent == 0 then setStatus("Nothing changed.") end
    if tilted and not s.core then setStatus("Tilt needs a realm update; the rest was sent.") end
    return sent > 0
end

-- Nudges: x / y / z by the move step, turn / pitch / roll by the turn step (each prop about itself for pitch and
-- roll; turn turns a group around its middle), size by 0.1.
function B.Nudge(field, sign)
    local p = B.state.p
    if not p then setStatus("Select a prop first."); return false end
    if field == "x" or field == "y" or field == "z" then
        return B.Command(string.format("move %s %s", field, tostring(sign * STEPS[B.step])))
    end
    local a = sign * ANGLE_STEPS[B.angle]
    if field == "turn" then return B.Command("turn " .. a) end
    if field == "pitch" or field == "roll" then
        if not B.state.core then setStatus("Tilt needs a realm update; turning works.") return false end
        return B.Command("tilt " .. field .. " " .. a)
    end
    if field == "scale" then
        local v = math.floor(((p.scale or 1) + sign * SIZE_STEP) * 100 + 0.5) / 100
        return B.Command(string.format("setscale %.2f", v))
    end
    return false
end

-- Path: a line of the picked prop, one every B.PATH_EVERY yards along the ground (.build path): in edit mode two
-- clicks (where it starts, where it ends); on foot the first press marks where you stand, the second lays the path
-- to where you stand then. One group and one undo entry (ov_build.lua).
B.PATH_EVERY = 3
function B.Path()
    if not B.pick or B.pick.kind then setStatus("Pick the prop the path is made of first (a platform, a rug, a lantern...)."); return false end
    local why = B.Blocked(B.pick)
    if why then setStatus(why); return false end
    remember(B.pick)
    B.SetMode("path")
    if editing() and W.EditMode.StartPath then
        W.EditMode.StartPath(B.pick, B.PATH_EVERY)
        setStatus("Click where the path starts, then where it ends.")
        return true
    end
    if B.pathFrom == B.pick.id then
        B.pathFrom = nil
        return B.Command(string.format("path %d to %d", B.pick.id, B.PATH_EVERY))
    end
    B.pathFrom = B.pick.id
    setStatus("Start marked where you stand: walk to where the path ends, then press Path again.")
    return B.Command("path " .. B.pick.id .. " from")
end

-- Stack: in edit mode, the next click on a prop is what the selection goes on top of; outside it, the first
-- prop picked is the one the rest go on.
function B.Stack()
    if editing() and W.EditMode.StartStack then W.EditMode.StartStack(); return true end
    return B.Command("stack")
end

-- The keys while something is selected (owner 3 Nov, his editor's list): Del deletes, U copies, Ins swaps it for the
-- gallery's pick (Shift+Ins: all of its kind), , and . turn it, Page Up / Down lift it by the step, Home brings it to
-- you, End puts it on the ground, Ctrl+Z / Ctrl+Shift+Z undo and redo, Tab the next, Esc lets go; numpad + / - size it.
-- Override bindings to hidden buttons, only while there is a selection and its panel is open, given back after; never
-- the movement keys (WASD, Q / E, the arrows) or the camera's mouse.
B.KEYS = {
    DELETE = function() return B.Command("delete") end,
    U = function() return B.Command("copy") end,
    INSERT = function() return B.Swap(false) end, ["SHIFT-INSERT"] = function() return B.Swap(true) end,
    [","] = function() return B.Nudge("turn", 1) end, ["."] = function() return B.Nudge("turn", -1) end,
    PAGEUP = function() return B.Nudge("z", 1) end, PAGEDOWN = function() return B.Nudge("z", -1) end,
    HOME = function() return B.Pad("bring") end, END = function() return B.Command("floor") end,
    NUMPADPLUS = function() return B.Nudge("scale", 1) end, NUMPADMINUS = function() return B.Nudge("scale", -1) end,
    TAB = function() return B.SelectNext() end,
    ESCAPE = function() return B.Deselect() end,
    ["CTRL-Z"] = function() return B.Command("undo") end, ["CTRL-SHIFT-Z"] = function() return B.Command("redo") end,
    ["CTRL-Y"] = function() return B.Command("redo") end,
    ["CTRL-C"] = function() return B.Command("copy") end, ["CTRL-V"] = function() return B.Paste() end,
    ["CTRL-D"] = function() return B.Command("dup") end,
    -- while a picked prop waits to be placed (B.Arm): Enter places it
    ENTER = function() return B.PlaceArmed() end, NUMPADENTER = function() return B.PlaceArmed() end,
}
local KEY_ORDER = { "DELETE", "U", "INSERT", "SHIFT-INSERT", ",", ".", "PAGEUP", "PAGEDOWN", "HOME", "END", "NUMPADPLUS", "NUMPADMINUS",
    "TAB", "ESCAPE", "CTRL-Z", "CTRL-SHIFT-Z", "CTRL-Y", "CTRL-C", "CTRL-V", "CTRL-D" }
B.KEY_TEXT = "Del delete · U copy · Ins swap · Shift+Ins all of its kind · , . turn · PgUp PgDn lift · Home bring here · End to ground · Ctrl+Shift+Z redo"
local ARM_KEYS = { "ENTER", "NUMPADENTER" }
B.KEY_ORDER, B.ARM_KEYS = KEY_ORDER, ARM_KEYS
local keyOwner, armOwner
-- a hidden button per key, named from the key ("-" and "=" are keys too)
local function keyButton(key)
    local name = "LeylinePropKey" .. key:gsub("[^%w]", function(c) return string.format("%02X", c:byte()) end)
    if _G[name] then return name end
    local b = CreateFrame("Button", name, UIParent)
    b:EnableMouse(false)
    if b.RegisterForClicks then b:RegisterForClicks("AnyDown") end
    b:SetScript("OnClick", function(_, _, down) if down ~= false then B.KEYS[key]() end end)
    return name
end
B.KeyButton = keyButton
-- The selection's keys while its panel is open and something is selected (not in the browser's edit mode, whose
-- camera has its own keys).
function B.BindKeys(on)
    if not SetOverrideBindingClick or not ClearOverrideBindings then return false end
    if not keyOwner then keyOwner = CreateFrame("Frame", "LeylinePropKeys", UIParent) end
    ClearOverrideBindings(keyOwner)
    B.keysOn = nil
    if on and B.state.selId and not editing() and not (InCombatLockdown and InCombatLockdown()) then
        for _, k in ipairs(KEY_ORDER) do SetOverrideBindingClick(keyOwner, true, k, keyButton(k)) end
        B.keysOn = true
    end
    return B.keysOn == true
end
-- Enter while a picked prop waits to be placed (and given back as soon as it is placed or let go).
function B.BindArmKeys(on)
    if not SetOverrideBindingClick or not ClearOverrideBindings then return false end
    if not armOwner then armOwner = CreateFrame("Frame", "LeylineArmKeys", UIParent) end
    ClearOverrideBindings(armOwner)
    B.armKeysOn = nil
    if on and not (InCombatLockdown and InCombatLockdown()) then
        for _, k in ipairs(ARM_KEYS) do SetOverrideBindingClick(armOwner, true, k, keyButton(k)) end
        B.armKeysOn = true
    end
    return B.armKeysOn == true
end

-- The pad (MOVE · TURN · SIZE from where you stand): forward is the way you face. Den Lite moves it from where you
-- stand (step); a realm without that: along the world's x and y by your facing.
B.PAD = {
    fwd = { "up", "Forward" }, back = { "down", "Back" }, left = { "left", "Left" }, right = { "right", "Right" },
    turnleft = { "turnleft", "Turn left" }, turnright = { "turnright", "Turn right" }, bring = { "bring", "Bring it here (Home)" },
    smaller = { "smaller", "Smaller" }, bigger = { "bigger", "Bigger" }, up = { "liftup", "Lift up (Page Up)" }, down = { "liftdown", "Lift down (Page Down)" },
}
function B.Pad(what)
    if not B.state.selId then setStatus("Select a prop first."); return false end
    local v = STEPS[B.step]
    if what == "turnleft" then return B.Nudge("turn", 1) end
    if what == "turnright" then return B.Nudge("turn", -1) end
    if what == "up" then return B.Nudge("z", 1) end
    if what == "down" then return B.Nudge("z", -1) end
    if what == "smaller" or what == "bigger" then
        if B.Can("grow") then return B.Command(what == "bigger" and "grow 1.25" or "grow 0.8") end
        return B.Nudge("scale", what == "bigger" and 1 or -1)
    end
    if what == "bring" then
        if B.Can("bring") then return B.Command("bring") end
        setStatus("Bring here needs a Den update."); return false
    end
    if B.Can("step") then return B.Command("step " .. what .. " " .. v) end
    -- the full Den: from your facing, along x and y
    local o = (GetPlayerFacing and GetPlayerFacing()) or 0
    local s, c = math.sin(o), math.cos(o)
    local d = ({ fwd = { c, s }, back = { -c, -s }, left = { -s, c }, right = { s, -c } })[what]
    if not d then return false end
    local sent = false
    for i, axis in ipairs({ "x", "y" }) do
        local m = math.floor(d[i] * v * 100 + 0.5) / 100
        if m ~= 0 then B.Command("move " .. axis .. " " .. m); sent = true end
    end
    return sent
end

-- Swap with (the gallery's pick): the selection becomes it, where it stands (turn and size kept); all: every prop of
-- the selection's kind (one undo). Picking the card armed it for placing: Swap puts that away.
function B.Swap(all)
    local it = B.pick
    if not B.state.selId then setStatus("Select a prop first."); return false end
    if not it or it.kind then setStatus("Click a card in the gallery first: Swap puts that one in its place."); return false end
    local verb = all and "swapkind" or "swap"
    if not B.Can(verb) then setStatus("Swap needs a Den update."); return false end
    if W.Gizmo and W.Gizmo.armed == "place" and W.Gizmo.Cancel then W.Gizmo.Cancel() end
    B.Arm(nil)
    B.HandDone()
    return B.Command(verb .. " " .. it.id)
end

-- The pick in hand (Den Lite's build mode: the real prop follows the cursor): the panel shows it, "in hand · click
-- the ground", until it is placed or put away.
function B.InHand(item)
    B.inHand = (item and not item.kind) and item or nil
    if B.inHand then B.ShowProps(true); B.propsAuto = true end
    B.PropsFill()
end
-- `placed`: the panel stays open for the new selection (its state comes next)
function B.HandDone(placed)
    if not B.inHand then return end
    B.inHand = nil
    if panel and panel:IsShown() then
        if B.state.selId or placed then B.PropsFill() elseif B.propsAuto then B.ShowProps(false) end
    end
end

-- Chips in a row, as wide as their words, wrapping at maxW: { {text, fn, key}, ... } -> the chips by key (or text).
local function flow(parent, list, x0, y0, maxW, into)
    local x, y = x0, y0
    into = into or {}
    for _, it in ipairs(list) do
        local c = chip(parent, it[1], nil, it[2])
        local w = tonumber((c:GetWidth())) or 60
        if x > x0 and x + w > x0 + maxW then x, y = x0, y - S().ED_SIZE.chip - 6 end
        c:SetPoint("TOPLEFT", parent, "TOPLEFT", x, y)
        x = x + w + 6
        into[it[3] or it[1]] = c
        c.only = it[4]
    end
    return into, y - S().ED_SIZE.chip
end
B.Flow = flow

-- The selection's panel, as the owner's own editor lays it out (owner 3 Nov, his video: "it should function like
-- this"): the prop's model on top (right-drag turns it, the wheel zooms), its name and "selected" (or "in hand ·
-- click the ground" while a pick waits to be placed); PIECE: POSITION x / y / z (red, green, blue; type and Enter,
-- or - / + and the wheel); MOVE · TURN · SIZE from where you stand (the pad: turn left, forward, turn right / left,
-- bring here, right / smaller, back, bigger; lift up and down; the step 0.1, 0.5, 1, 5 yd); ROTATION · SIZE (turn,
-- tilt, roll by the turn step 15 / 45 / 90; size; the sliders); the quick buttons (tilt, upright, to ground, settle,
-- random, centre, duplicate, reset, delete; Apply, Lock, Copy, Paste, More); SWAP WITH (the gallery's pick: Swap, All
-- of this kind); the keys; SELECTION (nearest, next, all of its kind, clear).
local PANEL_W, PANEL_PAD, COL_W = 436, 14, 136
local PV_H = 92
local AXIS_COLOUR = { x = { 0.93, 0.33, 0.31 }, y = { 0.40, 0.80, 0.36 }, z = { 0.36, 0.56, 0.95 } }
B.AXIS_COLOUR = AXIS_COLOUR
local function section(parent, text, y)
    local fs = smallCaps(parent, text, true, "TOPLEFT", PANEL_PAD, y)
    local rule = solid(parent, "BORDER", S().ED.edge)
    rule:SetHeight(1)
    rule:SetPoint("LEFT", fs, "RIGHT", 8, 0); rule:SetPoint("RIGHT", parent, "RIGHT", -PANEL_PAD, 0)
    rule:SetPoint("TOP", parent, "TOP", 0, y - 7)
    return fs, rule
end
-- a field: its letter, -, the number, + (the wheel over the number or the buttons nudges it); x / y / z in their colour
local function field(f, x, y, parent)
    parent = parent or panel.body
    local key = f[1]
    panel.rowLabel[key] = smallCaps(parent, f[3] or f[2], false, "TOPLEFT", x, y - 7)
    local lx = x + 18
    if f[3] and #f[3] > 1 then lx = x + 40 end
    local bw = COL_W - (lx - x) - 2 * 20 - 8
    panel.minus[key] = chip(parent, "-", 20, function() B.Nudge(key, -1) end, "TOPLEFT", lx, y)
    local e = editBox(parent, bw, 12, "TOPLEFT", lx + 22, y)
    e:SetHeight(28)
    e:SetScript("OnEditFocusGained", function() B.propsEditing = true end)
    e:SetScript("OnEnterPressed", function(self) self:ClearFocus(); B.ApplyProps() end)
    panel.box[key] = e
    local ac = AXIS_COLOUR[key]
    if ac then
        local edge = solid(e, "OVERLAY", { ac[1], ac[2], ac[3], 1 })
        edge:SetWidth(3); edge:SetPoint("TOPLEFT", e, "TOPLEFT", 0, 0); edge:SetPoint("BOTTOMLEFT", e, "BOTTOMLEFT", 0, 0)
        panel.rowLabel[key]:SetTextColor(ac[1], ac[2], ac[3])
    end
    panel.plus[key] = chip(parent, "+", 20, function() B.Nudge(key, 1) end, "TOPLEFT", lx + 22 + bw + 2, y)
    for _, w in ipairs({ e, panel.minus[key], panel.plus[key] }) do
        if w.EnableMouseWheel then w:EnableMouseWheel(true) end
        w:SetScript("OnMouseWheel", function(_, delta) B.Nudge(key, (tonumber(delta) or 0) > 0 and 1 or -1) end)
    end
end
-- a slider in the editor look: a track, a gold thumb; onCommit when let go (not every frame: one change to the realm)
local function slider(parent, w, lo, hi, step, onCommit, ...)
    local sl = CreateFrame("Slider", nil, parent)
    sl:SetWidth(w); sl:SetHeight(18)
    if sl.SetOrientation then sl:SetOrientation("HORIZONTAL") end
    sl:SetMinMaxValues(lo, hi)
    if sl.SetValueStep then sl:SetValueStep(step) end
    local track = solid(sl, "BACKGROUND", { 1, 1, 1, 0.12 })
    track:SetHeight(4); track:SetPoint("LEFT", 0, 0); track:SetPoint("RIGHT", 0, 0)
    local thumb = sl:CreateTexture(nil, "OVERLAY")
    thumb:SetTexture(W.Style.WHITE or "Interface\\Buttons\\WHITE8X8")
    local g = S().ED.gold
    thumb:SetVertexColor(g[1], g[2], g[3], 1)
    thumb:SetWidth(10); thumb:SetHeight(18)
    if sl.SetThumbTexture then sl:SetThumbTexture(thumb) end
    sl.thumb = thumb
    sl:EnableMouse(true)
    if sl.EnableMouseWheel then sl:EnableMouseWheel(true) end
    sl:SetScript("OnMouseUp", function(self) local v = tonumber((self:GetValue())); if v and not self.setting then onCommit(v) end end)
    sl:SetScript("OnMouseWheel", function(self, delta)
        local v = tonumber((self:GetValue())) or lo
        v = math.max(lo, math.min(hi, v + ((tonumber(delta) or 0) > 0 and step or -step)))
        self:Put(v)
        onCommit(v)
    end)
    sl.Put = function(self, v) self.setting = true; self:SetValue(v); self.setting = nil end
    if select("#", ...) > 0 then sl:SetPoint(...) end
    return sl
end
-- a square icon button of ours (32): its tooltip says what it does
local function iconBtn(parent, icon, label, fn, ...)
    local b = S().EdIcon(parent, 32, icon, label, fn, ...)
    b.edLabel = label
    return b
end

local function buildPanel()
    panel = CreateFrame("Frame", "WrathUnleashedPropsFrame", UIParent)
    panel:SetWidth(PANEL_W)
    panel:SetPoint("TOPRIGHT", UIParent, "TOPRIGHT", -LAYOUT.propsRight, -LAYOUT.propsTop)
    panel:SetFrameStrata("DIALOG")
    panel:EnableMouse(true)
    panel:SetMovable(true)
    panel:RegisterForDrag("LeftButton")
    panel:SetScript("OnDragStart", panel.StartMoving)
    panel:SetScript("OnDragStop", panel.StopMovingOrSizing)
    S().EdPanel(panel, 0.92, S().ED.panel2)
    local P = PANEL_PAD
    local m = S().ED.muted
    panel.box, panel.minus, panel.plus, panel.rowLabel = {}, {}, {}, {}

    -- the model on top, from the side (right-drag turns it, the wheel zooms); the category's icon without a model
    local pv = CreateFrame("Frame", "WrathUnleashedPreviewFrame", panel)
    pv:SetWidth(PANEL_W - 2 * P); pv:SetHeight(PV_H)
    pv:SetPoint("TOPLEFT", panel, "TOPLEFT", P, -8)
    pv:EnableMouse(true)
    S().EdPanel(pv, 0.6, S().ED.tile)
    pv.model = newViewModel(pv)
    pv.model:SetWidth(PV_H - 8); pv.model:SetHeight(PV_H - 8)
    pv.model:SetPoint("CENTER", pv, "CENTER", 0, 0)
    pv.model:SetScript("OnShow", function() applyView(pv) end)
    pv.icon = pv:CreateTexture(nil, "ARTWORK")
    pv.icon:SetWidth(44); pv.icon:SetHeight(44)
    pv.icon:SetPoint("CENTER", pv, "CENTER", 0, 0)
    pv.icon:Hide()
    pv.hint = S().EdText(pv, S().ED_SIZE.small)
    pv.hint:SetPoint("BOTTOMLEFT", 8, 6)
    pv.hint:SetWidth(112); pv.hint:SetHeight(32); pv.hint:SetJustifyH("LEFT")
    pv.hint:SetTextColor(m[1], m[2], m[3])
    pv.hint:SetText("Right-drag turns. Wheel zooms.")
    -- its size in yards, at the preview's foot
    panel.size = S().EdText(pv, S().ED_SIZE.small)
    panel.size:SetPoint("BOTTOMRIGHT", -8, 6)
    panel.size:SetWidth(PANEL_W - 2 * P - 136); panel.size:SetHeight(32); panel.size:SetJustifyH("RIGHT")
    panel.size:SetTextColor(m[1], m[2], m[3])
    local pvTick = B.AttachViewMouse(pv, pv)
    pv:SetScript("OnUpdate", function(self, elapsed)
        pvTick(self)
        -- Spin while the gallery (whose tick turns everything) is closed
        if not (win and win:IsShown()) then B.Turntable(elapsed) end
    end)
    panel.preview, B.preview = pv, pv
    panel.close = chip(panel, "x", 28, function() B.ShowProps(false) end, "TOPRIGHT", -P - 4, -12)
    panel.close.edLabel = "Close (Esc lets go of the selection)"
    panel.help = W.HelpButton(panel, {
        "Right-click a prop to select it; Shift+right-click adds one. Click empty ground to let go.",
        "Drag a coloured handle (right-click it, move the mouse, click to drop): red X, green Y, blue up.",
        "The pad moves it from where you stand: forward is the way you face. Page Up / Down lift it.",
        "Type exact numbers and Enter, or nudge with - and + (or the mouse wheel over them).",
        "Swap with: click a card in the gallery, then Swap (or All of this kind).",
    }, 460, "TOPRIGHT", -P - 36, -12)

    -- the name and what it is now: selected, or in hand
    local y = -8 - PV_H - 6
    panel.title2 = S().EdText(panel, S().ED_SIZE.title)
    panel.title2:SetPoint("TOPLEFT", P, y)
    panel.title2:SetWidth(PANEL_W - 2 * P - 170); panel.title2:SetJustifyH("LEFT")
    panel.title2:SetHeight(18)
    panel.stateText = S().EdText(panel, S().ED_SIZE.small)
    panel.stateText:SetPoint("TOPRIGHT", -P, y - 2)
    panel.stateText:SetWidth(166); panel.stateText:SetHeight(16); panel.stateText:SetJustifyH("RIGHT")
    panel.stateText:SetTextColor(m[1], m[2], m[3])
    -- while a pick is in hand: what to do (the rest waits for a selection)
    panel.handText = S().EdText(panel, S().ED_SIZE.small)
    panel.handText:SetPoint("TOPLEFT", P, y - 26)
    panel.handText:SetWidth(PANEL_W - 2 * P); panel.handText:SetHeight(48); panel.handText:SetJustifyH("LEFT")
    panel.handText:SetTextColor(m[1], m[2], m[3])
    panel.handText:SetText("It follows the mouse on the ground: click to place it, Shift+click to place more. Esc or a right-click puts it away.")
    panel.handText:Hide()

    -- everything about the selection (hidden while a pick is in hand): the tabs and their pages
    local body = CreateFrame("Frame", nil, panel)
    body:SetPoint("TOPLEFT", panel, "TOPLEFT", 0, 0); body:SetPoint("BOTTOMRIGHT", panel, "BOTTOMRIGHT", 0, 0)
    panel.body = body
    y = y - 24
    panel.tabs, panel.pages = {}, {}
    local tx = P
    for _, t in ipairs(B.PANEL_TABS) do
        local c = chip(body, t[2], nil, function() B.PanelTab(t[1]) end, "TOPLEFT", tx, y)
        c.edLabel = t[3]
        tx = tx + (tonumber((c:GetWidth())) or 80) + 6
        panel.tabs[t[1]] = c
        local pg = CreateFrame("Frame", nil, body)
        pg:SetPoint("TOPLEFT", body, "TOPLEFT", 0, 0); pg:SetPoint("BOTTOMRIGHT", body, "BOTTOMRIGHT", 0, 0)
        panel.pages[t[1]] = pg
    end
    local top = y - 38
    local piece, swapPg, selPg = panel.pages.piece, panel.pages.swap, panel.pages.select

    -- PIECE: POSITION
    y = top
    section(piece, "Position", y)
    y = y - 18
    for i, f in ipairs({ { "x", "X" }, { "y", "Y" }, { "z", "Z" } }) do field(f, P + (i - 1) * COL_W, y, piece) end

    -- MOVE · TURN · SIZE (from where you stand): the pad, the lift, the step; the turn's slider beside it
    y = y - 38
    section(piece, "Move · turn · size", y)
    panel.stepChips = {}
    local x = PANEL_W - P
    for i = #STEP_PRESETS, 1, -1 do
        local si = STEP_PRESETS[i]
        local c = chip(piece, tostring(STEPS[si]), nil, function() B.SetStep(si) end)
        c.edLabel = "Move and lift by " .. STEPS[si] .. " yd"
        local cw = tonumber((c:GetWidth())) or 44
        x = x - cw
        c:SetPoint("TOPRIGHT", piece, "TOPLEFT", x + cw, y + 6)
        x = x - 4
        panel.stepChips[si] = c
    end
    panel.stepBtn = chip(piece, "Step " .. STEPS[B.step] .. " yd", 96, function() B.SetStep(B.step % #STEPS + 1) end)
    panel.stepBtn.edLabel = "The move step: click for the next"
    panel.stepBtn:Hide()   -- the presets show the step; the chip stays for the keys' text and older callers
    panel.step = panel.stepBtn
    y = y - 22
    panel.pad = {}
    local PAD = { { "turnleft", "fwd", "turnright" }, { "left", "bring", "right" }, { "smaller", "back", "bigger" } }
    for r, row in ipairs(PAD) do
        for c, what in ipairs(row) do
            local d = B.PAD[what]
            panel.pad[what] = iconBtn(piece, d[1], d[2], function() B.Pad(what) end, "TOPLEFT", P + (c - 1) * 35, y - (r - 1) * 35)
        end
    end
    panel.pad.up = iconBtn(piece, B.PAD.up[1], B.PAD.up[2], function() B.Pad("up") end, "TOPLEFT", P + 3 * 35 + 8, y - 17)
    panel.pad.down = iconBtn(piece, B.PAD.down[1], B.PAD.down[2], function() B.Pad("down") end, "TOPLEFT", P + 3 * 35 + 8, y - 52)
    local rx = P + 4 * 35 + 20
    local rw = PANEL_W - P - rx
    panel.turnSlider = slider(piece, rw - 52, 0, 360, 5, function(v)
        local p = B.state.p
        if not p then return end
        panel.box.turn:SetText(string.format("%.0f", v))
        B.ApplyProps()
    end, "TOPLEFT", rx, y - 6)
    panel.turnText = S().EdText(piece, S().ED_SIZE.small)
    panel.turnText:SetPoint("TOPRIGHT", -P, y - 7)
    panel.turnText:SetWidth(46); panel.turnText:SetHeight(16); panel.turnText:SetJustifyH("RIGHT")
    panel.turnText:SetTextColor(m[1], m[2], m[3])
    panel.padText = S().EdText(piece, S().ED_SIZE.small)
    panel.padText:SetPoint("TOPLEFT", rx, y - 34)
    panel.padText:SetWidth(rw); panel.padText:SetHeight(64); panel.padText:SetJustifyH("LEFT")
    panel.padText:SetTextColor(m[1], m[2], m[3])
    panel.padText:SetText("Forward is the way you face; the middle brings it to you. Or drag its coloured handles.")

    -- ROTATION · SIZE: the turn step; turn, tilt, roll; size and its slider
    y = y - 3 * 35 - 6
    section(piece, "Rotation · size", y)
    panel.angleChips = {}
    x = PANEL_W - P
    for i = 3, 1, -1 do
        local a = ({ 15, 45, 90 })[i]
        local c = chip(piece, a .. "°", 44, function() B.SetAngle(a) end)
        c.edLabel = ", and . turn by " .. a .. "°"
        x = x - 44
        c:SetPoint("TOPRIGHT", piece, "TOPLEFT", x + 44, y + 6)
        x = x - 4
        panel.angleChips[a] = c
    end
    panel.angle = chip(piece, "Turn " .. ANGLE_STEPS[B.angle] .. "°", 100, function()
        B.SetAngle(ANGLE_STEPS[B.angle % #ANGLE_STEPS + 1])
    end)
    panel.angle:Hide()
    y = y - 20
    for i, f in ipairs({ { "turn", "Turn", "Turn" }, { "pitch", "Pitch", "Tilt" }, { "roll", "Roll", "Roll" } }) do field(f, P + (i - 1) * COL_W, y, piece) end
    y = y - 32
    field({ "scale", "Size", "" }, P, y, piece)
    panel.scaleSlider = slider(piece, PANEL_W - 2 * P - COL_W - 8, 0.1, 5, 0.05, function(v)
        if not B.state.p then return end
        panel.box.scale:SetText(string.format("%.2f", v))
        B.ApplyProps()
    end, "TOPLEFT", P + COL_W + 8, y - 5)

    -- the quick buttons (our icons): tilt, upright, to ground, settle, random, centre, duplicate, reset, delete; Apply
    y = y - 36
    panel.actions = {}
    local QUICK = {
        { "Tilt", "tilt", "Tilt it by the turn step", function() B.Nudge("pitch", 1) end },
        { "Upright", "upright", "Upright: no tilt", function() B.Command("upright") end },
        { "Floor", "floor", "To the ground under it (End)", function() B.Command("floor") end },
        { "Settle", "settle", "Settle: on the ground and upright", function() B.Command("settle") end },
        { "Random", "random", "A random turn", function() B.Command("random") end },
        { "Centre", "centre", "To the Den's middle", function() B.Command("centre") end },
        { "Duplicate", "duplicate", "A copy next to it (Ctrl+D)", function() B.Command("dup") end },
        { "Reset", "reset", "Reset: no turn, no tilt, size 1", function() B.Command("reset") end },
        { "Delete", "delete", "Delete it (Del)", function() B.Command("delete") end },
    }
    for i, q in ipairs(QUICK) do
        panel.actions[q[1]] = iconBtn(piece, q[2], q[3], q[4], "TOPLEFT", P + (i - 1) * 35, y)
    end
    panel.actions.Apply = chip(piece, "Apply", nil, function() B.ApplyProps() end)
    panel.actions.Apply:SetPoint("TOPRIGHT", piece, "TOPRIGHT", -P, y - 2)
    panel.actions.Apply.edLabel = "Send the numbers you typed (Enter in a box does too)"
    panel.apply = panel.actions.Apply
    local bottom = y - 34

    -- SWAP: the gallery's pick; Swap (Ins), All of this kind (Shift+Ins); Lock, Copy, Paste, More; the keys
    y = top
    section(swapPg, "Swap with", y)
    y = y - 18
    local sw = CreateFrame("Frame", nil, swapPg)
    sw:SetWidth(40); sw:SetHeight(40)
    sw:SetPoint("TOPLEFT", swapPg, "TOPLEFT", P, y)
    S().EdPanel(sw, 0.6, S().ED.tile)
    panel.swapIcon = sw:CreateTexture(nil, "ARTWORK")
    panel.swapIcon:SetPoint("TOPLEFT", 4, -4); panel.swapIcon:SetPoint("BOTTOMRIGHT", -4, 4)
    panel.swapNameW = PANEL_W - 2 * P - 48
    panel.swapName = S().EdText(swapPg, S().ED_SIZE.small)
    panel.swapName:SetPoint("TOPLEFT", P + 48, y - 2)
    panel.swapName:SetWidth(panel.swapNameW); panel.swapName:SetHeight(16); panel.swapName:SetJustifyH("LEFT")
    panel.swap = {}
    panel.swap.Swap = chip(swapPg, "Swap", 76, function() B.Swap(false) end, "TOPLEFT", P + 48, y - 20)
    panel.swap.Swap.edLabel = "Put the gallery's pick in its place (Ins)"
    panel.swap.SwapKind = chip(swapPg, "All of this kind", nil, function() B.Swap(true) end, "TOPLEFT", P + 48 + 82, y - 20)
    panel.swap.SwapKind.edLabel = "Every prop of this kind becomes the pick, one undo (Shift+Ins)"
    y = y - 60
    section(swapPg, "Copy and keep", y)
    local ay
    _, ay = flow(swapPg, {
        { "Lock", function() B.Lock() end }, { "Copy", function() B.Command("copy") end },
        { "Paste", function() B.Paste() end }, { "More", function() B.PropsMore() end },
    }, P, y - 18, PANEL_W - 2 * P, panel.actions)
    y = ay - 12
    section(swapPg, "Keys", y)
    panel.hint = S().EdText(swapPg, S().ED_SIZE.small)
    panel.hint:SetPoint("TOPLEFT", P, y - 18)
    panel.hint:SetWidth(PANEL_W - 2 * P); panel.hint:SetHeight(80); panel.hint:SetJustifyH("LEFT")
    panel.hint:SetTextColor(m[1], m[2], m[3])
    panel.hint:SetText(B.KEY_TEXT)
    bottom = math.min(bottom, y - 18 - 80)

    -- SELECTION: nearest, the next near you (Tab), all of its kind, nothing; how to select with the mouse
    y = top
    panel.selTitle = section(selPg, "Selection · 0", y)
    local sy
    panel.sel, sy = flow(selPg, {
        { "Nearest", function() B.Command("near") end, "Near" }, { "Next (Tab)", function() B.SelectNext() end, "Next" },
        { "All of its kind", function() B.SelectKind() end, "Kind" }, { "Clear (Esc)", function() B.Deselect() end, "Clear" },
    }, P, y - 18, PANEL_W - 2 * P)
    panel.selText = S().EdText(selPg, S().ED_SIZE.small)
    panel.selText:SetPoint("TOPLEFT", P, sy - 10)
    panel.selText:SetWidth(PANEL_W - 2 * P); panel.selText:SetHeight(64); panel.selText:SetJustifyH("LEFT")
    panel.selText:SetTextColor(m[1], m[2], m[3])
    panel.selText:SetText("Right-click a prop: selected. Shift+right-click adds one. A click on empty ground lets go. Del deletes them all.")
    bottom = math.min(bottom, sy - 10 - 64)

    panel.status = S().EdText(panel, S().ED_SIZE.small)
    panel.status:SetPoint("TOPLEFT", P, bottom - 6)
    panel.status:SetWidth(PANEL_W - 2 * P); panel.status:SetJustifyH("LEFT")
    panel.status:SetHeight(16)
    panel:SetHeight(-(bottom - 6) + 24)
    B.PanelTab(B.panelTab or "piece")

    -- More: the rest of the selection's tools, beside the panel (what only the full Den does is hidden on Den Lite)
    local MW = PANEL_W - 180
    panel.tools = {}
    panel.more = CreateFrame("Frame", nil, panel)
    panel.more:SetWidth(MW)
    panel.more:SetPoint("TOPRIGHT", panel, "TOPLEFT", -4, 0)
    panel.more.outside = true
    S().EdPanel(panel.more, 0.92, S().ED.panel2)
    local function cmdFn(c) return function() B.Command(c) end end
    local _, my = flow(panel.more, {
        { "Select my last", cmdFn("selall"), "Select mine" },
        -- Den Lite does these too since 4 Nov (its caps: buildtools.rs); "full": only the full Den's
        { "Unlock", cmdFn("unlock"), nil, "unlock" }, { "Hide", cmdFn("hide"), nil, "hide" },
        { "Show all", cmdFn("show all"), nil, "show" }, { "Measure", cmdFn("measure"), nil, "measure" },
        { "Align X", cmdFn("align x"), nil, "align" }, { "Align Y", cmdFn("align y"), nil, "align" }, { "Align Z", cmdFn("align z"), nil, "align" },
        { "Bottoms", cmdFn("align bottom"), nil, "align" }, { "Stack on", B.Stack, nil, "full" },
        { "Group", cmdFn("group"), nil, "group" }, { "Ungroup", cmdFn("ungroup"), nil, "group" }, { "Put away", cmdFn("store"), nil, "full" },
    }, P, -12, MW - 2 * P, panel.tools)
    panel.tools.Near = panel.sel.Near
    panel.tools.Clear = panel.sel.Clear
    panel.tools.Lock = panel.actions.Lock
    panel.tools.Duplicate = panel.actions.Duplicate
    panel.tools.Floor = panel.actions.Floor
    panel.tools.Undo = chip(panel.more, "Undo", 60, cmdFn("undo")); panel.tools.Undo:Hide()
    panel.tools.Redo = chip(panel.more, "Redo", 60, cmdFn("redo")); panel.tools.Redo:Hide()
    panel.more:SetHeight(-my + 14)
    panel.more:SetScript("OnShow", function()
        local lite = B.state.lite == "1"
        local list = {}
        for _, k in ipairs({ "Select mine", "Unlock", "Hide", "Show all", "Measure", "Align X", "Align Y", "Align Z", "Bottoms", "Stack on",
            "Group", "Ungroup", "Put away" }) do
            local c = panel.tools[k]
            -- on Den Lite: what it says it does (caps), never what only the full Den does
            local off = lite and (c and (c.only == "full" or (c.only and not B.Can(c.only))))
            if c then if off then c:Hide() else c:Show(); list[#list + 1] = c end end
        end
        local xx, yy = P, -12
        for _, c in ipairs(list) do
            local w = tonumber((c:GetWidth())) or 60
            if xx > P and xx + w > MW - P then xx, yy = P, yy - S().ED_SIZE.chip - 6 end
            c:ClearAllPoints(); c:SetPoint("TOPLEFT", panel.more, "TOPLEFT", xx, yy)
            xx = xx + w + 6
        end
        panel.more:SetHeight(-(yy - S().ED_SIZE.chip) + 14)
    end)
    panel.more:Hide()
    panel:SetScript("OnShow", function() B.propsEditing = nil; B.PropsFill(); B.BindKeys(true) end)
    panel:SetScript("OnHide", function() B.BindKeys(false); B.propsAuto = nil end)
    panel:Hide()
end

-- The panel's tabs (owner's editor: a row of tabs under the preview): Piece (position, the pad, rotation and size, the
-- quick buttons), Swap (swap with, lock, copy and paste, the keys), Select.
B.PANEL_TABS = { { "piece", "Piece", "Where it is, the pad, its turn and size" }, { "swap", "Swap · keys", "Swap it for another, copy, the keys" },
    { "select", "Select", "Nearest, next, all of its kind" } }
function B.PanelTab(which)
    if not panel then return false end
    if not panel.pages[which] then which = "piece" end
    B.panelTab = which
    for k, pg in pairs(panel.pages) do if k == which then pg:Show() else pg:Hide() end end
    for k, c in pairs(panel.tabs) do c:SetActive(k == which) end
    return true
end

function B.PropsMore(show)
    if not panel then buildPanel() end
    if show == nil then show = not panel.more:IsShown() end
    if show then panel.more:Show() else panel.more:Hide() end
    panel.actions.More:SetActive(show)
    return show
end

function B.ShowProps(show)
    if not panel then buildPanel() end
    B.Dock(panel, editing())
    if show then panel:Show() else panel:Hide() end
    chromeRefresh()
end
function B.ToggleProps() B.ShowProps(not (panel and panel:IsShown())) end

-- Move and Rotate on the rail: the popover, with that row lit.
function B.Focus(what)
    B.ShowProps(true)
    local gold = { x = what == "move", y = what == "move", z = what == "move", turn = what == "rotate", pitch = what == "rotate", roll = what == "rotate" }
    local ED = S().ED
    for k, fs in pairs(panel.rowLabel) do
        local c = AXIS_COLOUR[k] or (gold[k] and ED.gold or ED.muted)
        fs:SetTextColor(c[1], c[2], c[3])
    end
    B.SetMode(what)
end

-- ---------------------------------------------------------------- Files

buildFiles = function()
    local FW = 520
    files = CreateFrame("Frame", "WrathUnleashedFilesFrame", UIParent)
    files:SetWidth(FW)
    files:SetPoint("TOP", UIParent, "TOP", 0, -LAYOUT.topMargin)
    files:SetFrameStrata("DIALOG")
    files:EnableMouse(true)
    S().EdPanel(files, 0.94, S().ED.panel2)
    smallCaps(files, "Files", true, "TOPLEFT", 12, -10)
    files.close = chip(files, "x", 28, function() files:Hide() end, "TOPRIGHT", -6, -6)
    if UISpecialFrames then tinsert(UISpecialFrames, "WrathUnleashedFilesFrame") end
    local LX, BX = 12, 112          -- the row's label, its name box
    local function row(y, label) return smallCaps(files, label, false, "TOPLEFT", LX, y - 7) end
    -- this Den, by name (DenBar.lua: .den save <name>; Open: .den load <name> replace, one undo takes it back)
    files.denLabel = row(-40, "This Den")
    files.den = editBox(files, 170, 24, "TOPLEFT", BX, -40)
    files.denSave = chip(files, "Save", 64, function()
        local D = W.DenBar
        if D and D.Save then D.Save(files.den:GetText()) end
    end, "TOPLEFT", BX + 178, -40)
    files.denOpen = chip(files, "Open", 64, function()
        local D = W.DenBar
        if D and D.Open then D.Open(files.den:GetText()) end
    end, "TOPLEFT", BX + 248, -40)
    files.denInfo = S().EdText(files, S().ED_SIZE.small)
    files.denInfo:SetPoint("TOPLEFT", BX, -72)
    files.denInfo:SetText("It saves itself. Open replaces it (Undo undoes).")
    files.denInfo:SetTextColor(S().ED.muted[1], S().ED.muted[2], S().ED.muted[3])
    -- builds: your props in this area, by name or a 6-letter code (the full Den)
    files.full = {}
    local function full(f) files.full[#files.full + 1] = f; return f end
    full(row(-100, "Build"))
    files.code = full(editBox(files, 170, 24, "TOPLEFT", BX, -100))
    files.save = full(chip(files, "Save", 64, function() B.Save() end, "TOPLEFT", BX + 178, -100))
    files.load = full(chip(files, "Load", 64, function() B.Load() end, "TOPLEFT", BX + 248, -100))
    files.share = full(chip(files, "Share", 64, function() B.Share() end, "TOPLEFT", BX + 318, -100))
    files.info = full(S().EdText(files, S().ED_SIZE.small))
    files.info:SetPoint("TOPLEFT", BX, -132); files.info:SetText("A name or a 6-letter code.")
    -- prefabs: the selection, saved by name
    full(row(-156, "Prefab"))
    files.prefab = full(editBox(files, 170, 24, "TOPLEFT", BX, -156))
    files.prefabSave = full(chip(files, "Save", 64, function() local n = prefabName(); if n then B.Command("prefab save " .. n) end end, "TOPLEFT", BX + 178, -156))
    files.prefabPaste = full(chip(files, "Paste", 64, function() local n = prefabName(); if n then B.PastePrefab(n) end end, "TOPLEFT", BX + 248, -156))
    files.prefabShare = full(chip(files, "Share", 64, function() local n = prefabName(); if n then B.Command("prefab share " .. n) end end, "TOPLEFT", BX + 318, -156))
    -- the plot
    full(row(-196, "Plot"))
    files.claim = full(chip(files, "Claim", 64, function() B.Command("plot claim") end, "TOPLEFT", BX, -196))
    files.plotInfoBtn = full(chip(files, "Info", 52, function() B.Command("plot") end, "TOPLEFT", BX + 70, -196))
    files.plotName = full(editBox(files, 110, 12, "TOPLEFT", BX + 128, -196))
    files.plotAdd = full(chip(files, "Add", 48, function() local n = plotName(); if n then B.Command("plot add " .. n) end end, "TOPLEFT", BX + 244, -196))
    files.plotVisitor = full(chip(files, "Visitor", 66, function() local n = plotName(); if n then B.Command("plot add " .. n .. " visitor") end end, "TOPLEFT", BX + 298, -196))
    files.plotRemove = full(chip(files, "Remove", 70, function() local n = plotName(); if n then B.Command("plot remove " .. n) end end, "TOPLEFT", BX + 370, -196))
    files.plot = full(S().EdText(files, S().ED_SIZE.small))
    files.plot:SetPoint("TOPLEFT", BX, -228)
    files.plot:SetWidth(FW - BX - 12); files.plot:SetJustifyH("LEFT")
    files.plot:SetText(B.PlotText())
    files.status = S().EdText(files, S().ED_SIZE.small)
    files.status:SetWidth(FW - 24); files.status:SetJustifyH("LEFT")
    files.help = W.HelpButton(files, {
        "This Den: a save of the whole Den by name (the autosave keeps it anyway); Open puts a save back.",
        "Build: Save or Share your props in this area by name; Load a name or a code where you stand.",
        "Prefab: the selection, saved by name; Paste puts a copy down (in edit mode: click the ground).",
        "Plot: Claim one, then Add a friend (or a Visitor) by name.",
    }, 560, "TOPRIGHT", -40, -6)
    files:SetScript("OnShow", function()
        local D = W.DenBar
        local den = D and D.IsActive and D.IsActive()
        local lite = B.state.lite == "1"
        for _, f in ipairs({ files.denLabel, files.den, files.denSave, files.denOpen, files.denInfo }) do if den then f:Show() else f:Hide() end end
        for _, f in ipairs(files.full) do if lite then f:Hide() else f:Show() end end
        if den and (files.den:GetText() or "") == "" then files.den:SetText((D and D.lastName) or "my-den") end
        files.plot:SetText(B.PlotText())
        local bottom = lite and -96 or -252
        files.status:ClearAllPoints(); files.status:SetPoint("TOPLEFT", 12, bottom)
        files:SetHeight(-bottom + 30)
    end)
    files:SetHeight(282)
    files:Hide()
end

function B.Files() if not files then buildFiles() end return files end
function B.ShowFiles(show)
    B.Files()
    B.Dock(files, editing())
    if show == nil then show = not files:IsShown() end
    if show then files:Show() else files:Hide() end
    return show
end

-- ---------------------------------------------------------------- the asset browser

local function buildTile(i)
    local col, row = (i - 1) % COLS, math.floor((i - 1) / COLS)
    local t = CreateFrame("Button", nil, win.grid)
    t:SetWidth(TILE); t:SetHeight(TILE + NAME_H)
    t:SetPoint("TOPLEFT", col * (TILE + GAP), -row * (TILE + NAME_H + GAP))
    -- the tile's square: a soft rounded card (owner 3 Nov: "not so boxy")
    t.card = CreateFrame("Frame", nil, t)
    t.card:SetWidth(TILE); t.card:SetHeight(TILE)
    t.card:SetPoint("TOPLEFT", 0, 0)
    if t.card.SetBackdrop then
        t.card:SetBackdrop(S().ROUND_SMALL)
        local c = S().ED.tile
        t.card:SetBackdropColor(c[1], c[2], c[3], c[4])
        t.card:SetBackdropBorderColor(0, 0, 0, 0)
    else
        solid(t.card, "BACKGROUND", S().ED.tile):SetAllPoints(t.card)
    end
    t.hover = solid(t, "BORDER", { 1, 1, 1, 0.05 })
    t.hover:SetPoint("TOPLEFT", 3, -3); t.hover:SetWidth(TILE - 6); t.hover:SetHeight(TILE - 6)
    t.hover:Hide()
    -- the picture where there's no model (a building, a prop without a model in this game's files): on purpose,
    -- the category's icon (a building: the home stone) at full strength, and a word why
    t.icon = t:CreateTexture(nil, "ARTWORK")
    t.icon:SetWidth(46); t.icon:SetHeight(46)
    t.icon:SetPoint("TOP", t, "TOP", 0, -16)
    t.icon:SetVertexColor(0.62, 0.64, 0.69, 1)
    t.why = S().EdText(t, S().ED_SIZE.small)
    t.why:SetPoint("TOP", t, "TOP", 0, -TILE + 22)
    t.why:SetWidth(TILE); t.why:SetJustifyH("CENTER")
    t.why:SetTextColor(S().ED.muted[1], S().ED.muted[2], S().ED.muted[3])
    -- above the model: the edge (gold when picked), the corner icon, the points and the favourite star
    t.top = CreateFrame("Frame", nil, t)
    t.top:SetWidth(TILE); t.top:SetHeight(TILE)
    t.top:SetPoint("TOPLEFT", 0, 0)
    -- the model, from the side, turning (Spin; a right-drag turns it, the wheel zooms: B.AttachViewMouse)
    tileModel(t)
    t.dragTick = B.AttachViewMouse(t, t, function(delta) B.Scroll(-delta) end)
    -- the edge: rounded, over the model (gold when picked)
    t.edges = {}
    if t.top.SetBackdrop then
        t.top:SetBackdrop({ edgeFile = S().ROUND_EDGE, edgeSize = 12, insets = { left = 3, right = 3, top = 3, bottom = 3 } })
        t.edges[1] = { SetTexture = function(_, r, g, b, a) t.edgeColour = { r, g, b, a }; t.top:SetBackdropBorderColor(r, g, b, a) end }
    else
        t.edges[1] = solid(t.top, "OVERLAY", S().ED.edge)
    end
    t.badge = t.top:CreateTexture(nil, "OVERLAY")
    t.badge:SetWidth(18); t.badge:SetHeight(18)
    t.badge:SetPoint("BOTTOMRIGHT", -4, 4)
    t.badge:SetVertexColor(0.62, 0.64, 0.69, 1)
    t.badge:SetAlpha(0.85)
    t.badge:Hide()
    t.cost = S().EdText(t.top, S().ED_SIZE.small)
    t.cost:SetPoint("TOPLEFT", 5, -4)
    t.fav = S().EdText(t.top, S().ED_SIZE.title)
    t.fav:SetPoint("TOPRIGHT", -5, -3)
    local g = S().ED.gold
    t.fav:SetTextColor(g[1], g[2], g[3])
    t.name = S().EdText(t, S().ED_SIZE.text)
    t.name:SetPoint("TOPLEFT", t, "TOPLEFT", 0, -TILE - 3)
    t.name:SetWidth(TILE); t.name:SetHeight(NAME_H - 2)
    t.name:SetJustifyH("CENTER")
    t:RegisterForClicks("LeftButtonUp", "RightButtonUp")
    t:RegisterForDrag("LeftButton")
    t:SetScript("OnClick", function(self, mouse)
        if not self.item then return end
        if mouse == "RightButton" then
            -- a right-drag turned the model: not a right-click
            if self.dragged then self.dragged = nil; return end
            B.ToggleFavourite(self.item); return
        end
        B.Choose(self.item)
    end)
    t:SetScript("OnDoubleClick", function(self) if self.item then B.Place(self.item) end end)
    t:SetScript("OnEnter", function(self)
        self.over = true; paintTile(self)
        local it = self.item
        if not it then return end
        -- a name cut short: the whole of it under the tile
        if self.elided then self.edTipAt, self.edTipTo, self.edTipX, self.edTipY = "TOP", "BOTTOM", 0, -2; S().EdTip(self, tileNameText(it)) end
        if it.kind then setStatus(it.name .. ((it.kind == "stamp") and (": " .. (it.line or "")) or "") .. "  Double-click to place it.")
        else setStatus(string.format("%s: %d build point%s. %s", it.name, it.cost or 1, (it.cost or 1) == 1 and "" or "s",
            editing() and "Double-click, then click the ground." or "Double-click to place it in front of you.")) end
    end)
    t:SetScript("OnLeave", function(self) self.over = nil; paintTile(self); S().EdTip(nil) end)
    -- drag a tile out of the window: placed (in front of you; in edit mode where you click next)
    -- Den Lite's build mode (owner's LANDSCAPE editor, build-tool-core.js: "drag a card from the Build palette and drop
    -- the piece under your cursor"): the drag picks the card up at once, its copy glides under the cursor over the
    -- world (Gizmo.lua: over the world while a card is dragged out of the window), and letting go there puts it down
    -- (one undo) and selects it; let go over the window, it stays in hand (click the ground to place it).
    local function gizmoDrag(item) local G = W.Gizmo; return G and G.build and item and not item.kind and G.DropCard end
    t:SetScript("OnDragStart", function(self)
        B.dragging = self.item
        if not self.item then return end
        if gizmoDrag(self.item) then B.Choose(self.item); setStatus("Let go on the ground to put it there.")
        else setStatus("Let go outside the window to place it.") end
    end)
    t:SetScript("OnDragStop", function()
        local it = B.dragging
        B.dragging = nil
        local outside = not (win.IsMouseOver and win:IsMouseOver())
        if it and gizmoDrag(it) then if outside then W.Gizmo.DropCard() end
        elseif it and outside then B.Place(it) end
    end)
    t:Hide()
    return t
end

-- How many rows of tiles fit between the gallery's bottom (above the chat) and the player frame's bottom: 4 on a
-- 1080p screen at the addon's own scale, fewer on a short one (2 at least).
function B.FitRows(screenH)
    screenH = tonumber(screenH) or (GetScreenHeight and GetScreenHeight()) or 768
    local room = screenH - LAYOUT.galleryBottom - LAYOUT.galleryTop - HEAD_H - FOOT_H + GAP
    return math.max(2, math.min(ROWS, math.floor(room / (TILE + NAME_H + GAP))))
end

-- The game's own frames the gallery keeps off (DenBar.lua D.GAME_FRAMES, the den-test's list, plus the target
-- frame, which comes and goes). Owner's den-features 3 Nov: "WrathUnleashedBuilderFrame/ChatFrame1": his chat is
-- taller than the fixed place allowed for, so the gallery goes where the frames really are.
B.KEEP_OFF = { "ChatFrame1", "MainMenuBar", "MultiBarBottomLeft", "MultiBarBottomRight", "MultiBarRight", "MultiBarLeft",
    "PlayerFrame", "TargetFrame", "MinimapCluster",
    -- and the editor's own pieces beside it
    "LeylineDenBar", "LeylineDenRail", "LeylineEditorStatus", "WrathUnleashedPropsFrame" }
local KEEP_GAP = 8
-- frames whose place counts while hidden (they come and go where they are)
local KEEP_HIDDEN = { TargetFrame = true, WrathUnleashedPropsFrame = true }

-- A frame's box in UIParent's units ({ l, b, r, t }), shown or not (the target frame's place counts while hidden).
local function boxOf(f)
    if not (f and f.GetLeft) then return nil end
    local l, b, w, h = f:GetLeft(), f:GetBottom(), f:GetWidth(), f:GetHeight()
    if not (l and b and w and h) or w <= 0 or h <= 0 then return nil end
    local k = 1
    if f.GetEffectiveScale and UIParent and UIParent.GetEffectiveScale then
        local fe, ue = tonumber((f:GetEffectiveScale())), tonumber((UIParent:GetEffectiveScale()))
        if fe and ue and ue > 0 then k = fe / ue end
    end
    return { l = l * k, b = b * k, r = (l + w) * k, t = (b + h) * k }
end
B.BoxOf = boxOf

-- The boxes of B.KEEP_OFF's frames there are now (the target frame's even while hidden; the others when shown).
function B.KeepOffBoxes()
    local out = {}
    for _, n in ipairs(B.KEEP_OFF) do
        local f = _G[n]
        if f and (KEEP_HIDDEN[n] or (f.IsShown and f:IsShown())) then
            local bx = boxOf(f)
            -- soft: kept off when there's room, given up before the game's own frames are covered
            if bx then bx.name = n; bx.soft = (n:match("^Leyline") or n:match("^WrathUnleashed") or not (f.IsShown and f:IsShown())) and true or nil; out[#out + 1] = bx end
        end
    end
    return out
end

local function hits(l, b, r, t, boxes)
    for _, g in ipairs(boxes) do
        if math.min(r, g.r) - math.max(l, g.l) > 2 and math.min(t, g.t) - math.max(b, g.b) > 2 then return g end
    end
    return nil
end
B.Hits = hits

-- Where the gallery goes (pure, for the test): the most rows (as many as B.FitRows gives, down to 2) at the first
-- free place of: its usual
-- place (above the chat, right of the rail), just above whatever is under it, just right of whatever is beside it,
-- the screen's right side. `k`: the gallery's units in UIParent's. Returns x, bottom, rows (in UIParent's units for
-- x and bottom), and false when nothing is free (then the usual place, the most rows that fit the screen).
function B.PlaceGallery(screenW, screenH, boxes, k)
    k = tonumber(k) or 1
    boxes = boxes or {}
    local w = WIN_W * k
    local xs, bs = { LAYOUT.galleryX }, { LAYOUT.galleryBottom }
    for _, g in ipairs(boxes) do
        bs[#bs + 1] = g.t + KEEP_GAP
        xs[#xs + 1] = g.r + KEEP_GAP
    end
    xs[#xs + 1] = screenW - w - KEEP_GAP
    table.sort(bs)
    local top = screenH - LAYOUT.galleryTop
    local hard = {}
    for _, g in ipairs(boxes) do if not g.soft then hard[#hard + 1] = g end end
    for _, keep in ipairs({ boxes, hard }) do
        for rows = B.FitRows(screenH), 2, -1 do
            local h = winHeight(rows) * k
            for _, b in ipairs(bs) do
                if b >= LAYOUT.galleryBottom and b + h <= top then
                    for _, x in ipairs(xs) do
                        if x >= LAYOUT.galleryX and x + w <= screenW - KEEP_GAP and not hits(x, b, x + w, b + h, keep) then
                            return x, b, rows, true
                        end
                    end
                end
            end
        end
    end
    return LAYOUT.galleryX, LAYOUT.galleryBottom, B.FitRows(screenH), false
end

-- The gallery's place and rows for this screen (again after a resize, a scale change, or a game frame moving).
function B.Layout()
    if not win then return end
    if not B.moved then
        local k = 1
        if win.GetEffectiveScale and UIParent and UIParent.GetEffectiveScale then
            local fe, ue = tonumber((win:GetEffectiveScale())), tonumber((UIParent:GetEffectiveScale()))
            if fe and ue and ue > 0 then k = fe / ue end
        end
        local sw = tonumber(GetScreenWidth and GetScreenWidth()) or 1024
        local sh = tonumber(GetScreenHeight and GetScreenHeight()) or 768
        local x, b, rows = B.PlaceGallery(sw, sh, B.KeepOffBoxes(), k)
        B.rows = rows
        win:SetHeight(winHeight(B.rows))
        win:ClearAllPoints()
        win:SetPoint("BOTTOMLEFT", UIParent, "BOTTOMLEFT", x / k, b / k)
        B.placedAt = { x, b }
    else
        B.rows = B.FitRows()
        win:SetHeight(winHeight(B.rows))
    end
    win.grid:SetHeight(B.rows * (TILE + NAME_H + GAP))
    local tabs = 0
    for _, b in ipairs(win.tabs) do if b:IsShown() or not tabHidden(b.tab, B.state.lite == "1") then tabs = tabs + 1 end end
    win.tabStep = math.max(18, math.min(26, math.floor((winHeight(B.rows) - 2 * PAD) / math.max(1, tabs))))
    for _, b in ipairs(win.tabs) do b:SetHeight(win.tabStep - 2) end
end

local function build()
    win = CreateFrame("Frame", "WrathUnleashedBuilderFrame", UIParent)
    win:SetWidth(WIN_W); win:SetHeight(WIN_H)
    win:SetPoint("BOTTOMLEFT", UIParent, "BOTTOMLEFT", LAYOUT.galleryX, LAYOUT.galleryBottom)
    win:SetFrameStrata("DIALOG")
    win:EnableMouse(true)
    win:SetMovable(true)
    win:RegisterForDrag("LeftButton")
    win:SetScript("OnDragStart", win.StartMoving)
    win:SetScript("OnDragStop", function(self) self:StopMovingOrSizing(); B.moved = true end)
    S().EdPanel(win, 0.9)
    tinsert(UISpecialFrames, "WrathUnleashedBuilderFrame")   -- Esc closes it
    local st = store()
    if st.spin ~= nil then B.spin = st.spin and true or false end
    B.az = st.az and true or false

    -- the head: the tab's name, the page, paging, close
    local x0 = PAD + CAT_W + PAD
    win.title = S().EdText(win, S().ED_SIZE.title)
    win.title:SetPoint("TOPLEFT", x0, -14)
    win.title:SetTextColor(S().ED.gold[1], S().ED.gold[2], S().ED.gold[3])
    win.close = chip(win, "x", 28, function() win:Hide() end, "TOPRIGHT", -PAD, -PAD + 2)
    win.close.edLabel = "Close the gallery (Esc)"
    win.next = chip(win, ">", 28, function() B.Page(1) end, "TOPRIGHT", -PAD - 34, -PAD + 2)
    win.prev = chip(win, "<", 28, function() B.Page(-1) end, "TOPRIGHT", -PAD - 66, -PAD + 2)
    win.next.edLabel, win.prev.edLabel = "Next page", "Previous page"
    win.count = S().EdText(win, S().ED_SIZE.text)
    win.count:SetPoint("TOPRIGHT", -PAD - 104, -15)
    win.count:SetJustifyH("RIGHT")
    local m = S().ED.muted
    win.count:SetTextColor(m[1], m[2], m[3])

    -- search, A-Z, Spin, Reload
    local search = editBox(win, 240, 30, "TOPLEFT", x0, -42)
    search:SetHeight(28)
    search:SetScript("OnTextChanged", function(self)
        B.search = (self:GetText() or ""):sub(1, 30)
        B.offset = 0
        if win.searchHint then if B.search == "" then win.searchHint:Show() else win.searchHint:Hide() end end
        if not SEARCHED[B.tab] then B.Filter() end
    end)
    search:SetScript("OnEnterPressed", function(self)
        self:ClearFocus()
        if SEARCHED[B.tab] then B.Find(B.tab, B.search) end
    end)
    win.search = search
    win.searchHint = S().EdText(win, S().ED_SIZE.text)
    win.searchHint:SetPoint("LEFT", search, "LEFT", 8, 0)
    win.searchHint:SetTextColor(S().ED.off[1], S().ED.off[2], S().ED.off[3])
    win.searchHint:SetText("Search props...")
    win.az = chip(win, "A-Z", 48, function() B.SetAZ(not B.az) end, "TOPLEFT", x0 + 248, -42)
    win.az.edLabel = "Sort by name"
    win.spinBtn = chip(win, "Spin", 52, function() B.SetSpin(not B.spin) end, "TOPLEFT", x0 + 302, -42)
    win.spinBtn.edLabel = "Turn the models slowly"
    win.reload = chip(win, "Reload", 66, function() B.RequestCatalog() end, "TOPLEFT", x0 + 360, -42)
    win.reload.edLabel = "Ask for the props again"

    -- the category column: an icon and the name, one row each (the one on in gold)
    win.tabs = {}
    win.tabStep = 24
    for i, t in ipairs(TABS) do
        local b = chip(win, "", CAT_W, function(self) B.ShowTab(self.tab) end, "TOPLEFT", PAD, -PAD - (i - 1) * 24)
        b:SetHeight(22)
        b.tab = t
        b.edText:ClearAllPoints()
        b.edText:SetPoint("LEFT", 28, 0); b.edText:SetPoint("RIGHT", -4, 0)
        b.edText:SetJustifyH("LEFT")
        b:SetText(tabText(t))
        b.tabIcon = b:CreateTexture(nil, "ARTWORK")
        b.tabIcon:SetWidth(18); b.tabIcon:SetHeight(18)
        b.tabIcon:SetPoint("LEFT", 5, 0)
        W.Style.SetIcon(b.tabIcon, iconName(t), S().ED.muted)
        b.iconPath = catIcon(t)
        win.tabs[i] = b
    end
    local sep = solid(win, "BORDER", S().ED.edge)
    sep:SetPoint("TOPLEFT", PAD + CAT_W + PAD / 2, -PAD); sep:SetPoint("BOTTOMLEFT", PAD + CAT_W + PAD / 2, PAD); sep:SetWidth(1)

    -- the grid of tiles
    win.grid = CreateFrame("Frame", nil, win)
    win.grid:SetWidth(COLS * TILE + (COLS - 1) * GAP); win.grid:SetHeight(ROWS * (TILE + NAME_H + GAP))
    win.grid:SetPoint("TOPLEFT", x0, -HEAD_H)
    win.grid:EnableMouseWheel(true)
    win.grid:SetScript("OnMouseWheel", function(_, delta) B.Scroll(-delta) end)
    win.grid:SetScript("OnUpdate", function(_, elapsed) B.TileTick(elapsed) end)
    win.tiles = {}
    for i = 1, PAGE do win.tiles[i] = buildTile(i) end
    win.empty = S().EdText(win, S().ED_SIZE.text)
    win.empty:SetPoint("TOPLEFT", x0 + 6, -HEAD_H - 10)
    win.empty:SetWidth(COLS * TILE); win.empty:SetJustifyH("LEFT")

    -- the foot: the budget meter and Place
    win.barBg = solid(win, "ARTWORK", { 1, 1, 1, 0.08 })
    win.barBg:SetWidth(160); win.barBg:SetHeight(4)
    win.barBg:SetPoint("BOTTOMLEFT", x0, 12)
    win.bar = win:CreateTexture(nil, "OVERLAY")
    win.bar:SetTexture(1, 1, 1, 1)
    win.bar:SetWidth(1); win.bar:SetHeight(4)
    win.bar:SetPoint("BOTTOMLEFT", x0, 12)
    win.meter = S().EdText(win, S().ED_SIZE.small)
    win.meter:SetPoint("BOTTOMLEFT", x0, 20)
    win.meter:SetTextColor(m[1], m[2], m[3])
    win.spawn = chip(win, "Place", 200, function() B.Spawn() end, "BOTTOMRIGHT", -PAD, 8)
    win.spawn:SetActive(true)

    win:SetScript("OnUpdate", function(_, elapsed)
        -- a game frame moved or grew under the gallery (the chat resized, the target frame came): laid out again
        B.keepOffT = (B.keepOffT or 0) + (tonumber(elapsed) or 0)
        if B.keepOffT > 1 and not B.moved then
            B.keepOffT = 0
            local me = boxOf(win)
            local hard = {}
            for _, g in ipairs(B.KeepOffBoxes()) do if not g.soft then hard[#hard + 1] = g end end
            if me and hits(me.l, me.b, me.r, me.t, hard) then B.Layout() end
        end
        if B.waiting and B.askedAt and now() - B.askedAt > ANSWER_WAIT then
            B.waiting = nil
            setStatus("The Builder isn't on this realm yet.")
            B.Refresh()
        end
    end)
    win:SetScript("OnShow", function()
        if PlaySound then pcall(PlaySound, "igCharacterInfoOpen") end
        if #B.catalog == 0 then B.RequestCatalog() end
        B.Layout()
        -- a Model frame may not draw what it was given while hidden: every tile takes its model again
        for _, t in ipairs(win.tiles) do t.modelPath = nil end
        B.Filter()
        refreshMeter()
        B.ShowPill(true)
        chromeRefresh()
    end)
    win:SetScript("OnHide", function()
        B.heldOpen = nil
        S().EdTip(nil)
        chromeRefresh()
    end)
    win:Hide()
end

-- Is any editor piece open (the chrome outside a Den follows this)?
function B.EditorOpen()
    return (win and win:IsShown()) or (panel and panel:IsShown()) or (files and files:IsShown()) or false
end

function B.ShowBuilder(show)
    if not win then build() end
    if show then win:Show() else win:Hide() end
    return win:IsShown()
end

function W.ToggleBuilder()
    if not win then build() end
    if win:IsShown() then win:Hide() else win:Show() end
end

-- Hold Q in edit mode, /spawnmenu: the gallery, on the world frame while editing (centred), hidden again on letting
-- go when Q opened it.
function B.ShowMenu(show)
    if not win then build() end
    B.Dock(win, editing())
    if show then
        if not win:IsShown() then B.heldOpen = true end
        win:Show()
        B.Layout()
    elseif B.heldOpen then
        win:Hide()
    end
end
function B.ToggleMenu()
    if win and win:IsShown() then win:Hide() else B.ShowMenu(true); B.heldOpen = nil end
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:SetScript("OnEvent", function(_, _, prefix, msg, channel, sender) B.OnMessage(prefix, msg, channel, sender) end)

SLASH_WUBUILDER1, SLASH_WUBUILDER2 = "/builder", "/build"
SlashCmdList["WUBUILDER"] = function() W.ToggleBuilder() end
SLASH_WUSPAWNMENU1 = "/spawnmenu"
SlashCmdList["WUSPAWNMENU"] = function() B.ToggleMenu() end

W.Builder = B
B.SetStatus = setStatus   -- the Den bar (DenBar.lua) shows Den Lite's answers here too
B.Window, B.STEPS, B.CATS, B.CAT_ICONS, B.CatIcon = function() return win end, STEPS, CATS, CAT_ICONS, catIcon
B.Menu, B.Panel, B.MENU_TABS, B.TABS, B.TAB_ICONS = function() return win end, function() return panel end, TABS, TABS, TAB_ICONS
B.STEPS = STEPS
B.PAGE, B.COLS, B.STEP_PRESETS, B.ANGLE_STEPS, B.TabText = PAGE, COLS, STEP_PRESETS, ANGLE_STEPS, tabText
