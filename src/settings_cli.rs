//! CLI settings protocol for cosmic-applet-settings hub integration.

use crate::tailscale;

pub fn describe() {
    let prefs = match tailscale::get_prefs() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to get tailscale prefs: {e}");
            std::process::exit(1);
        }
    };

    let schema = serde_json::json!({
        "title": "Tailscale Settings",
        "description": "Configure Tailscale VPN preferences.",
        "sections": [
            {
                "title": "Account",
                "items": [
                    {
                        "type": "info",
                        "key": "login_name",
                        "label": "Login",
                        "value": prefs.login_name
                    }
                ]
            },
            {
                "title": "Network",
                "items": [
                    {
                        "type": "toggle",
                        "key": "accept_dns",
                        "label": "Accept DNS",
                        "value": prefs.accept_dns
                    },
                    {
                        "type": "toggle",
                        "key": "accept_routes",
                        "label": "Accept Routes",
                        "value": prefs.accept_routes
                    },
                    {
                        "type": "toggle",
                        "key": "shields_up",
                        "label": "Shields Up",
                        "value": prefs.shields_up
                    }
                ]
            },
            {
                "title": "Services",
                "items": [
                    {
                        "type": "toggle",
                        "key": "ssh",
                        "label": "SSH",
                        "value": prefs.ssh
                    },
                    {
                        "type": "toggle",
                        "key": "advertise_exit_node",
                        "label": "Advertise as Exit Node",
                        "value": prefs.advertise_exit_node
                    },
                    {
                        "type": "toggle",
                        "key": "exit_node_allow_lan",
                        "label": "Allow LAN Access",
                        "value": prefs.exit_node_allow_lan
                    },
                    {
                        "type": "toggle",
                        "key": "webclient",
                        "label": "Web Client",
                        "value": prefs.webclient
                    }
                ]
            },
            {
                "title": "Advanced",
                "items": [
                    {
                        "type": "text",
                        "key": "hostname",
                        "label": "Hostname",
                        "value": prefs.hostname,
                        "placeholder": "Override hostname"
                    },
                    {
                        "type": "text",
                        "key": "advertise_routes",
                        "label": "Advertise Routes",
                        "value": prefs.advertise_routes,
                        "placeholder": "10.0.0.0/24,192.168.1.0/24"
                    }
                ]
            }
        ],
        "actions": [
            {"id": "open_admin_console", "label": "Open Admin Console", "style": "standard"},
            {"id": "reload", "label": "Reload Settings", "style": "standard"}
        ]
    });

    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}

pub fn set(key: &str, value: &str) {
    let result = match key {
        "accept_dns" => set_bool("accept-dns", value),
        "accept_routes" => set_bool("accept-routes", value),
        "shields_up" => set_bool("shields-up", value),
        "ssh" => set_bool("ssh", value),
        "advertise_exit_node" => {
            match serde_json::from_str::<bool>(value) {
                Ok(v) => {
                    // Get current routes, add/remove exit routes
                    let current_prefs = tailscale::get_prefs();
                    let routes = match &current_prefs {
                        Ok(p) => p.advertise_routes.clone(),
                        Err(_) => String::new(),
                    };
                    let mut route_list: Vec<String> = if routes.is_empty() {
                        vec![]
                    } else {
                        routes.split(',').map(|s| s.trim().to_string()).collect()
                    };
                    if v {
                        if !route_list.contains(&"0.0.0.0/0".to_string()) {
                            route_list.push("0.0.0.0/0".to_string());
                        }
                        if !route_list.contains(&"::/0".to_string()) {
                            route_list.push("::/0".to_string());
                        }
                    } else {
                        route_list.retain(|r| r != "0.0.0.0/0" && r != "::/0");
                    }
                    let all_routes = route_list.join(",");
                    tailscale::set_string_pref("advertise-routes", &all_routes)
                        .map(|_| "Updated exit node".to_string())
                        .map_err(|e| e)
                }
                Err(e) => Err(format!("Invalid boolean: {e}")),
            }
        }
        "exit_node_allow_lan" => set_bool("exit-node-allow-lan-access", value),
        "webclient" => set_bool("webclient", value),
        "hostname" => {
            match serde_json::from_str::<String>(value) {
                Ok(v) => tailscale::set_string_pref("hostname", &v)
                    .map(|_| "Updated hostname".to_string())
                    .map_err(|e| e),
                Err(e) => Err(format!("Invalid string: {e}")),
            }
        }
        "advertise_routes" => {
            match serde_json::from_str::<String>(value) {
                Ok(v) => {
                    // Preserve exit node routes if present
                    let current_prefs = tailscale::get_prefs();
                    let has_exit = current_prefs.as_ref().map(|p| p.advertise_exit_node).unwrap_or(false);
                    let mut routes = v;
                    if has_exit {
                        if !routes.is_empty() {
                            routes.push_str(",0.0.0.0/0,::/0");
                        } else {
                            routes = "0.0.0.0/0,::/0".to_string();
                        }
                    }
                    tailscale::set_string_pref("advertise-routes", &routes)
                        .map(|_| "Updated routes".to_string())
                        .map_err(|e| e)
                }
                Err(e) => Err(format!("Invalid string: {e}")),
            }
        }
        _ => Err(format!("Unknown key: {key}")),
    };

    match result {
        Ok(msg) => print_response(true, &msg),
        Err(e) => print_response(false, &e),
    }
}

pub fn action(id: &str) {
    match id {
        "open_admin_console" => {
            use std::process::Command;
            match Command::new("xdg-open")
                .arg("https://login.tailscale.com/admin/machines")
                .spawn()
            {
                Ok(_) => print_response(true, "Opened admin console"),
                Err(e) => print_response(false, &format!("Failed to open: {e}")),
            }
        }
        "reload" => {
            // Just re-describe will refresh
            print_response(true, "Settings reloaded");
        }
        _ => print_response(false, &format!("Unknown action: {id}")),
    }
}

fn set_bool(flag: &str, value: &str) -> Result<String, String> {
    match serde_json::from_str::<bool>(value) {
        Ok(v) => tailscale::set_bool_pref(flag, v)
            .map(|_| format!("Updated {flag}"))
            .map_err(|e| e),
        Err(e) => Err(format!("Invalid boolean: {e}")),
    }
}

fn print_response(ok: bool, message: &str) {
    let resp = serde_json::json!({"ok": ok, "message": message});
    println!("{}", resp);
}
