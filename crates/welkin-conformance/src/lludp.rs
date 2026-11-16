// SPDX-License-Identifier: LGPL-3.0-or-later
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tokio::io::AsyncWriteExt;
use tokio::io::BufWriter;
use tokio::net::UdpSocket;
use tracing::{info, warn};

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
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

pub async fn lludp_record(
    listen_addr: SocketAddr,
    server_addr: SocketAddr,
    output_dir: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    tokio::fs::create_dir_all(&output_dir).await?;

    let start = Instant::now();
    let socket = UdpSocket::bind(listen_addr).await?;

    let filename_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = output_dir.join(format!("lludp_{filename_ts}.jsonl"));
    info!("LLUDP recording on {} -> {}", listen_addr, path.display());

    let file = tokio::fs::File::create(&path).await?;
    let mut writer = BufWriter::new(file);

    let mut client_addr: Option<SocketAddr> = None;
    let mut buf = vec![0u8; 65535];

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Ctrl-C received, stopping recording");
                break;
            }
            res = socket.recv_from(&mut buf) => {
                let (n, src) = res?;
                let data = &buf[..n];
                let ms = start.elapsed().as_millis() as u64;

                let is_client = match client_addr {
                    None => {
                        client_addr = Some(src);
                        info!("Client detected: {src}");
                        true
                    }
                    Some(ca) => src == ca,
                };

                if src == server_addr {
                    if let Some(ca) = client_addr {
                        let _ = socket.send_to(data, ca).await;
                    }
                    let line = format!(
                        r#"{{"ts_ms": {ms}, "dir": "s2c", "data": "{}"}}"#,
                        hex_encode(data)
                    );
                    writer.write_all(line.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                } else if is_client {
                    let _ = socket.send_to(data, server_addr).await;
                    let line = format!(
                        r#"{{"ts_ms": {ms}, "dir": "c2s", "data": "{}"}}"#,
                        hex_encode(data)
                    );
                    writer.write_all(line.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                }
            }
        }
    }

    writer.flush().await?;
    info!("Recording written to {}", path.display());
    Ok(())
}

pub async fn lludp_replay(
    server_addr: SocketAddr,
    recording_path: PathBuf,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let content = tokio::fs::read_to_string(&recording_path).await?;
    let socket = UdpSocket::bind("0.0.0.0:0").await?;

    info!(
        "Replaying {} against {server_addr}",
        recording_path.display()
    );

    let mut matched = 0usize;
    let mut mismatched = 0usize;
    let mut prev_ts: Option<u64> = None;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                warn!("Skipping invalid JSON line: {e}");
                continue;
            }
        };
        let ts_ms = v["ts_ms"].as_u64().unwrap_or(0);
        let dir = v["dir"].as_str().unwrap_or("");
        let data_hex = v["data"].as_str().unwrap_or("");
        let data = match hex_decode(data_hex) {
            Some(d) => d,
            None => {
                warn!("Bad hex data in line, skipping");
                continue;
            }
        };

        if let Some(pt) = prev_ts {
            if ts_ms > pt {
                tokio::time::sleep(tokio::time::Duration::from_millis(ts_ms - pt)).await;
            }
        } else if ts_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(ts_ms)).await;
        }
        prev_ts = Some(ts_ms);

        match dir {
            "c2s" => {
                if let Err(e) = socket.send_to(&data, server_addr).await {
                    warn!("send error: {e}");
                }
            }
            "s2c" => {
                let mut recv_buf = vec![0u8; 65535];
                match tokio::time::timeout(
                    tokio::time::Duration::from_secs(2),
                    socket.recv_from(&mut recv_buf),
                )
                .await
                {
                    Ok(Ok((n, _src))) => {
                        let received = &recv_buf[..n];
                        // Skip LLUDP header bytes 0-6 (flags, sequence, acks)
                        let recorded_body = data.get(7..).unwrap_or(&[]);
                        let received_body = received.get(7..).unwrap_or(&[]);
                        if recorded_body == received_body {
                            matched += 1;
                        } else {
                            mismatched += 1;
                            warn!(
                                "s2c mismatch: recorded len {} received len {}",
                                data.len(),
                                received.len()
                            );
                        }
                    }
                    _ => {
                        mismatched += 1;
                        warn!("s2c timeout or no response");
                    }
                }
            }
            _ => {}
        }
    }

    info!("Replay complete: matched={matched}, mismatched={mismatched}");
    Ok((matched, mismatched))
}
