use std::io;
use std::time::{Duration, Instant};

use clap::Args;
use crossterm::cursor::Show;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::block::{Position, Title};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::{Frame, Terminal};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::api::{ApiClient, ApiResponse};
use crate::app::Runtime;
use crate::config::{CliConfig, profile_mut, profile_ref, save_config};
use crate::cute::{CuteMode, load_cute_mode};
use crate::errors::CliError;

// PHASE 3: Import handler modules and response parsing
use crate::parse::response::{extract_reply, extract_provider_model, extract_usage_line};
use crate::tui::types::*;
use crate::tui::handlers::{handle_event, handle_tui_msg};
use crate::tui::handlers::async_ops::{
    spawn_models_fetch, spawn_health_fetch, spawn_workspaces_fetch,
    spawn_chat_request_stream, spawn_tool_propose,
};

#[derive(Debug, Args)]
pub struct TuiArgs {
    /// Model selector. Examples: "vertex:gemini-3-flash-preview" or "auto"
    #[arg(short = 'm', long = "model")]
    pub model: Option<String>,
}
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self, CliError> {
        enable_raw_mode()
            .map_err(|e| CliError::Generic(format!("Failed to enable raw mode: {e}")))?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)
            .map_err(|e| CliError::Generic(format!("Failed to enter alternate screen: {e}")))?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
    }
}

pub async fn handle(runtime: &Runtime, args: TuiArgs) -> Result<(), CliError> {
    if runtime.output.json {
        return Err(CliError::Usage(
            "`--json` is not supported for `starbott tui`.".to_string(),
        ));
    }

    let api = runtime.api_client()?;
    let api_url = runtime.resolved_api_url()?;
    let profile = runtime.active_profile();
    let config = runtime.config.clone();
    let token_present = runtime.resolved_token().is_some();
    let cute = load_cute_mode();
    let selected_workspace_id = profile_ref(&config, &profile).and_then(|p| p.workspace_id.clone());

    let (initial_provider, initial_model) = parse_model_selector(args.model.as_deref());

    let guard = TerminalGuard::enter()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)
        .map_err(|e| CliError::Generic(format!("Failed to init terminal: {e}")))?;
    terminal
        .clear()
        .map_err(|e| CliError::Generic(format!("Failed to clear terminal: {e}")))?;
    terminal
        .hide_cursor()
        .map_err(|e| CliError::Generic(format!("Failed to hide cursor: {e}")))?;

    let mut app = App {
        mode: Mode::Chat,
        should_quit: false,
        api_url,
        config,
        profile,
        token_present,
        cute,
        rng: seed_rng(),
        last_phrase: None,
        success_count: 0,
        lane: None,
        hints: ProviderHints {
            vertex_ok: None,
            azure_present: false,
            cf_present: false,
        },
        spinner_step: 0,
        spinner_last: Instant::now(),
        bg_tasks: 0,
        messages: vec![ChatMsg {
            role: ChatRole::System,
            content: "Starbot TUI. Enter to send. F2 models. F3 workspace. F1 help. Esc quit.".to_string(),
            sendable: false,
        }],
        input: Vec::new(),
        cursor: 0,
        waiting: false,
        status: startup_status(cute).to_string(),
        last_request_id: None,
        last_elapsed_ms: None,
        last_provider: None,
        last_model: None,
        last_usage: None,
        activity_lines: Vec::new(),
        current_file: None,
        auto_edits: false,
        working_dir: std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string()),
        model_options: default_model_options(),
        model_state: ListState::default(),
        selected_provider: initial_provider,
        selected_model: initial_model,
        // Inline completion state
        completions: Vec::new(),
        selected_completion: None,
        show_completions: false,
        completion_active: false,
        workspace_options: Vec::new(),
        workspace_state: ListState::default(),
        selected_workspace_id,
        selected_workspace_name: None,
        pending_workspace_retry: false,
        thread_options: Vec::new(),
        thread_state: ListState::default(),
        active_thread_id: None,
        active_thread_title: None,
        memory_items: Vec::new(),
        memory_state: ListState::default(),
        memory_settings: MemorySettings::default(),
        memory_enabled: true,
        choice_prompt: None,
        choice_state: ListState::default(),
        text_prompt: None,
        scroll_from_bottom: 0,
        show_debug: false,
        pending_tool: None,
        tool_approval_history: Vec::new(),
        // File browser state
        file_browser_path: "/".to_string(),
        file_browser_files: Vec::new(),
        file_browser_state: ListState::default(),
        file_browser_selected: None,
    };

    app.model_state.select(Some(0));
    app.workspace_state.select(Some(0));

    let (tx, mut rx) = mpsc::unbounded_channel::<TuiMsg>();
    app.bg_tasks = app.bg_tasks.saturating_add(2);
    spawn_models_fetch(api.clone(), tx.clone());
    spawn_health_fetch(api.clone(), tx.clone());
    if app.token_present {
        app.bg_tasks = app.bg_tasks.saturating_add(1);
        spawn_workspaces_fetch(api.clone(), tx.clone());
    }

    loop {
        update_spinner(&mut app);
        terminal
            .draw(|f| ui(f, &mut app))
            .map_err(|e| CliError::Generic(format!("Failed to draw: {e}")))?;

        if app.should_quit {
            break;
        }

        while let Ok(msg) = rx.try_recv() {
            handle_tui_msg(&api, &tx, &mut app, msg);
        }

        // Don't redraw at full speed when idle; keep a calmer cadence.
        let poll_ms = if app.waiting || app.bg_tasks > 0 || app.mode != Mode::Chat {
            50
        } else {
            120
        };
        if crossterm::event::poll(Duration::from_millis(poll_ms))
            .map_err(|e| CliError::Generic(format!("Event poll failed: {e}")))?
        {
            let event = crossterm::event::read()
                .map_err(|e| CliError::Generic(format!("Event read failed: {e}")))?;
            if let Err(err) = handle_event(&api, &tx, &mut app, event) {
                app.messages.push(ChatMsg {
                    role: ChatRole::System,
                    content: format!("Error: {err}"),
                    sendable: false,
                });
                app.status = "Error".to_string();
            }
        }
    }

    terminal
        .show_cursor()
        .map_err(|e| CliError::Generic(format!("Failed to restore cursor: {e}")))?;
    drop(guard);
    Ok(())
}

fn default_model_options() -> Vec<ModelOption> {
    vec![
        ModelOption {
            provider: "azure".to_string(),
            model: Some("claude-haiku-4-5".to_string()),
            label: "Claude Haiku 4.5".to_string(),
        },
        ModelOption {
            provider: "auto".to_string(),
            model: None,
            label: "Auto".to_string(),
        },
        ModelOption {
            provider: "kimi".to_string(),
            model: None,
            label: "Kimi K2".to_string(),
        },
        ModelOption {
            provider: "vertex".to_string(),
            model: Some("gemini-3-flash-preview".to_string()),
            label: "Gemini 3 Flash Preview".to_string(),
        },
    ]
}

fn parse_model_selector(selector: Option<&str>) -> (String, Option<String>) {
    let Some(raw) = selector else {
        return ("azure".to_string(), Some("claude-haiku-4-5".to_string()));
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return ("azure".to_string(), Some("claude-haiku-4-5".to_string()));
    }

    if let Some((provider, model)) = trimmed.split_once(':') {
        let p = provider.trim().to_ascii_lowercase();
        let m = model.trim().to_string();
        return (p, if m.is_empty() { None } else { Some(m) });
    }

    let lower = trimmed.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "auto" | "kimi" | "gemini" | "vertex" | "cloudflare" | "azure" | "openai"
    ) {
        return (lower, None);
    }

    // Convenient default for "gemini-*" model ids.
    ("vertex".to_string(), Some(trimmed.to_string()))
}








pub fn parse_model_options(payload: &Value) -> Option<Vec<ModelOption>> {
    let providers = payload.get("providers")?.as_array()?;
    let mut options = Vec::new();

    for p in providers {
        let label = p
            .get("label")
            .and_then(|v| v.as_str())
            .or_else(|| p.get("id").and_then(|v| v.as_str()))
            .unwrap_or("unknown")
            .to_string();

        let provider = p
            .get("provider")
            .and_then(|v| v.as_str())
            .or_else(|| p.get("id").and_then(|v| v.as_str()))
            .unwrap_or("auto")
            .to_string();

        let model = p
            .get("model")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        // Normalize provider for "auto"/"kimi" entries where provider is absent and "id" is used.
        let provider = if provider.contains(':') {
            provider.split(':').next().unwrap_or("auto").to_string()
        } else {
            provider
        };

        options.push(ModelOption {
            provider,
            model,
            label,
        });
    }

    Some(options)
}

pub fn parse_workspace_options(payload: &Value) -> Option<Vec<WorkspaceOption>> {
    let workspaces = payload.get("workspaces")?.as_array()?;
    let mut options = Vec::new();

    for w in workspaces {
        let id = w.get("id").and_then(|v| v.as_str()).map(str::trim).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let name = w
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("workspace")
            .to_string();
        let root_path = w
            .get("rootPath")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let archived = w.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
        let last_used_at = w
            .get("lastUsedAt")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        options.push(WorkspaceOption {
            id: id.to_string(),
            name,
            root_path,
            archived,
            last_used_at,
        });
    }

    Some(options)
}

