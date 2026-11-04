--[[
    Core.lua  (LeylineChat)
    The chat window, restyled, with the quality-of-life features players usually install chat addons for. The stock
    chat frames (ChatFrame1..10 and the whisper windows the game opens), their tabs and edit boxes are kept: Enter to
    type, tabs, channels, slash commands, whisper reply and other addons' hooks all work as before; this addon only
    styles them and post-processes the lines they print.

        Kit.lua      settings storage, Print, art, the small window kit
        Core.lua     settings, themes, the line pipeline (timestamps, short channel names, class colours, links,
                     mentions, spam control), the tab menu rows, /lchat
        Skin.lua     the look: panel, edge, shadow, tabs, input line, scrollbar, fade, resize
        Tools.lua    copy box, link box, typed history
        World.lua    the World tab (realm-wide chat + Discord [D] lines), channel cycle, /world
        Expand.lua   the expanded history window with search
        Settings.lua the settings window (/lchat)

    Port of Orion's 1.12 Chat addon to 3.3.5a: handlers take (self, event, ...), Lua 5.1, the chat event's
    author reaches the line pipeline through a message event filter (ChatFrame_AddMessageEventFilter), whisper tabs
    are the game's own whisper windows. Turning the addon off (/lchat off) puts the stock chat back as it was.
]]--

local C = LeylineChat
local ID = C.ID

local strfind, strsub, strlen, strlower, strupper = string.find, string.sub, string.len, string.lower, string.upper
local gsub, format, floor, strmatch = string.gsub, string.format, math.floor, string.match
local now = C.Now

-- -- Themes ------------------------------------------------------------------------------------
-- Art: Media\Chat_Panel<suffix>, Chat_Tab<suffix>, Chat_Edge<suffix>. The colours are the flat fallbacks (and the
-- tint of text, tabs and the input line).

C.THEME_ORDER = { "slate", "parchment", "midnight", "wildwood", "glass" }
C.THEMES = {
    slate = {
        label = "Slate", suffix = "",
        face = { 0.105, 0.112, 0.13 }, inset = { 0.05, 0.055, 0.066 }, tab = { 0.085, 0.09, 0.105 },
        edge = { 0.27, 0.29, 0.34 }, top = { 0.56, 0.58, 0.64 }, accent = { 1, 0.82, 0 },
        text = { 0.93, 0.93, 0.95 }, muted = { 0.60, 0.62, 0.68 }, stamp = "8a909c",
        flags = "OUTLINE", edgeSize = 8, edgeOut = 2, shadow = true, topLine = true,
    },
    parchment = {
        label = "Parchment", suffix = "_Parchment",
        face = { 0.78, 0.66, 0.47 }, inset = { 0.68, 0.56, 0.38 }, tab = { 0.70, 0.58, 0.40 },
        edge = { 0.40, 0.27, 0.14 }, top = { 0.96, 0.90, 0.74 }, accent = { 0.62, 0.16, 0.10 },
        text = { 0.20, 0.12, 0.06 }, muted = { 0.36, 0.26, 0.16 }, stamp = "6b5236",
        flags = "", ink = true, edgeSize = 12, edgeOut = 3, shadow = true,
    },
    midnight = {
        label = "Midnight", suffix = "_Midnight",
        face = { 0.035, 0.05, 0.09 }, inset = { 0.02, 0.03, 0.06 }, tab = { 0.03, 0.04, 0.075 },
        edge = { 0.20, 0.27, 0.42 }, top = { 0.46, 0.60, 0.88 }, accent = { 0.55, 0.80, 1 },
        text = { 0.90, 0.93, 1 }, muted = { 0.52, 0.58, 0.72 }, stamp = "6f82a8",
        flags = "OUTLINE", edgeSize = 8, edgeOut = 2, shadow = true, topLine = true,
    },
    wildwood = {
        label = "Wildwood", suffix = "_Wildwood",
        face = { 0.10, 0.095, 0.065 }, inset = { 0.055, 0.05, 0.035 }, tab = { 0.20, 0.15, 0.09 },
        edge = { 0.38, 0.30, 0.19 }, top = { 0.62, 0.52, 0.34 }, accent = { 0.59, 1, 0.67 },
        text = { 0.95, 0.92, 0.85 }, muted = { 0.68, 0.64, 0.54 }, stamp = "8f8a6c",
        flags = "OUTLINE", edgeSize = 14, edgeOut = 6, shadow = true,
    },
    glass = {
        label = "Glass", suffix = "_Glass",
        face = { 0, 0, 0 }, faceAlpha = 0.45, inset = { 0, 0, 0 }, tab = { 0, 0, 0 },
        edge = { 1, 1, 1 }, edgeAlpha = 0.14, top = { 1, 1, 1 }, accent = { 1, 0.82, 0 },
        text = { 0.96, 0.96, 0.96 }, muted = { 0.76, 0.76, 0.80 }, stamp = "a0a0a8",
        flags = "OUTLINE", edgeSize = 6, edgeOut = 1, shadow = false, topLine = true,
    },
}

-- -- Settings ----------------------------------------------------------------------------------

