function ,bkp() {
    local file_path="$1"
    if [ -z "$file_path" ]; then
        >&2 echo "No path provided"
        return 1
    fi
    if [ ! -e "$file_path" ]; then
        >&2 echo "Path does not exist: $file_path"
        return 1
    fi

    local backup_file_path="$file_path.bkp"
    if [ -e "$backup_file_path" ]; then
        ,bkp "$backup_file_path"
    fi

    mv -- "$file_path" "$backup_file_path"
}

pre_ssh_workstation_functions=()
post_ssh_workstation_functions=()

function ,sw() {
    local workstation="$1"

    local ssh_options=(-t) # The remote command is interactive, so it needs a terminal
    # A keepalive every minute stops an idle connection from being dropped
    ssh_options+=(-o ServerAliveInterval=60 -o ServerAliveCountMax=3)

    for port in {50000..50019}; do
        ssh_options+=(-L "0.0.0.0:${port}:localhost:${port}")
    done

    for f in "${pre_ssh_workstation_functions[@]}"; do "$f" "$workstation"; done
    # ssh runs the command in a shell that has not read ~/.profile, so PATH lacks ~/.local/bin
    ssh "${ssh_options[@]}" "$workstation" 'source ~/.profile && ,tmux-launch'
    for f in "${post_ssh_workstation_functions[@]}"; do "$f"; done
}
