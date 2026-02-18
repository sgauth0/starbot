// Async operation handlers for Starbot_TUI
// Calls the local Starbot_API (localhost:3737)

use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::api::{ApiClient, ApiResponse};
use crate::tui::types::{ChatMsg, ChatRole, TuiMsg, Completion};

pub fn spawn_health_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        let res = api.get_json("/health", None, false).await;
        let _ = tx.send(TuiMsg::Health(res));
    });
}

pub fn spawn_projects_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        let res = api.get_json("/v1/projects", None, false).await;
        let _ = tx.send(TuiMsg::Projects(res));
    });
}

pub fn spawn_chats_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>, project_id: String) {
    tokio::spawn(async move {
        let path = format!("/v1/projects/{}/chats", project_id);
        let res = api.get_json(&path, None, false).await;
        let _ = tx.send(TuiMsg::Chats(res));
    });
}

pub fn spawn_messages_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>, chat_id: String) {
    tokio::spawn(async move {
        let path = format!("/v1/chats/{}/messages", chat_id);
        let res = api.get_json(&path, None, false).await;
        let _ = tx.send(TuiMsg::Messages(res));
    });
}

pub fn spawn_create_project(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>, name: String) {
    tokio::spawn(async move {
        let body = json!({ "name": name });
        let res = api.post_json("/v1/projects", Some(body), false).await;
        let _ = tx.send(TuiMsg::ProjectCreated(res));
    });
}

pub fn spawn_create_chat(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>, project_id: String, title: Option<String>) {
    tokio::spawn(async move {
        let path = format!("/v1/projects/{}/chats", project_id);
        let mut body = json!({});
        if let Some(t) = title {
            body["title"] = json!(t);
        }
        let res = api.post_json(&path, Some(body), false).await;
        let _ = tx.send(TuiMsg::ChatCreated(res));
    });
}

pub fn spawn_add_message(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>, chat_id: String, role: String, content: String) {
    tokio::spawn(async move {
        let path = format!("/v1/chats/{}/messages", chat_id);
        let body = json!({
            "role": role,
            "content": content,
        });
        let res = api.post_json(&path, Some(body), false).await;
        let _ = tx.send(TuiMsg::MessageAdded(res));
    });
}

/// Streaming chat request (SSE)
/// First adds user message to chat, then starts generation stream
pub fn spawn_chat_request_stream(
    api: ApiClient,
    tx: mpsc::UnboundedSender<TuiMsg>,
    chat_id: String,
    mode: String,
    speed: bool,
) {
    tokio::spawn(async move {
        let path = format!("/v1/chats/{}/run", chat_id);
        let body = json!({
            "mode": mode,
            "speed": speed,
            "auto": true,
        });

        match api.post_stream(&path, Some(body), false).await {
            Ok(mut rx) => {
                while let Some(event) = rx.recv().await {
                    match event.event_type.as_str() {
                        "status" => {
                            // Status update (routing, thinking, etc.)
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                if let Some(msg) = data.get("message").and_then(|v| v.as_str()) {
                                    let _ = tx.send(TuiMsg::StreamStatus(msg.to_string()));
                                }
                            }
                        }
                        "token.delta" => {
                            // Streaming token
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                if let Some(text) = data.get("text").and_then(|v| v.as_str()) {
                                    let _ = tx.send(TuiMsg::StreamToken(text.to_string()));
                                }
                            }
                        }
                        "message.final" => {
                            // Final assistant message
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                let _ = tx.send(TuiMsg::StreamDone(data));
                            }
                        }
                        "error" => {
                            // Error during generation
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                if let Some(msg) = data.get("message").and_then(|v| v.as_str()) {
                                    let _ = tx.send(TuiMsg::StreamError(msg.to_string()));
                                }
                            }
                        }
                        _ => {
                            // Ignore unknown events for now (tool.start, tool.end, etc.)
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(TuiMsg::StreamError(e.to_string()));
            }
        }
    });
}

pub fn spawn_cancel_chat(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>, chat_id: String) {
    tokio::spawn(async move {
        let path = format!("/v1/chats/{}/cancel", chat_id);
        let res = api.post_json(&path, None, false).await;
        let _ = tx.send(TuiMsg::ChatCancelled(res));
    });
}

// Legacy functions kept for compatibility - these will be removed once TUI is fully refactored
// For now, they return empty/placeholder responses

pub fn spawn_models_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        let res = api.get_json("/v1/models", None, false).await;
        let _ = tx.send(TuiMsg::Models(res));
    });
}

