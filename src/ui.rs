use crate::app::{App, InputMode};
use crate::model::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};

pub fn draw(frame: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(0),   // body
            Constraint::Length(1), // footer
        ])
        .split(frame.area());

    draw_header(frame, outer[0], app);

    match app.view_mode {
        ViewMode::Feed | ViewMode::Bookmarks => draw_feed(frame, outer[1], app),
        ViewMode::Reader => draw_reader(frame, outer[1], app),
        ViewMode::Sources => draw_sources(frame, outer[1], app),
    }

    draw_footer(frame, outer[2], app);

    if app.show_help {
        draw_help_overlay(frame, app);
    }
}

// ============================================================
// Header
// ============================================================

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;

    let fetch_indicator = if app.is_fetching {
        format!(" {} Fetching...", app.spinner_char())
    } else {
        format!(" Refresh: {}s", app.refresh_seconds_remaining())
    };

    let filter_text = format!(" Filter:{}", app.filter_mode.label());
    let ticker_filter_text = if let Some(ref t) = app.ticker_filter {
        format!(" [{}]", t)
    } else {
        String::new()
    };
    let watchlist_text = if app.watchlist.is_empty() {
        String::new()
    } else {
        format!(" Tickers:{}", app.watchlist.join(","))
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " StockNewsTUI ",
            Style::default()
                .fg(theme.header)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " {}total {}unread",
                app.total_articles, app.unread_count
            ),
            Style::default().fg(theme.muted),
        ),
        Span::styled(filter_text, Style::default().fg(theme.accent)),
        Span::styled(
            ticker_filter_text,
            Style::default()
                .fg(theme.positive)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(watchlist_text, Style::default().fg(theme.muted)),
        Span::styled(
            format!(" Theme:{}", app.theme_name.label()),
            Style::default().fg(theme.muted),
        ),
        Span::styled(fetch_indicator, Style::default().fg(theme.muted)),
    ]));
    frame.render_widget(header, area);
}

