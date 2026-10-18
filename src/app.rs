use image::DynamicImage;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::audio::{AudioEngine, VisualizerAnalyzer};
use crate::ui::lyrics::LyricLine;
use crate::ui::ThemeMode;

#[allow(dead_code)]
pub struct App {
    pub current_track: String,
    pub current_artist: String,
    pub current_album: String,
    pub duration: Duration,
    pub is_playing: bool,
    pub volume: f32,
    pub playback_time_ms: u64,

    pub album_cover: Option<DynamicImage>,
    pub theme: ThemeMode,

    // Collapsible Playlists & Search
    pub show_playlists: bool,
    pub is_searching: bool,
    pub search_query: String,
    pub playlists: Vec<String>,
    pub selected_playlist: usize,

    pub lyrics: Vec<LyricLine>,
    pub visualizer: VisualizerAnalyzer,
    pub audio_engine: AudioEngine,

    pub anim_time: f32,
    pub should_quit: bool,
    last_tick: Instant,
}

impl App {
    pub fn new() -> Self {
        let (audio_engine, _) = AudioEngine::new();
        let visualizer = VisualizerAnalyzer::new(32, 44100);

        // Load reference album art if available
        let cover_path = Path::new("/home/insane/spotify-terminal/example/cover.png");
        let album_cover = if cover_path.exists() {
            image::open(cover_path).ok()
        } else {
            None
        };

        let playlists = vec![
            "Bollywood Soul & Ghazals".to_string(),
            "Late Night Acoustic & Lo-Fi".to_string(),
            "Cyberpunk 2077 Synthwave".to_string(),
            "Top 50 Global Favorites".to_string(),
            "Coding Flow & Deep Focus".to_string(),
            "♥ Liked Songs (420)".to_string(),
        ];

        // Authentic synced lyrics from the reference image
        let lyrics = vec![
            LyricLine { time_ms: 0, text: "मौला मेरे, मौला मेरे, मौला मेरे, मौला मेरे".to_string() },
            LyricLine { time_ms: 12_000, text: "मौला मेरे, मौला मेरे, मौला मेरे, मौला मेरे".to_string() },
            LyricLine { time_ms: 24_000, text: "मौला मेरे, मौला मेरे, मौला मेरे, मौला मेरे".to_string() },
            LyricLine { time_ms: 36_000, text: "मौला मेरे, मौला मेरे, मौला मेरे, मौला मेरे".to_string() },
            LyricLine { time_ms: 42_000, text: "आँखें तेरी ...".to_string() },
            LyricLine { time_ms: 48_000, text: "आँखें तेरी कितनी हसीं".to_string() },
            LyricLine { time_ms: 55_000, text: "के इनका आशिक़ मैं बन गया हूँ".to_string() },
            LyricLine { time_ms: 63_000, text: "मुझको बसा ले इनमें तू".to_string() },
            LyricLine { time_ms: 72_000, text: "मौला मेरे, मौला मेरे, मौला मेरे, मौला मेरे (इश्क़ है)".to_string() },
            LyricLine { time_ms: 84_000, text: "मौला मेरे, मौला मेरे, मौला मेरे, मौला मेरे (इश्क़ है)".to_string() },
            LyricLine { time_ms: 96_000, text: "मौला मेरे, मौला मेरे, मौला मेरे, मौला मेरे".to_string() },
        ];

        Self {
            current_track: "Maula Mere Maula".to_string(),
            current_artist: "Roop Kumar Rathod • Anwar".to_string(),
            current_album: "Anwar (2007)".to_string(),
            duration: Duration::from_secs(364), // 6:04
            is_playing: true,
            volume: 0.85,
            playback_time_ms: 42_000, // 0:42 matching screenshot

            album_cover,
            theme: ThemeMode::DiscoIce,

            show_playlists: false,
            is_searching: false,
            search_query: String::new(),
            playlists,
            selected_playlist: 0,

            lyrics,
            visualizer,
            audio_engine,

            anim_time: 0.0,
            should_quit: false,
            last_tick: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        Duration::from_millis(self.playback_time_ms)
    }

    pub fn current_time_str(&self) -> String {
        let total_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Local time offset approximation (India Standard Time UTC+5:30)
        let local_secs = total_secs + (5 * 3600 + 30 * 60);
        let hours = (local_secs / 3600) % 24;
        let mins = (local_secs / 60) % 60;
        let secs = local_secs % 60;

        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }

    pub fn on_tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        self.anim_time += dt;

        if self.is_playing {
            self.playback_time_ms += (dt * 1000.0) as u64;
            if self.playback_time_ms > self.duration.as_millis() as u64 {
                self.playback_time_ms = 0;
            }

            // Real-time audio spectrum animation
            self.visualizer.update_procedural(self.anim_time);
        }
    }

    pub fn toggle_play(&mut self) {
        self.is_playing = !self.is_playing;
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.playback_time_ms = 0;
    }

    pub fn next_theme(&mut self) {
        self.theme = self.theme.next();
    }

    pub fn toggle_playlists(&mut self) {
        self.show_playlists = !self.show_playlists;
        if !self.show_playlists {
            self.is_searching = false;
        }
    }

    pub fn next_playlist(&mut self) {
        if !self.playlists.is_empty() {
            self.selected_playlist = (self.selected_playlist + 1) % self.playlists.len();
        }
    }

    pub fn prev_playlist(&mut self) {
        if !self.playlists.is_empty() {
            if self.selected_playlist == 0 {
                self.selected_playlist = self.playlists.len() - 1;
            } else {
                self.selected_playlist -= 1;
            }
        }
    }

    pub fn volume_up(&mut self) {
        self.volume = (self.volume + 0.05).min(1.0);
    }

    pub fn volume_down(&mut self) {
        self.volume = (self.volume - 0.05).max(0.0);
    }
}
