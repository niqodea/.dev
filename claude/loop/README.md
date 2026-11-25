# loop

Named Claude loops.
Each loop is a `bodies/<name>.md` with the tasks Claude repeats, and `,claude-loop <name>` starts Claude on them every 30 minutes.

## Use

The module's `install-bin` installs the `,claude-loop` command and copies each `bodies/<name>.md` to `~/$DOTDEV_ATDIR/claude-loops/<name>/.claude/loop.md`, with a link to it at `~/$DOTDEV_ATDIR/claude-loops/<name>/body.md` that is easier to find.
Then run `,claude-loop inbox`.

`,claude-loop <name>` starts Claude in the loop's folder with `/loop 30m`, so Claude Code runs that folder's `.claude/loop.md` every 30 minutes.
Each loop gets its own folder because Claude looks for `loop.md` only in the folder it starts in.
That folder is outside git and installing overwrites its copy, so edit the loops here.

To start a loop with another command than `claude "/loop 30m"`, like a wrapper's, set it in `CLAUDE_LOOP_COMMAND`.
`,claude-loop` runs it in the loop's folder through the shell, as git runs `GIT_SSH_COMMAND`, so quote an argument that holds spaces: `CLAUDE_LOOP_COMMAND='claudio --command "loop 30m"'`.

A loop runs at most once: a second `,claude-loop` of the same loop stops with "already running".

The first time a loop starts, Claude asks whether to trust its folder.

## Loops

- `inbox`: reports the unread Slack messages and Outlook mail that need an action or a reply.
