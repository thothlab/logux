//! Field/preset configuration for log output.
//!
//! The actual rendering of log entries lives in `cli::tui::render_entry`
//! (two-line layout: header row + indented message). This module only
//! holds the shared `FormatConfig` that controls which fields are shown
//! and their widths.

use serde::{Deserialize, Serialize};

/// How log entries are laid out on screen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    /// Two-line layout: metadata header + indented message below (default).
    #[default]
    Linear,
    /// Single-line layout: all fields in fixed-width columns, message truncated.
    Compact,
}

impl LayoutMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Compact => "compact",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "linear" => Some(Self::Linear),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Compact,
    Threadtime,
    Verbose,
    Minimal,
    Json,
}

impl Preset {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "compact" => Some(Self::Compact),
            "threadtime" => Some(Self::Threadtime),
            "verbose" => Some(Self::Verbose),
            "minimal" => Some(Self::Minimal),
            "json" => Some(Self::Json),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Threadtime => "threadtime",
            Self::Verbose => "verbose",
            Self::Minimal => "minimal",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnWidths {
    pub timestamp: u16,
    pub level: u16,
    pub pid: u16,
    pub tid: u16,
    pub tag: u16,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        Self {
            timestamp: 20,
            level: 4,
            pid: 7,
            tid: 7,
            tag: 25,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatConfig {
    pub timestamp: bool,
    pub level: bool,
    pub tag: bool,
    pub pid: bool,
    pub tid: bool,
    pub message: bool,
    pub preset: Preset,
    #[serde(default)]
    pub widths: ColumnWidths,
    #[serde(default)]
    pub layout_mode: LayoutMode,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            timestamp: true,
            level: true,
            tag: true,
            pid: false,
            tid: false,
            message: true,
            preset: Preset::Compact,
            widths: ColumnWidths::default(),
            layout_mode: LayoutMode::default(),
        }
    }
}

impl FormatConfig {
    pub fn apply_preset(&mut self, preset: Preset) {
        self.preset = preset;
        match preset {
            Preset::Compact => {
                self.timestamp = true;
                self.level = true;
                self.tag = true;
                self.pid = false;
                self.tid = false;
                self.message = true;
            }
            Preset::Threadtime | Preset::Verbose => {
                self.timestamp = true;
                self.level = true;
                self.tag = true;
                self.pid = true;
                self.tid = true;
                self.message = true;
            }
            Preset::Minimal => {
                self.timestamp = false;
                self.level = true;
                self.tag = true;
                self.pid = false;
                self.tid = false;
                self.message = true;
            }
            Preset::Json => {}
        }
    }

    pub fn toggle_field(&mut self, name: &str, enabled: bool) -> bool {
        match name {
            "timestamp" => self.timestamp = enabled,
            "level" => self.level = enabled,
            "tag" => self.tag = enabled,
            "pid" => self.pid = enabled,
            "tid" => self.tid = enabled,
            "message" => self.message = enabled,
            _ => return false,
        }
        true
    }
}

pub struct LogFormatter {
    pub config: FormatConfig,
    pub highlight_text: String,
}

impl Default for LogFormatter {
    fn default() -> Self {
        Self {
            config: FormatConfig::default(),
            highlight_text: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_field() {
        let mut cfg = FormatConfig::default();
        assert!(cfg.toggle_field("pid", true));
        assert!(cfg.pid);
        assert!(!cfg.toggle_field("nonexistent", true));
    }

    #[test]
    fn test_preset_roundtrip() {
        for p in [Preset::Compact, Preset::Threadtime, Preset::Verbose, Preset::Minimal, Preset::Json] {
            assert_eq!(Preset::from_name(p.as_str()), Some(p));
        }
    }
}
