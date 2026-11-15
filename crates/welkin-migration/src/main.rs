// SPDX-License-Identifier: LGPL-3.0-or-later

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "osim-migrate",
    about = "OpenSimulator migration toolkit (C# → Rust)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a database schema against the expected Rust schema
    ValidateDb {
        /// Database connection string (mysql://, postgres://, or sqlite://)
        #[arg(long)]
        connection: String,

        /// Generate ALTER TABLE statements for fixable differences
        #[arg(long, default_value_t = false)]
        generate_alters: bool,
    },

    /// Upgrade password hashes from MD5 to Argon2id
    UpgradePasswords {
        /// Database connection string
        #[arg(long)]
        connection: String,

        /// Number of rows to process per batch
        #[arg(long, default_value_t = 100)]
        batch_size: usize,

        /// Print what would be done without changing anything
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Keep the original MD5 hash in a backup column
        #[arg(long, default_value_t = false)]
        rollback: bool,
    },

    /// Convert a C# OpenSim INI configuration to Rust-compatible TOML
    ConvertConfig {
        /// Path to the C# INI file (e.g. OpenSim.ini or Robust.ini)
        #[arg(long)]
        input: String,

        /// Output path (TOML). Prints to stdout if omitted.
        #[arg(long)]
        output: Option<String>,
    },
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

    let result: Result<(), Box<dyn std::error::Error>> = rt.block_on(async {
        match cli.command {
            Commands::ValidateDb {
                connection,
                generate_alters,
            } => {
                let report = welkin_migration::db_validator::validate_schema(&connection).await?;
                println!("{report}");
                if generate_alters {
                    let alters = welkin_migration::db_validator::generate_alter_statements(&report);
                    if alters.is_empty() {
                        println!("\nNo ALTER statements needed.");
                    } else {
                        println!("\n-- Generated ALTER statements:");
                        for stmt in &alters {
                            println!("{stmt}");
                        }
                    }
                }
                Ok(())
            }
            Commands::UpgradePasswords {
                connection,
                batch_size,
                dry_run,
                rollback,
            } => {
                let config = welkin_migration::password_upgrade::PasswordUpgradeConfig {
                    connection_string: connection,
                    batch_size,
                    dry_run,
                    rollback,
                };
                let report = welkin_migration::password_upgrade::upgrade_passwords(&config).await?;
                println!("{report}");
                Ok(())
            }
            Commands::ConvertConfig { input, output } => {
                let ini_content = std::fs::read_to_string(&input)?;
                let report = welkin_migration::config_converter::convert_config(&ini_content)?;
                let toml_output = report.to_toml();
                match output {
                    Some(path) => {
                        std::fs::write(&path, &toml_output)?;
                        println!("Written to {path}");
                    }
                    None => {
                        print!("{toml_output}");
                    }
                }
                if !report.skipped.is_empty() {
                    eprintln!("\nSkipped (unsupported):");
                    for s in &report.skipped {
                        eprintln!("  {s}");
                    }
                }
                if !report.warnings.is_empty() {
                    eprintln!("\nWarnings (deprecated):");
                    for w in &report.warnings {
                        eprintln!("  {w}");
                    }
                }
                Ok(())
            }
        }
    });

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
