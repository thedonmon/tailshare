# tailshare

Share clipboard and files across machines over [Tailscale](https://tailscale.com).

No iCloud, no cloud sync, no accounts — just your devices on your tailnet, connected via SSH.

## Features

- **Clipboard sync** — send/get text between machines
- **Image clipboard** — screenshots and copied images transfer natively
- **File detection** — copy a file in Finder, `tailshare send` transfers the actual file
- **File transfer** — send any file to a remote device via `tailshare file`
- **Watch mode** — auto-sync clipboard changes bi-directionally
- **Device discovery** — lists all devices on your Tailscale network
- **SSH setup wizard** — generates keys and configures SSH in one command
- **Cross-platform** — macOS, Linux (X11/Wayland), Windows

## Install

### From source (requires Rust)

```bash
git clone https://github.com/thedonmon/tailshare.git
cd tailshare
cargo install --path .
```

### Prerequisites

- [Tailscale](https://tailscale.com/download) installed and connected on both machines
- SSH enabled on the remote machine
- Rust toolchain (for building from source)

## Quick Start

```bash
# 1. List your Tailscale devices
tailshare devices

# 2. Set up SSH key auth with a device
tailshare setup "Device Name"

# 3. Configure the SSH user and set as default
tailshare config set-user "Device Name" username
tailshare config set-default "device-name"

# 4. Send your clipboard
tailshare send
```

## Usage

### Clipboard

```bash
# Send clipboard to default device
tailshare send

# Get clipboard from default device
tailshare get

# Send to a specific device
tailshare send "device-name"

# Auto-sync clipboard every 2 seconds (Ctrl+C to stop)
tailshare watch

# Custom sync interval
tailshare watch --interval 5
```

Clipboard `send` automatically detects the content type:

| Content | What happens |
|---|---|
| Text | Sent to remote clipboard |
| Screenshot / image | Transferred via SCP, set as image on remote clipboard |
| File copied in Finder | File transferred to remote `~/.cache/tailshare/`, set on remote clipboard |

### File Transfer

```bash
# Send a file (lands in ~/Downloads on remote)
tailshare file /path/to/file.zip

# Send to a specific destination
tailshare file /path/to/file.zip --dest ~/Documents/
```

### Device Management

```bash
# List all Tailscale devices
tailshare devices

# Set up SSH keys with a device
tailshare setup "Device Name"
```

### Configuration

```bash
# Set default target device
tailshare config set-default "device-name"

# Set SSH user for a device
tailshare config set-user "Device Name" username

# Create a short alias
tailshare config alias mini "device-name"

# Set OS for a device (overrides auto-detection)
tailshare config set-os "Device Name" macos    # or: linux, windows
tailshare config set-os local macos

# Show config
tailshare config show
```

## Raycast Integration

Raycast script commands are included for quick clipboard sync:

1. Create a script commands directory: `mkdir -p ~/raycast-scripts`
2. Add these scripts:

**Send Clipboard** (`send-clipboard.sh`):
```bash
#!/bin/bash
# @raycast.schemaVersion 1
# @raycast.title Send Clipboard
# @raycast.mode silent
# @raycast.icon 📋
# @raycast.packageName Tailshare

tailshare send
```

**Get Clipboard** (`get-clipboard.sh`):
```bash
#!/bin/bash
# @raycast.schemaVersion 1
# @raycast.title Get Clipboard
# @raycast.mode silent
# @raycast.icon 📋
# @raycast.packageName Tailshare

tailshare get
```

3. `chmod +x ~/raycast-scripts/*.sh`
4. In Raycast: Settings → Extensions → Script Commands → Add Script Directory → select `~/raycast-scripts`

## How It Works

tailshare uses SSH to execute clipboard commands on remote machines. It discovers devices via `tailscale status --json` and uses platform-specific clipboard tools:

| Platform | Read | Write |
|---|---|---|
| macOS | `pbpaste` / `osascript` | `pbcopy` / `osascript` |
| Linux (X11) | `xclip` | `xclip` |
| Linux (Wayland) | `wl-paste` | `wl-copy` |
| Windows | `Get-Clipboard` | `clip.exe` |

Files and images are transferred via SCP to preserve binary data integrity.

## License

MIT
