//! Enhanced Tool System with CLI Agent Patterns
//!
//! This module provides a unified tool execution system that can work
//! with both local tools (direct execution) and remote tools (API proposals).
//! It includes validation, security, and retry mechanisms.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

use crate::api::ApiClient;
use crate::errors::CliError;
use crate::commands::pty::{PtyConfig, PtyManager, PtySession};

/// Tool execution mode
#[derive(Debug, Clone, PartialEq)]
pub enum ToolMode {
    /// Direct execution (local tools)
    Direct,
    /// API proposal mode (remote tools)
    Proposal,
    /// Hybrid mode (try direct first, then proposal)
    Hybrid,
}

/// Tool configuration
#[derive(Debug, Clone)]
pub struct ToolConfig {
    /// Maximum execution time in seconds
    pub timeout_seconds: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Enable tool validation
    pub enable_validation: bool,
    /// Execution mode
    pub mode: ToolMode,
    /// Workspace ID for tool execution
    pub workspace_id: Option<String>,
    /// Enable PTY for interactive commands
    pub enable_pty: bool,
    /// PTY configuration
    pub pty_config: Option<PtyConfig>,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            max_retries: 3,
            enable_validation: true,
            mode: ToolMode::Hybrid,
            workspace_id: None,
            enable_pty: false,
            pty_config: None,
        }
    }
}

/// Tool result with enhanced metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool executed successfully
    pub success: bool,
    /// The tool output
    pub output: String,
    /// Error message if failed
    pub error: Option<String>,
    /// Metadata about the execution
    pub metadata: Option<ToolMetadata>,
}

/// Tool execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    /// Tool execution ID (for remote tools)
    pub execution_id: Option<String>,
    /// Tool call ID
    pub call_id: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Number of retries
    pub retry_count: u32,
    /// Remote tool URL (if applicable)
    pub remote_url: Option<String>,
    /// Workspace ID
    pub workspace_id: Option<String>,
}

impl ToolResult {
    pub fn success(output: String) -> Self {
        Self {
            success: true,
            output,
            error: None,
            metadata: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(error),
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: ToolMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Tool definition schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
    pub category: String,
    pub safe: bool,
    pub file_operations: bool,
    pub network_operations: bool,
}

/// Tool parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub r#type: String,
    pub description: String,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub enum_values: Option<Vec<String>>,
    pub validation_regex: Option<String>,
}

/// Tool validator for parameter validation
pub struct ToolValidator;

impl ToolValidator {
    /// Validate tool arguments against the tool definition
    pub fn validate_arguments(
        tool_def: &ToolDefinition,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<(), CliError> {
        // Check required parameters
        for param in &tool_def.parameters {
            if param.required && !args.contains_key(&param.name) {
                return Err(CliError::Usage(format!(
                    "Required parameter '{}' missing for tool '{}'",
                    param.name, tool_def.name
                )));
            }
        }

        // Validate parameter values
        for (param_name, value) in args {
            if let Some(param) = tool_def.parameters.iter().find(|p| p.name == *param_name) {
                Self::validate_parameter_value(param, value)?;
            }
        }

        Ok(())
    }

    fn validate_parameter_value(param: &ToolParameter, value: &serde_json::Value) -> Result<(), CliError> {
        match value {
            serde_json::Value::String(s) => {
                // Check enum values
                if let Some(enum_values) = &param.enum_values {
                    if !enum_values.contains(&s) {
                        return Err(CliError::Usage(format!(
                            "Invalid value '{}' for parameter '{}'. Must be one of: {:?}",
                            s, param.name, enum_values
                        )));
                    }
                }

                // Check regex validation
                if let Some(regex_str) = &param.validation_regex {
                    let regex = regex::Regex::new(regex_str)
                        .map_err(|e| CliError::Usage(format!("Invalid regex pattern: {}", e)))?;
                    if !regex.is_match(s) {
                        return Err(CliError::Usage(format!(
                            "Value '{}' for parameter '{}' doesn't match required pattern",
                            s, param.name
                        )));
                    }
                }
            }
            serde_json::Value::Number(n) => {
                // TODO: Add numeric range validation
            }
            serde_json::Value::Bool(_) => {
                // TODO: Add boolean validation if needed
            }
            _ => {
                return Err(CliError::Usage(format!(
                    "Invalid type for parameter '{}'",
                    param.name
                )));
            }
        }

        Ok(())
    }
}

/// Enhanced tool executor that supports both local and remote tools
pub struct EnhancedToolExecutor {
    api_client: ApiClient,
    config: ToolConfig,
    tool_definitions: HashMap<String, ToolDefinition>,
    pty_manager: Arc<Mutex<PtyManager>>,
}

impl EnhancedToolExecutor {
    pub fn new(api_client: ApiClient, config: ToolConfig) -> Self {
        let pty_config = config.pty_config.clone().unwrap_or_default();
        let pty_manager = PtyManager::new(pty_config);

        let mut executor = Self {
            api_client,
            config,
            tool_definitions: HashMap::new(),
            pty_manager: Arc::new(Mutex::new(pty_manager)),
        };
        executor.register_default_tools();
        executor
    }

