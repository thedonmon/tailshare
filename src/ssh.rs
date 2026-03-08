use anyhow::{Context, Result};
use colored::Colorize;
use std::io::Write;
use std::process::Command;

use crate::tailscale::Device;

pub fn ssh_target(device: &Device) -> String {
    // Prefer MagicDNS short name, then FQDN, then IP
    let host = if !device.short_name.is_empty() {
        &device.short_name
    } else if !device.dns_name.is_empty() {
        &device.dns_name
    } else {
        &device.ip
    };

    // Check if there's a user configured for this device
    if let Some(cfg) = crate::config::load().ok().flatten() {
        if let Some(user) = cfg.users.get(&device.name) {
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
                device.name
            );
        }
        anyhow::bail!("SSH command failed on {}: {}", device.name, stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn run_command_bytes(device: &Device, cmd: &str) -> Result<Vec<u8>> {
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
        anyhow::bail!("SSH command failed on {}: {}", device.name, stderr);
    }

    Ok(output.stdout)
}

pub fn pipe_bytes_to_command(device: &Device, cmd: &str, input: &[u8]) -> Result<()> {
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

    child.stdin.as_mut().unwrap().write_all(input)?;
    drop(child.stdin.take());

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("SSH command failed on {}: {}", device.name, stderr);
    }

    Ok(())
}

pub fn scp_to(device: &Device, local_path: &str, remote_path: &str) -> Result<()> {
    let target = ssh_target(device);
    // Extract user@host from target
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
        "Setting up SSH key auth with {}...",
        device.name.bold()
    );

    let key_path = dirs::home_dir()
        .unwrap()
        .join(".ssh")
        .join(format!("tailshare_{}", device.name));

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
                "-C", &format!("tailshare-{}", device.name),
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

    // Copy key to remote
    println!(
        "  Copying key to {} (you may be prompted for password)...",
        device.name
    );
    let status = Command::new("ssh-copy-id")
        .args(["-i", &pub_key_path, &device.dns_name])
        .status()
        .context("Failed to run ssh-copy-id")?;

    if !status.success() {
        anyhow::bail!("ssh-copy-id failed. Make sure you can SSH to {} with a password.", device.name);
    }

    // Add to SSH config
    let ssh_config_path = dirs::home_dir().unwrap().join(".ssh").join("config");
    let config_entry = format!(
        "\n# Added by tailshare\nHost {}\n    HostName {}\n    IdentityFile {}\n    ControlMaster auto\n    ControlPath ~/.ssh/sockets/%r@%h-%p\n    ControlPersist 600\n",
        device.name, device.dns_name, key_path_str
    );

    // Check if entry already exists
    let existing = std::fs::read_to_string(&ssh_config_path).unwrap_or_default();
    if !existing.contains(&format!("Host {}", device.name)) {
        // Ensure sockets dir exists
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
            &device.dns_name,
            "echo tailshare-ok",
        ])
        .output()?;

    if output.status.success() {
        println!(
            "\n{} Setup complete! You can now use:\n  tailshare send {}\n  tailshare get {}",
            "✓".green().bold(),
            device.name,
            device.name
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
