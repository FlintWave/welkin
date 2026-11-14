// SPDX-License-Identifier: GPL-3.0-or-later

//! Password upgrade tool: migrates OpenSimulator auth table from MD5 to Argon2id.
//!
//! C# OpenSim stores `MD5(password + ":" + salt)` in the `passwordHash` column and
//! the salt in `passwordSalt`. After upgrade the chain becomes:
//! viewer sends `MD5(pass)` → server verifies `Argon2id(MD5(pass))`.

use std::fmt;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand_core::OsRng;
use argon2::Argon2;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum PasswordError {
    /// Argon2 hashing or verification failed.
    Argon2(String),
    /// The stored hash string could not be parsed.
    InvalidHash(String),
    /// A configuration value is out of range.
    Config(String),
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Argon2(msg) => write!(f, "argon2 error: {msg}"),
            Self::InvalidHash(msg) => write!(f, "invalid hash: {msg}"),
            Self::Config(msg) => write!(f, "config error: {msg}"),
        }
    }
}

impl std::error::Error for PasswordError {}

// ---------------------------------------------------------------------------
// Config & data types
// ---------------------------------------------------------------------------

/// Configuration for a password-upgrade run.
#[derive(Debug, Clone)]
pub struct PasswordUpgradeConfig {
    /// Database connection string (not used directly here; the caller connects).
    pub connection_string: String,
    /// How many rows to process in each batch.
    pub batch_size: usize,
    /// When `true`, report what *would* be done without writing anything.
    pub dry_run: bool,
    /// When `true`, keep the original MD5 hash in a backup column.
    pub rollback: bool,
}

impl PasswordUpgradeConfig {
    pub fn validate(&self) -> Result<(), PasswordError> {
        if self.batch_size == 0 {
            return Err(PasswordError::Config(
                "batch_size must be greater than zero".into(),
            ));
        }
        Ok(())
    }
}

/// One row from the `auth` table.
#[derive(Debug, Clone)]
pub struct PasswordRecord {
    pub uuid: String,
    pub password_hash: String,
    pub password_salt: String,
}

/// Outcome of upgrading a single record.
#[derive(Debug, Clone)]
pub struct UpgradedRecord {
    pub uuid: String,
    /// The new Argon2id hash string (PHC format).
    pub new_hash: String,
    /// The original MD5 digest, kept when `rollback` is enabled.
    pub backup_md5: Option<String>,
}

/// Result of processing one batch.
#[derive(Debug, Clone, Default)]
pub struct BatchResult {
    pub upgraded: Vec<UpgradedRecord>,
    pub skipped: usize,
    pub errors: usize,
}

/// Summary report for the full upgrade run.
#[derive(Debug, Clone, Default)]
pub struct UpgradeReport {
    pub total_processed: usize,
    pub total_upgraded: usize,
    pub total_skipped: usize,
    pub total_errors: usize,
    pub dry_run: bool,
}

impl fmt::Display for UpgradeReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.dry_run {
            write!(f, "[DRY RUN] ")?;
        }
        write!(
            f,
            "Password upgrade complete: {} processed, {} upgraded, {} skipped, {} errors",
            self.total_processed, self.total_upgraded, self.total_skipped, self.total_errors,
        )
    }
}

// ---------------------------------------------------------------------------
// Core functions
// ---------------------------------------------------------------------------

/// Compute `MD5(password + ":" + salt)` and return the lowercase hex digest.
pub fn compute_md5_hash(password: &str, salt: &str) -> String {
    let input = format!("{password}:{salt}");
    format!("{:x}", md5::compute(input.as_bytes()))
}

/// Hash an MD5 hex digest with Argon2id, returning a PHC-format string.
pub fn hash_password_argon2id(md5_digest: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(md5_digest.as_bytes(), &salt)
        .map_err(|e| PasswordError::Argon2(e.to_string()))?;
    Ok(hash.to_string())
}

