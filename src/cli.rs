use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "tailshare",
    about = "Share clipboard and files across machines over Tailscale",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all devices on your Tailscale network
    Devices,

    /// Send your clipboard to a remote device
    Send {
        /// Device name, alias, or "default"
        #[arg(default_value = "default")]
        device: String,
    },

    /// Get the clipboard from a remote device
    Get {
        /// Device name, alias, or "default"
        #[arg(default_value = "default")]
        device: String,
    },

    /// Watch and auto-sync clipboard changes bi-directionally
    Watch {
        /// Device name, alias, or "default"
        #[arg(default_value = "default")]
        device: String,

        /// Sync interval in seconds
        #[arg(short, long, default_value = "2")]
        interval: u64,
    },

    /// Set up SSH key authentication with a device
    Setup {
        /// Device name to set up
        device: String,
    },

    /// Sync device names after renaming machines on Tailscale
    ///
    /// With no args: auto-detects stale names and fixes unambiguous ones.
    /// With args: renames a specific device across config and SSH.
    Sync {
        /// Old device name to replace
        old_name: Option<String>,
        /// New device name (must exist on tailnet)
        new_name: Option<String>,
        /// Preview changes without applying them
        #[arg(long)]
        dry_run: bool,
    },

    /// Send a file to a remote device
    #[command(name = "file")]
    FileSend {
        /// Path to the file to send
        path: String,

        /// Device name, alias, or "default"
        #[arg(default_value = "default")]
        device: String,

        /// Destination path on remote (defaults to ~/Downloads/)
        #[arg(short, long)]
        dest: Option<String>,
    },

    /// Manage configuration
    #[command(subcommand)]
    Config(ConfigCommands),
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Set the default device
    SetDefault {
        /// Device name to use as default
        device: String,
    },

    /// Add an alias for a device
    Alias {
        /// Short alias name
        name: String,
        /// Full device name
        device: String,
    },

    /// Set the SSH user for a device
    SetUser {
        /// Device name
        device: String,
        /// SSH username on that device
        user: String,
    },

    /// Set the OS for a device (overrides Tailscale detection)
    SetOs {
        /// Device name (or "local" for this machine)
        device: String,
        /// Operating system: macos, linux, windows
        #[arg(value_parser = ["macos", "linux", "windows"])]
        os: String,
    },

    /// Show current configuration
    Show,

    /// Check config for stale device names and suggest fixes
    Doctor,
}
