// SPDX-License-Identifier: LGPL-3.0-or-later
use thiserror::Error;

/// Top-level error type for RealmWalker operations.
#[derive(Debug, Error)]
pub enum RealmWalkerError {
    /// XMLRPC login was rejected or could not be parsed.
    #[error("login failed: {0}")]
    LoginFailed(#[from] LoginFailure),

    /// UDP circuit to the simulator could not be established.
    #[error("circuit setup failed: {0}")]
    CircuitFailed(String),

    /// The connection was lost.
    #[error("disconnected: {0}")]
    Disconnected(#[from] DisconnectReason),

    /// A packet could not be decoded.
    #[error("packet error: {0}")]
    PacketError(#[from] welkin_msg::DecodeError),

    /// An HTTP request (caps, login) failed.
    #[error("http error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// A low-level I/O error (UDP socket, file).
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    /// An operation exceeded its deadline.
    #[error("operation timed out")]
    Timeout,

    /// XML parsing or serialization failed.
    #[error("xml error: {0}")]
    XmlError(#[from] quick_xml::DeError),
}

/// Reasons the XMLRPC login can fail.
#[derive(Debug, Error)]
pub enum LoginFailure {
    /// Wrong first name, last name, or password.
    #[error("bad credentials")]
    BadCredentials,

    /// The grid's login service is unreachable or returned a non-200 status.
    #[error("grid unavailable")]
    GridUnavailable,

    /// The account exists but is disabled by the grid administrator.
    #[error("account disabled")]
    AccountDisabled,

    /// Another session is already active for this avatar and the grid
    /// does not allow concurrent logins.
    #[error("presence conflict: avatar already logged in")]
    PresenceConflict,

    /// The XML response could not be parsed.
    #[error("xml parse error: {0}")]
    XmlParseError(String),

    /// A login failure reason the SDK does not have a dedicated variant for.
    #[error("{0}")]
    Other(String),
}

impl LoginFailure {
    /// Map the `reason` string from an XMLRPC login response to a typed failure.
    ///
    /// OpenSim returns freeform English strings; this matches the common ones.
    pub fn from_reason(reason: &str) -> Self {
        let lower = reason.to_lowercase();
        if lower.contains("password") || lower.contains("credentials") || lower.contains("denied") {
            Self::BadCredentials
        } else if lower.contains("disabled") || lower.contains("blocked") {
            Self::AccountDisabled
        } else if lower.contains("already logged in") || lower.contains("presence") {
            Self::PresenceConflict
        } else if lower.contains("unavailable") || lower.contains("offline") || lower.contains("could not connect") {
            Self::GridUnavailable
        } else {
            Self::Other(reason.to_owned())
        }
    }
}

/// Reasons the connection can be lost after a successful login.
#[derive(Debug, Clone, Error)]
pub enum DisconnectReason {
    /// The client requested a graceful logout.
    #[error("client requested disconnect")]
    ClientRequested,

    /// The simulator kicked us with a message.
    #[error("kicked by server: {0}")]
    ServerKicked(String),

    /// No packets received within the keepalive window.
    #[error("network timeout")]
    NetworkTimeout,

    /// A protocol-level error made the connection unrecoverable.
    #[error("protocol error: {0}")]
    ProtocolError(String),
}

pub type Result<T> = std::result::Result<T, RealmWalkerError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_failure_from_reason_maps_common_strings() {
        assert!(matches!(
            LoginFailure::from_reason("Error: bad password"),
            LoginFailure::BadCredentials
        ));
        assert!(matches!(
            LoginFailure::from_reason("Your account has been disabled"),
            LoginFailure::AccountDisabled
        ));
        assert!(matches!(
            LoginFailure::from_reason("You are already logged in"),
            LoginFailure::PresenceConflict
        ));
        assert!(matches!(
            LoginFailure::from_reason("Grid service unavailable"),
            LoginFailure::GridUnavailable
        ));
        assert!(matches!(
            LoginFailure::from_reason("something unexpected"),
            LoginFailure::Other(_)
        ));
    }

    #[test]
    fn display_formats() {
        let e = RealmWalkerError::Timeout;
        assert_eq!(e.to_string(), "operation timed out");

        let e = RealmWalkerError::CircuitFailed("port blocked".into());
        assert_eq!(e.to_string(), "circuit setup failed: port blocked");

        let e = RealmWalkerError::LoginFailed(LoginFailure::BadCredentials);
        assert_eq!(e.to_string(), "login failed: bad credentials");

        let d = DisconnectReason::ServerKicked("go away".into());
        assert_eq!(d.to_string(), "kicked by server: go away");
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let rw_err: RealmWalkerError = io_err.into();
        assert!(matches!(rw_err, RealmWalkerError::IoError(_)));
    }
}
