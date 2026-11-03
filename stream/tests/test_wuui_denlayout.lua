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

local function session(pw, ph, scale)
    -- a fresh game: the fake API, the addon, a Den Lite that answers as labs/den-lite ovmod.rs
    for k in pairs(package.loaded) do if k:match("^den") then package.loaded[k] = nil end end
    WrathUnleashedUI, WrathUnleashedUIDB, WrathUnleashedUIChar = nil, { den = { introSeen = true } }, nil
    local F = dofile(uidir .. "/fakeui.lua")
    local k = scale or math.floor(math.max(0.64, math.min(1.0, 768 / ph)) * 100 + 1e-6) / 100
    F.setup(768 / k * pw / ph, 768 / k)
    F.cvars.uiScale = tostring(k)
    local st = { undo = 0, sel = nil, sent = {} }
    local function state()
        return string.format("mine=%d/800;undo=%d;redo=0;den=owner;room=%d/800;yard=0/0;core=1;lite=1;grid=0;clip=0;area=den;code=-;", st.undo, st.undo, st.undo)
            .. (st.caps or "") .. (st.sel and string.format("sel=%d,%d,Barrel of Powder;n=1;pos=1.00,2.00,3.00,0,1.00;rot=0,0,0;ids=%d;gids=%d;", st.sel, 1670, st.sel, st.sel) or "sel=-;n=0;ids=;gids=;")
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

