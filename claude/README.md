# Claude

Module for [Claude Code](https://claude.com/claude-code), an AI coding agent that runs in the terminal.

## Concepts

### Claude pane

Claude runs in a tmux [special pane](../tmux/README.md#special-panes) of type `claude`, started in the session's start directory.

### Waiting status

Claude reports through hooks whether it is working or waiting for us, and stores the answer in the pane's `@status` option, just as zsh does for its own panes.
Only the Claude we open in tmux gets these hooks, through the settings file passed with `--settings`, so a Claude started anywhere else leaves tmux alone.

The status bar shows the key that reaches a waiting Claude: ; for the Claude in this session, ' for a Claude in another session.
M-' lists the waiting Claudes in a popup and moves to the one we pick.

### Approval

Claude only reads.
A CLI that changes something asks `,approval` for the change instead of making it, with `,approval submit <cli> <action> <argument>...`, and we decide.
`,approval` needs the [python](../python/README.md) module.

A CLI installs the changes it can ask for under `~/.config/approval/clis/<cli>/`:

- `actions/<action>`: an executable, not on `PATH`, that `,approval` runs with `describe` or `run` and then the request's arguments.
  `describe` only reads: it checks the arguments and prints the request's row as tab-separated columns.
  `run` makes the change, exits with 0 when it succeeded and prints the reason to stderr when not.
- `auto-approved`: the requests that run without asking.
  A line lists an action and its first arguments, and matches a request that starts with the same words.
  Lines have no patterns and no negation: a condition that needs more becomes an argument, or an action that checks it.

Any other request waits in the queue that `APPROVAL_QUEUE` names, until we decide on it.
`,approval-answer` lets us decide in any terminal, and any other front end can be built on `,approval list`, `,approval count` and `,approval decide`.
In tmux, every session gets a queue of its own, and M-y opens `,approval-answer` in a popup when a request waits.
The `approval-status` skill, with the script it runs, shows Claude what waits and what was decided.
`,approval` gives an action only `PATH` and `HOME`, and stops it after a time limit.

This steers how Claude works, and protects nothing: Claude's shell has the same logins the actions use.
