use crate::app::Runtime;
use crate::errors::CliError;

pub async fn handle(runtime: &Runtime) -> Result<(), CliError> {
    let api = runtime.api_client()?;
    let res = api.get_json("/health", None, false).await?;
    runtime.output.print_verbose(&format!(
        "request_id={:?} elapsed_ms={}",
        res.request_id, res.elapsed_ms
    ));

    if runtime.output.json {
        runtime.output.print_json(&res.json)?;
        return Ok(());
    }

    let ok = res
        .json
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let version = res
        .json
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let inference = res
        .json
        .get("inference")
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    runtime.output.print_human(&format!("ok: {ok}"));
    runtime.output.print_human(&format!("version: {version}"));
    runtime
        .output
        .print_human(&format!("inference: {inference}"));

    if let Some(providers) = res.json.get("providers").and_then(|v| v.as_object()) {
        for (name, status) in providers {
            runtime.output.print_human(&format!(
                "provider.{name}: {}",
                status.as_str().unwrap_or("-")
            ));
        }
    }

    Ok(())
}
