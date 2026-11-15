// SPDX-License-Identifier: GPL-3.0-or-later

//! Remote user-account connector (client side).
//!
//! Talks to a ROBUST UserAccountService endpoint to resolve agent UUIDs to
//! first/last names.

use super::RemoteError;

pub struct RemoteAccountsConnector {
    client: reqwest::Client,
    accounts_uri: String,
}

impl RemoteAccountsConnector {
    pub fn new(server_uri: &str) -> Self {
        let base = server_uri.trim_end_matches('/');
        Self {
            client: reqwest::Client::new(),
            accounts_uri: format!("{base}/accounts"),
        }
    }

    pub async fn get_name(&self, user_id: &str) -> Result<(String, String), RemoteError> {
        let params = [("METHOD", "getaccount"), ("UserID", user_id)];
        let body = self
            .client
            .post(&self.accounts_uri)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;

        let first = super::extract_text(&body, "FirstName")
            .unwrap_or_else(|| "Unknown".to_string());
        let last = super::extract_text(&body, "LastName")
            .unwrap_or_else(|| "User".to_string());
        Ok((first, last))
    }
}
