//! Unified CLI Agent - Task-Oriented AI Assistant
//!
//! This module implements an agent that creates chats, calls the generation
//! endpoint (`POST /v1/chats/:chatId/run`) via SSE streaming, handles tool
//! calls, and manages tasks.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api::ApiClient;
use crate::app::Runtime;
use crate::errors::CliError;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Agent state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    Idle,
    Reasoning,
    Executing,
    Processing,
    Error,
}

/// Agent configuration
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub system_prompt: String,
    pub max_iterations: u32,
    pub enable_tasks: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "auto".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            system_prompt: include_str!("agent_system_prompt.md").to_string(),
            max_iterations: 50,
            enable_tasks: true,
        }
    }
}

/// Message in agent conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_results: Option<Vec<ToolResult>>,
}

/// Tool call request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

impl ToolResult {
    pub fn success(content: String) -> Self {
        Self { success: true, output: content, error: None, metadata: None }
    }
    pub fn error(content: String) -> Self {
        Self { success: false, output: content, error: None, metadata: None }
    }
}

// ---------------------------------------------------------------------------
// Agent run – the real entry point
// ---------------------------------------------------------------------------

/// Handle `starbott agent run "prompt"`.
///
/// Flow:
///   1. Ensure a project exists (or pick the first one).
///   2. Create or reuse a chat.
///   3. Add the user message.
///   4. Stream generation via `POST /v1/chats/:chatId/run`.
///   5. Print tokens as they arrive, report tool calls.
pub async fn handle_run(
    api: &ApiClient,
    runtime: &Runtime,
    prompt: String,
    project_id: Option<String>,
    chat_id: Option<String>,
    model_prefs: Option<String>,
) -> Result<(), CliError> {
    // 1. Resolve project
    let pid = match project_id {
        Some(id) => id,
        None => resolve_or_create_project(api).await?,
    };

    // 2. Resolve chat
    let cid = match chat_id {
        Some(id) => id,
        None => create_chat(api, &pid).await?,
    };

    // 3. Post user message to the chat
    add_message(api, &cid, "user", &prompt).await?;

    // 4. Stream generation
    let body = json!({
        "mode": "standard",
        "auto": true,
        "model_prefs": model_prefs.as_deref().unwrap_or("auto"),
        "client_context": {
            "working_dir": std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("/"))
                .display()
                .to_string(),
        },
    });

    let rx = api.post_stream(
        &format!("/v1/chats/{}/run", cid),
        Some(body),
        true,
    ).await?;

    stream_to_terminal(rx, runtime).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// SSE stream consumer
// ---------------------------------------------------------------------------