C.DEFAULTS = {
    enabled = true,
    theme = "slate", opacity = 85, fontSize = 13,
    timestamps = "24", channelNames = "short", fade = "20",
    classColors = true, links = true, collapse = true, goldFilter = false,
    mentions = true, mentionSound = true, typedHistory = true,
    wheel = true, arrows = true, sticky = true, history = true, alerts = true,
    worldTab = true, discord = true, worldJoin = true,
}

local function choices(list)
    local out = {}
    for _, c in ipairs(list) do table.insert(out, { value = c[1], label = c[2] }) end
    return out
end

local themeChoices = {}
for _, k in ipairs(C.THEME_ORDER) do table.insert(themeChoices, { k, C.THEMES[k].label }) end

C.SCHEMA = {
    title = "Chat",
    desc = "A sleek, readable chat window. It works as before: Enter to type, the same tabs, channels and slash commands.",
    options = {
        { key = "theme", label = "Theme", type = "choice", choices = choices(themeChoices) },
        { key = "opacity", label = "Background opacity", type = "slider", min = 40, max = 100, step = 5, fmt = "%d%%" },
        { key = "fontSize", label = "Text size", type = "slider", min = 10, max = 18, step = 1, fmt = "%d pt" },
        { key = "timestamps", label = "Timestamps", type = "choice",
          choices = choices({ { "off", "Off" }, { "24", "24-hour" }, { "12", "12-hour" } }) },
        { key = "channelNames", label = "Channel names", type = "choice",
          choices = choices({ { "short", "Short ([G], [T])" }, { "full", "Full names" } }) },
        { key = "fade", label = "Fade when quiet", type = "choice",
          choices = choices({ { "20", "After 20 s" }, { "60", "After a minute" }, { "never", "Never" } }) },
        { key = "classColors", label = "Class-coloured names", sub = "Names take their class colour once you have seen the player (group, guild, friends, target, /who)." },
        { key = "links", label = "Clickable web links", sub = "Web addresses in chat become links that open a box you can copy them from." },
        { key = "collapse", label = "Collapse repeats", sub = "The same line said again shows as \"\195\1512\", \"\195\1513\" instead of filling the window." },
        { key = "goldFilter", label = "Hide gold sellers", sub = "Hides channel and whisper adverts for gold and power-levelling websites." },
        { key = "mentions", label = "Highlight my name", sub = "Lines that mention you stand out. Turn it off for one tab from the chat menu." },
        { key = "mentionSound", label = "Sound when mentioned", sub = "A soft ping, at most once every few seconds." },
        { key = "whisperTabs", label = "Whisper tabs", sub = "Each whisper conversation gets its own tab (the game's whisper windows)." },
        { key = "typedHistory", label = "Remember what I typed", sub = "Up and down arrows bring back lines you sent, even after a reload." },
        { key = "wheel", label = "Scroll with the mouse wheel", sub = "Hold Shift to jump to the top or bottom." },
        { key = "arrows", label = "Arrow keys without Alt", sub = "Move the cursor in the chat box with the arrow keys alone." },
        { key = "sticky", label = "Sticky channels", sub = "Whispers, officer, raid and numbered channels stay selected for the next line." },
        { key = "history", label = "Keep 1,000 lines", sub = "A longer scroll-back in every chat tab." },
        { key = "alerts", label = "Guild log-on alerts", sub = "Say when guild members log on and off." },
        { key = "worldTab", label = "World tab", sub = "Realm-wide World chat (both factions) in a tab of its own. Right-click the tab for its menu." },
        { key = "discord", label = "Show Discord messages", sub = "Lines from the realm's Discord, marked [D]." },
        { key = "worldJoin", label = "Join World chat", sub = "Be in the World channel on every character. Off leaves it (and stays off)." },
    },
}

-- "whisperTabs" is the game's own setting (whisper windows), read and written live.
local function whisperTabsOn()
    local v = GetCVar and C.Try(GetCVar, "whisperMode")
    return v ~= nil and v ~= "inline"
end

function C:Get(key)
    if key == "whisperTabs" then return whisperTabsOn() end
    local v = self:Settings()[key]
    if v == nil then return self.DEFAULTS[key] end
    return v
end

function C:SetOption(key, value)
    if key == "whisperTabs" then
        if SetCVar then C.Try(SetCVar, "whisperMode", value and "popout" or "inline") end
    else
        self:Settings()[key] = value
    end
    self:Reapply()
end

function C:Reapply()
    self:Refresh(self:Get("enabled"))
    if self.RefreshSettings then self:RefreshSettings() end
end

function C:Theme()
    local key = self:Get("theme")
    local t = self.THEMES[key]
    if t then return t, key end
    return self.THEMES.slate, "slate"
end

-- The on/off names for /lchat and the tab menu.
C.OPTIONS = {
    { key = "wheel",      label = "Scroll chat with the mouse wheel" },
    { key = "arrows",     label = "Arrow keys without Alt" },
    { key = "sticky",     label = "Sticky whisper, officer, raid and channels" },
    { key = "history",    label = "Keep 1,000 lines of history" },
    { key = "timestamps", label = "Timestamps" },
    { key = "alerts",     label = "Guild member log-on alerts" },
}

