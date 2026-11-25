# outlook

An Outlook CLI for Claude, for the mail and calendar of a Microsoft 365 account, in one Python file using only the standard library.
It reads, and requests a new mail or a reply.

## Setup

The CLI signs in as two apps registered in the account's organization, one that reads and one that sends.
It reads the organization and the apps' IDs from `~/.config/outlook-cli/config.toml`:

```toml
tenant = "example.com"
read_app = "<appId>"
send_app = "<appId>"
```

Each app is a public client, so it has no secret.
`--is-fallback-public-client` is what allows the device-code sign-in.

### Reading

The reading app only has the delegated permissions `Mail.Read` and `Calendars.Read`, so nothing run with its tokens can send, move or delete mail.
Register it with the Azure CLI, and put its `appId` in `read_app`:

```sh
az login --tenant example.com
az ad app create \
  --display-name outlook-cli \
  --sign-in-audience AzureADMyOrg \
  --is-fallback-public-client true \
  --public-client-redirect-uris https://login.microsoftonline.com/common/oauth2/nativeclient \
  --required-resource-accesses @permissions.json
```

The first command you run prints a code and a link: open the link, enter the code and sign in.
The CLI keeps the refresh token in the macOS Keychain, under the service `outlook-cli`, and only asks again after 90 days without use.

### Sending

`send` and `reply` ask for [approval](../../README.md#approval) of the mail, and it is sent once a person approves it.
The sending app only has the delegated permission `Mail.Send`, so it can send and reply, and cannot read, move or delete mail.
Register it, and put its `appId` in `send_app`:

```sh
az ad app create \
  --display-name outlook-write-cli \
  --sign-in-audience AzureADMyOrg \
  --is-fallback-public-client true \
  --public-client-redirect-uris https://login.microsoftonline.com/common/oauth2/nativeclient \
  --required-resource-accesses @permissions-write.json
```

Once installed, sign in once in a terminal, since the action runs without one:

```sh
~/.config/approval/clis/outlook/action sign-in
```

It keeps the refresh token in the Keychain under the service `outlook-write-cli`.
To show the person deciding who a reply goes to, `describe` reads the mail with the reading app.

## Use

The module's `install-bin` installs `core` in `~/.config/approval/clis/outlook/`, as the `describe` and the `action` that approval runs, and as the `command` of the `outlook` Claude Code skill.
Run `~/.claude/skills/outlook/command --help` for the commands.
Claude is told only of `command`, which reads and asks, and never of the mail it sends.
