// SPDX-License-Identifier: LGPL-3.0-or-later

//! Per-IP and per-account login throttle with expiring windows and temporary lockouts.
//! Tracks failure counts, resolves client IPs through trusted proxies, and
//! evicts least-recently-used unlocked entries when capacity is reached.
//! Known limitation: `is_blocked` and `record_failure` are separate calls, so
//! concurrent requests can overshoot a limit by the number in flight.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::HeaderMap;

#[derive(Clone, Debug)]
pub struct LoginThrottleConfig {
    pub max_failures_per_ip: u32,
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
            max_failures_per_account: 5,
            window: Duration::from_secs(300),
            lockout: Duration::from_secs(300),
            max_tracked_keys: 10_000,
            trusted_proxies: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
struct Entry {
    window_start: Instant,
    failures: u32,
    locked_until: Option<Instant>,
}

pub struct LoginThrottle {
    config: LoginThrottleConfig,
    ip_entries: Mutex<HashMap<IpAddr, Entry>>,
    account_entries: Mutex<HashMap<String, Entry>>,
}

impl LoginThrottle {
    pub fn new(config: LoginThrottleConfig) -> Self {
        let mut config = config;
        config.trusted_proxies = config.trusted_proxies.into_iter().map(canonical).collect();
        Self {
            config,
            ip_entries: Mutex::new(HashMap::new()),
            account_entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn is_blocked(&self, ip: Option<IpAddr>, account: &str) -> bool {
        self.is_blocked_at(ip, account, Instant::now())
    }

    pub fn is_blocked_at(&self, ip: Option<IpAddr>, account: &str, now: Instant) -> bool {
        let account_key = account.to_lowercase();
        let accounts = self
            .account_entries
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = accounts.get(&account_key)
            && let Some(until) = entry.locked_until
            && now < until
        {
            return true;
        }
        drop(accounts);

        if let Some(ip) = ip {
            let ip = canonical(ip);
            let ips = self.ip_entries.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(entry) = ips.get(&ip)
                && let Some(until) = entry.locked_until
                && now < until
            {
                return true;
            }
        }
        false
    }

    pub fn record_failure(&self, ip: Option<IpAddr>, account: &str) {
        self.record_failure_at(ip, account, Instant::now());
    }

    pub fn record_failure_at(&self, ip: Option<IpAddr>, account: &str, now: Instant) {
        let account_key = account.to_lowercase();
        record_in_map(
            &self.account_entries,
            account_key,
            now,
            self.config.max_failures_per_account,
            self.config.window,
            self.config.lockout,
            self.config.max_tracked_keys,
        );
        if let Some(ip) = ip {
            let ip = canonical(ip);
            record_in_map(
                &self.ip_entries,
                ip,
                now,
                self.config.max_failures_per_ip,
                self.config.window,
                self.config.lockout,
                self.config.max_tracked_keys,
            );
        }
    }

    pub fn record_success(&self, account: &str) {
        self.record_success_at(account);
    }

    pub fn record_success_at(&self, account: &str) {
        let account_key = account.to_lowercase();
        let mut accounts = self
            .account_entries
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        accounts.remove(&account_key);
    }

    pub fn client_ip(&self, peer: Option<SocketAddr>, headers: &HeaderMap) -> Option<IpAddr> {
        self.client_ip_at(peer, headers)
    }

    pub fn client_ip_at(&self, peer: Option<SocketAddr>, headers: &HeaderMap) -> Option<IpAddr> {
        let peer_ip = canonical(peer.map(|addr| addr.ip())?);
        if !self.config.trusted_proxies.contains(&peer_ip) {
            return Some(peer_ip);
        }

        let mut forwarded = Vec::new();
        for header in headers.get_all("x-forwarded-for").iter() {
            let value = match header.to_str() {
                Ok(value) => value,
                Err(_) => return Some(peer_ip),
            };
            forwarded.extend(value.split(','));
        }

        for entry in forwarded.into_iter().rev() {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            match trimmed.parse::<IpAddr>() {
                Ok(ip) => {
                    let ip = canonical(ip);
                    if !self.config.trusted_proxies.contains(&ip) {
                        return Some(ip);
                    }
                }
                Err(_) => return Some(peer_ip),
            }
        }

        if let Some(value) = headers.get("x-real-ip").and_then(|h| h.to_str().ok()) {
            let trimmed = value.trim();
            if let Ok(ip) = trimmed.parse::<IpAddr>() {
                return Some(canonical(ip));
            }
        }

        Some(peer_ip)
    }
}

fn canonical(ip: IpAddr) -> IpAddr {
    ip.to_canonical()
}

fn expired(entry: &Entry, now: Instant, window: Duration) -> bool {
    let window_elapsed = now
        .checked_duration_since(entry.window_start)
        .unwrap_or_default()
        >= window;
    let lockout_expired = entry.locked_until.map_or(true, |until| now >= until);
    window_elapsed && lockout_expired
}

fn record_in_map<K>(
    mutex: &Mutex<HashMap<K, Entry>>,
    key: K,
    now: Instant,
    threshold: u32,
    window: Duration,
    lockout: Duration,
    max_keys: usize,
) where
    K: Clone + Eq + std::hash::Hash,
{
    if max_keys == 0 {
        return;
    }

    let mut map = mutex.lock().unwrap_or_else(|e| e.into_inner());

    if !map.contains_key(&key) && map.len() >= max_keys {
        map.retain(|_, entry| !expired(entry, now, window));
        if map.len() >= max_keys {
            let candidate = map
                .iter()
                .filter(|(_, entry)| entry.locked_until.map_or(true, |until| now >= until))
                .min_by_key(|(_, entry)| entry.window_start)
                .map(|(existing, _)| existing.clone());
            match candidate {
                Some(oldest_key) => {
                    map.remove(&oldest_key);
                }
                None => return,
            }
        }
    }

    let entry = map.entry(key).or_insert(Entry {
        window_start: now,
        failures: 0,
        locked_until: None,
    });

    if now
        .checked_duration_since(entry.window_start)
        .unwrap_or_default()
        >= window
    {
        entry.window_start = now;
        entry.failures = 0;
        if entry.locked_until.map_or(true, |until| now >= until) {
            entry.locked_until = None;
        }
    }

    if let Some(until) = entry.locked_until {
        if now >= until {
            entry.locked_until = None;
            entry.failures = 0;
        }
    }

    entry.failures = entry.failures.saturating_add(1);
    if threshold == 0 || entry.failures >= threshold {
        entry.locked_until = Some(now + lockout);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr};

    #[test]
    fn two_peer_ips_do_not_share_a_bucket() {
        let config = LoginThrottleConfig {
            max_failures_per_ip: 1,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let ip_one = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip_two = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        let now = Instant::now();

        throttle.record_failure_at(Some(ip_one), "account-a", now);

        assert!(throttle.is_blocked_at(Some(ip_one), "account-b", now));
        assert!(!throttle.is_blocked_at(Some(ip_two), "account-b", now));
    }

    #[test]
    fn successful_logins_never_cause_a_block() {
        let throttle = LoginThrottle::new(Default::default());
        let now = Instant::now();

        for _ in 0..10 {
            throttle.record_success_at("alice");
        }

        assert!(!throttle.is_blocked_at(None, "alice", now));
    }

    #[test]
    fn account_lockout_triggers_and_expires() {
        let config = LoginThrottleConfig {
            max_failures_per_account: 3,
            window: Duration::from_secs(10),
            lockout: Duration::from_secs(10),
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();

        for _ in 0..3 {
            throttle.record_failure_at(None, "alice", now);
        }

        assert!(throttle.is_blocked_at(None, "alice", now));
        assert!(!throttle.is_blocked_at(None, "alice", now + Duration::from_secs(10)));
    }

    #[test]
    fn spoofed_forwarded_for_from_untrusted_peer_is_ignored() {
        let throttle = LoginThrottle::new(Default::default());
        let peer_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9));
        let peer = Some(SocketAddr::new(peer_ip, 4000));
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "198.51.100.7".parse().unwrap());

        assert_eq!(throttle.client_ip(peer, &headers), Some(peer_ip));
    }

    #[test]
    fn forwarded_for_from_trusted_proxy_is_honoured() {
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
            "203.0.113.7, 10.0.0.1".parse().unwrap(),
        );

        assert_eq!(
            throttle.client_ip(peer, &headers),
            Some(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)))
        );
    }

    #[test]
    fn table_stays_bounded() {
        let config = LoginThrottleConfig {
            max_tracked_keys: 5,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();

        for i in 0..15u8 {
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i));
            throttle.record_failure_at(Some(ip), &format!("account-{i}"), now);
        }

        let ips = throttle
            .ip_entries
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let accounts = throttle
            .account_entries
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert!(ips.len() <= 5);
        assert!(accounts.len() <= 5);
    }

    #[test]
    fn none_ip_does_not_block_other_clients() {
        let config = LoginThrottleConfig {
            max_failures_per_ip: 1,
            max_failures_per_account: 1,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();

        throttle.record_failure_at(None, "bad-account", now);

        assert!(throttle.is_blocked_at(None, "bad-account", now));
        let other_ip = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 3));
        assert!(!throttle.is_blocked_at(Some(other_ip), "good-account", now));
    }

    #[test]
    fn full_locked_table_retains_lockouts() {
        let config = LoginThrottleConfig {
            max_tracked_keys: 2,
            max_failures_per_ip: 1,
            max_failures_per_account: 100,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();
        let ip_one = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip_two = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        let ip_new = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3));

        throttle.record_failure_at(Some(ip_one), "shared-account", now);
        throttle.record_failure_at(Some(ip_two), "shared-account", now);

        assert!(throttle.is_blocked_at(Some(ip_one), "shared-account", now));
        assert!(throttle.is_blocked_at(Some(ip_two), "shared-account", now));

        throttle.record_failure_at(Some(ip_new), "shared-account", now);

        let ips = throttle
            .ip_entries
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(ips.len(), 2);
        drop(ips);

        assert!(throttle.is_blocked_at(Some(ip_one), "shared-account", now));
        assert!(throttle.is_blocked_at(Some(ip_two), "shared-account", now));
        assert!(!throttle.is_blocked_at(Some(ip_new), "shared-account", now));
    }

    #[test]
    fn mapped_peer_ip_is_treated_as_trusted_proxy() {
        let proxy = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
        let config = LoginThrottleConfig {
            trusted_proxies: vec![proxy],
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let peer = Some(SocketAddr::new("::ffff:10.0.0.5".parse().unwrap(), 4000));

        assert_eq!(throttle.client_ip(peer, &HeaderMap::new()), Some(proxy));
    }

    #[test]
    fn mapped_and_unmapped_ips_share_a_bucket() {
        let config = LoginThrottleConfig {
            max_failures_per_ip: 1,
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let now = Instant::now();
        let mapped: IpAddr = "::ffff:1.2.3.4".parse().unwrap();
        let unmapped = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        throttle.record_failure_at(Some(mapped), "account", now);

        assert!(throttle.is_blocked_at(Some(unmapped), "account", now));
    }

    #[test]
    fn forwarded_for_with_unknown_entry_returns_peer_ip() {
        let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let config = LoginThrottleConfig {
            trusted_proxies: vec![proxy_ip],
            ..Default::default()
        };
        let throttle = LoginThrottle::new(config);
        let peer_ip = proxy_ip;
        let peer = Some(SocketAddr::new(peer_ip, 4000));
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "6.6.6.6, unknown".parse().unwrap());

        assert_eq!(throttle.client_ip(peer, &headers), Some(peer_ip));
    }

    #[test]
    fn forwarded_for_returns_rightmost_untrusted_entry() {
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
    fn forwarded_for_uses_all_header_lines() {
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
}
