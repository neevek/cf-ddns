use anyhow::{Context, Result, bail};
use clap::Parser;
use lmrc_cloudflare::{CloudflareClient, dns::RecordType};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt::time::OffsetTime};

mod config;
mod ddns;
mod ip;

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() -> ExitCode {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let format = time::format_description::parse(
        "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]",
    )
    .unwrap_or_else(|_| {
        time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]Z")
            .unwrap()
    });
    let timer = OffsetTime::new(offset, format);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(true)
        .with_timer(timer)
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
    let client = CloudflareClient::builder()
        .api_token(config.api_token.clone())
        .build()?;
    let zone_id = client
        .zones()
        .get_zone_id(&config.zone)
        .await
        .with_context(|| format!("failed to resolve zone_id for {}", config.zone))?;

    let tick = Duration::from_secs(1);
    let mut interval = tokio::time::interval(tick);
    let mut last_wall = SystemTime::now();
    let mut next_due = add_duration(
        SystemTime::now(),
        update_with_delay(&config, &client, &zone_id, record_type, &record_name).await,
    );

    loop {
        interval.tick().await;
        let now = SystemTime::now();
        let wall_jump = now
            .duration_since(last_wall)
            .unwrap_or(Duration::ZERO);

        if wall_jump > tick + Duration::from_secs(2) {
            info!(
                "wake detected (wall_jump={}s), forcing immediate update",
                wall_jump.as_secs()
            );
            next_due = now;
        }

        if now.duration_since(next_due).is_ok() {
            let delay =
                update_with_delay(&config, &client, &zone_id, record_type, &record_name).await;
            next_due = add_duration(now, delay);
        }

        last_wall = now;
    }
}

async fn update_with_delay(
    config: &config::Config,
    client: &CloudflareClient,
    zone_id: &str,
    record_type: RecordType,
    record_name: &str,
) -> Duration {
    match update(config, client, zone_id, record_type, record_name).await {
        Ok(()) => Duration::from_secs(config.interval_seconds),
        Err(err) => {
            error!("{}", format_error(&err));
            Duration::from_secs(config.retry_seconds)
        }
    }
}

async fn update(
    config: &config::Config,
    client: &CloudflareClient,
    zone_id: &str,
    record_type: RecordType,
    record_name: &str,
) -> Result<()> {
    info!(
        "ddns: zone={}, record={}, type={}, interval={}s, iface={}",
        config.zone,
        record_name,
        config.record_type,
        config.interval_seconds,
        config.interface_name.as_deref().unwrap_or("auto")
    );

    ddns::update(config, &client, &zone_id, record_type, record_name).await
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

fn add_duration(base: SystemTime, duration: Duration) -> SystemTime {
    base.checked_add(duration).unwrap_or(base)
}
