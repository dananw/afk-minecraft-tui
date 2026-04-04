use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::core::{Theme, UiState};

pub struct InputPanel {
    theme: Theme,
}

impl Default for InputPanel {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

impl InputPanel {
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, state: &UiState) {
        let prefix = "> ";
        let display_text = format!("{}{}", prefix, state.input_buffer);

        let title = " Input (Chat - ? for help) ";

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.success));

        let paragraph = Paragraph::new(display_text)
            .block(block)
            .wrap(Wrap { trim: true });

        paragraph.render(area, buf);

        // Render cursor
        let cursor_x = area.x + prefix.len() as u16 + state.cursor_position as u16 + 1;
        let cursor_y = area.y + 1;

        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            if let Some(cell) = buf.cell_mut(Position::new(cursor_x, cursor_y)) {
                let style = cell.style().add_modifier(ratatui::style::Modifier::REVERSED);
                cell.set_style(style);
            }
        }
    }
}
