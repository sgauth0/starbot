// Async operation handlers for Starbot_TUI
// Calls the local Starbot_API (localhost:3737)

use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::tui::types::{TuiMsg, ChatMsg};

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
        // Models endpoint not yet implemented in Starbot_API
        // Return placeholder
        let placeholder = json!({
            "models": []
        });
        let _ = tx.send(TuiMsg::Models(Ok(crate::api::ApiResponse {
            request_id: None,
            elapsed_ms: 0,
            json: placeholder,
        })));
    });
}

pub fn spawn_workspaces_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    // Renamed to projects
    spawn_projects_fetch(api, tx);
}

pub fn spawn_threads_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        // Threads are now chats - need project_id to fetch
        // Return placeholder for now
        let placeholder = json!({
            "chats": []
        });
        let _ = tx.send(TuiMsg::Threads(Ok(crate::api::ApiResponse {
            request_id: None,
            elapsed_ms: 0,
            json: placeholder,
        })));
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
        let _ = tx.send(TuiMsg::Chat(Ok(crate::api::ApiResponse {
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
    _provider: String,
    _model: Option<String>,
    _messages: Vec<ChatMsg>,
    _workspace_id: Option<String>,
) {
    tokio::spawn(async move {
        // Placeholder for now - requires full TUI refactor to:
        // 1. Get/create project
        // 2. Get/create chat in project
        // 3. Add user message to chat
        // 4. Call spawn_chat_request_stream with chat_id
        let placeholder = json!({
            "content": "Starbot_TUI needs refactoring to work with new API.\n\nNew flow required:\n1. Select/create project\n2. Select/create chat\n3. Send message to chat\n4. Stream response\n\nThis requires updating the key handler to manage chat_id state."
        });
        let _ = tx.send(TuiMsg::StreamDone(placeholder));
    });
}
