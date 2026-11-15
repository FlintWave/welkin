//! Raw UDP client for the OpenSim LLUDP protocol.
//!
//! This is a client-side circuit manager that connects to a region's simulator
//! over UDP, handles packet acknowledgement, ping keep-alive, and provides a
//! broadcast channel for dispatching parsed incoming packets to higher-level
//! handlers.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;
use welkin_msg::msgs::*;
use welkin_msg::{build_packet, parse_packet, IncomingPacket, Msg, PacketFlags};

use crate::error::RealmWalkerError;

/// Maximum number of retries for unacknowledged reliable packets before
/// declaring them lost.
const MAX_RETRIES: u8 = 5;

/// How long to wait for the initial UseCircuitCode ack before giving up.
const CIRCUIT_ACK_TIMEOUT: Duration = Duration::from_secs(10);

/// Interval for the ack/resend/ping maintenance loop.
const ACK_LOOP_INTERVAL: Duration = Duration::from_millis(100);

/// How long a reliable packet can go unacknowledged before resending.
const RESEND_INTERVAL: Duration = Duration::from_millis(500);

/// Interval between pings when idle.
const PING_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum UDP receive buffer size.
const RECV_BUF_SIZE: usize = 4096;

/// Broadcast channel capacity for incoming packets.
const PACKET_CHANNEL_CAPACITY: usize = 512;

/// A reliable packet awaiting acknowledgement from the server.
struct UnackedPacket {
    data: Vec<u8>,
    sent: Instant,
    retries: u8,
}

/// Raw UDP circuit to an OpenSim region simulator.
///
/// After calling [`Network::connect`], the caller should spawn both
/// [`run_packet_pump`] and [`run_ack_loop`] as concurrent tasks. Incoming
/// packets are distributed via the broadcast channel returned by
/// [`subscribe`].
pub struct Network {
    socket: Arc<UdpSocket>,
    server_addr: SocketAddr,
    agent_id: Uuid,
    session_id: Uuid,
    sequence: AtomicU32,
    /// Acks we owe the server (their reliable packet sequence numbers).
    pending_acks: Mutex<Vec<u32>>,
    /// Our reliable packets that the server has not yet acknowledged.
    unacked: Mutex<HashMap<u32, UnackedPacket>>,
    connected: AtomicBool,
    packet_tx: broadcast::Sender<IncomingPacket>,
}

impl Network {
    /// Open a UDP circuit to the simulator.
    ///
    /// Binds a local socket, sends `UseCircuitCode`, and waits for the server
    /// to acknowledge the sequence number before returning.
    pub async fn connect(
        sim_ip: &str,
        sim_port: u16,
        agent_id: Uuid,
        session_id: Uuid,
        circuit_code: u32,
    ) -> Result<Self, RealmWalkerError> {
        let server_addr: SocketAddr = format!("{sim_ip}:{sim_port}")
            .parse()
            .map_err(|e| RealmWalkerError::CircuitFailed(format!("bad address: {e}")))?;

        // Bind to any available local port.
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(server_addr).await?;

        let (packet_tx, _) = broadcast::channel(PACKET_CHANNEL_CAPACITY);

        let net = Self {
            socket: Arc::new(socket),
            server_addr,
            agent_id,
            session_id,
            sequence: AtomicU32::new(1),
            pending_acks: Mutex::new(Vec::new()),
            unacked: Mutex::new(HashMap::new()),
            connected: AtomicBool::new(false),
            packet_tx,
        };

        // Build and send UseCircuitCode.
        let msg = UseCircuitCode {
            circuit_code: use_circuit_code::CircuitCode {
                code: circuit_code,
                session_id,
                id: agent_id,
            },
        };

        tracing::debug!(
            %server_addr,
            circuit_code,
            "sending UseCircuitCode"
        );

        let seq = net.next_sequence();
        let mut buf = Vec::new();
        build_packet(&msg, seq, true, false, &[], &mut buf);
        net.socket.send(&buf).await?;

        // Store as unacked so the ack loop can resend if needed.
        {
            let mut map = net.unacked.lock().await;
            map.insert(seq, UnackedPacket {
                data: buf,
                sent: Instant::now(),
                retries: 0,
            });
        }

        // Wait for the server to ack our sequence number.
        let deadline = tokio::time::Instant::now() + CIRCUIT_ACK_TIMEOUT;
        let mut recv_buf = [0u8; RECV_BUF_SIZE];

        loop {
            let remaining = deadline - tokio::time::Instant::now();
            if remaining.is_zero() {
                return Err(RealmWalkerError::Timeout);
            }

            match tokio::time::timeout(remaining, net.socket.recv(&mut recv_buf)).await {
                Ok(Ok(n)) => {
                    if let Ok(pkt) = parse_packet(&recv_buf[..n]) {
                        // Check if the server acknowledged our UseCircuitCode.
                        let acked = pkt.acks.contains(&seq)
                            || is_packet_ack_for(&pkt, seq);

                        // If the packet is reliable, queue our ack.
                        if pkt.flags & PacketFlags::RELIABLE != 0 {
                            let mut pending = net.pending_acks.lock().await;
                            pending.push(pkt.sequence);
                        }

                        if acked {
                            {
                                let mut map = net.unacked.lock().await;
                                map.remove(&seq);
                            }
                            net.connected.store(true, Ordering::Release);
                            tracing::debug!(
                                %server_addr,
                                "circuit established"
                            );
                            return Ok(net);
                        }
                    }
                }
                Ok(Err(e)) => {
                    return Err(RealmWalkerError::IoError(e));
                }
                Err(_) => {
                    return Err(RealmWalkerError::Timeout);
                }
            }
        }
    }

