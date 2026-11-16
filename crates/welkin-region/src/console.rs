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
    LoadOar(String),
    Shutdown,
}

pub struct ConsoleContext {
    pub scene: Arc<Scene>,
    pub sim_store: Arc<welkin_data::simulation::SimulationDataStore>,
    pub region_id: String,
    pub started: std::time::Instant,
    pub actions: UnboundedSender<ConsoleAction>,
    pub grid_uri: String,
    pub http: reqwest::Client,
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
    /// `load oar <file>`: load an OpenSim region archive.
    Load {
        kind: String,
        path: String,
    },
    Quit,
    #[command(name = "create-user")]
    CreateUser {
        first: String,
        last: String,
        password: String,
        email: Option<String>,
    },
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

#[cfg(test)]
fn parse(line: &str) -> Result<Sub, String> {
    let args = split_args(line);
    let cmd = Cmd::try_parse_from(&args).map_err(|e| e.to_string())?;
    Ok(cmd.sub)
}

pub fn pick_match(query: &str, names: &[(String, String)]) -> Result<usize, String> {
    let query_trimmed = query.trim();
    let query_lower = query_trimmed.to_lowercase();

    let full_matches: Vec<usize> = names
        .iter()
        .enumerate()
        .filter(|(_, (first, last))| {
            format!("{} {}", first, last).to_lowercase() == query_lower
        })
        .map(|(index, _)| index)
        .collect();

    if !full_matches.is_empty() {
        if full_matches.len() == 1 {
            return Ok(full_matches[0]);
        }

        let matched: Vec<String> = full_matches
            .iter()
            .map(|&index| format!("{} {}", names[index].0, names[index].1))
            .collect();
        return Err(format!(
            "ambiguous name '{}': matches {}",
            query,
            matched.join(", ")
        ));
    }

    let first_matches: Vec<usize> = names
        .iter()
        .enumerate()
        .filter(|(_, (first, _))| first.to_lowercase() == query_lower)
        .map(|(index, _)| index)
        .collect();

    if !first_matches.is_empty() {
        if first_matches.len() == 1 {
            return Ok(first_matches[0]);
        }

        let matched: Vec<String> = first_matches
            .iter()
            .map(|&index| format!("{} {}", names[index].0, names[index].1))
            .collect();
        return Err(format!(
            "ambiguous name '{}': matches {}",
            query,
            matched.join(", ")
        ));
    }

    Err(format!("no user named '{}'", query))
}

fn find_presence(ctx: &ConsoleContext, query: &str) -> Result<PresenceSummary, String> {
    let mut names = Vec::new();
    let mut summaries = Vec::new();

    ctx.scene.for_each_presence(|p: &ScenePresence| {
        names.push((p.first_name.clone(), p.last_name.clone()));
        summaries.push(PresenceSummary {
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
    });

    let index = pick_match(query, &names)?;
    Ok(summaries[index].clone())
}

pub fn validate_new_user(first: &str, last: &str, password: &str) -> Result<(), String> {
    if first.is_empty() || first.chars().any(char::is_whitespace) {
        return Err("error: first name must be non-empty and contain no whitespace".to_string());
    }
    if last.is_empty() || last.chars().any(char::is_whitespace) {
        return Err("error: last name must be non-empty and contain no whitespace".to_string());
    }
    if password.chars().count() < 8 {
        return Err("error: password must be at least 8 characters".to_string());
    }
    Ok(())
}

pub fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].to_string())
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
             quit - close the console\n\
             create-user <first> <last> <password> [email] - create a grid user account"
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
            Err(err) => err,
            Ok(p) => format!(
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
            let terrain = crate::persist::backup_terrain(&ctx.scene, &ctx.sim_store, &ctx.region_id).await;
            if terrain {
                format!("backed up {} objects and the terrain", count)
            } else {
                format!("backed up {} objects", count)
            }
        }
        Sub::Alert { message } => {
            let message = message.join(" ");
            let n = ctx.scene.presence_count();
            if n == 0 {
                "no clients connected; alert not sent".to_string()
            } else {
                let _ = ctx.actions.send(ConsoleAction::Alert(message));
                format!("alert queued for {} connected client(s)", n)
            }
        }
        Sub::Kick { name, reason } => match find_presence(ctx, &name) {
            Err(err) => err,
            Ok(p) => {
                let reason = reason.join(" ");
                let _ = ctx.actions.send(ConsoleAction::Kick {
                    agent_id: p.uuid,
                    reason,
                });
                format!("kick queued for {} {}", p.first_name, p.last_name)
            }
        },
        Sub::Shutdown => {
            let _ = ctx.actions.send(ConsoleAction::Shutdown);
            "shutdown requested".to_string()
        }
        Sub::Load { kind, path } => {
            if kind.eq_ignore_ascii_case("oar") {
                let _ = ctx.actions.send(ConsoleAction::LoadOar(path.clone()));
                format!("loading OAR {path} (progress is logged)")
            } else {
                format!("unknown load target '{kind}'; supported: oar")
            }
        }
        Sub::Quit => "quit".to_string(),
        Sub::CreateUser {
            first,
            last,
            password,
            email,
        } => {
            if let Err(err) = validate_new_user(&first, &last, &password) {
                return err;
            }
            if ctx.grid_uri.trim().is_empty() {
                return "error: no grid service URI configured".to_string();
            }

            let url = format!("{}/accounts", ctx.grid_uri.trim_end_matches('/'));
            let form = vec![
                ("METHOD", "createuser"),
                ("FirstName", first.as_str()),
                ("LastName", last.as_str()),
                ("Password", password.as_str()),
                ("Email", email.as_deref().unwrap_or("")),
            ];

            let response = match ctx.http.post(&url).form(&form).send().await {
                Ok(response) => response,
                Err(err) => {
                    return format!("error: grid service unreachable: {err}");
                }
            };

            let text = match response.text().await {
                Ok(text) => text,
                Err(err) => {
                    return format!("error: grid service unreachable: {err}");
                }
            };

            if text.contains("<result>Failure</result>") {
                return "error: grid refused to create the user (name already exists or invalid data)"
                    .to_string();
            }

            match extract_tag(&text, "PrincipalID") {
                Some(id) => format!("created user {first} {last} ({id})"),
                None => "error: grid response missing PrincipalID".to_string(),
            }
        }
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

