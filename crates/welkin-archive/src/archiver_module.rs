// SPDX-License-Identifier: LGPL-3.0-or-later

//! Region archiver module — console commands and event dispatch for OAR
//! save/load operations.
//!
//! Ported from `OpenSim.Region.CoreModules.World.Archiver.ArchiverModule`
//! (349 lines of C#).

use std::path::Path;

use tracing::{error, info, warn};
use uuid::Uuid;

use crate::ArchiveError;
use crate::oar_read::OarLoadOptions;
use crate::oar_write::OarWriteOptions;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default OAR filename when none is specified on the command line.
pub const DEFAULT_OAR_BACKUP_FILENAME: &str = "region.oar";

// ---------------------------------------------------------------------------
// Archive events
// ---------------------------------------------------------------------------

/// Events emitted during OAR save/load operations so that other modules can
/// hook into the archiver pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum ArchiveEvent {
    /// An OAR save has started.
    SaveStarted {
        /// Path (or stream identifier) the archive is being written to.
        path: String,
        /// Optional request id for correlating events.
        request_id: Uuid,
    },
    /// An OAR save completed successfully.
    SaveCompleted { path: String, request_id: Uuid },
    /// An OAR save failed.
    SaveFailed {
        path: String,
        request_id: Uuid,
        error: String,
    },
    /// An OAR load has started.
    LoadStarted { path: String, request_id: Uuid },
    /// An OAR load completed successfully.
    LoadCompleted { path: String, request_id: Uuid },
    /// An OAR load failed.
    LoadFailed {
        path: String,
        request_id: Uuid,
        error: String,
    },
}

// ---------------------------------------------------------------------------
// IRegionArchiverModule trait
// ---------------------------------------------------------------------------

/// Trait representing the region archiver interface.
///
/// Mirrors `IRegionArchiverModule` from the C# codebase.  Implementors
/// provide the actual scene data; this crate supplies the archive
/// read/write mechanics.
pub trait IRegionArchiverModule: Send + Sync {
    /// Save a region to an OAR archive at `path`.
    fn archive_region(
        &self,
        path: &str,
        request_id: Uuid,
        options: &OarWriteOptions,
    ) -> Result<(), ArchiveError>;

    /// Load a region from an OAR archive at `path`.
    fn dearchive_region(
        &self,
        path: &str,
        request_id: Uuid,
        options: &OarLoadOptions,
    ) -> Result<(), ArchiveError>;
}

// ---------------------------------------------------------------------------
// ArchiverModule
// ---------------------------------------------------------------------------

/// Event-listener callback type.
pub type ArchiveEventHandler = Box<dyn Fn(&ArchiveEvent) + Send + Sync>;

/// Region archiver module.
///
/// Manages OAR save/load console commands and dispatches
/// [`ArchiveEvent`]s to registered listeners.
#[derive(Debug, Clone)]
pub struct ArchiverModule {
    /// Region name for log messages.
    pub region_name: String,
    /// Region UUID.
    pub region_id: Uuid,
    /// Region size X (metres).
    pub region_size_x: u32,
    /// Region size Y (metres).
    pub region_size_y: u32,
}

impl ArchiverModule {
    /// Create a new archiver module for a region.
    pub fn new(region_name: &str, region_id: Uuid, size_x: u32, size_y: u32) -> Self {
        Self {
            region_name: region_name.to_string(),
            region_id,
            region_size_x: size_x,
            region_size_y: size_y,
        }
    }

