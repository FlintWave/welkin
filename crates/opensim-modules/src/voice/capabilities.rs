// SPDX-License-Identifier: GPL-3.0-or-later

//! Voice capability handlers: ProvisionVoiceAccountRequest and
//! ParcelVoiceInfoRequest.
//!
//! These generate LLSD XML responses matching the viewer protocol for
//! WebRTC voice (Aurora Viewer compatible).

use tracing::debug;
use uuid::Uuid;

/// Request context for ProvisionVoiceAccountRequest capability.
#[derive(Debug, Clone)]
pub struct ProvisionVoiceAccountRequest {
    pub agent_id: Uuid,
    pub agent_name: String,
}

/// Response for ProvisionVoiceAccountRequest.
#[derive(Debug, Clone)]
pub struct ProvisionVoiceAccountResponse {
    pub username: String,
    pub password: String,
    pub voice_server_type: String,
    pub voice_sip_uri_hostname: String,
}

/// Request context for ParcelVoiceInfoRequest capability.
#[derive(Debug, Clone)]
pub struct ParcelVoiceInfoRequest {
    pub agent_id: Uuid,
    pub region_name: String,
    pub parcel_local_id: i32,
    pub estate_voice_enabled: bool,
    pub parcel_voice_enabled: bool,
    pub channel_uri: String,
    pub channel_credentials: String,
}

/// Response for ParcelVoiceInfoRequest.
#[derive(Debug, Clone)]
pub struct ParcelVoiceInfoResponse {
    pub region_name: String,
    pub parcel_local_id: i32,
    pub channel_uri: String,
    pub channel_credentials: String,
}

/// Build a ProvisionVoiceAccountResponse from voice credentials.
pub fn build_provision_response(
    creds: &super::wolfvoice_client::VoiceCredentials,
) -> ProvisionVoiceAccountResponse {
    // Extract hostname from the voice server URI.
    // The URI is expected to be in the form "scheme://host:port/path" or similar.
    let hostname = extract_hostname(&creds.voice_server_uri);

    ProvisionVoiceAccountResponse {
        username: creds.username.clone(),
        password: creds.password.clone(),
        voice_server_type: "webrtc".to_string(),
        voice_sip_uri_hostname: hostname,
    }
}

/// Extract hostname from a URI string.
///
/// Handles formats like "webrtc://host:port/path", "host:port", or bare "host".
fn extract_hostname(uri: &str) -> String {
    let without_scheme = if let Some(idx) = uri.find("://") {
        &uri[idx + 3..]
    } else {
        uri
    };

    // Strip path
    let without_path = if let Some(idx) = without_scheme.find('/') {
        &without_scheme[..idx]
    } else {
        without_scheme
    };

    // Strip port
    let hostname = if let Some(idx) = without_path.rfind(':') {
        &without_path[..idx]
    } else {
        without_path
    };

    hostname.to_string()
}

/// Escape a string for inclusion in XML text content.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Encode a ProvisionVoiceAccountResponse as LLSD XML.
pub fn encode_provision_response(resp: &ProvisionVoiceAccountResponse) -> String {
    format!(
        "<llsd><map>\
         <key>username</key><string>{}</string>\
         <key>password</key><string>{}</string>\
         <key>voice_server_type</key><string>{}</string>\
         <key>voice_sip_uri_hostname</key><string>{}</string>\
         </map></llsd>",
        xml_escape(&resp.username),
        xml_escape(&resp.password),
        xml_escape(&resp.voice_server_type),
        xml_escape(&resp.voice_sip_uri_hostname),
    )
}

/// Handle the ProvisionVoiceAccountRequest capability.
/// Returns the LLSD XML response string.
pub fn handle_provision_voice_account(
    request: &ProvisionVoiceAccountRequest,
    creds: &super::wolfvoice_client::VoiceCredentials,
) -> String {
    debug!(
        agent_id = %request.agent_id,
        agent_name = %request.agent_name,
        "Handling ProvisionVoiceAccountRequest"
    );

    let resp = build_provision_response(creds);
    encode_provision_response(&resp)
}

