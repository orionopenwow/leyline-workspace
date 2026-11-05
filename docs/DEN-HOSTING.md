# Host your Den for friends (no Docker)

Your Den runs on Den Lite, the small server built into Leyline. Friends join it through the Leyline relay:
no Docker, no router settings, and your home address is never shown to them. (How the three ways in work, same
network, your own address and the relay, is in [DEN-INVITE.md](DEN-INVITE.md).)

## For the host

1. Open **Your Den** in Leyline.
2. On the **Share with friends** card, press **Invite a friend**. A link is copied, and a code (`DEN-...`) shows
   under it. Send either one to one friend. Each friend gets their own.
3. The **Friends list** under it shows:
   - who is in your Den now, and since when;
   - the links that still work for friends who aren't in;
   - for each friend: **Let build** (they can build with you, kept for next time), **Keep** (their link keeps
     working, a week from each visit, instead of 24 hours) and **Kick** (out now, and their link stops working;
     **Take back** for a friend who isn't in);
   - **Let everyone build** (every friend, until your Den restarts).
4. **Stop sharing** takes your Den off the relay and stops every link.

In the game the same works by typing: `.den friends` (who is in), `.den friends kick <name>`,
`.den friends build <name> on|off`, `.den friends build on|off` (everyone).

**After a restart:** while you share, Leyline brings your Den and the relay back by itself when it starts again,
so your friends' links keep working after your PC restarts. Kept friends and build rights are saved in your Den.

**Move to a full server…** (a quiet link on the card) still moves your Den to the Docker Den, for combat, rated
duels and more players.

## For the friend

- Open the link and press **Open in Leyline**, or in Leyline open the realm picker next to PLAY, pick
  **Join a friend's Den…**, paste the link or the code and press **Join**.
- You need Leyline, your own game folder and a Hero Key. Nothing else: the link has a login made for you.
- Game files never travel. Each of you plays with your own game folder.

## How it works (for developers)

| Piece | Where |
|---|---|
| Relay tunnel (one outbound connection, private, signup off) | `launcher/src-tauri/src/host/litenet.rs` |
| A login per friend, build and keep switches (guests.json, only salt and verifier) | `labs/den-lite/src/guests.rs`, `den-lite guest add/list/revoke/build/keep` |
| Who's in (`<dens>/den-lite.online`) and the host's asks (`<dens>/den-lite.host-ask`: kick, build, keep) | `labs/den-lite/src/hosting.rs` |
| Friends list, invite code, sharing kept (`<den-lite>/state/den-sharing`) | `launcher/src-tauri/src/host/denhost.rs`, app.rs `den_friends`, `den_friend`, `den_share_resume` |
| The card and the list | `launcher/ui/den.js` (friendsBox, codeRow), `ui/app.js` (Join a friend's Den…) |

The asks file lives in the Dens folder on the host's PC: a friend's game can't write it. A friend's panel
buttons in the game now go through the same guest rules as what they type.

Tests: `launcher/tests/denhost.browser.test.cjs`; Rust unit tests in hosting.rs, guests.rs, denhost.rs, lite.rs,
dens.rs (need `cargo test`).
