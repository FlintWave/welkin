// SPDX-License-Identifier: LGPL-3.0-or-later

//! Remote grid-service connector (client side).
//!
//! Talks to a Robust grid endpoint over HTTP using URL-encoded form POSTs and
//! `<ServerResponse>` XML replies.

use quick_xml::{Reader, events::Event};
use welkin_data::grid::RegionRow;

use super::RemoteError;

/// HTTP client that talks to a remote grid service (e.g. `http://grid:8003/grid`).
pub struct RemoteGridConnector {
    client: reqwest::Client,
    grid_uri: String,
}

impl RemoteGridConnector {
    /// Create a new connector.  `server_uri` is the base URL of the Robust
    /// server (e.g. `http://grid.example.com:8003`).
    pub fn new(server_uri: &str) -> Self {
        let base = server_uri.trim_end_matches('/');
        Self {
            client: reqwest::Client::new(),
            grid_uri: format!("{base}/grid"),
        }
    }

    /// Register (or update) a region with the remote grid.
    pub async fn register_region(&self, region: &RegionRow) -> Result<(), RemoteError> {
        let params = [
            ("METHOD", "register"),
            ("uuid", &region.uuid),
            ("locX", &region.loc_x.to_string()),
            ("locY", &region.loc_y.to_string()),
            ("sizeX", &region.size_x.to_string()),
            ("sizeY", &region.size_y.to_string()),
            ("regionName", &region.region_name),
            ("serverIP", &region.server_ip),
            ("serverPort", &region.server_port.unwrap_or(0).to_string()),
            ("serverURI", &region.server_uri),
            (
                "serverHttpPort",
                &region.server_http_port.unwrap_or(0).to_string(),
            ),
            ("regionMapTexture", &region.region_map_texture),
            ("parcelMapTexture", &region.parcel_map_texture),
            ("access", &region.access.to_string()),
            ("regionSecret", &region.region_secret),
            ("owner_uuid", &region.owner_uuid),
            ("Token", &region.token),
            ("flags", &region.flags.to_string()),
            ("ScopeID", &region.scope_id),
            ("PrincipalID", &region.principal_id),
        ];
        let body = self
            .client
            .post(&self.grid_uri)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;

        // Check for <Result>Success</Result>
        let result = super::extract_text(&body, "Result").unwrap_or_default();
        if result == "Success" {
            Ok(())
        } else {
            let msg = super::extract_text(&body, "Message").unwrap_or(result);
            Err(RemoteError::Server(msg))
        }
    }

    /// Deregister (remove) a region from the remote grid.
    pub async fn deregister_region(&self, region_id: &str) -> Result<(), RemoteError> {
        let params = [("METHOD", "deregister"), ("REGIONID", region_id)];
        let body = self
            .client
            .post(&self.grid_uri)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;

        let result = super::extract_text(&body, "Result").unwrap_or_default();
        if result == "Success" {
            Ok(())
        } else {
            let msg = super::extract_text(&body, "Message").unwrap_or(result);
            Err(RemoteError::Server(msg))
        }
    }

    /// Fetch the neighbours of a region.
    pub async fn get_neighbours(
        &self,
        scope_id: &str,
        region_id: &str,
    ) -> Result<Vec<RegionRow>, RemoteError> {
        let params = [
            ("METHOD", "get_neighbours"),
            ("SCOPEID", scope_id),
            ("REGIONID", region_id),
        ];
        let body = self
            .client
            .post(&self.grid_uri)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;

        parse_region_list(&body)
    }

    /// Look up a single region by UUID.  Returns `None` when the grid says "null".
    pub async fn get_region_by_uuid(
        &self,
        scope_id: &str,
        region_id: &str,
    ) -> Result<Option<RegionRow>, RemoteError> {
        let params = [
            ("METHOD", "get_region_by_uuid"),
            ("SCOPEID", scope_id),
            ("REGIONID", region_id),
        ];
        let body = self
            .client
            .post(&self.grid_uri)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;

        parse_single_region(&body)
    }
}

// ---------------------------------------------------------------------------
// XML parsing helpers
// ---------------------------------------------------------------------------

/// Parse a `<ServerResponse>` that contains a single `<result type="List">`
/// region, or `<result>null</result>` for "not found".
fn parse_single_region(xml: &str) -> Result<Option<RegionRow>, RemoteError> {
    // Quick check for the "null" sentinel.
    if let Some(text) = super::extract_text(xml, "result")
        && text == "null"
    {
        return Ok(None);
    }
    // Try to parse the region from <result type="List">...</result>
    let regions = parse_regions_from_xml(xml)?;
    Ok(regions.into_iter().next())
}