pub fn parse_thread_options(payload: &Value) -> Option<Vec<ThreadOption>> {
    let threads = payload.get("threads")?.as_array()?;
    let mut options = Vec::new();

    for t in threads {
        let id = t.get("id").and_then(|v| v.as_str()).map(str::trim).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let title = t
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Untitled")
            .to_string();
        let mode = t
            .get("mode")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let last_message_at = t
            .get("lastMessageAt")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let is_pinned = t.get("isPinned").and_then(|v| v.as_bool()).unwrap_or(false);
        let message_count = t.get("_count")
            .and_then(|v| v.get("messages"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        options.push(ThreadOption {
            id: id.to_string(),
            title,
            mode,
            last_message_at,
            is_pinned,
            message_count,
        });
    }

    Some(options)
}

pub fn parse_memory_items(payload: &Value) -> Option<Vec<MemoryItem>> {
    let items = payload.get("items")?.as_array()?;
    let mut options = Vec::new();

    for item in items {
        let id = item.get("id").and_then(|v| v.as_str()).map(str::trim).unwrap_or("");
        if id.is_empty() {
            continue;
        }

        let scope = item.get("scope").and_then(|v| v.as_str()).unwrap_or("global").to_string();
        let project_id = item.get("projectId").and_then(|v| v.as_str()).map(|s| s.to_string());
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("fact").to_string();
        let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let tags = item.get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();
        let salience = item.get("salience").and_then(|v| v.as_f64()).unwrap_or(0.5);
        let confidence = item.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.7);
        let source = item.get("source").and_then(|v| v.as_str()).unwrap_or("manual").to_string();
        let enabled = item.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        let created_at = item.get("createdAt").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let updated_at = item.get("updatedAt").and_then(|v| v.as_str()).unwrap_or("").to_string();

        options.push(MemoryItem {
            id: id.to_string(),
            scope,
            project_id,
            item_type,
            content,
            tags,
            salience,
            confidence,
            source,
            enabled,
            created_at,
            updated_at,
        });
    }

    Some(options)
}

pub fn parse_memory_settings(payload: &Value) -> Option<MemorySettings> {
    Some(MemorySettings {
        enabled: payload.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
        allow_auto_capture: payload.get("allowAutoCapture").and_then(|v| v.as_bool()).unwrap_or(true),
        max_context_tokens: payload.get("maxContextTokens").and_then(|v| v.as_u64()).unwrap_or(600) as usize,
        max_items_injected: payload.get("maxItemsInjected").and_then(|v| v.as_u64()).unwrap_or(12) as usize,
        include_global: payload.get("includeGlobal").and_then(|v| v.as_bool()).unwrap_or(true),
        include_project: payload.get("includeProject").and_then(|v| v.as_bool()).unwrap_or(true),
    })
}

pub fn parse_choice_prompt(payload: &Value) -> Option<ChoicePrompt> {
    let cp = payload
        .get("choicePrompt")
        .or_else(|| payload.get("choice_prompt"))?;
    let title = cp
        .get("title")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Choose an option")
        .to_string();
    let id = cp
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("choice")
        .to_string();
    let hint = cp
        .get("hint")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let allow_custom = cp
        .get("allowCustom")
        .or_else(|| cp.get("allow_custom"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let custom_placeholder = cp
        .get("customPlaceholder")
        .or_else(|| cp.get("custom_placeholder"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("")
        .to_string();

    let options_v = cp.get("options")?.as_array()?;
    let mut options: Vec<ChoiceOption> = Vec::new();

    for o in options_v {
        let oid = o.get("id").and_then(|v| v.as_str()).map(str::trim).unwrap_or("");
        let label = o
            .get("label")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(oid)
            .to_string();
        if label.is_empty() {
            continue;
        }
        let desc = o
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("")
            .to_string();

        let action = o.get("action")?;
        let typ = action.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let parsed_action = match typ {
            "set_workspace" => {
                let ws = action
                    .get("workspaceId")
                    .or_else(|| action.get("workspace_id"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .unwrap_or("");
                if ws.is_empty() {
                    continue;
                }
                ChoiceAction::SetWorkspace {
                    workspace_id: ws.to_string(),
                }
            }
            "tool" => {
                let tool = action
                    .get("toolName")
                    .or_else(|| action.get("tool_name"))
                    .or_else(|| action.get("tool"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .unwrap_or("");
                if tool.is_empty() {
                    continue;
                }
                let input = action.get("input").cloned().unwrap_or_else(|| json!({}));
                ChoiceAction::Tool {
                    tool_name: tool.to_string(),
                    input,
                }
            }
            "input" => {
                let prompt = action
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .unwrap_or("Input");
                ChoiceAction::Input {
                    prompt: prompt.to_string(),
                }
            }
            "send_message" => {
                let text = action
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .unwrap_or("");
                if text.is_empty() {
                    continue;
                }
                ChoiceAction::SendMessage {
                    text: text.to_string(),
                }
            }
            _ => {
                continue;
            }
        };

        options.push(ChoiceOption {
            id: oid.to_string(),
            label,
            description: desc,
            action: parsed_action,
        });
    }

    if options.is_empty() {
        return None;
    }

    Some(ChoicePrompt {
        id,
        title,
        hint,
        allow_custom,
        custom_placeholder,
        options,
    })
}

pub fn find_selected_model_index(
    options: &[ModelOption],
    provider: &str,
    model: Option<&str>,
) -> Option<usize> {
    options.iter().position(|o| {
        o.provider == provider
            && match (o.model.as_deref(), model) {
                (None, None) => true,
                (Some(a), Some(b)) => a == b,
                _ => false,
            }
    })
}








fn ui(f: &mut Frame<'_>, app: &mut App) {
    let size = f.area();

    // Layout: file panel | main list | input | info panel
    // If a file is active, show it at top; otherwise give all space to the main list.
    let has_file = app.current_file.is_some();
    let file_height = if has_file { 6u16 } else { 0 };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(file_height),  // file being edited
            Constraint::Min(1),              // main list (chat)
            Constraint::Length(3),           // input prompt
            Constraint::Length(4),           // info panel (dir/model/auto-edits/status)
        ])
        .split(size);

    // ── File panel (top) ──
    if has_file {
        render_file_panel(f, app, layout[0]);
    }

    // ── Main list (chat) ──
    let (chat_area, debug_area) = if app.show_debug {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(layout[1]); // main list row
        (cols[0], Some(cols[1]))
    } else {
        (layout[1], None)
    };

    let chat = render_chat(app, chat_area);
    f.render_widget(chat, chat_area);

    if let Some(area) = debug_area {
        let dbg = render_debug(app);
        f.render_widget(dbg, area);
    }

    // ── Input prompt (enclosed with heart prefix) ──
    let input = render_input(app);
    f.render_widget(input, layout[2]);

    if app.mode == Mode::Chat {
        let prompt_cols = input_prompt_len(app) as u16;
        let x = layout[2]
            .x
            .saturating_add(1)
            .saturating_add(prompt_cols)
            .saturating_add(app.cursor as u16);
        let y = layout[2].y.saturating_add(1);
        f.set_cursor_position((x.min(layout[2].x + layout[2].width - 2), y));
    }

    // ── Info panel (bottom, below the input) ──
    render_info_panel(f, app, layout[3]);

    // ── Popups ──
    match app.mode {
        Mode::ModelPicker => {
            let area = centered_rect(80, 70, size);
            f.render_widget(Clear, area);
            render_model_picker_popup(f, app, area);
        }
        Mode::WorkspacePicker => {
            let area = centered_rect(80, 70, size);
            f.render_widget(Clear, area);
            render_workspace_picker_popup(f, app, area);
        }
        Mode::ThreadPicker => {
            let area = centered_rect(80, 70, size);
            f.render_widget(Clear, area);
            render_thread_picker_popup(f, app, area);
        }
        Mode::MemoryPanel => {
            let area = centered_rect(80, 70, size);
            f.render_widget(Clear, area);
            render_memory_panel_popup(f, app, area);
        }
        Mode::ChoiceModal => {
            let area = centered_rect(80, 70, size);
            f.render_widget(Clear, area);
            render_choice_modal_popup(f, app, area);
        }
        Mode::TextPromptModal => {
            let area = centered_rect(70, 40, size);
            f.render_widget(Clear, area);
            render_text_prompt_popup(f, app, area);
        }
        Mode::Help => {
            let area = centered_rect(70, 60, size);
            f.render_widget(Clear, area);
            let popup = render_help(app);
            f.render_widget(popup, area);
        }
        Mode::ToolCard => {
            let area = centered_rect(70, 50, size);
            f.render_widget(Clear, area);
            let popup = render_tool_card(app);
            f.render_widget(popup, area);
        }
        Mode::FileBrowser => {
            let area = centered_rect(80, 70, size);
            f.render_widget(Clear, area);
            render_file_browser_popup(f, app, area);
        }
        Mode::Chat => {}
    }
}

fn render_file_panel(f: &mut Frame<'_>, app: &App, area: Rect) {
    let file_name = app.current_file.as_deref().unwrap_or("");
    let title = format!(" {} ", file_name);

    let mut lines = Vec::new();
    // Show last few activity lines as context for what's happening to this file
    let start = app.activity_lines.len().saturating_sub(4);
    for line in &app.activity_lines[start..] {
        lines.push(Line::from(vec![Span::styled(
            format!("  {line}"),
            Style::default().fg(c_muted()),
        )]));
    }
    if lines.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (no activity)",
            Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
        )]));
    }

    let widget = Paragraph::new(Text::from(lines))
        .block({
            let mut block = Block::default()
                .borders(Borders::ALL)
                .title(title);
            if app.cute != CuteMode::Off {
                block = block.border_style(Style::default().fg(c_sparkle()));
            }
            block
        })
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}

fn render_info_bar(f: &mut Frame<'_>, app: &App, area: Rect) {
    let auto_label = if app.auto_edits { "auto-edits: on" } else { "auto-edits: off" };

    let dir_display = if app.working_dir.len() > 30 {
        let truncated = &app.working_dir[app.working_dir.len().saturating_sub(28)..];
        format!("..{truncated}")
    } else {
        app.working_dir.clone()
    };

    let model_display = match (&app.last_provider, &app.last_model) {
        (Some(p), Some(m)) => format!("{p}:{m}"),
        (Some(p), None) => p.clone(),
        _ => "none".to_string(),
    };

    let spans = if app.cute == CuteMode::Off {
        vec![
            Span::raw(format!(
                " {auto_label}  |  dir: {dir_display}  |  model: {model_display}  |  {}", app.status
            )),
        ]
    } else {
        vec![
            Span::styled(" ♡ ", Style::default().fg(c_heart()).add_modifier(Modifier::BOLD)),
            Span::styled(auto_label, Style::default().fg(c_muted())),
            Span::styled("  |  ", Style::default().fg(c_muted()).add_modifier(Modifier::DIM)),
            Span::styled(format!("dir: {dir_display}"), Style::default().fg(c_muted())),
            Span::styled("  |  ", Style::default().fg(c_muted()).add_modifier(Modifier::DIM)),
            Span::styled(format!("model: {model_display}"), Style::default().fg(c_sparkle())),
            Span::styled("  |  ", Style::default().fg(c_muted()).add_modifier(Modifier::DIM)),
            Span::styled(app.status.clone(), Style::default().fg(c_muted())),
        ]
    };

    let bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::Rgb(30, 30, 40)).fg(Color::White));
    f.render_widget(bar, area);
}

