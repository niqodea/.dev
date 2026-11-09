# TODO

- (maybe) build a tablet note-taking app that takes the pen in one hand and the keyboard in the other, combining both, since no app found does this

- Explore customizing Claude Code's UI

- claudio: words are cut where the pane's margin wraps them. No terminal or tmux wraps at words, and other tools either cut words or break lines themselves, which breaks copying and resizing. Plan: fork tmux 3.8 with a private mode the client turns on around each paragraph of Claude's, so tmux wraps that text at spaces, flags its lines to wrap at spaces again on resize, and leaves the empty cells at a row's end out of copies. The client draws its own input box, so nothing else predicts the rows a paragraph takes. Pick an unused mode number against xterm's control sequences and the terminal working group first

- claudio: keys for useful Claude Code commands like /compact; check the docs for which commands work under `claude -p` and which earn a key

- claudio: an option for the input box's height, now fixed at 3 rows

- claudio: shorten the lines that are not Claude's prose, like tool lines, to at most 88 characters, ideally configurable, since some are very long

- claudio: show clearly, not by color alone, when an answer to a question or a plan does not read as one, so Enter answers nothing; (maybe) color the parts of an answer as it is typed, keys, own words and reason, unless it costs too much complexity

- claudio: support permission prompts, now refused since only auto mode is supported. They belong behind Ctrl-Y too, where plans and multiple-choice questions already wait, so work out how they fit that view; ,approval-answer, which marks y or n on each request and carries out the marked ones on Enter, is one model. Each decision should add one line to the conversation naming the call it answers, like `allowed #01 Bash rm -rf build`

- claudio: show Claude's prose word by word as it streams, now that the client draws its input box itself; tables and code blocks still need all their lines

- zsh: show whether vi mode is in insert or normal mode

- Replace logic that guesses a time with waiting for the event it depends on:
  - ,approval: polls every 10 ms whether an action ended, and stops it after 60 s
  - test-macos-install: retries SSH every 5 s for up to 5 minutes, and waits up to 300 s for the virtual machine's address
