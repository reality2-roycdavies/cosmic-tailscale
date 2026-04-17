use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::{Length, Rectangle};
use cosmic::iced_runtime::core::window;
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{self, text};
use cosmic::Element;

use crate::config::{AppConfig, Credentials};
use crate::tailscale::{self, PeerInfo, TailscaleStatus, VncType};

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

/// Which service a credential dialog is for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CredService {
    Ssh,
    Rdp,
    Vnc,
    NoMachine,
}

impl CredService {
    fn label(&self) -> &'static str {
        match self {
            Self::Ssh => "SSH",
            Self::Rdp => "RDP",
            Self::Vnc => "VNC",
            Self::NoMachine => "NoMachine",
        }
    }
    fn key(&self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Rdp => "rdp",
            Self::Vnc => "vnc",
            Self::NoMachine => "nomachine",
        }
    }
}

#[derive(Debug, Clone)]
struct CredDialog {
    service: CredService,
    dns_name: String,
    ip: String,
    username: String,
    remember: bool,
    vnc_type: VncType,
}

#[derive(Debug, Clone)]
pub enum Message {
    PollStatus,
    ToggleConnection,
    CopyToClipboard(String),
    // Service launches (direct, no dialog)
    LaunchHttp(String),
    // Credential dialog
    ShowCredDialog {
        service: CredService,
        dns_name: String,
        ip: String,
        vnc_type: VncType,
    },
    CredUsername(String),
    CredRemember(bool),
    CredConnect,
    CredCancel,
    // Settings
    OpenSettings,
    OpenAdminConsole,
    // Popup
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
    // Self info
    self_hostname: String,
    self_ip: String,
    self_dns_name: String,
    self_https_url: String,
    self_relay: String,
    version: String,
    tailnet_name: String,
    exit_node_active: bool,
    exit_node_name: String,
    // Peers
    peers: Vec<PeerInfo>,
    error: Option<String>,
    // Clipboard feedback
    copied_text: Option<String>,
    copied_hold_ticks: u8,
    // Config
    config: AppConfig,
    // Credential dialog
    cred_dialog: Option<CredDialog>,
    // Background thread channels
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
        let (
            connected,
            self_hostname,
            self_ip,
            self_dns_name,
            self_https_url,
            self_relay,
            version,
            tailnet_name,
            peers,
            exit_node_active,
            exit_node_name,
        ) = match tailscale::get_status() {
            Ok(status) => {
                let https_url = status.self_node.https_url(&status.cert_domains);
                (
                    status.backend_state == "Running",
                    status.self_node.display_name().to_string(),
                    status
                        .self_node
                        .tailscale_ips
                        .first()
                        .cloned()
                        .unwrap_or_default(),
                    status.self_node.dns_display(),
                    https_url,
                    status.self_node.relay.clone(),
                    status.version.clone(),
                    status.tailnet_name.clone(),
                    status.peers.clone(),
                    status.exit_node_active,
                    status.exit_node_name.clone(),
                )
            }
            Err(_) => (
                false,
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                vec![],
                false,
                String::new(),
            ),
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
            self_dns_name,
            self_https_url,
            self_relay,
            version,
            tailnet_name,
            peers,
            exit_node_active,
            exit_node_name,
            error: None,
            copied_text: None,
            copied_hold_ticks: 0,
            config: AppConfig::load(),
            cred_dialog: None,
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
                        self.copied_text = None;
                    }
                }

                while let Ok(event) = self.event_rx.try_recv() {
                    match event {
                        TailscaleEvent::StatusUpdate(result) => match result {
                            Ok(status) => {
                                self.connected = status.backend_state == "Running";
                                self.self_hostname =
                                    status.self_node.display_name().to_string();
                                self.self_ip = status
                                    .self_node
                                    .tailscale_ips
                                    .first()
                                    .cloned()
                                    .unwrap_or_default();
                                self.self_dns_name = status.self_node.dns_display();
                                self.self_https_url =
                                    status.self_node.https_url(&status.cert_domains);
                                self.self_relay = status.self_node.relay.clone();
                                self.version = status.version.clone();
                                self.tailnet_name = status.tailnet_name;
                                self.peers = status.peers;
                                self.exit_node_active = status.exit_node_active;
                                self.exit_node_name = status.exit_node_name;
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

            Message::CopyToClipboard(text_val) => {
                let text_clone = text_val.clone();
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("wl-copy")
                        .arg(&text_clone)
                        .spawn();
                });
                self.copied_text = Some(text_val);
                self.copied_hold_ticks = 1; // ~3 seconds (1 tick at 3s poll ≈ 3s)
            }

            Message::LaunchHttp(dns_name) => {
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&format!("http://{dns_name}"))
                        .spawn();
                });
            }

            Message::ShowCredDialog {
                service,
                dns_name,
                ip,
                vnc_type,
            } => {
                let saved = self.config.get_creds(service.key(), &dns_name);
                self.cred_dialog = Some(CredDialog {
                    username: saved
                        .map(|c| c.username.clone())
                        .unwrap_or_default(),
                    remember: true,
                    service,
                    dns_name,
                    ip,
                    vnc_type,
                });
            }

            Message::CredUsername(val) => {
                if let Some(ref mut d) = self.cred_dialog {
                    d.username = val;
                }
            }

            Message::CredRemember(val) => {
                if let Some(ref mut d) = self.cred_dialog {
                    d.remember = val;
                }
            }

            Message::CredConnect => {
                if let Some(dialog) = self.cred_dialog.take() {
                    // Save credentials if requested
                    if dialog.remember {
                        let creds = Credentials {
                            username: dialog.username.clone(),
                        };
                        self.config
                            .save_creds(dialog.service.key(), &dialog.dns_name, creds);

                        // Also update ssh_usernames for backwards compat
                        if dialog.service == CredService::Ssh {
                            if let Some(peer) = self.peers.iter().find(|p| p.dns_display() == dialog.dns_name) {
                                if !dialog.username.is_empty() {
                                    self.config.ssh_usernames.insert(peer.hostname.clone(), dialog.username.clone());
                                } else {
                                    self.config.ssh_usernames.remove(&peer.hostname);
                                }
                                self.config.save();
                            }
                        }
                    }

                    // Launch the service
                    let username = dialog.username;
                    let dns_name = dialog.dns_name;
                    let ip = dialog.ip;
                    let vnc_type = dialog.vnc_type;

                    match dialog.service {
                        CredService::Ssh => {
                            let target = if username.is_empty() {
                                dns_name
                            } else {
                                format!("{username}@{dns_name}")
                            };
                            std::thread::spawn(move || {
                                let terminals = [
                                    ("cosmic-term", vec!["-e", "ssh", &target]),
                                    ("gnome-terminal", vec!["--", "ssh", &target]),
                                    ("konsole", vec!["-e", "ssh", &target]),
                                    ("xterm", vec!["-e", "ssh", &target]),
                                ];
                                for (term, args) in &terminals {
                                    if std::process::Command::new(term)
                                        .args(args)
                                        .spawn()
                                        .is_ok()
                                    {
                                        return;
                                    }
                                }
                                eprintln!("Failed to launch ssh: no terminal emulator found");
                            });
                        }
                        CredService::Rdp => {
                            std::thread::spawn(move || {
                                let remmina_dir = dirs::home_dir()
                                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                                    .join(".local/share/remmina");
                                let _ = std::fs::create_dir_all(&remmina_dir);
                                let profile = remmina_dir
                                    .join(format!(
                                        "tailscale-rdp-{}.remmina",
                                        dns_name.replace(|c: char| !c.is_alphanumeric(), "_")
                                    ))
                                    .to_string_lossy()
                                    .to_string();
                                // Only create the file if it doesn't exist yet;
                                // reuse any existing file so user customisations persist.
                                if !std::path::Path::new(&profile).exists() {
                                    let content = format!("\
[remmina]
name={dns_name}
protocol=RDP
server={dns_name}
username={username}
colordepth=32
quality=2
glyph-cache=true
network=lan
gfx=false
rfx=false
disableautoreconnect=0
");
                                    if let Err(e) = std::fs::write(&profile, &content) {
                                        eprintln!("Failed to write remmina profile: {e}");
                                        return;
                                    }
                                }
                                let native = std::process::Command::new("remmina")
                                    .args(["-c", &profile])
                                    .spawn();
                                if native.is_err() {
                                    if let Err(e) = std::process::Command::new("flatpak")
                                        .args([
                                            "run",
                                            "org.remmina.Remmina",
                                            "-c",
                                            &profile,
                                        ])
                                        .spawn()
                                    {
                                        eprintln!("Failed to launch remmina: {e}");
                                    }
                                }
                            });
                        }
                        CredService::Vnc => {
                            std::thread::spawn(move || {
                                if vnc_type == VncType::RealVnc {
                                    if let Err(e) = std::process::Command::new("vncviewer")
                                        .arg(&dns_name)
                                        .spawn()
                                    {
                                        eprintln!("Failed to launch vncviewer: {e}");
                                    }
                                } else {
                                    let target = if username.is_empty() {
                                        format!("vnc://{dns_name}")
                                    } else {
                                        format!("vnc://{username}@{dns_name}")
                                    };
                                    let native =
                                        std::process::Command::new("remmina").arg(&target).spawn();
                                    if native.is_err() {
                                        if let Err(e) = std::process::Command::new("flatpak")
                                            .args([
                                                "run",
                                                "org.remmina.Remmina",
                                                &target,
                                            ])
                                            .spawn()
                                        {
                                            eprintln!("Failed to launch remmina: {e}");
                                        }
                                    }
                                }
                            });
                        }
                        CredService::NoMachine => {
                            std::thread::spawn(move || {
                                let nx_dir = dirs::home_dir()
                                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                                    .join(".nx");
                                let _ = std::fs::create_dir_all(&nx_dir);
                                let nxs_file = nx_dir
                                    .join(format!(
                                        "tailscale-nx-{}.nxs",
                                        dns_name.replace(|c: char| !c.is_alphanumeric(), "_")
                                    ))
                                    .to_string_lossy()
                                    .to_string();
                                // Only create the file if it doesn't exist yet;
                                // reuse any existing file so user customisations persist.
                                if !std::path::Path::new(&nxs_file).exists() {
                                    let nxs_content = format!("\
<!DOCTYPE NXClientSettings>
<NXClientSettings version=\"2.3\" application=\"nxclient\" >
 <group name=\"General\" >
  <option key=\"Connection service\" value=\"nx\" />
  <option key=\"NoMachine daemon port\" value=\"4000\" />
 </group>
 <group name=\"Local Settings\" >
  <option key=\"Server name\" value=\"{dns_name}\" />
  <option key=\"List of hosts\" value=\"{ip}\" />
  <option key=\"List of ports\" value=\"4000\" />
  <option key=\"List of protocols\" value=\"nx\" />
 </group>
 <group name=\"Login\" >
  <option key=\"Server authentication method\" value=\"system\" />
  <option key=\"System login method\" value=\"password\" />
  <option key=\"User\" value=\"{username}\" />
 </group>
</NXClientSettings>
");
                                    if let Err(e) = std::fs::write(&nxs_file, &nxs_content) {
                                        eprintln!("Failed to write .nxs file: {e}");
                                        return;
                                    }
                                }
                                let native = std::process::Command::new("nxplayer")
                                    .args(["--session", &nxs_file])
                                    .spawn();
                                if native.is_err() {
                                    if let Err(e) = std::process::Command::new("flatpak")
                                        .args([
                                            "run",
                                            "--nosocket=wayland",
                                            "com.nomachine.nxplayer",
                                            "--session",
                                            &nxs_file,
                                        ])
                                        .spawn()
                                    {
                                        eprintln!("Failed to launch nxplayer: {e}");
                                    }
                                }
                            });
                        }
                    }
                }
            }

            Message::CredCancel => {
                self.cred_dialog = None;
            }

            Message::OpenSettings => {
                std::thread::spawn(|| {
                    let unified = std::process::Command::new("cosmic-applet-settings")
                        .arg(APP_ID)
                        .spawn();
                    if unified.is_err() {
                        let exe =
                            std::env::current_exe().unwrap_or_else(|_| "cosmic-tailscale".into());
                        if let Err(e) = std::process::Command::new(exe)
                            .arg("--settings-standalone")
                            .spawn()
                        {
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

                            let popup_width = 380u32;
                            let popup_height = 500u32;

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

        let tooltip: &str = if self.connected {
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

        // If credential dialog is active, show it instead of the normal content
        if let Some(ref dialog) = self.cred_dialog {
            return self.cred_dialog_view(dialog);
        }

        // Title row
        let title_row = row![
            text::body("Tailscale"),
            horizontal_space(),
            widget::button::custom(text::caption("Admin"))
                .on_press(Message::OpenAdminConsole)
                .padding([2, 6])
                .class(cosmic::theme::Button::MenuItem),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Status
        let status_text = format!("Status: {}", self.status_message);
        let mut info_col = column![text::body(status_text)].spacing(2);

        if self.connected {
            // This Device section
            if !self.self_hostname.is_empty() {
                info_col = info_col.push(text::caption(format!("Name: {}", self.self_hostname)));
            }
            if !self.self_ip.is_empty() {
                info_col =
                    info_col.push(text::caption(format!("Tailscale IP: {}", self.self_ip)));
            }
            // DNS Name (clickable to copy)
            if !self.self_dns_name.is_empty() {
                let is_copied = self.copied_text.as_deref() == Some(&self.self_dns_name);
                let label = if is_copied {
                    "DNS Name: Copied!".to_string()
                } else {
                    format!("DNS Name: {}", self.self_dns_name)
                };
                let dns = self.self_dns_name.clone();
                info_col = info_col.push(
                    widget::button::custom(text::caption(label))
                        .on_press(Message::CopyToClipboard(dns))
                        .padding([0, 0])
                        .class(cosmic::theme::Button::MenuItem),
                );
            }
            // HTTPS URL (clickable to copy)
            if !self.self_https_url.is_empty() {
                let is_copied = self.copied_text.as_deref() == Some(&self.self_https_url);
                let label = if is_copied {
                    "HTTPS: Copied!".to_string()
                } else {
                    format!("HTTPS: {}", self.self_https_url)
                };
                let url = self.self_https_url.clone();
                info_col = info_col.push(
                    widget::button::custom(text::caption(label))
                        .on_press(Message::CopyToClipboard(url))
                        .padding([0, 0])
                        .class(cosmic::theme::Button::MenuItem),
                );
            }
            // Relay
            if !self.self_relay.is_empty() {
                info_col =
                    info_col.push(text::caption(format!("Relay: {}", self.self_relay)));
            }
            // Network section
            if !self.tailnet_name.is_empty() {
                info_col =
                    info_col.push(text::caption(format!("Tailnet: {}", self.tailnet_name)));
            }
            if self.exit_node_active {
                let label = if self.exit_node_name.is_empty() {
                    "Exit node: active".to_string()
                } else {
                    format!("Exit node: {}", self.exit_node_name)
                };
                info_col = info_col.push(text::caption(label));
            }
            // Version
            if !self.version.is_empty() {
                info_col = info_col.push(text::caption(format!("Version: {}", self.version)));
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
        let peers_header = text::body(format!("Peers ({online_count}/{total_count} online)"));

        let mut peers_col = column![peers_header].spacing(2);

        if self.connected {
            for peer in &self.peers {
                peers_col = peers_col.push(self.peer_row(peer));
            }
        }

        // Bottom actions row
        let actions_row = row![
            widget::button::standard("Admin Console").on_press(Message::OpenAdminConsole),
            horizontal_space(),
            widget::button::standard("Settings...").on_press(Message::OpenSettings),
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
        use cosmic::iced::widget::{column, row};
        use cosmic::iced::Alignment;

        let ip_str = peer.tailscale_ips.first().cloned().unwrap_or_default();
        let dns_display = peer.dns_display();

        let status_indicator = if peer.online { "● " } else { "○ " };

        let name_label = format!(
            "{status_indicator}{} ({})",
            peer.display_name(),
            peer.os
        );

        // Show "Copied!" feedback for the peer whose DNS/IP was just copied
        let copy_target = if dns_display.is_empty() {
            ip_str.clone()
        } else {
            dns_display.clone()
        };
        let is_copied = self.copied_text.as_deref() == Some(&copy_target);
        let ip_label = if is_copied {
            "  Copied!".to_string()
        } else if !dns_display.is_empty() {
            format!("  {dns_display}")
        } else {
            format!("  {ip_str}")
        };

        let mut peer_col = column![text::caption(name_label), text::caption(ip_label),].spacing(0);

        if peer.exit_node {
            peer_col = peer_col.push(text::caption("  Exit node (active)"));
        }

        // Wrap in a clickable button — copies DNS name (or IP if no DNS)
        let copy_val = copy_target.clone();
        let peer_btn: Element<Message> = widget::button::custom(peer_col)
            .on_press(Message::CopyToClipboard(copy_val))
            .padding([4, 8])
            .class(cosmic::theme::Button::MenuItem)
            .width(Length::Fill)
            .into();

        if peer.online {
            let svc = &peer.services;
            let mut buttons: Vec<Element<Message>> = Vec::new();

            let dns_or_ip = if dns_display.is_empty() {
                ip_str.clone()
            } else {
                dns_display.clone()
            };

            // SSH
            if svc.ssh || peer.ssh_enabled {
                buttons.push(Self::icon_btn(
                    "utilities-terminal-symbolic",
                    true,
                    Message::ShowCredDialog {
                        service: CredService::Ssh,
                        dns_name: dns_or_ip.clone(),
                        ip: ip_str.clone(),
                        vnc_type: VncType::None,
                    },
                ));
            }

            // VNC
            if svc.vnc {
                let icon = if svc.vnc_type == VncType::RealVnc {
                    "io.github.reality2_roycdavies.cosmic-tailscale-realvnc"
                } else {
                    "io.github.reality2_roycdavies.cosmic-tailscale-vnc"
                };
                buttons.push(Self::icon_btn(
                    icon,
                    false,
                    Message::ShowCredDialog {
                        service: CredService::Vnc,
                        dns_name: dns_or_ip.clone(),
                        ip: ip_str.clone(),
                        vnc_type: svc.vnc_type.clone(),
                    },
                ));
            }

            // RDP
            if svc.rdp {
                buttons.push(Self::icon_btn(
                    "folder-remote-symbolic",
                    true,
                    Message::ShowCredDialog {
                        service: CredService::Rdp,
                        dns_name: dns_or_ip.clone(),
                        ip: ip_str.clone(),
                        vnc_type: VncType::None,
                    },
                ));
            }

            // NoMachine
            if svc.nomachine {
                buttons.push(Self::icon_btn(
                    "io.github.reality2_roycdavies.cosmic-tailscale-nomachine",
                    false,
                    Message::ShowCredDialog {
                        service: CredService::NoMachine,
                        dns_name: dns_or_ip.clone(),
                        ip: ip_str.clone(),
                        vnc_type: VncType::None,
                    },
                ));
            }

            // HTTP
            if svc.http {
                buttons.push(Self::icon_btn(
                    "web-browser-symbolic",
                    true,
                    Message::LaunchHttp(dns_or_ip.clone()),
                ));
            }

            // HTTPS copy
            if svc.https {
                let url = peer.https_url();
                if !url.is_empty() {
                    let is_url_copied = self.copied_text.as_deref() == Some(&url);
                    let icon = if is_url_copied {
                        "object-select-symbolic"
                    } else {
                        "edit-copy-symbolic"
                    };
                    buttons.push(Self::icon_btn(
                        icon,
                        true,
                        Message::CopyToClipboard(url),
                    ));
                }
            }

            if buttons.is_empty() {
                peer_btn
            } else {
                let mut buttons_row = row![].spacing(4).align_y(Alignment::Center);
                for btn in buttons {
                    buttons_row = buttons_row.push(btn);
                }
                column![peer_btn, buttons_row].spacing(2).into()
            }
        } else {
            peer_btn
        }
    }

    fn icon_btn(icon_name: &str, symbolic: bool, msg: Message) -> Element<'static, Message> {
        let icon: Element<Message> = widget::icon::from_name(icon_name)
            .symbolic(symbolic)
            .size(16)
            .into();
        widget::button::custom(icon)
            .on_press(msg)
            .padding([4, 4])
            .class(cosmic::theme::Button::MenuItem)
            .into()
    }

    fn cred_dialog_view<'a>(&'a self, dialog: &'a CredDialog) -> widget::Column<'a, Message> {
        use cosmic::iced::widget::{column, horizontal_space, row};
        use cosmic::iced::Alignment;

        let title = format!("{} Connect", dialog.service.label());
        let host_label = dialog.dns_name.clone();

        let username_input: Element<Message> = widget::text_input("username", &dialog.username)
            .on_input(Message::CredUsername)
            .width(Length::Fill)
            .into();

        let mut content = column![
            text::body(title),
            text::caption(host_label),
            row![text::caption("Username"), username_input]
                .spacing(8)
                .align_y(Alignment::Center),
        ]
        .spacing(8)
        .padding(12);

        // Remember checkbox
        let remember: Element<Message> = widget::toggler(dialog.remember)
            .on_toggle(Message::CredRemember)
            .into();
        content = content.push(
            row![text::caption("Remember"), remember]
                .spacing(8)
                .align_y(Alignment::Center),
        );

        // Buttons
        content = content.push(
            row![
                widget::button::standard("Cancel").on_press(Message::CredCancel),
                horizontal_space(),
                widget::button::suggested("Connect").on_press(Message::CredConnect),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );

        content
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

        // Poll current status and probe services on online peers
        match tailscale::get_status() {
            Ok(mut status) => {
                // Probe services for online peers
                for peer in &mut status.peers {
                    if peer.online {
                        if let Some(ip) = peer.tailscale_ips.first() {
                            peer.services = tailscale::probe_services(ip);
                        }
                    }
                }
                let _ = event_tx.send(TailscaleEvent::StatusUpdate(Ok(status)));
            }
            Err(e) => {
                let _ = event_tx.send(TailscaleEvent::StatusUpdate(Err(e)));
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

pub fn run_applet() -> cosmic::iced::Result {
    cosmic::applet::run::<TailscaleApplet>(())
}
