use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;

/// Returns (paste_cmd, paste_args) for reading clipboard on the local platform
fn paste_command() -> (&'static str, &'static [&'static str]) {
    if cfg!(target_os = "macos") {
        ("pbpaste", &[])
    } else if cfg!(target_os = "windows") {
        ("powershell.exe", &["-NoProfile", "-Command", "Get-Clipboard"])
    } else {
        // Linux: prefer wl-paste (Wayland), fall back to xclip (X11)
        if which_exists("wl-paste") {
            ("wl-paste", &[])
        } else {
            ("xclip", &["-selection", "clipboard", "-o"])
        }
    }
}

/// Returns (copy_cmd, copy_args) for writing clipboard on the local platform
fn copy_command() -> (&'static str, &'static [&'static str]) {
    if cfg!(target_os = "macos") {
        ("pbcopy", &[])
    } else if cfg!(target_os = "windows") {
        ("clip.exe", &[])
    } else {
        if which_exists("wl-copy") {
            ("wl-copy", &[])
        } else {
            ("xclip", &["-selection", "clipboard"])
        }
    }
}

fn which_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Returns the remote clipboard command based on the device's OS
pub fn remote_paste_cmd(os: &str) -> &'static str {
    match os {
        "macOS" => "pbpaste",
        "windows" => "powershell.exe -NoProfile -Command Get-Clipboard",
        // Linux
        _ => "bash -c 'if command -v wl-paste >/dev/null 2>&1; then wl-paste; elif command -v xclip >/dev/null 2>&1; then xclip -selection clipboard -o; else echo \"ERROR: no clipboard tool found\" >&2; exit 1; fi'",
    }
}

/// Returns the remote copy command based on the device's OS
pub fn remote_copy_cmd(os: &str) -> &'static str {
    match os {
        "macOS" => "pbcopy",
        "windows" => "clip.exe",
        _ => "bash -c 'if command -v wl-copy >/dev/null 2>&1; then wl-copy; elif command -v xclip >/dev/null 2>&1; then xclip -selection clipboard; else echo \"ERROR: no clipboard tool found\" >&2; exit 1; fi'",
    }
}

pub fn get_local_clipboard() -> Result<String> {
    let (cmd, args) = paste_command();
    let output = Command::new(cmd)
        .args(args)
        .output()
        .context(format!("Failed to read clipboard. Is '{}' installed?", cmd))?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn set_local_clipboard(content: &str) -> Result<()> {
    let (cmd, args) = copy_command();
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context(format!("Failed to write clipboard. Is '{}' installed?", cmd))?;

    child.stdin.as_mut().unwrap().write_all(content.as_bytes())?;
    child.wait()?;
    Ok(())
}
