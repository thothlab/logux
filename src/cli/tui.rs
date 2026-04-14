//! TUI — terminal user interface with split log/input layout.
//!
//! Logs are rendered as a table with fixed-width columns.
//! Long messages wrap inside the message column only.
//! The command prompt stays fixed at the bottom.

use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::io::{self, Write as IoWrite};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind, EnableMouseCapture, DisableMouseCapture};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::prelude::*;
use ratatui::widgets::*;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use crate::adb::AdbClient;
use crate::logs::filters::{self, FilterState};
use crate::logs::formatter::{FormatConfig, Preset};
use crate::logs::parser::{LogLevel, parse_logcat_line};
use crate::mock::MockEngine;
use crate::traffic::TrafficProxy;

use super::commands::{dispatch, CommandContext};
use super::completer;

const MAX_LOG_LINES: usize = 10_000;

const BANNER: &str = r#" ╦  ╔═╗╔═╗╦ ╦═╗ ╦
 ║  ║ ║║ ╦║ ║╔╩╦╝
 ╩═╝╚═╝╚═╝╚═╝╩ ╚═  v2.0"#;

const STACKTRACE_MARKERS: &[&str] = &["at ", "Caused by:", "java.", "kotlin.", "android."];

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct LogEntryData {
    timestamp: String,
    level: LogLevel,
    pid: u32,
    tid: u32,
    tag: String,
    message: String,
}

#[derive(Clone)]
enum LogLine {
    System(String),      // ANSI-formatted system / command output
    Entry(LogEntryData), // Structured log entry → rendered as table row
}

/// Column widths for the table layout.
struct ColumnLayout {
    ts_w: usize,
    level_w: usize,
    pid_w: usize,
    tid_w: usize,
    tag_w: usize,
    prefix_w: usize, // sum of above
    msg_w: usize,    // remaining
}

