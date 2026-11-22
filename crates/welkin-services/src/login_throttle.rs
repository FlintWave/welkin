// SPDX-License-Identifier: LGPL-3.0-or-later

//! Per-IP, per-account+IP, and per-account login throttle with expiring
//! windows, in-flight reservation, and temporary lockouts.
//! `begin` atomically reserves an attempt, so concurrent requests cannot
//! overshoot a limit. Tables are bounded; locked entries are evicted when
//! necessary rather than dropping the new record.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;

#[derive(Clone, Debug)]
pub struct LoginThrottleConfig {
    pub max_failures_per_ip: u32,
    pub max_failures_per_account_ip: u32,
    pub max_failures_per_account: u32,
    pub window: Duration,
    pub lockout: Duration,
    pub max_tracked_keys: usize,
    pub trusted_proxies: Vec<IpAddr>,
}

impl Default for LoginThrottleConfig {
    fn default() -> Self {
        Self {
            max_failures_per_ip: 20,
            max_failures_per_account_ip: 5,
            max_failures_per_account: 50,
            window: Duration::from_secs(300),
            lockout: Duration::from_secs(300),
            max_tracked_keys: 10_000,
            trusted_proxies: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct Entry {
    window_start: Instant,
    failures: u32,
    in_flight: u32,
    locked_until: Option<Instant>,
}

pub struct LoginAttempt<'a> {
    throttle: &'a LoginThrottle,
    ip: Option<IpAddr>,
    account: String,
    now: Instant,
    resolved: bool,
}

impl<'a> LoginAttempt<'a> {
    pub fn unknown_account(mut self) {
        self.throttle.release_in_flight(self.ip, &self.account, self.now);
        if let Some(ip) = self.ip {
            count_failure_in_map(
                &self.throttle.ip_entries,
                ip,
                self.throttle.config.max_failures_per_ip,
                self.now,
                self.throttle.config.window,
                self.throttle.config.lockout,
                self.throttle.config.max_tracked_keys,
            );
        }
        self.resolved = true;
    }

    pub fn failed(mut self) {
        if let Some(ip) = self.ip {
            count_failure_in_map(
                &self.throttle.ip_entries,
                ip,
                self.throttle.config.max_failures_per_ip,
                self.now,
                self.throttle.config.window,
                self.throttle.config.lockout,
                self.throttle.config.max_tracked_keys,
            );
            let pair_key = (self.account.clone(), ip);
            count_failure_in_map(
                &self.throttle.pair_entries,
                pair_key,
                self.throttle.config.max_failures_per_account_ip,
                self.now,
                self.throttle.config.window,
                self.throttle.config.lockout,
                self.throttle.config.max_tracked_keys,
            );
        }

        count_failure_in_map(
            &self.throttle.account_entries,
            self.account.clone(),
            self.throttle.config.max_failures_per_account,
            self.now,
            self.throttle.config.window,
            self.throttle.config.lockout,
            self.throttle.config.max_tracked_keys,
        );

        self.throttle.release_in_flight(self.ip, &self.account, self.now);

        self.resolved = true;
    }

    pub fn succeeded(mut self) {
        self.throttle.release_in_flight(self.ip, &self.account, self.now);

        if let Some(ip) = self.ip {
            let pair_key = (self.account.clone(), ip);
            let mut map = self
                .throttle
                .pair_entries
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            map.remove(&pair_key);
        }

        self.resolved = true;
    }
}

impl Drop for LoginAttempt<'_> {
    fn drop(&mut self) {
        if !self.resolved {
            self.throttle
                .release_in_flight(self.ip, &self.account, self.now);
        }
    }
}

pub struct LoginThrottle {
    config: LoginThrottleConfig,
    ip_entries: Mutex<HashMap<IpAddr, Entry>>,
    pair_entries: Mutex<HashMap<(String, IpAddr), Entry>>,
    account_entries: Mutex<HashMap<String, Entry>>,
    warned_untrusted_forwarding: AtomicBool,
}

impl LoginThrottle {
    pub fn new(config: LoginThrottleConfig) -> Self {
        let mut config = config;
        config.trusted_proxies = config.trusted_proxies.into_iter().map(canonical).collect();
        Self {
            config,
            ip_entries: Mutex::new(HashMap::new()),
            pair_entries: Mutex::new(HashMap::new()),
            account_entries: Mutex::new(HashMap::new()),
            warned_untrusted_forwarding: AtomicBool::new(false),
        }
    }

