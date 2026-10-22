# zsh reads this file in every shell, including the ones Claude Code runs commands in, which skip .zshrc

# The built-in log hides macOS's /usr/bin/log and fails silently, so it is dropped on every OS alike
disable log
