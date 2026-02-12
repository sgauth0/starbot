use serde::Serialize;

use crate::errors::CliError;

#[derive(Debug, Clone)]
pub struct OutputMode {
    pub json: bool,
    pub quiet: bool,
    pub verbose: bool,
    pub debug: bool,
}

impl OutputMode {
    pub fn print_json<T: Serialize>(&self, value: &T) -> Result<(), CliError> {
        let text = serde_json::to_string(value)?;
        println!("{text}");
        Ok(())
    }

    pub fn print_human(&self, message: &str) {
        if self.json || self.quiet {
            return;
        }
        println!("{message}");
    }

    pub fn print_stderr(&self, message: &str) {
        if self.json || self.quiet {
            return;
        }
        eprintln!("{message}");
    }

    pub fn print_verbose(&self, message: &str) {
        if !self.verbose || self.json || self.quiet {
            return;
        }
        eprintln!("{message}");
    }
}

pub fn print_error(error: &CliError, mode: &OutputMode) {
    if mode.json {
        let payload = serde_json::json!({
            "error": error.to_string(),
            "code": error.exit_code()
        });
        println!(
            "{}",
            serde_json::to_string(&payload)
                .unwrap_or_else(|_| "{\"error\":\"unknown\"}".to_string())
        );
        return;
    }

    eprintln!("Error: {error}");
}
