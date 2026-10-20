function() {

    # PROJECT
    if [ -n "$DOTDEV_PROJECT" ]; then
        alias ,cdp='cd "$HOME/$DOTDEV_PROJECT"'
    fi

    # DOTDEV_ATDIR
    # An atdir is a folder named $DOTDEV_ATDIR inside a working directory, where programs keep state that belongs to that directory
    # Many configs rely on DOTDEV_ATDIR
    alias ,mk@='mkdir "$DOTDEV_ATDIR"'

    # SHELL VARIABLE
    # SHELL is the zsh first in PATH, so a zsh in ~/.local/bin wins over /bin/zsh
    # Changing the login shell with chsh is risky, so SHELL is set here instead
    # /proc/$$/exe would name the running zsh, but macOS has no /proc
    export SHELL="$(whence -p zsh)"

    # HISTORY
    HISTSIZE=100000 # Commands kept in memory
    SAVEHIST=100000 # Commands saved to the file
    setopt HIST_IGNORE_ALL_DUPS
    setopt INC_APPEND_HISTORY # Other shells can load a command as soon as it runs
    setopt EXTENDED_HISTORY   # Timestamps mark how far a project's history has been merged into the global one
    if [ -n "$DOTDEV_PROJECT" ]; then
        # zsh reads HISTFILE after this file, so project commands end up on top and match first
        fc -R "$HOME/.zsh_history"
        HISTFILE="$HOME/$DOTDEV_PROJECT/$DOTDEV_ATDIR/project/history.zsh"
    else
        # zsh saves no history without HISTFILE, and not every OS sets it
        HISTFILE="$HOME/.zsh_history"
    fi

    # DIRECTORY STACK
    # Ref: https://thevaluable.dev/zsh-install-configure-mouseless/
    setopt AUTO_PUSHD
    setopt PUSHD_IGNORE_DUPS
    setopt PUSHD_SILENT

    # RECENT DIRECTORIES
    autoload -Uz chpwd_recent_dirs cdr add-zsh-hook
    add-zsh-hook chpwd chpwd_recent_dirs
    if [ -n "$DOTDEV_PROJECT" ]; then
        # A project keeps its own recent directories, the way it keeps its own history
        zstyle ':chpwd:*' recent-dirs-file "$HOME/$DOTDEV_PROJECT/$DOTDEV_ATDIR/project/recent-dirs.zsh"
    fi
    function ,cdr() {
        local choice="$(cdr -l | fzf --prompt='recent > ' --border=rounded --reverse --no-sort)"
        if [ -z "$choice" ]; then
            return
        fi
        cdr "${choice%% *}"
    }

    # COMPLETION
    # Ref: https://thevaluable.dev/zsh-completion-guide-examples/
    # Ref: https://github.com/Phantas0s/.dotfiles/blob/4c41b4865528714dbdeb18c8da42b4d1ca5a2182/zsh/completion.zsh
    autoload -Uz compinit
    compinit -C # Use cache to reduce startup time by ~0.1s
    # A background compinit refreshes the cache, in a subshell so its job messages stay hidden
    (
        autoload -Uz compinit
        compinit &
    )

    zstyle ':completion:*' menu 'select'
    unsetopt LIST_BEEP
    # ^C in the menu still beeps, and silencing it means wrapping the completion widget, which is brittle

    zstyle ':completion:*' group-name '' # Matches are grouped by kind, under the kind's name
    zstyle ':completion:*:*:*:*:descriptions' format '%F{cyan}-- %d --%f'
    zstyle ':completion:*:*:*:*:warnings' format '%F{red}no matches found%f'

    # Vim-like keybindings for menu navigation
    zmodload -i zsh/complist
    bindkey -M menuselect '^h' 'vi-backward-char'
    bindkey -M menuselect '^k' 'vi-up-line-or-history'
    bindkey -M menuselect '^j' 'vi-down-line-or-history'
    bindkey -M menuselect '^l' 'vi-forward-char'
    bindkey -M menuselect '^d' 'vi-forward-word'
    bindkey -M menuselect '^u' 'vi-backward-word'

    # Complete . and .. too
    # Ref: https://stackoverflow.com/a/716926
    zstyle ':completion:*' special-dirs 'true'

    # MISCELLANEOUS
    # vi mode
    # Ref: https://stackoverflow.com/a/58188295
    bindkey -v
    # Backspace otherwise deletes only what was typed since entering insert mode
    # Ref: https://unix.stackexchange.com/a/290403
    bindkey -v '^?' backward-delete-char
    # Remove compinit's Esc bindings, so only key codes like arrows conflict with a lone Esc
    bindkey -M viins -r '^[/' '^[,' '^[~'
    # zsh acts on a lone Esc after the shortest wait, 10 ms, since 0 would wait forever
    # The bytes of a key code arrive together, so zsh still reads them as one sequence
    KEYTIMEOUT=1

    # LOCAL
    local local_zsh_path="$HOME/.local.zsh"
    if [[ -f "$local_zsh_path" ]]; then
        source "$local_zsh_path"
    fi

    # EXTENSIONS
    source "$HOME/.zsh-modules/main.zsh"

}

# The anonymous function keeps these variables out of the shell
# Ref: https://stackoverflow.com/a/13670749
# autoload always takes -Uz: -U ignores the caller's aliases, and -z loads the function the zsh way
# Ref: https://unix.stackexchange.com/a/214306
