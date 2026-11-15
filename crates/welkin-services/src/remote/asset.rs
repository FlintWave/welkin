// SPDX-License-Identifier: LGPL-3.0-or-later

//! Remote asset-service connector (client side).
//!
//! Talks to a Robust asset endpoint over REST: `GET /assets/{id}`,
//! `POST /assets`, `POST /get_assets_exist`.

use welkin_data::assets::AssetRow;

use super::RemoteError;
use crate::assets::{asset_to_xml, parse_asset_xml};

/// HTTP client that talks to a remote asset service.
pub struct RemoteAssetConnector {
    client: reqwest::Client,
    base_uri: String,
}

fn now_secs() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i32)
        .unwrap_or(0)
}

impl RemoteAssetConnector {
    /// Create a new connector.  `server_uri` is the base URL of the Robust
    /// server (e.g. `http://assets.example.com:8003`).
    pub fn new(server_uri: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_uri: server_uri.trim_end_matches('/').to_string(),
        }
    }

    /// Fetch a full asset (data included) from the remote service.
    /// Returns `None` on 404.
    pub async fn get_asset(&self, id: &str) -> Result<Option<AssetRow>, RemoteError> {
        let url = format!("{}/assets/{id}", self.base_uri);
        let resp = self.client.get(&url).send().await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let resp = resp.error_for_status()?;
        let body = resp.text().await?;

        let parsed =
            parse_asset_xml(&body).map_err(|e| RemoteError::Parse(format!("asset XML: {e}")))?;

        let now = now_secs();
        Ok(Some(AssetRow {
            name: parsed.name,
            description: parsed.description,
            asset_type: parsed.asset_type,
            local: parsed.local,
            temporary: parsed.temporary,
            data: parsed.data,
            id: parsed.id,
            create_time: now,
            access_time: now,
            asset_flags: parsed.flags,
            creator_id: parsed.creator_id,
        }))
    }

    /// Store an asset on the remote service.
    pub async fn store_asset(&self, asset: &AssetRow) -> Result<(), RemoteError> {
        let url = format!("{}/assets", self.base_uri);
        let xml = asset_to_xml(asset);
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "text/xml")
            .body(xml)
            .send()
            .await?
            .error_for_status()?;

        // The server returns <string>{id}</string> on success.
        let body = resp.text().await?;
        if body.contains(&asset.id) {
            Ok(())
        } else {
            Err(RemoteError::Server(format!(
                "unexpected store response: {body}"
            )))
        }
    }

    /// Check whether an asset exists on the remote service.
    pub async fn asset_exists(&self, id: &str) -> Result<bool, RemoteError> {
        let url = format!("{}/get_assets_exist", self.base_uri);
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
             <ArrayOfString \
              xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
              xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">\
              <string>{id}</string>\
             </ArrayOfString>"
        );
        let body = self
            .client
            .post(&url)
            .header("Content-Type", "text/xml")
            .body(xml)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        // Response is <ArrayOfBoolean><boolean>true/false</boolean></ArrayOfBoolean>
        let booleans = parse_boolean_array(&body)?;
        Ok(booleans.first().copied().unwrap_or(false))
    }
}

/// Parse an `<ArrayOfBoolean>` response into a `Vec<bool>`.
fn parse_boolean_array(xml: &str) -> Result<Vec<bool>, RemoteError> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut in_boolean = false;
    let mut current = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"boolean" => {
                in_boolean = true;
                current.clear();
            }
            Ok(Event::Text(t)) if in_boolean => {
                if let Ok(text) = t.unescape() {
                    current.push_str(&text);
                }
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"boolean" => {
                in_boolean = false;
                out.push(current.trim().eq_ignore_ascii_case("true"));
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(RemoteError::Parse(format!("boolean array: {e}"))),
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_trims_slash() {
        let c = RemoteAssetConnector::new("http://assets.example.com:8003/");
        assert_eq!(c.base_uri, "http://assets.example.com:8003");

        let c2 = RemoteAssetConnector::new("http://assets.example.com:8003");
        assert_eq!(c2.base_uri, "http://assets.example.com:8003");
    }

    #[test]
    fn roundtrip_asset_xml() {
        let row = AssetRow {
            name: "Test Asset".to_string(),
            description: "A test".to_string(),
            asset_type: 0,
            local: 0,
            temporary: 0,
            data: b"hello world".to_vec(),
            id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
            create_time: 1000,
            access_time: 1000,
            asset_flags: 0,
            creator_id: "11111111-2222-3333-4444-555555555555".to_string(),
        };
        let xml = asset_to_xml(&row);
        let parsed = parse_asset_xml(&xml).unwrap();
        assert_eq!(parsed.id, row.id);
        assert_eq!(parsed.name, row.name);
        assert_eq!(parsed.description, row.description);
        assert_eq!(parsed.asset_type, row.asset_type);
        assert_eq!(parsed.data, row.data);
        assert_eq!(parsed.creator_id, row.creator_id);
    }

    #[test]
    fn parse_boolean_array_response() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><ArrayOfBoolean xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema"><boolean>true</boolean><boolean>false</boolean></ArrayOfBoolean>"#;
        let bools = parse_boolean_array(xml).unwrap();
        assert_eq!(bools, vec![true, false]);
    }
}
