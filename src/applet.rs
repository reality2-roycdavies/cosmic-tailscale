use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::{Length, Rectangle};
use cosmic::iced_runtime::core::window;
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{self, text};
use cosmic::Element;

use crate::config::AppConfig;
use crate::tailscale::{self, PeerInfo, TailscaleStatus};

const APP_ID: &str = "io.github.reality2_roycdavies.cosmic-tailscale";

enum TailscaleCommand {
    Toggle,
}

#[derive(Debug)]
enum TailscaleEvent {
    StatusUpdate(Result<TailscaleStatus, String>),
    ToggleStarted,
    ToggleComplete(Result<String, String>),
}

#[derive(Debug, Clone)]
pub enum Message {
    PollStatus,
    ToggleConnection,
    CopyIp(String),
    LaunchNoMachine(String),
    LaunchSsh(String),
    OpenSettings,
    OpenAdminConsole,
    EditSshUser(String),
    SshUserInput(String),
    SaveSshUser(String),
    PopupClosed(Id),
    Surface(cosmic::surface::Action),
}

pub struct TailscaleApplet {
    core: Core,
    popup: Option<Id>,
    connected: bool,
    is_toggling: bool,
    status_message: String,
    status_hold_ticks: u8,
    self_hostname: String,
    self_ip: String,
    tailnet_name: String,
    peers: Vec<PeerInfo>,
    exit_node_active: bool,
    error: Option<String>,
    copied_ip: Option<String>,
    copied_hold_ticks: u8,
    config: AppConfig,
    editing_ssh_user: Option<(String, String)>,
    cmd_tx: std::sync::mpsc::Sender<TailscaleCommand>,
    event_rx: std::sync::mpsc::Receiver<TailscaleEvent>,
}

impl cosmic::Application for TailscaleApplet {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let (event_tx, event_rx) = std::sync::mpsc::channel();

