// SPDX-License-Identifier: GPL-3.0-or-later

use opensim_msg::build_raw;
use opensim_msg::{Msg, PacketFlags, Writer};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const MAX_RESENDS: u8 = 4;
const DEDUPE_WINDOW: usize = 2048;
const ACK_FLUSH_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_RTT: Duration = Duration::from_millis(300);

struct Unacked {
    payload: Vec<u8>,
    zerocoded: bool,
    sent: Instant,
    retries: u8,
    max_retries: u8,
    timeout: Option<Duration>,
}

pub struct Circuit {
    addr: SocketAddr,
    next_seq: u32,
    unacked: HashMap<u32, Unacked>,
    pending_acks: Vec<u32>,
    last_ack_flush: Instant,
    recent: VecDeque<u32>,
    recent_set: HashSet<u32>,
    pub last_recv: Instant,
    rtt: Duration,
    buf: Vec<u8>,
    dropped: Vec<u32>,
}

pub struct CircuitTick {
    pub packets: Vec<Vec<u8>>,
    pub dropped: Vec<u32>,
}

impl Circuit {
    pub fn new(addr: SocketAddr) -> Self {
        let now = Instant::now();
        Self {
            addr,
            next_seq: 1,
            unacked: HashMap::new(),
            pending_acks: Vec::new(),
            last_ack_flush: now,
            recent: VecDeque::new(),
            recent_set: HashSet::new(),
            last_recv: now,
            rtt: DEFAULT_RTT,
            buf: Vec::with_capacity(1500),
            dropped: Vec::new(),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn next_sequence(&mut self) -> u32 {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        seq
    }

    pub fn send_raw(
        &mut self,
        payload: &[u8],
        zerocoded: bool,
        reliable: bool,
    ) -> (Vec<u8>, u32) {
        let seq = self.next_sequence();
        let mut out = std::mem::take(&mut self.buf);
        let n = build_raw(
            payload,
            zerocoded,
            seq,
            reliable,
            false,
            &self.pending_acks,
            &mut out,
        );
        self.pending_acks.drain(..n);

        if reliable {
            self.unacked.insert(
                seq,
                Unacked {
                    payload: payload.to_vec(),
                    zerocoded,
                    sent: Instant::now(),
                    retries: 0,
                    max_retries: MAX_RESENDS,
                    timeout: None,
                },
            );
        }

        let result = out.clone();
        self.buf = out;
        (result, seq)
    }

    pub fn send_msg<M: Msg>(&mut self, msg: &M, reliable: bool) -> (Vec<u8>, u32) {
        let mut w = Writer::new();
        M::ID.encode(&mut w.buf);
        msg.encode_body(&mut w);
        self.send_raw(&w.buf, M::ZEROCODED, reliable)
    }

    pub fn on_receive(&mut self, flags: u8, seq: u32, acks: &[u32]) -> bool {
        self.last_recv = Instant::now();

        for ack in acks {
            self.unacked.remove(ack);
        }

        if flags & PacketFlags::RESENT != 0 && self.recent_set.contains(&seq) {
            if flags & PacketFlags::RELIABLE != 0 {
                self.pending_acks.push(seq);
            }
            return false;
        }

        true
    }

    pub fn accept(&mut self, flags: u8, seq: u32) {
        if flags & PacketFlags::RELIABLE != 0 {
            self.pending_acks.push(seq);
        }

        if self.recent_set.contains(&seq) {
            return;
        }

        if self.recent.len() >= DEDUPE_WINDOW {
            if let Some(old) = self.recent.pop_front() {
                self.recent_set.remove(&old);
            }
        }

        self.recent.push_back(seq);
        self.recent_set.insert(seq);
    }

    pub fn tick(&mut self) -> CircuitTick {
        let now = Instant::now();
        let timeout = (self.rtt * 3)
            .max(Duration::from_millis(500))
            .min(Duration::from_secs(3));

        let mut to_drop = Vec::new();
        let mut to_resend = Vec::new();

        for (seq, unacked) in &self.unacked {
            let elapsed = now.duration_since(unacked.sent);
            if elapsed > unacked.timeout.unwrap_or(timeout) {
                if unacked.retries >= unacked.max_retries {
                    to_drop.push(*seq);
                } else {
                    to_resend.push(*seq);
                }
            }
        }

        for seq in to_drop {
            self.unacked.remove(&seq);
            self.dropped.push(seq);
        }

        let mut packets = Vec::new();
        for seq in to_resend {
            if let Some(unacked) = self.unacked.get_mut(&seq) {
                unacked.retries += 1;
                unacked.sent = now;

                let mut out = std::mem::take(&mut self.buf);
                build_raw(
                    &unacked.payload,
                    unacked.zerocoded,
                    seq,
                    true,
                    true,
                    &[],
                    &mut out,
                );
                packets.push(out.clone());
                self.buf = out;
            }
        }

        if !self.pending_acks.is_empty()
            && now.duration_since(self.last_ack_flush) >= ACK_FLUSH_INTERVAL
        {
            packets.extend(self.flush_acks());
        }

        let dropped = std::mem::take(&mut self.dropped);
        CircuitTick { packets, dropped }
    }

    pub fn flush_acks(&mut self) -> Vec<Vec<u8>> {
        self.last_ack_flush = Instant::now();
        let mut result = Vec::new();

        while !self.pending_acks.is_empty() {
            let n = self.pending_acks.len().min(250);
            let ids: Vec<u32> = self.pending_acks.drain(..n).collect();

            let m = opensim_msg::msgs::PacketAck {
                packets: ids
                    .iter()
                    .map(|&id| opensim_msg::msgs::packet_ack::Packets { id })
                    .collect(),
            };

            let mut w = Writer::new();
            opensim_msg::msgs::PacketAck::ID.encode(&mut w.buf);
            m.encode_body(&mut w);

            let (pkt, _) = self.send_raw(&w.buf, false, false);
            result.push(pkt);
        }

        result
    }

    pub fn take_dropped(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.dropped)
    }

    pub fn update_rtt(&mut self, rtt: Duration) {
        self.rtt = (self.rtt * 7 + rtt) / 8;
    }

    pub fn rtt(&self) -> Duration {
        self.rtt
    }

    pub fn unacked_count(&self) -> usize {
        self.unacked.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Circuit {
        fn force_resend_timeout(&mut self, seq: u32) {
            if let Some(unacked) = self.unacked.get_mut(&seq) {
                unacked.sent = Instant::now() - Duration::from_secs(10);
            }
        }
    }

    #[test]
    fn test_sequence_numbers() {
        let mut c = Circuit::new("127.0.0.1:8000".parse().unwrap());
        assert_eq!(c.next_sequence(), 1);
        assert_eq!(c.next_sequence(), 2);
        assert_eq!(c.next_sequence(), 3);
    }

    #[test]
    fn test_send_and_ack() {
        let mut c = Circuit::new("127.0.0.1:8000".parse().unwrap());
        let (_pkt, seq) = c.send_raw(&[0xFF, 0x01], false, true);
        assert_eq!(c.unacked_count(), 1);
        c.on_receive(0, 0, &[seq]);
        assert_eq!(c.unacked_count(), 0);
    }

    #[test]
    fn test_duplicate_detection() {
        let mut c = Circuit::new("127.0.0.1:8000".parse().unwrap());
        c.accept(PacketFlags::RELIABLE, 5);
        assert!(!c.on_receive(
            PacketFlags::RESENT | PacketFlags::RELIABLE,
            5,
            &[]
        ));
    }

    #[test]
    fn test_tick_resend() {
        let mut c = Circuit::new("127.0.0.1:8000".parse().unwrap());
        let (_pkt, seq) = c.send_raw(&[0xFF, 0x01], false, true);
        c.force_resend_timeout(seq);
        let tick = c.tick();
        assert_eq!(tick.packets.len(), 1);
        assert!(tick.dropped.is_empty());
    }
}
