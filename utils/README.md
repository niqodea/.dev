# Utils

Module for the tools that belong to no other module.

## Concepts

### Programs

This module installs [pass](https://www.passwordstore.org/), [FFmpeg](https://ffmpeg.org/), [ImageMagick](https://imagemagick.org/), [Docker](https://www.docker.com/) and [Breadcrumbs](https://github.com/niqodea/breadcrumbs).
On macOS, Docker runs its containers in a [Colima](https://github.com/abiosoft/colima) virtual machine, and [Tart](https://github.com/openai/tart) runs macOS virtual machines.

### Passwords

pass keeps passwords encrypted with a gpg key named after the user.
`,gpg-create-key` creates the key and `,pass-init` sets up the store with it.
The gpg agent remembers the passphrase for a while after its last use, as set in `.gnupg/gpg-agent.conf`.

`,pass-shortcut add <item>` adds a password to the shortcuts, numbered in the order they were added.
In tmux, <kbd>Meta</kbd> + <kbd>Ctrl</kbd> + <kbd>F1</kbd> and <kbd>F2</kbd> type shortcuts 1 and 2 into the current pane.

### SSH

`.ssh/config.sample` makes later connections to a host reuse the first one, so they open faster.

`,ssh-clip connect <host>` connects to a host with plain ssh.
Inside the session, <kbd>Enter</kbd> `~C` opens ssh's command line, where `!,ssh-clip yank` copies the host's clipboard to this machine's, and `!,ssh-clip put` copies this machine's clipboard to the host's.

### Processes

`,proc` picks among our processes with fzf, each shown with the TCP ports it listens on, and prints the PIDs of the ones picked, as in `kill $(,proc <name>)`.

### lm

[lm](https://github.com/niqodea/lm) is a minimal Claude command line.
Its `,lm` aliases keep their data in the atdir.
