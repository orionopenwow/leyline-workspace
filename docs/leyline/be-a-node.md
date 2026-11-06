# Be a node

Your PC can help the Leyline network while you're not playing. No game running, no extra programs, no ports to open.

<!-- TODO(owner): this opening in your words -->

## The switch

In the launcher: **Host**. The **Be a node** card is at the top. Press the switch: **On**.

While it is on, your PC shares Leyline's own files with other players: the launcher's installers, the Host pack,
the Leyline addons and the source code of the server software. Each file is checked against Leyline's signed list
before it is shared. It never shares game files or a game client, and it never shows your address.

The card says what it is doing and how much it has shared today and in all.

- **Pause** stops it until you press **Resume**.
- **Limits**: upload in KB/s, connections at most, MB per day (then it rests until tomorrow), and **Only while this PC
  is idle and plugged in** (on unless you untick it).
- **Keep running when I close the window**: Leyline stays in the taskbar, minimized. **Quit Leyline** on the card
  closes it for real.
- **Start with Windows**: Leyline opens minimized when you log in to Windows, the node working.

It shows **Not open yet** until it opens for everyone at once. The Leyline team can stop every node at once; yours
then stops within half an hour and starts again by itself when they turn it back on.

## Advanced: carry players

Leyline's relays pass players' game connections to the realms they play on. You can help carry that load from your own PC.

**What it does:** passes other players' game connections through your PC. It never stores or shares game files or game clients.

### Turn it on

On the Be a node card, open **Advanced: full relay**, then **Open Help carry the network** (or Host, then the **Network** tab, then **Help carry the network**).

In the launcher: Host, then the **Network** tab, then **Help carry the network**.

**Can other players reach this PC?** on the same card tells you first whether your router lets players in.

1. Type your PC's public name (like `node.example.org`).
2. Set your caps: upload in KB/s, and the most players at once.
3. Leave **Only while this PC is idle and plugged in** on if you use this PC.
4. Press **Start helping**.

The line above the caps then says how many players your PC carries now. **Turn off** stops it in one click.

It shows **Not open yet** until it opens for everyone at once.

### What it needs

| | |
|---|---|
| A public name | a DNS name that points at your PC |
| Open ports | TCP 80, 443 and 41000 to 41051, reachable from the internet |
| Docker | the Host pack runs the relay in a container |

Hosting a realm needs no open ports. Carrying for others does, because players connect to your PC.

### Caps and pausing

- **Upload, KB/s**: everything your PC sends for others, together.
- **Players at most**: new players are turned away once it is full; they go to another relay.
- **Only while idle and plugged in**: when you use your PC, or it runs on battery, it takes no new players. Players already on it finish their session.

### What other people see

- Players connect to your PC's public name. Your home address is that name's address.
- Your PC sees their game traffic, chat included. It never sees passwords: the game's login does not send them.
- Hosts never see players' addresses, and players never see hosts' addresses.

### How the network stays safe

- Your PC signs up with a key it makes itself. The Leyline relay checks every two minutes that it answers with that key.
- Players are sent to a node only after it passed that check, and to the closest one with room. If it does not answer, they use our relays at once.
- The Leyline team can stop one node, or every node at once. A stopped node takes no new players within about a minute.

### Turning it off

Press **Turn off**. Or, in the Host pack folder:

```
docker compose -f docker-compose.volunteer-relay.yml down
```
