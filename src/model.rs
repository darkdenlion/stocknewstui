#![allow(dead_code)]

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ============================================================
// Article
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: i64,
    pub title: String,
    pub source: String,
    pub url: String,
    pub tickers: Vec<String>,
    pub published_at: i64, // unix timestamp
    pub fetched_at: i64,
    pub read: bool,
    pub bookmarked: bool,
    pub sentiment: Sentiment,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Sentiment {
    Positive,
    Negative,
    Neutral,
}

impl Sentiment {
    pub fn label(&self) -> &str {
        match self {
            Sentiment::Positive => "+",
            Sentiment::Negative => "-",
            Sentiment::Neutral => "~",
        }
    }

    pub fn color(&self, theme: &Theme) -> Color {
        match self {
            Sentiment::Positive => theme.positive,
            Sentiment::Negative => theme.negative,
            Sentiment::Neutral => theme.muted,
        }
    }
}

// ============================================================
// Feed Source
// ============================================================

#[derive(Debug, Clone)]
pub struct FeedSource {
    pub name: String,
    pub url: String,
    pub enabled: bool,
}

impl FeedSource {
    pub fn defaults() -> Vec<FeedSource> {
        vec![
            FeedSource {
                name: "Bisnis.com".to_string(),
                url: "https://www.bisnis.com/rss".to_string(),
                enabled: true,
            },
            FeedSource {
                name: "Kontan".to_string(),
                url: "https://www.kontan.co.id/rss".to_string(),
                enabled: true,
            },
            FeedSource {
                name: "CNBC Indo".to_string(),
                url: "https://www.cnbcindonesia.com/market/rss".to_string(),
                enabled: true,
            },
            FeedSource {
                name: "IDNFinancials".to_string(),
                url: "https://www.idnfinancials.com/rss".to_string(),
                enabled: true,
            },
        ]
    }
}

// ============================================================
// View / Filter
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Feed,
    Reader,
    Bookmarks,
    Sources,
}

