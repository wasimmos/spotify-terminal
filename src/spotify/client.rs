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
                let home = std::env::var("HOME").unwrap_or_else(|_| "/home/insane".to_string());

                // Search standard config directory first, then repository folder, then current working directory
                dotenvy::dotenv().ok();
                dotenvy::from_path(format!("{}/.config/spotif/.env", home)).ok();
                dotenvy::from_path(format!("{}/spotify-terminal/.env", home)).ok();

                let client_id = match std::env::var("RSPOTIFY_CLIENT_ID")
                    .or_else(|_| std::env::var("SPOTIFY_CLIENT_ID"))
                {
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

                let config_cache = PathBuf::from(format!("{}/.config/spotif/token_cache.json", home));
                let legacy_cache = PathBuf::from(format!("{}/spotify-terminal/.spotify_token_cache.json", home));
                let cache_file = if config_cache.exists() {
                    config_cache
                } else if legacy_cache.exists() {
                    legacy_cache
                } else {
                    config_cache
                };

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

                // 1. Try cached token first (with robust auto-refresh so user never re-authenticates)
                if cache_file.exists() {
                    let mut tok_opt = spotify.read_token_cache(false).await.ok().flatten();
                    if tok_opt.is_none() {
                        if let Ok(raw_json) = std::fs::read_to_string(&cache_file) {
                            tok_opt = serde_json::from_str::<rspotify::model::Token>(&raw_json).ok();
                        }
                    }

                    if let Some(tok) = tok_opt {
                        *spotify.get_token().lock().await.unwrap() = Some(tok.clone());

                        // Check if token is expired or within 60s of expiring
                        let is_expired = tok.expires_at.map(|exp| exp <= chrono::Utc::now() + chrono::Duration::seconds(60)).unwrap_or(false);

                        if is_expired {
                            let _ = event_sender.send(SpotifyEvent::Disconnected("🔄 Refreshing session…".to_string()));
                            let mut refreshed = false;
                            if let Some(ref r_tok) = tok.refresh_token {
                                if let Some(new_tok) = direct_refresh_token(&client_id, r_tok, &cache_file).await {
                                    *spotify.get_token().lock().await.unwrap() = Some(new_tok);
                                    refreshed = true;
                                }
                            }
                            if !refreshed {
                                if let Ok(()) = spotify.refresh_token().await {
                                    let _ = spotify.write_token_cache().await;
                                    refreshed = true;
                                }
                            }

                            if refreshed {
                                is_auth = true;
                                let _ = event_sender.send(SpotifyEvent::Connected("🟢 Connected".to_string()));
                            }
                        } else {
                            // Token is currently not expired, verify it
                            match spotify.current_user().await {
                                Ok(_) => {
                                    is_auth = true;
                                    let _ = event_sender.send(SpotifyEvent::Connected("🟢 Connected".to_string()));
                                }
                                Err(_) => {
                                    // Try refreshing with refresh_token before prompting OAuth!
                                    let mut refreshed = false;
                                    if let Some(ref r_tok) = tok.refresh_token {
                                        if let Some(new_tok) = direct_refresh_token(&client_id, r_tok, &cache_file).await {
                                            *spotify.get_token().lock().await.unwrap() = Some(new_tok);
                                            refreshed = true;
                                        }
                                    }
                                    if !refreshed {
                                        if let Ok(()) = spotify.refresh_token().await {
                                            let _ = spotify.write_token_cache().await;
                                            refreshed = true;
                                        }
                                    }

                                    if refreshed {
                                        is_auth = true;
                                        let _ = event_sender.send(SpotifyEvent::Connected("🟢 Connected".to_string()));
                                    }
                                }
                            }
                        }
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
                                    if let Some(token) = api_get_token(&spotify, &client_id, &cache_file).await {
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
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count, &client_id, &cache_file).await;
                                    }
                                }
                                SpotifyCommand::Play => {
                                    if let Some(token) = api_get_token(&spotify, &client_id, &cache_file).await {
                                        let client = reqwest::Client::new();
                                        let ok = send_player_cmd(&client, &token, reqwest::Method::PUT, "play").await;
                                        if !ok {
                                            activate_device_and_play(&client, &token).await;
                                        }
                                        tokio::time::sleep(Duration::from_millis(250)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count, &client_id, &cache_file).await;
                                    }
                                }
                                SpotifyCommand::Pause => {
                                    if let Some(token) = api_get_token(&spotify, &client_id, &cache_file).await {
                                        let client = reqwest::Client::new();
                                        let _ = send_player_cmd(&client, &token, reqwest::Method::PUT, "pause").await;
                                        tokio::time::sleep(Duration::from_millis(250)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count, &client_id, &cache_file).await;
                                    }
                                }
                                SpotifyCommand::NextTrack => {
                                    if let Some(token) = api_get_token(&spotify, &client_id, &cache_file).await {
                                        let client = reqwest::Client::new();
                                        let _ = send_player_cmd(&client, &token, reqwest::Method::POST, "next").await;
                                        tokio::time::sleep(Duration::from_millis(300)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count, &client_id, &cache_file).await;
                                    }
                                }
                                SpotifyCommand::PreviousTrack => {
                                    if let Some(token) = api_get_token(&spotify, &client_id, &cache_file).await {
                                        let client = reqwest::Client::new();
                                        let _ = send_player_cmd(&client, &token, reqwest::Method::POST, "previous").await;
                                        tokio::time::sleep(Duration::from_millis(300)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count, &client_id, &cache_file).await;
                                    }
                                }
                                SpotifyCommand::Seek(pos_ms) => {
                                    if let Some(token) = api_get_token(&spotify, &client_id, &cache_file).await {
                                        let client = reqwest::Client::new();
                                        let url = format!("https://api.spotify.com/v1/me/player/seek?position_ms={}", pos_ms);
                                        let _ = client.put(&url)
                                            .bearer_auth(&token)
                                            .header("Content-Length", "0")
                                            .send()
                                            .await;
                                        tokio::time::sleep(Duration::from_millis(200)).await;
                                        poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count, &client_id, &cache_file).await;
                                    }
                                }
                                SpotifyCommand::FetchPlaylists => {
                                    fetch_and_send_playlists(&spotify, &event_sender).await;
                                }
                            }
                        }

                        _ = poll_interval.tick() => {
                            poll_playback(&spotify, &event_sender, &mut last_art_url, &mut last_lyrics_track, &mut error_count, &client_id, &cache_file).await;
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

async fn direct_refresh_token(
    client_id: &str,
    refresh_token: &str,
    cache_path: &PathBuf,
) -> Option<rspotify::model::Token> {
    let client = reqwest::Client::new();
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];
    let resp = client
        .post("https://accounts.spotify.com/api/token")
        .form(&params)
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let val = resp.json::<serde_json::Value>().await.ok()?;
    let access_token = val["access_token"].as_str()?.to_string();
    let expires_in_sec = val["expires_in"].as_u64().unwrap_or(3600);
    let new_refresh = val["refresh_token"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| refresh_token.to_string());

    let mut cache_json: serde_json::Value = std::fs::read_to_string(cache_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    cache_json["access_token"] = serde_json::Value::String(access_token);
    cache_json["expires_in"] = serde_json::json!(expires_in_sec);
    cache_json["refresh_token"] = serde_json::Value::String(new_refresh);
    let now = chrono::Utc::now();
    let exp_at = now + chrono::Duration::seconds(expires_in_sec as i64);
    cache_json["expires_at"] = serde_json::json!(exp_at.to_rfc3339());

    if let Ok(serialized) = serde_json::to_string_pretty(&cache_json) {
        let _ = std::fs::write(cache_path, serialized);
    }

    serde_json::from_value(cache_json).ok()
}

async fn api_get_token(
    spotify: &AuthCodePkceSpotify,
    client_id: &str,
    cache_path: &PathBuf,
) -> Option<String> {
    let mut needs_refresh = false;
    let mut refresh_tok_str = None;
    {
        if let Ok(guard) = spotify.get_token().lock().await {
            if let Some(ref t) = *guard {
                if t.expires_at.map(|exp| exp <= chrono::Utc::now() + chrono::Duration::seconds(30)).unwrap_or(false) {
                    needs_refresh = true;
                    refresh_tok_str = t.refresh_token.clone();
                } else {
                    return Some(t.access_token.clone());
                }
            }
        }
    }

    if needs_refresh {
        if let Some(r_tok) = refresh_tok_str {
            if let Some(new_tok) = direct_refresh_token(client_id, &r_tok, cache_path).await {
                let access = new_tok.access_token.clone();
                if let Ok(mut guard) = spotify.get_token().lock().await {
                    *guard = Some(new_tok);
                }
                return Some(access);
            }
        }
        if let Ok(()) = spotify.refresh_token().await {
            let _ = spotify.write_token_cache().await;
            if let Ok(guard) = spotify.get_token().lock().await {
                return guard.as_ref().map(|t| t.access_token.clone());
            }
        }
    }

    if let Ok(guard) = spotify.get_token().lock().await {
        guard.as_ref().map(|t| t.access_token.clone())
    } else {
        None
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
    client_id: &str,
    cache_path: &PathBuf,
) {
    let access_token = api_get_token(spotify, client_id, cache_path).await;

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
            // Token expired during runtime: perform silent background auto-refresh!
            let r_tok = match spotify.get_token().lock().await {
                Ok(guard) => guard.as_ref().and_then(|t| t.refresh_token.clone()),
                Err(_) => None,
            };
            let mut refreshed = false;
            if let Some(ref rt) = r_tok {
                if let Some(new_tok) = direct_refresh_token(client_id, rt, cache_path).await {
                    if let Ok(mut guard) = spotify.get_token().lock().await {
                        *guard = Some(new_tok);
                    }
                    refreshed = true;
                }
            }
            if !refreshed {
                if let Ok(()) = spotify.refresh_token().await {
                    let _ = spotify.write_token_cache().await;
                    refreshed = true;
                }
            }

            if refreshed {
                *error_count = 0;
                let _ = event_sender.send(SpotifyEvent::Connected("🟢 Connected".to_string()));
            } else {
                *error_count += 1;
                if *error_count >= 3 {
                    let _ = event_sender.send(SpotifyEvent::Disconnected(
                        "🔄 Refreshing Spotify session…".to_string(),
                    ));
                }
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
                        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/insane".to_string());
                        let _ = std::fs::create_dir_all(format!("{}/.config/spotif", home));
                        let _ = std::fs::write(format!("{}/.config/spotif/current_cover.png", home), &bytes);
                        let _ = std::fs::write(format!("{}/spotify-terminal/.current_cover.png", home), &bytes);
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
                                            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/insane".to_string());
                                            let _ = std::fs::create_dir_all(format!("{}/.config/spotif", home));
                                            let _ = std::fs::write(format!("{}/.config/spotif/current_cover.png", home), &bytes);
                                            let _ = std::fs::write(format!("{}/spotify-terminal/.current_cover.png", home), &bytes);
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

/// Multi-source lyrics aggregator and real-time English translation engine
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

    // 1. Exact match LRCLIB (track, primary_artist, album, duration)
    let mut raw_lines = fetch_lrclib_exact(&client, &clean_title, primary_artist, &album_name, duration_secs, duration_ms).await;

    // 2. Clean title + primary artist LRCLIB
    if raw_lines.is_empty() {
        raw_lines = fetch_lrclib_title_artist(&client, &clean_title, primary_artist, duration_ms).await;
    }

    // 3. Search query LRCLIB (clean title + primary artist)
    if raw_lines.is_empty() {
        raw_lines = fetch_lrclib_search(&client, &format!("{} {}", clean_title, primary_artist), duration_ms).await;
    }

    // 4. Search query LRCLIB with clean title only
    if raw_lines.is_empty() {
        raw_lines = fetch_lrclib_search(&client, &clean_title, duration_ms).await;
    }

    // 5. Lyrics.ovh API with primary artist and clean title
    if raw_lines.is_empty() {
        raw_lines = fetch_lyrics_ovh(&client, primary_artist, &clean_title, duration_ms).await;
    }

    // 6. Lyrics.ovh API with full artist name
    if raw_lines.is_empty() && primary_artist != artist_name {
        raw_lines = fetch_lyrics_ovh(&client, &artist_name, &clean_title, duration_ms).await;
    }

    // 7. If STILL no lyrics found anywhere, generate structured English musical/acoustic guide
    if raw_lines.is_empty() {
        raw_lines = generate_instrumental_guide(duration_ms, &clean_title, primary_artist);
    }

    // 8. Translation pipeline: if lyrics are in any non-English language, translate to English!
    let final_lyrics = if !raw_lines.is_empty() && !is_lyrics_english(&raw_lines) {
        translate_lyrics_to_english(&client, raw_lines).await
    } else {
        raw_lines
    };

    let _ = event_sender.send(SpotifyEvent::LyricsLoaded {
        track_id,
        lyrics: final_lyrics,
    });
}

async fn fetch_lrclib_exact(
    client: &reqwest::Client,
    track: &str,
    artist: &str,
    album: &str,
    duration_secs: u64,
    duration_ms: u64,
) -> Vec<LyricLine> {
    let url = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}&album_name={}&duration={}",
        urlencode(track),
        urlencode(artist),
        urlencode(album),
        duration_secs
    );
    if let Ok(resp) = client.get(&url).send().await {
        if resp.status().is_success() {
            if let Ok(val) = resp.json::<serde_json::Value>().await {
                if let Some(synced) = val["syncedLyrics"].as_str() {
                    return parse_lrc(synced);
                } else if let Some(plain) = val["plainLyrics"].as_str() {
                    return parse_plain_lyrics(plain, duration_ms);
                }
            }
        }
    }
    vec![]
}

async fn fetch_lrclib_title_artist(
    client: &reqwest::Client,
    track: &str,
    artist: &str,
    duration_ms: u64,
) -> Vec<LyricLine> {
    let url = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}",
        urlencode(track),
        urlencode(artist),
    );
    if let Ok(resp) = client.get(&url).send().await {
        if resp.status().is_success() {
            if let Ok(val) = resp.json::<serde_json::Value>().await {
                if let Some(synced) = val["syncedLyrics"].as_str() {
                    return parse_lrc(synced);
                } else if let Some(plain) = val["plainLyrics"].as_str() {
                    return parse_plain_lyrics(plain, duration_ms);
                }
            }
        }
    }
    vec![]
}

