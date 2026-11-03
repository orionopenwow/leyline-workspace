-- Leyline name guard (owner 2026-10-31: "Bad actor measures, username blocks, in game name blocks"). mod-ale.
--
-- The same blocklist as the website's names (stream/blocklist.json, stream/namecheck.py; the table below is written by
-- `python3 stream/namecheck.py --lua realm/custom/lua_scripts/ov_names.lua`), plus the owner's own words from Leyline
-- HQ (the scripts step copies them into characters.ov_blocked_words). The world server already refuses the whole
-- names in reserved_name and profanity_name at character create (the scripts step fills them; read at a restart);
-- this script catches the rest (look-alikes, digits for letters, run-together words) without deleting anything:
--   * a character whose name trips the list is flagged (characters.ov_name_flags) when it is made and when it logs in.
--     Slurs, sexual terms, profanity and the owner's words: the character is asked for a new name at its next login
--     (the same flag as the GM command .character rename). Names that pass for the team (Admin, GM...): flagged for a
--     Game Master, who renames from Leyline HQ (Needs you > Force a rename) or in game.
--   * a guild whose name trips it is flagged for a Game Master (nothing is renamed or disbanded).
--   * public chat (say, yell, emote, chat channels): the list's words are starred out. Nothing is muted; whispers,
--     party, raid and guild chat are left alone. Item and spell links are kept whole.
-- Console / SOAP (Leyline HQ, stream/hq_words.py):
--   ovnames list [open|all] [n]     -> "ROW \t what \t id \t name \t kind \t at \t status" lines
--   ovnames rename <guid>           the character picks a new name at its next login (status "renamed")
--   ovnames done <char|guild> <id>  handled, kept for the record (status "done")
--   ovnames scan [from-guid]        flags the characters already made (500 at a time; "ovadmin next <guid>")
--   ovnames check <text>            -> "ovadmin name <kind|ok>"
--   ovnames reload                  reads characters.ov_blocked_words again
-- Every answer ends with "ovadmin end <rows>" or starts with "ovadmin error: ..." (the shape ov_admin.lua uses).
-- Settings: OV_NAMES=0 turns it all off; OV_NAMES_CHAT=0 leaves chat alone.

local N = {}
OVNames = N

local function env(k, d) local v = os.getenv and os.getenv(k); if v == nil or v == "" then return d end; return v end
local ON = env("OV_NAMES", "1") ~= "0"
local CHAT = env("OV_NAMES_CHAT", "1") ~= "0"
local AT_LOGIN_RENAME = 0x01
local AUTO = { slur = true, sexual = true, profanity = true, custom = true, brand = true }

-- >>> blocklist (generated: python3 stream/namecheck.py --lua <this file>; do not edit by hand)
local LISTS = {
    slur = { inside = { "nigger", "nigga", "faggot", "wetback", "raghead", "towelhead", "beaner", "hitler", "kkk" },
        word = { "fag", "fags", "kike", "spic", "chink", "gook", "coon", "dyke", "tranny", "retard", "nazi", "kys", "paki", "jap", "wop", "zipperhead", "jigaboo", "porchmonkey", "sambo" } },
    sexual = { inside = { "porn", "dildo", "vagina", "rapist", "molest", "pedophile", "paedophile", "incest", "blowjob", "handjob", "cumshot", "jizz", "hentai", "masturbat", "orgasm", "bestiality" },
        word = { "rape", "raped", "raping", "penis", "cock", "dick", "pussy", "cum", "sex", "sexy", "anal", "anus", "tits", "titty", "titties", "boob", "boobs", "slut", "whore", "hooker", "pedo", "paedo", "nude", "nudes", "milf", "horny", "clit", "scrotum", "testicle", "semen", "boner", "nsfw", "xxx", "onlyfans" } },
    profanity = { inside = { "fuck", "cunt", "motherf", "bitch" },
        word = { "shit", "shitty", "bullshit", "shithead", "shitface", "shitlord", "dipshit", "horseshit", "twat", "wank", "wanker", "bastard", "prick", "asshole", "arsehole" } },
    staff = { edge = { "admin", "administrator", "moderator", "gamemaster", "leyline", "openvanilla", "sysadmin", "sysop", "developer", "customerservice", "helpdesk" },
        word = { "gm", "gms", "mod", "mods", "dev", "devs", "staff", "owner", "official", "support", "admins", "moderators", "employee" } },
    allow = { "scunthorpe", "penistone", "shiitake", "shitake", "therapist", "therapists", "snigger", "sniggers", "sniggering", "niggard", "niggardly", "bitchute", "cocktail", "cockpit", "peacock", "hancock", "cockburn", "dickens", "analyst", "analysis", "canal", "sussex", "essex", "middlesex", "grape", "drape", "scrape", "cassandra", "assassin", "classic", "spice", "spicy", "chinking", "racoon", "raccoon", "cocoon", "tycoon", "nazir", "japan", "japanese", "document", "cumulus", "cucumber", "circumstance", "pakistan", "badminton", "manuscript", "cocker", "dicker", "toshitaka" },
}
-- <<< blocklist