impl ViewMode {
    pub fn label(&self) -> &str {
        match self {
            ViewMode::Feed => "Feed",
            ViewMode::Reader => "Reader",
            ViewMode::Bookmarks => "Bookmarks",
            ViewMode::Sources => "Sources",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterMode {
    All,
    Watchlist,
    Source,
    Unread,
}

impl FilterMode {
    pub fn label(&self) -> &str {
        match self {
            FilterMode::All => "All",
            FilterMode::Watchlist => "Watchlist",
            FilterMode::Source => "Source",
            FilterMode::Unread => "Unread",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            FilterMode::All => FilterMode::Watchlist,
            FilterMode::Watchlist => FilterMode::Unread,
            FilterMode::Unread => FilterMode::Source,
            FilterMode::Source => FilterMode::All,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "watchlist" => FilterMode::Watchlist,
            "unread" => FilterMode::Unread,
            "source" => FilterMode::Source,
            _ => FilterMode::All,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            FilterMode::All => "all",
            FilterMode::Watchlist => "watchlist",
            FilterMode::Unread => "unread",
            FilterMode::Source => "source",
        }
    }
}

// ============================================================
// Theme (matching stocktui)
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThemeName {
    Dark,
    Light,
    Solarized,
    Gruvbox,
}

impl ThemeName {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "light" => ThemeName::Light,
            "solarized" => ThemeName::Solarized,
            "gruvbox" => ThemeName::Gruvbox,
            _ => ThemeName::Dark,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            ThemeName::Dark => ThemeName::Light,
            ThemeName::Light => ThemeName::Solarized,
            ThemeName::Solarized => ThemeName::Gruvbox,
            ThemeName::Gruvbox => ThemeName::Dark,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            ThemeName::Dark => "Dark",
            ThemeName::Light => "Light",
            ThemeName::Solarized => "Solarized",
            ThemeName::Gruvbox => "Gruvbox",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub border: Color,
    pub border_selected: Color,
    pub title: Color,
    pub positive: Color,
    pub negative: Color,
    pub header: Color,
    pub muted: Color,
    pub accent: Color,
}

impl Theme {
    pub fn from_name(name: ThemeName) -> Self {
        match name {
            ThemeName::Dark => Theme {
                bg: Color::Reset,
                fg: Color::White,
                border: Color::DarkGray,
                border_selected: Color::Cyan,
                title: Color::Cyan,
                positive: Color::Green,
                negative: Color::Red,
                header: Color::Cyan,
                muted: Color::DarkGray,
                accent: Color::Yellow,
            },
            ThemeName::Light => Theme {
                bg: Color::Reset,
                fg: Color::Black,
                border: Color::Gray,
                border_selected: Color::Blue,
                title: Color::Blue,
                positive: Color::Green,
                negative: Color::Red,
                header: Color::Blue,
                muted: Color::Gray,
                accent: Color::Magenta,
            },
            ThemeName::Solarized => Theme {
                bg: Color::Reset,
                fg: Color::Rgb(131, 148, 150),
                border: Color::Rgb(88, 110, 117),
                border_selected: Color::Rgb(38, 139, 210),
                title: Color::Rgb(38, 139, 210),
                positive: Color::Rgb(133, 153, 0),
                negative: Color::Rgb(220, 50, 47),
                header: Color::Rgb(38, 139, 210),
                muted: Color::Rgb(88, 110, 117),
                accent: Color::Rgb(181, 137, 0),
            },
            ThemeName::Gruvbox => Theme {
                bg: Color::Reset,
                fg: Color::Rgb(235, 219, 178),
                border: Color::Rgb(146, 131, 116),
                border_selected: Color::Rgb(250, 189, 47),
                title: Color::Rgb(250, 189, 47),
                positive: Color::Rgb(184, 187, 38),
                negative: Color::Rgb(251, 73, 52),
                header: Color::Rgb(250, 189, 47),
                muted: Color::Rgb(146, 131, 116),
                accent: Color::Rgb(254, 128, 25),
            },
        }
    }
}

// ============================================================
// Sentiment Analysis (keyword-based)
// ============================================================

// ============================================================
// Title Similarity (for deduplication)
// ============================================================

pub fn normalize_title(title: &str) -> String {
    let lower = title.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
        .collect();
    let stop_words: HashSet<&str> = [
        "dan", "di", "ke", "dari", "yang", "untuk", "dengan", "ini", "itu",
        "the", "a", "an", "in", "on", "of", "to", "and", "for", "is", "at",
    ]
    .into_iter()
    .collect();
    cleaned
        .split_whitespace()
        .filter(|w| !stop_words.contains(w) && w.len() > 1)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn title_similarity(a: &str, b: &str) -> f64 {
    let norm_a = normalize_title(a);
    let norm_b = normalize_title(b);
    let words_a: HashSet<&str> = norm_a.split_whitespace().collect();
    let words_b: HashSet<&str> = norm_b.split_whitespace().collect();
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    let intersection = words_a.intersection(&words_b).count() as f64;
    let union = words_a.union(&words_b).count() as f64;
    intersection / union
}

pub fn analyze_sentiment(title: &str) -> Sentiment {
    let lower = title.to_lowercase();

    let positive_words = [
        "naik", "melonjak", "menguat", "rally", "cetak laba", "rekor",
        "surplus", "tumbuh", "positif", "optimis", "bullish",
        "melesat", "melejit", "cuan", "untung", "laba bersih",
        "beats", "record", "upgrade", "growth", "raises",
        "outperform", "buy", "overweight",
    ];

    let negative_words = [
        "turun", "anjlok", "melemah", "jatuh", "rugi", "defisit",
        "resesi", "pesimis", "bearish", "koreksi", "tekanan",
        "merosot", "ambles", "buntung", "gagal bayar",
        "misses", "downgrade", "layoffs", "slows", "cuts",
        "underperform", "sell", "underweight",
    ];

    let pos_count = positive_words.iter().filter(|w| lower.contains(*w)).count();
    let neg_count = negative_words.iter().filter(|w| lower.contains(*w)).count();

    if pos_count > neg_count {
        Sentiment::Positive
    } else if neg_count > pos_count {
        Sentiment::Negative
    } else {
        Sentiment::Neutral
    }
}