    pub fn begin(&self, ip: Option<IpAddr>, account_key: &str) -> Option<LoginAttempt<'_>> {
        self.begin_at(ip, account_key, Instant::now())
    }

    pub fn begin_at(
        &self,
        ip: Option<IpAddr>,
        account_key: &str,
        now: Instant,
    ) -> Option<LoginAttempt<'_>> {
        let ip = ip.map(ip_key);

        // Fixed lock order: ip, pair, account.
        let mut ip_guard = ip.map(|_| self.ip_entries.lock().unwrap_or_else(|e| e.into_inner()));
        let mut pair_guard = self.pair_entries.lock().unwrap_or_else(|e| e.into_inner());
        let mut account_guard = self.account_entries.lock().unwrap_or_else(|e| e.into_inner());

        let pair_key = ip.map(|key| (account_key.to_string(), key));
        let account_key_owned = account_key.to_string();

        // Check applicable buckets.
        if let Some(current_ip) = ip
            && let Some(map) = ip_guard.as_mut()
            && let Some(entry) = map.get_mut(&current_ip)
            && blocked(entry, self.config.max_failures_per_ip, now, self.config.window)
        {
            return None;
        }

        if let Some(key) = &pair_key
            && let Some(entry) = pair_guard.get_mut(key)
            && blocked(entry, self.config.max_failures_per_account_ip, now, self.config.window)
        {
            return None;
        }

        if let Some(entry) = account_guard.get_mut(&account_key_owned)
            && blocked(entry, self.config.max_failures_per_account, now, self.config.window)
        {
            return None;
        }

        // Reserve in each applicable bucket.
        if let Some(current_ip) = ip
            && let Some(map) = ip_guard.as_mut()
        {
            ensure_entry(
                map,
                &current_ip,
                now,
                self.config.window,
                self.config.lockout,
                self.config.max_tracked_keys,
            );
            if let Some(entry) = map.get_mut(&current_ip) {
                refresh_entry(entry, now, self.config.window);
                entry.in_flight = entry.in_flight.saturating_add(1);
            }
        }

        if let Some(key) = &pair_key
            && let Some(map) = Some(&mut pair_guard)
        {
            ensure_entry(
                map,
                key,
                now,
                self.config.window,
                self.config.lockout,
                self.config.max_tracked_keys,
            );
            if let Some(entry) = map.get_mut(key) {
                refresh_entry(entry, now, self.config.window);
                entry.in_flight = entry.in_flight.saturating_add(1);
            }
        }

        {
            let map = &mut account_guard;
            ensure_entry(
                map,
                &account_key_owned,
                now,
                self.config.window,
                self.config.lockout,
                self.config.max_tracked_keys,
            );
            if let Some(entry) = map.get_mut(&account_key_owned) {
                refresh_entry(entry, now, self.config.window);
                entry.in_flight = entry.in_flight.saturating_add(1);
            }
        }

        drop(ip_guard);
        drop(pair_guard);
        drop(account_guard);

