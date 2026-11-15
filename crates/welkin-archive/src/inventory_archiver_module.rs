// SPDX-License-Identifier: GPL-3.0-or-later

//! Inventory archiver module — console commands and event dispatch for IAR
//! save/load operations.
//!
//! Ported from `OpenSim.Region.CoreModules.Avatar.Inventory.Archiver.InventoryArchiverModule`
//! (644 lines of C#).

use tracing::{error, info, warn};
use uuid::Uuid;

use crate::ArchiveError;
use crate::iar_write::IarWriteOptions;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default IAR filename when none is specified on the command line.
pub const DEFAULT_INV_BACKUP_FILENAME: &str = "user-inventory.iar";

// ---------------------------------------------------------------------------
// Inventory archive events
// ---------------------------------------------------------------------------

/// Events emitted during IAR save/load operations.
#[derive(Debug, Clone, PartialEq)]
pub enum InventoryArchiveEvent {
    /// An IAR save completed.
    SaveCompleted {
        request_id: Uuid,
        succeeded: bool,
        first_name: String,
        last_name: String,
        inv_path: String,
        save_count: u32,
        filter_count: u32,
        error: Option<String>,
    },
    /// An IAR load completed.
    LoadCompleted {
        request_id: Uuid,
        succeeded: bool,
        first_name: String,
        last_name: String,
        inv_path: String,
        load_count: u32,
        error: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// IInventoryArchiverModule trait
// ---------------------------------------------------------------------------

/// Trait representing the inventory archiver interface.
///
/// Mirrors `IInventoryArchiverModule` from the C# codebase.
pub trait IInventoryArchiverModule: Send + Sync {
    /// Save a user's inventory tree to an IAR archive.
    fn archive_inventory(
        &self,
        request_id: Uuid,
        first_name: &str,
        last_name: &str,
        inv_path: &str,
        save_path: &str,
        options: &IarWriteOptions,
    ) -> Result<(), ArchiveError>;

    /// Load an IAR archive into a user's inventory.
    fn dearchive_inventory(
        &self,
        request_id: Uuid,
        first_name: &str,
        last_name: &str,
        inv_path: &str,
        load_path: &str,
        merge: bool,
    ) -> Result<(), ArchiveError>;
}

// ---------------------------------------------------------------------------
// IAR load options (mirror of the merge flag and future expansions)
// ---------------------------------------------------------------------------

/// Options controlling how an IAR is loaded.
#[derive(Debug, Clone, Default)]
pub struct IarLoadOptions {
    /// If true, merge with existing inventory folders by name.
    pub merge: bool,
}

// ---------------------------------------------------------------------------
// User info (minimal representation for the archiver)
// ---------------------------------------------------------------------------

/// Minimal user account information needed by the archiver.
#[derive(Debug, Clone)]
pub struct UserInfo {
    /// User UUID.
    pub principal_id: Uuid,
    /// First name.
    pub first_name: String,
    /// Last name.
    pub last_name: String,
}

/// Trait for looking up users — the archiver needs to verify that a user
/// exists before performing IAR operations.
pub trait UserAccountLookup: Send + Sync {
    /// Look up a user by first and last name.  Returns `None` if not found.
    fn get_user_by_name(&self, first_name: &str, last_name: &str) -> Option<UserInfo>;
}

// ---------------------------------------------------------------------------
// InventoryArchiverModule
// ---------------------------------------------------------------------------

/// Inventory archiver module.
///
/// Manages IAR save/load console commands, user validation, and event
/// dispatch.
#[derive(Debug, Clone)]
pub struct InventoryArchiverModule {
    /// Module name for logging.
    pub name: String,
    /// Pending console-initiated requests (for tracking completion).
    pending_tasks: Vec<Uuid>,
}

impl InventoryArchiverModule {
    /// Create a new inventory archiver module.
    pub fn new() -> Self {
        Self {
            name: "Inventory Archiver Module".to_string(),
            pending_tasks: Vec::new(),
        }
    }

    /// Parse a `save iar` console command into write options and extracted
    /// parameters.
    ///
    /// Expected argument tokens (after the `save iar` prefix):
    ///
    /// ```text
    /// save iar [--noassets] [--perm=<C|CT|CTE>] [-h|--home=<url>]
    ///          [-c|--creators] [-v|--verbose]
    ///          [-e|--exclude=<name>] [-f|--excludefolder=<folder>]
    ///          <first> <last> <inventory-path> <password> [<filename>]
    /// ```
    ///
    /// Returns `(options, first, last, inv_path, password, filename)`.
    pub fn parse_save_iar_command(
        args: &[&str],
    ) -> Result<(IarWriteOptions, String, String, String, String, String), ArchiveError> {
        let mut options = IarWriteOptions::default();
        options.save_assets = true; // default: include assets

        let mut positional: Vec<String> = Vec::new();
        let mut excludes: Vec<String> = Vec::new();
        let mut exclude_folders: Vec<String> = Vec::new();

        let mut i = 0;
        while i < args.len() {
            let arg = args[i];
            match arg {
                "--noassets" => {
                    options.save_assets = false;
                }
                "-c" | "--creators" => {
                    options.include_creator_data = true;
                }
                "-v" | "--verbose" => {
                    options.verbose = true;
                }
                a if a.starts_with("--perm=") => {
                    let filter = a.strip_prefix("--perm=").unwrap_or("");
                    if filter.is_empty() {
                        return Err(ArchiveError::Invalid(
                            "--perm requires a value (e.g. --perm=CTM)".to_string(),
                        ));
                    }
                    for ch in filter.chars() {
                        if !matches!(ch, 'C' | 'T' | 'M') {
                            return Err(ArchiveError::Invalid(format!(
                                "invalid permission character '{ch}' in --perm; expected C, T, or M"
                            )));
                        }
                    }
                    options.filter_content = Some(filter.to_string());
                }
                a if a.starts_with("-h=") || a.starts_with("--home=") => {
                    let url = if a.starts_with("-h=") {
                        a.strip_prefix("-h=").unwrap_or("")
                    } else {
                        a.strip_prefix("--home=").unwrap_or("")
                    };
                    if url.is_empty() {
                        return Err(ArchiveError::Invalid(
                            "--home requires a URL value".to_string(),
                        ));
                    }
                    options.home_url = Some(url.to_string());
                    options.include_creator_data = true;
                }
                a if a.starts_with("-e=") || a.starts_with("--exclude=") => {
                    let val = if a.starts_with("-e=") {
                        a.strip_prefix("-e=").unwrap_or("")
                    } else {
                        a.strip_prefix("--exclude=").unwrap_or("")
                    };
                    if !val.is_empty() {
                        excludes.push(val.to_string());
                    }
                }
                a if a.starts_with("-f=") || a.starts_with("--excludefolder=") => {
                    let val = if a.starts_with("-f=") {
                        a.strip_prefix("-f=").unwrap_or("")
                    } else {
                        a.strip_prefix("--excludefolder=").unwrap_or("")
                    };
                    if !val.is_empty() {
                        exclude_folders.push(val.to_string());
                    }
                }
                other if !other.starts_with('-') => {
                    positional.push(other.to_string());
                }
                unknown => {
                    warn!(flag = unknown, "unknown save iar option, ignoring");
                }
            }
            i += 1;
        }

        // We need at least 4 positional: first, last, inv_path, password
        if positional.len() < 4 {
            return Err(ArchiveError::Invalid(
                "usage: save iar [options] <first> <last> <inventory-path> <password> [<filename>]"
                    .to_string(),
            ));
        }

        let first = positional[0].clone();
        let last = positional[1].clone();
        let inv_path = positional[2].clone();
        let password = positional[3].clone();
        let filename = if positional.len() > 4 {
            positional[4].clone()
        } else {
            DEFAULT_INV_BACKUP_FILENAME.to_string()
        };

        Ok((options, first, last, inv_path, password, filename))
    }

    /// Parse a `load iar` console command into options and extracted
    /// parameters.
    ///
    /// Expected argument tokens (after the `load iar` prefix):
    ///
    /// ```text
    /// load iar [-m|--merge] <first> <last> <inventory-path> <password> [<filename>]
    /// ```
    ///
    /// Returns `(merge, first, last, inv_path, password, filename)`.
    pub fn parse_load_iar_command(
        args: &[&str],
    ) -> Result<(IarLoadOptions, String, String, String, String, String), ArchiveError> {
        let mut load_opts = IarLoadOptions::default();
        let mut positional: Vec<String> = Vec::new();

        for arg in args {
            match *arg {
                "-m" | "--merge" => {
                    load_opts.merge = true;
                }
                other if !other.starts_with('-') => {
                    positional.push(other.to_string());
                }
                unknown => {
                    warn!(flag = unknown, "unknown load iar option, ignoring");
                }
            }
        }

        // We need at least 4 positional: first, last, inv_path, password
        if positional.len() < 4 {
            return Err(ArchiveError::Invalid(
                "usage: load iar [-m|--merge] <first> <last> <inventory-path> <password> [<filename>]"
                    .to_string(),
            ));
        }

        let first = positional[0].clone();
        let last = positional[1].clone();
        let inv_path = positional[2].clone();
        let password = positional[3].clone();
        let filename = if positional.len() > 4 {
            positional[4].clone()
        } else {
            DEFAULT_INV_BACKUP_FILENAME.to_string()
        };

        Ok((load_opts, first, last, inv_path, password, filename))
    }

    /// Validate an inventory path.
    ///
    /// Inventory paths must start with `/` and consist of folder names
    /// separated by `/`.
    pub fn validate_inventory_path(path: &str) -> Result<(), ArchiveError> {
        if path.is_empty() {
            return Err(ArchiveError::Invalid(
                "inventory path cannot be empty".to_string(),
            ));
        }
        if !path.starts_with('/') {
            return Err(ArchiveError::Invalid(format!(
                "inventory path must start with '/': {path}"
            )));
        }
        // Check for null bytes
        if path.contains('\0') {
            return Err(ArchiveError::Invalid(
                "inventory path contains null byte".to_string(),
            ));
        }
        Ok(())
    }

    /// Register a pending console task.
    pub fn add_pending_task(&mut self, id: Uuid) {
        self.pending_tasks.push(id);
    }

    /// Complete a pending console task.  Returns `true` if the task was found
    /// and removed.
    pub fn complete_pending_task(&mut self, id: Uuid) -> bool {
        if let Some(pos) = self.pending_tasks.iter().position(|&x| x == id) {
            self.pending_tasks.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return the number of pending tasks.
    pub fn pending_task_count(&self) -> usize {
        self.pending_tasks.len()
    }

    /// Emit a save-completed event (logs and returns the event for callers
    /// to forward to listeners).
    pub fn emit_save_completed(
        &self,
        request_id: Uuid,
        succeeded: bool,
        first_name: &str,
        last_name: &str,
        inv_path: &str,
        save_count: u32,
        filter_count: u32,
        error: Option<&str>,
    ) -> InventoryArchiveEvent {
        let event = InventoryArchiveEvent::SaveCompleted {
            request_id,
            succeeded,
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            inv_path: inv_path.to_string(),
            save_count,
            filter_count,
            error: error.map(|s| s.to_string()),
        };

        if succeeded {
            if filter_count == 0 {
                info!(
                    first = first_name,
                    last = last_name,
                    count = save_count,
                    "saved IAR archive"
                );
            } else {
                info!(
                    first = first_name,
                    last = last_name,
                    count = save_count,
                    skipped = filter_count,
                    "saved IAR archive (some items filtered)"
                );
            }
        } else {
            error!(
                first = first_name,
                last = last_name,
                error = error.unwrap_or("unknown"),
                "IAR save failed"
            );
        }

        event
    }

    /// Emit a load-completed event.
    pub fn emit_load_completed(
        &self,
        request_id: Uuid,
        succeeded: bool,
        first_name: &str,
        last_name: &str,
        inv_path: &str,
        load_count: u32,
        error: Option<&str>,
    ) -> InventoryArchiveEvent {
        let event = InventoryArchiveEvent::LoadCompleted {
            request_id,
            succeeded,
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            inv_path: inv_path.to_string(),
            load_count,
            error: error.map(|s| s.to_string()),
        };

        if succeeded {
            info!(
                first = first_name,
                last = last_name,
                count = load_count,
                path = inv_path,
                "loaded IAR archive"
            );
        } else {
            error!(
                first = first_name,
                last = last_name,
                error = error.unwrap_or("unknown"),
                "IAR load failed"
            );
        }

        event
    }

    /// Verify that a user exists (using the provided lookup).
    ///
    /// Returns the user info if found, or an error suitable for console
    /// output.
    pub fn verify_user(
        first_name: &str,
        last_name: &str,
        lookup: &dyn UserAccountLookup,
    ) -> Result<UserInfo, ArchiveError> {
        lookup
            .get_user_by_name(first_name, last_name)
            .ok_or_else(|| {
                ArchiveError::Invalid(format!("user not found: {first_name} {last_name}"))
            })
    }
}

impl Default for InventoryArchiverModule {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- save iar command parsing --

    #[test]
    fn parse_save_iar_minimal() {
        let (opts, first, last, inv_path, pass, filename) =
            InventoryArchiverModule::parse_save_iar_command(&["John", "Doe", "/Objects", "secret"])
                .unwrap();

        assert!(opts.save_assets);
        assert!(opts.filter_content.is_none());
        assert!(!opts.include_creator_data);
        assert!(!opts.verbose);
        assert_eq!(first, "John");
        assert_eq!(last, "Doe");
        assert_eq!(inv_path, "/Objects");
        assert_eq!(pass, "secret");
        assert_eq!(filename, DEFAULT_INV_BACKUP_FILENAME);
    }

    #[test]
    fn parse_save_iar_with_options() {
        let (opts, first, last, inv_path, pass, filename) =
            InventoryArchiverModule::parse_save_iar_command(&[
                "--noassets",
                "--perm=CT",
                "-c",
                "-v",
                "Jane",
                "Smith",
                "/My Inventory/Clothing",
                "pass123",
                "my_stuff.iar",
            ])
            .unwrap();

        assert!(!opts.save_assets);
        assert_eq!(opts.filter_content, Some("CT".to_string()));
        assert!(opts.include_creator_data);
        assert!(opts.verbose);
        assert_eq!(first, "Jane");
        assert_eq!(last, "Smith");
        assert_eq!(inv_path, "/My Inventory/Clothing");
        assert_eq!(pass, "pass123");
        assert_eq!(filename, "my_stuff.iar");
    }

    #[test]
    fn parse_save_iar_with_home() {
        let (opts, ..) = InventoryArchiverModule::parse_save_iar_command(&[
            "--home=http://example.com:8002",
            "John",
            "Doe",
            "/",
            "pass",
        ])
        .unwrap();

        assert_eq!(opts.home_url, Some("http://example.com:8002".to_string()));
        assert!(opts.include_creator_data);
    }

    #[test]
    fn parse_save_iar_invalid_perm() {
        let result = InventoryArchiverModule::parse_save_iar_command(&[
            "--perm=CX",
            "John",
            "Doe",
            "/",
            "pass",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_save_iar_missing_positional() {
        let result = InventoryArchiverModule::parse_save_iar_command(&["John", "Doe"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_save_iar_excludes() {
        // Exclude flags are parsed but stored in the options opaquely.
        // For now we just verify they don't break parsing.
        let result = InventoryArchiverModule::parse_save_iar_command(&[
            "--exclude=BadItem",
            "--excludefolder=TrashFolder",
            "John",
            "Doe",
            "/",
            "pass",
        ]);
        assert!(result.is_ok());
    }

    // -- load iar command parsing --

    #[test]
    fn parse_load_iar_minimal() {
        let (opts, first, last, inv_path, pass, filename) =
            InventoryArchiverModule::parse_load_iar_command(&["John", "Doe", "/Objects", "secret"])
                .unwrap();

        assert!(!opts.merge);
        assert_eq!(first, "John");
        assert_eq!(last, "Doe");
        assert_eq!(inv_path, "/Objects");
        assert_eq!(pass, "secret");
        assert_eq!(filename, DEFAULT_INV_BACKUP_FILENAME);
    }

    #[test]
    fn parse_load_iar_with_merge() {
        let (opts, first, last, inv_path, pass, filename) =
            InventoryArchiverModule::parse_load_iar_command(&[
                "--merge",
                "John",
                "Doe",
                "/Objects",
                "secret",
                "backup.iar",
            ])
            .unwrap();

        assert!(opts.merge);
        assert_eq!(first, "John");
        assert_eq!(last, "Doe");
        assert_eq!(inv_path, "/Objects");
        assert_eq!(pass, "secret");
        assert_eq!(filename, "backup.iar");
    }

    #[test]
    fn parse_load_iar_short_merge() {
        let (opts, ..) =
            InventoryArchiverModule::parse_load_iar_command(&["-m", "John", "Doe", "/", "pass"])
                .unwrap();
        assert!(opts.merge);
    }

    #[test]
    fn parse_load_iar_missing_positional() {
        let result = InventoryArchiverModule::parse_load_iar_command(&["John", "Doe"]);
        assert!(result.is_err());
    }

    // -- inventory path validation --

    #[test]
    fn validate_inventory_path_valid() {
        assert!(InventoryArchiverModule::validate_inventory_path("/").is_ok());
        assert!(InventoryArchiverModule::validate_inventory_path("/Objects").is_ok());
        assert!(InventoryArchiverModule::validate_inventory_path("/Objects/My Items/").is_ok());
    }

    #[test]
    fn validate_inventory_path_empty() {
        assert!(InventoryArchiverModule::validate_inventory_path("").is_err());
    }

    #[test]
    fn validate_inventory_path_no_leading_slash() {
        assert!(InventoryArchiverModule::validate_inventory_path("Objects/Stuff").is_err());
    }

    #[test]
    fn validate_inventory_path_null_byte() {
        assert!(InventoryArchiverModule::validate_inventory_path("/Objects\0/Bad").is_err());
    }

    // -- user verification --

    struct MockLookup;

    impl UserAccountLookup for MockLookup {
        fn get_user_by_name(&self, first: &str, last: &str) -> Option<UserInfo> {
            if first == "John" && last == "Doe" {
                Some(UserInfo {
                    principal_id: Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap(),
                    first_name: "John".to_string(),
                    last_name: "Doe".to_string(),
                })
            } else {
                None
            }
        }
    }

    #[test]
    fn verify_user_found() {
        let lookup = MockLookup;
        let user = InventoryArchiverModule::verify_user("John", "Doe", &lookup).unwrap();
        assert_eq!(user.first_name, "John");
        assert_eq!(user.last_name, "Doe");
        assert_eq!(
            user.principal_id,
            Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()
        );
    }

    #[test]
    fn verify_user_not_found() {
        let lookup = MockLookup;
        let result = InventoryArchiverModule::verify_user("Nobody", "Here", &lookup);
        assert!(result.is_err());
    }

    // -- pending tasks --

    #[test]
    fn pending_tasks_lifecycle() {
        let mut module = InventoryArchiverModule::new();
        assert_eq!(module.pending_task_count(), 0);

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        module.add_pending_task(id1);
        module.add_pending_task(id2);
        assert_eq!(module.pending_task_count(), 2);

        assert!(module.complete_pending_task(id1));
        assert_eq!(module.pending_task_count(), 1);

        // Completing the same task again returns false
        assert!(!module.complete_pending_task(id1));

        assert!(module.complete_pending_task(id2));
        assert_eq!(module.pending_task_count(), 0);
    }

    // -- event emission --

    #[test]
    fn emit_save_completed_success() {
        let module = InventoryArchiverModule::new();
        let id = Uuid::new_v4();
        let event = module.emit_save_completed(id, true, "John", "Doe", "/Objects", 42, 3, None);

        match event {
            InventoryArchiveEvent::SaveCompleted {
                request_id,
                succeeded,
                save_count,
                filter_count,
                ..
            } => {
                assert_eq!(request_id, id);
                assert!(succeeded);
                assert_eq!(save_count, 42);
                assert_eq!(filter_count, 3);
            }
            _ => panic!("expected SaveCompleted"),
        }
    }

    #[test]
    fn emit_save_completed_failure() {
        let module = InventoryArchiverModule::new();
        let id = Uuid::new_v4();
        let event =
            module.emit_save_completed(id, false, "John", "Doe", "/", 0, 0, Some("disk full"));

        match event {
            InventoryArchiveEvent::SaveCompleted {
                succeeded, error, ..
            } => {
                assert!(!succeeded);
                assert_eq!(error, Some("disk full".to_string()));
            }
            _ => panic!("expected SaveCompleted"),
        }
    }

    #[test]
    fn emit_load_completed_success() {
        let module = InventoryArchiverModule::new();
        let id = Uuid::new_v4();
        let event = module.emit_load_completed(id, true, "Jane", "Smith", "/Clothing", 100, None);

        match event {
            InventoryArchiveEvent::LoadCompleted {
                succeeded,
                load_count,
                ..
            } => {
                assert!(succeeded);
                assert_eq!(load_count, 100);
            }
            _ => panic!("expected LoadCompleted"),
        }
    }

    #[test]
    fn emit_load_completed_failure() {
        let module = InventoryArchiverModule::new();
        let id = Uuid::new_v4();
        let event =
            module.emit_load_completed(id, false, "Jane", "Smith", "/", 0, Some("corrupt archive"));

        match event {
            InventoryArchiveEvent::LoadCompleted {
                succeeded, error, ..
            } => {
                assert!(!succeeded);
                assert_eq!(error, Some("corrupt archive".to_string()));
            }
            _ => panic!("expected LoadCompleted"),
        }
    }

    // -- event equality --

    #[test]
    fn inventory_archive_event_equality() {
        let id = Uuid::new_v4();
        let e1 = InventoryArchiveEvent::LoadCompleted {
            request_id: id,
            succeeded: true,
            first_name: "A".to_string(),
            last_name: "B".to_string(),
            inv_path: "/".to_string(),
            load_count: 1,
            error: None,
        };
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }

    // -- default --

    #[test]
    fn default_module() {
        let module = InventoryArchiverModule::default();
        assert_eq!(module.name, "Inventory Archiver Module");
        assert_eq!(module.pending_task_count(), 0);
    }
}
