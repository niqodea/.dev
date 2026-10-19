function set_tmux_pane_idle() {
    tmux select-pane -t "$TMUX_PANE" -T ''
    tmux set-option -t "$TMUX_PANE" -p @status idle
}

function set_tmux_pane_running() {
    COMMAND="$1"
    # tmux does not show a title with a newline in it
    tmux select-pane -t "$TMUX_PANE" -T "${COMMAND//$'\n'/\\n}"
    tmux set-option -t "$TMUX_PANE" -p @status running
}

function set_tmux_pane_cwd() {
    tmux set-option -t "$TMUX_PANE" -p @cwd "$PWD"
}

function set_tmux_pane_zsh_extra_modules {
    tmux set-option -t "$TMUX_PANE" -p @zsh_extra_modules "$DOTDEV_ZSH_EXTRA_MODULES"
}

function set_tmux_ssh_workstation () {
    workstation="$1"
    tmux set-option -p @ssh_workstation "$workstation"
}

function unset_tmux_ssh_workstation () {
    tmux set-option -p -u @ssh_workstation
}

function () {
    if [ "$DOTDEV_ZSH_TMUX_SETUP" = 'true' ]; then
        return
    fi
    export DOTDEV_ZSH_TMUX_SETUP='true'

    if [ -z "$TMUX_PANE" ]; then
        >&2 echo 'Error: not in a TMUX pane'
        return 1
    fi

    tmux set-option -t "$TMUX_PANE" -p @pane_type zsh
    tmux set-option -t "$TMUX_PANE" -p @zsh_pid "$$"

    set_tmux_pane_idle
    preexec_functions+=(set_tmux_pane_running)
    precmd_functions+=(set_tmux_pane_idle)

    set_tmux_pane_cwd
    chpwd_functions+=(set_tmux_pane_cwd)
    set_tmux_pane_zsh_extra_modules
    precmd_functions+=(set_tmux_pane_zsh_extra_modules)

    pre_ssh_workstation_functions+=(set_tmux_ssh_workstation)
    post_ssh_workstation_functions+=(unset_tmux_ssh_workstation)
}
