#![allow(dead_code)]

use crate::model::*;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub struct DisplayRow {
    pub article_idx: usize,
    pub dup_count: usize,
    pub other_sources: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    SourceAdd(SourceInputField),
    SourceEdit(SourceInputField),
    SourceDelete,
}

#[derive(Debug, PartialEq)]
pub enum SourceInputField {
    Name,
    Url,
}

pub struct SourceFetchState {
    pub last_fetch: Option<Instant>,
    pub consecutive_failures: u32,
    pub backoff_until: Option<Instant>,
}

impl SourceFetchState {
    pub fn new() -> Self {
        Self {
            last_fetch: None,
            consecutive_failures: 0,
            backoff_until: None,
        }
    }

    pub fn can_fetch(&self, min_interval: Duration) -> bool {
        if let Some(until) = self.backoff_until {
            if Instant::now() < until {
                return false;
            }
        }
        if let Some(last) = self.last_fetch {
            last.elapsed() >= min_interval
        } else {
            true
        }
    }

    pub fn record_success(&mut self) {
        self.last_fetch = Some(Instant::now());
        self.consecutive_failures = 0;
        self.backoff_until = None;
    }

    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        let backoff_secs = 60u64 * 2u64.pow(self.consecutive_failures.min(6));
        self.backoff_until = Some(Instant::now() + Duration::from_secs(backoff_secs));
        self.last_fetch = Some(Instant::now());
    }
}

pub struct App {
    // Articles
    pub articles: Vec<Article>,
    pub selected_index: usize,
    pub scroll_offset: usize,

    // Input
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub should_quit: bool,

    // View
    pub view_mode: ViewMode,
    pub filter_mode: FilterMode,
    pub theme_name: ThemeName,
    pub theme: Theme,
    pub show_help: bool,
    pub show_sources: bool,

    // Feed sources
    pub sources: Vec<FeedSource>,

    // Watchlist filter
    pub watchlist: Vec<String>,

    // Refresh
    pub refresh_interval: Duration,
    pub last_refresh: Option<Instant>,
    pub is_fetching: bool,

    // Rate limiting
    pub source_fetch_state: HashMap<String, SourceFetchState>,
    pub min_fetch_interval: Duration,

    // Stats
    pub total_articles: i64,
    pub unread_count: i64,
    pub last_fetch_results: Vec<(String, Result<usize, String>)>,

    // Status
    pub status_message: Option<(String, Instant)>,

    // Spinner
    pub tick_count: u64,

    // Search results (filtered article indices)
    pub search_query: String,

    // Reader state
    pub reader_content: Option<String>,
    pub reader_scroll: u16,
    pub content_loading: bool,

    // Content cache: url -> content
    pub content_cache: HashMap<String, String>,

    // Ticker filter (quick filter for a specific ticker)
    pub ticker_filter: Option<String>,

    // Failed content URLs (don't re-fetch)
    pub failed_content_urls: std::collections::HashSet<String>,

    // Source editing state
    pub source_edit_name: String,
    pub source_edit_url: String,
    pub source_edit_index: Option<usize>,

    // Cached display (filtered + deduplicated)
    pub cached_display: Vec<DisplayRow>,
    pub display_dirty: bool,
}

impl App {
    pub fn new(watchlist: Vec<String>, sources: Vec<FeedSource>) -> Self {
        Self {
            articles: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            should_quit: false,
            view_mode: ViewMode::Feed,
            filter_mode: FilterMode::All,
            theme_name: ThemeName::Dark,
            theme: Theme::from_name(ThemeName::Dark),
            show_help: false,
            show_sources: false,
            sources,
            watchlist,
            refresh_interval: Duration::from_secs(300),
            last_refresh: None,
            is_fetching: false,
            source_fetch_state: HashMap::new(),
            min_fetch_interval: Duration::from_secs(60),
            total_articles: 0,
            unread_count: 0,
            last_fetch_results: Vec::new(),
            status_message: None,
            tick_count: 0,
            search_query: String::new(),
            reader_content: None,
            reader_scroll: 0,
            content_loading: false,
            content_cache: HashMap::new(),
            ticker_filter: None,
            failed_content_urls: std::collections::HashSet::new(),
            source_edit_name: String::new(),
            source_edit_url: String::new(),
            source_edit_index: None,
            cached_display: Vec::new(),
            display_dirty: true,
        }
    }