fn render_info_panel(f: &mut Frame<'_>, app: &App, area: Rect) {
    let auto_label = if app.auto_edits { "auto-edits: on" } else { "auto-edits: off" };

    let ws_display = match (&app.selected_workspace_name, &app.selected_workspace_id) {
        (Some(name), _) if !name.trim().is_empty() => name.trim().to_string(),
        (_, Some(id)) if !id.trim().is_empty() => id.trim().to_string(),
        _ => "none".to_string(),
    };

    let last_model_display = match (&app.last_provider, &app.last_model) {
        (Some(p), Some(m)) => format!("{p}:{m}"),
        (Some(p), None) => p.clone(),
        _ => "none".to_string(),
    };

    let selected_display = format!(
        "{}{}",
        app.selected_provider,
        app.selected_model
            .as_deref()
            .map(|m| format!(":{m}"))
            .unwrap_or_default()
    );
    let show_last_used_model = app.selected_provider == "auto";

    let status_line = if app.cute == CuteMode::Off {
        Line::from(format!(" {}", app.status))
    } else {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "♡",
            Style::default().fg(c_heart()).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  "));
        spans.extend(lane_spans(app.lane));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(app.status.clone(), Style::default().fg(c_muted())));
        Line::from(spans)
    };

    let (line2_spans, line3_spans) = if app.cute == CuteMode::Off {
        let line3 = if show_last_used_model {
            format!(" selected: {selected_display}  |  last: {last_model_display} ")
        } else {
            format!(" selected: {selected_display} ")
        };
        (
            vec![Span::raw(format!(" {auto_label}  |  workspace: {ws_display} "))],
            vec![Span::raw(line3)],
        )
    } else {
        let line2_spans = vec![
            Span::styled(auto_label, Style::default().fg(c_muted())),
            Span::styled(
                "  |  ",
                Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
            ),
            Span::styled("workspace: ", Style::default().fg(c_muted())),
            Span::styled(ws_display, Style::default().fg(c_sparkle())),
        ];

        let mut line3_spans = vec![
            Span::raw("   "),
            Span::styled("selected: ", Style::default().fg(c_muted())),
            Span::styled(selected_display, Style::default().fg(c_sparkle())),
        ];
        if show_last_used_model {
            line3_spans.push(Span::styled(
                "  |  ",
                Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
            ));
            line3_spans.push(Span::styled("last: ", Style::default().fg(c_muted())));
            line3_spans.push(Span::styled(last_model_display, Style::default().fg(c_muted())));
        }
        (line2_spans, line3_spans)
    };

    let req = app.last_request_id.as_deref().unwrap_or("-");
    let elapsed = app
        .last_elapsed_ms
        .map(|v| format!("{v}ms"))
        .unwrap_or_else(|| "-".to_string());
    let usage_right = app
        .last_usage
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    let total_width = area.width.max(1) as usize;

    // Right column: req/elapsed above usage, both aligned to the right edge.
    let req_elapsed = format!("req: {req} | elapsed: {elapsed}");
    let req_elapsed_right = if app.cute == CuteMode::Off {
        vec![Span::raw(req_elapsed)]
    } else {
        vec![Span::styled(req_elapsed, Style::default().fg(c_muted()))]
    };

    let usage_right = if usage_right.is_empty() {
        Vec::new()
    } else if app.cute == CuteMode::Off {
        vec![Span::raw(usage_right.to_string())]
    } else {
        vec![Span::styled(
            usage_right.to_string(),
            Style::default().fg(c_muted()),
        )]
    };

    let line2 = compose_lr_line(total_width, &line2_spans, &req_elapsed_right);
    let line3 = compose_lr_line(total_width, &line3_spans, &usage_right);

    // Shift everything down one row.
    let panel = Paragraph::new(Text::from(vec![Line::from(""), status_line, line2, line3]))
        .style(Style::default().bg(Color::Rgb(30, 30, 40)).fg(Color::White))
        .wrap(Wrap { trim: false });
    f.render_widget(panel, area);
}

fn render_header(f: &mut Frame<'_>, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let base = Style::default().fg(Color::Black).bg(Color::White);
    let left = Paragraph::new(header_left_line(app)).style(base);

    let sel = format!(
        "{}{}",
        app.selected_provider,
        app.selected_model
            .as_deref()
            .map(|m| format!(":{m}"))
            .unwrap_or_default()
    );
    let token = if app.token_present {
        "token=yes"
    } else {
        "token=no"
    };
    let mut right_spans = provider_health_spans(app.cute, app.hints);
    right_spans.push(Span::raw(format!("  api={}  profile={}  {}  {sel} ", app.api_url, app.profile, token)));
    let right = Paragraph::new(Line::from(right_spans))
        .style(base)
        .alignment(Alignment::Right);

    f.render_widget(left, cols[0]);
    f.render_widget(right, cols[1]);
}

fn render_chat(app: &App, area: Rect) -> Paragraph<'static> {
    let width = area.width.saturating_sub(2).max(1) as usize;
    let height = area.height.saturating_sub(2).max(1) as usize;

    let all_lines = build_chat_lines(
        &app.messages,
        width,
        app.cute,
        app.spinner_step,
        app.spinner_last,
    );
    let total = all_lines.len();
    let max_scroll = total.saturating_sub(height);
    let scroll = app.scroll_from_bottom.min(max_scroll);
    let top = max_scroll.saturating_sub(scroll);
    let end = (top + height).min(total);

    let visible = &all_lines[top..end];
    let text = Text::from(visible.to_vec());

    let base_style = if app.cute == CuteMode::Off {
        Style::default()
    } else {
        // Match the logo "negative space" and the bottom panels.
        Style::default()
            .bg(Color::Rgb(30, 30, 40))
            .fg(Color::White)
    };

    Paragraph::new(text)
        .block(
            {
                let left_title = "Chat (PgUp/PgDn scroll, Ctrl+D debug)";
                let title_width = area.width.saturating_sub(2).max(1) as usize;
                let left_w = left_title.width();
                let label = "dir: ";
                // Add a little spacing so the right title doesn't crowd the left title.
                let max_dir_w = title_width.saturating_sub(left_w + label.width() + 4);
                let dir_display = tail_truncate_to_width(&app.working_dir, max_dir_w);

                let mut block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(left_title);

                // Put cwd on the top-right of the chat border.
                let dir_span = if app.cute == CuteMode::Off {
                    Span::raw(format!(" {label}{dir_display} "))
                } else {
                    Span::styled(
                        format!(" {label}{dir_display} "),
                        Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
                    )
                };
                block = block.title(
                    Title::from(Line::from(vec![dir_span]))
                        .alignment(Alignment::Right)
                        .position(Position::Top),
                );
                if app.cute != CuteMode::Off && (app.waiting || app.bg_tasks > 0) {
                    block = block.title(
                        Title::from(Line::from(border_blink_spans(
                            app.spinner_step,
                            area.width,
                        )))
                            .alignment(Alignment::Left)
                            .position(Position::Bottom),
                    );
                }
                if app.cute != CuteMode::Off {
                    block = block.border_style(Style::default().fg(c_sparkle()));
                }
                block
            },
        )
        .style(base_style)
        .wrap(Wrap { trim: false })
}

fn render_debug(app: &App) -> Paragraph<'static> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "Debug",
        Style::default().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(format!("waiting={}", app.waiting)));
    lines.push(Line::from(format!(
        "selected={}",
        format!(
            "{}{}",
            app.selected_provider,
            app.selected_model
                .as_deref()
                .map(|m| format!(":{m}"))
                .unwrap_or_default()
        )
    )));
    lines.push(Line::from(format!(
        "req_id={}",
        app.last_request_id.as_deref().unwrap_or("-")
    )));
    lines.push(Line::from(format!(
        "elapsed_ms={}",
        app.last_elapsed_ms
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string())
    )));
    lines.push(Line::from(""));
    lines.push(Line::from("Keys"));
    lines.push(Line::from("F2: models"));
    lines.push(Line::from("F3: workspaces"));
    lines.push(Line::from("F1: help"));
    lines.push(Line::from("Esc: quit"));
    lines.push(Line::from("Ctrl+R: reload"));

    Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title("Info"))
        .wrap(Wrap { trim: false })
}

