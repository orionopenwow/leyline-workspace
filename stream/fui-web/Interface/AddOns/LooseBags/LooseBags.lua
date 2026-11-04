-- Loose Bags by Eric R (MIT, see LICENSE.txt), ported to the 3.3.5a client for Leyline.
-- Saved layout, reading the bags, Clean Up, the slash command and the events.
local Logic = LooseBagsLogic
local LB = Logic.LB
local C = LB.Compat

local ADDON = "LooseBags"

function LB.Say(text)
	local line = "|cffffd100Loose Bags:|r " .. text
	if DEFAULT_CHAT_FRAME and DEFAULT_CHAT_FRAME.AddMessage then
		DEFAULT_CHAT_FRAME:AddMessage(line)
	elseif print then
		print(line)
	end
end

function LB.Account()
	if type(LooseBagsDB) ~= "table" then
		LooseBagsDB = {}
	end
	return Logic.FillSaved(Logic.NormalizeAccount(LooseBagsDB))
end

function LB.Char()
	local account = LB.Account()
	local key = nil
	if type(UnitName) == "function" and type(GetRealmName) == "function" then
		key = Logic.CharKey(UnitName("player"), GetRealmName())
	end
	if not key then
		LB.unkeyed = LB.unkeyed or Logic.NormalizeChar({})
		return LB.unkeyed
	end
	local row = account.chars[key]
	if type(row) ~= "table" then
		row = {}
		account.chars[key] = row
	end
	return Logic.NormalizeChar(row)
end

-- On or off is per character (Leyline: the first-login choice, Choice.lua). Until a character chooses, Loose Bags
-- is on. The original's account-wide switch (LooseBagsDB.enabled) is no longer read.
function LB.Enabled()
	return LB.Char().bags ~= "standard"
end

function LB.SetEnabled(on)
	local char = LB.Char()
	char.bags = on and "loose" or "standard"
	char.chosen = true
end

function LB.Plain(value)
	if type(value) ~= "number" then
		return nil
	end
	return value
end

function LB.Text(value)
	if type(value) ~= "string" then
		return nil
	end
	return value
end

