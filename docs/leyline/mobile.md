# On your phone

Leyline plays on a phone two ways: in your phone's browser, or in the Leyline app for Android. Both play the same
browser game with touch controls. The showcase page, with screenshots, is [Leyline on your phone](/wrath/mobile).

## What a phone can do

| What | On a phone | How |
|---|---|---|
| The official realm | Yes | Play, in the browser or the app |
| A friend's realm on their PC | Yes | Their invite link: the app's **Join a friend**, or **Play in your browser** on the link's page |
| Community realms | Most | Those for the browser game's client play in the app; others need the game on a computer |
| Your own world | Yes | **Play solo on this phone**: the phone runs it, just for you |
| Hosting a realm for others | No | Host on a PC: [Host for friends](host-for-friends.md) |
| A friend's Den | No | Dens play in the game on a computer: [Invite a friend to your Den](den-invite.md) |
| Interface addons | Yes | **Your addons** in the game page, from the addon's .zip |
| Realm mods and game modes | Yes | On a realm that runs them: nothing to install |
| Client mods, looks, HD packs | No | They change a game folder: Leyline on a computer |

## No server on the phone (and the one exception)

A phone can't keep a realm up for friends: it sleeps, changes networks and has no address others can reach. So
playing together always goes through a realm someone hosts: the official realm, a community realm, or a friend's PC.

The exception is **Play solo on this phone**. The game page runs a small server of our own inside itself, so your
world needs no realm. It is a sandbox for one: your companion, training dummies, classes and gear, building. No
quests or loot there yet. It is saved on the phone.

## Join a friend from your phone

1. Your friend opens Leyline on their PC and presses **Invite a friend**. When every line is green, the link is copied.
   They send it to you.
2. On your phone, copy the link, open the Leyline app and tap **Paste invite link and join** (on Play). Or paste it
   under **Realms**, **Got an invite link?**, and press **Join**.
3. The realm opens in the app. Its own page asks you for an account name and password, once. Your password never
   leaves your phone.

No app? Open the link in your phone's browser, wait for "this computer is let in", and press **Play in your browser**.

The secret part of an invite link (after the #) is never sent to our servers: the app hands it to the game page in the
address's fragment, and the game page hands it to the realm's own join page.

## Get the app

The app comes from the Leyline repo's GitHub releases: **Leyline for Android (latest)**, file **Leyline-debug.apk**,
with its **.sha256** checksum beside it. GitHub's own machines build it from our source and start it on an Android
emulator before it is published. Android asks you once to allow installing from your browser or Files app.

## When something's wrong

| What you see | What to do |
|---|---|
| "This phone's web view can't draw the game yet" | Tap **Open in Chrome**. Updating Chrome and Android System WebView often fixes it |
| The touch controls don't show | Add `?touch=1` to the game's address once; it is remembered |
| Play solo says it can't start here | This browser doesn't allow what solo needs; the game plays on the realm instead. Use Chrome |
| Join a friend says it's a Den | Dens need the game on a computer. Ask for a realm invite instead |
| "This invite link no longer works" | Links work for 24 hours. Ask your friend for a new one |
| The friend's realm isn't answering | Your friend presses **Invite a friend** on their PC: it checks everything and says what to fix |
| The app won't install | Allow **Install unknown apps** for the app you downloaded it with |
