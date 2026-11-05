-- Leyline: the one-time choice on a character's first login. Loose Bags is the default; the standard bags are one
-- click away, and /lbags off or /lbags on switches later. Closing the window keeps Loose Bags.
local Logic = LooseBagsLogic
local LB = Logic.LB
local C = LB.Compat

LB.CHOICE_TITLE = "Bags"
LB.CHOICE_ASK = "How would you like your bags?"
LB.CHOICE_LOOSE = "One loose backpack"
LB.CHOICE_LOOSE_NOTE = "Items stay where you leave them."
LB.CHOICE_STANDARD = "The standard bags"
LB.CHOICE_STANDARD_NOTE = "One window per bag, slot by slot."
LB.CHOICE_LATER = "Change it any time: /lbags off or /lbags on"

local function Choose(which)
	local frame = LB.choiceFrame
	if frame then
		frame.chose = true
		frame:Hide()
	end
	if which == "standard" then
		LB.TurnOff(true)
		LB.Say("Standard bags. /lbags on switches to Loose Bags.")
	else
		LB.TurnOn(true, false)
		LB.Say("Loose Bags on. Press B to open it. /lbags off switches to the standard bags.")
	end
end
LB.Choose = Choose

local function Option(frame, name, text, note, x, which)
	local button, made = C.Make("Button", name, frame, "UIPanelButtonTemplate")
	if not made and button.SetNormalFontObject and GameFontNormal then
		button:SetNormalFontObject(GameFontNormal)
	end
	button:SetSize(160, 26)
	if LB.Skin then
		LB.Skin.TextButton(button)
	end
	button:SetPoint("TOP", frame, "TOP", x, -64)
	button:SetText(text)
	button:SetScript("OnClick", function()
		Choose(which)
	end)
	local line = frame:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
	line:SetPoint("TOP", button, "BOTTOM", 0, -6)
	line:SetWidth(160)
	line:SetText(note)
	button.note = line
	return button
end

local function Ensure()
	if LB.choiceFrame then
		return LB.choiceFrame
	end
	local frame = CreateFrame("Frame", "LooseBagsChoice", UIParent)
	frame:SetSize(372, 156)
	frame:SetPoint("CENTER", UIParent, "CENTER", 0, 120)
	frame:SetFrameStrata("DIALOG")
	frame:EnableMouse(true)
	frame:SetMovable(true)
	frame:SetClampedToScreen(true)
	frame:RegisterForDrag("LeftButton")
	frame:SetScript("OnDragStart", function(self)
		self:StartMoving()
	end)
	frame:SetScript("OnDragStop", function(self)
		self:StopMovingOrSizing()
	end)
	-- Forever UI's dialog frame when it is loaded (Skin.lua), else the client's dialog border.
	LB.Skin.Panel(frame)
	local title = frame:CreateFontString(nil, "OVERLAY", "GameFontNormalLarge")
	title:SetPoint("TOP", frame, "TOP", 0, -18)
	title:SetText(LB.CHOICE_TITLE)
	frame.title = title
	local ask = frame:CreateFontString(nil, "OVERLAY", "GameFontHighlight")
	ask:SetPoint("TOP", title, "BOTTOM", 0, -8)
	ask:SetText(LB.CHOICE_ASK)
	frame.ask = ask
	frame.Loose = Option(frame, "LooseBagsChoiceLoose", LB.CHOICE_LOOSE, LB.CHOICE_LOOSE_NOTE, -86, "loose")
	frame.Standard = Option(frame, "LooseBagsChoiceStandard", LB.CHOICE_STANDARD, LB.CHOICE_STANDARD_NOTE, 86, "standard")
	-- The default is lit.
	if frame.Loose.LockHighlight then
		frame.Loose:LockHighlight()
	end
	local later = frame:CreateFontString(nil, "OVERLAY", "GameFontDisableSmall")
	later:SetPoint("BOTTOM", frame, "BOTTOM", 0, 18)
	later:SetText(LB.CHOICE_LATER)
	frame.later = later
	local close = C.Make("Button", "LooseBagsChoiceClose", frame, "UIPanelCloseButton")
	LB.Skin.Close(close, frame, -6, -6)
	close:SetScript("OnClick", function()
		frame:Hide()
	end)
	frame:Hide()
	-- Closed without a pick (the X or Esc): the default stays, and the question is not asked again. Hidden only
	-- because the whole interface hid (Alt-Z, the new character's opening scene): still shown, still asking.
	frame:SetScript("OnHide", function(self)
		if not self.chose and not self:IsShown() then
			Choose("loose")
		end
	end)
	C.Special("LooseBagsChoice")
	LB.choiceFrame = frame
	return frame
end
LB.EnsureChoice = Ensure

function LB.ChoicePending()
	local char = LB.Char()
	if char == LB.unkeyed then
		return false
	end
	return char.chosen ~= true
end

-- Not in Your Den (the launcher's Den Lite realm, WrathUnleashedUI W.DEN_REALM): a building room, where the window
-- came up over the editor at the first visit (review 4 Nov). The choice waits, still pending, for a real realm.
LB.DEN_REALM = "Your Den"
function LB.InDen()
	if not GetRealmName then
		return false
	end
	local ok, name = pcall(GetRealmName)
	return ok and name == LB.DEN_REALM
end

function LB.OfferChoice()
	if not LB.ChoicePending() or LB.InDen() then
		return false
	end
	local frame = Ensure()
	if frame:IsShown() then
		return true
	end
	frame.chose = false
	frame:Show()
	return true
end
