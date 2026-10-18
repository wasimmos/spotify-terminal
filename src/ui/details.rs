use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

use crate::ui::theme::ThemeMode;

pub struct TrackDetails {
    pub album: String,
    pub artist: String,
    pub release_year: String,
    pub genre: String,
    pub format: String,
    pub bitrate: String,
    pub tempo: String,
    pub key_signature: String,
    pub up_next: Vec<(String, String, String)>, // (Title, Artist, Duration)
}

pub struct SongDetailsWidget<'a> {
    pub details: &'a TrackDetails,
    pub theme: ThemeMode,
}

impl<'a> Widget for SongDetailsWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 12 || area.height < 4 {
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7), // Metadata rows
                Constraint::Min(4),    // Up Next Queue
            ])
            .split(area);

        let meta_area = chunks[0];
        let queue_area = chunks[1];

        // 1. Track Metadata Card
        let label_style = Style::default().fg(Color::Rgb(130, 130, 150));
        let val_style = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
        let badge_style = Style::default()
            .fg(self.theme.title_color())
            .bg(Color::Rgb(35, 30, 50))
            .add_modifier(Modifier::BOLD);

        let lines = vec![
            Line::from(vec![
                Span::styled("  ALBUM        ", label_style),
                Span::styled(&self.details.album, val_style),
            ]),
            Line::from(vec![
                Span::styled("  ARTIST       ", label_style),
                Span::styled(&self.details.artist, val_style),
            ]),
            Line::from(vec![
                Span::styled("  RELEASED     ", label_style),
                Span::styled(format!("{} • {}", self.details.release_year, self.details.genre), val_style),
            ]),
            Line::from(vec![
                Span::styled("  AUDIO SPEC   ", label_style),
                Span::styled(format!("  {} • {}  ", self.details.format, self.details.bitrate), badge_style),
            ]),
            Line::from(vec![
                Span::styled("  TEMPO & KEY  ", label_style),
                Span::styled(format!("{} • {}", self.details.tempo, self.details.key_signature), val_style),
            ]),
        ];

        Paragraph::new(lines).render(meta_area, buf);

        // 2. Up Next Queue Preview
        let queue_block = Block::default()
            .borders(Borders::TOP)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(60, 55, 75)))
            .title(Span::styled(" ── UP NEXT IN QUEUE ── ", Style::default().fg(self.theme.title_color())));

        let inner_queue = queue_block.inner(queue_area);
        queue_block.render(queue_area, buf);

        let mut queue_lines = Vec::new();
        for (i, (title, artist, dur)) in self.details.up_next.iter().enumerate() {
            if queue_lines.len() >= inner_queue.height as usize {
                break;
            }
            let num_span = Span::styled(format!("  {}. ", i + 1), Style::default().fg(self.theme.title_color()));
            let title_span = Span::styled(title, Style::default().fg(Color::White));
            let artist_span = Span::styled(format!(" — {}", artist), Style::default().fg(Color::Rgb(140, 140, 155)));
            let dur_span = Span::styled(format!("  {}", dur), Style::default().fg(Color::Rgb(100, 100, 120)));

            queue_lines.push(Line::from(vec![num_span, title_span, artist_span, dur_span]));
        }

        Paragraph::new(queue_lines).render(inner_queue, buf);
    }
}
