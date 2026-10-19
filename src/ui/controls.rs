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

        let play_icon = if self.is_playing { "⏸" } else { "▶" };

        // Each button: padding + icon + padding
        // Layout: [⏮] [▶] [⏭]  (stop removed — space is play/pause)
        struct Btn<'a> {
            icon: &'a str,
            is_accent: bool,
        }

        let buttons = [
            Btn { icon: "⏮", is_accent: false },
            Btn { icon: play_icon, is_accent: true },
            Btn { icon: "⏭", is_accent: false },
        ];

        let accent_bg = self.theme.title_color();
        let accent_fg = Color::Rgb(15, 12, 25);
        let normal_bg = Color::Rgb(38, 34, 52);
        let normal_fg = Color::Rgb(200, 200, 220);

        // Calculate total width for centering
        // Each button is " icon " (3 chars) + 1 separator gap, except last
        let btn_width: u16 = 5; // " X " with padding
        let total_w = btn_width * buttons.len() as u16 + (buttons.len() as u16 - 1);
        let start_x = area.x + area.width.saturating_sub(total_w) / 2;
        let y = area.y;

        let mut cx = start_x;
        for (i, btn) in buttons.iter().enumerate() {
            let (fg, bg) = if btn.is_accent {
                (accent_fg, accent_bg)
            } else {
                (normal_fg, normal_bg)
            };

            let style = Style::default()
                .fg(fg)
                .bg(bg)
                .add_modifier(Modifier::BOLD);

            // Render " icon " (padded)
            let label = format!(" {} ", btn.icon);
            buf.set_string(cx, y, &label, style);
            cx += label.chars().count() as u16;

            // Separator between buttons (not after last)
            if i < buttons.len() - 1 {
                buf.set_string(
                    cx,
                    y,
                    "│",
                    Style::default().fg(Color::Rgb(60, 55, 80)),
                );
                cx += 1;
            }
        }

        // Second row: volume indicator (if height allows)
        if area.height >= 2 {
            let vol_pct = 85u8; // TODO: wire to app.volume when controls get click support
            let vol_str = format!("🔊 {}%", vol_pct);
            let vx = area.x + area.width.saturating_sub(vol_str.chars().count() as u16) / 2;
            buf.set_string(
                vx,
                area.y + 1,
                &vol_str,
                Style::default().fg(Color::Rgb(110, 110, 130)),
            );
        }
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
