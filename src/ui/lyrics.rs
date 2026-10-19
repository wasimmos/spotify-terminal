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
            let lines = [
                "♪  No Synced Lyrics Available  ♪",
                "Play a track on Spotify to get started",
            ];
            let center_y = area.y + area.height / 2;
            for (i, msg) in lines.iter().enumerate() {
                let y = center_y.saturating_add(i as u16);
                if y >= area.bottom() {
                    break;
                }
                let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
                let style = if i == 0 {
                    Style::default().fg(self.theme.title_color()).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Rgb(90, 90, 110))
                };
                buf.set_string(x, y, msg, style);
            }
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

        // Position lyrics higher up in the card: active line around row 3, so previous 3 lines start at top
        let active_y = (area.y + 3).min(area.bottom().saturating_sub(1));

        for (i, line) in self.lyrics.iter().enumerate() {
            let diff = i as i32 - active_idx as i32;
            // Show only 6-7 lines at once (active line ± 3 lines)
            if diff.abs() > 3 {
                continue;
            }

            let line_y = active_y as i32 + diff;

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
