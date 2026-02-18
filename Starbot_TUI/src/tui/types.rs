// PHASE 3: TUI type definitions extracted from tui.rs
// Contains all structs, enums, and their implementations

use std::time::Instant;
use ratatui::widgets::ListState;
use serde_json::Value;

use crate::config::CliConfig;
use crate::cute::CuteMode;
use crate::api::ApiResponse;
use crate::errors::CliError;

// ============================================================================
// Request routing types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Quick,
    Standard,
    Deep,
}

impl Lane {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "quick" => Some(Self::Quick),
            "standard" => Some(Self::Standard),
            "deep" => Some(Self::Deep),
            _ => None,
        }
    }
}

// ============================================================================
// Model and workspace picker types
// ============================================================================

#[derive(Debug, Clone)]
pub struct ModelOption {
    pub provider: String,
    pub model: Option<String>,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct WorkspaceOption {
    pub id: String,
    pub name: String,
    pub root_path: Option<String>,
    pub archived: bool,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ThreadOption {
    pub id: String,
    pub title: String,
    pub mode: Option<String>,
    pub last_message_at: Option<String>,
    pub is_pinned: bool,
    pub message_count: usize,
}

// ============================================================================
// Memory management types (Phase 5)
// ============================================================================

#[derive(Debug, Clone)]
pub struct MemoryItem {
    pub id: String,
    pub scope: String,       // "global" or "project"
    pub project_id: Option<String>,
    pub item_type: String,   // "fact", "preference", "decision", "constraint", "todo"
    pub content: String,
    pub tags: Vec<String>,
    pub salience: f64,       // 0.0-1.0 importance score
    pub confidence: f64,     // 0.0-1.0 confidence score
    pub source: String,      // "manual", "auto", etc.
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct MemorySettings {
    pub enabled: bool,
    pub allow_auto_capture: bool,
    pub max_context_tokens: usize,
    pub max_items_injected: usize,
    pub include_global: bool,
    pub include_project: bool,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            allow_auto_capture: true,
            max_context_tokens: 600,
            max_items_injected: 12,
            include_global: true,
            include_project: true,
        }
    }
}

// ============================================================================
// Choice prompt modal types (SPEC17)
// ============================================================================

#[derive(Debug, Clone)]
pub enum ChoiceAction {
    SetWorkspace { workspace_id: String },
    Tool { tool_name: String, input: Value },
    Input { prompt: String },
    SendMessage { text: String },
}

#[derive(Debug, Clone)]
pub struct ChoiceOption {
    pub id: String,
    pub label: String,
    pub description: String,
    pub action: ChoiceAction,
}

#[derive(Debug, Clone)]
pub struct ChoicePrompt {
    pub id: String,
    pub title: String,
    pub hint: String,
    pub allow_custom: bool,
    pub custom_placeholder: String,
    pub options: Vec<ChoiceOption>,
}

#[derive(Debug, Clone)]
pub struct TextPromptState {
    pub prompt: String,
    pub input: Vec<char>,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub last_modified: Option<String>,
}

// ============================================================================
// Chat message types
// ============================================================================

#[derive(Debug, Clone)]
pub struct ChatMsg {
    pub role: ChatRole,
    pub content: String,
    pub sendable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

impl ChatRole {
    pub fn as_str(self) -> &'static str {
        match self {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::System => "system",
        }
    }
}

// ============================================================================
// UI mode enum
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Chat,
    ModelPicker,
    WorkspacePicker,
    ThreadPicker,
    MemoryPanel,
    ChoiceModal,
    TextPromptModal,
    Help,
    ToolCard,
    FileBrowser,
}

// ============================================================================
// Provider health hints
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub struct ProviderHints {
    pub vertex_ok: Option<bool>,
    pub azure_present: bool,
    pub cf_present: bool,
}

// ============================================================================
// Tool approval types
// ============================================================================

#[derive(Debug, Clone)]
pub struct PendingToolCard {
    pub tool_name: String,
    pub target_files: Vec<String>,
    pub preview: String,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone)]
pub struct ToolApprovalEntry {
    pub tool_name: String,
    pub approved: bool,
}

// ============================================================================
// Main App state
// ============================================================================

#[derive(Debug)]
pub struct App {
    pub mode: Mode,
    pub should_quit: bool,

