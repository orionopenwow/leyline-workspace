-- lua5.1 glue/test_solo_glue.lua glue/LeylineSolo.lua [session.lua]
-- Leyline Solo's first-era login screen add-on (LeylineSolo.lua) against a stand-in login screen: sign-in, the realm
-- by name, `pick`, Quick start's `make` on the create screen, `enter`, and stepping aside on anything unexpected.
-- The optional session file is one the launcher wrote (host/classicsign.rs session_lua: escaped bytes); it must be
-- a Quick start for "Tamsin", a night elf druid. Runs in Lua 5.1 (CI) and later (texlua); the add-on itself is Lua
-- 5.0 only (host/classicsign.rs checks that).

local GLUE, SESSION = arg[1], arg[2]
local calls, failures, ran = {}, 0, 0

local function count(t) local n = 0; for _ in ipairs(t) do n = n + 1 end return n end
local function frame(shown) return { s = shown, IsShown = function(self) return self.s end } end

-- a stand-in login screen: frames, the realm list, character select and the create screen
local RACES = { { "Human", "Human" }, { "Dwarf", "Dwarf" }, { "Night Elf", "NightElf" }, { "Gnome", "Gnome" },
	{ "Orc", "Orc" }, { "Undead", "Scourge" }, { "Tauren", "Tauren" }, { "Troll", "Troll" } }
local CLASSES = { { "Warrior", "WARRIOR" }, { "Paladin", "PALADIN" }, { "Hunter", "HUNTER" }, { "Rogue", "ROGUE" },
	{ "Priest", "PRIEST" }, { "Shaman", "SHAMAN" }, { "Mage", "MAGE" }, { "Warlock", "WARLOCK" }, { "Druid", "DRUID" } }
local chars, race, class

local function screen(opts)
	opts = opts or {}
	calls = {}
	chars = opts.chars or { "Bob", "Wren" }
	race, class = 1, 1
	AccountLogin, CharacterSelect, RealmList, GlueDialog, CharacterCreate = frame(true), frame(false), frame(false), frame(false), frame(false)
	CharacterCreateGenderButtonFemale = { Click = function() calls.female = true end }
	LeylineSoloNote = { SetText = function(self, t) calls.note = t end, SetFont = function() end }
	LeylineSoloDriver = { Show = function() end }
	function DefaultServerLogin(u, p)
		calls.login = u .. "/" .. p
		AccountLogin.s = false
		if opts.refuse then
			GlueDialog.s, GlueDialog.which = true, "OKAY"
		elseif opts.realms then
			RealmList.s = true
		else
			CharacterSelect.s = true
		end
	end
	function GetNumRealms() return opts.realms and count(opts.realms) or 0 end
	function GetRealmInfo(i) return opts.realms[i] end
	function ChangeRealm(i) calls.realm = i; RealmList.s = false; CharacterSelect.s = true end
	function GetNumCharacters() return count(chars) end
	function GetCharacterInfo(i) return chars[i] end
	function SelectCharacter(i) calls.select = i end
	function EnterWorld() calls.enter = true end
	function SetGlueScreen(name)
		calls.screen = name
		if name == "charcreate" then CharacterSelect.s = false; CharacterCreate.s = true end
	end
	function SetSelectedRace(i) if not RACES[i] then error("no race " .. i) end race = i end
	function GetNameForRace() return RACES[race][1], RACES[race][2] end
	function SetSelectedClass(i) if not CLASSES[i] then error("no class " .. i) end class = i end
	function GetSelectedClass() return CLASSES[class][1], CLASSES[class][2] end
	function SetSelectedSex(s) calls.sex = s end
	function CreateCharacter(name)
		calls.made = name .. ":" .. RACES[race][2] .. ":" .. CLASSES[class][2]
		table.insert(chars, name)
		CharacterCreate.s = false
		CharacterSelect.s = true
	end
end

local function run(ticks, dt)
	for _ = 1, ticks do LeylineSolo_OnUpdate(dt or 0.2) end
end

local function load(session)
	LEYLINE_SOLO_SESSION = session
	dofile(GLUE)
	assert(LEYLINE_SOLO_SESSION == nil, "the global is cleared")
end

