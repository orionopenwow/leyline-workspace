-- Leyline: the look of Loose Bags (Loose Bags by Eric R, MIT, see LICENSE.txt). With Forever UI loaded, every piece
-- comes from Forever UI's own public functions at run time, the same ones its bag windows use (ForeverUI.SetPanelArt:
-- the metal bag frame and portrait ring; Templates.CloseButton; the search box, Clean Up button, purse box and slot
-- outline of its Bags.lua; Templates.DialogFrame and ThreeSliceButton for the settings). No art is copied: the
-- atlas names are looked up in Forever UI's tables. Without Forever UI (or if any call fails) a clean look from the
-- client's own art is drawn instead. Every Forever UI call is guarded: a client that lacks one gets the plain look.
local Logic = LooseBagsLogic
local LB = Logic.LB
local C = LB.Compat
local S = {}
LB.Skin = S

local SEP = string.char(92)
local function Path(...)
	return table.concat({ ... }, SEP)
end
S.Path = Path

local SQUARE_HOVER = Path("Interface", "Buttons", "ButtonHilight-Square")
local FLAT = Path("Interface", "ChatFrame", "ChatFrameBackground")
local MONEY_ICONS = Path("Interface", "MoneyFrame", "UI-MoneyIcons")
S.BACKPACK_ICON = Path("Interface", "Icons", "INV_Misc_Bag_08")
S.EMPTY_BAG = Path("Interface", "PaperDoll", "UI-PaperDoll-Slot-Bag")
-- Forever UI's panel background colour (AtlasUtil.lua PANEL_BACKGROUND, measured on the modern client): the plain
-- look uses it too, so both read as the same window.
S.PANEL = { 16 / 255, 14 / 255, 12 / 255 }
-- Forever UI frames every slot in dark grey; the quality colour takes over from uncommon (Bags.lua).
S.FRAME_GRAY = 0.39

local function FUI()
	local F = rawget(_G, "ForeverUI")
	if type(F) ~= "table" then
		return nil
	end
	return F
end

-- Forever UI is there and its atlas data answers.
function S.Forever()
	local F = FUI()
	if not F or type(F.SetPanelArt) ~= "function" or type(F.SetAtlas) ~= "function" or type(F.AtlasEntry) ~= "function" then
		return false
	end
	if type(F.Templates) ~= "table" then
		return false
	end
	local ok, entry = pcall(F.AtlasEntry, "ui-frame-metal-cornertopright")
	return ok and type(entry) == "table"
end

local function Try(fn, ...)
	if type(fn) ~= "function" then
		return false
	end
	local ok, a, b, c = pcall(fn, ...)
	if not ok then
		return false
	end
	return true, a, b, c
end
S.Try = Try

-- ForeverUI.SetAtlas on a texture; false when the atlas is unknown or the call fails.
function S.Atlas(texture, name, keepSize)
	local F = FUI()
	if not F or not texture then
		return false
	end
	local ok, done = Try(F.SetAtlas, texture, name, keepSize)
	return ok and done == true
end

local function Entry(name)
	local F = FUI()
	if not F then
		return nil
	end
	local ok, entry = Try(F.AtlasEntry, name)
	if ok and type(entry) == "table" then
		return entry
	end
	return nil
end
S.Entry = Entry

local function Tpl()
	local F = FUI()
	return F and type(F.Templates) == "table" and F.Templates or nil
end

local function Backdrop(frame, edge, edgeSize, inset, r, g, b, a)
	if not frame.SetBackdrop then
		local bg = frame:CreateTexture(nil, "BACKGROUND")
		bg:SetAllPoints(frame)
		C.Solid(bg, r, g, b, a)
		return
	end
	frame:SetBackdrop({
		bgFile = FLAT,
		edgeFile = edge,
		tile = true,
		tileSize = 16,
		edgeSize = edgeSize,
		insets = { left = inset, right = inset, top = inset, bottom = inset },
	})
	frame:SetBackdropColor(r, g, b, a)
end

