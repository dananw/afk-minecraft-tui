use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::core::{BotState, Theme};

pub struct StatusBar {
    pub connected: bool,
    pub server: String,
    pub username: String,
    pub online_players: usize,
    pub health: f32,
    pub max_health: f32,
    pub position: (f64, f64, f64),
    pub fps: f32,
    theme: Theme,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

impl StatusBar {
    pub fn new(theme: Theme) -> Self {
        Self {
            connected: false,
            server: String::new(),
            username: String::new(),
            online_players: 0,
            health: 20.0,
            max_health: 20.0,
            position: (0.0, 0.0, 0.0),
            fps: 0.0,
            theme,
        }
    }

    pub fn update_connection(
        &mut self,
        connected: bool,
        server: impl Into<String>,
        username: impl Into<String>,
    ) {
        self.connected = connected;
        self.server = server.into();
        self.username = username.into();
    }

    pub fn update_health(&mut self, health: f32, max_health: f32) {
        self.health = health;
        self.max_health = max_health;
    }

    pub fn update_position(&mut self, x: f64, y: f64, z: f64) {
        self.position = (x, y, z);
    }

    pub fn update_online_players(&mut self, count: usize) {
        self.online_players = count;
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, bot_state: &BotState) {
        let status_text = if self.connected {
            ("● CONNECTED", self.theme.success)
        } else {
            ("● DISCONNECTED", self.theme.error)
        };

        let health_bar = self.render_health_bar();

        // Show different info based on state
        let state_info = match bot_state {
            BotState::Connecting { server, .. } => {
                format!(" | Connecting to {}...", server)
            }
            BotState::Reconnecting { server, attempt, delay_secs } => {
                format!(" | Reconnecting to {} (attempt {}, {}s)", server, attempt, delay_secs)
            }
            _ => String::new(),
        };

        let text = format!(
            " {} | Server: {} | User: {} | Players: {} | {} | Pos: ({:.1}, {:.1}, {:.1}) | {:.0} FPS{}",
            status_text.0,
            self.server,
            self.username,
            self.online_players,
            health_bar,
            self.position.0,
            self.position.1,
            self.position.2,
            self.fps,
            state_info
        );

        let paragraph = Paragraph::new(text)
            .style(Style::default().fg(self.theme.foreground).bg(self.theme.background));

        paragraph.render(area, buf);
    }

    fn render_health_bar(&self) -> String {
        let health_percentage = self.health / self.max_health;
        let filled_hearts = (health_percentage * 10.0) as usize;
        let empty_hearts = 10 - filled_hearts;

        format!(
            "[{}{}] {:.1}/{:.1}",
            "❤".repeat(filled_hearts),
            "♡".repeat(empty_hearts),
            self.health,
            self.max_health
        )
    }
}
