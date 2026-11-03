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

A CLI installs the changes it can ask for under `~/.config/approval/clis/<cli>/`:

- `action`: an executable, not on `PATH`, that `,approval` runs with `parse` or `run`, then the action's name, then the request's arguments.
  It can be the CLI itself under another name, so the CLI and its changes share their code while Claude is told only of the CLI.
  `parse` only parses the arguments, without reading or changing anything, and prints the name of each argument on a line of its own, or fails with the reason when the action cannot run with them.
  `,approval` queues nothing until `parse` succeeds, and stores the request's arguments as an object from each name to its word, or to its list of words when the name repeats.
  The words of a name that repeats come one after another, so `run` gets them back in the order they came in.
  `run` checks what can change while the request waits, like whether a pod still exists, makes the change, exits with 0 when it succeeded and prints the reason to stderr when not.
  We see a request only as its CLI, its action and its arguments with their names, so the arguments name everything the change touches.
- `auto-approved.json`: the requests that run without asking, as a list of rules like `{"action": "run", "arguments": {"environment": "dev"}}`.
  A rule matches a request for its action that has each of its arguments, with the same value.
  Rules have no patterns and no negation: a condition that needs more becomes an argument, or an action that checks it.

Any other request waits until we decide on it, in the queue of the project the shell is in: `<atdir>/approval` at the project's root, so it moves with the project.
Only `,approval-answer` decides, in any terminal: `,approval` itself can only submit a request and report what waits.
In tmux, M-y opens `,approval-answer` on the session's project in a popup when a request waits.
The `approval-status` skill, with the script it runs, shows Claude what waits and what was decided.
`,approval` gives an action only `PATH` and `HOME`, and stops it, with every process it started, after a time limit.

This steers how Claude works, and protects nothing: Claude's shell has the same logins the actions use.
