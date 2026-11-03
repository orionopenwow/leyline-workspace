-- lua5.1 stream/tests/test_wuui_gmskin.lua [addon dir] [tools/den-ui dir]
-- The GM panel's Leyline skin (WrathUnleashedUI SkinGM.lua; owner, 3 Nov: "dramatically upgrade this UI too", his
-- shot docs/den-ui/azerothadmin-before.png). AzerothAdmin is third-party and never copied here: its window is built
-- from stand-ins of the same names, sizes and anchors (tools/den-ui/aa-stub.lua with azerothadmin-frames.lua, read
-- from its source) on the no-game fake API (tools/den-ui/fakeui.lua). Checked: nothing breaks without AzerothAdmin
-- or MiniGM; the skin applies at login (our panel, "LEYLINE GM" for the logo, pill tabs, chips, switches, slim
-- sliders, the search-style parameter field); every original button keeps its own OnClick and tooltip; the GM tab
-- is compact (680 wide) with every control inside the window and none overlapping; the chips' labels fit; the other
-- tabs get their width back; /aa skin off puts every part back as it was (anchors, sizes, alpha, text, fonts) and
-- /aa skin on brings the skin back; plain /aa still opens and closes the window; MiniGM's buttons become chips (its
-- close button stays); no game-brand names in anything the skin shows.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local uidir = arg[2] or "tools/den-ui"

local pass, fail = 0, 0
local function check(name, ok, extra)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name .. (extra and ("  " .. tostring(extra)) or "")) end
end

local ROWS = dofile(uidir .. "/azerothadmin-frames.lua")
local BRAND = { "world of warcraft", "warcraft", "lich king", "blizzard" }
local function branded(s)
    local l = tostring(s or ""):lower()
    for _, w in ipairs(BRAND) do if l:find(w, 1, true) then return w end end
    if l:find("%f[%a]wow%f[%A]") then return "wow" end
end

-- a fresh game: the fake API, (AzerothAdmin), (MiniGM), our addon, the login events
local F, A
local function session(opts)
    opts = opts or {}
    for _, k in ipairs({ "AzerothAdmin", "MiniGMFrame", "WrathUnleashedUI", "WrathUnleashedUIChar", "MAJOR_VERSION", "MINOR_VERSION" }) do _G[k] = nil end
    for k in pairs(_G) do if type(k) == "string" and (k:match("^ma_") or k:match("^MiniGM")) then _G[k] = nil end end
    WrathUnleashedUIDB = opts.db or {}
    F = dofile(uidir .. "/fakeui.lua")
    F.setup(1352, 1081)
    A = dofile(uidir .. "/aa-stub.lua")
    if opts.aa ~= false then A.build(ROWS) end
    if opts.mini then A.buildMini() end
    local ok, err = pcall(F.load, dir)
    check((opts.label or "session") .. ": the addon loads", ok, err)
    local ok2, err2 = pcall(function()
        F.fire("ADDON_LOADED", "WrathUnleashedUI")
        F.fire("PLAYER_ENTERING_WORLD")
        F.fire("PLAYER_LOGIN")
    end)
    check((opts.label or "session") .. ": login runs without errors", ok2, err2)
    return WrathUnleashedUI.SkinGM
end