-- ---------------------------------------------------------------- the window
-- Forever UI: SetPanelArt (HeldBagLayout metal, the small portrait ring), the portrait cut round by the ring at
-- (14, -17), the title in a child band from 35 to -24 (Bags.lua skinFrame). Plain: the client's dialog border on
-- the same dark panel colour, a round portrait, the title in the same band.
function S.Window(frame, title)
	local parts = {}
	local fancy = S.Forever()
	if fancy then
		local F = FUI()
		fancy = Try(F.SetPanelArt, frame)
	end
	parts.forever = fancy and true or false
	local portrait = frame:CreateTexture("LooseBagsFramePortrait", fancy and "BORDER" or "ARTWORK")
	if fancy then
		portrait:SetWidth(28)
		portrait:SetHeight(28)
		portrait:SetPoint("CENTER", frame, "TOPLEFT", 14, -17)
		portrait:SetTexture(S.BACKPACK_ICON)
		portrait:SetTexCoord(4 / 64, 60 / 64, 4 / 64, 60 / 64)
	else
		Backdrop(frame, Path("Interface", "DialogFrame", "UI-DialogBox-Border"), 24, 6, S.PANEL[1], S.PANEL[2], S.PANEL[3], 0.97)
		portrait:SetWidth(30)
		portrait:SetHeight(30)
		portrait:SetPoint("TOPLEFT", frame, "TOPLEFT", 9, -8)
		if type(SetPortraitToTexture) == "function" then
			if not Try(SetPortraitToTexture, portrait, S.BACKPACK_ICON) then
				portrait:SetTexture(S.BACKPACK_ICON)
			end
		else
			portrait:SetTexture(S.BACKPACK_ICON)
		end
	end
	parts.portrait = portrait
	local band = CreateFrame("Frame", nil, frame)
	band:SetFrameLevel(frame:GetFrameLevel() + 2)
	band:SetHeight(20)
	band:SetPoint("TOPLEFT", frame, "TOPLEFT", fancy and 35 or 42, fancy and -1 or -6)
	band:SetPoint("TOPRIGHT", frame, "TOPRIGHT", -24, fancy and -1 or -6)
	local text = band:CreateFontString(nil, "OVERLAY", "GameFontNormal")
	text:SetPoint("TOP", band, "TOP", 0, -5)
	text:SetPoint("LEFT", band, "LEFT")
	text:SetPoint("RIGHT", band, "RIGHT")
	if text.SetJustifyH then
		text:SetJustifyH("CENTER")
	end
	text:SetText(title or "")
	parts.band = band
	parts.title = text
	return parts
end

-- After a resize: Forever UI crops the bottom corners of a short window (NineSliceUtil.UpdateCornerCropping).
function S.Corners(frame)
	local F = FUI()
	if F and frame.foreverPanel then
		Try(F.UpdatePanelCorners, frame)
	end
end

-- The red X of modern panels (Templates.CloseButton), at TOPRIGHT (1, 0) as on Forever UI's bags.
function S.Close(button, frame, x, y)
	local T = Tpl()
	if S.Forever() and T and Try(T.CloseButton, button, frame) then
		button:ClearAllPoints()
		button:SetPoint("TOPRIGHT", frame, "TOPRIGHT", x or 1, y or 0)
		return true
	end
	button:SetSize(28, 28)
	button:ClearAllPoints()
	button:SetPoint("TOPRIGHT", frame, "TOPRIGHT", x or -1, y or -1)
	return false
end

-- ---------------------------------------------------------------- search box (Bags.lua: InputBoxTemplate, the
-- magnifier at LEFT (1, -1), the placeholder at 16, the clear button at RIGHT (-3, 0))
function S.Search(box)
	local parts = {}
	local fancy = S.Forever()
	if box.SetTextInsets then
		box:SetTextInsets(fancy and 16 or 6, 20, 0, 0)
	end
	if fancy then
		local glass = box:CreateTexture(nil, "OVERLAY")
		if S.Atlas(glass, "common-search-magnifyingglass", true) then
			glass:SetWidth(10)
			glass:SetHeight(10)
			glass:SetPoint("LEFT", box, "LEFT", 1, -1)
			parts.glass = glass
		else
			glass:Hide()
			if box.SetTextInsets then
				box:SetTextInsets(6, 20, 0, 0)
			end
			fancy = false
		end
	end
	local hint = box:CreateFontString(nil, "ARTWORK", "GameFontDisableSmall")
	hint:SetPoint("LEFT", box, "LEFT", fancy and 16 or 6, 0)
	hint:SetText(rawget(_G, "SEARCH") or "Search")
	parts.hint = hint
	local clear = CreateFrame("Button", nil, box)
	clear:SetSize(17, 17)
	clear:SetPoint("RIGHT", box, "RIGHT", -3, 0)
	local mark = clear:CreateTexture(nil, "ARTWORK")
	if fancy and S.Atlas(mark, "common-search-clearbutton", true) then
		mark:SetWidth(10)
		mark:SetHeight(10)
		mark:SetPoint("TOPLEFT", clear, "TOPLEFT", 3, -3)
	else
		mark:SetTexture(Path("Interface", "Buttons", "UI-GroupLoot-Pass-Up"))
		mark:SetWidth(12)
		mark:SetHeight(12)
		mark:SetPoint("CENTER", clear, "CENTER", 0, 0)
	end
	mark:SetAlpha(0.5)
	clear:SetScript("OnEnter", function()
		mark:SetAlpha(1)
	end)
	clear:SetScript("OnLeave", function()
		mark:SetAlpha(0.5)
	end)
	clear:Hide()
	parts.clear = clear
	return parts
