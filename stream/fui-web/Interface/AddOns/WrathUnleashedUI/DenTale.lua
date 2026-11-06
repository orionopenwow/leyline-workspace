-- Den tales (3.3.5a API): the small tracker for the tale under way in the Den, Jaina's draft with Play it / Not now,
-- and the Den's code to copy (docs/DEN-NEXT.md; labs/den-lite taleplay.rs and tale.rs). /tale opens it; /tale play,
-- /tale stop and /tale list are the Den's own `.den tale ...`.
--
-- The server's lines (mod "wuden", over the realm's addon channel; nothing in them is ever run, all of it is shown
-- as plain text):
--   tale   step|<n>|<of>|<title>|<goal>     the step under way
--          hint|<text>                      its hint
--          reward|<text>                    a reward earned (looks only)
--          done                             the tale is over
--          off                              no tale (stopped)
--          draft|<title>|<steps>            Jaina's draft, waiting for a yes or no
--          draft-off                        no draft
--   pack   <i>/<n> parts of a code or a leyline://pack/ link (.den pack, .den tale code)
-- Buttons send Den commands ("tale yes", "tale no", "tale stop": EditMode.lua DenCmd, as the Tournament window).

local W = WrathUnleashedUI or {}
WrathUnleashedUI = W
local T = { step = nil, hint = nil, reward = nil, done = false, draft = nil, code = nil, parts = {} }
local MOD = "wuden"