fn compute_layout(cfg: &FormatConfig, total_w: u16) -> ColumnLayout {
    let ts_w = if cfg.timestamp { 20 } else { 0 };
    let level_w = if cfg.level { 4 } else { 0 };
    let pid_w = if cfg.pid { 7 } else { 0 };
    let tid_w = if cfg.tid { 7 } else { 0 };
    let tag_w = if cfg.tag { 25 } else { 0 };
    let prefix_w = ts_w + level_w + pid_w + tid_w + tag_w;
    let msg_w = (total_w as usize).saturating_sub(prefix_w).max(10);
    ColumnLayout {
        ts_w,
        level_w,
        pid_w,
        tid_w,
        tag_w,
        prefix_w,
        msg_w,
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct App {
    all_lines: VecDeque<LogLine>,  // unfiltered: all entries + system messages
    log_lines: VecDeque<LogLine>,  // filtered view for display
    scroll_offset: usize,
    auto_scroll: bool,

    input: String,
    cursor_pos: usize,

    history: Vec<String>,
    history_idx: Option<usize>,
    history_saved_input: String,

    suggestions: Vec<String>,
    suggestion_idx: Option<usize>,
    show_suggestions: bool,

    adb: AdbClient,
    filters: FilterState,
    formatter: crate::logs::formatter::LogFormatter,
    traffic: TrafficProxy,
    mock_engine: MockEngine,
    streaming: bool,
    paused: bool,
    save_path: Option<String>,

    stream_stop: Option<Arc<AtomicBool>>,

    app_history: Vec<String>,

    should_exit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            all_lines: VecDeque::with_capacity(MAX_LOG_LINES),
            log_lines: VecDeque::with_capacity(MAX_LOG_LINES),
            scroll_offset: 0,
            auto_scroll: true,

            input: String::new(),
            cursor_pos: 0,

            history: Vec::new(),
            history_idx: None,
            history_saved_input: String::new(),

            suggestions: Vec::new(),
            suggestion_idx: None,
            show_suggestions: false,

            adb: AdbClient::new(),
            filters: FilterState::default(),
            formatter: crate::logs::formatter::LogFormatter::default(),
            traffic: TrafficProxy::new(8888),
            mock_engine: MockEngine::new(),
            streaming: false,
            paused: false,
            save_path: None,

            stream_stop: None,

            app_history: crate::config::load_app_history(),

            should_exit: false,
        }
    }

    fn push_system(&mut self, msg: String) {
        let line = LogLine::System(msg);
        self.all_lines.push_back(line.clone());
        self.log_lines.push_back(line);
        self.trim_buffer();
        self.auto_scroll_to_end();
    }

    fn push_entry(&mut self, entry: LogEntryData) {
        let line = LogLine::Entry(entry);
        self.all_lines.push_back(line.clone());
        // When paused, only buffer — don't update display
        if !self.paused {
            if self.entry_passes_filter(&line) {
                self.log_lines.push_back(line);
            }
            self.trim_buffer();
            self.auto_scroll_to_end();
        } else {
            // Still trim all_lines to prevent unbounded growth
            while self.all_lines.len() > MAX_LOG_LINES {
                self.all_lines.pop_front();
            }
        }
    }

    fn entry_passes_filter(&self, line: &LogLine) -> bool {
        match line {
            LogLine::System(_) => true,
            LogLine::Entry(e) => {
                let entry = crate::logs::parser::LogEntry {
                    timestamp: e.timestamp.clone(),
                    pid: e.pid,
                    tid: e.tid,
                    level: e.level,
                    tag: e.tag.clone(),
                    message: e.message.clone(),
                    raw: String::new(),
                };
                filters::matches(&entry, &self.filters)
            }
        }
    }

    fn trim_buffer(&mut self) {
        while self.all_lines.len() > MAX_LOG_LINES {
            self.all_lines.pop_front();
        }
        while self.log_lines.len() > MAX_LOG_LINES {
            self.log_lines.pop_front();
            if !self.auto_scroll && self.scroll_offset > 0 {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
        }
    }

    fn auto_scroll_to_end(&mut self) {
        if self.auto_scroll {
            self.scroll_offset = 0;
        }
    }

    /// Re-filter all_lines into log_lines using current filters.
    fn rebuild_filtered(&mut self) {
        self.log_lines.clear();
        for line in &self.all_lines {
            if self.entry_passes_filter(line) {
                self.log_lines.push_back(line.clone());
            }
        }
        if self.auto_scroll {
            self.scroll_offset = 0;
        }
    }

    /// Resume from pause: rebuild display to include entries buffered while paused.
    fn resume(&mut self) {
        self.paused = false;
        self.rebuild_filtered();
    }

    fn is_stream_running(&self) -> bool {
        self.stream_stop
            .as_ref()
            .is_some_and(|f| !f.load(Ordering::Relaxed))
    }

    fn stop_stream(&mut self) {
        if let Some(ref flag) = self.stream_stop {
            flag.store(true, Ordering::Relaxed);
        }
        self.stream_stop = None;
    }

    fn update_suggestions(&mut self) {
        if self.input.is_empty() || !self.input.starts_with('/') {
            self.suggestions.clear();
            self.show_suggestions = false;
            return;
        }
        let fg = if self.input.starts_with("/app") {
            self.adb.get_foreground_package()
        } else {
            None
        };
        let suggestions = completer::complete(
            &self.input,
            &self.app_history,
            fg.as_deref(),
            &self.filters.package,
        );
        self.suggestions = suggestions;
        self.show_suggestions = !self.suggestions.is_empty();
        self.suggestion_idx = None;
    }

    fn apply_suggestion(&mut self) {
        let text = if let Some(idx) = self.suggestion_idx {
            self.suggestions.get(idx).cloned()
        } else if self.suggestions.len() == 1 {
            Some(self.suggestions[0].clone())
        } else {
            None
        };
        if let Some(t) = text {
            self.input = if t.ends_with(' ') { t } else { format!("{t} ") };
            self.cursor_pos = self.input.len();
        }
        self.show_suggestions = false;
        self.suggestions.clear();
        self.suggestion_idx = None;
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub async fn run() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        original_hook(info);
    }));

    terminal::enable_raw_mode().expect("Failed to enable raw mode");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).expect("Failed to enter alternate screen");
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

    let mut app = App::new();

    // Banner
    for line in BANNER.lines() {
        app.push_system(format!("\x1b[1;36m{line}\x1b[0m"));
    }
    app.push_system("\x1b[2mType /help for commands, /exit to quit\x1b[0m".into());
    app.push_system(String::new());

    let (ok, version) = app.adb.check_adb();
    if ok {
        app.push_system(format!("\x1b[32mADB: {version}\x1b[0m"));
    } else {
        app.push_system(format!("\x1b[31mADB: {version}\x1b[0m"));
    }

    startup_devices(&mut app);

    // Load command history
    let history_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".logux");
    let _ = std::fs::create_dir_all(&history_dir);
    let history_file = history_dir.join("history");
    if let Ok(content) = std::fs::read_to_string(&history_file) {
        app.history = content.lines().map(|s| s.to_string()).collect();
    }

    // Channel for structured log entries
    let (log_tx, mut log_rx) = mpsc::unbounded_channel::<LogEntryData>();

    // Main event loop
    loop {
        if app.streaming && !app.is_stream_running() {
            start_log_stream(&mut app, log_tx.clone());
        }

        while let Ok(entry) = log_rx.try_recv() {
            app.push_entry(entry);
        }

        let _ = terminal.draw(|frame| render_ui(frame, &app));

        if event::poll(Duration::from_millis(33)).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) => {
                    if key.kind == KeyEventKind::Press {
                        handle_key_event(key, &mut app).await;
                    }
                }
                Ok(Event::Mouse(mouse)) => {
                    handle_mouse_event(mouse.kind, &mut app);
                }
                _ => {}
            }
        }

        if app.should_exit {
            break;
        }
    }

    // Cleanup
    app.stop_stream();
    let _ = app.traffic.stop().await;

    if let Ok(mut f) = std::fs::File::create(&history_file) {
        for line in app.history.iter().rev().take(1000).rev() {
            let _ = writeln!(f, "{}", line);
        }
    }

    let _ = terminal::disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), DisableMouseCapture, LeaveAlternateScreen);
    let _ = terminal.show_cursor();
    println!("\x1b[2mBye!\x1b[0m");
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

