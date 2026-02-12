use std::io::{self, Read};
use std::path::PathBuf;

use clap::Subcommand;
use serde_json::{Value, json};

use crate::app::Runtime;
use crate::errors::CliError;

#[derive(Debug, Subcommand)]
pub enum ToolsCommand {
    /// Propose a tool run (safe tools execute immediately; confirm tools require approval + commit).
    Propose {
        /// Workspace id
        #[arg(long)]
        workspace_id: String,
        /// Tool name (example: file.search, file.read, file.patch, web.search)
        #[arg(long)]
        tool_name: String,
        /// Tool input as JSON string
        #[arg(long)]
        input: Option<String>,
        /// Read tool input JSON from a file
        #[arg(long)]
        input_file: Option<PathBuf>,
        /// Read tool input JSON from stdin
        #[arg(long)]
        stdin: bool,
        /// Auto-approve confirm tools (skips prompt)
        #[arg(long)]
        yes: bool,
        /// Reason to include when denying a proposal
        #[arg(long)]
        deny_reason: Option<String>,
    },
    /// Commit an approved proposal.
    Commit {
        #[arg(long)]
        proposal_id: String,
    },
    /// Deny a pending proposal.
    Deny {
        #[arg(long)]
        proposal_id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// List your tool runs.
    Runs {
        #[arg(long)]
        workspace_id: Option<String>,
        #[arg(long)]
        tool_name: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
}

pub async fn handle(runtime: &Runtime, command: ToolsCommand) -> Result<(), CliError> {
    match command {
        ToolsCommand::Propose {
            workspace_id,
            tool_name,
            input,
            input_file,
            stdin,
            yes,
            deny_reason,
        } => propose(runtime, workspace_id, tool_name, input, input_file, stdin, yes, deny_reason).await,
        ToolsCommand::Commit { proposal_id } => commit(runtime, proposal_id).await,
        ToolsCommand::Deny { proposal_id, reason } => deny(runtime, proposal_id, reason).await,
        ToolsCommand::Runs {
            workspace_id,
            tool_name,
            limit,
        } => list_runs(runtime, workspace_id, tool_name, limit).await,
    }
}

async fn propose(
    runtime: &Runtime,
    workspace_id: String,
    tool_name: String,
    input: Option<String>,
    input_file: Option<PathBuf>,
    stdin: bool,
    yes: bool,
    deny_reason: Option<String>,
) -> Result<(), CliError> {
    let api = runtime.api_client()?;

    let tool_name = tool_name.trim().to_string();
    if tool_name.is_empty() {
        return Err(CliError::Usage("--tool-name must be non-empty.".to_string()));
    }

    let input_json = read_input_json(input, input_file, stdin)?;

    let body = json!({
        "workspaceId": workspace_id.trim(),
        "toolName": tool_name,
        "input": input_json,
    });

    let res = api.post_json("/v1/tools/propose", Some(body), true).await?;
    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }

    let requires = res
        .json
        .get("requiresConfirmation")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !requires {
        let run_id = res.json.get("runId").and_then(|v| v.as_str()).unwrap_or("-");
        runtime
            .output
            .print_human(&format!("tool completed: runId={run_id}"));

        if let Some(result) = res.json.get("result") {
            runtime.output.print_human(&format_json(result)?);
        }
        return Ok(());
    }

    let proposal_id = res
        .json
        .get("proposalId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if proposal_id.is_empty() {
        return Err(CliError::Server("Missing proposalId in response".to_string()));
    }

    let expires_at = res.json.get("expiresAt").and_then(|v| v.as_str()).unwrap_or("-");
    runtime
        .output
        .print_human(&format!("tool requires approval: proposalId={proposal_id} expiresAt={expires_at}"));

    if let Some(preview) = res.json.get("preview") {
        runtime.output.print_human(&format_json(preview)?);
    }

    let approve = if yes {
        true
    } else {
        prompt_approve()?
    };

    if approve {
        let commit_res = api
            .post_json("/v1/tools/commit", Some(json!({ "proposalId": proposal_id })), true)
            .await?;
        runtime.output.print_human("committed.");
        runtime.output.print_human(&format_json(&commit_res.json)?);
        return Ok(());
    }

    let deny_body = if let Some(reason) = deny_reason {
        json!({ "proposalId": proposal_id, "reason": reason })
    } else {
        json!({ "proposalId": proposal_id })
    };
    let deny_res = api.post_json("/v1/tools/deny", Some(deny_body), true).await?;
    runtime.output.print_human("denied.");
    runtime.output.print_human(&format_json(&deny_res.json)?);
    Ok(())
}

async fn commit(runtime: &Runtime, proposal_id: String) -> Result<(), CliError> {
    let api = runtime.api_client()?;
    let pid = proposal_id.trim();
    if pid.is_empty() {
        return Err(CliError::Usage("--proposal-id is required.".to_string()));
    }

    let res = api
        .post_json("/v1/tools/commit", Some(json!({ "proposalId": pid })), true)
        .await?;

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }

