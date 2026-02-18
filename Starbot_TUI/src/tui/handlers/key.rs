// PHASE 3: TUI key handler extracted from tui.rs
// Contains handle_event and handle_key functions for keyboard input

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::widgets::ListState;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::config::{profile_mut, save_config};

// Helper functions
fn parent_directory(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let components: Vec<&str> = path.split('/').collect();
    if components.len() > 1 {
        Some(components[..components.len() - 1].join("/"))
    } else {
        Some(".".to_string())
    }
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} KB");
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{mb:.1} MB");
    }
    let gb = mb / 1024.0;
    format!("{gb:.1} GB")
}

fn parse_local_list_target(prompt: &str) -> Option<String> {
    let trimmed = prompt.trim();
    let trimmed = trimmed.trim_end_matches(['?', '!', '.']);

    if let Some(rest) = trimmed.strip_prefix("/ls") {
        let target = rest.trim();
        if !target.is_empty() {
            return Some(target.to_string());
        }
        return None;
    }

    if trimmed == "ls" {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("ls ") {
        let target = rest.trim();
        if !target.is_empty() {
            return Some(target.to_string());
        }
    }

    let lower = trimmed.to_ascii_lowercase();
    let natural_prefixes = [
        "what's in the ",
        "whats in the ",
        "what is in the ",
        "what's in ",
        "whats in ",
        "what is in ",
        "show me ",
        "list ",
        "look in ",
        "look inside ",
    ];
    let natural_suffixes = [
        " folder",
        " directory",
        " dir",
        " files",
        " file",
        " contents",
        " content",
    ];

    for prefix in natural_prefixes {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let mut candidate = rest.trim().to_string();
            if let Some(after_the) = candidate.strip_prefix("the ") {
                candidate = after_the.trim().to_string();
            }
            for suffix in natural_suffixes {
                if let Some(base) = candidate.strip_suffix(suffix) {
                    candidate = base.trim().to_string();
                    break;
                }
            }
            candidate = candidate
                .trim_matches('`')
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string();
            if !candidate.is_empty()
                && candidate != "here"
                && candidate != "in here"
                && candidate != "this directory"
                && candidate != "this dir"
            {
                return Some(candidate);
            }
        }
    }

    None
}