fn startup_devices(app: &mut App) {
    let devices = app.adb.list_devices().to_vec();
    let online: Vec<_> = devices.iter().filter(|d| d.is_online()).cloned().collect();
    let total = devices.len();

    if total == 0 {
        app.push_system("\x1b[33mNo devices connected\x1b[0m".into());
        app.push_system(String::new());
        return;
    }

    let online_count = online.len();
    app.push_system(format!(
        "\x1b[2mDevices: {online_count} online / {total} total\x1b[0m"
    ));

    if online_count == 1 {
        let dev = online[0].clone();
        let name = dev.display_name();
        app.adb.selected_device = Some(dev);
        app.push_system(format!("\x1b[32mAuto-selected: {name}\x1b[0m"));
    } else if online_count > 1 {
        app.push_system("\x1b[36mMultiple devices found:\x1b[0m".into());
        for (i, dev) in online.iter().enumerate() {
            let name = dev.display_name();
            app.push_system(format!("  \x1b[33m{}\x1b[0m  {name}", i + 1));
        }
        app.push_system(
            "\x1b[2mType device number to select, or use /connect\x1b[0m".into(),
        );
    }
    app.push_system(String::new());
}

// ---------------------------------------------------------------------------
// Log streaming
// ---------------------------------------------------------------------------

fn start_log_stream(app: &mut App, tx: mpsc::UnboundedSender<LogEntryData>) {
    app.stop_stream();

    let stop = Arc::new(AtomicBool::new(false));
    app.stream_stop = Some(stop.clone());

    let save = app.save_path.clone();

    if let Ok(child) = app.adb.start_logcat(false) {
        tokio::spawn(async move {
            stream_logs(child, stop, save, tx).await;
        });
    }
}

async fn stream_logs(
    mut child: tokio::process::Child,
    stop: Arc<AtomicBool>,
    save_path: Option<String>,
    tx: mpsc::UnboundedSender<LogEntryData>,
) {
    let stdout = match child.stdout.take() {
        Some(out) => out,
        None => return,
    };

    let mut reader = BufReader::new(stdout).lines();

    let mut save_file = save_path.and_then(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .ok()
    });

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        let line_result = tokio::select! {
            r = reader.next_line() => r,
            _ = tokio::time::sleep(Duration::from_millis(200)) => continue,
        };

        match line_result {
            Ok(Some(line)) => {
                if let Some(entry) = parse_logcat_line(&line) {
                    let data = LogEntryData {
                        timestamp: entry.timestamp.clone(),
                        level: entry.level,
                        pid: entry.pid,
                        tid: entry.tid,
                        tag: entry.tag.clone(),
                        message: entry.message.clone(),
                    };
                    if tx.send(data).is_err() {
                        break;
                    }
                    if let Some(ref mut f) = save_file {
                        let _ = writeln!(f, "{}", entry.raw);
                    }
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    let _ = child.kill().await;
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_ui(frame: &mut Frame, app: &App) {
    let size = frame.area();
    if size.height < 4 || size.width < 20 {
        return;
    }

    let suggestion_h = if app.show_suggestions {
        (app.suggestions.len() as u16).min(8).max(1)
    } else {
        0
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(suggestion_h),
            Constraint::Length(1), // separator
            Constraint::Length(1), // status bar
            Constraint::Length(1), // separator
            Constraint::Length(1), // input
        ])
        .split(size);

    render_logs(frame, app, chunks[0]);
    if app.show_suggestions && suggestion_h > 0 {
        render_suggestions(frame, app, chunks[1]);
    }
    render_status_bar(frame, app, chunks[3]);
    render_input(frame, app, chunks[5]);
}

fn render_logs(frame: &mut Frame, app: &App, area: Rect) {
    let height = area.height as usize;
    if height == 0 {
        return;
    }

    // Scroll indicator block
    let block = if app.scroll_offset > 0 {
        Block::default().title(
            Line::from(format!(" SCROLL +{} ", app.scroll_offset))
                .right_aligned()
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::DIM),
                ),
        )
    } else {
        Block::default()
    };
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let inner_h = inner.height as usize;
    if inner_h == 0 {
        return;
    }

    let layout = compute_layout(&app.formatter.config, inner.width);
    let total = app.log_lines.len();
    let end = total.saturating_sub(app.scroll_offset);

    // Build visual lines backwards from `end`
    let mut visual_rev: Vec<Line<'static>> = Vec::new();
    let mut idx = end;

    while idx > 0 && visual_rev.len() < inner_h * 2 {
        idx -= 1;
        let entry_lines = match &app.log_lines[idx] {
            LogLine::System(s) => vec![parse_ansi_line(s)],
            LogLine::Entry(e) => render_entry(
                e,
                &layout,
                &app.formatter.config,
                &app.formatter.highlight_text,
            ),
        };
        for line in entry_lines.into_iter().rev() {
            visual_rev.push(line);
        }
    }

    visual_rev.reverse();

    // Take last inner_h lines
    let start = visual_rev.len().saturating_sub(inner_h);
    let visible = &visual_rev[start..];

    // Pad empty lines at top
    let pad = inner_h.saturating_sub(visible.len());
    let mut display: Vec<Line> = vec![Line::from(""); pad];
    display.extend(visible.iter().cloned());

    let paragraph = Paragraph::new(Text::from(display));
    frame.render_widget(paragraph, inner);
}

