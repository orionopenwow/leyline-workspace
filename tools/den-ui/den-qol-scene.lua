-- lua5.1 tools/den-ui/den-qol-scene.lua <addon dir> <out dir> [screen WxH in pixels]
-- The builder's comforts (12 Nov; docs/DEN-NEXT.md "The builder's comforts"), laid out without the game
-- (tools/den-ui/fakeui.lua) and written as JSON for tools/den-ui/render.cjs:
--   node tools/den-ui/render.cjs <out dir> docs/den-ui 1920x1080 den-
-- Scenes: the first build's tour on arrival; a prop under the cursor with the tip's second line; three selected and
-- dragged together; a card in hand snapped by the magnet; Alt+drag's circle; the Tools window (Magnet, On the slope;
-- Fill gap, Fill box); the keys card; the Guide's Keys page. A fake Den Lite answers as labs/den-lite ovmod.rs and
-- qol.rs do. No game data: the world under the editor is drawn as nothing (render.cjs draws the game's own frames as
-- outlines).
local dir, out, px = arg[1], arg[2], arg[3] or "1920x1080"
local here = (arg[0] or ""):match("^(.*)[/\\]") or "."
local F = dofile(here .. "/fakeui.lua")
local pw, ph = px:match("^(%d+)x(%d+)$")
pw, ph = tonumber(pw), tonumber(ph)
local k = math.floor(math.max(0.64, math.min(1.0, 768 / ph)) * 100 + 1e-6) / 100
local uh = 768 / k
local uw = uh * pw / ph
F.setup(uw, uh)
F.cvars.useUiScale, F.cvars.uiScale, F.cvars.gxResolution = "1", tostring(k), px

local CATALOG = {
    { 2413, "Wooden Chair", "seating", "World\\Generic\\Human\\Passive Doodads\\Chairs\\Chair01.m2", "0.50x0.50x1.10x0.00" },
    { 181075, "Long Table", "tables", "World\\Generic\\Human\\Passive Doodads\\Tables\\LongTable01.m2", "2.40x0.90x0.90x0.00" },
    { 1670, "Barrel of Powder", "barrels", "World\\Generic\\Human\\Passive Doodads\\Barrels\\Barrel01.m2", "0.60x0.60x1.00x0.00" },
    { 19028, "Stone Wall", "fences", "World\\Generic\\Human\\Passive Doodads\\Walls\\StoneWall.m2", "4.00x0.50x2.00x0.00" },
    { 1953, "Campfire", "fires", "World\\Generic\\Human\\Passive Doodads\\Campfire\\Campfire01.m2", "1.20x1.20x0.60x0.00" },
    { 2109, "Fragrant Flowers", "nature", "World\\Generic\\Nature\\Flowers\\Flowers01.m2", "0.80x0.80x0.50x0.00" },
}
local CAPS = "caps=spawn,near,move,turn,delete,undo,redo,copy,paste,dup,selall,selclear,grid,seladd,grow,setscale,upright,ground,step,"
    .. "preview,hover,commit,axis,grab,cam,gizmo,mode,click,drag,tool,path,array,ring,align,distribute,land,solid,hide,show,group,"
    .. "measure,history,stamps,dupby,selnear,slope,camframe,cammark,camto,flyspeed,groupmove,slopefollow,magnet,nudge,selcircle,fillgap,fillbox,free;"
