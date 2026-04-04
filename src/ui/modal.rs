use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Block, Borders, Clear, Paragraph, Wrap, Widget},
};

use crate::core::{ModalType, Theme};

pub struct Modal;

impl Modal {
    pub fn render(modal_type: &ModalType, area: Rect, buf: &mut Buffer, theme: &Theme) {
        // Clear background
        Clear.render(area, buf);

        let (title, content, style) = match modal_type {
            ModalType::ConfirmQuit => (
                " Confirm ",
                "Are you sure you want to quit?\n\nPress Enter to confirm, Esc to cancel.".to_string(),
                Style::default().fg(theme.warning),
            ),
            ModalType::ConfirmReconnect => (
                " Confirm Reconnect ",
                "Reconnect to server?\n\nPress Enter to confirm, Esc to cancel.".to_string(),
                Style::default().fg(theme.primary),
            ),
            ModalType::Error(msg) => (
                " Error ",
                format!("{}\n\nPress Esc to close.", msg),
                Style::default().fg(theme.error),
            ),
            ModalType::Help => (
                " Help ",
                "Keyboard shortcuts:\n\
                \n\
                Ctrl+Q / Esc  - Quit\n\
                Tab           - Toggle Chat/Command mode\n\
                Up/Down       - Command history\n\
                PageUp/Down   - Scroll chat\n\
                ?             - Toggle help\n\
                \n\
                Press Esc to close."
                    .to_string(),
                Style::default().fg(theme.foreground),
            ),
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(style);

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        paragraph.render(area, buf);
    }
}
