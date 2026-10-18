use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ThemeMode {
    DiscoIce,       // Matching the user's reference screenshot!
    Cyberpunk,      // Neon pink, purple & cyan
    TokyoSunset,    // Crimson, orange & warm amber
    MatrixEmerald,  // Phosphor green & neon mint
    Vaporwave,      // Pastel lilac & cotton candy
    NordicNight,    // Deep navy & arctic teal
    Monochrome,     // Crisp minimal white & slate
}

#[allow(dead_code)]
impl ThemeMode {
    pub fn next(&self) -> Self {
        match self {
            ThemeMode::DiscoIce => ThemeMode::Cyberpunk,
            ThemeMode::Cyberpunk => ThemeMode::TokyoSunset,
            ThemeMode::TokyoSunset => ThemeMode::MatrixEmerald,
            ThemeMode::MatrixEmerald => ThemeMode::Vaporwave,
            ThemeMode::Vaporwave => ThemeMode::NordicNight,
            ThemeMode::NordicNight => ThemeMode::Monochrome,
            ThemeMode::Monochrome => ThemeMode::DiscoIce,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ThemeMode::DiscoIce => "Disco Ice (Classic)",
            ThemeMode::Cyberpunk => "Cyberpunk Neon",
            ThemeMode::TokyoSunset => "Tokyo Sunset",
            ThemeMode::MatrixEmerald => "Matrix Emerald",
            ThemeMode::Vaporwave => "Vaporwave Pastel",
            ThemeMode::NordicNight => "Nordic Frost",
            ThemeMode::Monochrome => "Minimalist Slate",
        }
    }

    pub fn card_border(&self) -> Color {
        match self {
            ThemeMode::DiscoIce => Color::Rgb(110, 48, 155),    // Deep violet border from screenshot
            ThemeMode::Cyberpunk => Color::Rgb(150, 30, 180),
            ThemeMode::TokyoSunset => Color::Rgb(180, 50, 40),
            ThemeMode::MatrixEmerald => Color::Rgb(25, 120, 50),
            ThemeMode::Vaporwave => Color::Rgb(140, 100, 180),
            ThemeMode::NordicNight => Color::Rgb(40, 80, 140),
            ThemeMode::Monochrome => Color::Rgb(70, 70, 80),
        }
    }

    pub fn title_color(&self) -> Color {
        match self {
            ThemeMode::DiscoIce => Color::Rgb(255, 195, 45),    // Golden amber title from screenshot
            ThemeMode::Cyberpunk => Color::Rgb(255, 50, 180),   // Neon pink
            ThemeMode::TokyoSunset => Color::Rgb(255, 140, 40), // Orange
            ThemeMode::MatrixEmerald => Color::Rgb(60, 255, 130),
            ThemeMode::Vaporwave => Color::Rgb(240, 150, 240),
            ThemeMode::NordicNight => Color::Rgb(0, 220, 255),
            ThemeMode::Monochrome => Color::Rgb(240, 240, 240),
        }
    }

    pub fn active_lyric(&self) -> Color {
        match self {
            ThemeMode::DiscoIce => Color::Rgb(255, 240, 200),   // Warm bright cream
            ThemeMode::Cyberpunk => Color::Rgb(255, 230, 255),
            ThemeMode::TokyoSunset => Color::Rgb(255, 235, 190),
            ThemeMode::MatrixEmerald => Color::Rgb(220, 255, 230),
            ThemeMode::Vaporwave => Color::Rgb(255, 240, 255),
            ThemeMode::NordicNight => Color::Rgb(220, 250, 255),
            ThemeMode::Monochrome => Color::Rgb(255, 255, 255),
        }
    }

    pub fn progress_bar(&self) -> Color {
        match self {
            ThemeMode::DiscoIce => Color::Rgb(240, 45, 130),    // Vibrant magenta elapsed bar
            ThemeMode::Cyberpunk => Color::Rgb(0, 240, 255),
            ThemeMode::TokyoSunset => Color::Rgb(255, 90, 30),
            ThemeMode::MatrixEmerald => Color::Rgb(40, 230, 100),
            ThemeMode::Vaporwave => Color::Rgb(220, 130, 220),
            ThemeMode::NordicNight => Color::Rgb(30, 180, 255),
            ThemeMode::Monochrome => Color::Rgb(200, 200, 200),
        }
    }

    pub fn key_accent(&self) -> Color {
        match self {
            ThemeMode::DiscoIce => Color::Rgb(255, 195, 45),
            ThemeMode::Cyberpunk => Color::Rgb(255, 60, 190),
            ThemeMode::TokyoSunset => Color::Rgb(255, 150, 50),
            ThemeMode::MatrixEmerald => Color::Rgb(50, 255, 120),
            ThemeMode::Vaporwave => Color::Rgb(230, 150, 230),
            ThemeMode::NordicNight => Color::Rgb(50, 210, 255),
            ThemeMode::Monochrome => Color::Rgb(255, 255, 255),
        }
    }

    /// Color for the discrete visualizer bars
    pub fn bar_color_at(&self, ratio: f32) -> Color {
        let t = ratio.clamp(0.0, 1.0);
        match self {
            // Ice blue / soft lavender matching reference image
            ThemeMode::DiscoIce => {
                let r = lerp(190.0, 215.0, t) as u8;
                let g = lerp(205.0, 225.0, t) as u8;
                let b = lerp(240.0, 255.0, t) as u8;
                Color::Rgb(r, g, b)
            }
            ThemeMode::Cyberpunk => {
                // Violet to hot pink to neon cyan
                if t < 0.5 {
                    let local = t * 2.0;
                    Color::Rgb(
                        lerp(140.0, 255.0, local) as u8,
                        lerp(30.0, 50.0, local) as u8,
                        lerp(240.0, 180.0, local) as u8,
                    )
                } else {
                    let local = (t - 0.5) * 2.0;
                    Color::Rgb(
                        lerp(255.0, 0.0, local) as u8,
                        lerp(50.0, 240.0, local) as u8,
                        lerp(180.0, 255.0, local) as u8,
                    )
                }
            }
            ThemeMode::TokyoSunset => {
                Color::Rgb(
                    lerp(220.0, 255.0, t) as u8,
                    lerp(40.0, 200.0, t) as u8,
                    lerp(30.0, 50.0, t) as u8,
                )
            }
            ThemeMode::MatrixEmerald => {
                Color::Rgb(
                    lerp(20.0, 100.0, t) as u8,
                    lerp(180.0, 255.0, t) as u8,
                    lerp(70.0, 180.0, t) as u8,
                )
            }
            ThemeMode::Vaporwave => {
                Color::Rgb(
                    lerp(170.0, 255.0, t) as u8,
                    lerp(130.0, 190.0, t) as u8,
                    lerp(240.0, 230.0, t) as u8,
                )
            }
            ThemeMode::NordicNight => {
                Color::Rgb(
                    lerp(30.0, 120.0, t) as u8,
                    lerp(120.0, 240.0, t) as u8,
                    lerp(220.0, 255.0, t) as u8,
                )
            }
            ThemeMode::Monochrome => {
                let v = lerp(120.0, 245.0, t) as u8;
                Color::Rgb(v, v, v)
            }
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
