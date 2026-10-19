use futures::StreamExt;
use image::DynamicImage;
use rspotify::{
    prelude::*,
    scopes, AuthCodePkceSpotify, Config, Credentials, OAuth,
};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc as tokio_mpsc;

use crate::ui::lyrics::LyricLine;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SpotifyPlaybackState {
    pub track_name: String,
    pub artist_name: String,
    pub album_name: String,
    pub album_art_url: Option<String>,
    pub progress_ms: u64,
    pub duration_ms: u64,
    pub is_playing: bool,
}

#[derive(Debug)]
pub enum SpotifyEvent {
    Connected(String),
    Disconnected(String),
    PlaybackUpdate(SpotifyPlaybackState),
    AlbumArtLoaded(DynamicImage),
    PlaylistsLoaded(Vec<String>),
    LyricsLoaded {
        track_id: String,
        lyrics: Vec<LyricLine>,
    },
}

#[derive(Debug)]
pub enum SpotifyCommand {
    TogglePlay,
    Play,
    Pause,
    NextTrack,
    PreviousTrack,
    Seek(u64),
    FetchPlaylists,
}

#[allow(dead_code)]
pub struct SpotifyManager {
    cmd_sender: tokio_mpsc::Sender<SpotifyCommand>,
}

#[allow(dead_code)]
impl SpotifyManager {
    pub fn spawn(event_sender: Sender<SpotifyEvent>) -> Self {
        let (cmd_tx, mut cmd_rx) = tokio_mpsc::channel::<SpotifyCommand>(32);

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = event_sender.send(SpotifyEvent::Disconnected(format!(
                        "Runtime error: {}",
                        e
                    )));
                    return;
                }
            };

            rt.block_on(async move {
                dotenvy::dotenv().ok();

                let client_id = match std::env::var("RSPOTIFY_CLIENT_ID") {
                    Ok(id) if !id.trim().is_empty() => id.trim().to_string(),
                    _ => {
                        let _ = event_sender.send(SpotifyEvent::Disconnected(
                            "⚠ No Client ID in .env".to_string(),
                        ));
                        while let Some(_cmd) = cmd_rx.recv().await {}
                        return;
                    }
                };

                let creds = Credentials::new_pkce(&client_id);

                let oauth = OAuth {
                    redirect_uri: "http://127.0.0.1:8888/callback".to_string(),
                    scopes: scopes!(
                        "user-read-playback-state",
                        "user-modify-playback-state",
                        "user-read-currently-playing",
                        "playlist-read-private",
                        "playlist-read-collaborative",
                        "user-library-read",
                        "user-read-recently-played"
                    ),
                    ..Default::default()
                };

                let cache_file = PathBuf::from(
                    std::env::var("HOME").unwrap_or_else(|_| "/home/insane".to_string())
                        + "/spotify-terminal/.spotify_token_cache.json",
                );

                let config = Config {
                    token_cached: true,
                    token_refreshing: true,
                    cache_path: cache_file.clone(),
                    ..Default::default()
                };

                let mut spotify = AuthCodePkceSpotify::with_config(creds, oauth, config);

                // --- Authentication ---
                let _ = event_sender.send(SpotifyEvent::Disconnected("🟡 Authenticating…".to_string()));

                let mut is_auth = false;

                // Try cached token first
                if cache_file.exists() {
                    match spotify.read_token_cache(true).await {
                        Ok(Some(tok)) => {
                            *spotify.get_token().lock().await.unwrap() = Some(tok);
                            // Verify it actually works by making a test call
                            match spotify.current_user().await {
                                Ok(_) => {
                                    is_auth = true;
                                    let _ = event_sender.send(SpotifyEvent::Connected("🟢 Connected".to_string()));
                                }
                                Err(e) => {
                                    // Token invalid/expired — need fresh auth
                                    let _ = event_sender.send(SpotifyEvent::Disconnected(
                                        format!("🟡 Token invalid ({}), re-authorizing…", e),
                                    ));
                                    let _ = std::fs::remove_file(&cache_file);
                                    let creds2 = Credentials::new_pkce(&client_id);
                                    let oauth2 = OAuth {
                                        redirect_uri: "http://127.0.0.1:8888/callback".to_string(),
                                        scopes: scopes!(
                                            "user-read-playback-state",
                                            "user-modify-playback-state",
                                            "user-read-currently-playing",
                                            "playlist-read-private",
                                            "playlist-read-collaborative",
                                            "user-library-read",
                                            "user-read-recently-played"
                                        ),
                                        ..Default::default()
                                    };
                                    let config2 = Config {
                                        token_cached: true,
                                        token_refreshing: true,
                                        cache_path: cache_file.clone(),
                                        ..Default::default()
                                    };
                                    spotify = AuthCodePkceSpotify::with_config(creds2, oauth2, config2);
                                }
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {}
                    }
                }

                // Fresh OAuth flow if not auth
                if !is_auth {
                    let listener = match TcpListener::bind("127.0.0.1:8888").await {
                        Ok(l) => l,
                        Err(e) => {
                            let _ = event_sender.send(SpotifyEvent::Disconnected(
                                format!("⚠ Port 8888 busy: {}", e),
                            ));
                            while let Some(_) = cmd_rx.recv().await {}
                            return;
                        }
                    };

                    let auth_url = match spotify.get_authorize_url(None) {
                        Ok(url) => url,
                        Err(e) => {
                            let _ = event_sender.send(SpotifyEvent::Disconnected(format!("OAuth URL error: {}", e)));
                            return;
                        }
                    };

                    let _ = event_sender.send(SpotifyEvent::Disconnected("🟡 Opening browser…".to_string()));
                    let _ = webbrowser::open(&auth_url);

                    match wait_for_auth_code(listener).await {
                        Some(code) => {
                            match spotify.request_token(&code).await {
                                Ok(()) => {
                                    let _ = spotify.write_token_cache().await;
                                    match spotify.current_user().await {
                                        Ok(_) => {
                                            is_auth = true;
                                            let _ = event_sender.send(SpotifyEvent::Connected("🟢 Connected".to_string()));
                                        }
                                        Err(e) => {
                                            let _ = event_sender.send(SpotifyEvent::Disconnected(
                                                format!("⚠ Auth OK but API failed: {}", e),
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    let _ = event_sender.send(SpotifyEvent::Disconnected(
                                        format!("⚠ Token exchange failed: {}", e),
                                    ));
                                }
                            }
                        }
                        None => {
                            let _ = event_sender.send(SpotifyEvent::Disconnected("⚠ No auth code received".to_string()));
                        }
                    }
                }

                if !is_auth {
                    while let Some(_) = cmd_rx.recv().await {}
                    return;
                }

                let mut last_art_url: Option<String> = None;
                let mut last_lyrics_track: Option<String> = None;

                // Fetch playlists immediately
                fetch_and_send_playlists(&spotify, &event_sender).await;

                // Show last played if nothing currently playing
                fetch_recent_or_idle_track(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track).await;

                // Poll every 1.2s for prompt updates
                let mut poll_interval = tokio::time::interval(Duration::from_millis(1200));
                let mut error_count = 0u32;

                loop {
                    tokio::select! {
                        biased;

                        Some(cmd) = cmd_rx.recv() => {
                            match cmd {
                                SpotifyCommand::TogglePlay => {
                                    if let Some(token) = api_get_token(&spotify).await {
                                        let client = reqwest::Client::new();
                                        let is_playing = if let Ok(resp) = client
                                            .get("https://api.spotify.com/v1/me/player")
                                            .bearer_auth(&token)
                                            .send()
                                            .await
                                        {
                                            if let Ok(text) = resp.text().await {
                                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                                    val["is_playing"].as_bool().unwrap_or(false)
                                                } else {
                                                    false
                                                }
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        };

                                        if is_playing {
                                            let _ = send_player_cmd(&client, &token, reqwest::Method::PUT, "pause").await;
                                        } else {
                                            let ok = send_player_cmd(&client, &token, reqwest::Method::PUT, "play").await;
                                            if !ok {
                                                activate_device_and_play(&client, &token).await;
                                            }
                                        }
                                        tokio::time::sleep(Duration::from_millis(250)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count).await;
                                    }
                                }
                                SpotifyCommand::Play => {
                                    if let Some(token) = api_get_token(&spotify).await {
                                        let client = reqwest::Client::new();
                                        let ok = send_player_cmd(&client, &token, reqwest::Method::PUT, "play").await;
                                        if !ok {
                                            activate_device_and_play(&client, &token).await;
                                        }
                                        tokio::time::sleep(Duration::from_millis(250)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count).await;
                                    }
                                }
                                SpotifyCommand::Pause => {
                                    if let Some(token) = api_get_token(&spotify).await {
                                        let client = reqwest::Client::new();
                                        let _ = send_player_cmd(&client, &token, reqwest::Method::PUT, "pause").await;
                                        tokio::time::sleep(Duration::from_millis(250)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count).await;
                                    }
                                }
                                SpotifyCommand::NextTrack => {
                                    if let Some(token) = api_get_token(&spotify).await {
                                        let client = reqwest::Client::new();
                                        let _ = send_player_cmd(&client, &token, reqwest::Method::POST, "next").await;
                                        tokio::time::sleep(Duration::from_millis(300)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count).await;
                                    }
                                }
                                SpotifyCommand::PreviousTrack => {
                                    if let Some(token) = api_get_token(&spotify).await {
                                        let client = reqwest::Client::new();
                                        let _ = send_player_cmd(&client, &token, reqwest::Method::POST, "previous").await;
                                        tokio::time::sleep(Duration::from_millis(300)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count).await;
                                    }
                                }
                                SpotifyCommand::Seek(pos_ms) => {
                                    if let Some(token) = api_get_token(&spotify).await {
                                        let client = reqwest::Client::new();
                                        let url = format!("https://api.spotify.com/v1/me/player/seek?position_ms={}", pos_ms);
                                        let _ = client.put(&url)
                                            .bearer_auth(&token)
                                            .header("Content-Length", "0")
                                            .send()
                                            .await;
                                        tokio::time::sleep(Duration::from_millis(200)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count).await;
                                    }
                                }
                                SpotifyCommand::FetchPlaylists => {
                                    fetch_and_send_playlists(&spotify, &event_sender).await;
                                }
                            }
                        }

                        _ = poll_interval.tick() => {
                            poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count).await;
                        }
                    }
                }
            });
        });

        Self { cmd_sender: cmd_tx }
    }

    pub fn toggle_play(&self) {
        let _ = self.cmd_sender.try_send(SpotifyCommand::TogglePlay);
    }

    pub fn play(&self) {
        let _ = self.cmd_sender.try_send(SpotifyCommand::Play);
    }

    pub fn pause(&self) {
        let _ = self.cmd_sender.try_send(SpotifyCommand::Pause);
    }

    pub fn next_track(&self) {
        let _ = self.cmd_sender.try_send(SpotifyCommand::NextTrack);
    }

    pub fn previous_track(&self) {
        let _ = self.cmd_sender.try_send(SpotifyCommand::PreviousTrack);
    }

    pub fn seek(&self, position_ms: u64) {
        let _ = self.cmd_sender.try_send(SpotifyCommand::Seek(position_ms));
    }

    pub fn fetch_playlists(&self) {
        let _ = self.cmd_sender.try_send(SpotifyCommand::FetchPlaylists);
    }
}

async fn api_get_token(spotify: &AuthCodePkceSpotify) -> Option<String> {
    match spotify.get_token().lock().await {
        Ok(guard) => guard.as_ref().map(|t| t.access_token.clone()),
        Err(_) => None,
    }
}

async fn send_player_cmd(
    client: &reqwest::Client,
    token: &str,
    method: reqwest::Method,
    endpoint: &str,
) -> bool {
    let url = format!("https://api.spotify.com/v1/me/player/{}", endpoint);
    if let Ok(resp) = client
        .request(method, &url)
        .bearer_auth(token)
        .header("Content-Length", "0")
        .send()
        .await
    {
        resp.status().is_success()
    } else {
        false
    }
}

async fn activate_device_and_play(client: &reqwest::Client, token: &str) {
    if let Ok(resp) = client
        .get("https://api.spotify.com/v1/me/player/devices")
        .bearer_auth(token)
        .send()
        .await
    {
        if let Ok(val) = resp.json::<serde_json::Value>().await {
            if let Some(devices) = val["devices"].as_array() {
                if let Some(dev) = devices.first() {
                    if let Some(dev_id) = dev["id"].as_str() {
                        let body = serde_json::json!({
                            "device_ids": [dev_id],
                            "play": true
                        });
                        let _ = client
                            .put("https://api.spotify.com/v1/me/player")
                            .bearer_auth(token)
                            .json(&body)
                            .send()
                            .await;
                    }
                }
            }
        }
    }
}

/// Core polling function using raw HTTP to bypass rspotify's brittle JSON
/// deserialization (which fails on podcasts, local files, etc.)
async fn poll_playback(
    spotify: &AuthCodePkceSpotify,
    event_sender: &Sender<SpotifyEvent>,
    last_art_url: &mut Option<String>,
    last_lyrics_track: &mut Option<String>,
    error_count: &mut u32,
) {
    let access_token = {
        match spotify.get_token().lock().await {
            Ok(guard) => guard.as_ref().map(|t| t.access_token.clone()),
            Err(_) => None,
        }
    };

    let token = match access_token {
        Some(t) => t,
        None => return,
    };

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.spotify.com/v1/me/player")
        .bearer_auth(&token)
        .send()
        .await;

    match resp {
        Ok(r) if r.status().as_u16() == 204 => {
            // 204 = no active playback — keep current display
        }
        Ok(r) if r.status().is_success() => {
            *error_count = 0;
            if let Ok(text) = r.text().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    handle_playback_json(val, event_sender, last_art_url, last_lyrics_track);
                }
            }
        }
        Ok(r) if r.status().as_u16() == 401 => {
            *error_count += 1;
            if *error_count >= 2 {
                let _ = event_sender.send(SpotifyEvent::Disconnected(
                    "🔄 Token expired — restart to re-auth".to_string(),
                ));
            }
        }
        Ok(r) => {
            *error_count += 1;
            if *error_count >= 3 {
                let _ = event_sender.send(SpotifyEvent::Disconnected(
                    format!("⚠ Spotify: HTTP {}", r.status().as_u16()),
                ));
            }
        }
        Err(e) => {
            *error_count += 1;
            if *error_count >= 3 {
                let _ = event_sender.send(SpotifyEvent::Disconnected(
                    format!("⚠ Network: {}", e),
                ));
            }
        }
    }
}

