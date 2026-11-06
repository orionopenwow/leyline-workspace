# Host for friends

Play on your own PC, alone or with friends. Leyline runs the world on your PC and brings your friends in through the
Leyline relay: no router settings, no ports to open, and your home address is never shown to anyone.

## Pick your world

| | Your Den (recommended for friends) | Full realm |
|---|---|---|
| What it is | Your own place to build, explore and hang out together | The whole game together: quests, dungeons, bots, your own rules |
| What it needs on your PC | Nothing more: it is built into Leyline (a Windows or Linux PC) | Docker Desktop (free), about 4 GB of free memory, a one-time setup of 5 to 20 minutes |
| How fast it starts | A few seconds | A minute or two, once set up |
| Accounts | None: each invite has a login made for that friend | Friends make an account on the invite link's page |
| Where to start | **Play with friends** (below), or the **Your Den** page | The **Host page**: Private, a template, Host |

Both are private: only people with your link get in. Both use your friends' own game folders: game files never
travel, not through the link, the relay or between your PCs.

## Play solo first

- **Your Den**: press **Play solo** on Home (or **Open my Den** on the Your Den page). The first time, Leyline gets
  Your Den's server (one small download, checked), then your game opens inside your Den.
- **A full realm**: on the Host page pick **Private**, a template, then **Host**. When it says it's live, press
  **Play**.

Your friends can join any time after. You don't have to be in the game for them to play, but Leyline must stay open
and your PC awake.

## Invite a friend (one step)

1. Press **Invite a friend**: on Home, in the play bar's realm menu (**Invite a friend…**), on the Host page's
   banner, or on Your Den's **Share with friends** card.
2. Leyline starts your world if it isn't running, puts it on the Leyline network (private), makes one link for one
   friend, and runs **Ready for friends?** (below).
3. When every line is green, the link is copied. Send it to one friend: a message, Discord, email. **Share…** uses
   your system's share menu.

Want a second friend? Press **Invite another friend**: each friend gets their own link. A Your Den link works for
24 hours; **Keep** on the Friends list makes it last (a week from each visit). **Stop sharing** ends every link.

## Ready for friends?

Before you send the link, Leyline checks the same way your friend's game will come in, one line at a time. Each red
line says what to do, and most have a button for it.

| Line | What is checked | When it's red or yellow |
|---|---|---|
| Your world is running | Your world runs on this PC and answers a login on its port | **Start again**. Still red: restart your PC |
| Connected to the Leyline relay | This PC's one connection out to the relay is up | Check this PC is online. A VPN, school or work network may block it: try another network |
| The relay sees your world | The relay itself lists your world as online | Wait a few seconds, **Check again** |
| A friend can reach you | A test login goes through the relay to your world and back, the exact path your friend uses | **Start again**, then **Check again**. Still red: **Copy diagnostics** and send it to us |
| Your connection | How far the relay is, and the round trip through it | Yellow means friends may feel lag: see "It's laggy" below |
| Your PC stays awake | Your PC's sleep timer (Windows) | **Open sleep settings** and set Sleep to Never while friends play |
| Firewall | Nothing to open for the relay. Your Den's faster same-network way may want Den Lite allowed | **Open firewall settings**, allow Den Lite on private networks |

The test login uses an account name no world has, so no account, password or login is used, and it never counts as a
wrong password.

**Copy diagnostics** copies the checks as text for whoever helps you. It never has your invite link, a friend's
login or a password.

## For your friend

1. They need **Leyline**, **their own game folder** (the same version as yours, added under **Game folders**) and a
   **Hero Key**.
2. They open your link and press **Open in Leyline**, then **Join**. Or, in Leyline: **Realms**, **Got an invite
   link?**, paste it, **Join**.
3. Leyline shows what's happening: **Reaching your friend's world**, **Their world answers**, **Logging in**. If a
   step stops, the line says why and what to do.

A Your Den link has their login inside: the game signs them in by itself. A full realm's link page has **Make my
account** (the password never leaves their browser).

## When something's wrong

| What you see | Why | What to do |
|---|---|---|
| Your friend's game loops between **Connecting** and **Connected** | The relay is reached, but your world isn't answering behind it: still starting, stopped, or your PC slept | Host: press **Invite a friend** (or **Check again**) and fix what's red. Keep Leyline open and the PC awake. Your friend's join page now says "the host's world isn't answering" instead of looping |
| It's laggy | Every move goes your PC → the relay → your friend. A far relay, Wi-Fi, or a busy upload (cloud backups, streaming, a big download on the same line) adds delay. A shared internet address (CGNAT) works, but some of those lines are slower | Use a wired connection, pause big uploads, or let the friend with the faster internet host. **Your connection** on the checklist shows the delay |
| The invite says it **isn't valid** | Links work for 24 hours, and stop when you press **Stop sharing** or make a new link for a full realm | Press **Invite a friend** again and send the new link. **Keep** makes a Your Den link last |
| Your friend **can't make an account** | Your Den needs none: the link has their login. A full realm makes accounts only from its invite link | Your Den: they press **Open in Leyline** (the login comes with it). Full realm: they open the link on the PC they play on and use **Make my account** |
| "Private realm: open your invite link on this computer first" | Their PC hasn't opened the link yet, or their internet address changed since | They open the link again on that PC |
| "Your friend's world isn't online" | Leyline is closed on your PC, or the PC slept or lost its internet | Open Leyline: sharing comes back by itself after a restart |
| "Too many tries from your address" | Several wrong passwords from one address in a few minutes | Wait ten minutes, then try again |
| Your friend's Leyline says it needs a game folder | Their game folder isn't added, or it's another version | They add their own folder of the same version under **Game folders** |

## One PC, one host

Your world is there while Leyline is open and your PC is awake: when you close it, your friends' game stops too. For
a world that's always up, run a full realm on a computer that stays on (see [Keep your realm up 24/7](always-on.md)),
or ask a friend with a faster connection to host.

## More

- [Invite a friend to your Den](den-invite.md): the three ways in (same network, your own address, the relay).
- [Host a realm](host-a-realm.md): the Host page, start to finish.
- [Your Den](den.md): building, saving and sharing your Den.
