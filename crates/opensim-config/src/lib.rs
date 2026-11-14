use std::collections::BTreeMap;
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Clone, Default)]
pub struct Config {
    sections: BTreeMap<String, BTreeMap<String, toml::Value>>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unresolved substitution: ${{{0}|{1}}}")]
    UnresolvedSubstitution(String, String),
}

impl Config {
    pub fn from_toml_file(path: &std::path::Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let table: toml::Table = toml::from_str(&contents)?;
        let mut cfg = Config::default();
        for (key, value) in table {
            match value {
                toml::Value::Table(t) => {
                    let mut sec = BTreeMap::new();
                    for (k, v) in t {
                        sec.insert(k, v);
                    }
                    cfg.sections.insert(key, sec);
                }
                other => {
                    cfg.sections
                        .entry("General".to_string())
                        .or_default()
                        .insert(key, other);
                }
            }
        }
        Ok(cfg)
    }

    pub fn merge(&mut self, other: &Config) {
        for (section, kv) in &other.sections {
            let target = self.sections.entry(section.clone()).or_default();
            for (k, v) in kv {
                target.insert(k.clone(), v.clone());
            }
        }
    }

    pub fn apply_env_overrides(&mut self) {
        for (var, value) in std::env::vars() {
            if let Some(rest) = var.strip_prefix("OPENSIM__")
                && let Some((section, key)) = rest.split_once("__")
            {
                let parsed = if let Ok(b) = value.parse::<bool>() {
                    toml::Value::Boolean(b)
                } else if let Ok(i) = value.parse::<i64>() {
                    toml::Value::Integer(i)
                } else if let Ok(f) = value.parse::<f64>() {
                    toml::Value::Float(f)
                } else {
                    toml::Value::String(value.clone())
                };
                debug!(
                    "Config override from env: {} -> [{}] {} = {:?}",
                    var, section, key, parsed
                );
                self.sections
                    .entry(section.to_string())
                    .or_default()
                    .insert(key.to_string(), parsed);
            }
        }
    }

    pub fn resolve_substitutions(&mut self) -> Result<(), ConfigError> {
        for _pass in 0..10 {
            let snapshot = self.sections.clone();
            let mut any_replaced = false;

            for (sec, kv) in &mut self.sections {
                for (key, val) in kv.iter_mut() {
                    if let toml::Value::String(s) = val {
                        if !s.contains("${") {
                            continue;
                        }
                        match replace_substitutions(&snapshot, s) {
                            Ok(new) => {
                                if new != *s {
                                    any_replaced = true;
                                    *s = new;
                                }
                            }
                            Err(ConfigError::UnresolvedSubstitution(a, b)) => {
                                return Err(ConfigError::UnresolvedSubstitution(a, b));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    let _ = (sec, key);
                }
            }

            if !any_replaced {
                return Ok(());
            }
        }
        Ok(())
    }

    pub fn get_string(&self, section: &str, key: &str) -> Option<&str> {
        self.sections.get(section)?.get(key)?.as_str()
    }

    pub fn get_bool(&self, section: &str, key: &str) -> Option<bool> {
        self.sections.get(section)?.get(key)?.as_bool()
    }

    pub fn get_i64(&self, section: &str, key: &str) -> Option<i64> {
        self.sections.get(section)?.get(key)?.as_integer()
    }

    pub fn get_f64(&self, section: &str, key: &str) -> Option<f64> {
        self.sections.get(section)?.get(key)?.as_float()
    }

    pub fn get_string_or(&self, section: &str, key: &str, default: &str) -> String {
        self.get_string(section, key)
            .map(|s| s.to_string())
            .unwrap_or_else(|| default.to_string())
    }

    pub fn get_bool_or(&self, section: &str, key: &str, default: bool) -> bool {
        self.get_bool(section, key).unwrap_or(default)
    }

    pub fn get_i64_or(&self, section: &str, key: &str, default: i64) -> i64 {
        self.get_i64(section, key).unwrap_or(default)
    }

    pub fn get_f64_or(&self, section: &str, key: &str, default: f64) -> f64 {
        self.get_f64(section, key).unwrap_or(default)
    }

    pub fn sections(&self) -> impl Iterator<Item = &str> + '_ {
        self.sections.keys().map(|s| s.as_str())
    }

    pub fn keys(&self, section: &str) -> Option<impl Iterator<Item = &str> + '_> {
        self.sections
            .get(section)
            .map(|m| m.keys().map(|k| k.as_str()))
    }

    fn get_value(&self, section: &str, key: &str) -> Option<&toml::Value> {
        self.sections.get(section)?.get(key)
    }
}

pub fn import_ini(path: &std::path::Path) -> Result<Config, ConfigError> {
    let mut ini = configparser::ini::Ini::new();
    ini.load(
        path.to_str()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path"))?,
    )
    .map_err(|e| {
        ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    let mut cfg = Config::default();
    let map = ini.get_map_ref();
    for (section, keys) in map {
        let sec = cfg.sections.entry(section.clone()).or_default();
        for (k, v) in keys {
            let raw = v.clone().unwrap_or_default();
            // configparser lowercases keys; Include- directives need case-insensitive check
            let value = if k.starts_with("include-") {
                toml::Value::String(raw)
            } else {
                infer_value(&raw)
            };
            sec.insert(k.clone(), value);
        }
    }
    Ok(cfg)
}

fn infer_value(v: &str) -> toml::Value {
    match v {
        "true" | "True" => toml::Value::Boolean(true),
        "false" | "False" => toml::Value::Boolean(false),
        _ => {
            if let Ok(i) = v.parse::<i64>() {
                toml::Value::Integer(i)
            } else if let Ok(f) = v.parse::<f64>() {
                toml::Value::Float(f)
            } else {
                toml::Value::String(v.to_string())
            }
        }
    }
}

pub fn write_toml(config: &Config) -> String {
    let mut out = String::new();
    for (section, kv) in &config.sections {
        let header = if section.contains('.') || section.contains('-') {
            format!("[\"{}\"]", escape(section))
        } else {
            format!("[{}]", section)
        };
        out.push_str(&header);
        out.push('\n');
        for (k, v) in kv {
            let key = if is_bare_key(k) {
                k.clone()
            } else {
                format!("\"{}\"", escape(k))
            };
            out.push_str(&format!("{} = {}\n", key, v));
        }
        out.push('\n');
    }
    out
}

fn replace_substitutions(
    snapshot: &BTreeMap<String, BTreeMap<String, toml::Value>>,
    input: &str,
) -> Result<String, ConfigError> {
    let mut result = String::new();
    let mut i = 0;
    while i < input.len() {
        if input[i..].starts_with("${")
            && let Some(rel_end) = input[i..].find('}')
        {
            let end = i + rel_end;
            let inner = &input[i + 2..end];
            if let Some((sec, key)) = inner.split_once('|') {
                let sec = sec.trim();
                let key = key.trim();
                if let Some(v) = snapshot.get(sec).and_then(|m| m.get(key)) {
                    result.push_str(&toml_value_to_string(v));
                    i = end + 1;
                    continue;
                } else {
                    return Err(ConfigError::UnresolvedSubstitution(
                        sec.to_string(),
                        key.to_string(),
                    ));
                }
            }
        }
        let ch = input[i..].chars().next().unwrap();
        result.push(ch);
        i += ch.len_utf8();
    }
    Ok(result)
}

fn toml_value_to_string(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn is_bare_key(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
