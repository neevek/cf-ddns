use crate::config::Config;
use crate::ip::select_ip;
use anyhow::{Context, Result};
use lmrc_cloudflare::{CloudflareClient, dns::RecordType};
use tracing::info;

pub async fn update(
    config: &Config,
    client: &CloudflareClient,
    zone_id: &str,
    record_type: RecordType,
    record_name: &str,
) -> Result<()> {
    let desired_v6 = matches!(record_type, RecordType::AAAA);
    let ip = select_ip(config, desired_v6).await?;
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
