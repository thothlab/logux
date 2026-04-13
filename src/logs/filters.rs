//! Log filter engine — composable filters that can be changed on the fly.

use regex::Regex;
use std::collections::HashSet;

use super::parser::{LogEntry, LogLevel};

pub struct FilterState {
    pub package: String,
    pub pids: HashSet<u32>,
    pub tags: HashSet<String>,
    pub min_level: LogLevel,
    pub text: String,
    pub regex: Option<Regex>,
    pub threads: HashSet<u32>,
    pub package_tracking: bool,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            package: String::new(),
            pids: HashSet::new(),
            tags: HashSet::new(),
            min_level: LogLevel::Verbose,
            text: String::new(),
            regex: None,
            threads: HashSet::new(),
            package_tracking: false,
        }
    }
}

impl FilterState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_package(&mut self, package: &str, pid: Option<u32>) {
        self.package = package.to_string();
        self.pids.clear();
        if let Some(p) = pid {
            self.pids.insert(p);
        }
        self.package_tracking = !package.is_empty();
    }

    pub fn update_pid(&mut self, pid: u32) {
        self.pids.clear();
        self.pids.insert(pid);
    }

    pub fn add_tag(&mut self, tag: &str) {
        self.tags.insert(tag.to_string());
    }

    pub fn set_level(&mut self, level: LogLevel) {
        self.min_level = level;
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
    }

    pub fn set_regex(&mut self, pattern: &str) -> Result<(), regex::Error> {
        let re = Regex::new(&format!("(?i){pattern}"))?;
        self.regex = Some(re);
        Ok(())
    }

    pub fn set_pid(&mut self, pid: u32) {
        self.package_tracking = false;
        self.pids.clear();
        self.pids.insert(pid);
    }

    pub fn set_threads(&mut self, tids: HashSet<u32>) {
        self.threads = tids;
    }

    pub fn description(&self) -> String {
        let mut parts = Vec::new();
        if !self.package.is_empty() {
            parts.push(format!("app={}", self.package));
        }
        if !self.pids.is_empty() {
            let pids: Vec<_> = self.pids.iter().map(|p| p.to_string()).collect();
            parts.push(format!("pid={}", pids.join(",")));
        }
        if !self.tags.is_empty() {
            let tags: Vec<_> = self.tags.iter().cloned().collect();
            parts.push(format!("tag={}", tags.join(",")));
        }
        if self.min_level > LogLevel::Verbose {
            parts.push(format!("level>={}", self.min_level.char()));
        }
        if !self.text.is_empty() {
            parts.push(format!("text='{}'", self.text));
        }
        if let Some(ref re) = self.regex {
            parts.push(format!("regex='{}'", re.as_str()));
        }
        if !self.threads.is_empty() {
            let tids: Vec<_> = self.threads.iter().map(|t| t.to_string()).collect();
            parts.push(format!("thread={}", tids.join(",")));
        }
        if parts.is_empty() {
            "no filters".to_string()
        } else {
            parts.join(" | ")
        }
    }
}

pub fn matches(entry: &LogEntry, state: &FilterState) -> bool {
    // Level
    if entry.level < state.min_level {
        return false;
    }

    // PID
    if !state.pids.is_empty() && !state.pids.contains(&entry.pid) {
        return false;
    }

    // Tag
    if !state.tags.is_empty() {
        let tag_lower = entry.tag.to_lowercase();
        if !state.tags.iter().any(|t| tag_lower.contains(&t.to_lowercase())) {
            return false;
        }
    }

    // Text (case-insensitive)
    if !state.text.is_empty() {
        let haystack = format!("{} {}", entry.tag, entry.message).to_lowercase();
        if !haystack.contains(&state.text.to_lowercase()) {
            return false;
        }
    }

    // Regex
    if let Some(ref re) = state.regex {
        let haystack = format!("{} {}", entry.tag, entry.message);
        if !re.is_match(&haystack) {
            return false;
        }
    }

    // Thread
    if !state.threads.is_empty() && !state.threads.contains(&entry.tid) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(pid: u32, tid: u32, level: LogLevel, tag: &str, message: &str) -> LogEntry {
        LogEntry {
            timestamp: "04-13 12:00:00.000".to_string(),
            pid,
            tid,
            level,
            tag: tag.to_string(),
            message: message.to_string(),
            raw: String::new(),
        }
    }

    #[test]
    fn test_no_filters() {
        let state = FilterState::default();
        assert!(matches(&make_entry(1, 1, LogLevel::Debug, "T", "m"), &state));
    }

    #[test]
    fn test_level_filter() {
        let mut state = FilterState::default();
        state.set_level(LogLevel::Warn);
        assert!(!matches(&make_entry(1, 1, LogLevel::Debug, "T", "m"), &state));
        assert!(matches(&make_entry(1, 1, LogLevel::Warn, "T", "m"), &state));
        assert!(matches(&make_entry(1, 1, LogLevel::Error, "T", "m"), &state));
    }

    #[test]
    fn test_pid_filter() {
        let mut state = FilterState::default();
        state.set_pid(100);
        assert!(matches(&make_entry(100, 1, LogLevel::Debug, "T", "m"), &state));
        assert!(!matches(&make_entry(200, 1, LogLevel::Debug, "T", "m"), &state));
    }

    #[test]
    fn test_tag_filter() {
        let mut state = FilterState::default();
        state.add_tag("Test");
        assert!(matches(&make_entry(1, 1, LogLevel::Debug, "TestTag", "m"), &state));
        assert!(!matches(&make_entry(1, 1, LogLevel::Debug, "Other", "m"), &state));
    }

    #[test]
    fn test_text_filter() {
        let mut state = FilterState::default();
        state.set_text("hello");
        assert!(matches(&make_entry(1, 1, LogLevel::Debug, "T", "Hello World"), &state));
        assert!(!matches(&make_entry(1, 1, LogLevel::Debug, "T", "Goodbye"), &state));
    }

    #[test]
    fn test_regex_filter() {
        let mut state = FilterState::default();
        state.set_regex(r"error\d+").unwrap();
        assert!(matches(&make_entry(1, 1, LogLevel::Debug, "T", "found error123"), &state));
        assert!(!matches(&make_entry(1, 1, LogLevel::Debug, "T", "no match"), &state));
    }

    #[test]
    fn test_combined() {
        let mut state = FilterState::default();
        state.set_level(LogLevel::Info);
        state.add_tag("Net");
        state.set_text("request");
        assert!(matches(&make_entry(1, 1, LogLevel::Info, "Network", "HTTP request"), &state));
        assert!(!matches(&make_entry(1, 1, LogLevel::Debug, "Network", "HTTP request"), &state));
    }

    #[test]
    fn test_package_tracking() {
        let mut state = FilterState::default();
        state.set_package("com.example", Some(100));
        assert!(state.package_tracking);
        assert!(matches(&make_entry(100, 1, LogLevel::Debug, "T", "m"), &state));
        assert!(!matches(&make_entry(200, 1, LogLevel::Debug, "T", "m"), &state));

        state.update_pid(300);
        assert!(matches(&make_entry(300, 1, LogLevel::Debug, "T", "m"), &state));
        assert!(!matches(&make_entry(100, 1, LogLevel::Debug, "T", "m"), &state));
    }

    #[test]
    fn test_reset() {
        let mut state = FilterState::default();
        state.set_pid(100);
        state.add_tag("X");
        state.set_level(LogLevel::Error);
        state.reset();
        assert!(matches(&make_entry(1, 1, LogLevel::Verbose, "T", "m"), &state));
    }
}
