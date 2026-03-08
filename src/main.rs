mod cli;
mod clipboard;
mod config;
mod file;
mod platform;
mod ssh;
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
        Commands::Config(config_cmd) => {
            config::handle_command(config_cmd)?;
        }
    }

    Ok(())
}

async fn resolve_device(device: &str) -> Result<tailscale::Device> {
    if let Some(cfg) = config::load()? {
        if let Some(alias) = cfg.aliases.get(device) {
            return tailscale::find_device(alias).await;
        }
    }
    if device == "default" {
        if let Some(cfg) = config::load()? {
            if let Some(default) = &cfg.default_device {
                return tailscale::find_device(default).await;
            }
        }
        anyhow::bail!("No default device set. Run: tailshare config set-default <device>");
    }
    tailscale::find_device(device).await
}
