// SPDX-License-Identifier: LGPL-3.0-or-later

//! Core protocol handlers registered on the packet dispatcher.
//!
//! Only handlers that do real work belong here. Messages without a
//! registered handler fall through to a trace-level "unhandled message"
//! log in the region binary's dispatch loop.

use crate::dispatch::PacketDispatcher;

/// Register all core protocol handlers on a dispatcher.
pub fn register_core_handlers(_dispatcher: &mut PacketDispatcher) {
    // All previously registered handlers were log-only stubs.
    // Real message handling lives in the region binary's inline dispatch
    // (main.rs) where it has access to scene, clients, and socket.
    //
    // Messages with no handler fall through to the "unhandled message"
    // trace log — this is the correct behavior per project rules.
}
