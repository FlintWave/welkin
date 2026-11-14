// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed packet handler dispatch: routes [`IncomingPacket`]s to registered
//! handler closures that receive a fully decoded message struct.

use opensim_msg::{DecodeError, IncomingPacket, Msg, MsgId};
use std::collections::HashMap;
use std::net::SocketAddr;
use uuid::Uuid;

/// Context passed to every packet handler.
pub struct PacketContext {
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub circuit_code: u32,
    pub addr: SocketAddr,
}

/// A handler function that processes a raw message body.
///
/// The closure stored in the dispatch table decodes the body and forwards
/// the typed message to the user-supplied handler.
pub type PacketHandler =
    Box<dyn Fn(&PacketContext, &[u8]) -> Result<(), HandleError> + Send + Sync>;

/// Errors returned by the dispatch pipeline.
#[derive(Debug, thiserror::Error)]
pub enum HandleError {
    #[error("decode error: {0}")]
    Decode(#[from] DecodeError),
    #[error("handler error: {0}")]
    Handler(String),
    #[error("no handler for message")]
    Unhandled,
}

/// A table mapping [`MsgId`] values to typed handler closures.
pub struct PacketDispatcher {
    handlers: HashMap<MsgId, PacketHandler>,
}

impl PacketDispatcher {
    /// Create an empty dispatcher with no registered handlers.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for message type `M`.
    ///
    /// When a packet with `M::ID` arrives, the dispatcher decodes the body
    /// into `M` and calls `handler` with the result.  Registering a second
    /// handler for the same message type replaces the previous one.
    pub fn register<M, F>(&mut self, handler: F)
    where
        M: Msg + 'static,
        F: Fn(&PacketContext, M) -> Result<(), HandleError> + Send + Sync + 'static,
    {
        let boxed: PacketHandler = Box::new(move |ctx, body| {
            let msg = M::decode(body)?;
            handler(ctx, msg)
        });
        self.handlers.insert(M::ID, boxed);
    }

    /// Dispatch an incoming packet to its registered handler.
    ///
    /// Returns [`HandleError::Unhandled`] when no handler is registered for
    /// the packet's message id.
    pub fn dispatch(
        &self,
        ctx: &PacketContext,
        packet: &IncomingPacket,
    ) -> Result<(), HandleError> {
        match self.handlers.get(&packet.id) {
            Some(handler) => handler(ctx, &packet.body),
            None => Err(HandleError::Unhandled),
        }
    }

    /// Check whether a handler is registered for `msg_id`.
    pub fn has_handler(&self, msg_id: &MsgId) -> bool {
        self.handlers.contains_key(msg_id)
    }

    /// Number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for PacketDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensim_msg::msgs::*;
    use opensim_msg::{str_field, Writer};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn test_ctx() -> PacketContext {
        PacketContext {
            agent_id: Uuid::from_u128(1),
            session_id: Uuid::from_u128(2),
            circuit_code: 42,
            addr: "127.0.0.1:9000".parse().unwrap(),
        }
    }

    fn make_packet<M: Msg>(msg: &M) -> IncomingPacket {
        let mut w = Writer::new();
        msg.encode_body(&mut w);
        IncomingPacket {
            flags: 0,
            sequence: 1,
            id: M::ID,
            body: w.buf,
            acks: vec![],
            expanded_len: 0,
        }
    }

    #[test]
    fn register_and_dispatch() {
        let called = Arc::new(AtomicBool::new(false));
        let called2 = called.clone();

        let mut d = PacketDispatcher::new();
        d.register::<ChatFromViewer, _>(move |_ctx, msg| {
            assert_eq!(msg.chat_data.channel, 42);
            called2.store(true, Ordering::SeqCst);
            Ok(())
        });

        let mut msg = ChatFromViewer::default();
        msg.agent_data.agent_id = Uuid::from_u128(1);
        msg.chat_data.message = str_field("test");
        msg.chat_data.channel = 42;
        msg.chat_data.type_ = 1;
        let pkt = make_packet(&msg);

        let ctx = test_ctx();
        d.dispatch(&ctx, &pkt).unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn dispatch_unhandled() {
        let d = PacketDispatcher::new();
        let msg = ChatFromViewer::default();
        let pkt = make_packet(&msg);
        let ctx = test_ctx();
        let err = d.dispatch(&ctx, &pkt).unwrap_err();
        assert!(matches!(err, HandleError::Unhandled));
    }

    #[test]
    fn handler_count() {
        let mut d = PacketDispatcher::new();
        assert_eq!(d.handler_count(), 0);
        d.register::<ChatFromViewer, _>(|_, _| Ok(()));
        assert_eq!(d.handler_count(), 1);
        d.register::<AgentUpdate, _>(|_, _| Ok(()));
        assert_eq!(d.handler_count(), 2);
    }

    #[test]
    fn register_core_handlers_populates_all() {
        let mut d = PacketDispatcher::new();
        crate::handlers::register_core_handlers(&mut d);

        // 2.4 core: 4 handlers
        assert!(d.has_handler(&UseCircuitCode::ID));
        assert!(d.has_handler(&CompleteAgentMovement::ID));
        assert!(d.has_handler(&AgentUpdate::ID));
        assert!(d.has_handler(&ChatFromViewer::ID));

        // 2.5 object: 6 handlers
        assert!(d.has_handler(&ObjectAdd::ID));
        assert!(d.has_handler(&ObjectDelete::ID));
        assert!(d.has_handler(&ObjectSelect::ID));
        assert!(d.has_handler(&ObjectDeselect::ID));
        assert!(d.has_handler(&ObjectGrab::ID));
        assert!(d.has_handler(&MultipleObjectUpdate::ID));

        // 2.6 avatar: 3 handlers
        assert!(d.has_handler(&AvatarAnimation::ID));
        assert!(d.has_handler(&AgentSetAppearance::ID));
        assert!(d.has_handler(&ViewerEffect::ID));

        assert_eq!(d.handler_count(), 13);
    }

    #[test]
    fn packet_context_construction() {
        let ctx = PacketContext {
            agent_id: Uuid::nil(),
            session_id: Uuid::from_u128(0xdead_beef),
            circuit_code: 99,
            addr: "[::1]:8080".parse().unwrap(),
        };
        assert_eq!(ctx.circuit_code, 99);
        assert_eq!(ctx.agent_id, Uuid::nil());
        assert_eq!(ctx.addr.port(), 8080);
    }

    #[test]
    fn dispatch_with_real_bytes() {
        let mut d = PacketDispatcher::new();
        let received_channel = Arc::new(std::sync::atomic::AtomicI32::new(-1));
        let rc = received_channel.clone();

        d.register::<ChatFromViewer, _>(move |_ctx, msg| {
            rc.store(msg.chat_data.channel, Ordering::SeqCst);
            Ok(())
        });

        // Build a ChatFromViewer with Writer, then dispatch the raw bytes.
        let mut msg = ChatFromViewer::default();
        msg.agent_data.agent_id = Uuid::from_u128(0xA);
        msg.agent_data.session_id = Uuid::from_u128(0xB);
        msg.chat_data.message = str_field("Hello world");
        msg.chat_data.type_ = 1;
        msg.chat_data.channel = 7;

        let mut w = Writer::new();
        msg.encode_body(&mut w);

        let pkt = IncomingPacket {
            flags: 0,
            sequence: 100,
            id: ChatFromViewer::ID,
            body: w.buf,
            acks: vec![],
            expanded_len: 0,
        };

        let ctx = test_ctx();
        d.dispatch(&ctx, &pkt).unwrap();
        assert_eq!(received_channel.load(Ordering::SeqCst), 7);
    }
}