/// Parse a `<ServerResponse>` that contains zero or more `<regionN type="List">`
/// children, each holding one region's fields.
fn parse_region_list(xml: &str) -> Result<Vec<RegionRow>, RemoteError> {
    parse_regions_from_xml(xml)
}

/// Walk the XML and collect every region element (any direct child of
/// `<ServerResponse>` that carries `type="List"` and contains `<uuid>`).
fn parse_regions_from_xml(xml: &str) -> Result<Vec<RegionRow>, RemoteError> {
    let mut reader = Reader::from_str(xml);
    let mut regions: Vec<RegionRow> = Vec::new();
    let mut depth: Vec<String> = Vec::new();
    let mut current: Option<RegionBuilder> = None;
    let mut current_field: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if depth.len() == 1 {
                    // Direct child of <ServerResponse>
                    let is_list = e.attributes().flatten().any(|a| {
                        a.key.as_ref() == b"type"
                            && a.unescape_value().map(|v| v == "List").unwrap_or(false)
                    });
                    if is_list {
                        current = Some(RegionBuilder::default());
                    }
                } else if depth.len() == 2 && current.is_some() {
                    current_field = Some(name.clone());
                }
                depth.push(name);
            }
            Ok(Event::End(_)) => {
                depth.pop();
                if depth.len() == 1
                    && let Some(builder) = current.take()
                    && let Some(r) = builder.build()
                {
                    regions.push(r);
                }
                if depth.len() == 2 {
                    current_field = None;
                }
            }
            Ok(Event::Text(t)) => {
                if let (Some(b), Some(field)) = (&mut current, &current_field)
                    && let Ok(text) = t.unescape()
                {
                    b.set(field, &text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(RemoteError::Parse(e.to_string())),
            _ => {}
        }
    }
    Ok(regions)
}

/// Accumulates region fields from XML text and produces a `RegionRow`.
#[derive(Default)]
struct RegionBuilder {
    uuid: String,
    loc_x: i32,
    loc_y: i32,
    size_x: i32,
    size_y: i32,
    region_name: String,
    flags: i32,
    server_ip: String,
    server_http_port: i32,
    server_uri: String,
    server_port: i32,
    region_map_texture: String,
    parcel_map_texture: String,
    access: i32,
    region_secret: String,
    owner_uuid: String,
    token: String,
    scope_id: String,
    principal_id: String,
}

impl RegionBuilder {
    fn set(&mut self, field: &str, value: &str) {
        match field {
            "uuid" => self.uuid = value.to_string(),
            "locX" => self.loc_x = value.trim().parse().unwrap_or(0),
            "locY" => self.loc_y = value.trim().parse().unwrap_or(0),
            "sizeX" => self.size_x = value.trim().parse().unwrap_or(0),
            "sizeY" => self.size_y = value.trim().parse().unwrap_or(0),
            "regionName" => self.region_name = value.to_string(),
            "flags" => self.flags = value.trim().parse().unwrap_or(0),
            "serverIP" => self.server_ip = value.to_string(),
            "serverHttpPort" => self.server_http_port = value.trim().parse().unwrap_or(0),
            "serverURI" => self.server_uri = value.to_string(),
            "serverPort" => self.server_port = value.trim().parse().unwrap_or(0),
            "regionMapTexture" => self.region_map_texture = value.to_string(),
            "parcelMapTexture" => self.parcel_map_texture = value.to_string(),
            "access" => self.access = value.trim().parse().unwrap_or(0),
            "regionSecret" => self.region_secret = value.to_string(),
            "owner_uuid" => self.owner_uuid = value.to_string(),
            "Token" => self.token = value.to_string(),
            "ScopeID" => self.scope_id = value.to_string(),
            "PrincipalID" => self.principal_id = value.to_string(),
            _ => {}
        }
    }

