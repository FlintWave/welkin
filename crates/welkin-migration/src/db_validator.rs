// SPDX-License-Identifier: LGPL-3.0-or-later

//! Database schema validator for the OpenSimulator C# → Rust migration.
//!
//! Compares the actual database schema against the expected Rust-side schema
//! and reports mismatches (missing tables, extra tables, column type
//! differences, missing columns).

use std::collections::BTreeMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Schema data structures
// ---------------------------------------------------------------------------

/// A single column in an expected or actual table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDef {
    pub name: String,
    /// SQL type name (e.g. `VARCHAR(36)`, `TEXT`, `INTEGER`).  Compared
    /// case-insensitively during validation.
    pub col_type: String,
    pub nullable: bool,
}

/// A table definition (expected or actual).
#[derive(Debug, Clone)]
pub struct TableDef {
    pub name: String,
    pub columns: Vec<ColumnDef>,
}

impl TableDef {
    fn column_map(&self) -> BTreeMap<String, &ColumnDef> {
        self.columns
            .iter()
            .map(|c| (c.name.to_lowercase(), c))
            .collect()
    }
}

/// The full expected schema — a collection of [`TableDef`]s.
#[derive(Debug, Clone)]
pub struct ExpectedSchema {
    pub tables: Vec<TableDef>,
}

/// The actual schema read from the database.
#[derive(Debug, Clone)]
pub struct ActualSchema {
    pub tables: Vec<TableDef>,
}

// ---------------------------------------------------------------------------
// Validation report
// ---------------------------------------------------------------------------

/// One difference found during validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaDiff {
    MissingTable(String),
    ExtraTable(String),
    MissingColumn {
        table: String,
        column: String,
    },
    ExtraColumn {
        table: String,
        column: String,
    },
    TypeMismatch {
        table: String,
        column: String,
        expected: String,
        actual: String,
    },
    NullabilityMismatch {
        table: String,
        column: String,
        expected_nullable: bool,
        actual_nullable: bool,
    },
}

/// Full validation report returned by [`compare_schemas`] / [`validate_schema`].
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub diffs: Vec<SchemaDiff>,
}

