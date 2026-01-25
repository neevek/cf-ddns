use anyhow::{Context, Result, bail};
use clap::Parser;
use lmrc_cloudflare::{CloudflareClient, dns::RecordType};
use std::process::ExitCode;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod config;
mod ddns;
mod ip;

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() -> ExitCode {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(true)
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{}", format_error(&err));
            ExitCode::from(1)
        }
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();

    let config_path = &args.config_path;
    let config = config::Config::load(config_path)
        .with_context(|| format!("failed to load config at {config_path}"))?;
    let record_type = record_type(&config)?;
    let record_name = config.record_name();

    loop {
        let next_delay = match update(&config, &record_name, record_type).await {
            Ok(()) => std::time::Duration::from_secs(config.interval_seconds),
            Err(err) => {
                error!("{}", format_error(&err));
                std::time::Duration::from_secs(config.retry_seconds)
            }
        };
        tokio::time::sleep(next_delay).await;
    }
}

async fn update(config: &config::Config, record_name: &str, record_type: RecordType) -> Result<()> {
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

    ddns::update(&client, config, &zone_id, record_name, record_type).await
}

fn record_type(config: &config::Config) -> Result<RecordType> {
    match config.record_type.trim().to_ascii_uppercase().as_str() {
        "A" => Ok(RecordType::A),
        "AAAA" => Ok(RecordType::AAAA),
        other => bail!("unsupported record_type: {other} (use A or AAAA)"),
    }
}

#[derive(Parser)]
#[command(
    name = "cf-ddns",
    version,
    about = "Cloudflare DDNS client (intranet-first)"
)]
struct Args {
    #[arg(long = "cfg", default_value = "config.toml", value_name = "PATH")]
    config_path: String,
}

fn format_error(err: &anyhow::Error) -> String {
    let top = err.to_string();
    let root = err.root_cause().to_string();
    if top == root {
        format!("Error: {top}")
    } else {
        format!("Error: {top} (cause: {root})")
    }
}
