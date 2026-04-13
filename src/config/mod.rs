//! Preset management — save/load filter+format configurations.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::logs::filters::FilterState;
use crate::logs::formatter::FormatConfig;
use crate::logs::parser::LogLevel;

#[derive(Serialize, Deserialize)]
struct PresetData {
    name: String,
    filters: PresetFilters,
    format: FormatConfig,
}

#[derive(Serialize, Deserialize)]
struct PresetFilters {
    package: String,
    tags: Vec<String>,
    min_level: u8,
    text: String,
    regex: Option<String>,
}

fn presets_dir() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".logux")
        .join("presets");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn save_preset(name: &str, filters: &FilterState, format_config: &FormatConfig) -> Result<PathBuf, String> {
    let data = PresetData {
        name: name.to_string(),
        filters: PresetFilters {
            package: filters.package.clone(),
            tags: filters.tags.iter().cloned().collect(),
            min_level: filters.min_level as u8,
            text: filters.text.clone(),
            regex: filters.regex.as_ref().map(|r| r.as_str().to_string()),
        },
        format: format_config.clone(),
    };

    let path = presets_dir().join(format!("{name}.json"));
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn load_preset(name: &str, filters: &mut FilterState, format_config: &mut FormatConfig) -> Result<(), String> {
    let path = presets_dir().join(format!("{name}.json"));
    let json = fs::read_to_string(&path).map_err(|e| format!("Preset not found: {e}"))?;
    let data: PresetData = serde_json::from_str(&json).map_err(|e| e.to_string())?;

    filters.package = data.filters.package;
    filters.tags = data.filters.tags.into_iter().collect();
    filters.min_level = match data.filters.min_level {
        0 => LogLevel::Verbose,
        1 => LogLevel::Debug,
        2 => LogLevel::Info,
        3 => LogLevel::Warn,
        4 => LogLevel::Error,
        5 => LogLevel::Fatal,
        _ => LogLevel::Verbose,
    };
    filters.text = data.filters.text;
    if let Some(pattern) = data.filters.regex {
        let _ = filters.set_regex(&pattern);
    } else {
        filters.regex = None;
    }

    *format_config = data.format;
    Ok(())
}

pub fn list_presets() -> Vec<String> {
    let dir = presets_dir();
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.path().file_stem() {
                if entry.path().extension().is_some_and(|e| e == "json") {
                    names.push(name.to_string_lossy().to_string());
                }
            }
        }
    }
    names.sort();
    names
}

pub fn delete_preset(name: &str) -> bool {
    let path = presets_dir().join(format!("{name}.json"));
    fs::remove_file(path).is_ok()
}

// --- App history ---

fn app_history_file() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".logux");
    let _ = fs::create_dir_all(&dir);
    dir.join("app_history.json")
}

pub fn load_app_history() -> Vec<String> {
    let path = app_history_file();
    if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    }
}

pub fn save_app_to_history(package: &str) {
    let mut history = load_app_history();
    history.retain(|p| p != package);
    history.insert(0, package.to_string());
    if history.len() > 50 {
        history.truncate(50);
    }
    let path = app_history_file();
    let _ = fs::write(path, serde_json::to_string_pretty(&history).unwrap_or_default());
}

// --- Filter history per app ---

fn filter_history_file() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".logux");
    let _ = fs::create_dir_all(&dir);
    dir.join("filter_history.json")
}

pub fn load_filter_history(app_package: &str) -> Vec<String> {
    let path = filter_history_file();
    if let Ok(content) = fs::read_to_string(&path) {
        let map: std::collections::HashMap<String, Vec<String>> =
            serde_json::from_str(&content).unwrap_or_default();
        map.get(app_package).cloned().unwrap_or_default()
    } else {
        Vec::new()
    }
}

// --- Filter presets (auto-saved editable filter strings) ---

fn filter_presets_file() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".logux");
    let _ = fs::create_dir_all(&dir);
    dir.join("filter_presets.json")
}

/// Returns list of (name, edit_string) pairs, most recent first.
pub fn list_filter_presets() -> Vec<(String, String)> {
    let path = filter_presets_file();
    if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// Auto-save a filter expression. Uses the expression itself as a key,
/// deduplicates, and keeps the 20 most recent.
pub fn save_filter_preset(edit_string: &str) {
    if edit_string.trim().is_empty() {
        return;
    }
    let path = filter_presets_file();
    let mut presets: Vec<(String, String)> = if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Build a short name from the filter keys
    let name = edit_string
        .split_whitespace()
        .filter_map(|t| t.split_once('=').map(|(k, _)| k))
        .collect::<Vec<_>>()
        .join("+");
    let name = if name.is_empty() { "filter".to_string() } else { name };

    // Remove duplicates by expression
    presets.retain(|(_, expr)| expr != edit_string);
    presets.insert(0, (name, edit_string.to_string()));
    if presets.len() > 20 {
        presets.truncate(20);
    }

    let _ = fs::write(&path, serde_json::to_string_pretty(&presets).unwrap_or_default());
}

// --- Per-app last filter state ---

fn app_filters_file() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".logux");
    let _ = fs::create_dir_all(&dir);
    dir.join("app_filters.json")
}

/// Save the current filter edit string for a specific app package.
pub fn save_app_filters(package: &str, edit_string: &str) {
    if package.is_empty() {
        return;
    }
    let path = app_filters_file();
    let mut map: std::collections::HashMap<String, String> =
        if let Ok(content) = fs::read_to_string(&path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        };
    if edit_string.trim().is_empty() {
        map.remove(package);
    } else {
        map.insert(package.to_string(), edit_string.to_string());
    }
    let _ = fs::write(&path, serde_json::to_string_pretty(&map).unwrap_or_default());
}

/// Load the last saved filter edit string for an app package.
pub fn load_app_filters(package: &str) -> Option<String> {
    let path = app_filters_file();
    let content = fs::read_to_string(&path).ok()?;
    let map: std::collections::HashMap<String, String> =
        serde_json::from_str(&content).ok()?;
    map.get(package).cloned().filter(|s| !s.trim().is_empty())
}

pub fn save_filter_to_history(app_package: &str, preset_name: &str) {
    let path = filter_history_file();
    let mut map: std::collections::HashMap<String, Vec<String>> = if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let entry = map.entry(app_package.to_string()).or_default();
    entry.retain(|p| p != preset_name);
    entry.insert(0, preset_name.to_string());
    if entry.len() > 20 {
        entry.truncate(20);
    }

    let _ = fs::write(&path, serde_json::to_string_pretty(&map).unwrap_or_default());
}
