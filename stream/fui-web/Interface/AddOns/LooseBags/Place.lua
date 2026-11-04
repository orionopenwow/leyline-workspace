-- Loose Bags by Eric R (MIT, see LICENSE.txt), ported to the 3.3.5a client for Leyline.
-- The item buttons: layout, drag, drop and clicks. Each button is the client's own bag item button
-- (ContainerFrameItemButtonTemplate) on a holder frame whose id is its bag, as in the standard bags, so a
-- right-click uses the item through the client's own click code (an addon's own call cannot use an item that casts).
local Logic = LooseBagsLogic
local LB = Logic.LB
local C = LB.Compat
LB.gesture = { mode = "idle" }

local function PlaceKey(place)
	return place.itemID .. ":" .. place.inst
end

local function Cursor()
	local x, y = GetCursorPosition()
	if type(issecretvalue) == "function" and (issecretvalue(x) or issecretvalue(y)) then
		return nil
	end
	local scale = 1
	if UIParent.GetEffectiveScale then
		scale = UIParent:GetEffectiveScale() or 1
	end
	if type(issecretvalue) == "function" and issecretvalue(scale) then
		scale = 1
	end
	if type(scale) ~= "number" or scale == 0 then
		scale = 1
	end
	return x / scale, y / scale
end

local function plainNumber(value)
	if type(issecretvalue) == "function" and issecretvalue(value) then
		return nil
	end
	if type(value) ~= "number" then
		return nil
	end
	return value
end

local function Inside(frame, x, y)
	if not frame or not frame.GetLeft then
		return false
	end
	local left = plainNumber(frame:GetLeft())
	local right = plainNumber(frame:GetRight())
	local top = plainNumber(frame:GetTop())
	local bottom = plainNumber(frame:GetBottom())
	if not left or not right or not top or not bottom then
		return false
	end
	return x >= left and x <= right and y >= bottom and y <= top
end

local function LocalPoint(host, x, y)
	local left = plainNumber(host:GetLeft())
	local top = plainNumber(host:GetTop())
	if not left or not top then
		return nil
	end
	return x - left, top - y
end

function LB.DropCursorInEmpty()
	if not LB.PickupSlot then
		return
	end
	local bags = LB.BagList()
	for index = 1, #bags do
		local bag = bags[index]
		if C.BagTakesCursor(bag) then
			local slots = C.NumSlots(bag)
			for slot = 1, slots do
				if not LB.ReadSlot(bag, slot) then
					LB.PickupSlot(bag, slot)
					return
				end
			end
		end
	end
end

local function HideTooltip()
	if GameTooltip_Hide then
		GameTooltip_Hide()
	elseif GameTooltip then
		GameTooltip:Hide()
	end
end

-- One holder frame per bag (its id is the bag), like the standard bag frames: the client's item button code reads
-- the bag from the button's parent and the slot from the button's own id.
local function Holder(bag)
	local frame = LB.frame
	frame.bagHolders = frame.bagHolders or {}
	local holder = frame.bagHolders[bag]
	if holder then
		return holder
	end
	holder = CreateFrame("Frame", "LooseBagsBag" .. bag, frame)
	holder:SetID(bag)
	holder:SetAllPoints(frame)
	frame.bagHolders[bag] = holder
	return holder
end
LB.Holder = Holder

local function ButtonPart(button, suffix)
	local name = button.GetName and button:GetName()
	return name and _G[name .. suffix] or nil
end

-- Without the client's template (a client that lacks it): an icon, a count and a tooltip of our own.
local function PlainParts(button, name)
	local icon = button:CreateTexture(name .. "IconTexture", "BORDER")
	icon:SetAllPoints(button)
	local count = button:CreateFontString(name .. "Count", "ARTWORK", "NumberFontNormal")
	count:SetPoint("BOTTOMRIGHT", button, "BOTTOMRIGHT", -5, 2)
	button:SetScript("OnEnter", function(self)
		if not GameTooltip or not GameTooltip.SetBagItem then
			return
		end
		GameTooltip:SetOwner(self, "ANCHOR_RIGHT")
		GameTooltip:SetBagItem(self.bag, self.slot)
		GameTooltip:Show()
	end)
	button:SetScript("OnLeave", HideTooltip)