local function noPassword(pw)
	for _, v in pairs(LeylineSolo) do assert(v ~= pw, "no password kept") end
end

local function case(name, f)
	ran = ran + 1
	local ok, err = pcall(f)
	if ok then print("ok   " .. name) else failures = failures + 1; print("FAIL " .. name .. ": " .. tostring(err)) end
end

case("signs in and stops at character select", function()
	screen()
	load({ user = "SOLO", pass = "PW12", realm = "Leyline Solo" })
	run(20)
	assert(calls.login == "SOLO/PW12", tostring(calls.login))
	assert(LeylineSolo.state == "done" and LeylineSolo.why == nil, LeylineSolo.state)
	assert(not calls.enter, "nothing entered without pick, make or enter")
	noPassword("PW12")
end)

case("picks the realm by name, then enters the picked character", function()
	screen({ realms = { "Somewhere else", "Leyline Solo" } })
	load({ user = "SOLO", pass = "PW12", realm = "Leyline Solo", pick = "Wren" })
	run(40)
	assert(calls.realm == 2, "the realm by its name")
	assert(calls.select == 2 and calls.enter, "Wren selected and entered")
	assert(LeylineSolo.state == "entering", LeylineSolo.state)
	noPassword("PW12")
end)

case("Phase 0's `char` is the same as pick", function()
	screen()
	load({ user = "SOLO", pass = "PW12", realm = "Leyline Solo", char = "Bob" })
	run(40)
	assert(calls.select == 1 and calls.enter)
end)

case("enter: whichever character the list has", function()
	screen()
	load({ user = "SOLO", pass = "PW12", realm = "Leyline Solo", enter = true })
	run(40)
	assert(calls.enter and calls.select == nil)
end)

case("a picked character that isn't there stops and says why", function()
	screen()
	load({ user = "SOLO", pass = "PW12", realm = "Leyline Solo", pick = "Nobody" })
	run(60)
	assert(LeylineSolo.state == "done" and LeylineSolo.why == "no character", tostring(LeylineSolo.why))
	assert(not calls.enter)
	assert(string.find(calls.note, "stopped"), calls.note)
end)

case("Quick start: made on the create screen, then entered", function()
	screen()
	if SESSION then
		dofile(SESSION)       -- the launcher's own escaped session
		load(LEYLINE_SOLO_SESSION)
	else
		load({ user = "SOLO", pass = "PW12", realm = "Leyline Solo", make = { name = "Tamsin", race = "NightElf", class = "DRUID", female = true } })
	end
	run(80)
	assert(calls.screen == "charcreate", tostring(calls.screen))
	assert(calls.made == "Tamsin:NightElf:DRUID", tostring(calls.made))
	assert(calls.female, "the female button")
	assert(calls.select == 3 and calls.enter, "the new one entered by its name")
	assert(LeylineSolo.state == "entering", LeylineSolo.state)
end)

case("Quick start with a race this world hasn't: nothing made, it says why", function()
	screen()
	load({ user = "SOLO", pass = "PW12", realm = "Leyline Solo", make = { name = "Tamsin", race = "BloodElf", class = "MAGE" } })
	run(80)
	assert(calls.made == nil and not calls.enter)
	assert(LeylineSolo.why == "character create", tostring(LeylineSolo.why))
end)

case("an error dialog stops it and says why", function()
	screen({ refuse = true })
	load({ user = "SOLO", pass = "X", realm = "Leyline Solo" })
	run(5)
	assert(LeylineSolo.state == "done" and LeylineSolo.why == "dialog", tostring(LeylineSolo.why))
	assert(string.find(calls.note, "stopped"), calls.note)
end)

case("without a session nothing runs", function()
	screen()
	load(nil)
	run(10, 0.5)
	assert(calls.login == nil and LeylineSolo.state == "off")
end)

case("no login screen: it gives up and says so", function()
	screen()
	AccountLogin.s = false
	load({ user = "SOLO", pass = "PW12" })
	run(30, 1)
	assert(LeylineSolo.why == "no login screen", tostring(LeylineSolo.why))
end)

print(string.format("%d ran, %d failed", ran, failures))
if failures == 0 then print("all passed") else os.exit(1) end