fn is_local_pwd_request(prompt: &str) -> bool {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Prefer listing behavior when a prompt can be interpreted as directory contents.
    if is_local_list_request(trimmed) {
        return false;
    }

    if trimmed == "pwd" || trimmed == "/pwd" {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();
    [
        "what directory are we in",
        "what dir are we in",
        "what folder are we in",
        "where are we",
        "where am i",
        "current directory",
        "current dir",
        "working directory",
        "show current directory",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_local_list_request(prompt: &str) -> bool {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed == "ls" || trimmed.starts_with("ls ") || trimmed.starts_with("/ls") {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();
    [
        "list directory",
        "directory contents",
        "show directory",
        "show dir",
        "list dir",
        "this dir",
        "in this dir",
        "what's in this dir",
        "whats in this dir",
        "what is in this dir",
        "can we even look at files in here",
        "can we look at files in here",
        "can you access files",
        "do you have access to files",
        "can you see files",
        "look at files",
        "look at the files",
        "look at files in here",
        "look at the files in here",
        "look in there",
        "look in here",
        "show current directory",
        "list current directory",
        "files in here",
        "files here",
        "what files are here",
        "list files in this directory",
        "show files in this directory",
        "show contents",
        "show the contents",
        "directory content",
        "what is in this directory",
        "what's in this directory",
        "whats in this directory",
        "what are the contents",
        "what's the contents",
        "what is the contents",
        "tell me what the contents are",
        "contents of our working directory",
        "contents of the working directory",
        "contents of current directory",
        "current folder contents",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
        || ((lower.contains("what's in")
            || lower.contains("whats in")
            || lower.contains("what is in")
            || lower.contains("look in")
            || lower.contains("look inside")
            || lower.contains("show me")
            || lower.contains("list"))
            && (lower.contains("folder")
                || lower.contains("directory")
                || lower.contains(" dir")
                || lower.ends_with("dir")))
        || (lower.contains("contents")
            && (lower.contains("working directory")
                || lower.contains("current directory")
                || lower.contains("our working directory")
                || lower.contains("our directory")
                || lower.contains("folder")
                || lower.contains("directory")
                || lower.contains(" dir")
                || lower.contains("here")
                || lower.contains("there")
                || lower.contains("current")
                || lower.contains("our")))
        || ((lower.contains("files") || lower.contains("directory") || lower.contains("folder"))
            && (lower.contains("here")
                || lower.contains("in here")
                || lower.contains("this directory"))
            && (lower.contains("look")
                || lower.contains("access")
                || lower.contains("list")
                || lower.contains("show")
                || lower.contains("what")))
        || ((lower.contains("dir") || lower.contains("directory") || lower.contains("folder"))
            && (lower.contains("here")
                || lower.contains("in here")
                || lower.contains("this dir")
                || lower.contains("this directory"))
            && (lower.contains("look")
                || lower.contains("access")
                || lower.contains("list")
                || lower.contains("show")
                || lower.contains("what")
                || lower.contains("what's")
                || lower.contains("whats")))
        || (lower.starts_with("of ")
            && (lower.contains("working directory")
                || lower.contains("current directory")
                || lower.contains("our directory")
                || lower.contains("this directory")
                || lower.contains("this dir")))
}

fn is_local_access_request(prompt: &str) -> bool {
    let lower = prompt.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    (lower.contains("access")
        || lower.contains("see")
        || lower.contains("read")
        || lower.contains("view"))
        && (lower.contains("file")
            || lower.contains("files")
            || lower.contains("directory")
            || lower.contains("folder")
            || lower.contains("filesystem"))
}

fn render_local_dir_listing(working_dir: &str, prompt: &str) -> Option<String> {
    if !is_local_list_request(prompt) {
        return None;
    }

    let target = parse_local_list_target(prompt);
    let target_path = match target {
        Some(raw) => {
            let candidate = PathBuf::from(raw);
            if candidate.is_absolute() {
                candidate
            } else {
                PathBuf::from(working_dir).join(candidate)
            }
        }
        None => PathBuf::from(working_dir),
    };

    let entries = match fs::read_dir(&target_path) {
        Ok(v) => v,
        Err(err) => {
            return Some(format!(
                "Local directory listing failed for `{}`: {}",
                target_path.display(),
                err
            ));
        }
    };

    let mut dirs: Vec<(String, Option<u64>)> = Vec::new();
    let mut files: Vec<(String, Option<u64>)> = Vec::new();
    let mut other: Vec<(String, Option<u64>)> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.is_empty() {
            continue;
        }

        let file_type = entry.file_type().ok();
        let bytes = fs::metadata(entry.path()).ok().map(|m| m.len());

        if file_type.map(|t| t.is_dir()).unwrap_or(false) {
            dirs.push((name, bytes));
        } else if file_type.map(|t| t.is_file()).unwrap_or(false) {
            files.push((name, bytes));
        } else {
            other.push((name, bytes));
        }
    }

    dirs.sort_by_key(|(name, _)| name.to_ascii_lowercase());
    files.sort_by_key(|(name, _)| name.to_ascii_lowercase());
    other.sort_by_key(|(name, _)| name.to_ascii_lowercase());

    const MAX_PER_GROUP: usize = 80;
    let truncated =
        dirs.len() > MAX_PER_GROUP || files.len() > MAX_PER_GROUP || other.len() > MAX_PER_GROUP;

    let mut out = Vec::new();
    let total = dirs.len() + files.len() + other.len();
    out.push(format!(
        "Local directory listing for `{}` ({} entries{}):",
        target_path.display(),
        total,
        if truncated { ", truncated" } else { "" }
    ));

    let mut push_group = |label: &str, group: &[(String, Option<u64>)], is_dir: bool| {
        if group.is_empty() {
            return;
        }
        out.push(String::new());
        out.push(format!("{label}:"));
        for (name, size) in group.iter().take(MAX_PER_GROUP) {
            let suffix = if is_dir && !name.ends_with('/') {
                "/"
            } else {
                ""
            };
            match size {
                Some(bytes) => out.push(format!("- {name}{suffix} ({})", format_file_size(*bytes))),
                None => out.push(format!("- {name}{suffix}")),
            }
        }
        if group.len() > MAX_PER_GROUP {
            out.push("- ...".to_string());
        }
    };

    push_group("Folders", &dirs, true);
    push_group("Files", &files, false);
    push_group("Other", &other, false);

    Some(out.join("\n"))
}

fn handle_local_pwd_prompt(app: &mut App, prompt: &str) -> bool {
    if !is_local_pwd_request(prompt) {
        return false;
    }

    app.messages.push(ChatMsg {
        role: ChatRole::User,
        content: prompt.to_string(),
        sendable: true,
    });
    app.messages.push(ChatMsg {
        role: ChatRole::Assistant,
        content: format!("Current working directory:\n`{}`", app.working_dir),
        sendable: true,
    });
    app.status = "Reported local working directory.".to_string();
    app.input.clear();
    app.cursor = 0;
    true
}

fn handle_local_dir_prompt(app: &mut App, prompt: &str) -> bool {
    let Some(listing) = render_local_dir_listing(&app.working_dir, prompt) else {
        return false;
    };

    app.messages.push(ChatMsg {
        role: ChatRole::User,
        content: prompt.to_string(),
        sendable: true,
    });
    app.messages.push(ChatMsg {
        role: ChatRole::Assistant,
        content: listing,
        sendable: true,
    });
    app.status = "Listed local directory.".to_string();
    app.input.clear();
    app.cursor = 0;
    true
}

fn handle_local_access_prompt(app: &mut App, prompt: &str) -> bool {
    if !is_local_access_request(prompt) {
        return false;
    }

    let mut content = format!(
        "Yes. I have local access to files in:\n`{}`",
        app.working_dir
    );
    if let Some(listing) = render_local_dir_listing(&app.working_dir, "ls") {
        content.push_str("\n\n");
        content.push_str(&listing);
    }

    app.messages.push(ChatMsg {
        role: ChatRole::User,
        content: prompt.to_string(),
        sendable: true,
    });
    app.messages.push(ChatMsg {
        role: ChatRole::Assistant,
        content,
        sendable: true,
    });
    app.status = "Confirmed local file access.".to_string();
    app.input.clear();
    app.cursor = 0;
    true
}
use crate::cute::CuteMode;
use crate::errors::CliError;
use crate::tui::types::{
    App, ChatMsg, ChatRole, ChoiceAction, Mode, TextPromptState, ToolApprovalEntry, TuiMsg,
};

use super::async_ops::{
    spawn_chat_request_stream_legacy, spawn_completion_request, spawn_file_list_fetch,
    spawn_health_fetch, spawn_memory_fetch, spawn_memory_settings_fetch, spawn_memory_toggle,
    spawn_models_fetch, spawn_threads_fetch, spawn_tool_propose, spawn_workspaces_fetch,
};

// Import helper functions from parent tui module
use crate::commands::tui::thinking_status;

pub fn handle_event(
    api: &ApiClient,
    tx: &mpsc::UnboundedSender<TuiMsg>,
    app: &mut App,
    event: Event,
) -> Result<(), CliError> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            handle_key(api, tx, app, key)?;
        }
        _ => {}
    }
    Ok(())
}

pub fn handle_key(
    api: &ApiClient,
    tx: &mpsc::UnboundedSender<TuiMsg>,
    app: &mut App,
    key: crossterm::event::KeyEvent,
) -> Result<(), CliError> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // Global model picker shortcut across all modes.
    if key.code == KeyCode::F(2) {
        app.choice_prompt = None;
        app.text_prompt = None;
        app.pending_tool = None;
        app.mode = Mode::ModelPicker;
        return Ok(());
    }

    match app.mode {
        Mode::Help => {
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Enter
                || key.code == KeyCode::Char('q')
            {
                app.mode = Mode::Chat;
            }
            return Ok(());
        }
        Mode::ToolCard => {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    if let Some(tool) = app.pending_tool.take() {
                        app.tool_approval_history.push(ToolApprovalEntry {
                            tool_name: tool.tool_name.clone(),
                            approved: true,
                        });
                        app.messages.push(ChatMsg {
                            role: ChatRole::System,
                            content: format!("Approved tool: {}", tool.tool_name),
                            sendable: false,
                        });
                        app.status = format!("Tool approved: {}", tool.tool_name);
                    }
                    app.mode = Mode::Chat;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    if let Some(tool) = app.pending_tool.take() {
                        app.tool_approval_history.push(ToolApprovalEntry {
                            tool_name: tool.tool_name.clone(),
                            approved: false,
                        });
                        app.messages.push(ChatMsg {
                            role: ChatRole::System,
                            content: format!("Denied tool: {}", tool.tool_name),
                            sendable: false,
                        });
                        app.status = format!("Tool denied: {}", tool.tool_name);
                    }
                    app.mode = Mode::Chat;
                }
                _ => {}
            }
            return Ok(());
        }
        Mode::ModelPicker => {
            match key.code {
                KeyCode::Esc => app.mode = Mode::Chat,
                KeyCode::Up => move_selection(&mut app.model_state, -1, app.model_options.len()),
                KeyCode::Down => move_selection(&mut app.model_state, 1, app.model_options.len()),
                KeyCode::PageUp => {
                    move_selection(&mut app.model_state, -5, app.model_options.len())
                }
                KeyCode::PageDown => {
                    move_selection(&mut app.model_state, 5, app.model_options.len())
                }
                KeyCode::Enter => {
                    if let Some(idx) = app.model_state.selected() {
                        if let Some(opt) = app.model_options.get(idx) {
                            app.selected_provider = opt.provider.clone();
                            app.selected_model = opt.model.clone();
                            // Avoid putting the full model id in the status line; the bottom panel
                            // already shows the selected model separately.
                            app.status = if app.selected_provider == "auto" {
                                "Selected auto".to_string()
                            } else {
                                format!("Selected {}", app.selected_provider)
                            };
                        }
                    }
                    app.mode = Mode::Chat;
                }
                KeyCode::Char('r') if ctrl => {
                    app.status = "Reloading models...".to_string();
                    app.bg_tasks = app.bg_tasks.saturating_add(1);
                    spawn_models_fetch(api.clone(), tx.clone());
                }
                _ => {}
            }
            return Ok(());
        }
        Mode::WorkspacePicker => {
            match key.code {
                KeyCode::Esc => {
                    app.pending_workspace_retry = false;
                    app.mode = Mode::Chat;
                }
                KeyCode::Up => {
                    move_selection(&mut app.workspace_state, -1, app.workspace_options.len())
                }
                KeyCode::Down => {
                    move_selection(&mut app.workspace_state, 1, app.workspace_options.len())
                }
                KeyCode::PageUp => {
                    move_selection(&mut app.workspace_state, -5, app.workspace_options.len())
                }
                KeyCode::PageDown => {
                    move_selection(&mut app.workspace_state, 5, app.workspace_options.len())
                }
                KeyCode::Enter => {
                    if let Some(idx) = app.workspace_state.selected() {
                        let (id, name, archived) = match app.workspace_options.get(idx) {
                            Some(opt) => (opt.id.clone(), opt.name.clone(), opt.archived),
                            None => ("".to_string(), "".to_string(), false),
                        };
                        if id.trim().is_empty() {
                            app.status = "No workspace selected.".to_string();
                            return Ok(());
                        }
                        if archived {
                            app.status = format!("Workspace is archived: {name}");
                            return Ok(());
                        }
                        apply_workspace_selection(app, &id, Some(&name));
                        app.status = format!("Workspace: {name}");
                    }
                    app.mode = Mode::Chat;

                    if app.pending_workspace_retry {
                        app.pending_workspace_retry = false;
                        retry_last_chat(api, tx, app);
                    }
                }
                KeyCode::Char('r') if ctrl => {
                    if !app.token_present {
                        app.status = "Missing token. Run `starbott auth login` first.".to_string();
                        return Ok(());
                    }
                    app.status = "Reloading workspaces...".to_string();
                    app.bg_tasks = app.bg_tasks.saturating_add(1);
                    spawn_workspaces_fetch(api.clone(), tx.clone());
                }
                _ => {}
            }
            return Ok(());
        }
        Mode::ThreadPicker => {
            match key.code {
                KeyCode::Esc => app.mode = Mode::Chat,
                KeyCode::Up => move_selection(&mut app.thread_state, -1, app.thread_options.len()),
                KeyCode::Down => move_selection(&mut app.thread_state, 1, app.thread_options.len()),
                KeyCode::PageUp => {
                    move_selection(&mut app.thread_state, -5, app.thread_options.len())
                }
                KeyCode::PageDown => {
                    move_selection(&mut app.thread_state, 5, app.thread_options.len())
                }
                KeyCode::Enter => {
                    if let Some(idx) = app.thread_state.selected() {
                        if let Some(thread) = app.thread_options.get(idx) {
                            app.active_thread_id = Some(thread.id.clone());
                            app.active_thread_title = Some(thread.title.clone());
                            app.status = format!("Switched to thread: {}", thread.title);
                            // Clear current messages when switching threads
                            app.messages.clear();
                        }
                    }
                    app.mode = Mode::Chat;
                }
                _ => {}
            }
            return Ok(());
        }
        Mode::MemoryPanel => {
            match key.code {
                KeyCode::Esc => app.mode = Mode::Chat,
                KeyCode::Up => move_selection(&mut app.memory_state, -1, app.memory_items.len()),
                KeyCode::Down => move_selection(&mut app.memory_state, 1, app.memory_items.len()),
                KeyCode::PageUp => {
                    move_selection(&mut app.memory_state, -5, app.memory_items.len())
                }
                KeyCode::PageDown => {
                    move_selection(&mut app.memory_state, 5, app.memory_items.len())
                }
                KeyCode::Char('m') if ctrl => {
                    // Toggle memory
                    app.memory_enabled = !app.memory_enabled;
                    app.status = format!(
                        "Toggling memory {}...",
                        if app.memory_enabled { "ON" } else { "OFF" }
                    );
                    app.bg_tasks = app.bg_tasks.saturating_add(1);
                    spawn_memory_toggle(api.clone(), tx.clone(), app.memory_enabled);
                }
                KeyCode::Char('r') if ctrl => {
                    // Refresh memory list
                    app.status = "Refreshing memory...".to_string();
                    app.bg_tasks = app.bg_tasks.saturating_add(2);
                    spawn_memory_fetch(api.clone(), tx.clone());
                    spawn_memory_settings_fetch(api.clone(), tx.clone());
                }
                _ => {}
            }
            return Ok(());
        }
        Mode::ChoiceModal => {
            match key.code {
                KeyCode::Esc => {
                    app.choice_prompt = None;
                    app.mode = Mode::Chat;
                }
                KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => {
                    let len = app
                        .choice_prompt
                        .as_ref()
                        .map(|p| p.options.len())
                        .unwrap_or(0);
                    move_selection_wrap(&mut app.choice_state, 1, len);
                }
                KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => {
                    let len = app
                        .choice_prompt
                        .as_ref()
                        .map(|p| p.options.len())
                        .unwrap_or(0);
                    move_selection_wrap(&mut app.choice_state, -1, len);
                }
                KeyCode::PageUp => {
                    let len = app
                        .choice_prompt
                        .as_ref()
                        .map(|p| p.options.len())
                        .unwrap_or(0);
                    move_selection(&mut app.choice_state, -5, len);
                }
                KeyCode::PageDown => {
                    let len = app
                        .choice_prompt
                        .as_ref()
                        .map(|p| p.options.len())
                        .unwrap_or(0);
                    move_selection(&mut app.choice_state, 5, len);
                }
                KeyCode::Enter => {
                    let Some(prompt) = app.choice_prompt.clone() else {
                        app.mode = Mode::Chat;
                        return Ok(());
                    };
                    let Some(idx) = app.choice_state.selected() else {
                        return Ok(());
                    };
                    let Some(opt) = prompt.options.get(idx).cloned() else {
                        return Ok(());
                    };

                    // Close modal before executing.
                    app.choice_prompt = None;
                    app.mode = Mode::Chat;

                    match opt.action {
                        ChoiceAction::SetWorkspace { workspace_id } => {
                            apply_workspace_selection(app, &workspace_id, Some(&opt.label));
                            app.status = format!("Workspace: {}", opt.label);

                            if app.pending_workspace_retry {
                                app.pending_workspace_retry = false;
                                retry_last_chat(api, tx, app);
                            }
                        }
                        ChoiceAction::Tool { tool_name, input } => {
                            let ws = match app.selected_workspace_id.clone() {
                                Some(v) if !v.trim().is_empty() => v,
                                _ => {
                                    app.messages.push(ChatMsg {
                                        role: ChatRole::System,
                                        content: "No workspace selected. Press F3 to pick one."
                                            .to_string(),
                                        sendable: false,
                                    });
                                    app.status = "Workspace required.".to_string();
                                    return Ok(());
                                }
                            };

                            app.messages.push(ChatMsg {
                                role: ChatRole::System,
                                content: format!("Running tool: {tool_name}"),
                                sendable: false,
                            });
                            app.waiting = true;
                            app.status = format!("Running tool: {tool_name}...");
                            app.bg_tasks = app.bg_tasks.saturating_add(1);
                            spawn_tool_propose(
                                api.clone(),
                                tx.clone(),
                                tool_name.clone(),
                                ws,
                                input,
                            );
                        }
                        ChoiceAction::Input { prompt } => {
                            app.text_prompt = Some(TextPromptState {
                                prompt,
                                input: Vec::new(),
                                cursor: 0,
                            });
                            app.mode = Mode::TextPromptModal;
                        }
                        ChoiceAction::SendMessage { text } => {
                            send_chat_text(api, tx, app, text);
                        }
                    }
                }
                _ => {}
            }
            return Ok(());
        }
        Mode::TextPromptModal => {
            let Some(mut st) = app.text_prompt.clone() else {
                app.mode = Mode::Chat;
                return Ok(());
            };

            match key.code {
                KeyCode::Esc => {
                    app.text_prompt = None;
                    app.mode = Mode::Chat;
                }
                KeyCode::Enter => {
                    let value = st.input.iter().collect::<String>();
                    let trimmed = value.trim().to_string();
                    if !trimmed.is_empty() {
                        let msg = format!("{}\n{}", st.prompt.trim(), trimmed);
                        app.text_prompt = None;
                        app.mode = Mode::Chat;
                        send_chat_text(api, tx, app, msg);
                    }
                }
                KeyCode::Backspace => {
                    if st.cursor > 0 && st.cursor <= st.input.len() {
                        st.cursor -= 1;
                        st.input.remove(st.cursor);
                    }
                    app.text_prompt = Some(st);
                }
                KeyCode::Left => {
                    st.cursor = st.cursor.saturating_sub(1);
                    app.text_prompt = Some(st);
                }
                KeyCode::Right => {
                    st.cursor = (st.cursor + 1).min(st.input.len());
                    app.text_prompt = Some(st);
                }
                KeyCode::Home => {
                    st.cursor = 0;
                    app.text_prompt = Some(st);
                }
                KeyCode::End => {
                    st.cursor = st.input.len();
                    app.text_prompt = Some(st);
                }
                KeyCode::Char(ch) => {
                    if ctrl {
                        return Ok(());
                    }
                    if st.cursor > st.input.len() {
                        st.cursor = st.input.len();
                    }
                    st.input.insert(st.cursor, ch);
                    st.cursor += 1;
                    app.text_prompt = Some(st);
                }
                _ => {}
            }
            return Ok(());
        }
        Mode::Chat => {}
        Mode::FileBrowser => {
            // Handle file browser navigation
            match key.code {
                KeyCode::Esc => {
                    app.mode = Mode::Chat;
                    app.file_browser_files = Vec::new();
                }
                KeyCode::Down => {
                    if let Some(mut state) = app.file_browser_state.selected() {
                        state = (state + 1).min(app.file_browser_files.len().saturating_sub(1));
                        app.file_browser_state.select(Some(state));
                    }
                }
                KeyCode::Up => {
                    if let Some(mut state) = app.file_browser_state.selected() {
                        state = state.saturating_sub(1);
                        app.file_browser_state.select(Some(state));
                    }
                }
                KeyCode::Enter => {
                    if let Some(selected_idx) = app.file_browser_state.selected() {
                        if let Some(selected_file) =
                            app.file_browser_files.get(selected_idx).cloned()
                        {
                            if selected_file.is_dir {
                                // Enter directory - load its contents
                                app.file_browser_path = selected_file.path.clone();
                                app.bg_tasks += 1;
                                let workspace_id =
                                    app.selected_workspace_id.clone().unwrap_or_default();
                                let path = selected_file.path.clone();
                                spawn_file_list_fetch(api.clone(), tx.clone(), workspace_id, path);
                            } else {
                                // Open file
                                app.mode = Mode::Chat;
                                app.input.clear();
                                app.cursor = 0;
                                app.file_browser_files = Vec::new();
                                let file_name = selected_file.name.clone();
                                // TODO: Load file contents into input for editing
                                app.status = format!("Opened file: {}", file_name);
                            }
                        }
                    }
                }
                KeyCode::BackTab | KeyCode::Left => {
                    // Go up one directory
                    if let Some(parent_path) = parent_directory(&app.file_browser_path) {
                        app.file_browser_path = parent_path;
                        app.bg_tasks += 1;
                        let workspace_id = app.selected_workspace_id.clone().unwrap_or_default();
                        let path = app.file_browser_path.clone();
                        spawn_file_list_fetch(api.clone(), tx.clone(), workspace_id, path);
                    }
                }
                _ => {}
            }
        }
    }

    // Mode::Chat key handling
    match key.code {
        KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('c') if ctrl => app.should_quit = true,
        KeyCode::F(1) => app.mode = Mode::Help,
        KeyCode::F(2) => app.mode = Mode::ModelPicker,
        // Completion shortcuts
        KeyCode::Tab if !app.input.is_empty() => {
            // Accept completion
            if app.show_completions && app.selected_completion.is_some() {
                if let Some(idx) = app.selected_completion {
                    if let Some(completion) = app.completions.get(idx) {
                        // Insert completion at cursor
                        app.input.extend(completion.text.chars());
                        app.cursor += completion.text.len();
                        app.show_completions = false;
                        app.completions.clear();
                    }
                }
            }
        }
        KeyCode::Esc if app.show_completions => {
            // Cancel completion
            app.show_completions = false;
            app.completions.clear();
            app.selected_completion = None;
        }
        KeyCode::Down if app.show_completions => {
            // Cycle completions down
            if let Some(current) = app.selected_completion {
                let next = (current + 1) % app.completions.len();
                app.selected_completion = Some(next);
            }
        }
        KeyCode::Up if app.show_completions => {
            // Cycle completions up
            if let Some(current) = app.selected_completion {
                let next = if current == 0 {
                    app.completions.len() - 1
                } else {
                    current - 1
                };
                app.selected_completion = Some(next);
            }
        }
        KeyCode::F(3) => {
            if !app.token_present {
                app.messages.push(ChatMsg {
                    role: ChatRole::System,
                    content: "Missing token. Run `starbott auth login` to select a workspace."
                        .to_string(),
                    sendable: false,
                });
                app.status = "Missing token.".to_string();
                return Ok(());
            }
            if app.workspace_options.is_empty() {
                app.status = "Loading workspaces...".to_string();
                app.bg_tasks = app.bg_tasks.saturating_add(1);
                spawn_workspaces_fetch(api.clone(), tx.clone());
            }
            app.mode = Mode::WorkspacePicker;
        }
        KeyCode::F(4) => {
            if !app.token_present {
                app.messages.push(ChatMsg {
                    role: ChatRole::System,
                    content: "Missing token. Run `starbott auth login` to access threads."
                        .to_string(),
                    sendable: false,
                });
                app.status = "Missing token.".to_string();
                return Ok(());
            }
            if app.thread_options.is_empty() {
                app.status = "Loading threads...".to_string();
                app.bg_tasks = app.bg_tasks.saturating_add(1);
                spawn_threads_fetch(api.clone(), tx.clone(), app.selected_workspace_id.clone());
            }
            app.mode = Mode::ThreadPicker;
        }
        KeyCode::F(6) => {
            app.mode = Mode::FileBrowser;
            app.file_browser_path = app.working_dir.clone(); // Start from launch directory
            app.bg_tasks += 1;
            let workspace_id = app.selected_workspace_id.clone().unwrap_or_default();
            let path = app.file_browser_path.to_string();
            spawn_file_list_fetch(api.clone(), tx.clone(), workspace_id, path);
        }
        KeyCode::F(5) => {
            if !app.token_present {
                app.messages.push(ChatMsg {
                    role: ChatRole::System,
                    content: "Missing token. Run `starbott auth login` to access memory."
                        .to_string(),
                    sendable: false,
                });
                app.status = "Missing token.".to_string();
                return Ok(());
            }
            // Load memory items and settings on first open
            if app.memory_items.is_empty() {
                app.status = "Loading memory...".to_string();
                app.bg_tasks = app.bg_tasks.saturating_add(2);
                spawn_memory_fetch(api.clone(), tx.clone());
                spawn_memory_settings_fetch(api.clone(), tx.clone());
            }
            app.mode = Mode::MemoryPanel;
        }
        KeyCode::Char('m') if ctrl => app.mode = Mode::ModelPicker,
        KeyCode::Char('d') if ctrl => app.show_debug = !app.show_debug,
        KeyCode::Char('r') if ctrl => {
            app.status = "Reloading...".to_string();
            app.bg_tasks = app.bg_tasks.saturating_add(2);
            spawn_models_fetch(api.clone(), tx.clone());
            spawn_health_fetch(api.clone(), tx.clone());
            if app.token_present {
                app.bg_tasks = app.bg_tasks.saturating_add(1);
                spawn_workspaces_fetch(api.clone(), tx.clone());
            }
        }
        KeyCode::PageUp => app.scroll_from_bottom = app.scroll_from_bottom.saturating_add(5),
        KeyCode::PageDown => app.scroll_from_bottom = app.scroll_from_bottom.saturating_sub(5),
        KeyCode::Enter => {
            if app.waiting {
                return Ok(());
            }
            let prompt = app.input.iter().collect::<String>();
            let trimmed = prompt.trim().to_string();
            if trimmed.is_empty() {
                return Ok(());
            }

            app.messages.push(ChatMsg {
                role: ChatRole::User,
                content: trimmed,
                sendable: true,
            });
            if app.cute != CuteMode::Off {
                // UI-only typing bubble: animated via spinner_step and not sent to the API.
                app.messages.push(ChatMsg {
                    role: ChatRole::Assistant,
                    content: "…".to_string(),
                    sendable: false,
                });
            }
            app.input.clear();
            app.cursor = 0;
            app.waiting = true;
            app.spinner_step = 0;
            app.spinner_last = Instant::now();
            app.status = thinking_status(app.cute, &mut app.rng, &mut app.last_phrase).to_string();

            let provider = app.selected_provider.clone();
            let model = app.selected_model.clone();
            let messages = app.messages.clone();
            let workspace_id = app.selected_workspace_id.clone();
            let active_thread_id = app.active_thread_id.clone();
            spawn_chat_request_stream_legacy(
                api.clone(),
                tx.clone(),
                provider,
                model,
                messages,
                workspace_id,
                active_thread_id,
                app.working_dir.clone(),
            );
        }
        KeyCode::Backspace => {
            if app.cursor > 0 && app.cursor <= app.input.len() {
                app.cursor -= 1;
                app.input.remove(app.cursor);
            }
        }
        KeyCode::Left => {
            app.cursor = app.cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            app.cursor = (app.cursor + 1).min(app.input.len());
        }
        KeyCode::Home => app.cursor = 0,
        KeyCode::End => app.cursor = app.input.len(),
        KeyCode::Char(ch) => {
            if ctrl {
                return Ok(());
            }
            if app.cursor > app.input.len() {
                app.cursor = app.input.len();
            }
            app.input.insert(app.cursor, ch);
            app.cursor += 1;

            // Trigger completion after typing
            if !app.completion_active && app.input.len() > 2 {
                // Debounce completion requests
                if app.cute == CuteMode::Off || app.rng % 10 == 0 {
                    // 10% chance in cute mode
                    trigger_completion(api, tx, app);
                }
            }
        }
        _ => {}
    }

    Ok(())
}

