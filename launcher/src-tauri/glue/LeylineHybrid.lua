-- Leyline hybrid client (docs/HYBRID-CLIENT.md): the desktop game starts like the browser game.
-- Packed by the Leyline launcher (launcher/src-tauri/src/hybrid.rs) into the player's own patch archive, listed in
-- GlueXML.toc right after Forever UI's glue, and loaded after LeylineSession.lua (the launcher writes that file for
-- one start of the game and empties it again when the game closes).
--
-- With a session: the login screen signs in by itself with the one-time login the realm made for this start
-- (DefaultServerLogin, as the stock Login button does), under a cover with the spinning Leyline mark, a bar and a
-- tip, until character select is up; the realm is picked by name when the realm list shows; with `enter`, the
-- last character enters the world. Without a session nothing here runs, and the screens are Forever UI's own.
--
-- Rules: the password is read once into a local, the global is cleared at once, and the local is dropped as soon
-- as it was handed over. Every step is pcall-guarded; anything unexpected (an error dialog, a timeout) takes the
-- cover down and leaves the stock screen as it is, so the player can always sign in by hand.

local S = LEYLINE_SESSION
LEYLINE_SESSION = nil
if type(S) ~= "table" or type(S.user) ~= "string" or type(S.pass) ~= "string" or S.user == "" or S.pass == "" then
	return
end

local L = { state = "wait", t = 0, tipAt = 0, tip = 0 }
LeylineHybrid = L   -- for /dump in a debug build; holds no password

local TIPS = {
	"Type /realms to see every realm on the network and hop between them.",
	"Your Hero Key signs you in on any realm in the network. No password to remember.",
	"Build in your Den: drop a stamp like the Inn, then move, turn and undo each piece.",
	"Type /shot to hide the interface for a clean screenshot.",
	"Ideas, questions or a realm to share? The forum is open at leylinelauncher.com.",
	"Host your own realm from the Leyline launcher in three steps.",
}
local WAIT_LOGIN, WAIT_SELECT, WAIT_ENTER = 20, 45, 20
local MARK = "Interface\\Glues\\Leyline\\LeylineMark"

local function shown(f) return f and f.IsShown and f:IsShown() end

-- ---------- the cover
local cover
local function rotate(tex, a)
	local c, s = math.cos(a), math.sin(a)
	local function at(dx, dy) return 0.5 + dx * c - dy * s, 0.5 + dx * s + dy * c end
	local ulx, uly = at(-0.5, -0.5)
	local llx, lly = at(-0.5, 0.5)
	local urx, ury = at(0.5, -0.5)
	local lrx, lry = at(0.5, 0.5)
	tex:SetTexCoord(ulx, uly, llx, lly, urx, ury, lrx, lry)
end

local function makeCover()
	local f = CreateFrame("Frame", "LeylineHybridCover", GlueParent or UIParent)
	f:SetAllPoints()
	f:SetFrameStrata("FULLSCREEN")
	f:EnableMouse(true)
	local bg = f:CreateTexture(nil, "BACKGROUND")
	bg:SetAllPoints()
	bg:SetTexture(0.03, 0.04, 0.07, 1)
	local mark = f:CreateTexture(nil, "ARTWORK")
	mark:SetTexture(MARK)
	mark:SetWidth(128); mark:SetHeight(128)
	mark:SetPoint("CENTER", f, "CENTER", 0, 40)
	f.mark = mark
	local track = f:CreateTexture(nil, "ARTWORK")
	track:SetTexture(1, 1, 1, 0.12)
	track:SetWidth(360); track:SetHeight(4)
	track:SetPoint("TOP", mark, "BOTTOM", 0, -36)
	local seg = f:CreateTexture(nil, "OVERLAY")
	seg:SetTexture(0.55, 0.75, 1, 0.9)
	seg:SetWidth(90); seg:SetHeight(4)
	f.track, f.seg = track, seg
	local line = f:CreateFontString(nil, "OVERLAY")
	line:SetFont("Fonts\\FRIZQT__.TTF", 15)
	line:SetTextColor(0.92, 0.94, 1)
	line:SetPoint("TOP", track, "BOTTOM", 0, -16)
	f.line = line
	local tip = f:CreateFontString(nil, "OVERLAY")
	tip:SetFont("Fonts\\FRIZQT__.TTF", 12)
	tip:SetTextColor(0.7, 0.75, 0.85)
	tip:SetWidth(560)
	tip:SetPoint("TOP", line, "BOTTOM", 0, -12)
	f.tip = tip
	return f
