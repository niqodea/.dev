# TODO

- (maybe) build a tablet note-taking app that takes the pen in one hand and the keyboard in the other, combining both, since no app found does this

- (maybe) add GPU usage to the load levels in the tmux status bar
- Explore customizing Claude Code's UI
- Build on the session's Claude pane
  - (paused) Tell a Claude waiting for a permission apart from one that finished, in the status bar and the picker, once Claude Code fires a hook when a permission prompt is answered (https://github.com/anthropics/claude-code/issues/103295). Until then, a Claude whose prompt was approved would show as waiting until its tool finishes
  - Show a Claude that waits on a background shell as running, not idle, once Claude Code can tell such a wait from a shell that runs forever, like a dev server (https://github.com/anthropics/claude-code/issues/98373). Until then, when a Claude shows as idle while it waits on a background shell, ask that Claude to wait for the shell in the foreground
