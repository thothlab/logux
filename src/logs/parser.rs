//! Logcat output parser — converts raw lines into structured LogEntry.

use regex::Regex;
use std::fmt;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    Verbose = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
    Silent = 6,
}

impl LogLevel {
    pub fn from_char(c: char) -> Self {
        match c.to_ascii_uppercase() {
            'V' => Self::Verbose,
            'D' => Self::Debug,
            'I' => Self::Info,
            'W' => Self::Warn,
            'E' => Self::Error,
            'F' => Self::Fatal,
            'S' => Self::Silent,
            _ => Self::Verbose,
        }
    }

    pub fn char(&self) -> char {
        match self {
            Self::Verbose => 'V',
            Self::Debug => 'D',
            Self::Info => 'I',
            Self::Warn => 'W',
            Self::Error => 'E',
            Self::Fatal => 'F',
            Self::Silent => 'S',
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "v" | "verbose" => Some(Self::Verbose),
            "d" | "debug" => Some(Self::Debug),
            "i" | "info" => Some(Self::Info),
            "w" | "warn" | "warning" => Some(Self::Warn),
            "e" | "error" => Some(Self::Error),
            "f" | "fatal" => Some(Self::Fatal),
            "s" | "silent" => Some(Self::Silent),
            _ => None,
        }
    }

    pub fn color_code(&self) -> &'static str {
        match self {
            Self::Verbose => "37",    // white dim
            Self::Debug => "34",      // blue
            Self::Info => "32",       // green
            Self::Warn => "33",       // yellow
            Self::Error => "31",      // red
            Self::Fatal => "1;37;41", // bold white on red
            Self::Silent => "2;37",   // dim
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.char())
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub pid: u32,
    pub tid: u32,
    pub level: LogLevel,
    pub tag: String,
    pub message: String,
    pub raw: String,
}

// threadtime: "MM-DD HH:MM:SS.mmm  PID  TID LEVEL TAG: MESSAGE"
static THREADTIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\.\d{3})\s+(\d+)\s+(\d+)\s+([VDIWEFS])\s+(.+?)\s*:\s+(.*)"
    ).unwrap()
});

// brief: "LEVEL/TAG(PID): MESSAGE"
static BRIEF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([VDIWEFS])/(.+?)\(\s*(\d+)\):\s+(.*)").unwrap()
});

pub fn parse_logcat_line(line: &str) -> Option<LogEntry> {
    let line = line.trim_end();
    if line.is_empty() {
        return None;
    }
    if line.starts_with("---------") {
        return None;
    }

    // Try threadtime
    if let Some(caps) = THREADTIME_RE.captures(line) {
        return Some(LogEntry {
            timestamp: caps[1].to_string(),
            pid: caps[2].parse().unwrap_or(0),
            tid: caps[3].parse().unwrap_or(0),
            level: LogLevel::from_char(caps[4].chars().next().unwrap_or('V')),
            tag: caps[5].trim().to_string(),
            message: caps[6].to_string(),
            raw: line.to_string(),
        });
    }

    // Try brief
    if let Some(caps) = BRIEF_RE.captures(line) {
        return Some(LogEntry {
            timestamp: String::new(),
            pid: caps[3].parse().unwrap_or(0),
            tid: 0,
            level: LogLevel::from_char(caps[1].chars().next().unwrap_or('V')),
            tag: caps[2].trim().to_string(),
            message: caps[4].to_string(),
            raw: line.to_string(),
        });
    }

    // Unparseable — continuation
    Some(LogEntry {
        timestamp: String::new(),
        pid: 0,
        tid: 0,
        level: LogLevel::Verbose,
        tag: String::new(),
        message: line.to_string(),
        raw: line.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threadtime_format() {
        let line = "04-13 12:34:56.789  1234  5678 D MyTag   : Hello World";
        let entry = parse_logcat_line(line).unwrap();
        assert_eq!(entry.timestamp, "04-13 12:34:56.789");
        assert_eq!(entry.pid, 1234);
        assert_eq!(entry.tid, 5678);
        assert_eq!(entry.level, LogLevel::Debug);
        assert_eq!(entry.tag, "MyTag");
        assert_eq!(entry.message, "Hello World");
    }

    #[test]
    fn test_threadtime_error() {
        let line = "04-13 12:34:56.789  1234  5678 E CrashTag: java.lang.NullPointerException";
        let entry = parse_logcat_line(line).unwrap();
        assert_eq!(entry.level, LogLevel::Error);
        assert!(entry.message.contains("NullPointerException"));
    }

    #[test]
    fn test_brief_format() {
        let line = "D/MyTag( 1234): Some debug message";
        let entry = parse_logcat_line(line).unwrap();
        assert_eq!(entry.level, LogLevel::Debug);
        assert_eq!(entry.tag, "MyTag");
        assert_eq!(entry.pid, 1234);
    }

    #[test]
    fn test_header_skipped() {
        assert!(parse_logcat_line("--------- beginning of main").is_none());
    }

    #[test]
    fn test_empty_line() {
        assert!(parse_logcat_line("").is_none());
    }

    #[test]
    fn test_unparseable() {
        let entry = parse_logcat_line("random text").unwrap();
        assert_eq!(entry.message, "random text");
    }

    #[test]
    fn test_level_roundtrip() {
        for level in [LogLevel::Verbose, LogLevel::Debug, LogLevel::Info, LogLevel::Warn, LogLevel::Error, LogLevel::Fatal] {
            assert_eq!(LogLevel::from_char(level.char()), level);
        }
    }
}
