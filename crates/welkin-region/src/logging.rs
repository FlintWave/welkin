// SPDX-License-Identifier: LGPL-3.0-or-later

use std::io;

use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer, Registry};

pub fn build_directives(level: &str, filter: &str) -> String {
    if filter.trim().is_empty() {
        level.to_string()
    } else {
        format!("{},{}", level, filter)
    }
}

pub fn init(config: &welkin_config::Config) -> Option<WorkerGuard> {
    let directory = config.get_string_or("Logging", "Directory", "logs");
    let file_prefix = config.get_string_or("Logging", "FilePrefix", "region.log");
    let rotation = config.get_string_or("Logging", "Rotation", "daily");
    let max_files = config.get_i64_or("Logging", "MaxFiles", 14);
    let level = config.get_string_or("Logging", "Level", "info");
    let filter = config.get_string_or("Logging", "Filter", "");
    let json = config.get_bool_or("Logging", "Json", false);
    let console = config.get_bool_or("Logging", "Console", true);
    let file = config.get_bool_or("Logging", "File", true);

    let env_filter = if std::env::var_os("RUST_LOG").is_some() {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    } else {
        let directives = build_directives(&level, &filter);
        EnvFilter::try_new(directives).unwrap_or_else(|_| EnvFilter::new("info"))
    };

    let console_layer = if console {
        let layer: Box<dyn Layer<Registry> + Send + Sync> = if json {
            Box::new(fmt::layer().json().with_writer(io::stderr))
        } else {
            Box::new(fmt::layer().with_writer(io::stderr))
        };
        Some(layer)
    } else {
        None
    };

    let mut file_layer = None;
    let mut guard = None;

    if file {
        let rotation = match rotation.to_lowercase().as_str() {
            "hourly" => rolling::Rotation::HOURLY,
            "never" => rolling::Rotation::NEVER,
            _ => rolling::Rotation::DAILY,
        };

        let mut builder = rolling::Builder::new()
            .rotation(rotation)
            .filename_prefix(file_prefix);
        if max_files > 0 {
            builder = builder.max_log_files(max_files as usize);
        }

        let _ = std::fs::create_dir_all(&directory);

        if let Ok(appender) = builder.build(&directory) {
            let (writer, worker_guard) = tracing_appender::non_blocking(appender);
            let layer: Box<dyn Layer<Registry> + Send + Sync> = if json {
                Box::new(fmt::layer().json().with_writer(writer).with_ansi(false))
            } else {
                Box::new(fmt::layer().with_writer(writer).with_ansi(false))
            };
            file_layer = Some(layer);
            guard = Some(worker_guard);
        }
    }

    let layers: Vec<Box<dyn Layer<Registry> + Send + Sync>> =
        console_layer.into_iter().chain(file_layer).collect();
    let _ = tracing_subscriber::registry()
        .with(layers)
        .with(env_filter)
        .try_init();

    guard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_directives_from_config_values() {
        assert_eq!(
            build_directives("info", "welkin_net=debug,welkin_scene=warn"),
            "info,welkin_net=debug,welkin_scene=warn"
        );
    }

    #[test]
    fn builds_directives_with_empty_filter() {
        assert_eq!(build_directives("info", ""), "info");
    }
}