    #[test]
    fn pick_match_exact_full_name_wins_over_first_name_match() {
        let names = vec![
            ("Alice".to_string(), "Smith".to_string()),
            ("Alice".to_string(), "Jones".to_string()),
        ];

        assert_eq!(pick_match("alice smith", &names), Ok(0));
    }

    #[test]
    fn pick_match_ambiguous_first_name_errors() {
        let names = vec![
            ("Alice".to_string(), "Smith".to_string()),
            ("Alice".to_string(), "Jones".to_string()),
        ];

        assert_eq!(
            pick_match("alice", &names),
            Err("ambiguous name 'alice': matches Alice Smith, Alice Jones".to_string())
        );
    }

    #[test]
    fn pick_match_none_errors() {
        let names = vec![("Alice".to_string(), "Smith".to_string())];

        assert_eq!(
            pick_match("charlie", &names),
            Err("no user named 'charlie'".to_string())
        );
    }

    #[test]
    fn extract_tag_found() {
        assert_eq!(
            extract_tag("<PrincipalID>abc-123</PrincipalID>", "PrincipalID"),
            Some("abc-123".to_string())
        );
    }

    #[test]
    fn extract_tag_not_found() {
        assert_eq!(extract_tag("<result>Success</result>", "PrincipalID"), None);
    }

    #[test]
    fn validate_new_user_accepts_valid() {
        assert!(validate_new_user("Alice", "Smith", "password123").is_ok());
    }

    #[test]
    fn validate_new_user_rejects_invalid() {
        assert!(validate_new_user("", "Smith", "password123").is_err());
        assert!(validate_new_user("A lice", "Smith", "password123").is_err());
        assert!(validate_new_user("Alice", "", "password123").is_err());
        assert!(validate_new_user("Alice", "Smith", "short").is_err());
    }
}