// ============================================================================
// Helper functions for key handling
// ============================================================================

fn move_selection(state: &mut ListState, delta: isize, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let cur = state.selected().unwrap_or(0) as isize;
    let mut next = cur + delta;
    if next < 0 {
        next = 0;
    }
    if next as usize >= len {
        next = (len - 1) as isize;
    }
    state.select(Some(next as usize));
}

fn move_selection_wrap(state: &mut ListState, delta: isize, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let cur = state.selected().unwrap_or(0) as isize;
    let mut next = cur + delta;
    if next < 0 {
        next = (len - 1) as isize;
    }
    if next as usize >= len {
        next = 0;
    }
    state.select(Some(next as usize));
}

fn apply_workspace_selection(app: &mut App, workspace_id: &str, name_hint: Option<&str>) {
    let ws = workspace_id.trim();
    if ws.is_empty() {
        return;
    }

    app.selected_workspace_id = Some(ws.to_string());
    app.selected_workspace_name = name_hint
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    if let Some(idx) = app.workspace_options.iter().position(|w| w.id == ws) {
        app.workspace_state.select(Some(idx));
        if app.selected_workspace_name.is_none() {
            app.selected_workspace_name = Some(app.workspace_options[idx].name.clone());
        }
    }

    // Workspace switch invalidates cached thread/chat selection.
    app.thread_options.clear();
    app.thread_state.select(None);
    app.active_thread_id = None;
    app.active_thread_title = None;

    if let Some(p) = profile_mut(&mut app.config, &app.profile) {
        p.workspace_id = Some(ws.to_string());
        if let Err(e) = save_config(&app.config) {
            app.messages.push(ChatMsg {
                role: ChatRole::System,
                content: format!("Failed saving workspace selection: {e}"),
                sendable: false,
            });
        }
    } else {
        app.messages.push(ChatMsg {
            role: ChatRole::System,
            content: "Failed saving workspace selection: profile not found.".to_string(),
            sendable: false,
        });
    }
}