fn render_input(app: &App) -> Paragraph<'static> {
    let input = app.input.iter().collect::<String>();

    let text = if app.cute == CuteMode::Off {
        // Show input with ghost text if available
        if app.show_completions && !app.completions.is_empty() {
            if let Some(selected_idx) = app.selected_completion {
                if let Some(selected_completion) = app.completions.get(selected_idx) {
                    let input_text = input.chars().collect::<String>();
                    let ghost_text = format!("{}{}", input_text, selected_completion.text);
                    Text::from(ghost_text)
                } else {
                    Text::from(input)
                }
            } else {
                Text::from(input)
            }
        } else {
            Text::from(input)
        }
    } else {
        let prefix = input_prompt_prefix(app.cute);
        let main_text = if app.show_completions && !app.completions.is_empty() {
            if let Some(selected_idx) = app.selected_completion {
                if let Some(selected_completion) = app.completions.get(selected_idx) {
                    let input_text = input.chars().collect::<String>();
                    let ghost_text = format!("{}{}", input_text, selected_completion.text);
                    Text::from(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(c_heart()).add_modifier(Modifier::BOLD)),
                        Span::raw(ghost_text),
                    ]))
                } else {
                    Text::from(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(c_heart()).add_modifier(Modifier::BOLD)),
                        Span::raw(input),
                    ]))
                }
            } else {
                Text::from(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(c_heart()).add_modifier(Modifier::BOLD)),
                    Span::raw(input),
                ]))
            }
        } else {
            Text::from(Line::from(vec![
                Span::styled(prefix, Style::default().fg(c_heart()).add_modifier(Modifier::BOLD)),
                Span::raw(input),
            ]))
        };
        main_text
    };

    // Show completion hint
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    if app.show_completions && !app.completions.is_empty() {
        let hint_style = if app.cute != CuteMode::Off {
            Style::default().fg(c_sparkle())
        } else {
            Style::default().fg(Color::Gray)
        };
        block = block.border_style(hint_style);
    } else if app.cute != CuteMode::Off {
        let c = if app.waiting { c_heart() } else { c_muted() };
        block = block.border_style(Style::default().fg(c));
    }

    Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
}

fn render_model_picker_popup(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Models (Up/Down, Enter select, Esc cancel)");
    if app.cute != CuteMode::Off {
        block = block.border_style(Style::default().fg(c_sparkle()));
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let list = render_model_picker(app);
    f.render_stateful_widget(list, rows[0], &mut app.model_state);

    let stargazing = render_stargazing_bar(app);
    f.render_widget(stargazing, rows[1]);
}

fn render_workspace_picker_popup(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Workspaces (Up/Down, Enter select, Esc cancel, Ctrl+R refresh)");
    if app.cute != CuteMode::Off {
        block = block.border_style(Style::default().fg(c_sparkle()));
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let list = render_workspace_picker(app);
    f.render_stateful_widget(list, rows[0], &mut app.workspace_state);

    let stargazing = render_stargazing_bar(app);
    f.render_widget(stargazing, rows[1]);
}

fn render_choice_modal_popup(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let title = app
        .choice_prompt
        .as_ref()
        .map(|p| p.title.as_str())
        .unwrap_or("Choose");
    let hint = app
        .choice_prompt
        .as_ref()
        .map(|p| p.hint.as_str())
        .unwrap_or("Tab/↑↓ select • Enter choose • Esc cancel");

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title);
    if app.cute != CuteMode::Off {
        block = block.border_style(Style::default().fg(c_sparkle()));
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    let list = render_choice_list(app);
    f.render_stateful_widget(list, rows[0], &mut app.choice_state);

    let hint_line = Paragraph::new(Line::from(vec![Span::styled(
        hint.to_string(),
        Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
    )]))
    .alignment(Alignment::Center);
    f.render_widget(hint_line, rows[1]);

    let stargazing = render_stargazing_bar(app);
    f.render_widget(stargazing, rows[2]);
}

fn render_text_prompt_popup(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let prompt = app
        .text_prompt
        .as_ref()
        .map(|p| p.prompt.as_str())
        .unwrap_or("Input");
    let input = app
        .text_prompt
        .as_ref()
        .map(|p| p.input.iter().collect::<String>())
        .unwrap_or_default();
    let cursor = app.text_prompt.as_ref().map(|p| p.cursor).unwrap_or(0);

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Input (Enter submit, Esc cancel)");
    if app.cute != CuteMode::Off {
        block = block.border_style(Style::default().fg(c_sparkle()));
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    let prefix = "> ";
    let lines = vec![
        Line::from(vec![Span::styled(
            prompt.to_string(),
            Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(prefix, Style::default().fg(c_sparkle()).add_modifier(Modifier::BOLD)),
            Span::raw(input.clone()),
        ]),
    ];

    let widget = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    f.render_widget(widget, inner);

    // Cursor on input line.
    let x = inner
        .x
        .saturating_add(prefix.width() as u16)
        .saturating_add(cursor as u16);
    let y = inner.y.saturating_add(2);
    if x < inner.x + inner.width && y < inner.y + inner.height {
        f.set_cursor_position((x, y));
    }
}

fn render_stargazing_bar(app: &App) -> Paragraph<'static> {
    if app.cute == CuteMode::Off {
        return Paragraph::new(Line::from(""));
    }

    // From SPECS/LOADINGSPECS.md.
    const SYMBOLS: &[&str] = &["✦", "✧", "✶", "✷", "✶", "✧"];
    let sym = SYMBOLS[(app.spinner_step as usize) % SYMBOLS.len()];
    let sym_style = match sym {
        "✶" | "✷" => Style::default().fg(c_heart()).add_modifier(Modifier::BOLD),
        _ => Style::default().fg(c_sparkle()).add_modifier(Modifier::BOLD),
    };

    let line = Line::from(vec![
        Span::styled(
            "stargazing… ",
            Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
        ),
        Span::styled(sym.to_string(), sym_style),
    ]);

    Paragraph::new(line).alignment(Alignment::Center)
}

fn render_model_picker(app: &App) -> List<'static> {
    let items = if app.model_options.is_empty() {
        vec![ListItem::new("No models loaded.")]
    } else {
        app.model_options
            .iter()
            .map(|m| {
                let id = format!(
                    "{}{}",
                    m.provider,
                    m.model
                        .as_deref()
                        .map(|v| format!(":{v}"))
                        .unwrap_or_default()
                );
                ListItem::new(Line::from(format!("{:<24} {}", id, m.label)))
            })
            .collect::<Vec<_>>()
    };

    List::new(items)
        .highlight_style(
            Style::default()
                .bg(c_sparkle())
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
}

fn render_workspace_picker(app: &App) -> List<'static> {
    let items = if app.workspace_options.is_empty() {
        vec![ListItem::new("No workspaces loaded.")]
    } else {
        app.workspace_options
            .iter()
            .map(|w| {
                let archived = if w.archived { " (archived)" } else { "" };
                let mut line = format!("{}{}  ({})", w.name, archived, w.id);
                if let Some(ref root) = w.root_path {
                    let root_disp = tail_truncate_to_width(root, 46);
                    line.push_str(&format!("  {root_disp}"));
                }
                if let Some(ref last) = w.last_used_at {
                    let last_disp = tail_truncate_to_width(last, 26);
                    line.push_str(&format!("  last={last_disp}"));
                }
                ListItem::new(Line::from(line))
            })
            .collect::<Vec<_>>()
    };

    List::new(items)
        .highlight_style(
            Style::default()
                .bg(c_sparkle())
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
}

fn render_thread_picker_popup(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Threads (Up/Down, Enter select, Esc cancel, Ctrl+N new)");
    if app.cute != CuteMode::Off {
        block = block.border_style(Style::default().fg(c_sparkle()));
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let list = render_thread_picker(app);
    f.render_stateful_widget(list, rows[0], &mut app.thread_state);

    let stargazing = render_stargazing_bar(app);
    f.render_widget(stargazing, rows[1]);
}

fn render_thread_picker(app: &App) -> List<'static> {
    let items = if app.thread_options.is_empty() {
        vec![ListItem::new("No threads loaded.")]
    } else {
        app.thread_options
            .iter()
            .map(|t| {
                let pin_indicator = if t.is_pinned { "📌 " } else { "" };
                let mode_str = t.mode.as_deref().unwrap_or("chat");
                let mut line = format!("{}{} [{}] ({} msgs)", pin_indicator, t.title, mode_str, t.message_count);
                if let Some(ref last) = t.last_message_at {
                    let last_disp = tail_truncate_to_width(last, 26);
                    line.push_str(&format!(" • {}", last_disp));
                }
                ListItem::new(Line::from(line))
            })
            .collect::<Vec<_>>()
    };

    List::new(items)
        .highlight_style(
            Style::default()
                .bg(c_sparkle())
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
}

fn render_memory_panel_popup(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let status = if app.memory_enabled { "ON" } else { "OFF" };
    let title = format!(
        "Memory {} • {} items (Ctrl+M toggle, Del delete, Esc close)",
        status,
        app.memory_items.len()
    );

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title);
    if app.cute != CuteMode::Off {
        block = block.border_style(Style::default().fg(c_sparkle()));
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2), Constraint::Length(1)])
        .split(inner);

    // Memory list
    let list = render_memory_list(app);
    f.render_stateful_widget(list, rows[0], &mut app.memory_state);

    // Settings info
    let settings_text = format!(
        "Settings: Max {} items, {} tokens context • Scope: {}{}",
        app.memory_settings.max_items_injected,
        app.memory_settings.max_context_tokens,
        if app.memory_settings.include_global { "global" } else { "" },
        if app.memory_settings.include_project { "+project" } else { "" }
    );
    let settings_para = Paragraph::new(Line::from(vec![Span::styled(
        settings_text,
        Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
    )]))
    .alignment(Alignment::Center);
    f.render_widget(settings_para, rows[1]);

    // Stargazing bar
    let stargazing = render_stargazing_bar(app);
    f.render_widget(stargazing, rows[2]);
}

fn render_memory_list(app: &App) -> List<'static> {
    let items = if app.memory_items.is_empty() {
        vec![ListItem::new("No memory items yet. Memory will be captured automatically.")]
    } else {
        app.memory_items
            .iter()
            .map(|m| {
                let enabled_indicator = if m.enabled { "✓" } else { "✗" };
                let type_badge = match m.item_type.as_str() {
                    "fact" => "📝",
                    "preference" => "⭐",
                    "decision" => "🎯",
                    "constraint" => "🔒",
                    "todo" => "☑",
                    _ => "•",
                };

                // Truncate content for display
                let content_disp = if m.content.len() > 80 {
                    format!("{}...", &m.content[..77])
                } else {
                    m.content.clone()
                };

                let line = format!(
                    "{} {} [{}] {} (salience: {:.2}, {})",
                    enabled_indicator,
                    type_badge,
                    m.scope,
                    content_disp,
                    m.salience,
                    m.source
                );

                ListItem::new(Line::from(line))
            })
            .collect::<Vec<_>>()
    };

    List::new(items)
        .highlight_style(
            Style::default()
                .bg(c_sparkle())
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
}

fn render_choice_list(app: &App) -> List<'static> {
    let items = if let Some(ref prompt) = app.choice_prompt {
        if prompt.options.is_empty() {
            vec![ListItem::new("No options.")]
        } else {
            prompt
                .options
                .iter()
                .map(|o| {
                    let mut lines = Vec::new();
                    lines.push(Line::from(vec![Span::styled(
                        o.label.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    )]));
                    if !o.description.trim().is_empty() {
                        lines.push(Line::from(vec![Span::styled(
                            o.description.clone(),
                            Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
                        )]));
                    }
                    ListItem::new(Text::from(lines))
                })
                .collect::<Vec<_>>()
        }
    } else {
        vec![ListItem::new("No active prompt.")]
    };

    List::new(items)
        .highlight_style(
            Style::default()
                .bg(c_sparkle())
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
}

fn border_blink_spans(step: u64, area_width: u16) -> Vec<Span<'static>> {
    // A small star cluster that travels right and "repairs" (re-draws) the border behind it.
    // No bullets/dots: empty space is the actual border line char.
    let usable = area_width.saturating_sub(2) as usize; // exclude corners
    if usable < 6 {
        return Vec::new();
    }

    const HEAD_W: usize = 5;
    let max_offset = usable.saturating_sub(HEAD_W);

    let grow_len = 5usize;
    let shrink_len = 4usize;
    // After reaching 5 stars, move the cluster right for ~15 steps (clamped to fit).
    let slide_len = 15usize.min(max_offset.saturating_sub(grow_len + shrink_len - 1));
    let total = grow_len + slide_len + shrink_len;
    if total == 0 {
        return Vec::new();
    }

    let idx = (step as usize) % total;

    let (offset, stars, right_align) = if idx < grow_len {
        // "Grow" diagonally: 1..5 stars.
        (idx, idx + 1, false)
    } else if idx < grow_len + slide_len {
        // Slide right with 5 stars.
        (idx, 5, false)
    } else {
        // Shrink: 4..1 stars, right-aligned inside the 5-cell head.
        let sidx = idx - (grow_len + slide_len);
        (idx, 4usize.saturating_sub(sidx), true)
    };

    let offset = offset.min(max_offset);

    let border_style = Style::default().fg(c_sparkle());
    let rainbow = |slot: usize| -> Color {
        match slot {
            0 => Color::Rgb(239, 68, 68),   // red
            1 => c_star(),                  // yellow
            2 => Color::Rgb(34, 197, 94),   // green
            3 => Color::Rgb(59, 130, 246),  // blue
            _ => c_brand(),                 // purple
        }
    };

    let mut spans: Vec<Span<'static>> = Vec::new();

    // Re-draw the border behind the moving head.
    if offset > 0 {
        spans.push(Span::styled("─".repeat(offset), border_style));
    }

    for i in 0..HEAD_W {
        let in_star = if stars == 0 {
            false
        } else if right_align {
            i >= HEAD_W.saturating_sub(stars)
        } else {
            i < stars
        };
        if in_star {
            spans.push(Span::styled(
                "★",
                Style::default()
                    .fg(rainbow(i))
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled("─", border_style));
        }
    }

    spans
}

fn render_help(_app: &App) -> Paragraph<'static> {
    let lines = vec![
        Line::from(vec![Span::styled(
            "Starbot TUI",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Enter: send prompt"),
        Line::from("Esc: quit (or close popup)"),
        Line::from("F2 / Ctrl+M: model picker"),
        Line::from("F3: workspace picker"),
        Line::from("Ctrl+R: reload"),
        Line::from("PgUp/PgDn: scroll chat"),
        Line::from("Ctrl+D: toggle debug panel"),
        Line::from(""),
        Line::from("Choice modal: Tab/↑↓ select, Enter choose, Esc cancel"),
        Line::from("Input modal: type, Enter submit, Esc cancel"),
        Line::from(""),
        Line::from(
            "If chat says missing token: run `starbott auth login` or use scripts/starbott-dev.sh.",
        ),
    ];

    Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Help (Esc to close)"),
        )
        .wrap(Wrap { trim: false })
}

fn render_tool_card(app: &App) -> Paragraph<'static> {
    let mut lines = Vec::new();

    if let Some(ref tool) = app.pending_tool {
        lines.push(Line::from(vec![Span::styled(
            format!("Tool: {}", tool.tool_name),
            Style::default().fg(c_sparkle()).add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));

        if !tool.target_files.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Files:",
                Style::default().add_modifier(Modifier::BOLD),
            )]));
            for file in &tool.target_files {
                lines.push(Line::from(format!("  {file}")));
            }
            lines.push(Line::from(""));
        }

        if !tool.preview.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Preview:",
                Style::default().add_modifier(Modifier::BOLD),
            )]));
            for preview_line in tool.preview.lines().take(15) {
                let style = if preview_line.starts_with('+') {
                    Style::default().fg(c_ok())
                } else if preview_line.starts_with('-') {
                    Style::default().fg(c_heart())
                } else {
                    Style::default().fg(c_muted())
                };
                lines.push(Line::from(vec![Span::styled(
                    format!("  {preview_line}"),
                    style,
                )]));
            }
            lines.push(Line::from(""));
        }

        if tool.requires_confirmation {
            lines.push(Line::from(vec![
                Span::styled("Y", Style::default().fg(c_ok()).add_modifier(Modifier::BOLD)),
                Span::raw(" Approve  "),
                Span::styled("N", Style::default().fg(c_heart()).add_modifier(Modifier::BOLD)),
                Span::raw(" Deny"),
            ]));
        }
    } else {
        lines.push(Line::from("No pending tool."));
    }

    Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Tool Approval (Y/N)"),
        )
        .wrap(Wrap { trim: false })
}

