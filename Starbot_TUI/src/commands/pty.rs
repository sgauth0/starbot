//! PTY (Pseudo-Terminal) Management for Interactive Terminal Support
//!
//! This module provides functionality to spawn and manage interactive shell sessions
//! within the CLI agent, allowing for commands that require user interaction.

use std::collections::{HashMap, VecDeque};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::errors::CliError;

/// PTY session state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtyState {
    /// PTY is ready
    Ready,
    /// PTY is running a command
    Running,
    /// PTY is waiting for input
    Waiting,
    /// PTY has exited
    Exited,
    /// PTY has an error
    Error,
}

/// PTY session configuration
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Shell to use (default: /bin/bash)
    pub shell: String,
    /// Terminal type
    pub term_type: String,
    /// Number of columns
    pub columns: u16,
    /// Number of rows
    pub rows: u16,
    /// Timeout for waiting for prompts (in seconds)
    pub prompt_timeout: u64,
    /// Maximum buffer size
    pub max_buffer_size: usize,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            shell: "/bin/bash".to_string(),
            term_type: "xterm-256color".to_string(),
            columns: 80,
            rows: 24,
            prompt_timeout: 30,
            max_buffer_size: 1024 * 1024, // 1MB
        }
    }
}

/// PTY session output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyOutput {
    /// The output content
    pub content: String,
    /// Whether the output is stderr
    pub is_stderr: bool,
    /// Timestamp
    pub timestamp: u64,
    /// Whether the PTY is waiting for input
    pub waiting_for_input: bool,
}

/// PTY session
pub struct PtySession {
    config: PtyConfig,
    state: PtyState,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    buffer: VecDeque<u8>,
    output_lines: VecDeque<String>,
    last_activity: Instant,
}

impl PtySession {
    /// Create a new PTY session
    pub fn new(config: PtyConfig) -> Self {
        Self {
            config,
            state: PtyState::Ready,
            child: None,
            stdin: None,
            stdout: None,
            buffer: VecDeque::new(),
            output_lines: VecDeque::new(),
            last_activity: Instant::now(),
        }
    }

