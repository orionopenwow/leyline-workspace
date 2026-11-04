-- Loose Bags by Eric R (MIT, see LICENSE.txt), ported to the 3.3.5a client for Leyline.
-- The window, in Forever UI's bag window style (Skin.lua draws it with Forever UI's own functions when Forever UI is
-- loaded, and a clean look from the client's art otherwise): portrait ring and title, the red X, the search strip
-- with the bag slots, settings and Clean Up buttons, the items, the new-loot strip, and the purse at the bottom with
-- the free slots count and the money. Every piece is a child of the window and anchored to it, so all of it moves
-- with the window.
local Logic = LooseBagsLogic
local LB = Logic.LB
local C = LB.Compat
local S = LB.Skin

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
LB.Tip = Tip

local function Title()
	return Logic.FRAME_TITLE
end

local function Plain(value)
	if type(issecretvalue) == "function" and issecretvalue(value) then
		return nil
	end
	if type(value) ~= "number" or value ~= value then
		return nil
	end
	return value
end

-- Width of the right-hand action bars that show (the standard bags open left of them).
local function RightBars()
	local width = 0
	local names = { "MultiBarRight", "MultiBarLeft" }
	for index = 1, #names do
		local bar = _G[names[index]]
		if bar and bar.IsShown and bar:IsShown() and bar.GetWidth then
			width = width + (Plain(bar:GetWidth()) or 0)
		end
	end
	return width
end

-- Until the player moves it the bag opens bottom right, above the bag bar, as the standard bags; after a move it
-- stays where it was left. Either way it is anchored by its bottom right corner, so a bag that grows with its items
-- grows up and to the left and never off the bag bar.
function LB.PlaceWindow(frame)
	frame = frame or LB.frame
	if not frame then
		return
	end
	local char = LB.Char()
	frame:ClearAllPoints()
	if char.moved then
		frame:SetPoint(char.point, UIParent, char.relPoint, char.x, char.y)
		return
	end
	local point, relPoint, x, y = Logic.HomePoint(RightBars())
	frame:SetPoint(point, UIParent, relPoint, x, y)
end

local function SavePoint(frame)
	local char = LB.Char()
	local right = Plain(frame.GetRight and frame:GetRight())
	local bottom = Plain(frame.GetBottom and frame:GetBottom())
	if right and bottom then
		Logic.Apply(char, { op = "move_frame", window = 1, x = right, y = bottom, point = "BOTTOMRIGHT", relPoint = "BOTTOMLEFT" })
		char.moved = true
		frame:ClearAllPoints()
		frame:SetPoint("BOTTOMRIGHT", UIParent, "BOTTOMLEFT", right, bottom)
		return
	end
	local point, _, relPoint, x, y = frame:GetPoint(1)
	if type(x) == "number" and type(y) == "number" then
		Logic.Apply(char, {
			op = "move_frame",
			window = 1,
			x = x,
			y = y,
			point = type(point) == "string" and point or nil,
			relPoint = type(relPoint) == "string" and relPoint or nil,
		})
		char.moved = true
	end
end

local function StartMove(frame)
	frame:StartMoving()
end

-- The client would also keep a moved frame's spot in its own layout cache and put it back at login, over ours: the
-- spot is ours alone (saved per character, anchored by the bottom right corner).
local function NotUserPlaced(frame)
	if frame.SetUserPlaced then
		pcall(frame.SetUserPlaced, frame, false)
	end
end

local function StopMove(frame)
	frame:StopMovingOrSizing()
	SavePoint(frame)
	NotUserPlaced(frame)
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
	S.Close(close, frame, -6, -6)
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
	S.Panel(frame)
	local title = frame:CreateFontString(nil, "OVERLAY", "GameFontNormal")
	title:SetPoint("TOPLEFT", frame, "TOPLEFT", 18, -16)
	title:SetText("Loose Bags help")
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

