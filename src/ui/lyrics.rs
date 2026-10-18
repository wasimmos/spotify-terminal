use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};
use crate::ui::theme::ThemeMode;

#[derive(Clone, Debug)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
}

pub struct LyricsWidget<'a> {
    lyrics: &'a [LyricLine],
    current_time_ms: u64,
    theme: ThemeMode,
}

impl<'a> LyricsWidget<'a> {
    pub fn new(lyrics: &'a [LyricLine], current_time_ms: u64, theme: ThemeMode) -> Self {
        Self {
            lyrics,
            current_time_ms,
            theme,
        }
    }
}

impl<'a> Widget for LyricsWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 2 || self.lyrics.is_empty() {
            let msg = "♪ Instrumental / No Synced Lyrics ♪";
            let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
            let y = area.y + area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(Color::DarkGray));
            return;
        }

        // Find the active line index
        let mut active_idx = 0;
        for (i, line) in self.lyrics.iter().enumerate() {
            if self.current_time_ms >= line.time_ms {
                active_idx = i;
            } else {
                break;
            }
        }

        let center_y = area.y + (area.height / 2);

        for (i, line) in self.lyrics.iter().enumerate() {
            let diff = i as i32 - active_idx as i32;
            // 1 empty terminal row between lyrics for elegant typography & breathing room
            let line_y = center_y as i32 + diff;

            if line_y < area.y as i32 || line_y >= area.bottom() as i32 {
                continue;
            }

            let y = line_y as u16;
            let is_active = diff == 0;

            let (prefix, style) = if is_active {
                (
                    "▶  ",
                    Style::default()
                        .fg(self.theme.active_lyric())
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                let dist = diff.abs();
                let color = match dist {
                    1 => Color::Rgb(210, 210, 225), // Soft clean white-gray
                    2 => Color::Rgb(140, 140, 155), // Muted gray
                    3 => Color::Rgb(95, 95, 110),   // Faint gray
                    _ => Color::Rgb(60, 60, 75),    // Deep faded gray
                };
                ("   ", Style::default().fg(color))
            };

            let full_text = format!("{}{}", prefix, line.text);

            // Indented cleanly
            let x = area.x + 2;
            if x < area.right() {
                let max_w = (area.right() - x) as usize;
                buf.set_stringn(x, y, &full_text, max_w, style);
            }
        }
    }
}
