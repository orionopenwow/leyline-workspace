-- lua5.1 stream/tests/test_wuui_denlayout.lua [addon dir] [tools/den-ui dir]
-- The Den editor at real screen sizes (owner, 3 Nov 08:40: "UI too small and bulky"; his 1920x1080 shot: a 28 px
-- strip of tiny text, a rail of generic icons with a "STEP 0.1 0... 1 3" stack, a huge invite bar over the minimap,
-- the chat full of system lines). Laid out by tools/den-ui/fakeui.lua (the same fake the screenshots in docs/den-ui
-- come from) at 1920x1080, 2560x1440 and 1280x720 with the addon's own UI scale: no editor window over the game's
-- minimap, chat, action bars or player frame; text 13 or more; the gallery 5 x 4 tiles of 88 or more (fewer rows on a
-- short screen); every tile a model or an icon on purpose; the tile's camera fitted from the model's size; the Den's
-- system lines over the status pill, not in the chat (a typed dot command still answers there); the den-test's
-- actions and report (DenBar.lua D.TestAction, D.Report) through the editor's own buttons.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local uidir = arg[2] or "tools/den-ui"

local pass, fail = 0, 0
local function check(name, ok, extra)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name .. (extra and ("  " .. tostring(extra)) or "")) end
end

local CATALOG = "2413,woodenchair,seating,Wooden Chair,1,World\\Generic\\Chair01.m2,0.50x0.50x1.10x0.00;"
    .. "1670,barrelofpowder,barrels,Barrel of Powder,1,World\\Generic\\Barrel01.m2,0.60x0.60x1.00x0.00;"
    .. "19006,house,buildings,House,1,-wmo;"
    .. "2562,bakedbread,decor,Baked Bread,1,-;"
    .. "1558,candle,lights,Candle,1,World\\Generic\\Candle01.mdx,0.10x0.10x0.30x0.00;"
    .. "2109,flowers,nature,Fragrant Flowers,1,World\\Generic\\Flowers01.m2;"

