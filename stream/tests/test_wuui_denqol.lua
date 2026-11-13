-- texlua stream/tests/test_wuui_denqol.lua [addon dir] [tools/den-ui dir]
-- The builder's comforts (12 Nov; DenKeys.lua, Gizmo.lua, BuildTools.lua, DenCoach.lua; Den Lite labs/den-lite
-- src/qol.rs). Owner: "creates and suggests features for the den and makes it more user intuitive, adding quality of
-- life features and everything, including Noggit and the editor that follows terrain, click and drag to move". The
-- whole addon in .toc order on tools/den-ui/fakeui.lua with a fake Den Lite answering as ovmod.rs / gizmo.rs / qol.rs:
--   * the nudge keys (Numpad, Ctrl+arrows): only with a selection; the grid's step, Alt fine, Shift big, turn keys;
--     an older Den Lite gets the Builder's own `step` / `turn`;
--   * M the magnet and Ctrl+L follow the slope: said to Den Lite, kept, said again on entering build mode, the Tools
--     window's chips lit; `snapped` shows in the status line;
--   * Alt held with something in hand: `hover ... free`; Q / E by 1 degree;
--   * Alt+drag: the circle drawn and `selcircle`; Alt+Shift adds; a still Alt+click sends nothing;
--   * a drag of one of several: the status line says they move together;
--   * the hover tip's second line says what the mouse does;
--   * Fill gap / Fill box (the Tools window and DenKeys): two selected first;
--   * the first build's tour (DenCoach.lua): shows by itself, moves on as each step is done, Skip, End, /dentour;
--   * the keys card and the Guide's Keys page say the new keys; no game-brand words; play mode leaves none bound.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local uidir = arg[2] or "tools/den-ui"

local pass, fail = 0, 0
local function check(name, ok, extra)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name .. (extra and ("  " .. tostring(extra)) or "")) end
end

WrathUnleashedUI, WrathUnleashedUIDB = nil, { den = { introSeen = true } }
local F = dofile(uidir .. "/fakeui.lua")
F.setup(1352, 1081)
F.cvars.uiScale = "0.71"

local CAPS = "caps=spawn,near,move,turn,delete,undo,redo,copy,paste,dup,selall,selclear,grid,seladd,grow,setscale,upright,ground,step,"
    .. "preview,hover,commit,axis,grab,cam,gizmo,mode,click,drag,tool,dupby,selnear,slope,camframe,cammark,camto,flyspeed,"
    .. "groupmove,slopefollow,magnet,nudge,selcircle,fillgap,fillbox,free;"
local st = { sel = nil, n = 1, x = 10, y = 20, z = 5, yaw = 90, scale = 1, grid = 0, caps = CAPS, cmds = {} }
local function state()
    local s = string.format("mine=1/800;undo=1;redo=0;den=owner;room=1/800;yard=0/0;core=1;lite=1;grid=%s;clip=0;area=den;code=-;", tostring(st.grid)) .. st.caps
    if st.sel then
        s = s .. string.format("sel=%d,2413,Wooden Chair;n=%d;pos=%.2f,%.2f,%.2f,%.0f,%.2f;rot=0,0,0;ids=%d;gids=%d;", st.sel, st.n, st.x, st.y, st.z, st.yaw, st.scale, st.sel, st.sel)
    else
        s = s .. "sel=-;n=0;ids=;gids=;"
    end
    return s
