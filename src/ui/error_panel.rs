use chrono::Local;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, Wrap},
};
use std::collections::VecDeque;

use crate::core::{Theme};

#[derive(Clone, Debug)]
pub struct ErrorLog {
    pub timestamp: String,
    pub source: String,
    pub message: String,
}

impl ErrorLog {
    pub fn new(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            source: source.into(),
            message: message.into(),
        }
    }
}

pub struct ErrorPanel {
    logs: VecDeque<ErrorLog>,
    max_logs: usize,
    theme: Theme,
}

impl Default for ErrorPanel {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

impl ErrorPanel {
    pub fn new(theme: Theme) -> Self {
        Self {
            logs: VecDeque::with_capacity(100),
            max_logs: 100,
            theme,
        }
    }

    pub fn add_log(&mut self, source: impl Into<String>, message: impl Into<String>) {
        self.logs.push_back(ErrorLog::new(source, message));
        if self.logs.len() > self.max_logs {
            self.logs.pop_front();
        }
    }

    pub fn add_internal_error(&mut self, message: impl Into<String>) {
        let msg = message.into();

        if Self::is_noise_error(&msg) {
            return;
        }

        self.add_log("internal", msg);
    }

    fn is_noise_error(message: &str) -> bool {
        let noise_patterns = [
            "Event channel has more than 1,000 items",
            "packet-event",
            "Error reading packet set_objective",
            "Invalid root type",
            "Could not set global logger",
            "bevy_log",
            "azalea::swarm",
            "azalea_client::plugins::connection",
        ];

        noise_patterns.iter().any(|pattern| message.contains(pattern))
    }

    pub fn clear(&mut self) {
        self.logs.clear();
    }

    pub fn get_log_count(&self) -> usize {
        self.logs.len()
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, scroll_offset: usize) {
        if self.logs.is_empty() {
            return;
        }

        let block = Block::default()
            .title(format!(" Error Logs ({}) ", self.logs.len()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.error));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let text: Vec<Line> = self
            .logs
            .iter()
            .map(|log| {
                let source_style = match log.source.as_str() {
                    "internal" => Style::default().fg(self.theme.warning),
                    "network" => Style::default().fg(self.theme.highlight),
                    "bot" => Style::default().fg(self.theme.primary),
                    _ => Style::default().fg(self.theme.foreground),
                };

                let content = format!("[{}] [{}] {}", log.timestamp, log.source, log.message);
                Line::styled(content, source_style)
            })
            .collect();

        let total_lines = text.len();
        let visible_lines = inner_area.height as usize;
        let start = total_lines.saturating_sub(visible_lines + scroll_offset);
        let end = (start + visible_lines).min(total_lines);
        let visible: Vec<Line> = text[start..end].to_vec();

        let paragraph = Paragraph::new(visible)
            .wrap(Wrap { trim: true });

        paragraph.render(inner_area, buf);

        // Scrollbar
        if total_lines > inner_area.height as usize {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .style(Style::default().fg(self.theme.border));
            scrollbar.render(
                inner_area,
                buf,
                &mut ratatui::widgets::ScrollbarState::new(total_lines)
                    .position(scroll_offset.min(total_lines - 1)),
            );
        }
    }
}
