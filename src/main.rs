mod cli;
mod clipboard;
mod config;
mod file;
mod platform;
mod ssh;
mod sync;
mod tailscale;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Devices => {
            tailscale::list_devices().await?;
        }
        Commands::Send { device } => {
            let target = resolve_device(&device).await?;
            clipboard::send(&target).await?;
        }
        Commands::Get { device } => {
            let target = resolve_device(&device).await?;
            clipboard::get(&target).await?;
        }
        Commands::Watch { device, interval } => {
            let target = resolve_device(&device).await?;
            clipboard::watch(&target, interval).await?;
        }
        Commands::FileSend { path, device, dest } => {
            let target = resolve_device(&device).await?;
            file::send_file(&target, &path, dest.as_deref()).await?;
        }
        Commands::Setup { device } => {
            let target = resolve_device(&device).await?;
            ssh::setup(&target).await?;
        }
        Commands::Sync { old_name, new_name, dry_run } => {
            sync::run_sync(old_name.as_deref(), new_name.as_deref(), dry_run)?;
        }
        Commands::Config(cli::ConfigCommands::Doctor) => {
            run_doctor()?;
        }
        Commands::Config(config_cmd) => {
            config::handle_command(config_cmd)?;
        }
    }

    Ok(())
}

async fn resolve_device(device: &str) -> Result<tailscale::Device> {
    let result = if let Some(cfg) = config::load()? {
        if let Some(alias) = cfg.aliases.get(device) {
            tailscale::find_device(alias).await
        } else if device == "default" {
            if let Some(default) = &cfg.default_device {
                tailscale::find_device(default).await
            } else {
                anyhow::bail!("No default device set. Run: tailshare config set-default <device>");
            }
        } else {
            tailscale::find_device(device).await
        }
    } else if device == "default" {
        anyhow::bail!("No default device set. Run: tailshare config set-default <device>");
    } else {
        tailscale::find_device(device).await
    };

    // Record the IP for future name-change detection
    if let Ok(ref dev) = result {
        let _ = config::record_device_ip(&dev.short_name, &dev.ip);
    }

    result
}

fn run_doctor() -> Result<()> {
    use colored::Colorize;

    let devices = tailscale::get_all_devices()?;

    // Backfill IPs for existing config entries before checking
    let backfilled = config::backfill_device_ips(&devices)?;
    if backfilled > 0 {
        println!(
            "{} Recorded IPs for {} device(s) (for future rename detection)\n",
            "✓".green(),
            backfilled
        );
    }

    let cfg = match config::load()? {
        Some(cfg) => cfg,
        None => {
            println!("{} No configuration found. Nothing to check.", "✓".green());
            return Ok(());
        }
    };

    let stale = config::validate_config(&cfg, &devices);

    if stale.is_empty() {
        println!(
            "{} All config entries match devices on your tailnet.",
            "✓".green().bold()
        );
        return Ok(());
    }

    println!(
        "{} Found {} stale device reference(s) in config:\n",
        "!".yellow().bold(),
        stale.len()
    );

    for entry in &stale {
        println!(
            "  {} [{}] '{}' not found on tailnet",
            "✗".red(),
            entry.section.bold(),
            entry.key
        );
        if entry.suggestions.is_empty() {
            println!("    No similar devices found.");
        } else {
            println!("    Did you mean?");
            for s in &entry.suggestions {
                println!("      - {}", s.green());
            }
        }
        println!();
    }

    println!("To fix automatically:");
    println!("  tailshare sync              # auto-fix unambiguous renames");
    println!("  tailshare sync old-name new-name  # rename a specific device");
    println!(
        "\nOr edit manually: {}",
        dirs::config_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap().join(".config"))
            .join("tailshare")
            .join("config.toml")
            .display()
    );

    Ok(())
}
