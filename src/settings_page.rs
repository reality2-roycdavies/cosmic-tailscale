use cosmic::iced::Length;
use cosmic::widget::{button, settings, text, text_input, toggler};
use cosmic::Element;

use crate::tailscale;

pub struct State {
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
    pub status_message: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    ToggleAcceptDns(bool),
    ToggleAcceptRoutes(bool),
    ToggleShieldsUp(bool),
    ToggleSsh(bool),
    ToggleAdvertiseExitNode(bool),
    ToggleExitNodeAllowLan(bool),
    ToggleWebclient(bool),
    HostnameChanged(String),
    ApplyHostname,
    AdvertiseRoutesChanged(String),
    ApplyRoutes,
    OpenAdminConsole,
    Reload,
}

pub fn init() -> State {
    let (prefs, status_message) = match tailscale::get_prefs() {
        Ok(p) => (p, String::new()),
        Err(e) => (
            tailscale::TailscalePrefs {
                accept_dns: true,
                accept_routes: false,
                shields_up: false,
                ssh: false,
                advertise_exit_node: false,
                exit_node_allow_lan: false,
                webclient: false,
                hostname: String::new(),
                advertise_routes: String::new(),
                login_name: String::new(),
            },
            format!("Failed to load preferences: {e}"),
        ),
    };

    State {
        accept_dns: prefs.accept_dns,
        accept_routes: prefs.accept_routes,
        shields_up: prefs.shields_up,
        ssh: prefs.ssh,
        advertise_exit_node: prefs.advertise_exit_node,
        exit_node_allow_lan: prefs.exit_node_allow_lan,
        webclient: prefs.webclient,
        hostname: prefs.hostname,
        advertise_routes: prefs.advertise_routes,
        login_name: prefs.login_name,
        status_message,
    }
}

pub fn update(state: &mut State, message: Message) {
    match message {
        Message::ToggleAcceptDns(val) => {
            match tailscale::set_bool_pref("accept-dns", val) {
                Ok(()) => {
                    state.accept_dns = val;
                    state.status_message = "Accept DNS updated".to_string();
                }
                Err(e) => state.status_message = format!("Error: {e}"),
            }
        }
        Message::ToggleAcceptRoutes(val) => {
            match tailscale::set_bool_pref("accept-routes", val) {
                Ok(()) => {
                    state.accept_routes = val;
                    state.status_message = "Accept routes updated".to_string();
                }
                Err(e) => state.status_message = format!("Error: {e}"),
            }
        }
        Message::ToggleShieldsUp(val) => {
            match tailscale::set_bool_pref("shields-up", val) {
                Ok(()) => {
                    state.shields_up = val;
                    state.status_message = "Shields up updated".to_string();
                }
                Err(e) => state.status_message = format!("Error: {e}"),
            }
        }
        Message::ToggleSsh(val) => {
            match tailscale::set_bool_pref("ssh", val) {
                Ok(()) => {
                    state.ssh = val;
                    state.status_message = "SSH server updated".to_string();
                }
                Err(e) => state.status_message = format!("Error: {e}"),
            }
        }
        Message::ToggleAdvertiseExitNode(val) => {
            match tailscale::set_bool_pref("advertise-exit-node", val) {
                Ok(()) => {
                    state.advertise_exit_node = val;
                    state.status_message = "Exit node setting updated".to_string();
                }
                Err(e) => state.status_message = format!("Error: {e}"),
            }
        }
        Message::ToggleExitNodeAllowLan(val) => {
            match tailscale::set_bool_pref("exit-node-allow-lan-access", val) {
                Ok(()) => {
                    state.exit_node_allow_lan = val;
                    state.status_message = "LAN access setting updated".to_string();
                }
                Err(e) => state.status_message = format!("Error: {e}"),
            }
        }
        Message::ToggleWebclient(val) => {
            match tailscale::set_bool_pref("webclient", val) {
                Ok(()) => {
                    state.webclient = val;
                    state.status_message = "Web client updated".to_string();
                }
                Err(e) => state.status_message = format!("Error: {e}"),
            }
        }
        Message::HostnameChanged(val) => {
            state.hostname = val;
        }
        Message::ApplyHostname => {
            match tailscale::set_string_pref("hostname", &state.hostname) {
                Ok(()) => state.status_message = "Hostname updated".to_string(),
                Err(e) => state.status_message = format!("Error: {e}"),
            }
        }
        Message::AdvertiseRoutesChanged(val) => {
            state.advertise_routes = val;
        }
        Message::ApplyRoutes => {
            match tailscale::set_string_pref("advertise-routes", &state.advertise_routes) {
                Ok(()) => state.status_message = "Advertised routes updated".to_string(),
                Err(e) => state.status_message = format!("Error: {e}"),
            }
        }
        Message::OpenAdminConsole => {
            std::thread::spawn(|| {
                let _ = std::process::Command::new("xdg-open")
                    .arg("https://login.tailscale.com/admin/machines")
                    .spawn();
            });
        }
        Message::Reload => {
            match tailscale::get_prefs() {
                Ok(prefs) => {
                    state.accept_dns = prefs.accept_dns;
                    state.accept_routes = prefs.accept_routes;
                    state.shields_up = prefs.shields_up;
                    state.ssh = prefs.ssh;
                    state.advertise_exit_node = prefs.advertise_exit_node;
                    state.exit_node_allow_lan = prefs.exit_node_allow_lan;
                    state.webclient = prefs.webclient;
                    state.hostname = prefs.hostname;
                    state.advertise_routes = prefs.advertise_routes;
                    state.login_name = prefs.login_name;
                    state.status_message = "Settings reloaded".to_string();
                }
                Err(e) => state.status_message = format!("Error reloading: {e}"),
            }
        }
    }
}