pub fn spawn_workspaces_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        let res = api.get_json("/v1/projects", None, false).await;
        let mapped = res.map(|resp| {
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
                                "rootPath": Value::Null,
                                "archived": false,
                                "lastUsedAt": last_used_at,
                            }))
                        })
                        .collect::<Vec<Value>>()
                })
                .unwrap_or_default();

            ApiResponse {
                request_id: resp.request_id,
                elapsed_ms: resp.elapsed_ms,
                json: json!({ "workspaces": workspaces }),
            }
        });

        let _ = tx.send(TuiMsg::Workspaces(mapped));
    });
}

pub fn spawn_threads_fetch(
    api: ApiClient,
    tx: mpsc::UnboundedSender<TuiMsg>,
    workspace_id: Option<String>,
) {
    tokio::spawn(async move {
        let mut project_id = trim_non_empty(workspace_id);

        if project_id.is_none() {
            match api.get_json("/v1/projects", None, false).await {
                Ok(resp) => {
                    project_id = find_first_id(&resp.json, "projects");
                }
                Err(err) => {
                    let _ = tx.send(TuiMsg::Threads(Err(err)));
                    return;
                }
            }
        }

        let Some(project_id) = project_id else {
            let _ = tx.send(TuiMsg::Threads(Ok(ApiResponse {
                request_id: None,
                elapsed_ms: 0,
                json: json!({ "threads": [] }),
            })));
            return;
        };

        let path = format!("/v1/projects/{project_id}/chats");
        let res = api.get_json(&path, None, false).await;
        let mapped = res.map(|resp| {
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
                                "mode": Value::Null,
                                "lastMessageAt": updated_at,
                                "isPinned": false,
                                "_count": { "messages": message_count },
                            }))
                        })
                        .collect::<Vec<Value>>()
                })
                .unwrap_or_default();

            ApiResponse {
                request_id: resp.request_id,
                elapsed_ms: resp.elapsed_ms,
                json: json!({ "threads": threads }),
            }
        });

        let _ = tx.send(TuiMsg::Threads(mapped));
    });
}

pub fn spawn_memory_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        // Memory not yet implemented in Starbot_API
        let placeholder = json!({
            "items": []
        });
        let _ = tx.send(TuiMsg::Memory(Ok(crate::api::ApiResponse {
            request_id: None,
            elapsed_ms: 0,
            json: placeholder,
        })));
    });
}

pub fn spawn_memory_settings_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        // Memory settings not yet implemented
        let placeholder = json!({
            "enabled": false
        });
        let _ = tx.send(TuiMsg::MemorySettings(Ok(crate::api::ApiResponse {
            request_id: None,
            elapsed_ms: 0,
            json: placeholder,
        })));
    });
}

pub fn spawn_memory_toggle(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>, enabled: bool) {
    tokio::spawn(async move {
        // Memory toggle not yet implemented
        let placeholder = json!({
            "enabled": enabled
        });
        let _ = tx.send(TuiMsg::MemorySettings(Ok(crate::api::ApiResponse {
            request_id: None,
            elapsed_ms: 0,
            json: placeholder,
        })));
    });
}

pub fn spawn_tool_propose(
    api: ApiClient,
    tx: mpsc::UnboundedSender<TuiMsg>,
    tool_name: String,
    workspace_id: String,
    input: Value,
) {
    tokio::spawn(async move {
        // Tool proposal not yet implemented
        let placeholder = json!({
            "approved": false,
            "message": "Tool execution not yet implemented in Starbot_API"
        });
        let _ = tx.send(TuiMsg::Tool(tool_name, Ok(crate::api::ApiResponse {
            request_id: None,
            elapsed_ms: 0,
            json: placeholder,
        })));
    });
}

pub fn spawn_chat_request(
    api: ApiClient,
    tx: mpsc::UnboundedSender<TuiMsg>,
    _provider: String,
    _model: Option<String>,
    _messages: Vec<ChatMsg>,
    _workspace_id: Option<String>,
) {
    tokio::spawn(async move {
        // Non-streaming chat not yet fully wired - return placeholder
        // User should use streaming mode
        let placeholder = json!({
            "error": "Non-streaming mode not yet implemented. Use streaming mode."
        });
        let _ = tx.send(TuiMsg::Chat(Ok(ApiResponse {
            request_id: None,
            elapsed_ms: 0,
            json: placeholder,
        })));
    });
}

