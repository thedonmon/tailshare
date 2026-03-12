use anyhow::{Context, Result};
use colored::Colorize;

use crate::config;
use crate::tailscale;

/// Preview what a rename would change, without writing anything.
fn preview_rename(old_name: &str, new_name: &str) -> Result<Vec<String>> {
    let mut changes = Vec::new();

    // 1. Check tailshare config
    if let Some(cfg) = config::load()? {
        if cfg.default_device.as_deref() == Some(old_name) {
            changes.push(format!("  config: default_device '{}' -> '{}'", old_name, new_name));
        }
        for (alias, target) in &cfg.aliases {
            if target == old_name {
                changes.push(format!("  config: alias '{}' target '{}' -> '{}'", alias, old_name, new_name));
            }
        }
        if cfg.users.contains_key(old_name) {
            changes.push(format!("  config: users['{}'] -> users['{}']", old_name, new_name));
        }
        if cfg.os_overrides.contains_key(old_name) {
            changes.push(format!("  config: os_overrides['{}'] -> os_overrides['{}']", old_name, new_name));
        }
        if cfg.device_ips.contains_key(old_name) {
            changes.push(format!("  config: device_ips['{}'] -> device_ips['{}']", old_name, new_name));
        }
    }

    // 2. Check ~/.ssh/config
    let ssh_config_path = dirs::home_dir()
        .context("No home directory")?
        .join(".ssh")
        .join("config");

    if ssh_config_path.exists() {
        let content = std::fs::read_to_string(&ssh_config_path)?;
        let new_content = rename_ssh_config_entry(&content, old_name, new_name);
        if content != new_content {
            changes.push(format!("  ~/.ssh/config: Host '{}' -> '{}'  (# Added by tailshare block only)", old_name, new_name));
        }
    }

    // 3. Check SSH key files
    let ssh_dir = dirs::home_dir()
        .context("No home directory")?
        .join(".ssh");

    let old_key = ssh_dir.join(format!("tailshare_{}", old_name));
    let new_key = ssh_dir.join(format!("tailshare_{}", new_name));

    if old_key.exists() && !new_key.exists() {
        changes.push(format!("  rename: ~/.ssh/tailshare_{} -> ~/.ssh/tailshare_{}", old_name, new_name));
        changes.push(format!("  rename: ~/.ssh/tailshare_{}.pub -> ~/.ssh/tailshare_{}.pub", old_name, new_name));
        changes.push(format!("  ~/.ssh/config: IdentityFile path updated  (# Added by tailshare block only)"));
    }

    if changes.is_empty() {
        changes.push("  (no changes needed)".into());
    }

    Ok(changes)
}

/// Rename a device everywhere: tailshare config, ~/.ssh/config, and SSH key files.
/// Does NOT touch authorized_keys on the remote — the key pair is still valid.
pub fn rename_device(old_name: &str, new_name: &str) -> Result<()> {
    // 1. Update tailshare config (~/.config/tailshare/config.toml)
    config::rename_device(old_name, new_name)?;

    // 2. Update ~/.ssh/config — only tailshare-managed entries
    let ssh_config_path = dirs::home_dir()
        .context("No home directory")?
        .join(".ssh")
        .join("config");

    if ssh_config_path.exists() {
        let content = std::fs::read_to_string(&ssh_config_path)?;
        let new_content = rename_ssh_config_entry(&content, old_name, new_name);
        if content != new_content {
            std::fs::write(&ssh_config_path, &new_content)?;
        }
    }

    // 3. Rename SSH key files (only tailshare-prefixed keys)
    let ssh_dir = dirs::home_dir()
        .context("No home directory")?
        .join(".ssh");

    let old_key = ssh_dir.join(format!("tailshare_{}", old_name));
    let old_pub = ssh_dir.join(format!("tailshare_{}.pub", old_name));
    let new_key = ssh_dir.join(format!("tailshare_{}", new_name));
    let new_pub = ssh_dir.join(format!("tailshare_{}.pub", new_name));

    let keys_renamed = old_key.exists() && !new_key.exists();
    if keys_renamed {
        std::fs::rename(&old_key, &new_key)?;
    }
    if old_pub.exists() && !new_pub.exists() {
        std::fs::rename(&old_pub, &new_pub)?;
    }

    // 4. Update IdentityFile path in the tailshare SSH config block
    //    (only within "# Added by tailshare" blocks, not globally)
    if keys_renamed && ssh_config_path.exists() {
        let content = std::fs::read_to_string(&ssh_config_path)?;
        let updated = replace_in_tailshare_blocks(
            &content,
            new_name, // match the block we just renamed in step 2
            &format!("tailshare_{}", old_name),
            &format!("tailshare_{}", new_name),
        );
        if content != updated {
            std::fs::write(&ssh_config_path, &updated)?;
        }
    }

    Ok(())
}