    runtime.output.print_human(&format_json(&res.json)?);
    Ok(())
}

async fn deny(runtime: &Runtime, proposal_id: String, reason: Option<String>) -> Result<(), CliError> {
    let api = runtime.api_client()?;
    let pid = proposal_id.trim();
    if pid.is_empty() {
        return Err(CliError::Usage("--proposal-id is required.".to_string()));
    }

    let body = if let Some(r) = reason.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        json!({ "proposalId": pid, "reason": r })
    } else {
        json!({ "proposalId": pid })
    };

    let res = api.post_json("/v1/tools/deny", Some(body), true).await?;
    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }
    runtime.output.print_human(&format_json(&res.json)?);
    Ok(())
}

async fn list_runs(
    runtime: &Runtime,
    workspace_id: Option<String>,
    tool_name: Option<String>,
    limit: Option<u32>,
) -> Result<(), CliError> {
    let api = runtime.api_client()?;

    let mut query: Vec<(String, String)> = Vec::new();
    if let Some(ws) = workspace_id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        query.push(("workspaceId".to_string(), ws));
    }
    if let Some(t) = tool_name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        query.push(("tool".to_string(), t));
    }
    if let Some(l) = limit {
        query.push(("limit".to_string(), l.to_string()));
    }

    let query_ref = if query.is_empty() { None } else { Some(query.as_slice()) };
    let res = api.get_json("/v1/tools/runs", query_ref, true).await?;

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }

    let runs = res.json.get("runs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if runs.is_empty() {
        runtime.output.print_human("No tool runs.");
        return Ok(());
    }

    for r in runs {
        let tool = r.get("toolName").and_then(|v| v.as_str()).unwrap_or("-");
        let status = r.get("status").and_then(|v| v.as_str()).unwrap_or("-");
        let created = r.get("createdAt").and_then(|v| v.as_str()).unwrap_or("-");
        let id = r.get("id").and_then(|v| v.as_str()).unwrap_or("-");
        runtime.output.print_human(&format!("- {created} {tool} {status} ({id})"));
    }

    Ok(())
}

fn read_input_json(input: Option<String>, input_file: Option<PathBuf>, stdin: bool) -> Result<Value, CliError> {
    let sources = [
        input.as_ref().map(|_| "input"),
        input_file.as_ref().map(|_| "input_file"),
        if stdin { Some("stdin") } else { None },
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if sources.len() > 1 {
        return Err(CliError::Usage(
            "Pass only one of --input, --input-file, or --stdin.".to_string(),
        ));
    }

    let text = if let Some(raw) = input {
        raw
    } else if let Some(path) = input_file {
        std::fs::read_to_string(&path).map_err(|e| {
            CliError::Usage(format!("Failed to read {}: {e}", path.display()))
        })?
    } else if stdin {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| CliError::Generic(format!("Failed reading stdin: {e}")))?;
        buf
    } else {
        "{}".to_string()
    };

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }

    let parsed = serde_json::from_str::<Value>(trimmed).map_err(|e| {
        CliError::Usage(format!("Invalid JSON input: {e}"))
    })?;

    if !parsed.is_object() {
        return Err(CliError::Usage("Tool input must be a JSON object.".to_string()));
    }

    Ok(parsed)
}

fn prompt_approve() -> Result<bool, CliError> {
    eprint!("Approve? [y/N] ");
    let _ = io::Write::flush(&mut io::stderr());
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|e| CliError::Generic(format!("Failed reading input: {e}")))?;
    let s = line.trim().to_ascii_lowercase();
    Ok(s == "y" || s == "yes")
}

fn format_json(value: &Value) -> Result<String, CliError> {
    Ok(serde_json::to_string_pretty(value)?)
}

