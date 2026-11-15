// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-client connection wrapper combining [`Circuit`] + [`ClientThrottle`].
//!
//! An `LLUDPClient` wraps a [`ClientSession`] and the shared UDP socket,
//! providing a high-level send API that handles sequence numbering, reliable
//! tracking, throttling and tick maintenance.

use crate::circuit::CircuitTick;
use crate::server::ClientSession;
use crate::throttle::ThrottleCategory;
use welkin_msg::Msg;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

/// Default timeout before a client is considered disconnected.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

/// A per-client connection handle used by the application layer to send
/// messages and run periodic maintenance on a single viewer session.
pub struct LLUDPClient {
    pub session: Arc<Mutex<ClientSession>>,
    socket: Arc<UdpSocket>,
}

impl LLUDPClient {
    /// Create a new client handle from a shared session and socket.
    pub fn new(session: Arc<Mutex<ClientSession>>, socket: Arc<UdpSocket>) -> Self {
        Self { session, socket }
    }

    /// The remote address of this client.
    pub async fn addr(&self) -> SocketAddr {
        self.session.lock().await.addr
    }

    /// Encode and send a typed message through the circuit.
    ///
    /// The packet is assigned a sequence number, optionally marked reliable,
    /// and appended acks are piggybacked when space allows.
    pub async fn send_msg<M: Msg>(&self, msg: &M, reliable: bool) -> std::io::Result<()> {
        let (data, _seq) = {
            let mut s = self.session.lock().await;
            s.circuit.send_msg(msg, reliable)
        };
        let addr = self.session.lock().await.addr;
        self.socket.send_to(&data, addr).await?;
        Ok(())
    }

    /// Send a pre-encoded payload (message number + body already written).
    pub async fn send_raw(
        &self,
        payload: &[u8],
        zerocoded: bool,
        reliable: bool,
    ) -> std::io::Result<()> {
        let (data, _seq) = {
            let mut s = self.session.lock().await;
            s.circuit.send_raw(payload, zerocoded, reliable)
        };
        let addr = self.session.lock().await.addr;
        self.socket.send_to(&data, addr).await?;
        Ok(())
    }

    /// Run one circuit tick — sends resends and queued ack packets.
    ///
    /// Returns the [`CircuitTick`] so the caller can inspect dropped sequences.
    pub async fn tick(&self) -> std::io::Result<CircuitTick> {
        let (tick_result, addr) = {
            let mut s = self.session.lock().await;
            let t = s.tick();
            (t, s.addr)
        };

        for pkt in &tick_result.packets {
            self.socket.send_to(pkt, addr).await?;
        }

        for seq in &tick_result.dropped {
            tracing::debug!("dropped reliable seq {} for {}", seq, addr);
        }

        Ok(tick_result)
    }

    /// Whether the client is still considered connected (has sent data
    /// within the timeout window).
    pub async fn is_connected(&self) -> bool {
        let s = self.session.lock().await;
        !s.circuit.is_timed_out(CLIENT_TIMEOUT)
    }

    /// Mark the session as disconnected by forcing its last-recv timestamp
    /// into the past so the next `is_connected` check returns false.
    pub async fn disconnect(&self) {
        let mut s = self.session.lock().await;
        let past = std::time::Instant::now() - Duration::from_secs(3600);
        s.last_activity = past;
        s.circuit.last_recv = past;
        tracing::info!("client {} disconnected", s.addr);
    }

    /// Pass a viewer-supplied throttle byte block (7 × f32-LE, bits/sec per
    /// category) to update the per-client token buckets.
    pub async fn update_throttle(&self, data: &[u8]) {
        let mut s = self.session.lock().await;
        s.throttle.set_from_viewer(data);
        tracing::debug!("throttle updated for {}", s.addr);
    }

    /// Try to consume throttle tokens for the given category and byte count.
    /// Returns `true` when the send is allowed.
    pub async fn try_throttle(&self, category: ThrottleCategory, bytes: usize) -> bool {
        let mut s = self.session.lock().await;
        s.throttle.try_send(category, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::ClientSession;
    use welkin_msg::msgs;
    use uuid::Uuid;

    async fn make_pair() -> (LLUDPClient, UdpSocket) {
        let server_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let viewer_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let viewer_addr = viewer_sock.local_addr().unwrap();

        let session = ClientSession::new(
            viewer_addr,
            42,
            Uuid::from_u128(1),
            Uuid::from_u128(2),
        );
        let client = LLUDPClient::new(
            Arc::new(Mutex::new(session)),
            Arc::new(server_sock),
        );
        (client, viewer_sock)
    }

    #[tokio::test]
    async fn client_creation() {
        let (client, _viewer) = make_pair().await;
        assert!(client.is_connected().await);
    }

    #[tokio::test]
    async fn send_msg_delivers_bytes() {
        let (client, viewer) = make_pair().await;
        let ping = msgs::CompletePingCheck {
            ping_id: msgs::complete_ping_check::PingID { ping_id: 7 },
        };
        client.send_msg(&ping, false).await.unwrap();

        let mut buf = [0u8; 2048];
        let (len, _src) =
            tokio::time::timeout(Duration::from_secs(1), viewer.recv_from(&mut buf))
                .await
                .expect("timeout")
                .expect("recv failed");
        assert!(len > 0);

        // Parse what arrived and verify the message type.
        let pkt = welkin_msg::parse_packet(&buf[..len]).unwrap();
        assert_eq!(pkt.id, msgs::CompletePingCheck::ID);
    }

    #[tokio::test]
    async fn send_raw_delivers_bytes() {
        let (client, viewer) = make_pair().await;

        // Manually encode a CompletePingCheck.
        let mut w = welkin_msg::Writer::new();
        msgs::CompletePingCheck::ID.encode(&mut w.buf);
        let msg = msgs::CompletePingCheck {
            ping_id: msgs::complete_ping_check::PingID { ping_id: 3 },
        };
        msg.encode_body(&mut w);

        client.send_raw(&w.buf, false, true).await.unwrap();

        let mut buf = [0u8; 2048];
        let (len, _) =
            tokio::time::timeout(Duration::from_secs(1), viewer.recv_from(&mut buf))
                .await
                .unwrap()
                .unwrap();
        let pkt = welkin_msg::parse_packet(&buf[..len]).unwrap();
        assert_eq!(pkt.id, msgs::CompletePingCheck::ID);
        assert!(pkt.reliable());
    }

    #[tokio::test]
    async fn tick_runs_without_error() {
        let (client, _viewer) = make_pair().await;
        let result = client.tick().await;
        assert!(result.is_ok());
        let tick = result.unwrap();
        assert!(tick.packets.is_empty());
        assert!(tick.dropped.is_empty());
    }

    #[tokio::test]
    async fn disconnect_marks_not_connected() {
        let (client, _viewer) = make_pair().await;
        assert!(client.is_connected().await);
        client.disconnect().await;
        assert!(!client.is_connected().await);
    }

    #[tokio::test]
    async fn update_throttle_applies() {
        let (client, _viewer) = make_pair().await;
        // 7 categories × 4 bytes each = 28 bytes of f32-LE bits/sec.
        let mut data = Vec::with_capacity(28);
        for _ in 0..7 {
            data.extend_from_slice(&200_000.0f32.to_le_bytes());
        }
        client.update_throttle(&data).await;

        // After updating, the throttle should allow a small send.
        assert!(
            client
                .try_throttle(ThrottleCategory::Task, 100)
                .await
        );
    }
}
