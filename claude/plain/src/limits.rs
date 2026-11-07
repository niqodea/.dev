//! The plan's usage limits as the API counts them: how much of each the account used, over five
//! hours and over seven days. Claude Code sends them along with Claude's replies.

use crate::protocol::Limit;

/// The shares of a limit that raise its level by one each. They are this client's own choice:
/// Claude Code's server decides where Claude Code warns, and no source lists those points. The
/// steps get smaller near the limit, where less usage remains.
const LEVEL_BOUNDS: [f64; 3] = [0.5, 0.75, 0.9];

/// The number of bounds passed from which the indicator turns orange.
pub const NEAR_BOUNDS_PASSED: usize = 2;

/// How much of a limit the account used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    /// Short of the limit, with how many of `LEVEL_BOUNDS` the account passed.
    Below(usize),
    Reached,
}

impl Level {
    /// The number the indicator shows: the bounds passed, then one more once reached.
    pub fn to_number(self) -> usize {
        match self {
            Level::Below(passed) => passed,
            Level::Reached => LEVEL_BOUNDS.len() + 1,
        }
    }
}

pub struct Limits {
    session: Option<Limit>,
    week: Option<Limit>,
}

impl Limits {
    pub fn new() -> Self {
        Limits {
            session: None,
            week: None,
        }
    }

    /// Stores the limits Claude Code sent. A limit it left out keeps the value it sent last.
    pub fn note(&mut self, session: Option<Limit>, week: Option<Limit>) {
        self.session = session.or(self.session);
        self.week = week.or(self.week);
    }

    /// The level of the limit over five hours, none before Claude Code sends it.
    pub fn session_level(&self) -> Option<Level> {
        self.session.map(level)
    }

    /// The level of the limit over seven days, none before Claude Code sends it.
    pub fn week_level(&self) -> Option<Level> {
        self.week.map(level)
    }
}

fn level(limit: Limit) -> Level {
    if limit.is_reached {
        return Level::Reached;
    }
    let passed = LEVEL_BOUNDS
        .iter()
        .take_while(|&&bound| limit.utilization >= bound)
        .count();
    Level::Below(passed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn used(utilization: f64) -> Limit {
        Limit {
            utilization,
            is_reached: false,
        }
    }

    #[test]
    fn each_bound_passed_raises_the_level() {
        assert_eq!(level(used(0.0)), Level::Below(0));
        assert_eq!(level(used(0.49)), Level::Below(0));
        assert_eq!(level(used(0.5)), Level::Below(1));
        assert_eq!(level(used(0.75)), Level::Below(2));
        assert_eq!(level(used(0.9)), Level::Below(3));
        assert_eq!(level(used(0.99)), Level::Below(3));
    }

    #[test]
    fn a_reached_limit_shows_one_past_the_last_bound() {
        let reached = Limit {
            utilization: 1.0,
            is_reached: true,
        };
        assert_eq!(level(reached), Level::Reached);
        assert_eq!(Level::Reached.to_number(), 4);
    }

    #[test]
    fn a_limit_left_untold_keeps_its_level() {
        let mut limits = Limits::new();
        limits.note(Some(used(0.6)), Some(used(0.2)));
        limits.note(None, Some(used(0.8)));
        assert_eq!(limits.session_level(), Some(Level::Below(1)));
        assert_eq!(limits.week_level(), Some(Level::Below(2)));
    }
}
