use std::time::{Duration, Instant};

use crate::audio::{AudioEngine, VisualizerAnalyzer};
use crate::ui::lyrics::LyricLine;
use crate::ui::Mood;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum FocusedSection {
    Search,
    Playlists,
}

#[allow(dead_code)]
pub struct App {
    pub current_track: String,
    pub current_artist: String,
    pub current_album: String,
    pub duration: Duration,
    pub is_playing: bool,
    pub volume: f32,
    pub playback_time_ms: u64,

    pub mood: Mood,
    pub focused: FocusedSection,
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
        let visualizer = VisualizerAnalyzer::new(64, 44100);

        let playlists = vec![
            "Cyberpunk Drive 2077".to_string(),
            "Lo-Fi Coding Beats".to_string(),
            "Synthwave Retrowave Vibes".to_string(),
            "Late Night Tokyo Jazz".to_string(),
            "Deep Focus Techno".to_string(),
            "♥ Liked Songs (342)".to_string(),
        ];

        // Demo synced lyrics
        let lyrics = vec![
            LyricLine { time_ms: 0, text: "♪ (Synth Intro) ♪".to_string() },
            LyricLine { time_ms: 8_000, text: "Waiting in a car".to_string() },
            LyricLine { time_ms: 12_500, text: "Waiting for a ride in the dark".to_string() },
            LyricLine { time_ms: 18_000, text: "The night city grows".to_string() },
            LyricLine { time_ms: 22_500, text: "Look and see her eyes, they glow".to_string() },
            LyricLine { time_ms: 28_000, text: "Waiting in a car".to_string() },
            LyricLine { time_ms: 32_000, text: "Waiting for a ride in the dark".to_string() },
            LyricLine { time_ms: 36_000, text: "The city is my church".to_string() },
            LyricLine { time_ms: 41_000, text: "It wraps me in the blinding twilight".to_string() },
            LyricLine { time_ms: 48_000, text: "Waiting in a car...".to_string() },
            LyricLine { time_ms: 54_000, text: "The sky is full of electric dreams".to_string() },
            LyricLine { time_ms: 60_000, text: "♪ (Saxophone & Synth Drop) ♪".to_string() },
        ];

        Self {
            current_track: "Midnight City".to_string(),
            current_artist: "M83".to_string(),
            current_album: "Hurry Up, We're Dreaming".to_string(),
            duration: Duration::from_secs(243),
            is_playing: true,
            volume: 0.85,
            playback_time_ms: 18_500,

            mood: Mood::Cyberpunk,
            focused: FocusedSection::Playlists,
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

    pub fn cycle_mood(&mut self) {
        self.mood = self.mood.next();
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
