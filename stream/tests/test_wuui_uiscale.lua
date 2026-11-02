-- lua5.1 stream/tests/test_wuui_uiscale.lua [WrathUnleashedUI.lua]
-- The UI size (WrathUnleashedUI.lua "UI size"), owner 1 Nov 20:56 on the desktop game: "the UI scale, the client
-- scale to immediately adapt to player screens and not jump all over the place". One size from the real screen,
-- set before the world is drawn and only when it differs (one layout, no second one), again once after a resize,
-- the player's own size kept, the browser client's own pixel mode left alone. Fake 3.3.5a API only.
local file = arg[1] or "stream/fui-web/Interface/AddOns/WrathUnleashedUI/WrathUnleashedUI.lua"
local pass, fail = 0, 0
local function check(name, ok, extra) if ok then pass = pass + 1 else fail = fail + 1 end
  print((ok and "PASS " or "FAIL ") .. name .. (extra ~= nil and ("  " .. tostring(extra)) or "")) end

-- ---- fake game ----------------------------------------------------------------------------------
local frames = {}
local function newFrame()
    local f = { events = {}, scripts = {} }
    function f:RegisterEvent(e) self.events[e] = true end
    function f:UnregisterEvent(e) self.events[e] = nil end
    function f:SetScript(k, fn) self.scripts[k] = fn end
    function f:Hide() end
    function f:Show() end
    frames[#frames + 1] = f
    return f
end
local screen = { w = 1920, h = 1080 }
local cvars, sets, uiParentScaled
local browser = false
local combat = false
local function reset(c)
    frames, sets, uiParentScaled = {}, {}, 0
    cvars = c or { useUiScale = "0", uiScale = "1" }
end
function CreateFrame() return newFrame() end
UIParent = { GetScale = function() return 0.9 end, SetScale = function() uiParentScaled = uiParentScaled + 1 end }
-- UIParent in UI units: 768 / scale tall, the window's shape
function GetScreenHeight() return 768 / (cvars.useUiScale == "1" and tonumber(cvars.uiScale) or 0.9) end
function GetScreenWidth() return GetScreenHeight() * screen.w / screen.h end
function GetCVar(k)
    if k == "gxResolution" then return screen.w .. "x" .. screen.h end
    if k == "uiScaleMode" then return browser and (cvars.uiScaleMode or "pixel") or nil end
    return cvars[k]
end
function SetCVar(k, v)
    cvars[k] = tostring(v)
    if k == "uiScale" then sets[#sets + 1] = tonumber(v) end
end
function InCombatLockdown() return combat end
function UnitLevel() return 70 end
function UnitName() return "Me" end
function UnitClass() return "Warrior", "WARRIOR" end
function SendAddonMessage() end
function GetRealmName() return "Wrath Unleashed" end
StaticPopupDialogs = { DELETE_ITEM = {}, DELETE_GOOD_ITEM = { hasEditBox = 1 } }
SlashCmdList = {}
DEFAULT_CHAT_FRAME = { AddMessage = function() end }

local function load(db)
    WrathUnleashedUI, WrathUnleashedUIDB, WrathUnleashedUIChar = nil, db, nil
    dofile(file)
    return WrathUnleashedUI
end
local function fire(event, ...)
    for _, f in ipairs(frames) do
        if f.events[event] and f.scripts.OnEvent then f.scripts.OnEvent(f, event, ...) end
    end
end
local function tick()
    for _, f in ipairs(frames) do if f.scripts.OnUpdate then f.scripts.OnUpdate(f, 0.02) end end
end
local function login(db)
    local W = load(db)
    fire("ADDON_LOADED", "WrathUnleashedUI")
    fire("VARIABLES_LOADED")
    fire("PLAYER_LOGIN")
    return W
end

-- ---- the size for common screens ----------------------------------------------------------------
reset()
local W = load({})
local want = {
    { 1280, 720, 0.93 }, { 1366, 768, 0.93 }, { 1920, 1080, 0.71 },
    { 2560, 1440, 0.64 }, { 3440, 1440, 0.64 }, { 3840, 2160, 0.64 },
}
local half = W.ROW_HALF + W.FIT_MARGIN
for _, s in ipairs(want) do
    local k = W.FitScale(s[1], s[2], half)
    local units = 768 * s[1] / s[2] / k              -- the screen's width in UI units at that size
    check(("%dx%d: %.2f"):format(s[1], s[2], s[3]), k == s[3], k)
    check(("%dx%d: the whole bottom row with its gryphons is on screen"):format(s[1], s[2]), units >= 2 * half, units)
end
check("4:3 (1024x768) is made smaller so the row still fits", W.FitScale(1024, 768, half) == 0.69, W.FitScale(1024, 768, half))
check("never above 1.0 nor below the slider's 0.5", W.FitScale(800, 600, 2000) == 0.5 and W.FitScale(640, 480, 10) == 1)
check("no screen size: no size", W.FitScale(0, 0) == nil and W.FitScale(nil, 1080) == nil)

-- Forever UI's measured row (its BottomRow, the gryphons 154 wide with 30 over the bar)
ForeverUI = { BottomRow = { left = -587.5, right = 529, top = 61 },
    ActionBarEndCaps = { left = { GetWidth = function() return 154 end }, inset = 30 } }
check("the row is measured from Forever UI", W.RowHalf() == 587.5 + 124 + 8, W.RowHalf())
-- the owner's window (1896x1071): at the old 1.2 the row is wider than the screen; the chosen size holds it
screen.w, screen.h = 1896, 1071
local k = W.ModernScale()
check("the owner's window: 0.71", k == 0.71, k)
check("the owner's window at 1.2 (his shot): the row doesn't fit", 768 * 1896 / 1071 / 1.2 < 2 * W.RowHalf())
check("the owner's window at the chosen size: it fits", 768 * 1896 / 1071 / k >= 2 * W.RowHalf())
ForeverUI = nil

-- ---- desktop, first login on a Leyline realm --------------------------------------------------------
screen.w, screen.h = 1920, 1080
reset()
local db = { uiScale = 1.2 }        -- saved by the old Mods page whenever it opened
W = login(db)
check("the old saved size (not a choice) is dropped", db.uiScale == nil and db.uiScaleV == 2)
check("another server / before the realm's hello: nothing set", #sets == 0)
fire("PLAYER_ENTERING_WORLD")
W.SetLeyline()
check("on the Leyline realm: the size set once", #sets == 1 and sets[1] == 0.71 and cvars.useUiScale == "1", sets[1])
fire("UI_SCALE_CHANGED"); fire("CVAR_UPDATE", "USE_UISCALE"); fire("DISPLAY_SIZE_CHANGED"); tick(); tick()
check("its own events: no second change", #sets == 1, #sets)
check("our change isn't taken for the player's", db.uiScaleOwn == nil and db.uiFit == true)
check("UIParent:SetScale never used (it fought the game's own size: the jumps)", uiParentScaled == 0)

-- ---- the next login: set before the world, nothing to do ---------------------------------------------
local saved = cvars
reset(saved)
W = login(db)
check("next login, same screen: no change at all (one layout)", #sets == 0, #sets)
fire("PLAYER_ENTERING_WORLD"); W.SetLeyline(); fire("UI_SCALE_CHANGED")
check("next login: still none after entering the world", #sets == 0)

-- a different monitor between sessions: set at load, before anything is drawn
screen.w, screen.h = 1280, 720
reset(saved)
W = login(db)
check("a new screen: set while loading, once", #sets == 1 and sets[1] == 0.93, sets[1])
fire("PLAYER_ENTERING_WORLD"); W.SetLeyline(); fire("UI_SCALE_CHANGED")
check("a new screen: no second change after entering the world", #sets == 1)

-- ---- window resized ---------------------------------------------------------------------------------
screen.w, screen.h = 2560, 1440
fire("DISPLAY_SIZE_CHANGED"); fire("DISPLAY_SIZE_CHANGED"); fire("UI_SCALE_CHANGED")
check("resize: nothing until the next frame", #sets == 1)
tick(); tick()
check("resize: one change to the new screen's size", #sets == 2 and sets[2] == 0.64, sets[2])
fire("UI_SCALE_CHANGED")
fire("DISPLAY_SIZE_CHANGED"); tick()
check("resize to the same size: nothing", #sets == 2)
combat = true
screen.w, screen.h = 1920, 1080
fire("DISPLAY_SIZE_CHANGED"); tick()
check("in combat: waits", #sets == 2)
combat = false
fire("PLAYER_REGEN_ENABLED")
check("after the fight: set", #sets == 3 and sets[3] == 0.71, sets[3])

-- ---- the player's own size ----------------------------------------------------------------------------
check("the slider: the player's size", W.SetOwnScale(0.85) and db.uiScaleOwn == 0.85 and cvars.uiScale == "0.85")
local n = #sets
screen.w, screen.h = 1280, 720
fire("UI_SCALE_CHANGED"); fire("DISPLAY_SIZE_CHANGED"); tick()
check("the player's size stays at any window size", #sets == n and cvars.uiScale == "0.85")
reset(cvars)
W = login(db)
fire("PLAYER_ENTERING_WORLD"); W.SetLeyline()
check("the player's size stays after a login", #sets == 0 and W.CurrentScale() == 0.85)
check("Fit to screen: back to the screen's size", W.FitToScreen() and db.uiScaleOwn == nil and cvars.uiScale == "0.93")
-- the game's own Interface options
cvars.uiScale = "0.78"; fire("UI_SCALE_CHANGED")
check("a size set in the game's own options is the player's", db.uiScaleOwn == 0.78, db.uiScaleOwn)
n = #sets
screen.w, screen.h = 1920, 1080
fire("DISPLAY_SIZE_CHANGED"); tick()
check("and isn't fought", #sets == n)
W.FitToScreen()
cvars.useUiScale = "0"; fire("CVAR_UPDATE", "USE_UISCALE")
check("'Use UI Scale' unticked: the game's own size, left alone", db.uiScaleOwn == "game")
n = #sets
screen.w, screen.h = 1366, 768
fire("DISPLAY_SIZE_CHANGED"); tick()
check("unticked: nothing set after a resize", #sets == n)

-- ---- the browser client: its own pixel mode ---------------------------------------------------------
browser = true
screen.w, screen.h = 1920, 1080
reset({ uiScaleMode = "pixel", uiScale = "1", useUiScale = "0" })
db = {}
W = login(db)
fire("PLAYER_ENTERING_WORLD"); W.SetLeyline(); fire("UI_SCALE_CHANGED")
screen.w, screen.h = 1280, 720
fire("DISPLAY_SIZE_CHANGED"); tick()
check("browser: nothing set at login or on resize (the client sizes it every frame)", #sets == 0 and cvars.uiScaleMode == "pixel")
check("browser: the size shown is the client's (768 / height)", W.CurrentScale() == 1)
screen.w, screen.h = 1920, 1080
check("browser: 1080 px tall shows 0.71", W.CurrentScale() == 0.71, W.CurrentScale())
W.SetOwnScale(0.9)
check("browser: the slider still sets the game's size", cvars.uiScale == "0.90" and db.uiScaleOwn == 0.9)
W.FitToScreen()
check("browser: Fit to screen gives it back to the client's pixel mode", cvars.uiScaleMode == "pixel" and db.uiScaleOwn == nil)
browser = false

print(("\n%d/%d passed"):format(pass, pass + fail)); os.exit(fail == 0 and 0 or 1)