        // Get initial status
        let (connected, self_hostname, self_ip, tailnet_name, peers, exit_node_active) =
            match tailscale::get_status() {
                Ok(status) => (
                    status.backend_state == "Running",
                    status.self_node.hostname.clone(),
                    status
                        .self_node
                        .tailscale_ips
                        .first()
                        .cloned()
                        .unwrap_or_default(),
                    status.tailnet_name.clone(),
                    status.peers.clone(),
                    status.exit_node_active,
                ),
                Err(_) => (false, String::new(), String::new(), String::new(), vec![], false),
            };

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(run_background(cmd_rx, event_tx));
        });

        let applet = Self {
            core,
            popup: None,
            connected,
            is_toggling: false,
            status_hold_ticks: 0,
            status_message: if connected {
                "Connected".to_string()
            } else {
                "Disconnected".to_string()
            },
            self_hostname,
            self_ip,
            tailnet_name,
            peers,
            exit_node_active,
            error: None,
            copied_ip: None,
            copied_hold_ticks: 0,
            config: AppConfig::load(),
            editing_ssh_user: None,
            cmd_tx,
            event_rx,
        };

        (applet, Task::none())
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::PollStatus => {
                // Decrement copied feedback timer
                if self.copied_hold_ticks > 0 {
                    self.copied_hold_ticks -= 1;
                    if self.copied_hold_ticks == 0 {
                        self.copied_ip = None;
                    }
                }

                while let Ok(event) = self.event_rx.try_recv() {
                    match event {
                        TailscaleEvent::StatusUpdate(result) => match result {
                            Ok(status) => {
                                self.connected = status.backend_state == "Running";
                                self.self_hostname = status.self_node.hostname;
                                self.self_ip = status
                                    .self_node
                                    .tailscale_ips
                                    .first()
                                    .cloned()
                                    .unwrap_or_default();
                                self.tailnet_name = status.tailnet_name;
                                self.peers = status.peers;
                                self.exit_node_active = status.exit_node_active;
                                self.error = None;

                                if self.status_hold_ticks > 0 {
                                    self.status_hold_ticks -= 1;
                                } else if !self.is_toggling {
                                    self.status_message = if self.connected {
                                        "Connected".to_string()
                                    } else {
                                        "Disconnected".to_string()
                                    };
                                }
                            }
                            Err(e) => {
                                self.connected = false;
                                self.error = Some(e);
                                if self.status_hold_ticks > 0 {
                                    self.status_hold_ticks -= 1;
                                } else if !self.is_toggling {
                                    self.status_message = "Not running".to_string();
                                }
                            }
                        },
                        TailscaleEvent::ToggleStarted => {
                            self.is_toggling = true;
                            self.status_message = if self.connected {
                                "Disconnecting...".to_string()
                            } else {
                                "Connecting...".to_string()
                            };
                        }
                        TailscaleEvent::ToggleComplete(result) => {
                            self.is_toggling = false;
                            self.status_hold_ticks = 3;
                            match result {
                                Ok(msg) => self.status_message = msg,
                                Err(e) => self.status_message = format!("Error: {e}"),
                            }
                        }
                    }
                }
            }

            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }

            Message::Surface(action) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(action),
                ));
            }

            Message::ToggleConnection => {
                let _ = self.cmd_tx.send(TailscaleCommand::Toggle);
                self.is_toggling = true;
                self.status_message = if self.connected {
                    "Disconnecting...".to_string()
                } else {
                    "Connecting...".to_string()
                };
            }

            Message::CopyIp(ip) => {
                // Use wl-copy for Wayland clipboard
                let ip_clone = ip.clone();
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("wl-copy")
                        .arg(&ip_clone)
                        .spawn();
                });
                self.copied_ip = Some(ip);
                self.copied_hold_ticks = 3; // ~9 seconds at 3s poll
            }

            Message::LaunchNoMachine(ip) => {
                std::thread::spawn(move || {
                    let nxs_content = format!(
                        r#"<!DOCTYPE NXClientSettings>
<NXClientSettings version="2.3" application="nxclient">
 <group name="General">
  <option key="Connection service" value="nx" />
  <option key="Server host" value="{ip}" />
  <option key="Server port" value="22" />
  <option key="NoMachine daemon port" value="4000" />
  <option key="Session" value="unix" />
 </group>
 <group name="Login">
  <option key="Server authentication method" value="system" />
  <option key="NX login method" value="password" />
  <option key="Auth" value="EMPTY_PASSWORD" />
  <option key="User" value="" />
 </group>
</NXClientSettings>"#
                    );
                    // Write to ~/.nx/ which the Flatpak sandbox can access
                    let nx_dir = dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                        .join(".nx");
                    let _ = std::fs::create_dir_all(&nx_dir);
                    let path = nx_dir.join(format!("cosmic-tailscale-{ip}.nxs"));
                    if let Err(e) = std::fs::write(&path, nxs_content) {
                        eprintln!("Failed to write .nxs file: {e}");
                        return;
                    }
                    if let Err(e) = std::process::Command::new("flatpak")
                        .args(["run", "com.nomachine.nxplayer", "--session"])
                        .arg(&path)
                        .spawn()
                    {
                        eprintln!("Failed to launch nxplayer: {e}");
                    }
                });
            }

            Message::LaunchSsh(ip) => {
                // Look up configured SSH username for this peer
                let ssh_target = self
                    .peers
                    .iter()
                    .find(|p| p.tailscale_ips.first().map(|s| s.as_str()) == Some(&ip))
                    .and_then(|p| self.config.ssh_usernames.get(&p.hostname))
                    .filter(|u| !u.is_empty())
                    .map(|user| format!("{user}@{ip}"))
                    .unwrap_or_else(|| ip.clone());

                std::thread::spawn(move || {
                    // Try cosmic-term first, then common terminal emulators
                    let terminals = [
                        ("cosmic-term", vec!["-e", "ssh", &ssh_target]),
                        ("gnome-terminal", vec!["--", "ssh", &ssh_target]),
                        ("konsole", vec!["-e", "ssh", &ssh_target]),
                        ("xterm", vec!["-e", "ssh", &ssh_target]),
                    ];
                    for (term, args) in &terminals {
                        if let Ok(_) = std::process::Command::new(term)
                            .args(args)
                            .spawn()
                        {
                            return;
                        }
                    }
                    eprintln!("Failed to launch ssh: no terminal emulator found");
                });
            }

            Message::EditSshUser(hostname) => {
                let current = self
                    .config
                    .ssh_usernames
                    .get(&hostname)
                    .cloned()
                    .unwrap_or_default();
                self.editing_ssh_user = Some((hostname, current));
            }

            Message::SshUserInput(text) => {
                if let Some((_, ref mut input)) = self.editing_ssh_user {
                    *input = text;
                }
            }

            Message::SaveSshUser(hostname) => {
                if let Some((_, ref input)) = self.editing_ssh_user {
                    let username = input.trim().to_string();
                    if username.is_empty() {
                        self.config.ssh_usernames.remove(&hostname);
                    } else {
                        self.config.ssh_usernames.insert(hostname, username);
                    }
                    self.config.save();
                }
                self.editing_ssh_user = None;
            }

            Message::OpenSettings => {
                std::thread::spawn(|| {
                    // Don't spawn a second instance if already running
                    if let Ok(output) = std::process::Command::new("pgrep").arg("-f").arg("cosmic-applet-settings").output() {
                        if output.status.success() { return; }
                    }
                    let unified = std::process::Command::new("cosmic-applet-settings")
                        .arg("tailscale")
                        .spawn();
                    if unified.is_err() {
                        let exe = std::env::current_exe()
                            .unwrap_or_else(|_| "cosmic-tailscale".into());
                        if let Err(e) = std::process::Command::new(exe).arg("--settings").spawn() {
                            eprintln!("Failed to launch settings: {e}");
                        }
                    }
                });
            }

            Message::OpenAdminConsole => {
                std::thread::spawn(|| {
                    let _ = std::process::Command::new("xdg-open")
                        .arg("https://login.tailscale.com/admin/machines")
                        .spawn();
                });
            }
        }

        Task::none()
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        cosmic::iced::time::every(std::time::Duration::from_secs(3))
            .map(|_| Message::PollStatus)
    }

    fn view(&self) -> Element<'_, Message> {
        let icon_name = if self.connected {
            "io.github.reality2_roycdavies.cosmic-tailscale-connected-symbolic"
        } else {
            "io.github.reality2_roycdavies.cosmic-tailscale-disconnected-symbolic"
        };

        let icon: Element<Message> = widget::icon::from_name(icon_name)
            .symbolic(true)
            .into();

        let have_popup = self.popup;
        let btn = self
            .core
            .applet
            .button_from_element(icon, true)
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = have_popup {
                    Message::Surface(destroy_popup(id))
                } else {
                    Message::Surface(app_popup::<TailscaleApplet>(
                        move |state: &mut TailscaleApplet| {
                            let new_id = Id::unique();
                            state.popup = Some(new_id);

                            let popup_width = 320u32;
                            let popup_height = 450u32;

                            let mut popup_settings = state.core.applet.get_popup_settings(
                                state.core.main_window_id().unwrap(),
                                new_id,
                                Some((popup_width, popup_height)),
                                None,
                                None,
                            );
                            popup_settings.positioner.anchor_rect = Rectangle {
                                x: (bounds.x - offset.x) as i32,
                                y: (bounds.y - offset.y) as i32,
                                width: bounds.width as i32,
                                height: bounds.height as i32,
                            };
                            popup_settings
                        },
                        Some(Box::new(|state: &TailscaleApplet| {
                            Element::from(
                                state.core.applet.popup_container(state.popup_content()),
                            )
                            .map(cosmic::Action::App)
                        })),
                    ))
                }
            });

        let tooltip = if self.connected {
            "Tailscale (Connected)"
        } else {
            "Tailscale (Disconnected)"
        };

        Element::from(self.core.applet.applet_tooltip::<Message>(
            btn,
            tooltip,
            self.popup.is_some(),
            |a| Message::Surface(a),
            None,
        ))
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        "".into()
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}

