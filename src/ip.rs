use anyhow::{Context, Result, bail};
use get_if_addrs::{IfAddr, Interface, get_if_addrs};
use std::net::IpAddr;
use std::str::FromStr;

use crate::config::Config;

pub async fn select_ip(config: &Config, desired_v6: bool) -> Result<String> {
    if config.use_public_ip {
        return fetch_public_ip(&config.public_ip_urls, desired_v6).await;
    }
    select_local_ip(config.interface_name.as_deref(), desired_v6)
}

fn select_local_ip(interface_name: Option<&str>, desired_v6: bool) -> Result<String> {
    let addrs = get_if_addrs().context("failed to read network interfaces")?;

    if let Some(name) = interface_name {
        for iface in &addrs {
            if iface.name == name
                && addr_matches(&iface.addr, desired_v6)
                && addr_viable(&iface.addr)
            {
                return Ok(addr_to_string(&iface.addr));
            }
        }
        bail!("no matching IP found on interface {name}");
    }

    if let Some(ip) = pick_ip(&addrs, desired_v6, true) {
        return Ok(ip);
    }
    if let Some(ip) = pick_ip(&addrs, desired_v6, false) {
        return Ok(ip);
    }

    let names = unique_interface_names(&addrs);
    bail!(
        "no suitable interface found for {}; available: {}",
        if desired_v6 { "AAAA" } else { "A" },
        names.join(", ")
    );
}

async fn fetch_public_ip(urls: &[String], desired_v6: bool) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("failed to build HTTP client")?;

    let mut last_err: Option<anyhow::Error> = None;
    for url in urls {
        let body = match client.get(url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(text) => text,
                Err(err) => {
                    last_err = Some(err.into());
                    continue;
                }
            },
            Err(err) => {
                last_err = Some(err.into());
                continue;
            }
        };
        let ip = body.trim();
        if ip.is_empty() {
            last_err = Some(anyhow::anyhow!("empty ip response from {url}"));
            continue;
        }
        let parsed = match IpAddr::from_str(ip) {
            Ok(value) => value,
            Err(err) => {
                last_err = Some(err.into());
                continue;
            }
        };
        let is_v6 = matches!(parsed, IpAddr::V6(_));
        if desired_v6 != is_v6 {
            last_err = Some(anyhow::anyhow!(
                "ip family mismatch from {url}: got {}, expected {}",
                parsed,
                if desired_v6 { "IPv6" } else { "IPv4" }
            ));
            continue;
        }
        return Ok(parsed.to_string());
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no public IP URLs configured")))
}

fn pick_ip(addrs: &[Interface], desired_v6: bool, prefer_physical: bool) -> Option<String> {
    for iface in addrs {
        if is_loopback(&iface.addr) || is_virtual_name(&iface.name) {
            continue;
        }
        if !addr_viable(&iface.addr) {
            continue;
        }
        if prefer_physical && !is_preferred_physical(&iface.name) {
            continue;
        }
        if addr_matches(&iface.addr, desired_v6) {
            return Some(addr_to_string(&iface.addr));
        }
    }
    None
}

fn addr_matches(addr: &IfAddr, desired_v6: bool) -> bool {
    match addr {
        IfAddr::V4(_) => !desired_v6,
        IfAddr::V6(_) => desired_v6,
    }
}

fn addr_to_string(addr: &IfAddr) -> String {
    match addr {
        IfAddr::V4(v4) => v4.ip.to_string(),
        IfAddr::V6(v6) => v6.ip.to_string(),
    }
}

fn addr_viable(addr: &IfAddr) -> bool {
    match addr {
        IfAddr::V4(v4) => {
            let ip = v4.ip;
            !ip.is_unspecified() && !ip.is_multicast() && !ip.is_broadcast() && !ip.is_link_local()
        }
        IfAddr::V6(v6) => {
            let ip = v6.ip;
            !ip.is_unspecified() && !ip.is_multicast() && !ip.is_unicast_link_local()
        }
    }
}

fn is_loopback(addr: &IfAddr) -> bool {
    match addr {
        IfAddr::V4(v4) => v4.ip.is_loopback(),
        IfAddr::V6(v6) => v6.ip.is_loopback(),
    }
}

fn is_virtual_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let prefixes = [
        "lo",
        "docker",
        "veth",
        "virbr",
        "br-",
        "vmnet",
        "utun",
        "tun",
        "tap",
        "wg",
        "zt",
        "vboxnet",
        "awdl",
        "llw",
        "p2p",
        "bridge",
        "ham",
        "tailscale",
    ];
    prefixes.iter().any(|p| n.starts_with(p))
}

fn is_preferred_physical(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if cfg!(target_os = "macos") {
        n.starts_with("en")
    } else if cfg!(target_os = "linux") {
        n.starts_with("en")
            || n.starts_with("eth")
            || n.starts_with("wl")
            || n.starts_with("wlan")
            || n.starts_with("wlp")
            || n.starts_with("eno")
            || n.starts_with("ens")
            || n.starts_with("enp")
            || n.starts_with("em")
            || n.starts_with("p")
    } else if cfg!(target_os = "windows") {
        n.contains("ethernet") || n.contains("wi-fi") || n.contains("wifi")
    } else {
        false
    }
}

fn unique_interface_names(addrs: &[Interface]) -> Vec<String> {
    let mut names: Vec<String> = addrs.iter().map(|a| a.name.clone()).collect();
    names.sort();
    names.dedup();
    names
}
