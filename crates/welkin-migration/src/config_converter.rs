// SPDX-License-Identifier: LGPL-3.0-or-later

//! Converts C# OpenSimulator INI configuration files to Rust-compatible TOML.

use std::collections::BTreeMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ConfigConvertError {
    /// A section header was opened but never closed (missing `]`).
    MalformedSection { line_number: usize, line: String },
    /// A key=value line appeared before any section header.
    OrphanKey { line_number: usize, line: String },
}

impl fmt::Display for ConfigConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedSection { line_number, line } => {
                write!(f, "line {line_number}: malformed section header: {line}")
            }
            Self::OrphanKey { line_number, line } => {
                write!(
                    f,
                    "line {line_number}: key=value before any section header: {line}"
                )
            }
        }
    }
}

impl std::error::Error for ConfigConvertError {}

// ---------------------------------------------------------------------------
// INI model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct IniEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct IniSection {
    pub name: String,
    pub entries: Vec<IniEntry>,
}

// ---------------------------------------------------------------------------
// Conversion report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ConversionReport {
    /// Converted sections keyed by their TOML path (e.g. `"services.grid"`).
    pub sections: BTreeMap<String, BTreeMap<String, String>>,
    /// Entries that were skipped because they are unsupported in the Rust build.
    pub skipped: Vec<String>,
    /// Warnings about deprecated settings.
    pub warnings: Vec<String>,
    /// Total number of key=value pairs that were converted.
    pub converted_count: usize,
}

impl fmt::Display for ConversionReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Converted {} settings across {} sections ({} skipped, {} warnings)",
            self.converted_count,
            self.sections.len(),
            self.skipped.len(),
            self.warnings.len(),
        )
    }
}

impl ConversionReport {
    /// Render the converted configuration as a TOML string.
    pub fn to_toml(&self) -> String {
        let mut out = String::new();

        for (section, entries) in &self.sections {
            // Dotted section names need quoting if they contain a dot — TOML
            // handles `[a.b]` as nested tables which is exactly what we want.
            out.push_str(&format!("[{section}]\n"));
            for (k, v) in entries {
                out.push_str(&format!("{k} = {}\n", toml_value_string(v)));
            }
            out.push('\n');
        }

        out
    }
}

/// Wrap a value as a TOML literal.  Booleans and integers are left bare;
/// everything else is double-quoted.
fn toml_value_string(v: &str) -> String {
    // Boolean
    if v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("false") {
        return v.to_lowercase();
    }
    // Integer (including negative)
    if v.parse::<i64>().is_ok() {
        return v.to_string();
    }
    // Float
    if v.parse::<f64>().is_ok() && v.contains('.') {
        return v.to_string();
    }
    // Everything else → quoted string, escaping embedded quotes and backslashes.
    let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

// ---------------------------------------------------------------------------
// Section name mapping
// ---------------------------------------------------------------------------

/// Map a C# INI section name to its Rust TOML equivalent.
fn map_section_name(cs_name: &str) -> String {
    match cs_name {
        "Startup" => "general".to_string(),
        "Network" => "network".to_string(),
        "DatabaseService" => "database".to_string(),
        "GridService" => "services.grid".to_string(),
        "AssetService" => "services.asset".to_string(),
        "InventoryService" => "services.inventory".to_string(),
        "AuthenticationService" => "services.authentication".to_string(),
        "UserAccountService" => "services.user_account".to_string(),
        "PresenceService" => "services.presence".to_string(),
        "AvatarService" => "services.avatar".to_string(),
        "FriendsService" => "services.friends".to_string(),
        "GridUserService" => "services.grid_user".to_string(),
        "MapImageService" => "services.map_image".to_string(),
        "Hypergrid" => "hypergrid".to_string(),
        "Messaging" => "messaging".to_string(),
        other => format!("legacy.{other}"),
    }
}

// ---------------------------------------------------------------------------
// Unsupported / deprecated detection
// ---------------------------------------------------------------------------

/// Returns `true` if the key/value represents a C#-specific / unsupported option.
fn is_unsupported_entry(key: &str, value: &str) -> bool {
    let lower_key = key.to_ascii_lowercase();
    let lower_val = value.to_ascii_lowercase();
    lower_key.contains("dll")
        || lower_val.contains(".dll")
        || key.contains("Assembly")
        || key.contains("Plugin")
        || key.starts_with("Mono.")
        || key.starts_with("System.")
}

const DEPRECATED_KEYS: &[&str] = &["gridmode", "standalone", "physics_plugin"];

fn is_deprecated_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    DEPRECATED_KEYS.iter().any(|d| lower == *d)
}

