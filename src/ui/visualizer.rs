use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use crate::audio::VisualizerAnalyzer;
use crate::ui::theme::Mood;

const SUB_BLOCKS: [char; 9] = [' ', ' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub struct VisualizerWidget<'a> {
    analyzer: &'a VisualizerAnalyzer,
    mood: Mood,
    show_peaks: bool,
}

impl<'a> VisualizerWidget<'a> {
    pub fn new(analyzer: &'a VisualizerAnalyzer, mood: Mood) -> Self {
        Self {
            analyzer,
            mood,
            show_peaks: true,
        }
    }
}

impl<'a> Widget for VisualizerWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 2 {
            return;
        }

        let num_bars = self.analyzer.bands.len().min(area.width as usize);
        if num_bars == 0 {
            return;
        }

        let total_height = area.height as f32;
        let left_offset = area.x + (area.width.saturating_sub(num_bars as u16)) / 2;

        for (i, (&amplitude, &peak)) in self.analyzer.bands.iter().zip(&self.analyzer.peaks).take(num_bars).enumerate() {
            let x = left_offset + i as u16;
            if x >= area.right() {
                break;
            }

            // Frequency ratio along spectrum for gradient coloring
            let freq_ratio = i as f32 / num_bars as f32;
            let bar_color = self.mood.gradient_at(freq_ratio);

            // Bar height in fractional rows
            let bar_height = (amplitude.clamp(0.0, 1.0) * total_height).max(0.0);
            let peak_row = ((1.0 - peak.clamp(0.0, 1.0)) * total_height) as u16;

            for row in 0..area.height {
                let y = area.bottom() - 1 - row;
                let cell_bottom = row as f32;
                let cell_top = (row + 1) as f32;

                let (ch, style) = if bar_height >= cell_top {
                    // Fully covered cell
                    (SUB_BLOCKS[8], Style::default().fg(bar_color))
                } else if bar_height > cell_bottom {
                    // Partially covered top cell (8 sub-levels)
                    let frac = (bar_height - cell_bottom) / 1.0;
                    let level = (frac * 8.0).round() as usize;
                    let idx = level.clamp(1, 8);
                    (SUB_BLOCKS[idx], Style::default().fg(bar_color))
                } else if self.show_peaks && (area.height - 1 - row) == peak_row && peak > 0.05 {
                    // Floating peak marker
                    (
                        '▔',
                        Style::default()
                            .fg(Color::Rgb(255, 255, 255))
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    (' ', Style::default())
                };

                buf[(x, y)].set_char(ch).set_style(style);
            }
        }
    }
}
