function ,cdv () {
    local venv_path="$VIRTUAL_ENV"

    if [ -z "${venv_path+x}" ]; then
        >&2 echo "No active virtual environment found"
        return 1
    fi

    if [[ "$venv_path" == *"/$DOTDEV_ATDIR/venv" ]]; then
        local base_path="${venv_path%/$DOTDEV_ATDIR/venv}"
    elif [[ "$venv_path" == *"/.venv"  ]]; then
        local base_path="${venv_path%/.venv}"
    else
        local base_path="$venv_path"
    fi

    cd "$base_path"
}

alias ,pv='echo "$VIRTUAL_ENV"'

function _activate_venv() {
    local venv_relative_path="$1"

    local venv_path="$PWD/$venv_relative_path"

    if [ ! -d "$venv_path" ]; then
        >&2 echo "No virtual environment found at $venv_path"
        return 1
    fi

    _activate_extra_module venv "{\"path\": \"$venv_path\"}"
}
alias ,amv='_activate_venv .venv'
alias ,amv@='_activate_venv $DOTDEV_ATDIR/venv'
