//! Server-side LLSD API for OpenSim.
//!
//! This crate wraps `aurora-llsd`, a viewer library, and is the only place in
//! the workspace that names it. Every other crate depends on `opensim-llsd`,
//! so a change to the Aurora API surface is absorbed here.
//!
//! It exposes the [`Llsd`] value type, its [`Map`] alias, the [`LlsdError`]
//! error type, and the three wire codecs used by capability HTTP bodies: XML
//! (`from_xml`, `to_xml`), binary (`from_binary`, `to_binary`) and notation
//! (`from_notation`, `to_notation`).

pub use aurora_llsd::{
    Llsd, LlsdError, Map, from_binary, from_binary_local, from_notation, from_xml,
    from_xml_element, to_binary, to_notation, to_pretty_xml_string, to_serialized_xml, to_xml,
    to_xml_string,
};

pub mod xmlrpc;
