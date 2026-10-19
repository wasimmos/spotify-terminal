use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, CardViewMode};
use crate::ui::{
    art::AlbumArtWidget,
    controls::{AlbumSubControlsWidget, ProgressBarWidget},
    details::SongDetailsWidget,
    lyrics::LyricsWidget,
    playlist_modal::PlaylistModalWidget,
    visualizer::VisualizerWidget,
};

pub fn render_ui(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let theme = app.theme;

    // Outer layout:
    // 1. Top Title Bar: "✦ wasimmos" (1 row)
    // 2. Now Playing Card (flexible)
    // 3. Middle Progress Bar ONLY (1 row - removed hotkey line!)
    // 4. Bottom Discrete CAVA Visualizer (60-65% of screen height)
    let vis_height = ((size.height as f32) * 0.62).round().max(10.0) as u16;

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                  // Top title bar: "wasimmos"
            Constraint::Min(8),                     // Now Playing card (~35% of screen)
            Constraint::Length(1),                  // Clean progress bar only!
            Constraint::Length(vis_height),         // Discrete CAVA Visualizer (60-65% of screen!)
        ])
        .split(size);

    let title_bar_area = main_chunks[0];
    let card_area = main_chunks[1];
    let progress_area = main_chunks[2];
    let visualizer_area = main_chunks[3];

    // ==========================================
    // 1. TOP TITLE BAR: "wasimmos"
    // ==========================================
    let clock_str = app.current_time_str();
    let title_left = Span::styled(" ✦ wasimmos ", Style::default().fg(theme.title_color()).add_modifier(Modifier::BOLD));
    let status_span = Span::styled(format!(" [{}] ", app.spotify_status), Style::default().fg(Color::Rgb(170, 220, 180)));
    let clock_right = Span::styled(format!(" {} ", clock_str), Style::default().fg(Color::Rgb(150, 150, 170)));

    let used_len = (14 + app.spotify_status.len() + clock_str.len() + 4) as u16;
    let spacer_len = title_bar_area.width.saturating_sub(used_len) as usize;

    let top_bar_line = Line::from(vec![
        title_left,
        status_span,
        Span::raw(" ".repeat(spacer_len)),
        clock_right,
    ]);
    f.render_widget(Paragraph::new(top_bar_line), title_bar_area);

    // ==========================================
    // 2. UPPER NOW PLAYING CARD
    // ==========================================
    let card_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.card_border()));

    let inner_card = card_block.inner(card_area);
    f.render_widget(card_block, card_area);

    if inner_card.width > 20 && inner_card.height > 6 {
        // Calculate square cover geometry
        let max_art_rows = inner_card.height.saturating_sub(1);
        let album_col_width: u16 = if inner_card.width > 100 { 26 } else if inner_card.width > 80 { 22 } else { 18 };
        let square_rows = (album_col_width / 2).min(max_art_rows);

        let card_horiz = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(album_col_width),
                Constraint::Min(20),
            ])
            .split(inner_card);

        let left_album_pane = card_horiz[0];
        let right_content_pane = card_horiz[1];

        // Left Pane: Album Art at top + transport controls directly below (zero gap!)
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(square_rows), // Album cover positioned at top
                Constraint::Length(1),           // Transport controls directly beneath cover
                Constraint::Min(0),              // Remaining spacer
            ])
            .split(left_album_pane);

        let art_area = left_chunks[0];
        let sub_controls_area = left_chunks[1];

        // Render Album Art: use high-resolution graphics protocol if available, or procedural fallback
        if let Some(ref mut image_state) = app.album_image_state {
            let img_widget = ratatui_image::StatefulImage::default()
                .resize(ratatui_image::Resize::Fit(None));
            f.render_stateful_widget(img_widget, art_area, image_state);
        } else {
            f.render_widget(
                AlbumArtWidget::new(theme, app.album_cover.as_ref(), app.anim_time),
                art_area,
            );
        }

        // Render sleek Transport Controls directly below Album Art
        f.render_widget(
            AlbumSubControlsWidget {
                is_playing: app.is_playing,
                theme,
            },
            sub_controls_area,
        );

        // Right Pane: Header (Song title + Playing Badge) + Body (Lyrics / Details)
        // Taking header to 2 rows moves lyrics higher up
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Clean compact header line
                Constraint::Min(4),    // Content area (Lyrics OR Song Details)
            ])
            .split(right_content_pane);

        let header_area = right_chunks[0];
        let body_area = right_chunks[1];

        // Header: `🎤 Song Title  •  Artist` (full width, no buttons to truncate singer name!)
        let play_badge = if app.is_playing { " 🎵 playing " } else { " ⏸ paused " };

        let title_line = Line::from(vec![
            Span::styled("🎤  ", Style::default().fg(theme.title_color())),
            Span::styled(&app.current_track, Style::default().fg(theme.title_color()).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  •  {}", app.current_artist), Style::default().fg(Color::Rgb(160, 160, 180))),
        ]);

        let right_badge_line = Line::from(vec![
            Span::styled(play_badge, Style::default().fg(Color::White).bg(Color::Rgb(40, 38, 55))),
        ]);

        let header_horiz = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(10),
                Constraint::Length(14),
            ])
            .split(header_area);

        f.render_widget(Paragraph::new(title_line), header_horiz[0]);
        f.render_widget(Paragraph::new(right_badge_line), header_horiz[1]);

        // Body: Switch between Lyrics mode and Song Details mode (toggled via 'l' and 'd' hotkeys)
        match app.view_mode {
            CardViewMode::Lyrics => {
                f.render_widget(
                    LyricsWidget::new(&app.lyrics, app.playback_time_ms, theme),
                    body_area,
                );
            }
            CardViewMode::SongDetails => {
                f.render_widget(
                    SongDetailsWidget {
                        details: &app.track_details,
                        theme,
                    },
                    body_area,
                );
            }
        }
    }

    // ==========================================
    // 3. MIDDLE PROGRESS BAR ONLY (Clean & Minimalist!)
    // ==========================================
    f.render_widget(
        ProgressBarWidget {
            elapsed: app.elapsed(),
            total: app.duration,
            theme,
        },
        progress_area,
    );

    // ==========================================
    // 4. BOTTOM CAVA VISUALIZER (90% size of reference)
    // ==========================================
    let bar_width = if visualizer_area.width > 120 { 3 } else { 2 };
    let gap_width = 1;
    let num_bars = ((visualizer_area.width + gap_width) / (bar_width + gap_width)) as usize;
    app.visualizer.resize_bars(num_bars);

    f.render_widget(
        VisualizerWidget::new(&app.visualizer, theme),
        visualizer_area,
    );

    // ==========================================
    // 5. COLLAPSIBLE PLAYLIST MODAL (OVERLAY)
    // ==========================================
    if app.show_playlists {
        f.render_widget(
            PlaylistModalWidget {
                playlists: &app.playlists,
                selected: app.selected_playlist,
                search_query: &app.search_query,
                is_searching: app.is_searching,
                theme,
            },
            size,
        );
    }
}
