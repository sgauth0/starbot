use std::io::{self, Read};

use clap::Args;
use serde_json::{Value, json};

use crate::app::Runtime;
use crate::errors::CliError;
use crate::parse::response::{extract_reply, extract_provider_model, extract_usage_line};

#[derive(Debug, Args)]
pub struct ChatArgs {
    /// Prompt text
    pub prompt: Option<String>,
    /// Model or provider selector. Examples: "azure:gpt-5.2-chat" or "auto"
    #[arg(short = 'm', long = "model")]
    pub model: Option<String>,
    /// Optional conversation id
    #[arg(short = 'c', long = "conversation")]
    pub conversation: Option<String>,
    /// Read prompt from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Request streaming response (falls back to non-streaming if unsupported)
    #[arg(long)]
    pub stream: bool,
    /// Optional max output tokens passthrough
    #[arg(long = "max-tokens")]
    pub max_tokens: Option<u32>,
}

pub async fn handle(runtime: &Runtime, args: ChatArgs) -> Result<(), CliError> {
    let prompt = resolve_prompt(&args)?;
    let api = runtime.api_client()?;

    // Check if user wants to create a task
    if prompt.to_lowercase().contains("create task") || prompt.to_lowercase().contains("make task") {
        let task_title = extract_task_title(&prompt);
        let task_description = extract_task_description(&prompt);

        let task_data = json!({
            "title": task_title,
            "description": task_description,
            "priority": extract_task_priority(&prompt),
        });

        let res = api.post_json("/v1/tasks", Some(task_data), true).await?;

        if runtime.output.json {
            runtime.output.print_json(&res.json)?;
        } else {
            let task_id = res.json.get("task")
                .and_then(|t| t.get("id"))
                .and_then(|i| i.as_str())
                .unwrap_or("unknown");
            let title = res.json.get("task")
                .and_then(|t| t.get("title"))
                .and_then(|t| t.as_str())
                .unwrap_or("Untitled");
            runtime.output.print_human(&format!("✓ Created task: {} (ID: {})", title, task_id));
        }

        return Ok(());
    }

    if args.stream {
        runtime.output.print_verbose(
            "Requested --stream. Using standard response (CLI streaming output is not implemented yet).",
        );
    }

    let mut body = json!({
        "messages": [
            { "role": "user", "content": prompt }
        ],
        "client": "cli",
        "provider": "auto"
    });

    if let Some(conversation_id) = &args.conversation {
        body["conversationId"] = json!(conversation_id);
    }
    if let Some(max_tokens) = args.max_tokens {
        body["max_tokens"] = json!(max_tokens);
    }

    if let Some(model_or_provider) = &args.model {
        apply_model_selector(&mut body, model_or_provider);
    }

    let res = api.post_json("/v1/inference/chat", Some(body), true).await?;
    runtime.output.print_verbose(&format!(
        "request_id={:?} elapsed_ms={}",
        res.request_id, res.elapsed_ms
    ));

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }

    let reply = extract_reply(&res.json).unwrap_or_else(|| "(No text response)".to_string());
    runtime.output.print_human(&reply);

    if runtime.output.verbose {
        let (provider, model) = extract_provider_model(&res.json);
        let usage = extract_usage_line(&res.json);
        runtime.output.print_stderr(&format!(
            "provider={} model={} {}",
            provider.unwrap_or_else(|| "-".to_string()),
            model.unwrap_or_else(|| "-".to_string()),
            usage
        ));
    }

    Ok(())
}

/// Extract task title from prompt
fn extract_task_title(prompt: &str) -> String {
    // Simple extraction - look for quotes or use first reasonable phrase
    if let Some(start) = prompt.find('"') {
        if let Some(end) = prompt[start + 1..].find('"') {
            return prompt[start + 1..start + 1 + end].to_string();
        }
    }

    // Fallback: use the first few words after "create task" or "make task"
    let words: Vec<&str> = prompt.split_whitespace().collect();
    if let Some(idx) = words.iter().position(|w| w.to_lowercase() == "task") {
        if idx + 1 < words.len() {
            return words[idx + 1..].join(" ").trim_matches('.').to_string();
        }
    }

    "Untitled Task".to_string()
}

/// Extract task description from prompt
fn extract_task_description(prompt: &str) -> Option<String> {
    // Look for description or details in the prompt
    if let Some(desc_start) = prompt.to_lowercase().find("description:") {
        let desc = prompt[desc_start + 12..].trim();
        if !desc.is_empty() {
            return Some(desc.to_string());
        }
    }

    // Look for anything after the task title
    let title = extract_task_title(prompt);
    if let Some(title_pos) = prompt.find(&title) {
        if title_pos + title.len() < prompt.len() {
            let desc = prompt[title_pos + title.len()..].trim();
            if !desc.is_empty() && !desc.to_lowercase().contains("priority:") {
                return Some(desc.to_string());
            }
        }
    }

    None
}

/// Extract task priority from prompt
fn extract_task_priority(prompt: &str) -> i32 {
    if prompt.to_lowercase().contains("priority: high") || prompt.to_lowercase().contains("high priority") {
        8
    } else if prompt.to_lowercase().contains("priority: low") || prompt.to_lowercase().contains("low priority") {
        2
    } else if prompt.to_lowercase().contains("priority: medium") || prompt.to_lowercase().contains("medium priority") {
        5
    } else {
        0
    }
}

fn resolve_prompt(args: &ChatArgs) -> Result<String, CliError> {
    if args.stdin {
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .map_err(|e| CliError::Generic(format!("Failed reading stdin: {e}")))?;
        let trimmed = input.trim().to_string();
        if trimmed.is_empty() {
            return Err(CliError::Usage(
                "No prompt provided via stdin. Pipe text or pass a prompt argument.".to_string(),
            ));
        }
        return Ok(trimmed);
    }

    match &args.prompt {
        Some(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(CliError::Usage(
            "Missing prompt. Use `starbott chat \"...\"` or pass `--stdin`.".to_string(),
        )),
    }
}

fn apply_model_selector(body: &mut Value, selector: &str) {
    let trimmed = selector.trim();
    if trimmed.is_empty() {
        return;
    }

    if let Some((provider, model)) = trimmed.split_once(':') {
        if !provider.trim().is_empty() {
            body["provider"] = json!(provider.trim());
        }
        if !model.trim().is_empty() {
            body["model"] = json!(model.trim());
        }
        return;
    }

    let lower = trimmed.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "auto" | "kimi" | "gemini" | "vertex" | "cloudflare" | "azure" | "openai"
    ) {
        body["provider"] = json!(lower);
    } else {
        body["model"] = json!(trimmed);
    }
}

// PHASE 3: Functions moved to crate::parse::response module
// (Removed 60 lines of duplicate code)