/// Render one log entry as 1+ visual lines (first line has columns, rest are indented continuation).
fn render_entry<'a>(
    entry: &LogEntryData,
    layout: &ColumnLayout,
    cfg: &FormatConfig,
    highlight: &str,
) -> Vec<Line<'a>> {
    // JSON preset — single line, no columnar layout
    if cfg.preset == Preset::Json {
        let json = format!(
            "{{\"timestamp\":\"{}\",\"level\":\"{}\",\"pid\":{},\"tid\":{},\"tag\":\"{}\",\"message\":\"{}\"}}",
            entry.timestamp,
            entry.level.char(),
            entry.pid,
            entry.tid,
            entry.tag.replace('"', "\\\""),
            entry.message.replace('"', "\\\"").replace('\n', "\\n"),
        );
        return vec![Line::from(Span::raw(json))];
    }

    // Build prefix spans (all columns except message)
    let mut prefix: Vec<Span<'a>> = Vec::new();

    if cfg.timestamp && layout.ts_w > 0 {
        let w = layout.ts_w - 1;
        let ts = if entry.timestamp.len() > w {
            entry.timestamp[..w].to_string()
        } else {
            format!("{:<w$}", entry.timestamp)
        };
        prefix.push(Span::styled(
            ts,
            Style::default().fg(Color::Gray),
        ));
        prefix.push(Span::raw(" "));
    }

    if cfg.level && layout.level_w > 0 {
        let ch = entry.level.char();
        prefix.push(Span::styled(
            format!(" {ch} "),
            level_style(entry.level),
        ));
    }

    if cfg.pid && layout.pid_w > 0 {
        prefix.push(Span::styled(
            format!("{:>5}  ", entry.pid),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }

    if cfg.tid && layout.tid_w > 0 {
        prefix.push(Span::styled(
            format!("{:<5}  ", entry.tid),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }

    if cfg.tag && layout.tag_w > 0 {
        let tag_display = if entry.tag.len() > 24 {
            &entry.tag[..24]
        } else {
            &entry.tag
        };
        prefix.push(Span::styled(
            format!("{:<24} ", tag_display),
            tag_style(&entry.tag),
        ));
    }

    // Determine message style
    let is_stack = STACKTRACE_MARKERS
        .iter()
        .any(|m| entry.message.trim_start().starts_with(m));
    let msg_style = if is_stack {
        Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::DIM | Modifier::ITALIC)
    } else {
        level_style(entry.level)
    };

    // Wrap message text
    let msg_lines = wrap_text(&entry.message, layout.msg_w);

    let padding_str: String = " ".repeat(layout.prefix_w);
    let mut lines: Vec<Line<'a>> = Vec::with_capacity(msg_lines.len());

    for (i, chunk) in msg_lines.iter().enumerate() {
        let msg_spans = if !highlight.is_empty() && !is_stack {
            highlight_spans(chunk, highlight, msg_style)
        } else {
            vec![Span::styled(chunk.clone(), msg_style)]
        };

        if i == 0 {
            let mut spans = prefix.clone();
            spans.extend(msg_spans);
            lines.push(Line::from(spans));
        } else {
            let mut spans = vec![Span::styled(
                padding_str.clone(),
                Style::default(),
            )];
            spans.extend(msg_spans);
            lines.push(Line::from(spans));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(prefix));
    }

    lines
}

fn render_suggestions(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<Line> = app
        .suggestions
        .iter()
        .enumerate()
        .take(area.height as usize)
        .map(|(i, s)| {
            if Some(i) == app.suggestion_idx {
                Line::from(Span::styled(
                    format!(" > {s}"),
                    Style::default().fg(Color::Black).bg(Color::Cyan),
                ))
            } else {
                Line::from(Span::styled(
                    format!("   {s}"),
                    Style::default().fg(Color::White).bg(Color::DarkGray),
                ))
            }
        })
        .collect();

    let paragraph =
        Paragraph::new(Text::from(items)).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mut parts: Vec<Span> = Vec::new();

    if let Some(ref dev) = app.adb.selected_device {
        let name = if !dev.model.is_empty() {
            &dev.model
        } else {
            &dev.serial
        };
        parts.push(Span::styled(
            format!(" {name} "),
            Style::default().fg(Color::Black).bg(Color::Green),
        ));
        parts.push(Span::raw(" "));
    }

    if !app.filters.package.is_empty() {
        parts.push(Span::styled(
            format!(" {} ", app.filters.package),
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ));
        parts.push(Span::raw(" "));
    }

    if app.paused && !app.auto_scroll {
        parts.push(Span::styled(
            format!(" SCROLL +{} ", app.scroll_offset),
            Style::default().fg(Color::White).bg(Color::Yellow),
        ));
        parts.push(Span::raw(" "));
        parts.push(Span::styled(
            " PageDown to resume ",
            Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
        ));
    } else if app.paused {
        parts.push(Span::styled(
            " PAUSED ",
            Style::default().fg(Color::White).bg(Color::Red),
        ));
    } else if app.streaming {
        parts.push(Span::styled(
            " STREAMING ",
            Style::default().fg(Color::Black).bg(Color::Green),
        ));
    }

    if app.traffic.is_running() {
        parts.push(Span::raw(" "));
        parts.push(Span::styled(
            " PROXY ",
            Style::default().fg(Color::Black).bg(Color::Magenta),
        ));
    }

    let count_text = format!(" {} lines ", app.log_lines.len());
    let used: usize = parts.iter().map(|s| s.width()).sum();
    let remaining = (area.width as usize).saturating_sub(used + count_text.len());
    parts.push(Span::styled(
        " ".repeat(remaining),
        Style::default().bg(Color::DarkGray),
    ));
    parts.push(Span::styled(
        count_text,
        Style::default()
            .fg(Color::Gray)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    ));

    let bar = Paragraph::new(Line::from(parts)).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(bar, area);
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let prompt_str = "logux > ";
    let prompt_w = prompt_str.chars().count();
    let total_w = area.width as usize;
    if total_w <= prompt_w {
        return;
    }
    let input_w = total_w - prompt_w;

    let chars: Vec<char> = app.input.chars().collect();
    let cursor_char = app.input[..app.cursor_pos].chars().count();

    // Horizontal scroll: keep the cursor visible with a small right margin.
    // If input fits, no scroll. Otherwise ensure cursor is within [scroll, scroll+input_w-1].
    let scroll = if chars.len() < input_w {
        0
    } else if cursor_char >= input_w {
        cursor_char + 1 - input_w
    } else {
        0
    };

    let visible_end = (scroll + input_w).min(chars.len());
    let visible: String = chars[scroll..visible_end].iter().collect();

    // Leading ellipsis indicator when scrolled
    let leading = if scroll > 0 { "…" } else { "" };
    let visible_display = if !leading.is_empty() && !visible.is_empty() {
        // Replace first visible char with ellipsis to keep width constant
        let mut it = visible.chars();
        it.next();
        format!("{leading}{}", it.collect::<String>())
    } else {
        visible
    };

    let prompt = Span::styled(
        prompt_str,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let input_span = Span::raw(visible_display);
    let paragraph = Paragraph::new(Line::from(vec![prompt, input_span]));
    frame.render_widget(paragraph, area);

    let cx = area.x + prompt_w as u16 + (cursor_char - scroll) as u16;
    if cx < area.x + area.width {
        frame.set_cursor_position((cx, area.y));
    }
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

/// Wrap text to fit within `width`, breaking at spaces when possible.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut pos = 0;
    let bytes = text.as_bytes();

    while pos < text.len() {
        let remaining = &text[pos..];
        if remaining.len() <= width {
            lines.push(remaining.to_string());
            break;
        }

        // Find the farthest char-boundary at or before pos+width
        let mut end = (pos + width).min(text.len());
        while end > pos && !text.is_char_boundary(end) {
            end -= 1;
        }

        // Try to break at whitespace within the chunk
        let chunk = &text[pos..end];
        let break_at = chunk
            .rfind(|c: char| c.is_whitespace())
            .filter(|&p| p > chunk.len() / 4)
            .map(|p| pos + p)
            .unwrap_or(end);

        lines.push(text[pos..break_at].to_string());

        pos = break_at;
        // Skip the whitespace at break point
        while pos < text.len() && bytes.get(pos).map_or(false, |b| b.is_ascii_whitespace()) {
            pos += 1;
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Highlight occurrences of `needle` within `text`.
fn highlight_spans<'a>(text: &str, needle: &str, base: Style) -> Vec<Span<'a>> {
    if needle.is_empty() {
        return vec![Span::styled(text.to_string(), base)];
    }

    let lower = text.to_lowercase();
    let lower_needle = needle.to_lowercase();
    let mut spans = Vec::new();
    let mut pos = 0;

    let hl = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    while let Some(idx) = lower[pos..].find(&lower_needle) {
        let abs = pos + idx;
        if abs > pos {
            spans.push(Span::styled(text[pos..abs].to_string(), base));
        }
        spans.push(Span::styled(
            text[abs..abs + needle.len()].to_string(),
            hl,
        ));
        pos = abs + needle.len();
    }

    if pos < text.len() {
        spans.push(Span::styled(text[pos..].to_string(), base));
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base));
    }
    spans
}

// ---------------------------------------------------------------------------
// Styling helpers
// ---------------------------------------------------------------------------

fn level_style(level: LogLevel) -> Style {
    match level {
        LogLevel::Verbose => Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::DIM),
        LogLevel::Debug => Style::default().fg(Color::Blue),
        LogLevel::Info => Style::default().fg(Color::Green),
        LogLevel::Warn => Style::default().fg(Color::Yellow),
        LogLevel::Error => Style::default().fg(Color::Red),
        LogLevel::Fatal => Style::default()
            .fg(Color::White)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD),
        LogLevel::Silent => Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::DIM),
    }
}

fn tag_style(tag: &str) -> Style {
    const COLORS: &[Color] = &[
        Color::Cyan,
        Color::Magenta,
        Color::LightBlue,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightMagenta,
        Color::LightCyan,
        Color::Blue,
        Color::LightRed,
        Color::Yellow,
    ];
    let mut hasher = DefaultHasher::new();
    tag.hash(&mut hasher);
    let idx = (hasher.finish() as usize) % COLORS.len();
    Style::default().fg(COLORS[idx])
}

// ---------------------------------------------------------------------------
// ANSI parser (for system messages only)
// ---------------------------------------------------------------------------

fn parse_ansi_line(s: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_text = String::new();
    let mut current_style = Style::default();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if !current_text.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(&mut current_text),
                    current_style,
                ));
            }
            if chars.peek() == Some(&'[') {
                chars.next();
                let mut seq = String::new();
                loop {
                    match chars.peek() {
                        Some(&c) if c.is_ascii_alphabetic() => {
                            chars.next();
                            if c == 'm' {
                                current_style = parse_sgr(&seq);
                            }
                            break;
                        }
                        Some(_) => seq.push(chars.next().unwrap()),
                        None => break,
                    }
                }
            }
        } else {
            current_text.push(ch);
        }
    }

    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }
    if spans.is_empty() {
        Line::from("")
    } else {
        Line::from(spans)
    }
}

