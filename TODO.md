# TODO

- (maybe) build a tablet note-taking app that takes the pen in one hand and the keyboard in the other, combining both, since no app found does this

- Explore customizing Claude Code's UI

- tmux: when a narrower screen wraps rows into more, take the extra rows from the empty rows below the cursor first, as a shorter screen already does (`screen_resize_y`), rather than pushing the screen's top into the scrollback. Today `grid_reflow` keeps the last rows that fit, empty ones included, so until claudio's conversation fills the screen, a narrower screen pushes the start of claudio's label row into the scrollback, where it stays as a copy (README's Resizing). Unchanged on tmux master as of 2026-11-16, and no tmux issue reports it then. https://github.com/tmux/tmux/issues/516 is the same family: shells' prompts broken by the rewrap on a resize, which claudio's climb already avoids. Its maintainer refuses new options and new meanings for escape sequences, and asks for fixes that work by default, as this one does. It helps shells too: a wide prompt near the top of a mostly empty screen goes to the scrollback the same way. The patch goes in `screen_resize_cursor` in `screen.c`; open an issue and send it upstream, or carry it in the tmux fork planned for word wrapping

- claudio: words are cut where the pane's margin wraps them. No terminal or tmux wraps at words, and other tools either cut words or break lines themselves, which breaks copying and resizing. Plan: fork tmux 3.8 with a private mode the client turns on around each paragraph of Claude's, so tmux wraps that text at spaces, flags its lines to wrap at spaces again on resize, and leaves the empty cells at a row's end out of copies. The client draws its own input box, so nothing else predicts the rows a paragraph takes. Pick an unused mode number against xterm's control sequences and the terminal working group first

- claudio: consider drawing the question view below the conversation, growing the bottom of the terminal, instead of on the alternate screen, so the conversation that led to the questions stays in view

- claudio: keep styled text as text and style apart until it is written to the terminal. Today it is a `String` with ANSI escapes inside, so where a code sits changes its meaning (a color set outside a string is lost after `cut`'s `…`, seen on 2026-11-15 with the question view's field names), every text operation parses the escapes again (5 hand-written scanners in `render.rs`, `input_line.rs` and `protocol.rs`), and the rule that a line holds its own colors lives only in `cut`'s doc comment. Plan: a small `Span` (text and one style) and `Line` (spans) type, as ratatui models text, with cut, width and wrap on it, and escapes written only at the end; move the question screen first, then the conversation renderer and `code.rs`, then delete the scanners

- claudio: add btw command if possible

- claudio: format tables to not go over X columns somehow

- claudio: reconsider setting code in bold. Bold otherwise means what to read first, like headings, tool names and the option under the cursor, while code in bold means literal text, as in man pages. Backticks tell code from strong text, so it works for now, but it is the first use to change if bold gets crowded

- claudio: better integrate subagents. Today the Agent call and its report show, and each tool call a subagent makes shows as an indented line, but: its calls have no result line, so a failing command inside it is invisible; its text is skipped; `parent_tool_use_id` is read only as a yes/no `is_subagent`, so the calls of parallel subagents interleave and can't be told apart (first step: give each subagent line its Agent call's tag, like `  ○ #0a Bash cargo test`); the label row says `running Agent` and skips a subagent's slow permission check; and background subagents (`run_in_background`) likely go through the background task lines, but the contract checks cover only `local_bash` tasks

- claudio: list the background tasks that are running in the status view (Ctrl+S), each with the tag of the call that started it and its description, since the label row only counts them (`B<n>`)

- zsh: show whether vi mode is in insert or normal mode

- Replace logic that guesses a time with waiting for the event it depends on:
  - ,approval: polls every 10 ms whether an action ended, and stops it after 60 s
  - test-macos-install: retries SSH every 5 s for up to 5 minutes, and waits up to 300 s for the virtual machine's address

- Fix bug where tmux fish would not recognize tokens broken up over multiple lines
