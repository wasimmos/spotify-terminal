mod app;
mod audio;
mod ui;

use std::io::{stdout, Result};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::{App, FocusedSection};
use ui::render_ui;

fn main() -> Result<()> {
    // Install panic hook to safely restore terminal if an unexpected crash occurs
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
        original_hook(panic_info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    // 60 FPS target tick rate (~16ms)
    let tick_rate = Duration::from_millis(16);

    loop {
        app.on_tick();

        terminal.draw(|f| {
            render_ui(f, &mut app);
        })?;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.focused {
                        FocusedSection::Search => match key.code {
                            KeyCode::Esc | KeyCode::Enter => {
                                app.focused = FocusedSection::Playlists;
                            }
                            KeyCode::Backspace => {
                                app.search_query.pop();
                            }
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                            }
                            _ => {}
                        },
                        _ => match key.code {
                            KeyCode::Char('q') => {
                                app.should_quit = true;
                            }
                            KeyCode::Esc => {
                                app.should_quit = true;
                            }
                            KeyCode::Char(' ') => {
                                app.toggle_play();
                            }
                            KeyCode::Char('m') => {
                                app.cycle_mood();
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.next_playlist();
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.prev_playlist();
                            }
                            KeyCode::Char('+') | KeyCode::Char('=') => {
                                app.volume_up();
                            }
                            KeyCode::Char('-') => {
                                app.volume_down();
                            }
                            KeyCode::Char('/') => {
                                app.focused = FocusedSection::Search;
                            }
                            KeyCode::Char('n') => {
                                // Demo skip next
                                app.playback_time_ms = 0;
                            }
                            KeyCode::Char('p') => {
                                // Demo skip prev
                                app.playback_time_ms = 0;
                            }
                            _ => {}
                        },
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, crossterm::cursor::Show)?;
    terminal.show_cursor()?;

    println!("👋 Thanks for playing! Exited cleanly.");
    Ok(())
}
