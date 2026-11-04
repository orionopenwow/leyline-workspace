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

-- Their minimap buttons too (owner 4 Nov 14:20: "remove azeroth admin icon from minimap too"): any button on the
-- minimap whose name says AzerothAdmin or MiniGM (its own button, or LibDBIcon's LibDBIcon10_AzerothAdmin).
local MINIMAP_PAT = { "azerothadmin", "minigm", "^ma_" }
function H.MinimapButtons()
    local out = {}
    for _, parent in ipairs({ _G.Minimap, _G.MinimapCluster, _G.MinimapBackdrop }) do
        if parent and parent.GetChildren then
            local kids = { pcall(parent.GetChildren, parent) }
            for i = 2, #kids do
                local c = kids[i]
                local n = type(c) == "table" and c.GetName and c:GetName()
                if type(n) == "string" then
                    local l = n:lower()
                    for _, pat in ipairs(MINIMAP_PAT) do
                        if l:find(pat) then out[#out + 1] = n; break end
                    end
                end
            end
        end
    end
    return out
end

function H.Apply()
    for _, name in ipairs(FRAMES) do hideOne(name) end
    for _, name in ipairs(H.MinimapButtons()) do hideOne(name) end
end

local ev = CreateFrame("Frame")
ev:RegisterEvent("PLAYER_ENTERING_WORLD")
ev:RegisterEvent("ADDON_LOADED")
ev:SetScript("OnEvent", function() H.Apply() end)
local later, waited = CreateFrame("Frame"), 0
later:SetScript("OnUpdate", function(self, dt)
    waited = waited + (dt or 0)
    if waited >= 5 then self:SetScript("OnUpdate", nil); H.Apply() end
end)

SLASH_WUGMBAR1 = "/gmbar"
SlashCmdList.WUGMBAR = function(msg)
    msg = (msg or ""):lower()
    H.allow = msg ~= "off"
    if H.allow then
        for _, name in ipairs(FRAMES) do
            local f = _G[name]
            if f and f.Show and (name == "ma_minibgframe" or name == "MiniGMFrame") then f:Show() end
        end
        for _, name in ipairs(H.MinimapButtons()) do local f = _G[name]; if f and f.Show then f:Show() end end
        print("|cff66ccffLeyline|r: GM bars shown for this session. /gmbar off hides them.")
    else
        H.Apply()
        print("|cff66ccffLeyline|r: GM bars hidden.")
    end
end