/// Build a ParcelVoiceInfoResponse, checking estate and parcel voice flags.
/// If voice is not enabled at either level, returns empty channel_uri.
pub fn build_parcel_voice_response(
    request: &ParcelVoiceInfoRequest,
) -> ParcelVoiceInfoResponse {
    let (channel_uri, channel_credentials) =
        if request.estate_voice_enabled && request.parcel_voice_enabled {
            (
                request.channel_uri.clone(),
                request.channel_credentials.clone(),
            )
        } else {
            debug!(
                agent_id = %request.agent_id,
                estate_voice = request.estate_voice_enabled,
                parcel_voice = request.parcel_voice_enabled,
                "Voice disabled, returning empty channel URI"
            );
            (String::new(), String::new())
        };

    ParcelVoiceInfoResponse {
        region_name: request.region_name.clone(),
        parcel_local_id: request.parcel_local_id,
        channel_uri,
        channel_credentials,
    }
}

/// Encode a ParcelVoiceInfoResponse as LLSD XML.
pub fn encode_parcel_voice_response(resp: &ParcelVoiceInfoResponse) -> String {
    format!(
        "<llsd><map>\
         <key>region_name</key><string>{}</string>\
         <key>parcel_local_id</key><integer>{}</integer>\
         <key>voice_credentials</key><map>\
         <key>channel_uri</key><string>{}</string>\
         <key>channel_credentials</key><string>{}</string>\
         </map>\
         </map></llsd>",
        xml_escape(&resp.region_name),
        resp.parcel_local_id,
        xml_escape(&resp.channel_uri),
        xml_escape(&resp.channel_credentials),
    )
}

/// Handle the ParcelVoiceInfoRequest capability.
/// Returns the LLSD XML response string.
pub fn handle_parcel_voice_info(
    request: &ParcelVoiceInfoRequest,
) -> String {
    debug!(
        agent_id = %request.agent_id,
        region = %request.region_name,
        parcel_id = request.parcel_local_id,
        "Handling ParcelVoiceInfoRequest"
    );

    let resp = build_parcel_voice_response(request);
    encode_parcel_voice_response(&resp)
}

/// Produce the empty/error LLSD response.
pub fn llsd_undef() -> String {
    "<llsd><undef/></llsd>".to_string()
}

