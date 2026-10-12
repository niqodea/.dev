# Claude

Module for [Claude Code](https://claude.com/claude-code), an AI coding agent that runs in the terminal.

## Concepts

### One Claude per session

Each tmux session has at most one Claude, opened in a split next to the current pane and started in the session's start directory.
Asking for Claude again moves to the one that already runs instead of opening a second one.

Claude is the pane's process rather than a command typed into a shell, so the pane closes when Claude exits.
The pane's type is `claude`, which tells our tmux commands to treat it differently from a zsh pane.

### Waiting status

Claude reports through hooks whether it is working or waiting for us, and stores the answer in the pane's `@status` option, just as zsh does for its own panes.
Only the Claude we open in tmux gets these hooks, through the settings file passed with `--settings`, so a Claude started anywhere else leaves tmux alone.

The status bar shows the key that reaches a waiting Claude: ; for the Claude in this session, ' for a Claude in another session. M-' lists the waiting Claudes in a popup and moves to the one we pick.
