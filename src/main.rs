mod applet;
mod settings;
mod settings_page;
mod tailscale;

fn main() -> cosmic::iced::Result {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--settings" | "-s" => settings::run_settings(),
            "--help" | "-h" => {
                print_help(&args[0]);
                Ok(())
            }
            "--version" | "-v" => {
                println!("cosmic-tailscale {}", env!("CARGO_PKG_VERSION"));
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

fn print_help(program: &str) {
    println!("Tailscale VPN applet for COSMIC Desktop\n");
    println!("Usage: {} [OPTIONS]\n", program);
    println!("Options:");
    println!("  (none)             Run as COSMIC panel applet");
    println!("  --settings, -s     Open the settings window");
    println!("  --version, -v      Show version information");
    println!("  --help, -h         Show this help message");
}