// ============================================================
// Footer
// ============================================================

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;

    if let Some(status) = app.status_text() {
        let footer = Paragraph::new(Span::styled(
            format!(" {}", status),
            Style::default().fg(theme.accent),
        ));
        frame.render_widget(footer, area);
        return;
    }

    let footer = match &app.input_mode {
        InputMode::Search => Paragraph::new(Line::from(vec![
            Span::styled(" /", Style::default().fg(theme.accent)),
            Span::raw(&app.input_buffer),
            Span::styled("_", Style::default().fg(theme.accent)),
            Span::styled(
                "  [Enter]Search [Esc]Cancel",
                Style::default().fg(theme.muted),
            ),
        ])),
        InputMode::SourceAdd(_) | InputMode::SourceEdit(_) => {
            Paragraph::new(Line::from(vec![
                Span::styled(" [Tab]", Style::default().fg(theme.accent)),
                Span::styled("Switch field ", Style::default().fg(theme.fg)),
                Span::styled("[Enter]", Style::default().fg(theme.accent)),
                Span::styled("Next/Confirm ", Style::default().fg(theme.fg)),
                Span::styled("[Esc]", Style::default().fg(theme.accent)),
                Span::styled("Cancel", Style::default().fg(theme.fg)),
            ]))
        }
        InputMode::SourceDelete => Paragraph::new(Line::from(vec![
            Span::styled(" [y]", Style::default().fg(theme.accent)),
            Span::styled("Confirm delete ", Style::default().fg(theme.fg)),
            Span::styled("[any]", Style::default().fg(theme.accent)),
            Span::styled("Cancel", Style::default().fg(theme.fg)),
        ])),
        InputMode::Normal => match app.view_mode {
            ViewMode::Feed | ViewMode::Bookmarks => {
                let mut spans = vec![
                    Span::styled("[?]", Style::default().fg(theme.accent)),
                    Span::styled("Help ", Style::default().fg(theme.fg)),
                    Span::styled("[q]", Style::default().fg(theme.accent)),
                    Span::styled("Quit ", Style::default().fg(theme.fg)),
                    Span::styled("[Enter]", Style::default().fg(theme.accent)),
                    Span::styled("Read ", Style::default().fg(theme.fg)),
                    Span::styled("[o]", Style::default().fg(theme.accent)),
                    Span::styled("Open ", Style::default().fg(theme.fg)),
                    Span::styled("[T]", Style::default().fg(theme.accent)),
                    Span::styled("Ticker ", Style::default().fg(theme.fg)),
                ];
                if app.ticker_filter.is_some() {
                    spans.push(Span::styled("[c]", Style::default().fg(theme.accent)));
                    spans.push(Span::styled("Clear ", Style::default().fg(theme.fg)));
                }
                spans.extend_from_slice(&[
                    Span::styled("[f]", Style::default().fg(theme.accent)),
                    Span::styled("Filter ", Style::default().fg(theme.fg)),
                    Span::styled("[r]", Style::default().fg(theme.accent)),
                    Span::styled("Refresh ", Style::default().fg(theme.fg)),
                    Span::styled("[/]", Style::default().fg(theme.accent)),
                    Span::styled("Search", Style::default().fg(theme.fg)),
                ]);
                Paragraph::new(Line::from(spans))
            }
            ViewMode::Reader => Paragraph::new(Line::from(vec![
                Span::styled("[Esc]", Style::default().fg(theme.accent)),
                Span::styled("Back ", Style::default().fg(theme.fg)),
                Span::styled("[j/k]", Style::default().fg(theme.accent)),
                Span::styled("Scroll ", Style::default().fg(theme.fg)),
                Span::styled("[d/u]", Style::default().fg(theme.accent)),
                Span::styled("Page ", Style::default().fg(theme.fg)),
                Span::styled("[n/p]", Style::default().fg(theme.accent)),
                Span::styled("Next/Prev ", Style::default().fg(theme.fg)),
                Span::styled("[o]", Style::default().fg(theme.accent)),
                Span::styled("Browser ", Style::default().fg(theme.fg)),
                Span::styled("[b]", Style::default().fg(theme.accent)),
                Span::styled("Bookmark ", Style::default().fg(theme.fg)),
                Span::styled("[T]", Style::default().fg(theme.accent)),
                Span::styled("Ticker", Style::default().fg(theme.fg)),
            ])),
            ViewMode::Sources => Paragraph::new(Line::from(vec![
                Span::styled("[Esc]", Style::default().fg(theme.accent)),
                Span::styled("Back ", Style::default().fg(theme.fg)),
                Span::styled("[Space]", Style::default().fg(theme.accent)),
                Span::styled("Toggle ", Style::default().fg(theme.fg)),
                Span::styled("[a]", Style::default().fg(theme.accent)),
                Span::styled("Add ", Style::default().fg(theme.fg)),
                Span::styled("[e]", Style::default().fg(theme.accent)),
                Span::styled("Edit ", Style::default().fg(theme.fg)),
                Span::styled("[d]", Style::default().fg(theme.accent)),
                Span::styled("Delete", Style::default().fg(theme.fg)),
            ])),
        },
    };
    frame.render_widget(footer, area);
}

// ============================================================
// Feed View
// ============================================================

