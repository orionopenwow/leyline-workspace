-- Leyline Solo (docs/LEYLINE-SOLO.md; Phase 0: docs/LEYLINE-SOLO-PHASE0.md): the first-era login screen signs in to
-- the player's own local world by itself. Packed by the Leyline launcher (launcher/src-tauri/src/host/classicsign.rs,
-- the "addon" sign-in) or by tools/solo/solo_proof.py (signin) into a patch archive of ours in the player's own game
-- folder, after the login screen's own files. LeylineSoloSession.lua (loaded just before this file) holds the local
-- sign-in: account, password, realm name, and optionally what to do at character select:
--   pick = "Name"                                   enter the world with that character (`char` is the same, Phase 0)
--   make = { name =, race =, class =, female = }    make that character first (the race's and class's file names, as
--                                                   the create screen's GetNameForRace / GetSelectedClass give them),
--                                                   then enter it (Quick start)
--   enter = true                                    enter with the character the list has selected
-- With none of them it stops at character select.
--
-- Our own code only, written for the first-era login screen: Lua 5.0 (no # operator, no %, no string.match) and its
-- handlers (`this`, `arg1`). Stock functions only: DefaultServerLogin (as the stock Login button), the realm list's
-- GetNumRealms / GetRealmInfo / ChangeRealm, GetNumCharacters / GetCharacterInfo / SelectCharacter / EnterWorld, and
-- for Quick start SetGlueScreen("charcreate"), SetSelectedRace / GetNameForRace, SetSelectedClass / GetSelectedClass
-- and CreateCharacter (the stock create screen's own calls).
--
-- Rules: the password is read once into a local, the global is cleared at once, and the local is dropped as soon
-- as it was handed over. Every step is pcall-guarded; anything unexpected (an error dialog, a timeout) stops here
-- and leaves the stock screen as it is, with one line that says why, so the player can always sign in by hand.

local S = LEYLINE_SOLO_SESSION
LEYLINE_SOLO_SESSION = nil
local L = { state = "off", t = 0 }
LeylineSolo = L   -- for a look from a debug build; never holds the password

if type(S) == "table" and type(S.user) == "string" and type(S.pass) == "string" and S.user ~= "" and S.pass ~= "" then
	L.state = "wait"
	if S.pick == nil and type(S.char) == "string" then S.pick = S.char end
else
	S = nil
end

local WAIT_LOGIN, WAIT_SELECT, WAIT_ENTER, WAIT_CREATE = 20, 60, 20, 20
-- the stock dialogs that mean "stopped" (the connecting dialog is a different one and is waited out)
local STOP_DIALOGS = { OKAY = 1, OKAY_HTML = 1, CONNECTION_HELP = 1, CONNECTION_HELP_HTML = 1, ERROR = 1 }
local WHY = {
	["dialog"] = "the server answered with a message",
	["no login screen"] = "the login screen didn't show",
	["no character select"] = "character select didn't show",
	["no character"] = "that character isn't on this world yet",
	["character create"] = "making the character didn't finish",
	["enter world"] = "entering the world didn't start",
	["login call failed"] = "the login screen didn't take it",
}

local function shown(f)
	return f and f.IsShown and f:IsShown()
end

local function say(text)
	pcall(function()
		if LeylineSoloNote.SetFont then LeylineSoloNote:SetFont("Fonts\\FRIZQT__.TTF", 13) end
		LeylineSoloNote:SetText(text)
		LeylineSoloDriver:Show()
	end)
end

local function stop(why)
	L.state, L.why = "done", why
	S = nil
	if why then
		say("Leyline Solo: signing in by itself stopped (" .. (WHY[why] or why) .. "). Carry on here as usual.")
		L.noteFor = 20
	else
		say("")
	end
end

local function dialogStops()
	if not shown(GlueDialog) then return false end
	local w = GlueDialog.which
	return w ~= nil and STOP_DIALOGS[w] ~= nil
end

local function okName(n)
	return type(n) == "string" and string.len(n) >= 2 and string.len(n) <= 12 and string.find(n, "^[A-Za-z]+$") ~= nil
end

local function upper(v)
	if type(v) == "string" then return string.upper(v) end
	return ""
end

-- The realm called S.realm on the realm list (or the only one there is), chosen as the list's OK button would.
local function pickRealm()
	local ok, hit = pcall(function()
		local n = GetNumRealms and GetNumRealms() or 0
		local only = nil
		for i = 1, n do
			local name = GetRealmInfo(i)
			if type(S.realm) == "string" and name == S.realm then
				ChangeRealm(i)
				return true
			end
			only = i
		end
		if n == 1 and only then
			ChangeRealm(only)
			return true
		end
		return false
	end)
	return ok and hit
end

-- The character called `want` on character select, selected (true), when it is in the list.
local function selectChar(want)
	local ok, hit = pcall(function()
		local n = GetNumCharacters and GetNumCharacters() or 0
		for i = 1, n do
			local name = GetCharacterInfo(i)
			if type(name) == "string" and string.lower(name) == string.lower(want) then
				SelectCharacter(i)
				return true
			end
		end
		return false
	end)
	return ok and hit
end

-- The create screen's race (or class) whose file name is `want`: tried by index, as the stock buttons set them.
local function selectBy(set, get, want, n)
	if type(want) ~= "string" or want == "" then return false end
	want = string.upper(want)
	for i = 1, n do
		if pcall(set, i) then
			local a, b = get()
			if upper(b) == want or upper(a) == want then return true end
		end
	end
	return false
end

-- Quick start's character: its race, sex and class picked as the stock create screen's buttons do, then
-- CreateCharacter, as the stock Create button. A race or class this world doesn't offer: false, nothing made.
local function createCharacter()
	local m = S.make
	local ok, made = pcall(function()
		if not selectBy(SetSelectedRace, GetNameForRace, m.race, 12) then return false end
		if m.female then
			if CharacterCreateGenderButtonFemale and CharacterCreateGenderButtonFemale.Click then
				CharacterCreateGenderButtonFemale:Click()
			elseif SetSelectedSex then
				SetSelectedSex(1)
			end
		end
		if not selectBy(SetSelectedClass, GetSelectedClass, m.class, 12) then return false end
		CreateCharacter(m.name)
		return true
	end)
	return ok and made
end

local function wantsWorld()
	return okName(S.pick) or type(S.make) == "table" or S.enter == true
end

local function step()
	if L.state == "wait" then
		if shown(AccountLogin) then
			say("Leyline Solo: signing you in to your own world...")
			local user, pass = S.user, S.pass
			S.pass = nil
			local ok = pcall(DefaultServerLogin, user, pass)
			pass = nil
			if not ok then return stop("login call failed") end
			L.state, L.t = "sent", 0
		elseif L.t > WAIT_LOGIN then
			stop("no login screen")
		end
	elseif L.state == "sent" then
		if dialogStops() then
			stop("dialog")                       -- the stock dialog says why; the login screen stays as it is
		elseif shown(CharacterSelect) then
			if type(S.make) == "table" and okName(S.make.name) then
				L.state, L.t = "tocreate", 0
				say("Leyline Solo: making your character...")
			elseif wantsWorld() then
				L.state, L.t = "select", 0
				say("Leyline Solo: entering the world...")
			else
				stop(nil)                        -- character select, by itself
			end
		elseif shown(RealmList) then
			if not L.realmTried then
				L.realmTried = true
				pickRealm()
			end
		elseif L.t > WAIT_SELECT then
			stop("no character select")
		end
	elseif L.state == "tocreate" then
		if L.t > 1 then
			if not SetGlueScreen or not CreateCharacter then return stop("character create") end
			if not pcall(SetGlueScreen, "charcreate") then return stop("character create") end
			L.state, L.t = "create", 0
		end
	elseif L.state == "create" then
		if dialogStops() then
			stop("character create")
		elseif shown(CharacterCreate) and not L.sentCreate then
			if L.t > 0.5 then
				L.sentCreate = true
				if not createCharacter() then return stop("character create") end
				S.pick = S.make.name                -- then entered by its name
				S.make = nil
			end
		elseif L.sentCreate and shown(CharacterSelect) then
			L.state, L.t = "select", 0           -- the list now has it
			say("Leyline Solo: entering the world...")
		elseif L.t > WAIT_CREATE then
			stop("character create")
		end
	elseif L.state == "select" then
		if L.t > 1 then
			local chosen = false
			if okName(S.pick) then
				chosen = selectChar(S.pick)
			else
				chosen = (GetNumCharacters and GetNumCharacters() or 0) > 0
			end
			if chosen then
				pcall(EnterWorld)
				L.state, L.t = "entering", 0
				S = nil
			elseif L.t > 4 then
				stop("no character")
			end
		end
	elseif L.state == "entering" then
		if dialogStops() or L.t > WAIT_ENTER then stop("enter world") end
	end
end

function LeylineSolo_OnUpdate(elapsed)
	elapsed = tonumber(elapsed) or 0
	L.t = L.t + elapsed
	if L.state ~= "done" and L.state ~= "off" then
		local ok = pcall(step)
		if not ok then stop("something unexpected happened") end
	elseif L.noteFor then
		L.noteFor = L.noteFor - elapsed
		if L.noteFor <= 0 then
			L.noteFor = nil
			say("")
		end
	end
end
