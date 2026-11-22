// SPDX-License-Identifier: LGPL-3.0-or-later
//! Outbound URL filter and HTTP request throttle for `llHTTPRequest`.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use std::time::Instant;
use reqwest::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpNetwork {
    addr: u32,
    mask: u32,
}

impl IpNetwork {
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        let (ip_str, prefix_str) = match s.split_once('/') {
            Some((a, b)) => (a.trim(), Some(b.trim())),
            None => (s, None),
        };
        let ip: Ipv4Addr = ip_str.parse().map_err(|_| format!("invalid IPv4 address: {ip_str}"))?;
        let prefix: u8 = match prefix_str {
            None => 32,
            Some(p) => p.parse::<u8>().map_err(|_| format!("invalid prefix: {p}"))?,
        };
        if prefix > 32 { return Err(format!("prefix out of range: {prefix}")); }
        let mask = if prefix == 0 { 0u32 } else { u32::MAX << (32 - prefix) };
        Ok(Self { addr: u32::from(ip) & mask, mask })
    }
    pub fn contains(&self, ip: &Ipv4Addr) -> bool {
        (u32::from(*ip) & self.mask) == self.addr
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndPoint { pub addr: Ipv4Addr, pub port: u16 }

#[derive(Debug, Clone)]
pub struct OutboundUrlFilter {
    pub name: String,
    blacklist_networks: Vec<IpNetwork>,
    blacklist_endpoints: Vec<EndPoint>,
    exception_networks: Vec<IpNetwork>,
    exception_endpoints: Vec<EndPoint>,
}

impl OutboundUrlFilter {
    pub const DEFAULT_BLACKLIST: &'static str = "0.0.0.0/8|10.0.0.0/8|100.64.0.0/10|127.0.0.0/8|169.254.0.0/16|172.16.0.0/12|192.0.0.0/24|192.0.2.0/24|192.88.99.0/24|192.168.0.0/16|198.18.0.0/15|198.51.100.0/24|203.0.113.0/24|224.0.0.0/4|240.0.0.0/4|255.255.255.255/32";

    pub fn new(name: impl Into<String>, blacklist_networks: Vec<IpNetwork>, blacklist_endpoints: Vec<EndPoint>, exception_networks: Vec<IpNetwork>, exception_endpoints: Vec<EndPoint>) -> Self {
        Self { name: name.into(), blacklist_networks, blacklist_endpoints, exception_networks, exception_endpoints }
    }

    pub fn from_config_strings(name: impl Into<String>, blacklist: &str, exceptions: &str) -> Self {
        let (bn, be) = parse_config_list(blacklist);
        let (en, ee) = parse_config_list(exceptions);
        Self::new(name, bn, be, en, ee)
    }

    pub fn from_default(name: impl Into<String>) -> Self {
        Self::from_config_strings(name, Self::DEFAULT_BLACKLIST, "")
    }

    pub fn name(&self) -> &str { &self.name }

    pub fn check_allowed(&self, url: &str) -> bool {
        let parsed = match Url::parse(url) { Ok(u) => u, Err(_) => return false };
        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" { return false; }
        let host = match parsed.host_str() { Some(h) if !h.is_empty() => h.to_string(), _ => return false };
        let port = parsed.port_or_known_default().unwrap_or(if scheme == "https" { 443 } else { 80 });
        let addrs: Vec<IpAddr> = match (host.as_str(), port).to_socket_addrs() {
            Ok(iter) => iter.map(|sa| sa.ip()).collect(),
            Err(_) => return true,
        };
        let mut found_ipv4 = false;
        for addr in addrs {
            if let IpAddr::V4(v4) = addr {
                found_ipv4 = true;
                let in_blacklist = ip_in_list(&v4, port, &self.blacklist_networks, &self.blacklist_endpoints);
                if in_blacklist {
                    let excepted = ip_in_list(&v4, port, &self.exception_networks, &self.exception_endpoints);
                    if !excepted { return false; }
                }
            }
        }
        if !found_ipv4 { return false; }
        true
    }

    pub fn check_allowed_url(&self, url: &str) -> bool { self.check_allowed(url) }
    pub fn is_allowed(&self, url: &str) -> bool { self.check_allowed(url) }
    pub fn filter_url(&self, url: &str) -> bool { self.check_allowed(url) }
    pub fn url_allowed(&self, url: &str) -> bool { self.check_allowed(url) }
}

pub fn parse_config_list(input: &str) -> (Vec<IpNetwork>, Vec<EndPoint>) {
    let mut networks = Vec::new();
    let mut endpoints = Vec::new();
    for entry in input.split('|') {
        let entry = entry.trim();
        if entry.is_empty() { continue; }
        if entry.contains('/') {
            if let Ok(net) = IpNetwork::parse(entry) { networks.push(net); }
            continue;
        }
        let (host, port) = if let Some((h, p)) = entry.rsplit_once(':') {
            match p.parse::<u16>() { Ok(port) => (h.trim(), port), Err(_) => (entry, 80u16) }
        } else { (entry, 80u16) };
        let host_with_scheme = format!("http://{host}");
        if let Ok(url) = Url::parse(&host_with_scheme) {
            let host_str = url.host_str().unwrap_or(host);
            if let Ok(iter) = (host_str, port).to_socket_addrs() {
                for sa in iter {
                    if let IpAddr::V4(v4) = sa.ip() {
                        endpoints.push(EndPoint { addr: v4, port });
                    }
                }
            }
        }
    }
    (networks, endpoints)
}

fn ip_in_list(ip: &Ipv4Addr, port: u16, networks: &[IpNetwork], endpoints: &[EndPoint]) -> bool {
    for n in networks { if n.contains(ip) { return true; } }
    for ep in endpoints { if ep.addr == *ip && ep.port == port { return true; } }
    false
}

pub fn check_url_allowed(url: &str) -> bool { OutboundUrlFilter::from_default("default").check_allowed(url) }
pub fn is_url_allowed(url: &str) -> bool { check_url_allowed(url) }
pub fn filter_url(url: &str) -> bool { check_url_allowed(url) }
pub fn url_allowed(url: &str) -> bool { check_url_allowed(url) }

#[derive(Debug, Clone, Copy)]
struct ThrottleData { last_time: f64, control: f32 }

#[derive(Debug)]
pub struct HttpThrottle {
    pub prim_per_sec: f32,
    pub prim_burst: f32,
    pub owner_per_sec: f32,
    pub owner_burst: f32,
    prim: HashMap<u32, ThrottleData>,
    owner: HashMap<Uuid, ThrottleData>,
}

impl HttpThrottle {
    pub fn new() -> Self { Self { prim_per_sec: 1.0, prim_burst: 3.0, owner_per_sec: 25.0, owner_burst: 5.0, prim: HashMap::new(), owner: HashMap::new() } }
    pub fn with_rates(prim_per_sec: f32, prim_burst: f32, owner_per_sec: f32, owner_burst: f32) -> Self { Self { prim_per_sec, prim_burst, owner_per_sec, owner_burst, prim: HashMap::new(), owner: HashMap::new() } }
    pub fn check_throttle(&mut self, local_id: u32, owner_id: Uuid) -> bool {
        let now = now_ts();
        if let Some(th) = self.prim.get_mut(&local_id) {
            let delta = now - th.last_time;
            th.last_time = now;
            let add = self.prim_per_sec * delta as f32;
            th.control += add;
            if th.control > self.prim_burst { th.control = self.prim_burst - 1.0; }
            else {
                let ok = th.control > 0.0;
                if ok { th.control -= 1.0; } else { return false; }
            }
        } else {
            self.prim.insert(local_id, ThrottleData { last_time: now, control: self.prim_burst - 1.0 });
        }
        if let Some(th) = self.owner.get_mut(&owner_id) {
            let delta = now - th.last_time;
            th.last_time = now;
            let add = self.owner_per_sec * delta as f32;
            th.control += add;
            if th.control > self.owner_burst { th.control = self.owner_burst - 1.0; return true; }
            let ok = th.control > 0.0;
            if ok { th.control -= 1.0; }
            ok
        } else {
            self.owner.insert(owner_id, ThrottleData { last_time: now, control: self.owner_burst - 1.0 });
            true
        }
    }
}
impl Default for HttpThrottle { fn default() -> Self { Self::new() } }

fn now_ts() -> f64 {
    use std::sync::OnceLock;
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_secs_f64()
}

pub type IpFilter = OutboundUrlFilter;
pub type UrlFilter = OutboundUrlFilter;
pub type RequestThrottle = HttpThrottle;
pub type HttpRequestThrottle = HttpThrottle;
pub type HttpRequestUrlFilter = OutboundUrlFilter;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_network() {
        let n = IpNetwork::parse("127.0.0.0/8").unwrap();
        assert!(n.contains(&"127.1.2.3".parse().unwrap()));
        assert!(!n.contains(&"8.8.8.8".parse().unwrap()));
    }
    #[test]
    fn blocks_loopback() {
        let f = OutboundUrlFilter::from_default("t");
        assert!(!f.check_allowed("http://127.0.0.1/"));
        assert!(!f.check_allowed("http://localhost/"));
    }
    #[test]
    fn allows_public_ip() {
        let f = OutboundUrlFilter::from_default("t");
        assert!(f.check_allowed("http://8.8.8.8/"));
    }
    #[test]
    fn rejects_bad_scheme() {
        let f = OutboundUrlFilter::from_default("t");
        assert!(!f.check_allowed("ftp://8.8.8.8/"));
        assert!(!f.check_allowed("not a url"));
    }
    #[test]
    fn throttle_basic() {
        let mut t = HttpThrottle::with_rates(10.0, 3.0, 25.0, 5.0);
        let owner = Uuid::nil();
        assert!(t.check_throttle(1, owner));
    }
    #[test]
    fn aliases() {
        assert!(is_url_allowed("http://8.8.8.8/"));
        assert!(!url_allowed("http://127.0.0.1/"));
    }
}
