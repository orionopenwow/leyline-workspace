# Friends

Add your friends in the Leyline launcher, see when they're on, and invite them into your world. No account or
website is needed for this: you and each friend sign your friendship yourselves, and it is kept on your two PCs.
It works on the same home network with no internet, and from anywhere through a relay that can't read your messages.

Where it is: in the launcher, open **Single player**, then the **Friends** area. Your friends list is at the bottom.

## Add a friend

There are two ways.

**With a friend code** (works from anywhere):

1. Press **Show my friend code**. It is copied for you. It looks like `LF1-…`.
2. Send it to your friend however you like: a message, Discord, a text. A code works for a day.
3. Your friend pastes it under **Add a friend** and presses Add. You get a request.
4. You press **Accept** on your Friends list. You're friends.

**On the same network** (no code needed): when you're both on one home or office network, you see each other under
**People near you**. Press Ask on their name; they accept on their list.

If you're not both online, the request waits and goes out the next time you are ("Request saved").

## See who's on

A friend shows as online while their launcher is open (for up to 10 minutes after the last time it said so), or
while it is heard on your network. If they chose to share it, you also see what they're playing.

In **Your profile** you choose your name, a short status, and what friends may see: what you're playing, and whether
friends can reach you through the Leyline relay's mailbox.

## Invite a friend to your world

On your Friends list, press **Invite** on a friend. Only that friend gets the invite, sealed so the relay can't read
it. They press **Join** on your name within a day. Their character stays on your world, so next time they press
**Join again** and need no new link.

Stopping your world (or Stop sharing) takes the invite away at once.

## Remove a friend

Press **Remove** on their name. That ends the friendship on both PCs, and an old copy of it can't be brought back.
To be friends again, add each other again.

## Your friend key

Your launcher makes a key for you the first time (a file called `peer.key` in Leyline's own folder). It never leaves
your PC. It is how your friends know it's you. **If you lose it, your friends need to add you again**, so keep it
when you move to a new PC (there is no export button yet: copy the file).

## When something goes wrong

- **"Paste your friend's code first."** The box was empty. Paste the whole `LF1-…` code.
- **The code doesn't work.** A friend code lasts a day. Ask for a new one.
- **"Request saved: it goes out when you're both online."** Nothing is wrong: it is sent the next time you are both
  online.
- **You don't see each other under People near you.** You must be on the same network (the same router), and your
  firewall must let Leyline talk on the local network. Use a friend code instead.
- **Nothing arrives from a friend far away.** Their relay mailbox may be off. The relay mailbox is new and is
  switched on per relay. Until it is on, friends on different networks need to be online at the same time. TODO(owner):
  say here when the mailbox is on for the official relay.
- **A friend shows offline but is playing.** They may have turned off sharing what they play, or their launcher is
  closed. Ask them.
- **Something else.** Use Report a bug in the launcher, or ask on the Discord.

## What we keep

Nothing. Friendships, profiles and messages live only on your PC and your friend's PC. A relay that carries a
message for you holds it sealed, for a day at most, and can't read it.

The details, for the curious: `docs/LEYLINE-FRIENDS.md` in the repo.
