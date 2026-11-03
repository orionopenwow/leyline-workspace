-- lua5.1 tools/den-ui/scenes.lua <addon dir> <out dir> <screen WxH in pixels> [ui scale]
-- The Den editor's scenes, laid out without the game (tools/den-ui/fakeui.lua) and written as JSON for
-- tools/den-ui/render.cjs: the editor as you enter Your Den, the gallery open with a prop picked, a prop selected
-- (its popover), the menu, Files, an invite, play mode. A fake Den Lite answers as labs/den-lite ovmod.rs does (its
-- catalog with model paths, a WMO building with none, the state line). The UI scale is the addon's own fit for the
-- screen (WrathUnleashedUI.lua W.FitScale) unless given: UIParent is 768 / scale UI units high.
local dir, out, px, scaleArg = arg[1], arg[2], arg[3] or "1920x1080", arg[4]
local here = (arg[0] or ""):match("^(.*)[/\\]") or "."
local F = dofile(here .. "/fakeui.lua")
local pw, ph = px:match("^(%d+)x(%d+)$")
pw, ph = tonumber(pw), tonumber(ph)
-- the addon's own fit (W.FitScale): 768 / height, between 0.64 and 1.0
local k = tonumber(scaleArg or "") or math.max(0.64, math.min(1.0, 768 / ph))
k = math.floor(k * 100 + 1e-6) / 100
local uh = 768 / k
local uw = uh * pw / ph
F.setup(uw, uh)
F.cvars.useUiScale, F.cvars.uiScale, F.cvars.gxResolution = "1", tostring(k), px