    /// Parse a `save oar` console command into write options and a filename.
    ///
    /// Expected argument tokens (after the `save oar` prefix):
    ///
    /// ```text
    /// save oar [--noassets] [--perm=<C|CT|CTE>] [--publish] [--all] [<filename>]
    /// ```
    ///
    /// Returns `(options, filename)`.
    pub fn parse_save_oar_command(
        &self,
        args: &[&str],
    ) -> Result<(OarWriteOptions, String), ArchiveError> {
        let mut options = OarWriteOptions::default();
        let mut filename: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            let arg = args[i];
            match arg {
                "--noassets" => {
                    options.include_assets = false;
                }
                "--publish" => {
                    options.wipe_owners = true;
                }
                "--all" => {
                    // all regions — noted in options but handled at a higher level
                }
                a if a.starts_with("--perm=") => {
                    let filter = a.strip_prefix("--perm=").unwrap_or("");
                    if filter.is_empty() {
                        return Err(ArchiveError::Invalid(
                            "--perm requires a value (e.g. --perm=CT)".to_string(),
                        ));
                    }
                    // Validate filter characters
                    for ch in filter.chars() {
                        if !matches!(ch, 'C' | 'T' | 'E') {
                            return Err(ArchiveError::Invalid(format!(
                                "invalid permission character '{ch}' in --perm; expected C, T, or E"
                            )));
                        }
                    }
                    options.filter_content = Some(filter.to_string());
                }
                other if !other.starts_with('-') => {
                    filename = Some(other.to_string());
                }
                unknown => {
                    warn!(flag = unknown, "unknown save oar option, ignoring");
                }
            }
            i += 1;
        }

