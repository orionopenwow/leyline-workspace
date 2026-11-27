# Joining a world

**Status, 27 Nov 2026:** joining on the same network **works today**. Joining Your Den from far away works today if
the host opens two ports. Dialing a friend's launcher by their key is **built and tested on one PC** (two launchers
in one test, through a mutual friend too); it has **not yet been tried between two homes**, so treat "from anywhere"
as **coming** until the owner's two-network test is written down.

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
| Anywhere else, no ports opened | Coming | Built, tested on one PC only: **From anywhere** in Invite a friend |

On the same network, your launchers find each other by themselves. Your friend appears under **People near you**
on the Friends list, and an invite needs no internet at all.

For Your Den from far away, the host types their own internet address under **How your friend gets in**. See
[Invite a friend to your Den](den-invite.md#2-your-own-address) for the router steps.

For a single player world or the original world, **Invite a friend** now puts your launcher's key in the link
(never an address). Your friend's launcher dials it by that key and passes the game to your world; their game still
connects to their own PC.

The step by step is in [Play with friends](guide-play-with-friends.md).

## How dial by key works

You don't need an address. Your launcher reaches a friend's launcher by their key, trying these ways in order and
keeping the first one that works:

1. **Same network.** Your launchers find each other at home, with no internet.
2. **Direct.** Straight to your friend's PC: over IPv6 if both lines have it, or through a port your router opens
   for Leyline when you allow it (Settings › Network, "Let my router open a port for Leyline"; off until you turn
   it on).
3. **Hole punching.** Both launchers reach out at the same moment, so both routers let the other in. No router
   settings. Projects that do this report a direct connection most of the time: one says about 90% in use, and a
   study of 4.4 million tries found about 70%
   ([iroh](https://pinggy.io/blog/iroh_1_0_dial_keys_not_ips/), [the study](https://arxiv.org/html/2510.27500v1)).
   Hole punching needs a helper both launchers can reach (a relay server a player runs, listed in a signed relay
   card or typed in Settings › Network). Leyline runs none, so until players offer helpers this way is skipped.
4. **A friend you both know.** If a mutual friend's launcher is online and allows it, it passes the bytes along. It
   can't read them. This is Settings › Network, **Help friends connect**: on by default, only between two of your
   own friends, at most 1 GB a day and 4 connections at once unless you change it.
5. **Community relays.** Run by players who choose to carry others (Settings › Network, "Carry sealed connections
   for other players", only while Be a node is on and within its caps). They pass sealed bytes and can't read them.

About one pair of friends in three can't connect straight across (the measurements above), so ways 4 and 5 matter.

Each way is sealed end to end with your two keys. Whoever carries the bytes in between sees only noise.

## Friend requests and messages

- When you're both online, they go straight from your PC to your friend's.
- When your friend is offline, the message waits with a friend you both have, sealed so only your friend can open
  it, for up to 7 days (at most 32 messages per person). It goes on when your friend is back. Built and tested on
  one PC. A blind mailbox on community relays is **coming**.

## Invite links

An invite link for a single player world or the original world carries your friend's key and the world, never an
address. If your friend's PC moves to a new network, the same link still works. Clicking the link opens the launcher
on **Join** (built: the `leyline://join/…` link and the site's `/j/#…` page). Join tries the same network first,
then your friend's key from anywhere (built, tested on one PC only).

## What other people see

- **Direct and same network:** your friend's PC sees your internet address, as when you play any game with a friend.
- **Through a mutual friend or community relays:** your friend doesn't see your address, and whoever carries the bytes sees
  only sealed data and the address you connect from.
- **Leyline:** sees nothing. We run no part of it.
