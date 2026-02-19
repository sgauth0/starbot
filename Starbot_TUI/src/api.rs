use std::time::{Duration, Instant};

use reqwest::{Client, Method, StatusCode};
use serde_json::{Value, json};
use tokio::time::sleep;
use tokio::sync::mpsc;
use futures::StreamExt;

use crate::errors::{CliError, redact_secret, with_debug_hint};

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
    token: Option<String>,
    retries: u32,
    debug: bool,
}

#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub request_id: Option<String>,
    pub elapsed_ms: u128,
    pub json: Value,
}

impl ApiClient {
    pub fn new(
        base_url: String,
        token: Option<String>,
        timeout_ms: u64,
        retries: u32,
        debug: bool,
    ) -> Result<Self, CliError> {
        let timeout = Duration::from_millis(timeout_ms.max(1));
        let client = Client::builder().timeout(timeout).build()?;
        Ok(Self {
            client,
            base_url,
            token,
            retries,
            debug,
        })
    }

    pub async fn get_json(
        &self,
        path: &str,
        query: Option<&[(String, String)]>,
        auth_required: bool,
    ) -> Result<ApiResponse, CliError> {
        self.request_json(Method::GET, path, query, None, auth_required, true)
            .await
    }

    pub async fn post_json(
        &self,
        path: &str,
        body: Option<Value>,
        auth_required: bool,
    ) -> Result<ApiResponse, CliError> {
        self.request_json(Method::POST, path, None, body, auth_required, false)
            .await
    }

    pub async fn post_json_with_body(
        &self,
        path: &str,
        body: &Value,
        auth_required: bool,
    ) -> Result<ApiResponse, CliError> {
        self.request_json(Method::POST, path, None, Some(body.clone()), auth_required, false)
            .await
    }

    pub async fn request_json(
        &self,
        method: Method,
        path: &str,
        query: Option<&[(String, String)]>,
        body: Option<Value>,
        auth_required: bool,
        idempotent: bool,
    ) -> Result<ApiResponse, CliError> {
        let token = if auth_required {
            Some(self.token.clone().ok_or_else(|| {
                CliError::Auth("Missing token. Run `starbott auth login` first.".to_string())
            })?)
        } else {
            self.token.clone()
        };

        let url = join_url(&self.base_url, path);
        let max_attempts = if idempotent {
            self.retries.saturating_add(1)
        } else {
            1
        };

        for attempt in 0..max_attempts {
            let started = Instant::now();
            let mut request = self.client.request(method.clone(), url.clone());

            if let Some(query_items) = query {
                request = request.query(query_items);
            }

            if let Some(ref bearer) = token {
                request = request.bearer_auth(bearer);
            }

            if let Some(ref payload) = body {
                request = request.json(payload);
            }

            let response = request.send().await;
            match response {
                Ok(resp) => {
                    let status = resp.status();
                    let request_id = resp
                        .headers()
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());

                    let text = resp.text().await.unwrap_or_default();
                    if is_retryable_status(status) && idempotent && attempt + 1 < max_attempts {
                        sleep(backoff_delay_ms(attempt)).await;
                        continue;
                    }

                    let parsed = if text.trim().is_empty() {
                        json!({})
                    } else {
                        serde_json::from_str::<Value>(&text)
                            .unwrap_or_else(|_| json!({ "raw": text }))
                    };

                    if status.is_success() {
                        return Ok(ApiResponse {
                            request_id,
                            elapsed_ms: started.elapsed().as_millis(),
                            json: parsed,
                        });
                    }

                    return Err(self.http_error(status, request_id, parsed));
                }
                Err(err) => {
                    let transient = err.is_timeout() || err.is_connect() || err.is_request();
                    if transient && idempotent && attempt + 1 < max_attempts {
                        sleep(backoff_delay_ms(attempt)).await;
                        continue;
                    }

                    let message = if err.is_timeout() {
                        "Request timed out.".to_string()
                    } else {
                        format!("Network request failed: {err}")
                    };
                    return Err(CliError::Network(with_debug_hint(&message, self.debug)));
                }
            }
        }

