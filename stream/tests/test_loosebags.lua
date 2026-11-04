-- texlua stream/tests/test_loosebags.lua [addon dir]   (or lua5.1)
-- Loose Bags (Eric R, MIT), ported to 3.3.5a: every file in .toc order on a fake 3.3.5a API. Covers: loads clean
-- and in Lua 5.1 syntax, the first-login choice (default Loose Bags, X keeps it, Standard switches), B and the bag
-- buttons open it, items from fake bags show on the client's own item buttons (holder id = bag, button id = slot),
-- a drag out and an Alt-click pick up the right bag/slot, a right-click uses it once through the client's own
-- click code, a BAG_UPDATE burst is one pass, /lbags off gives the standard bags back (and Forever UI's hook on
-- them runs), /lbags on, reset and help, and no brand words where players read. The Forever UI redesign: the window
-- fits its items and opens bottom right above the bag bar, the money line is the window's own (inside, anchored to it,
-- all three coins), free slots, Clean Up's spot and tidy grid, search veils by name or kind, the bag slots row, the
-- settings drawer, grip size and Fit to items, the old-pitch migration, and Forever UI's own functions drawing it
-- (a fake ForeverUI), with a plain fallback when they fail.
local dir = arg and arg[1] or "stream/fui-web/Interface/AddOns/LooseBags"

local pass, fail = 0, 0
local function check(name, ok)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name) end
end
unpack = unpack or table.unpack

-- ---------------------------------------------------------------- fake widgets (as test_wuui_collections.lua)
local frames, created = {}, {}
local Frame = {}
Frame.__index = function(self, k)
    local v = rawget(Frame, k)
    if v ~= nil then return v end
    if type(k) == "string" and k:match("^%u") then return function() end end -- other widget methods: no-ops
    return nil
end
function Frame:SetScript(n, fn) self.scripts[n] = fn end
function Frame:GetScript(n) return self.scripts[n] end
function Frame:HookScript(n, fn)
    local old = self.scripts[n]
    self.scripts[n] = function(...) if old then old(...) end; return fn(...) end
end
function Frame:Fire(n, ...) local fn = self.scripts[n]; if fn then return fn(self, ...) end end
function Frame:Show()
    local was = self:IsVisible(); self.shown = true
    if not was and self:IsVisible() then self:Fire("OnShow") end
end
function Frame:Hide()
    local was = self:IsVisible(); self.shown = false
    if was then self:Fire("OnHide") end
end
function Frame:IsShown() return self.shown end
function Frame:IsVisible()
    if not self.shown then return false end
    return self.parent == nil or self.parent.top or self.parent:IsVisible()
end
function Frame:SetText(t) self.text = t end
function Frame:GetText() return self.text end
function Frame:SetTexture(t) self.texture = t end
function Frame:SetNormalTexture(t)
    self.normal = t; self.normalTex = self.normalTex or self:CreateTexture(); self.normalTex.texture = t
