use anyhow::{Context, Result, bail};
use lmrc_cloudflare::{CloudflareClient, dns::RecordType};
use std::{env, process::ExitCode};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod config;
mod ddns;
mod ip;

#[tokio::main]
async fn main() -> ExitCode {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).with_ansi(true).init();

    if let Err(err) = run().await {
        error!("Error: {err:#}");
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}

async fn run() -> Result<()> {
    let config_path = resolve_config_path()?;
    let config = config::Config::load(&config_path)
        .with_context(|| format!("failed to load config at {config_path}"))?;

    let record_type = record_type(&config)?;
    let record_name = config.record_name();

    let client = CloudflareClient::builder()
        .api_token(config.api_token.clone())
        .build()?;
    let zone_id = client
        .zones()
        .get_zone_id(&config.zone)
        .await
        .with_context(|| format!("failed to resolve zone_id for {}", config.zone))?;

    info!(
        "ddns: zone={}, record={}, type={}, interval={}s, iface={}",
        config.zone,
        record_name,
        config.record_type,
        config.interval_seconds,
        config.interface_name.as_deref().unwrap_or("auto")
    );

    ddns::run(&client, &config, &zone_id, &record_name, record_type).await
}

fn record_type(config: &config::Config) -> Result<RecordType> {
    match config.record_type.trim().to_ascii_uppercase().as_str() {
        "A" => Ok(RecordType::A),
        "AAAA" => Ok(RecordType::AAAA),
        other => bail!("unsupported record_type: {other} (use A or AAAA)"),
    }
}

fn resolve_config_path() -> Result<String> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--cfg" {
            if let Some(path) = args.next() {
                return Ok(path);
            }
            bail!("--cfg requires a path");
        }
        if let Some(rest) = arg.strip_prefix("--cfg=") {
            if !rest.is_empty() {
                return Ok(rest.to_string());
            }
            bail!("--cfg requires a path");
        }
    }
    Ok("config.toml".to_string())
}
