# .dev

Monorepo containing dotfiles and programs for my development environment.

## Module Structure

Each directory at the root of the repository is a module with the following standardized structure:

- `install-bin`: a script to install programs on the machine
- `dotfiles/`: the dotfiles to install
- `exported-dotfiles/`: dotfiles installed by other modules
- `bin-submodules/`: git submodules containing programs to install
- `dotfiles-submodules/`: git submodules containing dotfiles to install
- `README.md`: information about the module

### Dotfiles management

Each file in the `dotfiles` directory is of the form:

```
.path%2Fto%2Fdotfile
```

When installed, the path is first resolved by replacing instances of `%2F` as `/`.
Then, they are copied in the user's home directory, possibly replacing existing ones.
In the case of directories, dotfiles are replaced entirely and not merged.

Dotfiles with a `.sample` suffix are intended to be customized for local machine configurations.
This involves renaming them by removing the suffix and editing their contents to suit the settings of the specific machine.

### Rationale for exporting dotfiles

Sometimes, a module's configuration includes elements that are closely related to the functionality of another module.
For instance, `zsh`'s aliases may be more logically connected to the programs they alias, belonging to a different module.

To maintain a cohesive structure, we create these dotfiles within the module offering these functionalities.
Then, we create symbolic links to these files in the dotfiles directory of the module that imports them.

### Git submodules

We use git submodules to make our repository smaller and easier to manage, especially when working with external projects.

Submodules in `bin-submodules` help us build the programs.
This is useful because it lets us tailor the setup, install programs without needing special permissions, use the newest versions, and ensure consistent performance by locking the repository to a specific commit.

Submodules in `dotfiles-submodules` include files we link to in the dotfiles directory.
This helps us know which files we handle and which come from outside sources.

### Breadcrumbs

Our project uses symlinks extensively to make sure each file has only one main version.
To move up in the directory structure, we use a method called [Breadcrumbs](https://github.com/niqodea/breadcrumbs).
It is better than using lots of `../` in symlinks, which can be hard to read and might break unexpectedly.
Breadcrumbs are the symlinks named `..root` and `..module`, which point at the root of the repository and of the module they sit in.
A symlink that points up the directory tree goes through one of them, like `..root/<module>/<file>` or `..module/<file>`, so you can read where it points without counting `../`.

### Command names

Every command of our own starts with a comma, like `,<name>`, whether it is an alias, a function or a script.
No real command starts with one, so ours can never shadow a command that a package installs, now or later.
Two kinds of names are exceptions: `git-<name>` scripts, which git only runs as `git <name>` under that exact name, and the `..` aliases, since no command is named with dots alone.
Ref: [Start all of your commands with a comma](https://rhodesmill.org/brandon/2009/commands-with-comma/)

## Foundation

The `.foundation` directory holds what every module builds on, so it is not a module itself.
Installing it sets up the shell profile, the commands shared by all modules, Homebrew on macOS, and the packages that modules depend on.

Commands in `.foundation/bin` are either a single file, used on every operating system, or a directory with one file per operating system.

## Installation

To install the foundation, run this from the root of the repository before anything else:

```sh
./install-foundation
```

To install programs, run:

```sh
./install-bin [module1] [module2] [...]
```

You can also specify the installation path with `-p` (default is `~/.local`).

To install dotfiles, run:

```sh
./install-dotfiles [module1] [module2] [...]
```

The script will move existing, conflicting dotfiles in a backup directory.

Note: both scripts install all the modules if no module is specified.

## Linting

To check every shell script in the repository with [ShellCheck](https://www.shellcheck.net/), run:

```sh
./lint
```