-- ---------------------------------------------------------------- settings, lock, bag row, money, free slots
local TOGGLES = {
	{ key = "autoPlace", label = "Auto-place", body = "Put new items with their category instead of on the New items strip." },
	{ key = "autoOrder", label = "Auto-order", body = "On, Clean Up sorts inside each category. Off, it keeps each group's shape and order." },
	{ key = "sameTypeSnap", label = "Same-type snap", body = "Snap and group only with the same kind of item." },
	{ key = "useZones", label = "Zones", body = "Categories grow from the corner or edge nearest each tag. Edit zones places the tags." },
	{ key = "locked", label = "Lock size", body = "Keep this size: the corner grip hides. Drag the title or the portrait to move the bag." },
}

local function Lit(button, on)
	if not button then
		return
	end
	if on and button.LockHighlight then
		button:LockHighlight()
	elseif not on and button.UnlockHighlight then
		button:UnlockHighlight()
	end
end

-- The drawer opens beside the window, on the side with room (left of a bag at the right of the screen).
local function PlaceDrawer(frame)
	local drawer = frame.Settings
	if not drawer then
		return
	end
	drawer:ClearAllPoints()
	local left = Plain(frame.GetLeft and frame:GetLeft())
	local width = Plain(drawer:GetWidth()) or 210
	local gap = frame.forever and 16 or 4
	if left == nil or left > width + gap + 8 then
		drawer:SetPoint("TOPRIGHT", frame, "TOPLEFT", -gap, -24)
	else
		drawer:SetPoint("TOPLEFT", frame, "TOPRIGHT", gap, -24)
	end
end

function LB.RefreshSettings()
	local frame = LB.frame
	if not frame then
		return
	end
	local char = LB.Char()
	for index = 1, #TOGGLES do
		local key = TOGGLES[index].key
		local check = frame.Toggles and frame.Toggles[key]
		if check and check.SetChecked then
			check:SetChecked(char[key] and true or false)
		end
	end
	if frame.ZoneButton then
		frame.ZoneButton:SetText(LB.zoneEdit and "Done" or "Edit zones")
	end
	if frame.FitButton and frame.FitButton.Enable then
		if type(char.width) == "number" then
			frame.FitButton:Enable()
		else
			frame.FitButton:Disable()
		end
	end
	if char.collapsed then
		frame.Settings:Hide()
	else
		PlaceDrawer(frame)
		frame.Settings:Show()
	end
	Lit(frame.Collapse, not char.collapsed)
	Lit(frame.BagsButton, char.bagRow)
end

function LB.RefreshLock()
	local frame = LB.frame
	if not frame then
		return
	end
	local char = LB.Char()
	if frame.Grip then
		C.SetShown(frame.Grip, not char.locked)
	end
	frame:SetResizable(not char.locked)
end

-- The money line: gold, silver and copper, always all three, from the player's own money.
function LB.UpdateMoney()
	local frame = LB.frame
	local purse = frame and frame.Money
	if not purse or not purse.coins then
		return
	end
	local money = C.Call("GetMoney")
	local gold, silver, copper = Logic.MoneyParts(money)
	purse.coins.gold.text:SetText(Logic.Grouped(gold))
	purse.coins.silver.text:SetText(silver)
	purse.coins.copper.text:SetText(copper)
	purse.money = gold * 10000 + silver * 100 + copper
end

