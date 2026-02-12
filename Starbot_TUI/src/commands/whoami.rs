use crate::app::Runtime;
use crate::errors::CliError;

pub async fn handle(runtime: &Runtime) -> Result<(), CliError> {
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

    let id = res.json.get("id").and_then(|v| v.as_str()).unwrap_or("-");
    let email = res
        .json
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    runtime.output.print_human(&format!("id: {id}"));
    runtime.output.print_human(&format!("email: {email}"));

    let plan_status = res
        .json
        .get("planStatus")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let period_end = res
        .json
        .get("currentPeriodEnd")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    runtime.output.print_human(&format!("plan_status: {plan_status}"));
    runtime.output.print_human(&format!("period_end: {period_end}"));

    Ok(())
}
