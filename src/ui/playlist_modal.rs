use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Widget},
};

use crate::ui::theme::ThemeMode;

pub struct PlaylistModalWidget<'a> {
    pub playlists: &'a [String],
    pub selected: usize,
    pub search_query: &'a str,
    pub is_searching: bool,
    pub theme: ThemeMode,
}

impl<'a> Widget for PlaylistModalWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate modal dimensions (centered, ~60% width, ~65% height)
        let modal_width = (area.width * 65 / 100).clamp(40, 80).min(area.width);
        let modal_height = (area.height * 70 / 100).clamp(14, 28).min(area.height);

        let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
        let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;

        let modal_rect = Rect::new(modal_x, modal_y, modal_width, modal_height);

        // Clear background under modal
        Clear.render(modal_rect, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.theme.card_border()))
            .title(Span::styled(" 📂 Playlists & Library ", Style::default().fg(self.theme.title_color()).add_modifier(Modifier::BOLD)))
            .title_bottom(Span::styled(" [↑/↓] Select  [/] Search  [Esc/P] Close ", Style::default().fg(Color::DarkGray)));

        let inner = block.inner(modal_rect);
        block.render(modal_rect, buf);

        if inner.height < 4 {
            return;
        }

        // Split into Search Bar (height 3) and Playlist items (remainder)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(4)])
            .split(inner);

        // Search Bar
        let search_border_style = if self.is_searching {
            Style::default().fg(self.theme.title_color())
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let search_text = if self.search_query.is_empty() {
            Span::styled("Type to search playlists or songs...", Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(self.search_query, Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        };

        let search_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(search_border_style)
            .title(" 🔍 Search ");

        Paragraph::new(search_text)
            .block(search_block)
            .render(chunks[0], buf);

        // Playlist Items
        let items: Vec<ListItem> = self
            .playlists
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let is_sel = i == self.selected;
                let (prefix, style) = if is_sel {
                    (
                        "▶  ",
                        Style::default()
                            .fg(self.theme.title_color())
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    ("   ", Style::default().fg(Color::Rgb(180, 180, 195)))
                };
                ListItem::new(format!("{}{}", prefix, name)).style(style)
            })
            .collect();

        List::new(items).render(chunks[1], buf);
    }
}