        Some(LoginAttempt {
            throttle: self,
            ip,
            account: account_key_owned,
            now,
            resolved: false,
        })
    }

    pub fn client_ip(&self, peer: Option<SocketAddr>, headers: &HeaderMap) -> Option<IpAddr> {
        let peer_ip_raw = peer.map(|addr| addr.ip())?;
        let peer_ip = canonical(peer_ip_raw);

        if !self.config.trusted_proxies.contains(&peer_ip) {
            if peer_ip.is_loopback()
                && headers.contains_key("x-forwarded-for")
                && !self
                    .warned_untrusted_forwarding
                    .swap(true, Ordering::Relaxed)
            {
                tracing::warn!(
                    "forwarding headers from an untrusted peer are ignored; set [LoginService] TrustedProxies"
                );
            }
            return Some(peer_ip);
        }

        let mut entries: Vec<&[u8]> = Vec::new();
        for value in headers.get_all("x-forwarded-for").iter() {
            for part in value.as_bytes().split(|byte| *byte == b',') {
                entries.push(part);
            }
        }

        if entries.is_empty() {
            if let Some(value) = headers.get("x-real-ip")
                && let Ok(text) = value.to_str()
            {
                let trimmed = text.trim();
                if let Ok(ip) = trimmed.parse::<IpAddr>() {
                    let ip = canonical(ip);
                    if !self.config.trusted_proxies.contains(&ip) {
                        return Some(ip);
                    }
                }
            }
            return None;
        }

        for entry in entries.iter().rev() {
            let trimmed = trim_ascii(entry);
            if trimmed.is_empty() {
                continue;
            }
            let text = match std::str::from_utf8(trimmed) {
                Ok(text) => text,
                Err(_) => return None,
            };
            let ip: IpAddr = match text.parse() {
                Ok(ip) => ip,
                Err(_) => return None,
            };
            let ip = canonical(ip);
            if self.config.trusted_proxies.contains(&ip) {
                continue;
            }
            return Some(ip);
        }

        None
    }

    fn release_in_flight(&self, ip: Option<IpAddr>, account: &str, now: Instant) {
        if let Some(ip) = ip {
            release_one(&self.ip_entries, &ip, now);
            let pair_key = (account.to_string(), ip);
            release_one(&self.pair_entries, &pair_key, now);
        }
        release_one(&self.account_entries, &account.to_string(), now);
    }
}

fn canonical(ip: IpAddr) -> IpAddr {
    ip.to_canonical()
}

pub fn ip_key(ip: IpAddr) -> IpAddr {
    let ip = ip.to_canonical();
    match ip {
        IpAddr::V4(address) => IpAddr::V4(address),
        IpAddr::V6(address) => {
            let value: u128 = address.into();
            let masked = value & !((1u128 << 64) - 1);
            IpAddr::V6(Ipv6Addr::from(masked))
        }
    }
}

fn clear_expired_lock(entry: &mut Entry, now: Instant) {
    if let Some(until) = entry.locked_until {
        if now >= until {
            entry.locked_until = None;
            entry.failures = 0;
            entry.window_start = now;
        }
    }
}

fn blocked(entry: &mut Entry, limit: u32, now: Instant, window: Duration) -> bool {
    if let Some(until) = entry.locked_until {
        if now >= until {
            entry.locked_until = None;
            entry.failures = 0;
            entry.window_start = now;
        } else {
            return true;
        }
    }
    let window_expired = now
        .checked_duration_since(entry.window_start)
        .unwrap_or_default()
        >= window;
    if window_expired {
        false
    } else {
        entry.failures.saturating_add(entry.in_flight) >= limit
    }
}

fn refresh_entry(entry: &mut Entry, now: Instant, window: Duration) {
    let window_expired = now
        .checked_duration_since(entry.window_start)
        .unwrap_or_default()
        >= window;
    if window_expired {
        entry.window_start = now;
        entry.failures = 0;
        if entry.locked_until.map_or(true, |until| now >= until) {
            entry.locked_until = None;
        }
    } else if let Some(until) = entry.locked_until {
        if now >= until {
            entry.locked_until = None;
            entry.failures = 0;
        }
    }
}

fn expired_entry(entry: &Entry, now: Instant, window: Duration, lockout: Duration) -> bool {
    let window_elapsed = now
        .checked_duration_since(entry.window_start)
        .unwrap_or_default()
        >= window;
    let lockout_elapsed = entry.locked_until.map_or(true, |until| now >= until);
    window_elapsed && lockout_elapsed && entry.in_flight == 0
    && now.checked_duration_since(entry.window_start).unwrap_or_default() >= Duration::ZERO
    && lockout >= Duration::ZERO
}

