-- lua5.1 tools/den-ui/gm-scene.lua <addon dir> <out dir> [screen WxH in pixels]
-- The GM panel (AzerothAdmin, /aa) before and after the Leyline skin (WrathUnleashedUI SkinGM.lua), laid out without
-- the game: AzerothAdmin's window as stand-ins (tools/den-ui/aa-stub.lua, its names, sizes and anchors read from its
-- source), written as JSON for tools/den-ui/render.cjs:
--   node tools/den-ui/render.cjs <out dir> docs/den-ui 1920x1080 gm-
-- gives docs/den-ui/gm-before-1080.png (the stand-ins in AzerothAdmin's own grey look), gm-after-1080.png (the GM
-- tab) and gm-after-1080-char.png (the character tab, its own layout in the new look).
local dir, out, px = arg[1], arg[2], arg[3] or "1920x1080"
local here = (arg[0] or ""):match("^(.*)[/\\]") or "."
local F = dofile(here .. "/fakeui.lua")
local A = dofile(here .. "/aa-stub.lua")
local pw, ph = px:match("^(%d+)x(%d+)$")
pw, ph = tonumber(pw), tonumber(ph)
local k = math.floor(math.max(0.64, math.min(1.0, 768 / ph)) * 100 + 1e-6) / 100
local uh = 768 / k
local uw = uh * pw / ph
F.setup(uw, uh)
local tag = tostring(ph)

WrathUnleashedUIDB = { gmSkin = false }
A.build(dofile(here .. "/azerothadmin-frames.lua"))
F.load(dir)
F.fire("ADDON_LOADED", "WrathUnleashedUI")
F.fire("PLAYER_LOGIN")
local G = WrathUnleashedUI.SkinGM
F.dump(out .. "/before-" .. tag .. ".json", "GM panel before: AzerothAdmin's own look (stand-ins)", "")

G.SetEnabled(true)
-- a GM mid-session: GM mode and fly sent on, whispers off; a value in the parameter field
ma_gmonbutton:Click(); ma_flyonbutton:Click(); ma_whisperoffbutton:Click(); ma_cheatgodonbutton:Click()
ma_speedslider:SetValue(3)
ma_speedsliderText:SetText("Speed: 3.0")
F.dump(out .. "/after-" .. tag .. ".json", "GM panel after: the Leyline skin, GM tab", "")

AzerothAdmin:InstantGroupToggle("char")
F.dump(out .. "/after-" .. tag .. "-char.json", "GM panel after: the character tab in the new look", "")
print("GM scenes written to " .. out)
