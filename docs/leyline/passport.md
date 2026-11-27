# The Passport

**Status, 27 Nov 2026:** partly works today. The recovery phrase, devices, guardians, a key for each world and the
login record are built and tested inside the launcher. The test on two real PCs ([the way forward](way-forward.md),
phase 1's gate) is still to do, so treat it as new.

The Passport is how you log in with Leyline when no website and no server of ours is there to log in to. It is a
key on your PC. Your friends and the worlds you play on know you by it. You find it with your friends: **Single
player**, **Friends**, **Your Passport**.

## What works today

### Your Passport key and your recovery phrase

Your launcher has its own key, in a file called `peer.key` in Leyline's own folder. It never leaves your PC unless
you move it to your own other PC. Your **friend code** (`LF1-…`) is made from it, your friendships are signed with
it, and your friends' worlds know you by it.

- **A new launcher** makes its key from **12 words**. The first time you open Your Passport, give a name (and a
  picture if you like), press **Make my Passport**, write the 12 words on paper, tick **I wrote these words down**
  and press **Done**. With those words, any PC becomes you again.
- **A launcher from before the Passport** keeps the key it has, so your friends and your characters stay as they
  are. Its phrase is **24 words** (**Back up**, **Show my phrase**). They bring back the same key.
- **Back up** also has **Save a backup file…**: the same key as a file. Keep it private.
- **Recover**, **With your recovery phrase**: type the words and this PC is you again (the key it had is kept aside
  in Leyline's folder). Restart Leyline after.

### Your devices

- **Devices** lists every PC on your Passport.
- **Add a device**: on the new PC, open **Your Passport**, **Recover**, **From my other PC**, **Add this PC**. On the
  PC that has your Passport, **Devices**, **Find it**. The new PC shows a 6-digit code; type it on the old one and
  press **Send my Passport**. Both PCs need to be on the same network (or the new one reachable at an address you
  type). Nothing goes through a website. If the code doesn't match, nothing is sent.
- **No network between them?** **Use a file**: the old PC saves a sealed file and shows 6 words. On the new PC, pick
  the file and type the 6 words.
- **Remove** takes a device off your Passport. Every device holds your whole Passport today, so if a PC was lost,
  also think about guardians or a new phrase.

### A different key for each world

When you join a friend's world for the first time, your launcher uses a key made only for that world. The host sees
that key, never your Passport key, so two hosts can't put their notes together to follow you. Your account name on
that world is made from the world key, the same every time. Worlds you joined before keep the login they gave you.

### The login record

When you join, your launcher and the host's launcher write one short record together: this world key, this world,
this account name, and a number that goes up each time. Both launchers sign it and both keep a copy. If the host
loses their list of friends but keeps their world, your copy brings your account back.

### Guardians

**Back up**, **Guardians**: pick 2 to 5 friends and press **Save guardians**. Their launchers get the list.

- If you lose every PC and your words: on a new PC, **Recover**, **Ask 2 guardians**. It shows a code (`LR1-…`).
  Call or meet two guardians; each opens **Your Passport**, **Help a friend**, pastes your code and sends you back
  an answer (`LG1-…`). Paste both answers. Your new PC is your Passport, with new 12 words to write down.
- One guardian alone can't do it.
- Your friends see the change after **3 days**. If someone else tries it, any PC that still has your Passport hears
  of it and shows **Stop it**.
- Worlds you joined before the Passport follow you to the new key after the 3 days. Worlds you joined with a world
  key come back with your words, or with your copy of the login record.

### The old website key

The key you used on our website stopped working when the server closed. **Back up**, **Bring in my Hero Key
file…** imports it once: your Passport keeps its id and a fingerprint, never the key. You don't need it for anything
else; you can delete the file after.

## What is coming

- **Scanning a QR code** instead of typing the 6 digits.
- **Device keys for every day**: today each device holds the whole Passport. Next, devices sign day to day with
  their own keys and the Passport key stays where it is safest.
- **Your profile card**: who sees it (friends only, or anyone you play with).
- **The game password**: your launcher makes a long random one and types it for you (it does this today; joining by
  Passport alone, without an invite, is coming with [connections](way-forward.md)).

## What we keep

Nothing. No keys, no emails, no internet addresses, no passwords. Your Passport, your devices and your recovery
setup live on your PCs and with the friends you choose.

## Still being decided

- The name. "Passport" is the working name.
- How many guardians to suggest by default, and whether to ask for them on first run or later.
