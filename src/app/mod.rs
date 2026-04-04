use crate::core::{BotCommand, BotState, ModalType, Theme, UiEvent, UiState};
use crate::ui::{ChatMessage, ChatPanel, ErrorPanel, InputPanel, Modal, StatusBar};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::Widget,
    Frame, Terminal,
};
use std::{
    io,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

pub mod args;
pub use args::Args;

pub enum AppEvent {
    Tick,
    Key(event::KeyEvent),
    Minecraft(UiEvent),
    ConfigReloaded,
}

use tokio::sync::broadcast::Sender as BroadcastSender;

pub struct App {
    pub should_quit: bool,
    pub state: UiState,
    pub bot_state: BotState,
    pub chat_panel: ChatPanel,
    pub error_panel: ErrorPanel,
    pub input_panel: InputPanel,
    pub status_bar: StatusBar,
    pub tick_rate: Duration,
    pub event_receiver: Receiver<AppEvent>,
    pub bot_sender: BroadcastSender<BotCommand>,
    pub theme: Theme,
    pub last_tick: Instant,
    pub dirty: bool,
}

impl App {
    pub fn new(
        bot_sender: BroadcastSender<BotCommand>,
        server: String,
        username: String,
        theme: Theme,
    ) -> (Self, Sender<AppEvent>) {
        let (event_sender, event_receiver) = mpsc::channel();

        let mut status_bar = StatusBar::new(theme.clone());
        status_bar.server = server;
        status_bar.username = username;

        let app = Self {
            should_quit: false,
            state: UiState::default(),
            bot_state: BotState::Disconnected { last_error: None },
            chat_panel: ChatPanel::new(theme.clone()),
            error_panel: ErrorPanel::new(theme.clone()),
            input_panel: InputPanel::new(theme.clone()),
            status_bar,
            tick_rate: Duration::from_millis(50),
            event_receiver,
            bot_sender,
            theme,
            last_tick: Instant::now(),
            dirty: true,
        };

        (app, event_sender)
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let mut last_frame = Instant::now();

        while !self.should_quit {
            // Calculate FPS
            let elapsed = last_frame.elapsed();
            if elapsed.as_millis() > 0 {
                self.state.current_fps = 1000.0 / elapsed.as_millis() as f32;
            }
            last_frame = Instant::now();

            // Draw only if dirty or on tick
            if self.dirty {
                terminal.draw(|f| self.draw(f))?;
                self.dirty = false;
            }

            // Handle timeout for ticks
            let timeout = self.tick_rate.saturating_sub(self.last_tick.elapsed());

            // Process events
            if let Ok(event) = self.event_receiver.recv_timeout(timeout) {
                match event {
                    AppEvent::Tick => {
                        self.on_tick();
                    }
                    AppEvent::Key(key) => {
                        self.on_key(key)?;
                    }
                    AppEvent::Minecraft(bot_event) => {
                        self.on_bot_event(bot_event);
                    }
                    AppEvent::ConfigReloaded => {
                        self.chat_panel.add_message(ChatMessage::info("Configuration reloaded"));
                        self.dirty = true;
                    }
                }
                self.dirty = true;
            }

            // Handle periodic tick
            if self.last_tick.elapsed() >= self.tick_rate {
                self.on_tick();
                self.last_tick = Instant::now();
                self.dirty = true;
            }
        }

        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Clear background
        let bg = self.theme.background;
        frame.render_widget(
            ratatui::widgets::Block::default().style(Style::default().bg(bg)),
            area,
        );

        // Calculate layout
        let has_errors = self.error_panel.get_log_count() > 0 && self.state.error_visible;
        let error_height = if has_errors { 8 } else { 0 };

        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),
                Constraint::Length(error_height),
                Constraint::Length(1),
                Constraint::Length(3),
            ])
            .split(area);

        // Chat panel
        self.chat_panel.render(main_chunks[0], frame.buffer_mut(), self.state.chat_scroll);

        // Error panel
        if has_errors {
            self.error_panel.render(main_chunks[1], frame.buffer_mut(), self.state.error_scroll);
        }

        // Status bar
        self.status_bar.render(main_chunks[2], frame.buffer_mut(), &self.bot_state);

        // Input panel
        self.input_panel.render(main_chunks[3], frame.buffer_mut(), &self.state);

        // Modal overlay
        if let Some(modal) = &self.state.active_modal {
            let modal_area = self.centered_rect(60, 40, area);
            Modal::render(modal, modal_area, frame.buffer_mut(), &self.theme);
        }

        // Help overlay
        if self.state.show_help {
            let help_area = self.centered_rect(80, 80, area);
            self.render_help(help_area, frame.buffer_mut());
        }
    }

    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    fn render_help(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

        let help_text = r#"
Keyboard Shortcuts:

  General:
    Ctrl+Q / Esc      Quit application
    ?                 Toggle this help

  Input:
    Enter             Send message/command
    Up/Down           Navigate command history
    Left/Right        Move cursor
    Home/End          Jump to start/end
    Backspace         Delete previous character
    Delete            Delete next character

  Chat Panel:
    PageUp            Scroll up
    PageDown          Scroll down

  Usage:
    Type normally     Send as chat message
    Start with /      Send as command

  Built-in Commands:
    /quit, /exit      Exit application
    /clear            Clear chat
    /reconnect        Force reconnect
    /status           Show connection status
    /clear-errors     Clear error logs
    /toggle-errors    Toggle error panel visibility

  Any other /command is sent to the server.
"#;

        // Clear the area first so chat text doesn't bleed through
        Clear.render(area, buf);

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.primary))
            .style(Style::default().bg(self.theme.background));

        let paragraph = Paragraph::new(help_text)
            .block(block)
            .style(Style::default().fg(self.theme.foreground).bg(self.theme.background))
            .wrap(Wrap { trim: true });

        paragraph.render(area, buf);
    }

    fn on_key(&mut self, key: event::KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        // Handle modal first
        if let Some(modal) = &self.state.active_modal {
            match key.code {
                KeyCode::Enter => {
                    // Check modal type and perform action
                    match modal {
                        ModalType::ConfirmQuit => {
                            self.should_quit = true;
                        }
                        ModalType::ConfirmReconnect => {
                            let _ = self.bot_sender.send(BotCommand::Reconnect);
                            self.chat_panel.add_message(ChatMessage::info("Reconnecting..."));
                        }
                        _ => {}
                    }
                    self.state.active_modal = None;
                }
                KeyCode::Esc => {
                    self.state.active_modal = None;
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle help overlay
        if self.state.show_help {
            if key.code == KeyCode::Char('?') || key.code == KeyCode::Esc {
                self.state.show_help = false;
            }
            return Ok(());
        }

        match key.code {
            // Help
            KeyCode::Char('?') => {
                self.state.show_help = true;
            }

            // Quit
            KeyCode::Char('q') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                self.state.active_modal = Some(ModalType::ConfirmQuit);
            }
            KeyCode::Esc => {
                if !self.state.input_buffer.is_empty() {
                    self.state.input_buffer.clear();
                    self.state.cursor_position = 0;
                } else {
                    self.should_quit = true;
                }
            }

            // Input handling
            KeyCode::Enter => {
                self.submit_input();
            }
            KeyCode::Char(c) => {
                self.state.input_buffer.insert(self.state.cursor_position, c);
                self.state.cursor_position += 1;
                // Clear history navigation when typing
                self.state.history_index = None;
            }
            KeyCode::Backspace => {
                if self.state.cursor_position > 0 {
                    self.state.cursor_position -= 1;
                    self.state.input_buffer.remove(self.state.cursor_position);
                }
            }
            KeyCode::Delete => {
                if self.state.cursor_position < self.state.input_buffer.len() {
                    self.state.input_buffer.remove(self.state.cursor_position);
                }
            }
            KeyCode::Left => {
                self.state.cursor_position = self.state.cursor_position.saturating_sub(1);
            }
            KeyCode::Right => {
                self.state.cursor_position = (self.state.cursor_position + 1)
                    .min(self.state.input_buffer.len());
            }
            KeyCode::Home => {
                self.state.cursor_position = 0;
            }
            KeyCode::End => {
                self.state.cursor_position = self.state.input_buffer.len();
            }

            // Command history
            KeyCode::Up => {
                if !self.state.command_history.is_empty() {
                    let new_index = match self.state.history_index {
                        None => self.state.command_history.len() - 1,
                        Some(i) => i.saturating_sub(1),
                    };
                    self.state.history_index = Some(new_index);
                    self.state.input_buffer = self.state.command_history[new_index].clone();
                    self.state.cursor_position = self.state.input_buffer.len();
                }
            }
            KeyCode::Down => {
                if let Some(i) = self.state.history_index {
                    if i + 1 < self.state.command_history.len() {
                        self.state.history_index = Some(i + 1);
                        self.state.input_buffer = self.state.command_history[i + 1].clone();
                        self.state.cursor_position = self.state.input_buffer.len();
                    } else {
                        self.state.history_index = None;
                        self.state.input_buffer.clear();
                        self.state.cursor_position = 0;
                    }
                }
            }

            // Scroll chat
            KeyCode::PageUp => {
                self.state.chat_scroll = self.state.chat_scroll.saturating_add(5);
            }
            KeyCode::PageDown => {
                self.state.chat_scroll = self.state.chat_scroll.saturating_sub(5);
            }

            _ => {}
        }

        Ok(())
    }

    fn submit_input(&mut self) {
        let content = self.state.input_buffer.clone();
        if content.is_empty() {
            return;
        }

        // Add to history
        self.state.command_history.push(content.clone());
        if self.state.command_history.len() > 1000 {
            self.state.command_history.remove(0);
        }
        self.state.history_index = None;
        self.state.input_buffer.clear();
        self.state.cursor_position = 0;

        // Detect commands by / prefix (like Minecraft)
        if let Some(cmd) = content.strip_prefix('/') {
            // Check built-in commands first
            match cmd {
                "quit" | "exit" => {
                    self.should_quit = true;
                    return;
                }
                "clear" => {
                    self.chat_panel.clear();
                    self.chat_panel.add_message(ChatMessage::info("Chat cleared"));
                    return;
                }
                "reconnect" => {
                    self.chat_panel.add_message(ChatMessage::info("Reconnecting..."));
                    let _ = self.bot_sender.send(BotCommand::Reconnect);
                    return;
                }
                "status" => {
                    self.chat_panel.add_message(ChatMessage::info(format!(
                        "Status: {} | Server: {} | User: {}",
                        if self.bot_state.is_connected() { "Connected" } else { "Disconnected" },
                        self.status_bar.server,
                        self.status_bar.username
                    )));
                    return;
                }
                "clear-errors" => {
                    self.chat_panel.clear();
                    self.chat_panel.add_message(ChatMessage::info("Error logs cleared"));
                    return;
                }
                "toggle-errors" => {
                    self.state.error_visible = !self.state.error_visible;
                    let status = if self.state.error_visible { "shown" } else { "hidden" };
                    self.chat_panel.add_message(ChatMessage::info(format!("Error panel {}", status)));
                    return;
                }
                _ => {}
            }
            // Send to server as command
            let _ = self.bot_sender.send(BotCommand::Command(content));
        } else {
            // Send as chat message
            let _ = self.bot_sender.send(BotCommand::Chat(content));
        }
    }

    fn on_tick(&mut self) {
        // Periodic updates
    }

    fn on_bot_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Connected { server, username } => {
                self.bot_state = BotState::Connected {
                    server: server.clone(),
                    username: username.clone(),
                    since: Instant::now(),
                };
                self.status_bar.update_connection(true, server, username);
                self.chat_panel.add_message(ChatMessage::system(format!(
                    "Connected to server as {}",
                    self.status_bar.username
                )));
            }
            UiEvent::Disconnected { reason } => {
                self.bot_state = BotState::Disconnected {
                    last_error: reason.clone(),
                };
                self.status_bar.connected = false;
                self.chat_panel.add_message(ChatMessage::error(format!(
                    "Disconnected: {}",
                    reason.unwrap_or_else(|| "Unknown".to_string())
                )));
            }
            UiEvent::ChatReceived { username, message } => {
                self.chat_panel.add_message(ChatMessage::chat(username, message));
            }
            UiEvent::SystemMessage(message) => {
                self.chat_panel.add_message(ChatMessage::system(message));
            }
            UiEvent::PositionUpdate { x, y, z } => {
                self.status_bar.update_position(x, y, z);
            }
            UiEvent::HealthUpdate { health, max_health } => {
                self.status_bar.update_health(health, max_health);
            }
            UiEvent::PlayerCount(count) => {
                self.status_bar.update_online_players(count);
            }
            UiEvent::Error(error) => {
                if error.contains("azalea") || error.contains("packet") || error.contains("channel") {
                    self.error_panel.add_internal_error(error);
                } else {
                    self.chat_panel.add_message(ChatMessage::error(error));
                }
            }
            _ => {}
        }
    }
}

pub fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.autoresize()?;
    Ok(terminal)
}

pub fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

pub fn spawn_event_reader(sender: Sender<AppEvent>, tick_rate: Duration) {
    std::thread::spawn(move || {
        let mut last_tick = Instant::now();

        loop {
            let timeout = tick_rate.saturating_sub(last_tick.elapsed());

            if event::poll(timeout).unwrap_or(false) {
                match event::read() {
                    Ok(Event::Key(key)) if sender.send(AppEvent::Key(key)).is_err() => break,
                    Ok(Event::Resize(_, _)) if sender.send(AppEvent::Tick).is_err() => break,
                    _ => {}
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if sender.send(AppEvent::Tick).is_err() {
                    break;
                }
                last_tick = Instant::now();
            }
        }
    });
}
