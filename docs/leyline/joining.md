# Joining a world

**Status, 27 Nov 2026:** joining on the same network **works today**. Joining Your Den from far away works today if
the host opens two ports. Dialing a friend by their key from anywhere is **coming** (phase 2 of
[the way forward](way-forward.md)).

How your launcher gets your game into a friend's world, now that Leyline runs no relays of its own.

## Your game never changes

Your game client always talks to your own PC (`127.0.0.1`). A small forwarder in your launcher carries what it
sends to your friend's world, and their launcher hands it to their world. The game never knows. Nothing in your
game folder is changed to make this work, and game files never travel between you.

## What works today

| Where your friend is | Your Den | Single player or the original world |
|---|---|---|
| Same home or office network | Works | Works |
| Anywhere else, host opens ports 3724 and 8085 | Works: **Your own address** in Your Den | Coming |
| Anywhere else, no ports opened | Coming | Coming |

On the same network, your launchers find each other by themselves. Your friend appears under **People near you**
on the Friends list, and an invite needs no internet at all.

For Your Den from far away, the host types their own internet address under **How your friend gets in**. See
[Invite a friend to your Den](den-invite.md#2-your-own-address) for the router steps.

The step by step is in [Play with friends](guide-play-with-friends.md).

## How it will work: dial by key

You won't need an address. Your launcher will reach a friend's launcher by their Passport key, trying these ways in
order and keeping the first one that works:

1. **Same network.** Your launchers find each other at home, with no internet.
2. **Direct.** Straight to your friend's PC: over IPv6 if both lines have it, or through a port your router opens
   for Leyline when you allow it (UPnP or NAT-PMP).
3. **Hole punching.** Both launchers reach out at the same moment, so both routers let the other in. No router
   settings. Projects that do this report a direct connection most of the time: one says about 90% in use, and a
   study of 4.4 million tries found about 70%
   ([iroh](https://pinggy.io/blog/iroh_1_0_dial_keys_not_ips/), [the study](https://arxiv.org/html/2510.27500v1)).
4. **A friend you both know.** If a mutual friend's launcher is online and you both allow it, it passes the bytes
   along. It can't read them.
5. **Community relays.** Run by players who choose to carry others. They pass sealed bytes and can't read them.

Each way is sealed end to end with your two keys. Whoever carries the bytes in between sees only noise.

## Friend requests and messages

- When you're both online, they go straight from your PC to your friend's.
- When your friend is offline, the message waits, sealed so only they can open it. It waits with a friend you both
  have, or in a blind mailbox on community relays, which hold sealed messages they can't read, for a short time.

## Invite links

An invite link will open your launcher on **Join**, with your friend's world already picked. The link carries your
friend's key and the world, never an address. If your friend's PC moves to a new network, the same link still
works.

## What other people see

- **Direct and same network:** your friend's PC sees your internet address, as when you play any game with a friend.
- **Through a mutual friend or community relays:** your friend doesn't see your address, and whoever carries the bytes sees
  only sealed data and the address you connect from.
- **Leyline:** sees nothing. We run no part of it.
