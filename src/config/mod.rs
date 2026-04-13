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