-- ------------------------------------------------------------------------------------------- normalising
-- UTF-8 letters that pass for Latin ones (accents off, Cyrillic and Greek look-alikes), and invisible characters out.
local FOLD = {
    ["\195\128"] = "A", ["\195\129"] = "A", ["\195\130"] = "A", ["\195\131"] = "A", ["\195\132"] = "A", ["\195\133"] = "A",
    ["\195\134"] = "AE", ["\195\135"] = "C", ["\195\136"] = "E", ["\195\137"] = "E", ["\195\138"] = "E", ["\195\139"] = "E",
    ["\195\140"] = "I", ["\195\141"] = "I", ["\195\142"] = "I", ["\195\143"] = "I", ["\195\144"] = "D", ["\195\145"] = "N",
    ["\195\146"] = "O", ["\195\147"] = "O", ["\195\148"] = "O", ["\195\149"] = "O", ["\195\150"] = "O", ["\195\152"] = "O",
    ["\195\153"] = "U", ["\195\154"] = "U", ["\195\155"] = "U", ["\195\156"] = "U", ["\195\157"] = "Y", ["\195\158"] = "Th",
    ["\195\159"] = "ss", ["\195\160"] = "a", ["\195\161"] = "a", ["\195\162"] = "a", ["\195\163"] = "a", ["\195\164"] = "a",
    ["\195\165"] = "a", ["\195\166"] = "ae", ["\195\167"] = "c", ["\195\168"] = "e", ["\195\169"] = "e", ["\195\170"] = "e",
    ["\195\171"] = "e", ["\195\172"] = "i", ["\195\173"] = "i", ["\195\174"] = "i", ["\195\175"] = "i", ["\195\176"] = "d",
    ["\195\177"] = "n", ["\195\178"] = "o", ["\195\179"] = "o", ["\195\180"] = "o", ["\195\181"] = "o", ["\195\182"] = "o",
    ["\195\184"] = "o", ["\195\185"] = "u", ["\195\186"] = "u", ["\195\187"] = "u", ["\195\188"] = "u", ["\195\189"] = "y",
    ["\195\190"] = "th", ["\195\191"] = "y", ["\196\177"] = "i", ["\197\130"] = "l", ["\197\129"] = "L", ["\197\147"] = "oe",
    ["\197\146"] = "OE",
    -- Cyrillic
    ["\208\144"] = "A", ["\208\176"] = "a", ["\208\146"] = "B", ["\208\178"] = "b", ["\208\149"] = "E", ["\208\181"] = "e",
    ["\208\154"] = "K", ["\208\186"] = "k", ["\208\156"] = "M", ["\208\188"] = "m", ["\208\157"] = "H", ["\208\189"] = "h",
    ["\208\158"] = "O", ["\208\190"] = "o", ["\208\160"] = "P", ["\209\128"] = "p", ["\208\161"] = "C", ["\209\129"] = "c",
    ["\208\162"] = "T", ["\209\130"] = "t", ["\208\163"] = "Y", ["\209\131"] = "y", ["\208\165"] = "X", ["\209\133"] = "x",
    ["\208\134"] = "I", ["\209\150"] = "i", ["\208\136"] = "J", ["\209\152"] = "j", ["\208\133"] = "S", ["\209\149"] = "s",
    ["\208\172"] = "b", ["\209\140"] = "b",
    -- Greek
    ["\206\145"] = "A", ["\206\177"] = "a", ["\206\146"] = "B", ["\206\149"] = "E", ["\206\181"] = "e", ["\206\150"] = "Z",
    ["\206\151"] = "H", ["\206\153"] = "I", ["\206\185"] = "i", ["\206\154"] = "K", ["\206\186"] = "k", ["\206\156"] = "M",
    ["\206\157"] = "N", ["\206\189"] = "v", ["\206\159"] = "O", ["\206\191"] = "o", ["\206\161"] = "P", ["\207\129"] = "p",
    ["\206\164"] = "T", ["\207\132"] = "t", ["\206\165"] = "Y", ["\207\133"] = "u", ["\206\167"] = "X", ["\207\135"] = "x",
    ["\207\137"] = "w",
}
local LEET = { ["0"] = "o", ["1"] = "i", ["3"] = "e", ["4"] = "a", ["5"] = "s", ["7"] = "t", ["8"] = "b", ["9"] = "g",
    ["@"] = "a", ["$"] = "s", ["!"] = "i", ["|"] = "l" }