local function session(pw, ph)
    -- a fresh game: the fake API, the addon, a Den Lite that answers as labs/den-lite ovmod.rs
    for k in pairs(package.loaded) do if k:match("^den") then package.loaded[k] = nil end end
    WrathUnleashedUI, WrathUnleashedUIDB, WrathUnleashedUIChar = nil, { den = { introSeen = true } }, nil
    local F = dofile(uidir .. "/fakeui.lua")
    local k = math.floor(math.max(0.64, math.min(1.0, 768 / ph)) * 100 + 1e-6) / 100
    F.setup(768 / k * pw / ph, 768 / k)
    F.cvars.uiScale = tostring(k)
    local st = { undo = 0, sel = nil, sent = {} }
    local function state()
        return string.format("mine=%d/800;undo=%d;redo=0;den=owner;room=%d/800;yard=0/0;core=1;lite=1;grid=0;clip=0;area=den;code=-;", st.undo, st.undo, st.undo)
            .. (st.sel and string.format("sel=%d,%d,Barrel of Powder;n=1;pos=1.00,2.00,3.00,0,1.00;rot=0,0,0;ids=%d;gids=%d;", st.sel, 1670, st.sel, st.sel) or "sel=-;n=0;ids=;gids=;")
    end
    F.server = function(msg)
        local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
        st.sent[#st.sent + 1] = { mod, name, data }
        if mod == "wuden" and name == "hello" then F.toClient("wuden\tstatus\t1/1\trole=owner;props=0;saved=5")
        elseif mod == "wubuild" and name == "catalog" then
            F.toClient("wubuild\tcatalog\t1/1\t" .. CATALOG); F.toClient("wubuild\tstate\t1/1\t" .. state())
        elseif mod == "wubuild" and name == "cmd" then
            if data:match("^spawn ") then st.undo = st.undo + 1; st.sel = 40 + st.undo end
            if data == "undo" and st.undo > 0 then st.undo = st.undo - 1; st.sel = nil end
            F.toClient("wubuild\tstate\t1/1\t" .. state())
        end
    end
    F.load(dir)
    F.fire("PLAYER_ENTERING_WORLD")
    return F, st, WrathUnleashedUI
end

local function overlaps(F, names, stock)
    local out = {}
    for _, n in ipairs(names) do
        local a = F.rectOf(n)
        if a then
            for _, g in ipairs(stock) do
                local b = F.rectOf(g)
                if b then
                    local ox = math.min(a.x + a.w, b.x + b.w) - math.max(a.x, b.x)
                    local oy = math.min(a.y + a.h, b.y + b.h) - math.max(a.y, b.y)
                    if ox > 2 and oy > 2 then out[#out + 1] = n .. "/" .. g end
                end
            end
        end
    end
    return out
end

local EDITOR = { "LeylineDenBar", "LeylineDenRail", "WrathUnleashedBuilderFrame", "LeylineEditorStatus", "WrathUnleashedPropsFrame",
    "WrathUnleashedFilesFrame", "LeylineDenMenu", "LeylineDenDrawer" }
local STOCK = { "MinimapCluster", "ChatFrame1", "MainMenuBar", "MultiBarBottomLeft", "MultiBarBottomRight", "MultiBarRight", "MultiBarLeft", "PlayerFrame" }

for _, scr in ipairs({ { 1920, 1080, 4 }, { 2560, 1440, 4 }, { 1280, 720, 2 } }) do
    local pw, ph, rows = scr[1], scr[2], scr[3]
    local tag = pw .. "x" .. ph .. ": "
    local F, st, W = session(pw, ph)
    local D, B = W.DenBar, W.Builder
    check(tag .. "in Your Den the strip, the rail and the pill", LeylineDenBar:IsShown() and LeylineDenRail:IsShown() and LeylineEditorStatus:IsShown())
    -- every window the editor has, open at once
    D.TestAction("gallery")
    D.TestAction("place 1670")
    D.TestAction("props")
    D.ShowMenu(true)
    local ov = overlaps(F, EDITOR, STOCK)
    check(tag .. "nothing over the minimap, the chat, the bars or the player frame (gallery, popover, menu open)", #ov == 0, table.concat(ov, ", "))
    D.ShowMenu(false)
    D.TestAction("files")
    D.Invite()
    ov = overlaps(F, EDITOR, STOCK)
    check(tag .. "Files and the invite card: nothing over them either", #ov == 0, table.concat(ov, ", "))
    check(tag .. "the addon's own overlap check agrees (the den-test's report)", #D.Overlaps() == 0, table.concat(D.Overlaps(), ", "))
    D.CloseDrawer(); B.ShowFiles(false)
    -- text sizes: everything the editor shows is 13 or more
    local small, where = 99, ""
    for _, e in ipairs(F.layout()) do
        if e.kind == "FontString" and e.f.text and e.f.text ~= "" then
            local root = e.root or ""
            local mine = false
            for _, n in ipairs(EDITOR) do if root == n then mine = true end end
            if mine and F.fontSize(e.f) < small then small, where = F.fontSize(e.f), root .. ": " .. tostring(e.f.text) end
        end
    end
    F.endLayout()
    check(tag .. "every text in the editor is 13 or more", small >= 13, small .. " " .. where)
    local bar = D.Bar()
    check(tag .. "the strip is 40 high, its chips 28", (F.rectOf("LeylineDenBar") or {}).h == 40 and bar.undo:GetHeight() == 28)
    check(tag .. (pw < 1600 and "a narrow screen: the strip without its title" or "the strip with its title"), (pw < 1600) == (bar.compact == true))
    -- the gallery: 5 columns, 4 rows of 92 (3 on a short screen), tiles at least 88
    local win = B.Window()
    local shown = 0
    for _, t in ipairs(win.tiles) do if t:IsShown() then shown = shown + 1 end end
    check(tag .. "the gallery: " .. rows .. " rows of 5", B.rows == rows and B.PageSize() == 5 * rows and win.tiles[1]:GetWidth() >= 88)
    local r = B.TileReport()
    check(tag .. "every tile a model or an icon on purpose (none empty)", r.tiles == 6 and r.empty == 0 and r.model == 4 and r.icon == 2, r.tiles .. "/" .. r.model .. "/" .. r.icon .. "/" .. r.empty)
    local rail = F.rectOf("LeylineDenRail")
    check(tag .. "the rail: under the player frame, above the chat", rail and rail.y + rail.h <= ph * 0 + (768 / (F.cvars.uiScale + 0)) - 110 and rail.y >= 215, rail and (rail.y .. "," .. rail.h))
    check(tag .. "no step stack on the rail", #bar.steps == 0)
end

-- ---------------------------------------------------------------- the tiles: models fitted, buildings and props without one
do
    local F, st, W = session(1920, 1080)
    local B = W.Builder
    W.DenBar.TestAction("gallery")
    local byId = {}
    for _, c in ipairs(B.catalog) do byId[c.id] = c end
    check("the size rides with the path (Den Lite's ,<w>x<d>x<h>x<bottom>)", byId[2413].model == "World\\Generic\\Chair01.mdx"
        and byId[2413].size and byId[2413].size.h == 1.1 and byId[2413].size.w == 0.5)
    check("-wmo: a building, -: no model; neither a path", byId[19006].wmo and not byId[19006].model and byId[2562].noModel and not byId[2562].model)
    check("a path without a size still shows", byId[2109].model == "World\\Generic\\Flowers01.mdx" and byId[2109].size == nil)
    local function tile(id) for _, t in ipairs(B.Window().tiles) do if t:IsShown() and t.item and t.item.id == id then return t end end end
    local t = tile(2413)
    local scale, z = B.TileFit(byId[2413].size)
    check("the chair's tile: SetModel, then scaled so its largest side is the fit, moved down by its middle",
        t.model.model == "World\\Generic\\Chair01.mdx" and t.model:IsShown() and math.abs(t.model.mscale - scale) < 1e-9
        and math.abs(scale - B.FIT.yards / 1.1) < 1e-9 and math.abs(t.model.mpos[3] - z) < 1e-9 and z < 0)
    local tc = tile(1558)
    check("a small candle is scaled up more than the chair", tc.model.mscale > t.model.mscale)
    check("the house: the home stone, 'building'", tile(19006).icon.path == "Interface\\Icons\\INV_Misc_Rune_01" and tile(19006).why:GetText() == "building"
        and not tile(19006).model:IsShown())
    check("the bread: its category's icon, 'no preview'", tile(2562).icon.path == "Interface\\Icons\\INV_Misc_Book_09" and tile(2562).why:GetText() == "no preview")
    WrathUnleashedUIDB.den.fit = { yards = 3, lift = 0.5, cam = 0 }
    local s2, z2 = B.TileFit(byId[2413].size)
    check("the fit can be tuned on a PC (WrathUnleashedUIDB.den.fit)", math.abs(s2 - 3 / 1.1) < 1e-9 and math.abs(z2 - (-0.55 * s2 + 0.5)) < 1e-9)
end

-- ---------------------------------------------------------------- the chat: the Den's system lines go to the pill
do
    local F, st, W = session(1920, 1080)
    local D, B = W.DenBar, W.Builder
    local before = #F.chat
    F.systemLine("Placed. Move, turn or Delete it; Undo takes it back.")
    check("a system line in the Den: not in the chat, over the pill", #F.chat == before and B.Pill().note:GetText():find("Placed%.") ~= nil)
    SendChatMessage(".den size")
    F.systemLine("Your Den is 16 yards across.")
    check("what you typed yourself answers in the chat", F.chat[#F.chat] == "Your Den is 16 yards across.")
    F.clock = F.clock + 10
    F.systemLine("Saved.")
    check("a few seconds later: the pill again", F.chat[#F.chat] ~= "Saved.")
    D.SetActive(false); F.realm = "Somewhere else"
    F.systemLine("Elsewhere.")
    check("outside a Den the chat is the game's", F.chat[#F.chat] == "Elsewhere.")
end

-- ---------------------------------------------------------------- the den-test's actions and report
do
    local F, st, W = session(1920, 1080)
    local D, B = W.DenBar, W.Builder
    local function lastCmd() for i = #st.sent, 1, -1 do if st.sent[i][2] == "cmd" then return st.sent[i][3] end end end
    check("ui gallery: the rail's Build opens it", D.TestAction("gallery") and B.Window():IsShown())
    check("ui place <id>: picked and Place pressed: spawn sent", D.TestAction("place 1670") and lastCmd() == "spawn 1670" and st.undo == 1)
    check("Undo 1 on the strip", D.Bar().undo:GetText() == "Undo 1")
    check("ui nudge z 1: the popover's +", D.TestAction("nudge z 1") and lastCmd():match("^move z ") ~= nil and B.Panel():IsShown())
    check("ui key Q: the popover's turn key", D.TestAction("key q") and lastCmd():match("^turn ") ~= nil)
    check("ui menu copy: the menu's row", D.TestAction("menu copy") and lastCmd() == "copy")
    check("ui menu duplicate", D.TestAction("menu duplicate") and lastCmd() == "dup")
    check("ui menu delete", D.TestAction("menu delete") and lastCmd() == "delete")
    check("ui undo: the strip's button", D.TestAction("undo") and lastCmd() == "undo")
    check("ui redo: the strip's button", D.TestAction("redo") and lastCmd() == "redo")
    check("ui menu group: not on Den Lite (lite=1), said so", not D.TestAction("menu group"))
    local n = #st.sent
    check("ui save t1: Files > This Den > Save", D.TestAction("save t1") and st.sent[#st.sent][1] == "wuden" and st.sent[#st.sent][3] == "save t1")
    check("ui open t1: Files > This Den > Open (replace)", D.TestAction("open t1") and st.sent[#st.sent][3] == "load t1 replace")
    check("ui play / build", D.TestAction("play") and D.play and D.TestAction("build") and not D.play)
    D.TestAction("gallery")
    n = #st.sent
    D.TestAction("report")
    local lines = {}
    for i = n + 1, #st.sent do if st.sent[i][2] == "report" then lines[#lines + 1] = st.sent[i][3] end end
    local all = table.concat(lines, "\n")
    check("report: the gallery's tiles", all:find("gallery shown=1 tiles=6 model=4 icon=2 empty=0 rows=4 cols=5 tile=92", 1, true) ~= nil, all)
    check("report: the catalog's models", all:find("catalog n=6 model=4 boxed=3 wmo=1 none=1", 1, true) and all:find("nomodel Baked Bread", 1, true))
    check("report: nothing overlapping", all:find("overlaps none", 1, true) ~= nil)
    check("report: each line fits one addon message", (function() for _, l in ipairs(lines) do if #l > 200 then return false end end return #lines >= 6 end)())
    D.SetActive(false)
    check("outside a Den the test's actions do nothing", not D.TestAction("gallery"))
end

-- no game-brand words on screen
do
    local F = session(1920, 1080)
    WrathUnleashedUI.DenBar.TestAction("gallery"); WrathUnleashedUI.DenBar.ShowMenu(true)
    local bad
    for _, f in ipairs(F.created) do
        local t = f.kind == "FontString" and f.text
        if t and f:IsVisible() then
            for _, w in ipairs({ "World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard" }) do
                if t:find(w, 1, true) and not t:find("World\\", 1, true) then bad = t end
            end
        end
    end
    check("no game-brand names on screen", bad == nil, bad)
end

print(string.format("\n%d/%d passed", pass, pass + fail))
if fail > 0 then os.exit(1) end
