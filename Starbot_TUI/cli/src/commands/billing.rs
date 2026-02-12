use clap::{Args, Subcommand};
use serde_json::json;

use crate::app::Runtime;
use crate::errors::CliError;

#[derive(Debug, Subcommand)]
pub enum BillingCommand {
    /// Show subscription and plan status
    Status,
    /// Create a billing portal session URL
    Portal(PortalArgs),
}

#[derive(Debug, Args)]
pub struct PortalArgs {
    #[arg(long)]
    pub open: bool,
}

pub async fn handle(runtime: &Runtime, command: BillingCommand) -> Result<(), CliError> {
    match command {
        BillingCommand::Status => status(runtime).await,
        BillingCommand::Portal(args) => portal(runtime, args).await,
    }
}

async fn status(runtime: &Runtime) -> Result<(), CliError> {
    let api = runtime.api_client()?;
    let res = api.get_json("/v1/auth/me", None, true).await?;
    runtime.output.print_verbose(&format!(
        "request_id={:?} elapsed_ms={}",
        res.request_id, res.elapsed_ms
    ));

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }

    let plan_status = res
        .json
        .get("planStatus")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let renews = res
        .json
        .get("currentPeriodEnd")
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    runtime.output.print_human(&format!("plan_status: {plan_status}"));
    runtime.output.print_human(&format!("period_end: {renews}"));
    Ok(())
}

async fn portal(runtime: &Runtime, args: PortalArgs) -> Result<(), CliError> {
    let api = runtime.api_client()?;
    let res = api.post_json("/v1/billing/portal-session", None, true).await?;
    runtime.output.print_verbose(&format!(
        "request_id={:?} elapsed_ms={}",
        res.request_id, res.elapsed_ms
    ));

    let url = res
        .json
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            CliError::Server("Billing portal URL was not returned by the server.".to_string())
        })?;

    if runtime.output.json {
        runtime.output.print_json(&json!({ "url": url }))?;
    } else {
        runtime.output.print_human(url);
    }

    if args.open {
        open::that(url).map_err(|e| {
            CliError::Generic(format!("Failed to open browser for portal URL: {e}"))
        })?;
    }

    Ok(())
}
