function ,bkp () {
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

function ,sw () {
    local workstation="$1"

    local ssh_command="ssh -t"  # -t to specify a command to run at the end
    # Options for sturdier connections
    ssh_command="$ssh_command -o ServerAliveInterval=60 -o ServerAliveCountMax=3"

    for port in {50000..50019}; do
        ssh_command="$ssh_command -L 0.0.0.0\:$port\:localhost\:$port"
    done

    ssh_command="$ssh_command $workstation"
    # Connect to tmux, launching the server if necessary
    # Source ~/.profile as otherwise ~/.local/bin is not included in the path
    ssh_command="$ssh_command 'source ~/.profile && tmux ls &> /dev/null && tmux attach || tmux new'"

    for f in "${pre_ssh_workstation_functions[@]}"; do "$f" "$workstation"; done
    sh -c "$ssh_command"
    for f in "${post_ssh_workstation_functions[@]}"; do "$f"; done
}
