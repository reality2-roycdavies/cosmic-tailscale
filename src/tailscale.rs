use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Command;
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Default)]
pub struct ServiceInfo {
    pub ssh: bool,
    pub vnc: bool,
    pub vnc_type: VncType,
    pub rdp: bool,
    pub nomachine: bool,
    pub http: bool,
    pub https: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum VncType {
    #[default]
    None,
    RealVnc,
    Other,
}

#[derive(Debug, Clone)]
pub struct TailscaleStatus {
    pub backend_state: String,
    pub version: String,
    pub self_node: NodeInfo,
    pub peers: Vec<PeerInfo>,
    pub tailnet_name: String,
    pub exit_node_active: bool,
    pub exit_node_name: String,
    pub cert_domains: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub hostname: String,
    pub dns_name: String,
    pub tailscale_ips: Vec<String>,
    pub relay: String,
}

impl NodeInfo {
    /// The display name: first label of DNSName if available, otherwise hostname.
    pub fn display_name(&self) -> &str {
        if !self.dns_name.is_empty() {
            self.dns_name.split('.').next().unwrap_or(&self.hostname)
        } else {
            &self.hostname
        }
    }

    /// Full DNS name without trailing dot.
    pub fn dns_display(&self) -> String {
        self.dns_name.trim_end_matches('.').to_string()
    }

    /// HTTPS URL if the DNS name is in the cert domains.
    pub fn https_url(&self, cert_domains: &[String]) -> String {
        let dns = self.dns_display();
        if !dns.is_empty() && cert_domains.iter().any(|d| d == &dns) {
            format!("https://{dns}")
        } else {
            String::new()
        }
    }
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub hostname: String,
    pub dns_name: String,
    pub tailscale_ips: Vec<String>,
    pub os: String,
    pub online: bool,
    pub exit_node: bool,
    pub ssh_enabled: bool,
    #[allow(dead_code)]
    pub relay: String,
    pub services: ServiceInfo,
}

impl PeerInfo {
    /// The display name: first label of DNSName if available, otherwise hostname.
    pub fn display_name(&self) -> &str {
        if !self.dns_name.is_empty() {
            self.dns_name.split('.').next().unwrap_or(&self.hostname)
        } else {
            &self.hostname
        }
    }

    /// Full DNS name without trailing dot.
    pub fn dns_display(&self) -> String {
        self.dns_name.trim_end_matches('.').to_string()
    }

    /// HTTPS URL for this peer (requires MagicDNS).
    pub fn https_url(&self) -> String {
        let dns = self.dns_display();
        if !dns.is_empty() {
            format!("https://{dns}")
        } else {
            String::new()
        }
    }
}

#[derive(Debug, Clone)]
pub struct TailscalePrefs {
    pub accept_dns: bool,
    pub accept_routes: bool,
    pub login_name: String,
}

// --- Port probing ---

fn check_port(ip: &str, port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("{ip}:{port}").parse().unwrap_or_else(|_| {
            std::net::SocketAddr::from(([0, 0, 0, 0], port))
        }),
        PROBE_TIMEOUT,
    )
    .is_ok()
}