// ---------------------------------------------------------------------------
// INI parser
// ---------------------------------------------------------------------------

/// Parse a C# OpenSimulator INI string into a list of sections.
///
/// Handles:
/// - `[Section]` headers
/// - `Key = Value` pairs (with or without spaces around `=`)
/// - Comment lines starting with `;` or `#`
/// - Inline comments (`;` or `#` after a value, unless inside quotes)
/// - Blank lines
/// - Quoted values (`"..."`)
/// - Whitespace trimming
pub fn parse_ini(content: &str) -> Result<Vec<IniSection>, ConfigConvertError> {
    let mut sections: Vec<IniSection> = Vec::new();
    let mut current: Option<IniSection> = None;

    for (idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();

        // Skip blank lines
        if line.is_empty() {
            continue;
        }

        // Skip full-line comments
        if line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        // Section header
        if line.starts_with('[') {
            let end = line
                .find(']')
                .ok_or_else(|| ConfigConvertError::MalformedSection {
                    line_number: idx + 1,
                    line: raw_line.to_string(),
                })?;
            let name = line[1..end].trim().to_string();

            if let Some(sec) = current.take() {
                sections.push(sec);
            }
            current = Some(IniSection {
                name,
                entries: Vec::new(),
            });
            continue;
        }

        // Key = Value
        if let Some(eq_pos) = line.find('=') {
            let sec = current
                .as_mut()
                .ok_or_else(|| ConfigConvertError::OrphanKey {
                    line_number: idx + 1,
                    line: raw_line.to_string(),
                })?;

            let key = line[..eq_pos].trim().to_string();
            let raw_value = line[eq_pos + 1..].trim();
            let value = strip_inline_comment(raw_value);

            sec.entries.push(IniEntry { key, value });
        }
        // Lines that don't match any pattern are silently skipped (e.g. bare
        // words used as comments in some configs).
    }

    if let Some(sec) = current.take() {
        sections.push(sec);
    }

    Ok(sections)
}

