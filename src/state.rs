use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ViewState {
    pub filter_mode: Option<String>,
    pub search_query: Option<String>,
    pub ticker_filter: Option<String>,
    pub theme_name: Option<String>,
    pub selected_index: Option<usize>,
}

fn state_path() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("stocknewstui");
    let _ = fs::create_dir_all(&dir);
    dir.join("state.json")
}

pub fn load_state() -> ViewState {
    let path = state_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_state(state: &ViewState) {
    let path = state_path();
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::write(path, json);
    }
}
