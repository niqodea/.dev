---
name: approval-status
description: Shows the changes CLIs asked for that wait for a person's decision, and what was decided. Use when a CLI printed that a change waits, or when the user asks what a change became.
allowed-tools: Bash(${CLAUDE_SKILL_DIR}/approval-status)
---
A CLI that changes data never makes the change itself: it asks for it, and says whether the change was made right away (`succeeded:` or `failed:`) or waits for a person (`waiting:`). Ask for changes only through those CLIs.

When a change waits, tell the user, do not ask for it again, and run `${CLAUDE_SKILL_DIR}/approval-status` on their next message. It lists what waits, and the last changes decided, with what each printed or why it failed.
