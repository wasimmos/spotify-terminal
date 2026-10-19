use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::Widget,
};

use crate::audio::VisualizerAnalyzer;
use crate::ui::theme::ThemeMode;

const SUB_BLOCKS: [char; 9] = [' ', ' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub struct VisualizerWidget<'a> {
    analyzer: &'a VisualizerAnalyzer,
    theme: ThemeMode,
}

impl<'a> VisualizerWidget<'a> {
    pub fn new(analyzer: &'a VisualizerAnalyzer, theme: ThemeMode) -> Self {
        Self { analyzer, theme }
    }
}

impl<'a> Widget for VisualizerWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 8 || area.height < 2 {
            return;
        }

        // Larger, chunky bars with breathing room
        let bar_width: u16 = if area.width > 130 {
            5
        } else if area.width > 90 {
            4
        } else {
            3
        };
        let gap_width: u16 = if bar_width >= 4 { 2 } else { 1 };
        let slot_width = bar_width + gap_width;

        let num_bars = ((area.width + gap_width) / slot_width).saturating_sub(2).max(12) as usize;
        if num_bars == 0 {
            return;
        }

        // Center the bars horizontally
        let total_drawn_width = (num_bars as u16 * slot_width).saturating_sub(gap_width);
        let left_offset = area.x + (area.width.saturating_sub(total_drawn_width)) / 2;

        let total_height = area.height as f32;

        for i in 0..num_bars {
            let bar_start_x = left_offset + (i as u16 * slot_width);
            if bar_start_x + bar_width > area.right() {
                break;
            }

            // Frequency ratio for smooth color styling
            let freq_ratio = i as f32 / num_bars as f32;
            let bar_color = self.theme.bar_color_at(freq_ratio);
            let bar_style = Style::default().fg(bar_color);

            // Get amplitude from analyzer (clamped to available bands)
            let amplitude = if i < self.analyzer.bands.len() {
                self.analyzer.bands[i]
            } else {
                0.0
            };

            let bar_height = (amplitude.clamp(0.0, 1.0) * total_height).max(0.0);

            for row in 0..area.height {
                let y = area.bottom() - 1 - row;
                let cell_bottom = row as f32;
                let cell_top = (row + 1) as f32;

                let ch = if bar_height >= cell_top {
                    SUB_BLOCKS[8] // Solid '█'
                } else if bar_height > cell_bottom {
                    // Fractional sub-block for silky smooth animation
                    let frac = (bar_height - cell_bottom) / 1.0;
                    let level = (frac * 8.0).round() as usize;
                    SUB_BLOCKS[level.clamp(1, 8)]
                } else {
                    ' '
                };

                // Render character across the full bar width
                for col in 0..bar_width {
                    let x = bar_start_x + col;
                    if x < area.right() {
                        buf[(x, y)].set_char(ch).set_style(bar_style);
                    }
                }
            }
        }
    }
}
