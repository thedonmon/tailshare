use anyhow::{Context, Result};
use colored::Colorize;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Device {
    pub name: String,
    pub dns_name: String,
    /// MagicDNS short name (e.g., "dons-mac-mini")
    pub short_name: String,
    pub ip: String,
    pub online: bool,
    pub is_self: bool,
    pub os: String,
}

#[derive(Deserialize)]
struct TailscaleStatus {
    #[serde(rename = "Self")]
    self_node: TailscaleNode,
    #[serde(rename = "Peer")]
    peer: Option<HashMap<String, TailscaleNode>>,
}

#[derive(Deserialize)]
struct TailscaleNode {
    #[serde(rename = "HostName")]
    hostname: String,
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Vec<String>,
    #[serde(rename = "Online")]
    online: bool,
    #[serde(rename = "OS")]
    os: String,
    #[allow(dead_code)]
    #[serde(rename = "UserID")]
    user_id: i64,
}

fn tailscale_bin() -> &'static str {
    if cfg!(target_os = "macos") {
        if std::path::Path::new("/Applications/Tailscale.app").exists() {
            return "/Applications/Tailscale.app/Contents/MacOS/Tailscale";
        }
    }
    // Linux, Windows, or macOS with CLI install
    "tailscale"
}

fn get_status() -> Result<TailscaleStatus> {
    let output = Command::new(tailscale_bin())
        .args(["status", "--json"])
        .env("TERM", "xterm")
        .output()
        .context("Failed to run 'tailscale'. Is Tailscale installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tailscale status failed: {}", stderr);
    }

    let status: TailscaleStatus =
        serde_json::from_slice(&output.stdout).context("Failed to parse tailscale status")?;

    Ok(status)
}

fn node_to_device(node: &TailscaleNode, is_self: bool) -> Device {
    let dns_clean = node.dns_name.trim_end_matches('.').to_string();
    let short_name = dns_clean.split('.').next().unwrap_or("").to_string();

    // Check for OS override in config
    let os = if let Ok(Some(cfg)) = crate::config::load() {
        if is_self {
            cfg.local_os.unwrap_or_else(|| node.os.clone())
        } else {
            cfg.os_overrides
                .get(&short_name)
                .cloned()
                .unwrap_or_else(|| node.os.clone())
        }
    } else {
        node.os.clone()
    };

    // Normalize OS names: config uses "macos"/"linux"/"windows", Tailscale uses "macOS"/"linux"/"windows"
    let os = match os.as_str() {
        "macos" => "macOS".to_string(),
        other => other.to_string(),
    };

    Device {
        name: node.hostname.clone(),
        dns_name: dns_clean,
        short_name,
        ip: node.tailscale_ips.first().cloned().unwrap_or_default(),
        online: node.online,
        is_self,
        os,
    }
}

pub fn get_all_devices() -> Result<Vec<Device>> {
    let status = get_status()?;
    let mut devices = vec![node_to_device(&status.self_node, true)];

    if let Some(peers) = &status.peer {
        for node in peers.values() {
            devices.push(node_to_device(node, false));
        }
    }

    Ok(devices)
}

pub async fn list_devices() -> Result<()> {
    let devices = get_all_devices()?;

    println!("{}", "Tailscale Devices".bold());
    println!("{}", "─".repeat(60));

    for device in &devices {
        let status = if device.is_self {
            "(this device)".cyan().to_string()
        } else if device.online {
            "online".green().to_string()
        } else {
            "offline".red().to_string()
        };

        let os_icon = match device.os.as_str() {
            "macOS" => "mac",
            "linux" => "linux",
            "windows" => "win",
            "iOS" => "ios",
            "android" => "android",
            _ => &device.os,
        };

        println!(
            "  {} {} [{}] [{}]",
            device.name.bold(),
            format!("({})", device.ip).dimmed(),
            os_icon,
            status
        );
        println!("    {}", device.dns_name.dimmed());
    }

    println!("\n{} devices found", devices.len());
    Ok(())
}

pub async fn find_device(query: &str) -> Result<Device> {
    let mut devices = get_all_devices()?;
    let query_lower = query.to_lowercase();

    // Match by hostname, short name, dns name, or IP
    let found = devices.iter().position(|d| {
        d.name.to_lowercase() == query_lower
            || d.short_name.to_lowercase() == query_lower
            || d.dns_name.to_lowercase().starts_with(&query_lower)
            || d.ip == query
            || d.short_name.to_lowercase().contains(&query_lower)
            || d.name.to_lowercase().contains(&query_lower)
    });

    if let Some(idx) = found {
        return Ok(devices.remove(idx));
    }

    // Build a helpful error message with available devices and suggestions
    let available: Vec<&Device> = devices.iter().filter(|d| !d.is_self).collect();

    let mut msg = format!("Device '{}' not found on your tailnet.\n", query);

    // Check if this looks like a stale config reference
    if let Ok(Some(cfg)) = crate::config::load() {
        let mut stale_refs = Vec::new();
        if cfg.default_device.as_deref() == Some(query) {
            stale_refs.push("default_device".to_string());
        }
        for (alias, target) in &cfg.aliases {
            if target == query {
                stale_refs.push(format!("alias '{}'", alias));
            }
        }
        if cfg.users.contains_key(query) {
            stale_refs.push("users".to_string());
        }
        if cfg.os_overrides.contains_key(query) {
            stale_refs.push("os_overrides".to_string());
        }
        if !stale_refs.is_empty() {
            msg.push_str(&format!(
                "\nThis name appears in your config ({}) but no longer matches a device.\n\
                 The machine may have been renamed. Run 'tailshare config doctor' to check.\n",
                stale_refs.join(", ")
            ));
        }
    }

    // Suggest similar device names
    let suggestions: Vec<&str> = available
        .iter()
        .filter(|d| {
            let sn = d.short_name.to_lowercase();
            let ql = query.to_lowercase();
            // Share at least one word segment
            ql.split('-').any(|part| part.len() >= 3 && sn.contains(part))
                || sn.split('-').any(|part| part.len() >= 3 && ql.contains(part))
        })
        .map(|d| d.short_name.as_str())
        .collect();

    if !suggestions.is_empty() {
        msg.push_str(&format!("\nDid you mean?\n"));
        for s in &suggestions {
            msg.push_str(&format!("  - {}\n", s));
        }
    }

    msg.push_str(&format!("\nAvailable devices:\n"));
    for d in &available {
        let status = if d.online { "online" } else { "offline" };
        msg.push_str(&format!("  {} ({}) [{}]\n", d.short_name, d.ip, status));
    }
    msg.push_str("\nTo update your config: tailshare config doctor");

    Err(anyhow::anyhow!("{}", msg))
}