impl TailscaleApplet {
    fn popup_content(&self) -> widget::Column<'_, Message> {
        use cosmic::iced::widget::{column, container, horizontal_space, row, Space};
        use cosmic::iced::{Alignment, Color};

        let divider = || {
            container(Space::new(Length::Fill, Length::Fixed(1.0))).style(
                |theme: &cosmic::Theme| {
                    let cosmic = theme.cosmic();
                    container::Style {
                        background: Some(cosmic::iced::Background::Color(Color::from(
                            cosmic.palette.neutral_5,
                        ))),
                        ..Default::default()
                    }
                },
            )
        };

        // Title row
        let title_row = row![text::body("Tailscale"), horizontal_space(),]
            .spacing(8)
            .align_y(Alignment::Center);

        // Status and self info
        let status_text = format!("Status: {}", self.status_message);
        let mut info_col = column![text::body(status_text)].spacing(2);

        if self.connected {
            if !self.self_hostname.is_empty() {
                info_col = info_col.push(text::caption(format!("Host: {}", self.self_hostname)));
            }
            if !self.self_ip.is_empty() {
                info_col = info_col.push(text::caption(format!("IP: {}", self.self_ip)));
            }
            if !self.tailnet_name.is_empty() {
                info_col =
                    info_col.push(text::caption(format!("Network: {}", self.tailnet_name)));
            }
            if self.exit_node_active {
                info_col = info_col.push(text::caption("Exit node: active"));
            }
        }