async fn fetch_lrclib_search(
    client: &reqwest::Client,
    query: &str,
    duration_ms: u64,
) -> Vec<LyricLine> {
    let url = format!("https://lrclib.net/api/search?q={}", urlencode(query));
    if let Ok(resp) = client.get(&url).send().await {
        if resp.status().is_success() {
            if let Ok(items) = resp.json::<Vec<serde_json::Value>>().await {
                for it in items {
                    if let Some(synced) = it["syncedLyrics"].as_str() {
                        let lines = parse_lrc(synced);
                        if !lines.is_empty() {
                            return lines;
                        }
                    } else if let Some(plain) = it["plainLyrics"].as_str() {
                        let lines = parse_plain_lyrics(plain, duration_ms);
                        if !lines.is_empty() {
                            return lines;
                        }
                    }
                }
            }
        }
    }
    vec![]
}

async fn fetch_lyrics_ovh(
    client: &reqwest::Client,
    artist: &str,
    title: &str,
    duration_ms: u64,
) -> Vec<LyricLine> {
    let url = format!(
        "https://api.lyrics.ovh/v1/{}/{}",
        urlencode(artist),
        urlencode(title)
    );
    if let Ok(resp) = client.get(&url).send().await {
        if resp.status().is_success() {
            if let Ok(val) = resp.json::<serde_json::Value>().await {
                if let Some(lyrics_str) = val["lyrics"].as_str() {
                    let lines = parse_plain_lyrics(lyrics_str, duration_ms);
                    if !lines.is_empty() {
                        return lines;
                    }
                }
            }
        }
    }
    vec![]
}

