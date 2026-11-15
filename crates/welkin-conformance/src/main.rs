mod bot_scripting;
pub mod cutover;
mod lludp;
mod pcampbot;
pub mod protocol_replay;
pub mod service_conformance;
pub mod spike;
pub mod trajectory;

use axum::{
    Router,
    extract::{Request, State},
    response::Response,
};
use bytes::Bytes;
use clap::{Parser, Subcommand};
use http_body_util::{BodyExt, Full};
use hyper::header::{HOST, HeaderName, HeaderValue};
use hyper::{HeaderMap, Method, StatusCode, Uri};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process;
use tokio::net::TcpListener;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "osim-conform", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Record {
        #[arg(long)]
        listen: String,
        #[arg(long)]
        target: String,
        #[arg(long, default_value = "recordings")]
        output: PathBuf,
    },
    Replay {
        #[arg(long)]
        target: String,
        #[arg(long)]
        recordings: PathBuf,
        #[arg(long)]
        strict: bool,
    },
    LludpRecord {
        #[arg(long, default_value = "0.0.0.0:9000")]
        listen: SocketAddr,
        #[arg(long)]
        server: SocketAddr,
        #[arg(long, default_value = "recordings")]
        output: PathBuf,
    },
    LludpReplay {
        #[arg(long)]
        server: SocketAddr,
        #[arg(long)]
        recording: PathBuf,
    },
    Bot {
        /// Path to a bot configuration file (JSON)
        #[arg(long)]
        config: PathBuf,
        /// Override the number of bots in the config
        #[arg(long)]
        count: Option<usize>,
        /// Override the action interval (milliseconds)
        #[arg(long)]
        interval: Option<u64>,
    },
    /// Run a programmable bot script.
    BotScript {
        /// Path to a BotScript JSON file.
        #[arg(long)]
        script_path: PathBuf,
        /// Login URI (simulated).
        #[arg(long, default_value = "http://localhost:9000")]
        login_uri: String,
    },
    /// Run service conformance tests against a Robust server.
    ServiceConformance {
        /// Base URL of the target Robust server.
        #[arg(long)]
        target: String,
        /// Path to a directory of recorded responses.
        #[arg(long)]
        recordings_path: Option<PathBuf>,
    },
    PhysicsSpike {
        #[arg(long)]
        recording: PathBuf,
        #[arg(long, default_value = "2.0")]
        tolerance: f32,
    },
}

#[derive(Clone)]
struct AppState {
    target: String,
    output: PathBuf,
    client: hyper_util::client::legacy::Client<
        hyper_util::client::legacy::connect::HttpConnector,
        Full<Bytes>,
    >,
}

