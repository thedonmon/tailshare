use anyhow::Result;
use colored::Colorize;
use std::time::Duration;
use tokio::time;

use crate::platform;
use crate::ssh;
use crate::tailscale::Device;

fn get_remote_clipboard(device: &Device) -> Result<String> {
    let cmd = platform::remote_paste_cmd(&device.os);
    ssh::run_command(device, cmd)
}

fn set_remote_clipboard(device: &Device, content: &str) -> Result<()> {
    let cmd = platform::remote_copy_cmd(&device.os);
    ssh::pipe_to_command(device, cmd, content)
}

fn truncate_preview(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.replace('\n', " ").to_string()
    } else {
        format!("{}...", s[..max].replace('\n', " "))
    }
}

pub async fn send(device: &Device) -> Result<()> {
    let content = platform::get_local_clipboard()?;
    if content.is_empty() {
        println!("{}", "Clipboard is empty, nothing to send.".yellow());
        return Ok(());
    }

    set_remote_clipboard(device, &content)?;

    let preview = truncate_preview(&content, 50);
    println!(
        "{} Sent to {}: \"{}\"",
        "✓".green(),
        device.name.bold(),
        preview.dimmed()
    );
    Ok(())
}

pub async fn get(device: &Device) -> Result<()> {
    let content = get_remote_clipboard(device)?;
    if content.is_empty() {
        println!(
            "{}",
            format!("Clipboard on {} is empty.", device.name).yellow()
        );
        return Ok(());
    }

    platform::set_local_clipboard(&content)?;

    let preview = truncate_preview(&content, 50);
    println!(
        "{} Got from {}: \"{}\"",
        "✓".green(),
        device.name.bold(),
        preview.dimmed()
    );
    Ok(())
}

pub async fn watch(device: &Device, interval_secs: u64) -> Result<()> {
    println!(
        "Watching clipboard between {} and {} (every {}s, Ctrl+C to stop)",
        "this device".cyan(),
        device.name.bold(),
        interval_secs
    );

    let mut last_local = platform::get_local_clipboard().unwrap_or_default();
    let mut last_remote = get_remote_clipboard(device).unwrap_or_default();

    let interval = Duration::from_secs(interval_secs);

    loop {
        time::sleep(interval).await;

        if let Ok(current_local) = platform::get_local_clipboard() {
            if current_local != last_local && !current_local.is_empty() {
                if let Ok(()) = set_remote_clipboard(device, &current_local) {
                    let preview = truncate_preview(&current_local, 40);
                    println!(
                        "  {} -> {}: \"{}\"",
                        "local".cyan(),
                        device.name.bold(),
                        preview.dimmed()
                    );
                    last_local = current_local;
                    last_remote = last_local.clone();
                    continue;
                }
            }
        }

        if let Ok(current_remote) = get_remote_clipboard(device) {
            if current_remote != last_remote && !current_remote.is_empty() {
                if let Ok(()) = platform::set_local_clipboard(&current_remote) {
                    let preview = truncate_preview(&current_remote, 40);
                    println!(
                        "  {} -> {}: \"{}\"",
                        device.name.bold(),
                        "local".cyan(),
                        preview.dimmed()
                    );
                    last_remote = current_remote;
                    last_local = last_remote.clone();
                }
            }
        }
    }
}
