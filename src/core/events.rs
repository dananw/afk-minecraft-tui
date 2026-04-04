use tokio::sync::{broadcast, mpsc};

/// Events flowing from bot to UI
#[derive(Clone, Debug)]
pub enum UiEvent {
    Connected { server: String, username: String },
    Disconnected { reason: Option<String> },
    ChatReceived { username: String, message: String },
    SystemMessage(String),
    PositionUpdate { x: f64, y: f64, z: f64 },
    HealthUpdate { health: f32, max_health: f32 },
    PlayerCount(usize),
    Error(String),
    Log { level: LogLevel, message: String },
    Metrics(MetricsData),
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, Default)]
pub struct MetricsData {
    pub render_fps: f32,
    pub network_latency_ms: Option<u32>,
    pub memory_mb: u64,
    pub packet_count: u64,
}

/// Commands flowing from UI to bot
#[derive(Clone, Debug)]
pub enum BotCommand {
    Chat(String),
    Command(String),
    Reconnect,
    Disconnect,
}

/// Central event bus for decoupled communication
pub struct EventBus {
    pub ui_tx: mpsc::UnboundedSender<UiEvent>,
    pub ui_rx: mpsc::UnboundedReceiver<UiEvent>,
    pub cmd_tx: broadcast::Sender<BotCommand>,
    pub cmd_rx: broadcast::Receiver<BotCommand>,
}

impl EventBus {
    pub fn new() -> Self {
        let (ui_tx, ui_rx) = mpsc::unbounded_channel();
        let (cmd_tx, cmd_rx) = broadcast::channel(100);
        Self { ui_tx, ui_rx, cmd_tx, cmd_rx }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
