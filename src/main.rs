mod applet;
mod config;
mod settings;
mod settings_cli;
mod settings_page;
mod tailscale;

const APPLET_ID: &str = "io.github.reality2_roycdavies.cosmic-tailscale";

fn main() -> cosmic::iced::Result {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--settings" | "-s" => open_settings(),
            "--settings-standalone" => settings::run_settings(),
            "--help" | "-h" => {
                print_help(&args[0]);
                Ok(())
            }
            "--version" | "-v" => {
                println!("cosmic-tailscale {}", env!("CARGO_PKG_VERSION"));
                Ok(())
            }
            "--settings-describe" => {
                settings_cli::describe();
                Ok(())
            }
            "--settings-set" => {
                if args.len() < 4 {
                    eprintln!("Usage: cosmic-tailscale --settings-set <key> <json_value>");
                    std::process::exit(1);
                }
                settings_cli::set(&args[2], &args[3]);
                Ok(())
            }
            "--settings-action" => {
                if args.len() < 3 {
                    eprintln!("Usage: cosmic-tailscale --settings-action <action_id>");
                    std::process::exit(1);
                }
                settings_cli::action(&args[2]);
                Ok(())
            }
            _ => {
                eprintln!("Unknown argument: {}", args[1]);
                eprintln!("Use --help for usage information");
                std::process::exit(1);
            }
        }
    } else {
        applet::run_applet()
    }
}

/// Try to open settings via cosmic-applet-settings hub; fall back to standalone.
fn open_settings() -> cosmic::iced::Result {
    use std::process::Command;
    if Command::new("cosmic-applet-settings")
        .arg(APPLET_ID)
        .spawn()
        .is_ok()
    {
        Ok(())
    } else {
        settings::run_settings()
    }
}

fn print_help(program: &str) {
    println!("Tailscale VPN applet for COSMIC Desktop\n");
    println!("Usage: {} [OPTIONS]\n", program);
    println!("Options:");
    println!("  (none)             Run as COSMIC panel applet");
    println!("  --settings, -s     Open settings (via hub or standalone)");
    println!("  --settings-standalone  Open standalone settings window");
    println!("  --version, -v      Show version information");
    println!("  --help, -h         Show this help message");
}
