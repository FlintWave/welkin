// SPDX-License-Identifier: LGPL-3.0-or-later

//! Remote user-account connector (client side).

use super::RemoteError;
use std::collections::HashMap;
use uuid::Uuid;

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

    pub async fn get_created(&self, user_id: &str) -> Result<Option<i64>, RemoteError> {
        let params = [("METHOD", "getaccount"), ("UserID", user_id)];
        let body = self
            .client
            .post(&self.accounts_uri)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;
        Ok(super::extract_text(&body, "Created").and_then(|s| s.trim().parse::<i64>().ok()))
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

    pub async fn get_names(&self, user_ids: &[Uuid]) -> Result<HashMap<Uuid, String>, RemoteError> {
        let mut out: HashMap<Uuid, String> = HashMap::new();
        for id in user_ids {
            out.insert(*id, "Unknown User".to_string());
        }
        if user_ids.is_empty() {
            return Ok(out);
        }

        let mut form: Vec<(String, String)> = vec![("METHOD".to_string(), "getmultiaccounts".to_string())];
        for id in user_ids {
            form.push(("IDS[]".to_string(), id.to_string()));
        }
        let body = self
            .client
            .post(&self.accounts_uri)
            .form(&form)
            .send()
            .await?
            .text()
            .await?;

        for (pid, first, last) in parse_multi_accounts(&body) {
            if let Ok(id) = pid.parse::<Uuid>() {
                out.insert(id, format!("{first} {last}"));
            }
        }

        for id in user_ids {
            if out.get(id).map(|n| n == "Unknown User").unwrap_or(true) {
                if let Ok((first, last)) = self.get_name(&id.to_string()).await {
                    if !(first == "Unknown" && last == "User") {
                        out.insert(*id, format!("{first} {last}"));
                    }
                }
            }
        }

        Ok(out)
    }

    pub async fn get_users_names(&self, user_ids: &[Uuid]) -> Result<HashMap<Uuid, String>, RemoteError> {
        self.get_names(user_ids).await
    }
}

pub fn parse_multi_accounts(xml: &str) -> Vec<(String, String, String)> {
    let mut result = Vec::new();
    let mut rest = xml;
    while let Some(start_idx) = rest.find("<account") {
        let after = &rest[start_idx..];
        let Some(tag_end) = after.find('>') else { break; };
        let tag_name = after[..tag_end].trim_start_matches('<').split_whitespace().next().unwrap_or("");
        if tag_name.is_empty() || !tag_name.starts_with("account") {
            rest = &rest[start_idx + 1..];
            continue;
        }
        let close_tag = format!("</{}>", tag_name);
        let Some(end_rel) = after.find(&close_tag) else {
            rest = &rest[start_idx + 1..];
            continue;
        };
        let block = &after[tag_end + 1..tag_end + 1 + (end_rel - (tag_end + 1))];
        let pid = super::extract_text(block, "PrincipalID")
            .or_else(|| super::extract_text(block, "UserID"))
            .unwrap_or_default();
        let first = super::extract_text(block, "FirstName").unwrap_or_else(|| "Unknown".to_string());
        let last = super::extract_text(block, "LastName").unwrap_or_else(|| "User".to_string());
        result.push((pid, first, last));
        rest = &after[end_rel + close_tag.len()..];
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_multi_accounts_extracts_each_account() {
        let xml = r#"<ServerResponse><account0 type="List"><PrincipalID>11111111-2222-3333-4444-555555555555</PrincipalID><FirstName>Ada</FirstName><LastName>Lovelace</LastName></account0><account1 type="List"><PrincipalID>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</PrincipalID><FirstName>Grace</FirstName><LastName>Hopper</LastName></account1></ServerResponse>"#;
        let got = parse_multi_accounts(xml);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].1, "Ada");
        assert_eq!(got[0].2, "Lovelace");
        assert_eq!(got[1].1, "Grace");
    }

    #[test]
    fn parse_multi_accounts_empty_on_no_blocks() {
        let xml = r#"<ServerResponse><result>null</result></ServerResponse>"#;
        assert!(parse_multi_accounts(xml).is_empty());
    }
}