#[derive(Debug, Serialize, Deserialize)]
struct CapturedRequest {
    method: String,
    uri: String,
    headers: BTreeMap<String, String>,
    body: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CapturedResponse {
    status: u16,
    headers: BTreeMap<String, String>,
    body: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Exchange {
    timestamp: String,
    request: CapturedRequest,
    response: CapturedResponse,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Record {
            listen,
            target,
            output,
        } => {
            if let Err(err) = run_record(listen, target, output).await {
                error!("record mode failed: {err}");
                process::exit(1);
            }
        }
        Commands::Replay {
            target,
            recordings,
            strict,
        } => {
            run_replay(target, recordings, strict).await;
        }
        Commands::LludpRecord {
            listen,
            server,
            output,
        } => {
            if let Err(e) = lludp::lludp_record(listen, server, output).await {
                error!("LLUDP record failed: {e}");
                process::exit(1);
            }
        }
        Commands::LludpReplay { server, recording } => {
            match lludp::lludp_replay(server, recording).await {
                Ok((matched, mismatched)) => {
                    println!("\nLLUDP: {matched} matched, {mismatched} mismatched");
                    if mismatched > 0 {
                        process::exit(1);
                    }
                }
                Err(e) => {
                    error!("LLUDP replay failed: {e}");
                    process::exit(1);
                }
            }
        }
        Commands::Bot {
            config,
            count,
            interval,
        } => match pcampbot::run_bots(config, count, interval).await {
            Ok(report) => {
                println!(
                    "\npCampBot: {} connected, {} disconnected, {} actions ({} failed), avg {:.1}ms",
                    report.connected,
                    report.disconnected,
                    report.actions_completed,
                    report.actions_failed,
                    report.avg_latency_ms,
                );
                if report.actions_failed > 0 {
                    process::exit(1);
                }
            }
            Err(e) => {
                error!("pCampBot failed: {e}");
                process::exit(1);
            }
        },
        Commands::BotScript {
            script_path,
            login_uri,
        } => match bot_scripting::run_bot_script(script_path, login_uri).await {
            Ok(result) => {
                println!(
                    "\nBotScript: {} run, {} failed, passed={}",
                    result.actions_run, result.actions_failed, result.passed
                );
                for f in &result.failures {
                    println!("  FAIL action {}: {}", f.action_index, f.error);
                }
                if !result.passed {
                    process::exit(1);
                }
            }
            Err(e) => {
                error!("BotScript failed: {e}");
                process::exit(1);
            }
        },
        Commands::ServiceConformance {
            target,
            recordings_path,
        } => {
            let report =
                service_conformance::run_conformance_suite(&target, recordings_path.as_deref());
            println!(
                "\nConformance: {} total, {} passed, {} failed, {} skipped",
                report.total, report.passed, report.failed, report.skipped
            );
            for f in &report.failures {
                println!("  FAIL {:?} ({}): {}", f.endpoint, f.test_name, f.reason);
            }
            if report.failed > 0 {
                process::exit(1);
            }
        }
        Commands::PhysicsSpike {
            recording,
            tolerance,
        } => match spike::run_spike(&recording, tolerance) {
            Ok(report) => {
                for r in &report.results {
                    println!(
                        "actor {}: max_err={:.4} mean_err={:.4} frames={}",
                        r.actor_id, r.max_position_error, r.mean_position_error, r.frame_count
                    );
                }
                println!(
                    "\noverall: {} (tolerance={:.2})",
                    if report.overall_pass { "PASS" } else { "FAIL" },
                    report.tolerance
                );
                if !report.overall_pass {
                    process::exit(1);
                }
            }
            Err(e) => {
                error!("physics spike failed: {e}");
                process::exit(1);
            }
        },
    }
}

async fn run_record(listen: String, target: String, output: PathBuf) -> Result<(), String> {
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http();

    let state = AppState {
        target: target.trim_end_matches('/').to_string(),
        output,
        client,
    };

    let app = Router::new().fallback(proxy).with_state(state);

    let listener = TcpListener::bind(&listen)
        .await
        .map_err(|e| format!("bind failed: {e}"))?;

    info!("recording proxy listening on {listen}");
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("server error: {e}"))?;

    Ok(())
}

async fn proxy(State(state): State<AppState>, req: Request) -> Response {
    let (parts, body) = req.into_parts();

    let req_bytes = body
        .collect()
        .await
        .map(|c| c.to_bytes())
        .unwrap_or_default();

    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/");

    let url = format!("{}{}", state.target, path_and_query);

    let mut out_headers = HeaderMap::new();
    for (name, value) in &parts.headers {
        if name == hyper::header::HOST
            || name == hyper::header::CONNECTION
            || name == hyper::header::CONTENT_LENGTH
            || name == hyper::header::TRANSFER_ENCODING
        {
            continue;
        }
        out_headers.insert(name.clone(), value.clone());
    }

    if let Ok(uri) = url.parse::<Uri>()
        && let Some(auth) = uri.authority()
        && let Ok(host) = HeaderValue::from_str(auth.as_str())
    {
        out_headers.insert(HOST, host);
    }

    let out_req = match hyper::Request::builder()
        .method(parts.method.clone())
        .uri(url.parse::<Uri>().unwrap_or_else(|_| Uri::from_static("/")))
        .body(Full::new(Bytes::from(req_bytes.to_vec())))
    {
        Ok(mut r) => {
            *r.headers_mut() = out_headers;
            r
        }
        Err(e) => {
            error!("failed to build upstream request: {e}");
            let mut r = Response::new(axum::body::Body::from(format!("proxy error: {e}")));
            *r.status_mut() = StatusCode::BAD_GATEWAY;
            return r;
        }
    };

    match state.client.request(out_req).await {
        Ok(resp) => {
            let status = resp.status();
            let resp_headers = resp.headers().clone();

            let resp_bytes = resp
                .into_body()
                .collect()
                .await
                .map(|c| c.to_bytes())
                .unwrap_or_default();

            let timestamp = timestamp_now();
            let exchange = Exchange {
                timestamp: timestamp.clone(),
                request: CapturedRequest {
                    method: parts.method.to_string(),
                    uri: parts.uri.to_string(),
                    headers: capture_headers(&parts.headers),
                    body: bytes_to_field(&req_bytes),
                },
                response: CapturedResponse {
                    status: status.as_u16(),
                    headers: capture_headers(&resp_headers),
                    body: bytes_to_field(&resp_bytes),
                },
            };

            let filename = format!(
                "{}_{}_{}.json",
                timestamp,
                parts.method.as_str(),
                slug(parts.uri.path())
            );

            let _ = tokio::fs::create_dir_all(&state.output).await;
            if let Ok(json) = serde_json::to_string_pretty(&exchange) {
                let _ = tokio::fs::write(state.output.join(filename), json).await;
            }

            let mut response = Response::new(axum::body::Body::from(resp_bytes.to_vec()));
            *response.status_mut() = status;
            *response.headers_mut() = resp_headers;
            response
        }
        Err(e) => {
            error!("upstream request failed: {e}");
            let mut response = Response::new(axum::body::Body::from(format!("proxy error: {e}")));
            *response.status_mut() = StatusCode::BAD_GATEWAY;
            response
        }
    }
}