/// Rename a tailshare-managed Host block in SSH config.
/// Only touches blocks preceded by "# Added by tailshare".
fn rename_ssh_config_entry(content: &str, old_name: &str, new_name: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        // Look for "# Added by tailshare" followed by "Host <old_name>"
        if lines[i].trim() == "# Added by tailshare"
            && i + 1 < lines.len()
            && lines[i + 1].trim() == format!("Host {}", old_name)
        {
            // Emit the comment
            result.push(lines[i].to_string());
            i += 1;
            // Rewrite the Host line
            result.push(lines[i].replace(
                &format!("Host {}", old_name),
                &format!("Host {}", new_name),
            ));
            i += 1;
            // Rewrite indented options within this block
            while i < lines.len() && (lines[i].starts_with("    ") || lines[i].starts_with('\t') || lines[i].is_empty()) {
                if lines[i].is_empty() {
                    result.push(String::new());
                    i += 1;
                    break; // empty line ends the block
                }
                // Replace HostName if it matches old_name
                let line = if lines[i].trim().starts_with("HostName") {
                    lines[i].replace(
                        &format!("HostName {}", old_name),
                        &format!("HostName {}", new_name),
                    )
                } else {
                    lines[i].to_string()
                };
                result.push(line);
                i += 1;
            }
        } else {
            result.push(lines[i].to_string());
            i += 1;
        }
    }

    result.join("\n")
}

/// Replace a string only within a specific tailshare-managed Host block in SSH config.
/// `host_name` is the Host value to match, `old` and `new` are the strings to replace within that block.
fn replace_in_tailshare_blocks(content: &str, host_name: &str, old: &str, new: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        if lines[i].trim() == "# Added by tailshare"
            && i + 1 < lines.len()
            && lines[i + 1].trim() == format!("Host {}", host_name)
        {
            result.push(lines[i].to_string());
            i += 1;
            // Process the block — replace old->new only in indented lines
            while i < lines.len() {
                if lines[i].is_empty() {
                    result.push(String::new());
                    i += 1;
                    break;
                }
                result.push(lines[i].replace(old, new));
                i += 1;
            }
        } else {
            result.push(lines[i].to_string());
            i += 1;
        }
    }

    result.join("\n")
}

