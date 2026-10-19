mod app;
mod audio;
mod spotify;
mod ui;

use std::io::{stdout, Result};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::App;
use ui::render_ui;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h" || a == "help") {
        print_help();
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("wasimmos (spotif) v0.1.0 - Minimalist Terminal Music Player");
        return Ok(());
    }

    // Panic hook to safely restore terminal on error
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, crossterm::cursor::Show, DisableMouseCapture);
        original_hook(panic_info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::cursor::Hide,
        EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut picker = ratatui_image::picker::Picker::from_query_stdio().unwrap_or_else(|_| {
        ratatui_image::picker::Picker::halfblocks()
    });
    if picker.protocol_type() == ratatui_image::picker::ProtocolType::Halfblocks
        && (std::env::var("KITTY_WINDOW_ID").is_ok()
            || std::env::var("TERM").map(|t| t.contains("kitty")).unwrap_or(false))
    {
        picker.set_protocol_type(ratatui_image::picker::ProtocolType::Kitty);
    }

    let mut app = App::new(picker);

    // 60 FPS tick rate (~16ms)
    let tick_rate = Duration::from_millis(16);

    loop {
        app.on_tick();

        terminal.draw(|f| {
            render_ui(f, &mut app);
        })?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
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
                                app.next_track();
                            }
                            KeyCode::Char('p') => {
                                app.previous_track();
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
            Event::Mouse(mouse) => {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    app.handle_mouse_click(mouse.column, mouse.row);
                }
            }
            _ => {}
        }
    }

    if app.should_quit {
        break;
    }
}

// Restore terminal
disable_raw_mode()?;
execute!(terminal.backend_mut(), LeaveAlternateScreen, crossterm::cursor::Show, DisableMouseCapture)?;
terminal.show_cursor()?;

    println!("👋 Thanks for using wasimmos player! Exited cleanly.");
    Ok(())
}

fn print_help() {
    println!(r#"
✦ wasimmos (spotif) - Minimalist Terminal Music Player with Spotify Integration

USAGE:
    spotif [OPTIONS]

OPTIONS:
    -h, --help       Show this help message and exit
    -v, --version    Show version information and exit

KEYBOARD CONTROLS:
    Playback:
        Space        Play / Pause current track
        s            Stop playback
        n            Skip to next track
        p            Skip to previous track
        + / =        Increase volume (+5%)
        -            Decrease volume (-5%)

    Interface & Views:
        l, d         Toggle View Mode (Synced Lyrics ↔ Song Details)
        Tab, P       Toggle Playlist Browser & Search
        t, m         Cycle visual color theme (Disco Ice, Neon Sunset, etc.)
        q, Esc       Quit player

    Playlist Navigation (when playlist browser is open):
        j / Down     Select next playlist
        k / Up       Select previous playlist
        Enter        Confirm and load selected playlist
        /            Search / filter playlists
        Esc          Close playlist browser

    Mouse Controls:
        Click Progress Bar   Seek directly to that position in the track
"#);
}
