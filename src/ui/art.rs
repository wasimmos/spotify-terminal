use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

use crate::ui::theme::Mood;
use image::{imageops::FilterType, DynamicImage, GenericImageView};

pub struct AlbumArtWidget<'a> {
    mood: Mood,
    image: Option<&'a DynamicImage>,
    time: f32,
}

impl<'a> AlbumArtWidget<'a> {
    pub fn new(mood: Mood, image: Option<&'a DynamicImage>, time: f32) -> Self {
        Self { mood, image, time }
    }
}

impl<'a> Widget for AlbumArtWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 || area.height < 2 {
            return;
        }

        let pixel_width = area.width as u32;
        let pixel_height = (area.height * 2) as u32; // 2 vertical pixels per cell using '▀'

        if let Some(img) = self.image {
            // Resize image to fit area
            let resized = img.resize_exact(pixel_width, pixel_height, FilterType::Triangle);
            for y_cell in 0..area.height {
                for x in 0..area.width {
                    let top_y = (y_cell * 2) as u32;
                    let bot_y = (y_cell * 2 + 1) as u32;

                    let top_pixel = resized.get_pixel(x as u32, top_y);
                    let bot_pixel = if bot_y < pixel_height {
                        resized.get_pixel(x as u32, bot_y)
                    } else {
                        top_pixel
                    };

                    let top_color = Color::Rgb(top_pixel[0], top_pixel[1], top_pixel[2]);
                    let bot_color = Color::Rgb(bot_pixel[0], bot_pixel[1], bot_pixel[2]);

                    buf[(area.x + x, area.y + y_cell)]
                        .set_char('▀')
                        .set_style(Style::default().fg(top_color).bg(bot_color));
                }
            }
        } else {
            // Generate aesthetic procedural retro/synthwave album art
            for y_cell in 0..area.height {
                for x in 0..area.width {
                    let top_y = (y_cell * 2) as f32;
                    let bot_y = (y_cell * 2 + 1) as f32;

                    let top_color = procedural_art_color(
                        x as f32,
                        top_y,
                        pixel_width as f32,
                        pixel_height as f32,
                        self.mood,
                        self.time,
                    );
                    let bot_color = procedural_art_color(
                        x as f32,
                        bot_y,
                        pixel_width as f32,
                        pixel_height as f32,
                        self.mood,
                        self.time,
                    );

                    buf[(area.x + x, area.y + y_cell)]
                        .set_char('▀')
                        .set_style(Style::default().fg(top_color).bg(bot_color));
                }
            }
        }
    }
}

/// Generates a synthwave sun + neon grid + horizon glow album artwork procedurally
fn procedural_art_color(x: f32, y: f32, width: f32, height: f32, mood: Mood, time: f32) -> Color {
    let nx = (x / width) * 2.0 - 1.0;  // -1.0 .. 1.0
    let ny = (y / height) * 2.0 - 1.0; // -1.0 .. 1.0

    let horizon = 0.15;

    if ny < horizon {
        // Sky & Glowing Sun
        let sun_x = 0.0;
        let sun_y = -0.15;
        let sun_radius = 0.52;
        let dist = ((nx - sun_x).powi(2) + (ny - sun_y).powi(2)).sqrt();

        if dist < sun_radius {
            // Horizontal laser cut lines through the sun
            let line_phase = (ny * 22.0).sin();
            if ny > -0.05 && line_phase > 0.4 {
                // Sun slice background
                mood.gradient_at(0.2)
            } else {
                // Sun core gradient (warm neon center)
                let sun_ratio = (ny - (sun_y - sun_radius)) / (sun_radius * 2.0);
                mood.gradient_at(1.0 - sun_ratio.clamp(0.0, 1.0))
            }
        } else {
            // Sky gradient (deep dark atmosphere)
            let sky_ratio = ((ny + 1.0) / (horizon + 1.0)).clamp(0.0, 1.0);
            let glow = (1.0 - (dist - sun_radius).clamp(0.0, 1.0)).powi(2) * 0.4;
            let base = mood.gradient_at(0.05 + sky_ratio * 0.25);
            tint_color(base, glow)
        }
    } else {
        // Perspective Grid / Floor
        let depth = (ny - horizon) / (1.0 - horizon); // 0.0 near horizon, 1.0 at bottom
        let grid_y = (1.0 / (depth + 0.08) + time * 0.6) % 1.0;
        let grid_x = (nx / (depth + 0.15) * 1.5) % 1.0;

        let on_grid = grid_y > 0.85 || grid_x.abs() > 0.85;

        if on_grid {
            mood.gradient_at(0.5 + depth * 0.4)
        } else {
            // Floor reflection
            let base = mood.gradient_at(0.1);
            tint_color(base, (1.0 - depth) * 0.3)
        }
    }
}

fn tint_color(c: Color, amount: f32) -> Color {
    if let Color::Rgb(r, g, b) = c {
        let r = ((r as f32) + (255.0 - r as f32) * amount).clamp(0.0, 255.0) as u8;
        let g = ((g as f32) + (255.0 - g as f32) * amount).clamp(0.0, 255.0) as u8;
        let b = ((b as f32) + (255.0 - b as f32) * amount).clamp(0.0, 255.0) as u8;
        Color::Rgb(r, g, b)
    } else {
        c
    }
}