end
function Frame:GetNormalTexture() return self.normalTex end
function Frame:SetVertexColor(r, g, b) self.color = { r, g, b } end
function Frame:RegisterEvent(e) self.events[e] = true end
function Frame:UnregisterEvent(e) self.events[e] = nil end
function Frame:GetName() return self.name end
function Frame:GetParent() return self.parent end
function Frame:SetParent(p) self.parent = p end
function Frame:SetID(i) self.id = i end
function Frame:GetID() return self.id or 0 end
function Frame:SetSize(w, h) self.w, self.h = w, h end
function Frame:SetWidth(w) self.w = w end
function Frame:SetHeight(h) self.h = h end
function Frame:GetWidth() return self.w or 0 end
function Frame:GetHeight() return self.h or 0 end
function Frame:SetFrameLevel(l) self.level = l end
function Frame:GetFrameLevel() return self.level or 1 end
function Frame:GetEffectiveScale() return 1 end
function Frame:SetPoint(...) self.points = self.points or {}; self.points[#self.points + 1] = { ... } end
function Frame:ClearAllPoints() self.points = {} end
function Frame:GetPoint(i) local p = self.points and self.points[i or 1]; if p then return unpack(p) end end
function Frame:GetLeft() return self.rect and self.rect.l end
function Frame:GetRight() return self.rect and self.rect.r end
function Frame:GetTop() return self.rect and self.rect.t end
function Frame:GetBottom() return self.rect and self.rect.b end
function Frame:RegisterForClicks(...) self.clicks = { ... } end
function Frame:LockHighlight() self.lit = true end
local function new(kind, name, parent)
    local f = setmetatable({ kind = kind, name = name, parent = parent, scripts = {}, events = {}, shown = true }, Frame)
    if name then _G[name] = f end
    created[#created + 1] = f
    return f
end
function Frame:CreateFontString(name) return new("FontString", name, self) end
function Frame:CreateTexture(name) return new("Texture", name, self) end

-- ---------------------------------------------------------------- fake 3.3.5a API
local log = { pickup = {}, use = {}, stock = {}, sounds = {} }
local templates = {}
function CreateFrame(kind, name, parent, template)
    if template and not templates[template] then error("no template " .. template) end
    local f = new(kind, name, parent); frames[#frames + 1] = f
    if templates[template] then templates[template](f, name) end
    return f
end
templates.UIPanelButtonTemplate = function() end
templates.UICheckButtonTemplate = function(f) f.SetChecked = function(self, v) self.checked = v end end
templates.UIPanelCloseButton = function() end
templates.InputBoxTemplate = function() end
templates.SmallMoneyFrameTemplate = function(f, name)
    for _, part in ipairs({ "GoldButton", "SilverButton", "CopperButton" }) do new("Button", name .. part, f) end
end
-- The client's bag item button: its own OnClick uses the item in (parent id, own id).
templates.ContainerFrameItemButtonTemplate = function(f, name)
    f:CreateTexture(name .. "IconTexture"); f:CreateFontString(name .. "Count")
    f:CreateTexture(name .. "IconQuestTexture"); new("Cooldown", name .. "Cooldown", f)
    f.scripts.OnClick = function(self, button)
        if button == "RightButton" then UseContainerItem(self:GetParent():GetID(), self:GetID())
        else PickupContainerItem(self:GetParent():GetID(), self:GetID()) end
    end
    f.scripts.OnEnter = function(self) f.tipped = { self:GetParent():GetID(), self:GetID() } end
end

UIParent = new("Frame", "UIParent"); UIParent.top = true; UIParent.w, UIParent.h = 1920, 1080
UISpecialFrames = {}
GameFontNormalSmall, GameFontHighlightSmall, GameFontDisableSmall = {}, {}, {}
SlashCmdList = {}
local said = {}
DEFAULT_CHAT_FRAME = { AddMessage = function(_, m) said[#said + 1] = m end }
NUM_BAG_SLOTS, NUM_BAG_FRAMES, NUM_CONTAINER_FRAMES = 4, 4, 13
TEXTURE_ITEM_QUEST_BANG, TEXTURE_ITEM_QUEST_BORDER = "bang", "border"
local player = "Orionne"
function UnitName() return player end
function GetRealmName() return "Wrath Unleashed" end
local combat = false
function InCombatLockdown() return combat end
local mods = {}
function IsShiftKeyDown() return mods.shift end
function IsAltKeyDown() return mods.alt end
function IsControlKeyDown() return mods.ctrl end
function IsModifiedClick(what)
    if what == "SPLITSTACK" then return mods.shift end
    if what == "DRESSUP" then return mods.ctrl end
    return mods.shift or mods.alt or mods.ctrl
end
function IsMouseButtonDown() return false end
local cursor = { x = 0, y = 0, item = nil }
function GetCursorPosition() return cursor.x, cursor.y end
function CursorHasItem() return cursor.item ~= nil end
function GetCursorInfo() if cursor.item then return "item", cursor.item, "|Hitem:" .. cursor.item .. ":0|h" end end
function PlaySound(s) log.sounds[#log.sounds + 1] = s end
function GetItemQualityColor(q) return 1, 0.5, q / 10 end
ITEM_QUALITY_COLORS = { [2] = { r = 0.1, g = 1, b = 0 }, [3] = { r = 0, g = 0.4, b = 1 }, [4] = { r = 0.6, g = 0.2, b = 0.9 } }
function hooksecurefunc(name, fn)
    local old = _G[name]
    _G[name] = function(...) local r = { old(...) }; fn(...); return unpack(r) end
end
function SetItemButtonTexture(b, t) _G[b:GetName() .. "IconTexture"]:SetTexture(t) end
function SetItemButtonCount(b, c) _G[b:GetName() .. "Count"]:SetText(c) end
function SetItemButtonDesaturated(b, l) b.desat = l end
function GetAuctionItemClasses()
    return "Weapon", "Armor", "Container", "Consumable", "Glyph", "Trade Goods", "Projectile", "Quiver", "Recipe", "Gem", "Miscellaneous", "Quest"
end

-- The bags: backpack 16 slots, bag 1 with 10, the rest empty.
local items = {
    [6948] = { "Hearthstone", 1, "Miscellaneous", "Interface\\Icons\\INV_Misc_Rune_01" },
    [33447] = { "Runic Healing Potion", 1, "Consumable", "Interface\\Icons\\INV_Alchemy_Elixir_05" },
    [36913] = { "Saronite Bar", 1, "Trade Goods", "Interface\\Icons\\INV_Ingot_Platinum" },
    [40703] = { "Grim Toll", 4, "Armor", "Interface\\Icons\\INV_Jewelry_Talisman_12" },
    [2589] = { "Linen Cloth", 1, "Trade Goods", "Interface\\Icons\\INV_Fabric_Linen_01" },
}
local bags = { [0] = { size = 16 }, [1] = { size = 10 }, [2] = { size = 0 }, [3] = { size = 0 }, [4] = { size = 0 } }
local function put(bag, slot, id, count) bags[bag][slot] = { id = id, count = count or 1 } end
put(0, 1, 6948); put(0, 2, 33447, 12); put(0, 5, 36913, 20); put(1, 3, 40703); put(1, 7, 2589, 5)
local function link(id) return "|cffffffff|Hitem:" .. id .. ":0:0:0:0:0:0:0:80|h[" .. items[id][1] .. "]|h|r" end
function GetContainerNumSlots(bag) return bags[bag] and bags[bag].size or 0 end
function GetContainerItemInfo(bag, slot)
    local it = bags[bag] and bags[bag][slot]
    if not it then return nil end
    return items[it.id][4], it.count, it.locked, items[it.id][2], nil, nil, link(it.id)
end
function GetContainerItemLink(bag, slot) local it = bags[bag] and bags[bag][slot]; return it and link(it.id) end
function GetContainerItemQuestInfo() return nil, nil, nil end
function GetContainerNumFreeSlots(bag)
    local b = bags[bag]; if not b then return 0, 0 end
    local free = 0
    for slot = 1, b.size do if not b[slot] then free = free + 1 end end
    return free, 0
end
local money = 1234567
function GetMoney() return money end
function ContainerIDToInventoryID(bag) return 19 + bag end
function GetInventoryItemTexture(_, inv) if inv == 20 then return "Interface\\Icons\\INV_Misc_Bag_07" end end
function PutItemInBag(inv) log.putbag = inv end
function PickupBagFromSlot(inv) log.pickbag = inv end
MultiBarRight = new("Frame", "MultiBarRight", UIParent); MultiBarRight.shown = false; MultiBarRight.w = 38
function GetItemInfo(l)
    local id = tonumber(tostring(l):match("item:(%d+)")); local it = items[id]
    if not it then return nil end
    return it[1], link(id), it[2], 80, 1, it[3], "", 20, "", it[4], 1
end
function PickupContainerItem(bag, slot) log.pickup[#log.pickup + 1] = { bag, slot } end
function UseContainerItem(bag, slot) log.use[#log.use + 1] = { bag, slot } end
function SplitContainerItem() end

-- The client's own bag toggles and bag windows; Forever UI only hooks ContainerFrame_GenerateFrame.
for i = 1, NUM_CONTAINER_FRAMES do local f = new("Frame", "ContainerFrame" .. i, UIParent); f.shown = false end
function ContainerFrame_GenerateFrame(frame, size, id) frame:SetID(id); frame:Show() end
local function stockOpen(what) log.stock[#log.stock + 1] = what; ContainerFrame_GenerateFrame(ContainerFrame1, 16, 0) end
function ToggleBackpack() stockOpen("ToggleBackpack") end
function ToggleBag(id) stockOpen("ToggleBag" .. id) end
function OpenBag(id) stockOpen("OpenBag" .. id) end
function OpenBackpack() stockOpen("OpenBackpack") end
function OpenAllBags(force) stockOpen("OpenAllBags") end
function CloseAllBags() log.stock[#log.stock + 1] = "CloseAllBags"; ContainerFrame1:Hide() end
MainMenuBarBackpackButton = new("CheckButton", "MainMenuBarBackpackButton", UIParent)
function MainMenuBarBackpackButton:SetChecked(on) self.checked = on end
local fuiSkinned = 0
ForeverUI = {}
hooksecurefunc("ContainerFrame_GenerateFrame", function() fuiSkinned = fuiSkinned + 1 end)

-- ---------------------------------------------------------------- Lua 5.1 only (the game's Lua)
local function code(text)
    text = text:gsub("%-%-%[(=*)%[.-%]%1%]", " "):gsub("%-%-[^\n]*", " ")
    text = text:gsub("%[(=*)%[.-%]%1%]", '""'):gsub('"[^"\n]*"', '""'):gsub("'[^'\n]*'", "''")
    return text
end
local BANNED = { "World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard" }
local toc = assert(io.open(dir .. "/LooseBags.toc")):read("*a")
local files = {}
for line in toc:gmatch("[^\r\n]+") do if not line:match("^#") and line:match("%S") then files[#files + 1] = line end end
check("toc: interface 30300, the credit kept", toc:match("## Interface: 30300") and toc:match("## Author: Eric R")
    and toc:match("## Title: Loose Bags") and toc:match("SavedVariables: LooseBagsDB"))
local syntaxOk, brandOk = true, true
for _, f in ipairs(files) do
    local text = assert(io.open(dir .. "/" .. f)):read("*a")
    local c = code(text)
    for _, pat in ipairs({ "%f[%w]goto%f[%W]", "::[%a_]+::", "//", "[^~]~[^=]", "[^<]<<", ">>", "&", "|", "\\z",
                           "utf8%.", "math%.type", "table%.unpack", "table%.pack", "table%.move", "math%.tointeger" }) do
        if c:find(pat) then syntaxOk = false; print("  5.1: " .. f .. " has " .. pat) end
    end
    for _, w in ipairs(BANNED) do
        local hit = (w == "WoW") and text:find("%f[%a]WoW%f[%A]") or (w ~= "WoW" and text:find(w, 1, true))
        if hit then brandOk = false; print("  brand: " .. f .. " says " .. w) end
    end
end
check("every file is Lua 5.1 (no goto, //, bit operators, \\z, utf8, 5.2+ library calls)", syntaxOk and #files == 8)
check("no brand words in any file or the .toc", brandOk and not toc:find("Blizzard") and not toc:find("%f[%a]WoW%f[%A]"))

-- ---------------------------------------------------------------- load
for _, f in ipairs(files) do
    local chunk = assert(loadfile(dir .. "/" .. f))
    local ok, err = pcall(chunk)
    check("loads " .. f, ok); if not ok then print("  " .. tostring(err)) end
end
local Logic = assert(LooseBagsLogic)
local LB = assert(Logic.LB)
check("/lbags registered", SLASH_LOOSEBAGS1 == "/lbags" and SlashCmdList.LOOSEBAGS)
local function event(e, ...) for _, f in ipairs(frames) do if f.events[e] then f:Fire("OnEvent", e, ...) end end end
local function tick(n)
    for _ = 1, n or 1 do
        for _, f in ipairs(frames) do if f.scripts.OnUpdate and f:IsVisible() then f:Fire("OnUpdate", 0.02) end end
    end
end
local function slash(s) SlashCmdList.LOOSEBAGS(s) end

event("ADDON_LOADED", "LooseBags")
event("PLAYER_LOGIN")
local frame = assert(LooseBagsFrame, "LooseBagsFrame")
check("built hidden at login, Esc closes it", not frame:IsShown() and UISpecialFrames[1] == "LooseBagsFrame")
check("on by default before any choice", LB.Enabled())

-- ---------------------------------------------------------------- the first-login choice
event("PLAYER_ENTERING_WORLD")
local choice = LooseBagsChoice
check("first login: the choice window shows", choice and choice:IsShown())
check("choice: loose backpack (lit, the default) or the standard bags",
    LooseBagsChoiceLoose.text == "One loose backpack" and LooseBagsChoiceLoose.lit
    and LooseBagsChoiceStandard.text == "The standard bags" and LooseBagsChoiceLoose.note.text == "Items stay where you leave them.")
choice:Fire("OnHide")   -- the whole interface hid (Alt-Z, the opening scene): still asking
check("the interface hiding is not a choice", LooseBagsDB.chars["Orionne-Wrath Unleashed"].chosen ~= true)
LooseBagsChoiceClose:Fire("OnClick")
check("closing it keeps Loose Bags and remembers the choice", not choice:IsShown() and LB.Enabled()
    and LooseBagsDB.chars["Orionne-Wrath Unleashed"].chosen == true and LooseBagsDB.chars["Orionne-Wrath Unleashed"].bags == "loose")
event("PLAYER_ENTERING_WORLD")
check("...and does not ask again", not choice:IsShown())
tick(2)

-- ---------------------------------------------------------------- B opens it
log.stock = {}
OpenAllBags()
check("B (OpenAllBags) opens Loose Bags, not the standard bags", frame:IsShown() and #log.stock == 0 and not ContainerFrame1:IsShown())
check("open sound", log.sounds[#log.sounds] == "igBackPackOpen")
local function shownButtons()
    local out = {}
    for _, b in ipairs(LB.buttons) do if b:IsShown() then out[#out + 1] = b end end
    return out
end
local shown = shownButtons()
check("the five items from the fake bags show", #shown == 5)
local byID = {}
for _, b in ipairs(shown) do byID[b.place.itemID] = b end
local hs, pot, toll = byID[6948], byID[33447], byID[40703]
check("item buttons are the client's own, on a holder whose id is the bag, button id = slot",
    toll and toll:GetParent():GetID() == 1 and toll:GetID() == 3 and toll.clientClick and toll.name:match("^LooseBagsItem%d+$")
    and pot:GetParent():GetID() == 0 and pot:GetID() == 2)
check("icon and stack count drawn", _G[pot.name .. "IconTexture"].texture == "Interface\\Icons\\INV_Alchemy_Elixir_05"
    and _G[pot.name .. "Count"].text == 12)
check("quality border on the epic, none on the common", toll.QualityBorder.shown and not hs.QualityBorder.shown)
check("categories from the item class names", toll.place.classID == 4 and pot.place.classID == 0 and byID[36913].place.classID == 7)
-- (redesign) the first open tidies every item into a window fitted to them: no size remembered, nothing held
local anyHeld = false
for _, p in ipairs(LB.Char().places) do if p.held then anyHeld = true end end
check("first open: every item tidied in, the window fitted to them (no size remembered)",
    LB.Char().width == nil and LB.Char().firstRun == true and not anyHeld)
check("...compact: the default columns wide, as tall as the items plus one free row",
    frame.w == Logic.BAG_WIDTH + Logic.SIDE_PAD and frame.h < 260 and frame.h >= Logic.CHROME_MIN_H)
local p1 = frame.points[1]
check("it opens bottom right above the bag bar, as the standard bags",
    p1 and p1[1] == "BOTTOMRIGHT" and p1[2] == UIParent and p1[3] == "BOTTOMRIGHT" and p1[4] == -10 and p1[5] == 85)
local purse = frame.Money
local function anchoredTo(region, host)
    for _, pt in ipairs(region.points or {}) do if pt[2] ~= host then return false end end
    return #(region.points or {}) > 0
end
local coinsInside = true
for _, which in ipairs({ "gold", "silver", "copper" }) do
    local c = purse.coins[which]
    if c.icon.parent ~= purse or c.text.parent ~= purse then coinsInside = false end
    for _, r in ipairs({ c.icon, c.text }) do
        for _, pt in ipairs(r.points or {}) do if pt[2] == UIParent or pt[2] == nil then coinsInside = false end end
    end
end
check("money line: a child of the window, anchored to the window's bottom on both sides, coins inside it",
    purse.parent == frame and anchoredTo(purse, frame) and purse.points[1][1] == "BOTTOMLEFT" and purse.points[2][1] == "BOTTOMRIGHT"
    and coinsInside)
check("money line shows gold, silver and copper (123g 45s 67c)", purse.coins.gold.text.text == "123"
    and purse.coins.silver.text.text == 45 and purse.coins.copper.text.text == 67)
money = 50
event("PLAYER_MONEY")
check("...and follows the money (0g 0s 50c, every coin still shown)", purse.coins.gold.text.text == "0"
    and purse.coins.silver.text.text == 0 and purse.coins.copper.text.text == 50 and purse.coins.gold.icon.shown)
money = 1234567
check("free slots counter in the window (21 of 26 general slots free)", frame.FreeText.text == "21 free"
    and frame.FreeText.parent == purse)
check("Clean Up in Forever UI's spot (top right, under the close button)", frame.SortButton.points[1][1] == "TOPRIGHT"
    and frame.SortButton.points[1][2] == frame and frame.SortButton.points[1][4] == -9 and frame.SortButton.points[1][5] == -40)
toll:Fire("OnEnter")
check("tooltip through the client's own code reads bag 1 slot 3", toll.tipped and toll.tipped[1] == 1 and toll.tipped[2] == 3)
OpenAllBags()
check("B again closes it", not frame:IsShown())
ToggleBackpack()
check("the backpack button opens it", frame:IsShown() and #log.stock == 0)
MainMenuBarBackpackButton.checked = false
tick()
check("...and the backpack button stays lit while it is open", MainMenuBarBackpackButton.checked == true)
ToggleBackpack()
ToggleBag(2)
check("a bag slot button opens it", frame:IsShown() and #log.stock == 0)
ToggleBag(-2)
check("the keyring stays the standard one", log.stock[1] == "ToggleBag-2")
log.stock = {}
OpenBackpack()
check("a vendor asking for the bags leaves it as it is (as the original)", frame:IsShown() and #log.stock == 0)

-- ---------------------------------------------------------------- drag out picks up; Alt-click; right-click
frame.rect = { l = 100, r = 520, t = 800, b = 400 }
frame.Content.rect = { l = 110, r = 510, t = 736, b = 430 }
frame.Content.w, frame.Content.h = 400, 306
log.pickup = {}
cursor.x, cursor.y = 150, 700
toll:Fire("OnMouseDown", "LeftButton")
cursor.x, cursor.y = 900, 200   -- off the bag
tick()
toll:Fire("OnDragStart")
toll:Fire("OnDragStop")
check("dragging one icon out of the bag picks it up from bag 1 slot 3",
    #log.pickup == 1 and log.pickup[1][1] == 1 and log.pickup[1][2] == 3)
check("...and its saved spot stays", LB.drag == nil and toll.place.x and LB.gesture.mode == "idle")

log.pickup = {}
mods.alt = true
cursor.x, cursor.y = 150, 700
pot:Fire("OnMouseDown", "LeftButton"); pot:Fire("OnMouseUp", "LeftButton")
mods.alt = nil
check("Alt-click picks it up (bag 0 slot 2)", #log.pickup == 1 and log.pickup[1][1] == 0 and log.pickup[1][2] == 2)

log.pickup = {}
pot:Fire("OnMouseDown", "LeftButton"); pot:Fire("OnMouseUp", "LeftButton")
check("a plain click does nothing (as the original)", #log.pickup == 0)

log.use = {}
pot:Fire("OnMouseDown", "RightButton"); pot:Fire("OnMouseUp", "RightButton"); pot:Fire("OnClick", "RightButton")
check("right-click uses it once, through the client's own click (bag 0 slot 2)",
    #log.use == 1 and log.use[1][1] == 0 and log.use[1][2] == 2)

-- a move inside the bag is saved; it never calls the bag API
log.pickup = {}
local oldX = byID[36913].place.x
cursor.x, cursor.y = 110 + byID[36913].place.x, 736 - byID[36913].place.y
byID[36913]:Fire("OnMouseDown", "LeftButton")
cursor.x = cursor.x + 10
tick()
cursor.x, cursor.y = 110 + 360, 736 - 60
tick()
byID[36913]:Fire("OnDragStop")
check("a drag inside the bag moves the icon and picks nothing up", #log.pickup == 0 and byID[36913].place.x ~= oldX)

-- an item on the cursor dropped on empty space goes into the first empty slot
log.pickup = {}
cursor.item = 2589
frame.Content:Fire("OnMouseUp", "LeftButton")
cursor.item = nil
check("dropping on empty space puts it in the first empty slot (bag 0 slot 3)",
    #log.pickup == 1 and log.pickup[1][1] == 0 and log.pickup[1][2] == 3)

-- ---------------------------------------------------------------- new loot: one pass per burst
put(0, 9, 2589, 3)
event("BAG_UPDATE", 0); event("BAG_UPDATE", 0); event("BAG_UPDATE", 1)
check("BAG_UPDATE waits for the next frame", #shownButtons() == 5)
tick()
local held = 0
for _, b in ipairs(shownButtons()) do if b.place.held then held = held + 1 end end
check("then the new item shows, on the new-loot strip", #shownButtons() == 6 and held == 1)
local tallWithStrip = frame.h

-- ---------------------------------------------------------------- search, bag slots, settings, size (the redesign)
local search = LooseBagsSearchBox
local function veiled(b) return b.SearchVeil.shown == true end
search:SetText("trade"); search:Fire("OnTextChanged", true)
check("search matches the kind too: trade goods stay lit, the rest is veiled in black",
    not veiled(byID[36913]) and veiled(pot) and veiled(toll) and veiled(hs))
check("...and the clear button shows", frame.SearchClear.shown)
search:SetText(""); search:Fire("OnTextChanged", true)
check("clearing the search lifts every veil", not veiled(byID[36913]) and not veiled(pot) and not veiled(toll))

local h0 = frame.h
LooseBagsBagSlotsButton:Fire("OnClick")
check("the bag slots row opens and the window grows by its height (the items keep their room)",
    LooseBagsBagRow.shown and LB.Char().bagRow and frame.h == h0 + Logic.BAG_ROW)
check("...one slot per bag, the equipped bag's icon, an empty slot's art", LooseBagsBagSlot1.parts.icon.texture == "Interface\\Icons\\INV_Misc_Bag_07"
    and LooseBagsBagSlot2.parts.icon.texture:find("UI%-PaperDoll%-Slot%-Bag") ~= nil and LooseBagsBagSlot0 ~= nil)
LooseBagsBagSlot1:Fire("OnEnter")
check("pointing at a bag lights only its items", not veiled(toll) and veiled(pot) and veiled(hs))
LooseBagsBagSlot1:Fire("OnLeave")
check("...until the pointer leaves", not veiled(pot))
cursor.item = 9999
LooseBagsBagSlot1:Fire("OnReceiveDrag")
cursor.item = nil
check("a bag dropped on a slot is equipped there", log.putbag == 20)
LooseBagsBagSlotsButton:Fire("OnClick")
check("...and the row closes again", not LooseBagsBagRow.shown and frame.h == h0)

check("settings start tucked away", not LooseBagsSettings.shown)
LooseBagsSettingsButton:Fire("OnClick")
check("the cog opens the settings drawer beside the window", LooseBagsSettings.shown and LooseBagsSettings.parent == frame)
LooseBagsToggleuseZones:Fire("OnClick")
check("a tick box flips its setting", LB.Char().useZones == true and LooseBagsToggleuseZones.checked == true)
LooseBagsToggleuseZones:Fire("OnClick")
LooseBagsSettingsButton:Fire("OnClick")
check("...and the cog closes it", not LooseBagsSettings.shown and LB.Char().useZones == false)

local autoW = frame.w
frame.w, frame.h = 500, 420
LooseBagsGrip:Fire("OnMouseDown", "LeftButton")
LooseBagsGrip:Fire("OnMouseUp", "LeftButton")
check("the grip chooses a size, remembered", LB.Char().width == 500 and LB.Char().height == 420 and frame.w == 500)
LooseBagsFitItems:Fire("OnClick")
-- (moved below)
check("Fit to items forgets it: the window fits its items again", LB.Char().width == nil and frame.w == autoW
    and frame.h == tallWithStrip)

LB.Char().moved = false   -- (the grip above saved the corner it dragged)
LB.Hide(); MultiBarRight.shown = true; LB.Show()
p1 = frame.points[1]
check("an unmoved bag opens left of the shown right-hand bars", p1[1] == "BOTTOMRIGHT" and p1[4] == -48 and p1[5] == 85)
MultiBarRight.shown = false
frame.rect = { l = 400, r = 900, t = 600, b = 200 }
frame:Fire("OnDragStop")
check("a moved bag remembers its bottom right corner", LB.Char().moved == true and LB.Char().point == "BOTTOMRIGHT"
    and LB.Char().relPoint == "BOTTOMLEFT" and LB.Char().x == 900 and LB.Char().y == 200)
LB.Hide(); LB.Show()
p1 = frame.points[1]
check("...and opens there next time (it grows up and left from that corner)", p1[1] == "BOTTOMRIGHT" and p1[3] == "BOTTOMLEFT"
    and p1[4] == 900 and p1[5] == 200)
frame.rect = nil

-- ---------------------------------------------------------------- the layout rules behind it (pure Logic)
local old = Logic.NormalizeChar({ places = { { itemID = 1, inst = 1, x = 18.5, y = 18.5 }, { itemID = 2, inst = 1, x = 55.5, y = 18.5 } },
    width = 420, height = 320, point = "CENTER", relPoint = "CENTER", x = 0, y = 120, firstRun = true,
    homes = { armor = { x = 74, y = 37 } } })
check("a layout saved at the old 37 px pitch is scaled to the new one, no overlap, its old size dropped",
    old.places[1].x == 20.5 and old.places[2].x == 61.5 and old.homes.armor.x == 82 and old.width == nil and old.pitch == Logic.ICON
    and old.moved == false and Logic.PlacesOk(old.places, Logic.ICON))
Logic.NormalizeChar(old)
check("...once", old.places[2].x == 61.5)
local movedOld = Logic.NormalizeChar({ point = "TOPLEFT", relPoint = "BOTTOMLEFT", x = 300, y = 700 })
check("an old bag the player had moved stays where it was", movedOld.moved == true and movedOld.x == 300)
local few = {}
for i = 1, 3 do few[i] = { x = 20.5 + (i - 1) * 41, y = 20.5 } end
local many = {}
for i = 1, 40 do many[i] = { x = 20.5 + ((i - 1) % 8) * 41, y = 20.5 + math.floor((i - 1) / 8) * 41 } end
local _, hFew = Logic.AutoSize(few, 0, 1920, 1080)
local wMany, hMany = Logic.AutoSize(many, 0, 1920, 1080)
check("fitted window: small with a few items, taller as items come, one free row below the last",
    hFew == Logic.TOP_BASE + 2 * Logic.ICON + Logic.BOTTOM_PAD and hMany == Logic.TOP_BASE + 6 * Logic.ICON + Logic.BOTTOM_PAD
    and wMany == Logic.BAG_WIDTH + Logic.SIDE_PAD)
local mix, mid = {}, 1
for _, spec in ipairs({ { 11, 0 }, { 2, 4 }, { 3, 2 }, { 25, 7 } }) do
    for _ = 1, spec[1] do mix[#mix + 1] = { itemID = mid, inst = 1, x = 0, y = 0, classID = spec[2], quality = 1 }; mid = mid + 1 end
end
local tidy = Logic.Tidy(mix, nil, Logic.BAG_WIDTH, Logic.BAG_WIDTH)
local onGrid, minY = true, 1e9
for _, p in ipairs(tidy) do
    local col = (p.x - Logic.ICON / 2) / Logic.ICON
    if col ~= math.floor(col) or col < 0 or col >= Logic.AUTO_COLS then onGrid = false end
    if p.y < minY then minY = p.y end
end
local apart = true
for _, a in ipairs(tidy) do for _, b in ipairs(tidy) do
    if a ~= b and Logic.Category(a.classID, a.quality) ~= Logic.Category(b.classID, b.quality)
        and math.abs(a.x - b.x) < Logic.ICON + 1 and math.abs(a.y - b.y) < Logic.ICON + 1 then apart = false end
end end
check("Clean Up: a tidy grid from the top left, every icon on one column grid, categories never touching",
    onGrid and minY == Logic.ICON / 2 and apart and Logic.PlacesOk(tidy, Logic.ICON))
local hundred = {}
for i = 1, 100 do hundred[i] = { itemID = i, inst = 1, x = 0, y = 0, classID = ({ 0, 7, 15, 4, 2 })[i % 5 + 1], quality = 1 } end
local wide, wideW = Logic.AutoTidy(hundred, nil, false, 768)
local _, wideH = Logic.AutoSize(wide, 0, nil, nil)
check("a full bag too tall for the screen at the default width gets more columns, and fits",
    wideW > Logic.BAG_WIDTH and wideH <= 768 - Logic.HOME_BOTTOM and Logic.PlacesOk(wide, Logic.ICON))
local g, sv, c = Logic.MoneyParts(1234567)
check("money parts and grouping", g == 123 and sv == 45 and c == 67 and Logic.Grouped(1234567) == "1,234,567"
    and Logic.Grouped(12) == "12" and select(1, Logic.MoneyParts(nil)) == 0)
check("search matches name, kind or sub-kind", Logic.SearchMatch("lin", "Linen Cloth") and Logic.SearchMatch("cloth", "x", "Trade Goods", "Cloth")
    and not Logic.SearchMatch("ore", "Linen Cloth", "Trade Goods", "Cloth") and Logic.SearchMatch("", nil))

-- ---------------------------------------------------------------- /lbags off and on
log.stock = {}
fuiSkinned = 0
slash("off")
check("/lbags off closes Loose Bags", not frame:IsShown() and not LB.Enabled())
OpenAllBags()
check("...and B opens the standard backpack again", log.stock[1] == "OpenAllBags" and ContainerFrame1:IsShown())
check("...with Forever UI's hook on the standard bag window running", fuiSkinned == 1)
ToggleBackpack()
check("...the backpack button too", log.stock[2] == "ToggleBackpack")
check("/lbags off says how to come back", said[#said]:find("/lbags on", 1, true) ~= nil)
log.stock = {}
slash("on")
check("/lbags on: Loose Bags back, the standard window closed", frame:IsShown() and LB.Enabled() and not ContainerFrame1:IsShown())
ContainerFrame_GenerateFrame(ContainerFrame2, 16, 0)
check("a standard backpack window something else opens while on is closed again", not ContainerFrame2:IsShown())
ContainerFrame_GenerateFrame(ContainerFrame3, 28, 5)
check("...but not the bank's bags", ContainerFrame3:IsShown())
CloseAllBags()
check("CloseAllBags closes it too", not frame:IsShown())

slash("reset")
check("/lbags reset clears the layout", #LB.Char().places == 0 or LB.Char().places[1].x ~= nil)
slash("help")
check("/lbags help opens the help panel", LooseBagsHelp and LooseBagsHelp:IsShown())
LooseBagsHelp:Hide()
slash("")
check("/lbags toggles the bag", frame:IsShown())
slash("")

-- ---------------------------------------------------------------- another character picks the standard bags
player = "Altchar"
event("PLAYER_ENTERING_WORLD")
check("a new character is asked too", choice:IsShown())
LooseBagsChoiceStandard:Fire("OnClick")
check("choosing the standard bags turns Loose Bags off for that character", not choice:IsShown() and not LB.Enabled()
    and LooseBagsDB.chars["Altchar-Wrath Unleashed"].bags == "standard")
log.stock = {}
OpenAllBags()
check("...and B opens the standard bags", log.stock[1] == "OpenAllBags" and not frame:IsShown())
player = "Orionne"
check("the first character keeps Loose Bags", LB.Enabled())

-- ---------------------------------------------------------------- what players read
local readable = {}
for _, f in ipairs(created) do
    if type(f.text) == "string" then readable[#readable + 1] = f.text end
end
for _, row in ipairs(Logic.HELP) do readable[#readable + 1] = row.gesture .. " " .. row.detail end
for _, m in ipairs(said) do readable[#readable + 1] = m end
local clean = true
for _, t in ipairs(readable) do
    for _, w in ipairs(BANNED) do
        if (w == "WoW" and t:find("%f[%a]WoW%f[%A]")) or (w ~= "WoW" and t:find(w, 1, true)) then clean = false; print("  reads: " .. t) end
    end
    if t:find("3%.3%.5") or t:find("1%.12") then clean = false; print("  version in prose: " .. t) end
end
check("nothing a player reads names the game, its maker or a client version", clean and #readable > 30)

-- ---------------------------------------------------------------- with Forever UI loaded: its own functions draw it
local fuiLog = {}
local function fakeForeverUI(throw)
    local function rec(name, fn)
        return function(...)
            fuiLog[#fuiLog + 1] = { name, ... }
            if throw then error("not in this client") end
            if fn then return fn(...) end
        end
    end
    ForeverUI = {
        SetPanelArt = rec("SetPanelArt", function(f) f.foreverPanel = {}; return f.foreverPanel end),
        UpdatePanelCorners = rec("UpdatePanelCorners"),
        SetAtlas = rec("SetAtlas", function(t, name) t.atlas = name; return true end),
        AtlasEntry = function(name) return { "file-" .. name, 0, 1, 0, 1, 32, 32 } end,
        Templates = {
            CloseButton = rec("CloseButton", function(b) return b end),
            DialogFrame = rec("DialogFrame"),
            ThreeSliceButton = rec("ThreeSliceButton", function(b) return b end),
            Outline = rec("Outline", function(b) local t = b:CreateTexture(); b.foreverOutline = t; return t end),
            Art = function(name) return { "file-" .. name } end,
            Place = rec("Place", function(t, name) t.placed = name end),
        },
    }
end
local function fuiCalled(name, target)
    for _, c in ipairs(fuiLog) do if c[1] == name and (target == nil or c[2] == target) then return true end end
    return false
end
local function reload()
    LooseBagsLogic = nil
    for _, f in ipairs(files) do assert(loadfile(dir .. "/" .. f))() end
    local L = LooseBagsLogic.LB
    L.frame = nil
    return L
end
fakeForeverUI(false)
local LB3 = reload()
local okF, errF = pcall(function() LB3.OnEvent(nil, "PLAYER_LOGIN"); LB3.Show() end)
if not okF then print("  " .. tostring(errF)) end
local f3 = LB3.frame
check("Forever UI: the window is its bag frame (SetPanelArt), the red X its CloseButton", okF and f3.forever == true
    and fuiCalled("SetPanelArt", f3) and fuiCalled("CloseButton", LooseBagsFrameClose))
check("...Clean Up is its autosort button, the cog and bag buttons its atlases, the purse its coin box",
    LooseBagsCleanUp.normalTex and LooseBagsCleanUp.normalTex.atlas == "bags-button-autosort-up"
    and LooseBagsSettingsButton.normalTex.atlas == "questlog-icon-setting" and LooseBagsBagSlotsButton.normalTex.atlas == "bag-main"
    and fuiCalled("SetAtlas") and (function() for _, c in ipairs(fuiLog) do if c[1] == "SetAtlas" and c[3] == "common-coinbox-left" then return true end end end)())
check("...the search box has its magnifier and clear button", (function()
    local m, x = false, false
    for _, c in ipairs(fuiLog) do if c[1] == "SetAtlas" and c[3] == "common-search-magnifyingglass" then m = true end
        if c[1] == "SetAtlas" and c[3] == "common-search-clearbutton" then x = true end end
    return m and x end)())
local fuiToll
for _, b in ipairs(LB3.buttons) do if b:IsShown() and b.place.itemID == 40703 then fuiToll = b end end
local fuiHs
for _, b in ipairs(LB3.buttons) do if b:IsShown() and b.place.itemID == 6948 then fuiHs = b end end
check("...item slots wear its outline: the epic in its quality colour, the common in its grey",
    fuiToll and fuiToll.lbOutline and fuiToll.lbOutline.color[1] == 0.6 and fuiHs.lbOutline.color[1] == LB3.Skin.FRAME_GRAY
    and fuiCalled("Outline", fuiToll))
check("...settings drawer: its dialog frame, three-slice buttons and tick boxes", fuiCalled("DialogFrame", LooseBagsSettings)
    and fuiCalled("ThreeSliceButton", LooseBagsHelpButton) and LooseBagsToggleautoPlace.normalTex.placed == "checkbox-minimal")
check("...the money line is still the window's own, inside it", f3.Money.parent == f3 and anchoredTo(f3.Money, f3)
    and f3.Money.coins.gold.text.text == "123")

fuiLog = {}
fakeForeverUI(true)
local LB4 = reload()
local okT, errT = pcall(function() LB4.OnEvent(nil, "PLAYER_LOGIN"); LB4.Show() end)
if not okT then print("  " .. tostring(errT)) end
check("a Forever UI whose calls fail: the bag still opens, in the plain look, with its items and money",
    okT and LB4.frame:IsShown() and LB4.frame.forever == false and #LB4.buttons > 0 and #fuiLog > 0
    and LB4.frame.Money.coins.copper.text.text == 67)
ForeverUI = {}

-- ---------------------------------------------------------------- a client without the templates
for k in pairs(templates) do templates[k] = nil end
LooseBagsLogic = nil
for _, f in ipairs(files) do assert(loadfile(dir .. "/" .. f))() end
local LB2 = LooseBagsLogic.LB
LB2.frame = nil
local ok, err = pcall(function()
    LB2.OnEvent(nil, "PLAYER_LOGIN")
    LB2.Show()
end)
check("without the client's templates it still opens, with plain buttons", ok and LB2.frame:IsShown()
    and #LB2.buttons > 0 and not LB2.buttons[1].clientClick)
if not ok then print("  " .. tostring(err)) end
log.use = {}
local b = LB2.buttons[1]
b:Fire("OnMouseDown", "RightButton"); b:Fire("OnMouseUp", "RightButton")
check("...and a right-click there uses the item itself", #log.use == 1 and log.use[1][1] == b.bag and log.use[1][2] == b.slot)

print(("%d passed, %d failed"):format(pass, fail))
if fail > 0 then os.exit(1) end