/// Parse raw JSON from GET /me/player — works for tracks, podcasts, local files, anything.
fn handle_playback_json(
    val: serde_json::Value,
    event_sender: &Sender<SpotifyEvent>,
    last_art_url: &mut Option<String>,
    last_lyrics_track: &mut Option<String>,
) {
    let is_playing = val["is_playing"].as_bool().unwrap_or(false);
    let progress_ms = val["progress_ms"].as_u64().unwrap_or(0);

    let item = &val["item"];
    if item.is_null() {
        return;
    }

    let track_name = item["name"].as_str().unwrap_or("Unknown Track").to_string();
    let duration_ms = item["duration_ms"].as_u64().unwrap_or(0);

    // Tracks have artists array; episodes have show.name as publisher
    let artist_name = if let Some(artists) = item["artists"].as_array() {
        artists
            .iter()
            .filter_map(|a| a["name"].as_str())
            .collect::<Vec<_>>()
            .join(", ")
    } else if let Some(show) = item["show"]["name"].as_str() {
        show.to_string()
    } else {
        "—".to_string()
    };

    let album_name = item["album"]["name"]
        .as_str()
        .or_else(|| item["show"]["name"].as_str())
        .unwrap_or("—")
        .to_string();

    // Prefer highest resolution 640x640 image (index 0) for crystal clear HD album cover!
    let art_url: Option<String> = item["album"]["images"]
        .as_array()
        .and_then(|imgs| imgs.first())
        .and_then(|img| img["url"].as_str())
        .or_else(|| {
            item["images"]
                .as_array()
                .and_then(|imgs| imgs.first())
                .and_then(|img| img["url"].as_str())
        })
        .map(|s| s.to_string());

    // Spawn art download if URL changed
    if art_url != *last_art_url {
        *last_art_url = art_url.clone();
        if let Some(ref url) = art_url {
            let url_clone = url.clone();
            let sender_clone = event_sender.clone();
            tokio::spawn(async move {
                if let Ok(resp) = reqwest::get(&url_clone).await {
                    if let Ok(bytes) = resp.bytes().await {
                        let _ = std::fs::write("/home/insane/spotify-terminal/.current_cover.png", &bytes);
                        if let Ok(img) = image::load_from_memory(&bytes) {
                            let _ = sender_clone.send(SpotifyEvent::AlbumArtLoaded(img));
                        }
                    }
                }
            });
        }
    }

    // Spawn lyrics fetch if track changed
    let track_id = format!("{}::{}", track_name, artist_name);
    if last_lyrics_track.as_ref() != Some(&track_id) {
        *last_lyrics_track = Some(track_id.clone());
        let t_clone = track_name.clone();
        let a_clone = artist_name.clone();
        let alb_clone = album_name.clone();
        let dur = duration_ms;
        let sender_clone = event_sender.clone();
        tokio::spawn(async move {
            fetch_synced_lyrics(track_id, t_clone, a_clone, alb_clone, dur, sender_clone).await;
        });
    }

    let _ = event_sender.send(SpotifyEvent::PlaybackUpdate(SpotifyPlaybackState {
        track_name,
        artist_name,
        album_name,
        album_art_url: art_url,
        progress_ms,
        duration_ms,
        is_playing,
    }));
}