    /// Send a typed message over the circuit.
    ///
    /// If `reliable` is true, the packet is stored for retransmission until
    /// the server acknowledges it.
    pub async fn send_msg<M: Msg>(
        &self,
        msg: &M,
        reliable: bool,
    ) -> Result<(), RealmWalkerError> {
        let seq = self.next_sequence();
        let pending_acks = {
            let mut pa = self.pending_acks.lock().await;
            std::mem::take(&mut *pa)
        };

        let mut buf = Vec::new();
        build_packet(msg, seq, reliable, false, &pending_acks, &mut buf);

        self.socket.send(&buf).await?;

        if reliable {
            let mut map = self.unacked.lock().await;
            map.insert(seq, UnackedPacket {
                data: buf,
                sent: Instant::now(),
                retries: 0,
            });
        }

        Ok(())
    }

    /// Send a pre-built raw datagram.
    pub async fn send_raw(&self, data: &[u8]) -> Result<(), RealmWalkerError> {
        self.socket.send(data).await?;
        Ok(())
    }

    /// Subscribe to the incoming packet broadcast stream.
    ///
    /// Each subscriber independently receives all incoming packets. Slow
    /// receivers will receive a `Lagged` error and skip to the latest packet.
    pub fn subscribe(&self) -> broadcast::Receiver<IncomingPacket> {
        self.packet_tx.subscribe()
    }

    /// The main receive loop. Reads datagrams from the socket, parses them,
    /// processes acks and pings, and broadcasts them to subscribers.
    ///
    /// This should be spawned as a long-running task. It exits when the
    /// circuit is disconnected or an unrecoverable I/O error occurs.
    pub async fn run_packet_pump(&self) -> Result<(), RealmWalkerError> {
        let mut recv_buf = [0u8; RECV_BUF_SIZE];

        while self.connected.load(Ordering::Acquire) {
            let n = match tokio::time::timeout(
                Duration::from_secs(30),
                self.socket.recv(&mut recv_buf),
            )
            .await
            {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    if self.connected.load(Ordering::Acquire) {
                        tracing::warn!(error = %e, "socket recv error");
                    }
                    break;
                }
                Err(_) => {
                    // 30-second receive timeout with no data -- still connected,
                    // the ack loop handles pings. Just loop.
                    continue;
                }
            };

            let pkt = match parse_packet(&recv_buf[..n]) {
                Ok(pkt) => pkt,
                Err(e) => {
                    tracing::debug!(error = %e, len = n, "dropping malformed packet");
                    continue;
                }
            };

            // Process ack information embedded in this packet.
            self.handle_acks(&pkt).await;

            // If this packet is reliable, we must ack its sequence number.
            if pkt.flags & PacketFlags::RELIABLE != 0 {
                let mut pending = self.pending_acks.lock().await;
                pending.push(pkt.sequence);
            }

            // Handle pings inline so latency stays low.
            if pkt.id == StartPingCheck::ID {
                if let Ok(ping) = StartPingCheck::decode(&pkt.body) {
                    let reply = CompletePingCheck {
                        ping_id: complete_ping_check::PingID {
                            ping_id: ping.ping_id.ping_id,
                        },
                    };
                    if let Err(e) = self.send_msg(&reply, false).await {
                        tracing::debug!(error = %e, "failed to send ping reply");
                    }
                }
            }

            // Broadcast to subscribers. If nobody is listening, just drop it.
            let _ = self.packet_tx.send(pkt);
        }

