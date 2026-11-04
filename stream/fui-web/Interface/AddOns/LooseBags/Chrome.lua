-- Loose Bags by Eric R (MIT, see LICENSE.txt), ported to the 3.3.5a client for Leyline.
-- The window. 3.3.5a has no portrait frame template, bag search box or atlases: the frame is drawn here from
-- the client's own backdrop, icon and button art, in the same places as the original.
local Logic = LooseBagsLogic
local LB = Logic.LB
local C = LB.Compat

local function HideTip()
	if GameTooltip_Hide then
		GameTooltip_Hide()
	elseif GameTooltip then
		GameTooltip:Hide()
	end
end

local function Tip(owner, title, body)
	owner:SetScript("OnEnter", function(self)
		if not GameTooltip then
			return
		end
		GameTooltip:SetOwner(self, "ANCHOR_RIGHT")
		GameTooltip:SetText(title, 1, 1, 1)
		if body and GameTooltip.AddLine then
			GameTooltip:AddLine(body, 0.9, 0.9, 0.9, true)
		end
		GameTooltip:Show()
	end)
	owner:SetScript("OnLeave", HideTip)
end

local function Title()
	return Logic.FRAME_TITLE
end

local function SavePoint(frame)
	local point, _, relPoint, x, y = frame:GetPoint(1)
	local char = LB.Char()
	local plainX = LB.Plain(x)
	local plainY = LB.Plain(y)
	Logic.Apply(char, {
		op = "move_frame",
		window = 1,
		x = plainX,
		y = plainY,
		point = type(point) == "string" and point or nil,
		relPoint = type(relPoint) == "string" and relPoint or nil,
	})
end

local function StartMove(frame)
	frame:StartMoving()
end

local function StopMove(frame)
	frame:StopMovingOrSizing()
	SavePoint(frame)
end

local function MoveHandle(frame)
	local handle = CreateFrame("Button", nil, frame)
	handle:RegisterForDrag("LeftButton")
	handle:SetScript("OnDragStart", function()
		StartMove(frame)
	end)
	handle:SetScript("OnDragStop", function()
		StopMove(frame)
	end)
	return handle
end

local function FitMoney(money)
	if not money or not money.GetWidth or not money.GetLeft then
		return
	end
	local frameWidth = LB.Plain(money:GetWidth())
	local frameLeft = LB.Plain(money:GetLeft())
	if not frameWidth or not frameLeft then
		return
	end
	local name = money.GetName and money:GetName()
	local buttons = { money.GoldButton, money.SilverButton, money.CopperButton }
	if name then
		buttons = { _G[name .. "GoldButton"], _G[name .. "SilverButton"], _G[name .. "CopperButton"] }
	end
	local contentLeft
	for index = 1, #buttons do
		local button = buttons[index]
		if button and button.IsShown and button:IsShown() and button.GetLeft then
			local left = LB.Plain(button:GetLeft())
			if left then
				local inset = left - frameLeft
				if not contentLeft or inset < contentLeft then
					contentLeft = inset
				end
			end
		end
	end
	if not contentLeft then
		return
	end
	local width = Logic.MoneyBoxWidth(frameWidth, contentLeft)
	if width and width ~= frameWidth then
		money:SetWidth(width)
	end
end

local function EnsureGlow(button)
	if button.looseGlow then
		return button.looseGlow
	end
	local glow = button:CreateTexture(nil, "ARTWORK")
	glow:SetAllPoints(button)
	glow:SetTexture("Interface\\Buttons\\ButtonHilight-Square")
	glow:SetBlendMode("ADD")
	glow:SetAlpha(0.45)
	glow:Hide()
	button.looseGlow = glow
	return glow
end

local function PaintToggle(button, on)
	if not button or not button.SetButtonState then
		return
	end
	local state, _, showGlow = Logic.TogglePaint(on)
	button:SetButtonState(state, state == "PUSHED")
	Logic.PaintToggleFont(button, on)
	if not button.CreateTexture then
		return
	end
	local glow = EnsureGlow(button)
	if showGlow then
		glow:Show()
	else
		glow:Hide()
	end
end

