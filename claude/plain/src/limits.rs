//! The plan's usage limits as the API counts them: how much of each the account used, over five
//! hours and over seven days. Claude Code sends them along with Claude's replies.

use crate::protocol::Limit;

/// The shares of a limit from which its level is 1 to 3. They are this client's own choice:
/// Claude Code's server decides where Claude Code warns, and no source lists those points. The
/// steps get smaller near the limit, where less usage remains.
const LEVEL_BOUNDS: [f64; 3] = [0.5, 0.75, 0.9];

/// The level from which the indicator turns orange.
pub const NEAR_LEVEL: usize = 2;

/// The level of a limit the account reached.
pub const REACHED_LEVEL: usize = LEVEL_BOUNDS.len() + 1;

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
    pub fn session_level(&self) -> Option<usize> {
        self.session.map(level)
    }

    /// The level of the limit over seven days, none before Claude Code sends it.
    pub fn week_level(&self) -> Option<usize> {
        self.week.map(level)
    }
}

/// From 0, below the first warning, to `REACHED_LEVEL`, at the limit.
fn level(limit: Limit) -> usize {
    if limit.is_reached {
        return REACHED_LEVEL;
    }
    LEVEL_BOUNDS
        .iter()
        .take_while(|&&bound| limit.utilization >= bound)
        .count()
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
    fn each_warning_passed_raises_the_level() {
        assert_eq!(level(used(0.0)), 0);
        assert_eq!(level(used(0.49)), 0);
        assert_eq!(level(used(0.5)), 1);
        assert_eq!(level(used(0.75)), 2);
        assert_eq!(level(used(0.9)), 3);
        assert_eq!(level(used(0.99)), 3);
    }

    #[test]
    fn a_reached_limit_has_a_level_of_its_own() {
        let reached = Limit {
            utilization: 1.0,
            is_reached: true,
        };
        assert_eq!(level(reached), REACHED_LEVEL);
    }

    #[test]
    fn a_limit_left_untold_keeps_its_level() {
        let mut limits = Limits::new();
        limits.note(Some(used(0.6)), Some(used(0.2)));
        limits.note(None, Some(used(0.8)));
        assert_eq!(limits.session_level(), Some(1));
        assert_eq!(limits.week_level(), Some(2));
    }
}
