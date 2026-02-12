use clap::{Args, ValueEnum};

use crate::app::Runtime;
use crate::errors::CliError;

#[derive(Debug, Clone, ValueEnum)]
pub enum UsageGroup {
    Day,
    Model,
    Provider,
}

#[derive(Debug, Args)]
pub struct UsageArgs {
    #[arg(long)]
    pub since: Option<String>,
    #[arg(long)]
    pub until: Option<String>,
    #[arg(long, value_enum)]
    pub group: Option<UsageGroup>,
}

pub async fn handle(runtime: &Runtime, args: UsageArgs) -> Result<(), CliError> {
    let api = runtime.api_client()?;
    // v1 API currently exposes the current monthly usage window only.
    // Ignore grouping filters for now (kept for backward compat CLI flags).
    let _ = args;

    let res = api.get_json("/v1/usage/current", None, true).await?;
    runtime.output.print_verbose(&format!(
        "request_id={:?} elapsed_ms={}",
        res.request_id, res.elapsed_ms
    ));

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }

    let total = res
        .json
        .get("totalTokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let limit = res
        .json
        .get("tokenLimit")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let start = res
        .json
        .get("periodStart")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let end = res
        .json
        .get("periodEnd")
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    runtime.output.print_human(&format!("total_tokens: {total}"));
    runtime.output.print_human(&format!("token_limit: {limit}"));
    runtime.output.print_human(&format!("period_start: {start}"));
    runtime.output.print_human(&format!("period_end: {end}"));
    Ok(())
}