fn detect_vnc_server(ip: &str) -> VncType {
    let addr = match format!("{ip}:5900").parse() {
        Ok(a) => a,
        Err(_) => return VncType::None,
    };
    let mut stream = match TcpStream::connect_timeout(&addr, PROBE_TIMEOUT) {
        Ok(s) => s,
        Err(_) => return VncType::None,
    };
    let _ = stream.set_read_timeout(Some(PROBE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(PROBE_TIMEOUT));

    // Read RFB version banner (12 bytes like "RFB 003.008\n")
    let mut banner = [0u8; 12];
    if stream.read_exact(&mut banner).is_err() {
        return VncType::None;
    }
    // Echo back the banner as our version reply
    if stream.write_all(&banner).is_err() {
        return VncType::None;
    }
    // Read security types
    let mut buf = [0u8; 64];
    match stream.read(&mut buf) {
        Ok(n) if n > 1 => {
            let num_types = buf[0] as usize;
            let sec_types = &buf[1..1 + num_types.min(n - 1)];
            // Type 30 = RealVNC authentication
            if sec_types.contains(&30) {
                VncType::RealVnc
            } else {
                VncType::Other
            }
        }
        _ => VncType::None,
    }
}

pub fn probe_services(ip: &str) -> ServiceInfo {
    let mut info = ServiceInfo::default();

    info.ssh = check_port(ip, 22);

    let vnc_type = detect_vnc_server(ip);
    info.vnc = vnc_type != VncType::None;
    info.vnc_type = vnc_type;

    info.rdp = check_port(ip, 3389);
    info.nomachine = check_port(ip, 4000);
    info.http = check_port(ip, 80);
    info.https = check_port(ip, 443);

    info
}

// --- Serde structs for parsing `tailscale status --json` ---

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawStatus {
    backend_state: String,
    #[serde(default)]
    version: String,
    #[serde(rename = "Self")]
    self_node: RawPeer,
    #[serde(default)]
    peer: HashMap<String, RawPeer>,
    #[serde(default)]
    current_tailnet: Option<RawTailnet>,
    #[serde(default)]
    cert_domains: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawPeer {
    #[serde(default)]
    host_name: String,
    #[serde(default, rename = "DNSName")]
    dns_name: String,
    #[serde(default, rename = "TailscaleIPs")]
    tailscale_ips: Vec<String>,
    #[serde(default, rename = "OS")]
    os: String,
    #[serde(default)]
    online: bool,
    #[serde(default)]
    exit_node: bool,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    relay: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawTailnet {
    #[serde(default)]
    name: String,
}

// Serde structs for parsing `tailscale debug prefs`

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawPrefs {
    #[serde(default)]
    corp_dns: bool,
    #[serde(default)]
    route_all: bool,
    #[serde(default)]
    config: Option<RawConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawConfig {
    #[serde(default)]
    user_profile: Option<RawUserProfile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawUserProfile {
    #[serde(default)]
    login_name: String,
}

pub fn get_status() -> Result<TailscaleStatus, String> {
    let output = Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .map_err(|e| format!("Failed to run tailscale: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tailscale status failed: {stderr}"));
    }

    let raw: RawStatus = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse tailscale JSON: {e}"))?;

    let cert_domains = raw.cert_domains.unwrap_or_default();

    let self_node = NodeInfo {
        hostname: raw.self_node.host_name,
        dns_name: raw.self_node.dns_name,
        tailscale_ips: raw.self_node.tailscale_ips,
        relay: raw.self_node.relay,
    };

    let mut exit_node_name = String::new();

    let mut peers: Vec<PeerInfo> = raw
        .peer
        .into_values()
        .map(|p| {
            let ssh_enabled = p.capabilities.iter().any(|c| c.contains("/cap/ssh"));
            let is_exit = p.exit_node;
            let display = if !p.dns_name.is_empty() {
                p.dns_name.split('.').next().unwrap_or(&p.host_name).to_string()
            } else {
                p.host_name.clone()
            };
            if is_exit {
                exit_node_name = display;
            }
            PeerInfo {
                hostname: p.host_name,
                dns_name: p.dns_name,
                tailscale_ips: p.tailscale_ips,
                os: p.os,
                online: p.online,
                exit_node: p.exit_node,
                ssh_enabled,
                relay: p.relay,
                services: ServiceInfo::default(),
            }
        })
        .collect();

    // Sort: online first, then alphabetical by hostname
    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()))
    });

    let exit_node_active = peers.iter().any(|p| p.exit_node);

    let tailnet_name = match raw.current_tailnet {
        Some(t) => t.name,
        None => String::new(),
    };

    // Truncate version to just the number part
    let version = {
        let v = raw.version;
        match v.find('-') {
            Some(i) => v[..i].to_string(),
            None => v,
        }
    };

    Ok(TailscaleStatus {
        backend_state: raw.backend_state,
        version,
        self_node,
        peers,
        tailnet_name,
        exit_node_active,
        exit_node_name,
        cert_domains,
    })
}

pub fn get_prefs() -> Result<TailscalePrefs, String> {
    let output = Command::new("tailscale")
        .args(["debug", "prefs"])
        .output()
        .map_err(|e| format!("Failed to run tailscale debug prefs: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tailscale debug prefs failed: {stderr}"));
    }

    let raw: RawPrefs = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse prefs JSON: {e}"))?;

    let login_name = raw
        .config
        .and_then(|c| c.user_profile)
        .map(|u| u.login_name)
        .unwrap_or_default();

    Ok(TailscalePrefs {
        accept_dns: raw.corp_dns,
        accept_routes: raw.route_all,
        login_name,
    })
}

pub fn set_bool_pref(flag: &str, value: bool) -> Result<(), String> {
    let arg = if value {
        format!("--{flag}")
    } else {
        format!("--{flag}=false")
    };

    let output = Command::new("tailscale")
        .args(["set", &arg])
        .output()
        .map_err(|e| format!("Failed to run tailscale set: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tailscale set {flag} failed: {stderr}"));
    }

    Ok(())
}

pub fn connect() -> Result<String, String> {
    let output = Command::new("tailscale")
        .args(["up"])
        .output()
        .map_err(|e| format!("Failed to run tailscale up: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tailscale up failed: {stderr}"));
    }

    Ok("Connected".to_string())
}

pub fn disconnect() -> Result<String, String> {
    let output = Command::new("tailscale")
        .args(["down"])
        .output()
        .map_err(|e| format!("Failed to run tailscale down: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tailscale down failed: {stderr}"));
    }

    Ok("Disconnected".to_string())
}
