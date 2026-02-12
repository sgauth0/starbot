// PHASE 3: Shared response parsing functions
// Extracted from tui.rs and chat.rs to eliminate 60 lines of duplication

use serde_json::Value;

/// Extract the reply text from an API response
pub fn extract_reply(payload: &Value) -> Option<String> {
    if let Some(reply) = payload.get("reply").and_then(|v| v.as_str()) {
        return Some(reply.to_string());
    }
    payload
        .get("message")
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extract provider and model names from an API response
pub fn extract_provider_model(payload: &Value) -> (Option<String>, Option<String>) {
    let provider = payload
        .get("provider")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            payload
                .get("message")
                .and_then(|v| v.get("provider"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });
    let model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            payload
                .get("message")
                .and_then(|v| v.get("model"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });
    (provider, model)
}

/// Extract usage statistics and format as a display string
pub fn extract_usage_line(payload: &Value) -> String {
    if let Some(usage) = payload.get("usage") {
        let input = usage
            .get("inputTokens")
            .or_else(|| usage.get("input_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output = usage
            .get("outputTokens")
            .or_else(|| usage.get("output_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total = usage
            .get("totalTokens")
            .or_else(|| usage.get("total_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(input + output);
        return format!("usage(input={input}, output={output}, total={total})");
    }
    "usage(unknown)".to_string()
}