/// Auto-sync: detect stale entries and fix them.
/// With no args, finds stale entries with unambiguous suggestions and fixes them.
/// Returns (fixed_count, ambiguous_count).
pub fn auto_sync(dry_run: bool) -> Result<(usize, usize)> {
    let devices = tailscale::get_all_devices()?;

    // Backfill IPs for any config entries that don't have them yet
    // (handles configs created before IP tracking was added)
    if !dry_run {
        let backfilled = config::backfill_device_ips(&devices)?;
        if backfilled > 0 {
            println!(
                "  {} Recorded IPs for {} existing device(s)",
                "✓".green(),
                backfilled
            );
        }
    } else {
        // In dry-run, still show what would be backfilled
        let cfg = config::load()?.unwrap_or_default();
        let mut would_backfill = 0;
        let mut referenced: std::collections::HashSet<String> = std::collections::HashSet::new();
        if let Some(ref d) = cfg.default_device { referenced.insert(d.clone()); }
        for t in cfg.aliases.values() { referenced.insert(t.clone()); }
        for k in cfg.users.keys() { referenced.insert(k.clone()); }
        for k in cfg.os_overrides.keys() { referenced.insert(k.clone()); }
        for name in &referenced {
            if !cfg.device_ips.contains_key(name) {
                if devices.iter().any(|d| d.short_name.eq_ignore_ascii_case(name)) {
                    would_backfill += 1;
                }
            }
        }
        if would_backfill > 0 {
            println!(
                "  {} Would record IPs for {} existing device(s)",
                "~".cyan(),
                would_backfill
            );
        }
    }

    // Re-load config after backfill so validate_config sees the new IPs
    let cfg = match config::load()? {
        Some(cfg) => cfg,
        None => return Ok((0, 0)),
    };

    let stale = config::validate_config(&cfg, &devices);

    if stale.is_empty() {
        return Ok((0, 0));
    }

    let mut fixed = 0;
    let mut ambiguous = 0;

    // Deduplicate: multiple stale entries may reference the same old name
    let mut seen_renames: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in &stale {
        if seen_renames.contains(&entry.key) {
            continue;
        }

        if entry.suggestions.len() == 1 {
            let new_name = &entry.suggestions[0];
            if dry_run {
                println!(
                    "  {} '{}' -> '{}' (dry run)",
                    "~".cyan(),
                    entry.key,
                    new_name.cyan()
                );
                for line in preview_rename(&entry.key, new_name)? {
                    println!("{}", line.dimmed());
                }
            } else {
                println!(
                    "  {} '{}' -> '{}'",
                    "✓".green(),
                    entry.key,
                    new_name.green()
                );
                rename_device(&entry.key, new_name)?;
            }
            seen_renames.insert(entry.key.clone());
            fixed += 1;
        } else {
            ambiguous += 1;
            if entry.suggestions.is_empty() {
                println!(
                    "  {} '{}' — no matching device found, skipping",
                    "?".yellow(),
                    entry.key
                );
            } else {
                println!(
                    "  {} '{}' — multiple matches ({}), use: tailshare sync {} <new-name>",
                    "?".yellow(),
                    entry.key,
                    entry.suggestions.join(", "),
                    entry.key
                );
            }
        }
    }

    Ok((fixed, ambiguous))
}

/// Test SSH connection to a device after sync.
fn test_connection(device: &tailscale::Device) -> bool {
    let target = crate::ssh::ssh_target(device);
    let output = std::process::Command::new("ssh")
        .args([
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=5",
            "-o", "StrictHostKeyChecking=accept-new",
            &target,
            "echo tailshare-ok",
        ])
        .output();

    matches!(output, Ok(o) if o.status.success())
}

/// Print a reminder to sync on other machines too.
fn print_remote_reminder() {
    println!();
    println!(
        "{} Remember to run {} on your other machines too.",
        "!".yellow().bold(),
        "tailshare sync".bold()
    );
}

