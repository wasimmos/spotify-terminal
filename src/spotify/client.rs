use futures::StreamExt;
use image::DynamicImage;
use rspotify::{
    model::{AdditionalType, PlayableItem},
    prelude::*,
    scopes, AuthCodeSpotify, Credentials, OAuth,
};
use std::sync::mpsc::Sender;
use std::time::Duration;
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

                let creds = match Credentials::from_env() {
                    Some(c) => c,
                    None => {
                        let _ = event_sender.send(SpotifyEvent::Disconnected(
                            "No Spotify credentials found in .env (RSPOTIFY_CLIENT_ID / RSPOTIFY_CLIENT_SECRET)".to_string(),
                        ));
                        // Keep listening for dummy commands or wait
                        while let Some(_cmd) = cmd_rx.recv().await {}
                        return;
                    }
                };

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

                let spotify = AuthCodeSpotify::new(creds, oauth);

                let auth_url = match spotify.get_authorize_url(false) {
                    Ok(url) => url,
                    Err(e) => {
                        let _ = event_sender.send(SpotifyEvent::Disconnected(format!("OAuth error: {}", e)));
                        return;
                    }
                };

                // Attempt to load token from cache
                let token_res = spotify.read_token_cache(false).await;
                let is_auth = match token_res {
                    Ok(Some(tok)) => {
                        *spotify.get_token().lock().await.unwrap() = Some(tok);
                        true
                    }
                    _ => false,
                };

                if !is_auth {
                    let _ = event_sender.send(SpotifyEvent::Disconnected(format!(
                        "Authorize Spotify at: {}",
                        auth_url
                    )));
                } else {
                    let _ = event_sender.send(SpotifyEvent::Connected("Spotify Connected".to_string()));
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
