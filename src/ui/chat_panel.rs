use chrono::Local;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, Wrap},
};
use std::collections::VecDeque;

use crate::core::Theme;

#[derive(Clone, Debug)]
pub enum MessageType {
    System,
    Chat(#[allow(dead_code)] String),
    Command,
    Error,
    Info,
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub timestamp: String,
    pub message_type: MessageType,
    pub content: String,
}

impl ChatMessage {
    pub fn new(message_type: MessageType, content: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message_type,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(MessageType::System, content)
    }

    pub fn chat(username: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(MessageType::Chat(username.into()), content)
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self::new(MessageType::Error, content)
    }

    pub fn info(content: impl Into<String>) -> Self {
        Self::new(MessageType::Info, content)
    }

    pub fn command(content: impl Into<String>) -> Self {
        Self::new(MessageType::Command, content)
    }
}

pub struct ChatPanel {
    messages: VecDeque<ChatMessage>,
    max_messages: usize,
    theme: Theme,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

impl ChatPanel {
    pub fn new(theme: Theme) -> Self {
        Self {
            messages: VecDeque::with_capacity(1000),
            max_messages: 1000,
            theme,
        }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push_back(message);
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, scroll_offset: usize) {
        let block = Block::default()
            .title(" Chat ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.border));

        let inner_area = block.inner(area);
        block.render(area, buf);

        if self.messages.is_empty() {
            let empty_text = Paragraph::new("No messages yet...")
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(self.theme.foreground));
            empty_text.render(inner_area, buf);
            return;
        }

        let text: Vec<Line> = self
            .messages
            .iter()
            .map(|msg| {
                let prefix_style = match &msg.message_type {
                    MessageType::System => Style::default().fg(self.theme.warning),
                    MessageType::Command => Style::default().fg(self.theme.primary),
                    MessageType::Error => Style::default().fg(self.theme.error),
                    MessageType::Info => Style::default().fg(self.theme.success),
                    MessageType::Chat(_) => Style::default().fg(self.theme.foreground),
                };

                let prefix = match &msg.message_type {
                    MessageType::System => "[SYS]",
                    MessageType::Chat(_) => "[C]",
                    MessageType::Command => "[CMD]",
                    MessageType::Error => "[ERR]",
                    MessageType::Info => "[INFO]",
                };

                let timestamp_span = Span::styled(
                    format!("{} ", msg.timestamp),
                    Style::default().fg(Color::DarkGray),
                );

                let prefix_span = Span::styled(
                    format!("{} ", prefix),
                    prefix_style,
                );

                // Parse ANSI colors from Minecraft chat, or use fallback style
                let content_spans = parse_ansi_to_spans(&msg.content, prefix_style);

                let mut spans = vec![timestamp_span, prefix_span];
                spans.extend(content_spans);

                Line::from(spans)
            })
            .collect();

        // Calculate visible range based on scroll
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

/// Parse ANSI escape sequences into ratatui Spans with proper colors.
/// Handles SGR sequences: \x1b[...m (colors, bold, italic, reset, etc.)
fn parse_ansi_to_spans<'a>(input: &'a str, fallback_style: Style) -> Vec<Span<'a>> {
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut current_style = fallback_style;
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut text_start = 0;

    while i < len {
        if bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[' {
            // Flush text before this escape
            if text_start < i {
                let text = &input[text_start..i];
                if !text.is_empty() {
                    spans.push(Span::styled(text.to_string(), current_style));
                }
            }

            // Parse SGR sequence: ESC [ <params> m
            let seq_start = i + 2;
            let mut seq_end = seq_start;
            while seq_end < len && bytes[seq_end] != b'm' && seq_end - seq_start < 32 {
                seq_end += 1;
            }

            if seq_end < len && bytes[seq_end] == b'm' {
                let seq = &input[seq_start..seq_end];
                current_style = apply_sgr(seq, current_style, fallback_style);
                i = seq_end + 1;
            } else {
                // Malformed sequence, skip ESC
                i += 1;
            }
            text_start = i;
        } else {
            i += 1;
        }
    }

    // Flush remaining text
    if text_start < len {
        let text = &input[text_start..];
        if !text.is_empty() {
            spans.push(Span::styled(text.to_string(), current_style));
        }
    }

    // If no spans were created (no ANSI codes), return the whole string
    if spans.is_empty() && !input.is_empty() {
        spans.push(Span::styled(input.to_string(), fallback_style));
    }

