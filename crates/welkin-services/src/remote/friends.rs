// SPDX-License-Identifier: LGPL-3.0-or-later

use std::time::Duration;

use quick_xml::events::Event;
use uuid::Uuid;

use super::RemoteError;

#[derive(Debug, Clone, PartialEq)]
pub struct FriendRecord {
    pub friend_id: Uuid,
    pub my_flags: i32,
    pub their_flags: i32,
}

#[derive(Default)]
struct PartialRecord {
    friend: Option<String>,
    my_flags: i32,
    their_flags: i32,
}

pub fn parse_friends(xml: &str) -> Vec<FriendRecord> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut records = Vec::new();
    let mut current: Option<PartialRecord> = None;
    let mut current_field: Option<String> = None;
    let mut text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());

                if is_friend_record(local) {
                    if let Some(partial) = current.take() {
                        finalize_record(&mut records, partial);
                    }
                    current = Some(PartialRecord::default());
                    current_field = None;
                    text.clear();
                } else if current.is_some() {
                    current_field = Some(String::from_utf8_lossy(local).into_owned());
                    text.clear();
                }
            }
            Ok(Event::Text(ref e)) => {
                if current.is_some() && current_field.is_some() {
                    text.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::CData(ref e)) => {
                if current.is_some() && current_field.is_some() {
                    text.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());

                if current.is_some() {
                    if is_friend_record(local) {
                        let finished = current.take().unwrap_or_default();
                        finalize_record(&mut records, finished);
                        current_field = None;
                        text.clear();
                    } else if let Some(field) = current_field.take() {
                        let value = text.trim().to_string();
                        if let Some(record) = current.as_mut() {
                            match field.as_str() {
                                "Friend" => record.friend = Some(value),
                                "MyFlags" => record.my_flags = value.parse::<i32>().unwrap_or(0),
                                "TheirFlags" => record.their_flags = value.parse::<i32>().unwrap_or(0),
                                _ => {}
                            }
                        }
                        text.clear();
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }

        buf.clear();
    }

    records
}

fn finalize_record(records: &mut Vec<FriendRecord>, partial: PartialRecord) {
    if let Some(friend) = partial.friend {
        let id_text = friend
            .split(';')
            .next()
            .unwrap_or_default()
            .trim();
        if let Ok(friend_id) = Uuid::parse_str(id_text) {
            records.push(FriendRecord {
                friend_id,
                my_flags: partial.my_flags,
                their_flags: partial.their_flags,
            });
        }
    }
}

fn is_friend_record(name: &[u8]) -> bool {
    name.starts_with(b"friend")
}

fn local_name(name: &[u8]) -> &[u8] {
    if let Some(position) = name.iter().rposition(|byte| *byte == b':') {
        &name[position + 1..]
    } else {
        name
    }
}

pub struct RemoteFriendsConnector {
    client: reqwest::Client,
    friends_uri: String,
}

impl RemoteFriendsConnector {
    pub fn new(server_uri: &str) -> Self {
        let base = server_uri.trim_end_matches('/');
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            friends_uri: format!("{base}/friends"),
        }
    }

    pub async fn get_friends(
        &self,
        principal: Uuid,
    ) -> Result<Vec<FriendRecord>, RemoteError> {
        let principal_id = principal.to_string();
        let params = [("METHOD", "getfriends"), ("PRINCIPALID", principal_id.as_str())];

        let body = self
            .client
            .post(&self.friends_uri)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;

        Ok(parse_friends(&body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(value: &str) -> Uuid {
        Uuid::parse_str(value).expect("valid test uuid")
    }

    #[test]
    fn parses_two_friends_with_flags() {
        let xml = r#"<ServerResponse>
            <friend0 type="List">
                <PrincipalID>principal</PrincipalID>
                <Friend>11111111-1111-1111-1111-111111111111</Friend>
                <MyFlags>1</MyFlags>
                <TheirFlags>2</TheirFlags>
            </friend0>
            <friend1 type="List">
                <PrincipalID>principal</PrincipalID>
                <Friend>22222222-2222-2222-2222-222222222222</Friend>
                <MyFlags>3</MyFlags>
                <TheirFlags>4</TheirFlags>
            </friend1>
        </ServerResponse>"#;

        let friends = parse_friends(xml);

        assert_eq!(friends.len(), 2);
        assert_eq!(
            friends[0].friend_id,
            uuid("11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(friends[0].my_flags, 1);
        assert_eq!(friends[0].their_flags, 2);
        assert_eq!(
            friends[1].friend_id,
            uuid("22222222-2222-2222-2222-222222222222")
        );
        assert_eq!(friends[1].my_flags, 3);
        assert_eq!(friends[1].their_flags, 4);
    }

    #[test]
    fn null_result_is_empty() {
        let xml = r#"<ServerResponse><result>null</result></ServerResponse>"#;
        assert!(parse_friends(xml).is_empty());
    }

    #[test]
    fn hypergrid_suffix_keeps_uuid() {
        let xml = r#"<ServerResponse>
            <friend0 type="List">
                <Friend>11111111-1111-1111-1111-111111111111;http://host;First Last</Friend>
                <MyFlags>7</MyFlags>
                <TheirFlags>9</TheirFlags>
            </friend0>
        </ServerResponse>"#;

        let friends = parse_friends(xml);
        assert_eq!(friends.len(), 1);
        assert_eq!(
            friends[0].friend_id,
            uuid("11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(friends[0].my_flags, 7);
        assert_eq!(friends[0].their_flags, 9);
    }

    #[test]
    fn invalid_friend_id_is_skipped() {
        let xml = r#"<ServerResponse>
            <friend0 type="List">
                <Friend>not-a-uuid</Friend>
                <MyFlags>1</MyFlags>
                <TheirFlags>2</TheirFlags>
            </friend0>
            <friend1 type="List">
                <Friend>11111111-1111-1111-1111-111111111111</Friend>
                <MyFlags>3</MyFlags>
                <TheirFlags>4</TheirFlags>
            </friend1>
        </ServerResponse>"#;

        let friends = parse_friends(xml);
        assert_eq!(friends.len(), 1);
        assert_eq!(
            friends[0].friend_id,
            uuid("11111111-1111-1111-1111-111111111111")
        );
    }

    #[test]
    fn capitalised_friend_is_not_a_record() {
        let xml = r#"<ServerResponse>
            <Friend type="List">
                <PrincipalID>principal</PrincipalID>
                <Friend>11111111-1111-1111-1111-111111111111</Friend>
                <MyFlags>1</MyFlags>
                <TheirFlags>2</TheirFlags>
            </Friend>
        </ServerResponse>"#;

        assert!(parse_friends(xml).is_empty());
    }

    #[test]
    fn truncated_xml_returns_first_complete_record() {
        let xml = r#"<ServerResponse>
            <friend0 type="List">
                <Friend>11111111-1111-1111-1111-111111111111</Friend>
                <MyFlags>1</MyFlags>
                <TheirFlags>2</TheirFlags>
            </friend0>
            <friend1 type="List">
                <Friend>22222222-2222-2222-2222-222222222222</Friend>
                <MyFlags>3"#;

        let friends = parse_friends(xml);
        assert_eq!(friends.len(), 1);
        assert_eq!(
            friends[0].friend_id,
            uuid("11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(friends[0].my_flags, 1);
        assert_eq!(friends[0].their_flags, 2);
    }
}