-- Free slots in the general bags (a quiver, soul bag or profession bag holds only its own kind, so it is not counted,
-- as the client's own free slot count).
function LB.FreeSlots()
	local free, total = 0, 0
	local bags = LB.BagList()
	for index = 1, #bags do
		local bag = bags[index]
		local slots = C.NumSlots(bag)
		local open, bagType = C.Call("GetContainerNumFreeSlots", bag)
		if slots > 0 and (type(bagType) ~= "number" or bagType == 0) then
			total = total + slots
			if type(open) == "number" then
				free = free + open
			end
		end
	end
	return free, total
end

function LB.UpdateFree()
	local frame = LB.frame
	if not frame or not frame.FreeText then
		return
	end
	local free, total = LB.FreeSlots()
	local text, line = Logic.FreeText(free, total)
	frame.FreeText:SetText(text)
	frame.freeLine = line
	if free == 0 then
		frame.FreeText:SetTextColor(1, 0.25, 0.2)
	else
		frame.FreeText:SetTextColor(1, 1, 1)
	end
end

local function BagInvID(bag)
	if bag == 0 then
		return nil
	end
	local id = C.Call("ContainerIDToInventoryID", bag)
	if type(id) == "number" then
		return id
	end
	return nil
end

function LB.RefreshBagRow()
	local frame = LB.frame
	if not frame or not frame.BagRow then
		return
	end
	local row = frame.BagRow
	C.SetShown(row, LB.Char().bagRow)
	for index = 1, #row.slots do
		local button = row.slots[index]
		local bag = button.bag
		local texture
		if bag == 0 then
			texture = S.BACKPACK_ICON
		else
			local invID = BagInvID(bag)
			if invID then
				texture = C.Call("GetInventoryItemTexture", "player", invID)
			end
		end
		button.parts.icon:SetTexture(texture or S.EMPTY_BAG)
		S.BagRing(button.parts, texture ~= nil)
		local slots = C.NumSlots(bag)
		button.Count:SetText(slots > 0 and slots or "")
	end
end

local function BagSlotButton(row, bag, index)
	local button = CreateFrame("Button", "LooseBagsBagSlot" .. bag, row)
	button:SetSize(30, 30)
	button:SetPoint("LEFT", row, "LEFT", (index - 1) * 34, 0)
	button.bag = bag
	button.parts = S.BagSlot(button)
	local count = button:CreateFontString(nil, "OVERLAY", "NumberFontNormal")
	count:SetPoint("BOTTOMRIGHT", button, "BOTTOMRIGHT", -2, 2)
	button.Count = count
	button:RegisterForDrag("LeftButton")
	local function Put()
		if bag == 0 then
			C.Call("PutItemInBackpack")
		else
			local invID = BagInvID(bag)
			if invID then
				C.Call("PutItemInBag", invID)
			end
		end
	end
	button:SetScript("OnClick", function()
		if CursorHasItem and CursorHasItem() then
			Put()
			return
		end
		local invID = BagInvID(bag)
		if invID then
			C.Call("PickupBagFromSlot", invID)
		end
	end)
	button:SetScript("OnDragStart", function()
		local invID = BagInvID(bag)
		if invID then
			C.Call("PickupBagFromSlot", invID)
		end
	end)
	button:SetScript("OnReceiveDrag", Put)
	button:SetScript("OnEnter", function(self)
		LB.hoverBag = bag
		if LB.ApplySearch then
			LB.ApplySearch()
		end
		if not GameTooltip then
			return
		end
		GameTooltip:SetOwner(self, "ANCHOR_LEFT")
		local invID = BagInvID(bag)
		local shown = false
		if invID and GameTooltip.SetInventoryItem then
			shown = C.Call("GetInventoryItemTexture", "player", invID) ~= nil and select(1, pcall(GameTooltip.SetInventoryItem, GameTooltip, "player", invID))
		end
		if not shown then
			GameTooltip:SetText(bag == 0 and "Backpack" or "Bag slot", 1, 1, 1)
			if bag ~= 0 and GameTooltip.AddLine then
				GameTooltip:AddLine("Drop a bag here to equip it.", 0.9, 0.9, 0.9, true)
			end
		end
		GameTooltip:Show()
	end)
	button:SetScript("OnLeave", function()
		LB.hoverBag = nil
		if LB.ApplySearch then
			LB.ApplySearch()
		end
		HideTip()
	end)
	return button
end

local function Coin(purse, which, anchor, gap)
	local icon = purse:CreateTexture(nil, "ARTWORK")
	S.Coin(icon, which)
	if anchor then
		icon:SetPoint("RIGHT", anchor, "LEFT", -gap, 0)
	else
		icon:SetPoint("RIGHT", purse, "RIGHT", -gap, 0)
	end
	local text = purse:CreateFontString(nil, "ARTWORK", "NumberFontNormal")
	text:SetPoint("RIGHT", icon, "LEFT", -1, 0)
	if text.SetJustifyH then
		text:SetJustifyH("RIGHT")
	end
	text:SetText("0")
	return { icon = icon, text = text }
end

-- ---------------------------------------------------------------- the window
function LB.Ensure()
	if LB.frame then
		return LB.frame
	end
	local frame = CreateFrame("Frame", "LooseBagsFrame", UIParent)
	frame:SetSize(Logic.CHROME_MIN_W, Logic.CHROME_MIN_H)
	frame:SetFrameStrata("MEDIUM")
	frame:SetMovable(true)
	frame:SetResizable(true)
	frame:SetClampedToScreen(true)
	frame:SetToplevel(true)
	frame:EnableMouse(true)
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
	-- Only the grip's own resize is remembered (the window also changes size when it fits its items).
	frame:SetScript("OnSizeChanged", function(self, width, height)
		S.Corners(self)
		if not LB.sizing or LB.inLayout then
			return
		end
		local plainW = LB.Plain(width)
		local plainH = LB.Plain(height)
		if not plainW or not plainH then
			return
		end
		local char = LB.Char()
		char.width = plainW
		char.height = plainH - LB.BarHeight(char)
		LB.Layout()
	end)
	-- Esc closes the bag, as it does the standard ones.
	C.Special("LooseBagsFrame")
	frame:SetScript("OnShow", function()
		if LB.SyncBagButtons then
			LB.SyncBagButtons()
		end
		LB.UpdateMoney()
		LB.UpdateFree()
	end)
	frame:SetScript("OnHide", function()
		if LB.SyncBagButtons then
			LB.SyncBagButtons()
		end
		LB.hoverBag = nil
		local char = LB.Char()
		char.collapsed = true
		if frame.Settings then
			frame.Settings:Hide()
		end
	end)

	-- Frame, portrait and title (Skin.lua: Forever UI's bag window, or the plain one).
	local look = S.Window(frame, Title())
	frame.forever = look.forever
	frame.Portrait = look.portrait
	frame.TitleText = look.title
	local close = C.Make("Button", "LooseBagsFrameClose", frame, "UIPanelCloseButton")
	S.Close(close, frame)
	if close.SetFrameLevel then
		close:SetFrameLevel(frame:GetFrameLevel() + 6)
	end
	close:SetScript("OnClick", function()
		LB.Hide()
	end)
	frame.CloseButton = close
	local portraitDrag = MoveHandle(frame)
	portraitDrag:SetPoint("TOPLEFT", frame, "TOPLEFT", -4, 4)
	portraitDrag:SetSize(40, 40)
	local titleDrag = MoveHandle(frame)
	titleDrag:SetPoint("TOPLEFT", frame, "TOPLEFT", 36, 0)
	titleDrag:SetPoint("BOTTOMRIGHT", close, "BOTTOMLEFT", 0, 0)
	LB.PlaceWindow(frame)

	-- The search strip: Clean Up in the corner (Forever UI's spot, TOPRIGHT -9, -40), the settings cog and the bag
	-- slots button beside it, the search box filling the rest.
	local sort = CreateFrame("Button", "LooseBagsCleanUp", frame)
	S.SortButton(sort)
	sort:SetPoint("TOPRIGHT", frame, "TOPRIGHT", -9, -40)
	sort:SetScript("OnClick", function()
		C.Sound("igMainMenuOptionCheckBoxOn")
		LB.Sort()
	end)
	Tip(sort, "Clean Up Bags", "Sort the bag into a tidy grid from the top left, one block per category. With Zones on, each category grows from its tag.")
	frame.SortButton = sort

	local gear = CreateFrame("Button", "LooseBagsSettingsButton", frame)
	S.SmallButton(gear, "questlog-icon-setting", S.Path("Interface", "Icons", "INV_Misc_Gear_01"), 18)
	gear:SetPoint("RIGHT", sort, "LEFT", -5, 0)
	gear:SetScript("OnClick", function()
		local row = LB.Char()
		row.collapsed = not row.collapsed
		C.Sound(row.collapsed and "igMainMenuOptionCheckBoxOff" or "igMainMenuOptionCheckBoxOn")
		LB.RefreshSettings()
	end)
	Tip(gear, "Settings", "Auto-place, Auto-order, Same-type snap, Zones, Lock size and help.")
	frame.Collapse = gear

	local bagsButton = CreateFrame("Button", "LooseBagsBagSlotsButton", frame)
	S.SmallButton(bagsButton, "bag-main", S.BACKPACK_ICON, 20)
	bagsButton:SetPoint("RIGHT", gear, "LEFT", -5, 0)
	bagsButton:SetScript("OnClick", function()
		local row = LB.Char()
		row.bagRow = not row.bagRow
		C.Sound("igMainMenuOptionCheckBoxOn")
		LB.RefreshBagRow()
		LB.RefreshSettings()
		LB.Layout()
	end)
	Tip(bagsButton, "Bag slots", "Show or hide your bag slots. Point at a bag to light up its items.")
	frame.BagsButton = bagsButton

	local search, templated = C.Make("EditBox", "LooseBagsSearchBox", frame, "InputBoxTemplate")
	if not templated then
		if search.SetFontObject and ChatFontNormal then
			search:SetFontObject(ChatFontNormal)
		end
		if search.SetBackdrop then
			search:SetBackdrop({
				bgFile = S.Path("Interface", "ChatFrame", "ChatFrameBackground"),
				edgeFile = S.Path("Interface", "Tooltips", "UI-Tooltip-Border"),
				tile = true, tileSize = 16, edgeSize = 12,
				insets = { left = 3, right = 3, top = 3, bottom = 3 },
			})
			search:SetBackdropColor(0, 0, 0, 0.6)
		end
	end
	search:SetAutoFocus(false)
	if search.SetMaxLetters then
		search:SetMaxLetters(30)
	end
	search:SetHeight(18)
	search:SetPoint("TOPLEFT", frame, "TOPLEFT", 42, -43)
	search:SetPoint("RIGHT", bagsButton, "LEFT", -10, 0)
	local box = S.Search(search)
	local hint, clear = box.hint, box.clear
	frame.SearchHint = hint
	frame.SearchClear = clear
	local function ShowHint(self)
		local text = self:GetText() or ""
		local focused = self.HasFocus and self:HasFocus()
		C.SetShown(hint, text == "" and not focused)
		C.SetShown(clear, text ~= "")
	end
	local function Hook(widget, script, fn)
		if widget.HookScript and widget:GetScript(script) then
			widget:HookScript(script, fn)
		else
			widget:SetScript(script, fn)
		end
	end
	Hook(search, "OnEditFocusGained", function()
		hint:Hide()
	end)
	Hook(search, "OnEditFocusLost", ShowHint)
	search:SetScript("OnEscapePressed", function(self)
		if (self:GetText() or "") ~= "" then
			self:SetText("")
		end
		self:ClearFocus()
	end)
	search:SetScript("OnEnterPressed", function(self)
		self:ClearFocus()
	end)
	-- Four identical characters in a row let go of the box (Forever UI's BagSearch_OnChar): a held key never traps
	-- the player in it.
	search:SetScript("OnChar", function(self)
		local text = self:GetText() or ""
		local length = string.len(text)
		if length >= 4 then
			local last = string.sub(text, length, length)
			if string.sub(text, length - 3, length) == string.rep(last, 4) then
				self:ClearFocus()
			end
		end
	end)
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
	clear:SetScript("OnClick", function()
		C.Sound("igMainMenuOptionCheckBoxOn")
		search:SetText("")
		search:ClearFocus()
	end)
	frame.Search = search
	frame.searchText = ""

	-- The bag slots row (toggled; the items move down by its height).
	local bagRow = CreateFrame("Frame", "LooseBagsBagRow", frame)
	bagRow:SetHeight(32)
	bagRow:SetPoint("TOPLEFT", frame, "TOPLEFT", Logic.SIDE_INSET + 2, -(Logic.TOP_BASE - 2))
	bagRow:SetPoint("TOPRIGHT", frame, "TOPRIGHT", -Logic.SIDE_INSET, -(Logic.TOP_BASE - 2))
	bagRow.slots = {}
	local bags = LB.BagList()
	for index = 1, #bags do
		bagRow.slots[index] = BagSlotButton(bagRow, bags[index], index)
	end
	bagRow:Hide()
	frame.BagRow = bagRow

	-- Items and the new-loot strip.
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
	local stripLabel = strip:CreateFontString(nil, "OVERLAY", "GameFontNormalSmall")
	stripLabel:SetPoint("BOTTOMLEFT", strip, "TOPLEFT", 2, 4)
	stripLabel:SetText("New items")
	local rule = strip:CreateTexture(nil, "ARTWORK")
	rule:SetHeight(1)
	rule:SetPoint("LEFT", stripLabel, "RIGHT", 6, 0)
	rule:SetPoint("RIGHT", strip, "RIGHT", 0, 0)
	S.Rule(rule)
	Tip(strip, "New items", "New loot waits here. Drag it into place, or turn on Auto-place in the settings.")
	strip.Label = stripLabel
	frame.Strip = strip

	-- The purse: inside the window at the bottom, Forever UI's place for it (8 in from each side, 14 up, 13 high).
	-- Its own child frame, coins and texts: nothing in it is anchored outside the window.
	local purse = CreateFrame("Frame", "LooseBagsPurse", frame)
	purse:SetHeight(Logic.PURSE_H)
	purse:SetPoint("BOTTOMLEFT", frame, "BOTTOMLEFT", Logic.PURSE_SIDE, Logic.PURSE_BOTTOM)
	purse:SetPoint("BOTTOMRIGHT", frame, "BOTTOMRIGHT", -Logic.PURSE_SIDE, Logic.PURSE_BOTTOM)
	S.Purse(purse)
	local copper = Coin(purse, "copper", nil, 7)
	local silver = Coin(purse, "silver", copper.text, 5)
	local gold = Coin(purse, "gold", silver.text, 5)
	purse.coins = { gold = gold, silver = silver, copper = copper }
	local freeText = purse:CreateFontString("LooseBagsFreeText", "ARTWORK", "GameFontHighlightSmall")
	freeText:SetPoint("LEFT", purse, "LEFT", 9, 0)
	frame.FreeText = freeText
	local freeHover = CreateFrame("Frame", nil, purse)
	freeHover:SetPoint("TOPLEFT", purse, "TOPLEFT", 0, 2)
	freeHover:SetPoint("BOTTOMRIGHT", purse, "BOTTOM", 0, -2)
	freeHover:EnableMouse(true)
	freeHover:SetScript("OnEnter", function(self)
		if not GameTooltip then
			return
		end
		GameTooltip:SetOwner(self, "ANCHOR_RIGHT")
		GameTooltip:SetText("Free slots", 1, 1, 1)
		if GameTooltip.AddLine and frame.freeLine then
			GameTooltip:AddLine(frame.freeLine, 0.9, 0.9, 0.9, true)
		end
		GameTooltip:Show()
	end)
	freeHover:SetScript("OnLeave", HideTip)
	frame.Money = purse

	-- The resize grip: the bottom right corner, below the purse.
	local grip = CreateFrame("Button", "LooseBagsGrip", frame)
	grip:SetSize(12, 12)
	grip:SetPoint("BOTTOMRIGHT", frame, "BOTTOMRIGHT", -1, 1)
	if grip.SetFrameLevel then
		grip:SetFrameLevel(frame:GetFrameLevel() + 8)
	end
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
		LB.sizing = true
		frame:StartSizing("BOTTOMRIGHT")
	end)
	grip:SetScript("OnMouseUp", function(self)
		if self.SetButtonState then
			self:SetButtonState("NORMAL", false)
		end
		frame:StopMovingOrSizing()
		local wasSizing = LB.sizing
		LB.sizing = false
		local width = LB.Plain(frame:GetWidth())
		local height = LB.Plain(frame:GetHeight())
		if wasSizing and width and height and not LB.Char().locked then
			local char = LB.Char()
			char.width = width
			char.height = height - LB.BarHeight(char)
			SavePoint(frame)
			NotUserPlaced(frame)
			LB.Layout()
			LB.RefreshSettings()
		end
	end)
	Tip(grip, "Resize", "Drag to choose the bag's size. Fit to items, in the settings, makes it fit its items again.")
	frame.Grip = grip

	-- The settings drawer (the cog): tick boxes, Edit zones, Fit to items and Help.
	local drawer = CreateFrame("Frame", "LooseBagsSettings", frame)
	drawer:SetSize(206, 76 + #TOGGLES * 24 + 58)
	drawer:EnableMouse(true)
	if drawer.SetFrameLevel then
		drawer:SetFrameLevel(frame:GetFrameLevel() + 30)
	end
	S.Panel(drawer)
	local heading = drawer:CreateFontString(nil, "OVERLAY", "GameFontNormal")
	heading:SetPoint("TOP", drawer, "TOP", 0, -18)
	heading:SetText("Bag settings")
	frame.Toggles = {}
	for index = 1, #TOGGLES do
		local spec = TOGGLES[index]
		local check = C.Make("CheckButton", "LooseBagsToggle" .. spec.key, drawer, "UICheckButtonTemplate")
		check:SetSize(22, 22)
		check:SetPoint("TOPLEFT", drawer, "TOPLEFT", 18, -(38 + (index - 1) * 24))
		S.Check(check)
		local label = drawer:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
		label:SetPoint("LEFT", check, "RIGHT", 4, 1)
		label:SetText(spec.label)
		check.label = label
		check:SetScript("OnClick", function()
			local char = LB.Char()
			char[spec.key] = not char[spec.key]
			C.Sound(char[spec.key] and "igMainMenuOptionCheckBoxOn" or "igMainMenuOptionCheckBoxOff")
			if spec.key == "autoPlace" and char.autoPlace and LB.FileHeld then
				LB.FileHeld()
			end
			if spec.key == "locked" then
				LB.RefreshLock()
			end
			LB.RefreshSettings()
		end)
		Tip(check, spec.label, spec.body)
		frame.Toggles[spec.key] = check
	end
	frame.AutoPlace = frame.Toggles.autoPlace
	frame.AutoOrder = frame.Toggles.autoOrder
	frame.SameType = frame.Toggles.sameTypeSnap
	frame.Zones = frame.Toggles.useZones
	frame.LockButton = frame.Toggles.locked
	local buttonsTop = -(38 + #TOGGLES * 24 + 6)
	local function DrawerButton(name, text, x, y, onClick, title, body)
		local button = C.Make("Button", name, drawer, "UIPanelButtonTemplate")
		button:SetSize(82, 22)
		button:SetPoint("TOPLEFT", drawer, "TOPLEFT", x, y)
		button:SetText(text)
		S.TextButton(button)
		button:SetScript("OnClick", onClick)
		Tip(button, title, body)
		return button
	end
	frame.ZoneButton = DrawerButton("LooseBagsEditZones", "Edit zones", 18, buttonsTop, function()
		LB.zoneEdit = not LB.zoneEdit
		LB.Layout()
		LB.RefreshSettings()
	end, "Edit zones", "Hide the items and drag one tag per category to where it should sit.")
	frame.FitButton = DrawerButton("LooseBagsFitItems", "Fit to items", 106, buttonsTop, function()
		local char = LB.Char()
		char.width = nil
		char.height = nil
		LB.Layout()
		LB.RefreshSettings()
	end, "Fit to items", "Forget the size chosen with the grip: the bag fits its items again and grows as they come.")
	frame.HelpButton = DrawerButton("LooseBagsHelpButton", "Help", 18, buttonsTop - 26, function()
		LB.ToggleHelp()
	end, "Help", "How to use Loose Bags.")
	local done = DrawerButton("LooseBagsSettingsDone", "Close", 106, buttonsTop - 26, function()
		LB.Char().collapsed = true
		LB.RefreshSettings()
	end, "Close", "Close the settings.")
	frame.SettingsDone = done
	drawer:Hide()
	frame.Settings = drawer

	LB.frame = frame
	LB.buttons = {}
	LB.RefreshBagRow()
	LB.RefreshSettings()
	LB.RefreshLock()
	frame:Hide()
	return frame
end

function LB.Show()
	local frame = LB.Ensure()
	LB.ApplyFirstRun("open")
	local wasShown = frame:IsShown()
	if not wasShown then
		LB.PlaceWindow(frame)
	end
	frame:Show()
	LB.Refresh()
	LB.RefreshBagRow()
	LB.UpdateMoney()
	LB.UpdateFree()
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
