// PHASE 3: Async operation handlers extracted from tui.rs
// Contains spawn_* functions for background API requests

use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::tui::types::{TuiMsg, ChatMsg};

pub fn spawn_models_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        let res = api.get_json("/v1/models", None, false).await;
        let _ = tx.send(TuiMsg::Models(res));
    });
}

pub fn spawn_health_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        let res = api.get_json("/health", None, false).await;
        let _ = tx.send(TuiMsg::Health(res));
    });
}

pub fn spawn_workspaces_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        let res = api.get_json("/v1/workspaces", None, true).await;
        let _ = tx.send(TuiMsg::Workspaces(res));
    });
}

pub fn spawn_threads_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        let res = api.get_json("/v1/threads", None, true).await;
        let _ = tx.send(TuiMsg::Threads(res));
    });
}

pub fn spawn_memory_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        let res = api.get_json("/v1/memory?limit=50", None, true).await;
        let _ = tx.send(TuiMsg::Memory(res));
    });
}

pub fn spawn_memory_settings_fetch(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>) {
    tokio::spawn(async move {
        let res = api.get_json("/v1/memory/settings", None, true).await;
        let _ = tx.send(TuiMsg::MemorySettings(res));
    });
}

pub fn spawn_memory_toggle(api: ApiClient, tx: mpsc::UnboundedSender<TuiMsg>, enabled: bool) {
    tokio::spawn(async move {
        let body = json!({ "enabled": enabled });
        let res = api.post_json("/v1/memory/settings", Some(body), true).await;
        let _ = tx.send(TuiMsg::MemorySettings(res));
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
        let body = json!({
            "workspaceId": workspace_id,
            "toolName": tool_name,
            "input": input,
        });
        let tool = body
            .get("toolName")
            .and_then(|v| v.as_str())
            .unwrap_or("tool")
            .to_string();
        let res = api.post_json("/v1/tools/propose", Some(body), true).await;
        let _ = tx.send(TuiMsg::Tool(tool, res));
    });
}

pub fn spawn_chat_request(
    api: ApiClient,
    tx: mpsc::UnboundedSender<TuiMsg>,
    provider: String,
    model: Option<String>,
    messages: Vec<ChatMsg>,
    workspace_id: Option<String>,
) {
    tokio::spawn(async move {
        let body = build_chat_body(&provider, model.as_deref(), &messages, workspace_id.as_deref());
        let res = api.post_json("/v1/inference/chat", Some(body), true).await;
        let _ = tx.send(TuiMsg::Chat(res));
    });
}

/// Streaming version of chat request (SSE)
pub fn spawn_chat_request_stream(
    api: ApiClient,
    tx: mpsc::UnboundedSender<TuiMsg>,
    provider: String,
    model: Option<String>,
    messages: Vec<ChatMsg>,
    workspace_id: Option<String>,
) {
    tokio::spawn(async move {
        let body = build_chat_body(&provider, model.as_deref(), &messages, workspace_id.as_deref());

        match api.post_stream("/v1/inference/chat/stream", Some(body), true).await {
            Ok(mut rx) => {
                let mut final_metadata = json!({});

                while let Some(event) = rx.recv().await {
                    match event.event_type.as_str() {
                        "token" => {
                            // Parse token data
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                if let Some(text) = data.get("text").and_then(|v| v.as_str()) {
                                    let _ = tx.send(TuiMsg::StreamToken(text.to_string()));
                                }
                            }
                        }
                        "done" => {
                            // Parse final metadata
                            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                                final_metadata = data;
                            }
                        }
                        _ => {
                            // Ignore other events for now (job_state, tool_proposal, etc.)
                        }
                    }
                }

                // Send final done message
                let _ = tx.send(TuiMsg::StreamDone(final_metadata));
            }
            Err(e) => {
                let _ = tx.send(TuiMsg::StreamError(e.to_string()));
            }
        }
    });
}

fn build_chat_body(
    provider: &str,
    model: Option<&str>,
    messages: &[ChatMsg],
    workspace_id: Option<&str>,
) -> Value {
    let mut body = json!({
        // Only send actual conversation turns. Local UI/system messages (errors, hints) should not
        // be fed back into the model.
        "messages": messages
            .iter()
            .filter(|m| m.sendable)
            .map(|m| json!({"role": m.role.as_str(), "content": m.content}))
            .collect::<Vec<_>>(),
        "client": "cli",
        "provider": provider,
        "toolsEnabled": true,
    });
    if let Some(m) = model.map(str::trim).filter(|s| !s.is_empty()) {
        body["model"] = json!(m);
    }
    if let Some(ws) = workspace_id.map(str::trim).filter(|s| !s.is_empty()) {
        body["workspaceId"] = json!(ws);
    }
    body
}
