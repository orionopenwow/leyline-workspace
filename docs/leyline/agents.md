# Agents

Your own AI assistant, such as Claude, can help you with Leyline: check if a realm is up, find a mod, read your realm's
logs and tell you what went wrong. You decide what it may do. Nothing it does changes anything without your OK.

## Turn it on

1. Open Leyline and go to **Agents** (at the bottom of the side menu).
2. Turn on **Allow agents**. It is off until you turn it on. Off, agents get no answers at all.
3. Under **Connect**, pick your assistant:
   - **Claude Code**: press **Add for me**, or copy the command and run it yourself. Then start a new Claude Code
     session.
   - **Claude desktop app**: press **Add for me**, then quit and reopen the Claude app.
   - **Other agents**: copy the command or the settings shown there into your assistant's settings for local
     (MCP) servers.

Leyline keeps a copy of the settings file it changes, next to it, before it changes anything. **Remove** takes Leyline
out again and leaves the rest as it was.

Your assistant starts Leyline in the background by itself. Nothing listens on your network.

## What an agent can do

**Look, without asking you:**

- If the official realm is up, and how many people play
- The realm list
- Your game folders and the mods in each
- Search the Workshop
- Your own realm: if it runs, and its recent logs
- Your own forum threads

Before your realm's logs reach the agent, Leyline hides addresses, account names, e-mail addresses, links with
codes in them, keys and passwords.

**Only after you press Approve:**

- Install or update a mod
- Remove a mod (it can be installed again)
- Start or stop your own realm
- Post a forum thread or a reply as you

Each time, a card shows in Leyline with what exactly will happen. Press **Approve** or **Deny**. If you don't answer
in 2 minutes, it counts as Deny. Leyline must be open for this: with Leyline closed, the agent is told to ask you to
open it. One card at a time.

**Never:**

- Sign in, see or change a password, a Hero Key or your account
- Anything with money
- Delete files, remove a game folder, wipe a realm
- Make your realm public, open its sign-up, or make invites
- Kick, ban or unban players, or change who may join
- Change your realm list address, or your launcher's updates
- Use the website's staff tools

These have no button for an agent at all, so no trick can make it ask for them. Do these yourself.

## What agents did

The Agents page lists every call an agent made: when, which assistant, what it asked for, and how it ended (done,
denied, timed out, or blocked because agents were off).

## Good to know

- An agent can never approve its own request. Only your click in the Leyline window does.
- Text an agent reads from realms, mods, the forum or logs is treated as text, not as orders.
- Turning **Allow agents** off stops everything at once. You don't need to remove the connection.
- Making mods with an agent is a separate tool for mod makers: see [Mod format](mods.md).
