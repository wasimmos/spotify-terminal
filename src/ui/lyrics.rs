use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use crate::ui::theme::Mood;

#[derive(Clone, Debug)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
}

pub struct LyricsWidget<'a> {
    lyrics: &'a [LyricLine],
    current_time_ms: u64,
    mood: Mood,
}

impl<'a> LyricsWidget<'a> {
    pub fn new(lyrics: &'a [LyricLine], current_time_ms: u64, mood: Mood) -> Self {
        Self {
            lyrics,
            current_time_ms,
            mood,
        }
    }
}

impl<'a> Widget for LyricsWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 3 || self.lyrics.is_empty() {
            let msg = if self.lyrics.is_empty() {
                "♪ No Synced Lyrics Available ♪"
            } else {
                "..."
            };
            let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
            let y = area.y + area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(Color::DarkGray));
            return;
        }

        // Find current lyric line index
        let mut active_idx = 0;
        for (i, line) in self.lyrics.iter().enumerate() {
            if self.current_time_ms >= line.time_ms {
                active_idx = i;
            } else {
                break;
            }
        }

        let center_y = area.y + area.height / 2;

        for (i, line) in self.lyrics.iter().enumerate() {
            let diff = i as i32 - active_idx as i32;
            // Spacing: 2 terminal rows per lyric line for breathing room and readability
            let line_y = center_y as i32 + diff * 2;

            if line_y < area.y as i32 || line_y >= area.bottom() as i32 {
                continue;
            }

            let y = line_y as u16;
            let is_active = diff == 0;

            let (text_to_draw, style) = if is_active {
                let formatted = format!("✦  {}  ✦", line.text);
                (
                    formatted,
                    Style::default()
                        .fg(self.mood.primary_color())
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                let distance = diff.abs();
                let color = match distance {
                    1 => self.mood.secondary_color(),
                    2 => Color::Rgb(120, 120, 140),
                    3 => Color::Rgb(80, 80, 95),
                    _ => Color::Rgb(50, 50, 65),
                };
                (line.text.clone(), Style::default().fg(color))
            };

            // Center align text
            let text_len = text_to_draw.chars().count() as u16;
            let x = if text_len < area.width {
                area.x + (area.width - text_len) / 2
            } else {
                area.x
            };

            buf.set_stringn(x, y, &text_to_draw, area.width as usize, style);
        }
    }
}