async fn fetch_and_send_playlists(spotify: &AuthCodePkceSpotify, event_sender: &Sender<SpotifyEvent>) {
    let mut stream = spotify.current_user_playlists();
    let mut names = Vec::new();
    while let Some(Ok(playlist)) = stream.next().await {
        names.push(playlist.name);
        if names.len() >= 50 {
            break;
        }
    }
    if !names.is_empty() {
        let _ = event_sender.send(SpotifyEvent::PlaylistsLoaded(names));
    }
}

async fn fetch_recent_or_idle_track(
    spotify: &AuthCodePkceSpotify,
    event_sender: &Sender<SpotifyEvent>,
    last_art_url: &mut Option<String>,
    last_lyrics_track: &mut Option<String>,
) {
    let access_token = {
        match spotify.get_token().lock().await {
            Ok(guard) => guard.as_ref().map(|t| t.access_token.clone()),
            Err(_) => None,
        }
    };
    let token = match access_token {
        Some(t) => t,
        None => return,
    };

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.spotify.com/v1/me/player/recently-played?limit=1")
        .bearer_auth(&token)
        .send()
        .await;

    if let Ok(r) = resp {
        if r.status().is_success() {
            if let Ok(text) = r.text().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(first) = val["items"].as_array().and_then(|a| a.first()) {
                        let track = &first["track"];
                        let name = track["name"].as_str().unwrap_or("Unknown").to_string();
                        let artists = track["artists"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x["name"].as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_else(|| "—".to_string());
                        let album = track["album"]["name"].as_str().unwrap_or("—").to_string();
                        let duration_ms = track["duration_ms"].as_u64().unwrap_or(0);
                        let art_url: Option<String> = track["album"]["images"]
                            .as_array()
                            .and_then(|imgs| imgs.get(1).or_else(|| imgs.first()))
                            .and_then(|img| img["url"].as_str())
                            .map(|s| s.to_string());

                        if art_url != *last_art_url {
                            *last_art_url = art_url.clone();
                            if let Some(ref url) = art_url {
                                let url_clone = url.clone();
                                let sender_clone = event_sender.clone();
                                tokio::spawn(async move {
                                    if let Ok(resp) = reqwest::get(&url_clone).await {
                                        if let Ok(bytes) = resp.bytes().await {
                                            let _ = std::fs::write("/home/insane/spotify-terminal/.current_cover.png", &bytes);
                                            if let Ok(img) = image::load_from_memory(&bytes) {
                                                let _ = sender_clone.send(SpotifyEvent::AlbumArtLoaded(img));
                                            }
                                        }
                                    }
                                });
                            }
                        }

                        let track_id = format!("{}::{}", name, artists);
                        if last_lyrics_track.as_ref() != Some(&track_id) {
                            *last_lyrics_track = Some(track_id.clone());
                            let t_clone = name.clone();
                            let a_clone = artists.clone();
                            let alb_clone = album.clone();
                            let dur = duration_ms;
                            let sender_clone = event_sender.clone();
                            tokio::spawn(async move {
                                fetch_synced_lyrics(track_id, t_clone, a_clone, alb_clone, dur, sender_clone).await;
                            });
                        }

                        let _ = event_sender.send(SpotifyEvent::PlaybackUpdate(SpotifyPlaybackState {
                            track_name: name,
                            artist_name: artists,
                            album_name: album,
                            album_art_url: art_url,
                            progress_ms: 0,
                            duration_ms,
                            is_playing: false,
                        }));
                    }
                }
            }
        }
    }
}

