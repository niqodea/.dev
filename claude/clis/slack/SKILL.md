---
name: slack
description: Reads and searches Slack channels, DMs and threads, and requests a message or a reply, with the slack CLI. Use when the user mentions Slack, a channel, a DM, a thread or a Slack link.
allowed-tools: Bash(${CLAUDE_SKILL_DIR}/command *)
---
Run the CLI as `${CLAUDE_SKILL_DIR}/command`.

Quick start:

- List the user's channels and DMs: `${CLAUDE_SKILL_DIR}/command channels`
- A thread from a copied link: `${CLAUDE_SKILL_DIR}/command thread '<link>'`
- Recent messages: `${CLAUDE_SKILL_DIR}/command read eng --days 1`, `${CLAUDE_SKILL_DIR}/command read @alice --days 7`
- Messages the user has not read, and new replies in threads they follow: `${CLAUDE_SKILL_DIR}/command unread`, or in one conversation `${CLAUDE_SKILL_DIR}/command unread eng`
- Messages that mention the user: `${CLAUDE_SKILL_DIR}/command mentions --days 7`
- Advanced search, with Slack's own syntax: `${CLAUDE_SKILL_DIR}/command search 'from:@alice in:#eng deploy' --limit 20`
- Request a message: `${CLAUDE_SKILL_DIR}/command post eng 'Deployed to prod'`, or a reply in a thread: `${CLAUDE_SKILL_DIR}/command reply '<thread ID or link>' 'Fixed'`

`read` and `mentions` give the latest 50 unless given `--limit`, `--days` or `--since`, and `search` the first 20 unless given `--limit`. Each says on stderr, before the results, when it left some out.

`post` and `reply` ask for the message, which waits for a person, as the approval-status skill follows. The message posts as the user, so run them only once the user has asked for that message.

To tag someone, write their whole name as `read` shows it, inside `<@` and `>`, like `<@alice>` or `<@Alice Smith>`. A bare `@alice` tags nobody. A name that matches nobody fails with the list of people, and one that several people share fails with their ids, written `<@U…>`, to use instead.

For anything else, run `${CLAUDE_SKILL_DIR}/command --help`, and `${CLAUDE_SKILL_DIR}/command <command> --help` for options.
