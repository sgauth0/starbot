use clap::Subcommand;
use serde_json::json;

use crate::app::Runtime;
use crate::config::{ensure_profile, profile_mut, save_config};
use crate::errors::CliError;

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Authenticate via device code flow (QR or manual)
    Login {
        /// Paste a token directly instead of using device code flow
        #[arg(long)]
        token: Option<String>,
    },
    /// Remove bearer token from active profile
    Logout,
}

pub async fn handle(runtime: &mut Runtime, command: AuthCommand) -> Result<(), CliError> {
    match command {
        AuthCommand::Login { token } => login(runtime, token).await,
        AuthCommand::Logout => logout(runtime).await,
    }
}

async fn login(runtime: &mut Runtime, token: Option<String>) -> Result<(), CliError> {
    // If --token provided, use direct token save (legacy / CI mode)
    if let Some(t) = token {
        let t = t.trim().to_string();
        if t.is_empty() {
            return Err(CliError::Usage("Token cannot be empty.".to_string()));
        }
        return save_token(runtime, t, None);
    }

    if is_ci() {
        return Err(CliError::Usage(
            "CI mode detected. Pass `--token` explicitly.".to_string(),
        ));
    }

    // Device code flow
    device_code_flow(runtime).await
}

async fn device_code_flow(runtime: &mut Runtime) -> Result<(), CliError> {
    let api = runtime.api_client()?;

    // 1. POST /v1/auth/device/start
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());

    let start_body = json!({
        "client_name": "starbott",
        "client_version": env!("CARGO_PKG_VERSION"),
        "device_meta": {
            "hostname": hostname,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        }
    });

    let res = api
        .post_json_with_body("/v1/auth/device/start", &start_body, false)
        .await?;

    let device_code = res
        .json
        .get("deviceCode")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CliError::Server("Missing deviceCode in response".to_string()))?
        .to_string();

    let user_code = res
        .json
        .get("userCode")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CliError::Server("Missing userCode in response".to_string()))?
        .to_string();

    let verification_url = res
        .json
        .get("verificationUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CliError::Server("Missing verificationUrl in response".to_string()))?
        .to_string();

    let interval = res
        .json
        .get("interval")
        .and_then(|v| v.as_u64())
        .unwrap_or(5);

    // 2. Render QR code
    let auth_url = format!("{}?code={}", verification_url, user_code);
    render_qr(&auth_url);

    runtime.output.print_human("");
    runtime
        .output
        .print_human(&format!("  Scan the QR code or visit: {}", verification_url));
    runtime.output.print_human(&format!("  Code: {}", user_code));
    runtime.output.print_human("");
    runtime
        .output
        .print_human("  Waiting for authorization...");

    // 3. Poll loop
    let poll_body = json!({ "deviceCode": device_code });
    let max_attempts = 180 / interval; // 3 minutes max

    for _ in 0..max_attempts {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

        let poll_res = match api
            .post_json_with_body("/v1/auth/device/poll", &poll_body, false)
            .await
        {
            Ok(r) => r,
            Err(_) => continue, // network blip, retry
        };

        let status = poll_res
            .json
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending");

        match status {
            "authorized" => {
                let access_token = poll_res
                    .json
                    .get("accessToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let refresh_token = poll_res
                    .json
                    .get("refreshToken")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                save_token(runtime, access_token, refresh_token)?;

                if runtime.output.json {
                    runtime.output.print_json(&json!({ "ok": true }))?;
                } else {
                    runtime.output.print_human("  Logged in successfully.");
                }
                return Ok(());
            }
            "expired" => {
                return Err(CliError::Auth(
                    "Device code expired. Please try again.".to_string(),
                ));
            }
            _ => {
                // pending — keep polling
            }
        }
    }

    Err(CliError::Auth(
        "Authorization timed out. Please try again.".to_string(),
    ))
}

fn render_qr(url: &str) {
    use qrcode::QrCode;

    let code = match QrCode::new(url.as_bytes()) {
        Ok(c) => c,
        Err(_) => {
            // If QR generation fails, just skip it
            return;
        }
    };

    let string = code
        .render::<char>()
        .quiet_zone(true)
        .module_dimensions(2, 1)
        .build();

    println!();
    for line in string.lines() {
        println!("  {}", line);
    }
}

fn save_token(
    runtime: &mut Runtime,
    access_token: String,
    refresh_token: Option<String>,
) -> Result<(), CliError> {
    let profile_name = runtime.active_profile();
    ensure_profile(&mut runtime.config, &profile_name);
    let profile = profile_mut(&mut runtime.config, &profile_name)
        .ok_or_else(|| CliError::Generic("Failed to load active profile.".to_string()))?;
    profile.token = Some(access_token);
    if let Some(rt) = refresh_token {
        profile.refresh_token = Some(rt);
    }
    save_config(&runtime.config)?;
    Ok(())
}

async fn logout(runtime: &mut Runtime) -> Result<(), CliError> {
    let profile_name = runtime.active_profile();
    ensure_profile(&mut runtime.config, &profile_name);
    let profile = profile_mut(&mut runtime.config, &profile_name)
        .ok_or_else(|| CliError::Generic("Failed to load active profile.".to_string()))?;
    profile.token = None;
    profile.refresh_token = None;
    save_config(&runtime.config)?;

    if runtime.output.json {
        runtime.output.print_json(&json!({ "ok": true }))?;
    } else {
        runtime.output.print_human("Logged out.");
    }
    Ok(())
}

fn is_ci() -> bool {
    std::env::var("CI")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
