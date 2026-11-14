// SPDX-License-Identifier: GPL-3.0-or-later

use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ThrottleCategory {
    Resend = 0,
    Land = 1,
    Wind = 2,
    Cloud = 3,
    Task = 4,
    Texture = 5,
    Asset = 6,
}

pub const CATEGORY_COUNT: usize = 7;

impl ThrottleCategory {
    pub const ALL: [ThrottleCategory; 7] = [
        ThrottleCategory::Resend,
        ThrottleCategory::Land,
        ThrottleCategory::Wind,
        ThrottleCategory::Cloud,
        ThrottleCategory::Task,
        ThrottleCategory::Texture,
        ThrottleCategory::Asset,
    ];
}

pub struct TokenBucket {
    rate: f64,
    capacity: f64,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(rate_bytes_per_sec: f64) -> Self {
        Self {
            rate: rate_bytes_per_sec,
            capacity: rate_bytes_per_sec,
            tokens: rate_bytes_per_sec,
            last_refill: Instant::now(),
        }
    }

    pub fn set_rate(&mut self, rate_bytes_per_sec: f64) {
        self.rate = rate_bytes_per_sec;
        self.capacity = rate_bytes_per_sec;
        if self.tokens > self.capacity {
            self.tokens = self.capacity;
        }
    }

    pub fn try_consume(&mut self, bytes: usize) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;
        self.tokens = (self.tokens + elapsed * self.rate).min(self.capacity);

        if self.tokens >= bytes as f64 {
            self.tokens -= bytes as f64;
            true
        } else {
            false
        }
    }

    pub fn available(&mut self) -> f64 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;
        self.tokens = (self.tokens + elapsed * self.rate).min(self.capacity);
        self.tokens
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }
}

pub struct ClientThrottle {
    buckets: [TokenBucket; CATEGORY_COUNT],
    total: TokenBucket,
}

impl ClientThrottle {
    pub fn new() -> Self {
        let total_rate = 625_000.0;
        let category_rate = total_rate / CATEGORY_COUNT as f64;
        Self {
            buckets: std::array::from_fn(|_| TokenBucket::new(category_rate)),
            total: TokenBucket::new(total_rate),
        }
    }

    pub fn set_from_viewer(&mut self, data: &[u8]) {
        if data.len() < 28 {
            return;
        }

        let mut total = 0.0f64;
        for i in 0..7 {
            let bits_per_sec =
                f32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().unwrap()) as f64;
            let bytes_per_sec = bits_per_sec / 8.0;
            let clamped = bytes_per_sec.clamp(1000.0, 250_000.0);
            self.buckets[i].set_rate(clamped);
            total += clamped;
        }

        self.total.set_rate(total.min(1_875_000.0));
    }

    pub fn try_send(&mut self, category: ThrottleCategory, bytes: usize) -> bool {
        if !self.buckets[category as usize].try_consume(bytes) {
            return false;
        }
        if !self.total.try_consume(bytes) {
            return false;
        }
        true
    }

    pub fn category_rate(&self, category: ThrottleCategory) -> f64 {
        self.buckets[category as usize].rate()
    }
}

impl Default for ClientThrottle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_token_bucket_basic() {
        let mut bucket = TokenBucket::new(1000.0);
        assert!(bucket.try_consume(500));
        assert!(!bucket.try_consume(600));
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(10000.0);
        assert!(bucket.try_consume(10000));
        thread::sleep(Duration::from_millis(100));
        assert!(bucket.try_consume(500));
    }

    #[test]
    fn test_set_from_viewer() {
        let mut throttle = ClientThrottle::new();
        let mut data = Vec::with_capacity(28);
        for _ in 0..7 {
            data.extend_from_slice(&100000.0f32.to_le_bytes());
        }

        throttle.set_from_viewer(&data);

        for category in ThrottleCategory::ALL {
            let rate = throttle.category_rate(category);
            assert!((rate - 12500.0).abs() < 1.0);
        }
    }

    #[test]
    fn test_try_send() {
        let mut throttle = ClientThrottle::new();
        assert!(throttle.try_send(ThrottleCategory::Task, 100));
    }
}
