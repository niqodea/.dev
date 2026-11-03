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
A CLI that changes something asks for the change instead of making it, and we decide.
It asks with `submit <action> <arguments>`, where `<arguments>` is a JSON object from each argument's name to its value.

A CLI installs the changes it can ask for under `~/.config/approval/clis/<cli>/`:

- `submit`: a link to `,approval`, which takes the CLI from the folder the link is in, so a CLI can only ask for its own actions.
- `action`: an executable, not on `PATH`, that `,approval` runs with the action's name, and the arguments as the same JSON object on its standard input.
  It can be the CLI itself under another name, so the CLI and its changes share their code while Claude is told only of the CLI.
  It checks the arguments, fails with the reason for a name it does not know, one it misses, or a value it cannot use, and checks what can change while the request waits, like whether a pod still exists.
  It makes the change, exits with 0 when it succeeded and prints the reason to stderr when not.
  We see a request only as its CLI, its action and its arguments with their names, which are exactly what the action gets, so the arguments name everything the change touches.
- `auto-approved.json`: the requests that run without asking, as a list of rules like `{"action": "run", "arguments": {"environment": "dev"}}`.
  A rule matches a request for its action that has each of its arguments, with the same value.
  Rules have no patterns and no negation: a condition that needs more becomes an argument, or an action that checks it.

Any other request waits until we decide on it, in the queue of the project the shell is in: `<atdir>/approval` at the project's root, so it moves with the project.
Only `,approval-answer` decides, in any terminal: `,approval` itself only reports what waits.
An open `,approval-answer` shows a request as soon as it is queued, since `,approval` signals it with `SIGUSR1`.
After a queue changes, `,approval` runs `~/.config/approval/on-change <project>` if it is installed, which keeps the count in the project's tmux session, so the status line shows `y` as soon as a request waits.
In tmux, M-y opens `,approval-answer` on the session's project in an `approval` [special pane](../tmux/README.md#special-panes) when a request waits, or moves to it if it is already open.
The `approval-status` skill, with the script it runs, shows Claude what waits and what was decided.
`,approval` gives an action only `PATH` and `HOME`, and stops it, with every process it started, after a time limit.

This steers how Claude works, and protects nothing: Claude's shell has the same logins the actions use.
