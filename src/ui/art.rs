use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

use crate::ui::theme::ThemeMode;
use image::{imageops::FilterType, DynamicImage, GenericImageView};

pub struct AlbumArtWidget<'a> {
    theme: ThemeMode,
    image: Option<&'a DynamicImage>,
    time: f32,
}

impl<'a> AlbumArtWidget<'a> {
    pub fn new(theme: ThemeMode, image: Option<&'a DynamicImage>, time: f32) -> Self {
        Self { theme, image, time }
    }
}

impl<'a> Widget for AlbumArtWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 || area.height < 2 {
            return;
        }

        // Terminal character aspect ratio: 1 column wide x 0.5 row tall per half-block ('▀').
        // Therefore, for a 1:1 square image:
        // pixel_width = square_cols
        // pixel_height = square_rows * 2
        // where square_cols = square_rows * 2.
        let max_cols_from_height = area.height.saturating_mul(2);
        let square_cols = area.width.min(max_cols_from_height) & !1;
        let square_rows = square_cols / 2;

        if square_cols == 0 || square_rows == 0 {
            return;
        }

        let pixel_width = square_cols as u32;
        let pixel_height = (square_rows * 2) as u32;

        let offset_x = area.x + (area.width.saturating_sub(square_cols)) / 2;
        let offset_y = area.y;

        if let Some(img) = self.image {
            // High-quality Lanczos3 filter preserves fine details and sharp edges
            let resized = img.resize_exact(pixel_width, pixel_height, FilterType::Lanczos3);

            for y_cell in 0..square_rows {
                for x in 0..square_cols {
                    let top_y = (y_cell * 2) as u32;
                    let bot_y = (y_cell * 2 + 1) as u32;

                    let top_pixel = resized.get_pixel(x as u32, top_y);
                    let bot_pixel = if bot_y < pixel_height {
                        resized.get_pixel(x as u32, bot_y)
                    } else {
                        top_pixel
                    };

                    let top_color = Color::Rgb(
                        boost_color(top_pixel[0]),
                        boost_color(top_pixel[1]),
                        boost_color(top_pixel[2]),
                    );
                    let bot_color = Color::Rgb(
                        boost_color(bot_pixel[0]),
                        boost_color(bot_pixel[1]),
                        boost_color(bot_pixel[2]),
                    );

                    let target_x = offset_x + x;
                    let target_y = offset_y + y_cell;

                    if target_x < area.right() && target_y < area.bottom() {
                        buf[(target_x, target_y)]
                            .set_char('▀')
                            .set_style(Style::default().fg(top_color).bg(bot_color));
                    }
                }
            }
        } else {
            // Procedural synthwave cover fallback
            for y_cell in 0..square_rows {
                for x in 0..square_cols {
                    let top_y = (y_cell * 2) as f32;
                    let bot_y = (y_cell * 2 + 1) as f32;

                    let top_color = procedural_art_color(
                        x as f32,
                        top_y,
                        pixel_width as f32,
                        pixel_height as f32,
                        self.theme,
                        self.time,
                    );
                    let bot_color = procedural_art_color(
                        x as f32,
                        bot_y,
                        pixel_width as f32,
                        pixel_height as f32,
                        self.theme,
                        self.time,
                    );

                    let target_x = offset_x + x;
                    let target_y = offset_y + y_cell;

                    if target_x < area.right() && target_y < area.bottom() {
                        buf[(target_x, target_y)]
                            .set_char('▀')
                            .set_style(Style::default().fg(top_color).bg(bot_color));
                    }
                }
            }
        }
    }
}

#[inline]
fn boost_color(c: u8) -> u8 {
    let f = c as f32 / 255.0;
    // S-curve contrast enhancement
    let s = if f < 0.5 {
        2.0 * f * f
    } else {
        1.0 - 2.0 * (1.0 - f) * (1.0 - f)
    };
    (s * 255.0).clamp(0.0, 255.0) as u8
}

fn procedural_art_color(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    theme: ThemeMode,
    time: f32,
) -> Color {
    let nx = (x / width) * 2.0 - 1.0;
    let ny = (y / height) * 2.0 - 1.0;
    let horizon = 0.2;

    if ny < horizon {
        let sun_dist = (nx * nx + (ny + 0.1).powi(2)).sqrt();
        if sun_dist < 0.55 {
            let line = (ny * 20.0).sin();
            if ny > -0.05 && line > 0.45 {
                theme.card_border()
            } else {
                theme.title_color()
            }
        } else {
            theme.bar_color_at(0.15)
        }
    } else {
        let depth = (ny - horizon) / (1.0 - horizon);
        let grid_y = (1.0 / (depth + 0.1) + time * 0.5) % 1.0;
        let grid_x = (nx / (depth + 0.2) * 1.5) % 1.0;

        if grid_y > 0.88 || grid_x.abs() > 0.88 {
            theme.progress_bar()
        } else {
            theme.card_border()
        }
    }
}
