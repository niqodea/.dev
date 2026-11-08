# TODO

- (maybe) build a tablet note-taking app that takes the pen in one hand and the keyboard in the other, combining both, since no app found does this

- Explore customizing Claude Code's UI

- ,claude-plain: check live that the context level turns red, with a * in front, when the cache expires after an hour idle

- ,claude-plain: words are cut where the pane's margin wraps them; look up how others solve it

- ,claude-plain: keys for useful Claude Code commands like /compact; check the docs for which commands work under `claude -p` and which earn a key

- ,claude-plain: a detailed status to look at on demand, with exact context and limit percentages, reset times and token counts

- ,claude-plain: a key to see the whole conversation uncollapsed, without reading Claude Code's transcript file and without holding the session in memory

- ,claude-plain: an option for the input box's height, now fixed at 3 rows

- ,claude-plain: show Claude's prose word by word as it streams, now that the client draws its input box itself; tables and code blocks still need all their lines

- zsh: show whether vi mode is in insert or normal mode

- Replace logic that guesses a time with waiting for the event it depends on:
  - ,approval: polls every 10 ms whether an action ended, and stops it after 60 s
  - test-macos-install: retries SSH every 5 s for up to 5 minutes, and waits up to 300 s for the virtual machine's address