fn generate_instrumental_guide(duration_ms: u64, track_name: &str, artist_name: &str) -> Vec<LyricLine> {
    let dur = if duration_ms == 0 { 180_000 } else { duration_ms };
    let step = dur / 6;

    vec![
        LyricLine {
            time_ms: 1_000,
            text: format!("♫ [Musical Intro — {} by {}] ♫", track_name, artist_name),
        },
        LyricLine {
            time_ms: step.max(8_000),
            text: "♫ [Melodic Theme & Atmospheric Rhythm] ♫".to_string(),
        },
        LyricLine {
            time_ms: (step * 2).min(dur.saturating_sub(40_000)),
            text: "♫ [Harmonic Progression / Verse Dynamic] ♫".to_string(),
        },
        LyricLine {
            time_ms: (step * 3).min(dur.saturating_sub(30_000)),
            text: "♫ [Musical Climax / Main Theme] ♫".to_string(),
        },
        LyricLine {
            time_ms: (step * 4).min(dur.saturating_sub(20_000)),
            text: "♫ [Instrumental Solo & Dynamic Breakdown] ♫".to_string(),
        },
        LyricLine {
            time_ms: (step * 5).min(dur.saturating_sub(10_000)),
            text: "♫ [Outro / Smooth Deceleration & Fade] ♫".to_string(),
        },
    ]
}

