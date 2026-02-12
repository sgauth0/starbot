use std::path::PathBuf;

use clap::Subcommand;
use serde_json::{Value, json};

use crate::app::Runtime;
use crate::errors::CliError;

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// Create a workspace rooted at an absolute directory path.
    Create {
        /// Workspace name (defaults to the directory name)
        #[arg(long)]
        name: Option<String>,
        /// Absolute path to workspace root (defaults to current directory)
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// List workspaces visible to the current user.
    List,
    /// Set workspace permissions (owner-only).
    Permissions {
        /// Workspace id
        workspace_id: String,
        /// Target user id (defaults to self)
        #[arg(long)]
        user_id: Option<String>,
        #[arg(long, value_parser = clap::value_parser!(bool))]
        can_read_files: Option<bool>,
        #[arg(long, value_parser = clap::value_parser!(bool))]
        can_write_files: Option<bool>,
        #[arg(long, value_parser = clap::value_parser!(bool))]
        can_read_images: Option<bool>,
        #[arg(long, value_parser = clap::value_parser!(bool))]
        can_write_images: Option<bool>,
        #[arg(long, value_parser = clap::value_parser!(bool))]
        can_web_search: Option<bool>,
    },
}

pub async fn handle(runtime: &Runtime, command: WorkspaceCommand) -> Result<(), CliError> {
    match command {
        WorkspaceCommand::Create { name, root } => create(runtime, name, root).await,
        WorkspaceCommand::List => list(runtime).await,
        WorkspaceCommand::Permissions {
            workspace_id,
            user_id,
            can_read_files,
            can_write_files,
            can_read_images,
            can_write_images,
            can_web_search,
        } => {
            set_permissions(
                runtime,
                workspace_id,
                user_id,
                can_read_files,
                can_write_files,
                can_read_images,
                can_write_images,
                can_web_search,
            )
            .await
        }
    }
}

async fn create(runtime: &Runtime, name: Option<String>, root: Option<PathBuf>) -> Result<(), CliError> {
    let api = runtime.api_client()?;

    let root = root.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let resolved = std::fs::canonicalize(&root).map_err(|e| {
        CliError::Usage(format!("Invalid workspace root {}: {e}", root.display()))
    })?;

    let default_name = resolved
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("workspace")
        .to_string();

    let name = name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(default_name);

    let body = json!({
        "name": name,
        "rootPath": resolved.display().to_string(),
    });

    let res = api.post_json("/v1/workspaces", Some(body), true).await?;
    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }

    let ws = res.json.get("workspace").cloned().unwrap_or_else(|| json!({}));
    let id = ws.get("id").and_then(|v| v.as_str()).unwrap_or("-");
    let root_path = ws.get("rootPath").and_then(|v| v.as_str()).unwrap_or("-");
    runtime.output.print_human(&format!("workspace created: id={id} rootPath={root_path}"));
    Ok(())
}

async fn list(runtime: &Runtime) -> Result<(), CliError> {
    let api = runtime.api_client()?;
    let res = api.get_json("/v1/workspaces", None, true).await?;

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }

    let items = res
        .json
        .get("workspaces")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if items.is_empty() {
        runtime.output.print_human("No workspaces.");
        return Ok(());
    }

    for w in items {
        let id = w.get("id").and_then(|v| v.as_str()).unwrap_or("-");
        let name = w.get("name").and_then(|v| v.as_str()).unwrap_or("-");
        let root = w.get("rootPath").and_then(|v| v.as_str()).unwrap_or("-");
        runtime.output.print_human(&format!("- {name}  ({id})  {root}"));
    }

    Ok(())
}

async fn set_permissions(
    runtime: &Runtime,
    workspace_id: String,
    user_id: Option<String>,
    can_read_files: Option<bool>,
    can_write_files: Option<bool>,
    can_read_images: Option<bool>,
    can_write_images: Option<bool>,
    can_web_search: Option<bool>,
) -> Result<(), CliError> {
    let api = runtime.api_client()?;

    let mut body = json!({});
    if let Some(uid) = user_id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        body["userId"] = json!(uid);
    }

    // Send snake_case keys (server also accepts camelCase).
    if let Some(v) = can_read_files { body["can_read_files"] = json!(v); }
    if let Some(v) = can_write_files { body["can_write_files"] = json!(v); }
    if let Some(v) = can_read_images { body["can_read_images"] = json!(v); }
    if let Some(v) = can_write_images { body["can_write_images"] = json!(v); }
    if let Some(v) = can_web_search { body["can_web_search"] = json!(v); }

    if body.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        return Err(CliError::Usage(
            "No permission fields provided. Pass e.g. --can-write-files true".to_string(),
        ));
    }

    let path = format!("/v1/workspaces/{}/permissions", workspace_id.trim());
    let res = api.post_json(&path, Some(body), true).await?;

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }

    let perm = res.json.get("permission").cloned().unwrap_or_else(|| json!({}));
    runtime.output.print_human(&format!("permissions updated: {}", summarize_perm(&perm)));
    Ok(())
}

fn summarize_perm(perm: &Value) -> String {
    let bool_field = |k: &str| perm.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
    format!(
        "read_files={} write_files={} read_images={} write_images={} web_search={}",
        bool_field("can_read_files"),
        bool_field("can_write_files"),
        bool_field("can_read_images"),
        bool_field("can_write_images"),
        bool_field("can_web_search"),
    )
}

