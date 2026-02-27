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

## License

MIT License - See [LICENSE](LICENSE) for details.

## Acknowledgments

- [System76](https://system76.com/) for the COSMIC desktop environment
- [Tailscale](https://tailscale.com/) for the mesh VPN
