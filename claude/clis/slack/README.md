# slack

A Slack CLI for Claude, in one Python file using only the standard library.
It reads, and requests a message or a reply in a thread.

It calls only the methods Slack documents.
The ones Slack's own apps use without documenting them, like the Threads view's `subscriptions.thread.getView`, need the login of a browser session.
That login can do anything you can do in Slack, so it would get around the scopes of the two tokens below, and it stops working whenever the browser session ends.

## Setup

### Reading

1. Go to https://api.slack.com/apps → **Create New App** → **From a manifest**, pick your workspace, and paste `manifest.json`.
2. **Install to Workspace**, then copy the **User OAuth Token** (`xoxp-...`) from **OAuth & Permissions**.
3. Keep the token in the macOS Keychain and export it from your shell profile:

```sh
# security has no long flags: -s is the service, -a the account, -w the password
security add-generic-password -s slack-cli -a "$USER" -w 'xoxp-...'
export SLACK_TOKEN="$(security find-generic-password -s slack-cli -w)"
```

The token only has read scopes, so nothing run with it can post, react or edit.

The app must stay internal to your workspace.
Slack limits apps distributed outside its Marketplace to one history call a minute, and exempts apps built for your own workspace ([changelog](https://docs.slack.dev/changelog/2025/06/03/rate-limits-clarity/)).

### Posting

`post` and `reply` ask [`,claude-approval`](../../README.md#approval) for the message, and it is posted once a person approves it.
To show the person where the message goes, `describe` looks up the conversation's name with the reading token, from the Keychain.
A second app, which can only post, holds the token that posts:

1. Go to https://api.slack.com/apps → **Create New App** → **From scratch**, name it `Slack Write-only CLI`, and pick your workspace.
2. In **OAuth & Permissions**, add `chat:write` under **User Token Scopes**, and nothing else.
3. **Install to Workspace**, then copy the **User OAuth Token** (`xoxp-...`).
4. Keep it in the macOS Keychain, and never export it, so only the action reads it:

```sh
security add-generic-password -s slack-write-cli -a "$USER" -w 'xoxp-...'
```

## Use

The module's `install-bin` installs `core` in `~/.config/approval/clis/slack/`, as the `describe` and the `action` that `,claude-approval` runs, and as the `command` of the `slack` Claude Code skill.
Run `~/.claude/skills/slack/command --help` for the commands.
Claude is told only of `command`, which reads and asks, and never of the posts.