/// Construct a channel URI from region and parcel info.
/// Format: "webrtc://{region_name}/{parcel_local_id}"
pub fn make_channel_uri(region_name: &str, parcel_local_id: i32) -> String {
    format!("webrtc://{}/{}", region_name, parcel_local_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_creds() -> super::super::wolfvoice_client::VoiceCredentials {
        super::super::wolfvoice_client::VoiceCredentials {
            username: "agent_user".to_string(),
            password: "s3cret".to_string(),
            voice_server_uri: "webrtc://voice.example.com:9000/voice".to_string(),
        }
    }

    fn sample_parcel_request(estate_voice: bool, parcel_voice: bool) -> ParcelVoiceInfoRequest {
        ParcelVoiceInfoRequest {
            agent_id: Uuid::nil(),
            region_name: "TestRegion".to_string(),
            parcel_local_id: 42,
            estate_voice_enabled: estate_voice,
            parcel_voice_enabled: parcel_voice,
            channel_uri: "webrtc://TestRegion/42".to_string(),
            channel_credentials: "creds123".to_string(),
        }
    }

    #[test]
    fn test_build_provision_response_maps_fields() {
        let creds = sample_creds();
        let resp = build_provision_response(&creds);

        assert_eq!(resp.username, "agent_user");
        assert_eq!(resp.password, "s3cret");
        assert_eq!(resp.voice_server_type, "webrtc");
        assert_eq!(resp.voice_sip_uri_hostname, "voice.example.com");
    }

    #[test]
    fn test_encode_provision_response_produces_valid_llsd() {
        let resp = ProvisionVoiceAccountResponse {
            username: "test_user".to_string(),
            password: "pass".to_string(),
            voice_server_type: "webrtc".to_string(),
            voice_sip_uri_hostname: "voice.example.com".to_string(),
        };
        let xml = encode_provision_response(&resp);

        assert!(xml.starts_with("<llsd><map>"));
        assert!(xml.ends_with("</map></llsd>"));
        assert!(xml.contains("<key>username</key><string>test_user</string>"));
        assert!(xml.contains("<key>password</key><string>pass</string>"));
        assert!(xml.contains("<key>voice_server_type</key><string>webrtc</string>"));
        assert!(xml.contains("<key>voice_sip_uri_hostname</key><string>voice.example.com</string>"));
    }

    #[test]
    fn test_handle_provision_voice_account_end_to_end() {
        let request = ProvisionVoiceAccountRequest {
            agent_id: Uuid::nil(),
            agent_name: "Test User".to_string(),
        };
        let creds = sample_creds();
        let xml = handle_provision_voice_account(&request, &creds);

        assert!(xml.starts_with("<llsd><map>"));
        assert!(xml.contains("<key>username</key><string>agent_user</string>"));
        assert!(xml.contains("<key>password</key><string>s3cret</string>"));
        assert!(xml.contains("<key>voice_server_type</key><string>webrtc</string>"));
        assert!(xml.contains("<key>voice_sip_uri_hostname</key><string>voice.example.com</string>"));
    }

    #[test]
    fn test_build_parcel_voice_response_with_voice_enabled() {
        let request = sample_parcel_request(true, true);
        let resp = build_parcel_voice_response(&request);

        assert_eq!(resp.region_name, "TestRegion");
        assert_eq!(resp.parcel_local_id, 42);
        assert_eq!(resp.channel_uri, "webrtc://TestRegion/42");
        assert_eq!(resp.channel_credentials, "creds123");
    }

    #[test]
    fn test_build_parcel_voice_response_estate_disabled() {
        let request = sample_parcel_request(false, true);
        let resp = build_parcel_voice_response(&request);

        assert!(resp.channel_uri.is_empty());
        assert!(resp.channel_credentials.is_empty());
    }

    #[test]
    fn test_build_parcel_voice_response_parcel_disabled() {
        let request = sample_parcel_request(true, false);
        let resp = build_parcel_voice_response(&request);

        assert!(resp.channel_uri.is_empty());
        assert!(resp.channel_credentials.is_empty());
    }

    #[test]
    fn test_encode_parcel_voice_response_produces_valid_llsd() {
        let resp = ParcelVoiceInfoResponse {
            region_name: "MyRegion".to_string(),
            parcel_local_id: 7,
            channel_uri: "webrtc://MyRegion/7".to_string(),
            channel_credentials: "abc".to_string(),
        };
        let xml = encode_parcel_voice_response(&resp);

        assert!(xml.starts_with("<llsd><map>"));
        assert!(xml.ends_with("</map></llsd>"));
        assert!(xml.contains("<key>region_name</key><string>MyRegion</string>"));
        assert!(xml.contains("<key>parcel_local_id</key><integer>7</integer>"));
        assert!(xml.contains("<key>voice_credentials</key><map>"));
        assert!(xml.contains("<key>channel_uri</key><string>webrtc://MyRegion/7</string>"));
        assert!(xml.contains("<key>channel_credentials</key><string>abc</string>"));
    }

    #[test]
    fn test_handle_parcel_voice_info_end_to_end() {
        let request = sample_parcel_request(true, true);
        let xml = handle_parcel_voice_info(&request);

        assert!(xml.starts_with("<llsd><map>"));
        assert!(xml.contains("<key>region_name</key><string>TestRegion</string>"));
        assert!(xml.contains("<key>parcel_local_id</key><integer>42</integer>"));
        assert!(xml.contains("<key>channel_uri</key><string>webrtc://TestRegion/42</string>"));
    }

    #[test]
    fn test_make_channel_uri_format() {
        let uri = make_channel_uri("SimWorld", 99);
        assert_eq!(uri, "webrtc://SimWorld/99");
    }

    #[test]
    fn test_llsd_undef() {
        assert_eq!(llsd_undef(), "<llsd><undef/></llsd>");
    }

    #[test]
    fn test_xml_escape_special_chars() {
        let resp = ProvisionVoiceAccountResponse {
            username: "user<>&\"'test".to_string(),
            password: "pass".to_string(),
            voice_server_type: "webrtc".to_string(),
            voice_sip_uri_hostname: "host".to_string(),
        };
        let xml = encode_provision_response(&resp);
        assert!(xml.contains("user&lt;&gt;&amp;&quot;&apos;test"));
    }

    #[test]
    fn test_extract_hostname_bare() {
        assert_eq!(extract_hostname("voice.example.com"), "voice.example.com");
    }

    #[test]
    fn test_extract_hostname_with_port() {
        assert_eq!(extract_hostname("voice.example.com:9000"), "voice.example.com");
    }

    #[test]
    fn test_extract_hostname_with_scheme_and_path() {
        assert_eq!(
            extract_hostname("webrtc://voice.example.com:9000/voice"),
            "voice.example.com"
        );
    }

    #[test]
    fn test_build_parcel_voice_response_both_disabled() {
        let request = sample_parcel_request(false, false);
        let resp = build_parcel_voice_response(&request);
        assert!(resp.channel_uri.is_empty());
        assert!(resp.channel_credentials.is_empty());
    }
}
