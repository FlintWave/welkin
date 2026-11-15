// SPDX-License-Identifier: GPL-3.0-or-later

//! Grid portal integration: voice status and estate voice toggle endpoints.

/// Voice service status reported to the grid portal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct VoiceServiceStatus {
    pub healthy: bool,
    pub active_channels: usize,
    pub active_users: usize,
    pub wolfvoice_version: Option<String>,
}

/// Request to toggle voice for an estate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EstateVoiceToggleRequest {
    pub estate_id: u32,
    pub enabled: bool,
}

/// Response from the estate voice toggle.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EstateVoiceToggleResponse {
    pub estate_id: u32,
    pub voice_enabled: bool,
    pub message: String,
}

/// Build a VoiceServiceStatus from the current module state.
///
/// `healthy` is true when the wolfvoice sidecar is reachable.
pub fn build_voice_status(
    healthy: bool,
    active_channels: usize,
    active_users: usize,
    version: Option<String>,
) -> VoiceServiceStatus {
    VoiceServiceStatus {
        healthy,
        active_channels,
        active_users,
        wolfvoice_version: version,
    }
}

/// Encode VoiceServiceStatus as JSON.
pub fn encode_voice_status(status: &VoiceServiceStatus) -> String {
    serde_json::to_string(status).unwrap_or_else(|e| {
        tracing::error!("Failed to serialize VoiceServiceStatus: {}", e);
        "{}".to_string()
    })
}

/// Handle a voice status request, returning JSON.
pub fn handle_voice_status_request(
    healthy: bool,
    active_channels: usize,
    active_users: usize,
    version: Option<String>,
) -> String {
    let status = build_voice_status(healthy, active_channels, active_users, version);
    encode_voice_status(&status)
}

/// Handle an estate voice toggle request.
///
/// Returns a response indicating the new state.
pub fn handle_estate_voice_toggle(request: &EstateVoiceToggleRequest) -> EstateVoiceToggleResponse {
    let message = if request.enabled {
        format!("Voice enabled for estate {}", request.estate_id)
    } else {
        format!("Voice disabled for estate {}", request.estate_id)
    };

    EstateVoiceToggleResponse {
        estate_id: request.estate_id,
        voice_enabled: request.enabled,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_voice_status_constructs_correctly() {
        let status = build_voice_status(true, 5, 12, Some("2.1.0".to_string()));

        assert!(status.healthy);
        assert_eq!(status.active_channels, 5);
        assert_eq!(status.active_users, 12);
        assert_eq!(status.wolfvoice_version, Some("2.1.0".to_string()));
    }

    #[test]
    fn encode_voice_status_produces_valid_json() {
        let status = build_voice_status(true, 3, 7, Some("1.0.0".to_string()));
        let json = encode_voice_status(&status);

        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["healthy"], true);
        assert_eq!(parsed["active_channels"], 3);
        assert_eq!(parsed["active_users"], 7);
        assert_eq!(parsed["wolfvoice_version"], "1.0.0");
    }

    #[test]
    fn encode_voice_status_with_no_version() {
        let status = build_voice_status(false, 0, 0, None);
        let json = encode_voice_status(&status);

        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["healthy"], false);
        assert_eq!(parsed["active_channels"], 0);
        assert_eq!(parsed["active_users"], 0);
        assert!(parsed["wolfvoice_version"].is_null());
    }

    #[test]
    fn handle_estate_voice_toggle_enable() {
        let request = EstateVoiceToggleRequest {
            estate_id: 42,
            enabled: true,
        };

        let response = handle_estate_voice_toggle(&request);

        assert_eq!(response.estate_id, 42);
        assert!(response.voice_enabled);
        assert!(response.message.contains("enabled"));
        assert!(response.message.contains("42"));
    }

    #[test]
    fn handle_estate_voice_toggle_disable() {
        let request = EstateVoiceToggleRequest {
            estate_id: 99,
            enabled: false,
        };

        let response = handle_estate_voice_toggle(&request);

        assert_eq!(response.estate_id, 99);
        assert!(!response.voice_enabled);
        assert!(response.message.contains("disabled"));
        assert!(response.message.contains("99"));
    }

    #[test]
    fn voice_service_status_default_values() {
        let status = VoiceServiceStatus::default();

        assert!(!status.healthy);
        assert_eq!(status.active_channels, 0);
        assert_eq!(status.active_users, 0);
        assert!(status.wolfvoice_version.is_none());
    }

    #[test]
    fn json_roundtrip_for_voice_service_status() {
        let original = build_voice_status(true, 10, 25, Some("3.0.0".to_string()));
        let json = serde_json::to_string(&original).expect("serialize");
        let roundtripped: VoiceServiceStatus = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original.healthy, roundtripped.healthy);
        assert_eq!(original.active_channels, roundtripped.active_channels);
        assert_eq!(original.active_users, roundtripped.active_users);
        assert_eq!(original.wolfvoice_version, roundtripped.wolfvoice_version);
    }
}
