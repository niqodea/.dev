setopt PROMPT_SUBST

# TODO: Change to prepend/append to prompt base
function setup_prompt_base() {
    export DOTDEV_PROMPT_BASE="$1"

    local COLOR_OFF='%f'
    local SHELL_STATE='%#'
    export PROMPT="$DOTDEV_PROMPT_BASE$COLOR_OFF$SHELL_STATE "
}

# TODO: Rename this function (does not necessarily compress)
# TODO: This will probably need to be refactored/implemented in a different language
#       if we want to support injectable burger parameters
function format_path() {
    # TODO: Maybe build string instead of printing as we go

    local input="$1"
    local num_start_components="$2"
    local num_end_components="$3"

    local project_path="$HOME/$DOTDEV_PROJECT"

    # The slashes keep a sibling like ~/devx from matching ~/dev
    if [[ -n "$DOTDEV_PROJECT" && "$input/" == "$project_path/"* ]]; then
        printf "."
        local relpath="${input#$project_path}"
    elif [[ "$input/" == "$HOME/"* ]]; then
        printf "~"
        local relpath="${input#$HOME}"
    else
        printf " "
        local relpath="$input"
    fi

    if [ -z "$relpath" ]; then
        printf "/"
        return
    fi

    local burgerized_relpath="$(hburger hash-path -s "$num_start_components" -e "$num_end_components" -p " " -- "$relpath")"
    printf '%s' "$burgerized_relpath"
}

function prompt_userhost() {
    local burger_user="$(hburger hash -l 1 -c 1 -r 1 -p " " -- "$USER")"
    local burger_host="$(hburger hash -l 1 -c 1 -r 1 -p " " -- "$HOST")"
    printf "$burger_user@$burger_host"
}

function prompt_cwd() {
    local cwd="$PWD"
    local formatted_cwd="$(format_path "$cwd" 1 2)"
    local escaped_formatted_cwd="${formatted_cwd//\%/%%}"
    printf '%s' "$escaped_formatted_cwd"
}

function() {
    if [ -n "${DOTDEV_PROMPT_BASE:+x}" ]; then
        return
    fi

    local color_userhost='%F{099}'  # Purple
    local color_cwd='%F{220}'  # Yellow

    setup_prompt_base "$color_userhost"'$(prompt_userhost)'"$color_cwd"'[$(prompt_cwd)]'
}

function() {
    if [ -z "${DOTDEV_PROMPT_BASE:+x}" ]; then
        local color_userhost='%F{099}'  # Purple
        local color_cwd='%F{220}'  # Yellow
        export DOTDEV_PROMPT_BASE="$color_userhost"'$(prompt_userhost)'"$color_cwd"'[$(prompt_cwd)]'
    fi

    setup_prompt_base "$DOTDEV_PROMPT_BASE"
}