local function stripInvisible(s)
    s = s:gsub("\194\173", "")                          -- soft hyphen
    s = s:gsub("\226\128[\139-\143\170-\174]", "")      -- zero-width, joiners, bidi overrides
    s = s:gsub("\226\129[\160-\175]", "")               -- word joiner, invisible operators, isolates
    s = s:gsub("\239\187\191", "")                      -- BOM
    s = s:gsub("\239\184[\128-\143]", "")               -- variation selectors
    return s
end
N.StripInvisible = stripInvisible

local function fold(s)   -- keeps case (for run-together words); any other non-ASCII byte becomes a separator
    s = stripInvisible(tostring(s or ""))
    -- full-width forms (U+FF01-FF5E) to ASCII
    s = s:gsub("\239\188([\129-\191])", function(c) return string.char(c:byte() - 0x60) end)
    s = s:gsub("\239\189([\128-\158])", function(c) return string.char(c:byte() - 0x20) end)
    s = s:gsub("[\192-\223][\128-\191]", function(c) return FOLD[c] or " " end)
    s = s:gsub("[\224-\247][\128-\191]+", " ")
    return s
end
local function leet(s, oneIsL)
    return (s:gsub("[01345789@%$!|]", function(c)
        if c == "1" and oneIsL then return "l" end
        return LEET[c] or c
    end))
end

