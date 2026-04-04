use std::time::Instant;
use serde::{Deserialize, Serialize};

/// Application state machine for clear state management
#[derive(Clone, Debug)]
pub enum BotState {
    /// Initial state or disconnected
    Disconnected { last_error: Option<String> },
    /// Currently connecting
    Connecting { server: String, started_at: Instant },
    /// Successfully connected
    Connected { server: String, username: String, since: Instant },
    /// Connection lost, will reconnect
    Reconnecting { server: String, attempt: u32, delay_secs: u64 },
    /// Transferring to different server
    Transferring { from: String, to: String },
}

impl Default for BotState {
    fn default() -> Self {
        BotState::Disconnected { last_error: None }
    }
}

impl BotState {
    pub fn is_connected(&self) -> bool {
        matches!(self, BotState::Connected { .. })
    }

    pub fn server(&self) -> Option<&str> {
        match self {
            BotState::Connecting { server, .. } => Some(server),
            BotState::Connected { server, .. } => Some(server),
            BotState::Reconnecting { server, .. } => Some(server),
            _ => None,
        }
    }

    pub fn username(&self) -> Option<&str> {
        match self {
            BotState::Connected { username, .. } => Some(username),
            _ => None,
        }
    }
}

/// UI State independent from bot state
#[derive(Clone, Debug, Default)]
pub struct UiState {
    /// Current scroll position in chat
    pub chat_scroll: usize,
    /// Current scroll position in error panel
    pub error_scroll: usize,
    /// Whether error panel is visible
    pub error_visible: bool,
    /// Command history
    pub command_history: Vec<String>,
    /// Current position in history (None = not navigating)
    pub history_index: Option<usize>,
    /// Current input buffer (separate from history)
    pub input_buffer: String,
    /// Cursor position in input
    pub cursor_position: usize,
    /// Whether help overlay is shown
    pub show_help: bool,
    /// Active modal dialog
    pub active_modal: Option<ModalType>,
    /// Last frame time for FPS calculation
    pub last_frame_time: Option<Instant>,
    /// Current FPS
    pub current_fps: f32,
}

#[derive(Clone, Debug)]
pub enum ModalType {
    ConfirmReconnect,
    ConfirmQuit,
    Error(String),
    Help,
}

/// Complete application state
#[derive(Clone, Debug, Default)]
pub struct AppState {
    pub bot: BotState,
    pub ui: UiState,
    /// Player data
    pub player: PlayerData,
    /// Configuration
    pub config: Config,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerData {
    pub health: f32,
    pub max_health: f32,
    pub position: (f64, f64, f64),
    pub online_players: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_server")]
    pub server: String,

    #[serde(default = "default_username")]
    pub username: String,

    #[serde(default)]
    pub email: Option<String>,

    #[serde(default)]
    pub password: Option<String>,

    #[serde(default = "default_reconnect_delay")]
    pub reconnect_delay: u64,

    #[serde(default = "default_view_distance")]
    pub view_distance: u8,

    #[serde(default)]
    pub auto_commands: Vec<String>,

    #[serde(default)]
    pub theme: ThemeConfig,
}

fn default_server() -> String {
    "localhost:25565".to_string()
}

fn default_username() -> String {
    "afk_bot".to_string()
}

fn default_reconnect_delay() -> u64 {
    5
}

fn default_view_distance() -> u8 {
    8
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub name: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self { name: "dark".to_string() }
    }
}

impl ThemeConfig {
    pub fn to_theme(&self) -> Theme {
        match self.name.as_str() {
            "light" => Theme::light(),
            _ => Theme::dark(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    pub fn new() -> Self {
        Self {
            server: default_server(),
            username: default_username(),
            email: None,
            password: None,
            reconnect_delay: default_reconnect_delay(),
            view_distance: default_view_distance(),
            auto_commands: Vec::new(),
            theme: ThemeConfig::default(),
        }
    }

    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn save_to_file(&self, path: &str) -> anyhow::Result<()> {
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub primary: ratatui::style::Color,
    pub success: ratatui::style::Color,
    pub error: ratatui::style::Color,
    pub warning: ratatui::style::Color,
    pub info: ratatui::style::Color,
    pub background: ratatui::style::Color,
    pub foreground: ratatui::style::Color,
    pub border: ratatui::style::Color,
    pub highlight: ratatui::style::Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: ratatui::style::Color::Cyan,
            success: ratatui::style::Color::Green,
            error: ratatui::style::Color::Red,
            warning: ratatui::style::Color::Yellow,
            info: ratatui::style::Color::Blue,
            background: ratatui::style::Color::Black,
            foreground: ratatui::style::Color::White,
            border: ratatui::style::Color::Gray,
            highlight: ratatui::style::Color::Magenta,
        }
    }
}

impl Theme {
    pub fn dark() -> Self {
        Self::default()
    }

    pub fn light() -> Self {
        Self {
            primary: ratatui::style::Color::Blue,
            success: ratatui::style::Color::Green,
            error: ratatui::style::Color::Red,
            warning: ratatui::style::Color::Yellow,
            info: ratatui::style::Color::Cyan,
            background: ratatui::style::Color::White,
            foreground: ratatui::style::Color::Black,
            border: ratatui::style::Color::Gray,
            highlight: ratatui::style::Color::Magenta,
        }
    }
}
