// SPDX-License-Identifier: GPL-3.0-or-later

//! LLUDP server — accepts viewer connections and dispatches parsed packets.

use crate::circuit::{Circuit, CircuitTick};
use crate::throttle::ClientThrottle;
use opensim_msg::{IncomingPacket, Msg, parse_packet};
use opensim_msg::msgs::PacketAck;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock, mpsc};
use uuid::Uuid;

/// Per-viewer session state held by the server.
pub struct ClientSession {
    pub circuit: Circuit,
    pub throttle: ClientThrottle,
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub circuit_code: u32,
    pub addr: SocketAddr,
    pub last_activity: Instant,
}

impl ClientSession {
    /// Create a new session from a UseCircuitCode handshake.
    pub fn new(addr: SocketAddr, code: u32, agent_id: Uuid, session_id: Uuid) -> Self {
        Self {
            circuit: Circuit::new(addr),
            throttle: ClientThrottle::new(),
            agent_id,
            session_id,
            circuit_code: code,
            addr,
            last_activity: Instant::now(),
        }
    }

    /// Run a circuit tick, returning packets to send and dropped sequences.
    pub fn tick(&mut self) -> CircuitTick {
        self.circuit.tick()
    }

    /// Record receive-side bookkeeping and dedup. Returns `true` when the
    /// packet is new (not a duplicate).
    pub fn on_receive(&mut self, pkt: &IncomingPacket) -> bool {
        self.last_activity = Instant::now();
        let dominated = !self.circuit.on_receive(pkt.flags, pkt.sequence, &pkt.acks);
        if dominated {
            return false;
        }
        self.circuit.accept(pkt.flags, pkt.sequence);
        true
    }
}

/// Async LLUDP server that accepts viewer connections over a single UDP socket.
pub struct LLUDPServer {
    socket: Arc<UdpSocket>,
    clients: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>>,
}

