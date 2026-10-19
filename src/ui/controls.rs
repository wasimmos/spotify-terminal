use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

use crate::ui::theme::ThemeMode;
use std::time::Duration;


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
