-- DenBar (3.3.5a API): Den mode, the one place to deal with Your Den (owner, 1 Nov 23:46: "There was no interface to
-- deal with the den. Nothing made sense ... make it make sense, not spam the chat").
--
-- On entering Your Den (the launcher's own Den on this PC: realm "Your Den", W.InDen; or any Den whose server answers
-- the bar's hello) a small bar opens by itself at the top of the screen, in the Leyline look (Style.lua):
--   Build (F7)      the Builder: place anything from the whole catalog (search, categories), move, turn, delete,
--                   undo. In the browser client the free building camera (EditMode.lua) instead.
--   Shape ground    asks the launcher to open Forge (the world editor) on the Den's ground (".den forge").
--   Play (F8)       play mode: every editor window, the shelf and the handles go away and you just walk your Den;
--                   the bar shrinks to one button. F8 again, that button, or Esc > Back to building returns.
--   Save            names a save (".den save <name>"); the autosave keeps the Den anyway, and the bar says when:
--                   "Saved 12 s ago".
--   Invite          one link for one friend (".den invite"); it lands in a copy box here (Ctrl+C).
--   Help            the short intro again.
-- The first time (remembered in WrathUnleashedUIDB.den.introSeen) a short intro of four cards shows; Next / Skip.
--
-- Den Lite (labs/den-lite notice.rs) talks to the bar over the realm's addon channel, mod "wuden", whispered:
--   client -> server   hello <version>     the bar is here: Den Lite's own lines become toasts from now on
--                      status              send the status line again
--                      cmd <text>          a .den command (save, invite, forge), as the Den panel's
--   server -> client   status  role=..;props=<n>;saved=<seconds ago>|-
--                      toast   <kind>|<text>   kind: info, ok, warn, jaina, cheer: one line under the bar for a few
--                                              seconds, one at a time (a short queue, no repeats)
--                      saved   auto|<name>     the Den was just kept (the autosave, or a save by name)
--                      invite  <link>          the invite link (checked as EditMode.lua checks it)
-- Nothing the server sends is run; texts are shown plain (colour codes taken out).
--
-- Keys (override bindings, only while in the Den, given back on leaving): F7 Build and F8 Play when the player hasn't
-- bound them to something (a stock game binds neither); else Ctrl+F7 / Ctrl+F8; in the browser client F7 is its own
-- (the building camera), so only Play is bound.

local W = WrathUnleashedUI
local D = { toasts = {}, play = false, active = false, saved = nil, keys = {} }
local MOD = "wuden"
local VERSION = "1"
local TOAST_SECS = 5
local TOAST_QUEUE = 4
local NAME_MAX = 24
local BAR_W, BAR_H, BTN_W, BTN_H, GAP = 560, 54, 84, 26, 6
local KEY_CHOICES = { build = { "F7", "CTRL-F7" }, play = { "F8", "CTRL-F8" } }
local KIND_COLOUR = { info = "text", ok = "ok", warn = "warn", jaina = "violet", cheer = "accent" }

-- TODO(owner): a voice pass on the intro cards, his words.
local CARDS = {
    { "This is your Den", "An empty white world that's yours. Everything you do here keeps itself: it saves as you go." },
    { "Build", "Press Build (or F7) to place anything: search the whole catalog, then move, turn or delete it. Undo takes the last change back." },
    { "Shape ground", "Opens the world editor on your Den's ground: raise hills, dig, paint the ground. Your game shows it after a restart." },
    { "Play and invite", "Play (F8) hides the tools so you just walk your Den. Invite makes one link for a friend: send it, and they join you here." },
}
D.CARDS = CARDS

local bar, drawer, intro, menuButton, keyOwner

local function clock() return (GetTime and GetTime()) or 0 end
local function S() return W.Style end
local function plain(s)
    return (tostring(s or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("|H.-|h", ""):gsub("|h", ""):gsub("|T.-|t", ""):gsub("|", ""):gsub("%c", " "))
end
local function hex(name) local st = S(); return st and st.Hex(name) or "|cffffffff" end

local function send(name, data)
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, "OVMOD", MOD .. "\t" .. name .. "\t1/1\t" .. (data or ""), "WHISPER", UnitName("player"))
    return true
end
local function denCmd(text)
    local E = W.EditMode
    if E and E.DenCmd then return E.DenCmd(text) end
    return send("cmd", (tostring(text or ""):gsub("[^%w%s%-_]", ""):sub(1, 60)))
end

local function db()
    WrathUnleashedUIDB = type(WrathUnleashedUIDB) == "table" and WrathUnleashedUIDB or {}
    local d = WrathUnleashedUIDB
    d.den = type(d.den) == "table" and d.den or {}
    return d.den
end

-- ---------------------------------------------------------------- the status line: toasts, and "Saved 12 s ago"
function D.Ago(secs)
    secs = math.max(0, math.floor(tonumber(secs) or 0))
    if secs < 60 then return secs .. " s ago" end
    if secs < 3600 then return math.floor(secs / 60) .. " min ago" end
    return math.floor(secs / 3600) .. " h ago"
end

function D.SavedLine()
    local s = D.saved
    if not s then return "Your Den saves itself as you build." end
    local what = (s.name and s.name ~= "auto") and ("Saved as " .. s.name .. " ") or "Saved "
    return what .. D.Ago(clock() - s.at)
end

local function showLine()
    if not bar then return end
    local t = D.current
    local text = t and (hex(KIND_COLOUR[t.kind] or "text") .. (t.kind == "jaina" and "Jaina: " or "") .. t.text .. "|r") or (hex("muted") .. D.SavedLine() .. "|r")
    bar.line:SetText(text)
    if bar.pill then bar.pill.line:SetText(text) end
end

-- One line in the bar for TOAST_SECS, one at a time; the same line twice in a row shows once. `now`: before the
-- others (the bar's own answer to a click).
function D.Toast(kind, text, now)
    text = plain(text):gsub("^%s+", ""):gsub("%s+$", "")
    if text == "" then return false end
    kind = KIND_COLOUR[kind or ""] and kind or "info"
    local last = D.toasts[#D.toasts] or D.current
    if last and last.text == text then return false end
    if #D.toasts >= TOAST_QUEUE then table.remove(D.toasts, 1) end
    if now then
        table.insert(D.toasts, 1, { kind = kind, text = text })
        D.NextToast()
    else
        D.toasts[#D.toasts + 1] = { kind = kind, text = text }
        if not D.current then D.NextToast() end
    end
    -- the Builder's own status line says it too, when its window is open (its answers used to be in the chat)
    local B = W.Builder
    local win = B and B.Window and B.Window()
    if win and win:IsShown() and B.SetStatus and kind ~= "jaina" and kind ~= "cheer" then B.SetStatus(text) end
    return true
end

function D.NextToast()
    D.current = table.remove(D.toasts, 1)
    if D.current then D.current.until_ = clock() + TOAST_SECS end
    showLine()
end

function D.Tick()
    if D.current and clock() >= D.current.until_ then D.NextToast() end
    if not D.current then showLine() end   -- "Saved 12 s ago" counts on
end

-- ---------------------------------------------------------------- the bar
local function button(parent, text, w, onClick, kind, ...)
    local st = S()
    local b
    if st then
        b = st.Button(parent, text, w, onClick, kind, ...)
    else
        b = CreateFrame("Button", nil, parent, "UIPanelButtonTemplate")
        b:SetWidth(w); b:SetText(text); b:SetPoint(...); b:SetScript("OnClick", onClick)
    end
    b:SetHeight(BTN_H)
    return b
end

local function keyText(which) local k = D.keys[which]; return k and (" (" .. k:gsub("CTRL%-", "Ctrl+") .. ")") or "" end

function D.Labels()
    if not bar then return end
    bar.build:SetText("Build" .. keyText("build"))
    bar.playBtn:SetText("Play" .. keyText("play"))
    if bar.pill then bar.pill.back:SetText("Back to building" .. keyText("play")) end
end

local function buildBar()
    bar = CreateFrame("Frame", "LeylineDenBar", UIParent)
    bar:SetWidth(BAR_W); bar:SetHeight(BAR_H)
    bar:SetPoint("TOP", UIParent, "TOP", 0, -10)
    bar:SetFrameStrata("MEDIUM")
    bar:EnableMouse(true)
    if S() then S().Box(bar, 0.92) end
    local x = 8
    local function add(key, text, w, fn, kind)
        local b = button(bar, text, w, fn, kind, "TOPLEFT", x, -6)
        x = x + w + GAP
        bar[key] = b
        return b
    end
    add("build", "Build", BTN_W + 16, function() D.Build() end, "primary")
    add("ground", "Shape ground", BTN_W + 22, function() D.ShapeGround() end)
    add("playBtn", "Play", BTN_W + 8, function() D.SetPlay(true) end)
    add("save", "Save", BTN_W - 16, function() D.OpenSave() end)
    add("invite", "Invite", BTN_W - 10, function() D.Invite() end)
    add("help", "Help", BTN_W - 26, function() D.ShowIntro(1) end)
    bar.line = bar:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    bar.line:SetPoint("TOPLEFT", 10, -BTN_H - 12)
    bar.line:SetWidth(BAR_W - 20); bar.line:SetJustifyH("LEFT")
    bar.line.styleKeep = true

    -- play mode: one small button and the line
    local pill = CreateFrame("Frame", "LeylineDenPlayPill", UIParent)
    pill:SetWidth(330); pill:SetHeight(BAR_H)
    pill:SetPoint("TOP", UIParent, "TOP", 0, -10)
    pill:EnableMouse(true)
    if S() then S().Box(pill, 0.8) end
    pill.back = button(pill, "Back to building", 170, function() D.SetPlay(false) end, "ghost", "TOPLEFT", 8, -6)
    pill.line = pill:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    pill.line:SetPoint("TOPLEFT", 10, -BTN_H - 12)
    pill.line:SetWidth(310); pill.line:SetJustifyH("LEFT")
    pill.line.styleKeep = true
    pill:Hide()
    bar.pill = pill

    -- the drawer under the bar: a name for Save, or the invite link to copy
    drawer = CreateFrame("Frame", "LeylineDenDrawer", bar)
    drawer:SetWidth(BAR_W); drawer:SetHeight(40)
    drawer:SetPoint("TOP", bar, "BOTTOM", 0, -4)
    drawer:EnableMouse(true)
    if S() then S().Box(drawer, 0.95) end
    drawer.label = drawer:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    drawer.label:SetPoint("LEFT", 10, 0)
    drawer.label:SetWidth(110); drawer.label:SetJustifyH("LEFT")
    drawer.box = (S() and S().Input and S().Input(drawer, 300, 22)) or CreateFrame("EditBox", nil, drawer, "InputBoxTemplate")
    drawer.box:SetWidth(300); drawer.box:SetHeight(22)
    drawer.box:SetPoint("LEFT", 124, 0)
    drawer.box:SetAutoFocus(false)
    drawer.box:SetScript("OnEscapePressed", function(self) self:ClearFocus(); D.CloseDrawer() end)
    drawer.box:SetScript("OnEnterPressed", function() if D.drawerKind == "save" then D.Save(drawer.box:GetText()) else D.CloseDrawer() end end)
    drawer.box:SetScript("OnEditFocusGained", function(self) if D.drawerKind == "invite" then self:HighlightText() end end)
    -- the link stays as it came (a typed key doesn't change it)
    drawer.box:SetScript("OnTextChanged", function(self)
        if D.drawerKind == "invite" and D.link and self:GetText() ~= D.link then self:SetText(D.link); self:HighlightText() end
    end)
    drawer.ok = button(drawer, "Save", 60, function() if D.drawerKind == "save" then D.Save(drawer.box:GetText()) else D.CloseDrawer() end end, "primary", "LEFT", 432, 0)
    drawer.close = button(drawer, "Close", 56, function() D.CloseDrawer() end, "ghost", "LEFT", 496, 0)
    drawer:Hide()

    bar:Hide()
    if S() and S().Recolor then S().Recolor(bar) end
    D.Labels()
end
function D.Bar() if not bar then buildBar() end return bar end
function D.Drawer() D.Bar(); return drawer end

function D.OpenDrawer(kind)
    D.Bar()
    D.drawerKind = kind
    if kind == "save" then
        drawer.label:SetText("Name this save:")
        drawer.ok:SetText("Save")
        drawer.box:SetMaxLetters(NAME_MAX)
        drawer.box:SetText(D.lastName or "my-den")
        drawer.box:SetFocus()
        drawer.box:HighlightText()
    else
        drawer.label:SetText("Invite link:")
        drawer.ok:SetText("Done")
        drawer.box:SetMaxLetters(0)
        drawer.box:SetText(D.link or "")
        if D.link then drawer.box:SetFocus(); drawer.box:HighlightText() end
    end
    drawer:Show()
end
function D.CloseDrawer()
    if not drawer then return end
    D.drawerKind = nil
    drawer.box:ClearFocus()
    drawer:Hide()
end

-- ---------------------------------------------------------------- the buttons
function D.Build()
    if D.play then D.SetPlay(false) end
    local E = W.EditMode
    -- the browser client's free building camera; on a player's own game the Builder window does everything
    if E and E.Desktop and not E.Desktop() and E.Toggle then E.Toggle(); return "camera" end
    if W.ToggleBuilder then W.ToggleBuilder() end
    return "builder"
end

function D.ShapeGround()
    denCmd("forge")
    return true
end

function D.OpenSave() D.OpenDrawer("save") end

function D.Save(name)
    local n = tostring(name or ""):gsub("^%s+", ""):gsub("%s+$", ""):gsub("%s+", "-")
    if n == "" or #n > NAME_MAX or not n:match("^[%w_%-]+$") then
        D.Toast("warn", "A save's name: letters, numbers, - and _ (up to " .. NAME_MAX .. ").", true)
        return false
    end
    D.lastName = n
    denCmd("save " .. n)
    D.CloseDrawer()
    return true
end

function D.Invite()
    D.link = nil
    denCmd("invite")
    D.OpenDrawer("invite")
    drawer.box:SetText("")
    drawer.label:SetText("Making a link...")
    return true
end

-- What play mode hides: every editor window, the shelf (with edit mode), what is being placed, the selection.
local function editorFrames()
    local out = {}
    local B, E, H = W.Builder, W.EditMode, W.DenHome
    local function add(f) if f then out[#out + 1] = f end end
    if B then add(B.Window and B.Window()); add(B.Menu and B.Menu()); add(B.Panel and B.Panel()) end
    if E then add(E.DenPanel and E.DenPanel()) end
    if H then add(H.Panel and H.Panel()) end
    add(intro); add(drawer)
    return out
end

function D.SetPlay(on)
    on = on and true or false
    if not D.active then on = false end
    D.play = on
    if on then
        local E, H, B = W.EditMode, W.DenHome, W.Builder
        if H and H.Placing and H.Placing() and H.Cancel then H.Cancel(true) end
        if E and E.IsOn and (E.IsOn() or E.allowed or E.asking) and E.Leave then E.Leave() end
        for _, f in ipairs(editorFrames()) do if f.IsShown and f:IsShown() then f:Hide() end end
        D.CloseDrawer()
        -- no handles on a selected prop while you walk
        if B and B.state and B.state.selId and B.Command then B.Command("selclear") end
        if LeylineEdit then pcall(LeylineEdit, "group") end
        D.Toast("info", "Play mode: just you and your Den. " .. (D.keys.play and (D.keys.play:gsub("CTRL%-", "Ctrl+") .. " or ") or "") .. "Esc > Back to building.", true)
    end
    D.Refresh()
    return D.play
end
function D.TogglePlay() return D.SetPlay(not D.play) end

-- ---------------------------------------------------------------- the intro
local function buildIntro()
    intro = CreateFrame("Frame", "LeylineDenIntro", UIParent)
    intro:SetWidth(400); intro:SetHeight(190)
    intro:SetPoint("CENTER", UIParent, "CENTER", 0, 80)
    intro:SetFrameStrata("DIALOG")
    intro:EnableMouse(true)
    if S() then S().Window(intro, "Your Den") end
    intro.head = intro:CreateFontString(nil, "OVERLAY", "GameFontNormalLarge")
    intro.head:SetPoint("TOPLEFT", 16, -44)
    intro.head:SetJustifyH("LEFT")
    intro.body = intro:CreateFontString(nil, "OVERLAY", "GameFontHighlight")
    intro.body:SetPoint("TOPLEFT", 16, -72)
    intro.body:SetWidth(368); intro.body:SetJustifyH("LEFT")
    intro.count = intro:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    intro.count:SetPoint("BOTTOMLEFT", 16, 16)
    intro.skip = button(intro, "Skip", 70, function() D.EndIntro() end, "ghost", "BOTTOMRIGHT", -104, 10)
    intro.next = button(intro, "Next", 86, function() D.NextCard() end, "primary", "BOTTOMRIGHT", -12, 10)
    if UISpecialFrames then tinsert(UISpecialFrames, "LeylineDenIntro") end
    intro:SetScript("OnHide", function() if D.card then D.EndIntro() end end)
    intro:Hide()
end

function D.ShowIntro(n)
    if not intro then buildIntro() end
    if D.play then D.SetPlay(false) end
    D.card = math.max(1, math.min(#CARDS, tonumber(n) or 1))
    local c = CARDS[D.card]
    intro.head:SetText(c[1])
    intro.body:SetText(c[2])
    intro.count:SetText(D.card .. " / " .. #CARDS)
    intro.next:SetText(D.card == #CARDS and "Start building" or "Next")
    if D.card == #CARDS then intro.skip:Hide() else intro.skip:Show() end
    intro:Show()
    return D.card
end
function D.NextCard()
    if not D.card then return nil end
    if D.card >= #CARDS then D.EndIntro(); return nil end
    return D.ShowIntro(D.card + 1)
end
function D.EndIntro()
    D.card = nil
    db().introSeen = true
    if intro and intro:IsShown() then intro:Hide() end
end
function D.Intro() return intro end

-- ---------------------------------------------------------------- keys and the Esc menu
local keyDownSeen, keyHalf = false, {}
local function keyButton(which, fn)
    local name = which == "build" and "LeylineDenKeyBuild" or "LeylineDenKeyPlay"
    if _G[name] then return name end
    local b = CreateFrame("Button", name, UIParent)
    b:EnableMouse(false)
    if b.RegisterForClicks then b:RegisterForClicks("AnyDown", "AnyUp") end
    -- a click binding clicks as the key goes down and as it comes up (DenHome.lua's way): once per press
    b:SetScript("OnClick", function(_, _, down)
        if down == true then keyDownSeen = true end
        if keyDownSeen then
            if down == true then fn() end
        else
            keyHalf[which] = not keyHalf[which]
            if keyHalf[which] then fn() end
        end
    end)
    return name
end

local function free(key)
    if not GetBindingAction then return true end
    local ok, a = pcall(GetBindingAction, key)
    return not ok or a == nil or a == ""
end

function D.BindKeys()
    if not keyOwner then keyOwner = CreateFrame("Frame", "LeylineDenBarKeys", UIParent) end
    if ClearOverrideBindings then ClearOverrideBindings(keyOwner) end
    D.keys = {}
    if D.active and SetOverrideBindingClick and not (InCombatLockdown and InCombatLockdown()) then
        local E = W.EditMode
        local browser = E and E.Desktop and not E.Desktop()
        for _, which in ipairs({ "build", "play" }) do
            if not (which == "build" and browser) then
                for _, k in ipairs(KEY_CHOICES[which]) do
                    if free(k) then
                        SetOverrideBindingClick(keyOwner, false, k, keyButton(which, which == "build" and D.Build or D.TogglePlay))
                        D.keys[which] = k
                        break
                    end
                end
            elseif browser then
                D.keys.build = "F7"   -- the browser client's own key
            end
        end
    end
    D.Labels()
    return D.keys
end

local function menuRefresh()
    if not GameMenuFrame then return end
    if not menuButton then
        menuButton = CreateFrame("Button", "GameMenuButtonLeylineDen", GameMenuFrame, "GameMenuButtonTemplate")
        menuButton:SetScript("OnClick", function()
            if HideUIPanel then HideUIPanel(GameMenuFrame) end
            D.TogglePlay()
        end)
    end
    menuButton:SetText(D.play and "Back to building" or "Play your Den")
    if D.active then menuButton:Show() else menuButton:Hide() end
    if GameMenuFrame:IsShown() and W.LayoutGameMenu then W.LayoutGameMenu() end
end
function D.MenuButton() return menuButton end

-- ---------------------------------------------------------------- on and off
function D.Refresh()
    D.Bar()
    if D.active and not D.play then bar:Show() else bar:Hide() end
    if D.active and D.play then bar.pill:Show() else bar.pill:Hide() end
    menuRefresh()
    showLine()
end

function D.SetActive(on)
    on = on and true or false
    if D.active == on then return end
    D.active = on
    if not on then D.play = false; D.CloseDrawer(); if intro then intro:Hide() end end
    D.BindKeys()
    D.Refresh()
    if on and not db().introSeen then D.ShowIntro(1) end
end
function D.IsActive() return D.active end

-- Entering the world: in Your Den the bar opens and says hello; Den Lite answers with the status (any Den that
-- answers gets the bar too). Elsewhere it stays shut (and gives the keys back).
function D.Enter()
    local inDen = W.InDen and W.InDen() or false
    D.SetActive(inDen)
    send("hello", VERSION)
end

-- ---------------------------------------------------------------- the server's answers
local function inviteOk(data)
    return data:match("^https://[%w%.%-]+[:%d]*/join/[a-z2-7]+#[A-Z2-9]+$")
        or data:match("^https://[%w%.%-]+[:%d]*/join/[a-z2-7]+#[A-Z2-9]+~~G[A-Z0-9]+%.[A-Z0-9]+$")
end

function D.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, name, data = msg:match("^([^\t]+)\t([^\t]+)\t1/1\t(.*)$")
    if mod ~= MOD then return end
    if name == "status" then
        local s = {}
        for k, v in data:gmatch("(%a+)=([^;]*)") do s[k] = plain(v) end
        D.status = s
        local ago = tonumber(s.saved or "")
        if ago then D.saved = { at = clock() - ago, name = (D.saved and D.saved.name) or "auto" } end
        D.SetActive(true)
        showLine()
    elseif name == "toast" then
        local kind, text = data:match("^(%a+)|(.*)$")
        if kind then D.Toast(kind, text) end
    elseif name == "saved" then
        local n = plain(data):gsub("[^%w_%-]", ""):sub(1, NAME_MAX)
        D.saved = { at = clock(), name = n ~= "" and n or "auto" }
        showLine()
    elseif name == "invite" and inviteOk(data) then
        D.link = data
        if D.active then
            D.OpenDrawer("invite")
            drawer.label:SetText("Invite link:")
            drawer.box:SetText(data)
            drawer.box:SetFocus()
            drawer.box:HighlightText()
        end
    end
end

local listener = CreateFrame("Frame")
listener:RegisterEvent("CHAT_MSG_ADDON")
listener:RegisterEvent("PLAYER_ENTERING_WORLD")
listener:SetScript("OnEvent", function(_, event, prefix, msg, channel, sender)
    if event == "CHAT_MSG_ADDON" then return D.OnMessage(prefix, msg, channel, sender) end
    if event == "PLAYER_ENTERING_WORLD" then return D.Enter() end
end)
local ticker = CreateFrame("Frame", nil, WorldFrame or UIParent)
local acc = 0
ticker:SetScript("OnUpdate", function(_, elapsed)
    acc = acc + (elapsed or 0)
    if acc < 0.5 then return end
    acc = 0
    if D.active then D.Tick() end
end)

SLASH_LEYLINEDENBAR1 = "/denbar"
SlashCmdList["LEYLINEDENBAR"] = function(msg)
    msg = tostring(msg or ""):lower()
    if msg == "play" then D.TogglePlay() elseif msg == "help" then D.ShowIntro(1) else D.SetActive(not D.active) end
end

D.VERSION, D.TOAST_SECS, D.KEY_CHOICES = VERSION, TOAST_SECS, KEY_CHOICES
W.DenBar = D
