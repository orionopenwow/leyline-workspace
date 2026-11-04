-- Loose Bags by Eric R (MIT, see LICENSE.txt), ported to the 3.3.5a client for Leyline.
-- The client's bag toggles (B, the backpack and bag slot buttons, Shift-B and the rest) open Loose Bags while it is
-- on; while it is off every call goes to the client's own function, so the standard bags (and Forever UI's look for
-- them, which only hooks the standard bag frames) work as if Loose Bags were not there.
local Logic = LooseBagsLogic
local LB = Logic.LB
local C = LB.Compat

local originals = {}
local installed = false
local closing = false

-- The backpack (0) and the four bag slots. The keyring (-2) and the bank's bags stay the standard ones.
local function PlayerBag(id)
	if type(id) ~= "number" then
		return false
	end
	return id >= 0 and id <= (NUM_BAG_SLOTS or NUM_BAG_FRAMES or 4)
end
LB.PlayerBag = PlayerBag

-- Hide any standard bag window that shows one of the player's bags (switching on, or something that opened one
-- without the toggles).
function LB.CloseStock()
	if closing or not LB.Enabled() then
		return
	end
	closing = true
	local count = NUM_CONTAINER_FRAMES or 13
	for index = 1, count do
		local frame = _G["ContainerFrame" .. index]
		if frame and frame.IsShown and frame:IsShown() and frame.GetID and PlayerBag(frame:GetID()) then
			frame:Hide()
		end
	end
	closing = false
end

function LB.PickupSlot(bag, slot)
	if type(bag) ~= "number" or type(slot) ~= "number" then
		return
	end
	local locked = type(InCombatLockdown) == "function" and InCombatLockdown()
	if locked then
		LB.pending = LB.pending or { applied = 0 }
		Logic.QueueProtected(LB.pending, {
			op = "pickup",
			protected = true,
			bag = bag,
			slot = slot,
		}, true)
		return
	end
	C.Call("PickupContainerItem", bag, slot)
end

function LB.FlushProtected()
	local queue = LB.pending
	if type(queue) ~= "table" then
		return
	end
	local waiting = {}
	for index = 1, #queue do
		waiting[index] = queue[index]
	end
	LB.pending = { applied = 0 }
	for index = 1, #waiting do
		local command = waiting[index]
		local result = Logic.QueueProtected(LB.pending, command, false)
		if result == "applied" and command.op == "pickup" then
			C.Call("PickupContainerItem", command.bag, command.slot)
		end
	end
end

-- A window asking for the bags (a vendor, the mailbox, the bank) leaves Loose Bags as it is, as the original
-- does: it opens only on B, the bag buttons or /lbags.
local function ShowOurs()
	if not Logic.ShouldOpen("restore") then
		return
	end
	if LB.Show then
		LB.Show()
	end
end

-- The backpack and bag slot buttons are lit while a standard bag window is open; they follow ours instead. (They
-- flip themselves when clicked, so this runs again on the next frame.)
function LB.SyncBagButtons()
	if not LB.Enabled() then
		return
	end
	local open = LB.frame and LB.frame:IsShown() and true or false
	local buttons = { MainMenuBarBackpackButton }
	for index = 0, (NUM_BAG_SLOTS or 4) - 1 do
		buttons[#buttons + 1] = _G["CharacterBag" .. index .. "Slot"]
	end
	for index = 1, #buttons do
		local button = buttons[index]
		if button and button.SetChecked then
			pcall(button.SetChecked, button, open)
		end
	end
end

local function Toggled()
	LB.Toggle()
	C.After(0, LB.SyncBagButtons)
end

local function Pass(name, ...)
	local fn = originals[name]
	if type(fn) == "function" then
		return fn(...)
	end
end

function LB.Install()
	if installed then
		return
	end
	installed = true
	originals.ToggleBackpack = ToggleBackpack
	originals.ToggleBag = ToggleBag
	originals.OpenBag = OpenBag
	originals.OpenBackpack = OpenBackpack
	originals.OpenAllBags = OpenAllBags
	originals.ToggleAllBags = ToggleAllBags
	originals.CloseAllBags = CloseAllBags
	LB.originals = originals

	ToggleBackpack = function(...)
		if LB.Enabled() then
			Toggled()
			return
		end
		return Pass("ToggleBackpack", ...)
	end

	ToggleBag = function(id, ...)
		if LB.Enabled() and PlayerBag(id) then
			Toggled()
			return
		end
		return Pass("ToggleBag", id, ...)
	end

	OpenBag = function(id, ...)
		if LB.Enabled() and PlayerBag(id) then
			ShowOurs()
			return
		end
		return Pass("OpenBag", id, ...)
	end

	OpenBackpack = function(...)
		if LB.Enabled() then
			ShowOurs()
			return
		end
		return Pass("OpenBackpack", ...)
	end

	-- 3.3.5a: B is OpenAllBags() and toggles; a window that wants the bags open passes true (forceOpen).
	OpenAllBags = function(forceOpen, ...)
		if LB.Enabled() then
			if forceOpen then
				ShowOurs()
			else
				LB.Toggle()
			end
			return
		end
		return Pass("OpenAllBags", forceOpen, ...)
	end

	if type(originals.ToggleAllBags) == "function" then
		ToggleAllBags = function(...)
			if LB.Enabled() then
				LB.Toggle()
				return
			end
			return Pass("ToggleAllBags", ...)
		end
	end

	CloseAllBags = function(...)
		if LB.Enabled() and LB.Hide then
			LB.Hide()
		end
		return Pass("CloseAllBags", ...)
	end

	-- Anything that still builds a standard bag window for a player bag while Loose Bags is on: closed again.
	if type(hooksecurefunc) == "function" and type(ContainerFrame_GenerateFrame) == "function" then
		hooksecurefunc("ContainerFrame_GenerateFrame", function(frame)
			if LB.Enabled() and frame and frame.GetID and PlayerBag(frame:GetID()) then
				LB.CloseStock()
			end
		end)
	end
end
