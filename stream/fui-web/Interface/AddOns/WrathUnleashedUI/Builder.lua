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
local PAGE = COLS * ROWS                        -- tiles on a page: 20 frames, reused
local TILE, GAP, NAME_H = 74, 6, 14
local CAT_W = 64                                -- the category column
local WIN_W = CAT_W + 8 + COLS * TILE + (COLS - 1) * GAP + 10
local WIN_H = 62 + ROWS * (TILE + NAME_H + GAP) + 32
local ANSWER_WAIT = 6
local STEPS = { 0.01, 0.1, 0.25, 1, 3, 10 }    -- the move step, in yards
local STEP_PRESETS = { 2, 3, 4, 5 }             -- the rail's presets (indices into STEPS): 0.1, 0.25, 1, 3
local ANGLE_STEPS = { 1, 5, 15, 45, 90 }        -- the turn step, in degrees (the top strip offers 15, 45 and 90)
local SIZE_STEP = 0.1
local SPIN_SPEED = 0.8                          -- radians a second
local REST_FACING = 0.6                         -- a three-quarter view while Spin is off
-- "objects" and "models" aren't catalog categories: the realm searches them (.build find / .build models; a Den
-- places any safe object and any model of the host's own game files), and their lists are the realm's answers.
-- "buildings" (houses, inns, towers: a Den's catalog only) shows only when the realm's catalog has them.
local CATS = { "all", "seating", "tables", "barrels", "crates", "lights", "banners", "tents", "fires", "fences", "nature", "decor",
    "structure", "buildings", "furniture", "objects", "models" }
local SEARCHED = { objects = "find", models = "models" }
-- "furniture" (a house's palette) isn't a realm category either: the seating and tables, and the beds, rugs, the rack
-- and the book from decor. "stamps" are a Den's ready-made areas (DenHome.lua, .den stamp).
local FURNITURE_IDS = { [13948] = true, [19504] = true, [181396] = true, [181077] = true, [188346] = true, [188347] = true,
    [188348] = true, [19635] = true, [2695] = true }
local function inCat(c, want)
    return want == "all" or c.cat == want or (want == "furniture" and (c.cat == "seating" or c.cat == "tables" or FURNITURE_IDS[c.id] == true))
end
local RECENT_MAX = 12
-- A small picture per category (stock 3.3.5a icons, Interface\Icons\...): the column, and a tile without a model.
local ICON = "Interface\\Icons\\"
local CAT_ICONS = { seating = "Spell_Nature_Sleep", tables = "INV_Misc_Food_15", barrels = "INV_Drink_05",
    crates = "INV_Crate_01", lights = "INV_Torch_Lit", banners = "INV_BannerPVP_02", tents = "INV_Fabric_Linen_01",
    fires = "Spell_Fire_Fire", fences = "Ability_Defend", nature = "Spell_Nature_Regeneration", decor = "INV_Misc_Book_09",
    structure = "Trade_Mining", buildings = "Trade_BlackSmithing", furniture = "Trade_Tailoring", objects = "INV_Misc_Gear_01",
    models = "Spell_Frost_WizardMark" }
local TAB_ICONS = { favourites = "Spell_Arcane_StarFire", recent = "INV_Misc_PocketWatch_01", prefabs = "INV_Misc_Note_01",
    stamps = "INV_Misc_Map_01", storage = "INV_Misc_Bag_10", all = "INV_Misc_Bag_08" }
local function catIcon(cat) return ICON .. (CAT_ICONS[cat or ""] or TAB_ICONS[cat or ""] or "INV_Misc_QuestionMark") end

-- The column: Favourites, Recent, Prefabs, Stamps, Storage, Everything, then every category.
local TABS = { "favourites", "recent", "prefabs", "stamps", "storage", "all" }
for i = 2, #CATS do TABS[#TABS + 1] = CATS[i] end
local TAB_TEXT = { favourites = "Favourites", recent = "Recent", prefabs = "Prefabs", stamps = "Stamps", storage = "Storage",
    all = "Everything", objects = "Every object", models = "Models" }
local function tabText(t) return TAB_TEXT[t] or (t:sub(1, 1):upper() .. t:sub(2)) end

local win, panel, files, pill
local B = { catalog = {}, shown = {}, offset = 0, tab = "all", step = 4, search = "", state = {}, parts = {}, prefabs = {},
    storage = {}, found = {}, angle = 3, spin = true, az = false, facing = REST_FACING, mode = "place" }

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
    local b = W.Style.Button(parent, "?", 22, nil, nil, ...)
    local tip = CreateFrame("Frame", nil, b)
    width = width or 420
    tip:SetWidth(width); tip:SetHeight(14 + #lines * 14)
    tip:SetPoint("BOTTOMRIGHT", b, "TOPRIGHT", 0, 4)   -- above, to the left (b.tip can be put elsewhere)
    tip:SetFrameStrata("TOOLTIP")
    W.Style.EdPanel(tip, 0.96, W.Style.ED.panel2)
    tip.lines = {}
    for i, line in ipairs(lines) do
        local fs = tip:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
        fs:SetPoint("TOPLEFT", 8, -7 - (i - 1) * 14)
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
    B.status = text or ""
    if pill then pill.note:SetText(B.status) end
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
        store().items[c.id] = { key = c.key, cat = c.cat, name = c.name, cost = c.cost, model = c.model }
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
            -- the 6th field: a model path, or (catalog v2, docs/DEN-OVERHAUL.md) a bare display id whose path
            -- comes later; either way an older 5-field line still reads
            local display = model and model:match("^%d+$") and tonumber(model) or nil
            list[#list + 1] = { id = tonumber(id), key = key, cat = cat, name = name:gsub("|", ""), cost = tonumber(cost) or 1,
                model = (not display) and B.ModelPath(model) or nil, display = display }
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
        -- something new selected while the editor is open: its properties pop up (and go when nothing is)
        if B.state.selId and B.state.selId ~= was and win and win:IsShown() and not (panel and panel:IsShown()) then
            B.ShowProps(true); B.propsAuto = true
        elseif not B.state.selId and B.propsAuto and panel and panel:IsShown() then
            B.ShowProps(false)
        end
        B.PropsFill()
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

local function byIdMap()
    local m = {}
    for _, c in ipairs(B.catalog) do m[c.id] = c end
    return m
end

-- What a tab lists: only the realm's catalog (and your prefabs, a Den's stamps), never anything else.
function B.ListFor(tab, search)
    local q = tostring(search or ""):lower()
    local function match(c) return q == "" or c.name:lower():find(q, 1, true) or (c.key and c.key:find(q, 1, true)) end
    local out = {}
    if tab == "prefabs" then
        for _, f in ipairs(B.prefabs) do if match(f) then out[#out + 1] = f end end
        return out
    end
    if tab == "stamps" then
        for _, st in ipairs((W.DenHome and W.DenHome.StampItems and W.DenHome.StampItems()) or {}) do if match(st) then out[#out + 1] = st end end
        return out
    end
    if SEARCHED[tab] then
        for _, c in ipairs((B.found[tab] or {}).list or {}) do out[#out + 1] = c end
        return out
    end
    local ids = byIdMap()
    for id, it in pairs(store().items) do   -- found objects and models, for Recent, Favourites and Storage
        if not ids[id] and type(it) == "table" and it.name then
            ids[id] = { id = id, key = it.key, cat = it.cat, name = it.name, cost = it.cost or 1, model = B.ModelPath(it.model) }
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
    return catIcon(c.cat)
end

local function paintTile(t)
    local c = t.item
    local picked = c ~= nil and c == B.pick
    local e = picked and S().ED.gold or (t.over and { 1, 1, 1, 0.22 } or S().ED.edge)
    for _, x in ipairs(t.edges) do x:SetTexture(e[1], e[2], e[3], e[4]) end
    if t.over or picked then t.hover:Show() else t.hover:Hide() end
end
B.PaintTile = paintTile

local function setTileModel(t, path)
    if path then
        if t.modelPath ~= path then
            t.modelPath = path
            if t.model.ClearModel then pcall(t.model.ClearModel, t.model) end
            pcall(t.model.SetModel, t.model, path)
            pcall(t.model.SetPosition, t.model, 0, 0, 0)
            pcall(t.model.SetFacing, t.model, B.facing or REST_FACING)
        end
        t.model:Show()
        t.icon:Hide()
        t.badge:Show()
    else
        if t.modelPath then t.modelPath = nil; if t.model.ClearModel then pcall(t.model.ClearModel, t.model) end end
        t.model:Hide()
        t.icon:Show()
        t.badge:Hide()
    end
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
    local maxOff = math.max(0, math.ceil((n - PAGE) / COLS) * COLS)
    if B.offset > maxOff then B.offset = maxOff end
    if B.offset < 0 then B.offset = 0 end
    local favs = store().favs
    for i, t in ipairs(win.tiles) do
        local c = B.shown[B.offset + i]
        t.item = c
        if c then
            t.name:SetText(tileNameText(c))
            t.fav:SetText(favs[c.id] and c.kind == nil and "*" or "")
            t.cost:SetText((c.cost or 1) > 1 and tostring(c.cost) or "")
            t.icon:SetTexture(tileIcon(c)); t.icon.path = tileIcon(c)
            t.badge:SetTexture(tileIcon(c))
            setTileModel(t, c.model)
            paintTile(t)
            t:Show()
        else
            t:Hide()
        end
    end
    win.title:SetText(tabText(B.tab):upper())
    for _, b in ipairs(win.tabs) do b:SetActive(b.tab == B.tab) end
    win.az:SetActive(B.az)
    win.spinBtn:SetActive(B.spin)
    local f = SEARCHED[B.tab] and B.found[B.tab]
    win.count:SetText(n > 0 and string.format("%d-%d of %d%s", B.offset + 1, math.min(n, B.offset + PAGE), n,
        (f and f.more) and "+" or "") or "")
    win.empty:SetText(emptyText(n))
    if B.offset > 0 then win.prev:Enable() else win.prev:Disable() end
    if B.offset + PAGE < n or (f and f.more) then win.next:Enable() else win.next:Disable() end
    win.spawn:SetText(B.pick and ("Place " .. B.pick.name) or "Place")
    if B.pick then win.spawn:Enable() else win.spawn:Disable() end
end

-- Paging: Next / Prev a page; the wheel a row. At the end of a searched list Next asks the realm for more.
function B.Page(dir)
    if dir > 0 and B.offset + PAGE >= #B.shown then
        if SEARCHED[B.tab] then return B.MoreFound(B.tab) end
        return false
    end
    B.offset = math.max(0, B.offset + dir * PAGE)
    B.Refresh()
    return true
end
function B.Scroll(rows)
    B.offset = math.max(0, B.offset + rows * COLS)
    B.Refresh()
end

-- Spin: the tiles' models turn slowly; off, they rest at a three-quarter view.
function B.SetSpin(on)
    B.spin = on and true or false
    store().spin = B.spin
    if not B.spin then
        B.facing = REST_FACING
        if win then for _, t in ipairs(win.tiles) do if t.modelPath then pcall(t.model.SetFacing, t.model, B.facing) end end end
    end
    B.Refresh()
end
function B.Turntable(elapsed)
    if not B.spin or not win then return end
    B.facing = ((B.facing or REST_FACING) + (tonumber(elapsed) or 0) * SPIN_SPEED) % (2 * math.pi)
    for _, t in ipairs(win.tiles) do
        if t.modelPath and t:IsShown() then pcall(t.model.SetFacing, t.model, B.facing) end
    end
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

-- One click on a tile: picked (the gold edge), and the status says what it is.
function B.Choose(item)
    if not item then return false end
    B.pick = item
    B.mode = "place"
    if item.kind == "prefab" then setStatus(string.format("%s: %d props. Double-click to paste it.", item.name, item.count or 0))
    elseif item.kind == "stamp" then setStatus(item.name .. ": " .. (item.line or "") .. " Double-click to mark where it goes.")
    else setStatus(string.format("%s: %d build point%s. Double-click to place it, right-click to favourite it.",
        item.name, item.cost or 1, (item.cost or 1) == 1 and "" or "s")) end
    B.Refresh()
    return true
end

-- Places the picked prop: at your feet, or in edit mode where you click next.
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
        setStatus("Click the ground where it goes.")
        return true
    end
    return B.Command("spawn " .. B.pick.id)
end

-- Double-click, Place, or a tile dragged out of the window: a prop, a prefab (pasted) or a stamp (marked).
function B.Place(item)
    if not item then return false end
    local ok
    if item.kind == "prefab" then
        ok = B.PastePrefab(item.name)
    elseif item.kind == "stamp" then
        ok = W.DenHome and W.DenHome.Stamp(item.key, editing())   -- in edit mode: as the shelf's tiles
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
    if panel then panel.stepBtn:SetText("Step " .. STEPS[B.step] .. " yd") end
    if refreshPill then refreshPill() end
    chromeRefresh()
    return true
end
function B.SetAngle(deg)
    for i, a in ipairs(ANGLE_STEPS) do
        if a == tonumber(deg) then
            B.angle = i
            if SetCVar then pcall(SetCVar, "leylineEditAngle", tostring(a)) end   -- edit mode's Q / E follow
            if panel then panel.angle:SetText("Turn " .. a .. "°") end
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
    local parts = { MODE_TEXT[B.mode or "place"] or "Place" }
    if B.pick and (B.mode == "place" or B.mode == nil) then parts[#parts + 1] = B.pick.name end
    if (B.mode == "move") then parts[#parts + 1] = "step " .. STEPS[B.step] .. " yd" end
    local g = B.Snap()
    parts[#parts + 1] = "snap " .. ((g and g > 0) and (g .. " yd") or "off")
    parts[#parts + 1] = ANGLE_STEPS[B.angle] .. "°"
    if s.count and s.count > 1 then parts[#parts + 1] = s.count .. " props selected"
    elseif s.selName then parts[#parts + 1] = s.selName .. " selected" end
    if s.den and s.room then parts[#parts + 1] = "room " .. s.room
    elseif s.mine then parts[#parts + 1] = "props " .. s.mine end
    return table.concat(parts, " · ")
end

local function buildPill()
    pill = CreateFrame("Frame", "LeylineEditorStatus", UIParent)
    pill:SetWidth(460); pill:SetHeight(24)
    pill:SetPoint("BOTTOM", UIParent, "BOTTOM", 0, 14)
    pill:SetFrameStrata("MEDIUM")
    S().EdPanel(pill, 0.82)
    pill.line = pill:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    pill.line:SetPoint("CENTER", 0, 0)
    pill.line.styleKeep = true
    pill.note = pill:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    pill.note:SetPoint("BOTTOM", pill, "TOP", 0, 5)
    pill.note:SetWidth(620)
    pill.note.styleKeep = true
    local m = S().ED.muted
    pill.note:SetTextColor(m[1], m[2], m[3])
    pill.note:SetText(B.status or "")
    pill:Hide()
end
refreshPill = function()
    if pill then pill.line:SetText(B.StatusLine()) end
end
function B.Pill() if not pill then buildPill() end return pill end
function B.ShowPill(show)
    B.Pill()
    B.Dock(pill, editing())
    if show then refreshPill(); pill:Show() else pill:Hide() end
end

-- ---------------------------------------------------------------- the properties popover

local FIELDS = { { "x", "X" }, { "y", "Y" }, { "z", "Z" }, { "turn", "Turn" }, { "pitch", "Pitch" }, { "roll", "Roll" },
    { "scale", "Size" } }
local ANGLE_FIELDS = { turn = true, pitch = true, roll = true }

function B.PropsFill()
    if not panel then return end
    local s = B.state
    local p = s.p
    panel.title2:SetText(p and ((s.count or 1) > 1 and (s.count .. " props (numbers for the last one)") or (s.selName or "")) or "Nothing selected")
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

-- The keys while the popover is open (not in edit mode, whose camera has its own): override bindings to hidden
-- buttons, given back when it closes. Arrows move x / y by the step, Page Up / Down lift and lower, Q / E turn.
B.KEYS = {
    LEFT = function() return B.Nudge("y", 1) end, RIGHT = function() return B.Nudge("y", -1) end,
    UP = function() return B.Nudge("x", 1) end, DOWN = function() return B.Nudge("x", -1) end,
    PAGEUP = function() return B.Nudge("z", 1) end, PAGEDOWN = function() return B.Nudge("z", -1) end,
    Q = function() return B.Nudge("turn", 1) end, E = function() return B.Nudge("turn", -1) end,
}
local KEY_ORDER = { "LEFT", "RIGHT", "UP", "DOWN", "PAGEUP", "PAGEDOWN", "Q", "E" }
local keyOwner
local function keyButton(key)
    local name = "LeylinePropKey" .. key
    if _G[name] then return name end
    local b = CreateFrame("Button", name, UIParent)
    b:EnableMouse(false)
    if b.RegisterForClicks then b:RegisterForClicks("AnyDown") end
    b:SetScript("OnClick", function(_, _, down) if down ~= false then B.KEYS[key]() end end)
    return name
end
function B.BindKeys(on)
    if not SetOverrideBindingClick or not ClearOverrideBindings then return false end
    if not keyOwner then keyOwner = CreateFrame("Frame", "LeylinePropKeys", UIParent) end
    ClearOverrideBindings(keyOwner)
    B.keysOn = nil
    if on and not editing() and not (InCombatLockdown and InCombatLockdown()) then
        for _, k in ipairs(KEY_ORDER) do SetOverrideBindingClick(keyOwner, true, k, keyButton(k)) end
        B.keysOn = true
    end
    return B.keysOn == true
end

local function buildPanel()
    panel = CreateFrame("Frame", "WrathUnleashedPropsFrame", UIParent)
    panel:SetWidth(300); panel:SetHeight(226)
    panel:SetPoint("BOTTOMRIGHT", UIParent, "BOTTOMRIGHT", -16, 52)
    panel:SetFrameStrata("DIALOG")
    panel:EnableMouse(true)
    panel:SetMovable(true)
    panel:RegisterForDrag("LeftButton")
    panel:SetScript("OnDragStart", panel.StartMoving)
    panel:SetScript("OnDragStop", panel.StopMovingOrSizing)
    S().EdPanel(panel, 0.92, S().ED.panel2)
    smallCaps(panel, "Selection", true, "TOPLEFT", 10, -8)
    panel.title2 = label(panel, "", "GameFontHighlight", "TOPLEFT", 10, -20)
    panel.title2:SetWidth(250)
    panel.close = chip(panel, "x", 20, function() B.ShowProps(false) end, "TOPRIGHT", -5, -5)
    panel.box, panel.minus, panel.plus, panel.rowLabel = {}, {}, {}, {}
    -- three rows of three: position, rotation, size and the two steps
    for i, f in ipairs(FIELDS) do
        local col, row = (i - 1) % 3, math.floor((i - 1) / 3)
        local x, y = 10 + col * 96, -42 - row * 40
        panel.rowLabel[f[1]] = smallCaps(panel, f[2], false, "TOPLEFT", x, y)
        panel.minus[f[1]] = chip(panel, "-", 16, function() B.Nudge(f[1], -1) end, "TOPLEFT", x, y - 12)
        local e = editBox(panel, 52, 12, "TOPLEFT", x + 18, y - 13)
        e:SetScript("OnEditFocusGained", function() B.propsEditing = true end)
        e:SetScript("OnEnterPressed", function(self) self:ClearFocus(); B.ApplyProps() end)
        panel.box[f[1]] = e
        panel.plus[f[1]] = chip(panel, "+", 16, function() B.Nudge(f[1], 1) end, "TOPLEFT", x + 72, y - 12)
    end
    panel.stepBtn = chip(panel, "Step " .. STEPS[B.step] .. " yd", 88, function()
        B.SetStep(B.step % #STEPS + 1)
    end, "TOPLEFT", 106, -134)
    panel.step = panel.stepBtn
    panel.angle = chip(panel, "Turn " .. ANGLE_STEPS[B.angle] .. "°", 88, function()
        B.SetAngle(ANGLE_STEPS[B.angle % #ANGLE_STEPS + 1])
    end, "TOPLEFT", 202, -134)
    smallCaps(panel, "Steps", false, "TOPLEFT", 106, -122)
    -- what you do with it
    panel.actions = {}
    local acts = {
        { "Apply", function() B.ApplyProps() end }, { "Copy", function() B.Command("copy") end }, { "Paste", function() B.Paste() end },
        { "Delete", function() B.Command("delete") end }, { "Undo", function() B.Command("undo") end },
        { "Redo", function() B.Command("redo") end }, { "More", function() B.PropsMore() end },
    }
    for i, a in ipairs(acts) do panel.actions[a[1]] = chip(panel, a[1], 40, a[2], "TOPLEFT", 10 + (i - 1) * 40, -164) end
    panel.apply = panel.actions.Apply
    panel.hint = label(panel, "Arrows move  ·  PgUp / PgDn up, down  ·  Q / E turn", "GameFontDisableSmall", "TOPLEFT", 10, -192)
    panel.status = label(panel, "", "GameFontHighlightSmall", "TOPLEFT", 10, -207)
    panel.status:SetWidth(280)
    -- More: the rest of the selection's tools
    panel.tools = {}
    local rows = {
        { { "Lock", "lock" }, { "Unlock", "unlock" }, { "Hide", "hide" }, { "Show all", "show all" }, { "Measure", "measure" } },
        { { "Align X", "align x" }, { "Align Y", "align y" }, { "Align Z", "align z" }, { "Bottoms", "align bottom" }, { "Stack on", B.Stack } },
        { { "Select mine", "selall" }, { "Clear", "selclear" }, { "Group", "group" }, { "Ungroup", "ungroup" }, { "Put away", "store" } },
        { { "Duplicate", "dup" }, { "Floor", "floor" }, { "Near", "near" }, { "Undo", "undo" }, { "Redo", "redo" } },
    }
    panel.more = CreateFrame("Frame", nil, panel)
    panel.more:SetWidth(300); panel.more:SetHeight(4 * 26 + 8)
    panel.more:SetPoint("TOPLEFT", panel, "BOTTOMLEFT", 0, -2)
    S().EdPanel(panel.more, 0.92, S().ED.panel2)
    for r, row in ipairs(rows) do
        for c, t in ipairs(row) do
            local act = t[2]
            local b = chip(panel.more, t[1], 54, function()
                if type(act) == "function" then act() else B.Command(act) end
            end, "TOPLEFT", 8 + (c - 1) * 57, -6 - (r - 1) * 26)
            if not panel.tools[t[1]] then panel.tools[t[1]] = b end
        end
    end
    panel.more:Hide()
    panel.help = W.HelpButton(panel, {
        "Type exact numbers and Enter, or nudge with - and +.",
        "Angles in degrees; pitch and roll tilt the prop.",
        "Arrows move it by the step; Page Up / Down lift it; Q / E turn it.",
        "Align: lines the rest up with the last one picked.",
        "Stack on: the rest go on top of the first one picked.",
    }, 340, "TOPRIGHT", -28, -5)
    panel:SetScript("OnShow", function() B.propsEditing = nil; B.PropsFill(); B.BindKeys(true) end)
    panel:SetScript("OnHide", function() B.BindKeys(false); B.propsAuto = nil end)
    panel:Hide()
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
        local c = gold[k] and ED.gold or ED.muted
        fs:SetTextColor(c[1], c[2], c[3])
    end
    B.SetMode(what)
end

-- ---------------------------------------------------------------- Files

buildFiles = function()
    files = CreateFrame("Frame", "WrathUnleashedFilesFrame", UIParent)
    files:SetWidth(380); files:SetHeight(214)
    files:SetPoint("TOPRIGHT", UIParent, "TOPRIGHT", -12, -44)
    files:SetFrameStrata("DIALOG")
    files:EnableMouse(true)
    S().EdPanel(files, 0.94, S().ED.panel2)
    smallCaps(files, "Files", true, "TOPLEFT", 10, -8)
    files.close = chip(files, "x", 20, function() files:Hide() end, "TOPRIGHT", -5, -5)
    if UISpecialFrames then tinsert(UISpecialFrames, "WrathUnleashedFilesFrame") end
    -- this Den, by name (DenBar.lua: .den save <name>)
    files.denLabel = smallCaps(files, "This Den", false, "TOPLEFT", 10, -30)
    files.den = editBox(files, 150, 24, "TOPLEFT", 80, -26)
    files.denSave = chip(files, "Save", 60, function()
        local D = W.DenBar
        if D and D.Save then D.Save(files.den:GetText()) end
    end, "TOPLEFT", 236, -25)
    -- builds: your props in this area, by name or a 6-letter code
    smallCaps(files, "Build", false, "TOPLEFT", 10, -60)
    files.code = editBox(files, 150, 24, "TOPLEFT", 80, -56)
    files.save = chip(files, "Save", 60, function() B.Save() end, "TOPLEFT", 236, -55)
    files.load = chip(files, "Load", 60, function() B.Load() end, "TOPLEFT", 236 + 62, -55)
    files.share = chip(files, "Share", 60, function() B.Share() end, "TOPLEFT", 236 + 124 - 62, -80)
    files.share:ClearAllPoints(); files.share:SetPoint("TOPLEFT", files, "TOPLEFT", 80, -80)
    files.info = label(files, "A name or a 6-letter code.", "GameFontDisableSmall", "TOPLEFT", 146, -85)
    -- prefabs: the selection, saved by name
    smallCaps(files, "Prefab", false, "TOPLEFT", 10, -112)
    files.prefab = editBox(files, 150, 24, "TOPLEFT", 80, -108)
    files.prefabSave = chip(files, "Save", 60, function() local n = prefabName(); if n then B.Command("prefab save " .. n) end end, "TOPLEFT", 236, -107)
    files.prefabPaste = chip(files, "Paste", 60, function() local n = prefabName(); if n then B.PastePrefab(n) end end, "TOPLEFT", 298, -107)
    files.prefabShare = chip(files, "Share", 60, function() local n = prefabName(); if n then B.Command("prefab share " .. n) end end, "TOPLEFT", 80, -131)
    -- the plot
    smallCaps(files, "Plot", false, "TOPLEFT", 10, -162)
    files.claim = chip(files, "Claim", 46, function() B.Command("plot claim") end, "TOPLEFT", 80, -157)
    files.plotInfoBtn = chip(files, "Info", 40, function() B.Command("plot") end, "TOPLEFT", 128, -157)
    files.plotName = editBox(files, 80, 12, "TOPLEFT", 170, -158)
    files.plotAdd = chip(files, "Add", 36, function() local n = plotName(); if n then B.Command("plot add " .. n) end end, "TOPLEFT", 252, -157)
    files.plotVisitor = chip(files, "Visitor", 46, function() local n = plotName(); if n then B.Command("plot add " .. n .. " visitor") end end, "TOPLEFT", 290, -157)
    files.plotRemove = chip(files, "Remove", 40, function() local n = plotName(); if n then B.Command("plot remove " .. n) end end, "TOPLEFT", 338, -157)
    files.plot = label(files, B.PlotText(), "GameFontDisableSmall", "TOPLEFT", 80, -182)
    files.plot:SetWidth(290)
    files.status = label(files, B.status or "", "GameFontHighlightSmall", "TOPLEFT", 10, -198)
    files.status:SetWidth(360)
    files.help = W.HelpButton(files, {
        "This Den: a save of the whole Den by name (the autosave keeps it anyway).",
        "Build: Save or Share your props in this area by name; Load a name or a code where you stand.",
        "Prefab: the selection, saved by name; Paste puts a copy down (in edit mode: click the ground).",
        "Plot: Claim one, then Add a friend (or a Visitor) by name.",
        "Every object and Models (the gallery): type a word, Enter; Next loads more.",
    }, 470, "TOPRIGHT", -28, -5)
    files:SetScript("OnShow", function()
        local D = W.DenBar
        local den = D and D.IsActive and D.IsActive()
        if den then files.denLabel:Show(); files.den:Show(); files.denSave:Show() else files.denLabel:Hide(); files.den:Hide(); files.denSave:Hide() end
        if den and (files.den:GetText() or "") == "" then files.den:SetText((D and D.lastName) or "my-den") end
        files.plot:SetText(B.PlotText())
    end)
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
    local sq = solid(t, "BACKGROUND", S().ED.tile)
    sq:SetPoint("TOPLEFT", 0, 0); sq:SetWidth(TILE); sq:SetHeight(TILE)
    t.hover = solid(t, "BORDER", { 1, 1, 1, 0.05 })
    t.hover:SetPoint("TOPLEFT", 0, 0); t.hover:SetWidth(TILE); t.hover:SetHeight(TILE)
    t.hover:Hide()
    -- the picture where there's no model: the category's icon, faint
    t.icon = t:CreateTexture(nil, "ARTWORK")
    t.icon:SetWidth(38); t.icon:SetHeight(38)
    t.icon:SetPoint("TOP", t, "TOP", 0, -18)
    t.icon:SetTexCoord(0.08, 0.92, 0.08, 0.92)
    t.icon:SetAlpha(0.55)
    -- the model, turning
    t.model = CreateFrame("Model", nil, t)
    t.model:SetWidth(TILE - 4); t.model:SetHeight(TILE - 4)
    t.model:SetPoint("TOPLEFT", 2, -2)
    t.model:Hide()
    -- above the model: the edge (gold when picked), the corner icon, the points and the favourite star
    t.top = CreateFrame("Frame", nil, t)
    t.top:SetWidth(TILE); t.top:SetHeight(TILE)
    t.top:SetPoint("TOPLEFT", 0, 0)
    local lv = t.model.GetFrameLevel and t.model:GetFrameLevel()
    if type(lv) == "number" then t.top:SetFrameLevel(lv + 2) end
    t.edges = {}
    for k, side in ipairs({ { "TOPLEFT", "TOPRIGHT", 0, 1 }, { "BOTTOMLEFT", "BOTTOMRIGHT", 0, 1 },
                            { "TOPLEFT", "BOTTOMLEFT", 1, 0 }, { "TOPRIGHT", "BOTTOMRIGHT", 1, 0 } }) do
        local e = solid(t.top, "OVERLAY", S().ED.edge)
        e:SetPoint(side[1]); e:SetPoint(side[2])
        if side[3] == 1 then e:SetWidth(1) else e:SetHeight(1) end
        t.edges[k] = e
    end
    t.badge = t.top:CreateTexture(nil, "OVERLAY")
    t.badge:SetWidth(14); t.badge:SetHeight(14)
    t.badge:SetPoint("BOTTOMRIGHT", -3, 3)
    t.badge:SetTexCoord(0.08, 0.92, 0.08, 0.92)
    t.badge:SetAlpha(0.8)
    t.badge:Hide()
    t.cost = t.top:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    t.cost:SetPoint("TOPLEFT", 4, -3)
    t.cost.styleKeep = true
    t.fav = t.top:CreateFontString(nil, "OVERLAY", "GameFontNormal")
    t.fav:SetPoint("TOPRIGHT", -4, -2)
    t.fav.styleKeep = true
    local g = S().ED.gold
    t.fav:SetTextColor(g[1], g[2], g[3])
    t.name = t:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    t.name:SetPoint("TOPLEFT", t, "TOPLEFT", 0, -TILE - 1)
    t.name:SetWidth(TILE); t.name:SetHeight(NAME_H)
    t.name:SetJustifyH("CENTER")
    t.name.styleKeep = true
    t:RegisterForClicks("LeftButtonUp", "RightButtonUp")
    t:RegisterForDrag("LeftButton")
    t:SetScript("OnClick", function(self, mouse)
        if not self.item then return end
        if mouse == "RightButton" then B.ToggleFavourite(self.item); return end
        B.Choose(self.item)
    end)
    t:SetScript("OnDoubleClick", function(self) if self.item then B.Place(self.item) end end)
    t:SetScript("OnEnter", function(self)
        self.over = true; paintTile(self)
        local it = self.item
        if not it then return end
        if it.kind then setStatus(it.name .. ((it.kind == "stamp") and (": " .. (it.line or "")) or "") .. "  Double-click to place it.")
        else setStatus(string.format("%s: %d build point%s. %s", it.name, it.cost or 1, (it.cost or 1) == 1 and "" or "s",
            editing() and "Double-click, then click the ground." or "Double-click to place it at your feet.")) end
    end)
    t:SetScript("OnLeave", function(self) self.over = nil; paintTile(self) end)
    -- drag a tile out of the window: placed (at your feet; in edit mode where you click next)
    t:SetScript("OnDragStart", function(self) B.dragging = self.item; if self.item then setStatus("Let go outside the window to place it.") end end)
    t:SetScript("OnDragStop", function()
        local it = B.dragging
        B.dragging = nil
        if it and not (win.IsMouseOver and win:IsMouseOver()) then B.Place(it) end
    end)
    t:Hide()
    return t
end

local function build()
    win = CreateFrame("Frame", "WrathUnleashedBuilderFrame", UIParent)
    win:SetWidth(WIN_W); win:SetHeight(WIN_H)
    win:SetPoint("BOTTOMLEFT", UIParent, "BOTTOMLEFT", 54, 46)
    win:SetFrameStrata("DIALOG")
    win:EnableMouse(true)
    win:SetMovable(true)
    win:RegisterForDrag("LeftButton")
    win:SetScript("OnDragStart", win.StartMoving)
    win:SetScript("OnDragStop", win.StopMovingOrSizing)
    S().EdPanel(win, 0.88)
    tinsert(UISpecialFrames, "WrathUnleashedBuilderFrame")   -- Esc closes it
    local st = store()
    if st.spin ~= nil then B.spin = st.spin and true or false end
    B.az = st.az and true or false

    -- the head: the tab's name, the page, paging, close
    local x0 = CAT_W + 8
    win.title = smallCaps(win, "Everything", true, "TOPLEFT", x0, -10)
    win.close = chip(win, "x", 20, function() win:Hide() end, "TOPRIGHT", -6, -6)
    win.next = chip(win, ">", 22, function() B.Page(1) end, "TOPRIGHT", -32, -6)
    win.prev = chip(win, "<", 22, function() B.Page(-1) end, "TOPRIGHT", -56, -6)
    win.count = win:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    win.count:SetPoint("TOPRIGHT", -84, -11)
    win.count:SetJustifyH("RIGHT")
    win.count.styleKeep = true
    local m = S().ED.muted
    win.count:SetTextColor(m[1], m[2], m[3])

    -- search, A-Z, Spin
    local search = editBox(win, 196, 30, "TOPLEFT", x0, -32)
    search:SetScript("OnTextChanged", function(self)
        B.search = (self:GetText() or ""):sub(1, 30)
        B.offset = 0
        if not SEARCHED[B.tab] then B.Filter() end
    end)
    search:SetScript("OnEnterPressed", function(self)
        self:ClearFocus()
        if SEARCHED[B.tab] then B.Find(B.tab, B.search) end
    end)
    win.search = search
    win.searchHint = smallCaps(win, "Search", false, "LEFT", search, "RIGHT", -60, 0)
    win.az = chip(win, "A-Z", 40, function() B.SetAZ(not B.az) end, "TOPLEFT", x0 + 204, -31)
    win.spinBtn = chip(win, "Spin", 44, function() B.SetSpin(not B.spin) end, "TOPLEFT", x0 + 248, -31)
    win.reload = chip(win, "Reload", 52, function() B.RequestCatalog() end, "TOPLEFT", x0 + 296, -31)

    -- the category column: two narrow columns of icons
    win.tabs = {}
    for i, t in ipairs(TABS) do
        local col, row = (i - 1) % 2, math.floor((i - 1) / 2)
        local b = S().EdIcon(win, 26, catIcon(t), tabText(t), function(self) B.ShowTab(self.tab) end,
            "TOPLEFT", 6 + col * 28, -8 - row * 28)
        b.tab = t
        b.edTipAt, b.edTipTo = "LEFT", "RIGHT"
        win.tabs[i] = b
    end
    local sep = solid(win, "BORDER", S().ED.edge)
    sep:SetPoint("TOPLEFT", CAT_W + 1, -6); sep:SetPoint("BOTTOMLEFT", CAT_W + 1, 6); sep:SetWidth(1)

    -- the grid of tiles
    win.grid = CreateFrame("Frame", nil, win)
    win.grid:SetWidth(COLS * TILE + (COLS - 1) * GAP); win.grid:SetHeight(ROWS * (TILE + NAME_H + GAP))
    win.grid:SetPoint("TOPLEFT", x0, -60)
    win.grid:EnableMouseWheel(true)
    win.grid:SetScript("OnMouseWheel", function(_, delta) B.Scroll(-delta) end)
    win.grid:SetScript("OnUpdate", function(_, elapsed) B.Turntable(elapsed) end)
    win.tiles = {}
    for i = 1, PAGE do win.tiles[i] = buildTile(i) end
    win.empty = label(win, "", "GameFontHighlightSmall", "TOPLEFT", x0 + 6, -70)
    win.empty:SetWidth(COLS * TILE)

    -- the foot: the budget meter and Place
    win.barBg = solid(win, "ARTWORK", { 1, 1, 1, 0.08 })
    win.barBg:SetWidth(120); win.barBg:SetHeight(4)
    win.barBg:SetPoint("BOTTOMLEFT", x0, 12)
    win.bar = win:CreateTexture(nil, "OVERLAY")
    win.bar:SetTexture(1, 1, 1, 1)
    win.bar:SetWidth(1); win.bar:SetHeight(4)
    win.bar:SetPoint("BOTTOMLEFT", x0, 12)
    win.meter = label(win, "", "GameFontHighlightSmall", "BOTTOMLEFT", x0, 20)
    win.meter.styleKeep = true
    win.spawn = chip(win, "Place", 150, function() B.Spawn() end, "BOTTOMRIGHT", -8, 8)
    win.spawn:SetActive(true)

    win:SetScript("OnUpdate", function()
        if B.waiting and B.askedAt and now() - B.askedAt > ANSWER_WAIT then
            B.waiting = nil
            setStatus("The Builder isn't on this realm yet.")
            B.Refresh()
        end
    end)
    win:SetScript("OnShow", function()
        if PlaySound then pcall(PlaySound, "igCharacterInfoOpen") end
        if #B.catalog == 0 then B.RequestCatalog() end
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
        if editing() then win:ClearAllPoints(); win:SetPoint("CENTER", WorldFrame or UIParent, "CENTER", 0, 40) end
        win:Show()
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
B.PAGE, B.COLS, B.STEP_PRESETS, B.ANGLE_STEPS, B.TabText = PAGE, COLS, STEP_PRESETS, ANGLE_STEPS, tabText