local function plain(s) return (tostring(s or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("|", "/"):gsub("%c", " "):sub(1, 160)) end

-- a font string, or a stand-in where the game (or a test's fake) gives none
local function label(parent, size)
    local fs = parent and parent.CreateFontString and parent:CreateFontString(nil, "OVERLAY", size or "GameFontHighlightSmall")
    if not fs then fs = { SetText = function(self, t) self.text = t end, SetPoint = function() end, SetJustifyH = function() end,
        SetWidth = function() end, Show = function() end, Hide = function() end } end
    return fs
end

local function send(text)
    local E = W.EditMode
    if E and E.DenCmd then return E.DenCmd(text) end
    if not SendAddonMessage or not UnitName then return false end
    pcall(SendAddonMessage, "OVMOD", MOD .. "\tcmd\t1/1\t" .. tostring(text):gsub("[^%w%s%-_]", ""):sub(1, 60), "WHISPER", UnitName("player"))
    return true
end
T.Send = send

local panel
local function button(parent, text, w, fn)
    local b = CreateFrame("Button", nil, parent, "UIPanelButtonTemplate")
    if b.SetSize then b:SetSize(w or 80, 20) end
    if b.SetText then b:SetText(text) end
    if b.SetScript then b:SetScript("OnClick", fn) end
    b.label = text
    return b
end

function T.Build()
    if panel then return panel end
    panel = CreateFrame("Frame", "LeylineDenTale", UIParent)
    if panel.SetSize then panel:SetSize(260, 116) end
    if panel.SetPoint then panel:SetPoint("TOPRIGHT", UIParent, "TOPRIGHT", -24, -220) end
    if panel.SetBackdrop then
        pcall(panel.SetBackdrop, panel, { bgFile = "Interface\\Tooltips\\UI-Tooltip-Background", edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border", edgeSize = 12,
            insets = { left = 3, right = 3, top = 3, bottom = 3 } })
        if panel.SetBackdropColor then panel:SetBackdropColor(0.05, 0.06, 0.09, 0.86) end
    end
    panel.title = label(panel, "GameFontNormal")
    panel.title:SetPoint("TOPLEFT", panel, "TOPLEFT", 10, -8)
    panel.line = label(panel)
    panel.line:SetPoint("TOPLEFT", panel, "TOPLEFT", 10, -28)
    panel.line:SetWidth(240)
    panel.line:SetJustifyH("LEFT")
    panel.hint = label(panel)
    panel.hint:SetPoint("TOPLEFT", panel, "TOPLEFT", 10, -62)
    panel.hint:SetWidth(240)
    panel.hint:SetJustifyH("LEFT")
    panel.yes = button(panel, "Play it", 80, function() send("tale yes") end)
    panel.no = button(panel, "Not now", 80, function() send("tale no") end)
    panel.stop = button(panel, "Stop", 60, function() send("tale stop") end)
    panel.close = button(panel, "x", 20, function() panel:Hide() end)
    if panel.yes.SetPoint then
        panel.yes:SetPoint("BOTTOMLEFT", panel, "BOTTOMLEFT", 8, 8)
        panel.no:SetPoint("LEFT", panel.yes, "RIGHT", 6, 0)
        panel.stop:SetPoint("BOTTOMRIGHT", panel, "BOTTOMRIGHT", -8, 8)
        panel.close:SetPoint("TOPRIGHT", panel, "TOPRIGHT", -4, -4)
    end
    -- the code to copy (Ctrl+C), when one came
    panel.code = CreateFrame("EditBox", nil, panel, "InputBoxTemplate")
    if panel.code.SetSize then panel.code:SetSize(236, 18) end
    if panel.code.SetPoint then panel.code:SetPoint("BOTTOMLEFT", panel, "BOTTOMLEFT", 14, 32) end
    if panel.code.SetAutoFocus then panel.code:SetAutoFocus(false) end
    if panel.code.SetScript then
        panel.code:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
        panel.code:SetScript("OnEditFocusGained", function(self) if self.HighlightText then self:HighlightText() end end)
    end
    if panel.Hide then panel:Hide() end
    return panel
end

-- what the tracker shows now (the tests read it)
function T.View()
    if T.draft then
        return "Jaina's tale: " .. T.draft.title, T.draft.steps, "draft"
    end
    if T.step then
        local s = T.step
        return "Tale, step " .. s.n .. " of " .. s.of, s.title .. ": " .. s.goal, T.done and "done" or "step"
    end
    if T.done then return "Tale", T.reward or "The tale is done.", "done" end
    if T.code then return "Your code", "Press Ctrl+C to copy it, then paste it to a friend.", "code" end
    return nil
end

function T.Paint()
    local title, line, kind = T.View()
    if not title then
        if panel and panel.Hide then panel:Hide() end
        return
    end
    T.Build()
    panel.title:SetText(plain(title))
    panel.line:SetText(plain(line))
    panel.hint:SetText(plain(kind == "draft" and "Play it, or Not now." or (T.reward and ("Earned: " .. T.reward)) or T.hint or ""))
    local function vis(b, on) if b then if on then b:Show() else b:Hide() end end end
    vis(panel.yes, kind == "draft")
    vis(panel.no, kind == "draft")
    vis(panel.stop, kind == "step")
    vis(panel.code, T.code ~= nil)
    if T.code and panel.code.SetText then panel.code:SetText(T.code) end
    panel:Show()
end

function T.OnMessage(prefix, msg, channel, sender)
    if prefix ~= "OVMOD" or type(msg) ~= "string" or channel ~= "WHISPER" then return end
    if UnitName and sender ~= UnitName("player") then return end
    local mod, nm, i, n, data = msg:match("^([^\t]+)\t([^\t]+)\t(%d+)/(%d+)\t(.*)$")
    if mod ~= MOD then return end
    i, n = tonumber(i), tonumber(n)
    if nm == "pack" then
        if not (i and n) or n > 40 or i < 1 or i > n then return end
        if i == 1 then T.parts = {} end
        T.parts[i] = data
        for k = 1, n do if T.parts[k] == nil then return end end
        local all = table.concat(T.parts):gsub("[^%w%.%-_:/]", "")
        if all:match("^leyline://pack/ley1%.") or all:match("^ley1%.") then T.code = all; T.Paint() end
        return
    end
    if nm ~= "tale" or i ~= 1 or n ~= 1 then return end
    local kind, rest = data:match("^([%a%-]+)|?(.*)$")
    if kind == "step" then
        local sn, of, title, goal = rest:match("^(%d+)|(%d+)|([^|]*)|(.*)$")
        if not sn then return end
        T.step = { n = tonumber(sn), of = tonumber(of), title = plain(title), goal = plain(goal) }
        T.hint, T.done, T.draft = nil, false, nil
    elseif kind == "hint" then
        T.hint = plain(rest)
    elseif kind == "reward" then
        T.reward = plain(rest)
    elseif kind == "done" then
        T.done, T.step = true, nil
    elseif kind == "off" then
        T.step, T.hint, T.done, T.reward = nil, nil, false, nil
    elseif kind == "draft" then
        local title, steps = rest:match("^([^|]*)|(.*)$")
        if not title then return end
        T.draft = { title = plain(title), steps = plain(steps) }
    elseif kind == "draft-off" then
        T.draft = nil
    else
        return
    end
    T.Paint()
end

local listener = CreateFrame("Frame")
if listener.RegisterEvent then listener:RegisterEvent("CHAT_MSG_ADDON") end
if listener.SetScript then
    listener:SetScript("OnEvent", function(_, event, prefix, msg, channel, sender)
        if event == "CHAT_MSG_ADDON" then return T.OnMessage(prefix, msg, channel, sender) end
    end)
end

SLASH_LEYLINETALE1 = "/tale"
SLASH_LEYLINETALE2 = "/quest"
SlashCmdList = SlashCmdList or {}
SlashCmdList["LEYLINETALE"] = function(msg)
    msg = tostring(msg or ""):gsub("^%s+", ""):gsub("%s+$", "")
    if msg == "" then
        if panel and panel.IsShown and panel:IsShown() then panel:Hide() else T.Paint() end
        return
    end
    send("tale " .. msg)
end

W.DenTale = T
