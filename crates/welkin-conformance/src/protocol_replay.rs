// SPDX-License-Identifier: GPL-3.0-or-later

//! Full protocol replay infrastructure.
//!
//! Builds on the recording / replay concepts in `main.rs` and `lludp.rs`,
//! adding structured diff analysis with divergence categorisation so
//! conformance reports can separate real bugs from cosmetic noise.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Recorded exchange
// ---------------------------------------------------------------------------

/// A single recorded HTTP request / response pair with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedExchange {
    pub request: RecordedRequest,
    pub response: RecordedResponse,
    pub timestamp: String,
    pub labels: Vec<String>,
}

/// The request half of a [`RecordedExchange`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

/// The response half of a [`RecordedExchange`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

// ---------------------------------------------------------------------------
// Diff analysis
// ---------------------------------------------------------------------------

/// The result of comparing a recorded response with an actual one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiffResult {
    /// Responses are byte-identical (after normalisation).
    Identical,
    /// A field's content differs.
    ContentDifference {
        field: String,
        expected: String,
        actual: String,
    },
    /// The HTTP status code differs.
    StatusDifference { expected: u16, actual: u16 },
    /// A required field is missing from the actual response.
    MissingField { name: String },
    /// The actual response contains a field that was not recorded.
    ExtraField { name: String },
}

/// Category assigned to a divergence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DivergenceCategory {
    /// A genuine functional bug.
    Bug,
    /// A deliberate change from the reference implementation.
    Intentional,
    /// A difference that does not affect correctness (e.g. whitespace, dates).
    Cosmetic,
}

/// Rules used by [`categorize_divergence`] to decide how to classify a diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorizationRules {
    /// Header names whose differences are always Cosmetic.
    pub cosmetic_headers: Vec<String>,
    /// Header names whose differences are always Intentional.
    pub intentional_headers: Vec<String>,
    /// Body field paths whose differences are Cosmetic.
    pub cosmetic_body_fields: Vec<String>,
}

impl Default for CategorizationRules {
    fn default() -> Self {
        Self {
            cosmetic_headers: vec![
                "date".to_string(),
                "server".to_string(),
                "x-request-id".to_string(),
            ],
            intentional_headers: vec!["x-opensim-version".to_string()],
            cosmetic_body_fields: vec!["timestamp".to_string(), "created".to_string()],
        }
    }
}

/// Classify a [`DiffResult`] into a [`DivergenceCategory`].
pub fn categorize_divergence(diff: &DiffResult, rules: &CategorizationRules) -> DivergenceCategory {
    match diff {
        DiffResult::Identical => DivergenceCategory::Cosmetic,
        DiffResult::StatusDifference { .. } => DivergenceCategory::Bug,
        DiffResult::MissingField { .. } => DivergenceCategory::Bug,
        DiffResult::ExtraField { name } => {
            if rules
                .cosmetic_headers
                .iter()
                .any(|h| h.eq_ignore_ascii_case(name))
            {
                DivergenceCategory::Cosmetic
            } else {
                DivergenceCategory::Intentional
            }
        }
        DiffResult::ContentDifference { field, .. } => {
            let lower = field.to_ascii_lowercase();
            if rules
                .cosmetic_headers
                .iter()
                .any(|h| h.eq_ignore_ascii_case(&lower))
            {
                return DivergenceCategory::Cosmetic;
            }
            if rules
                .cosmetic_body_fields
                .iter()
                .any(|f| f.eq_ignore_ascii_case(&lower))
            {
                return DivergenceCategory::Cosmetic;
            }
            if rules
                .intentional_headers
                .iter()
                .any(|h| h.eq_ignore_ascii_case(&lower))
            {
                return DivergenceCategory::Intentional;
            }
            DivergenceCategory::Bug
        }
    }
}

// ---------------------------------------------------------------------------
// Replay report
// ---------------------------------------------------------------------------

/// A divergence entry in the replay report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivergenceEntry {
    pub exchange_index: usize,
    pub diff: DiffResult,
    pub category: DivergenceCategory,
}

