use crate::app::{App, InputMode, SourceFetchState, SourceInputField};
use crate::config;
use crate::db::Db;
use crate::feed;
use crate::model::*;
use crate::ui;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const POLL_RATE: Duration = Duration::from_millis(100);

struct FeedMsg {
    results: Vec<(String, Result<Vec<Article>, String>)>,
}

struct ContentMsg {
    url: String,
    content: String,
}

pub fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
    db: Db,
) -> io::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .build()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let (feed_tx, mut feed_rx) = mpsc::channel::<FeedMsg>(8);
    let (content_tx, mut content_rx) = mpsc::channel::<ContentMsg>(8);

    // Load existing articles from DB
    reload_articles(&db, &mut app);

    // Initial fetch (all sources, bypass rate limit for first fetch)
    spawn_fetch(&rt, &client, &app.sources, &feed_tx);
    app.is_fetching = true;
    app.last_refresh = Some(Instant::now());
    // Mark all sources as just fetched
    for source in &app.sources {
        app.source_fetch_state
            .entry(source.name.clone())
            .or_insert_with(SourceFetchState::new)
            .last_fetch = Some(Instant::now());
    }

    loop {
        // Recompute display cache if data changed (filter + dedup)
        if app.display_dirty {
            app.recompute_display();
        }

        // Render
        terminal.draw(|f| ui::draw(f, &app))?;

        // Poll events
        if event::poll(POLL_RATE)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    handle_key(&mut app, key, &rt, &client, &feed_tx, &content_tx, &db);
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        // Drain feed messages
        while let Ok(msg) = feed_rx.try_recv() {
            app.is_fetching = false;
            let mut total_new = 0;
            let mut fetch_results = Vec::new();

            for (source_name, result) in msg.results {
                // Update per-source rate limit state
                let state = app
                    .source_fetch_state
                    .entry(source_name.clone())
                    .or_insert_with(SourceFetchState::new);
                match &result {
                    Ok(_) => state.record_success(),
                    Err(_) => state.record_failure(),
                }

                match result {
                    Ok(articles) => {
                        let mut inserted = 0;
                        for article in &articles {
                            if let Ok(true) = db.insert_article(article) {
                                inserted += 1;
                            }
                        }
                        total_new += inserted;
                        fetch_results.push((source_name, Ok(inserted)));
                    }
                    Err(e) => {
                        fetch_results.push((source_name, Err(e)));
                    }
                }
            }

            app.last_fetch_results = fetch_results;
            reload_articles(&db, &mut app);

            if total_new > 0 {
                app.set_status(format!("{} new articles fetched", total_new));
            } else {
                app.set_status("Feeds refreshed, no new articles".to_string());
            }
        }

        // Drain content messages
        while let Ok(msg) = content_rx.try_recv() {
            // Persist content to DB
            if let Some(article) = app.articles.iter().find(|a| a.url == msg.url) {
                let _ = db.save_content(article.id, &msg.content);
            }

            // Cache in memory
            if let Some(article) = app.selected_article() {
                if article.url == msg.url {
                    app.cache_content(msg.url, msg.content);
                } else {
                    app.content_cache.insert(msg.url, msg.content);
                }
            } else {
                app.content_cache.insert(msg.url, msg.content);
            }
        }

        if app.should_quit {
            crate::state::save_state(&app.to_view_state());
            return Ok(());
        }

        // Auto-refresh (using rate-limited eligible sources)
        if let Some(last) = app.last_refresh {
            if last.elapsed() >= app.refresh_interval && !app.is_fetching {
                let eligible = app.eligible_sources();
                if !eligible.is_empty() {
                    spawn_fetch(&rt, &client, &eligible, &feed_tx);
                    app.is_fetching = true;
                }
                app.last_refresh = Some(Instant::now());
            }
        }

        app.tick_count = app.tick_count.wrapping_add(1);
    }
}

fn spawn_fetch(
    rt: &tokio::runtime::Runtime,
    client: &reqwest::Client,
    sources: &[FeedSource],
    tx: &mpsc::Sender<FeedMsg>,
) {
    let client = client.clone();
    let sources: Vec<FeedSource> = sources.to_vec();
    let tx = tx.clone();
    rt.spawn(async move {
        let results = feed::fetch_all_feeds(&client, &sources).await;
        let _ = tx.send(FeedMsg { results }).await;
    });
}

fn spawn_content_fetch(
    rt: &tokio::runtime::Runtime,
    client: &reqwest::Client,
    url: &str,
    tx: &mpsc::Sender<ContentMsg>,
) {
    let client = client.clone();
    let url = url.to_string();
    let tx = tx.clone();
    rt.spawn(async move {
        let content = match feed::fetch_article_content(&client, &url).await {
            Ok(text) => text,
            Err(e) => format!("Failed to load article: {}\n\nPress [o] to open in browser.", e),
        };
        let _ = tx.send(ContentMsg { url, content }).await;
    });
}

