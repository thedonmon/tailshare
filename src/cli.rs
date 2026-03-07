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

    /// Show current configuration
    Show,
}
