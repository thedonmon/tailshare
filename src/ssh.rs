use anyhow::{Context, Result};
use colored::Colorize;
use std::io::Write;
use std::process::Command;

use crate::tailscale::Device;

pub fn ssh_target(device: &Device) -> String {
    let host = if !device.short_name.is_empty() {
        &device.short_name
    } else if !device.dns_name.is_empty() {
        &device.dns_name
    } else {
        &device.ip
    };

    // Look up user by short_name (the stable kebab-case identifier)
    if let Some(cfg) = crate::config::load().ok().flatten() {
        if let Some(user) = cfg.users.get(&device.short_name) {
            return format!("{}@{}", user, host);
        }
    }

    host.clone()
}

pub fn run_command(device: &Device, cmd: &str) -> Result<String> {
    let target = ssh_target(device);
    let output = Command::new("ssh")
        .args([
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=5",
            "-o", "StrictHostKeyChecking=accept-new",
            &target,
            cmd,
        ])
        .output()
        .context(format!("Failed to SSH to {}", device.name))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Permission denied") {
            anyhow::bail!(
                "SSH auth failed for {}. Run: tailshare setup {}",
                device.name,
                device.short_name
            );
        }
        anyhow::bail!("SSH command failed on {}: {}", device.name, stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn scp_to(device: &Device, local_path: &str, remote_path: &str) -> Result<()> {
    let target = ssh_target(device);
    let remote_dest = format!("{}:{}", target, remote_path);
    let status = Command::new("scp")
        .args([
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=5",
            local_path,
            &remote_dest,
        ])
        .status()
        .context(format!("Failed to SCP to {}", device.name))?;

    if !status.success() {
        anyhow::bail!("SCP failed to {}", device.name);
    }
    Ok(())
}

pub fn scp_from(device: &Device, remote_path: &str, local_path: &str) -> Result<()> {
    let target = ssh_target(device);
    let remote_src = format!("{}:{}", target, remote_path);
    let status = Command::new("scp")
        .args([
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=5",
            &remote_src,
            local_path,
        ])
        .status()
        .context(format!("Failed to SCP from {}", device.name))?;

    if !status.success() {
        anyhow::bail!("SCP from {} failed", device.name);
    }
    Ok(())
}

pub fn pipe_to_command(device: &Device, cmd: &str, input: &str) -> Result<()> {
    let target = ssh_target(device);
    let mut child = Command::new("ssh")
        .args([
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=5",
            "-o", "StrictHostKeyChecking=accept-new",
            &target,
            cmd,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context(format!("Failed to SSH to {}", device.name))?;

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())?;

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("SSH command failed on {}: {}", device.name, stderr);
    }

    Ok(())
}

pub async fn setup(device: &Device) -> Result<()> {
    println!(
        "Setting up SSH key auth with {} ({})...",
        device.name.bold(),
        device.short_name.dimmed()
    );

    // Record the device's stable IP for future name-change detection
    let _ = crate::config::record_device_ip(&device.short_name, &device.ip);

    let key_path = dirs::home_dir()
        .unwrap()
        .join(".ssh")
        .join(format!("tailshare_{}", device.short_name));

    let key_path_str = key_path.to_string_lossy().to_string();
    let pub_key_path = format!("{}.pub", key_path_str);

    // Generate key if it doesn't exist
    if !key_path.exists() {
        println!("  Generating SSH key...");
        let status = Command::new("ssh-keygen")
            .args([
                "-t", "ed25519",
                "-f", &key_path_str,
                "-N", "",
                "-C", &format!("tailshare-{}", device.short_name),
            ])
            .status()
            .context("Failed to generate SSH key")?;

        if !status.success() {
            anyhow::bail!("ssh-keygen failed");
        }
        println!("  {} Key generated: {}", "✓".green(), key_path_str);
    } else {
        println!("  Key already exists: {}", key_path_str);
    }

    // Copy key to remote (uses configured user via ssh_target)
    let target = ssh_target(device);
    println!(
        "  Copying key to {} (you may be prompted for password)...",
        target
    );
    let status = Command::new("ssh-copy-id")
        .args(["-i", &pub_key_path, &target])
        .status()
        .context("Failed to run ssh-copy-id")?;

    if !status.success() {
        anyhow::bail!(
            "ssh-copy-id failed. Make sure you can SSH to {} or set the user first:\n  tailshare config set-user {} <username>",
            device.name,
            device.short_name
        );
    }

    // Add to SSH config
    let ssh_config_path = dirs::home_dir().unwrap().join(".ssh").join("config");
    let host = &device.short_name;
    let user_line = if let Some(cfg) = crate::config::load().ok().flatten() {
        cfg.users.get(&device.short_name).map(|u| format!("\n    User {}", u))
    } else {
        None
    };
    let config_entry = format!(
        "\n# Added by tailshare\nHost {}{}\n    HostName {}\n    IdentityFile {}\n    ControlMaster auto\n    ControlPath ~/.ssh/sockets/%r@%h-%p\n    ControlPersist 600\n",
        host,
        user_line.unwrap_or_default(),
        host,
        key_path_str
    );

    let existing = std::fs::read_to_string(&ssh_config_path).unwrap_or_default();
    if !existing.contains(&format!("Host {}", host)) {
        let sockets_dir = dirs::home_dir().unwrap().join(".ssh").join("sockets");
        std::fs::create_dir_all(&sockets_dir)?;

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&ssh_config_path)?;
        file.write_all(config_entry.as_bytes())?;
        println!("  {} SSH config updated", "✓".green());
    } else {
        println!("  SSH config entry already exists");
    }

    // Test connection
    println!("  Testing connection...");
    let output = Command::new("ssh")
        .args([
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=5",
            &target,
            "echo tailshare-ok",
        ])
        .output()?;

    if output.status.success() {
        println!(
            "\n{} Setup complete! You can now use:\n  tailshare send {}\n  tailshare get {}",
            "✓".green().bold(),
            device.short_name,
            device.short_name
        );
    } else {
        println!(
            "\n{} Connection test failed. You may need to check SSH settings on {}.",
            "✗".red().bold(),
            device.name
        );
    }

    Ok(())
}
