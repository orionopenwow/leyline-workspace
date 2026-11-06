-- texlua stream/tests/test_wuui_denkeys.lua [addon dir] [tools/den-ui dir]
-- Your Den's editor keys, Noggit's way (DenKeys.lua with Gizmo.lua, Builder.lua, DenBar.lua; Den Lite labs/den-lite
-- src/noggit.rs). Owner 5 Nov: "Continue to work on the den until everything functions especially the moving the
-- objects, distinctly being in play vs build mode, camera working, move objects working and every other convenient
-- and noggit feature possible". The whole addon in .toc order on tools/den-ui/fakeui.lua (override bindings kept per
-- owner, as the game does), with a fake Den Lite answering as ovmod.rs / gizmo.rs / noggit.rs:
--   * play vs build: build mode binds the editor's keys; F7 to play mode leaves no editor binding of any owner (only
--     the Den bar's own F7 / F8), shows the Play banner and hides the editor; back again binds them again; leaving the
--     Den gives every key back; a change asked for in combat waits for its end;
--   * Esc: cancel, then deselect, then the game's own;
--   * the camera: the right mouse + wheel sets the flight's speed (the `cam fly ... v<speed>` word, the status
--     line's readout), F frames, Home comes back, Ctrl+1..3 keep a view and Shift+1..3 go back (kept in the saved
--     settings);
--   * the editor's keys: 1 / 2 / 3 tools (the wheel turns or sizes), a Turn drag (one commit) and a Size drag (one
--     grow), Ctrl+Z / Y, Ctrl+C / V (in hand with its turn, then its size), Ctrl+D by the last offset, Del, Ctrl+A,
--     Ctrl+drag box select, G / Shift+G grid, R turn step, Shift+R scatter, L / Shift+L slope / upright;
--   * the hint line says the tool's keys; no game-brand words.
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