/// Run the full sync flow.
pub fn run_sync(old_name: Option<&str>, new_name: Option<&str>, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("{}", "Dry run — no changes will be made.\n".cyan().bold());
    }

    match (old_name, new_name) {
        (Some(old), Some(new)) => {
            // Verify new_name is a real device
            let devices = tailscale::get_all_devices()?;
            let target = devices.iter().find(|d| d.short_name.eq_ignore_ascii_case(new));

            match target {
                Some(device) => {
                    let device = device.clone();

                    if dry_run {
                        println!("Would sync '{}' -> '{}':", old, new.cyan());
                        for line in preview_rename(old, new)? {
                            println!("{}", line);
                        }
                        println!("\nRun without --dry-run to apply.");
                    } else {
                        println!(
                            "Syncing '{}' -> '{}'...",
                            old.bold(),
                            new.green().bold()
                        );
                        rename_device(old, new)?;
                        // Record the IP under the new name
                        let _ = config::record_device_ip(new, &device.ip);
                        println!("  {} Config and SSH updated", "✓".green());

                        // Test connection
                        print!("  Testing SSH connection...");
                        if test_connection(&device) {
                            println!(" {}", "OK".green());
                        } else {
                            println!(" {}", "FAILED".red());
                            println!(
                                "  SSH may need re-setup: tailshare setup {}",
                                new
                            );
                        }

                        println!("\n{} Sync complete!", "✓".green().bold());
                        print_remote_reminder();
                    }
                }
                None => {
                    anyhow::bail!(
                        "Device '{}' not found on your tailnet. Run 'tailshare devices' to see available devices.",
                        new
                    );
                }
            }
        }
        (Some(old), None) => {
            anyhow::bail!(
                "Missing new name. Usage: tailshare sync {} <new-name>",
                old
            );
        }
        _ => {
            // Auto-sync mode
            println!("{}", "Auto-syncing device names...".bold());
            println!();
            let (fixed, ambiguous) = auto_sync(dry_run)?;

            if fixed == 0 && ambiguous == 0 {
                println!(
                    "{} Everything is up to date!",
                    "✓".green().bold()
                );
            } else {
                println!();
                if dry_run {
                    println!(
                        "{} Would fix {} device name(s). Run without --dry-run to apply.",
                        "~".cyan().bold(),
                        fixed
                    );
                } else if fixed > 0 {
                    println!(
                        "{} Fixed {} device name(s)",
                        "✓".green().bold(),
                        fixed
                    );
                }
                if ambiguous > 0 {
                    println!(
                        "{} {} device(s) need manual resolution (see above)",
                        "!".yellow().bold(),
                        ambiguous
                    );
                }
                if !dry_run && fixed > 0 {
                    print_remote_reminder();
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rename_ssh_config_entry() {
        let input = r#"# Some other config
Host myserver
    HostName myserver.example.com

# Added by tailshare
Host old-mac
    HostName old-mac
    IdentityFile ~/.ssh/tailshare_old-mac
    ControlMaster auto
    ControlPath ~/.ssh/sockets/%r@%h-%p
    ControlPersist 600

# Added by tailshare
Host other-device
    HostName other-device
    IdentityFile ~/.ssh/tailshare_other-device
"#;

        let result = rename_ssh_config_entry(input, "old-mac", "new-mac");

        assert!(result.contains("Host new-mac"));
        assert!(result.contains("HostName new-mac"));
        assert!(!result.contains("Host old-mac"));
        // other-device should be untouched
        assert!(result.contains("Host other-device"));
        assert!(result.contains("HostName other-device"));
        // non-tailshare entry should be untouched
        assert!(result.contains("Host myserver"));
    }

    #[test]
    fn test_rename_leaves_non_tailshare_entries() {
        let input = r#"Host old-mac
    HostName old-mac.example.com
"#;
        let result = rename_ssh_config_entry(input, "old-mac", "new-mac");
        // Should NOT be renamed because there's no "# Added by tailshare" comment
        assert!(result.contains("Host old-mac"));
    }

    #[test]
    fn test_replace_in_tailshare_blocks_scoped() {
        let input = r#"# Some unrelated entry that happens to mention tailshare_old-mac
Host work-server
    IdentityFile ~/.ssh/tailshare_old-mac
    HostName work.example.com

# Added by tailshare
Host new-mac
    HostName new-mac
    IdentityFile ~/.ssh/tailshare_old-mac
    ControlMaster auto
"#;
        let result = replace_in_tailshare_blocks(
            input,
            "new-mac",
            "tailshare_old-mac",
            "tailshare_new-mac",
        );

        // The tailshare block should be updated
        assert!(result.contains("IdentityFile ~/.ssh/tailshare_new-mac"));
        // The unrelated entry should NOT be touched
        let work_block: String = result
            .lines()
            .skip_while(|l| !l.contains("Host work-server"))
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            work_block.contains("tailshare_old-mac"),
            "Non-tailshare block should be untouched, got: {}",
            work_block
        );
    }

    #[test]
    fn test_identity_file_rename_only_in_correct_block() {
        // Two tailshare blocks, only one should be updated
        let input = r#"# Added by tailshare
Host device-a
    HostName device-a
    IdentityFile ~/.ssh/tailshare_device-a

# Added by tailshare
Host device-b
    HostName device-b
    IdentityFile ~/.ssh/tailshare_device-b
"#;
        let result = replace_in_tailshare_blocks(
            input,
            "device-a",
            "tailshare_device-a",
            "tailshare_new-a",
        );

        assert!(result.contains("IdentityFile ~/.ssh/tailshare_new-a"));
        // device-b should be untouched
        assert!(result.contains("IdentityFile ~/.ssh/tailshare_device-b"));
    }
}