end

local function coverUp(text)
	pcall(function()
		cover = cover or makeCover()
		cover.line:SetText(text)
		cover:Show()
	end)
end

local function coverDown()
	pcall(function() if cover then cover:Hide() end end)
end

local function coverTick(now)
	if not shown(cover) then return end
	pcall(function()
		rotate(cover.mark, -now * 2.2)
		local x = (now * 0.6) % 1                      -- a segment gliding along the track
		cover.seg:ClearAllPoints()
		cover.seg:SetPoint("LEFT", cover.track, "LEFT", x * 270, 0)
		if now >= L.tipAt then
			L.tip = L.tip % #TIPS + 1
			cover.tip:SetText(TIPS[L.tip])
			L.tipAt = now + 7
		end
	end)
end

-- ---------- the steps
local function giveUp(why)
	L.state, L.why = "done", why
	S = nil
	coverDown()
end

local function pickRealm()
	if type(S.realm) ~= "string" or S.realm == "" then return false end
	local ok, hit = pcall(function()
		local cats = { GetRealmCategories() }
		for c = 1, math.max(#cats, 1) do
			for i = 1, (GetNumRealms(c) or 0) do
				if GetRealmInfo(c, i) == S.realm then
					ChangeRealm(c, i)
					return true
				end
			end
		end
		return false
	end)
	return ok and hit
end

local function step(now)
	if L.state == "wait" then
		if shown(AccountLogin) then
			coverUp("Signing you in...")
			local user, pass = S.user, S.pass
			S.pass = nil
			local ok = pcall(DefaultServerLogin, user, pass)
			pass = nil
			if not ok then return giveUp("login call failed") end
			L.state, L.t = "sent", now
		elseif now - L.t > WAIT_LOGIN then
			giveUp("no login screen")
		end
	elseif L.state == "sent" then
		if shown(GlueDialog) then
			giveUp("dialog")                           -- the stock dialog says why; the login screen stays as it is
		elseif shown(CharacterSelect) then
			L.state, L.t = S.enter and "select" or "done", now
			if not S.enter then coverDown(); S = nil end
		elseif shown(RealmList) then
			if not L.realmTried then
				L.realmTried = true
				if pickRealm() then coverUp("Joining the realm...") else coverDown() end
			end
		elseif now - L.t > WAIT_SELECT then
			giveUp("no character select")
		end
	elseif L.state == "select" then
		if not shown(CharacterSelect) then
			if now - L.t > WAIT_ENTER then giveUp("left character select") end
		elseif (GetNumCharacters and GetNumCharacters() or 0) > 0 and now - L.t > 1 then
			coverUp("Entering the world...")
			pcall(EnterWorld)
			L.state, L.t = "entering", now
			S = nil
		elseif now - L.t > WAIT_ENTER then
			giveUp("no character")
		end
	elseif L.state == "entering" then
		-- the game's own loading screen follows; if it doesn't (a dialog, a refusal), the cover goes
		if shown(GlueDialog) or now - L.t > WAIT_ENTER then giveUp("enter world") end
	end
end

local driver = CreateFrame("Frame", "LeylineHybridDriver", GlueParent or UIParent)
L.t = GetTime()
driver:SetScript("OnUpdate", function(self)
	local now = GetTime()
	local ok, err = pcall(step, now)
	if not ok then giveUp("error: " .. tostring(err)) end
	coverTick(now)
	if L.state == "done" and not shown(cover) then self:SetScript("OnUpdate", nil) end
end)