/// Summary report from a replay session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayReport {
    pub total_exchanges: usize,
    pub identical: usize,
    pub bugs: usize,
    pub intentional: usize,
    pub cosmetic: usize,
    pub errors: usize,
    pub divergences: Vec<DivergenceEntry>,
}

impl ReplayReport {
    fn new() -> Self {
        Self {
            total_exchanges: 0,
            identical: 0,
            bugs: 0,
            intentional: 0,
            cosmetic: 0,
            errors: 0,
            divergences: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Replay session
// ---------------------------------------------------------------------------

/// Loads recorded exchanges from a directory and replays them against a
/// target server, producing a [`ReplayReport`].
pub struct ReplaySession {
    pub exchanges: Vec<RecordedExchange>,
    pub rules: CategorizationRules,
}

impl ReplaySession {
    /// Load all `.json` exchange files from a directory.
    pub fn load(dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut exchanges = Vec::new();

        let entries = std::fs::read_dir(dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let data = std::fs::read_to_string(&path)?;
            match serde_json::from_str::<RecordedExchange>(&data) {
                Ok(ex) => exchanges.push(ex),
                Err(e) => {
                    warn!("Skipping {}: {e}", path.display());
                }
            }
        }

        exchanges.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(Self {
            exchanges,
            rules: CategorizationRules::default(),
        })
    }

    /// Load from a vec of exchanges (useful in tests).
    pub fn from_exchanges(exchanges: Vec<RecordedExchange>) -> Self {
        Self {
            exchanges,
            rules: CategorizationRules::default(),
        }
    }

    /// Compare two responses and produce a list of diffs.
    pub fn compare_responses(
        recorded: &RecordedResponse,
        actual: &RecordedResponse,
    ) -> Vec<DiffResult> {
        let mut diffs = Vec::new();

        if recorded.status != actual.status {
            diffs.push(DiffResult::StatusDifference {
                expected: recorded.status,
                actual: actual.status,
            });
        }

        // Header comparison
        for (name, expected_val) in &recorded.headers {
            match actual.headers.get(name) {
                Some(actual_val) => {
                    if expected_val != actual_val {
                        diffs.push(DiffResult::ContentDifference {
                            field: name.clone(),
                            expected: expected_val.clone(),
                            actual: actual_val.clone(),
                        });
                    }
                }
                None => {
                    diffs.push(DiffResult::MissingField { name: name.clone() });
                }
            }
        }

        for name in actual.headers.keys() {
            if !recorded.headers.contains_key(name) {
                diffs.push(DiffResult::ExtraField { name: name.clone() });
            }
        }

        // Body comparison (string-level)
        if recorded.body != actual.body {
            diffs.push(DiffResult::ContentDifference {
                field: "body".to_string(),
                expected: truncate(&recorded.body, 200),
                actual: truncate(&actual.body, 200),
            });
        }

        if diffs.is_empty() {
            diffs.push(DiffResult::Identical);
        }

        diffs
    }

    /// Replay all exchanges against an actual set of responses (offline mode).
    pub fn replay_offline(&self, actual_responses: &[RecordedResponse]) -> ReplayReport {
        let mut report = ReplayReport::new();
        report.total_exchanges = self.exchanges.len();

        for (i, exchange) in self.exchanges.iter().enumerate() {
            if let Some(actual) = actual_responses.get(i) {
                let diffs = Self::compare_responses(&exchange.response, actual);
                let mut is_identical = true;

                for diff in &diffs {
                    if *diff == DiffResult::Identical {
                        continue;
                    }
                    is_identical = false;
                    let category = categorize_divergence(diff, &self.rules);
                    match category {
                        DivergenceCategory::Bug => report.bugs += 1,
                        DivergenceCategory::Intentional => report.intentional += 1,
                        DivergenceCategory::Cosmetic => report.cosmetic += 1,
                    }
                    report.divergences.push(DivergenceEntry {
                        exchange_index: i,
                        diff: diff.clone(),
                        category,
                    });
                }

                if is_identical {
                    report.identical += 1;
                }
            } else {
                report.errors += 1;
            }
        }

        report
    }
}

// ---------------------------------------------------------------------------
// LLUDP replay extensions
// ---------------------------------------------------------------------------

/// A recorded LLUDP packet with sequence tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LludpRecordedPacket {
    pub timestamp_ms: u64,
    pub direction: String,
    pub sequence_number: u32,
    pub data: String,
    pub message_id: Option<u16>,
}

/// Compare two LLUDP packets, skipping the header region.
pub fn compare_lludp_packets(
    recorded: &LludpRecordedPacket,
    actual: &LludpRecordedPacket,
    header_skip_bytes: usize,
) -> DiffResult {
    let rec_data = recorded.data.as_bytes();
    let act_data = actual.data.as_bytes();

    let rec_body = rec_data.get(header_skip_bytes..).unwrap_or(&[]);
    let act_body = act_data.get(header_skip_bytes..).unwrap_or(&[]);

    if rec_body == act_body {
        DiffResult::Identical
    } else {
        DiffResult::ContentDifference {
            field: "lludp_body".to_string(),
            expected: format!("{} bytes", rec_body.len()),
            actual: format!("{} bytes", act_body.len()),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_exchange(status: u16, body: &str, labels: &[&str]) -> RecordedExchange {
        RecordedExchange {
            request: RecordedRequest {
                method: "POST".to_string(),
                path: "/test".to_string(),
                headers: BTreeMap::new(),
                body: String::new(),
            },
            response: RecordedResponse {
                status,
                headers: BTreeMap::new(),
                body: body.to_string(),
            },
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            labels: labels.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn make_response(status: u16, body: &str) -> RecordedResponse {
        RecordedResponse {
            status,
            headers: BTreeMap::new(),
            body: body.to_string(),
        }
    }

    #[test]
    fn test_identical_responses() {
        let recorded = make_response(200, "<result>ok</result>");
        let actual = make_response(200, "<result>ok</result>");
        let diffs = ReplaySession::compare_responses(&recorded, &actual);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0], DiffResult::Identical);
    }

    #[test]
    fn test_status_difference() {
        let recorded = make_response(200, "ok");
        let actual = make_response(500, "ok");
        let diffs = ReplaySession::compare_responses(&recorded, &actual);
        assert!(diffs.iter().any(|d| matches!(d, DiffResult::StatusDifference { expected: 200, actual: 500 })));
    }

    #[test]
    fn test_body_difference() {
        let recorded = make_response(200, "expected body");
        let actual = make_response(200, "actual body");
        let diffs = ReplaySession::compare_responses(&recorded, &actual);
        assert!(diffs.iter().any(|d| matches!(d, DiffResult::ContentDifference { field, .. } if field == "body")));
    }

    #[test]
    fn test_missing_header() {
        let mut recorded = make_response(200, "ok");
        recorded
            .headers
            .insert("x-custom".to_string(), "value".to_string());
        let actual = make_response(200, "ok");
        let diffs = ReplaySession::compare_responses(&recorded, &actual);
        assert!(diffs.iter().any(|d| matches!(d, DiffResult::MissingField { name } if name == "x-custom")));
    }

    #[test]
    fn test_extra_header() {
        let recorded = make_response(200, "ok");
        let mut actual = make_response(200, "ok");
        actual
            .headers
            .insert("x-extra".to_string(), "value".to_string());
        let diffs = ReplaySession::compare_responses(&recorded, &actual);
        assert!(diffs.iter().any(|d| matches!(d, DiffResult::ExtraField { name } if name == "x-extra")));
    }

    #[test]
    fn test_categorize_status_as_bug() {
        let diff = DiffResult::StatusDifference {
            expected: 200,
            actual: 500,
        };
        let cat = categorize_divergence(&diff, &CategorizationRules::default());
        assert_eq!(cat, DivergenceCategory::Bug);
    }

    #[test]
    fn test_categorize_cosmetic_header() {
        let diff = DiffResult::ContentDifference {
            field: "date".to_string(),
            expected: "Mon, 01 Jan 2025".to_string(),
            actual: "Tue, 02 Jan 2025".to_string(),
        };
        let cat = categorize_divergence(&diff, &CategorizationRules::default());
        assert_eq!(cat, DivergenceCategory::Cosmetic);
    }

    #[test]
    fn test_categorize_intentional_header() {
        let diff = DiffResult::ContentDifference {
            field: "x-opensim-version".to_string(),
            expected: "0.9.2".to_string(),
            actual: "0.9.3".to_string(),
        };
        let cat = categorize_divergence(&diff, &CategorizationRules::default());
        assert_eq!(cat, DivergenceCategory::Intentional);
    }

    #[test]
    fn test_replay_offline_all_identical() {
        let ex1 = make_exchange(200, "ok", &["test"]);
        let ex2 = make_exchange(200, "fine", &["test"]);
        let session = ReplaySession::from_exchanges(vec![ex1, ex2]);
        let actuals = vec![make_response(200, "ok"), make_response(200, "fine")];
        let report = session.replay_offline(&actuals);
        assert_eq!(report.total_exchanges, 2);
        assert_eq!(report.identical, 2);
        assert_eq!(report.bugs, 0);
    }

    #[test]
    fn test_replay_offline_with_divergence() {
        let ex1 = make_exchange(200, "expected", &[]);
        let session = ReplaySession::from_exchanges(vec![ex1]);
        let actuals = vec![make_response(500, "error")];
        let report = session.replay_offline(&actuals);
        assert_eq!(report.total_exchanges, 1);
        assert_eq!(report.identical, 0);
        assert!(report.bugs > 0);
    }

    #[test]
    fn test_replay_offline_missing_actual() {
        let ex1 = make_exchange(200, "ok", &[]);
        let session = ReplaySession::from_exchanges(vec![ex1]);
        let actuals: Vec<RecordedResponse> = vec![];
        let report = session.replay_offline(&actuals);
        assert_eq!(report.errors, 1);
    }

    #[test]
    fn test_lludp_packet_compare_identical() {
        let p1 = LludpRecordedPacket {
            timestamp_ms: 0,
            direction: "c2s".to_string(),
            sequence_number: 1,
            data: "HEADER_BODY_CONTENT".to_string(),
            message_id: Some(1),
        };
        let p2 = p1.clone();
        let result = compare_lludp_packets(&p1, &p2, 7);
        assert_eq!(result, DiffResult::Identical);
    }

    #[test]
    fn test_lludp_packet_compare_different() {
        let p1 = LludpRecordedPacket {
            timestamp_ms: 0,
            direction: "c2s".to_string(),
            sequence_number: 1,
            data: "HEADER_BODY_AAA".to_string(),
            message_id: Some(1),
        };
        let p2 = LludpRecordedPacket {
            timestamp_ms: 0,
            direction: "c2s".to_string(),
            sequence_number: 2,
            data: "HEADER_BODY_BBB".to_string(),
            message_id: Some(1),
        };
        let result = compare_lludp_packets(&p1, &p2, 7);
        assert!(matches!(result, DiffResult::ContentDifference { .. }));
    }

    #[test]
    fn test_recorded_exchange_serde_roundtrip() {
        let ex = make_exchange(200, "test body", &["label1", "label2"]);
        let json = serde_json::to_string(&ex).expect("serialize");
        let restored: RecordedExchange = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.response.status, 200);
        assert_eq!(restored.labels.len(), 2);
    }

    #[test]
    fn test_categorization_rules_default() {
        let rules = CategorizationRules::default();
        assert!(rules.cosmetic_headers.contains(&"date".to_string()));
        assert!(rules
            .intentional_headers
            .contains(&"x-opensim-version".to_string()));
    }

    #[test]
    fn test_truncate_helper() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("a long string here", 6), "a long...");
    }
}