/// Remove an inline comment from a value string, respecting double quotes.
fn strip_inline_comment(raw: &str) -> String {
    let mut in_quotes = false;
    let mut end = raw.len();

    for (i, ch) in raw.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if !in_quotes && (ch == ';' || ch == '#') {
            end = i;
            break;
        }
    }

    let trimmed = raw[..end].trim();

    // Strip surrounding quotes if present.
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Convert the contents of a C# OpenSimulator INI file to a
/// [`ConversionReport`] containing the TOML-compatible representation.
pub fn convert_config(ini_content: &str) -> Result<ConversionReport, ConfigConvertError> {
    let ini_sections = parse_ini(ini_content)?;

    let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut converted_count: usize = 0;

    for sec in &ini_sections {
        let toml_section = map_section_name(&sec.name);

        for entry in &sec.entries {
            // Check unsupported
            if is_unsupported_entry(&entry.key, &entry.value) {
                skipped.push(format!(
                    "[{}] {} = {} (unsupported C#-specific option)",
                    sec.name, entry.key, entry.value,
                ));
                continue;
            }

            // Check deprecated
            if is_deprecated_key(&entry.key) {
                warnings.push(format!(
                    "[{}] {} is deprecated — review manually",
                    sec.name, entry.key,
                ));
                // Still convert it so the user can see it in context.
            }

            sections
                .entry(toml_section.clone())
                .or_default()
                .insert(entry.key.clone(), entry.value.clone());
            converted_count += 1;
        }
    }

    Ok(ConversionReport {
        sections,
        skipped,
        warnings,
        converted_count,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_ini() {
        let ini = "\
[Startup]
region_name = My Region
; a comment
physics = BulletSim
";
        let sections = parse_ini(ini).expect("parse should succeed");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "Startup");
        assert_eq!(sections[0].entries.len(), 2);
        assert_eq!(sections[0].entries[0].key, "region_name");
        assert_eq!(sections[0].entries[0].value, "My Region");
    }

    #[test]
    fn test_section_mapping() {
        let ini = "\
[Startup]
foo = bar

[GridService]
port = 8003

[AssetService]
url = http://localhost:8003

[Hypergrid]
enabled = true
";
        let report = convert_config(ini).expect("convert should succeed");
        assert!(report.sections.contains_key("general"));
        assert!(report.sections.contains_key("services.grid"));
        assert!(report.sections.contains_key("services.asset"));
        assert!(report.sections.contains_key("hypergrid"));
    }

    #[test]
    fn test_unsupported_detection() {
        let ini = "\
[GridService]
LocalServiceModule = OpenSim.Services.GridService.dll
StorageProvider = OpenSim.Data.MySQL.dll
AllowDuplicateNames = true
";
        let report = convert_config(ini).expect("convert should succeed");
        // The two DLL keys should be skipped.
        assert_eq!(report.skipped.len(), 2);
        // The non-DLL key should be converted.
        assert_eq!(report.converted_count, 1);
        assert!(report.skipped[0].contains("unsupported"));
    }

    #[test]
    fn test_deprecated_warning() {
        let ini = "\
[Startup]
gridmode = true
region_name = Test
";
        let report = convert_config(ini).expect("convert should succeed");
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("deprecated"));
        // Deprecated keys are still converted.
        assert_eq!(report.converted_count, 2);
    }

    #[test]
    fn test_empty_input() {
        let report = convert_config("").expect("convert should succeed");
        assert_eq!(report.converted_count, 0);
        assert!(report.sections.is_empty());
        assert!(report.skipped.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn test_comments_handling() {
        let ini = "\
; File header comment
# Another comment
[Network]
http_listener_port = 9000 ; inline comment
base_url = \"http://example.com\" # another inline
";
        let sections = parse_ini(ini).expect("parse should succeed");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].entries.len(), 2);
        assert_eq!(sections[0].entries[0].value, "9000");
        // Quoted value should have inline comment stripped but quotes removed.
        assert_eq!(sections[0].entries[1].value, "http://example.com");
    }

    #[test]
    fn test_unknown_section_mapping() {
        let ini = "\
[MyCustomModule]
setting = value
";
        let report = convert_config(ini).expect("convert should succeed");
        assert!(report.sections.contains_key("legacy.MyCustomModule"));
        assert_eq!(report.converted_count, 1);
    }

    #[test]
    fn test_full_round_trip() {
        let ini = "\
[Startup]
region_name = My Region
physics = BulletSim

[DatabaseService]
ConnectionString = Data Source=localhost;Database=opensim;

[GridService]
LocalServiceModule = OpenSim.Services.GridService.dll
AllowDuplicateNames = false

[Hypergrid]
enabled = true
";
        let report = convert_config(ini).expect("convert should succeed");
        let toml_output = report.to_toml();

        // Verify the TOML contains expected sections.
        assert!(toml_output.contains("[general]"));
        assert!(toml_output.contains("[database]"));
        assert!(toml_output.contains("[services.grid]"));
        assert!(toml_output.contains("[hypergrid]"));

        // The DLL entry should have been skipped.
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].contains("LocalServiceModule"));

        // Booleans should be rendered as bare TOML booleans.
        assert!(toml_output.contains("enabled = true"));
        assert!(toml_output.contains("AllowDuplicateNames = false"));
    }

    #[test]
    fn test_assembly_and_plugin_keys_skipped() {
        let ini = "\
[AuthenticationService]
AuthenticationServiceAssembly = OpenSim.Services.AuthenticationService
PasswordPlugin = OpenSim.Data.MySQL
timeout = 30
";
        let report = convert_config(ini).expect("convert should succeed");
        assert_eq!(report.skipped.len(), 2);
        assert_eq!(report.converted_count, 1);
        let toml_section = report
            .sections
            .get("services.authentication")
            .expect("section should exist");
        assert!(toml_section.contains_key("timeout"));
    }

    #[test]
    fn test_multiple_sections_same_mapping() {
        // Two different unknown sections should each get their own legacy entry.
        let ini = "\
[FooModule]
a = 1

[BarModule]
b = 2
";
        let report = convert_config(ini).expect("convert should succeed");
        assert!(report.sections.contains_key("legacy.FooModule"));
        assert!(report.sections.contains_key("legacy.BarModule"));
        assert_eq!(report.converted_count, 2);
    }

    #[test]
    fn test_malformed_section_error() {
        let ini = "[Broken\nfoo = bar";
        let err = parse_ini(ini).unwrap_err();
        assert!(err.to_string().contains("malformed section header"));
    }

    #[test]
    fn test_orphan_key_error() {
        let ini = "orphan_key = value\n[Section]\nok = true";
        let err = parse_ini(ini).unwrap_err();
        assert!(err.to_string().contains("key=value before any section"));
    }
}
