use futures::StreamExt;
use image::DynamicImage;
use rspotify::{
    model::{AdditionalType, PlayableItem},
    prelude::*,
    scopes, AuthCodePkceSpotify, Config, Credentials, OAuth,
};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc as tokio_mpsc;

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
}

#[derive(Debug)]
pub enum SpotifyCommand {
    TogglePlay,
    NextTrack,
    PreviousTrack,
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
                                        format!("🟡 Token invalid ({}), re-authorizing…", e)
                                    ));
                                    // Delete stale cache
                                    let _ = std::fs::remove_file(&cache_file);
                                    // Reset spotify instance
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
                                format!("⚠ Port 8888 busy: {}", e)
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
                                    // Verify
                                    match spotify.current_user().await {
                                        Ok(_) => {
                                            is_auth = true;
                                            let _ = event_sender.send(SpotifyEvent::Connected("🟢 Connected".to_string()));
                                        }
                                        Err(e) => {
                                            let _ = event_sender.send(SpotifyEvent::Disconnected(
                                                format!("⚠ Auth OK but API failed: {}", e)
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    let _ = event_sender.send(SpotifyEvent::Disconnected(
                                        format!("⚠ Token exchange failed: {}", e)
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

                // Fetch playlists immediately
                fetch_and_send_playlists(&spotify, &event_sender).await;

                // Show last played if nothing currently playing
                fetch_recent_or_idle_track(&spotify, &event_sender).await;

                // Main poll loop
                let mut last_art_url: Option<String> = None;
                // Poll every 1.5s — fast enough to feel live, slow enough for rate limits
                let mut poll_interval = tokio::time::interval(Duration::from_millis(1500));
                let mut error_count = 0u32;

                loop {
                    tokio::select! {
                        biased;

                        Some(cmd) = cmd_rx.recv() => {
                            match cmd {
                                SpotifyCommand::TogglePlay => {
                                    match spotify.current_playback(None, None::<&[AdditionalType]>).await {
                                        Ok(Some(p)) => {
                                            if p.is_playing {
                                                let _ = spotify.pause_playback(None).await;
                                            } else {
                                                let _ = spotify.resume_playback(None, None).await;
                                            }
                                        }
                                        Ok(None) => {
                                            // Try to resume on available device
                                            if let Ok(devices) = spotify.device().await {
                                                if let Some(dev) = devices.first() {
                                                    if let Some(ref dev_id) = dev.id {
                                                        let _ = spotify.transfer_playback(dev_id, Some(true)).await;
                                                    }
                                                }
                                            }
                                        }
                                        Err(_) => {}
                                    }
                                }
                                SpotifyCommand::NextTrack => {
                                    let _ = spotify.next_track(None).await;
                                    // Small delay then poll immediately
                                    tokio::time::sleep(Duration::from_millis(300)).await;
                                    poll_playback(&spotify, &event_sender, &mut last_art_url, &mut error_count).await;
                                }
                                SpotifyCommand::PreviousTrack => {
                                    let _ = spotify.previous_track(None).await;
                                    tokio::time::sleep(Duration::from_millis(300)).await;
                                    poll_playback(&spotify, &event_sender, &mut last_art_url, &mut error_count).await;
                                }
                                SpotifyCommand::FetchPlaylists => {
                                    fetch_and_send_playlists(&spotify, &event_sender).await;
                                }
                            }
                        }

                        _ = poll_interval.tick() => {
                            poll_playback(&spotify, &event_sender, &mut last_art_url, &mut error_count).await;
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

    pub fn next_track(&self) {
        let _ = self.cmd_sender.try_send(SpotifyCommand::NextTrack);
    }

    pub fn previous_track(&self) {
        let _ = self.cmd_sender.try_send(SpotifyCommand::PreviousTrack);
    }

    pub fn fetch_playlists(&self) {
        let _ = self.cmd_sender.try_send(SpotifyCommand::FetchPlaylists);
    }
}

/// Core polling function — extracted to be callable from both the interval tick
/// and immediately after next/prev track commands.
async fn poll_playback(
    spotify: &AuthCodePkceSpotify,
    event_sender: &Sender<SpotifyEvent>,
    last_art_url: &mut Option<String>,
    error_count: &mut u32,
) {
    match spotify.current_playback(None, None::<&[AdditionalType]>).await {
        Ok(Some(playback)) => {
            *error_count = 0;

            if let Some(item) = playback.item {
                if let PlayableItem::Track(full_track) = item {
                    let artists = full_track
                        .artists
                        .iter()
                        .map(|a| a.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ");

                    let art_url = full_track.album.images.first().map(|img| img.url.clone());
                    let progress_ms = playback
                        .progress
                        .map(|d| d.num_milliseconds().max(0) as u64)
                        .unwrap_or(0);
                    let duration_ms = full_track.duration.num_milliseconds().max(0) as u64;

                    // Download album art in a spawned task so it doesn't block polling
                    if art_url != *last_art_url {
                        *last_art_url = art_url.clone();
                        if let Some(ref url) = art_url {
                            let url_clone = url.clone();
                            let sender_clone = event_sender.clone();
                            tokio::spawn(async move {
                                if let Ok(resp) = reqwest::get(&url_clone).await {
                                    if let Ok(bytes) = resp.bytes().await {
                                        if let Ok(img) = image::load_from_memory(&bytes) {
                                            let _ = sender_clone.send(SpotifyEvent::AlbumArtLoaded(img));
                                        }
                                    }
                                }
                            });
                        }
                    }

                    let _ = event_sender.send(SpotifyEvent::PlaybackUpdate(SpotifyPlaybackState {
                        track_name: full_track.name,
                        artist_name: artists,
                        album_name: full_track.album.name,
                        album_art_url: art_url,
                        progress_ms,
                        duration_ms,
                        is_playing: playback.is_playing,
                    }));
                }
            }
        }
        Ok(None) => {
            // Nothing currently playing — that's fine, stay on current display
        }
        Err(e) => {
            *error_count += 1;
            // Only show error after a few consecutive failures to avoid flicker
            if *error_count >= 3 {
                let _ = event_sender.send(SpotifyEvent::Disconnected(
                    format!("⚠ API error: {}", e)
                ));
            }
        }
    }
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

async fn fetch_recent_or_idle_track(spotify: &AuthCodePkceSpotify, event_sender: &Sender<SpotifyEvent>) {
    // Only show recent track if nothing is currently playing
    if let Ok(recent) = spotify.current_user_recently_played(Some(1), None).await {
        if let Some(first) = recent.items.first() {
            let track = &first.track;
            let artists = track
                .artists
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            let art_url = track.album.images.first().map(|img| img.url.clone());
            let duration_ms = track.duration.num_milliseconds().max(0) as u64;

            if let Some(ref url) = art_url {
                let url_clone = url.clone();
                let sender_clone = event_sender.clone();
                tokio::spawn(async move {
                    if let Ok(resp) = reqwest::get(&url_clone).await {
                        if let Ok(bytes) = resp.bytes().await {
                            if let Ok(img) = image::load_from_memory(&bytes) {
                                let _ = sender_clone.send(SpotifyEvent::AlbumArtLoaded(img));
                            }
                        }
                    }
                });
            }

            let _ = event_sender.send(SpotifyEvent::PlaybackUpdate(SpotifyPlaybackState {
                track_name: track.name.clone(),
                artist_name: artists,
                album_name: track.album.name.clone(),
                album_art_url: art_url,
                progress_ms: 0,
                duration_ms,
                is_playing: false,
            }));
        }
    }
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
