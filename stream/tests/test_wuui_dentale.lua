-- lua5.1 stream/tests/test_wuui_dentale.lua [addon dir]   (texlua runs it too)
-- The Den tale tracker (DenTale.lua; docs/DEN-NEXT.md): Den Lite's `wuden tale` lines (labs/den-lite taleplay.rs) show
-- the step under way, its hint, a reward, the end; Jaina's draft shows Play it / Not now, which send `tale yes` /
-- `tale no`; a code in parts is put back together for Ctrl+C; nothing from the server is run, and a line from
-- anyone but the player is ignored.
local dir = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI"
local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end

local Frame = {}
Frame.__index = function(self, k)
    local v = rawget(Frame, k)
    if v ~= nil then return v end
    if type(k) == "string" and k:match("^%u") then return function() end end
    return nil
end
function Frame:SetScript(n, fn) self.scripts[n] = fn end
function Frame:Show() self.shown = true end
function Frame:Hide() self.shown = false end
function Frame:IsShown() return self.shown end
function Frame:SetText(t) self.text = t end
function Frame:GetText() return self.text end
function Frame:CreateFontString() return setmetatable({ scripts = {} }, Frame) end
local made = {}
function CreateFrame(kind, name, parent, template)
    local f = setmetatable({ kind = kind, scripts = {}, shown = true, parent = parent }, Frame)
    made[#made + 1] = f
    return f
end
UIParent = setmetatable({ scripts = {} }, Frame)
SlashCmdList = {}
function UnitName() return "Wren" end
local sent = {}
function SendAddonMessage(prefix, text, channel, to) sent[#sent + 1] = { prefix, text, channel, to } end
WrathUnleashedUI = {}

local chunk = assert(loadfile(dir .. "/DenTale.lua"))
chunk()
local T = WrathUnleashedUI.DenTale
check("loads and is WrathUnleashedUI.DenTale", T ~= nil)
local listener = made[1]
local function msg(text, who) listener.scripts.OnEvent(listener, "CHAT_MSG_ADDON", "OVMOD", text, "WHISPER", who or "Wren") end

check("nothing to show at first", T.View() == nil)
msg("wuden\ttale\t1/1\tstep|1|4|Say hello|talk to Jaina")
local title, line, kind = T.View()
check("the step under way", title == "Tale, step 1 of 4" and line == "Say hello: talk to Jaina" and kind == "step")
msg("wuden\ttale\t1/1\thint|Target a dummy and attack.")
check("its hint", T.hint == "Target a dummy and attack.")
msg("wuden\ttale\t1/1\tstep|2|4|Spar|defeat 3 training dummies (1/3)")
check("the next step clears the hint", T.hint == nil and select(2, T.View()):find("(1/3)", 1, true) ~= nil)
-- someone else's line, a broken line, colour codes and a pipe: ignored or shown plain
msg("wuden\ttale\t1/1\tstep|9|9|Hacked|x", "Mallory")
check("a line from someone else is ignored", T.step.n == 2)
msg("wuden\ttale\t1/1\tstep|x|y")
check("a broken step line is ignored", T.step.n == 2)
msg("wuden\ttale\t1/1\treward||cffff0000Sky Walker|r")
check("a reward is shown plain", T.reward == "Sky Walker")
msg("wuden\ttale\t1/1\tdone")
check("the end", select(3, T.View()) == "done" and T.step == nil)
msg("wuden\ttale\t1/1\toff")
check("off: nothing", T.View() == nil)

-- Jaina's draft: Play it / Not now send the Den's own commands
msg("wuden\ttale\t1/1\tdraft|The Lost Lantern|1. Hear Jaina out / 2. Find the lantern")
local t2, l2, k2 = T.View()
check("the draft", k2 == "draft" and t2 == "Jaina's tale: The Lost Lantern" and l2:find("Find the lantern", 1, true))
local panel = _G.LeylineDenTale or made[2]
local yes
for _, f in ipairs(made) do if f.label == "Play it" then yes = f end end
check("a Play it button", yes ~= nil and yes.shown)
yes.scripts.OnClick(yes)
check("Play it sends tale yes", sent[#sent] and sent[#sent][2] == "wuden\tcmd\t1/1\ttale yes" and sent[#sent][4] == "Wren")
msg("wuden\ttale\t1/1\tdraft-off")
check("no draft any more", T.draft == nil)

-- a code in three parts, then for Ctrl+C
msg("wuden\tpack\t1/3\tleyline://pack/ley1.den.z.AAA")
check("part one alone shows nothing", T.code == nil)
msg("wuden\tpack\t2/3\tBBB")
msg("wuden\tpack\t3/3\tCCC.0123abcd")
check("the whole code", T.code == "leyline://pack/ley1.den.z.AAABBBCCC.0123abcd")
msg("wuden\tpack\t1/1\t/run DoSomething()")
check("not a code: kept as it was", T.code == "leyline://pack/ley1.den.z.AAABBBCCC.0123abcd")

-- /tale words are the Den's .den tale
SlashCmdList.LEYLINETALE("play den-1")
check("/tale play sends tale play den-1", sent[#sent][2] == "wuden\tcmd\t1/1\ttale play den-1")

-- no game-brand names in the file's own words
local f = assert(io.open(dir .. "/DenTale.lua")):read("*a")
check("no brand words", not f:lower():find("warcraft") and not f:lower():find("blizzard") and not f:find("%f[%a][Ww][Oo][Ww]%f[%A]"))

print(("%d passed, %d failed"):format(pass, fail))
if fail > 0 then os.exit(1) end