fn retry_last_chat(api: &ApiClient, tx: &mpsc::UnboundedSender<TuiMsg>, app: &mut App) {
    if app.waiting {
        return;
    }
    if !app
        .messages
        .iter()
        .any(|m| m.sendable && matches!(m.role, ChatRole::User))
    {
        return;
    }

    if app.cute != CuteMode::Off {
        app.messages.push(ChatMsg {
            role: ChatRole::Assistant,
            content: "…".to_string(),
            sendable: false,
        });
    }
    app.waiting = true;
    app.spinner_step = 0;
    app.spinner_last = Instant::now();
    app.status = thinking_status(app.cute, &mut app.rng, &mut app.last_phrase).to_string();

    let provider = app.selected_provider.clone();
    let model = app.selected_model.clone();
    let messages = app.messages.clone();
    let workspace_id = app.selected_workspace_id.clone();
    let active_thread_id = app.active_thread_id.clone();
    spawn_chat_request_stream_legacy(
        api.clone(),
        tx.clone(),
        provider,
        model,
        messages,
        workspace_id,
        active_thread_id,
        app.working_dir.clone(),
    );
}

fn send_chat_text(
    api: &ApiClient,
    tx: &mpsc::UnboundedSender<TuiMsg>,
    app: &mut App,
    text: String,
) {
    if app.waiting {
        return;
    }
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return;
    }

    app.messages.push(ChatMsg {
        role: ChatRole::User,
        content: trimmed,
        sendable: true,
    });
    if app.cute != CuteMode::Off {
        app.messages.push(ChatMsg {
            role: ChatRole::Assistant,
            content: "…".to_string(),
            sendable: false,
        });
    }
    app.input.clear();
    app.cursor = 0;
    app.waiting = true;
    app.spinner_step = 0;
    app.spinner_last = Instant::now();
    app.status = thinking_status(app.cute, &mut app.rng, &mut app.last_phrase).to_string();

    let provider = app.selected_provider.clone();
    let model = app.selected_model.clone();
    let messages = app.messages.clone();
    let workspace_id = app.selected_workspace_id.clone();
    let active_thread_id = app.active_thread_id.clone();
    spawn_chat_request_stream_legacy(
        api.clone(),
        tx.clone(),
        provider,
        model,
        messages,
        workspace_id,
        active_thread_id,
        app.working_dir.clone(),
    );
}