fn reload_articles(db: &Db, app: &mut App) {
    match app.filter_mode {
        FilterMode::All => {
            if let Ok(articles) = db.get_articles(100) {
                app.articles = articles;
            }
        }
        FilterMode::Watchlist => {
            if let Ok(articles) = db.get_articles_by_tickers(&app.watchlist, 100) {
                app.articles = articles;
            }
        }
        FilterMode::Unread => {
            if let Ok(articles) = db.get_unread_articles(100) {
                app.articles = articles;
            }
        }
        FilterMode::Source => {
            if let Ok(articles) = db.get_articles(100) {
                app.articles = articles;
            }
        }
    }

    app.total_articles = db.article_count().unwrap_or(0);
    app.unread_count = db.unread_count().unwrap_or(0);
    app.display_dirty = true;
}

fn handle_key(
    app: &mut App,
    key: event::KeyEvent,
    rt: &tokio::runtime::Runtime,
    client: &reqwest::Client,
    feed_tx: &mpsc::Sender<FeedMsg>,
    content_tx: &mpsc::Sender<ContentMsg>,
    db: &Db,
) {
    // Global: Ctrl+C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }

    // Help overlay
    if app.show_help {
        if key.code == KeyCode::Char('?') || key.code == KeyCode::Esc {
            app.show_help = false;
        }
        return;
    }

    match app.input_mode {
        InputMode::Normal => handle_normal_key(app, key, rt, client, feed_tx, content_tx, db),
        InputMode::Search => handle_search_key(app, key, db),
        InputMode::SourceAdd(_) | InputMode::SourceEdit(_) | InputMode::SourceDelete => {
            handle_source_input_key(app, key);
        }
    }
}

fn handle_normal_key(
    app: &mut App,
    key: event::KeyEvent,
    rt: &tokio::runtime::Runtime,
    client: &reqwest::Client,
    feed_tx: &mpsc::Sender<FeedMsg>,
    content_tx: &mpsc::Sender<ContentMsg>,
    db: &Db,
) {
    match app.view_mode {
        ViewMode::Feed | ViewMode::Bookmarks => {
            handle_feed_key(app, key, rt, client, feed_tx, content_tx, db)
        }
        ViewMode::Reader => handle_reader_key(app, key, rt, client, content_tx, db),
        ViewMode::Sources => handle_sources_key(app, key),
    }
}

