# How the Leyline network works

A plain guide for players: how your launcher finds worlds and friends, and what we never do.

## The network now

**Status, 27 Nov 2026:** Leyline runs no server and no relays of its own. Same-network joins and the hand-picked
realm list **work today**. The rest of this section is **coming** ([the way forward](way-forward.md)).

```
  You (launcher)  <---- straight to your friend's PC when it can ---->  Your friend's PC
        |                                                                      ^
        +---- else: a friend you both know, or community relays ---------------+
                 (they pass sealed bytes and can't read them)
```

1. **No centre.** No part of the network is ours to switch off. Each part can come from more than one place.
2. **You dial a key, not an address.** Your launcher finds a friend's PC by their [Passport](passport.md) key: the
   same network first, then straight across the internet, then hole punching, then a friend you both know, then
   community relays. See [Joining a world](joining.md).
3. **Community relays are run by players.** Anyone can carry others' sealed traffic from their own PC. See
   [Help carry the network](be-a-node.md).
4. **Lists are signed records.** Realm lists, releases, profile cards and blocks are small signed records. Your
   launcher collects them from other launchers, from a public lookup network, from copies of our site and repo,
   from community relays, from your home network and from your friends. A record is kept only if its signature
   checks.
5. **Blocks need two of three.** A block (a bad realm, a bad release) counts only when two of a three-person council
   sign it.
6. **Anyone can fork.** The code, the record formats and a kit to start your own copy are public.

## How it worked until November 2026

The rest of this page is how the network ran on our server. The relay and the self-filling realm list closed with
it on 26 Nov 2026.

```
  You (launcher, game or browser)
            |
            v
   Leyline relay  ---- the realm list (every public realm)
            |
            v
  A host's PC, running their realm
```

1. **Realms are run by players.** A host runs a realm on their own PC. Leyline does not rent servers.
2. **The relay connects you.** You talk to the relay and the relay passes your game through to the host's PC. Nobody opens ports.
3. **Every realm has an id.** Sixteen letters and digits, made from the realm's own key on the host's PC. It never changes, so links keep working.
4. **Your address stays hidden.** Hosts never see players' internet addresses, and players never see the host's.
5. **Invite links open private realms.** The secret part of the link stays in your browser. A host can make a new link at any time, and the old one stops working.
6. **Public realms are listed.** A public realm shows on the realm list about a minute after the host's PC connects, and leaves it a few hours after the PC goes off.
7. **Claimed means a real account.** A host can claim their realm with a Hero Key account, so the list shows who runs it. Unclaimed realms say so.
8. **Official and community.** Wrath Unleashed is the official realm, run by Leyline. Every other realm is a community realm, run by its host under their own house rules.
9. **Dens are private rooms.** A Den is a small, invite-only realm to build in with friends. It is never on the public list.
10. **What we never do.** We never ship, upload or share game files: every host and player uses the game folder they already have. We never sell places on the list, and bots are always counted apart from players.

Want to host? See [Hosting](hosting.md) and [Play with friends](guide-play-with-friends.md). Something wrong with a realm? Use **Report** on its card on the [realm list](https://leylinelauncher.com/wrath/realms).