// ============================================================================
// Completion helper functions
// ============================================================================

fn trigger_completion(api: &ApiClient, tx: &mpsc::UnboundedSender<TuiMsg>, app: &mut App) {
    // Create a mock file path and content for completion
    let file_path = "/current_file.js".to_string(); // In real implementation, this would track current file
    let content = app.input.iter().collect::<String>();

    // Get current cursor position (end of input for now)
    let cursor_pos = (app.input.len(), app.input.len());

    app.completion_active = true;
    spawn_completion_request(api.clone(), tx.clone(), file_path, content, cursor_pos);
}

#[cfg(test)]
mod tests {
    use super::{is_local_list_request, is_local_pwd_request, parse_local_list_target};

    #[test]
    fn list_request_matches_contents_prompt() {
        assert!(is_local_list_request("can you tell me what the contents are?"));
        assert!(is_local_list_request("can you tell me what the contents are though?"));
    }

    #[test]
    fn list_request_matches_followup_working_directory_fragment() {
        assert!(is_local_list_request("of our working directory?"));
    }

    #[test]
    fn pwd_does_not_override_list_prompt() {
        assert!(!is_local_pwd_request("can you tell me what the contents are?"));
    }

    #[test]
    fn parse_target_from_natural_language() {
        assert_eq!(
            parse_local_list_target("what's in the deploy folder?"),
            Some("deploy".to_string())
        );
        assert_eq!(
            parse_local_list_target("look inside `src` directory"),
            Some("src".to_string())
        );
    }
}
