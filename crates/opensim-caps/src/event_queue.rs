// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::VecDeque;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use opensim_llsd::{Llsd, Map, to_xml_string};
use tokio::sync::{Mutex, Notify};

pub struct EventQueue {
    events: Mutex<VecDeque<Vec<u8>>>,
    next_id: AtomicI32,
    notify: Notify,
}

pub enum EventQueueResponse {
    Events { events: Vec<u8>, id: i32 },
    Timeout,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(VecDeque::new()),
            next_id: AtomicI32::new(1),
            notify: Notify::new(),
        }
    }

    pub async fn enqueue(&self, event_name: &str, body: Llsd) {
        let mut event_map = Map::new();
        event_map.insert("message".into(), Llsd::String(event_name.into()));
        event_map.insert("body".into(), body);
        let xml = to_xml_string(&Llsd::Map(event_map));
        self.enqueue_raw(xml.into_bytes()).await;
    }

    pub async fn enqueue_raw(&self, data: Vec<u8>) {
        {
            let mut queue = self.events.lock().await;
            queue.push_back(data);
        }
        self.notify.notify_waiters();
    }

    pub async fn poll(&self, timeout: Duration) -> EventQueueResponse {
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);

        loop {
            {
                let mut queue = self.events.lock().await;
                if !queue.is_empty() {
                    let drained: Vec<Vec<u8>> = queue.drain(..).collect();
                    let id = self.next_id.fetch_add(1, Ordering::Relaxed);
                    let response_body = build_event_response(&drained, id);
                    return EventQueueResponse::Events {
                        events: response_body,
                        id,
                    };
                }
            }

            let notified = self.notify.notified();
            tokio::select! {
                _ = notified => continue,
                _ = &mut deadline => return EventQueueResponse::Timeout,
            }
        }
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

fn build_event_response(event_blobs: &[Vec<u8>], id: i32) -> Vec<u8> {
    let mut events_array = Vec::with_capacity(event_blobs.len());
    for blob in event_blobs {
        match opensim_llsd::from_xml(blob) {
            Ok(parsed) => events_array.push(parsed),
            Err(_) => {
                events_array.push(Llsd::Binary(blob.clone()));
            }
        }
    }

    let mut response = Map::new();
    response.insert("events".into(), Llsd::Array(events_array));
    response.insert("id".into(), Llsd::Integer(id));
    to_xml_string(&Llsd::Map(response)).into_bytes()
}

pub async fn event_queue_handler(
    eq: std::sync::Arc<EventQueue>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    let timeout = Duration::from_secs(58);
    match eq.poll(timeout).await {
        EventQueueResponse::Events { events, .. } => (
            StatusCode::OK,
            [("content-type", "application/llsd+xml")],
            events,
        )
            .into_response(),
        EventQueueResponse::Timeout => StatusCode::BAD_GATEWAY.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_enqueue_and_poll() {
        let eq = EventQueue::new();
        let body = Llsd::Map({
            let mut m = Map::new();
            m.insert("sim_name".into(), Llsd::String("Test Region".into()));
            m
        });
        eq.enqueue("TeleportFinish", body).await;

        let resp = eq.poll(Duration::from_secs(1)).await;
        match resp {
            EventQueueResponse::Events { events, id } => {
                assert_eq!(id, 1);
                let parsed = opensim_llsd::from_xml(&events).unwrap();
                let ev_array = parsed.get("events").as_array();
                assert_eq!(ev_array.len(), 1);
                let first = &ev_array[0];
                assert_eq!(first.get("message").as_str(), "TeleportFinish");
            }
            EventQueueResponse::Timeout => panic!("expected events, got timeout"),
        }
    }

    #[tokio::test]
    async fn test_poll_timeout() {
        let eq = EventQueue::new();
        let resp = eq.poll(Duration::from_millis(10)).await;
        assert!(matches!(resp, EventQueueResponse::Timeout));
    }

    #[tokio::test]
    async fn test_multiple_events() {
        let eq = EventQueue::new();
        for i in 0..3 {
            eq.enqueue(
                &format!("Event{i}"),
                Llsd::Map(Map::new()),
            )
            .await;
        }

        let resp = eq.poll(Duration::from_secs(1)).await;
        match resp {
            EventQueueResponse::Events { events, .. } => {
                let parsed = opensim_llsd::from_xml(&events).unwrap();
                let ev_array = parsed.get("events").as_array();
                assert_eq!(ev_array.len(), 3);
                assert_eq!(ev_array[0].get("message").as_str(), "Event0");
                assert_eq!(ev_array[1].get("message").as_str(), "Event1");
                assert_eq!(ev_array[2].get("message").as_str(), "Event2");
            }
            EventQueueResponse::Timeout => panic!("expected events"),
        }
    }

    #[tokio::test]
    async fn test_event_id_increments() {
        let eq = EventQueue::new();

        eq.enqueue("A", Llsd::Map(Map::new())).await;
        let r1 = eq.poll(Duration::from_secs(1)).await;

        eq.enqueue("B", Llsd::Map(Map::new())).await;
        let r2 = eq.poll(Duration::from_secs(1)).await;

        let id1 = match r1 {
            EventQueueResponse::Events { id, .. } => id,
            _ => panic!("expected events"),
        };
        let id2 = match r2 {
            EventQueueResponse::Events { id, .. } => id,
            _ => panic!("expected events"),
        };
        assert!(id2 > id1);
    }

    #[tokio::test]
    async fn test_concurrent_notify() {
        let eq = Arc::new(EventQueue::new());
        let eq2 = Arc::clone(&eq);

        let poll_handle = tokio::spawn(async move {
            eq2.poll(Duration::from_secs(5)).await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        eq.enqueue("Wakeup", Llsd::Map(Map::new())).await;

        let resp = poll_handle.await.unwrap();
        match resp {
            EventQueueResponse::Events { events, .. } => {
                let parsed = opensim_llsd::from_xml(&events).unwrap();
                let ev_array = parsed.get("events").as_array();
                assert_eq!(ev_array.len(), 1);
                assert_eq!(ev_array[0].get("message").as_str(), "Wakeup");
            }
            EventQueueResponse::Timeout => panic!("expected wakeup event, not timeout"),
        }
    }
}
