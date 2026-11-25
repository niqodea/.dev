---
name: outlook
description: Reads the user's Outlook mail and calendar, and requests a new mail or a reply, with the outlook CLI. Use when the user mentions email, their inbox, Outlook, a meeting or their calendar.
allowed-tools: Bash(${CLAUDE_SKILL_DIR}/command *)
---
Run the CLI as `${CLAUDE_SKILL_DIR}/command`.

Quick start:

- Inbox messages the user has not read: `${CLAUDE_SKILL_DIR}/command unread`
- Recent inbox messages: `${CLAUDE_SKILL_DIR}/command read --days 2`
- Search every folder, with Outlook's own syntax: `${CLAUDE_SKILL_DIR}/command search 'from:alice subject:invoice' --limit 20`
- One message in full, by the ID a list prints: `${CLAUDE_SKILL_DIR}/command message <id>`
- One attachment the message lists, saved to a new file: `${CLAUDE_SKILL_DIR}/command download <id> invoice.pdf /tmp/invoice.pdf`
- Today's events: `${CLAUDE_SKILL_DIR}/command calendar`, or a week: `${CLAUDE_SKILL_DIR}/command calendar --since 2026-10-20 --days 5`
- Request a new mail: `${CLAUDE_SKILL_DIR}/command send --to alice@example.com --cc bob@example.com --subject 'Invoice' 'Hi Alice, ...'`
- Request a reply to the sender of a mail: `${CLAUDE_SKILL_DIR}/command reply <id> 'Thanks, I will check'`

`read` gives the latest 50 messages unless given `--limit`, `--days` or `--since`, and `search` the first 20 unless given `--limit`. Each says on stderr, before the results, when it left some out.

`send` and `reply` ask for the mail, which waits for a person, as the approval-status skill follows. The mail goes out from the user's account, so run them only once the user has asked for that mail. `send` takes email addresses, which `message` prints for the people on a mail.

For anything else, run `${CLAUDE_SKILL_DIR}/command --help`, and `${CLAUDE_SKILL_DIR}/command <command> --help` for options.
