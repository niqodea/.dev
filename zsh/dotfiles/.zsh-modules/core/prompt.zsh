setopt PROMPT_SUBST

# TODO: Take what to add before or after the prompt base, instead of a whole new base
function setup_prompt_base() {
    export DOTDEV_PROMPT_BASE="$1"

    local COLOR_OFF='%f'
    local SHELL_STATE='%#'
    export PROMPT="$DOTDEV_PROMPT_BASE$COLOR_OFF$SHELL_STATE "
}

# TODO: Move to another language once hburger's parameters need to be configurable
function format_path() {
    local input="$1"
    local num_start_components="$2"
    local num_end_components="$3"

    local project_path="$HOME/$DOTDEV_PROJECT"

    local result relpath
    # The slashes keep a sibling like ~/devx from matching ~/dev
    if [[ -n "$DOTDEV_PROJECT" && "$input/" == "$project_path/"* ]]; then
        result="."
        relpath="${input#$project_path}"
    elif [[ "$input/" == "$HOME/"* ]]; then
        result="~"
        relpath="${input#$HOME}"
    else
        result=" "
        relpath="$input"
    fi

    if [ -z "$relpath" ]; then
        result+="/"
    else
        result+="$(hburger hash-path -s "$num_start_components" -e "$num_end_components" -p " " -- "$relpath")"
    fi

    printf '%s' "$result"
}

function prompt_userhost() {
    local burger_user="$(hburger hash -l 1 -c 1 -r 1 -p " " -- "$USER")"
    local burger_host="$(hburger hash -l 1 -c 1 -r 1 -p " " -- "$HOST")"
    printf '%s' "$burger_user@$burger_host"
}

function prompt_cwd() {
    local cwd="$PWD"
    local formatted_cwd="$(format_path "$cwd" 1 2)"
    local escaped_formatted_cwd="${formatted_cwd//\%/%%}"
    printf '%s' "$escaped_formatted_cwd"
}

function() {
    if [ -z "${DOTDEV_PROMPT_BASE:+x}" ]; then
        local color_userhost='%F{099}' # Purple
        local color_cwd='%F{220}'      # Yellow
        export DOTDEV_PROMPT_BASE="$color_userhost"'$(prompt_userhost)'"$color_cwd"'[$(prompt_cwd)]'
    fi

    setup_prompt_base "$DOTDEV_PROMPT_BASE"
}
