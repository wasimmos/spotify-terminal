use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use crate::ui::theme::Mood;
use std::time::Duration;

pub struct TransportControlsWidget {
    pub is_playing: bool,
    pub elapsed: Duration,
    pub total: Duration,
    pub volume: f32,
    pub mood: Mood,
}

impl Widget for TransportControlsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 10 {
            return;
        }

        let elapsed_secs = self.elapsed.as_secs();
        let total_secs = self.total.as_secs();
        let elapsed_str = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);
        let total_str = format!("{:02}:{:02}", total_secs / 60, total_secs % 60);

        // Line 1: Progress bar
        let time_labels_len = (elapsed_str.len() + total_str.len() + 2) as u16;
        let bar_width = area.width.saturating_sub(time_labels_len + 2) as usize;

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

            // Draw time line
            let y_time = area.y;
            buf.set_string(
                area.x,
                y_time,
                &elapsed_str,
                Style::default().fg(Color::DarkGray),
            );
            buf.set_string(
                area.x + elapsed_str.len() as u16 + 1,
                y_time,
                &bar_str,
                Style::default().fg(self.mood.primary_color()),
            );
            buf.set_string(
                area.right().saturating_sub(total_str.len() as u16),
                y_time,
                &total_str,
                Style::default().fg(Color::DarkGray),
            );
        }

        // Line 2: Playback buttons
        if area.height >= 2 {
            let y_btns = area.y + 1;
            let play_icon = if self.is_playing { " ⏸ " } else { " ▶ " };
            let btns = format!("  ⏮  {}  ⏭   ⏹    🔊 {}%  ", play_icon, (self.volume * 100.0) as u32);
            let btns_len = btns.chars().count() as u16;
            let x_btns = area.x + (area.width.saturating_sub(btns_len)) / 2;

            buf.set_string(
                x_btns,
                y_btns,
                &btns,
                Style::default()
                    .fg(self.mood.primary_color())
                    .add_modifier(Modifier::BOLD),
            );
        }
    }
}