function C:On(key)
    if key == "timestamps" then return self:Get("timestamps") ~= "off" end
    return self:Get(key) and true or false
end

function C:Set(key, on)
    if key == "timestamps" then
        local cur = self:Get("timestamps")
        if on then self:SetOption("timestamps", (cur ~= "off" and cur) or "24") else self:SetOption("timestamps", "off") end
        return
    end
    self:SetOption(key, on and true or false)
end

-- -- Class colours -----------------------------------------------------------------------------
-- Account-wide memory: the class of names seen, for class-coloured names.

C.classCount = 0
function C:Learn(name, class)
    if not name or name == "" or not class or class == "" then return end
    local token = gsub(strupper(class), " ", "")
    local db = self:DB()
    local t = db.classes
    if t[name] ~= token then
        if not t[name] then
            self.classCount = self.classCount + 1
            if self.classCount > 1500 then db.classes = {}; t = db.classes; self.classCount = 0 end
        end
        t[name] = token
    end
end

function C:LearnUnit(unit)
    if not UnitExists or not UnitExists(unit) then return end
    if UnitIsPlayer and not UnitIsPlayer(unit) then return end
    local _, token = UnitClass(unit)
    self:Learn(UnitName(unit), token)
end

function C:ClassOf(name)
    return name and self:DB().classes[name]
end

local function hex(r, g, b)
    return format("%02x%02x%02x", floor((r or 1) * 255 + 0.5), floor((g or 1) * 255 + 0.5), floor((b or 1) * 255 + 0.5))
end
C.Hex = hex

function C:ClassHex(token)
    local list = CUSTOM_CLASS_COLORS or RAID_CLASS_COLORS
    local c = list and list[token or ""]
    if c then return hex(c.r, c.g, c.b) end
end

-- -- Text helpers ------------------------------------------------------------------------------

local function esc(s)
    return (gsub(s, "([%^%$%(%)%%%.%[%]%*%+%-%?])", "%%%1"))
end
C.Escape = esc

-- Apply fn to the parts of text that are not inside a |H...|h...|h link.
local function plainMap(text, fn)
    local out, pos = {}, 1
    while true do
        local s, e = strfind(text, "|H.-|h.-|h", pos)
        if not s then
            table.insert(out, fn(strsub(text, pos)))
            break
        end
        table.insert(out, fn(strsub(text, pos, s - 1)))
        table.insert(out, strsub(text, s, e))
        pos = e + 1
    end
    return table.concat(out)
end
C.PlainMap = plainMap

-- Colour codes, links and inline icons removed: what a player would select and copy.
function C.Strip(text)
    text = text or ""
    text = gsub(text, "|H[^|]*|h(.-)|h", "%1")
    text = gsub(text, "|c%x%x%x%x%x%x%x%x", "")
    text = gsub(text, "|r", "")
    text = gsub(text, "|T[^|]*|t", "")
    text = gsub(text, "||", "|")
    return text
end

-- "2. Trade - City" -> "T", "Guild" -> "G" ...
local GROUP_SHORT = {
    Guild = "G", Officer = "O", Party = "P", ["Party Leader"] = "PL", ["Raid Leader"] = "RL", ["Raid Warning"] = "RW",
    Raid = "R", ["Battleground Leader"] = "BL", Battleground = "BG", ["Dungeon Guide"] = "DG",
}
function C.ShortLabel(label)
    local num, name = strmatch(label, "^(%d+)%. (.*)$")
    if num then
        local l = strlower(name)
        if strfind(l, "^trade") then return "T"
        elseif strfind(l, "^lookingforgroup") or strfind(l, "^lfg") then return "LFG"
        elseif strfind(l, "^world") then return "W"
        elseif strfind(l, "^general") then return num
        end
        return num
    end
    return GROUP_SHORT[label]
end

-- Only at the start of a line: the 3.3.5 channel link |Hchannel:..|h[2. Trade - City]|h, or a plain [Guild].
function C:ShortChannel(text)
    local head, label, rest = strmatch(text, "^(|Hchannel:[^|]*|h)%[([^%]]-)%](|h.*)$")
    if head then
        local short = C.ShortLabel(label)
        if short then return head .. "[" .. short .. "]" .. rest end
        return text
    end
    label, rest = strmatch(text, "^%[([^%]]-)%](.*)$")
    if label then
        local short = C.ShortLabel(label)
        if short then return "[" .. short .. "]" .. rest end
    end
    return text
end

-- The author's name in their class colour: |Hplayer:Name:..|h[Name]|h, or a plain [Name].
function C:ColourName(text, author)
    local hx = self:ClassHex(self:ClassOf(author))
    if not hx then return text end
    local coloured = "|cff" .. hx .. author .. "|r"
    local s, e = strfind(text, "|Hplayer:" .. esc(author) .. "[^|]*|h%[")
    if s and strsub(text, e + 1, e + strlen(author)) == author then
        return strsub(text, 1, e) .. coloured .. strsub(text, e + 1 + strlen(author))
    end
    s = strfind(text, "[" .. author .. "]", 1, true)
    if s then
        return strsub(text, 1, s) .. coloured .. strsub(text, s + 1 + strlen(author))
    end
    return text
end

