# Listing files
alias ,l='ls --color=auto'
alias ,la='ls --color=auto -a'
alias ,ll='ls --color=auto -hl'
alias ,lal='ls --color=auto -ahl'

# Easier navigation in directories
alias ..='cd ..'
alias ...='cd ../..'
alias ....='cd ../../..'
alias .....='cd ../../../..'

# Size of files
alias ,duh='du -hs'

# Find
alias ,fn='find . -name'

# Search specific processes
alias ,psg='ps -eo pid,user,etime,comm | grep'
# Kill them
alias ,k9='kill -9'  # SIGKILL

# Load updated global history
alias ,fcr='fc -R'

alias ,pp='pwd -P'