    pub fn enter_reader(&mut self) {
        self.view_mode = ViewMode::Reader;
        self.reader_scroll = 0;

        // Check cache first (use display cache for correct article lookup)
        let url = self.selected_article().map(|a| a.url.clone());
        if let Some(url) = url {
            if let Some(content) = self.content_cache.get(&url) {
                self.reader_content = Some(content.clone());
                self.content_loading = false;
            } else {
                self.reader_content = None;
                self.content_loading = true;
            }
        }
    }

    pub fn cache_content(&mut self, url: String, content: String) {
        self.content_cache.insert(url, content.clone());
        self.reader_content = Some(content);
        self.content_loading = false;
    }

    pub fn set_ticker_filter(&mut self, ticker: Option<String>) {
        self.ticker_filter = ticker;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.display_dirty = true;
    }

    pub fn select_next(&mut self) {
        let len = self.cached_display.len();
        if len > 0 {
            self.selected_index = (self.selected_index + 1).min(len - 1);
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn select_first(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    pub fn select_last(&mut self) {
        let len = self.cached_display.len();
        if len > 0 {
            self.selected_index = len - 1;
        }
    }

    pub fn selected_article(&self) -> Option<&Article> {
        self.cached_display
            .get(self.selected_index)
            .and_then(|row| self.articles.get(row.article_idx))
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    pub fn status_text(&self) -> Option<&str> {
        if let Some((msg, when)) = &self.status_message {
            if when.elapsed() < Duration::from_secs(5) {
                return Some(msg.as_str());
            }
        }
        None
    }

    pub fn spinner_char(&self) -> char {
        const CHARS: &[char] = &['\u{25dc}', '\u{25dd}', '\u{25de}', '\u{25df}'];
        CHARS[(self.tick_count as usize / 2) % CHARS.len()]
    }

    pub fn cycle_theme(&mut self) {
        self.theme_name = self.theme_name.next();
        self.theme = Theme::from_name(self.theme_name);
    }

    pub fn cycle_filter(&mut self) {
        self.filter_mode = self.filter_mode.next();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.display_dirty = true;
    }

    pub fn refresh_seconds_remaining(&self) -> u64 {
        if let Some(last) = self.last_refresh {
            let elapsed = last.elapsed();
            if elapsed < self.refresh_interval {
                return (self.refresh_interval - elapsed).as_secs();
            }
        }
        0
    }

    /// Get sources eligible for fetching (respects rate limits)
    pub fn eligible_sources(&self) -> Vec<FeedSource> {
        self.sources
            .iter()
            .filter(|s| s.enabled)
            .filter(|s| {
                self.source_fetch_state
                    .get(&s.name)
                    .map(|state| state.can_fetch(self.min_fetch_interval))
                    .unwrap_or(true)
            })
            .cloned()
            .collect()
    }

    /// Recompute the cached display list (filtering + deduplication).
    /// Called once when data changes, not on every render frame.
    pub fn recompute_display(&mut self) {
        // Pre-compute search query once
        let search_lower = self.search_query.to_lowercase();
        let has_search = !self.search_query.is_empty();

        // Step 1: Filter articles to indices
        let filtered_indices: Vec<usize> = (0..self.articles.len())
            .filter(|&i| {
                let a = &self.articles[i];
                match self.filter_mode {
                    FilterMode::All | FilterMode::Source => true,
                    FilterMode::Watchlist => {
                        if self.watchlist.is_empty() {
                            true
                        } else {
                            a.tickers.iter().any(|t| self.watchlist.contains(t))
                                || self
                                    .watchlist
                                    .iter()
                                    .any(|w| a.title.to_uppercase().contains(w))
                        }
                    }
                    FilterMode::Unread => !a.read,
                }
            })
            .filter(|&i| {
                if let Some(ref ticker) = self.ticker_filter {
                    let a = &self.articles[i];
                    a.tickers.iter().any(|t| t == ticker)
                        || a.title.to_uppercase().contains(ticker.as_str())
                } else {
                    true
                }
            })
            .filter(|&i| {
                if has_search {
                    let a = &self.articles[i];
                    a.title.to_lowercase().contains(&search_lower)
                        || a.tickers
                            .iter()
                            .any(|t| t.to_lowercase().contains(&search_lower))
                        || self
                            .content_cache
                            .get(&a.url)
                            .map(|c| c.to_lowercase().contains(&search_lower))
                            .unwrap_or(false)
                } else {
                    true
                }
            })
            .collect();

        // Step 2: Deduplicate with pre-computed normalized titles
        if filtered_indices.len() <= 1 {
            self.cached_display = filtered_indices
                .into_iter()
                .map(|idx| DisplayRow {
                    article_idx: idx,
                    dup_count: 0,
                    other_sources: vec![],
                })
                .collect();
        } else {
            // Pre-compute normalized titles and word sets once
            let normalized: Vec<String> = filtered_indices
                .iter()
                .map(|&idx| normalize_title(&self.articles[idx].title))
                .collect();
            let word_sets: Vec<HashSet<&str>> = normalized
                .iter()
                .map(|n| n.split_whitespace().collect())
                .collect();

            let threshold = 0.7;
            let mut consumed = vec![false; filtered_indices.len()];
            let mut result = Vec::new();

            for i in 0..filtered_indices.len() {
                if consumed[i] {
                    continue;
                }
                let mut other_sources = Vec::new();
                for j in (i + 1)..filtered_indices.len() {
                    if consumed[j] {
                        continue;
                    }
                    if !word_sets[i].is_empty() && !word_sets[j].is_empty() {
                        let intersection =
                            word_sets[i].intersection(&word_sets[j]).count() as f64;
                        let union = word_sets[i].union(&word_sets[j]).count() as f64;
                        if union > 0.0 && (intersection / union) >= threshold {
                            other_sources
                                .push(self.articles[filtered_indices[j]].source.clone());
                            consumed[j] = true;
                        }
                    }
                }
                let dup_count = other_sources.len();
                result.push(DisplayRow {
                    article_idx: filtered_indices[i],
                    dup_count,
                    other_sources,
                });
            }

            self.cached_display = result;
        }

        // Keep selected_index in bounds
        if self.cached_display.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.cached_display.len() {
            self.selected_index = self.cached_display.len() - 1;
        }

        self.display_dirty = false;
    }

    // Source management
    pub fn start_add_source(&mut self) {
        self.input_mode = InputMode::SourceAdd(SourceInputField::Name);
        self.source_edit_name.clear();
        self.source_edit_url.clear();
        self.source_edit_index = None;
    }

    pub fn start_edit_source(&mut self) {
        if let Some(source) = self.sources.get(self.selected_index) {
            self.source_edit_name = source.name.clone();
            self.source_edit_url = source.url.clone();
            self.source_edit_index = Some(self.selected_index);
            self.input_mode = InputMode::SourceEdit(SourceInputField::Name);
        }
    }

    pub fn confirm_add_source(&mut self) {
        if !self.source_edit_name.is_empty() && !self.source_edit_url.is_empty() {
            self.sources.push(FeedSource {
                name: self.source_edit_name.clone(),
                url: self.source_edit_url.clone(),
                enabled: true,
            });
            self.set_status(format!("Added source: {}", self.source_edit_name));
        }
        self.input_mode = InputMode::Normal;
    }

    pub fn confirm_edit_source(&mut self) {
        if let Some(idx) = self.source_edit_index {
            if let Some(source) = self.sources.get_mut(idx) {
                source.name = self.source_edit_name.clone();
                source.url = self.source_edit_url.clone();
                self.set_status(format!("Updated source: {}", self.source_edit_name));
            }
        }
        self.input_mode = InputMode::Normal;
    }

    pub fn delete_source(&mut self) {
        if self.selected_index < self.sources.len() {
            let name = self.sources[self.selected_index].name.clone();
            self.sources.remove(self.selected_index);
            if self.selected_index >= self.sources.len() && self.selected_index > 0 {
                self.selected_index -= 1;
            }
            self.set_status(format!("Deleted source: {}", name));
        }
        self.input_mode = InputMode::Normal;
    }

    // View state persistence
    pub fn to_view_state(&self) -> crate::state::ViewState {
        crate::state::ViewState {
            filter_mode: Some(self.filter_mode.as_str().to_string()),
            search_query: if self.search_query.is_empty() {
                None
            } else {
                Some(self.search_query.clone())
            },
            ticker_filter: self.ticker_filter.clone(),
            theme_name: Some(self.theme_name.label().to_lowercase()),
            selected_index: Some(self.selected_index),
        }
    }

    pub fn restore_view_state(&mut self, state: &crate::state::ViewState) {
        if let Some(ref fm) = state.filter_mode {
            self.filter_mode = FilterMode::from_str(fm);
        }
        if let Some(ref q) = state.search_query {
            self.search_query = q.clone();
        }
        self.ticker_filter = state.ticker_filter.clone();
        if let Some(ref tn) = state.theme_name {
            self.theme_name = ThemeName::from_str(tn);
            self.theme = Theme::from_name(self.theme_name);
        }
        if let Some(idx) = state.selected_index {
            self.selected_index = idx;
        }
    }
}
