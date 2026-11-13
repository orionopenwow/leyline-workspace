-- lua5.1 tools/den-ui/bots-scene.lua <addon dir> <out dir> [screen WxH in pixels]
-- The Bots window and its new windows, laid out without the game (tools/den-ui/fakeui.lua) and written as JSON for
-- tools/den-ui/render.cjs:
--   node tools/den-ui/render.cjs <out dir> docs/den-ui 1920x1080 bots-
-- Scenes: Your bots (four bots, one hurt, one low on mana), the Tactics tab, the Party tab with saved parties, a
-- bot's gear window with an upgrade, the Ready? checklist. A fake realm answers as ov_companions.lua does (roster,
-- gear). Nothing here is game data: icons and portraits are drawn as named squares.
local dir, out, px = arg[1], arg[2], arg[3] or "1920x1080"
local here = (arg[0] or ""):match("^(.*)[/\\]") or "."
local F = dofile(here .. "/fakeui.lua")
F.realm = "Wrath Unleashed"   -- not Your Den: no Den editor under the windows
local pw, ph = px:match("^(%d+)x(%d+)$")
pw, ph = tonumber(pw), tonumber(ph)
local k = math.floor(math.max(0.64, math.min(1.0, 768 / ph)) * 100 + 1e-6) / 100
local uh = 768 / k
F.setup(uh * pw / ph, uh)

-- four bots in the party: name, class id, role, spec, hp %, mana %
local BOTS = {
    { "Brannoc", 1, "tank", "prot pve", 0.92, 1 }, { "Selwyn", 5, "healer", "holy pve", 0.85, 0.42 },
    { "Ivara", 8, "dps", "fire pve", 0.3, 0.9 }, { "Tamsin", 4, "dps", "combat pve", 1, 1 },
}
local units = {}
for i, b in ipairs(BOTS) do units["party" .. i] = b end
function UnitName(u) if u == "player" then return "Wren" end; local b = units[u]; return b and b[1] or nil end
function UnitHealth(u) local b = units[u]; return b and math.floor(b[5] * 10000) or 0 end
function UnitHealthMax(u) return units[u] and 10000 or 0 end
function UnitPower(u) local b = units[u]; return b and math.floor(b[6] * 8000) or 0 end
function UnitPowerMax(u) return units[u] and 8000 or 0 end
function UnitPowerType(u) local b = units[u]; if b and (b[2] == 1 or b[2] == 4) then return 1, "RAGE" end; return 0, "MANA" end
function UnitIsDeadOrGhost() return false end
function UnitLevel() return 80 end
function GetNumPartyMembers() return 4 end
function IsPartyLeader() return true end
function SetPortraitTexture(tex, unit) tex:SetTexture("Portrait\\" .. (UnitName(unit) or unit)) end
function GetItemIcon(id) return "Interface\\Icons\\Item" .. id end
function GetLFGRoles() return true, false, false, true end
function GetInventoryItemDurability(slot) if slot == 8 then return 21, 100 end; return 80, 100 end
RAID_CLASS_COLORS = { WARRIOR = { r = 0.78, g = 0.61, b = 0.43 }, PRIEST = { r = 1, g = 1, b = 1 },
    MAGE = { r = 0.41, g = 0.8, b = 0.94 }, ROGUE = { r = 1, g = 0.96, b = 0.41 } }
PowerBarColor = { MANA = { r = 0, g = 0, b = 1 }, RAGE = { r = 1, g = 0, b = 0 } }