end

local function Acquire(index, parent)
	local button = LB.buttons[index]
	if button then
		return button
	end
	local name = "LooseBagsItem" .. index
	local templated
	button, templated = C.Make("Button", name, parent, "ContainerFrameItemButtonTemplate")
	-- SLOT wide in an ICON cell: snapped neighbours keep Forever UI's gap between slots.
	button:SetSize(Logic.SLOT, Logic.SLOT)
	button.clientClick = templated
	if templated then
		-- Right-click (use, and its modified clicks) stays the client's own; left clicks and drags are ours.
		button:RegisterForClicks("RightButtonUp")
	else
		PlainParts(button, name)
		button:RegisterForClicks("LeftButtonUp", "RightButtonUp")
	end
	button:RegisterForDrag("LeftButton")
	if button.SetHighlightTexture then
		button:SetHighlightTexture("Interface\\Buttons\\ButtonHilight-Square", "ADD")
	end
	-- Forever UI's slot look (Skin.lua): grey outline tinted by quality, the new-item glow, the search veil.
	local look = LB.Skin.Slot(button, name)
	button.lbOutline = look.outline
	button.QualityBorder = look.glow
	button.NewItemTexture = look.fresh
	button.SearchVeil = look.veil
	button:SetScript("OnMouseDown", function(self, mouseButton)
		LB.BeginPress(self, mouseButton)
	end)
	button:SetScript("OnDragStart", function(self)
		LB.PromoteDrag(self)
	end)
	button:SetScript("OnDragStop", function(self)
		LB.ReleasePress(self, "LeftButton")
	end)
	button:SetScript("OnMouseUp", function(self, mouseButton)
		LB.ReleasePress(self, mouseButton)
	end)
	LB.buttons[index] = button
	return button
end

local function SetIcon(button, texture)
	if SetItemButtonTexture then
		SetItemButtonTexture(button, texture)
		return
	end
	local icon = ButtonPart(button, "IconTexture")
	if icon then
		icon:SetTexture(texture)
	end
end

local function SetCount(button, count)
	if SetItemButtonCount then
		SetItemButtonCount(button, count)
		return
	end
	local text = ButtonPart(button, "Count")
	if text then
		text:SetText((count or 1) > 1 and count or "")
	end
end

local function SetLocked(button, locked)
	if SetItemButtonDesaturated then
		SetItemButtonDesaturated(button, locked)
		return
	end
	local icon = ButtonPart(button, "IconTexture")
	if icon and icon.SetDesaturated then
		icon:SetDesaturated(locked and true or false)
	end
end

local function SetQuality(button, item)
	LB.Skin.Quality(button, item and item.quality)
end

-- The standard bags' quest marks: "!" starts a quest, the gold border is an item a quest wants.
local function SetQuest(button, item)
	local mark = ButtonPart(button, "IconQuestTexture")
	if not mark then
		return
	end
	if item and item.questID and not item.questActive and TEXTURE_ITEM_QUEST_BANG then
		mark:SetTexture(TEXTURE_ITEM_QUEST_BANG)
		mark:Show()
	elseif item and (item.questID or item.isQuest) and TEXTURE_ITEM_QUEST_BORDER then
		mark:SetTexture(TEXTURE_ITEM_QUEST_BORDER)
		mark:Show()
	else
		mark:Hide()
	end
end

local function SetCooldown(button)
	local cooldown = ButtonPart(button, "Cooldown")
	if not cooldown or not button.bag or not button.slot then
		return
	end
	local start, duration, enable = C.Call("GetContainerItemCooldown", button.bag, button.slot)
	if type(start) ~= "number" then
		return
	end
	if CooldownFrame_SetTimer then
		CooldownFrame_SetTimer(cooldown, start, duration, enable)
	end