-- The backpack and the four bag slots (3.3.5a has no reagent bag; the keyring stays the standard one).
local function BagList()
	local bags = { 0 }
	local last = NUM_BAG_SLOTS or NUM_BAG_FRAMES or 4
	for bag = 1, last do
		bags[#bags + 1] = bag
	end
	return bags
end

function LB.BagList()
	return BagList()
end

function LB.ReadSlot(bag, slot)
	local info = C.SlotInfo(bag, slot)
	if not info then
		return nil
	end
	return {
		bag = bag,
		slot = slot,
		itemID = info.itemID,
		count = info.stackCount or 1,
		classID = info.classID,
		quality = info.quality,
		isQuest = info.isQuestItem == true,
		questID = info.questID,
		questActive = info.questActive,
		icon = info.iconFileID,
		link = LB.Text(info.hyperlink),
		name = LB.Text(info.itemName) or (LB.Text(info.hyperlink) and string.match(info.hyperlink, "%[(.+)%]")),
		kind = LB.Text(info.itemType),
		subKind = LB.Text(info.itemSubType),
		locked = info.isLocked == true,
	}
end

function LB.Scan()
	local live = {}
	local backpackSlots = 0
	local bags = BagList()
	for index = 1, #bags do
		local bag = bags[index]
		local slots = C.NumSlots(bag)
		if bag == 0 then
			backpackSlots = slots
		end
		for slot = 1, slots do
			local item = LB.ReadSlot(bag, slot)
			if item then
				live[#live + 1] = item
			end
		end
	end
	return live, backpackSlots
end

-- The settings live in a drawer beside the window and the bag slots row adds to the window's height (Place.lua), so
-- the room for items, and a size chosen with the grip, never depend on either.
local function SettingsBar(char)
	return 0
end

function LB.BarHeight(char)
	char = char or LB.Char()
	if char.bagRow then
		return Logic.BAG_ROW
	end
	return 0
end

local function CountHeld(places)
	local count = 0
	for index = 1, #places do
		if places[index].held then
			count = count + 1
		end
	end
	return count
end

local function ViewSize()
	if not UIParent or not UIParent.GetWidth or not UIParent.GetHeight then
		return nil, nil
	end
	return LB.Plain(UIParent:GetWidth()), LB.Plain(UIParent:GetHeight())
end

-- The room new items, Clean Up and the zones work in. A size chosen with the grip gives its own; a bag that fits its
-- items uses the width its items already take (at least the default) and a square of it.
local function WorkBox(char)
	if type(char.width) == "number" and type(char.height) == "number" then
		return Logic.ContentBox(char.width, char.height, SettingsBar(char), CountHeld(char.places))
	end
	local _, _, boxW = Logic.AutoSize(char.places, 0, nil, nil)
	if not boxW or boxW < Logic.BAG_WIDTH then
		boxW = Logic.BAG_WIDTH
	end
	return boxW, boxW
end
LB.WorkBox = WorkBox

function LB.Sync()
	local char = LB.Char()
	local live, backpackSlots = LB.Scan()
	if #char.places > 0 and not Logic.ReadingUsable(backpackSlots) then
		return char
	end
	local matched = Logic.Match(char.places, live)
	LB.vacated = Logic.Vacate(LB.vacated, matched.vacated, matched.places)
	local boxW, boxH = WorkBox(char)
	if type(char.width) == "number" and type(char.height) == "number" then
		char.homes = Logic.ClampHomes(char.homes, boxW, boxH)
	end
	char.places = Logic.PlaceFresh(matched.places, matched.unmatched, char.autoPlace, Logic.ActiveHomes(char.useZones, char.homes), boxW, boxH, LB.vacated)
	if Logic.HeldOverflow(char.places) and not (char.useZones and Logic.HasHomes(char.homes)) then
		char.places = Logic.Tidy(char.places)
	end
	return char
end

function LB.Refresh()
	LB.Sync()
	if LB.Layout then
		LB.Layout()
	end
end

function LB.Sort()
	local char = LB.Sync()
	local bar = SettingsBar(char)
	local tidyW, tidyH = Logic.BAG_WIDTH, Logic.BAG_WIDTH
	local saved = type(char.width) == "number" and type(char.height) == "number"
	if saved then
		tidyW, tidyH = Logic.ContentBox(char.width, char.height, bar, CountHeld(char.places))
	end
	local homes = Logic.ActiveHomes(char.useZones, char.homes)
	local keepOrder = char.autoOrder == false
	if not saved then
		-- Fitted to its items: the tidy grid from the top left, wider only if it would not fit on the screen.
		local _, screenH = ViewSize()
		char.places = Logic.AutoTidy(char.places, homes, keepOrder, screenH)
		if LB.Layout then
			LB.Layout()
		end
		return
	end
	char.places = Logic.Tidy(char.places, homes, tidyW, tidyH, keepOrder)
	if saved then
		local screenW, screenH = ViewSize()
		local grownW, grownH = Logic.GrownSize(char.width, char.height, char.places, bar, screenW, screenH)
		if grownW ~= char.width then
			local contentW, contentH = Logic.ContentBox(grownW, grownH, bar, CountHeld(char.places))
			char.places = Logic.Tidy(char.places, homes, contentW, contentH, keepOrder)
			grownW, grownH = Logic.GrownSize(grownW, grownH, char.places, bar, screenW, screenH)
		end
		char.width = grownW
		char.height = grownH
	end
	if LB.Layout then
		LB.Layout()
	end
end

function LB.ApplyFirstRun(reason)
	local char = LB.Char()
	if char == LB.unkeyed then
		return false
	end
	if not Logic.FirstRunPending(char) then
		return false
	end
	local items = LB.Scan()
	local slots = 0
	local bags = BagList()
	for index = 1, #bags do
		slots = slots + C.NumSlots(bags[index])
	end
	if not Logic.FirstRunReady(reason, #items, slots) then
		return false
	end
	-- Leyline: Clean Up's tidy grid from the top left, and the window fitted to the items (no size remembered).
	local _, screenH = ViewSize()
	local plan = Logic.PlanAutoFirstRun(items, {
		screenH = screenH,
		homes = Logic.ActiveHomes(char.useZones, char.homes),
		keepOrder = char.autoOrder == false,
	})
	return Logic.CommitAutoFirstRun(char, plan)
end

function LB.FileHeld()
	local char = LB.Char()
	local boxW, boxH = WorkBox(char)
	char.places = Logic.FileHeld(char.places, Logic.ActiveHomes(char.useZones, char.homes), boxW, boxH, LB.vacated)
	if type(char.width) == "number" and type(char.height) == "number" then
		local bar = SettingsBar(char)
		local screenW, screenH = ViewSize()
		char.width, char.height = Logic.GrownSize(char.width, char.height, char.places, bar, screenW, screenH)
	end
	if LB.Layout then
		LB.Layout()
	end
end

function LB.ResetLayout()
	local char = LB.Char()
	char.places = {}
	char.homes = {}
	char.width = nil
	char.height = nil
	LB.zoneEdit = false
	LB.Refresh()
	if LB.RefreshSettings then
		LB.RefreshSettings()
	end
	LB.Say("Layout cleared. The bag fits its items again.")
end

function LB.Help()
	if LB.OpenHelp then
		LB.OpenHelp()
		return
	end
	local rows = Logic.HELP or {}
	for index = 1, #rows do
		local row = rows[index]
		LB.Say(row.gesture .. ". " .. row.detail)
	end
end

-- Off: our bag closes and B, the bag buttons and the rest open the standard bags again (Hooks.lua passes them on).
function LB.TurnOff(quiet)
	LB.SetEnabled(false)
	if LB.frame then
		LB.frame:Hide()
	end
	if LB.helpFrame then
		LB.helpFrame:Hide()
	end
	if not quiet then
		LB.Say("Standard bags back. /lbags on brings Loose Bags back.")
	end
end

function LB.TurnOn(quiet, open)
	LB.SetEnabled(true)
	if LB.CloseStock then
		LB.CloseStock()
	end
	if open ~= false then
		LB.Show()
	end
	if not quiet then
		LB.Say("Loose Bags on. /lbags off goes back to the standard bags.")
	end
end

local function Slash(msg)
	msg = string.lower(msg or "")
	msg = string.gsub(msg, "^%s+", "")
	msg = string.gsub(msg, "%s+$", "")
	if msg == "help" then
		LB.Help()
		return
	end
	if msg == "reset" then
		LB.ResetLayout()
		return
	end
	if msg == "off" then
		LB.TurnOff()
		return
	end
	if msg == "on" then
		LB.TurnOn()
		return
	end
	if msg == "sort" then
		LB.Sort()
		return
	end
	LB.Toggle()
end

SLASH_LOOSEBAGS1 = "/lbags"
SLASH_LOOSEBAGS2 = "/loosebags"
if type(SlashCmdList) == "table" then
	SlashCmdList["LOOSEBAGS"] = Slash
end
LB.Slash = Slash

-- 3.3.5a has no BAG_UPDATE_DELAYED: a burst of BAG_UPDATE (loot, a vendor, a sort) becomes one pass on the next frame.
local delayedQueued = false
local function Delayed()
	delayedQueued = false
	LB.OnEvent(nil, "BAG_UPDATE_DELAYED")
end

local function QueueDelayed()
	if delayedQueued then
		return
	end
	delayedQueued = true
	C.After(0, Delayed)
end
LB.QueueDelayed = QueueDelayed

function LB.OnEvent(_, event, name)
	if event == "ADDON_LOADED" and name == ADDON then
		LB.Account()
		return
	end
	if event == "PLAYER_LOGIN" then
		if LB.Install then
			LB.Install()
		end
		Logic.BeginSession(LB.Char())
		if LB.Ensure then
			LB.Ensure()
		end
		return
	end
	if event == "PLAYER_ENTERING_WORLD" then
		if LB.OfferChoice then
			LB.OfferChoice()
		end
		QueueDelayed()
		return
	end
	if event == "PLAYER_MONEY" then
		if LB.UpdateMoney then
			LB.UpdateMoney()
		end
		return
	end
	if event == "PLAYER_REGEN_ENABLED" then
		if LB.FlushProtected then
			LB.FlushProtected()
		end
		return
	end
	if event == "BAG_UPDATE" then
		QueueDelayed()
		return
	end
	if event == "BAG_UPDATE_DELAYED" then
		if LB.Enabled() then
			LB.ApplyFirstRun("delayed")
		end
		if LB.frame and LB.frame:IsShown() then
			Logic.Apply(LB.Char(), { op = "refresh" })
			LB.Refresh()
			if LB.RefreshBagRow then
				LB.RefreshBagRow()
			end
		end
		return
	end
	if event == "ITEM_LOCK_CHANGED" then
		if LB.frame and LB.frame:IsShown() and LB.Layout then
			LB.Layout()
		end
		return
	end
	if event == "BAG_UPDATE_COOLDOWN" then
		if LB.frame and LB.frame:IsShown() and LB.UpdateCooldowns then
			LB.UpdateCooldowns()
		end
	end
end

local events = CreateFrame("Frame")
for index = 1, #Logic.Events do
	events:RegisterEvent(Logic.Events[index])
end
events:SetScript("OnEvent", LB.OnEvent)
LB.events = events