fn handle_feed_key(
    app: &mut App,
    key: event::KeyEvent,
    rt: &tokio::runtime::Runtime,
    client: &reqwest::Client,
    feed_tx: &mpsc::Sender<FeedMsg>,
    content_tx: &mpsc::Sender<ContentMsg>,
    db: &Db,
) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('?') => app.show_help = !app.show_help,

        // Navigation
        KeyCode::Char('j') | KeyCode::Down => app.select_next(),
        KeyCode::Char('k') | KeyCode::Up => app.select_prev(),
        KeyCode::Char('g') => app.select_first(),
        KeyCode::Char('G') => app.select_last(),

        // Open reader with content fetch
        KeyCode::Enter => {
            let article_data = app.selected_article().map(|a| (a.id, a.url.clone()));
            if let Some((article_id, url)) = article_data {
                let _ = db.mark_read(article_id);
                app.enter_reader();
                // Check DB for content, then network fetch
                if app.reader_content.is_none() {
                    if let Ok(Some(content)) = db.get_content(article_id) {
                        app.cache_content(url, content);
                    } else if !app.failed_content_urls.contains(&url) {
                        spawn_content_fetch(rt, client, &url, content_tx);
                    } else {
                        app.content_loading = false;
                    }
                }
                reload_articles(db, app);
            }
        }

        // Open in browser
        KeyCode::Char('o') => {
            let article_data = app.selected_article().map(|a| (a.id, a.url.clone()));
            if let Some((id, url)) = article_data {
                let _ = db.mark_read(id);
                let _ = open::that(&url);
                app.set_status("Opened in browser".to_string());
                reload_articles(db, app);
            }
        }

        // Bookmark
        KeyCode::Char('b') => {
            let article_id = app.selected_article().map(|a| a.id);
            if let Some(id) = article_id {
                if let Ok(bookmarked) = db.toggle_bookmark(id) {
                    let msg = if bookmarked {
                        "Bookmarked"
                    } else {
                        "Unbookmarked"
                    };
                    app.set_status(msg.to_string());
                    reload_articles(db, app);
                }
            }
        }

        // View bookmarks
        KeyCode::Char('B') => {
            if app.view_mode == ViewMode::Bookmarks {
                app.view_mode = ViewMode::Feed;
                reload_articles(db, app);
            } else {
                app.view_mode = ViewMode::Bookmarks;
                if let Ok(articles) = db.get_bookmarked_articles(100) {
                    app.articles = articles;
                    app.display_dirty = true;
                }
                app.selected_index = 0;
            }
        }

        // Sources view
        KeyCode::Char('S') => {
            app.view_mode = ViewMode::Sources;
            app.selected_index = 0;
        }

        // Filter
        KeyCode::Char('f') => {
            app.cycle_filter();
            reload_articles(db, app);
            app.set_status(format!("Filter: {}", app.filter_mode.label()));
        }

        // Quick ticker filter: pick first ticker from selected article
        KeyCode::Char('T') => {
            let ticker = app
                .selected_article()
                .and_then(|a| a.tickers.first().cloned());
            if let Some(ticker) = ticker {
                app.set_ticker_filter(Some(ticker.clone()));
                app.set_status(format!("Ticker filter: {}", ticker));
            } else {
                app.set_status("No ticker detected in this article".to_string());
            }
        }

        // Clear ticker filter
        KeyCode::Char('c') => {
            if app.ticker_filter.is_some() {
                app.set_ticker_filter(None);
                app.set_status("Ticker filter cleared".to_string());
            }
        }

        // Refresh (rate-limited)
        KeyCode::Char('r') => {
            if !app.is_fetching {
                let eligible = app.eligible_sources();
                if eligible.is_empty() {
                    app.set_status("All sources are rate-limited, try again later".to_string());
                } else {
                    spawn_fetch(rt, client, &eligible, feed_tx);
                    app.is_fetching = true;
                    app.last_refresh = Some(Instant::now());
                    app.set_status("Refreshing feeds...".to_string());
                }
            }
        }

        // Search
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Search;
            app.input_buffer.clear();
        }

        // Theme
        KeyCode::Char('t') => {
            app.cycle_theme();
            app.set_status(format!("Theme: {}", app.theme_name.label()));
        }

        _ => {}
    }
}

fn handle_reader_key(
    app: &mut App,
    key: event::KeyEvent,
    rt: &tokio::runtime::Runtime,
    client: &reqwest::Client,
    content_tx: &mpsc::Sender<ContentMsg>,
    db: &Db,
) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.view_mode = ViewMode::Feed;
            app.reader_content = None;
            app.reader_scroll = 0;
            reload_articles(db, app);
        }

        // Scroll content
        KeyCode::Char('j') | KeyCode::Down => {
            app.reader_scroll = app.reader_scroll.saturating_add(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.reader_scroll = app.reader_scroll.saturating_sub(1);
        }

        // Page down / page up
        KeyCode::Char('d') => {
            app.reader_scroll = app.reader_scroll.saturating_add(10);
        }
        KeyCode::Char('u') => {
            app.reader_scroll = app.reader_scroll.saturating_sub(10);
        }

        // Scroll to top/bottom
        KeyCode::Char('g') => {
            app.reader_scroll = 0;
        }
        KeyCode::Char('G') => {
            // Scroll to a large number, UI will clamp
            app.reader_scroll = u16::MAX;
        }

        // Next/prev article
        KeyCode::Char('n') => {
            app.select_next();
            open_reader_with_content(app, rt, client, content_tx, db);
        }
        KeyCode::Char('p') => {
            app.select_prev();
            open_reader_with_content(app, rt, client, content_tx, db);
        }

        // Open in browser
        KeyCode::Char('o') => {
            if let Some(article) = app.selected_article() {
                let url = article.url.clone();
                let _ = open::that(&url);
                app.set_status("Opened in browser".to_string());
            }
        }

        // Bookmark
        KeyCode::Char('b') => {
            let article_id = app.selected_article().map(|a| a.id);
            if let Some(id) = article_id {
                if let Ok(bookmarked) = db.toggle_bookmark(id) {
                    let msg = if bookmarked {
                        "Bookmarked"
                    } else {
                        "Unbookmarked"
                    };
                    app.set_status(msg.to_string());
                    reload_articles(db, app);
                }
            }
        }

        // Ticker filter from reader
        KeyCode::Char('T') => {
            let ticker = app
                .selected_article()
                .and_then(|a| a.tickers.first().cloned());
            if let Some(ticker) = ticker {
                app.set_ticker_filter(Some(ticker.clone()));
                app.view_mode = ViewMode::Feed;
                app.reader_content = None;
                app.reader_scroll = 0;
                app.set_status(format!("Ticker filter: {}", ticker));
            }
        }

        _ => {}
    }
}