end

-- ---------------------------------------------------------------- buttons
-- Clean Up: Forever UI's bags-button-autosort atlas, 28 x 26 (Bags.lua applyState); plain: the bag icon.
local function AtlasState(button, setter, getter, name)
	local entry = Entry(name)
	if not entry then
		return false
	end
	if not Try(button[setter], button, entry[1]) then
		return false
	end
	local texture = button[getter] and button[getter](button)
	if not texture or not S.Atlas(texture, name, true) then
		return false
	end
	texture:ClearAllPoints()
	texture:SetAllPoints(button)
	return true
end

local function IconFace(button, icon, crop)
	button:SetNormalTexture(icon)
	button:SetPushedTexture(icon)
	local normal = button.GetNormalTexture and button:GetNormalTexture()
	local pushed = button.GetPushedTexture and button:GetPushedTexture()
	if crop then
		if normal and normal.SetTexCoord then
			normal:SetTexCoord(0.08, 0.92, 0.08, 0.92)
		end
		if pushed and pushed.SetTexCoord then
			pushed:SetTexCoord(0.08, 0.92, 0.08, 0.92)
		end
	end
	if pushed and pushed.SetVertexColor then
		pushed:SetVertexColor(0.6, 0.6, 0.6)
	end
end
S.IconFace = IconFace

local function Hover(button, width, height)
	button:SetHighlightTexture(SQUARE_HOVER, "ADD")
	local glow = button.GetHighlightTexture and button:GetHighlightTexture()
	if glow and width then
		glow:ClearAllPoints()
		glow:SetWidth(width)
		glow:SetHeight(height)
		glow:SetPoint("CENTER", button, "CENTER", 0, 0)
	end
end

function S.SortButton(button)
	button:SetSize(28, 26)
	if S.Forever() and AtlasState(button, "SetNormalTexture", "GetNormalTexture", "bags-button-autosort-up") then
		AtlasState(button, "SetPushedTexture", "GetPushedTexture", "bags-button-autosort-down")
		Hover(button, 24, 23)
		return true
	end
	IconFace(button, Path("Interface", "Icons", "INV_Misc_Bag_10"), true)
	button:SetSize(24, 24)
	Hover(button)
	return false
end

-- A small top-bar button: an atlas when Forever UI has one, else a cropped icon.
function S.SmallButton(button, atlas, icon, size)
	size = size or 20
	button:SetSize(size, size)
	if atlas and S.Forever() and AtlasState(button, "SetNormalTexture", "GetNormalTexture", atlas) then
		local pushed = Entry(atlas) and AtlasState(button, "SetPushedTexture", "GetPushedTexture", atlas)
		if pushed and button.GetPushedTexture then
			local texture = button:GetPushedTexture()
			if texture and texture.SetVertexColor then
				texture:SetVertexColor(0.7, 0.7, 0.7)
			end
		end
		Hover(button, size, size)
		return true
	end
	IconFace(button, icon, true)
	Hover(button)
	return false
end

-- Settings and help buttons: Forever UI's red three-slice button (Templates.ThreeSliceButton, as its Esc menu),
-- else the client's panel button.
function S.TextButton(button)
	local T = Tpl()
	if S.Forever() and T and GameFontHighlightSmall then
		if Try(T.ThreeSliceButton, button, "128-redbutton", { GameFontHighlightSmall, GameFontHighlightSmall, GameFontDisableSmall or GameFontHighlightSmall }) then
			return true
		end
	end
	if button.SetNormalFontObject and GameFontNormalSmall then
		button:SetNormalFontObject(GameFontNormalSmall)
	end
	return false
end