-- ---------------------------------------------------------------- a tall chat (owner's den-features 3 Nov: the gallery over
-- ChatFrame1 after the stamps panel): the gallery goes above it or beside it, never over it, also when the chat grows
-- while the gallery is open; a target frame shown keeps it off too
for _, scr in ipairs({ { 1920, 1080 }, { 1280, 720 } }) do
    local F, st, W = session(scr[1], scr[2])
    local D, B = W.DenBar, W.Builder
    local tag = scr[1] .. "x" .. scr[2] .. " tall chat: "
    ChatFrame1:SetHeight(230)
    D.TestAction("gallery")
    B.Window():GetScript("OnUpdate")(B.Window(), 1.5)   -- open already: laid out again within a second
    local GAL = { "WrathUnleashedBuilderFrame" }
    local ov = overlaps(F, GAL, STOCK)
    check(tag .. "the gallery clear of the chat", #ov == 0 and B.Window():IsShown(), table.concat(ov, ", "))
    check(tag .. "still 2 rows or more", (B.rows or 0) >= 2, B.rows)
    ChatFrame1:SetHeight(120)
    B.Layout()
    D.TestAction("rail stamps")
    ChatFrame1:SetHeight(260)
    B.Window():GetScript("OnUpdate")(B.Window(), 1.5)
    ov = overlaps(F, GAL, STOCK)
    check(tag .. "the chat grew while it was open: moved off within a second", #ov == 0, table.concat(ov, ", "))
    local own = {}
    for _, l in ipairs(D.Overlaps()) do if l:match("^WrathUnleashedBuilderFrame/") then own[#own + 1] = l end end
    check(tag .. "the den-test's own check agrees", #own == 0, table.concat(own, ", "))
    TargetFrame:Show()
    ov = overlaps(F, { "WrathUnleashedBuilderFrame" }, { "TargetFrame", "PlayerFrame", "MinimapCluster" })
    check(tag .. "nothing over the target frame either", #ov == 0, table.concat(ov, ", "))
end
-- the place itself (pure): the usual one when free; above a tall chat; beside it when above has no room
do
    local F, st, W = session(1920, 1080)
    local B = W.Builder
    local x, b, rows, ok = B.PlaceGallery(1500, 1080, {})
    check("place: the usual one when nothing is in the way", ok and x == 80 and b == 232, x .. "," .. b)
    local chat = { l = 32, b = 95, r = 462, t = 420 }
    x, b, rows, ok = B.PlaceGallery(1500, 1080, { chat })
    check("place: above or beside a tall chat", ok and not B.Hits(x, b, x + 668, b + 300, { chat }), x .. "," .. b .. "," .. rows)
    local huge = { l = 32, b = 95, r = 462, t = 900 }
    x, b, rows, ok = B.PlaceGallery(1500, 1080, { huge })
    check("place: beside a chat as tall as the screen", ok and x >= 470, x .. "," .. b .. "," .. rows)
end

-- ---------------------------------------------------------------- nothing clipped (owner 3 Nov 10:35: "fix all UI windows to
-- not be clipped and so boxy"): at 720p, 1080p and 1440p and UI scales 0.64 to 1.0, every Den window in turn (the strip,
-- rail, gallery, selection panel, menu, Files, Snap, the invite card, the Dojo, Den home, the status pill, play mode's
-- bar and its class list): every text fits its box (one line: its width; wrapped: its height), every region inside its
-- own frame, every frame inside its parent (but what opens beside it on purpose: menus, the pill's note) and on screen.
local ROOTS = { LeylineDenBar = true, LeylineDenRail = true, WrathUnleashedBuilderFrame = true, LeylineEditorStatus = true,
    WrathUnleashedPropsFrame = true, WrathUnleashedFilesFrame = true, LeylineDenPlayPill = true, LeylineDojoPanel = true,
    LeylineDenHomePanel = true, LeylineDenModeBanner = true, LeylineDenGuide = true }
local function clipped(F)
    local out = {}
    local function add(e, why) if #out < 8 then out[#out + 1] = (e.root or "?") .. " " .. e.kind .. " '" .. tostring(e.f.text or e.f.name or "") .. "' " .. why end end
    local rects = {}
    local layout = F.layout()
    for _, e in ipairs(layout) do rects[e.f] = e end
    for _, e in ipairs(layout) do
        local f = e.f
        if ROOTS[e.root or ""] or ROOTS[f.name or ""] then
            -- on screen
            if e.x < -0.5 or e.y < -0.5 or e.x + e.w > F.screen.w + 0.5 or e.y + e.h > F.screen.h + 0.5 then add(e, "off screen") end
            -- a text fits its box
            if f.kind == "FontString" and f.text and f.text ~= "" and not e.f.autoW then
                local tw, size = F.textWidth(f), F.fontSize(f)
                local lines = math.ceil(tw / math.max(1, e.w) - 1e-6)
                if e.h < size * 1.9 then
                    if tw > e.w + 0.5 then add(e, string.format("wider than its box (%.0f > %.0f)", tw, e.w)) end
                elseif lines * (size + 3) - 1 > e.h + 1 then
                    add(e, string.format("taller than its box (%d lines in %.0f)", lines, e.h))
                end
            end
            if f.kind == "EditBox" and not f.scrolls and f.text and f.text ~= "" and F.textWidth(f) + 14 > e.w + 0.5 then add(e, "an input too narrow for its number") end
            -- inside its own frame (a region) or its parent (a frame), unless it opens beside it on purpose
            local par = f.parent
            local pe = par and rects[par]
            local empty = (f.kind == "FontString" and (f.text or "") == "")
            if pe and not empty and not f.outside then
                if e.x < pe.x - 1 or e.y < pe.y - 1 or e.x + e.w > pe.x + pe.w + 1 or e.y + e.h > pe.y + pe.h + 1 then add(e, "outside its frame") end
            end
        end
    end
    F.endLayout()
    return out
end

for _, scr in ipairs({ { 1280, 720 }, { 1920, 1080 }, { 2560, 1440 } }) do
    for _, k in ipairs({ false, 0.64, 1.0 }) do
        local F, st, W = session(scr[1], scr[2], k or nil)
        local D, B = W.DenBar, W.Builder
        local tag = string.format("%dx%d at %s: ", scr[1], scr[2], k and tostring(k) or "the addon's scale")
        local bad = {}
        local function look(what)
            for _, l in ipairs(clipped(F)) do bad[#bad + 1] = what .. ": " .. l end
        end
        look("the editor")
        D.TestAction("gallery"); D.TestAction("place 1670"); D.TestAction("props")
        look("gallery and panel")
        -- the editor's own windows apart too: the gallery, the selection's panel and the status pill (with its box)
        local mine = overlaps(F, { "WrathUnleashedPropsFrame" }, { "WrathUnleashedBuilderFrame", "LeylineEditorStatus", "LeylineDenBar", "LeylineDenRail" })
        local pill = overlaps(F, { "LeylineEditorStatus" }, { "WrathUnleashedBuilderFrame", "LeylineDenRail" })
        for _, l in ipairs(mine) do bad[#bad + 1] = "overlap: " .. l end
        for _, l in ipairs(pill) do bad[#bad + 1] = "overlap: " .. l end
        B.PropsMore(true); look("the panel's More"); B.PropsMore(false)
        D.ShowMenu(true); look("menu"); D.ShowMenu(false)
        D.ShowSnap(true); look("snap"); D.ShowSnap(false)
        B.ShowFiles(true); look("files"); B.ShowFiles(false)
        D.Invite(); F.toClient("wuden\tinvite\t1/1\thttps://relay.example.app/join/abcdefghijklmnop#ABCDEFGHJKLMNPQRSTUVWXYZ"); look("invite"); D.CloseDrawer()
        if W.Dojo then W.Dojo.Show(true); look("dojo"); W.Dojo.Show(false) end
        if W.DenHome then W.DenHome.Show(true); look("den home"); W.DenHome.Show(false) end
        -- the Den guide, every page
        if W.DenGuide then
            for i = 1, #W.DenGuide.PAGES do W.DenGuide.Show(true, i); look("guide page " .. i) end
            local gov = overlaps(F, { "LeylineDenGuide" }, STOCK)
            for _, l in ipairs(gov) do bad[#bad + 1] = "overlap: " .. l end
            W.DenGuide.Show(false)
        end
        D.SetPlay(true); D.ShowClasses(true); look("play mode"); D.ShowClasses(false)
        check(tag .. "every Den window: no text clipped, nothing outside its frame or the screen", #bad == 0, table.concat(bad, " | "))
    end
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
    -- from the side (owner's PC sweep, 3 Nov: "fit-5-playermodel-own"): a PlayerModel, framed by its own camera
    check("the chair's tile: a PlayerModel given the chair, its own framing (scale 1 at 0,0,0), shown",
        t.model.kind == "PlayerModel" and t.model.model == "World\\Generic\\Chair01.mdx" and t.model:IsShown() and t.model.mscale == 1
        and t.model.mpos and t.model.mpos[1] == 0 and t.model.mpos[2] == 0 and t.model.mpos[3] == 0, t.model.kind)
    check("the house: the home stone, 'building'", tile(19006).icon.path == "Interface\\AddOns\\WrathUnleashedUI\\Media\\Icons\\buildings" and tile(19006).why:GetText() == "building"
        and not tile(19006).model:IsShown())
    check("the bread: its category's icon, 'no preview'", tile(2562).icon.path == "Interface\\AddOns\\WrathUnleashedUI\\Media\\Icons\\decor" and tile(2562).why:GetText() == "no preview")
    -- the wheel over a tile zooms that tile only; Shift + wheel pages; the grid's wheel is the gallery's
    local other = tile(1670)
    t:GetScript("OnMouseWheel")(t, 1)
    check("the wheel over a tile zooms its model in (that tile only)", math.abs(t.model.mscale - B.ZOOM_STEP) < 1e-9 and other.model.mscale == 1)
    for _ = 1, 30 do t:GetScript("OnMouseWheel")(t, 1) end
    check("zoom stops at its most", math.abs(t.model.mscale - B.ZOOM_MAX) < 1e-9)
    for _ = 1, 40 do t:GetScript("OnMouseWheel")(t, -1) end
    check("and at its least", math.abs(t.model.mscale - B.ZOOM_MIN) < 1e-9)
    -- a right-drag turns it (the cursor moves while the right button is held); a right-click is still a favourite
    B.SetSpin(false)
    local f0 = t.model.facing
    F.cursor = { 500, 300 }; F.buttons = { RightButton = true }
    t:GetScript("OnMouseDown")(t, "RightButton")
    F.cursor = { 560, 300 }
    B.Window().grid:GetScript("OnUpdate")(B.Window().grid, 0.016)
    check("a right-drag turns the tile's model by the cursor's way", t.model.facing and math.abs(t.model.facing - f0 - 60 * B.TURN_PER_PX) < 1e-6,
        tostring(t.model.facing) .. " " .. tostring(f0))
    F.buttons = {}
    t:GetScript("OnMouseUp")(t, "RightButton")
    local favs = 0
    for _ in pairs((WrathUnleashedUIDB.den or {}).favs or {}) do favs = favs + 1 end
    t:Fire("OnClick", "RightButton")
    local favs2 = 0
    for _ in pairs((WrathUnleashedUIDB.den or {}).favs or {}) do favs2 = favs2 + 1 end
    check("the right-button up after a drag is not a favourite click", favs2 == favs)
    B.SetSpin(true)
    F.tick(0.5)
    check("Spin still turns it, its own turn added", math.abs(t.model.facing - (B.facing + t.turn)) < 1e-6)
    -- the selection's larger preview, on top of the panel (the owner's editor): the selected prop's model, from the side; the same mouse
    F.toClient("wubuild\tstate\t1/1\tmine=1/800;undo=1;redo=0;den=owner;room=1/800;yard=0/0;core=1;lite=1;grid=0;clip=0;area=den;code=-;"
        .. "sel=41,2413,Wooden Chair;n=1;pos=1.00,2.00,3.00,0,1.00;rot=0,0,0;ids=41;gids=41;")
    W.DenBar.TestAction("props")
    local pv = B.preview
    check("the preview: shown on top of the panel with the chair, a PlayerModel at its own framing", pv and pv:IsShown() and pv.model.kind == "PlayerModel"
        and pv.model.model == "World\\Generic\\Chair01.mdx" and pv.model.mscale == 1, pv and tostring(pv.modelPath))
    pv:GetScript("OnMouseWheel")(pv, 1); pv:GetScript("OnMouseWheel")(pv, 1)
    check("the wheel over the preview zooms it", math.abs(pv.model.mscale - B.ZOOM_STEP ^ 2) < 1e-9)
    check("the den-test's view actions: turn and zoom", W.DenBar.TestAction("view preview turn 90") and math.abs(pv.turn - math.pi / 2) < 1e-6
        and W.DenBar.TestAction("view tile zoom 2"))
    F.toClient("wubuild\tstate\t1/1\tmine=1/800;undo=1;redo=0;den=owner;room=1/800;yard=0/0;core=1;lite=1;grid=0;clip=0;area=den;code=-;"
        .. "sel=42,2562,Baked Bread;n=1;pos=1.00,2.00,3.00,0,1.00;rot=0,0,0;ids=42;gids=42;")
    check("a prop without a model: its category's icon in the preview, no model", pv:IsShown() and not pv.model:IsShown() and pv.icon:IsShown()
        and pv.icon:GetTexture() == "Interface\\AddOns\\WrathUnleashedUI\\Media\\Icons\\decor", tostring(pv.icon:GetTexture()))
end

-- ---------------------------------------------------------------- the Den guide (owner 3 Nov): a paged help window
do
    local F, st, W = session(1920, 1080)
    local D, G = W.DenBar, W.DenGuide
    check("the guide: a Guide button on the Den bar", D.Bar().guide ~= nil)
    D.Bar().guide:Click()
    check("Guide opens it at its first page", G.IsOpen() and LeylineDenGuide:IsShown() and G.page == 1 and D.Bar().guide.edActive)
    local need = { "start", "modes", "gallery", "place", "select", "stamps", "dojo", "class", "invite", "keys" }
    local missing = {}
    for _, k in ipairs(need) do if not G.Index(k) then missing[#missing + 1] = k end end
    check("pages: Build and Play, the gallery, placing, selecting, stamps and Jaina, the Dojo, class, invite, keys", #missing == 0, table.concat(missing, ","))
    local text = {}
    for _, p in ipairs(G.PAGES) do text[#text + 1] = table.concat(p[5], " ") end
    text = table.concat(text, " ")
    for _, verb in ipairs({ ".den class", ".den mounts", ".den gear", ".den dojo", ".den jaina build", ".den fly", ".den speed", ".den grow",
        ".build undo", "Invite", "Right-click", "F7", "Esc" }) do
        check("the guide says " .. verb, text:find(verb, 1, true) ~= nil)
    end
    check("short sentences (none over 30 words)", (function()
        for sent in text:gmatch("[^%.]+") do local n = 0; for _ in sent:gmatch("%S+") do n = n + 1 end; if n > 30 then return false end end
        return true end)())
    G.win = LeylineDenGuide
    LeylineDenGuide.next:Click()
    check("Next: the next page", G.page == 2 and LeylineDenGuide.heading:GetText() == G.PAGES[2][4])
    LeylineDenGuide.back:Click()
    check("Back: the page before", G.page == 1)
    LeylineDenGuide.list[G.Index("dojo")]:Click()
    check("a page from the list", G.PAGES[G.page][1] == "dojo" and LeylineDenGuide.icon.iconName == "dojo")
    check("its icons are ours", (function() for _, p in ipairs(G.PAGES) do if not W.Style.Icon(p[3]):find("Media\\Icons\\" .. p[3], 1, true) then return false end end return true end)())
    G.Show(false)
    F.toClient("wuden\tguide\t1/1\topen")
    check(".den help: Den Lite's 'guide open' opens it", G.IsOpen())
    G.Show(false)
    SlashCmdList["LEYLINEDEN"]("guide")
    check("/den guide opens it", G.IsOpen())
    SlashCmdList["LEYLINEDENGUIDE"]("keys")
    check("/denguide keys: at that page", G.PAGES[G.page][1] == "keys")
    check("the den-test's action and report", D.TestAction("guide stamps") and G.Report():find("open=1 page=" .. G.Index("stamps"), 1, true) ~= nil)
    check("Esc closes it (a special frame)", (function() for _, n in ipairs(UISpecialFrames or {}) do if n == "LeylineDenGuide" then return true end end return false end)())
    D.SetPlay(true)
    check("play mode's corner has Guide too", D.Bar().pill.play.guide ~= nil)
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

-- ---------------------------------------------------------------- the selection panel as the owner's editor (3 Nov video): a
-- pick in hand, then placed and selected; the pad from where you stand; Swap with the gallery's pick; the tabs
do
    local F, st, W = session(1920, 1080)
    local D, B = W.DenBar, W.Builder
    local function lastCmd() for i = #st.sent, 1, -1 do if st.sent[i][2] == "cmd" then return st.sent[i][3] end end end
    local caps = "caps=spawn,move,turn,delete,undo,redo,copy,paste,dup,step,grow,bring,ground,settle,upright,random,centre,reset,swap,swapkind,seladd,lock;"
    D.TestAction("gallery")
    B.InHand(B.ById(1670))
    local panel = B.Panel()
    check("a pick in hand: the panel says so, its model on top, the rest waits", panel:IsShown() and panel.stateText:GetText() == "in hand · click the ground"
        and panel.title2:GetText() == "Barrel of Powder" and not panel.body:IsShown() and panel.handText:IsShown() and panel.preview:IsShown())
    B.HandDone(true)
    st.caps, st.sel = caps, 41
    F.toClient("wubuild\tstate\t1/1\tmine=1/800;undo=1;redo=0;den=owner;room=1/800;yard=0/0;core=1;lite=1;grid=0;clip=0;area=den;code=-;" .. caps
        .. "sel=41,1670,Barrel of Powder;n=1;pos=1.00,2.00,3.00,0,1.00;rot=0,0,0;ids=41;gids=41;")
    check("placed: selected at once, the panel open on it", panel:IsShown() and panel.body:IsShown() and not panel.handText:IsShown()
        and panel.stateText:GetText() == "selected" and panel.selTitle:GetText() == "SELECTION · 1")
    check("POSITION in red, green, blue", panel.rowLabel.x.tcolor and panel.rowLabel.x.tcolor[1] > 0.8 and panel.rowLabel.y.tcolor[2] > 0.7 and panel.rowLabel.z.tcolor[3] > 0.8)
    panel.pad.fwd:Click()
    check("the pad: forward from where you stand, by the step (1 yd)", lastCmd() == "step fwd 1")
    panel.stepChips[2]:Click(); panel.pad.left:Click()
    check("the step 0.1: left by 0.1", lastCmd() == "step left 0.1")
    panel.pad.bigger:Click()
    check("bigger", lastCmd() == "grow 1.25")
    panel.pad.bring:Click()
    check("the middle: bring it here", lastCmd() == "bring")
    panel.pad.up:Click()
    check("lift up by the step", lastCmd() == "move z 0.1")
    panel.pad.turnleft:Click()
    check("turn left by the turn step", lastCmd() == "turn 15")
    panel.actions.Settle:Click()
    check("a quick button: Settle", lastCmd() == "settle" and panel.actions.Settle.enabled ~= false)
    B.PanelTab("swap")
    check("the tabs: Swap · keys shows its page only", panel.pages.swap:IsShown() and not panel.pages.piece:IsShown() and panel.tabs.swap:IsActive()
        and panel.hint:GetText():find("Ins swap"))
    D.TestAction("pick 2413")
    D.TestAction("key insert")
    check("Ins: Swap with the gallery's pick", lastCmd() == "swap 2413" and B.inHand == nil)
    D.TestAction("key shift-insert")
    check("Shift+Ins: all of its kind", lastCmd() == "swapkind 2413")
    panel.swap.Swap:Click()
    check("the Swap button", lastCmd() == "swap 2413")
    B.PanelTab("piece")
    st.sel = nil
    F.toClient("wubuild\tstate\t1/1\tmine=1/800;undo=1;redo=0;den=owner;room=1/800;yard=0/0;core=1;lite=1;grid=0;clip=0;area=den;code=-;sel=-;n=0;ids=;gids=;")
    check("nothing selected: the selection's keys given back", B.keysOn ~= true)
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
    check("ui key .: the panel's turn key (the owner's editor: , and . turn)", D.TestAction("key .") and lastCmd():match("^turn ") ~= nil)
    check("no movement key is taken (Q, E, the arrows)", not D.TestAction("key q") and not D.TestAction("key up"))
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
