mod app;
mod config;
mod db;
mod event;
mod feed;
mod model;
mod state;
mod ui;

use app::App;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use model::{FeedSource, Theme};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self};
use std::time::Duration;

fn main() -> io::Result<()> {
    // Install panic handler to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));

    // Parse CLI args
    let args = config::CliArgs::parse();

    // Load config file
    let cfg = config::load_config(args.config.as_ref());

    // Resolve settings
    let resolved = config::resolve(&args, &cfg);

    // Build feed sources from config or defaults
    let sources = if !cfg.sources.is_empty() {
        cfg.sources
            .iter()
            .map(|s| FeedSource {
                name: s.name.clone(),
                url: s.url.clone(),
                enabled: s.enabled,
            })
            .collect()
    } else {
        FeedSource::defaults()
    };

    // Open database
    let db_path = config::db_path();
    let db = db::Db::open(&db_path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Build app
    let mut app = App::new(resolved.watchlist, sources);
    app.refresh_interval = Duration::from_secs(resolved.refresh_interval);
    app.min_fetch_interval = Duration::from_secs(resolved.min_fetch_interval);

    // Restore saved view state (before CLI overrides)
    let saved_state = state::load_state();
    app.restore_view_state(&saved_state);

    // CLI overrides take precedence
    app.theme_name = resolved.theme;
    app.theme = Theme::from_name(resolved.theme);

    // Run the app
    let result = event::run_loop(&mut terminal, app, db);

    // Terminal teardown
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {}", err);
    }

    Ok(())
}