    spans
}

/// Apply SGR (Select Graphic Rendition) parameters to a style.
/// Handles: reset, bold, italic, 4-bit colors, 8-bit colors, 24-bit RGB colors.
fn apply_sgr(seq: &str, mut style: Style, fallback_style: Style) -> Style {
    if seq.is_empty() {
        return fallback_style; // ESC[m = reset
    }

    let params: Vec<u8> = seq
        .split(';')
        .filter_map(|s| s.parse::<u8>().ok())
        .collect();

    let mut idx = 0;
    while idx < params.len() {
        match params[idx] {
            0 => style = fallback_style,                        // Reset
            1 => style = style.add_modifier(Modifier::BOLD),    // Bold
            3 => style = style.add_modifier(Modifier::ITALIC),  // Italic
            4 => style = style.add_modifier(Modifier::UNDERLINED), // Underline
            22 => style = style.remove_modifier(Modifier::BOLD),
            23 => style = style.remove_modifier(Modifier::ITALIC),
            24 => style = style.remove_modifier(Modifier::UNDERLINED),

            // Standard foreground colors (30-37)
            30 => style = style.fg(Color::Black),
            31 => style = style.fg(Color::Red),
            32 => style = style.fg(Color::Green),
            33 => style = style.fg(Color::Yellow),
            34 => style = style.fg(Color::Blue),
            35 => style = style.fg(Color::Magenta),
            36 => style = style.fg(Color::Cyan),
            37 => style = style.fg(Color::White),

            // Bright foreground colors (90-97)
            90 => style = style.fg(Color::DarkGray),
            91 => style = style.fg(Color::LightRed),
            92 => style = style.fg(Color::LightGreen),
            93 => style = style.fg(Color::LightYellow),
            94 => style = style.fg(Color::LightBlue),
            95 => style = style.fg(Color::LightMagenta),
            96 => style = style.fg(Color::LightCyan),
            97 => style = style.fg(Color::White),

            // Extended color: 38;2;r;g;b (24-bit RGB) or 38;5;n (256-color)
            38 => {
                if idx + 1 < params.len() {
                    // Need to re-parse from raw sequence for RGB values > 255
                    // But params are u8 so we handle what we can
                    let color = parse_extended_color(&seq, idx, &params);
                    if let Some((c, advance)) = color {
                        style = style.fg(c);
                        idx += advance;
                    }
                }
            }

            // Extended background: 48;2;r;g;b or 48;5;n
            48 => {
                if idx + 1 < params.len() {
                    let color = parse_extended_color(&seq, idx, &params);
                    if let Some((c, advance)) = color {
                        style = style.bg(c);
                        idx += advance;
                    }
                }
            }

            // Default foreground
            39 => style = style.fg(fallback_style.fg.unwrap_or(Color::Reset)),
            // Default background
            49 => style = style.bg(Color::Reset),

            _ => {} // Ignore unknown
        }
        idx += 1;
    }

    style
}

/// Parse extended color sequences (38;2;r;g;b or 38;5;n).
/// Returns the Color and how many additional params to skip.
fn parse_extended_color(seq: &str, start_idx: usize, _params: &[u8]) -> Option<(Color, usize)> {
    // Re-parse from the raw sequence string to handle values > 255
    let parts: Vec<&str> = seq.split(';').collect();

    // Find the position corresponding to start_idx
    if start_idx + 1 >= parts.len() {
        return None;
    }

    let mode_str = parts.get(start_idx + 1)?;
    let mode: u8 = mode_str.parse().ok()?;

    match mode {
        2 => {
            // 24-bit RGB: 38;2;r;g;b
            if start_idx + 4 < parts.len() {
                let r: u8 = parts.get(start_idx + 2)?.parse().ok()?;
                let g: u8 = parts.get(start_idx + 3)?.parse().ok()?;
                let b: u8 = parts.get(start_idx + 4)?.parse().ok()?;
                Some((Color::Rgb(r, g, b), 4))
            } else {
                None
            }
        }
        5 => {
            // 256-color: 38;5;n
            if start_idx + 2 < parts.len() {
                let n: u8 = parts.get(start_idx + 2)?.parse().ok()?;
                Some((Color::Indexed(n), 2))
            } else {
                None
            }
        }
        _ => None,
    }
}
