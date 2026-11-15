// SPDX-License-Identifier: GPL-3.0-or-later

//! Docker Compose definitions for the wolfvoice sidecar container.

/// Configuration for the wolfvoice Docker service.
#[derive(Debug, Clone)]
pub struct WolfvoiceDockerConfig {
    /// Docker image name (default: "wolfvoice/wolfvoice:latest")
    pub image: String,
    /// Host port to map to container port 8090 (default: 8090)
    pub host_port: u16,
    /// TURN server URL for WebRTC
    pub turn_server: String,
    /// Secret key for authentication
    pub secret_key: String,
    /// Restart policy (default: "unless-stopped")
    pub restart_policy: String,
    /// Additional environment variables
    pub extra_env: Vec<(String, String)>,
}

impl Default for WolfvoiceDockerConfig {
    fn default() -> Self {
        Self {
            image: "wolfvoice/wolfvoice:latest".to_string(),
            host_port: 8090,
            turn_server: String::new(),
            secret_key: String::new(),
            restart_policy: "unless-stopped".to_string(),
            extra_env: Vec::new(),
        }
    }
}

/// Generate a Docker Compose YAML snippet for the wolfvoice service.
///
/// This produces a service definition that can be included in a
/// docker-compose.yml file.
pub fn generate_compose_service(config: &WolfvoiceDockerConfig) -> String {
    let mut yaml = String::new();

    yaml.push_str("  wolfvoice:\n");
    yaml.push_str(&format!("    image: {}\n", config.image));
    yaml.push_str("    ports:\n");
    yaml.push_str(&format!("      - \"{}:8090\"\n", config.host_port));
    yaml.push_str("    environment:\n");
    yaml.push_str(&format!("      - TURN_SERVER={}\n", config.turn_server));
    yaml.push_str(&format!("      - SECRET_KEY={}\n", config.secret_key));

    for (key, value) in &config.extra_env {
        yaml.push_str(&format!("      - {}={}\n", key, value));
    }

    yaml.push_str("    healthcheck:\n");
    yaml.push_str("      test: [\"CMD\", \"curl\", \"-f\", \"http://localhost:8090/health\"]\n");
    yaml.push_str("      interval: 30s\n");
    yaml.push_str("      timeout: 10s\n");
    yaml.push_str("      retries: 3\n");
    yaml.push_str(&format!("    restart: {}\n", config.restart_policy));

    yaml
}

/// Generate a complete Docker Compose file with the wolfvoice service.
pub fn generate_compose_file(config: &WolfvoiceDockerConfig) -> String {
    let mut yaml = String::new();
    yaml.push_str("version: \"3.8\"\n");
    yaml.push_str("services:\n");
    yaml.push_str(&generate_compose_service(config));
    yaml
}

/// Validate a Docker Compose configuration.
pub fn validate_config(config: &WolfvoiceDockerConfig) -> Result<(), String> {
    if config.image.is_empty() {
        return Err("image must not be empty".to_string());
    }
    if config.host_port == 0 {
        return Err("host_port must be greater than 0".to_string());
    }
    if config.secret_key.is_empty() {
        return Err("secret_key must not be empty".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = WolfvoiceDockerConfig::default();
        assert_eq!(config.image, "wolfvoice/wolfvoice:latest");
        assert_eq!(config.host_port, 8090);
        assert!(config.turn_server.is_empty());
        assert!(config.secret_key.is_empty());
        assert_eq!(config.restart_policy, "unless-stopped");
        assert!(config.extra_env.is_empty());
    }

    #[test]
    fn generate_compose_service_contains_expected_elements() {
        let config = WolfvoiceDockerConfig {
            image: "wolfvoice/wolfvoice:1.0".to_string(),
            host_port: 9090,
            turn_server: "turn:example.com:3478".to_string(),
            secret_key: "my-secret".to_string(),
            restart_policy: "always".to_string(),
            extra_env: vec![("LOG_LEVEL".to_string(), "debug".to_string())],
        };

        let yaml = generate_compose_service(&config);

        assert!(yaml.contains("wolfvoice:"));
        assert!(yaml.contains("image: wolfvoice/wolfvoice:1.0"));
        assert!(yaml.contains("\"9090:8090\""));
        assert!(yaml.contains("TURN_SERVER=turn:example.com:3478"));
        assert!(yaml.contains("SECRET_KEY=my-secret"));
        assert!(yaml.contains("LOG_LEVEL=debug"));
        assert!(yaml.contains("healthcheck:"));
        assert!(yaml.contains("http://localhost:8090/health"));
        assert!(yaml.contains("interval: 30s"));
        assert!(yaml.contains("timeout: 10s"));
        assert!(yaml.contains("retries: 3"));
        assert!(yaml.contains("restart: always"));
    }

    #[test]
    fn generate_compose_file_starts_with_version_and_services() {
        let config = WolfvoiceDockerConfig {
            secret_key: "test-key".to_string(),
            ..Default::default()
        };

        let yaml = generate_compose_file(&config);

        assert!(yaml.starts_with("version: \"3.8\"\n"));
        assert!(yaml.contains("services:\n"));
        assert!(yaml.contains("  wolfvoice:"));
    }

    #[test]
    fn validate_config_catches_empty_image() {
        let config = WolfvoiceDockerConfig {
            image: String::new(),
            secret_key: "ok".to_string(),
            ..Default::default()
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("image"));
    }

    #[test]
    fn validate_config_catches_empty_secret_key() {
        let config = WolfvoiceDockerConfig::default();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("secret_key"));
    }

    #[test]
    fn validate_config_accepts_valid_config() {
        let config = WolfvoiceDockerConfig {
            secret_key: "valid-secret".to_string(),
            ..Default::default()
        };

        assert!(validate_config(&config).is_ok());
    }
}