fn draw_feed(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let display = &app.cached_display;

    if display.is_empty() {
        let msg = if app.articles.is_empty() {
            if app.is_fetching {
                format!("  {} Fetching news...", app.spinner_char())
            } else {
                "  No articles yet. Press [r] to refresh feeds.".to_string()
            }
        } else {
            "  No articles match current filter.".to_string()
        };
        let empty = Paragraph::new(Span::styled(msg, Style::default().fg(theme.muted)));
        frame.render_widget(empty, area);
        return;
    }

    let title = match app.view_mode {
        ViewMode::Bookmarks => " Bookmarked Articles ",
        _ => " News Feed ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(Span::styled(
            title,
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ));

    let header = Row::new(vec!["", "Source", "Time", "Title", "Tickers"])
        .style(
            Style::default()
                .fg(theme.header)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let rows: Vec<Row> = display
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let article = &app.articles[row.article_idx];
            let is_selected = i == app.selected_index;
            let sentiment_indicator = article.sentiment.label();

            let read_marker = if article.bookmarked {
                "*"
            } else if article.read {
                " "
            } else {
                "+"
            };

            let time_ago = format_time_ago(article.published_at);
            let tickers_str = if article.tickers.is_empty() {
                String::new()
            } else {
                article.tickers.join(",")
            };

            let title_text = if row.dup_count > 0 {
                format!("{} (+{})", article.title, row.dup_count)
            } else {
                article.title.clone()
            };

            let style = if is_selected {
                Style::default()
                    .fg(theme.fg)
                    .add_modifier(Modifier::BOLD)
                    .bg(ratatui::style::Color::Rgb(40, 40, 50))
            } else if !article.read {
                Style::default().fg(theme.fg)
            } else {
                Style::default().fg(theme.muted)
            };

            Row::new(vec![
                format!("{}{}", read_marker, sentiment_indicator),
                article.source.clone(),
                time_ago,
                title_text,
                tickers_str,
            ])
            .style(style)
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(14),
        Constraint::Length(8),
        Constraint::Min(20),
        Constraint::Length(16),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(
        table,
        area,
        &mut ratatui::widgets::TableState::default().with_selected(Some(app.selected_index)),
    );
}

// ============================================================
// Reader View
// ============================================================

fn draw_reader(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;

    let article = match app.selected_article() {
        Some(a) => a,
        None => {
            let empty = Paragraph::new("No article selected")
                .style(Style::default().fg(theme.muted));
            frame.render_widget(empty, area);
            return;
        }
    };

    let time_str = chrono::DateTime::from_timestamp(article.published_at, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_default();

    let sentiment_text = match article.sentiment {
        Sentiment::Positive => "Positive",
        Sentiment::Negative => "Negative",
        Sentiment::Neutral => "Neutral",
    };
    let sentiment_color = article.sentiment.color(theme);

    let bookmark_text = if article.bookmarked {
        " [Bookmarked]"
    } else {
        ""
    };

    let tickers_text = if article.tickers.is_empty() {
        "None detected".to_string()
    } else {
        article.tickers.join(", ")
    };

    // Build header lines
    let mut lines = vec![
        Line::from(Span::styled(
            &article.title,
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Source: ", Style::default().fg(theme.muted)),
            Span::styled(&article.source, Style::default().fg(theme.accent)),
            Span::styled("  ", Style::default()),
            Span::styled(&time_str, Style::default().fg(theme.muted)),
        ]),
        Line::from(vec![
            Span::styled("Sentiment: ", Style::default().fg(theme.muted)),
            Span::styled(sentiment_text, Style::default().fg(sentiment_color)),
            Span::styled(bookmark_text, Style::default().fg(theme.accent)),
        ]),
        Line::from(vec![
            Span::styled("Tickers: ", Style::default().fg(theme.muted)),
            Span::styled(tickers_text, Style::default().fg(theme.title)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "\u{2500}".repeat(60),
            Style::default().fg(theme.border),
        )),
        Line::from(""),
    ];

    // Article content
    if app.content_loading {
        lines.push(Line::from(Span::styled(
            format!("  {} Loading article content...", app.spinner_char()),
            Style::default().fg(theme.muted),
        )));
    } else if let Some(ref content) = app.reader_content {
        for line in content.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(theme.fg),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  No content loaded. Press [o] to open in browser.",
            Style::default().fg(theme.muted),
        )));
    }

    // Trailing space + URL
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "\u{2500}".repeat(60),
        Style::default().fg(theme.border),
    )));
    lines.push(Line::from(vec![
        Span::styled("  URL: ", Style::default().fg(theme.muted)),
        Span::styled(
            &article.url,
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::UNDERLINED),
        ),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_selected))
        .title(Span::styled(
            " Article ",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.reader_scroll, 0));
    frame.render_widget(paragraph, area);
}

// ============================================================
// Sources View
// ============================================================

