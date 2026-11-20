# TODO

- (maybe) build a tablet note-taking app that takes the pen in one hand and the keyboard in the other, combining both, since no app found does this

- Explore customizing Claude Code's UI

- tmux: when a narrower screen wraps rows into more, take the extra rows from the empty rows below the cursor first, as a shorter screen already does (`screen_resize_y`), rather than pushing the screen's top into the scrollback. Today `grid_reflow` keeps the last rows that fit, empty ones included, so until claudio's conversation fills the screen, a narrower screen pushes the start of claudio's label row into the scrollback, where it stays as a copy (README's Resizing). Unchanged on tmux master as of 2026-11-16, and no tmux issue reports it then. https://github.com/tmux/tmux/issues/516 is the same family: shells' prompts broken by the rewrap on a resize, which claudio's climb already avoids. Its maintainer refuses new options and new meanings for escape sequences, and asks for fixes that work by default, as this one does. It helps shells too: a wide prompt near the top of a mostly empty screen goes to the scrollback the same way. The patch goes in `screen_resize_cursor` in `screen.c`; open an issue and send it upstream, or carry it in the tmux fork planned for word wrapping

- zsh: show whether vi mode is in insert or normal mode

- Replace logic that guesses a time with waiting for the event it depends on:
  - ,approval: polls every 10 ms whether an action ended, and stops it after 60 s
  - test-macos-install: retries SSH every 5 s for up to 5 minutes, and waits up to 300 s for the virtual machine's address

- Fix bug where tmux fish would not recognize tokens broken up over multiple lines

## claudio

### Before going public

- Rewrite the README so its first screen says what claudio is and what it is not: unofficial, checked only in tmux 3.5 for resizing, pinned to one version of Claude Code, and built on behavior that is observed rather than documented

- Say in the README that each version of Claude Code needs claudio's checks and a release of its own, since claudio runs only the version it was checked against, so users wait for that release before a newer Claude Code

- Keep the README's note on bare mode in view: Claude Code's docs say `--bare` will become the default for `-p`, and bare mode ignores the subscription login, so the release that does it needs claudio to turn it back off

- Keep the README's note on server-side flags in view: the checks against the fake run on Claude Code's default flags, so a user's session, which gets the flags Anthropic serves, can differ with no new version of Claude Code

- Decide how users install claudio: today `cargo install --path .` with `publish = false`, which needs a clone of the repo and Rust

### Later

- Words are cut where the pane's margin wraps them. No terminal or tmux wraps at words, and other tools either cut words or break lines themselves, which breaks copying and resizing. Plan: fork tmux 3.8 with a private mode the client turns on around each paragraph of Claude's, so tmux wraps that text at spaces, flags its lines to wrap at spaces again on resize, and leaves the empty cells at a row's end out of copies. The client draws its own input box, so nothing else predicts the rows a paragraph takes. Pick an unused mode number against xterm's control sequences and the terminal working group first

- Consider jumping between the user's messages in the full view with less's tags, which a `/` search leaves alone: claudio notes the full view's line of each message it shows, writes a ctags file of them, newest first after a tag where the view opens, and starts less with `-T <file> -t <tag>`, so `t` goes back a message and `T` forward. Tried with less 581: `t` and `T` count from the last tag jumped to, not from the line in view, so after a scroll or a search the next jump can surprise; that limit is why it waits

- Ring the terminal's bell, ASCII 7, when Claude's turn ends and when a request starts waiting, if someone asks for it. The bell takes nothing from the user: tmux marks the window in its status bar with `monitor-bell`, and `bell-action` decides whether the window in view rings too, so claudio rings always and leaves the rest to the terminal. The turn ends where `Output::note_state` goes back to `Status::Idle`, and a request arrives as `Event::Ask` in `read_claude`; a flag in `Output` that the input loop takes and writes, next to `take_text`, rings once for requests that arrive together

- Consider drawing the question view below the conversation, growing the bottom of the terminal, instead of on the alternate screen, so the conversation that led to the questions stays in view

- Add btw command if possible

- Reconsider setting code in bold. Bold otherwise means what to read first, like headings, tool names and the option under the cursor, while code in bold means literal text, as in man pages. Backticks tell code from strong text, so it works for now, but it is the first use to change if bold gets crowded

- List the background tasks in the status view (Ctrl+S), each with the tag of the call that started it and its description, since the label row only counts them (`B<n>`), and let Ctrl+K there stop the one selected, once it is clear how the status view takes keys

- Bound the list of subagents (`src/subagents.rs`), which grows by about 100 bytes for each subagent of a session, since claudio keeps an ended one so that its tag names it again if Claude resumes it. Claude Code forgets an ended subagent after 30 seconds and loads it back from its transcript on disk on a resume, which claudio cannot read

- Name a subagent by its tag when Claude messages it by name. Today the line shows the name as written, since Claude Code 2.1.322 renames a subagent whose name a running one holds, like `reviewer-2`, and frees a name only 30 seconds after its holder ends, so claudio cannot tell which subagent a name reaches. Checked on 2026-11-19: the SDK 0.3.325's types report the given name nowhere, neither `AgentOutput` in `sdk-tools.d.ts` nor `SDKTaskStartedMessage` in `sdk.d.ts`, and no issue asks for it. A lead: <https://github.com/anthropics/claude-code/issues/100198> says Claude Code writes a subagent's display name to `subagents/agent-<id>.meta.json` beside the transcript, which is undocumented, and may hold the description rather than the given name: check it before relying on it, or ask upstream on that issue

- Look into what SendMessage reaches and make its line unsurprising. Besides subagents, Claude Code 2.1.322's code sends it to the main agent, teammates and other Claude sessions on the machine or in the cloud, and claudio shows any target but a known subagent's id as written

- Design the full view (Ctrl+O) as a whole, which grew line by line and was never laid out: what it holds for each call and result, how a subagent's calls (`#01#03`) and report read in it, and whether its tags are worth keeping