        tracing::debug!("packet pump stopped");
        Ok(())
    }

    /// Periodic maintenance loop: flush pending acks, resend unacknowledged
    /// reliable packets, and send keep-alive pings.
    ///
    /// Should be spawned as a long-running task alongside [`run_packet_pump`].
    pub async fn run_ack_loop(&self) -> Result<(), RealmWalkerError> {
        let mut ping_id: u8 = 0;
        let mut last_ping = Instant::now();

        while self.connected.load(Ordering::Acquire) {
            tokio::time::sleep(ACK_LOOP_INTERVAL).await;

            // Flush pending acks as a PacketAck message.
            let acks_to_send = {
                let mut pending = self.pending_acks.lock().await;
                if pending.is_empty() {
                    Vec::new()
                } else {
                    std::mem::take(&mut *pending)
                }
            };

            if !acks_to_send.is_empty() {
                let msg = PacketAck {
                    packets: acks_to_send
                        .iter()
                        .map(|&id| packet_ack::Packets { id })
                        .collect(),
                };
                if let Err(e) = self.send_msg(&msg, false).await {
                    tracing::debug!(error = %e, "failed to send PacketAck");
                }
            }

            // Resend unacked reliable packets that have timed out.
            let now = Instant::now();
            let mut lost = Vec::new();
            {
                let mut map = self.unacked.lock().await;
                let mut to_remove = Vec::new();

                for (&seq, entry) in map.iter_mut() {
                    if now.duration_since(entry.sent) >= RESEND_INTERVAL {
                        if entry.retries >= MAX_RETRIES {
                            tracing::warn!(seq, "packet lost after {MAX_RETRIES} retries");
                            to_remove.push(seq);
                            lost.push(seq);
                        } else {
                            entry.retries += 1;
                            entry.sent = now;
                            // Rebuild with RESENT flag.
                            let mut resend = entry.data.clone();
                            if !resend.is_empty() {
                                resend[0] |= PacketFlags::RESENT;
                            }
                            tracing::debug!(seq, retry = entry.retries, "resending packet");
                            if let Err(e) = self.socket.send(&resend).await {
                                tracing::debug!(seq, error = %e, "resend failed");
                            }
                        }
                    }
                }

                for seq in to_remove {
                    map.remove(&seq);
                }
            }

            // Send keep-alive ping if enough time has passed.
            if now.duration_since(last_ping) >= PING_INTERVAL {
                let oldest_unacked = {
                    let map = self.unacked.lock().await;
                    map.keys().copied().min().unwrap_or(0)
                };

                let msg = StartPingCheck {
                    ping_id: start_ping_check::PingID {
                        ping_id,
                        oldest_unacked,
                    },
                };
                if let Err(e) = self.send_msg(&msg, false).await {
                    tracing::debug!(error = %e, "failed to send ping");
                }
                ping_id = ping_id.wrapping_add(1);
                last_ping = now;
            }
        }

        tracing::debug!("ack loop stopped");
        Ok(())
    }

    /// Gracefully shut down the circuit. Sets the connected flag to false,
    /// which causes both the packet pump and ack loop to exit.
    pub async fn disconnect(&self) {
        if self
            .connected
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            // Send a LogoutRequest as a courtesy. The server may not receive
            // it, but we try.
            let msg = LogoutRequest {
                agent_data: logout_request::AgentData {
                    agent_id: self.agent_id,
                    session_id: self.session_id,
                },
            };
            let _ = self.send_msg(&msg, true).await;
            tracing::debug!("disconnect: logout sent");
        }
    }

    /// Whether the circuit is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    /// The remote simulator address.
    pub fn server_addr(&self) -> SocketAddr {
        self.server_addr
    }

    /// Process acknowledgement information from an incoming packet.
    ///
    /// Three sources of acks:
    /// 1. Acks appended to the packet (when ACK flag is set) -- these are the
    ///    server acknowledging OUR reliable packets.
    /// 2. If the packet is a PacketAck message, decode it and remove those
    ///    sequence numbers from our unacked map.
    /// 3. (Acks we owe the server are handled by the caller adding to
    ///    pending_acks when the packet has the RELIABLE flag.)
    async fn handle_acks(&self, pkt: &IncomingPacket) {
        let mut map = self.unacked.lock().await;

        // 1. Appended acks from the packet header.
        for &acked_seq in &pkt.acks {
            if map.remove(&acked_seq).is_some() {
                tracing::debug!(seq = acked_seq, "ack received (appended)");
            }
        }

        // 2. PacketAck message body.
        if pkt.id == PacketAck::ID {
            if let Ok(ack_msg) = PacketAck::decode(&pkt.body) {
                for entry in &ack_msg.packets {
                    if map.remove(&entry.id).is_some() {
                        tracing::debug!(seq = entry.id, "ack received (PacketAck)");
                    }
                }
            }
        }
    }

    /// Allocate the next outgoing sequence number.
    fn next_sequence(&self) -> u32 {
        self.sequence.fetch_add(1, Ordering::Relaxed)
    }
}