fn parse_sgr(seq: &str) -> Style {
    if seq.is_empty() || seq == "0" {
        return Style::default();
    }
    let mut style = Style::default();
    for param in seq.split(';') {
        match param {
            "0" => style = Style::default(),
            "1" => style = style.add_modifier(Modifier::BOLD),
            "2" => style = style.add_modifier(Modifier::DIM),
            "3" => style = style.add_modifier(Modifier::ITALIC),
            "4" => style = style.add_modifier(Modifier::UNDERLINED),
            "30" => style = style.fg(Color::Black),
            "31" => style = style.fg(Color::Red),
            "32" => style = style.fg(Color::Green),
            "33" => style = style.fg(Color::Yellow),
            "34" => style = style.fg(Color::Blue),
            "35" => style = style.fg(Color::Magenta),
            "36" => style = style.fg(Color::Cyan),
            "37" => style = style.fg(Color::White),
            "90" => style = style.fg(Color::DarkGray),
            "91" => style = style.fg(Color::LightRed),
            "92" => style = style.fg(Color::LightGreen),
            "93" => style = style.fg(Color::LightYellow),
            "94" => style = style.fg(Color::LightBlue),
            "95" => style = style.fg(Color::LightMagenta),
            "96" => style = style.fg(Color::LightCyan),
            "97" => style = style.fg(Color::White),
            "40" => style = style.bg(Color::Black),
            "41" => style = style.bg(Color::Red),
            "42" => style = style.bg(Color::Green),
            "43" => style = style.bg(Color::Yellow),
            "44" => style = style.bg(Color::Blue),
            "45" => style = style.bg(Color::Magenta),
            "46" => style = style.bg(Color::Cyan),
            "47" => style = style.bg(Color::White),
            _ => {}
        }
    }
    style
}