    /// Register a tool definition
    pub fn register_tool(&mut self, tool: ToolDefinition) {
        self.tool_definitions.insert(tool.name.clone(), tool);
    }

    /// Register all built-in tool definitions
    fn register_default_tools(&mut self) {
        let tools = vec![
            ToolDefinition {
                name: "read_file".to_string(),
                description: "Read the contents of a file".to_string(),
                parameters: vec![ToolParameter {
                    name: "path".to_string(),
                    r#type: "string".to_string(),
                    description: "File path to read".to_string(),
                    required: true,
                    default_value: None,
                    enum_values: None,
                    validation_regex: None,
                }],
                category: "filesystem".to_string(),
                safe: true,
                file_operations: true,
                network_operations: false,
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "Write content to a file".to_string(),
                parameters: vec![
                    ToolParameter {
                        name: "path".to_string(),
                        r#type: "string".to_string(),
                        description: "File path to write".to_string(),
                        required: true,
                        default_value: None,
                        enum_values: None,
                        validation_regex: None,
                    },
                    ToolParameter {
                        name: "content".to_string(),
                        r#type: "string".to_string(),
                        description: "Content to write".to_string(),
                        required: true,
                        default_value: None,
                        enum_values: None,
                        validation_regex: None,
                    },
                ],
                category: "filesystem".to_string(),
                safe: true,
                file_operations: true,
                network_operations: false,
            },
            ToolDefinition {
                name: "search_files".to_string(),
                description: "Search for files matching a pattern".to_string(),
                parameters: vec![
                    ToolParameter {
                        name: "pattern".to_string(),
                        r#type: "string".to_string(),
                        description: "Search pattern".to_string(),
                        required: false,
                        default_value: Some(serde_json::Value::String("*".to_string())),
                        enum_values: None,
                        validation_regex: None,
                    },
                    ToolParameter {
                        name: "path".to_string(),
                        r#type: "string".to_string(),
                        description: "Directory to search".to_string(),
                        required: false,
                        default_value: Some(serde_json::Value::String(".".to_string())),
                        enum_values: None,
                        validation_regex: None,
                    },
                ],
                category: "filesystem".to_string(),
                safe: true,
                file_operations: true,
                network_operations: false,
            },
            ToolDefinition {
                name: "execute_command".to_string(),
                description: "Execute a shell command".to_string(),
                parameters: vec![ToolParameter {
                    name: "command".to_string(),
                    r#type: "string".to_string(),
                    description: "Shell command to execute".to_string(),
                    required: true,
                    default_value: None,
                    enum_values: None,
                    validation_regex: None,
                }],
                category: "system".to_string(),
                safe: true,
                file_operations: false,
                network_operations: false,
            },
            ToolDefinition {
                name: "interactive_shell".to_string(),
                description: "Execute command in interactive shell with PTY".to_string(),
                parameters: vec![
                    ToolParameter {
                        name: "command".to_string(),
                        r#type: "string".to_string(),
                        description: "Command to execute".to_string(),
                        required: true,
                        default_value: None,
                        enum_values: None,
                        validation_regex: None,
                    },
                    ToolParameter {
                        name: "input".to_string(),
                        r#type: "string".to_string(),
                        description: "Optional input to send".to_string(),
                        required: false,
                        default_value: None,
                        enum_values: None,
                        validation_regex: None,
                    },
                    ToolParameter {
                        name: "timeout".to_string(),
                        r#type: "number".to_string(),
                        description: "Timeout in seconds".to_string(),
                        required: false,
                        default_value: Some(serde_json::Value::Number(30.into())),
                        enum_values: None,
                        validation_regex: None,
                    },
                ],
                category: "system".to_string(),
                safe: true,
                file_operations: false,
                network_operations: false,
            },
            ToolDefinition {
                name: "create_task".to_string(),
                description: "Create a new task".to_string(),
                parameters: vec![
                    ToolParameter {
                        name: "title".to_string(),
                        r#type: "string".to_string(),
                        description: "Task title".to_string(),
                        required: true,
                        default_value: None,
                        enum_values: None,
                        validation_regex: None,
                    },
                    ToolParameter {
                        name: "description".to_string(),
                        r#type: "string".to_string(),
                        description: "Task description".to_string(),
                        required: false,
                        default_value: None,
                        enum_values: None,
                        validation_regex: None,
                    },
                    ToolParameter {
                        name: "priority".to_string(),
                        r#type: "number".to_string(),
                        description: "Priority 0-10".to_string(),
                        required: false,
                        default_value: Some(serde_json::Value::Number(0.into())),
                        enum_values: None,
                        validation_regex: None,
                    },
                ],
                category: "tasks".to_string(),
                safe: true,
                file_operations: false,
                network_operations: true,
            },
            ToolDefinition {
                name: "update_task".to_string(),
                description: "Update an existing task".to_string(),
                parameters: vec![
                    ToolParameter {
                        name: "task_id".to_string(),
                        r#type: "string".to_string(),
                        description: "Task ID".to_string(),
                        required: true,
                        default_value: None,
                        enum_values: None,
                        validation_regex: None,
                    },
                    ToolParameter {
                        name: "title".to_string(),
                        r#type: "string".to_string(),
                        description: "New title".to_string(),
                        required: false,
                        default_value: None,
                        enum_values: None,
                        validation_regex: None,
                    },
                    ToolParameter {
                        name: "status".to_string(),
                        r#type: "string".to_string(),
                        description: "New status".to_string(),
                        required: false,
                        default_value: None,
                        enum_values: Some(vec![
                            "PENDING".to_string(),
                            "IN_PROGRESS".to_string(),
                            "COMPLETED".to_string(),
                            "CANCELLED".to_string(),
                        ]),
                        validation_regex: None,
                    },
                ],
                category: "tasks".to_string(),
                safe: true,
                file_operations: false,
                network_operations: true,
            },
            ToolDefinition {
                name: "list_tasks".to_string(),
                description: "List tasks with optional filters".to_string(),
                parameters: vec![
                    ToolParameter {
                        name: "status".to_string(),
                        r#type: "string".to_string(),
                        description: "Filter by status".to_string(),
                        required: false,
                        default_value: None,
                        enum_values: Some(vec![
                            "PENDING".to_string(),
                            "IN_PROGRESS".to_string(),
                            "COMPLETED".to_string(),
                            "CANCELLED".to_string(),
                        ]),
                        validation_regex: None,
                    },
                    ToolParameter {
                        name: "limit".to_string(),
                        r#type: "number".to_string(),
                        description: "Max results".to_string(),
                        required: false,
                        default_value: Some(serde_json::Value::Number(10.into())),
                        enum_values: None,
                        validation_regex: None,
                    },
                ],
                category: "tasks".to_string(),
                safe: true,
                file_operations: false,
                network_operations: true,
            },
            ToolDefinition {
                name: "complete_task".to_string(),
                description: "Mark a task as completed".to_string(),
                parameters: vec![ToolParameter {
                    name: "task_id".to_string(),
                    r#type: "string".to_string(),
                    description: "Task ID".to_string(),
                    required: true,
                    default_value: None,
                    enum_values: None,
                    validation_regex: None,
                }],
                category: "tasks".to_string(),
                safe: true,
                file_operations: false,
                network_operations: true,
            },
        ];

        for tool in tools {
            self.register_tool(tool);
        }
    }

