use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::errors::CliError;

pub const DEFAULT_API_URL: &str = "http://localhost:3737";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub api_url: String,
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            api_url: DEFAULT_API_URL.to_string(),
            token: None,
            refresh_token: None,
            workspace_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub profile: String,
    pub profiles: HashMap<String, ProfileConfig>,
}

impl Default for CliConfig {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert("default".to_string(), ProfileConfig::default());
        Self {
            profile: "default".to_string(),
            profiles,
        }
    }
}

pub fn config_path() -> Result<PathBuf, CliError> {
    let base = dirs::config_dir().ok_or_else(|| {
        CliError::Generic("Could not resolve config directory for this OS.".to_string())
    })?;
    Ok(base.join("starbott").join("config.json"))
}

pub fn load_config() -> Result<CliConfig, CliError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(CliConfig::default());
    }

    let text = fs::read_to_string(&path)?;
    let mut config: CliConfig = serde_json::from_str(&text)?;
    let profile = config.profile.clone();
    ensure_profile(&mut config, &profile);
    Ok(config)
}

pub fn save_config(config: &CliConfig) -> Result<PathBuf, CliError> {
    let path = config_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| CliError::Generic("Invalid config path.".to_string()))?;
    fs::create_dir_all(parent)?;
    fs::write(&path, serde_json::to_string_pretty(config)?)?;
    Ok(path)
}

pub fn active_profile_name(config: &CliConfig, profile_override: Option<&str>) -> String {
    profile_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.profile.clone())
}

pub fn ensure_profile(config: &mut CliConfig, profile_name: &str) {
    if !config.profiles.contains_key(profile_name) {
        config
            .profiles
            .insert(profile_name.to_string(), ProfileConfig::default());
    }
}

pub fn profile_ref<'a>(config: &'a CliConfig, profile_name: &str) -> Option<&'a ProfileConfig> {
    config.profiles.get(profile_name)
}

pub fn profile_mut<'a>(
    config: &'a mut CliConfig,
    profile_name: &str,
) -> Option<&'a mut ProfileConfig> {
    config.profiles.get_mut(profile_name)
}

pub fn resolve_api_url(
    config: &CliConfig,
    profile_name: &str,
    api_override: Option<&str>,
) -> Result<String, CliError> {
    if let Some(url) = api_override {
        validate_url(url)?;
        return Ok(url.to_string());
    }

    let profile = profile_ref(config, profile_name)
        .ok_or_else(|| CliError::Usage(format!("Profile '{profile_name}' does not exist.")))?;
    validate_url(&profile.api_url)?;
    Ok(profile.api_url.clone())
}

pub fn resolve_token(config: &CliConfig, profile_name: &str) -> Option<String> {
    if let Ok(token) = std::env::var("STARBOTT_TOKEN") {
        if !token.trim().is_empty() {
            return Some(token.trim().to_string());
        }
    }

    profile_ref(config, profile_name).and_then(|p| p.token.clone())
}

pub fn validate_url(value: &str) -> Result<(), CliError> {
    let parsed = Url::parse(value)?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(CliError::Usage(
            "API URL must use http:// or https://.".to_string(),
        ));
    }
    Ok(())
}
