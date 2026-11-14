use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{error, info, warn};

pub struct BotResult {
    pub connected: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

pub async fn run_pcampbot(
    pcampbot_bin: PathBuf,
    login_uri: String,
    first_name: String,
    last_name: String,
    password: String,
    bot_count: usize,
) -> Result<BotResult, Box<dyn std::error::Error>> {
    info!("Starting pCampBot: {} bots against {login_uri}", bot_count);

    let mut child = Command::new(&pcampbot_bin)
        .arg("-loginuri")
        .arg(&login_uri)
        .arg("-firstname")
        .arg(&first_name)
        .arg("-lastname")
        .arg(&last_name)
        .arg("-password")
        .arg(&password)
        .arg("-n")
        .arg(bot_count.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    let mut errors = Vec::new();

    let stdout_task = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut c = 0usize;
        let mut f = 0usize;
        let mut errs = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.contains("Connected") || line.contains("logged in") {
                c += 1;
                info!("pCampBot: {line}");
            } else if line.contains("Error") || line.contains("FAIL") || line.contains("exception")
            {
                f += 1;
                errs.push(line.clone());
                warn!("pCampBot: {line}");
            }
        }
        (c, f, errs)
    });

    let stderr_task = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        let mut errs = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            errs.push(line.clone());
            error!("pCampBot stderr: {line}");
        }
        errs
    });

    let status = child.wait().await?;
    let (stdout_connected, stdout_failed, stdout_errors) = stdout_task.await?;
    let stderr_errors = stderr_task.await?;

    let connected = stdout_connected;
    let mut failed = stdout_failed;
    errors.extend(stdout_errors);
    errors.extend(stderr_errors);

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        errors.push(format!("pCampBot exited with code {code}"));
        if failed == 0 {
            failed = 1;
        }
    }

    info!(
        "pCampBot finished: connected={connected}, failed={failed}, exit={}",
        status.code().unwrap_or(-1)
    );

    Ok(BotResult {
        connected,
        failed,
        errors,
    })
}