local function catalogLine()
    local items = {}
    for _, c in ipairs(CATALOG) do
        items[#items + 1] = string.format("%d,%s,%s,%s,1,%s,%s", c[1], c[2]:lower():gsub("[^%w]", ""), c[3], c[2], c[4], c[5])
    end
    return table.concat(items, ";") .. ";"
end
local sel = nil
local function stateLine()
    local s = "mine=6/800;pts=6/800;undo=4;redo=0;den=owner;room=6/800;yard=0/0;core=1;lite=1;grid=0;clip=0;area=den;code=-;" .. CAPS
    if sel then
        s = s .. string.format("sel=%d,%d,%s;n=%d;pos=16232.20,16257.00,13.20,90,1.00;rot=90,0,0;ids=%d;gids=%d;", sel.id, sel.entry, sel.name, sel.n or 1, sel.id, sel.id)
    else
        s = s .. "sel=-;n=0;ids=;gids=;"
    end
    return s
end
F.server = function(msg)
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod == "wuden" and name == "hello" then F.toClient("wuden\tstatus\t1/1\trole=owner;props=6;saved=8")
    elseif mod == "wubuild" and name == "catalog" then F.toClient("wubuild\tcatalog\t1/1\t" .. catalogLine()); F.toClient("wubuild\tstate\t1/1\t" .. stateLine())
    elseif mod == "wubuild" and name == "state" then F.toClient("wubuild\tstate\t1/1\t" .. stateLine())
    elseif mod == "wubuild" and name == "cmd" then
        if data:match("^preview %d") then F.toClient("wubuild\taim\t1/1\tplace," .. data:match("^preview (%d+)") .. ",1543") end
    end
end

WrathUnleashedUIDB = { den = { introSeen = true } }
F.load(dir)
local W = WrathUnleashedUI
local D, B, G, K, C = W.DenBar, W.Builder, W.Gizmo, W.DenKeys, W.DenCoach

local function scene(name, title, notes) F.dump(out .. "/" .. name .. ".json", title, notes) end
local function setSel(id, n) sel = id and { id = id, entry = 2413, name = "Wooden Chair", n = n } or nil; F.toClient("wubuild\tstate\t1/1\t" .. stateLine()) end
local function hideTour() if C and C.Hide then C.Hide() end end

F.fire("PLAYER_ENTERING_WORLD")
F.fire("ADDON_LOADED", "WrathUnleashedUI")
F.fire("PLAYER_LOGIN")
D.SetActive(true)
F.tick(0.6); F.tick(0.6)

-- 1: the tour on arrival
scene("qol-1-tour", "Your first build: the tour shows itself on arrival", "nine steps; each moves on when you've done it")

-- 2: a prop under the cursor: its tip says what the mouse does
hideTour(); C.dismissed = true
setSel(41, 1)
F.cursor = { pw * 0.55, ph * 0.45 }
G.hovered = { id = 42, name = "Long Table" }
G.TipUpdate()
F.tick(0.1)
scene("qol-2-hover-tip", "A prop under the cursor: its name and what the mouse does", "click carries, drag slides, Shift+click adds, Q E turn")

-- 3: three selected and one dragged: they move together
setSel(41, 3)
G.hovered = nil; G.TipUpdate()
G.dragging = true
G.ShowTag()
F.tick(0.1)
scene("qol-3-group-drag", "Three selected, one dragged: they move together (one undo)", "the status line says how many")
G.dragging = nil

-- 4: a card in hand, snapped by the magnet
if K.MagnetToggle and not K.magnet then K.MagnetToggle() end
if K.SlopeToggle and not K.slope then K.SlopeToggle() end
B.Choose({ id = 19028, name = "Stone Wall" })
F.toClient("wubuild\tsnapped\t1/1\t1")
F.tick(0.1)
scene("qol-4-magnet", "The magnet: a wall in hand snaps edge to edge with the last one", "M turns it on, Alt holds it off for the precise spot; Ctrl+L follows the slope")
G.Cancel()

-- 5: Alt+drag: the circle
setSel(41, 1)
F.cursor = { pw * 0.5, ph * 0.5 }
K.CircleDown()
F.cursor = { pw * 0.5 + ph * 0.14, ph * 0.5 + ph * 0.06 }
K.CircleUpdate()
F.tick(0.05)
scene("qol-5-circle", "Alt+drag: circle select from the middle out", "Alt+Shift adds to what is selected")
K.circle = nil
if _G.LeylineDenSelectCircle then _G.LeylineDenSelectCircle:Hide() end

-- 6, 7: the Tools window: Select (Magnet, On the slope), Arrange (Fill gap, Fill box)
setSel(41, 2)
local T = W.BuildTools
if T and T.Show then T.Show("select") end
F.tick(0.1)
scene("qol-6-tools-select", "Tools > Select: Magnet and On the slope (kept between sessions)", "with the grid, the turn step and Land")
if T and T.Show then T.Show("arrange") end
F.tick(0.1)
scene("qol-7-tools-fill", "Tools > Arrange: Fill gap and Fill box", "copies of the first selected between it and the second, or over their box")
if _G.WrathUnleashedToolsFrame then _G.WrathUnleashedToolsFrame:Hide() end

-- 8: the keys card
local intro = D.Intro and D.Intro()
if D.ShowIntro then D.ShowIntro(5) elseif intro then intro:Show() end
F.tick(0.1)
scene("qol-8-keys", "The keys card (the ? on the strip): the new keys at its foot", "nudge, Alt, circle, magnet, slope, /dentour")
intro = D.Intro and D.Intro()
if intro then intro:Hide() end

-- 9: the Guide's Keys page
local Gd = W.DenGuide
if Gd and Gd.Show then Gd.Show(true, "morekeys") end
F.tick(0.1)
scene("qol-9-guide-keys", "The Guide's More keys page", "the builder's comforts in plain sentences")

print("scenes written to " .. out .. string.format(" (UI %.0fx%.0f units at scale %.2f)", uw, uh, k))