fn is_lyrics_english(lines: &[LyricLine]) -> bool {
    let mut non_latin_count = 0;
    let mut total_words = 0;
    let mut eng_word_count = 0;
    let mut hinglish_count = 0;

    let hinglish_markers = [
        "mera", "meri", "tere", "teri", "hum", "tum", "hai", "nahi", "kya", "dil", "pyar",
        "ishq", "zindagi", "saath", "guzara", "chahat", "deewana", "jaana", "aankhein",
        "khuda", "dua", "mohabbat", "sanam", "kabhi", "hoga", "raha", "rahi", "hote",
        "kaun", "kuch", "sab", "apna", "apni", "badal", "kare", "karna", "baat", "yaad",
    ];

    let english_words = [
        "the", "you", "and", "to", "in", "that", "it", "is", "was", "for", "on", "are",
        "with", "my", "me", "we", "your", "love", "baby", "all", "can", "so", "just",
        "be", "this", "have", "from", "but", "not", "what", "like", "see", "go", "know",
        "get", "time", "feel", "night", "now", "down", "day", "way", "say", "girl", "boy",
    ];

    for line in lines {
        for c in line.text.chars() {
            let u = c as u32;
            if (0x0400..=0x04FF).contains(&u)
                || (0x0600..=0x06FF).contains(&u)
                || (0x0900..=0x097F).contains(&u)
                || (0x3040..=0x30FF).contains(&u)
                || (0x4E00..=0x9FFF).contains(&u)
                || (0xAC00..=0xD7AF).contains(&u)
                || (0x0E00..=0x0E7F).contains(&u)
            {
                non_latin_count += 1;
            }
        }

        for word in line.text.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase();
            if clean.is_empty() {
                continue;
            }
            total_words += 1;
            if hinglish_markers.contains(&clean.as_str()) {
                hinglish_count += 1;
            }
            if english_words.contains(&clean.as_str()) {
                eng_word_count += 1;
            }
        }
    }

    if non_latin_count >= 2 {
        return false;
    }
    if hinglish_count >= 2 {
        return false;
    }
    if total_words > 0 && (eng_word_count as f32 / total_words as f32) >= 0.12 {
        return true;
    }
    false
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

