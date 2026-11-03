-- Tests for LeylineHybrid.lua with a stand-in for the glue screens (plain Lua 5.1, no game needed):
--   lua5.1 glue/test_hybrid_glue.lua glue/LeylineHybrid.lua     (hybrid.rs runs it when a Lua is installed)
-- Each case loads the file afresh into a fresh fake glue world, then ticks OnUpdate and moves between screens.
local FILE = arg[1] or "LeylineHybrid.lua"
local failed = 0

local function world()
	local W = { now = 0, calls = {}, frames = {} }
	local function frame(name)
		local f = { shown = false, scripts = {}, name = name }
		function f:IsShown() return self.shown end
		function f:Show() self.shown = true end
		function f:Hide() self.shown = false end
		function f:SetScript(k, fn) self.scripts[k] = fn end
		local function noop() end
		for _, m in ipairs({ "SetAllPoints", "SetFrameStrata", "EnableMouse", "SetPoint", "ClearAllPoints", "SetWidth", "SetHeight" }) do f[m] = noop end
		function f:CreateTexture()
			local t = {}
			for _, m in ipairs({ "SetAllPoints", "SetTexture", "SetWidth", "SetHeight", "SetPoint", "ClearAllPoints", "SetTexCoord" }) do t[m] = noop end
			return t
		end
		function f:CreateFontString()
			local t = { text = "" }
			for _, m in ipairs({ "SetFont", "SetTextColor", "SetPoint", "SetWidth" }) do t[m] = noop end
			function t:SetText(s) self.text = s end
			return t
		end
		return f
	end
	local env = {
		math = math, pcall = pcall, type = type, tostring = tostring, ipairs = ipairs, string = string,
		GetTime = function() return W.now end,
		CreateFrame = function(_, name) local f = frame(name); W.frames[#W.frames + 1] = f; if name then W[name] = f end; return f end,
		DefaultServerLogin = function(u, p) W.calls[#W.calls + 1] = "login " .. u .. " " .. p end,
		EnterWorld = function() W.calls[#W.calls + 1] = "enter" end,
		GetNumCharacters = function() return W.chars or 0 end,
		GetRealmCategories = function() return "Realms" end,
		GetNumRealms = function() return #(W.realms or {}) end,
		GetRealmInfo = function(_, i) return W.realms[i] end,
		ChangeRealm = function(c, i) W.calls[#W.calls + 1] = "realm " .. c .. " " .. i end,
		SetGlueScreen = function(n) W.calls[#W.calls + 1] = "screen " .. n; if W.onScreen then W.onScreen(n) end end,
		SetSelectedRace = function(r) W.race = r end,
		SetSelectedSex = function(x) W.sex = x end,
		SetSelectedClass = function(c) W.class = c end,
		CreateCharacter = function(n) W.calls[#W.calls + 1] = "create " .. n; if W.onCreate then W.onCreate(n) end end,
	}
	env.AccountLogin, env.CharacterSelect, env.GlueDialog, env.RealmList, env.GlueParent, env.CharacterCreate =
		frame("AccountLogin"), frame("CharacterSelect"), frame("GlueDialog"), frame("RealmList"), frame("GlueParent"), frame("CharacterCreate")
	-- Forever UI's glue, when a case puts it in: it keeps the stock screens and adds its own frames and table
	function W.foreverui()
		env.ForeverUIGlue = { version = "test" }
		env.CharacterSelectUI = frame("CharacterSelectUI")
		env.ForeverUICharacterSelectNavBar = frame("ForeverUICharacterSelectNavBar")
	end
	W.env = env
	function W.load(session)
		env.LEYLINE_SESSION = session
		local chunk = assert(loadfile(FILE))
		setfenv(chunk, env)
		chunk()
	end
	function W.tick(dt, n)
		for _ = 1, (n or 1) do
			W.now = W.now + (dt or 0.1)
			local d = W.LeylineHybridDriver
			if d and d.scripts.OnUpdate then d.scripts.OnUpdate(d) end
		end
	end
	function W.screen(name)
		for _, k in ipairs({ "AccountLogin", "CharacterSelect", "RealmList", "CharacterCreate" }) do env[k].shown = (k == name) end
	end
	function W.note() return W.LeylineHybridNote and W.LeylineHybridNote.shown and W.LeylineHybridNote.line.text end
	function W.cover() return W.LeylineHybridCover and W.LeylineHybridCover.shown end
	return W
end

local function case(name, fn)
	local ok, err = pcall(fn)
	if ok then print("ok   " .. name) else failed = failed + 1; print("FAIL " .. name .. ": " .. tostring(err)) end
end
local function eq(a, b, what) if a ~= b then error((what or "") .. ": " .. tostring(a) .. " ~= " .. tostring(b), 2) end end

local SESSION = { user = "HKAAAA", pass = "secret", realm = nil, enter = false }
local function copy(t) local o = {} for k, v in pairs(t) do o[k] = v end return o end

case("no session: nothing runs", function()
	local W = world()
	W.load(nil)
	W.screen("AccountLogin"); W.tick(0.1, 50)
	eq(#W.calls, 0, "calls"); eq(W.LeylineHybridDriver, nil, "no driver")
end)

case("signs in once, global cleared, cover up then down at character select", function()
	local W = world()
	W.load(copy(SESSION))
	eq(W.env.LEYLINE_SESSION, nil, "global cleared at load")
	W.screen("AccountLogin"); W.tick(0.1, 5)
	eq(#W.calls, 1, "one call"); eq(W.calls[1], "login HKAAAA secret")
	assert(W.cover(), "cover up while signing in")
	W.tick(0.1, 20); eq(#W.calls, 1, "never twice")
	W.screen("CharacterSelect"); W.tick(0.1, 2)
	assert(not W.cover(), "cover down"); eq(#W.calls, 1, "no enter without enter")
	assert(W.env.LeylineHybrid.state == "done")
	eq(W.env.LeylineHybrid.pass, nil, "no password on the debug table")
end)

case("an error dialog takes the cover down and stops", function()
	local W = world()
	W.load(copy(SESSION))
	W.screen("AccountLogin"); W.tick()
	W.env.GlueDialog.shown = true; W.tick()
	assert(not W.cover()); eq(W.env.LeylineHybrid.why, "dialog")
	W.env.GlueDialog.shown = false; W.screen("AccountLogin"); W.tick(0.1, 10)
	eq(#W.calls, 1, "the player signs in by hand from here")
end)

case("no login screen within 20 s: gives up without calling", function()
	local W = world()
	W.load(copy(SESSION))
	W.tick(1, 25)
	W.screen("AccountLogin"); W.tick(0.1, 5)
	eq(#W.calls, 0); eq(W.env.LeylineHybrid.why, "no login screen")
end)

case("stuck after sign-in: cover down after 45 s", function()
	local W = world()
	W.load(copy(SESSION))
	W.screen("AccountLogin"); W.tick()
	W.tick(1, 50)
	assert(not W.cover()); eq(W.env.LeylineHybrid.why, "no character select")
end)

case("realm picked by name on the realm list", function()
	local W = world()
	local s = copy(SESSION); s.realm = "Den Two"
	W.load(s)
	W.realms = { "Den One", "Den Two" }
	W.screen("AccountLogin"); W.tick()
	W.screen("RealmList"); W.tick(0.1, 3)
	eq(W.calls[2], "realm 1 2"); eq(#W.calls, 2, "picked once")
	W.screen("CharacterSelect"); W.tick()
	assert(not W.cover())
end)

case("enter: the last character enters the world once", function()
	local W = world()
	local s = copy(SESSION); s.enter = true
	W.load(s)
	W.screen("AccountLogin"); W.tick()
	W.screen("CharacterSelect"); W.chars = 2; W.tick(0.1, 30)
	eq(W.calls[2], "enter"); eq(#W.calls, 2, "once")
	assert(W.cover(), "cover stays for the game's loading screen")
	W.tick(1, 25)
	assert(not W.cover(), "and goes if nothing happens")
end)

case("enter with no character: asks for the create screen once, stops when it never shows", function()
	local W = world()
	local s = copy(SESSION); s.enter = true
	W.load(s)
	W.screen("AccountLogin"); W.tick()
	W.screen("CharacterSelect"); W.chars = 0; W.tick(1, 30)
	eq(W.calls[2], "screen charcreate"); eq(#W.calls, 2, "once"); assert(not W.cover())
	eq(W.env.LeylineHybrid.why, "character create")
end)

case("a broken session table is ignored", function()
	for _, bad in ipairs({ "x", { user = "", pass = "p" }, { user = "u" }, { user = 1, pass = 2 } }) do
		local W = world()
		W.load(bad)
		W.screen("AccountLogin"); W.tick(0.1, 5)
		eq(#W.calls, 0)
	end
end)

-- My Den (app.rs den_session): DENAUTO with the local password, "Your Den", enter = true
local DEN = { user = "DENAUTO", pass = "den", realm = "Your Den", enter = true }

local function denFlow(W, chars)
	W.load(copy(DEN))
	W.realms = { "Your Den" }
	W.chars = chars
	-- the game: CharacterCreate opens on SetGlueScreen, and CreateCharacter brings back character select with one
	W.onScreen = function(n) if n == "charcreate" then W.screen("CharacterCreate") end end
	W.onCreate = function() W.chars = 1; W.screen("CharacterSelect") end
	W.screen("AccountLogin"); W.tick(0.1, 3)
	eq(W.calls[1], "login DENAUTO den", "signed in")
	assert(W.cover(), "the login screen is hidden under the cover")
	W.screen("RealmList"); W.tick(0.1, 3)
	eq(W.calls[2], "realm 1 1", "Your Den picked")
	W.screen("CharacterSelect"); W.tick(0.1, 60)
end

for _, fui in ipairs({ false, true }) do
	local tag = fui and " (with Forever UI's glue)" or " (stock glue)"
	case("Den: login, realm, character select, enter" .. tag, function()
		local W = world()
		if fui then W.foreverui() end
		denFlow(W, 1)
		eq(W.calls[3], "enter"); eq(#W.calls, 3, "nothing else")
		assert(W.cover(), "cover over the game's own loading screen")
	end)
	case("Den: no character: one is made, then enter" .. tag, function()
		local W = world()
		if fui then W.foreverui() end
		denFlow(W, 0)
		eq(W.calls[3], "screen charcreate"); eq(W.calls[4], "create Wren")
		eq(W.race, 1); eq(W.sex, 3); eq(W.class, 1)
		eq(W.calls[5], "enter"); eq(#W.calls, 5)
	end)
end

case("Den: the only realm is picked when its name isn't on the list", function()
	local W = world()
	W.load(copy(DEN))
	W.realms = { "Another name" }
	W.screen("AccountLogin"); W.tick()
	W.screen("RealmList"); W.tick(0.1, 2)
	eq(W.calls[2], "realm 1 1")
	-- two realms, neither ours: the list stays for the player
	local W2 = world()
	W2.load(copy(DEN))
	W2.realms = { "A", "B" }
	W2.screen("AccountLogin"); W2.tick()
	W2.screen("RealmList"); W2.tick(0.1, 2)
	eq(#W2.calls, 1); assert(not W2.cover(), "the list shows")
end)

case("Den: making a character that doesn't finish stops with a plain line", function()
	local W = world()
	W.load(copy(DEN))
	W.onScreen = function(n) if n == "charcreate" then W.screen("CharacterCreate") end end
	W.onCreate = function() end           -- the server never answers
	W.screen("AccountLogin"); W.tick()
	W.screen("CharacterSelect"); W.chars = 0; W.tick(1, 30)
	eq(W.env.LeylineHybrid.why, "character create")
	assert(not W.cover(), "the normal screen again")
	local n = W.note()
	assert(n and n:find("making a character didn't finish", 1, true), tostring(n))
	assert(not n:find("den", 1, true), "never the password")
	W.tick(1, 25)
	assert(not W.note(), "the line goes after a while")
end)

case("every stop says why on the normal screen", function()
	local W = world()
	W.load(copy(SESSION))
	W.tick(1, 25)
	assert(W.note() and W.note():find("login screen didn't show", 1, true))
end)

if failed > 0 then print(failed .. " failed"); os.exit(1) end
print("all passed")