/// Check whether an `IncomingPacket` is a `PacketAck` message that
/// acknowledges the given sequence number.
fn is_packet_ack_for(pkt: &IncomingPacket, seq: u32) -> bool {
    if pkt.id != PacketAck::ID {
        return false;
    }
    if let Ok(ack_msg) = PacketAck::decode(&pkt.body) {
        return ack_msg.packets.iter().any(|p| p.id == seq);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use welkin_msg::MsgId;

    #[test]
    fn next_sequence_increments() {
        let seq = AtomicU32::new(1);
        assert_eq!(seq.fetch_add(1, Ordering::Relaxed), 1);
        assert_eq!(seq.fetch_add(1, Ordering::Relaxed), 2);
        assert_eq!(seq.fetch_add(1, Ordering::Relaxed), 3);
    }

    #[test]
    fn is_packet_ack_for_matches() {
        // Build a PacketAck message containing sequence 42.
        let msg = PacketAck {
            packets: vec![
                packet_ack::Packets { id: 10 },
                packet_ack::Packets { id: 42 },
                packet_ack::Packets { id: 99 },
            ],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 1, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("parse should succeed");

        assert!(is_packet_ack_for(&pkt, 42));
        assert!(is_packet_ack_for(&pkt, 10));
        assert!(is_packet_ack_for(&pkt, 99));
        assert!(!is_packet_ack_for(&pkt, 7));
    }

    #[test]
    fn is_packet_ack_for_rejects_other_messages() {
        // A StartPingCheck is not a PacketAck.
        let msg = StartPingCheck {
            ping_id: start_ping_check::PingID {
                ping_id: 0,
                oldest_unacked: 0,
            },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 1, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("parse should succeed");

        assert!(!is_packet_ack_for(&pkt, 1));
    }

    #[test]
    fn use_circuit_code_builds_correctly() {
        let agent_id = Uuid::from_u128(0x1234);
        let session_id = Uuid::from_u128(0x5678);
        let msg = UseCircuitCode {
            circuit_code: use_circuit_code::CircuitCode {
                code: 999,
                session_id,
                id: agent_id,
            },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 1, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(3));
        assert!(pkt.reliable());
        let decoded: UseCircuitCode = pkt.decode().expect("should decode");
        assert_eq!(decoded.circuit_code.code, 999);
        assert_eq!(decoded.circuit_code.id, agent_id);
        assert_eq!(decoded.circuit_code.session_id, session_id);
    }

    #[test]
    fn logout_request_builds_correctly() {
        let agent_id = Uuid::from_u128(0xaaaa);
        let session_id = Uuid::from_u128(0xbbbb);
        let msg = LogoutRequest {
            agent_data: logout_request::AgentData {
                agent_id,
                session_id,
            },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 5, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(252));
        let decoded: LogoutRequest = pkt.decode().expect("should decode");
        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.agent_data.session_id, session_id);
    }

    #[test]
    fn complete_ping_check_builds_correctly() {
        let msg = CompletePingCheck {
            ping_id: complete_ping_check::PingID { ping_id: 7 },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 100, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::High(2));
        let decoded: CompletePingCheck = pkt.decode().expect("should decode");
        assert_eq!(decoded.ping_id.ping_id, 7);
    }

    #[test]
    fn chat_from_viewer_builds_correctly() {
        let agent_id = Uuid::from_u128(1);
        let session_id = Uuid::from_u128(2);
        let msg = ChatFromViewer {
            agent_data: chat_from_viewer::AgentData {
                agent_id,
                session_id,
            },
            chat_data: chat_from_viewer::ChatData {
                message: b"Hello, world!".to_vec(),
                type_: 1,
                channel: 0,
            },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 10, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        assert_eq!(pkt.id, MsgId::Low(80));
        let decoded: ChatFromViewer = pkt.decode().expect("should decode");
        assert_eq!(decoded.chat_data.message, b"Hello, world!");
        assert_eq!(decoded.chat_data.type_, 1);
        assert_eq!(decoded.chat_data.channel, 0);
    }
}