fn draw_sources(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(Span::styled(
            " Feed Sources ",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ));

    let mut lines = vec![Line::from("")];

    for (i, source) in app.sources.iter().enumerate() {
        let check = if source.enabled { "[x]" } else { "[ ]" };
        let style = if i == app.selected_index {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        lines.push(Line::from(Span::styled(
            format!("  {} {} - {}", check, source.name, source.url),
            style,
        )));
    }

    // Source input/delete UI
    match &app.input_mode {
        InputMode::SourceAdd(field) | InputMode::SourceEdit(field) => {
            let is_add = matches!(app.input_mode, InputMode::SourceAdd(_));
            let is_name = matches!(field, crate::app::SourceInputField::Name);
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                if is_add {
                    "  -- Add New Source --"
                } else {
                    "  -- Edit Source --"
                },
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled(
                    if is_name { "  > Name: " } else { "    Name: " },
                    Style::default().fg(theme.muted),
                ),
                Span::styled(&app.source_edit_name, Style::default().fg(theme.fg)),
                if is_name {
                    Span::styled("_", Style::default().fg(theme.accent))
                } else {
                    Span::raw("")
                },
            ]));
            lines.push(Line::from(vec![
                Span::styled(
                    if !is_name { "  > URL:  " } else { "    URL:  " },
                    Style::default().fg(theme.muted),
                ),
                Span::styled(&app.source_edit_url, Style::default().fg(theme.fg)),
                if !is_name {
                    Span::styled("_", Style::default().fg(theme.accent))
                } else {
                    Span::raw("")
                },
            ]));
        }
        InputMode::SourceDelete => {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(
                    "  Delete '{}'? [y]Confirm [any]Cancel",
                    app.sources
                        .get(app.selected_index)
                        .map(|s| s.name.as_str())
                        .unwrap_or("?")
                ),
                Style::default().fg(theme.negative),
            )));
        }
        _ => {}
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

// ============================================================
// Help Overlay
// ============================================================

fn draw_help_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 70, frame.area());
    frame.render_widget(Clear, area);

    let theme = &app.theme;
    let help_text = vec![
        Line::from(Span::styled(
            " StockNewsTUI Keyboard Shortcuts ",
            Style::default()
                .fg(theme.header)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Navigation",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.accent),
        )),
        Line::from(" j/k or Up/Dn  Navigate articles"),
        Line::from(" g/G            Go to first/last"),
        Line::from(" Enter          Open article reader"),
        Line::from(" Esc            Go back"),
        Line::from(""),
        Line::from(Span::styled(
            " Actions",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.accent),
        )),
        Line::from(" o              Open in browser"),
        Line::from(" b              Toggle bookmark"),
        Line::from(" r              Refresh feeds"),
        Line::from(" /              Search (title+tickers+body)"),
        Line::from(" T              Filter by ticker"),
        Line::from(" c              Clear ticker filter"),
        Line::from(""),
        Line::from(Span::styled(
            " Reader",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.accent),
        )),
        Line::from(" j/k            Scroll up/down"),
        Line::from(" d/u            Page down/up"),
        Line::from(" n/p            Next/prev article"),
        Line::from(" g/G            Top/bottom"),
        Line::from(""),
        Line::from(Span::styled(
            " Display",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.accent),
        )),
        Line::from(" f              Cycle filter mode"),
        Line::from(" B              View bookmarks"),
        Line::from(" S              View feed sources"),
        Line::from(" t              Cycle theme"),
        Line::from(""),
        Line::from(Span::styled(
            " Sources",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.accent),
        )),
        Line::from(" a              Add new source"),
        Line::from(" e              Edit source"),
        Line::from(" d              Delete source"),
        Line::from(" Space          Toggle enable/disable"),
        Line::from(""),
        Line::from(Span::styled(
            " General",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.accent),
        )),
        Line::from(" ?              Toggle help"),
        Line::from(" q / Ctrl+C     Quit"),
        Line::from(""),
        Line::from(Span::styled(
            " Press ? to close ",
            Style::default().fg(theme.muted),
        )),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_selected))
                .title(" Help "),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(help, area);
}

// ============================================================
// Utilities
// ============================================================

fn format_time_ago(timestamp: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let diff = now - timestamp;

    if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
