use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use crate::ui::theme::ThemeMode;
use std::time::Duration;

/// Prominent playback controls placed directly beneath the album art
pub struct AlbumSubControlsWidget {
    pub is_playing: bool,
    pub theme: ThemeMode,
}

impl Widget for AlbumSubControlsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height == 0 {
            return;
        }

        let play_btn = if self.is_playing { " ⏸ " } else { " ▶ " };

        // Styled button segments
        let prev_btn = " ⏮ ";
        let stop_btn = " ⏹ ";
        let next_btn = " ⏭ ";

        let full_text = format!("{} {} {} {}", prev_btn, play_btn, stop_btn, next_btn);
        let len = full_text.chars().count() as u16;
        let x = area.x + (area.width.saturating_sub(len)) / 2;
        let y = area.y;

        // Render with high-contrast accent styling
        let active_play_style = if self.is_playing {
            Style::default()
                .fg(Color::Rgb(20, 20, 30))
                .bg(self.theme.title_color())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.theme.title_color())
                .bg(Color::Rgb(40, 35, 55))
                .add_modifier(Modifier::BOLD)
        };

        let normal_style = Style::default()
            .fg(Color::Rgb(220, 220, 235))
            .bg(Color::Rgb(30, 28, 42))
            .add_modifier(Modifier::BOLD);

        let mut cur_x = x;

        // ⏮ Prev
        buf.set_string(cur_x, y, prev_btn, normal_style);
        cur_x += prev_btn.chars().count() as u16 + 1;

        // ▶ / ⏸ Play
        buf.set_string(cur_x, y, play_btn, active_play_style);
        cur_x += play_btn.chars().count() as u16 + 1;

        // ⏹ Stop
        buf.set_string(cur_x, y, stop_btn, normal_style);
        cur_x += stop_btn.chars().count() as u16 + 1;

        // ⏭ Next
        buf.set_string(cur_x, y, next_btn, normal_style);
    }
}

/// Middle progress bar
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
            buf.set_string(area.x, y, &elapsed_str, Style::default().fg(Color::Rgb(170, 170, 185)));
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
                Style::default().fg(Color::Rgb(170, 170, 185)),
            );
        }
    }
}