// Legacy streaming wrapper - provides old signature for compatibility
// TODO: Remove once TUI flow is refactored to use chat_id directly
pub fn spawn_chat_request_stream_legacy(
    api: ApiClient,
    tx: mpsc::UnboundedSender<TuiMsg>,
    provider: String,
    model: Option<String>,
    messages: Vec<ChatMsg>,
    workspace_id: Option<String>,
    active_thread_id: Option<String>,
    working_dir: String,
) {
    tokio::spawn(async move {
        let Some(prompt) = extract_last_user_prompt(&messages) else {
            let _ = tx.send(TuiMsg::StreamError(
                "No user message found to send.".to_string(),
            ));
            return;
        };

        let mut project_id = trim_non_empty(workspace_id);

        if project_id.is_none() {
            match api.get_json("/v1/projects", None, false).await {
                Ok(resp) => {
                    project_id = find_first_id(&resp.json, "projects");
                }
                Err(err) => {
                    let _ = tx.send(TuiMsg::StreamError(format!("Failed to list projects: {err}")));
                    return;
                }
            }
        }

        if project_id.is_none() {
            match api
                .post_json("/v1/projects", Some(json!({ "name": "Default Project" })), false)
                .await
            {
                Ok(resp) => {
                    project_id = resp
                        .json
                        .get("project")
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());
                }
                Err(err) => {
                    let _ = tx.send(TuiMsg::StreamError(format!("Failed to create project: {err}")));
                    return;
                }
            }
        }

        let Some(project_id) = project_id else {
            let _ = tx.send(TuiMsg::StreamError(
                "No project available for chat.".to_string(),
            ));
            return;
        };

        let mut chat_id = trim_non_empty(active_thread_id);
        if chat_id.is_none() {
            let path = format!("/v1/projects/{project_id}/chats");
            match api.get_json(&path, None, false).await {
                Ok(resp) => {
                    chat_id = find_first_id(&resp.json, "chats");
                }
                Err(err) => {
                    let _ = tx.send(TuiMsg::StreamError(format!("Failed to list chats: {err}")));
                    return;
                }
            }
        }

        if chat_id.is_none() {
            let path = format!("/v1/projects/{project_id}/chats");
            let title = default_chat_title(&prompt);
            match api.post_json(&path, Some(json!({ "title": title })), false).await {
                Ok(resp) => {
                    chat_id = resp
                        .json
                        .get("chat")
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());
                }
                Err(err) => {
                    let _ = tx.send(TuiMsg::StreamError(format!("Failed to create chat: {err}")));
                    return;
                }
            }
        }

        let Some(mut chat_id) = chat_id else {
            let _ = tx.send(TuiMsg::StreamError(
                "No chat available for streaming.".to_string(),
            ));
            return;
        };

        // If the active chat ID points to a deleted/missing chat, recover by creating a new chat once.
        let mut recovered_missing_chat = false;
        loop {
            let add_message_path = format!("/v1/chats/{chat_id}/messages");
            let add_message_body = json!({
                "role": "user",
                "content": prompt,
            });

            match api
                .post_json(&add_message_path, Some(add_message_body), false)
                .await
            {
                Ok(_) => break,
                Err(err) => {
                    let err_text = err.to_string();
                    if !recovered_missing_chat
                        && err_text.to_ascii_lowercase().contains("not found")
                    {
                        recovered_missing_chat = true;
                        let path = format!("/v1/projects/{project_id}/chats");
                        let title = default_chat_title(&prompt);
                        match api.post_json(&path, Some(json!({ "title": title })), false).await {
                            Ok(resp) => {
                                if let Some(new_chat_id) = resp
                                    .json
                                    .get("chat")
                                    .and_then(|v| v.get("id"))
                                    .and_then(|v| v.as_str())
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string())
                                {
                                    chat_id = new_chat_id;
                                    continue;
                                }
                                let _ = tx.send(TuiMsg::StreamError(
                                    "Failed to recover missing chat: new chat id was empty."
                                        .to_string(),
                                ));
                                return;
                            }
                            Err(create_err) => {
                                let _ = tx.send(TuiMsg::StreamError(format!(
                                    "Failed to recover missing chat: {create_err}"
                                )));
                                return;
                            }
                        }
                    }

                    let _ = tx.send(TuiMsg::StreamError(format!(
                        "Failed to add user message: {err}"
                    )));
                    return;
                }
            }
        }

        let mut run_body = json!({
            "mode": "standard",
            "speed": false,
            "auto": true,
            "client_context": {
                "working_dir": working_dir,
            }
        });
        if let Some(model_prefs) = build_model_prefs(&provider, model.as_deref()) {
            run_body["model_prefs"] = json!(model_prefs);
        }

        let run_path = format!("/v1/chats/{chat_id}/run");
        match api.post_stream(&run_path, Some(run_body), false).await {
            Ok(mut rx) => {
                let mut final_payload: Option<Value> = None;
                let mut chat_update: Option<Value> = None;

                while let Some(event) = rx.recv().await {
                    match event.event_type.as_str() {
                        "status" => {
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                if let Some(msg) = data.get("message").and_then(|v| v.as_str()) {
                                    let _ = tx.send(TuiMsg::StreamStatus(msg.to_string()));
                                }
                            }
                        }
                        "token.delta" => {
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                if let Some(text) = data.get("text").and_then(|v| v.as_str()) {
                                    let _ = tx.send(TuiMsg::StreamToken(text.to_string()));
                                }
                            }
                        }
                        "message.final" => {
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                final_payload = Some(data);
                            }
                        }
                        "chat.updated" => {
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                chat_update = Some(data);
                            }
                        }
                        "error" => {
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                if let Some(msg) = data.get("message").and_then(|v| v.as_str()) {
                                    let _ = tx.send(TuiMsg::StreamError(msg.to_string()));
                                } else {
                                    let _ = tx.send(TuiMsg::StreamError(event.data));
                                }
                            } else {
                                let _ = tx.send(TuiMsg::StreamError(event.data));
                            }
                            return;
                        }
                        _ => {}
                    }
                }

                let mut done = final_payload.unwrap_or_else(|| json!({}));
                if !done.is_object() {
                    done = json!({ "message": done });
                }
                if let Some(obj) = done.as_object_mut() {
                    obj.insert("chatId".to_string(), json!(chat_id.clone()));
                    if let Some(chat) = chat_update {
                        if let Some(id) = chat
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            obj.insert("chatId".to_string(), json!(id));
                        }
                        if let Some(title) = chat
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            obj.insert("chatTitle".to_string(), json!(title));
                        }
                        obj.insert("chat".to_string(), chat);
                    }
                }

                let _ = tx.send(TuiMsg::StreamDone(done));
            }
            Err(err) => {
                let _ = tx.send(TuiMsg::StreamError(err.to_string()));
            }
        }
    });
}

