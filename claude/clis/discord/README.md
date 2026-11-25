# discord

A read-only Discord CLI for Claude, in one Python file using only the standard library.
It reads the one server its bot is a member of.

## Setup

1. Go to https://discord.com/developers/applications → **New Application**, and name it, like `Read-only CLI`.
2. On **Installation**, set **Install Link** to **None**. Then on **Bot**, turn off **Public Bot**, so nobody else can add the bot to a server. Discord refuses to turn it off while the install link is set.
3. On **Bot**, turn on **Message Content Intent**. Without it, Discord sends every message with empty text.
4. On **Bot**, **Reset Token**, and copy the token.
5. Add the bot to the server, with the application ID from **General Information**. This needs the Manage Server permission on the server:

   ```
   https://discord.com/oauth2/authorize?client_id=<application id>&scope=bot&permissions=66560
   ```

   66560 is View Channels plus Read Message History, so the bot can read and nothing else. It sees the channels everyone sees. For a private channel, add the bot to the channel's permissions. A new channel starts with its category's permissions, so adding the bot to a category covers the channels created in it later.
6. Keep the token in the macOS Keychain and export it from your shell profile:

```sh
security add-generic-password -s discord-cli -a "$USER" -w '<token>'
export DISCORD_TOKEN="$(security find-generic-password -s discord-cli -w)"
```

The CLI only sends GET requests, and the bot only has read permissions, so nothing run with the token can post, react or edit.

## Use

The module's `install-bin` installs `command` and `SKILL.md` as the `discord` Claude Code skill.
Run `~/.claude/skills/discord/command --help` for the commands.
