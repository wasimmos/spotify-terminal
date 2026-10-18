use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::App;
use crate::ui::{
    art::AlbumArtWidget,
    controls::{AlbumSubControlsWidget, ProgressBarWidget},
    lyrics::LyricsWidget,
    playlist_modal::PlaylistModalWidget,
    visualizer::VisualizerWidget,
};

pub fn render_ui(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let theme = app.theme;

    // Outer layout:
    // 1. Top Title Bar (1 row)
    // 2. Now Playing Card (flexible, ~48-52%)
    // 3. Middle Progress & Hotkeys Bar (3 rows)
    // 4. Bottom CAVA Visualizer (~40-42% of screen, 90% of reference image)
    let vis_height = ((size.height as f32) * 0.40).round().max(6.0) as u16;

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                  // Top title bar
            Constraint::Min(12),                    // Now Playing card
            Constraint::Length(3),                  // Progress bar & key hint bar
            Constraint::Length(vis_height),         // Discrete CAVA Visualizer
        ])
        .split(size);

    let title_bar_area = main_chunks[0];
    let card_area = main_chunks[1];
    let controls_area = main_chunks[2];
    let visualizer_area = main_chunks[3];

    // ==========================================
    // 1. TOP TITLE BAR
    // ==========================================
    let clock_str = app.current_time_str();
    let title_left = Span::styled(" 🌐 Disco Terminal ", Style::default().fg(Color::Rgb(160, 160, 180)));
    let clock_right = Span::styled(format!(" {} ", clock_str), Style::default().fg(Color::Rgb(160, 160, 180)));

    let top_bar_line = Line::from(vec![
        title_left,
        Span::raw(" ".repeat(title_bar_area.width.saturating_sub(24 + clock_str.len() as u16) as usize)),
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
        // Left Column (Album Art + Sub Controls) vs Right Column (Header + Lyrics)
        // Album art is square: in terminal cells with halfblocks, width ~ (height - 2) * 1.8
        let album_col_width = (((inner_card.height.saturating_sub(2)) as f32 * 1.8).round() as u16)
            .clamp(20, (inner_card.width * 42 / 100).max(22));

        let card_horiz = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(album_col_width),
                Constraint::Min(20),
            ])
            .split(inner_card);

        let left_album_pane = card_horiz[0];
        let right_lyrics_pane = card_horiz[1];

        // Left Pane: Album Art (top) + Sub Controls (bottom 2 rows)
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(4),
                Constraint::Length(2),
            ])
            .split(left_album_pane);

        let art_area = left_chunks[0];
        let sub_controls_area = left_chunks[1];

        // Render Album Art
        f.render_widget(
            AlbumArtWidget::new(theme, app.album_cover.as_ref(), app.anim_time),
            art_area,
        );

        // Render Sub-controls directly below Album Art (⏮  ▶/⏸  ⏹  ⏭)
        f.render_widget(
            AlbumSubControlsWidget {
                is_playing: app.is_playing,
                volume: app.volume,
                theme,
            },
            sub_controls_area,
        );

        // Right Pane: Header (Song title + badge) + Synced Lyrics
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header row
                Constraint::Min(4),    // Lyrics
            ])
            .split(right_lyrics_pane);

        let lyrics_header_area = right_chunks[0];
        let lyrics_body_area = right_chunks[1];

        // Header: `🎤 Song Title` + `Artist` (left), and `[ 🎵 playing ]` badge (right)
        let play_badge = if app.is_playing { "🎵 playing" } else { "⏸ paused" };
        let badge_len = play_badge.len() as u16 + 6;

        let title_spans = vec![
            Span::styled("🎤  ", Style::default().fg(theme.title_color())),
            Span::styled(&app.current_track, Style::default().fg(theme.title_color()).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  •  {}", app.current_artist), Style::default().fg(Color::Rgb(160, 160, 175))),
        ];

        let header_line = Line::from(title_spans);
        let badge_line = Line::from(vec![
            Span::styled(format!("  {}  ", play_badge), Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 55))),
        ]);

        let header_horiz = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(10),
                Constraint::Length(badge_len),
            ])
            .split(lyrics_header_area);

        f.render_widget(Paragraph::new(header_line), header_horiz[0]);
        f.render_widget(Paragraph::new(badge_line), header_horiz[1]);

        // Lyrics Body
        f.render_widget(
            LyricsWidget::new(&app.lyrics, app.playback_time_ms, theme),
            lyrics_body_area,
        );
    }

    // ==========================================
    // 3. MIDDLE PROGRESS & HOTKEYS BAR
    // ==========================================
    let controls_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(2)])
        .split(controls_area);

    // Progress Bar
    f.render_widget(
        ProgressBarWidget {
            elapsed: app.elapsed(),
            total: app.duration,
            theme,
        },
        controls_chunks[0],
    );

    // Hotkey Bar matching screenshot style
    let hotkeys_line = Line::from(vec![
        Span::styled("space ", Style::default().fg(theme.key_accent()).add_modifier(Modifier::BOLD)),
        Span::styled("Play/Pause   ", Style::default().fg(Color::Rgb(180, 180, 195))),
        Span::styled("n ", Style::default().fg(theme.key_accent()).add_modifier(Modifier::BOLD)),
        Span::styled("Next   ", Style::default().fg(Color::Rgb(180, 180, 195))),
        Span::styled("p ", Style::default().fg(theme.key_accent()).add_modifier(Modifier::BOLD)),
        Span::styled("Prev   ", Style::default().fg(Color::Rgb(180, 180, 195))),
        Span::styled("s ", Style::default().fg(theme.key_accent()).add_modifier(Modifier::BOLD)),
        Span::styled("Stop   ", Style::default().fg(Color::Rgb(180, 180, 195))),
        Span::styled("P ", Style::default().fg(theme.key_accent()).add_modifier(Modifier::BOLD)),
        Span::styled("Playlists   ", Style::default().fg(Color::Rgb(180, 180, 195))),
        Span::styled("t ", Style::default().fg(theme.key_accent()).add_modifier(Modifier::BOLD)),
        Span::styled(format!("Theme [{}]   ", theme.name()), Style::default().fg(Color::Rgb(180, 180, 195))),
        Span::styled("+/- ", Style::default().fg(theme.key_accent()).add_modifier(Modifier::BOLD)),
        Span::styled(format!("Vol {}%   ", (app.volume * 100.0) as u32), Style::default().fg(Color::Rgb(180, 180, 195))),
        Span::styled("q ", Style::default().fg(theme.key_accent()).add_modifier(Modifier::BOLD)),
        Span::styled("Quit", Style::default().fg(Color::Rgb(180, 180, 195))),
    ]);
    f.render_widget(Paragraph::new(hotkeys_line), controls_chunks[1]);

    // ==========================================
    // 4. BOTTOM CAVA VISUALIZER (90% size of reference)
    // ==========================================
    // Adjust number of bands dynamically
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
