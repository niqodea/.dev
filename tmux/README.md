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
Start tmux with `,tmux-session-picker`, which offers the home session first when no session runs yet.

### Pane naming

We leverage shell hooks to display currently running commands as pane names.
This is useful when long running commands produce long outputs, eventually pushing the originating command out of the screen.

### Modules

We integrate additional features by sourcing external files in `~/.tmux-modules`.