-- Staff lines (arg6 "GM"): a small blue <GM> before the name, in place of the stock mark (an inline chat icon in
-- 3.3.5, the <GM> text flag elsewhere).
C.GM_TAG = "|cff3fa9ff<GM>|r "
function C:GmTag(text, author)
    local s = strfind(text, "|Hplayer:" .. author, 1, true) or strfind(text, "[" .. author .. "]", 1, true)
    if not s then return text end
    local before = strsub(text, 1, s - 1)
    before = gsub(before, "|T[^|]*[Cc]hat[Ii]con[^|]*|t%s?$", "")
    local flag = CHAT_FLAG_GM or "<GM>"
    if flag ~= "" and strsub(before, -strlen(flag)) == flag then before = strsub(before, 1, -strlen(flag) - 1) end
    return before .. self.GM_TAG .. strsub(text, s)
end

-- Web addresses -> |Hurl:...|h links (opened in the link box by SetItemRef, see Tools.lua).
local TLD = {
    com = 1, net = 1, org = 1, gg = 1, io = 1, co = 1, uk = 1, de = 1, fr = 1, eu = 1, tv = 1,
    info = 1, app = 1, dev = 1, xyz = 1, ru = 1, nl = 1, ly = 1, link = 1, wiki = 1, gl = 1,
}
C.LINK_COLOUR = "6fb6ff"

local function isUrl(tok)
    if strfind(tok, "|") then return false end
    if strfind(tok, "^%a[%w%+%.%-]*://[%w]") then return true end
    if strfind(tok, "^www%.[%w%-]+%.%a") then return true end
    local host = strmatch(tok, "^([%w%-%.]+)")
    if not host or not strfind(host, "%.") then return false end
    local tld = strmatch(host, "%.(%a+)$")
    if not tld or not TLD[strlower(tld)] then return false end
    local first = strmatch(host, "^([%w%-]+)%.")
    return first and strlen(first) >= 2 and strfind(first, "%a") and true or false
end
C.IsUrl = isUrl

function C:Linkify(text)
    if not strfind(text, "[%.:]") then return text end
    return plainMap(text, function(part)
        return (gsub(part, "(%S+)", function(tok)
            local body, trail = strmatch(tok, "^(.-)([%.,;:!%?%)]*)$")
            if body and body ~= "" and isUrl(body) then
                return "|cff" .. C.LINK_COLOUR .. "|Hurl:" .. body .. "|h[" .. body .. "]|h|r" .. (trail or "")
            end
            return tok
        end))
    end)
end

-- Your name in a line someone else wrote. Case-insensitive, whole words only.
local function nameSpots(text, name)
    local spots = {}
    local lt, ln = strlower(text), strlower(name)
    local pos = 1
    while true do
        local s, e = strfind(lt, ln, pos, true)
        if not s then break end
        local before = s > 1 and strsub(lt, s - 1, s - 1) or ""
        local after = strsub(lt, e + 1, e + 1)
        if not strfind(before, "[%w\128-\255]") and not strfind(after, "[%w\128-\255]") then
            table.insert(spots, { s, e })
        end
        pos = e + 1
    end
    return spots
end

function C:Mentions(msg)
    local me = UnitName and UnitName("player")
    if not me or me == "" or type(msg) ~= "string" then return false end
    return nameSpots(msg, me)[1] ~= nil
end

function C:Highlight(text)
    local me = UnitName("player")
    local hx = hex(1, 0.62, 0.18)
    return plainMap(text, function(part)
        local spots = nameSpots(part, me)
        if not spots[1] then return part end
        local out, pos = {}, 1
        for _, sp in ipairs(spots) do
            table.insert(out, strsub(part, pos, sp[1] - 1))
            table.insert(out, "|cff" .. hx .. strsub(part, sp[1], sp[2]) .. "|r")
            pos = sp[2] + 1
        end
        table.insert(out, strsub(part, pos))
        return table.concat(out)
    end)
end

-- Gold-seller adverts: a web address (www, .com, "dot com"...) together with a selling word.
function C:IsGoldSpam(msg)
    if type(msg) ~= "string" then return false end
    local m = strlower(msg)
    m = gsub(m, "|c%x%x%x%x%x%x%x%x", "")
    m = gsub(m, "[%s%*_,~'\"`]", "")
    m = gsub(m, "0", "o")
    m = gsub(m, "dot", ".")
    m = gsub(m, "%(%.%)", ".")
    m = gsub(m, "%[%.%]", ".")
    local site = strfind(m, "www") or strfind(m, "%.com") or strfind(m, "%.net") or strfind(m, "%.org")
        or strfind(m, "%.cn") or strfind(m, "%.ru") or strfind(m, "http")
    if not site then return false end
    local sell = strfind(m, "gold") or strfind(m, "usd") or strfind(m, "%$") or strfind(m, "eur")
        or strfind(m, "cheap") or strfind(m, "deliver") or strfind(m, "powerlevel") or strfind(m, "lvling")
        or strfind(m, "leveling") or strfind(m, "levelling") or strfind(m, "discount")
    return sell and true or false
end

-- -- Timestamps --------------------------------------------------------------------------------

