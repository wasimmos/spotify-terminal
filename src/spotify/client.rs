use futures::StreamExt;
use image::DynamicImage;
use rspotify::{
    model::{AdditionalType, PlayableItem},
    prelude::*,
    scopes, AuthCodePkceSpotify, Credentials, OAuth,
};
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
    #[allow(dead_code)]
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

        // Spawn background tokio runtime worker
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = event_sender.send(SpotifyEvent::Disconnected(format!(
                        "Failed to start Tokio runtime: {}",
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
                            "No Spotify Client ID found. Set RSPOTIFY_CLIENT_ID in .env".to_string(),
                        ));
                        while let Some(_cmd) = cmd_rx.recv().await {}
                        return;
                    }
                };

                let creds = Credentials::new_pkce(&client_id);

                let oauth = OAuth {
                    redirect_uri: std::env::var("RSPOTIFY_REDIRECT_URI")
                        .unwrap_or_else(|_| "http://127.0.0.1:8888/callback".to_string()),
                    scopes: scopes!(
                        "user-read-playback-state",
                        "user-modify-playback-state",
                        "user-read-currently-playing",
                        "playlist-read-private",
                        "playlist-read-collaborative",
                        "user-library-read"
                    ),
                    ..Default::default()
                };

                let mut spotify = AuthCodePkceSpotify::new(creds, oauth);

                // Attempt to read token from cache
                let mut is_auth = false;
                if let Ok(Some(tok)) = spotify.read_token_cache(false).await {
                    *spotify.get_token().lock().await.unwrap() = Some(tok);
                    is_auth = true;
                    let _ = event_sender.send(SpotifyEvent::Connected("Spotify Connected (Cached)".to_string()));
                }

                if !is_auth {
                    // Try to start local loopback listener on port 8888 for 1-click automatic auth
                    let listener = TcpListener::bind("127.0.0.1:8888").await.ok();

                    let auth_url = match spotify.get_authorize_url(None) {
                        Ok(url) => url,
                        Err(e) => {
                            let _ = event_sender.send(SpotifyEvent::Disconnected(format!("OAuth error: {}", e)));
                            return;
                        }
                    };

                    let _ = event_sender.send(SpotifyEvent::Disconnected(format!(
                        "Login at: {}",
                        auth_url
                    )));

                    // Auto-open browser
                    let _ = webbrowser::open(&auth_url);

                    // Wait for the browser redirect callback on 127.0.0.1:8888
                    if let Some(listener) = listener {
                        if let Some(code) = wait_for_auth_code(listener).await {
                            if let Ok(()) = spotify.request_token(&code).await {
                                is_auth = true;
                                let _ = event_sender.send(SpotifyEvent::Connected("Spotify Connected!".to_string()));
                            }
                        }
                    }
                }

                let mut last_art_url: Option<String> = None;
                let mut poll_interval = tokio::time::interval(Duration::from_millis(1500));

                loop {
                    tokio::select! {
                        _ = poll_interval.tick() => {
                            if !is_auth {
                                continue;
                            }

                            // Fetch current playback state from Spotify
                            if let Ok(Some(playback)) = spotify.current_playback(None, None::<&[AdditionalType]>).await {
                                if let Some(item) = playback.item {
                                    if let PlayableItem::Track(full_track) = item {
                                        let artists = full_track.artists.iter().map(|a| a.name.clone()).collect::<Vec<_>>().join(", ");
                                        let art_url = full_track.album.images.first().map(|img| img.url.clone());

                                        let progress_ms = playback.progress.map(|d| d.num_milliseconds().max(0) as u64).unwrap_or(0);
                                        let duration_ms = full_track.duration.num_milliseconds().max(0) as u64;

                                        // If album art changed, download in background
                                        if art_url != last_art_url {
                                            last_art_url = art_url.clone();
                                            if let Some(ref url) = art_url {
                                                if let Ok(resp) = reqwest::get(url).await {
                                                    if let Ok(bytes) = resp.bytes().await {
                                                        if let Ok(img) = image::load_from_memory(&bytes) {
                                                            let _ = event_sender.send(SpotifyEvent::AlbumArtLoaded(img));
                                                        }
                                                    }
                                                }
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
                        }

                        Some(cmd) = cmd_rx.recv() => {
                            if !is_auth {
                                continue;
                            }
                            match cmd {
                                SpotifyCommand::TogglePlay => {
                                    if let Ok(Some(p)) = spotify.current_playback(None, None::<&[AdditionalType]>).await {
                                        if p.is_playing {
                                            let _ = spotify.pause_playback(None).await;
                                        } else {
                                            let _ = spotify.resume_playback(None, None).await;
                                        }
                                    }
                                }
                                SpotifyCommand::NextTrack => {
                                    let _ = spotify.next_track(None).await;
                                }
                                SpotifyCommand::PreviousTrack => {
                                    let _ = spotify.previous_track(None).await;
                                }
                                SpotifyCommand::FetchPlaylists => {
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
                            }
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

/// Listens on loopback port 8888 for the Spotify OAuth redirect and extracts the auth code
async fn wait_for_auth_code(listener: TcpListener) -> Option<String> {
    if let Ok((mut socket, _)) = listener.accept().await {
        let mut buf = [0u8; 2048];
        if let Ok(n) = socket.read(&mut buf).await {
            let request = String::from_utf8_lossy(&buf[..n]);
            if let Some(pos) = request.find("code=") {
                let after = &request[pos + 5..];
                let end = after.find('&').or_else(|| after.find(' ')).unwrap_or(after.len());
                let code = after[..end].to_string();

                let html = "<!DOCTYPE html><html><body style='font-family:system-ui,sans-serif;text-align:center;padding:60px;background:#100e18;color:#eee'>\
                    <h1 style='color:#32ff78;font-size:2rem'>✦ Spotify Connected Successfully!</h1>\
                    <p style='color:#aaa;font-size:1.1rem'>You can now close this browser tab and return to <b>wasimmos</b> player.</p>\
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
