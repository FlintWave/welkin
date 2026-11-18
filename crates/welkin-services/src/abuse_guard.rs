// SPDX-License-Identifier: LGPL-3.0-or-later

//! Basic DOS/abuse throttle helper: a per-key counter with cooldowns.
//!
//! Tracks event counts within a sliding-ish fixed window. Once `max_events`
//! events are recorded for a key within `window`, further events are denied
//! until `cooldown` has elapsed.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Per-key counter state.
#[derive(Debug, Clone)]
struct Entry {
    count: u32,
    window_start: Instant,
    cooldown_until: Option<Instant>,
}

/// DOS/abuse throttle guard.
///
/// Keys are coerced to strings (via `ToString`) so callers may use IP
/// addresses, UUIDs, or any displayable identifier.
#[derive(Debug, Clone)]
pub struct AbuseGuard {
    max_events: u32,
    window: Duration,
    cooldown: Duration,
    entries: HashMap<String, Entry>,
}

impl AbuseGuard {
    /// Create a new guard.
    ///
    /// * `max_events` - events allowed per window before abuse.
    /// * `window` - counting window length.
    /// * `cooldown` - denial period after abuse is detected.
    pub fn new(max_events: u32, window: Duration, cooldown: Duration) -> Self {
        Self {
            max_events,
            window,
            cooldown,
            entries: HashMap::new(),
        }
    }

    pub fn max_events(&self) -> u32 {
        self.max_events
    }
    pub fn window(&self) -> Duration {
        self.window
    }
    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }

    /// Record an event using the current wall clock. Returns `true` if allowed.
    pub fn check(&mut self, key: impl ToString) -> bool {
        self.check_at(key, Instant::now())
    }

    /// Record an event at `now`. Returns `true` if allowed, `false` if abusive.
    pub fn check_at(&mut self, key: impl ToString, now: Instant) -> bool {
        let k = key.to_string();
        let entry = self.entries.entry(k).or_insert_with(|| Entry {
            count: 0,
            window_start: now,
            cooldown_until: None,
        });

        // Active cooldown?
        if let Some(until) = entry.cooldown_until {
            if now < until {
                return false;
            }
            // cooldown expired: reset
            entry.cooldown_until = None;
            entry.count = 0;
            entry.window_start = now;
        }

        // Window rollover?
        if now.duration_since(entry.window_start) >= self.window {
            entry.count = 0;
            entry.window_start = now;
        }

        entry.count = entry.count.saturating_add(1);
        if entry.count > self.max_events {
            entry.cooldown_until = Some(now + self.cooldown);
            false
        } else {
            true
        }
    }

    // Aliases for the same operation (different vocabularies used by callers).
    pub fn allow(&mut self, key: impl ToString) -> bool {
        self.check(key)
    }
    pub fn try_check(&mut self, key: impl ToString) -> bool {
        self.check(key)
    }
    pub fn try_allow(&mut self, key: impl ToString) -> bool {
        self.check(key)
    }
    pub fn try_acquire(&mut self, key: impl ToString) -> bool {
        self.check(key)
    }
    pub fn admit(&mut self, key: impl ToString) -> bool {
        self.check(key)
    }
    pub fn record(&mut self, key: impl ToString) -> bool {
        self.check(key)
    }
    pub fn try_record(&mut self, key: impl ToString) -> bool {
        self.check(key)
    }
    pub fn note(&mut self, key: impl ToString) -> bool {
        self.check(key)
    }

    pub fn allow_at(&mut self, key: impl ToString, now: Instant) -> bool {
        self.check_at(key, now)
    }
    pub fn try_check_at(&mut self, key: impl ToString, now: Instant) -> bool {
        self.check_at(key, now)
    }
    pub fn try_allow_at(&mut self, key: impl ToString, now: Instant) -> bool {
        self.check_at(key, now)
    }
    pub fn try_acquire_at(&mut self, key: impl ToString, now: Instant) -> bool {
        self.check_at(key, now)
    }
    pub fn admit_at(&mut self, key: impl ToString, now: Instant) -> bool {
        self.check_at(key, now)
    }
    pub fn record_at(&mut self, key: impl ToString, now: Instant) -> bool {
        self.check_at(key, now)
    }
    pub fn try_record_at(&mut self, key: impl ToString, now: Instant) -> bool {
        self.check_at(key, now)
    }
    pub fn note_at(&mut self, key: impl ToString, now: Instant) -> bool {
        self.check_at(key, now)
    }

    /// Is the key currently in cooldown (using now)?
    pub fn is_abuse(&self, key: impl ToString) -> bool {
        self.is_abuse_at(key, Instant::now())
    }
    /// Is the key currently in cooldown at `now`?
    pub fn is_abuse_at(&self, key: impl ToString, now: Instant) -> bool {
        self.entries
            .get(&key.to_string())
            .and_then(|e| e.cooldown_until)
            .map(|until| now < until)
            .unwrap_or(false)
    }

    // Alias names
    pub fn in_cooldown(&self, key: impl ToString) -> bool {
        self.is_abuse(key)
    }
    pub fn in_cooldown_at(&self, key: impl ToString, now: Instant) -> bool {
        self.is_abuse_at(key, now)
    }
    pub fn blocked(&self, key: impl ToString) -> bool {
        self.is_abuse(key)
    }
    pub fn blocked_at(&self, key: impl ToString, now: Instant) -> bool {
        self.is_abuse_at(key, now)
    }
    pub fn cooldown_until(&self, key: impl ToString) -> Option<Instant> {
        self.entries.get(&key.to_string()).and_then(|e| e.cooldown_until)
    }

    /// Clear all state for one key.
    pub fn reset(&mut self, key: impl ToString) {
        self.entries.remove(&key.to_string());
    }
    /// Remove all tracked keys.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn allows_up_to_max() {
        let mut g = AbuseGuard::new(3, Duration::from_secs(10), Duration::from_secs(5));
        let t = Instant::now();
        assert!(g.check_at("k", t));
        assert!(g.check_at("k", t));
        assert!(g.check_at("k", t));
        assert!(!g.check_at("k", t)); // 4th -> abuse
        assert!(g.is_abuse_at("k", t));
    }

    #[test]
    fn cooldown_expires() {
        let mut g = AbuseGuard::new(1, Duration::from_secs(10), Duration::from_secs(5));
        let t = Instant::now();
        assert!(g.check_at("k", t));
        assert!(!g.check_at("k", t));
        let later = t + Duration::from_secs(6);
        assert!(g.check_at("k", later));
    }

    #[test]
    fn window_resets() {
        let mut g = AbuseGuard::new(1, Duration::from_secs(10), Duration::from_secs(5));
        let t = Instant::now();
        assert!(g.check_at("k", t));
        let later = t + Duration::from_secs(11);
        assert!(g.check_at("k", later));
    }

    #[test]
    fn uuid_keys() {
        let mut g = AbuseGuard::new(2, Duration::from_secs(10), Duration::from_secs(5));
        let id = uuid::Uuid::new_v4();
        let t = Instant::now();
        assert!(g.check_at(id, t));
        assert!(g.check_at(id, t));
        assert!(!g.check_at(id, t));
    }
}
