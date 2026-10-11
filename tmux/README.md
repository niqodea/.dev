# tmux

Module for [tmux](https://github.com/tmux/tmux), a terminal multiplexer.

## Concepts

### Meta keybinds

We use the <kbd>Meta</kbd> modifier key to define most custom keybinds in tmux.
Since other programs in our workflow do not leverage <kbd>Meta</kbd>, we can omit the tmux prefix in custom keybinds and achieve a much smoother experience.

### Session management

For each session we create a start directory with the same name in the home directory.
The start directory contains the files needed to carry out the particular project or effort associated with the session.

Session `0` is the home session: it belongs to no project, starts in the home directory, and is created again on demand if it was killed.

### Pane naming

We leverage shell hooks to display currently running commands as pane names.
This is useful when long running commands produce long outputs, eventually pushing the originating command out of the screen.

### Modules

We integrate additional features by sourcing external files in `~/.tmux-modules`.