    /// Execute a tool with enhanced error handling and retries
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<ToolResult, CliError> {
        // If tool definition exists, validate; otherwise try direct execution
        if let Some(tool_def) = self.tool_definitions.get(tool_name) {
            if self.config.enable_validation {
                ToolValidator::validate_arguments(tool_def, args)?;
            }

            match self.config.mode {
                ToolMode::Direct => return self.execute_direct(tool_def, args).await,
                ToolMode::Proposal => return self.execute_proposal(tool_def, args).await,
                ToolMode::Hybrid => {
                    match self.execute_direct(tool_def, args).await {
                        Ok(result) => return Ok(result),
                        Err(e) => {
                            eprintln!("Direct execution failed: {}", e);
                            return self.execute_proposal(tool_def, args).await;
                        }
                    }
                }
            }
        }

        // Fallback: try direct execution even without definition
        let local_executor = LocalToolExecutor::new(self.api_client.clone());
        local_executor.execute(tool_name.to_string(), args.clone()).await
    }

    /// Execute tool directly (local execution)
    async fn execute_direct(
        &self,
        tool_def: &ToolDefinition,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<ToolResult, CliError> {
        let start_time = std::time::Instant::now();

        // Create local tool executor
        let local_executor = LocalToolExecutor::new(self.api_client.clone());

        // Execute with timeout
        let result = timeout(
            Duration::from_secs(self.config.timeout_seconds),
            local_executor.execute(tool_def.name.clone(), args.clone()),
        ).await
        .map_err(|_| {
            CliError::Generic(format!("Tool '{}' timed out after {} seconds", tool_def.name, self.config.timeout_seconds))
        })??;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        Ok(result.with_metadata(ToolMetadata {
            execution_id: None,
            call_id: None,
            duration_ms,
            retry_count: 0,
            remote_url: None,
            workspace_id: self.config.workspace_id.clone(),
        }))
    }

    /// Execute tool via API proposal
    async fn execute_proposal(
        &self,
        tool_def: &ToolDefinition,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<ToolResult, CliError> {
        let start_time = std::time::Instant::now();

        // Convert args to JSON string
        let _args_json = serde_json::to_string(args)
            .map_err(|e| CliError::Generic(format!("Failed to serialize arguments: {}", e)))?;

        // Build proposal request
        let proposal_request = serde_json::json!({
            "workspaceId": self.config.workspace_id.as_deref().unwrap_or("default"),
            "toolName": tool_def.name,
            "input": serde_json::Value::Object(serde_json::Map::from_iter(args.iter().map(|(k, v)| (k.to_string(), v.clone())))),
        });

        // Send proposal
        let res = self.api_client
            .post_json("/v1/tools/propose", Some(proposal_request), true)
            .await?;

        // Check if immediate execution
        let requires_confirmation = res.json.get("requiresConfirmation")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !requires_confirmation {
            // Execute immediately
            let run_id = res.json.get("runId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            // Get result
            let result_res = self.api_client
                .get_json(&format!("/v1/tools/runs/{}/result", run_id), None, true)
                .await?;

            let duration_ms = start_time.elapsed().as_millis() as u64;
            Ok(ToolResult::success(
                result_res.json.get("result")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Execution completed")
                    .to_string()
            ).with_metadata(ToolMetadata {
                execution_id: Some(run_id.to_string()),
                call_id: None,
                duration_ms,
                retry_count: 0,
                remote_url: Some("/v1/tools/propose".to_string()),
                workspace_id: self.config.workspace_id.clone(),
            }))
        } else {
            // Require manual approval
            Err(CliError::Usage(format!(
                "Tool '{}' requires manual approval. Please use the tools command to approve or deny.",
                tool_def.name
            )))
        }
    }
}

/// Local tool executor for direct tool execution
struct LocalToolExecutor {
    api_client: ApiClient,
}

impl LocalToolExecutor {
    fn new(api_client: ApiClient) -> Self {
        Self { api_client }
    }

    async fn execute(&self, tool_name: String, args: HashMap<String, serde_json::Value>) -> Result<ToolResult, CliError> {
        match tool_name.as_str() {
            "read_file" => self.execute_read_file(args).await,
            "write_file" => self.execute_write_file(args).await,
            "search_files" => self.execute_search_files(args).await,
            "execute_command" => self.execute_command(args).await,
            "interactive_shell" => self.execute_interactive_shell(args).await,
            "create_task" => self.execute_create_task(args).await,
            "update_task" => self.execute_update_task(args).await,
            "list_tasks" => self.execute_list_tasks(args).await,
            "complete_task" => self.execute_complete_task(args).await,
            _ => Ok(ToolResult::error(format!("Unknown local tool: {}", tool_name))),
        }
    }

    // Local tool implementations (similar to existing agent tools)
    async fn execute_read_file(&self, args: HashMap<String, serde_json::Value>) -> Result<ToolResult, CliError> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CliError::Generic("Path is required".to_string()))?;

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| CliError::Generic(format!("Failed to read file: {}", e)))?;

        Ok(ToolResult::success(content))
    }