impl LLUDPServer {
    /// Bind a new server to the given address.
    pub async fn bind(addr: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        tracing::info!("LLUDP server listening on {}", socket.local_addr()?);
        Ok(Self {
            socket: Arc::new(socket),
            clients: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Wrap an already-bound socket (useful for tests).
    pub fn from_socket(socket: UdpSocket) -> Self {
        Self {
            socket: Arc::new(socket),
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// A handle to the shared socket (e.g. for sending replies from outside).
    pub fn socket(&self) -> &Arc<UdpSocket> {
        &self.socket
    }

    /// A handle to the client map.
    pub fn clients(&self) -> &Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>> {
        &self.clients
    }

    /// Look up a client session by address.
    pub async fn get_client(&self, addr: &SocketAddr) -> Option<Arc<Mutex<ClientSession>>> {
        self.clients.read().await.get(addr).cloned()
    }

    /// Remove a client session.
    pub async fn remove_client(&self, addr: &SocketAddr) {
        if self.clients.write().await.remove(addr).is_some() {
            tracing::info!("removed client {}", addr);
        }
    }

    /// Send raw bytes to a specific address.
    pub async fn send_to(&self, addr: SocketAddr, data: &[u8]) -> std::io::Result<usize> {
        self.socket.send_to(data, addr).await
    }

    /// The main receive loop.
    ///
    /// Reads datagrams, parses them, handles UseCircuitCode to create sessions,
    /// and forwards all other packets through `msg_tx`.
    ///
    /// Runs until the `cancel` token is cancelled.
    pub async fn run(
        &self,
        msg_tx: mpsc::UnboundedSender<(SocketAddr, IncomingPacket)>,
        cancel: tokio_util::sync::CancellationToken,
    ) {
        let recv_task = {
            let socket = self.socket.clone();
            let clients = self.clients.clone();
            let msg_tx = msg_tx.clone();
            let cancel = cancel.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        result = socket.recv_from(&mut buf) => {
                            let (len, src) = match result {
                                Ok(v) => v,
                                Err(e) => {
                                    tracing::warn!("recv error: {}", e);
                                    continue;
                                }
                            };
                            let data = &buf[..len];
                            let pkt = match parse_packet(data) {
                                Ok(p) => p,
                                Err(e) => {
                                    tracing::debug!("parse error from {}: {}", src, e);
                                    continue;
                                }
                            };
                            Self::handle_packet(&clients, &msg_tx, src, pkt).await;
                        }
                    }
                }
            })
        };

        let tick_task = {
            let socket = self.socket.clone();
            let clients = self.clients.clone();
            let cancel = cancel.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        _ = interval.tick() => {
                            Self::tick_all(&clients, &socket).await;
                        }
                    }
                }
            })
        };

        // Wait for either task to finish (both stop on cancel).
        let _ = tokio::try_join!(recv_task, tick_task);
    }

    /// Process a single parsed packet.
    async fn handle_packet(
        clients: &Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>>,
        msg_tx: &mpsc::UnboundedSender<(SocketAddr, IncomingPacket)>,
        src: SocketAddr,
        pkt: IncomingPacket,
    ) {
        // UseCircuitCode (Low 3) — create a new session.
        if pkt.id == opensim_msg::msgs::UseCircuitCode::ID {
            match pkt.decode::<opensim_msg::msgs::UseCircuitCode>() {
                Ok(ucc) => {
                    let code = ucc.circuit_code.code;
                    let agent_id = ucc.circuit_code.id;
                    let session_id = ucc.circuit_code.session_id;
                    tracing::info!(
                        "UseCircuitCode from {} code={} agent={}",
                        src,
                        code,
                        agent_id
                    );
                    let session = ClientSession::new(src, code, agent_id, session_id);
                    let session = Arc::new(Mutex::new(session));
                    {
                        // Ack the packet inside the new session.
                        let mut s = session.lock().await;
                        s.on_receive(&pkt);
                    }
                    clients.write().await.insert(src, session);
                }
                Err(e) => {
                    tracing::warn!("bad UseCircuitCode from {}: {}", src, e);
                }
            }
            // Always forward so the application layer can respond (e.g.
            // send RegionHandshake).
            let _ = msg_tx.send((src, pkt));
            return;
        }

        // All other messages — look up existing session.
        let client = { clients.read().await.get(&src).cloned() };
        match client {
            Some(session) => {
                let is_new = {
                    let mut s = session.lock().await;
                    let is_new = s.on_receive(&pkt);
                    if pkt.id == PacketAck::ID {
                        if let Ok(ack_msg) = pkt.decode::<PacketAck>() {
                            let ack_ids: Vec<u32> = ack_msg.packets.iter().map(|p| p.id).collect();
                            s.circuit.on_receive(0, 0, &ack_ids);
                        }
                    }
                    is_new
                };
                if is_new {
                    let _ = msg_tx.send((src, pkt));
                } else {
                    tracing::debug!("duplicate seq {} from {}", pkt.sequence, src);
                }
            }
            None => {
                tracing::debug!("packet from unknown client {}, ignoring", src);
            }
        }
    }

    /// Tick all client circuits — sends resends and ack packets.
    async fn tick_all(
        clients: &Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<ClientSession>>>>>,
        socket: &Arc<UdpSocket>,
    ) {
        let snapshot: Vec<(SocketAddr, Arc<Mutex<ClientSession>>)> = {
            clients
                .read()
                .await
                .iter()
                .map(|(a, s)| (*a, s.clone()))
                .collect()
        };

        for (addr, session) in snapshot {
            let tick_result = {
                let mut s = session.lock().await;
                s.tick()
            };

            for pkt in &tick_result.packets {
                if let Err(e) = socket.send_to(pkt, addr).await {
                    tracing::warn!("send error to {}: {}", addr, e);
                }
            }

            for seq in &tick_result.dropped {
                tracing::debug!("dropped reliable seq {} for {}", seq, addr);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensim_msg::{Msg, MsgId, PacketFlags, build_packet};
    use std::net::SocketAddr;
    use uuid::Uuid;

    fn localhost() -> SocketAddr {
        "127.0.0.1:9000".parse().unwrap()
    }

    #[test]
    fn client_session_creation() {
        let agent = Uuid::from_u128(1);
        let session = Uuid::from_u128(2);
        let s = ClientSession::new(localhost(), 42, agent, session);
        assert_eq!(s.circuit_code, 42);
        assert_eq!(s.agent_id, agent);
        assert_eq!(s.session_id, session);
        assert_eq!(s.addr, localhost());
    }

    #[tokio::test]
    async fn server_bind_and_socket() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = LLUDPServer::bind(addr).await.unwrap();
        assert_ne!(server.socket().local_addr().unwrap().port(), 0);
    }

    #[tokio::test]
    async fn server_from_socket() {
        let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let local = sock.local_addr().unwrap();
        let server = LLUDPServer::from_socket(sock);
        assert_eq!(server.socket().local_addr().unwrap(), local);
    }

    #[tokio::test]
    async fn use_circuit_code_creates_session() {
        let server_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_sock.local_addr().unwrap();
        let server = LLUDPServer::from_socket(server_sock);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let cancel = tokio_util::sync::CancellationToken::new();

        // Spawn the server loop.
        let cancel2 = cancel.clone();
        let handle = tokio::spawn({
            let server_ref = LLUDPServer {
                socket: server.socket.clone(),
                clients: server.clients.clone(),
            };
            async move { server_ref.run(tx, cancel2).await }
        });

        // Send a UseCircuitCode from a "viewer" socket.
        let viewer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let viewer_addr = viewer.local_addr().unwrap();
        let mut ucc = opensim_msg::msgs::UseCircuitCode::default();
        ucc.circuit_code.code = 1234;
        ucc.circuit_code.id = Uuid::from_u128(10);
        ucc.circuit_code.session_id = Uuid::from_u128(20);
        let mut buf = Vec::new();
        build_packet(&ucc, 1, true, false, &[], &mut buf);
        viewer.send_to(&buf, server_addr).await.unwrap();

        // Wait for the message to arrive.
        let (src, pkt) = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        assert_eq!(src, viewer_addr);
        assert_eq!(pkt.id, opensim_msg::msgs::UseCircuitCode::ID);

        // Verify the session was created.
        let client = server.get_client(&viewer_addr).await;
        assert!(client.is_some());
        let binding = client.unwrap();
        let s = binding.lock().await;
        assert_eq!(s.circuit_code, 1234);
        assert_eq!(s.agent_id, Uuid::from_u128(10));

        cancel.cancel();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn unknown_client_ignored() {
        let server_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_sock.local_addr().unwrap();
        let server = LLUDPServer::from_socket(server_sock);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let cancel = tokio_util::sync::CancellationToken::new();

        let cancel2 = cancel.clone();
        let handle = tokio::spawn({
            let server_ref = LLUDPServer {
                socket: server.socket.clone(),
                clients: server.clients.clone(),
            };
            async move { server_ref.run(tx, cancel2).await }
        });

        // Send a random packet (not UseCircuitCode) from an unknown address.
        let viewer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let ping = opensim_msg::msgs::CompletePingCheck {
            ping_id: opensim_msg::msgs::complete_ping_check::PingID { ping_id: 1 },
        };
        let mut buf = Vec::new();
        build_packet(&ping, 1, false, false, &[], &mut buf);
        viewer.send_to(&buf, server_addr).await.unwrap();

        // Should NOT appear on the channel (no session).
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await;
        assert!(result.is_err(), "should have timed out — unknown client");

        cancel.cancel();
        let _ = handle.await;
    }

    #[test]
    fn session_dedup() {
        let mut session =
            ClientSession::new(localhost(), 1, Uuid::nil(), Uuid::nil());
        let pkt = IncomingPacket {
            flags: PacketFlags::RELIABLE,
            sequence: 42,
            id: MsgId::Low(999),
            body: vec![],
            acks: vec![],
            expanded_len: 10,
        };
        assert!(session.on_receive(&pkt));
        // Second receive with RESENT flag should be rejected as duplicate.
        let dup = IncomingPacket {
            flags: PacketFlags::RELIABLE | PacketFlags::RESENT,
            sequence: 42,
            id: MsgId::Low(999),
            body: vec![],
            acks: vec![],
            expanded_len: 10,
        };
        assert!(!session.on_receive(&dup));
    }

    #[test]
    fn tick_produces_ack_packets() {
        // Create a session and accept a reliable packet so it has pending acks.
        let addr: SocketAddr = "127.0.0.1:55555".parse().unwrap();
        let mut session = ClientSession::new(addr, 1, Uuid::nil(), Uuid::nil());
        let pkt = IncomingPacket {
            flags: PacketFlags::RELIABLE,
            sequence: 100,
            id: MsgId::Low(999),
            body: vec![],
            acks: vec![],
            expanded_len: 10,
        };
        session.on_receive(&pkt);

        // Flush acks explicitly — verifies that accepting a reliable packet
        // queues ack generation.
        let ack_pkts = session.circuit.flush_acks();
        assert!(
            !ack_pkts.is_empty(),
            "flush_acks should produce packets for pending acks"
        );
    }

    #[tokio::test]
    async fn remove_client_works() {
        let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server = LLUDPServer::from_socket(sock);
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let session = ClientSession::new(addr, 1, Uuid::nil(), Uuid::nil());
        server
            .clients
            .write()
            .await
            .insert(addr, Arc::new(Mutex::new(session)));
        assert!(server.get_client(&addr).await.is_some());
        server.remove_client(&addr).await;
        assert!(server.get_client(&addr).await.is_none());
    }
}
