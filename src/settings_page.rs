use cosmic::widget::{button, settings, text, toggler};
use cosmic::Element;

use crate::tailscale;

pub struct State {
    pub accept_dns: bool,
    pub accept_routes: bool,
    pub login_name: String,
    pub status_message: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    ToggleAcceptDns(bool),
    ToggleAcceptRoutes(bool),
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
                login_name: String::new(),
            },
            format!("Failed to load preferences: {e}"),
        ),
    };

    State {
        accept_dns: prefs.accept_dns,
        accept_routes: prefs.accept_routes,
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
        ));

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
        ));

    // Actions section
    let actions_section = settings::section()
        .title("Management")
        .add(settings::item(
            "Advanced settings require the admin console",
            button::suggested("Open Admin Console")
                .on_press(Message::OpenAdminConsole),
        ))
        .add(settings::item_row(vec![
            button::standard("Reload Settings")
                .on_press(Message::Reload)
                .into(),
        ]));

    let mut content_items: Vec<Element<'_, Message>> = vec![
        page_title.into(),
        account_section.into(),
        network_section.into(),
        actions_section.into(),
    ];

    if !state.status_message.is_empty() {
        content_items.push(text::body(&state.status_message).into());
    }

    settings::view_column(content_items).into()
}
