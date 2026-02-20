use crate::model::ThemeName;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// --- CLI Arguments ---

#[derive(Parser, Debug)]
#[command(name = "stocknewstui", about = "Indonesian Stock News Terminal")]
pub struct CliArgs {
    /// Filter news by ticker symbols (e.g., BBCA TLKM BBRI)
    pub tickers: Vec<String>,

    /// Color theme: dark, light, solarized, gruvbox
    #[arg(short, long)]
    pub theme: Option<String>,

    /// Refresh interval in seconds
    #[arg(long, default_value = "300")]
    pub refresh: u64,

    /// Path to config file
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

// --- Config File ---

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ConfigFile {
    #[serde(default)]
    pub watchlist: Vec<String>,
    #[serde(default = "default_refresh")]
    pub refresh_interval: u64,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default = "default_min_fetch")]
    pub min_fetch_interval: u64,
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SourceConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_refresh() -> u64 {
    300
}

fn default_min_fetch() -> u64 {
    60
}

fn default_true() -> bool {
    true
}

// --- Path Helpers ---

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("stocknewstui")
}

pub fn config_file_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn db_path() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("stocknewstui");
    let _ = fs::create_dir_all(&dir);
    dir.join("articles.db")
}

// --- Load Config ---

pub fn load_config(path: Option<&PathBuf>) -> ConfigFile {
    let path = path.cloned().unwrap_or_else(config_file_path);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

// --- Resolve ---

pub struct ResolvedConfig {
    pub watchlist: Vec<String>,
    pub refresh_interval: u64,
    pub min_fetch_interval: u64,
    pub theme: ThemeName,
}

pub fn resolve(args: &CliArgs, config: &ConfigFile) -> ResolvedConfig {
    let watchlist = if !args.tickers.is_empty() {
        args.tickers.iter().map(|s| s.to_uppercase()).collect()
    } else {
        config.watchlist.clone()
    };

    let refresh_interval = if args.refresh != 300 {
        args.refresh
    } else {
        config.refresh_interval
    };

    let theme_str = args
        .theme
        .as_deref()
        .or(config.theme.as_deref())
        .unwrap_or("dark");
    let theme = ThemeName::from_str(theme_str);

    ResolvedConfig {
        watchlist,
        refresh_interval,
        min_fetch_interval: config.min_fetch_interval,
        theme,
    }
}

// --- Save Sources ---

pub fn save_sources(sources: &[crate::model::FeedSource]) {
    let path = config_file_path();
    let mut cfg = load_config(None);
    cfg.sources = sources
        .iter()
        .map(|s| SourceConfig {
            name: s.name.clone(),
            url: s.url.clone(),
            enabled: s.enabled,
        })
        .collect();
    if let Ok(toml_str) = toml::to_string_pretty(&cfg) {
        let _ = fs::create_dir_all(config_dir());
        let _ = fs::write(path, toml_str);
    }
}
