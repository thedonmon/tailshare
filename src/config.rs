use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::cli::ConfigCommands;
use crate::tailscale::Device;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub default_device: Option<String>,
    /// OS of the current (local) machine: "macos", "linux", "windows"
    pub local_os: Option<String>,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    #[serde(default)]
    pub users: HashMap<String, String>,
    /// Override OS per device (device name -> "macos"/"linux"/"windows")
    #[serde(default)]
    pub os_overrides: HashMap<String, String>,
    /// Stable Tailscale IP for each device (short_name -> IP).
    /// Used to match devices after name changes.
    #[serde(default)]
    pub device_ips: HashMap<String, String>,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap().join(".config"))
        .join("tailshare")
        .join("config.toml")
}

pub fn load() -> Result<Option<Config>> {
    let path = config_path();
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(Some(config))
}

fn save(config: &Config) -> Result<()> {
    let path = config_path();
    std::fs::create_dir_all(path.parent().unwrap())?;
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Record device IPs so we can match them after name changes.
/// Called during setup and sync to keep the mapping current.
pub fn record_device_ip(short_name: &str, ip: &str) -> Result<()> {
    let mut config = load()?.unwrap_or_default();
    let existing = config.device_ips.get(short_name);
    if existing.map(|s| s.as_str()) != Some(ip) {
        config.device_ips.insert(short_name.to_string(), ip.to_string());
        save(&config)?;
    }
    Ok(())
}

/// Backfill IPs for all known devices in config that match current Tailscale devices.
/// Also adds IPs for devices referenced anywhere in the config (default, aliases, users, etc.).
/// This ensures IP tracking is populated even for configs created before IP tracking existed.
pub fn backfill_device_ips(devices: &[Device]) -> Result<usize> {
    let mut config = load()?.unwrap_or_default();
    let mut added = 0;

    // Collect all device names referenced in config
    let mut referenced_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(ref d) = config.default_device {
        referenced_names.insert(d.clone());
    }
    for target in config.aliases.values() {
        referenced_names.insert(target.clone());
    }
    for key in config.users.keys() {
        referenced_names.insert(key.clone());
    }
    for key in config.os_overrides.keys() {
        referenced_names.insert(key.clone());
    }

    // For each referenced name, if it matches a real device and we don't have its IP, record it
    for name in &referenced_names {
        if config.device_ips.contains_key(name) {
            continue;
        }
        if let Some(device) = devices.iter().find(|d| d.short_name.eq_ignore_ascii_case(name)) {
            config.device_ips.insert(name.clone(), device.ip.clone());
            added += 1;
        }
    }

    if added > 0 {
        save(&config)?;
    }
    Ok(added)
}

/// A stale config entry that references a device name not found on the tailnet.
#[derive(Debug)]
pub struct StaleEntry {
    pub section: String,
    pub key: String,
    pub suggestions: Vec<String>,
}

/// Validate all device names in config against actual Tailscale devices.
/// Returns a list of stale entries with suggested replacements.
/// Uses stored IPs for exact matching when available, falls back to name similarity.
pub fn validate_config(config: &Config, devices: &[Device]) -> Vec<StaleEntry> {
    let device_names: Vec<&str> = devices.iter().map(|d| d.short_name.as_str()).collect();
    let mut stale = Vec::new();

    let find_suggestions = |name: &str| -> Vec<String> {
        // First try: exact match by stored IP
        if let Some(stored_ip) = config.device_ips.get(name) {
            if let Some(device) = devices.iter().find(|d| &d.ip == stored_ip) {
                // IP matches a device with a different name — that's our rename
                if !device.short_name.eq_ignore_ascii_case(name) {
                    return vec![device.short_name.clone()];
                }
            }
        }

        // Fallback: name similarity
        let nl = name.to_lowercase();
        devices
            .iter()
            .filter(|d| {
                let sn = d.short_name.to_lowercase();
                nl.split('-').any(|part| part.len() >= 3 && sn.contains(part))
                    || sn.split('-').any(|part| part.len() >= 3 && nl.contains(part))
            })
            .map(|d| d.short_name.clone())
            .collect()
    };

    // Check default_device
    if let Some(ref default) = config.default_device {
        if !device_names.iter().any(|n| n.eq_ignore_ascii_case(default)) {
            stale.push(StaleEntry {
                section: "default_device".into(),
                key: default.clone(),
                suggestions: find_suggestions(default),
            });
        }
    }

    // Check alias targets
    for (alias, target) in &config.aliases {
        if !device_names.iter().any(|n| n.eq_ignore_ascii_case(target)) {
            stale.push(StaleEntry {
                section: format!("aliases.{}", alias),
                key: target.clone(),
                suggestions: find_suggestions(target),
            });
        }
    }

    // Check user keys
    for key in config.users.keys() {
        if !device_names.iter().any(|n| n.eq_ignore_ascii_case(key)) {
            stale.push(StaleEntry {
                section: "users".into(),
                key: key.clone(),
                suggestions: find_suggestions(key),
            });
        }
    }

    // Check os_overrides keys
    for key in config.os_overrides.keys() {
        if !device_names.iter().any(|n| n.eq_ignore_ascii_case(key)) {
            stale.push(StaleEntry {
                section: "os_overrides".into(),
                key: key.clone(),
                suggestions: find_suggestions(key),
            });
        }
    }

    stale
}

/// Rename a device key across all config sections.
#[allow(dead_code)]
pub fn rename_device(old_name: &str, new_name: &str) -> Result<()> {
    let mut config = load()?.unwrap_or_default();
    let mut changed = false;

    if config.default_device.as_deref() == Some(old_name) {
        config.default_device = Some(new_name.to_string());
        changed = true;
    }

    // Update alias targets
    for target in config.aliases.values_mut() {
        if target == old_name {
            *target = new_name.to_string();
            changed = true;
        }
    }

    // Update users key
    if let Some(user) = config.users.remove(old_name) {
        config.users.insert(new_name.to_string(), user);
        changed = true;
    }

    // Update os_overrides key
    if let Some(os) = config.os_overrides.remove(old_name) {
        config.os_overrides.insert(new_name.to_string(), os);
        changed = true;
    }

    // Update device_ips key
    if let Some(ip) = config.device_ips.remove(old_name) {
        config.device_ips.insert(new_name.to_string(), ip);
        changed = true;
    }

    if changed {
        save(&config)?;
    }
    Ok(())
}

pub fn handle_command(cmd: ConfigCommands) -> Result<()> {
    match cmd {
        ConfigCommands::SetDefault { device } => {
            let mut config = load()?.unwrap_or_default();
            config.default_device = Some(device.clone());
            save(&config)?;
            println!("{} Default device set to: {}", "✓".green(), device.bold());
        }
        ConfigCommands::Alias { name, device } => {
            let mut config = load()?.unwrap_or_default();
            config.aliases.insert(name.clone(), device.clone());
            save(&config)?;
            println!(
                "{} Alias '{}' -> '{}'",
                "✓".green(),
                name.bold(),
                device
            );
        }
        ConfigCommands::SetUser { device, user } => {
            let mut config = load()?.unwrap_or_default();
            config.users.insert(device.clone(), user.clone());
            save(&config)?;
            println!(
                "{} SSH user for '{}' set to: {}",
                "✓".green(),
                device.bold(),
                user
            );
        }
        ConfigCommands::SetOs { device, os } => {
            let mut config = load()?.unwrap_or_default();
            if device == "local" {
                config.local_os = Some(os.clone());
                save(&config)?;
                println!("{} Local OS set to: {}", "✓".green(), os.bold());
            } else {
                config.os_overrides.insert(device.clone(), os.clone());
                save(&config)?;
                println!(
                    "{} OS for '{}' set to: {}",
                    "✓".green(),
                    device.bold(),
                    os
                );
            }
        }
        ConfigCommands::Show => {
            match load()? {
                Some(config) => {
                    println!("{}", "Tailshare Config".bold());
                    println!("{}", "─".repeat(40));
                    println!(
                        "  Default device: {}",
                        config
                            .default_device
                            .as_deref()
                            .unwrap_or("(not set)")
                    );
                    println!(
                        "  Local OS: {}",
                        config.local_os.as_deref().unwrap_or("(auto-detect)")
                    );
                    if !config.os_overrides.is_empty() {
                        println!("  OS overrides:");
                        for (device, os) in &config.os_overrides {
                            println!("    {} -> {}", device.bold(), os);
                        }
                    }
                    if config.users.is_empty() {
                        println!("  SSH users: (none)");
                    } else {
                        println!("  SSH users:");
                        for (device, user) in &config.users {
                            println!("    {} -> {}", device.bold(), user);
                        }
                    }
                    if config.aliases.is_empty() {
                        println!("  Aliases: (none)");
                    } else {
                        println!("  Aliases:");
                        for (alias, device) in &config.aliases {
                            println!("    {} -> {}", alias.bold(), device);
                        }
                    }
                    println!("\n  Config file: {}", config_path().display());
                }
                None => {
                    println!("No configuration found. Run 'tailshare config set-default <device>' to get started.");
                }
            }
        }
        ConfigCommands::Doctor => {
            unreachable!("Doctor is handled in main.rs");
        }
    }
    Ok(())
}
