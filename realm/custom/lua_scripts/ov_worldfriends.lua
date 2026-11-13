-- ov_worldfriends.lua: the realm half of the Leyline tab in the game's Friends window (WrathUnleashedUI/WorldFriends.lua),
-- for a world on the player's own PC (the Built-in engine's Single player / the original world). Off everywhere else:
-- the launcher sets OV_WORLDFRIENDS=1 for its own worlds only (launcher/src-tauri/src/host/engine.rs world_env).
--
-- What the tab shows and does:
--   the host (the world's own player): this world's invite link (to copy), "Invite a friend" (a new link), and the
--     friends who have a lasting login here: their characters, online or not, trusted or not;
--   a friend (a lasting or one-time friend's login): that their character here is kept, and how to come back.
--
-- The game can't reach the launcher, so the two meet in three small tables of this world's characters database (the
-- launcher reads and writes them every few seconds while the world runs: host/worldfriends.rs, INGAME_TABLES):
--   ov_world_asks     the host's asks, run by the launcher ("invite": a new invite, as the launcher's own Invite button),
--                     answered "ok:<text>" / "err:<text>";
--   ov_world_share    k/v: sharing (0/1), link (the invite link: a secret, so only ever sent to the host);
--   ov_world_friends  the lasting friends: account, login, trusted, enabled.
--
-- Transport: the Open Mod API (ov_modapi.lua, prefix OVMOD, whispered to yourself, never relayed). mod "wufriends":
--   client -> server   info     (data empty)
--                      invite   (data empty)       the host only; one ask per ASK_COOLDOWN seconds
--   server -> client   info     key=value lines (\n): role (host|friend), sharing (0|1), link (host only), asking
--                                (0|1: an ask is waiting for the launcher), answer (the launcher's last answer:
--                                ok:<text> / err:<text>), then one line per friend (host only):
--                                f=<login>\t<trusted 0|1>\t<online 0|1>\t<Name:level,Name:level,...>
--                      ok / err <text>
--
-- mod-ale used: CharDBQuery / CharDBExecute, AuthDBQuery, Player:GetAccountId.

if (os.getenv("OV_WORLDFRIENDS") or "0") ~= "1" then return end
local OVMod = require("ov_modapi")

local MOD = "wufriends"
local ASK_COOLDOWN = 15
local MAX_FRIENDS = 20
local MAX_CHARS = 10

local T = {}
OVWorldFriends = T

for _, q in ipairs({
    "CREATE TABLE IF NOT EXISTS ov_world_share (k VARCHAR(16) NOT NULL PRIMARY KEY, v VARCHAR(512) NOT NULL DEFAULT '', at INT UNSIGNED NOT NULL DEFAULT 0)",
    "CREATE TABLE IF NOT EXISTS ov_world_friends (account INT UNSIGNED NOT NULL PRIMARY KEY, username VARCHAR(32) NOT NULL, trusted TINYINT NOT NULL DEFAULT 0, enabled TINYINT NOT NULL DEFAULT 0)",
    "CREATE TABLE IF NOT EXISTS ov_world_asks (id INT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, kind VARCHAR(16) NOT NULL, account INT UNSIGNED NOT NULL DEFAULT 0, made INT UNSIGNED NOT NULL DEFAULT 0, done TINYINT NOT NULL DEFAULT 0, answer VARCHAR(255) NOT NULL DEFAULT '')",
}) do CharDBExecute(q) end

-- A friend's login: one-time (G + 9) or lasting (G + 15); never the host's own (SOLO, ORIGINAL, HK...).
function T.FriendLogin(name)
    name = tostring(name or "")
    return (#name == 10 and name:match("^G[A-Z0-9]+$") ~= nil) or (#name == 16 and name:match("^G[A-Z2-7]+$") ~= nil)
end

-- Only the characters an invite link has (the launcher writes only such; checked again here).
function T.LinkOk(l)
    return type(l) == "string" and #l <= 500 and l:match("^https://[%w%.%-]+/join/[%w]+#[%w~%.:=_%-/]+$") ~= nil
end

local function shareGet(k)
    local q = CharDBQuery("SELECT v FROM ov_world_share WHERE k = '" .. k .. "'")
    return q and q:GetString(0) or ""
end

function T.IsFriend(player)
    local id = tonumber(player:GetAccountId()) or 0
    if CharDBQuery("SELECT 1 FROM ov_world_friends WHERE account = " .. id) then return true end
    local a = AuthDBQuery("SELECT username FROM account WHERE id = " .. id)
    return T.FriendLogin(a and a:GetString(0))
end

local function friendLines()
    local out = {}
    local q = CharDBQuery("SELECT account, username, trusted FROM ov_world_friends ORDER BY username LIMIT " .. MAX_FRIENDS)
    if not q then return out end
    repeat
        local account, user, trusted = tonumber(q:GetString(0)) or 0, q:GetString(1), q:GetString(2) == "1"
        if T.FriendLogin(user) then
            local chars, online = {}, false
            local c = CharDBQuery("SELECT name, level, online FROM characters WHERE account = " .. account
                .. " ORDER BY online DESC, level DESC LIMIT " .. MAX_CHARS)
            if c then
                repeat
                    local name = c:GetString(0)
                    if name:match("^%a+$") then chars[#chars + 1] = name .. ":" .. (tonumber(c:GetString(1)) or 0) end
                    if c:GetString(2) ~= "0" then online = true end
                until not c:NextRow()
            end
            out[#out + 1] = "f=" .. user .. "\t" .. (trusted and "1" or "0") .. "\t" .. (online and "1" or "0") .. "\t" .. table.concat(chars, ",")
        end
    until not q:NextRow()
    return out
end

-- The asks older than ASK_EXPIRE seconds that nobody ran, answered for the launcher.
T.ASK_EXPIRE = 120
function T.ExpireSql(now)
    return "UPDATE ov_world_asks SET done = 1, answer = 'err:The Leyline launcher is closed, so nobody answered. Open Leyline, then ask again.' "
        .. "WHERE done = 0 AND made > 0 AND made < " .. math.floor((tonumber(now) or 0) - T.ASK_EXPIRE)
end

function T.Info(player)
    local friend = T.IsFriend(player)
    local lines = { "role=" .. (friend and "friend" or "host"), "sharing=" .. (shareGet("sharing") == "1" and "1" or "0") }
    if not friend then
        local link = shareGet("link")
        if T.LinkOk(link) then lines[#lines + 1] = "link=" .. link end
        -- an ask nobody ran in two minutes: the launcher is closed (the world keeps running without it), so the game
        -- stops waiting and says so
        CharDBExecute(T.ExpireSql(os.time()))
        lines[#lines + 1] = "asking=" .. (CharDBQuery("SELECT id FROM ov_world_asks WHERE done = 0 LIMIT 1") and "1" or "0")
        local a = CharDBQuery("SELECT answer FROM ov_world_asks WHERE done = 1 ORDER BY id DESC LIMIT 1")
        if a then lines[#lines + 1] = "answer=" .. a:GetString(0):gsub("[|\n]", " ") end
        for _, l in ipairs(friendLines()) do lines[#lines + 1] = l end
    end
    return table.concat(lines, "\n")
end

-- The login tips (ov_qol.lua, one a login, in turn): on your own world, how to play with a friend in game.
OV_EXTRA_TIPS = rawget(_G, "OV_EXTRA_TIPS") or {}
T.TIPS = {
    { text = "Play with a friend: open Friends (|cffffff00O|r), the Leyline tab, and press Invite a friend. Send them the "
        .. "link; their character here is theirs to keep. |cffffff00/together|r opens it too." },
    { text = "A friend already joined? Friends (|cffffff00O|r), the Leyline tab lists them and their characters here, online or not." },
}
for _, t in ipairs(T.TIPS) do table.insert(OV_EXTRA_TIPS, t) end

local lastAsk = -1e9
OVMod.On(MOD, "info", function(player) OVMod.Send(player, MOD, "info", T.Info(player)) end)
OVMod.On(MOD, "invite", function(player)
    if T.IsFriend(player) then
        OVMod.Send(player, MOD, "err", "Only the world's owner invites friends here.")
        return
    end
    local now = os.time()
    if now - lastAsk < ASK_COOLDOWN then
        OVMod.Send(player, MOD, "err", "Asked already: the link shows here in a few seconds.")
        return
    end
    lastAsk = now
    CharDBExecute("INSERT INTO ov_world_asks (kind, account, made) VALUES ('invite', " .. (tonumber(player:GetAccountId()) or 0) .. ", " .. now .. ")")
    OVMod.Send(player, MOD, "ok", "Asked the Leyline launcher for a new invite: the link shows here in a few seconds.")
end)