/// Fetch real synced lyrics from LRCLIB database (free, no token needed)
async fn fetch_synced_lyrics(
    track_id: String,
    track_name: String,
    artist_name: String,
    album_name: String,
    duration_ms: u64,
    event_sender: Sender<SpotifyEvent>,
) {
    let clean_title = clean_track_title(&track_name);
    let primary_artist = artist_name.split(',').next().unwrap_or(&artist_name).trim();

    let client = match reqwest::Client::builder()
        .user_agent("wasimmos/0.1.0 (https://github.com/wasimmos/spotify-terminal)")
        .timeout(Duration::from_secs(6))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let duration_secs = (duration_ms / 1000).max(1);

    // Attempt 1: Exact match with track_name, primary_artist, album_name, duration
    let url1 = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}&album_name={}&duration={}",
        urlencode(&clean_title),
        urlencode(primary_artist),
        urlencode(&album_name),
        duration_secs
    );

    let mut result_json: Option<serde_json::Value> = None;
    if let Ok(resp) = client.get(&url1).send().await {
        if resp.status().is_success() {
            if let Ok(val) = resp.json::<serde_json::Value>().await {
                if !val["syncedLyrics"].is_null() || !val["plainLyrics"].is_null() {
                    result_json = Some(val);
                }
            }
        }
    }

    // Attempt 2: Match with clean track_name and primary artist only
    if result_json.is_none() {
        let url2 = format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}",
            urlencode(&clean_title),
            urlencode(primary_artist),
        );
        if let Ok(resp) = client.get(&url2).send().await {
            if resp.status().is_success() {
                if let Ok(val) = resp.json::<serde_json::Value>().await {
                    if !val["syncedLyrics"].is_null() || !val["plainLyrics"].is_null() {
                        result_json = Some(val);
                    }
                }
            }
        }
    }

    // Attempt 3: Try full artist name if primary differed
    if result_json.is_none() && primary_artist != artist_name {
        let url3 = format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}",
            urlencode(&clean_title),
            urlencode(&artist_name),
        );
        if let Ok(resp) = client.get(&url3).send().await {
            if resp.status().is_success() {
                if let Ok(val) = resp.json::<serde_json::Value>().await {
                    if !val["syncedLyrics"].is_null() || !val["plainLyrics"].is_null() {
                        result_json = Some(val);
                    }
                }
            }
        }
    }

    let lyrics = if let Some(val) = result_json {
        if let Some(synced) = val["syncedLyrics"].as_str() {
            parse_lrc(synced)
        } else if let Some(plain) = val["plainLyrics"].as_str() {
            parse_plain_lyrics(plain, duration_ms)
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let _ = event_sender.send(SpotifyEvent::LyricsLoaded { track_id, lyrics });
}

fn clean_track_title(title: &str) -> String {
    let mut t = title.to_string();
    if let Some(pos) = t.find("(feat.") {
        t.truncate(pos);
    }
    if let Some(pos) = t.find("(Feat.") {
        t.truncate(pos);
    }
    if let Some(pos) = t.find("[feat.") {
        t.truncate(pos);
    }
    if let Some(pos) = t.find("[Feat.") {
        t.truncate(pos);
    }
    let patterns = [
        " - Remastered",
        " - Remaster",
        " - Single Version",
        " - Radio Edit",
        " - Bonus Track",
        " - Deluxe",
        " - Live",
    ];
    for pat in patterns {
        if let Some(pos) = t.find(pat) {
            t.truncate(pos);
        }
    }
    t.trim().to_string()
}

fn urlencode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.starts_with('[') {
            if let Some(close_idx) = trimmed.find(']') {
                let time_part = &trimmed[1..close_idx];
                let lyric = trimmed[close_idx + 1..].trim();
                if let Some(colon) = time_part.find(':') {
                    if let (Ok(mins), Ok(secs)) = (
                        time_part[..colon].parse::<u64>(),
                        time_part[colon + 1..].parse::<f32>(),
                    ) {
                        let time_ms = mins * 60_000 + (secs * 1000.0) as u64;
                        if !lyric.is_empty() {
                            lines.push(LyricLine {
                                time_ms,
                                text: lyric.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
    lines.sort_by_key(|l| l.time_ms);
    lines
}

fn parse_plain_lyrics(text: &str, duration_ms: u64) -> Vec<LyricLine> {
    let raw_lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if raw_lines.is_empty() {
        return vec![];
    }
    let total_lines = raw_lines.len();
    let step_ms = if total_lines > 0 {
        (duration_ms.saturating_sub(10_000)) / total_lines as u64
    } else {
        4000
    };
    let step_ms = step_ms.clamp(2000, 8000);

    raw_lines
        .into_iter()
        .enumerate()
        .map(|(i, t)| LyricLine {
            time_ms: 3000 + (i as u64 * step_ms),
            text: t.to_string(),
        })
        .collect()
}

async fn wait_for_auth_code(listener: TcpListener) -> Option<String> {
    if let Ok((mut socket, _)) = listener.accept().await {
        let mut buf = [0u8; 4096];
        if let Ok(n) = socket.read(&mut buf).await {
            let request = String::from_utf8_lossy(&buf[..n]);

            if let Some(pos) = request.find("code=") {
                let after = &request[pos + 5..];
                let end = after.find('&').or_else(|| after.find(' ')).unwrap_or(after.len());
                let code = after[..end].to_string();

                let html = "<!DOCTYPE html><html><body style='font-family:system-ui,sans-serif;text-align:center;padding:60px;background:#100e18;color:#eee'>\
                    <h1 style='color:#32ff78;font-size:2rem'>✦ Spotify Connected!</h1>\
                    <p style='color:#aaa;font-size:1.1rem'>You can now close this tab and return to <b>wasimmos</b>.</p>\
                    </body></html>";

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                    html.len(),
                    html
                );
                let _ = socket.write_all(response.as_bytes()).await;
                return Some(code);
            }
        }
    }
    None
}