// ---------------------------------------------------------------------------
// Mouse handling
// ---------------------------------------------------------------------------

fn handle_mouse_event(kind: MouseEventKind, app: &mut App) {
    match kind {
        MouseEventKind::ScrollUp => {
            let max = app.log_lines.len().saturating_sub(1);
            app.scroll_offset = (app.scroll_offset + 3).min(max);
            app.auto_scroll = false;
            if !app.paused && app.streaming {
                app.paused = true;
            }
        }
        MouseEventKind::ScrollDown => {
            if app.scroll_offset > 3 {
                app.scroll_offset -= 3;
            } else {
                app.scroll_offset = 0;
                app.auto_scroll = true;
                if app.paused && app.streaming {
                    app.resume();
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

async fn handle_key_event(key: KeyEvent, app: &mut App) {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') => {
                app.should_exit = true;
                return;
            }
            KeyCode::Char('l') => {
                app.all_lines.clear();
                app.log_lines.clear();
                app.scroll_offset = 0;
                return;
            }
            KeyCode::Char('u') => {
                app.input.clear();
                app.cursor_pos = 0;
                app.show_suggestions = false;
                return;
            }
            KeyCode::Char('a') => {
                app.cursor_pos = 0;
                return;
            }
            KeyCode::Char('e') => {
                app.cursor_pos = app.input.len();
                return;
            }
            KeyCode::Char('w') => {
                if app.cursor_pos > 0 {
                    let before = &app.input[..app.cursor_pos];
                    let trimmed = before.trim_end();
                    let new_end = trimmed
                        .rfind(|c: char| c.is_whitespace() || c == '/')
                        .map(|i| {
                            let ch = trimmed[i..].chars().next().unwrap();
                            i + ch.len_utf8()
                        })
                        .unwrap_or(0);
                    app.input.drain(new_end..app.cursor_pos);
                    app.cursor_pos = new_end;
                    app.update_suggestions();
                }
                return;
            }
            _ => {}
        }
    }

    // Shift+Up/Down for scrolling (alternative to PageUp/Down)
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Up => {
                let max = app.log_lines.len().saturating_sub(1);
                app.scroll_offset = (app.scroll_offset + 5).min(max);
                app.auto_scroll = false;
                if !app.paused && app.streaming {
                    app.paused = true;
                }
                return;
            }
            KeyCode::Down => {
                if app.scroll_offset > 5 {
                    app.scroll_offset -= 5;
                } else {
                    app.scroll_offset = 0;
                    app.auto_scroll = true;
                    if app.paused && app.streaming {
                        app.resume();
                    }
                }
                return;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Enter => handle_enter(app).await,

        KeyCode::Backspace => {
            if app.cursor_pos > 0 {
                let prev = app.input[..app.cursor_pos]
                    .chars()
                    .next_back()
                    .unwrap();
                app.cursor_pos -= prev.len_utf8();
                app.input.remove(app.cursor_pos);
                app.update_suggestions();
            }
        }
        KeyCode::Delete => {
            if app.cursor_pos < app.input.len() {
                let cur = app.input[app.cursor_pos..].chars().next().unwrap();
                for _ in 0..cur.len_utf8() {
                    app.input.remove(app.cursor_pos);
                }
                app.update_suggestions();
            }
        }

        KeyCode::Left => {
            if app.cursor_pos > 0 {
                let prev = app.input[..app.cursor_pos]
                    .chars()
                    .next_back()
                    .unwrap();
                app.cursor_pos -= prev.len_utf8();
            }
        }
        KeyCode::Right => {
            if app.cursor_pos < app.input.len() {
                let next = app.input[app.cursor_pos..]
                    .chars()
                    .next()
                    .unwrap();
                app.cursor_pos += next.len_utf8();
            }
        }
        KeyCode::Home => app.cursor_pos = 0,
        KeyCode::End => app.cursor_pos = app.input.len(),

        KeyCode::Up => {
            if app.show_suggestions {
                match app.suggestion_idx {
                    Some(idx) if idx > 0 => app.suggestion_idx = Some(idx - 1),
                    None if !app.suggestions.is_empty() => {
                        app.suggestion_idx = Some(app.suggestions.len() - 1)
                    }
                    _ => {}
                }
            } else if !app.history.is_empty() {
                match app.history_idx {
                    None => {
                        app.history_saved_input = app.input.clone();
                        app.history_idx = Some(app.history.len() - 1);
                        app.input = app.history.last().unwrap().clone();
                    }
                    Some(idx) if idx > 0 => {
                        app.history_idx = Some(idx - 1);
                        app.input = app.history[idx - 1].clone();
                    }
                    _ => {}
                }
                app.cursor_pos = app.input.len();
            }
        }

        KeyCode::Down => {
            if app.show_suggestions {
                match app.suggestion_idx {
                    Some(idx) if idx + 1 < app.suggestions.len() => {
                        app.suggestion_idx = Some(idx + 1)
                    }
                    None if !app.suggestions.is_empty() => app.suggestion_idx = Some(0),
                    _ => {}
                }
            } else if let Some(idx) = app.history_idx {
                if idx + 1 < app.history.len() {
                    app.history_idx = Some(idx + 1);
                    app.input = app.history[idx + 1].clone();
                } else {
                    app.history_idx = None;
                    app.input = std::mem::take(&mut app.history_saved_input);
                }
                app.cursor_pos = app.input.len();
            }
        }

        KeyCode::Tab => {
            if app.show_suggestions {
                if app.suggestion_idx.is_some() {
                    app.apply_suggestion();
                } else if !app.suggestions.is_empty() {
                    app.suggestion_idx = Some(0);
                }
            } else {
                app.update_suggestions();
                if app.suggestions.len() == 1 {
                    app.apply_suggestion();
                }
            }
        }

        KeyCode::Esc => {
            app.show_suggestions = false;
            app.suggestions.clear();
            app.suggestion_idx = None;
        }

        KeyCode::PageUp => {
            let max = app.log_lines.len().saturating_sub(1);
            let step = 30;
            app.scroll_offset = (app.scroll_offset + step).min(max);
            app.auto_scroll = false;
            if !app.paused && app.streaming {
                app.paused = true;
            }
        }
        KeyCode::PageDown => {
            let step = 30;
            if app.scroll_offset > step {
                app.scroll_offset -= step;
            } else {
                app.scroll_offset = 0;
                app.auto_scroll = true;
                if app.paused && app.streaming {
                    app.resume();
                }
            }
        }

        KeyCode::Char(c) => {
            app.input.insert(app.cursor_pos, c);
            app.cursor_pos += c.len_utf8();
            app.update_suggestions();
        }

        _ => {}
    }
}

async fn handle_enter(app: &mut App) {
    if app.show_suggestions && app.suggestion_idx.is_some() {
        app.apply_suggestion();
        return;
    }

    let input = app.input.trim().to_string();
    if input.is_empty() {
        return;
    }

    if app.history.last().map_or(true, |l| l != &input) {
        app.history.push(input.clone());
    }
    app.history_idx = None;
    app.show_suggestions = false;
    app.suggestions.clear();

    app.input.clear();
    app.cursor_pos = 0;

    // Device selection by number
    if app.adb.selected_device.is_none() {
        if let Ok(num) = input.parse::<usize>() {
            let devices: Vec<_> = app.adb.list_devices().to_vec();
            let online: Vec<_> = devices.into_iter().filter(|d| d.is_online()).collect();
            if num >= 1 && num <= online.len() {
                let dev = online[num - 1].clone();
                let name = dev.display_name();
                app.adb.selected_device = Some(dev);
                app.push_system(format!("\x1b[32mSelected: {name}\x1b[0m"));
                return;
            }
        }
    }

    if input == "/clear" {
        app.all_lines.clear();
        app.log_lines.clear();
        app.scroll_offset = 0;
        return;
    }

    if input.starts_with('/') {
        // /filter or /filter edit — populate input with current filters for editing
        if input == "/filter edit" || input == "/filter" {
            let edit_str = app.filters.to_edit_string();
            app.input = format!("/filter set {edit_str}");
            app.cursor_pos = app.input.len();
            // Show saved filter presets as suggestions
            app.suggestions = crate::config::list_filter_presets()
                .into_iter()
                .map(|(name, expr)| format!("/filter set {expr}  # {name}"))
                .collect();
            app.show_suggestions = !app.suggestions.is_empty();
            app.suggestion_idx = None;
            return;
        }

        // Lift auto-pause from previous command output
        if app.paused && app.auto_scroll {
            app.resume();
        }

        let mut output = Vec::new();
        let mut exit_requested = false;
        {
            let mut ctx = CommandContext {
                adb: &mut app.adb,
                filters: &mut app.filters,
                formatter: &mut app.formatter,
                traffic: &mut app.traffic,
                mock_engine: &mut app.mock_engine,
                streaming: &mut app.streaming,
                paused: &mut app.paused,
                save_path: &mut app.save_path,
                exit_requested: &mut exit_requested,
                output: &mut output,
            };
            dispatch(&mut ctx, &input).await;
        }

        // Auto-pause for informational commands so output isn't buried.
        // Skip for stream-control and single-line action confirmations.
        let cmd = input.split_whitespace().next().unwrap_or("");
        let is_control = matches!(
            cmd,
            "/stop" | "/pause" | "/resume" | "/app" | "/pid" | "/tag"
                | "/level" | "/grep" | "/msg" | "/regex" | "/connect" | "/disconnect"
                | "/clear" | "/exit" | "/quit" | "/q" | "/save"
                | "/format" | "/fields" | "/exclude"
        );
        if !is_control && !output.is_empty() && app.streaming && !app.paused {
            app.paused = true;
        }

        if !output.is_empty() {
            app.push_system(String::new());
            for line in output {
                app.push_system(line);
            }
            app.push_system(String::new());
        }

        if exit_requested {
            app.should_exit = true;
            return;
        }

        if !app.streaming {
            app.stop_stream();
        }

        // Re-filter the entire buffer with updated filters
        app.rebuild_filtered();

        // Save current filters for the active app
        if !app.filters.package.is_empty() {
            let edit = app.filters.to_edit_string();
            crate::config::save_app_filters(&app.filters.package, &edit);
        }

        // Track app history
        if input.starts_with("/app ") {
            let pkg = input.strip_prefix("/app ").unwrap_or("").trim();
            if !pkg.is_empty() {
                crate::config::save_app_to_history(pkg);
                if !app.app_history.contains(&pkg.to_string()) {
                    app.app_history.push(pkg.to_string());
                }
            }
        }
    } else {
        // Quick filter
        app.filters.set_text(&input);
        app.formatter.highlight_text = input.clone();
        app.push_system(format!("\x1b[32mQuick filter: '{input}'\x1b[0m"));
        app.rebuild_filtered();
        if !app.filters.package.is_empty() {
            let edit = app.filters.to_edit_string();
            crate::config::save_app_filters(&app.filters.package, &edit);
        }
    }
}
