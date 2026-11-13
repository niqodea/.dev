# TODO

- (maybe) build a tablet note-taking app that takes the pen in one hand and the keyboard in the other, combining both, since no app found does this

- Explore customizing Claude Code's UI

- claudio: words are cut where the pane's margin wraps them. No terminal or tmux wraps at words, and other tools either cut words or break lines themselves, which breaks copying and resizing. Plan: fork tmux 3.8 with a private mode the client turns on around each paragraph of Claude's, so tmux wraps that text at spaces, flags its lines to wrap at spaces again on resize, and leaves the empty cells at a row's end out of copies. The client draws its own input box, so nothing else predicts the rows a paragraph takes. Pick an unused mode number against xterm's control sequences and the terminal working group first

- claudio: show clearly, not by color alone, when an answer to a question or a plan does not read as one, so Enter answers nothing; (maybe) color the parts of an answer as it is typed, keys, own words and reason, unless it costs too much complexity

- claudio: support permission prompts, now refused since only auto mode is supported. They belong behind Ctrl-Y too, where plans and multiple-choice questions already wait, so work out how they fit that view; ,approval-answer, which marks y or n on each request and carries out the marked ones on Enter, is one model. Each decision should add one line to the conversation naming the call it answers, like `allowed #01 Bash rm -rf build` Seen with Claude Code 2.1.322: the main agent asks one request at a time, even for tool calls made in parallel, while parallel subagents ask at the same time, and subagents have no AskUserQuestion, so what waits together is at most one request of the main agent's, a question batch, a plan or a permission, and any number of subagents' permissions. A subagent's request carries an `agent_id`, and the call it asks about carries the `Agent` call that started the subagent as its `parent_tool_use_id`, whose description can name it. Each fact is a contract check once claudio relies on it Design, settled: every request, a question batch, a plan or a permission, is a unit of its own in the question view, which shows one request at a time, and each one goes to Claude Code as soon as it is answered, apart from the others. A plan and a permission are requests of one question, which Enter sends at once: allow or deny, with the user's own answer as the reason for a denial. `q` `w` `e` `r` jump to the first four requests and `}`/`{` move to the next and previous one, beside `a` `s` `d` `f` and `]`/`[` for the questions of the request shown. The rule above the question names the request: its kind, the agent that asked, none for the main agent, and its place in the line, like `── Bash · Explore the code · 2/5 ──`. The label row's right end shows both levels, like `q w e r +1 │ a s`, with the ones shown in reverse. A level shows its names, and the rule its place in the line, only when it has more than one item, by the same code for both levels, so one request of one question shows neither Ctrl-C refuses the request shown, and Enter on a message refuses every request after a second press. A burst of subagents' permissions is answered one at a time, and a list that marks y or n on each, as ,approval-answer does, comes only if bursts turn out tedious

- claudio: consider drawing the question view below the conversation, growing the bottom of the terminal, instead of on the alternate screen, so the conversation that led to the questions stays in view

- claudio: add btw command if possible

- claudio: format tables to not go over X columns somehow

- claudio: reconsider setting code in bold. Bold otherwise means what to read first, like headings, tool names and the option under the cursor, while code in bold means literal text, as in man pages. Backticks tell code from strong text, so it works for now, but it is the first use to change if bold gets crowded

- claudio: better integrate subagents. Today the Agent call and its report show, and each tool call a subagent makes shows as an indented line, but: its calls have no result line, so a failing command inside it is invisible; its text is skipped; `parent_tool_use_id` is read only as a yes/no `is_subagent`, so the calls of parallel subagents interleave and can't be told apart (first step: give each subagent line its Agent call's tag, like `  ○ #0a Bash cargo test`); the label row says `running Agent` and skips a subagent's slow permission check; and background subagents (`run_in_background`) likely go through the background task lines, but the contract checks cover only `local_bash` tasks

- claudio: list the background tasks that are running in the status view (Ctrl+S), each with the tag of the call that started it and its description, since the label row only counts them (`B<n>`)

- claudio: fix lines lost when the terminal is resized. Seen on 2026-11-12 with Claude Code 2.1.322: after a resize while Claude worked, its reply showed only its last paragraph, with no `‹‹` before it, so the rest was lost from the scrollback. A past session looked into this and stopped right before a fix

- zsh: show whether vi mode is in insert or normal mode

- Replace logic that guesses a time with waiting for the event it depends on:
  - ,approval: polls every 10 ms whether an action ended, and stops it after 60 s
  - test-macos-install: retries SSH every 5 s for up to 5 minutes, and waits up to 300 s for the virtual machine's address

- Fix bug where tmux fish would not recognize tokens broken up over multiple lines
