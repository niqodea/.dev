# Claude

Module for [Claude Code](https://claude.com/claude-code), an AI coding agent that runs in the terminal.

## Concepts

### Claude pane

Claude runs in a tmux [special pane](../tmux/README.md#special-panes) of type `claude`, started in the session's start directory.

### Waiting status

Claude reports through hooks whether it is working or waiting for us, and stores the answer in the pane's `@status` option, just as zsh does for its own panes.
Only the Claude we open in tmux gets these hooks, through the settings file passed with `--settings`, so a Claude started anywhere else leaves tmux alone.

Claude Code fires no hook when we answer a permission prompt, so a mod passed with `--plugin-dir` reports Claude as working again when the call it asked about starts, which it learns from the call's row in the transcript redrawing as running.

A Claude that ends its turn while work it started still runs in the background, like a shell or a subagent, counts as working, since Claude Code wakes it when that work ends.
This assumes Claude starts no daemon in the background, since nothing tells a shell Claude waits on from one that runs forever: services run through `,dev-services`, outside Claude.
A Claude that starts one anyway shows as working until the daemon exits.

The status bar shows the key that reaches a waiting Claude: ; for the Claude in this session, w for the one on its [workstation](../tmux/README.md#workstation), ' for a Claude in another session.
When both Claudes of this session wait, it shows ;.
M-' lists the waiting Claudes in a popup and moves to the one we pick, a Claude on a workstation shown as its session followed by `[w]`.
Picking it moves to the workstation pane, as we left it.

The workstation pane mirrors since when the Claude of its session on the workstation waits, and the hooks signal the mirror whenever they change `@status`.
A Claude pane killed from outside Claude signals nothing, so the workstation pane shows it waiting until another Claude there signals.

### Plain client

`,claude-plain` runs Claude Code in plain text: replies are appended to the terminal, above an input line.
It drives `claude -p`, and passes its arguments on to `claude`.

When the shell has `ANTHROPIC_API_KEY` or `ANTHROPIC_AUTH_TOKEN` set, Claude Code pays with them instead of our subscription.
The interactive Claude asks first, `claude -p` does not, so a key exported for another project gets billed silently.
To always use the subscription, add this to `~/.claude/settings.json`, which empties both for every Claude Code session:

```json
{ "env": { "ANTHROPIC_API_KEY": "", "ANTHROPIC_AUTH_TOKEN": "" } }
```

Claude Code's [headless docs](https://code.claude.com/docs/en/headless#start-faster-with-bare-mode) say `--bare` will become the default for `-p` in a future release.
Bare mode ignores the subscription login, as well as CLAUDE.md files, hooks, plugins and auto-memory, so from that release `,claude-plain` needs whatever turns it back off.

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
Its [panel](../tmux/README.md#special-panes) shows what `,approval status` prints: what waits and what was last decided.
The `approval-status` skill, with the script it runs, shows Claude what waits and what was decided.
`,approval` gives an action only `PATH` and `HOME`, and stops it, with every process it started, after a time limit.

This steers how Claude works, and protects nothing: Claude's shell has the same logins the actions use.