fn seed_rng() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    now.as_nanos() as u64 ^ 0x9E37_79B9_7F4A_7C15u64
}

fn rand_idx(rng: &mut u64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*rng >> 32) as usize) % len
}

fn pick_phrase(
    rng: &mut u64,
    last: &mut Option<&'static str>,
    pool: &'static [&'static str],
) -> &'static str {
    if pool.is_empty() {
        return "";
    }
    let mut idx = rand_idx(rng, pool.len());
    if let Some(prev) = last {
        if pool.len() > 1 && pool[idx] == *prev {
            idx = (idx + 1) % pool.len();
        }
    }
    let chosen = pool[idx];
    *last = Some(chosen);
    chosen
}

fn startup_status(cute: CuteMode) -> &'static str {
    match cute {
        CuteMode::On => "hi bestie ✨",
        CuteMode::Minimal => "Loading…",
        CuteMode::Off => "Loading models...",
    }
}

pub fn ready_status(cute: CuteMode, previous: &str) -> String {
    // Don't overwrite a more specific status (like an error).
    if previous.to_ascii_lowercase().contains("failed")
        || previous.to_ascii_lowercase().contains("error")
    {
        return previous.to_string();
    }
    match cute {
        CuteMode::On => "we're in, babe.".to_string(),
        CuteMode::Minimal => "Ready".to_string(),
        CuteMode::Off => "Ready.".to_string(),
    }
}

pub fn thinking_status(cute: CuteMode, rng: &mut u64, last: &mut Option<&'static str>) -> String {
    match cute {
        CuteMode::On => {
            const POOL: &[&str] = &[
                "stargazing... ✦",
                "consulting the constellations...",
                "picking the prettiest brain for this...",
            ];
            pick_phrase(rng, last, POOL).to_string()
        }
        CuteMode::Minimal => "Thinking...".to_string(),
        CuteMode::Off => "Sending...".to_string(),
    }
}

pub fn format_success_status(
    cute: CuteMode,
    rng: &mut u64,
    last: &mut Option<&'static str>,
    success_count: u32,
    lane: Option<Lane>,
) -> String {
    match cute {
        CuteMode::Off => "ok".to_string(),
        CuteMode::Minimal => "♡✓".to_string(),
        CuteMode::On => {
            let big_win = success_count == 1 || matches!(lane, Some(Lane::Deep));
            if big_win {
                // Keep this low frequency: first win, or deep lane.
                const POOL: &[&str] = &[
                    "verified. ♡ c'est bon.",
                    "sealed with a heart.",
                    "green flags only. ✓",
                ];
                let phrase = pick_phrase(rng, last, POOL);
                format!("♡✓ {phrase}")
            } else {
                "♡✓".to_string()
            }
        }
    }
}

pub fn format_error_status(
    cute: CuteMode,
    rng: &mut u64,
    last: &mut Option<&'static str>,
    err: &CliError,
) -> String {
    let (is_auth, is_scary) = match err {
        CliError::Auth(_) => (true, true),
        CliError::RateLimited(_) => (false, true),
        CliError::Server(_) => (false, true),
        _ => (false, false),
    };

    if is_auth {
        return "Authentication required. (No cute mode for auth errors.)".to_string();
    }

    let message = err.to_string();
    match cute {
        CuteMode::Off => format!("error: {message}"),
        CuteMode::Minimal => format!("♡! {message}"),
        CuteMode::On => {
            if is_scary {
                return format!("♡! {message}");
            }
            const POOL: &[&str] = &["oops - something's off.", "tiny detour, same destination."];
            let phrase = pick_phrase(rng, last, POOL);
            format!("♡! {phrase} {message}")
        }
    }
}

fn c_star() -> Color {
    Color::Rgb(255, 205, 86)
}

fn c_heart() -> Color {
    // Hot pink (closer to "actual pink" than red).
    Color::Rgb(255, 45, 149)
}

fn c_sparkle() -> Color {
    // Cyan.
    Color::Rgb(0, 255, 255)
}