-- A settings tick box: Forever UI's minimal check box, else the client's.
function S.Check(check)
	local T = Tpl()
	if S.Forever() and T and type(T.Art) == "function" and type(T.Place) == "function" then
		local ok, box = Try(T.Art, "checkbox-minimal")
		local ok2, mark = Try(T.Art, "checkmark-minimal")
		if ok and ok2 and type(box) == "table" and type(mark) == "table" then
			local done = Try(function()
				check:SetNormalTexture(box[1])
				T.Place(check:GetNormalTexture(), "checkbox-minimal")
				check:SetPushedTexture(box[1])
				T.Place(check:GetPushedTexture(), "checkbox-minimal")
				check:SetCheckedTexture(mark[1])
				T.Place(check:GetCheckedTexture(), "checkmark-minimal")
				check:SetHighlightTexture(box[1], "ADD")
				local glow = check:GetHighlightTexture()
				T.Place(glow, "checkbox-minimal")
				glow:SetAlpha(0.35)
			end)
			if done then
				return true
			end
		end
	end
	if check.SetNormalTexture then
		check:SetNormalTexture(Path("Interface", "Buttons", "UI-CheckBox-Up"))
		check:SetPushedTexture(Path("Interface", "Buttons", "UI-CheckBox-Down"))
		check:SetHighlightTexture(Path("Interface", "Buttons", "UI-CheckBox-Highlight"), "ADD")
		if check.SetCheckedTexture then
			check:SetCheckedTexture(Path("Interface", "Buttons", "UI-CheckBox-Check"))
		end
	end
	return false
end

-- The settings drawer and the help window: Forever UI's dialog frame (Templates.DialogFrame), else the client's
-- dialog border on the panel colour.
function S.Panel(frame)
	local T = Tpl()
	if S.Forever() and T and Try(T.DialogFrame, frame) then
		return true
	end
	Backdrop(frame, Path("Interface", "DialogFrame", "UI-DialogBox-Border"), 24, 6, S.PANEL[1], S.PANEL[2], S.PANEL[3], 0.97)
	return false
end

-- ---------------------------------------------------------------- purse box (Bags.lua skinPurse: two 8.5 x 17
-- ends and a stretched middle)
function S.Purse(purse)
	if S.Forever() then
		local left = purse:CreateTexture(nil, "BACKGROUND")
		if S.Atlas(left, "common-coinbox-left", true) then
			left:SetWidth(8.5)
			left:SetHeight(17)
			left:SetPoint("LEFT", purse, "LEFT", 0, 0)
			local right = purse:CreateTexture(nil, "BACKGROUND")
			S.Atlas(right, "common-coinbox-right", true)
			right:SetWidth(8.5)
			right:SetHeight(17)
			right:SetPoint("RIGHT", purse, "RIGHT", 0, 0)
			local middle = purse:CreateTexture(nil, "BACKGROUND")
			S.Atlas(middle, "_common-coinbox-center", true)
			middle:SetPoint("TOPLEFT", left, "TOPRIGHT")
			middle:SetPoint("BOTTOMRIGHT", right, "BOTTOMLEFT")
			return true
		end
		left:Hide()
	end
	local bg = purse:CreateTexture(nil, "BACKGROUND")
	bg:SetPoint("TOPLEFT", purse, "TOPLEFT", 0, 2)
	bg:SetPoint("BOTTOMRIGHT", purse, "BOTTOMRIGHT", 0, -2)
	C.Solid(bg, 0, 0, 0, 0.45)
	local line = purse:CreateTexture(nil, "BORDER")
	line:SetHeight(1)
	line:SetPoint("BOTTOMLEFT", bg, "TOPLEFT", 0, 0)
	line:SetPoint("BOTTOMRIGHT", bg, "TOPRIGHT", 0, 0)
	C.Solid(line, 1, 0.82, 0, 0.18)
	return false
end

-- One coin of the money line: the client's coin icons (UI-MoneyIcons, as its own money frames).
local COIN_U = { gold = 0, silver = 0.25, copper = 0.5 }
function S.Coin(texture, which)
	texture:SetTexture(MONEY_ICONS)
	local u = COIN_U[which] or 0
	if texture.SetTexCoord then
		texture:SetTexCoord(u, u + 0.25, 0, 1)
	end
	texture:SetWidth(12)
	texture:SetHeight(12)
end

