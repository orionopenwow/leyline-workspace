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
	}
	env.AccountLogin, env.CharacterSelect, env.GlueDialog, env.RealmList, env.GlueParent =
		frame("AccountLogin"), frame("CharacterSelect"), frame("GlueDialog"), frame("RealmList"), frame("GlueParent")
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
		for _, k in ipairs({ "AccountLogin", "CharacterSelect", "RealmList" }) do env[k].shown = (k == name) end
	end
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

case("enter with no character: stays on character select", function()
	local W = world()
	local s = copy(SESSION); s.enter = true
	W.load(s)
	W.screen("AccountLogin"); W.tick()
	W.screen("CharacterSelect"); W.chars = 0; W.tick(1, 25)
	eq(#W.calls, 1); assert(not W.cover())
end)

case("a broken session table is ignored", function()
	for _, bad in ipairs({ "x", { user = "", pass = "p" }, { user = "u" }, { user = 1, pass = 2 } }) do
		local W = world()
		W.load(bad)
		W.screen("AccountLogin"); W.tick(0.1, 5)
		eq(#W.calls, 0)
	end
end)

if failed > 0 then print(failed .. " failed"); os.exit(1) end
print("all passed")
