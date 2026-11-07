//! The conversation's context as the API holds it: how many tokens it has, and whether the API
//! still keeps them in its prompt cache. Each call of Claude's reads the whole context, from the
//! cache when it is warm. A call on a cold cache writes the whole context to the cache again, at
//! a price above that of new input, so a message on a cold cache waits for the user to send it
//! a second time.

use std::time::{Duration, Instant};

use crate::protocol::Usage;

/// The upper bounds of the levels T0 to T2, above which the context is at T3. A new session
/// starts at about 25k tokens, its system prompt, tools and memory, so T0 is a context that
/// holds little more than that.
const LEVEL_BOUNDS: [u64; 3] = [30_000, 80_000, 150_000];

/// The shortest TTL the API gives a cache entry, which a call that wrote nothing to the cache
/// keeps when no call before it told its TTL.
const SHORTEST_TTL: Duration = Duration::from_mins(5);

pub struct Context {
    tokens: u64,
    cache: Cache,
    /// The TTL of the cache entry, which the last call that wrote one told.
    ttl: Duration,
    /// Whether the user sent the message on the input line once on the cold cache, so it goes
    /// the next time.
    is_cold_confirmed: bool,
}

enum Cache {
    /// The cache before Claude Code tells whether the session had a call.
    Starting,
    /// The cache of a new session, which holds nothing yet to lose.
    Empty,
    /// The cache is warm until this instant.
    WarmUntil(Instant),
    /// The cache of a resumed session before its first call here, whose age is unknown.
    Unknown,
}

impl Context {
    pub fn new() -> Self {
        Context {
            tokens: 0,
            cache: Cache::Starting,
            ttl: SHORTEST_TTL,
            is_cold_confirmed: false,
        }
    }

    /// Notes a call of Claude's whose reply starts at `now`, and returns when its cache turns
    /// cold. The API keeps a cache entry for its TTL after the call that last read or wrote it,
    /// and that call reached the API a little before its reply started, so the cache can turn
    /// cold that much earlier. Claude Code 2.1.316 counts its own cache's TTL the same way, from
    /// its own clock and with no margin.
    pub fn note_call(&mut self, usage: &Usage, now: Instant) -> Instant {
        self.tokens = usage.tokens;
        if let Some(ttl) = usage.cache_ttl {
            self.ttl = ttl;
        }
        let cold_at = now + self.ttl;
        self.cache = Cache::WarmUntil(cold_at);
        self.is_cold_confirmed = false;
        cold_at
    }

    /// Notes the last call of the session as Claude Code starts it, at an unknown time, none in
    /// a new session.
    pub fn note_last_call(&mut self, usage: Option<&Usage>) {
        // A call of this process tells more
        if let Cache::Starting = self.cache {
            self.cache = match usage {
                Some(usage) => {
                    self.tokens = usage.tokens;
                    Cache::Unknown
                }
                None => Cache::Empty,
            };
        }
    }

    /// The level of the context's size, from 0 to 3, none before Claude Code tells it.
    pub fn level(&self) -> Option<usize> {
        if let Cache::Starting = self.cache {
            return None;
        }
        let level = LEVEL_BOUNDS
            .iter()
            .take_while(|&&bound| self.tokens >= bound)
            .count();
        Some(level)
    }

    pub fn is_cold(&self, now: Instant) -> bool {
        match self.cache {
            Cache::Starting | Cache::Empty => false,
            Cache::WarmUntil(cold_at) => now >= cold_at,
            Cache::Unknown => true,
        }
    }

    /// Whether the next message goes on the cold cache, as the user confirmed.
    pub fn is_cold_confirmed(&self, now: Instant) -> bool {
        self.is_cold(now) && self.is_cold_confirmed
    }