async fn run_replay(target: String, recordings: PathBuf, strict: bool) {
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http();

    let target = target.trim_end_matches('/').to_string();

    let mut dir = match tokio::fs::read_dir(&recordings).await {
        Ok(d) => d,
        Err(e) => {
            error!("cannot read recordings dir: {e}");
            process::exit(1);
        }
    };

    let mut files = Vec::new();
    while let Ok(Some(entry)) = dir.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            files.push(path);
        }
    }

    let mut loaded: Vec<(PathBuf, Exchange)> = Vec::new();
    let mut failed = 0usize;
    let mut passed = 0usize;

    for path in &files {
        let data = match tokio::fs::read_to_string(path).await {
            Ok(d) => d,
            Err(e) => {
                println!("FAIL: {} - read error: {e}", path.display());
                failed += 1;
                continue;
            }
        };
        match serde_json::from_str::<Exchange>(&data) {
            Ok(ex) => loaded.push((path.clone(), ex)),
            Err(e) => {
                println!("FAIL: {} - invalid JSON: {e}", path.display());
                failed += 1;
            }
        }
    }

    loaded.sort_by(|a, b| a.1.timestamp.cmp(&b.1.timestamp));

    for (path, rec) in &loaded {
        let mut reason: Option<String> = None;

        let req_bytes = field_to_bytes(&rec.request.body);
        let url = format!("{}{}", target, rec.request.uri);

        let method = rec.request.method.parse::<Method>().unwrap_or(Method::GET);

        let uri = match url.parse::<Uri>() {
            Ok(u) => u,
            Err(e) => {
                println!("FAIL: {} - bad URL: {e}", path.display());
                failed += 1;
                continue;
            }
        };

        let mut req = match hyper::Request::builder()
            .method(method.clone())
            .uri(uri)
            .body(Full::new(Bytes::from(req_bytes.clone())))
        {
            Ok(r) => r,
            Err(e) => {
                println!("FAIL: {} - request build error: {e}", path.display());
                failed += 1;
                continue;
            }
        };

        for (name, value) in &rec.request.headers {
            if name.eq_ignore_ascii_case("host")
                || name.eq_ignore_ascii_case("connection")
                || name.eq_ignore_ascii_case("content-length")
                || name.eq_ignore_ascii_case("transfer-encoding")
            {
                continue;
            }
            if let (Ok(hn), Ok(hv)) = (name.parse::<HeaderName>(), value.parse::<HeaderValue>()) {
                req.headers_mut().insert(hn, hv);
            }
        }

        if let Ok(host) = target
            .parse::<Uri>()
            .ok()
            .and_then(|u| u.authority().map(|a| a.as_str().to_string()))
            .unwrap_or_default()
            .parse::<HeaderValue>()
        {
            req.headers_mut().insert(HOST, host);
        }

        match client.request(req).await {
            Ok(resp) => {
                let status = resp.status();
                let actual_headers = resp.headers().clone();

                let resp_bytes = resp
                    .into_body()
                    .collect()
                    .await
                    .map(|c| c.to_bytes())
                    .unwrap_or_default();

                if status.as_u16() != rec.response.status {
                    reason = Some(format!(
                        "status mismatch: expected {}, got {}",
                        rec.response.status,
                        status.as_u16()
                    ));
                } else {
                    let rec_ct = get_header(&rec.response.headers, "content-type");
                    let actual_ct = actual_headers
                        .get(hyper::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();

                    let rec_body = field_to_bytes(&rec.response.body);
                    if !bodies_equal(&rec_ct, &actual_ct, &rec_body, &resp_bytes) {
                        reason = Some("response body mismatch".to_string());
                    } else if strict {
                        let rec_headers = normalize_headers_map(&rec.response.headers);
                        let actual_map = capture_headers(&actual_headers);
                        let rec_filtered = filter_ignored_headers(&rec_headers);
                        let actual_filtered = filter_ignored_headers(&actual_map);
                        if rec_filtered != actual_filtered {
                            reason = Some("response headers mismatch".to_string());
                        }
                    }
                }
            }
            Err(e) => {
                reason = Some(format!("request failed: {e}"));
            }
        }

        if let Some(r) = reason {
            println!("FAIL: {} - {r}", path.display());
            failed += 1;
        } else {
            println!("PASS: {}", path.display());
            passed += 1;
        }
    }

    println!("\n{passed} passed, {failed} failed");
    if failed > 0 {
        process::exit(1);
    }
}

