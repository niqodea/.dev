# Git

Module for [Git](https://git-scm.com/), a version control system.

## Concepts

### Aliases

Most git aliases are `,g` followed by the initials of the git command, like `,gs` for `git status` and `,gca` for `git commit --amend`.

The `git` zsh extra module, activated with `,amg`, shows the repository's state in the prompt.
`B`, `M`, `S`, `C` and `R` mean a rebase, merge, bisect, cherry-pick or revert is in progress, and `.` means none is.
After the letter comes a shortened branch name, or the commit on a detached HEAD.

### Base branch

Git does not record which branch a branch started from, so `git base-branch <branch>` records it in the current branch's git config, as its base.
`git base-branch` prints the base.
A branch with no base set, or a detached HEAD, has `origin/HEAD` as its base.

`git active-files` lists the files the current branch's commits changed since it left its base, relative to the current folder.

### Recent files

`git recent-files` lists the repository's tracked files, and its untracked ones that are not ignored, most recently modified first.

### Empty root commit

`git commit-root` makes the first commit of a new repository: with no files, no message, and a fixed author and date.
Every repository started this way has the same root commit.

### Atdirs

The global `.gitignore` ignores every atdir.