fn ensure_entry<K>(
    map: &mut HashMap<K, Entry>,
    key: &K,
    now: Instant,
    window: Duration,
    lockout: Duration,
    max: usize,
) where
    K: Clone + Eq + std::hash::Hash,
{
    if !map.contains_key(key) && max > 0 && map.len() >= max {
        map.retain(|_, entry| !expired_entry(entry, now, window, lockout));
        if map.len() >= max {
            let candidate = map
                .iter()
                .filter(|(_, entry)| entry.locked_until.map_or(true, |until| now >= until))
                .min_by_key(|(_, entry)| entry.window_start)
                .map(|(existing, _)| existing.clone());

            let candidate = candidate.or_else(|| {
                map.iter()
                    .min_by_key(|(_, entry)| entry.window_start)
                    .map(|(existing, _)| existing.clone())
            });

            if let Some(key) = candidate {
                map.remove(&key);
            }
        }
    }

    let owned_key = key.clone();
    map.entry(owned_key).or_insert(Entry {
        window_start: now,
        failures: 0,
        in_flight: 0,
        locked_until: None,
    });

    if let Some(entry) = map.get_mut(key) {
        refresh_entry(entry, now, window);
    }
}

fn count_failure_in_map<K>(
    mutex: &Mutex<HashMap<K, Entry>>,
    key: K,
    limit: u32,
    now: Instant,
    window: Duration,
    lockout: Duration,
    max: usize,
) where
    K: Clone + Eq + std::hash::Hash,
{
    let mut map = mutex.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(entry) = map.get_mut(&key) {
        clear_expired_lock(entry, now);
    }
    ensure_entry(&mut map, &key, now, window, lockout, max);
    if let Some(entry) = map.get_mut(&key) {
        refresh_entry(entry, now, window);
        entry.failures = entry.failures.saturating_add(1);
        if entry.failures >= limit {
            entry.locked_until = Some(now + lockout);
        }
    }
}

