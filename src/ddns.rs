use anyhow::{Context, Result};
use lmrc_cloudflare::{CloudflareClient, dns::RecordType};
use std::time::Duration;
use tracing::{error, info};

use crate::config::Config;
use crate::ip::select_local_ip;

pub async fn run(
    client: &CloudflareClient,
    config: &Config,
    zone_id: &str,
    record_name: &str,
    record_type: RecordType,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(config.interval_seconds));
    loop {
        interval.tick().await;
        if let Err(err) = update(client, config, zone_id, record_name, record_type).await {
            error!(error = %err, "ddns update failed");
        }
    }
}

async fn update(
    client: &CloudflareClient,
    config: &Config,
    zone_id: &str,
    record_name: &str,
    record_type: RecordType,
) -> Result<()> {
    let desired_v6 = matches!(record_type, RecordType::AAAA);
    let ip = select_local_ip(config.interface_name.as_deref(), desired_v6)?;
    let existing = client
        .dns()
        .find_record(zone_id, record_name, record_type)
        .await
        .context("failed to lookup DNS record")?;

    if let Some(record) = existing {
        if record.content == ip {
            info!(ip = %ip, "ddns no change");
            return Ok(());
        }

        let mut update = client
            .dns()
            .update_record(zone_id, &record.id)
            .name(record_name)
            .record_type(record_type)
            .content(&ip);

        if let Some(proxied) = config.proxied {
            update = update.proxied(proxied);
        }
        if let Some(ttl) = config.ttl {
            update = update.ttl(ttl);
        }

        update.send().await.context("failed to update DNS record")?;
        info!(record = %record_name, ip = %ip, "ddns updated");
        return Ok(());
    }

    let mut create = client
        .dns()
        .create_record(zone_id)
        .name(record_name)
        .record_type(record_type)
        .content(&ip);

    if let Some(proxied) = config.proxied {
        create = create.proxied(proxied);
    }
    if let Some(ttl) = config.ttl {
        create = create.ttl(ttl);
    }

    create.send().await.context("failed to create DNS record")?;
    info!(record = %record_name, ip = %ip, "ddns created");
    Ok(())
}
