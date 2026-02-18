// PHASE 3: TUI message handler extracted from tui.rs
// Contains handle_tui_msg function for processing async results

use tokio::sync::mpsc;
use serde_json::json;

use crate::api::ApiClient;
use crate::tui::types::{App, TuiMsg, ChatMsg, ChatRole, Mode, ThreadOption};
use crate::parse::response::{extract_reply, extract_provider_model, extract_usage_line};

// Import helper functions from parent tui module
// These remain in tui.rs for now (can be extracted to parse/format modules later)
use crate::commands::tui::{
    parse_model_options, find_selected_model_index,
    parse_vertex_ok, parse_workspace_options, parse_thread_options, parse_choice_prompt,
    parse_memory_items, parse_memory_settings,
    format_dir_listing_for_user, format_file_read_for_user,
    parse_lane, format_tool_propose_result,
    ready_status, format_success_status, format_error_status,
    remove_typing_placeholder,
};

use super::async_ops::spawn_workspaces_fetch;

pub fn handle_tui_msg(api: &ApiClient, tx: &mpsc::UnboundedSender<TuiMsg>, app: &mut App, msg: TuiMsg) {
    match msg {
        TuiMsg::Models(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
            Ok(resp) => {
                if let Some(options) = parse_model_options(&resp.json) {
                    if !options.is_empty() {
                        app.hints.azure_present = options.iter().any(|o| o.provider == "azure");
                        app.hints.cf_present = options.iter().any(|o| o.provider == "cloudflare");
                        app.model_options = options;
                        let idx = find_selected_model_index(
                            &app.model_options,
                            &app.selected_provider,
                            app.selected_model.as_deref(),
                        )
                        .unwrap_or(0);
                        app.model_state.select(Some(idx));
                        app.status = ready_status(app.cute, app.status.as_str()).to_string();
                    } else {
                        app.status = "No models returned by server.".to_string();
                    }
                } else {
                    app.status = "Failed parsing /v1/models response.".to_string();
                }
            }
            Err(err) => {
                app.status = format!("Failed loading models: {err}");
            }
        }
        }
        TuiMsg::Health(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
            Ok(resp) => {
                app.hints.vertex_ok = parse_vertex_ok(&resp.json);
            }
            Err(_) => {
                app.hints.vertex_ok = None;
            }
        }
        }
        TuiMsg::Workspaces(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
                Ok(resp) => {
                    if let Some(options) = parse_workspace_options(&resp.json) {
                        app.workspace_options = options;
                        if app.workspace_state.selected().is_none() && !app.workspace_options.is_empty() {
                            app.workspace_state.select(Some(0));
                        }

                        if let Some(ref sel_id) = app.selected_workspace_id {
                            if let Some(idx) = app
                                .workspace_options
                                .iter()
                                .position(|w| w.id == *sel_id)
                            {
                                app.workspace_state.select(Some(idx));
                                app.selected_workspace_name =
                                    Some(app.workspace_options[idx].name.clone());
                            } else {
                                app.selected_workspace_name = None;
                            }
                        }
                    } else {
                        app.status = "Failed parsing /v1/workspaces response.".to_string();
                    }
                }
                Err(err) => {
                    app.status = format!("Failed loading workspaces: {err}");
                }
            }
        }
        TuiMsg::Threads(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
                Ok(resp) => {
                    if let Some(options) = parse_thread_options(&resp.json) {
                        app.thread_options = options;
                        if app.thread_state.selected().is_none() && !app.thread_options.is_empty() {
                            app.thread_state.select(Some(0));
                        }

                        // Restore selection if we have an active thread
                        if let Some(ref thread_id) = app.active_thread_id {
                            if let Some(idx) = app
                                .thread_options
                                .iter()
                                .position(|t| t.id == *thread_id)
                            {
                                app.thread_state.select(Some(idx));
                                app.active_thread_title =
                                    Some(app.thread_options[idx].title.clone());
                            } else {
                                app.active_thread_title = None;
                            }
                        }
                    } else {
                        app.status = "Failed parsing /v1/threads response.".to_string();
                    }
                }
                Err(err) => {
                    app.status = format!("Failed loading threads: {err}");
                }
            }
        }
        TuiMsg::Memory(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
                Ok(resp) => {
                    if let Some(options) = parse_memory_items(&resp.json) {
                        app.memory_items = options;
                        if app.memory_state.selected().is_none() && !app.memory_items.is_empty() {
                            app.memory_state.select(Some(0));
                        }
                        app.status = format!("Loaded {} memory items", app.memory_items.len());
                    } else {
                        app.status = "Failed parsing /v1/memory response.".to_string();
                    }
                }
                Err(err) => {
                    app.status = format!("Failed loading memory: {err}");
                }
            }
        }
        TuiMsg::MemorySettings(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
                Ok(resp) => {
                    if let Some(settings) = parse_memory_settings(&resp.json) {
                        app.memory_settings = settings;
                        app.memory_enabled = app.memory_settings.enabled;
                        app.status = format!(
                            "Memory: {} (max {} items)",
                            if app.memory_enabled { "enabled" } else { "disabled" },
                            app.memory_settings.max_items_injected
                        );
                    } else {
                        app.status = "Failed parsing /v1/memory/settings response.".to_string();
                    }
                }
                Err(err) => {
                    app.status = format!("Failed loading memory settings: {err}");
                }
            }
        }
        TuiMsg::Chat(res) => {
            app.waiting = false;
            remove_typing_placeholder(app);
            match res {
                Ok(resp) => {
                    app.last_request_id = resp.request_id.clone();
                    app.last_elapsed_ms = Some(resp.elapsed_ms);

                    let need_workspace = resp
                        .json
                        .get("needWorkspace")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let choice_prompt = parse_choice_prompt(&resp.json);

                    if need_workspace && choice_prompt.is_none() {
                        if let Some(options) = parse_workspace_options(&resp.json) {
                            app.workspace_options = options;
                            if !app.workspace_options.is_empty() {
                                app.workspace_state.select(Some(0));
                            }
                        } else if app.token_present {
                            app.bg_tasks = app.bg_tasks.saturating_add(1);
                            spawn_workspaces_fetch(api.clone(), tx.clone());
                        }

                        let msg = extract_reply(&resp.json)
                            .unwrap_or_else(|| "Pick a workspace to use, then retry your request.".to_string());
                        app.messages.push(ChatMsg {
                            role: ChatRole::System,
                            content: msg,
                            sendable: false,
                        });
                        app.pending_workspace_retry = true;
                        app.mode = Mode::WorkspacePicker;
                        app.status = "Workspace required. Select one (F3).".to_string();
                        return;
                    }

                    // Show server auto-tools deterministically in the chat (UI-only).
                    if let Some(auto_tools) = resp.json.get("autoTools").and_then(|v| v.as_array())
                    {
                        for t in auto_tools {
                            let tool_name = t
                                .get("toolName")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            if tool_name == "file.dir" {
                                if let Some(result) = t.get("result") {
                                    if let Some(listing) = format_dir_listing_for_user(result) {
                                        app.messages.push(ChatMsg {
                                            role: ChatRole::System,
                                            content: listing,
                                            sendable: false,
                                        });
                                    }
                                }
                            } else if tool_name == "file.read" {
                                if let Some(result) = t.get("result") {
                                    if let Some(text) = format_file_read_for_user(result) {
                                        app.messages.push(ChatMsg {
                                            role: ChatRole::System,
                                            content: text,
                                            sendable: false,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    app.lane = parse_lane(&resp.json);
                    app.success_count = app.success_count.saturating_add(1);

                    let reply =
                        extract_reply(&resp.json).unwrap_or_else(|| "(No text response)".to_string());
                    app.messages.push(ChatMsg {
                        role: ChatRole::Assistant,
                        content: reply,
                        sendable: true,
                    });

                    if let Some(cp) = choice_prompt {
                        let title = cp.title.clone();
                        let hint = if cp.hint.trim().is_empty() {
                            "Tab/↑↓ select • Enter choose • Esc cancel".to_string()
                        } else {
                            cp.hint.clone()
                        };
                        app.messages.push(ChatMsg {
                            role: ChatRole::System,
                            content: format!("Choice: {title}\n{hint}"),
                            sendable: false,
                        });
                        app.choice_prompt = Some(cp);
                        app.choice_state.select(Some(0));
                        app.mode = Mode::ChoiceModal;
                        app.status = "Choice prompt".to_string();
                        if need_workspace
                            || resp
                                .json
                                .get("error")
                                .and_then(|v| v.as_str())
                                .map(|s| s == "NO_WORKSPACE_SELECTED")
                                .unwrap_or(false)
                        {
                            app.pending_workspace_retry = true;
                        }
                    }

                    let (provider, model) = extract_provider_model(&resp.json);
                    app.last_provider = provider.clone();
                    app.last_model = model.clone();
                    app.activity_lines.push(format!(
                        "Completed: {} via {}",
                        model.as_deref().unwrap_or("-"),
                        provider.as_deref().unwrap_or("-"),
                    ));
                    let usage = extract_usage_line(&resp.json);
                    app.last_usage = Some(usage);
                    app.status = format_success_status(
                        app.cute,
                        &mut app.rng,
                        &mut app.last_phrase,
                        app.success_count,
                        app.lane,
                    );
                }
                Err(err) => {
                    app.messages.push(ChatMsg {
                        role: ChatRole::System,
                        content: format!("Error: {err}"),
                        sendable: false,
                    });
                    app.status =
                        format_error_status(app.cute, &mut app.rng, &mut app.last_phrase, &err);
                }
            }
        }
        TuiMsg::Tool(tool_name, res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            app.waiting = false;
            match res {
                Ok(resp) => {
                    let rendered = format_tool_propose_result(&tool_name, &resp.json);
                    app.messages.push(ChatMsg {
                        role: ChatRole::System,
                        content: rendered,
                        sendable: true,
                    });
                    app.status = format!("Tool completed: {tool_name}");
                }
                Err(err) => {
                    app.messages.push(ChatMsg {
                        role: ChatRole::System,
                        content: format!("Tool error ({tool_name}): {err}"),
                        sendable: false,
                    });
                    app.status = format!("Tool failed: {tool_name}");
                }
            }
        }
        TuiMsg::StreamToken(token) => {
            // Append token to the last assistant message (streaming)
            if let Some(last_msg) = app.messages.last_mut() {
                if last_msg.role == ChatRole::Assistant && !last_msg.sendable {
                    // This is the streaming placeholder message
                    if last_msg.content == "…" {
                        last_msg.content = token;
                    } else {
                        last_msg.content.push_str(&token);
                    }
                }
            }
        }
        TuiMsg::StreamDone(metadata) => {
            // Streaming complete - mark message as complete and extract metadata
            app.waiting = false;

            if let Some(last_msg) = app.messages.last_mut() {
                if last_msg.role == ChatRole::Assistant && !last_msg.sendable {
                    // Mark as sendable (complete)
                    last_msg.sendable = true;

                    // If still placeholder, try to extract content from message.final event
                    if last_msg.content == "…" {
                        if let Some(content) = metadata.get("content").and_then(|v| v.as_str()) {
                            if !content.is_empty() {
                                last_msg.content = content.to_string();
                            } else {
                                last_msg.content = "(No text response)".to_string();
                            }
                        } else {
                            last_msg.content = "(No text response)".to_string();
                        }
                    }
                }
            }

            let chat_id = metadata
                .get("chatId")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .or_else(|| {
                    metadata
                        .get("chat")
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                });

            let chat_title = metadata
                .get("chatTitle")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .or_else(|| {
                    metadata
                        .get("chat")
                        .and_then(|v| v.get("title"))
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                });

            let chat_updated_at = metadata
                .get("chat")
                .and_then(|v| v.get("updatedAt"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            if let Some(chat_id) = chat_id {
                app.active_thread_id = Some(chat_id.clone());

                if let Some(existing) = app.thread_options.iter_mut().find(|t| t.id == chat_id) {
                    if let Some(title) = chat_title.clone() {
                        existing.title = title;
                    }
                    if let Some(updated_at) = chat_updated_at.clone() {
                        existing.last_message_at = Some(updated_at);
                    }
                    existing.message_count = existing.message_count.saturating_add(1);
                } else {
                    app.thread_options.insert(
                        0,
                        ThreadOption {
                            id: chat_id.clone(),
                            title: chat_title.clone().unwrap_or_else(|| "Current Chat".to_string()),
                            mode: None,
                            last_message_at: chat_updated_at.clone(),
                            is_pinned: false,
                            message_count: 1,
                        },
                    );
                }

                if let Some(idx) = app.thread_options.iter().position(|t| t.id == chat_id) {
                    app.thread_state.select(Some(idx));
                }
            }

            if let Some(title) = chat_title {
                app.active_thread_title = Some(title);
            }

            // Extract metadata
            let (provider, model) = extract_provider_model(&metadata);
            app.last_provider = provider.clone();
            app.last_model = model.clone();
            app.activity_lines.push(format!(
                "Completed: {} via {}",
                model.as_deref().unwrap_or("-"),
                provider.as_deref().unwrap_or("-"),
            ));

            let usage = extract_usage_line(&metadata);
            app.last_usage = Some(usage);

            app.lane = parse_lane(&metadata);
            app.success_count = app.success_count.saturating_add(1);
            app.status = format_success_status(
                app.cute,
                &mut app.rng,
                &mut app.last_phrase,
                app.success_count,
                app.lane,
            );
        }
        TuiMsg::StreamError(error) => {
            app.waiting = false;
            remove_typing_placeholder(app);

            app.messages.push(ChatMsg {
                role: ChatRole::System,
                content: format!("Streaming error: {error}"),
                sendable: false,
            });
            app.status = format_error_status(
                app.cute,
                &mut app.rng,
                &mut app.last_phrase,
                &crate::errors::CliError::Generic(error),
            );
        }
        // New Starbot_API message handlers
        TuiMsg::Projects(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
                Ok(resp) => {
                    let workspaces = resp
                        .json
                        .get("projects")
                        .and_then(|v| v.as_array())
                        .map(|projects| {
                            projects
                                .iter()
                                .filter_map(|project| {
                                    let id = project
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .map(str::trim)
                                        .filter(|s| !s.is_empty())?;
                                    let name = project
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .map(str::trim)
                                        .filter(|s| !s.is_empty())
                                        .unwrap_or("Project");
                                    let last_used_at = project
                                        .get("updatedAt")
                                        .or_else(|| project.get("createdAt"))
                                        .and_then(|v| v.as_str());
                                    Some(json!({
                                        "id": id,
                                        "name": name,
                                        "rootPath": serde_json::Value::Null,
                                        "archived": false,
                                        "lastUsedAt": last_used_at,
                                    }))
                                })
                                .collect::<Vec<serde_json::Value>>()
                        })
                        .unwrap_or_default();

                    let normalized = json!({ "workspaces": workspaces });
                    if let Some(options) = parse_workspace_options(&normalized) {
                        app.workspace_options = options;
                        if app.workspace_state.selected().is_none() && !app.workspace_options.is_empty() {
                            app.workspace_state.select(Some(0));
                        }
                        if let Some(ref sel_id) = app.selected_workspace_id {
                            if let Some(idx) = app.workspace_options.iter().position(|w| w.id == *sel_id) {
                                app.workspace_state.select(Some(idx));
                                app.selected_workspace_name =
                                    Some(app.workspace_options[idx].name.clone());
                            }
                        }
                        app.status = format!("Loaded {} projects", app.workspace_options.len());
                    } else {
                        app.status = "Failed parsing /v1/projects response.".to_string();
                    }
                }
                Err(err) => {
                    app.status = format!("Failed to load projects: {err}");
                }
            }
        }
        TuiMsg::Chats(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
                Ok(resp) => {
                    let threads = resp
                        .json
                        .get("chats")
                        .and_then(|v| v.as_array())
                        .map(|chats| {
                            chats
                                .iter()
                                .filter_map(|chat| {
                                    let id = chat
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .map(str::trim)
                                        .filter(|s| !s.is_empty())?;
                                    let title = chat
                                        .get("title")
                                        .and_then(|v| v.as_str())
                                        .map(str::trim)
                                        .filter(|s| !s.is_empty())
                                        .unwrap_or("Untitled");
                                    let updated_at = chat.get("updatedAt").and_then(|v| v.as_str());
                                    let message_count = chat
                                        .get("_count")
                                        .and_then(|v| v.get("messages"))
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);

                                    Some(json!({
                                        "id": id,
                                        "title": title,
                                        "mode": serde_json::Value::Null,
                                        "lastMessageAt": updated_at,
                                        "isPinned": false,
                                        "_count": { "messages": message_count },
                                    }))
                                })
                                .collect::<Vec<serde_json::Value>>()
                        })
                        .unwrap_or_default();

                    let normalized = json!({ "threads": threads });
                    if let Some(options) = parse_thread_options(&normalized) {
                        app.thread_options = options;
                        if app.thread_state.selected().is_none() && !app.thread_options.is_empty() {
                            app.thread_state.select(Some(0));
                        }
                        if let Some(ref thread_id) = app.active_thread_id {
                            if let Some(idx) = app.thread_options.iter().position(|t| t.id == *thread_id) {
                                app.thread_state.select(Some(idx));
                                app.active_thread_title =
                                    Some(app.thread_options[idx].title.clone());
                            }
                        }
                        app.status = format!("Loaded {} chats", app.thread_options.len());
                    } else {
                        app.status = "Failed parsing /v1/chats response.".to_string();
                    }
                }
                Err(err) => {
                    app.status = format!("Failed to load chats: {err}");
                }
            }
        }
        TuiMsg::Messages(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
                Ok(resp) => {
                    if let Some(items) = resp.json.get("messages").and_then(|v| v.as_array()) {
                        let parsed = items
                            .iter()
                            .filter_map(|item| {
                                let role = match item.get("role").and_then(|v| v.as_str()) {
                                    Some("user") => ChatRole::User,
                                    Some("assistant") => ChatRole::Assistant,
                                    Some("system") | Some("tool") => ChatRole::System,
                                    _ => return None,
                                };
                                let content = item
                                    .get("content")
                                    .and_then(|v| v.as_str())
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())?
                                    .to_string();
                                Some(ChatMsg {
                                    role,
                                    content,
                                    sendable: role != ChatRole::System,
                                })
                            })
                            .collect::<Vec<ChatMsg>>();

                        app.messages = parsed;
                        app.status = format!("Loaded {} messages", app.messages.len());
                    } else {
                        app.status = "Failed parsing /v1/messages response.".to_string();
                    }
                }
                Err(err) => {
                    app.status = format!("Failed to load messages: {err}");
                }
            }
        }
        TuiMsg::ProjectCreated(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
                Ok(resp) => {
                    if let Some(project) = resp.json.get("project") {
                        if let Some(name) = project.get("name").and_then(|n| n.as_str()) {
                            app.status = format!("Created project: {name}");
                        }
                    }
                }
                Err(err) => {
                    app.status = format!("Failed to create project: {err}");
                }
            }
        }
        TuiMsg::ChatCreated(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
                Ok(resp) => {
                    if let Some(chat) = resp.json.get("chat") {
                        if let Some(title) = chat.get("title").and_then(|t| t.as_str()) {
                            app.status = format!("Created chat: {title}");
                        }
                    }
                }
                Err(err) => {
                    app.status = format!("Failed to create chat: {err}");
                }
            }
        }
        TuiMsg::MessageAdded(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            match res {
                Ok(_resp) => {
                    app.status = "Message added".to_string();
                }
                Err(err) => {
                    app.status = format!("Failed to add message: {err}");
                }
            }
        }
        TuiMsg::ChatCancelled(res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            app.waiting = false;
            match res {
                Ok(_) => {
                    app.status = "Generation cancelled".to_string();
                }
                Err(err) => {
                    app.status = format!("Failed to cancel: {err}");
                }
            }
        }
        TuiMsg::CompletionRequest(file_path, res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);
            app.completion_active = false;

            match res {
                Ok(resp) => {
                    if let Some(suggestions) = resp.json.get("suggestions").and_then(|v| v.as_array()) {
                        app.completions = suggestions
                            .iter()
                            .filter_map(|item| {
                                Some(crate::tui::types::Completion {
                                    text: item.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    confidence: item.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5),
                                    language: item.get("language").and_then(|v| v.as_str()).unwrap_or("text").to_string(),
                                })
                            })
                            .collect();

                        if !app.completions.is_empty() {
                            app.show_completions = true;
                            app.selected_completion = Some(0);
                            app.status = format!("{} completions", app.completions.len());
                        } else {
                            app.show_completions = false;
                            app.status = "No completions available".to_string();
                        }
                    } else {
                        app.show_completions = false;
                        app.status = "Failed to parse completion response".to_string();
                    }
                }
                Err(err) => {
                    app.show_completions = false;
                    app.completions.clear();
                    app.status = format!("Completion failed: {err}");
                }
            }
        }
        TuiMsg::FileListRequest(workspace_id, path, res) => {
            app.bg_tasks = app.bg_tasks.saturating_sub(1);

            match res {
                Ok(resp) => {
                    if let Some(files) = resp.json.get("files").and_then(|v| v.as_array()) {
                        app.file_browser_files = files
                            .iter()
                            .filter_map(|item| {
                                Some(crate::tui::types::FileNode {
                                    name: item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    path: item.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    is_dir: item.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false),
                                    size: item.get("size").and_then(|v| v.as_u64()),
                                    last_modified: item.get("last_modified").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                })
                            })
                            .collect();
                        app.status = format!("Loaded {} files", app.file_browser_files.len());
                    }
                }
                Err(err) => {
                    app.status = format!("Failed to load files: {}", err);
                    app.file_browser_files = Vec::new();
                }
            }
        }
        TuiMsg::StreamStatus(status) => {
            // Show routing/thinking status in UI
            app.status = format!("⚡ {status}");
        }
    }
}