fn capture_headers(headers: &HeaderMap) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (name, value) in headers {
        map.insert(name.to_string(), value.to_str().unwrap_or("").to_string());
    }
    map
}

fn bytes_to_field(bytes: &[u8]) -> String {
    match String::from_utf8(bytes.to_vec()) {
        Ok(s) => s,
        Err(_) => format!("hex:{}", hex_encode(bytes)),
    }
}

fn field_to_bytes(s: &str) -> Vec<u8> {
    if let Some(hex) = s.strip_prefix("hex:") {
        hex_decode(hex).unwrap_or_default()
    } else {
        s.as_bytes().to_vec()
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        out.push(u8::from_str_radix(&s[i..i + 2], 16).ok()?);
    }
    Some(out)
}

fn timestamp_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let (y, m, d) = civil_from_days(secs.div_euclid(86400));
    let rem = secs.rem_euclid(86400) as u32;
    let hh = rem / 3600;
    let mm = (rem / 60) % 60;
    let ss = rem % 60;

    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

fn slug(path: &str) -> String {
    let s: String = path
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if s.is_empty() {
        "_".to_string()
    } else {
        s.chars().take(60).collect()
    }
}

fn get_header(headers: &BTreeMap<String, String>, name: &str) -> String {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn bodies_equal(rec_ct: &str, actual_ct: &str, rec_bytes: &[u8], actual_bytes: &[u8]) -> bool {
    let structural = rec_ct.to_ascii_lowercase().contains("xml")
        || rec_ct.to_ascii_lowercase().contains("json")
        || actual_ct.to_ascii_lowercase().contains("xml")
        || actual_ct.to_ascii_lowercase().contains("json");

    if !structural {
        return rec_bytes == actual_bytes;
    }

    if (rec_ct.to_ascii_lowercase().contains("json")
        || actual_ct.to_ascii_lowercase().contains("json"))
        && let (Ok(a), Ok(b)) = (
            serde_json::from_slice::<serde_json::Value>(rec_bytes),
            serde_json::from_slice::<serde_json::Value>(actual_bytes),
        )
    {
        return a == b;
    }

    if rec_ct.to_ascii_lowercase().contains("xml") || actual_ct.to_ascii_lowercase().contains("xml")
    {
        return normalize_xml(&String::from_utf8_lossy(rec_bytes))
            == normalize_xml(&String::from_utf8_lossy(actual_bytes));
    }

    rec_bytes == actual_bytes
}

fn normalize_xml(input: &str) -> String {
    let mut s = input.trim().to_string();

    if s.starts_with("<?xml")
        && let Some(end) = s.find("?>")
    {
        s = s[end + 2..].trim().to_string();
    }

    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
            result.push(ch);
            continue;
        }
        if ch == '>' {
            in_tag = false;
            result.push(ch);
            continue;
        }
        if in_tag {
            result.push(ch);
        } else if ch.is_whitespace() {
            if !result.ends_with('>') && !result.ends_with(' ') {
                result.push(' ');
            }
        } else {
            result.push(ch);
        }
    }

    result.replace("> <", "><").trim().to_string()
}

fn normalize_headers_map(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (k, v) in headers {
        out.insert(k.to_ascii_lowercase(), v.clone());
    }
    out
}

fn filter_ignored_headers(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let ignored = ["date", "server", "x-request-id"];
    headers
        .iter()
        .filter(|(k, _)| !ignored.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