        if let Some(ref err) = self.error {
            info_col = info_col.push(text::caption(format!("Error: {err}")));
        }

        // Toggle button
        let toggle_btn: Element<Message> = if self.is_toggling {
            widget::button::standard(if self.connected {
                "Disconnecting..."
            } else {
                "Connecting..."
            })
            .into()
        } else if self.connected {
            widget::button::destructive("Disconnect")
                .on_press(Message::ToggleConnection)
                .into()
        } else {
            widget::button::suggested("Connect")
                .on_press(Message::ToggleConnection)
                .into()
        };

        let toggle_row = row![text::body("VPN"), horizontal_space(), toggle_btn,]
            .spacing(8)
            .align_y(Alignment::Center);

        // Peers section
        let online_count = self.peers.iter().filter(|p| p.online).count();
        let total_count = self.peers.len();
        let peers_header = text::body(format!("Devices ({online_count}/{total_count} online)"));

        let mut peers_col = column![peers_header].spacing(2);

        if self.connected {
            for peer in &self.peers {
                peers_col = peers_col.push(self.peer_row(peer));
            }
        }

        // Bottom actions row
        let actions_row = row![
            widget::button::standard("Admin Console")
                .on_press(Message::OpenAdminConsole),
            horizontal_space(),
            widget::button::standard("Settings...")
                .on_press(Message::OpenSettings),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Assemble
        let mut content = column![title_row, divider(), info_col, divider(), toggle_row,]
            .spacing(8)
            .padding(12);

        if self.connected && !self.peers.is_empty() {
            content = content.push(divider()).push(peers_col);
        }

        content = content.push(divider()).push(actions_row);

        content
    }

    fn peer_row(&self, peer: &PeerInfo) -> Element<'_, Message> {
        use cosmic::iced::widget::{column, horizontal_space, row};
        use cosmic::iced::Alignment;

        let ip_str = peer.tailscale_ips.first().cloned().unwrap_or_default();

        let status_indicator = if peer.online { "● " } else { "○ " };

