use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use crate::ui::theme::ThemeMode;
use std::time::Duration;

/// Transport controls placed directly beneath the album art
pub struct AlbumSubControlsWidget {
    pub is_playing: bool,
    #[allow(dead_code)]
    pub volume: f32,
    pub theme: ThemeMode,
}

impl Widget for AlbumSubControlsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height == 0 {
            return;
        }

        let play_icon = if self.is_playing { "⏸" } else { "▶" };
        let btns = format!("⏮   {}   ⏹   ⏭", play_icon);
        let btns_len = btns.chars().count() as u16;

        let x = area.x + (area.width.saturating_sub(btns_len)) / 2;
        let y = area.y;

        buf.set_string(
            x,
            y,
            &btns,
            Style::default()
                .fg(self.theme.title_color())
                .add_modifier(Modifier::BOLD),
        );
    }
}

/// Middle progress bar and hotkey bar
pub struct ProgressBarWidget {
    pub elapsed: Duration,
    pub total: Duration,
    pub theme: ThemeMode,
}

impl Widget for ProgressBarWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 12 || area.height == 0 {
            return;
        }

        let elapsed_secs = self.elapsed.as_secs();
        let total_secs = self.total.as_secs();

        let elapsed_str = format!("{}:{:02}", elapsed_secs / 60, elapsed_secs % 60);
        let total_str = format!("{}:{:02}", total_secs / 60, total_secs % 60);

        let labels_len = (elapsed_str.len() + total_str.len() + 4) as u16;
        let bar_width = area.width.saturating_sub(labels_len) as usize;

        if bar_width > 4 {
            let progress = if total_secs > 0 {
                (elapsed_secs as f32 / total_secs as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let filled_len = ((bar_width as f32) * progress).round() as usize;

            let mut bar_str = String::with_capacity(bar_width);
            for i in 0..bar_width {
                if i < filled_len {
                    bar_str.push('━');
                } else if i == filled_len {
                    bar_str.push('●');
                } else {
                    bar_str.push('─');
                }
            }

            let y = area.y;
            buf.set_string(area.x, y, &elapsed_str, Style::default().fg(Color::Rgb(160, 160, 175)));
            buf.set_string(
                area.x + elapsed_str.len() as u16 + 2,
                y,
                &bar_str,
                Style::default().fg(self.theme.progress_bar()),
            );
            buf.set_string(
                area.right().saturating_sub(total_str.len() as u16),
                y,
                &total_str,
                Style::default().fg(Color::Rgb(160, 160, 175)),
            );
        }
    }
}
