use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::cli::ConfigCommands;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub default_device: Option<String>,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    #[serde(default)]
    pub users: HashMap<String, String>,
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
    }
    Ok(())
}
