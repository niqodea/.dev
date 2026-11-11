# TODO

- (maybe) build a tablet note-taking app that takes the pen in one hand and the keyboard in the other, combining both, since no app found does this

- Explore customizing Claude Code's UI

- claudio: words are cut where the pane's margin wraps them. No terminal or tmux wraps at words, and other tools either cut words or break lines themselves, which breaks copying and resizing. Plan: fork tmux 3.8 with a private mode the client turns on around each paragraph of Claude's, so tmux wraps that text at spaces, flags its lines to wrap at spaces again on resize, and leaves the empty cells at a row's end out of copies. The client draws its own input box, so nothing else predicts the rows a paragraph takes. Pick an unused mode number against xterm's control sequences and the terminal working group first

- claudio: keys for useful Claude Code commands like /compact; check the docs for which commands work under `claude -p` and which earn a key

- claudio: show clearly, not by color alone, when an answer to a question or a plan does not read as one, so Enter answers nothing; (maybe) color the parts of an answer as it is typed, keys, own words and reason, unless it costs too much complexity

- claudio: support permission prompts, now refused since only auto mode is supported. They belong behind Ctrl-Y too, where plans and multiple-choice questions already wait, so work out how they fit that view; ,approval-answer, which marks y or n on each request and carries out the marked ones on Enter, is one model. Each decision should add one line to the conversation naming the call it answers, like `allowed #01 Bash rm -rf build`

- claudio: no message can ever be sent when Claude Code refuses the `get_context_usage` request sent at startup, or answers it in a shape claudio cannot read, like a renamed `maxTokens`. The cache then stays in its starting state, where `Context::holds_message` holds every message, and nothing ends that state. Treat a refused or unreadable answer like a resumed session whose cache age is unknown, so a message waits for a second Enter once and then goes

- claudio: warn at startup when Claude Code's version is newer than the one its undocumented behaviour was last checked against, 2.1.319 today, so a release that changes that behaviour shows up as a warning, not as a silent failure. Check whether the init message reports the version, as a `claude_code_version` field in the SDK's types; `claude --version` works otherwise. Keep the checked version in one constant next to the protocol code, and bump it after checking each "Undocumented" comment against a new release

- claudio: consider drawing the question view below the conversation, growing the bottom of the terminal, instead of on the alternate screen, so the conversation that led to the questions stays in view

- zsh: show whether vi mode is in insert or normal mode

- Replace logic that guesses a time with waiting for the event it depends on:
  - ,approval: polls every 10 ms whether an action ended, and stops it after 60 s
  - test-macos-install: retries SSH every 5 s for up to 5 minutes, and waits up to 300 s for the virtual machine's address
