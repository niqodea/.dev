# tmux

Module for [tmux](https://github.com/tmux/tmux), a terminal multiplexer.

## Concepts

### Meta keybinds

We use the <kbd>Meta</kbd> modifier key to define most custom keybinds in tmux.
Since other programs in our workflow do not leverage <kbd>Meta</kbd>, we can omit the tmux prefix in custom keybinds and achieve a much smoother experience.

### Session management

Every session but the home one belongs to a project: a directory with the same name in the home directory, holding the files needed to carry out that project or effort.
The session starts in the project directory, and its shells keep their history in the project.

Session `~` is the home session: it belongs to no project, starts in the home directory, and is created again on demand if it was killed.
Start tmux with `,tmux-launch`, which opens the home session.

### Special panes

A special pane runs a program instead of a shell, and closes when that program exits.
A session has at most one special pane of each type: its keybind opens it in a split next to the current pane, or moves to it if it is already open.
Its border has its own color, and it is named after its type instead of its running command.

There are two types:

- `claude`, opened with <kbd>Meta</kbd> + <kbd>;</kbd>, runs [Claude Code](../claude/README.md).
- `workstation`, opened with <kbd>Meta</kbd> + <kbd>w</kbd>, connects to the project's workstation.

### Workstation

A workstation is a remote machine we work on over ssh.
A project names its workstation's ssh host on the first line of `workstation` in its atdir.

The workstation pane opens the session of the same name on the workstation, and creates the project there if it is missing.
It forwards the project's ports, so a server listening on one of them on the workstation answers on the same port here.
A project's ports are a range picked from its name, which `,get-project-port-range` prints.
If one of them is already taken here, the connection fails.
The home session belongs to no project, so it forwards no ports.

From the workstation pane, <kbd>Meta</kbd> + <kbd>Ctrl</kbd> + <kbd>y</kbd> copies the workstation's clipboard to this machine's.
<kbd>Meta</kbd> + <kbd>Ctrl</kbd> + <kbd>p</kbd> copies this machine's clipboard to the workstation's.

### Pane naming

We leverage shell hooks to display currently running commands as pane names.
This is useful when long running commands produce long outputs, eventually pushing the originating command out of the screen.

### Modules

We integrate additional features by sourcing external files in `~/.tmux-modules`.