fn c_brand() -> Color {
    Color::Rgb(131, 56, 236)
}

fn c_ok() -> Color {
    Color::Rgb(22, 163, 74)
}

fn c_warn() -> Color {
    Color::Rgb(245, 158, 11)
}

fn c_muted() -> Color {
    Color::Rgb(100, 116, 139)
}

// Base UI animation cadence. Keep stable; derive faster per-element animations
// from elapsed time rather than changing this (which would affect borders, etc.).
const SPINNER_INTERVAL_MS: u64 = 275;

fn update_spinner(app: &mut App) {
    if app.cute == CuteMode::Off {
        return;
    }
    if !app.waiting
        && app.bg_tasks == 0
        && !matches!(
            app.mode,
            Mode::ModelPicker | Mode::WorkspacePicker | Mode::ChoiceModal | Mode::TextPromptModal
        )
    {
        return;
    }
    // Keep this independent of the render loop cadence.
    // Speed: ~2x faster than the previous 550ms tick.
    let now = Instant::now();
    if now.duration_since(app.spinner_last) >= Duration::from_millis(SPINNER_INTERVAL_MS) {
        app.spinner_last = now;
        app.spinner_step = app.spinner_step.wrapping_add(1);
    }
}

fn header_left_line(app: &App) -> Line<'static> {
    match app.cute {
        CuteMode::Off => {
            let lane = lane_plain(app.lane);
            Line::from(vec![
                Span::raw(" Starbot  "),
                Span::styled(lane, Style::default().add_modifier(Modifier::DIM)),
                Span::raw(" "),
            ])
        }
        CuteMode::Minimal | CuteMode::On => {
            let mut spans: Vec<Span<'static>> = Vec::new();
            spans.push(Span::raw(" "));
            spans.extend(bespoke_star_spans());
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                "Starbot",
                Style::default()
                    .fg(c_brand())
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                "♡",
                Style::default().fg(c_heart()).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
            spans.extend(lane_spans(app.lane));
            spans.push(Span::raw(" "));
            Line::from(spans)
        }
    }
}

fn bespoke_star_spans() -> Vec<Span<'static>> {
    vec![
        Span::styled("˖", Style::default().fg(c_muted()).add_modifier(Modifier::DIM)),
        Span::styled("✦", Style::default().fg(c_sparkle()).add_modifier(Modifier::BOLD)),
        Span::styled("★", Style::default().fg(c_star()).add_modifier(Modifier::BOLD)),
        Span::styled("✦", Style::default().fg(c_sparkle()).add_modifier(Modifier::BOLD)),
        Span::styled("˖", Style::default().fg(c_muted()).add_modifier(Modifier::DIM)),
    ]
}

fn lane_plain(lane: Option<Lane>) -> String {
    match lane.unwrap_or(Lane::Standard) {
        Lane::Quick => "lane=quick".to_string(),
        Lane::Standard => "lane=standard".to_string(),
        Lane::Deep => "lane=deep".to_string(),
    }
}

fn lane_spans(lane: Option<Lane>) -> Vec<Span<'static>> {
    match lane.unwrap_or(Lane::Standard) {
        Lane::Quick => vec![Span::styled(
            "★",
            Style::default().fg(c_star()).add_modifier(Modifier::BOLD),
        )],
        Lane::Standard => vec![
            Span::styled("★", Style::default().fg(c_star()).add_modifier(Modifier::BOLD)),
            Span::styled("♡", Style::default().fg(c_heart()).add_modifier(Modifier::BOLD)),
        ],
        Lane::Deep => vec![
            Span::styled("★", Style::default().fg(c_star()).add_modifier(Modifier::BOLD)),
            // "Wings" for deep lane (SPEC5) with a little sparkle tint.
            Span::styled("︵", Style::default().fg(c_sparkle()).add_modifier(Modifier::BOLD)),
            Span::styled("♡", Style::default().fg(c_heart()).add_modifier(Modifier::BOLD)),
        ],
    }
}

fn provider_health_spans(cute: CuteMode, hints: ProviderHints) -> Vec<Span<'static>> {
    // Keep serious, short, and stable.
    let v = match hints.vertex_ok {
        Some(true) => ("OK", Some(true)),
        Some(false) => ("?", Some(false)),
        None => ("?", None),
    };
    let a = if hints.azure_present { ("OK", Some(true)) } else { ("?", None) };
    let cf = if hints.cf_present { ("OK", Some(true)) } else { ("?", None) };

    if cute == CuteMode::Off {
        return vec![Span::raw(format!(
            "Vertex:{} Azure:{} CF:{}",
            v.0, a.0, cf.0
        ))];
    }

    let status_style = |ok: Option<bool>| match ok {
        Some(true) => Style::default().fg(c_ok()).add_modifier(Modifier::BOLD),
        Some(false) => Style::default().fg(c_warn()).add_modifier(Modifier::BOLD),
        None => Style::default().fg(c_warn()).add_modifier(Modifier::BOLD),
    };

    vec![
        Span::styled("Vertex: ", Style::default().fg(c_muted())),
        Span::styled(v.0, status_style(v.1)),
        Span::raw("  "),
        Span::styled("Azure: ", Style::default().fg(c_muted())),
        Span::styled(a.0, status_style(a.1)),
        Span::raw("  "),
        Span::styled("CF: ", Style::default().fg(c_muted())),
        Span::styled(cf.0, status_style(cf.1)),
    ]
}

fn ping_pong_index(step: u64, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }
    let period = (len - 1) * 2;
    let pos = (step as usize) % period;
    if pos < len {
        pos
    } else {
        period - pos
    }
}

fn spinner_spans(cute: CuteMode, lane: Option<Lane>, step: u64) -> Option<Vec<Span<'static>>> {
    if cute == CuteMode::Off {
        return None;
    }

    // From SPECS/LOADINGSPECS.md (multi-char, ping-pong loop).
    // Keep frame width stable so the status line doesn't jitter.
    const MAIN: &[&str] = &["✦ ★", "✦★ ", " ★✦", "★ ✦"];

    let _ = lane;
    let idx = ping_pong_index(step, MAIN.len());
    let frame = MAIN[idx];

    let modif = match cute {
        CuteMode::On => Modifier::BOLD,
        CuteMode::Minimal => Modifier::BOLD,
        CuteMode::Off => Modifier::empty(),
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    for ch in frame.chars() {
        let style = match ch {
            '★' => Style::default().fg(c_heart()).add_modifier(modif),
            '✦' | '✧' => Style::default().fg(c_sparkle()).add_modifier(modif),
            _ => Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
        };
        if ch == ' ' {
            spans.push(Span::raw(" "));
        } else {
            spans.push(Span::styled(ch.to_string(), style));
        }
    }

    Some(spans)
}

fn input_title_line(app: &App) -> Line<'static> {
    match app.cute {
        CuteMode::Off => {
            let lane = lane_plain(app.lane);
            if app.waiting {
                return Line::from(format!("Input (waiting)  {lane}  {}", app.status));
            }
            Line::from(format!("Input  {lane}  {}", app.status))
        }
        CuteMode::Minimal | CuteMode::On => {
            let mut spans: Vec<Span<'static>> = Vec::new();
            spans.push(Span::styled(
                "Input",
                Style::default().add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));

            if app.waiting {
                if let Some(sp) = spinner_spans(app.cute, app.lane, app.spinner_step) {
                    spans.extend(sp);
                    spans.push(Span::raw("  "));
                }
            }

            spans.extend(lane_spans(app.lane));
            spans.push(Span::raw("  "));
            spans.push(Span::styled(app.status.clone(), Style::default().fg(c_muted())));
            Line::from(spans)
        }
    }
}

pub fn parse_lane(payload: &Value) -> Option<Lane> {
    payload
        .get("triage")
        .and_then(|v| v.get("lane"))
        .and_then(|v| v.as_str())
        .and_then(Lane::from_str)
}

pub fn parse_vertex_ok(payload: &Value) -> Option<bool> {
    let providers = payload.get("providers")?;
    let gemini = providers.get("gemini")?.as_str()?;
    match gemini {
        "available" => Some(true),
        "unavailable" => Some(false),
        _ => None,
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    let vertical = popup_layout[1];
    let popup_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical);

    popup_layout[1]
}

fn build_chat_lines(
    messages: &[ChatMsg],
    width: usize,
    cute: CuteMode,
    spinner_step: u64,
    spinner_last: Instant,
) -> Vec<Line<'static>> {
    let mut out = Vec::new();

    // Startup splash: STARBOT wordmark + mascot.
    out.extend(startup_banner_lines(width, cute));

    for msg in messages {
        let typing = msg.role == ChatRole::Assistant && !msg.sendable;
        let (prefix_spans, content_style) = if cute == CuteMode::Off {
            let (tag, tag_style, content_style) = match msg.role {
                ChatRole::User => (
                    "You",
                    Style::default().fg(c_ok()).add_modifier(Modifier::BOLD),
                    Style::default(),
                ),
                ChatRole::Assistant => (
                    "AI",
                    Style::default().fg(c_sparkle()).add_modifier(Modifier::BOLD),
                    Style::default(),
                ),
                ChatRole::System => (
                    "SYS",
                    Style::default().fg(c_warn()).add_modifier(Modifier::BOLD),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            };

            let prefix = format!("[{tag}] ");
            (vec![Span::styled(prefix, tag_style)], content_style)
        } else {
            match msg.role {
                ChatRole::User => (
                    vec![Span::styled(
                        "♡ ",
                        Style::default().fg(c_heart()).add_modifier(Modifier::BOLD),
                    )],
                    Style::default(),
                ),
                ChatRole::Assistant => {
                    (
                        assistant_prefix_spans(cute, spinner_step, typing),
                        if typing {
                            assistant_typing_style()
                        } else {
                            Style::default()
                        },
                    )
                }
                ChatRole::System => (
                    vec![Span::styled(
                        "💬 ",
                        Style::default().fg(c_muted()).add_modifier(Modifier::DIM),
                    )],
                    Style::default().add_modifier(Modifier::DIM),
                ),
            }
        };

        let prefix_len = spans_width(&prefix_spans);
        let content_raw = if typing {
            assistant_typing_content(spinner_step, spinner_last)
        } else {
            msg.content.as_str()
        };
        let content = content_raw.replace("\r\n", "\n");
        let indent = " ".repeat(prefix_len);
        let avail = width.saturating_sub(prefix_len).max(1);

        for (idx, line) in content.split('\n').enumerate() {
            let remaining = line.trim_end_matches('\r');
            if idx == 0 {
                if remaining.is_empty() {
                    out.push(Line::from(prefix_spans.clone()));
                    continue;
                }
                let wrapped = wrap_line(remaining, avail);
                for (widx, part) in wrapped.into_iter().enumerate() {
                    if widx == 0 {
                        let mut spans = Vec::new();
                        spans.extend(prefix_spans.clone());
                        spans.push(Span::styled(part, content_style));
                        out.push(Line::from(spans));
                    } else {
                        out.push(Line::from(vec![
                            Span::raw(indent.clone()),
                            Span::styled(part, content_style),
                        ]));
                    }
                }
            } else {
                if remaining.is_empty() {
                    out.push(Line::from(""));
                    continue;
                }
                let wrapped = wrap_line(remaining, avail);
                for part in wrapped {
                    out.push(Line::from(vec![
                        Span::raw(indent.clone()),
                        Span::styled(part, content_style),
                    ]));
                }
            }
        }

        // spacer line between messages
        out.push(Line::from(""));
    }
    out
}

