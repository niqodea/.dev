# TODO

- (maybe) build a tablet note-taking app that takes the pen in one hand and the keyboard in the other, combining both, since no app found does this

- Explore customizing Claude Code's UI

- Make the CLIs in `claude/clis` work on Debian too: slack, discord and outlook keep their tokens in the macOS Keychain, which pass, from the `utils` module, could replace on both systems. outlook also saves the refresh token it gets back, so it needs pass to write as well as read

- Decide whether `claude/install-bin` should keep installing the CLIs in `claude/clis`, or whether another way fits better, like a step each CLI describes for itself

- Make a warning a field ,claude-approval itself highlights, once two or three CLIs need one, like the replies the slack CLI's `describe` shows when a thread got new ones after the request

- Give `,claude-approval` a `status` too, like Claude's side has, if we want to see what was decided outside the answering screen

- Consider making what needs `DOTDEV_PROJECT` work outside a project too, like in the tmux `~` session:
  - approval: `read_queue_directory` in `claude/approval/src/lib.rs` puts a queue in the project's atdir and fails without a project, and `run_change_hook` skips the status line hook
  - ports: `,get-project-port-range` picks a range from the project's name and fails without a project, and so do `,get-project-port` and `,list-project-listeners`, which call it

- Move the commands no person types from `~/.local/bin` to `~/.local/libexec`, like the `,tmux-*` ones that only tmux key bindings run, as `claude-approval-request` already is

- Give each Claude loop its own interval in a file next to its body, like `claude/loop/bodies/inbox.interval`, once a second loop needs another pace than 30 minutes. `CLAUDE_LOOP_COMMAND` then names only the program, like `claudio`, and `,claude-loop` adds `"/loop <interval>"` itself, since today the interval sits inside the command and so applies to every loop

- tmux: when a narrower pane rewraps lines into more rows, tmux pushes the top of the screen into the scrollback even when empty rows sit below the cursor, while a shorter pane uses those empty rows first (`screen_resize_y`). This leaves copies of claudio's label row in the scrollback (claudio's README, Resizing), and does the same to wide shell prompts. Open an issue asking the maintainers whether this is intended before writing a fix: a fix, which drops the empty lines below the cursor after the reflow in `screen_resize_cursor` in `screen.c`, changes the expected results of 4 of tmux's tests that pin where a reflow leaves the cursor and content. Keep it out of the word-wrapping fork unless they agree

- tmux: send upstream the fix on branch `fix-wrapped-edge-space` in `~/oss/tmux`, with its regression test: in plain tmux, vi copy mode sticks on the word before a space in a wrapped line's last column, since `grid_line_length` strips that space. The word-wrapping fork, branch `niqodea`, builds on it

- Replace logic that guesses a time with waiting for the event it depends on:
  - approval: polls every 10 ms whether an action ended, and stops it after 60 s

## claudio

### Before going public

- Rewrite the README so its first screen says what claudio is and what it is not: unofficial, checked only in tmux 3.8 for resizing, pinned to one version of Claude Code, and built on behavior that is observed rather than documented

- Say in the README that each version of Claude Code needs claudio's checks and a release of its own, since claudio runs only the version it was checked against, so users wait for that release before a newer Claude Code

- Keep the README's note on bare mode in view: Claude Code's docs say `--bare` will become the default for `-p`, and bare mode ignores the subscription login, so the release that does it needs claudio to turn it back off

- Keep the README's note on server-side flags in view: the hermetic checks run on Claude Code's default flags, so a user's session, which gets the flags Anthropic serves, can differ with no new version of Claude Code

- Decide how users install claudio: today `cargo install --path .` with `publish = false`, which needs a clone of the repo and Rust

- Go through all of the code, comments above all, and remove what does not belong in a public tool

### Later

- Hand the input line the keys typed while claudio waits for the terminal's answers, like whether it wraps at spaces: they arrive among the answers, and `wraps_at_spaces` in `src/terminal.rs` drops them today

- Ask Anthropic to add to the answer to `get_usage` when Claude Code fetched its numbers, which its `/usage` already shows, so claudio can stop reading the copy of the usage in Claude Code's config file

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

- Decide how to show tabs. claudio shows a tab as one space, so the cut at 88 columns counts right, but indentation made of tabs, as in Go code or a Makefile, shows flattened. A terminal moves a tab to its next tab stop, every 8 columns by default, so the tab's width depends on its column. less (`-x`), bat (`--tabs`) and vim (`tabstop`) replace a tab with spaces up to the next tab stop. Emoji sequences, like flags, also take different widths in different terminals

- Consider keeping inline code in prose whole when the terminal wraps at spaces, as if it were one word, since a break at a space inside it splits one piece of code into what reads as two. Inline code too long for a row would wrap at the edge, as code blocks do, for the reason the README's Wrapping at spaces gives. The terminal needs a way to be told which spaces it must not break at: a no-break space, U+00A0, would do it, but copies as a different character, so it likely takes a mark of its own, next to `OSC 2029`