    /// Spawn the PTY session
    pub async fn spawn(&mut self) -> Result<(), CliError> {
        let shell_path = std::path::Path::new(&self.config.shell);
        if !shell_path.exists() {
            return Err(CliError::Generic(format!(
                "Shell not found: {}",
                self.config.shell
            )));
        }

        let mut cmd = Command::new(&self.config.shell);
        cmd.env("TERM", &self.config.term_type)
            .env("COLUMNS", self.config.columns.to_string())
            .env("LINES", self.config.rows.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                self.stdin = match child.stdin.take() {
                    Some(stdin) => Some(stdin),
                    None => return Err(CliError::Generic("Failed to capture stdin".to_string())),
                };
                self.stdout = match child.stdout.take() {
                    Some(stdout) => Some(stdout),
                    None => return Err(CliError::Generic("Failed to capture stdout".to_string())),
                };
                self.child = Some(child);
                self.state = PtyState::Running;
                self.last_activity = Instant::now();
                Ok(())
            }
            Err(e) => Err(CliError::Generic(format!("Failed to spawn PTY: {}", e))),
        }
    }

    /// Send input to the PTY
    pub async fn send(&mut self, input: &str) -> Result<(), CliError> {
        if let Some(ref mut stdin) = self.stdin {
            stdin
                .write_all(input.as_bytes())
                .await
                .map_err(|e| CliError::Generic(format!("Failed to write to PTY: {}", e)))?;
            stdin
                .flush()
                .await
                .map_err(|e| CliError::Generic(format!("Failed to flush PTY: {}", e)))?;
            self.last_activity = Instant::now();
            Ok(())
        } else {
            Err(CliError::Generic("PTY not initialized".to_string()))
        }
    }

    /// Send a line followed by newline to the PTY
    pub async fn send_line(&mut self, line: &str) -> Result<(), CliError> {
        self.send(&format!("{}\n", line)).await
    }

    /// Read output from the PTY
    pub async fn read(&mut self) -> Result<PtyOutput, CliError> {
        if let Some(ref mut stdout) = self.stdout {
            let mut buf = [0u8; 4096];
            let n = stdout
                .read(&mut buf)
                .await
                .map_err(|e| CliError::Generic(format!("Failed to read from PTY: {}", e)))?;

            if n > 0 {
                let content = String::from_utf8_lossy(&buf[..n]).to_string();
                self.buffer.extend(buf[..n].iter().cloned());

                // Extract complete lines
                while let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
                    let line_bytes: Vec<u8> = self.buffer.drain(..=pos).collect();
                    if let Ok(line) = String::from_utf8(line_bytes) {
                        self.output_lines.push_back(line);
                    }
                }

                self.last_activity = Instant::now();

                // Check if we're waiting for input (no activity in timeout)
                let waiting_for_input = self.last_activity.elapsed()
                    > Duration::from_secs(self.config.prompt_timeout);

                Ok(PtyOutput {
                    content,
                    is_stderr: false,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    waiting_for_input,
                })
            } else {
                // Check if the child has exited
                if let Some(ref mut child) = self.child {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            self.state = PtyState::Exited;
                            Ok(PtyOutput {
                                content: format!("\n[Process exited with status: {}]", status),
                                is_stderr: false,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                                waiting_for_input: false,
                            })
                        }
                        Ok(None) => {
                            // Still running, check for timeout
                            let waiting_for_input = self.last_activity.elapsed()
                                > Duration::from_secs(self.config.prompt_timeout);
                            if waiting_for_input {
                                self.state = PtyState::Waiting;
                            }
                            Ok(PtyOutput {
                                content: String::new(),
                                is_stderr: false,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                                waiting_for_input,
                            })
                        }
                        Err(e) => Err(CliError::Generic(format!("Failed to check child status: {}", e))),
                    }
                } else {
                    Ok(PtyOutput {
                        content: String::new(),
                        is_stderr: false,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        waiting_for_input: false,
                    })
                }
            }
        } else {
            Err(CliError::Generic("PTY not initialized".to_string()))
        }
    }

    /// Read with a timeout
    pub async fn read_timeout(&mut self, timeout_ms: u64) -> Result<PtyOutput, CliError> {
        match tokio::time::timeout(Duration::from_millis(timeout_ms), self.read()).await {
            Ok(result) => result,
            Err(_) => {
                // Timeout - check if we're waiting for input
                let waiting_for_input = self.last_activity.elapsed()
                    > Duration::from_secs(self.config.prompt_timeout);
                if waiting_for_input {
                    self.state = PtyState::Waiting;
                }
                Ok(PtyOutput {
                    content: String::new(),
                    is_stderr: false,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    waiting_for_input,
                })
            }
        }
    }

    /// Send a command and wait for completion
    pub async fn execute(&mut self, command: &str) -> Result<String, CliError> {
        self.state = PtyState::Running;
        self.send_line(command).await?;

        let mut output = String::new();
        let mut waiting_count = 0;

        loop {
            let pty_output = self.read_timeout(100).await?;

            if !pty_output.content.is_empty() {
                output.push_str(&pty_output.content);
                waiting_count = 0;
            }

            if pty_output.waiting_for_input {
                waiting_count += 1;
                if waiting_count > 5 {
                    // Consistently waiting for input, assume command is done
                    break;
                }
            }

            if self.state == PtyState::Exited {
                break;
            }
        }

        Ok(output)
    }

    /// Get the current state
    pub fn state(&self) -> &PtyState {
        &self.state
    }

    /// Check if the PTY is ready for input
    pub fn is_ready(&self) -> bool {
        matches!(self.state, PtyState::Running | PtyState::Waiting)
    }

    /// Get all buffered output lines
    pub fn get_output_lines(&self) -> Vec<String> {
        self.output_lines.iter().cloned().collect()
    }

    /// Clear the output buffer
    pub fn clear_output(&mut self) {
        self.output_lines.clear();
        self.buffer.clear();
    }

    /// Resize the terminal
    pub fn resize(&mut self, columns: u16, rows: u16) {
        self.config.columns = columns;
        self.config.rows = rows;
        // In a real PTY implementation, we would use ioctl to resize
    }

    /// Kill the PTY session
    pub async fn kill(&mut self) -> Result<(), CliError> {
        if let Some(mut child) = self.child.take() {
            child
                .kill()
                .await
                .map_err(|e| CliError::Generic(format!("Failed to kill PTY: {}", e)))?;
            self.state = PtyState::Exited;
        }
        Ok(())
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
    }
}

