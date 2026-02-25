use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct TailscaleStatus {
    pub backend_state: String,
    pub self_node: NodeInfo,
    pub peers: Vec<PeerInfo>,
    pub tailnet_name: String,
    pub exit_node_active: bool,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub hostname: String,
    pub tailscale_ips: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub hostname: String,
    pub tailscale_ips: Vec<String>,
    pub os: String,
    pub online: bool,
    pub exit_node: bool,
}

#[derive(Debug, Clone)]
pub struct TailscalePrefs {
    pub accept_dns: bool,
    pub accept_routes: bool,
    pub shields_up: bool,
    pub ssh: bool,
    pub advertise_exit_node: bool,
    pub exit_node_allow_lan: bool,
    pub webclient: bool,
    pub hostname: String,
    pub advertise_routes: String,
    pub login_name: String,
}

// Serde structs for parsing `tailscale status --json`.
// Extra fields in the JSON are silently ignored by serde.

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawStatus {
    backend_state: String,
    #[serde(rename = "Self")]
    self_node: RawPeer,
    #[serde(default)]
    peer: HashMap<String, RawPeer>,
    #[serde(default)]
    current_tailnet: Option<RawTailnet>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawPeer {
    #[serde(default)]
    host_name: String,
    #[serde(default, rename = "TailscaleIPs")]
    tailscale_ips: Vec<String>,
    #[serde(default, rename = "OS")]
    os: String,
    #[serde(default)]
    online: bool,
    #[serde(default)]
    exit_node: bool,
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
    shields_up: bool,
    #[serde(default, rename = "RunSSH")]
    run_ssh: bool,
    #[serde(default)]
    run_web_client: bool,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    advertise_routes: Option<Vec<String>>,
    #[serde(default)]
    exit_node_allow_lan_access: bool,
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

    let self_node = NodeInfo {
        hostname: raw.self_node.host_name,
        tailscale_ips: raw.self_node.tailscale_ips,
    };

    let exit_node_active = raw.peer.values().any(|p| p.exit_node);

    let mut peers: Vec<PeerInfo> = raw
        .peer
        .into_values()
        .map(|p| PeerInfo {
            hostname: p.host_name,
            tailscale_ips: p.tailscale_ips,
            os: p.os,
            online: p.online,
            exit_node: p.exit_node,
        })
        .collect();

    // Sort: online first, then alphabetical by hostname
    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()))
    });

    let tailnet_name = match raw.current_tailnet {
        Some(t) => t.name,
        None => String::new(),
    };

    Ok(TailscaleStatus {
        backend_state: raw.backend_state,
        self_node,
        peers,
        tailnet_name,
        exit_node_active,
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

    // Check if exit node is being advertised by looking at advertise_routes
    // (tailscale advertises 0.0.0.0/0 and ::/0 when acting as exit node)
    let advertise_exit_node = raw
        .advertise_routes
        .as_ref()
        .map(|routes| routes.iter().any(|r| r == "0.0.0.0/0" || r == "::/0"))
        .unwrap_or(false);

    let advertise_routes = raw
        .advertise_routes
        .as_ref()
        .map(|routes| {
            routes
                .iter()
                .filter(|r| *r != "0.0.0.0/0" && *r != "::/0")
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();

    let login_name = raw
        .config
        .and_then(|c| c.user_profile)
        .map(|u| u.login_name)
        .unwrap_or_default();

    Ok(TailscalePrefs {
        accept_dns: raw.corp_dns,
        accept_routes: raw.route_all,
        shields_up: raw.shields_up,
        ssh: raw.run_ssh,
        advertise_exit_node,
        exit_node_allow_lan: raw.exit_node_allow_lan_access,
        webclient: raw.run_web_client,
        hostname: raw.hostname,
        advertise_routes,
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

pub fn set_string_pref(flag: &str, value: &str) -> Result<(), String> {
    let arg = format!("--{flag}={value}");

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
