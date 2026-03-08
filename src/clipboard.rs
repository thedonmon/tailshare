use anyhow::Result;
use colored::Colorize;
use std::time::Duration;
use tokio::time;

use crate::platform::{self, ClipboardContent};
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
    let flat = s.replace('\n', " ");
    if flat.len() <= max {
        flat
    } else {
        let truncated: String = flat.chars().take(max).collect();
        format!("{}...", truncated)
    }
}

pub async fn send(device: &Device) -> Result<()> {
    let content = platform::get_local_clipboard_content()?;

    match content {
        ClipboardContent::Empty => {
            println!("{}", "Clipboard is empty, nothing to send.".yellow());
        }
        ClipboardContent::Text(text) => {
            set_remote_clipboard(device, &text)?;
            let preview = truncate_preview(&text, 50);
            println!(
                "{} Sent to {}: \"{}\"",
                "✓".green(),
                device.name.bold(),
                preview.dimmed()
            );
        }
        ClipboardContent::File(path) => {
            let file_name = std::path::Path::new(&path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            let metadata = std::fs::metadata(&path)?;
            let size = metadata.len();
            let size_display = if size < 1024 {
                format!("{}B", size)
            } else if size < 1024 * 1024 {
                format!("{:.1}KB", size as f64 / 1024.0)
            } else {
                format!("{:.1}MB", size as f64 / (1024.0 * 1024.0))
            };

            // Use a cache dir for clipboard sends (not Downloads)
            let home = ssh::run_command(device, "echo $HOME")?.trim().to_string();
            let cache_dir = format!("{}/.cache/tailshare", home);
            ssh::run_command(device, &format!("mkdir -p {}", cache_dir))?;
            let remote_path = format!("{}/{}", cache_dir, file_name);
            ssh::scp_to(device, &path, &remote_path)?;

            // Set file on remote clipboard so it shows in clipboard history
            if device.os == "macOS" {
                let expanded_path = &remote_path;
                // Check if it's an image — set as image clipboard
                let ext = std::path::Path::new(&path)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tiff") {
                    let script = format!(
                        r#"osascript -e 'set theFile to POSIX file "{}"' -e 'set the clipboard to (read theFile as «class PNGf»)'"#,
                        expanded_path
                    );
                    let _ = ssh::run_command(device, &script);
                } else {
                    // For non-image files, set the file reference on clipboard
                    let script = format!(
                        r#"osascript -e 'set the clipboard to POSIX file "{}"'"#,
                        expanded_path
                    );
                    let _ = ssh::run_command(device, &script);
                }
            }

            println!(
                "{} Sent file {} ({}) to {}:{}",
                "✓".green(),
                file_name.bold(),
                size_display,
                device.name.bold(),
                remote_path
            );
        }
        ClipboardContent::Image(data) => {
            let size_kb = data.len() / 1024;
            // Write image to local temp file, then SCP it (preserves binary data)
            let tmp_local = std::env::temp_dir().join("tailshare_send.png");
            std::fs::write(&tmp_local, &data)?;
            ssh::scp_to(device, &tmp_local.to_string_lossy(), "/tmp/tailshare_img.png")?;
            let _ = std::fs::remove_file(&tmp_local);

            // Set it as clipboard on remote macOS
            if device.os == "macOS" {
                ssh::run_command(
                    device,
                    r#"osascript -e 'set theFile to POSIX file "/tmp/tailshare_img.png"' -e 'set the clipboard to (read theFile as «class PNGf»)'"#,
                )?;
                ssh::run_command(device, "rm /tmp/tailshare_img.png")?;
            }
            println!(
                "{} Sent image ({}KB) to {}",
                "✓".green(),
                size_kb,
                device.name.bold()
            );
        }
    }

    Ok(())
}

pub async fn get(device: &Device) -> Result<()> {
    // Check if remote has an image on clipboard (macOS only)
    if device.os == "macOS" {
        let info = ssh::run_command(device, "osascript -e 'clipboard info'").unwrap_or_default();
        if info.contains("PNGf") || info.contains("TIFF") {
            // Extract image from remote clipboard
            ssh::run_command(
                device,
                r#"osascript -e 'set theFile to POSIX file "/tmp/tailshare_img.png"' -e 'set fRef to open for access theFile with write permission' -e 'set eof fRef to 0' -e 'write (the clipboard as «class PNGf») to fRef' -e 'close access fRef'"#,
            )?;
            // SCP the image back (preserves binary data)
            let tmp_local = std::env::temp_dir().join("tailshare_recv.png");
            let tmp_local_str = tmp_local.to_string_lossy().to_string();
            ssh::scp_from(device, "/tmp/tailshare_img.png", &tmp_local_str)?;
            ssh::run_command(device, "rm /tmp/tailshare_img.png")?;
            let image_data = std::fs::read(&tmp_local)?;
            let _ = std::fs::remove_file(&tmp_local);

            if !image_data.is_empty() {
                platform::set_local_image(&image_data)?;
                let size_kb = image_data.len() / 1024;
                println!(
                    "{} Got image ({}KB) from {}",
                    "✓".green(),
                    size_kb,
                    device.name.bold()
                );
                return Ok(());
            }
        }
    }

    // Fall back to text
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