    pub api_url: String,
    pub config: CliConfig,
    pub profile: String,
    pub token_present: bool,
    pub cute: CuteMode,
    pub rng: u64,
    pub last_phrase: Option<&'static str>,
    pub success_count: u32,
    pub lane: Option<Lane>,
    pub hints: ProviderHints,
    pub spinner_step: u64,
    pub spinner_last: Instant,
    pub bg_tasks: u32,

    pub messages: Vec<ChatMsg>,
    pub input: Vec<char>,
    pub cursor: usize,
    pub waiting: bool,

    // Inline completion state
    pub completions: Vec<Completion>,
    pub selected_completion: Option<usize>,
    pub show_completions: bool,
    pub completion_active: bool,

    pub status: String,
    pub last_request_id: Option<String>,
    pub last_elapsed_ms: Option<u128>,
    pub last_provider: Option<String>,
    pub last_model: Option<String>,
    pub last_usage: Option<String>,

    pub activity_lines: Vec<String>,
    pub current_file: Option<String>,
    pub auto_edits: bool,
    pub working_dir: String,

    pub model_options: Vec<ModelOption>,
    pub model_state: ListState,
    pub selected_provider: String,
    pub selected_model: Option<String>,

    pub workspace_options: Vec<WorkspaceOption>,
    pub workspace_state: ListState,
    pub selected_workspace_id: Option<String>,
    pub selected_workspace_name: Option<String>,
    pub pending_workspace_retry: bool,

    // Thread management state (PHASE 5)
    pub thread_options: Vec<ThreadOption>,
    pub thread_state: ListState,
    pub active_thread_id: Option<String>,
    pub active_thread_title: Option<String>,

    // Memory management state (Phase 5)
    pub memory_items: Vec<MemoryItem>,
    pub memory_state: ListState,
    pub memory_settings: MemorySettings,
    pub memory_enabled: bool,  // Quick access to enabled status

    // Choice prompt modal state (SPEC17)
    pub choice_prompt: Option<ChoicePrompt>,
    pub choice_state: ListState,
    pub text_prompt: Option<TextPromptState>,

    pub scroll_from_bottom: usize,
    pub show_debug: bool,

    // Tool card state
    pub pending_tool: Option<PendingToolCard>,
    pub tool_approval_history: Vec<ToolApprovalEntry>,

    // File browser state
    pub file_browser_path: String,
    pub file_browser_files: Vec<FileNode>,
    pub file_browser_state: ListState,
    pub file_browser_selected: Option<String>,
}

// ============================================================================
// Inline completion types
// ============================================================================

#[derive(Debug, Clone)]
pub struct Completion {
    pub text: String,
    pub confidence: f64,
    pub language: String,
}

#[derive(Debug)]
pub struct CompletionRequest {
    pub file_path: String,
    pub content: String,
    pub cursor_pos: (usize, usize), // (line, column)
    pub suggestions: Vec<Completion>,
}

// ============================================================================
// Async message enum
// ============================================================================

#[derive(Debug)]
pub enum TuiMsg {
    Models(Result<ApiResponse, CliError>),
    Health(Result<ApiResponse, CliError>),
    Workspaces(Result<ApiResponse, CliError>),
    Threads(Result<ApiResponse, CliError>),
    Memory(Result<ApiResponse, CliError>),
    MemorySettings(Result<ApiResponse, CliError>),
    Chat(Result<ApiResponse, CliError>),
    Tool(String, Result<ApiResponse, CliError>),
    // New Starbot_API messages
    Projects(Result<ApiResponse, CliError>),
    Chats(Result<ApiResponse, CliError>),
    Messages(Result<ApiResponse, CliError>),
    ProjectCreated(Result<ApiResponse, CliError>),
    ChatCreated(Result<ApiResponse, CliError>),
    MessageAdded(Result<ApiResponse, CliError>),
    ChatCancelled(Result<ApiResponse, CliError>),
    // Completion messages
    CompletionRequest(String, Result<ApiResponse, CliError>),
    // File browser messages
    FileListRequest(String, String, Result<ApiResponse, CliError>),
    // Streaming events
    StreamStatus(String),
    StreamToken(String),
    StreamDone(serde_json::Value),
    StreamError(String),
}
