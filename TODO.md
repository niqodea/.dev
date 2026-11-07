# TODO

- (maybe) build a tablet note-taking app that takes the pen in one hand and the keyboard in the other, combining both, since no app found does this

- Explore customizing Claude Code's UI

- ,claude-plain: check live that the context level turns red when the cache expires after an hour idle

- Replace logic that guesses a time with waiting for the event it depends on:
  - ,approval: polls every 10 ms whether an action ended, and stops it after 60 s
  - ,claude-plain: counts the cache as cold 60 s before its TTL ends, a guess of how long the API took before the reply started
  - test-macos-install: retries SSH every 5 s for up to 5 minutes, and waits up to 300 s for the virtual machine's address