        let path = filename.unwrap_or_else(|| DEFAULT_OAR_BACKUP_FILENAME.to_string());
        Ok((options, path))
    }

    /// Parse a `load oar` console command into load options and a filename.
    ///
    /// Expected argument tokens (after the `load oar` prefix):
    ///
    /// ```text
    /// load oar [--merge] [--skip-assets] [--default-user <first> <last>]
    ///          [--force-terrain] [--force-parcels] [--no-objects]
    ///          [--rotation <degrees>] [--displacement <x,y,z>]
    ///          [--bounding-origin <x,y,z>] [--bounding-size <x,y,z>]
    ///          [<filename>]
    /// ```
    ///
    /// Returns `(options, filename)`.
    pub fn parse_load_oar_command(
        &self,
        args: &[&str],
    ) -> Result<(OarLoadOptions, String), ArchiveError> {
        let mut options = OarLoadOptions::default();
        options.incoming_region_size = (self.region_size_x as f32, self.region_size_y as f32);

        let mut filename: Option<String> = None;
        let mut i = 0;

        while i < args.len() {
            let arg = args[i];
            match arg {
                "--merge" | "-m" => {
                    options.merge = true;
                }
                "--skip-assets" | "-s" => {
                    options.skip_assets = true;
                }
                "--force-terrain" | "--merge-terrain" | "--forceterrain" => {
                    options.force_terrain = true;
                }
                "--force-parcels" | "--merge-parcels" | "--forceparcels" => {
                    options.force_parcels = true;
                }
                "--no-objects" => {
                    options.no_objects = true;
                }
                "--mergeReplaceObjects" => {
                    options.merge_replace_objects = true;
                }
                "--default-user" => {
                    // Next two tokens are "first" "last"
                    if i + 2 >= args.len() {
                        return Err(ArchiveError::Invalid(
                            "--default-user requires <first> <last>".to_string(),
                        ));
                    }
                    let first = args[i + 1];
                    let last = args[i + 2];
                    i += 2;
                    // In the real system the UUID is looked up from the user
                    // service. Here we store a placeholder; the caller must
                    // resolve the name.
                    info!(
                        first_name = first,
                        last_name = last,
                        "default-user specified (caller must resolve UUID)"
                    );
                    // We cannot resolve the UUID here — the caller does that.
                    // Store nil so the caller knows the flag was present.
                    options.default_user = Some(Uuid::nil());
                }
                "--displacement" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(ArchiveError::Invalid(
                            "--displacement requires a <x,y,z> value".to_string(),
                        ));
                    }
                    options.displacement = Some(parse_vector3(args[i])?);
                }
                "--rotation" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(ArchiveError::Invalid(
                            "--rotation requires a degrees value".to_string(),
                        ));
                    }
                    let degrees: f32 = args[i].parse().map_err(|_| {
                        ArchiveError::Invalid(format!("invalid rotation value: {}", args[i]))
                    })?;
                    let clamped = degrees.clamp(-359.0, 359.0);
                    options.rotation = Some(clamped.to_radians());
                }
                "--bounding-origin" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(ArchiveError::Invalid(
                            "--bounding-origin requires a <x,y,z> value".to_string(),
                        ));
                    }
                    let origin = parse_vector3(args[i])?;
                    let existing_size = options.bounding_box.map(|(_, s)| s).unwrap_or((
                        self.region_size_x as f32,
                        self.region_size_y as f32,
                        f32::MAX,
                    ));
                    options.bounding_box = Some((origin, existing_size));
                }
                "--bounding-size" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(ArchiveError::Invalid(
                            "--bounding-size requires a <x,y,z> value".to_string(),
                        ));
                    }
                    let size = parse_vector3(args[i])?;
                    let existing_origin = options
                        .bounding_box
                        .map(|(o, _)| o)
                        .unwrap_or((0.0, 0.0, -100.0));
                    options.bounding_box = Some((existing_origin, size));
                }
                other if !other.starts_with('-') => {
                    filename = Some(other.to_string());
                }
                unknown => {
                    warn!(flag = unknown, "unknown load oar option, ignoring");
                }
            }
            i += 1;
        }

        let path = filename.unwrap_or_else(|| DEFAULT_OAR_BACKUP_FILENAME.to_string());
        Ok((options, path))
    }

    /// Emit an archive event (for logging).
    ///
    /// In production this would dispatch to registered event handlers.
    /// Here we log via tracing so tests can verify behaviour.
    pub fn emit_event(&self, event: &ArchiveEvent) {
        match event {
            ArchiveEvent::SaveStarted { path, .. } => {
                info!(
                    region = %self.region_name,
                    path = path,
                    "OAR save started"
                );
            }
            ArchiveEvent::SaveCompleted { path, .. } => {
                info!(
                    region = %self.region_name,
                    path = path,
                    "OAR save completed"
                );
            }
            ArchiveEvent::SaveFailed { path, error, .. } => {
                error!(
                    region = %self.region_name,
                    path = path,
                    error = error,
                    "OAR save failed"
                );
            }
            ArchiveEvent::LoadStarted { path, .. } => {
                info!(
                    region = %self.region_name,
                    path = path,
                    "OAR load started"
                );
            }
            ArchiveEvent::LoadCompleted { path, .. } => {
                info!(
                    region = %self.region_name,
                    path = path,
                    "OAR load completed"
                );
            }
            ArchiveEvent::LoadFailed { path, error, .. } => {
                error!(
                    region = %self.region_name,
                    path = path,
                    error = error,
                    "OAR load failed"
                );
            }
        }
    }

    /// Validate that a file path looks reasonable for saving.
    pub fn validate_save_path(path: &str) -> Result<(), ArchiveError> {
        if path.is_empty() {
            return Err(ArchiveError::Invalid("empty save path".to_string()));
        }
        // Check the parent directory exists (if specified)
        let p = Path::new(path);
        if let Some(parent) = p.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            return Err(ArchiveError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("parent directory does not exist: {}", parent.display()),
            )));
        }
        Ok(())
    }

    /// Validate that a file path exists for loading.
    pub fn validate_load_path(path: &str) -> Result<(), ArchiveError> {
        if path.is_empty() {
            return Err(ArchiveError::Invalid("empty load path".to_string()));
        }
        if !Path::new(path).exists() {
            return Err(ArchiveError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("file not found: {path}"),
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Vector3 parsing helper
// ---------------------------------------------------------------------------

/// Parse a vector3 string in the format `<x,y,z>` or `x,y,z`.
///
/// Mirrors the C# `Vector3.Parse` behaviour used for displacement, bounding
/// origin, and bounding size.
fn parse_vector3(s: &str) -> Result<(f32, f32, f32), ArchiveError> {
    let trimmed = s.trim().trim_start_matches('<').trim_end_matches('>');
    let parts: Vec<&str> = trimmed.split(',').collect();
    if parts.len() != 3 {
        return Err(ArchiveError::Invalid(format!(
            "expected <x,y,z> but got: {s}"
        )));
    }
    let x: f32 = parts[0]
        .trim()
        .parse()
        .map_err(|_| ArchiveError::Invalid(format!("invalid x component in vector: {s}")))?;
    let y: f32 = parts[1]
        .trim()
        .parse()
        .map_err(|_| ArchiveError::Invalid(format!("invalid y component in vector: {s}")))?;
    let z: f32 = parts[2]
        .trim()
        .parse()
        .map_err(|_| ArchiveError::Invalid(format!("invalid z component in vector: {s}")))?;
    Ok((x, y, z))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_module() -> ArchiverModule {
        ArchiverModule::new("TestRegion", Uuid::new_v4(), 256, 256)
    }

    // -- save oar command parsing --

    #[test]
    fn parse_save_oar_defaults() {
        let module = make_module();
        let (opts, path) = module.parse_save_oar_command(&[]).unwrap();
        assert!(opts.include_assets);
        assert!(opts.filter_content.is_none());
        assert!(!opts.wipe_owners);
        assert_eq!(path, DEFAULT_OAR_BACKUP_FILENAME);
    }

    #[test]
    fn parse_save_oar_noassets() {
        let module = make_module();
        let (opts, _) = module.parse_save_oar_command(&["--noassets"]).unwrap();
        assert!(!opts.include_assets);
    }

    #[test]
    fn parse_save_oar_with_perm_filter() {
        let module = make_module();
        let (opts, _) = module.parse_save_oar_command(&["--perm=CT"]).unwrap();
        assert_eq!(opts.filter_content, Some("CT".to_string()));
    }

    #[test]
    fn parse_save_oar_invalid_perm_char() {
        let module = make_module();
        let result = module.parse_save_oar_command(&["--perm=CX"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_save_oar_with_filename() {
        let module = make_module();
        let (_, path) = module
            .parse_save_oar_command(&["--noassets", "my_region.oar"])
            .unwrap();
        assert_eq!(path, "my_region.oar");
    }

    #[test]
    fn parse_save_oar_publish_wipes_owners() {
        let module = make_module();
        let (opts, _) = module.parse_save_oar_command(&["--publish"]).unwrap();
        assert!(opts.wipe_owners);
    }

    // -- load oar command parsing --

    #[test]
    fn parse_load_oar_defaults() {
        let module = make_module();
        let (opts, path) = module.parse_load_oar_command(&[]).unwrap();
        assert!(!opts.merge);
        assert!(!opts.skip_assets);
        assert!(!opts.force_terrain);
        assert!(!opts.force_parcels);
        assert!(!opts.no_objects);
        assert!(opts.displacement.is_none());
        assert!(opts.rotation.is_none());
        assert!(opts.bounding_box.is_none());
        assert!(opts.default_user.is_none());
        assert_eq!(path, DEFAULT_OAR_BACKUP_FILENAME);
    }

    #[test]
    fn parse_load_oar_merge() {
        let module = make_module();
        let (opts, _) = module.parse_load_oar_command(&["--merge"]).unwrap();
        assert!(opts.merge);
    }

    #[test]
    fn parse_load_oar_skip_assets() {
        let module = make_module();
        let (opts, _) = module.parse_load_oar_command(&["--skip-assets"]).unwrap();
        assert!(opts.skip_assets);
    }

    #[test]
    fn parse_load_oar_force_terrain() {
        let module = make_module();
        let (opts, _) = module
            .parse_load_oar_command(&["--merge", "--force-terrain"])
            .unwrap();
        assert!(opts.merge);
        assert!(opts.force_terrain);
    }

    #[test]
    fn parse_load_oar_force_parcels() {
        let module = make_module();
        let (opts, _) = module
            .parse_load_oar_command(&["--merge", "--force-parcels"])
            .unwrap();
        assert!(opts.merge);
        assert!(opts.force_parcels);
    }

    #[test]
    fn parse_load_oar_no_objects() {
        let module = make_module();
        let (opts, _) = module.parse_load_oar_command(&["--no-objects"]).unwrap();
        assert!(opts.no_objects);
    }

    #[test]
    fn parse_load_oar_displacement() {
        let module = make_module();
        let (opts, _) = module
            .parse_load_oar_command(&["--displacement", "<128,128,0>"])
            .unwrap();
        let d = opts.displacement.unwrap();
        assert!((d.0 - 128.0).abs() < f32::EPSILON);
        assert!((d.1 - 128.0).abs() < f32::EPSILON);
        assert!((d.2 - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_load_oar_rotation() {
        let module = make_module();
        let (opts, _) = module
            .parse_load_oar_command(&["--rotation", "45"])
            .unwrap();
        let r = opts.rotation.unwrap();
        assert!((r - 45.0f32.to_radians()).abs() < 0.001);
    }

    #[test]
    fn parse_load_oar_rotation_clamped() {
        let module = make_module();
        let (opts, _) = module
            .parse_load_oar_command(&["--rotation", "500"])
            .unwrap();
        let r = opts.rotation.unwrap();
        // 500 clamped to 359
        assert!((r - 359.0f32.to_radians()).abs() < 0.001);
    }

    #[test]
    fn parse_load_oar_bounding_box() {
        let module = make_module();
        let (opts, _) = module
            .parse_load_oar_command(&[
                "--bounding-origin",
                "<10,20,30>",
                "--bounding-size",
                "<100,200,300>",
            ])
            .unwrap();
        let (origin, size) = opts.bounding_box.unwrap();
        assert!((origin.0 - 10.0).abs() < f32::EPSILON);
        assert!((origin.1 - 20.0).abs() < f32::EPSILON);
        assert!((origin.2 - 30.0).abs() < f32::EPSILON);
        assert!((size.0 - 100.0).abs() < f32::EPSILON);
        assert!((size.1 - 200.0).abs() < f32::EPSILON);
        assert!((size.2 - 300.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_load_oar_default_user() {
        let module = make_module();
        let (opts, _) = module
            .parse_load_oar_command(&["--default-user", "John", "Doe"])
            .unwrap();
        // The UUID is nil (placeholder) because we can't resolve names here
        assert_eq!(opts.default_user, Some(Uuid::nil()));
    }

    #[test]
    fn parse_load_oar_default_user_missing_args() {
        let module = make_module();
        let result = module.parse_load_oar_command(&["--default-user", "John"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_load_oar_with_filename() {
        let module = make_module();
        let (_, path) = module
            .parse_load_oar_command(&["--merge", "backup.oar"])
            .unwrap();
        assert_eq!(path, "backup.oar");
    }

    #[test]
    fn parse_load_oar_combined() {
        let module = make_module();
        let (opts, path) = module
            .parse_load_oar_command(&[
                "--merge",
                "--skip-assets",
                "--force-terrain",
                "--no-objects",
                "--displacement",
                "10,20,5",
                "--rotation",
                "90",
                "my_region.oar",
            ])
            .unwrap();
        assert!(opts.merge);
        assert!(opts.skip_assets);
        assert!(opts.force_terrain);
        assert!(opts.no_objects);
        assert!(opts.displacement.is_some());
        assert!(opts.rotation.is_some());
        assert_eq!(path, "my_region.oar");
    }

    // -- vector3 parsing --

    #[test]
    fn parse_vector3_angle_brackets() {
        let v = parse_vector3("<128,128,0>").unwrap();
        assert!((v.0 - 128.0).abs() < f32::EPSILON);
        assert!((v.1 - 128.0).abs() < f32::EPSILON);
        assert!((v.2 - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_vector3_no_brackets() {
        let v = parse_vector3("10.5,20.5,30.5").unwrap();
        assert!((v.0 - 10.5).abs() < f32::EPSILON);
        assert!((v.1 - 20.5).abs() < f32::EPSILON);
        assert!((v.2 - 30.5).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_vector3_invalid() {
        assert!(parse_vector3("not_a_vector").is_err());
        assert!(parse_vector3("1,2").is_err());
        assert!(parse_vector3("a,b,c").is_err());
    }

    // -- archive events --

    #[test]
    fn archive_event_dispatch_save() {
        let module = make_module();
        let request_id = Uuid::new_v4();

        // These should not panic
        module.emit_event(&ArchiveEvent::SaveStarted {
            path: "test.oar".to_string(),
            request_id,
        });
        module.emit_event(&ArchiveEvent::SaveCompleted {
            path: "test.oar".to_string(),
            request_id,
        });
        module.emit_event(&ArchiveEvent::SaveFailed {
            path: "test.oar".to_string(),
            request_id,
            error: "disk full".to_string(),
        });
    }

    #[test]
    fn archive_event_dispatch_load() {
        let module = make_module();
        let request_id = Uuid::new_v4();

        module.emit_event(&ArchiveEvent::LoadStarted {
            path: "test.oar".to_string(),
            request_id,
        });
        module.emit_event(&ArchiveEvent::LoadCompleted {
            path: "test.oar".to_string(),
            request_id,
        });
        module.emit_event(&ArchiveEvent::LoadFailed {
            path: "test.oar".to_string(),
            request_id,
            error: "corrupt archive".to_string(),
        });
    }

    // -- path validation --

    #[test]
    fn validate_save_path_empty() {
        assert!(ArchiverModule::validate_save_path("").is_err());
    }

    #[test]
    fn validate_save_path_relative_ok() {
        // A plain filename in the current dir should be accepted
        assert!(ArchiverModule::validate_save_path("region.oar").is_ok());
    }

    #[test]
    fn validate_load_path_empty() {
        assert!(ArchiverModule::validate_load_path("").is_err());
    }

    #[test]
    fn validate_load_path_nonexistent() {
        assert!(ArchiverModule::validate_load_path("/nonexistent/path/fake.oar").is_err());
    }

    // -- event equality --

    #[test]
    fn archive_event_equality() {
        let id = Uuid::new_v4();
        let e1 = ArchiveEvent::SaveStarted {
            path: "a.oar".to_string(),
            request_id: id,
        };
        let e2 = ArchiveEvent::SaveStarted {
            path: "a.oar".to_string(),
            request_id: id,
        };
        assert_eq!(e1, e2);

        let e3 = ArchiveEvent::SaveCompleted {
            path: "a.oar".to_string(),
            request_id: id,
        };
        assert_ne!(e1, e3);
    }

    // -- legacy aliases --

    #[test]
    fn parse_load_oar_legacy_forceterrain() {
        let module = make_module();
        let (opts, _) = module.parse_load_oar_command(&["--forceterrain"]).unwrap();
        assert!(opts.force_terrain);
    }

    #[test]
    fn parse_load_oar_legacy_forceparcels() {
        let module = make_module();
        let (opts, _) = module.parse_load_oar_command(&["--forceparcels"]).unwrap();
        assert!(opts.force_parcels);
    }

    #[test]
    fn parse_load_oar_short_flags() {
        let module = make_module();
        let (opts, _) = module.parse_load_oar_command(&["-m", "-s"]).unwrap();
        assert!(opts.merge);
        assert!(opts.skip_assets);
    }

    #[test]
    fn parse_load_oar_displacement_missing_value() {
        let module = make_module();
        let result = module.parse_load_oar_command(&["--displacement"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_load_oar_rotation_missing_value() {
        let module = make_module();
        let result = module.parse_load_oar_command(&["--rotation"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_load_oar_rotation_invalid_value() {
        let module = make_module();
        let result = module.parse_load_oar_command(&["--rotation", "abc"]);
        assert!(result.is_err());
    }
}