async fn stream_to_terminal(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<crate::api::StreamEvent>,
    runtime: &Runtime,
) -> Result<(), CliError> {
    let mut full_response = String::new();
    let mut _tool_active = false;

    while let Some(event) = rx.recv().await {
        match event.event_type.as_str() {
            // Streaming tokens from the model
            "token.delta" => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
                        full_response.push_str(text);
                        // Print token immediately for streaming UX
                        print!("{}", text);
                        let _ = std::io::stdout().flush();
                    }
                }
            }

            // Status updates from the backend pipeline
            "status" => {
                if runtime.output.verbose {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        if let Some(msg) = parsed.get("message").and_then(|m| m.as_str()) {
                            eprintln!("\x1b[90m[status] {}\x1b[0m", msg);
                        }
                    }
                }
            }

            // Tool execution start
            "tool.start" => {
                _tool_active = true;
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                    let name = parsed.get("tool_name").and_then(|n| n.as_str()).unwrap_or("?");
                    eprintln!("\x1b[33m[tool] executing: {}\x1b[0m", name);
                }
            }

            // Tool arguments (debug)
            "tool.arguments" => {
                if runtime.output.verbose || runtime.output.debug {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        let name = parsed.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                        let empty = json!({});
                        let args = parsed.get("arguments").unwrap_or(&empty);
                        eprintln!("\x1b[90m[tool.args] {} {}\x1b[0m", name, args);
                    }
                }
            }

            // Tool execution end
            "tool.end" => {
                _tool_active = false;
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                    let name = parsed.get("tool_name").and_then(|n| n.as_str()).unwrap_or("?");
                    let success = parsed.get("success").and_then(|s| s.as_bool()).unwrap_or(false);
                    let icon = if success { "\x1b[32m✓\x1b[0m" } else { "\x1b[31m✗\x1b[0m" };
                    let duration = parsed.get("duration_ms").and_then(|d| d.as_u64()).unwrap_or(0);
                    eprintln!("{} \x1b[33m[tool] {} ({}ms)\x1b[0m", icon, name, duration);

                    // Show preview if available
                    if let Some(preview) = parsed.get("preview").and_then(|p| p.as_str()) {
                        if !preview.is_empty() && runtime.output.verbose {
                            eprintln!("\x1b[90m  {}\x1b[0m", preview);
                        }
                    }
                }
            }

            // Memory injection debug
            "memory.injected" => {
                if runtime.output.verbose {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        let identity = parsed.get("identity_chunks").and_then(|c| c.as_u64()).unwrap_or(0);
                        let chat = parsed.get("chat_chunks").and_then(|c| c.as_u64()).unwrap_or(0);
                        eprintln!(
                            "\x1b[90m[memory] identity={} chat={}\x1b[0m",
                            identity, chat
                        );
                    }
                }
            }

            // Interpreter debug
            "interpreter.debug" => {
                if runtime.output.debug {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        let intent = parsed.get("primary_intent").and_then(|i| i.as_str()).unwrap_or("?");
                        let confidence = parsed.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0);
                        eprintln!(
                            "\x1b[90m[interpreter] intent={} confidence={:.2}\x1b[0m",
                            intent, confidence
                        );
                    }
                }
            }

            // Final message with metadata
            "message.final" => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                    // If we haven't been streaming tokens (e.g. clarification), print content now
                    if full_response.is_empty() {
                        if let Some(content) = parsed.get("content").and_then(|c| c.as_str()) {
                            println!("{}", content);
                        }
                    } else {
                        // Ensure final newline after streamed tokens
                        println!();
                    }

                    // Print usage/model info if verbose
                    if runtime.output.verbose {
                        let provider = parsed.get("provider").and_then(|p| p.as_str()).unwrap_or("?");
                        let model = parsed.get("modelDisplayName").and_then(|m| m.as_str()).unwrap_or("?");
                        let usage = parsed.get("usage");
                        let prompt_tokens = usage.and_then(|u| u.get("promptTokens")).and_then(|t| t.as_u64()).unwrap_or(0);
                        let completion_tokens = usage.and_then(|u| u.get("completionTokens")).and_then(|t| t.as_u64()).unwrap_or(0);
                        eprintln!(
                            "\x1b[90m[model] {} ({}) prompt={} completion={}\x1b[0m",
                            model, provider, prompt_tokens, completion_tokens
                        );
                    }
                }
            }

            // Chat updated
            "chat.updated" => {
                if runtime.output.verbose {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        let id = parsed.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                        let title = parsed.get("title").and_then(|t| t.as_str()).unwrap_or("?");
                        eprintln!("\x1b[90m[chat] id={} title=\"{}\"\x1b[0m", id, title);
                    }
                }
            }

            // Errors
            "error" => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                    let msg = parsed.get("message").and_then(|m| m.as_str())
                        .or_else(|| parsed.get("error_message").and_then(|m| m.as_str()))
                        .unwrap_or("Unknown error");
                    let fatal = parsed.get("fatal").and_then(|f| f.as_bool()).unwrap_or(false);

                    if fatal {
                        eprintln!("\x1b[31m[error] {}\x1b[0m", msg);
                    } else {
                        // Non-fatal (e.g. provider fallback)
                        if runtime.output.verbose {
                            let provider = parsed.get("provider").and_then(|p| p.as_str()).unwrap_or("?");
                            eprintln!(
                                "\x1b[33m[warn] {} failed: {}\x1b[0m",
                                provider, msg
                            );
                        }
                    }
                }
            }

            _ => {
                // Unknown event type
                if runtime.output.debug {
                    eprintln!("\x1b[90m[event:{}] {}\x1b[0m", event.event_type, event.data);
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// API helpers
// ---------------------------------------------------------------------------

/// Get or create a default project to scope the chat.
async fn resolve_or_create_project(api: &ApiClient) -> Result<String, CliError> {
    // Try listing existing projects
    let res = api.get_json("/v1/projects", None, true).await?;
    if let Some(projects) = res.json.as_array() {
        if let Some(first) = projects.first() {
            if let Some(id) = first.get("id").and_then(|i| i.as_str()) {
                return Ok(id.to_string());
            }
        }
    }

    // Also try nested format
    if let Some(projects) = res.json.get("projects").and_then(|p| p.as_array()) {
        if let Some(first) = projects.first() {
            if let Some(id) = first.get("id").and_then(|i| i.as_str()) {
                return Ok(id.to_string());
            }
        }
    }

    // No projects — create one
    let body = json!({ "name": "Default" });
    let res = api.post_json("/v1/projects", Some(body), true).await?;
    let id = res.json.get("id")
        .or_else(|| res.json.get("project").and_then(|p| p.get("id")))
        .and_then(|i| i.as_str())
        .ok_or_else(|| CliError::Generic("Failed to create project".to_string()))?;
    Ok(id.to_string())
}

/// Create a new chat inside a project.
async fn create_chat(api: &ApiClient, project_id: &str) -> Result<String, CliError> {
    let body = json!({ "title": "Agent Chat" });
    let res = api.post_json(
        &format!("/v1/projects/{}/chats", project_id),
        Some(body),
        true,
    ).await?;

    let id = res.json.get("id")
        .or_else(|| res.json.get("chat").and_then(|c| c.get("id")))
        .and_then(|i| i.as_str())
        .ok_or_else(|| CliError::Generic("Failed to create chat".to_string()))?;
    Ok(id.to_string())
}

/// Add a message to a chat.
async fn add_message(api: &ApiClient, chat_id: &str, role: &str, content: &str) -> Result<(), CliError> {
    let body = json!({ "role": role, "content": content });
    api.post_json(
        &format!("/v1/chats/{}/messages", chat_id),
        Some(body),
        true,
    ).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// CLIAgent – higher-level wrapper (kept for backward compat)
// ---------------------------------------------------------------------------

/// Agent stats
#[derive(Debug, Default)]
pub struct AgentStats {
    pub total_requests: u64,
    pub total_tool_calls: u64,
    pub successful_tasks: u64,
    pub failed_tasks: u64,
}

/// Agent context
pub struct AgentContext {
    pub session_id: String,
    pub working_directory: PathBuf,
    pub messages: Vec<Message>,
    pub current_task: Option<String>,
}

/// Main CLI Agent
pub struct CLIAgent {
    config: AgentConfig,
    api_client: ApiClient,
    context: Option<AgentContext>,
    state: AgentState,
    stats: AgentStats,
}

impl CLIAgent {
    pub fn new(config: AgentConfig, api_client: ApiClient, _session_id: String) -> Self {
        Self {
            config,
            api_client,
            context: None,
            state: AgentState::Idle,
            stats: AgentStats::default(),
        }
    }

    pub async fn initialize(&mut self) -> Result<(), CliError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let working_directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        self.context = Some(AgentContext {
            session_id,
            working_directory,
            messages: Vec::new(),
            current_task: None,
        });
        self.state = AgentState::Idle;
        Ok(())
    }

    pub async fn process(&mut self, user_input: String) -> Result<String, CliError> {
        self.state = AgentState::Reasoning;

        if let Some(ref mut ctx) = self.context {
            ctx.messages.push(Message {
                role: "user".to_string(),
                content: user_input.clone(),
                tool_calls: None,
                tool_results: None,
            });
        }

        // Use the real API: resolve project, create chat, run generation
        let pid = resolve_or_create_project(&self.api_client).await?;
        let cid = create_chat(&self.api_client, &pid).await?;
        add_message(&self.api_client, &cid, "user", &user_input).await?;

        let body = json!({
            "mode": "standard",
            "auto": true,
            "model_prefs": &self.config.model,
            "client_context": {
                "working_dir": self.context.as_ref()
                    .map(|c| c.working_directory.display().to_string())
                    .unwrap_or_else(|| "/".to_string()),
            },
        });

        let rx = self.api_client.post_stream(
            &format!("/v1/chats/{}/run", cid),
            Some(body),
            true,
        ).await?;

        // Collect full response from stream
        let mut full_response = String::new();
        let mut rx = rx;
        while let Some(event) = rx.recv().await {
            match event.event_type.as_str() {
                "token.delta" => {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
                            full_response.push_str(text);
                        }
                    }
                }
                "message.final" => {
                    if full_response.is_empty() {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&event.data) {
                            if let Some(content) = parsed.get("content").and_then(|c| c.as_str()) {
                                full_response = content.to_string();
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.stats.total_requests += 1;
        self.state = AgentState::Idle;

        if let Some(ref mut ctx) = self.context {
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content: full_response.clone(),
                tool_calls: None,
                tool_results: None,
            });
        }

        Ok(full_response)
    }

    pub fn state(&self) -> &AgentState { &self.state }
    pub fn stats(&self) -> &AgentStats { &self.stats }

    pub fn set_current_task(&mut self, task_id: Option<String>) {
        if let Some(ref mut ctx) = self.context {
            ctx.current_task = task_id;
        }
    }
}

/// CLI Agent Commands (factory + task processor)
pub struct CLIAgentCommands;

impl CLIAgentCommands {
    pub async fn create(config: AgentConfig, api_client: ApiClient) -> Result<CLIAgent, CliError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let mut agent = CLIAgent::new(config, api_client, session_id);
        agent.initialize().await?;
        Ok(agent)
    }

    pub async fn process_task(agent: &mut CLIAgent, task_id: &str) -> Result<(), CliError> {
        agent.set_current_task(Some(task_id.to_string()));

        // Fetch task details first
        let task = agent.api_client.get_task(task_id).await?;

        let prompt = format!(
            "Process the following task:\n\nTitle: {}\nDescription: {}\nStatus: {}\nPriority: {}\n\nPlease complete this task.",
            task.title,
            task.description.as_deref().unwrap_or("No description"),
            task.status,
            task.priority,
        );

        let result = agent.process(prompt).await?;

        // Mark task started
        let _ = agent.api_client.start_task(task_id).await;

        if result.contains("completed") || result.contains("done") || result.contains("finished") {
            let _ = agent.api_client.complete_task(task_id).await;
            agent.stats.successful_tasks += 1;
        }

        Ok(())
    }
}
