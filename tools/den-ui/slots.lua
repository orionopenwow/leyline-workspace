-- texlua tools/den-ui/slots.lua <addon dir> <out dir> <WxH pixels> <ui scale>
-- The Den's slots (WrathUnleashedUI DenLayout.lua, owner 5 Nov: "the ui is a mess too"), laid out without the game
-- (tools/den-ui/fakeui.lua) and written as JSON per scene; tools/den-ui/slots_png.py draws them with Pillow as
-- rectangles and text so overlaps can be seen. Scenes: build (the gallery, a selection, the status line with the
-- hint), the Dojo (the selection waiting), play mode (its corner, the Dojo, the invite card).
local dir, out, px, scaleArg = arg[1], arg[2], arg[3] or "1920x1080", arg[4]
local here = (arg[0] or ""):match("^(.*)[/\\]") or "."
local F = dofile(here .. "/fakeui.lua")
local pw, ph = px:match("^(%d+)x(%d+)$")
pw, ph = tonumber(pw), tonumber(ph)
local k = tonumber(scaleArg or "") or math.max(0.64, math.min(1.0, 768 / ph))
local uh = 768 / k
F.setup(uh * pw / ph, uh)
F.cvars.uiScale = tostring(k)
WrathUnleashedUIDB = { den = { introSeen = true } }

local CATALOG = "2413,woodenchair,seating,Wooden Chair,1,World\\Generic\\Chair01.m2,0.50x0.50x1.10x0.00;"
    .. "1670,barrelofpowder,barrels,Barrel of Powder,1,World\\Generic\\Barrel01.m2,0.60x0.60x1.00x0.00;"
    .. "2109,flowers,nature,Fragrant Flowers,1,World\\Generic\\Flowers01.m2;19006,house,buildings,House,1,-wmo;"
local undo, sel = 0, nil
local function state()
    return string.format("mine=%d/800;undo=%d;redo=0;den=owner;room=%d/800;yard=0/0;core=1;lite=1;grid=0;clip=0;area=den;code=-;", undo, undo, undo)
        .. (sel and string.format("sel=%d,1670,Barrel of Powder;n=1;pos=1.00,2.00,3.00,0,1.00;rot=0,0,0;ids=%d;gids=%d;", sel, sel, sel) or "sel=-;n=0;ids=;gids=;")
end
F.server = function(msg)
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod == "wuden" and name == "hello" then F.toClient("wuden\tstatus\t1/1\trole=owner;props=0;saved=5")
    elseif mod == "wubuild" and name == "catalog" then F.toClient("wubuild\tcatalog\t1/1\t" .. CATALOG); F.toClient("wubuild\tstate\t1/1\t" .. state())
    elseif mod == "wubuild" and name == "cmd" then
        if data:match("^spawn ") then undo = undo + 1; sel = 40 + undo end
        F.toClient("wubuild\tstate\t1/1\t" .. state())
    end
end
F.load(dir)
F.fire("PLAYER_ENTERING_WORLD")
local W = WrathUnleashedUI
local D, B, J = W.DenBar, W.Builder, W.Dojo
F.toClient("wuden\tdojo\t1/1\tserver=lite;fly=0;speed=1;dummies=0;ring=0;combat=1;class=mage;level=80;fight=0;parse=-;best=-;god=0")

D.TestAction("gallery"); D.TestAction("place 1670"); D.TestAction("props")
F.tick(0.6)
F.dump(out .. "/1-build.json", "Build: the gallery, a selection, the status line", px .. " at " .. k)
J.Show(true); J.Page("train")
F.dump(out .. "/2-dojo.json", "The Dojo in the inspector (the selection waits)", px .. " at " .. k)
J.Page("jaina")
F.dump(out .. "/3-dojo-jaina.json", "The Dojo's Jaina tab", px .. " at " .. k)
J.Show(false)
D.SetPlay(true); J.Show(true); J.Page("train")
D.Invite(); F.toClient("wuden\tinvite\t1/1\thttps://relay.example.app/join/abcdefghijklmnop#ABCDEFGHJKLMNPQRSTUVWXYZ")
F.dump(out .. "/4-play.json", "Play mode: its corner, the Dojo, the invite card", px .. " at " .. k)
print("overlaps: " .. (#D.Overlaps() == 0 and "none" or table.concat(D.Overlaps(), ", ")))