function LB.RefreshSettings()
	local frame = LB.frame
	if not frame then
		return
	end
	local char = LB.Char()
	PaintToggle(frame.AutoPlace, char.autoPlace)
	PaintToggle(frame.AutoOrder, char.autoOrder)
	PaintToggle(frame.SameType, char.sameTypeSnap)
	PaintToggle(frame.Zones, char.useZones)
	local closed = char.collapsed
	if closed then
		frame.Collapse:SetNormalTexture("Interface\\Buttons\\UI-PlusButton-Up")
		frame.Collapse:SetPushedTexture("Interface\\Buttons\\UI-PlusButton-Down")
	else
		frame.Collapse:SetNormalTexture("Interface\\Buttons\\UI-MinusButton-Up")
		frame.Collapse:SetPushedTexture("Interface\\Buttons\\UI-MinusButton-Down")
	end
	if closed then
		frame.Settings:Hide()
	else
		frame.Settings:Show()
	end
end

local function TextBlockHeight(fontString)
	local h
	if fontString and fontString.GetStringHeight then
		h = fontString:GetStringHeight()
	end
	if type(issecretvalue) == "function" and issecretvalue(h) then
		return nil
	end
	if type(h) ~= "number" or h < 1 then
		return nil
	end
	return h
end

local function HelpViewLimit()
	local h
	if UIParent and UIParent.GetHeight then
		h = UIParent:GetHeight()
	end
	if type(issecretvalue) == "function" and issecretvalue(h) then
		return nil
	end
	if type(h) ~= "number" or h < 200 then
		return nil
	end
	return h - 80
end

local function LayoutHelp(frame, deferred)
	local rows = Logic.HELP or {}
	local content = frame.helpContent
	local lines = frame.helpLines
	local heights = {}
	for index = 1, #rows do
		local spec = rows[index]
		local line = lines[index]
		if not line then
			line = {
				gesture = content:CreateFontString(nil, "OVERLAY", "GameFontNormal"),
				detail = content:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall"),
			}
			line.gesture:SetJustifyH("LEFT")
			line.detail:SetJustifyH("LEFT")
			if line.gesture.SetJustifyV then
				line.gesture:SetJustifyV("TOP")
			end
			if line.detail.SetJustifyV then
				line.detail:SetJustifyV("TOP")
			end
			lines[index] = line
		end
		if line.gesture.Show then
			line.gesture:Show()
			line.detail:Show()
		end
		if line.gesture.SetWidth then
			line.gesture:SetWidth(180)
		end
		if line.detail.SetWidth then
			line.detail:SetWidth(410)
		end
		line.gesture:SetText(spec.gesture)
		line.detail:SetText(spec.detail)
		heights[index] = {
			gesture = TextBlockHeight(line.gesture),
			detail = TextBlockHeight(line.detail),
		}
	end
	for index = #rows + 1, #lines do
		local extra = lines[index]
		if extra.gesture.Hide then
			extra.gesture:Hide()
			extra.detail:Hide()
		end
	end
	local layout = Logic.HelpLayout(heights, HelpViewLimit())
	for index = 1, #rows do
		local line = lines[index]
		local row = layout.rows[index]
		if line.gesture.SetHeight then
			line.gesture:SetHeight(row.height)
		end
		if line.detail.SetHeight then
			line.detail:SetHeight(row.height)
		end
		line.gesture:ClearAllPoints()
		line.detail:ClearAllPoints()
		line.gesture:SetPoint("TOPLEFT", content, "TOPLEFT", 16, -row.y)
		line.detail:SetPoint("TOPLEFT", content, "TOPLEFT", 210, -row.y)
	end
	content:SetHeight(layout.content)
	frame:SetSize(640, layout.height)
	frame.helpScrollMax = layout.maxScroll
	local offset = frame.helpOffset or 0
	if offset < 0 then
		offset = 0
	end
	if offset > layout.maxScroll then
		offset = layout.maxScroll
	end
	frame.helpOffset = offset
	local scroll = frame.helpScroll
	scroll:SetSize(624, layout.visible)
	if scroll.SetVerticalScroll then
		scroll:SetVerticalScroll(offset)
	end
	if scroll.EnableMouseWheel then
		scroll:EnableMouseWheel(layout.maxScroll > 0)
	end
	if not deferred then
		C.After(0, function()
			if frame:IsShown() then
				LayoutHelp(frame, true)
			end
		end)
	end
end