-- the Den Lite: its state (the selection's id, entry, place, turn, size) and every command kept
local CAPS = "caps=spawn,near,move,turn,delete,undo,redo,copy,paste,dup,selall,selclear,grid,seladd,grow,setscale,upright,ground,"
    .. "preview,hover,commit,axis,grab,cam,gizmo,mode,click,drag,tool,dupby,selnear,slope,camframe,cammark,camto,flyspeed;"
local st = { sel = nil, x = 10, y = 20, z = 5, yaw = 90, scale = 1.5, clip = 0, cmds = {} }
local function state()
    local s = string.format("mine=1/800;undo=1;redo=0;den=owner;room=1/800;yard=0/0;core=1;lite=1;grid=0;clip=%d;area=den;code=-;", st.clip) .. CAPS
    if st.sel then
        s = s .. string.format("sel=%d,1670,Barrel of Powder;n=1;pos=%.2f,%.2f,%.2f,%.0f,%.2f;rot=0,0,0;ids=%d;gids=%d;", st.sel, st.x, st.y, st.z, st.yaw, st.scale, st.sel, st.sel)
    else
        s = s .. "sel=-;n=0;ids=;gids=;"
    end
    return s
end
local CATALOG = "2413,woodenchair,seating,Wooden Chair,1,World\\Generic\\Chair01.m2,0.50x0.50x1.10x0.00;1670,barrelofpowder,barrels,Barrel of Powder,1,-;"
F.server = function(msg)
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod == "wuden" and name == "hello" then F.toClient("wuden\tstatus\t1/1\trole=owner;props=1;saved=5")
    elseif mod == "wubuild" and name == "catalog" then F.toClient("wubuild\tcatalog\t1/1\t" .. CATALOG); F.toClient("wubuild\tstate\t1/1\t" .. state())
    elseif mod == "wubuild" and name == "state" then F.toClient("wubuild\tstate\t1/1\t" .. state())
    elseif mod == "wubuild" and name == "cmd" then
        st.cmds[#st.cmds + 1] = data
        if data == "selclear" then st.sel = nil; F.toClient("wubuild\tstate\t1/1\t" .. state()) end
        if data == "copy" then st.clip = 1 end
        -- noggit.rs dupby: the copy is made by the offset and selected
        local dx, dy, dz = data:match("^dupby ([%-%d%.]+) ([%-%d%.]+) ([%-%d%.]+)$")
        if dx and st.sel then st.sel = st.sel + 20; st.x, st.y, st.z = st.x + tonumber(dx), st.y + tonumber(dy), st.z + tonumber(dz) end
        if data:match("^preview %d") then F.toClient("wubuild\taim\t1/1\tplace," .. data:match("^preview (%d+)") .. ",1543") end
        if data == "preview move" then F.toClient("wubuild\taim\t1/1\tmove," .. tostring(st.sel) .. ",1543") end
        if data == "preview off" or data == "commit" then F.toClient("wubuild\taim\t1/1\toff") end
        if data == "cam mark" then F.toClient("wubuild\tcammark\t1/1\t16230.50,16257.00,13.20,90,35.0") end
        if data:match("^cam frame") or data:match("^cam to ") then F.toClient("wubuild\tcam\t1/1\tfar,30.0,0,90") end
        if data == "cam off" then F.toClient("wubuild\tcam\t1/1\tnear") end
        if not data:match("^hover") and not data:match("^preview") and not data:match("^cam") and data ~= "commit" and data ~= "selclear" then
            F.toClient("wubuild\tstate\t1/1\t" .. state())
        end
    end
end
F.load(dir)

-- the mouse and the keyboard, held as the test says
local mouse = { x = 960, y = 540, right = false, left = false }
local keys = { shift = false, ctrl = false, alt = false }
function GetCursorPosition() return mouse.x, mouse.y end
function IsMouseButtonDown(b) if b == "RightButton" then return mouse.right end; if b == "LeftButton" then return mouse.left end; return false end
function IsShiftKeyDown() return keys.shift end
function IsControlKeyDown() return keys.ctrl end
function IsAltKeyDown() return keys.alt end
local combat = false
function InCombatLockdown() return combat end
local cursorSet = {}
function SetCursor(c) cursorSet[#cursorSet + 1] = c end
function ResetCursor() cursorSet[#cursorSet + 1] = "reset" end

F.fire("PLAYER_ENTERING_WORLD")
local W = WrathUnleashedUI
local G, K, D, B = W.Gizmo, W.DenKeys, W.DenBar, W.Builder
check("DenKeys.lua loaded after Gizmo.lua", K and G and K.Bind and G.Escape)
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
local function count(t) local n = 0; for _ in pairs(t) do n = n + 1 end; return n end
local function tagText() local t = _G.LeylineGizmoTag; return t and t.fs and t.fs.GetText and (t.fs:GetText() or "") or "" end

-- ---------------------------------------------------------------- build mode: the editor's keys
check("in the Den on Den Lite: build mode", G.build == true, tostring(G.build))
local k = bound("LeylineDenKeys")
check("build mode binds the editor's keys (undo, redo, the tools, marks, grid, turn step, select near, box)",
    k["CTRL-Z"] and k["CTRL-Y"] and k["CTRL-SHIFT-Z"] and k["1"] and k["2"] and k["3"] and k["CTRL-1"] and k["SHIFT-3"]
    and k.R and k["SHIFT-R"] and k.G and k["SHIFT-G"] and k["CTRL-A"] and k["CTRL-BUTTON1"] == "LeylineDenKeyBox")
check("nothing selected: no Delete, copy, duplicate, frame or slope; no Esc of ours (the game's menu)",
    not (k.DELETE or k["CTRL-C"] or k["CTRL-D"] or k.F or k.L) and not bound("LeylineGizmo").ESCAPE)
check("never a movement key of DenKeys.lua's", not (k.W or k.A or k.S or k.D or k.Q or k.E or k.SPACE or k.X))
check("the hint line: the Move tool's keys", tagText():find("Move (1)", 1, true) and tagText():find("Ctrl+drag box", 1, true), tagText())

-- a selection
st.sel = 41
F.toClient("wubuild\tstate\t1/1\t" .. state())
k = bound("LeylineDenKeys")
check("a selection: Del, Ctrl+C, Ctrl+D, F, L, Shift+L; Esc ours (it deselects next)",
    k.DELETE and k["CTRL-C"] and k["CTRL-D"] and k.F and k.L and k["SHIFT-L"] and bound("LeylineGizmo").ESCAPE == "LeylineGizmoCancel")
check("the selection read from the state (id, entry, place, turn, size)", K.sel and K.sel.id == 41 and K.sel.entry == 1670 and K.sel.x == 10 and K.sel.yaw == 90 and K.sel.scale == 1.5)

-- ---------------------------------------------------------------- Esc: cancel, deselect, the game's
local n = #st.cmds
B.Choose({ id = 2413, name = "Wooden Chair" })
check("a card in hand", G.armed == "place" and find(cmds(n), "^preview 2413"))
n = #st.cmds
press("ESCAPE")
check("Esc 1: the card put away, the selection kept", has(cmds(n), "preview off") and G.armed == nil and st.sel == 41)
n = #st.cmds
press("ESCAPE")
check("Esc 2: the selection let go", has(cmds(n), "selclear") and G.sel == nil)
check("Esc 3: not ours (the game's own menu: nothing that changes the Den)", not bound("LeylineGizmo").ESCAPE and not F.overrides.ESCAPE)

-- ---------------------------------------------------------------- play mode leaves nothing bound
st.sel = 41; F.toClient("wubuild\tstate\t1/1\t" .. state())
if B.ShowProps then B.ShowProps(true) end
F.tick(0.6)
local function editorOwners()
    local out = {}
    for owner, keys2 in pairs(F.byOwner) do
        if owner ~= "LeylineDenBarKeys" and count(keys2) > 0 then out[#out + 1] = owner .. ":" .. count(keys2) end
    end
    table.sort(out)
    return table.concat(out, ",")
end
check("build mode with a selection: the Gizmo's, the editor's and the selection panel's keys", count(bound("LeylineGizmo")) > 0 and count(bound("LeylineDenKeys")) > 0, editorOwners())
press(D.keys.build or "F7")
F.tick(0.6)
check("F7: play mode", D.play == true and G.build == false)
local left = editorOwners()
check("play mode: no editor binding of any owner (only the Den bar's F7 / F8 and Teleports' own)", left == "" or not left:find("Gizmo") and not left:find("DenKeys") and not left:find("PropKeys") and not left:find("ArmKeys") and not left:find("Tools"), left)
check("play mode: F7 still there to come back", bound("LeylineDenBarKeys")[D.keys.build or "F7"] ~= nil)
check("play mode: the Play banner", _G.LeylineDenModeBanner and _G.LeylineDenModeBanner:IsShown() and _G.LeylineDenModeBanner.label:GetText() == "Play mode")
check("play mode: the editor's frames hidden (the bar, the rail, the gold edge, the gallery)", not _G.LeylineDenBar:IsShown() and not _G.LeylineDenRail:IsShown()
    and not _G.LeylineDenBuildEdge:IsShown() and not (_G.WrathUnleashedBuilderFrame and _G.WrathUnleashedBuilderFrame:IsShown()))
check("play mode: the strafe keys are the game's again (A / D turn)", not F.overrides.A and not F.overrides.D)
press(D.keys.build or "F7")
F.tick(0.6)
check("F7 again: build mode, its keys and the Build banner", G.build == true and count(bound("LeylineDenKeys")) > 0 and _G.LeylineDenModeBanner.label:GetText() == "Build mode")
check("build mode: the gold edge, one only (the Gizmo's own word and edges hidden in a Den)", _G.LeylineDenBuildEdge:IsShown()
    and not _G.LeylineGizmoBuildFrame.word:IsShown() and (function()
        for _, t in ipairs(_G.LeylineGizmoBuildFrame.edges or {}) do if t:IsShown() then return false end end
        return true
    end)())

-- in combat the game refuses: the change waits for its end
combat = true
press(D.keys.build or "F7")
F.tick(0.6)
check("in combat: play mode, the keys wait (the game would refuse them)", D.play == true and G.bindPending == true)
combat = false
F.fire("PLAYER_REGEN_ENABLED")
check("combat over: the editor's keys given back", count(bound("LeylineGizmo")) == 0 and count(bound("LeylineDenKeys")) == 0, editorOwners())
press(D.keys.build or "F7"); F.tick(0.6)

-- leaving the Den
D.SetActive(false)
F.tick(0.6)
check("leaving the Den: every editor key given back, the Den bar's too", count(bound("LeylineGizmo")) == 0 and count(bound("LeylineDenKeys")) == 0
    and count(bound("LeylineDenBarKeys")) == 0 and count(bound("LeylinePropKeys")) == 0, editorOwners())
D.SetActive(true); F.tick(0.6); F.tick(0.6)
check("back in the Den: build mode again", G.build == true)

-- ---------------------------------------------------------------- the camera
st.sel = 41; F.toClient("wubuild\tstate\t1/1\t" .. state())
n = #st.cmds
press("F")
check("F frames the selection (the far view round it)", has(cmds(n), "cam frame") and G.far ~= nil)
check("in the far view: Home and Numpad 0 come back", bound("LeylineDenKeys").HOME and bound("LeylineDenKeys").NUMPAD0)
check("the status line: the flight's speed", tagText():find("fly 16 yd/s", 1, true), tagText())
mouse.right = true
G.Wheel(1); G.Wheel(1)
mouse.right = false
check("right mouse + wheel: faster (16, 24, 32), kept", K.Speed() == 32 and WrathUnleashedUIDB.den.flySpeed == 6)
check("the readout follows", tagText():find("fly 32 yd/s", 1, true), tagText())
n = #st.cmds
G.PanKey("W", true)
check("the flight at that speed (`cam fly ... v32.0`)", find(cmds(n), "^cam fly 1 0 0 [%-%d%.]+ v32%.0$"), table.concat(cmds(n), "|"))
keys.shift = true; n = #st.cmds; G.PanKey("W", true); keys.shift = false
check("Shift: three times as fast", find(cmds(n), " v96%.0$"))
keys.alt = true; n = #st.cmds; G.PanKey("W", true); keys.alt = false
check("Alt: a third", find(cmds(n), " v10%.7$"))
G.PanKey("W", false)
n = #st.cmds
press("HOME")
check("Home: the view back to your character", has(cmds(n), "cam off") and G.far == nil)
mouse.right = true; for _ = 1, 20 do G.Wheel(-1) end; mouse.right = false
check("the slowest is 2 yards a second", K.Speed() == 2)
mouse.right = true; G.Wheel(1); G.Wheel(1); mouse.right = false
n = #st.cmds
G.Wheel(1)
check("without the right mouse the wheel zooms, as always", find(cmds(n), "^cam zoom") or find(cmds(n), "^cam far"), table.concat(cmds(n), "|"))

-- the view marks
n = #st.cmds
press("CTRL-2")
check("Ctrl+2: the view kept (Den Lite says where it is)", has(cmds(n), "cam mark") and WrathUnleashedUIDB.den.camMarks and WrathUnleashedUIDB.den.camMarks[2] == "16230.50,16257.00,13.20,90,35.0")
n = #st.cmds
press("SHIFT-2")
check("Shift+2: back to it", has(cmds(n), "cam to 16230.50 16257.00 13.20 90 35.0"))
n = #st.cmds
press("SHIFT-3")
check("Shift+3 with nothing kept: nothing sent, a note says how", #cmds(n) == 0 and (K.lastNote or ""):find("Ctrl%+3"))
press("HOME")

-- ---------------------------------------------------------------- the tools
n = #st.cmds
press("2")
check("2: the Turn tool", K.Tool() == "rotate" and B.mode == "rotate" and tagText():find("Turn (2)", 1, true), tagText())
G.Wheel(1)
check("Turn: the wheel turns the selection by the turn step", has(cmds(n), "turn 15"), table.concat(cmds(n), "|"))
press("3")
n = #st.cmds
G.Wheel(-1)
check("3: the Size tool; the wheel sizes", K.Tool() == "scale" and find(cmds(n), "^grow 0%.909$"), table.concat(cmds(n), "|"))
keys.ctrl = true; n = #st.cmds; G.Wheel(1); keys.ctrl = false
check("Ctrl+wheel zooms whatever the tool", not find(cmds(n), "^grow") and (find(cmds(n), "^cam zoom") or find(cmds(n), "^cam far")))

-- a Turn drag: pressed on a prop, dragged right: it turns live, one commit
press("2")
F.toClient("wubuild\thovered\t1/1\t41 Barrel of Powder")
mouse.x, mouse.y = 900, 500
mouse.left = true
G.MouseDown("LeftButton")
mouse.x = 945
n = #st.cmds
G.DragCheck()
F.tick(0.05)
local c = cmds(n)
check("Turn drag: the prop selected and picked up in place", find(c, "^click [%-%d%.]+ [%-%d%.]+ [%d%.]+ %d+$") and has(c, "preview move") and K.drag and K.drag.kind == "rotate")
check("Turn drag: two steps right: turned 30 degrees clockwise, live", has(c, "preview turn -30"), table.concat(c, "|"))
check("Turn drag: no cursor place sent (it stays where it stood)", not find(c, "^hover"))
check("the hint says how", tagText():find("Turning", 1, true), tagText())
n = #st.cmds
mouse.left = false
G.MouseUp("LeftButton")
check("let go: one commit", has(cmds(n), "commit") and K.drag == nil)

-- Esc during a Turn drag puts it back
F.toClient("wubuild\thovered\t1/1\t41 Barrel of Powder")
mouse.left = true
G.MouseDown("LeftButton"); mouse.x = 980; G.DragCheck(); F.tick(0.05)
n = #st.cmds
press("ESCAPE")
check("Esc during a Turn drag: put back", has(cmds(n), "preview off") and K.drag == nil)
mouse.left = false
G.MouseUp("LeftButton")

-- a Size drag: up 20 px
press("3")
F.toClient("wubuild\thovered\t1/1\t41 Barrel of Powder")
mouse.x, mouse.y = 900, 500
mouse.left = true
G.MouseDown("LeftButton")
mouse.y = 520
G.DragCheck(); F.tick(0.05)
n = #st.cmds
mouse.left = false
G.MouseUp("LeftButton")
check("Size drag: one grow on the let-go (1.01 a pixel)", find(cmds(n), "^grow 1%.22%d$"), table.concat(cmds(n), "|"))
press("1")
check("1: back to Move", K.Tool() == "move")
F.toClient("wubuild\thovered\t1/1\t-")

-- ---------------------------------------------------------------- the editor's keys
st.sel = 41; F.toClient("wubuild\tstate\t1/1\t" .. state())
n = #st.cmds; press("CTRL-Z"); press("CTRL-Y"); press("CTRL-SHIFT-Z")
check("Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z: undo, redo, redo", table.concat(cmds(n), "|") == "undo|redo|redo", table.concat(cmds(n), "|"))
n = #st.cmds; press("DELETE")
check("Del deletes", has(cmds(n), "delete"))
st.sel = 41; F.toClient("wubuild\tstate\t1/1\t" .. state())
n = #st.cmds; press("CTRL-C")
check("Ctrl+C: copied (its entry, turn and size kept for the paste)", has(cmds(n), "copy") and K.clip and K.clip.entry == 1670 and K.clip.yaw == 90 and K.clip.scale == 1.5)
check("then Ctrl+V is bound", bound("LeylineDenKeys")["CTRL-V"])
n = #st.cmds; press("CTRL-V")
c = cmds(n)
check("Ctrl+V: in hand under the cursor with its turn", c[1] == "yaw set 90" and find(c, "^preview 1670") and G.armed == "place", table.concat(c, "|"))
n = #st.cmds
F.toClient("wubuild\tplaced\t1/1\t60")
check("put down: its size too", has(cmds(n), "setscale 1.50"), table.concat(cmds(n), "|"))
F.toClient("wubuild\taim\t1/1\toff")

-- Ctrl+D: a yard along, then by the offset the copy was moved to
st.sel, st.x, st.y, st.z = 41, 10, 20, 5
F.toClient("wubuild\tstate\t1/1\t" .. state())
n = #st.cmds; press("CTRL-D")
check("Ctrl+D the first time: a yard along x (the copy selected)", has(cmds(n), "dupby 1.00 0.00 0.00") and st.sel == 61 and K.sel.x == 11, table.concat(cmds(n), "|"))
st.x = 13          -- the copy (made at 11) moved on to 13
F.toClient("wubuild\tstate\t1/1\t" .. state())
n = #st.cmds; press("CTRL-D")
check("Ctrl+D again: the offset the copy was moved to (3 yards)", has(cmds(n), "dupby 3.00 0.00 0.00") and st.x == 16, table.concat(cmds(n), "|"))
n = #st.cmds; press("CTRL-D")
check("and on along the row", has(cmds(n), "dupby 3.00 0.00 0.00"))
st.sel, st.x = 41, 10; F.toClient("wubuild\tstate\t1/1\t" .. state())
n = #st.cmds; press("CTRL-D")
check("a different prop selected: the last offset from it", has(cmds(n), "dupby 3.00 0.00 0.00"))

n = #st.cmds; press("CTRL-A")
check("Ctrl+A: every prop within 30 yards", has(cmds(n), "selnear 30"))
n = #st.cmds; press("L"); press("SHIFT-L")
check("L lies it on the slope, Shift+L stands it upright", table.concat(cmds(n), "|") == "slope|upright", table.concat(cmds(n), "|"))

-- the grid and the turn step
local was = B.Snap()
press("G")
local g1 = B.Snap()
press("G")
check("G: snap on (a yard), then off", g1 == 1 and B.Snap() == 0, tostring(was) .. " " .. tostring(g1) .. " " .. tostring(B.Snap()))
press("SHIFT-G"); local s1 = B.Snap(); press("SHIFT-G"); local s2 = B.Snap()
check("Shift+G: the grid's size round 0.5, 1, 2, 5", s1 == 0.5 and s2 == 1, tostring(s1) .. " " .. tostring(s2))
press("G")
local a0 = B.TurnStep(); press("R"); local a1 = B.TurnStep(); press("R"); local a2 = B.TurnStep()
check("R: the turn step round 15, 45, 90", a0 == 15 and a1 == 45 and a2 == 90, a0 .. " " .. a1 .. " " .. a2)
press("R")

-- scatter
press("SHIFT-R")
check("Shift+R: scatter on (kept)", K.scatter == true and WrathUnleashedUIDB.den.scatter == true and tagText():find("scatter on", 1, true))
n = #st.cmds
B.Choose({ id = 2413, name = "Wooden Chair" })
c = cmds(n)
check("scatter: a card in hand starts at a random turn", find(c, "^preview turn %d+$"), table.concat(c, "|"))
n = #st.cmds
F.toClient("wubuild\tplaced\t1/1\t70")
check("scatter: each prop placed gets a random size (0.85 to 1.15)", (function()
    local v = tonumber((find(cmds(n), "^grow ") or ""):match("grow ([%d%.]+)") or "")
    return v and v >= 0.85 and v <= 1.15
end)(), table.concat(cmds(n), "|"))
G.Cancel()
press("SHIFT-R")
check("Shift+R again: scatter off", K.scatter == false)

-- box select: Ctrl+drag
local box = _G.LeylineDenKeyBox
mouse.x, mouse.y = 400, 300
keys.ctrl = true
box:Fire("OnClick", "LeftButton", true)
mouse.x, mouse.y = 700, 600
F.tick(0.05)
check("Ctrl+drag: the box drawn as you drag", _G.LeylineDenSelectBox:IsShown())
n = #st.cmds
box:Fire("OnClick", "LeftButton", false)
keys.ctrl = false
c = cmds(n)
check("let go: the props inside selected (the tools' box: two corners on the screen)", find(c, "^tool box [%-%d%.]+ [%-%d%.]+ [%-%d%.]+ [%-%d%.]+ [%d%.]+ %d+$") and not _G.LeylineDenSelectBox:IsShown(), table.concat(c, "|"))
keys.ctrl, keys.shift = true, true
box:Fire("OnClick", "LeftButton", true); mouse.x = 300; box:Fire("OnClick", "LeftButton", false)
keys.ctrl, keys.shift = false, false
check("Ctrl+Shift+drag adds", find(cmds(n), " add$"))
n = #st.cmds
box:Fire("OnClick", "LeftButton", true); box:Fire("OnClick", "LeftButton", false)
check("a still Ctrl+click adds the prop there", find(cmds(n), "^click .* add$"))

-- the cursor: the editor's while something is in hand
cursorSet = {}
B.Choose({ id = 2413, name = "Wooden Chair" })
F.tick(0.05)
check("in hand over the world: the placing cursor", cursorSet[#cursorSet] == K.CURSOR_PLACE, tostring(cursorSet[#cursorSet]))
G.Cancel(); F.tick(0.05)
check("put away: the game's cursor again", cursorSet[#cursorSet] == "reset", tostring(cursorSet[#cursorSet]))

-- the Builder's own keys (the selection panel's) do the same in build mode
st.sel = 41; F.toClient("wubuild\tstate\t1/1\t" .. state())
n = #st.cmds
B.KEYS["CTRL-D"]()
check("the panel's Ctrl+D: the same duplicate", find(cmds(n), "^dupby "))
B.Choose({ id = 2413, name = "Wooden Chair" })
n = #st.cmds
B.KEYS.ESCAPE()
check("the panel's Esc: the same ladder (cancel first, the selection kept)", has(cmds(n), "preview off") and not has(cmds(n), "selclear") and st.sel ~= nil)

-- no game-brand words
local f = io.open(dir .. "/DenKeys.lua"); local src = f:read("*a"); f:close()
check("no game-brand words in DenKeys.lua", not (src:find("World of Warcraft") or src:find("Warcraft") or src:find("WoW") or src:find("Lich King") or src:find("Blizzard")))
check("the hint and the help card say the keys", K.Hint():find("1 2 3 tools", 1, true) and D.CARDS[5][2]:find("Ctrl+D duplicate", 1, true) and D.CARDS[5][2]:find("Right mouse + wheel: fly speed", 1, true))

print(string.format("\n%d passed, %d failed", pass, fail))
if fail > 0 then os.exit(1) end
