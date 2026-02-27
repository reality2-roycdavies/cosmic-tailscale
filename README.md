# Tailscale VPN Applet for COSMIC Desktop

A Tailscale VPN status and control applet for the [COSMIC desktop environment](https://system76.com/cosmic) on Linux.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-2021-orange.svg)
![COSMIC](https://img.shields.io/badge/desktop-COSMIC-purple.svg)

## Features

- **Native COSMIC Panel Applet**: Integrates directly into the COSMIC panel
- **Connection Status**: Icon reflects Tailscale connection state (connected/disconnected)
- **Quick Controls**: Click the applet for Tailscale status and controls
- **Settings Page**: Configurable via the unified COSMIC applet settings app

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) toolchain (1.75+)
- [just](https://github.com/casey/just) command runner
- [Tailscale](https://tailscale.com/download/linux) installed and configured
- System libraries:

```bash
# Debian/Ubuntu/Pop!_OS
sudo apt install libwayland-dev libxkbcommon-dev libssl-dev pkg-config just

# Fedora
sudo dnf install wayland-devel libxkbcommon-devel openssl-devel just

# Arch
sudo pacman -S wayland libxkbcommon openssl just
```

### Build and Install

```bash
git clone https://github.com/reality2-roycdavies/cosmic-tailscale.git
cd cosmic-tailscale

# Build release binary
just build-release

# Install binary, desktop entry, and icons to ~/.local
just install-local
```

Then add the applet to your COSMIC panel via **Settings -> Desktop -> Panel -> Applets**.

### Other just commands

```bash
just build-debug       # Debug build
just run               # Build debug and run
just run-release       # Build release and run
just check             # Run clippy checks
just fmt               # Format code
just clean             # Clean build artifacts
just uninstall-local   # Remove installed files
```

### Uninstalling

```bash
just uninstall-local
```

## Related COSMIC Applets

This is part of a suite of custom applets for the COSMIC desktop:

| Applet | Description |
|--------|-------------|
| **[cosmic-applet-settings](https://github.com/reality2-roycdavies/cosmic-applet-settings)** | Unified settings app for all custom COSMIC applets |
| **[cosmic-runkat](https://github.com/reality2-roycdavies/cosmic-runkat)** | Animated running cat CPU indicator for the panel |
| **[cosmic-bing-wallpaper](https://github.com/reality2-roycdavies/cosmic-bing-wallpaper)** | Daily Bing wallpaper manager with auto-update |
| **[cosmic-pie-menu](https://github.com/reality2-roycdavies/cosmic-pie-menu)** | Radial/pie menu app launcher with gesture support |
| **[cosmic-hotspot](https://github.com/reality2-roycdavies/cosmic-hotspot)** | WiFi hotspot toggle applet |
| **[cosmic-konnect](https://github.com/reality2-roycdavies/cosmic-konnect)** | Device connectivity and sync between Linux and Android |
| **[cosmic-konnect-android](https://github.com/reality2-roycdavies/cosmic-konnect-android)** | Android companion app for Cosmic Konnect |

## License

MIT License - See [LICENSE](LICENSE) for details.

## Acknowledgments

- [System76](https://system76.com/) for the COSMIC desktop environment
- [Tailscale](https://tailscale.com/) for the mesh VPN