        let name_label = format!(
            "{status_indicator}{} ({})",
            peer.hostname, peer.os
        );

        // Show "Copied!" feedback for the peer whose IP was just copied
        let ip_label = if self.copied_ip.as_deref() == Some(&ip_str) {
            "  Copied!".to_string()
        } else {
            format!("  {ip_str}")
        };

        let mut peer_col = column![text::caption(name_label), text::caption(ip_label),].spacing(0);

        if peer.exit_node {
            peer_col = peer_col.push(text::caption("  Exit node (active)"));
        }

        // Wrap in a clickable button using MenuItem style — full width
        let ip_for_click = ip_str.clone();
        let peer_btn: Element<Message> = widget::button::custom(peer_col)
            .on_press(Message::CopyIp(ip_for_click))
            .padding([4, 8])
            .class(cosmic::theme::Button::MenuItem)
            .width(Length::Fill)
            .into();

        if peer.online {
            let is_editing = self
                .editing_ssh_user
                .as_ref()
                .map(|(h, _)| h == &peer.hostname)
                .unwrap_or(false);

            let nx_btn: Element<Message> = widget::button::standard("NX")
                .on_press(Message::LaunchNoMachine(ip_str.clone()))
                .into();

            let buttons_row = if is_editing {
                let input_value = self
                    .editing_ssh_user
                    .as_ref()
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("");
                let hostname = peer.hostname.clone();
                let hostname2 = peer.hostname.clone();

                let input: Element<Message> = widget::text_input("username", input_value)
                    .on_input(Message::SshUserInput)
                    .on_submit(move |_| Message::SaveSshUser(hostname.clone()))
                    .width(Length::Fixed(100.0))
                    .into();

                let save_btn: Element<Message> = widget::button::standard("Save")
                    .on_press(Message::SaveSshUser(hostname2))
                    .into();

                row![input, save_btn, horizontal_space(), nx_btn]
                    .spacing(4)
                    .align_y(Alignment::Center)
            } else {
                let configured_user = self
                    .config
                    .ssh_usernames
                    .get(&peer.hostname)
                    .filter(|u| !u.is_empty());

                let user_label = match configured_user {
                    Some(user) => format!("{user}@"),
                    None => "user@".to_string(),
                };

                let hostname = peer.hostname.clone();
                let user_btn: Element<Message> = widget::button::custom(text::caption(user_label))
                    .on_press(Message::EditSshUser(hostname))
                    .padding([2, 4])
                    .class(cosmic::theme::Button::MenuItem)
                    .into();

                let ssh_btn: Element<Message> = widget::button::standard("SSH")
                    .on_press(Message::LaunchSsh(ip_str))
                    .into();

                row![user_btn, ssh_btn, horizontal_space(), nx_btn]
                    .spacing(4)
                    .align_y(Alignment::Center)
            };

            column![peer_btn, buttons_row]
                .spacing(2)
                .into()
        } else {
            peer_btn
        }
    }
}

async fn run_background(
    cmd_rx: std::sync::mpsc::Receiver<TailscaleCommand>,
    event_tx: std::sync::mpsc::Sender<TailscaleEvent>,
) {
    loop {
        // Check for commands from the UI
        if let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                TailscaleCommand::Toggle => {
                    let _ = event_tx.send(TailscaleEvent::ToggleStarted);

                    let currently_running = tailscale::get_status()
                        .map(|s| s.backend_state == "Running")
                        .unwrap_or(false);

                    let result = if currently_running {
                        tailscale::disconnect()
                    } else {
                        tailscale::connect()
                    };

                    let _ = event_tx.send(TailscaleEvent::ToggleComplete(result));
                }
            }
        }

        // Poll current status
        let status = tailscale::get_status();
        let _ = event_tx.send(TailscaleEvent::StatusUpdate(status));

        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }
}

pub fn run_applet() -> cosmic::iced::Result {
    cosmic::applet::run::<TailscaleApplet>(())
}