fn handle_sources_key(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.view_mode = ViewMode::Feed;
            app.selected_index = 0;
        }

        KeyCode::Char('j') | KeyCode::Down => {
            if app.selected_index < app.sources.len().saturating_sub(1) {
                app.selected_index += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.selected_index > 0 {
                app.selected_index -= 1;
            }
        }

        KeyCode::Char(' ') | KeyCode::Enter => {
            if app.selected_index < app.sources.len() {
                app.sources[app.selected_index].enabled =
                    !app.sources[app.selected_index].enabled;
                let name = app.sources[app.selected_index].name.clone();
                let enabled_str = if app.sources[app.selected_index].enabled {
                    "enabled"
                } else {
                    "disabled"
                };
                app.set_status(format!("{}: {}", name, enabled_str));
                config::save_sources(&app.sources);
            }
        }

        // Add source
        KeyCode::Char('a') => app.start_add_source(),

        // Edit source
        KeyCode::Char('e') => app.start_edit_source(),

        // Delete source
        KeyCode::Char('d') => {
            if app.selected_index < app.sources.len() {
                app.input_mode = InputMode::SourceDelete;
            }
        }

        _ => {}
    }
}

fn handle_source_input_key(app: &mut App, key: event::KeyEvent) {
    match &app.input_mode {
        InputMode::SourceAdd(field) | InputMode::SourceEdit(field) => {
            let is_name = matches!(field, SourceInputField::Name);
            let is_add = matches!(app.input_mode, InputMode::SourceAdd(_));
            match key.code {
                KeyCode::Esc => {
                    app.input_mode = InputMode::Normal;
                }
                KeyCode::Tab => {
                    // Toggle between fields
                    if is_name {
                        app.input_mode = if is_add {
                            InputMode::SourceAdd(SourceInputField::Url)
                        } else {
                            InputMode::SourceEdit(SourceInputField::Url)
                        };
                    } else {
                        app.input_mode = if is_add {
                            InputMode::SourceAdd(SourceInputField::Name)
                        } else {
                            InputMode::SourceEdit(SourceInputField::Name)
                        };
                    }
                }
                KeyCode::Enter => {
                    if is_name {
                        // Advance to URL
                        app.input_mode = if is_add {
                            InputMode::SourceAdd(SourceInputField::Url)
                        } else {
                            InputMode::SourceEdit(SourceInputField::Url)
                        };
                    } else {
                        // Confirm
                        if is_add {
                            app.confirm_add_source();
                        } else {
                            app.confirm_edit_source();
                        }
                        config::save_sources(&app.sources);
                    }
                }
                KeyCode::Backspace => {
                    if is_name {
                        app.source_edit_name.pop();
                    } else {
                        app.source_edit_url.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if is_name {
                        app.source_edit_name.push(c);
                    } else {
                        app.source_edit_url.push(c);
                    }
                }
                _ => {}
            }
        }
        InputMode::SourceDelete => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.delete_source();
                config::save_sources(&app.sources);
            }
            _ => {
                app.input_mode = InputMode::Normal;
                app.set_status("Delete cancelled".to_string());
            }
        },
        _ => {}
    }
}

fn open_reader_with_content(
    app: &mut App,
    rt: &tokio::runtime::Runtime,
    client: &reqwest::Client,
    content_tx: &mpsc::Sender<ContentMsg>,
    db: &Db,
) {
    let article_data = app.selected_article().map(|a| (a.id, a.url.clone()));
    if let Some((article_id, url)) = article_data {
        let _ = db.mark_read(article_id);
        app.enter_reader();
        if app.reader_content.is_none() {
            if let Ok(Some(content)) = db.get_content(article_id) {
                app.cache_content(url, content);
            } else if !app.failed_content_urls.contains(&url) {
                spawn_content_fetch(rt, client, &url, content_tx);
            } else {
                app.content_loading = false;
            }
        }
        reload_articles(db, app);
    }
}

fn handle_search_key(app: &mut App, key: event::KeyEvent, _db: &Db) {
    match key.code {
        KeyCode::Enter => {
            app.search_query = app.input_buffer.clone();
            app.input_mode = InputMode::Normal;
            app.input_buffer.clear();
            app.selected_index = 0;
            app.display_dirty = true;
            if app.search_query.is_empty() {
                app.set_status("Search cleared".to_string());
            } else {
                app.set_status(format!("Search: {}", app.search_query));
            }
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.input_buffer.clear();
            app.search_query.clear();
            app.selected_index = 0;
            app.display_dirty = true;
        }
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) => {
            app.input_buffer.push(c);
        }
        _ => {}
    }
}