function C:Stamp()
    local mode = self:Get("timestamps")
    if mode == "off" or not date then return nil end
    if mode == "12" then
        local h = tonumber(date("%I")) or 12
        local ap = (date("%p") == "PM") and "pm" or "am"
        return h .. date(":%M") .. ap
    end
    return date("%H:%M")
end

function C:StampText(text)
    local st = self:Stamp()
    if st then return "|cff" .. self:Theme().stamp .. st .. "|r " .. text end
    return text
end

-- -- Parchment ink: bright game colours turned into dark ink that reads on paper ----------------

function C.InkRGB(r, g, b)
    r, g, b = r or 1, g or 1, b or 1
    local w = math.min(r, math.min(g, b))
    local k = 0.44
    local lift = 0.24
    return math.max(0.03, r * k - w * lift), math.max(0.02, (g * k - w * lift) * 0.84), math.max(0.01, (b * k - w * lift) * 0.62)
end

function C.InkText(text)
    return (gsub(text, "|c(%x%x)(%x%x)(%x%x)(%x%x)", function(a, rr, gg, bb)
        local r, g, b = C.InkRGB(tonumber(rr, 16) / 255, tonumber(gg, 16) / 255, tonumber(bb, 16) / 255)
        return "|c" .. a .. hex(r, g, b)
    end))
end

-- -- The line pipeline -------------------------------------------------------------------------
-- A message event filter (run by the stock handler just before it prints) notes which chat event, author and words
-- the next line on that frame is for; each frame's AddMessage is wrapped once and stays wrapped, passing lines
-- through untouched while the addon is off, so other addons' hooks above or below it keep working.

local PLAYER_EVENTS = {
    CHAT_MSG_SAY = 1, CHAT_MSG_YELL = 1, CHAT_MSG_WHISPER = 1, CHAT_MSG_WHISPER_INFORM = 1,
    CHAT_MSG_CHANNEL = 1, CHAT_MSG_PARTY = 1, CHAT_MSG_PARTY_LEADER = 1, CHAT_MSG_RAID = 1, CHAT_MSG_RAID_LEADER = 1,
    CHAT_MSG_RAID_WARNING = 1, CHAT_MSG_GUILD = 1, CHAT_MSG_OFFICER = 1, CHAT_MSG_EMOTE = 1,
    CHAT_MSG_BATTLEGROUND = 1, CHAT_MSG_BATTLEGROUND_LEADER = 1,
}
C.PLAYER_EVENTS = PLAYER_EVENTS
local PUBLIC = { CHAT_MSG_SAY = 1, CHAT_MSG_YELL = 1, CHAT_MSG_WHISPER = 1, CHAT_MSG_CHANNEL = 1, CHAT_MSG_EMOTE = 1 }

C.MAX_BUFFER = 1000
C.filtered = 0
C.mentionAt = -100

function C.ContextFilter(frame, event, msg, author, _, _, _, flag)
    C.ctxEvent, C.ctxMsg, C.ctxAuthor, C.ctxFrame, C.ctxFlag = event, msg, author, frame, flag
    if C.active and PUBLIC[event] and C:Get("goldFilter") and type(msg) == "string"
        and author ~= (UnitName and UnitName("player")) and C:IsGoldSpam(msg) then
        if C.lastSpam ~= msg then C.lastSpam = msg; C.filtered = C.filtered + 1 end
        return true
    end
    return false
end

if ChatFrame_AddMessageEventFilter then
    for ev in pairs(PLAYER_EVENTS) do ChatFrame_AddMessageEventFilter(ev, C.ContextFilter) end
end

-- True when the frame shows its newest line (GetCurrentScroll counts lines up from the bottom).
function C.AtBottom(f)
    if f.GetCurrentScroll then
        local c = f:GetCurrentScroll()
        if c then return c == 0 end
    end
    if f.AtBottom then return f:AtBottom() and true or false end
    return true
end

local function remember(frame, entry)
    local lines = frame.lcLines
    if not lines then lines = {}; frame.lcLines = lines end
    table.insert(lines, entry)
    if #lines > C.MAX_BUFFER then table.remove(lines, 1) end
end

-- Print one buffered line (ink applied for the Parchment theme).
function C:Emit(frame, e)
    local add = frame.lcOrigAdd or frame.AddMessage
    if self.active and self.inked then
        local r, g, b = C.InkRGB(e.r, e.g, e.b)
        return add(frame, C.InkText(e.text), r, g, b, e.id)
    end
    return add(frame, e.text, e.r, e.g, e.b, e.id)
end

-- Print the buffer again (theme change, a collapsed repeat). Keeps the newest lines.
function C:Render(frame)
    if not frame.lcLines or not frame.Clear then return end
    frame:Clear()
    for _, e in ipairs(frame.lcLines) do self:Emit(frame, e) end
    self.renders = (self.renders or 0) + 1
end

-- Every chat frame: ChatFrame1..NUM_CHAT_WINDOWS and the whisper windows the game opened (CHAT_FRAMES).
function C.Frames()
    local out, seen = {}, {}
    for i = 1, (NUM_CHAT_WINDOWS or 10) do
        local f = _G["ChatFrame" .. i]
        if f and not seen[f] then seen[f] = true; table.insert(out, f) end
    end
    if type(CHAT_FRAMES) == "table" then
        for _, name in ipairs(CHAT_FRAMES) do
            local f = _G[name]
            if f and not seen[f] then seen[f] = true; table.insert(out, f) end
        end
    end
    return out