    async fn execute_write_file(&self, args: HashMap<String, serde_json::Value>) -> Result<ToolResult, CliError> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CliError::Generic("Path is required".to_string()))?;
        let content = args.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CliError::Generic("Content is required".to_string()))?;

        tokio::fs::write(path, content)
            .await
            .map_err(|e| CliError::Generic(format!("Failed to write file: {}", e)))?;

        Ok(ToolResult::success(format!("File written to {}", path)))
    }

    async fn execute_search_files(&self, args: HashMap<String, serde_json::Value>) -> Result<ToolResult, CliError> {
        let pattern = args.get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("*");
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let mut results = Vec::new();
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(e) => return Ok(ToolResult::error(format!("Failed to read directory: {}", e))),
        };

        for entry in entries {
            if let Ok(entry) = entry {
                if let Ok(file_name) = entry.file_name().into_string() {
                    if file_name.contains(pattern) {
                        results.push(file_name);
                    }
                }
            }
        }

        Ok(ToolResult::success(format!("Found files: {:?}", results)))
    }

    async fn execute_command(&self, args: HashMap<String, serde_json::Value>) -> Result<ToolResult, CliError> {
        let command = args.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CliError::Generic("Command is required".to_string()))?;

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| CliError::Generic(format!("Failed to execute command: {}", e)))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        if output.status.success() {
            Ok(ToolResult::success(output_str.to_string()))
        } else {
            Ok(ToolResult::error(format!("Command failed: {}", output_str)))
        }
    }

    /// Execute a command in an interactive shell
    async fn execute_interactive_shell(&self, args: HashMap<String, serde_json::Value>) -> Result<ToolResult, CliError> {
        let command = args.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CliError::Generic("Command is required".to_string()))?;

        let input = args.get("input")
            .and_then(|v| v.as_str());

        let timeout_secs = args.get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        // Create a PTY session
        let pty_config = PtyConfig {
            shell: "/bin/bash".to_string(),
            term_type: "xterm-256color".to_string(),
            columns: 80,
            rows: 24,
            prompt_timeout: timeout_secs,
            max_buffer_size: 1024 * 1024,
        };

        let mut session = PtySession::new(pty_config);

        // Spawn the PTY
        match session.spawn().await {
            Ok(_) => {},
            Err(e) => return Ok(ToolResult::error(format!("Failed to spawn PTY: {}", e))),
        }

        // Send the command
        match session.send_line(command).await {
            Ok(_) => {},
            Err(e) => return Ok(ToolResult::error(format!("Failed to send command: {}", e))),
        }

        // If input is provided, send it
        if let Some(input) = input {
            match session.send_line(input).await {
                Ok(_) => {},
                Err(e) => return Ok(ToolResult::error(format!("Failed to send input: {}", e))),
            }
        }

        // Read output with timeout
        let mut output = String::new();
        let start = std::time::Instant::now();

        loop {
            if start.elapsed().as_secs() > timeout_secs {
                break;
            }

            match tokio::time::timeout(Duration::from_millis(100), session.read()).await {
                Ok(Ok(pty_output)) => {
                    if !pty_output.content.is_empty() {
                        output.push_str(&pty_output.content);
                    }
                    if pty_output.waiting_for_input {
                        // Shell is waiting for input, assume command is done
                        break;
                    }
                    if matches!(session.state(), crate::commands::pty::PtyState::Exited) {
                        break;
                    }
                }
                Ok(Err(e)) => {
                    return Ok(ToolResult::error(format!("Failed to read from PTY: {}", e)));
                }
                Err(_) => {
                    // Timeout, check if we have output
                    if !output.is_empty() {
                        break;
                    }
                    continue;
                }
            }
        }

        // Kill the session
        let _ = session.kill().await;

        Ok(ToolResult::success(output))
    }

    // Task-related tool implementations
    async fn execute_create_task(&self, args: HashMap<String, serde_json::Value>) -> Result<ToolResult, CliError> {
        let title = args.get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CliError::Generic("Title is required".to_string()))?;

        let description = args.get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let priority = args.get("priority")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as i32;

        let task_data = serde_json::json!({
            "title": title,
            "description": description,
            "priority": priority,
        });

        match self.api_client.post_json("/v1/tasks", Some(task_data), true).await {
            Ok(res) => {
                let task_id = res.json.get("task")
                    .and_then(|t| t.get("id"))
                    .and_then(|i| i.as_str())
                    .unwrap_or("unknown");
                Ok(ToolResult::success(format!("Task created with ID: {}", task_id)))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to create task: {}", e))),
        }
    }

    async fn execute_update_task(&self, args: HashMap<String, serde_json::Value>) -> Result<ToolResult, CliError> {
        let task_id = args.get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CliError::Generic("Task ID is required".to_string()))?;

        let mut update_data = serde_json::json!({});
        if let Some(title) = args.get("title").and_then(|v| v.as_str()) {
            update_data["title"] = serde_json::Value::String(title.to_string());
        }
        if let Some(status) = args.get("status").and_then(|v| v.as_str()) {
            update_data["status"] = serde_json::Value::String(status.to_string());
        }

        match self.api_client.put_json(&format!("/v1/tasks/{}", task_id), Some(update_data), true).await {
            Ok(_res) => {
                Ok(ToolResult::success("Task updated successfully".to_string()))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to update task: {}", e))),
        }
    }

    async fn execute_list_tasks(&self, args: HashMap<String, serde_json::Value>) -> Result<ToolResult, CliError> {
        let status = args.get("status").and_then(|v| v.as_str());
        let _priority = args.get("priority").and_then(|v| v.as_u64()).map(|p| p as i32);

        let tasks = self.api_client.list_tasks(status.map(|s| s.to_string()), 20).await
            .map_err(|e| CliError::Generic(format!("Failed to list tasks: {}", e)))?;

        let task_list = tasks.iter()
            .map(|task| format!("- [{}] {} (Priority: {})", task.status, task.title, task.priority))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult::success(format!("Tasks:\n{}", task_list)))
    }

    async fn execute_complete_task(&self, args: HashMap<String, serde_json::Value>) -> Result<ToolResult, CliError> {
        let task_id = args.get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CliError::Generic("Task ID is required".to_string()))?;

        match self.api_client.post_json(&format!("/v1/tasks/{}/complete", task_id), None, true).await {
            Ok(_res) => {
                Ok(ToolResult::success("Task completed successfully".to_string()))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to complete task: {}", e))),
        }
    }
}