@echo off
setlocal
title WrathUnleashed - sign-in and warm-up
rem The server half of the faster start (run "Update browser client.bat" first, for the new client):
rem  - play.openvanilla.app/wrath signs in by itself with a Hero Key saved in the browser,
rem  - the realm list downloads the browser client in the background while you read it,
rem  - the client's own files (no game data) skip the sign-in gate so that download can happen.
rem Backs up first and puts things back if anything refuses. One upload and one SSH login.
rem Needs next to this file: wrath_portal.py, fastloop.sh and www-files\art (the pages' art and theme).
rem Optional: docs.py with ..\docs\leyline (the Leyline docs at /wrath/docs).
cd /d "%~dp0"
set "KEY=%USERPROFILE%\.ssh\openwow"
set "HOST=root@play.openvanilla.app"
set "OPTS=-i "%KEY%" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30"
for %%f in (wrath_portal.py fastloop.sh www-files\art\wu-theme.css www-files\art\wu-card.webp) do if not exist "%%f" (echo %%f is missing next to this file. & pause & exit /b 1)
if not exist "%KEY%" (echo SSH key %KEY% not found. & pause & exit /b 1)
set "T=%TEMP%\wu-fastloop.tgz"
if exist "%T%" del /q "%T%"
set "LINKS="
if exist links.py set "LINKS=links.py"
rem the shared rate limits (ratelimit.py) every module below uses; the portal runs a stand-in without it
if exist ratelimit.py set "LINKS=%LINKS% ratelimit.py"
if exist demo.py set "LINKS=%LINKS% demo.py"
if exist armory.py set "LINKS=%LINKS% armory.py"
if exist modes.py if exist modes_data.py set "LINKS=%LINKS% modes.py modes_data.py"
rem the game modes (/wrath/modes.json, Games on /wrath/modes, ?go=mode:): playmodes.py and its catalog, always as a pair
if exist playmodes.py if exist playmodes.json set "LINKS=%LINKS% playmodes.py playmodes.json"
rem modes anyone can host and fork (/wrath/modes/<id>, /wrath/modes/jam): modepack.py and its catalog, always as a pair
if exist modepack.py if exist modepacks.json set "LINKS=%LINKS% modepack.py modepacks.json"
if exist workshop.py set "LINKS=%LINKS% workshop.py"
if exist creations.py set "LINKS=%LINKS% creations.py"
if exist projects.py set "LINKS=%LINKS% projects.py"
rem Leyline flags.json and the Host pack listing (leyline_net.py; hostpack.json from hosting\pack\make-hostpack.py)
if exist leyline_net.py set "LINKS=%LINKS% leyline_net.py"
if exist leyline_net.py if exist hostpack.json set "LINKS=%LINKS% hostpack.json"
rem reviewed upstream addons for all.json (upstream_addons.py and upstream-addons.json from tools\addon-curate\curate.py), always as a pair
if exist upstream_addons.py if exist upstream-addons.json set "LINKS=%LINKS% upstream_addons.py upstream-addons.json"
rem the Leyline network: the realm browser, /wrath/realms.json and realm reports (network.py)
if exist network.py set "LINKS=%LINKS% network.py"
rem Report a bug: /wrath/bugs and the launcher's /wrath/bugs/api/report, into the bugs manifest (bugreport.py)
if exist bugreport.py set "LINKS=%LINKS% bugreport.py"
rem Leyline friends and messages (/wrath/api/social, /wrath/link)
if exist social.py set "LINKS=%LINKS% social.py"
rem the one blocklist for names (namecheck.py and its blocklist.json, always as a pair; social.py, profiles.py, forum.py,
rem hostflow.py and HQ use it) and HQ's Blocked words page (hq_words.py)
if exist namecheck.py if exist blocklist.json set "LINKS=%LINKS% namecheck.py blocklist.json"
if exist hq_words.py set "LINKS=%LINKS% hq_words.py"
rem the Leyline forum (/wrath/leyline/forum, /wrath/forum/api)
if exist forum.py set "LINKS=%LINKS% forum.py"
rem the forum's Discord link (forum_discord.py: off until portal.env turns it on; it uses ed25519_pure.py, shipped with hostflow.py)
if exist forum.py if exist forum_discord.py set "LINKS=%LINKS% forum_discord.py"
rem the Discord bot's growth side (discord_growth.py and discord_template.json, always as a pair: setup, live realms, Find a group, roles, AutoMod; every part off until portal.env turns it on, docs\DISCORD.md)
if exist forum_discord.py if exist discord_growth.py if exist discord_template.json set "LINKS=%LINKS% discord_growth.py discord_template.json"
rem the upgraded Discord channel plan (docs\DISCORD-PLAN.md), opt-in with WU_DISCORD_TEMPLATE in portal.env
if exist discord_growth.py if exist discord_template_v2.json set "LINKS=%LINKS% discord_template_v2.json"
rem the duel leaderboards (/wrath/leaderboards, /wrath/api/leaderboards; duels-export.py fills them on the realm)
if exist duels.py set "LINKS=%LINKS% duels.py"
rem Leyline profiles and the sign-in chooser (/wrath/signin, /wrath/profile, /wrath/avatar)
if exist profiles.py set "LINKS=%LINKS% profiles.py"
rem Your Hero Key (/wrath/key, and the launcher's Get your Hero Key: herokeys.py; the site's /account/key goes there)
if exist herokeys.py set "LINKS=%LINKS% herokeys.py"
rem the Leyline Admin Console (admin.py, /wrath/admin) and the mod checker it can re-run (mods\wu-mod.py)
if exist admin.py if exist ..\mods\wu-mod.py copy /y "..\mods\wu-mod.py" wu-mod.py >nul
if exist admin.py set "LINKS=%LINKS% admin.py"
if exist admin.py if exist wu-mod.py set "LINKS=%LINKS% wu-mod.py"
rem Leyline HQ, the home of the console (hq.py beside admin.py: /wrath/admin, docs\ADMIN-CONSOLE.md)
if exist admin.py if exist hq.py set "LINKS=%LINKS% hq.py"
rem the hidden Leyline hub (/wrath/leyline-preview): hub.py and its index (hub\community-index.json), always as a pair
if exist hub.py if exist hub\community-index.json set "LINKS=%LINKS% hub.py hub"
rem the hub rankings (hub_rank.py: /wrath/leyline/mods.json, installs and ratings; its store is /data/hub_rank.sqlite3)
if exist hub_rank.py set "LINKS=%LINKS% hub_rank.py"
rem List your mod (listings.py: /wrath/mods/list, /wrath/api/listings) with the one rules file the launcher builds in
if exist listings.py if exist ..\launcher\src-tauri\src\upstream-rules.json copy /y "..\launcher\src-tauri\src\upstream-rules.json" upstream-rules.json >nul
if exist listings.py if exist upstream-rules.json set "LINKS=%LINKS% listings.py upstream-rules.json"
rem "+ Your server" (hostflow.py) with the kit's Ed25519, always as a pair
if exist hostflow.py if exist ..\hosting\kit\tools\wunet\ed25519_pure.py copy /y "..\hosting\kit\tools\wunet\ed25519_pure.py" ed25519_pure.py >nul
if exist hostflow.py if exist ed25519_pure.py set "LINKS=%LINKS% hostflow.py ed25519_pure.py"
rem Hero Key sign-in on Beacons (herokey_sso.py, docs\HERO-KEY-SSO.md; idle until the herokey_sso flag is on), with the kit's Ed25519
if exist herokey_sso.py if not exist ed25519_pure.py if exist ..\hosting\kit\tools\wunet\ed25519_pure.py copy /y "..\hosting\kit\tools\wunet\ed25519_pure.py" ed25519_pure.py >nul
if exist herokey_sso.py set "LINKS=%LINKS% herokey_sso.py"
if exist herokey_sso.py if exist ed25519_pure.py if not exist hostflow.py set "LINKS=%LINKS% ed25519_pure.py"
rem the Leyline docs: docs.py and the pages, which live in the repo's docs\leyline (or docs\leyline next to this file)
set "DOCS="
if exist docs.py if exist docs\leyline\index.md set "DOCS=docs.py docs/leyline"
if exist docs.py if not defined DOCS if exist ..\docs\leyline\index.md set "DOCS=docs.py -C .. docs/leyline"
tar -czf "%T%" wrath_portal.py %LINKS% fastloop.sh www-files/art %DOCS% || (echo Packing failed. & pause & exit /b 1)

echo [1/2] Uploading...
scp %OPTS% "%T%" %HOST%:/tmp/wu-fastloop.tgz || (echo Upload failed. If it says "timed out", wait 2 minutes and run this again. & pause & exit /b 1)

echo [2/2] Applying it on the server (under a minute)...
ssh %OPTS% %HOST% "mkdir -p /srv/wrath/fastloop && rm -rf /srv/wrath/fastloop/www-files /srv/wrath/fastloop/docs /srv/wrath/fastloop/hub && tar -xzf /tmp/wu-fastloop.tgz -C /srv/wrath/fastloop && rm -f /tmp/wu-fastloop.tgz && cd /srv/wrath/fastloop && sed -i 's/\r$//' fastloop.sh wrath_portal.py $(ls ratelimit.py links.py demo.py armory.py modes.py modes_data.py playmodes.py modepack.py workshop.py creations.py hostflow.py ed25519_pure.py herokey_sso.py projects.py social.py forum.py forum_discord.py discord_growth.py duels.py profiles.py herokeys.py network.py bugreport.py docs.py leyline_net.py upstream_addons.py admin.py wu-mod.py hq.py hub.py hub_rank.py listings.py 2>/dev/null) && bash fastloop.sh 2>&1 | tee fastloop.log; grep -q '^FASTLOOP OK' fastloop.log"
if errorlevel 1 (
  echo.
  echo It did not report success; see the lines above. Anything it changed was put back.
  del /q "%T%" 2>nul
  pause
  exit /b 1
)
del /q "%T%" 2>nul
echo.
echo Done. Open https://play.openvanilla.app/wrath signed out: a Hero Key saved in this browser signs you in.
pause
exit /b 0