end
local CATALOG = "2413,woodenchair,seating,Wooden Chair,1,World\\Generic\\Chair01.m2,0.50x0.50x1.10x0.00;"
F.server = function(msg)
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod == "wuden" and name == "hello" then F.toClient("wuden\tstatus\t1/1\trole=owner;props=1;saved=5")
    elseif mod == "wubuild" and name == "catalog" then F.toClient("wubuild\tcatalog\t1/1\t" .. CATALOG); F.toClient("wubuild\tstate\t1/1\t" .. state())
    elseif mod == "wubuild" and name == "state" then F.toClient("wubuild\tstate\t1/1\t" .. state())
    elseif mod == "wubuild" and name == "cmd" then
        st.cmds[#st.cmds + 1] = data
        if data == "selclear" then st.sel = nil; F.toClient("wubuild\tstate\t1/1\t" .. state()) end
        if data:match("^preview %d") then F.toClient("wubuild\taim\t1/1\tplace," .. data:match("^preview (%d+)") .. ",1543") end
        if data == "preview off" or data == "commit" then F.toClient("wubuild\taim\t1/1\toff") end
        local f, r, u = data:match("^nudge ([%-%d%.]+) ([%-%d%.]+) ([%-%d%.]+)")
        if f then F.toClient(string.format("wubuild\tnudged\t1/1\t%s,%s,%s,0", f, r, u)) end
    end
end
F.load(dir)

local mouse = { x = 960, y = 540, right = false, left = false }
local keys = { shift = false, ctrl = false, alt = false }
function GetCursorPosition() return mouse.x, mouse.y end
function IsMouseButtonDown(b) if b == "RightButton" then return mouse.right end; if b == "LeftButton" then return mouse.left end; return false end
function IsShiftKeyDown() return keys.shift end
function IsControlKeyDown() return keys.ctrl end
function IsAltKeyDown() return keys.alt end

F.fire("PLAYER_ENTERING_WORLD")
local W = WrathUnleashedUI
local G, K, D, B, C = W.Gizmo, W.DenKeys, W.DenBar, W.Builder, W.DenCoach
check("DenCoach.lua loaded after DenKeys.lua", C and C.Saw and K and K.Nudge and G)
local n0 = #st.cmds
D.SetActive(true)
F.tick(0.6); F.tick(0.6)

local function cmds(from) local out = {}; for i = (from or 0) + 1, #st.cmds do out[#out + 1] = st.cmds[i] end; return out end
local function has(list, want) for _, v in ipairs(list) do if v == want then return true end end return false end
local function find(list, pat) for _, v in ipairs(list) do if v:match(pat) then return v end end return nil end
local function press(key)
    local b = _G[F.overrides[key] or ""]
    if not b then return false end
    b:Fire("OnClick", "LeftButton", true)
    return true
end
local function bound(owner) return F.bound(owner) end
local function tagText() local t = _G.LeylineGizmoTag; return t and t.fs and t.fs.GetText and (t.fs:GetText() or "") or "" end
local function select(id, n)
    st.sel, st.n = id, n or 1
    F.toClient("wubuild\tstate\t1/1\t" .. state())
end

check("in the Den on Den Lite: build mode", G.build == true)
check("entering build mode: the magnet and the slope said (off the first time)", has(cmds(n0), "gizmo magnet off") and has(cmds(n0), "gizmo slope off"),
    table.concat(cmds(n0), "|"))

-- ---------------------------------------------------------------- the first build's tour
local card = _G.LeylineDenCoach
check("the tour shows by itself the first time (intro seen, never finished)", card and card:IsShown() and C.shown)
check("already in Build mode: the tour starts at step 2, Pick a prop", C.step == 2 and card.head:GetText() == "Pick a prop" and card.count:GetText() == "2 / 9",
    tostring(C.step) .. " " .. tostring(card and card.head:GetText()))
check("it says what was done", card.done:GetText() == "Done: Build mode")
B.Choose({ id = 2413, name = "Wooden Chair" })
check("a card picked: on to Put it down", C.step == 3 and card.head:GetText() == "Put it down")
F.toClient("wubuild\tplaced\t1/1\t41")
check("placed: on to Move it", C.step == 4)
G.Cancel()
-- a plain commit (nothing carried) doesn't count as a move
C.Saw("out", "cmd", "commit")
check("a commit with nothing carried isn't a move", C.step == 4)
F.toClient("wubuild\tcarried\t1/1\t41")
G.Send("commit")
check("carried and put down: on to Turn it", C.step == 5)
G.Send("turnat 41 15")
check("Q / E: on to Copy it", C.step == 6)
G.Send("dupby 1.00 0.00 0.00")
check("Ctrl+D: on to Take it back", C.step == 7)
G.Send("undo")
check("Ctrl+Z: on to Select several", C.step == 8)
select(41, 1)
check("one selected isn't several", C.step == 8)
select(41, 3)
check("three selected: on to Play it", C.step == 9 and card.head:GetText() == "Play it")
G.Send("mode play")
check("F7 to play: the tour is done, the card says so", C.step == 10 and card.head:GetText() == "Your Den is yours" and WrathUnleashedUIDB.den.coach.done == true
    and not card.skip:IsShown())
card.close:Fire("OnClick")
check("Close: hidden, not shown again by itself", not card:IsShown() and not C.Wanted())
SlashCmdList.LEYLINEDENTOUR("")
check("/dentour: from the start again (Build mode passed at once)", card:IsShown() and C.step == 2 and WrathUnleashedUIDB.den.coach.done == false)
card.skip:Fire("OnClick")
check("Skip step", C.step == 3 and card.done:GetText() == "")
SlashCmdList.LEYLINEDENTOUR("off")
check("/dentour off: ended", not card:IsShown() and WrathUnleashedUIDB.den.coach.done == true)
G.Cancel()

-- ---------------------------------------------------------------- the nudge keys
select(nil)
local k = bound("LeylineDenKeys")
check("nothing selected: no nudge keys", not k.NUMPAD8 and not k["CTRL-UP"])
check("always in build mode: M (magnet), Ctrl+L (follow the slope), Alt+drag (circle)", k.M == "LeylineDenKey_magnet" and k["CTRL-L"] == "LeylineDenKey_slopefollow"
    and k["ALT-BUTTON1"] == "LeylineDenKeyCircle" and k["ALT-SHIFT-BUTTON1"] == "LeylineDenKeyCircle")
select(41)
k = bound("LeylineDenKeys")
check("a selection: Numpad 8 2 4 6 9 3 7 1, Ctrl+arrows, Ctrl+PgUp / PgDn, with Alt and Shift", k.NUMPAD8 and k.NUMPAD2 and k.NUMPAD4 and k.NUMPAD6 and k.NUMPAD9
    and k.NUMPAD3 and k.NUMPAD7 and k.NUMPAD1 and k["CTRL-UP"] and k["CTRL-LEFT"] and k["CTRL-PAGEUP"] and k["ALT-NUMPAD8"] and k["SHIFT-NUMPAD6"]
    and k["ALT-CTRL-UP"] and k["CTRL-SHIFT-RIGHT"])
check("Numpad 0 stays Home's (the far view) and never a nudge", not (k.NUMPAD0 and k.NUMPAD0:find("nudge")))
local n = #st.cmds
press("NUMPAD8")
check("Numpad 8: forward by half a yard (the grid off)", has(cmds(n), "nudge 0.50 0.00 0.00 0"), table.concat(cmds(n), "|"))
check("the status line says how far", K.lastNote and K.lastNote:find("Nudged", 1, true))
n = #st.cmds
keys.alt = true; press("ALT-NUMPAD4"); keys.alt = false
check("Alt+Numpad 4: left by a tenth of a yard", has(cmds(n), "nudge 0.00 -0.10 0.00 0"), table.concat(cmds(n), "|"))
n = #st.cmds
keys.shift = true; press("SHIFT-NUMPAD9"); keys.shift = false
check("Shift+Numpad 9: up five steps", has(cmds(n), "nudge 0.00 0.00 2.50 0"), table.concat(cmds(n), "|"))
n = #st.cmds
press("CTRL-RIGHT")
check("Ctrl+Right: right", has(cmds(n), "nudge 0.00 0.50 0.00 0"))
n = #st.cmds
press("NUMPAD7")
check("Numpad 7: turn left by the turn step", has(cmds(n), "nudge 0.00 0.00 0.00 15"), table.concat(cmds(n), "|"))
n = #st.cmds
keys.alt = true; press("ALT-NUMPAD1"); keys.alt = false
check("Alt+Numpad 1: turn right by 1 degree", has(cmds(n), "nudge 0.00 0.00 0.00 -1"), table.concat(cmds(n), "|"))
st.grid = 2; F.toClient("wubuild\tstate\t1/1\t" .. state())
if B.SetSnap then B.SetSnap(2) end
n = #st.cmds
press("NUMPAD2")
check("the grid on (2 yd): a nudge is one grid step", has(cmds(n), "nudge -2.00 0.00 0.00 0"), table.concat(cmds(n), "|"))
if B.SetSnap then B.SetSnap(0) end
st.grid = 0
-- an older Den Lite (no qol.rs): the Builder's own step and turn
st.caps = "caps=spawn,near,move,turn,delete,undo,redo,copy,paste,dup,selall,selclear,grid,step,preview,hover,commit,cam,gizmo,mode,click,drag;"
F.toClient("wubuild\tstate\t1/1\t" .. state())
n = #st.cmds
press("NUMPAD6")
check("an older Den Lite: Numpad 6 is the Builder's step right", has(cmds(n), "step right 0.50"), table.concat(cmds(n), "|"))
n = #st.cmds
press("NUMPAD7")
check("an older Den Lite: Numpad 7 is the Builder's turn", has(cmds(n), "turn 15"), table.concat(cmds(n), "|"))
n = #st.cmds
press("M")
check("an older Den Lite: the magnet isn't sent (it has none)", not find(cmds(n), "^gizmo magnet"))
press("M")
st.caps = CAPS
F.toClient("wubuild\tstate\t1/1\t" .. state())

-- ---------------------------------------------------------------- the magnet and the slope
n = #st.cmds
press("M")
check("M: the magnet on, said and kept", has(cmds(n), "gizmo magnet on") and K.magnet == true and WrathUnleashedUIDB.den.magnet == true)
n = #st.cmds
press("CTRL-L")
check("Ctrl+L: follow the slope on, said and kept", has(cmds(n), "gizmo slope on") and K.slope == true and WrathUnleashedUIDB.den.slope == true)
check("the hint line says both are on", K.Hint():find("magnet on (M)", 1, true) and K.Hint():find("on the slope (Ctrl+L)", 1, true), K.Hint())
press(D.keys.build or "F7"); F.tick(0.6)
check("play mode: none of the new keys bound", not bound("LeylineDenKeys").M and not bound("LeylineDenKeys").NUMPAD8 and not F.overrides["ALT-BUTTON1"])
n = #st.cmds
press(D.keys.build or "F7"); F.tick(0.6)
check("back in build mode: both said again (Den Lite forgets nothing, but a new Den Lite would)", has(cmds(n), "gizmo magnet on") and has(cmds(n), "gizmo slope on"))
-- the Tools window's chips
local T = W.BuildTools
if T and T.Show then T.Show("select") end
local sp = _G.WrathUnleashedToolsFrame and _G.WrathUnleashedToolsFrame.pages and _G.WrathUnleashedToolsFrame.pages.select
check("the Tools window: Magnet and On the slope lit", sp and sp.c.magnet and sp.c.slope and sp.c.magnet.active ~= false and sp.c.slope.active ~= false)
if sp and sp.c.magnet then sp.c.magnet:Fire("OnClick") end
check("its Magnet chip turns it off", K.magnet == false)
if sp and sp.c.magnet then sp.c.magnet:Fire("OnClick") end
if T and T.Hide then T.Hide() elseif _G.WrathUnleashedToolsFrame then _G.WrathUnleashedToolsFrame:Hide() end

-- ---------------------------------------------------------------- in hand: Alt is free, snapped shows
B.Choose({ id = 2413, name = "Wooden Chair" })
check("a card in hand", G.armed == "place")
G.lastSend = 0; mouse.x = mouse.x + 40
n = #st.cmds
G.Tick(true)
check("a hover without Alt: no free word", find(cmds(n), "^hover [%-%d%.]+ [%-%d%.]+ [%d%.]+ %d+$"), table.concat(cmds(n), "|"))
keys.alt = true
n = #st.cmds; mouse.x = mouse.x + 40
G.Tick(true)
check("Alt held: `hover ... free` (no grid, no magnet)", find(cmds(n), "^hover .* free$"), table.concat(cmds(n), "|"))
n = #st.cmds
G.Turn(1)
check("Alt+Q: 1 degree", has(cmds(n), "preview turn 1"), table.concat(cmds(n), "|"))
keys.alt = false
n = #st.cmds
G.Turn(1)
check("Q: the full step", has(cmds(n), "preview turn 15"))
F.toClient("wubuild\tsnapped\t1/1\t1")
check("snapped: the status line says so", tagText():find("Snapped edge to edge", 1, true), tagText())
F.toClient("wubuild\tsnapped\t1/1\t0")
check("unsnapped: gone", not tagText():find("Snapped", 1, true))
G.Cancel()
check("put away: the snap forgotten", not K.snapped)

-- ---------------------------------------------------------------- group drag and the hover tip
select(41, 3)
G.dragging = true; G.ShowTag()
check("dragging one of three: the status line says they move together", tagText():find("Moving 3 together", 1, true), tagText())
G.dragging = nil; G.ShowTag()
G.hovered = { id = 41, name = "Wooden Chair" }
G.TipUpdate()
local tip = _G.LeylineGizmoHoverTip
check("the hover tip: the name and what the mouse does (three selected: drag slides all 3)", tip and tip:IsShown() and tip.fs:GetText() == "Wooden Chair"
    and tip.hint and tip.hint:GetText():find("slide all 3", 1, true), tip and tip.hint and tip.hint:GetText())
select(41, 1)
G.TipUpdate()
check("one selected: the plain hint with the menu", tip.hint:GetText():find("right-click: menu", 1, true))
G.hovered = nil; G.TipUpdate()

-- ---------------------------------------------------------------- circle select
local circle = _G.LeylineDenKeyCircle
mouse.x, mouse.y = 600, 400
keys.alt = true
circle:Fire("OnClick", "LeftButton", true)
mouse.x, mouse.y = 700, 450
F.tick(0.05)
check("Alt+drag: the circle drawn as you drag", _G.LeylineDenSelectCircle:IsShown())
n = #st.cmds
circle:Fire("OnClick", "LeftButton", false)
keys.alt = false
check("let go: selcircle from the middle to the edge", find(cmds(n), "^selcircle [%-%d%.]+ [%-%d%.]+ [%-%d%.]+ [%-%d%.]+ [%d%.]+ %d+$") and not _G.LeylineDenSelectCircle:IsShown(),
    table.concat(cmds(n), "|"))
keys.alt, keys.shift = true, true
n = #st.cmds
circle:Fire("OnClick", "LeftButton", true); mouse.x = 500; circle:Fire("OnClick", "LeftButton", false)
keys.alt, keys.shift = false, false
check("Alt+Shift+drag adds", find(cmds(n), "^selcircle .* add$"))
n = #st.cmds
circle:Fire("OnClick", "LeftButton", true); circle:Fire("OnClick", "LeftButton", false)
check("a still Alt+click sends nothing", #cmds(n) == 0, table.concat(cmds(n), "|"))

-- ---------------------------------------------------------------- fill
select(41, 1)
n = #st.cmds
K.Fill("gap")
check("Fill gap with one selected: says to select two, sends nothing", #cmds(n) == 0 and K.lastNote:find("select two", 1, true))
select(41, 2)
K.Fill("gap")
check("two selected: fill gap", has(cmds(n), "fill gap"))
n = #st.cmds
K.Fill("box", 2)
check("fill box with a step", has(cmds(n), "fill box 2.00"))
if T and T.Show then T.Show("arrange") end
local ap = _G.WrathUnleashedToolsFrame and _G.WrathUnleashedToolsFrame.pages and _G.WrathUnleashedToolsFrame.pages.arrange
n = #st.cmds
if ap and ap.c.fillgap then ap.c.fillgap:Fire("OnClick") end
if ap and ap.c.fillbox then ap.c.fillbox:Fire("OnClick") end
check("the Tools window's Arrange: Fill gap and Fill box", has(cmds(n), "fill gap") and has(cmds(n), "fill box"), table.concat(cmds(n), "|"))
if _G.WrathUnleashedToolsFrame then _G.WrathUnleashedToolsFrame:Hide() end

-- ---------------------------------------------------------------- the help says it
local card5 = D.CARDS[5][2]
check("the keys card: nudge, Alt, circle, magnet, slope, the tour", card5:find("nudge", 1, true) and card5:find("Alt+drag circle", 1, true)
    and card5:find("M magnet", 1, true) and card5:find("Ctrl+L", 1, true) and card5:find("/dentour", 1, true))
local guideKeys
for _, p in ipairs(W.DenGuide and W.DenGuide.PAGES or {}) do if p[1] == "keys" then guideKeys = table.concat(p[5], " ") end end
check("the Guide's Keys page: the same", guideKeys and guideKeys:find("Numpad", 1, true) and guideKeys:find("magnet", 1, true) and guideKeys:find("/dentour", 1, true))
check("the hint line: Alt+drag circle and the nudge", K.Hint():find("Alt+drag circle", 1, true) and K.Hint():find("Numpad nudges", 1, true))

-- no game-brand words; owner copy marked
for _, file in ipairs({ "DenKeys.lua", "DenCoach.lua", "Gizmo.lua", "BuildTools.lua" }) do
    local f = io.open(dir .. "/" .. file); local src = f:read("*a"); f:close()
    check("no game-brand words in " .. file, not (src:find("World of Warcraft") or src:find("Warcraft") or src:find("WoW") or src:find("Lich King") or src:find("Blizzard")))
end
local f = io.open(dir .. "/DenCoach.lua"); local src = f:read("*a"); f:close()
check("the tour's words wait for the owner's voice pass (TODO(owner))", src:find("TODO(owner)", 1, true))
local toc = io.open(dir .. "/WrathUnleashedUI.toc"):read("*a")
check("DenCoach.lua in the .toc after DenKeys.lua", toc:find("DenKeys.lua\nDenCoach.lua", 1, true) or toc:find("DenKeys.lua\r\nDenCoach.lua", 1, true))

print(string.format("\n%d passed, %d failed", pass, fail))
if fail > 0 then os.exit(1) end
