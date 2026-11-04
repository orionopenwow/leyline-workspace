-- Loose Bags by Eric R (MIT, see LICENSE.txt), 0.1.3, ported to the 3.3.5a client for Leyline.
-- The layout rules: pure Lua, no game calls. The original's rules, with the event list and the help, and Leyline's
-- Forever UI geometry (cell pitch, chrome, the window fitted to its items, the money and free slots lines).
local Logic = {}
LooseBagsLogic = Logic

-- Leyline: the window follows Forever UI's bag windows (Bags.lua: 37 px slots, 4 to 5 px apart, the purse at the
-- bottom). ICON is the cell pitch every layout rule uses; the button drawn in it is SLOT wide, so neighbours that snap
-- flush show the same gap as a Forever UI bag. Saved layouts from the old 37 px pitch are scaled once (NormalizeChar).
Logic.ICON = 41
Logic.SLOT = 37
Logic.OLD_PITCH = 37
Logic.GRID = 4
Logic.GAP = 12
Logic.REACH = 0.75
Logic.DRAG_THRESHOLD = 8
Logic.AUTO_COLS = 8
-- The fitted window's room: eight cells and one category gap, so Clean Up's category blocks sit two by two
-- (4 + 4, 3 + 5) in rows from the top left instead of leaving a column empty.
Logic.BAG_WIDTH = Logic.AUTO_COLS * Logic.ICON + Logic.GAP
Logic.MAX_WIDTH = Logic.BAG_WIDTH
Logic.ZONE_MAX = 4
Logic.FRAME_TITLE = "Backpack"
-- Chrome, in window pixels. TOP_BASE: title band and the search strip (Forever UI: header 48 + search strip,
-- the grid starts 75 down). BOTTOM_PAD: the purse (14 up, 13 high) and the gap above it.
Logic.SIDE_INSET = 9
Logic.TOP_BASE = 74
Logic.BAG_ROW = 40
Logic.STRIP_GAP = 20
Logic.PURSE_SIDE = 8
Logic.PURSE_BOTTOM = 14
Logic.PURSE_H = 13
Logic.BOTTOM_PAD = Logic.PURSE_BOTTOM + Logic.PURSE_H + 7
Logic.CHROME_MIN_W = 6 * Logic.ICON + 2 * Logic.SIDE_INSET
Logic.CHROME_MIN_H = Logic.TOP_BASE + Logic.ICON + Logic.BOTTOM_PAD
Logic.FRAME_MAX_W = 800
Logic.FRAME_MAX_H = 800
-- Where a bag opens until the player moves it: bottom right, above the bag bar, as the standard bags
-- (CONTAINER_OFFSET_X / _Y; Forever UI's rightEdge and bottomEdge).
Logic.HOME_RIGHT = 10
Logic.HOME_BOTTOM = 85

Logic.HELP = {
	{ gesture = "Drag", detail = "Every item in the bag can be dragged, including miscellaneous items. Move one icon. A drop near an inner edge snaps flush to that edge, the same way it snaps to a neighbor, then to a 4 pixel grid. The icon can sit against the left and right edges and the top of the bag. The bottom edge sits just above the money line. While you drag, the icon stays inside the bag. The saved place changes only when you let go. A drop that would stack icons, or leave any icon outside the bag, puts every moved icon back where the drag started." },
	{ gesture = "Shift-drag", detail = "Move every icon sitting against the one you grab. They keep their gaps and snap as one group. Grab any icon in the group. The whole group snaps flush to the inner edges, the corners, and other icons, and stays inside the bag. If the group does not fit, they all go back. Dragging a group out of the bag does not pick it up." },
	{ gesture = "Alt-drag", detail = "Move one icon, even if Shift is held." },
	{ gesture = "Drag out", detail = "Drag one icon out of the bag to pick it up. Alt-click picks it up too." },
	{ gesture = "Right-click", detail = "Use the item." },
	{ gesture = "Ctrl-click", detail = "Try the item on." },
	{ gesture = "Shift-click", detail = "Link the item when chat is open. Shift alone does nothing." },
	{ gesture = "Split", detail = "The stack-split key opens the split frame." },
	{ gesture = "Search", detail = "Type in the box under the title. Items whose name or kind does not match go dark. Esc or the small x clears it." },
	{ gesture = "Clean Up Bags", detail = "The Clean Up button at the top right sorts the bag into a tidy grid from the top left and leaves a gap between categories. After that, a shift-drag grabs one category. When Zones is on, each category grows from the corner or edge nearest its tag, and the space between tags stays." },
	{ gesture = "Auto-place", detail = "Put new items with their category. A tick means on. A new item keeps the shape you gave its group. It fills a gap in the group first, then finishes the last row, then starts a new row at the group's usual width. A group against the right or bottom edge grows the other way. If you sell an item and buy it back, it returns to its old spot. A category with no items yet goes to its zone tag when Zones is on. Auto-place never moves an item you placed, including the Hearthstone. Clean Up Bags still sorts every item into its zone." },
	{ gesture = "Auto-order", detail = "On, Clean Up sorts the items inside each category. Off, Clean Up can move a category to its zone or push it clear of another, but keeps its shape and the order you set. New items then go to the nearest free spot beside their category. It starts on." },
	{ gesture = "Same-type snap", detail = "Off, any item can snap or group with any other. On, a drop snaps only to the same category. A drop on a different category goes back." },
	{ gesture = "Zones", detail = "On, each category grows from the corner or edge nearest its tag. Icons in a category stay tight on the grid. The space you left between tags stays, snapped to that grid. Off, Clean Up packs the bag the usual way." },
	{ gesture = "Edit zones", detail = "Edit zones, in the settings, hides the items and shows one tag per category. Drag a tag to say where that category sits. A tag only drags. It does not click. While you drag, the tag hides and only the box shows, as a soft glow. The box is where that category will land, sized to its item count, one slot for an empty category. The box snaps to the icon grid and sits flush on any inner edge or corner. Clean Up puts the items in that same box. Past the edge, the box stays at the nearest spot inside. A spot where the tag would cover another tag is skipped. On drop the tag shows again at the snapped spot. A tag left outside after the bag gets smaller comes back inside at the nearest free spot. Click Edit zones again to show the items." },
	{ gesture = "New loot", detail = "New items wait on the New items strip at the bottom of the bag until you drag them into place, or turn on Auto-place. With Auto-place on they go straight to their category." },
	{ gesture = "Bag slots", detail = "The bag button beside the settings cog shows your four bag slots. Point at one to light up only its items. Drop a bag on a slot to equip it." },
	{ gesture = "Free slots", detail = "The number at the bottom left counts the empty slots in your bags. The money line beside it shows your gold, silver and copper." },
	{ gesture = "Settings", detail = "The cog left of Clean Up Bags opens the settings: Auto-place, Auto-order, Same-type snap, Zones, Edit zones, Lock size, Fit to items and this help. After login or a reload they start closed." },
	{ gesture = "First open", detail = "The first time this character has no saved layout, Loose Bags sorts every item the same way as Clean Up, from the top left. The window fits its items: a few items make a small bag, and it grows as items come, one free row below the last. Later logins and reloads leave the bag as you left it." },
	{ gesture = "Resize", detail = "Drag the corner grip to choose your own width and height. The bag then stops fitting itself to its items. Fit to items, in the settings, goes back. Icons stay inside the new size without sitting on each other. The size is remembered for this character. Login, camp, and reload restore that size. An icon that would sit outside the bag comes back inside. Other icons stay where you put them." },
	{ gesture = "Lock size", detail = "Freezes the bag's size. The grip hides until you unlock. Drag the title or the portrait to move the bag. It opens at the bottom right, above the bag bar, until you move it; then it remembers where you left it. A new character starts unlocked. Login, camp, and reload keep the size, the position, the lock, and each item's spot. Opening the bag before the items have loaded does not clear those spots. An unlocked bag does not resize itself." },
	{ gesture = "/lbags", detail = "Toggle the bag. Login and reload leave it closed until you press B. /lbags on brings Loose Bags back. /lbags off goes back to the standard bags. /lbags reset clears the layout and the size. /lbags sort cleans up. /lbags help opens this panel." },
}

Logic.SIDE_PAD = Logic.SIDE_INSET * 2

local function plainHeight(value)
	if type(issecretvalue) == "function" and issecretvalue(value) then
		return nil
	end
	if type(value) ~= "number" or value < 1 then
		return nil
	end
	return value
end

function Logic.HelpRowHeight(gestureHeight, detailHeight)
	local gesture = plainHeight(gestureHeight) or 16
	local detail = plainHeight(detailHeight) or 16
	local textH = gesture
	if detail > textH then
		textH = detail
	end
	if textH < 20 then
		textH = 20
	end
	return textH
end

function Logic.HelpLayout(heights, viewLimit)
	heights = heights or {}
	local rows = {}
	local y = 0
	for index = 1, #heights do
		local row = heights[index] or {}
		local textH = Logic.HelpRowHeight(row.gesture, row.detail)
		rows[index] = { y = y, height = textH }
		y = y + textH + 4
	end
	if y < 1 then
		y = 1
	end
	local full = 36 + y + 12
	local height = full
	local limit = plainHeight(viewLimit)
	if limit and full > limit then
		height = limit
	end
	local visible = height - 48
	if visible < 1 then
		visible = 1
	end
	local maxScroll = y - visible
	if maxScroll < 0 then
		maxScroll = 0
	end
	return {
		rows = rows,
		content = y,
		height = height,
		visible = visible,
		maxScroll = maxScroll,
	}
end

local function plainSize(value)
	if type(issecretvalue) == "function" and issecretvalue(value) then
		return nil
	end
	if type(value) ~= "number" or value ~= value or value <= 0 then
		return nil
	end
	return value
end

function Logic.LockTextures(locked)
	if locked then
		return "Interface\\Buttons\\LockButton-Locked-Up", "Interface\\Buttons\\LockButton-Locked-Down"
	end
	return "Interface\\Buttons\\LockButton-Unlocked-Up", "Interface\\Buttons\\LockButton-Unlocked-Down"
end

function Logic.ColumnCount(span, icon)
	icon = icon or Logic.ICON
	local cols = math.floor(span / icon)
	if cols < 1 then
		cols = 1
	end
	return cols
end

function Logic.RowCount(count, cols)
	if type(count) ~= "number" or count < 1 then
		return 1
	end
	if type(cols) ~= "number" or cols < 1 then
		cols = 1
	end
	return math.ceil(count / cols)
end

function Logic.MinHeightForWidth(width, bodyCount, heldCount, bar)
	bar = bar or 0
	local contentW = width - Logic.SIDE_PAD
	if contentW < Logic.ICON then
		contentW = Logic.ICON
	end
	local cols = Logic.ColumnCount(contentW, Logic.ICON)
	local bodyH = Logic.RowCount(bodyCount, cols) * Logic.ICON
	local heldH = 0
	if type(heldCount) == "number" and heldCount > 0 then
		heldH = Logic.STRIP_GAP + Logic.RowCount(heldCount, cols) * Logic.ICON
	end
	local height = Logic.TOP_BASE + bar + bodyH + heldH + Logic.BOTTOM_PAD
	local chrome = Logic.CHROME_MIN_H + bar
	if height < chrome then
		height = chrome
	end
	return height
end

-- (bar: the bag slots row's height when it shows; the settings sit in a drawer, so the width no longer depends on it.)
function Logic.MinFrameWidth(bar)
	return Logic.CHROME_MIN_W
end

function Logic.ClampSize(width, height, bodyCount, heldCount, bar, screenW, screenH)
	bodyCount = bodyCount or 0
	heldCount = heldCount or 0
	bar = bar or 0
	local maxW = Logic.FRAME_MAX_W
	local maxH = Logic.FRAME_MAX_H
	screenW = plainSize(screenW)
	screenH = plainSize(screenH)
	if screenW and screenW < maxW then
		maxW = screenW
	end
	if screenH and screenH < maxH then
		maxH = screenH
	end
	local minW = Logic.MinFrameWidth(bar)
	if minW > maxW then
		minW = maxW
	end
	width = plainSize(width) or minW
	if width < minW then
		width = minW
	end
	if width > maxW then
		width = maxW
	end
	local minH = Logic.MinHeightForWidth(width, bodyCount, heldCount, bar)
	if minH > maxH then
		local try = width
		while try < maxW do
			local nextW = try + Logic.ICON
			if nextW > maxW then
				nextW = maxW
			end
			try = nextW
			local nextH = Logic.MinHeightForWidth(try, bodyCount, heldCount, bar)
			width = try
			minH = nextH
			if nextH <= maxH or try >= maxW then
				break
			end
		end
		if minH > maxH then
			minH = maxH
		end
	end
	height = plainSize(height) or minH
	if height < minH then
		height = minH
	end
	if height > maxH then
		height = maxH
	end
	return width, height
end

function Logic.LayoutSize(savedW, savedH, packW, packH, bodyCount, heldCount, bar, screenW, screenH)
	local width = packW + Logic.SIDE_PAD
	local height = Logic.TOP_BASE + (bar or 0) + packH + Logic.BOTTOM_PAD
	if type(heldCount) == "number" and heldCount > 0 then
		local holding = Logic.HoldingPositions(heldCount, Logic.ICON, packW)
		height = height + Logic.STRIP_GAP + holding.height
	end
	local remembered = type(savedW) == "number" and type(savedH) == "number"
	if remembered then
		width = savedW
		height = savedH
	end
	width, height = Logic.ClampSize(width, height, bodyCount, heldCount, bar, screenW, screenH)
	return width, height, remembered
end

function Logic.KeepFrame(row)
	if type(row) ~= "table" then
		return nil
	end
	if type(row.width) ~= "number" or type(row.height) ~= "number" then
		return nil
	end
	if row.width <= 0 or row.height <= 0 then
		return nil
	end
	return row.width, row.height, row.locked == true, row.x, row.y, row.point, row.relPoint
end

function Logic.RememberSize(row, width, height)
	if type(row) ~= "table" or Logic.KeepFrame(row) then
		return false
	end
	if type(width) ~= "number" or type(height) ~= "number" or width <= 0 or height <= 0 then
		return false
	end
	row.width = width
	row.height = height
	return true
end

function Logic.ResizeMinimum(bar)
	bar = bar or 0
	return Logic.MinFrameWidth(bar), Logic.CHROME_MIN_H + bar
end

function Logic.ResizeLimits(bar, frameW, frameH, screenW, screenH)
	local minW, minH = Logic.ResizeMinimum(bar)
	local maxW = Logic.FRAME_MAX_W
	local maxH = Logic.FRAME_MAX_H
	screenW = plainSize(screenW)
	screenH = plainSize(screenH)
	if screenW and screenW < maxW then
		maxW = screenW
	end
	if screenH and screenH < maxH then
		maxH = screenH
	end
	if type(frameW) == "number" and frameW > 0 and frameW < minW then
		minW = frameW
	end
	if type(frameH) == "number" and frameH > 0 and frameH < minH then
		minH = frameH
	end
	if type(frameW) == "number" and frameW > maxW then
		maxW = frameW
	end
	if type(frameH) == "number" and frameH > maxH then
		maxH = frameH
	end
	if minW > maxW then
		minW = maxW
	end
	if minH > maxH then
		minH = maxH
	end
	return minW, minH, maxW, maxH
end

function Logic.ContentBox(frameW, frameH, bar, heldCount)
	bar = bar or 0
	local contentW = frameW - Logic.SIDE_PAD
	if contentW < Logic.ICON then
		contentW = Logic.ICON
	end
	local cols = Logic.ColumnCount(contentW, Logic.ICON)
	local heldH = 0
	if type(heldCount) == "number" and heldCount > 0 then
		heldH = Logic.STRIP_GAP + Logic.RowCount(heldCount, cols) * Logic.ICON
	end
	local contentH = frameH - Logic.TOP_BASE - bar - Logic.BOTTOM_PAD - heldH
	if contentH < Logic.ICON then
		contentH = Logic.ICON
	end
	return contentW, contentH
end

local function iconHits(x, y, accepted, icon)
	local limit = icon - 1
	for index = 1, #accepted do
		local other = accepted[index]
		local dx = x - other.x
		local dy = y - other.y
		if dx < 0 then
			dx = -dx
		end
		if dy < 0 then
			dy = -dy
		end
		if dx < limit and dy < limit then
			return true
		end
	end
	return false
end

function Logic.TooClose(x, y, points, icon)
	icon = icon or Logic.ICON
	points = points or {}
	local slack = Logic.GRID
	local touch = icon + slack
	for index = 1, #points do
		local other = points[index]
		local dx = x - other.x
		local dy = y - other.y
		if dx < 0 then
			dx = -dx
		end
		if dy < 0 then
			dy = -dy
		end
		if dx <= touch and dy <= slack then
			return true
		end
		if dy <= touch and dx <= slack then
			return true
		end
	end
	return false
end

function Logic.CategoryTouch(group, others, icon)
	group = group or {}
	for index = 1, #group do
		local point = group[index]
		if Logic.TooClose(point.x, point.y, others, icon) then
			return true
		end
	end
	return false
end

function Logic.IconInside(x, y, contentW, contentH, icon)
	if type(x) ~= "number" or type(y) ~= "number" then
		return false
	end
	if type(contentW) ~= "number" or type(contentH) ~= "number" then
		return false
	end
	local half = (icon or Logic.ICON) / 2
	if x < half or y < half then
		return false
	end
	if x + half > contentW + 0.001 or y + half > contentH + 0.001 then
		return false
	end
	return true
end

function Logic.PairOverlap(ax, ay, bx, by, icon)
	local limit = (icon or Logic.ICON) - 1
	local dx = ax - bx
	local dy = ay - by
	if dx < 0 then
		dx = -dx
	end
	if dy < 0 then
		dy = -dy
	end
	return dx < limit and dy < limit
end

function Logic.MoveIsValid(proposed, others, contentW, contentH, icon)
	proposed = proposed or {}
	others = others or {}
	icon = icon or Logic.ICON
	if #proposed == 0 then
		return false
	end
	for index = 1, #proposed do
		local item = proposed[index]
		if not Logic.IconInside(item.x, item.y, contentW, contentH, icon) then
			return false
		end
		for otherIndex = 1, index - 1 do
			local other = proposed[otherIndex]
			if Logic.PairOverlap(item.x, item.y, other.x, other.y, icon) then
				return false
			end
		end
		for otherIndex = 1, #others do
			local other = others[otherIndex]
			if Logic.PairOverlap(item.x, item.y, other.x, other.y, icon) then
				return false
			end
		end
	end
	return true
end

function Logic.CommitMove(proposed, others, contentW, contentH, icon, starts)
	if Logic.MoveIsValid(proposed, others, contentW, contentH, icon) then
		return proposed
	end
	return starts
end

function Logic.ClampInside(points, contentW, contentH, icon)
	points = points or {}
	icon = icon or Logic.ICON
	if type(contentW) ~= "number" or type(contentH) ~= "number" then
		return points
	end
	if contentW < icon or contentH < icon or #points == 0 then
		return points
	end
	local half = icon / 2
	local minX, minY, maxX, maxY
	for index = 1, #points do
		local point = points[index]
		if type(point) ~= "table" or type(point.x) ~= "number" or type(point.y) ~= "number" then
			return points
		end
		if not minX or point.x < minX then
			minX = point.x
		end
		if not maxX or point.x > maxX then
			maxX = point.x
		end
		if not minY or point.y < minY then
			minY = point.y
		end
		if not maxY or point.y > maxY then
			maxY = point.y
		end
	end
	local lowX, highX = half, contentW - half
	local lowY, highY = half, contentH - half
	local dx, dy = 0, 0
	if minX < lowX then
		dx = lowX - minX
	elseif maxX > highX then
		dx = highX - maxX
	end
	if minY < lowY then
		dy = lowY - minY
	elseif maxY > highY then
		dy = highY - maxY
	end
	local function copy(shiftX, shiftY, limit)
		local moved = {}
		for index = 1, #points do
			local point = points[index]
			local x = point.x + shiftX
			local y = point.y + shiftY
			if limit then
				if x < lowX then
					x = lowX
				elseif x > highX then
					x = highX
				end
				if y < lowY then
					y = lowY
				elseif y > highY then
					y = highY
				end
			end
			moved[index] = { x = x, y = y }
		end
		return moved
	end
	local shifted = copy(dx, dy, false)
	for index = 1, #shifted do
		local point = shifted[index]
		if not Logic.IconInside(point.x, point.y, contentW, contentH, icon) then
			return copy(dx, dy, true)
		end
	end
	return shifted
end

function Logic.BodyOverlaps(points, icon)
	points = points or {}
	for index = 1, #points do
		local item = points[index]
		if type(item) == "table" and not item.held then
			for otherIndex = 1, index - 1 do
				local other = points[otherIndex]
				if type(other) == "table" and not other.held and Logic.PairOverlap(item.x, item.y, other.x, other.y, icon) then
					return true
				end
			end
		end
	end
	return false
end

function Logic.PlacesOk(places, icon)
	if Logic.BodyOverlaps(places, icon) then
		return false
	end
	places = places or {}
	icon = icon or Logic.ICON
	local groups = {}
	for index = 1, #places do
		local place = places[index]
		if type(place) == "table" and not place.held then
			local category = place.category
			if type(category) ~= "string" then
				category = Logic.Category(place.classID, place.quality, place.isQuest)
			end
			groups[category] = groups[category] or {}
			groups[category][#groups[category] + 1] = place
		end
	end
	local placed = {}
	for index = 1, #Logic.CATEGORIES do
		local group = groups[Logic.CATEGORIES[index]]
		if group then
			if Logic.CategoryTouch(group, placed, icon) then
				return false
			end
			for itemIndex = 1, #group do
				placed[#placed + 1] = group[itemIndex]
			end
		end
	end
	return true
end

function Logic.FitPlaces(places, contentW, contentH, icon)
	icon = icon or Logic.ICON
	places = places or {}
	local half = icon / 2
	local maxX = contentW - half
	local maxY = contentH - half
	if maxX < half then
		maxX = half
	end
	if maxY < half then
		maxY = half
	end
	local function clampCenter(x, y)
		if type(x) ~= "number" then
			x = half
		end
		if type(y) ~= "number" then
			y = half
		end
		if x < half then
			x = half
		end
		if y < half then
			y = half
		end
		if x > maxX then
			x = maxX
		end
		if y > maxY then
			y = maxY
		end
		return x, y
	end
	local body = {}
	for index = 1, #places do
		local place = places[index]
		if type(place) == "table" and not place.held then
			body[#body + 1] = place
		end
	end
	table.sort(body, function(a, b)
		local ay = type(a.y) == "number" and a.y or 0
		local by = type(b.y) == "number" and b.y or 0
		if ay ~= by then
			return ay < by
		end
		local ax = type(a.x) == "number" and a.x or 0
		local bx = type(b.x) == "number" and b.x or 0
		if ax ~= bx then
			return ax < bx
		end
		return tostring(a.itemID) < tostring(b.itemID)
	end)
	local cols = Logic.ColumnCount(contentW, icon)
	local rows = Logic.ColumnCount(contentH, icon)
	local accepted = {}
	local function foreignClose(x, y, category)
		local foreign = {}
		for index = 1, #accepted do
			if accepted[index].category ~= category then
				foreign[#foreign + 1] = accepted[index]
			end
		end
		return Logic.TooClose(x, y, foreign, icon)
	end
	local function nearest(x, y, category)
		local bestX, bestY, bestDist
		local row
		for row = 0, rows - 1 do
			local col
			for col = 0, cols - 1 do
				local cx = half + col * icon
				local cy = half + row * icon
				if cx <= maxX and cy <= maxY and not iconHits(cx, cy, accepted, icon) and not foreignClose(cx, cy, category) then
					local dx = cx - x
					local dy = cy - y
					local dist = dx * dx + dy * dy
					local better = bestDist == nil or dist < bestDist
					if not better and dist == bestDist then
						better = cy < bestY or (cy == bestY and cx < bestX)
					end
					if better then
						bestDist = dist
						bestX = cx
						bestY = cy
					end
				end
			end
		end
		return bestX, bestY
	end
	local results = {}
	for index = 1, #body do
		local place = body[index]
		local category = Logic.Category(place.classID, place.quality, place.isQuest)
		local rawX = place.x
		local rawY = place.y
		local x, y = clampCenter(rawX, rawY)
		local outside = not Logic.IconInside(rawX, rawY, contentW, contentH, icon)
		if iconHits(x, y, accepted, icon) or outside then
			local nextX, nextY = nearest(x, y, category)
			if nextX then
				x = nextX
				y = nextY
			else
				x = rawX
				y = rawY
			end
		end
		accepted[#accepted + 1] = { x = x, y = y, category = category }
		results[#results + 1] = { place = place, x = x, y = y, category = category }
	end
	return results
end

function Logic.BringInside(places, contentW, contentH, icon)
	icon = icon or Logic.ICON
	places = places or {}
	local half = icon / 2
	local maxX = (contentW or 0) - half
	local maxY = (contentH or 0) - half
	if maxX < half then
		maxX = half
	end
	if maxY < half then
		maxY = half
	end
	local function clampCenter(x, y)
		if type(x) ~= "number" then
			x = half
		end
		if type(y) ~= "number" then
			y = half
		end
		if x < half then
			x = half
		end
		if y < half then
			y = half
		end
		if x > maxX then
			x = maxX
		end
		if y > maxY then
			y = maxY
		end
		return x, y
	end
	local accepted = {}
	local moves = {}
	for index = 1, #places do
		local place = places[index]
		if type(place) == "table" and not place.held and Logic.IconInside(place.x, place.y, contentW, contentH, icon) then
			accepted[#accepted + 1] = { x = place.x, y = place.y }
		end
	end
	local cols = Logic.ColumnCount(contentW or 0, icon)
	local rows = Logic.ColumnCount(contentH or 0, icon)
	for index = 1, #places do
		local place = places[index]
		if type(place) == "table" and not place.held and not Logic.IconInside(place.x, place.y, contentW, contentH, icon) then
			local fromX = type(place.x) == "number" and place.x or half
			local fromY = type(place.y) == "number" and place.y or half
			local bestX, bestY, bestDist
			local row
			for row = 0, rows - 1 do
				local col
				for col = 0, cols - 1 do
					local cx = half + col * icon
					local cy = half + row * icon
					if cx <= maxX and cy <= maxY and not iconHits(cx, cy, accepted, icon) then
						local dx = cx - fromX
						local dy = cy - fromY
						local dist = dx * dx + dy * dy
						local better = bestDist == nil or dist < bestDist
						if not better and dist == bestDist then
							better = cy < bestY or (cy == bestY and cx < bestX)
						end
						if better then
							bestDist = dist
							bestX = cx
							bestY = cy
						end
					end
				end
			end
			local x, y
			if bestX then
				x, y = bestX, bestY
			else
				x, y = clampCenter(fromX, fromY)
			end
			accepted[#accepted + 1] = { x = x, y = y }
			moves[#moves + 1] = { place = place, x = x, y = y }
		end
	end
	return moves
end

local function shiftPoints(group, dy)
	local points = {}
	for index = 1, #group do
		local place = group[index]
		points[index] = { x = place.x, y = place.y + dy }
	end
	return points
end

function Logic.SeparateCategories(places, icon)
	places = places or {}
	icon = icon or Logic.ICON
	local groups = {}
	for index = 1, #places do
		local place = places[index]
		if type(place) == "table" and not place.held then
			local category = Logic.Category(place.classID, place.quality, place.isQuest)
			groups[category] = groups[category] or {}
			groups[category][#groups[category] + 1] = place
		end
	end
	local placed = {}
	local step = Logic.GRID + 1
	for index = 1, #Logic.CATEGORIES do
		local group = groups[Logic.CATEGORIES[index]]
		if group then
			local dy = 0
			local guard = 0
			while Logic.CategoryTouch(shiftPoints(group, dy), placed, icon) and guard < 80 do
				dy = dy + step
				guard = guard + 1
			end
			if dy ~= 0 then
				for itemIndex = 1, #group do
					group[itemIndex].y = group[itemIndex].y + dy
				end
			end
			for itemIndex = 1, #group do
				placed[#placed + 1] = { x = group[itemIndex].x, y = group[itemIndex].y }
			end
		end
	end
	return places
end

function Logic.GrownSize(width, height, places, bar, screenW, screenH)
	places = places or {}
	bar = bar or 0
	local bodyCount = 0
	local heldCount = 0
	local bottom = 0
	local right = 0
	for index = 1, #places do
		local place = places[index]
		if place.held then
			heldCount = heldCount + 1
		else
			bodyCount = bodyCount + 1
			local edgeX = place.x + Logic.ICON / 2
			local edgeY = place.y + Logic.ICON / 2
			if edgeX > right then
				right = edgeX
			end
			if edgeY > bottom then
				bottom = edgeY
			end
		end
	end
	local contentW, contentH = Logic.ContentBox(width, height, bar, heldCount)
	if right > contentW then
		width = width + (right - contentW)
	end
	if bottom > contentH then
		height = height + (bottom - contentH)
	end
	return Logic.ClampSize(width, height, bodyCount, heldCount, bar, screenW, screenH)
end

-- 3.3.5a has no BAG_UPDATE_DELAYED: LooseBags.lua folds each burst of BAG_UPDATE into one "delayed" pass on the
-- next frame. ITEM_LOCK_CHANGED greys a picked-up item, BAG_UPDATE_COOLDOWN runs the cooldown spirals.
Logic.Events = {
	"ADDON_LOADED",
	"PLAYER_LOGIN",
	"PLAYER_ENTERING_WORLD",
	"BAG_UPDATE",
	"ITEM_LOCK_CHANGED",
	"BAG_UPDATE_COOLDOWN",
	"PLAYER_REGEN_ENABLED",
	"PLAYER_MONEY",
}

Logic.CATEGORIES = {
	"consumable",
	"armor",
	"weapon",
	"trade",
	"quest",
	"junk",
	"other",
}

Logic.CATEGORY_LABEL = {
	consumable = "Consumables",
	armor = "Armor",
	weapon = "Weapons",
	trade = "Reagents",
	quest = "Quest",
	junk = "Junk",
	other = "Other",
}

Logic.TAG_W = 100
Logic.TAG_H = 22

local RANK = {
	consumable = 1,
	armor = 2,
	weapon = 3,
	trade = 4,
	quest = 5,
	junk = 6,
	other = 7,
}

local DEFAULT_ZONES = {
	consumable = 1,
	armor = 1,
	weapon = 2,
	trade = 2,
	quest = 3,
	junk = 3,
	other = 4,
}

local ANCHORS = {
	CENTER = true,
	TOP = true,
	BOTTOM = true,
	LEFT = true,
	RIGHT = true,
	TOPLEFT = true,
	TOPRIGHT = true,
	BOTTOMLEFT = true,
	BOTTOMRIGHT = true,
}

local function copyHomes(source)
	local homes = {}
	if type(source) ~= "table" then
		return homes
	end
	for index = 1, #Logic.CATEGORIES do
		local category = Logic.CATEGORIES[index]
		local home = source[category]
		if type(home) == "table" and type(home.x) == "number" and type(home.y) == "number" then
			homes[category] = { x = home.x, y = home.y }
		end
	end
	return homes
end

local function copyZones(source)
	local zones = {}
	for index = 1, #Logic.CATEGORIES do
		local category = Logic.CATEGORIES[index]
		local value = source and source[category]
		if type(value) ~= "number" or value < 1 or value > Logic.ZONE_MAX then
			value = DEFAULT_ZONES[category]
		end
		zones[category] = math.floor(value)
	end
	return zones
end

function Logic.DefaultZones()
	return copyZones(DEFAULT_ZONES)
end

function Logic.CycleZone(zones, category)
	local nextZones = copyZones(zones)
	if not RANK[category] then
		return nextZones
	end
	local value = nextZones[category] + 1
	if value > Logic.ZONE_MAX then
		value = 1
	end
	nextZones[category] = value
	return nextZones
end

function Logic.Category(classID, quality, isQuest)
	if quality == 0 then
		return "junk"
	end
	if isQuest or classID == 12 then
		return "quest"
	end
	if classID == 0 then
		return "consumable"
	end
	if classID == 4 then
		return "armor"
	end
	if classID == 2 then
		return "weapon"
	end
	if classID == 7 or classID == 5 or classID == 9 or classID == 3 or classID == 8 then
		return "trade"
	end
	return "other"
end

function Logic.CharKey(name, realm)
	if type(name) ~= "string" or name == "" then
		return nil
	end
	if type(realm) ~= "string" then
		realm = ""
	end
	return name .. "-" .. realm
end

local function numberOr(value, fallback)
	if type(value) == "number" then
		return value
	end
	return fallback
end

local function anchorOr(value)
	if type(value) == "string" and ANCHORS[value] then
		return value
	end
	return "CENTER"
end

function Logic.NormalizeAccount(db)
	if type(db) ~= "table" then
		db = {}
	end
	if type(db.chars) ~= "table" then
		db.chars = {}
	end
	if type(db.enabled) ~= "boolean" then
		db.enabled = true
	end
	return db
end

function Logic.TogglePaint(on)
	if on then
		return "PUSHED", "GameFontHighlight", true
	end
	return "NORMAL", "GameFontNormal", false
end

function Logic.PaintToggleFont(button, on)
	local _, fontName = Logic.TogglePaint(on)
	local fontObject = _G[fontName] or fontName
	if button.SetNormalFontObject then
		button:SetNormalFontObject(fontObject)
	end
	local font = button.GetFontString and button:GetFontString()
	if font and font.SetFontObject then
		font:SetFontObject(fontObject)
	end
end

function Logic.BeginSession(row)
	row = Logic.NormalizeChar(row)
	row.collapsed = true
	return row
end

function Logic.ShouldOpen(reason)
	return reason == "toggle" or reason == "slash"
end

function Logic.NormalizeChar(row)
	if type(row) ~= "table" then
		row = {}
	end
	row.point = anchorOr(row.point)
	row.relPoint = anchorOr(row.relPoint)
	row.x = numberOr(row.x, 0)
	row.y = numberOr(row.y, 120)
	if type(row.collapsed) ~= "boolean" then
		row.collapsed = true
	end
	row.locked = row.locked == true
	if type(row.width) ~= "number" or row.width <= 0 then
		row.width = nil
	end
	if type(row.height) ~= "number" or row.height <= 0 then
		row.height = nil
	end
	row.autoPlace = row.autoPlace == true
	row.autoOrder = row.autoOrder ~= false
	row.sameTypeSnap = row.sameTypeSnap == true
	row.useZones = row.useZones == true
	row.zones = nil
	row.homes = copyHomes(row.homes)
	local places = {}
	local seen = {}
	local raw = row.places
	if type(raw) == "table" then
		for index = 1, #raw do
			local place = raw[index]
			if type(place) == "table" and type(place.itemID) == "number" then
				local itemID = place.itemID
				local inst = place.inst
				if type(inst) ~= "number" or inst < 1 or seen[itemID .. ":" .. inst] then
					inst = nil
				end
				if not inst then
					local nextInst = 1
					while seen[itemID .. ":" .. nextInst] do
						nextInst = nextInst + 1
					end
					inst = nextInst
				end
				seen[itemID .. ":" .. inst] = true
				place.inst = inst
				place.x = numberOr(place.x, Logic.ICON / 2)
				place.y = numberOr(place.y, Logic.ICON / 2)
				place.bag = numberOr(place.bag, 0)
				place.slot = numberOr(place.slot, 0)
				place.count = numberOr(place.count, 1)
				place.held = place.held == true
				if type(place.classID) ~= "number" then
					place.classID = nil
				end
				if type(place.quality) ~= "number" then
					place.quality = nil
				end
				place.isQuest = place.isQuest == true
				places[#places + 1] = place
			end
		end
	end
	row.places = places
	Logic.MigratePitch(row)
	row.bagRow = row.bagRow == true
	row.moved = row.moved == true
	if row.firstRun ~= true then
		if Logic.FirstRunPending(row) then
			row.firstRun = nil
		else
			row.firstRun = true
		end
	end
	return row
end

-- Leyline: a layout saved at another cell pitch (the port's first 37 px) is scaled to this one once, so icons keep
-- their arrangement and never overlap. Its old size was made for the old pitch: the bag fits its items again. A bag
-- still at the old default spot (screen centre) opens where the standard bags do from now on.
function Logic.MigratePitch(row)
	if type(row) ~= "table" or row.pitch == Logic.ICON then
		return row
	end
	local old = row.pitch
	if type(old) ~= "number" or old <= 0 then
		old = Logic.OLD_PITCH
	end
	local k = Logic.ICON / old
	local places = type(row.places) == "table" and row.places or {}
	for index = 1, #places do
		local place = places[index]
		if type(place.x) == "number" then
			place.x = place.x * k
		end
		if type(place.y) == "number" then
			place.y = place.y * k
		end
	end
	if type(row.homes) == "table" then
		for _, home in pairs(row.homes) do
			if type(home) == "table" and type(home.x) == "number" and type(home.y) == "number" then
				home.x = home.x * k
				home.y = home.y * k
			end
		end
	end
	if row.moved == nil then
		local x, y = row.x, row.y
		local atOldDefault = (row.point == nil or row.point == "CENTER") and (row.relPoint == nil or row.relPoint == "CENTER")
			and (x == nil or x == 0) and (y == nil or y == 120)
		row.moved = not atOldDefault
	end
	row.width = nil
	row.height = nil
	row.pitch = Logic.ICON
	return row
end

-- The window fitted to its items (no size chosen with the grip): the default columns wide, as tall as the lowest
-- item plus one free row to drop into, plus the new-loot strip. bar: the bag slots row; minRows: a floor (zone
-- editing wants room for its tags). Returns frame width and height, then the content box.
function Logic.AutoSize(places, bar, screenW, screenH, minRows)
	places = places or {}
	bar = bar or 0
	local right = 0
	local bottom = 0
	local held = 0
	for index = 1, #places do
		local place = places[index]
		if place.held then
			held = held + 1
		elseif type(place.x) == "number" and type(place.y) == "number" then
			local edgeX = place.x + Logic.ICON / 2
			local edgeY = place.y + Logic.ICON / 2
			if edgeX > right then
				right = edgeX
			end
			if edgeY > bottom then
				bottom = edgeY
			end
		end
	end
	local contentW = Logic.BAG_WIDTH
	if right > contentW then
		contentW = math.ceil(right)
	end
	local contentH = Logic.ICON
	if bottom > 0 then
		contentH = math.ceil(bottom) + Logic.ICON
	end
	local floor = (type(minRows) == "number" and minRows or 1) * Logic.ICON
	if contentH < floor then
		contentH = floor
	end
	local maxW = Logic.FRAME_MAX_W
	local maxH = Logic.FRAME_MAX_H
	screenW = plainSize(screenW)
	screenH = plainSize(screenH)
	if screenW and screenW < maxW then
		maxW = screenW
	end
	if screenH and screenH < maxH then
		maxH = screenH
	end
	local frameW = contentW + Logic.SIDE_PAD
	if frameW < Logic.CHROME_MIN_W then
		frameW = Logic.CHROME_MIN_W
	end
	if frameW > maxW then
		frameW = maxW
	end
	local cols = Logic.ColumnCount(frameW - Logic.SIDE_PAD, Logic.ICON)
	local heldH = 0
	if held > 0 then
		heldH = Logic.STRIP_GAP + Logic.RowCount(held, cols) * Logic.ICON
	end
	local frameH = Logic.TOP_BASE + bar + contentH + heldH + Logic.BOTTOM_PAD
	if frameH > maxH then
		frameH = maxH
	end
	local boxW, boxH = Logic.ContentBox(frameW, frameH, bar, held)
	return frameW, frameH, boxW, boxH
end

-- The bag's spot until the player moves it: bottom right, above the bag bar, left of any shown right-hand bars.
function Logic.HomePoint(rightBars)
	local right = Logic.HOME_RIGHT
	if type(rightBars) == "number" and rightBars > 0 then
		right = right + rightBars
	end
	return "BOTTOMRIGHT", "BOTTOMRIGHT", -right, Logic.HOME_BOTTOM
end

-- Copper to the three coins (all three always shown on the money line).
function Logic.MoneyParts(money)
	if type(money) ~= "number" or money ~= money or money < 0 then
		money = 0
	end
	money = math.floor(money)
	local gold = math.floor(money / 10000)
	local silver = math.floor((money - gold * 10000) / 100)
	local copper = money - gold * 10000 - silver * 100
	return gold, silver, copper
end

-- 1234567 -> "1,234,567" (the gold amount; the client of this era has no number grouping of its own).
function Logic.Grouped(number)
	local text = tostring(math.floor(tonumber(number) or 0))
	local sign = ""
	if string.sub(text, 1, 1) == "-" then
		sign = "-"
		text = string.sub(text, 2)
	end
	local out = ""
	while string.len(text) > 3 do
		out = "," .. string.sub(text, -3) .. out
		text = string.sub(text, 1, -4)
	end
	return sign .. text .. out
end

-- The free slots counter: "21 free", and its tooltip line.
function Logic.FreeText(free, total)
	free = type(free) == "number" and free or 0
	total = type(total) == "number" and total or 0
	return free .. " free", free .. " of " .. total .. " bag slots are free."
end

-- Search: a match on the item's name, its kind or its sub-kind (as Forever UI's bag search). Empty text matches all.
function Logic.SearchMatch(text, name, kind, subKind)
	if type(text) ~= "string" or text == "" then
		return true
	end
	local fields = { name, kind, subKind }
	for index = 1, 3 do
		local field = fields[index]
		if type(field) == "string" and string.find(string.lower(field), text, 1, true) then
			return true
		end
	end
	return false
end

function Logic.NextInst(places, itemID)
	local highest = 0
	for index = 1, #places do
		local place = places[index]
		if place.itemID == itemID and place.inst > highest then
			highest = place.inst
		end
	end
	return highest + 1
end

local function countDistance(place, item)
	local delta = place.count - item.count
	if delta < 0 then
		delta = -delta
	end
	return delta
end

local function copyMatchedPlace(place, item)
	return {
		itemID = place.itemID,
		inst = place.inst,
		x = place.x,
		y = place.y,
		bag = item.bag,
		slot = item.slot,
		count = item.count,
		held = place.held == true,
		classID = item.classID,
		quality = item.quality,
		isQuest = item.isQuest == true,
	}
end

local function matchUnmovedStacks(saved, live, usedSaved, usedLive, places)
	for liveIndex = 1, #live do
		local item = live[liveIndex]
		for savedIndex = 1, #saved do
			local place = saved[savedIndex]
			if not usedSaved[savedIndex]
				and place.bag == item.bag
				and place.slot == item.slot
				and place.itemID == item.itemID then
				usedSaved[savedIndex] = true
				usedLive[liveIndex] = true
				places[#places + 1] = copyMatchedPlace(place, item)
				break
			end
		end
	end
end

local function rematchMovedStacksByClosestCount(saved, live, usedSaved, usedLive, places)
	for savedIndex = 1, #saved do
		if not usedSaved[savedIndex] then
			local place = saved[savedIndex]
			local bestIndex = nil
			local bestDistance = nil
			local bestBag = nil
			local bestSlot = nil
			for liveIndex = 1, #live do
				local item = live[liveIndex]
				if not usedLive[liveIndex] and item.itemID == place.itemID then
					local distance = countDistance(place, item)
					local better = bestIndex == nil
						or distance < bestDistance
						or (distance == bestDistance and item.bag < bestBag)
						or (distance == bestDistance and item.bag == bestBag and item.slot < bestSlot)
					if better then
						bestIndex = liveIndex
						bestDistance = distance
						bestBag = item.bag
						bestSlot = item.slot
					end
				end
			end
			if bestIndex then
				local item = live[bestIndex]
				usedSaved[savedIndex] = true
				usedLive[bestIndex] = true
				places[#places + 1] = copyMatchedPlace(place, item)
			end
		end
	end
end

local function unmatchedLiveItems(live, usedLive)
	local unmatched = {}
	for liveIndex = 1, #live do
		if not usedLive[liveIndex] then
			unmatched[#unmatched + 1] = live[liveIndex]
		end
	end
	return unmatched
end

local function vacatedPlaces(saved, usedSaved)
	local vacated = {}
	for savedIndex = 1, #saved do
		if not usedSaved[savedIndex] and not saved[savedIndex].held then
			vacated[#vacated + 1] = saved[savedIndex]
		end
	end
	return vacated
end

function Logic.Match(saved, live)
	local places = {}
	local usedSaved = {}
	local usedLive = {}
	saved = saved or {}
	live = live or {}
	matchUnmovedStacks(saved, live, usedSaved, usedLive, places)
	rematchMovedStacksByClosestCount(saved, live, usedSaved, usedLive, places)
	return { places = places, unmatched = unmatchedLiveItems(live, usedLive), vacated = vacatedPlaces(saved, usedSaved) }
end

local function overlaps(x, y, others, icon)
	local limit = icon - 1
	for index = 1, #others do
		local other = others[index]
		local dx = x - other.x
		local dy = y - other.y
		if dx < 0 then
			dx = -dx
		end
		if dy < 0 then
			dy = -dy
		end
		if dx < limit and dy < limit then
			return true
		end
	end
	return false
end

Logic.MEMORY_MAX = 64

function Logic.Vacate(memory, vacated, places)
	local body = {}
	for index = 1, #(places or {}) do
		if not places[index].held then
			body[#body + 1] = places[index]
		end
	end
	local nextMemory = {}
	local function keep(entry)
		if not overlaps(entry.x, entry.y, body, Logic.ICON) then
			nextMemory[#nextMemory + 1] = entry
		end
	end
	for index = 1, #(memory or {}) do
		keep(memory[index])
	end
	for index = 1, #(vacated or {}) do
		local place = vacated[index]
		keep({ itemID = place.itemID, category = Logic.Category(place.classID, place.quality, place.isQuest), x = place.x, y = place.y })
	end
	while #nextMemory > Logic.MEMORY_MAX do
		table.remove(nextMemory, 1)
	end
	return nextMemory
end

local function roundGrid(value, grid)
	return math.floor(value / grid + 0.5) * grid
end

function Logic.Snap(x, y, others, icon, category, contentW, contentH)
	icon = icon or Logic.ICON
	others = others or {}
	local reach = Logic.REACH * icon
	local bestX, bestY, bestDist
	local function take(px, py, dist)
		local better = bestDist == nil
			or dist < bestDist
			or (dist == bestDist and px < bestX)
			or (dist == bestDist and px == bestX and py < bestY)
		if better then
			bestDist = dist
			bestX = px
			bestY = py
		end
	end
	local sides = {
		{ 1, 0 },
		{ -1, 0 },
		{ 0, 1 },
		{ 0, -1 },
	}
	for index = 1, #others do
		local other = others[index]
		if not category or other.category == category then
			for side = 1, #sides do
				local step = sides[side]
				local flushX = other.x + step[1] * icon
				local flushY = other.y + step[2] * icon
				local dx = flushX - x
				local dy = flushY - y
				local dist = math.sqrt(dx * dx + dy * dy)
				if dist <= reach and not overlaps(flushX, flushY, others, icon) then
					take(flushX, flushY, dist)
				end
			end
		end
	end
	if type(contentW) == "number" and type(contentH) == "number" and contentW >= icon and contentH >= icon then
		local half = icon / 2
		local minX = half
		local maxX = contentW - half
		local minY = half
		local maxY = contentH - half
		local function clamp(value, low, high)
			if value < low then
				return low
			end
			if value > high then
				return high
			end
			return value
		end
		local function offer(px, py)
			if overlaps(px, py, others, icon) then
				return
			end
			if not Logic.IconInside(px, py, contentW, contentH, icon) then
				return
			end
			local dx = px - x
			local dy = py - y
			local dist = math.sqrt(dx * dx + dy * dy)
			if dist <= reach then
				take(px, py, dist)
			end
		end
		local alongY = clamp(y, minY, maxY)
		local alongX = clamp(x, minX, maxX)
		offer(minX, alongY)
		offer(maxX, alongY)
		offer(alongX, minY)
		offer(alongX, maxY)
	end
	if bestX then
		return bestX, bestY
	end

	local bounded = type(contentW) == "number" and type(contentH) == "number" and contentW >= icon and contentH >= icon
	local function fits(px, py)
		return not bounded or Logic.IconInside(px, py, contentW, contentH, icon)
	end
	local gridX = roundGrid(x, Logic.GRID)
	local gridY = roundGrid(y, Logic.GRID)
	if not overlaps(gridX, gridY, others, icon) then
		return gridX, gridY
	end
	if category then
		for index = 1, #others do
			local other = others[index]
			if other.category ~= category and overlaps(gridX, gridY, { other }, icon) then
				return gridX, gridY
			end
		end
	end
	for radius = 1, 10 do
		local foundX, foundY, foundDist
		for ox = -radius, radius do
			for oy = -radius, radius do
				if math.abs(ox) == radius or math.abs(oy) == radius then
					local tryX = gridX + ox * Logic.GRID
					local tryY = gridY + oy * Logic.GRID
					if fits(tryX, tryY) and not overlaps(tryX, tryY, others, icon) then
						local dx = tryX - x
						local dy = tryY - y
						local dist = dx * dx + dy * dy
						if foundDist == nil or dist < foundDist then
							foundDist = dist
							foundX = tryX
							foundY = tryY
						end
					end
				end
			end
		end
		if foundX then
			return foundX, foundY
		end
	end
	return gridX, gridY
end

function Logic.SnapGroup(starts, grab, proposedX, proposedY, others, icon, category, contentW, contentH)
	icon = icon or Logic.ICON
	others = others or {}
	local origin = starts[grab]
	local wantDx = proposedX - origin.x
	local wantDy = proposedY - origin.y
	local reach = Logic.REACH * icon
	local half = icon / 2
	local minX, minY, maxX, maxY = origin.x, origin.y, origin.x, origin.y
	for index = 1, #starts do
		local start = starts[index]
		minX = math.min(minX, start.x)
		minY = math.min(minY, start.y)
		maxX = math.max(maxX, start.x)
		maxY = math.max(maxY, start.y)
	end
	local bounded = type(contentW) == "number" and type(contentH) == "number"
		and contentW >= maxX - minX + icon and contentH >= maxY - minY + icon
	local function fits(dx, dy)
		for index = 1, #starts do
			local x = starts[index].x + dx
			local y = starts[index].y + dy
			if bounded and not Logic.IconInside(x, y, contentW, contentH, icon) then
				return false
			end
			if overlaps(x, y, others, icon) then
				return false
			end
		end
		return true
	end
	local function moved(dx, dy)
		local points = {}
		for index = 1, #starts do
			points[index] = { x = starts[index].x + dx, y = starts[index].y + dy }
		end
		return points
	end
	local function distance(dx, dy)
		return math.sqrt((dx - wantDx) * (dx - wantDx) + (dy - wantDy) * (dy - wantDy))
	end
	local bestDx, bestDy, bestDist
	local sides = { { 1, 0 }, { -1, 0 }, { 0, 1 }, { 0, -1 } }
	for index = 1, #starts do
		local start = starts[index]
		for otherIndex = 1, #others do
			local other = others[otherIndex]
			if not category or other.category == category then
				for side = 1, #sides do
					local dx = other.x + sides[side][1] * icon - start.x
					local dy = other.y + sides[side][2] * icon - start.y
					local dist = distance(dx, dy)
					if dist <= reach and (bestDist == nil or dist < bestDist) and fits(dx, dy) then
						bestDx, bestDy, bestDist = dx, dy, dist
					end
				end
			end
		end
	end
	if bounded then
		local function axis(want, low, high)
			if want <= low or math.abs(want - low) <= reach then
				return low, true
			end
			if want >= high or math.abs(want - high) <= reach then
				return high, true
			end
			return want, false
		end
		local edgeDx, snapX = axis(wantDx, half - minX, contentW - half - maxX)
		local edgeDy, snapY = axis(wantDy, half - minY, contentH - half - maxY)
		if snapX or snapY then
			local dist = distance(edgeDx, edgeDy)
			if (bestDist == nil or dist < bestDist) and fits(edgeDx, edgeDy) then
				bestDx, bestDy, bestDist = edgeDx, edgeDy, dist
			end
		end
	end
	if bestDx then
		return moved(bestDx, bestDy)
	end
	local gridDx = roundGrid(proposedX, Logic.GRID) - origin.x
	local gridDy = roundGrid(proposedY, Logic.GRID) - origin.y
	if fits(gridDx, gridDy) then
		return moved(gridDx, gridDy)
	end
	if category then
		for index = 1, #starts do
			for otherIndex = 1, #others do
				local other = others[otherIndex]
				if other.category ~= category and overlaps(starts[index].x + gridDx, starts[index].y + gridDy, { other }, icon) then
					return moved(gridDx, gridDy)
				end
			end
		end
	end
	for radius = 1, 10 do
		local foundDx, foundDy, foundDist
		for ox = -radius, radius do
			for oy = -radius, radius do
				if math.abs(ox) == radius or math.abs(oy) == radius then
					local dx = gridDx + ox * Logic.GRID
					local dy = gridDy + oy * Logic.GRID
					local dist = distance(dx, dy)
					if (foundDist == nil or dist < foundDist) and fits(dx, dy) then
						foundDx, foundDy, foundDist = dx, dy, dist
					end
				end
			end
		end
		if foundDx then
			return moved(foundDx, foundDy)
		end
	end
	return moved(gridDx, gridDy)
end

local function blockShape(count, maxWidth, icon)
	local cols = math.ceil(math.sqrt(count))
	if cols < 1 then
		cols = 1
	end
	local maxCols = math.floor(maxWidth / icon)
	if maxCols < 1 then
		maxCols = 1
	end
	if cols > maxCols then
		cols = maxCols
	end
	local rows = math.ceil(count / cols)
	return cols, rows
end

function Logic.Pack(entries, zones, maxWidth)
	maxWidth = maxWidth or Logic.MAX_WIDTH
	zones = copyZones(zones)
	local groups = {}
	local order = {}
	for index = 1, #entries do
		local entry = entries[index]
		local category = entry.category
		if not RANK[category] then
			category = "other"
		end
		if not groups[category] then
			groups[category] = {}
			order[#order + 1] = category
		end
		groups[category][#groups[category] + 1] = entry
	end
	table.sort(order, function(left, right)
		if zones[left] ~= zones[right] then
			return zones[left] < zones[right]
		end
		return RANK[left] < RANK[right]
	end)

	local positions = {}
	local cursorX = 0
	local cursorY = 0
	local rowHeight = 0
	local width = 0
	local height = 0
	for orderIndex = 1, #order do
		local category = order[orderIndex]
		local group = groups[category]
		local cols, rows = blockShape(#group, maxWidth, Logic.ICON)
		local blockWidth = cols * Logic.ICON
		local blockHeight = rows * Logic.ICON
		if cursorX > 0 and cursorX + blockWidth > maxWidth then
			cursorY = cursorY + rowHeight + Logic.GAP
			cursorX = 0
			rowHeight = 0
		end
		for itemIndex = 1, #group do
			local col = (itemIndex - 1) % cols
			local row = math.floor((itemIndex - 1) / cols)
			positions[group[itemIndex].id] = {
				x = cursorX + Logic.ICON / 2 + col * Logic.ICON,
				y = cursorY + Logic.ICON / 2 + row * Logic.ICON,
			}
		end
		if blockHeight > rowHeight then
			rowHeight = blockHeight
		end
		local right = cursorX + blockWidth
		local bottom = cursorY + blockHeight
		if right > width then
			width = right
		end
		if bottom > height then
			height = bottom
		end
		cursorX = cursorX + blockWidth + Logic.GAP
	end
	return { positions = positions, width = width, height = height }
end

-- Leyline: Clean Up's tidy grid. Categories in the original's order, each filling whole cells left to right from
-- the top left, so every icon sits on one column grid. A category starts a new row band, GAP below the last; a small
-- category that fits on the current band's single row joins it after one empty cell. No two categories touch, so a
-- Shift-drag still picks up one category. Same return as Pack.
function Logic.Bands(entries, zones, maxWidth)
	maxWidth = maxWidth or Logic.MAX_WIDTH
	zones = copyZones(zones)
	local icon = Logic.ICON
	local cols = math.floor(maxWidth / icon)
	if cols < 1 then
		cols = 1
	end
	local groups = {}
	local order = {}
	for index = 1, #entries do
		local entry = entries[index]
		local category = entry.category
		if not RANK[category] then
			category = "other"
		end
		if not groups[category] then
			groups[category] = {}
			order[#order + 1] = category
		end
		groups[category][#groups[category] + 1] = entry
	end
	table.sort(order, function(left, right)
		if zones[left] ~= zones[right] then
			return zones[left] < zones[right]
		end
		return RANK[left] < RANK[right]
	end)
	local positions = {}
	local bandTop = 0
	local bandRows = 0
	local used = 0
	local width = 0
	local height = 0
	for orderIndex = 1, #order do
		local group = groups[order[orderIndex]]
		local count = #group
		local startCol = 0
		local top
		if bandRows == 1 and used > 0 and used + 1 + count <= cols then
			startCol = used + 1
			top = bandTop
			used = startCol + count
		else
			if bandRows > 0 then
				bandTop = bandTop + bandRows * icon + Logic.GAP
			end
			top = bandTop
			bandRows = math.ceil(count / cols)
			used = count
			if bandRows > 1 then
				used = cols
			end
		end
		for itemIndex = 1, count do
			local cell = startCol + itemIndex - 1
			local col = cell % cols
			local row = math.floor(cell / cols)
			if startCol > 0 then
				col = cell
				row = 0
			end
			local x = icon / 2 + col * icon
			local y = top + icon / 2 + row * icon
			positions[group[itemIndex].id] = { x = x, y = y }
			if x + icon / 2 > width then
				width = x + icon / 2
			end
			if y + icon / 2 > height then
				height = y + icon / 2
			end
		end
	end
	return { positions = positions, width = width, height = height }
end

function Logic.HoldingPositions(count, icon, maxWidth)
	icon = icon or Logic.ICON
	maxWidth = maxWidth or Logic.BAG_WIDTH
	local cols = math.floor(maxWidth / icon)
	if cols < 1 then
		cols = 1
	end
	local spots = {}
	local rows = 0
	for index = 1, count do
		local col = (index - 1) % cols
		local row = math.floor((index - 1) / cols)
		spots[index] = {
			x = icon / 2 + col * icon,
			y = icon / 2 + row * icon,
		}
		if row + 1 > rows then
			rows = row + 1
		end
	end
	local width = 0
	if count > 0 then
		local used = count
		if used > cols then
			used = cols
		end
		width = used * icon
	end
	return { spots = spots, width = width, height = rows * icon }
end

function Logic.FrameContent(packWidth, packHeight, holdingCount, barHeight)
	local holding = Logic.HoldingPositions(holdingCount)
	local width = packWidth
	if holding.width > width then
		width = holding.width
	end
	local height = packHeight + barHeight
	if holdingCount > 0 then
		height = height + Logic.STRIP_GAP + holding.height
	end
	return width, height
end

local function appendPlace(places, item, x, y, held)
	places[#places + 1] = {
		itemID = item.itemID,
		inst = Logic.NextInst(places, item.itemID),
		x = x,
		y = y,
		bag = item.bag,
		slot = item.slot,
		count = item.count,
		held = held,
		classID = item.classID,
		quality = item.quality,
		isQuest = item.isQuest == true,
	}
end

local function groupEntries(entries)
	local groups = {}
	for index = 1, #entries do
		local entry = entries[index]
		local category = entry.category
		if not RANK[category] then
			category = "other"
		end
		if not groups[category] then
			groups[category] = {}
		end
		local group = groups[category]
		group[#group + 1] = entry
	end
	return groups
end

local function gridCount(limit)
	local count = math.floor(limit / Logic.ICON + 1e-9) - 1
	if count < 0 then
		count = 0
	end
	return count
end

local function centerOf(index)
	return Logic.ICON / 2 + index * Logic.ICON
end

local function regionSign(value, limit)
	if value < limit / 3 then
		return -1
	end
	if value < limit * 2 / 3 then
		return 0
	end
	return 1
end

local function farOffset(limit)
	return limit - (gridCount(limit) + 1) * Logic.ICON
end

local function zoneAxis(tag, cells, limit, tagHalf)
	local maxIndex = gridCount(limit)
	local room = maxIndex - (cells - 1)
	if room < 0 then
		room = 0
	end
	local sign = regionSign(tag, limit)
	local offset = 0
	local origin
	if sign < 0 then
		origin = math.floor((tag - tagHalf) / Logic.ICON + 0.5)
	elseif sign > 0 then
		offset = farOffset(limit)
		local fromFar = math.floor((limit - (tag + tagHalf)) / Logic.ICON + 0.5)
		origin = maxIndex - fromFar - (cells - 1)
	else
		origin = math.floor((tag - cells * Logic.ICON / 2) / Logic.ICON + 0.5)
	end
	if origin < 0 then
		origin = 0
	end
	if origin > room then
		origin = room
	end
	return origin, offset, sign
end

local function placeGroup(group, tagX, tagY, maxW, maxH)
	local maxN = gridCount(maxW)
	local maxM = gridCount(maxH)
	local count = #group
	local cols, rows = blockShape(count, maxW, Logic.ICON)
	local maxRows = maxM + 1
	if rows > maxRows then
		local maxCols = maxN + 1
		if maxCols < 1 then
			maxCols = 1
		end
		cols = math.ceil(count / maxRows)
		if cols > maxCols then
			cols = maxCols
		end
		if cols < 1 then
			cols = 1
		end
		rows = math.ceil(count / cols)
	end
	local originN, offX, hx = zoneAxis(tagX, cols, maxW, Logic.TAG_W / 2)
	local originM, offY, hy = zoneAxis(tagY, rows, maxH, Logic.TAG_H / 2)
	local points = {}
	for itemIndex = 1, count do
		local col = (itemIndex - 1) % cols
		local row = math.floor((itemIndex - 1) / cols)
		local xIndex = col
		local yIndex = row
		if hx > 0 then
			xIndex = (cols - 1) - col
		end
		if hy > 0 then
			yIndex = (rows - 1) - row
		end
		points[itemIndex] = {
			id = group[itemIndex].id,
			x = centerOf(originN + xIndex) + offX,
			y = centerOf(originM + yIndex) + offY,
		}
	end
	return points, offX, offY
end

local function pointIndex(value)
	return math.floor((value - Logic.ICON / 2) / Logic.ICON + 0.5)
end

local function shiftGroup(points, ox, oy, maxN, maxM, occupied, offX, offY)
	local moved = {}
	for index = 1, #points do
		local n = pointIndex(points[index].x - offX) + ox
		local m = pointIndex(points[index].y - offY) + oy
		if n < 0 or m < 0 or n > maxN or m > maxM then
			return nil
		end
		local x = centerOf(n) + offX
		local y = centerOf(m) + offY
		if iconHits(x, y, occupied, Logic.ICON) then
			return nil
		end
		moved[index] = { id = points[index].id, x = x, y = y }
	end
	return moved
end

local function settleGroup(points, maxN, maxM, occupied, offX, offY)
	local stay = shiftGroup(points, 0, 0, maxN, maxM, occupied, offX, offY)
	if stay then
		return stay
	end
	local limit = maxN + maxM
	if limit < 1 then
		limit = 1
	end
	local best, bestDist, bestOy, bestOx
	for oy = -limit, limit do
		for ox = -limit, limit do
			local dist = ox * ox + oy * oy
			local better = bestDist == nil or dist < bestDist
			if not better and dist == bestDist then
				better = oy < bestOy or (oy == bestOy and ox < bestOx)
			end
			if better then
				local moved = shiftGroup(points, ox, oy, maxN, maxM, occupied, offX, offY)
				if moved then
					best = moved
					bestDist = dist
					bestOy = oy
					bestOx = ox
				end
			end
		end
	end
	if best then
		return best
	end
	return points
end

function Logic.HasHomes(homes)
	if type(homes) ~= "table" then
		return false
	end
	for index = 1, #Logic.CATEGORIES do
		local home = homes[Logic.CATEGORIES[index]]
		if type(home) == "table" and type(home.x) == "number" and type(home.y) == "number" then
			return true
		end
	end
	return false
end

local function categoryTagged(groups, homes)
	for index = 1, #Logic.CATEGORIES do
		local category = Logic.CATEGORIES[index]
		if groups[category] then
			local home = homes[category]
			if type(home) == "table" and type(home.x) == "number" and type(home.y) == "number" then
				return true
			end
		end
	end
	return false
end

function Logic.Arrange(entries, homes, maxWidth, maxHeight, taken)
	maxWidth = maxWidth or Logic.BAG_WIDTH
	if type(maxHeight) ~= "number" or maxHeight < Logic.ICON then
		maxHeight = maxWidth
	end
	entries = entries or {}
	if not Logic.HasHomes(homes) then
		return Logic.Bands(entries, nil, maxWidth)
	end
	local groups = groupEntries(entries)
	if not categoryTagged(groups, homes) then
		return Logic.Bands(entries, nil, maxWidth)
	end
	local positions = {}
	local occupied = {}
	for index = 1, #(taken or {}) do
		occupied[index] = taken[index]
	end
	local width = 0
	local height = 0
	local function note(x, y)
		local right = x + Logic.ICON / 2
		local bottom = y + Logic.ICON / 2
		if right > width then
			width = right
		end
		if bottom > height then
			height = bottom
		end
	end
	local maxN = gridCount(maxWidth)
	local maxM = gridCount(maxHeight)
	for index = 1, #Logic.CATEGORIES do
		local category = Logic.CATEGORIES[index]
		local group = groups[category]
		if group then
			local home = homes[category]
			local tagX = Logic.ICON / 2
			local tagY = Logic.ICON / 2
			if type(home) == "table" and type(home.x) == "number" and type(home.y) == "number" then
				tagX = home.x
				tagY = home.y
			end
			local points, offX, offY = placeGroup(group, tagX, tagY, maxWidth, maxHeight)
			local placed = settleGroup(points, maxN, maxM, occupied, offX, offY)
			for pointIndex = 1, #placed do
				local point = placed[pointIndex]
				positions[point.id] = { x = point.x, y = point.y }
				occupied[#occupied + 1] = { x = point.x, y = point.y }
				note(point.x, point.y)
			end
		end
	end
	return { positions = positions, width = width, height = height }
end

function Logic.ZoneFootprint(count, tagX, tagY, contentW, contentH)
	if type(count) ~= "number" or count < 1 then
		count = 1
	end
	local entries = {}
	for index = 1, count do
		entries[index] = { id = index, category = "other" }
	end
	local laid = Logic.Arrange(entries, { other = { x = tagX, y = tagY } }, contentW, contentH)
	local minX, minY, maxX, maxY
	for index = 1, count do
		local pos = laid.positions[index]
		if pos then
			if minX == nil or pos.x < minX then
				minX = pos.x
			end
			if minY == nil or pos.y < minY then
				minY = pos.y
			end
			if maxX == nil or pos.x > maxX then
				maxX = pos.x
			end
			if maxY == nil or pos.y > maxY then
				maxY = pos.y
			end
		end
	end
	if minX == nil then
		return 0, 0, Logic.ICON, Logic.ICON
	end
	local half = Logic.ICON / 2
	return minX - half, minY - half, (maxX - minX) + Logic.ICON, (maxY - minY) + Logic.ICON
end

function Logic.ZonePreviewAnchor(count, tagX, tagY, contentW, contentH)
	local left, top, width, height = Logic.ZoneFootprint(count, tagX, tagY, contentW, contentH)
	return left - tagX, top - tagY, width, height
end

function Logic.CursorLocal(cursorX, cursorY, scale, left, top)
	if type(cursorX) ~= "number" or type(cursorY) ~= "number" then
		return nil
	end
	if type(left) ~= "number" or type(top) ~= "number" then
		return nil
	end
	if type(scale) ~= "number" or scale == 0 then
		scale = 1
	end
	return (cursorX / scale) - left, top - (cursorY / scale)
end

function Logic.GrabbedPoint(localX, localY, grabX, grabY)
	if type(localX) ~= "number" or type(localY) ~= "number" then
		return nil
	end
	if type(grabX) ~= "number" then
		grabX = 0
	end
	if type(grabY) ~= "number" then
		grabY = 0
	end
	return localX - grabX, localY - grabY
end

function Logic.ItemDraggable(item)
	if type(item) ~= "table" then
		return false
	end
	if type(item.itemID) ~= "number" then
		return false
	end
	return true
end

function Logic.DragRelease(buttonStillDown, mode)
	if buttonStillDown and (mode == "pressed" or mode == "dragging") then
		return "ignore"
	end
	return "finish"
end

function Logic.ItemLevel(hostLevel, index)
	return hostLevel + index
end

local function clonePlace(place, x, y, held)
	return {
		itemID = place.itemID,
		inst = place.inst,
		x = x,
		y = y,
		bag = place.bag,
		slot = place.slot,
		count = place.count,
		held = held,
		classID = place.classID,
		quality = place.quality,
		isQuest = place.isQuest == true,
	}
end

local function rigidAxis(tag, size, limit, tagHalf)
	local sign = regionSign(tag, limit)
	local low
	if sign < 0 then
		low = math.floor((tag - tagHalf) / Logic.ICON + 0.5) * Logic.ICON
	elseif sign > 0 then
		local far = limit - math.floor((limit - (tag + tagHalf)) / Logic.ICON + 0.5) * Logic.ICON
		low = far - size
	else
		low = math.floor((tag - size / 2) / Logic.ICON + 0.5) * Logic.ICON
	end
	return math.max(0, math.min(low, limit - size))
end

local function rectsHit(left, top, width, height, rects)
	for index = 1, #rects do
		local rect = rects[index]
		if left < rect.left + rect.width - 0.001 and rect.left < left + width - 0.001
			and top < rect.top + rect.height - 0.001 and rect.top < top + height - 0.001 then
			return true
		end
	end
	return false
end

local function settleRect(left, top, width, height, rects, limitW, limitH)
	local function fits(l, t)
		return l >= -0.001 and t >= -0.001 and l + width <= limitW + 0.001 and t + height <= limitH + 0.001
			and not rectsHit(l, t, width, height, rects)
	end
	if fits(left, top) then
		return left, top
	end
	local rings = math.ceil(math.max(limitW, math.min(limitH, limitW * 4)) / Logic.ICON)
	for radius = 1, rings do
		local bestL, bestT, bestDist
		for ox = -radius, radius do
			for oy = -radius, radius do
				if math.abs(ox) == radius or math.abs(oy) == radius then
					local l = math.max(0, math.min(left + ox * Logic.ICON, limitW - width))
					local t = math.max(0, math.min(top + oy * Logic.ICON, limitH - height))
					local dist = (l - left) * (l - left) + (t - top) * (t - top)
					if (bestDist == nil or dist < bestDist) and fits(l, t) then
						bestL, bestT, bestDist = l, t, dist
					end
				end
			end
		end
		if bestL then
			return bestL, bestT
		end
	end
	return nil
end

local function keptTidy(places, homes, maxWidth, maxHeight)
	maxWidth = maxWidth or Logic.BAG_WIDTH
	local half = Logic.ICON / 2
	local byCategory = {}
	local held = {}
	for index = 1, #places do
		local place = places[index]
		if place.held then
			held[#held + 1] = index
		else
			local category = Logic.Category(place.classID, place.quality, place.isQuest)
			if not RANK[category] then
				category = "other"
			end
			byCategory[category] = byCategory[category] or {}
			local members = byCategory[category]
			members[#members + 1] = index
		end
	end
	local groups = {}
	for category in pairs(byCategory) do
		groups[category] = true
	end
	local zoned = Logic.HasHomes(homes) and categoryTagged(groups, homes)
	if type(maxHeight) ~= "number" or maxHeight < Logic.ICON then
		maxHeight = zoned and maxWidth or math.huge
	end
	local nextPlaces = {}
	local rects = {}
	local cursorX, cursorY, rowHeight = 0, 0, 0
	for orderIndex = 1, #Logic.CATEGORIES do
		local category = Logic.CATEGORIES[orderIndex]
		local members = byCategory[category]
		if members then
			local minX, minY, maxX, maxY = math.huge, math.huge, -math.huge, -math.huge
			for index = 1, #members do
				local place = places[members[index]]
				minX = math.min(minX, place.x)
				minY = math.min(minY, place.y)
				maxX = math.max(maxX, place.x)
				maxY = math.max(maxY, place.y)
			end
			local width = maxX - minX + Logic.ICON
			local height = maxY - minY + Logic.ICON
			local left, top
			if zoned then
				local home = homes[category]
				local tagX, tagY = half, half
				if type(home) == "table" and type(home.x) == "number" and type(home.y) == "number" then
					tagX, tagY = home.x, home.y
				end
				left = rigidAxis(tagX, width, maxWidth, Logic.TAG_W / 2)
				top = rigidAxis(tagY, height, maxHeight, Logic.TAG_H / 2)
			else
				if cursorX > 0 and cursorX + width > maxWidth then
					cursorY = cursorY + rowHeight + Logic.GAP
					cursorX = 0
					rowHeight = 0
				end
				left, top = cursorX, cursorY
				cursorX = cursorX + width + Logic.GAP
				rowHeight = math.max(rowHeight, height)
			end
			left, top = settleRect(left, top, width, height, rects, maxWidth, maxHeight)
			if not left then
				return nil
			end
			rects[#rects + 1] = { left = left, top = top, width = width, height = height }
			for index = 1, #members do
				local place = places[members[index]]
				nextPlaces[members[index]] = clonePlace(place, left + half + (place.x - minX), top + half + (place.y - minY), false)
			end
		end
	end
	for index = 1, #held do
		local place = places[held[index]]
		nextPlaces[held[index]] = clonePlace(place, place.x, place.y, true)
	end
	return nextPlaces
end

function Logic.Tidy(places, homes, maxWidth, maxHeight, keepOrder)
	places = places or {}
	if keepOrder then
		local kept = keptTidy(places, homes, maxWidth, maxHeight)
		if kept then
			return kept
		end
		local unchanged = {}
		for index = 1, #places do
			local place = places[index]
			unchanged[index] = clonePlace(place, place.x, place.y, place.held == true)
		end
		return unchanged
	end
	local entries = {}
	for index = 1, #places do
		local place = places[index]
		entries[index] = {
			id = place.itemID .. ":" .. place.inst,
			category = Logic.Category(place.classID, place.quality, place.isQuest),
		}
	end
	local packed = Logic.Arrange(entries, homes, maxWidth, maxHeight)
	local nextPlaces = {}
	for index = 1, #places do
		local place = places[index]
		local pos = packed.positions[place.itemID .. ":" .. place.inst]
		if pos then
			nextPlaces[index] = clonePlace(place, pos.x, pos.y, false)
		else
			nextPlaces[index] = clonePlace(place, place.x, place.y, place.held == true)
		end
	end
	local groups = groupEntries(entries)
	if Logic.HasHomes(homes) and categoryTagged(groups, homes) then
		return nextPlaces
	end
	if keepOrder then
		return nextPlaces
	end
	return Logic.SeparateCategories(nextPlaces)
end

function Logic.FirstRunPending(row)
	if type(row) ~= "table" then
		return true
	end
	if row.firstRun == true then
		return false
	end
	if type(row.places) == "table" and #row.places > 0 then
		return false
	end
	if type(row.width) == "number" and row.width > 0 and type(row.height) == "number" and row.height > 0 then
		return false
	end
	if Logic.HasHomes(row.homes) then
		return false
	end
	return true
end

function Logic.FirstRunReady(reason, itemCount, slotCount)
	itemCount = itemCount or 0
	slotCount = slotCount or 0
	if reason == "delayed" then
		return itemCount > 0 or slotCount > 0
	end
	if reason == "open" then
		return itemCount > 0
	end
	return false
end

function Logic.ReadingUsable(backpackSlots)
	return type(backpackSlots) == "number" and backpackSlots > 0
end

function Logic.CommitFirstRun(row, plan)
	if not Logic.FirstRunPending(row) or type(plan) ~= "table" then
		return false
	end
	if type(plan.width) ~= "number" or type(plan.height) ~= "number" or type(plan.places) ~= "table" then
		return false
	end
	row.places = plan.places
	row.width = plan.width
	row.height = plan.height
	row.firstRun = true
	return true
end

-- Leyline: the first open with the bag fitted to its items: Clean Up's tidy grid from the top left, no size chosen.
-- Clean Up for a bag that fits its items: the default width first; a bag too tall for the screen at that width gets
-- two more columns at a time. Returns the places and the width used.
function Logic.AutoTidy(places, homes, keepOrder, screenH, bar)
	local limit = plainSize(screenH)
	if limit then
		limit = limit - Logic.HOME_BOTTOM - 10
	end
	local cols = Logic.AUTO_COLS
	local best, bestW
	while true do
		local width = cols * Logic.ICON + Logic.GAP
		local tidy = Logic.Tidy(places, homes, width, width, keepOrder)
		local _, height = Logic.AutoSize(tidy, bar or 0, nil, nil)
		best, bestW = tidy, width
		if not limit or height <= limit or cols >= Logic.AUTO_COLS + 8 then
			break
		end
		cols = cols + 2
	end
	return best, bestW
end

function Logic.PlanAutoFirstRun(items, options)
	options = options or {}
	local blank = {}
	items = items or {}
	for index = 1, #items do
		local item = items[index]
		if type(item) == "table" and type(item.itemID) == "number" then
			appendPlace(blank, item, 0, 0, false)
		end
	end
	if #blank == 0 then
		return { places = blank, auto = true }
	end
	local places = Logic.AutoTidy(blank, options.homes, options.keepOrder == true, options.screenH)
	return { places = places, auto = true }
end

function Logic.CommitAutoFirstRun(row, plan)
	if not Logic.FirstRunPending(row) or type(plan) ~= "table" or type(plan.places) ~= "table" then
		return false
	end
	row.places = plan.places
	row.width = nil
	row.height = nil
	row.firstRun = true
	return true
end

function Logic.PlanFirstRun(items, options)
	options = options or {}
	local bar = options.bar or 0
	local homes = options.homes
	local keepOrder = options.keepOrder == true
	local screenW = options.screenW
	local screenH = options.screenH
	local blank = {}
	items = items or {}
	for index = 1, #items do
		local item = items[index]
		if type(item) == "table" and type(item.itemID) == "number" then
			appendPlace(blank, item, 0, 0, false)
		end
	end
	if #blank == 0 then
		local width, height = Logic.ClampSize(Logic.MinFrameWidth(bar), Logic.CHROME_MIN_H + bar, 0, 0, bar, screenW, screenH)
		return { places = blank, width = width, height = height }
	end
	local function heldCount(places)
		local count = 0
		for index = 1, #places do
			if places[index].held then
				count = count + 1
			end
		end
		return count
	end
	local function spillOf(places, contentW, contentH)
		local spill = 0
		for index = 1, #places do
			local place = places[index]
			if place.held or not Logic.IconInside(place.x, place.y, contentW, contentH, Logic.ICON) then
				spill = spill + 1
			end
		end
		if not Logic.PlacesOk(places, Logic.ICON) then
			spill = spill + #places
		end
		return spill
	end
	-- ponytail: step one icon at a time, 40 tries; binary search if FRAME_MAX grows
	local width = Logic.MinFrameWidth(bar)
	local tried = {}
	local best
	local guard = 0
	while guard < 40 do
		guard = guard + 1
		local height = Logic.MinHeightForWidth(width, #blank, 0, bar)
		local frameW, frameH = Logic.ClampSize(width, height, #blank, 0, bar, screenW, screenH)
		local mark = math.floor(frameW * 10 + 0.5)
		if tried[mark] then
			break
		end
		tried[mark] = true
		local contentW, contentH = Logic.ContentBox(frameW, frameH, bar, 0)
		local tidy = Logic.Tidy(blank, homes, contentW, contentH, keepOrder)
		local grownW, grownH = Logic.GrownSize(frameW, frameH, tidy, bar, screenW, screenH)
		if grownW ~= frameW then
			local nextW, nextH = Logic.ContentBox(grownW, grownH, bar, heldCount(tidy))
			tidy = Logic.Tidy(blank, homes, nextW, nextH, keepOrder)
			grownW, grownH = Logic.GrownSize(grownW, grownH, tidy, bar, screenW, screenH)
		end
		local fitW, fitH = Logic.ContentBox(grownW, grownH, bar, heldCount(tidy))
		local spill = spillOf(tidy, fitW, fitH)
		local aspect = grownW / grownH
		local score = spill * 1000 + math.abs(aspect - 1.15)
		local area = grownW * grownH
		if not best or score < best.score - 0.001 or (math.abs(score - best.score) <= 0.001 and area < best.area) then
			best = { places = tidy, width = grownW, height = grownH, score = score, area = area }
		end
		width = frameW + Logic.ICON
	end
	return { places = best.places, width = best.width, height = best.height }
end

function Logic.HeldOverflow(places, maxWidth)
	maxWidth = maxWidth or Logic.BAG_WIDTH
	places = places or {}
	if #places == 0 then
		return false
	end
	local right = 0
	for index = 1, #places do
		local place = places[index]
		if not place.held then
			return false
		end
		local edge = place.x + Logic.ICON / 2
		if edge > right then
			right = edge
		end
	end
	return right > maxWidth
end

function Logic.ActiveHomes(useZones, homes)
	if useZones then
		return homes
	end
	return nil
end

function Logic.SetHome(homes, category, x, y)
	local nextHomes = copyHomes(homes)
	if type(x) ~= "number" or type(y) ~= "number" then
		return nextHomes
	end
	nextHomes[category] = { x = x, y = y }
	return nextHomes
end

function Logic.ZoneTagPoint(homes, category, index)
	local home = homes and homes[category]
	if type(home) == "table" and type(home.x) == "number" and type(home.y) == "number" then
		return home.x, home.y
	end
	index = index or 1
	local col = (index - 1) % 3
	local row = math.floor((index - 1) / 3)
	local x = Logic.TAG_W / 2 + col * (Logic.TAG_W + Logic.GRID)
	local y = Logic.TAG_H / 2 + row * (Logic.TAG_H + Logic.GRID)
	return x, y
end

local function tagHits(x, y, others)
	local limitW = Logic.TAG_W - 1
	local limitH = Logic.TAG_H - 1
	for index = 1, #others do
		local other = others[index]
		local dx = x - other.x
		local dy = y - other.y
		if dx < 0 then
			dx = -dx
		end
		if dy < 0 then
			dy = -dy
		end
		if dx < limitW and dy < limitH then
			return true
		end
	end
	return false
end

local function tagInside(x, y, contentW, contentH)
	if type(contentW) ~= "number" or type(contentH) ~= "number" then
		return false
	end
	local halfW = Logic.TAG_W / 2
	local halfH = Logic.TAG_H / 2
	if x < halfW or y < halfH then
		return false
	end
	if x + halfW > contentW + 0.001 or y + halfH > contentH + 0.001 then
		return false
	end
	return true
end

local function footprintAxis(size, limit, tagHalf)
	local spots = {}
	local function add(low, tag)
		if tag - tagHalf >= -0.001 and tag + tagHalf <= limit + 0.001 then
			spots[#spots + 1] = { low = low, tag = tag }
		end
	end
	local nearEnd = (gridCount(limit) + 1) * Logic.ICON
	local offset = farOffset(limit)
	for n = 0, gridCount(limit) do
		local low = n * Logic.ICON
		if low + size <= nearEnd + 0.001 then
			if regionSign(low + tagHalf, limit) < 0 then
				add(low, low + tagHalf)
			end
			if regionSign(low + size / 2, limit) == 0 then
				add(low, low + size / 2)
			end
		end
		local high = limit - n * Logic.ICON
		if high - size >= offset - 0.001 and regionSign(high - tagHalf, limit) > 0 then
			add(high - size, high - tagHalf)
		end
	end
	return spots
end

function Logic.SnapFootprint(count, left, top, others, contentW, contentH)
	if type(left) ~= "number" or type(top) ~= "number" or type(contentW) ~= "number" or type(contentH) ~= "number" then
		return nil
	end
	others = others or {}
	local _, _, width, height = Logic.ZoneFootprint(count, Logic.TAG_W / 2, Logic.TAG_H / 2, contentW, contentH)
	local columns = footprintAxis(width, contentW, Logic.TAG_W / 2)
	local rows = footprintAxis(height, contentH, Logic.TAG_H / 2)
	local best, bestRow, bestDist
	for a = 1, #columns do
		local column = columns[a]
		for b = 1, #rows do
			local row = rows[b]
			if not tagHits(column.tag, row.tag, others) then
				local dx = column.low - left
				local dy = row.low - top
				local dist = dx * dx + dy * dy
				if bestDist == nil or dist < bestDist then
					best, bestRow, bestDist = column, row, dist
				end
			end
		end
	end
	if not best then
		return nil
	end
	return best.tag, bestRow.tag, best.low, bestRow.low
end

local function nearestTagSpot(x, y, kept, contentW, contentH)
	local halfW = Logic.TAG_W / 2
	local halfH = Logic.TAG_H / 2
	local limitX = math.max(halfW, contentW - halfW)
	local limitY = math.max(halfH, contentH - halfH)
	local clampedX = math.min(math.max(x, halfW), limitX)
	local clampedY = math.min(math.max(y, halfH), limitY)
	local bestX, bestY, bestDist
	local function offer(px, py)
		if px < halfW or py < halfH or px > limitX + 0.001 or py > limitY + 0.001 or tagHits(px, py, kept) then
			return
		end
		local dist = (px - x) * (px - x) + (py - y) * (py - y)
		if bestDist == nil or dist < bestDist then
			bestX, bestY, bestDist = px, py, dist
		end
	end
	offer(clampedX, clampedY)
	local step = Logic.ICON + Logic.GAP
	for n = 0, math.floor((limitX - halfW) / step) do
		for m = 0, math.floor((limitY - halfH) / step) do
			offer(halfW + n * step, halfH + m * step)
			offer(limitX, halfH + m * step)
			offer(halfW + n * step, limitY)
		end
	end
	offer(limitX, limitY)
	for index = 1, #kept do
		local other = kept[index]
		offer(other.x + Logic.TAG_W, other.y)
		offer(other.x - Logic.TAG_W, other.y)
		offer(other.x, other.y + Logic.TAG_H)
		offer(other.x, other.y - Logic.TAG_H)
	end
	if bestX then
		return bestX, bestY
	end
	return clampedX, clampedY
end

function Logic.ClampHomes(homes, contentW, contentH)
	local nextHomes = copyHomes(homes)
	local shown = {}
	if type(contentW) ~= "number" or type(contentH) ~= "number" then
		for index = 1, #Logic.CATEGORIES do
			local x, y = Logic.ZoneTagPoint(nextHomes, Logic.CATEGORIES[index], index)
			shown[index] = { x = x, y = y }
		end
		return nextHomes, shown
	end
	local kept = {}
	for index = 1, #Logic.CATEGORIES do
		local x, y = Logic.ZoneTagPoint(nextHomes, Logic.CATEGORIES[index], index)
		if tagInside(x, y, contentW, contentH) and not tagHits(x, y, kept) then
			shown[index] = { x = x, y = y }
			kept[#kept + 1] = shown[index]
		end
	end
	for index = 1, #Logic.CATEGORIES do
		if not shown[index] then
			local category = Logic.CATEGORIES[index]
			local x, y = Logic.ZoneTagPoint(nextHomes, category, index)
			x, y = nearestTagSpot(x, y, kept, contentW, contentH)
			shown[index] = { x = x, y = y }
			kept[#kept + 1] = shown[index]
			if nextHomes[category] then
				nextHomes[category] = { x = x, y = y }
			end
		end
	end
	return nextHomes, shown
end

local function cardinalNeighbor(a, b, icon, slack)
	if a.held ~= b.held then
		return false
	end
	local dx = a.x - b.x
	local dy = a.y - b.y
	if dx < 0 then
		dx = -dx
	end
	if dy < 0 then
		dy = -dy
	end
	local touch = icon + slack
	if dx <= touch and dy <= slack then
		return true
	end
	if dy <= touch and dx <= slack then
		return true
	end
	return false
end

local GROW_RANK = { inside = 0, flow = 1, outward = 2, anchor = 3, back = 4 }

local function largestCluster(points)
	local seen = {}
	local best
	for index = 1, #points do
		if not seen[index] then
			seen[index] = true
			local cluster = { points[index] }
			local head = 1
			while cluster[head] do
				local current = cluster[head]
				head = head + 1
				for other = 1, #points do
					if not seen[other] and cardinalNeighbor(current, points[other], Logic.ICON, Logic.GRID) then
						seen[other] = true
						cluster[#cluster + 1] = points[other]
					end
				end
			end
			if not best or #cluster > #best then
				best = cluster
			end
		end
	end
	return best
end

local function spanOf(row)
	return row.max - row.min + 1
end

local function groupShape(members, incoming, contentW, contentH)
	local origin = members[1]
	for index = 2, #members do
		local member = members[index]
		if member.y < origin.y - 1 or (math.abs(member.y - origin.y) <= 1 and member.x < origin.x) then
			origin = member
		end
	end
	local shape = { x = origin.x, y = origin.y, rows = {}, taken = {} }
	for index = 1, #members do
		local col = math.floor((members[index].x - origin.x) / Logic.ICON + 0.5)
		local row = math.floor((members[index].y - origin.y) / Logic.ICON + 0.5)
		shape.taken[row .. ":" .. col] = true
		local line = shape.rows[row]
		if line then
			line.min = math.min(line.min, col)
			line.max = math.max(line.max, col)
		else
			shape.rows[row] = { min = col, max = col }
		end
		shape.minRow = math.min(shape.minRow or row, row)
		shape.maxRow = math.max(shape.maxRow or row, row)
		shape.minCol = math.min(shape.minCol or col, col)
		shape.maxCol = math.max(shape.maxCol or col, col)
	end
	local half = Logic.ICON / 2
	local touchLeft = shape.x + shape.minCol * Logic.ICON - half <= 1
	local touchRight = shape.x + shape.maxCol * Logic.ICON + half >= contentW - 1
	local touchTop = shape.y + shape.minRow * Logic.ICON - half <= 1
	local touchBottom = shape.y + shape.maxRow * Logic.ICON + half >= contentH - 1
	local leftFlush, rightFlush = 0, 0
	for row = shape.minRow, shape.maxRow do
		local line = shape.rows[row]
		if line and line.min == shape.minCol then
			leftFlush = leftFlush + 1
		end
		if line and line.max == shape.maxCol then
			rightFlush = rightFlush + 1
		end
	end
	shape.right = rightFlush > leftFlush or (rightFlush == leftFlush and touchRight and not touchLeft)
	local topSpan = spanOf(shape.rows[shape.minRow])
	local bottomSpan = spanOf(shape.rows[shape.maxRow])
	local bottom = topSpan < bottomSpan or (topSpan == bottomSpan and touchBottom and not touchTop)
	shape.step = bottom and -1 or 1
	shape.firstRow = bottom and shape.maxRow or shape.minRow
	shape.lastRow = bottom and shape.minRow or shape.maxRow
	local votes = {}
	local width
	for row = shape.firstRow, shape.lastRow - shape.step, shape.step do
		local line = shape.rows[row]
		if line then
			local span = spanOf(line)
			votes[span] = (votes[span] or 0) + 1
			if not width or votes[span] > votes[width] or (votes[span] == votes[width] and span > width) then
				width = span
			end
		end
	end
	if not width then
		width = math.max(spanOf(shape.rows[shape.lastRow]), (blockShape(#members + incoming, contentW, Logic.ICON)))
	end
	if shape.right then
		shape.bandLow, shape.bandHigh = shape.maxCol - width + 1, shape.maxCol
	else
		shape.bandLow, shape.bandHigh = shape.minCol, shape.minCol + width - 1
	end
	return shape
end

local function readingKey(shape, cell)
	local col = cell.col
	if shape.right then
		col = -col
	end
	return (cell.row - shape.firstRow) * shape.step, col
end

local function shapeCells(shape)
	local cells = {}
	local function addRow(row, low, high)
		for col = low, high do
			cells[#cells + 1] = { row = row, col = col }
		end
	end
	local row = shape.firstRow
	while true do
		local line = shape.rows[row]
		local low, high = shape.bandLow, shape.bandHigh
		if line then
			low, high = math.min(low, line.min), math.max(high, line.max)
		end
		addRow(row, low, high)
		if row == shape.lastRow then
			break
		end
		row = row + shape.step
	end
	addRow(shape.lastRow + shape.step, shape.bandLow, shape.bandHigh)
	table.sort(cells, function(a, b)
		local aRow, aCol = readingKey(shape, a)
		local bRow, bCol = readingKey(shape, b)
		if aRow ~= bRow then
			return aRow < bRow
		end
		return aCol < bCol
	end)
	return cells
end

local function growDirection(shape, cell)
	if (cell.row - shape.lastRow) * shape.step > 0 then
		return "flow"
	end
	if (cell.row - shape.firstRow) * shape.step < 0 then
		return "back"
	end
	if cell.col > shape.maxCol then
		return shape.right and "anchor" or "outward"
	end
	if cell.col < shape.minCol then
		return shape.right and "outward" or "anchor"
	end
	return "inside"
end

local function growCells(shape)
	local cells = {}
	local seen = {}
	local sides = { { 1, 0 }, { -1, 0 }, { 0, 1 }, { 0, -1 } }
	for key in pairs(shape.taken) do
		local row, col = key:match("^(-?%d+):(-?%d+)$")
		row, col = tonumber(row), tonumber(col)
		for side = 1, #sides do
			local nextRow, nextCol = row + sides[side][1], col + sides[side][2]
			local nextKey = nextRow .. ":" .. nextCol
			if not shape.taken[nextKey] and not seen[nextKey] then
				seen[nextKey] = true
				local cell = { row = nextRow, col = nextCol }
				cell.rank = GROW_RANK[growDirection(shape, cell)]
				cells[#cells + 1] = cell
			end
		end
	end
	table.sort(cells, function(a, b)
		if a.rank ~= b.rank then
			return a.rank < b.rank
		end
		local aRow, aCol = readingKey(shape, a)
		local bRow, bCol = readingKey(shape, b)
		if aRow ~= bRow then
			return aRow < bRow
		end
		return aCol < bCol
	end)
	return cells
end

local function strangersOf(body, category)
	local strangers = {}
	for index = 1, #body do
		if body[index].category ~= category then
			strangers[#strangers + 1] = body[index]
		end
	end
	return strangers
end

local function openCell(x, y, body, strangers, contentW, contentH, ownFootprint)
	if not Logic.IconInside(x, y, contentW, contentH, Logic.ICON) or overlaps(x, y, body, Logic.ICON) then
		return false
	end
	return ownFootprint or not Logic.TooClose(x, y, strangers, Logic.ICON)
end

local function placeInGroup(members, incoming, body, strangers, contentW, contentH)
	local shape = groupShape(members, incoming, contentW, contentH)
	local function try(cells)
		for index = 1, #cells do
			local cell = cells[index]
			local x = shape.x + cell.col * Logic.ICON
			local y = shape.y + cell.row * Logic.ICON
			local inside = cell.row >= shape.minRow and cell.row <= shape.maxRow and cell.col >= shape.minCol and cell.col <= shape.maxCol
			if openCell(x, y, body, strangers, contentW, contentH, inside) then
				return x, y
			end
		end
	end
	local x, y = try(shapeCells(shape))
	if not x then
		x, y = try(growCells(shape))
	end
	return x, y
end

local function remembered(memory, matches, body, contentW, contentH)
	for index = #memory, 1, -1 do
		local entry = memory[index]
		if matches(entry) and openCell(entry.x, entry.y, body, strangersOf(body, entry.category), contentW, contentH, false) then
			return entry.x, entry.y
		end
	end
end

local function firstOpen(body, strangers, contentW, contentH)
	local half = Logic.ICON / 2
	local lowest = 0
	for index = 1, #body do
		lowest = math.max(lowest, body[index].y + half)
	end
	local limitH = math.min(contentH, lowest + Logic.GAP + Logic.ICON * 2)
	for y = half, limitH - half, Logic.ICON do
		for x = half, contentW - half, Logic.ICON do
			if openCell(x, y, body, strangers, contentW, limitH, false) then
				return x, y
			end
		end
	end
end

local function tagged(homes, category)
	local home = type(homes) == "table" and homes[category]
	return type(home) == "table" and type(home.x) == "number" and type(home.y) == "number"
end

function Logic.PlaceFresh(places, fresh, autoPlace, homes, maxWidth, maxHeight, vacated)
	places = places or {}
	fresh = fresh or {}
	if #places == 0 then
		local blank = {}
		for index = 1, #fresh do
			appendPlace(blank, fresh[index], 0, 0, true)
		end
		return Logic.Tidy(blank, homes)
	end
	local limitW = maxWidth or Logic.BAG_WIDTH
	local limitH = maxHeight or math.huge
	vacated = vacated or {}
	local nextPlaces = {}
	local body = {}
	local heldCount = 0
	for index = 1, #places do
		local place = places[index]
		nextPlaces[index] = place
		if place.held then
			heldCount = heldCount + 1
		else
			body[#body + 1] = { x = place.x, y = place.y, category = Logic.Category(place.classID, place.quality, place.isQuest) }
		end
	end
	local incoming = {}
	for index = 1, #fresh do
		local category = Logic.Category(fresh[index].classID, fresh[index].quality, fresh[index].isQuest)
		incoming[category] = (incoming[category] or 0) + 1
	end
	local zoned = {}
	local entries = {}
	for index = 1, #fresh do
		local item = fresh[index]
		local category = Logic.Category(item.classID, item.quality, item.isQuest)
		local x, y
		if autoPlace then
			local members = {}
			for bodyIndex = 1, #body do
				if body[bodyIndex].category == category then
					members[#members + 1] = body[bodyIndex]
				end
			end
			local strangers = strangersOf(body, category)
			x, y = remembered(vacated, function(entry) return entry.itemID == item.itemID end, body, limitW, limitH)
			if not x and #members > 0 then
				x, y = placeInGroup(largestCluster(members), incoming[category], body, strangers, limitW, limitH)
			elseif not x then
				x, y = remembered(vacated, function(entry) return entry.category == category end, body, limitW, limitH)
				if not x and (zoned[category] or tagged(homes, category)) then
					zoned[category] = true
				elseif not x then
					x, y = firstOpen(body, strangers, limitW, limitH)
				end
			end
		end
		incoming[category] = incoming[category] - 1
		if x then
			appendPlace(nextPlaces, item, x, y, false)
			body[#body + 1] = { x = x, y = y, category = category }
		elseif zoned[category] then
			appendPlace(nextPlaces, item, 0, 0, false)
			local place = nextPlaces[#nextPlaces]
			entries[#entries + 1] = { id = place.itemID .. ":" .. place.inst, category = category, place = place }
		else
			heldCount = heldCount + 1
			local spot = Logic.HoldingPositions(heldCount, Logic.ICON, Logic.BAG_WIDTH).spots[heldCount]
			appendPlace(nextPlaces, item, spot.x, spot.y, true)
		end
	end
	if #entries > 0 then
		local arranged = Logic.Arrange(entries, homes, limitW, maxHeight, body)
		for index = 1, #entries do
			local pos = arranged.positions[entries[index].id]
			if pos then
				entries[index].place.x = pos.x
				entries[index].place.y = pos.y
			end
		end
	end
	return nextPlaces
end

function Logic.FileHeld(places, homes, maxWidth, maxHeight, vacated)
	places = places or {}
	local kept = {}
	local fresh = {}
	for index = 1, #places do
		local place = places[index]
		if place.held then
			fresh[#fresh + 1] = {
				bag = place.bag,
				slot = place.slot,
				itemID = place.itemID,
				count = place.count,
				classID = place.classID,
				quality = place.quality,
				isQuest = place.isQuest,
			}
		else
			kept[#kept + 1] = place
		end
	end
	if #fresh == 0 then
		return places
	end
	return Logic.PlaceFresh(kept, fresh, true, homes, maxWidth, maxHeight, vacated)
end

function Logic.ClassifyClick(button, mods, moved, exited)
	mods = mods or {}
	if moved and exited and button == "LeftButton" then
		return "pickup"
	end
	if moved and button == "LeftButton" and not mods.alt then
		return "layout"
	end
	if mods.chat and mods.shift then
		return "link"
	end
	if mods.split then
		return "split"
	end
	if mods.ctrl then
		return "dress"
	end
	if mods.alt and button == "LeftButton" then
		return "pickup"
	end
	if button == "RightButton" then
		return "use"
	end
	return "ignore"
end

function Logic.Connected(members, leaderKey, icon, sameType)
	members = members or {}
	icon = icon or Logic.ICON
	local slack = Logic.GRID
	local leader
	for index = 1, #members do
		if members[index].key == leaderKey then
			leader = members[index]
		end
	end
	if not leader then
		return {}
	end
	local seen = {}
	local queue = { leader }
	local group = {}
	seen[leaderKey] = true
	local head = 1
	while queue[head] do
		local current = queue[head]
		head = head + 1
		group[#group + 1] = current
		for index = 1, #members do
			local other = members[index]
			local sameKind = (not sameType) or other.category == leader.category
			if sameKind and not seen[other.key] and cardinalNeighbor(current, other, icon, slack) then
				seen[other.key] = true
				queue[#queue + 1] = other
			end
		end
	end
	return group
end

function Logic.DragMembers(shiftDown, altDown, members, leaderKey, icon, sameType)
	members = members or {}
	local leader
	for index = 1, #members do
		if members[index].key == leaderKey then
			leader = members[index]
		end
	end
	if not leader then
		return {}
	end
	if altDown or not shiftDown then
		return { leader }
	end
	return Logic.Connected(members, leaderKey, icon, sameType)
end

function Logic.ShiftGroup(originX, originY, snappedX, snappedY, starts)
	starts = starts or {}
	local dx = snappedX - originX
	local dy = snappedY - originY
	local moved = {}
	for index = 1, #starts do
		local start = starts[index]
		moved[index] = { x = start.x + dx, y = start.y + dy }
	end
	return moved
end

function Logic.FillSaved(saved)
	if type(saved) ~= "table" then
		saved = {}
	end
	if type(saved.scale) ~= "number" then
		saved.scale = 1
	end
	if saved.columns == nil then
		saved.columns = 4
	end
	if type(saved.slots) ~= "table" then
		saved.slots = {}
	end
	if type(saved.staging) ~= "table" then
		saved.staging = {}
	end
	if type(saved.frame) ~= "table" then
		saved.frame = { id = 1, x = 0, y = 0 }
	end
	return saved
end

function Logic.Apply(layout, command)
	if type(layout) ~= "table" or type(command) ~= "table" then
		return nil
	end
	if command.op == "move_frame" then
		local window = command.window
		if window == nil then
			window = 1
		end
		if window ~= 1 then
			return nil
		end
		if type(command.x) == "number" then
			layout.x = command.x
		end
		if type(command.y) == "number" then
			layout.y = command.y
		end
		if type(command.point) == "string" then
			layout.point = command.point
		end
		if type(command.relPoint) == "string" then
			layout.relPoint = command.relPoint
		end
		if type(layout.frame) == "table" then
			if type(command.x) == "number" then
				layout.frame.x = command.x
			end
			if type(command.y) == "number" then
				layout.frame.y = command.y
			end
			layout.frame.id = 1
		end
		return layout
	end
	if command.op == "move_item" then
		if type(command.place) == "table" then
			if type(command.x) == "number" then
				command.place.x = command.x
			end
			if type(command.y) == "number" then
				command.place.y = command.y
			end
			if command.held ~= nil then
				command.place.held = command.held == true
			end
			return layout
		end
		if command.staging ~= nil then
			if type(layout.staging) ~= "table" or type(layout.slots) ~= "table" then
				return nil
			end
			local item = layout.staging[command.staging]
			if item == nil then
				return nil
			end
			local to = command.to or 1
			layout.slots[to] = item
			table.remove(layout.staging, command.staging)
			return layout
		end
		if type(layout.slots) ~= "table" then
			return nil
		end
		local moving = layout.slots[command.from]
		layout.slots[command.from] = layout.slots[command.to]
		layout.slots[command.to] = moving
		return layout
	end
	if command.op == "refresh" then
		return layout
	end
	return nil
end

function Logic.TickApplies(state)
	if type(state) ~= "table" or state.mode ~= "dragging" then
		return 0
	end
	return 1
end

function Logic.QueueProtected(queue, command, inCombat)
	if type(queue) ~= "table" or type(command) ~= "table" then
		return nil
	end
	if inCombat and command.protected then
		queue[#queue + 1] = command
		queue.applied = queue.applied or 0
		return "deferred"
	end
	queue.applied = (queue.applied or 0) + 1
	return "applied"
end

function Logic.CountBagApplies(eventCount, slots)
	local applies = 0
	local total = 0
	local function layoutSum(rows)
		local sum = 0
		for index = 1, #rows do
			sum = sum + rows[index]
		end
		return sum
	end
	for _ = 1, eventCount do
		applies = applies + 1
		total = total + layoutSum(slots)
	end
	return applies, total
end

function Logic.Step(state, event)
	if event == "detach" then
		return nil, "illegal"
	end
	if type(state) ~= "table" then
		return nil, "illegal"
	end
	if event == "down" and state.mode == "idle" then
		return { mode = "pressed" }
	end
	if event == "move" and state.mode == "pressed" then
		return { mode = "dragging" }
	end
	if event == "up" and state.mode == "dragging" then
		return { mode = "idle" }
	end
	if event == "up" and state.mode == "pressed" then
		return { mode = "idle", click = true }
	end
	if event == "use" and state.mode == "dragging" then
		return nil, "use_during_drag"
	end
	if event == "use" and state.mode == "idle" then
		return { mode = "idle", used = true }
	end
	return nil, "illegal"
end