        Err(CliError::Network(with_debug_hint(
            "Request failed after retries.",
            self.debug,
        )))
    }

    fn http_error(
        &self,
        status: StatusCode,
        request_id: Option<String>,
        payload: Value,
    ) -> CliError {
        let message = payload
            .get("error")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("message").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Request failed with status {}", status.as_u16()));

        let mut details = message;
        if let Some(id) = request_id {
            details.push_str(&format!(" (request_id: {id})"));
        }
        if self.debug {
            let mut payload_text = payload.to_string();
            if let Some(token) = &self.token {
                payload_text = payload_text.replace(token, &redact_secret(token));
            }
            details.push_str(&format!(" payload={payload_text}"));
        } else {
            details = with_debug_hint(&details, false);
        }

        match status.as_u16() {
            400 => CliError::Usage(details),
            401 | 403 => CliError::Auth(details),
            429 => CliError::RateLimited(details),
            500..=599 => CliError::Server(details),
            _ => CliError::Generic(details),
        }
    }

    pub async fn put_json(&self, path: &str, body: Option<Value>, auth_required: bool) -> Result<ApiResponse, CliError> {
        self.request_json(Method::PUT, path, None, body, auth_required, true)
            .await
    }

    pub async fn patch_json(&self, path: &str, body: Option<Value>, auth_required: bool) -> Result<ApiResponse, CliError> {
        self.request_json(Method::PATCH, path, None, body, auth_required, false)
            .await
    }

    pub async fn delete_json(&self, path: &str, body: Option<Value>, auth_required: bool) -> Result<ApiResponse, CliError> {
        self.request_json(Method::DELETE, path, None, body, auth_required, true)
            .await
    }

    /// Start a streaming POST request (SSE format)
    /// Returns a receiver channel that yields (event_type, data) tuples
    pub async fn post_stream(
        &self,
        path: &str,
        body: Option<Value>,
        auth_required: bool,
    ) -> Result<mpsc::UnboundedReceiver<StreamEvent>, CliError> {
        let token = if auth_required {
            Some(self.token.clone().ok_or_else(|| {
                CliError::Auth("Missing token. Run `starbott auth login` first.".to_string())
            })?)
        } else {
            self.token.clone()
        };

        let url = join_url(&self.base_url, path);
        let mut request = self.client.post(url);

        if let Some(token_str) = token {
            request = request.header("Authorization", format!("Bearer {}", token_str));
        }

        if let Some(body_val) = body {
            request = request.json(&body_val);
        }

        let response = request.send().await.map_err(|e| {
            CliError::Network(format!("Stream request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
            return Err(self.http_error(status, None, payload));
        }

        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn a task to process the stream
        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut current_event = String::new();
            let mut current_data_lines: Vec<String> = Vec::new();

            let flush_event = |event_type: &mut String, data_lines: &mut Vec<String>| {
                if data_lines.is_empty() {
                    event_type.clear();
                    return;
                }
                let kind = if event_type.trim().is_empty() {
                    "message".to_string()
                } else {
                    event_type.trim().to_string()
                };
                let data = data_lines.join("\n");
                let _ = tx.send(StreamEvent {
                    event_type: kind,
                    data,
                });
                data_lines.clear();
                event_type.clear();
            };

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Ok(text) = std::str::from_utf8(&chunk) {
                            buffer.push_str(text);

                            // Process complete lines
                            while let Some(newline_pos) = buffer.find('\n') {
                                let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                                buffer = buffer[newline_pos + 1..].to_string();

                                if line.is_empty() {
                                    // Empty line signals end of event.
                                    flush_event(&mut current_event, &mut current_data_lines);
                                } else if let Some(event_type) = line.strip_prefix("event:") {
                                    current_event = event_type.trim().to_string();
                                } else if let Some(data) = line.strip_prefix("data:") {
                                    current_data_lines.push(data.trim_start().to_string());
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }

            // Flush trailing buffered event if the server closed without a blank line.
            flush_event(&mut current_event, &mut current_data_lines);
        });

        Ok(rx)
    }

    // Task management operations
    pub async fn list_tasks(&self, status: Option<String>, limit: i32) -> Result<Vec<Task>, CliError> {
        let mut query = Vec::new();
        if let Some(s) = status {
            query.push(("status".to_string(), s));
        }
        if limit > 0 {
            query.push(("limit".to_string(), limit.to_string()));
        }

        let res = self.get_json("/v1/tasks", Some(&query), true).await?;
        let tasks = serde_json::from_value::<Vec<Task>>(res.json.get("data").cloned().unwrap_or(json!([])))
            .map_err(|e| CliError::Generic(format!("Failed to parse tasks: {}", e)))?;
        Ok(tasks)
    }

    pub async fn get_task(&self, task_id: &str) -> Result<Task, CliError> {
        let res = self.get_json(&format!("/v1/tasks/{}", task_id), None, true).await?;
        let task = serde_json::from_value::<Task>(res.json.get("task").cloned().unwrap_or(json!({})))
            .map_err(|e| CliError::Generic(format!("Failed to parse task: {}", e)))?;
        Ok(task)
    }

    pub async fn create_task(&self, task: CreateTaskRequest) -> Result<Task, CliError> {
        let body = serde_json::to_value(task).map_err(|e| CliError::Generic(format!("Failed to serialize task: {}", e)))?;
        let res = self.post_json("/v1/tasks", Some(body), true).await?;
        let created_task = serde_json::from_value::<Task>(res.json.get("task").cloned().unwrap_or(json!({})))
            .map_err(|e| CliError::Generic(format!("Failed to parse created task: {}", e)))?;
        Ok(created_task)
    }

    pub async fn update_task(&self, task_id: String, task: UpdateTaskRequest) -> Result<Task, CliError> {
        let body = serde_json::to_value(task).map_err(|e| CliError::Generic(format!("Failed to serialize task: {}", e)))?;
        let res = self.put_json(&format!("/v1/tasks/{}", task_id), Some(body), true).await?;
        let updated_task = serde_json::from_value::<Task>(res.json.get("task").cloned().unwrap_or(json!({})))
            .map_err(|e| CliError::Generic(format!("Failed to parse updated task: {}", e)))?;
        Ok(updated_task)
    }

    pub async fn complete_task(&self, task_id: &str) -> Result<Task, CliError> {
        let res = self.post_json(&format!("/v1/tasks/{}/complete", task_id), None, true).await?;
        let completed_task = serde_json::from_value::<Task>(res.json.get("task").cloned().unwrap_or(json!({})))
            .map_err(|e| CliError::Generic(format!("Failed to parse completed task: {}", e)))?;
        Ok(completed_task)
    }

    pub async fn delete_task(&self, task_id: &str) -> Result<(), CliError> {
        self.delete_json(&format!("/v1/tasks/{}", task_id), None, true).await?;
        Ok(())
    }

    pub async fn start_task(&self, task_id: &str) -> Result<Task, CliError> {
        let res = self.post_json(&format!("/v1/tasks/{}/start", task_id), None, true).await?;
        let started_task = serde_json::from_value::<Task>(res.json.get("task").cloned().unwrap_or(json!({})))
            .map_err(|e| CliError::Generic(format!("Failed to parse started task: {}", e)))?;
        Ok(started_task)
    }

    /// Chat operations
    pub async fn create_chat(&self, request: CreateChatRequest) -> Result<ChatResponse, CliError> {
        let body = serde_json::to_value(request).map_err(|e| CliError::Generic(format!("Failed to serialize chat request: {}", e)))?;
        let res = self.post_json("/v1/inference/chat", Some(body), true).await?;

        let chat_response = serde_json::from_value::<ChatResponse>(res.json)
            .map_err(|e| CliError::Generic(format!("Failed to parse chat response: {}", e)))?;
        Ok(chat_response)
    }
}

// Additional data structures
#[derive(Debug, serde::Serialize)]
pub struct CreateChatRequest {
    pub project_id: Option<String>,
    pub workspace_id: Option<String>,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ChatResponse {
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// Task data structures
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: i32,
    pub due_date: Option<String>,
    pub estimated_hours: Option<i32>,
    pub actual_hours: Option<i32>,
    pub parent_id: Option<String>,
    pub chat_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub chat_id: Option<String>,
    pub parent_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct StreamEvent {
    pub event_type: String,
    pub data: String,
}

fn join_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }

    let trimmed_path = path.trim_start_matches('/');
    let mut base = base_url.trim_end_matches('/').to_string();

    // Guard against configs that include a "/v1" suffix while callers also pass "/v1/*" paths.
    if base.ends_with("/v1") && trimmed_path.starts_with("v1/") {
        base.truncate(base.len().saturating_sub(3));
        base = base.trim_end_matches('/').to_string();
    }

    format!("{base}/{trimmed_path}")
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn backoff_delay_ms(attempt: u32) -> Duration {
    let pow = attempt.min(6);
    let factor = 1u64 << pow;
    Duration::from_millis(200 * factor)
}
