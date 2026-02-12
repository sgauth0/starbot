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