local function pat(w)   -- "shit" -> "s+h+i+t+"; "nigger" -> "n+i+ggg*e+r+" (a double letter stays at least double)
    local out, i = {}, 1
    while i <= #w do
        local c = w:sub(i, i)
        local j = i
        while w:sub(j + 1, j + 1) == c do j = j + 1 end
        local n = j - i + 1
        out[#out + 1] = (n == 1) and (c .. "+") or (string.rep(c, n) .. c .. "*")
        i = j + 1
    end
    return table.concat(out)
end
local SUFFIX = { "", "s", "es", "z", "ed", "er", "ers" }
local ALLOW_SUFFIX = { "", "s", "es", "d", "ed", "r", "er", "ers", "y", "ing", "ings" }
local function fullWith(t, p, sufs)
    for _, s in ipairs(sufs) do if t:match("^" .. p .. s .. "$") then return true end end
    return false
end

-- ------------------------------------------------------------------------------------------- the lists
local entries, staffEdge, staffWord, allowSet, allowPats = {}, {}, {}, {}, {}
local function addEntry(kind, w, mode)
    if type(w) ~= "string" or not w:match("^%l%l+$") or #w > 24 then return end
    entries[#entries + 1] = { kind = kind, w = w, mode = mode, p = pat(w) }
end
local function build(extra)
    entries, staffEdge, staffWord, allowSet, allowPats = {}, {}, {}, {}, {}
    for _, kind in ipairs({ "slur", "sexual", "profanity" }) do
        local sec = LISTS[kind] or {}
        for _, w in ipairs(sec.word or {}) do addEntry(kind, w, "word") end
        for _, w in ipairs(sec.inside or {}) do addEntry(kind, w, "inside") end
    end
    for _, w in ipairs(LISTS.staff.edge or {}) do staffEdge[#staffEdge + 1] = { w = w, p = pat(w) } end
    for _, w in ipairs(LISTS.staff.word or {}) do staffWord[#staffWord + 1] = { w = w, p = pat(w) } end
    for _, a in ipairs(LISTS.allow or {}) do allowSet[a] = true; allowPats[#allowPats + 1] = pat(a) end
    table.sort(allowPats, function(a, b) return #a > #b end)
    for _, x in ipairs(extra or {}) do addEntry("custom", x.w, x.mode == "inside" and "inside" or "word") end
end
N.Build = build

local function allowed(t)
    if allowSet[t] then return true end
    for _, p in ipairs(allowPats) do if fullWith(t, p, ALLOW_SUFFIX) then return true end end
    return false
end
local function unallowed(t)
    for _, p in ipairs(allowPats) do t = t:gsub(p, "-") end
    return t
end

local function tokenHit(t, kinds)
    if not t or t == "" or allowed(t) then return nil end
    local inner
    for _, e in ipairs(entries) do
        if not kinds or kinds[e.kind] then
            if fullWith(t, e.p, SUFFIX) then return e.kind, e.w end
            if e.mode == "inside" then
                inner = inner or unallowed(t)
                if inner:find(e.p) then return e.kind, e.w end
            end
        end
    end
    return nil
end

local function tokens(text)
    local cased = fold(text)
    local seen, toks = {}, {}
    local function add(t) if t ~= "" and not seen[t] then seen[t] = true; toks[#toks + 1] = t end end
    local words, joined = {}, {}
    local plain = leet(cased:lower())
    for w in plain:gmatch("%a+") do words[#words + 1] = w; joined[#joined + 1] = w; add(w) end
    local camel = cased:gsub("(%l)(%u)", "%1 %2"):gsub("(%u+)(%u%l)", "%1 %2")
    for w in leet(camel):lower():gmatch("%a+") do add(w) end
    local run = {}
    for i = 1, #words + 1 do                      -- runs of one-letter words: "f u c k"
        local w = words[i]
        if w and #w == 1 then run[#run + 1] = w
        else
            if #run >= 2 then add(table.concat(run)) end
            run = {}
        end
    end
    for chunk in plain:gmatch("%S+") do add((chunk:gsub("%A", ""))) end   -- "f.u.c.k", "sh-it"
    if cased:find("1", 1, true) then
        local alt = leet(cased:lower(), true)
        for w in alt:gmatch("%a+") do add(w) end
        for chunk in alt:gmatch("%S+") do add((chunk:gsub("%A", ""))) end
    end
    return toks, table.concat(joined)
end

local BRANDS = { "worldofwarcraft", "warcraft", "lichking", "blizzard" }

local function staffHit(original, toks, joined)
    local cased = fold(original):gsub("%A", "")
    if cased:match("^GM%u") or cased == "GM" or cased:match("^GM$") or cased:match("GM$") or cased:match("^Gm%u") then
        return "staff", "gm"
    end
    for _, e in ipairs(staffEdge) do
        if joined:match("^" .. e.p) or joined:match(e.p .. "$") then return "staff", e.w end
        for _, t in ipairs(toks) do
            if t:match("^" .. e.p) or t:match(e.p .. "$") then return "staff", e.w end
        end
    end
    for _, e in ipairs(staffWord) do
        if fullWith(joined, e.p, { "", "s" }) then return "staff", e.w end
        for _, t in ipairs(toks) do if fullWith(t, e.p, { "", "s" }) then return "staff", e.w end end
    end
    return nil
end

-- (kind, word) for a character or guild name, else nil
function N.NameHit(name)
    if type(name) ~= "string" or name == "" then return nil end
    local toks, joined = tokens(name)
    if fold(name):find("WoW", 1, true) then return "brand", "brand" end
    for _, t in ipairs(toks) do if t == "wow" then return "brand", "brand" end end
    for _, b in ipairs(BRANDS) do if joined:find(b, 1, true) then return "brand", "brand" end end
    for _, t in ipairs(toks) do
        local k, w = tokenHit(t)
        if k then return k, w end
    end
    if joined ~= "" then
        local k, w = tokenHit(joined)
        if k then return k, w end
    end
    return staffHit(name, toks, joined)
end

-- the text with the list's words starred out, the same length (item and spell links kept whole)
local function maskPlain(text)
    local out = text:gsub("%S+", function(run)
        local toks = tokens(run)
        for _, t in ipairs(toks) do if tokenHit(t) then return string.rep("*", #run) end end
        return run
    end)
    -- spaced out over several runs: "f u c k"
    local pieces = {}
    for ws, run in out:gmatch("(%s*)(%S+)") do pieces[#pieces + 1] = { ws = ws, run = run } end
    local i = 1
    while i <= #pieces do
        local j = i
        while j <= #pieces and #pieces[j].run == 1 do j = j + 1 end
        if j - i >= 3 then
            local letters = {}
            for k = i, j - 1 do letters[#letters + 1] = pieces[k].run end
            local joined = leet(fold(table.concat(letters)):lower()):gsub("%A", "")
            if tokenHit(joined) then for k = i, j - 1 do pieces[k].run = "*" end end
            i = j
        else
            i = i + 1
        end
    end
    local b = {}
    for _, p in ipairs(pieces) do b[#b + 1] = p.ws .. p.run end
    local tail = out:match("%s*$") or ""
    return table.concat(b) .. tail
end

function N.Mask(text)
    if type(text) ~= "string" or text == "" then return text end
    if not text:find("|H", 1, true) then return maskPlain(text) end
    local out, at = {}, 1
    while true do
        local s, e = text:find("|c%x%x%x%x%x%x%x%x|H.-|h.-|h|r", at)
        if not s then break end
        out[#out + 1] = maskPlain(text:sub(at, s - 1))
        out[#out + 1] = text:sub(s, e)
        at = e + 1
    end
    out[#out + 1] = maskPlain(text:sub(at))
    return table.concat(out)
end

-- ------------------------------------------------------------------------------------------- storage
local function charQuery(sql)
    if not CharDBQuery then return nil end
    local ok, q = pcall(CharDBQuery, sql)
    if ok then return q end
end
local function charExec(sql) if CharDBExecute then pcall(CharDBExecute, sql) end end
local function flat(s) return (tostring(s or ""):gsub("[%c|\\'\"]", " ")) end

charExec([[CREATE TABLE IF NOT EXISTS ov_blocked_words (
  word VARCHAR(24) NOT NULL PRIMARY KEY, mode VARCHAR(8) NOT NULL DEFAULT 'word')]])
charExec([[CREATE TABLE IF NOT EXISTS ov_name_flags (
  what VARCHAR(6) NOT NULL, id INT UNSIGNED NOT NULL, name VARCHAR(24) NOT NULL, kind VARCHAR(12) NOT NULL,
  at INT UNSIGNED NOT NULL, status VARCHAR(8) NOT NULL DEFAULT 'open', PRIMARY KEY (what, id), KEY status (status))]])

function N.Reload()
    local extra = {}
    local q = charQuery("SELECT word, mode FROM ov_blocked_words LIMIT 1000")
    if q then
        repeat
            local w, m = q:GetString(0), q:GetString(1)
            if w:match("^%l%l+$") and #w <= 24 then extra[#extra + 1] = { w = w, mode = m } end
        until not q:NextRow()
    end
    build(extra)
    return #extra
end
N.Reload()

local function flag(what, id, name, kind)
    id = tonumber(id)
    if not id or id <= 0 then return end
    charExec(string.format("INSERT INTO ov_name_flags (what, id, name, kind, at, status) VALUES ('%s', %d, '%s', '%s', %d, 'open') "
        .. "ON DUPLICATE KEY UPDATE name = VALUES(name), kind = VALUES(kind), "
        .. "status = IF(status = 'open' OR name <> VALUES(name), 'open', status)",
        what, id, flat(name):sub(1, 24), kind, os.time()))
end
N.Flag = flag

local function say(player, text)
    pcall(function() player:SendBroadcastMessage("|cff66ccff[Leyline]|r " .. text) end)
end
local function try(o, m, ...)
    if not o or type(o[m]) ~= "function" then return nil end
    local ok, v = pcall(o[m], o, ...)
    if ok then return v end
end

local RENAME_TEXT = "This character's name isn't allowed here. Log out to the character list: you'll pick a new name "
    .. "there. Nothing else about the character changes."

local function setRename(guid, player)
    if player then
        try(player, "SetAtLoginFlag", AT_LOGIN_RENAME)
        try(player, "SaveToDB")
    end
    charExec(string.format("UPDATE characters SET at_login = at_login | %d WHERE guid = %d", AT_LOGIN_RENAME, guid))
end

-- ------------------------------------------------------------------------------------------- events
local function isStaff(player) return (tonumber(try(player, "GetGMRank")) or 0) > 0 end

local function onCreate(event, player)
    if not ON or not player then return end
    local name = try(player, "GetName")
    local kind = N.NameHit(name)
    if not kind or (kind == "staff" and isStaff(player)) then return end
    local guid = tonumber(try(player, "GetGUIDLow")) or 0
    flag("char", guid, name, kind)
    if AUTO[kind] then
        try(player, "SetAtLoginFlag", AT_LOGIN_RENAME)
        -- the character row is written after this event: set the flag in the database a moment later too
        if CreateLuaEvent then CreateLuaEvent(function() setRename(guid) end, 3000, 1) end
    end
end

local function onLogin(event, player)
    if not ON or not player then return end
    local name = try(player, "GetName")
    local kind = N.NameHit(name)
    if not kind or (kind == "staff" and isStaff(player)) then return end
    local guid = tonumber(try(player, "GetGUIDLow")) or 0
    flag("char", guid, name, kind)
    if AUTO[kind] then
        setRename(guid, player)
        say(player, RENAME_TEXT)
    end
end

local function onGuildCreate(event, guild, leader, name)
    if not ON then return end
    local kind = N.NameHit(name)
    if not kind then return end
    flag("guild", try(guild, "GetId"), name, kind)
    if leader then say(leader, "A Game Master will look at this guild's name: please pick another one if they ask.") end
end

local function chat(msg)
    if not ON or not CHAT or type(msg) ~= "string" then return nil end
    local m = N.Mask(msg)
    if m ~= msg then return nil, m end   -- mod-ale: the second return value replaces the line
    return nil
end
local function onChat(event, player, msg, msgType, lang) return chat(msg) end
local function onChannelChat(event, player, msg, msgType, lang, channel) return chat(msg) end

-- ------------------------------------------------------------------------------------------- console
local STATES = { open = true, all = true }

function N.Console(command, out)
    local args = {}
    for w in tostring(command or ""):gmatch("%S+") do args[#args + 1] = w end
    local sub = args[2]
    if sub == "list" then
        local state = args[3] or "open"
        if not STATES[state] then return out("ovadmin error: state is open or all") end
        local n = math.max(1, math.min(200, math.floor(tonumber(args[4]) or 50)))
        local q = charQuery(string.format("SELECT what, id, name, kind, at, status FROM ov_name_flags %s ORDER BY at DESC LIMIT %d",
            state == "open" and "WHERE status = 'open'" or "", n))
        local rows = 0
        if q then
            repeat
                rows = rows + 1
                out(table.concat({ "ROW", q:GetString(0), q:GetString(1), flat(q:GetString(2)), q:GetString(3),
                    q:GetString(4), q:GetString(5) }, "\t"))
            until not q:NextRow()
        end
        return out("ovadmin end " .. rows)
    elseif sub == "rename" then
        local guid = tonumber(args[3])
        if not guid or guid <= 0 or guid ~= math.floor(guid) then return out("ovadmin error: usage: ovnames rename <guid>") end
        local q = charQuery(string.format("SELECT name FROM characters WHERE guid = %d", guid))
        if not q then return out("ovadmin error: no character with that guid") end
        local name = q:GetString(0)
        local p = GetPlayerByName and name ~= "" and GetPlayerByName(name) or nil
        setRename(guid, p)
        if p then say(p, RENAME_TEXT) end
        charExec(string.format("INSERT INTO ov_name_flags (what, id, name, kind, at, status) VALUES ('char', %d, '%s', 'gm', %d, 'renamed') "
            .. "ON DUPLICATE KEY UPDATE status = 'renamed'", guid, flat(name):sub(1, 24), os.time()))
        out(string.format("ovadmin ok: %s renames at the next login", flat(name)))
        return out("ovadmin end 0")
    elseif sub == "done" then
        local what, id = args[3], tonumber(args[4])
        if (what ~= "char" and what ~= "guild") or not id or id <= 0 or id ~= math.floor(id) then
            return out("ovadmin error: usage: ovnames done <char|guild> <id>")
        end
        charExec(string.format("UPDATE ov_name_flags SET status = 'done' WHERE what = '%s' AND id = %d", what, id))
        out("ovadmin ok: done")
        return out("ovadmin end 0")
    elseif sub == "scan" then
        local from = math.max(0, math.floor(tonumber(args[3]) or 0))
        local q = charQuery(string.format("SELECT guid, name FROM characters WHERE guid > %d ORDER BY guid LIMIT 500", from))
        local last, found = from, 0
        if q then
            repeat
                local guid, name = tonumber(q:GetString(0)) or 0, q:GetString(1)
                last = math.max(last, guid)
                local kind = N.NameHit(name)
                if kind then flag("char", guid, name, kind); found = found + 1 end
            until not q:NextRow()
        end
        out(string.format("ovadmin next %d", last))
        return out("ovadmin end " .. found)
    elseif sub == "check" then
        local text = tostring(command):match("^%S+%s+check%s+(.+)$") or ""
        local kind = N.NameHit(text)
        out("ovadmin name " .. (kind or "ok"))
        return out("ovadmin end 1")
    elseif sub == "reload" then
        local n = N.Reload()
        out(string.format("ovadmin ok: %d of the owner's words", n))
        return out("ovadmin end 0")
    end
    return out("ovadmin error: usage: ovnames list|rename|done|scan|check|reload")
end

local function onCommand(event, player, command, handler)
    if player then return nil end   -- console and SOAP only
    local c = tostring(command or "")
    if not c:match("^ovnames%f[%s%z]") and c ~= "ovnames" then return nil end
    local function out(line) if handler then pcall(handler.SendSysMessage, handler, line) end end
    N.Console(c, out)
    return false
end

RegisterPlayerEvent(1, onCreate)          -- PLAYER_EVENT_ON_CHARACTER_CREATE
RegisterPlayerEvent(3, onLogin)           -- PLAYER_EVENT_ON_LOGIN
RegisterPlayerEvent(18, onChat)           -- PLAYER_EVENT_ON_CHAT (say, yell, emote)
RegisterPlayerEvent(22, onChannelChat)    -- PLAYER_EVENT_ON_CHANNEL_CHAT
RegisterPlayerEvent(42, onCommand)        -- PLAYER_EVENT_ON_COMMAND
if RegisterGuildEvent then RegisterGuildEvent(5, onGuildCreate) end   -- GUILD_EVENT_ON_CREATE

N._test = { tokens = tokens, fold = fold, onCreate = onCreate, onLogin = onLogin, onChat = onChat,
    onChannelChat = onChannelChat, onGuildCreate = onGuildCreate, onCommand = onCommand }
return N
