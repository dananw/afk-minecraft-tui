use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use azalea::{
    prelude::*,
    ClientInformation, WalkDirection,
};
use tokio::sync::broadcast;

use crate::core::{BotCommand, UiEvent};

const TICKS_PER_SECOND: u64 = 20;
const MOVE_PHASE_TICKS: u64 = TICKS_PER_SECOND * 2;
const IDLE_PHASE_TICKS: u64 = TICKS_PER_SECOND * 3;
const JUMP_INTERVAL_TICKS: u64 = TICKS_PER_SECOND * 15;
const LOOK_INTERVAL_TICKS: u64 = TICKS_PER_SECOND * 10;
const PLAYER_COUNT_INTERVAL_TICKS: u64 = TICKS_PER_SECOND * 5;

/// Bot state component
#[derive(Clone, Component, Default)]
pub struct BotStateComponent {
    pub view_distance: u8,
    pub login_password: Option<String>,
    pub commands: Vec<String>,
    pub chat: Option<String>,
    pub server_address: String,
    pub username: String,
}

/// Channel-based event sender
#[derive(Clone, Component)]
pub struct EventSender {
    pub tx: tokio::sync::mpsc::UnboundedSender<UiEvent>,
}

impl Default for EventSender {
    fn default() -> Self {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        Self { tx }
    }
}

/// Persistent command receiver — subscribed once, shared across all handler calls
#[derive(Clone, Component)]
pub struct CommandReceiver {
    rx: Arc<Mutex<broadcast::Receiver<BotCommand>>>,
}

impl CommandReceiver {
    pub fn new(rx: broadcast::Receiver<BotCommand>) -> Self {
        Self { rx: Arc::new(Mutex::new(rx)) }
    }

    /// Drain all pending commands
    pub fn drain(&self) -> Vec<BotCommand> {
        let mut commands = Vec::new();
        if let Ok(mut rx) = self.rx.lock() {
            loop {
                match rx.try_recv() {
                    Ok(cmd) => commands.push(cmd),
                    Err(broadcast::error::TryRecvError::Lagged(n)) => {
                        // Some messages were missed due to buffer overflow, continue draining
                        eprintln!("[WARN] Missed {} commands due to channel lag", n);
                        continue;
                    }
                    Err(_) => break,
                }
            }
        }
        commands
    }
}

impl Default for CommandReceiver {
    fn default() -> Self {
        let (_tx, rx) = broadcast::channel(100);
        Self::new(rx)
    }
}

/// Combined state for Azalea ECS
#[derive(Clone, Component)]
pub struct BotECSState {
    pub state: BotStateComponent,
    pub event_sender: EventSender,
    pub cmd_rx: CommandReceiver,
    /// Shared flag for force reconnect
    pub force_reconnect: Arc<AtomicBool>,
}

impl Default for BotECSState {
    fn default() -> Self {
        Self {
            state: BotStateComponent::default(),
            event_sender: EventSender::default(),
            cmd_rx: CommandReceiver::default(),
            force_reconnect: Arc::new(AtomicBool::new(false)),
        }
    }
}

static AFK_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub async fn handle_event(
    bot: Client,
    event: Event,
    ecs_state: BotECSState,
) -> anyhow::Result<()> {
    let BotECSState { state, event_sender, cmd_rx, force_reconnect } = ecs_state;
    let send_event = |event: UiEvent| {
        let _ = event_sender.tx.send(event);
    };

    match event {
        Event::Init => {
            bot.set_client_information(ClientInformation {
                view_distance: state.view_distance,
                ..Default::default()
            });

            send_event(UiEvent::Connected {
                server: state.server_address.clone(),
                username: state.username.clone(),
            });
            send_event(UiEvent::SystemMessage("Connected to server".to_string()));
        }
        Event::Login => {
            send_event(UiEvent::SystemMessage("Login successful".to_string()));
        }
        Event::Spawn => {
            AFK_TICK.store(0, Ordering::Relaxed);
            send_event(UiEvent::SystemMessage("Spawned and ready".to_string()));

            let pos = bot.position();
            send_event(UiEvent::PositionUpdate { x: pos.x, y: pos.y, z: pos.z });
            send_event(UiEvent::HealthUpdate { health: bot.health(), max_health: 20.0 });
            send_event(UiEvent::PlayerCount(bot.tab_list().len()));

            // Auto commands
            let login_password = state.login_password.clone();
            let commands = state.commands.clone();
            let chat = state.chat.clone();
            let bot_clone = bot.clone();

            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                if let Some(password) = login_password {
                    bot_clone.chat(format!("/login {}", password));
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                }

                for cmd in commands {
                    bot_clone.chat(&cmd);
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                }

                if let Some(msg) = chat {
                    bot_clone.chat(&msg);
                }
            });
        }
        Event::Chat(chat_event) => {
            // Send ANSI-encoded message to preserve Minecraft color formatting
            let ansi_message = chat_event.message().to_ansi();
            match chat_event.split_sender_and_content() {
                (Some(username), _) => {
                    send_event(UiEvent::ChatReceived { username, message: ansi_message });
                }
                (None, _) => {
                    send_event(UiEvent::SystemMessage(ansi_message));
                }
            }
        }
        Event::Tick => {
            // Drain ALL pending commands from TUI using persistent receiver
            for cmd in cmd_rx.drain() {
                match cmd {
                    BotCommand::Chat(msg) => {
                        bot.chat(&msg);
                    }
                    BotCommand::Command(cmd) => {
                        // Normalize: strip leading slash if present, then send as /command
                        let cmd_without_slash = cmd.strip_prefix('/').unwrap_or(&cmd);
                        bot.chat(format!("/{}", cmd_without_slash));
                        send_event(UiEvent::SystemMessage(format!("[CMD] /{}", cmd_without_slash)));
                    }
                    BotCommand::Reconnect => {
                        send_event(UiEvent::SystemMessage("[RECONNECT] Forcing reconnect...".to_string()));
                        force_reconnect.store(true, Ordering::Relaxed);
                    }
                    BotCommand::Disconnect => {}
                }
            }

            anti_afk_tick(&bot);

            let current_tick = AFK_TICK.load(Ordering::Relaxed);
            if current_tick % PLAYER_COUNT_INTERVAL_TICKS == 0 {
                send_event(UiEvent::PlayerCount(bot.tab_list().len()));
            }
        }
        Event::Disconnect(reason) => {
            let reason_text = reason
                .map(|formatted| formatted.to_ansi().to_string())
                .unwrap_or_else(|| "no reason provided".to_string());
            AFK_TICK.store(0, Ordering::Relaxed);

            send_event(UiEvent::Disconnected {
                reason: Some(reason_text),
            });
        }
        _ => {}
    }

    Ok(())
}