end

function LB.UpdateCooldowns()
	for index = 1, #(LB.buttons or {}) do
		local button = LB.buttons[index]
		if button:IsShown() then
			SetCooldown(button)
		end
	end
end

function LB.ApplySearch()
	local frame = LB.frame
	if not frame then
		return
	end
	-- Search (name, kind or sub-kind, as Forever UI's bag search) and the bag row's hover (only that bag's items)
	-- veil what does not match in black, as Forever UI's bags do.
	local text = frame.searchText or ""
	local onlyBag = LB.hoverBag
	for index = 1, #LB.buttons do
		local button = LB.buttons[index]
		if button:IsShown() then
			local match = Logic.SearchMatch(text, button.itemName, button.itemKind, button.itemSubKind)
			if onlyBag ~= nil and button.bag ~= onlyBag then
				match = false
			end
			if button.SearchVeil then
				C.SetShown(button.SearchVeil, not match)
			else
				button:SetAlpha(match and 1 or 0.25)
			end
			button.dimmed = not match
		end
	end
end

local function ScreenSize()
	if not UIParent or not UIParent.GetWidth or not UIParent.GetHeight then
		return nil, nil
	end
	return plainNumber(UIParent:GetWidth()), plainNumber(UIParent:GetHeight())
end

local function HostFor(place, frame)
	if place.held then
		return frame.Strip
	end
	return frame.Content
end

function LB.Layout()
	if LB.inLayout then
		return
	end
	LB.inLayout = true
	local frame = LB.Ensure()
	local char = LB.Char()
	-- bar: the bag slots row when it shows. It adds to the window's height, never takes from the items' room.
	local bar = LB.BarHeight(char)
	local held = {}
	for index = 1, #char.places do
		local place = char.places[index]
		if place.held then
			held[#held + 1] = place
		end
	end
	local screenW, screenH = ScreenSize()
	-- No size chosen with the grip: the window fits its items (grows as they come). A chosen size stays.
	local frameW, frameH = Logic.KeepFrame(char)
	if frameW then
		frameH = frameH + bar
	else
		frameW, frameH = Logic.AutoSize(char.places, bar, screenW, screenH, LB.zoneEdit and 5 or 1)
	end
	local contentW, contentH = Logic.ContentBox(frameW, frameH, bar, #held)
	char.homes = Logic.ClampHomes(char.homes, contentW, contentH)
	local holding = Logic.HoldingPositions(#held, Logic.ICON, contentW)
	for index = 1, #held do
		local spot = holding.spots[index]
		Logic.Apply(char, {
			op = "move_item",
			place = held[index],
			x = spot.x,
			y = spot.y,
			held = true,
		})
	end
	local inside = Logic.BringInside(char.places, contentW, contentH, Logic.ICON)
	for index = 1, #inside do
		local spot = inside[index]
		Logic.Apply(char, {
			op = "move_item",
			place = spot.place,
			x = spot.x,
			y = spot.y,
		})
	end
	local top = Logic.TOP_BASE + bar
	frame:SetSize(frameW, frameH)
	local minW, minH, maxW, maxH = Logic.ResizeLimits(bar, frameW, frameH, screenW, screenH)
	C.SetResizeBounds(frame, minW, minH, maxW, maxH)
	LB.Skin.Corners(frame)
	frame.Content:ClearAllPoints()
	frame.Content:SetPoint("TOPLEFT", frame, "TOPLEFT", Logic.SIDE_INSET, -top)
	frame.Content:SetPoint("TOPRIGHT", frame, "TOPRIGHT", -Logic.SIDE_INSET, -top)
	frame.Content:SetHeight(contentH)
	frame.Strip:ClearAllPoints()
	frame.Strip:SetPoint("TOPLEFT", frame.Content, "BOTTOMLEFT", 0, -Logic.STRIP_GAP)
	frame.Strip:SetPoint("TOPRIGHT", frame.Content, "BOTTOMRIGHT", 0, -Logic.STRIP_GAP)
	frame.Strip:SetHeight(holding.height > 0 and holding.height or Logic.ICON)
	if #held > 0 then
		frame.Strip:Show()
	else
		frame.Strip:Hide()
	end

	local live = {}
	local scanned = LB.Scan()
	for index = 1, #scanned do
		local item = scanned[index]
		live[item.bag .. ":" .. item.slot] = item
	end

	for index = 1, #char.places do
		local place = char.places[index]
		local item = live[place.bag .. ":" .. place.slot]
		local host = HostFor(place, frame)
		local holder = Holder(place.bag)
		local button = Acquire(index, holder)
		button:SetParent(holder)
		if button.SetID then
			button:SetID(place.slot)
		end
		button:ClearAllPoints()
		button:SetPoint("CENTER", host, "TOPLEFT", place.x, -place.y)
		if button.SetFrameLevel and host.GetFrameLevel then
			button:SetFrameLevel(Logic.ItemLevel(host:GetFrameLevel(), index))
		end
		button.host = host
		button.bag = place.bag
		button.slot = place.slot
		-- read by the client's own item button code (tooltip cursor, Shift-click split)
		button.hasItem = item and 1 or nil
		button.count = item and item.count or 0
		button.place = place
		button.itemName = item and item.name or nil
		button.itemKind = item and item.kind or nil
		button.itemSubKind = item and item.subKind or nil
		if item and item.icon then
			SetIcon(button, item.icon)
		end
		if item then
			SetCount(button, item.count)
		end
		SetQuality(button, item)
		SetQuest(button, item)
		SetLocked(button, item and item.locked)
		C.SetShown(button.NewItemTexture, place.held)
		button:Show()
		SetCooldown(button)
	end
	for index = #char.places + 1, #LB.buttons do
		LB.buttons[index]:Hide()
	end
	if LB.zoneEdit then
		for index = 1, #LB.buttons do
			LB.buttons[index]:Hide()
		end
		if LB.ShowZoneTags then
			LB.ShowZoneTags()
		end
	elseif LB.HideZoneTags then
		LB.HideZoneTags()
	end
	LB.ApplySearch()
	if LB.RefreshLock then
		LB.RefreshLock()
	end
	if LB.UpdateFree then
		LB.UpdateFree()
	end
	LB.inLayout = false
end

function LB.BeginPress(button, mouseButton)
	if button and button.place and not Logic.ItemDraggable(button.place) then
		return
	end
	local x, y = Cursor()
	if not x then
		return
	end
	if type(LB.gesture) ~= "table" or LB.gesture.mode ~= "idle" then
		LB.gesture = { mode = "idle" }
	end
	local nextState = Logic.Step(LB.gesture, "down")
	if not nextState then
		return
	end
	LB.gesture = nextState
	LB.press = {
		button = button,
		mouse = mouseButton,
		startX = x,
		startY = y,
	}
	-- (button.host: the bag area or the new-loot strip it sits on; its parent is its bag's holder)
	local parent = button.host or (button.GetParent and button:GetParent())
	if button.SetFrameLevel and parent and parent.GetFrameLevel then
		button:SetFrameLevel(parent:GetFrameLevel() + 40)
	end
	-- The press is followed from a frame of our own: the item button keeps the client's own OnUpdate.
	if not LB.tracker then
		LB.tracker = CreateFrame("Frame")
		LB.tracker:Hide()
		LB.tracker:SetScript("OnUpdate", function(self)
			local press = LB.press
			if not press then
				self:Hide()
				return
			end
			LB.TrackPress(press.button)
		end)
	end
	LB.tracker:Show()
end

local function ButtonFor(place)
	for index = 1, #LB.buttons do
		local button = LB.buttons[index]
		if button.place == place then
			return button
		end
	end
	return nil
end

function LB.ReleasePress(button, mouseButton)
	local held = false
	if IsMouseButtonDown then
		local down = IsMouseButtonDown(mouseButton)
		if type(issecretvalue) ~= "function" or not issecretvalue(down) then
			held = down == true
		end
	end
	local mode = LB.gesture and LB.gesture.mode
	if Logic.DragRelease(held, mode) == "ignore" then
		return
	end
	LB.FinishPress(button, mouseButton)
end

function LB.PromoteDrag(button)
	if LB.gesture and LB.gesture.mode == "dragging" then
		return
	end
	if not LB.gesture or LB.gesture.mode ~= "pressed" then
		LB.BeginPress(button, "LeftButton")
	end
	if not LB.gesture or LB.gesture.mode ~= "pressed" then
		return
	end
	local nextState = Logic.Step(LB.gesture, "move")
	if not nextState then
		return
	end
	LB.gesture = nextState
	LB.BeginDrag(button)
end

local function StartDrag(button)
	local press = LB.press
	local x, y = Cursor()
	if not LB.gesture or LB.gesture.mode ~= "pressed" or not press or not press.startX or not x then
		return false
	end
	local dx = x - press.startX
	local dy = y - press.startY
	if dx < 0 then
		dx = -dx
	end
	if dy < 0 then
		dy = -dy
	end
	if dx < Logic.DRAG_THRESHOLD and dy < Logic.DRAG_THRESHOLD then
		return false
	end
	local nextState = Logic.Step(LB.gesture, "move")
	if not nextState then
		return false
	end
	LB.gesture = nextState
	if not (LB.drag and LB.drag.button == button) then
		LB.BeginDrag(button)
	end
	return true
end

function LB.TrackPress(button)
	local press = LB.press
	if not press or press.button ~= button then
		return
	end
	if LB.gesture.mode == "pressed" then
		StartDrag(button)
		return
	end
	if LB.gesture.mode == "dragging" then
		LB.FollowDrag()
	end
end

local function GroupPlaces(place, shiftDown, altDown)
	local char = LB.Char()
	local members = {}
	for index = 1, #char.places do
		local other = char.places[index]
		local key = PlaceKey(other)
		members[index] = {
			key = key,
			x = other.x,
			y = other.y,
			held = other.held == true,
			place = other,
			category = Logic.Category(other.classID, other.quality, other.isQuest),
		}
	end
	local picked = Logic.DragMembers(shiftDown and true or false, altDown and true or false, members, PlaceKey(place), Logic.ICON, char.sameTypeSnap)
	local group = {}
	for index = 1, #picked do
		group[index] = picked[index].place
	end
	if #group == 0 then
		group[1] = place
	end
	return group
end

local function GroupStarts(drag)
	local starts = {}
	for index = 1, #drag.group do
		starts[index] = drag.starts[PlaceKey(drag.group[index])]
	end
	return starts
end

function LB.BeginDrag(button)
	if LB.drag and LB.drag.button == button then
		return
	end
	local place = button.place
	if not place then
		return
	end
	local shiftDown = IsShiftKeyDown and IsShiftKeyDown()
	local altDown = IsAltKeyDown and IsAltKeyDown()
	local group = GroupPlaces(place, shiftDown, altDown)
	local starts = {}
	for index = 1, #group do
		local member = group[index]
		starts[PlaceKey(member)] = { x = member.x, y = member.y, held = member.held == true }
	end
	local x, y = Cursor()
	if not x then
		return
	end
	local host = button.host or button:GetParent()
	local localX, localY = LocalPoint(host, x, y)
	if not localX then
		return
	end
	LB.drag = {
		button = button,
		place = place,
		group = group,
		starts = starts,
		grabX = localX - place.x,
		grabY = localY - place.y,
		host = host,
	}
end

local function Others(group, hostHeld)
	local skip = {}
	for index = 1, #group do
		skip[PlaceKey(group[index])] = true
	end
	local others = {}
	local char = LB.Char()
	for index = 1, #char.places do
		local place = char.places[index]
		if place.held == hostHeld and not skip[PlaceKey(place)] then
			others[#others + 1] = {
				x = place.x,
				y = place.y,
				category = Logic.Category(place.classID, place.quality, place.isQuest),
			}
		end
	end
	return others
end

local function SnapCategory(place)
	if not place or not LB.Char().sameTypeSnap then
		return nil
	end
	return Logic.Category(place.classID, place.quality, place.isQuest)
end

local function MoveGroup(drag, proposedX, proposedY, others, boundsW, boundsH)
	local starts = GroupStarts(drag)
	if #drag.group > 1 then
		local grab = 1
		for index = 1, #drag.group do
			if drag.group[index] == drag.place then
				grab = index
			end
		end
		return Logic.SnapGroup(starts, grab, proposedX, proposedY, others, Logic.ICON, SnapCategory(drag.place), boundsW, boundsH)
	end
	local snappedX, snappedY = Logic.Snap(proposedX, proposedY, others, Logic.ICON, SnapCategory(drag.place), boundsW, boundsH)
	local origin = drag.starts[PlaceKey(drag.place)]
	return Logic.ShiftGroup(origin.x, origin.y, snappedX, snappedY, starts)
end

-- Set false to let the icon follow an outside proposal until release. Saved writes stay on FinishDrag.
local CLAMP_DRAG_PREVIEW = true

function LB.FollowDrag()
	local drag = LB.drag
	if not drag or not drag.host:GetLeft() then
		return
	end
	local x, y = Cursor()
	if not x then
		return
	end
	local localX, localY = LocalPoint(drag.host, x, y)
	if not localX then
		return
	end
	local proposedX = localX - drag.grabX
	local proposedY = localY - drag.grabY
	local boundsW, boundsH
	if LB.frame and drag.host == LB.frame.Content then
		boundsW = plainNumber(drag.host:GetWidth())
		boundsH = plainNumber(drag.host:GetHeight())
	end
	local moved = MoveGroup(drag, proposedX, proposedY, Others(drag.group, drag.place.held), boundsW, boundsH)
	local preview = moved
	if CLAMP_DRAG_PREVIEW and boundsW and boundsH then
		preview = Logic.ClampInside(moved, boundsW, boundsH, Logic.ICON)
	end
	for index = 1, #drag.group do
		local memberButton = ButtonFor(drag.group[index])
		if memberButton then
			memberButton:SetPoint("CENTER", drag.host, "TOPLEFT", preview[index].x, -preview[index].y)
		end
	end
end

function LB.FinishPress(button, mouseButton)
	if mouseButton == "LeftButton" then
		StartDrag(button)
	end
	if LB.tracker then
		LB.tracker:Hide()
	end
	local previous = LB.gesture and LB.gesture.mode
	local nextState = Logic.Step(LB.gesture or { mode = "idle" }, "up")
	if nextState then
		LB.gesture = nextState
	else
		LB.gesture = { mode = "idle" }
	end
	LB.press = nil
	if previous == "dragging" then
		LB.FinishDrag(button)
		return
	end
	if nextState and nextState.click then
		LB.Click(button, mouseButton)
	end
end

local function RestoreDrag(drag)
	for index = 1, #drag.group do
		local member = drag.group[index]
		local start = drag.starts[PlaceKey(member)]
		Logic.Apply(LB.Char(), {
			op = "move_item",
			place = member,
			x = start.x,
			y = start.y,
			held = start.held == true,
		})
	end
end

function LB.FinishDrag(button)
	local drag = LB.drag
	LB.drag = nil
	if not drag then
		return
	end
	local x, y = Cursor()
	local frame = LB.frame
	if not x or not frame or not Inside(frame, x, y) then
		RestoreDrag(drag)
		if x and frame and #drag.group == 1 and LB.PickupSlot then
			LB.PickupSlot(button.bag, button.slot)
		end
		LB.Layout()
		return
	end
	local strip = frame.Strip
	local content = frame.Content
	local overContent = content and Inside(content, x, y)
	local overStrip = (not overContent) and strip and strip:IsShown() and Inside(strip, x, y)
	if not overStrip and not content then
		RestoreDrag(drag)
		LB.Layout()
		return
	end
	local host = overStrip and strip or content
	local localX, localY = LocalPoint(host, x, y)
	if not localX then
		RestoreDrag(drag)
		LB.Layout()
		return
	end
	local proposedX = localX
	local proposedY = localY
	if host == drag.host then
		proposedX = localX - drag.grabX
		proposedY = localY - drag.grabY
	end
	local boundsW, boundsH
	if host == content then
		boundsW = plainNumber(content:GetWidth())
		boundsH = plainNumber(content:GetHeight())
	end
	local moved = MoveGroup(drag, proposedX, proposedY, Others(drag.group, overStrip), boundsW, boundsH)
	local points = moved
	local accepted = true
	if host == content then
		local proposed = {}
		local starts = {}
		for index = 1, #drag.group do
			proposed[index] = { x = moved[index].x, y = moved[index].y }
			local start = drag.starts[PlaceKey(drag.group[index])]
			starts[index] = { x = start.x, y = start.y }
		end
		local contentW = plainNumber(content:GetWidth())
		local contentH = plainNumber(content:GetHeight())
		local neighbors = Others(drag.group, false)
		accepted = Logic.MoveIsValid(proposed, neighbors, contentW, contentH, Logic.ICON)
		points = Logic.CommitMove(proposed, neighbors, contentW, contentH, Logic.ICON, starts)
	end
	for index = 1, #drag.group do
		local member = drag.group[index]
		local held = overStrip
		if host == content and not accepted then
			held = drag.starts[PlaceKey(member)].held == true
		end
		Logic.Apply(LB.Char(), {
			op = "move_item",
			place = member,
			x = points[index].x,
			y = points[index].y,
			held = held,
		})
	end
	LB.Layout()
end

function LB.Click(button, mouseButton)
	local place = button.place
	if not place then
		return
	end
	-- Every right-click on the client's own button is its own OnClick's (use, link, try on, split).
	if button.clientClick and mouseButton == "RightButton" then
		return
	end
	if CursorHasItem and CursorHasItem() and mouseButton == "LeftButton" then
		if LB.PickupSlot then
			LB.PickupSlot(button.bag, button.slot)
		end
		return
	end
	local chat = false
	if ChatEdit_GetActiveWindow and ChatEdit_GetActiveWindow() then
		chat = true
	elseif ChatFrameEditBox and ChatFrameEditBox.IsVisible and ChatFrameEditBox:IsVisible() then
		chat = true
	end
	local mods = {
		shift = IsShiftKeyDown and IsShiftKeyDown(),
		ctrl = IsControlKeyDown and IsControlKeyDown(),
		alt = IsAltKeyDown and IsAltKeyDown(),
		chat = chat,
		split = IsModifiedClick and IsModifiedClick("SPLITSTACK"),
	}
	local action = Logic.ClassifyClick(mouseButton, mods, false, false)
	if action == "link" or action == "dress" then
		local item = LB.ReadSlot(button.bag, button.slot)
		if item and item.link and HandleModifiedItemClick then
			HandleModifiedItemClick(item.link)
		end
		return
	end
	if action == "split" then
		local item = LB.ReadSlot(button.bag, button.slot)
		if item and item.count > 1 then
			button.SplitStack = function(_, split)
				C.Call("SplitContainerItem", button.bag, button.slot, split)
			end
			if OpenStackSplitFrame then
				OpenStackSplitFrame(item.count, button, "BOTTOMRIGHT", "TOPRIGHT")
			end
		end
		return
	end
	if action == "use" then
		local state = Logic.Step(LB.gesture or { mode = "idle" }, "use")
		if not state then
			return
		end
		LB.gesture = state
		-- The client's own button already used it (its OnClick, right button only).
		if button.clientClick then
			return
		end
		C.Call("UseContainerItem", button.bag, button.slot)
		return
	end
	if action == "pickup" and LB.PickupSlot then
		LB.PickupSlot(button.bag, button.slot)
	end
end
