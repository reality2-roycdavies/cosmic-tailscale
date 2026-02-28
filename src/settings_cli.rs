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
        "description": "Basic Tailscale preferences. Use the Admin Console for advanced settings.",
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
                    }
                ]
            }
        ],
        "actions": [
            {"id": "open_admin_console", "label": "Open Admin Console", "style": "suggested"},
            {"id": "reload", "label": "Reload Settings", "style": "standard"}
        ]
    });

    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}

pub fn set(key: &str, value: &str) {
    let result = match key {
        "accept_dns" => set_bool("accept-dns", value),
        "accept_routes" => set_bool("accept-routes", value),
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
