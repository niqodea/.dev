ZSH_MONITOR_EXIT_CODE=0

# An empty command line keeps the last exit code, which is not reported twice
function enable_monitor_exit_code() {
    ZSH_MONITOR_EXIT_CODE=1
}
preexec_functions+=(enable_monitor_exit_code)

function monitor_exit_code() {
    exit_code="$?"  # Any command in this function overwrites $?

    if [ "$ZSH_MONITOR_EXIT_CODE" != 1 ]; then
        return
    fi
    ZSH_MONITOR_EXIT_CODE=0

    if [ "$exit_code" != 0 ]; then
        echo -e "\e[31mExit code: $exit_code\e[0m"
    fi
}
precmd_functions+=(monitor_exit_code)
