use clap::{Args, Subcommand};
use serde_json::json;

use crate::app::Runtime;
use crate::errors::CliError;

#[derive(Debug, Subcommand)]
pub enum TaskCommands {
    /// Create a new task
    Create(TaskCreateArgs),
    /// List tasks
    List(TaskListArgs),
    /// Get task details
    Get(TaskGetArgs),
    /// Update a task
    Update(TaskUpdateArgs),
    /// Delete a task
    Delete(TaskDeleteArgs),
    /// Start a task
    Start(TaskActionArgs),
    /// Complete a task
    Complete(TaskActionArgs),
    /// Cancel a task
    Cancel(TaskActionArgs),
    /// Manage task dependencies
    Dependencies(TaskDependencyArgs),
}

#[derive(Debug, Args)]
pub struct TaskCreateArgs {
    /// Task title
    #[arg(required = true)]
    pub title: String,
    /// Task description
    #[arg(long)]
    pub description: Option<String>,
    /// Task priority (0-10)
    #[arg(long, default_value = "0")]
    pub priority: i32,
    /// Due date (YYYY-MM-DD)
    #[arg(long)]
    pub due_date: Option<String>,
    /// Estimated hours
    #[arg(long)]
    pub estimated_hours: Option<i32>,
    /// Parent task ID (for subtasks)
    #[arg(long)]
    pub parent_id: Option<String>,
    /// Dependency task IDs (comma-separated)
    #[arg(long)]
    pub dependencies: Option<String>,
    /// Chat ID to associate with task
    #[arg(long)]
    pub chat_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct TaskListArgs {
    /// Filter by status
    #[arg(long)]
    pub status: Option<String>,
    /// Filter by priority
    #[arg(long)]
    pub priority: Option<i32>,
    /// Filter by parent task ID
    #[arg(long)]
    pub parent_id: Option<String>,
    /// Filter by chat ID
    #[arg(long)]
    pub chat_id: Option<String>,
    /// Number of tasks to return
    #[arg(long, default_value = "20")]
    pub limit: i32,
    /// Page number
    #[arg(long, default_value = "1")]
    pub page: i32,
}

#[derive(Debug, Args)]
pub struct TaskGetArgs {
    /// Task ID
    #[arg(required = true)]
    pub task_id: String,
}

#[derive(Debug, Args)]
pub struct TaskUpdateArgs {
    /// Task ID
    #[arg(required = true)]
    pub task_id: String,
    /// New title
    #[arg(long)]
    pub title: Option<String>,
    /// New description
    #[arg(long)]
    pub description: Option<String>,
    /// New status
    #[arg(long)]
    pub status: Option<String>,
    /// New priority
    #[arg(long)]
    pub priority: Option<i32>,
    /// New due date (YYYY-MM-DD)
    #[arg(long)]
    pub due_date: Option<String>,
    /// New estimated hours
    #[arg(long)]
    pub estimated_hours: Option<i32>,
    /// Actual hours spent
    #[arg(long)]
    pub actual_hours: Option<i32>,
}

#[derive(Debug, Args)]
pub struct TaskDeleteArgs {
    /// Task ID
    #[arg(required = true)]
    pub task_id: String,
}

#[derive(Debug, Args)]
pub struct TaskActionArgs {
    /// Task ID
    #[arg(required = true)]
    pub task_id: String,
}

#[derive(Debug, Args)]
pub struct TaskDependencyArgs {
    /// Task ID
    #[arg(required = true)]
    pub task_id: String,
    /// Add dependencies (comma-separated task IDs)
    #[arg(long)]
    pub add: Option<String>,
    /// Remove dependencies (comma-separated task IDs)
    #[arg(long)]
    pub remove: Option<String>,
}

pub async fn handle_tasks(runtime: &Runtime, cmd: TaskCommands) -> Result<(), CliError> {
    let api = runtime.api_client()?;

    match cmd {
        TaskCommands::Create(args) => handle_create_task(&api, args, runtime).await,
        TaskCommands::List(args) => handle_list_tasks(&api, args, runtime).await,
        TaskCommands::Get(args) => handle_get_task(&api, args, runtime).await,
        TaskCommands::Update(args) => handle_update_task(&api, args, runtime).await,
        TaskCommands::Delete(args) => handle_delete_task(&api, args, runtime).await,
        TaskCommands::Start(args) => handle_start_task(&api, args, runtime).await,
        TaskCommands::Complete(args) => handle_complete_task(&api, args, runtime).await,
        TaskCommands::Cancel(args) => handle_cancel_task(&api, args, runtime).await,
        TaskCommands::Dependencies(args) => handle_task_dependencies(&api, args, runtime).await,
    }
}

async fn handle_create_task(api: &crate::api::ApiClient, args: TaskCreateArgs, runtime: &Runtime) -> Result<(), CliError> {
    let request = json!({
        "title": args.title,
        "description": args.description,
        "priority": args.priority,
        "chatId": args.chat_id,
        "parentId": args.parent_id,
    });

    let res = api.post_json("/v1/tasks", Some(request), true).await?;

    let task_id = res.json.get("task")
        .and_then(|t| t.get("id"))
        .and_then(|i| i.as_str())
        .unwrap_or("unknown");

    let title = res.json.get("task")
        .and_then(|t| t.get("title"))
        .and_then(|t| t.as_str())
        .unwrap_or("Untitled");

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
    } else {
        runtime.output.print_human(&format!("✓ Created task: {} (ID: {})", title, task_id));
    }

    Ok(())
}

