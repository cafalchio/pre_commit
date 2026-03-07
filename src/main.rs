mod app;
mod checks;
mod config;
mod draw;
mod runner;

use std::io;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, Mode};
use config::parse_args;
use draw::draw;
use runner::run_headless;

fn main() -> io::Result<()> {
    let config = parse_args().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        eprintln!("Run with --help for usage.");
        std::process::exit(1);
    });

    // Headless mode: run one check and exit, no TUI.
    if let Some(ref name) = config.run_check {
        run_headless(name, &config.repo);
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);

    loop {
        app.refresh_sys_stats();
        terminal.draw(|f| draw(f, &mut app))?;

        if let Mode::Running { .. } = &app.mode {
            app.tick_running();

            if event::poll(std::time::Duration::from_millis(16))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            app.cancel_running();
                            break;
                        }
                        KeyCode::Char('m') => {
                            app.mouse_capture = !app.mouse_capture;
                            if app.mouse_capture {
                                execute!(terminal.backend_mut(), EnableMouseCapture)?;
                            } else {
                                execute!(terminal.backend_mut(), DisableMouseCapture)?;
                            }
                        }
                        KeyCode::PageUp => app.output_scroll_up(),
                        KeyCode::PageDown => app.output_scroll_down(),
                        _ => {}
                    },
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::ScrollUp => app.output_scroll_up(),
                        MouseEventKind::ScrollDown => app.output_scroll_down(),
                        _ => {}
                    },
                    _ => {}
                }
            }
            continue;
        }

        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Esc => break,
                    KeyCode::Char('q') if !matches!(&app.mode, Mode::Setup) => break,

                    // ── Setup mode ────────────────────────────────────────────
                    KeyCode::Tab | KeyCode::Down if matches!(&app.mode, Mode::Setup) => {
                        app.setup_focus = 1 - app.setup_focus;
                    }
                    KeyCode::Up if matches!(&app.mode, Mode::Setup) => {
                        app.setup_focus = 1 - app.setup_focus;
                    }
                    KeyCode::Enter if matches!(&app.mode, Mode::Setup) => {
                        app.confirm_setup();
                    }
                    KeyCode::Char(c) if matches!(&app.mode, Mode::Setup) => {
                        app.setup_type_char(c);
                    }
                    KeyCode::Backspace if matches!(&app.mode, Mode::Setup) => {
                        app.setup_backspace();
                    }

                    // ── Selecting / Done modes ─────────────────────────────────
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Char(' ') if matches!(&app.mode, Mode::Selecting) => {
                        app.toggle_current();
                    }
                    KeyCode::Char('a') if matches!(&app.mode, Mode::Selecting) => {
                        app.select_all();
                    }
                    KeyCode::Char('n') if matches!(&app.mode, Mode::Selecting) => {
                        app.select_none();
                    }
                    KeyCode::Enter
                        if matches!(&app.mode, Mode::Selecting | Mode::Done) =>
                    {
                        app.start_running();
                    }
                    KeyCode::Char('r') if matches!(&app.mode, Mode::Done) => {
                        app.reset_to_selecting();
                    }
                    KeyCode::Char('m') => {
                        app.mouse_capture = !app.mouse_capture;
                        if app.mouse_capture {
                            execute!(terminal.backend_mut(), EnableMouseCapture)?;
                        } else {
                            execute!(terminal.backend_mut(), DisableMouseCapture)?;
                        }
                    }
                    KeyCode::PageUp => app.output_scroll_up(),
                    KeyCode::PageDown => app.output_scroll_down(),
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if matches!(&app.mode, Mode::Selecting) {
                            app.handle_mouse_click(mouse.column, mouse.row);
                        }
                    }
                    MouseEventKind::ScrollUp => app.output_scroll_up(),
                    MouseEventKind::ScrollDown => app.output_scroll_down(),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
