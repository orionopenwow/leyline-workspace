-- WrathUnleashed: Leyline realms on character select (the browser game). Listed in ForeverUIGlueWeb.xml after
-- ForeverUIGlueWeb.lua; the file name starts with ForeverUI so fui-overlay.py takes it into the overlay.
--
-- The REALMS button of Forever UI's top bar opens the Leyline realm browser while the page offers it (the browser game:
-- wrath-page.js sets WUUI_LEYLINE_BROWSE = 1 on its poll); otherwise, and with Shift held, it does what it always did
-- (this server's own realm list). Without that button, a "Realms" button of our own sits at the top left.
-- The browser: every realm on the Leyline network from the page (search, Plays in browser, Own files, Has players),
-- a preview of the one picked (the host's words, house rules, players online, who's on when the realm shares it, its
-- mods, required and optional, the ones on your side marked, the version chip, uptime, Hero Key sign-in) and Join.
-- Join asks the page to hop (WUUI_LEYLINE_HOP, wrath-page.js fastHop: with Hero Key straight to that realm's
-- characters). A realm that needs another game version is never hopped to: the page shows its launcher button.
-- The page protocol is the in-game window's (WrathUnleashedUI/Leyline.lua): WUUI_LEYLINE_ASK out, WUUI_LEYLINE_NET /
-- WUUI_LEYLINE_PREVIEW in, then WUUI_LEYLINE_ONNET_GLUE(kind).
-- Every step is pcall-guarded: a failure leaves character select as Forever UI made it.

if not CharacterSelect then return end

local LG = { rows = {}, filter = {}, query = "", page = 1 }
LeylineGlue = LG

local W_, H_, ROWS, ROW_H = 720, 470, 9, 34
local FONT = {}
for _, k in ipairs({ "Normal", "Highlight", "NormalSmall", "HighlightSmall", "NormalLarge", "DisableSmall" }) do
	FONT[k] = (rawget(_G, "GlueFont" .. k) and ("GlueFont" .. k)) or (rawget(_G, "GameFont" .. k) and ("GameFont" .. k)) or nil
end

local function now() return (GetTime and GetTime()) or 0 end

local function text(parent, font, ...)
	local fs = parent:CreateFontString(nil, "OVERLAY", FONT[font])
	if not FONT[font] and fs.SetFontObject and rawget(_G, "SystemFont_Small") then fs:SetFontObject(SystemFont_Small) end
	fs:SetJustifyH("LEFT")
	if select("#", ...) > 0 then fs:SetPoint(...) end
	return fs
end

local function fill(frame, r, g, b, a, layer)
	local t = frame:CreateTexture(nil, layer or "BACKGROUND")
	t:SetTexture(r, g, b, a)
	t:SetAllPoints(frame)
	return t
end

local function button(parent, label, w, onClick, primary, ...)
	local b = CreateFrame("Button", nil, parent)
	b:SetWidth(w); b:SetHeight(24)
	fill(b, 0.20, 0.23, 0.28, 1)
	b.bg = fill(b, primary and 0.21 or 0.15, primary and 0.57 or 0.17, primary and 0.81 or 0.22, 1, "BORDER")
	b.bg:ClearAllPoints(); b.bg:SetPoint("TOPLEFT", 1, -1); b.bg:SetPoint("BOTTOMRIGHT", -1, 1)
	local hl = b:CreateTexture(nil, "HIGHLIGHT"); hl:SetTexture(1, 1, 1, 0.08); hl:SetAllPoints(b)
	local fs = text(b, "HighlightSmall", "LEFT", 4, 0)
	fs:SetPoint("RIGHT", -4, 0)
	fs:SetJustifyH("CENTER")
	b.label = fs
	b.SetLabel = function(self, t) fs:SetText(t) end
	b:SetLabel(label)
	if select("#", ...) > 0 then b:SetPoint(...) end
	b:SetScript("OnClick", onClick)
	return b
end

function LG.Ask(s) WUUI_LEYLINE_ASK = s; LG.askedAt = now() end

function LG.AskList(page)
	local f = {}
	if LG.filter.browser then f[#f + 1] = "b" end
	if LG.filter.own then f[#f + 1] = "o" end
	if LG.filter.players then f[#f + 1] = "p" end
	LG.page = math.max(1, page or 1)
	LG.Ask("list|" .. LG.page .. "|" .. table.concat(f, ",") .. "|" .. LG.query:gsub("[|%c]", ""):sub(1, 32))
	LG.Status("Asking for the Leyline list...")
end

function LG.Status(t) if LG.win then LG.win.status:SetText(t or "") end end

-- The preview's words (the same rules as the in-game window's L.PreviewTexts).
function LG.Texts(r, p)
	local t = { title = (p and p.name) or r.name }
	local chips = { "|cff8b5cf6" .. (r.chip or "?") .. "|r", (p and p.browser or r.browser) and "Plays in browser" or "Own game files" }
	local hk = p and p.hk or (r.hk == true and "yes" or nil)
	if hk == "ready" or hk == "yes" then chips[#chips + 1] = "|cff4cc38aHero Key sign-in|r" elseif hk == "no" then chips[#chips + 1] = "Own accounts" end
	t.chips = table.concat(chips, "  ·  ")
	local live = (p and p.live ~= nil) and p.live or r.live
	local online = p and tonumber(p.online) or tonumber(r.online)
	t.players = not live and "|cff9aa3b5Offline|r" or (online and (online .. (online == 1 and " player" or " players") .. " online now") or "Online")
	local up = p and tonumber(p.up) or tonumber(r.up)
	t.players = t.players .. (up and ("  ·  " .. up .. "% up lately") or "")
	t.desc = (p and p.desc ~= "" and p.desc) or r.desc or ""
	t.rules = p and (p.rules ~= "" and p.rules or "The host hasn't written house rules.") or "Loading..."
	if p and type(p.who) == "table" then
		t.who = #p.who > 0 and table.concat(p.who, ", ") or "Nobody right now."
	else
		t.who = p and "This realm doesn't share who's on." or "Loading..."
	end
	if p and type(p.mods) == "table" then
		local req, opt, other = {}, {}, {}
		for _, m in ipairs(p.mods) do
			if type(m) == "table" and type(m.name) == "string" then
				local n = m.name .. (m.client == true and " |cff5b8def(on your side)|r" or "")
				if m.req == true then req[#req + 1] = n elseif m.req == false then opt[#opt + 1] = n else other[#other + 1] = n end
			end
		end
		local lines = {}
		if #req > 0 then lines[#lines + 1] = "Required: " .. table.concat(req, ", ") end
		if #opt > 0 then lines[#lines + 1] = "Optional: " .. table.concat(opt, ", ") end
		if #other > 0 then lines[#lines + 1] = "Uses: " .. table.concat(other, ", ") end
		t.mods = #lines > 0 and table.concat(lines, "\n") or "No mods listed."
	else
		t.mods = "Loading..."
	end
	t.verdict = p and p.verdict or (not r.live and "offline") or ((r.build or 12340) ~= 12340 and "launcher") or "ok"
	t.why = p and p.why or ""
	if r.here then t.join, t.why = "You're here", "You're on this realm now."
	elseif t.verdict == "launcher" then t.join = "Open in launcher"
	elseif t.verdict == "offline" then t.join = "Offline"
	else t.join = "Join" end
	return t
end

function LG.Pick(r)
	LG.selected, LG.detail = r, nil
	if r and not r.here then LG.Ask("preview|" .. r.id) end
	LG.ShowPreview()
end

function LG.Join()
	local r = LG.selected
	if not r then return false end
	local t = LG.Texts(r, LG.detail)
	local w = LG.win
	if t.join == "You're here" then w.info:SetText("You're on this realm now."); return false end
	if t.verdict == "launcher" then
		LG.Ask("launch|" .. r.id)
		w.info:SetText(r.name .. " needs another game version: the bar at the top of the page opens it in the Leyline launcher.")
		return "launcher"
	end
	if t.verdict == "offline" then w.info:SetText(r.name .. " is offline right now."); return false end
	WUUI_LEYLINE_HOP = r.id   -- the page's watcher takes it (wrath-page.js watchHops -> fastHop)
	w.info:SetText("Going to " .. r.name .. "...")
	return true
end

function LG.OnNet(kind)
	local w = LG.win
	if not w then return end
	if kind == "list" then
		local n = WUUI_LEYLINE_NET
		if type(n) ~= "table" then return end
		LG.askedAt = nil
		if type(n.err) == "string" then LG.rows = {}; LG.Status(n.err); LG.Refresh(); return end
		local out = {}
		for _, r in ipairs(type(n.rows) == "table" and n.rows or {}) do
			if type(r) == "table" and type(r.id) == "string" and #r.id == 16 and #out < 50 then out[#out + 1] = r end
		end
		LG.rows, LG.offset, LG.net = out, 0, n
		local net = type(n.network) == "table" and n.network or {}
		LG.Status((tonumber(n.total) or 0) == 0 and "No realm matches." or string.format("%d realms · %d players online",
			tonumber(net.realms) or 0, tonumber(net.players) or 0))
		LG.Refresh()
	elseif kind == "preview" then
		local p = WUUI_LEYLINE_PREVIEW
		if type(p) == "table" and LG.selected and p.id == LG.selected.id then LG.detail = p; LG.ShowPreview() end
	end
end
WUUI_LEYLINE_ONNET_GLUE = function(kind) pcall(LG.OnNet, kind) end

function LG.Refresh()
	local w = LG.win
	if not w then return end
	w.fb:SetLabel(LG.filter.browser and "In browser: on" or "In browser")
	w.fo:SetLabel(LG.filter.own and "Own files: on" or "Own files")
	w.fp:SetLabel(LG.filter.players and "Has players" or "Everyone")
	for i, row in ipairs(w.rowsUi) do
		local r = LG.rows[(LG.offset or 0) + i]
		row.realm = r
		if r then
			row.name:SetText(r.name .. (r.here and "  |cff8b5cf6(you are here)|r" or "") .. (r.featured and "  |cff8b5cf6Featured|r" or ""))
			local chips = { r.chip or "?", r.browser and "Plays in browser" or "Own game files" }
			if r.hk == true then chips[#chips + 1] = "Hero Key" end
			row.sub:SetText(table.concat(chips, " · "))
			row.state:SetText(r.live and ("|cff4cc38a" .. (tonumber(r.online) or 0) .. " playing|r") or "|cff9aa3b5offline|r")
			if LG.selected and LG.selected.id == r.id then row.sel:Show() else row.sel:Hide() end
			row:Show()
		else
			row:Hide()
		end
	end
	local n = LG.net
	w.count:SetText(n and (tonumber(n.pages) or 1) > 1 and string.format("Page %d of %d", tonumber(n.page) or 1, tonumber(n.pages) or 1) or "")
end

function LG.Scroll(dir)
	local n = LG.net or {}
	if dir > 0 then
		if (LG.offset or 0) + ROWS < #LG.rows then LG.offset = LG.offset + ROWS
		elseif (tonumber(n.page) or 1) < (tonumber(n.pages) or 1) then LG.AskList((tonumber(n.page) or 1) + 1) end
	else
		if (LG.offset or 0) > 0 then LG.offset = math.max(0, LG.offset - ROWS)
		elseif (tonumber(n.page) or 1) > 1 then LG.AskList((tonumber(n.page) or 1) - 1) end
	end
	LG.Refresh()
end

function LG.ShowPreview()
	local w, r = LG.win, LG.selected
	if not w then return end
	local p = w.pv
	if not r then p:Hide(); w.empty:Show(); return end
	local t = LG.Texts(r, LG.detail)
	p.title:SetText(t.title); p.chips:SetText(t.chips); p.players:SetText(t.players); p.desc:SetText(t.desc)
	p.rules:SetText(t.rules); p.who:SetText(t.who); p.mods:SetText(t.mods)
	p.why:SetText(t.verdict == "launcher" and ("|cfff0d389" .. t.why .. "|r") or t.why)
	p.join:SetLabel(t.join)
	if t.join == "Offline" or t.join == "You're here" then p.join:Disable() else p.join:Enable() end
	w.empty:Hide(); p:Show()
	LG.Refresh()
end

local function build()
	local w = CreateFrame("Frame", "LeylineGlueFrame", CharacterSelect)
	w:SetWidth(W_); w:SetHeight(H_)
	w:SetPoint("CENTER", CharacterSelect, "CENTER", 0, 20)
	w:SetFrameStrata("DIALOG")
	w:EnableMouse(true)
	fill(w, 0.204, 0.227, 0.278, 1)
	local inner = fill(w, 0.102, 0.114, 0.145, 0.98, "BORDER")
	inner:ClearAllPoints(); inner:SetPoint("TOPLEFT", 1, -1); inner:SetPoint("BOTTOMRIGHT", -1, 1)
	local bar = w:CreateTexture(nil, "ARTWORK"); bar:SetTexture(0.082, 0.090, 0.118, 1)
	bar:SetPoint("TOPLEFT", 1, -1); bar:SetPoint("TOPRIGHT", -1, -1); bar:SetHeight(30)
	text(w, "Normal", "TOPLEFT", 14, -9):SetText("Leyline realms")
	button(w, "Close", 60, function() w:Hide() end, false, "TOPRIGHT", -6, -4)
	w.status = text(w, "HighlightSmall", "TOPLEFT", 14, -40)
	w.status:SetWidth(300)

	local ok, box = pcall(CreateFrame, "EditBox", "LeylineGlueSearch", w)
	if ok and box then
		box:SetWidth(150); box:SetHeight(22)
		box:SetPoint("TOPLEFT", 14, -60)
		fill(box, 0.08, 0.09, 0.12, 1)
		if box.SetFontObject and FONT.HighlightSmall then pcall(box.SetFontObject, box, _G[FONT.HighlightSmall]) end
		if box.SetTextInsets then box:SetTextInsets(6, 6, 0, 0) end
		box:SetAutoFocus(false)
		if box.SetMaxLetters then box:SetMaxLetters(32) end
		box.hint = text(box, "DisableSmall", "LEFT", box, "LEFT", 6, 0)
		box.hint:SetText("Search realms")
		box:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
		box:SetScript("OnEnterPressed", function(self) self:ClearFocus() end)
		box:SetScript("OnTextChanged", function(self)
			local q = (self:GetText() or ""):gsub("[|%c]", "")
			if q ~= LG.query then LG.query, LG.searchAt = q, now() end
			if q == "" then self.hint:Show() else self.hint:Hide() end
		end)
		w.search = box
	end
	w.fb = button(w, "In browser", 96, function() LG.filter.browser = not LG.filter.browser or nil; LG.AskList(1); LG.Refresh() end, false, "TOPLEFT", 170, -59)
	w.fo = button(w, "Own files", 88, function() LG.filter.own = not LG.filter.own or nil; LG.AskList(1); LG.Refresh() end, false, "TOPLEFT", 270, -59)
	w.fp = button(w, "Everyone", 88, function() LG.filter.players = not LG.filter.players or nil; LG.AskList(1); LG.Refresh() end, false, "TOPLEFT", 362, -59)

	w.rowsUi = {}
	for i = 1, ROWS do
		local row = CreateFrame("Button", nil, w)
		row:SetWidth(436); row:SetHeight(ROW_H - 2)
		row:SetPoint("TOPLEFT", 14, -92 - (i - 1) * ROW_H)
		local line = row:CreateTexture(nil, "BACKGROUND"); line:SetTexture(0.204, 0.227, 0.278, 1)
		line:SetPoint("BOTTOMLEFT", 0, -1); line:SetPoint("BOTTOMRIGHT", 0, -1); line:SetHeight(1)
		row.sel = row:CreateTexture(nil, "BORDER"); row.sel:SetTexture(0.21, 0.57, 0.81, 0.18); row.sel:SetAllPoints(row); row.sel:Hide()
		local hl = row:CreateTexture(nil, "HIGHLIGHT"); hl:SetTexture(1, 1, 1, 0.06); hl:SetAllPoints(row)
		row.name = text(row, "Normal", "TOPLEFT", 4, -2); row.name:SetWidth(330)
		row.sub = text(row, "HighlightSmall", "TOPLEFT", row.name, "BOTTOMLEFT", 0, -2); row.sub:SetWidth(330)
		row.state = text(row, "HighlightSmall", "RIGHT", -6, 0)
		row:SetScript("OnClick", function(self) if self.realm then LG.Pick(self.realm) end end)
		row:Hide()
		w.rowsUi[i] = row
	end
	w.count = text(w, "HighlightSmall", "BOTTOMLEFT", 16, 50)
	button(w, "Up", 50, function() LG.Scroll(-1) end, false, "BOTTOMLEFT", 344, 44)
	button(w, "Down", 54, function() LG.Scroll(1) end, false, "BOTTOMLEFT", 398, 44)
	w.info = text(w, "HighlightSmall", "BOTTOMLEFT", 16, 16); w.info:SetWidth(W_ - 230)
	w.native = button(w, "This server's realms", 150, function() w:Hide(); if LG.nativeClick then pcall(LG.nativeClick, LG.nav) end end, false,
		"BOTTOMRIGHT", -14, 12)

	-- the preview
	local pv = CreateFrame("Frame", nil, w)
	pv:SetPoint("TOPLEFT", 466, -40); pv:SetPoint("BOTTOMRIGHT", -14, 44)
	local function line(font, anchor, gap)
		local fs = text(pv, font, "TOPLEFT", anchor, "BOTTOMLEFT", 0, gap)
		fs:SetWidth(W_ - 466 - 14)
		return fs
	end
	pv.title = text(pv, "NormalLarge", "TOPLEFT", 0, 0); pv.title:SetWidth(W_ - 480)
	pv.chips = line("HighlightSmall", pv.title, -5)
	pv.players = line("HighlightSmall", pv.chips, -5)
	pv.desc = line("HighlightSmall", pv.players, -6)
	local h1 = line("Normal", pv.desc, -10); h1:SetText("House rules")
	pv.rules = line("HighlightSmall", h1, -3)
	local h2 = line("Normal", pv.rules, -10); h2:SetText("Who's on")
	pv.who = line("HighlightSmall", h2, -3)
	local h3 = line("Normal", pv.who, -10); h3:SetText("Mods")
	pv.mods = line("HighlightSmall", h3, -3)
	pv.why = text(pv, "HighlightSmall", "BOTTOMLEFT", 0, 30); pv.why:SetWidth(W_ - 480)
	pv.join = button(pv, "Join", 140, function() LG.Join() end, true, "BOTTOMLEFT", 0, 0)
	pv:Hide()
	w.pv = pv
	w.empty = text(w, "HighlightSmall", "TOPLEFT", 466, -60)
	w.empty:SetWidth(W_ - 480)
	w.empty:SetText("Pick a realm to see its rules, who's on and its mods before you join.")

	w:SetScript("OnUpdate", function()
		if LG.searchAt and now() - LG.searchAt >= 0.5 then LG.searchAt = nil; LG.AskList(1) end
		if LG.askedAt and now() - LG.askedAt > 6 then LG.askedAt = nil; LG.Status("Realm list unavailable") end
	end)
	w:SetScript("OnShow", function() LG.AskList(LG.page or 1) end)
	w:Hide()
	LG.win = w
	return w
end

function LG.Open()
	local w = LG.win or build()
	if w:IsShown() then w:Hide() else w:Show() end
	return w
end

-- The REALMS button of Forever UI's top bar, or our own.
pcall(function()
	local nav = ForeverUICharacterSelectNavBar
	local tray = nav and nav.GetChildren and nav:GetChildren()
	local found
	if tray and tray.GetChildren then
		for _, b in ipairs({ tray:GetChildren() }) do
			local t = b.GetText and b:GetText()
			if type(t) == "string" and (t:upper() == "REALMS" or t:upper() == "REALM LIST") then found = b end
		end
	end
	local function useOurs() return WUUI_LEYLINE_BROWSE == 1 and not (IsShiftKeyDown and IsShiftKeyDown()) end
	if found then
		LG.nav, LG.nativeClick = found, found:GetScript("OnClick")
		found:SetScript("OnClick", function(self, ...)
			if useOurs() then
				local ok = pcall(LG.Open)
				if ok then return end
			end
			if LG.nativeClick then return LG.nativeClick(self, ...) end
		end)
	else
		local parent = CharacterSelectUI or CharacterSelect
		LG.own = button(parent, "Realms", 110, function()
			if WUUI_LEYLINE_BROWSE == 1 then pcall(LG.Open) end
		end, false, "TOPLEFT", parent, "TOPLEFT", 20, -20)
	end
end)

LG.ROWS = ROWS

-- No stock realm wizard (2026-10-31 live: a first visit to a realm where you have no characters yet, e.g. the
-- Builders' Realm, showed the game's own "Choose a Realm" wizard with the old game logo). Every Leyline session
-- already has its realm, so the wizard goes straight on to character select, which takes a player with no
-- characters to character create. pcall-guarded: a failure leaves the wizard as it was.
pcall(function()
	local wiz = RealmWizard
	if not (wiz and wiz.HookScript and SetGlueScreen) then return end
	wiz:HookScript("OnShow", function(self)
		pcall(function()
			self:Hide()
			SetGlueScreen("charselect")
		end)
	end)
	LG.skipWizard = true
end)
