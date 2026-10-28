# zsh

Module for [zsh](https://www.zsh.org/), a shell.

## Concepts

### Modules

The configuration is split into modules in `~/.zsh-modules`.
Core modules load in every shell.
Extra modules load only once activated, with aliases like `,amg` for the `git` module.

`DOTDEV_ZSH_EXTRA_MODULES` lists the active extra modules and is exported, so a shell started from this one loads the same ones.
Other modules add their own zsh modules through exported dotfiles.

### Project history

Inside a project, the shell saves its history in the project's atdir instead of `~/.zsh_history`.
It loads the global history first, so the project's commands come after it and match first when searching.
`,export-project-zsh-history` copies the project's new commands into the global history.

A project also keeps its own recent directories.
`,cdr` picks one with fzf.

### Vi mode

The command line uses vi keybindings.
A lone <kbd>Esc</kbd> switches to normal mode after the shortest wait zsh allows.

### Local settings

Settings for a single machine go in `~/.local.zsh`, made from `.local.zsh.sample`.
It loads before the modules.

### .zshenv

zsh reads `.zshenv` in every shell, even non-interactive ones, which skip `.zshrc`.
