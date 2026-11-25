---
name: discord
description: Reads the channels, threads and forum posts of the Discord server the user's bot is in, with the read-only discord CLI. Use when the user mentions Discord, the community server, a Discord channel or a Discord link.
allowed-tools: Bash(${CLAUDE_SKILL_DIR}/command *)
---
Run the CLI as `${CLAUDE_SKILL_DIR}/command`.

Quick start:

- List the channels and their active threads and forum posts: `${CLAUDE_SKILL_DIR}/command channels`
- Recent messages: `${CLAUDE_SKILL_DIR}/command read general --days 1`, `${CLAUDE_SKILL_DIR}/command read general --limit 100`
- A thread or forum post, by the ID `read` or `channels` prints, or a copied link: `${CLAUDE_SKILL_DIR}/command read <id or link> --limit 100`
- The messages around a copied message link: `${CLAUDE_SKILL_DIR}/command around '<link>' --limit 20`
- Every message across all channels and threads, over a few days: `${CLAUDE_SKILL_DIR}/command recent --days 3`. Discord tells bots nothing about unread messages, so this replaces an unread command

`read` gives the latest 50 messages unless given `--limit`, `--days` or `--since`, and says on stderr, before the messages, when it left some out.

For anything else, run `${CLAUDE_SKILL_DIR}/command --help`, and `${CLAUDE_SKILL_DIR}/command <command> --help` for options.
