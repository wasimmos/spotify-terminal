use image::DynamicImage;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::audio::{AudioEngine, VisualizerAnalyzer};
use crate::spotify::{SpotifyEvent, SpotifyManager};
use crate::ui::details::TrackDetails;
use crate::ui::lyrics::LyricLine;
use crate::ui::ThemeMode;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CardViewMode {
    Lyrics,
    SongDetails,
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

    pub album_cover: Option<DynamicImage>,
    pub theme: ThemeMode,
    pub view_mode: CardViewMode,

    // Spotify Integration
    pub spotify: SpotifyManager,
    pub spotify_rx: Receiver<SpotifyEvent>,
    pub spotify_status: String,

    // Collapsible Playlists & Search
    pub show_playlists: bool,
    pub is_searching: bool,
    pub search_query: String,
    pub playlists: Vec<String>,
    pub selected_playlist: usize,

    pub lyrics: Vec<LyricLine>,
    pub track_details: TrackDetails,
    pub visualizer: VisualizerAnalyzer,
    pub audio_engine: AudioEngine,
    pub audio_buffer: Vec<f32>,

    pub anim_time: f32,
    pub should_quit: bool,
    last_tick: Instant,

    // Track change detection
    last_track_id: String,
}

impl App {
    pub fn new() -> Self {
        let audio_engine = AudioEngine::new();
        let visualizer = VisualizerAnalyzer::new(32, 44100);

        // Spotify background manager channel
        let (spotify_tx, spotify_rx) = channel::<SpotifyEvent>();
        let spotify = SpotifyManager::spawn(spotify_tx);

        // Start with NO album cover — procedural synthwave fallback renders until real art arrives
        let album_cover: Option<DynamicImage> = None;

        let playlists = vec![
            "Bollywood Soul & Ghazals".to_string(),
            "Late Night Acoustic & Lo-Fi".to_string(),
            "Cyberpunk 2077 Synthwave".to_string(),
            "Top 50 Global Favorites".to_string(),
            "Coding Flow & Deep Focus".to_string(),
            "♥ Liked Songs".to_string(),
        ];

        let lyrics = vec![];

        let track_details = TrackDetails {
            album: "—".to_string(),
            artist: "—".to_string(),
            release_year: "—".to_string(),
            genre: "—".to_string(),
            format: "Spotify Web Stream".to_string(),
            bitrate: "Ogg Vorbis 320 kbps".to_string(),
            tempo: "—".to_string(),
            key_signature: "—".to_string(),
            up_next: vec![],
        };

        Self {
            current_track: "No track playing".to_string(),
            current_artist: "Connect to Spotify to start".to_string(),
            current_album: "—".to_string(),
            duration: Duration::from_secs(0),
            is_playing: false,
            volume: 0.85,
            playback_time_ms: 0,

            album_cover,
            theme: ThemeMode::DiscoIce,
            view_mode: CardViewMode::Lyrics,

            spotify,
            spotify_rx,
            spotify_status: "Connecting…".to_string(),

            show_playlists: false,
            is_searching: false,
            search_query: String::new(),
            playlists,
            selected_playlist: 0,

            lyrics,
            track_details,
            visualizer,
            audio_engine,
            audio_buffer: vec![0.0; 1024],

            anim_time: 0.0,
            should_quit: false,
            last_tick: Instant::now(),

            last_track_id: String::new(),
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

        // Drain ALL pending Spotify events this tick
        while let Ok(event) = self.spotify_rx.try_recv() {
            match event {
                SpotifyEvent::Connected(msg) => {
                    self.spotify_status = msg;
                    self.spotify.fetch_playlists();
                }
                SpotifyEvent::Disconnected(msg) => {
                    self.spotify_status = msg;
                    self.is_playing = false;
                }
                SpotifyEvent::PlaybackUpdate(state) => {
                    let new_track_id = format!("{}::{}", state.track_name, state.artist_name);
                    if new_track_id != self.last_track_id {
                        self.last_track_id = new_track_id;

                        // Reset stale art and lyrics while fresh data fetches
                        self.album_cover = None;
                        self.lyrics = vec![];

                        self.track_details.release_year = "—".to_string();
                        self.track_details.genre = "—".to_string();
                        self.track_details.tempo = "—".to_string();
                        self.track_details.key_signature = "—".to_string();
                        self.track_details.up_next = vec![];
                    }

                    // Update live playback state
                    self.current_track = state.track_name;
                    self.current_artist = state.artist_name;
                    self.current_album = state.album_name.clone();

                    // Update details panel
                    self.track_details.album = state.album_name;
                    self.track_details.artist = self.current_artist.clone();
                    self.track_details.format = "Spotify Web Stream".to_string();
                    self.track_details.bitrate = "Ogg Vorbis 320 kbps".to_string();

                    if state.is_playing || self.playback_time_ms == 0 {
                        self.playback_time_ms = state.progress_ms;
                    }
                    self.duration = Duration::from_millis(state.duration_ms);
                    self.is_playing = state.is_playing;
                }
                SpotifyEvent::AlbumArtLoaded(img) => {
                    self.album_cover = Some(img);
                }
                SpotifyEvent::PlaylistsLoaded(playlists) => {
                    if !playlists.is_empty() {
                        self.playlists = playlists;
                    }
                }
                SpotifyEvent::LyricsLoaded { track_id, lyrics } => {
                    if self.last_track_id == track_id {
                        self.lyrics = lyrics;
                    }
                }
            }
        }

        // Advance local playback clock between Spotify polls
        if self.is_playing {
            self.playback_time_ms += (dt * 1000.0) as u64;
            if self.duration.as_millis() > 0
                && self.playback_time_ms > self.duration.as_millis() as u64
            {
                self.playback_time_ms = self.duration.as_millis() as u64;
            }
        }

        // Real audio visualizer analysis (captured live from default speaker output)
        let mut new_samples = Vec::new();
        self.audio_engine.read_samples(&mut new_samples);

        if !new_samples.is_empty() {
            self.audio_buffer.extend(new_samples);
            if self.audio_buffer.len() > 1024 {
                let overflow = self.audio_buffer.len() - 1024;
                self.audio_buffer.drain(0..overflow);
            }
            self.visualizer.process_samples(&self.audio_buffer);
        } else {
            // Decay bars smoothly towards zero when silent/paused
            self.visualizer.decay_bands();
        }
    }

    pub fn toggle_play(&mut self) {
        self.is_playing = !self.is_playing;
        self.spotify.toggle_play();
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.playback_time_ms = 0;
        self.spotify.toggle_play();
    }

    pub fn next_track(&mut self) {
        self.playback_time_ms = 0;
        self.last_track_id = String::new();
        self.spotify.next_track();
    }

    pub fn previous_track(&mut self) {
        self.playback_time_ms = 0;
        self.last_track_id = String::new();
        self.spotify.previous_track();
    }

    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            CardViewMode::Lyrics => CardViewMode::SongDetails,
            CardViewMode::SongDetails => CardViewMode::Lyrics,
        };
    }

    pub fn next_theme(&mut self) {
        self.theme = self.theme.next();
    }

    pub fn toggle_playlists(&mut self) {
        self.show_playlists = !self.show_playlists;
        if self.show_playlists {
            self.spotify.fetch_playlists();
        } else {
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
