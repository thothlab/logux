//! Log formatter — colored output with configurable fields and presets.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::parser::LogEntry;

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

const TAG_COLORS: &[&str] = &[
    "36", "35", "94", "92", "93", "95", "96", "34", "91", "33",
];

fn tag_color(tag: &str) -> &'static str {
    let mut hasher = DefaultHasher::new();
    tag.hash(&mut hasher);
    let idx = (hasher.finish() as usize) % TAG_COLORS.len();
    TAG_COLORS[idx]
}

const STACKTRACE_MARKERS: &[&str] = &["at ", "Caused by:", "java.", "kotlin.", "android."];

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

impl LogFormatter {
    pub fn format_entry(&self, entry: &LogEntry) -> String {
        if self.config.preset == Preset::Json {
            return self.format_json(entry);
        }
        self.format_colored(entry)
    }

    fn format_colored(&self, entry: &LogEntry) -> String {
        let mut out = String::with_capacity(256);
        let level_color = entry.level.color_code();

        // Timestamp
        if self.config.timestamp && !entry.timestamp.is_empty() {
            out.push_str(&format!("\x1b[2;36m{}\x1b[0m ", entry.timestamp));
        }

        // Level
        if self.config.level {
            out.push_str(&format!("\x1b[{level_color}m {} \x1b[0m ", entry.level.char()));
        }

        // PID/TID
        if self.config.pid && entry.pid > 0 {
            out.push_str(&format!("\x1b[2m{:>5}", entry.pid));
            if self.config.tid && entry.tid > 0 {
                out.push_str(&format!("/{:<5}", entry.tid));
            }
            out.push_str("\x1b[0m ");
        } else if self.config.tid && entry.tid > 0 {
            out.push_str(&format!("\x1b[2m{:>5}\x1b[0m ", entry.tid));
        }

        // Tag
        if self.config.tag && !entry.tag.is_empty() {
            let color = tag_color(&entry.tag);
            let tag_display = if entry.tag.len() > 24 {
                &entry.tag[..24]
            } else {
                &entry.tag
            };
            out.push_str(&format!("\x1b[{color}m{tag_display:<24}\x1b[0m "));
        }

        // Message
        if self.config.message {
            let msg = &entry.message;
            let is_stacktrace = STACKTRACE_MARKERS
                .iter()
                .any(|m| msg.trim_start().starts_with(m));

            if is_stacktrace {
                out.push_str(&format!("\x1b[2;3;31m{msg}\x1b[0m"));
            } else if !self.highlight_text.is_empty() {
                out.push_str(&highlight_in_message(msg, &self.highlight_text, level_color));
            } else {
                out.push_str(&format!("\x1b[{level_color}m{msg}\x1b[0m"));
            }
        }

        out
    }

    fn format_json(&self, entry: &LogEntry) -> String {
        serde_json::json!({
            "timestamp": entry.timestamp,
            "level": entry.level.char().to_string(),
            "pid": entry.pid,
            "tid": entry.tid,
            "tag": entry.tag,
            "message": entry.message,
        })
        .to_string()
    }
}

fn highlight_in_message(msg: &str, needle: &str, base_color: &str) -> String {
    let lower_msg = msg.to_lowercase();
    let lower_needle = needle.to_lowercase();
    let mut result = String::new();
    let mut pos = 0;

    while let Some(idx) = lower_msg[pos..].find(&lower_needle) {
        let abs_idx = pos + idx;
        result.push_str(&format!(
            "\x1b[{base_color}m{}\x1b[0m",
            &msg[pos..abs_idx]
        ));
        result.push_str(&format!(
            "\x1b[1;30;43m{}\x1b[0m",
            &msg[abs_idx..abs_idx + needle.len()]
        ));
        pos = abs_idx + needle.len();
    }
    if pos < msg.len() {
        result.push_str(&format!("\x1b[{base_color}m{}\x1b[0m", &msg[pos..]));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logs::parser::LogLevel;

    fn make_entry() -> LogEntry {
        LogEntry {
            timestamp: "04-13 12:00:00.000".to_string(),
            pid: 100,
            tid: 200,
            level: LogLevel::Info,
            tag: "TestTag".to_string(),
            message: "Hello World".to_string(),
            raw: String::new(),
        }
    }

    #[test]
    fn test_json_format() {
        let mut fmt = LogFormatter::default();
        fmt.config.apply_preset(Preset::Json);
        let out = fmt.format_entry(&make_entry());
        assert!(out.contains("\"tag\":\"TestTag\""));
        assert!(out.contains("\"message\":\"Hello World\""));
    }

    #[test]
    fn test_compact_has_tag_and_message() {
        let fmt = LogFormatter::default();
        let out = fmt.format_entry(&make_entry());
        assert!(out.contains("TestTag"));
        assert!(out.contains("Hello World"));
    }

    #[test]
    fn test_minimal_no_timestamp() {
        let mut fmt = LogFormatter::default();
        fmt.config.apply_preset(Preset::Minimal);
        let out = fmt.format_entry(&make_entry());
        assert!(!out.contains("12:00:00"));
        assert!(out.contains("Hello World"));
    }

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
