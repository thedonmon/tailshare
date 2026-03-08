use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

use crate::ssh;
use crate::tailscale::Device;

pub async fn send_file(device: &Device, path: &str, dest: Option<&str>) -> Result<()> {
    let local_path = Path::new(path);
    if !local_path.exists() {
        anyhow::bail!("File not found: {}", path);
    }

    let file_name = local_path
        .file_name()
        .context("Invalid file path")?
        .to_string_lossy();

    let remote_path = match dest {
        Some(d) => d.to_string(),
        None => format!("~/Downloads/{}", file_name),
    };

    // Ensure remote Downloads directory exists
    if dest.is_none() {
        ssh::run_command(device, "mkdir -p ~/Downloads")?;
    }

    let metadata = std::fs::metadata(local_path)?;
    let size = metadata.len();
    let size_display = format_size(size);

    println!(
        "Sending {} ({}) to {}:{}",
        file_name.bold(),
        size_display,
        device.name.bold(),
        remote_path
    );

    ssh::scp_to(device, path, &remote_path)?;

    println!(
        "{} File sent: {} -> {}:{}",
        "✓".green(),
        file_name,
        device.name.bold(),
        remote_path
    );

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
