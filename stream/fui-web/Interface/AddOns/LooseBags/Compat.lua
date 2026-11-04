-- Loose Bags by Eric R (MIT, see LICENSE.txt), ported to the 3.3.5a client for Leyline.
-- The newer client's helpers, made from what 3.3.5a has: container reads, item class, a timer, small widget shims.
-- Every game call is looked up when used and checked first, so a client without one (the browser client) just
-- gets less, never an error.
local Logic = LooseBagsLogic
local LB = {}
Logic.LB = LB
local C = {}
LB.Compat = C

-- ---------------------------------------------------------------- calls that may be missing
local function pack(...)
	return { n = select("#", ...), ... }
end

local function call(name, ...)
	local fn = _G[name]
	if type(fn) ~= "function" then
		return nil
	end
	local result = pack(pcall(fn, ...))
	if not result[1] then
		return nil
	end
	return unpack(result, 2, result.n)
end
C.Call = call

-- ---------------------------------------------------------------- the player's bags
function C.NumSlots(bag)
	local n = call("GetContainerNumSlots", bag)
	if type(n) ~= "number" then
		return 0
	end
	return n
end

function C.ItemIDOf(link)
	if type(link) ~= "string" then
		return nil
	end
	local id = string.match(link, "item:(%-?%d+)")
	return tonumber(id)
end

-- The newer client's item class numbers (Logic.Category reads them). 3.3.5a names the class instead; the
-- auction house list gives the same names in the player's own language, in this order.
local AUCTION_ORDER = { 2, 4, 1, 0, 16, 7, 6, 11, 9, 3, 15, 12 }
local ENGLISH = {
	["Consumable"] = 0, ["Container"] = 1, ["Weapon"] = 2, ["Gem"] = 3, ["Armor"] = 4, ["Reagent"] = 5,
	["Projectile"] = 6, ["Trade Goods"] = 7, ["Recipe"] = 9, ["Quiver"] = 11, ["Quest"] = 12, ["Key"] = 13,
	["Miscellaneous"] = 15, ["Glyph"] = 16,
}
local classByName
local function ClassMap()
	if classByName then
		return classByName
	end
	local map = {}
	for name, id in pairs(ENGLISH) do
		map[name] = id
	end
	local names = { call("GetAuctionItemClasses") }
	for index = 1, #names do
		if type(names[index]) == "string" and AUCTION_ORDER[index] then
			map[names[index]] = AUCTION_ORDER[index]
		end
	end
	classByName = map
	return map
end

function C.ClassID(className)
	if type(className) ~= "string" then
		return nil
	end
	return ClassMap()[className]
end

-- One slot as the original's C_Container.GetContainerItemInfo table, or nil for an empty slot.
-- 3.3.5a: texture, count, locked, quality, readable, lootable, link = GetContainerItemInfo(bag, slot)
function C.SlotInfo(bag, slot)
	local texture, count, locked, quality, _, _, link = call("GetContainerItemInfo", bag, slot)
	if not texture and not link then
		return nil
	end
	if type(link) ~= "string" then
		link = call("GetContainerItemLink", bag, slot)
	end
	local itemID = C.ItemIDOf(link)
	if not itemID then
		return nil
	end
	local name, _, infoQuality, _, _, className, subClassName = call("GetItemInfo", link)
	if type(quality) ~= "number" or quality < 0 then
		quality = infoQuality
	end
	local isQuest, questID, questActive = call("GetContainerItemQuestInfo", bag, slot)
	return {
		itemID = itemID,
		stackCount = type(count) == "number" and count or 1,
		quality = type(quality) == "number" and quality or nil,
		classID = C.ClassID(className),
		isQuestItem = isQuest and true or false,
		questID = questID,
		questActive = questActive and true or false,
		isLocked = locked and true or false,
		iconFileID = texture,
		hyperlink = link,
		itemName = name,
		itemType = className,
		itemSubType = subClassName,
	}
end

-- Can the item on the cursor go into this bag? (A soul bag, quiver or herb bag takes only its own kind.)
function C.BagTakesCursor(bag)
	local _, bagType = call("GetContainerNumFreeSlots", bag)
	if type(bagType) ~= "number" or bagType == 0 then
		return true
	end
	local kind, _, link = call("GetCursorInfo")
	if kind ~= "item" then
		return false
	end
	local family = call("GetItemFamily", link)
	if type(family) ~= "number" or not bit or type(bit.band) ~= "function" then
		return false
	end
	return bit.band(family, bagType) > 0
end

-- ---------------------------------------------------------------- a timer (C_Timer.After)
local waiting = {}
local timer
function C.After(delay, fn)
	if type(fn) ~= "function" then
		return
	end
	if not timer then
		timer = CreateFrame("Frame")
		timer:Hide()
		timer:SetScript("OnUpdate", function(self, elapsed)
			local now = waiting
			waiting = {}
			for index = 1, #now do
				local job = now[index]
				job.left = job.left - (elapsed or 0)
				if job.left <= 0 then
					local ok, err = pcall(job.fn)
					if not ok and type(geterrorhandler) == "function" then
						geterrorhandler()(err)
					end
				else
					waiting[#waiting + 1] = job
				end
			end
			if #waiting == 0 then
				self:Hide()
			end
		end)
	end
	waiting[#waiting + 1] = { left = delay or 0, fn = fn }
	timer:Show()
end

-- ---------------------------------------------------------------- widgets
function C.SetShown(region, shown)
	if not region then
		return
	end
	if shown then
		region:Show()
	else
		region:Hide()
	end
end

function C.SetResizeBounds(frame, minW, minH, maxW, maxH)
	if frame.SetResizeBounds then
		frame:SetResizeBounds(minW, minH, maxW, maxH)
		return
	end
	if frame.SetMinResize and minW and minH then
		frame:SetMinResize(minW, minH)
	end
	if frame.SetMaxResize and maxW and maxH then
		frame:SetMaxResize(maxW, maxH)
	end
end

function C.Solid(texture, r, g, b, a)
	if texture.SetColorTexture then
		texture:SetColorTexture(r, g, b, a)
	else
		texture:SetTexture(r, g, b, a)
	end
end

-- CreateFrame with a template the client may not have: the template, else the plain widget.
function C.Make(kind, name, parent, template)
	if template then
		local ok, frame = pcall(CreateFrame, kind, name, parent, template)
		if ok and frame then
			return frame, true
		end
	end
	return CreateFrame(kind, name, parent), false
end

-- 3.3.5a's PlaySound takes the sound's name.
function C.Sound(name)
	call("PlaySound", name)
end

function C.Special(name)
	if type(UISpecialFrames) ~= "table" then
		return
	end
	for index = 1, #UISpecialFrames do
		if UISpecialFrames[index] == name then
			return
		end
	end
	table.insert(UISpecialFrames, name)
end
