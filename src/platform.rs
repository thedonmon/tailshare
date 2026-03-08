use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardContent {
    Text(String),
    Image(Vec<u8>),
    /// A file copied from Finder (contains the full path)
    File(String),
    Empty,
}

// --- Local clipboard: text ---

fn paste_command() -> (&'static str, &'static [&'static str]) {
    if cfg!(target_os = "macos") {
        ("pbpaste", &[])
    } else if cfg!(target_os = "windows") {
        ("powershell.exe", &["-NoProfile", "-Command", "Get-Clipboard"])
    } else {
        if which_exists("wl-paste") {
            ("wl-paste", &[])
        } else {
            ("xclip", &["-selection", "clipboard", "-o"])
        }
    }
}

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

// --- Local clipboard: images ---

pub fn has_local_image() -> bool {
    if cfg!(target_os = "macos") {
        let output = Command::new("osascript")
            .args(["-e", "clipboard info"])
            .output();
        match output {
            Ok(o) => {
                let info = String::from_utf8_lossy(&o.stdout);
                info.contains("PNGf") || info.contains("TIFF")
            }
            Err(_) => false,
        }
    } else if cfg!(target_os = "linux") {
        if which_exists("wl-paste") {
            Command::new("wl-paste")
                .args(["--list-types"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains("image/png"))
                .unwrap_or(false)
        } else {
            Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "TARGETS", "-o"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains("image/png"))
                .unwrap_or(false)
        }
    } else {
        false // Windows image clipboard is complex, skip for now
    }
}

pub fn get_local_image() -> Result<Vec<u8>> {
    if cfg!(target_os = "macos") {
        // Use osascript to write clipboard image to a temp file, then read it
        let tmp = std::env::temp_dir().join("tailshare_clip.png");
        let tmp_str = tmp.to_string_lossy();
        let script = format!(
            r#"set theFile to POSIX file "{}"
set fRef to open for access theFile with write permission
set eof fRef to 0
write (the clipboard as «class PNGf») to fRef
close access fRef"#,
            tmp_str
        );
        let status = Command::new("osascript")
            .args(["-e", &script])
            .status()
            .context("Failed to extract image from clipboard")?;
        if !status.success() {
            anyhow::bail!("osascript failed to extract clipboard image");
        }
        let data = std::fs::read(&tmp)?;
        let _ = std::fs::remove_file(&tmp);
        Ok(data)
    } else if cfg!(target_os = "linux") {
        let output = if which_exists("wl-paste") {
            Command::new("wl-paste")
                .args(["--type", "image/png"])
                .output()
        } else {
            Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "image/png", "-o"])
                .output()
        };
        let output = output.context("Failed to read image from clipboard")?;
        if !output.status.success() {
            anyhow::bail!("Failed to read image from clipboard");
        }
        Ok(output.stdout)
    } else {
        anyhow::bail!("Image clipboard not supported on this platform yet")
    }
}

pub fn set_local_image(data: &[u8]) -> Result<()> {
    if cfg!(target_os = "macos") {
        let tmp = std::env::temp_dir().join("tailshare_clip_in.png");
        std::fs::write(&tmp, data)?;
        let tmp_str = tmp.to_string_lossy();
        let script = format!(
            r#"set theFile to POSIX file "{}"
set the clipboard to (read theFile as «class PNGf»)"#,
            tmp_str
        );
        let status = Command::new("osascript")
            .args(["-e", &script])
            .status()
            .context("Failed to set clipboard image")?;
        let _ = std::fs::remove_file(&tmp);
        if !status.success() {
            anyhow::bail!("osascript failed to set clipboard image");
        }
        Ok(())
    } else if cfg!(target_os = "linux") {
        let child = if which_exists("wl-copy") {
            Command::new("wl-copy")
                .args(["--type", "image/png"])
                .stdin(std::process::Stdio::piped())
                .spawn()
        } else {
            Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "image/png"])
                .stdin(std::process::Stdio::piped())
                .spawn()
        };
        #[allow(unused_mut)]
        let mut child = child.context("Failed to set clipboard image")?;
        child.stdin.as_mut().unwrap().write_all(data)?;
        child.wait()?;
        Ok(())
    } else {
        anyhow::bail!("Image clipboard not supported on this platform yet")
    }
}

/// Check if clipboard contains a file reference (e.g. copied from Finder)
pub fn get_local_file_path() -> Option<String> {
    if cfg!(target_os = "macos") {
        // Check if clipboard has a file URL
        let info = Command::new("osascript")
            .args(["-e", "clipboard info"])
            .output()
            .ok()?;
        let info_str = String::from_utf8_lossy(&info.stdout);
        if !info_str.contains("furl") {
            return None;
        }
        // Get the POSIX path
        let output = Command::new("osascript")
            .args(["-e", "POSIX path of (the clipboard as «class furl»)"])
            .output()
            .ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && std::path::Path::new(&path).exists() {
                return Some(path);
            }
        }
    }
    None
}

pub fn get_local_clipboard_content() -> Result<ClipboardContent> {
    // Check for file reference first (Finder copy)
    if let Some(path) = get_local_file_path() {
        return Ok(ClipboardContent::File(path));
    }
    // Then check for image data (screenshot, image copy)
    if has_local_image() {
        match get_local_image() {
            Ok(data) => return Ok(ClipboardContent::Image(data)),
            Err(_) => {} // Fall through to text
        }
    }
    let text = get_local_clipboard()?;
    if text.is_empty() {
        Ok(ClipboardContent::Empty)
    } else {
        Ok(ClipboardContent::Text(text))
    }
}

// --- Remote clipboard commands ---

pub fn remote_paste_cmd(os: &str) -> &'static str {
    match os {
        "macOS" => "pbpaste",
        "windows" => "powershell.exe -NoProfile -Command Get-Clipboard",
        _ => "bash -c 'if command -v wl-paste >/dev/null 2>&1; then wl-paste; elif command -v xclip >/dev/null 2>&1; then xclip -selection clipboard -o; else echo \"ERROR: no clipboard tool found\" >&2; exit 1; fi'",
    }
}

pub fn remote_copy_cmd(os: &str) -> &'static str {
    match os {
        "macOS" => "pbcopy",
        "windows" => "clip.exe",
        _ => "bash -c 'if command -v wl-copy >/dev/null 2>&1; then wl-copy; elif command -v xclip >/dev/null 2>&1; then xclip -selection clipboard; else echo \"ERROR: no clipboard tool found\" >&2; exit 1; fi'",
    }
}

// --- File transfer helpers ---

