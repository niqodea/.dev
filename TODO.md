# TODO

- (maybe) check out https://github.com/abhinav/tmux-fastcopy or alternatives
- (maybe) consider using a mini-modal solution to pane resizing, both in tmux and nvim, to make it unnecessary for alt and ctrl to be pressed together (inspiration: https://github.com/whame/tmux-modal)
- (maybe) move all dotfiles to separate git submodule
- Write READMEs for all modules
- (maybe) build a tablet note-taking app that takes the pen in one hand and the keyboard in the other, combining both, since no app found does this

- (maybe) refactor nvim dev modules to always be directories with an `init.lua` and other files that can be exported
- (maybe) add GPU usage to the load levels in the tmux status bar
- Build on the session's Claude pane
  - (paused) Tell a Claude waiting for a permission apart from one that finished, in the status bar and the picker, once Claude Code fires a hook when a permission prompt is answered (https://github.com/anthropics/claude-code/issues/77700). Until then, a Claude whose prompt was approved would show as waiting until its tool finishes
  - Show a Claude that waits on a background shell as running, not idle, once Claude Code can tell such a wait from a shell that runs forever, like a dev server (https://github.com/anthropics/claude-code/issues/98373). Until then, when a Claude shows as idle while it waits on a background shell, ask that Claude to wait for the shell in the foreground
