use clap::{Subcommand, ValueEnum};
use serde_json::json;

use crate::app::Runtime;
use crate::config::{ensure_profile, profile_mut, profile_ref, save_config, validate_url};
use crate::errors::{CliError, redact_secret};

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Initialize config file and profile
    Init {
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        token: Option<String>,
    },
    /// Read a config key from the active profile
    Get {
        key: ConfigKey,
        #[arg(long)]
        show_token: bool,
    },
    /// Set a config key on the active profile
    Set { key: ConfigKey, value: String },
    /// List all profiles
    Profiles,
    /// Switch active profile
    Use { profile: String },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ConfigKey {
    #[value(name = "apiUrl")]
    ApiUrl,
    #[value(name = "token")]
    Token,
}

pub async fn handle(runtime: &mut Runtime, command: ConfigCommand) -> Result<(), CliError> {
    match command {
        ConfigCommand::Init { api_url, token } => init(runtime, api_url, token).await,
        ConfigCommand::Get { key, show_token } => get(runtime, key, show_token).await,
        ConfigCommand::Set { key, value } => set(runtime, key, value).await,
        ConfigCommand::Profiles => profiles(runtime).await,
        ConfigCommand::Use { profile } => use_profile(runtime, profile).await,
    }
}

async fn init(
    runtime: &mut Runtime,
    api_url: Option<String>,
    token: Option<String>,
) -> Result<(), CliError> {
    let profile_name = runtime.active_profile();
    ensure_profile(&mut runtime.config, &profile_name);
    if let Some(profile) = profile_mut(&mut runtime.config, &profile_name) {
        if let Some(url) = api_url {
            validate_url(&url)?;
            profile.api_url = url;
        }

        if let Some(value) = token {
            profile.token = Some(value);
        } else if !is_ci() && !runtime.output.json && !runtime.output.quiet {
            let maybe_token = rpassword::prompt_password("Token (optional, Enter to skip): ")
                .map_err(|e| CliError::Generic(format!("Failed reading token: {e}")))?;
            if !maybe_token.trim().is_empty() {
                profile.token = Some(maybe_token.trim().to_string());
            }
        }
    }

    runtime.config.profile = profile_name;
    let path = save_config(&runtime.config)?;
    runtime.config_path = path.clone();

    if runtime.output.json {
        runtime
            .output
            .print_json(&json!({ "ok": true, "path": path }))?;
    } else {
        runtime
            .output
            .print_human(&format!("Config initialized: {}", path.display()));
    }
    Ok(())
}

async fn get(runtime: &mut Runtime, key: ConfigKey, show_token: bool) -> Result<(), CliError> {
    let profile_name = runtime.active_profile();
    let profile = profile_ref(&runtime.config, &profile_name).ok_or_else(|| {
        CliError::Usage(format!(
            "Profile '{profile_name}' not found. Run `starbott config init` first."
        ))
    })?;

    match key {
        ConfigKey::ApiUrl => {
            if runtime.output.json {
                runtime.output.print_json(&json!({
                    "key": "apiUrl",
                    "value": profile.api_url
                }))?;
            } else {
                runtime.output.print_human(&profile.api_url);
            }
        }
        ConfigKey::Token => {
            let resolved = runtime.resolved_token();
            let display = resolved.as_deref().map(|t| {
                if show_token {
                    t.to_string()
                } else {
                    redact_secret(t)
                }
            });

            if runtime.output.json {
                runtime.output.print_json(&json!({
                    "key": "token",
                    "value": display
                }))?;
            } else if let Some(v) = display {
                runtime.output.print_human(&v);
            } else {
                runtime.output.print_human("(not set)");
            }
        }
    }

    Ok(())
}

async fn set(runtime: &mut Runtime, key: ConfigKey, value: String) -> Result<(), CliError> {
    let profile_name = runtime.active_profile();
    ensure_profile(&mut runtime.config, &profile_name);
    let profile = profile_mut(&mut runtime.config, &profile_name).ok_or_else(|| {
        CliError::Generic(format!(
            "Failed to resolve profile '{profile_name}' while setting config."
        ))
    })?;

    match key {
        ConfigKey::ApiUrl => {
            validate_url(&value)?;
            profile.api_url = value;
        }
        ConfigKey::Token => {
            profile.token = Some(value);
        }
    }

    let path = save_config(&runtime.config)?;
    runtime.config_path = path;

    if runtime.output.json {
        runtime.output.print_json(&json!({ "ok": true }))?;
    } else {
        runtime.output.print_human("Config updated.");
    }

    Ok(())
}

async fn profiles(runtime: &mut Runtime) -> Result<(), CliError> {
    let active = runtime.active_profile();
    let mut names: Vec<String> = runtime.config.profiles.keys().cloned().collect();
    names.sort();

    if runtime.output.json {
        let payload = names
            .iter()
            .map(|name| {
                let profile = runtime.config.profiles.get(name);
                json!({
                    "name": name,
                    "active": name == &active,
                    "apiUrl": profile.map(|p| p.api_url.clone()).unwrap_or_default(),
                    "hasToken": profile
                        .and_then(|p| p.token.as_ref())
                        .map(|t| !t.is_empty())
                        .unwrap_or(false)
                })
            })
            .collect::<Vec<_>>();
        runtime.output.print_json(&json!({ "profiles": payload }))?;
        return Ok(());
    }

    for name in names {
        let marker = if name == active { "*" } else { " " };
        runtime.output.print_human(&format!("{marker} {name}"));
    }
    Ok(())
}

async fn use_profile(runtime: &mut Runtime, profile_name: String) -> Result<(), CliError> {
    ensure_profile(&mut runtime.config, &profile_name);
    runtime.config.profile = profile_name.clone();
    let path = save_config(&runtime.config)?;
    runtime.config_path = path;

    if runtime.output.json {
        runtime
            .output
            .print_json(&json!({ "ok": true, "profile": profile_name }))?;
    } else {
        runtime
            .output
            .print_human(&format!("Active profile: {profile_name}"));
    }

    Ok(())
}

fn is_ci() -> bool {
    std::env::var("CI")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