local function AttachHelpChrome(frame)
	frame:SetMovable(true)
	frame:SetClampedToScreen(true)
	-- Esc closes it (3.3.5a frames cannot pass keys on, so it goes on the close-with-Esc list instead).
	C.Special("LooseBagsHelp")
	local drag = CreateFrame("Button", nil, frame)
	drag:SetPoint("TOPLEFT", frame, "TOPLEFT", 0, 0)
	drag:SetPoint("TOPRIGHT", frame, "TOPRIGHT", -28, 0)
	drag:SetHeight(28)
	drag:RegisterForDrag("LeftButton")
	drag:SetScript("OnDragStart", function()
		frame:StartMoving()
	end)
	drag:SetScript("OnDragStop", function()
		frame:StopMovingOrSizing()
	end)
	local close = C.Make("Button", "LooseBagsHelpClose", frame, "UIPanelCloseButton")
	close:SetSize(24, 24)
	close:SetPoint("TOPRIGHT", frame, "TOPRIGHT", -2, -2)
	close:SetScript("OnClick", function()
		frame:Hide()
	end)
	local catcher = CreateFrame("Button", nil, UIParent)
	catcher:SetAllPoints(UIParent)
	catcher:SetFrameStrata("DIALOG")
	catcher:EnableMouse(true)
	catcher:SetScript("OnClick", function()
		frame:Hide()
	end)
	catcher:Hide()
	frame:HookScript("OnShow", function()
		catcher:SetFrameLevel(1)
		frame:SetFrameLevel(20)
		catcher:Show()
	end)
	frame:HookScript("OnHide", function()
		catcher:Hide()
	end)
end