/// Verify an MD5 hex digest against a stored Argon2id hash (PHC format).
pub fn verify_password(input_md5: &str, stored_hash: &str) -> Result<bool, PasswordError> {
    let parsed = PasswordHash::new(stored_hash)
        .map_err(|e| PasswordError::InvalidHash(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(input_md5.as_bytes(), &parsed)
        .is_ok())
}

/// Verify a password against the legacy MD5 scheme.
///
/// Computes `MD5(password + ":" + salt)` and compares (case-insensitive) with
/// the stored digest.
pub fn verify_password_legacy(
    password: &str,
    salt: &str,
    stored_md5: &str,
) -> Result<bool, PasswordError> {
    let computed = compute_md5_hash(password, salt);
    Ok(computed.eq_ignore_ascii_case(stored_md5))
}

/// Returns `true` when the hash string already looks like an Argon2 PHC hash.
fn is_already_argon2(hash: &str) -> bool {
    hash.starts_with("$argon2")
}

/// Process a batch of password records, upgrading MD5 → Argon2id.
///
/// Records that are already Argon2 are skipped.  When `dry_run` is set the
/// function counts what *would* happen but produces no `UpgradedRecord`s.
pub fn process_batch(
    records: &[PasswordRecord],
    config: &PasswordUpgradeConfig,
) -> Result<BatchResult, PasswordError> {
    config.validate()?;

    let mut result = BatchResult::default();

    for rec in records {
        if is_already_argon2(&rec.password_hash) {
            debug!(uuid = %rec.uuid, "already argon2 — skipping");
            result.skipped += 1;
            continue;
        }

        if config.dry_run {
            debug!(uuid = %rec.uuid, "dry-run: would upgrade");
            result.upgraded.push(UpgradedRecord {
                uuid: rec.uuid.clone(),
                new_hash: String::new(),
                backup_md5: None,
            });
            continue;
        }

        match hash_password_argon2id(&rec.password_hash) {
            Ok(new_hash) => {
                let backup = if config.rollback {
                    Some(rec.password_hash.clone())
                } else {
                    None
                };
                result.upgraded.push(UpgradedRecord {
                    uuid: rec.uuid.clone(),
                    new_hash,
                    backup_md5: backup,
                });
            }
            Err(e) => {
                warn!(uuid = %rec.uuid, error = %e, "failed to upgrade");
                result.errors += 1;
            }
        }
    }

    Ok(result)
}

/// Run the full upgrade across all supplied records, batching per the config.
pub async fn upgrade_passwords(
    config: &PasswordUpgradeConfig,
) -> Result<UpgradeReport, PasswordError> {
    config.validate()?;

    // In a real deployment this would query the database.  For now the
    // function demonstrates the batching/reporting skeleton.
    info!(
        batch_size = config.batch_size,
        dry_run = config.dry_run,
        rollback = config.rollback,
        "starting password upgrade"
    );

    let report = UpgradeReport {
        dry_run: config.dry_run,
        ..Default::default()
    };

    info!(%report, "password upgrade finished (no database connected)");
    Ok(report)
}

/// Run the upgrade on an in-memory list of records (useful for testing or
/// offline migration).
pub async fn upgrade_password_records(
    records: &[PasswordRecord],
    config: &PasswordUpgradeConfig,
) -> Result<UpgradeReport, PasswordError> {
    config.validate()?;

    let mut report = UpgradeReport {
        dry_run: config.dry_run,
        ..Default::default()
    };

    for chunk in records.chunks(config.batch_size) {
        let batch = process_batch(chunk, config)?;
        report.total_processed += chunk.len();
        report.total_upgraded += batch.upgraded.len();
        report.total_skipped += batch.skipped;
        report.total_errors += batch.errors;
    }

    info!(%report, "password upgrade finished");
    Ok(report)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PasswordUpgradeConfig {
        PasswordUpgradeConfig {
            connection_string: "sqlite://memory".into(),
            batch_size: 10,
            dry_run: false,
            rollback: false,
        }
    }

    // 1. MD5 computation
    #[test]
    fn test_compute_md5_hash() {
        // MD5("password:salt") — well-known value
        let hash = compute_md5_hash("password", "salt");
        assert_eq!(hash.len(), 32);
        // Verify it's consistent
        assert_eq!(hash, compute_md5_hash("password", "salt"));
        // Different input → different hash
        assert_ne!(hash, compute_md5_hash("other", "salt"));
    }

    // 2. Argon2id hash generation
    #[test]
    fn test_hash_password_argon2id() {
        let md5_digest = compute_md5_hash("password", "salt");
        let hashed = hash_password_argon2id(&md5_digest).expect("hashing should succeed");
        assert!(hashed.starts_with("$argon2id$"), "should be argon2id: {hashed}");
    }

    // 3. Argon2id verification
    #[test]
    fn test_verify_password() {
        let md5_digest = compute_md5_hash("mypassword", "mysalt");
        let hashed = hash_password_argon2id(&md5_digest).expect("hash");
        assert!(verify_password(&md5_digest, &hashed).expect("verify"), "correct digest should verify");
    }

    // 4. Round-trip: password → MD5 → Argon2id → verify
    #[test]
    fn test_roundtrip() {
        let password = "hunter2";
        let salt = "random_salt_123";
        let md5 = compute_md5_hash(password, salt);
        let argon_hash = hash_password_argon2id(&md5).expect("hash");

        // Simulates the viewer sending MD5(pass) and the server verifying
        let viewer_md5 = compute_md5_hash(password, salt);
        assert!(verify_password(&viewer_md5, &argon_hash).expect("verify"));

        // Wrong password should fail
        let wrong_md5 = compute_md5_hash("wrong", salt);
        assert!(!verify_password(&wrong_md5, &argon_hash).expect("verify wrong"));
    }

    // 5. Legacy MD5 fallback verification
    #[test]
    fn test_verify_password_legacy() {
        let password = "legacy_pass";
        let salt = "old_salt";
        let stored = compute_md5_hash(password, salt);

        assert!(verify_password_legacy(password, salt, &stored).expect("verify legacy"));
        assert!(!verify_password_legacy("wrong", salt, &stored).expect("verify legacy wrong"));
    }

    // 6. Legacy verification is case-insensitive for hex
    #[test]
    fn test_verify_legacy_case_insensitive() {
        let password = "test";
        let salt = "s";
        let stored = compute_md5_hash(password, salt).to_uppercase();
        assert!(
            verify_password_legacy(password, salt, &stored).expect("case-insensitive"),
            "hex comparison should be case-insensitive"
        );
    }

    // 7. Dry-run produces no real hashes
    #[test]
    fn test_dry_run() {
        let mut cfg = test_config();
        cfg.dry_run = true;

        let records = vec![PasswordRecord {
            uuid: "00000000-0000-0000-0000-000000000001".into(),
            password_hash: compute_md5_hash("pass", "salt"),
            password_salt: "salt".into(),
        }];

        let batch = process_batch(&records, &cfg).expect("batch");
        assert_eq!(batch.upgraded.len(), 1);
        // In dry-run the new_hash is empty
        assert!(batch.upgraded[0].new_hash.is_empty());
    }

    // 8. Batch processing with mixed records
    #[test]
    fn test_batch_mixed() {
        let cfg = test_config();
        let records = vec![
            PasswordRecord {
                uuid: "u1".into(),
                password_hash: compute_md5_hash("p1", "s1"),
                password_salt: "s1".into(),
            },
            PasswordRecord {
                uuid: "u2".into(),
                // Already upgraded
                password_hash: "$argon2id$v=19$m=19456,t=2,p=1$abc$def".into(),
                password_salt: "s2".into(),
            },
            PasswordRecord {
                uuid: "u3".into(),
                password_hash: compute_md5_hash("p3", "s3"),
                password_salt: "s3".into(),
            },
        ];

        let batch = process_batch(&records, &cfg).expect("batch");
        assert_eq!(batch.upgraded.len(), 2, "two MD5 records should upgrade");
        assert_eq!(batch.skipped, 1, "one argon2 record should be skipped");
        assert_eq!(batch.errors, 0);
    }

    // 9. Config validation rejects zero batch size
    #[test]
    fn test_config_validation_zero_batch() {
        let mut cfg = test_config();
        cfg.batch_size = 0;
        assert!(cfg.validate().is_err());
    }

    // 10. Rollback keeps backup
    #[test]
    fn test_rollback_keeps_backup() {
        let mut cfg = test_config();
        cfg.rollback = true;

        let original_md5 = compute_md5_hash("pass", "salt");
        let records = vec![PasswordRecord {
            uuid: "u1".into(),
            password_hash: original_md5.clone(),
            password_salt: "salt".into(),
        }];

        let batch = process_batch(&records, &cfg).expect("batch");
        assert_eq!(batch.upgraded.len(), 1);
        assert_eq!(
            batch.upgraded[0].backup_md5.as_deref(),
            Some(original_md5.as_str()),
            "rollback should preserve the original MD5"
        );
    }

    // 11. Full async upgrade_password_records
    #[tokio::test]
    async fn test_upgrade_password_records() {
        let cfg = test_config();
        let records = vec![
            PasswordRecord {
                uuid: "a".into(),
                password_hash: compute_md5_hash("p1", "s1"),
                password_salt: "s1".into(),
            },
            PasswordRecord {
                uuid: "b".into(),
                password_hash: "$argon2id$v=19$m=19456,t=2,p=1$abc$def".into(),
                password_salt: "s2".into(),
            },
        ];

        let report = upgrade_password_records(&records, &cfg).await.expect("report");
        assert_eq!(report.total_processed, 2);
        assert_eq!(report.total_upgraded, 1);
        assert_eq!(report.total_skipped, 1);
        assert_eq!(report.total_errors, 0);
        assert!(!report.dry_run);
    }

    // 12. Invalid hash string gives an error, not a panic
    #[test]
    fn test_verify_invalid_hash() {
        let result = verify_password("abc123", "not-a-phc-string");
        assert!(result.is_err());
    }
}