    /// Whether a message the user sends at `now` waits, which it does each time before Claude
    /// Code tells whether the session had a call, and the first time on a cold cache. Each
    /// message on a cold cache waits once, since a call that failed leaves the cache cold.
    pub fn holds_message(&mut self, now: Instant) -> bool {
        if let Cache::Starting = self.cache {
            return true;
        }
        let holds = self.is_cold(now) && !self.is_cold_confirmed;
        self.is_cold_confirmed = holds;
        holds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: Duration = Duration::from_hours(1);

    fn usage(tokens: u64, cache_ttl: Option<Duration>) -> Usage {
        Usage { tokens, cache_ttl }
    }

    #[test]
    fn a_new_session_is_at_t0_and_sends_at_once() {
        let mut context = Context::new();
        context.note_last_call(None);
        let now = Instant::now();
        assert_eq!(context.level(), Some(0));
        assert!(!context.holds_message(now + HOUR));
    }

    #[test]
    fn each_bound_starts_the_next_level() {
        let mut context = Context::new();
        let now = Instant::now();
        for (tokens, level) in [(29_999, 0), (30_000, 1), (80_000, 2), (150_000, 3)] {
            context.note_call(&usage(tokens, None), now);
            assert_eq!(context.level(), Some(level));
        }
    }

    #[test]
    fn the_cache_turns_cold_once_its_ttl_passes() {
        let mut context = Context::new();
        let now = Instant::now();
        let cold_at = context.note_call(&usage(40_000, Some(HOUR)), now);
        assert_eq!(cold_at, now + HOUR);
        assert!(!context.is_cold(now));
        assert!(context.is_cold(cold_at));
    }

    #[test]
    fn a_call_that_writes_nothing_keeps_the_ttl_before_it() {
        let mut context = Context::new();
        let now = Instant::now();
        context.note_call(&usage(40_000, Some(HOUR)), now);
        let cold_at = context.note_call(&usage(40_000, None), now);
        assert_eq!(cold_at, now + HOUR);
    }

    #[test]
    fn a_message_on_a_cold_cache_goes_the_second_time() {
        let mut context = Context::new();
        let now = Instant::now();
        let cold_at = context.note_call(&usage(40_000, Some(HOUR)), now);
        assert!(context.holds_message(cold_at));
        assert!(context.is_cold_confirmed(cold_at));
        assert!(!context.holds_message(cold_at));
        assert!(!context.is_cold_confirmed(cold_at));
    }

    #[test]
    fn the_next_message_on_a_cold_cache_waits_again() {
        let mut context = Context::new();
        let now = Instant::now();
        let cold_at = context.note_call(&usage(40_000, Some(HOUR)), now);
        context.holds_message(cold_at);
        context.holds_message(cold_at);
        assert!(context.holds_message(cold_at));
    }

    #[test]
    fn a_call_ends_the_confirmation_of_the_cold_cache() {
        let mut context = Context::new();
        let now = Instant::now();
        let cold_at = context.note_call(&usage(40_000, Some(HOUR)), now);
        context.holds_message(cold_at);
        let cold_again_at = context.note_call(&usage(40_000, Some(HOUR)), cold_at);
        assert!(context.holds_message(cold_again_at));
    }

    #[test]
    fn a_message_waits_until_claude_code_tells_whether_the_session_is_new() {
        let mut context = Context::new();
        let now = Instant::now();
        assert_eq!(context.level(), None);
        assert!(!context.is_cold(now));
        assert!(context.holds_message(now));
        assert!(context.holds_message(now));
    }

    #[test]
    fn a_resumed_session_starts_cold_at_its_size() {
        let mut context = Context::new();
        context.note_last_call(Some(&usage(90_000, None)));
        let now = Instant::now();
        assert_eq!(context.level(), Some(2));
        assert!(context.is_cold(now));
        assert!(context.holds_message(now));
    }

    #[test]
    fn a_call_of_this_process_outweighs_the_resumed_one() {
        let mut context = Context::new();
        let now = Instant::now();
        context.note_call(&usage(40_000, Some(HOUR)), now);
        context.note_last_call(Some(&usage(90_000, None)));
        assert_eq!(context.level(), Some(1));
        assert!(!context.is_cold(now));
    }
}