fn startup_banner_lines(width: usize, cute: CuteMode) -> Vec<Line<'static>> {
    // From SPECS/SPEC12.md (mascot art). Keep as-is (Unicode); terminals/fonts that
    // support braille/block symbols will render it nicely.
    const MASCOT: &[&str] = &[
        "⠀⠀⠀⢀⣠⣤⣤⣤⣀⠀⠀⠀⠀⣀⣠⣤⣤⣤⣄⡀⠀⠀⠀⠀⠀",
        "⠀⠀⣠⣿⠿⠛⠛⠛⠛⠛⢿⣷⣤⣾⠿⠛⠛⠙⠛⠛⠿⠗⠀⠀⠀⠀",
        "⠀⣾⡿⠁⠀⠀⠀⠀⠀⠀⠀⠙⡿⠁⠀⢀⣤⣀⠀⠀⢀⣤⣶⡆⠀⠀",
        "⢸⣿⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⣿⣿⣿⣿⣿⣿⣿⡇⠀⠀",
        "⠸⣿⡆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣸⣿⣿⣿⣿⣿⣿⣧⣄⠀",
        "⠀⢹⣿⠀⣿⣷⣄⣀⣤⡄⠀⠀⠀⠀⢀⣴⣿⣿⣿⣿⣿⣿⣿⣿⣿⠷",
        "⠀⠀⣁⣤⣿⣿⣿⣿⣿⠃⠀⠀⠀⠀⠘⠛⠛⠛⠻⣿⣿⣿⠋⠉⠀⠀",
        "⠀⠘⠻⢿⣿⣿⣿⣿⣿⡄⠀⠀⠀⠀⠀⠀⠀⢀⡀⠹⣿⡟⠀⠀⠀⠀",
        "⠀⠀⠀⠀⢹⣿⠟⢙⠛⠛⠀⠀⠀⠀⠀⣀⣴⡿⠓⠀⠀⠀⠀⠀⠀⠀",
        "⠀⠀⠀⠀⠈⠁⠀⠈⠻⢿⣦⣄⠀⣠⣾⡿⠋⠀⠀⠀⠀⠀⠀⠀⠀⠀",
        "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠙⢿⣿⠿⠋⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
        "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    ];

    // Wordmark (requested). Keep as-is to preserve the look.
    const LOGO: &[&str] = &[
        "★   *      .       .   *   .   *",
        "   .     *      .    ★     .      .",
        "        ███████╗████████╗ █████╗ ██████╗ ██████╗  ██████╗ ████████╗",
        "        ██╔════╝╚══██╔══╝██╔══██╗██╔══██╗██╔══██╗██╔═══██╗╚══██╔══╝",
        "        ███████╗   ██║   ███████║██████╔╝██████╔╝██║   ██║   ██║",
        "        ╚════██║   ██║   ██╔══██║██╔══██╗██╔══██╗██║   ██║   ██║",
        "  ★     ███████║   ██║   ██║  ██║██║  ██║██████╔╝╚██████╔╝   ██║     ★",
        "        ╚══════╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝  ╚═════╝    ╚═╝",
        "    .    *    .   *      .   *   .   *",
    ];

    let mut out = Vec::new();

    let logo_w = LOGO
        .iter()
        .map(|l| l.trim_end().width())
        .max()
        .unwrap_or(0);
    let mascot_w = MASCOT.iter().map(|l| l.width()).max().unwrap_or(0);
    let min_gap = if logo_w == 0 || mascot_w == 0 { 0 } else { 3 };
    let combined_w = logo_w + min_gap + mascot_w;

    let logo_style_for_row = |row: usize| -> Style {
        if cute == CuteMode::Off {
            return Style::default();
        }
        // Starfield lines + the "ambient" row.
        if row <= 1 || row + 1 == LOGO.len() {
            return Style::default().fg(c_muted()).add_modifier(Modifier::DIM);
        }
        // Big block wordmark.
        Style::default().fg(c_sparkle()).add_modifier(Modifier::BOLD)
    };

    let mascot_style = if cute == CuteMode::Off {
        Style::default()
    } else {
        Style::default().fg(c_muted()).add_modifier(Modifier::DIM)
    };

    // Prefer side-by-side (logo left, mascot right). If the terminal is too narrow,
    // fall back to stacked so the art doesn't wrap.
    if width < combined_w {
        // ── Stacked ──
        let logo_pad = width.saturating_sub(logo_w) / 2;
        for (row, line) in LOGO.iter().enumerate() {
            out.push(Line::from(vec![
                Span::raw(" ".repeat(logo_pad)),
                Span::styled((*line).to_string(), logo_style_for_row(row)),
            ]));
        }
        out.push(Line::from(""));
        let mascot_pad = width.saturating_sub(mascot_w) / 2;
        for line in MASCOT {
            out.push(Line::from(vec![
                Span::raw(" ".repeat(mascot_pad)),
                Span::styled((*line).to_string(), mascot_style),
            ]));
        }
        out.push(Line::from(""));
        return out;
    }

    // ── Side-by-side ──
    let total_h = LOGO.len().max(MASCOT.len());

    for row in 0..total_h {
        let logo_line = LOGO.get(row).copied().unwrap_or("").trim_end();
        let logo_w_row = logo_line.width();
        let mascot_line = MASCOT.get(row).copied().unwrap_or("");

        // Logo is left-aligned; mascot is right-aligned.
        // Insert spaces so the mascot's max-width block ends at the right edge.
        let space_between = width
            .saturating_sub(mascot_w)
            .saturating_sub(logo_w_row);

        out.push(Line::from(vec![
            Span::styled(logo_line.to_string(), logo_style_for_row(row)),
            Span::raw(" ".repeat(space_between)),
            Span::styled(mascot_line.to_string(), mascot_style),
        ]));
    }

    out.push(Line::from(""));
    out
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if width <= 1 {
        return vec![line.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();

    for word in line.split_whitespace() {
        if cur.is_empty() {
            cur.push_str(word);
            continue;
        }
        if cur.as_str().width() + 1 + word.width() <= width {
            cur.push(' ');
            cur.push_str(word);
        } else {
            out.push(cur);
            cur = word.to_string();
        }
    }

    if !cur.is_empty() {
        out.push(cur);
    }

    if out.is_empty() {
        out.push(String::new());
    }

    out
}

fn truncate_to_width(input: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if input.width() <= max_width {
        return input.to_string();
    }

    const ELLIPSIS: &str = "…";
    let ell_w = ELLIPSIS.width();
    if max_width <= ell_w {
        return ELLIPSIS.to_string();
    }

    let mut out = String::new();
    let mut w = 0usize;
    for ch in input.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw + ell_w > max_width {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push_str(ELLIPSIS);
    out
}

fn tail_truncate_to_width(input: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if input.width() <= max_width {
        return input.to_string();
    }

    const ELLIPSIS: &str = "…";
    if max_width == 1 {
        return ELLIPSIS.to_string();
    }
    if max_width == 2 {
        return "..".to_string();
    }

    let keep_w = max_width.saturating_sub(2);
    let mut tail_rev = String::new();
    let mut w = 0usize;
    for ch in input.chars().rev() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > keep_w {
            break;
        }
        tail_rev.push(ch);
        w += cw;
    }
    let tail: String = tail_rev.chars().rev().collect();
    format!("..{tail}")
}

fn input_prompt_len(app: &App) -> usize {
    input_prompt_prefix(app.cute).width()
}

fn spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|s| s.content.as_ref().width()).sum()
}

