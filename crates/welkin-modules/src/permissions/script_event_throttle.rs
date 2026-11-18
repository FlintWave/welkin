// SPDX-License-Identifier: LGPL-3.0-or-later

//! Script event throttle helper.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct KeyState {
    count: u32,
    window_start: Instant,
    cooldown_until: Option<Instant>,
    in_flight: usize,
}

#[derive(Debug, Clone)]
pub struct ScriptEventThrottle {
    max_events: u32,
    window: Duration,
    cooldown: Duration,
    max_concurrency: usize,
    entries: HashMap<String, KeyState>,
}

impl Default for ScriptEventThrottle {
    fn default() -> Self { Self::new(64, Duration::from_secs(1), Duration::from_secs(1), 4) }
}

impl ScriptEventThrottle {
    pub fn new(max_events: u32, window: Duration, cooldown: Duration, max_concurrency: usize) -> Self {
        Self { max_events, window, cooldown, max_concurrency, entries: HashMap::new() }
    }
    pub fn with_rate(max_events: u32, window: Duration) -> Self { Self::new(max_events, window, Duration::ZERO, usize::MAX) }
    pub fn with_concurrency(max_concurrency: usize) -> Self { Self::new(u32::MAX, Duration::ZERO, Duration::ZERO, max_concurrency) }
    pub fn with_limits(max_events: u32, window: Duration, max_concurrency: usize) -> Self { Self::new(max_events, window, Duration::ZERO, max_concurrency) }

    pub fn max_events(&self) -> u32 { self.max_events }
    pub fn window(&self) -> Duration { self.window }
    pub fn cooldown(&self) -> Duration { self.cooldown }
    pub fn max_concurrency(&self) -> usize { self.max_concurrency }

    fn entry(&mut self, key: &str, now: Instant) -> &mut KeyState {
        self.entries.entry(key.to_string()).or_insert_with(|| KeyState { count: 0, window_start: now, cooldown_until: None, in_flight: 0 })
    }

    fn rate_allowed_at(&mut self, key: &str, now: Instant) -> bool {
        let max_events = self.max_events; let window = self.window; let cooldown = self.cooldown;
        let e = self.entry(key, now);
        if let Some(until) = e.cooldown_until {
            if now < until { return false; }
            e.cooldown_until = None; e.count = 0; e.window_start = now;
        }
        if now.duration_since(e.window_start) >= window { e.count = 0; e.window_start = now; }
        e.count = e.count.saturating_add(1);
        if e.count > max_events { e.cooldown_until = Some(now + cooldown); false } else { true }
    }

    pub fn check(&mut self, key: impl ToString) -> bool { self.check_at(key, Instant::now()) }
    pub fn check_at(&mut self, key: impl ToString, now: Instant) -> bool { let k = key.to_string(); self.rate_allowed_at(&k, now) }
    pub fn allow(&mut self, key: impl ToString) -> bool { self.check(key) }
    pub fn try_acquire(&mut self, key: impl ToString) -> bool { self.try_acquire_at(key, Instant::now()) }
    pub fn try_acquire_at(&mut self, key: impl ToString, now: Instant) -> bool {
        let k = key.to_string();
        if self.in_flight(&k) >= self.max_concurrency { return false; }
        if !self.rate_allowed_at(&k, now) { return false; }
        let e = self.entry(&k, now); e.in_flight = e.in_flight.saturating_add(1); true
    }
    pub fn release(&mut self, key: impl ToString) {
        let k = key.to_string();
        if let Some(e) = self.entries.get_mut(&k) { e.in_flight = e.in_flight.saturating_sub(1); }
    }
    pub fn in_flight(&self, key: impl ToString) -> usize { self.entries.get(&key.to_string()).map(|e| e.in_flight).unwrap_or(0) }
    pub fn available_concurrency(&self, key: impl ToString) -> usize { self.max_concurrency.saturating_sub(self.in_flight(key)) }
    pub fn is_abuse(&self, key: impl ToString) -> bool { self.is_abuse_at(key, Instant::now()) }
    pub fn is_abuse_at(&self, key: impl ToString, now: Instant) -> bool {
        self.entries.get(&key.to_string()).and_then(|e| e.cooldown_until).map(|until| now < until).unwrap_or(false)
    }
    pub fn reset(&mut self, key: impl ToString) { self.entries.remove(&key.to_string()); }
    pub fn clear(&mut self) { self.entries.clear(); }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn check_global(&mut self) -> bool { self.check("") }
    pub fn allow_global(&mut self) -> bool { self.check("") }
    pub fn try_acquire_global(&mut self) -> bool { self.try_acquire("") }
    pub fn release_global(&mut self) { self.release("") }
    pub fn available_slots(&self) -> usize { self.available_concurrency("") }
    pub fn global_in_flight(&self) -> usize { self.in_flight("") }
}