fn anti_afk_tick(bot: &Client) {
    let tick = AFK_TICK.fetch_add(1, Ordering::Relaxed);
    let phase = tick % (MOVE_PHASE_TICKS + IDLE_PHASE_TICKS);

    if phase < MOVE_PHASE_TICKS {
        bot.walk(WalkDirection::Forward);
    } else {
        bot.walk(WalkDirection::None);
    }

    let jump_window = tick % JUMP_INTERVAL_TICKS;
    bot.set_jumping(jump_window == 0 || jump_window == 1);

    if tick % LOOK_INTERVAL_TICKS == 0 {
        let yaw = ((tick / LOOK_INTERVAL_TICKS) % 8) as f32 * 45.0 - 180.0;
        bot.set_direction(yaw, 0.0);
    }
}

pub async fn run_bot(
    args: crate::app::Args,
    cmd_tx: broadcast::Sender<BotCommand>,
    event_tx: tokio::sync::mpsc::UnboundedSender<UiEvent>,
) -> anyhow::Result<()> {
    use anyhow::Context;

    let account = if let Some(email) = &args.email {
        Account::microsoft(email)
            .await
            .with_context(|| format!("Failed to login Microsoft for {}", email))?
    } else {
        Account::offline(&args.username)
    };

    let reconnect_delay = tokio::time::Duration::from_secs(args.reconnect_delay_seconds);
    let server = args.server.clone();

    // Shared flag for force reconnect
    let force_reconnect = Arc::new(AtomicBool::new(false));

    loop {
        // Reset flag at start of each connection attempt
        force_reconnect.store(false, Ordering::Relaxed);

        // Subscribe ONCE per connection — this receiver persists across all handle_event calls
        let cmd_rx = CommandReceiver::new(cmd_tx.subscribe());

        let ecs_state = BotECSState {
            state: BotStateComponent {
                view_distance: args.view_distance,
                login_password: args.login_password.clone(),
                commands: args.command.clone(),
                chat: args.chat.clone(),
                server_address: server.clone(),
                username: args.username.clone(),
            },
            event_sender: EventSender { tx: event_tx.clone() },
            cmd_rx,
            force_reconnect: force_reconnect.clone(),
        };

        // Run bot — reconnect_after(None) means Azalea won't auto-reconnect,
        // we handle reconnection ourselves in this loop
        let bot_future = ClientBuilder::new()
            .set_handler(handle_event)
            .set_state(ecs_state)
            .reconnect_after(None)
            .start(account.clone(), server.clone());

        // Wait for bot to finish OR force reconnect flag
        let result = tokio::select! {
            r = bot_future => r,
            _ = wait_for_force_reconnect(force_reconnect.clone()) => {
                AppExit::Success
            }
        };

        // Check if force reconnect was requested
        if force_reconnect.load(Ordering::Relaxed) {
            let _ = event_tx.send(UiEvent::SystemMessage(
                "[RECONNECT] Reconnecting now...".to_string()
            ));
            force_reconnect.store(false, Ordering::Relaxed);
            // Short delay to allow server to clear session
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        } else {
            // Normal disconnect — reconnect after delay
            let _ = event_tx.send(UiEvent::SystemMessage(
                format!("Reconnecting in {} seconds...", args.reconnect_delay_seconds)
            ));
            tokio::time::sleep(reconnect_delay).await;

            if result == AppExit::Success {
                break;
            }
        }
    }

    Ok(())
}

/// Async helper: resolves when force_reconnect flag is set to true
async fn wait_for_force_reconnect(flag: Arc<AtomicBool>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        if flag.load(Ordering::Relaxed) {
            return;
        }
    }
}
