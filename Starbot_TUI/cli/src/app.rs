use std::path::PathBuf;

use crate::api::ApiClient;
use crate::config::{CliConfig, active_profile_name, resolve_api_url, resolve_token};
use crate::errors::CliError;
use crate::output::OutputMode;

#[derive(Debug, Clone)]
pub struct Runtime {
    pub output: OutputMode,
    pub config: CliConfig,
    pub config_path: PathBuf,
    pub profile_override: Option<String>,
    pub api_url_override: Option<String>,
    pub timeout_ms: u64,
    pub retries: u32,
}

impl Runtime {
    pub fn active_profile(&self) -> String {
        active_profile_name(&self.config, self.profile_override.as_deref())
    }

    pub fn resolved_api_url(&self) -> Result<String, CliError> {
        resolve_api_url(
            &self.config,
            &self.active_profile(),
            self.api_url_override.as_deref(),
        )
    }

    pub fn resolved_token(&self) -> Option<String> {
        resolve_token(&self.config, &self.active_profile())
    }

    pub fn api_client(&self) -> Result<ApiClient, CliError> {
        ApiClient::new(
            self.resolved_api_url()?,
            self.resolved_token(),
            self.timeout_ms,
            self.retries,
            self.output.debug,
        )
    }
}