-- ---------------------------------------------------------------- item slots (Bags.lua skinButton: the client's
-- slot art hidden, the icon filling the button, a grey WhiteIconFrame outline tinted by quality, a black search veil)
function S.Slot(button, name)
	local parts = {}
	local T = Tpl()
	local fancy = S.Forever() and T and type(T.Outline) == "function"
	if fancy then
		local ok, outline = Try(T.Outline, button)
		if ok and outline then
			parts.outline = outline
			outline:SetVertexColor(S.FRAME_GRAY, S.FRAME_GRAY, S.FRAME_GRAY)
			outline:Show()
			local normal = button.GetNormalTexture and button:GetNormalTexture()
			if normal and normal.SetAlpha then
				normal:SetAlpha(0)
			end
			local icon = name and _G[name .. "IconTexture"]
			if icon then
				icon:ClearAllPoints()
				icon:SetAllPoints(button)
				if icon.SetTexCoord then
					icon:SetTexCoord(0, 1, 0, 1)
				end
			end
		else
			fancy = false
		end
	end
	parts.forever = fancy and true or false
	if not fancy then
		local border = button:CreateTexture(nil, "OVERLAY")
		border:SetTexture(Path("Interface", "Buttons", "UI-ActionButton-Border"))
		border:SetBlendMode("ADD")
		border:SetPoint("CENTER", button, "CENTER", 0, 0)
		border:SetSize(Logic.SLOT * 1.8, Logic.SLOT * 1.8)
		border:SetAlpha(0.7)
		border:Hide()
		parts.glow = border
	end
	local fresh = button:CreateTexture(nil, "OVERLAY")
	if not (fancy and S.Atlas(fresh, "bags-newitem", true)) then
		fresh:SetTexture(Path("Interface", "Buttons", "CheckButtonHilight"))
		fresh:SetBlendMode("ADD")
	end
	fresh:SetAllPoints(button)
	fresh:Hide()
	parts.fresh = fresh
	local veil = button:CreateTexture(nil, "OVERLAY")
	C.Solid(veil, 0, 0, 0, 0.8)
	veil:SetAllPoints(button)
	veil:Hide()
	parts.veil = veil
	return parts
end

-- Quality on a slot: Forever UI's outline (grey, coloured from uncommon), else the glow from uncommon.
function S.Quality(button, quality)
	local outline = rawget(button, "lbOutline")
	if outline then
		local color = type(quality) == "number" and quality >= 2 and rawget(_G, "ITEM_QUALITY_COLORS") and ITEM_QUALITY_COLORS[quality]
		if color then
			outline:SetVertexColor(color.r, color.g, color.b)
		else
			outline:SetVertexColor(S.FRAME_GRAY, S.FRAME_GRAY, S.FRAME_GRAY)
		end
		outline:Show()
		return
	end
	local glow = rawget(button, "QualityBorder")
	if type(glow) ~= "table" then
		return
	end
	if type(quality) ~= "number" or quality < 2 then
		glow:Hide()
		return
	end
	local r, g, b = C.Call("GetItemQualityColor", quality)
	if type(r) ~= "number" then
		glow:Hide()
		return
	end
	glow:SetVertexColor(r, g, b)
	glow:Show()
end

-- A bag slot of the bag row: Forever UI's bag-border ring (bag-border-empty when no bag), else the client's slot.
function S.BagSlot(button)
	local parts = {}
	local icon = button:CreateTexture(nil, "BORDER")
	icon:SetPoint("TOPLEFT", button, "TOPLEFT", 3, -3)
	icon:SetPoint("BOTTOMRIGHT", button, "BOTTOMRIGHT", -3, 3)
	parts.icon = icon
	local ring = button:CreateTexture(nil, "OVERLAY")
	ring:SetAllPoints(button)
	if S.Forever() and S.Atlas(ring, "bag-border", true) then
		parts.ringAtlas = true
	else
		ring:SetTexture(Path("Interface", "Buttons", "UI-Quickslot2"))
		ring:ClearAllPoints()
		ring:SetPoint("CENTER", button, "CENTER", 0, 0)
		ring:SetWidth(button:GetWidth() * 1.72)
		ring:SetHeight(button:GetHeight() * 1.72)
	end
	parts.ring = ring
	Hover(button)
	return parts
end

function S.BagRing(parts, filled)
	if parts and parts.ringAtlas then
		S.Atlas(parts.ring, filled and "bag-border" or "bag-border-empty", true)
	end
end

-- The thin gold rule of the new-loot strip's label.
function S.Rule(texture)
	C.Solid(texture, 1, 0.82, 0, 0.25)
end