local function rect(o) return F.rectOf(o) end
local function inside(r, box) return r and box and r.x >= box.x - 0.5 and r.y >= box.y - 0.5 and r.x + r.w <= box.x + box.w + 0.5 and r.y + r.h <= box.y + box.h + 0.5 end
local function overlap(a, b) return a.x < b.x + b.w - 0.5 and b.x < a.x + a.w - 0.5 and a.y < b.y + b.h - 0.5 and b.y < a.y + a.h - 0.5 end
local function names(kind, group)
    local out = {}
    for _, r in ipairs(A.rows) do
        if (not kind or r.kind == kind) and (not group or r.group == group) then out[#out + 1] = r.name end
    end
    return out
end
local function allNames()
    local out = {}
    for _, r in ipairs(A.rows) do
        out[#out + 1] = r.name
        for _, k in ipairs({ "_texture", "Text", "Low", "High", "Left", "Middle", "Right" }) do
            if _G[r.name .. k] and type(_G[r.name .. k]) == "table" then out[#out + 1] = r.name .. k end
        end
    end
    return out
end
local function visibleTexts()
    local out = {}
    for _, e in ipairs(F.layout()) do
        local t = e.f.kind == "FontString" and e.f.text
        if t and t ~= "" then out[#out + 1] = t end
    end
    F.endLayout()
    return out
end

-- ---------------------------------------------------------------- without AzerothAdmin or MiniGM
do
    local G = session({ aa = false, label = "no GM addons" })
    check("no GM addons: the skin is there and quiet", G and G.on ~= true or (G and G.Present() == false))
    check("no GM addons: Apply does nothing", G.Apply() == false)
    check("no GM addons: /gmskin off and on run", pcall(SlashCmdList.LEYLINEGMSKIN, "off") and pcall(SlashCmdList.LEYLINEGMSKIN, "on"))
    check("no GM addons: no /aa of ours", SlashCmdList.ACECONSOLE_AA == nil)
end

-- ---------------------------------------------------------------- AzerothAdmin with the skin
local G = session({ label = "AzerothAdmin" })
local before
do
    -- what the window was before the skin: taken from a second session with the skin off at login
    local G0 = session({ db = { gmSkin = false }, label = "skin off at login" })
    check("skin off at login: not applied", G0.on == false and ma_logoframe:GetAlpha() == 1)
    before = A.snapshot(allNames())
    G = session({ label = "AzerothAdmin" })
end

check("applied at login", G.on == true)
check("the big logo is gone", ma_logoframe:GetAlpha() == 0)
check("our header: LEYLINE GM", G.title and G.title:GetText():find("LEYLINE") and G.title:GetText():find("GM") and G.title:IsVisible())
check("the version line without the client's version number", not tostring(ma_revtext:GetText()):find("3%.3") and ma_revtext:GetText():find("AzerothAdmin"))
check("the header lines without colour codes", not ma_toptext:GetText():find("|c") and ma_toptext:GetText():find("Wren"))
check("FrameLib's grey fills are clear", ma_midframe_tex == nil and (function()
    for _, n in ipairs({ "ma_bgframe", "ma_topframe", "ma_midframe", "ma_leftframe", "ma_rightframe", "ma_menubgframe" }) do
        for _, r in ipairs({ _G[n]:GetRegions() }) do
            if r.kind == "Texture" and not r.lgmOwn and r:GetAlpha() ~= 0 then return false end
        end
    end
    return true
end)())
local panel = ma_bgframe.lgmPanel
check("our panel: fill, a soft shadow and the gold line", panel and panel.fill:IsVisible() and #panel.all >= 14 and panel.line.color[1] > 0.8 and panel.line.color[3] < 0.3)

-- the tabs: pills in a row inside the window, the GM tab's with the gold underline
do
    local prev, ok, okIn = nil, true, true
    local box = rect(ma_bgframe)
    for _, g in ipairs(A.CONTENT) do
        local b = _G["ma_tabbutton_" .. g]
        local r = rect(b)
        if not r or r.h ~= 22 or (prev and math.abs(r.x - (prev.x + prev.w + 4)) > 0.5) then ok = false end
        if not inside(r, box) then okIn = false end
        prev = r
    end
    check("tabs: 22 high, side by side", ok)
    check("tabs: inside the window, under the header", okIn and rect(ma_tabbutton_main).y + 22 <= box.y + box.h - G.HEAD + 0.5)
    check("tabs: the GM tab is the active one (gold underline)", ma_tabbutton_main.lgm.active and ma_tabbutton_main.lgm.under:IsVisible() and not ma_tabbutton_char.lgm.under:IsVisible())
end

-- the GM tab
check("GM tab: the window is compact (680 x " .. (G.HEAD + G.TABS + G.MID_GM + G.FOOT) .. ")", math.abs(rect(ma_bgframe).w - 680) < 0.5
    and math.abs(rect(ma_bgframe).h - (G.HEAD + G.TABS + G.MID_GM + G.FOOT)) < 0.5)
do
    local box = rect(ma_bgframe)
    local rects, bad, outside = {}, {}, {}
    for _, n in ipairs(names(nil, "main")) do
        local o = _G[n]
        if o.kind ~= "FontString" or n == "ma_gridnavigatortext" then
            local r = rect(o)
            if r then
                if not inside(r, box) then outside[#outside + 1] = n end
                if o.kind ~= "FontString" then rects[#rects + 1] = { n, r } end
            end
        end
    end
    for _, n in ipairs(names("button", "bg")) do local r = rect(_G[n]); if r then rects[#rects + 1] = { n, r }; if not inside(r, box) then outside[#outside + 1] = n end end end
    for _, n in ipairs(names("button", "tabbuttons")) do local r = rect(_G[n]); if r then rects[#rects + 1] = { n, r } end end
    for i = 1, #rects do
        for j = i + 1, #rects do
            local pair = rects[i][1] .. "/" .. rects[j][1]
            -- Clear sits inside the parameter field on purpose
            if overlap(rects[i][2], rects[j][2]) and pair ~= "ma_parameter/ma_gmclearbutton" and pair ~= "ma_gmclearbutton/ma_parameter" then bad[#bad + 1] = rects[i][1] .. "/" .. rects[j][1] end
        end
    end
    check("GM tab: every control inside the window", #outside == 0, table.concat(outside, " "))
    check("GM tab: no two controls overlap", #bad == 0, table.concat(bad, " "))
    local shown = 0
    for _, n in ipairs(names("button", "main")) do if _G[n]:IsVisible() then shown = shown + 1 end end
    check("GM tab: all " .. #names("button", "main") .. " of its buttons still there", shown == #names("button", "main"))
end
-- chips: our fill and edge, FrameLib's texture clear, labels that fit, 12 px text
do
    local fit, bad = true, {}
    for _, s in ipairs(G.SECTIONS) do
        for _, e in ipairs(s[3]) do
            local b = _G[e[1]]
            local fs = b:GetFontString()
            local w = F.textWidth(fs)
            if b:GetText() ~= e[2] or w > rect(b).w - 2 or F.fontSize(fs) < 12 then fit = false; bad[#bad + 1] = e[2] end
        end
    end
    for _, e in ipairs(G.LOOKUPS) do
        local b = _G[e[1]]
        if b:GetText() ~= e[2] or F.textWidth(b:GetFontString()) > rect(b).w - 2 then fit = false; bad[#bad + 1] = e[2] end
    end
    check("chips: their labels fit at 12 px", fit, table.concat(bad, ", "))
    check("chips: our fill and edge, FrameLib's texture clear", ma_castbutton.lgm.bg:IsVisible() and ma_castbutton_texture:GetAlpha() == 0 and #ma_castbutton.lgm.edge == 4)
    check("chips: 22 high in the sections", rect(ma_castbutton).h == 22 and rect(ma_itembutton).h == 22)
    -- hover and press
    ma_castbutton:Fire("OnEnter")
    local over = ma_castbutton.lgm.bg.color
    ma_castbutton:Fire("OnMouseDown")
    local down = ma_castbutton.lgm.bg.color
    ma_castbutton:Fire("OnMouseUp"); ma_castbutton:Fire("OnLeave")
    local rest = ma_castbutton.lgm.bg.color
    check("chips: hover, press and rest look different", over[1] ~= rest[1] and down[1] ~= over[1] and math.abs(down[1] - 0.878) < 0.01)
    check("chips: the tooltip still shows (AzerothAdmin's OnEnter)", (A.tips.ma_castbutton or 0) == 1)
    ma_goticketbutton = ma_goticketbutton
    local tb = ma_inforefreshbutton
    tb:Disable()
    local dis = tb.lgm.bg.color
    tb:Enable()
    check("chips: a disabled chip is dimmed", dis[1] < 0.07 and tb.lgm.bg.color[1] > 0.07)
end
-- switches
do
    local row = G.rows.gm
    check("switches: one row per On / Off pair (" .. #G.TOGGLES .. ") and insta-kill", row and row.track:IsVisible() and ma_instantkillbutton.lgmRow ~= nil)
    check("switches: no On / Off text", ma_gmonbutton:GetText() == "" and ma_gmoffbutton:GetText() == "")
    check("switches: the knob in the middle before anything is sent", row.knob.points[1].p == "CENTER")
    ma_gmonbutton:Click()
    check("switches: On sends its command and the knob goes right, gold", A.clicks.ma_gmonbutton == 1 and G.state.gm == "on" and row.knob.points[1].p == "RIGHT" and math.abs(row.knob.color[1] - 0.878) < 0.01)
    ma_gmoffbutton:Click()
    check("switches: Off sends its command and the knob goes left", A.clicks.ma_gmoffbutton == 1 and G.state.gm == "off" and row.knob.points[1].p == "LEFT")
    local r1, r2 = rect(ma_gmoffbutton), rect(ma_gmonbutton)
    check("switches: Off is the track's left half, On its right", r1 and r2 and r1.x < r2.x and math.abs(r1.x + r1.w - r2.x) < 0.5 and inside(r1, rect(row.track)) and inside(r2, rect(row.track)))
    local ik = ma_instantkillbutton.lgmRow
    ma_instantkillbutton:Click()
    check("switches: insta-kill (a CheckButton) switches on when clicked", ma_instantkillbutton:GetChecked() and ik.knob.points[1].p == "RIGHT" and A.clicks.ma_instantkillbutton == 1)
    ma_instantkillbutton:SetChecked(false)
    check("switches: and follows SetChecked", ik.knob.points[1].p == "LEFT")
    check("switches: the CheckButton's own art is clear", ma_instantkillbutton:GetNormalTexture():GetAlpha() == 0)
end
-- sliders and the parameter field
do
    local sl = ma_speedslider
    check("sliders: the template's backdrop and Low / High clear", sl.bdcolor[4] == 0 and ma_speedsliderLow:GetAlpha() == 0)
    check("sliders: a gold thumb 8 x 14", sl:GetThumbTexture().color and sl:GetThumbTexture().w == 8 and sl:GetThumbTexture().h == 14)
    sl:SetValue(5.05)
    check("sliders: the gold fill follows the value", math.abs(sl.lgm.fill.w - 140 * 0.5) < 1)
    check("sliders: Reset is a chip", ma_speedsliderreset_button:GetText() == "Reset" and ma_speedsliderreset_button.lgm.bg:IsVisible())
    check("parameter field: full width over the chips with Clear inside", math.abs(rect(ma_parameter).w - 448) < 0.5 and inside(rect(ma_gmclearbutton), rect(ma_parameter)))
    check("parameter field: the hint shows when empty, hides when typed in", G.hint:IsVisible() and ma_parametertext:GetAlpha() == 0)
    ma_parameter:SetText("Wren")
    local hidden = not G.hint:IsVisible()
    ma_parameter:SetText("")
    ma_parameter:Fire("OnEditFocusGained")
    check("parameter field: typing hides the hint, focus turns the edge gold", hidden and not G.hint:IsVisible() and math.abs(ma_parameter.lgm.edge[1].color[1] - 0.878) < 0.01)
    ma_parameter:Fire("OnEditFocusLost")
end

-- every original button keeps its OnClick (and its tooltip): each one's command runs once per click
do
    local missing = {}
    for _, n in ipairs(names("button")) do
        local b = _G[n]
        local was = A.clicks[n] or 0
        b:Click()
        if (A.clicks[n] or 0) ~= was + 1 then missing[#missing + 1] = n end
    end
    check("every one of AzerothAdmin's " .. #names("button") .. " buttons keeps its own click", #missing == 0, table.concat(missing, " "))
    local tips = {}
    for _, n in ipairs(names("button")) do
        local was = A.tips[n] or 0
        _G[n]:Fire("OnEnter"); _G[n]:Fire("OnLeave")
        if (A.tips[n] or 0) ~= was + 1 then tips[#tips + 1] = n end
    end
    check("... and its tooltip", #tips == 0, table.concat(tips, " "))
end

-- the other tabs: their own layout at 750, in the new look; back to the compact GM tab
AzerothAdmin:InstantGroupToggle("char")
check("other tabs: 750 wide (their layouts need it), the char tab active", math.abs(rect(ma_bgframe).w - 750) < 0.5 and ma_tabbutton_char.lgm.active and not ma_tabbutton_main.lgm.active)
check("other tabs: their chips skinned too", _G.ma_kickbutton == nil or ma_kickbutton.lgm ~= nil)
do
    local all = true
    for _, n in ipairs(names("button", "char")) do if not (_G[n].lgm or _G[n].lgmRow) then all = false end end
    check("other tabs: every char-tab button is a chip or a switch", all)
end
local topLeft = { rect(ma_bgframe).x, rect(ma_bgframe).y + rect(ma_bgframe).h }
AzerothAdmin:InstantGroupToggle("main")
check("back on the GM tab: 680 wide, grown from the same top left", math.abs(rect(ma_bgframe).w - 680) < 0.5
    and math.abs(rect(ma_bgframe).x - topLeft[1]) < 0.5 and math.abs(rect(ma_bgframe).y + rect(ma_bgframe).h - topLeft[2]) < 0.5)

-- no game-brand names, no client version number in what the skin shows
do
    local hits = {}
    for _, t in ipairs(visibleTexts()) do
        local w = branded(t)
        if w then hits[#hits + 1] = t end
        if t:find("3%.3%.5") then hits[#hits + 1] = t end
    end
    check("no game-brand names or client version on screen", #hits == 0, table.concat(hits, " | "))
    local src = io.open(dir .. "/SkinGM.lua"):read("*a")
    local shown = {}
    for s in src:gmatch('"([^"\n]*)"') do if branded(s) then shown[#shown + 1] = s end end
    check("no game-brand names in SkinGM.lua's strings", #shown == 0, table.concat(shown, " | "))
end

-- /aa skin off: every part back as AzerothAdmin made it; /aa still toggles the window; /aa skin on again
do
    SlashCmdList.ACECONSOLE_AA("skin off")
    check("/aa skin off: off and remembered", G.on == false and WrathUnleashedUIDB.gmSkin == false)
    local after = A.snapshot(allNames())
    local diff = {}
    for n, b in pairs(before) do
        local a = after[n]
        for _, k in ipairs({ "alpha", "text", "points", "font", "tcolor", "w", "h" }) do
            if a and tostring(a[k]) ~= tostring(b[k]) then diff[#diff + 1] = n .. "." .. k .. " " .. tostring(b[k]) .. " -> " .. tostring(a[k]) end
        end
    end
    check("/aa skin off: every part as before (anchors, sizes, alpha, text, fonts, colours)", #diff == 0, table.concat(diff, "; ", 1, math.min(#diff, 8)))
    local ours = 0
    for _, e in ipairs(F.layout()) do if e.f.lgmOwn then ours = ours + 1 end end
    F.endLayout()
    check("/aa skin off: nothing of ours left on screen", ours == 0, ours)
    local was = A.clicks.ma_gmonbutton
    ma_gmonbutton:Click()
    check("/aa skin off: the buttons still work", A.clicks.ma_gmonbutton == was + 1)
    SlashCmdList.ACECONSOLE_AA("")
    check("/aa still closes the window", not ma_bgframe:IsShown())
    SlashCmdList.ACECONSOLE_AA("")
    check("/aa still opens it", ma_bgframe:IsShown())
    SlashCmdList.ACECONSOLE_AA("skin on")
    check("/aa skin on: back", G.on == true and WrathUnleashedUIDB.gmSkin == true and ma_logoframe:GetAlpha() == 0 and math.abs(rect(ma_bgframe).w - 680) < 0.5)
    check("/aa skin on: the sections back too", G.deco:IsVisible() and rect(ma_castbutton).h == 22 and ma_castbutton:GetText() == "Cast")
    SlashCmdList.LEYLINEGMSKIN("off")
    check("/gmskin off works too", G.on == false)
    SlashCmdList.LEYLINEGMSKIN("on")
    -- the window shown again later (its OnShow) keeps the look
    SlashCmdList.ACECONSOLE_AA(""); SlashCmdList.ACECONSOLE_AA("")
    check("shown again: still skinned", G.on and ma_logoframe:GetAlpha() == 0)
end

-- MiniGM alongside: its panel buttons become chips, its close button and header art as they should be
do
    G = session({ mini = true, label = "AzerothAdmin + MiniGM" })
    MiniGMFrame:Show()
    local b = A.miniButtons[1]
    check("MiniGM: its buttons are chips", b.lgm and b.lgm.bg:IsVisible() and b:GetNormalTexture():GetAlpha() == 0)
    check("MiniGM: the close button keeps its art", A.miniClose.lgm == nil and A.miniClose:GetNormalTexture():GetAlpha() == 1)
    check("MiniGM: our panel, the dialog header art clear", MiniGMFrame.lgmPanel and A.miniHeader:GetAlpha() == 0 and MiniGMFrame.bdcolor[4] == 0)
    check("MiniGM: the field restyled", A.miniBox.lgm and _G.MiniGMStubBoxLeft:GetAlpha() == 0)
    b:Click()
    check("MiniGM: a button still runs its command", A.miniClicks["Fly on"] == 1)
    G.SetEnabled(false)
    check("MiniGM: skin off restores", b:GetNormalTexture():GetAlpha() == 1 and A.miniHeader:GetAlpha() == 1 and MiniGMFrame.bdcolor[4] == 1)
end
do
    G = session({ aa = false, mini = true, label = "MiniGM alone" })
    check("MiniGM alone: skinned", G.on and A.miniButtons[2].lgm ~= nil)
end

print(string.format("\n%d/%d passed", pass, pass + fail))
if fail > 0 then os.exit(1) end