    fn build(self) -> Option<RegionRow> {
        if self.uuid.is_empty() {
            return None;
        }
        let loc_x = self.loc_x;
        let loc_y = self.loc_y;
        Some(RegionRow {
            uuid: self.uuid,
            region_handle: (i64::from(loc_x) << 32) | i64::from(loc_y),
            region_name: self.region_name,
            region_secret: self.region_secret,
            server_ip: self.server_ip,
            server_port: Some(self.server_port),
            server_uri: self.server_uri,
            loc_x,
            loc_y,
            loc_z: 0,
            region_map_texture: self.region_map_texture,
            server_http_port: Some(self.server_http_port),
            owner_uuid: self.owner_uuid,
            access: self.access,
            scope_id: self.scope_id,
            size_x: if self.size_x == 0 { 256 } else { self.size_x },
            size_y: if self.size_y == 0 { 256 } else { self.size_y },
            flags: self.flags,
            last_seen: 0,
            principal_id: self.principal_id,
            token: self.token,
            parcel_map_texture: self.parcel_map_texture,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_formats_uri() {
        let c = RemoteGridConnector::new("http://grid.example.com:8003");
        assert_eq!(c.grid_uri, "http://grid.example.com:8003/grid");

        let c2 = RemoteGridConnector::new("http://grid.example.com:8003/");
        assert_eq!(c2.grid_uri, "http://grid.example.com:8003/grid");
    }

    #[test]
    fn parse_single_region_null() {
        let xml = r#"<?xml version="1.0"?><ServerResponse><result>null</result></ServerResponse>"#;
        let r = parse_single_region(xml).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn parse_single_region_found() {
        let xml = r#"<?xml version="1.0"?><ServerResponse><result type="List"><uuid>aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee</uuid><locX>256000</locX><locY>256000</locY><sizeX>256</sizeX><sizeY>256</sizeY><regionName>Test Region</regionName><flags>8</flags><serverIP>127.0.0.1</serverIP><serverHttpPort>9000</serverHttpPort><serverURI>http://127.0.0.1:9000</serverURI><serverPort>9000</serverPort><regionMapTexture>00000000-0000-0000-0000-000000000000</regionMapTexture><parcelMapTexture>00000000-0000-0000-0000-000000000000</parcelMapTexture><access>1</access><regionSecret>secret</regionSecret><owner_uuid>11111111-2222-3333-4444-555555555555</owner_uuid><Token>tok</Token></result></ServerResponse>"#;
        let r = parse_single_region(xml).unwrap().unwrap();
        assert_eq!(r.uuid, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        assert_eq!(r.loc_x, 256000);
        assert_eq!(r.loc_y, 256000);
        assert_eq!(r.size_x, 256);
        assert_eq!(r.region_name, "Test Region");
        assert_eq!(r.server_ip, "127.0.0.1");
        assert_eq!(r.server_port, Some(9000));
        assert_eq!(r.flags, 8);
    }

    #[test]
    fn parse_region_list_multiple() {
        let xml = r#"<?xml version="1.0"?><ServerResponse><region0 type="List"><uuid>aaaa0000-0000-0000-0000-000000000000</uuid><locX>256000</locX><locY>256000</locY><sizeX>256</sizeX><sizeY>256</sizeY><regionName>Region A</regionName><flags>0</flags><serverIP>10.0.0.1</serverIP><serverHttpPort>9000</serverHttpPort><serverURI>http://10.0.0.1:9000</serverURI><serverPort>9000</serverPort><regionMapTexture>00000000-0000-0000-0000-000000000000</regionMapTexture><parcelMapTexture>00000000-0000-0000-0000-000000000000</parcelMapTexture><access>1</access><regionSecret></regionSecret><owner_uuid>00000000-0000-0000-0000-000000000000</owner_uuid><Token></Token></region0><region1 type="List"><uuid>bbbb0000-0000-0000-0000-000000000000</uuid><locX>256256</locX><locY>256000</locY><sizeX>256</sizeX><sizeY>256</sizeY><regionName>Region B</regionName><flags>0</flags><serverIP>10.0.0.2</serverIP><serverHttpPort>9000</serverHttpPort><serverURI>http://10.0.0.2:9000</serverURI><serverPort>9000</serverPort><regionMapTexture>00000000-0000-0000-0000-000000000000</regionMapTexture><parcelMapTexture>00000000-0000-0000-0000-000000000000</parcelMapTexture><access>1</access><regionSecret></regionSecret><owner_uuid>00000000-0000-0000-0000-000000000000</owner_uuid><Token></Token></region1></ServerResponse>"#;
        let regions = parse_region_list(xml).unwrap();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].region_name, "Region A");
        assert_eq!(regions[1].region_name, "Region B");
        assert_eq!(regions[1].loc_x, 256256);
    }
}