async fn translate_lyrics_to_english(client: &reqwest::Client, mut lines: Vec<LyricLine>) -> Vec<LyricLine> {
    if lines.is_empty() {
        return lines;
    }

    let mut hinglish_score = 0;
    let hinglish_markers = [
        "mera", "meri", "tere", "teri", "hum", "tum", "hai", "nahi", "kya", "dil", "pyar",
        "ishq", "zindagi", "saath", "guzara", "chahat", "deewana", "jaana", "khuda", "dua",
    ];
    for line in &lines {
        for word in line.text.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase();
            if hinglish_markers.contains(&clean.as_str()) {
                hinglish_score += 1;
            }
        }
    }
    let langpair = if hinglish_score >= 2 { "hi|en" } else { "autodetect|en" };

    // Batch lines into chunks of max 6 lines or ~350 chars
    let mut chunk_indices: Vec<Vec<usize>> = Vec::new();
    let mut current_chunk: Vec<usize> = Vec::new();
    let mut current_chars = 0;

    for (idx, line) in lines.iter().enumerate() {
        if !current_chunk.is_empty() && (current_chunk.len() >= 6 || current_chars + line.text.len() > 350) {
            chunk_indices.push(current_chunk);
            current_chunk = Vec::new();
            current_chars = 0;
        }
        current_chars += line.text.len() + 1;
        current_chunk.push(idx);
    }
    if !current_chunk.is_empty() {
        chunk_indices.push(current_chunk);
    }

    let mut handles = Vec::new();
    for (chunk_idx, chunk) in chunk_indices.into_iter().enumerate() {
        let client_c = client.clone();
        let texts: Vec<String> = chunk.iter().map(|&i| lines[i].text.clone()).collect();
        let lp = langpair.to_string();
        handles.push(tokio::spawn(async move {
            let payload = texts.join("\n");
            let url = format!(
                "https://api.mymemory.translated.net/get?q={}&langpair={}",
                urlencode(&payload),
                lp
            );
            if let Ok(resp) = client_c.get(&url).send().await {
                if resp.status().is_success() {
                    if let Ok(val) = resp.json::<serde_json::Value>().await {
                        if let Some(trans_str) = val["responseData"]["translatedText"].as_str() {
                            let translated: Vec<String> = trans_str
                                .lines()
                                .map(|s| decode_html_entities(s.trim()))
                                .collect();
                            if translated.len() == texts.len() {
                                return (chunk_idx, chunk, Some(translated));
                            }
                        }
                    }
                }
            }
            (chunk_idx, chunk, None)
        }));
    }

    let results = futures::future::join_all(handles).await;
    for res in results {
        if let Ok((_c_idx, chunk, Some(trans_lines))) = res {
            for (i, &line_idx) in chunk.iter().enumerate() {
                let t = &trans_lines[i];
                if !t.is_empty() && !t.starts_with("MYMEMORY WARNING:") {
                    lines[line_idx].text = t.clone();
                }
            }
        }
    }

    lines
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