impl ValidationReport {
    /// Returns `true` when the schema matches perfectly.
    pub fn is_ok(&self) -> bool {
        self.diffs.is_empty()
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.diffs.is_empty() {
            return write!(f, "Schema validation passed — no differences found.");
        }
        writeln!(
            f,
            "Schema validation found {} difference(s):",
            self.diffs.len()
        )?;
        for d in &self.diffs {
            match d {
                SchemaDiff::MissingTable(t) => {
                    writeln!(f, "  MISSING TABLE: {t}")?;
                }
                SchemaDiff::ExtraTable(t) => {
                    writeln!(f, "  EXTRA TABLE:   {t}")?;
                }
                SchemaDiff::MissingColumn { table, column } => {
                    writeln!(f, "  MISSING COLUMN: {table}.{column}")?;
                }
                SchemaDiff::ExtraColumn { table, column } => {
                    writeln!(f, "  EXTRA COLUMN:   {table}.{column}")?;
                }
                SchemaDiff::TypeMismatch {
                    table,
                    column,
                    expected,
                    actual,
                } => {
                    writeln!(
                        f,
                        "  TYPE MISMATCH:  {table}.{column}: expected {expected}, got {actual}"
                    )?;
                }
                SchemaDiff::NullabilityMismatch {
                    table,
                    column,
                    expected_nullable,
                    actual_nullable,
                } => {
                    writeln!(
                        f,
                        "  NULLABLE MISMATCH: {table}.{column}: expected nullable={expected_nullable}, got nullable={actual_nullable}"
                    )?;
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Comparison engine
// ---------------------------------------------------------------------------

/// Compare an expected schema against an actual one and return every
/// difference.
pub fn compare_schemas(expected: &ExpectedSchema, actual: &ActualSchema) -> ValidationReport {
    let mut diffs = Vec::new();

    let expected_map: BTreeMap<String, &TableDef> = expected
        .tables
        .iter()
        .map(|t| (t.name.to_lowercase(), t))
        .collect();

    let actual_map: BTreeMap<String, &TableDef> = actual
        .tables
        .iter()
        .map(|t| (t.name.to_lowercase(), t))
        .collect();

    // Missing tables (in expected but not actual).
    for name in expected_map.keys() {
        if !actual_map.contains_key(name) {
            diffs.push(SchemaDiff::MissingTable(name.clone()));
        }
    }

    // Extra tables (in actual but not expected).
    for name in actual_map.keys() {
        if !expected_map.contains_key(name) {
            diffs.push(SchemaDiff::ExtraTable(name.clone()));
        }
    }

    // Column-level comparison for tables present in both.
    for (name, exp_table) in &expected_map {
        let Some(act_table) = actual_map.get(name) else {
            continue; // already reported as MissingTable
        };

        let exp_cols = exp_table.column_map();
        let act_cols = act_table.column_map();

        for (col_name, exp_col) in &exp_cols {
            match act_cols.get(col_name) {
                None => {
                    diffs.push(SchemaDiff::MissingColumn {
                        table: name.clone(),
                        column: col_name.clone(),
                    });
                }
                Some(act_col) => {
                    if !types_match(&exp_col.col_type, &act_col.col_type) {
                        diffs.push(SchemaDiff::TypeMismatch {
                            table: name.clone(),
                            column: col_name.clone(),
                            expected: exp_col.col_type.clone(),
                            actual: act_col.col_type.clone(),
                        });
                    }
                    if exp_col.nullable != act_col.nullable {
                        diffs.push(SchemaDiff::NullabilityMismatch {
                            table: name.clone(),
                            column: col_name.clone(),
                            expected_nullable: exp_col.nullable,
                            actual_nullable: act_col.nullable,
                        });
                    }
                }
            }
        }

        for col_name in act_cols.keys() {
            if !exp_cols.contains_key(col_name) {
                diffs.push(SchemaDiff::ExtraColumn {
                    table: name.clone(),
                    column: col_name.clone(),
                });
            }
        }
    }

    ValidationReport { diffs }
}

/// Case-insensitive type comparison with common alias normalisation.
fn types_match(expected: &str, actual: &str) -> bool {
    let norm = |s: &str| -> String {
        let s = s.to_uppercase().trim().to_string();
        // Treat common aliases as equivalent.
        match s.as_str() {
            "INT" | "INTEGER" | "INT(11)" => "INTEGER".to_string(),
            "BOOL" | "BOOLEAN" | "TINYINT(1)" => "BOOLEAN".to_string(),
            "BLOB" | "LONGBLOB" | "MEDIUMBLOB" => "BLOB".to_string(),
            _ => s,
        }
    };
    norm(expected) == norm(actual)
}

// ---------------------------------------------------------------------------
// ALTER statement generation
// ---------------------------------------------------------------------------

/// Produce `ALTER TABLE` SQL for differences that can be fixed mechanically
/// (missing columns and type mismatches).  Missing tables are not covered here
/// — the caller should generate `CREATE TABLE` for those.
pub fn generate_alter_statements(report: &ValidationReport) -> Vec<String> {
    let mut stmts = Vec::new();

    for diff in &report.diffs {
        match diff {
            SchemaDiff::MissingColumn { table, column } => {
                // We don't know the type from the diff alone, so emit a
                // placeholder that the operator must fill in.
                stmts.push(format!(
                    "ALTER TABLE `{table}` ADD COLUMN `{column}` <TYPE> /* TODO: fill in type */;"
                ));
            }
            SchemaDiff::TypeMismatch {
                table,
                column,
                expected,
                ..
            } => {
                stmts.push(format!(
                    "ALTER TABLE `{table}` MODIFY COLUMN `{column}` {expected};"
                ));
            }
            SchemaDiff::NullabilityMismatch {
                table,
                column,
                expected_nullable,
                ..
            } => {
                let constraint = if *expected_nullable {
                    "NULL"
                } else {
                    "NOT NULL"
                };
                stmts.push(format!(
                    "ALTER TABLE `{table}` MODIFY COLUMN `{column}` /* type */ {constraint};"
                ));
            }
            // Extra tables/columns and missing tables are not auto-fixable
            // with ALTER.
            _ => {}
        }
    }

    stmts
}

// ---------------------------------------------------------------------------
// Public entry point (async, requires a real DB connection in the future)
// ---------------------------------------------------------------------------

/// Errors that can occur during schema validation.
#[derive(Debug)]
pub enum ValidateError {
    /// The connection string scheme is not supported yet.
    UnsupportedScheme(String),
    /// A database driver error (placeholder for real sqlx errors later).
    Database(String),
}

impl fmt::Display for ValidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedScheme(s) => write!(f, "unsupported database scheme: {s}"),
            Self::Database(msg) => write!(f, "database error: {msg}"),
        }
    }
}

impl std::error::Error for ValidateError {}

/// Validate the database at `connection_string` against the expected schema.
///
/// Currently only the comparison engine is implemented.  Real database
/// introspection (MySQL, PostgreSQL, SQLite via sqlx) will be added once the
/// `sqlx` dependency is wired in.
pub async fn validate_schema(connection_string: &str) -> Result<ValidationReport, ValidateError> {
    let scheme = connection_string
        .split("://")
        .next()
        .unwrap_or("")
        .to_lowercase();

    match scheme.as_str() {
        "sqlite" | "mysql" | "postgres" | "postgresql" => {
            // Stub: in production this will connect via sqlx, run
            // information_schema / PRAGMA queries, build ActualSchema, then
            // call compare_schemas.
            Err(ValidateError::Database(format!(
                "live database introspection for {scheme}:// is not yet implemented; \
                 use compare_schemas() directly for testing"
            )))
        }
        _ => Err(ValidateError::UnsupportedScheme(scheme)),
    }
}

// ---------------------------------------------------------------------------
// Expected schema definition for OpenSimulator core tables
// ---------------------------------------------------------------------------

fn col(name: &str, col_type: &str, nullable: bool) -> ColumnDef {
    ColumnDef {
        name: name.to_string(),
        col_type: col_type.to_string(),
        nullable,
    }
}

fn table(name: &str, columns: Vec<ColumnDef>) -> TableDef {
    TableDef {
        name: name.to_string(),
        columns,
    }
}

/// Build the expected schema for core OpenSimulator tables.
pub fn expected_schema() -> ExpectedSchema {
    ExpectedSchema {
        tables: vec![
            table(
                "UserAccounts",
                vec![
                    col("PrincipalID", "CHAR(36)", false),
                    col("ScopeID", "CHAR(36)", true),
                    col("FirstName", "VARCHAR(64)", false),
                    col("LastName", "VARCHAR(64)", false),
                    col("Email", "VARCHAR(64)", true),
                    col("ServiceURLs", "TEXT", true),
                    col("Created", "INTEGER", true),
                    col("UserLevel", "INTEGER", true),
                    col("UserFlags", "INTEGER", true),
                    col("UserTitle", "VARCHAR(64)", true),
                ],
            ),
            table(
                "auth",
                vec![
                    col("UUID", "CHAR(36)", false),
                    col("passwordHash", "CHAR(32)", false),
                    col("passwordSalt", "CHAR(32)", false),
                    col("webLoginKey", "CHAR(36)", true),
                    col("accountType", "VARCHAR(32)", true),
                ],
            ),
            table(
                "tokens",
                vec![
                    col("UUID", "CHAR(36)", false),
                    col("token", "VARCHAR(255)", false),
                    col("validity", "DATETIME", false),
                ],
            ),
            table(
                "Presence",
                vec![
                    col("UserID", "VARCHAR(255)", false),
                    col("RegionID", "CHAR(36)", false),
                    col("SessionID", "CHAR(36)", false),
                    col("SecureSessionID", "CHAR(36)", false),
                ],
            ),
            table(
                "GridUser",
                vec![
                    col("UserID", "VARCHAR(255)", false),
                    col("HomeRegionID", "CHAR(36)", true),
                    col("HomePosition", "CHAR(64)", true),
                    col("HomeLookAt", "CHAR(64)", true),
                    col("LastRegionID", "CHAR(36)", true),
                    col("LastPosition", "CHAR(64)", true),
                    col("LastLookAt", "CHAR(64)", true),
                    col("Online", "CHAR(5)", true),
                    col("Login", "CHAR(16)", true),
                    col("Logout", "CHAR(16)", true),
                ],
            ),
            table(
                "inventoryfolders",
                vec![
                    col("folderName", "VARCHAR(64)", true),
                    col("type", "SMALLINT", false),
                    col("version", "INTEGER", true),
                    col("folderID", "CHAR(36)", false),
                    col("agentID", "CHAR(36)", true),
                    col("parentFolderID", "CHAR(36)", true),
                ],
            ),
            table(
                "inventoryitems",
                vec![
                    col("assetID", "CHAR(36)", true),
                    col("assetType", "INTEGER", true),
                    col("inventoryName", "VARCHAR(64)", true),
                    col("inventoryDescription", "VARCHAR(128)", true),
                    col("inventoryNextPermissions", "INTEGER", true),
                    col("inventoryCurrentPermissions", "INTEGER", true),
                    col("invType", "INTEGER", true),
                    col("creatorID", "VARCHAR(128)", true),
                    col("inventoryBasePermissions", "INTEGER", true),
                    col("inventoryEveryOnePermissions", "INTEGER", true),
                    col("salePrice", "INTEGER", true),
                    col("saleType", "TINYINT", true),
                    col("creationDate", "INTEGER", true),
                    col("groupID", "CHAR(36)", true),
                    col("groupOwned", "BOOLEAN", true),
                    col("flags", "INTEGER", true),
                    col("inventoryID", "CHAR(36)", false),
                    col("avatarID", "CHAR(36)", true),
                    col("parentFolderID", "CHAR(36)", true),
                    col("inventoryGroupPermissions", "INTEGER", true),
                ],
            ),
            table(
                "assets",
                vec![
                    col("name", "VARCHAR(64)", true),
                    col("description", "VARCHAR(64)", true),
                    col("assetType", "TINYINT", false),
                    col("local", "BOOLEAN", false),
                    col("temporary", "BOOLEAN", false),
                    col("data", "LONGBLOB", false),
                    col("id", "CHAR(36)", false),
                    col("create_time", "INTEGER", true),
                    col("access_time", "INTEGER", true),
                    col("asset_flags", "INTEGER", true),
                    col("CreatorID", "VARCHAR(128)", true),
                ],
            ),
            table(
                "regions",
                vec![
                    col("uuid", "CHAR(36)", false),
                    col("regionHandle", "BIGINT", true),
                    col("regionName", "VARCHAR(128)", true),
                    col("regionDataURI", "VARCHAR(255)", true),
                    col("serverIP", "VARCHAR(64)", true),
                    col("serverPort", "INTEGER", true),
                    col("serverURI", "VARCHAR(255)", true),
                    col("locX", "INTEGER", true),
                    col("locY", "INTEGER", true),
                    col("locZ", "INTEGER", true),
                    col("eastOverrideHandle", "BIGINT", true),
                    col("westOverrideHandle", "BIGINT", true),
                    col("southOverrideHandle", "BIGINT", true),
                    col("northOverrideHandle", "BIGINT", true),
                    col("regionAssetURI", "VARCHAR(255)", true),
                    col("regionAssetRecvKey", "VARCHAR(128)", true),
                    col("regionAssetSendKey", "VARCHAR(128)", true),
                    col("regionUserURI", "VARCHAR(255)", true),
                    col("regionUserRecvKey", "VARCHAR(128)", true),
                    col("regionUserSendKey", "VARCHAR(128)", true),
                    col("regionMapTexture", "CHAR(36)", true),
                    col("serverHttpPort", "INTEGER", true),
                    col("serverRemotingPort", "INTEGER", true),
                    col("owner_uuid", "CHAR(36)", true),
                    col("PrincipalID", "CHAR(36)", true),
                    col("Token", "VARCHAR(255)", true),
                    col("sizeX", "INTEGER", true),
                    col("sizeY", "INTEGER", true),
                    col("flags", "INTEGER", true),
                    col("last_seen", "INTEGER", true),
                    col("parcelMapTexture", "CHAR(36)", true),
                ],
            ),
            table(
                "estate_settings",
                vec![
                    col("EstateID", "INTEGER", false),
                    col("EstateName", "VARCHAR(64)", true),
                    col("AbuseEmailToEstateOwner", "BOOLEAN", true),
                    col("DenyAnonymous", "BOOLEAN", true),
                    col("ResetHomeOnTeleport", "BOOLEAN", true),
                    col("FixedSun", "BOOLEAN", true),
                    col("DenyTransacted", "BOOLEAN", true),
                    col("BlockDwell", "BOOLEAN", true),
                    col("DenyIdentified", "BOOLEAN", true),
                    col("AllowVoice", "BOOLEAN", true),
                    col("UseGlobalTime", "BOOLEAN", true),
                    col("PricePerMeter", "INTEGER", true),
                    col("TaxFree", "BOOLEAN", true),
                    col("AllowDirectTeleport", "BOOLEAN", true),
                    col("RedirectGridX", "INTEGER", true),
                    col("RedirectGridY", "INTEGER", true),
                    col("ParentEstateID", "INTEGER", true),
                    col("SunPosition", "DOUBLE", true),
                    col("EstateSkipScripts", "BOOLEAN", true),
                    col("BillableFactor", "FLOAT", true),
                    col("PublicAccess", "BOOLEAN", true),
                    col("AbuseEmail", "VARCHAR(255)", true),
                    col("EstateOwner", "CHAR(36)", true),
                    col("DenyMinors", "BOOLEAN", true),
                    col("AllowLandmark", "BOOLEAN", true),
                    col("AllowParcelChanges", "BOOLEAN", true),
                    col("AllowSetHome", "BOOLEAN", true),
                ],
            ),
            table(
                "estate_map",
                vec![
                    col("RegionID", "CHAR(36)", false),
                    col("EstateID", "INTEGER", false),
                ],
            ),
            table(
                "os_groups_groups",
                vec![
                    col("GroupID", "CHAR(36)", false),
                    col("Location", "VARCHAR(255)", true),
                    col("Name", "VARCHAR(255)", true),
                    col("Charter", "TEXT", true),
                    col("InsigniaID", "CHAR(36)", true),
                    col("FounderID", "CHAR(36)", true),
                    col("MembershipFee", "INTEGER", true),
                    col("OpenEnrollment", "VARCHAR(255)", true),
                    col("ShowInList", "BOOLEAN", true),
                    col("AllowPublish", "BOOLEAN", true),
                    col("MaturePublish", "BOOLEAN", true),
                    col("OwnerRoleID", "CHAR(36)", true),
                ],
            ),
            table(
                "os_groups_membership",
                vec![
                    col("GroupID", "CHAR(36)", false),
                    col("PrincipalID", "VARCHAR(255)", false),
                    col("SelectedRoleID", "CHAR(36)", true),
                    col("Contribution", "INTEGER", true),
                    col("ListInProfile", "BOOLEAN", true),
                    col("AcceptNotices", "BOOLEAN", true),
                    col("AccessToken", "CHAR(36)", true),
                ],
            ),
            table(
                "os_groups_roles",
                vec![
                    col("GroupID", "CHAR(36)", false),
                    col("RoleID", "CHAR(36)", false),
                    col("Name", "VARCHAR(255)", true),
                    col("Description", "VARCHAR(255)", true),
                    col("Title", "VARCHAR(255)", true),
                    col("Powers", "BIGINT", true),
                ],
            ),
            table(
                "Friends",
                vec![
                    col("PrincipalID", "VARCHAR(255)", false),
                    col("Friend", "VARCHAR(255)", false),
                    col("Flags", "VARCHAR(16)", true),
                    col("Offered", "VARCHAR(32)", true),
                ],
            ),
            table(
                "Avatars",
                vec![
                    col("PrincipalID", "CHAR(36)", false),
                    col("Name", "VARCHAR(32)", false),
                    col("Value", "TEXT", true),
                ],
            ),
            table(
                "land",
                vec![
                    col("UUID", "CHAR(36)", false),
                    col("RegionUUID", "CHAR(36)", true),
                    col("LocalLandID", "INTEGER", true),
                    col("Bitmap", "LONGBLOB", true),
                    col("Name", "VARCHAR(255)", true),
                    col("Description", "VARCHAR(255)", true),
                    col("OwnerUUID", "CHAR(36)", true),
                    col("IsGroupOwned", "BOOLEAN", true),
                    col("Area", "INTEGER", true),
                    col("AuctionID", "INTEGER", true),
                    col("Category", "TINYINT", true),
                    col("ClaimDate", "INTEGER", true),
                    col("ClaimPrice", "INTEGER", true),
                    col("GroupUUID", "CHAR(36)", true),
                    col("SalePrice", "INTEGER", true),
                    col("LandStatus", "TINYINT", true),
                    col("LandFlags", "INTEGER", true),
                    col("LandingType", "TINYINT", true),
                    col("MediaAutoScale", "TINYINT", true),
                    col("MediaTextureUUID", "CHAR(36)", true),
                    col("MediaURL", "VARCHAR(255)", true),
                    col("MusicURL", "VARCHAR(255)", true),
                    col("PassHours", "FLOAT", true),
                    col("PassPrice", "INTEGER", true),
                    col("SnapshotUUID", "CHAR(36)", true),
                    col("UserLocationX", "FLOAT", true),
                    col("UserLocationY", "FLOAT", true),
                    col("UserLocationZ", "FLOAT", true),
                    col("UserLookAtX", "FLOAT", true),
                    col("UserLookAtY", "FLOAT", true),
                    col("UserLookAtZ", "FLOAT", true),
                    col("AuthbuyerID", "CHAR(36)", true),
                    col("OtherCleanTime", "INTEGER", true),
                    col("Dwell", "FLOAT", true),
                    col("MediaType", "VARCHAR(32)", true),
                    col("MediaDescription", "VARCHAR(255)", true),
                    col("MediaSize", "VARCHAR(16)", true),
                    col("MediaLoop", "BOOLEAN", true),
                    col("ObscureMusic", "BOOLEAN", true),
                    col("ObscureMedia", "BOOLEAN", true),
                    col("SeeAVs", "BOOLEAN", true),
                    col("AnyAVSounds", "BOOLEAN", true),
                    col("GroupAVSounds", "BOOLEAN", true),
                ],
            ),
            table(
                "landaccesslist",
                vec![
                    col("LandUUID", "CHAR(36)", true),
                    col("AccessUUID", "CHAR(36)", true),
                    col("Flags", "INTEGER", true),
                    col("Expires", "INTEGER", true),
                ],
            ),
        ],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal actual schema from a list of (table_name, columns).
    fn actual(tables: Vec<(&str, Vec<ColumnDef>)>) -> ActualSchema {
        ActualSchema {
            tables: tables
                .into_iter()
                .map(|(name, columns)| TableDef {
                    name: name.to_string(),
                    columns,
                })
                .collect(),
        }
    }

    #[test]
    fn identical_schemas_report_no_diffs() {
        let expected = ExpectedSchema {
            tables: vec![table("Foo", vec![col("id", "INTEGER", false)])],
        };
        let actual = actual(vec![("Foo", vec![col("id", "INTEGER", false)])]);
        let report = compare_schemas(&expected, &actual);
        assert!(report.is_ok(), "expected no diffs: {report}");
    }

    #[test]
    fn missing_table_detected() {
        let expected = ExpectedSchema {
            tables: vec![
                table("Foo", vec![col("id", "INTEGER", false)]),
                table("Bar", vec![col("id", "INTEGER", false)]),
            ],
        };
        let actual = actual(vec![("Foo", vec![col("id", "INTEGER", false)])]);
        let report = compare_schemas(&expected, &actual);
        assert!(!report.is_ok());
        assert!(
            report
                .diffs
                .contains(&SchemaDiff::MissingTable("bar".to_string())),
            "should detect missing table Bar: {report}"
        );
    }

    #[test]
    fn extra_table_detected() {
        let expected = ExpectedSchema {
            tables: vec![table("Foo", vec![col("id", "INTEGER", false)])],
        };
        let actual = actual(vec![
            ("Foo", vec![col("id", "INTEGER", false)]),
            ("Unexpected", vec![col("x", "TEXT", true)]),
        ]);
        let report = compare_schemas(&expected, &actual);
        assert!(
            report
                .diffs
                .contains(&SchemaDiff::ExtraTable("unexpected".to_string())),
            "should detect extra table: {report}"
        );
    }

    #[test]
    fn missing_column_detected() {
        let expected = ExpectedSchema {
            tables: vec![table(
                "Foo",
                vec![col("id", "INTEGER", false), col("name", "TEXT", true)],
            )],
        };
        let actual = actual(vec![("Foo", vec![col("id", "INTEGER", false)])]);
        let report = compare_schemas(&expected, &actual);
        assert!(
            report.diffs.iter().any(|d| matches!(
                d,
                SchemaDiff::MissingColumn { table, column }
                    if table == "foo" && column == "name"
            )),
            "should detect missing column: {report}"
        );
    }

    #[test]
    fn extra_column_detected() {
        let expected = ExpectedSchema {
            tables: vec![table("Foo", vec![col("id", "INTEGER", false)])],
        };
        let actual = actual(vec![(
            "Foo",
            vec![col("id", "INTEGER", false), col("extra", "TEXT", true)],
        )]);
        let report = compare_schemas(&expected, &actual);
        assert!(
            report.diffs.iter().any(|d| matches!(
                d,
                SchemaDiff::ExtraColumn { table, column }
                    if table == "foo" && column == "extra"
            )),
            "should detect extra column: {report}"
        );
    }

    #[test]
    fn type_mismatch_detected() {
        let expected = ExpectedSchema {
            tables: vec![table("Foo", vec![col("val", "VARCHAR(64)", false)])],
        };
        let actual = actual(vec![("Foo", vec![col("val", "TEXT", false)])]);
        let report = compare_schemas(&expected, &actual);
        assert!(
            report.diffs.iter().any(|d| matches!(
                d,
                SchemaDiff::TypeMismatch { table, column, expected, actual }
                    if table == "foo" && column == "val"
                        && expected == "VARCHAR(64)" && actual == "TEXT"
            )),
            "should detect type mismatch: {report}"
        );
    }

    #[test]
    fn nullability_mismatch_detected() {
        let expected = ExpectedSchema {
            tables: vec![table("Foo", vec![col("id", "INTEGER", false)])],
        };
        let actual = actual(vec![("Foo", vec![col("id", "INTEGER", true)])]);
        let report = compare_schemas(&expected, &actual);
        assert!(
            report.diffs.iter().any(|d| matches!(
                d,
                SchemaDiff::NullabilityMismatch {
                    table,
                    column,
                    expected_nullable: false,
                    actual_nullable: true,
                } if table == "foo" && column == "id"
            )),
            "should detect nullability mismatch: {report}"
        );
    }

    #[test]
    fn type_alias_int_matches_integer() {
        let expected = ExpectedSchema {
            tables: vec![table("Foo", vec![col("n", "INT", false)])],
        };
        let actual = actual(vec![("Foo", vec![col("n", "INTEGER", false)])]);
        let report = compare_schemas(&expected, &actual);
        assert!(
            report.is_ok(),
            "INT and INTEGER should be equivalent: {report}"
        );
    }

    #[test]
    fn case_insensitive_table_and_column_names() {
        let expected = ExpectedSchema {
            tables: vec![table(
                "UserAccounts",
                vec![col("PrincipalID", "CHAR(36)", false)],
            )],
        };
        let actual = actual(vec![(
            "useraccounts",
            vec![col("principalid", "CHAR(36)", false)],
        )]);
        let report = compare_schemas(&expected, &actual);
        assert!(
            report.is_ok(),
            "comparison should be case-insensitive: {report}"
        );
    }

    #[test]
    fn generate_alters_for_missing_column() {
        let report = ValidationReport {
            diffs: vec![SchemaDiff::MissingColumn {
                table: "auth".to_string(),
                column: "argon2Hash".to_string(),
            }],
        };
        let stmts = generate_alter_statements(&report);
        assert_eq!(stmts.len(), 1);
        assert!(
            stmts[0].contains("ADD COLUMN"),
            "should produce ADD COLUMN: {}",
            stmts[0]
        );
        assert!(
            stmts[0].contains("argon2Hash"),
            "should name the column: {}",
            stmts[0]
        );
    }

    #[test]
    fn generate_alters_for_type_mismatch() {
        let report = ValidationReport {
            diffs: vec![SchemaDiff::TypeMismatch {
                table: "assets".to_string(),
                column: "data".to_string(),
                expected: "LONGBLOB".to_string(),
                actual: "BLOB".to_string(),
            }],
        };
        let stmts = generate_alter_statements(&report);
        assert_eq!(stmts.len(), 1);
        assert!(
            stmts[0].contains("MODIFY COLUMN"),
            "should produce MODIFY COLUMN: {}",
            stmts[0]
        );
        assert!(
            stmts[0].contains("LONGBLOB"),
            "should use expected type: {}",
            stmts[0]
        );
    }

    #[test]
    fn display_impl_no_diffs() {
        let report = ValidationReport { diffs: vec![] };
        let s = format!("{report}");
        assert!(s.contains("passed"), "empty report should say passed: {s}");
    }

    #[test]
    fn expected_schema_covers_core_tables() {
        let schema = expected_schema();
        let names: Vec<String> = schema
            .tables
            .iter()
            .map(|t| t.name.to_lowercase())
            .collect();
        let required = [
            "useraccounts",
            "auth",
            "tokens",
            "presence",
            "griduser",
            "inventoryfolders",
            "inventoryitems",
            "assets",
            "regions",
            "estate_settings",
            "estate_map",
            "os_groups_groups",
            "os_groups_membership",
            "os_groups_roles",
            "friends",
            "avatars",
            "land",
            "landaccesslist",
        ];
        for req in required {
            assert!(
                names.contains(&req.to_string()),
                "expected schema must contain table {req}"
            );
        }
    }

    #[test]
    fn blob_aliases_match() {
        assert!(types_match("BLOB", "LONGBLOB"));
        assert!(types_match("MEDIUMBLOB", "BLOB"));
        assert!(types_match("LONGBLOB", "MEDIUMBLOB"));
    }

    #[test]
    fn boolean_aliases_match() {
        assert!(types_match("BOOL", "BOOLEAN"));
        assert!(types_match("TINYINT(1)", "BOOLEAN"));
    }
}