local function roster()
    local lines = { "H\t4\t4\t0\tarrow\t1\t0\ton\t1\t1\t1\t1\t0\t0" }
    for _, b in ipairs(BOTS) do
        lines[#lines + 1] = table.concat({ "B", b[1], b[2], b[3], "bot", b[4], 213, "follow", 80 }, "\t")
    end
    lines[#lines + 1] = "S\t1\tarms pve=dps,fury pve=dps,prot pve=tank"
    return table.concat(lines, "\n")
end
local NAMES = { [0] = "Helm of the Forgotten Vale", [1] = "Choker of Frost", [2] = "Spaulders of the Watch", [4] = "Chestguard of Ice",
    [5] = "Girdle of Bones", [6] = "Legplates of Valor", [7] = "Sabatons of the Storm", [8] = "Bracers of the North",
    [9] = "Gauntlets of Will", [10] = "Band of Stone", [11] = "Signet of Dawn", [12] = "Talisman of Iron", [13] = "Rune of Sorrow",
    [14] = "Cloak of the Fjord", [15] = "Axe of the Tundra", [16] = "Bulwark of Ice", [17] = "Throwing Axe" }
local function gear(name)
    local lines = { "G\t" .. name .. "\t1\t80\t213\tbot" }
    for slot = 0, 17 do
        if NAMES[slot] then
            lines[#lines + 1] = table.concat({ "I", slot, 40000 + slot, slot == 7 and 200 or 213, slot == 15 and 4 or 3, NAMES[slot] }, "\t")
        end
    end
    lines[#lines + 1] = "U\t7\t50007\t226\t4\tGreaves of the Pale Sun\t200"
    lines[#lines + 1] = "U\t11\t50011\t219\t3\tLoop of the Ancients\t213"
    return table.concat(lines, "\n")
end
F.server = function(msg)
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= "wucomp" then return end
    if name == "roster" then F.toClient("wucomp\troster\t1/1\t" .. roster())
    elseif name == "inspect" then F.toClient("wucomp\tgear\t1/1\t" .. gear(data))
    end
end

F.load(dir)
local C = WrathUnleashedUI.Companions
WrathUnleashedUIDB = { botsTipsSeen = true, botsKeyOffered = true }
WrathUnleashedUIChar = {}
F.fire("PLAYER_ENTERING_WORLD")
F.toClient("wucomp\troster\t1/1\t" .. roster())
-- The game draws UIPanelButtonTemplate (the red panel button) and UICheckButtonTemplate itself; fakeui has neither,
-- so for the picture each gets a stand-in: a dark red box with its label, a small box with a tick when checked.
local function dressTemplates()
    for _, f in ipairs(F.created) do
        if f.template == "UIPanelButtonTemplate" then
            if not f.standIn then
                f.standIn = f:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
                f.standIn:SetPoint("CENTER", f, "CENTER", 0, 0)
            end
            f.backdrop = { bgFile = "Interface\\Buttons\\UI-Panel-Button-Up", edgeFile = "Interface\\Buttons\\UI-Panel-Button-Up", edgeSize = 1 }
            local on = f.enabled ~= false
            f.bdcolor = on and { 0.45, 0.05, 0.04, 1 } or { 0.25, 0.22, 0.22, 1 }
            f.bdborder = (f.locked or f.gameSelected) and { 1, 0.82, 0, 1 } or { 0.7, 0.55, 0.2, 1 }
            f.standIn:SetText(f.text or "")
            f.standIn:SetTextColor(on and 1 or 0.55, on and 0.82 or 0.55, on and 0 or 0.55, 1)
        elseif f.template == "UICheckButtonTemplate" then
            if not f.standIn then
                f.standIn = f:CreateFontString(nil, "OVERLAY", "GameFontNormal")
                f.standIn:SetPoint("CENTER", f, "CENTER", 0, 0)
            end
            f.backdrop = { bgFile = "Interface\\Buttons\\UI-CheckBox-Up", edgeFile = "Interface\\Buttons\\UI-CheckBox-Up", edgeSize = 1 }
            f.bdcolor, f.bdborder = { 0.1, 0.1, 0.1, 1 }, { 0.75, 0.75, 0.75, 1 }
            f.standIn:SetText(f.checked and "x" or "")
            f.standIn:SetTextColor(1, 0.82, 0, 1)
        end
    end
end
local function scene(name, title, notes) dressTemplates(); F.dump(out .. "/" .. name .. ".json", title, notes or "") end

C.Toggle()
F.tick(0.6)
C.SetStatus("Regrouping on you.")
scene("1-your-bots", "Bots window: Your bots", "portraits, roles, what each needs (hurt, low on mana), orders with tooltips")
C.ShowTab("tactics")
F.tick(0.3)
scene("2-tactics", "Bots window: Tactics", "how they fight and heal, formation, loot")
C.SetPick(1, "warrior", "prot pve"); C.SetPick(2, "priest", "holy pve"); C.SetPick(3, "mage", "fire pve")
C.SavePreset(1)
C.SetPick(3, "rogue", "combat pve"); C.SavePreset(2)
C.ShowTab("party")
F.tick(0.3)
scene("3-party", "Bots window: Party, with saved parties", "")
C.Toggle()
C.OpenGear("Brannoc")
F.tick(0.3)
scene("4-gear", "Gear and upgrades: one bot", "what it wears, the better items in its bags, Equip")
C.GearWindow():Hide()
C.preparedAt = nil
C.OpenReady()
F.tick(1.1)
scene("5-ready", "Ready? before a dungeon", "roles with your own, alive, close by, level, prepared, mana, repairs, food")
print("bots scenes written to " .. out)
