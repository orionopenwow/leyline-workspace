-- lua5.1 stream/tests/test_ov_domain.lua [lua_scripts dir]
-- The domain move in game (docs/DOMAIN-MOVE.md): ov_modapi.lua OVMod.Names says the old names until
-- tools/domain/domain.sh on writes leyline-domain.on (re-read every 30 s, no reload), then the new ones; and the
-- realm scripts' chat lines go through it (ov_go.lua, under a fake mod-ale).
local scripts = arg[1] or "realm/custom/lua_scripts"
local pass, fail = 0, 0
local function check(name, ok, got)
    if ok then pass = pass + 1; print("PASS " .. name) else fail = fail + 1; print("FAIL " .. name .. " " .. tostring(got)) end
end
local env = {}
local realGetenv = os.getenv
local flag = os.tmpname(); os.remove(flag)
env.OV_DOMAIN_FILE = flag
env.OV_BRAND_FILE = os.tmpname() .. ".none"
os.getenv = function(k) if env[k] ~= nil then return env[k] end return realGetenv(k) end
local clock = 1000000
os.time = function() return clock end
function RegisterServerEvent() end
function RegisterPlayerEvent() end
function RegisterPlayerGossipEvent() end
function GetPlayersInWorld() return {} end
dofile(scripts .. "/ov_modapi.lua")
local OVMod = assert(_G.OVMod)
local line = "Boards: play.openvanilla.app/wrath/leaderboards, the game https://wrath.openvanilla.app/, relay.openvanilla.app:40002"
check("off: the old names", OVMod.Names(line) == line, OVMod.Names(line))
local f = assert(io.open(flag, "wb")); f:write("on\n"); f:close()
check("the flag is read again only after 30 s", OVMod.Names(line) == line)
clock = clock + 31
check("on: the new names", OVMod.Names(line) == "Boards: leylinelauncher.com/wrath/leaderboards, the game https://play.leylinelauncher.com/, relay.leylinelauncher.com:40002", OVMod.Names(line))
check("not text: left alone", OVMod.Names(nil) == nil and OVMod.Names(5) == 5)
env.OV_LEYLINE_DOMAIN = "0"
check("OV_LEYLINE_DOMAIN=0 wins over the flag", OVMod.Names(line) == line)
env.OV_LEYLINE_DOMAIN = nil
os.remove(flag); clock = clock + 31
check("off again: the old names", OVMod.Names(line) == line)
-- every realm script that prints the site's address sends its chat lines through OVMod.Names
for _, s in ipairs({ "ov_demo.lua", "ov_duels.lua", "ov_go.lua", "ov_playmode.lua", "ov_workshop.lua" }) do
    local fh = assert(io.open(scripts .. "/" .. s, "rb")); local src = fh:read("*a"); fh:close()
    check(s .. ": its say() uses OVMod.Names", src:find("local function say%([^\n]*OVMod%.Names") ~= nil)
end
print(pass .. "/" .. (pass + fail) .. " passed")
os.exit(fail == 0 and 0 or 1)