fn trim_non_empty(value: Option<String>) -> Option<String> {
    value
        .and_then(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
}

fn find_first_id(payload: &Value, collection_key: &str) -> Option<String> {
    payload
        .get(collection_key)
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items.iter().find_map(|item| {
                item.get("id")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
        })
}

fn extract_last_user_prompt(messages: &[ChatMsg]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|m| m.sendable && m.role == ChatRole::User)
        .map(|m| m.content.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn default_chat_title(prompt: &str) -> String {
    let title = prompt.chars().take(60).collect::<String>();
    let trimmed = title.trim();
    if trimmed.is_empty() {
        "New Chat".to_string()
    } else {
        trimmed.to_string()
    }
}

fn build_model_prefs(provider: &str, model: Option<&str>) -> Option<String> {
    let provider = provider.trim().to_ascii_lowercase();
    let model = model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    if provider.is_empty() || provider == "auto" {
        return model;
    }

    match model {
        Some(model_name) => Some(format!("{provider}:{model_name}")),
        None => Some(provider),
    }
}

pub fn spawn_completion_request(
    api: ApiClient,
    tx: mpsc::UnboundedSender<TuiMsg>,
    file_path: String,
    content: String,
    cursor_pos: (usize, usize),
) {
    tokio::spawn(async move {
        let body = json!({
            "file_path": file_path,
            "content": content,
            "cursor_position": {
                "line": cursor_pos.0,
                "column": cursor_pos.1,
            },
            "max_suggestions": 3,
        });

        let res = api.post_json("/v1/completion", Some(body), false).await;
        let _ = tx.send(TuiMsg::CompletionRequest(file_path, res));
    });
}

pub fn spawn_file_list_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>, workspace_id: String, path: String) {
    tokio::spawn(async move {
        let workspace_id_clone = workspace_id.clone();
        let path_clone = path.clone();
        let query = vec![
            ("workspace_id".to_string(), workspace_id),
            ("path".to_string(), path),
        ];
        let res = api.get_json("/v1/files/list", Some(&query), false).await;
        let _ = tx.send(TuiMsg::FileListRequest(workspace_id_clone, path_clone, res));
    });
}
