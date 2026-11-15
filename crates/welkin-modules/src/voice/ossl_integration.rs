// SPDX-License-Identifier: GPL-3.0-or-later

//! OSSL integration: wire `osSetParcelSIPAddress` to the voice module.
//!
//! When a script calls `osSetParcelSIPAddress`, the engine looks up the
//! [`super::IVoiceModule`] in the scene's module registry and delegates to
//! it.  This module provides the glue functions and validation.

use uuid::Uuid;

/// Result of an OSSL voice operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsslVoiceResult {
    /// Operation succeeded.
    Success,
    /// Voice module is not available/enabled.
    ModuleNotAvailable,
    /// Invalid arguments provided.
    InvalidArguments(String),
}

/// Handle the `osSetParcelSIPAddress` script function.
///
/// In a full implementation, this looks up the `IVoiceModule` in the scene's
/// module registry.  Here we operate on the module directly.
pub fn os_set_parcel_sip_address(
    module: &mut super::WebRtcVoiceModule,
    parcel_global_id: Uuid,
    sip_address: &str,
) -> OsslVoiceResult {
    if !validate_sip_address(sip_address) {
        return OsslVoiceResult::InvalidArguments(format!("invalid SIP address: '{sip_address}'"));
    }

    tracing::debug!(
        parcel_id = %parcel_global_id,
        sip_address,
        "osSetParcelSIPAddress: setting SIP override"
    );

    use super::IVoiceModule;
    module.set_land_sip_address(parcel_global_id, sip_address);
    OsslVoiceResult::Success
}

/// Validate a SIP address for basic format correctness.
///
/// Must be non-empty and contain no control characters.
pub fn validate_sip_address(address: &str) -> bool {
    if address.is_empty() {
        return false;
    }
    // Reject any control characters (U+0000..U+001F, U+007F).
    if address.chars().any(|c| c.is_control()) {
        return false;
    }
    true
}

/// Clear any SIP address override for a parcel, reverting to the
/// auto-generated channel URI.
pub fn os_clear_parcel_sip_address(
    module: &mut super::WebRtcVoiceModule,
    parcel_global_id: Uuid,
) -> OsslVoiceResult {
    tracing::debug!(
        parcel_id = %parcel_global_id,
        "osSetParcelSIPAddress: clearing SIP override"
    );

    // Whether or not the override existed, the end state is the same.
    let _ = module.remove_sip_override(parcel_global_id);
    OsslVoiceResult::Success
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_module() -> super::super::WebRtcVoiceModule {
        super::super::WebRtcVoiceModule::new()
    }

    // -- Test 1: os_set_parcel_sip_address stores the address ----------------

    #[test]
    fn set_parcel_sip_address_stores() {
        let mut module = make_module();
        let parcel_id = Uuid::new_v4();
        let addr = "sip:voice@example.com";

        let result = os_set_parcel_sip_address(&mut module, parcel_id, addr);
        assert_eq!(result, OsslVoiceResult::Success);
        assert_eq!(module.get_sip_override(parcel_id), Some(&addr.to_string()));
    }

    // -- Test 2: empty address returns InvalidArguments ----------------------

    #[test]
    fn set_empty_address_returns_invalid() {
        let mut module = make_module();
        let parcel_id = Uuid::new_v4();

        let result = os_set_parcel_sip_address(&mut module, parcel_id, "");
        assert!(matches!(result, OsslVoiceResult::InvalidArguments(_)));
        assert_eq!(module.get_sip_override(parcel_id), None);
    }

    // -- Test 3: validate_sip_address rejects empty and control chars --------

    #[test]
    fn validate_rejects_bad_addresses() {
        assert!(!validate_sip_address(""));
        assert!(!validate_sip_address("sip:bad\x00addr"));
        assert!(!validate_sip_address("sip:bad\x1Faddr"));
        assert!(!validate_sip_address("\x07bell"));
        assert!(!validate_sip_address("sip:with\ttab"));
        assert!(!validate_sip_address("sip:with\nnewline"));
    }

    // -- Test 4: validate_sip_address accepts valid addresses ----------------

    #[test]
    fn validate_accepts_valid_addresses() {
        assert!(validate_sip_address("sip:voice@example.com"));
        assert!(validate_sip_address("webrtc://region/42"));
        assert!(validate_sip_address("simple-address"));
    }

    // -- Test 5: os_clear_parcel_sip_address removes override ----------------

    #[test]
    fn clear_removes_override() {
        let mut module = make_module();
        let parcel_id = Uuid::new_v4();

        os_set_parcel_sip_address(&mut module, parcel_id, "sip:test@example.com");
        assert!(module.get_sip_override(parcel_id).is_some());

        let result = os_clear_parcel_sip_address(&mut module, parcel_id);
        assert_eq!(result, OsslVoiceResult::Success);
        assert_eq!(module.get_sip_override(parcel_id), None);
    }

    // -- Test 6: clear nonexistent override still succeeds -------------------

    #[test]
    fn clear_nonexistent_succeeds() {
        let mut module = make_module();
        let parcel_id = Uuid::new_v4();

        let result = os_clear_parcel_sip_address(&mut module, parcel_id);
        assert_eq!(result, OsslVoiceResult::Success);
    }

    // -- Test 7: overwrite previous address ----------------------------------

    #[test]
    fn overwrite_previous_address() {
        let mut module = make_module();
        let parcel_id = Uuid::new_v4();

        os_set_parcel_sip_address(&mut module, parcel_id, "sip:first@example.com");
        os_set_parcel_sip_address(&mut module, parcel_id, "sip:second@example.com");

        assert_eq!(
            module.get_sip_override(parcel_id),
            Some(&"sip:second@example.com".to_string())
        );
    }

    // -- Test 8: address with control char in middle rejected ----------------

    #[test]
    fn reject_control_char_in_middle() {
        let mut module = make_module();
        let parcel_id = Uuid::new_v4();

        let result = os_set_parcel_sip_address(&mut module, parcel_id, "sip:bad\x00addr");
        assert!(matches!(result, OsslVoiceResult::InvalidArguments(_)));
    }
}