/// PTY manager for managing multiple sessions by ID
pub struct PtyManager {
    sessions: Arc<Mutex<HashMap<String, PtySession>>>,
    default_config: PtyConfig,
}

impl PtyManager {
    /// Create a new PTY manager
    pub fn new(default_config: PtyConfig) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            default_config,
        }
    }

    /// Create a new PTY session, returns its ID
    pub async fn create_session(&self) -> Result<String, CliError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let mut session = PtySession::new(self.default_config.clone());
        session.spawn().await?;

        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), session);

        Ok(session_id)
    }

    /// Send input to a session
    pub async fn send(&self, session_id: &str, input: &str) -> Result<(), CliError> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            session.send(input).await?;
            Ok(())
        } else {
            Err(CliError::Generic(format!("PTY session not found: {}", session_id)))
        }
    }

    /// Read from a session
    pub async fn read(&self, session_id: &str) -> Result<PtyOutput, CliError> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            session.read().await
        } else {
            Err(CliError::Generic(format!("PTY session not found: {}", session_id)))
        }
    }

    /// Execute a command in a session
    pub async fn execute(&self, session_id: &str, command: &str) -> Result<String, CliError> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            session.execute(command).await
        } else {
            Err(CliError::Generic(format!("PTY session not found: {}", session_id)))
        }
    }

    /// Kill a session and remove it
    pub async fn kill_session(&self, session_id: &str) -> Result<(), CliError> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(mut session) = sessions.remove(session_id) {
            session.kill().await?;
        }
        Ok(())
    }

    /// List active session IDs
    pub fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions.keys().cloned().collect()
    }
}

/// Interactive shell helper
pub struct InteractiveShell {
    pty: PtySession,
    history: Vec<String>,
    history_index: usize,
}

impl InteractiveShell {
    /// Create a new interactive shell
    pub fn new(config: PtyConfig) -> Self {
        Self {
            pty: PtySession::new(config),
            history: Vec::new(),
            history_index: 0,
        }
    }

    /// Initialize the shell
    pub async fn initialize(&mut self) -> Result<(), CliError> {
        self.pty.spawn().await?;

        // Set up the shell environment
        self.pty.send_line("export HISTCONTROL=ignoreboth").await?;
        self.pty.send_line("export PS1='\\$ '").await?;

        Ok(())
    }

    /// Execute a command and capture output
    pub async fn execute(&mut self, command: &str) -> Result<String, CliError> {
        // Add to history
        self.history.push(command.to_string());
        self.history_index = self.history.len();

        // Execute the command
        let output = self.pty.execute(command).await?;

        Ok(output)
    }

    /// Get command history
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Get next history entry
    pub fn history_next(&mut self) -> Option<&str> {
        if self.history_index < self.history.len() {
            self.history_index += 1;
            self.history.get(self.history_index - 1).map(|s| s.as_str())
        } else {
            None
        }
    }

    /// Get previous history entry
    pub fn history_prev(&mut self) -> Option<&str> {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.history.get(self.history_index).map(|s| s.as_str())
        } else {
            None
        }
    }

    /// Check if the shell is ready
    pub fn is_ready(&self) -> bool {
        self.pty.is_ready()
    }

    /// Get the PTY session state
    pub fn state(&self) -> &PtyState {
        self.pty.state()
    }

    /// Get the PTY session
    pub fn pty_mut(&mut self) -> &mut PtySession {
        &mut self.pty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pty_config_default() {
        let config = PtyConfig::default();
        assert_eq!(config.shell, "/bin/bash");
        assert_eq!(config.term_type, "xterm-256color");
        assert_eq!(config.columns, 80);
        assert_eq!(config.rows, 24);
    }

    #[tokio::test]
    async fn test_pty_session_creation() {
        let session = PtySession::new(PtyConfig::default());
        assert_eq!(session.state(), &PtyState::Ready);
    }
}