end

function C:RenderAll()
    for _, f in ipairs(C.Frames()) do
        if f.lcLines then self:Render(f) end
    end
    if self.RenderWorld then self:RenderWorld() end
end

function C:MentionsOn(frame)
    if not self:Get("mentions") then return false end
    local off = self:Settings().mentionOff
    return not (type(off) == "table" and frame and frame.GetName and off[frame:GetName() or ""])
end

local function isCombatLog(frame)
    if IsCombatLog then return C.Try(IsCombatLog, frame) and true or false end
    return COMBATLOG ~= nil and frame == COMBATLOG
end

function C:Add(frame, orig, msg, r, g, b, id, hold)
    if type(msg) ~= "string" or isCombatLog(frame) then return orig(frame, msg, r, g, b, id, hold) end
    if not self.active then
        remember(frame, { text = msg, r = r, g = g, b = b, id = id })
        return orig(frame, msg, r, g, b, id, hold)
    end
    local ev, author, raw, flag
    if self.ctxFrame == frame then
        ev, author, raw, flag = self.ctxEvent, self.ctxAuthor, self.ctxMsg, self.ctxFlag
        self.ctxFrame = nil                                    -- one line per event
        -- a stale note (an event this frame did not print) never colours an unrelated line
        if author and author ~= "" and not strfind(msg, author, 1, true) then ev, author, raw, flag = nil, nil, nil, nil end
    end
    local player = ev and PLAYER_EVENTS[ev]
    local me = UnitName and UnitName("player")
    local mine = author and author == me

    local text = msg
    local mention = false
    if player then
        if self:Get("channelNames") ~= "full" then text = self:ShortChannel(text) end
        if flag == "GM" and author and author ~= "" then text = self:GmTag(text, author) end
        if self:Get("classColors") and author and author ~= "" then text = self:ColourName(text, author) end
        if not mine and ev ~= "CHAT_MSG_WHISPER_INFORM" and self:MentionsOn(frame) and self:Mentions(raw) then
            text = self:Highlight(text)
            mention = true
        end
    end
    if self:Get("links") then text = self:Linkify(text) end

    -- collapse a line said again (same event, author and words) into "x2", "x3"
    if player and self:Get("collapse") then
        local key = ev .. "\1" .. (author or "") .. "\1" .. (raw or msg)
        local lines = frame.lcLines
        local last = lines and lines[#lines]
        if last and last.key == key and now() - (last.at or 0) < 120 then
            last.count = (last.count or 1) + 1
            last.at = now()
            last.text = last.base .. " |cff9aa0aa\195\151" .. last.count .. "|r"
            self.collapsed = (self.collapsed or 0) + 1
            if C.AtBottom(frame) then self:Render(frame) end
            self:Touch(frame, mention)
            return
        end
    end

    text = self:StampText(text)
    local entry = { text = text, base = text, r = r, g = g, b = b, id = id }
    if player then entry.key = ev .. "\1" .. (author or "") .. "\1" .. (raw or msg); entry.at = now() end
    remember(frame, entry)
    self:Emit(frame, entry)
    self:Touch(frame, mention)
end

-- A new line woke the window (fade timer) and marks its tab when you are on another one.
function C:Touch(frame, mention)
    self.lastActivity = now()
    if mention then
        frame.lcMention = true
        if self:Get("mentionSound") and now() - self.mentionAt > 4 then
            self.mentionAt = now()
            if PlaySound then C.Try(PlaySound, "MapPing") end
        end
    end
    if self.MarkUnread then self:MarkUnread(frame, mention) end
end

function C:HookFrame(f)
    if not f or f.lcOrigAdd or not f.AddMessage then return end
    local orig = f.AddMessage
    f.lcOrigAdd = orig
    f.AddMessage = function(self, msg, r, g, b, id, hold)
        return C:Add(self, orig, msg, r, g, b, id, hold)
    end
end

function C:HookFrames()
    for _, f in ipairs(C.Frames()) do self:HookFrame(f) end
end

C:HookFrames()

-- -- The older options: wheel, arrows, sticky channels, long history, log-on alerts ------------

function C.OnWheel(self, delta)
    if (delta or 0) > 0 then
        if IsShiftKeyDown and IsShiftKeyDown() and self.ScrollToTop then self:ScrollToTop() elseif self.ScrollUp then self:ScrollUp() end
    else
        if IsShiftKeyDown and IsShiftKeyDown() and self.ScrollToBottom then self:ScrollToBottom() elseif self.ScrollDown then self:ScrollDown() end
    end
    C.lastActivity = now()
end

-- The edit boxes: each chat frame's own (ChatFrame1EditBox ...); ChatFrameEditBox on a client with one.
function C.EditBoxes()
    local out, seen = {}, {}
    for _, f in ipairs(C.Frames()) do
        local eb = f.editBox or _G[(f:GetName() or "") .. "EditBox"]
        if eb and not seen[eb] then seen[eb] = true; table.insert(out, eb) end
    end
    if ChatFrameEditBox and not seen[ChatFrameEditBox] then table.insert(out, ChatFrameEditBox) end
    return out
end

-- The edit box Enter opens (classic style: the main window's).
function C.MainEditBox()
    local main = DEFAULT_CHAT_FRAME or ChatFrame1
    return (main and (main.editBox or _G[(main:GetName() or "") .. "EditBox"])) or ChatFrameEditBox
end

function C.ActiveEditBox()
    local eb = ChatEdit_GetActiveWindow and C.Try(ChatEdit_GetActiveWindow)
    return eb or ACTIVE_CHAT_EDIT_BOX or C.MainEditBox()
end

-- The edit box's chat type and target (3.3.5 keeps them as attributes; older clients as fields).
function C.EditAttr(eb, key)
    if eb.GetAttribute then
        local v = eb:GetAttribute(key)
        if v ~= nil then return v end
    end
    return eb[key]
end
function C.SetEditAttr(eb, key, value)
    if eb.SetAttribute then eb:SetAttribute(key, value) end
    eb[key] = value
end

local STICKY = { "WHISPER", "OFFICER", "RAID", "CHANNEL", "PARTY", "GUILD" }
local stickyStock = {}

function C:ApplyBehaviour()
    local on = self.active
    for _, f in ipairs(C.Frames()) do
        if f.lcWheelOrig == nil then f.lcWheelOrig = f.GetScript and f:GetScript("OnMouseWheel") or false end
        if on and self:Get("wheel") then
            if f.EnableMouseWheel then f:EnableMouseWheel(true) end
            f:SetScript("OnMouseWheel", C.OnWheel)
        elseif f.lcWheelOrig ~= nil then
            f:SetScript("OnMouseWheel", f.lcWheelOrig or nil)
        end
        local want = (on and self:Get("history")) and 1000 or nil
        if want and f.lcMaxLines ~= want and f.SetMaxLines then
            f.lcStockMax = f.lcStockMax or (f.GetMaxLines and f:GetMaxLines()) or 128
            f:SetMaxLines(want)
            f.lcMaxLines = want
        elseif not want and f.lcMaxLines and f.SetMaxLines then
            f:SetMaxLines(f.lcStockMax or 128)
            f.lcMaxLines = nil
        end
    end
    for _, eb in ipairs(C.EditBoxes()) do
        if eb.SetAltArrowKeyMode then eb:SetAltArrowKeyMode(not (on and self:Get("arrows"))) end
    end
    if ChatTypeInfo then
        for _, t in ipairs(STICKY) do
            local info = ChatTypeInfo[t]
            if info then
                if stickyStock[t] == nil then stickyStock[t] = info.sticky or 0 end
                info.sticky = (on and self:Get("sticky")) and 1 or stickyStock[t]
            end
        end
    end
    if on and SetCVar then C.Try(SetCVar, "guildMemberNotify", self:Get("alerts") and "1" or "0") end
    self:StockStamps()
end

-- The game's own timestamps (Interface options) stand down while ours are on, so a line never gets two.
function C:StockStamps()
    local ours = self.active and self:Get("timestamps") ~= "off"
    if ours then
        if CHAT_TIMESTAMP_FORMAT then
            self.stockStampFormat = CHAT_TIMESTAMP_FORMAT
            CHAT_TIMESTAMP_FORMAT = nil
        end
    elseif self.stockStampFormat then
        if CHAT_TIMESTAMP_FORMAT == nil then CHAT_TIMESTAMP_FORMAT = self.stockStampFormat end
        self.stockStampFormat = nil
    end
end

function C:OpenSettings()
    if self.ToggleSettings then self:ToggleSettings(true) end
end

-- -- The chat tab's right-click menu ----------------------------------------------------------

local function addRows(level)
    if not UIDropDownMenu_AddButton then return end
    if (level or UIDROPDOWNMENU_MENU_LEVEL or 1) ~= 1 then return end
    UIDropDownMenu_AddButton({ text = C.NAME, isTitle = 1, notCheckable = 1 }, 1)
    if not C.active then
        UIDropDownMenu_AddButton({ text = "Turn on " .. C.NAME, notCheckable = 1, func = function() C:SetOption("enabled", true) end }, 1)
        return
    end
    UIDropDownMenu_AddButton({ text = "Show the World tab", checked = C:Get("worldTab") and 1 or nil, keepShownOnClick = 1,
        func = function() C:SetOption("worldTab", not C:Get("worldTab")) end }, 1)
    for _, o in ipairs(C.OPTIONS) do
        local opt = o
        UIDropDownMenu_AddButton({ text = opt.label, checked = C:On(opt.key) and 1 or nil, keepShownOnClick = 1,
            func = function() C:Set(opt.key, not C:On(opt.key)) end }, 1)
    end
    UIDropDownMenu_AddButton({ text = "All chat settings...", notCheckable = 1, func = function() C:OpenSettings() end }, 1)
end
C.AddMenuRows = addRows

local function hookMenu()
    if C.menuHooked then return end
    C.menuHooked = C.Hook("FCFOptionsDropDown_Initialize", function(dropDown, level) addRows(level) end)
end

-- -- /lchat --------------------------------------------------------------------------------------

function C:Status()
    local parts = { (self:Get("enabled") and "on" or "off"), "world tab " .. (self:Get("worldTab") and "on" or "off") }
    for _, o in ipairs(self.OPTIONS) do table.insert(parts, o.key .. " " .. (self:On(o.key) and "on" or "off")) end
    return "Chat: " .. table.concat(parts, ", ") .. ". /lchat opens the settings; /lchat <name> toggles one; "
        .. "/lchat theme <name>, /lchat copy, /lchat expand, /lchat world, /lchat on|off."
end

SLASH_LEYLINECHAT1 = "/lchat"
SLASH_LEYLINECHAT2 = "/leylinechat"
SlashCmdList = SlashCmdList or {}
SlashCmdList["LEYLINECHAT"] = function(msg)
    msg = strlower(C.Trim(msg or ""))
    local verb, rest = strmatch(msg, "^(%S+)%s*(.*)$")
    if msg == "" or msg == "settings" or msg == "options" or msg == "config" then
        C:OpenSettings()
        return
    elseif msg == "on" or msg == "off" then
        C:SetOption("enabled", msg == "on")
        C.Print(C.NAME .. " is " .. msg .. ".")
        return
    elseif msg == "world" then
        C:SetOption("worldTab", not C:Get("worldTab"))
    elseif verb == "theme" then
        if C.THEMES[rest] then C:SetOption("theme", rest) end
        C.Print("Chat theme: " .. C:Theme().label .. ". Themes: slate, parchment, midnight, wildwood, glass.")
        return
    elseif msg == "expand" then
        if C.OpenExpanded then C:OpenExpanded() end
        return
    elseif msg == "copy" then
        if C.OpenCopy then C:OpenCopy() end
        return
    elseif msg ~= "help" and msg ~= "status" then
        for _, o in ipairs(C.OPTIONS) do
            if o.key == msg then C:Set(o.key, not C:On(o.key)) end
        end
    end
    C.Print(C:Status())
end

-- -- Apply and events ------------------------------------------------------------------------------

function C:Refresh(active)
    local wasInked = self.inked
    self.active = active and true or false
    local theme = self:Theme()
    self.inked = self.active and theme.ink and true or false
    self:ApplyBehaviour()
    if self.ApplySkin then self:ApplySkin() end
    if self.ApplyTools then self:ApplyTools() end
    if self.ApplyWorld then self:ApplyWorld() end
    if (wasInked or false) ~= self.inked then self:RenderAll() end
end

local function learnRoster(getNum, getInfo, nameAt, classAt, tokenAt)
    if not getNum or not getInfo then return end
    for i = 1, (getNum() or 0) do
        local info = { getInfo(i) }
        C:Learn(info[nameAt], info[tokenAt or 0] or info[classAt])
    end
end

local ev = CreateFrame("Frame", "LeylineChatEvents")
ev:RegisterEvent("ADDON_LOADED")
ev:RegisterEvent("PLAYER_ENTERING_WORLD")
ev:RegisterEvent("PLAYER_TARGET_CHANGED")
ev:RegisterEvent("UPDATE_MOUSEOVER_UNIT")
ev:RegisterEvent("PARTY_MEMBERS_CHANGED")
ev:RegisterEvent("RAID_ROSTER_UPDATE")
ev:RegisterEvent("GUILD_ROSTER_UPDATE")
ev:RegisterEvent("FRIENDLIST_UPDATE")
ev:RegisterEvent("WHO_LIST_UPDATE")
ev:SetScript("OnEvent", function(self, event, addon)
    if event == "ADDON_LOADED" then
        if addon == ID then C:DB(); C:CharDB(); C.loaded = true end
    elseif event == "PLAYER_ENTERING_WORLD" then
        C:DB()
        hookMenu()
        C:HookFrames()
        C:LearnUnit("player")
        C.entered = true
        C:Refresh(C:Get("enabled"))
    elseif event == "PLAYER_TARGET_CHANGED" then
        C:LearnUnit("target")
    elseif event == "UPDATE_MOUSEOVER_UNIT" then
        C:LearnUnit("mouseover")
    elseif event == "PARTY_MEMBERS_CHANGED" then
        for i = 1, 4 do C:LearnUnit("party" .. i) end
    elseif event == "RAID_ROSTER_UPDATE" then
        for i = 1, 40 do C:LearnUnit("raid" .. i) end
    elseif event == "GUILD_ROSTER_UPDATE" then
        -- name, rank, rankIndex, level, class, zone, note, officernote, online, status, classFileName
        learnRoster(GetNumGuildMembers and function() return GetNumGuildMembers(true) end, GetGuildRosterInfo, 1, 5, 11)
    elseif event == "FRIENDLIST_UPDATE" then
        learnRoster(GetNumFriends, GetFriendInfo, 1, 3)
    elseif event == "WHO_LIST_UPDATE" then
        -- name, guild, level, race, class, zone, classFileName
        learnRoster(GetNumWhoResults, GetWhoInfo, 1, 5, 7)
    end
end)
C.eventFrame = ev
