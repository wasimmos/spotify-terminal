use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mood {
    Cyberpunk,
    Sunset,
    Matrix,
    Ocean,
    Vaporwave,
    Monochrome,
}

impl Mood {
    pub fn next(&self) -> Self {
        match self {
            Mood::Cyberpunk => Mood::Sunset,
            Mood::Sunset => Mood::Matrix,
            Mood::Matrix => Mood::Ocean,
            Mood::Ocean => Mood::Vaporwave,
            Mood::Vaporwave => Mood::Monochrome,
            Mood::Monochrome => Mood::Cyberpunk,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Mood::Cyberpunk => "Cyberpunk Neon",
            Mood::Sunset => "Sunset Horizon",
            Mood::Matrix => "Matrix Emerald",
            Mood::Ocean => "Deep Ocean",
            Mood::Vaporwave => "Vaporwave Pastel",
            Mood::Monochrome => "Minimalist Monochrome",
        }
    }

    pub fn primary_color(&self) -> Color {
        match self {
            Mood::Cyberpunk => Color::Rgb(255, 60, 180),   // Neon Pink
            Mood::Sunset => Color::Rgb(255, 120, 40),      // Vibrant Orange
            Mood::Matrix => Color::Rgb(50, 255, 120),      // Neon Lime
            Mood::Ocean => Color::Rgb(0, 210, 255),        // Electric Cyan
            Mood::Vaporwave => Color::Rgb(230, 140, 230),  // Pastel Magenta
            Mood::Monochrome => Color::Rgb(230, 230, 230), // Crisp White
        }
    }

    pub fn secondary_color(&self) -> Color {
        match self {
            Mood::Cyberpunk => Color::Rgb(140, 60, 255),  // Purple
            Mood::Sunset => Color::Rgb(255, 60, 70),      // Crimson
            Mood::Matrix => Color::Rgb(20, 180, 80),      // Deep Mint
            Mood::Ocean => Color::Rgb(60, 100, 240),      // Royal Blue
            Mood::Vaporwave => Color::Rgb(120, 180, 240), // Sky Blue
            Mood::Monochrome => Color::Rgb(140, 140, 140),// Silver
        }
    }

    pub fn border_color(&self) -> Color {
        match self {
            Mood::Cyberpunk => Color::Rgb(70, 40, 110),
            Mood::Sunset => Color::Rgb(110, 40, 30),
            Mood::Matrix => Color::Rgb(20, 80, 40),
            Mood::Ocean => Color::Rgb(30, 60, 110),
            Mood::Vaporwave => Color::Rgb(80, 70, 110),
            Mood::Monochrome => Color::Rgb(60, 60, 60),
        }
    }

    /// Interpolates a color along the spectrum from 0.0 (sub-bass) to 1.0 (treble)
    pub fn gradient_at(&self, ratio: f32) -> Color {
        let t = ratio.clamp(0.0, 1.0);
        let (c1, c2, c3) = match self {
            Mood::Cyberpunk => (
                (140, 20, 255),  // Deep Violet
                (255, 40, 160),  // Neon Pink
                (0, 240, 255),   // Electric Cyan
            ),
            Mood::Sunset => (
                (190, 20, 30),   // Crimson
                (255, 110, 20),  // Orange
                (255, 220, 60),  // Golden Yellow
            ),
            Mood::Matrix => (
                (10, 70, 30),    // Dark Moss
                (30, 210, 80),   // Electric Green
                (160, 255, 200), // Mint Phosphor
            ),
            Mood::Ocean => (
                (20, 40, 120),   // Deep Navy
                (0, 150, 240),   // Blue
                (80, 250, 230),  // Aquamarine
            ),
            Mood::Vaporwave => (
                (180, 110, 230), // Lilac
                (255, 150, 190), // Soft Pink
                (120, 240, 220), // Mint
            ),
            Mood::Monochrome => (
                (80, 80, 80),    // Charcoal
                (180, 180, 180), // Silver
                (255, 255, 255), // Bright White
            ),
        };

        let (r, g, b) = if t < 0.5 {
            let local_t = t * 2.0;
            (
                lerp(c1.0 as f32, c2.0 as f32, local_t) as u8,
                lerp(c1.1 as f32, c2.1 as f32, local_t) as u8,
                lerp(c1.2 as f32, c2.2 as f32, local_t) as u8,
            )
        } else {
            let local_t = (t - 0.5) * 2.0;
            (
                lerp(c2.0 as f32, c3.0 as f32, local_t) as u8,
                lerp(c2.1 as f32, c3.1 as f32, local_t) as u8,
                lerp(c2.2 as f32, c3.2 as f32, local_t) as u8,
            )
        };

        Color::Rgb(r, g, b)
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
