use std::collections::HashSet;

/// Tracks which (metric, threshold) pairs have already been notified
/// to avoid spamming the user.
pub struct NotificationTracker {
    fired: HashSet<(String, u8)>,
}

impl NotificationTracker {
    pub fn new() -> Self {
        Self {
            fired: HashSet::new(),
        }
    }

    /// Returns the single highest threshold that should fire for this metric/utilization.
    /// Marks all exceeded thresholds as fired. Clears fired thresholds when utilization drops below them.
    /// When multiple thresholds are crossed at once, only the highest one is returned
    /// to avoid spamming the user with multiple notifications.
    pub fn check(&mut self, metric: &str, utilization: f64, thresholds: &[u8]) -> Vec<u8> {
        let mut highest_new: Option<u8> = None;

        for &threshold in thresholds {
            let key = (metric.to_string(), threshold);
            if utilization >= threshold as f64 {
                if !self.fired.contains(&key) {
                    self.fired.insert(key);
                    // Track the highest newly-exceeded threshold
                    if highest_new.map_or(true, |h| threshold > h) {
                        highest_new = Some(threshold);
                    }
                }
            } else {
                // Reset so it can fire again when usage rises again
                self.fired.remove(&key);
            }
        }

        highest_new.into_iter().collect()
    }
}

/// Minutes before expiry at which the "token about to expire" warning may fire.
pub const TOKEN_EXPIRY_MARKS_MIN: [u64; 4] = [55, 30, 15, 5];

/// Spaces out the token-expiry warning: one balloon per mark in
/// [`TOKEN_EXPIRY_MARKS_MIN`], never one per poll.
#[derive(Default)]
pub struct TokenExpiryTracker {
    /// Smallest mark already announced for the current token, if any.
    announced: Option<u64>,
}

impl TokenExpiryTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed the time left on the token (milliseconds). Returns the whole
    /// minutes to show when a new mark has just been crossed.
    ///
    /// Crossing several marks at once (the machine slept, a long poll gap)
    /// yields a single warning. A token with more than the first mark left was
    /// refreshed, so the sequence starts over for the next one.
    pub fn check(&mut self, remaining_ms: u64) -> Option<u64> {
        let minutes = remaining_ms / 60_000;
        let first_mark = TOKEN_EXPIRY_MARKS_MIN[0];
        if remaining_ms == 0 || minutes > first_mark {
            self.announced = None;
            return None;
        }
        let crossed = TOKEN_EXPIRY_MARKS_MIN
            .iter()
            .copied()
            .filter(|&mark| minutes <= mark)
            .min()?;
        if self.announced.is_some_and(|done| crossed >= done) {
            return None;
        }
        self.announced = Some(crossed);
        Some(minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    #[test]
    fn token_warning_fires_once_per_mark() {
        let mut t = TokenExpiryTracker::new();
        assert_eq!(t.check(54 * MIN), Some(54));
        // Every minute of polling after that stays quiet until the next mark.
        for m in (31..54).rev() {
            assert_eq!(t.check(m * MIN), None, "minute {m}");
        }
        assert_eq!(t.check(30 * MIN), Some(30));
        assert_eq!(t.check(29 * MIN), None);
        assert_eq!(t.check(15 * MIN), Some(15));
        assert_eq!(t.check(5 * MIN), Some(5));
        assert_eq!(t.check(2 * MIN), None);
    }

    #[test]
    fn token_warning_collapses_skipped_marks() {
        let mut t = TokenExpiryTracker::new();
        // Woke up with 12 minutes left: one warning, not three.
        assert_eq!(t.check(12 * MIN), Some(12));
        assert_eq!(t.check(11 * MIN), None);
    }

    #[test]
    fn token_warning_restarts_after_refresh() {
        let mut t = TokenExpiryTracker::new();
        assert_eq!(t.check(20 * MIN), Some(20));
        // Refreshed token: 8 hours left.
        assert_eq!(t.check(480 * MIN), None);
        assert_eq!(t.check(50 * MIN), Some(50));
    }

    #[test]
    fn token_warning_ignores_expired_and_far_tokens() {
        let mut t = TokenExpiryTracker::new();
        assert_eq!(t.check(0), None);
        assert_eq!(t.check(56 * MIN), None);
        assert_eq!(t.check(90 * MIN), None);
    }

    #[test]
    fn test_notification_dedup() {
        let mut tracker = NotificationTracker::new();
        let thresholds = [50u8, 75, 90];

        // First time at 60% → fires only threshold 50
        let fired = tracker.check("five_hour", 60.0, &thresholds);
        assert_eq!(fired, vec![50]);

        // Second time at 60% → no new fires
        let fired = tracker.check("five_hour", 60.0, &thresholds);
        assert!(fired.is_empty());

        // Drops below 50% → clears the 50% threshold
        let fired = tracker.check("five_hour", 40.0, &thresholds);
        assert!(fired.is_empty());

        // Back to 60% → fires 50% again
        let fired = tracker.check("five_hour", 60.0, &thresholds);
        assert_eq!(fired, vec![50]);
    }

    #[test]
    fn test_only_highest_threshold_fires() {
        let mut tracker = NotificationTracker::new();
        let thresholds = [50u8, 75, 90];

        // Jump straight to 96% → only fires 90 (the highest), not all three
        let fired = tracker.check("five_hour", 96.0, &thresholds);
        assert_eq!(fired, vec![90]);

        // All thresholds are now marked as fired, so no new fires
        let fired = tracker.check("five_hour", 96.0, &thresholds);
        assert!(fired.is_empty());

        // Drop to 80% → clears 90, but 50 and 75 still fired
        let fired = tracker.check("five_hour", 80.0, &thresholds);
        assert!(fired.is_empty());

        // Back to 96% → only 90 fires again (50 and 75 still marked)
        let fired = tracker.check("five_hour", 96.0, &thresholds);
        assert_eq!(fired, vec![90]);
    }

    #[test]
    fn test_gradual_increase_fires_each_threshold_once() {
        let mut tracker = NotificationTracker::new();
        let thresholds = [50u8, 75, 90];

        // Gradually increasing usage fires each threshold individually
        let fired = tracker.check("five_hour", 55.0, &thresholds);
        assert_eq!(fired, vec![50]);

        let fired = tracker.check("five_hour", 78.0, &thresholds);
        assert_eq!(fired, vec![75]);

        let fired = tracker.check("five_hour", 95.0, &thresholds);
        assert_eq!(fired, vec![90]);
    }
}
