use std::fmt;

use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub enum ExitCode {
    Generic = 1,
    Auth = 2,
    Usage = 3,
    Network = 4,
    RateLimited = 5,
    Server = 6,
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Auth(String),
    #[error("{0}")]
    Network(String),
    #[error("{0}")]
    RateLimited(String),
    #[error("{0}")]
    Server(String),
    #[error("{0}")]
    Generic(String),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Usage(_) => ExitCode::Usage as i32,
            CliError::Auth(_) => ExitCode::Auth as i32,
            CliError::Network(_) => ExitCode::Network as i32,
            CliError::RateLimited(_) => ExitCode::RateLimited as i32,
            CliError::Server(_) => ExitCode::Server as i32,
            CliError::Generic(_) => ExitCode::Generic as i32,
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(value: std::io::Error) -> Self {
        CliError::Generic(format!("I/O error: {value}"))
    }
}

impl From<serde_json::Error> for CliError {
    fn from(value: serde_json::Error) -> Self {
        CliError::Generic(format!("JSON error: {value}"))
    }
}

impl From<url::ParseError> for CliError {
    fn from(value: url::ParseError) -> Self {
        CliError::Usage(format!("Invalid URL: {value}"))
    }
}

impl From<reqwest::Error> for CliError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_timeout() {
            return CliError::Network("Request timed out.".to_string());
        }
        CliError::Network(format!("Network request failed: {value}"))
    }
}

pub fn with_debug_hint(message: &str, debug: bool) -> String {
    if debug {
        return message.to_string();
    }
    format!("{message} (try --debug for details)")
}

pub fn redact_secret(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let bytes = input.as_bytes();
    for (idx, b) in bytes.iter().enumerate() {
        if idx < 3 || idx + 3 >= bytes.len() {
            out.push(*b as char);
        } else {
            out.push('*');
        }
    }
    out
}

impl fmt::Display for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self as i32)
    }
}
