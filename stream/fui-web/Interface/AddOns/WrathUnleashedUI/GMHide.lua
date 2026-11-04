-- The GM panels (AzerothAdmin's window and its side bar, MiniGM's bar) belong to Your Den, not to the official realm
-- (owner, 4 Nov: "azeroth admin shouldnt be on wrath unleashed"). Outside the Den they are hidden and kept hidden;
-- /gmbar shows them again for this session (a GM who needs them), /gmbar off hides them again.
local W = WrathUnleashedUI
if not W then return end

local FRAMES = {
    "ma_bgframe", "ma_minibgframe", "ma_menubgframe",
    "MiniGMFrame", "MiniGMClassMenu", "MiniGMModifyMenu", "MiniGMAltMenu", "MiniGMTelePicker", "MiniGMSelfWarn",
}
local H = { allow = false, hooked = {} }
W.GMHide = H

function H.Wanted()
    if H.allow then return false end
    return not (W.InDen and W.InDen())
end

local function hideOne(name)
    local f = _G[name]
    if not f or not f.Hide then return end
    if not H.hooked[name] and f.HookScript then
        H.hooked[name] = true
        f:HookScript("OnShow", function(self) if H.Wanted() then self:Hide() end end)
    end
    if H.Wanted() and f.IsShown and f:IsShown() then f:Hide() end
end

function H.Apply()
    for _, name in ipairs(FRAMES) do hideOne(name) end
end

local ev = CreateFrame("Frame")
ev:RegisterEvent("PLAYER_ENTERING_WORLD")
ev:RegisterEvent("ADDON_LOADED")
ev:SetScript("OnEvent", function() H.Apply() end)

SLASH_WUGMBAR1 = "/gmbar"
SlashCmdList.WUGMBAR = function(msg)
    msg = (msg or ""):lower()
    H.allow = msg ~= "off"
    if H.allow then
        for _, name in ipairs(FRAMES) do
            local f = _G[name]
            if f and f.Show and (name == "ma_minibgframe" or name == "MiniGMFrame") then f:Show() end
        end
        print("|cff66ccffLeyline|r: GM bars shown for this session. /gmbar off hides them.")
    else
        H.Apply()
        print("|cff66ccffLeyline|r: GM bars hidden.")
    end
end
