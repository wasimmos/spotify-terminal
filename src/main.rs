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

use app::App;
use ui::render_ui;

fn main() -> Result<()> {
    // Panic hook to safely restore terminal on error
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

    // 60 FPS tick rate (~16ms)
    let tick_rate = Duration::from_millis(16);

    loop {
        app.on_tick();

        terminal.draw(|f| {
            render_ui(f, &mut app);
        })?;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.show_playlists {
                        // Modal is active
                        if app.is_searching {
                            match key.code {
                                KeyCode::Esc | KeyCode::Enter => {
                                    app.is_searching = false;
                                }
                                KeyCode::Backspace => {
                                    app.search_query.pop();
                                }
                                KeyCode::Char(c) => {
                                    app.search_query.push(c);
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('P') => {
                                    app.show_playlists = false;
                                }
                                KeyCode::Enter => {
                                    app.show_playlists = false;
                                }
                                KeyCode::Char('/') => {
                                    app.is_searching = true;
                                }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    app.next_playlist();
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    app.prev_playlist();
                                }
                                _ => {}
                            }
                        }
                    } else {
                        // Main player controls
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.should_quit = true;
                            }
                            KeyCode::Char(' ') => {
                                app.toggle_play();
                            }
                            KeyCode::Char('s') => {
                                app.stop();
                            }
                            // Toggle view mode between Lyrics and Song Details!
                            KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Char('d') | KeyCode::Char('D') => {
                                app.toggle_view_mode();
                            }
                            // Toggle collapsible playlists
                            KeyCode::Char('P') | KeyCode::Tab => {
                                app.toggle_playlists();
                            }
                            // Cycle color themes
                            KeyCode::Char('t') | KeyCode::Char('m') => {
                                app.next_theme();
                            }
                            KeyCode::Char('n') => {
                                // Next track
                                app.playback_time_ms = 0;
                            }
                            KeyCode::Char('p') => {
                                // Previous track
                                app.playback_time_ms = 0;
                            }
                            KeyCode::Char('+') | KeyCode::Char('=') => {
                                app.volume_up();
                            }
                            KeyCode::Char('-') => {
                                app.volume_down();
                            }
                            _ => {}
                        }
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

    println!("👋 Thanks for using wasimmos player! Exited cleanly.");
    Ok(())
}