local function EnsureHelpFrame()
	if LB.helpFrame then
		return LB.helpFrame
	end
	local frame = CreateFrame("Frame", "LooseBagsHelp", UIParent)
	frame:SetSize(640, 48)
	frame:SetPoint("CENTER", UIParent, "CENTER", 0, 0)
	frame:SetFrameStrata("DIALOG")
	frame:EnableMouse(true)
	local bg = frame:CreateTexture(nil, "BACKGROUND")
	bg:SetAllPoints(frame)
	C.Solid(bg, 0.06, 0.05, 0.04, 0.94)
	if frame.SetBackdrop then
		frame:SetBackdrop({ edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border", edgeSize = 16 })
	end
	local title = frame:CreateFontString(nil, "OVERLAY", "GameFontNormal")
	title:SetPoint("TOPLEFT", frame, "TOPLEFT", 16, -14)
	title:SetText("Help")
	local scroll = CreateFrame("ScrollFrame", nil, frame)
	scroll:SetPoint("TOPLEFT", frame, "TOPLEFT", 8, -36)
	scroll:EnableMouse(true)
	local content = CreateFrame("Frame", nil, scroll)
	content:SetSize(624, 1)
	if scroll.SetScrollChild then
		scroll:SetScrollChild(content)
	end
	frame.helpScroll = scroll
	frame.helpContent = content
	frame.helpLines = {}
	scroll:SetScript("OnMouseWheel", function(_, delta)
		local maxScroll = frame.helpScrollMax or 0
		if maxScroll <= 0 or type(delta) ~= "number" then
			return
		end
		local offset = (frame.helpOffset or 0) - delta * 48
		if offset < 0 then
			offset = 0
		end
		if offset > maxScroll then
			offset = maxScroll
		end
		frame.helpOffset = offset
		LayoutHelp(frame, true)
	end)
	frame:SetScript("OnShow", function(self)
		LayoutHelp(self, false)
	end)
	AttachHelpChrome(frame)
	LayoutHelp(frame, true)
	frame:Hide()
	LB.helpFrame = frame
	return frame
end

function LB.OpenHelp()
	EnsureHelpFrame():Show()
end

function LB.ToggleHelp()
	local frame = EnsureHelpFrame()
	if frame:IsShown() then
		frame:Hide()
		return
	end
	LB.OpenHelp()
end

function LB.RefreshLock()
	local frame = LB.frame
	if not frame or not frame.LockButton then
		return
	end
	local char = LB.Char()
	local up, down = Logic.LockTextures(char.locked)
	frame.LockButton:SetNormalTexture(up)
	frame.LockButton:SetPushedTexture(down)
	if frame.Grip then
		if char.locked then
			frame.Grip:Hide()
		else
			frame.Grip:Show()
		end
	end
	frame:SetResizable(not char.locked)
end

local BACKDROP = {
	bgFile = "Interface\\Tooltips\\UI-Tooltip-Background",
	edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border",
	tile = true,
	tileSize = 16,
	edgeSize = 16,
	insets = { left = 4, right = 4, top = 4, bottom = 4 },
}

local function IconButton(parent, width, height, texture, crop)
	local button = CreateFrame("Button", nil, parent)
	button:SetSize(width, height)
	button:SetNormalTexture(texture)
	button:SetPushedTexture(texture)
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
	button:SetHighlightTexture("Interface\\Buttons\\ButtonHilight-Square", "ADD")
	return button
end

function LB.Ensure()
	if LB.frame then
		return LB.frame
	end
	local frame = CreateFrame("Frame", "LooseBagsFrame", UIParent)
	frame:SetSize(420, 320)
	frame:SetFrameStrata("MEDIUM")
	frame:SetMovable(true)
	frame:SetResizable(true)
	frame:SetClampedToScreen(true)
	frame:SetToplevel(true)
	frame:EnableMouse(true)
	if frame.SetBackdrop then
		frame:SetBackdrop(BACKDROP)
		frame:SetBackdropColor(0.04, 0.04, 0.04, 0.94)
		frame:SetBackdropBorderColor(0.62, 0.62, 0.62, 1)
	end
	frame:RegisterForDrag("LeftButton")
	frame:SetScript("OnDragStart", function(self)
		if LB.Char().locked then
			return
		end
		StartMove(self)
	end)
	frame:SetScript("OnDragStop", function(self)
		StopMove(self)
	end)
	frame:SetScript("OnSizeChanged", function(self, width, height)
		if LB.inLayout then
			return
		end
		local plainW = LB.Plain(width)
		local plainH = LB.Plain(height)
		if not plainW or not plainH then
			return
		end
		local char = LB.Char()
		if char.locked then
			return
		end
		char.width = plainW
		char.height = plainH
		LB.Layout()
	end)
	-- Esc closes the bag, as it does the standard ones.
	C.Special("LooseBagsFrame")
	frame:SetScript("OnShow", function()
		if LB.SyncBagButtons then
			LB.SyncBagButtons()
		end
	end)
	frame:SetScript("OnHide", function()
		if LB.SyncBagButtons then
			LB.SyncBagButtons()
		end
	end)

	local portrait = frame:CreateTexture("LooseBagsFramePortrait", "ARTWORK")
	portrait:SetSize(50, 50)
	portrait:SetPoint("TOPLEFT", frame, "TOPLEFT", 6, -6)
	if type(SetPortraitToTexture) == "function" then
		SetPortraitToTexture(portrait, "Interface\\Icons\\INV_Misc_Bag_08")
	else
		portrait:SetTexture("Interface\\Icons\\INV_Misc_Bag_08")
	end
	frame.Portrait = portrait
	local title = frame:CreateFontString(nil, "OVERLAY", "GameFontNormal")
	title:SetPoint("TOP", frame, "TOP", 10, -12)
	title:SetText(Title())
	frame.TitleText = title
	local close = C.Make("Button", "LooseBagsFrameClose", frame, "UIPanelCloseButton")
	close:SetSize(30, 30)
	close:SetPoint("TOPRIGHT", frame, "TOPRIGHT", 0, 0)
	close:SetScript("OnClick", function()
		LB.Hide()
	end)
	frame.CloseButton = close
	MoveHandle(frame):SetAllPoints(portrait)
	local titleDrag = MoveHandle(frame)
	titleDrag:SetPoint("TOPLEFT", frame, "TOPLEFT", 58, 0)
	titleDrag:SetPoint("BOTTOMRIGHT", close, "BOTTOMLEFT", 0, 4)

	local char = LB.Char()
	frame:ClearAllPoints()
	frame:SetPoint(char.point, UIParent, char.relPoint, char.x, char.y)

	local sort = IconButton(frame, 26, 26, "Interface\\Icons\\INV_Misc_Bag_10", true)
	sort:SetPoint("TOPRIGHT", frame, "TOPRIGHT", -9, -34)
	sort:SetScript("OnClick", function()
		C.Sound("igMainMenuOptionCheckBoxOn")
		LB.Sort()
	end)
	Tip(sort, "Clean Up Bags", "Sort the bag. Zones grows each category from the corner or edge nearest its tag.")
	frame.SortButton = sort

	local collapse = CreateFrame("Button", nil, frame)
	collapse:SetSize(16, 16)
	collapse:SetPoint("RIGHT", sort, "LEFT", -6, 0)
	collapse:SetHighlightTexture("Interface\\Buttons\\UI-PlusButton-Hilight", "ADD")
	collapse:SetScript("OnClick", function()
		local row = LB.Char()
		row.collapsed = not row.collapsed
		LB.RefreshSettings()
		LB.Layout()
	end)
	Tip(collapse, "Settings", "Show or hide the settings row.")
	frame.Collapse = collapse

	local search, templated = C.Make("EditBox", "LooseBagsSearchBox", frame, "InputBoxTemplate")
	if not templated then
		if search.SetFontObject and ChatFontNormal then
			search:SetFontObject(ChatFontNormal)
		end
		if search.SetBackdrop then
			search:SetBackdrop(BACKDROP)
			search:SetBackdropColor(0, 0, 0, 0.6)
		end
		if search.SetTextInsets then
			search:SetTextInsets(6, 6, 0, 0)
		end
		search:SetScript("OnEscapePressed", function(self)
			self:ClearFocus()
		end)
	end
	search:SetAutoFocus(false)
	search:SetPoint("TOPLEFT", frame, "TOPLEFT", 68, -37)
	search:SetPoint("RIGHT", collapse, "LEFT", -8, 0)
	search:SetHeight(20)
	local hint = search:CreateFontString(nil, "ARTWORK", "GameFontDisableSmall")
	hint:SetPoint("LEFT", search, "LEFT", templated and 2 or 8, 0)
	hint:SetText("Search")
	frame.SearchHint = hint
	local function ShowHint(self)
		local text = self:GetText() or ""
		local focused = self.HasFocus and self:HasFocus()
		C.SetShown(hint, text == "" and not focused)
	end
	local function Hook(widget, script, fn)
		if widget.HookScript and widget:GetScript(script) then
			widget:HookScript(script, fn)
		else
			widget:SetScript(script, fn)
		end
	end
	Hook(search, "OnEditFocusGained", function(self)
		hint:Hide()
	end)
	Hook(search, "OnEditFocusLost", ShowHint)
	local previousSearch = search:GetScript("OnTextChanged")
	search:SetScript("OnTextChanged", function(self, userInput)
		if previousSearch then
			previousSearch(self, userInput)
		end
		local text = self:GetText() or ""
		frame.searchText = string.lower(text)
		ShowHint(self)
		if LB.ApplySearch then
			LB.ApplySearch()
		end
	end)
	frame.Search = search
	frame.searchText = ""

	local settings = CreateFrame("Frame", nil, frame)
	settings:Hide()
	settings:SetHeight(32)
	settings:SetPoint("TOPLEFT", frame, "TOPLEFT", Logic.SETTINGS_LEFT, -60)
	settings:SetPoint("TOPRIGHT", frame, "TOPRIGHT", -Logic.SETTINGS_RIGHT, -60)
	local function PanelButton(name, text, width)
		local button, made = C.Make("Button", name, settings, "UIPanelButtonTemplate")
		if not made and button.SetNormalFontObject and GameFontNormal then
			button:SetNormalFontObject(GameFontNormal)
		end
		button:SetSize(width, 22)
		button:SetText(text)
		return button
	end
	local function MakeToggle(text, width, anchor, point, gap, key, title, body, onTurnOn)
		local button = PanelButton("LooseBagsToggle" .. key, text, width)
		button:SetPoint("LEFT", anchor, point, gap, 0)
		button:SetScript("OnClick", function()
			local char = LB.Char()
			char[key] = not char[key]
			if char[key] and onTurnOn then
				onTurnOn()
			end
			LB.RefreshSettings()
		end)
		Tip(button, title, body)
		return button
	end
	local auto = MakeToggle("Auto-place", 88, settings, "LEFT", 0, "autoPlace", "Auto-place", "Put new items with their category. White means on.", function()
		if LB.FileHeld then
			LB.FileHeld()
		end
	end)
	local order = MakeToggle("Auto-order", 88, auto, "RIGHT", 4, "autoOrder", "Auto-order", "On, Clean Up sorts inside each category. Off, it keeps each group's shape and order. White means on.")
	local same = MakeToggle("Same-type", 78, order, "RIGHT", 4, "sameTypeSnap", "Same-type snap", "Snap and group only with the same kind of item. White means on.")
	local zones = MakeToggle("Zones", 58, same, "RIGHT", 4, "useZones", "Zones", "Categories grow from the corner or edge nearest each tag. White means on.")
	local gear = IconButton(settings, 16, 16, "Interface\\Icons\\INV_Misc_Gear_01", true)
	gear:SetPoint("LEFT", zones, "RIGHT", 0, 0)
	gear:SetScript("OnClick", function()
		LB.zoneEdit = not LB.zoneEdit
		if LB.Layout then
			LB.Layout()
		end
	end)
	Tip(gear, "Edit zones")
	local help = PanelButton("LooseBagsHelpButton", "?", 32)
	help:SetPoint("LEFT", gear, "RIGHT", 6, 0)
	help:SetScript("OnClick", function()
		LB.ToggleHelp()
	end)
	Tip(help, "Help", "How to use Loose Bags.")
	frame.AutoPlace = auto
	frame.AutoOrder = order
	frame.SameType = same
	frame.Zones = zones
	frame.ZoneGear = gear
	frame.HelpButton = help
	frame.Settings = settings

	local function DropHere()
		if CursorHasItem and CursorHasItem() then
			LB.DropCursorInEmpty()
		end
	end
	local content = CreateFrame("Frame", nil, frame)
	content:EnableMouse(true)
	content:SetScript("OnMouseUp", DropHere)
	content:SetScript("OnReceiveDrag", DropHere)
	frame.Content = content

	local strip = CreateFrame("Frame", nil, frame)
	strip:EnableMouse(true)
	strip:SetScript("OnReceiveDrag", DropHere)
	frame.Strip = strip

	local grip = CreateFrame("Button", nil, frame)
	grip:SetSize(16, 16)
	grip:SetPoint("BOTTOMRIGHT", frame, "BOTTOMRIGHT", -2, Logic.GRIP_BOTTOM)
	grip:SetNormalTexture("Interface\\ChatFrame\\UI-ChatIM-SizeGrabber-Up")
	grip:SetHighlightTexture("Interface\\ChatFrame\\UI-ChatIM-SizeGrabber-Highlight")
	grip:SetPushedTexture("Interface\\ChatFrame\\UI-ChatIM-SizeGrabber-Down")
	grip:SetScript("OnMouseDown", function(self)
		if LB.Char().locked then
			return
		end
		if self.SetButtonState then
			self:SetButtonState("PUSHED", true)
		end
		local highlight = self.GetHighlightTexture and self:GetHighlightTexture()
		if highlight then
			highlight:Hide()
		end
		frame:StartSizing("BOTTOMRIGHT")
	end)
	grip:SetScript("OnMouseUp", function(self)
		if self.SetButtonState then
			self:SetButtonState("NORMAL", false)
		end
		local highlight = self.GetHighlightTexture and self:GetHighlightTexture()
		if highlight then
			highlight:Show()
		end
		frame:StopMovingOrSizing()
		local width = LB.Plain(frame:GetWidth())
		local height = LB.Plain(frame:GetHeight())
		if width and height and not LB.Char().locked then
			local char = LB.Char()
			char.width = width
			char.height = height
			LB.Layout()
		end
	end)
	frame.Grip = grip

	local lock = CreateFrame("Button", nil, frame)
	lock:SetSize(20, 20)
	lock:SetPoint("BOTTOMRIGHT", grip, "BOTTOMLEFT", -2, 0)
	lock:SetHighlightTexture("Interface\\Buttons\\UI-Common-MouseHilight", "ADD")
	lock:SetScript("OnClick", function()
		local char = LB.Char()
		char.locked = not char.locked
		LB.RefreshLock()
	end)
	lock:SetScript("OnEnter", function(self)
		if not GameTooltip then
			return
		end
		local char = LB.Char()
		GameTooltip:SetOwner(self, "ANCHOR_RIGHT")
		if char.locked then
			GameTooltip:SetText("Locked", 1, 1, 1)
			if GameTooltip.AddLine then
				GameTooltip:AddLine("The bag keeps this size. Drag the title or the logo to move it.", 0.9, 0.9, 0.9, true)
			end
		else
			GameTooltip:SetText("Unlocked", 1, 1, 1)
			if GameTooltip.AddLine then
				GameTooltip:AddLine("Drag the bag to move it. Drag the corner to resize it.", 0.9, 0.9, 0.9, true)
			end
		end
		GameTooltip:Show()
	end)
	lock:SetScript("OnLeave", HideTip)
	frame.LockButton = lock

	-- The standard backpack's money line (SmallMoneyFrameTemplate, the player's own money).
	local money, moneyMade = C.Make("Frame", "LooseBagsMoneyFrame", frame, "SmallMoneyFrameTemplate")
	money:SetPoint("BOTTOMRIGHT", lock, "BOTTOMLEFT", -6, Logic.COIN_LIFT)
	if moneyMade then
		C.Call("MoneyFrame_SetType", money, "PLAYER")
		C.Call("MoneyFrame_UpdateMoney", money)
	end
	frame.Money = money
	if hooksecurefunc and type(MoneyFrame_Update) == "function" then
		hooksecurefunc("MoneyFrame_Update", function(frameName)
			local updated = frameName
			if type(frameName) == "string" then
				updated = _G[frameName]
			end
			if updated == money then
				FitMoney(money)
			end
		end)
	end

	LB.frame = frame
	LB.buttons = {}
	LB.RefreshSettings()
	LB.RefreshLock()
	frame:Hide()
	return frame
end

function LB.Show()
	local frame = LB.Ensure()
	LB.ApplyFirstRun("open")
	local wasShown = frame:IsShown()
	frame:Show()
	LB.Refresh()
	if LB.CloseStock then
		LB.CloseStock()
	end
	if not wasShown then
		C.Sound("igBackPackOpen")
	end
end

local function ZoneScale(content)
	local scale = 1
	if content and content.GetEffectiveScale then
		scale = content:GetEffectiveScale() or 1
	end
	if type(issecretvalue) == "function" and issecretvalue(scale) then
		scale = 1
	end
	if type(scale) ~= "number" or scale == 0 then
		scale = 1
	end
	return scale
end

local function ZonePoint(host)
	if not GetCursorPosition or not host then
		return nil
	end
	local x, y = GetCursorPosition()
	if type(issecretvalue) == "function" and (issecretvalue(x) or issecretvalue(y)) then
		return nil
	end
	if type(x) ~= "number" or type(y) ~= "number" then
		return nil
	end
	local left = host.GetLeft and host:GetLeft()
	local top = host.GetTop and host:GetTop()
	if type(issecretvalue) == "function" and (issecretvalue(left) or issecretvalue(top)) then
		return nil
	end
	return Logic.CursorLocal(x, y, ZoneScale(host), left, top)
end

local function ZonePlain(value)
	if type(issecretvalue) == "function" and issecretvalue(value) then
		return nil
	end
	if type(value) ~= "number" then
		return nil
	end
	return value
end

local function ZoneOthers(tag)
	local others = {}
	local tags = LB.frame and LB.frame.zoneTags or {}
	for index = 1, #tags do
		local other = tags[index]
		if other ~= tag and type(other.anchorX) == "number" then
			others[#others + 1] = { x = other.anchorX, y = other.anchorY }
		end
	end
	return others
end

local function CategoryCount(category)
	local char = LB.Char()
	local places = char.places or {}
	local count = 0
	for index = 1, #places do
		local place = places[index]
		if not place.held and Logic.Category(place.classID, place.quality, place.isQuest) == category then
			count = count + 1
		end
	end
	return count
end

local function EnsurePreview(tag, content)
	if tag.preview then
		return tag.preview
	end
	local preview = CreateFrame("Frame", nil, content)
	preview:EnableMouse(false)
	preview:SetFrameStrata("FULLSCREEN_DIALOG")
	local fill = preview:CreateTexture(nil, "BACKGROUND")
	fill:SetAllPoints()
	fill:SetTexture("Interface\\Buttons\\ButtonHilight-Square")
	fill:SetBlendMode("ADD")
	fill:SetAlpha(0.28)
	preview:Hide()
	tag.preview = preview
	return preview
end

local function MovePreview(tag, anchorX, anchorY, content)
	local preview = EnsurePreview(tag, content)
	local contentW = ZonePlain(content:GetWidth())
	local contentH = ZonePlain(content:GetHeight())
	if not contentW or not contentH or type(anchorX) ~= "number" or type(anchorY) ~= "number" then
		preview:Hide()
		return
	end
	local left, top, width, height = Logic.ZoneFootprint(CategoryCount(tag.category), anchorX, anchorY, contentW, contentH)
	preview:ClearAllPoints()
	preview:SetPoint("TOPLEFT", content, "TOPLEFT", left, -top)
	preview:SetSize(width, height)
	preview:Show()
end

local function HidePreview(tag)
	if tag and tag.preview then
		tag.preview:Hide()
	end
	if tag then
		tag:SetAlpha(1)
	end
end

local function SnapZoneDrag(tag, drag, content)
	local localX, localY = ZonePoint(content)
	if not localX then
		return nil
	end
	local left, top = Logic.GrabbedPoint(localX, localY, drag.grabX, drag.grabY)
	local contentW = ZonePlain(content:GetWidth())
	local contentH = ZonePlain(content:GetHeight())
	return Logic.SnapFootprint(CategoryCount(tag.category), left, top, ZoneOthers(tag), contentW, contentH)
end

function LB.FollowZoneDrag(tag)
	local drag = LB.zoneDrag
	local content = LB.frame and LB.frame.Content
	if not drag or drag.tag ~= tag or not content then
		return
	end
	local snapX, snapY = SnapZoneDrag(tag, drag, content)
	if snapX then
		MovePreview(tag, snapX, snapY, content)
	end
end

function LB.BeginZoneDrag(tag)
	local content = LB.frame and LB.frame.Content
	local localX, localY = ZonePoint(content)
	local contentW = content and ZonePlain(content:GetWidth())
	local contentH = content and ZonePlain(content:GetHeight())
	if not localX or not contentW or not contentH then
		return
	end
	local left, top = Logic.ZoneFootprint(CategoryCount(tag.category), tag.anchorX, tag.anchorY, contentW, contentH)
	LB.zoneDrag = {
		tag = tag,
		grabX = localX - left,
		grabY = localY - top,
	}
	tag:SetAlpha(0)
	MovePreview(tag, tag.anchorX, tag.anchorY, content)
	tag:SetScript("OnUpdate", function(self)
		LB.FollowZoneDrag(self)
	end)
end

function LB.FinishZoneDrag(tag)
	tag:SetScript("OnUpdate", nil)
	HidePreview(tag)
	local drag = LB.zoneDrag
	LB.zoneDrag = nil
	local content = LB.frame and LB.frame.Content
	if not drag or not content then
		return
	end
	local x, y = SnapZoneDrag(tag, drag, content)
	if not x then
		return
	end
	local char = LB.Char()
	char.homes = Logic.SetHome(char.homes, tag.category, x, y)
	tag.anchorX = x
	tag.anchorY = y
	tag:ClearAllPoints()
	tag:SetPoint("CENTER", content, "TOPLEFT", x, -y)
end

function LB.ShowZoneTags()
	local frame = LB.frame
	if not frame or not frame.Content then
		return
	end
	local content = frame.Content
	local char = LB.Char()
	local shown
	char.homes, shown = Logic.ClampHomes(char.homes, ZonePlain(content:GetWidth()), ZonePlain(content:GetHeight()))
	frame.zoneTags = frame.zoneTags or {}
	for index = 1, #Logic.CATEGORIES do
		local category = Logic.CATEGORIES[index]
		local tag = frame.zoneTags[index]
		if tag and not tag.zoneFace then
			tag:Hide()
			tag = nil
		end
		if not tag then
			tag = CreateFrame("Button", nil, content)
			tag:SetSize(Logic.TAG_W, Logic.TAG_H)
			tag:SetScript("OnClick", nil)
			tag:RegisterForDrag("LeftButton")
			tag:SetScript("OnDragStart", function(self)
				LB.BeginZoneDrag(self)
			end)
			tag:SetScript("OnDragStop", function(self)
				LB.FinishZoneDrag(self)
			end)
			local function face(left, right, stick)
				local texture = tag:CreateTexture(nil, "BACKGROUND")
				texture:SetTexture("Interface\\Buttons\\UI-Panel-Button-Up")
				texture:SetTexCoord(left, right, 0, 0.6875)
				if stick == "LEFT" then
					texture:SetSize(12, Logic.TAG_H)
					texture:SetPoint("TOPLEFT")
					texture:SetPoint("BOTTOMLEFT")
					tag.faceLeft = texture
				elseif stick == "RIGHT" then
					texture:SetSize(12, Logic.TAG_H)
					texture:SetPoint("TOPRIGHT")
					texture:SetPoint("BOTTOMRIGHT")
					tag.faceRight = texture
				else
					texture:SetPoint("TOPLEFT", tag.faceLeft, "TOPRIGHT")
					texture:SetPoint("BOTTOMRIGHT", tag.faceRight, "BOTTOMLEFT")
				end
			end
			face(0, 0.09375, "LEFT")
			face(0.53125, 0.625, "RIGHT")
			face(0.09375, 0.53125, "MID")
			local label = tag:CreateFontString(nil, "OVERLAY", "GameFontNormal")
			label:SetPoint("CENTER", 0, 0)
			tag.label = label
			tag.zoneFace = true
			frame.zoneTags[index] = tag
		end
		tag.category = category
		tag.label:SetText(Logic.CATEGORY_LABEL[category] or category)
		local x, y = shown[index].x, shown[index].y
		tag.anchorX = x
		tag.anchorY = y
		tag:ClearAllPoints()
		tag:SetPoint("CENTER", content, "TOPLEFT", x, -y)
		tag:Show()
	end
end

function LB.HideZoneTags()
	local frame = LB.frame
	if not frame or not frame.zoneTags then
		return
	end
	for index = 1, #frame.zoneTags do
		HidePreview(frame.zoneTags[index])
		frame.zoneTags[index]:Hide()
	end
end

function LB.Hide()
	if not LB.frame or not LB.frame:IsShown() then
		return
	end
	LB.frame:Hide()
	C.Sound("igBackPackClose")
end

function LB.Toggle()
	if not LB.Enabled() then
		LB.SetEnabled(true)
	end
	LB.Ensure()
	if LB.frame:IsShown() then
		LB.Hide()
	else
		LB.Show()
	end
end