fn truncate_spans_to_width(spans: &[Span<'static>], max_width: usize) -> Vec<Span<'static>> {
    if max_width == 0 {
        return Vec::new();
    }
    if spans_width(spans) <= max_width {
        return spans.to_vec();
    }

    const ELLIPSIS: &str = "…";
    let ell_w = ELLIPSIS.width();
    if max_width <= ell_w {
        return vec![Span::raw(ELLIPSIS)];
    }

    let mut out: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    let limit = max_width.saturating_sub(ell_w);

    for sp in spans {
        let style = sp.style;
        let text = sp.content.as_ref();
        if text.is_empty() {
            continue;
        }

        let mut chunk = String::new();
        for ch in text.chars() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            if used + cw > limit {
                break;
            }
            chunk.push(ch);
            used += cw;
        }

        if !chunk.is_empty() {
            out.push(Span::styled(chunk, style));
        }

        if used >= limit {
            break;
        }
    }

    out.push(Span::raw(ELLIPSIS));
    out
}

fn compose_lr_line(
    total_width: usize,
    left: &[Span<'static>],
    right: &[Span<'static>],
) -> Line<'static> {
    if total_width == 0 {
        return Line::from("");
    }

    let right_w = spans_width(right);
    if right_w == 0 {
        return Line::from(truncate_spans_to_width(left, total_width));
    }
    if right_w >= total_width {
        return Line::from(truncate_spans_to_width(right, total_width));
    }

    let max_left = total_width.saturating_sub(right_w).saturating_sub(1);
    let left_trunc = truncate_spans_to_width(left, max_left);
    let left_w = spans_width(&left_trunc);
    let spaces = total_width.saturating_sub(left_w + right_w);

    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.extend(left_trunc);
    if spaces > 0 {
        spans.push(Span::raw(" ".repeat(spaces)));
    }
    spans.extend(right.iter().cloned());
    Line::from(spans)
}

fn input_prompt_prefix(cute: CuteMode) -> &'static str {
    if cute == CuteMode::Off {
        ""
    } else {
        "♡ "
    }
}

fn assistant_prefix_spans(cute: CuteMode, step: u64, typing: bool) -> Vec<Span<'static>> {
    let _ = (cute, step, typing);
    // Keep prefix width stable but remove the old star+sparkle glyphs.
    let star = Span::styled("⭐", Style::default().fg(c_star()).add_modifier(Modifier::BOLD));
    vec![Span::raw(" "), star, Span::raw(" ")]
}

fn assistant_typing_content(spinner_step: u64, spinner_last: Instant) -> &'static str {
    // Single-cell glyph cycle (loop).
    //
    // Note: don't reuse `spinner_step` directly (275ms cadence). We derive a
    // faster tick from "approx elapsed time" so this animates smoothly without
    // speeding up other UI animations (border, etc.).
    const FRAMES: &[&str] = &["⣤", "⣰", "⢸", "⠹", "⠛", "⠏", "⡇", "⣆"];
    const TYPING_INTERVAL_MS: u128 = 60;

    let now = Instant::now();
    let elapsed_ms = (spinner_step as u128) * (SPINNER_INTERVAL_MS as u128)
        + now.duration_since(spinner_last).as_millis();
    let idx = ((elapsed_ms / TYPING_INTERVAL_MS) % (FRAMES.len() as u128)) as usize;
    FRAMES[idx]
}

fn assistant_typing_style() -> Style {
    // Single solid color (no gradient).
    Style::default()
        .fg(c_muted())
        .add_modifier(Modifier::BOLD)
}

pub fn remove_typing_placeholder(app: &mut App) {
    if let Some(idx) = app
        .messages
        .iter()
        .rposition(|m| m.role == ChatRole::Assistant && !m.sendable)
    {
        app.messages.remove(idx);
    }
}




fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = (bytes as f64) / 1024.0;
    if kb < 1024.0 {
        return format!("{:.1} KB", kb);
    }
    let mb = kb / 1024.0;
    format!("{:.1} MB", mb)
}

pub fn format_dir_listing_for_user(result: &Value) -> Option<String> {
    let path = result
        .get("path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(".");
    let entries = result.get("entries")?.as_array()?;
    let truncated = result.get("truncated").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut dirs: Vec<(String, Option<u64>)> = Vec::new();
    let mut files: Vec<(String, Option<u64>)> = Vec::new();
    let mut other: Vec<(String, Option<u64>)> = Vec::new();

    for e in entries {
        let name = e
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let typ = e
            .get("type")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("other");
        let bytes = e.get("bytes").and_then(|v| v.as_u64());

        match typ {
            "dir" => dirs.push((name.to_string(), bytes)),
            "file" => files.push((name.to_string(), bytes)),
            _ => other.push((name.to_string(), bytes)),
        }
    }

    let mut out: Vec<String> = Vec::new();
    out.push("Auto tool: file.dir".to_string());
    out.push(String::new());
    out.push(format!(
        "Directory listing for `{}` ({} entries{}):",
        path,
        entries.len(),
        if truncated { ", truncated" } else { "" }
    ));

    let mut show_group = |label: &str, arr: &[(String, Option<u64>)], is_dir: bool| {
        if arr.is_empty() {
            return;
        }
        out.push(String::new());
        out.push(format!("{label}:"));
        for (name, bytes) in arr.iter().take(50) {
            let mut line = String::new();
            line.push_str("- ");
            line.push_str(name);
            if is_dir && !name.ends_with('/') {
                line.push('/');
            }
            if let Some(b) = bytes {
                line.push_str(&format!(" ({})", format_bytes(*b)));
            }
            out.push(line);
        }
        if arr.len() > 50 {
            out.push("- ...".to_string());
        }
    };

    show_group("Folders", &dirs, true);
    show_group("Files", &files, false);
    show_group("Other", &other, false);

    out.push(String::new());
    out.push(format!("Truncated: {}.", if truncated { "yes" } else { "no" }));

    Some(out.join("\n"))
}

fn truncate_chars(input: &str, max_chars: usize) -> (String, bool) {
    if max_chars == 0 {
        return (String::new(), input.chars().next().is_some());
    }
    let mut out = String::new();
    let mut count = 0usize;
    for ch in input.chars() {
        if count >= max_chars {
            return (out, true);
        }
        out.push(ch);
        count += 1;
    }
    (out, false)
}

pub fn format_file_read_for_user(result: &Value) -> Option<String> {
    let path = result
        .get("path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("-");
    let detected = result
        .get("detectedType")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("text");
    let truncated = result.get("truncated").and_then(|v| v.as_bool()).unwrap_or(false);
    let total_bytes = result.get("totalBytes").and_then(|v| v.as_u64()).unwrap_or(0);
    let line_start = result.get("lineStart").and_then(|v| v.as_u64()).unwrap_or(1);
    let line_end = result.get("lineEnd").and_then(|v| v.as_u64()).unwrap_or(1);
    let content = result.get("content")?.as_str().unwrap_or("");

    let (snippet, clipped) = truncate_chars(content, 8000);

    let mut out: Vec<String> = Vec::new();
    out.push("Auto tool: file.read".to_string());
    out.push(String::new());
    out.push(format!(
        "File: `{}` (type={}, lines {}-{}, bytes={}, truncated={})",
        path,
        detected,
        line_start,
        line_end,
        total_bytes,
        if truncated { "yes" } else { "no" }
    ));
    out.push(String::new());
    out.push(format!("```{detected}"));
    out.push(snippet);
    if clipped {
        out.push("\n…(truncated)".to_string());
    }
    out.push("```".to_string());

    Some(out.join("\n"))
}

pub fn format_tool_propose_result(tool_name: &str, payload: &Value) -> String {
    let requires_confirmation = payload
        .get("requiresConfirmation")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if requires_confirmation {
        let preview = payload.get("preview").cloned().unwrap_or_else(|| json!({}));
        let preview_text =
            serde_json::to_string_pretty(&preview).unwrap_or_else(|_| preview.to_string());
        let (snippet, clipped) = truncate_chars(&preview_text, 8000);
        let mut out = vec![
            format!("Tool proposal (requires confirmation): {tool_name}"),
            String::new(),
            snippet,
        ];
        if clipped {
            out.push("…(truncated)".to_string());
        }
        out.push(String::new());
        out.push("This tool requires confirmation; choices only support safe tools.".to_string());
        return out.join("\n");
    }

    let run_id = payload
        .get("runId")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("-");
    let result = payload.get("result").cloned().unwrap_or_else(|| json!({}));

    let mut out: Vec<String> = Vec::new();
    out.push(format!("Tool result: {tool_name} (runId: {run_id})"));
    out.push(String::new());

    if tool_name == "file.dir" {
        if let Some(listing) = format_dir_listing_for_user(&result) {
            out.push(listing);
        } else {
            out.push(result.to_string());
        }
        return out.join("\n");
    }

    if tool_name == "file.read" {
        if let Some(text) = format_file_read_for_user(&result) {
            out.push(text);
        } else {
            out.push(result.to_string());
        }
        return out.join("\n");
    }

    let text = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
    let (snippet, clipped) = truncate_chars(&text, 8000);
    out.push(snippet);
    if clipped {
        out.push("…(truncated)".to_string());
    }
    out.join("\n")
}


fn render_file_browser_popup(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!("File Browser: {}", app.file_browser_path));

    if app.cute != CuteMode::Off {
        block = block.border_style(Style::default().fg(c_sparkle()));
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    // List files
    let items: Vec<ListItem> = app.file_browser_files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let mut line = if file.is_dir {
                format!("📁 {}", file.name)
            } else {
                format!("📄 {}", file.name)
            };

            // Show size for files
            if !file.is_dir && file.size.is_some() {
                let size = file.size.unwrap();
                let size_str = if size < 1024 {
                    format!(" ({})", size)
                } else if size < 1024 * 1024 {
                    format!(" ({:.1}K)", size as f64 / 1024.0)
                } else {
                    format!(" ({:.1}M)", size as f64 / (1024.0 * 1024.0))
                };
                line.push_str(&size_str);
            }

            // Show date if available
            if file.last_modified.is_some() {
                line.push_str(&format!("  {}", file.last_modified.as_ref().unwrap()));
            }

            let style = if Some(file.path.clone()) == app.file_browser_selected {
                Style::default().fg(c_heart()).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::styled(line, style)
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("➤ ");

    f.render_stateful_widget(list, inner, &mut app.file_browser_state);

    // Instructions
    let help_text = "↑/↓: Navigate | Enter: Open | Backspace: Back | Space: Select | Esc: Close";
    let help = Paragraph::new(Line::from(vec![
        Span::styled(help_text, Style::default().fg(Color::Gray))
    ]))
    .alignment(Alignment::Center);

    let help_area = Rect {
        x: area.x,
        y: area.bottom() - 1,
        width: area.width,
        height: 1,
    };
    f.render_widget(help, help_area);
}
