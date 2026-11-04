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
Its border has its own color, and it is named after its type instead of its running command.

There are four types:

- `claude`, opened with <kbd>Meta</kbd> + <kbd>;</kbd>, runs [Claude Code](../claude/README.md).
- `workstation`, opened with <kbd>Meta</kbd> + <kbd>w</kbd>, connects to the project's workstation.
- `approval`, opened with <kbd>Meta</kbd> + <kbd>y</kbd>, decides on the changes waiting in the project's [approval](../claude/README.md#approval) queue.
- `view` shows one thing, like a conversation, and holds nothing that closing it would lose.

A session has at most one `claude`, one `workstation` and one `approval`: their keybind opens one in a split next to the current pane, or moves to it if it is already open.
Every special pane splits the current pane side by side when it is at least 160 columns wide, room for two terminals of 80, and one above the other otherwise.
A session can have any number of views.

<kbd>Meta</kbd> + <kbd>o</kbd> opens the current pane's panel: a popup about the pane, which <kbd>Esc</kbd> closes.
A panel shows the pane's state and may act on it, while anything meant to stay open as we work gets a pane or a command of its own.
A module gives a pane type its panel by setting `@panel_<type>` to a shell command that opens it.

### Workstation

A workstation is a remote machine we work on over ssh.
A project names its workstation's ssh host on the first line of `workstation/host` in its atdir.

The workstation pane opens the session of the same name on the workstation, and creates the project there if it is missing.
It forwards the project's ports, so a server listening on one of them on the workstation answers on the same port here.
A project's ports are a range picked from its name, which `,get-project-port-range` prints.
If one of them is already taken here, the connection fails.
The home session belongs to no project, so it forwards no ports.

The workstation pane's panel is the workstation's dashboard.
It shows the workstation's address, how long the connection has been up, the workstation's load, and the servers listening on the project's ports there.
From it, <kbd>y</kbd> copies the workstation's clipboard to this machine's, as it was when the dashboard opened, and <kbd>p</kbd> copies this machine's clipboard to the workstation's, as it was when <kbd>p</kbd> was pressed.
Each first says how much text would cross, and copies it once <kbd>Return</kbd> confirms.
There, <kbd>v</kbd> shows the text with its control characters made visible, so a hidden command shows as text.

### Pane naming

We leverage shell hooks to display currently running commands as pane names.
This is useful when long running commands produce long outputs, eventually pushing the originating command out of the screen.

### Modules

We integrate additional features by sourcing external files in `~/.tmux-modules`.