-- a fake Den Lite (ovmod.rs): the catalog with models, the state line, the Builder's commands
local CATALOG = {
    { 2413, "Wooden Chair", "seating", "World\\Generic\\Human\\Passive Doodads\\Chairs\\Chair01.m2", "0.50x0.50x1.10x0.00" },
    { 2489, "High Back Chair", "seating", "World\\Generic\\Human\\Passive Doodads\\Chairs\\HighBackChair.m2", "0.60x0.60x1.50x0.00" },
    { 181075, "Long Table", "tables", "World\\Generic\\Human\\Passive Doodads\\Tables\\LongTable01.m2", "2.40x0.90x0.90x0.00" },
    { 1670, "Barrel of Powder", "barrels", "World\\Generic\\Human\\Passive Doodads\\Barrels\\Barrel01.m2", "0.60x0.60x1.00x0.00" },
    { 2130, "Wine Cask", "barrels", "World\\Generic\\Human\\Passive Doodads\\Barrels\\Cask01.m2", "1.20x0.90x1.00x0.00" },
    { 19028, "Watch Post", "fences", "World\\Generic\\Human\\Passive Doodads\\Posts\\WatchPost.m2", "2.00x2.00x6.00x0.00" },
    { 1953, "Campfire", "fires", "World\\Generic\\Human\\Passive Doodads\\Campfire\\Campfire01.m2", "1.20x1.20x0.60x0.00" },
    { 1558, "Candle", "lights", "World\\Generic\\Human\\Passive Doodads\\Candles\\Candle01.m2", "0.10x0.10x0.30x0.00" },
    { 2109, "Fragrant Flowers", "nature", "World\\Generic\\Nature\\Flowers\\Flowers01.m2", "0.80x0.80x0.50x0.00" },
    { 13948, "Fancy Bed", "decor", "World\\Generic\\Human\\Passive Doodads\\Beds\\FancyBed.m2", "2.20x1.40x1.20x0.00" },
    { 2695, "Book", "decor", "World\\Generic\\Human\\Passive Doodads\\Books\\Book01.m2", "0.30x0.20x0.10x0.00" },
    { 19006, "House", "buildings", "wmo", "" },
    { 19007, "Inn", "buildings", "wmo", "" },
    { 2686, "Apothecary Table", "tables", "World\\Generic\\Human\\Passive Doodads\\Tables\\Apothecary.m2", "1.80x0.80x1.00x0.00" },
    { 2687, "Bottle", "decor", "World\\Generic\\Human\\Passive Doodads\\Bottles\\Bottle01.m2", "0.10x0.10x0.30x0.00" },
    { 2562, "Baked Bread", "decor", "", "" },
    { 2711, "Charging Stone", "nature", "World\\Generic\\Nature\\Stones\\Stone01.m2", "1.00x1.00x1.40x0.00" },
    { 2148, "Cheese Cutters", "decor", "World\\Generic\\Human\\Passive Doodads\\Food\\Cheese.m2", "0.40x0.40x0.20x0.00" },
    { 181396, "Rug", "decor", "World\\Generic\\Human\\Passive Doodads\\Rugs\\Rug01.m2", "2.00x1.40x0.05x0.00" },
    { 2371, "Skull", "decor", "World\\Generic\\Passive Doodads\\Skulls\\Skull01.m2", "0.30x0.30x0.30x0.00" },
    { 19057, "Bridge", "structure", "wmo", "" },
    { 2552, "Cauldron", "decor", "World\\Generic\\Passive Doodads\\Cauldron\\Cauldron01.m2", "1.20x1.20x1.00x0.00" },
}
local function catalogLine(v2)
    local items = {}
    for _, c in ipairs(CATALOG) do
        local key = c[2]:lower():gsub("[^%w]", ""):sub(1, 20)
        local model = c[4]
        if model == "wmo" then model = v2 and "-wmo" or "" end
        local tail = ""
        if model ~= "" then tail = "," .. model .. ((v2 and c[5] ~= "") and ("," .. c[5]) or "") end
        items[#items + 1] = string.format("%d,%s,%s,%s,1%s", c[1], key, c[3], c[2], tail)
    end
    return table.concat(items, ";") .. ";"
end
local sel, undo, redo = nil, 0, 0
local function stateLine()
    local s = string.format("mine=%d/800;pts=%d/800;undo=%d;redo=%d;den=owner;room=%d/800;yard=0/0;core=1;lite=1;area=den;code=-;",
        undo, undo, undo, redo, undo)
    if sel then
        s = s .. string.format("sel=%d,%d,%s;n=1;pos=%.2f,%.2f,%.2f,%d,%.2f;rot=%d,0,0;ids=%d;gids=%d;", sel.id, sel.entry, sel.name,
            -8913.2, 554.1, 93.1, 45, 1.0, 45, sel.id, sel.id)
    else
        s = s .. "sel=-;n=0;ids=;gids=;"
    end
    return s
end
local V2 = os.getenv("DENUI_V1") == nil
F.server = function(msg)
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod == "wuden" and name == "hello" then
        F.toClient("wuden\tstatus\t1/1\trole=owner;props=3;saved=12")
    elseif mod == "wubuild" and name == "catalog" then
        F.toClient("wubuild\tcatalog\t1/1\t" .. catalogLine(V2))
        F.toClient("wubuild\tstate\t1/1\t" .. stateLine())
    elseif mod == "wubuild" and name == "state" then
        F.toClient("wubuild\tstate\t1/1\t" .. stateLine())
    elseif mod == "wubuild" and name == "cmd" then
        local verb, rest = data:match("^(%S+)%s*(.*)$")
        if verb == "spawn" then
            local id = tonumber(rest)
            for _, c in ipairs(CATALOG) do if c[1] == id then sel = { id = 40 + undo, entry = id, name = c[2] } end end
            undo = undo + 1
        elseif verb == "near" then sel = sel or { id = 41, entry = 2413, name = "Wooden Chair" } end
        F.toClient("wubuild\tstate\t1/1\t" .. stateLine())
    end
end

F.load(dir)
local W = WrathUnleashedUI
local D, B = W.DenBar, W.Builder
WrathUnleashedUIDB = WrathUnleashedUIDB or {}
WrathUnleashedUIDB.den = { introSeen = true }

local function scene(name, title, notes)
    F.dump(out .. "/" .. name .. ".json", title, notes)
end
local function closeAll()
    if D.ShowMenu then pcall(D.ShowMenu, false) end
    if D.ShowSnap then pcall(D.ShowSnap, false) end
    if B.ShowProps then pcall(B.ShowProps, false) end
    if B.ShowFiles then pcall(B.ShowFiles, false) end
    if D.CloseDrawer then pcall(D.CloseDrawer) end
    if D.CloseInvite then pcall(D.CloseInvite) end
end

F.fire("PLAYER_ENTERING_WORLD")
F.fire("ADDON_LOADED", "WrathUnleashedUI")
F.fire("PLAYER_LOGIN")
-- the system lines Den Lite says at login (a game whose bar said hello late)
F.systemLine("Builder: F7 or /builder (place, move, turn, delete, undo). Den panel: /den (Invite a friend, Shape the ground, save and load).")
F.systemLine("Placed. Move, turn or Delete it; Undo takes it back.")
F.tick(0.6)
scene("1-editor", "Entering Your Den: the editor", "the top strip, the tool rail, the status pill")

if W.ToggleBuilder then W.ToggleBuilder() end
F.tick(0.6)
local first
for _, c in ipairs(B.catalog or {}) do if c.id == 1670 then first = c end end
if first then B.Choose(first) end
F.tick(0.6)
scene("2-gallery", "The gallery: a prop picked", "5 x 4 tiles, models spinning; WMO buildings and props without a model show an icon")

if first then B.Place(first) end
F.toClient("wubuild\tstate\t1/1\t" .. stateLine())
if B.ShowProps then B.ShowProps(true) end
F.tick(0.6)
scene("3-selection", "A prop placed and selected: its popover", "exact numbers, nudges, the selection's actions")

closeAll()
if D.ShowMenu then D.ShowMenu(true) end
F.tick(0.1)
scene("4-menu", "The menu", "the rarely used actions, grouped")

closeAll()
if B.ShowFiles then B.ShowFiles(true) end
F.tick(0.1)
scene("5-files", "Files", "the Den's saves by name")

closeAll()
if D.Invite then D.Invite() end
F.toClient("wuden\tinvite\t1/1\thttps://relay.example.app/join/abcdefghijklmnop#ABCDEFGHJKLMNPQRSTUVWXYZ")
F.tick(0.1)
scene("6-invite", "Invite: one link for one friend", "a small toast with the link to copy")

closeAll()
if W.ToggleBuilder and B.Window() and B.Window():IsShown() then W.ToggleBuilder() end
if D.SetPlay then D.SetPlay(true) end
F.tick(0.6)
scene("7-play", "Play mode (F8)", "only a small chip: Back to building")

-- the chat: what Den Lite said that reached the chat frame
local h = assert(io.open(out .. "/chat.txt", "wb"))
for _, l in ipairs(F.chat) do h:write(l, "\n") end
h:close()
print("scenes written to " .. out .. string.format(" (UI %.0fx%.0f units at scale %.2f)", uw, uh, k))
