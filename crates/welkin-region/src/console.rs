// SPDX-License-Identifier: LGPL-3.0-or-later

use clap::Parser;
use clap::Subcommand;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use welkin_scene::scene::Scene;
use welkin_scene::presence::ScenePresence;

pub enum ConsoleAction {
    Alert(String),
    Kick {
        agent_id: Uuid,
        reason: String,
    },
    Shutdown,
}

pub struct ConsoleContext {
    pub scene: Arc<Scene>,
    pub sim_store: Arc<welkin_data::simulation::SimulationDataStore>,
    pub region_id: String,
    pub config: Arc<welkin_config::Config>,
    pub started: std::time::Instant,
    pub actions: UnboundedSender<ConsoleAction>,
}

#[derive(Parser, Debug)]
#[command(no_binary_name = true, disable_help_subcommand = true)]
struct Cmd {
    #[command(subcommand)]
    sub: Sub,
}

#[derive(Subcommand, Debug)]
enum Sub {
    Help,
    Version,
    Uptime,
    Users,
    User {
        name: String,
    },
    Region,
    Stats,
    Backup,
    Alert {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        message: Vec<String>,
    },
    Kick {
        name: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        reason: Vec<String>,
    },
    Shutdown,
    Quit,
}

#[derive(Clone)]
struct PresenceSummary {
    uuid: Uuid,
    first_name: String,
    last_name: String,
    x: f32,
    y: f32,
    z: f32,
    is_flying: bool,
    is_sitting: bool,
    is_god: bool,
}

fn split_args(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut in_token = false;

    for c in line.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
            in_token = true;
        } else if c.is_whitespace() && !in_quotes {
            if in_token {
                args.push(std::mem::take(&mut current));
                in_token = false;
            }
        } else {
            current.push(c);
            in_token = true;
        }
    }

    if in_token {
        args.push(current);
    }

    args
}

fn parse(line: &str) -> Result<Sub, String> {
    let args = split_args(line);
    let cmd = Cmd::try_parse_from(&args).map_err(|e| e.to_string())?;
    Ok(cmd.sub)
}

fn find_presence(ctx: &ConsoleContext, query: &str) -> Option<PresenceSummary> {
    let query = query.trim().to_lowercase();
    let mut found = None;

    ctx.scene.for_each_presence(|p: &ScenePresence| {
        if found.is_some() {
            return;
        }

        let full_name = format!("{} {}", p.first_name, p.last_name);
        if full_name.to_lowercase() == query || p.first_name.to_lowercase() == query {
            found = Some(PresenceSummary {
                uuid: p.uuid,
                first_name: p.first_name.clone(),
                last_name: p.last_name.clone(),
                x: p.position.x,
                y: p.position.y,
                z: p.position.z,
                is_flying: p.is_flying,
                is_sitting: p.is_sitting,
                is_god: p.is_god,
            });
        }
    });

    found
}

pub async fn execute(ctx: &ConsoleContext, line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let args = split_args(trimmed);
    let cmd = match Cmd::try_parse_from(&args) {
        Ok(cmd) => cmd,
        Err(err) => return err.to_string(),
    };

    match cmd.sub {
        Sub::Help => {
            "help - show this help\n\
             version - show region server version\n\
             uptime - show server uptime\n\
             users - list connected users\n\
             user <name> - show details for a user\n\
             region - show region information\n\
             stats - show region statistics\n\
             backup - persist dirty objects\n\
             alert <message...> - broadcast an alert\n\
             kick <name> [reason...] - kick a user\n\
             shutdown - request server shutdown\n\
             quit - close the console"
                .to_string()
        }
        Sub::Version => env!("CARGO_PKG_VERSION").to_string(),
        Sub::Uptime => {
            let elapsed = ctx.started.elapsed().as_secs();
            let days = elapsed / 86_400;
            let rest = elapsed % 86_400;
            let hours = rest / 3_600;
            let minutes = (rest % 3_600) / 60;
            let seconds = rest % 60;

            if days > 0 {
                format!("up {}d {:02}:{:02}:{:02}", days, hours, minutes, seconds)
            } else {
                format!("up {:02}:{:02}:{:02}", hours, minutes, seconds)
            }
        }
        Sub::Users => {
            let mut lines = Vec::new();
            ctx.scene.for_each_presence(|p: &ScenePresence| {
                lines.push(format!(
                    "{} {} ({:.1}, {:.1}, {:.1}) flying={}",
                    p.first_name, p.last_name, p.position.x, p.position.y, p.position.z, p.is_flying
                ));
            });

            if lines.is_empty() {
                "no users".to_string()
            } else {
                lines.join("\n")
            }
        }
        Sub::User { name } => match find_presence(ctx, &name) {
            None => format!("unknown user: {}", name),
            Some(p) => format!(
                "name: {} {}\nuuid: {}\nposition: {:.1}, {:.1}, {:.1}\nflying: {}\nsitting: {}\ngod: {}",
                p.first_name,
                p.last_name,
                p.uuid,
                p.x,
                p.y,
                p.z,
                p.is_flying,
                p.is_sitting,
                p.is_god
            ),
        },
        Sub::Region => {
            let info = &ctx.scene.region_info;
            format!(
                "{} {} loc {}/{} size {}x{} handle {} {}:{}",
                info.region_name,
                info.region_id,
                info.region_loc_x,
                info.region_loc_y,
                info.region_size_x,
                info.region_size_y,
                info.region_handle,
                info.external_host_name,
                info.internal_port
            )
        }
        Sub::Stats => format!(
            "agents={} objects={} frame_time={}",
            ctx.scene.presence_count(),
            ctx.scene.object_count(),
            ctx.scene.config.frame_time
        ),
        Sub::Backup => {
            let count =
                crate::persist::backup_dirty_objects(&ctx.scene, &ctx.sim_store, &ctx.region_id).await;
            format!("backed up {} objects", count)
        }
        Sub::Alert { message } => {
            let message = message.join(" ");
            let _ = ctx.actions.send(ConsoleAction::Alert(message));
            "alert sent".to_string()
        }
        Sub::Kick { name, reason } => match find_presence(ctx, &name) {
            None => format!("unknown user: {}", name),
            Some(p) => {
                let reason = reason.join(" ");
                let _ = ctx.actions.send(ConsoleAction::Kick {
                    agent_id: p.uuid,
                    reason,
                });
                format!("kicked {} {}", p.first_name, p.last_name)
            }
        },
        Sub::Shutdown => {
            let _ = ctx.actions.send(ConsoleAction::Shutdown);
            "shutdown requested".to_string()
        }
        Sub::Quit => "quit".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_args_quotes_and_empty() {
        assert_eq!(split_args(""), Vec::<String>::new());
        assert_eq!(split_args(r#"a "b c" d"#), vec!["a", "b c", "d"]);
        assert_eq!(split_args(r#""""#), vec![""]);
        assert_eq!(split_args(r#"  "quoted empty"  "#), vec!["quoted empty"]);
    }

    #[test]
    fn unknown_command_is_parse_error() {
        assert!(parse("definitely-not-a-command").is_err());
    }
}
