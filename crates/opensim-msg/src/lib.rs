//! Server-side message codec facade over `aurora-msg`.
//!
//! Re-exports the packet framing, zerocoding, field reader/writer and typed
//! template messages so the rest of the workspace depends on this crate alone.
//! `opensim_msg::msgs` holds every generated message struct.

pub use aurora_msg::msgs;
pub use aurora_msg::packet::{HEADER_SIZE, MTU, build_raw};
pub use aurora_msg::{
    DecodeError, IncomingPacket, Msg, MsgId, MsgInfo, PacketFlags, Reader, Writer, build_packet,
    field_str, message_info, parse_packet, str_field, unpack_quat,
};

pub use opensim_types;