fn release_one<K>(map_mutex: &Mutex<HashMap<K, Entry>>, key: &K, _now: Instant)
where
    K: Eq + std::hash::Hash,
{
    let mut map = map_mutex.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(entry) = map.get_mut(key) {
        entry.in_flight = entry.in_flight.saturating_sub(1);
        let should_remove = entry.in_flight == 0
            && entry.failures == 0
            && entry.locked_until.is_none();
        if should_remove {
            map.remove(key);
        }
    }
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = bytes.len();
    while start < end
        && matches!(
            bytes[start],
            b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c
        )
    {
        start += 1;
    }
    while end > start
        && matches!(
            bytes[end - 1],
            b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c
        )
    {
        end -= 1;
    }
    &bytes[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

    #[test]
    fn two_different_ipv4_peers_do_not_share() {
        let config = LoginThrottleConfig {
            max_failures_per_ip: 1,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();
        let a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let b = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));

        let attempt = throttle.begin_at(Some(a), "alice", now).unwrap();
        attempt.failed();

        assert!(throttle.begin_at(Some(a), "bob", now).is_none());
        assert!(throttle.begin_at(Some(a), "carol", now).is_none());
        assert!(throttle.begin_at(Some(b), "bob", now).is_some());
    }

    #[test]
    fn successes_never_block() {
        let throttle = LoginThrottle::new(Default::default());
        let now = Instant::now();
        for _ in 0..10 {
            let attempt = throttle.begin_at(None, "alice", now).unwrap();
            attempt.succeeded();
        }
        assert!(throttle.begin_at(None, "alice", now).is_some());
    }

    #[test]
    fn six_begin_with_limit_five() {
        let config = LoginThrottleConfig {
            max_failures_per_account_ip: 5,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let mut attempts = Vec::new();
        for _ in 0..5 {
            attempts.push(throttle.begin_at(Some(ip), "alice", now).unwrap());
        }
        assert!(throttle.begin_at(Some(ip), "alice", now).is_none());
        drop(attempts.pop().unwrap());
        assert!(throttle.begin_at(Some(ip), "alice", now).is_some());
    }

    #[test]
    fn account_wide_backstop_expires() {
        let config = LoginThrottleConfig {
            max_failures_per_account: 2,
            lockout: Duration::from_secs(10),
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();
        for i in 0..2 {
            let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, i + 1));
            let attempt = throttle.begin_at(Some(ip), "alice", now).unwrap();
            attempt.failed();
        }
        let third_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3));
        assert!(throttle.begin_at(Some(third_ip), "alice", now).is_none());
        assert!(throttle.begin_at(Some(third_ip), "alice", now + Duration::from_secs(10)).is_some());
    }

    #[test]
    fn ipv6_same_prefix_shares_bucket() {
        let config = LoginThrottleConfig {
            max_failures_per_ip: 1,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();
        let first: IpAddr = "2001:db8::1".parse().unwrap();
        let second: IpAddr = "2001:db8::2".parse().unwrap();
        let attempt = throttle.begin_at(Some(first), "alice", now).unwrap();
        attempt.failed();
        assert!(throttle.begin_at(Some(second), "bob", now).is_none());
    }

    #[test]
    fn ipv4_mapped_shares_with_plain() {
        let config = LoginThrottleConfig {
            max_failures_per_ip: 1,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();
        let mapped: IpAddr = "::ffff:1.2.3.4".parse().unwrap();
        let plain = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        let attempt = throttle.begin_at(Some(mapped), "alice", now).unwrap();
        attempt.failed();
        assert!(throttle.begin_at(Some(plain), "bob", now).is_none());
    }

    #[test]
    fn unknown_account_does_not_create_account_or_pair() {
        let throttle = LoginThrottle::new(Default::default());
        let now = Instant::now();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let attempt = throttle.begin_at(Some(ip), "missing", now).unwrap();
        attempt.unknown_account();

        let pairs = throttle.pair_entries.lock().unwrap_or_else(|e| e.into_inner());
        let accounts = throttle.account_entries.lock().unwrap_or_else(|e| e.into_inner());
        assert!(pairs.is_empty());
        assert!(accounts.is_empty());
    }

    #[test]
    fn client_ip_untrusted_with_xff_returns_peer() {
        let throttle = LoginThrottle::new(Default::default());
        let peer_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9));
        let peer = Some(SocketAddr::new(peer_ip, 4000));
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "198.51.100.7".parse().unwrap());
        assert_eq!(throttle.client_ip(peer, &headers), Some(peer_ip));
    }

    #[test]
    fn client_ip_trusted_xff_simple() {
        let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let config = LoginThrottleConfig {
            trusted_proxies: vec![proxy_ip],
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let peer = Some(SocketAddr::new(proxy_ip, 4000));
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "6.6.6.6, 7.7.7.7".parse().unwrap());
        assert_eq!(
            throttle.client_ip(peer, &headers),
            Some(IpAddr::V4(Ipv4Addr::new(7, 7, 7, 7)))
        );
    }

    #[test]
    fn client_ip_trusted_two_lines() {
        let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let config = LoginThrottleConfig {
            trusted_proxies: vec![proxy_ip],
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let peer = Some(SocketAddr::new(proxy_ip, 4000));
        let mut headers = HeaderMap::new();
        headers.append("x-forwarded-for", "6.6.6.6".parse().unwrap());
        headers.append("x-forwarded-for", "7.7.7.7".parse().unwrap());
        assert_eq!(
            throttle.client_ip(peer, &headers),
            Some(IpAddr::V4(Ipv4Addr::new(7, 7, 7, 7)))
        );
    }

    #[test]
    fn client_ip_trusted_unknown_returns_none() {
        let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let config = LoginThrottleConfig {
            trusted_proxies: vec![proxy_ip],
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let peer = Some(SocketAddr::new(proxy_ip, 4000));
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4, unknown".parse().unwrap());
        assert_eq!(throttle.client_ip(peer, &headers), None);
    }

    #[test]
    fn client_ip_trusted_non_utf8_returns_none() {
        let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let config = LoginThrottleConfig {
            trusted_proxies: vec![proxy_ip],
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let peer = Some(SocketAddr::new(proxy_ip, 4000));
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            axum::http::HeaderValue::from_bytes(b"\xff").unwrap(),
        );
        assert_eq!(throttle.client_ip(peer, &headers), None);
    }

    #[test]
    fn client_ip_trusted_only_trusted_returns_none() {
        let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let config = LoginThrottleConfig {
            trusted_proxies: vec![proxy_ip],
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let peer = Some(SocketAddr::new(proxy_ip, 4000));
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "10.0.0.1".parse().unwrap());
        assert_eq!(throttle.client_ip(peer, &headers), None);
    }

    #[test]
    fn client_ip_real_ip_fallback() {
        let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let config = LoginThrottleConfig {
            trusted_proxies: vec![proxy_ip],
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let peer = Some(SocketAddr::new(proxy_ip, 4000));
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "5.5.5.5".parse().unwrap());
        assert_eq!(
            throttle.client_ip(peer, &headers),
            Some(IpAddr::V4(Ipv4Addr::new(5, 5, 5, 5)))
        );
    }

    #[test]
    fn client_ip_mapped_trusted_proxy() {
        let proxy = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
        let config = LoginThrottleConfig {
            trusted_proxies: vec![proxy],
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let peer = Some(SocketAddr::new("::ffff:10.0.0.5".parse().unwrap(), 4000));
        assert_eq!(throttle.client_ip(peer, &HeaderMap::new()), None);

        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "7.7.7.7".parse().unwrap());
        assert_eq!(
            throttle.client_ip(peer, &headers),
            Some(IpAddr::V4(Ipv4Addr::new(7, 7, 7, 7)))
        );
    }

    #[test]
    fn full_table_records_new_failure() {
        let config = LoginThrottleConfig {
            max_tracked_keys: 1,
            max_failures_per_ip: 1,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();
        let first = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let attempt = throttle.begin_at(Some(first), "alice", now).unwrap();
        attempt.failed();

        let second = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        let attempt = throttle.begin_at(Some(second), "bob", now).unwrap();
        attempt.failed();

        {
            let map = throttle.ip_entries.lock().unwrap_or_else(|e| e.into_inner());
            assert_eq!(map.len(), 1);
            assert!(map.contains_key(&ip_key(second)));
        }
        assert!(throttle.begin_at(Some(second), "carol", now).is_none());
    }

    #[test]
    fn table_bound_large() {
        let config = LoginThrottleConfig {
            max_tracked_keys: 5,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();
        for i in 0..15u8 {
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i));
            if let Some(attempt) = throttle.begin_at(Some(ip), &format!("account-{i}"), now) {
                attempt.failed();
            }
        }
        let ips = throttle.ip_entries.lock().unwrap_or_else(|e| e.into_inner());
        let accounts = throttle.account_entries.lock().unwrap_or_else(|e| e.into_inner());
        assert!(ips.len() <= 5);
        assert!(accounts.len() <= 5);
        let _ = Ipv6Addr::LOCALHOST;
    }

    #[test]
    fn unknown_names_leave_no_account_entries() {
        let throttle = LoginThrottle::new(Default::default());
        let now = Instant::now();
        for i in 0..100usize {
            let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, (i + 1) as u8));
            let attempt = throttle
                .begin_at(Some(ip), &format!("missing-{i}"), now)
                .unwrap();
            attempt.unknown_account();
        }

        let pair_entries = throttle.pair_entries.lock().unwrap_or_else(|e| e.into_inner());
        let account_entries = throttle.account_entries.lock().unwrap_or_else(|e| e.into_inner());
        assert!(pair_entries.is_empty());
        assert!(account_entries.is_empty());
    }

    #[test]
    fn success_leaves_no_entries_but_failures_stay() {
        let throttle = LoginThrottle::new(Default::default());
        let now = Instant::now();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        let success_attempt = throttle.begin_at(Some(ip), "successful", now).unwrap();
        success_attempt.succeeded();

        {
            let pair_entries = throttle.pair_entries.lock().unwrap_or_else(|e| e.into_inner());
            let account_entries = throttle.account_entries.lock().unwrap_or_else(|e| e.into_inner());
            assert!(pair_entries.is_empty());
            assert!(account_entries.is_empty());
        }

        let failed_attempt = throttle.begin_at(Some(ip), "failed", now).unwrap();
        failed_attempt.failed();

        let pair_entries = throttle.pair_entries.lock().unwrap_or_else(|e| e.into_inner());
        let account_entries = throttle.account_entries.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(pair_entries.len(), 1);
        assert_eq!(account_entries.len(), 1);

        let pair_key = ("failed".to_string(), ip_key(ip));
        let pair_entry = pair_entries.get(&pair_key).unwrap();
        assert_eq!(pair_entry.failures, 1);
        assert_eq!(pair_entry.in_flight, 0);
        assert!(pair_entry.locked_until.is_none());

        let account_entry = account_entries.get("failed").unwrap();
        assert_eq!(account_entry.failures, 1);
        assert_eq!(account_entry.in_flight, 0);
        assert!(account_entry.locked_until.is_none());
    }
}
