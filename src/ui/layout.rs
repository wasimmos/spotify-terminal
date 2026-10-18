use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, FocusedSection};
use crate::ui::{
    art::AlbumArtWidget,
    controls::TransportControlsWidget,
    lyrics::LyricsWidget,
    visualizer::VisualizerWidget,
};

pub fn render_ui(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let mood = app.mood;
    let border_style = Style::default().fg(mood.border_color());
    let active_border_style = Style::default().fg(mood.primary_color()).add_modifier(Modifier::BOLD);

    // Vertical Split: Main content (Top) vs Visualizer (Bottom)
    let vert_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(12),       // Main UI
            Constraint::Length(8),      // Bottom CAVA Visualizer
        ])
        .split(size);

    let main_area = vert_chunks[0];
    let visualizer_area = vert_chunks[1];

    // Horizontal Split: Left Column (Search, Playlists, Album Art, Controls) vs Right Column (Header, Lyrics)
    let left_width = if main_area.width > 90 {
        38
    } else {
        (main_area.width * 38 / 100).max(28)
    };

    let horiz_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_width),
            Constraint::Min(20),
        ])
        .split(main_area);

    let left_column = horiz_chunks[0];
    let right_column = horiz_chunks[1];

    // Left Column vertical split:
    // 1. Search Box (3)
    // 2. Playlists List (Min 4)
    // 3. Album Cover (10-12)
    // 4. Playback Controls below Album Cover (3)
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Search
            Constraint::Min(4),         // Playlists
            Constraint::Length(10),     // Album Cover Art
            Constraint::Length(3),      // Stop / Next / Prev Controls
        ])
        .split(left_column);

    let search_area = left_chunks[0];
    let playlists_area = left_chunks[1];
    let album_art_area = left_chunks[2];
    let controls_area = left_chunks[3];

    // Right Column vertical split:
    // 1. Now Playing Title / Artist Header (3)
    // 2. Synced Lyrics Area (Min 6)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Header
            Constraint::Min(6),         // Lyrics
        ])
        .split(right_column);

    let header_area = right_chunks[0];
    let lyrics_area = right_chunks[1];

    // ==========================================
    // 1. SEARCH BOX
    // ==========================================
    let search_focused = app.focused == FocusedSection::Search;
    let search_text = if app.search_query.is_empty() {
        if search_focused {
            Span::styled("Type to search...", Style::default().fg(Color::DarkGray))
        } else {
            Span::styled("Press / to search...", Style::default().fg(Color::DarkGray))
        }
    } else {
        Span::styled(&app.search_query, Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
    };

    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if search_focused { active_border_style } else { border_style })
        .title(Span::styled(" 🔍 Search ", Style::default().fg(mood.primary_color())));

    f.render_widget(Paragraph::new(search_text).block(search_block), search_area);

    // ==========================================
    // 2. PLAYLISTS LIST
    // ==========================================
    let playlists_focused = app.focused == FocusedSection::Playlists;
    let playlist_items: Vec<ListItem> = app
        .playlists
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let is_selected = i == app.selected_playlist;
            let (prefix, style) = if is_selected {
                (
                    "▶ ",
                    Style::default()
                        .fg(mood.primary_color())
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ("  ", Style::default().fg(Color::Gray))
            };
            ListItem::new(format!("{}{}", prefix, p)).style(style)
        })
        .collect();

    let playlists_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if playlists_focused { active_border_style } else { border_style })
        .title(Span::styled(" 📂 Playlists ", Style::default().fg(mood.primary_color())));

    f.render_widget(List::new(playlist_items).block(playlists_block), playlists_area);

    // ==========================================
    // 3. ALBUM COVER ART
    // ==========================================
    let album_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(" 💽 Album Cover ", Style::default().fg(mood.primary_color())));

    let inner_album = album_block.inner(album_art_area);
    f.render_widget(album_block, album_art_area);
    f.render_widget(
        AlbumArtWidget::new(mood, None, app.anim_time),
        inner_album,
    );

    // ==========================================
    // 4. TRANSPORT CONTROLS (Below Album Cover)
    // ==========================================
    let controls_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(" ⏯ Playback ", Style::default().fg(mood.primary_color())));

    let inner_controls = controls_block.inner(controls_area);
    f.render_widget(controls_block, controls_area);
    f.render_widget(
        TransportControlsWidget {
            is_playing: app.is_playing,
            elapsed: app.elapsed(),
            total: app.duration,
            volume: app.volume,
            mood,
        },
        inner_controls,
    );

    // ==========================================
    // 5. NOW PLAYING HEADER
    // ==========================================
    let header_lines = vec![
        Line::from(vec![
            Span::styled(" 🎵 ", Style::default().fg(mood.primary_color())),
            Span::styled(&app.current_track, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("  •  ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.current_artist, Style::default().fg(mood.secondary_color())),
            Span::styled(format!("  [{}]", mood.name()), Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(" Now Playing ", Style::default().fg(mood.primary_color())));

    f.render_widget(Paragraph::new(header_lines).block(header_block), header_area);

    // ==========================================
    // 6. SYNCED LYRICS PANEL
    // ==========================================
    let lyrics_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(" 📝 Synced Lyrics ", Style::default().fg(mood.primary_color())));

    let inner_lyrics = lyrics_block.inner(lyrics_area);
    f.render_widget(lyrics_block, lyrics_area);
    f.render_widget(
        LyricsWidget::new(&app.lyrics, app.elapsed().as_millis() as u64, mood),
        inner_lyrics,
    );

    // ==========================================
    // 7. BOTTOM CAVA-STYLE VISUALIZER
    // ==========================================
    let keybindings_hint = " [Space] Play/Pause  [m] Mood Gradient  [j/k] Playlists  [/] Search  [q] Quit ";
    let vis_title = format!(" 🌊 Spectrum Visualizer • {} ", mood.name());

    let vis_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(vis_title, Style::default().fg(mood.primary_color())))
        .title_bottom(Span::styled(keybindings_hint, Style::default().fg(Color::DarkGray)));

    let inner_vis = vis_block.inner(visualizer_area);
    f.render_widget(vis_block, visualizer_area);

    // Auto-resize analyzer frequency bars to fit screen width
    app.visualizer.resize_bars(inner_vis.width as usize);
    f.render_widget(VisualizerWidget::new(&app.visualizer, mood), inner_vis);
}
