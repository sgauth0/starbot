// PHASE 3: TUI key handler extracted from tui.rs
// Contains handle_event and handle_key functions for keyboard input

use std::time::Instant;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::config::{profile_mut, save_config};
use crate::cute::CuteMode;
use crate::errors::CliError;
use crate::tui::types::{
    App, TuiMsg, ChatMsg, ChatRole, Mode, ChoiceAction, TextPromptState, ToolApprovalEntry,
};

use super::async_ops::{
    spawn_models_fetch, spawn_health_fetch, spawn_workspaces_fetch, spawn_threads_fetch,
    spawn_memory_fetch, spawn_memory_settings_fetch, spawn_memory_toggle,
    spawn_chat_request_stream, spawn_tool_propose,
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
                KeyCode::Up => {
                    move_selection(&mut app.thread_state, -1, app.thread_options.len())
                }
                KeyCode::Down => {
                    move_selection(&mut app.thread_state, 1, app.thread_options.len())
                }
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
                KeyCode::Up => {
                    move_selection(&mut app.memory_state, -1, app.memory_items.len())
                }
                KeyCode::Down => {
                    move_selection(&mut app.memory_state, 1, app.memory_items.len())
                }
                KeyCode::PageUp => {
                    move_selection(&mut app.memory_state, -5, app.memory_items.len())
                }
                KeyCode::PageDown => {
                    move_selection(&mut app.memory_state, 5, app.memory_items.len())
                }
                KeyCode::Char('m') if ctrl => {
                    // Toggle memory
                    app.memory_enabled = !app.memory_enabled;
                    app.status = format!("Toggling memory {}...", if app.memory_enabled { "ON" } else { "OFF" });
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
    }

    // Mode::Chat key handling
    match key.code {
        KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('c') if ctrl => app.should_quit = true,
        KeyCode::F(1) => app.mode = Mode::Help,
        KeyCode::F(2) => app.mode = Mode::ModelPicker,
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
                    content: "Missing token. Run `starbott auth login` to access threads.".to_string(),
                    sendable: false,
                });
                app.status = "Missing token.".to_string();
                return Ok(());
            }
            if app.thread_options.is_empty() {
                app.status = "Loading threads...".to_string();
                app.bg_tasks = app.bg_tasks.saturating_add(1);
                spawn_threads_fetch(api.clone(), tx.clone());
            }
            app.mode = Mode::ThreadPicker;
        }
        KeyCode::F(5) => {
            if !app.token_present {
                app.messages.push(ChatMsg {
                    role: ChatRole::System,
                    content: "Missing token. Run `starbott auth login` to access memory.".to_string(),
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
            spawn_chat_request_stream(api.clone(), tx.clone(), provider, model, messages, workspace_id);
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
    spawn_chat_request_stream(api.clone(), tx.clone(), provider, model, messages, workspace_id);
}

fn send_chat_text(api: &ApiClient, tx: &mpsc::UnboundedSender<TuiMsg>, app: &mut App, text: String) {
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
    spawn_chat_request_stream(api.clone(), tx.clone(), provider, model, messages, workspace_id);
}
