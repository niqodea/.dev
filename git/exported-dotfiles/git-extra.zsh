function prompt_git() {
    local hash_burger_length=7 # Same as git default for short SHA

    local git_dir=$(git rev-parse --git-dir 2> /dev/null)
    if [ -z "$git_dir" ]; then
        printf 'X%*s' $((hash_burger_length + 1)) '' # Not a git repo
        return
    fi

    local result
    if [ -d "$git_dir/rebase-merge" ] || [ -d "$git_dir/rebase-apply" ]; then
        result='B'
    elif [ -f "$git_dir/MERGE_HEAD" ]; then
        result='M'
    elif [ -f "$git_dir/BISECT_LOG" ]; then
        result='S'
    elif [ -f "$git_dir/CHERRY_PICK_HEAD" ]; then
        result='C'
    elif [ -f "$git_dir/REVERT_HEAD" ]; then
        result='R'
    else
        result='.'
    fi

    local current_ref=$(git rev-parse --abbrev-ref HEAD)
    if [ "$current_ref" = "HEAD" ]; then
        result+=" $(git rev-parse --short HEAD)" # Detached HEAD
    else
        result+=">$(hburger hash -l 3 -c 2 -r 2 -p " " -- "$current_ref")" # On a branch
    fi

    printf '%s' "$result"
}

function() {
    if [ "$DOTDEV_ZSH_GIT_SETUP" = 'true' ]; then
        return
    fi
    export DOTDEV_ZSH_GIT_SETUP='true'

    local color_git='%F{034}' # Green
    setup_prompt_base "$DOTDEV_PROMPT_BASE$color_git"'($(prompt_git))'
}
