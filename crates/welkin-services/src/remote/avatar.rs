// SPDX-License-Identifier: LGPL-3.0-or-later

//! Remote avatar-service connector (client side): the key/value appearance data the grid
//! keeps per avatar, including which inventory items are worn as attachments (`_ap_N`).

use quick_xml::{Reader, events::Event};

use super::RemoteError;

pub struct RemoteAvatarConnector {
    client: reqwest::Client,
    avatar_uri: String,
}

impl RemoteAvatarConnector {
    pub fn new(server_uri: &str) -> Self {
        let base = server_uri.trim_end_matches('/');
        Self {
            client: reqwest::Client::new(),
            avatar_uri: format!("{base}/avatar"),
        }
    }

    /// All stored name/value pairs for an avatar (empty if none are stored).
    pub async fn get_avatar(&self, user_id: &str) -> Result<Vec<(String, String)>, RemoteError> {
        let params = [("METHOD", "getavatar"), ("UserID", user_id)];
        let body = self.client.post(&self.avatar_uri).form(&params).send().await?.text().await?;
        Ok(parse_avatar_response(&body))
    }

    /// Set (add or replace) individual appearance items.
    pub async fn set_items(&self, user_id: &str, items: &[(String, String)]) -> Result<bool, RemoteError> {
        if items.is_empty() {
            return Ok(true);
        }
        let mut params: Vec<(String, String)> = vec![
            ("METHOD".into(), "setitems".into()),
            ("UserID".into(), user_id.into()),
        ];
        for (name, _) in items {
            params.push(("Names[]".into(), name.clone()));
        }
        for (_, value) in items {
            params.push(("Values[]".into(), value.clone()));
        }
        self.send(&params).await
    }

    /// Remove individual appearance items by name.
    pub async fn remove_items(&self, user_id: &str, names: &[String]) -> Result<bool, RemoteError> {
        if names.is_empty() {
            return Ok(true);
        }
        let mut params: Vec<(String, String)> = vec![
            ("METHOD".into(), "removeitems".into()),
            ("UserID".into(), user_id.into()),
        ];
        for name in names {
            params.push(("Names[]".into(), name.clone()));
        }
        self.send(&params).await
    }

    async fn send(&self, params: &[(String, String)]) -> Result<bool, RemoteError> {
        let body = self.client.post(&self.avatar_uri).form(params).send().await?.text().await?;
        Ok(body.contains("Success"))
    }
}

/// The children of the `<result type="List">` element of a `getavatar` reply, in order,
/// without the `AvatarType` marker.
fn parse_avatar_response(xml: &str) -> Vec<(String, String)> {
    let mut reader = Reader::from_str(xml);
    let mut depth = 0usize;
    let mut in_result = false;
    let mut current: Option<String> = None;
    let mut text = String::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if name == "result" && depth == 1 {
                    in_result = true;
                } else if in_result && depth == 2 {
                    current = Some(name);
                    text.clear();
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if in_result && depth == 2 && name != "AvatarType" {
                    out.push((name, String::new()));
                }
            }
            Ok(Event::Text(t)) => {
                if current.is_some() {
                    text.push_str(&t.unescape().map(|c| c.into_owned()).unwrap_or_default());
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if in_result && depth == 2 {
                    if let Some(name) = current.take()
                        && name != "AvatarType"
                    {
                        out.push((name, std::mem::take(&mut text)));
                    }
                } else if depth == 1 {
                    in_result = false;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_items_of_a_getavatar_reply() {
        let xml = "<?xml version=\"1.0\"?><ServerResponse><result type=\"List\"><AvatarType>1</AvatarType><Height>1.7</Height><_ap_2>aaa-111</_ap_2><Empty></Empty></result></ServerResponse>";
        assert_eq!(
            parse_avatar_response(xml),
            vec![
                ("Height".to_string(), "1.7".to_string()),
                ("_ap_2".to_string(), "aaa-111".to_string()),
                ("Empty".to_string(), String::new()),
            ]
        );
        assert!(parse_avatar_response("<ServerResponse><result>Failure</result></ServerResponse>").is_empty());
    }
}
