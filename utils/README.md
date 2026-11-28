# Utils

Module for the tools that belong to no other module.

## Concepts

### Programs

This module installs [pass](https://www.passwordstore.org/), [FFmpeg](https://ffmpeg.org/), [ImageMagick](https://imagemagick.org/), [Podman](https://podman.io/) and [Breadcrumbs](https://github.com/niqodea/breadcrumbs).
On macOS, [Tart](https://github.com/openai/tart) runs macOS virtual machines.

### Containers

Podman runs containers, and takes the same commands as Docker, as in `podman run --rm --interactive debian:13`.
We use Podman and not Docker because Podman has no daemon: each command is a process of our own user, and so is each container it starts.

- Installing or upgrading Podman never stops a running container. Docker's packages restart the Docker daemon on every upgrade, which stops every container on the machine
- Our setup never touches the system's Docker. On a server that runs Docker, the server's own setup installs and upgrades it
- Podman is in Debian's own repositories, so it needs no extra apt source or key, and it runs without root, so it needs no group that grants root

Rootless Docker would keep a daemon, run as our user, and comes from the same apt repository as the system's Docker.

On macOS, Podman runs its containers in a Linux virtual machine, which `podman machine init` creates once and `podman machine start` starts.

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
