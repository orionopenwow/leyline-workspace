# Invite a friend to your Den

Your Den is your own private world on your PC. You can bring friends into it. One button makes one link for one
friend. Their Leyline finds the best way in by itself.

## What your friend needs

- **Leyline**, the launcher, on their own PC.
- **Their own game folder**, added under Game folders in Leyline (the same kind your Den plays with).
- **A Hero Key**, to log in to Leyline.

That's all. They don't make an account in your Den: the link has a login made just for them. It works for 24 hours.

**What never travels:** game files. Not through the link, not through the relay, not between your PCs. Each of you
plays with your own game folder. Your own models (My models) stay on your PC too: a friend sees a plain stand-in in
their place.

## Send the link

1. Open **Your Den** in Leyline.
2. Press **Invite friends**. The link is copied.
3. Send it to one friend (a message, Discord, email).
4. Under the buttons, **How your friend gets in** shows the ways the link has, in the order your friend's Leyline
   tries them.

Want to invite a second friend? Press **Invite friends** again: each friend gets their own link.

## How your friend joins

1. They open the link and press **Open in Leyline**, or paste it in Leyline under **Realms**, **Got an invite link?**,
   and press **Join**.
2. Leyline tries each way in turn and starts their game. It logs them in by itself.
3. Leyline tells them which way it used.

## The three ways in

Your friend's Leyline tries them in this order. You don't have to pick one.

### 1. Same network

For a friend in your home, or on the same network as you (the same Wi-Fi, a LAN party).

- Nothing to set up. The first time you invite, your PC may ask whether to let Den Lite talk on your network.
  Choose **Allow** for private networks.
- It is the fastest way, and it needs no internet.

If it says **off**: allow Den Lite in your firewall for private networks, then press **Invite friends** again.

### 2. Your own address

For players who run their own server, or who have set up their router. Skip this if you don't know what it means:
the relay below works without it.

1. Give your PC a fixed address on your network (your router's settings call this a reservation).
2. In your router, send ports **3724** and **8085** (TCP) to your PC. This is often called port forwarding.
3. Find your address on the internet. A name (for example from a free dynamic name service) is better than a number,
   because the number can change.
4. In Your Den, under **How your friend gets in**, type it in **Your own address** and press **Save**.
5. Press **Invite friends** again: the new link has your address in it.

On your own server instead of your PC: run Your Den's server there with the same two ports open, and put that
server's address in **Your own address**.

### 3. The Leyline relay

Works from anywhere, with nothing to set up. Your PC keeps one connection out to the Leyline relay, and the relay
passes your friend's game connection through it.

- It is private: only people with your link get in.
- The relay only passes the connection along. It keeps no game files and no logins.
- **Stop sharing** in Your Den takes your Den off the relay and stops every friend's link.

## If a way fails

- Your friend's Leyline moves on to the next way by itself, and tells them what it tried.
- If none works: press **Invite a friend** again. **Ready for friends?** checks every step a friend's game takes and says
  what to fix (the Host for friends guide has every fix).
- A link stops working after 24 hours, or when you press **Stop sharing**. Send a new one.

## In your Den

- Friends see you, your props and what you say.
- Only you build, unless you let a friend (the Friends list on Your Den, see DEN-HOSTING.md) or type `.den friends build on`.
- Each friend's character is made for them when they come in.
