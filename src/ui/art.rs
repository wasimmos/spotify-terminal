use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};
use std::io::Write;

use crate::ui::theme::ThemeMode;
use image::{imageops::FilterType, DynamicImage, GenericImageView};

pub const COVER_CACHE_PATH: &str = "/home/insane/spotify-terminal/.current_cover.png";

pub fn is_kitty_terminal() -> bool {
    std::env::var("KITTY_WINDOW_ID").is_ok()
        || std::env::var("TERM")
            .map(|t| t.contains("kitty") || t.contains("ghostty"))
            .unwrap_or(false)
        || std::env::var("TERM_PROGRAM")
            .map(|t| t == "ghostty" || t == "WezTerm")
            .unwrap_or(false)
}

pub fn clear_kitty_images() {
    if is_kitty_terminal() {
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(b"\x1b_Ga=d,d=A\x1b\\");
        let _ = stdout.flush();
    }
}

pub fn render_kitty_art(area: Rect) {
    if !is_kitty_terminal() {
        return;
    }
    if !std::path::Path::new(COVER_CACHE_PATH).exists() {
        return;
    }

    let square_cols = area.width.min(area.height * 2) & !1;
    let square_rows = square_cols / 2;
    if square_cols == 0 || square_rows == 0 {
        return;
    }

    let offset_x = area.x + (area.width.saturating_sub(square_cols)) / 2;
    let offset_y = area.y + (area.height.saturating_sub(square_rows)) / 2;

    let b64 = base64_encode(COVER_CACHE_PATH.as_bytes());

    // Position cursor at exact cell coordinates (1-based row and col)
    // Send Kitty graphics command: a=T (transmit/display), f=100 (png), t=f (file path),
    // c=columns, r=rows, C=1 (don't move cursor), z=1 (render above buffer)
    let seq = format!(
        "\x1b[{};{}H\x1b_Ga=T,f=100,t=f,c={},r={},C=1,z=1;{}\x1b\\",
        offset_y + 1,
        offset_x + 1,
        square_cols,
        square_rows,
        b64
    );

    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(seq.as_bytes());
    let _ = stdout.flush();
}

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

        let max_cols_from_height = area.height.saturating_mul(2);
        let square_cols = area.width.min(max_cols_from_height) & !1;
        let square_rows = square_cols / 2;

        if square_cols == 0 || square_rows == 0 {
            return;
        }

        let pixel_width = square_cols as u32;
        let pixel_height = (square_rows * 2) as u32;

        let offset_x = area.x + (area.width.saturating_sub(square_cols)) / 2;
        let offset_y = area.y + (area.height.saturating_sub(square_rows)) / 2;

        if let Some(img) = self.image {
            // High-resolution Lanczos3 downsampling
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

pub fn base64_encode(input: &[u8]) -> String {
    const SET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i] as u32;
        let b1 = if i + 1 < input.len() { input[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(SET[((triple >> 18) & 63) as usize] as char);
        out.push(SET[((triple >> 12) & 63) as usize] as char);
        if i + 1 < input.len() {
            out.push(SET[((triple >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < input.len() {
            out.push(SET[(triple & 63) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}