pub fn view(state: &State) -> Element<'_, Message> {
    let page_title = text::title1("Tailscale Settings");

    // Account section
    let account_label = if state.login_name.is_empty() {
        "Not logged in".to_string()
    } else {
        state.login_name.clone()
    };

    let account_section = settings::section()
        .title("Account")
        .add(settings::item(
            "Logged in as",
            text::body(account_label),
        ))
        .add(settings::item_row(vec![
            button::suggested("Open Admin Console")
                .on_press(Message::OpenAdminConsole)
                .into(),
        ]));

    // Network section
    let network_section = settings::section()
        .title("Network")
        .add(settings::item(
            "Accept DNS",
            toggler(state.accept_dns).on_toggle(Message::ToggleAcceptDns),
        ))
        .add(settings::item(
            "Accept routes",
            toggler(state.accept_routes).on_toggle(Message::ToggleAcceptRoutes),
        ))
        .add(settings::item(
            "Shields up (block incoming)",
            toggler(state.shields_up).on_toggle(Message::ToggleShieldsUp),
        ));

    // Services section
    let services_section = settings::section()
        .title("Services")
        .add(settings::item(
            "Run SSH server",
            toggler(state.ssh).on_toggle(Message::ToggleSsh),
        ))
        .add(settings::item(
            "Advertise as exit node",
            toggler(state.advertise_exit_node).on_toggle(Message::ToggleAdvertiseExitNode),
        ))
        .add(settings::item(
            "Allow LAN access via exit node",
            toggler(state.exit_node_allow_lan).on_toggle(Message::ToggleExitNodeAllowLan),
        ))
        .add(settings::item(
            "Web client",
            toggler(state.webclient).on_toggle(Message::ToggleWebclient),
        ));

    // Advanced section
    let advanced_section = settings::section()
        .title("Advanced")
        .add(settings::item(
            "Hostname override",
            cosmic::iced::widget::row![
                text_input("(use OS hostname)", &state.hostname)
                    .on_input(Message::HostnameChanged)
                    .on_submit(|_| Message::ApplyHostname)
                    .width(Length::Fixed(250.0)),
                button::standard("Apply").on_press(Message::ApplyHostname),
            ]
            .spacing(8),
        ))
        .add(settings::item(
            "Advertise routes",
            cosmic::iced::widget::row![
                text_input("e.g. 10.0.0.0/24,192.168.1.0/24", &state.advertise_routes)
                    .on_input(Message::AdvertiseRoutesChanged)
                    .on_submit(|_| Message::ApplyRoutes)
                    .width(Length::Fixed(250.0)),
                button::standard("Apply").on_press(Message::ApplyRoutes),
            ]
            .spacing(8),
        ));

    // Actions section
    let actions_section = settings::section()
        .title("Actions")
        .add(settings::item_row(vec![
            button::standard("Reload Settings")
                .on_press(Message::Reload)
                .into(),
        ]));

    let mut content_items: Vec<Element<'_, Message>> = vec![
        page_title.into(),
        account_section.into(),
        network_section.into(),
        services_section.into(),
        advanced_section.into(),
        actions_section.into(),
    ];

    if !state.status_message.is_empty() {
        content_items.push(text::body(&state.status_message).into());
    }

    settings::view_column(content_items).into()
}
