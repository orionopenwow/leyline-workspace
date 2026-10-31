# Bring your server

Already run a server? Put it on the Leyline network. Players find it on the realm list, hop to it from inside the game, and sign in with their Hero Key: no new account, no password to type.

**For:** AzerothCore and TrinityCore servers for the Wrath client. Other cores are not ready for Hero Key sign-in yet: you can still list them with **Link my server** (see [Keep your realm up 24/7](always-on.md)).

| You need | |
|---|---|
| Docker | on the PC or server that runs your realm |
| Your server's login and world addresses | as that PC reaches them, like `127.0.0.1:3724` |
| A database user | one that may add and change accounts in your login database |
| Open ports | none: the helper talks out to the relay |

## What it does

One small helper runs next to your server. It:

- talks out to the Leyline relay, so players reach you without knowing your address;
- puts your server on the realm list and keeps its line up to date;
- makes an account for each Hero Key player the first time they sign in, named `HK` and 14 letters.

It changes nothing else: not your realm list, other accounts, characters or files. The database password is asked once, kept in `state/link-db.env` (only you can read it) and never shown.

## Join

On Linux or macOS: `join-network.sh` is in [the Host pack zip](https://github.com/orionopenwow/Leyline/releases/download/host-kit/WrathUnleashed-host-kit.zip). It also works on its own: copy just that file into an empty folder on your server, and run:

```
bash join-network.sh
```

It asks ten short questions: the name for the list, your core, the two addresses, and your login database. Then it checks the database, puts your server on the list, starts the helper and runs the self-test below.

Windows: run **Link my server** from the Host pack first. Hero Key sign-in for a server on Windows is not in a button yet.

## Test Hero Key sign-in

```
bash join-network.sh test
```

Or press **Test Hero Key sign-in** in the launcher: Host, then the **Network** tab. It is a dry run. It checks each step and says what to fix:

1. Hero Key sign-in is switched on for your realm.
2. The website's key is pinned on your PC.
3. The helper reaches your login database, the server is for the Wrath client, and the user may add accounts.
4. A test sign-in passes the same check a real one does; a copied or changed one is refused.
5. A test account is made, its one-time login checked, and all of it undone.
6. The relay offers Hero Key sign-in for your realm.

Nothing it does stays in your database.

## Two settings on your server

- Players all arrive from the helper's address. Set `WrongPass.MaxCount = 0` in `authserver.conf`; the relay limits wrong passwords per player instead.
- The realm address your server hands out must already work for players. Leyline leaves it alone.

## The Network tab

In the launcher, Host, then **Network**:

- **On the realm list**: listed, or invite link only.
- **Hero Key sign-in**: on or off, and the test.
- **Who's online**: the count, and a box to show character names on the list. Off by default; GMs and bots are never shown.
- **Mods players need**: each one required or optional, and whether players install it.
- **House rules**: shown on the list and in game before a hop.

## Banning a Hero Key player

A Hero Key player is an ordinary account on your server. Ban it as you ban any other. Each player gets a different `HK` name on every server, so a ban here stays here.

## Stop or leave

```
bash join-network.sh stop      # offline at once, off the list after 6 hours
bash join-network.sh remove    # the helper is gone; your realm id stays in state/
```