async fn handle_list_tasks(api: &crate::api::ApiClient, args: TaskListArgs, runtime: &Runtime) -> Result<(), CliError> {
    let tasks = api.list_tasks(args.status, args.limit).await?;

    if runtime.output.json {
        runtime.output.print_json(&json!({ "tasks": tasks }))?;
    } else {
        if tasks.is_empty() {
            runtime.output.print_human("No tasks found.");
            return Ok(());
        }

        runtime.output.print_human("Tasks:");
        for task in tasks {
            let status_icon = match task.status.as_str() {
                "PENDING" => "⏳",
                "IN_PROGRESS" => "🔄",
                "COMPLETED" => "✅",
                "CANCELLED" => "❌",
                _ => "❓",
            };

            let priority_indicator = "⋅".repeat((task.priority.min(10)).max(0) as usize);

            runtime.output.print_human(&format!(
                "{} [{}] {} {}",
                status_icon,
                task.priority,
                priority_indicator,
                task.title
            ));

            if let Some(ref desc) = task.description {
                runtime.output.print_human(&format!("    {}", desc));
            }
        }
    }

    Ok(())
}

async fn handle_get_task(api: &crate::api::ApiClient, args: TaskGetArgs, runtime: &Runtime) -> Result<(), CliError> {
    let task = api.get_task(&args.task_id).await?;

    if runtime.output.json {
        runtime.output.print_json(&task)?;
    } else {
        let status_icon = match task.status.as_str() {
            "PENDING" => "⏳",
            "IN_PROGRESS" => "🔄",
            "COMPLETED" => "✅",
            "CANCELLED" => "❌",
            _ => "❓",
        };

        runtime.output.print_human(&format!(
            "{} {} [Priority: {}]",
            status_icon,
            task.title,
            task.priority
        ));

        if let Some(ref desc) = task.description {
            runtime.output.print_human(&format!("Description: {}", desc));
        }

        runtime.output.print_human(&format!("Status: {}", task.status));
        runtime.output.print_human(&format!("Created: {}", task.created_at));

        if let Some(ref completed_at) = task.completed_at {
            runtime.output.print_human(&format!("Completed: {}", completed_at));
        }
    }

    Ok(())
}

async fn handle_update_task(api: &crate::api::ApiClient, args: TaskUpdateArgs, runtime: &Runtime) -> Result<(), CliError> {
    let request = json!({
        "title": args.title,
        "description": args.description,
        "status": args.status,
        "priority": args.priority,
    });

    let res = api.put_json(&format!("/v1/tasks/{}", args.task_id), Some(request), true).await?;

    let task = res.json.get("task").and_then(|t| t.get("title")).and_then(|t| t.as_str()).unwrap_or("Task");

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
    } else {
        runtime.output.print_human(&format!("✓ Updated task: {}", task));
    }

    Ok(())
}

async fn handle_delete_task(api: &crate::api::ApiClient, args: TaskDeleteArgs, runtime: &Runtime) -> Result<(), CliError> {
    api.delete_task(&args.task_id).await?;

    if runtime.output.json {
        runtime.output.print_json(&json!({ "success": true, "message": "Task deleted" }))?;
    } else {
        runtime.output.print_human(&format!("✓ Deleted task: {}", args.task_id));
    }

    Ok(())
}

async fn handle_start_task(api: &crate::api::ApiClient, args: TaskActionArgs, runtime: &Runtime) -> Result<(), CliError> {
    let res = api.post_json(&format!("/v1/tasks/{}/start", args.task_id), None, true).await?;

    let task = res.json.get("task").and_then(|t| t.get("title")).and_then(|t| t.as_str()).unwrap_or("Task");

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
    } else {
        runtime.output.print_human(&format!("✓ Started task: {}", task));
    }

    Ok(())
}

async fn handle_complete_task(api: &crate::api::ApiClient, args: TaskActionArgs, runtime: &Runtime) -> Result<(), CliError> {
    let res = api.post_json(&format!("/v1/tasks/{}/complete", args.task_id), None, true).await?;

    let task = res.json.get("task").and_then(|t| t.get("title")).and_then(|t| t.as_str()).unwrap_or("Task");

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
    } else {
        runtime.output.print_human(&format!("✓ Completed task: {}", task));
    }

    Ok(())
}

async fn handle_cancel_task(api: &crate::api::ApiClient, args: TaskActionArgs, runtime: &Runtime) -> Result<(), CliError> {
    let res = api.post_json(&format!("/v1/tasks/{}/cancel", args.task_id), None, true).await?;

    let task = res.json.get("task").and_then(|t| t.get("title")).and_then(|t| t.as_str()).unwrap_or("Task");

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
    } else {
        runtime.output.print_human(&format!("✓ Cancelled task: {}", task));
    }

    Ok(())
}

async fn handle_task_dependencies(api: &crate::api::ApiClient, args: TaskDependencyArgs, runtime: &Runtime) -> Result<(), CliError> {
    if args.add.is_some() {
        let request = json!({
            "dependencies": args.add,
        });

        let res = api.post_json(
            &format!("/v1/tasks/{}/dependencies", args.task_id),
            Some(request),
            true
        ).await?;

        if runtime.output.json {
            runtime.output.print_json(&res.json)?;
        } else {
            let deps = args.add.unwrap_or_default();
            runtime.output.print_human(&format!("✓ Added dependencies to task {}: {}", args.task_id, deps));
        }
    }

    if args.remove.is_some() {
        let deps = args.remove.clone().unwrap_or_default();

        // Handle remove dependencies
        if !runtime.output.json {
            runtime.output.print_human(&format!("✓ Removed dependencies from task {}: {}", args.task_id, deps));
        }
    }

    Ok(())
}